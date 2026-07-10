# 采集器数据源与路径过滤契约 (Collector Spec)

> 后端 `src-tauri/src/collector/` 的可执行契约。覆盖 Claude Code jsonl 数据源、
> 路径过滤匹配、以及采集命令的跨层参数契约。

## Scenario: 对话采集 + 路径过滤

### 1. Scope / Trigger

触发 code-spec 深度的原因:
- 新增/变更 Tauri 命令签名(`collect_conversations` 增 `filter` 参数)。
- 跨层请求契约变更(Rust `PathFilterParam` ↔ TS `PathFilter`)。
- 绑定外部数据源格式(Claude Code jsonl),其结构非显然、靠实测确认。

任何修改采集器、过滤规则、命令参数、或读取 jsonl 字段的工作,都必须遵守本契约。

### 2. Signatures

**数据源路径**:`~/.claude/projects/<编码项目路径>/*.jsonl`(Windows 为
`C:\Users\<user>\.claude\projects\`),入口 `claude_code::home_projects_dir()`。

**路径过滤**(`collector/mod.rs`):
```rust
// 已规范化(小写、分隔符统一)的路径
pub struct PathFilter { pub includes: Vec<PathBuf>, pub excludes: Vec<PathBuf> }

// 命令层入参(原始字符串),#[serde(rename_all = "camelCase")]
pub struct PathFilterParam {
    pub include_paths: Vec<String>,  // → camelCase includePaths
    pub exclude_paths: Vec<String>,  // → camelCase excludePaths
}
impl PathFilterParam { pub fn normalize(&self) -> PathFilter }  // trim/去空串/norm
```

**采集器 trait**(`Collector::collect` 签名含 `filter`):
```rust
fn collect(&self, date: NaiveDate, filter: &PathFilter)
    -> Result<(Vec<SessionDigest>, usize), String>;
```

**Tauri 命令**:
```rust
#[tauri::command]
pub async fn collect_conversations(
    date: String,            // "YYYY-MM-DD",空=今天
    tools: Vec<String>,      // 工具 id,如 ["claude-code"]
    filter: PathFilterParam, // 路径过滤;空数组=不过滤
) -> Result<CollectResult, String>
```

**过滤纯函数**(`claude_code.rs`):
```rust
pub(super) fn norm(p: &str) -> PathBuf                 // 小写 + `/`→`\` + 去尾分隔符
fn session_allowed(cwd: Option<&Path>, includes: &[PathBuf], excludes: &[PathBuf]) -> bool
```

### 3. Contracts

#### 3a. Claude Code jsonl 数据源(硬契约)
- 每个 `*.jsonl` 一个 session,**每行一个事件 JSON**(append-only)。
- **目录名 = 编码后的项目路径**:`:` `\` `/` 全部替换为 `-`(`D:\Easy`→`D--Easy`)。
- **编码有歧义,禁止靠目录名做匹配**:中文/特殊字符变成连串 `-`,且与真实路径里的
  `-` 无法区分(本机存在 `D--Easy-------`、`D--Easy---------` 等)。**任何路径维度的
  判断都必须用 session 内真实未编码的 `cwd` 字段,不得用目录名。**
- **行顺序**:前两行常为 `mode` / `file-history-snapshot` 事件(**无 `cwd`、无 `timestamp`**);
  `cwd` 从第 2~3 行起的 `attachment` / `user` 等事件才出现,**且带 `cwd` 的事件均带 `timestamp`**。
  → 不能「读首行就跳过」;时间过滤与 cwd 提取都需遍历到带 `timestamp` 的行。
- **时间过滤(硬契约)**:按每行 `timestamp`(UTC, RFC3339)转本地时区后比 date;
  绝不按文件修改时间(session 跨天累积)。

#### 3b. 路径过滤匹配(基于真实 cwd)
- **组件级前缀匹配**:规范化后用 `Path::starts_with`,子目录继承父级规则
  (`D:\work` 命中 `D:\work\sub`)。组件级天然规避 `work` 误命中 `workplace`。
- **归一化** (`norm`):去首尾空白 → 整体小写(Windows 大小写)→ `/` 统一为 `\` → 去尾部分隔符。
- **优先级:排除优先**。
  1. 命中任一 `exclude` → 拒绝(黑名单覆盖白名单,敏感目录绝不进日报)。
  2. `include` 非空 → cwd 必须落在某条 include 下(含自身),否则拒绝。
  3. `include` 为空 → 放行(白名单空 = 不限制)。
- **cwd 为 None**:无法匹配黑名单;`include` 非空 → 拒绝(无法证实白名单),否则放行。
- **过滤点**:`ClaudeCodeCollector::collect()` 内 `parse_session` 之后、`push` 之前。
  不得侵入 `parse_session`(单一职责)。

#### 3c. 跨层参数契约
- Rust 命令入参结构体必须 `#[serde(rename_all = "camelCase")]`,与 TS interface 一一对应。
- `collect_conversations` 命令参数名(`date`/`tools`/`filter`)必须与前端 `invoke({ ... })`
  的 key 完全一致;单单词名无需大小写转换,复合字段靠 struct 的 camelCase。
- 新增 Collector:在 `all_collectors()` 注册一处,并实现 `Collector` trait(含 `filter` 参数)。

### 4. Validation & Error Matrix

| 条件 | 行为 |
|------|------|
| `~/.claude/projects` 不存在/不可读 | 命令返回 `Err("读取 Claude 目录失败: {path}: {e}")` |
| 某 jsonl 行 JSON 非法 | 计入 `skipped_lines`,跳过该行,继续 |
| 行无 `timestamp` 或解析失败 | 计入 `skipped_lines`,跳过 |
| 行 `timestamp` 落在目标 date(本地时区) | 进入解析;否则跳过(不计 skipped,正常过滤) |
| session 无任何目标日期行 | 返回 `None`(不产出 digest) |
| cwd 命中黑名单 / 不在白名单 | digest 被丢弃(不进结果,不计 skipped) |
| `include_paths`/`exclude_paths` 为空数组 | 等价于不过滤(默认/向后兼容) |
| 旧 `data.json` 无新字段 | `#[serde(default)]` 回填空数组 → 不过滤 |

### 5. Good / Base / Bad Cases

- **Good**:`include=[D:\work]`、`exclude=[D:\work\secret]` →
  `D:\work\app` 采集;`D:\work\secret` 排除(黑名单覆盖);`D:\personal` 排除(不在白名单)。
- **Base**:`include=[]`、`exclude=[D:\aaaa]` → 除 `D:\aaaa` 及其子目录外全采集。
- **Bad(反例,不得实现)**:靠编码目录名前缀匹配做过滤——
  `D:\work` 编码为 `D--work`,会误命中 `D--workplace`(`D:\workplace`),且中文段编码为连串
  `-` 无法还原。→ 必须用真实 `cwd`。

### 6. Tests Required

`claude_code.rs` 的 `#[cfg(test)]` 必须覆盖(断言点):
- `work` 不命中 `workplace`(`norm("D:\\work")` 不作为 `norm("D:\\workplace")` 的前缀)。
- 子目录继承:`D:\work\sub` 命中 include `D:\work`。
- 排除优先:同一路径在 include 与 exclude 时,exclude 获胜。
- 空规则放行全部;仅黑名单时黑名单子树被排除。
- 分隔符不变性:`D:/work` 与 `D:\work` 等价。
- 大小写不敏感:`D:\Work` 与 `D:\work` 等价。
- `cwd=None`:include 非空→拒绝;include 空→放行。
- 多条 include:命中任一即放行。
- `norm`:去空白、`/`→`\`、去尾分隔符。

### 7. Wrong vs Correct

#### Wrong — 靠编码目录名过滤
```rust
// ❌ 编码歧义:D:\work 会误匹配 D:\workplace;中文段无法还原
let encoded = user_path.replace(':', "-").replace('\\', "-").replace('/', "-");
if project_name.starts_with(&encoded) { skip_dir(); }
```

#### Correct — 基于真实 cwd 的组件级匹配
```rust
// ✅ parse 之后、push 之前,用 digest.cwd 判定
let cwd_path = digest.cwd.as_deref().map(Path::new);
if session_allowed(cwd_path, &filter.includes, &filter.excludes) {
    digests.push(digest);
}
// session_allowed 内:nc = norm(cwd);excludes.iter().any(|ex| nc.starts_with(ex)) → 拒绝
```

---

## 关联
- 任务来源:`.trellis/tasks/07-10-collect-path-filter/`(prd/design/implement)。
- 字段级内容过滤(策略①,保留 user/assistant 文本 + tool_use 摘要,丢 tool_result/thinking)
  见 `claude_code.rs::extract_line` 及其现有单测,不在本契约范围。
