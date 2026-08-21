# 设计:主题导入导出

## 边界

- 改动文件:`src/lib/theme.ts`(+2 纯函数)、`src/lib/theme.test.ts`(+用例)、`src/lib/components/settings/ThemeTab.svelte`(编排 + 导入按钮)、`src/lib/components/settings/ThemeDropdown.svelte`(行内导出按钮)、新组件 `src/lib/components/settings/ImportThemeDialog.svelte`。
- 不动:Rust 侧、bindings.ts、theme-state.svelte.ts(draft/preview 状态机不变)。

## 数据契约(分享 JSON)

```ts
/** 分享格式:仅 name + colors;id 不参与(导入方永远新造)。 */
export function exportThemeJson(theme: CustomTheme): string;
export function parseThemeJson(text: string): { name: string; colors: ThemeColors } | null;
```

- `exportThemeJson`:`JSON.stringify({ name, colors })`,colors 只输出 THEME_VAR_KEYS 内的键(存储层可能有多余键,导出即规范化)。
- `parseThemeJson` 全量防御:
  1. `JSON.parse` 失败 → null;
  2. `name` 非非空 string → null(不静默改名,错误留给 UI 提示;极端「name 缺失」也视为格式错误);
  3. `colors` 非对象 → null;
  4. 逐 key 过滤:仅保留 THEME_VAR_KEYS 内且 `hexToRgb(key 值) !== null` 的项;结果为空(无任何合法色)→ null;
  5. 返回 `{ name: trim 后的 name, colors }`(缺 key 由 resolveColors 兜底预设,不在此补全)。

## 交互与编排(ThemeTab)

### 导出(下拉行内)

- ThemeDropdown 新增 `onexport: (id: string) => void` 回调 + 行内按钮(与 ✎/🗑 同一 hover 操作组)。
- ThemeTab:

```ts
async function exportTheme(id: string) {
  const t = get(config).themeConfig.custom.find((x) => x.id === id);
  if (!t) return;
  await writeText(exportThemeJson(t));
  notify('ok', `已复制「${t.name}」主题 JSON`);
}
```

- 剪贴板失败(极少)→ `notify('err', String(e))`。

### 导入(弹窗)

- ThemeTab「当前主题」区块 `.sec-title` 旁挂「导入主题」ghost 按钮 → 打开 ImportThemeDialog。
- ImportThemeDialog(自治组件,仿 UpdateDialog overlay 模式):
  - props:`onimport(payload: { name; colors })`、`onclose`;
  - 局部状态:`text = $state('')`;Esc / 遮罩点击 / 取消 → onclose;
  - 确认:`parseThemeJson(text)` → null 则 `notify('err', 'JSON 格式不正确,请检查后重试')` 并保持弹窗;否则 `onimport(payload)` 且自清空关闭。
- ThemeTab 接收后(编排复用 saveTheme 的落盘形状):

```ts
async function importTheme(p: { name: string; colors: ThemeColors }) {
  const base = get(config);
  const id = crypto.randomUUID();
  const item: CustomTheme = { id, name: dedupeName(p.name, base.themeConfig.custom), colors: p.colors };
  const merged = { ...base, themeConfig: { activeId: id, custom: [...base.themeConfig.custom, item] } };
  await saveConfig(merged); config.set(merged);
  applyTheme(resolveColors(merged.themeConfig)); theme.preview = null;
  theme.draft = { baseId: id, colors: resolveColors(merged.themeConfig) };
  notify('ok', `已导入并启用「${item.name}」`);
}
```

  - 与 saveTheme 差异仅:名称来自 JSON(经 dedupeName)而非 nextThemeName;故两者并存不复用(语义不同,硬凑泛化反而绕)。

## 权衡记录

- **导出到剪贴板而非文件**:分享场景主路径是 IM 传文本;写文件涉及对话框+路径,留待需要时再加。
- **导入后立即启用**:与「保存主题」行为一致(立即生效给用户正反馈);只入库不启用会让人以为没导成功。
- **colors 存原始(未补全)色板**:resolveColors 已兜底缺 key;存储层补全 10 色反而掩盖「这是导入主题」且体积无益。
- **弹窗保留错误内容**:粘贴长 JSON 打错一处不该清空重来。

## 测试

`theme.test.ts` 新增 describe:
- exportThemeJson:输出可被 JSON.parse;仅含 name/colors;多余键被剔除。
- parseThemeJson:合法 / name 空 / name 缺失 / colors 非对象 / 非法 hex 被剔除 / 全非法 → null / 多余 key 被忽略 / 3 位 hex(#fff)合法 / 前后空白容忍。

## 回滚

全部为前端增量改动,git revert 单 commit 即可;无数据迁移(导入主题走既有 ThemeConfig 存储)。
