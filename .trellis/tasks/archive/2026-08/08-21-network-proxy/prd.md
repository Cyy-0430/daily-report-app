# 网络代理配置(API + 检查更新)

## Goal

设置页 API 页新增网络代理配置(如 `127.0.0.1:7890`),同时作用于 LLM API 请求与更新检查/下载;留空 = 直连(现状不变)。

## Requirements

### 配置与 UI

- API 设置页「API 配置」区块新增「网络代理」字段:placeholder 示例 `127.0.0.1:7890`,留空 = 不代理。
- HelpTip 说明:作用于 API 请求与检查更新;支持 `host:port`(按 HTTP 代理处理)或完整 `http://…` 地址;仅支持 HTTP(S) 代理。
- 随设置页整页保存(与 BaseURL/Key 同生命周期);「测试连接」用**当前表单值**(含未保存的代理)。

### 生效范围(Rust,LLM 请求)

- `test_connection`、日报 `generate_report`(stream_chat_once)、周报 map(complete_once)/reduce 全部经代理。
- 代理非法(如 parse 失败)时返回可读错误,不静默直连。

### 生效范围(前端,更新)

- `checkForUpdate` / `downloadAndInstallWithProgress` 的 updater `check()` 传 `proxy` 选项(插件官方支持;下载用同一 check 句柄,亦走代理)。
- 调用方(+layout.svelte 启动自动检查、AboutTab 手动检查)从 config 读代理传入。
- 未配置代理时行为与现状完全一致(不传 proxy 选项)。

### 兼容

- 旧配置无该字段 → `#[serde(default)]` 回退空串(直连),无损升级(项目惯例:新增 config 字段必须 default)。

## Acceptance Criteria

- [ ] 配置 `127.0.0.1:7890` 后:测试连接、生成日报/周报请求、检查更新均经该代理(可用本地代理工具日志验证);清空配置恢复直连。
- [ ] 输入 `http://127.0.0.1:7890` 与裸 `127.0.0.1:7890` 行为等价(归一化补 scheme)。
- [ ] 旧 SQLite 配置(无 proxy 字段)加载不报错,行为 = 直连;`bindings.ts` ApiConfig 与 Rust ApiConfig 字段一致。
- [ ] Rust 侧归一化函数有单测(空/裸 host:port/带 scheme);`cargo test`、`pnpm check` 全绿。

## Notes

- reqwest 未开 `socks` feature → 仅 HTTP(S) 代理(7890 类混合端口走 HTTP 协议即可,HelpTip 已注明)。
- updater 端点是 GitHub releases,是本需求的主要动机(网络可达性)。
