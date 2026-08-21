# 实施计划:网络代理配置

执行者:trellis-implement。前置:`task.py start` 已将本任务置为 in_progress。
关键契约见 design.md;改 config 字段前已核对 `.trellis/spec/backend/storage-spec.md` 与 guides「改常量先搜索」规则。

## 顺序清单

1. [ ] `src-tauri/src/config.rs`:ApiConfig 加 `#[serde(default)] pub proxy: String`;tests 补 legacy-缺字段默认空 + round-trip camelCase 两例(仿现有 auto_check_update 用例写法)。
2. [ ] `src-tauri/src/llm.rs`:
   - `use reqwest::Proxy;`
   - `normalize_proxy(&str) -> Option<String>` + `build_client(Duration, &str) -> Result<Client, String>`(见 design §2,含可读错误);
   - `stream_chat_once` / `complete_once` / `test_connection` 三处 Client 构造替换为 build_client(超时保持原值 120s/120s/30s);
   - `normalize_proxy` 单测(空/空白/裸 host:port/带 scheme)。
3. [ ] `src/lib/bindings.ts`:ApiConfig 加 `proxy: string`;emptyConfig() 补 `proxy: ''`。
4. [ ] `src/lib/components/settings/ApiTab.svelte`:「API 配置」区块新增网络代理 fld(placeholder/HelpTip 文案见 design §4),`bind:value={api.proxy}`。
5. [ ] `src/routes/settings/+page.svelte`:核对 `$state` 初值 `{ ...c.apiConfig }` 与 save() 均覆盖 proxy(应自动成立,仅核对;若 save 是显式列字段则补)。
6. [ ] `src/lib/updater.ts`:`normalizeProxy` + `checkForUpdate(proxy?)` + `downloadAndInstallWithProgress(onProgress, proxy?)`(两处 check 都传)。
7. [ ] `src/routes/+layout.svelte` 与 `src/lib/components/settings/AboutTab.svelte`:checkForUpdate 调用处传 `get(config).apiConfig.proxy || undefined`(AboutTab 若未 import config store 则补)。
8. [ ] 验证命令全绿。

## 验证命令

```bash
cargo test          # config round-trip + normalize_proxy
cargo check
pnpm check
pnpm test           # 防前端回归
```

手工(有代理工具时):设置代理 → 测试连接/检查更新走代理;填非法串(如 `://`)→ 测试连接报「代理配置无效」;清空 → 直连恢复。

## 审查门

- bindings.ts ↔ config.rs ApiConfig 字段逐一对齐(proxy 是唯一新增)。
- 确认没有其它 `Client::builder` 残留(grep)。
- api_incomplete 未被改动(代理可选)。

## 回滚点

单 commit;revert 后旧库缺 proxy 字段由 serde default 兜底直连。
