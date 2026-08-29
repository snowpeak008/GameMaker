# 开发流水线执行/验证/打包职责重构计划

> 目标：修正当前 UI 与流水线阶段职责混乱的问题，把 Step14 集成验证归回执行阶段，取消重复的平级“验证阶段”，将打包能力从开发流水线中拆出为独立“打包阶段”，并暂时取消版本历史与项目审查功能。

---

## 实施结果（2026-07-04）

- 已将开发流水线收口为 Step00-14。
- 已将 Step14 归入“执行阶段”。
- 已移除顶部“验证阶段 / 版本历史 / 项目审查”入口。
- 已新增顶部“打包阶段”，由 `core/packaging/` 和 `core/ui/package_panel.py` 提供。
- 已删除旧 Step15/16/17 流水线插件注册与源码。
- 已移除 `core/version/`、旧验证面板、版本面板、项目审查面板、`tools/audit/`。
- 已清理 `version_manifest` 主动维护路径。
- 已把打包输出改为 `outputs/package/current/`，不再写 `outputs/artifacts/stage_15/`。
- 已将人工源包类型补齐 `SceneAssembly`，并移除旧 `Build/DeltaPatch` 源包类型。
- 已同步 `AI_README.md`、`README.md`、`core/README.md`、`pipeline/README.md`、artifact layer 说明与依赖图。

验证结果：

- `python -m compileall core\packaging core\ui\main_window.py core\ui\pipeline_panel.py core\ui\package_panel.py core\ui\workbench.py core\ui\app_window.py core\registry.py core\main.py core\source\groups.py core\source\importer.py core\engines\generation.py core\iteration core\save\manager.py`：通过。
- `python -m pytest core\tests\unit\test_ui_panels_import.py core\tests\unit\test_validation_cli.py core\tests\unit\test_iteration_development.py core\tests\unit\test_draft_archive_paths.py core\tests\integration\test_plugins.py -q`：40 passed。
- `python -m pytest core\tests -q`：293 passed。

---

## 一、当前问题复述

### 1. Step14 集成验证被放错阶段

当前实际代码状态：

- `pipeline/step_13_scene_assembly/`：场景组装
- `pipeline/step_14_integration_validation/`：集成验证
- `pipeline/step_15_build_package/`：构建打包
- `pipeline/step_16_delta_patch/`：差量补丁
- `pipeline/step_17_migration_audit/`：迁移审计

但 `core/ui/pipeline_panel.py` 中的分组是：

```python
("执行阶段", range(11, 14))  # 11,12,13
("验证阶段", range(14, 18))  # 14,15,16,17
```

这会造成概念错误：

- Step13 场景组装属于执行阶段，当前是对的。
- Step14 集成验证不是独立“验证阶段”的起点，而是执行阶段的收口检查。
- Step14 验证的是 Step11 程序执行、Step12 美术生产、Step13 场景组装能否形成可运行 Demo，因此它仍然属于开发执行闭环。

结论：Step14 应该进入开发流水线的“执行阶段”，执行阶段应为 Step11-14。

---

### 2. 开发流水线内部“验证阶段”与顶部“验证阶段”重复

当前 UI 同时存在两个验证入口：

1. 开发流水线内部的“验证阶段”：Step14-17。
2. 与“开发流水线”平级的顶部导航“验证阶段”：`core/ui/validation_panel.py`，当前按钮是 Step13、Step14、Step17。

这两个入口职责重叠，并且顶部验证阶段的 Step13/14/17 组合本身也是错误设计：

- Step13 是场景组装，不应该作为独立验证入口。
- Step14 是执行阶段的集成验证，不应该被顶部验证阶段重复触发。
- Step17 当前是迁移审计/项目审查类能力，不应该混入验证阶段。

结论：取消顶部“验证阶段”的现有设计，不再保留 Step13/14/17 的独立验证面板。

---

### 3. Step15-17 不应继续挂在开发流水线内部

当前 Step15-17 的职责分别是：

- Step15：Build Package，构建打包。
- Step16：Delta Patch，差量补丁。
- Step17：Migration Audit，迁移审计。

这些并不属于“开发流水线”的核心开发执行闭环：

