# 执行计划:设置页拆分为多组件

前置:通读 `src/routes/settings/+page.svelte`(旧版,git 可随时对照)、`src/lib/components/ReportPanel.svelte`(组件惯例)、`src/app.css`(全局类)。

## 步骤

### 1. 基础件(无行为影响,先行)

- [x] 1.1 新建 `src/lib/components/settings/HelpTip.svelte`:迁移 `.help`/`.tip` 标记与全部样式(hover/focus-visible 显示、`z-index: 30`、`max-width: 300px` 等),children snippet 渲染正文。
- [x] 1.2 新建 `src/lib/components/settings/SettingsTabs.svelte`(`generics="T extends string"`,`active` $bindable),迁移 `.tabs`/`.tab` 样式(sticky、active 下划线)。(后续随横滑改造改为只读 `active` + `onselect` 回调,见文末「实现记录·二」)
- [x] 1.3 新建 `src/lib/components/settings/settings-shared.css`:`.sec`(overflow: visible / hover z-index: 20)、`.sec-title`、`.sec-title-row`、`.sec-actions-row`、`.sub-title`、`.fld`、`.fld-check`、`.fld > span`、`.var`。

### 2. 四个 tab 组件

- [x] 2.1 `ApiTab.svelte`:`bind:api`、`bind:exportDir`;局部 `showKey`/`testing`/`test()`/`pickDir()`;scoped `.grid-2`/`.row-input`;两段 section(用 HelpTip 替换原问号结构)。
- [x] 2.2 `PromptTab.svelte`:`bind:template`/`weeklyMap`/`weeklyReduce`;局部 `customDefault`/`weeklyDefMap`/`weeklyDefReduce`(own onMount `loadConfig`)+ 6 个默认值函数;scoped `.tmpl`。
- [x] 2.3 `CollectTab.svelte`:`bind:toolEnabled`/`includePaths`/`excludePaths`/`toolPaths`,只读 `defaultPaths`;局部 6 个路径增删选函数;scoped `.path-group`/`.path-row`/`.tool-path-row`/`.path-add`。
- [x] 2.4 `AboutTab.svelte`:`bind:autoCheckUpdate`;局部 `appVersion`/`checking`/`checkUpdateManual()`;scoped `.about-*`/`.meta-row`/`.link-btn`/`.star-*`/`.toggle-*`/`.switch`/`.knob`。

### 3. 页面重写

- [x] 3.1 重写 `src/routes/settings/+page.svelte`:保留 `SettingsTab`/`SETTINGS_TABS`/`activeTab`/持久化 `$state`/onMount 加载(去掉随组件下放的三项默认值字段)/5 个 save 函数/`dedupePaths`/`storedPath`;模板改为 `SettingsTabs` + 四个 tab 组件 + page-foot;`import '$lib/components/settings/settings-shared.css'`;`.page-scroll` 加 `scrollbar-gutter: stable`。
- [x] 3.2 tab 切换过渡(2026-08-19 需求变更:方向性横滑,取代初版「仅 in 纵向淡入上移」):`<div class="tab-panes">` 内 `{#key activeTab}` 包单个 `<div class="tab-pane">`(其内保留四分支 `{#if}`,组件用法与 bind 不变);`in:fly={{ x: direction * 64, duration: 220, easing: cubicOut }}` + `out:fly={{ x: direction * -64, duration: 220, easing: cubicOut }}`,方向由 `selectTab` 在改 `activeTab` 前按下标差设定(前进 1 / 后退 -1);`.tab-panes { display: grid }` / `.tab-pane { grid-area: 1 / 1 }` 新旧面板同格叠加(design §6,详见文末「实现记录·二」)。

### 4. 偏移修复

- [x] 4.1 `src/routes/history/+page.svelte` 的 `.page-scroll` 加 `scrollbar-gutter: stable`。

### 5. 验证(全量,最后一轮)

- [x] 5.1 `pnpm check` 零错误(svelte-check;本项目无 ESLint)。
- [x] 5.2 行为不变量逐条核验(prd.md 不变量 1–6):
  - 切 tab 改字→切回→编辑还在;只保存当前 tab 字段;
  - 设为默认立即生效、恢复默认回退链正确;
  - 保存按钮文案/禁用/分发;
  - tooltip 越界与提层;
  - aria/tabindex 齐全。
  (逐条结论见文末「实现记录」,为代码级核验 + `pnpm check`/`pnpm build` 通过;交互层面待 5.3 人工确认)
