# 设置页新增关于tab与自动更新检查

## Goal

在设置页新增第 4 个 tab「关于」，集中展示 app 信息（名称 / 版本 / 作者 / GitHub 仓库），
并提供「自动检查更新」开关（默认开启）与「立即检查更新」按钮。当检测到新版本时，
弹出对话框展示版本与更新内容，用户可选择「立即下载并安装」（走 Tauri updater 插件
在 app 内下载并重启安装）或「稍后」。对话框内同时建议用户去 GitHub 给项目点 Star。

## Background / Scope decisions（用户已确认）

1. **更新方式 = (b) app 内下载并安装**：使用 Tauri 2 官方 updater 插件（`@tauri-apps/plugin-updater`），
   `check()` → `downloadAndInstall()` → `relaunch()`（`tauri-plugin-process`），而非仅跳转浏览器下载。
2. **检查时机**：app 启动时（layout 挂载、配置加载完成后）若开关为开，自动检查一次；
   「关于」tab 内另有「立即检查更新」按钮供手动触发（无论开关状态）。
3. **元数据来源**：版本号取自运行时 `getVersion()`（`@tauri-apps/api/app`，值来自 `tauri.conf.json`，
   与打包产物一致）；GitHub 地址 `https://github.com/Cyy-0430/daily-report-app`；作者 `cyy`。

## Requirements

### R1 — 「关于」tab

- 在设置页 tab 栏追加「关于」（`about`），成为第 4 个 tab。
- 内容卡片展示：应用名（`日报生成` / `DailyReport`）、当前版本号（运行时 `getVersion()`）、
  作者（`cyy`）、GitHub 仓库地址（可点击，用 opener 打开外部链接）。
- tab 遵循现有「按页保存」模式：只有「自动检查更新」开关需持久化，其「保存」按钮保存该开关。

### R2 — 自动检查更新开关

- `AppConfig` 新增字段 `autoCheckUpdate: boolean`，**默认 `true`**。
  - Rust 侧 `#[serde(default = "default_true")]`，保证旧配置升级不丢字段（遵循 storage-spec 不变式）。
  - TS 侧 `bindings.ts` 同步字段，`emptyConfig()` 设为 `true`。
- 开关状态写入/读取 SQLite `config` 表（与其它配置同路径，无新表）。
- 关 = 关：启动时不自动检查；开 = 默认：启动时自动检查一次。手动按钮不受开关影响。

### R3 — 更新检测与安装（选项 b）

- 集成 Tauri updater 插件：Rust 注册 `tauri-plugin-updater` + `tauri-plugin-process`；
  capabilities 加 `updater:default` + `process` 重启权限；npm 加两个插件包。
- `tauri.conf.json`：`bundle.createUpdaterArtifacts = true`，`plugins.updater.pubkey` + `endpoints`
  指向 GitHub Releases 的 `latest.json`。
- 启动自动检查 + 手动检查均调用统一封装 `checkForUpdate()`（`src/lib/updater.ts`）：
  返回 `{ available, version?, body? }` 或在出错时抛出。
- 检测到新版本 → 弹出 `UpdateDialog`（模态）：展示新版本号、更新说明（release body）、
  「立即下载并安装」「稍后」按钮，以及一行「给项目点 ⭐ Star」提示（链接 opener 打开仓库）。
- 用户点「立即下载并安装」→ `downloadAndInstall()`（含下载进度条）→ 成功后 `relaunch()`。

### R4 — 行为细节（容错 / 不打扰）

- 启动自动检查失败（网络/未签名 dev 构建等）→ **静默失败**，不弹 toast、不打扰用户。
- 手动检查：无新版本 → toast「已是最新版本」；出错 → toast 报错。
- 对话框为模态遮罩，符合 Editorial Paper 设计系统（`--paper` / `--ink` / `--accent`）。

## Acceptance Criteria

- [ ] 设置页出现第 4 个 tab「关于」，展示应用名、版本、作者、GitHub 链接，链接点击可在外部浏览器打开仓库。
- [ ] 「关于」tab 有「自动检查更新」开关，默认开启，保存后持久化；重启 app 后开关状态保留。
- [ ] 开关开启时，app 启动会自动检查一次更新（无新版本/失败均静默）。
- [ ] 「立即检查更新」按钮可用：无新版 → toast 提示最新；有新版 → 弹出更新对话框。
- [ ] 更新对话框正确展示新版本号与更新说明，含「立即下载并安装 / 稍后」与 Star 引导。
- [ ] `AppConfig` 新字段两端同步且旧配置可无损升级（`#[serde(default)]` 生效，相关单测通过）。
- [ ] updater / process 插件已在 Rust + capabilities + npm + tauri.conf.json 四处正确配置。
- [ ] `pnpm check` 通过、`cargo test` 通过、`cargo check` 通过。
- [ ] 发布 CI（`release.yml`）已配置签名密钥环境变量，`createUpdaterArtifacts` 产物正确生成。

## 用户侧前置依赖（非代码，需用户本人完成）

> 选项 (b) 的**端到端真实更新**依赖一个签名密钥对，此项必须由仓库 owner（用户）生成并托管，
> 否则 CI 无法生成可被 updater 验证的签名安装包（代码已就绪但更新链路不通）。

1. 生成密钥：`pnpm tauri signer generate -w ~/.tauri/dailyreport.key`
   （输出公钥 + 私钥文件；若设了密码则一并记录）。
2. 把**公钥**填入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`（本任务会在 design 中留好占位）。
3. 在 GitHub 仓库 **Settings → Secrets** 添加：
   - `TAURI_SIGNING_PRIVATE_KEY` = 私钥文件内容；
   - （若设密码）`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = 密码。
4. 之后每次打 `v*` tag，`tauri-action` 会自动生成 `latest.json` 与签名安装包并发布到 Release，
   updater 即可据此检测/下载/安装。

## Out of Scope

- 不做后台定时轮询（仅启动时一次 + 手动触发）。
- 不做更新渠道（beta/stable）切换。
- 不做多语言 i18n（沿用现有中文文案风格）。
- 不改动周报/采集/日报任何既有流程。
