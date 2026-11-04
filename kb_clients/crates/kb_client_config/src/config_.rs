//! 连接配置的内容：`kb_admin_desktop` 怎么找到 / 启动 `kb_core`。
//!
//! 格式是 **TOML**（给人手改的：有注释、嵌套不吵）；这份文件**不是**协议，
//! 只在客户端自己这一侧使用，因此可以用 serde 的内部标签
//! （`kind = "tcp"`）与 `skip_serializing_if`——协议类型走 denco MessagePack，
//! 那份文件有自己的字段顺序约定；TOML 配置不受那条禁令影响。
//!
//! # 三种连接方式
//!
//! ```toml
//! version = 1
//! startup_countdown_seconds = 10  # 启动页自动连接倒计时；0 = 关闭
//! default = "本机"          # 用户偏好（可缺省）
//! last_used = "实验室"      # 最近一次启动 / 连接用的（由客户端写回，可缺省）
//!
//! [[connections]]
//! name = "本机"
//! kind = "local-launch"          # 起一个本机 kb_core 并连它的 IPC
//! kb_core = "/usr/local/bin/kb-core"
//! runtime_dir = "/run/user/1000/llm_kb"
//! storage_dir = "/home/me/.local/share/kb_admin_desktop/storage" # 必填：自己选一个持久目录
//! handshake_timeout_millis = 10000
//!
//! [[connections]]
//! name = "已在本机跑着的"
//! kind = "local-attach"          # 只连已经在跑的 kb_core
//! runtime_dir = "/run/user/1000/llm_kb"
//! connect_timeout_millis = 5000
//!
//! [[connections]]
//! name = "实验室"
//! kind = "tcp"                   # 经 kb_core_rproxy 走 TCP
//! address = "192.168.1.5:8788"
//! connect_timeout_millis = 5000
//! request_timeout_millis = 15000
//! ```

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::error_::ConfigError;

/// 这份配置的格式版本。
///
/// 与协议版本无关：它只关乎这个文件怎么写。将来若改了字段形状，读的时候就能
/// 明确拒绝"看不懂的版本"，而不是把旧字段当成新字段用。
pub const CONFIG_VERSION: u32 = 1;

/// 启动页**自动连接倒计时**的默认秒数。
///
/// 配置文件里的 `startup_countdown_seconds` 可以覆盖它；`0` 表示关闭自动连接
/// （启动页只显示连接列表，等用户点「连接」）。
pub const DEFAULT_STARTUP_COUNTDOWN_SECONDS: u32 = 10;

/// 缺省的"等 `kb_core` 公布端点"的时长（`local-launch`）。
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// 缺省的"连上去"的时长（`local-attach` 与 `tcp`）。
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 缺省的单次请求时长。
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// 连接方式的种类。
///
/// 单独列一个枚举，是为了让界面能列出"有哪些可选方式"而不必解析字段；
/// TOML 里的取值是 [`ConnectionKind::as_str`] 给出的字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConnectionKind {
    /// 启动一个本机 `kb_core` 子进程，再连它的 IPC。
    LocalLaunch,

    /// 只连已经在跑的本机 `kb_core`（扫运行时目录里的 UDS socket）。
    LocalAttach,

    /// 经 `kb_core_rproxy` 走 TCP。
    Tcp,
}

impl ConnectionKind {
    /// 配置文件里的取值（也是界面上的标识）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalLaunch => "local-launch",
            Self::LocalAttach => "local-attach",
            Self::Tcp => "tcp",
        }
    }

    /// 从配置文件里的取值解析；不认识时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "local-launch" => Some(Self::LocalLaunch),
            "local-attach" => Some(Self::LocalAttach),
            "tcp" => Some(Self::Tcp),
            _ => None,
        }
    }

    /// 界面上给人看的一句话说明。
    pub fn description(self) -> &'static str {
        match self {
            Self::LocalLaunch => "启动一个本机的 kb_core，并连上它的 IPC",
            Self::LocalAttach => "连接已经在跑的本机 kb_core",
            Self::Tcp => "经 kb_core_rproxy 连接远程 kb_core（无鉴权，仅受信网络）",
        }
    }
}

