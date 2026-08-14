# Design — 关于tab与自动更新检查

> 关联 `prd.md`。本文件描述边界、契约、数据流、跨层兼容与 CI/签名。MVP 取最小可工作形态。

## 1. 架构边界

```
+---------------------------+        invoke         +-----------------------------+
|  前端 (Svelte 5 runes)    |  ------------------>  |  Rust (Tauri commands)      |
|  settings/+page.svelte    |   load/save_config     |  config.rs (AppConfig)      |
|  + 新「关于」tab          |                        |  + autoCheckUpdate 字段     |
|  UpdateDialog.svelte      |                        |  lib.rs (注册 updater/      |
|  src/lib/updater.ts       |                        |   process 插件)             |
|  src/lib/app-meta.ts      |                        +-----------------------------+
+---------------------------+
        |  @tauri-apps/plugin-updater::check()   ← 插件命令，直连 GitHub Releases
        |  plugin-updater::downloadAndInstall()
        |  plugin-process::relaunch()
        v
   GitHub Release (latest.json + 签名安装包)
```

- **检查/下载/安装全在前端**：updater 插件把能力直接暴露给 JS（`check()` / `downloadAndInstall()`）。
  Rust 侧**只**做两件事：① 注册 `tauri-plugin-updater` + `tauri-plugin-process`；
  ② 在 `AppConfig` 持久化 `autoCheckUpdate`。无需新增任何 `#[tauri::command]`。
- **配置仍走既有 `load_config`/`save_config`**，遵循「按页保存」：关于 tab 的保存 = load 全量 →
  overlay `autoCheckUpdate` → 整份回写。

## 2. 跨层契约（必须两端同步）

### 2.1 AppConfig 新字段

**Rust `src-tauri/src/config.rs`**
```rust
pub struct AppConfig {
    // ...既有字段...
    /// 是否在启动时自动检查更新;默认 true。
    #[serde(default = "default_true")]
    pub auto_check_update: bool,
}

fn default_true() -> bool { true }
```
> `AppConfig` 已整体 `#[serde(rename_all = "camelCase")]`，故序列化键为 `autoCheckUpdate`。
> `#[serde(default)]` 兜底（storage-spec 不变式：新字段必须 default）。

**TS `src/lib/bindings.ts`**
```ts
export interface AppConfig {
  // ...既有字段...
  /** 启动时是否自动检查更新(默认 true)。 */
  autoCheckUpdate: boolean;
}

export function emptyConfig(): AppConfig {
  return { /* ...既有... */ autoCheckUpdate: true };
}
```
> 无 codegen，手改两侧 + `invoke` 键（本字段只在 `load/save_config` 的整体对象里，无独立 invoke 键）。

### 2.2 更新检测返回类型（纯前端，无 IPC 契约）

`src/lib/updater.ts` 导出 `checkForUpdate()`：
```ts
export interface UpdateInfo { available: boolean; version?: string; body?: string }
export async function checkForUpdate(): Promise<UpdateInfo>
```
内部包 `updater.check()`：有更新返回 `{ available:true, version, body }`；无更新返回 `{ available:false }`；
**任何抛错都向上抛**（由调用方决定静默或 toast）。

## 3. 前端组件设计

### 3.1 新增 `src/lib/app-meta.ts`（静态元数据，单一来源）
```ts
export const APP_NAME = '日报生成';
export const APP_NAME_EN = 'DailyReport';
export const APP_AUTHOR = 'cyy';
export const GITHUB_URL = 'https://github.com/Cyy-0430/daily-report-app';
```
> 作者/GitHub 不在构建配置里，集中为常量；版本号不在此处，运行时用 `getVersion()` 取（见 3.2）。

### 3.2 版本号来源
- 用 `import { getVersion } from '@tauri-apps/api/app'`，运行时返回 `tauri.conf.json` 的 `version`。
  与打包产物一致；CI 用 tag 覆盖该字段后即为发布版本。**不**从 `package.json` 读，避免与产物不符。

### 3.3 settings/+page.svelte 改动
- `SettingsTab` 增加 `'about'`；`SETTINGS_TABS` 追加 `{ id:'about', label:'关于' }`。
- 新增本地状态 `autoCheckUpdate = $state(true)`，`onMount` 里从 `c.autoCheckUpdate` 初始化。
- 新增 `saveAbout()`：`loadConfig()` → `{ ...cur, autoCheckUpdate }` → `saveConfig()`。
- `saveActive()` 增加 `if (activeTab==='about') return saveAbout()`。
- 新增手动检查逻辑：`checking` 状态 + `checkForUpdate()` 调用：
  - 无更新 → `notify('ok','已是最新版本')`；
  - 有更新 → 设置 `updateInfo`（打开 `UpdateDialog`）；
  - 出错 → `notify('err', ...)`。
- 关于 tab 的「保存关于」按钮触发 `saveAbout()`（沿用 sticky footer）。
- 关于 tab 内另有「立即检查更新」按钮（与保存按钮并列或独立行）。

