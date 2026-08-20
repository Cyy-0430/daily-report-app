# 日报/周报公共组件抽取 + 页面状态跨路由保留

## Goal

消除日报页与周报页之间、以及设置页提示词 tab 内部的重复代码;并让用户在日报/周报页与设置页(及历史页)之间切换时,页面工作内容不丢失。

## Background

- `src/routes/+page.svelte`(日报)与 `src/routes/weekly/+page.svelte`(周报)存在大段逐字重复:
  - 脚本:`enabledToolIds` / `collectSourceLabel` 两个 derived、`todayStr/fmt` 日期工具、构造路径 filter、API 就绪检查,各写一份。
  - 样式:`.editor-grid / .panel / .collect-bar / .collect-src / .collect-date / .collect-meta / .editor-textarea / .meta / .arrow` 约 100 行 CSS 两页几乎相同(周报页注释自述「与日报页一致」)。
- `src/lib/components/settings/PromptTab.svelte` 内三段模板编辑区(日报模板 / 每日摘要模板 / 整周汇总模板)结构完全相同(标题 + HelpTip + 设为默认/恢复默认 + textarea),9 个 set/reset 函数为三份复制。
- 日报/周报页所有状态是组件内 `$state`;SvelteKit 路由切换(`/` ↔ `/settings` ↔ `/history`)卸载页面组件,日期、要点、采集结果、生成/编辑中的报告全部丢失。

## Requirements

### R1 日报/周报公共代码抽取

- R1.1 两页重复的 derived / 工具函数 / filter 构造收敛为单一实现,两页复用。
- R1.2 两页重复的 CSS 收敛为单一来源(共享样式或组件),不留两份拷贝。
- R1.3 抽取后两页视觉与交互**零变化**(布局、间距、字数统计、按钮态、空态、预览/编辑切换等逐一保持)。

### R2 设置页提示词模板组件抽取

- R2.1 PromptTab 三段模板编辑区抽为一个可复用组件,差异(标题、帮助文案、绑定值、持久化字段、回退默认模板)经 props 传入。
- R2.2 「设为默认」(即时持久化 loadConfig→改→saveConfig)与「恢复默认」(自定义默认 → 内置默认回退链)行为保持不变。
- R2.3 与设置页其余 tab 的联动不变:正文双向绑定到页面 `$state`,切 tab 不丢未保存编辑,页面「保存」仍按页保存。

### R3 页面状态跨路由保留

- R3.1 从日报页或周报页切到设置/历史页再切回时,以下内容原样保留:
  - 日报:所选日期、左侧要点输入、采集结果(含展开/收起状态)、生成/编辑中的日报正文、生成中状态;
  - 周报:起止日期、区间采集结果、本周补充要点、生成/编辑中的周报正文、map/reduce 进度、生成中状态。
- R3.2 生成中切走再切回,流式输出继续增长,回到页面能看到已累计的内容(不中断、不重置)。
- R3.3 「清空」按钮语义不变:仍一次清空本页全部工作内容。
- R3.4 现有路由与导航结构不变(仍是 `/`、`/weekly`、`/settings`、`/history` 四路由,导航高亮逻辑不动)。

## Constraints

- 遵循 `.trellis/spec/frontend/component-guidelines.md`:Svelte 5 runes;路由页为薄壳;共享类名的 CSS 作用域约定;`pnpm check` 0 警告。
- 不改任何 Rust 代码与 IPC 接口(纯前端重构)。
- 不改配置结构(`AppConfig` 字段名不变,避免触发后端兼容规则)。

## Acceptance Criteria

- [ ] 日报/周报页脚本中不再存在两份 `enabledToolIds` / `collectSourceLabel` / `todayStr` 实现及重复 filter 构造;重复 CSS 只剩单一来源。
- [ ] PromptTab 中模板编辑区由单一组件渲染三份,三组 set/reset 函数收敛为一组。
- [ ] 手动核验:日报页生成内容 → 切设置 → 切回,日期/要点/采集结果/报告正文全部还在;周报页同理(含生成中切换,流式继续)。
- [ ] `pnpm check` 0 error 0 警告;`pnpm tauri dev` 冒烟:四页导航、日报采集+生成、周报区间采集+生成、设置页提示词三模板的设为默认/恢复默认/保存均正常。
