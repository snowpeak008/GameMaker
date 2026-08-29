# NEWrust 内容设计规范

## 1. 产品内容边界

AutoDesignMaker 是游戏设计文档自动生成流水线。NEWrust 必须围绕内容质量重建，而不是围绕按钮数量重建。

核心内容面：

- 设计工作台：项目画像、领域、节点、L4 选项、L5 实体、风险、缺失项、摘要、校验。
- 开发流水线：Step00-14 typed stage contract。
- 补充开发：新增需求分析、影响范围、patch task、验证要求。
- 打包阶段：读取 Step14 和制品注册表，生成交付包。
- 运行日志：严格事件和上下文。
- SDK 知识库：候选、审核、批准、提示词上下文。

## 2. 内容优先级

内容质量优先级高于 UI 可见度：

1. 数据可追溯。
2. 语义与项目身份一致。
3. 每个阶段有下游消费者。
4. 每个输出有验收标准。
5. UI 只显示真实状态。

## 3. Step00-14 内容契约

每个阶段必须输出稳定章节：

- `Stage Identity`
- `Input Evidence`
- `Structured Content`
- `Decision Record`
- `Acceptance Criteria`
- `Downstream Contract`
- `Risk And Open Questions`
- `Provenance`

最低内容要求：

| Step | 内容重点 | 不可接受输出 |
| --- | --- | --- |
| Step00 | 创意收集、项目画像、受众、平台、商业模式 | 只有项目名或泛泛摘要 |
| Step01 | 玩法框架、核心循环、系统边界 | 只列关键词 |
| Step02 | 设计冻结、未决问题、风险承认 | 没有冻结原因 |
| Step03 | 程序能力、系统需求、验收探针 | 没有可开发任务 |
| Step04 | 美术需求、资产分类、风格风险 | 没有资产粒度 |
| Step05 | 程序评审、阻塞项、修正建议 | 只写 pass |
| Step06 | 美术评审、覆盖率、风格一致性 | 没有缺失资产 |
| Step07 | 风格候选、确认、重生成条件 | 没有人类确认记录 |
| Step08 | 程序计划、依赖、并行边界 | 任务不可执行 |
| Step09 | 美术计划、资产生产批次 | 无验收口径 |
| Step10 | 资源对齐、合同映射、缺口 | 未链接 Step03/04/08/09 |
| Step11 | 程序执行记录、状态、失败恢复 | 只写完成 |
| Step12 | 美术生产记录、交付路径 | 无素材状态 |
| Step13 | 场景组装、挂载关系、初始化 | 无运行场景 |
| Step14 | 集成验证、可玩验收、阻塞项 | 无真实/模拟证据分层 |

## 4. 内容质量门

每个 stage document 必须通过：

- identity coverage：项目名、类型、平台、核心体验一致。
- semantic coverage：内容覆盖该阶段的 required fields。
- downstream readiness：下游阶段能读取 typed contract。
- placeholder rejection：拒绝 TODO、placeholder、未命名、generic boilerplate。
- evidence level：标明 mock/static/local/real。

## 5. 语义一致性

所有内容必须由 `ProjectIdentity` 驱动：

- project_id
- project_name
- genre
- audience
- platform
- player_promise
- core_loop
- visual_direction
- technical_target

任何阶段输出与 `ProjectIdentity` 冲突时，必须产生 risk item，不能静默覆盖。

## 6. AI 内容规则

AI 可以参与：

- 补充候选内容。
- 生成解释性文本。
- 提供评审建议。

AI 不可以直接越权：

- 修改冻结合同。
- 写入 L5 实体而无 validator。
- 声称真实 Unity 或真实 provider 通过。
- 覆盖用户确认的风格选择。

所有 AI 写入必须记录：

- provider_id
- model
- prompt_hash
- input_contract_hash
- output_hash
- confidence
- validation_result

## 7. 人类确认点

必须保留人工确认：

- Step02 设计冻结。
- Step07 风格确认。
- Step10 资源缺口接受或修复。
- Step14 最终验收。
- 真实 Unity PlayMode evidence 接收。
- 真实 AI provider acceptance 接收。

