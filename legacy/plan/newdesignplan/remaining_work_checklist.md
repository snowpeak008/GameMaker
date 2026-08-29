# 剩余工作清单（源码级 → 正式发布级）

状态：第三次开发已继续执行（W1.2/W1.3/W1.4/W1.5 完成；W4.1 源码级 VLM 门禁接线完成；W4.7 源码级正式发布证据接线完成；W4.8 A05 UI 五态可见性完成；W4.9 Step02 门禁去恒真完成；真实 VLM Provider/Unity/EXE/人工签署发布门仍未关闭）
日期：2026-07-17
来源：Claude 四路并行审查（A02–A07 门禁核对、A08a–f 门禁核对、A09/A10+fixture 核对、bug 猎查）+ `cargo test --workspace --locked --quiet` 全通过确认
结论基线：**源码级开发已完成**；以下是达到"正式发布级 / R1 停止门可关闭"还需要的全部工作。

执行顺序建议：W1 → W2 → W3 → W4 → W5。每项完成后勾选，并在"完成记录"栏填写验证方式。

---

## W1. P0 — v2 链接入产品路径（最大缺口，优先做）

当前 `product_executor.rs:892-903` 与 `development_registry.rs:14-17` 仍注册 legacy `step07.rs` / `step08_14.rs`。整条 step07_v2…step14_v2 只被测试和 A09 harness 调用，用户从 UI 运行的是旧代码。

- [x] **W1.1** 在产品执行器中按 `game_spec_v2` 开关路由：开关开启时 Step07–14 调用 v2 stage 实现，关闭时走 legacy（保持渐进式替换原则，不删旧路径）
  - 验收：开关开启的项目从 UI 运行 Step07，产物出现 v2 输出（美术规范+锚点+硬门禁报告），而不是 legacy 输出
- [x] **W1.2** Step11 v2 接入真实编码代理：`adm-new-application/src/work_unit.rs` 已将 Codex/Claude CLI 开发 executor 桥接为 `WorkspaceTaskAgent`，并通过 Tauri pipeline 命令注入 `ProductPipelineExecutor`；配置不可用时记录 warning 并回落到本地 filesystem fallback。
  - 验收：一个真实任务经真实代理执行 → 变更集 → scope 校验 → 合入，全程审计
  - 第三次开发记录：新增 `workspace_task_agent_from_config(...)`；`AiDevelopmentWorkUnitExecutor` 复用现有隔离 CLI、CAS 提交、Unity batchmode 验证并映射为 `WorkspaceTransactionResult`；新增离线 preflight 回归测试。真实 Codex/Claude + Unity 执行仍需目标环境验收。
- [x] **W1.3** Step11 v2 实现真实工作副本与合入：产品路径已使用 target 快照创建隔离工作副本，操作先落到 isolated，再用备份目录串行替换 target；冲突/越界在合入前拒绝，失败不污染目标工作区。`support.rs` 的 `merge_tree_hash` 仅保留为 Step11 状态哈希推进，不再代表产品路径文件合入。
  - 验收：任务在独立工作副本生成文件，串行合入到目标工作区，越界写入被物理拒绝
  - 第三次开发记录：修复隔离副本不是 target 检出的问题，`RenameFile` 可从 target 快照读取源文件；新增备份回滚式合入；新增冲突不变异目标目录测试；Step10 冻结哈希漂移时 Step11/12 fail-closed 且不覆盖 `stage_10`。
- [x] **W1.4** Step11 每次合入后跑构建 + smoke（总计划 6.2 第 7 条）
  - 验收：合入后编译失败 → 该任务进入修复循环/修正队列，不污染后续任务
  - 第三次开发记录：产品/UI v2 Step11 已禁止无真实 agent 的本地 fallback；真实 Codex/Claude agent 路径将 `WorkspaceChangeSet` 中的 compile/test/smoke `command_permissions` 交给 `UnityBatchmodeProgramVerifier`，合入后执行 Unity batchmode compile 与受信 smoke/test，并将失败映射到 v2 修正队列。本地 filesystem fallback 仅保留为测试/离线合同落盘路径，输出 `not_executed` 证据，不再伪装通过。注意：这只关闭 Step11 合入后的构建与受信 smoke/test，不关闭 W4.7/W5 的完整 EXE playable smoke 与人工试玩签署。
