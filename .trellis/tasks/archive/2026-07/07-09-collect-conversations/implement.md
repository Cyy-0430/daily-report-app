# 执行计划:从本地 AI 工具对话自动采集生成日报

> 前置:本任务为复杂任务,`prd.md` + `design.md` + 本文件齐备后方可 `task.py start`。

## 依赖准备

- [ ] `src-tauri/Cargo.toml` 增 `dirs`、`glob`(确认版本)。
- [ ] 确认 `chrono` 可用 `Local` / `DateTime` / `TimeZone`(已在依赖中)。

## 后端(Rust)

- [ ] **B1** 新建 `src-tauri/src/collector/mod.rs`:定义 `Role`、`ConversationLine`、`SessionDigest`、`CollectResult`、`Collector` trait;实现 token 估算工具函数。
- [ ] **B2** 新建 `src-tauri/src/collector/claude_code.rs`:`ClaudeCodeCollector` —— 路径定位(`dirs::home_dir`)、glob jsonl、逐行解析、**时区过滤(硬契约)**、字段级过滤(策略①)、渲染为 `SessionDigest`。宽容解析 + skipped_lines 计数。
- [ ] **B3** 新增 `collect_conversations(date, tools)` command(`collector/mod.rs` 或 `llm.rs`),按 `tools` 路由到对应 Collector(MVP 仅 claude-code),返回 `CollectResult`。
- [ ] **B4** `llm.rs`:`render_template` 增 `conversations` 参数 + `{{conversations}}` 替换;`generate_report` 增 `conversations` 参数透传。
- [ ] **B5** `config.rs`:`CollectConfig` + `AppConfig.collect_config`(`#[serde(default)]`)。
- [ ] **B6** `lib.rs`:`mod collector;` + `invoke_handler!` 注册 `collect_conversations`。

## 前端(Svelte)

- [ ] **F1** 重新生成 `bindings.ts`(`pnpm check` 触发 `svelte-kit sync`),确认 `collectConversations`、`CollectResult`、新 `generateReport` 签名存在。
- [ ] **F2** `src/lib/template.ts`:默认模板增 `{{conversations}}` 段(参考素材,空时不影响生成)。
- [ ] **F3** 主页 `src/routes/+page.svelte`:新增「采集区」(工具勾选 + 日期 + 采集按钮 + 只读预览含 token 估算);采集结果存 state。
- [ ] **F4** 主页生成按钮:`generateReport({ input, conversations, onEvent })` 同步新签名。
- [ ] **F5** 设置页 `src/routes/settings/+page.svelte`:加「启用工具」勾选(写 `collectConfig.enabledTools`),或 MVP 直接在主页固定 Claude Code。

## 验证命令

```bash
cargo check --manifest-path src-tauri/Cargo.toml   # Rust 编译
pnpm check                                          # svelte-check 类型
pnpm tauri dev                                      # 手测
```

## 手测验收脚本(对照 prd Acceptance)

- [ ] **今天 + Claude Code**:采集 → 预览 token 在万级、内容含今天真实对话 → 生成日报格式正常。
- [ ] **昨天**:切换日期采集,不含今天、含昨天(验证按行 timestamp 过滤)。
- [ ] **跨天 session**:某跨天文件只出现其当天行,历史不混入。
- [ ] **时区**:本地凌晨 0-8 点事件归入当天(其 UTC 在前一天)。
- [ ] **空结果**:某无对话日采集 → 空提示,不进入生成。
- [ ] **向后兼容**:不点采集、手写 input 生成 → 行为同旧版;旧 `data.json` 能正常加载。

## 风险点 / 回滚

- `render_template` / `generate_report` 签名变更 → 用 codegraph 确认所有调用点(主页、历史复用)同步改。
- AppConfig 迁移 → `#[serde(default)]` 兜底;若异常可回滚到旧 data.json。
- 采集模块独立,故障可整体禁用而不影响现有生成流程。

## task.py start 前检查

- [ ] prd.md / design.md / implement.md 齐全且用户已审阅。
- [ ] 无阻塞 Open Question。
