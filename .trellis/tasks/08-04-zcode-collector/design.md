# Design — ZCode 对话记录采集

## 1. 边界

新增 `src-tauri/src/collector/zcode.rs`，定义 `ZCodeCollector`，实现 `Collector` trait。**只负责"读 ZCode SQLite → 产出 `SessionDigest`"**；路径过滤、token 估算、渲染、日期解析、命令层 `collect_conversations` 全部复用现有代码，不改。

改动文件清单：
- `src-tauri/src/collector/zcode.rs`（新建）
- `src-tauri/src/collector/mod.rs`（`pub mod zcode` + `pub use` + `all_collectors()` 登记）
- `src-tauri/src/collector/claude_code.rs`（仅当需把 `norm`/`session_allowed` 提升为 `pub(super)` 复用——见 §6）
- `src/lib/bindings.ts`（如改默认 `enabledTools`）
- `src/routes/settings/+page.svelte`（单 toggle → 双开关）
- `src/routes/+page.svelte`（fallback 默认值，如需）

## 2. 数据源契约（已探针确认）

ZCode 主会话存在 `~/.zcode/cli/db/db.sqlite`，三张核心表：

### `session`（会话）
| 列 | 含义 | 用途 |
|---|---|---|
| `id` | 会话 id（`sess_*`；subagent 形如 `sess_subagent_*`） | session_id |
| `parent_id` | 父会话；**NULL = 主会话** | R3 主会话过滤 |
| `directory` / `path` | 真实 cwd（`D:\hand\yqnf\yqnf-contract`） | R6 路径过滤 |
| `title` | 会话标题 | project 显示名 |
| `time_created` / `time_updated` | **Unix 毫秒** | 起止时间参考 |

样本：38 主会话（parent_id IS NULL）/ 31 subagent。

### `message`（消息，2551 行）
| 列 | 含义 |
|---|---|
| `id`, `session_id`, `sequence` | 主键 / 外键 / 顺序 |
| `time_created` | **Unix 毫秒** → 时间过滤 + ts 展示 |
| `data` | JSON：`{role:"user"\|"assistant", path:{cwd}, modelID, tokens, finish, ...}` |

`data.role` 分布：user 377 / assistant 2174（assistant 每步一条，故多于 user）。

### `part`（消息内容片段，9355 行）
| 列 | 含义 |
|---|---|
| `id`, `message_id`, `session_id`, `sequence` | 外键 + 顺序 |
| `time_created` | Unix 毫秒 |
| `data` | JSON，`data.type` 区分内容 |

`data.type` 分布与映射：

| `data.type` | 数量 | 处理 | 字段 |
|---|---|---|---|
| `text` | 1961 | **保留** → `ConversationLine.text` | `data.text`（多条 join） |
| `tool` | 2644 | **保留** → `ConversationLine.tools` | `data.tool`(名) + `data.state.input`(参数) |
| `reasoning` | 432 | **丢弃**（= thinking） | — |
| `step-start` / `step-finish` | 2174 / 2135 | **丢弃**（步骤元数据） | — |
| `file` | 9 | **丢弃** | — |

`text` part 实样：`{"type":"text","text":"Cherry-pick 成功完成 ✅\n\n## 处理总结..."}`。
`tool` part 实样：`{"type":"tool","callID":"...","tool":"Bash","state":{"status":"completed","input":{"command":"...","description":"..."}}}`。

## 3. 字段映射（ZCode → `ConversationLine` / `SessionDigest`）

```
ConversationLine {
  ts:     message.time_created(ms) /1000 → Local → "%H:%M"
  role:   message.data.role == "user" ? User : Assistant
  text:   join(part.data.text  where part.type=="text"  and part.message_id==message.id)
  tools:  part where type=="tool" → "{data.tool}: {key}"
            key = data.state.input.{file_path | path | command | pattern | url | description}[0..80]
}
```

`SessionDigest`：
- `tool`: `"ZCode"`
- `project`: `session.title`（可读；空则回退 `directory` 的 basename）
- `cwd`: `session.directory`
- `session_id`: `session.id`
- `started_at` / `ended_at`: 该 session **在目标日期内** message 的 min/max `time_created` → Local `%Y-%m-%d %H:%M` / `%H:%M`
- `line_count` / `est_tokens` / `lines`: 复用 `session_tokens` 既有逻辑

## 4. 采集算法（`ZCodeCollector::collect`）

```
fn collect(date, filter):
  db = ~/.zcode/cli/db/db.sqlite
  if !db.exists(): return Ok((vec![], 0))            # R2 安静跳过
  conn = Connection::open_with_flags(db, READ_ONLY | NO_MUTEX)  # C3 只读
  skipped = 0
  for row in conn.query(
      "SELECT id, directory, title FROM session WHERE parent_id IS NULL"):  # R3
    (sid, dir, title) = row
    cwd = dir
    if !session_allowed(cwd, filter.includes, filter.excludes): continue     # R6
    lines = []
    for m in conn.query(
        "SELECT id, time_created, data FROM message
         WHERE session_id=? ORDER BY sequence", sid):
      local = ms_to_local(m.time_created)
      if local.date_naive() != date: continue          # R5 时间过滤（非目标日不计 skipped）
      role = m.data["role"]
      parts = conn.query("SELECT data FROM part WHERE message_id=? ORDER BY sequence", m.id)
      (text, tools) = extract_from_parts(parts)        # R4 策略①
      if text or tools 非空:
        lines.push(ConversationLine{ ts: local%HH:MM, role, text, tools })
    if !lines.empty:
      digests.push(SessionDigest{ ... started/ended 取 lines 时间, lines })
  digests.sort_by(started_at)
  Ok((digests, skipped))
```

