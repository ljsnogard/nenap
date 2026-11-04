//! 连接装配与服务端派发。

use abs_kb_svc::v1::desktop_rpc::{
    KbApiV1, RpcError, TrGeneration, TrHandshake, TrSessionService, TrWorkspaceService,
};
use rpc_core_v1::preludes::{CoreAlloc, MptpServer, Router, RpcRouter, Shared, TrRpcService};
use smux_v1::connection::Dock;
use smux_v1::flow_ctrl::DefaultPolicy;
use smux_v1::handshake::agent::{AcceptAllEntries, HandshakeAgent};
use smux_v1::handshake::opts::BasicOpts;
use smux_v1::x_deps::abs_smux::conn::TrConnection;

use crate::config_::{Conn, K_LISTEN_RESERVE, K_SERVING_DOCK, Rx, SrvCfg, Tx};
use crate::error_::MptpIpcError;

/// 四个按域 service 的集合。
pub struct KbServiceSet<H, W, S, G> {
    /// 握手。
    pub handshake: H,
    /// 工作区。
    pub workspace: W,
    /// 会话。
    pub session: S,
    /// 生成。
    pub generation: G,
}

/// 一条已建立的本机连接（服务端视角）。
pub struct Connection {
    /// 复用连接。
    conn_: Conn,
}

impl Connection {
    pub(crate) fn new_(conn: Conn) -> Self {
        Self { conn_: conn }
    }

    /// 服务这条连接，直到对端断开。
    pub async fn serve<H, W, S, G>(
        self,
        services: KbServiceSet<H, W, S, G>,
    ) -> Result<(), MptpIpcError>
    where
        H: TrHandshake + TrRpcService<KbApiV1, SrvCfg, Err = RpcError> + 'static,
        W: TrWorkspaceService + TrRpcService<KbApiV1, SrvCfg, Err = RpcError> + 'static,
        S: TrSessionService + TrRpcService<KbApiV1, SrvCfg, Err = RpcError> + 'static,
        G: TrGeneration + TrRpcService<KbApiV1, SrvCfg, Err = RpcError> + 'static,
    {
        let shared = Shared::new(self.conn_, CoreAlloc);
        let mut binding = shared
            .bind_async(Dock::new(K_SERVING_DOCK))
            .await
            .map_err(|error| MptpIpcError::Serve(error.to_string()))?;

        let KbServiceSet {
            handshake,
            workspace,
            session,
            generation,
        } = services;
        let router = Router::<SrvCfg, _>::new().call(move |ctx| {
            RpcRouter::<KbApiV1>::new(ctx)
                .add_service::<H>(move |_| handshake)
                .add_service::<W>(move |_| workspace)
                .add_service::<S>(move |_| session)
                .add_service::<G>(move |_| generation)
        });
        let server = MptpServer::<SrvCfg, _>::new(router);
        server
            .serve_listener_async(&mut binding, K_LISTEN_RESERVE)
            .await
            .map_err(|error| MptpIpcError::Serve(error.to_string()))
    }
}

/// 服务端在一条 UDS 流上完成 smux 握手。
pub(crate) async fn handshake_listen_(tx: Tx, rx: Rx) -> Result<Conn, MptpIpcError> {
    let opts = BasicOpts::default();
    let delivery = HandshakeAgent::new(tx, rx)
        .listen_async(&opts, AcceptAllEntries)
        .await
        .map_err(|error| MptpIpcError::Connect(format!("smux 握手失败: {error:?}")))?;
    let (delivery, cfg) = smux_v1::connection::DefaultConnCfg::new(delivery, DefaultPolicy);
    Ok(smux_v1::connection::MuxConnection::new(delivery, cfg))
}

/// 客户端在一条 UDS 流上完成 smux 握手。
pub(crate) async fn handshake_invite_(tx: Tx, rx: Rx) -> Result<Conn, MptpIpcError> {
    let opts = BasicOpts::default();
    let delivery = HandshakeAgent::new(tx, rx)
        .invite_async(&opts, AcceptAllEntries)
        .await
        .map_err(|error| MptpIpcError::Connect(format!("smux 握手失败: {error:?}")))?;
    let (delivery, cfg) = smux_v1::connection::DefaultConnCfg::new(delivery, DefaultPolicy);
    Ok(smux_v1::connection::MuxConnection::new(delivery, cfg))
}
