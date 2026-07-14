# 关键文件清单

> 上次更新：2026-06-19  
> 来源：完整项目扫描

以下文件是项目的核心，理解它们就能把握整个系统。

---

## 核心运行时（core/）

### core/main.py
- **行数**：229
- **职责**：唯一程序入口，包含 run_range() 流水线编排器
- **关键函数**：run_range(from_step, stop_step, auto_approve, skip_preflight, skip_all_gates, skip_gates)
- **调用链**：emit_dependency_graph → topological_step_order → 循环执行 plugin.run() → artifact review/validation → retry_sync
- **缓存状态**：✓ 有效（2026-06-26）

### core/engines/generation.py
- **行数**：5370
- **职责**：Step00-16 阶段的全部业务输出逻辑，单一巨文件
- **关键函数**：apply_development_plan_outputs(), _stage0_outputs() ~ _stage14_outputs(), _stage7_art_style_generation_outputs(), _stage8_art_style_confirmation_outputs()（旧兼容）, _parse_design_text()
- **注意事项**：每个 _stageN_outputs() 是独立逻辑块，互不调用，全部返回 dict
- **缓存状态**：✓ 有效（2026-06-26）

### core/registry.py
- **行数**：105
- **职责**：STEP_SPECS 注册表，Step00-16 步骤元数据（名称、依赖、slug）
- **关键常量**：STEP_SPECS dict, DESIGN_STEP_SPECS, max_step_number()
- **缓存状态**：✓ 有效（2026-06-26）

### core/paths.py
- **行数**：135
- **职责**：所有路径常量的单一来源，基于 .project_root 定位
- **关键函数**：locate_project_root(), get_stage_artifact_dir(), project_path()
- **关键常量**：PROJECT_ROOT, ARTIFACTS_DIR, SOURCE_ARTIFACTS_DIR, SANDBOX_DIR
- **缓存状态**：✓ 有效（2026-06-19）

### core/plugin_manager.py
- **行数**：129
- **职责**：动态加载 pipeline/_registry.json，实例化 StagePlugin
- **关键类**：PluginManager
- **关键方法**：load_stage(stage_id), validate()
- **缓存状态**：✓ 有效（2026-06-19）

### core/stage_plugin.py
- **行数**：51
- **职责**：StagePlugin 抽象基类，所有步骤插件必须继承
- **关键方法**：execute(ctx), run(ctx), validate_inputs/outputs()
- **缓存状态**：✓ 有效（2026-06-19）

### core/context.py
- **行数**：48
- **职责**：StageContext、StageResult 数据结构定义
- **关键类型**：StageStatus = Literal["success", "failed", "skipped", "blocked", "waiting_confirmation"]
- **缓存状态**：✓ 有效（2026-06-26）

---

## 引擎层（core/engines/）

### core/engines/delta_patch.py
- **行数**：未读取（非关键路径）
- **职责**：增量补丁生成器（步骤14）
- **缓存状态**：未缓存

### core/engines/handoff_loader.py
- **行数**：未读取
- **职责**：加载设计交接契约
- **缓存状态**：未缓存

---

## 适配器层（core/adapters/）

### core/adapters/base.py
- **行数**：29
- **职责**：ModelAdapter 接口定义
- **关键类**：ModelAdapter, ModelTask, ModelResult
- **缓存状态**：✓ 有效（2026-06-19）

### core/adapters/openai_adapter.py
- **行数**：未完整读取
- **职责**：OpenAI 兼容 API 适配器
- **用途**：流水线生成任务
- **缓存状态**：部分缓存

### core/adapters/codex_adapter.py
- **行数**：未完整读取
- **职责**：Codex CLI 包装
- **用途**：GUI AI 访谈
- **缓存状态**：部分缓存

### core/adapters/registry.py
- **行数**：已读取
- **职责**：get_adapter(name) 工厂函数
- **缓存状态**：✓ 有效（2026-06-19）

---

## 制品审查（core/artifact/）

