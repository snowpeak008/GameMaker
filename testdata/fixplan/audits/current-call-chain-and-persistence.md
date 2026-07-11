# 当前调用链、schema 与持久化审查

## 2026-07-11 最终实现补充（本节取代下方历史风险结论）

下方长文保留 2026-07-10 的开发前/开发中审查轨迹；凡与本节冲突，以本节及 `baseline/2026-07-11-final.md` 为准。

| 领域 | 2026-07-11 当前实现 |
| --- | --- |
| 项目配置 | 逻辑项目文档只保存 `binding_id` 与非机器字段；绝对项目/编辑器路径保存在机器级 `settings/project_bindings.json`，该文件已加入仓库忽略规则。原生目录/文件选择、Unity 识别、编辑器发现和失效路径重连已接入。IPC 的 `saved_path` 不再返回内部存储路径。落盘 preflight 会清除机器路径并脱敏消息。 |
| AI 配置 | Web 只接收掩码密钥；保存时与磁盘秘密做字段级 mask merge。`dev/image/completion` 各自解析活动项；CLI/API probe 是显式动作。危险的带 userinfo/query/fragment URL 不进入 IPC/DOM且无法通过保存校验。CLI Prompt 走 stdin，提供商错误在桌面边界改写为固定安全消息。 |
| 流水线 IPC | `PipelineView` 仅序列化安全 `PipelineStateView` 和阶段摘要；raw state、outputs、artifact records、文件正文和 Base64 均不进入普通视图。Step07 只通过受控读取生成 Blob URL 预览，释放时 revoke。 |
| checkpoint/恢复 | current checkpoint 是 CAS commit point；revision 精确递增、身份/指纹冻结、终态不可回退、Committed/Skipped 单元不可重写。停止会持久化 `StopRequested`，准备失败会终结 checkpoint；`RecoveryBlocked` 可在用户修复外部状态后再次只读核验，只有证明安全才转回 `Recoverable`。 |
| Step07/11/12 | 三个长副作用阶段均使用安全工作单元。Step11 在临时隔离工作区运行 CLI，只复制声明输入，只提交声明输出，并扫描全部意外变化；输入内容哈希变化会使旧结果失效。Step12 PNG 写入校验最近既存祖先，拒绝越过项目根的 symlink/junction。空、相对失效或非 Unity 项目路径不会回退到应用数据目录。 |
| 日志/Debug | 普通日志统一屏蔽绝对路径、URL、凭据、JWT 和长 Base64；敏感 DTO 使用安全 Debug。Windows CLI 超时会终止进程树，stdin 写入与 stdout/stderr 排空不再绕过超时。 |
| 打包 | `tools/build-portable.ps1` 是唯一 portable 构建真相；本机默认构建校验并保留既有 `user_data`，显式 `-CleanUserData` 生成干净分发候选且拒绝覆盖非空数据。发布 exe 支持隔离 `--smoke --smoke-report`。 |

仍然成立的边界：AI v3 对未建模的顶层/category/entry 字段不承诺 round-trip，扩展字段应放在 `extra_json`；真实第三方 Provider 的账号权限、网络连通性和主观图片质量必须在用户环境验收；Provider v4 与 Godot/Unreal 完整扩展属于需产品确认的 Phase 8。

- 审查日期：2026-07-10
- 审查对象：`NEWrust` 当前工作树
- 性质：现状记录，不把后续计划目标写成已实现
- 路径约定：下文的 `<data-root>` 是桌面运行时数据根，`<session>` 是动态草稿会话 ID

## 1. 总览

| 领域 | Web 入口 | Tauri 命令 | application/domain | 主持久化位置 | 当前 schema |
| --- | --- | --- | --- | --- | --- |
| 项目配置 | `features/settings-style.js` | `commands/config.rs` | `RuntimeApplicationService` | 草稿逻辑配置 + 机器绑定表 | project 2；binding store 1 |
| AI 配置 | `features/ai-config.js` | `commands/config.rs` | `AiConfigApplicationService` → `AiConfigService` → `adm-new-config` | `settings/ai_config.json` | 3 |
| 流水线状态 | `features/pipeline.js` | `commands/pipeline.rs` | `PipelineApplicationService` / `PipelineService` / executor | 两份 state + checkpoint | state 2；checkpoint 1 |
| Step07 | `features/pipeline.js` 专用投影 | pipeline 命令与受控 artifact read | `ProductPipelineExecutor` → `Step07OutputGenerator` | `stage_07` 内 JSON/PNG | 各文档 1 |