- Step15 是交付打包能力，应放到与开发流水线平级的“打包阶段”。
- Step16 依赖版本差异与历史版本概念；当前暂不考虑版本历史，因此不应继续暴露。
- Step17 是审计类能力；当前暂不考虑项目审查，因此不应继续暴露。

结论：

- 开发流水线的主流程终点调整为 Step14。
- 顶部原“验证阶段”改名为“打包阶段”。
- 打包阶段先承接 Step15 的构建打包能力。
- Step16 差量补丁因依赖版本历史，本轮取消。
- Step17 迁移审计因属于项目审查，本轮取消。

---

### 4. 版本历史功能暂不考虑，需要清理

当前版本历史相关入口和代码包括：

- 顶部导航：`版本历史`
- UI：`core/ui/version_panel.py`
- 核心模块：`core/version/`
- 相关调用：
  - `core/main.py::run_iterate()`
  - `core/save/manager.py` 中的 version manifest/index 维护
  - `pipeline/step_17_migration_audit/plugin.py` 成功后调用 `complete_current_version()`
- 测试：
  - `core/tests/unit/test_version_manager.py`
  - `core/tests/unit/test_iteration_development.py` 中部分版本断言
  - `core/tests/unit/test_ui_panels_import.py` 中 VersionPanel 断言

结论：当前版本历史功能从 UI、主流程、保存同步、Step17 完成逻辑中移除。后续如重新设计版本功能，需要作为独立需求重新立项，不保留半成品入口。

---

### 5. 项目审查功能暂不考虑，需要清理

当前项目审查相关入口和代码包括：

- 顶部导航：`项目审查`
- UI：`core/ui/audit_panel.py`
- 工具：`tools/audit/`
- 流水线阶段：`pipeline/step_17_migration_audit/`
- 相关生成物：`project_audit_report.json`、`project_audit_report.md`、`PROJECT_BIBLE.md`
- 测试：
  - `core/tests/unit/test_project_audit_tools.py`
  - `core/tests/unit/test_ui_panels_import.py` 中 AuditPanel 断言

结论：项目审查入口、工具链、Step17 审计阶段从当前版本中取消。后续如需要审查/项目圣经能力，应作为独立功能重新设计，不与打包阶段混合。

---

## 二、目标结构

### 1. 顶部导航目标

保留：

- 设计工作台
- 开发流水线
- 补充开发
- 打包阶段
- 运行日志
- SDK 知识库

移除：

- 验证阶段
- 版本历史
- 项目审查

说明：

- 原顶部“验证阶段”改为“打包阶段”。
- 打包阶段不是验证阶段改名后继续跑 Step13/14/17，而是重新实现为独立打包入口。
- 打包阶段不直接操作开发流水线 Step13/14。

---

### 2. 开发流水线目标分组

开发流水线只保留 Step00-14：

| 分组 | 步骤范围 | 职责 |
|---|---:|---|
| 设计阶段 | Step00-06 | 从创意到程序/美术需求评审 |
| 风格确认 | Step07 | 美术风格生成与确认 |
| 计划阶段 | Step08-10 | 程序计划、美术计划、资产契约对齐 |
| 执行阶段 | Step11-14 | 程序执行、美术生产、场景组装、集成验证 |

删除开发流水线内部的“验证阶段”分组。

Step14 在 UI 中显示为“集成验证”，但归属执行阶段。

---

### 3. 打包阶段目标职责

新增或重构 `core/ui/package_panel.py`，替代 `validation_panel.py`。

打包阶段只处理已经完成开发执行闭环后的交付动作：

1. 读取 Step14 集成验证结果。
2. 检查 DemoScene、Build Settings、运行时资产挂载、音频占位、关键 manifest 是否满足打包准入。
3. 生成当前项目的一次性打包输出：
   - package manifest
   - build report
   - package notes
   - package validation report
4. 输出到 active draft/save 的明确路径，例如：
   - `outputs/package/current/package_manifest.json`
   - `outputs/package/current/build_report.json`
   - `outputs/package/current/package_validation_report.json`
   - `outputs/package/current/PACKAGE_NOTES.md`

打包阶段不负责：

- 不跑 Step13 场景组装。
- 不跑 Step14 集成验证。
- 不生成版本历史。
- 不生成差量补丁。
- 不生成项目审查报告。
- 不生成 PROJECT_BIBLE。

