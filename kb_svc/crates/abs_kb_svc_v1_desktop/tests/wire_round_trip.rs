//! 协议消息在 denco MessagePack 上的往返。
//!
//! 线上与存储都走 denco MessagePack，这里守住
//! "代表性消息能编码再解码回原值"这条底线。

#![recursion_limit = "512"]

use abs_cancel::{NonCancellableToken, TrMayCancel};
use abs_denco::{de::TrDecodableOwned, ser::TrEncodable};
use abs_kb_svc_v1_desktop::{
    AddWorkspaceRequest, AskRequest, ClientInfo, ErrorCode, ErrorReply, Event, LocalId,
    PROTOCOL_VERSION, Reply, ReplyEnvelope, Request, RequestEnvelope, ServerInfo, SessionDetail,
    SessionId, SessionSummary, TextDelta, Turn, TurnId, TurnState, Workspace, WorkspaceId,
};
use abs_llm::v1::cont::{LogicOutput, Role};
use mp_denco::{MpDecoder, MpEncoder};

/// 把值编成 MessagePack 字节。
fn encode_<T: TrEncodable>(value: &T) -> Vec<u8> {
    let mut storage = vec![0u8; 64 * 1024];
    let total = storage.len();
    let written = {
        let mut sink: &mut [u8] = storage.as_mut_slice();
        futures_lite::future::block_on(async {
            value
                .encode_async(MpEncoder::new(&mut sink))
                .may_cancel_with(NonCancellableToken::new())
                .await
        })
        .expect("编码应当成功");
        total - sink.len()
    };
    storage.truncate(written);
    storage
}

/// 从 MessagePack 字节解码。
fn decode_<T: TrDecodableOwned>(bytes: &[u8]) -> T {
    let mut input: &[u8] = bytes;
    futures_lite::future::block_on(async {
        T::decode_async(MpDecoder::new(&mut input))
            .may_cancel_with(NonCancellableToken::new())
            .await
    })
    .expect("解码应当成功")
}

/// 测试代表性协议消息能通过 MessagePack 往返。
/// Test that representative protocol messages round-trip through MessagePack.
///
/// - 手段：构造请求、应答、事件、握手与消息正文，分别编码后解码。
/// - 判断：解码结果与原值完全相等。
///
/// - Method: build requests, replies, events, handshake and content values; encode then decode.
/// - Assert: every decoded value equals the original.
#[test]
fn representative_messages_round_trip_() {
    let workspace_id = WorkspaceId::new("w-1");
    let session_id = SessionId::new("s-1");
    let turn_id = TurnId::new("t-1");

    let requests = vec![
        RequestEnvelope::new(
            "q-1",
            Request::Hello(ClientInfo {
                client_name: "test".to_string(),
                client_version: "0.1.0".to_string(),
                protocol_version: PROTOCOL_VERSION,
            }),
        ),
        RequestEnvelope::new(
            "q-2",
            Request::AddWorkspace(AddWorkspaceRequest {
                local_id: LocalId::new("l-1"),
                name: "笔记".to_string(),
                path: "/tmp/notes".to_string(),
            }),
        ),
        RequestEnvelope::new(
            "q-3",
            Request::Ask(AskRequest {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                question: "你好".to_string(),
                service_id: None,
            }),
        ),
    ];
    for value in &requests {
        let bytes = encode_(value);
        let parsed: RequestEnvelope = decode_(&bytes);
        assert_eq!(&parsed, value);
    }

    let replies = vec![
        ReplyEnvelope::new(
            "q-1",
            Reply::Hello(ServerInfo {
                server_version: "0.1.0".to_string(),
                protocol_version: PROTOCOL_VERSION,
            }),
        ),
        ReplyEnvelope::new(
            "q-2",
            Reply::WorkspaceAdded {
                local_id: LocalId::new("l-1"),
                workspace: Workspace {
                    workspace_id: workspace_id.clone(),
                    name: "笔记".to_string(),
                    path: "/tmp/notes".to_string(),
                },
            },
        ),
        ReplyEnvelope::new(
            "q-3",
            Reply::Error(ErrorReply {
                code: ErrorCode::NotFound,
                message: "工作区不存在".to_string(),
            }),
        ),
    ];
    for value in &replies {
        let bytes = encode_(value);
        let parsed: ReplyEnvelope = decode_(&bytes);
        assert_eq!(&parsed, value);
    }

    let events = vec![
        Event::Delta(TextDelta {
            turn_id: turn_id.clone(),
            logic: LogicOutput::Answer,
            text: "你好".to_string(),
        }),
        Event::SessionChanged(abs_kb_svc_v1_desktop::SessionChanged {
            summary: SessionSummary {
                session_id: session_id.clone(),
                workspace_id: workspace_id.clone(),
                title: "新会话".to_string(),
                updated_at_millis: 1,
                turn_count: 0,
            },
        }),
    ];
    for value in &events {
        let bytes = encode_(value);
        let parsed: Event = decode_(&bytes);
        assert_eq!(&parsed, value);
    }

    let detail = SessionDetail {
        summary: SessionSummary {
            session_id: session_id.clone(),
            workspace_id: workspace_id.clone(),
            title: "第一段".to_string(),
            updated_at_millis: 2,
            turn_count: 1,
        },
        turns: vec![Turn {
            turn_id: turn_id.clone(),
            role: Role::User,
            text: "你好".to_string(),
            reasoning: String::new(),
            state: TurnState::Done,
            tool_calls: Vec::new(),
            usage: None,
            notice: None,
        }],
    };
    let bytes = encode_(&detail);
    let parsed: SessionDetail = decode_(&bytes);
    assert_eq!(parsed, detail);
}
