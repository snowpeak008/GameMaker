# 中英文技术术语过度翻译纠正计划

## 状态

已完成（2026-07-11）：中英文语言包、流水线恢复状态和候选类型文案已对齐；AI、API、CLI、SDK、URL、ID、JSON、Markdown 等技术词受阻断式术语扫描保护。

## 问题

前一轮中英文界面本地化虽然完成了语言切换、双语 key 对齐和英文纯度检查，但中文文案对部分技术术语进行了过度意译，导致界面与项目代码、配置文件、日志和用户实际使用的技术名称不一致。

已经确认的典型问题包括：

- `SDK` 被翻译成“开发工具包”；
- `AI 配置` 被翻译成“智能助手配置/智能助手配置管理”；
- `API` 被大量翻译成“接口”；
- `CLI` 被翻译成“命令行”；
- `URL` 被翻译成“网址”；
- `ID` 被翻译成“标识/编号”；
- `Markdown` 被翻译成“标记文本”；
- `JSON` 被翻译成“结构化数据”；
- `AI Adapter` 被翻译成“人工智能适配器”；
- 一些 `Prompt` 字段在技术 UI 中被全部翻译成“提示词”。

这些翻译并非都错误，但在配置、状态、字段名、导航和技术诊断场景下会隐藏真实协议/工具名称，造成用户寻找配置项、对照 Python 项目或查看日志时的认知成本。

## 审计范围与证据

主要审计范围是 NEWrust Web 的应用自有语言包和静态初始文案：

- `NEWrust/web/src/locales/shell.js`
- `NEWrust/web/src/locales/settings.js`
- `NEWrust/web/src/locales/utility.js`
- `NEWrust/web/src/locales/pipeline.js`
- `NEWrust/web/src/locales/design.js`
- `NEWrust/web/src/locales/design-content.generated.js` 及其生成脚本/源数据
- `NEWrust/web/src/index.html` 中用于首次渲染和无脚本场景的中文 fallback 文案
- `NEWrust/web/src/i18n.js`、`NEWrust/web/scripts/i18n-test.mjs` 中的术语检查能力

已定位的代表性 key：

| 文件/区域 | 当前文案 | 问题 |
| --- | --- | --- |
| `shell.js` 的 `nav.sdk`、`sdk.*` | “开发工具包知识库”“开发工具包名称”等 | SDK 是项目和用户使用的正式技术缩写，不应在技术导航和字段中隐藏 |
| `shell.js` 的 `action.aiConfig`、`aiConfig.*` | “智能助手配置”“智能助手配置管理” | AI 被扩写为产品含义不完全相同的“智能助手” |
| `shell.js` 的 `state.*Ai*`、`ai.*` | “等待智能助手状态”“智能访谈”等 | AI 相关状态和角色名称不统一 |
| `settings.js` 的 `settings.aiConfig.*`、`enum.aiConfig*` | “开发接口”“图像生成接口”“文本补全接口” | API 被泛化为“接口”，丢失 API 语义和检索关键词 |
| `settings.js` 的 API 字段 | “接口地址”“接口密钥” | 应直接显示 API URL、API Key |
| `settings.js` 的 CLI 类型 | “本地 Codex 命令行”“Codex 命令行图像生成” | CLI 是实际配置类型和启动入口名称 |
| `utility.js` 的 `utility.sdk.*` | 多处“开发工具包” | SDK 记录、SDK 状态和 SDK 表格应保持同一术语 |
| `pipeline.js` 的 `pipeline.detail.aiAdapter` | “人工智能适配器” | 语气冗长，且与后端/代码中的 AI adapter 不一致 |
| `shell.js` 的格式枚举 | “标记文本”“结构化数据” | 用户需要选择 Markdown、JSON 格式，而不是抽象描述 |
| `shell.js`/`index.html` 的 ID 字段 | “节点标识”“存档编号”等 | 技术字段应保留 ID，便于与日志、请求和文件字段对应 |

当前 `NEWrust/web/scripts/i18n-test.mjs` 只检查 key 对称、占位符、英文汉字和部分括号文案，没有检查中文技术词是否被过度翻译，也没有统一术语表。因此现有测试可以全部通过，仍然无法发现本问题。