---

## 三、阶段编号与功能调整

### 1. 开发流水线最终编号

| Step | 保留/调整 | 新归属 | 说明 |
|---:|---|---|---|
| 00-10 | 保留 | 设计/计划 | 不在本轮重构重点 |
| 11 | 保留 | 执行阶段 | 程序开发执行 |
| 12 | 保留 | 执行阶段 | 美术生产执行 |
| 13 | 保留 | 执行阶段 | 场景组装 |
| 14 | 保留并移动分组 | 执行阶段 | 集成验证，开发执行闭环的最终门禁 |
| 15 | 从流水线移出 | 打包阶段 | 构建打包能力迁移为独立 package service |
| 16 | 取消 | 暂不暴露 | 差量补丁依赖版本历史，本轮清理 |
| 17 | 取消 | 暂不暴露 | 迁移审计/项目审查，本轮清理 |

---

### 2. 需要删除或迁移的流水线插件

保留：

- `pipeline/step_13_scene_assembly/`
- `pipeline/step_14_integration_validation/`

迁移：

- `pipeline/step_15_build_package/`
  - 不再作为 `StagePlugin` 参与开发流水线。
  - 将其可用逻辑迁移到 `core/packaging/` 或 `core/package/` 服务层。
  - UI 由 `PackagePanel` 调用服务层，不通过 `PluginManager.load_stage("15")`。

取消：

- `pipeline/step_16_delta_patch/`
- `pipeline/step_17_migration_audit/`

删除后必须同步清理：

- `pipeline/_registry.json`
- `core/registry.py`
- `pipeline/artifact_layer/registry.json`
- `pipeline/README.md`
- `core/artifact/reviewer.py` / `core/artifact/validator.py` 中针对 final step 的特殊 `migration_audit.json` 判断
- 相关测试

---

## 四、代码改造计划

### Phase 1：UI 导航重构

目标：先修正用户可见结构。

修改文件：

- `core/ui/main_window.py`
- `core/ui/pipeline_panel.py`
- 新增 `core/ui/package_panel.py`
- 删除或废弃 `core/ui/validation_panel.py`
- 删除 `core/ui/version_panel.py`
- 删除 `core/ui/audit_panel.py`

具体任务：

1. 顶部导航移除：
   - `验证阶段`
   - `版本历史`
   - `项目审查`
2. 顶部导航新增：
   - `打包阶段`
3. `_show_validation()`、`_get_validation_panel()` 改为 `_show_package()`、`_get_package_panel()`。
4. 删除 `_get_version_panel()`、`_show_versions()`。
5. 删除 `_get_audit_panel()`、`_show_audit()`。
6. `PipelinePanel._GROUPS` 改为：

```python
_GROUPS = [
    ("设计阶段", range(0, 7)),
    ("风格确认", range(7, 8)),
    ("计划阶段", range(8, 11)),
    ("执行阶段", range(11, 15)),
]
```

7. `_CN_TITLES` 保留 0-14，移除 15-17 或不再展示 15-17。

验收：

- 顶部不再出现“验证阶段/版本历史/项目审查”。
- 顶部出现“打包阶段”。
- 开发流水线没有“验证阶段”分组。
- Step14 出现在“执行阶段”。

---

### Phase 2：开发流水线注册表收口到 Step14

目标：让开发流水线运行范围与 UI 一致。

修改文件：

- `core/registry.py`
- `pipeline/_registry.json`
- `pipeline/README.md`
- `core/main.py`

具体任务：

1. `STEP_SPECS` 移除 15-17。
2. `max_step_number()` 变为 14。
3. `pipeline/_registry.json` 移除 stage 15-17 插件注册。
4. `pipeline/README.md` 改为 Step00-14。
5. `core/main.py` 默认 `--stop-step` 应自然落到 14。
6. 移除 `run_validate()` 中只支持 13/14/17 的 standalone validation 设计。
7. 如果仍需 CLI 打包，新增独立命令，例如：

```bash
python -m core.main package
```

或后续单独实现；本轮不再保留 `validate --step 13/14/17`。

验收：

