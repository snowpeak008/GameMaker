# NEWrust 存档系统差异审查与优化

日期：2026-07-10

## 1. 审查范围

本轮不是把 Python 存档代码逐行搬到 Rust，而是对照以下边界重做可用版：

- 界面必须能区分新建空白项目、另存副本、保存当前、加载、重命名、删除和打开目录。
- 草稿与正式存档必须分离，正式存档写入失败时不得留下新旧混合工作区。
- 切换存档必须明确处理当前草稿，不能暗中覆盖另一个存档。
- 多窗口不得共享可写草稿；同一正式存档不得被两个进程同时编辑。
- 损坏的索引或草稿不能阻止应用启动，损坏正式存档必须可见且可处置。
- Python 真实旧存档即使没有 Rust `autosave_state.json`，也应能从已验证的 `design_project` execution object 恢复设计状态。
- `runtime/`、停止请求和活跃运行上下文属于易失状态，不得随正式存档恢复。

## 2. UI 差异

| 项目 | Python 旧版 | 改造前 NEWrust | 优化后 NEWrust |
| --- | --- | --- | --- |
| 布局 | 980x520 单表格，名称/时间/阶段/ID | 基础表格，详情和异常状态不足 | 主列表 + 详情区；宽屏并排、窄屏纵向，无水平溢出 |
| 新建语义 | “新建存档”清空流水线并保留当前设计 | “新建”实际复制整个当前工作区 | 明确拆为“新建项目存档”和“另存为副本” |
| 保存语义 | 可“保存到选中存档”，会改变绑定并直接写目标 | 保存当前 | 只允许保存当前绑定存档；不提供危险的跨存档覆盖 |
| 加载 | 单次覆盖确认 | 直接加载，并暗中提交当前草稿 | 三选确认：保存当前并切换、放弃草稿并切换、取消 |
| 删除 | 有确认，但后端锁保护不足 | 无确认 | 明确显示名称的删除确认；后端同时检查存档锁 |
| 状态可见性 | 当前标记和阶段进度 | 当前标记、原始时间 | 当前/草稿状态、设计进度、Step00-14 进度、锁、完整性、事务号、文件数、大小、原因和路径 |
| 时间 | 本地可读时间 | `unix:<seconds>` 原样显示 | 按当前语言格式化为本地时间 |
| 错误 | messagebox | 列表错误可能被当作空数据 | 对话框内保留错误；busy 时禁用操作并阻止重复提交 |
| 可访问性 | Treeview 键盘行为 | 主要依赖鼠标 | 行可聚焦、方向键/Enter 选择，确认层具备明确焦点 |
| 语言 | 仅中文 | 中英文混用 | `zh-CN` 纯中文和 `en-US` 纯英文词条对称 |

## 3. 功能语义差异

### 3.1 保留并强化的 Python 设计

- 保留“每个运行会话编辑草稿，显式保存提交正式存档”的模型。
- 保留删除当前存档后继续保有当前内容的“已删除存档副本”状态，避免用户工作立即消失。
- 保留正式存档锁和死 PID 回收，并把锁检查扩展到加载、保存、重命名和删除。
- 保留 file map、SHA-256、timeline、full/delta snapshot 和每存档最近 5 次历史。
- 保留空白新建时清理生成产物的规则，但只删除 `source_artifacts/devflow_*`，不删除用户导入的其他源文件。

### 3.2 有意不复制的 Python 行为

- 不复制“保存到选中存档”。Python 实现没有先加载目标完整工作区，也没有把绑定迁移和全部文件写入放在同一事务中，可能把 A 的设计写入 B 的流水线产物。
- 不复制坏 JSON 静默返回空对象。该行为可能随后用空索引覆盖原始坏文件；Rust 会隔离坏文件并重建可审查索引。
- 不复制逐文件覆盖正式工作区。正式提交使用 staging/backup/swap，失败后回到完整旧提交。
- 不把活跃 `runtime/`、`outputs/runtime_control/`、停止请求或运行锁写入正式存档。
- 不把设计检查项进度冒充 Step00-14 流水线进度。

