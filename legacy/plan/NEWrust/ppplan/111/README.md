# NEWrust 完全独立化原子开发计划

状态：ATOM-000～022、ATOM-024 的代码与自动门禁已完成；ATOM-023 仍需干净 Windows 外部验收，完成前不创建最终标签。实时实施状态见 [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md)。

评审记录：架构独立性、Git/清理安全、跨机器运行三路复核均通过；执行中不得弱化 protected/ephemeral/backup 边界。

执行协议：严格按编号顺序。ATOM-000/001 发生在独立 Git 初始化前，禁止 add/commit，其成果统一进入 ATOM-002 基线；初始化前只允许 `git -C <parent-root>` 的显式只读检查。ATOM-002 完成后，每项采用“实现 → 定向验证 → 垃圾清理 → Git index 审计 → checkpoint commit”。硬门禁失败立即停止。正式目标路径未给出不阻塞当前目录独立化，但阻止最终剪切旧目录。

## ATOM-000 冻结边界与保护证据

- 盘点全部父依赖、运行数据组和持久化绝对路径。
- 记录 design data、Schema、artifact layer、market data、SDK seed、Skill seed、UI baseline 的相对路径/字节/SHA-256。
- 记录当前 portable/user_data 文件数、字节和 tree digest；确认应用/锁不存活。
- G0 仅 dry-run，禁止实际删除。
- 完成：生成 `docs/independence/source-boundary-inventory.json`，清单可重复。

## ATOM-001 Git 忽略、属性与泄密防线

- 完善 `.gitignore`、`.gitattributes`。
- 递归忽略任意 `user_data`、dist、target、Web dist/node_modules、gate 输出、日志、dump、stage/previous、`.env*`、精确列名的本地 profile/config、证书和密钥；显式保留 `.env.example`、gate README、fixture，禁止 `*config*` 宽泛规则。
- 建立最小 secret/path fixture allowlist，精确到文件和值，包含理由、owner、到期条件；禁止整个测试目录豁免。
- 此时只编写规则，不执行依赖独立 Git 的 `git check-ignore`。
- 生成按文件 allowlist 的暂存脚本/流程；禁止 `git add .`。
- 完成：secret/path scan 和 ignore tests 通过。

## ATOM-002 父仓库切割与独立 Git 基线

- 确认父仓库 index 不含 `NEWrust`；记录父 `.git/info/exclude` 摘要，仅在精确行缺失时加入 `/NEWrust/`，失败只撤销本次插入，不修改共享 ignore。
- `git init -b main`，断言独立 toplevel；后续只用 `git -C <root>`。
- 检查 repo-local 用户身份；缺失则停止请求配置。
- 现在运行 `git check-ignore`，证明保护项被忽略、构建配置/README/fixture 可跟踪。
- 按 allowlist 暂存，扫描 staged blobs，提交迁移前基线。
- 验收：父仓库不再报告 NEWrust，独立 HEAD 存在且不含 dist/user_data/secret。

## ATOM-003 安全垃圾清理工具（先实现后清理）

- 新增 `tools/clean-generated.ps1`，默认 dry-run，执行需显式开关。
- 仅 allowlist：Cargo target、Web dist、生成 gate、browser profile、已验证 stage、专用 temp。
- canonicalize 边界；拒绝 reparse、项目根/父根、`.git`、源码/资源/fixture/docs。
- 定义 `protected-user-data`：基线真实数据和用户指定 local release 数据，普通 cleaner 永远拒绝其本身及祖先；禁止递归删 `dist`。
- 定义 `owned-ephemeral-user-data`：任务在预先不存在的唯一 temp 根创建，带 nonce/owner manifest；仅在源摘要不变后可由普通 cleaner 删除。
- 定义 `owned-ephemeral-workspace`：clean clone/relocation 只能在预先不存在的专用 temp 根创建，owner manifest/nonce 必须位于 clone 外；专用 workspace finalizer 可删除该 clone 的 `.git`，但必须拒绝真实独立源码根。
- 含 protected data 的 backup 不进入普通 cleaner，只由 `Finalize-PortableSwap` 专用操作处理。
- 测试覆盖两类 data、祖先包含 protected data、伪 owner manifest、越界、symlink、重复执行、stage/previous 全量扫描。

