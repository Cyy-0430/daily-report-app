# Design — 采集工具路径可编辑与恢复默认

> 技术设计。仅改「数据源定位」，不触碰采集器的过滤 / 解析 / 时间切片契约。

## 1. 数据模型

### 1.1 配置层（`config.rs`）

`CollectConfig` 新增一个 `tool_paths` 字段，键 = 工具 id，值 = 自定义路径串。**空串 / 缺键 = 用默认**。

```rust
use std::collections::HashMap;

pub struct CollectConfig {
    #[serde(default = "default_enabled_tools")]
    pub enabled_tools: Vec<String>,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    /// 各采集工具的自定义数据源路径(覆盖默认)。键=工具 id,值=路径串;
    /// 空串或缺失 = 用该工具默认路径。支持 `~` 展开。
    #[serde(default)]
    pub tool_paths: HashMap<String, String>,
}
```

- `#[serde(default)]` → 老配置反序列化时回填空 map，向后兼容（满足 AC6/R7）。
- `Default` impl 里加 `tool_paths: HashMap::new()`。
- 序列化为 JSON object，TS 侧 `Record<string, string>`。
- 既有「所有字段 `#[serde(default)]`」约定保持。

### 1.2 前端模型（`bindings.ts`）

```ts
export interface CollectConfig {
  enabledTools: string[];
  includePaths: string[];
  excludePaths: string[];
  toolPaths: Record<string, string>; // 新增
}
```

`emptyConfig()` 同步加 `toolPaths: {}`。

## 2. 采集器 trait 变更（`collector/mod.rs`）

两个改动：暴露默认路径、`collect` 接收覆盖路径。

```rust
pub trait Collector: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    /// 该工具数据源的默认路径(已展开 ~)。None = 无法定位(如取不到主目录)。
    fn default_path(&self) -> Option<PathBuf>;
    /// 采集指定本地日期的对话。custom_path 非空 → 用它(展开 ~),否则用默认。
    fn collect(
        &self,
        date: NaiveDate,
        filter: &PathFilter,
        custom_path: Option<&str>,
    ) -> Result<(Vec<SessionDigest>, usize), String>;
}
```

**为什么改 `collect` 签名而不带状态构造采集器**：四个采集器现均为 unit struct，且 `collect_real_*` 端到端测试直接 `Collector.collect(date, &PathFilter::default())` 调用。改签名只需在 3 处 ignored 测试补 `, None`；改为带状态 struct 要重写 4 个 struct + 所有引用。前者更小、更显式。

### 2.1 路径解析约定（新增共享纯函数）

`collector/mod.rs` 新增 `pub(super)` 工具函数（带单测，见 §6）：

```rust
/// 展开 `~` / `~/x` / `~\x` 为真实主目录;空串 → None;其余原样返回。
pub(super) fn expand_home(p: &str) -> Option<PathBuf> {
    let p = p.trim();
    if p.is_empty() { return None; }
    let home = || dirs::home_dir();
    match p {
        "~" => home(),
        s if s.starts_with("~/") => home().map(|h| h.join(&s[2..])),
        s if s.starts_with("~\\") => home().map(|h| h.join(&s[2..])),
        s => Some(PathBuf::from(s)),
    }
}
```

每个采集器在 `collect` 内统一解析：

```rust
let path = custom_path
    .and_then(expand_home)        // Some(非空串)→ Some(展开后);空/None → None
    .or_else(|| self.default_path());
```

随后各采集器按自身既有语义处理 `path`：
- claude-code：`None` → `Err("无法定位用户主目录")`（保持现有硬错）；目录不可读 → `Err`。
- codex / zcode / opencode：`None` 或路径不存在 → `Ok((vec![], 0))` 静默跳过（保持现有语义）。

> **契约不变**：「路径不存在时谁报错谁跳过」与今天完全一致（claude-code 报错上抛，其余静默）。本任务不调整该语义。

### 2.2 各采集器的 `default_path()` 实现

把现有路径解析函数直接接上：

| 工具 | 现有入口 fn | `default_path()` 返回 |
|---|---|---|
| claude-code | `home_projects_dir()` `Result<PathBuf,String>` | `.ok()` → `~/.claude/projects` |
| codex | `home_sessions_dir()` `Result<PathBuf,String>` | `.ok()` → `~/.codex/sessions` |
| zcode | `db_path()` `Option<PathBuf>` | 直接 → `~/.zcode/cli/db/db.sqlite` |
| opencode | `db_path()` `Option<PathBuf>` | 直接 → 主路径/XDG 回退（既有逻辑） |

> claude-code / codex 的 `Result→Option` 转换只丢「无法定位主目录」这种环境错误信息；`default_path()` 仅用于「展示默认 + 回退」，不参与错误上抛（上抛仍由 `collect` 内的 `None` 分支处理）。

## 3. 采集命令变更（`collector/mod.rs`）

### 3.1 `collect_blocking` 接收路径 map

```rust
fn collect_blocking(
    date: &str,
    tools: &[String],
    filter: &PathFilter,
    tool_paths: &HashMap<String, String>,
) -> Result<CollectResult, String> {
    ...
    for c in all_collectors() {
        if !tools.iter().any(|t| t == c.id()) { continue; }
        // 该工具的覆盖路径:取键、trim、非空才传 Some。
        let custom = tool_paths.get(c.id()).map(|s| s.as_str()).filter(|s| !s.trim().is_empty());
        let (sessions, skipped) = c.collect(target, filter, custom)?;
        ...
    }
}
```

