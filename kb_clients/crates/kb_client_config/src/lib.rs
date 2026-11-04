//! # kb_client_config
//!
//! `kb_admin_desktop` **自己**的连接配置：启动时读它，决定"怎么找到 / 启动
//! `kb_core`"。
//!
//! 这份文件与 `kb_core` 无关——服务端不知道也不需要知道它。它是客户端的本地
//! 选择：本机起一个 `kb_core`、附着到已经在跑的那个、还是经 `kb_core_rproxy`
//! 连远程的。
//!
//! # 生命周期
//!
//! ```text
//! 启动
//!   ├─ ClientConfig::load(path)
//!   │     ├─ Ok(config) 且连接非空   → 启动页默认选中 config.startup_connection()
//!   │     │                            （last_used → default → 第一条）
//!   │     ├─ Ok(config) 但列表为空   → 配置向导（空列表是“还没有配置过”的信号）
//!   │     ├─ Err(NotFound)           → 首次运行：问用户"怎么连"
//!   │     │                            → ClientConfig::with_single(…) / 手工组装
//!   │     │                            → config.save(path)
//!   │     └─ Err(Parse/…)            → 让用户修，**不要**静默覆盖
//!   └─ 连接（由 kb_client_conn_mgr 负责）；用户选中 / 倒计时开始时
//!      set_last_used(名字) + save(path)，下次启动就能默认选中它
//! ```
//!
//! 路径由 [`path`] 决定，优先级是：
//!
//! 1. 显式环境变量 [`CONFIG_PATH_ENV`](path::CONFIG_PATH_ENV)；
//! 2. 便携模式 [`PORTABLE_HOME_ENV`](path::PORTABLE_HOME_ENV) 下的 `<base>/config.toml`；
//! 3. 平台约定目录（Linux `~/.config/…`、macOS `Application Support`、Windows `%APPDATA%`）。
//!
//! 便携模式还会把默认运行时目录变成 `<base>/run`。**存储目录没有默认值**：
//! `local-launch` 必须由用户明确选一个持久目录——`kb_client_config` 不提供
//! “放到运行时目录下的 storage”这种缺省，避免把知识库放进 tmpfs 或临时目录。
//!
//! # 格式：TOML
//!
//! ```toml
//! version = 1
//! default = "本机"          # 用户偏好（可缺省）
//! last_used = "实验室"      # 最近一次启动 / 连接用的（客户端写回，可缺省）
//!
//! [[connections]]
//! name = "本机"
//! kind = "local-launch"
//! kb_core = "/usr/local/bin/kb-core"
//! runtime_dir = "/run/user/1000/llm_kb"
//! storage_dir = "/home/me/.local/share/kb_admin_desktop/storage" # 必填：自己选一个持久目录
//! ```
//!
//! 三种 `kind` 与各自的字段见 [`Connection`] 的文档。`default` 与 `last_used`
//! 都指向 `connections` 里已经存在的名字：前者是用户偏好，后者是启动页自动
//! 写回的「上次用的那条」。
//!
//! # 写是原子的
//!
//! [`ClientConfig::save`] 先写同目录下的 `.tmp` 再 `rename`，所以读到的要么是
//! 旧内容、要么是新内容，不会是被写了一半的 TOML——与 `kb_core` 存储层的约定一致。
//!
//! # 示例
//!
//! ```
//! use kb_client_config::{ClientConfig, Connection};
//!
//! // 首次运行：界面问完用户之后，组装一条连接方式并写出去。
//! let config = ClientConfig::with_single(Connection::local_launch(
//!     "本机",
//!     "/usr/local/bin/kb-core",
//!     "/run/user/1000/llm_kb",
//!     "/run/user/1000/llm_kb/storage",
//! ));
//!
//! let text = config.to_toml().expect("应当能序列化");
//! assert!(text.contains("kind = \"local-launch\""), "实际 TOML:\n{text}");
//!
//! // 下次启动：读回来，取启动页默认该选的那一条。
//! let mut parsed = ClientConfig::from_toml(&text).expect("应当能解析");
//! assert_eq!(parsed.startup_connection().expect("有一条").name(), "本机");
//!
//! // 用户选中 / 倒计时开始自动执行时，记下“最近一次”。
//! parsed.set_last_used("本机").expect("名字存在");
//! assert_eq!(parsed.last_used.as_deref(), Some("本机"));
//! ```

pub mod path;

mod config_;
mod error_;

pub use config_::{
    CONFIG_VERSION, ClientConfig, Connection, ConnectionKind, DEFAULT_CONNECT_TIMEOUT,
    DEFAULT_HANDSHAKE_TIMEOUT, DEFAULT_REQUEST_TIMEOUT,
};
pub use error_::ConfigError;
pub use path::{
    APP_DIR, CONFIG_FILE, CONFIG_PATH_ENV, PORTABLE_HOME_ENV, config_path, default_config_path,
    default_runtime_dir, portable_home,
};
