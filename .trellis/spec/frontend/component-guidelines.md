# Component Guidelines

> How components are built in this project. (Svelte 5 runes; 范例:`src/lib/components/ReportPanel.svelte`、`src/lib/components/settings/*`)

---

## Overview

- 组件一律 **Svelte 5 runes**(`$state` / `$derived` / `$props` / `$bindable`),不用 svelte stores 做局部状态;全局共享状态才进 `src/lib/store.ts`(stores)。
- 两类组件:
  - **功能面板组件**(`src/lib/components/**`):自包含 UI + 局部交互,通过 props/bind 与父级通信;
  - **路由页面**(`src/routes/**/+page.svelte`):薄壳 —— 编排(onMount 数据加载、IPC 调用、保存)、注入共享 CSS;不写具体 UI 标记。跨路由保留的工作状态放模块级 `$state`(见 state-management.md),不再由页面 `$state` 持有。
- 范例:设置页 `src/routes/settings/+page.svelte`(薄壳)+ `src/lib/components/settings/`(SettingsTabs / HelpTip / ApiTab / PromptTab / CollectTab / AboutTab / TemplateEditor)+ `settings-shared.css`;报告页 `src/routes/+page.svelte` / `src/routes/weekly/+page.svelte`(薄壳)+ `src/lib/components/report/`(InputPanel + report-shared.css)+ 共享的 ReportPanel。

---

## Component Structure

### Convention: 状态归属(页面持有持久化,子组件持有瞬时)

**What**: 跨组件切换后必须存活的状态(未保存编辑等)放在路由页面 `$state`,经 `bind:` 下传;只影响本组件 UI 的瞬时状态(`testing`/`checking`/对话框选择等)放组件局部 `$state`,随卸载销毁。

**Why**: 设置页 tab 组件随 `{#if}` 卸载 —— 持久化字段若沉入子组件,切 tab 即丢未保存编辑;瞬时状态留在页面则白白膨胀页面脚本。

**Example**:
```svelte
<!-- 页面:持久化字段 + 保存编排 -->
let api = $state<ApiConfig>({ baseUrl: '', apiKey: '', model: '' });
<ApiTab bind:api bind:exportDir />

<!-- ApiTab:局部瞬时状态 -->
let showKey = $state(false);
```

**Related**: 「设为默认」类即时持久化操作(loadConfig→改→saveConfig)随域组件走(PromptTab),不等页面「保存」按钮。

---

## Props Conventions

- `$props()` 解构 + 内联类型注释;双向绑定用 `$bindable`(参照 ReportPanel 的 `output = $bindable('')`)。
- **回调 props(`onselect` 等)用于「父级必须先于状态变更做某事」的场景**:方向性过渡的 direction 必须在新面板挂载(过渡参数定型)之前算好,所以 SettingsTabs 是只读 `active` + `onselect`,页面在 `selectTab()` 里先算 `direction` 再改 `activeTab` —— 不可用 `$effect` 事后追(详见 Common Mistakes)。
- 绑定对象为 Record/数组时,子组件对 `$bindable` prop 的元素级赋值(`toolEnabled[t.id] = x`)与整体重赋值(`excludePaths = [...]`)均合法(页面 `$state` 深层代理,父级可见)。
- 子组件接收只读数据用普通 prop(`defaultPaths`),命名表达语义,不加 `on:` 前缀(Svelte 5 用回调 props 取代事件)。

---

## Styling Patterns

- 组件独有样式 scoped 在组件 `<style>`;跨组件共享的类放**共享全局 CSS**,由该功能的页面导入一次:
  - 设置页:`src/lib/components/settings/settings-shared.css`(`.sec` / `.sec-title` / `.sec-title-row` / `.sub-title` / `.fld` / `.var` 等);
  - **类名按约定仅限该功能使用,勿在其它路由复用**(它们是全局生效的)。
- 全局基础类(`.panel` / `.field` / `.btn` / `.page-foot` 等)在 `src/app.css`,组件直接用、不覆盖。
- **snippet(children)内容按父组件作用域编译**:子组件(如 HelpTip)内渲染的 children 里用到的类(如 `<code class="var">`)必须留在全局 CSS,scoped 进子组件会失效。同理:页面经 snippet(`InputPanel` 的 `head`/`extra`)注入的标记,其中用到的类(如 `.collect-bar`/`.meta`)必须放共享 CSS(`report-shared.css`)或页面 scoped,不能 scoped 进接收 snippet 的组件。
- tooltip 需要越出卡片边界:卡片容器 `overflow: visible` 覆盖 `.panel` 的 `overflow: hidden`,悬浮卡片 `:hover` 提 `z-index` 盖过相邻卡片(见 settings-shared.css 的 `.sec` / `.sec:hover`)。

---

## Accessibility

- 非按钮的可聚焦交互元素补 `tabindex="0"` + `role="button"` + `aria-label`(HelpTip 问号);开关用 `role="switch"` + `aria-checked` + `aria-label`。
- 纯装饰标记 `aria-hidden="true"`。
- `pnpm check`(svelte-check)会报 a11y 警告,保持 0 警告。

---

## Common Mistakes

### Common Mistake: 跨组件实例的相邻选择器被 scoped CSS 剪除