## ATOM-004 首次受保护清理 G1

- 运行清理 dry-run，人工核对白名单，再执行。
- 前后重新计算真实 user_data 文件数/字节/tree digest，必须完全一致。
- 清理旧 target、Web build、生成 gates、无数据的过期输出；保留当前正式 portable 和全部用户数据。
- 提交小型清理摘要，删除大清单/日志。

## ATOM-005 权威资源一次性迁入

- 通过同盘 staging 复制：`design_data`、`schemas`、`market_data`、`sdks` seed、`skills` seed、`artifact_layer`、UI baseline。
- 目标相同 no-op，不同硬停止；禁止 mirror-delete。
- 写版本化资源 manifest，不把 156/93 等本次快照数硬编码为永久协议。
- 明确 `ai_memory`、旧 decisions/governance/ucos 不迁入。
- 验收：manifest 摘要一致、无 reparse、所有必需资源进入 staged Git 文件清单。

## ATOM-006 可复现工具链与 CRT 契约

- 新增 `rust-toolchain.toml` 固定 Rust 1.96 和 MSVC target。
- 新增 `.cargo/config.toml` 使用静态 CRT；package engines 固定 Node/npm 支持范围。
- README 区分 portable runtime、source online build，不承诺未 vendor 的 offline source build。
- 新增 PE import gate，拒绝 VCRUNTIME 动态依赖；manifest 记录工具链/架构/OS/WebView2/lockfile 摘要。

## ATOM-007 独立项目根契约

- 定义 `SourceProjectRoot`：`.project_root` 版本化 manifest，校验 Cargo workspace、lockfiles 和 source resource manifest。
- 同时定义独立的 `PortableResourceRoot`/portable manifest；它不要求 Cargo workspace，不得复用 SourceProjectRoot validator。
- foundation 提供统一 root resolver/safe join/test helper。
- 根改名、中文/空格、伪父资源、显式错误 root 测试。
- G2：测试使用外置唯一 Cargo target/temp，finally 清理。

## ATOM-008 Web 数据根独立化

- `generate-design-content.mjs` 从 Web 上一级独立根读取资源，不再读父项目。
- 增加 root manifest、缺失、越界、改名测试。
- 执行 `npm ci`、design-content check、unit/build。
- G2/G3：清 `web/dist`、临时 node 输出和报告；是否保留 node_modules 由显式依赖缓存策略决定。

## ATOM-009 桌面资源与协议根独立化

- 生产资源顺序改为经过 PortableResourceRoot 校验的 portable/安装 root；开发显式 root 使用 SourceProjectRoot 校验。
- `ADM_NEWRUST_SOURCE_ROOT` 仅开发/测试可用且必须指向合法独立根。
- 删除 cwd/exe 任意祖先和编译期 `CARGO_MANIFEST_DIR` 发布回退。
- design data、Schema、registry 为 release 必需；fallback taxonomy 仅显式测试模式。
- 验收：完整模式通过、缺资源 smoke 失败、伪父资源不被读取。

## ATOM-010 Rust 测试资源根统一

- contracts、design、knowledge、pipeline、tauri commands 的所有 `../../..` 和 parent root 测试改用统一 resolver。
- 测试只读内部 knowledge/pipeline/testdata。
- static scan 无父资源拼接；相关 crate 与 workspace test 通过。
- G2：外置 Cargo target 和 temp 全清。

## ATOM-011 Portable 构建和恢复安全

