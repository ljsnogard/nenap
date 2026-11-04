//! 客户端门面：宿主线程 + 作业队列。

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use abs_cancel::{TrCancellationToken, TrMayCancel};
use abs_kb_svc::v1::desktop::{
    AddWorkspaceRequest, AskRequest, ClientInfo, CreateSessionRequest, ErrorCode, ErrorReply,
    Reply, ReplyEnvelope, Request, RequestEnvelope, ServerInfo, SessionDetail, SessionId,
    SessionList, SessionSummary, Workspace, WorkspaceId, WorkspaceList,
};
use abs_kb_svc::v1::desktop_rpc::{
    KbApiV1, KbRpcError, RpcError, TrGeneration, TrHandshake, TrSessionService, TrWorkspaceService,
    business_error_from_rpc_,
};
use futures_channel::{mpsc::UnboundedSender, oneshot};
use gen_mcf2::gen_may_cancel_future;
use rpc_core_v1::client::TrRpcClientExt;
use rpc_core_v1::preludes::MptpClient;

use crate::config_::CliCfg;
use crate::error_::MptpIpcError;
use crate::host_::{Job, connect_host_};
use crate::rendezvous_::DEFAULT_CONNECT_TIMEOUT;

/// `kb_core` 的本机客户端。
///
/// 它**是** `Send + Sync` 的：连接、smux 握手与 `MptpClient` 都在宿主线程上，
/// 调用方只投递作业并等完成量。调用方因此不需要选运行时。
pub struct Client {
    /// 作业队列。
    job_tx_: UnboundedSender<Job>,
}

impl Client {
    /// 连上 `runtime_dir` 下最新公布的 socket，缺省最多等 [`DEFAULT_CONNECT_TIMEOUT`]。
    pub fn connect(runtime_dir: impl AsRef<std::path::Path>) -> Result<Self, MptpIpcError> {
        Self::connect_with_timeout(runtime_dir, DEFAULT_CONNECT_TIMEOUT)
    }

    /// 同上，但自定等待时长。
    pub fn connect_with_timeout(
        runtime_dir: impl AsRef<std::path::Path>,
        timeout: Duration,
    ) -> Result<Self, MptpIpcError> {
        let job_tx_ = connect_host_(runtime_dir.as_ref().to_path_buf(), timeout)?;
        Ok(Self { job_tx_ })
    }

