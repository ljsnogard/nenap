# kb_core

知识库**主进程**（可执行文件 `kb-core`）：持有工作区与会话的存储，
把按域 RPC 实现在本机 UDS 上服务给客户端。

## 1. 当前状态

**已经跑通的**：

- 工作区 / 会话的增删查改、读取会话正文；
- 应用层握手与协议版本校验；
- 本机 UDS 上的 MPTP RPC（`kb_svc_mptp_ipc`）；
- 提问 `Ask`：`kb_core` 里是**临时模拟的 LLM**，把问题按字符逆序输出当回答，
  一问一答落盘，并按 `turn_id` 幂等；
- 跨机路径：由 `kb_core_rproxy` 启动本进程并把 TCP 请求转成同一套 RPC。

**还没接的**：设置域、目录浏览域、流式生成 / `Cancel`、并发服务多个客户端、
优雅退出、鉴权，以及把本地文件存储换成 Turso。

本进程**一次只服务一个客户端**：上一个断开后才回到 `accept`。这够桌面客户端用，
也让端点生命周期保持简单。

## 2. 先跑起来

### 2.1 启动服务

```bash
cargo run -p kb_core
```

预期日志（目录与时间戳随环境变化）：

```text
[INFO kb_core::serve_] kb_core v0.1.0（协议 v1）
[INFO kb_core::serve_] 运行时目录: /run/user/1000/llm_kb
[INFO kb_core::serve_] 存储目录: /run/user/1000/llm_kb/storage
[INFO kb_core::serve_] 本机 socket: /run/user/1000/llm_kb/kb-20261105-<uuid>.sock
[INFO kb_core::serve_] 已登记工作区: 0 个
[INFO kb_core::serve_] 等待客户端连接（Ctrl-C 退出）
```

进程会一直运行，直到 `Ctrl-C`。客户端（桌面端 / `kb_core_rproxy`）在
**运行时目录**里找这个 socket 连上来：

```text
运行时目录/
└── kb-<日期>-<uuid>.sock     本次启动专属；进程活着时就在
```

- 文件名由 `kb_core` 决定，每次启动独一无二；
- 启动时 `Listener::bind` 会清理运行时目录里残留的 `kb-*.sock` / `kb-*.ipc`；
- socket 权限设为 0600（仅当前用户）。

如果 `kb_core` 是被**父进程**启动的，让父进程去扫目录猜名字既有竞态又有歧义，
所以有一个显式通道：

```bash
kb-core --handshake-prompt stdio --runtime-dir /tmp/kb-demo/run --storage-dir /tmp/kb-demo/data
```

它会在 stdout 上打**一行** JSON：

```json
{"event":"ipc_ready","ipc_name_file":"…/kb-20261105-<uuid>.sock","protocol_version":1,"pid":1234}
```

- 类型是 [`abs_kb_core_handshake::IpcReadyNotice`](../abs_kb_core_handshake/)；
- 字段名沿用 `ipc_name_file`（消息兼容），**语义是 socket 路径**；
- 默认 `none`：stdout 保持干净，只有显式要求时才打。

这是**系统层握手**——只解决“父进程知道连哪里”。应用层握手是
`Request::Hello` → `Reply::Hello`，由 `kb_svc_mptp_ipc::Client` 在连上之后发起。

### 2.2 从零跑一轮 CRUD

有两条等价的路：**走 IPC**（真实链路）或**用临时子命令**（不开客户端）。
两者共用同一个存储层。

#### 走 IPC

```rust
use kb_svc_mptp_ipc::Client;

let client = Client::connect_with_timeout("/tmp/kb-core/run", std::time::Duration::from_secs(5))?;
let workspaces = client.list_workspaces().await?;
```

#### 用临时子命令

```bash
cargo build -p kb_core

kb() { ./target/debug/kb-core \
  --runtime-dir /tmp/kb-core/run \
  --storage-dir /tmp/kb-core/data "$@"; }

WID=$(kb workspace add --name 笔记 --path /tmp/notes | cut -f1)
SID=$(kb session add --workspace "$WID" --title 第一问 | cut -f1)

kb workspace list
kb session list --workspace "$WID"
kb session show  --workspace "$WID" "$SID"

kb workspace update "$WID" --name 资料库
kb session update   --workspace "$WID" "$SID" --title 改名后

kb session remove   --workspace "$WID" "$SID"
kb workspace remove "$WID"
```

`workspace list` / `session list` 每行是制表符分隔的字段，最后一行 `# 共 N 个`，
所以 `cut -f1` 能取到标识。`session show` 为了方便人读，把 MessagePack 存储
转成 JSON 打印；存储本身仍是 `.mp`。

### 2.3 自动化验证

```bash
cargo test -p kb_core
```

**27 个库测试 + 1 个端到端集成测试**，覆盖存储语义、命令行解析，以及
“真起 `kb-core` 子进程 → UDS → MPTP → 存储”的整条链路。

## 3. 命令行参考

