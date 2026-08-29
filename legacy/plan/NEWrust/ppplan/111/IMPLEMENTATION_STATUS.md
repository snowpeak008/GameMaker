# NEWrust 独立化实施状态

更新时间：2026-07-12

当前结论：独立化代码与全部自动化门禁已完成，独立 Git 主线为 `main`，当前 HEAD 为 `a8750a5e438a7db9133f5767ff7a2bc3238c8053`。schema-v2 正式证据为 `passed`，21/21 检查通过；Rust `release-gate` 为 `passed`、blocker 为 0。正式 portable 绑定事务 `36a8d3473bf345ad9553e4ba20c843e2`，回执状态 `finalized`。仅 ATOM-023 干净 Windows GUI 外部验收未执行，因此仍不创建 `standalone-v1` 标签。

## 已完成

- 独立根、资源清单、Rust/Node 锁定工具链和 x64 静态 CRT 契约。
- design data、schemas、market、SDK、Skill、artifact registry、UI baseline 的独立权威副本。
- SourceProjectRoot 与 PortableResourceRoot 分离；删除父目录、固定目录名、编译机路径和 cwd 祖先回退。
- project-owned 相对路径迁移、machine-binding 重验证、启动前事务恢复和幂等迁移。
- SDK/Skill seed + overlay + tombstone 数据模型及 CLI、应用、桌面跨层接入。
- 流水线 Step00、Step13/14 的可迁移路径身份和证据协议。
- 独立 knowledge 资源发现与只读 freshness 行为。
- portable build、stage/live/backup/failed 事务、输出锁、崩溃恢复、回执保留和资源完整性校验。
- 默认 dry-run 的受控清理、cleanup lease、tombstone 恢复、正式证据保护和 `user_data` 防删边界。
- 无共享对象 Git clone、中文+空格路径、反伪造扫描、21 项 schema-v2 正式证据协议。
- Windows PowerShell 5 进程数组兼容修复；portable GUI smoke 改为显式进程句柄等待、UTF-8 stdout/stderr、真实退出码和有界超时回收。

## 已通过验证

- Web unit、i18n（2566 keys）、design content（1655 values）、E2E、language、90 张 UI 截图和 93 条 baseline。
- `cargo fmt --check`、`cargo check --workspace --locked -j1`、`cargo test --workspace --locked -j1`。
- Rust governance 定向测试 46/46。
- portable 事务夹具 23/23；cleanup 夹具 14/14、lease 夹具 9/9。
- verifier `-SelfTest`：21 项契约、242 个扫描命中、84 条精确白名单、零文件变更。
- 无共享对象 clean clone、`git fsck --full`、中文+空格改名根、资源清单和 standalone boundary。
- 真实 portable smoke runner：退出码 0、等待真实进程约 908ms、输出完整、残留进程 0。
- 正式 verifier：21/21 passed，绑定当前 HEAD、正式 portable、资源摘要、x64 静态 CRT、finalized swap receipt 和 cleanup-last 证据。
- Rust `release-gate` 消费者：`status=passed`、`blocker_count=0`。
- G6 清理：删除 gate 1 个文件、node_modules 175 个文件、29 个历史测试目录（2851 个文件、45,727,157B）；source target、Web 产物、项目临时目录和活跃应用进程均为 0。
- 受保护 `user_data` 保持 291 个文件、13,954,471B，tree digest 为 `7778cac532c1fcf1798191f9d2eec7093a77d0293e2d59a9f5a1db7da35468ae`。

## 正式门禁发现并修复的问题

1. Windows PowerShell 5 对 `List[object]` 的 `@(...)` 转换会触发 `Argument types do not match`；改为显式 `ToArray()` 并加入回归夹具。
2. Windows GUI subsystem EXE 由 PowerShell `&` 启动时可能在进程退出前返回；构建与 verifier 现共用同步 smoke runner。
3. 一轮正式构建曾受两个非本项目 TriposR worker 占用约 8.56GB 私有内存而 OOM；外部进程自行退出后，在不降低优化等级的前提下完整重跑通过。
4. Rust `release-gate` 曾把普通 `E:\...` 回执路径与 canonical `\\?\E:\...` 非存在事务路径误判为不一致；改为从最近存在祖先安全 canonicalize，仅允许绝对、无遍历、无 reparse 的缺失尾段，并新增正负回归测试。

## 仍需按顺序完成

1. 在 Windows Sandbox、干净 VM 或第二台电脑执行 ATOM-023：无 Rust/Node/Git/旧项目条件下启动、WebView ready、主要 UI、创建存档、退出和重启。
2. ATOM-023 通过后才创建幂等 `standalone-v1` 标签；当前不得打标签。
3. 用户给出最终目标目录后，可整目录移动独立源码仓库或复制正式 portable；当前不剪切原目录。

## 保留与禁止

- 保留一个明确的正式 portable、最终正式证据、独立源码和受保护用户数据。
- 不设置 remote，不 push，不删除旧项目资源，不移动源码目录（目标路径尚未指定）。
- 不以 blocked、旧提交或手工修改的证据代替正式门禁。