- `python gui_app.py` 的开发流水线最多显示 Step14。
- `python -m core.main --list` 不再列出 Step15-17。
- `python -m core.main --from-step 0 --stop-step 14` 是完整开发流水线。
- 直接请求 Step15-17 应明确失败为 unknown step，不允许静默运行旧逻辑。

---

### Phase 3：打包阶段服务化

目标：把原 Step15 的可用能力迁移为独立打包服务，而不是继续作为流水线 stage。

新增建议结构：

```text
core/
  packaging/
    __init__.py
    service.py
    manifest.py
    validation.py
```

职责：

- `service.py`
  - `run_package(project_root: Path, mode: str = "current") -> dict`
  - 聚合 Step14 输出，生成打包产物。
- `manifest.py`
  - 定义 package manifest 的字段规范。
  - 负责 JSON 读写与路径转换。
- `validation.py`
  - 检查 Step14 是否成功。
  - 检查场景、构建配置、资源挂载、音频占位、关键报告是否存在。
  - 生成 `package_validation_report.json`。

`PackagePanel` UI：

- 显示 Step14 当前状态。
- 提供“生成打包资料”按钮。
- 提供“刷新状态”按钮。
- 输出 JSON/Markdown 报告路径。
- 不提供 Step13/Step14/Step17 运行按钮。

输出建议：

```text
outputs/package/current/
  package_manifest.json
  build_report.json
  package_validation_report.json
  PACKAGE_NOTES.md
```

验收：

- Step14 未成功时，打包按钮禁用或返回 blocked。
- Step14 成功后，可生成完整打包资料。
- 打包阶段输出不写入 `outputs/artifacts/stage_15`。
- 打包阶段不修改 Step00-14 的 pipeline state。

---

### Phase 4：取消版本历史功能

目标：移除当前版本历史入口与主动维护逻辑，避免半成品版本概念继续污染打包/验证链路。

删除或改造：

- 删除 `core/ui/version_panel.py`
- 删除 `core/version/`
- 移除 `core/main.py::run_iterate()` 对 `core.version.manager` 的依赖
- 移除 `core/save/manager.py` 中主动创建/维护 `version_manifest.json`、`version_index.json` 的逻辑
- 移除 `pipeline/step_17_migration_audit/plugin.py` 中 `complete_current_version()` 逻辑，随后 Step17 整体删除
- 移除或重写以下测试：
  - `core/tests/unit/test_version_manager.py`
  - `core/tests/unit/test_iteration_development.py` 中版本历史断言
  - `core/tests/unit/test_ui_panels_import.py` 中 VersionPanel 断言

注意：

- 如果补充开发/迭代开发仍需要“变更说明”，只能保留无版本号的当前工作区变更计划。
- 不再生成 `version_manifest.json` 作为打包/审计准入条件。
- 不再支持 rollback UI。

验收：

- 代码搜索 `VersionPanel` 无引用。
- 代码搜索 `rollback_to_version` 无引用。
- 代码搜索 `version_index.json` 无主动写入。
- 创建/保存项目不再自动写版本历史文件。

---

### Phase 5：取消项目审查功能

目标：移除项目审查面板、审查工具和 Step17 审计阶段。

删除或改造：

- 删除 `core/ui/audit_panel.py`
- 删除 `tools/audit/`
- 删除 `pipeline/step_17_migration_audit/`
- 删除 `core/source/importer.py` 中仅服务 Step17 的 `run_audit_step()` / migration audit finalize 逻辑
- 删除 `core/stage.py` 中针对 `migration_audit` 文件名的特殊处理
- 删除 `core/artifact/reviewer.py`、`core/artifact/validator.py` 中 final step 使用 `migration_audit.json` 的分支
- 移除 `pipeline/artifact_layer/registry.json` 中 `stage_17.migration_audit_bundle`
- 移除测试：
  - `core/tests/unit/test_project_audit_tools.py`
  - `core/tests/unit/test_ui_panels_import.py` 中 AuditPanel 断言

验收：

- 顶部不再有“项目审查”。
- 代码搜索 `AuditPanel` 无引用。
- 代码搜索 `PROJECT_BIBLE` 无当前功能引用。
- 代码搜索 `run_audit_step` 无运行路径引用。

---

### Phase 6：取消 Delta Patch 当前功能

