# Rust UI 与数据交互全量重建计划

日期：2026-07-06  
范围：`RUST/` 桌面端 Slint UI、Rust 数据模型、服务回调、旧版工作台交互契约迁移  
目标：重建一套 Rust 原生的六任务区界面与数据交互，复刻旧 Python 项目的信息架构、交互逻辑和功能路径，但不复用 Python UI/运行时代码。

## 1. 问题复盘

当前 Rust 桌面端的主要问题不是单纯的视觉样式，而是数据和功能没有接入到旧版工作台的真实交互闭环：

- `RUST/apps/adm-desktop/ui/main.slint` 目前只是六任务区外壳和若干概览列表，无法承载旧版设计工作台的节点编辑、L4 选项选择、L5 实体编辑、AI 访谈、模板、导出、存档等完整交互。
- `RUST/crates/adm-design` 目前只有简化的 `GameDesignBrief` / `DesignProject`，以及一个只读 `WorkbenchReference`。它能读取旧知识库概要，但没有旧版 `DesignEngine` 等价层，也没有 `WorkbenchState` 的可变状态、校验、派生摘要和导出逻辑。
- `RUST/crates/adm-ui-model` 仍是通用 shell 状态，视图枚举也不是用户要求的六个任务区完整模型。
- `RUST/apps/adm-desktop/src/main.rs` 中已有大量归档、AI、打包、SDK、构建相关回调，但设计工作台和 Step00-14 流水线仍未按旧版信息架构重建。
- 当前显示错误的根源之一是页面布局先行、数据结构滞后，导致 UI 只能拼接文本和概览行，不能稳定表达真实业务状态。

本计划的核心调整：先建立 Rust 侧数据契约和服务命令，再绑定 Slint UI；UI 不再作为静态展示层，而是作为 `WorkbenchState`、`PipelineState`、`ArchiveState`、`AiConfigState` 等状态的投影。

## 2. 不变约束

- 继续使用 Slint；不切换到其他 GUI 框架。
- 不实现旧版可拖拽 pane/splitter 边界；Rust 版使用固定响应式分区。
- Rust 必须能独立运行，不依赖旧 Python 运行时代码；Python 代码只作为参考契约。
- 保留当前 Rust 已完成的归档、AI provider、打包、SDK、发布诊断等能力，能复用的服务层继续复用。
- 不做真实 Unity PlayMode 自动验收；Unity 真实验收由用户手动检测。
- 不修改外部 Unity 项目文件；所有测试和产物限制在项目工作区或 Rust 测试临时目录。
- 目标 UI 必须是纯中文界面。
- 双击入口最终仍以 `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe` 为交付入口，但发布包必须通过正式 staging 流程生成，不再只靠手工覆盖 exe。

## 3. 旧版项目契约梳理

### 3.1 顶层任务区

旧版 `core/ui/main_window.py` 的顶层任务区是：

- 设计工作台：`CommercialDesignApp`
- 开发流水线：`PipelinePanel`
- 补充开发：`PatchPanel`
- 打包阶段：`PackagePanel`
- 运行日志：`LogPanel`
- SDK 知识库：`SdkPanel`

Rust 新版必须以这六个按钮作为唯一顶层导航，并且每个按钮对应独立页面状态，而不是把功能混在一个长页面里。

### 3.2 设计工作台

旧版 `core/ui/app_window.py` 的真实契约：

- 左侧：
  - `领域总览`
  - `项目画像`
  - 各领域覆盖率、节点进度、L4 完整度、聚焦领域状态
- 中间：
  - 当前领域标题和描述
  - 搜索框
  - 过滤：`全部`、`已决策`、`未完成`、`有风险`、`不适用`、`L4 未完整`
  - 节点卡片
  - checklist 勾选
  - L4 option group 单选/多选、主选项、软冲突提示、设计问题
  - L5 设计实体 JSON 编辑、保存、清空、校验错误
  - 设计描述、风险说明、不适用原因
  - 玩法系统选择、自定义系统、权重、核心循环、兜底访谈
  - AI 访谈入口和嵌入面板
- 右侧：
  - `摘要`
  - `缺失项`
  - `风险`
  - `校验`
  - 这些内容都由 `DesignEngine` 根据当前 `project_state` 派生，不是静态文本。
- 顶部功能：
  - 项目名称
  - 导出格式
  - 导出
  - 存档管理
  - 模板查看
  - 另存为模板
  - 重置

### 3.3 开发流水线

旧版 `core/ui/pipeline_panel.py` 的真实契约：