/// 一条连接方式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Connection {
    /// 起一个本机 `kb_core` 子进程，再连它。
    LocalLaunch {
        /// 界面上显示的名字，也是 `default` 引用的键。
        name: String,

        /// `kb_core` 可执行文件。
        kb_core: PathBuf,

        /// 运行时目录（UDS socket 放在这里）。
        runtime_dir: PathBuf,

        /// 知识库数据目录。
        ///
        /// **必填，没有默认值**：界面必须让用户明确选一个持久目录。
        /// 本 crate 刻意不提供“运行时目录下的 storage”这种缺省，避免把知识库
        /// 放进 `$XDG_RUNTIME_DIR` 之类的 tmpfs / 临时目录。
        storage_dir: PathBuf,

        /// 等 `kb_core` 公布端点的时限；缺省 [`DEFAULT_HANDSHAKE_TIMEOUT`]。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handshake_timeout_millis: Option<u64>,
    },

    /// 只连已经在跑的本机 `kb_core`。
    LocalAttach {
        /// 界面上显示的名字。
        name: String,

        /// 运行时目录。
        runtime_dir: PathBuf,

        /// 连上去的时限；缺省 [`DEFAULT_CONNECT_TIMEOUT`]。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connect_timeout_millis: Option<u64>,
    },

    /// 经 `kb_core_rproxy` 走 TCP。
    Tcp {
        /// 界面上显示的名字。
        name: String,

        /// 网关地址，`主机:端口`。
        address: String,

        /// 建立 TCP 连接的时限；缺省 [`DEFAULT_CONNECT_TIMEOUT`]。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connect_timeout_millis: Option<u64>,

        /// 单次请求的时限；缺省 [`DEFAULT_REQUEST_TIMEOUT`]。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_timeout_millis: Option<u64>,
    },
}

impl Connection {
    /// 造一条"启动本机 `kb_core`"的连接，时限用缺省值。
    ///
    /// `storage_dir` **必须由调用方给出**：本 crate 没有存储目录的默认值。
    pub fn local_launch(
        name: impl Into<String>,
        kb_core: impl Into<PathBuf>,
        runtime_dir: impl Into<PathBuf>,
        storage_dir: impl Into<PathBuf>,
    ) -> Self {
        Self::LocalLaunch {
            name: name.into(),
            kb_core: kb_core.into(),
            runtime_dir: runtime_dir.into(),
            storage_dir: storage_dir.into(),
            handshake_timeout_millis: None,
        }
    }

    /// 造一条"附着到已在跑的 `kb_core`"的连接，时限用缺省值。
    pub fn local_attach(name: impl Into<String>, runtime_dir: impl Into<PathBuf>) -> Self {
        Self::LocalAttach {
            name: name.into(),
            runtime_dir: runtime_dir.into(),
            connect_timeout_millis: None,
        }
    }

    /// 造一条 TCP 连接，时限用缺省值。
    pub fn tcp(name: impl Into<String>, address: impl Into<String>) -> Self {
        Self::Tcp {
            name: name.into(),
            address: address.into(),
            connect_timeout_millis: None,
            request_timeout_millis: None,
        }
    }

    /// 界面上显示的名字。
    pub fn name(&self) -> &str {
        match self {
            Self::LocalLaunch { name, .. }
            | Self::LocalAttach { name, .. }
            | Self::Tcp { name, .. } => name,
        }
    }

    /// 种类。
    pub fn kind(&self) -> ConnectionKind {
        match self {
            Self::LocalLaunch { .. } => ConnectionKind::LocalLaunch,
            Self::LocalAttach { .. } => ConnectionKind::LocalAttach,
            Self::Tcp { .. } => ConnectionKind::Tcp,
        }
    }

