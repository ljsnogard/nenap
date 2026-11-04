//! `kb_core` 与 `kb_svc_mptp_ipc::Client` 的端到端往返：真子进程、真 UDS、真 mptp_rpc。
//!
//! 这是迁移后的主验收：服务端是 `kb-core` 可执行文件，客户端是新传输门面；
//! 覆盖应用层握手、工作区 / 会话 CRUD 与一次提问。

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

use abs_kb_svc::v1::desktop::{
    AddWorkspaceRequest, AskRequest, ClientInfo, CreateSessionRequest, ErrorCode, LocalId,
    PROTOCOL_VERSION, SessionId, TurnId, WorkspaceId,
};
use kb_svc_mptp_ipc::{Client, KbRpcError};

/// 子进程守卫：测试结束（包括 panic）时结束它。
struct ChildGuard_(Child);

impl Drop for ChildGuard_ {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// 测试本机 UDS + mptp_rpc 的完整往返。
/// Test the full local UDS + mptp_rpc round trip.
///
/// - 手段：以 `--handshake-prompt stdio` 起 `kb-core` 子进程，读它就绪那一行，
///   再用 `kb_svc_mptp_ipc::Client` 依次做 hello / 工作区 / 会话 / 提问。
/// - 判断：每一步都返回成功，且提问后的会话里有两条新消息。
///
/// - Method: spawn `kb-core` with `--handshake-prompt stdio`, read the ready line,
///   then drive hello / workspace / session / ask through the new client.
/// - Assert: every step succeeds and the asked session has two new turns.
#[test]
fn round_trip_over_uds_() {
    let root = tempfile::tempdir().expect("应当能建临时目录");
    let runtime_dir = root.path().join("run");
    let storage_dir = root.path().join("data");

    let mut child = Command::new(env!("CARGO_BIN_EXE_kb-core"))
        .arg("--handshake-prompt")
        .arg("stdio")
        .arg("--runtime-dir")
        .arg(&runtime_dir)
        .arg("--storage-dir")
        .arg(&storage_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("应当能启动 kb-core");
    let stdout = child.stdout.take().expect("应当有 stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("应当能读到就绪通知");
    assert!(line.contains("ipc_ready"), "实际通知: {line}");
    let _guard = ChildGuard_(child);

    let client = Client::connect_with_timeout(&runtime_dir, std::time::Duration::from_secs(10))
        .expect("应当能连上 kb-core");

    let server = futures_lite::future::block_on(async {
        client
            .hello(ClientInfo {
                client_name: "round-trip-test".to_string(),
                client_version: "0.1.0".to_string(),
                protocol_version: PROTOCOL_VERSION,
            })
            .await
    })
    .expect("握手应当成功");
    assert_eq!(server.protocol_version, PROTOCOL_VERSION);

    // 业务错误：不存在的工作区。服务端用 460–466 状态码，客户端剥离前缀还原。
    let missing = futures_lite::future::block_on(async {
        client
            .get_session(WorkspaceId::new("w-404"), SessionId::new("s-404"))
            .await
    })
    .expect_err("不存在的工作区应当报错");
    match missing {
        KbRpcError::Business(reply) => {
            assert_eq!(reply.code, ErrorCode::NotFound);
            assert!(!reply.message.is_empty());
        }
        other => panic!("应当是业务错误，实际: {other:?}"),
    }

    let workspaces =
        futures_lite::future::block_on(async { client.list_workspaces().await }).expect("列工作区");
    assert!(workspaces.workspaces.is_empty());

    let workspace = futures_lite::future::block_on(async {
        client
            .add_workspace(AddWorkspaceRequest {
                local_id: LocalId::new("l-1"),
                name: "笔记".to_string(),
                path: "/tmp/notes".to_string(),
            })
            .await
    })
    .expect("新增工作区应当成功");

    let session = futures_lite::future::block_on(async {
        client
            .create_session(CreateSessionRequest {
                local_id: LocalId::new("l-2"),
                workspace_id: workspace.workspace_id.clone(),
                title: Some("第一段对话".to_string()),
                turns: Vec::new(),
            })
            .await
    })
    .expect("新建会话应当成功");

    let detail = futures_lite::future::block_on(async {
        client
            .ask(AskRequest {
                workspace_id: workspace.workspace_id.clone(),
                session_id: session.session_id.clone(),
                turn_id: TurnId::new("t-1"),
                question: "你好".to_string(),
                service_id: None,
            })
            .await
    })
    .expect("提问应当成功");
    assert_eq!(detail.turns.len(), 2, "一问一答应当落两条消息");

    futures_lite::future::block_on(async {
        client
            .remove_session(
                workspace.workspace_id.clone(),
                SessionId::new(detail.summary.session_id.as_str()),
            )
            .await
    })
    .expect("删除会话应当成功");
}
