//! `kb_core` 主动推给客户端的事件。
//!
//! 事件不与请求关联，且**可能被丢弃**（例如界面卡顿时）：
//! 需要可靠送达的内容应当放进 [`Reply`](crate::Reply)。
//!
//! # 一轮生成的完整序列
//!
//! ```text
//! Event::TurnStarted   开始（含实际使用的服务与模型）
//! Event::Delta …       0..n 条增量（按 LogicOutput 分流到正文/推理）
//! Event::ToolCall …    0..n 次工具调用
//! Event::Usage         0..1 次用量
//! Event::TurnFinished  结束（含结束原因）
//! ```

use abs_llm::v1::cont::{Capabilities, FinishReason, LogicOutput};

use super::content_::{TokenUsage, ToolCallRecord};
use super::error_::ErrorCode;
use super::handshake_::ServerState;
use super::ids_::{ServiceId, SessionId, TurnId};
use super::workspace_::SessionSummary;

/// 一轮生成已经开始。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct TurnStarted {
    /// 回合标识。
    pub turn_id: TurnId,

    /// 该回合所属会话。
    pub session_id: SessionId,

    /// 实际使用的服务标识。
    pub service_id: ServiceId,

    /// 实际使用的模型名。
    pub model: String,

    /// 插件自报的能力。
    pub capabilities: Capabilities,
}

/// 一段增量文本。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct TextDelta {
    /// 回合标识。
    pub turn_id: TurnId,

    /// 这段文本属于哪个逻辑部分（answer / reasoning / …）。
    pub logic: LogicOutput,

    /// 文本片段。
    pub text: String,
}

/// 模型要求调用工具。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct ToolCallEvent {
    /// 回合标识。
    pub turn_id: TurnId,

    /// 调用记录。
    pub call: ToolCallRecord,
}

/// 用量信息。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct UsageEvent {
    /// 回合标识。
    pub turn_id: TurnId,

    /// 用量明细。
    pub usage: TokenUsage,
}

/// 本轮生成结束。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct TurnFinished {
    /// 回合标识。
    pub turn_id: TurnId,

    /// 结束原因；并非所有 provider 都会给出。
    pub reason: Option<FinishReason>,
}

/// 一条与生成过程有关的错误。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct ErrorEvent {
    /// 相关回合；与具体回合无关的错误为 `None`。
    pub turn_id: Option<TurnId>,

    /// 错误类别。
    pub code: ErrorCode,

    /// 面向用户的说明。
    pub message: String,
}

/// 一个会话的元信息发生了变化（标题、时间、条数）。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct SessionChanged {
    /// 变化后的摘要。
    pub summary: SessionSummary,
}

/// `kb_core` 主动推给桌面客户端的事件。
///
/// # 示例
///
/// ```
/// use abs_llm::v1::cont::LogicOutput;
/// use abs_kb_svc_v1_desktop::{Event, TextDelta, TurnId};
///
/// let event = Event::Delta(TextDelta {
///     turn_id: TurnId::new("t-1"),
///     logic: LogicOutput::Answer,
///     text: "你好".to_string(),
/// });
///
/// match event {
///     Event::Delta(delta) => {
///         assert_eq!(delta.logic, LogicOutput::Answer);
///         assert_eq!(delta.text, "你好");
///     }
///     _ => panic!("应当是 Delta"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub enum Event {
    /// 握手完成后的第一条推送，以及每次状态刷新。
    Ready(ServerState),

    /// 一轮生成开始。
    TurnStarted(TurnStarted),

    /// 一段增量文本。
    Delta(TextDelta),

    /// 模型要求调用工具。
    ToolCall(ToolCallEvent),

    /// 用量信息。
    Usage(UsageEvent),

    /// 本轮生成结束。
    TurnFinished(TurnFinished),

    /// 生成过程相关的错误。
    Error(ErrorEvent),

    /// 服务端状态变化（插件上下线、服务配置变化）。
    StateChanged(ServerState),

    /// 会话元信息变化。
    SessionChanged(SessionChanged),
}