    /// 等 `kb_core` 公布端点的时限（只有 `local-launch` 用得上）。
    pub fn handshake_timeout(&self) -> Duration {
        match self {
            Self::LocalLaunch {
                handshake_timeout_millis,
                ..
            } => millis_or_(*handshake_timeout_millis, DEFAULT_HANDSHAKE_TIMEOUT),
            _ => DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }

    /// 建立连接的时限。
    pub fn connect_timeout(&self) -> Duration {
        match self {
            Self::LocalLaunch { .. } => DEFAULT_CONNECT_TIMEOUT,
            Self::LocalAttach {
                connect_timeout_millis,
                ..
            }
            | Self::Tcp {
                connect_timeout_millis,
                ..
            } => millis_or_(*connect_timeout_millis, DEFAULT_CONNECT_TIMEOUT),
        }
    }

    /// 单次请求的时限。
    pub fn request_timeout(&self) -> Duration {
        match self {
            Self::Tcp {
                request_timeout_millis,
                ..
            } => millis_or_(*request_timeout_millis, DEFAULT_REQUEST_TIMEOUT),
            _ => DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// 检查这一条自身是否可用（名字非空、必填字段非空）。
    fn validate_(&self) -> Result<(), ConfigError> {
        if self.name().trim().is_empty() {
            return Err(ConfigError::EmptyName);
        }

        let empty = |field: &'static str| ConfigError::EmptyField {
            connection: self.name().to_string(),
            field,
        };

        match self {
            Self::LocalLaunch {
                kb_core,
                runtime_dir,
                storage_dir,
                ..
            } => {
                if kb_core.as_os_str().is_empty() {
                    return Err(empty("kb_core"));
                }
                if runtime_dir.as_os_str().is_empty() {
                    return Err(empty("runtime_dir"));
                }
                if storage_dir.as_os_str().is_empty() {
                    return Err(empty("storage_dir"));
                }
            }
            Self::LocalAttach { runtime_dir, .. } => {
                if runtime_dir.as_os_str().is_empty() {
                    return Err(empty("runtime_dir"));
                }
            }
            Self::Tcp { address, .. } => {
                if address.trim().is_empty() {
                    return Err(empty("address"));
                }
            }
        }

        Ok(())
    }
}

/// 整个配置文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientConfig {
    /// 格式版本（[`CONFIG_VERSION`]）。
    pub version: u32,

    /// 启动页自动连接倒计时的秒数。
    ///
    /// - 缺省 / 老配置没有这个字段：用 [`DEFAULT_STARTUP_COUNTDOWN_SECONDS`]（10 秒）；
    /// - `0`：关闭自动连接，启动页停在列表上等用户点「连接」；
    /// - 其它值：启动页默认选中 `startup_connection()`，倒数该秒数后自动连接。
    #[serde(default = "default_startup_countdown_seconds_")]
    pub startup_countdown_seconds: u32,

    /// 缺省用哪一条连接方式；缺省时用第一条。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    /// 最近一次被选中 / 尝试连接的连接方式（启动页默认选它）。
    ///
    /// 与 [`default`](Self::default) 的区别：`default` 是用户的偏好，
    /// `last_used` 是启动流程自动写回的「上次用的那条」。两者都可以缺省；
    /// 启动选择的顺序是 `last_used` → `default` → 第一条
    /// （见 [`ClientConfig::startup_connection`]）。
    ///
    /// 旧版本配置文件没有这个字段也能读：缺省即 `None`，行为回退到 `default`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<String>,

    /// 若干条连接方式。
    #[serde(default)]
    pub connections: Vec<Connection>,
}

impl ClientConfig {
    /// 造一份只有一条连接方式的配置。
    pub fn with_single(connection: Connection) -> Self {
        Self {
            version: CONFIG_VERSION,
            startup_countdown_seconds: DEFAULT_STARTUP_COUNTDOWN_SECONDS,
            default: Some(connection.name().to_string()),
            last_used: None,
            connections: vec![connection],
        }
    }

    /// 从 TOML 文本解析。
    ///
    /// # Errors
    ///
    /// 语法/字段不合法 → [`ConfigError::Parse`]；版本不认识、或校验不过
    /// （空名字、重名、`default` 指向不存在的项）→ 相应的 [`ConfigError`]。
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(text).map_err(ConfigError::Parse)?;
        config.validate()?;
        Ok(config)
    }