- [x] **W1.5** A08c 验收里"三套旧实现调用方全部切换"未达：`AiDevelopmentWorkUnitExecutor`、`CodexPatchRunner` 仅打了 Legacy 注释，桌面产品路径仍独占旧实现。在 W1.1–W1.3 完成后做实际切换或写明保留策略修订记录
  - 验收：调用方切换完成，或宪章级修订记录说明保留原因与影响面
  - 第三次开发记录：采用保留策略而非删除旧入口。`contracts/W1_5_legacy_execution_boundary.md` 已冻结保留原因、影响面和禁止用途；源码新增 `WORK_UNIT_EXECUTOR_*`、`AI_DEVELOPMENT_EXECUTOR_*`、`CODEX_PATCH_RUNNER_*` 边界常量和测试。结论：v2 产品 Step11 的权威路径是 `WorkspaceTaskAgent + WorkspaceChangeSet`，旧入口只保留给 `game_spec_v2=false` legacy Step08-14、Step07 图片任务、CLI patch 与 R0 harness。

## W2. P1 Bugs（必须修，均已确认可触发）

- [x] **W2.1** 长项目名 panic：`crates/adm-new-design/src/game_spec_projection.rs:559` — `safe_spec_id()` 消毒不截断，超 96 字节时 `SpecId::new` 拒绝 → `.expect()` panic。触发路径：磁盘上的长名 `ProjectState` → `apply_game_spec_v2_sidecar_migration` 直接 abort
  - 修法：`safe_spec_id` 按字符边界截断至 96 字节以内（截断后可追加短哈希防碰撞），或入口 validate 返回 `Err`
  - 验收：>96 字节项目名的迁移返回错误或成功降级，不 panic；加回归测试
- [x] **W2.2** A09 门禁 fail-open：`cross_genre_evaluation/runner.rs:75-90` — `Passed/Blocked` 判断不检查 `third_layer.mutation_rejection_count == mutation_rejections_required`，证据算了、存了、但不参与判定。编译器退化时 A09 照样 Passed 并放行 R2 发布
  - 验收：人为让 trace_links 清空的规格通过编译（或 mock），A09 必须 Blocked；加负面测试
- [x] **W2.3** A09 硬编码证据：`runner.rs:365-367` — `no_ai_mode_supported: true`、`bounded_ai_repeat_stable: true` 无条件写死，未测量
  - 修法：真实跑一次无 AI 路径（ConfirmationMode::Disabled 下流程可人工完成）和有界 AI 重复 N=20 稳定性，用测量值填充
  - 验收：两项证据来自真实执行结果；把执行关掉时对应项变 false 且 A09 Blocked

## W3. P2 Bugs（提交前修完）

- [x] **W3.1** Windows 大小写路径绕过：`adm-new-change-kernel/src/workspace_change_set.rs:18-48` — 路径比较大小写敏感，`SRC/x.rs` 与 `src/x.rs` 被视为不重叠但落盘同一文件，可绕 scope 分离与受信测试保护
  - 修法：`WorkspaceRelativePath::parse` 统一小写归一（或比较时 case-fold），并禁止声明集中出现 case-fold 后重复的路径
  - 验收：大小写变体声明/写入被拒绝；加测试
- [x] **W3.2** 迁移半提交状态：`r2_release.rs:168-203` — backup→game_spec→report→receipt 序列写、无清理；中途失败留下无 receipt 的 game_spec.json，看起来像成功迁移
  - 修法：先写临时目录再原子 rename，或失败时调用已有的 `rollback_game_spec_v2_sidecar_migration` 清理
  - 验收：注入中途写失败，侧车目录要么完整要么为空；加测试
- [x] **W3.3** readiness 报告双写：`r2_release.rs:288-294` — 同一路径写两次（第二次才含 output_paths），期间崩溃/并发读会看到不一致内容
  - 修法：先填 output_paths 再一次性写入
- [x] **W3.4** RequiresAny 环不检测：`adm-new-design/src/decision_graph/mod.rs:589` — 入度累积只算 Requires 边，纯 RequiresAny 环静默按 key 排序而不报 `decision_graph.dependency_cycle`
  - 修法：环检测把 RequiresAny 边计入（或明确文档化 RequiresAny 不构成排序约束并从 build_edges 分离）
- [x] **W3.5** Step07 margin 门禁漏洞：`step07_v2.rs:587（写入 max(8)） vs :522（校验 min(width/4)）` — 大 margin + 小图时校验被 clamp 到比声明更小的值，误放行。当前任务集不触发，属潜在门禁弱化
  - 修法：校验用声明的 margin 原值；margin ≥ 尺寸/2 的任务在编译期拒绝
- [x] **W3.6** A09 测试自相矛盾：`tests/a09_cross_genre_evaluation.rs:7,31` — 函数名说 three full production，断言 `== 4`（3 个 R2 切片 + 1 个 R1 参照）。改名或改断言，让契约一致

## W4. 停止门证据差距（"正式发布级"硬要求）

