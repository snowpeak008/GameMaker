# 项目架构理解

> 基于 2026-06-19 完整代码阅读  
> 来源：core/、pipeline/、artifact_layer/ 目录完整扫描

---

## 总体定位

AutoDesignMaker 是一个**Step00-16 确定性游戏设计文档流水线**。用户在 GUI 中完成设计决策后，导出为 `.md`/`.json` 文件，放入当前 draft 的 `source_artifacts/`，流水线将其转换为可执行的 Unity C# 代码和美术资产需求。

---

## 八层职责架构

| 层 | 目录 | 核心职责 | 可修改性 |
|----|------|----------|----------|
| **步骤插件层** | `pipeline/` | 每个阶段的业务逻辑，21个插件（D1-D4 + step_00~16） | 主要修改区 |
| **运行骨架层** | `core/` | 编排器、适配器、制品审查、工具函数 | 谨慎修改 |
| **知识层** | `knowledge/` | 设计规则、domain 数据、schema、skill、治理文档 | 内容更新 |
| **配置层** | `settings/` | API 密钥（api_config.toml）、Unity 路径（project_settings.json） | 本地修改 |
| **工具层** | `tools/` | 验证器、媒体工具、开发脚手架、构建脚本 | 按需添加 |
| **认知层** | `ucos/` | 游戏设计会话的 episodic 记忆（仅保留此类记忆） | 不添加 |
| **注册表层** | `pipeline/artifact_layer/` | registry.json 声明 Step00-16 制品依赖关系 | 随步骤更新 |
| **沙盒层** | `sandbox/`, `saves/`, `logs/` | 运行时输出，gitignore | 程序自动管理 |

---

## 核心执行链路

```
用户点击 GUI "导出"
  ↓
DesignEngine.export_project()  写入 .md/.json 到 sandbox/source_artifacts/
  ↓
用户运行：python -m core.main --auto-approve --from-step 0 --stop-step 16
  ↓
core/main.py::run_range()
  ├─ 1. emit_dependency_graph()  构建拓扑排序
  ├─ 2. topological_step_order()  决定执行顺序
  ├─ 3. for step_num in order:
  │     ├─ plugin_manager.load_stage(step_num)  动态加载 plugin.py
  │     ├─ plugin.execute(ctx)  执行步骤
  │     │    ├─ run_import_step()  导入源包（core/source/importer.py）
  │     │    ├─ apply_development_plan_outputs()  业务逻辑（core/engines/generation.py）
  │     │    └─ 返回 StageResult
  │     ├─ preflight_stage_contract()  预检制品合同
  │     ├─ run_review_pipeline()  4个 reviewer
  │     ├─ run_artifact_validators()  7个 validator
  │     └─ retry_sync()  存档快照
  └─ 4. write_run_state()  写入运行状态
```

---

## 三大引擎详解

### 1. core/engines/generation.py（5370行）

**职责**：Step00-16 阶段的全部业务输出逻辑。

**关键函数**：
- `apply_development_plan_outputs(step_num, report)` — 统一入口，根据 step_num 调度到对应 _stageN_outputs()
- `_stage0_outputs()` ~ `_stage14_outputs()` — 旧业务阶段生成逻辑；当前映射中旧 Step07-14 后移到 Step09-16
- `_stage7_art_style_generation_outputs()` — 美术风格生成、推荐评分与人工确认门禁；`_stage8_art_style_confirmation_outputs()` 仅保留旧调用兼容
- `_parse_design_text()` — 解析设计文档的通用解析器
- `_apply_unity_package_changes()` — 修改 Unity manifest.json 的依赖项

**特点**：
- 单一巨文件，Step00-16 逻辑混合
- 每个 _stageN_outputs() 返回 dict：`{"status": "success", "outputs": {...}}`
- 依赖 `core/io.py` 的 read_json/write_json 读写制品文件

### 2. core/design/engine.py（DesignEngine）

**职责**：管理游戏设计决策状态。加载 knowledge/design_data/domains/ 的17个领域定义，维护 project_state（用户选择的节点、checklist、L4选项）。

**关键方法**：
- `empty_state()` — 初始化空白项目状态
- `set_checklist_item()` — 用户勾选 checklist 时调用
- `effective_node_state()` — 计算节点实际状态（completed/selected/not_started）
- `lint()` — 调用 CrossLayerRuleSet 进行一致性检查

**与流水线的关系**：设计完成后通过 `core/design/exporter.py::write_export()` 导出为 `devflow_Concept_*.md`，这些文件成为步骤00的输入源包。

### 3. core/design/ai_backend.py（CodexCliBackend）

**职责**：封装 Codex CLI 作为 AI 后端，支持 AI 访谈和代码生成。

**关键方法**：
- `run_turn()` — 执行一轮 AI 对话，返回响应文本
- `run_json_task()` — 执行结构化任务，强制返回 JSON
- `validate_process_registry()` — 管理后台 Codex 进程生命周期

**用途**：
- AIInterviewWindow 用它进行游戏设计访谈
- 未来的流水线 AI 对话面板也会复用此后端

---

## 关键接口与抽象

### StagePlugin（core/stage_plugin.py）

所有22个步骤插件的基类。

```python
class StagePlugin(ABC):
    @property
    @abstractmethod
    def stage_id(self) -> str: ...
    
    @abstractmethod
    def execute(self, context: StageContext) -> StageResult: ...
    
    def run(self, context: StageContext) -> StageResult:
        # 模板方法：validate_inputs → execute → validate_outputs
```

### StageContext & StageResult（core/context.py）