    /// 序列化成 TOML 文本。
    ///
    /// # Errors
    ///
    /// 字段都是基本类型，正常不会失败；失败时返回 [`ConfigError::Encode`]。
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self).map_err(ConfigError::Encode)
    }

    /// 读配置文件。
    ///
    /// 文件不存在时返回 [`ConfigError::NotFound`]——**这不是异常**：首次运行时
    /// 由界面问用户"怎么连"，再调用 [`ClientConfig::save`] 生成它。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigError`]。
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(ConfigError::NotFound {
                    path: path.to_path_buf(),
                });
            }
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        Self::from_toml(&text)
    }

    /// 原子地写配置文件：先写同目录下的 `.tmp`，再 `rename`。
    ///
    /// 目录不存在会先建出来。`rename` 保证读到的要么是旧内容、要么是新内容，
    /// 不会是被写了一半的 TOML——与 `kb_core` 存储层的约定一致。
    ///
    /// # Errors
    ///
    /// 校验不过 → 相应的 [`ConfigError`]；建目录/写/改名失败 →
    /// [`ConfigError::Write`]。
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        self.validate()?;
        let text = self.to_toml()?;

        let write_error = |source: std::io::Error| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(write_error)?;
        }

        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text).map_err(write_error)?;
        std::fs::rename(&temp, path).map_err(write_error)?;

        log::info!("连接配置已写入 {}", path.display());
        Ok(())
    }

    /// 检查整份配置是否自洽。
    ///
    /// # Errors
    ///
    /// - 版本不是 [`CONFIG_VERSION`]；
    /// - 名字为空 / 重名 / 必填字段为空；
    /// - `default` / `last_used` 指向不存在的连接方式。
    ///
    /// **`connections` 可以是空列表**：这表示“还没有配置过”，是启动页运行
    /// 配置向导的信号。空列表时 [`ClientConfig::startup_connection`] /
    /// [`ClientConfig::default_connection`] 仍然会返回
    /// [`ConfigError::NoConnections`]，调用方据此决定是不是要先引导用户。
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(self.version));
        }
        let mut names = BTreeSet::new();
        for connection in &self.connections {
            connection.validate_()?;
            if !names.insert(connection.name().to_string()) {
                return Err(ConfigError::DuplicateName(connection.name().to_string()));
            }
        }

        if let Some(default) = &self.default
            && !names.contains(default)
        {
            return Err(ConfigError::UnknownDefault(default.clone()));
        }

        if let Some(last_used) = &self.last_used
            && !names.contains(last_used)
        {
            return Err(ConfigError::UnknownLastUsed(last_used.clone()));
        }

        Ok(())
    }

    /// 按名字取一条连接方式。
    ///
    /// # Errors
    ///
    /// 没有这个名字时返回 [`ConfigError::UnknownConnection`]。
    pub fn connection(&self, name: &str) -> Result<&Connection, ConfigError> {
        self.connections
            .iter()
            .find(|connection| connection.name() == name)
            .ok_or_else(|| ConfigError::UnknownConnection(name.to_string()))
    }

    /// 缺省该用哪一条：`default` 指定的那条；没指定就用第一条。
    ///
    /// # Errors
    ///
    /// 一条都没有时返回 [`ConfigError::NoConnections`]。
    pub fn default_connection(&self) -> Result<&Connection, ConfigError> {
        match &self.default {
            Some(name) => self.connection(name),
            None => self.connections.first().ok_or(ConfigError::NoConnections),
        }
    }

    /// 启动页应当默认选中的那一条：`last_used` → `default` → 第一条。
    ///
    /// `last_used` 由启动流程在用户选中 / 倒计时开始时写回
    /// （见 [`ClientConfig::set_last_used`]）；它缺省时回退到用户的偏好
    /// `default`，再缺省就用列表里的第一条。
    ///
    /// # Errors
    ///
    /// 一条都没有时返回 [`ConfigError::NoConnections`]；`last_used` / `default`
    /// 指向不存在的项时返回 [`ConfigError::UnknownConnection`]。
    pub fn startup_connection(&self) -> Result<&Connection, ConfigError> {
        match self.last_used.as_deref().or(self.default.as_deref()) {
            Some(name) => self.connection(name),
            None => self.connections.first().ok_or(ConfigError::NoConnections),
        }
    }

    /// 把「最近一次使用」记为 `name`。
    ///
    /// 只改内存里的值；写盘由 [`ClientConfig::save`] 完成。调用方通常是启动流程：
    /// 用户点选一条连接、或者配置的倒计时开始自动执行时，先记下来再尝试连接。
    ///
    /// # Errors
    ///
    /// `name` 不是已经存在的连接方式时返回 [`ConfigError::UnknownConnection`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use kb_client_config::{ClientConfig, Connection};
    ///
    /// let mut config = ClientConfig::with_single(Connection::local_attach("本机", "/run/kb"));
    /// config.set_last_used("本机").expect("这个名字存在");
    /// assert_eq!(config.last_used.as_deref(), Some("本机"));
    /// assert!(config.set_last_used("不存在").is_err());
    /// ```
    pub fn set_last_used(&mut self, name: impl Into<String>) -> Result<(), ConfigError> {
        let name = name.into();
        if !self
            .connections
            .iter()
            .any(|connection| connection.name() == name)
        {
            return Err(ConfigError::UnknownConnection(name));
        }
        self.last_used = Some(name);
        Ok(())
    }
}