- [ ] 5.3 手动冒烟 `pnpm tauri dev`:四 tab 渲染、测试连接、采集勾选+路径选择、检查更新、切 tab/路由无横向跳动、切 tab 过渡动画流畅无闪烁。(需真人操作 GUI,留给用户/主会话执行)
- [x] 5.4 确认无残留:旧页面中已迁移的样式/标记在 `+page.svelte` 中不再出现;全仓 grep `.help`/`.tip` 只在 HelpTip。

## 验证命令

```bash
pnpm check        # 前端类型检查(唯一 lint)
# cargo 不涉及(纯前端改动)
pnpm tauri dev    # 手动冒烟
```

## 回滚点

- 全部改动单提交;异常时 `git revert <commit>` 整体回滚。
- 组件均为新增文件,页面重写可通过 git diff 对照旧版逐段还原。

## 完成定义

prd.md 验收标准全部勾选 + `pnpm check` 通过 + 冒烟通过。

---

## 实现记录(2026-08-19)

### 文件清单

新建:

- `src/lib/components/settings/HelpTip.svelte` — 圆形问号 + 悬浮气泡;children snippet 放提示正文,`.help`/`.tip` 样式 scoped 于本组件(`z-index: 30`、`max-width: 300px` 等逐条对照旧文件迁移)。
- `src/lib/components/settings/SettingsTabs.svelte` — `generics="T extends string"`,`active` `$bindable`;`.tabs`/`.tab` scoped。
- `src/lib/components/settings/settings-shared.css` — `.sec`(overflow: visible / hover z-index: 20)、`.sec-title`、`.sec-title-row`(含 `.sec-title-row .sec-title` / `.sec-title-row .sub-title` 复合规则)、`.sec-actions-row`、`.sec-actions`、`.sub-title`、`.fld`、`.fld-check`(含 `.fld-check input`)、`.fld > span`、`.var`。
- `src/lib/components/settings/ApiTab.svelte` — `bind:api`/`bind:exportDir`;局部 `showKey`/`testing`/`test()`/`pickDir()`;scoped `.grid-2`/`.row-input`。
- `src/lib/components/settings/PromptTab.svelte` — `bind:template`/`weeklyMap`/`weeklyReduce`;局部 `customDefault`/`weeklyDefMap`/`weeklyDefReduce`(own onMount `loadConfig`)+ 6 个设为默认/恢复默认函数;scoped `.tmpl`。
- `src/lib/components/settings/CollectTab.svelte` — `bind:toolEnabled`/`includePaths`/`excludePaths`/`toolPaths` + 只读 `defaultPaths`;局部 6 个路径增删选函数;scoped `.path-group`(含 `-label`、`+ .path-group`)/`.path-row`/`.tool-path-row`/`.path-add`。
- `src/lib/components/settings/AboutTab.svelte` — `bind:autoCheckUpdate`;局部 `appVersion`(own onMount `getVersion`)/`checking`/`checkUpdateManual()`;scoped `.about-*`/`.meta-row`/`.link-btn`/`.star-*`/`.toggle-*`/`.switch`/`.knob`。

修改:

- `src/routes/settings/+page.svelte` — 重写为薄壳(987 行 → 247 行):仅保留 tab 类型/常量、持久化 `$state`、onMount 加载、5 个 save 函数、`dedupePaths`/`storedPath`、页面骨架;导入 settings-shared.css;`.page-scroll` 加 `scrollbar-gutter: stable`;四个 `{#if}` 分支包 `<div class="tab-pane" in:fly={{ y: 8, duration: 180, easing: cubicOut }}>`(仅 in 无 out)。
- `src/routes/history/+page.svelte` — `.page-scroll` 加 `scrollbar-gutter: stable`(仅此一行)。

### 验证结果

- `pnpm check`(svelte-check):**352 files,0 errors,0 warnings**。
- `pnpm build`(vite + adapter-static):构建成功(额外确认 CSS import 与组件编译无问题)。
- 残留 grep:`.help`/`.tip` 标记与样式仅存在于 HelpTip.svelte;`.var` 样式仅在 settings-shared.css(snippet 内容按父作用域编译,符合 design §4);`.tabs`/`.tab` 仅在 SettingsTabs.svelte;`+page.svelte` 无任何 tab UI 标记。
- `pnpm tauri dev` 冒烟未执行(需真人操作 GUI),见 5.3。

