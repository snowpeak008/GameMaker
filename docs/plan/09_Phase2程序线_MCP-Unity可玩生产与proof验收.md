# V4 · Phase 2 · 09 · 程序线：MCP 控制 Unity 可玩生产与 proof 验收

> 立项分册 09/10。总纲见 `06`，治理见 `07`，美术线见 `08`。本册是**程序线 + 可玩生产 + 验收**：
> 采纳 godogen（`build/01-03`）的 proof 驱动模型，用户定的 **MCP 控制 Unity 现场开发**替换旧的
> batchmode 生成 EXE。每处对既有设计的改动标「【优化】」。

---

## 1. 生产模型：现场驱动，不是离线生成

> **从冻结设计抽取最小可玩切片 → 渲染薄运行时说明 + Unity 引擎指南到隔离工程 →
> AI 经 MCP 现场控制 Unity 开发 → 捕获运行 proof → verdict → 缺陷回写修正队列。**

对照旧 batchmode 模型的三处根本改变：
| | 旧（弃） | 新（本册） |
|--|---------|-----------|
| 驱动 | 生成 C# 文件 → batchmode 构建 | **MCP 现场控制 Unity**（建场景/挂脚本/改预制体/跑 PlayMode），agent 在隔离工程 cwd 工作 |
| 完成标准 | 编译/测试通过 | **运行画面/视频 proof**（proof-over-claims，D21）|
| 上下文 | 一次性大契约 | **薄 runtime manifest + 一页引擎指南 + durable docs** |
| 顺序 | 六步全做 | **风险切片先行**（先打通最不确定的可玩核心）|

---

## 2. 可玩切片抽取（≈ godogen L3/L4 · 确定性）

从冻结 GameSpec 抽最小可玩目标，**不塞完整商业范围**：
```json
playable_slice: {
  "core_loop": "", "primary_input": [], "player_feedback": [],
  "scene": "", "success_or_fail_state": "", "excluded_scope": []
}
risk_slice_plan: [ { risk: "手感|画面|技术|资产", verify_by: "命令|截图|视频" } ]
```
- 检查：不超过 1 主场景 / 1 主操作 / 1 成败反馈；每个风险有独立验证方式；
- 抽取源 = 程序线 capability_contracts + GameSpec；抽取失败要可解释（R2）。

---

## 3. 薄运行时 + Unity 引擎指南（高信号，只写坑）

- **`runtime_manifest`**（极短）：目标、durable 状态文件、读引擎指南、proof 要求、失败修复循环；
- **`engine_guide`（Unity 一页）**：只写"模型容易错、编译不一定发现、运行才暴露"的坑 + 构建/运行/捕获命令。Unity 侧重点：.NET/C# partial、场景/预制体 API、**silent failure**（如打包保存静默失败）、PlayMode 进入/退出、**截帧/录屏方式**；
- **durable docs**（抵抗上下文丢失，可断点续跑）：`PLAYABLE_PLAN.md` / `PLAYABLE_STRUCTURE.md` / `PLAYABLE_ASSETS.md`（= 资产表，接 AssetGenome）/ `PLAYABLE_PROOF.md`。

**优化【指南按引擎注入，不混大 prompt】**：本期只 Unity 一份指南；其他引擎指南留空接缝（D17）。

---

## 4. MCP 控制 Unity 现场开发（用户第 1 点的落法）

**`EngineBackend` → `UnityMcpBackend`**：通过 MCP server 驱动 Unity Editor 现场开发（不是生成文件后 batchmode）。

```rust
pub trait EngineBackend {
    fn id(&self) -> &str;                                  // "unity-mcp"
    fn open_or_create_project(&self, seed, dir) -> Adm4Result<()>;
    fn agent_develop(&self, task: &SliceTask, ctx: &DevContext) -> Adm4Result<DevRound>;  // 经 MCP 现场操作
    fn run_playmode(&self, project) -> Adm4Result<RunResult>;        // 跑起来
    fn capture_proof(&self, project) -> Adm4Result<ProofBundle>;     // 截帧/录屏
}
```
- agent 的 `cwd` = 隔离目标 Unity 工程（与 AutoDesignMaker 仓库隔离，靠 manifest/proof 契约交换数据）；
- **现场操作经 MCP 工具**：创建 GameObject/场景、挂脚本、配预制体、填数据、进 PlayMode——这正是"MCP 控制 Unity 开发游戏"；
- 每轮记录命令/失败/修复摘要（durable，可停可续）；
- 其他引擎 backend 留接缝（D17）。