/// serde 缺省：老配置没有 `startup_countdown_seconds` 时用 10 秒。
fn default_startup_countdown_seconds_() -> u32 {
    DEFAULT_STARTUP_COUNTDOWN_SECONDS
}

/// 把"毫秒数"翻成时长；`None` 或 `0` 用缺省值。
fn millis_or_(millis: Option<u64>, fallback: Duration) -> Duration {
    match millis {
        Some(0) | None => fallback,
        Some(millis) => Duration::from_millis(millis),
    }
}

#[cfg(test)]
mod tests_ {
    use super::*;

    /// 组装一份覆盖三种连接方式的配置。
    fn sample_() -> ClientConfig {
        ClientConfig {
            version: CONFIG_VERSION,
            startup_countdown_seconds: DEFAULT_STARTUP_COUNTDOWN_SECONDS,
            default: Some("本机".to_string()),
            last_used: Some("实验室".to_string()),
            connections: vec![
                Connection::local_launch(
                    "本机",
                    "/usr/local/bin/kb-core",
                    "/run/kb",
                    "/run/kb/storage",
                ),
                Connection::local_attach("附着", "/run/kb"),
                Connection::tcp("实验室", "192.168.1.5:8788"),
            ],
        }
    }

    /// 测试 TOML 往返：三种 `kind` 都能写出去、读回来，取值一字不差。
    ///
    /// - 手段：把覆盖三种连接方式的配置序列化成 TOML，再解析回来。
    /// - 判断：往返结果与原值相等；`kind` 写成 kebab-case；缺省时限不写进文件
    ///   （`handshake_timeout_millis` 之类只在显式给过时才出现），这样手写的配置
    ///   文件干净，而"缺省值是什么"由常量单独决定。
    #[test]
    fn config_round_trips_through_toml_() {
        let config = sample_();
        let text = config.to_toml().expect("应当能序列化");

        assert!(
            text.contains("kind = \"local-launch\""),
            "实际 TOML:\n{text}"
        );
        assert!(
            text.contains("kind = \"local-attach\""),
            "实际 TOML:\n{text}"
        );
        assert!(text.contains("kind = \"tcp\""), "实际 TOML:\n{text}");
        assert!(
            text.contains("last_used = \"实验室\""),
            "最近一次使用应当写进文件:\n{text}"
        );
        assert!(
            text.contains("startup_countdown_seconds = 10"),
            "倒计时应写进文件:\n{text}"
        );
        assert!(
            !text.contains("timeout_millis"),
            "没显式给过的时限不该写进文件:\n{text}"
        );

        let parsed = ClientConfig::from_toml(&text).expect("应当能解析");
        assert_eq!(parsed, config);
    }

