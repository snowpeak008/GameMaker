# Phase 0: Workspace and Governance

状态：开发中。

## ATOM-000 Plan Gate Alignment

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 让现有 `adm-new-governance` 能识别当前评分规则：单项 >=90，综合 >=95，无 hard gate。 |
| 依赖 | Python 解构评分通过；NEWrust 设计评分通过；原子计划评分通过。 |
| 输入设计文档 | `newrust_design/09_testing_gates_release_design.md`, `python_deconstruction/scorecard.md`, `newrust_design/scorecard.md`, `atomic_backlog/scorecard.md` |
| 涉及 crate/file | `crates/adm-new-governance`, `apps/adm-new-cli` |
| Rust service/command | governance gate only |
| 数据契约 | scorecard parsing model |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-governance`; `cargo run -p adm-new-cli -- plan-gate` |
| 完成定义 | gate 正确读取三个阶段 scorecard，并拒绝未通过状态。 |
| 禁止事项 | 不把每项 >95 当成规则；不忽略综合分。 |

完成记录：

- 修改 `NEWrust/crates/adm-new-governance/src/lib.rs`，由旧静态 final scores 改为读取三个阶段 scorecard。
- 修改 `NEWrust/apps/adm-new-cli/src/main.rs` 的 help 文案。
- 修正 `newrust_design/scorecard.md` 和 `atomic_backlog/scorecard.md` 的一位小数综合分，使其与权重公式一致。
- 验收命令：
  - `cargo fmt --check`
  - `cargo test -p adm-new-governance`
  - `cargo test -p adm-new-cli`
  - `cargo run -p adm-new-cli -- plan-gate`
  - `cargo check --workspace`
```
result=passed
```

## ATOM-001 Workspace Expansion

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 在 `NEWrust/Cargo.toml` 加入后续 crate 空壳，保持 `unsafe_code=forbid`。 |
| 依赖 | ATOM-000 |
| 输入设计文档 | `newrust_design/02_workspace_and_crate_design.md` |
| 涉及 crate/file | `Cargo.toml`, `crates/adm-new-storage`, `adm-new-design`, `adm-new-save`, `adm-new-ai`, `adm-new-pipeline`, `adm-new-artifact`, `adm-new-packaging`, `adm-new-patch`, `adm-new-sdk`, `adm-new-application`, `adm-new-tauri-commands` |
| Rust service/command | 无 |
| 数据契约 | workspace package/lint policy |
| UI 影响 | 无 |
| 验收命令 | `cargo fmt --check`; `cargo check --workspace` |
| 完成定义 | workspace 全部 crate 能 check。 |
| 禁止事项 | 不引入 UI/Tauri 业务代码。 |

完成记录：

- 扩展 `NEWrust/Cargo.toml` workspace members。
- 新增空壳 crate：
  - `adm-new-storage`
  - `adm-new-design`
  - `adm-new-save`
  - `adm-new-ai`
  - `adm-new-pipeline`
  - `adm-new-artifact`
  - `adm-new-packaging`
  - `adm-new-patch`
  - `adm-new-sdk`
  - `adm-new-application`
  - `adm-new-tauri-commands`
- 每个新增 crate 仅包含最小 smoke test，不实现业务逻辑。
- 验收命令：
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```

## ATOM-002 Foundation Utilities

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 完成 error、time、ids、stable hash、path safety、atomic write 基础 API。 |
| 依赖 | ATOM-001 |
| 输入设计文档 | `newrust_design/02_workspace_and_crate_design.md`, `newrust_design/04_application_services_design.md` |
| 涉及 crate/file | `crates/adm-new-foundation/src/*` |
| Rust service/command | foundation helpers |
| 数据契约 | `AppError`, `EvidenceRef`, `GateReport` |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-foundation` |
| 完成定义 | path traversal 被拒绝；atomic write 测试通过；stable hash 稳定。 |
| 禁止事项 | foundation 不依赖业务 crate。 |

完成记录：

- 扩展 `NEWrust/crates/adm-new-foundation/src/lib.rs`。
- 新增/强化：
  - `unix_timestamp_millis()`
  - `sanitize_identifier()`
  - `new_stable_id()`
  - `ensure_child_path()`
  - `FileManifestEntry`
  - `file_manifest_entry()`
  - `write_text_atomic()`
- 保持 foundation 无业务 crate 依赖。
- 验收命令：
  - `cargo test -p adm-new-foundation`
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```
