# AI 配置 / Adapter / 日志契约拆解

状态：第一轮确认。

## AI Config v3

配置路径：

```text
settings/ai_config.json
```

schema 根：

- `schema_version = 3`
- `dev`
- `image`
- `completion`

三类 category 都是：

```text
APICategory
  category_id
  entries
  active_entry_id
```

entry：

```text
APIEntry
  id
  label
  config_type
  api_url
  api_key
  extra_json
  codex_toml_path
  codex_json_path
```

兼容 profile：

```text
AIProfile
  id
  name
  adapter
  llm
  image
  metadata
```

active dev entry 会同步为 `active_profile_id`。

## Config Types

Dev：

- `local_codex_cli`
- `local_claude_cli`
- `openai_dev_api`
- `custom_dev_api`

Image：

- `codex_cli_image`
- `openai_image_api`
- `sd_webui_api`
- `custom_image_api`

Completion：

- `local_codex_completion_cli`
- `local_claude_completion_cli`
- `openai_completion_api`
- `custom_completion_api`

## Validation

`AIConfigValidator`：

- 检查 schema version。
- 检查 active entry 是否存在。
- 检查 entry id 唯一性。
- API entry 必须有 `api_url` 和 `api_key`。
- custom entry 的 `extra_json` 必须是 JSON object。
- OpenAI profile 必须是 API source，且有 base_url/api_key/model。
- Codex/Claude profile 必须有 CLI path。
- image API 必须有 base_url/api_key/model。
- `cli_builtin` image 只允许 Codex adapter。
- `check_cli=True` 时才执行 CLI `--version`，默认不做外部调用。

## Adapter

统一接口：

```text
ModelTask
  task_id
  prompt
  input_files
  output_files
  allowed_write_paths
  timeout_seconds
  sandbox
  cwd

ModelResult
  task_id
  status
  text
  errors
```

`get_pipeline_adapter()`：

```text
get_active_profile() -> get_adapter(profile.adapter, profile=profile)
```

adapter 映射：

- `none` -> `LocalAdapter`
- `local` -> `LocalAdapter`
- `codex` -> `CodexAdapter`
- `claude` -> `ClaudeCodeModelAdapter`
- `openai` -> `OpenAIAdapter`

`LocalAdapter` 当前是禁用占位，`generate()` 返回 failed。

## Adapter 行为

`OpenAIAdapter`：

- 从 active profile 或 legacy api config 读取 base_url/api_key/model/provider/temperature。
- 读取 `ModelTask.input_files` 并拼接进 prompt。
- 调用 `OpenAICompatibleCaller.invoke()`。

`CodexAdapter`：

- 使用 `codex exec --cd <cwd> --sandbox <task.sandbox> --skip-git-repo-check`。
- 执行前通过 `validate_allowed_outputs()` 确认 `output_files` 不越过 `allowed_write_paths`。

`ClaudeCodeModelAdapter`：

- 使用 `claude --print -p <prompt>`。
- 使用 `task.timeout_seconds`。

`build_completion_adapter()`：

- 从 active completion entry 构造 Codex/Claude/OpenAI adapter。
- completion 和 dev 使用同一个 `ModelAdapter` 接口，但读取不同 config category。

## 结构化日志

持久路径：

```text
RUN_LOGS_DIR = OUTPUTS_DIR / "run_logs"
pipeline_run_<run_id>.jsonl
```

`LogEntry`：

- `timestamp`
- `level`
- `context`
- `message`
- `source`
- `metadata`

`JsonlLogWriter.for_run(run_type, run_id)` 会清洗文件名并写 JSONL。

`core/main.py::run_range()` 写入：

- stage started
- stage inherited
- stage completed successfully
- stage failed

`MainWindow._get_log_panel()` 读取最近 5 个 JSONL 并填充 `LogPanel`。

`LogPanel`：

- level filter
- clear
- export JSONL

## NEWrust 设计约束

- AI 配置编辑属于 Web UI；配置验证和 adapter 构造属于 Rust 后端。
- API key 存储必须沿用本地 settings 文件语义，不提交到仓库。
- pipeline AI 调用必须使用统一 `ModelTask/ModelResult` 等价接口。
- Codex output path guard 必须保留。
- 日志 JSONL 是运行证据，不能只做前端临时 console。
