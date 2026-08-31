# V4 · Phase 2 立项设计（总纲）

> 状态：**立项稿 v2（2026-08-30 重写）**。v1（batchmode 生成 EXE 模型）已废弃——经用户裁定，
> 那是重瀑布，方向错误。本稿以本项目**三代已验证的成熟设计**为基准重写：
> Python v1 的两条线防漂移治理（`knowledge/governance/`）+ `build/01-03` 的 godogen proof 驱动模型
> + v3（NEWrust）的 GameSpec 单一真源 + 美术风格锚点（Step07）。
> **纪律（用户约束）**：优化但保留既有设计，只做代码重构 + 真内容优化，**不过度扩散、不过度简化**。
> 每处对既有设计的改动都在正文显式标注「【优化】」，未标注处即原样吸收。

---

## 0. 立项文档集导航（五册，逐册可评审）

| 册 | 文件 | 主题 | 吸收自 |
|----|------|------|--------|
| **06（本文）** | `06_Phase2立项设计.md` | 总纲：模型、决策、边界、导航、未决项 | 全部 |
| 07 | `07_Phase2防漂移治理与两条线.md` | 单一真源 + 程序线/美术线 + 三条铁律 + 权威顺序 + asset_id 命名权威 + 对齐合流 + AssetGenome | py `knowledge/governance/`、v3 GameSpec |
| 08 | `08_Phase2美术线_风格锚点与资产生产.md` | 设计阶段风格锚点门（选项 A）+ 资产生产 skill + 预算门 + 哈希缓存 + 一致性比对 | py Step07/09/12、godogen 资产层 |
| 09 | `09_Phase2程序线_MCP-Unity可玩生产与proof验收.md` | 可玩切片 + 薄 manifest + 引擎指南 + MCP 控制 Unity 现场开发 + proof bundle + verdict | godogen `build/01-03` |
| 10 | `10_Phase2波次分解与派发.md` | 插件架构（强设计关联/弱代码耦合）+ G1-G5 波次任务卡 | py StagePlugin/artifact_layer |

阅读顺序：06 → 07（治理是地基）→ 08（美术线）→ 09（程序线+验收）→ 10（怎么落地开发）。

---

## 1. 一句话模型

> **把冻结设计从「单一权威源」派生出程序线与美术线两条机器契约、在对齐层合流防漂移；
> 美术风格在设计阶段看真图锁定锚点；然后由 AI 经 MCP 现场控制 Unity 开发可玩切片，
> 以运行画面/视频 proof 验收，缺陷回写修正队列。**

这不是"生成代码→构建 EXE"，而是"**受治理的、以运行事实为准的、现场驱动的可玩生产**"。

---

## 2. 立项决策（D17-D25）

| 决策 | 内容 |
|------|------|
| **D17 引擎** | 仅 Unity；其他引擎（Godot/Bevy/Babylon）保留方向，走 `EngineBackend` 接缝，本期只 Unity 实现。 |
| **D18 产物** | Windows 可玩程序；但**经 MCP 现场构建 + proof 捕获**，不是离线 batchmode 生成。 |
| **D19 生产方式** | AI 全自动；资产生产走 skill（AI 通道先行，本地软件通道后置接入，`AssetProducer` 接缝）。 |
| **D20 时机** | 现在立项（supersede D16 封印）。 |
| **D21 验收** | **proof-over-claims**：以运行画面/视频证明为完成标准，不以"编译通过/测试绿"为准。verdict = pass / pass_with_warnings / needs_repair / blocked_by_environment。 |
| **D22 防漂移治理** | 采纳既有两条线设计：单一真源 → 程序线/美术线派生 → 对齐合流；三铁律（下游只派生不发明 / 稳定 asset_id 单点锚定 / JSON 压过 Markdown）。**不重造第二真源**。 |
| **D23 美术风格锚点** | 落**设计阶段**（选项 A，用户已定）：冻结前看真图确认锁定风格锚点，Phase 2 只消费不重造。 |
| **D24 插件化** | 一切能力做成插件（StagePlugin + artifact_layer 依赖声明），**强设计关联（制品依赖图显式）/ 弱代码耦合（插件自治、接口统一）**——谁错谁的问题立即定位。 |
| **D25 优化纪律** | 优化但保留既有设计，只做代码重构 + 真内容优化，不过度扩散、不过度简化（用户约束，贯穿全册）。 |

---