## 4. 数据结构差异

### 4.1 Python 旧版

```text
drafts/<session>/
  draft_meta.json
  source_artifacts/
  outputs/
  workspace/
  snapshots/<seq_event>/full|delta/
  draft_file_map.json
  timeline.jsonl

saves/
  save_index.json
  <save_id>/
    manifest.json                 # 兼容 save_manifest.json
    workspace/
```

设计状态通常不在独立 autosave 文件中；真实旧存档主要通过
`workspace/outputs/execution_objects/execution_objects.json` 内最新且已验证的
`design_project.user_content` 恢复。

### 4.2 优化后的 NEWrust

```text
drafts/<desktop_session>/
  draft_meta.json
  autosave_state.json
  draft_file_map.json
  timeline.jsonl
  snapshots/
    <save_id>_tx_<seq>/
      snapshot_manifest.json
      snapshot_file_map.json
      delta.json
      full/
  source_artifacts/
  outputs/
    pipeline_state.json          # 可持久状态
    runtime_control/             # 易失状态，不归档
  workspace/
  iteration_specs/
  patches/

drafts/.session_locks/
  <desktop_session>.lock         # 桌面会话草稿 OS 锁

drafts/.transactions/
  global.lock                    # 项目级事务串行锁
  <desktop_session>.lock         # 会话事务锁
  <session>_<role>_<tx>.json     # 完整 before-image journal
  <session>_<role>_<tx>.commit   # 已提交标记（短暂存在）
  cleanup_warnings.jsonl

saves/
  .locks/
    index.lock                   # 真正的 index OS 锁
    archive_<save_id>.lock       # 真正的 archive OS 锁
  .save_index_lock               # 仅供诊断的 index 锁元数据
  save_index.json
  <save_id>/
    .archive_lock                # 仅供诊断的 archive 锁元数据
    manifest.json
    workspace/
```

锁元数据文件释放后保留并写为 `live=false`；文件是否存在不再代表锁仍被占用。
真正的互斥权来自进程持有的 `fs2` OS 文件句柄。

`SaveIndex` 额外暴露当前草稿状态：

- `workspace_state`
- `draft_updated_at`
- `origin_deleted_save_id`
- `has_autosave`

`SaveIndexEntry` 额外暴露可审计状态：

- `last_transaction_seq`
- `locked_by_other`、`lock_owner_pid`、`lock_owner_session`
- `integrity_status`、`integrity_message`
- `workspace_file_count`、`workspace_bytes`

`SaveProgress` 保留旧字段以兼容既有 JSON，同时拆分为：

- `design_passed` / `design_total` / `design_label`
- `pipeline_passed` / `pipeline_total` / `pipeline_label`

`ProjectState`、`NodeState`、玩法状态和 AI 访谈嵌套状态保留未知 JSON 字段，避免读取后再次保存时丢失未来版本或第三方扩展数据。

## 5. 事务与恢复边界

