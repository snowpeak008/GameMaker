# Python 功能复刻清单

状态：草案。

| 功能域 | Python 入口 | 策略 | 状态 |
| --- | --- | --- | --- |
| GUI 启动 | `gui_app.py` -> `core.ui.gui_app.main` | replicate/improve | partial |
| 六任务区导航 | `core/ui/main_window.py` | replicate | partial |
| 设计工作台 | `core/ui/app_window.py` | replicate/improve | pending deep read |
| 开发流水线 | `core/ui/pipeline_panel.py` + `core/main.py` | replicate/improve | pending deep read |
| 补充开发 | `core/ui/patch_panel.py` | replicate | pending |
| 打包阶段 | `core/ui/package_panel.py` | replicate | pending |
| 运行日志 | `core/ui/log_panel.py` | replicate | pending |
| SDK 知识库 | `core/ui/sdk_panel.py` | replicate | pending |
| Step00-14 | `pipeline/_registry.json` + plugins | replicate/improve | pending plugin read |
| 存档 | `core/save/manager.py` | replicate/improve | pending deep read |
| AI 配置/适配器 | `core/config` + `core/adapters` | replicate/improve | pending |
| 制品验证 | `core/artifact` + `pipeline/artifact_layer` | replicate/improve | pending |

## 本轮已提升状态

| 功能域 | 新状态 | 依据 |
| --- | --- | --- |
| 补充开发 | partial confirmed | `PatchPanel.analyze` -> `PatchAnalyzer` -> `PatchStore` |
| 打包阶段 | partial confirmed | `PackagePanel.run_package` -> `core.packaging.run_package` |
| SDK 知识库 | partial confirmed | `SdkPanel` -> `SdkKnowledgeBase` |
| 设计工作台交互 | partial confirmed | checklist/L4/L5/gameplay handlers 已追到 `DesignEngine` 和 `project_state` |
| 开发流水线运行 | partial confirmed | `_exec_range` -> `core.main.run_range` |
| 设计引擎状态与质量门 | confirmed round1 | `DesignEngine.empty_state/normalize_state/quality_metrics/design_completion_summary` |
| 设计静态数据加载 | confirmed round1 | `load_project_data()` 读取 domains/templates/entity_schemas/gameplay options |
| 用户导出 | confirmed round1 | `write_export()` -> markdown/json/text/prompt + archive sidecars |
| 设计到流水线交接 | confirmed round1 | `export_concept_package()` -> Concept/GameplayFramework/Design source packages + structured handoff |
| 制品验证 | confirmed round1 | `preflight_stage_contract` -> `run_review_pipeline` -> `run_artifact_validators` |
| 存档系统 | confirmed round1 | `save_index`、`manifest`、`draft_meta`、archive lock、sync/load/delete、autosave |
| 设计项目 execution object | confirmed round1 | `save_design_project()` manual save 自动 verified |
| AI 配置和 adapter | confirmed round1 | v3 `dev/image/completion` config -> active profile -> adapter factory |
| 运行日志 | confirmed round1 | `run_range()` -> `JsonlLogWriter` -> `LogPanel` |
| Structured handoff | confirmed round1 | `write_structured_handoff()` -> structured decisions/profile/archetype/contracts/traceability/manifest |
| Artifact schema refs | confirmed round1 | `15_artifact_schema_refs_map.md` |

## 当前复刻策略修正

| 功能域 | 复刻边界 |
| --- | --- |
| 设计工作台 | Web UI 复刻信息架构和交互；Rust 后端复刻 `DesignEngine` 状态归一化、质量计算和导出 payload |
| 设计数据 | 数据文件作为资产迁移；加载、模板展开、兼容、验证规则进入 Rust 后端 |
| Pipeline | UI 只发起运行/停止/确认；Rust 后端保留 registry、拓扑、插件/阶段服务、artifact gates |
| Artifact gates | 必须按 contract-first 后端服务复刻，不能降级为静态状态提示 |
| 导出交接 | `export_concept_package` 三包输出和 structured handoff 是核心功能，不可只实现用户导出 |
| 存档 | 必须保留 draft/formal archive 分离、archive lock、file map、snapshot、timeline、execution object ownership |
| AI | 配置 UI 复刻三类 entry；Rust 后端负责 adapter 构造、验证和任务执行边界 |
| 日志 | pipeline run JSONL 是持久化证据；Web UI 只读取、过滤、导出 |
