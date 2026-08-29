# NEWrust 详细设计总览

状态：第一轮设计完成，待评分。

进入条件：

- Python 解构评分合格。
- 单项 `>=90`。
- 综合 `>=95`。
- 无硬门禁失败。

固定技术路线：

```text
Tauri + Web UI + Rust 后端
```

## 设计文档顺序

1. `01_architecture_overview.md`
2. `02_workspace_and_crate_design.md`
3. `03_data_contracts_design.md`
4. `04_application_services_design.md`
5. `05_tauri_commands_and_view_models.md`
6. `06_web_ui_design.md`
7. `07_pipeline_artifact_engine_design.md`
8. `08_save_ai_package_runtime_design.md`
9. `09_testing_gates_release_design.md`
10. `10_risk_register.md`
11. `scorecard.md`

## 设计原则

- Python 解构文档是产品事实源。
- Web UI 只渲染 view model 和提交 user intent。
- Rust application services 是业务写入唯一入口。
- Tauri commands 是桥接层，不承载业务规则。
- 所有持久化写入必须生成 evidence 或 validation report。
- 旧 `RUST/` 只作为 reference，不作为实现续作。