## 术语纠正原则

### 需要保留标准写法的技术词

以下词在技术 UI、配置字段、状态标题、枚举和诊断信息中保持标准写法，中文只负责补充上下文：

| 标准词 | 中文界面建议 | 禁止继续使用的泛化写法 |
| --- | --- | --- |
| SDK | `SDK`、`SDK 知识库`、`SDK 名称` | 开发工具包 |
| AI | `AI`、`AI 配置`、`AI 访谈`、`AI 状态` | 智能助手、人工智能（作为普通 UI 前缀时） |
| API | `API`、`开发 API`、`图像生成 API`、`文本补全 API` | 开发接口、图像生成接口、文本补全接口 |
| CLI | `CLI`、`本地 Codex CLI`、`Codex CLI 图像生成` | 命令行（作为配置类型或工具名称时） |
| URL | `URL`、`API URL`、`来源 URL` | 网址（作为技术字段时） |
| API Key | `API Key` | 接口密钥 |
| ID | `配置项 ID`、`节点 ID`、`存档 ID`、`模板 ID` | 配置项标识、节点标识、存档编号 |
| Markdown | `Markdown` | 标记文本 |
| JSON | `JSON`、`附加 JSON` | 结构化数据（作为格式名称时） |
| TOML | `TOML` | 不应改写为中文格式描述 |
| Prompt | 技术字段/编辑器标题使用 `Prompt` 或 `图像 Prompt`；自然语言说明可使用“提示词” | 在所有技术字段中无条件替换为“提示词” |
| Adapter | `AI Adapter` 或 `AI 适配器` | 人工智能适配器 |
| Provider、Model、Token | 在配置和诊断字段中保留 `Provider`、`Model`、`Token`，必要时采用“Provider 名称”等混合写法 | 为追求纯中文而使用含义不确定的泛称 |

### 不进行无差别英文替换的普通产品词

本次是纠正过度翻译，不是把整个中文界面改成中英混排。因此以下词在当前产品语境中可以继续使用自然中文，除非后续单独发现语义问题：

- “设计工作台”；
- “开发流水线”；
- “补充开发”；
- “打包”；
- “存档”；
- “工作区”；
- “运行时”；
- “产物”；
- “校验/验证”；
- “风格提示词”等面向普通用户的解释性文案。

规则是：标准技术名称保留，产品流程和用户动作保持自然中文，避免再次出现反向的“过度英文设计”。

## 修复方案

以下为后续实施步骤，本轮不执行。

### P0：建立统一术语表和替换边界

1. 在 Web 本地化维护位置建立可审阅的技术术语表，至少覆盖 SDK、AI、API、CLI、URL、API Key、ID、Markdown、JSON、TOML、Prompt、Adapter、Provider、Model、Token。
2. 明确每个术语的三种使用场景：
   - 技术字段/配置类型：保留标准词；
   - 导航/按钮：采用简短混合词；
   - 普通解释句：允许自然中文，但不能改变技术对象含义。
3. 不修改 translation key、不修改协议枚举、不修改配置 schema、不修改后端字段；只调整展示文案和必要的检查规则。
4. 中英文语言包仍保持 key、占位符、换行和变量完全对称；术语修复不能破坏 `zh-CN`/`en-US` 的结构一致性。

### P0：纠正中文应用文案

1. `shell.js`：
   - `nav.sdk` 改为 `SDK 知识库`；
   - `action.aiConfig`、`aiConfig.title`、`state.waitingAiConfig` 及相关状态统一改为 `AI 配置` 语义；
   - `ai.*`、`settings.aiInterview.*`、风格对话中的“智能助手”统一审查为 `AI`；
   - `common.nodeId` 改为 `节点 ID`；
   - `format.markdown`、`format.json` 改为 `Markdown`、`JSON`；
   - `sdk.*` 全部将“开发工具包”改为 `SDK`，`sourceUrl` 使用 `来源 URL`。