- `build-portable.ps1` 只从独立根取全部资源，支持外置 target。
- clean release 与 local-data release 使用不同目录。
- 启动前扫描全部 stage/previous；stage 失败可清。
- 实现唯一 `Finalize-PortableSwap`：同时验证 transaction id、应用已退出且无锁、replacement smoke、live/backup/pre-swap 三方 user_data 摘要后才删除 backup；任一失败保留并硬停止。
- 恢复失败保留 backup、输出恢复路径、硬停止。
- 构建 manifest 写资源树、Git、工具链、PE、系统前提摘要。
- G4：只保留指定发布物，任何真实 user_data digest 不变。

## ATOM-012 UI baseline 独立化

- baseline 改到 `testdata/ui_baselines`。
- evidence path 全部 project-root-relative，不含固定 `NEWrust/`。
- active fixture/index 删除 `python_source`、旧 `source_plan`，替换为独立 UI contract ID；必要旧来源仅进入 `docs/migration-history`，不为 active baseline 建豁免。
- baseline/UI/language gates 在改名根通过。
- G3：清截图、browser profile、临时 manifest，只保留 baseline fixture 和小摘要。

## ATOM-013 Governance 根发现与输出路由

- governance 当前独立根即 repo root；移除 `repo_root/NEWrust`、固定目录名。
- gates 写内部相对输出根；CLI 可显式 `--project-root`。
- 从根、子目录、改名路径执行结果一致。

## ATOM-014 历史治理退役与独立 Release Gate

- 枚举 final handoff 强制读取的旧 Python/计划矩阵和命令。
- 必要历史摘要迁入 `docs/migration-history`，不作为构建/release 输入。
- 旧 parity/handoff 命令改为 deprecated 或从默认 doctor/release 移除。
- 新 release gate 仅聚合当前 format/Web/Rust/resource/boundary/portable/relocation 证据。
- 验收：父 `plan/NEWrust` 不存在仍可完成独立 release gate。

## ATOM-015 CLI 独立命令契约

- 移除 CLI 中 `repo_root/NEWrust/web|gates`。
- doctor/gates/dist/sdk 命令使用独立根和显式数据根。
- 旧 Ucos `memory` 命令从 help/doctor/release 移除；调用返回稳定 `legacy_memory_command_deprecated`，不得读取父目录。
- SDK/Skill 实现 overlay repository：只读 seed 低优先级、data-root 用户层高优先级；稳定 ID override、tombstone、seed 升级不覆写用户层、确定性合并索引、损坏隔离。
- 用户创建 SDK/Skill 写 runtime data root，不污染 Git。
- 命令 help、稳定错误码、根/子目录/改名回归测试通过。

## ATOM-016 持久化路径字段注册表

- 盘点 draft/save/index/run_context/source_artifacts/pipeline/checkpoint/patch/SDK/AI/CLI/log/lock 中所有绝对路径。
- 标注 project-owned、machine-binding、historical-display；记录是否参与寻址、迁移策略和敏感级别。
- 测试扫描普通盘符、JSON 转义、`\\?\`、UNC、`file://`；假密钥/坏路径 allowlist 必须版本化、带理由和到期条件。

## ATOM-017 启动前原子数据迁移

- schema-versioned migration 在正常加载前执行；project-owned 改为相对路径。
- machine bindings 缺失时标记需重配；AI secret 不打印不进入报告。
- before-image、同盘 staging、原子提交、失败回滚、幂等重启。
- 覆盖 active draft、formal save、index、custom templates、checkpoint/recovery、locks；历史 snapshot 的处置必须明确。
- 验收：新根首次启动即完成迁移；除批准的历史字段/机器绑定外不含旧根；数据摘要仅排除批准字段。
- 只清测试副本，真实 user_data 禁止修改。

## ATOM-018 外部工具失效与恢复矩阵

- 无 Unity/CLI、旧绑定失效、重新选择 Unity、重新发现 Codex/Claude 四类场景。
- 定义各功能可用性和 Step11-13 阻断边界。
- 验证失效不破坏设计/存档；重配后功能恢复且证据对应当前路径。
- 清全部测试 Unity/project/data 副本。