```python
@dataclass
class StageContext:
    stage_id: str
    inputs: dict[str, Any]
    outputs: dict[str, Any]
    metadata: dict[str, Any]
    knowledge: dict[str, str]  # knowledge_refs 注入
    skills: dict[str, Any]     # skill 定义
    test_mode: bool

@dataclass
class StageResult:
    status: StageStatus  # "success" | "failed" | "skipped" | "blocked" | "waiting_confirmation"
    outputs: dict[str, Any]
    errors: list[str]
    warnings: list[str]
```

### ModelAdapter（core/adapters/base.py）

AI 模型适配器接口（可插拔）。

```python
class ModelAdapter:
    def generate(self, task: ModelTask) -> ModelResult: ...
```

实现类：
- `CodexAdapter` — Codex CLI 包装
- `OpenAIAdapter` — OpenAI 兼容 API
- `ClaudeCodeAdapter` — Claude Code CLI（预留）

---

## 制品与依赖管理

**pipeline/artifact_layer/registry.json**：声明 Step00-16 的制品 bundle，每个 bundle 包含：
- `stage`: int
- `depends_on`: list[str]（上游制品 ID）
- `tasks`: list（子任务定义）
- `knowledge_refs`: list（引用的 knowledge/ 文件）
- `schema_refs`: list（遵循的 schema）

**执行前预检**（`core/artifact/preflight.py`）：
- 检查上游依赖是否已成功
- 检查 knowledge_refs 文件是否存在
- 检查 schema_refs 是否有效

**执行后审查**（`core/artifact/reviewer.py`）：
- dependency_reviewer — 上游制品状态
- knowledge_reviewer — 知识引用可达性
- schema_reviewer — schema 合规性
- consistency_reviewer — 制品内部一致性

**执行后验证**（`core/artifact/validator.py`）：
- file_existence_validator
- schema_contract_validator（步骤10+）
- dependency_status_validator
- 等7个验证器

---

## 数据流总览

```
用户 GUI 设计决策
  ↓ (导出)
sandbox/source_artifacts/devflow_Concept_*.md
  ↓ (步骤00 导入)
sandbox/outputs/artifacts/stage_00/design_machine_files.json
  ↓ (步骤01 生成框架)
stage_01/gameplay_framework_graph.json
  ↓ (步骤02 设计冻结)
stage_02/frozen_design_subsystems.json
  ↓ (步骤03 程序需求)
stage_03/program_requirements.json
  ↓ (步骤07 美术风格生成与确认)
stage_07/style_options.json
stage_07/style_confirmation.json
  ↓ (步骤08/09 程序计划与美术计划)
stage_08/program_task_breakdown.json
stage_09/art_task_breakdown.json
  ↓ (步骤11 程序执行) ← 调用 AI 生成 Unity C# 代码
stage_11/execution_objects.json
  ↓ (步骤13 集成验证)
stage_13/integration_test_report.json
  ↓ (步骤14 构建打包)
stage_14/build_output/
  ↓ (存档)
saves/{id}/snapshots/NNN/full/
```

---

## 配置与环境

**API 配置**（settings/api_config.toml）：
- 默认 base_url: `https://vip.auto-code.net/v1`
- 默认 model: `gpt-5.5`
- 可配置 reasoning_effort

**Unity 配置**（settings/project_settings.json）：
- `development_path`: Unity 项目根目录
- `editor_path`: Unity Editor 可执行文件路径
- 步骤03+ 依赖此配置，未配置时产出 blocked 状态

**预检检查**（`core/runtime/preflight.py::run_actual_development_preflight()`）：
- 验证 Unity 路径存在
- 验证 Assets/ 目录存在
- 返回 blockers 列表（空则通过）

---

## GUI 架构

**设计工作台**（core/ui/app_window.py::CommercialDesignApp）：
- tkinter 实现，当前继承 tk.Tk（计划改为 tk.Frame）
- 16个领域（domains）、节点（nodes）、检查项（checklist）、L4选项组（optionGroups）
- 状态保存在 project_state（内存 dict）
- 导出调用 core/design/exporter.py

**AI 访谈窗口**（core/ui/ai_interview_window.py::AIInterviewWindow）：
- 弹出式 Toplevel 窗口
- 三面板：路线概览 / 对话区 / 输出差异
- 后台线程执行 AI 轮次（worker_run_ai_turn）
- 通过 CodexCliBackend 与 AI 交互

**主题系统**（core/ui/theme.py）：
- 22个颜色常量（COLORS dict）
- 7个字体常量（FONT_BODY、FONT_TITLE 等）
- 所有 UI 组件统一使用，保证视觉一致

---

## 特殊设计决策

**为什么 generation.py 有5370行？**
历史原因。流水线阶段最初都在一个文件里快速迭代，后续未拆分。每个 `_stageN_outputs()` 是独立逻辑块，互不调用；2026-06-26 新增 Step07/08 风格生成与人工确认后，旧 Step07-15 后移为 Step09-17。

**为什么 CommercialDesignApp 每次点击都全量 render()？**  
初版快速实现，未做增量更新。每次状态变化都销毁重建所有 widget，导致卡顿。计划重构为局部更新。

**为什么 ucos/ 目录大部分文件被删除？**  
最近一次清扫（commit b3065f9），只保留 episodic 记忆，其他认知层组件暂时下线。

**为什么有两个适配器（OpenAIAdapter 和 CodexAdapter）？**  
OpenAI 用于流水线生成任务（API 调用），Codex 用于 GUI 内的 AI 访谈（CLI 进程）。两者接口统一但实现不同。