`ProjectPaths` 的当前目录投影是：

```text
<data-root>/
├─ settings/
├─ drafts/<session>/
│  ├─ project_config.json
│  └─ outputs/
│     ├─ artifacts/stage_07/
│     ├─ checkpoints/pipeline/<run-id>/
│     ├─ runtime_control/pipeline_state.json
│     └─ pipeline_state.json
└─ saves/
```

## 2. 项目配置

### 2.1 读取与写入调用链

读取：

```text
项目配置按钮
→ initSettingsStyleModals.openProjectConfig
→ createSettingsStyleApi.loadProjectConfig
→ invoke("load_project_config")
→ desktop-tauri commands::load_project_config
→ adm-new-tauri-commands::config::load_project_config
→ RuntimeApplicationService::load_project_settings(false)
→ 优先读 drafts/<session>/project_config.json
→ 按 binding_id 合并 settings/project_bindings.json
→ 返回有效 ProjectRuntimeSettings 给表单
```

保存：

```text
表单内“保存”
→ readProjectConfigForm（仍是弹窗草稿）
→ buildProjectConfigSaveRequest（snake_case）
→ invoke("save_project_config")
→ desktop 层先拒绝运行中修改
→ RuntimeApplicationService::save_project_settings
→ 机器路径写 settings/project_bindings.json
→ 逻辑字段写 drafts/<session>/project_config.json
→ 可选 run_actual_development_preflight
→ 返回有效 settings、saved_path、preflight
```

路径选择和 Unity 发现是独立的显式动作：

```text
选择项目/编辑器
→ select_native_path（Tauri dialog）
→ 仅把 selected path 写进当前表单；cancelled 保留旧值
→ 项目目录选择后 inspect_project_environment
→ 用户点击发现按钮后 discover_project_unity_editors
→ 用户从候选 select 明确选择
→ 只有再次点击保存才持久化
```

### 2.2 schema 与文件

`ProjectRuntimeSettings` 当前字段：

```text
schema_version=2
binding_id
project_engine
pipeline_adapter
custom_engine_name
required_editor_version
development_path       # 只存在于“有效设置”与机器绑定
editor_path            # 只存在于“有效设置”与机器绑定
```

| 文件 | 写入者 | 读取者 | 当前内容 |
| --- | --- | --- | --- |
| `drafts/<session>/project_config.json` | `save_project_settings` | `load_project_settings` | schema 2；逻辑字段、binding ID；会移除两项机器路径；保留已有未知字段 |
| `settings/project_bindings.json` | `save_project_settings` | `resolve_project_binding` | schema 1；binding ID → project/editor path、verified_at |
| `settings/project_settings.json` | 兼容/默认配置来源 | 空草稿及无 active document 时读取 | 旧设置兼容来源，不是保存当前项目的首选目标 |
| `drafts/<session>/outputs/preflight/actual_development_preflight.json` | `run_actual_development_preflight(true, …)` | 人工/诊断 | schema 1；当前仍包含合并后的有效 settings |
| `drafts/<session>/runtime/project_settings.snapshot.json` | `create_run_context` | 运行上下文 | schema 1；当前仍包含有效机器路径 |
| `drafts/<session>/runtime/run_context.json` | `create_run_context` | executor/隔离校验 | schema `"1.0"`；当前仍包含有效机器路径及多项绝对运行路径 |

### 2.3 UI、日志与存档面

- UI：项目路径与编辑器路径明确显示在输入框；Unity 候选路径作为 `<option value>` 存在。`ProjectConfigView.saved_path` 会经 IPC 返回，但当前 Web 不渲染它。
- 日志：保存本身只写状态文案；preflight 的 warning/error 字符串可能包含有效路径，若被上层直接记录仍需脱敏审查。
- 存档：主项目文档已不含两项机器路径；但 preflight、settings snapshot 与 run context 仍在草稿树中含机器/绝对路径。它们是否进入存档取决于同步边界，当前不能宣称“所有存档均无机器路径”。这是后续 FIN-02 必查项。

