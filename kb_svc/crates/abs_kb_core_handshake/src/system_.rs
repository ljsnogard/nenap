//! 系统层握手通知的类型与 JSON 编解码：`kb_core` 就绪后公布本机 socket 的那条消息。
//!
//! 编解码统一走 `json_denco`（denco 的 JSON 实现）；本 crate 不再依赖 serde，也不使用 postcard。
//! 通知很小（socket 路径 + 三个标量），因此用一块固定缓冲即可，不需要堆分配。

use core::future::Future;
use core::task::{Context, Poll, Waker};

use abs_cancel::{NonCancellableToken, TrMayCancel};
use abs_denco::{de::TrDecodable, ser::TrEncodable};
use json_denco::{JsonDecoder, JsonEncoder, JsonError};

/// 通知编码用的栈缓冲容量（字节）。
///
/// socket 路径上限约 107 字节，其余字段都是小整数；4 KiB 足够宽裕。
const K_NOTICE_CAP_: usize = 4096usize;

/// [`IpcReadyNotice::event`] 的取值：端点已就绪。
///
/// 单独列成一个类型而不是裸 `String`，是为了让"种类"这件事在编译期就有约束，
/// 也为将来可能出现的其它系统层通知留位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub enum HandshakeNoticeKind {
    /// `kb_core` 已经把本机 socket 准备好，父进程可以照着它去连接了。
    #[denco(rename = "ipc_ready")]
    IpcReady,
}

/// 系统层握手：`kb_core` 就绪后公布本机 socket 的那条通知。
///
/// `kb_core` 以 `--handshake-prompt=stdio` 启动时，会在 `Listener::bind` 之后往
/// **stdout** 打一行该类型的 JSON（打完就 flush；stdout 其余时间保持干净）：
///
/// ```json
/// {"event":"ipc_ready","ipc_name_file":"…/kb-20260918-….sock","protocol_version":1,"pid":1234}
/// ```
///
/// # 谁发、谁收
///
/// - 发：`kb_core`（`--handshake-prompt=stdio`）；
/// - 收：启动它的父进程——`kb_core_starter`（`kb_core_rproxy` 与桌面客户端都用它）。
///
/// 字段名沿用 `ipc_name_file`（消息兼容），语义是**本机 socket 路径**。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct IpcReadyNotice {
    /// 通知种类。
    pub event: HandshakeNoticeKind,

    /// 本机 socket 的路径。
    ///
    /// 语义是"这个 `kb_core` 实例"：socket 文件在进程活着时就在，
    /// 拿到通知后仍可能有短暂连不上（服务端还没进入 `accept`）。
    pub ipc_name_file: String,

    /// `kb_core` 使用的协议版本。
    ///
    /// 它只是报备：真正"版本不一致就拒绝"发生在应用层握手（`Request::Hello`）。
    pub protocol_version: u32,

    /// `kb_core` 的进程号（日志与排错用）。
    pub pid: u32,
}

/// 把通知编成一行 JSON（不含换行）。
///
/// # Errors
///
/// 缓冲不足或 JSON 编码失败时返回 [`JsonError`]。
pub fn encode_notice(notice: &IpcReadyNotice) -> Result<String, JsonError> {
    let mut storage = [0u8; K_NOTICE_CAP_];
    let total = storage.len();
    let mut sink: &mut [u8] = &mut storage;
    block_on_(
        notice
            .encode_async(JsonEncoder::new(&mut sink))
            .may_cancel_with(NonCancellableToken::new()),
    )?;
    let written = total - sink.len();
    String::from_utf8(storage[..written].to_vec()).map_err(|_| JsonError::new("通知不是合法 UTF-8"))
}

/// 从一行 JSON 解出通知（前后空白容忍）。
///
/// # Errors
///
/// 不是合法 JSON、字段缺失或 `event` 取值不认识时返回 [`JsonError`]。
pub fn decode_notice(line: &str) -> Result<IpcReadyNotice, JsonError> {
    let mut input: &[u8] = line.trim().as_bytes();
    let mut decoder = JsonDecoder::new(&mut input);
    block_on_(
        IpcReadyNotice::decode_async(&mut decoder).may_cancel_with(NonCancellableToken::new()),
    )
}

/// 在内存切片上驱动 denco 的异步编解码。
///
/// `json_denco` 的编解码器在 `&mut [u8]` / `&[u8]` 上不会真正挂起，
/// 因此用一次轮询循环即可；这里不做任何阻塞 IO。
fn block_on_<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let mut future = core::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(test)]
mod tests_ {
    use super::*;

    /// 测试通知的 JSON 形状与线格式一致，且能往返。
    /// Test that the notice JSON shape matches the wire format and round-trips.
    ///
    /// - 手段：构造 [`IpcReadyNotice`]，用 `encode_notice` 编码，再用 `decode_notice` 解码。
    /// - 判断：JSON 含 `"event":"ipc_ready"` 与四个字段名；解码结果与原值相等。
    ///
    /// - Method: build a notice, encode it, then decode it.
    /// - Assert: the JSON contains `"event":"ipc_ready"` and the four field names;
    ///   the decoded value equals the original.
    #[test]
    fn ipc_ready_notice_round_trips_() {
        let notice = IpcReadyNotice {
            event: HandshakeNoticeKind::IpcReady,
            ipc_name_file: "/run/user/1000/llm_kb/kb-20260918-abc.sock".to_string(),
            protocol_version: 1,
            pid: 1234,
        };

        let line = encode_notice(&notice).expect("应当能编码");
        assert!(line.contains(r#""event":"ipc_ready""#), "实际 JSON: {line}");
        assert!(
            line.contains(r#""ipc_name_file":"/run/user/1000/llm_kb/kb-20260918-abc.sock""#),
            "实际 JSON: {line}"
        );
        assert!(
            line.contains(r#""protocol_version":1"#),
            "实际 JSON: {line}"
        );
        assert!(line.contains(r#""pid":1234"#), "实际 JSON: {line}");

        let parsed = decode_notice(&line).expect("应当能解码");
        assert_eq!(parsed, notice);
    }

    /// 测试不认识的 `event` 会被拒绝，而不是被当成合法通知放过去。
    /// Test that an unknown `event` is rejected instead of being accepted.
    ///
    /// - 手段：分别喂 `"event":"IpcReady"`（枚举名而不是线上取值）与 `"event":"nope"`。
    /// - 判断：两次解码都返回错误。
    ///
    /// - Method: feed `"event":"IpcReady"` and `"event":"nope"`.
    /// - Assert: both decodes fail.
    #[test]
    fn unknown_event_is_rejected_() {
        for bad in [
            r#"{"event":"IpcReady","ipc_name_file":"/tmp/x.sock","protocol_version":1,"pid":1}"#,
            r#"{"event":"nope","ipc_name_file":"/tmp/x.sock","protocol_version":1,"pid":1}"#,
        ] {
            assert!(decode_notice(bad).is_err(), "不该解开: {bad}");
        }
    }
}
