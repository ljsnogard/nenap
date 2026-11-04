//! 把本地文件存储接上 IPC：`kb_core` 这一侧的按域 RPC trait 实现。
//!
//! [`KbService`] 把 [`TrHandshake`] / [`TrWorkspaceService`] / [`TrSessionService`]
//! 的每个方法原样转发给 [`Store`]，并把 [`StoreError`] 翻成协议里的业务错误：
//!
//! | `StoreError` | `ErrorCode` |
//! | :--- | :--- |
//! | `NotFound` | `NotFound` |
//! | `InvalidId` | `BadRequest` |
//! | `Cancelled` | `Internal`（服务端侧被取消） |
//! | 其它（I/O、解码、阻塞任务） | `Internal` |
//!
//! # 生成域：临时模拟的 LLM
//!
//! [`TrGeneration::ask`] 还没有真正的 LLM 插件可接，因此实现成
//! **"把问题按字符逆序输出"**：一次提问落两条消息（`user` + 逆序的
//! `assistant`），应答是提问之后的完整会话。细节与替换路径见 [`ask_async`]。
//!
//! 服务端这一侧不产生"传输层错误"（那是客户端代理的事），所有失败都是业务错误；
//! 派发层里 `RpcError::Transport` 那条分支对本实现不可达。
//!
//! # 关于 `AGENTS.md` 第 4 条
//!
//! 本 crate 因为要展开 [`gen_mcf2::gen_may_cancel_future`]，依赖链上出现了
//! `gen_mcf2`，于是第 4 条落在这里。处理方式是**一路贯到底**：
//!
//! - 本文件里每个方法都由宏展开，且都检查传进来的取消令牌；
//! - 取消令牌**继续往下传**给 [`crate::store_::Store`]
//!   （`.may_cancel_with(cancel)`），而不是在这里丢掉自己造一个；
//! - `Store` 的每个操作同样是宏展开的可取消 future（见那里模块文档），
//!   所以"客户端撤销了一次调用"能一路传到最后一次文件等待。
//!
//! （早先曾以"`kb_core` 是 bin、没有对外库接口"为由只给本文件加取消，
//! 那个判断是错的：只要存在被取消的可能性，就不该从设计上抹掉它。）

use abs_cancel::{TrCancellationToken, TrMayCancel};
use abs_kb_svc::v1::desktop::{
    AddWorkspaceRequest, AskRequest, ClientInfo, CreateSessionRequest, ErrorCode, Notice,
    PROTOCOL_VERSION, ServerInfo, SessionDetail, SessionId, SessionList, SessionSummary, Turn,
    TurnId, TurnState, Workspace, WorkspaceId, WorkspaceList,
};
use abs_kb_svc::v1::desktop_rpc::{
    Ack, KbApiV1, RpcError as MptpRpcError, RpcResult, TrGeneration as TrGenerationRpc,
    TrHandshake as TrHandshakeRpc, TrSessionService as TrSessionRpc,
    TrWorkspaceService as TrWorkspaceRpc, status_of_error_code,
};
use abs_llm::v1::cont::Role;
use gen_mcf2::gen_may_cancel_future;
use rpc_svc_macro::rpc;

use crate::store_::{Store, StoreError};

/// 服务端业务实现。
#[derive(Debug, Clone)]
pub struct KbService {
    /// 底层存储。
    store_: Store,
}

impl KbService {
    /// 用一份存储实现一个服务。
    pub fn new(store: Store) -> Self {
        Self { store_: store }
    }

    /// 底层存储（供日志与测试查看）。
    pub fn store(&self) -> &Store {
        &self.store_
    }
}

/// 把存储失败翻成业务错误应答。
fn store_error_(error: StoreError) -> MptpRpcError {
    let code = match &error {
        StoreError::NotFound { .. } => ErrorCode::NotFound,
        StoreError::InvalidId { .. } => ErrorCode::BadRequest,
        // 调用方输入不合法（空名字、空的「新会话」）：业务拒绝，不是服务端故障。
        StoreError::EmptySession | StoreError::EmptyName { .. } => ErrorCode::BadRequest,
        _ => ErrorCode::Internal,
    };
    MptpRpcError::Service {
        status: status_of_error_code(code),
        message: error.to_string(),
    }
}

