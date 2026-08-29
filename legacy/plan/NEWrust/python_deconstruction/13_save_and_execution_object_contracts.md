# Save 与 Execution Object 契约拆解

状态：第一轮确认。

## 核心原则

Python 当前存档系统有两层：

- current per-session draft：唯一运行时写入点。
- formal archive：`saves/<save_id>/`，只保留 `manifest.json` 和 `workspace/`。

NEWrust 不能把它简化为“项目 JSON 保存”。存档同时承担 workspace 快照、文件变更索引、运行输出归档、execution object 权属和多窗口锁。

## 路径和文件

```text
saves/
  save_index.json
  <save_id>/
    manifest.json
    .archive_lock
    workspace/

drafts/<session_id>/
  draft_meta.json
  autosave_state.json
  draft_file_map.json
  timeline.jsonl
  snapshots/
  source_artifacts/
  outputs/
  workspace/
  iteration_specs/
  patches/
```

## Save Index

`load_index()` / `save_index()` 管：

- `schema_version = 1`
- `current_save_id`
- `saves`
- `updated_at`

`saves` 由 `_save_entry()` 从 manifest 投影，按最后工作时间倒序。

## Manifest

`create_save()` / `create_blank_save()` 初始化：

- `schema_version = 1`
- `save_id`
- `display_name`
- `save_type`
- `created_by`
- `reason`
- `created_at`
- `last_worked_at`
- `last_transaction_seq`
- `progress`

`create_iteration_save()` 额外写：

- `change_type`
- `requested_version`
- `iteration_spec_path`

## Draft Meta

`_write_draft_meta()` 写：

- `session_id`
- `pid`
- `project_root`
- `draft_root`
- `linked_save_id`
- `linked_archive_path`
- `workspace_state`
- `origin_deleted_save_id`

`workspace_state` 用于 UI 关闭判断，尤其是 `unsaved_copy_of_deleted_save`。

## Sync

`_sync_save()` 流程：

1. 读取 formal manifest。
2. 必要时恢复缺失的上游 stage outputs。
3. 运行 legacy project id migration。
4. 读取上一轮 `draft_file_map.json`。
5. 计算下一 transaction seq。
6. `build_file_map()` 生成当前文件 map。
7. 计算 added / modified / removed。
8. 原子复制 active draft 到 formal `workspace/`。
9. 移除 formal runtime artifacts。
10. 写 draft snapshot full 和 delta。
11. 更新 manifest progress / last_worked_at / last_transaction_seq。
12. 写 `timeline.jsonl`。
13. 更新 save index 和 draft meta。
14. 保留最近 5 个当前 draft snapshots。

## Load / Delete / Lock

- `load_save()`：获取 `.archive_lock`，复制 formal workspace 到 active draft，恢复 current save，必要时 sync migration。
- `delete_save()`：删除 formal archive，更新 index；如果删除的是当前存档，active draft 变为 `unsaved_copy_of_deleted_save`。
- `release_current_lock()`：窗口关闭和进程退出都调用。
- `.archive_lock` 记录 `pid`、`session_id`、`acquired_at`，旧 pid 不存在时可被抢占。

## Design Project Execution Object

设计工作台不是只写 `autosave_state.json`。正式保存时：

```text
SaveManagerDialog -> save_design_project() -> ExecutionObjectStore
```

`save_design_project()` 会：

- force cancel 旧的 active `design_project` objects。
- 创建 draft object，`object_type = design_project`。
- 写 `user_content = project_state`。
- manual save 自动 submit。
- 自动 impact analysis。
- 自动 approve。
- 运行 drift / concurrency checks。
- start execution。
- verify，证据包含 `project_state_hash`。

`load_latest_design_project()` 只读取 verified `design_project` object，并按 `updated_at` 取最新。

## UI 交互

- `CommercialDesignApp._do_autosave()` 写 `autosave_state.json`。
- `MainWindow.on_close()` flush autosave，比较 state hash，决定是否提示保存。
- `SaveManagerDialog.on_new_save()` 新建空白 save 并保存 design_project。
- `SaveManagerDialog.on_save_to_selected()` 覆盖选中存档并保存 design_project。
- `SaveManagerDialog.on_load_selected()` 调用 `load_save()`，再加载 latest design_project。
- `SaveManagerDialog.on_delete_selected()` 永久删除 formal archive。

## NEWrust 设计约束

- Rust 后端需要 `SaveService`、`DraftService`、`ArchiveLockService`、`ExecutionObjectService`。
- Web UI 只发起 save/load/delete/rename/open-dir 命令。
- 存档文件 map 和 snapshot 是验收证据，不可省略。
- 多窗口 archive lock 必须进入 Tauri 后端，不应靠前端状态判断。
