# 采集工具新增 opencode 适配

## Goal

让日报采集器支持读取本机 **opencode**(sst/opencode)的对话记录,与已有的 Claude Code、
ZCode、Codex 并列,作为第四个可勾选的采集工具。采集仍是纯本地、无 LLM、无 token 的操作,
结果渲染进模板变量 `{{conversations}}`。

## Background / 数据源(已在本机探针确认)

opencode 把对话存 SQLite,与 ZCode **几乎完全同构**(`session` + `message` + `part` 三表):

- 路径:`~/.local/share/opencode/opencode.db`(实测 Windows 也落在此处,**不走** `%LOCALAPPDATA%`,
  opencode 跨平台统一用 Unix 风格 `$HOME/.local/share`)。
- `session`:`id` / `directory`(真实 cwd) / `title` / `time_created`(Unix 毫秒)。**无 `parent_id`。**
- `message`:`session_id` / `time_created`(Unix 毫秒) / `data`(`{role,...}`)。**无 `sequence`。**
- `part`:`message_id` / `data`(`{type:text|tool|reasoning|step-start|step-finish|patch,...}`)。**无 `sequence`。**
- 干净文本在 `part.type=text`;工具调用在 `part.type=tool`,其 `state.input` 用 **camelCase**
  (`filePath`/`command`/`pattern`),与 ZCode 的 snake_case 不同。

## Requirements

- **R1** 新增 `src-tauri/src/collector/opencode.rs`,定义 `OpencodeCollector`,实现 `Collector` trait
  (`id="opencode"`,`display_name="Opencode"`)。
- **R2** 数据源定位 `~/.local/share/opencode/opencode.db`;不存在 / 打开失败 → `Ok((vec![], 0))`
  **静默跳过**,不阻断其它采集器(与 ZCode 一致)。
- **R3** **全 session 采集**:opencode 无 `parent_id`(无 subagent 噪声),所有 session 都是用户直接交互。
- **R4 排序键 = `time_created`**(硬差异):opencode 的 message/part 表无 `sequence` 列(与 ZCode 不同),
  `ORDER BY time_created` 升序(实测同 message 内单调递增)。
- **R5 时间过滤(硬契约)**:按 `message.time_created`(Unix 毫秒)转本地时区→比 date;非目标日期
  的 message 跳过(不计 skipped)。同 session 跨天按目标日切片,`started_at/ended_at` 取当日首/末 ms。
- **R6 字段过滤(策略①)**:复用 `zcode::extract_from_parts`——`text`→文本;`tool`→`"{name}: {key}"`;
  `reasoning`/`step-*`/`file`/`patch` 一律丢弃。工具参数 key 回退链需兼容 **camelCase**(`filePath`),
  为此扩展 zcode 的 `extract_from_parts`(两套字段名并存,ZCode 数据只命中 snake_case,不受影响)。
- **R7 路径过滤**:cwd 取自 `session.directory`(真实路径),用既有 `session_allowed` 做组件级前缀匹配、
  排除优先(同 ZCode)。
- **R8 注册**:`mod.rs` 加 `pub mod opencode` + `pub use opencode::OpencodeCollector` + `all_collectors()` 登记。
- **R9 跨层默认值**:`config.rs` 的 `default_enabled_tools()` 加 `"opencode"`;`bindings.ts` 的 `COLLECT_TOOLS`
  加 opencode 项(`hint="~/.local/share/opencode"`)、默认 `enabledTools` 加入 `"opencode"`;
  `+page.svelte` fallback 默认值同步;`db.rs` 默认断言同步。
- **R10 纯函数复用**:复用 `zcode::{build_day_lines, extract_from_parts, ms_to_local}`,提升为 `pub(super)`,
  避免代码重复。

## Acceptance Criteria

- [x] `cargo test`(含新增 opencode 单测)全绿;`pnpm build` 通过。
- [x] 勾选 opencode 后,对存在 `~/.local/share/opencode/opencode.db` 的本机能采集到目标日期的会话
  (2026-08-03 实测:2 个会话,7+16 行,cwd/project/tools 均正确)。
- [x] 不勾选 opencode / 本机未装时,采集不报错、不影响其它采集器。
- [x] camelCase 工具参数(`filePath`/`command`)正确提取为 `"{tool}: {key}"`。
- [x] `patch` part 被丢弃,不污染正文。
- [x] 默认 `enabledTools` 在 Rust / TS / fallback / db 断言四处一致。

## Notes

- 本任务与 `08-04-zcode-collector` / `08-04-codex-collector` 同构,design 大量参照其结构。
- opencode 与 ZCode schema 同构是最大复用点:复用 `build_day_lines`/`extract_from_parts`/`ms_to_local`。
- 采集器 spec:`.trellis/spec/backend/collector-spec.md`(已追加 opencode 章节)。
