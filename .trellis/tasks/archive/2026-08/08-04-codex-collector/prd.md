# 采集工具新增 Codex 适配

## Goal

让日报采集器支持读取本机 **OpenAI Codex CLI** 的对话记录，与已有的 Claude Code、ZCode 并列，作为第三个可勾选的采集工具。采集仍是纯本地、无 LLM、无 token 的操作，结果渲染进模板变量 `{{conversations}}`。

## Background / 数据源（已在本机探针确认）

Codex CLI 把每个会话存为 append-only 的 rollout jsonl：

- 路径：`~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<时间戳>-<uuid>.jsonl`，**每个文件一个会话**。
- 每行一个事件 JSON，顶层字段：`timestamp`（RFC3339 UTC）、`type`、`payload`。
- 干净的用户可见对话在 `event_msg` 事件里：
  - `payload.type == "user_message"` → `payload.message`（用户真实输入）
  - `payload.type == "agent_message"` → `payload.message`（助手最终回复，`phase=final_answer`）
- 会话真实 cwd 在首行 `session_meta.payload.cwd`（无目录名编码歧义，与 ZCode 的 `directory` 同性质）。
- 另有 `response_item`/`message`（API 层原始消息，含大量注入的权限/AGENTS.md 噪声，**不用作文本源**）；本机所有会话均无 `function_call`（用户用 Codex 以纯问答为主）。

## Requirements

- **R1** 新增 `src-tauri/src/collector/codex.rs`，定义 `CodexCollector`，实现 `Collector` trait（`id="codex"`，`display_name="Codex"`）。
- **R2** 数据源定位 `~/.codex/sessions`；目录或文件不存在 / 读取失败 → 返回 `Ok((vec![], 0))` **静默跳过**，不阻断其它采集器（与 ZCode 一致）。
- **R3** 遍历 `sessions/<Y>/<M>/<D>/` 下的 `rollout-*.jsonl`，按**文件路径里的日期段不做过滤**——时间过滤一律按行 `timestamp`（硬契约，session 跨天累积）。
- **R4 文本源（策略①）**：仅取 `event_msg` 的 `user_message`（→ User）与 `agent_message`（→ Assistant）的 `payload.message`；`message` 为空/纯空白则跳过该行。不消费 `response_item`/`message`（避免系统噪声与重复）。
- **R5 时间过滤（硬契约）**：按每行顶层 `timestamp`（UTC RFC3339）转本地时区后比 date；非目标日期的行跳过（不计 skipped）。同一 session 跨天时按目标日切片，`started_at/ended_at` 取当天首/末行时间。
- **R6 路径过滤**：cwd 取自 `session_meta.payload.cwd`（真实路径），用既有 `session_allowed` 做组件级前缀匹配、排除优先。过滤在组装出 digest 后、push 前判定（同 Claude Code）。
- **R7 工具调用（防御性，可选）**：若出现 `response_item`/`function_call`（`payload.name`+`payload.arguments`）或 `local_shell_call`（`payload.action.command`），渲染为 `"{name}: {key}"` 的 Assistant 工具行；**本机无此类样本，属未实测路径**，用合成 fixture 单测钉住结构，并在 design.md 标注验证状态。无则该分支自然不触发。
- **R8 注册**：`mod.rs` 加 `pub mod codex` + `pub use codex::CodexCollector` + `all_collectors()` 登记一处。
- **R9 跨层默认值**：`bindings.ts` 的 `COLLECT_TOOLS` 增加 codex 项（`hint="~/.codex/sessions"`），默认 `enabledTools` 加入 `"codex"`；`+page.svelte` 的 fallback 默认值同步。settings 页已按 `COLLECT_TOOLS` 数据驱动渲染，无需改。
- **R10 跨层契约**：`collect_conversations` 命令签名/参数不变；`enabledTools` 仍是 `Vec<String>`/`string[]`，旧配置无新字段，靠 `#[serde(default)]` 回填（codex 默认是否启用见决策）。

## Acceptance Criteria

- [ ] `cargo test`（含新增 codex 单测）全绿；`pnpm check` 通过。
- [ ] 勾选 codex 后，对存在 `~/.codex/sessions` 的本机能采集到目标日期的主会话，渲染文本含 user/agent 可见对话。
- [ ] 不勾选 codex / 本机未装 Codex 时，采集不报错、不影响 Claude Code / ZCode 结果。
- [ ] 时间过滤按行 `timestamp`（非文件名日期、非 mtime）；跨天 session 只出现目标日切片。
- [ ] 路径过滤基于 `session_meta.cwd` 真实路径，排除优先、组件级前缀，行为与 Claude Code/ZCode 一致。
- [ ] 默认 `enabledTools` 与 `COLLECT_TOOLS`、`+page.svelte` fallback 三处一致（默认是否启用 codex 见 design 决策）。

## Notes

- 本任务与 `08-04-zcode-collector` 同构，design.md 大量参照其结构。
- 采集器 spec：`.trellis/spec/backend/collector-spec.md`（修改采集器必须遵守）。