## 3. AI 配置

### 3.1 读取、草稿与保存调用链

```text
打开 AI 配置
→ createAiConfigApi.load
→ invoke("load_ai_config")
→ desktop commands::load_ai_config
→ AiConfigApplicationService::load_or_default
→ AiConfigService::load_or_default
→ adm-new-config::load_ai_config_contract
→ settings/ai_config.json（不存在则默认）
→ Web normalize 后深拷贝为 modal draft
```

配置卡选择、字段输入、`setActive(categoryId, entryId)` 都只修改弹窗草稿。取消关闭时丢弃草稿；保存链为：

```text
同步当前详情表单到 draft
→ validate_ai_config(draft)
→ save_ai_config({ config: draft })
→ AiConfigService::save
→ normalize → validate
→ adm-new-config::save_ai_config_contract
→ 原子写 settings/ai_config.json
```

descriptor、resolution preview 和 CLI probe 均使用当前传入草稿，不隐式保存。CLI probe 只有用户触发时才运行。

### 3.2 schema 与兼容边界

- 文件 schema：3，文件字段为 `schema_version/category_id/active_entry_id/config_type/api_url/api_key/extra_json/...`。
- IPC contract：`adm-new-contracts::ai::AiConfig` 使用 camelCase serde；Web 同时容忍 camel/snake 输入。
- 三类别：`dev`、`image`、`completion` 各自保存 `active_entry_id`。
- `activeProfileId` 仅作为 dev 兼容投影；保存文件只写三类别，不写 profiles。
- 旧 schema/profile 由 `normalize_ai_config_value` 迁移成三类别。
- 当前稳定扩展边界是 entry 的 `extra_json` 对象。顶层、category 顶层或 entry 未建模字段会在当前 normalize/save 中丢失，因此不能把它们写进“保证 round-trip”的 fixture。

### 3.3 UI、日志与存档面

- UI/IPC：`load_ai_config` 必须把 API Key 交给编辑表单，因此秘密会存在于 IPC payload 和密码输入控件的 value；它不应进入 `textContent`、普通日志、preview 或截图。
- resolution view 和 CLI probe view 是脱敏类型；secret-bearing target 不实现 Serialize，Debug 也掩码。
- 保存目标是机器级 `settings/ai_config.json`，不应复制进项目存档。
- completion 运行链当前再次从 `AiConfigService` 读取 active completion entry；CLI/API adapter 的实际对齐仍应由跨层测试保证。

## 4. 流水线状态与恢复

### 4.1 调用链

```text
Web 输入 from/to
→ invoke("run_pipeline_range")
→ desktop 先调用 PipelineApplicationService::resolve_range
→ 生成 run_id / attempt_id、fingerprints
→ 创建 PipelineCheckpointObserver 并写初始 checkpoint
→ 更新并持久化 PipelineRunState
→ 立即返回 accepted view
→ spawn 后台 worker
→ command service → PipelineService → ProductPipelineExecutor
→ observer 在安全单元前后更新 checkpoint
→ worker 合并 stop 状态、写最终 state、写运行日志
→ Web 运行中每 750ms load_pipeline_view 轮询
```

停止使用 AppRuntime 的同一 `AtomicBool`，并同步更新 `PipelineRunState.stop_requested`。恢复入口是独立 `resume_pipeline` 命令；普通 run 不隐式恢复。

### 4.2 schema 与文件

`PipelineRunState` 默认 schema 2，包含 `run_id`、`attempt_id`、`parent_attempt_id`、`attempt_no`、`state_version`、canonical 范围、当前 stage/unit、recovery 摘要和各 stage 结果。

| 文件 | 用途 | 规则 |
| --- | --- | --- |
| `outputs/runtime_control/pipeline_state.json` | 当前控制副本 | 每次状态持久化原子写 |
| `outputs/pipeline_state.json` | 耐久副本 | 与控制副本同写；启动分歧时优先控制副本并重写 |
| `outputs/checkpoints/pipeline/<run>/current.json` | 当前 checkpoint | schema 1；revision 必须递增；验证 lineage/fingerprint |
| `outputs/checkpoints/pipeline/<run>/attempts/<attempt>.json` | attempt 历史 | 同 revision/lineage 规则 |

