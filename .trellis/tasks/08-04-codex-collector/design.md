# Design — Codex 对话记录采集

## 1. 边界

新增 `src-tauri/src/collector/codex.rs`，定义 `CodexCollector`，实现 `Collector` trait。**只负责"读 Codex rollout jsonl → 产出 `SessionDigest`"**；路径过滤、token 估算、渲染、日期解析、命令层 `collect_conversations` 全部复用现有代码，不改。

改动文件清单：
- `src-tauri/src/collector/codex.rs`（新建）
- `src-tauri/src/collector/mod.rs`（`pub mod codex` + `pub use` + `all_collectors()` 登记）
- `src/lib/bindings.ts`（`COLLECT_TOOLS` 加 codex 项 + 默认 `enabledTools`）
- `src/routes/+page.svelte`（fallback 默认值同步）

> `claude_code.rs` 的 `norm`/`session_allowed` 已是 `pub(super)`（ZCode 任务已提升），codex.rs 直接 `use super::claude_code::session_allowed`，**无需再改 claude_code.rs**。
> settings 页已按 `COLLECT_TOOLS` 数据驱动渲染（`{#each COLLECT_TOOLS}`），**无需改**。
> 无新 Rust 依赖（`serde_json`/`chrono`/`dirs` 均已在用）。

## 2. 数据源契约（已在本机探针确认）

Codex 每个会话一个 rollout jsonl：`~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<ts>-<uuid>.jsonl`，append-only，**每行一个事件 JSON**。

顶层字段：`timestamp`（RFC3339 UTC）、`type`、`payload`。

本机全量统计（所有 sessions）的事件分布：

| 顶层 `type` | `payload.type` | 数量 | 处理 |
|---|---|---|---|
| `session_meta` | — | 每文件首行 | **取 `payload.cwd`**（真实 cwd）、`payload.session_id` |
| `event_msg` | `user_message` | 231 | **保留** → User 行，文本=`payload.message` |
| `event_msg` | `agent_message` | 3746 | **保留** → Assistant 行，文本=`payload.message`（`phase=final_answer`） |
| `event_msg` | `task_started`/`task_complete`/`token_count`/`thread_settings_applied` | — | **丢弃**（元数据） |
| `response_item` | `message`（role=developer/user/assistant） | 3939 | **丢弃**（API 层原始消息，含注入的权限/AGENTS.md/skills 噪声；user 真实输入已由 `user_message` 干净覆盖，避免重复） |
| `response_item` | `function_call` / `local_shell_call` | **0（本机无样本）** | 防御性保留分支（§3/R7），合成 fixture 单测 |
| `world_state` / `turn_context` | — | — | **丢弃** |

实样（"hi" 会话，已验证）：
- `session_meta.payload.cwd` = `"D:\\hand\\yqnf\\yqnf-contract"`
- `event_msg`/`user_message`：`{"type":"user_message","message":"hi",...}`
- `event_msg`/`agent_message`：`{"type":"agent_message","message":"Hi. What are we working on?","phase":"final_answer",...}`

> **验证状态声明**：文本源（user_message/agent_message）经本机真实数据验证。工具调用分支（function_call/local_shell_call）本机零样本，**未经真实数据验证**，仅按公开 Codex rollout schema 防御性实现 + 合成单测；若用户实际以 agentic 模式用 Codex，该分支才会触发，届时可再用真实数据回归。

## 3. 字段映射（Codex → `ConversationLine` / `SessionDigest`）

```
ConversationLine {
  ts:     顶层 timestamp(UTC RFC3339) → Local → "%H:%M"
  role:   payload.type=="user_message" ? User : Assistant   // agent_message → Assistant
  text:   payload.message  (trim; 空 → 跳过该行)
  tools:  []   // 文本事件无工具；工具另见下
}

// 防御性工具行（response_item,function_call / local_shell_call）:
ConversationLine {
  ts:     顶层 timestamp → Local "%H:%M"
  role:   Assistant
  text:   ""
  tools:  ["{name}: {key}"]
            function_call:     name=payload.name, key=从 payload.arguments(JSON 字符串)解析
                               取 {file_path|path|command|pattern|url|description}[0..80]
            local_shell_call:  name="shell", key=payload.action.command[0..80]
}
```

