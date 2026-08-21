# 网络出口契约 (Network Spec)

> 沉淀自 08-21 网络代理任务。Rust `src-tauri/src/llm.rs` 的出站 HTTP 构造 + 前端
> `src/lib/updater.ts` 的更新检查链。任何发起新网络请求(加 IPC 命令、加下载、换 HTTP 库)前必读。

## Scenario: 应用出站流量统一走代理配置

### 1. Scope / Trigger

- 跨层契约变更:`ApiConfig` 新增 `proxy` 字段(Rust ↔ `bindings.ts` 手工镜像),影响全部 LLM 请求与更新检查/下载。
- 触发场景:新增任何出站 HTTP 请求、调整超时/代理、动 updater 检查链、给 `ApiConfig` 加字段。

### 2. Signatures

```rust
// llm.rs —— 唯一 Client 构造出口(全文件仅此一处 Client::builder)
fn normalize_proxy(proxy: &str) -> Option<String>  // trim;空→None;无 scheme 补 "http://";幂等
fn build_client(timeout: Duration, proxy: &str) -> Result<Client, String>
    // 空 proxy = 直连(与历史行为字节等价);parse 失败 → Err("代理配置无效（…）：…"),不静默直连

// config.rs —— 字段与归属
pub struct ApiConfig { base_url, api_key, model,
    #[serde(default)] pub proxy: String,  // 空 = 直连;放 ApiConfig(非顶层)使 test_connection 免签名改动即可测未保存表单值
}
```

```ts
// updater.ts —— 前端镜像
function normalizeProxy(proxy?: string): string | undefined  // 与 Rust normalize_proxy 规则一致
async function checkForUpdate(proxy?: string): Promise<UpdateInfo>
async function downloadAndInstallWithProgress(onProgress?, proxy?): Promise<void>  // 内部重新 check() 处同传
// 裸 check() 仅允许出现在 checkWithProxy 内;调用方(+layout 启动 / AboutTab 手动 / UpdateDialog 下载)
// 一律传 get(config).apiConfig.proxy || undefined
```

### 3. Contracts

- **单一出口**:Rust 侧所有出站 HTTP 必须 `build_client(超时, &api.proxy)`,禁止绕过裸建 `Client::builder()`(否则用户代理配置对它失效)。
- **超时不变量**:stream 120s / complete 120s / test_connection 30s;新请求显式选超时并写明。
- **两端归一化镜像**:裸 `host:port` 补 `http://`(reqwest/Url 不认裸地址);幂等;规则改动必须两侧同步 + 两侧各有单测。
- **失败语义**:代理配置非法 → 可读错误上抛(用户配代理通常有网络原因,静默直连 = 卡死或绕过)。
- **未配置 = 现状**:proxy 空时 Rust 不设 proxy、前端不传 check 选项,行为与引入前完全一致。
- 仅 HTTP(S) 代理(reqwest 未开 `socks` feature);`api_incomplete()` 不含 proxy(可选字段)。
- ApiConfig 加字段三同步:`#[serde(default)]` + `bindings.ts` 镜像(含 `emptyConfig()`)+ db.rs 测试字面量(编译器会逼);db 侧无新 KV key(`api_config` 整体 JSON)。

### 4. Validation & Error Matrix

| 条件 | 行为 |
|---|---|
| proxy 空/纯空白 | 直连(不设 proxy / 不传 check 选项) |
| `127.0.0.1:7890` 裸地址 | 补 `http://` 后生效 |
| 代理串 parse 失败 | `Err("代理配置无效（url）：原因")`,请求不发出 |
| 旧配置无 proxy 字段 | serde default = 空串 = 直连,无损升级 |
| 前端调用方漏传 proxy | 该调用点直连(评审用 grep `checkForUpdate(` 核对全调用点) |

### 5. Good / Base / Bad Cases

- **Good**:新增出站请求 → 直接调 `build_client`,天然继承代理与错误语义。
- **Base**:测试连接用表单值 `testConnection({ ...api })`,未保存的代理立即生效。
- **Bad(反例)**:`Client::builder().build()` 裸建;`Proxy::all(url).ok()` 吞错回退直连;updater 里裸 `check()`。

### 6. Tests Required

- `llm.rs`:`normalize_proxy` 空/空白/裸 host:port/带 scheme(幂等)。
- `config.rs`:legacy JSON 缺 proxy → 空串;round-trip 序列化含 `"proxy":` 键。
- `db.rs`:`config_roundtrip` 断言 proxy 等价(加字段时同步补)。

### 7. Wrong vs Correct

```rust
// Wrong:绕过出口,用户代理失效;parse 失败被吞
let client = Client::builder().timeout(t).build()?;
let proxy = Proxy::all(&api.proxy).ok();   // 错误静默丢弃

// Correct:唯一出口,错误显式上抛
let client = build_client(Duration::from_secs(120), &api.proxy)?;
```

```ts
// Wrong:裸 check(),更新检查绕过代理
const update = await check();

// Correct:经 checkWithProxy,读全局配置代理
const info = await checkForUpdate(get(config).apiConfig.proxy || undefined);
```

---

## 关联

- 任务来源:`.trellis/tasks/08-21-network-proxy/`(prd/design)。
- `ApiConfig` 存储见 `storage-spec.md` §3a(`api_config` 整体 JSON,无新 KV key)。
- updater 插件 `check({ proxy })` 为官方能力(Tauri v2 plugin-updater)。
