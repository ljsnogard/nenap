//! # kb_rig_llm_v1_agent
//!
//! 🚧 **WIP**：使用 rig 直接连接 LLM provider 的插件进程。
//!
//! 当前状态：插件线协议尚未落地，因此本 crate **还没有接入** `kb_core` 的
//! UDS/MPTP 主链路。它负责维护对话上下文、把 rig 的原始事件交给
//! `kb_rig_llm_v1_adapt` 做语义映射；接入主链路的方式会在插件协议定下来之后补。
//!
//! `abs_llm::v1` 提供与 provider 无关的语义类型（`Capabilities` /
//! `FinishReason` 等），本 crate 直接复用，不维护镜像枚举。

pub mod protocol;
