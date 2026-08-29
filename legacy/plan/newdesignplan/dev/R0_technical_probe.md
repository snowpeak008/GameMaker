# R0 技术探针

状态：已完成（2026-07-15，本地实现，按用户要求待总验收后提交）
依赖：A01（与 A02/A03 并行执行；是 A04 的前置）
性质：风险验证，**不是游戏内容成果**

## 目标

在投入编译器建设之前，用最小成本走通最致命的链路，提前暴露 Step11 工具链、工作区提交、Unity 批处理和打包的风险。

```
手写固定最小 GameSpec → 现有内核生成一个场景 + 少量代码 → Unity 批处理编译 → smoke → EXE
```

## 变更

1. 手写一份最小通道防守 GameSpec JSON（占位内容，不要求真实美术和完整 D1–D4）。
2. 用**现有**基础设施执行少量代码生成任务：`work_unit.rs` 工作单元（声明输出 + 越界拒绝）、`adm-new-patch::CodexPatchRunner`、现有 Unity 批处理 preflight/验证。不新建抽象。
3. 走通 Unity 编译 → smoke → EXE 打包。
4. 产出两样东西：
   - **可重复运行的 harness**（脚本 + 固定输入 + 判定），后续演化为 A08c 执行引擎的回归测试床；
   - **失败分类清单**：探针过程暴露的失败类型（compile/test/scope_violation/timeout/agent_error 及未预见类别）、越界模式、批处理坑位，作为 A04 设计 ChangeKernel 的输入。

## 验收（停止门）

- 全链路至少一次端到端成功：最小规格 → 可启动 EXE（占位内容即可）。
- harness 可重复运行且判定确定。
- 失败分类清单文档交付并在 A04 任务文档中被引用。

## 回滚

探针代码与占位内容可整体删除；harness 保留在独立目录。

## 红线提醒

- R0 产物不得被当作 R1 内容起点；占位资产用完即弃。
- 不在 R0 中"顺手"重构三套存量实现——那是 A04/A08c 的事。

## 实际结果

- 可重复 harness 位于 `NEWrust/tools/r0-probe/`；固定输入严格通过 A02 验证和规范哈希。探针二进制是 `adm-new-cli` package 下的独立 `adm-new-r0-probe` bin target，未修改产品启动入口。
- 全链路真实使用 `CodexCliAdapter` 子进程协议、`CodexPatchRunner`、`WorkUnit`、Unity batchmode、BuildPipeline 和 Windows Player smoke；不是内存 stub。
- 故意越界负例被完整拒绝，项目变更数为 0；真实编译失败会进入现有 `recovery_blocked`，未伪造成功。
- 修复了 `work_unit.rs` 向 Unity 传递 Windows verbatim 路径的兼容缺陷，并添加本地路径/UNC 回归测试；该修复只发生在外部进程参数边界。
- 两次完整运行均通过，稳定指纹同为 `af63ea319e9183048a74c0861df263fb08f84e8cfaaa6a02dd0db6b95376212e`。最终 Windows Player 与 `Assembly-CSharp.dll` 均存在，玩家退出码为 0 并输出 smoke 标记。
- 失败分类与 A04 约束已写入 `../evidence/R0_failure_catalog.md`，并由 A04 原子计划显式引用。
- 因“总验收前不同步仓库”的约束，独立提交延后到总验收通过后执行。
