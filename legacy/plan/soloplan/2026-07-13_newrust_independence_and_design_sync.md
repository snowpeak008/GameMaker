# NEWrust 独立性与设计同步分析方案

## 背景

本轮问题分成两类，不能混在一起判断：

- 启动恢复上次项目：`NEWrust/dist/AutoDesignMaker-NEWrust/user_data` 中保留了本地试用运行态，启动器固定设置 `ADM_NEWRUST_DATA_DIR=%~dp0user_data`，因此会恢复上次草稿或当前存档。
- Rust 独立化：`NEWrust` 运行时不能从父级 Python 项目读取产品数据、运行态、配置或持久化目录。Python 只能作为一次性迁移来源、行为参考或显式审计输入。

## 当前发现

- `NEWrust` 已有自己的 `.project_root`、`Cargo.toml` 和 `knowledge/resource-manifest.json`。
- 正式发布目录 `dist/AutoDesignMaker-NEWrust-release` 的 `build-manifest.json` 显示 `user_data_mode=clean_release`、`user_data_files=0`、`user_data_bytes=0`。
- 本地试用目录 `dist/AutoDesignMaker-NEWrust` 的 `user_data` 不是空目录，当前包含 “植物大战僵尸” 草稿和 `save_19f4f3983f2_0`。
- 实施前 `standalone_boundary_gate_report` 只检查少量固定源码文件中的少量 parent-path pattern，不能覆盖配置、脚本、前端、Tauri、资源清单和运行态路径。
- 设计细节同步目前主要依赖旧 parity/迁移证据，没有一个显式报告回答“Python 设计数据和 NEWrust 设计数据到底差了什么”。
- 本轮实现后的独立性报告显示 `standalone-boundary-gate` 通过，扫描 213 个文件，禁止父级路径命中数为 0。
- `ADM_NEWRUST_SOURCE_ROOT` 仅在 debug 构建可用，且必须通过 Rust v2 的项目标记、Cargo workspace、两份 lockfile 和资源清单身份校验；父级 Python 根无法通过。release 构建完全不读取该覆盖变量。
- “Python 存档兼容”只发生在用户显式打开已经位于 Rust 自有数据根中的导入归档时，读取归档内部 `design_project` 对象；不会访问 Python 工程目录，因此不构成跨工程运行依赖。
- 第一版设计同步审计显示 `status=attention_required`、`difference_count=144`，但复核确认这 144 项全部只是 CRLF/LF 或文件末尾换行差异，规范化文本内容一致；没有真实设计语义差异，也没有 `missing_in_rust` 或 `rust_only` 文件。该误报已由原子计划 `007` 修正。
- 实施前桌面运行时将 `auto_restore_current_save` 固定为 `true`，并在启动时优先读取 `drafts/<session>/autosave_state.json`，其次读取 `save_index.current_save_id`。因此，即使正式发布包初始 `user_data` 为空，只要同一数据目录运行过一次，后续启动也会恢复上一次项目；该问题已由原子计划 `006` 修正。

## 目标

1. 增强独立性检查：阻断 Rust 源码、脚本、配置和发布清单中的隐式父级 Python 路径依赖。
2. 明确运行态策略：正式发布必须 clean first-run；本地试用 `user_data` 只作为受保护用户数据，不得冒充初始空项目证据。
3. 增加设计同步分析：显式比较 Python 与 NEWrust 的设计数据、schema、pipeline artifact registry 等资源树，输出缺失、差异和 Rust-only 清单。
4. 保持边界：同步分析可以读取 Python 目录，但只能作为显式审计输入；运行时和门禁不得因此依赖 Python。
5. 修复启动语义：桌面产品默认进入新的空白未绑定草稿；历史正式存档和旧草稿不得被删除，恢复旧工作区只能通过显式恢复策略或用户主动打开存档发生。
6. 修复同步审计精度：区分字节一致、仅格式差异和语义内容差异，只有语义差异、缺失文件或 Rust-only 文件计入 `difference_count`。
7. 同步根入口设计：与 Python 项目一样提供根目录 `AutoDesignMaker.exe`，但由原生 Rust 薄启动器保持 Tauri 产品与便携资源根完整，不再使用根目录 CMD。

## 非目标

- 不删除用户已有 `user_data`。
- 不修改 Python 运行时代码。
- 不要求 Rust 兼容 Python save/draft 格式。
- 不把 Python `knowledge/` 作为 NEWrust 的运行时读取路径。

## 实施策略

