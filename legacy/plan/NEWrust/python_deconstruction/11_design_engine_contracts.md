# DesignEngine 契约拆解

状态：第一轮确认。

## 入口证据

- UI 入口：`core/ui/app_window.py::CommercialDesignApp`
- 数据加载：`core.design.data_loader.load_project_data()`
- 业务引擎：`core.design.engine.DesignEngine`
- 用户导出：`core.design.exporter`
- 流水线交接：`core.design.export_adapter`

## 状态根

`project_state` 是设计工作台的唯一业务状态根：

```text
project_state
  projectName
  profile
  nodes
    <node_id>
      decisionState
      designNote
      riskNote
      notApplicableReason
      designEntities
      entityValidationErrors
      checklist
      checklistOptions
      optionProvenance
  gameplaySystems
  aiInterview
```

`empty_state()` 从全部 domain nodes 建立完整空状态。`normalize_state()` 是读取存档、模板、autosave 后必须执行的迁移和清洗入口。

## 设计决策行为

必须复刻的命令语义：

- `set_checklist_item()`：取消 checklist 时清空该 item 下所有 option selections 和 provenance。
- `set_option_group_option()`：校验 option 是否属于 group；single group 自动替换；选中 option 自动勾选 checklist；取消 option 同步清 provenance；primary 不在 selected 时清空。
- `set_option_group_primary()`：只允许 `allowPrimary=true` 的 group；如果缺 provenance 则补 user_selected provenance。
- `set_node_state()`：只接受合法 node state。
- `refresh_node_state()`：按 checklist 完成度和 designNote 推导 `not_started/selected/completed`，但不覆盖 `not_applicable`。
- `normalize_node_design_entities()`：要求数组，逐个实体用 `EntitySchemaRegistry` 验证，错误为 warning。

## 进度和质量计算

必须作为 Rust 后端 service 迁移：

- `effective_node_state()`
- `node_progress()`
- `item_l4_progress()`
- `node_l4_progress()`
- `domain_l4_progress()`
- `project_l4_progress()`
- `domain_coverage()`
- `project_coverage()`
- `concreteness_coverage()`
- `consistency_score()`
- `quality_violations()`
- `quality_metrics()`
- `design_completion_summary()`

## 冲突和审核

`active_option_conflicts()` 从 checklist item 的 `optionRelations` 中查找已选 option 之间的 `soft_conflict`。`active_domain_option_conflicts()` 汇总到 domain 维度。

`design_completion_summary()` 的关键 gate：

- P0/P1/P2 节点统计。
- not applicable 节点若要求理由则必须填写。
- contract target 节点必须有 `user_selected` 或 `user_confirmed_ai` 且 confirmed 的 structured selection。
- `ai_inferred` 或 `migration_inferred` provenance 必须进入 review_items，不能算完全确认。
- common required contract 缺失时 status 为 blocked。

## NEWrust 设计约束

- Web UI 只展示 state 和发送 command。
- Rust 后端保存和归一化 `project_state`。
- Rust 后端负责 L4/L5、质量、冲突、completion summary。
- Tauri command 不直接改文件，应调用 application service。
- 所有 option provenance 必须保留，不能简化成 bool selection。
