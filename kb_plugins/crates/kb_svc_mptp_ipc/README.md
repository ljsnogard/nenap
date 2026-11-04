# kb_svc_mptp_ipc

知识库本机 IPC 的**传输实现**：Unix domain socket + `smux_v1` + `mptp_rpc`。

```text
kb_core（服务端）                             客户端
Listener::bind(runtime_dir)                  Client::connect_with_timeout(runtime_dir, timeout)
  → 清残留、生成 kb-<日期>-<uuid>.sock         → 扫最新 socket、连接、smux 握手
  → accept() → Connection::serve(services)     → 宿主线程 + 作业队列
```

业务消息与 RPC 契约在 `abs_kb_svc` 里；本 crate 只负责搬运。

## 公开接口

服务端：

- `Listener::bind(runtime_dir)`：清掉运行时目录里残留的 `kb-*.sock` / `kb-*.ipc`，
  生成新 socket，chmod 0600；
- `Listener::socket_path()`：本次启动的 socket 路径；
- `Listener::accept()`：等一个客户端并完成 smux 握手；
- `Connection::serve(KbServiceSet { handshake, workspace, session, generation })`：
  在该连接上跑 `MptpServer`，按 `#[rpc]` 路径派发。

客户端：

- `Client::connect(runtime_dir)` / `connect_with_timeout(runtime_dir, timeout)`：
  **同步**调用；内部由宿主线程完成异步连接；
- `Client` 是 `Send + Sync`；`hello` / 工作区 / 会话 / `ask` 等方法返回
  `WaitAsync`，既能 `.await`，也能 `.may_cancel_with(token).await`；
- `send_envelope` 给 `kb_core_rproxy` 这类“按信封转发”的调用方用。

错误类型 `MptpIpcError` 与业务错误 `KbRpcError<E>` 分开；契约 crate 负责把
460–466 状态码还原成 `ErrorReply`。

## 线程 / 运行时模型

| 位置 | 运行时 |
| :--- | :--- |
| `kb_core` 主线程 | compio（`#[compio::main]`） |
| `Client` 宿主线程 | 独立 compio Runtime + 作业队列 |
| 调用方 | 无要求；`futures_lite::block_on` 也能驱动 |

调用方拿到的 future 只轮询完成量与取消令牌，future 里没有阻塞调用。

## 端点

- 名字：`kb-<YYYYMMDD>-<uuid>.sock`；
- 启动时清理残留；`Drop` 时删除自己的 socket；
- 路径长度检查（Linux `sun_path` 上限）；
- 权限 0600；
- 客户端扫运行时目录里**最新**的 `kb-*.sock`，连接失败会重试到总超时。

## 取消

- 取消只放弃等待，不撤回请求；宿主线程用不可取消令牌把请求跑完；
- 取消落点是 `MptpIpcError` / `KbRpcError::Transport`，不是业务错误；
- `Listener` / `Connection` 侧把取消一路传给 `Store`（见 `kb_core`）。

## 边界

| 限制 | 说明 |
| :--- | :--- |
| Unix only | compio 的 UDS 是 Unix 专用；没有 Windows 命名管道 |
| 一次一个客户端 | 服务端 accept 一个、serve 到断开，再 accept 下一个 |
| 事件推送 | 没有实现；`Event` 类型保留 |
| 流式生成 / `Cancel` | 没有实现 |

## 验证

```bash
cargo test -p kb_core              # 含真进程 UDS 端到端
cargo test -p kb_client_conn_mgr   # 客户端路径
```

## 相关文档

- [dev-notes/kb_svc_mptp_ipc-20261105-0345.md](../../../dev-notes/kb_svc_mptp_ipc-20261105-0345.md)：
  传输设计原理、线程模型、错误映射；
- [`abs_kb_svc_v1_desktop_rpc`](../../../kb_svc/crates/abs_kb_svc_v1_desktop_rpc/)：RPC 契约；
- [`kb_core`](../../../kb_svc/crates/kb_core/README.md)：服务端。