2. `settings.js`：
   - 三个 AI 配置类别统一为 `开发 API`、`图像生成 API`、`文本补全 API`；
   - `apiUrl`、`apiKey` 统一为 `API URL`、`API Key`；
   - Codex/Claude 类型统一使用 `CLI`，例如 `本地 Codex CLI`、`本地 Claude CLI`；
   - OpenAI、自定义、Stable Diffusion WebUI 等类型保留产品名和 API/CLI 术语；
   - `entryId`、节点字段和相关校验信息使用 `ID`，不再使用泛化的“标识”；
   - `settings.aiInterview.*`、角色和 style prompt 对话统一审查 `AI`、`Prompt` 的显示方式。
3. `utility.js`：
   - `utility.sdk.*` 的状态、表格、验证文案统一使用 `SDK`；
   - `utility.save.*` 只在涉及技术 ID 的字段中将“编号”改为 `ID`，不改变“存档”这一用户熟悉的产品术语。
4. `pipeline.js`：
   - `pipeline.detail.aiAdapter` 改为 `AI Adapter：由后端配置决定` 或等价的简洁混合表达；
   - 对 `Prompt`、`Token`、`ID` 等技术词做同一规则审查，不把普通流程词批量替换成英文。
5. `index.html`：
   - 更新与上述 key 对应的静态中文初始内容、placeholder 和 aria-label，使首次渲染文案与 `zh-CN` catalog 一致；
   - 保留 `data-i18n`、`data-i18n-placeholder`、`data-i18n-aria-label` 结构，不通过修改 HTML 绕过翻译服务。

### P1：审查生成内容和英文目录

1. `design-content.generated.js` 是生成文件，不能直接手工修改；如技术术语扫描命中，应修改 `knowledge/design_data` 源数据或 `generate-design-content.mjs` 的术语处理，再重新生成。
2. 对设计内容只修复明确的标准技术词命中，例如 SDK、AI、API、UI、UX、JSON、NPC、DLC；不因中文表达风格不同而批量重译业务领域内容。
3. 英文目录保持 `AI`、`SDK`、`API`、`CLI`、`URL`、`ID` 的标准大小写，不将它们扩写成 `Artificial Intelligence` 或其他冗长表达。
4. 检查英文中的 `AI configuration`、`SDK Knowledge Base`、`API URL`、`API key`、`Prompt` 等大小写和搭配一致，但不为了中英文逐字对齐而改动自然的英文句子。

### P1：补充自动检查，防止回归

1. 在 `i18n-test.mjs` 或独立的 localization lint 中加入中文技术术语规则：
   - 禁止指定 key/场景出现“开发工具包”替代 SDK；
   - 禁止 AI 配置和 AI 状态标题出现“智能助手”或“人工智能”泛化写法；
   - 禁止配置类型中用“开发接口/图像生成接口/文本补全接口”替代 API；
   - 禁止 API 字段出现“接口地址/接口密钥”；
   - 禁止格式枚举使用“标记文本/结构化数据”替代 Markdown/JSON；
   - 禁止技术 ID 字段使用“标识/编号”替代 ID。
2. 规则应限定在明确的技术 key 和字段场景，避免把普通说明文案中的“接口”“提示词”误判为错误。
3. 加入术语表双向检查：同一个 key 在中文和英文中表达的技术对象必须一致，且关键 token 的大小写稳定。
4. 保留现有 key 对称、占位符、英文无汉字、HTML key 完整性和双语言 language/UI gate，不以新规则替代旧检查。

## 建议修订对照表

| 当前中文 | 建议中文 | 适用范围 |
| --- | --- | --- |
| 开发工具包 | SDK | SDK 导航、字段、状态、表格、上下文 |
| 开发工具包知识库 | SDK 知识库 | 导航和页面标题 |
| 智能助手配置 | AI 配置 | 按钮、标题、状态、空状态 |
| 智能访谈 | AI 访谈 | AI interview 页面和状态 |
| 智能助手 | AI | AI 角色、状态和对话标签 |
| 开发接口 | 开发 API | AI 配置类别和类型 |
| 图像生成接口 | 图像生成 API | AI 配置类别和类型 |
| 文本补全接口 | 文本补全 API | AI 配置类别和类型 |
| 接口地址 | API URL | 配置字段 |
| 接口密钥 | API Key | 配置字段 |
| 本地 Codex 命令行 | 本地 Codex CLI | CLI 配置类型 |
| 命令行状态 | CLI 状态 | CLI 诊断状态 |
| 人工智能适配器 | AI Adapter / AI 适配器 | 流水线技术状态 |
| 来源网址 | 来源 URL | SDK/API 来源字段 |
| 配置项标识 | 配置项 ID | 配置字段和校验错误 |
| 节点标识 | 节点 ID | 节点输入和诊断 |
| 存档编号 | 存档 ID | 技术详情字段 |
| 标记文本 | Markdown | 格式枚举 |
| 结构化数据 | JSON | 格式枚举 |
| 提示词（技术字段） | Prompt / 图像 Prompt | Prompt 编辑器标题和字段 |