### core/artifact/graph.py
- **行数**：99
- **职责**：依赖图构建、拓扑排序
- **关键函数**：topological_step_order(), emit_dependency_graph()
- **缓存状态**：✓ 有效（2026-06-19）

### core/artifact/preflight.py
- **行数**：未完整读取
- **职责**：执行前预检（依赖、知识引用、schema）
- **关键函数**：preflight_stage_contract()
- **缓存状态**：部分缓存

### core/artifact/reviewer.py
- **行数**：125
- **职责**：执行后审查，4个 reviewer
- **关键函数**：run_review_pipeline()
- **缓存状态**：✓ 有效（2026-06-19）

### core/artifact/validator.py
- **行数**：156
- **职责**：执行后验证，7个 validator
- **关键函数**：run_artifact_validators()
- **缓存状态**：✓ 有效（2026-06-19）

---

## 源包系统（core/source/）

### core/source/importer.py
- **行数**：532
- **职责**：源包导入引擎，run_import_step() 入口
- **关键函数**：run_import_step(), forbidden_runtime_matches()
- **注意事项**：forbidden_runtime_matches 检查 CrewAI 残留，严禁导入
- **缓存状态**：✓ 有效（2026-06-19）

### core/source/groups.py
- **行数**：未完整读取
- **职责**：SourceGroup 数据类定义
- **缓存状态**：部分缓存

### core/source/finder.py
- **行数**：未完整读取
- **职责**：find_sources() 源包发现
- **缓存状态**：部分缓存

---

## 配置加载（core/config/）

