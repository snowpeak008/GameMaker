# Phase 1: Contracts and Storage

状态：完成。

## ATOM-010 Contract Skeleton

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 建立 `adm-new-contracts` 模块结构和通用 Result/Report DTO。 |
| 依赖 | ATOM-002 |
| 输入设计文档 | `newrust_design/03_data_contracts_design.md`, `newrust_design/05_tauri_commands_and_view_models.md` |
| 涉及 crate/file | `crates/adm-new-contracts/src/lib.rs`, `project.rs`, `save.rs`, `pipeline.rs`, `artifact.rs`, `ai.rs`, `package.rs`, `patch.rs`, `sdk.rs`, `log.rs`, `view.rs` |
| Rust service/command | 无 |
| 数据契约 | command response/error DTO |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-contracts` |
| 完成定义 | serde roundtrip baseline tests 通过。 |
| 禁止事项 | 不做 IO；不做业务决策。 |

完成记录：

- 建立并导出 contracts 模块骨架：`project`, `save`, `execution_object`, `pipeline`, `artifact`, `ai`, `package`, `patch`, `sdk`, `log`, `view`, `response`。
- 新增通用 command response/error/diagnostic/evidence DTO，并补充 JSON roundtrip baseline tests。
- 为 foundation 中跨契约使用的 `EvidenceLevel`, `GateStatus`, `FileManifestEntry` 增加 serde 支持。
- 为 contracts 当前公开状态枚举增加稳定 JSON 名称，保证后续 Tauri command 和 Web UI view model 有明确边界。
- 验收命令：
  - `cargo test -p adm-new-contracts`
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace --quiet`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```

## ATOM-011 Project State Contracts

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 `ProjectState`、`NodeState`、`GameplaySystemsState`、`AiInterviewState` typed models。 |
| 依赖 | ATOM-010 |
| 输入设计文档 | `python_deconstruction/11_design_engine_contracts.md`, `16_ai_interview_and_completion_contracts.md` |
| 涉及 crate/file | `crates/adm-new-contracts/src/project.rs`, `ai.rs` |
| Rust service/command | 无 |
| 数据契约 | project_state schema-compatible model |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-contracts project_state` |
| 完成定义 | empty/default/serde roundtrip/invalid enum tests 通过。 |
| 禁止事项 | 不省略 `optionProvenance`、L4/L5 相关字段。 |

完成记录：

- 在 `contracts::project` 中实现 `ProjectState`, `NodeState`, `GameplaySystemsState` typed models。
- 在 `contracts::ai` 中实现 `AiInterviewState`, `AiRouteOverview`, `ConversationSummaryV1`, `FrameworkMemoryState` typed models。
- 保留 Python 事实源中的 `optionProvenance` 三层结构，并用 `OptionProvenanceEntry.extra` 保留未归一化元数据。
- 为 `designEntities`, `messages`, `inferences`, `optionDifferences`, L4/L5/quality 预留 JSON typed extension，避免把半结构化 Python 内容压扁成字符串。
- 用 serde default 模拟 Python `normalize_state()` 缺字段补默认值；非法 `DecisionState` enum 仍被拒绝。
- 验收命令：
  - `cargo test -p adm-new-contracts project_state`
  - `cargo test -p adm-new-contracts`
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace --quiet`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```

## ATOM-012 Save and Execution Object Contracts

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 save index、manifest、draft meta、lock、snapshot、execution object models。 |
| 依赖 | ATOM-010 |
| 输入设计文档 | `python_deconstruction/13_save_and_execution_object_contracts.md` |
| 涉及 crate/file | `crates/adm-new-contracts/src/save.rs`, `execution_object.rs` |
| Rust service/command | 无 |
| 数据契约 | save/execution-object |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-contracts save` |
| 完成定义 | required fields and state enums match Python docs。 |
| 禁止事项 | 不弱化 lock/session fields。 |

完成记录：

- 在 `contracts::save` 中实现 `SaveIndex`, `SaveIndexEntry`, `SaveManifest`, `DraftMeta`, `ArchiveLock`, `FileMap`, `SnapshotManifest`, `FileMapDelta`, `TimelineEntry`。
- `AutosaveState` 明确建模为 `ProjectState`，对应 Python `autosave_state.json` 直接写 project_state JSON 的事实。
- `DraftMeta` 保留 `session_id`, `pid`, `project_root`, `draft_root`, `linked_save_id`, `linked_archive_path`, `workspace_state`, `origin_deleted_save_id`。
- `ArchiveLock` 保留 `.archive_lock` 的 `pid`, `session_id`, `acquired_at`，并兼容 owner 查询时追加的 `live`, `lock_path`。
- 在 `contracts::execution_object` 中扩展完整 Python workflow 状态：`draft`, `stale_draft`, `submitted`, `analyzing`, `awaiting_confirmation`, `approved`, `conflict_blocked`, `stale_before_execution`, `executing`, `cancellation_requested`, `execution_failed`, `verified`, `rejected`, `cancelled`, `superseded`。
- 实现 `ExecutionObjectStoreDocument`, `ExecutionObject`, `SubmissionSnapshot`, `StateHistoryRecord`, `OwnershipMigration`，并保留审计/检查/verification JSON 内容字段。
- 验收命令：
  - `cargo test -p adm-new-contracts save`
  - `cargo test -p adm-new-contracts`
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace --quiet`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```

## ATOM-013 Pipeline and Artifact Contracts

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 stage spec、stage result、artifact registry、validation report models。 |
| 依赖 | ATOM-010 |
| 输入设计文档 | `python_deconstruction/07_pipeline_step_contracts.md`, `09_artifact_validation_flow.md`, `15_artifact_schema_refs_map.md` |
| 涉及 crate/file | `crates/adm-new-contracts/src/pipeline.rs`, `artifact.rs` |
| Rust service/command | 无 |
| 数据契约 | StageResult, ArtifactRegistry |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-contracts pipeline`; `cargo test -p adm-new-contracts artifact` |
| 完成定义 | status enum and validator names fully represented。 |
| 禁止事项 | 不硬编码 Step order 为 UI 常量。 |

