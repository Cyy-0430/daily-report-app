# 主题导入导出(JSON 分享)

## Goal

设置页主题定制:导出自定义主题为 JSON(复制到剪贴板)分享给他人;导入弹窗粘贴 JSON 新建自定义主题并启用。

## Requirements

### 导出

- 入口:主题下拉(ThemeDropdown)自定义主题行内,与现有 ✎ 重命名 / 🗑 删除并列,新增导出按钮(建议 ⤓ / 复制类 icon)。
- 行为:点击后把该主题序列化为紧凑 JSON 并复制到剪贴板,toast 提示「已复制主题 JSON」。
- 预设主题(纸墨)不可导出(预设不可变,现状无行内操作,维持)。
- 剪贴板实现与现有复制按钮一致:`@tauri-apps/plugin-clipboard-manager` 的 `writeText`。

### 导入

- 入口:「当前主题」区块(或下拉底部)新增「导入主题」按钮。
- 行为:点击弹出对话框(overlay + textarea),粘贴 JSON,确认后:
  - 校验失败 → toast 错误(「JSON 格式不正确」),弹窗保留内容供修改;
  - 校验成功 → 以新 UUID、名称(取 JSON.name,重名经 dedupeName 去重)新建自定义主题,保存 + 立即启用 + applyTheme,toast「已导入并启用「name」」。
- 弹窗交互:Esc / 点击遮罩 / 取消按钮关闭;对齐 UpdateDialog 的 overlay 模式。

### JSON 格式(分享契约)

```json
{"name":"夜读","colors":{"paper":"#101010","ink":"#f0eae0",…}}
```

- 仅两个字段:`name`(string)与 `colors`(Record<string,string>)。不带 id —— 导入方永远生成新 id。
- 兼容性:colors 缺 key 回退预设、多余 key 忽略(与 resolveColors 既有语义一致);导入时按 THEME_VAR_KEYS 白名单过滤,仅接受 `#rgb`/`#rrggbb` 合法值。

## Constraints

- 纯前端任务:不改动 Rust(CustomTheme/ThemeConfig 结构不变)、不新增 IPC。
- 序列化/校验逻辑放 `src/lib/theme.ts` 纯函数层并补 `theme.test.ts` 单测(UI 只做编排)。
- 遵循 `.trellis/spec/frontend/theming.md`(themeConfig 跨层字段契约)与 `component-guidelines.md`。

## Acceptance Criteria

- [ ] 下拉中每个自定义主题可一键导出:剪贴板得到合法 JSON,粘贴回导入框可原样导入(名称、色板一致)。
- [ ] 导入弹窗:合法 JSON → 新主题出现在下拉中并立即启用;非法 JSON / 非 JSON 文本 / colors 无一合法值 → toast 报错且不落盘。
- [ ] 导入的主题名与现有主题重名时自动加后缀,不覆盖既有主题。
- [ ] `theme.test.ts` 覆盖:导出序列化、导入解析(合法/缺字段/多余 key/非法 hex/空 name)。
- [ ] `pnpm check` 通过;`cargo test` 不回归(本次无 Rust 改动)。