- 左侧阶段树：
  - 分组：`设计阶段`、`风格确认`、`计划阶段`、`执行阶段`
  - 阶段：Step00 到 Step14
  - 每个阶段有状态、颜色、标题、点击选择
- 右侧顶部：
  - `项目配置`
  - `AI 配置`
  - `导出到流水线`
  - 从步骤 N 到 M
  - 跳过人工确认
  - 运行
  - 停止
- 右侧内容：
  - 当前阶段详情
  - 当前引擎配置和 AI adapter
  - semantic quality 面板
  - Step07 美术风格图片、选择、确认、重生成、编辑提示词
  - 阶段运行日志
- 状态来源：
  - `core.runtime.pipeline_state`
  - `core.runtime.control`
  - `core.main.run_range`
  - `core.registry.STEP_SPECS`

### 3.4 补充开发、打包、日志、SDK

- `PatchPanel`：接收用户补充需求，调用 patch analyzer，生成补充任务表。
- `PackagePanel`：只有 Step14 成功后才能打包，调用 package service，显示 JSON 结果。
- `LogPanel`：读取严格日志系统，按 level 过滤、刷新、清空、导出 JSONL。
- `SdkPanel`：SDK 名称/URL 输入，添加、待审核、批准、拒绝，同步 approved prompt context。

## 4. Rust 目标架构

### 4.1 `adm-design`

新增 Rust 原生设计工作台核心，替代当前只读 reference：

- `DesignDataRepository`
  - 读取 `knowledge/design_data/domain_order.json`
  - 读取 domains、profile schema、gameplay system options、project templates
  - 进行结构化归一化和校验
- `WorkbenchState`
  - `project_name`
  - `profile`
  - `nodes`
  - `gameplay_systems`
  - `ai_interview`
  - `version`
  - `dirty`
- `NodeState`
  - `decision_state`
  - `design_note`
  - `risk_note`
  - `not_applicable_reason`
  - `checklist`
  - `checklist_options`
  - `option_provenance`
  - `design_entities`
  - `entity_validation_errors`
- `OptionGroupState`
  - `selected`
  - `primary`
  - `source`
  - `confidence`
- `GameplaySystemsState`
  - preset selected ids
  - custom systems
  - weights
  - core loops
  - interview answers and parsed ids
- `AiInterviewState`
  - messages
  - candidate node ids
  - route overview
  - prompt meter
  - replay/runtime records
- `DesignEngine`
  - normalize state
  - set checklist item
  - set option group option
  - set option group primary
  - set node state
  - normalize and validate L5 entities
  - domain/project coverage
  - L4 progress
  - missing items
  - risk items
  - soft option conflicts
  - cross-layer violations
  - quality metrics
  - design completion summary
- `WorkbenchExporter`
  - decision export
  - archive export
  - markdown/json/txt/text/prompt
  - gameplay global appendix
- `TemplateRepository`
  - list builtin/custom templates
  - import template into state
  - save custom template
  - delete custom template

### 4.2 `adm-ui-model`

把 UI model 改成六任务区的稳定投影模型：

- `TaskSpace`
  - `DesignWorkbench`
  - `DevelopmentPipeline`
  - `SupplementalDevelopment`
  - `PackagingStage`
  - `RunLog`
  - `SdkKnowledgeBase`
- `DesignWorkbenchView`
  - domain rows
  - profile fields
  - node cards
  - checklist rows
  - option group rows
  - L5 editor text
  - right tab text
  - AI interview panel state
  - template/export dialog state
- `PipelineView`
  - Step00-14 rows
  - group headers
  - selected step detail
  - run range
  - run/stop state
  - semantic quality summary
  - Step07 style options
  - log tail
- `PatchView`
- `PackageView`
- `RunLogView`
- `SdkKnowledgeView`
- `StatusBarView`

UI model 必须只做可显示数据，不放业务逻辑；业务逻辑在 `adm-design`、`adm-application`、`adm-pipeline` 等 crate 中。

### 4.3 `adm-application`

新增面向桌面端的命令服务层：

- `WorkbenchService`
  - load workbench
  - mutate profile
  - select domain
  - search/filter nodes
  - toggle checklist
  - mutate L4 options
  - mutate L5 JSON
  - update notes/risk/not applicable
  - compute right tabs
  - autosave
  - export
  - template operations
  - save manager operations
- `PipelineService`
  - load Step00-14 state
  - export design handoff to pipeline
  - run range
  - stop run
  - select step
  - load semantic quality
  - load/confirm Step07 style
  - tail logs
- `PatchService`
- `PackageService`
- `RunLogService`
- `SdkKnowledgeService`

桌面端 Slint callbacks 只调用这些服务，不直接拼业务文件。

