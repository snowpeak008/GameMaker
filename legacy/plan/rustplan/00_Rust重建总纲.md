# AutoDesignMaker Rust 重建总纲

> 日期：2026-07-05
> 目标目录：`RUST/`
> 基本决策：当前 Python 项目仅作为参考项目，Rust 版是完整替代实现。

---

## 一、最终目标

Rust 版 AutoDesignMaker 的目标不是把 Python 代码逐行翻译成 Rust，而是在 `RUST/` 下重建一个完整的 **游戏设计与开发工作台**。

核心功能必须覆盖：

1. 游戏设计工作台
2. 游戏开发流水线
3. 游戏打包阶段
4. 游戏 SDK 知识库与接入辅助
5. AI 介入式生成、补全、评分和审查
6. 多项目并行打开
7. 项目正式存档、临时工作区、存档锁
8. 长任务运行、停止、恢复、日志、验证

Rust 版的交付形态：

- 双击打开桌面软件。
- 每次双击启动一个独立进程。
- 每个进程创建一个独立临时工作区。
- 不同正式存档可以被不同进程并行打开。
- 同一个正式存档同一时间只能被一个会话编辑。
- 内部正式存档使用目录式权威格式。
- 对外提供单文件项目导出包，用于备份、分享、转移。

---

## 二、已确认的不可逆决策

### 1. 完整 Rust 重建

结论：不保留 Python 运行时代码。

允许：

- 阅读 Python 代码，理解现有行为。
- 从 Python 项目提炼业务流程、边界、失败案例。
- 复用经过重新审查的知识内容、提示词思想、模板内容。

禁止：

- 在 Rust 版中长期保留 Python 兼容层。
- 使用 PyO3、Python subprocess、内嵌 Python 解释器作为核心运行机制。
- 把 Python 模块结构当作 Rust 模块结构。
- 用 Python 测试通过作为 Rust 正确性的证明。

### 2. 单 Git 仓库，`RUST/` 下 Cargo workspace

结论：不拆多个 Git 仓库。

Rust 项目结构必须放在：

```text
RUST/
  Cargo.toml
  crates/
  apps/
  resources/
  tests/
  docs/
```

`RUST/` 是 Rust 新产品的边界。旧 `core/`、`pipeline/`、`tools/`、`settings/`、`saves/`、`drafts/` 不作为 Rust 运行目录。

### 3. 框架优先

结论：先写底层骨架，后写业务链条。

第一阶段即使界面看不到完整业务，也必须先完成：

- crate 边界
- 错误模型
- 日志模型
- 配置模型
- 路径模型
- 项目会话模型
- 临时工作区模型
- 正式存档模型
- 存档锁模型
- 任务运行器
- 事件记录
- 测试框架
- AI Provider 抽象
- 流水线抽象

业务功能必须插到框架里，不能反过来让业务代码临时定义运行规则。

### 4. GUI 技术方向

结论：纯 Rust 桌面方向，优先验证 Slint。

要求：

- 不使用 Tkinter。
- 不使用 Python GUI。
- 不把业务规则写进 UI。
- GUI 调用 Rust application service。
- Slint 必须早期验证真实复杂界面能力。

Slint POC 必须覆盖：

- 项目启动页
- 存档管理
- 设计工作台三栏布局
- 流水线步骤列表
- 长日志面板
- AI 配置面板
- SDK 知识库列表
- 弹窗和确认流程

如果 POC 证明 Slint 不适合，再单独形成替代 ADR；在此之前计划按 Slint 设计。

### 5. 存档模型

结论：保留业务语义，重设文件格式。

保留：

- 临时工作区
- 正式存档
- 存档锁
- 显式保存
- 多进程多项目并行

不保留：

- Python 的 `saves/{save_id}/manifest.json` 具体格式
- Python 的 `drafts/{session_id}/draft_meta.json` 具体格式
- Python 的旧 runtime_control 文件格式
- Python 的执行对象 JSON 格式

### 6. 流水线模型

结论：保留阶段化流水线，但重设阶段边界。

Rust 版不强制沿用：

- D1-D4
- Step00-14
- Python plugin 目录
- `_registry.json`
- 旧 artifact_layer 结构

Rust 版必须保留：

- 阶段
- 依赖
- 门禁
- 产物
- 验证
- 运行状态
- 失败原因
- 停止/恢复
- AI 介入点

旧 Step 只作为参考映射，不作为强制结构。

### 7. 同项目流水线第一版串行

结论：同一个项目内部第一版串行执行。

允许并行：

- 多个软件进程
- 多个临时工作区
- 多个不同正式存档

第一版不做：

- 同一项目多个阶段并发
- 同一阶段多个任务并发写同一个项目
- Unity 项目写入并发
- 同一正式存档多窗口编辑

### 8. AI 是核心能力，但按需介入

结论：AI 不可缺席，但不能无条件常驻。

AI 介入场景：

- 内容不足
- 评分不足
- 用户要求生成
- 用户要求补全
- 阶段需要审查
- 生成结果需要验证
- 代码/资产/SDK 内容需要辅助

AI 输出必须经过：

- 结构校验
- 质量评分
- 规则检查
- 人工确认或阶段门禁
- 失败记录

### 9. AI Provider 可插拔

Rust 版必须按能力抽象 Provider：

- 文本生成
- 结构化输出
- 评分审查
- 代码生成
- 图像生成
- SDK 检索与解释
- 长任务代理执行

业务代码不能绑定具体供应商。

---

## 三、Rust 总体架构

推荐 workspace：