**Symptom**: 想让「上一个实例的 textarea」与「下一个实例的标题行」之间有间距,写在组件 scoped `<style>` 里(`.tmpl + .sec-title-row { margin-top: 1.1rem }`),svelte-check 报 `css_unused_selector`,规则被剪掉,间距真实丢失。

**Cause**: scoped 选择器只对「同一实例内部的相邻关系」做静态分析;两个元素分属同一组件的两个实例(每日摘要的 textarea + 整周汇总的标题行,均为 TemplateEditor 渲染)时,编译器判定选择器永不命中并剪除 —— 即使运行时它们确实是相邻兄弟。

**Fix**: 该类跨实例样式放功能域共享全局 CSS(本例 `settings-shared.css` 的 `.tmpl + .sec-title-row`),并确认加载链覆盖(该 CSS 由设置页路由导入、组件仅在该路由渲染)。

**Prevention**: 给组件写相邻/兄弟选择器前先问:两个元素是否**必定来自同一实例**?不是 → 直接进共享 CSS。

### Common Mistake: 过渡参数在挂载时定型,`$effect` 追不上

**Symptom**: 方向性过渡第一次播放方向随机/总是旧值。

**Cause**: `{#key}`/`{#if}` 新分支挂载时 `fly` 参数即被读取;`$effect` 在 DOM 更新后才跑,写 direction 已晚。

**Fix / Prevention**: 让状态变更经过父级函数(回调 prop),在改 `activeTab` 之前同步算好派生参数:

```ts
function selectTab(id: SettingsTab) {
  direction = to >= from ? 1 : -1; // 先算
  activeTab = id; // 后切,挂载时参数已定型
}
```

### Gotcha: grid 容器阻断 margin 折叠

**Symptom**: 给容器加 `display: grid`(如 tab 面板同格叠加 `.tab-panes`)后,其内部子元素与外部相邻元素的 margin 不再折叠,休息态间距变大。

**Fix**: 用负 margin 精确复原折叠语义(`.tab-panes { margin-bottom: -0.8rem }` 对冲 `.page-foot` 的 0.8rem 上边距),并复跑视觉核验。

### Gotcha: 等优先级覆盖依赖 CSS 注入顺序

**Symptom**: `.sec { overflow: visible }` 覆盖 app.css 的 `.panel { overflow: hidden }` —— 两者同为 (0,1,0) 优先级,靠「设置页路由 CSS 后于入口 app.css 注入」的顺序取胜。

**Prevention**: 若未来把 settings-shared.css 挪进入口加载链路,必须同步提升选择器优先级(如 `.panel.sec`);改动共享 CSS 加载方式时,冒烟检查 tooltip 越界是否仍正常。

### Pattern: 方向性换页过渡(手机滑动手感)

```svelte
<div class="tab-panes">
  {#key activeTab}
    <div
      class="tab-pane"
      in:fly={{ x: direction * 64, duration: 220, easing: cubicOut }}
      out:fly={{ x: direction * -64, duration: 220, easing: cubicOut }}
    >
      {#if activeTab === 'api'}…{:else if …}…{/if}
    </div>
  {/key}
</div>

<style>
  .tab-panes { display: grid; }
  .tab-pane { grid-area: 1 / 1; } /* 新旧同格叠加,不占两份文档流,不顶动 sticky/页脚 */
</style>
```

要点:`in`/`out` 的 x 严格相反;保留 fly 默认 opacity 交叉淡化(两张不透明卡片直接叠会穿帮);sticky 头部与页脚不参与过渡。

### Pattern: 第三方组件包装层(范例 ColorPicker.svelte → svelte-awesome-color-picker)

**What**(沉淀自 08-20 主题任务):引入第三方 UI 组件时不直接在业务组件里裸用,而是包一层薄 wrapper,**对外契约( props/回调)保持与旧自研实现完全不变**,业务调用点零改动;wrapper 内做三件事:

1. **令牌映射**:库的样式变量(如 `--cp-*`)在包装层 scoped CSS 里映射到本项目令牌,**必须 `var(--…)` 引用而非写死色值**——令牌运行时可变(见 theming.md),写死即脱离主题系统
2. **值归一**:库输出可能超出业务契约(如 hex 带 alpha 段),在 wrapper 归一后对外(本项目统一 `#rrggbb`)
3. **受控同步**:外部 value 变化(单项重置等)经 `$effect` 推入库组件;用**非响应式 `lastOut`** 记录最近对外发出/外部推入值,阻断「库归一化回写 → 再对外」「外部推入 → 库 onInput 回声」两类回环

**Why**:依赖可替换(换库只动 wrapper)、业务面零波及、主题一致性由映射保证。

### Common Mistake: svelte-awesome-color-picker 的 horizontal 模式把取色面和色相条排成一行

**Symptom**:`sliderDirection="horizontal"` 下弹层里 SV 面与色相条左右并排,弹层被撑到约 450px 宽。

**Cause**:库在 horizontal 模式下两个选择器均为 inline 级盒子(`inline-block`/`inline-flex`),弹层容器 `width: max-content` 单行最大化,默认流式布局未做堆叠。

**Fix**:包装层覆盖为块级——`:global(div.picker) { display: block }`、`:global(div.h) { display: flex }`。

**Prevention**:包装库组件时先在 dev 里过一遍全部模式开关;布局不对先查库的 display/容器约束,再写覆盖。
