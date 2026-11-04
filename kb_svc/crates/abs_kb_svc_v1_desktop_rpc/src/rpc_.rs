//! `#[rpc]` 契约：interface 与四个按域 service。

use abs_cancel::TrMayCancel;
use abs_kb_svc_v1_desktop::{
    AddWorkspaceRequest, AskRequest, ClientInfo, CreateSessionRequest, ServerInfo, SessionDetail,
    SessionId, SessionList, SessionSummary, Workspace, WorkspaceId, WorkspaceList,
};
use rpc_core_v1::messaging::RpcResult;
use rpc_svc_macro::rpc;

use crate::Ack;

/// 桌面协议 v1 的 interface；完整路径是 `/{interface}/{service}`。
#[rpc(
    path = "kb/v1",
    services = [
        TrHandshake,
        TrWorkspaceService,
        TrSessionService,
        TrGeneration
    ]
)]
pub struct KbApiV1;

/// 应用层握手：协议要求的第一条请求。
#[rpc(service = "handshake")]
pub trait TrHandshake {
    /// 自报身份并取回服务端身份。
    fn hello<'f>(
        &'f self,
        client: ClientInfo,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<ServerInfo>>;
}

/// 工作区域。
#[rpc(service = "workspace")]
pub trait TrWorkspaceService {
    /// 列出全部工作区。
    fn list_workspaces<'f>(
        &'f self,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<WorkspaceList>>;

    /// 登记一个（客户端本地先创建的）工作区。
    fn add_workspace<'f>(
        &'f self,
        request: AddWorkspaceRequest,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Workspace>>;

    /// 删除一个工作区（连同它名下的会话）。
    fn remove_workspace<'f>(
        &'f self,
        workspace_id: WorkspaceId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Ack>>;

    /// 重命名一个工作区。
    fn rename_workspace<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        name: String,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Workspace>>;
}

/// 会话域。
#[rpc(service = "session")]
pub trait TrSessionService {
    /// 列出某个工作区下的会话。
    fn list_sessions<'f>(
        &'f self,
        workspace_id: WorkspaceId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionList>>;

    /// 登记一个（客户端本地先创建的）会话。
    fn create_session<'f>(
        &'f self,
        request: CreateSessionRequest,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionSummary>>;

    /// 读取一个会话的完整内容。
    fn get_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionDetail>>;

    /// 删除一个会话。
    fn remove_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Ack>>;

    /// 重命名一个会话。
    fn rename_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        title: String,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionSummary>>;
}

/// 生成域。
#[rpc(service = "generation")]
pub trait TrGeneration {
    /// 就某个会话提问；应答是提问之后的完整会话。
    fn ask<'f>(
        &'f self,
        request: AskRequest,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionDetail>>;
}
