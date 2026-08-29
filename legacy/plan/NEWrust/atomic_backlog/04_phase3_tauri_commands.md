# Phase 3: Tauri Commands

状态：完成。

## ATOM-040 Tauri App Scaffold

| 字段 | 内容 |
| --- | --- |
| 目标 | 创建 `apps/desktop-tauri` 和最小 Tauri shell，连接 `web/` build 输出。 |
| 依赖 | ATOM-001 |
| 输入设计文档 | `newrust_design/01_architecture_overview.md`, `05_tauri_commands_and_view_models.md` |
| 涉及 crate/file | `apps/desktop-tauri`, `web/package.json` |
| Rust service/command | app bootstrap |
| 数据契约 | none |
| UI 影响 | 最小空 shell |
| 验收命令 | `cargo check --workspace`; `npm run build` |
| 完成定义 | Tauri app compiles with placeholder route。 |
| 禁止事项 | 不实现业务 UI。 |

开发状态：完成。

完成记录：

- 2026-07-08：创建 `apps/desktop-tauri`，加入 Cargo workspace，提供 app bootstrap、`tauri.conf.json` 和可编译 placeholder shell。
- 2026-07-08：创建 `web/` 零依赖 build scaffold，`npm run build` 生成 `web/dist` placeholder route，供 Tauri shell 连接。
- 2026-07-08：当前 lockfile 未含 Tauri crate；为避免离线引入未锁定外部依赖，本阶段落地 Tauri-ready scaffold，实际 runtime crate 在后续依赖锁定后接入。
- 验收通过：`npm run build`、`cargo check --workspace`、`cargo fmt --check`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-041 Command Result and Error Mapping

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现统一 command response/error DTO 和 Tauri handler helper。 |
| 依赖 | ATOM-010, ATOM-040 |
| 输入设计文档 | `newrust_design/05_tauri_commands_and_view_models.md` |
| 涉及 crate/file | `crates/adm-new-tauri-commands`, `apps/desktop-tauri/src` |
| Rust service/command | command adapter layer |
| 数据契约 | CommandResponse, CommandError |
| UI 影响 | frontend API client later |
| 验收命令 | `cargo test -p adm-new-tauri-commands` |
| 完成定义 | success/error serialization tests 通过。 |
| 禁止事项 | command handler 不直接写文件。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-tauri-commands` 中实现统一 command adapter helper，包含 `command_success`、`command_success_with`、`command_failure`、`command_failure_with_evidence`、`command_error_from_adm`、`map_command_result` 和 `handle_command`。
- 2026-07-08：固定 `AdmError` 到 `CommandError` 的基础映射：路径逃逸为 `PATH_GUARD_FAILED`，unknown/missing/not found 为 `NOT_FOUND`，invalid/cannot/must/required/empty 为 `VALIDATION_FAILED`，其余为 `COMMAND_FAILED`。
- 2026-07-08：补充 success/error JSON 序列化、evidence/diagnostics、recoverable、handler helper 映射测试；command helper 不写文件、不复制业务 service 逻辑。
- 验收通过：`cargo test -p adm-new-tauri-commands`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-042 Design and Save Commands

| 字段 | 内容 |
| --- | --- |
| 目标 | 暴露 design/save command group。 |
| 依赖 | ATOM-030, ATOM-031, ATOM-041 |
| 输入设计文档 | `newrust_design/05_tauri_commands_and_view_models.md` |
| 涉及 crate/file | `crates/adm-new-tauri-commands/src/design.rs`, `save.rs` |
| Rust service/command | `load_design_workbench`, `update_node`, `export_design`, `list_saves`, `save_project`, `load_save` |
| 数据契约 | DesignWorkbenchView, SaveView |
| UI 影响 | Design UI later |
| 验收命令 | `cargo test -p adm-new-tauri-commands design save` |
| 完成定义 | commands call service mocks and map errors。 |
| 禁止事项 | 不复制 DesignEngine 逻辑到 commands。 |

开发状态：完成。

完成记录：

- 2026-07-08：新增 `adm-new-tauri-commands/src/design.rs`，实现 `load_design_workbench`、`update_node`、`export_design` command adapter，包含 request/response DTO、`DesignCommandService` trait、真实 `DesignWorkbenchService` 实现和 mock service 测试。
- 2026-07-08：新增 `adm-new-tauri-commands/src/save.rs`，实现 `list_saves`、`create_save`、`save_project`、`load_save`、`rename_save`、`delete_save`、`get_autosave_state` command adapter，包含 request/response DTO、`SaveCommandService` trait、真实 `SaveApplicationService` 实现和 mock service 测试。
- 2026-07-08：在 `adm-new-application` 补齐 design facade：节点文本更新、checklist/option/primary 更新、design_entities 替换、markdown/text/json/prompt 导出；command 层只调用 facade，不复制 DesignEngine 规则。
- 2026-07-08：在 `adm-new-save` 补齐 `list_saves()`，在 `SaveApplicationService` 暴露 autosave/list/rename/delete index 返回，供 command 层调用；文件写入仍由 save service 负责。
- 2026-07-08：计划中的 `cargo test -p adm-new-tauri-commands design save` 双过滤在 Cargo 中拆分为 `design` 与 `save` 两条过滤命令执行。
- 验收通过：`cargo test -p adm-new-tauri-commands`、`cargo test -p adm-new-tauri-commands design`、`cargo test -p adm-new-tauri-commands save`、`cargo test -p adm-new-application design_workbench_service_delegates_state_and_view_model`、`cargo test -p adm-new-save save_service_autosaves_and_creates_formal_archive_snapshot`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-043 AI and Config Commands

| 字段 | 内容 |
| --- | --- |
| 目标 | 暴露 AI interview、AI config command group。 |
| 依赖 | ATOM-032, ATOM-033, ATOM-041 |
| 输入设计文档 | `newrust_design/05_tauri_commands_and_view_models.md` |
| 涉及 crate/file | `crates/adm-new-tauri-commands/src/ai.rs`, `config.rs` |
| Rust service/command | `submit_ai_turn`, `force_ai_output`, `mark_ai_inaccurate`, `load_ai_config`, `save_ai_config` |
| 数据契约 | AiInterviewState, AIConfig |
| UI 影响 | AI panel/config dialog later |
| 验收命令 | `cargo test -p adm-new-tauri-commands ai config` |
| 完成定义 | backend unavailable and validation errors serialized。 |
| 禁止事项 | 不在 command 里存 API key to logs。 |

开发状态：完成。

完成记录：

- 2026-07-08：新增 `adm-new-tauri-commands/src/ai.rs`，实现 `submit_ai_turn`、`force_ai_output`、`mark_ai_inaccurate` command adapter，包含 request/response DTO、`AiCommandService` trait、真实 `AiInterviewApplicationService` 实现和 mock service 测试。
- 2026-07-08：新增 `adm-new-tauri-commands/src/config.rs`，实现 `load_ai_config`、`save_ai_config`、`validate_ai_config`、`completion_adapter_spec` command adapter，包含无密钥 `CompletionAdapterSpecView`、validation report DTO、真实 `AiConfigApplicationService` 实现和 mock service 测试。
- 2026-07-08：在 `adm-new-application` 暴露 AI service report 类型，并补齐 `mark_inaccurate` facade；该 facade 只更新 `AiInterviewState`，不写文件、不记录 API key。
- 2026-07-08：扩展 command error mapping：backend unavailable 映射为 `BACKEND_UNAVAILABLE`，配置缺失映射为 `CONFIGURATION_REQUIRED`，schema mode 不允许 response mode 映射为 `VALIDATION_FAILED`。
- 2026-07-08：验证 `completion_adapter_spec` 只暴露 `has_api_key`，序列化结果不包含真实 `api_key`。
- 2026-07-08：计划中的 `cargo test -p adm-new-tauri-commands ai config` 双过滤在 Cargo 中拆分为 `ai` 与 `config` 两条过滤命令执行。
- 验收通过：`cargo test -p adm-new-tauri-commands`、`cargo test -p adm-new-tauri-commands ai`、`cargo test -p adm-new-tauri-commands config`、`cargo test -p adm-new-application ai_config_application_service_delegates_validation_save_and_adapter_spec`、`cargo test -p adm-new-application ai_interview_application_service_delegates_high_confidence_writeback`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-044 Pipeline and Package Commands

| 字段 | 内容 |
| --- | --- |
| 目标 | 暴露 pipeline/package command group。 |
| 依赖 | ATOM-034, ATOM-035, ATOM-036, ATOM-041 |
| 输入设计文档 | `newrust_design/05_tauri_commands_and_view_models.md` |
| 涉及 crate/file | `crates/adm-new-tauri-commands/src/pipeline.rs`, `package.rs` |
| Rust service/command | `load_pipeline_view`, `run_pipeline_range`, `stop_pipeline`, `confirm_style`, `package_current_project` |
| 数据契约 | PipelineView, PackageView |
| UI 影响 | Pipeline/package UI later |
| 验收命令 | `cargo test -p adm-new-tauri-commands pipeline package` |
| 完成定义 | blocked package and waiting_confirmation mapping tested。 |
| 禁止事项 | 不让 package command skip service validation。 |

开发状态：完成。

完成记录：

- 2026-07-08：新增 `adm-new-tauri-commands/src/pipeline.rs`，实现 `load_pipeline_view`、`run_pipeline_range`、`stop_pipeline`、`confirm_style` command adapter，包含 request/response DTO、`PipelineCommandService` trait、真实 `PipelineApplicationService` 实现和 mock service 测试。
- 2026-07-08：新增 `adm-new-tauri-commands/src/package.rs`，实现 `load_package_view`、`package_current_project` command adapter，包含 package view/result DTO、`PackageCommandService` trait、真实 `PackagingApplicationService` 实现和 mock service 测试。
- 2026-07-08：在 `adm-new-application` 补齐 `PipelineApplicationService::confirm_style` 和 `PackagingApplicationService::package_current_project_from_values`，command 层不复制 pipeline/package 规则。
- 2026-07-08：覆盖 `waiting_confirmation` 映射、`stop_requested` 状态、style confirmation 状态、unknown stage 错误映射、package blocked、required check failed 和 successful package view。
- 2026-07-08：计划中的 `cargo test -p adm-new-tauri-commands pipeline package` 双过滤在 Cargo 中拆分为 `pipeline` 与 `package` 两条过滤命令执行。
- 验收通过：`cargo test -p adm-new-tauri-commands`、`cargo test -p adm-new-tauri-commands pipeline`、`cargo test -p adm-new-tauri-commands package`、`cargo test -p adm-new-application pipeline_application_service_delegates_order_and_run_range`、`cargo test -p adm-new-application packaging_application_service_delegates_package_current_project`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-045 Patch, Logs, SDK Commands

| 字段 | 内容 |
| --- | --- |
| 目标 | 暴露 patch/log/sdk command group。 |
| 依赖 | ATOM-037, ATOM-041 |
| 输入设计文档 | `newrust_design/05_tauri_commands_and_view_models.md` |
| 涉及 crate/file | `crates/adm-new-tauri-commands/src/patch.rs`, `logs.rs`, `sdk.rs` |
| Rust service/command | `analyze_patch_request`, `list_patches`, `list_latest_logs`, `read_log_entries`, `list_sdks`, `add_sdk`, `update_sdk_review_status` |
| 数据契约 | PatchRecord, LogEntry, SdkSpec |
| UI 影响 | utility panels later |
| 验收命令 | `cargo test -p adm-new-tauri-commands patch logs sdk` |
| 完成定义 | validation and not-found cases mapped。 |
| 禁止事项 | 不 auto-approve SDK extraction。 |

开发状态：完成。

完成记录：

- 2026-07-08：新增 `adm-new-tauri-commands/src/patch.rs`，实现 `analyze_patch_request`、`list_patches`、`read_patch`、`update_patch_status` command adapter，包含 request DTO、`PatchCommandService` trait、真实 `PatchApplicationService` 实现和 mock service 测试。
- 2026-07-08：新增 `adm-new-tauri-commands/src/logs.rs`，实现 `list_latest_logs`、`read_log_entries`、`export_log_jsonl` command adapter，包含 filter/limit DTO、`LogsCommandService` trait、真实 `RunLogService` 实现和 mock service 测试。
- 2026-07-08：新增 `adm-new-tauri-commands/src/sdk.rs`，实现 `list_sdks`、`add_sdk`、`update_sdk_review_status`、`get_approved_sdk_context`、`extract_sdk_spec` command adapter，包含 request DTO、`SdkCommandService` trait、真实 `SdkKnowledgeApplicationService` 实现和 mock service 测试。
- 2026-07-08：在 `adm-new-patch` 补齐 `get()`，在 `adm-new-application` 暴露 patch read 与 SDK list/index facade；command 层不直接管理底层记录表。
- 2026-07-08：覆盖 patch empty validation、patch not-found、log latest/filter/export、SDK add/review/context、SDK AI extraction 强制 `PendingReview`、SDK validation/not-found。
- 2026-07-08：计划中的 `cargo test -p adm-new-tauri-commands patch logs sdk` 三过滤在 Cargo 中拆分为 `patch`、`logs`、`sdk` 三条过滤命令执行。
- 验收通过：`cargo test -p adm-new-tauri-commands`、`cargo test -p adm-new-tauri-commands patch`、`cargo test -p adm-new-tauri-commands logs`、`cargo test -p adm-new-tauri-commands sdk`、`cargo test -p adm-new-application patch_application_service_delegates_status_and_approved_context`、`cargo test -p adm-new-application logs_application_service_filters_latest_clears_and_exports_jsonl`、`cargo test -p adm-new-application sdk_application_service_keeps_ai_extraction_pending_until_approved`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。
