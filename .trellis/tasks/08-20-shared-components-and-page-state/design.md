# Design — 日报/周报公共组件抽取 + 页面状态跨路由保留

纯前端重构:不改 Rust、不改 IPC、不改 `AppConfig` 字段。

## D1 页面状态:模块级 runes(`src/lib/report-state.svelte.ts`)

新建 `src/lib/report-state.svelte.ts`,导出两个模块级 `$state` 对象(应用生命周期单例):

```ts
import type { CollectResult, RangeCollectResult, StreamChunk } from './bindings';

export const daily = $state({
  collectDate: todayStr(),          // 模块加载(应用启动)时初始化一次
  input: '',
  output: '',
  busy: false,
  collecting: false,
  collectResult: null as CollectResult | null,
  showConversations: false,
  mode: 'preview' as 'edit' | 'preview',   // ReportPanel 的模式也随页保留
});

export type WeeklyProgress = Extract<StreamChunk, { type: 'progress' }>;

export const weekly = $state({
  startDate: mondayStr(),
  endDate: todayStr(),
  weeklyInput: '',
  output: '',
  busy: false,
  collecting: false,
  progress: null as WeeklyProgress | null,
  rangeResult: null as RangeCollectResult | null,
  mode: 'preview' as 'edit' | 'preview',
});
```

- **为什么可行**:SvelteKit SPA 客户端导航不重新执行模块 → 状态跨路由存活;深层 proxy 使 `bind:value={daily.input}` 直接可用。
- **注意**:`.svelte.ts` 里不能导出可重赋值的 `$state` `let`(编译错误),只能导出 `const` 对象、改其属性 —— 本设计天然满足。
- **日期初始化时机变化**(有意):原来每次挂载页面取「今天」,现在应用启动时取一次;隔夜不自动跳新一天,但用户手选的日期不再被重置 —— 这正是 R3 要的行为。
- 生成中切走:异步闭包继续写模块状态,回到页面即见累计输出(R3.2,无需额外机制)。
- `onCollect` / `onGenerate` 等编排仍留在路由页(spec:页面负责编排),只是读写模块状态对象。
- 历史「复用」回填 `pendingInput`:保留日报页 onMount 读取逻辑,写入 `daily.input`。

同文件放两页共享的**纯函数**(参数传入,不依赖组件上下文;页面用一行 `$derived` 包装保持响应性):

```ts
export function enabledToolIdsOf(cfg: AppConfig): string[];   // 空 → DEFAULT_TOOL_IDS
export function sourceLabelOf(ids: string[]): string;         // id→label 逗号连接
export function buildFilter(cfg: AppConfig): PathFilter;      // include/exclude 缺省空数组
export function todayStr(): string;                           // YYYY-MM-DD
export function mondayStr(): string;                          // 本周一
```

`apiReady` 检查两页写法略异(周报已抽 `apiReady` derived),统一为一行 `$derived` 留在页面即可(一行不值得再抽)。

## D2 左面板公共组件:`src/lib/components/report/InputPanel.svelte`

与右侧已共享的 `ReportPanel` 对称的「输入面板」。两页真正相同的骨架:外层 `section.panel` + `panel-head`(label + 右侧操作区) + 中部扩展区 + textarea + `panel-foot`(字数 + 生成按钮)。差异(单日期/区间选择、采集条、日列表、进度条)用 **snippet** 注入。

```svelte
<InputPanel
  label="01 — 今日要点"
  bind:value={daily.input}
  placeholder="…"
  generateLabel="生成日报"
  {busy}
  disabled={busy || collecting}          // 生成按钮禁用条件(周报多一个 collecting)
  ongenerate={onGenerate}
>
  {#snippet head()}                       <!-- head 右区:日期(区间) + 清空 -->
  {#snippet extra()}                      <!-- collect-bar / 预览 / 日列表 / 进度 -->
</InputPanel>
```

- **snippet 编译作用域**(spec 已有教训):`head`/`extra` 里的类(`.collect-bar`、`.collect-date`…)按页面作用域编译 → 这些类的 CSS 不能 scoped 进 InputPanel,放共享 CSS(见 D3)。
- InputPanel 自身 scoped:`.panel`、`.editor-textarea`、`.panel-foot`、`.meta`、`.arrow`、head 的 flex 布局(`margin-left:auto` 由 `head` snippet 内元素承担,类放共享 CSS)。
- `清空`按钮行为各页不同(清的字段集不同)→ 留在 `head` snippet 里,由页面实现。

## D3 共享样式:`src/lib/components/report/report-shared.css`

