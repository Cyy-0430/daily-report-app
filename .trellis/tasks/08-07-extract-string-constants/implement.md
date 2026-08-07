# Implement — 提取重复/魔法字符串为常量

> 纯重构，行为零变更。每阶段 `[gate]` 跑验证；既有测试全绿是回归硬证据。顺序自上而下。
> 上下文走 inline 路径（`trellis-before-dev`），本任务不强制 implement.jsonl/check.jsonl 清单。

## 阶段 1 · 后端 db.rs（配置键 / meta / 文件名）

- [ ] 1.1 db.rs 顶部加常量：9 个 `KEY_*` 配置键（api_config / prompt_template /
      custom_default_template / weekly_map_template / weekly_reduce_template /
      weekly_default_map_template / weekly_default_reduce_template / export_dir / collect_config）；
      `META_SCHEMA_VERSION` / `META_MIGRATED_FROM_STORE` / `SCHEMA_VERSION_VALUE`。
- [ ] 1.2 get_config（L86–112）与 config_pairs（L128–137）两处字面量 → 引用 `KEY_*`，
      两处共用同一组 const（消除镜像）。
- [ ] 1.3 meta 调用替换：set_meta（L52 `schema_version`/`"1"`）、get_meta（L259
      `migrated_from_store`）、迁移标记 SQL（L288）、lib.rs:28 的 `get_meta(...,"migrated_from_store")`。
- [ ] 1.4 `LEGACY_STORE_FILE`("data.json") / `STORE_KEY_CONFIG`("config") 替换 db.rs:242–243；
      `DB_FILE_NAME`("daily_report.db") 就近定义在 lib.rs、替换 lib.rs:23。
- [ ] **[gate 1]** `cargo test`（db migration/CRUD 全绿，值未变）+ `cargo check`。

## 阶段 2 · 后端 llm.rs（端点 / SSE / 占位符 / 消息 / stage / title）

- [ ] 2.1 加 `CHAT_COMPLETINGS`/`V1`；build_endpoint（L127–136）替换。
- [ ] 2.2 加 `SSE_DATA`/`SSE_DONE`；stream_chat_once（L178,181,182）替换；
      `line["data:".len()..]` → `line[SSE_DATA.len()..]`。
- [ ] 2.3 加占位符 const `TPL_DATE`/`TPL_INPUT`/`TPL_CONV`/`TPL_DATE_RANGE`/`TPL_DAY_SUMMARIES`；
      render_template / render_weekly_map / render_weekly_reduce（L40–57）替换。
- [ ] 2.4 加 `MSG_API_INCOMPLETE`，替换 L289 + L396（去重 2→1）；加其余单次消息 const
      （choices 缺失 L227、API 未配置(简) L319、连接成功 L339、连接失败 L343、无素材 L478、
      API 返回错误格式串 L167/221、请求失败格式串 L163/217/337）。
- [ ] 2.5 加 `STAGE_MAP`/`STAGE_REDUCE`，替换 L457,465,487；加 title/分隔符 const
      （`"日报"`/`"周报"`/`"~"`/`"、"`/`"（无）"`）替换 L369,498,505,531,534。
- [ ] 2.6 日期格式：`FMT_DATE`/`FMT_HM`/`FMT_DATE_HM` 在 collector/mod.rs 定义（见阶段 3.1），
      llm.rs（L297,369,455,464,531,532）引用——与阶段 3 协同（先做 3.1 再回头替换 2.6）。
- [ ] **[gate 2]** `cargo test` + `cargo check`。

## 阶段 3 · 后端 collector（日期格式 / truncate 去重 / 魔法 80 / 工具 id）

- [ ] 3.1 collector/mod.rs 加 `pub(crate) const FMT_DATE/FMT_HM/FMT_DATE_HM`；替换所有
      `"%Y-%m-%d"`/`"%H:%M"`/`"%Y-%m-%d %H:%M"`：mod.rs（parse_target_date L217、
      collect_conversations_range L357）、claude_code.rs（L181,189,191）、codex.rs（L180,183,185）、
      zcode.rs（L157,232）、opencode.rs（L161,165）。回头完成 2.6 的 llm.rs 引用。
- [ ] 3.2 mod.rs 加 `pub(crate) const MSG_HOME_NOT_FOUND`；替换 claude_code.rs:42,93 + codex.rs:59,93（去重 4→1）。
- [ ] 3.3 `truncate` 三份（claude_code.rs:298 / codex.rs:389 / zcode.rs:291）合并到 mod.rs
      `pub(super) fn truncate`；删三份，各 collector 改引用（codex.rs/zcode.rs/opencode.rs 经
      `super::` 或 mod 已 re-export）。
- [ ] 3.4 加 `pub(crate) const TOOL_KEY_MAX_LEN: usize = 80`；替换 6 处 `truncate(.., 80)`
      （claude_code.rs:279；codex.rs:253,338,386；zcode.rs:275）。
