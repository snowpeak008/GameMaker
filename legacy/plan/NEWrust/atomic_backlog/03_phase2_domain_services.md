# Phase 2: Domain and Application Services

状态：完成。

## ATOM-030 Design Engine Service

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 empty/normalize/effective state/coverage/L4/L5/quality/export view model。 |
| 依赖 | ATOM-011, ATOM-020 |
| 输入设计文档 | `python_deconstruction/11_design_engine_contracts.md`, `19_ui_reproduction_specs.md` |
| 涉及 crate/file | `crates/adm-new-design`, `crates/adm-new-application` |
| Rust service/command | `DesignWorkbenchService` |
| 数据契约 | ProjectState, DesignWorkbenchView |
| UI 影响 | 后续 Design UI |
| 验收命令 | `cargo test -p adm-new-design`; `cargo test -p adm-new-application design` |
| 完成定义 | coverage/quality/node palette view model 与 Python 文档一致。 |
| 禁止事项 | 不在 Web UI 重算业务状态。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-design` 实现 `DesignEngineService`，覆盖 empty state、normalize、effective state、coverage、L4/L5 进度、quality metrics、node palette view model。
- 2026-07-08：在 `adm-new-application` 增加 `DesignWorkbenchService` 应用层包装，Web/Tauri 后续只能调用 service，不在 UI 重算业务状态。
- 验收通过：`cargo test -p adm-new-design`、`cargo test -p adm-new-application design`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-031 Save Service

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 autosave、save index、formal archive sync、lock、load/delete/rename。 |
| 依赖 | ATOM-012, ATOM-020 |
| 输入设计文档 | `python_deconstruction/13_save_and_execution_object_contracts.md` |
| 涉及 crate/file | `crates/adm-new-save`, `crates/adm-new-application` |
| Rust service/command | `SaveService` |
| 数据契约 | SaveIndex, SaveManifest, DraftMeta |
| UI 影响 | save dialogs later |
| 验收命令 | `cargo test -p adm-new-save`; `cargo test -p adm-new-application save` |
| 完成定义 | lock conflict/stale lock/deleted current save/temp root sync tests 通过。 |
| 禁止事项 | 不允许无锁覆盖 formal archive。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-save` 实现 `SaveService`，覆盖 autosave、create/sync/load/delete/rename、formal archive workspace、draft meta、save index、archive lock、file map、snapshot 和 timeline。
- 2026-07-08：在 `adm-new-application` 增加 `SaveApplicationService`，Web/Tauri 后续只通过 application service 发起保存操作。
- 验收通过：`cargo test -p adm-new-save`、`cargo test -p adm-new-application save`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-032 AI Config and Structured Completion

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 AI config v3 typed load/save/validate 和 generic structured completion service skeleton。 |
| 依赖 | ATOM-014, ATOM-020 |
| 输入设计文档 | `python_deconstruction/14_ai_config_adapter_log_contracts.md`, `16_ai_interview_and_completion_contracts.md` |
| 涉及 crate/file | `crates/adm-new-ai`, `crates/adm-new-application` |
| Rust service/command | `AiConfigService`, `StructuredCompletionService` |
| 数据契约 | AIConfig, APIEntry, CompletionJsonResult |
| UI 影响 | AI config dialog later |
| 验收命令 | `cargo test -p adm-new-ai ai_config`; `cargo test -p adm-new-ai completion`; `cargo test -p adm-new-application ai_config` |
| 完成定义 | dev/image/completion categories and unsupported config errors tested。 |
| 禁止事项 | 不把 API key 打进 logs/evidence。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-contracts::ai` 增加 `AiConfig`、`ApiCategory`、`ApiEntry`、`AiProfile`、`ModelTask`、`ModelResult` 等配置和 adapter 契约。
- 2026-07-08：在 `adm-new-ai` 实现 `AiConfigService`、配置 v3 load/save/validate、active completion adapter spec、`StructuredCompletionService` JSON 抽取/重试骨架和 Codex output path guard。
- 2026-07-08：在 `adm-new-application` 增加 `AiConfigApplicationService` 应用层委托；API key 只以 `has_api_key` 暴露给 adapter spec，不写入 evidence 输出。
- 验收通过：`cargo test -p adm-new-ai ai_config`、`cargo test -p adm-new-ai completion`、`cargo test -p adm-new-application ai_config`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-033 AI Interview Service

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 interview state machine、schema-mode validation、高置信写回、archive、memory event skeleton。 |
| 依赖 | ATOM-030, ATOM-032 |
| 输入设计文档 | `python_deconstruction/16_ai_interview_and_completion_contracts.md` |
| 涉及 crate/file | `crates/adm-new-ai`, `crates/adm-new-design`, `crates/adm-new-application` |
| Rust service/command | `AiInterviewService` |
| 数据契约 | AiInterviewState, AiPayload, ProjectState |
| UI 影响 | AI panel later |
| 验收命令 | `cargo test -p adm-new-ai interview`; `cargo test -p adm-new-application ai_interview` |
| 完成定义 | invalid JSON/schema mismatch/low confidence/high confidence/partial merge tests 通过。 |
| 禁止事项 | 不让 embedded/standalone 行为分叉。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-ai` 实现 `AiInterviewService`，覆盖 schema-mode validation、payload JSON 解析、aiInterview 状态更新、自动 archive path、framework memory event skeleton。
- 2026-07-08：实现 full project output 高置信写回、低置信隔离、partial project output domain 校验和分片合并，不允许 Web UI 自行合并状态。
- 2026-07-08：在 `adm-new-application` 增加 `AiInterviewApplicationService` 应用层委托，统一内嵌/独立访谈后端入口。
- 验收通过：`cargo test -p adm-new-ai interview`、`cargo test -p adm-new-application ai_interview`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-034 Pipeline and Runtime Service

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 registry load、dependency order、run state、stop signal、stage status transitions。 |
| 依赖 | ATOM-013, ATOM-020 |
| 输入设计文档 | `python_deconstruction/07_pipeline_step_contracts.md`, `newrust_design/07_pipeline_artifact_engine_design.md` |
| 涉及 crate/file | `crates/adm-new-pipeline`, `crates/adm-new-application` |
| Rust service/command | `PipelineService` |
| 数据契约 | StageSpec, StageResult, PipelineState |
| UI 影响 | Pipeline view later |
| 验收命令 | `cargo test -p adm-new-pipeline`; `cargo test -p adm-new-application pipeline` |
| 完成定义 | topological order, from/to range, stop, waiting_confirmation tested。 |
| 禁止事项 | 不绕过 registry 直接跑硬编码列表。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-contracts::pipeline` 增加 `StageKind`、`StageSpec`、`PipelineRegistry`、`PipelineRunState`、`PipelineStageRuntime` 等 runtime 契约。
- 2026-07-08：在 `adm-new-pipeline` 实现 `PipelineService`，覆盖 registry validation、topological order、run_range、stop request、dependency block、waiting_confirmation、failed/blocked/stopped 状态收敛。
- 2026-07-08：在 `adm-new-application` 增加 `PipelineApplicationService`，Tauri/Web 后续只调用 application service。
- 验收通过：`cargo test -p adm-new-pipeline`、`cargo test -p adm-new-application pipeline`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-035 Artifact Service

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 artifact registry、preflight、reviewer/validator skeleton、schema_refs validation。 |
| 依赖 | ATOM-013, ATOM-034 |
| 输入设计文档 | `python_deconstruction/09_artifact_validation_flow.md`, `15_artifact_schema_refs_map.md` |
| 涉及 crate/file | `crates/adm-new-artifact`, `crates/adm-new-application` |
| Rust service/command | `ArtifactService` |
| 数据契约 | ArtifactRegistry, ValidationReport |
| UI 影响 | Pipeline diagnostics later |
| 验收命令 | `cargo test -p adm-new-artifact` |
| 完成定义 | missing schema_refs/unknown reviewer/upstream failed all blocked。 |
| 禁止事项 | 不把 validator warning 当 success。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-artifact` 实现 `ArtifactService`，覆盖 registry validation、dependency graph、topological artifact order、stage preflight、review pipeline、artifact validators。
- 2026-07-08：实现 schema_refs/knowledge_refs/path evidence、unknown reviewer/validator、duplicate task、upstream validation status、review report status、manifest validation 等后端门禁。
- 2026-07-08：明确 warning 不计为 success；在 `adm-new-application` 增加 `ArtifactApplicationService` 应用层委托。
- 验收通过：`cargo test -p adm-new-artifact`、`cargo test -p adm-new-application artifact`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-036 Packaging Service

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 package validation、build report、validation report、notes、manifest。 |
| 依赖 | ATOM-014, ATOM-020, ATOM-034 |
| 输入设计文档 | `python_deconstruction/17_packaging_contracts.md` |
| 涉及 crate/file | `crates/adm-new-packaging`, `crates/adm-new-application` |
| Rust service/command | `PackagingService` |
| 数据契约 | PackageValidationReport, PackageManifest |
| UI 影响 | Package panel later |
| 验收命令 | `cargo test -p adm-new-packaging`; `cargo test -p adm-new-application package` |
| 完成定义 | Step14 success+changed_files success；missing changed_files blocked；missing unity blocked。 |
| 禁止事项 | 不信任 UI readiness。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-packaging` 实现 `PackagingService`，从 Step14 packaging sources 生成 typed validation report、build report、package manifest 和 PACKAGE_NOTES。
- 2026-07-08：后端重复验证 Step14 status、9 个 REQUIRED_INTEGRATION_CHECKS、actual_changed_files 非空、Unity validation valid，不依赖 UI 按钮状态。
- 2026-07-08：在 `adm-new-application` 增加 `PackagingApplicationService` 应用层委托。
- 验收通过：`cargo test -p adm-new-packaging`、`cargo test -p adm-new-application package`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-037 Patch, SDK, and Log Services

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 PatchStore/Analyzer shell、SdkKnowledgeBase、RunLogService。 |
| 依赖 | ATOM-014, ATOM-020, ATOM-032 |
| 输入设计文档 | `python_deconstruction/20_parity_gate_test_matrix.md` |
| 涉及 crate/file | `crates/adm-new-patch`, `crates/adm-new-sdk`, `crates/adm-new-application` |
| Rust service/command | `PatchService`, `SdkKnowledgeService`, `RunLogService` |
| 数据契约 | PatchRecord, SdkSpec, LogEntry |
| UI 影响 | Patch/SDK/Log panels later |
| 验收命令 | `cargo test -p adm-new-patch`; `cargo test -p adm-new-sdk`; `cargo test -p adm-new-application logs`; `cargo test -p adm-new-application sdk`; `cargo test -p adm-new-application patch` |
| 完成定义 | list/write/status/filter/approved context tests 通过。 |
| 禁止事项 | AI SDK extraction result cannot auto-approve。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-patch` 实现 `PatchService`，覆盖 empty request validation、analyze shell、write/list、status update/filter、approved context。
- 2026-07-08：在 `adm-new-sdk` 实现 `SdkKnowledgeService`，覆盖 placeholder、AI extracted spec pending review、review status update、approved context；AI 提取结果不能自动 approve。
- 2026-07-08：在 `adm-new-application` 增加 `PatchApplicationService`、`SdkKnowledgeApplicationService`、`RunLogService`，覆盖 latest/filter/clear/export JSONL。
- 验收通过：`cargo test -p adm-new-patch`、`cargo test -p adm-new-sdk`、`cargo test -p adm-new-application logs`、`cargo test -p adm-new-application sdk`、`cargo test -p adm-new-application patch`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。
