# 009 前端初始加载错误状态

## 目标

消除“后端读取失败等同于空项目/等待数据”的错误语义，并统一设计工作台、AI 访谈及工具面板的初始失败表现。

## 原子工作

1. 删除 `createDesignApi.load`、`createAiInterviewApi.load` 和工具只读 API 的全捕获 `null` 降级。
2. `DesignWorkbenchController.reload` 捕获错误并生成带 `loadError` 的空视图；渲染状态显示失败详情，而不是 waiting。
3. `initAiInterviewPanel` 将加载错误映射到现有 `lastError` 状态，不改变访谈命令和消息结构。
4. 为 patch/package/logs/sdk 的渲染入口增加统一 `__loadError` 输入约定；存档继续沿用现有约定。
5. 新增中英文加载失败文本，运行时错误原文按现有 content-origin 规则展示。
6. 增加单元测试，覆盖 API 拒绝、控制器错误模型、错误状态优先级及正常空数据不被误判。

## 验收

- 命令不可用、后端返回 `ok=false`、读取损坏或 IPC 拒绝时显示失败状态。
- 后端成功返回合法空集合时仍显示正常空状态。
- 流水线既有失败呈现保持不变。

## 实施结果（2026-07-14）

状态：**已完成**。设计工作台提供双语失败状态和重试按钮；AI 访谈使用现有 `lastError` 呈现失败；patch/package/logs/sdk/save 均保留加载错误。Web 单元测试覆盖 API 拒绝、控制器错误模型与合法空数据。