> `extract_from_parts` 是纯函数，对应 `claude_code.rs::extract_line`，单独单测。

### 4.1 跨天切片
一个 ZCode session 跨多天时，上面的"按 `local.date_naive() != date continue"天然只留目标日期的 message，`started_at/ended_at` 也只用当天 min/max——等价于 Claude Code 的按行 timestamp 过滤，无需额外切分逻辑。

## 5. 与 Claude Code 的差异矩阵（关键，实现时对照）

| 维度 | Claude Code | ZCode |
|---|---|---|
| 存储 | jsonl 文件（append-only） | SQLite 表（session/message/part） |
| 定位 | `~/.claude/projects/<enc>/*.jsonl` | `~/.zcode/cli/db/db.sqlite` |
| 对话单元 | 每行一个 `{type:user\|assistant, message.content}` | message（role） + part（type） 两表关联 |
| 时间 | RFC3339 字符串 | **Unix 毫秒** |
| cwd 来源 | 每行 `cwd` 字段 | `session.directory` |
| 主/sub 区分 | 无（目录即 session） | `session.parent_id IS NULL` |
| 路径编码 | 目录名编码（歧义，禁用）→ 用真实 cwd | `directory` 本就是真实路径 |
| 文本/工具 | `content[]` 里 `text`/`tool_use` block | `part` 表 `type=text`/`tool` |
| thinking | `thinking` block 丢弃 | `reasoning` part 丢弃 |

不变量（两者共享）：策略①字段过滤、按行/消息 timestamp 做日期过滤、真实 cwd 组件级前缀匹配、排除优先。

## 6. 复用点

- `mod.rs::PathFilter` / `estimate_tokens` / `session_tokens` / `render` / `ConversationLine` / `SessionDigest` / `parse_target_date`（经 `collect_conversations`）：**直接用，不改**。
- `claude_code.rs::norm` / `session_allowed`：当前 `norm` 是 `pub(super)`，`session_allowed` 是私有。**方案**：把 `session_allowed` 提为 `pub(super)`（与 `norm` 同可见性），供 zcode.rs 复用；签名不变。这是对 claude_code.rs 唯一的改动，零行为变化。
- SQLite 访问：项目已依赖 `rusqlite`（bundled），直接用 `Connection::open_with_flags` + `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`。**无需新依赖。**

## 7. 风险与对策

| 风险 | 对策 |
|---|---|
| ZCode 正在运行写 db，读取阻塞或被锁 | 只读 + `NO_MUTEX` 打开；WAL 模式下只读连接不阻塞写者；最坏读到稍旧数据，可接受 |
| db 被以非常规方式独占（罕见） | `open_with_flags` 失败 → 返回 `Ok((vec![], 0))` 静默跳过（与 R2 一致），不阻断整体采集 |
| `time_created` 单位误判（秒 vs 毫秒） | 已确认是毫秒；`/1000` 后用 chrono `Local.timestamp_millis_opt` 或 `from_secs`；单测覆盖 |
| user message 混入系统提醒（如"The TodoWrite tool hasn't been used..."） | MVP 保留（Claude Code 也有同类 meta 文本）；如需过滤后续加规则，不阻塞本任务 |
| `data` JSON 结构在不同 ZCode 版本漂移 | 用 `serde_json::Value` 动态取字段 + 缺失即跳过（同 Claude Code 的 `extract_line` 容错风格），单测钉住当前结构 |
| `part.data.state.input` 字段名不一（command/path/file_path...） | 沿用 Claude Code 的多键回退链 + `description` 兜底，取首个非空 |

## 8. 跨层与配置

- **config.rs `CollectConfig.enabledTools`**：结构不变（`Vec<String>`）。是否改默认值见 §9。
- **bindings.ts**：默认 `enabledTools` 若改动需同步；接口无新增字段（仍 `string[]`）。
- **settings/+page.svelte**：`collectEnabled: boolean` → `enabled: Record<toolId, boolean>`（或两个布尔 `ccEnabled`/`zcodeEnabled`），save 时映射回 `enabledTools: string[]`。
- **+page.svelte**：fallback `"claude-code"` → 是否含 zcode 取决于默认决策。

工具元数据建议在前端定义一处常量（避免 id/显示名散落）：
```ts
const COLLECT_TOOLS = [
  { id: "claude-code", label: "Claude Code" },
  { id: "zcode", label: "ZCode" },
];
```

## 9. 已定稿决策（review gate 确认）

1. **默认启用 ZCode**：`enabledTools` 默认值由 `["claude-code"]` 改为 `["claude-code","zcode"]`（`config.rs` 的 `CollectConfig::default()` 与 `bindings.ts` 默认值同步，确认 `#[serde(default)]` 链完整）。
2. **project 显示名**：用 `session.title`（可读，如"Cherry-pick 4051279c to dev-mysql"），空则回退 `directory` 的 basename。
