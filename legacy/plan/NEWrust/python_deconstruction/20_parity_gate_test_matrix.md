# Parity and Gate Test Matrix

状态：第一轮 QA/Parity 优化完成。

目标：把 Python 功能复刻要求转成 NEWrust 可执行验收，不让“文档写过”冒充“功能等价”。

## 1. 面板级功能矩阵

| 面板 | Python 入口 | 数据/服务 | NEWrust 验收 |
| --- | --- | --- | --- |
| MainWindow | `core/ui/main_window.py` | geometry, AI status, pipeline state, save lock | 6 个 task tabs 可切换；状态栏 2s 刷新；关闭时 running pipeline 阻断并请求 stop；未保存设计触发确认。 |
| Design Workbench | `core/ui/app_window.py` | `DesignEngine`, project_state, autosave, export, save manager | 三栏布局；16 领域；profile combobox；node/checklist/L4/L5 编辑；summary/gap/risk/validation tabs；autosave flush。 |
| Embedded AI | `core/ui/embedded_interview.py` | `aiInterview`, Codex schema backend, framework memory, archive | 用户输入、Ctrl+Enter、force output、mark inaccurate、archive；turn/readiness/full/partial/mapping/summary 状态闭环。 |
| Pipeline | `core/ui/pipeline_panel.py` | `run_range`, runtime control, artifact validators, Step07 confirmation | from/to range、skip manual gates、run/stop、log streaming、Step07 style grid/approval/regenerate。 |
| Patch | `core/ui/patch_panel.py` | `PatchAnalyzer`, `PatchStore`, `CompletionJsonService` | empty request validation；async analyze；patch_manifest 写入；Treeview 按 updated_at 倒序刷新。 |
| Package | `core/ui/package_panel.py` | `run_package`, Step14 state, package validation | Step14 未成功禁用；成功后可生成 4 个 package files；blocked issues 正确显示。 |
| Logs | `core/ui/log_panel.py` | `RUN_LOGS_DIR/*.jsonl`, `LogEntry` | latest 5 run logs 加载；level filter；clear；export_jsonl。 |
| SDK | `core/ui/sdk_panel.py` | `SdkKnowledgeBase`, optional CompletionJsonService extraction | add placeholder；approve/pending/reject；_index/spec 写入；approved context 渲染。 |
| AI Config | `core/ui/ai_config_unified_dialog.py` | `settings/ai_config.json`, validator, active profile | dev/image/completion 三分类；active entry 切换；local CLI/API/file config 条件字段；save/apply reload status。 |

## 2. 数据合同验收矩阵

| 合同 | Python 文件 | 必测字段/行为 |
| --- | --- | --- |
| project_state | `core/design/engine.py` | projectName/profile/nodes/gameplaySystems/aiInterview；normalize 后字段齐全。 |
| node state | `core/design/engine.py` | decisionState/designNote/riskNote/notApplicableReason/designEntities/checklist/checklistOptions/optionProvenance。 |
| save index | `core/save/manager.py` | schema_version/current_save_id/saves/updated_at；按 last_worked_at 排序。 |
| save manifest | `core/save/manager.py` | save_id/display_name/save_type/progress/last_transaction_seq/timestamps。 |
| execution object | `core/engines/execution_objects/*` | draft/submitted/approved/verified 状态机和 evidence。 |
| source package | `core/design/export_adapter.py` | package_manifest/operator_submission/human_approval/stage_input/attachments。 |
| structured handoff | `core/design/structured_handoff.py` | decisions/profile/archetype/traceability/playable contracts/handoff_manifest。 |
| artifact registry | `pipeline/artifact_layer/registry.json` | stage/kind/tasks/reviewers/validators/schema_refs/knowledge_refs/depends_on。 |
| artifact validation | `core/artifact/validator.py` | stage_files/review_report/manifest/schema/knowledge/dependency validators。 |
| AI schema | `core/design/ai_schema.py` | turn/readiness/full_output/partial_output/mapping/summary modes。 |
| AI archive | `core/ui/embedded_interview.py` | archiveVersion/archiveType/archiveId/session/messages/summary/runtimeRefs。 |
| patch manifest | `core/patch/record.py` | patch_id/request/status/tasks/changed_files/validation_summary/errors。 |
| SDK index/spec | `core/sdk/knowledge_base.py` | `_index.json`, `spec.json`, valid review statuses, approved context。 |
| log entry | `core/ui/log_entry.py` | timestamp/level/context/message/source/metadata JSONL。 |
| package manifest | `core/packaging/manifest.py` | schema_version/status/development_path/changed_files/source_stage/outputs。 |

## 3. Gate 映射

| Gate | Python 证据 | NEWrust gate |
| --- | --- | --- |
| source authority | `01_source_authority_index.md`, `18_garbage_isolation_draft.md` | no feature accepted without authoritative/reference classification。 |
| UI parity | `19_ui_reproduction_specs.md` | Playwright screenshots + DOM assertions for every task tab。 |
| data parity | `04_data_model_and_storage.md`, `13_*`, `15_*`, `20_*` | typed serialization roundtrip tests for every contract。 |
| pipeline parity | `07_pipeline_step_contracts.md`, `09_artifact_validation_flow.md` | stage order/dependency/preflight/review/validator tests。 |
| save parity | `13_save_and_execution_object_contracts.md` | lock/load/delete/sync/snapshot tests with temp roots。 |
| AI parity | `14_ai_config_adapter_log_contracts.md`, `16_*` | schema-mode validation, high-confidence writeback, archive, memory event tests。 |
| package parity | `17_packaging_contracts.md` | Step14 success + missing changed files + missing Unity summary cases。 |
| release readiness | Python gate docs + NEWrust gates | build/test/lint/package artifact manifest checks。 |

## 4. Anti-Fake Completion Rules

- A UI panel is not complete unless its backing data write/read path is tested.
- A data model is not complete unless it has serde roundtrip and invalid-input tests.
- A pipeline stage is not complete unless preflight, dependency, review, validator, state update, and stop handling are tested.
- AI integration is not complete unless invalid JSON, schema mismatch, low-confidence output, high-confidence writeback, and backend unavailable paths are tested.
- Package output is not complete unless blocked cases still write validation evidence.
- Screenshots alone do not prove behavior; behavior tests alone do not prove pixel parity.

## 5. Required NEWrust Test Skeleton

```text
crates/*:
  unit tests for domain services and contracts

apps/desktop-tauri:
  command tests for Tauri bridge input/output

web:
  component tests for view model rendering
  Playwright e2e for task tabs and workflows

gates:
  parity gate
  artifact gate
  package gate
  release gate
```

每个原子任务必须声明：

- Python evidence file。
- Rust target file。
- UI target file if any。
- Data contract touched。
- Test command。
- Acceptance artifact。
