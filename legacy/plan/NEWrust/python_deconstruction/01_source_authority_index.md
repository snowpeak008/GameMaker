# Source Authority Index

状态：第一轮完成，仍需后续文件级追踪。

目标：把 Python 项目文件按 `authoritative`、`reference`、`quarantine` 分类，避免垃圾内容污染 NEWrust 设计。

本轮证据读取：

- `gui_app.py`
- `core/ui/gui_app.py`
- `core/ui/main_window.py`
- `core/main.py`
- `pipeline/_registry.json`
- `core/registry.py`
- `core/paths.py`
- `core/plugin_manager.py`
- `core/ui/app_window.py` 前 140 行和方法索引
- `core/ui/pipeline_panel.py` 前 220 行和方法索引
- `core/save/manager.py` 前 220 行
- `core/ui/` 文件清单
- `core/runtime/` 文件清单

## 1. 已证实 authoritative

| 路径 | 分类 | 入口证据 | 复刻要求 |
| --- | --- | --- | --- |
| `gui_app.py` | authoritative | 根 GUI 入口，导入 `core.ui.gui_app.main()` | 复刻为 NEWrust desktop 启动入口 |
| `core/ui/gui_app.py` | authoritative | 初始化配置、退出锁释放、启动恢复存档、创建 `MainWindow` | 复刻启动生命周期、锁释放、自动恢复策略 |
| `core/ui/main_window.py` | authoritative | 顶层 Tk window，六任务区导航和状态栏 | 复刻六任务区信息架构和状态栏 |
| `core/ui/app_window.py` | authoritative | `CommercialDesignApp` 由 main window 加载，绑定 `DesignEngine`、模板、导出、AI 访谈、自动保存 | 复刻设计工作台 |
| `core/ui/pipeline_panel.py` | authoritative | `PipelinePanel` 由 main window 加载，调用 `run_range`、runtime control、preflight、Step07 风格确认 | 复刻开发流水线 |
| `core/ui/patch_panel.py` | authoritative | `PatchPanel` 由 main window 加载 | 复刻补充开发入口，需继续追 `core.patch` |
| `core/ui/package_panel.py` | authoritative | `PackagePanel` 由 main window 加载 | 复刻打包阶段入口，需继续追 `core.packaging` |
| `core/ui/log_panel.py` | authoritative | `LogPanel` 由 main window 加载，读取 `RUN_LOGS_DIR` jsonl | 复刻运行日志查看/导出 |
| `core/ui/sdk_panel.py` | authoritative | `SdkPanel` 由 main window 加载 | 复刻 SDK 知识库入口，需继续追 `core.sdk` |
| `core/main.py` | authoritative | 唯一 pipeline 入口，`run_range()` 调用 plugin、artifact preflight/review/validator、save sync、runtime state | 复刻 pipeline orchestrator |
| `pipeline/_registry.json` | authoritative | PluginManager 读取的 D1-D4 + Step00-14 注册表 | 复刻 stage registry |
| `core/plugin_manager.py` | authoritative | 读取 `_registry.json` 并动态加载 StagePlugin | 复刻插件/阶段加载模型或等价 typed registry |
| `core/registry.py` | authoritative | Step00-14 metadata、依赖、状态可运行判断 | 复刻 Step metadata 和依赖图 |
| `core/paths.py` | authoritative | `.project_root` 定位、drafts/session、runtime dirs、saves/logs/settings 路径事实源 | 复刻路径和 data root 策略 |
| `core/save/manager.py` | authoritative | draft/formal archive、save index、manifest、workspace sync、lock 相关基础 | 复刻存档模型 |
| `core/runtime/` | authoritative | runtime control/state/preflight/run_context/locks 等运行控制文件 | 复刻运行控制和锁语义 |
| `pipeline/step_d1_*` 到 `step_d4_*` | authoritative | `pipeline/_registry.json` enabled=true | 复刻设计前置阶段 |
| `pipeline/step_00_*` 到 `step_14_*` | authoritative | `pipeline/_registry.json` enabled=true，`core/main.py` 动态加载 | 复刻 Step00-14 |
| `pipeline/artifact_layer/` | authoritative | `core/artifact/*` 和 `core/main.py` artifact graph/preflight 引用 | 复刻制品注册和依赖图 |

## 2. 已证实或强候选 reference

