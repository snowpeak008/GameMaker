# AI 配置与 Python 行为对齐、Provider/CLI 设计复核

## 实施状态（2026-07-11）

- AI v3 的 `dev`、`completion`、`image` 三类解析、掩码合并、CLI/API 可用性探测和实际运行适配已对齐。
- 本地 Codex/Claude CLI、OpenAI-compatible API、本地/中转 HTTP 配置可独立选择；打开配置页不会自动访问网络。
- 密钥、危险 URL、CLI 路径和 Provider 原始错误已在 IPC、Debug、日志与存档边界脱敏。
- Provider Registry v4 迁移仍按总计划 Phase 8 显式延期，未擅自改变现有 v3 配置格式。

## 记录状态

- 以下内容保留为最初审查与设计依据；实际完成状态以上述“实施状态”和总计划最终基线为准。

## 用户目标

AI 配置需要同时支持以下场景，并且互不干扰：

1. 使用本机 Codex/Claude CLI 时，持续使用本地 CLI，不要求填写 API 地址或 API Key。
2. 使用中转平台、OpenAI-compatible 服务或其他独立 API 时，可以单独保存地址、认证方式、模型和扩展参数。
3. 本地 CLI、独立 API、中转 API 等配置可以并存；选择某个用途的当前配置时，不应覆盖其他用途的选择。
4. 当前配置的显示、保存和程序实际使用必须立即且一致，重启后仍能恢复。

## 代码审查结论

### Python 项目是当前行为基准

`core/config/ai_config_schema.py` 已经定义了 schema v3，并按 `dev`、`image`、`completion` 三个类别保存独立的 `active_entry_id`。每个条目包含 `config_type`、地址、密钥、扩展 JSON，以及 Codex 配置文件路径等字段。

Python 侧的关键行为如下：

- 本地类型包括 Codex CLI、Claude CLI，以及 Codex CLI 图像能力；这些类型不要求 API 地址和 API Key。
- `core/config/ai_config.py` 会把本地类型转换为 `source=cli`，分别使用 `codex` 或 `claude`；API 类型转换为 `source=api`，从扩展 JSON 读取 provider、model、timeout、temperature、reasoning 等参数。
- `core/config/validator.py` 不仅检查字段格式，还可以在 `check_cli=True` 时通过 PATH 定位 CLI 并执行版本检查；本地 CLI 不可用时应给出明确错误。
- `core/ui/ai_config_unified_dialog.py` 会显示探测到的 CLI 命令，并为 Codex TOML/JSON 配置提供原生文件选择器；文件路径是辅助/引用信息，不应替代 CLI 本身。
- 兼容层仍支持旧 profile、`api_config.toml` 和环境变量读取；保存时以 `settings/ai_config.json` 的 v3 结构为主。
- `profiles` 目前是由 dev 条目生成的兼容投影，并不是完全独立的一套 provider/launch profile 模型。

因此，Python 与 NEWrust 的差异重点不是“字段完全不同”，而是“运行时语义和配置发现能力没有完整迁移”。

### NEWrust 当前实现的对齐情况和缺口

`NEWrust` 的 Rust contract、config service 和前端模型已经基本采用同一套 v3 类别和条目字段，且完成/开发/图像类别的 active ID 也已存在。这部分可以保留，不建议为了引入新概念立即重写现有 JSON。

当前缺口如下：

- 本地 CLI 输入目前主要显示静态等待提示；没有复刻 Python 的 PATH 定位、`--version` 探测和“检测失败但仍可保存”的明确状态。
- Rust adapter 最终按 `codex`/`claude` 命令启动。若命令不在 PATH，错误只能在实际运行时暴露，而不是在配置界面或按需探测阶段暴露。
- Codex TOML/JSON 路径目前没有复刻 Python 的原生文件浏览和路径发现体验。
- `adm-new-config` 目前主要做 schema、重复 ID、active ID 和必填字段校验；本地 CLI 可用性检查尚未实现。
- `LocalModelAdapter` 是占位适配器，不等价于任意本地 OpenAI-compatible HTTP 服务。Ollama、局域网中转或 SD WebUI 一类服务应作为 API/custom API 条目保存地址、认证和模型，而不是误判成 CLI。
- 前端仍把“当前”建模为右侧表单中的 checkbox；左侧列表只有徽标，没有独立的“设为当前配置项”操作，且激活后列表状态和左侧总览刷新存在延迟。这与 `2026-07-10_002_ai-config-current-entry-ui.md` 关联处理。
- `active_profile_id` 目前主要由 dev 的 active entry 投影得到，不能表达“开发使用本地 Codex、完成使用独立中转、图像使用另一个服务”之外更丰富的任务/启动组合。
- 保存、激活、校验和实际启动之间缺少一个可观察的“解析结果”层，用户无法在启动前确认最终会使用哪个 provider、命令、地址和模型。