`SessionDigest`：
- `tool`: `"Codex"`
- `project`: 回退链——`session_meta` 无 title 字段；用 **`cwd` 的 basename**（如 `yqnf-contract`），与 ZCode 回退一致；basename 不可得则回退 `session_id` 前 8 位。
- `cwd`: `session_meta.payload.cwd`
- `session_id`: `session_meta.payload.session_id`（回退：文件名里的 uuid 段）
- `started_at` / `ended_at`: 该 session **在目标日期内**行的 min/max timestamp → Local `%Y-%m-%d %H:%M` / `%H:%M`
- `line_count` / `est_tokens` / `lines`: 复用 `session_tokens`

## 4. 采集算法（`CodexCollector::collect`）

```
fn collect(date, filter):
  base = ~/.codex/sessions
  if !base.exists(): return Ok((vec![], 0))            # R2 静默跳过
  skipped = 0
  for rollout in walk(base).filter(ext=="jsonl"):      # 递归 sessions/<Y>/<M>/<DD>/
    (digest_opt, sk) = parse_session(rollout, date)
    skipped += sk
    if let Some(d) = digest_opt:
      if !d.lines.empty():
        cwd_path = d.cwd.as_deref().map(Path::new)
        if session_allowed(cwd_path, filter.includes, filter.excludes):   # R6
          digests.push(d)
  digests.sort_by(started_at)
  Ok((digests, skipped))
```

`parse_session`（纯解析，单一职责，不含路径过滤——同 Claude Code）：

```
fn parse_session(path, date) -> (Option<SessionDigest>, usize):
  session_id = 文件名 uuid 段（回退）
  cwd = None; started=None; ended=None; lines=[]; skipped=0
  for raw in read_lines(path):
    ev = parse_json(raw) or { skipped+=1; continue }
    ts_str = ev["timestamp"] or { skipped+=1; continue }      # R5
    dt = parse_rfc3339(ts_str) or { skipped+=1; continue }
    local = dt.to_local()
    if local.date_naive() != date: continue                   # 非目标日，不计 skipped
    # cwd 只从 session_meta 取一次
    if cwd.is_none() and ev["type"]=="session_meta": cwd = ev["payload"]["cwd"]
    started = min(started, local); ended = max(ended, local)
    if let Some(line) = extract_line(&ev):                    # R4/R7 策略①
      line.ts = local("%H:%M"); lines.push(line)
  if lines.empty: return (None, skipped)
  (Some(SessionDigest{...}), skipped)
```

> cwd 提取：仅 `session_meta` 行携带真实 cwd；其它行（含 `turn_context` 也有 cwd）不覆盖，保持"会话级 cwd"语义，与 ZCode `session.directory` 对齐。

### 4.1 跨天切片
按 `local.date_naive() != date` 行级 continue，天然只留目标日期；`started/ended` 也只用当天 min/max。等价于 Claude Code 的按行 timestamp 过滤。

## 5. 与 Claude Code / ZCode 的差异矩阵（实现时对照）

| 维度 | Claude Code | ZCode | **Codex** |
|---|---|---|---|
| 存储 | jsonl 文件 | SQLite 表 | **jsonl 文件（rollout）** |
| 定位 | `~/.claude/projects/<enc>/*.jsonl` | `~/.zcode/cli/db/db.sqlite` | **`~/.codex/sessions/<Y>/<M>/<D>/rollout-*.jsonl`** |
| 对话单元 | 每行 `{type:user\|assistant,message.content}` | message+part 两表关联 | **每行一个事件；文本在 `event_msg`/`user_message`\|`agent_message`** |
| 时间 | RFC3339 字符串 | Unix 毫秒 | **RFC3339 字符串（顶层 timestamp）** |
| cwd 来源 | 每行 `cwd` 字段 | `session.directory` | **`session_meta.payload.cwd`** |
| 路径编码 | 目录名编码（歧义） | 真实路径 | **真实路径** |
| thinking 丢弃 | `thinking` block | `reasoning` part | **`response_item`/`message`（含 developer 注入）整类丢弃** |
| 工具调用 | `tool_use` block | `part` type=tool | **`response_item`/`function_call`\|`local_shell_call`（本机无样本）** |
| project 名 | 编码目录名 | `session.title` | **cwd basename** |