完成记录：

- 在 `contracts::pipeline` 中实现 `StageContextModel`, `PipelineStageResult`, `StageMetadata`, `SourceGroupSpec`。
- `StageStatus` 保留 Python 全部状态：`success`, `failed`, `skipped`, `blocked`, `stopped`, `waiting_confirmation`, `completed_with_review`。
- 在 `contracts::artifact` 中实现 `ArtifactRegistry`, `ArtifactContract`, `ArtifactTask`, `SchemaRef`, `ArtifactLayerManifest`, `PreflightReport`, `ArtifactReviewReport`, `ArtifactValidationLayerReport`, `DependencyGraph`。
- 保留 reviewer 白名单 4 项与 validator 白名单 7 项，并提供未知 reviewer/validator 和重复 task id 检测。
- 使用 `depends_on` 与 `DependencyGraph.topological_order` 表达执行拓扑，不把 Step00-14 写死为 UI 顺序。
- 验收命令：
  - `cargo test -p adm-new-contracts pipeline`
  - `cargo test -p adm-new-contracts artifact`
  - `cargo test -p adm-new-contracts`
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace --quiet`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```

## ATOM-014 AI, Patch, SDK, Log, Package Contracts

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 AI schema mode、patch manifest、SDK spec/index、log entry、package report/manifest models。 |
| 依赖 | ATOM-010 |
| 输入设计文档 | `16_ai_interview_and_completion_contracts.md`, `17_packaging_contracts.md`, `20_parity_gate_test_matrix.md` |
| 涉及 crate/file | `crates/adm-new-contracts/src/ai.rs`, `patch.rs`, `sdk.rs`, `log.rs`, `package.rs` |
| Rust service/command | 无 |
| 数据契约 | AI/Patch/SDK/Log/Package |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-contracts ai package patch sdk log` |
| 完成定义 | all persisted DTOs roundtrip and reject invalid status/mode。 |
| 禁止事项 | 不合并 interview schema service 与 generic completion service。 |

完成记录：

- 在 `contracts::ai` 中补齐 `AiResponseMode`, `AiResponsePayload`, `PartialProjectOutput`, `CodexRunResult`, `CompletionJsonResult`。
- 保持 AI 访谈 schema payload 与通用 `CompletionJsonResult` 分离，避免把 patch/SDK completion 混入访谈状态机。
- 在 `contracts::patch` 中实现 `PatchTask`, `PatchRecord`，覆盖 `patch_manifest.json` 字段。
- 在 `contracts::sdk` 中实现 `SdkIndex`, `SdkIndexEntry`, `SdkSpec`，并保留 `draft/pending_review/approved/rejected` 审核状态。
- 在 `contracts::log` 中实现 `LogEntry`，匹配 JSONL 的 timestamp/level/context/message/source/metadata。
- 在 `contracts::package` 中实现 `PackageValidationReport`, `PackageBuildReport`, `PackageManifest`, `PackageBlockingIssue`, `PackageRequiredCheck`, `PackageManifestOutputs`。
- 保留 9 个 `REQUIRED_INTEGRATION_CHECKS`，并以测试固定 `changed_files` 为空时必须 blocked。
- 验收命令：
  - `cargo test -p adm-new-contracts ai`
  - `cargo test -p adm-new-contracts package`
  - `cargo test -p adm-new-contracts patch`
  - `cargo test -p adm-new-contracts sdk`
  - `cargo test -p adm-new-contracts log`
  - `cargo test -p adm-new-contracts`
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace --quiet`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```

## ATOM-020 Storage Repositories

状态：完成。

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 project root resolver、typed JSON repository、atomic write、file manifest、path safety。 |
| 依赖 | ATOM-010 至 ATOM-014 |
| 输入设计文档 | `newrust_design/03_data_contracts_design.md` |
| 涉及 crate/file | `crates/adm-new-storage/src/*` |
| Rust service/command | repository layer |
| 数据契约 | all typed persisted models |
| UI 影响 | 无 |
| 验收命令 | `cargo test -p adm-new-storage` |
| 完成定义 | temp root tests 覆盖 read/missing/invalid/atomic write。 |
| 禁止事项 | 不读取真实用户 `drafts/`/`saves/` 作为测试依赖。 |

完成记录：

- 在 `adm-new-storage` 中实现 `ProjectRoot`，统一 project root canonicalize 与 relative path safety。
- 实现 generic `JsonRepository<T>`，支持 missing read、required read、typed serde read/write、atomic JSON write、file manifest hash。
- 实现 `SaveIndexRepository`, `SaveArchiveRepository`, `DraftWorkspaceRepository`, `SnapshotRepository`, `ArchiveLockRepository`。
- `ArchiveLockRepository::try_create()` 使用 create-new 语义，避免覆盖已有 `.archive_lock`。
- 所有测试使用临时 project root，不读取真实 `drafts/`/`saves/`。
- 验收命令：
  - `cargo test -p adm-new-storage`
  - `cargo fmt --check`
  - `cargo check --workspace`
  - `cargo test --workspace --quiet`
  - `cargo run -p adm-new-cli -- plan-gate`
```
result=passed
```
