# A07 流水线 Step00–06

状态：已完成（本地，待验收提交）
依赖：A06、R1-C0

## 目标

把设计阶段改为冻结规格驱动的确定性编译链，并交付贯穿 A08 全系列的通道防守冻结规格 fixture。

## 变更

1. 为 Step00–06 定义版本化输入/输出契约和纯编译器。
2. 门禁链：意图无矛盾（01）、能力激活理由完整（02）、核心循环可追踪（03）、引用/迁移/资源闭合（04）、每承诺至少一正一负场景（05）、冻结哈希稳定 + **ProductEnvelope 判定**（06，超包络即拒绝）。
3. 每个产物写入源规格哈希、编译器版本、追踪引用。
4. 禁止从旧目录猜测输入、空模板静默成功。
5. **交付通道防守冻结规格 fixture**：以 R1-C0 宪章的 AcceptanceScenario 草稿为种子，产出完整冻结 GameSpec，入库 `testdata/`，作为 A08a–A08f 的统一测试输入。
6. 每个编译器执行标签置换 + 能力扰动测试。

## 验收（停止门）

- 相同冻结规格重复执行得到相同语义哈希。
- 任一上游错误在最早步骤阻塞。
- 超包络规格在 Step06 被确定性拒绝。
- 通道防守 fixture 通过全部 00–06 门禁并冻结。

## 回滚

新编译链在 `game_spec_v2` 开关后；关闭开关回旧行为。

## 实际结果

- 新增 `adm-new-pipeline::game_spec_v2_steps`，提供 Step00–06 的版本化纯编译器、门禁报告、语义哈希冻结、产物落盘。
- Step00–06 依次执行输入契约、意图矛盾、能力理由、核心循环追踪、引用/资源闭合、正负验收场景覆盖、ProductEnvelope + 冻结哈希门禁；任一门禁失败即早停。
- 每个 step 报告包含 `sourceHash`、`compilerVersion`、`traceRefs` 和反过拟合证据（标签置换状态稳定、能力扰动哈希变化）。
- 新增 `NEWrust/testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json`，以 R1-C0 签署场景为种子，包含 12 条验收场景，作为 A08a–A08f 统一输入。

## 验证

- `cargo fmt --all`
- `cargo test -p adm-new-pipeline game_spec_v2_steps`
- `cargo test -p adm-new-game-spec fixtures`
