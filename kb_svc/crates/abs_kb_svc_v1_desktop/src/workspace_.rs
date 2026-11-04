//! 工作区与会话：由 `kb_core` 持有、多端同步的对象。
//!
//! # 归属与标识
//!
//! 工作区与会话都是 **`kb_core` 管理的服务端状态**，因此：
//!
//! - 每个工作区有自己的 [`WorkspaceId`]、每个会话有自己的 [`SessionId`]，**由 `kb_core` 分配**；
//! - 任何客户端都可以**先在本地创建**它们：此时它只能拿到 [`LocalId`]，
//!   直到同步（[`Request::AddWorkspace`](crate::Request::AddWorkspace)
//!   / [`Request::CreateSession`](crate::Request::CreateSession)）时
//!   才由 `kb_core` 分配真正的标识；
//! - 同步的顺序是**先工作区、后会话**：`CreateSession` 需要 `workspace_id`，
//!   而那个标识只能来自工作区同步的结果。
//!
//! # 为什么是扁平结构
//!
//! 与客户端的 `Workspace`（内嵌 `sessions`）不同，协议里的 [`Workspace`] **不含**会话。
//! 会话通过 [`Request::ListSessions`](crate::Request::ListSessions) 按需拉取，
//! 避免每次列工作区都把整棵历史树传一遍；"工作区 → 会话 → 消息"的树由客户端自己组装。
//!
//! # "当前选中"不属于这里
//!
//! "当前停在哪个工作区 / 哪个会话"是**每个客户端各自的界面状态**（多窗口时会不同），
//! 因此不上传。相反，"当前生效的 LLM 服务"是服务端状态，见
//! [`ServerState::active_service`](crate::ServerState::active_service)。

use super::content_::Turn;
use super::ids_::{SessionId, WorkspaceId};

/// 一个知识库工作区。
///
/// 对应磁盘上的一个目录。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct Workspace {
    /// 工作区标识，由 `kb_core` 分配。
    pub workspace_id: WorkspaceId,

    /// 展示名。
    pub name: String,

    /// 对应的磁盘目录。
    pub path: String,
}

/// 工作区列表。
#[derive(
    Debug, Clone, Default, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable,
)]
pub struct WorkspaceList {
    /// 全部工作区。
    pub workspaces: Vec<Workspace>,
}

/// 一个会话的摘要（不含消息正文）。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct SessionSummary {
    /// 会话标识，由 `kb_core` 分配。
    pub session_id: SessionId,

    /// 所属工作区。
    pub workspace_id: WorkspaceId,

    /// 会话标题（通常取首条用户消息的前若干字）。
    pub title: String,

    /// 最近一次活动时间（自 Unix 纪元起的毫秒数）。
    pub updated_at_millis: i64,

    /// 会话内的消息条数，供列表页展示。
    pub turn_count: u32,
}

/// 一个会话的完整内容。
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct SessionDetail {
    /// 会话摘要。
    pub summary: SessionSummary,

    /// 会话内的全部消息，按时间顺序。
    pub turns: Vec<Turn>,
}

/// 会话列表。
#[derive(
    Debug, Clone, Default, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable,
)]
pub struct SessionList {
    /// 某个工作区下的全部会话。
    pub sessions: Vec<SessionSummary>,
}
