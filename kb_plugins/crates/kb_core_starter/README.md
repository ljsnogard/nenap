# kb_core_starter

用命令行启动一个 `kb-core` 子进程，并**异步地**等它公布自己的本机 UDS 端点。

```text
调用方 ──Command::new("kb-core") --handshake-prompt=stdio──► kb-core（子进程）
       ◄──── stdout 上的一行 IpcReadyNotice（JSON）────────┘
```

## 它是什么 / 不是什么

- **是一个库**（没有 `[[bin]]`）：它不自己成为进程，而是由调用方进程链接；
- 它负责“起子进程 + 读通知 + 管子进程归属”，不负责应用层握手；
- 当前两个调用方都是链接库：

```text
kb_core_rproxy 进程
  └─ kb_core_starter (lib) ──spawn──► kb-core 子进程

kb_admin_desktop 进程
  └─ kb_client_conn_mgr (lib)
       └─ kb_core_starter (lib) ──spawn──► kb-core 子进程
```

## 它实现协议里的哪一块

| 层面 | 解决什么 | 消息 | 谁实现 |
| :--- | :--- | :--- | :--- |
| **系统层** | 找得到、连得上 | [`IpcReadyNotice`](../../../kb_svc/crates/abs_kb_core_handshake/src/system_.rs)（`event: "ipc_ready"`，带 `ipc_name_file` / `protocol_version` / `pid`） | **本 crate 收**；`kb_core::serve_` 发 |
| **应用层** | 谈得成 | `Request::Hello` → `Reply::Hello` | `kb_svc_mptp_ipc::Client`（本机）/ `TcpClient`（远程） |

`ipc_name_file` 是消息里的字段名（兼容保留），**语义是 socket 路径**。
本 crate 只把这条通知解出来交给调用方；连接由调用方用
`kb_svc_mptp_ipc::Client::connect(runtime_dir)`（或远程 TCP 客户端）完成。

## 形状

```rust
let spec = LaunchSpec {
    kb_core: "/usr/local/bin/kb-core".into(),
    runtime_dir: "/run/user/1000/llm_kb".into(),
    storage_dir: "/home/me/.local/share/kb_admin_desktop/storage".into(),
};

let launched = start(&spec).await?;                       // 不可取消
let launched = start(&spec).may_cancel_with(token).await?; // 可取消
```

- `start()` 返回的 future **与异步运行时无关**：`read_line` 发生在一条专职线程上，
  future 只轮询完成量与取消令牌；tokio / compio / `futures_lite::block_on` 都能驱动；
- **不自带定时器**：等多久算超时由调用方造取消令牌决定；
- 解析用 `json_denco`（与 `kb_core` 的发送侧共用同一份消息定义），不依赖 serde。

## 子进程归属

三条收场路径共用同一个子进程守卫：

| 收场 | 子进程 |
| :--- | :--- |
| 成功 | 所有权交给 `Launched`，它在 `Drop` 时结束进程 |
| 取消 / 读通知失败 | 守卫被丢弃 → 结束进程 |
| future 被直接丢弃 | 同上 |

因此只要 `kb-core` 是本 crate 起来的，等待失败之后就不会留下孤儿进程。

## `kb-core` 可执行文件在哪

[`LaunchSpec::kb_core`] **必须由调用方给出**。`default_kb_core_path()` 只是猜
“与本可执行文件同目录”：

- Cargo 构建时所有 bin 都在 `target/<profile>/`，通常猜得对；
- Flutter 打包出来的 App 里 `current_exe()` 是 Flutter runner，`kb-core`
  通常不在旁边——这时必须由配置 / 用户显式指定路径；
- 注意 Cargo 产物名是 `kb-core`（连字符），不是 crate 名 `kb_core`。

## 验证

```bash
cargo test -p kb_core_starter
```

2 个单元测试（通知解析）+ 7 个集成测试（用假 `kb-core` 脚本覆盖 正常 / 提前退出 /
坏通知 / 可执行文件不可用 / 已取消不起进程 / 等待中取消并结束子进程 /
`kb_core_beside`）+ 3 个文档测试。

## 相关文档

- [`abs_kb_core_handshake`](../../../kb_svc/crates/abs_kb_core_handshake/README.md)：
  系统层握手的消息与收发双方；
- [`kb_core_rproxy`](../kb_core_rproxy/README.md)：本 crate 的调用方之一；
- [`kb_client_conn_mgr`](../../../kb_clients/crates/kb_client_conn_mgr/README.md)：另一个调用方。
