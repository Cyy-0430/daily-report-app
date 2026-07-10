# 从本地AI工具对话自动采集生成日报

## Goal / 用户价值

让日报 App 能自动读取本地 AI 编程工具(Claude Code 等)在「当天或指定某一天」产生的对话记录,经字段级过滤压缩后作为生成日报的输入来源;前端可勾选要采集哪些工具。免去用户手动回忆/撰写「今天做了什么」。

## Confirmed Facts(已确认,无需再问)

### 数据源
- **Claude Code(已实测)**:对话存于 `~/.claude/projects/<编码项目路径>/*.jsonl`,每行一个事件 JSON。
  - 字段:`type`(user/assistant)、`message{role, content}`、`timestamp`(ISO8601, **UTC, 结尾 Z**)、`cwd`、`sessionId`、`gitBranch`、`version`。
  - `content`:user 多为字符串;assistant 为数组,块类型含 `thinking`/`text`/`tool_use`(带 `name`+`input`)/`tool_result`。
- **OpenCode(已探查)**:存储是 SQLite(`~/.local/share/opencode/opencode.db` + WAL),**非纯 JSON 文件**(修正前期判断);本机最近活动在 6 月。开源可查 schema,适配成本中等(需读 SQLite)。
- **Codex(已探查)**:本机无 `~/.codex` 数据 → 排除。
- **Cursor(已探查)**:本机有 15 个 workspace,但 `state.vscdb` 最近修改停留在 2025-06(逾年未用);SQLite + JSON blob、易碎 → 不进 MVP。

### Token 经济性(已实测 2026-07-09 约 2 小时用量)
- 不压缩 ≈ 381,120 token;**策略①字段级过滤后 ≈ 39,770 token(压到 10.4%)**。
- 过滤规则:保留 user 文本 + assistant 文本 + `tool_use` 的 name+关键参数(file_path 等);丢弃 `tool_result` 全文 + `thinking`。
- 结论:过滤后单次请求即可,任何主流模型窗口都装得下 → **MVP 不需要 Map-Reduce**;②仅作为重度日的可选增强(map 交本地 Ollama)。

### 硬性技术契约
1. **按每行 timestamp 过滤当天,绝不按文件 mtime**(session 跨天累积;实测最大文件 1.74MB 但当日仅 499 行)。
2. **timestamp 为 UTC,过滤「当天」必须先转本地时区再比 date**(中国 UTC+8:本地当日 = UTC 前一日 16:00 ~ 当日 16:00)。

### 现有架构(改动锚点)
- 链路:`手写 input → render_template({{input}},{{date}}) → Rust 流式调 OpenAI 兼容 API → 写历史`。
- `AppConfig = { apiConfig, promptTemplate, exportDir, history }`(`src-tauri/src/config.rs`)。
- `generate_report(app, input, on_event)`、`render_template(template, input, date)`(`src-tauri/src/llm.rs`)。
- 模板仅 `{{date}}`/`{{input}}` 两变量(`src/lib/template.ts`)。
- 设置页 A/B/C 三段:API / 模板 / 导出(`src/routes/settings/+page.svelte`)。
- 前端 config store(`src/lib/store.ts`)。

## Requirements(随收敛细化)

- **R1 采集命令(Rust)**:新增 command,入参 = 日期(默认今天)+ 勾选工具列表;出参 = 过滤后结构化对话文本。实现策略①过滤、按行 timestamp 过滤、UTC→本地时区换算、跨平台路径定位。
- **R2 前端采集入口**:工具勾选 + 日期 + 采集按钮 + 结果预览,接入生成链路。
- **R3 配置持久化**:工具勾选等偏好存入 AppConfig。

## Decisions

- **A ✅ 工具范围**:MVP 仅 Claude Code(JSONL、已实测)。采集器抽象为可插拔 trait(`Collector`);OpenCode(SQLite)/ Cursor(SQLite+blob)作为后续迭代各加实现,本次不写。
- **C ✅ 采集与输入**:新增模板变量 `{{conversations}}`,与手写 `{{input}}` 并存(手写可留空 → 纯对话生成)。影响项:`render_template` 增 conversations 参数;`generate_report` 增 conversations 参数;默认模板增 `{{conversations}}` 段;主页新增「采集区」(工具勾选 + 日期 + 采集按钮 + 只读预览)。(D 预览随之确定为:是,只读预览。)

### 工程默认(可否决,审阅时提出即可)

- **B 日期**:支持任意一天,默认今天(主页日期选择器;命令入参带 date)。
- **采集范围**:跨 Claude Code **所有 project 目录**(符合「当天所有对话」)。暂不做 project 级筛选(后续增强)。
- **F 排序**:按时间升序、按 session 分组,每组小标题标注 project 名 / cwd / 起止时间。
- **E 脱敏**:MVP 不做自动脱敏。理由:本地桌面 app + 用户自有数据 + 自配 API;策略①已丢弃 `tool_result`(敏感信息主要载体)。留作后续增强(可选开关)。
- **预览能力**:只读 + 显示 token 估算 + 各 session 条数;暂不支持勾选排除某 session(后续增强)。
- **空结果**:当天无对话时采集返回空、前端提示,不进入生成。

## Acceptance Criteria

- [ ] 选「Claude Code + 今天」采集,正确返回今天(本地时区)该工具所有 project 的对话,不含历史天数。
- [ ] 采集文本经过策略①过滤,token 显著小于原始,可在预览中查看。
- [ ] 采集 → 生成,产出符合现有模板格式的日报。
- [ ] 跨天 session 不污染当天结果。
- [ ] 时区正确:本地凌晨 0-8 点事件归入「当天」(其 UTC 在前一天)。

## Out of Scope(MVP)

- Map-Reduce / 本地 Ollama 摘要(留作重度日增强)。
- Cursor 适配(易碎,后续单独迭代)。

## Open Questions

(已全部收敛到上方 Decisions;无阻塞项。审阅时可否决任何「工程默认」。)
