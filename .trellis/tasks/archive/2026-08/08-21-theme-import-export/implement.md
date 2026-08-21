# 实施计划:主题导入导出

执行者:trellis-implement。前置:`task.py start` 已将本任务置为 in_progress。

## 顺序清单

1. [ ] `src/lib/theme.ts`:新增 `exportThemeJson` / `parseThemeJson`(签名与防御规则见 design「数据契约」;hexToRgb 复用现有导出)。
2. [ ] `src/lib/theme.test.ts`:补两组 describe(design「测试」所列用例)。
3. [ ] `src/lib/components/settings/ThemeDropdown.svelte`:行内操作组加导出按钮(⤓ 类 icon + title「导出」),新增 `onexport` prop;props 类型与 ✎/🗑 一致(callback id)。键盘可达(button 元素)。
4. [ ] 新建 `src/lib/components/settings/ImportThemeDialog.svelte`:overlay + textarea + 取消/导入按钮;Esc、遮罩点击关闭;样式对齐 UpdateDialog 与 settings-shared.css。
5. [ ] `src/lib/components/settings/ThemeTab.svelte`:
   - `exportTheme(id)`(writeText + toast;import `writeText` from '@tauri-apps/plugin-clipboard-manager');
   - `importTheme(payload)`(见 design 编排;失败 notify err);
   - ThemeDropdown 传 `onexport`;「当前主题」sec-title 挂「导入主题」按钮 + ImportThemeDialog 挂载(`importOpen = $state(false)`);
   - HelpTip 文案补一句导入导出说明。
6. [ ] `pnpm check` 全绿。
7. [ ] `pnpm test` 全绿(vitest,含新增 theme 用例)。

## 验证命令

```bash
pnpm check
pnpm test
cargo test   # 无 Rust 改动,防意外回归
```

手工(可选,dev 环境):导出→剪贴板→导入→主题出现在下拉且立即生效;粘非 JSON 报错且弹窗内容保留。

## 审查门

- 自查 design「数据契约」第 4 条白名单过滤是否与 resolveColors 语义一致(缺 key 回退、多余 key 忽略)。
- 检查 ThemeDropdown 行内三按钮(✎ ⤓ 🗑)的 hover 布局未破。

## 回滚点

单 commit 交付;异常时 `git revert` 该 commit,无持久化副作用。