### 行为不变量逐条核验(代码级)

1. 切 tab 未保存编辑不丢:持久化字段全部为页面层 `$state`,tab 组件经 `bind:` 读写;组件卸载只丢 UI 状态。保存仍按页 load→overlay→整份回写(5 个 save 函数逐行保留)。✔
2. 设为默认立即持久化:6 个函数随 PromptTab 迁移,逻辑逐行未改(loadConfig→改字段→saveConfig,不 `config.set`,与旧代码一致)。✔
3. 恢复默认回退链:`reset* = 自定义默认 || 内置默认模板`,`||` 链逐行保留。✔
4. 保存按钮:文案 `` `保存${activeTabLabel}` ``、`disabled={saving}`、`saveActive` 分发逻辑原样保留在页面。✔
5. tooltip:`.sec { overflow: visible }` 与 `.sec:hover { z-index: 20 }` 在 shared.css、`.tip { z-index: 30 }` 在 HelpTip,层级与覆盖关系不变。✔
6. 可达性:`.help` 的 `tabindex="0"`/`role="button"`/`aria-label`、开关的 `role="switch"`/`aria-checked`/`aria-label`、`aria-hidden` 等全部原样迁移。✔

### 与 design 的偏差

- `.sec-actions`(API tab 测试连接、关于 tab 检查更新两处共用的底部动作区)未列入 design §4 的 shared.css 清单,但被两个 tab 组件使用,无法 scoped 进单一组件,故放入 shared.css。design 清单本身不穷举(prd 只要求"跨 tab 共享样式"),此为必要补充。
- 无其它偏差:状态归属、组件契约、样式策略、过渡参数(180ms / y:8 / cubicOut / 仅 in)、`scrollbar-gutter: stable` 均按 design 执行。

---

## 质检记录(2026-08-19,检查代理独立核验)

### 逐项核验结论(1–8)

1. **`pnpm check` 独立复跑**:352 files,0 errors,0 warnings。
2. **改动面**:src/ 下仅两个声明文件被修改(`settings/+page.svelte` 重写为薄壳、`history/+page.svelte` 仅新增 `scrollbar-gutter: stable` 一行 + 注释)与 `src/lib/components/settings/` 新目录(6 组件 + 1 css);`git diff` 证实 `src/lib/bindings.ts`、`src/lib/template.ts`、`src/lib/store.ts`、`package.json`、`pnpm-lock.yaml`、`src-tauri/` 零改动,无新依赖。工作区另有 `.claude/`、`.trellis/scripts/`、`AGENTS.md` 等大量改动,为任务开始前已存在的 Trellis 平台升级内容,非本任务引入。
3. **行为不变量(逐函数 diff `git show HEAD` 旧版,非对照实现记录口述)**:
   - `saveApi`/`savePrompt`/`saveCollect`/`saveAbout`/`saveActive`/`dedupePaths`/`storedPath`:逐字节一致(仅提取造成的空行差异);
   - PromptTab 6 个设为默认/恢复默认函数:逐字节一致 —— loadConfig→改字段→saveConfig 即时持久化、无 `config.set`、`自定义 || 内置` 回退链保留;
   - ApiTab `test()`/`pickDir()`、CollectTab 6 个路径增删选函数:逐字节一致;
   - 持久化字段全在页面 `$state` 经 `bind:` 下传;`customDefault`/`weeklyDefMap`/`weeklyDefReduce`(own onMount 读取,赋值表达式与旧版一致)与 `appVersion` 按 design §2 归属 tab 局部;
   - page-foot 文案/`disabled={saving}`/`saveActive` 分发原样;HelpTip `tabindex="0"`/`role="button"`/`aria-label`、switch `role="switch"`/`aria-checked`/`aria-label`、`aria-hidden` 全量迁移。
