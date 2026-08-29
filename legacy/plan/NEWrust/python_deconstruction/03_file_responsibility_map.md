# Python 文件职责图

状态：草案。

| 文件/目录 | 职责 | 分类 | 证据 |
| --- | --- | --- | --- |
| `gui_app.py` | 根 GUI 包装入口 | authoritative | 导入 `core.ui.gui_app.main` |
| `core/ui/gui_app.py` | GUI 启动生命周期、退出锁释放、自动恢复 | authoritative | `main()` 创建 `MainWindow` |
| `core/ui/main_window.py` | 六任务区壳、状态栏、面板懒加载 | authoritative | `_get_*_panel` 和 `_show_*` |
| `core/ui/app_window.py` | 设计工作台 | authoritative | `CommercialDesignApp` |
| `core/ui/pipeline_panel.py` | 开发流水线 UI | authoritative | `PipelinePanel` |
| `core/main.py` | pipeline 运行编排 | authoritative | `run_range()` |
| `core/paths.py` | 路径事实源 | authoritative | `.project_root` 定位和 runtime dirs |
| `core/save/manager.py` | 存档和 draft/formal archive | authoritative | draft/save constants and manager funcs |
| `pipeline/_registry.json` | stage plugin 注册表 | authoritative | PluginManager manifest |
| `core/registry.py` | Step metadata 和依赖 | authoritative | `STEP_SPECS` |
| `core/design/data_loader.py` | 设计静态数据加载、模板展开、旧 ID 兼容、实体 schema 预验证 | authoritative | `load_project_data()` |
| `core/design/engine.py` | 设计工作台业务状态、L4/L5、冲突、质量、completion gate | authoritative | `DesignEngine` |
| `core/design/exporter.py` | 用户导出 payload 和 markdown/json/text/prompt 渲染 | authoritative | `build_payload()`、`write_export()` |
| `core/design/export_adapter.py` | 设计工作台到流水线 source package 交接 | authoritative | `export_concept_package()` |
| `core/artifact/registry_loader.py` | artifact registry 加载和 reviewer/validator 白名单 | authoritative | `load_registry()` |
| `core/artifact/graph.py` | artifact 依赖图和步骤拓扑排序 | authoritative | `topological_step_order()` |
| `core/artifact/preflight.py` | 步骤执行前 artifact contract 预检 | authoritative | `preflight_stage_contract()` |
| `core/artifact/reviewer.py` | 步骤执行后 4 reviewer 审查 | authoritative | `run_review_pipeline()` |
| `core/artifact/validator.py` | 步骤执行后 7 validator 验证 | authoritative | `run_artifact_validators()` |
| `pipeline/artifact_layer/registry.json` | Step00-14 artifact/task/schema 事实源 | authoritative | `core.artifact.registry_loader.REGISTRY_PATH` |
| `pipeline/artifact_layer/dependency_graph.json` | 当前物化依赖图参考输出 | reference | 可由 `emit_dependency_graph()` 再生成 |
| `core/ui/save_manager_dialog.py` | 存档管理 UI，保存/加载/重命名/删除/打开存档目录 | authoritative | `SaveManagerDialog` |
| `core/engines/execution_objects/design_project.py` | 将设计工作台状态保存为 verified execution object | authoritative | `save_design_project()`、`load_latest_design_project()` |
| `core/config/ai_config_schema.py` | AI 配置 v3 schema，dev/image/completion 三类 entry | authoritative | `SCHEMA_VERSION = 3` |
| `core/config/ai_config.py` | AI 配置加载、迁移、保存、active entry/profile | authoritative | `load_ai_config()`、`save_ai_config()` |
| `core/config/validator.py` | AI 配置静态验证和 CLI 可用性检查 | authoritative | `AIConfigValidator` |
| `core/adapters/base.py` | AI adapter 统一任务/结果接口 | authoritative | `ModelTask`、`ModelResult`、`ModelAdapter` |
| `core/adapters/registry.py` | pipeline adapter 工厂 | authoritative | `get_adapter()` |
| `core/adapters/openai_adapter.py` | OpenAI-compatible 模型调用 | authoritative | `OpenAIAdapter.generate()` |
| `core/adapters/codex_adapter.py` | Codex CLI adapter | authoritative | `CodexAdapter.generate()` |
| `core/adapters/claude_code_model_adapter.py` | Claude Code CLI adapter | authoritative | `ClaudeCodeModelAdapter.generate()` |
| `core/adapters/completion_adapter.py` | completion 类 AI adapter 工厂 | authoritative | `build_completion_adapter()` |
| `core/ui/log_entry.py` | 结构化日志 entry 和 JSONL 读写 | authoritative | `JsonlLogWriter` |
| `core/ui/log_panel.py` | 结构化日志面板、过滤、清空、导出 | authoritative | `LogPanel` |
| `core/design/structured_handoff.py` | D4 structured handoff 生成 | authoritative | `write_structured_handoff()` |
| `core/design/structured_context.py` | structured handoff 消费上下文 | authoritative | `StructuredDesignContext` |

待补充：AI 访谈 UI 与 ucos 写入链、packaging validation 的函数级职责。