    /// 测试显式时限会被保留，且认得出的取值换算正确。
    ///
    /// - 手段：造一条给了 `handshake_timeout_millis = 1234` 的 `local-launch`，
    ///   以及一条给了 `request_timeout_millis = 4321` 的 `tcp`，往返一次并与
    ///   缺省值对比。
    /// - 判断：往返相等；`handshake_timeout()` 是 1234ms，`request_timeout()` 是
    ///   4321ms；没给时限的 `local-attach` 用 [`DEFAULT_CONNECT_TIMEOUT`]。
    #[test]
    fn explicit_timeouts_survive_and_defaults_apply_() {
        let mut config = sample_();
        config.connections[0] = Connection::LocalLaunch {
            name: "本机".to_string(),
            kb_core: PathBuf::from("/usr/local/bin/kb-core"),
            runtime_dir: PathBuf::from("/run/kb"),
            storage_dir: PathBuf::from("/run/kb/storage"),
            handshake_timeout_millis: Some(1234),
        };
        config.connections[2] = Connection::Tcp {
            name: "实验室".to_string(),
            address: "192.168.1.5:8788".to_string(),
            connect_timeout_millis: None,
            request_timeout_millis: Some(4321),
        };

        let text = config.to_toml().expect("应当能序列化");
        let parsed = ClientConfig::from_toml(&text).expect("应当能解析");
        assert_eq!(parsed, config);

        assert_eq!(
            parsed.connections[0].handshake_timeout(),
            Duration::from_millis(1234)
        );
        assert_eq!(
            parsed.connections[2].request_timeout(),
            Duration::from_millis(4321)
        );
        assert_eq!(
            parsed.connections[1].connect_timeout(),
            DEFAULT_CONNECT_TIMEOUT
        );
    }

    /// 测试坏配置会被明确拒绝，而不是被"尽力而为"地接受。
    ///
    /// - 手段：分别构造"版本不认识"、"一条连接都没有"、"名字为空"、"名字重复"、
    ///   "`default` 指向不存在"、"`last_used` 指向不存在"、"tcp 地址为空"、
    ///   "`kind` 取值不认识"。
    /// - 判断：前六种各自得到对应的 [`ConfigError`] 变体；空列表是**合法**的
    ///   （启动页据此运行配置向导）；最后一种在解析阶段就失败
    ///   （[`ConfigError::Parse`]）——手改配置写错 `kind` 必须立刻报出来。
    #[test]
    fn bad_configs_are_rejected_() {
        let mut wrong_version = sample_();
        wrong_version.version = 99;
        assert!(matches!(
            wrong_version.validate(),
            Err(ConfigError::UnsupportedVersion(99))
        ));

        let empty = ClientConfig {
            version: CONFIG_VERSION,
            startup_countdown_seconds: DEFAULT_STARTUP_COUNTDOWN_SECONDS,
            default: None,
            last_used: None,
            connections: Vec::new(),
        };
        assert!(
            empty.validate().is_ok(),
            "空列表是合法配置：它是启动页运行配置向导的信号"
        );
        assert!(matches!(
            empty.startup_connection(),
            Err(ConfigError::NoConnections)
        ));

        let mut blank_name = sample_();
        blank_name.connections[1] = Connection::local_attach("   ", "/run/kb");
        assert!(matches!(blank_name.validate(), Err(ConfigError::EmptyName)));

        let mut duplicate = sample_();
        duplicate.connections[1] = Connection::local_attach("本机", "/run/kb");
        assert!(matches!(
            duplicate.validate(),
            Err(ConfigError::DuplicateName(name)) if name == "本机"
        ));

        let mut bad_default = sample_();
        bad_default.default = Some("不存在".to_string());
        assert!(matches!(
            bad_default.validate(),
            Err(ConfigError::UnknownDefault(name)) if name == "不存在"
        ));

        let mut bad_last_used = sample_();
        bad_last_used.last_used = Some("不存在".to_string());
        assert!(matches!(
            bad_last_used.validate(),
            Err(ConfigError::UnknownLastUsed(name)) if name == "不存在"
        ));

        let mut empty_address = sample_();
        empty_address.connections[2] = Connection::tcp("实验室", "  ");
        assert!(matches!(
            empty_address.validate(),
            Err(ConfigError::EmptyField {
                field: "address",
                ..
            })
        ));

        let text = r#"
version = 1
[[connections]]
name = "怪东西"
kind = "carrier-pigeon"
"#;
        assert!(matches!(
            ClientConfig::from_toml(text),
            Err(ConfigError::Parse(_))
        ));
    }