4. **CSS 对账(脚本化逐规则比对旧版 61 条规则与新去向)**:全部迁移且声明值零改动,去向与 design §4 一致;唯一差异 = `.page-scroll` 新增 `scrollbar-gutter: stable`(PRD 要求);无凭空新增/重复规则。`.sec` 覆盖 app.css `.panel` 的 `overflow: hidden` 依赖注入顺序 —— 已从构建产物实证:app.css 在入口 `<link>`(chunk 0),设置页样式在路由 chunk 由 `entry/app` 动态后置注入,`.sec` 稳定胜出(dev 模块求值顺序同理,layout 先于 page)。
5. **Svelte 5 正确性**:`$bindable`(含兜底默认)用法正确;绑定对象/数组的元素级赋值(`bind:checked={toolEnabled[t.id]}`、`toolPaths[t.id] = …`)与整体重赋值(`excludePaths = [...]`)均为合法深层代理写法,父组件可见;HelpTip children snippet 正确且 `.var` 因父作用域编译留全局;SettingsTabs `generics="T extends string"` 正确;四个 `{#if}` 分支均 `in:fly={{ y: 8, duration: 180, easing: cubicOut }}`,仅 in 无 out;`.tab-pane` 自身无样式、无边框/内边距,margin 折叠穿透不变,sticky `.tabs` 与 `page-foot` 位置不受影响。settings-shared.css 仅由设置页导入一次,无重复导入。
6. **声明偏差核验**:`.sec-actions` 进 shared.css 合理(ApiTab 与 AboutTab 两处使用,无法 scoped 进单一组件)。类归属矩阵核验无其它错置:shared 其余类或被 ≥2 组件使用(`.sec`/`.sec-title`/`.sub-title`/`.fld`/`.var`),或为 design §4 明确列入 shared 的清单项(`.sec-title-row`/`.sec-actions-row`/`.fld-check`/`.fld > span`);所有 scoped 类均单组件使用,无跨组件复制。
7. **残留与死代码**:`.help`/`.tip` 标记与样式仅存在于 HelpTip.svelte;`+page.svelte` 无任何 tab UI 标记;shared.css 全部类均被使用;各文件无未使用 import(updater/app-meta 等仅迁至 AboutTab,`+layout.svelte`/`UpdateDialog.svelte` 的引用为既有代码)。
8. **scrollbar-gutter**:settings 与 history 两页 `.page-scroll` 均有 `scrollbar-gutter: stable`(源码 diff 与构建产物 chunk 双确认)。

### 发现并修复的问题

- 无。全部核验项通过,未发现需要修复的问题,代码零改动。

### 遗留事项

- 5.3 手动冒烟(`pnpm tauri dev`:四 tab 渲染、测试连接、采集勾选+路径选择、检查更新、切 tab/路由无横向跳动、过渡动画流畅)仍需真人执行。
- 已知且可接受的行为差异(design §2 明确归属 tab 局部的瞬时 UI 状态,不在不变量 1–6 覆盖内):切走再切回后 `showKey`/`testing`/`checking` 复位为初始值(旧版保持在页面层)。
- 级联依赖备注:`.sec { overflow: visible }` 与 app.css `.panel { overflow: hidden }` 同为 (0,1,0) 优先级,靠「设置页路由 CSS 后于入口 app.css 注入」的顺序取胜(已实证);若未来将 settings-shared.css 挪进入口加载链路,需同步提升该选择器优先级(如 `.panel.sec`)。
- 后续已排期(主会话确认):tab 切换过渡动画将按新需求变更为方向性横滑,届时对该增量改动单独核验;本记录结论针对当前 fly 上移方案。

### 最终验证

- `pnpm check`(svelte-check):**352 files,0 errors,0 warnings**(检查代理独立复跑)。

---

## 实现记录·二(2026-08-19,方向性横滑改造)

需求变更:tab 切换过渡从「仅 in 纵向淡入上移」(y:8 / 180ms)改为**方向性横滑**(手机换页手感:前进旧面板左滑出、新面板右滑入;后退镜像),design §6 已同步重写。本节为该增量改造的实现记录。

### 改动文件(仅 2 个源文件 + 本文档,其余文件零改动)

