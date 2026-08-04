# 适配 ZCode 对话记录采集

## Goal

让 daily-report-app 能像采集 Claude Code 那样，采集本地 **ZCode**（智谱 GLM coding agent）的对话记录，渲染进 `{{conversations}}` 模板变量。ZCode 与 Claude Code 并列为两个可独立勾选的采集源，用户在生成页一键把当天两类会话都喂给报告生成。

## Background

ZCode 是类 Claude Code 的本地 agent，但**对话存储完全不同**：Claude Code 用 `~/.claude/projects/<enc>/*.jsonl`（append-only 事件流），ZCode 用 SQLite（`~/.zcode/cli/db/db.sqlite`），且主会话与 subagent 分表存储。本任务新增一个 `ZCodeCollector`，复用现有采集管线（`Collector` trait / `PathFilter` / `ConversationLine` / `render`），只实现"读 ZCode SQLite → 类型化投影"这一段。

## Requirements

### 功能需求
- **R1 新增采集器**：实现 `Collector` trait 的 `ZCodeCollector`（`id="zcode"`、`display_name="ZCode"`），在 `all_collectors()` 登记。
- **R2 数据源**：读取 `~/.zcode/cli/db/db.sqlite`；ZCode 未安装 / db 不存在时安静跳过（不报错、不阻断 Claude Code 采集）。
- **R3 仅主会话**：只采 `session.parent_id IS NULL` 的主会话；subagent（`sess_subagent_*`）的探索过程不进日报。
- **R4 字段过滤（策略①，与 Claude Code 对齐）**：保留 user/assistant 可见文本 + tool 调用摘要；丢弃 `reasoning`（= thinking）、`step-start`/`step-finish`/`file` 等元数据 part。
- **R5 时间过滤（硬契约）**：按 `message.time_created`（Unix 毫秒）转本地时区后比对目标日期；同一 session 跨天时，按目标日期切片成独立 digest。
- **R6 路径过滤**：基于 `session.directory`（真实 cwd）复用现有 `PathFilter`，include/exclude、排除优先、组件级前缀匹配语义与 Claude Code 完全一致。
- **R7 前端多选**：settings 页从"单 toggle（仅 Claude Code）"改为"Claude Code / ZCode 两个独立开关"；勾选状态写入 `collectConfig.enabledTools`（如 `["claude-code","zcode"]`）。
- **R8 跨层同步**：Rust 命令/结构体改动同步到 `src/lib/bindings.ts`（无 codegen，手工对齐）。

### 约束
- **C1 向后兼容**：`AppConfig` 任何新字段必须 `#[serde(default)]`；现有用户的 `load_config` 必须能 round-trip。
- **C2 不破坏既有**：Claude Code 采集行为零回归；`ConversationLine` / `SessionDigest` / `render` 的契约与渲染格式不变。
- **C3 只读访问**：以只读方式打开 ZCode 的 SQLite，绝不写入、不阻塞 ZCode 进程。
- **C4 安全边界**：API key 仍不进 JS 运行时；本任务不涉及 LLM 调用。
- **C5 复用优先**：路径规范化 / 过滤 / token 估算 / 渲染全部复用 `mod.rs` 与 `claude_code.rs` 既有函数，不复制实现。

## Acceptance Criteria

- [ ] `cargo test` 通过，且包含 ZCode 解析单元测试，至少覆盖：`text` part 提取、`tool` part 摘要、`reasoning` part 被丢弃、毫秒时间戳按目标日期过滤、跨天 session 切片。
- [ ] `pnpm check`（svelte-check）通过。
- [ ] `pnpm tauri dev` 下：settings 同时勾选 Claude Code + ZCode，生成页预览能看到当天本地 ZCode 主会话被采集并按现有格式渲染；只勾 ZCode 时也正常。
- [ ] 路径过滤对 ZCode 生效：在 settings 配 include/exclude，ZCode session 按 `session.directory` 正确纳入/排除（排除优先、组件级前缀）。
- [ ] **回归**：仅勾 Claude Code 时，采集结果与本任务前完全一致。
- [ ] **主会话-only**：`sess_subagent_*` 不出现在采集结果。
- [ ] ZCode 未安装时（`~/.zcode` 不存在），勾选 ZCode 不报错、不影响 Claude Code 采集。

## Out of Scope

- 不采集 ZCode 桌面版（`.zcode/v2/`）的数据（其结构是 tasks-index/logs，与 CLI 的 SQLite 不同）；如日后需要再单列任务。
- 不改 `{{conversations}}` 模板渲染格式（多工具已由 `render()` 统一处理，tool 字段会显示 "ZCode"）。
- 不做 ZCode db 的迁移/写入/清理。
