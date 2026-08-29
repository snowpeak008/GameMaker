# NEWrust 详细设计评分卡

状态：第一轮评分通过。

进入条件：Python 解构评分合格。

合格规则：

- 单项 `>=90`
- 综合 `>=95`
- 无硬门禁失败
- `confidence != low`

## 第一轮评分项

| 角色 | 领域 | 分数 | 权重 | confidence | evidence | issues | required_action |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| Python Archaeologist | Python 证据引用完整性 | 96 | 10 | high | 每份设计均回指 python_deconstruction 文档，旧 RUST/垃圾隔离已保留 | 少量版本选择留到实现阶段 | 原子计划必须继续列 evidence |
| Product Parity Reviewer | 功能覆盖和 UI parity | 95 | 15 | high | 六任务区、AI config、save、pipeline、patch、package、logs、sdk 均有 service/UI 设计 | 具体 UI component props 待原子任务细化 | 可进入原子计划 |
| Data Contract Architect | Rust typed contract 设计 | 95 | 15 | high | contract families、schema version、project/save/artifact/AI/package 合同已列 | 部分字段级 serde struct 待开发任务展开 | 原子计划先做 contracts |
| UI Reproduction Reviewer | Web UI 复刻可执行性 | 95 | 15 | high | Web component map、desktop parity、Step07/Design/Pipeline/utility panels 已设计 | 实现阶段需截图校准 Tk/Web 差异 | Playwright gate 必须优先建立 |
| Rust Architecture Reviewer | crate/service/Tauri 边界 | 96 | 15 | high | crate 依赖方向、service list、command groups、禁止事项清晰 | 当前 workspace 需按计划扩展 | 原子计划按依赖顺序拆 |
| QA Release Reviewer | 测试/gate/release 可执行性 | 95 | 15 | high | test layers、parity tests、gate reports、release rules 已列 | 具体 npm/test 命令待 web 初始化 | 原子计划每项绑定 test command |
| Red Team Reviewer | 伪完成和范围漂移 | 95 | 15 | high | hard gates、risk register、anti-fake rules、UI-first 禁令明确 | 后续最大风险是跳过 contract-first | 每小阶段继续回读计划 |

第一轮加权综合分：`95.2`。

第一轮结论：合格。单项均 `>=90`，综合 `>=95`，无硬门禁失败，confidence 均非 low。NEWrust 详细设计阶段可进入原子开发计划阶段。

## 防偏移记录

- 2026-07-08 stage=newrust_design_round1_written; plan_reread=done; drift_detected=false; drift_action=none; next=newrust_design_score_round1.
- 2026-07-08 stage=newrust_design_score_round1; plan_reread=done; drift_detected=false; drift_action=none; next=atomic_backlog.
