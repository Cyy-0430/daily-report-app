# 周报生成(区间采集 + map-reduce 摘要)

## Goal / 用户价值
用户选择一个时间区间(如本周一~周五),自动读取区间内已启用采集工具(Claude Code / ZCode /
Codex / Opencode)的本地对话记录,**按天分批摘要(map)→ 跨天归纳成整周周报(reduce)**,流式输出、
可编辑/复制/导出、落库历史。解决"日报好做、跨天周报因对话总量过大无法一次喂给 LLM"的核心矛盾
(单条 user 消息会撞上下文窗口或 120s 超时,见 `llm.rs:65-72`)。

## Background(已确认事实,来自代码勘察)
- **采集与生成两步解耦**:`onCollect`(src/routes/+page.svelte:61) 调 `collectConversations` 得
  `CollectResult{ sessions, renderedText, estTokens, skippedLines }`;`onGenerate`(:86) 把
  `renderedText` 作为 `conversations` 传入 `generateReport`(src/lib/bindings.ts:135)。→
  map-reduce 可在"采集→生成"之间自然插入,不重写日报流程。
- **前端已有单日选择器** `collectDate`(默认今天、可改任意日,:23,:72),采集不限于今天。
- **采集当前按单个 NaiveDate 过滤**:jsonl 类 `parse_session` 用 `local.date_naive() != date`
  (codex.rs:177 / claude_code.rs 同构);SQLite 类 `date_matches`(zcode.rs:196)/`build_day_lines`
  用 `t.date_naive() == date`。泛化成区间需动 4 个 collector + 纯函数 + collector-spec.md + 测试 +
  bindings.ts(跨层同步硬契约,见 CLAUDE.md)。
- **`{{date}}` 硬编码为今天**:`generate_stream`(llm.rs:60-62) 用 `now.month()/day()`。周报需
  区间串(如 `8.4–8.10`)。`render_template`(llm.rs:20) 已接受 `date_md` 参数。
- **默认模板是日报专用**(src/lib/template.ts)。周报需独立 map/reduce 双提示词。
- **HistoryItem.title 写死** `"M.D日报"`(llm.rs:179);周报需区分。history/export 流程可复用。
- **token 估算已存在**(`estimate_tokens`/`CollectResult.estTokens`),可预警,但当前无截断/分批机制。
- **IPC 单一注册面**:`lib.rs:41` `generate_handler!`;**nav 数组**:`+layout.svelte:11`。

## Key Decisions(已确认)
1. **map 批次粒度 = 按天切**:每天 = 一块 map 摘要("当日工作提炼");块数 = 天数(一周 5~7 块)。
2. **周报产物形态 = 整周凝练**:reduce 把 N 个日摘要跨天归纳成主题式周报(本周工作事项/遇到问题/
   总结,合并同类项);日摘要为原料而非可见分节。
3. **周报入口 = 新路由 /weekly**:route-per-feature;日报页零改动零回归;输出/导出面板抽成共享组件
   两边复用;nav 加一项。
4. **保留可选"本周补充要点"手写输入**:/weekly = 区间选择 + 采集 + 可选要点框(留空=纯自动);reduce
   把要点连同日摘要一起归纳。
5. **失败与重试 = 重试 3 次(指数退避)→ 跳过并标注缺失**:map 某天失败重试 3 次(1s/2s/4s)仍失败 →
   跳过该天,在最终周报里标注哪些天因失败缺失;reduce 同样重试 3 次,仍失败 → 报错(reduce 是最终产出,
   不可跳过)。
6. **周报模板 = 可配置(接入 config + settings)**(用户推翻原"硬编码"自决):map/reduce 双提示词
   作为 `AppConfig` 新字段 `weeklyMapTemplate`/`weeklyReduceTemplate`(`#[serde(default)]`);默认值
   在 `src/lib/template.ts`(`DEFAULT_WEEKLY_MAP_TEMPLATE`/`DEFAULT_WEEKLY_REDUCE_TEMPLATE`)+ Rust 内嵌
   兜底常量(配置为空时回退,避免未保存即生成产生空提示)。settings 页加"周报模板"区,两个模板均可编辑
   (每日摘要模板 / 周报汇总模板),各带「恢复默认」。
7. **进度反馈 = Channel `progress` 变体 + 步骤文案**(自决):扩展现有 `StreamChunk`(delta/done/error)
   加 `progress{ stage:"map"|"reduce", current, total, message }`;map 阶段只发 progress(中间摘要不回显),
   reduce 阶段发 progress 后流式 delta(可见周报正文)。

## Requirements
- **R1 区间采集**:新增命令 `collect_conversations_range(start,end,tools,filter,tool_paths)`,把单日
  `NaiveDate` 过滤泛化为 `[start,end]` 区间;按本地日期分组,返回每日 `DayCollect{ date, sessions,
  renderedText, estTokens }` + 总 token。区间外行绝不串入。