目标：因为版本历史暂不考虑，差量补丁不应继续作为独立功能暴露。

删除或改造：

- 删除 `pipeline/step_16_delta_patch/`
- 移除 `pipeline/artifact_layer/registry.json` 中 `stage_16.delta_patch_bundle`
- 移除 `core/engines/delta_patch.py`，除非其他仍保留的功能明确引用
- 移除 `tools/patch/` 中仅服务差量补丁历史包的工具
- 修改 `core/ui/patch_panel.py`：如果它是“补充开发”功能，不应依赖版本历史或 Step16；如果依赖，则本轮同步降级为普通补充开发入口。
- 更新测试：
  - `core/tests/unit/test_patch_channel.py`
  - `core/tests/unit/test_iteration_cli.py`
  - `core/tests/unit/test_iteration_development.py`

验收：

- 开发流水线无 Step16。
- 打包阶段不显示“差量补丁”。
- 代码中不存在通过版本历史生成 delta patch 的用户入口。

---

### Phase 7：制品注册表与验证层同步

目标：清理 artifact layer 中过期 stage，避免注册表仍认为 15-17 是流水线一部分。

修改文件：

- `pipeline/artifact_layer/registry.json`
- `core/artifact/graph.py`
- `core/artifact/preflight.py`
- `core/artifact/reviewer.py`
- `core/artifact/validator.py`
- `core/source/importer.py`

具体任务：

1. 移除 stage 15-17 artifact bundle。
2. 将 final pipeline stage 改为 14。
3. Stage14 成功即代表开发流水线完成。
4. 打包阶段输出不走 `stage_NN` artifact registry，而走 `core/packaging` 独立 manifest。
5. 所有 “final step = migration_audit” 的特殊处理改为普通 artifact index 验证。

验收：

- artifact graph 最终节点是 `stage_14.integration_validation_bundle`。
- `preflight_stage_contract(15)` 不再存在。
- `run_artifact_validators(14)` 能正常作为开发流水线最终校验。
- 不再要求 `migration_audit.json`。

---

### Phase 8：测试更新

必须新增或更新测试：

1. UI 导航测试
   - `MainWindow` 不再包含验证阶段、版本历史、项目审查按钮。
   - `MainWindow` 包含打包阶段按钮。

2. PipelinePanel 分组测试
   - Step14 属于执行阶段。
   - 不存在“验证阶段”分组。
   - Step15-17 不展示。

3. Registry 测试
   - `max_step_number() == 14`
   - `get_step(14).slug == "integration_validation"`
   - `get_step(15)` 抛出 unknown step。

4. PackagePanel 测试
   - Step14 未成功时打包 blocked。
   - Step14 成功时生成 `outputs/package/current/*`。
   - 打包不修改 pipeline state。

5. 删除/改写旧测试
   - 删除 validation panel 的 Step13/14/17 独立运行测试。
   - 删除 VersionPanel 测试。
   - 删除 AuditPanel 测试。
   - 删除 Step17 version completion 测试。
   - 删除 project audit tool 测试。

建议验证命令：

```bash
python -B -m compileall core pipeline tools
python -B -m pytest core/tests/unit/test_ui_panels_import.py -q
python -B -m pytest core/tests/unit/test_validation_cli.py -q
python -B -m pytest core/tests/unit/test_draft_archive_paths.py -q
python -B -m pytest core/tests/unit core/tests/integration -q
```

其中 `test_validation_cli.py` 需要改名或改造成 package CLI 测试；如果取消 validate CLI，则删除该测试。

---

## 五、最终用户流程

调整后的用户操作流程应该是：

1. 在“设计工作台”完成设计。
2. 导出到开发流水线。
3. 在“开发流水线”运行 Step00-14。
4. Step14 成功后，项目进入“可打包”状态。
5. 打开“打包阶段”。
6. 点击生成打包资料。
7. 查看 package manifest、build report、package validation report。

用户不再需要：

- 进入单独“验证阶段”跑 Step13/14/17。
- 进入“版本历史”。
- 进入“项目审查”。
- 理解 Step15-17 作为流水线尾部阶段。

---

## 六、验收标准

### UI 验收

