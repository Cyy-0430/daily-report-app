# 技术设计:从本地 AI 工具对话自动采集生成日报

## 1. 架构与边界

- **采集与生成分离**:采集是纯本地、无 LLM、无 token 的操作;生成完全复用现有 `llm.rs` 流式链路。两者通过一段文本(`{{conversations}}`)解耦。
- **新增 Rust 模块** `src-tauri/src/collector/`:采集器抽象 + Claude Code 实现。
- **前端** 在主页 `+page.svelte` 增加「采集区」组件;设置页可加工具开关(也可直接放主页)。
- 边界:采集器只读本地文件,不写入、不联网。采集失败/部分失败不阻断生成(返回已成功部分 + 错误计数)。

## 2. 核心抽象:Collector trait

```rust
// collector/mod.rs
pub struct ConversationLine {
    pub ts: DateTime<Utc>,
    pub role: Role,          // User / Assistant
    pub text: String,        // 过滤后的可见文本(user 文本 / assistant text)
    pub tool_summary: Vec<String>, // 如 ["Read: src/auth.ts", "Edit: src/llm.rs"]
}

pub struct SessionDigest {
    pub tool: String,            // "claude-code"
    pub project: String,         // 编码后的 project 目录名
    pub cwd: Option<String>,
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub lines: Vec<ConversationLine>,
    pub est_tokens: usize,
}

pub struct CollectResult {
    pub sessions: Vec<SessionDigest>,
    pub rendered_text: String,   // 直接喂给 {{conversations}} 的文本
    pub est_tokens: usize,       // rendered_text 的 token 估算
    pub skipped_lines: usize,    // 解析失败/跳过的行数(健康度)
}

pub trait Collector {
    fn id(&self) -> &'static str;          // "claude-code"
    fn display_name(&self) -> &'static str; // "Claude Code"
    fn collect(&self, date: NaiveDate) -> Result<Vec<SessionDigest>, String>;
}
```

MVP 仅实现 `ClaudeCodeCollector`。trait 为后续 OpenCode/Cursor 预留。

## 3. Claude Code 采集器实现要点

- **路径定位**:`dirs::home_dir().join(".claude/projects/*/*.jsonl")`,跨平台。
- **遍历**:`glob` 所有 project 目录下的 jsonl;逐文件、逐行 `serde_json::from_str`。
- **时间过滤(硬契约)**:
  1. 取每行 `timestamp`(形如 `2026-07-09T13:23:45.005Z`),parse 为 `DateTime<Utc>`。
  2. 转本地时区:`ts.with_timezone(&Local)`,取 `.date_naive()`。
  3. 与目标 `date` 比较,相等才保留。
- **字段级过滤(策略①)**:
  - `type=="user"` 且 `content` 为 str → 保留全文。
  - `type=="user"` 且 `content` 为数组 → 仅取其中 `type=="text"` 块,**跳过 `tool_result`**。
  - `type=="assistant"` → 取 `text` 块;`tool_use` 块仅记 `name` + 关键参数(`file_path`/`path`/`command`/`pattern`,截断 80 字)。
  - **丢弃** `thinking` 块、`tool_result` 全文。
- **宽容解析**:未知字段忽略;单行解析失败 → `skipped_lines++` 并跳过,不中断。
- **渲染**:每个 session 输出小标题(`[Claude Code] <project> · <起止> · N条`) + 过滤后行;多 session 按时间排序拼接成 `rendered_text`。

## 4. 时区处理(硬契约,独立成节强调)

- 所有源 timestamp 为 UTC。**禁止**用字符串前缀 `startswith("2026-07-09")` 匹配。
- 目标 date 是「用户本地时区的某一天」。中国 UTC+8:本地 7/9 = UTC `7/8 16:00 ~ 7/9 16:00`。
- 实现:`chrono::Local`(系统时区)+ `date_naive()` 比较。不硬编码 +8,依赖系统时区。

## 5. 命令契约

```rust
// collector/mod.rs 或 llm.rs
#[tauri::command]
pub async fn collect_conversations(
    date: String,        // "YYYY-MM-DD",本地时区日,空串=今天
    tools: Vec<String>,  // ["claude-code"]
) -> Result<CollectResult, String>
```

- `tools` 中非 `claude-code` 的 id:MVP 返回空 sessions + 提示「该工具尚未支持」。
- 在 `lib.rs` 的 `invoke_handler!` 注册。

## 6. 接入生成链路

- `render_template(template, input, date, conversations)` —— 末尾增 `conversations: &str`,内部 `.replace("{{conversations}}", conversations)`。
- `generate_report(app, input, conversations, on_event)` —— 增 `conversations: String`,透传给 `render_template`。
- **向后兼容**:`{{conversations}}` 为空时替换为空串;默认模板里该段写成「以下是今日各工具对话记录(参考素材):\n{{conversations}}」,空时不影响 LLM 仅凭 `{{input}}` 生成(行为同旧版)。
- 调用点:主页生成按钮同步改签名(实现时用 codegraph 确认所有 `generate_report` 调用点)。

## 7. 配置扩展(向后兼容)

```rust
// config.rs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectConfig {
    #[serde(default)]
    pub enabled_tools: Vec<String>, // 默认 ["claude-code"]
}

// AppConfig 增字段:
#[serde(default)]
pub collect_config: CollectConfig,
```

`#[serde(default)]` 保证旧 `data.json` 可正常加载。

## 8. 前端

- 主页 `+page.svelte` 新增「采集区」:
  - 工具勾选(Claude Code,来自 `collectConfig.enabledTools`)。
  - 日期 `<input type="date">`(默认今天)。
  - `[采集对话]` 按钮 → `invoke('collect_conversations', { date, tools })`。
  - 只读预览面板:列各 session(project / 条数 / token 估算)+ 总 token;展开可看渲染文本。
- 采集结果存组件 state;点生成时把 `renderedText` 作为 `conversations` 传给 `generate_report`。
- `bindings.ts`:`svelte-kit sync` 重新生成(或手补 `collectConversations` 与新类型)。

## 9. 依赖

- Rust 新增:`dirs`(跨平台 home 目录)、`glob`(或 `walkdir`,遍历 jsonl)。
- `chrono` 已在用(`llm.rs` 有 `chrono::Datelike`),补 `Local` / `DateTime` / `TimeZone`。
- `serde_json` 已有。
- **MVP 不引入 `rusqlite`**(Claude Code 不需要 SQLite;留待 OpenCode/Cursor 迭代)。

## 10. 兼容性 / 回滚

- AppConfig 增字段全 `#[serde(default)]` → 旧配置无损升级。
- 不采集 / conversations 为空 → 行为与当前版本完全一致(安全回滚面)。
- 采集是独立 command + 独立模块,出问题可整体禁用而不影响生成。

## 11. 风险与取舍

- **jsonl 格式随 Claude Code 版本变化** → 宽容解析(未知字段忽略、坏行跳过计数),降低脆弱性。
- **大文件性能** → 逐行流式读,不全量入内存;今天 2h/7 文件已实测可接受。
- **跨天 / 时区** → 已固化为硬契约(§4)。
- **token 估算精度** → 用经验公式(中 1.2 / ASCII 0.25),仅作预览参考,不用于计费。
