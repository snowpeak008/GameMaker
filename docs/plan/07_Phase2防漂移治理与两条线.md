# V4 · Phase 2 · 07 · 防漂移治理与两条线

> 立项分册 07/10。总纲见 `06`。本册是 Phase 2 的**地基**：把本项目三代已验证的"两条线防内容漂移"
> 治理原样吸收进 V4，只做 V4 化的代码重构与真内容优化（每处优化标「【优化】」）。
> 来源真相：`knowledge/governance/AI_NATIVE_REQUIREMENTS_HANDOFF_PROTOCOL.md`、`AlignmentProtocol.md`、
> `AssetGenome.md`、`ART_ASSET_NAMING_CONVENTION.md`；v3 `NEWrust/plan/..._universal_game_spec_compiler.md`。

---

## 1. 核心结构：一个真源 → 两条线 → 对齐合流

**不是"设计线 vs 生产线"，而是"一份权威设计事实 → 横向分叉出【程序线】与【美术线】两条机器契约 → 在对齐层重新合流"。**

```
                 GameSpec（单一权威真源，C0 从 FrozenDesign 确定性编译）
                   │  下游只能派生，不能发明（铁律①）
        ┌──────────┴───────────┐
   程序线（Program）          美术线（Art）
   capability_contracts       asset_spec_set + asset_registry
   engine_architecture        visual_language + drift_checks
   program_trace              art_trace
        └──────────┬───────────┘
              对齐合流（Alignment，≈ py Step10）
        unified_assets[program_ref + art_ref + uid]
        unresolved_conflicts[] / orphan_art_assets[]  → 人工决策
                   │
              可玩生产（册 09）→ AssetGenome 回填（设计 id → 实际文件）
```

**优化【采纳并简化真源】**：py 用 `design_handoff.json` 当真源；V4 已有 **GameSpec**（带 `SpecRef` 锚定 + `source_map` 全覆盖 + 冻结哈希绑定），语义更强。直接以 GameSpec 为唯一真源，**不再引入第二真源**——这是"代码重构"而非"内容改动"，两条线的派生规则原样保留。

---

## 2. 三条铁律（原样吸收，不动）

### 铁律① 下游只能派生，不能发明
- 程序线/美术线**不得创造新的设计事实**，只能 map（映射）+ 补充执行细节；
- 缺失的事实必须**标 gap，禁止用散文（Markdown 正文）填补**；
- **硬阻塞**：任何"只在 Markdown 出现、JSON 契约里没有"的事实 = 直接判失败（对应红线 R2 未知即停）。

### 铁律② 稳定 asset_id 单点锚定，从设计 ID 贯穿到磁盘文件名
- 美术线的**每个资产有稳定 `asset_id`**，所有下游对象 ID 从它派生（`visual_state_id = STATE-{asset_id}-...`、`binding_id = UX-{asset_id}-...`、`check_id = DRIFT-{asset_id}`）——单点锚定；
- `asset_id` → 命名规范约束的文件名 → 运行时加载路径，**一路贯穿，生产端不得自由起名**；
- 中央清单 `asset_registry.json`（美术线的命名权威）。

### 铁律③ JSON 契约永远压过 Markdown（权威顺序表）

| 优先级 | 来源 | 权威 |
|--------|------|------|
| 1 | **GameSpec**（V4） | 源游戏事实（唯一真源）|
| 2 | 派生协议（本册） | 派生规则 |
| 3 | `program_contract.json` | 程序事实 |
| 4 | `art_contract.json` | 美术事实 |
| 5 | `asset_registry.json` | 稳定资产身份与源映射（命名权威）|
| 6 | `*_trace.json` | 溯源/校验证据 |
| 7 | Markdown | 仅人类可读渲染 |

> Markdown 与 JSON 契约冲突时，**JSON 契约为准**。Markdown 永远不能改写事实。

**优化【VLM/AI 越权根绝，吸收 v3】**：采纳 NEWrust 的 `SpecPatch` 思路——**AI 只能产候选补丁，不能自判通过**（带 base_revision/base_hash，拒绝陈旧写入，单写者原子提交）。这从机制上根绝 AI 造成的内容漂移，与 V4 现有"AI 失败即 Err、人工门署名"红线一致。

---

## 3. 两种漂移与检测

| 漂移 | 定义 | 检测 | 处置 |
|------|------|------|------|
| **派生漂移** | 下游偷偷发明设计事实 | JSON 缺事实即 gap；Markdown-only 事实 = 硬阻塞 | Blocked + gap 清单（R2）|
| **视觉漂移** | 美术产出偏离风格锚点/ArtRules | `drift_checks`（每资产 `check_id=DRIFT-{asset_id}`，severity OK/WARNING/BLOCK/UNKNOWN）| BLOCK 路由回美术评审/风格门修正 |

视觉漂移的比对基准 = 设计阶段锁定的**风格锚点集**（`style_anchor_set`，册 08）。

