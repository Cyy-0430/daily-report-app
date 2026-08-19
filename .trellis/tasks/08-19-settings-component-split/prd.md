# 设置页拆分为多组件

## Goal

`src/routes/settings/+page.svelte` 目前 987 行,tab 导航、四个 tab 的全部 UI/状态/保存逻辑挤在一个文件里。将其拆分为职责清晰的多个组件,页面退化为薄壳(导航 + 状态编排 + 按页保存),不改任何用户可见行为。

## Requirements

- 拆出组件目录 `src/lib/components/settings/`,至少包含:
  - `SettingsTabs.svelte` — tab 导航条
  - `HelpTip.svelte` — 圆形问号悬浮气泡(现以 `.help`/`.tip` 结构复制粘贴 10+ 处)
  - `ApiTab.svelte` — API 配置 + 导出目录
  - `PromptTab.svelte` — 日报模板 / 周报双模板 + 设为默认/恢复默认
  - `CollectTab.svelte` — 采集工具勾选 + 数据源路径 + 路径过滤
  - `AboutTab.svelte` — 应用信息 + 更新
  - `settings-shared.css` — 跨 tab 共享样式
- **行为完全不变**(见验收标准,新增的过渡动画除外);重构是纯结构调整。
- **tab 切换过渡动画(方向性横滑)**:按 tab 顺序前进(如 API→提示词)时旧面板向左滑出、新面板从右滑入;后退方向镜像 —— 手机滑动换页的手感。新旧面板叠加过渡(同格 grid),不引起文档流跳动。
- 顺手修复:切 tab 时因垂直滚动条出现/消失导致的居中列左右偏移 —— 给设置页与历史页的 `.page-scroll` 加 `scrollbar-gutter: stable`。

## 行为不变量(重构必须逐条保住)

1. 切换 tab 后,未保存的本地编辑仍在内存中(切回来不丢);保存只写当前 tab 的字段(按页保存,load→overlay→save 整份回写)。
2. 「设为默认/恢复默认」立即持久化(loadConfig→改→saveConfig),不等「保存」按钮。
3. 恢复默认的回退链:自定义默认 → 内置默认模板。
4. 底部「保存{tabLabel}」按钮的文案、禁用态(`saving`)与分发逻辑不变。
5. tooltip(`.sec` 的 `overflow: visible`、悬浮卡片 `z-index: 20` 提到相邻卡片之上)行为不变。
6. 全键盘可达性(`tabindex="0"`、`role`、`aria-*`)不回退。

## Acceptance Criteria

- [ ] `src/routes/settings/+page.svelte` 不再包含任何 tab 的具体 UI 标记,仅保留:tab 类型/常量、持久化字段 `$state`、onMount 加载、按页保存函数、页面骨架(page-scroll/page-inner/tabs/page-foot)。
- [ ] 上述 6 个组件 + 1 个共享 CSS 文件存在并被使用;无未使用的残留样式或死代码。
- [ ] 行为不变量 1–6 逐条核验通过(实现后人工过一遍,记录在 implement.md 勾选)。
- [ ] `pnpm check`(svelte-check)零错误。
- [ ] 设置页、历史页 `.page-scroll` 均有 `scrollbar-gutter: stable`;切 tab、切路由不再左右跳动。
- [ ] 切 tab 过渡为方向性横滑(前进左滑、后退右滑,新面板滑入+旧面板滑出叠加进行),动画期间无布局跳动/闪烁;sticky 的 tab 条与底部保存栏不参与动画。
- [ ] 手动冒烟:四个 tab 渲染正常,保存/测试连接/设为默认/路径选择/检查更新均工作。

## Notes

- 纯前端改动,不涉及 Rust/IPC/bindings,`cargo test` 不受影响。
- 不引入新依赖、不改路由、不改 store。
