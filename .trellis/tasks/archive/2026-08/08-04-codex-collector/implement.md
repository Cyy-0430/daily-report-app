# Implement — Codex 对话记录采集

执行顺序按下方 checklist。每个 `[ ]` 完成后跑对应 validation；review gate 处停下等确认。

## 预检（已在本会话完成）
- [x] 探针本机 `~/.codex/sessions` 结构与 rollout jsonl 事件 schema（见 design §2，已验证）
- [x] 确认前端 settings 数据驱动、`claude_code::session_allowed` 已 `pub(super)`、默认值集中在 `config.rs::default_enabled_tools`

## Step 1 — 新建 `src-tauri/src/collector/codex.rs`

- [ ] 1.1 `CodexCollector` 结构体 + `impl Collector`（`id()="codex"`、`display_name()="Codex"`）
- [ ] 1.2 `collect(date, filter)`：定位 `~/.codex/sessions`（`home_sessions_dir()`，不存在 → `Ok((vec![],0))`）；三层 `read_dir`（Y/M/D）枚举 `rollout-*.jsonl`；逐文件 `parse_session`；组装 digest 后用 `session_allowed` 过滤、push；最后 `sort_by(started_at)`
- [ ] 1.3 `parse_session(path, date) -> (Option<SessionDigest>, usize)`：逐行解析，顶层 `timestamp`→Local→date 过滤（硬契约）；cwd 仅取自 `session_meta.payload.cwd`；调 `extract_line` 取行；空 lines 返回 None
- [ ] 1.4 `extract_line(ev: &Value) -> Option<ConversationLine>`（策略①纯函数）：
  - `event_msg` + `payload.type=="user_message"` → User，text=`payload.message`
  - `event_msg` + `payload.type=="agent_message"` → Assistant，text=`payload.message`
  - `response_item` + `payload.type=="function_call"` → Assistant 工具行，name=`payload.name`，key 从 `payload.arguments`(JSON str) 回退链取
  - `response_item` + `payload.type=="local_shell_call"` → Assistant 工具行，name="shell"，key=`payload.action.command`
  - 其余 → None；text/tools 皆空 → None
- [ ] 1.5 辅助：`truncate`（复刻）、`project_name(cwd, session_id)`（basename 回退 sid 前 8）
- [ ] 1.6 `#[cfg(test)]` 单测（纯函数，合成 + 真实派生 fixture）：
  - user_message → User 文本；agent_message → Assistant 文本；空白 message → None
  - developer role 的 response_item/message → None（噪声丢弃）
  - function_call → `"{name}: {key}"`；local_shell_call → `"shell: <cmd>"`（合成，标注未实测）
  - 跨天切片：同 session 两行分属不同日，只留目标日；目标日无行 → None
  - `#[ignore]` 端到端 `collect_real_codex_sample_day`：对 `~/.codex/sessions` 采集 `2026-07-31`（已知有"hi"会话），断言非空

**Validation：** `cargo test --manifest-path src-tauri/Cargo.toml codex`（库测试全绿）

## Step 2 — 注册到 `mod.rs`

- [ ] 2.1 `pub mod codex;` + `pub use codex::CodexCollector;`
- [ ] 2.2 `all_collectors()` 追加 `Box::new(CodexCollector)`
- [ ] 2.3 更新 `collect_conversations` 文档注释里的工具列表（`claude-code`/`zcode`/`codex`）

**Validation：** `cargo check --manifest-path src-tauri/Cargo.toml`

## Step 3 — 跨层默认值

- [ ] 3.1 `config.rs::default_enabled_tools()` 加 `"codex"`；更新结构体字段文档注释
- [ ] 3.2 `bindings.ts`：`COLLECT_TOOLS` 加 `{ id:"codex", label:"Codex", hint:"~/.codex/sessions" }`；`emptyConfig()` 默认 `enabledTools` 加 `"codex"`；更新 `enabledTools` 字段注释
- [ ] 3.3 `src/routes/+page.svelte` fallback 默认值加 `"codex"`

**Validation：** `pnpm check`

## Review Gate（Step 4 前）
- 跑 `cargo test`（全量）+ `pnpm check` 全绿
- `cargo run` 或 `pnpm tauri dev` 手测：勾选 codex 采集 `2026-07-31`，确认"hi"会话被采集、渲染含 user/agent 文本
- 确认 design §9 决策（默认启用、文本源、project 名、工具分支）与实现一致

## Step 4 — 收尾（Phase 3）
- [ ] 4.1 spec 更新：在 `collector-spec.md` 追加 "Scenario: Codex (rollout jsonl) 数据源"（数据源/字段映射/差异矩阵/错误矩阵/Tests Required），并更新文档头部覆盖说明
- [ ] 4.2 `cargo test` + `pnpm check` 最终全绿
- [ ] 4.3 提交（commit message遵循仓库风格；不 push 除非用户要求）

## 回滚点
- Step 1/2 出错：删除 `codex.rs` + 撤销 `mod.rs` 三行即可，无副作用。
- Step 3 默认值若引担忧：把 codex 移出默认 `enabledTools`（仍可手动勾选），不影响已实现逻辑。
