# 设计:网络代理配置

## 字段归属:`ApiConfig.proxy`(而非顶层 AppConfig)

- 理由①:`test_connection(api: ApiConfig)` 只收 api 对象 —— proxy 在 ApiConfig 内,「用未保存表单值测试」零签名改动自动成立(前端 `testConnection({ ...api })` 已整对象传)。
- 理由②:llm.rs 全链(`stream_chat_once`/`complete_once`/`test_connection`)已持 `&ApiConfig`,零参数穿透改动。
- 理由③:db.rs `api_config` 整体 JSON 存储,`#[serde(default)]` 即完成无损升级,无新 KV key。
- 代价:updater(前端)读 `config.apiConfig.proxy`,语义略窄;注释写明「同时供更新检查使用」。可接受。

## 改动清单

### 1. Rust `src-tauri/src/config.rs`

```rust
pub struct ApiConfig {
    ...,
    /// HTTP(S) 代理,如 "http://127.0.0.1:7890" 或裸 "127.0.0.1:7890";空 = 直连。
    #[serde(default)]
    pub proxy: String,
}
```

### 2. Rust `src-tauri/src/llm.rs` — 单一出口 build_client

```rust
/// 归一化代理串:trim;空 → None;无 scheme → 前缀 "http://"(reqwest Url 不认裸 host:port)。
fn normalize_proxy(proxy: &str) -> Option<String> { ... }

/// 统一 Client 构造:超时 + 可选代理。代理 parse 失败 → Err(可读错误,不静默直连)。
fn build_client(timeout: Duration, proxy: &str) -> Result<Client, String> {
    let mut b = Client::builder().timeout(timeout);
    if let Some(url) = normalize_proxy(proxy) {
        b = b.proxy(Proxy::all(&url).map_err(|e| format!("代理配置无效({url}):{e}"))?);
    }
    b.build().map_err(|e| e.to_string())
}
```

- 三处 `Client::builder().timeout(..).build()`(stream_chat_once / complete_once / test_connection)全部改调 `build_client(现有超时值, &api.proxy)`。
- `api_incomplete()` **不含** proxy(代理可选)。

### 3. TS `src/lib/bindings.ts`

- `ApiConfig` 加 `proxy: string;`(注释同 Rust);
- `emptyConfig()` 的 apiConfig 加 `proxy: ''`。

### 4. 设置页 `ApiTab.svelte` / `+page.svelte`

- ApiTab「API 配置」区块加一个 `fld`:label「网络代理」、placeholder `例如 127.0.0.1:7890(留空不代理)`;`bind:value={api.proxy}`(api 是 $bindable 整对象,页面层 `$state` 初值来自 `{ ...c.apiConfig }` 自动含 proxy)。
- HelpTip 追加:代理同时用于检查更新;支持完整地址或 host:port;仅 HTTP(S)。
- 保存路径:页面 save() 已整对象写 apiConfig,无额外改动(实现时核对一遍)。

### 5. 前端 updater `src/lib/updater.ts`

- 前端归一化(与 Rust 同规则,幂等):`normalizeProxy(proxy?: string): string | undefined` 放 updater.ts(唯一消费方)。
- `checkForUpdate(proxy?: string)`:`check(proxy ? { proxy } : undefined)`。
- `downloadAndInstallWithProgress(onProgress, proxy?)`:重新 `check()` 处同样传(下载句柄同代理)。
- 调用方传参:
  - `+layout.svelte` 启动检查:`checkForUpdate(get(config).apiConfig.proxy || undefined)`;
  - `AboutTab.svelte` 手动检查:同上(AboutTab 已可读 config store;实现时核对)。

## 数据流

```
设置页表单 ──save──> AppConfig.apiConfig.proxy ──> SQLite api_config(JSON)
                                   │
       ┌───────────────────────────┼──────────────────────────┐
       ▼(Rust, 每次请求读 api.proxy)                          ▼(前端, check 时传)
  generate/test/weekly … build_client()                updater check({proxy})
```

## 权衡记录

- **不写系统代理自动探测**:用户显式配置,行为可预期;reqwest 默认也不读系统代理(本项目 default-features=false)。
- **归一化放两端**而非仅存储时归一:存量已保存的裸值、手工改库的值都兜得住;两端规则镜像、幂等。
- **失败不回退直连**:代理配错时明确报错优于静默绕过(用户配代理通常有网络原因,直连会卡死或泄漏)。

## 测试

- `config.rs` tests:legacy JSON(无 proxy)→ proxy == "";round-trip camelCase(`"proxy":` 键)。
- `llm.rs` tests:`normalize_proxy`:空/空白 → None;`127.0.0.1:7890` → `http://127.0.0.1:7890`;带 scheme 原样;`http://` 前缀大写容忍按 Url 规则(测小写即可)。
- 手工:配代理后测试连接 + 检查更新走代理(代理工具连接日志),清空恢复。

## 回滚

跨层但均为增量字段 + 新函数;revert 单 commit 即可。存量库已写入 proxy 值时 revert 后 `#[serde(default)]` 缺字段 → 直连,无残留影响(字段被整体 JSON 覆盖前仍在,但无人消费)。
