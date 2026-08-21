# Implement — 主题定制 tab

按序执行;每步末尾跑该步验证命令,通过才进下一步。步骤 1-2 是跨层契约,先行落地并保持双侧同步(见 CLAUDE.md「Rust struct ↔ TS interface must stay in sync」)。

## 步骤清单

### 1. Rust:配置结构体 + KV DAO + 测试
- [ ] `src-tauri/src/config.rs`:`CustomTheme` / `ThemeConfig` 结构体(camelCase + Default derive + 字段级 `#[serde(default)]`),`AppConfig` 加 `theme_config`(结构见 design §2.1)
- [ ] `src-tauri/src/db.rs`:`KEY_THEME_CONFIG` 常量;`get_config`(`db.rs:107`)加分支;`config_pairs`(`db.rs:152`)追加对;`sample_config()`(`db.rs:419`)补 `theme_config` 字段(编译器强制)
- [ ] config.rs 测试:① legacy JSON(无 themeConfig)解析 → default;② round-trip:`themeConfig.activeId`/`custom[0].colors` camelCase 序列化断言
- [ ] db.rs 测试:`sample_config` set→get 往返 theme_config 一致;仅写旧 KV(无 theme key)读出 = `ThemeConfig::default()`
- 验证:`cargo test`(在 src-tauri/)、`cargo check`

### 2. TS 绑定
- [ ] `src/lib/bindings.ts`:`CustomTheme` / `ThemeConfig` 接口;`AppConfig` 加 `themeConfig`;`emptyConfig()`(`bindings.ts:129`)补 `themeConfig: { activeId: '', custom: [] }`
- 验证:`pnpm check`

### 3. 主题模块(纯函数 + 状态)
- [ ] `src/lib/theme.ts`:`PRESET_ID`/`PRESET_NAME`/`THEME_VAR_GROUPS`(10 变量 5 组,文案见 design §3)/`EDITORIAL_PAPER`/`resolveColors`/`applyTheme`/`hexToRgb`/`rgbToHex`/`nextThemeName`/`dedupeName`/`ThemeColors` 类型
- [ ] `src/app.css` `:root` 顶部加同步注释(指向 theme.ts 的 EDITORIAL_PAPER)
- [ ] `src/lib/theme-state.svelte.ts`:`theme` 模块 `$state`(draft/preview),仅状态,编排留组件(state-management.md 约定)
- 验证:`pnpm check`

### 4. 启动应用激活主题
- [ ] `src/routes/+layout.svelte` onMount:`await initConfig()` 后 `applyTheme(resolveColors(get(config).themeConfig))`
- 验证:`pnpm tauri dev` 启动,无自定义主题时外观与现在逐像素一致(预设值首帧即 :root)

### 5. ColorPicker 组件
- [ ] `src/lib/components/ColorPicker.svelte`(design §6.1):SV 面 + 色相条(CSS 渐变 + pointer capture)、HEX/R/G/B 双向输入、外点/Esc 关闭、a11y(role/aria/方向键)
- 验证:`pnpm check`(0 a11y 警告);dev 中手动取色、RGB 键入、双向同步

### 6. ThemeDropdown 组件
- [ ] `src/lib/components/settings/ThemeDropdown.svelte`(design §6.2):预设行 + 自定义行、悬停 ✎/🗑、行内重命名、Esc/外点关闭
- 验证:`pnpm check`;dev 中开关、重命名提交/取消、删除回调触发

### 7. ThemeTab + 设置页接线
- [ ] `src/lib/components/settings/ThemeTab.svelte`(design §6.3):三 section + 交互编排(选中/预览/结束预览/保存/重命名/删除,持久化模式 = get config → 改 → saveConfig → config.set)
- [ ] `src/routes/settings/+page.svelte`:`SettingsTab` 类型加 `'theme'`;`SETTINGS_TABS`(`+page.svelte:27`)插入 `{ id: 'theme', label: '主题' }`(collect 之后、about 之前);渲染分支 `<ThemeTab />`
- 验证:`pnpm check`

### 8. 调色盘替换为 svelte-awesome-color-picker(QA 驱动的方案变更,见 design §6.1 变更注记)
- [ ] `pnpm add svelte-awesome-color-picker`(v4,Svelte 5 重写版)
- [ ] 重写 `src/lib/components/ColorPicker.svelte` 内部为库包装层:**对外契约不变**(value/onchange/label),内部用库 `ColorPicker` 组件;`--cp-*` 变量映射到 Editorial Paper 令牌;不用 alpha
- [ ] `theme.ts` 清理仅剩旧实现使用的 HSV 换算纯函数(先 grep 确认无其他使用方)
- [ ] ThemeTab 的 10 个调用点零改动(契约不变的验收点)
- 验证:`pnpm check` 0 警告;dev 中取色/RGB 键入/格式切换正常、弹层视觉融入纸感主题

### 9. 全量校验 + 手工 QA 矩阵
- [ ] `pnpm check` + `cargo test` 全绿
- [ ] QA 矩阵(dev 环境逐项过):
  - 首次进入:下拉仅 "Editorial Paper · 预设",编辑器载入预设值
  - 改背景色 → 预览 → 全局变色;切日报/周报/历史/设置仍保持;**重启应用恢复预设**
  - 保存 → 出现"自定义主题 1"并激活;重启仍激活
  - 再保存一次 → "自定义主题 2"(基于 1 的当前 draft)
  - 下拉切回预设 → 立即恢复预设(持久化,重启仍是预设)
  - 重命名:空名保持原名;重名自动后缀;✎ 后 Esc 取消
  - 删除激活中的主题 → 回落预设
  - 调色盘:鼠标拖 SV/色相、RGB 键入越界钳制、HEX 3 位输入
  - 旧配置兼容:用现网配置文件启动(无 themeConfig)不报错、行为同预设
- [ ] 过一遍 design §7 边界表

## 风险文件 / 回滚点

| 文件 | 风险 | 回滚 |
|---|---|---|
| `src-tauri/src/db.rs` / `config.rs` | 配置结构变更影响 load/save 往返 | 步骤 1 测试先行;字段纯增量,回滚 = revert 单 commit,旧配置不含新 key 无需迁移 |
| `src/lib/bindings.ts` `emptyConfig()` | 漏改导致 undefined 访问 | `pnpm check` 强类型兜底 |
| `src/app.css` | 只加注释,不改值 | 无风险 |
| `+layout.svelte` | 应用时机错误 → 首帧闪烁 | 仅 initConfig 后一行调用,首帧由 :root 保证 |

整体回滚:功能收敛为单次提交(步骤 1-7),`git revert` 即完整回退;SQLite config 表多出的 `themeConfig` key 对旧代码无影响(get_config 按需读 key)。

## 收尾前置检查(task.py start 前已满足 / 实施完成后再核)

- [x] prd.md 收敛、design.md、implement.md 齐备(复杂任务三件套)
- [x] implement.jsonl / check.jsonl 已填真实条目
- [ ] 实施完成:全部 QA 矩阵项 + `pnpm check`/`cargo test` 通过后,进入 Phase 3(spec 沉淀:主题变量注册表契约可考虑补入 frontend spec;config 增量字段惯例已在 storage-spec)