```text
kb-core [--runtime-dir <目录>] [--storage-dir <目录>] [<子命令>]
```

### 3.1 全局选项

| 选项 | 默认值 | 说明 |
| :--- | :--- | :--- |
| `--runtime-dir <目录>` | `$XDG_RUNTIME_DIR/llm_kb`（回退系统临时目录下的 `llm_kb`） | 运行时目录；UDS socket 放在这里。 |
| `--storage-dir <目录>` | `<运行时目录>/storage` | 工作区与会话的存储目录。显式给出时与运行时目录完全独立。 |
| `--handshake-prompt <方式>` | `none` | `none`：stdout 保持干净；`stdio`：向 stdout 打一行 JSON 通知，供启动本进程的父进程读取。 |
| `-h` / `--help` | — | 打印用法说明。 |

> `--storage-dir` 的这个默认值只服务于**手工运行 `kb-core`**。桌面客户端选择
> `local-launch` 时**没有**存储默认值，必须让用户明确选一个持久目录
> （见 `kb_client_config`），并显式把 `--storage-dir` 传给本进程。

### 3.2 子命令

| 子命令 | 作用 |
| :--- | :--- |
| `workspace add --name <名字> --path <目录>` | 新建工作区，标识由 `kb_core` 生成。 |
| `workspace list` | 列出全部工作区（按标识升序）。 |
| `workspace show <工作区标识>` | 查看单个工作区。 |
| `workspace update <工作区标识> [--name <名字>] [--path <目录>]` | 改名字 / 改路径，两者至少给一个。 |
| `workspace remove <工作区标识>` | 删除工作区，并级联删除它名下的会话。 |
| `session add --workspace <工作区标识> [--title <标题>]` | 新建会话；不给标题时从首条用户消息推导。 |
| `session list --workspace <工作区标识>` | 列出该工作区下的会话（按最近活动时间降序）。 |
| `session show --workspace <工作区标识> <会话标识>` | 打印会话完整内容（JSON 仅用于人读）。 |
| `session update --workspace <工作区标识> <会话标识> --title <标题>` | 改会话标题。 |
| `session remove --workspace <工作区标识> <会话标识>` | 删除会话。 |

不带子命令即进入常驻服务。选项与位置参数**顺序不敏感**；不支持
`--key=value` 与短选项合并。

### 3.3 退出码

| 退出码 | 含义 |
| :---: | :--- |
| `0` | 成功（常驻服务被信号终止时由信号决定）。 |
| `1` | 运行期失败：标识不合法、目标不存在、文件读写失败等。 |
| `2` | 参数错误：未知选项 / 子命令、缺参数、多余参数。 |

常驻服务没有安装信号处理器，`Ctrl-C` 走操作系统默认处置。

## 4. 数据存在哪里

工作区与会话暂时用**本地文件**代替 Turso。文件内容是
`abs_kb_svc::v1::desktop` 协议类型的 **MessagePack** 表示——存储格式与线上格式
同源，不会出现“能存进去、却发不出去”的字段。

```text
<storage_dir>/
├── workspaces/
│   └── <workspace_id>.mp          一个工作区（Workspace）
└── sessions/
    └── <workspace_id>/
        └── <session_id>.mp        一个会话（SessionDetail：摘要 + 全部消息）
```

四条硬性约定（都有测试守着）：

1. **标识必须能安全地当文件名**：只允许 ASCII 字母、数字、`-`、`_`，且不超过
   128 字节。协议把标识当作不透明字符串，因此不能假定它一定由 `generate()`
   产生；含 `../` 的标识会在碰盘之前被拒绝。
2. **写入先落临时文件再 `rename`**：读到的内容要么是旧的、要么是新的，不会是
   半个文件。进程被强杀时最坏留下一个 `*.mp.tmp`，它不会被当作对象。
3. **会话属于工作区**：`create_session` / `list_sessions` 会先确认工作区存在；
   删除工作区会级联删除它的会话目录。工作区不存在时 `list_sessions`
   **返回错误而不是空列表**，避免客户端把标识写错时误以为“这里没有会话”。
4. **空会话不落盘**：一个会话要么有消息，要么有一个**非默认**的名字
   （默认名是「新会话」）；两者都不满足时 `create_session` / `save_session`
   返回 `StoreError::EmptySession`。这样“新建会话”必须等第一次提问
   （或先起个名字）才会在磁盘上出现。

## 5. 设计要点

### 5.1 运行时：compio

- `main` 用 `#[compio::main]`；
- 本地文件读写走 `compio::fs`；
- 目录遍历、递归删除这类 compio 0.19 还没有异步版本的操作，交给
  `compio::runtime::spawn_blocking`，不占执行器线程；
- `Listener::accept` 是异步的，服务循环不阻塞执行器。

### 5.2 传输：UDS + smux_v1 + mptp_rpc

- `kb_svc_mptp_ipc::Listener::bind(runtime_dir)` 生成
  `kb-<日期>-<uuid>.sock`，清理残留，chmod 0600；
