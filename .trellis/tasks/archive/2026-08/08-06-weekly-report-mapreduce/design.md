# Design — 周报生成(区间采集 + map-reduce)

> 配套 `prd.md`。本文聚焦边界、数据流、契约、兼容性与权衡。执行清单见 `implement.md`。

## 1. 架构总览

周报 = **区间采集(同步 IO,无 LLM)** + **map-reduce 生成(多轮 LLM,流式)** 两步,镜像日报的
"采集→生成"两步解耦,新增独立命令与路由,不动日报路径。

```
/weekly 页面
  │ ① 采集区间(预览,不耗 token)
  ├─► collect_conversations_range(start,end,tools,filter,tool_paths)
  │     └─ spawn_blocking: 各 collector.collect(DateRange) → 按本地日分组 →
  │        RangeCollectResult{ days:[DayCollect{date,sessions,renderedText,estTokens}], totalTokens }
  │     ◄─ 前端预览:每日会话/token + 总 token(超阈值告警)
  │
  │ ② 生成周报(map-reduce,流式 + 进度)
  └─► generate_weekly_report(start,end,tools,filter,tool_paths,weekly_input,on_event)
        ├─ spawn_blocking: 区间采集 → 按天分组(复用①逻辑)
        ├─ map: for each day → LLM 摘要(重试3次指数退避;失败→跳过+记 missing_days)
        │        每天发 progress{stage:"map",current,total}
        ├─ reduce: 跨天归纳整周周报(重试3次;失败→Err)
        │        发 progress{stage:"reduce"} → 流式 delta(可见正文) → done
        └─ 落库 HistoryItem(title="8.4–8.10周报",date=区间串) 返回
```

> **为何 generate 命令内部重新采集而非复用①返回的文本**:让 Rust 端拥有"采集→map→reduce"端到端
> 数据流,进度/重试/分组自洽,且不把大段对话文本经 IPC 再回传一遍(与日报把 conversations 字符串
> 传入的区别:周报文本量大得多)。collect 命令纯粹为"生成前预览/预算 token"。两次读本地 jsonl 廉价,
> 可接受。

## 2. 后端契约(Rust)

### 2.1 采集器 trait 泛化(硬契约变更,同步 collector-spec.md)

```rust
// collector/mod.rs
#[derive(Debug, Clone, Copy)]
pub struct DateRange { pub start: NaiveDate, pub end: NaiveDate }
impl DateRange {
    pub fn single(d: NaiveDate) -> Self { Self { start: d, end: d } }
    pub fn contains(&self, d: NaiveDate) -> bool { d >= self.start && d <= self.end }
}

pub trait Collector: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn default_path(&self) -> Option<PathBuf>;
    fn collect(&self, range: DateRange, filter: &PathFilter, custom_path: Option<&str>)
        -> Result<(Vec<SessionDigest>, usize), String>;   // date → range
}
```

- jsonl 类(codex.rs:177 / claude_code.rs):`local.date_naive() != date` → `!range.contains(local.date_naive())`。
- SQLite 类(zcode.rs:196 `date_matches` / `build_day_lines`):`== date` → `range.contains(...)`;签名
  由 `(ms, date)` 改为 `(ms, range)`(opencode 复用)。
- `collect_blocking` 接收 `DateRange`;日报命令 `collect_conversations` 仍收 `date:String`,内部
  `DateRange::single(parse_target_date(date))` → **日报外部签名零变更、零回归**。
- 过滤点不变(parse 之后、push 之前做路径过滤);仅日期谓词从相等改为区间包含。

### 2.2 新命令 ① 区间采集

```rust
#[derive(Serialize)] #[serde(rename_all="camelCase")]
pub struct DayCollect { pub date: String, pub sessions: Vec<SessionDigest>,
    pub rendered_text: String, pub est_tokens: usize }
#[derive(Serialize)] #[serde(rename_all="camelCase")]
pub struct RangeCollectResult { pub days: Vec<DayCollect>, pub total_tokens: usize,
    pub skipped_lines: usize }

#[tauri::command]
pub async fn collect_conversations_range(
    start: String, end: String, tools: Vec<String>,
    filter: PathFilterParam, tool_paths: HashMap<String, String>,
) -> Result<RangeCollectResult, String>
```
- `spawn_blocking`:对区间内每个本地日 D,用 `DateRange::single(D)` 跑各 collector?— 不:更高效是
  一次 `collect_blocking(DateRange{start,end}, …)` 拿到全部 sessions,再**按 session 行的本地日期分组**
  到各 DayCollect。但 `render()` 目前产出整段文本;需新增**按日渲染**:`render_day(sessions_of_day)`
  复用 `render` 的行格式。分组键 = session 内行的本地日期(session 可能跨天,按行归属日)。
  → 实现取舍:逐日 single-range 采集更简单(直接复用 collect_blocking,每日独立 render),代价是
  重复遍历目录 N 次。MVP 选**逐日 single-range 采集**(简单、与现有 render 零改动),性能后续优化。
  (见 §6 权衡 A)

