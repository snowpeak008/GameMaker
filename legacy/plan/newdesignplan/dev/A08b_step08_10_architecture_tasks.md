# A08b Step08–10 架构、资产清单与可信任务合同

状态：已完成（本地，待验收提交）
依赖：A07

## 目标

Step08–10 编译运行时架构、冻结 AssetManifest、产出携带机器可执行验收检查的可信任务图。

## 变更

1. **Step08 架构编译**：系统边界来自能力画像，不由类型标签决定；执行标签置换 + 能力扰动测试。
2. **Step09 AssetManifest 冻结**：每项资产有用途、规格（尺寸/格式/切片）、预算档位、依赖和验收标准；引用 Step07 风格锚点。冻结后 Step12 才批量生产。
3. **Step10 任务图**：每个任务必须携带完整合同——
   - 声明写入路径集合；
   - **机器可执行的验收检查**：编译目标 + 任务专属受信测试（Step10 生成，不靠 Step11 的 AI 自证；受信测试以基线树哈希防篡改）;
   - 依赖关系（修复现状 `dependencies: []` 的空依赖问题）；
   - 回滚边界。
4. 任务规模用序数档位（S/M/L），不用精确数量上限。
5. 产物写入源规格哈希与追踪引用。

## 验收（停止门）

- 通道防守 fixture 产出架构、冻结 AssetManifest 和任务图；每个任务合同四要素齐全。
- 任务依赖图无环且与架构系统边界一致。
- 受信测试可独立执行且树哈希可验证。
- 重复编译语义哈希一致。

## 回滚

新编译器在 v2 开关后；旧 step08_14.rs 路径不动。

## 实际结果

- 新增 `adm-new-pipeline::stages::step08_10_v2`，从冻结 GameSpec 和 Step07 锚点编译 `runtime_architecture.json`、`frozen_asset_manifest.json`、`trusted_task_graph.json`。
- Step08 架构系统边界由能力画像、空间/内容/信息/进度能力和验收场景推导；附带标签置换稳定与能力扰动变化证据。
- Step09 冻结 AssetManifest，每项资产记录用途、格式、尺寸、切片、预算档位、依赖、验收标准、source refs、style anchor refs。
- Step10 生成可信任务图，任务含声明写入路径、机器验收检查、非空依赖、回滚边界与 `WorkspaceChangeSet` 合同；所有合同通过 A04 验证器。
- 任务图验证包含无环检查、未知依赖检查和合同问题聚合。

## 验证

- `cargo fmt --all`
- `cargo test -p adm-new-pipeline step08_10_v2`