- **R2 采集器 trait 泛化**:`Collector::collect(date)` → `collect(range: DateRange)`;单日 = 单日区间特例,
  日报命令 `collect_conversations` 外部签名不变(仍 `date:String`,内部包单日区间)→ 日报零回归。
- **R3 map-reduce 生成**:新增命令 `generate_weekly_report(start,end,tools,filter,tool_paths,
  weekly_input, on_event)`;Rust 端编排:区间采集→按天分组→每天一次 map 摘要(重试3次指数退避,失败
  跳过+记录缺失天)→reduce 跨天归纳整周周报(流式 delta,重试3次,失败报错)→落库 HistoryItem 返回。
- **R4 进度反馈**:`StreamChunk` 加 `progress` 变体;前端展示"正在摘要 8.4 (2/5)…/跳过 8.5(失败)/
  正在汇总…"。
- **R5 前端 /weekly**:区间选择(默认本周一~今天)→"采集区间"预览(每日会话/token + 总 token,超阈值
  预警)→可选"本周补充要点"→"生成周报"(进度+流式输出);输出/导出面板与日报共享组件。
- **R6 历史/导出复用**:周报 HistoryItem(title=`"8.4–8.10周报"`、date=区间串)走现有 add_history/list/
  remove/export,无 schema 变更。
- **R7 跨层同步**:collector-spec.md(trait 签名 + 日期过滤 `contains` + 区间测试)、bindings.ts(
  DateRange/RangeCollectResult/DayCollect/progress 变体/新命令 wrapper)同步更新。
- **R8 周报模板可配置**:`AppConfig` 加 `weeklyMapTemplate`/`weeklyReduceTemplate`(`#[serde(default)]`,
  空→Rust 内嵌默认兜底);`db.rs` get/set_config 增对应 KV + `migrate_from_store` 填充默认;`template.ts`
  加双默认常量;settings 页加"周报模板"编辑区(每日摘要 / 周报汇总,各带「恢复默认」);bindings.ts /
  emptyConfig 同步。变量:map 用 `{{date}}`/`{{conversations}}`;reduce 用 `{{date_range}}`/
  `{{input}}`(本周补充要点)/`{{day_summaries}}`。

## Acceptance Criteria
- [ ] AC1 选任意区间(含跨周、单日、多周)采集,结果只含区间内行;跨工具按日分组、总 token 正确。
- [ ] AC2 生成周报不撞上下文窗口(整周对话量大时由 map-reduce 兜住);最终流式输出完整整周凝练周报。
- [ ] AC3 某天 map 连续失败时,重试 3 次(指数退避)后跳过,周报正文标注缺失天;reduce 失败重试 3 次后
      报错并提示。
- [ ] AC4 生成期间进度文案正确反映 map 逐天进度与 reduce 阶段。
- [ ] AC5 "本周补充要点"非空时,其内容被纳入周报归纳;留空时纯自动,行为正确。
- [ ] AC6 日报功能零回归:单日采集、生成、history、导出与改动前一致。
- [ ] AC7 周报可编辑/复制/导出 .md,并以区分于日报的标题落库历史。
- [ ] AC8 `cargo test`(含新增区间切片测试)+ `pnpm check` 全绿;collector-spec.md / bindings.ts 同步。
- [ ] AC9 周报模板(map/reduce)可在设置页编辑、保存、恢复默认;旧配置无新字段时按默认回退、不报错
      (round-trip 兼容);reduce 模板的 `{{input}}`/`{{day_summaries}}`/`{{date_range}}` 正确注入。

## Out of Scope
- 单个 map 批(某天)仍超 token 的二次自动切分(MVP 靠路径过滤 + estTokens 预警;二次切分列为风险项)。
- 区间采集的并发化(逐天顺序采集,MVP 足够;性能优化后续)。

## Risks / Deferred
- **单日对话仍超 token**:某热门项目某天对话量大,单块 map 仍可能超模型上下文。MVP 缓解:采集预览
  estTokens 超阈值告警 + 引导用户收窄区间/用路径过滤。二次自动切分 deferred。
- **map-reduce 延迟**:N+1 次 LLM 调用 × 重试可能较慢;进度文案缓解感知延迟。
- **采集器 trait 签名变更是硬契约**:动 4 个 collector + spec + 测试,需同 PR 同步,由测试把关。
- **config schema 变更**(新增两个周报模板字段):靠 `#[serde(default)]` + `migrate_from_store` 填默认,
  旧配置升级在位、不丢数据;`db.rs` 的 `LegacyAppConfig`/`migrate_from_store`/`sample_config` 测试需同步加字段
  (否则编译失败,作为同步安全网)。
