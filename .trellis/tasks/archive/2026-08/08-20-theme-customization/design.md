# Design — 主题定制 tab

## 1. 架构总览

```
┌─ 前端 ────────────────────────────────────────────────────┐
│ settings/+page.svelte ── tab: 'theme' → ThemeTab          │
│   ThemeTab                                                │
│     ├─ ThemeDropdown  (选中即激活/✎重命名/🗑删除)          │
│     ├─ 变量色块列表(10 个,分组)→ ColorPicker 弹层        │
│     └─ [预览] [结束预览] [保存]                            │
│                                                           │
│ src/lib/theme.ts          纯数据/纯函数(注册表/预设/换算)  │
│ src/lib/theme-state.svelte.ts  模块级 $state(draft/preview)│
│ store.ts config store     themeConfig(已保存,应用级)      │
│ +layout.svelte            initConfig() 后应用激活主题      │
└───────────────────────────────────────────────────────────┘
                    invoke save_config / load_config
┌─ 后端 ────────────────────────────────────────────────────┐
│ config.rs  ThemeConfig / CustomTheme 结构体 + AppConfig 字段│
│ db.rs      config KV:KEY_THEME_CONFIG 读写对               │
└───────────────────────────────────────────────────────────┘
```

无新 IPC 命令 —— 复用 `load_config` / `save_config`。

## 2. 数据模型与跨层契约

### 2.1 Rust(`src-tauri/src/config.rs`)

```rust
/// 单个自定义主题。colors 键 = 变量名(不带 --,如 "paper"),值 = "#rrggbb"。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomTheme {
    #[serde(default)] pub id: String,
    #[serde(default)] pub name: String,
    #[serde(default)] pub colors: HashMap<String, String>,
}

/// activeId = 自定义主题 id;空串或未命中 → 预设 "Editorial Paper"。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    #[serde(default)] pub active_id: String,
    #[serde(default)] pub custom: Vec<CustomTheme>,
}

// AppConfig 增加:
#[serde(default)]
pub theme_config: ThemeConfig,
```

两个结构体都 derive `Default`(否则字段级 `#[serde(default)]` 编不过)。

### 2.2 TS 镜像(`src/lib/bindings.ts`)

```ts
export interface CustomTheme { id: string; name: string; colors: Record<string, string>; }
export interface ThemeConfig { activeId: string; custom: CustomTheme[]; }
// AppConfig 增加 themeConfig: ThemeConfig;
// emptyConfig() 增加 themeConfig: { activeId: '', custom: [] },
```

### 2.3 db.rs 同步点(手写 KV DAO,逐字段显式)

- 新增 `const KEY_THEME_CONFIG: &str = "themeConfig";`
- `get_config`:加一个 `if let Some(v) = get_kv(conn, KEY_THEME_CONFIG)?` 分支
- `config_pairs`:追加 `(KEY_THEME_CONFIG, serde_json::to_string(&cfg.theme_config))`
- 测试辅助 `sample_config()`(`db.rs:419`):显式构造全字段,补 `theme_config`
- 兼容性:旧库无该 key → `AppConfig::default()` 路径,`theme_config` 为空 = 预设,无损升级

## 3. 主题变量注册表与预设(`src/lib/theme.ts`,纯函数)

```ts
export type ThemeColors = Record<string, string>; // key 同 Rust colors 键

export const PRESET_ID = 'editorial-paper';
export const PRESET_NAME = 'Editorial Paper';

export const THEME_VAR_GROUPS = [
  { group: '页面', vars: [
    { key: 'paper',      label: '页面背景', desc: '应用底色、滚动区背景' },
    { key: 'paper-card', label: '卡片底色', desc: '左右面板与设置卡片背景' } ] },
  { group: '文字', vars: [
    { key: 'ink',       label: '正文',     desc: '正文文字、主按钮底色' },
    { key: 'ink-soft',  label: '次级文字', desc: '说明、次级按钮文字' },
    { key: 'ink-faint', label: '弱化文字', desc: '占位符、计数、标签' } ] },
  { group: '线条', vars: [
    { key: 'line',        label: '边框线',   desc: '卡片边框、分隔线' },
    { key: 'line-strong', label: '强调边框', desc: '输入框边框、滚动条' } ] },
  { group: '强调', vars: [
    { key: 'accent',   label: '强调色',     desc: '强调按钮、选中态、光标选区' },
    { key: 'accent-2', label: '强调悬停色', desc: '强调按钮 hover' } ] },
  { group: '状态', vars: [
    { key: 'ok', label: '成功色', desc: '成功提示、完成标记' } ] },
] as const;

// 预设值,与 src/app.css :root 逐一对应 —— 改任一侧须同步另一侧
export const EDITORIAL_PAPER: ThemeColors = {
  paper: '#f3eee3', 'paper-card': '#fffdf7', ink: '#1f1c18',
  'ink-soft': '#5d564c', 'ink-faint': '#a59c8d', line: '#e7dfd0',
  'line-strong': '#d3c8b4', accent: '#9c3a26', 'accent-2': '#b9492f',
  ok: '#3d6b4e',
};
```