```text
RUST/
  Cargo.toml
  apps/
    adm-desktop/          # Slint 桌面程序入口
    adm-cli/              # CLI 入口，供测试和自动化调用
  crates/
    adm-foundation/       # 基础类型、错误、时间、ID、路径、校验工具
    adm-config/           # 配置加载、配置校验、机密信息引用
    adm-archive/          # 临时工作区、正式存档、存档锁、导入导出包
    adm-runtime/          # 会话、任务运行器、事件、日志、停止/恢复
    adm-ai/               # AI Provider 抽象、任务契约、调用记录
    adm-pipeline/         # 阶段、依赖图、门禁、产物、流水线执行
    adm-design/           # 游戏设计领域模型、模板、评分、设计状态
    adm-development/      # 开发任务、代码任务、项目写入计划
    adm-assets/           # 美术、音频、资产任务和资产验收
    adm-packaging/        # 打包阶段、交付资料、包清单
    adm-sdk/              # SDK 知识库、检索、集成建议
    adm-ui-model/         # UI 可绑定状态模型，不含 Slint 组件
    adm-validation/       # schema/contract/quality 验证框架
    adm-testkit/          # 测试辅助、fixture、临时目录、模拟 Provider
  resources/
    design_data/
    prompts/
    schemas/
    sdk/
  docs/
    architecture/
    decisions/
    user-flows/
  tests/
    integration/
    fixtures/
```

### crate 职责总览

#### `adm-foundation`

必须提供：

- `ProjectId`
- `SessionId`
- `ArchiveId`
- `RunId`
- `TaskId`
- `StageId`
- `ArtifactId`
- `AdmError`
- `AdmResult<T>`
- 时间戳工具
- 内容 hash 工具
- 安全路径工具
- 原子写文件工具
- JSON/TOML 读写封装

不得包含：

- GUI
- AI 调用
- 业务阶段逻辑
- 存档目录策略

#### `adm-archive`

必须提供：

- 创建临时工作区
- 打开正式存档到临时工作区
- 保存临时工作区到正式存档
- 正式存档锁
- 存档心跳
- 崩溃恢复
- 导出单文件项目包
- 导入项目包
- 存档目录版本
- 内容指纹
- 保存事务

硬规则：

- GUI 和 pipeline 不能直接写正式存档。
- 只有 `adm-archive` 可以提交正式保存。
- 同一个正式存档同一时间只能有一个编辑锁。

#### `adm-runtime`

必须提供：

- 应用会话
- 当前临时工作区绑定
- 任务运行器
- 任务状态机
- 停止请求
- 运行事件
- 结构化日志
- 长任务输出流
- 后台任务错误边界

第一版只支持同一项目内部串行任务。

#### `adm-ai`

必须提供：

- Provider trait
- 能力枚举
- AI task request
- AI task result
- AI 调用记录
- token/成本/耗时记录
- 重试策略
- 结构化输出校验入口
- mock provider

不得让业务层直接调用具体 HTTP API。

#### `adm-pipeline`

必须提供：

- stage 定义
- stage dependency graph
- stage gate
- stage artifact contract
- pipeline run state
- serial runner
- resume policy
- failure report
- AI intervention hook
- validation hook

Rust 版阶段不继承旧编号，只保留旧步骤参考映射。

#### `adm-design`

必须提供：

- 游戏项目设计状态
- 设计领域
- 设计节点
- 设计决策
- 质量评分
- 内容完整度
- 模板加载
- AI 介入触发条件

它不应该知道 GUI 控件，也不应该知道文件锁。

#### `adm-development`

必须提供：

- 开发任务模型
- 开发计划模型
- 代码生成任务契约
- 项目写入计划
- 变更审查
- 可回滚操作描述

不直接执行 AI；只声明需要何种 AI 能力。

#### `adm-assets`

必须提供：

- 美术任务模型
- 音频任务模型
- 资产需求
- 资产生成请求
- 资产验收
- Unity/引擎导入计划

不在第一阶段实现真实图像生成，只建契约和状态。

#### `adm-packaging`

必须提供：

- 打包前置检查
- 打包资料生成
- 包清单
- 交付检查
- 导出记录

它读取项目状态和阶段产物，不直接修改设计状态。

#### `adm-sdk`

必须提供：

- SDK 条目模型
- SDK 文档索引
- SDK 集成建议
- SDK 风险项
- AI 解释入口

SDK 内容是产品核心功能，不是附属文档。

#### `adm-ui-model`

必须提供：

- GUI 可绑定的 view model
- 命令输入模型
- 状态摘要
- 错误展示模型

不得包含业务规则。

---

## 四、运行时目录建议

开发环境：

```text
RUST/.runtime/
  drafts/
  archives/
  exports/
  logs/
  cache/
```

生产环境：

```text
{user_data_dir}/AutoDesignMakerRust/
  drafts/
  archives/
  exports/
  logs/
  cache/
  config/
```

要求：

- 测试只能写临时目录。
- 不允许测试污染真实用户目录。
- 不允许硬编码 `E:\workwork\...`。
- 所有路径必须经过 `adm-foundation` 或 `adm-archive`。

---

## 五、计划文件索引

本目录文档：

- `00_Rust重建总纲.md`：总目标、决策、架构。
- `01_迁移与重写矩阵.md`：哪些可参考、可迁移、必须重写。
- `02_框架优先开发计划.md`：分阶段开发计划。
- `03_开发注意事项.md`：开发红线、并发、AI、数据安全、测试注意事项。
- `04_开发保证书.md`：工程承诺和验收标准。