### core/config/loader.py
- **行数**：213
- **职责**：加载 settings/*.toml，构建 LLM 配置
- **关键函数**：load_config(), get_api_config(), build_llm()
- **关键常量**：DEFAULT_APP_CONFIG（默认 base_url: vip.auto-code.net, model: gpt-5.5）
- **缓存状态**：✓ 有效（2026-06-19）

### core/config/integrity.py
- **行数**：已读取
- **职责**：启动时数据完整性检查
- **缓存状态**：✓ 有效（2026-06-19）

---

## 运行时控制（core/runtime/）

### core/runtime/control.py
- **行数**：116
- **职责**：停止/恢复信号管理，PipelineStopRequested 异常
- **关键函数**：request_stop(), stop_requested(), mark_stopped()
- **缓存状态**：✓ 有效（2026-06-19）

### core/runtime/preflight.py
- **行数**：已读取
- **职责**：Unity 项目路径预检
- **关键函数**：run_actual_development_preflight(), load_project_settings()
- **缓存状态**：✓ 有效（2026-06-19）

### core/runtime/pipeline_state.py
- **行数**：已读取
- **职责**：流水线步骤状态读写（markdown 格式）
- **缓存状态**：✓ 有效（2026-06-19）

---

## 存档系统（core/save/）

### core/save/manager.py
- **行数**：834
- **职责**：项目存档管理、快照同步
- **关键函数**：ensure_current_save(), retry_sync(), load_save()
- **注意事项**：每次步骤成功后调用 retry_sync()，复制 sandbox/ 到 saves/{id}/snapshots/
- **缓存状态**：✓ 有效（2026-06-19）

---

## 设计引擎（core/design/）

### core/design/engine.py
- **行数**：已读取（部分）
- **职责**：DesignEngine，管理游戏设计决策状态
- **关键类**：DesignEngine
- **关键方法**：empty_state(), set_checklist_item(), effective_node_state(), lint()
- **缓存状态**：部分缓存（2026-06-19）

### core/design/exporter.py
- **行数**：已读取
- **职责**：导出设计文档为 .md/.json
- **关键函数**：write_export(), export_preview_lines()
- **缓存状态**：✓ 有效（2026-06-19）

### core/design/ai_backend.py
- **行数**：已读取
- **职责**：CodexCliBackend，AI 后端封装
- **关键类**：CodexCliBackend
- **关键方法**：run_turn(), run_json_task()
- **缓存状态**：✓ 有效（2026-06-19）

---

## GUI 层（core/ui/）

### core/ui/gui_app.py
- **行数**：31
- **职责**：GUI 入口
- **关键函数**：main() — 实例化 CommercialDesignApp
- **缓存状态**：✓ 有效（2026-06-19）

### core/ui/app_window.py
- **行数**：1861
- **职责**：CommercialDesignApp 主窗口（设计工作台）
- **关键类**：CommercialDesignApp(tk.Tk)（计划改为 tk.Frame）
- **注意事项**：render() 全量重建导致卡顿，需重构为局部更新
- **缓存状态**：✓ 有效（2026-06-19）

### core/ui/theme.py
- **行数**：已读取
- **职责**：颜色和字体常量
- **关键常量**：COLORS（22个颜色）, FONT_BODY/TITLE/SECTION 等
- **缓存状态**：✓ 有效（2026-06-19）

### core/ui/ai_interview_window.py
- **行数**：1453
- **职责**：AI 访谈窗口（弹出式 Toplevel）
- **关键类**：AIInterviewWindow
- **关键方法**：run_ai_turn(), worker_run_ai_turn()
- **缓存状态**：✓ 有效（2026-06-19）

---

## 流水线插件（pipeline/）

### pipeline/_registry.json
- **行数**：127
- **职责**：插件注册表，声明20个插件的 module/class/title
- **格式**：{"stages": {"D1": {...}, "00": {...}, ...}}
- **缓存状态**：✓ 有效（2026-06-19）

### pipeline/step_00_idea_intake/plugin.py
- **行数**：21
- **职责**：步骤00插件，创意收集
- **模式**：stage_id="00", _source_groups=[SourceGroup("concept", ...)], execute() 调用 run_import_step + apply_development_plan_outputs
- **缓存状态**：✓ 有效（2026-06-19）

**其他步骤插件（01-17）**：结构与 step_00 相同，仅 stage_id 和 _source_groups 不同；Step07/08 为风格生成与人工确认门禁，原 07-15 后移到 09-17。

---

## 制品注册表（pipeline/artifact_layer/）

### pipeline/artifact_layer/registry.json
- **行数**：已读取
- **职责**：Step00-16 阶段的制品依赖声明
- **格式**：{"artifacts": [{"id": "stage_00.concept_bundle", "stage": 0, "depends_on": [], "tasks": [...]}]}
- **缓存状态**：✓ 有效（2026-06-26）

---

## 工具脚本（tools/）

### tools/validators/
- **职责**：独立验证工具（contract、compile、environment、output）
- **缓存状态**：未缓存

### tools/build/
- **职责**：PyInstaller 构建脚本
- **关键文件**：AutoDesignMaker.spec, build.py
- **缓存状态**：未缓存

---

## 配置文件（settings/）

### settings/app.toml
- **行数**：已读取
- **职责**：应用配置（git 提交）
- **缓存状态**：✓ 有效（2026-06-19）

### settings/api_config.example.toml
- **行数**：已读取
- **职责**：API 配置模板
- **缓存状态**：✓ 有效（2026-06-19）

### settings/api_config.toml
- **行数**：未读取（gitignore，含密钥）
- **职责**：实际 API 密钥
- **缓存状态**：不缓存

### settings/project_settings.json
- **行数**：未读取（gitignore，本地配置）
- **职责**：Unity 项目路径等本地设置
- **缓存状态**：不缓存

---

## 快速参考

**最重要的5个文件**（理解它们就能修改流水线）：
1. `core/main.py` — 入口和编排器
2. `core/engines/generation.py` — Step00-16 阶段业务逻辑
3. `core/registry.py` — 步骤元数据
4. `core/paths.py` — 路径常量
5. `pipeline/artifact_layer/registry.json` — 制品依赖图

**新增步骤的关键文件**（按顺序修改）：
1. `pipeline/step_NN_name/plugin.py` — 创建插件
2. `pipeline/_registry.json` — 注册插件
3. `pipeline/artifact_layer/registry.json` — 声明制品
4. `core/registry.py::STEP_SPECS` — 添加元数据