仿 `settings-shared.css` 模式,两页各 import 一次(Vite 去重),收纳两页逐字重复的类:

`.editor-grid`、`.collect-bar`、`.collect-src`、`.collect-date`、`.collect-meta`

各页 `<style>` 只留本页独有:日报 `.collect-preview`;周报 `.range-pick`、`.sep`、`.day-list`、`.day-row`、`.warn-note`、`.progress-*`。

按 spec 约定:这些类名仅限报告编辑器页面族使用;页面 `.panel:first-child .panel-head` 覆盖如仍两页相同也收入共享文件。

## D4 设置页模板组件:`src/lib/components/settings/TemplateEditor.svelte`

三段同构编辑区抽为一个组件:

```svelte
<TemplateEditor
  title="日报模板"
  variant="sec"                        // 'sec'=.sec-title | 'sub'=.sub-title
  bind:value={template}
  configKey="customDefaultTemplate"    // 'customDefaultTemplate' | 'weeklyDefaultMapTemplate' | 'weeklyDefaultReduceTemplate'
  builtinDefault={DEFAULT_PROMPT_TEMPLATE}
>
  {#snippet help()}…<code class="var">{TPL_DATE}</code>…{/snippet}
</TemplateEditor>
```

组件内实现(替代 PromptTab 现 9 个函数,收敛为 2 个):

- `onMount`:loadConfig → `customDefault = c[configKey] || ''`(三个实例各自加载,SQLite 本地读,代价可忽略);
- `setAsDefault()`:loadConfig → `cur[configKey] = value` → saveConfig → 更新本地 `customDefault` → notify;
- `reset()`:`value = customDefault || builtinDefault`(回退链不变)。

PromptTab 退化为组合层:两个 `.panel.sec` 外壳(日报一个;周报一个,内含「周报模板」总标题 + 两个 sub TemplateEditor)。

- **样式归属迁移**:`.tmpl` 高度与 `.tmpl + .sec-title-row` 的 1.1rem 呼吸间距跟随 textarea/sec-title-row 一起进入 TemplateEditor 的 scoped 样式(相邻两个 TemplateEditor 实例的选择器仍命中 —— 同组件 scoped hash 一致)。
- **help 走 snippet 而非字符串**:内容含 `<code class="var">`,按 spec 该类在全局 CSS,snippet 内容按父作用域编译,天然正确。
- 页面层 `bind:template / bind:weeklyMap / bind:weeklyReduce` 三条绑定链不变(切 tab 不丢、按页保存不变)。

## D5 `ReportPanel` 小改

`mode` 从组件局部 `$state` 改为可选 `$bindable('preview')`;两页绑定到各自模块状态(`daily.mode` / `weekly.mode`),使「正在编辑」状态跨路由保留。默认值不变,组件其余行为不动。

## D6 文件清单

| 动作 | 路径 |
|---|---|
| 新增 | `src/lib/report-state.svelte.ts`(状态 + 纯函数) |
| 新增 | `src/lib/components/report/InputPanel.svelte` |
| 新增 | `src/lib/components/report/report-shared.css` |
| 新增 | `src/lib/components/settings/TemplateEditor.svelte` |
| 改写 | `src/routes/+page.svelte`(薄壳化,状态接 `daily`) |
| 改写 | `src/routes/weekly/+page.svelte`(薄壳化,状态接 `weekly`) |
| 精简 | `src/lib/components/settings/PromptTab.svelte`(组合 3×TemplateEditor) |
| 小改 | `src/lib/components/ReportPanel.svelte`(mode 可绑定) |

## 权衡与放弃的方案

- **放弃:单页 tab 化**(去掉路由用 tab 切换)—— 保持 R3.4 路由结构,改动面小;模块级 `$state` 已满足需求。
- **放弃:把页面状态写进 `src/lib/store.ts` svelte stores** —— stores 与 runes 混用会让 `bind:` 变笨重;`.svelte.ts` 模块 runes 是 Svelte 5 惯用法,且 spec 的「stores 管 config/toast/history」职责不被侵入。
- **放弃:抽「通用 CollectBar 组件」** —— 日报(采集/查看切换+预览)与周报(区间采集+日列表)中部差异大,强抽会 prop 爆炸;snippet 注入 + 共享 CSS 已消除重复。
- **不做:状态持久化到磁盘/localStorage** —— 用户只要求会话内跨页保留;整页刷新重置属可接受现状,不在本任务扩散。

## 回滚

单次提交,纯前端,`git revert` 即可整体回滚;无数据/配置迁移。