/// 服务端侧调用被取消时的应答。
///
/// 正常路径不会走到这里：派发层用不可取消的令牌调用这些方法。
/// 保留它是因为"实现不得假定调用者不会取消"（`AGENTS.md` 第 4 条）。
fn cancelled_() -> MptpRpcError {
    MptpRpcError::Service {
        status: status_of_error_code(ErrorCode::Internal),
        message: "服务端侧调用被取消".to_string(),
    }
}

// ── 模块级 async fn：宏只能作用于它们 ──────────────────────────────────

/// [`TrHandshake::hello`] 的服务端实现：**应用层握手**的第一步。
///
/// 服务端在这一步只做两件事：校验协议版本、把自己的身份报回去。
/// 它**不**负责"客户端怎么找到我"——那是系统层握手（传输实现）的事，
/// 两者的分工见 `abs_kb_svc::v1::desktop::handshake_` 的模块文档。
#[gen_may_cancel_future(Hello, pub)]
pub async fn hello_async<'s, C>(
    service: &'s KbService,
    client: ClientInfo,
    cancel: C,
) -> RpcResult<ServerInfo>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    let _ = service;

    if client.protocol_version != PROTOCOL_VERSION {
        return Err(MptpRpcError::Service {
            status: status_of_error_code(ErrorCode::BadRequest),
            message: format!(
                "协议版本不匹配：客户端说 v{}，服务端是 v{PROTOCOL_VERSION}",
                client.protocol_version
            ),
        });
    }

    Ok(ServerInfo {
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: PROTOCOL_VERSION,
    })
}