### 2.3 新命令 ② map-reduce 生成

```rust
#[tauri::command]
pub async fn generate_weekly_report(
    app: AppHandle, start: String, end: String, tools: Vec<String>,
    filter: PathFilterParam, tool_paths: HashMap<String, String>,
    weekly_input: String, on_event: Channel<StreamChunk>,
) -> Result<HistoryItem, String>
```
内部:
1. `load_config` 取 `api_config`;校验 baseUrl/key/model(同 `generate_stream`:52)。
2. spawn_blocking 区间采集→按日分组(同 §2.2);记 `days: Vec<(date_str, conv_text)>`。
3. **map**:`for (i,(date,conv)) in days.enumerate()`:
   - `on_event.send(progress{stage:"map", current:i+1, total:days.len(), message:format!("摘要 {date}")})`
   - `summary = with_retry(3, || complete_once(&api, &map_prompt(date,&conv))).await`;失败→`missing_days.push(date)`,
     `on_event.send(progress{…, message:format!("跳过 {date}(失败)")})`,continue。
   - map 提示词(硬编码常量):`{{date}}` + `{{conversations}}`;要求结构化当日摘要、限长。
4. **reduce**:`on_event.send(progress{stage:"reduce", current:1, total:1, message:"汇总"})`;
   - 用 reduce 提示词(硬编码):`{{date_range}}` + `{{weekly_input}}`(可选)+ `{{day_summaries}}`
     (各日摘要拼接,缺失天显式标注"⚠️ {date}:采集/摘要失败,已跳过")。
   - `final = with_retry_stream(3, || generate_stream_to_channel(&api,&reduce_prompt,&on_event)).await`;
     失败→`on_event.send(error)` + `return Err`。
5. 落库:`HistoryItem{ title:"{m.d_start}–{m.d_end}周报", date:"{start}~{end}", input:weekly_input,
   output:final, … }`,`insert_history` 返回。

### 2.4 StreamChunk 扩展(跨层同步)

```rust
// llm.rs
#[derive(Serialize, Clone)] #[serde(tag="type", rename_all="lowercase")]
pub enum StreamChunk {
    Delta { text: String },
    Done,
    Error { message: String },
    Progress { stage: String, current: usize, total: usize, message: String },  // 新增
}
```
- map 用非流式 `complete_once`(消费完整响应,不回显);reduce 复用现有流式逻辑(`generate_stream`
  改造:抽 `stream_into(api,prompt,channel)` 复用于 reduce;日报 `generate_report` 不变)。
- `with_retry`(map,非流式)与 `with_retry_stream`(reduce,流式)共享指数退避 `1s,2s,4s`(`tokio::time::sleep`)。

### 2.5 模板(可配置,镜像日报模板机制)
- `AppConfig` 加 `weekly_map_template`/`weekly_reduce_template`(`#[serde(default)]`,config.rs:71);
  `db.rs` get_config/set_config 增 KV `weekly_map_template`/`weekly_reduce_template`(同 `prompt_template`
  模式,见 db.rs:89/117);`migrate_from_store` 与 `LegacyAppConfig`(db.rs:204)同步加字段并填默认
  (保证迁移编译 + 旧 store 数据兼容)。
- **默认值双源**:`src/lib/template.ts` 加 `DEFAULT_WEEKLY_MAP_TEMPLATE`/`DEFAULT_WEEKLY_REDUCE_TEMPLATE`
  (UI 展示 + 「恢复默认」);Rust 内嵌同名兜底常量,`generate_weekly_report` 取 `cfg.weekly_*_template`,
  **空则回退内嵌默认**(避免用户未保存设置即生成 → 空提示,优于日报当前行为)。