## 验证与验收

### 静态验证

- `zh-CN` 和 `en-US` key 集合、占位符、换行和变量完全一致。
- 关键技术词在中文界面按术语表出现：SDK、AI、API、CLI、URL、Key、ID、Markdown、JSON、TOML、Prompt。
- 关键禁止词只在明确技术场景中清零：开发工具包、智能助手配置、人工智能适配器、开发接口、图像生成接口、文本补全接口、接口地址、接口密钥、标记文本、结构化数据。
- 生成的 `design-content.generated.js` 通过 `design-content` check，不能出现手工编辑造成的 stale 文件。

### UI 验收

- 中文主导航显示 `SDK 知识库`，不显示“开发工具包知识库”。
- AI 配置窗口标题、三个类别、空状态、状态栏和类型列表使用统一的 `AI/API/CLI` 混合术语。
- SDK 页面字段、按钮、表格、状态和上下文使用 `SDK`，不会同屏出现“开发工具包”和“SDK”两套称呼。
- API URL、API Key、Entry ID/Node ID 等配置字段可以直接与配置 JSON、日志和 Python 项目字段对应。
- 英文页面保持标准技术缩写和大小写，不出现不必要的全称扩写。
- 窄屏、弹窗、placeholder、aria-label、状态栏和截图基线中的术语保持一致。

### 回归验证

- 运行现有 `npm test`、`npm run e2e`、`npm run i18n-test`、`npm run language-gate` 和 UI gate。
- 验证 AI 配置保存、当前项切换、API Key 脱敏、SDK 新增/审批/拒绝、设计导出格式和流水线 AI 状态没有协议或行为变化。
- 抽查中文和英文启动 smoke；确认只改变展示文案，没有改变命令、配置字段、状态值、文件路径和 AI 请求 payload。

## 明确不做

- 不重命名 translation key，不修改 Rust/Python/Tauri 合约，不修改 AI 配置 schema。
- 不修改 API、SDK、CLI 的实际运行逻辑，不把 UI 术语修复误做成 Provider/Adapter 架构重构。
- 不把所有中文产品词强行改成英文；“流水线”“存档”“工作台”“补充开发”“打包”等保留自然中文，除非另有独立问题。
- 不直接编辑 `design-content.generated.js`；生成内容必须修改源数据或生成规则后再生成。
- 在用户当前“只写修复计划”的要求下，本记录不执行上述代码、语言包或测试修改。

## 后续事项

- 本计划实施后应将术语表作为后续新页面和新 key 的评审门槛，避免每次新增文案重新出现“SDK/AI/API 被展开翻译”的问题。
- 如果后续需要将 `Prompt`、`Provider`、`Model`、`Token` 全部固定为中英混合形式，应先以实际页面截图和用户习惯确认，不在本次修订中扩大范围。

## 来源

- `NEWrust/web/src/locales/shell.js`
- `NEWrust/web/src/locales/settings.js`
- `NEWrust/web/src/locales/utility.js`
- `NEWrust/web/src/locales/pipeline.js`
- `NEWrust/web/src/locales/design.js`
- `NEWrust/web/src/locales/design-content.generated.js`
- `NEWrust/web/src/index.html`
- `NEWrust/web/src/i18n.js`
- `NEWrust/web/scripts/i18n-test.mjs`
- `NEWrust/web/scripts/generate-design-content.mjs`
- `knowledge/ai_memory/session_history/2026-07-10-002.md`
- `plan/fixplan/2026-07-10_002_ai-config-current-entry-ui.md`
- `plan/fixplan/2026-07-10_003_ai-config-parity-and-provider-design.md`