    /// 实现一个方法：造作业、投递、等结果。
    fn submit_<'f, T>(
        &'f self,
        make: impl FnOnce(oneshot::Sender<Result<T, KbRpcError<MptpIpcError>>>) -> Job,
    ) -> WaitAsync<'f, 'f, T>
    where
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let job = make(tx);
        WaitAsync::new(self, job, rx)
    }

    /// 应用层握手。
    pub fn hello<'f>(&'f self, client: ClientInfo) -> WaitAsync<'f, 'f, ServerInfo> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc.call::<KbApiV1>().hello(client).await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 列出全部工作区。
    pub fn list_workspaces<'f>(&'f self) -> WaitAsync<'f, 'f, WorkspaceList> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc.call::<KbApiV1>().list_workspaces().await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 登记一个工作区。
    pub fn add_workspace<'f>(
        &'f self,
        request: AddWorkspaceRequest,
    ) -> WaitAsync<'f, 'f, Workspace> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc.call::<KbApiV1>().add_workspace(request).await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 删除一个工作区。
    pub fn remove_workspace<'f>(&'f self, workspace_id: WorkspaceId) -> WaitAsync<'f, 'f, ()> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc
                        .call::<KbApiV1>()
                        .remove_workspace(workspace_id)
                        .await;
                    let _ = sender.send(result.map(|_ack| ()).map_err(classify_));
                })
            })
        })
    }

    /// 重命名一个工作区。
    pub fn rename_workspace<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        name: String,
    ) -> WaitAsync<'f, 'f, Workspace> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc
                        .call::<KbApiV1>()
                        .rename_workspace(workspace_id, name)
                        .await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 列出某个工作区下的会话。
    pub fn list_sessions<'f>(
        &'f self,
        workspace_id: WorkspaceId,
    ) -> WaitAsync<'f, 'f, SessionList> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc
                        .call::<KbApiV1>()
                        .list_sessions(workspace_id)
                        .await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 登记一个会话。
    pub fn create_session<'f>(
        &'f self,
        request: CreateSessionRequest,
    ) -> WaitAsync<'f, 'f, SessionSummary> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc.call::<KbApiV1>().create_session(request).await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 读取一个会话的完整内容。
    pub fn get_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> WaitAsync<'f, 'f, SessionDetail> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc
                        .call::<KbApiV1>()
                        .get_session(workspace_id, session_id)
                        .await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 删除一个会话。
    pub fn remove_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> WaitAsync<'f, 'f, ()> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc
                        .call::<KbApiV1>()
                        .remove_session(workspace_id, session_id)
                        .await;
                    let _ = sender.send(result.map(|_ack| ()).map_err(classify_));
                })
            })
        })
    }

    /// 重命名一个会话。
    pub fn rename_session<'f>(
        &'f self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        title: String,
    ) -> WaitAsync<'f, 'f, SessionSummary> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc
                        .call::<KbApiV1>()
                        .rename_session(workspace_id, session_id, title)
                        .await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 就某个会话提问。
    pub fn ask<'f>(&'f self, request: AskRequest) -> WaitAsync<'f, 'f, SessionDetail> {
        self.submit_(move |sender| {
            Box::new(move |client_rc: std::rc::Rc<MptpClient<CliCfg>>| {
                Box::pin(async move {
                    let result = client_rc.call::<KbApiV1>().ask(request).await;
                    let _ = sender.send(result.map_err(classify_));
                })
            })
        })
    }

    /// 发一个**原始信封**并等它的应答（`request_id` 由调用方给）。
    ///
    /// 这是给代理/网关用的低层入口：调用方自己负责业务分派，本方法只负责
    /// "把信封送到、把对应 `request_id` 的应答拿回来"。
    pub fn send_envelope<'f>(&'f self, envelope: RequestEnvelope) -> SendEnvelopeAsync<'f, 'f> {
        SendEnvelopeAsync::new(self, envelope)
    }
}

/// [`Client::send_envelope`] 的实现体：把信封翻成一次 typed 调用。
#[gen_may_cancel_future(SendEnvelope, pub)]
async fn send_envelope_async<'c, C>(
    client: &'c Client,
    envelope: RequestEnvelope,
    cancel: C,
) -> Result<ReplyEnvelope, KbRpcError<MptpIpcError>>
where
    C: TrCancellationToken,
{
    let request_id = envelope.request_id.clone();
    let token = || cancel.child_token();
    let reply = match envelope.request {
        Request::Hello(info) => Reply::Hello(client.hello(info).may_cancel_with(token()).await?),
        Request::ListWorkspaces => {
            Reply::WorkspaceList(client.list_workspaces().may_cancel_with(token()).await?)
        }
        Request::AddWorkspace(request) => {
            let local_id = request.local_id.clone();
            let workspace = client
                .add_workspace(request)
                .may_cancel_with(token())
                .await?;
            Reply::WorkspaceAdded {
                local_id,
                workspace,
            }
        }
        Request::RemoveWorkspace { workspace_id } => {
            client
                .remove_workspace(workspace_id)
                .may_cancel_with(token())
                .await?;
            Reply::Ack
        }
        Request::RenameWorkspace { workspace_id, name } => Reply::WorkspaceRenamed(
            client
                .rename_workspace(workspace_id, name)
                .may_cancel_with(token())
                .await?,
        ),
        Request::ListSessions { workspace_id } => Reply::SessionList(
            client
                .list_sessions(workspace_id)
                .may_cancel_with(token())
                .await?,
        ),
        Request::CreateSession(request) => {
            let local_id = request.local_id.clone();
            let session = client
                .create_session(request)
                .may_cancel_with(token())
                .await?;
            Reply::SessionCreated { local_id, session }
        }
        Request::GetSession {
            workspace_id,
            session_id,
        } => Reply::SessionDetail(
            client
                .get_session(workspace_id, session_id)
                .may_cancel_with(token())
                .await?,
        ),
        Request::RemoveSession {
            workspace_id,
            session_id,
        } => {
            client
                .remove_session(workspace_id, session_id)
                .may_cancel_with(token())
                .await?;
            Reply::Ack
        }
        Request::RenameSession {
            workspace_id,
            session_id,
            title,
        } => Reply::SessionRenamed(
            client
                .rename_session(workspace_id, session_id, title)
                .may_cancel_with(token())
                .await?,
        ),
        Request::Ask(request) => {
            Reply::SessionDetail(client.ask(request).may_cancel_with(token()).await?)
        }
        other => Reply::Error(ErrorReply {
            code: ErrorCode::BadRequest,
            message: format!(
                "这条请求还没有实现（对应的按域 service 尚未落地）: {}",
                request_kind_(&other)
            ),
        }),
    };
    Ok(ReplyEnvelope::new(request_id, reply))
}

