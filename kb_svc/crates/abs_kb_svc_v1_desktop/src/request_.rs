//! 客户端 → `kb_core` 的请求。
//!
//! # 表示形式
//!
//! 线上用 denco 的 MessagePack：枚举变体按**声明顺序**编码，方法/字段顺序
//! 因此是契约；只允许在末尾追加变体或字段。

use super::content_::Turn;
use super::handshake_::ClientInfo;
use super::ids_::{LocalId, ServiceId, SessionId, TurnId, WorkspaceId};
use super::service_::ApiKeyUpdate;

/// 一次提问。
///
/// 回合标识 [`AskRequest::turn_id`] 由**客户端**生成：这样从"发出提问"到
/// "收到第一条增量"之间的窗口期里，界面也已经有一个标识可用。
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct AskRequest {
    /// 提问所在的工作区。
    pub workspace_id: WorkspaceId,

    /// 提问所在的会话。
    pub session_id: SessionId,

    /// 客户端生成的回合标识；服务端以此回报增量。
    pub turn_id: TurnId,

    /// 问题正文。
    pub question: String,

    /// 指定使用的服务；`None` 表示用当前生效的服务。
    pub service_id: Option<ServiceId>,
}

/// 新增或覆盖一个 LLM 服务。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct UpsertServiceRequest {
    /// 服务标识（用户可见的名字）。
    pub service_id: ServiceId,

    /// provider 标识。
    pub provider: String,

    /// 模型名。
    pub model: String,

    /// API base URL；空表示用 provider 默认地址。
    pub base_url: String,

    /// API key 的更新语义。
    pub api_key: ApiKeyUpdate,
}

/// 同步一个客户端**本地创建**的工作区。
///
/// 客户端可以在没有连上 `kb_core` 时就创建好工作区（目录是本地路径），
/// 同步时把它连同 [`LocalId`] 一起提交；`kb_core` 负责分配真正的
/// [`WorkspaceId`](super::ids_::WorkspaceId) 并在应答里回传配对关系。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct AddWorkspaceRequest {
    /// 客户端创建该工作区时使用的临时标识。
    ///
    /// 同步完成后客户端应当用应答里的 `workspace_id` 取代它。
    pub local_id: LocalId,

    /// 展示名。
    pub name: String,

    /// 磁盘目录。
    pub path: String,
}

/// 在某个工作区里同步一个客户端**本地创建**的会话。
///
/// 必须先同步工作区：`workspace_id` 只能来自
/// [`AddWorkspaceRequest`] 的应答。
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct CreateSessionRequest {
    /// 目标工作区（`kb_core` 分配的标识）。
    pub workspace_id: WorkspaceId,

    /// 客户端创建该会话时使用的临时标识。
    pub local_id: LocalId,

    /// 标题；`None` 表示由服务端取默认值。
    pub title: Option<String>,

    /// 客户端在同步之前**已经攒下的消息**，按时间顺序。
    ///
    /// 通常为空；离线使用过的客户端会带上它，这样历史不会丢。
    pub turns: Vec<Turn>,
}

/// 桌面客户端发给 `kb_core` 的请求。
///
/// # 示例
///
/// ```
/// use abs_kb_svc_v1_desktop::{Request, ServiceId};
///
/// let request = Request::UseService {
///     service_id: ServiceId::new("deepseek"),
/// };
///
/// assert!(matches!(request, Request::UseService { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub enum Request {
    /// 握手，必须是最先发出的一条。
    Hello(ClientInfo),

    /// 就某个会话提问。
    Ask(AskRequest),

    /// 取消某一轮生成。
    Cancel {
        /// 要取消的回合。
        turn_id: TurnId,
    },

    /// 列出已配置的 LLM 服务。
    ListServices,

    /// 新增或覆盖一个 LLM 服务。
    UpsertService(UpsertServiceRequest),

    /// 删除一个 LLM 服务。
    RemoveService {
        /// 要删除的服务。
        service_id: ServiceId,
    },

    /// 切换当前生效的 LLM 服务。
    UseService {
        /// 目标服务。
        service_id: ServiceId,
    },

    /// 列出全部工作区。
    ListWorkspaces,

    /// 同步一个本地创建的工作区，由 `kb_core` 分配标识。
    AddWorkspace(AddWorkspaceRequest),

    /// 删除一个工作区。
    RemoveWorkspace {
        /// 要删除的工作区。
        workspace_id: WorkspaceId,
    },

    /// 重命名一个工作区（只改展示名，不动磁盘目录）。
    RenameWorkspace {
        /// 要改名的工作区。
        workspace_id: WorkspaceId,

        /// 新名字。空白会被服务端拒绝：工作区没有"从消息推导名字"这条退路。
        name: String,
    },

    /// 列出某个工作区下的会话。
    ListSessions {
        /// 目标工作区。
        workspace_id: WorkspaceId,
    },

    /// 同步一个本地创建的会话，由 `kb_core` 分配标识。
    CreateSession(CreateSessionRequest),

    /// 删除一个会话。
    RemoveSession {
        /// 目标工作区。
        workspace_id: WorkspaceId,

        /// 要删除的会话。
        session_id: SessionId,
    },

    /// 重命名一个会话（改标题）。
    RenameSession {
        /// 目标工作区。
        workspace_id: WorkspaceId,

        /// 要改名的会话。
        session_id: SessionId,

        /// 新标题。空白表示"交回服务端重新推导"（取首条用户消息，取不到就是默认名）。
        title: String,
    },

    /// 读取一个会话的完整内容（含全部消息）。
    GetSession {
        /// 目标工作区。
        workspace_id: WorkspaceId,

        /// 目标会话。
        session_id: SessionId,
    },

    /// 列出工作区目录下的条目。
    ListDirectory {
        /// 目标工作区。
        workspace_id: WorkspaceId,

        /// 相对工作区根目录的路径；根目录用空串表示。
        relative_path: String,
    },
}