这些不是 bug，是计划冻结原则中"延后到对应任务启动时"的部分，现在到期：

- [ ] **W4.1** VLM 评审接入与持久缓存：`CachedVlmReviewService` 是确定性 stub、缓存仅内存、且 Step07 主流程与 Step12 都没调用它（A08d 声称复用但 grep 为零引用）
  - 要求：按图像内容哈希持久化缓存（落盘），Step07/Step12 实际调用，评审证据进审计
  - 第二次开发记录：已完成持久缓存文件、按 `config_id + image_hash` 复用、跨实例回归测试；Step07/Step12 主流程真实 VLM 调用与审计落盘仍未关闭。
  - 第三次开发记录：已完成源码级门禁接线：新增 `VlmReviewService` / `VlmImageReviewer` 合同、按 `config_id + image_hash` 的线程安全持久缓存、Step07 `vlm_style_review_report.json`、Step12 `vlm_asset_review_report.json`、产品路径实际调用 VLM 服务并在未配置时 fail-closed、Step07 确认前强制检查 VLM 报告。当前未伪装真实 Provider：应用层尚无可用 vision/VLM 适配器注入，默认 `vlm_unconfigured` 只写 unavailable 审计并阻断；发布级验收仍需接入真实 VLM Provider 后重跑 R1。
  - 第四次开发记录：已完成应用层真实 Provider 源码注入：`adm-new-application::vlm_review_service_from_config(...)` 从 active completion OpenAI-compatible API 配置创建 VLM reviewer，构造包含 PNG data URL 的 chat-completions 请求，要求 JSON response，并将解析失败/拒绝结果 fail-closed 映射为 VLM evidence；Tauri pipeline 在 Step07/12 范围内注入该服务，CLI completion 配置不会被误当成视觉评审。仍未关闭发布级外部验收：尚未使用真实密钥/真实视觉模型对 R1 目标环境跑通。
- [x] **W4.2** 资产硬门禁补全：OCR 水印检测（当前只有右下角暗像素启发式）、切片几何校验（完全缺失）、文件完整性专项检查
  - 第五次开发记录：已完成源码级确定性硬门禁补强：decode 前检查 metadata/空文件/PNG signature，decode 后检查零尺寸与文件大小合理性；水印检测从右下角暗像素扩展为边缘 OCR-like 文本笔画检测；按 `sprite/single_sprite`、`nine_slice_ui`、`full_frame/keyframe` 做切片几何校验；Step07/Step12 共享 `validate_anchor_images`，新增回归覆盖文件完整性、非右下角文本水印、切片几何失败。未引入第三方 OCR 引擎，当前为可审计的确定性文本状水印门禁。
- [x] **W4.3** Step12 真实引擎加载：`DeterministicHeadlessAssetLoader` 是纯记账（referenced+gated=loaded），需要 headless/smoke 实例化加载；另外 `default_bindings`（production.rs:283-315）给每个 manifest 项自动造 binding，孤儿检测在真实路径上永远不会触发——绑定必须来自真实场景/预制体引用
  - 第六次开发记录：已完成源码级假证据移除：删除 Step12 内部 `default_bindings(manifest)` 隐式合成路径，`run_step12_asset_production*` 改为必须由调用方传入 runtime binding；新增 `discover_asset_bindings_from_workspace(...)`，只从真实 workspace 中的 `.unity/.prefab/.asset` 引用文件扫描 asset id / imported path；新增 `WorkspaceReferenceAssetLoader`，要求生成资产文件存在、硬门禁通过、引用文件可读且实际包含该资产引用才可 `loaded/instantiated=true`；旧 `DeterministicHeadlessAssetLoader` 改为 fail-closed，不再把 referenced+gated 伪装成已加载；产品路径从 `workspace/game_spec_v2/target` 扫描绑定，扫描不到引用则 Step12 进入 correction queue。注意：本轮未伪装 Unity batchmode/headless smoke 已执行；真实 Unity 实例化与 PlayMode/场景执行仍在 W4.5/W4.7/W5 外部验收门继续关闭。
- [x] **W4.4** Step12 sample(n) 落地：`sample_count` 目前只格式化进字符串，无实际抽样与人工确认流；A05 的 `ConfirmationMode::Sample` 同样落在 `_` catch-all（policy.rs:91）无行为。两处一起实现
  - 第二次开发记录：Step12 已按 manifest 前 N 个资产抽样，非样本资产 `not_required` 且测试覆盖；A05 `ConfirmationMode::Sample` UI/策略联动仍未关闭。
  - 第七次开发记录：已完成 A05 `ConfirmationMode::Sample` 显式策略分支，审计记录新增 `sampleSize`，`sample_size=0` fail-closed 并要求人工确认；Step12 sample 抽样改为按 asset `slice` 家族优先覆盖再按清单补齐，主输出与 `asset_confirmation_report.json` 新增结构化 `confirmationRecords`（sampled/sampleIndex/sampleCount/requiresHuman/status）；产品路径已覆盖 sample 模式的等待确认、读取样本、批准样本、成功运行回路，同时保留显式 `auto_accept` 回归。
