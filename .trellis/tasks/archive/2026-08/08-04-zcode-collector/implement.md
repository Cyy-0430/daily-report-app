# Implement — ZCode 对话记录采集

执行顺序自上而下；每步结束跑该步的验证命令。Rollback 点标注于后。

## Step 1 — 提升复用函数可见性（claude_code.rs）
- 把 `claude_code.rs::session_allowed` 从私有改为 `pub(super)`（与已有的 `norm` 同级），签名/逻辑零改动。
- `norm` 已是 `pub(super)`，无需动。
- **验证**：`cargo check`（src-tauri 下）。
- **Rollback**：还原可见性即可。

## Step 2 — 新建 zcode.rs（核心）
- 定义 `pub struct ZCodeCollector;` + `impl Collector`：
  - `id() -> "zcode"`、`display_name() -> "ZCode"`
  - `collect(date, filter)` 按 design §4 算法：
    1. `home_dir().join(".zcode").join("cli").join("db").join("db.sqlite")`；不存在 → `Ok((vec![], 0))`
    2. `Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)`；失败 → `Ok((vec![], 0))`（静默）
    3. 查 `session WHERE parent_id IS NULL` → 逐行 `session_allowed(cwd, includes, excludes)` 过滤
    4. 每个通过 session：查其 `message`（按 sequence），按 `time_created`(ms) 转本地日期 == date 过滤
    5. 每条 message：查其 `part`（按 sequence），`extract_from_parts` 产出 (text, tools)
    6. 组 `ConversationLine` / `SessionDigest`（started/ended 取当天 message min/max ms）
- 纯函数 `extract_from_parts(parts: &[Value]) -> Option<(Role, String, Vec<String>)>`：对应 `claude_code::extract_line`；text part → `data.text`，tool part → `"{tool}: {key}"`（key 用 file_path/path/command/pattern/url/description 回退链，截断 80），reasoning/step-*/file 丢弃。
- 时间辅助：`fn ms_to_local(ms: i64) -> DateTime<Local>`（`Local.timestamp_millis_opt(ms).unwrap()`）。
- 复用 `super::{session_tokens, Collector, ConversationLine, PathFilter, Role, SessionDigest}` 与 `super::claude_code::{norm, session_allowed}`。
- **验证**：`cargo test zcode`（见 Step 5 单测）+ `cargo check`。

## Step 3 — 注册采集器（mod.rs）
- `pub mod zcode;` + `pub use zcode::ZCodeCollector;`
- `all_collectors()` 追加 `Box::new(ZCodeCollector)`。
- 更新 `collect_conversations` doc 注释里的"MVP 仅支持"措辞。
- **验证**：`cargo check`。
- **Rollback**：删两行 + 移除注册。

## Step 4 — 跨层：配置默认 + bindings + 前端多选
- **config.rs**（决策：默认启用）：`CollectConfig::default()` 的 `enabled_tools` 改为 `["claude-code","zcode"]`（确认 `#[serde(default)]` 链完整，老配置 round-trip 不破）。
- **bindings.ts**：默认 `enabledTools` 同步为 `["claude-code","zcode"]`；新增 `COLLECT_TOOLS` 常量（id + label）。
- **settings/+page.svelte**：
  - `collectEnabled: boolean` → 两个布尔（`ccEnabled` / `zcodeEnabled`），onMount 从 `enabledTools.includes(...)` 读，save 写 `enabledTools: [..若启用]`。
  - UI：单个采集开关 → 两个并列开关（Claude Code / ZCode），用 `COLLECT_TOOLS` 渲染。
- **+page.svelte**：fallback 默认值与 config 默认一致。
- **验证**：`pnpm check`。
- **Rollback**：逐文件还原。

## Step 5 — 单元测试（zcode.rs `#[cfg(test)]`）
覆盖（钉住 design §2 当前结构）：
- `extract_text_part`：`type=text` → 文本入 text。
- `extract_tool_part`：`type=tool` → `"{tool}: {key}"`，key 取 command/file_path；`reasoning` part 被丢弃。
- `extract_skips_reasoning_and_step`：纯 reasoning/step-finish → None。
- `ms_to_local_filter`：毫秒时间戳落在目标日期/非目标日期的行为（用固定 ms 构造）。
- `cross_day_session_slice`：同一 session 两条 message 跨日，只留目标日（可纯函数层模拟）。
- **验证**：`cargo test`（全绿；含既有 Claude Code 测试回归）。

## Step 6 — 集成验证（手动）
- `pnpm tauri dev`：
  1. settings 勾 Claude Code + ZCode → 生成页预览含当天两类会话；tool 字段显示 "Claude Code"/"ZCode"。
  2. 配 include/exclude（如 `D:\hand\yqnf\yqnf-contract`）→ ZCode session 按预期纳入/排除。
  3. 关掉 ZCode 开关 → 仅 Claude Code，结果与改动前一致（回归）。
  4. `sess_subagent_*` 不出现。
- **Rollback**：Step 3 移除注册即可让 ZCode 完全不参与采集（其余改动无害）。

## Step 7 — Spec 更新（finish 阶段，3.3）
- 在 `.trellis/spec/backend/collector-spec.md` 增加 ZCode 章节：数据源（SQLite 三表）、字段映射、与 Claude Code 差异矩阵、主会话过滤、只读打开、毫秒时间戳不变量。

## 验证命令汇总
```bash
# Rust（在 src-tauri/）
cargo check
cargo test                # 含 zcode 单测 + Claude Code 回归

# 前端
pnpm check                # svelte-check

# 集成
pnpm tauri dev
```

## Rollback 点
- 最早安全点：Step 3 之后——移除 `all_collectors()` 里一行即可禁用 ZCode，Claude Code 不受影响。
- 完整回退：按 Step 反序还原（4 → 3 → 2 → 1）。zcode.rs 删除即彻底移除。
