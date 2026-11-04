# abs_kb_svc_v1_desktop

知识库**协议 v1 的桌面端**：`kb_admin_desktop`（及其 Rust 侧）与 `kb_core`
之间交换的数据，以及两个层面的握手消息。

本 crate **只定义数据与语义**：

- 不实现任何传输（不知道 UDS、TCP、socket、运行时）；
- 不定义 RPC 的方法契约——那在
  [`abs_kb_svc_v1_desktop_rpc`](../abs_kb_svc_v1_desktop_rpc/)；
- 不依赖任何异步运行时，也不依赖 `serde`。

`abs_kb_svc` 把本 crate 聚合到稳定路径 `abs_kb_svc::v1::desktop::<类型名>`，
调用方不必跟着 crate 拆分改路径。

## 1. 协议里有什么

| 方向 | 类型 | 数量 |
| :--- | :--- | :--- |
| 客户端 → 服务端 | `Request` | 17 个变体 |
| 服务端 → 客户端 | `Reply` | 13 个变体（含 `Error`） |
| 服务端主动推送 | `Event` | 9 个变体 |

按数据概念分文件，由 `lib.rs` 逐个 `pub use`，公开路径始终是
`abs_kb_svc_v1_desktop::<类型名>`：

| 文件 | 内容 |
| :--- | :--- |
| `ids_` | `WorkspaceId` / `SessionId` / `TurnId` / `ServiceId` / `RequestId` / `LocalId` |
| `handshake_` | `PROTOCOL_VERSION`、`ClientInfo`、`ServerInfo`、`ServerState`；系统层通知的转出 |
| `content_` | `Turn`、`TurnState`、`TextDelta`、`ToolCallRecord`、`TokenUsage`、`Notice` |
| `service_` | `ServiceSummary` / `ServiceList` / `UpsertServiceRequest` / `ApiKeyUpdate` |
| `workspace_` | `Workspace` / `WorkspaceList` / `SessionSummary` / `SessionList` / `SessionDetail` |
| `fs_` | `DirectoryListing` / `DirEntry` / `DirEntryKind` |
| `error_` | `ErrorCode` / `ErrorReply` |
| `request_` / `reply_` / `event_` | 三个方向的消息 |
| `envelope_` | `RequestEnvelope` / `ReplyEnvelope`（`request_id` 关联） |

## 2. 线格式：denco MessagePack

本机 MPTP body、跨机 TCP 帧与本地存储都走 `mp_denco`（MessagePack）。
MessagePack 是**位置编码**：

> 结构体字段顺序与枚举变体顺序就是线上契约，**只允许在末尾追加**；
> 插入或重排会让对端解出错误的值。

- 协议类型只保留 `denco` 派生，不使用 serde；
- **禁止** `#[serde(...)]` 风格的内部标签、`skip_serializing_if`、`flatten` 等
  “自描述格式才安全”的写法；
- `tests/wire_round_trip.rs` 用真 MessagePack 编解码守住代表性消息的往返。

本机 MPTP 与跨机 TCP 都用同一套协议类型；TCP 只是在外面多了一层帧头
（见 `kb_core_rproxy_wire`）。

## 3. 两个层面的握手

| 层面 | 解决什么 | 消息 | 定义在 |
| :--- | :--- | :--- | :--- |
| **系统层** | 找得到、连得上 | `IpcReadyNotice`（stdout 一行 JSON，带 UDS socket 路径） | [`abs_kb_core_handshake`](../abs_kb_core_handshake/)，本 crate 转出 |
| **应用层** | 谈得成 | `Request::Hello(ClientInfo)` → `Reply::Hello(ServerInfo)` | 本 crate 的 `handshake_` |

- 系统层只承诺“socket 已 bind”，客户端仍要重试；
- 应用层校验协议版本：版本不一致时服务端明确拒绝，而不是“尽力而为”；
- 两层消息都是公开协议，但**机制**（端点类型、路径、重试）归传输实现。

## 4. RPC 契约在另一个 crate

本 crate 只放数据；“有哪些方法、参数与返回是什么、业务错误怎么过线”
在 [`abs_kb_svc_v1_desktop_rpc`](../abs_kb_svc_v1_desktop_rpc/)：

```text
#[rpc(path = "kb/v1", services = [TrHandshake, TrWorkspaceService, TrSessionService, TrGeneration])]
pub struct KbApiV1;
```

