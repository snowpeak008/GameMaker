# NEWrust 项目完全独立化设计计划

状态：独立化代码与全部自动门禁完成；干净 Windows GUI 外部验收尚未执行，因此暂不创建最终标签。实施明细见 [`111/IMPLEMENTATION_STATUS.md`](111/IMPLEMENTATION_STATUS.md)。

评审记录：架构独立性、Git/清理安全、跨机器运行三路复核均通过；所有首轮、二轮问题已写入本文硬门禁和原子计划。

设计目标：把 `NEWrust` 从 AutoDesignMaker 旧项目中彻底切割，形成可独立版本控制、可任意改名/移动、可在另一台满足运行前提的 Windows 电脑上构建或直接运行的第二代 Rust 项目。

## 1. 交付物定义

独立化不是只让 Cargo 能解析，而是同时交付两种独立产品：

1. `standalone-source`：干净 Git clone 不补拷任何父项目文件即可执行 Web 构建、Cargo 构建/测试、治理 gate 和 portable 打包。源码构建允许联网下载 lockfile 固定的依赖；离线源码构建不在本轮承诺范围。
2. `portable-runtime`：完整发布目录可复制到另一台 Windows x64 电脑的任意可写本地目录直接运行，不要求安装 Rust、Node、Git，也不要求旧项目或源码存在。

portable 的明确系统前提：Windows 10/11 x64 和 WebView2 Runtime。发布构建采用 MSVC CRT 静态链接，PE 依赖 gate 必须证明不再导入 `VCRUNTIME140.dll`、`VCRUNTIME140_1.dll`；WebView2 缺失由 launcher 前置检查给出可操作提示。

## 2. 已确认的现状

| 维度 | 当前结论 | 独立化要求 |
| --- | --- | --- |
| Cargo | 18 个 member 均在 `apps/`、`crates/` 内；无外部 path dependency | 保持 |
| Git | 当前无 `.git`，Git 顶层仍是父仓库 | 建立独立仓库、切断父仓库索引视图 |
| 核心数据 | Web/桌面/测试读取父目录 design data | 数据进入独立根并成为唯一权威副本 |
| 其他运行数据 | knowledge crate 还声明 `market_data`、`sdks`、`skills` 为 RuntimeLoaded | 分别确定只读 seed 与可写用户数据边界 |
| 协议 | portable 从父目录读取 schemas、artifact registry | 协议资源进入独立根 |
| 治理 | 深度绑定 `plan/NEWrust`、`repo_root/NEWrust` 和 Python 迁移证据 | 重建独立 release gate，旧迁移 gate 退役 |
| 运行时 | portable 相对定位；开发态仍有编译机路径/cwd 祖先回退 | 删除发布运行时的编译机和任意祖先回退 |
| 持久化 | draft meta、source artifacts、机器绑定可能含绝对路径 | 建立路径字段注册表和版本化迁移 |
| 垃圾 | Cargo/Web/gate/package 会产生大量可重建文件 | 清理工具先于任何实际清理，阶段化强制执行 |

## 3. 独立性不变量

1. 项目父目录不需要存在 AutoDesignMaker、Python 项目、`knowledge`、`pipeline` 或 `plan`。
2. 项目根名称不是协议；可以改成任意合法目录名。
3. 构建、测试、Web、治理、打包不得读取独立根以外的业务文件。
4. 发布程序不得使用编译机源码路径或任意 cwd 祖先作为资源候选。
5. 权威业务资源只能在独立 Git 中有一份；旧项目只作为一次性迁移源。
6. `target`、Web build、依赖缓存、gate 输出、dist、日志和用户数据不得进入 Git。
7. portable 必须整目录自包含；只移动 EXE 不是受支持交付方式。
8. 外部 Unity、Codex、Claude、API 是机器能力。缺失或旧绑定失效时稳定阻断对应功能并允许重新配置，不得崩溃或伪造成功。
9. 任何清理、构建交换和失败恢复都不能删除真实 `user_data`、正式存档或唯一 backup。
10. 干净 Git clone 是源码独立性的唯一验收输入，禁止用工作树复制掩盖未跟踪依赖。

## 4. 目标目录模型

