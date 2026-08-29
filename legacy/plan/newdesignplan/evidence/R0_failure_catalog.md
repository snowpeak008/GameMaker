# R0 失败分类与技术探针证据

日期：2026-07-15  
输入：`NEWrust/tools/r0-probe/minimal_game_spec.json`  
Harness：`NEWrust/tools/r0-probe/Invoke-R0Probe.ps1`  
运行证据：`NEWrust/target/r0-probe/evidence/r0-probe-report.json`（生成物，不入仓库）

## 稳定分类

| 类别 | 确定性判据 | 默认处置 | A04 合同含义 |
|---|---|---|---|
| `input` | 固定规格无法严格解析、验证或哈希 | 不重试，修正输入 | 变更进入内核前先验证 |
| `agent_error` | 适配器未启动、非成功退出或未交付声明输出 | 有界重试 | 记录适配器结果，不允许 AI 自证成功 |
| `scope_violation` | 隔离前后差异包含声明集合外路径 | 直接拒绝，不合入 | 写集合是硬边界；失败不得产生项目变更 |
| `compile` | Unity 日志出现编译错误确定性标记 | 修正代码后重试 | 编译证据属于变更集验收结果 |
| `test` | 编译成功但构建/smoke 标记、退出码或验收断言不成立 | 修正实现或测试合同 | 受信检查与实现写集合分离 |
| `timeout` | 适配器、Unity 或玩家超过任务时限 | 终止进程树后按预算处置 | 超时必须保留阶段和副作用状态 |
| `tooling` | 工具不可用、路径协议不兼容、进程无法启动/观察 | 修正本机绑定或工具边界 | 机器绑定不进入 GameSpec；内部路径与外部工具参数分层 |
| `evidence` | 声称成功但输出、哈希、日志或执行对象证据缺失 | fail closed | 无完整证据不得提交或升级状态 |

## 实际观测

1. **Windows verbatim 路径不兼容**：`std::fs::canonicalize` 产生的 `\\?\` 项目路径被 Unity `2022.3.62f3c1` 错误解释成 `/?/`，Package Manager 因此找不到已存在的 manifest。修复位于 `NEWrust/crates/adm-new-application/src/work_unit.rs`：内部继续使用规范路径做边界校验，仅在 Unity 进程边界转换为普通本地/UNC 表示，并有回归测试。
2. **空包项目的模块假设**：首版探针代码使用 IMGUI，真实 Unity 编译确认该空包项目未引用 IMGUI 模块并返回 `CS0103`。探针改用已存在的 `TextMesh` 模块；结论是代码任务合同必须以目标项目真实模块集编译，不能仅做文本或文件存在检查。
3. **越界写入**：负例适配器同时写入声明文件和未声明文件；`CodexPatchRunner` 拒绝整个结果，项目变更数为 0。`scope_violation` 不应重试同一提示。
4. **工具派生副作用**：Unity 导入声明的 `.cs` 时会生成 `.meta`。A04/A08c 必须区分“代理声明写入”与“受信工具派生写入”，两者都要进入证据，但不能把工具派生文件误判成代理越界。
5. **失败后的状态**：编译失败发生在代码已安全提交之后，现有 WorkUnit 正确进入 `recovery_blocked`，而不是伪装为无副作用失败。ChangeKernel 审计必须同时记录失败类别与副作用确定性。

## 成功证据

- 固定 GameSpec 规范哈希：`6a58f6c2db2c7ab8c6e681e09b4a328b2a40af1dd0d3dcf22819601153c39ee2`。
- 两次完整运行稳定指纹一致：`af63ea319e9183048a74c0861df263fb08f84e8cfaaa6a02dd0db6b95376212e`。
- 两次均通过六段判定：规格验证、补丁生成、越界拒绝、WorkUnit + Unity 编译、Windows 玩家构建、玩家 smoke。
- 最终 EXE 为真实 Windows Player（666,624 bytes），并存在 `Assembly-CSharp.dll`；玩家以退出码 0 输出 `ADM_R0_SMOKE_PASS`。

## A04 必须消费的约束

- 变更合同分别表达代理写集合、受信工具派生写集合和构建输出集合。
- 事务结果同时保存失败分类、是否已提交副作用、基础树哈希和验证证据。
- 工具调用使用本机路径绑定；权威内部路径不因兼容外部工具而降级。
- `scope_violation` 与证据缺失直接拒绝；`compile/test/agent_error/timeout` 由 ExecutionBudget 决定是否有界重试。
