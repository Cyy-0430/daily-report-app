# State Management

> How state is managed in this project.(沉淀自 08-20 报告页状态保留任务)

---

## Overview

- **局部状态**:组件内 `$state`(runes)。
- **跨路由页面工作状态**:模块级 `$state`(`.svelte.ts` 单例对象),范例 `src/lib/report-state.svelte.ts`。
- **应用级共享状态**:svelte stores(`src/lib/store.ts`:config / history / toast / pendingInput)。
- 三者职责不混:stores 管跨功能域的应用数据;模块 runes 管单个页面族的工作现场;组件 `$state` 只管瞬时 UI。

---

## State Categories

| 类别 | 载体 | 生命周期 | 范例 |
|---|---|---|---|
| 瞬时 UI 状态 | 组件 `$state` | 随组件卸载销毁 | `testing`/`showKey`/对话框选择 |
| 页面工作状态 | 模块级 `$state`(`.svelte.ts`) | 应用启动 → 关闭(跨路由导航存活,整页刷新重置) | `daily` / `weekly`(日期、要点、采集结果、output、mode) |
| 应用共享状态 | `src/lib/store.ts` stores | 应用启动 → 关闭 | `config` / `history` / `toast` / `pendingInput` |
| 后端持久状态 | SQLite via IPC | 跨启动 | history 表、config 表 |

---

## Convention: 跨路由页面状态用模块级 `$state`

**What**: 用户在路由页(`/`、`/weekly` 等)的工作内容若需要在切走再切回时保留,状态放 `.svelte.ts` 模块里导出的 `$state` 常量对象,页面直接 `bind:value={page.field}`。

**Why**: SvelteKit SPA 客户端导航**不重新执行模块** → 模块级 `$state` 天然跨路由存活;而组件 `$state` 随路由组件卸载销毁。附带收益:异步闭包(如 LLM 流式回调)在页面卸载后继续写模块状态,切回即见累计输出。

**Example**:

```ts
// src/lib/report-state.svelte.ts —— 模块加载一次,对象属性可变
export const daily = $state({
  collectDate: todayStr(),          // 日期在应用启动时初始化一次;
  input: '',                        // 隔夜不自动跳新一天(有意的:手选值不再被重置)
  output: '',
  mode: 'preview' as 'edit' | 'preview',
  // ...
});
```

```svelte
<!-- 页面:直接绑定对象属性(深层 proxy,bind: 可用) -->
<textarea bind:value={daily.input}></textarea>
<ReportPanel bind:output={daily.output} bind:mode={daily.mode} />
```

**Rules**:
- `.svelte.ts` 里**只能导出 `const` 的 `$state` 对象**(导出可重赋值的 `let $state` 是编译错误);变更一律改属性。
- 跨页面共享的纯函数(`todayStr` / `buildFilter` 等)与状态同文件收敛,单一实现。
- 页面仍是编排层(onMount 数据加载、IPC 调用、保存编排留在 `+page.svelte`),状态模块只放数据与纯函数 —— 与 component-guidelines 的「路由页面 = 薄壳 + 编排」一致。

**When NOT**: 应用级、跨功能域的数据(config / history / toast)仍走 `src/lib/store.ts` stores,不要塞进页面状态模块;需要跨启动持久的数据走后端 SQLite,不要在前端做 localStorage 旁路。

---

## Server State

后端数据(IPC invoke)不做前端缓存层:config/history 在 `initConfig()`(layout onMount)拉一次进 store,后续写操作直接 update store 并由后端持久化;页面局部数据(采集结果等)随页面状态模块持有,重新采集即覆盖。

---

## Common Mistakes

### Common Mistake: 把「要跨路由保留的状态」放组件 `$state`

**Symptom**: 从日报/周报页切到设置再切回,日期、输入、采集结果、生成内容全部丢失。

**Cause**: 路由组件随 SvelteKit 导航卸载,组件 `$state` 销毁。

**Fix**: 状态提升到 `.svelte.ts` 模块级 `$state`(见上);同时把跨页要保留的子组件状态(如 ReportPanel 的 `mode`)改为 `$bindable`,由页面绑定到模块状态。

**Prevention**: 新增页面状态字段时先问「切走再切回要不要还在?」—— 要 → 模块 `$state`;不要 → 组件 `$state`。
