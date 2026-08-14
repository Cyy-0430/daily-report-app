# Implement — 关于tab与自动更新检查

> 关联 `prd.md` / `design.md`。有序 checklist；每步后跑标注的验证命令。
> **外部门槛**：第 9 步（端到端真实更新）依赖用户先生成签名密钥并配置 GitHub Secrets（见 prd 前置依赖）；
> 代码可在该前提未完成时先就位，CI 真实产物在首个带 updater 配置的 tag 发布时生成。

## 步骤

### 1. Rust 配置字段
- [ ] `src-tauri/src/config.rs`：`AppConfig` 加 `auto_check_update: bool`，
      `#[serde(default = "default_true")]`；新增 `fn default_true() -> bool { true }`。
- [ ] 验证：`cargo check`（src-tauri/）。

### 2. Rust 插件注册
- [ ] `src-tauri/Cargo.toml`：加 `tauri-plugin-updater = "2"`、`tauri-plugin-process = "2"`。
- [ ] `src-tauri/src/lib.rs`：`.plugin(tauri_plugin_updater::Builder::new().build())`、
      `.plugin(tauri_plugin_process::init())`。
- [ ] 验证：`cargo check`。

### 3. tauri.conf.json + capabilities
- [ ] `src-tauri/tauri.conf.json`：`bundle.createUpdaterArtifacts = true`；
      新增 `plugins.updater`（`pubkey` 占位 `"<REPLACE_WITH_PUBKEY>"`，`endpoints` 指向 release latest.json）。
- [ ] `src-tauri/capabilities/default.json`：permissions 加 `updater:default`、`process:default`。
- [ ] 验证：`cargo check`（生成上下文会校验配置）。

### 4. TS 配置同步
- [ ] `src/lib/bindings.ts`：`AppConfig` 加 `autoCheckUpdate: boolean`；`emptyConfig()` 设 `true`。
- [ ] 验证：`pnpm check`。

### 5. npm 依赖
- [ ] `pnpm add @tauri-apps/plugin-updater @tauri-apps/plugin-process`。
- [ ] 验证：`pnpm check`。

### 6. 元数据 + 更新封装
- [ ] 新建 `src/lib/app-meta.ts`：`APP_NAME` / `APP_NAME_EN` / `APP_AUTHOR` / `GITHUB_URL`。
- [ ] 新建 `src/lib/updater.ts`：
      - `UpdateInfo` 接口 + `checkForUpdate(): Promise<UpdateInfo>`（包 `check()`，抛错上抛）。
      - `updateDialog` writable store（`{ version, body } | null`），供全局单例弹窗。
      - `downloadAndInstallWithProgress(onProgress)`：包 `downloadAndInstall()` 进度回调。
- [ ] 验证：`pnpm check`。

### 7. 更新对话框组件
- [ ] 新建 `src/lib/components/UpdateDialog.svelte`：模态，展示版本/说明/Star 引导，
      「立即下载并安装」（进度条）+「稍后」，成功 `relaunch()`，复用 `app.css` 变量与按钮风格。
- [ ] `src/routes/+layout.svelte`：挂载单例 `UpdateDialog`（监听 `updateDialog` store）；
      `onMount` 在 `initConfig()` 后按 `autoCheckUpdate` 调 `checkForUpdate()`，
      有更新则 `updateDialog.set(...)`，catch 静默。
- [ ] 验证：`pnpm check`。

### 8. 关于 tab UI
- [ ] `src/routes/settings/+page.svelte`：
      - `SettingsTab` 加 `'about'`；`SETTINGS_TABS` 追加「关于」。
      - `autoCheckUpdate = $state(true)`，`onMount` 初始化自 `c.autoCheckUpdate`。
      - `saveAbout()`（load→overlay→save）+ `saveActive()` 分支。
      - 关于 tab 内容：应用名/版本（`getVersion()`）/作者/GitHub 链接（`openUrl`）、
        自动检查开关、「立即检查更新」按钮（无新版→toast 最新；有新版→`updateDialog.set`）。
- [ ] 验证：`pnpm check`。

### 9. CI + 文档收尾
- [ ] `.github/workflows/release.yml`：tauri-action env 加 `TAURI_SIGNING_PRIVATE_KEY`、
      `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 两个 secret 引用。
- [ ] 在「关于」tab 或对话框中保留 Star 引导文案（design 已含）。
- [ ] （用户完成密钥后）把真实 pubkey 替换占位；首个 `v*` tag 验证端到端。

## 验证命令汇总

```bash
pnpm check                 # 前端类型检查
cargo test                 # Rust 单测（含新字段往返）
cargo check                # Rust 编译检查
```

## Review Gates

- 提交前：`pnpm check` + `cargo test` + `cargo check` 全绿。
- 自检跨层：`AppConfig.autoCheckUpdate` 两端字段名/默认值一致；updater/process 在
  Cargo/npm/tauri.conf.json/capabilities 四处齐全。
- 自检容错：启动失败静默、手动失败 toast、dev 无签名不崩。

## Rollback Points

- 第 1~5 步（配置/依赖）若编译不过：逐项还原，配置字段可单独回退（前端先不引用即可）。
- 第 6~8 步（UI）：组件/store 可整体移除，不影响既有功能（关于 tab 为纯增量）。
- 第 9 步 CI：仅加 env 引用，未配 secret 时 CI 仍可跑（secret 为空则跳过签名 → updater 产物缺失，但不阻断普通构建）。