/// 取请求变体的名字（只用于错误信息）。
fn request_kind_(request: &Request) -> &'static str {
    match request {
        Request::Hello(_) => "Hello",
        Request::Ask(_) => "Ask",
        Request::Cancel { .. } => "Cancel",
        Request::ListServices => "ListServices",
        Request::UpsertService(_) => "UpsertService",
        Request::RemoveService { .. } => "RemoveService",
        Request::UseService { .. } => "UseService",
        Request::ListWorkspaces => "ListWorkspaces",
        Request::AddWorkspace(_) => "AddWorkspace",
        Request::RemoveWorkspace { .. } => "RemoveWorkspace",
        Request::RenameWorkspace { .. } => "RenameWorkspace",
        Request::ListSessions { .. } => "ListSessions",
        Request::CreateSession(_) => "CreateSession",
        Request::RemoveSession { .. } => "RemoveSession",
        Request::RenameSession { .. } => "RenameSession",
        Request::GetSession { .. } => "GetSession",
        Request::ListDirectory { .. } => "ListDirectory",
    }
}

/// 把 `RpcResult` 的失败翻成「业务 / 传输」两分。
fn classify_(rpc: RpcError) -> KbRpcError<MptpIpcError> {
    match business_error_from_rpc_(&rpc) {
        Some(reply) => KbRpcError::Business(reply),
        None => KbRpcError::Transport(MptpIpcError::Rpc(rpc)),
    }
}

/// 等待一次作业完成，同时响应取消令牌。
#[gen_may_cancel_future(Wait, pub, new(pub))]
async fn wait_async<'c, T, C>(
    client: &'c Client,
    job: Job,
    rx: oneshot::Receiver<Result<T, KbRpcError<MptpIpcError>>>,
    cancel: C,
) -> Result<T, KbRpcError<MptpIpcError>>
where
    T: Send + 'static,
    C: TrCancellationToken,
{
    if cancel.is_cancelled() {
        return Err(KbRpcError::Transport(MptpIpcError::Rpc(
            RpcError::Cancelled,
        )));
    }
    client
        .job_tx_
        .unbounded_send(job)
        .map_err(|_| KbRpcError::Transport(MptpIpcError::HostGone))?;

    let mut rx = rx;
    let cancellation = cancel.cancellation();
    let mut cancellation = std::pin::pin!(cancellation);

    std::future::poll_fn(move |context| {
        if let Poll::Ready(outcome) = Pin::new(&mut rx).poll(context) {
            return Poll::Ready(match outcome {
                Ok(result) => result,
                Err(_) => Err(KbRpcError::Transport(MptpIpcError::HostGone)),
            });
        }
        if Pin::new(&mut cancellation).poll(context).is_ready() {
            return Poll::Ready(Err(KbRpcError::Transport(MptpIpcError::Rpc(
                RpcError::Cancelled,
            ))));
        }
        Poll::Pending
    })
    .await
}
