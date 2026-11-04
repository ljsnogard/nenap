//! 请求 / 应答的关联信封。
//!
//! 一个客户端与 `kb_core` 之间可以只开**一对**通道（一个上行、一个下行），
//! 由 [`RequestEnvelope::request_id`] 把应答配回请求；也可以每个请求开一对通道，
//! 此时 `request_id` 依然有用（用于日志与排错）。
//!
//! 事件不需要信封——它是单向推送，没有"配回哪个请求"的问题。

use super::ids_::RequestId;
use super::reply_::Reply;
use super::request_::Request;

/// 请求信封：把请求与它的关联标识绑在一起。
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct RequestEnvelope {
    /// 请求标识，由客户端生成，在本次连接内唯一。
    pub request_id: RequestId,

    /// 请求内容。
    pub request: Request,
}

impl RequestEnvelope {
    /// 构造一个请求信封。
    pub fn new(request_id: impl Into<RequestId>, request: Request) -> Self {
        Self {
            request_id: request_id.into(),
            request,
        }
    }
}

/// 应答信封。
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct ReplyEnvelope {
    /// 对应请求的标识。
    pub request_id: RequestId,

    /// 应答内容。
    pub reply: Reply,
}

impl ReplyEnvelope {
    /// 构造一个应答信封。
    pub fn new(request_id: impl Into<RequestId>, reply: Reply) -> Self {
        Self {
            request_id: request_id.into(),
            reply,
        }
    }
}