/// [`TrWorkspaceService::list_workspaces`] 的服务端实现。
#[gen_may_cancel_future(ListWorkspaces, pub)]
pub async fn list_workspaces_async<'s, C>(
    service: &'s KbService,
    cancel: C,
) -> RpcResult<WorkspaceList>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .list_workspaces()
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrWorkspaceService::add_workspace`] 的服务端实现。
#[gen_may_cancel_future(AddWorkspace, pub)]
pub async fn add_workspace_async<'s, C>(
    service: &'s KbService,
    request: AddWorkspaceRequest,
    cancel: C,
) -> RpcResult<Workspace>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .add_workspace(&request.name, &request.path)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrWorkspaceService::remove_workspace`] 的服务端实现。
#[gen_may_cancel_future(RemoveWorkspace, pub)]
pub async fn remove_workspace_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    cancel: C,
) -> RpcResult<()>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .remove_workspace(&workspace_id)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrWorkspaceService::rename_workspace`] 的服务端实现。
#[gen_may_cancel_future(RenameWorkspace, pub)]
pub async fn rename_workspace_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    name: String,
    cancel: C,
) -> RpcResult<Workspace>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .rename_workspace(&workspace_id, &name)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrSessionService::list_sessions`] 的服务端实现。
#[gen_may_cancel_future(ListSessions, pub)]
pub async fn list_sessions_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    cancel: C,
) -> RpcResult<SessionList>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .list_sessions(&workspace_id)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrSessionService::create_session`] 的服务端实现。
#[gen_may_cancel_future(CreateSession, pub)]
pub async fn create_session_async<'s, C>(
    service: &'s KbService,
    request: CreateSessionRequest,
    cancel: C,
) -> RpcResult<SessionSummary>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .create_session(&request.workspace_id, request.title, request.turns)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrSessionService::get_session`] 的服务端实现。
#[gen_may_cancel_future(GetSession, pub)]
pub async fn get_session_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    cancel: C,
) -> RpcResult<SessionDetail>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .get_session(&workspace_id, &session_id)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrGeneration::ask`] 的服务端实现：**临时模拟的 LLM**。
///
/// 真正的 LLM 插件还没接，所以这里把所有提问都当成"让模型复述"：
///
/// 1. 把客户端给的那条问题原样记为 `user` 回合（`turn_id` 用客户端生成的那个）；
/// 2. 生成一条 `assistant` 回合，正文是**问题按字符逆序**的结果，并挂一条说明性
///    [`Notice`]，让界面一眼看出这是模拟而不是真模型；
/// 3. 用 [`Store::append_turns`] 落盘（摘要里的 `turn_count` 与
///    `updated_at_millis` 由存储层维护）；
/// 4. 回**提问之后**的完整会话。
///
/// # 幂等（草稿流程依赖它）
///
/// 客户端的「新会话」是一个**草稿**：首次提问时先发 `CreateSession`（把这个
/// 问题作为首条 `user` 消息，好让 `kb_core` 据此起名），紧接着再发本条 `Ask`。
/// 于是 `Ask` 到这里时，用户回合**已经存在**了。
///
/// 所以这里按 `turn_id` 去重：
///
/// - 已经有这一轮的用户回合 → 不再重复添加；
/// - 已经有这一轮的助手回合（标识由用户回合确定性推导，见 [`answer_turn_id_`]）
///   → 原样回会话，不再生成第二条回答。
///
/// 这让"先建会话、再提问"与"重试一次提问"都安全。
///
/// 走"同步落盘再回详情"是为了先验证"新增会话内容在下次连线依然可见"这条目标；
/// 换成真 LLM 时，这里会变成事件流（见 [`TrGeneration`] 的文档），
/// [`Store::append_turns`] 仍会是落盘入口。
#[gen_may_cancel_future(Ask, pub)]
pub async fn ask_async<'s, C>(
    service: &'s KbService,
    request: AskRequest,
    cancel: C,
) -> RpcResult<SessionDetail>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }

    let workspace_id = request.workspace_id.clone();
    let session_id = request.session_id.clone();

    let existing = service
        .store_
        .get_session(&workspace_id, &session_id)
        .may_cancel_with(cancel.child_token())
        .await
        .map_err(store_error_)?;

    let answer_id = answer_turn_id_(&request.turn_id);
    let has_user = existing
        .turns
        .iter()
        .any(|turn| turn.turn_id == request.turn_id);
    let has_answer = existing.turns.iter().any(|turn| turn.turn_id == answer_id);

    if has_answer {
        return Ok(existing);
    }

    let mut turns = Vec::with_capacity(if has_user { 1 } else { 2 });
    if !has_user {
        turns.push(user_turn_(&request));
    }
    turns.push(assistant_turn_(&request, answer_id));

    service
        .store_
        .append_turns(&workspace_id, &session_id, &turns)
        .may_cancel_with(cancel.child_token())
        .await
        .map_err(store_error_)?;

    service
        .store_
        .get_session(&workspace_id, &session_id)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// 一次提问对应的用户回合。
fn user_turn_(request: &AskRequest) -> Turn {
    Turn {
        turn_id: request.turn_id.clone(),
        role: Role::User,
        text: request.question.clone(),
        reasoning: String::new(),
        state: TurnState::Done,
        tool_calls: Vec::new(),
        usage: None,
        notice: None,
    }
}

/// 一次提问对应的助手回合标识：**由用户回合标识确定性推导**。
///
/// 不用随机标识，是为了让 [`ask_async`] 的"已经有这一轮回答"判断成立——
/// 否则重试就会多出一条回答。
fn answer_turn_id_(user_turn_id: &TurnId) -> TurnId {
    TurnId::new(format!("{}-a", user_turn_id.as_str()))
}

/// 一次提问对应的助手回合（临时模拟的 LLM：正文是问题的逆序）。
fn assistant_turn_(request: &AskRequest, turn_id: TurnId) -> Turn {
    Turn {
        turn_id,
        role: Role::Assistant,
        text: simulated_answer_(&request.question),
        reasoning: String::new(),
        state: TurnState::Done,
        tool_calls: Vec::new(),
        usage: None,
        notice: Some(Notice {
            message: "（kb_core 临时模拟的 LLM：回答是把问题逆序输出）".to_string(),
            is_error: false,
        }),
    }
}

/// 临时模拟的 LLM：把问题按**字符**（不是字节）逆序输出，中文不会碎成半个。
fn simulated_answer_(question: &str) -> String {
    question.chars().rev().collect()
}

/// [`TrSessionService::remove_session`] 的服务端实现。
#[gen_may_cancel_future(RemoveSession, pub)]
pub async fn remove_session_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    cancel: C,
) -> RpcResult<()>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .remove_session(&workspace_id, &session_id)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

/// [`TrSessionService::rename_session`] 的服务端实现。
///
/// `title` 只有空白时，存储层会回去从首条用户消息推导（见
/// [`Store::rename_session`]），所以"清掉手工起的名字"也是一次普通调用。
#[gen_may_cancel_future(RenameSession, pub)]
pub async fn rename_session_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    title: String,
    cancel: C,
) -> RpcResult<SessionSummary>
where
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(cancelled_());
    }
    service
        .store_
        .rename_session(&workspace_id, &session_id, &title)
        .may_cancel_with(cancel)
        .await
        .map_err(store_error_)
}

// ── 把生成的 future 填进 trait 的关联类型 ─────────────────────────────

// ── mptp_rpc 契约的薄包装 ─────────────────────────────────────────────
//
// 旧的 GAT trait 仍保留给旧传输（P8 删除）；这里把同一批业务函数接上新契约。
// `*_async` 的旧错误类型是 `RpcError<Infallible>`，包装时统一翻成新 `RpcError`
// 的 `Service` 分支；业务 message 原样保留。

/// 握手 service。
pub struct KbHandshake(pub KbService);
/// 工作区 service。
pub struct KbWorkspace(pub KbService);
/// 会话 service。
pub struct KbSession(pub KbService);
/// 生成 service。
pub struct KbGeneration(pub KbService);

#[gen_may_cancel_future(HelloRpc, pub)]
async fn hello_rpc_async<'s, C>(
    service: &'s KbService,
    client: ClientInfo,
    cancel: C,
) -> RpcResult<ServerInfo>
where
    C: TrCancellationToken,
{
    hello_async(service, client, cancel).await
}

#[gen_may_cancel_future(ListWorkspacesRpc, pub)]
async fn list_workspaces_rpc_async<'s, C>(
    service: &'s KbService,
    cancel: C,
) -> RpcResult<WorkspaceList>
where
    C: TrCancellationToken,
{
    list_workspaces_async(service, cancel).await
}

#[gen_may_cancel_future(AddWorkspaceRpc, pub)]
async fn add_workspace_rpc_async<'s, C>(
    service: &'s KbService,
    request: AddWorkspaceRequest,
    cancel: C,
) -> RpcResult<Workspace>
where
    C: TrCancellationToken,
{
    add_workspace_async(service, request, cancel).await
}

#[gen_may_cancel_future(RemoveWorkspaceRpc, pub)]
async fn remove_workspace_rpc_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    cancel: C,
) -> RpcResult<Ack>
where
    C: TrCancellationToken,
{
    remove_workspace_async(service, workspace_id, cancel)
        .await
        .map(|()| Ack)
}

#[gen_may_cancel_future(RenameWorkspaceRpc, pub)]
async fn rename_workspace_rpc_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    name: String,
    cancel: C,
) -> RpcResult<Workspace>
where
    C: TrCancellationToken,
{
    rename_workspace_async(service, workspace_id, name, cancel).await
}

#[gen_may_cancel_future(ListSessionsRpc, pub)]
async fn list_sessions_rpc_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    cancel: C,
) -> RpcResult<SessionList>
where
    C: TrCancellationToken,
{
    list_sessions_async(service, workspace_id, cancel).await
}

#[gen_may_cancel_future(CreateSessionRpc, pub)]
async fn create_session_rpc_async<'s, C>(
    service: &'s KbService,
    request: CreateSessionRequest,
    cancel: C,
) -> RpcResult<SessionSummary>
where
    C: TrCancellationToken,
{
    create_session_async(service, request, cancel).await
}

#[gen_may_cancel_future(GetSessionRpc, pub)]
async fn get_session_rpc_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    cancel: C,
) -> RpcResult<SessionDetail>
where
    C: TrCancellationToken,
{
    get_session_async(service, workspace_id, session_id, cancel).await
}

#[gen_may_cancel_future(RemoveSessionRpc, pub)]
async fn remove_session_rpc_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    cancel: C,
) -> RpcResult<Ack>
where
    C: TrCancellationToken,
{
    remove_session_async(service, workspace_id, session_id, cancel)
        .await
        .map(|()| Ack)
}

#[gen_may_cancel_future(RenameSessionRpc, pub)]
async fn rename_session_rpc_async<'s, C>(
    service: &'s KbService,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    title: String,
    cancel: C,
) -> RpcResult<SessionSummary>
where
    C: TrCancellationToken,
{
    rename_session_async(service, workspace_id, session_id, title, cancel).await
}

#[gen_may_cancel_future(AskRpc, pub)]
async fn ask_rpc_async<'s, C>(
    service: &'s KbService,
    request: AskRequest,
    cancel: C,
) -> RpcResult<SessionDetail>
where
    C: TrCancellationToken,
{
    ask_async(service, request, cancel).await
}

#[rpc(service_of(KbApiV1))]
impl TrHandshakeRpc for KbHandshake {
    fn hello<'f>(
        &'f self,
        client: ClientInfo,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<ServerInfo>> {
        HelloRpcAsync::new(&self.0, client)
    }
}

#[rpc(service_of(KbApiV1))]
impl TrWorkspaceRpc for KbWorkspace {
    fn list_workspaces<'f>(
        &'f self,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<WorkspaceList>> {
        ListWorkspacesRpcAsync::new(&self.0)
    }

    fn add_workspace<'f>(
        &'f self,
        request: AddWorkspaceRequest,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Workspace>> {
        AddWorkspaceRpcAsync::new(&self.0, request)
    }

    fn remove_workspace<'f>(
        &'f self,
        workspace_id: WorkspaceId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Ack>> {
        RemoveWorkspaceRpcAsync::new(&self.0, workspace_id)
    }

    fn rename_workspace<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        name: String,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Workspace>> {
        RenameWorkspaceRpcAsync::new(&self.0, workspace_id, name)
    }
}

#[rpc(service_of(KbApiV1))]
impl TrSessionRpc for KbSession {
    fn list_sessions<'f>(
        &'f self,
        workspace_id: WorkspaceId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionList>> {
        ListSessionsRpcAsync::new(&self.0, workspace_id)
    }

    fn create_session<'f>(
        &'f self,
        request: CreateSessionRequest,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionSummary>> {
        CreateSessionRpcAsync::new(&self.0, request)
    }

    fn get_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionDetail>> {
        GetSessionRpcAsync::new(&self.0, workspace_id, session_id)
    }

    fn remove_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<Ack>> {
        RemoveSessionRpcAsync::new(&self.0, workspace_id, session_id)
    }

    fn rename_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        title: String,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionSummary>> {
        RenameSessionRpcAsync::new(&self.0, workspace_id, session_id, title)
    }
}

#[rpc(service_of(KbApiV1))]
impl TrGenerationRpc for KbGeneration {
    fn ask<'f>(
        &'f self,
        request: AskRequest,
    ) -> impl TrMayCancel<'f, MayCancelOutput = RpcResult<SessionDetail>> {
        AskRpcAsync::new(&self.0, request)
    }
}
