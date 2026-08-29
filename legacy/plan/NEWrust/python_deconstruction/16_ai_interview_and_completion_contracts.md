# AI Interview, Completion JSON, and UCOS Contracts

状态：第一轮闭环解构完成，迁移风险已标记。

证据文件：

- `core/ui/embedded_interview.py`
- `core/ui/ai_interview_window.py`
- `core/design/ai_interview.py`
- `core/design/ai_schema.py`
- `core/design/ai_backend.py`
- `core/design/ai_validator.py`
- `core/design/ai_mapping_agent.py`
- `core/design/ai_summary_agent.py`
- `core/design/framework_memory.py`
- `core/design/ai_ucos_bridge.py`
- `core/ai_design/completion_service.py`
- `core/adapters/completion_adapter.py`
- `core/patch/analyzer.py`
- `core/sdk/ai_extractor.py`

## 1. 权威边界

AI 访谈有两个 UI 外壳：

| UI | 权威性 | 说明 |
| --- | --- | --- |
| `EmbeddedInterviewPanel` | authoritative | 设计工作台内嵌入口，是主 UI 体验的一部分。 |
| `AIInterviewWindow` | authoritative/reference | 独立窗口实现更完整，包含 UCOS bridge 调用；迁移时要吸收其缺失能力。 |

通用 Completion JSON 服务与 AI 访谈后端不是同一层：

| 服务 | 调用方 | 契约 |
| --- | --- | --- |
| `CodexCliBackend.run_turn(schema_mode=...)` | AI 访谈、mapping、summary | 使用 `core/design/ai_schema.py` 的 schema mode 约束结构化输出。 |
| `CompletionJsonService.generate_json_contract()` | patch analyzer、SDK extractor | 使用 active completion adapter，解析普通 JSON 或 fenced JSON，失败后追加 retry hint。 |

## 2. aiInterview 状态模型

`empty_ai_interview_state()` 定义项目内 AI 状态，必须作为 NEWrust 后端持久模型复刻：

| 字段组 | 字段 | 迁移要求 |
| --- | --- | --- |
| session | `schemaVersion`, `codexSessionId`, `sessionTurnCount`, `status`, `activeTurnId` | 保留 session/turn 追踪。 |
| runtime | `runStartedAt`, `backendStage`, `backendStartedAt`, `lastBackendDurationSeconds`, `lastFirstEventSeconds` | UI 状态栏和诊断需要。 |
| question | `questionGroupCount`, `lastReadinessCheckGroup`, `currentQuestionText`, `currentQuestionTurnId`, `currentQuestionCount`, `awaitingUserAnswer` | 复刻当前提问区、输入提示和 readiness 节奏。 |
| archive | `interviewArchiveId`, `autoArchivePath`, `lastManualArchivePath`, `lastArchivedAt` | 用户提交、失败、完成都可能触发自动存档。 |
| content | `routeOverview`, `messages`, `summary.v1`, `inferences`, `recentQuestionTargets`, `applicabilityScores` | Prompt 构建和 UI 对话显示依赖。 |
| memory | `frameworkMemory.projectMemoryId`, `evaluationBatchId`, `batchStatus`, `promptVersionSnapshot`, `lastCompletedBatchId`, `reviewChains` | 与 `framework_memory.py` 对齐。 |
| output | `outputHistory`, `optionDifferences`, `lastError`, `updatedAt` | 高置信写回和差异回显依赖。 |

## 3. 主轮次生命周期

```text
用户输入
  -> EmbeddedInterviewPanel.run_ai_turn()
  -> ensure_ai_interview()
  -> ensure_project_memory()
  -> add_message(role=user, meta.turnId)
  -> status=running/backendStage=queued
  -> auto_save_interview_archive("user_submitted")
  -> worker_run_ai_turn()
      -> detect_force_output()
      -> should_force_readiness_check()
      -> build_interview_prompt()
      -> CodexCliBackend.run_turn(schema_mode=turn|readiness|full_output)
      -> handle_ai_result()
          -> validate_full_project_output()
          -> validate_ai_response_payload()
          -> add assistant message
          -> update routeOverview/inferences/applicability/summary
          -> question_group: currentQuestionText + record_question_group_review()
          -> readiness_check: schedule summary correction
          -> full_project_output: apply_high_confidence_output()
          -> record_prompt_runtime()
          -> write_turn_replay()
          -> auto_save_interview_archive("turn_completed")
          -> non-output: schedule background mapping
```

硬约束：

- 普通轮次不能直接写 `project_state.nodes`，只能更新 messages、summary、inferences、routeOverview。
- `full_project_output` 或合并后的 full output 才进入 `apply_high_confidence_output()`。
- 写回阈值是 `HIGH_CONFIDENCE_THRESHOLD = 0.75`。
- 低置信内容只能留在 `inferences` 或 memory context，不能伪装为已确认设计。

## 4. 分片全项目输出

强制输出时，嵌入式面板走分片并行：

```text
force_output
  -> choose_output_domain_partitions()
  -> for each domain partition:
       build_output_partition_prompt()
       CodexCliBackend.run_turn(schema_mode=partial_output)
       validate_partial_project_output(engine, payload, allowed_domain_ids)
       record_prompt_runtime()
       write_turn_replay()
  -> merge_partial_project_outputs()
  -> validate_full_project_output()
  -> apply_high_confidence_output()
```

分片输出必须满足：

- `mode=partial_project_output`
- `partialProjectOutput.domainIds` 等于当前分片 domainIds
- `projectStatePatchJson` 是 JSON 字符串
- patch 只能包含该分片 domain 下的 nodes
- `confidenceMapJson` 至少包含 `groups` 或 `nodes`

