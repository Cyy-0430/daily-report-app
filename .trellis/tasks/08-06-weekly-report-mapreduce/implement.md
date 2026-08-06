# Implement — 周报生成(区间采集 + map-reduce)

> 执行顺序自上而下。每个 `[gate]` 是验证关,过了再进下一步。复杂任务;`prd.md`+`design.md` 为依据。
> 上下文加载走 inline 路径(`trellis-before-dev`),本任务不强制 implement.jsonl/check.jsonl 清单。

## 阶段 1 · 后端采集层(trait 泛化 + 区间采集命令)

- [ ] **1.1** `collector/mod.rs`:新增 `DateRange{start,end}` + `single()` + `contains()`;`Collector::collect`
      签名 `date: NaiveDate` → `range: DateRange`;`collect_blocking` 接收 `DateRange`;日报命令
      `collect_conversations` 内部 `DateRange::single(parse_target_date(date))`(外部签名不变)。
- [ ] **1.2** jsonl 类:`codex.rs:177` / `claude_code.rs` 的 `local.date_naive() != date` →
      `!range.contains(local.date_naive())`;`parse_session` 透传 `range`。
- [ ] **1.3** SQLite 类:`zcode.rs::date_matches`/`build_day_lines` 由 `(ms, date)` 改 `(ms, range)`
      (`== date` → `range.contains(...)`);`opencode.rs` 复用;`zcode.rs::collect`/`opencode.rs::collect`
      传 `range`。
- [ ] **1.4** 新增 `collect_conversations_range` 命令 + `DayCollect`/`RangeCollectResult`(`#[serde(rename_all="camelCase")]`)。
      逐日 single-range 采集(权衡 A):对区间每日跑 `collect_blocking(DateRange::single(d))`,聚合为 `days`。
- [ ] **1.5** `lib.rs:41` `generate_handler!` 注册 `collect_conversations_range`。
- [ ] **[gate 1]** `cargo test`(既有单日切片测试应全绿——单日即单日区间,行为等价)+ 新增区间切片测试
      (跨天 session 只留区间内日;区间外日不串入)。`cargo check` 通过。

## 阶段 2 · 后端 LLM 层(map-reduce + StreamChunk + 重试)

- [ ] **2.1** `llm.rs`:`StreamChunk` 加 `Progress{ stage, current, total, message }` 变体。
- [ ] **2.2** 抽非流式 `complete_once(api, prompt) -> Result<String,String>`(map 用);把现有流式逻辑抽
      `stream_into(api, prompt, channel)` 供 reduce 复用;`generate_report`/`generate_stream` 行为不变。
- [ ] **2.3** `with_retry(max=3, base=1s, 指数退避, f)`(map 非流式)与 `with_retry_stream`(reduce 流式)
      共享退避逻辑(`tokio::time::sleep`)。
- [ ] **2.4** `config.rs` 加 `weekly_map_template`/`weekly_reduce_template`(`#[serde(default)]`);Rust 内嵌
      兜底常量 `WEEKLY_MAP_PROMPT`/`WEEKLY_REDUCE_PROMPT`(空配置回退);`generate_weekly_report` 取 cfg 字段、
      空则回退常量。变量:map=`{{date}}`/`{{conversations}}`;reduce=`{{date_range}}`/`{{input}}`/`{{day_summaries}}`。
- [ ] **2.4b** `db.rs` get_config/set_config 增 KV `weekly_map_template`/`weekly_reduce_template`(同
      `prompt_template` 模式);`LegacyAppConfig`/`migrate_from_store`/`sample_config`(db.rs:204/321)同步加字段
      并填默认;更新迁移/CRUD 测试覆盖新字段 round-trip。
- [ ] **2.5** 新增 `generate_weekly_report` 命令:load_config→spawn_blocking 区间采集按日分组→map(逐日
      `complete_once` + `with_retry`,失败跳过+记 missing_days,发 progress)→reduce(`stream_into` +
      `with_retry_stream`,失败 Err,发 progress+delta)→落库 HistoryItem(title/date 区间串)→返回。
- [ ] **2.6** `lib.rs` 注册 `generate_weekly_report`。
- [ ] **[gate 2]** `cargo test` + `cargo check` 通过。map/reduce 编排可手动构造小 fixture 或 ignored 端到端测试。

## 阶段 3 · 跨层同步(bindings.ts)