- [x] **W4.5** Step13 场景真实执行：当前是决策表（由 Step11/12 状态+policy 标志推导），且 `nominal_for_spec`（types.rs:98-103）默认把性能观测=预算上限、可访问性=true、人工评审=全部预填——默认路径永远不会因这三类失败。需要真实场景执行器/真实观测数据源，并把 nominal policy 限制为测试专用
  - 第二次开发记录：产品路径使用 `strict_unattended()`，人工/性能不再默认预填；真实场景执行器与真实观测数据源仍未关闭。
  - 第八次开发记录：已移除 Step13 隐式 `nominal_for_spec` 通过路径，新增 `Step13ExecutionEvidence` / `ScenarioExecutionObservation` 显式证据合同；Step13 现在总是写出 `scenario_execution_request.json`，产品路径只读取 `stage_13/scenario_execution_evidence.json`，缺失证据、build hash 不匹配、自动场景缺 observation、性能/可访问性缺观测均 fail-closed；A09 与测试改为显式 `test_only_nominal_for_spec` 夹具证据，不再混入产品路径。注意：本轮完成的是“无真实证据不通过”的源码门禁和证据输入合同，真实 Unity/PlayMode runner 的外部进程执行与目标环境验收仍归 W4.7/W5 发布门继续关闭。
- [x] **W4.6** Step13/14 去 fixture 化启发式：人工场景识别靠 summary 含 "human reviewer" 子串、性能场景靠 "maximum planned load" 子串、Step12 绑定路径靠 asset id `contains("hud")` ——改为规格字段驱动（scenario 加显式 manualReview/performanceBudget 标记）
  - 第二次开发记录：`AcceptanceScenario` 已新增 `manualReviewRequired` 与 `performanceBudgetRefs`，Step13 已改为字段驱动；Step12 `default_bindings` 的 asset id 启发式已由 W4.3 第六次开发记录移除。
  - 第九次开发记录：已补齐剩余资产场景启发式，`AcceptanceScenario` 新增 `assetValidationRequired` 字段，R1-C0 资产绑定场景显式标记；Step13 删除 `scenario_touches_assets()`，missing asset 只阻断 `assetValidationRequired=true` 的场景，不再读取 scenario id、summary、action/target 字符串里的 `asset`；`scenario_execution_request.json` 同步输出 `assetValidationRequired`。新增回归测试证明即使场景名含 `asset`，未显式标记也不会被 missing asset 误伤。
- [x] **W4.7** Step14 R1GateEvidence 接真实工具链：九项证据当前全由调用方传 bool。需要接 `tools/verify-standalone.ps1` 输出、可重现构建比对、真实 EXE smoke、AI 使用证据汇总；A09 harness 里 `user_playtest_signed: true` 硬编码必须移除（人工签署永远不可合成）
  - 第二次开发记录：产品 Step14 已要求显式 `r1_gate_evidence.json`，缺失即阻断；A09 已移除 `user_playtest_signed: true` 合成签署，改为 `manualSignatureRequired` 证据。真实工具链接入仍未关闭。
  - 第十次开发记录：已完成源码级真实工具链证据接线：Step14 新增 `derive_r1_gate_evidence_from_sources(...)`，从项目根 `gates/standalone-release-evidence.json` 读取 `tools/verify-standalone.ps1/v2` 正式发布证据，校验 schema/producer/project/status/sourceTreeClean/freshness/21 个必需 check/portable receipt；可重现构建、完整性、standalone boundary、EXE smoke 均由该工具证据派生，不再由产品路径手填 bool。流水线侧新增 `r1_pipeline_gate_evidence.json` 承载内容完成、AI 使用审计、AI-off flow、反过拟合证据引用；人工试玩拆为 `r1_user_playtest_signature.json`，必须是 `manual_user_playtest`、签署人/时间/acknowledgement 非空，并绑定同一个 standalone evidence id。产品 Step14 运行时自动落盘派生后的 `r1_gate_evidence.json` 与 `r1_gate_evidence_source_report.json`，缺失/失败项 fail-closed。注意：本轮只关闭源码接线；真正执行 `tools/verify-standalone.ps1`、重建正式包、真实 EXE smoke 和用户试玩签署仍是 W5 发布验收工作。