- [ ] 顶部导航只有：设计工作台、开发流水线、补充开发、打包阶段、运行日志、SDK 知识库。
- [ ] 开发流水线内没有“验证阶段”分组。
- [ ] Step14 显示在“执行阶段”。
- [ ] 打包阶段不显示 Step13/14/17 按钮。
- [ ] 不存在版本历史入口。
- [ ] 不存在项目审查入口。

### 流水线验收

- [ ] Step00-14 可以作为完整开发流水线运行。
- [ ] Step14 成功后，开发流水线即完成。
- [ ] Step15-17 不在 `STEP_SPECS`。
- [ ] `pipeline/_registry.json` 不注册 Step15-17。
- [ ] artifact graph 不依赖 stage 15-17。

### 打包验收

- [ ] Step14 未成功时不能打包。
- [ ] Step14 成功后可以生成打包资料。
- [ ] 打包资料输出到 `outputs/package/current/`。
- [ ] 打包不创建 `outputs/artifacts/stage_15/`。
- [ ] 打包不创建版本历史。
- [ ] 打包不创建项目审查报告。

### 清理验收

- [ ] `VersionPanel` 无引用。
- [ ] `AuditPanel` 无引用。
- [ ] `ValidationPanel` 无引用或已被 `PackagePanel` 完全替代。
- [ ] `core/version/` 无当前运行路径依赖。
- [ ] `tools/audit/` 无当前运行路径依赖。
- [ ] `pipeline/step_16_delta_patch/` 与 `pipeline/step_17_migration_audit/` 已删除或不再注册。

---

## 七、实施顺序建议

推荐按以下顺序开发，避免一次性大拆造成难追踪问题：

1. UI 导航与 PipelinePanel 分组调整。
2. Registry 收口到 Step14。
3. 新建 PackagePanel 与 `core/packaging` 服务。
4. 迁移 Step15 的打包能力到 `core/packaging`。
5. 移除 ValidationPanel。
6. 移除 VersionPanel 与版本历史运行路径。
7. 移除 AuditPanel 与项目审查运行路径。
8. 移除 Step16/17 注册、插件、artifact registry。
9. 更新测试。
10. 全量 compile/test。

---

## 八、风险与处理

### 风险 1：版本历史与补充开发存在耦合

当前 `core/main.py::run_iterate()`、`core/iteration/*`、`core/save/manager.py` 与 `core/version/manager.py` 有耦合。

处理方式：

- 本轮先取消用户可见版本历史。
- 如果补充开发必须保留，改成“当前工作区补充开发计划”，不再要求版本号、parent_version、rollback。
- 删除或重写所有 `version_manifest` 作为准入条件的逻辑。

### 风险 2：Step17 同时承担版本完成与审计完成

当前 Step17 成功后会调用 `complete_current_version()`，这把审计、版本完成、流水线完成混在一起。

处理方式：

- 删除 Step17 作为流水线阶段。
- 开发流水线完成态由 Step14 决定。
- 打包完成态由 `package_manifest.json` 和 `package_validation_report.json` 决定。

### 风险 3：原 ValidationPanel 可能被误认为必要的安全门

当前 ValidationPanel 是 standalone validation，它没有完整替代流水线门禁。

处理方式：

- 安全门保留在 Step14 内部。
- 打包阶段只读取 Step14 结果，不重新定义 Step13/14/17 的独立验证。

---

## 九、本轮明确不做

- 不做版本历史。
- 不做 rollback。
- 不做 delta patch。
- 不做项目审查。
- 不做 PROJECT_BIBLE。
- 不做独立 Step13/14/17 验证面板。
- 不把打包阶段重新塞回开发流水线。

---

## 十、完成后的结构判断

完成后，系统应该变成：

```text
设计工作台
  -> 产出设计输入

开发流水线
  -> Step00-14
  -> 最终完成：集成验证通过

打包阶段
  -> 基于 Step14 成功结果生成当前项目打包资料

补充开发
  -> 后续单独按非版本历史方式重构

运行日志 / SDK 知识库
  -> 保持辅助工具定位
```

这套结构中，每个阶段职责独立：

- 开发流水线负责把设计变成可运行 Demo。
- Step14 负责确认 Demo 集成闭环。
- 打包阶段负责把已集成 Demo 整理成可交付包。
- 版本历史和项目审查不参与当前主流程。
