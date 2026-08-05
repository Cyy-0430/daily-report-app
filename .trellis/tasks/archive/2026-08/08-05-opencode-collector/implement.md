# Implement — opencode 采集器

## 改动清单

### 后端(Rust)
1. **`src-tauri/src/collector/opencode.rs`(新增,~250 行)**
   - `OpencodeCollector` + `Collector` trait 实现(`id="opencode"`,`display_name="Opencode"`)。
   - `collect`:全 session 采集(无 parent_id 过滤),`ORDER BY time_created`(无 sequence),
     复用 `zcode::{build_day_lines, extract_from_parts, ms_to_local}` + `claude_code::session_allowed`。
   - `db_path()`:优先 `$HOME/.local/share/opencode/opencode.db`(.exists 判定),XDG 回退。
   - 9 个单测(含 1 ignored 真实库端到端),钉 camelCase input / patch 丢弃 / 跨天切片 / db_path。

2. **`src-tauri/src/collector/zcode.rs`(扩展复用)**
   - `build_day_lines` / `extract_from_parts` / `ms_to_local` / `date_matches` → `pub(super)`。
   - `extract_from_parts` 工具 key 回退链插入 camelCase(`filePath`),注释说明两套兼容。
   - 新增 `extract_tool_part_camelcase_input` 测试钉兼容不回归。

3. **`src-tauri/src/collector/mod.rs`**:`pub mod opencode` + `pub use` + `all_collectors()` 登记。

4. **`src-tauri/src/config.rs`**:`default_enabled_tools()` 加 `"opencode"`。

5. **`src-tauri/src/db.rs`**:默认 `enabled_tools` 断言加 `"opencode"`。

### 前端(TS/Svelte)
6. **`src/lib/bindings.ts`**:`COLLECT_TOOLS` 加 opencode 项;`emptyConfig().enabledTools` 加 `"opencode"`;注释同步。

7. **`src/routes/+page.svelte`**:`enabledToolIds` fallback 默认值加 `"opencode"`。

### 文档
8. **`.trellis/spec/backend/collector-spec.md`**:新增 "Scenario: opencode (SQLite) 数据源" 章节
   (含 ZCode 差异表),更新文件头 tool 列表与关联任务。

## 验证

- `cargo test --manifest-path src-tauri/Cargo.toml --lib`:64 passed / 0 failed / 0 warning / 3 ignored。
- `cargo test collect_real_opencode_sample_day -- --ignored --nocapture`:2026-08-03 采集 2 sessions 通过。
- `pnpm build`:通过。

## 零新增依赖
复用已有的 `rusqlite`(bundled)、`dirs`、`chrono`、`serde_json`。
