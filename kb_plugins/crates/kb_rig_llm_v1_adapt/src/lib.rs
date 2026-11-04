//! # kb_rig_llm_v1_adapt
//!
//! 🚧 **WIP**：把 rig 产出的原始数据转换成 `abs_llm::v1` 的语义类型。
//!
//! 当前状态：插件线协议尚未落地，因此本 crate **还没有接入** `kb_core` 的
//! UDS/MPTP 主链路。它只负责：
//!
//! 1. **语义映射**：rig 的流式分片 → `abs_llm::v1` 的 `LogicOutput` /
//!    `FinishReason` / `TrUsage` 等概念；
//! 2. **适配类型**：让 `abs_llm` 的泛型代码可以消费 rig 事件（`attachment` /
//!    `event` 两个模块）。
//!
//! 具体的插件线格式会在插件协议定下来之后补；本 crate 不决定最终传输。
//!
//! # 两类公开类型
//!
//! | 模块 | 用途 |
//! | :--- | :--- |
//! | [`attachment`] | 实现 `abs_llm::v1` 那些 trait 的具体类型，可被抽象层的泛型代码消费 |
//! | [`event`] | rig 流式事件的「可反序列化形态」，以及到上述类型的转换 |
//!
//! # 示例
//!
//! ```
//! use kb_rig_llm_v1_adapt::{attachment::TextDelta, event::LogicOutput};
//!
//! let delta = TextDelta::new(LogicOutput::Reasoning, "先想想");
//!
//! // 既有强类型视图……
//! assert_eq!(delta.logic_kind(), LogicOutput::Reasoning);
//! assert_eq!(delta.text_ref(), "先想想");
//!
//! // ……以及强类型字段本身。
//! assert_eq!(delta.text_ref(), "先想想");
//! ```

pub mod attachment;
pub mod event;