## 3. 边界：本期做 / 设计阶段前置 / 后置

**本期 Phase 2 做（生产 + 守护生产的治理）**：
- 两条线治理落地（asset_id 命名权威、asset_registry、对齐合流、AssetGenome 回填、drift 检测）；
- 资产生产 skill（AI 通道）+ 预算门 + 哈希缓存；
- MCP 控制 Unity 的可玩生产 + proof 捕获 + verdict + repair 回写；
- 全程插件化 + 红线在生产段强制。

**设计阶段前置（选项 A，Phase 1.x，一个新门，Phase 2 消费其产物）**：
- **美术风格锚点确认门**：设计工作台里看真图、对话式改提示词、反复重生成、确认锁定 `style_anchor_set`。它在冻结前完成，产出被 Phase 2 资产生产当作一致性基准。详见册 08。

**后置（接缝保留，本期不实现）**：
- 其他引擎后端（Godot/Bevy/Babylon）；资产本地软件通道（SD/ComfyUI/DCC）；完整商业级产线。

**优化取舍**：godogen `build/02-03` 建议"先 Babylon 后 Unity"（因 Babylon 截图快、Unity 捕获难）。**【优化/裁决】** 用户定 Unity，且**恰恰用 MCP 现场控制 Unity 解决了"驱动 + 捕获"这个难点**（Unity MCP 工具可建场景/跑 PlayMode/截帧），所以不绕道 Babylon，直接 Unity——但把"Unity proof 捕获方式"列为未决 Q（见 §5）。

---

## 4. 与 V4 现状的接缝（不重造）

V4 已有的、直接复用的地基（**优于 py 的 `design_handoff.json`，不重造**）：
- **GameSpec**（C0 从 FrozenDesign 确定性编译，带 `SpecRef` 锚定 + `source_map` 全覆盖）= 单一权威真源（D22 的真源就用它）；
- **C0-C6 文档编译**：C4 capability_contracts/engine_architecture = 程序线上游；C3 asset_spec_set = 美术线上游；
- **冻结门五道 + 红线 R1-R7 机器化**（`docs/design/05`）：Phase 2 沿用，不另立一套。

V4 缺、Phase 2 需补的（这是 Phase 2 治理的核心增量）：
1. **asset_id 命名权威 + asset_registry**（美术线命名单点锚定）——C3 侧补；
2. **程序线/美术线对齐合流 + AssetGenome 回填 + orphan/conflict 检测**（≈ py Step10）——新增对齐层；
3. **设计阶段美术风格锚点门**（≈ py Step07，选项 A）——设计工作台新增；
4. **可玩生产 + proof 验收**（≈ py Step11-14 + godogen）——Phase 2 主体。

---

## 5. 未决问题（重写细化前需补，不阻塞第一波 G1）

- **Q1 Unity 版本 + License**（LTS，如 Unity 6000 LTS / 2022.3 LTS；Personal/Pro）——G4 前必定。
- **Q2 Unity MCP 工具选型**：用哪个 MCP server 驱动 Unity（现成的 Unity-MCP 方案 / 自建）+ 它能否跑 PlayMode 与截帧/录屏——决定 proof 捕获可行性，G4 前必定。
- **Q3 proof 捕获方式**：Unity Recorder / 截帧脚本 / 外部录屏；无法捕获时的诚实降级（`blocked_by_environment`）。
- **Q4 AI 生产的模型/预算**：沿用 `adm4-ai` provider 还是 Phase 2 单列预算。
- **硬前置**：需一个真实走完设计→冻结→C0-C6 的 L6 项目做端到端夹具（`lane_defense` 全链 e2e 可作基底）。

---

## 6. 立项验收标准（本文档层面）

1. 决策 D17-D25 记录且约束清晰。✅
2. 两条线防漂移治理有明确落点（真源/派生/对齐/命名权威/回填/检测）。→ 册 07
3. 美术风格锚点在设计阶段（选项 A）有门与交互定义。→ 册 08
4. 可玩生产 + proof 验收模型明确（MCP-Unity、proof bundle、verdict、repair）。→ 册 09
5. 插件架构 + G1-G5 波次可派发。→ 册 10
6. 未决 Q1-Q4 显式登记，不阻塞 G1。✅

**下一步**：用户逐册评审 07-10 → 补 Q1/Q2（Unity 版本 + MCP 工具）→ 派发 G1（治理骨架 + 插件框架）。