- **变量**(Rust 侧 replace 注入):map 用 `{{date}}`、`{{conversations}}`;reduce 用 `{{date_range}}`、
  `{{input}}`(本周补充要点,与日报 `{{input}}` 命名一致)、`{{day_summaries}}`(各日摘要拼接,缺失天标注
  `⚠️ {date}:摘要失败,已跳过`)。

## 3. 前端契约

### 3.1 bindings.ts(同步)
- 新增 `DateRange`、`DayCollect`、`RangeCollectResult` interface。
- `AppConfig` 加 `weeklyMapTemplate`/`weeklyReduceTemplate`;`emptyConfig()` 同步(初值 `""`)。
- `StreamChunk` 加 `{ type:"progress"; stage:"map"|"reduce"; current:number; total:number; message:string }`。
- `collectConversationsRange(start,end,tools,filter,toolPaths)`、
  `generateWeeklyReport(start,end,tools,filter,toolPaths,weeklyInput,onMessage)` wrapper。

### 3.2 路由 /weekly(`src/routes/weekly/+page.svelte`)
- **输入面板**:开始/结束 `<input type="date">`(默认本周一~今天)、"采集区间"按钮、每日预览
  (会话数/token + 总 token + 超阈值告警)、可选"本周补充要点"`<textarea>`、"生成周报"按钮。
- **进度区**:`progress` chunk → 步骤文案 + 进度条(current/total)。
- **输出面板**:复用从 `/` 抽出的共享组件 `src/lib/components/ReportPanel.svelte`(编辑/预览/复制/导出)。
  抽取时把 `+page.svelte` 右侧 02 面板(output/mode/html/onCopy/onExport)参数化,日报与周报共用。

### 3.3 导航
- `+layout.svelte:11` nav 加 `{ href:"/weekly", label:"周报" }`。

### 3.4 设置页周报模板区(settings/+page.svelte)
- 镜像日报模板编辑机制(:16 template / :35-36 加载 / :98-112 setAsDefault+resetTemplate):新增
  "周报模板"区,两个 `<textarea>`——每日摘要模板(`weeklyMapTemplate`)、周报汇总模板(`weeklyReduceTemplate`),
  初值 `c.weeklyMapTemplate || DEFAULT_WEEKLY_MAP_TEMPLATE`(同 :35 的 `||` 回退模式);各带「恢复默认」。
- `save()`(:50)的 `merged` 增 `weeklyMapTemplate`/`weeklyReduceTemplate` 字段写入。

## 4. 兼容性 / 迁移
- **日报零回归**:`collect_conversations` 外部签名不变(仍 `date:String`,内部包单日区间);`generate_report`
  不变;日报页不改。
- **StreamChunk 加变体**:前后端同发布;TS 判别联合需补 `progress` case(否则 switch 不穷尽 →
  `pnpm check` 报错,正好强制同步)。
- **config schema 变更(新增两个周报模板字段)**:`#[serde(default)]` + `migrate_from_store` 填默认 → 旧
  配置升级在位、不丢数据;`db.rs` 的 `LegacyAppConfig`/`migrate_from_store`/`sample_config` 测试需同步加
  字段(编译失败 = 同步安全网);`config` 表 KV 新增 `weekly_map_template`/`weekly_reduce_template`
  (storage-spec.md 的 key 集合需同步)。
- **history 无 schema 变更**:title/date 复用为自由字符串。

## 5. 回滚形态
- 全部为新增(新命令、新路由、新组件、新变体)+ 采集器 trait 签名变更。回滚 = revert 该 PR;日报路径
  未触。trait 变更由 `cargo test`(含既有单日切片 + 新增区间切片)把关。

## 6. 关键权衡
- **权衡 A — 区间采集逐日 vs 一次**:MVP 选逐日 single-range(直接复用 `collect_blocking`+`render`,
  零改动、分组天然),代价是重复遍历目录 N 次(N=天数,本地 IO 廉价)。一次采集+按行分组更快但需新写
  按日 render 与跨天 session 拆分逻辑,留作性能优化。
- **权衡 B — generate 内部重采 vs 接收前端文本**:选内部重采,Rust 端数据流自洽、免大文本 IPC 回传;
  代价是读两遍 jsonl(廉价)。
- **权衡 C — map 非流式 / reduce 流式**:map 产物是中间摘要不回显,非流式更简单且重试干净;reduce 是
  可见正文,流式给用户即时反馈。
- **权衡 D — 跳过 vs 中止**:用户选定"重试3次→跳过+标注"(map),reduce 不可跳过故重试3次后报错。
