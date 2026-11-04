# kb_client_config

`kb_admin_desktop` **自己**的连接配置：启动时读它，决定"怎么找到 / 启动 `kb_core`"。

`kb_core` 不知道也不需要知道这份文件——它是客户端的本地选择。

## 生命周期

```text
启动
  ├─ ClientConfig::load(path)
  │     ├─ Ok(config) 且非空  → 启动页默认选中 config.startup_connection()
  │     │                       （last_used → default → 第一条）
  │     ├─ Ok(config) 但空    → 配置向导（空列表 = 还没有配置过）
  │     ├─ Err(NotFound)      → 首次运行：问用户"怎么连"
  │     │                       → 组装 ClientConfig → save(path)
  │     └─ Err(Parse/…)       → 让用户修，**不要**静默覆盖
  └─ 连接（由 kb_client_conn_mgr 负责）；用户选中 / 倒计时开始时
     set_last_used(名字) + save(path)，下次启动就能默认选中它
```

"文件不存在"与"文件坏了"是**两件事**：前者是 `ConfigError::NotFound`（界面进
首次运行），后者是 `ConfigError::Parse`（界面提示用户去改）。

## 格式：TOML

```toml
version = 1
default = "本机"      # 用户偏好（可省）
last_used = "实验室"  # 最近一次启动 / 连接用的（客户端写回，可省）

[[connections]]
name = "本机"
kind = "local-launch"
kb_core = "/usr/local/bin/kb-core"
runtime_dir = "/run/user/1000/llm_kb"
storage_dir = "/home/me/.local/share/kb_admin_desktop/storage" # 必填：自己选一个持久目录
handshake_timeout_millis = 10000   # 可省

[[connections]]
name = "已在本机跑着的"
kind = "local-attach"
runtime_dir = "/run/user/1000/llm_kb"

[[connections]]
name = "实验室"
kind = "tcp"
address = "192.168.1.5:8788"
connect_timeout_millis = 5000      # 可省
request_timeout_millis = 15000     # 可省
```

三种 `kind` 的语义见 `kb_client_conn_mgr` 的 README；这里只负责"怎么存"。
省掉的时限用本 crate 的缺省常量（`DEFAULT_*_TIMEOUT`）。

**`storage_dir` 没有默认值**：`local-launch` 必须显式给出一个持久目录。本 crate
刻意不提供“运行时目录下的 storage”这种缺省，避免把知识库放进
`$XDG_RUNTIME_DIR` 之类的 tmpfs / 临时目录。

`default` 与 `last_used` 都必须是 `connections` 里已经存在的名字（可以缺省）：

| 字段 | 谁写 | 用途 |
| :--- | :--- | :--- |
| `default` | 用户在连接对话框里保存时 | 用户偏好；`last_used` 缺省时的回退 |
| `last_used` | 启动流程（`ClientConfig::set_last_used`） | 启动页默认选中 / 倒计时目标 |

启动选择顺序是 `last_used` → `default` → 第一条。旧配置文件没有 `last_used`
也能直接读，行为回退到 `default`。

## 路径

配置文件的优先级：

1. 显式覆盖 `KB_ADMIN_DESKTOP_CONFIG`（开发、排错、或多份配置并存时用）；
2. 便携模式 `KB_ADMIN_DESKTOP_HOME/config.toml`；
3. 平台约定目录：

| 平台 | 位置 |
| :--- | :--- |
| Linux 等 | `$XDG_CONFIG_HOME/kb_admin_desktop/config.toml`（缺省 `~/.config/…`） |
| macOS | `~/Library/Application Support/kb_admin_desktop/config.toml` |
| Windows | `%APPDATA%\kb_admin_desktop\config.toml` |

### 便携模式

设置 `KB_ADMIN_DESKTOP_HOME=<base>` 后，配置与运行时目录都落在同一棵树下：

```text
<base>/
├── config.toml       # 连接配置
├── run/              # 默认运行时目录（UDS / 端点名字文件）
└── data/             # 可选；存储目录仍由用户自己选
```

存储目录**不会**自动变成 `<base>/data`：它必须由界面让用户明确选择。

### 默认运行时目录

| 情况 | 默认值 |
| :--- | :--- |
| 便携模式 | `<base>/run` |
| 普通模式 | `$XDG_RUNTIME_DIR/llm_kb`（回退系统临时目录下的 `llm_kb`） |

普通模式的这条规则与 `kb_core` 自己的缺省推导一致；便携模式是显式 opt-in，
不会改变普通用户的 XDG 行为。便携模式只是**客户端**的约定：手工运行 `kb-core`
时仍要显式给它 `--runtime-dir` / `--storage-dir`。

## 写是原子的

`ClientConfig::save` 先写同目录下的 `.tmp` 再 `rename`，所以读到的要么是旧内容、
要么是新内容，不会是被写了一半的 TOML——与 `kb_core` 存储层的约定一致。

## 校验

`load` / `save` 都会先 `validate()`：版本必须是 `CONFIG_VERSION`、名字非空且不重复、
`default` / `last_used` 必须指向存在的项、必填字段非空。
手改配置写错 `kind` 会在解析阶段就报出来，不会留到连接时才炸。

**`connections` 可以为空**：这是“还没有配置过”的信号，界面据此运行配置向导；
空列表时 `startup_connection()` / `default_connection()` 返回
`ConfigError::NoConnections`，但文件本身是合法的。

## 验证

```bash
cargo test -p kb_client_config
```

12 个单元测试 + 2 个文档测试：TOML 往返（含 `last_used`）、显式时限保留、
六种坏配置被拒、空列表合法（配置向导信号）、"文件不存在"与"文件坏了"分开、
原子写往返、缺省项选择、启动选择顺序（`last_used` → `default` → 第一条）、
旧配置兼容、便携 base / XDG 的路径优先级、`kind` 字符串对称。

## 相关文档

- [`kb_client_conn_mgr`](../kb_client_conn_mgr/README.md)：拿这里的一条连接方式去连 `kb_core`；
- 根 `README.md` §2：各 crate 的分工。
