# Python UI 互动矩阵

状态：草案。

| 页面 | 操作 | Python handler | 后端/数据影响 | 状态 |
| --- | --- | --- | --- | --- |
| MainWindow | 切换设计工作台 | `_show_design` | 懒加载 `CommercialDesignApp` | confirmed |
| MainWindow | 切换开发流水线 | `_show_pipeline` | 懒加载 `PipelinePanel`，调用 `refresh()` | confirmed |
| MainWindow | 关闭窗口 | `_on_close` | 请求停止 pipeline、flush autosave、检查存档状态 | partial |
| Design | 切换领域 | `change_domain` | `save_visible_notes()` 后切换 `current_domain_id` 并 `render()` | confirmed |
| Design | 搜索节点 | `on_search_key_release` -> `render_nodes_from_search` | debounce 后重绘节点列表 | confirmed |
| Design | 项目画像变化 | `on_profile_change` | 标记 state changed 并 `render()` | confirmed |
| Design | 玩法系统勾选 | `on_gameplay_system_toggle` | 更新 `project_state["gameplaySystems"]`、清理权重和 core loop | confirmed |
| Design | 新增自定义玩法系统 | `add_custom_gameplay_system` | `normalize_custom_system` 后写入 `gameplaySystems.custom/selected/weights/coreLoops` | confirmed |
| Design | 删除自定义玩法系统 | `delete_custom_gameplay_system` | 从 custom/selected/weights/coreLoops/interview parsed ids 删除 | confirmed |
| Design | 更新玩法权重 | `update_gameplay_weight` | 校验 0-100 数值后写入 `gameplaySystems.weights` | confirmed |
| Design | 更新核心循环 | `update_gameplay_core_loop` | 写入 `gameplaySystems.coreLoops` | confirmed |
| Design | 玩法兜底访谈保存 | `save_gameplay_interview_answers` | 写入 `gameplaySystems.interview.answers` | confirmed |
| Design | 玩法兜底访谈应用 | `apply_gameplay_interview_answers` | `parse_interview_answers_to_custom_systems` 后生成 custom systems | confirmed |
| Design | checklist 勾选 | `on_checklist_change` | `DesignEngine.set_checklist_item`，刷新节点/结果，autosave | confirmed |
| Design | L4 选项变化 | `on_option_group_option_change` | `DesignEngine.set_option_group_option`，刷新 option widgets/结果，autosave | confirmed |
| Design | L4 主选项变化 | `on_option_group_primary_change` | `DesignEngine.set_option_group_primary`，刷新 option widgets/结果，autosave | confirmed |
| Design | 风险标记 | `on_risk_toggle` | 写 `riskNote` 或清空，刷新节点状态和结果 | confirmed |
| Design | 不适用标记 | `on_not_applicable_change` | `DesignEngine.set_node_state`，清理 notApplicableReason 或刷新状态 | confirmed |
| Design | 节点文本更新 | `update_node_text` | 写 `project_state["nodes"][node_id][field]`，可能刷新 node state | confirmed |
| Design | L5 JSON 保存 | `update_node_design_entities` | JSON parse，`engine.normalize_node_design_entities`，写 `designEntities/entityValidationErrors` | confirmed |
| Design | L5 清空 | `clear_node_design_entities` | 清空 `designEntities/entityValidationErrors` 并刷新节点状态 | confirmed |
| Design | 导出 | `export_project` | `choose_export_options` 后 `write_export` 到用户选择目录，先校验 project-local path | confirmed |
| Design | 存档管理 | `save_project` | 打开 `SaveManagerDialog`，先 `save_visible_notes()` | confirmed |
| Design | 文件打开兜底 | `_open_project_from_file` | 读取 JSON，`engine.normalize_state`，可迁移到执行对象存储 | confirmed |
| Design | 重置 | `reset_project` | `engine.empty_state()`，重置 UI 状态并 render | confirmed |
| Pipeline | 单步运行 | `_run_single` | Step>=3 先 `run_actual_development_preflight`，再 `_exec_range(step, step)` | confirmed |
| Pipeline | 运行范围 | `_run_range` / `_exec_range` | background thread 调 `core.main.run_range(auto_approve=True, skip_all_gates=checkbox)` | confirmed |
| Pipeline | AI 配置 | `_open_ai_config` | 打开 `AIConfigUnifiedDialog`，保存后 refresh | confirmed |
| Pipeline | 风格确认检查 | `_check_and_show_confirmation_dialog` | 读取 run_state；Step07 style confirmation 可在右侧或弹窗处理 | confirmed |
| Pipeline | 导出到流水线 | `_export_to_pipeline` | `export_concept_package()`，写 `concept_export_record.json` 到 save workspace | confirmed |
| Pipeline | 停止 | `_stop` | `request_stop(PROJECT_ROOT)` | confirmed |
| Patch | 分析补充需求 | `PatchPanel.analyze` | `PatchAnalyzer.analyze()` 调 AI JSON contract，写 `PATCHES_DIR/<patch_id>/patch_manifest.json` | confirmed |
| Patch | 刷新列表 | `PatchPanel.refresh` | `PatchStore.list()` 读取 patch manifests | confirmed |
| Package | 打包刷新 | `PackagePanel.refresh` | `get_step_state(PROJECT_ROOT, 14)`，Step14 success 才允许打包 | confirmed |
| Package | 生成打包资料 | `PackagePanel.run_package` | `core.packaging.run_package()` 写 build report、validation report、notes、manifest | confirmed |
| SDK | 新增 SDK | `SdkPanel.add_sdk` | `SdkKnowledgeBase.add_placeholder` 写 SDK spec/index | confirmed |
| SDK | 审核状态更新 | `SdkPanel.update_selected` | `SdkKnowledgeBase.update_review_status` 写 spec/index | confirmed |
| SDK | 刷新 | `SdkPanel.refresh` | `read_index()`，显示 `approved_prompt_context()` | confirmed |

待补充：所有按钮、输入、选择、AI 访谈、Step07 风格确认。
