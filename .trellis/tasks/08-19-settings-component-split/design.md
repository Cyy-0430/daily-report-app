# 技术设计:设置页拆分为多组件

## 1. 总体架构

**页面 = 状态与持久化编排,tab 组件 = UI 与局部交互。** 持久化字段的 `$state` 全部留在 `+page.svelte`(这是「切 tab 不丢未保存编辑」这一既有设计的前提:tab 组件随 `{#if}` 卸载,状态必须活在页面层),通过 Svelte 5 `$bindable` + `bind:` 传给各 tab;tab 内的纯 UI 状态与只作用于本 tab 域的函数随组件走。

与 `ReportPanel.svelte` 的 `bind:output` 既有惯例一致,不引入新状态管理模式(不用 `.svelte.ts` 单例、不动 `store.ts`)。

## 2. 状态归属表

| 状态/函数 | 去向 | 理由 |
|---|---|---|
| `activeTab` / `SettingsTab` 类型 / `SETTINGS_TABS` | 页面 | 导航是页面职责 |
| `api`, `exportDir` | 页面 `$state` → `bind:` 给 ApiTab | saveApi 需要 |
| `showKey`, `testing`, `test()`, `pickDir()` | ApiTab 局部 | 只触及 API tab UI 与已绑定字段 |
| `template`, `weeklyMap`, `weeklyReduce` | 页面 `$state` → `bind:` 给 PromptTab | savePrompt 需要 |
| `customDefault`, `weeklyDefMap`, `weeklyDefReduce` + 6 个设为默认/恢复默认函数 | PromptTab 局部(own onMount 里 `loadConfig` 读取) | 「自定义默认」是提示词域概念;设为默认本身即时持久化,不依赖页面 save。多一次 `loadConfig`(SQLite 读)可忽略;PromptTab 首次挂载远晚于页面 onMount,无竞态 |
| `toolEnabled`, `includePaths`, `excludePaths`, `toolPaths` | 页面 `$state` → `bind:` 给 CollectTab | saveCollect 需要 |
| `defaultPaths` | 页面加载,只读 prop 传 CollectTab | saveCollect 的 `storedPath()` 也要用 |
| `dedupePaths()`, `storedPath()` | 留在页面(saveCollect 内使用) | 保存域辅助 |
| add/remove/pick 路径 6 个函数 | CollectTab 局部 | 纯 UI 交互,改的都是绑定字段 |
| `appVersion`, `autoCheckUpdate`, `checking`, `checkUpdateManual()` | AboutTab;`autoCheckUpdate` 用 `bind:` 回流 | saveAbout 需要 autoCheckUpdate;其余纯局部 |
| `saving`, `saveApi/Prompt/Collect/About/Active`, onMount 加载 | 页面 | 按页保存编排 |
| `.help`/`.tip` 结构与样式 | HelpTip 组件(children snippet 放提示正文,支持 `<code class="var">` 富文本) | 消除 10+ 处复制粘贴 |

绑定对象为 Record/数组时,子组件对 `$bindable` prop 的元素级赋值(`toolEnabled[t.id] = x`)与整体重赋值(`excludePaths = [...]`)均合法——页面 `$state` 是深层代理,双向可见。

## 3. 组件契约

```
SettingsTabs.svelte   generics="T extends string"
  props: tabs: { id: T; label: string }[]; active: T ($bindable)

HelpTip.svelte
  props: (无);children snippet = 提示正文(可含 {TPL_*} 的 <code class="var">)
  根元素 <span class="help" tabindex="0" role="button">,样式 scoped 在本组件

ApiTab.svelte
  bind: api: ApiConfig; exportDir: string
  局部: showKey, testing; 函数 test(), pickDir()

PromptTab.svelte
  bind: template: string; weeklyMap: string; weeklyReduce: string
  局部: customDefault, weeklyDefMap, weeklyDefReduce(own onMount 读取);6 个默认值函数

CollectTab.svelte
  bind: toolEnabled: Record<string, boolean>; includePaths: string[];
        excludePaths: string[]; toolPaths: Record<string, string>
  props: defaultPaths: Record<string, string>  (只读)
  局部: add/remove/pick 6 函数

AboutTab.svelte
  bind: autoCheckUpdate: boolean
  局部: appVersion, checking; 函数 checkUpdateManual()
```

页面骨架(保留):`page-scroll > page-inner > (SettingsTabs, {#if activeTab===...} 四选一, page-foot 保存按钮)`。

