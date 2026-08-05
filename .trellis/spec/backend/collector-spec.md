# 采集器数据源与路径过滤契约 (Collector Spec)

> 后端 `src-tauri/src/collector/` 的可执行契约。覆盖 Claude Code / Codex jsonl 数据源、
> ZCode / opencode SQLite 数据源、路径过滤匹配、以及采集命令的跨层参数契约。

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

**采集器 trait**(`Collector::collect` 签名含 `filter` + `custom_path`;另有 `default_path`):
```rust
fn default_path(&self) -> Option<PathBuf>;   // 该工具默认数据源(已展开 ~)
fn collect(&self, date: NaiveDate, filter: &PathFilter, custom_path: Option<&str>)
    -> Result<(Vec<SessionDigest>, usize), String>;
```

**Tauri 命令**:
```rust
#[tauri::command]
pub async fn collect_conversations(
    date: String,             // "YYYY-MM-DD",空=今天
    tools: Vec<String>,       // 工具 id,如 ["claude-code"]
    filter: PathFilterParam,  // 路径过滤;空数组=不过滤
    tool_paths: HashMap<String, String>, // 各工具自定义数据源路径;键缺失/空串=用默认
) -> Result<CollectResult, String>

#[tauri::command]
pub fn default_collect_paths() -> HashMap<String, String> // 各工具默认数据源(已展开 ~)
```