### 3.2 `collect_conversations` 新增 `tool_paths` 入参

```rust
#[tauri::command]
pub async fn collect_conversations(
    date: String,
    tools: Vec<String>,
    filter: PathFilterParam,
    tool_paths: HashMap<String, String>,   // 新增;JS key = toolPaths(Tauri 自动 camelCase)
) -> Result<CollectResult, String> {
    let filter = filter.normalize();
    tokio::task::spawn_blocking(move || collect_blocking(&date, &tools, &filter, &tool_paths))
        .await.map_err(|e| format!("采集任务异常: {e}"))?
}
```

> 与现有「`tools` / `filter` 由前端显式传入（而非后端 load_config）」设计一致——`toolPaths` 同样由前端从已加载配置显式传入，保持命令纯函数可测。

## 4. 新增「查默认路径」命令（`collector/mod.rs` + `lib.rs`）

供设置页展示初始值 + 「恢复默认」按钮取值（满足 R3：默认值由后端权威给出）。

```rust
#[tauri::command]
pub fn default_collect_paths() -> HashMap<String, String> {
    all_collectors()
        .into_iter()
        .filter_map(|c| c.default_path().map(|p| (c.id().to_string(), p.to_string_lossy().into_owned())))
        .collect()
}
```

`src-tauri/src/lib.rs::run` 的 `invoke_handler` 注册 `collector::default_collect_paths`。

## 5. 前端

### 5.1 `bindings.ts`

```ts
export const defaultCollectPaths = () =>
  invoke<Record<string, string>>("default_collect_paths");

export const collectConversations = (
  date: string,
  tools: string[],
  filter: PathFilter,
  toolPaths: Record<string, string>,   // 新增
) => invoke<CollectResult>("collect_conversations", { date, tools, filter, toolPaths });
```

`COLLECT_TOOLS` 增加 `kind: "dir" | "file"`（仅展示用：输入框 placeholder / label 区分「数据目录」vs「数据库文件」），与 `all_collectors()` 的 id 仍一一对应。

### 5.2 `settings/+page.svelte`

状态：
```ts
let defaultPaths = $state<Record<string, string>>({});   // 后端权威默认
let toolPaths = $state<Record<string, string>>({});      // 当前生效值(初始 = 覆盖 ?? 默认)
```

`onMount`：
```ts
defaultPaths = await defaultCollectPaths();
const saved = c.collectConfig?.toolPaths ?? {};
toolPaths = Object.fromEntries(COLLECT_TOOLS.map((t) => [
  t.id, saved[t.id]?.trim() || defaultPaths[t.id] || "",
]));
```

UI（每个工具一行，置于勾选框下方）：
```svelte
{#each COLLECT_TOOLS as t (t.id)}
  <label class="fld fld-check">...勾选...</label>
  <div class="path-row">
    <input class="field" bind:value={toolPaths[t.id]}
           placeholder={t.kind === "file" ? "数据库文件路径" : "数据目录路径"} />
    <button class="btn btn-ghost btn-sm"
            onclick={() => (toolPaths[t.id] = defaultPaths[t.id] || "")}
            disabled={toolPaths[t.id] === (defaultPaths[t.id] || "")}>
      恢复默认
    </button>
  </div>
{/each}
```
- 输入框直接 `bind:value` 到 `toolPaths[t.id]`（始终有真实路径值，从不空）。
- 「恢复默认」按钮在「当前值 === 默认值」时禁用（视觉提示无需恢复）。

`save()`：把 `toolPaths` 规整后写入 `collectConfig.toolPaths`——**等于默认或空的存 `""`**（=用默认），其余存 trim 后的值：
```ts
function storedPath(id: string): string {
  const v = (toolPaths[id] ?? "").trim();
  const d = (defaultPaths[id] ?? "").trim();
  return v && v !== d ? v : "";
}
// collectConfig.toolPaths = Object.fromEntries(COLLECT_TOOLS.map(t => [t.id, storedPath(t.id)]))
```

### 5.3 `+page.svelte`（生成页采集调用点）

`collectConversations(collectDate, tools, filter)` → 补传 `cfg.toolPaths ?? {}`。

## 6. 测试

Rust（`collector/mod.rs` 的 `#[cfg(test)]`）新增 `expand_home` 单测：
- `"~"` → `home_dir()`；`"~/x"` → `home.join("x")`；`"~\\y"`（Windows）→ `home.join("y")`。
- `""` / `"   "` → `None`。
- 绝对路径 / 无前缀 → 原样 `Some`。

`default_collect_paths` 行为可由现有 `collect_real_*`（ignored）间接覆盖；不新增依赖本机数据的测试。

## 7. 兼容性 / 回滚

- 向后兼容：`tool_paths` 全 `#[serde(default)]`；前端 `?? {}` 兜底。
- 回滚：纯新增字段 + 一处签名变更（`collect` 加尾参），`git revert` 单提交即可；无 DB schema 变更（`config` 表存 JSON 字符串）。

## 8. 风险

- **R-1**：claude-code 指向错误路径会硬错并中断整次采集（含其它工具）——这是既有契约，不在本任务调整范围；UI 上输入框失焦/保存时不做存在性校验（避免跨工具连锁失败时误判）。
- **R-2**：opencode 默认有主/XDG 双路径回退；`default_path()` 仍走原 `db_path()` 逻辑，自定义路径则直连（用户自负），二者语义一致（都指向 db 文件）。
