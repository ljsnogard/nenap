//! # kb_svc_mptp_ipc
//!
//! 知识库本机 IPC 的**新传输实现**：Unix domain socket + `smux_v1` + `mptp_rpc`。
//!
//! - 服务端：[`Listener`] bind 一个 `kb-<日期>-<uuid>.sock`，[`Connection::serve`]
//!   在该连接上跑 `MptpServer`；
//! - 客户端：[`Client`] 自带一条宿主线程（compio Runtime + 作业队列），
//!   对外仍是 `Send + Sync`、`connect` 仍是同步调用。
//!
//! 业务消息与 RPC 契约在 `abs_kb_svc` 里；本 crate 只负责搬运。

#![feature(impl_trait_in_assoc_type)]
#![recursion_limit = "512"]
// `gen_mcf2` 会把显式生命周期做成生成类型的泛型参数。
#![allow(clippy::needless_lifetimes)]

mod client_;
mod config_;
mod connection_;
mod error_;
mod host_;
mod listener_;
mod rendezvous_;

pub use abs_kb_svc::v1::desktop_rpc::{KbRpcError, RpcError, RpcResult};
pub use client_::{Client, WaitAsync};
pub use config_::{CliCfg, Conn, K_LISTEN_RESERVE, K_RING_CAP, K_SERVING_DOCK, Rx, SrvCfg, Tx};
pub use connection_::{Connection, KbServiceSet};
pub use error_::MptpIpcError;
pub use listener_::Listener;
pub use rendezvous_::{
    DEFAULT_CONNECT_TIMEOUT, SOCKET_FILE_EXTENSION, SOCKET_FILE_PREFIX, find_socket_in,
    new_socket_path_in,
};
