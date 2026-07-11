# 场景化技术术语表

原则：技术字段直接对应代码、配置与日志 token；普通产品说明保留自然中文。禁止无上下文全局替换。

| 标准 token | 技术场景允许写法 | 技术场景禁用写法 | 普通说明例外 | 主要 key/prefix | 大小写 |
| --- | --- | --- | --- | --- | --- |
| AI | `AI 配置`、`AI Adapter` | 人工智能（作为控件/字段技术名）、智能助手（作为配置技术名） | 面向用户的“智能助手提问/回答”可保留 | `action.aiConfig`、`settings.aiConfig.status.*`、`pipeline.detail.aiAdapter` | 固定大写 |
| API | `OpenAI API`、`自定义 API` | 把 API 类型写成“接口” | 普通句子“调用后端接口失败”可用接口 | `enum.aiConfigType.*_api` | 固定大写 |
| CLI | `本地 Codex CLI`、`Claude CLI` | 命令行（作为配置类型名） | “在命令行运行以下命令”是自然说明 | `enum.aiConfigType.*cli*`、`settings.aiConfig.cli.*` | 固定大写 |
| API URL | `API URL` | 接口地址、API Url | 普通网页“来源网址”不改成 URL | `settings.aiConfig.field.apiUrl` | API/URL 大写 |
| API Key | `API Key` | 接口密钥、API密钥、api key | 非技术安全说明可说“密钥”，但不能展示值 | `settings.aiConfig.field.apiKey` | 精确大小写 |
| ID | `配置项 ID`、`节点 ID` | 技术字段中的“标识/编号/Id” | 存档编号、订单编号等产品概念保留“编号” | `*.field.entryId`、`common.nodeId`、明确 `.id` 技术字段 | 固定大写 |
| SDK | `SDK`、`SDK 知识库` | 开发工具包（作为模块名） | 解释性正文可首次写“软件开发工具包（SDK）” | `nav.sdk`、`utility.sdk.*`、`sdk.*` | 固定大写 |
| Markdown | `Markdown` | 标记文本、Markdown 文本（若字段只表示格式名） | 普通“标记”动词不受影响 | `format.markdown`、`enum.export_format.markdown` | 首字母大写 |
| JSON | `JSON` | 结构化数据（若字段只表示 JSON 格式） | 泛指结构化数据时保留 | `format.json`、`enum.export_format.json`、`*extraJson*` | 固定大写 |
| URL | `URL` | 技术字段中的网址/Url | “来源网址”作为自然产品标签可保留 | `*sourceUrl*`、`*apiUrl*` | 固定大写 |
| Adapter | `AI Adapter` 或中文说明中的 `Adapter` | 技术详情写“适配器”但无法对应代码枚举 | 面向非技术用户的解释正文可写“适配器” | `pipeline.detail.aiAdapter`、resolution preview | 首字母大写 |
| Prompt | `Prompt`（明确协议/字段） | 把普通文案全部机械替成 Prompt | 风格提示词、提示词编辑等用户创作语境保留中文 | `format.prompt`、协议名/字段名 | 首字母大写 |
| Token | `Token`（模型计量/协议 token） | 技术计量字段写“令牌”导致歧义 | 登录令牌、安全令牌等中文安全语境可保留 | 模型用量、placeholder token 的明确技术 key | 首字母大写 |

## 扫描分级

1. 规则只在列出的 exact key、key pattern 或 prefix 下生效。
2. `fixtures/terminology` 的正反例用于防止“接口/提示词/编号”等自然中文被误报。
3. 旧命中清零并完成人工审查后，扫描已切为默认阻断；`--strict` 仍可用于显式表达阻断意图。
4. generated 内容必须修改来源或生成器，不能直接编辑 `design-content.generated.js`。