> **未决 Q2/Q3（总纲 §5）**：用哪个 Unity MCP server（现成方案/自建）、它能否跑 PlayMode 并截帧/录屏——决定本册可行性，G4 前必定。若 MCP 无法捕获，诚实降级 `blocked_by_environment`，不伪装成功（R7）。

---

## 5. proof 验收（proof-over-claims · 你第 4 点的答案）

### 5.1 proof bundle（运行事实）
```
proof/
  build_log.txt / run_log.txt
  screenshots/
  video.mp4
  proof_review.json   // 可见问题、覆盖的核心玩法、未覆盖项、verdict
  repair_queue.json   // 缺陷映射回设计/程序/美术/环境层
```

### 5.2 机器预检（谁验之一：机器）
proof 通过的机器条件（不满足即不算通过）：
- 游戏可启动；核心操作在视频中**可见**；关键资产存在并被**运行时加载**；
- 截图/视频**非空白、非单帧循环、能体现过程变化**；
- 捕获失败必须有明确机器/引擎原因 + 降级证据，**不能伪装成功**（R7、R1 真实证据非恒真）。

### 5.3 用户 verdict（谁验之二：人）
机器预检过后，用户在 proof 门做最终裁决（人看画面定，主观质量只有人能定）：
- `pass`：核心目标可见且可玩；
- `pass_with_warnings`：可玩但有轻微视觉/体验问题；
- `needs_repair`：核心玩法或画面 proof 不成立；
- `blocked_by_environment`：环境缺失，无法判断游戏质量。

### 5.4 谁来验、用什么标准（你第 4 点的完整回答）
| 问题 | 答案 |
|------|------|
| agent 验 / skill 验 / 专门标准？ | **三者组合**：机器预检（确定性代码，非 AI 打分）+ 用户 verdict（人看画面）+ `proof_review.json` 是标准格式 |
| 谁来验？ | 机器做客观预检（非空白/核心动作可见/资产加载），人做主观 verdict |
| asset-gen 是什么？ | **skill**（册 08 的 AssetProducer）|
| 为什么不用 AI 打分验收？ | 避免 AI"自己说做好了"——**以运行画面事实为准**，AI 不做自我验收（接 v3"AI 不能自判通过"）|

---

## 6. 缺陷回写（proof 反哺 · ≈ py Step14 → 修正队列）

`repair_queue` 每条缺陷映射到**归属层**，回写对应环节：
- 设计问题 → 设计工作台/冻结（改设计需新冻结版本）；
- 程序问题 → 程序线 capability/可玩切片；
- 美术问题 → 美术线风格门/资产生产（册 08）；
- 环境问题 → 配置/预检（Unity 路径/MCP/License）。

每个缺陷有归属层 + 处理建议，不丢证据，不覆盖用户未提交改动。

---

## 7. 红线映射（生产段）

| 红线 | 强制点 |
|------|--------|
| R1 指标即测量 | proof verdict/覆盖是真实运行证据，禁止恒真 |
| R2 未知即停 | 抽取失败/悬空引用/无法生产 → Blocked，不 stub |
| R3 评审工作量证明 | proof 门 verdict 署名 |
| R4 产出锚定 | 生成的场景/脚本/资产锚定 SpecRef/asset_id |
| R5 参考名扫描 | 代码/资产名/manifest 过 SkinScanner |
| R7 fallback 禁令 | 构建/运行/捕获失败 = Failed/blocked_by_environment，不伪装、不半成品 proof |

---

## 8. 本册落点（给册 10）

- `adm4-build/program/`：`slice.rs`（可玩切片抽取）、`manifest.rs`（runtime manifest 渲染）、`engine_guide.rs`；
- `adm4-build/engine/`：`EngineBackend` trait + `unity_mcp/`（MCP 驱动 Unity）；
- `adm4-build/proof/`：`bundle.rs`、`precheck.rs`（机器预检，确定性）、`verdict.rs`、`repair.rs`；
- `AppServices::build_run/build_status/build_confirm`（proof 门确认）；桌面「打包阶段」升级为可玩生产入口（仅 L6，接 proof 预览与 verdict）；
- 全部插件化（D24）；契约全 serde + `#[serde(default)]`。

> 优化诚实边界：**【优化/替换】** 旧稿的 P0-P5 batchmode + Unity Test Framework GWT 判定，被本册的"MCP 现场开发 + proof 画面验收"整体替换——这是用户第 1、4 点要求的方向改变，不是简化：proof 验收比"测试绿"更强（测试绿 ≠ 可玩 ≠ 画面对）。GWT 验收场景仍可作为 proof 的一项核对依据，但不再是唯一完成标准。