- `src/lib/components/settings/SettingsTabs.svelte` — 契约变更:`active` 由 `$bindable` 改为**只读 prop**(必传),新增 `onselect: (id: T) => void` 回调 prop;按钮 `onclick={() => onselect(t.id)}`。标记其余部分与 scoped 样式(`.tabs`/`.tab`)零改动。
- `src/routes/settings/+page.svelte` —
  - 新增 `direction = $state(1)` 与 `selectTab(id: SettingsTab)`:`direction = to >= from ? 1 : -1`(`SETTINGS_TABS` 下标差符号,前进 1 / 后退 -1;同 tab 点击时 `{#key}` 值不变、不触发任何过渡,兜底取 1 无副作用);**先算方向、再改 `activeTab`** —— 方向必须在新面板挂载前定型,不可用 `$effect` 事后追;
  - `<SettingsTabs>` 用法:`bind:active={activeTab}` → `tabs={SETTINGS_TABS} active={activeTab} onselect={selectTab}`;
  - 模板重构:四个 `{#if}` 分支各自的 `.tab-pane` 包裹层合并为一层 —— `<div class="tab-panes">` 内 `{#key activeTab}` 包单个 `<div class="tab-pane">`,其内保留原四分支 `{#if}`(四个 tab 组件的用法与全部 `bind:` 零改动);
  - 过渡:`in:fly={{ x: direction * 64, duration: 220, easing: cubicOut }}` 与 `out:fly={{ x: direction * -64, duration: 220, easing: cubicOut }}`;未覆盖 `opacity` 参数,保留 fly 默认透明度插值(重叠期两面板交叉淡化);`fly`/`cubicOut` 复用页面既有 import;
  - scoped 样式:新增 `.tab-panes { display: grid }`、`.tab-pane { grid-area: 1 / 1 }`(新旧面板同格叠加,不占两份文档流,过渡期容器高度 = max(旧, 新));原 `.tab-pane` 本无任何样式规则,无残留可删。
- `.trellis/tasks/08-19-settings-component-split/implement.md` — 步骤 1.2 / 3.2 更新 + 本节。

### 与 design §6 的符合性

- 前进(API→提示词→采集→关于)左滑出 / 右滑入、后退镜像:✔(`in` x 取 `direction * 64`,`out` x 取 `direction * -64`,两方向相反);
- 方向计算在点击时、先于 `activeTab` 变更(`selectTab` 同步函数,非 `$effect`):✔;
- SettingsTabs 契约为只读 `active` + `onselect` 回调:✔;
- grid 同格叠加(`.tab-panes { display: grid }` + `.tab-pane { grid-area: 1 / 1 }`,不顶动 sticky `.tabs` 与 `.page-foot`):✔;
- fly 默认透明度插值保留(重叠期交叉淡化):✔;
- 参数 220ms / x=64 / cubicOut:✔;
- `.tabs`(sticky)与 `.page-foot` 不参与过渡:✔;
- **偏差:无。**(同 tab 点击时 `direction` 取 1 为下标差 0 的边界规整,`{#key}` 值不变不重挂载,无可见影响。)

### 验证输出

- `pnpm check`(svelte-check,真实运行):**352 files,0 errors,0 warnings**。
- 残留 grep(全 `src/`):`y: 8` 纵向过渡参数 **0 处**;`out:fly` 参数确认 `direction * -64`(与 `in` 的 `direction * 64` 方向相反)。
- 改动面(`git status -- src/`):仅 `settings/+page.svelte`(既有改动上的增量)与 `settings/` 组件目录(其中仅 `SettingsTabs.svelte` 变更);`history/+page.svelte` 为上一阶段 scrollbar-gutter 改动,本次未触碰;ApiTab/PromptTab/CollectTab/AboutTab/HelpTip/settings-shared.css 均零改动。
- 手动冒烟(`pnpm tauri dev`:四 tab 渲染、横滑过渡方向正确且无闪烁/布局跳动)仍需真人执行,见步骤 5.3。

---

## 质检记录·二(2026-08-19,横滑增量独立核验)

### 逐项核验结论