## ATOM-019 Standalone Boundary Gate

- 静态扫描父业务目录、固定根名、编译机绝对路径、path escape、外部 reparse。
- 动态测试拒绝伪父资源并验证资源 manifest。
- fixture allowlist 精确到文件/值，包含理由、owner、到期条件。
- 纳入默认 doctor 和 release gate。

## ATOM-020 原位置全量验证与 RC 提交

- npm ci、Web unit/i18n/e2e/language/UI；Cargo fmt/check/test；governance；portable build/smoke/PE；secret/path scan。
- 执行 G2/G3/G4，审计 index，建立 release-candidate commit，记录 hash。
- 项目无 target、web/dist、stage、generated gates；保留指定 RC 发布物和受保护 user_data。

## ATOM-021 无共享对象 Clean Git Clone 验证

- 从 RC commit 创建无 shared object 的本地 clone 到中文、空格、改名路径；不得复制工作树未跟踪文件。
- `git fsck --full`、HEAD 一致、工作树干净；所有权威资源均在 `git ls-files`。
- clone 中执行 npm ci、核心 Web/Rust/governance/portable build；父项目不可访问仍通过。
- G5：finally 通过 `owned-ephemeral-workspace` finalizer 删除 clone（含其 `.git`），并清 target、node_modules、测试数据和发布副本；伪 owner、nonce 不符或目标等于真实源码根必须拒绝。

## ATOM-022 异盘/长路径 Portable 与真实数据副本验证

- clean portable 复制到两个不同可写路径（含中文/空格、不同盘符或尽可能长路径），执行 smoke、启动/保存/退出/重启。
- local portable 使用真实 user_data 的校验副本，验证启动前迁移、旧绑定失效和重配恢复。
- 绝不修改真实 user_data；测试结束清副本并复核原 digest。

## ATOM-023 干净 Windows GUI 验收

- 优先 Windows Sandbox/干净 VM/第二台电脑。
- 从 clean portable 启动 launcher，验证 WebView ready、主要 UI、创建存档、退出、重启。
- 验证无 Rust/Node/Git，静态 CRT，无父项目；WebView2 缺失前置提示可操作。
- 环境不可用时明确标记外部验收未完成，禁止宣称跨电脑完成。

## ATOM-024 最终清理、交接与最终提交

- 运行 G6 dry-run/执行；核对保护数据；提交迁移说明、验证矩阵、垃圾/磁盘摘要。
- 全工作树、staged blobs、全部 reachable Git objects 扫描 secret 和旧机绝对路径。
- 创建最终提交；再次 clean clone 快速验证，并在 try/finally 中执行 G5：由 `owned-ephemeral-workspace` finalizer 删除 clone（含 `.git`），再删除 target、node_modules、测试 portable/ephemeral user_data；复核 protected data 摘要和源仓库工作树干净。
- 标签 `standalone-v1`：同 HEAD 则 no-op，指向其他提交则硬停止，禁止强制覆盖。
- 不设置 remote、不 push；目标路径未给出则不剪切原目录。

## 全局硬门禁

- 禁止读取独立根父目录业务数据。
- 禁止生产运行时使用编译机源码路径。
- 禁止 `git add .`、提交 user_data/secret/build output。
- 禁止删除旧项目资源来制造“切割完成”。
- 禁止普通 cleaner 删除 protected user_data 或其祖先；owned ephemeral 只能通过 owner manifest/nonce 临时根入口删除。
- 禁止普通 cleaner 删除任何 `.git`；只有 clone 外部 owner manifest/nonce 验证通过的 `owned-ephemeral-workspace` finalizer 能删除临时 clone，且永远拒绝真实独立源码根。
- 禁止 replacement 未 smoke 就删除唯一 backup。
- 禁止验证失败后 commit/tag 或继续下一个原子任务。