- 客户端 `Client::connect_with_timeout(runtime_dir, timeout)` 扫最新的 socket，
  连接、smux 握手，然后按域发 RPC；
- 服务端 `Listener::accept` 之后用 `Connection::serve(KbServiceSet { .. })`
  服务四个按域 service；
- 一次一个连接；没有事件推送。
- 细节见 [`kb_svc_mptp_ipc`](../../../kb_plugins/crates/kb_svc_mptp_ipc/) 与
  [dev-notes/kb_svc_mptp_ipc-20261105-0345.md](../../../dev-notes/kb_svc_mptp_ipc-20261105-0345.md)。

### 5.3 业务错误

`KbService` 把 `StoreError` 翻成协议里的业务错误：

| `StoreError` | `ErrorCode` |
| :--- | :--- |
| `NotFound` | `NotFound` |
| `InvalidId` / `EmptySession` / `EmptyName` | `BadRequest` |
| 其它（I/O、解码、阻塞任务） | `Internal` |

到线上是 460–466 的 MPTP 状态码；客户端剥离前缀还原 `ErrorReply`。
业务错误与传输错误的分层见
[`abs_kb_svc_v1_desktop/README.md`](../abs_kb_svc_v1_desktop/README.md) §4。

### 5.4 取消一路贯到存储

`ipc_` 的每个方法由 `gen_mcf2` 展开，并**把请求带的取消令牌继续传给
`Store`**；`Store` 的每个操作也是可取消 future。取消的落点是
`store_::race_cancel_`：整个操作体与取消信号赛跑，令牌先触发就返回
`StoreError::Cancelled`，尚未完成的等待随 future 一起被丢弃。

`Store` 上没有裸 `async fn`；私有辅助只在已被令牌包住的操作体里调用，
不再各自包一层宏。

### 5.5 生成域是临时的

`TrGeneration::ask` 目前没有真正的 LLM 插件可接，实现成“把问题按字符逆序输出”：

- 一次提问落两条消息（`user` + 逆序的 `assistant`）；
- 应答是提问之后的完整会话；
- 用 `turn_id` 幂等：同一次提问重发不会重复添加用户回合；
- 助手回合挂一条非错误的 `Notice`，提示“临时模拟”。

等流式生成与事件落地时，`Ask` 的返回类型会换成流形状。

## 6. 模块地图

| 文件 | 职责 |
| :--- | :--- |
| `src/main.rs` | 入口：初始化日志 → 解析参数 → 分发 → 决定退出码 |
| `src/args_.rs` | 命令行与子命令解析；`USAGE` 是唯一的用法说明来源 |
| `src/error_.rs` | 进程级错误：把存储失败与 IPC 失败收在一处 |
| `src/serve_.rs` | 常驻服务：打开存储、`Listener::bind`、循环 accept + serve |
| `src/ipc_.rs` | `KbService`：四个按域 RPC service 的服务端实现（转发给 `Store`） |
| `src/cli_.rs` | 临时的 CRUD 子命令实现 |
| `src/store_/` | 本地文件存储：`Store`、错误类型、布局与标识校验 |

## 7. 一次请求的完整路径

```text
kb_admin_desktop / 测试客户端
    │  Client::list_workspaces()                  ← kb_svc_mptp_ipc 的代理
    │  ── MPTP Call /kb/v1/workspace ──►          ← UDS + smux 子流
    │                       Connection::serve
    │                       │  KbApiV1 路由
    │                       ▼
    │                   KbWorkspace::list_workspaces
    │                       │
    │                       ▼
    │                   Store::list_workspaces()
    │                       │  读 workspaces/*.mp
    │  ◄── WorkspaceList ───┘
    └─ 客户端拿到 WorkspaceList
```

跨机时，`kb_core_rproxy` 在中间把 TCP 帧（`RequestEnvelope` / `ReplyEnvelope`）
翻成同一套 typed RPC；`kb_core` 不需要知道请求来自本机还是远程。

## 8. 下一轮

1. 设置域、目录浏览域；
2. 流式生成、事件推送与 `Cancel`；
3. 并发服务多个客户端；
4. 优雅退出（撤下 socket、通知在连客户端）；
5. 把本地文件存储换成 Turso。

## 9. 相关文档

- [`abs_kb_svc_v1_desktop/README.md`](../abs_kb_svc_v1_desktop/README.md)：协议数据与实现契约；
- [`abs_kb_svc_v1_desktop_rpc`](../abs_kb_svc_v1_desktop_rpc/)：`#[rpc]` 契约；
- [`abs_kb_core_handshake/README.md`](../abs_kb_core_handshake/README.md)：系统层握手；
- [`kb_core_starter/README.md`](../../../kb_plugins/crates/kb_core_starter/README.md)：父进程如何启动本进程；
- [`kb_core_rproxy/README.md`](../../../kb_plugins/crates/kb_core_rproxy/README.md)：跨机网关；
- [dev-notes/kb_core-20261105-0345.md](../../../dev-notes/kb_core-20261105-0345.md)：存储与服务端设计原理。
