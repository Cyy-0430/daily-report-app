# 设置增强:主题分享 + 网络代理

## Goal

两个独立交付物:① 主题可导入导出(JSON)以便分享;② 网络代理配置,作用于 API 请求与检查更新。

## Requirement Set(来源:用户 2026-08-21)

1. 主题导入导出
   - 导出:点击导出时生成主题 JSON(复制到剪贴板分享)。
   - 导入:点击导入弹出输入框,粘贴 JSON 后新建自定义主题。
2. 网络代理
   - 设置页 API 页新增网络代理配置,例如 `127.0.0.1:7890`。
   - 代理同时作用于 API 请求(测试连接/生成)与检查更新(检测+下载)。

## Task Map(子任务)

| 子任务 | 交付物 | 层 |
|---|---|---|
| 08-21-theme-import-export | 主题 JSON 导出(剪贴板)+ 导入弹窗 + 校验/白名单过滤 | 前端 |
| 08-21-network-proxy | ApiConfig.proxy 字段 + Rust reqwest 代理 + updater 代理 | 前端+Rust |

两子任务相互独立,无先后依赖;可分别实现、验收、归档。

## Cross-child Acceptance Criteria

- [ ] 两个子任务各自的验收标准全部通过(见各子任务 prd.md)。
- [ ] `pnpm check`、`cargo test` 全绿;`src/lib/bindings.ts` 与 Rust 结构无漂移。
- [ ] 版本号按惯例 bump(package.json + tauri.conf.json),commit 遵循 `feat(scope): …` 惯例。

## Notes

- 本父任务不直接承载实现;集成审查(跨子验收 + 版本 bump)在两个子任务归档后进行。