### 3.4 新增 `UpdateDialog.svelte`（模态）
- props：`update: { version, body }`；`onclose` 事件。
- 遮罩 + 居中卡片，`--paper` / `--ink` / `--accent` 配色，复用 `app.css` 变量与按钮风格。
- 内容：标题「发现新版本 vX」、更新说明（`body`，markdown 可选，MVP 用 `<pre>`/白文本即可，避免引入渲染复杂度）、
  一行 Star 引导（链接用 `openUrl(GITHUB_URL)`）。
- 按钮：主按钮「立即下载并安装」（含下载进度：`downloadAndInstall()` 的 `onDownloadProgress`/`onInstallProgress` 回调更新进度条）、
  次按钮「稍后」。
- 安装成功后 `relaunch()`（失败 → toast 报错并关弹窗）。
- 进度态禁用所有按钮，防止重复点击。

### 3.5 启动自动检查（`src/routes/+layout.svelte`）
- `onMount` 里 `initConfig()` 完成后，读 `$config.autoCheckUpdate`；
  若为真则 `checkForUpdate().then(ui => ui.available && openUpdateDialog())`，**catch 静默**。
- 对话框为全局单例：在 `+layout.svelte` 渲染一个 `UpdateDialog`（受一个 layout 级 `$state` 控制），
  这样启动自动检查与关于 tab 的手动检查都能复用同一个弹窗。
  - 为此把「打开更新弹窗」做成一个共享 store（`src/lib/updater.ts` 导出 `updateDialog` writable store），
    任何处 `updateDialog.set({version,body})` 即弹；`UpdateDialog` 挂在 layout 监听它。

## 4. Rust / Tauri 配置改动

### 4.1 `src-tauri/Cargo.toml`
```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

### 4.2 `src-tauri/src/lib.rs`
```rust
.plugin(tauri_plugin_updater::Builder::new().build())
.plugin(tauri_plugin_process::init())
```
> updater 默认配置即可（端点/pubkey 来自 tauri.conf.json）。无需自定义 target/endpoint。

### 4.3 `src-tauri/tauri.conf.json`
```jsonc
{
  "bundle": {
    "createUpdaterArtifacts": true   // 追加
  },
  "plugins": {
    "updater": {
      "pubkey": "<USER_PUBLIC_KEY>",
      "endpoints": [
        "https://github.com/Cyy-0430/daily-report-app/releases/latest/download/latest.json"
      ]
    }
  }
}
```
> `pubkey` 占位由用户生成密钥后填入（见 prd「用户侧前置依赖」）。开发期占位可为空串？
> **不可**——updater 插件在 builder 阶段可能校验非空。处理：留一个明显占位字符串
> `"<REPLACE_WITH_PUBKEY>"` 并在文档/检查清单标注，dev 不触发 check 即不影响构建。

### 4.4 `src-tauri/capabilities/default.json`
permissions 追加：
```json
"updater:default",
"process:default"
```
> `process:default` 含 relaunch 权限；若收紧可用 `process:allow-restart`/`process:allow-exit`，
> MVP 用 `process:default`。

### 4.5 npm 依赖
```bash
pnpm add @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

## 5. CI 改动（`.github/workflows/release.yml`）

在 `tauri-apps/tauri-action` 步骤 env 加：
```yaml
TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```
> 设了 `createUpdaterArtifacts: true` + updater 配置后，tauri-action 会自动生成 `latest.json`
> 及签名安装包并上传到该 Release，作为 updater 的 endpoint 数据源。
> 两个 secret 由用户在 GitHub Secrets 配置（prd 前置依赖）。

## 6. 数据流（启动自动检查）

```
layout onMount
  └─ initConfig() → config store 就绪
      └─ if config.autoCheckUpdate:
          └─ checkForUpdate()            # updater.check()
              ├─ throw        → catch 静默 (不打扰)
              ├─ !available    → 静默
              └─ available     → updateDialog.set({version,body}) → 弹窗
```

## 7. 兼容性 / 风险

- **旧配置升级**：新字段 `#[serde(default = "default_true")]` + `emptyConfig` 默认 true → 既有用户无感知升级。✅
- **dev 构建无签名**：`check()` 在未签名/未安装构建下通常抛错或无更新 → 启动静默、手动 toast 报错，可接受。
- **pubkey 占位**：未填真实公钥时，端到端验证不通；但 app 仍可正常构建运行（只要不触发 check 成功路径）。
  → 在 implement 检查清单中标记「需用户填 pubkey + 配 secret 才能端到端验证」。
- **macOS 限制**：updater 替换安装要求 app 位于 `/Applications`；非该位置可能失败。属平台已知行为，文档备注。
- **首次无 `latest.json`**：当前仓库 Release 无该文件 → 首个带 updater 配置的 tag 发布后才生成。
  此前 `check()` 会 404 抛错 → 启动静默，符合 R4。

## 8. 验证策略

- 单元：`config.rs` 序列化往返测试（旧 JSON 缺 `autoCheckUpdate` → 反序列化得 `true`）。
- 类型：`pnpm check`（svelte-check）。
- Rust：`cargo test` + `cargo check`。
- 手动：dev 下点「立即检查更新」→ 预期 toast「已是最新版本/报错」（无签名）；端到端需用户配齐密钥后打 tag 验证。
