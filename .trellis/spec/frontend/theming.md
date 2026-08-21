# Theming — CSS 变量主题系统

> 沉淀自 08-20 主题定制任务。涉及 `src/app.css`、`src/lib/theme.ts`、`src/lib/theme-state.svelte.ts`、`src/lib/components/settings/ThemeTab.svelte` 与 `AppConfig.themeConfig`(跨层契约)。改动可定制颜色变量或主题应用方式前必读。

---

## Scenario: 运行时主题覆盖系统

### 1. Scope / Trigger

- 新增/修改可定制颜色变量、调整主题应用方式、动 `AppConfig.themeConfig` 结构时适用
- `themeConfig` 是跨层字段(Rust `ThemeConfig`/`CustomTheme` ↔ `bindings.ts` 同名接口),遵循 storage-spec 的增量字段契约(`#[serde(default)]` + db.rs 五个同步点)

### 2. Signatures

```ts
// src/lib/theme.ts —— 纯数据/纯函数层
export type ThemeColors = Record<string, string>; // key = CSS 变量名(不带 --),值 = "#rrggbb"
export const PRESET_ID = 'editorial-paper';       // id 不变(已存配置兼容);显示名 PRESET_NAME = '纸墨'
export const THEME_VAR_GROUPS;  // 注册表:10 变量按语义分组,新增变量在此声明
export const THEME_VAR_KEYS;    // 展平注册表(声明顺序),resolveColors/applyTheme 的遍历白名单
export const EDITORIAL_PAPER;   // 预设值
export function resolveColors(themeConfig, id?): ThemeColors; // 命中 custom 逐 key 合并;未命中 → 预设副本
export function applyTheme(colors): void;                    // 唯一 DOM 出口
export function hexToRgb / rgbToHex / nextThemeName / dedupeName;
export function exportThemeJson(theme: CustomTheme): string;          // 分享 JSON(见 Contracts)
export function parseThemeJson(text: string): { name: string; colors: ThemeColors } | null;

// src/lib/theme-state.svelte.ts —— 模块级 $state(仅状态)
export const theme = $state({ draft: null | { baseId, colors }, preview: null | ThemeColors });
```

### 3. Contracts

- **双源互锁**:`app.css :root`(首帧正确性)↔ `theme.ts EDITORIAL_PAPER`(运行时应用与编辑器基线)两份同值,改任一侧必须同步另一侧(两侧注释互指,`theme.test.ts` 断言注册表 10 key 全部在预设中合法)
- **applyTheme 单出口**:10 个变量统一 `setProperty` 显式写入,不做 removeProperty 分支——运行时单一代码路径,消除双源漂移;任何组件不得自行 `setProperty('--…')`
- **状态三层**(遵循 state-management.md):已保存主题 → `config` store(`themeConfig`,SQLite 持久);编辑现场 draft + 预览 preview → 模块 `$state`(跨路由存活、不落盘,关闭即失);生效色 = `preview ?? resolveColors(config.themeConfig)`
- **主题切换语义**:下拉选中 = 立即激活并持久化(get config → 改 → saveConfig → config.set,不回读);保存 = 总是新建主题(自动命名),不覆盖
- **分享 JSON 契约**(用户互操作格式,一旦发布不可随意变更):`{"name":"…","colors":{"paper":"#rrggbb",…}}` 仅两字段——**不带 id**(导入方永远 `crypto.randomUUID()` 新造 + `dedupeName` 去重,绝不覆盖既有主题);导出走 `THEME_VAR_KEYS` 白名单规范化,导入逐 key 白名单 + `hexToRgb` 过滤;导入色板存**原始未补全**值(缺 key 由 resolveColors 兜底,不存储补全);导入编排与保存同形(新主题 + 立即启用)
- 组件样式一律 `var(--paper)` 等令牌引用,禁止写死色值——令牌运行时可变,写死即脱离主题系统

### 4. Validation & Error Matrix

| 条件 | 行为 |
|---|---|
| `activeId` 空/未命中/指向已删 id | 回落预设副本 |
| `colors` 缺 key | 逐 key 回退 `EDITORIAL_PAPER` |
| `colors` 多 key / 非 string / 空串 | 忽略(只遍历 `THEME_VAR_KEYS` 白名单) |
| 非法 hex(`hexToRgb` → null) | 不对外(null 传播,调用方跳过) |
| 旧配置无 `themeConfig` | serde default = 空 = 预设,无损升级 |
| `parseThemeJson`:parse 失败 / name 非非空 string / colors 非普通对象 | → null(UI toast 报错,不落盘) |
| `parseThemeJson`:colors 过滤后无任何合法色 | → null(同上) |
| `parseThemeJson`:name 前后空白 | trim 后接受 |

### 5. Good/Base/Bad Cases

- **Good**:新增可定制变量 = 四处同步——① `app.css :root` 加变量(组件样式即可用)② `THEME_VAR_GROUPS` 加声明 ③ `EDITORIAL_PAPER` 加预设值 ④ `theme.test.ts` 的 10 改为新长度
- **Base**:只读场景——取生效色一律 `resolveColors(config.themeConfig)`,不自己拼对象
- **Bad**:组件里写死 `#f3eee3`;绕过 `applyTheme` 直接改 CSS 变量;预设值只改 app.css 不改 `EDITORIAL_PAPER`(或反之)

### 6. Tests Required

- `src/lib/theme.test.ts`(vitest):注册表↔预设互锁、resolveColors 回退/白名单/副本隔离、hex 双向换算与钳制、`nextThemeName` 最小空闲编号、`dedupeName` 递增后缀、`exportThemeJson`/`parseThemeJson`(导出→解析往返一致、非法输入全 null)——改主题数据结构时先改这里的断言

### 7. Wrong vs Correct

```css
/* Wrong:写死色值,主题定制对它无效 */
.panel { background: #fffdf7; }

/* Correct:令牌引用,跟随运行时主题 */
.panel { background: var(--paper-card); }
```

```ts
// Wrong:removeProperty 分支回预设(两套代码路径,与 :root 双源漂移)
if (isPreset) keys.forEach((k) => root.style.removeProperty(`--${k}`));

// Correct:统一显式写入(resolveColors 已保证预设场景返回预设值)
for (const k of THEME_VAR_KEYS) root.style.setProperty(`--${k}`, colors[k]);
```