- 在 `adm-new-governance` 中扩展 `standalone_boundary_gate_report`，扫描更完整的源文件和发布脚本。
- 新增设计同步分析结构和 CLI 命令，默认从当前 `NEWrust` 根向上识别父级 Python 根；报告中标记该读取仅用于审计，`attention_required` 表示资源内容差异需要人工判读，不等同于 Rust 运行时依赖 Python。
- 分析资源范围先覆盖稳定高价值树：`knowledge/design_data`、`knowledge/schemas`、`pipeline/artifact_layer`。
- 独立性门禁输出采用 `GateReport`，设计同步审计输出采用文本报告和 JSON 友好的稳定字段，方便后续接入 CI 或人工审查。
- 桌面运行时引入明确的启动项目策略。产品默认策略为 `blank`，每次启动使用新的独立草稿会话并写入空项目状态；原有恢复算法保留为显式 `restore` 策略，供恢复测试和故障恢复使用。
- 设计同步比较对 UTF-8 文本规范化换行和文件末尾换行；JSON 文件进一步按解析后的结构比较。报告保留原始树摘要，同时新增语义树摘要和仅格式差异计数。
- 必需审计资源组缺失或为空属于 `failed` 并返回非零退出码；只有真实内容差异使用非阻断的 `attention_required`，避免“两边都没有”被误判为同步。

## 实施结果

- 默认启动已改为 `blank`：新建独立、未绑定草稿，项目状态和流水线状态均为空；`ADM_NEWRUST_STARTUP_PROJECT=restore` 才启用旧恢复算法。
- 后端 shell、Tauri 默认配置、前端 fallback 和便携启动器均同步为 `autoRestoreCurrentSave=false` / `blank`。
- 历史 `saves/` 和旧 drafts 未删除；回归测试证明正式存档仍可列出并显式加载。
- 回归测试同时覆盖“空白窗口直接关闭”：未绑定空白草稿只保存自身 autosave，不会把旧正式存档覆盖为空项目。
- 设计同步最终报告为 `status=passed`、`difference_count=0`、`format_difference_count=144`，三组资源均无语义变更、Rust 缺失或 Rust-only 文件。
- 独立性门禁最终报告为 `status=passed`、`forbidden_hit_count=0`；本地试用数据被识别为受保护数据，正式发布 clean-first-run 判定通过。
- 根目录入口已改为原生 `AutoDesignMaker.exe`：它只从自身目录解析 Rust 便携根，校验关键清单、检测 WebView2、固定 Rust 自有数据目录并直接启动 Tauri 产品；根目录 CMD 已删除。
- 便携包内部 CMD 的 WebView2 通配误报已经修复，但它只作为现有发布清单的兼容支持文件保留，根 EXE 不调用也不依赖它。
- 根入口实测为 x64 静态 CRT、398,336 字节；自检、完整 workspace 测试、独立性门禁和真实 GUI 启动均通过。真实启动产生新的 103/103 `not_started` 空白草稿，没有恢复旧项目或流水线。

## 原子计划同步索引

1. `dev/001_independence_boundary_gate.md`：源树扫描、禁止父级路径模式和误报规避。
2. `dev/002_user_data_first_run_policy.md`：正式发布 clean first-run 与本地试用 `user_data` 保护策略。
3. `dev/003_design_sync_inventory.md`：Python/Rust 设计资源树差异模型。
4. `dev/004_cli_reports.md`：`design-sync-audit` CLI、文本报告与 JSON 输出。
5. `dev/005_verification.md`：格式化、测试、桌面启动策略回归和两个实际报告命令。
6. `dev/006_blank_startup_policy.md`：默认空项目启动、显式恢复模式和历史用户数据保护。
7. `dev/007_semantic_design_sync_audit.md`：字节/格式/语义三层差异模型及误报修复。
8. `dev/008_native_root_exe_launcher.md`：Python 同职责的根 EXE、Rust 便携根保护、构建安装和真实启动验证。

## 验收标准

- `adm-new-cli standalone-boundary-gate` 能报告扫描文件数量、禁止路径命中数量、正式发布 `user_data` 状态和本地试用 `user_data` 状态。
- `adm-new-cli design-sync-audit` 能输出 Python/Rust 设计资源树差异摘要。
- 当前三组资源的语义同步结果为 `status=passed`、`difference_count=0`，同时单独报告 144 个 `format_only_files`，不再把换行差异当作内容差异。
- 默认桌面启动得到空项目、空流水线和未绑定草稿；已有正式存档仍能在存档列表中看到并由用户显式打开。
- 显式恢复策略仍能恢复上一次草稿或当前存档，用于故障恢复，不作为产品默认启动行为。
- 新增或调整的测试覆盖独立性扫描、正式发布 clean user_data 判定、设计同步比较。
- 不引入 Python 运行依赖；Rust 代码仍可在 clean clone 的 `NEWrust` 内编译测试。
- 根目录 `AutoDesignMaker.exe` 可直接启动 `dist/AutoDesignMaker-NEWrust/AutoDesignMaker.exe`，不经过 CMD，默认仍为空白项目。
- 本轮生成报告：`NEWrust/gates/standalone-boundary-gate.adm`、`NEWrust/gates/design-sync-audit-gate.adm`。
