# Phase 基线报告模板

- 日期/时区：
- 操作者：
- Git commit：
- 工作树说明：
- 数据根：临时目录（填写模式，不填写真实用户绝对路径）
- 是否访问真实网络：否
- 是否读取/写入真实 `settings`、`drafts`、`saves`：否

## 环境

| 项 | 值 |
| --- | --- |
| OS | |
| Rust/Cargo | |
| Node/npm | |
| WebView/Playwright | |

## 命令与结果

| 门禁 | 命令 | 状态 | 测试数/耗时 | 失败归属 | 证据摘要 |
| --- | --- | --- | --- | --- | --- |
| Rust fmt | `cargo fmt --check` | | | | |
| Rust check | `cargo check --workspace` | | | | |
| Rust tests | `cargo test --workspace` | | | | |
| Web unit | `npm.cmd test` | | | | |
| Web build | `npm.cmd run build` | | | | |
| Web e2e | `npm.cmd run e2e` | | | | |
| i18n | `npm.cmd run i18n-test` | | | | |
| language | `npm.cmd run language-gate` | | | | |
| UI gate | `npm.cmd run ui-gate` | | | | |
| baseline UI | `npm.cmd run ui-baseline-gate` | | | | |
| fixtures | `node testdata/fixplan/scripts/verify-fixtures.mjs` | | | | |
| terminology warning scan | `node testdata/fixplan/scripts/terminology-scan.mjs` | | | | |

## 隔离与安全检查

- [ ] 测试使用新建临时数据根。
- [ ] 测试前后真实设置/存档时间戳未变化。
- [ ] 无真实 Key、环境变量值、Base64 或用户绝对路径进入报告/截图。
- [ ] 自动测试未访问真实外网。
- [ ] 失败项只登记归属，本轮没有顺手改变业务行为。

## 已知失败

| ID | 首次命令 | 最小复现 | 归属 | 是否阻断本 Phase | 后续任务 |
| --- | --- | --- | --- | --- | --- |

## 结论

填写“通过 / 有已登记失败 / 证据不足”，不得把未运行写成通过。