纯函数(全部可单测,虽本仓库前端无测试框架,保持小而直白):
- `resolveColors(themeConfig, id?): ThemeColors` — 命中自定义主题逐 key 合并(缺 key 回退 EDITORIAL_PAPER,多余 key 忽略);未命中/空 → EDITORIAL_PAPER 副本
- `applyTheme(colors)` — 10 个变量全部 `setProperty` 到 `document.documentElement`(**统一显式写入,不做 removeProperty 分支**;:root 样式表仅负责首帧,运行时以本函数为唯一出口,消除两份预设值的漂移问题)
- `hexToRgb(hex)` / `rgbToHex(r,g,b)` — 换算 + 钳制(0-255)、hex 3/6 位归一
- `nextThemeName(custom): string` — `"自定义主题 N"`,N = 未被占用的最小正整数(与现有 name 全等比较)
- `dedupeName(name, custom): string` — 重命名冲突时追加 `" (2)"` 递增后缀

app.css `:root` 顶部加一行注释指向 `EDITORIAL_PAPER` 同步义务。

## 4. 状态模型(`src/lib/theme-state.svelte.ts`)

遵循 `state-management.md` 三层约定:

| 状态 | 载体 | 生命周期 |
|---|---|---|
| 已保存主题(activeId + custom) | `config` store(`themeConfig` 字段) | 跨启动(SQLite) |
| 编辑现场 draft | 模块级 `$state` | 应用启动→关闭(切 tab/切路由均保留) |
| 预览 preview | 模块级 `$state` | 应用启动→关闭,**不落盘**(关闭即恢复) |

```ts
export const theme = $state({
  draft: null as null | { baseId: string; colors: ThemeColors },
  preview: null as null | ThemeColors,
});
```

DOM 应用统一走 `applyTheme`:
- 生效色 = `preview ?? resolveColors(config.themeConfig)`
- 启动:`+layout.svelte` onMount 中 `await initConfig()` 之后 `applyTheme(resolveColors(get(config).themeConfig))`(store.ts 保持纯数据层,不碰 DOM)
- 预览:`theme.preview = { ...theme.draft.colors }; applyTheme(...)`
- 结束预览:`theme.preview = null`,重新应用激活主题
- draft 的存在使"切到日报页再回设置页,调过的颜色和未保存现场还在"成为默认行为(与报告页状态保留同构)

## 5. 交互流(ThemeTab 编排,均为显式函数)

| 动作 | 行为 | 持久化 |
|---|---|---|
| 下拉选中主题 | `activeId = id` + `saveConfig` + `applyTheme`;同时 `draft = { baseId: id, colors: resolveColors }`;若在预览中,**预览结束**(用户已明确选择其他主题) | ✅ 立即 |
| 调色(ColorPicker) | 只改 `draft.colors[key]`,不触 DOM 全局 | ❌ |
| 单项重置 | `draft.colors[key] = resolveColors(baseId)[key]` | ❌ |
| 预览 | `preview = {...draft.colors}` + apply;tab 内出现"预览中"横幅 | ❌ |
| 结束预览 | `preview = null` + apply 激活主题 | ❌ |
| 保存 | `nextThemeName` 生成名 + `crypto.randomUUID()` 生成 id → push custom + `activeId = id` + `saveConfig` + apply + `preview = null` + `draft.baseId = 新id`(**保存总是新建主题**,即使基于自定义主题修改;覆盖式更新明确排除在 MVP 外) | ✅ 立即 |
| 重命名(下拉 ✎) | 行内输入框,Enter/失焦提交;trim 后空 → 保持原名;重名 → `dedupeName` | ✅ 立即 |
| 删除(下拉 🗑) | 移除该项;若删的是 activeId → `activeId = ''`(回落预设)+ apply;toast "已删除" | ✅ 立即 |
| 预设选中 | 同"下拉选中"(activeId = '' 即预设);预设行**无** ✎/🗑 | ✅ 立即 |

保存/切换的写盘模式照抄设置页现有惯例:`get(config)` → 改 → `saveConfig` → `config.set`(前端 store 与后端同步,不回读)。

## 6. 组件设计

### 6.1 `ColorPicker.svelte`(`src/lib/components/`,通用组件;svelte-awesome-color-picker 包装层)

> **2026-08-21 QA 期间决策变更**:自研取色面实现替换为组件库 `svelte-awesome-color-picker`(v4,Svelte 5 全重写,与 runes 栈匹配)。动因:RGB 数字被 spinner 遮挡等打磨成本;库自带 SV 面/色相条/HEX·RGB·HSL 格式切换输入区/键盘导航,成熟度远超手搓。项目首个 UI 组件依赖。