- [x] **W4.8** A05 UI 状态可见性：`BoundedCompletionService` / `CompletionRunStatus` 在 apps/、web/、adm-new-tauri-commands 零引用。A05 停止门要求五种状态（not_called/failed/rejected/confirmed/committed）在 UI 可区分
  - 第十一次开发记录：已完成源码级 UI 可见性接线：Tauri pipeline 阶段视图新增白名单 `bounded_completion` 视图，从阶段 outputs 中解析 `not_called/failed/rejected/confirmed/committed` 五态、模型配置、风险、尝试次数、人工确认摘要和错误摘要；无记录时显式 `not_called`，未知状态 fail-closed 为 `failed`，并复用可见文本脱敏。前端 pipeline 详情面板新增 `AI 补全状态` 区块，五态本地化可区分，仍丢弃 raw `outputs/artifacts`。本轮未改变 A05 实际补全执行策略，只关闭 UI 状态不可见缺口。
- [x] **W4.9** Step02 门禁去恒真：`gate_step02_capabilities`（game_spec_v2_steps.rs:324-353）检查的 reason 是内部 `format!("{:?}/{:?}")` 生成、永不为空，门禁形同虚设。应消费 A03 决策图的真实谓词证据（JSON Pointer + 期望/实际值）
  - 第十二次开发记录：已移除 Step02 内部 `capability_reasons` 字符串门禁，改为调用 A03 `CapabilityDecisionGraphCompiler` 编译真实 activation evidence；Step02 输出 `decisionGraphStatus`、`activeNodeCount`、`activationEvidenceCount`、`coveredCapabilityPaths`、`capabilityEvidence` 与完整 `decisionGraph`，每条 evidence 包含 node/domain、predicateId、capability JSON Pointer、operator、expected、actual。A03 domain 缺失、图为空或 activation evidence 不完整均 fail-closed；产品路径投影 GameSpec v2 时使用同一产品资源根 `knowledge/design_data`，不再只依赖源码默认路径。
- [x] **W4.10** 反过拟合工具加宽（低优先）：`permute_display_labels` 只置换 intent.title 和 audiences，不覆盖 entities/regions 的 tags 字段；步骤链能力扰动只测 SpaceTopology 一轴（A03 工具支持 16 轴）。A09 品牌扫描的文件清单与 token 清单是硬编码，新核心文件不会自动纳入
  - 第十三次开发记录：已完成源码级加宽：`permute_display_labels` 现在覆盖 intent.title/audiences、entity tags、region tags，并同步重写 `HasTag` 条件引用以保持语义稳定；新增 `capability_mutation_suite`，按 A03 的 16 个能力轴生成确定性替代扰动；Step00-06 anti-overfit evidence 记录 16 轴 hash change 结果，Step08-10 记录 16 轴 architecture reaction 的 changed/unchanged/failed 轴清单，不再只测 `SpaceTopology`。A09 source scan 改为从 A09 样例动态派生禁词，并从 `forbidden_source_tokens.json` 读取外部品牌别名；源码扫描从硬编码文件数组改为递归收集 GameSpec 核心、A03 decision graph、GameSpec projection、GameSpec v2 pipeline、v2 stages 与 A09 runner/types，排除样例定义文件，新增测试证明新增 v2 核心文件会自动纳入扫描。

## W5. 发布流程收尾（代码之外）

- [x] **W5.1** 提交：在 1.2 分支提交全部工作树变更（当前 A02–A10 全部未提交，停止门证据没有落在版本历史）
  - 第十四次开发记录：用户确认研发阶段分支改用 `0.1.0`，不是 `1.2`。已在 NEWrust 仓库切换到本地 `0.1.0` 跟踪 `origin/0.1.0`，提交 `df89b91d5442e1eb5beb0adb373767dd28e43849`（`feat: add game spec v2 production pipeline`），并推送到 `https://github.com/snowpeak008/GameMaker/tree/0.1.0`。提交前验证：`cargo fmt --all -- --check`、`npm.cmd test`、`git diff --check`（仅既有 CRLF 提示）、`cargo check --workspace --locked`、`cargo test --workspace --locked`、`npm.cmd run build` 均通过；暂存清单未包含 `dist/`、`target/`、`user_data/`、`.env` 或本地 AI 配置。