---

## 4. 对齐合流层（新增，≈ py Step10 · 确定性 · 无 AI）

两条线在对齐层合流，**确定性校验，不生成任何资产**：

```rust
pub struct AlignmentReport {
    pub unified_assets: Vec<UnifiedAsset>,        // 程序与美术钉在同一 uid
    pub unresolved_conflicts: Vec<Conflict>,      // 规格冲突（human_decision_required）
    pub orphan_art_assets: Vec<String>,           // 美术产出但程序无依赖
    pub missing_for_program: Vec<String>,         // 程序需要但美术无对应
}
pub struct UnifiedAsset {
    pub uid: String,
    pub program_ref: SpecRef,     // 如 hero_controller.idle_anim
    pub art_ref: String,          // 如 asset_id PlayerIdle
    pub naming_pattern: String,   // 如 player_idle_{frame:03d}.png
    pub spec_triple: SpecTriple,  // { frames, size, format } 三要素
}
```

- 交叉核对 `program_ref` ↔ `art_ref`，比对 **帧数 / 尺寸 / 格式** 三要素；
- 不符 → `unresolved_conflicts`（标 `human_decision_required`）；
- 美术有、程序无依赖 → `orphan_art_assets`；程序要、美术缺 → `missing_for_program`；
- 这是"设计的图 vs 实际产的图对不上"的**机器检测点**（红线 R1：真实核对，非橡皮图章）。

---

## 5. AssetGenome 回填（生产端 · 设计 ID ↔ 实际文件）

生产端产出资产后，**回填** `AssetGenome`：把设计资产 id 钉到实际磁盘文件。

```
id: "ART-ILL-0001-asset_001"
files: ["ArtAssets/Illustrations/art_ill_0001_asset_001.png"]
created_at: ...
in_game_size: ...
used_by: [program_ref ...]
```

- id 与文件名是同一标识的词法变体（`ART-ILL-0001` ↔ `art_ill_0001`）；
- **"资产路径 = 运行时加载路径一致"**（godogen 的硬约束）在这里闭环：AssetGenome 记录的 path 必须等于运行时真正加载的 path，不一致即 drift；
- 生产每完成一个资产就回填一条，可断点续跑、可审计。

---

## 6. 命名规范（V4 化，保留精神）

**优化【保留规则精神，去掉塔防专属词表】**：py 的 `ART_ASSET_NAMING_CONVENTION.md` 把类型前缀、阵营、磨损状态、LOD、禁止词根写得很细，但**阵营/磨损词表是某个具体塔防包的内容**。V4 是多品类工具——所以：
- **保留机制**：`{AssetType}_{Subject}_{Qualifier}_{State}_{LOD}` 骨架 + 类型前缀（`SM_/SK_/T_/M_/UI_/VFX_`）+ 禁止词根扫描（接 R5 换皮词表）+ 结尾强制条款（所有美术 agent 产出 ID/文件名逐条自检）；
- **下放内容**：阵营/磨损等**词表由品类包提供**（`knowledge/design_space/<pack>/naming/`），不写死在工程里（对齐 E4：内容由用户供给）；
- 阶段/制品命名沿用 V4 现有（`SpecRef`、`new_id(prefix)`）。

---

## 7. 红线映射（Phase 2 治理段）

| 红线 | 在本册的强制点 |
|------|----------------|
| R1 指标即测量 | 对齐三要素核对、覆盖率是真实核对结果带证据，非恒真 |
| R2 未知即停 | 铁律① gap 不填散文；Markdown-only 事实硬阻塞；命名/派生缺失 Blocked |
| R4 产出锚定 | 每资产/契约带 SpecRef/asset_id，可追溯真源 |
| R5 参考名扫描 | 命名/资产名/文档过 SkinScanner，禁止词根扫描 |
| R6 基数申报 | 一对多派生（实体→资产）申报映射+丢弃清单+期望区间 |

---

## 8. 本册的落点（给册 10 的实现指引）

- `adm4-build`（新 crate）新增治理模块：`program_line.rs` / `art_line.rs` / `alignment.rs` / `asset_registry.rs` / `asset_genome.rs`；
- 契约结构全 serde + `#[serde(default)]`（旧档兼容铁律）；权威顺序校验器（JSON 压 Markdown）做成独立校验插件；
- **不新造第二真源**：一切派生自 GameSpec；对齐/回填/命名是**校验与追溯**层，不是并行状态树。

> 优化诚实边界：py 的对齐匹配算法当年靠 AI 提示词 + 导入引擎驱动，未硬编码。**【优化】** V4 把"帧/尺寸/格式三要素核对 + orphan/conflict 判定"做成**确定性 Rust 代码**（不靠 AI），只有"如何取舍冲突"才交人工——这是把易漂移的部分从 AI 手里收回到确定性代码，符合 D25"真内容优化"。
