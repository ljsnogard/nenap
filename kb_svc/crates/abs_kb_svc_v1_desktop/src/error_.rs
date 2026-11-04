//! 业务错误。
//!
//! 这里只表达**业务**错误（请求不合法、目标不存在、缺少 API key……）。
//! **传输层**错误（对端断开、编解码失败）不属于协议，而是实现 crate 的错误类型——
//! 两类错误混在一个类型里会让调用方无法判断"该重试还是该提示用户"。

/// 业务错误的类别。
///
/// 按工作区 / 会话 / 生成的需要定义；到线上是 460–466 的业务状态码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
#[denco(rename_all = "snake_case")]
pub enum ErrorCode {
    /// 请求本身不合法（问题为空、同一会话已有进行中的回合等）。
    BadRequest,

    /// 目标不存在（工作区、会话等）。
    NotFound,

    /// 选定的服务不存在。
    UnknownService,

    /// 选定的服务还没有配置 API key。
    MissingApiKey,

    /// 插件当前不在线。
    PluginOffline,

    /// 插件上报了 provider 侧的错误。
    Provider,

    /// 服务端内部错误。
    Internal,
}

/// 一条业务错误应答。
///
/// 作为 [`Reply::Error`](crate::Reply::Error) 的载荷返回给请求方；
/// 与生成过程相关、需要主动推送的错误另外走
/// [`Event::Error`](crate::Event::Error)。
///
/// 它同时实现 [`core::error::Error`]，这样按域 RPC trait 可以直接拿它当
/// "业务失败"那一个分支（见 [`RpcError::Business`](crate::RpcError::Business)），
/// 而不必为每种实现再包一层。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct ErrorReply {
    /// 错误类别。
    pub code: ErrorCode,

    /// 面向用户的说明。
    pub message: String,
}

impl core::fmt::Display for ErrorReply {
    /// 只打印面向用户的说明。
    ///
    /// 类别不参与 `Display`：它已经在 [`ErrorReply::code`] 里，日志里需要时
    /// 用 `{:?}` 打印整个结构即可；把类别拼进这句人话里反而会让界面上
    /// 显示的文案多出一段机器词。
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl core::error::Error for ErrorReply {}
