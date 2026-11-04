# abs_kb_svc_v1_desktop_rpc

桌面协议 v1 的 **`#[rpc]` 契约**：interface、四个按域 service、`Ack`、业务错误映射。

数据消息仍定义在 [`abs_kb_svc_v1_desktop`](../abs_kb_svc_v1_desktop/)；本 crate
只回答“哪些方法、什么路径、参数与返回是什么”，以及“业务错误怎么过线”。

## Interface

```rust
#[rpc(
    path = "kb/v1",
    services = [TrHandshake, TrWorkspaceService, TrSessionService, TrGeneration]
)]
pub struct KbApiV1;
```

完整路径是 `/{interface}/{service}`。四个 service：

| service | 方法 | 入参 | 出参 |
| :--- | :--- | :--- | :--- |
| `handshake` | `hello` | `ClientInfo` | `ServerInfo` |
| `workspace` | `list_workspaces` | – | `WorkspaceList` |
| | `add_workspace` | `AddWorkspaceRequest` | `Workspace` |
| | `remove_workspace` | `WorkspaceId` | `Ack` |
| | `rename_workspace` | `WorkspaceId`, `String` | `Workspace` |
| `session` | `list_sessions` | `WorkspaceId` | `SessionList` |
| | `create_session` | `CreateSessionRequest` | `SessionSummary` |
| | `get_session` | `WorkspaceId`, `SessionId` | `SessionDetail` |
| | `remove_session` | `WorkspaceId`, `SessionId` | `Ack` |
| | `rename_session` | `WorkspaceId`, `SessionId`, `String` | `SessionSummary` |
| `generation` | `ask` | `AskRequest` | `SessionDetail` |

- 每个方法返回 `impl TrMayCancel<MayCancelOutput = RpcResult<T>>`；
- `Ack` 代替 `()`（denco 目前不支持空类型）；
- body 用 `mp_denco`。

## 业务错误

不改 mptp_rpc 上游，用“专用状态码 + 客户端剥离 Display 前缀”保真：

```text
服务端：ErrorCode → 460..=466 状态码
        ErrorReply.message 原样放进 RpcError::Service.message
客户端：从 RpcError::Remote.body 取第一个 "）：" 之后的内容
        还原 ErrorReply { code, message }
```

- `K_BUSINESS_STATUS_BASE = 460`；七个 `ErrorCode` 一一对应 460–466；
- 还原后统一是 `KbRpcError::Business(ErrorReply)`；
- 其它 RPC 错误 → `KbRpcError::Transport(E)`；
- 前缀格式由单元测试钉住；上游文案变化时测试先红。

## 为什么单独一个 crate

- 数据 crate 保持“纯消息”，不依赖 `mptp_rpc` / 异步宏；
- 只实现传输、不碰业务的调用方可以只依赖契约 crate；
- 契约的公开面（方法、路径、错误映射）在一个地方，服务端实现与客户端代理都对着它。

## 验证

```bash
cargo test -p abs_kb_svc_v1_desktop_rpc
```

状态码往返、业务错误前缀剥离等单元测试。

## 相关文档

- [`abs_kb_svc_v1_desktop/README.md`](../abs_kb_svc_v1_desktop/README.md)：数据与线格式；
- [`abs_kb_svc`](../abs_kb_svc/README.md)：聚合层；
- [`kb_svc_mptp_ipc`](../../../kb_plugins/crates/kb_svc_mptp_ipc/README.md)：本机传输；
- [dev-notes/kb_svc_mptp_ipc-20261105-0345.md](../../../dev-notes/kb_svc_mptp_ipc-20261105-0345.md)：
  RPC / 错误映射 / 线程模型。
