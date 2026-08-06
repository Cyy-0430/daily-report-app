# Implement — 采集工具路径可编辑与恢复默认

> 执行顺序自上而下。每个 [ ] 是一次可验证的小步。验证命令见末节。

## A. 后端：配置层

- [ ] **A1** `src-tauri/src/config.rs`
  - `use std::collections::HashMap;`
  - `CollectConfig` 加字段 `#[serde(default)] pub tool_paths: HashMap<String, String>`（注释：键=工具 id，值=路径，空=默认，支持 `~`）。
  - `Default` impl 加 `tool_paths: HashMap::new()`。
  - 自查：字段有 `#[serde(default)]`（AC6 向后兼容）。

## B. 后端：采集器 trait + 共享解析

- [ ] **B1** `src-tauri/src/collector/mod.rs`
  - `use std::path::PathBuf;` 已有；确认 `use crate::collector::...` 现有导入。
  - 新增 `pub(super) fn expand_home(p: &str) -> Option<PathBuf>`（见 design §2.1）。
  - `Collector` trait：加 `fn default_path(&self) -> Option<PathBuf>;`；`collect` 签名加尾参 `custom_path: Option<&str>`。
- [ ] **B2** `src-tauri/src/collector/claude_code.rs`
  - 加 `impl default_path`：`home_projects_dir().ok()`。
  - `collect` 内：`let path = custom_path.and_then(crate::collector::expand_home).or_else(|| self.default_path());`；`None` → `Err("无法定位用户主目录".into())`；`path` 替换原 `home_projects_dir()?`；其余（`read_dir` / `parse_session` / 过滤）不变。
- [ ] **B3** `src-tauri/src/collector/codex.rs`
  - `default_path`：`home_sessions_dir().ok()`。
  - `collect` 内同样解析 `path`；`None` 或 `collect_jsonl_files` 后无文件 → `Ok((vec![], 0))`（保持静默）。
- [ ] **B4** `src-tauri/src/collector/zcode.rs`
  - `default_path`：`db_path()`（直接返回原 `Option`）。
  - `collect` 内解析 `path`；`None` → `Ok((vec![], 0))`；db 打开失败 → 静默（保持现有）。
- [ ] **B5** `src-tauri/src/collector/opencode.rs`
  - `default_path`：`db_path()`。
  - `collect` 内解析 `path`；`None` 或 db 不存在/打开失败 → 静默。
- [ ] **B6** `src-tauri/src/collector/mod.rs` 同模块内被 `pub(super)` 引用：确认 `expand_home` 可被四个子模块以 `super::expand_home` 或 `crate::collector::expand_home` 调用（放 `mod.rs`，子模块用 `super::expand_home`）。
- [ ] **B7** 三个 ignored 端到端测试（zcode / opencode / codex 的 `collect_real_*_sample_day`）补 `, None`：`.collect(date, &PathFilter::default(), None)`。claude_code 若有同类测试同样处理。

## C. 后端：命令层

- [ ] **C1** `src-tauri/src/collector/mod.rs`
  - `collect_blocking` 加参 `tool_paths: &HashMap<String, String>`；循环内取 `let custom = tool_paths.get(c.id()).map(|s| s.as_str()).filter(|s| !s.trim().is_empty());` 并传 `c.collect(target, filter, custom)?`。
  - `collect_conversations` 加参 `tool_paths: HashMap<String, String>`，透传给 `collect_blocking`。
  - 新增 `#[tauri::command] pub fn default_collect_paths() -> HashMap<String, String>`（design §4）。
- [ ] **C2** `src-tauri/src/lib.rs`
  - `invoke_handler!` 注册 `collector::default_collect_paths`。

## D. 后端：单测

- [ ] **D1** `src-tauri/src/collector/mod.rs` `#[cfg(test)]` 加 `expand_home` 测试（design §6 4 组断言）。
- [ ] **D2** `cargo test`（src-tauri 内）全绿。

## E. 前端：绑定

- [ ] **E1** `src/lib/bindings.ts`
  - `CollectConfig` 加 `toolPaths: Record<string, string>`。
  - `emptyConfig().collectConfig` 加 `toolPaths: {}`。
  - 新增 `export const defaultCollectPaths = () => invoke<Record<string,string>>("default_collect_paths");`。
  - `collectConversations` 加尾参 `toolPaths: Record<string,string>`，invoke 加 `toolPaths`。
  - `COLLECT_TOOLS` 每项加 `kind: "dir" | "file"`（claude-code/codex=`dir`，zcode/opencode=`file`）。

## F. 前端：设置页

- [ ] **F1** `src/routes/settings/+page.svelte`
  - `onMount`：先 `defaultPaths = await defaultCollectPaths();`，再 `toolPaths = Object.fromEntries(COLLECT_TOOLS.map(t => [t.id, (c.collectConfig?.toolPaths?.[t.id] ?? "").trim() || defaultPaths[t.id] || ""]))`。
  - import `defaultCollectPaths`。
  - 新增 `storedPath(id)` 规整函数。
  - `save()` 的 `collectConfig` 加 `toolPaths: Object.fromEntries(COLLECT_TOOLS.map(t => [t.id, storedPath(t.id)]))`。
  - 模板：`{#each COLLECT_TOOLS}` 内勾选框后加 `.path-row`（输入框 `bind:value={toolPaths[t.id]}` + 「恢复默认」按钮，`disabled={toolPaths[t.id] === (defaultPaths[t.id] || "")}`）。
  - placeholder 按 `t.kind` 区分「数据库文件路径」/「数据目录路径」。

## G. 前端：生成页采集调用点

- [ ] **G1** `src/routes/+page.svelte`
  - `collectConversations(collectDate, tools, filter)` → `collectConversations(collectDate, tools, filter, cfg.toolPaths ?? {})`（确认 `cfg` 在该作用域，line ~62/72）。

## H. 验证（Review Gates）

- [ ] **H1** `pnpm check`（svelte-check）无类型错误（含 bindings↔Rust 字段对齐）。
- [ ] **H2** `cargo test`（src-tauri）全绿。
- [ ] **H3** `cargo check`（src-tauri）快检通过。
- [ ] **H4** 手测（dev）：设置页四工具显示真实默认路径；改一个 → 保存 → 重启 → 仍在；「恢复默认」可用；采集走自定义路径（指向有数据的目录/db）能取到会话。（可选，依赖本机数据）

## I. 收尾

- [ ] **I1** 同步 `collector-spec.md`：`collect` 新签名、`default_path` trait 方法、`default_collect_paths` 命令、`CollectConfig.toolPaths`、`expand_home` / `~` 展开契约、跨层 `toolPaths` 键。
- [ ] **I2** `python ./.trellis/scripts/task.py validate` 通过。
- [ ] **I3** 提交（用户确认后）。

## 验证命令

```bash
pnpm check
# Rust 部分:
cd src-tauri && cargo test && cargo check
# 或在仓库根:
# cargo test --manifest-path src-tauri/Cargo.toml
```

## 回滚点

- 每完成一个大写段（A–G）即可编译/类型检查一次；出问题回退该段。
- 无 DB migration、无不可逆操作；整体可 `git revert`。
