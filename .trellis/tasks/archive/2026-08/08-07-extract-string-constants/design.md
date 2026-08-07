# Design — 提取重复/魔法字符串为常量

> 配套 `prd.md`。纯重构：边界、常量组织、跨层契约、权衡。执行清单见 `implement.md`。

## 1. 重构性质

**行为零变更的字面量→常量替换**。不改控制流、不改任何字符串的运行时值；既有测试是
回归防线（AC1）。唯一非平移动作：`truncate` 三份合并（三份实现逐字相同，合并无行为差异）。

## 2. 常量组织策略

- **Rust：就近定义在使用模块顶部**，不新建 constants 模块（Rust 惯例，避免过度集中）：
  - `db.rs`：配置 KV 键、meta 键、哨兵值、文件名。
  - `llm.rs`：端点片段、SSE 标记、模板占位符、消息文案、stage、title/分隔符。
  - `collector/mod.rs`：跨采集器共享的日期格式、`truncate` + 截断长度、工具 id 常量
    （归属"工具"概念，llm.rs/db.rs/各 collector 引用）。
- **前端：派生自现有单一来源**，消除重写：
  - 工具默认数组 → `COLLECT_TOOLS.map(t => t.id)`。
  - 重复 UI 文案 → 提为常量（就近，见 §6 权衡 D）。

## 3. 常量清单与落点

### 3.1 db.rs
| 常量 | 值 | 替换点 |
|---|---|---|
| 配置键 9 个（`KEY_API_CONFIG` 等） | `"api_config"` … | get_config L86–112 + config_pairs L128–137（两处镜像→共用同一组 const） |
| `META_SCHEMA_VERSION` / `META_MIGRATED_FROM_STORE` | `"schema_version"` / `"migrated_from_store"` | db.rs:52,259,288；lib.rs:28 |
| `SCHEMA_VERSION_VALUE` | `"1"` | db.rs:52,288 |
| `LEGACY_STORE_FILE` / `STORE_KEY_CONFIG` | `"data.json"` / `"config"` | db.rs:242–243 |
| `DB_FILE_NAME` | `"daily_report.db"` | lib.rs:23（就近定义在 lib.rs） |

### 3.2 llm.rs
| 常量 | 值 | 替换点 |
|---|---|---|
| `CHAT_COMPLETIONS` / `V1` | `"/chat/completions"` / `"/v1"` | build_endpoint L129–134 |
| `SSE_DATA` / `SSE_DONE` | `"data:"` / `"[DONE]"` | stream_chat_once L178,181,182 |
| `TPL_DATE`/`TPL_INPUT`/`TPL_CONV`/`TPL_DATE_RANGE`/`TPL_DAY_SUMMARIES` | `"{{date}}"` 等 | render_template/render_weekly_map/render_weekly_reduce L40–57 |
| `MSG_API_INCOMPLETE` | `"请先在设置中填写完整的 API 配置（BaseURL / Key / 模型）"` | L289,396（去重 2→1） |
| 单次消息 const | `"响应缺少 choices[0].message.content"`、`"连接成功"`、`"连接失败 {status}：{text}"`、`"区间内无有效对话，且未填写补充要点，无法生成周报"`、`"请填写完整的 API 配置"`、`"请求失败：{e}"`、`"API 返回错误 {status}：{text}"` | L227,319,339,343,478,163/217/337,167/221 |
| `STAGE_MAP` / `STAGE_REDUCE` | `"map"` / `"reduce"` | L457,465,487 |
| title/分隔符 const | `"日报"`/`"周报"`/`"~"`/`"、"`/`"（无）"` | L369,498,505,531,534 |
| `FMT_DATE`/`FMT_HM`/`FMT_DATE_HM` | `"%Y-%m-%d"`/`"%H:%M"`/`"%Y-%m-%d %H:%M"` | 定义在 collector/mod.rs `pub(crate)`，llm.rs（L297,369,455,464,531,532）引用 |

> `FMT_*` 与 `MSG_HOME_NOT_FOUND` 因 collector 也用，定义在 collector/mod.rs `pub(crate)`，
> llm.rs 与各 collector 引用（跨模块共享）。