- [x] **W5.2** 干净树验证：提交后运行 `tools/verify-standalone.ps1`（需要 clean committed tree）
  - 第十五次开发记录：已在 NEWrust 仓库 `0.1.0` 分支的干净提交 `0aeabc3d8909e73abf0833323116df081b990e91` 上运行 `tools/verify-standalone.ps1`，正式 release evidence `03761d84200c44819fe7d96842093ad8` 状态为 `passed`。21 项必需检查全部通过：clean clone relocation、Web unit/i18n/design-content/build/e2e/language/ui/ui-baseline、`cargo fmt --check`、`cargo check --workspace --locked`、`cargo test --workspace --locked`、package contract self-test、standalone boundary、anti-fake scan、portable build、portable smoke、portable integrity、PE x64/static CRT、generated cleanup。随后运行 `cargo run --locked -p adm-new-cli -- release-gate`，汇总状态同样为 `passed`，确认 evidence 绑定当前 HEAD 且 source tree clean。
- [x] **W5.3** 重建本地验收包：现有 dist/ 里的 EXE 不是最新代码产物，重新构建后做 EXE smoke
  - 第十五次开发记录：`verify-standalone.ps1` 已重建 `dist/AutoDesignMaker-NEWrust-release`，`portable_build`、`portable_smoke`、`portable_integrity`、`pe_architecture_crt` 全部 passed；EXE SHA-256 为 `e92f09803292242b12fc171089219c8f2f8e08f9fa56aef7623c9b0cdeed48e0`，transaction `6b7c273fb3d7433b99f71f47fc82aaf4` 已 finalized，release 包 `user_data_files=0`。
- [ ] **W5.4** R1 停止门验收：v2 链从 UI 端到端跑通 R1-C0 fixture 项目，12 个 AcceptanceScenario 全部真实通过，用户签署 playtest（A08f 停止门，人工项不可合成）
- [x] **W5.5** 视需要 push / tag（用户决策）
  - 第十五次开发记录：按用户要求只推送新版 Rust 项目到 `https://github.com/snowpeak008/GameMaker/tree/0.1.0`，未推送父 Python 项目，未创建 tag。远端 `origin/0.1.0` 已更新到 `0aeabc3d8909e73abf0833323116df081b990e91`。

---

## W6. P1 — 并行能力迁移缺口（测试暴露的性能/能力回归）

这些是用户在真实测试中暴露的 Rust v2 相比 Python 版的并行能力缺口。它们会显著拉长流水线耗时，并影响 Step11 修复循环吞吐，但不应和 Step07 checkpoint 竞态修复混为一次提交。

- [ ] **W6.1** Step07 生图并行：Python 版 `generation.py` 使用 `ThreadPoolExecutor` 按图片数量动态并行生成风格图；Rust v2 当前逐张串行生成，实测单张 90s+ 时会线性放大总耗时。
  - 要求：Step07 v2 图片任务按 provider 能力和配置预算并行执行；保留确定性输出命名、每张图独立错误记录、失败任务可重试；并行度需要受 AI 配置或 ExecutionBudget 限制，不能无限开 worker。
  - 验收：至少 3 张图片任务在 mock 慢 provider 下并发执行，总耗时低于串行阈值；单张失败不会丢失其他成功图片的证据；真实 provider 限流错误进入可诊断失败。
- [ ] **W6.2** Step08/10 显式并行计划产物：Python 版拓扑输出要求 `parallel_groups`，Rust legacy 保留元数据但执行侧未消费，Rust v2 仅有 dependencies，缺少显式的并行分层计划。
  - 要求：Step08-10 v2 从任务 DAG 推导稳定的 `parallel_groups` / `execution_layers`，写入 stage_10 产物并进入冻结哈希；legacy 的 `write_task_parallelized=false` 保留策略需写明或关闭。
  - 验收：同一输入重复编译得到相同 parallel group；依赖环/跨组依赖错误 fail-closed；Step11 读取的是冻结后的并行计划而不是运行时临时推导。
- [ ] **W6.3** Step11 真并行执行：v2 当前 `max_workers` 只影响每轮取出的 ready task 数量，取出后仍单线程逐个执行，`max_workers=4` 不产生真实并发。
  - 要求：按 Step08/10 的冻结并行层执行 ready tasks；每个任务仍在隔离工作副本中运行；合入保持单写者串行；失败分类、修复循环、停车场和二分定位保留；默认 `max_workers=1` 可维持保守模式。
  - 验收：mock 慢 agent 下同层多个任务真实重叠执行；合入顺序稳定；一个任务失败不污染其他隔离结果；scope violation 不重试且不会提交；`max_workers=1` 与旧串行结果一致。

---

## 已确认无需处理（审查通过项，避免重复排查)