损坏 state 会改名隔离；旧 state 会升级到 schema 2。中断状态只有在 checkpoint 有效时才成为 `recoverable`，否则是 `recovery_blocked`。运行中单元在启动恢复审查时转为 `unknown + reconcile_required`，不能盲目继续。

### 4.3 UI、日志与存档面

- `PipelineView` 的 IPC 结构当前仍包含 stage `outputs`、`artifacts` 和 artifact `content_preview`；Web model 也仍 normalize 这些字段，但普通步骤渲染采用白名单，不显示通用产物/outputs。
- Step07 专用预览是唯一用户可见 artifact read：固定 stage 07、受控相对路径、Base64 只拼 `img.src`。
- recovery view 不返回 checkpoint path。
- 日志会记录范围、最终状态及 stage errors；error 字符串仍需持续做路径/秘密脱敏扫描。
- state 与 checkpoint 位于草稿 outputs；它们是内部恢复数据，不应进入通用 artifact UI/index。

## 5. Step07

### 5.1 生成与确认调用链

```text
ProductPipelineExecutor 执行 stage 07
→ Step07OutputGenerator::generate
→ 从 stage_04 与 stage_06 读取前置文档
→ generate_step07_outputs
→ 生成/复用 style options
→ generate_style_option_images（当前真实 provider 未接通时生成 640×384 fallback）
→ staging 校验后发布 generated_images
→ 写 style_options / generation manifests / pending confirmation
→ 返回 waiting_confirmation 和专用 style_options 投影
→ PipelineView::style_options
→ Web renderStyleOptions
→ read_pipeline_artifact(stage=07, image_path)
→ MIME/Base64/截断/PNG 尺寸检查
→ <img src="data:image/png;base64,…">
```

确认链：

```text
用户选择 style + notes
→ confirm_style
→ ProductPipelineExecutor::confirm_style 写 confirmation
→ 单独重跑 07 读取已批准 confirmation
→ 写 style_application_contract
→ pipeline state 变为成功/style_confirmed 投影
```

### 5.2 文件与 schema

`drafts/<session>/outputs/artifacts/stage_07/` 当前主要文件：

| 文件 | schema | 说明 |
| --- | --- | --- |
| `style_options.json` | 1 | options、推荐项、相对 `image_path`、image status/message |
| `generation_log.json` | 1 | requested/provider_generated/fallback/failed、逐项记录、尺寸/格式 |
| `generated_images_manifest.json` | 1 | 当前与 generation log 同投影 |
| `generated_images/*.png` | PNG | 当前 fallback 为 640×384；旧 1×1 只做兼容识别 |
| `style_confirmation_pending.json` | 1 | 人工门禁等待信息 |
| `style_confirmation.json` | 1 | 选择、notes、selected option |
| `style_application_contract_pending.json` / `style_application_contract.json` | 1 | 下游应用契约 |

统计语义：fallback 不计入 `provider_generated_count` 或结果的 `generated_image_count`；当前 fallback 总状态为 `degraded`。

### 5.3 可见面

- 用户看到标题、说明、推荐/来源状态及图片，不看到 `image_path`、文件名、Base64 文本或 generation manifest。
- 旧 1×1 图片显示“旧版占位图”提示，不冒充有效预览。
- IPC 的受控 artifact response 仍含 `relative_path` 与 Base64；这是后端能力边界，不是通用 UI 投影。

## 6. 当前风险清单（不是完成声明）

1. preflight、settings snapshot、run context 仍含机器/绝对路径；需在存档同步与日志边界继续审计。
2. AI v3 只保证 `extra_json` 内扩展 round-trip，未建模顶层字段会丢失。
3. Pipeline IPC 仍携带内部 outputs/artifact metadata；目前依赖 Web 白名单隐藏，跨层测试必须防止重新渲染。
4. Step07 当前可见 fallback 已不是 1×1，但真实 image provider 的完整执行与恢复单元仍属后续阶段。
5. whole-stage checkpoint 已建立；Step11/12 的细粒度副作用 reconcile 仍不能仅凭本审查宣称完成。