```text
<independent-root>/
├── .git/
├── .cargo/config.toml
├── .gitignore
├── .gitattributes
├── .project_root
├── rust-toolchain.toml
├── Cargo.toml
├── Cargo.lock
├── README.md
├── AGENTS.md
├── apps/
├── crates/
├── web/
├── tools/
├── docs/
│   ├── independence/
│   └── migration-history/       # 仅保留必要历史说明，不参与 release gate
├── knowledge/
│   ├── design_data/
│   ├── schemas/
│   ├── market_data/
│   ├── sdks/                    # 只读 seed
│   └── skills/                  # 只读 seed
├── pipeline/artifact_layer/
├── testdata/
│   ├── fixplan/
│   └── ui_baselines/
├── gates/README.md
└── dist/                        # Git 外本地发布物
```

## 5. 数据所有权决策

| 数据组 | 目标位置 | 处置 |
| --- | --- | --- |
| `design_data` | `knowledge/design_data` | 必需、只读、跟踪 |
| `schemas` | `knowledge/schemas` | 必需、只读、跟踪 |
| `artifact_layer` | `pipeline/artifact_layer` | 必需、只读、跟踪 |
| `market_data` | `knowledge/market_data` | 跟踪的 seed；运行时不得覆写 |
| `sdks` | `knowledge/sdks` | 跟踪的 seed；用户新增 SDK 写入 data root，不写源码 |
| `skills` | `knowledge/skills` | 跟踪的 seed；用户/会话扩展写入 data root |
| UI baseline | `testdata/ui_baselines` | 测试 fixture、跟踪 |
| `ai_memory`、旧 decisions/governance/ucos | 不整体迁入 | 旧项目开发记忆，不是第二代运行依赖 |
| 用户存档/设置/AI 密钥 | portable `user_data` 或 Tauri app data | 永不跟踪 |

迁移采用 staging + 相对路径/字节数/SHA-256 清单校验。目标不存在则原子落位；目标相同则 no-op；目标不同则硬停止，禁止 mirror-delete 覆盖。迁移完成后旧项目不再是同步源。

旧 CLI `memory` 命令依赖 Ucos 目录和旧项目 identity/Skill/context，因此从独立产品正式退役：从默认 help、doctor、release gate 移除；调用时返回稳定 `legacy_memory_command_deprecated`，不得静默读取父目录。第二代记忆系统如有需要另立设计，不在本轮用兼容层偷渡旧 Ucos。

SDK/Skill 使用版本化 overlay repository：

- tracked seed 是只读低优先级层；data-root overlay 是可写高优先级层；
- 同 ID 的用户记录可以显式覆盖 seed，删除 seed 以 tombstone 表示；
- seed 升级不得覆写用户记录/tombstone；
- 合并索引按稳定 ID 排序并记录 seed/overlay 版本；
- overlay 损坏只隔离对应记录并报告，不得回写源码 seed。

## 6. 根目录与资源解析

### SourceProjectRoot

- `.project_root` 升级为版本化 source manifest，校验 Cargo workspace、lockfiles 与 source resource manifest。
- crate、Web、CLI、governance 共用同一项目根语义，不再自行拼 `../../..`。
- CLI 支持显式 `--project-root`，默认从 cwd 向上寻找根清单。
- 目录名 `NEWrust` 只允许出现在产品显示或历史说明，不参与路径决策。

### PortableResourceRoot

portable 不含 Cargo workspace，使用独立的 `build-manifest.json`/resource manifest，不能套用 SourceProjectRoot 校验。

1. portable launcher 设置的 resource root，必须与 executable 相邻并通过 portable manifest 校验；
2. 非 portable 安装形态使用安装包声明的资源根；
3. `ADM_NEWRUST_SOURCE_ROOT` 仅用于开发/测试显式 override，且目标本身必须是合法独立根；
4. 禁止任意 cwd/exe 祖先搜索和 `env!("CARGO_MANIFEST_DIR")` 发布回退。

核心资源缺失时 release smoke 必须失败。内置 fallback taxonomy 只允许显式测试模式使用，不能让缺资源发布包被判定为健康。

### 数据根

- portable：`<portable>/user_data`；
- 非 portable：Tauri app-data；
- 测试：操作系统临时唯一目录；
- 源码根不接收用户生成的 SDK、Skill、存档、日志或流水线输出。

## 7. 构建与工具链