### 4.4 `adm-desktop`

重建 Slint UI 结构：

- 拆分当前单个 `main.slint`，至少分为：
  - `shell.slint`
  - `design_workbench.slint`
  - `pipeline.slint`
  - `patch.slint`
  - `package.slint`
  - `run_log.slint`
  - `sdk.slint`
  - `common.slint`
- 拆分 `src/main.rs`：
  - `main.rs`
  - `callbacks.rs`
  - `view_model.rs`
  - `design_callbacks.rs`
  - `pipeline_callbacks.rs`
  - `package_callbacks.rs`
  - `sdk_callbacks.rs`
  - `log_callbacks.rs`
- 所有页面使用固定响应式分区；不实现可拖拽边界。
- 所有显示文案直接中文化，不再依赖大表替换英文。

## 5. 数据契约优先级

第一优先级不是改 Slint 外观，而是先让 Rust 拥有可测试的数据闭环：

1. 读取旧知识库完整结构。
2. 生成空 `WorkbenchState`。
3. 在 Rust 中执行 checklist 勾选并刷新 node/domain/project progress。
4. 在 Rust 中执行 L4 选择、主选项和软冲突计算。
5. 在 Rust 中保存 L5 JSON，并返回结构化校验错误。
6. 在 Rust 中生成右侧 `摘要/缺失项/风险/校验`。
7. 在 Rust 中完成导出预览和写出。
8. 在 Rust 中完成模板读取、导入、另存、删除。
9. UI 再绑定这些状态和命令。

任何只更新 Slint、没有上述状态回写的页面，不计入完成。

## 6. 实施阶段

### 阶段 0：冻结参考契约

目标：把旧版交互契约变成 Rust 开发用验收清单。

工作：

- 记录 `core/ui/main_window.py` 六任务区结构。
- 记录 `core/ui/app_window.py` 设计工作台事件和状态字段。
- 记录 `core/design/engine.py` 的派生指标和 mutation API。
- 记录 `core/ui/pipeline_panel.py` 的 Step00-14、运行控制、Step07 风格确认。
- 记录 `core/save/manager.py`、`core/config/ai_config.py`、`core/packaging/service.py`、`core/ui/log_panel.py`、`core/ui/sdk_panel.py` 的服务边界。
- 建立 Rust 端 acceptance checklist。

验收：

- 计划文件中所有旧版功能都能映射到 Rust crate、service、UI 页面。

### 阶段 1：Rust 设计数据与 `WorkbenchState`

目标：`adm-design` 具备旧版设计工作台的核心状态能力。

工作：

- 新增结构化数据 loader，覆盖 domain order、domains、profile schema、gameplay systems、templates。
- 新增 `WorkbenchState`、`NodeState`、`ChecklistState`、`OptionGroupState`、`DesignEntityState`。
- 新增 `DesignEngine` mutation API。
- 新增 progress、missing、risk、validation、quality、completion summary 派生。
- 新增 L4 选项冲突和 L5 entity schema 校验。
- 新增单元测试，使用真实 `knowledge/design_data` fixture。

验收：

- `cargo test -p adm-design` 覆盖空状态、节点勾选、L4 主选项、L5 JSON 错误、右侧四页签摘要。
- 不需要启动 UI 即可证明设计工作台数据闭环可运行。

### 阶段 2：Rust 工作台服务层

目标：`adm-application` 提供 UI 可调用的稳定命令接口。

工作：

- 新增 `WorkbenchService`。
- 接入 autosave / archive workspace。
- 接入 export adapter。
- 接入 template repository。
- 接入 save manager。
- 对 Slint 需要的 view model 做一次性构建和增量刷新。

验收：

- `cargo test -p adm-application` 覆盖 load/mutate/export/template/autosave。
- 每个 UI 操作都有对应 service command，不在 Slint callback 里直接改 JSON。

### 阶段 3：设计工作台 Slint 重建

目标：完成用户要求的设计工作台 UI 与交互逻辑。

工作：

- 左侧复刻 `领域总览`、`项目画像`、领域进度信息。
- 中间复刻领域标题、搜索、过滤、节点卡片。
- 节点卡片接入：
  - checklist 勾选
  - L4 option group 单选/多选
  - 主选项
  - 软冲突提示
  - L5 JSON 编辑、保存、清空、校验错误
  - 设计描述
  - 风险说明
  - 不适用原因
- 底部接入 AI 访谈面板。
- 右侧接入 `摘要/缺失项/风险/校验` 四页签。
- 顶部接入项目名称、导出格式、导出、存档管理、模板查看、另存为模板、重置。

验收：