    /// 测试"没有配置文件"与"配置坏了"是两件事。
    ///
    /// - 手段：在一个空临时目录里 `load` 一个不存在的路径；再写一份坏 TOML 后
    ///   `load` 它。
    /// - 判断：前者是 [`ConfigError::NotFound`]（界面据此走"首次运行"分支），
    ///   后者是 [`ConfigError::Parse`]（界面应当让用户修，而不是静默覆盖）。
    #[test]
    fn missing_file_is_not_the_same_as_broken_file_() {
        let guard = tempfile::tempdir().expect("临时目录");
        let path = guard.path().join("config.toml");

        assert!(matches!(
            ClientConfig::load(&path),
            Err(ConfigError::NotFound { .. })
        ));

        std::fs::write(&path, "这不是 TOML").expect("应当能写");
        assert!(matches!(
            ClientConfig::load(&path),
            Err(ConfigError::Parse(_))
        ));
    }

    /// 测试保存是原子写（先临时文件再改名），且能原样读回。
    ///
    /// - 手段：把样例配置 `save` 到临时目录下**还不存在**的子目录里，再 `load`。
    /// - 判断：返回 `Ok`；读回来的配置与原值相等；目录被自动建出来；过程中没有
    ///   留下 `.tmp` 残留。
    #[test]
    fn save_is_atomic_and_round_trips_() {
        let guard = tempfile::tempdir().expect("临时目录");
        let path = guard.path().join("nested").join("config.toml");
        let config = sample_();

        config.save(&path).expect("应当能保存");

        assert!(path.is_file(), "应当写出配置文件");
        assert!(
            !path.with_extension("toml.tmp").exists(),
            "不应当留下临时文件"
        );
        assert_eq!(ClientConfig::load(&path).expect("应当能读回"), config);
    }

    /// 测试缺省连接的选择规则：`default` 优先，没有就用第一条。
    ///
    /// - 手段：分别用 `default = Some("实验室")`、`default = None` 调
    ///   `default_connection()`，再取一个不存在的名字。
    /// - 判断：前者给出"实验室"，中者给出第一条"本机"，后者返回
    ///   [`ConfigError::UnknownConnection`]。
    #[test]
    fn default_connection_selection_() {
        let mut config = sample_();

        config.default = Some("实验室".to_string());
        assert_eq!(
            config.default_connection().expect("应当能取到").name(),
            "实验室"
        );

        config.default = None;
        assert_eq!(
            config.default_connection().expect("应当能取到").name(),
            "本机"
        );

        assert!(matches!(
            config.connection("没有这个"),
            Err(ConfigError::UnknownConnection(name)) if name == "没有这个"
        ));
    }

    /// 测试启动页的选择顺序：最近一次使用 → 偏好默认 → 第一条。
    ///
    /// - 手段：样例配置里 `last_used = "实验室"`、`default = "本机"`；依次把
    ///   `last_used`、`default` 置空，观察 `startup_connection()` 的结果；
    ///   再用 `set_last_used` 改成一个存在的名字、一个不存在的名字。
    /// - 判断：三段分别给出「实验室」「本机」「本机」（第一条）；合法名字能更新
    ///   `last_used`，非法名字返回 [`ConfigError::UnknownConnection`] 且不改动原值。
    #[test]
    fn startup_connection_prefers_last_used_then_default_() {
        let mut config = sample_();
        assert_eq!(
            config.startup_connection().expect("应当能取到").name(),
            "实验室"
        );

        config.last_used = None;
        assert_eq!(
            config.startup_connection().expect("应当能取到").name(),
            "本机"
        );

        config.default = None;
        assert_eq!(
            config.startup_connection().expect("应当能取到").name(),
            "本机",
            "两者都缺省时回退到第一条"
        );

        config.set_last_used("附着").expect("名字存在");
        assert_eq!(config.last_used.as_deref(), Some("附着"));
        assert!(matches!(
            config.set_last_used("不存在"),
            Err(ConfigError::UnknownConnection(name)) if name == "不存在"
        ));
        assert_eq!(config.last_used.as_deref(), Some("附着"));
    }