1. **改动面**:`git status -- src/` + 组件文件 mtime 双确认 —— 本增量仅 `SettingsTabs.svelte`(10:06)与 `settings/+page.svelte`;其余 5 组件 + settings-shared.css(09:49–09:50)与 `history/+page.svelte`(diff 仍为 scrollbar-gutter 2 行)零改动。implement.md 步骤 1.2/3.2 已更新、「实现记录·二」完整,与实际代码逐条相符。
2. **契约与实现(读实际代码,非口述)**:
   - SettingsTabs:`active: T` 只读必传 + `onselect: (id: T) => void` 必传,`onclick={() => onselect(t.id)}`;全仓 grep 无 `bind:active` 残留;`.tabs`/`.tab` scoped 样式与首轮一致。
   - 页面:`selectTab` 为同步函数,先 `direction = to >= from ? 1 : -1` 再 `activeTab = id`(方向在新面板挂载、过渡参数定型之前算好;不用 `$effect` 的注释正确);`<SettingsTabs tabs active onselect>` 传参齐全。
   - `{#key activeTab}` 包单个 `.tab-pane`,四分支 `{#if}` 与全部 `bind:`(api/exportDir/template/weeklyMap/weeklyReduce/toolEnabled/includePaths/excludePaths/toolPaths/defaultPaths/autoCheckUpdate)零改动。
   - `in:fly={{ x: direction * 64, duration: 220, easing: cubicOut }}` / `out:fly={{ x: direction * -64, … }}`:in 起点(±64)与 out 终点(∓64)严格反向,前进右入左出、后退镜像;未传 `opacity`,保留 fly 默认交叉淡化。
   - `.tab-panes { display: grid }` + `.tab-pane { grid-area: 1/1 }` 同格叠加存在;`.tab-pane` 除 `grid-area` 外无样式;全仓 `y: 8` 0 处,无纵向残留;sticky `.tabs` 与 `.page-foot` 不参与过渡。
3. **`pnpm check` 独立复跑(修复后)**:352 files,0 errors,0 warnings。
4. **边界推演**:
   - 连点同一 tab:`to === from` → direction 归一 1,但 `{#key}` 值未变不重挂载,不触发过渡;同值 `activeTab = id` 赋值无副作用。✔
   - 快速连点不同 tab:keyed 块再次重建,进场中的面板被销毁 → Svelte 中止 intro、自当前状态起播 outro;多个退场面板与进场面板全部落在 grid 1/1 同格,容器高 = max(旧,新),page-foot 不横移,无布局破坏。✔
   - 切 tab 瞬间保存按钮:按钮在 `{#key}`/`.tab-panes` 之外,`saving` 与切 tab 无关;`activeTab` 同步更新 → 文案立即指向新 tab、随时可点,`saveActive` 按当前 activeTab 分发,语义正确。✔
   - 横向滚动条风险(补充检查):滑移 64px 需两侧余量 ≥64px;窗口 minWidth 900px(tauri.conf.json)、内容列 720px 居中 → 余量 90px,任何合法窗口尺寸横滑不触边,无瞬态横向滚动条。✔
5. **发现并修复(1 处)**:休息态间距回归 —— `.tab-panes` 设 `display: grid` 后,grid 容器阻断 margin 折叠:末卡片 `.sec` 的 1rem 下边距留在格内,不再与 `.page-foot` 的 0.8rem 上边距折叠为 max(1rem),静止间距变为 1.8rem(+12.8px 偏离基线)。修复:`.tab-panes` 加 `margin-bottom: -0.8rem`(与 page-foot 0.8rem 相邻折叠相消,净距复原为原 1rem;过渡期行为不变),复跑 `pnpm check` 通过。

### 遗留

- 人工冒烟(横滑方向手感、快速连点、220ms 交叉淡化无闪烁)待真人执行(步骤 5.3)。
- 无其它遗留。

---

## 实现记录·三(2026-08-19,整周汇总标题行间距修复)

用户冒烟前反馈:「整周汇总模板」标题行与上方「每日摘要」textarea 之间贴边、需要间隔。根因:`.field`(textarea)无 margin,`.sec-title-row` 只有 `margin-bottom: 0.6rem` 无上边距 —— **重构前即存在的旧问题**(旧版同样结构同样间距,首轮 CSS 对账因规则逐条一致未涉及),非本次重构引入。

### 改动(1 个源文件)

- `src/lib/components/settings/PromptTab.svelte` — scoped 新增 `.tmpl + .sec-title-row { margin-top: 1.1rem; }`(1.1rem 对齐独立 `.sub-title` 的上间距节奏;组件内唯一紧跟 textarea 的标题行即「整周汇总」,选择器只命中该处;`.tmpl` 自身无 margin-bottom,无折叠干扰)。

### 验证

- `pnpm check`(svelte-check,真实运行):**352 files,0 errors,0 warnings**。
- 观感与 1.1rem 节奏是否合适待用户冒烟确认。