## 5. JSON Schema Modes

`core/design/ai_schema.py` 是访谈结构化输出事实源：

| schema mode | 允许 mode | 必填重点 |
| --- | --- | --- |
| `turn` | `question_group`, `confirmation`, `readiness_check`, `maintenance`, `error` | `questionGroup`, `readinessCheck`, `inferences` |
| `readiness` | `readiness_check`, `maintenance`, `error` | `readinessCheck`, `inferences` |
| `full_output` | 全部主 mode | `fullProjectOutput`, `optionDifferences`, `inferences` |
| `partial_output` | `partial_project_output`, `maintenance`, `error` | `partialProjectOutput`, `inferences` |
| `mapping` | `mapping`, `maintenance`, `error` | `inferences` |
| `summary` | `summary_correction`, `maintenance`, `error` | `summary` |

`CodexCliBackend` 的执行契约：

- 写入 schema 文件到 `runtime_root/ai_runtime`。
- prompt 写入临时目录 `ai_codex_*`。
- 执行 `codex exec --skip-git-repo-check -C <workdir> -s read-only --json --output-schema <schema> -o <output_path> ...`。
- 从 stdout 解析 JSON events，从 output 文件抽取最终 JSON object。
- 返回 `CodexRunResult(payload, session_id, raw_output, raw_events, duration_seconds, first_event_seconds, response_chars, api_profile, api_model, api_base_url)`。

## 6. 高置信写回契约

`apply_high_confidence_output()` 的行为：

- 以 AI candidate state 生成新的 normalized state。
- `gameplaySystems` 只有 `confidence.nodes.gameplaySystems >= 0.75` 时才复制，否则沿用当前状态。
- node 级置信达标才复制 `designNote`、`riskNote`、`notApplicableReason`。
- `system_concrete` 和 `content_concrete` 节点只有实体字段通过 `normalize_node_design_entities()` 才写入 `designEntities`。
- group/item 级置信达标才复制 checklist option selected/primary。
- 写回后刷新节点状态并记录 `optionDifferences`、`outputHistory`。

NEWrust 必须把该逻辑放在 Rust 后端 domain service，不允许 Web UI 自行合并状态。

## 7. 后台 Mapping 和 Summary Correction

非 full output 的成功轮次可能触发 background mapping：

- 条件来自 `should_schedule_mapping()`：用户文本包含显式 option signal，或接近 readiness check window。
- 调用 `build_mapping_prompt()` 和 `schema_mode=mapping`。
- `_handle_mapping_result()` 只记录 runtime，当前嵌入式面板未把 mapping payload 直接写回 project state。

`readiness_check` 后触发 summary correction：

- 调用 `build_summary_correction_prompt()`。
- 使用 `schema_mode=summary`。
- 结果只用于 summary 校正和渲染，不等价于设计写回。

## 8. Framework Memory 和 UCOS Bridge

`framework_memory.py` 写入项目内 AI 记忆：

- `ensure_project_memory()` 创建/轮换 `projectMemoryId`、`evaluationBatchId`。
- `record_question_group_review()` 记录非连续复核链。
- `record_ai_payload_context()` 记录验证失败、低置信映射、option differences。
- `record_backend_runtime_event()` 记录 Codex 不可用或运行异常。
- `complete_evaluation_batch()` 结束 batch 并聚合 memory。

`ai_ucos_bridge.py` 写入 `knowledge/ucos`：

| 类别 | 目标 |
| --- | --- |
| raw turn replay | `knowledge/ucos/knowledge/episodic/turns/<project>/<batch>/<turn>.json` |
| short-term router context | `knowledge/ucos/knowledge/short_term/entries/stm_router_*.json` |
| high-confidence semantic staging | `knowledge/ucos/knowledge/semantic/staging/staged_sf_interview_*.json` |
| full design generation episode | `knowledge/ucos/knowledge/episodic/episodes/ep_design_*.json` |

风险：搜索结果显示 `record_interview_turn()` 当前只在 `core/ui/ai_interview_window.py` 中调用，`core/ui/embedded_interview.py` 未直接调用。NEWrust 迁移必须统一为后端事件，确保内嵌与独立访谈路径都写入 UCOS 或等价 memory bus。

## 9. 通用 Completion JSON 服务

`CompletionJsonService` 被以下功能调用：

- `PatchAnalyzer.analyze()`：把补充开发需求转为 patch tasks。
- `extract_sdk_spec_with_completion()`：把 SDK 文档抽取为待审核 SDK spec。

契约：

- 从 `build_completion_adapter()` 读取 active completion entry。
- 支持 local Codex completion CLI、local Claude completion CLI、OpenAI/custom completion API。
- 用 `ModelTask(sandbox="read-only")` 调用 adapter。
- 接受纯 JSON、markdown fenced JSON、文本中包裹的 JSON object。
- `max_retries=1` 时最多尝试 2 次，第二次追加 “Return only one JSON object matching schema”。
- 返回 `CompletionJsonResult(ok, data, raw_text, attempts, schema_name, errors)`。

## 10. NEWrust 设计要求

- Rust 后端提供 `AiInterviewService`，持有 prompt 构建、schema mode、payload validation、高置信写回、archive、memory event。
- Web UI 只负责渲染 `aiInterview` 状态、输入提交、强制输出、标记不准、保存访谈存档。
- `CompletionJsonService` 独立为 `StructuredCompletionService`，给 patch 和 SDK 使用，不混入访谈状态机。
- 所有 AI 输出必须通过 typed schema + validator + evidence log 后才更新项目状态。
- UCOS bridge 和 framework memory 写入必须由后端统一触发，避免不同 UI 面板行为不一致。
