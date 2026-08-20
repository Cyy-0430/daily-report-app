# Implement — 日报/周报公共组件抽取 + 页面状态跨路由保留

前置阅读顺序:`prd.md` → `design.md` → `.trellis/spec/frontend/component-guidelines.md`。
改动全部在前端;**禁止**改 `src-tauri/**`、`src/lib/bindings.ts` 的接口签名。

## 步骤

### Step 1 状态模块 + 纯函数

- [ ] 新建 `src/lib/report-state.svelte.ts`:导出 `daily` / `weekly` 两个 `$state` 对象(字段见 design D1,含 `mode`),`WeeklyProgress` 类型用 `Extract<StreamChunk, { type: 'progress' }>`。
- [ ] 同文件导出纯函数 `enabledToolIdsOf / sourceLabelOf / buildFilter / todayStr / mondayStr`(从现两页脚本原样搬运,单一实现)。
- 验证:`pnpm check` 通过(`.svelte.ts` 的 `$state` 编译无导出重赋值错误)。

### Step 2 共享样式 + InputPanel

- [ ] 新建 `src/lib/components/report/report-shared.css`:`.editor-grid / .collect-bar / .collect-src / .collect-date / .collect-meta`(以日报页现值为准,周报页值与之相同)。
- [ ] 新建 `src/lib/components/report/InputPanel.svelte`:props `label / value($bindable) / placeholder / generateLabel / busy / disabled / ongenerate`,snippet `head` / `extra`;scoped 样式收纳 `.panel / .editor-textarea / .panel-foot / .meta / .arrow` 与 head flex 布局(design D2)。
- 验证:`pnpm check` 0 警告。

### Step 3 日报页薄壳化

- [ ] 改写 `src/routes/+page.svelte`:状态读写全部指向 `daily`;derived 用纯函数一行包装;`head` snippet 放日期 + 清空(清空改清 `daily` 各字段);`extra` snippet 放采集条 + 预览;import `report-shared.css`;删被收敛的 scoped CSS(仅留 `.collect-preview`)。
- [ ] `onMount` 的 `pendingInput` 回填逻辑保留,写入 `daily.input`。
- [ ] `ReportPanel` 增加 `mode = $bindable('preview')`,日报页绑定 `daily.mode`。

### Step 4 周报页薄壳化

- [ ] 同 Step 3 改写 `src/routes/weekly/+page.svelte`:状态指向 `weekly`;`head` snippet 放区间选择 + 清空;`extra` snippet 放采集条 + 日列表 + 进度条;scoped CSS 仅留周报独有类。
- [ ] `ReportPanel` 绑定 `weekly.mode`;`exportName` 逻辑不变。

### Step 5 TemplateEditor

- [ ] 新建 `src/lib/components/settings/TemplateEditor.svelte`(design D4:props `title / variant / value($bindable) / configKey / builtinDefault` + `help` snippet;内部 2 个函数 setAsDefault/reset)。
- [ ] 精简 `PromptTab.svelte`:三段编辑区换为 3×TemplateEditor,保留两个 `.panel.sec` 外壳与「周报模板」总标题;`.tmpl` 相关样式迁入 TemplateEditor;顶部三个 `$bindable` prop 原样保留。
- 验证:`pnpm check` 0 警告。

### Step 6 质量检查(最后一轮全量,trellis-check)

- [ ] `pnpm check`(0 error 0 警告)。
- [ ] `pnpm tauri dev` 冒烟清单:
  - 日报:采集 → 生成(流式)→ 编辑 → 切设置 → 切回:日期/要点/采集结果(含展开态)/正文/编辑态全在;生成中切走切回,输出继续增长;
  - 周报:区间采集 → 日列表/warn 提示 → 生成(map 进度条)→ 同上跨路由核验;
  - 设置-提示词:三模板编辑、设为默认、恢复默认(自定义默认→内置回退链)、页面保存,重进应用后持久;
  - 历史:复用回填日报输入;删除正常;
  - 四页导航高亮正确,清空按钮语义不变。
- [ ] 视觉核验:两页布局/间距/字体与重构前一致(重点:collect-bar 折行、panel-foot 对齐、设置页模板区间距 1.1rem)。

## 回滚点

每 Step 一个可编译状态;整体为单次提交,`git revert` 全量回滚。

## 审查门

- Step 2 后:InputPanel props/snippet 形状与两页差异对齐再继续;
- Step 5 后:PromptTab 行为等价(设为默认的即时持久化不等「保存」)再进 Step 6。