- 对外契约不变:prop `value: string`(#rrggbb)+ 回调 `onchange(hex)` + 可选 `label`(a11y 名);内部渲染库的 `ColorPicker`,初始色 = value,`onInput` → `onchange(e.hex)`
- 主题化:包装层 scoped CSS 把库的 `--cp-*` 变量(`--cp-bg-color`/`--cp-border-color`/`--cp-text-color`/`--cp-input-color`/`--cp-button-hover-color` 及尺寸类)映射到 Editorial Paper 令牌(`--paper-card`/`--line`/`--line-strong`/`--ink`/`--ink-soft` 等)——令牌本身运行时可变(主题定制),调色盘自动跟随当前主题
- 不用 alpha 通道(只进出 hex);库默认"色块按钮 + 点开弹层"形态与原交互一致
- `theme.ts` 随替换清理:HSV 换算等仅被旧实现使用的纯函数删除,仍被别处引用的(hexToRgb/rgbToHex 等)保留

### 6.2 `ThemeDropdown.svelte`(`src/lib/components/settings/`)

- props:`activeId`(只读)、`custom`(只读)、回调 `onselect(id)` / `onrename(id, name)` / `ondel(id)`(编排留 ThemeTab,符合 component-guidelines 回调 props 约定)
- 本地瞬时状态:`open`、`renamingId`(哪一行处于行内重命名)
- 列表:预设行(`Editorial Paper · 预设`,无操作按钮)+ 自定义行(悬停显示 ✎ 🗑;✎ 将该行替换为 `<input class="field">`)
- 样式:scoped;弹层 z-index 压过面板(`settings-shared.css` 的 tooltip 越界模式可参照)
- a11y:`role="listbox"`/`option`、Esc 关闭、点击外部关闭

### 6.3 `ThemeTab.svelte`(`src/lib/components/settings/`)

- 复用 `settings-shared.css` 的 `.sec`/`.sec-title`/`.fld` 语言;三个 section:当前主题(下拉)、颜色定制(分组变量列表)、操作区
- 变量行:色块(内联 style 用 draft 值,实时反映)+ label + desc(HelpTip 或次级文字)+ 单项重置 ghost 按钮
- 操作区:`[预览] [结束预览(仅预览中可见)] [保存]`;预览中横幅说明"临时配色,关闭应用后自动恢复"
- draft 为 null 时(首次进入):自动以当前激活主题初始化 draft
- 状态读取走模块 `theme` + `config` store;编排函数(选择/预览/保存/重命名/删除 + saveConfig)全部在本组件 —— 与"域组件负责即时持久化操作"(PromptTab「设为默认」先例)一致

### 6.4 设置页接线(`src/routes/settings/+page.svelte`)

- `SettingsTab` 联合类型加 `'theme'`;`SETTINGS_TABS` 在 `'collect'` 与 `'about'` 之间插 `{ id: 'theme', label: '主题' }`
- `{#if activeTab === 'theme'}` 分支渲染 `<ThemeTab />`(无 bind —— 主题状态在模块层,不经页面转 hand,偏离其它 tab 的页面 $state 模式,理由:状态本就要跨路由存活,模块级是 spec 规定归宿)

## 7. 边界与异常

| 场景 | 行为 |
|---|---|
| 旧配置无 themeConfig | serde default → 空配置 = 预设,界面/editorial 正常 |
| activeId 指向已删除 id(数据异常) | `resolveColors` 回落预设 |
| colors 缺 key / 多 key | 逐 key 回退预设 / 忽略多余 |
| 预览中切换主题/保存 | 预览结束(切换→应用新选中;保存→新主题转正,视觉无跳变) |
| 全部自定义主题删光 | 回预设,下拉只剩预设行 |
| 首帧闪变 | app.css :root 即预设值,首帧正确;JS 应用主题只在有自定义激活时产生变化 |

## 8. 测试与验证

- Rust(`cargo test`):
  - config.rs:legacy JSON 缺 `themeConfig` → default;round-trip camelCase(`"themeConfig":{"activeId":…,"custom":[…]}`)
  - db.rs:`sample_config` 带 theme_config 的 set/get 往返;旧 KV 集(无 theme key)读出 = 默认
- 前端:`pnpm check`(0 警告,a11y 含在内)+ `pnpm test`(vitest;`theme.test.ts` 覆盖纯函数与注册表↔预设互锁)
- 手工 QA 矩阵(见 implement.md):预览跨路由/重启恢复、保存启用/重启保持、重命名去重、删除回落、旧配置升级

## 9. 取舍记录

- **保存总是新建** vs 覆盖当前自定义主题:取新建(用户原始语义"下拉列表多一个新的主题" + 全自动命名);覆盖留作后续增强
- **统一显式写 10 个变量** vs 预设走 removeProperty:取前者,运行时单一代码路径,规避预设双源漂移;代价是 app.css 与 EDITORIAL_PAPER 两份值需注释互锁
- **下拉选中即激活** vs 仅载入编辑器:用户已选前者(常规主题选择器心智)
- **自定义下拉** vs 原生 select:悬停行内 ✎/🗑 需求 + 全库无 select 先例,自研约 120 行,可控
- **主题状态放模块 $state** vs 页面 $state 下传:state-management.md 明确"跨路由工作状态 → 模块级",且预览本就要全局生效
- **调色盘自研 vs 组件库**:初版自研(QA 中暴露 spinner 遮挡、输入区简陋),QA 后改用 `svelte-awesome-color-picker` v4——引入首个 UI 依赖换取成熟交互与格式切换,`--cp-*` 变量映射保住纸感视觉(见 §6.1)
