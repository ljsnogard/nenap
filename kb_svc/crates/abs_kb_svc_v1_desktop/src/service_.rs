//! LLM 服务配置，以及 API key 的更新语义。
//!
//! # API key 的方向性
//!
//! 明文 key **只有一个方向会出现**：客户端 → `kb_core`（
//! [`ApiKeyUpdate::Set`]）。反方向（`kb_core` → 客户端）永远只有掩码，
//! 因为 [`ServiceSummary`] 里根本没有明文字段。需要明文的是**插件**，
//! 由 `kb_core` 在 `v1::plugin` 那一侧交给它。

use super::ids_::ServiceId;

/// API key 的掩码。
///
/// 与 `kb_svc_salvo::settings::MASKED_KEY` 以及桌面客户端的 `kMaskedApiKey`
/// 保持同一个字符串（U+2022 重复 8 次）。
///
/// **兼容性要求**：服务端在收到"把掩码原样回传"的旧式客户端时，
/// 必须把它解释为 [`ApiKeyUpdate::Keep`]，而不是把掩码写进配置。
pub const MASKED_API_KEY: &str = "••••••••";

/// API key 的更新语义。
///
/// 客户端拿到的服务配置里 key 永远是掩码，因此"编辑服务"这个动作必须能表达
/// "我没改 key"与"我要清空 key"两件事，不能只靠一个字符串。
///
/// # 示例
///
/// ```
/// use abs_kb_svc_v1_desktop::ApiKeyUpdate;
///
/// let keep = ApiKeyUpdate::Keep;
/// let set = ApiKeyUpdate::Set {
///     value: "sk-test".to_string(),
/// };
/// let clear = ApiKeyUpdate::Clear;
///
/// // 三者是不同的语义
/// assert_ne!(keep, set);
/// assert_ne!(set, clear);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub enum ApiKeyUpdate {
    /// 保持原有 key 不变。
    Keep,

    /// 设置为新值。
    Set {
        /// 新的 API key 明文（仅在客户端 → 服务端方向出现）。
        value: String,
    },

    /// 清空已有 key。
    Clear,
}

/// 一个 LLM 服务的对外摘要。
///
/// **注意**：这里只出现掩码，明文 key 永不出现在任何服务端 → 客户端的方向上。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct ServiceSummary {
    /// 服务标识（用户取的名字）。
    pub service_id: ServiceId,

    /// provider 标识，例如 `deepseek`。
    pub provider: String,

    /// 模型名，例如 `deepseek-chat`。
    pub model: String,

    /// API base URL；空表示用 provider 默认地址。
    pub base_url: String,

    /// 是否为当前生效的服务。
    pub active: bool,

    /// 是否已经配置了 API key。
    pub has_api_key: bool,

    /// 掩码后的 key，仅供界面回显；未配置时为空串。
    pub api_key_masked: String,
}

/// 服务列表及其生效项。
#[derive(
    Debug, Clone, Default, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable,
)]
pub struct ServiceList {
    /// 已配置的服务。
    pub services: Vec<ServiceSummary>,

    /// 当前生效的服务标识；没有可用服务时为 `None`。
    pub active_service: Option<ServiceId>,
}
