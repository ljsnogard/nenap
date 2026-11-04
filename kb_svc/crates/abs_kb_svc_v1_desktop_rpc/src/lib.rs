//! 桌面协议 v1 的 **mptp_rpc 契约**：interface、四个按域 service、错误映射。
//!
//! 数据消息仍定义在 `abs_kb_svc_v1_desktop`；本 crate 只回答"哪些方法、什么路径、
//! 参数与返回是什么"，以及"业务错误怎么过线"。
//!
//! # 业务错误
//!
//! 本轮不改 mptp_rpc 上游，而宏写错误回复时用 `RpcError::Service` 的 Display，
//! 会给 message 加上 `"RPC 业务错误 {n}（{name}）："` 前缀。因此：
//!
//! - 服务端把 `ErrorCode` 映射到 460–466，并把 `ErrorReply.message` 原样放进
//!   `RpcError::Service.message`；
//! - 客户端从 `RpcError::Remote.body` 中取**第一个** `"）："` 之后的内容，
//!   还原原始 message。
//!
//! 前缀格式由一条单元测试钉住；上游文案变化时测试先红。

#![feature(impl_trait_in_assoc_type)]
#![recursion_limit = "512"]

mod error_;
mod rpc_;

pub use error_::{
    K_BUSINESS_STATUS_BASE, KbRpcError, business_error_from_rpc_, error_code_of_status,
    status_of_error_code,
};
pub use rpc_::{KbApiV1, TrGeneration, TrHandshake, TrSessionService, TrWorkspaceService};

/// 一次成功的空操作。
///
/// denco 目前不支持 `()`，所以用它代替。
#[derive(Debug, Clone, Copy, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct Ack;

/// 重导出 RPC 结果类型，调用方不必直接依赖 `rpc_core_v1`。
pub use rpc_core_v1::messaging::{RpcError, RpcResult};
