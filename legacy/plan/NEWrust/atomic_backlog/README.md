# NEWrust 原子开发计划总览

状态：第一轮原子计划完成，待评分。

进入条件：

- Python 解构评分合格。
- NEWrust 详细设计评分合格。

每个原子任务必须包含：

- task id
- 目标
- 依赖
- 输入设计文档
- 涉及 crate/file
- Tauri command 或 Rust service
- 数据契约
- UI 影响
- 验收命令
- 完成定义
- 禁止事项

## 原子任务文件

1. `01_phase0_workspace_governance.md`
2. `02_phase1_contracts_storage.md`
3. `03_phase2_domain_services.md`
4. `04_phase3_tauri_commands.md`
5. `05_phase4_web_ui.md`
6. `06_phase5_gates_release.md`
7. `scorecard.md`

## 开发顺序

```text
Phase 0 workspace/governance
  -> Phase 1 contracts/storage
    -> Phase 2 domain/application services
      -> Phase 3 Tauri commands
        -> Phase 4 Web UI
          -> Phase 5 gates/release
```

任何 UI 任务开始前，必须存在对应 service 和 command 任务。

每完成一个 phase，必须回读：

- `plan/NEWrust/README.md`
- `plan/NEWrust/atomic_backlog/scorecard.md`
