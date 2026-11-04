//! `kb_core` → 客户端的应答。
//!
//! 每个变体与 [`Request`](crate::Request) 一一对应；
//! [`Reply::Ack`] 用于"成功但没有内容"的请求（删除、切换生效服务等）。
//! 业务错误走 [`Reply::Error`]。

use super::error_::ErrorReply;
use super::fs_::DirectoryListing;
use super::handshake_::ServerInfo;
use super::ids_::LocalId;
use super::service_::{ServiceList, ServiceSummary};
use super::workspace_::{SessionDetail, SessionList, SessionSummary, Workspace, WorkspaceList};

/// `kb_core` 对某个请求的应答。
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub enum Reply {
    /// [`Request::Hello`](crate::Request::Hello) 的应答。
    Hello(ServerInfo),

    /// 成功，且没有需要返回的内容。
    Ack,

    /// 服务列表（列出服务之后，以及服务配置变化之后）。
    ServiceList(ServiceList),

    /// 某个服务被新增或覆盖。
    ServiceUpdated(ServiceSummary),

    /// 工作区列表。
    WorkspaceList(WorkspaceList),

    /// 工作区已建立。
    ///
    /// 携带 `local_id` 是为了让客户端知道"这是你刚才用哪个临时标识提交的那个"，
    /// 从而把本地对象换成服务端分配的标识。
    WorkspaceAdded {
        /// 客户端提交时使用的临时标识。
        local_id: LocalId,

        /// 服务端建立的工作区（含 `kb_core` 分配的标识）。
        workspace: Workspace,
    },

    /// 工作区已改名（载荷是改名之后的工作区）。
    WorkspaceRenamed(Workspace),

    /// 会话列表。
    SessionList(SessionList),

    /// 会话已建立。
    ///
    /// 与 [`Reply::WorkspaceAdded`] 同理，携带 `local_id` 用于替换本地标识。
    SessionCreated {
        /// 客户端提交时使用的临时标识。
        local_id: LocalId,

        /// 服务端建立的会话摘要（含 `kb_core` 分配的标识）。
        session: SessionSummary,
    },

    /// 会话已改名（载荷是改名之后的会话摘要）。
    SessionRenamed(SessionSummary),

    /// 会话的完整内容。
    SessionDetail(SessionDetail),

    /// 目录列举结果。
    DirectoryListing(DirectoryListing),

    /// 业务错误。
    Error(ErrorReply),
}