- [ ] 3.5 工具 id：mod.rs 加 `TOOL_ID_CLAUDE_CODE`/`_ZCODE`/`_CODEX`/`_OPENCODE` +
      `DEFAULT_TOOL_IDS: &[&str]`；各 collector `id()`（claude_code.rs:20 / codex.rs:37 /
      zcode.rs:31 / opencode.rs:36）引用单常量。
- [ ] 3.6 config.rs default_enabled_tools（L37–44）→ 引用 `crate::collector::DEFAULT_TOOL_IDS`
      （权衡 A，接受 config→collector 单向依赖）。
- [ ] 3.7 db.rs 测试中工具 id 字面量（L364 `vec!["claude-code".into()]`、L421–426 四件套）
      → 引用 `crate::collector::TOOL_ID_*`（权衡 B）；`sample_config` 纯数据（"gpt-demo" 等）**保留**。
- [ ] **[gate 3]** `cargo test`（含各 collector 的 `truncate_works` 仍绿）+ `cargo check`。

## 阶段 4 · 前端（工具 id 派生 + 文案）

- [ ] 4.1 bindings.ts：导出 `export const DEFAULT_TOOL_IDS = COLLECT_TOOLS.map(t => t.id)`；
      `emptyConfig()` 的 `enabledTools`（L135）→ `DEFAULT_TOOL_IDS`。
- [ ] 4.2 +page.svelte:27 与 weekly/+page.svelte:29 的回退数组 → 引用 `DEFAULT_TOOL_IDS`。
- [ ] 4.3（B 组文案）`MSG_API_NOT_CONFIGURED`（"请先在「设置」中配置 API"）就近定义
      （bindings.ts 或 store.ts）；替换 +page.svelte:81 + weekly/+page.svelte:94。
- [ ] 4.4（C 组，可选低收益）template.ts 加导出 `TPL_DATE`/`TPL_INPUT`/`TPL_CONV`/
      `TPL_DATE_RANGE`/`TPL_DAY_SUMMARIES`；settings/+page.svelte:259–298 展示文本引用之。
- [ ] **[gate 4]** `pnpm check`（svelte-check 通过）。

## 阶段 5 · 复核与收尾

- [ ] 5.1 grep 复核：db.rs 无两份配置键字面量；前端 `["claude-code"` 字面量数组仅 COLLECT_TOOLS
      一处；`truncate` 仅 collector/mod.rs 一份；`80` 不再裸露（除测试/注释）。
- [ ] 5.2 全量：`cargo test` + `cargo check` + `pnpm check` 全绿，无新增警告。
- [ ] 5.3 spec 同步：若 `.trellis/spec/backend/storage-spec.md` 列了 config key 集合，确认无需
      改动（key 值未变，仅常量化）；`collector-spec.md` 无需改（无 trait/契约变化）。
- [ ] **[gate 5]** trellis-check 通过后进入 finish-work（commit）。

## 验证命令
```bash
cargo test            # src-tauri/ 内；既有测试全绿 = 零回归
cargo check           # src-tauri/ 内
pnpm check            # 根目录；svelte-check
```

## 风险点 / 回滚锚
- **漏改 / 常量值写错**：gate 1–3 的 `cargo test` 是防线（值未变则全绿）；grep 复核（5.1）兜底。
- **config→collector 依赖（3.6）**：单向、不循环；若编译报循环（不应发生），回退为 config 内
  本地 const 数组（放弃单一来源，仍消除 config 内重复）。
- **truncate 合并（3.3）**：三份逐字相同；既有 `truncate_works` 测试覆盖。回滚锚 = gate 3。
- **纯重构无数据风险**：回滚 = git revert，无迁移、无外部副作用。

## 完成记录（2026-08-07）

全部阶段完成，纯重构零回归：
- **AC1** `cargo test`：64 passed, 0 failed（3 ignored = 需本地 db 的端到端测试）。
- **AC2** `cargo check` + `pnpm check`（339 files）均 0 error / 0 warning。
- **AC3** grep 复核：db.rs 无 `get_kv(conn, "...")` 字面量；前端 `"claude-code"` 仅余
  `bindings.ts` COLLECT_TOOLS 定义;魔法数 `, 80)` 全消除。
- **AC4** 工具 id（`TOOL_ID_*` / `DEFAULT_TOOL_IDS`）、stage（`STAGE_*`）后端单一 const。
- **AC5** `truncate` 仅 `collector/mod.rs` 一份;魔法数 80 → `TOOL_KEY_MAX_LEN`。

**与计划的偏差（2.4）**：Rust `format!` 宏要求格式串为编译期字面量，故含 `{}` 的
`"请求失败：{e}"` / `"API 返回错误 {status}：{text}"` / `"连接失败 {status}：{text}"`
无法提为 const，保留就近字面量（已在 `llm.rs` 常量块注释说明）。仅纯字面量提 const。

**spec 同步**：config key / meta 值 / 工具 id 等**值均未变**（仅常量化），`storage-spec.md` /
`collector-spec.md` 契约无变化，无需更新。