| 路径 | 分类 | 原因 | 后续处理 |
| --- | --- | --- | --- |
| `RUST/` | reference | 旧 Rust/Slint 重建经验，可参考失败模式和部分 gate 思路，但不继续修补 | 只参考，不复制单体结构 |
| `NEWrust/` | target-workspace | 本轮新开发目标目录，不是 Python 项目事实源 | 开发阶段再按原子计划写入，不反向污染解构 |
| `AI_README.md` | reference | AI 项目导读，提供目录事实和规则 | 作为解构辅助 |
| `README.md` | reference | 用户说明，可能反映产品入口 | 后续用于产品 parity 校验 |
| `项目代码结构.md` | reference | 结构说明，需以代码事实校验 | 辅助，不直接作为权威 |
| `core/ui/workbench.py` | reference pending | AI_README 标注为旧桌面工作台辅助工具，当前无主入口引用 | 二次审计后定为 reference/drop |
| `tools/` | reference pending | 维护工具，不属于主运行链，但部分 validator/build 工具有验收价值 | 按调用证据逐项分类 |
| `knowledge/ai_memory/` | reference | 会话记忆，不是产品运行功能 | 不进入产品复刻，保留开发上下文 |

## 3. 初始 quarantine 候选

这些内容先隔离，不纳入核心复刻，除非后续找到真实入口证据：

| 路径 | 初始分类 | 原因 | 注意 |
| --- | --- | --- | --- |
| `_archive/` | quarantine candidate | 历史档案 | 不删除，只隔离 |
| `bug/` | quarantine candidate | 本地问题材料/过程文档 | 不进入产品复刻 |
| `plan/` 旧计划文件 | quarantine/reference candidate | 计划资料，不是运行时代码；其中 `plan/NEWrust` 是当前计划事实源 | 不作为 Python 产品功能 |
| `drafts/` | generated runtime data | 会话草稿输出 | 作为数据格式样本可参考，不复刻为源码 |
| `sandbox/` | generated runtime data | 历史/兼容运行输出 | 只作为样本 |
| `saves/` | generated runtime data | 正式存档数据 | 作为存档格式样本 |
| `logs/` | generated runtime data | 运行日志输出 | 作为日志格式样本 |
| `__pycache__/`、`.cache/` | quarantine | 生成缓存 | 不复刻 |
| `build/` | quarantine/reference candidate | 构建产物或历史打包材料 | 后续按是否被 tools/build 引用分类 |
| `locks/` | generated runtime data | 当前运行锁文件目录 | 复刻锁语义，不复制实例文件 |
| `_trash/` | quarantine/drop candidate | 已归档清理内容，含旧 Step15-17 和 runtime 输出 | 不进入核心复刻，必要时只抽样验证历史决策 |

完整隔离初稿见 `18_garbage_isolation_draft.md`。

## 4. 第一轮 UI 可达图

```text
gui_app.py
  -> core.ui.gui_app.main()
    -> load_config()
    -> MainWindow()
      -> CommercialDesignApp      # 设计工作台
      -> PipelinePanel            # 开发流水线
      -> PatchPanel               # 补充开发
      -> PackagePanel             # 打包阶段
      -> LogPanel                 # 运行日志
      -> SdkPanel                 # SDK 知识库
```

`MainWindow` 同时负责：

- 顶部六任务区导航。
- 底部 AI 配置状态。
- 底部 pipeline 进度状态。
- 系统运行状态。
- 关闭时流水线停止请求和设计工作台 autosave flush。

## 5. 第一轮 Pipeline 可达图

```text
core.main.run_range()
  -> ensure_run_context()
  -> assert_actual_development_preflight()
  -> emit_dependency_graph()
  -> topological_step_order()
  -> PluginManager.load_stage(stage_id)
  -> StagePlugin.run(ctx)
  -> preflight_stage_contract()
  -> run_review_pipeline()
  -> run_artifact_validators()
  -> update_step_state()
  -> retry_sync()
  -> write_run_state()
```

注册来源：

```text
pipeline/_registry.json
  -> D1-D4
  -> Step00-Step14
```

Step metadata 来源：

```text
core/registry.py
  -> STEP_SPECS
  -> dependency requirements
  -> max_step_number()
```

## 6. 下一轮读取目标

必须继续读取：

- `core/ui/app_window.py` 的 UI 构建和 interaction handlers。
- `core/design/engine.py`、`data_loader.py`、`exporter.py`、模板和 profile schema。
- `core/ui/pipeline_panel.py` 的 `_exec_range`、Step07 风格确认、导出到流水线。
- `core/ui/patch_panel.py` 与 `core/patch/`。
- `core/ui/package_panel.py` 与 `core/packaging/`。
- `core/ui/sdk_panel.py` 与 `core/sdk/`。
- `core/artifact/` 与 `pipeline/artifact_layer/`。
- 每个 `pipeline/step_*/plugin.py`。

## 7. 防偏移记录

```text
plan_reread=done
drift_detected=false
drift_action=none
```