不变量（三者共享）：策略①字段过滤、按行 timestamp 做日期过滤、真实 cwd 组件级前缀匹配、排除优先、未装/读取失败静默跳过。

## 6. 复用点

- `mod.rs`：`PathFilter` / `estimate_tokens` / `session_tokens` / `render` / `ConversationLine` / `SessionDigest` / `parse_target_date`（经 `collect_conversations`）——**直接用，不改**。
- `claude_code.rs::session_allowed`（`pub(super)`，ZCode 已提升）+ `norm`——**直接 use，不改 claude_code.rs**。
- 目录递归遍历：用 `std::fs` 手写两~三层 walk（`sessions/<Y>/<M>/<D>`，或用 `walkdir`——但项目未依赖 walkdir，且层级固定可手写，**不引新依赖**）。

## 7. 风险与对策

| 风险 | 对策 |
|---|---|
| `sessions` 目录层级深、文件多 | 只 `read_dir` 三层（Y/M/D）+ 文件 `*.jsonl`，按需读取；不预读全部 |
| 大 rollout 文件 | 逐行 `lines()` 流式解析，不整体载入；与 Claude Code 一致 |
| `event_msg` 与 `response_item` 双重表示导致重复 | **只用 event_msg 作文本源**，response_item/message 整类丢弃；agent_message 仅取 `phase=final_answer`（=最终回复），避免中间推理重复 |
| function_call 分支无真实样本 | 合成 fixture 单测钉 schema；实路径标注"未验证"；分支在无该事件时零成本不触发 |
| 用户 message 含 `text_elements`/images 等扩展字段 | 只读 `payload.message` 字符串字段，缺失即跳过（容错同 Claude Code `extract_line`） |
| Codex 版本升级导致 rollout schema 漂移 | `serde_json::Value` 动态取字段 + 缺失即跳过；单测钉当前结构；schema 变化最坏退化为少采，不报错 |
| 目录名日期段 vs 行 timestamp 不一致 | 一律按行 timestamp 过滤（硬契约），文件名日期段仅用于定位、不用于过滤 |

## 8. 跨层与配置

- `config.rs::CollectConfig.enabledTools`：结构不变（`Vec<String>`）。
- `bindings.ts`：`COLLECT_TOOLS` 加 `{ id:"codex", label:"Codex", hint:"~/.codex/sessions" }`；默认 `enabledTools` 加 `"codex"`。
- `+page.svelte`：fallback 默认值同步加 `"codex"`。
- settings 页：已数据驱动，无需改。
- `#[serde(default)]` 链：codex id 是运行时字符串，旧配置 `enabledTools` 不含 codex 不影响反序列化；是否默认采集见 §9。

## 9. 已定稿决策（review gate 确认）

1. **默认启用 Codex**：`enabledTools` 默认值由 `["claude-code","zcode"]` 改为 `["claude-code","zcode","codex"]`（`config.rs::CollectConfig::default()` + `bindings.ts` + `+page.svelte` fallback 三处同步）。未装 Codex 时静默跳过，对用户无干扰，故默认开启更省心。
2. **文本源用 event_msg**（user_message/agent_message），不用 response_item/message，以规避注入噪声与重复。
3. **project 显示名**：cwd basename（如 `yqnf-contract`），basename 不可得回退 session_id 前 8 位。
4. **工具调用分支保留但标注未验证**：合成单测覆盖，真实数据回归留待用户 agentic 使用后。
