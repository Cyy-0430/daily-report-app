# Component Guidelines

> How components are built in this project. (Svelte 5 runes; 范例:`src/lib/components/ReportPanel.svelte`、`src/lib/components/settings/*`)

---

## Overview

- 组件一律 **Svelte 5 runes**(`$state` / `$derived` / `$props` / `$bindable`),不用 svelte stores 做局部状态;全局共享状态才进 `src/lib/store.ts`(stores)。
- 两类组件:
  - **功能面板组件**(`src/lib/components/**`):自包含 UI + 局部交互,通过 props/bind 与父级通信;
  - **路由页面**(`src/routes/**/+page.svelte`):薄壳 —— 持有持久化字段 `$state`、数据加载(onMount)、保存编排;不写具体 UI 标记。
- 范例:设置页 `src/routes/settings/+page.svelte`(247 行薄壳)+ `src/lib/components/settings/` 六个组件(SettingsTabs / HelpTip / ApiTab / PromptTab / CollectTab / AboutTab)+ `settings-shared.css`。

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
- **snippet(children)内容按父组件作用域编译**:子组件(如 HelpTip)内渲染的 children 里用到的类(如 `<code class="var">`)必须留在全局 CSS,scoped 进子组件会失效。
- tooltip 需要越出卡片边界:卡片容器 `overflow: visible` 覆盖 `.panel` 的 `overflow: hidden`,悬浮卡片 `:hover` 提 `z-index` 盖过相邻卡片(见 settings-shared.css 的 `.sec` / `.sec:hover`)。

---

## Accessibility

- 非按钮的可聚焦交互元素补 `tabindex="0"` + `role="button"` + `aria-label`(HelpTip 问号);开关用 `role="switch"` + `aria-checked` + `aria-label`。
- 纯装饰标记 `aria-hidden="true"`。
- `pnpm check`(svelte-check)会报 a11y 警告,保持 0 警告。

---

## Common Mistakes

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