- [ ] **3.1** `src/lib/bindings.ts`:加 `DateRange`/`DayCollect`/`RangeCollectResult`;`AppConfig` 加
      `weeklyMapTemplate`/`weeklyReduceTemplate`,`emptyConfig()` 同步(初值 `""`);`StreamChunk` 加
      `progress` case;加 `collectConversationsRange`、`generateWeeklyReport` wrapper(参数名与 Rust 命令
      一致:camelCase)。
- [ ] **3.2** `src/lib/template.ts` 加 `DEFAULT_WEEKLY_MAP_TEMPLATE`/`DEFAULT_WEEKLY_REDUCE_TEMPLATE`
      (含上述变量占位符,语义与 Rust 兜底常量一致)。
- [ ] **[gate 3]** `pnpm check` 通过(TS 判别联合补 progress case,否则不通过——强制同步)。

## 阶段 4 · 前端(/weekly 路由 + 共享组件 + 导航)

- [ ] **4.1** 抽共享组件 `src/lib/components/ReportPanel.svelte`:把 `src/routes/+page.svelte` 右侧 02 面板
      (output/mode/html/复制/导出)参数化;`/` 改为引用该组件(行为不变)。
- [ ] **4.2** 新建 `src/routes/weekly/+page.svelte`:开始/结束日期选择(默认本周一~今天)、"采集区间"→
      `collectConversationsRange` 预览(每日会话/token + 总 token + 超阈值告警)、可选"本周补充要点"
      `<textarea>`、"生成周报"→`generateWeeklyReport`(progress→步骤文案+进度条;delta→流式 output);
      输出面板用 `ReportPanel`。
- [ ] **4.3** `+layout.svelte:11` nav 加 `{ href:"/weekly", label:"周报" }`。
- [ ] **4.4** `settings/+page.svelte` 加"周报模板"区:两个 `<textarea>`(每日摘要/周报汇总,镜像日报模板
      机制 :16/:35-36/:98-112),各带「恢复默认」;onMount 加载 `weeklyMapTemplate||DEFAULT_WEEKLY_MAP_TEMPLATE`
      等;`save()` 的 `merged`(:50)增两字段写入。
- [ ] **[gate 4]** `pnpm check` 通过;`pnpm tauri dev` 手测:日报页零回归 + /weekly 走通采集预览→生成→
      编辑/复制/导出→历史可见。

## 阶段 5 · spec 同步 + 收尾
- [ ] **5.1** 更新 `.trellis/spec/backend/collector-spec.md`:trait 签名 `collect(range)`;日期过滤硬契约
      `== date` → `range.contains(date)`;日报命令仍单日(包单日区间);新增"区间采集"测试要求。更新
      `.trellis/spec/backend/storage-spec.md` 的 `config` key 集合(加 `weekly_map_template`/
      `weekly_reduce_template`)。
- [ ] **5.2** 全量验证:`cargo test` + `pnpm check` 全绿;手测 AC1–AC9(prd.md)。
- [ ] **[gate 5]** trellis-check 通过后进入 finish-work(commit)。

## 验证命令
```bash
cargo test            # src-tauri/ 内;含既有单日 + 新增区间切片
cargo check           # src-tauri/ 内
pnpm check            # 根目录;svelte-check 类型检查(强制 StreamChunk 同步)
pnpm tauri dev        # 端到端手测
```

## 风险点 / 回滚锚
- **采集器 trait 签名变更(阶段1)**:动 4 个 collector,是硬契约。回滚锚 = gate 1;若既有测试红,先修谓词
  再进。单日 = 单日区间,既有切片测试应零改动通过——若不通过说明区间语义实现有误。
- **StreamChunk 变体(阶段2→3)**:前后端必须同发布;`pnpm check` 会卡住未同步的 TS,作为安全网。
- **config schema 变更(阶段2.4/2.4b)**:新字段 `#[serde(default)]` + `migrate_from_store` 填默认;`db.rs`
  测试的 `sample_config`/`LegacyAppConfig` 必须加字段否则编译失败(同步安全网);旧配置升级在位不丢数据。
- **共享组件抽取(阶段4.1)**:先抽再用;确保 `/` 行为不变(gate 4 手测日报页)是回归防线。
- **超大单日 map**:MVP 不处理;采集预览 estTokens 超阈值告警 + 引导收窄区间/路径过滤。二次切分 deferred。