- `rust-toolchain.toml` 固定 Rust 1.96、`x86_64-pc-windows-msvc`。
- `.cargo/config.toml` 为 Windows MSVC 设置静态 CRT；PE import 测试防止回退。
- `web/package.json` 声明 Node/npm engines；源码恢复流程使用 `npm ci`，Playwright 浏览器安装是 UI gate 前提。
- `Cargo.lock`、`package-lock.json` 必须跟踪；manifest 记录 lockfile 摘要、Git commit、Rust/Node/npm、目标架构、OS 下限和资源树摘要。
- `generate-design-content.mjs` 只从独立根读取数据。
- `build-portable.ps1` 只从独立根取资源，支持外置临时 Cargo target。

源码 online build 可下载依赖；portable runtime 运行时不因构建依赖联网。AI/API 功能是否联网由用户配置决定。

## 8. Portable 交换与恢复

- clean release 与带本机数据的 local release 使用不同输出目录。
- stage 失败可自动删除；backup 可能是唯一用户数据恢复副本，不进入普通 cleaner。
- backup 只能由专用 `Finalize-PortableSwap` 处理，并同时校验 transaction id、应用已退出/无锁、replacement smoke、live/backup/pre-swap 三方用户数据摘要；全部通过后才能删除。
- 恢复失败时保留 backup、报告人工恢复路径并硬停止。
- 启动构建前扫描全部 `.stage-*`、`.previous-*`，不只检查当前 PID。
- `protected-user-data` 是基线发现的真实数据和用户选择的 local release 数据，普通 cleaner 永远拒绝其本身及祖先。
- `owned-ephemeral-user-data` 必须由本次任务在预先不存在的唯一 temp 根创建，携带 nonce/owner manifest；仅在源摘要不变后可由普通 cleaner 删除。
- `owned-ephemeral-workspace` 是 clean clone/relocation 专用类型：必须由任务在预先不存在的专用 temp 根创建，owner manifest/nonce 存在于 clone 外部；只允许专用 workspace finalizer 删除。独立项目源码根及其 `.git` 永远不匹配该入口。

## 9. 持久化路径迁移

先建立版本化“持久化路径字段注册表”，覆盖 draft、正式 save、save index、run context、source artifacts、pipeline/checkpoint、patch、SDK、AI/CLI 设置、日志和 lock。

字段分三类：

1. project-owned：改为相对数据根存储；
2. machine-binding：保留 host-specific 属性，启动时验证，失效后要求重新关联；
3. historical-display：明确不参与寻址，可保留或在迁移副本中规范化。

迁移在正常加载前执行：同盘 staging、before-image、原子提交、失败回滚、版本号幂等。摘要比较排除被批准变更的路径字段；其余文件必须逐字节一致。AI 密钥不打印、不进入报告。

## 10. Governance 重建设计

旧 Python/计划迁移 gate 不再作为独立产品 release gate。治理重构拆成五层：

1. 根发现：当前独立根，不认固定目录名；
2. 输出路由：内部 `gates/`，相对路径证据；
3. 历史证据：必要摘要移到 `docs/migration-history`，只读且非硬门禁；
4. 独立 gate：format、Web、Rust、资源 manifest、standalone boundary、portable、relocation；
5. release manifest：只汇总当前独立产品证据。

过时命令要么改写为独立语义，要么明确返回 deprecated 并从默认 doctor/release 流程移除，不能暗中读取父项目。

active UI baseline 同样解除 Python 语义：`python_source`、旧 `source_plan` 等字段从活动 fixture/index 删除，替换为独立 UI contract ID 和当前验收说明。必要的旧来源只写入 `docs/migration-history`，standalone boundary gate 不为活动 baseline 设置永久豁免。

## 11. Git 切割和版本策略

1. Git 初始化前只编写独立 `.gitignore`/`.gitattributes` 和精确 secret/path fixture allowlist；ATOM-000/001 禁止 add/commit，也禁止让 Git 命令隐式作用于父仓库。
2. ignore 必须列出精确路径/后缀，禁止 `*config*` 一类宽泛规则；构建必需配置必须进入 staged 清单。
3. 在父仓库本地 `.git/info/exclude` 幂等加入 `/NEWrust/`：记录修改前摘要，只追加缺失的精确行；初始化失败仅撤销本次插入，最终移走后提示清除陈旧规则。
4. 独立 Git 初始化并确认 toplevel 后，才运行 `git check-ignore` 验证递归 `user_data`、dist、缓存、密钥、日志与恢复目录；README/fixture 用否定规则保留。
5. `git init -b main` 后断言 toplevel 精确等于独立根；所有命令使用 `git -C <root>`。
6. 缺少 repo-local 用户身份时停止请求配置，不伪造身份。
7. 禁止 `git add .`；按 allowlist 暂存，扫描 staged blobs 后提交。
8. 建立迁移前基线；从 ATOM-002 完成后才启用“验证 → 清理 → index 审计 → checkpoint commit”。
9. 最终从 Git 做无共享对象 clean clone，验证 `git fsck --full`、HEAD 和完整重建。
10. 全历史 secret/绝对路径审计通过、最终清理和交接文档提交后，最后创建幂等标签 `standalone-v1`；不设置远端、不 push。

