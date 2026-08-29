# Tauri Commands and View Models

状态：第一轮设计完成。

evidence=

- `newrust_design/04_application_services_design.md`
- `python_deconstruction/19_ui_reproduction_specs.md`
- `python_deconstruction/20_parity_gate_test_matrix.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=none

## 1. Command Rule

Tauri commands are adapters:

```text
deserialize request
call application service
serialize response
emit job events if needed
```

Forbidden:

- direct filesystem writes in command handlers。
- business validation only in TypeScript。
- command-level copies of pipeline/save/AI logic。

## 2. Command Groups

| group | commands |
| --- | --- |
| shell | `get_shell_state`, `set_window_geometry` |
| design | `load_design_workbench`, `update_profile`, `select_domain`, `update_checklist`, `update_node_text`, `update_design_entities`, `export_design`, `load_templates`, `save_template` |
| save | `list_saves`, `create_save`, `save_project`, `load_save`, `rename_save`, `delete_save`, `get_autosave_state` |
| ai | `submit_ai_turn`, `force_ai_output`, `mark_ai_inaccurate`, `save_interview_archive`, `get_ai_runtime_status` |
| pipeline | `load_pipeline_view`, `run_pipeline_range`, `run_pipeline_step`, `stop_pipeline`, `export_design_to_pipeline`, `confirm_style`, `regenerate_style` |
| patch | `analyze_patch_request`, `list_patches`, `read_patch` |
| package | `load_package_view`, `package_current_project` |
| logs | `list_latest_logs`, `read_log_entries`, `export_log_jsonl` |
| sdk | `list_sdks`, `add_sdk`, `update_sdk_review_status`, `get_approved_sdk_context`, `extract_sdk_spec` |
| config | `load_ai_config`, `save_ai_config`, `validate_ai_config`, `detect_cli` |

## 3. View Model Naming

All command responses:

```json
{
  "ok": true,
  "data": {},
  "evidence": [],
  "diagnostics": []
}
```

Error response:

```json
{
  "ok": false,
  "error": {
    "code": "PACKAGE_BLOCKED",
    "message": "...",
    "evidence": [],
    "recoverable": true
  }
}
```

## 4. Important View Models

`ShellState`:

- active_view
- ai_status
- pipeline_progress
- system_status

`DesignWorkbenchView`:

- project_name
- export_formats
- profile_fields
- domains
- current_domain
- nodes
- results_tabs
- ai_panel
- dirty_state

`PipelineView`:

- groups
- selected_step
- from_step
- to_step
- skip_manual_gates
- running
- runtime_log_tail

`PackageView`:

- step14_status
- can_package
- last_result
- blocking_issues

`LogView`:

- level_filter
- entries
- latest_sources

`SdkView`:

- rows
- approved_context
- status_message

## 5. Frontend State Rule

Frontend state may store:

- active route。
- form drafts。
- selected row ids。
- optimistic disabled/running flags while waiting for command。

Frontend state may not store as source of truth:

- project_state。
- save index。
- pipeline state。
- artifact validation result。
- AI high-confidence merge result。