## `cc-pane` 可借鉴的设计

本次只借鉴概念，不直接复制其实现：

- Provider registry 与 launch profile 分离。provider 负责地址、认证、模型、配置目录等可复用连接信息；launch profile 负责一次启动/任务使用哪个 CLI、provider、运行时、技能和环境。
- provider 支持增删改查、设为默认，并可在启动时明确选择；也可以继承默认值或选择不注入 provider。
- launch profile 有独立的 CRUD、默认项和 `preview/resolution` 阶段，在真正启动前解析出最终配置。
- provider service 还提供配置目录检查和在资源管理器中打开目录的能力，适合用于本地 CLI 配置发现和诊断。
- CLI 通过独立 adapter 层扩展，provider、启动 profile、CLI 适配器和持久化之间通过服务/IPC 分层，便于继续增加其他 CLI 或中转平台。

参考：[`cc-pane` README](https://github.com/wuxiran/cc-pane#readme)、[`providerService.ts`](https://github.com/wuxiran/cc-pane/blob/main/web/services/providerService.ts)、[`launchProfileService.ts`](https://github.com/wuxiran/cc-pane/blob/main/web/services/launchProfileService.ts)。

## 对齐后的目标设计

### 1. 保留现有 v3 条目作为兼容基础

短期不强制改写现有 `settings/ai_config.json`。继续识别以下语义：

- `source=cli`：本地 Codex/Claude CLI，命令由配置的 CLI 类型和可选路径覆盖决定。
- `source=api`：OpenAI、OpenAI-compatible 中转、其他自定义 HTTP API；地址、认证、模型和扩展参数独立保存。
- `source=cli_builtin`：仅用于已有的内置图像能力语义。

统一读取 camelCase/snake_case 兼容字段，保存时保留未知扩展字段，避免旧配置丢失。

### 2. 将“连接定义”和“使用选择”逐步解耦

建议形成两层概念（可先落在现有字段上，再逐步增加显式 ID）：

- `ProviderDefinition`：本地 CLI 或 API 服务的可复用定义。包括 provider kind、base URL、认证来源（Key/环境变量/无认证）、model、额外 header/body、超时、推理参数、能力标签，以及可选配置目录。
- `AIUseProfile`/`LaunchProfile`：某个类别或一次任务的使用方案。包括使用的 CLI/provider ID、运行时、环境策略和类别范围。

这样可以让同一个中转 provider 被开发和完成复用，也可以让完成单独切换而不改变开发配置；本地 CLI 仍是一个真实的 provider 类型，而不是伪装成 API。

### 3. 本地 CLI 的明确语义

- Codex/Claude CLI 条目不显示或不要求 API URL、API Key。
- 提供“检测 CLI”操作，返回命令路径、版本、可用/不可用和错误原因；保存配置不应因为暂时未安装 CLI 而破坏其他 API 配置。
- 默认先使用 PATH；必要时允许用户选择/填写命令路径覆盖，并在启动前再次解析。
- CLI 的本地配置目录（例如 Codex TOML/JSON）只作为可选诊断和引用，不把其中的密钥复制到项目配置或日志。

### 4. 独立 API/中转平台的明确语义

- 以 `api_url`/`base_url`、认证来源、model 和 `extra_json` 保存；OpenAI-compatible URL 统一规范化，避免重复拼接 `/v1` 或 `/chat/completions`。
- API Key 支持“直接保存”“引用环境变量”“无认证”三种模式；界面始终脱敏，日志和错误信息不得输出密钥。
- 自定义 API 允许额外 headers/body、timeout、temperature、reasoning effort 等参数，但应有结构校验和清晰的能力声明（文本、图像、完成等）。
- 本地 Ollama、局域网服务和中转平台只要走 HTTP，就归入 API/custom API；不新增一个语义含糊的 generic local adapter 来替代它们。

### 5. 类别和激活规则

- `dev`、`completion`、`image` 保持独立 active ID；改变一个类别的当前项不得隐式改变另外两个类别。
- 如果后续引入 launch profile，profile 只引用 provider，不复制密钥和完整连接参数；类别 active ID 或任务 profile 决定最终引用哪个 provider。
- UI 中“设为当前配置项”应是明确的动作按钮，动作完成后立即更新左侧列表、总览和当前标记，并在保存后持久化；取消/删除当前项时必须有确定的回退规则。
- 启动前增加 provider resolution/preview：展示最终 CLI 命令或 API 地址（脱敏）、模型、来源和能力；未选择 provider、CLI 不存在或 API 参数不完整时，在启动前失败。

### 6. 兼容与迁移

- 继续读取 Python v3 `settings/ai_config.json`，并兼容旧 `ai_profiles.json`、`api_config.toml` 和环境变量。
- 首次保存或显式迁移时再升级到新的内部版本；未识别字段保留，旧 profile 以只读/兼容 provider 方式投影，避免静默改变用户当前配置。
- 机器级、含密钥的 provider 数据与项目级“当前使用哪个 profile”可以在后续拆分；第一阶段仍可保持现有 settings 位置，待项目隔离需求明确后再增加覆盖层。

## 建议开发顺序

### P0：契约与回归基线

- 建立 Python v3 与 NEWrust 的固定 JSON fixture，覆盖三类别、CLI、OpenAI-compatible、自定义 API、旧字段和无效 active ID。
- 明确 `source`、认证来源、模型和 URL 规范化规则。
- 先完成前一条 fixplan 中的当前项即时刷新，不把 UI 状态问题与 provider 迁移混在一起。

### P1：CLI/API provider 能力补齐

- 实现 Codex/Claude PATH、版本和可选路径探测；区分“未安装”“权限失败”“命令运行失败”。
- 补齐 API/custom API 的认证来源、模型和扩展参数校验；保留本地 HTTP 服务作为 API provider。
- 加入按需 probe/preview，不在打开配置页面时默认发起网络请求。

### P2：provider registry 与使用 profile

- 在不破坏 v3 读取的前提下，引入 provider ID 引用和 provider CRUD/default。
- 让 dev/completion/image 或一次任务分别选择 provider；实现 inherit、explicit select、no injection 等策略时要先明确产品需求。
- 增加配置目录信息检查、原生文件浏览和“在资源管理器中打开”能力。

### P3：迁移、安全和扩展

- 提供旧配置迁移预览、回滚/备份和未知字段保留。
- 统一密钥脱敏、环境变量引用和日志审计。
- 再评估是否需要更多 CLI adapter、项目级覆盖或远程 provider 管理。

## 验收标准

- 仅安装本地 Codex/Claude CLI、没有 API URL/Key 时，配置可保存、检测可见、实际任务能继续使用本地 CLI。
- 本地 CLI、OpenAI-compatible 中转、自定义独立 API 可以同时存在；切换其中一个不会改写其他条目或类别的 active ID。
- API 配置可使用 URL + Key、环境变量 Key 或无认证模式；Key 在界面和日志中均脱敏。
- 当前项点击后立即反映在列表、总览和后端状态中，重启后 active ID 与实际 adapter 一致。
- Codex/Claude 不可用、API 地址/模型缺失、URL 不可解析等问题能在保存或启动前给出具体错误。
- 旧 Python v3 配置读取后语义不变，未知字段不丢失；没有通过 fixture 和回归测试前不进行默认迁移。

## 来源

- Python 行为基准：`core/config/ai_config_schema.py`、`core/config/ai_config.py`、`core/config/validator.py`、`core/ui/ai_config_unified_dialog.py`、`core/adapters/registry.py`。
- NEWrust 当前实现：`NEWrust/web/src/features/ai-config.js`、`NEWrust/web/src/index.html`、`NEWrust/crates/adm-new-contracts/src/ai.rs`、`NEWrust/crates/adm-new-config/src/lib.rs`、`NEWrust/crates/adm-new-ai/src/lib.rs`、`NEWrust/crates/adm-new-ai/src/adapters.rs`。
- 相关已有记录：`2026-07-10_001_project-config-path-discovery.md`、`2026-07-10_002_ai-config-current-entry-ui.md`。
- 外部参考：[`wuxiran/cc-pane`](https://github.com/wuxiran/cc-pane)、[`providerService.ts`](https://github.com/wuxiran/cc-pane/blob/main/web/services/providerService.ts)、[`launchProfileService.ts`](https://github.com/wuxiran/cc-pane/blob/main/web/services/launchProfileService.ts)。