## 4. 样式策略

Svelte 组件 `<style>` 是 scoped 的,跨组件共享类必须走全局 CSS:

- **`settings-shared.css`**(由设置页 `+page.svelte` 导入一次,类名仅设置页使用,勿在其它路由复用):`.sec`(含 `overflow: visible`、`z-index: 20` hover 提层)、`.sec-title`、`.sec-title-row`、`.sec-actions-row`、`.sub-title`、`.fld`、`.fld-check`、`.fld > span`、`.var`。
- **各组件 scoped**:`.tabs/.tab`(SettingsTabs)、`.help/.tip`(HelpTip)、`.grid-2/.row-input`(ApiTab)、`.tmpl`(PromptTab)、`.path-group/.path-row/.tool-path-row/.path-add`(CollectTab)、`.about-*/.meta-row/.link-btn/.star-*/.toggle-*/.switch/.knob`(AboutTab)。
- 注意:snippet(children)内容按父组件作用域编译,故 `.var` 必须留在全局 shared.css,不能放进 HelpTip。
- `.panel/.field/.btn/.page-foot` 等继续走 `app.css` 全局,不动。

## 5. 偏移修复

`settings/+page.svelte` 与 `history/+page.svelte` 的 `.page-scroll` 增加 `scrollbar-gutter: stable;`,滚动条槽常驻,`margin: 0 auto` 居中列宽度恒定,切 tab/路由不再横向跳动。WebView2(Chromium 94+)支持该属性。

## 6. Tab 切换过渡动画(方向性横滑)

> 2026-08-19 需求变更:用户要求手机换页式左右滑动,取代初版「仅 in 的纵向淡入上移」。

- **交互**:按 `SETTINGS_TABS` 顺序,前进(API→提示词→采集→关于)时旧面板向左滑出、新面板从右侧滑入;后退方向镜像(x 取反)。
- **实现**:页面用 `{#key activeTab}` 包住单个 `<div class="tab-pane">`,内部仍是四分支 `{#if}`;进出过渡同时启用:`in:fly={{ x: dir * 64, duration: 220, easing: cubicOut }}`、`out:fly={{ x: dir * -64, duration: 220, easing: cubicOut }}`。fly 自带透明度插值,重叠期两面板交叉淡化,避免两张不透明卡片叠穿帮。
- **方向计算在点击时**:`SettingsTabs` 契约由 `active $bindable` 改为只读 `active` + `onselect` 回调;页面 `selectTab()` 先按新旧 tab 下标差设 `direction`(±1)再改 `activeTab` —— 方向必须在新面板挂载前就绪,不可用 `$effect` 事后追(挂载时过渡参数已定型)。
- **布局**:`.tab-panes { display: grid }` + `.tab-pane { grid-area: 1 / 1 }` —— 新旧面板同格叠加,不占两份文档流,page-foot/sticky 不被顶动;过渡期容器高度 = max(旧, 新),220ms 后回落到新面板高度,瞬态可接受。
- `.tabs`(sticky)与 `.page-foot` 不加过渡。
- 动画与「未保存编辑不丢」无冲突(状态在页面层,`{#key}` 重建只影响标记;旧组件因 out 过渡延迟 ~220ms 卸载,无副作用)。

## 7. 兼容与回滚

- 不改 IPC/bindings/config 字段,`pnpm check` + 手动冒烟即可验证;Rust 侧零改动。
- 单次提交承载全部改动,出问题 `git revert` 一条回滚;组件文件均为新增,页面文件可对照 git diff 快速还原。

## 8. 风险

| 风险 | 缓解 |
|---|---|
| in/out 双过渡把两份内容顶开布局 | grid 同格叠加(`grid-area: 1/1`),两面板重叠而非并排(design §6) |
| 方向状态在面板挂载后才更新,滑错方向 | 点击时同步计算 direction 再切 activeTab,onselect 回调契约(design §6) |
| scoped 样式迁漏导致 UI 走样 | 按「样式策略」清单逐类迁移;冒烟逐 tab 对照旧截图/记忆 |
| snippet 内容样式失效(`.var`) | `.var` 放 shared.css(全局),已列入契约 |
| bind 对象深层变更父组件看不到 | 页面 `$state` 深层代理保证可见;冒烟验证采集 tab 勾选→保存链路 |
| PromptTab own-onMount 多一次 loadConfig | 读操作、SQLite 本地,成本可忽略;已论证无竞态 |