### 3.3 collector/mod.rs（跨采集器共享）
| 常量 | 值 | 说明 |
|---|---|---|
| `TOOL_ID_CLAUDE_CODE`/`_ZCODE`/`_CODEX`/`_OPENCODE` | `"claude-code"` 等 | 各 collector `id()` 引用 |
| `DEFAULT_TOOL_IDS: &[&str]` | 上述四者，顺序与 all_collectors / COLLECT_TOOLS 一致 | config.rs default_enabled_tools + 前端引用 |
| `TOOL_KEY_MAX_LEN` | `80` | 各 truncate 调用点（claude_code.rs:279 等 6 处） |
| `MSG_HOME_NOT_FOUND` | `"无法定位用户主目录"` | claude_code.rs:42,93；codex.rs:59,93（去重 4→1） |
| `truncate(s,n)` 合并 | — | 删除 claude_code.rs:298 / codex.rs:389 / zcode.rs:291 三份，提到 mod.rs `pub(super)` |

### 3.4 前端
| 改动 | 位置 |
|---|---|
| `emptyConfig()` 工具数组 → `COLLECT_TOOLS.map(t => t.id)` | bindings.ts:135 |
| `/` 与 `/weekly` 回退数组 → 派生（抽 `DEFAULT_TOOL_IDS` 导出复用） | +page.svelte:27；weekly/+page.svelte:29 |
| `MSG_API_NOT_CONFIGURED`（B 组文案） | +page.svelte:81；weekly/+page.svelte:94 |
| settings 展示的 `{{date}}` 等文本（C 组，可选）→ 引用 template.ts 导出占位符常量 | settings/+page.svelte:259–298 |

## 4. 跨层契约对齐

- **工具 id**：后端 `DEFAULT_TOOL_IDS`（collector/mod.rs）与前端 `COLLECT_TOOLS` 的 id 列表
  **值与顺序必须一致**（已是现状，常量化后更易核对）。前端默认启用集**派生**自 COLLECT_TOOLS，
  不再手写 → 天然一致。
- **进度 stage**：`"map"`/`"reduce"` 后端 `STAGE_*` const 与前端 StreamChunk 的
  `stage:"map"|"reduce"` 字面量类型（bindings.ts:52）对应；前端 weekly/+page.svelte:184
  `progress.stage === "reduce"` 保留字面量比较（TS 字面量类型不抽常量，见 prd 排除项）。

## 5. 兼容性

- 纯重构，无序列化/IPC/持久化格式变化；既有用户配置、历史、迁移逻辑零影响。
- `truncate` 合并：三份实现逐字相同（均已 `cargo test` 覆盖），合并后行为等价。
- 工具 id 常量化：常量值与原字面量逐字相同，运行时值不变。

## 6. 关键权衡

- **权衡 A — config.rs 是否引用 collector 的工具 id 常量**：default_enabled_tools（L37–44）
  若引用 `crate::collector::DEFAULT_TOOL_IDS`，会引入 config → collector 依赖（当前 config 仅
  依赖 db，collector 不依赖 config，故**不构成循环**）。收益：消除 4 个工具 id 字面量重复、
  语义更正（"默认启用全部已注册采集器"）。**推荐：做**，接受该单向依赖。
- **权衡 B — 测试是否引用生产常量**：db.rs 测试里的 `"claude-code"`（L364,421–426）改引用
  `crate::collector::TOOL_ID_*`；但 `sample_config` 的 `"gpt-demo"`/`"sk-xxx"` 等纯数据保留
  字面量（prd 排除）。**推荐：工具 id 引用常量（单一来源），纯数据保留**。
- **权衡 C — 配置键用数组迭代 vs 逐个 const**：config_pairs 已是数组，get_config 是逐字段
  if-let。**推荐：逐个 const，两处引用同一组 const**（改动小、可读、不引入动态分发/反射）。
- **权衡 D — 前端 UI 文案常量放哪**：`MSG_API_NOT_CONFIGURED` 仅 2 处。新建 messages.ts 过度；
  **推荐：就近放 `bindings.ts`（已承载类型/常量）或 `store.ts`**。低收益项，放最后。

## 7. 回滚形态

纯重构，回滚 = `git revert`；无数据/格式迁移，无外部副作用。