1. 草稿设计状态和两份可恢复 pipeline state 使用覆盖式原子写入，不执行 Windows 先删后改名。
2. 每个外层存档变更先有界获取项目级 `global.lock`，持锁恢复所有可接管的 pending journal，再获取本会话事务锁和所需 archive/index 锁；竞争超时返回可恢复的锁错误，不无限卡住界面。
3. 正式提交把允许归档的工作区复制到同盘 staging 目录。staging 完整构建并校验后，旧目录改名为 backup，staging 改名为正式目录。
4. journal 在变更前记录 index、manifest、draft meta/file map、timeline、snapshot 和目录交换的完整 before-image。commit marker 之前的残留回滚，marker 之后的残留只完成清理。
5. 创建、空白新建、同步、加载、重命名和删除都进入该事务边界；删除先把 archive 改名为同盘 tombstone，失败或崩溃时可以恢复原目录。
6. 真正的 index/archive 排他锁位于 `saves/.locks/`；`.save_index_lock` 和 `.archive_lock` 仅是 owner/live/path 诊断元数据。
7. 启动和每次后续变更前都会检查 pending transaction。坏 index 会先隔离再从存档目录重建；坏目录仍作为 `corrupt` 条目出现，不能静默消失。
8. 坏草稿 autosave 会被隔离，运行时脱离坏正式存档并进入可继续操作的未保存草稿；原坏文件和正式存档不被覆盖。运行中断状态会归一为 stopped，并重写 volatile/durable 两份 pipeline state。
9. commit 后清理失败不会把已成功提交误报成失败；警告写入 `cleanup_warnings.jsonl`，可返回的警告同时进入 Tauri diagnostics、界面本地化状态和 Runtime Logs。
10. 退出时先提交当前状态，成功后才释放存档 OS 锁。提交失败时阻止退出、保留窗口和锁，允许用户修复后重试。
11. 自动草稿清理默认关闭（`pruneDraftsKeepCount = 0`）；在有用户可见的保留/恢复策略前不自动删除恢复数据。每个正式存档的事务快照仍保留最近 5 份。

## 6. 验收重点

- 新建项目存档只保留当前设计，不带入旧流水线、补丁、日志或生成源包。
- 另存为副本完整复制当前可持久工作区，并切换到新存档。
- 加载三种选择均正确；目标加载失败时当前内存状态和当前锁不变。
- 两个桌面进程获得不同 draft session；同一正式存档的第二个编辑者看到锁冲突。
- 在复制、交换、manifest、snapshot、timeline、index 各阶段失败后，重启只能看到完整旧提交或完整新提交。
- 真实 Python 存档可从 verified `design_project` EO 读取，原目录保持只读不被迁移写回。
- 列表能显示 locked、corrupt、空存档、已删除副本和正常存档。
- 中文和英文模式下，普通、确认、错误、busy、locked、corrupt 及窄屏状态均无混合系统文案。

## 7. 实现与验证结果

本轮上述边界已实现，专项代码审查未发现剩余 P0/P1 交付阻断。

- Rust 全工作区 `cargo fmt --check`、`cargo check --workspace --locked`、`cargo test --workspace --locked` 全部通过。
- 存档相关联合测试 194/194；其中 `adm-new-save` 35/35，覆盖全局锁竞争、完整 before-image 回滚、跨会话运行期接管、rename 故障/残留、delete 故障/tombstone 残留和真实 Windows 32/33 锁竞争。
- Python 旧版 `test_draft_archive_paths.py` 与 `test_parallel_runtime_isolation.py` 41/41 通过，作为差异审查基线。
- Web unit/e2e、2426 个中英文对称键、两种语言各 10 个界面纯度门禁、48 张宽窄屏截图/溢出门禁和 93 项 UI 基线全部通过。
- 便携版 156 个 design-data 文件与源目录逐文件路径、长度、SHA-256 一致。
- 第一次真实 Tauri 启动/关闭生成 356,552-byte autosave、draft meta 和 clean shutdown 日志；第二次启动保持相同 autosave SHA-256，无 corrupt 文件或 pending journal，并成功恢复窗口。

交付入口：`NEWrust/dist/AutoDesignMaker-NEWrust/Start-AutoDesignMaker.cmd`

交付 EXE：20,584,960 bytes，SHA-256
`1720b9eff3e8cbc81cce5a23e11181fad029c6edd441c9dc974cc08c43efbd75`。

剩余非阻断风险：当前没有面向断电/介质损坏的长期 `.bak` 轮换；损坏 transaction journal 采用 fail-closed，不会猜测性覆盖数据。后续应在独立备份/恢复 UI 中处理，而不是削弱当前事务边界。