| service | 方法 |
| :--- | :--- |
| `handshake` | `hello(ClientInfo) -> ServerInfo` |
| `workspace` | `list_workspaces` / `add_workspace` / `remove_workspace` / `rename_workspace` |
| `session` | `list_sessions` / `create_session` / `get_session` / `remove_session` / `rename_session` |
| `generation` | `ask(AskRequest) -> SessionDetail`（当前是同步一问一答） |

业务错误不走“正常应答里的 `Error` 变体”那套本机信封，而是：

- 服务端把 `ErrorCode` 映射到 460–466 的 MPTP 状态码；
- 客户端从 `RpcError::Remote.body` 剥离上游 Display 前缀，还原 `ErrorReply`；
- 业务失败与传输失败分成 `KbRpcError::Business` / `KbRpcError::Transport`。

前缀格式由契约 crate 的单元测试钉住。

## 5. 当前覆盖范围

| 能力 | 状态 |
| :--- | :--- |
| 应用层握手 / 版本校验 | ✅ |
| 工作区增删查改 | ✅ |
| 会话增删查改、读取正文 | ✅ |
| 提问（`Ask`） | ✅ 同步一问一答；`kb_core` 里是临时模拟的 LLM |
| 设置（`ListServices` / `UpsertService` / …） | 数据已定义；RPC service 未接 |
| 目录浏览（`ListDirectory`） | 数据已定义；RPC service 未接 |
| 事件推送（`Event` 9 种） | 类型已定义；服务端尚未推送 |
| `Cancel` | 数据已定义；流式生成落地时再接 |

## 6. 实现方必须遵守的契约

这一节针对**传输 / RPC 实现**（`kb_svc_mptp_ipc`、`kb_core_rproxy_wire`、
桌面客户端的连接管理器），不是数据 crate 自身的代码：

1. **future 里禁止阻塞。**
   由 trait 返回的 future，其 `poll` 不得执行会阻塞 OS 线程的操作
   （`recv`、`select`、同步文件 IO、不可控的锁等待）。阻塞要收敛到实现方
   自有的 IO 线程上。
2. **完成量必须运行时可无关。**
   只使用 `core::task::Waker` / `futures_core` 层面的原语；公开 API 里不出现
   tokio / compio / async-std 的私有类型。
3. **唤醒不得丢失。**
   “IO 线程先完成、异步侧后注册 Waker”的时序必须正确处理。
4. **取消必须可达。**
   调用方丢弃 future 或令牌触发后，实现要么发显式取消信号，要么让连接断开被
   对端感知；不得让对端永久等待。
5. **背压必须在语义层表达。**
   不依赖传输层“自带背压”；TCP 上行由 `kb_core_rproxy` 的有界通道做，
   本机 MPTP 由 `smux` 的子流窗口做。
6. **错误要分层。**
   传输失败（断开、编解码、取消）与业务失败（不存在、缺 API Key）不得混在
   同一个类型里。
7. **字段 / 变体顺序是契约。**
   新增字段只能追加；需要“重命名”时不要改动线上位置。

## 7. 其它约定

- 所有标识都是新类型，避免把会话标识传给需要回合标识的位置；
- **API key 永不明文回传**：服务端只回掩码（`MASKED_API_KEY`）；
- 时间统一用“自 Unix 纪元起的毫秒数”（`i64`），不传格式化字符串；
- **只发送差异，不发送整棵树**：工作区 / 会话是扁平列表 + 按需拉取，
  客户端自己组装界面结构。

## 8. 验证

```bash
cargo test -p abs_kb_svc_v1_desktop
```

`tests/wire_round_trip.rs` 用真 MessagePack 编解码覆盖代表性消息的往返。

## 9. 相关文档

- [`abs_kb_svc_v1_desktop_rpc`](../abs_kb_svc_v1_desktop_rpc/)：RPC 契约与错误映射；
- [`abs_kb_core_handshake/README.md`](../abs_kb_core_handshake/README.md)：系统层握手；
- [`abs_kb_svc/README.md`](../abs_kb_svc/README.md)：聚合层；
- [`kb_core/README.md`](../kb_core/README.md)：服务端如何实现这些方法；
- [`kb_svc_mptp_ipc`](../../../kb_plugins/crates/kb_svc_mptp_ipc/)：本机 UDS 传输；
- [`kb_core_rproxy_wire`](../../../kb_plugins/crates/kb_core_rproxy_wire/)：跨机 TCP 帧。