## 12. 垃圾生命周期

安全清理工具必须在第一次实际清理前完成。G0 只盘点/dry-run。

| 节点 | 时机 | 规则 |
| --- | --- | --- |
| G0 | Git 基线前 | 只读盘点，记录保护数据摘要 |
| G1 | 清理工具完成后 | 清理旧 Cargo/Web/gate 垃圾；前后核对 user_data 清单 |
| G2 | 每个 Rust/Web task 后 | 使用外置唯一 target/temp；finally 清理 |
| G3 | 每个 gate 后 | 删除截图、browser profile、临时报告；只留小型摘要 |
| G4 | portable 后 | 仅在 smoke/摘要通过后清 stage/backup/重复输出 |
| G5 | clean clone/relocation 后 | 删除 clone、target、npm 输出、测试 user_data |
| G6 | commit/tag 前 | secret/path/ignored-file 检查和最终安全清理 |

清理工具默认 dry-run，目标必须 canonicalize 到独立根或专用 temp，拒绝 reparse point、根目录、父目录和 protected user_data 的祖先。普通项目清理保护 `.git`、knowledge、pipeline、testdata、apps、crates、web/src、tools、docs。owned ephemeral 数据仅能凭外部 owner manifest/nonce 通过唯一临时根入口清理；clean clone 的 `.git` 只允许 `owned-ephemeral-workspace` finalizer 删除，且该 finalizer 必须证明目标不是独立项目源码根。portable backup 只归 `Finalize-PortableSwap` 管理。

## 13. 验证矩阵

| 交付 | 路径/环境 | 数据 | 外部工具 | 验收 |
| --- | --- | --- | --- | --- |
| clean Git clone | 简单路径 | 空 | 无 | npm ci/build、Cargo、governance、package |
| clean Git clone | 中文+空格+改名 | 空 | 无 | 同上；无父访问 |
| clean Git clone | 不同盘符/较长路径 | 空 | 无 | 核心 build/test/package |
| clean portable | 两个不同可写路径 | 空 | 无 | smoke、GUI ready、保存、退出、重启 |
| local portable copy | 新路径 | 非空真实数据副本 | 旧绑定失效 | 原子迁移、保存、重启、重新绑定 |
| clean Windows VM/Sandbox/第二台电脑 | 可写本地路径 | 空 | 无 | launcher、WebView ready、保存和重启 |
| 新机器恢复能力 | 可写本地路径 | 测试数据 | 重配 Unity/CLI | 缺失阻断与重配成功两支均通过 |

不承诺：只读目录、`Program Files` 就地 portable 数据、UNC/网络盘、云同步目录或 reparse point。若未来支持，必须新增独立 gate。

## 14. 完成定义

- 所有必需资源在 `git ls-files` 中，clean clone 不补文件即可完整构建。
- 父项目改名、不可访问或不存在时验证仍通过。
- 根目录改名和异路径验证通过，无固定 `NEWrust` 路由。
- clean portable 的 PE、resource manifest、smoke 和 GUI gate 通过。
- 干净 Windows 环境运行 gate 通过；若环境不可用，本项目只能标为“代码完成、外部验收未完成”，不能宣称跨电脑完成。
- 用户数据迁移幂等、可回滚，外部绑定缺失与恢复成功均有验证。
- 独立 Git 工作树干净，历史和 staged blobs 无密钥/旧机绝对路径，最终标签指向最后提交。
- 最终垃圾审计只保留源码、跟踪资源、fixture、文档、一个明确保留的发布物和受保护用户数据。

## 15. 回滚与迁移边界

- 不直接剪切原目录；先在当前目录完成独立化、Git 历史和 clean clone 验证。
- 最终目标绝对路径尚未给出时，不移动/删除原目录，只交付可迁移仓库。
- 迁移成功前保留原目录只读副本；任何摘要不一致立即停止。
- 切割不删除旧项目资源，只保证新项目再也不读取它们。