    /// 测试倒计时的默认值、显式值与 `0` 语义。
    ///
    /// - 手段：解析三份 TOML：没有 `startup_countdown_seconds`、显式 `3`、
    ///   显式 `0`。
    /// - 判断：缺省是 [`DEFAULT_STARTUP_COUNTDOWN_SECONDS`]（10）；显式值原样保留；
    ///   `0` 合法（表示关闭自动连接）。
    #[test]
    fn startup_countdown_defaults_to_ten_and_zero_disables_() {
        let base = r#"
version = 1
startup_countdown_seconds = 3

[[connections]]
name = "本机"
kind = "local-attach"
runtime_dir = "/run/kb"
"#;

        let explicit = ClientConfig::from_toml(base).expect("显式值应当能读");
        assert_eq!(explicit.startup_countdown_seconds, 3);

        let defaulted =
            ClientConfig::from_toml(&base.replace("startup_countdown_seconds = 3\n", ""))
                .expect("缺省值应当能读");
        assert_eq!(
            defaulted.startup_countdown_seconds,
            DEFAULT_STARTUP_COUNTDOWN_SECONDS
        );
        assert_eq!(defaulted.startup_countdown_seconds, 10);

        let disabled = ClientConfig::from_toml(&base.replace(
            "startup_countdown_seconds = 3",
            "startup_countdown_seconds = 0",
        ))
        .expect("0 表示关闭，仍是合法配置");
        assert_eq!(disabled.startup_countdown_seconds, 0);
    }

    /// 测试旧配置文件（没有 `last_used`）仍能读，启动时回退到 `default`。
    ///
    /// - 手段：直接解析一份只有 `version` / `default` / 一条连接方式的 TOML。
    /// - 判断：`last_used` 是 `None`；`startup_connection()` 给出 `default` 那条
    ///   「本机」。这条保证加字段不会让老用户手里的配置变成"坏配置"。
    #[test]
    fn older_config_without_last_used_still_works_() {
        let text = r#"
version = 1
default = "本机"

[[connections]]
name = "本机"
kind = "local-attach"
runtime_dir = "/run/kb"
"#;
        let config = ClientConfig::from_toml(text).expect("旧配置应当能读");
        assert_eq!(config.last_used, None);
        assert_eq!(
            config.startup_countdown_seconds, DEFAULT_STARTUP_COUNTDOWN_SECONDS,
            "旧配置没有该字段时应当回退到 10 秒"
        );
        assert_eq!(config.startup_connection().expect("有一条").name(), "本机");
    }

    /// 测试空连接列表仍是合法配置，TOML 往返后仍为空。
    ///
    /// - 手段：造一份 `connections: []` 的配置，`validate`、`to_toml`、
    ///   `from_toml` 各走一遍。
    /// - 判断：校验通过；往返后仍没有连接方式；`startup_connection()` 返回
    ///   [`ConfigError::NoConnections`]——界面据此运行配置向导，而不是把文件判坏。
    #[test]
    fn empty_connections_are_valid_and_mean_first_run_() {
        let config = ClientConfig {
            version: CONFIG_VERSION,
            startup_countdown_seconds: DEFAULT_STARTUP_COUNTDOWN_SECONDS,
            default: None,
            last_used: None,
            connections: Vec::new(),
        };
        config.validate().expect("空列表应当合法");

        let text = config.to_toml().expect("应当能序列化");
        let parsed = ClientConfig::from_toml(&text).expect("应当能解析");
        assert!(parsed.connections.is_empty());
        assert_eq!(parsed.default, None);
        assert_eq!(parsed.last_used, None);
        assert!(matches!(
            parsed.startup_connection(),
            Err(ConfigError::NoConnections)
        ));
    }

    /// 测试三种 `kind` 的字符串与解析是对称的。
    ///
    /// - 手段：对 [`ConnectionKind`] 的三个取值做 `as_str` → `from_str` 往返，
    ///   再喂一个不认识的字符串。
    /// - 判断：往返得到同一个值；不认识的返回 `None`。这条钉住"界面下拉框里的取值"
    ///   与"配置文件里写的取值"是同一套。
    #[test]
    fn kind_strings_are_symmetric_() {
        for kind in [
            ConnectionKind::LocalLaunch,
            ConnectionKind::LocalAttach,
            ConnectionKind::Tcp,
        ] {
            assert_eq!(ConnectionKind::parse(kind.as_str()), Some(kind));
            assert!(!kind.description().is_empty());
        }
        assert_eq!(ConnectionKind::parse("carrier-pigeon"), None);
    }
}