- 用户在 UI 勾选 checklist 后，domain progress、node progress、右侧摘要立即变化。
- 用户选择 L4 选项后，主选项和冲突提示可刷新。
- 用户保存 L5 JSON 后，错误在节点卡和右侧校验页签同时出现。
- 导出文件来自真实 `WorkbenchState`，不是当前页面文本。

### 阶段 4：AI 访谈接入

目标：AI 访谈不再是静态槽位。

工作：

- Rust 端移植 AI interview state：
  - message list
  - question route
  - candidate node ids
  - missing/risk/L4/L5 prompt context
  - AI output partitions
  - replay/runtime record
- 接入已有 `adm-ai` provider router。
- 支持真实 provider 和 mock provider。
- 支持 AI 结果应用到 `WorkbenchState`，必须经 validator 通过后写入。

验收：

- 未配置真实 provider 时，UI 明确显示缺失配置，不静默失败。
- 配置 provider 后，访谈能生成问题、记录回复，并可将高置信结果应用到节点/L4/L5。
- 所有 AI 写入都有 provenance。

### 阶段 5：开发流水线 Step00-14 重建

目标：Rust UI 具备旧版开发流水线的真实状态和控制路径。

工作：

- 在 Rust 中定义 Step00-14 metadata：
  - Step00 初始想法输入
  - Step01 玩法框架确认
  - Step02 设计评审冻结
  - Step03 程序需求确认
  - Step04 美术需求确认
  - Step05 程序需求评审
  - Step06 美术需求评审
  - Step07 美术风格生成与确认
  - Step08 程序开发计划
  - Step09 美术制作计划
  - Step10 资产契约对齐
  - Step11 程序开发执行
  - Step12 美术制作执行
  - Step13 场景组装
  - Step14 集成验证
- 复刻分组：
  - 设计阶段：00-06
  - 风格确认：07
  - 计划阶段：08-10
  - 执行阶段：11-14
- 新增 `PipelineService`：
  - load state
  - select step
  - run range
  - stop
  - export design handoff
  - load logs
  - load semantic quality
  - Step07 style confirmation
- UI 接入：
  - 左侧阶段卡
  - 右侧配置栏
  - 阶段详情
  - 运行日志
  - 运行/停止按钮状态

验收：

- Step 状态和旧版状态映射一致。
- 运行范围 N 到 M 后，UI 可刷新当前运行状态和日志。
- 停止按钮写入 stop request 并更新 UI。
- Step07 可选择风格并确认，确认结果持久化。

### 阶段 6：补充开发、打包、运行日志、SDK

目标：四个辅助任务区接入真实服务。

工作：

- 补充开发：
  - 需求输入
  - 分析按钮
  - patch table
  - 状态刷新
- 打包阶段：
  - Step14 gating
  - package service
  - 输出 JSON 结果
  - 错误状态
- 运行日志：
  - 日志读取
  - level filter
  - clear
  - refresh
  - export JSONL
- SDK 知识库：
  - name/url 输入
  - add
  - pending
  - approve
  - reject
  - approved prompt context

验收：

- 每个按钮都调用 Rust service，不是占位。
- 所有列表都能从实际存储刷新。
- 错误信息以中文显示。

### 阶段 7：存档、配置、发布入口

目标：保证 Rust 双击版能独立运行并持久化。

工作：

- 统一 data root 策略。
- 存档管理与设计工作台状态打通。
- 顶部/底部状态栏接入：
  - AI 配置状态
  - pipeline progress
  - system running/idle
- 正式修复 Windows GUI 子系统：
  - release exe 不弹出额外 cmd 窗口
  - smoke 模式仍可命令行运行
- 正式 staging 到 `RUST/dist/AutoDesignMaker-rust`。

验收：

- 删除旧 Python 运行依赖后，Rust exe 可独立启动。
- 双击 exe 显示中文六任务区 UI。
- 重启后能恢复存档、工作台状态和关键配置。
- dist 包通过 release doctor/smoke。

### 阶段 8：视觉与交互验收

目标：解决当前显示错误，确保界面可用。

工作：

- 统一中文文案。
- 固定顶部导航、状态栏、页面分区。
- 检查 1280x860、1600x900、1920x1080。
- 检查长文本换行、按钮文字溢出、列表空态、滚动区高度。
- 检查所有页面无重叠、无白屏、无不可点击死区。
- 建立 Slint smoke/截图检查流程。

验收：

- 六个顶层按钮切换稳定。
- 任一页面空数据状态也能显示结构，不白屏。
- 中英文混杂清零，除技术路径、SDK 名称、模型名外全部中文。

## 7. 开发顺序

严格按以下顺序执行：

