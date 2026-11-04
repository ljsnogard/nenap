# abs_kb_core_handshake

**系统层握手**：`kb_core` 公布自己本机 UDS 端点的那条通知。

```text
kb_core（子进程）                            启动它的父进程
  Listener::bind(runtime_dir)
  → 往 stdout 打一行 IpcReadyNotice（JSON） ──►  读这一行，得到 socket 路径
                                                 → 之后照常去连 UDS / TCP 端点
```

## 两个层面的握手

客户端要跟 `kb_core` 谈成事，要过两道握手；**两边的消息都是公开协议**，
区别只在“消息之外的机制”归谁：

| 层面 | 解决什么 | 消息 | 定义在 |
| :--- | :--- | :--- | :--- |
| **系统层** | 找得到、连得上 | `IpcReadyNotice` + `HandshakeNoticeKind`（`event: "ipc_ready"`） | **本 crate** |
| **应用层** | 谈得成 | `PROTOCOL_VERSION` / `ClientInfo` / `ServerInfo` / `Request::Hello` → `Reply::Hello` | `abs_kb_svc_v1_desktop` |

“机制”指挑哪种内核端点、端点放在哪、失败怎么重试——这些由传输实现决定
（当前是 `kb_svc_mptp_ipc` 的 UDS + `smux_v1` + `mptp_rpc`）。

## 消息

```json
{"event":"ipc_ready","ipc_name_file":"…/kb-20261105-<uuid>.sock","protocol_version":1,"pid":1234}
```

- 字段名是 `ipc_name_file`（消息兼容），但值**直接就是 socket 路径**，不是另一个
  需要再打开的文件；
- `protocol_version` 只是报备；真正“版本不一致就拒绝”发生在应用层握手；
- `pid` 用于日志与排错。

编解码统一走 `json_denco`（denco 的 JSON 实现），不依赖 serde，也不使用 postcard。
通知很小，编码用一块 4 KiB 的栈缓冲，不需要堆分配。

## 谁发、谁收

| 谁 | 做什么 |
| :--- | :--- |
| `kb_core`（`serve_`） | `Listener::bind` 之后用 `encode_notice` 编出 JSON，`writeln!` 到 stdout 并 flush；stdout 其余时间保持干净（默认 `--handshake-prompt none`） |
| `kb_core_starter` | `decode_notice` 解析那一行，取出 `ipc_name_file`（socket 路径） |

共用同一份定义，所以改字段名 / 改 `event` 取值会编译不过，而不是“跑起来才发现
启动不了”。

## 它承诺什么、不承诺什么

- 只承诺 **socket 已经 bind**；拿到通知之后服务端可能还没进入 `accept`；
- 因此连接方仍要带**重试与超时**（`kb_svc_mptp_ipc::Client::connect_with_timeout`）；
- 独立运行、不需要父进程转告端点的客户端（自己扫运行时目录）不经过这条通知。

## 为什么是一个独立的 crate

因为**只想启动并找到 `kb_core` 的调用方不该依赖整套业务协议**：

- `kb_core_starter` 只需要“端点就绪”这一条消息；
- 如果这条通知留在业务协议 crate，starter 会连带依赖工作区 / 会话 / 服务类型
  以及背后的 `abs_llm` 等；
- 本 crate 的依赖面只有 denco / `abs_cancel` / 消息宏，不依赖任何运行时、传输或业务实现。

## 验证

```bash
cargo test -p abs_kb_core_handshake
```

2 个单元测试：通知 JSON 形状与往返、未知 `event` 被拒绝。

## 相关文档

- [`abs_kb_svc_v1_desktop/README.md`](../abs_kb_svc_v1_desktop/README.md)：应用层握手与业务协议；
- [`kb_core_starter/README.md`](../../../kb_plugins/crates/kb_core_starter/README.md)：本 crate 的主要调用方；
- [`kb_svc_mptp_ipc`](../../../kb_plugins/crates/kb_svc_mptp_ipc/)：当前 UDS 传输实现。
