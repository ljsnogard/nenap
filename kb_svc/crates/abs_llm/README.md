# abs_llm

LLM 的**语义抽象**：与 provider 无关的词汇与 trait。

```text
abs_llm::v1::cont      内容：Role / ContentPart / Capabilities / LogicOutput /
                       FinishReason / TrTextDelta / TrToolCall / LlmRespEvent / TrUsage
abs_llm::v1::interact  交互：TrMessage / TrChatRequest / TrConversation /
                       TrResponseStream / TrChatResponse / TrLlmService
abs_llm::v1::error     LlmError
```

- `no_std`；不依赖任何具体 provider、运行时或传输；
- 协议 crate（`abs_kb_svc_v1_desktop`）直接引用这里的 `Role` / `Usage` 等类型，
  不镜像第二套枚举；
- 具体的 provider 适配在 `kb_rig_llm_v1_*` 插件里（🚧，尚未接入主链路）。

## 为什么单独一层

LLM provider 的 SDK、流式分片形状、错误类型各不相同。业务协议只关心
“谁说的、说了什么、用了多少 token、为什么结束”，不关心是哪个 provider。
把这些词汇固定在本 crate，协议、插件、界面就不必跟着 provider 改。

## 当前状态

- 类型与 trait 已定义；`Cargo.toml` 的 `readme` 指向本文件；
- `kb_rig_llm_v1_agent` / `kb_rig_llm_v1_adapt` 是 WIP 适配层，
  还没有接入 `kb_core` 的 UDS/MPTP 主链路；
- `kb_svc_v1_desktop` 已用到 `Role`（`Turn.role`）等少量类型。

## 验证

```bash
cargo check -p abs_llm
```

## 相关文档

- [`kb_svc_v1_desktop/README.md`](../abs_kb_svc_v1_desktop/README.md)：协议如何复用它；
- [`kb_rig_llm_v1_agent`](../../../kb_plugins/crates/kb_rig_llm_v1_agent/)：WIP provider 适配。
