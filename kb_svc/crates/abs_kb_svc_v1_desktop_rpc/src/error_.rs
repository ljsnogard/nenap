//! 业务错误 ↔ RPC 错误的桥。
//!
//! 服务端把 `ErrorCode` 写成 460–466 的状态码；客户端从 `RpcError::Remote`
//! 还原 `ErrorReply`。业务错误与传输错误仍分成两个变体，调用方据此决定
//! "提示用户"还是"重试/换连接方式"。

use core::fmt;

use abs_kb_svc_v1_desktop::{ErrorCode, ErrorReply};
use rpc_core_v1::messaging::RpcError;
use rpc_core_v1::x_deps::abs_mptp::spec::Status;

/// 业务状态码命名空间：460–499，避开 MPTP/HTTP 已用的 400/404/500/504。
pub const K_BUSINESS_STATUS_BASE: u16 = 460;

/// 上游 `RpcError::Service` 的 Display 在前缀与 message 之间使用的分隔符。
const K_DISPLAY_SEPARATOR_: &str = "）：";

/// 一次 RPC 调用的失败：业务失败与传输失败分开。
///
/// `E` 由具体传输给出（本机是 `MptpIpcError`，跨机是 `TcpError`）。
#[derive(Debug)]
pub enum KbRpcError<E> {
    /// 服务端明确拒绝；`code` 与 `message` 已从线上还原。
    Business(ErrorReply),

    /// 连接、编解码、取消等传输层失败。
    Transport(E),
}

impl<E> KbRpcError<E> {
    /// 取出业务错误；是传输错误时返回 `None`。
    pub fn as_business(&self) -> Option<&ErrorReply> {
        match self {
            Self::Business(reply) => Some(reply),
            Self::Transport(_) => None,
        }
    }

    /// 是不是传输层失败。
    pub fn is_transport(&self) -> bool {
        matches!(self, Self::Transport(_))
    }

    /// 把传输错误换一种类型，业务错误原样保留。
    pub fn map_transport<F>(self, map: impl FnOnce(E) -> F) -> KbRpcError<F> {
        match self {
            Self::Business(reply) => KbRpcError::Business(reply),
            Self::Transport(error) => KbRpcError::Transport(map(error)),
        }
    }
}

impl<E: fmt::Display> fmt::Display for KbRpcError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Business(reply) => write!(formatter, "业务错误: {reply}"),
            Self::Transport(error) => write!(formatter, "传输错误: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for KbRpcError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Business(reply) => Some(reply),
            Self::Transport(error) => Some(error),
        }
    }
}

/// `ErrorCode` → 460–466 的业务状态码。
///
/// 用显式 match 而不是枚举判别值：顺序就是线上契约，必须稳定。
pub fn status_of_error_code(code: ErrorCode) -> Status {
    let offset = match code {
        ErrorCode::BadRequest => 0,
        ErrorCode::NotFound => 1,
        ErrorCode::UnknownService => 2,
        ErrorCode::MissingApiKey => 3,
        ErrorCode::PluginOffline => 4,
        ErrorCode::Provider => 5,
        ErrorCode::Internal => 6,
    };
    Status::new(K_BUSINESS_STATUS_BASE + offset)
}

/// 460–466 → `ErrorCode`；其它状态码返回 `None`。
pub fn error_code_of_status(status: Status) -> Option<ErrorCode> {
    let offset = status.inner().checked_sub(K_BUSINESS_STATUS_BASE)?;
    Some(match offset {
        0 => ErrorCode::BadRequest,
        1 => ErrorCode::NotFound,
        2 => ErrorCode::UnknownService,
        3 => ErrorCode::MissingApiKey,
        4 => ErrorCode::PluginOffline,
        5 => ErrorCode::Provider,
        6 => ErrorCode::Internal,
        _ => return None,
    })
}

/// 若这个 `RpcError` 是 460–466 的业务错误，剥离 Display 前缀并还原 `ErrorReply`。
pub fn business_error_from_rpc_(rpc: &RpcError) -> Option<ErrorReply> {
    let RpcError::Remote { status, body } = rpc else {
        return None;
    };
    let code = error_code_of_status(*status)?;
    let text = core::str::from_utf8(body).ok()?;
    let (_prefix, message) = text.split_once(K_DISPLAY_SEPARATOR_)?;
    Some(ErrorReply {
        code,
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests_ {
    use super::*;

    /// 测试状态码与 `ErrorCode` 一一对应。
    /// Test that status codes and `ErrorCode` map one-to-one.
    ///
    /// - 手段：遍历七个 `ErrorCode`，先转状态码再转回。
    /// - 判断：转回的值与原值相等；460 之前与 467 之后返回 `None`。
    ///
    /// - Method: round-trip all seven codes through the status namespace.
    /// - Assert: every code comes back unchanged; out-of-range statuses are `None`.
    #[test]
    fn error_code_status_round_trip_() {
        for code in [
            ErrorCode::BadRequest,
            ErrorCode::NotFound,
            ErrorCode::UnknownService,
            ErrorCode::MissingApiKey,
            ErrorCode::PluginOffline,
            ErrorCode::Provider,
            ErrorCode::Internal,
        ] {
            let status = status_of_error_code(code);
            assert_eq!(error_code_of_status(status), Some(code));
        }
        assert!(error_code_of_status(Status::new(459)).is_none());
        assert!(error_code_of_status(Status::new(467)).is_none());
    }

    /// 测试能从上游 Display 文本中还原原始 message。
    /// Test that the original message is recovered from the upstream Display text.
    ///
    /// - 手段：构造 `RpcError::Service`，把它的 `to_string()` 当作远端错误体，
    ///   message 里带一个 `"）："` 以验证只取第一个分隔符。
    /// - 判断：还原出的 `ErrorReply` 的 code 与 message 都与输入一致。
    ///
    /// - Method: build `RpcError::Service`, use its `to_string()` as the remote body,
    ///   with a `）：` inside the message.
    /// - Assert: the recovered code and message equal the inputs.
    #[test]
    fn business_message_is_recovered_() {
        let status = status_of_error_code(ErrorCode::NotFound);
        let service = RpcError::Service {
            status,
            message: "工作区）：不存在".to_string(),
        };
        let remote = RpcError::Remote {
            status,
            body: service.to_string().into_bytes(),
        };

        let reply = business_error_from_rpc_(&remote).expect("应当能还原");
        assert_eq!(reply.code, ErrorCode::NotFound);
        assert_eq!(reply.message, "工作区）：不存在");
    }
}
