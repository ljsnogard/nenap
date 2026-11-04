//! **系统层握手**：`kb_core` 公布自己本机端点的那条通知。
//!
//! `kb_core` 以 `--handshake-prompt=stdio` 启动时，会在 `Listener::bind` 之后往
//! **stdout** 打一行 [`IpcReadyNotice`] 的 JSON，把"本机 socket 在哪"告诉启动它的
//! 父进程。启动方（`kb_core_starter`）读这一行，就知道该去哪连接。
//!
//! # 它属于哪一层
//!
//! 客户端要跟 `kb_core` 谈成事，要过两道握手，它们的**消息都是公开协议**：
//!
//! | 层面 | 解决什么 | 消息定义在哪 |
//! | :--- | :--- | :--- |
//! | **系统层**（本 crate） | 找得到、连得上 | [`IpcReadyNotice`] + [`HandshakeNoticeKind`] |
//! | **应用层** | 谈得成 | `abs_kb_svc_v1_desktop` 的 `PROTOCOL_VERSION` / `ClientInfo` / `ServerInfo` / `Request::Hello` |
//!
//! 两者分开的理由是**依赖面**：系统层的消息只有一个"我准备好了、socket 在哪"，
//! 而应用层要带上工作区、会话、服务列表这些业务词汇。把系统层单独放一个 crate，
//! **只想启动并找到 `kb_core` 的调用方（`kb_core_starter`）就不必依赖整套业务协议**。
//!
//! # 同一份定义，两个方向
//!
//! | 谁 | 做什么 |
//! | :--- | :--- |
//! | `kb_core`（`serve_`） | [`encode_notice`] 编出 JSON，`writeln!` 到 stdout |
//! | `kb_core_starter` | [`decode_notice`] 解析那一行 |
//!
//! 两边共用这里的类型与编解码，所以改字段名/改 `event` 取值是**编译错误**，
//! 而不是"跑起来才发现启动不了"。
//!
//! # 它不承诺什么
//!
//! 只承诺**路径**：拿到通知之后 socket 已经 bind，但服务端可能还没进入 `accept`；
//! 连接方仍然要带重试与超时。
//!
//! # 示例
//!
//! ```
//! use abs_kb_core_handshake::{decode_notice, encode_notice, HandshakeNoticeKind, IpcReadyNotice};
//!
//! let notice = IpcReadyNotice {
//!     event: HandshakeNoticeKind::IpcReady,
//!     ipc_name_file: "/run/user/1000/llm_kb/kb-20260918-abc.sock".to_string(),
//!     protocol_version: 1,
//!     pid: 1234,
//! };
//! let line = encode_notice(&notice).expect("应当能编码");
//! assert!(line.contains(r#""event":"ipc_ready""#));
//!
//! let parsed = decode_notice(&line).expect("应当能解码");
//! assert_eq!(parsed, notice);
//! ```

// denco 派生用 GAT + `impl Trait` 表示异步编解码操作。
#![feature(impl_trait_in_assoc_type)]
#![recursion_limit = "512"]

mod system_;

pub use system_::{HandshakeNoticeKind, IpcReadyNotice, decode_notice, encode_notice};