- 规范哈希顺序不变性、fail-closed 无效样例、包络进哈希（A02 全套真实）
- SpecStore 单写者事务：无 TOCTOU、并发冲突真实双线程测试、审计不可变（A04）
- 重试逻辑：max_retries+1 无 off-by-one；scope_violation 永不重试；修正队列非空则 Step11 不判成功
- 路径校验：`../` 穿越、`\\?\` 前缀、前缀碰撞（src/foo vs src/foobar）均正确处理（大小写除外，见 W3.1）
- R1-C0 fixture：与签署宪章 A+M+P1+V2 一致、12 场景引用完整性无问题、Step06 包络强制有负面测试、8 处测试接线
- A10 侧车迁移：默认 off、只读候选、失败零副作用、回滚幂等，全部有测试

## 完成记录

| 编号 | 完成日期 | 验证方式 |
|---|---|---|
| W1.1 | 2026-07-16 | `cargo test -p adm-new-pipeline --test product_executor --locked`；覆盖 v2 Step07、Step08-10、Step11 产品路径；W1.3/W1.4 复查后回退为未完成 |
| W2.1/W3.2/W3.3 | 2026-07-16 | `cargo test -p adm-new-pipeline --test a10_migration_release --locked`；覆盖长项目名、侧车临时发布、readiness 单次写入 |
| W2.2/W2.3/W3.6/W4.7 部分 | 2026-07-16 | `cargo test -p adm-new-pipeline --test a09_cross_genre_evaluation --locked`；覆盖 fail-closed、无 AI/有界重复测量、人工签署不可合成 |
| W3.1 | 2026-07-16 | `cargo test -p adm-new-change-kernel --test workspace_contract --locked`；覆盖 Windows 大小写路径归一 |
| W3.4 | 2026-07-16 | `cargo test -p adm-new-design --test a03_decision_graph --locked`；覆盖 RequiresAny 环检测 |
| W3.5/W4.1 部分 | 2026-07-16 | `cargo test -p adm-new-pipeline step07_v2 --locked`；覆盖 margin 门禁与 VLM 持久缓存 |
| W4.4/W4.5/W4.6 部分 | 2026-07-16 | `cargo test -p adm-new-pipeline --test step12_v2 --locked`、`cargo test -p adm-new-pipeline --test step13_v2 --locked` |
| 第二次开发总验收 | 2026-07-16 | `cargo check --workspace --locked`、`cargo test -p adm-new-pipeline --locked` |
| W1.3/B6/W2.1 覆盖补强 | 2026-07-17 | `cargo test -p adm-new-pipeline filesystem_workspace_agent --locked`、`cargo test -p adm-new-pipeline --test product_executor --locked`、`cargo test -p adm-new-pipeline --test a10_migration_release a10_migration_handles_overlong --locked`、`cargo test -p adm-new-pipeline --locked`；覆盖隔离 target 快照、串行合入回滚、Step10 冻结哈希漂移拒绝、中文多字节长名迁移 |
| W1.2 源码接线 | 2026-07-17 | `cargo test -p adm-new-application development_executor_maps_workspace_task_preflight_failure_to_v2_transaction --locked`、`cargo check -p desktop-tauri --locked`；覆盖真实 CLI executor 到 v2 `WorkspaceTaskAgent` 的事务映射和 UI 注入接线 |
| W1.4 源码接线 | 2026-07-17 | `cargo test -p adm-new-application workspace_task_bridge_declares_compile_and_trusted_smoke_unity_plans --locked`、`cargo test -p adm-new-application legacy_development_work_units_keep_implicit_compile_only_unity_plan --locked`、`cargo test -p adm-new-pipeline --test product_executor game_spec_v2_product_step11_requires_real_agent_when_product_mode_demands_it --locked`、`cargo test -p adm-new-pipeline filesystem_workspace_agent_renames_from_target_snapshot --locked`、`cargo check -p desktop-tauri --locked`、`cargo test --workspace --locked`；覆盖 v2 agent 桥接 compile+trusted smoke/test、legacy compile-only 兼容、产品路径禁止 fallback、fallback 证据 `not_executed` |
| W1.5 保留策略固化 | 2026-07-17 | `cargo test -p adm-new-pipeline work_unit_executor_retention_boundary_excludes_v2_product_step11 --locked`、`cargo test -p adm-new-application development_executor_boundary_allows_v2_bridge_but_not_direct_v2_commit --locked`、`cargo test -p adm-new-patch codex_patch_runner_retention_boundary_excludes_gamespec_v2_products --locked`、`cargo test -p adm-new-pipeline --locked`、`cargo test -p adm-new-application --locked`、`cargo test -p adm-new-patch --locked`、`cargo check --workspace --locked`；覆盖 legacy `WorkUnitExecutor`、`AiDevelopmentWorkUnitExecutor`、`CodexPatchRunner` 的保留/禁止用途边界 |
| W4.1 源码级 VLM 门禁接线 | 2026-07-17 | `cargo test -p adm-new-pipeline step07_v2 --locked`、`cargo test -p adm-new-pipeline --test step12_v2 --locked`、`cargo test -p adm-new-pipeline --test product_executor --locked`、`cargo check --workspace --locked`；覆盖 Step07/Step12 VLM 服务调用、持久缓存复用、未配置服务 fail-closed、确认前阻断、Step12 VLM 失败进入修正队列。真实 VLM Provider 注入与目标环境验收仍未关闭。 |
| W4.1 应用层 VLM Provider 源码注入 | 2026-07-17 | `cargo test -p adm-new-application vlm_review --locked`、`cargo check -p desktop-tauri --locked`；覆盖 OpenAI-compatible completion API 配置创建 VLM 服务、PNG data URL payload、严格 JSON 响应解析、CLI completion 拒绝为视觉评审，以及桌面 pipeline Step07/12 注入接线。真实模型网络调用与 R1 目标环境验收仍未关闭。 |
| W4.4 sample(n) 源码闭环 | 2026-07-17 | `cargo test -p adm-new-ai bounded_completion --locked`、`cargo test -p adm-new-pipeline --test step12_v2 --locked`、`cargo test -p adm-new-pipeline --test product_executor game_spec_v2_product_step12_waits_for_explicit_asset_confirmation --locked`、`cargo check --workspace --locked`、`cargo test --workspace --locked`；覆盖 A05 sample 策略审计、`sample_size=0` fail-closed、Step12 按 asset slice 抽样、结构化确认报告、产品路径等待/批准 sample 回路。 |
| W4.5 Step13 执行证据门 | 2026-07-17 | `cargo test -p adm-new-pipeline --test step13_v2 --locked`、`cargo test -p adm-new-pipeline --test product_executor --locked`、`cargo test -p adm-new-pipeline --test a09_cross_genre_evaluation --locked`、`cargo test -p adm-new-pipeline --test step14_v2 --locked`；覆盖 Step13 缺执行证据 fail-closed、runner request 生成、显式 execution evidence 通过、产品路径证据文件读取、A09 test-only evidence 分离。 |
| W4.6 Step13/14 去启发式 | 2026-07-17 | `cargo test -p adm-new-game-spec --test fixtures --locked`、`cargo test -p adm-new-pipeline --test step13_v2 --locked`、`cargo test -p adm-new-pipeline --test product_executor --locked`、`cargo test -p adm-new-pipeline --test a09_cross_genre_evaluation --locked`、`cargo test -p adm-new-design --locked`；覆盖 `assetValidationRequired` 解析/序列化、Step13 missing asset 字段驱动阻断、产品路径兼容、A09 与设计投影兼容。 |
| W4.7 Step14 正式发布证据接线 | 2026-07-17 | `cargo test -p adm-new-pipeline --test step14_v2 --locked`、`cargo test -p adm-new-pipeline game_spec_v2_product_step14_derives_gate_evidence_from_release_sources --locked`、`cargo test -p adm-new-pipeline --test product_executor --locked`、`cargo test -p adm-new-pipeline --test a09_cross_genre_evaluation --locked`、`cargo check -p adm-new-pipeline --locked`、`cargo run --locked -p adm-new-cli -- --project-root . standalone-boundary-gate`；覆盖 `verify-standalone` 证据结构解析、21 个必需 check、portable smoke 失败映射、人工签署缺失阻断、产品 Step14 自动派生并落盘 `r1_gate_evidence.json` / `r1_gate_evidence_source_report.json`，并确认新增 release evidence 路径未破坏独立化 boundary scan。 |
| W4.8 A05 UI 五态可见性 | 2026-07-17 | `cargo test -p adm-new-tauri-commands pipeline_view_exposes_bounded_completion_five_states_without_raw_outputs --locked`、`cargo test -p adm-new-tauri-commands --locked`、`cargo fmt --check`、`npm.cmd test`、`npm.cmd run build`；覆盖 Tauri 阶段视图五态解析、无记录 `not_called`、未知状态 fail-closed、脱敏、raw `outputs/artifacts` 不序列化，以及前端详情面板/本地化状态显示。 |
| W4.9 Step02 门禁去恒真 | 2026-07-17 | `cargo test -p adm-new-pipeline game_spec_v2_steps --locked`、`cargo test -p adm-new-pipeline --test product_executor --locked`、`cargo test -p adm-new-pipeline --test a09_cross_genre_evaluation --locked`、`cargo test -p adm-new-design --test a03_decision_graph --locked`、`cargo test -p adm-new-pipeline --locked`、`cargo check --workspace --locked`；覆盖 Step02 消费 A03 activation evidence、移除 `capabilityReasons`、domain 缺失 fail-closed、产品路径资源根接线、A09 与 A03 决策图回归。 |