1. `adm-design` 数据契约和引擎。
2. `adm-application` 工作台服务。
3. 设计工作台 UI 绑定真实状态。
4. AI 访谈与 provider 写入。
5. Step00-14 流水线模型和服务。
6. 流水线 UI。
7. 补充开发、打包、日志、SDK。
8. 发布入口和视觉验收。

禁止先继续堆 UI 静态布局。没有数据 contract 和 service command 的 UI 视为未完成。

## 8. 验收标准

### 8.1 设计工作台

- 能读取真实 `knowledge/design_data`。
- 能显示领域总览、项目画像、节点、checklist、L4、L5。
- checklist/L4/L5 操作能写入 `WorkbenchState`。
- 右侧四页签实时派生。
- 模板查看、载入、另存、删除可用。
- 导出 markdown/json/txt/text/prompt 可用。
- 存档管理可用。

### 8.2 开发流水线

- Step00-14 显示完整。
- 分组、阶段状态、阶段详情、日志显示完整。
- 支持从步骤 N 到 M 运行。
- 支持停止。
- 支持导出设计到流水线。
- Step07 风格确认可用。

### 8.3 补充开发

- 能输入新增需求。
- 能分析并生成 patch 列表。
- 能显示状态和更新时间。

### 8.4 打包阶段

- Step14 未成功时阻止打包。
- Step14 成功后能运行打包服务。
- 输出结果可读并持久化。

### 8.5 运行日志

- 能按级别过滤。
- 能刷新、清空、导出。
- 日志来源和上下文可读。

### 8.6 SDK 知识库

- 能添加 SDK。
- 能查看待审核/已批准/已拒绝。
- 能批准/拒绝。
- 能生成 approved prompt context。

### 8.7 发布与独立运行

- `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe` 双击无 cmd 窗口。
- UI 非白屏。
- 纯中文界面。
- 不依赖 Python 运行时代码。
- smoke、workspace tests、release doctor 通过。

## 9. 测试策略

- `cargo fmt`
- `cargo test -p adm-design`
- `cargo test -p adm-application`
- `cargo test -p adm-ui-model`
- `cargo test -p adm-desktop`
- `cargo test --workspace`
- `AutoDesignMaker-rust.exe --smoke`
- release staging doctor
- Slint UI screenshot smoke
- 人工视觉验收：
  - 1280x860
  - 1600x900
  - 1920x1080
- 不执行真实 Unity PlayMode 自动验收。

## 10. 预计工作量

这是一次 Rust 侧产品级重建，不是单文件 UI 修补。

粗略估算：

- 设计数据和 `WorkbenchState`：2 到 3 天
- 设计工作台服务和 UI：3 到 5 天
- AI 访谈真实接入：1.5 到 3 天
- Step00-14 流水线模型、服务和 UI：3 到 5 天
- 补充开发、打包、日志、SDK：1.5 到 3 天
- 发布入口、视觉修复、回归测试：1 到 2 天

总计约 12 到 21 个工程日。  
如果先交付可验收的核心版本，建议第一里程碑只覆盖 `adm-design + 设计工作台完整闭环 + 存档/导出/模板`，预计 5 到 8 个工程日，占总工作量约 35% 到 45%。

## 11. 风险与处理

- 风险：旧版 `DesignEngine` 逻辑较大，直接人工重写容易漏字段。  
  处理：先写真实数据 fixture 测试，再迁移 UI。

- 风险：Slint 组件一次性写太大导致布局错误。  
  处理：拆分组件，每个页面独立 view model，先空态后数据态。

- 风险：当前 Rust pipeline 与旧版 Step00-14 模型不一致。  
  处理：保留已有 Rust 核心产物能力，但新增 Step00-14 UI/service 适配层，不把 5 阶段简化模型伪装成旧版流水线。

- 风险：AI provider 写入会污染设计状态。  
  处理：所有 AI 写入必须走 validator，并保留 provenance。

- 风险：发布包和开发 exe 不一致。  
  处理：所有最终验收必须基于 `dist` 正式 staging 产物。

## 12. 下一步执行建议

下一轮开发从阶段 1 开始：

1. 在 `adm-design` 增加 `workbench_state.rs`、`data_repository.rs`、`design_engine.rs`。
2. 先把旧 `knowledge/design_data` 完整加载成 Rust typed model。
3. 写 `empty_state`、`normalize_state`、`set_checklist_item`、`set_option_group_option`、`set_option_group_primary`。
4. 写 `summary/missing/risk/validation` 的最小可验收派生。
5. 通过 `cargo test -p adm-design` 后，再开始 Slint 页面绑定。