**过滤纯函数**(`claude_code.rs`):
```rust
pub(super) fn norm(p: &str) -> PathBuf                 // 小写 + `/`→`\` + 去尾分隔符
pub(super) fn session_allowed(cwd: Option<&Path>, includes: &[PathBuf], excludes: &[PathBuf]) -> bool  // 复用:zcode 共享
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

## Scenario: ZCode (SQLite) 数据源

> ZCode(智谱 GLM coding agent)的对话存 SQLite,与 Claude Code 的 jsonl 完全不同。
> 本场景覆盖其数据源结构、主会话过滤、毫秒时间戳、只读访问与字段映射。

### 1. Scope / Trigger
修改 `zcode.rs`、新增基于 SQLite 的采集器、或读取 ZCode `db.sqlite` 字段时遵守本契约。

### 2. Signatures
**数据源路径**:`~/.zcode/cli/db/db.sqlite`,入口 `zcode::db_path()`。

**只读打开**:
```rust
Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
```

**三张核心表**(已实测):
- `session`:`id` / `parent_id`(NULL=主会话) / `directory`(真实 cwd) / `title` / `time_created`(Unix 毫秒)。
- `message`:`session_id` / `time_created`(Unix 毫秒) / `data`(`{role:"user"|"assistant",...}`) / `sequence`。
- `part`:`message_id` / `data`(`{type:"text"|"tool"|"reasoning"|"step-start"|"step-finish"|"file",...}`) / `sequence`。

### 3. Contracts

#### 3d. ZCode SQLite 数据源(硬契约)
- **仅主会话**:`session.parent_id IS NULL`;`sess_subagent_*` 是被主会话 spawn 的探索子
  agent,**不采集**(噪音大,且非用户直接交互)。
- **真实 cwd 来自 `session.directory`**:本身是真实路径(无 Claude Code 的目录名编码歧义),
  直接喂给 `session_allowed`,沿用 §3b 的组件级前缀 / 排除优先语义。
- **时间过滤(硬契约)**:按 `message.time_created`(**Unix 毫秒**)`/1000`→本地时区→比 date;
  绝不按文件修改时间或 session 的 time_created(session 跨天累积,同 session 按目标日切片)。
- **字段过滤(策略①)**:`part.type=text` → 文本(`data.text`);`part.type=tool` →
  `"{data.tool}: {key}"`(key 从 `data.state.input` 取 file_path/path/command/pattern/url/description 回退,截断 80);
  `reasoning`/`step-*`/`file` 一律丢弃(等同 Claude Code 丢 thinking/tool_result)。
- **只读访问**:`READ_ONLY | NO_MUTEX` 打开,不写入、不阻塞 ZCode 进程(WAL 下只读连接不阻塞写者)。
- **project 显示名**:`session.title`,空回退 `directory` basename。
- **role**:`message.data.role`(`"assistant"`→Assistant,其余→User)。

#### 与 Claude Code 的关键差异(实现/维护时对照)
| 维度 | Claude Code | ZCode |
|---|---|---|
| 存储 | jsonl 文件(append-only) | SQLite(session/message/part) |
| 路径 | `~/.claude/projects/<enc>/*.jsonl` | `~/.zcode/cli/db/db.sqlite` |
| 对话单元 | 每行 `{type:user\|assistant,message.content}` | message(role) + part(type) 两表关联 |
| 时间 | RFC3339 字符串 | **Unix 毫秒** |
| cwd 来源 | 每行 `cwd` 字段 | `session.directory` |
| 主/sub 区分 | 无(目录即 session) | `parent_id IS NULL` |
| thinking 丢弃 | `thinking` block | `reasoning` part |

不变量(两者共享):策略①字段过滤、按消息时间做日期过滤、真实 cwd 组件级前缀匹配、排除优先。

### 4. Validation & Error Matrix

| 条件 | 行为 |
|------|------|
| `~/.zcode` 不存在 / db 缺失 / 打开失败 | 返回 `Ok((vec![], 0))` **静默跳过**(不阻断其它采集器) |
| `parent_id` 非空(subagent) | 整 session 不采集 |
| `message.time_created` 落目标 date(本地) | 进入解析;否则跳过(跨天切片,不计 skipped) |
| `part.data` JSON 非法 | 计入 `skipped_lines`,跳过该 part |
| cwd 命中黑名单 / 不在白名单 | 整 session 被丢弃(不计 skipped) |

### 5. Tests Required
`zcode.rs` 的 `#[cfg(test)]` 必须覆盖:
- `text` part → 文本;`tool` part → `"{name}: {key}"`;`reasoning`/`step-*`/`file` → 丢弃(返回 None)。
- 毫秒→本地日期过滤(`date_matches`),同 ms 在不同 date 判定不同。
- 跨天切片(`build_day_lines`):同 session 两条 message 分属不同日,只留目标日;目标日无 message → 空。
- 端到端:`collect_real_zcode_sample_day`(ignored,需本机 db)对真实 SQLite 采集主会话。

---

## Scenario: Codex (rollout jsonl) 数据源

> Codex(OpenAI Codex CLI)的对话存 jsonl rollout,与 Claude Code 同为 append-only
> 行式日志,但事件结构不同:对话有两套并行表示,文本源必须取展示层的 `event_msg`,
> 不能取 API 层的 `response_item`/`message`。本场景覆盖其数据源结构、文本源选择、
> cwd 来源、工具调用分支与字段映射。

### 1. Scope / Trigger
修改 `codex.rs`、新增基于 rollout jsonl 的采集器、或读取 Codex 会话字段时遵守本契约。

### 2. Signatures
**数据源路径**:`~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<ts>-<uuid>.jsonl`,入口
`codex::home_sessions_dir()`(递归收集 `*.jsonl`,见 `collect_jsonl_files`)。

**事件顶层字段**:`timestamp`(UTC, RFC3339)、`type`、`payload`。

**采集器**:`CodexCollector`(`id="codex"`,`display_name="Codex"`),复用
`claude_code::session_allowed`(已 `pub(super)`)做路径过滤。

### 3. Contracts

#### 3e. Codex rollout jsonl 数据源(硬契约)
- **文本源(硬契约)**:对话文本**只取 `event_msg`** 事件:
  - `payload.type=="user_message"` → User 行,文本=`payload.message`;
  - `payload.type=="agent_message"` → Assistant 行,文本=`payload.message`(`phase=final_answer`)。
  - `payload.message` 去空白后为空 → 跳过该行。
- **`response_item`/`message` 整类丢弃**(含 `role=developer|user|assistant`):它是 API 层
  原始消息,含注入的权限 / AGENTS.md / skills 指令噪声,user 真实输入已由 `user_message`
  干净覆盖——**不得用作文本源**(避免噪声与重复)。
- **cwd 来自 `session_meta.payload.cwd`**(真实路径,无 Claude Code 的目录名编码歧义),
  直接喂 `session_allowed`,沿用 §3b 组件级前缀 / 排除优先。**会话级 cwd,不随行覆盖。**
- **session_id**:`session_meta.payload.session_id`,回退文件名 uuid 段。
- **时间过滤(硬契约)**:按每行顶层 `timestamp`(UTC RFC3339)转本地时区后比 date;
  绝不按文件名日期段或文件 mtime——rollout 按**会话起始日**归档,跨天延续的会话需靠
  timestamp 才能被目标日命中(同 session 按目标日切片)。
- **字段过滤(策略①)**:`event_msg`/`{user,agent}_message` 保留;`task_started`/
  `task_complete`/`token_count`/`thread_settings_applied`/`session_meta`/`world_state`/
  `turn_context` 一律丢弃。
- **扁平文本内嵌块剥离(策略①,关键)**:Codex 把 tool_result / 命令输出摊平成文本标签
  塞进 `event_msg` 的 `message`(与 Claude Code 的独立 tool_result block 不同)。`clean_message`
  在保留前剥除这些块,等价实现"丢 tool_result 全文":
  - `[external_agent_tool_result] … [/external_agent_tool_result]` → 丢;
  - `[external_agent_tool_call[: NAME]] … [/external_agent_tool_call]` → 留 NAME 摘要;
  - `<task-notification> … </task-notification>`、`<command-name>` / `<command-message>` /
    `<command-args>` / `<local-command-stdout>` → 丢。
  - 剥光后(text 与 tools 皆空)的 message 整条丢弃。
  - 实测本机 2026-07-31:est_tokens 812k → 208k(↓74%),证明 tool_result 全文是 token 膨胀主因。
- **工具调用(防御性,本机零样本)**:`response_item`/`function_call`(`payload.name`+
  `payload.arguments`,arguments 为 **JSON 编码字符串**,解析后取
  `{file_path|path|command|pattern|url|description}` 回退链截断 80)、`response_item`/
  `local_shell_call`(`payload.action.command`,name 固定 `"shell"`)→ Assistant 工具行。
  本机实测仅有内嵌于 `agent_message.message` 的 `[external_agent_tool_call: …]` 文本标签
  (作为 Assistant 文本保留,同 Claude Code 对 meta 文本的 MVP 处理),**无真实
  `function_call` 事件**;该分支由合成 fixture 单测钉结构,留待 agentic 使用后回归。
- **project 显示名**:cwd basename(如 `yqnf-contract`);不可得回退 session_id 前 8 位。

#### 与 Claude Code / ZCode 的关键差异(实现/维护时对照)
| 维度 | Claude Code | ZCode | Codex |
|---|---|---|---|
| 存储 | jsonl 文件 | SQLite(session/message/part) | jsonl 文件(rollout) |
| 路径 | `~/.claude/projects/<enc>/*.jsonl` | `~/.zcode/cli/db/db.sqlite` | `~/.codex/sessions/<Y>/<M>/<D>/rollout-*.jsonl` |
| 文本源 | 每行 `message.content` | `part` type=text | **`event_msg`/`{user,agent}_message`.message** |
| 时间 | RFC3339 字符串 | Unix 毫秒 | RFC3339 字符串(顶层 timestamp) |
| cwd 来源 | 每行 `cwd` 字段 | `session.directory` | `session_meta.payload.cwd` |
| 噪声丢弃 | `thinking`/`tool_result` block | `reasoning`/`step-*`/`file` part | **`response_item`/`message` 整类(含 developer 注入)** |
| 工具调用 | `tool_use` block | `part` type=tool | `response_item`/`function_call`\|`local_shell_call`(本机无样本) |

不变量(三者共享):策略①字段过滤、按行/消息 timestamp 做日期过滤、真实 cwd 组件级前缀匹配、
排除优先、未装/读取失败静默跳过。

### 4. Validation & Error Matrix

| 条件 | 行为 |
|------|------|
| `~/.codex/sessions` 不存在 | 返回 `Ok((vec![], 0))` **静默跳过**(不阻断其它采集器) |
| 某 rollout 行 JSON 非法 | 计入 `skipped_lines`,跳过该行,继续 |
| 行无 `timestamp` 或解析失败 | 非 `session_meta` 行会计入 `skipped_lines`;`session_meta` 等无 timestamp 行正常跳过(不计) |
| 行 `timestamp` 落目标 date(本地) | 进入解析;否则跳过(跨天切片,不计 skipped) |
| `event_msg`/`{user,agent}_message` 的 `message` 为空白 | 跳过(不产 line,不计 skipped) |
| `response_item`/`message`(任意 role) | 整类丢弃(不计 skipped,正常过滤) |
| cwd 命中黑名单 / 不在白名单 | digest 被丢弃(不计 skipped) |
| session 无任何目标日期有效行 | 返回 `None`(不产出 digest) |

### 5. Tests Required
`codex.rs` 的 `#[cfg(test)]` 必须覆盖:
- `user_message` → User 文本;`agent_message` → Assistant 文本;空白 message → None。
- **`clean_message` 内嵌块剥离**:`[external_agent_tool_result]` 整块丢;`[external_agent_tool_call: X]`
  → 留工具名 X;`<command-*>` / `<task-notification>` / `<local-command-stdout>` 丢;整条剥光 → None;
  未闭合开标签当字面量保留(容错)。
- `task_started`/`token_count` 等元数据事件 → None;`response_item`/`message`(含 developer/user/assistant role)→ None。
- `function_call`(arguments 为 JSON 字符串 + 对象两种)→ `"{name}: {key}"`;`local_shell_call` → `"shell: <cmd>"`(合成 fixture)。
- `session_meta`/`world_state`/`turn_context` → None。
- `project_name`:cwd basename 回退;无 cwd → session_id 前 8 位。
- 跨天切片(`parse_session_cross_day_slice`):同 session 两行分属不同日只留目标日;目标日无行 → None。
- 端到端:`collect_real_codex_sample_day`(ignored,需本机 sessions)对真实 rollout 采集会话。

---

## Scenario: opencode (SQLite) 数据源

> opencode(sst/opencode)的对话同样存 SQLite,且与 ZCode **几乎完全同构**
> (`session` + `message` + `part` 三表,`part.data.type` 同为
> text/tool/reasoning/step-start/step-finish)。本场景覆盖其与 ZCode 的关键差异:
> 无 subagent(无 parent_id)、无 sequence 列、工具参数 camelCase、XDG 路径、额外 patch part。
> opencode 采集器**复用** ZCode 的 `build_day_lines` / `extract_from_parts` /
> `date_matches` / `ms_to_local` 纯函数,只新写 `collect` 与 `db_path`。

### 1. Scope / Trigger
修改 `opencode.rs`、新增基于 SQLite 的采集器、或读取 opencode `opencode.db` 字段时遵守本契约。

### 2. Signatures
**数据源路径**:`~/.local/share/opencode/opencode.db`,入口 `opencode::db_path()`
(opencode 跨平台统一用 Unix 风格 `$HOME/.local/share`,**不走** Windows `%LOCALAPPDATA%`;
回退 `data_local_dir()` 兜底 XDG)。

**只读打开**(同 ZCode):
```rust
Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
```

**采集器**:`OpencodeCollector`(`id="opencode"`,`display_name="Opencode"`),复用
`claude_code::session_allowed` 与 `zcode::{build_day_lines, extract_from_parts, ...}`。

**三张核心表**(与 ZCode 同构,实测):
- `session`:`id` / `directory`(真实 cwd) / `title` / `time_created`(Unix 毫秒)。**无 `parent_id`。**
- `message`:`session_id` / `time_created`(Unix 毫秒) / `data`(`{role:"user"|"assistant",...}`)。**无 `sequence`。**
- `part`:`message_id` / `data`(`{type:"text"|"tool"|"reasoning"|"step-start"|"step-finish"|"patch",...}`)。**无 `sequence`。**

### 3. Contracts

#### 3f. opencode SQLite 数据源(硬契约)
- **全 session 采集**(无主/sub 区分):opencode **没有** `parent_id` 列,所有 session 都是
  用户直接交互(无 ZCode 那种 subagent 噪声),一律采集。
- **排序键 = `time_created`**(硬差异):opencode 的 message/part 表**没有 `sequence` 列**
  (与 ZCode 不同),`ORDER BY time_created` 升序(实测同 message 内单调递增)。
- **真实 cwd 来自 `session.directory`**:直接喂 `session_allowed`,沿用 §3b 组件级前缀 / 排除优先。
- **时间过滤(硬契约)**:按 `message.time_created`(**Unix 毫秒**)转本地时区→比 date;
  绝不按文件 mtime(session 跨天累积,同 session 按目标日切片)。
- **字段过滤(策略①)**:复用 `zcode::extract_from_parts`——`part.type=text` → 文本(`data.text`);
  `part.type=tool` → `"{data.tool}: {key}"`;`reasoning`/`step-*`/`file`/**`patch`** 一律丢弃。
- **工具参数 camelCase(硬差异,关键)**:opencode 的 `state.input` 用 **camelCase**(`filePath`/
  `command`/`pattern`/`description`),而 ZCode 用 snake_case。`extract_from_parts` 的 key 回退链
  **同时列两套**(`file_path`→`filePath`→`path`→`command`→...)兼容两者——ZCode 数据只命中
  snake_case,opencode 数据只命中 camelCase,互不干扰。
- **`patch` part 丢弃**:opencode 特有的代码补丁 part(`{type:"patch",path,content}`),
  等同 tool_result 全文,丢入 `_ => {}` 分支(与 ZCode 的 `file` 同处理)。
- **只读访问**:`READ_ONLY | NO_MUTEX` 打开,不写入、不阻塞 opencode 进程。
- **project 显示名**:`session.title`(opencode 标题质量高,如"提交 vin-finance-map 业务代码并
  同步分支"),空回退 `directory` basename,再回退 session_id。
- **role**:`message.data.role`(`"assistant"`→Assistant,其余→User)。

#### 与 ZCode 的关键差异(实现/维护时对照——两者 SQLite schema 高度同构)
| 维度 | ZCode | opencode |
|---|---|---|
| db 路径 | `~/.zcode/cli/db/db.sqlite` | `~/.local/share/opencode/opencode.db`(XDG 风格) |
| 主/sub 区分 | `parent_id IS NULL` 过滤 subagent | **无 parent_id**,全采 |
| 排序列 | `ORDER BY sequence` | **无 sequence**,`ORDER BY time_created` |
| 工具参数字段 | snake_case(`file_path`) | **camelCase**(`filePath`) |
| 额外丢弃 part | `file` | `file` + **`patch`**(代码补丁) |
| 纯函数 | 自持 | **复用 zcode 的 `build_day_lines`/`extract_from_parts`/`ms_to_local`** |

不变量(与 ZCode 共享):策略①字段过滤、按 `message.time_created` 毫秒做日期过滤(跨天切片)、
真实 cwd 组件级前缀匹配、排除优先、未装/读取失败静默跳过 `Ok((vec![], 0))`。

### 4. Validation & Error Matrix

| 条件 | 行为 |
|------|------|
| `~/.local/share/opencode/opencode.db` 不存在 / 打开失败 | 返回 `Ok((vec![], 0))` **静默跳过**(不阻断其它采集器) |
| `message.time_created` 落目标 date(本地) | 进入解析;否则跳过(跨天切片,不计 skipped) |
| `part.data` JSON 非法 | 计入 `skipped_lines`,跳过该 part |
| cwd 命中黑名单 / 不在白名单 | 整 session 被丢弃(不计 skipped) |

### 5. Tests Required
`opencode.rs` 的 `#[cfg(test)]` 必须覆盖(重点钉**差异点**):
- `extract_from_parts`(复用 zcode 的):`text`→文本;`tool` + **camelCase input**(`filePath`/`command`)→
  `"{name}: {key}"`;`reasoning`/`step-*`/`patch`→丢弃;混合留 text+tool。
- `build_day_lines`(复用 zcode 的)跨天切片:目标日只留当日 message;目标日无 message → 空。
- `db_path`:命中真实存在的 `~/.local/share/opencode/opencode.db`,父目录以 opencode 收尾。
- 端到端:`collect_real_opencode_sample_day`(ignored,需本机 db)对真实 SQLite 采集(2026-08-03 已知会话)。

---

## Scenario: 数据源路径可编辑与默认值

> 各采集工具的数据源路径默认硬编码(`home_projects_dir`/`home_sessions_dir`/`db_path`),
> 用户可经设置页**覆盖**为自定义路径(如工具装在非默认位置)。本场景覆盖覆盖语义、
> `~` 展开、默认值来源,以及「空覆盖 = 用默认」这一跨层硬契约。**只改数据源定位,
> 不动过滤/解析/时间切片**(它们仍按各数据源场景的既有契约执行)。

### 1. Scope / Trigger
修改采集器路径解析、`Collector::collect`/`default_path`、`collect_conversations` 入参、
或 `CollectConfig.tool_paths` 时遵守本契约。

### 2. Signatures
```rust
// collector/mod.rs —— 共享路径解析纯函数
pub(super) fn expand_home(p: &str) -> Option<PathBuf> // `~`/`~/x`/`~\x`→主目录;空→None

// 各采集器内统一解析:非空覆盖(展开 ~)优先,否则用默认
let path = custom_path.and_then(super::expand_home).or_else(|| self.default_path());
```
`CollectConfig.tool_paths: HashMap<String, String>`(`#[serde(default)]`,键=工具 id,
值=路径串);TS 侧 `toolPaths: Record<string, string>`。

### 3. Contracts

#### 3g. 数据源路径覆盖(硬契约)
- **空覆盖 = 用默认(跨层一致)**:`tool_paths` 中键缺失、或值为空串/纯空白 → 用该工具
  `default_path()`。Rust(`expand_home` 返回 None 即回退默认)与 TS(`saved[t.id] ?? ""`、
  `storedPath` 等于默认存 `""`)两侧语义必须一致。
- **`~` 展开(`expand_home`)**:`~` → 主目录;`~/x`、`~\x` → `home.join(x)`;绝对/无前缀
  路径原样返回(去首尾空白)。**自定义路径支持 `~`,默认路径已由各 `db_path`/`home_*_dir` 展开。**
- **默认值由后端权威给出**:前端**不得**硬编码默认路径(opencode 有主路径/XDG 回退,只有后端
  `db_path()` 能给出权威默认)。设置页初值与「恢复默认」均取自 `default_collect_paths()` 命令。
- **覆盖点**:覆盖只决定「数据源定位」(`base`/`db` 变量),**其后 read_dir / 递归收集 /
  SQL 查询 / 过滤 / 解析全部沿用各数据源场景的既有契约,不变。**
- **路径缺失的既有语义保持不变**:claude-code 取不到主目录或目录不可读 → `Err` 上抛;
  codex 取不到主目录 → `Err`;目录/db 不存在或打开失败 → zcode/codex/opencode `Ok((vec![], 0))`
  静默跳过。**自定义路径指向不存在位置时,沿用所属工具的上述语义**(claude-code 报错上抛会中断
  整次采集——这是既有契约,UI 不做存在性预校验以免跨工具连锁误判)。
- **opencode 默认的双路径回退仅对默认生效**:`default_path()` 走原 `db_path()`(主路径存在用主,
  否则 XDG 兜底);自定义路径则直连用户指定文件(用户自负)。
- **向后兼容**:`tool_paths` 全 `#[serde(default)]`;老配置无此字段 → 空 map → 全用默认。

### 4. Validation & Error Matrix

| 条件 | 行为 |
|------|------|
| `tool_paths[tool]` 缺失 / 空串 / 纯空白 | 用该工具 `default_path()`(回退默认) |
| 覆盖为 `~/x` 形式 | `expand_home` 展开为主目录下 `x` |
| 覆盖为绝对路径 | 原样用作数据源 |
| claude-code 覆盖目录不可读 | `Err`(上抛,中断整次采集)——既有语义 |
| zcode/codex/opencode 覆盖路径不存在 | `Ok((vec![], 0))` 静默跳过——既有语义 |
| 取不到主目录(无覆盖且默认也 None) | claude-code/codex `Err`;zcode/opencode 静默跳过 |
| 老配置无 `tool_paths` | 空 map,全用默认(`#[serde(default)]`) |

### 5. Tests Required
`collector/mod.rs` 的 `#[cfg(test)]` 覆盖 `expand_home`:
- `~` → 主目录;`~/x`、`~\x` → `home.join(x)`。
- `""` / `"   "` → `None`;绝对路径 / 无前缀 → 原样(去空白)。

---

## 关联
- Claude Code 路径过滤任务:`.trellis/tasks/07-10-collect-path-filter/`(prd/design/implement)。
- ZCode 采集任务:`.trellis/tasks/08-04-zcode-collector/`(prd/design/implement)。
- Codex 采集任务:`.trellis/tasks/08-04-codex-collector/`(prd/design/implement)。
- opencode 采集任务:`.trellis/tasks/08-05-opencode-collector/`(prd/design/implement)。
- 数据源路径可编辑与恢复默认任务:`.trellis/tasks/08-05-collector-paths-editable/`(prd/design/implement)。
- 字段级内容过滤(策略①,保留 user/assistant 文本 + tool 摘要,丢 tool_result/thinking/reasoning)
  见 `claude_code.rs::extract_line` 与 `zcode.rs::extract_from_parts` 及其单测。
- opencode 复用 zcode 的纯函数:`zcode.rs::{build_day_lines, extract_from_parts, ms_to_local}`
  均 `pub(super)`;camelCase 工具参数回退链(`filePath` 等)在 `extract_from_parts` 内两套兼容。
