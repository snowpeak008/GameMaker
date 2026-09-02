# W7 提案 B：类型系统落地（工程建模派）

> **提案 B · 工程建模派 · 待红队评审**
> 2026-09-02。基于 `12_W7设计方法论重构_立项简报.md`（共同真源）、两份审计报告、以及对 `adm4-decision/types.rs`、`adm4-spec/model.rs`、`adm4-pipeline/c0_compile.rs`、`c4_capabilities.rs`、`adm4-space/model.rs`、`adm4-authoring/freeze.rs`、`grid_strategy/pack.json` 的逐行阅读。
> 立场声明：本提案自底向上，从"现有编译链哪一行代码要改"出发。设计学概念（重度的语义维度、系统边界判据）留给提案 A 定义，本文在 §8 逐条列出对 A 的需求。**每一项扩展都附"保确定性论证"——为什么 C0-C6 仍然只映射不发明。**

---

## 0. 总纲：三条工程不变量

一切扩展必须同时满足以下三条，违反任何一条的方案本文不采纳：

- **I1 确定性守恒**：C0-C6 的每个新处理分支都必须是纯函数投影（输入相同则输出逐字节相同），AI 只做命名与叙述改写。凡是设计者没写的内容，编译链宁可 R2 阻塞也不补全。
- **I2 serde 旧档守恒**：所有新字段 `#[serde(default)]`，所有新枚举分支用新 tag；旧存档、旧 pack.json、旧 FrozenDesign 反序列化后行为与扩展前逐字节一致（这是现有代码库已验证的扩展纪律，见 `types.rs` 中 `PointRequirement`/`SelectionMode` 的先例注释）。
- **I3 校验前置**：新表达力带来的新非法状态（悬空图边、循环传导、缺 GWT 的 custom 效果）必须在冻结门或 C0 校验拦截，不允许流到 C4 之后才发现。

---

## 1. 系统模块（SystemModule）的类型建模

### 1.1 现状问题

现在"系统"只在两处存在：pack 里 L3 决策点的 `compiler_tags: {"spec_role": "system"}`，和 C0 产出的 `SystemSpec {id, name, purpose, interfaces: Vec<String>}`。`interfaces` 是自由字符串数组（实际填的是选项 implications），机器不可校验；系统无法跨 pack 复用——grid_strategy 想要装备系统只能把决策点抄一遍。

### 1.2 类型草案

新增知识层一等资产：系统模块。存放于 `knowledge/systems/<module_id>/module.json`，由 `adm4-space` 加载。

```rust
// adm4-decision/src/system_module.rs（新文件）

pub type SystemModuleId = String;   // "sys.equipment"
pub type NounId = String;           // "equipment.item"、"combat.attack_power"
pub type TierId = String;           // "light" / "medium" / "heavy"

/// 游戏名词声明：系统接口的原子单位。名词是系统间对话的唯一合法词汇，
/// 取代现在 SystemSpec.interfaces 的自由字符串。
#[derive(Serialize, Deserialize)]
pub struct NounDecl {
    pub id: NounId,
    pub kind: NounKind,
    pub display_name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NounKind {
    /// 可增减的量（金币、经验、行动点）
    Resource,
    /// 实体类别（装备、单位、宝石）——落地为 entity_table 的行
    EntityClass,
    /// 实体上的属性（攻击力、耐久）——落地为表列
    Property { of: NounId },
    /// 广播信号（升级完成、装备损坏）
    Signal,
    /// 规则槽位：允许其他系统的 RuleMod 效果作用的挂点（见 §4）
    RuleSlot,
}

/// 系统接口：provides/consumes/modifies 三张名词表。
#[derive(Serialize, Deserialize)]
pub struct SystemInterface {
    /// 本系统对外提供的名词（装备系统 provides: equipment.item、equipment.atk_bonus）
    pub provides: Vec<NounId>,
    /// 本系统消费的、必须由组合中其他系统或 pack 核心提供的名词
    /// （装备系统 consumes: combat.attack_power——没有战斗系统就没有装备的意义）
    pub consumes: Vec<NounId>,
    /// 本系统会修改的外部名词（装备系统 modifies: combat.attack_power）
    pub modifies: Vec<NounId>,
}

/// 系统模块：跨玩法复用的积木。决策点内嵌，随模块实例化，不复制进 pack。
#[derive(Serialize, Deserialize)]
pub struct SystemModule {
    pub module_id: SystemModuleId,
    pub module_version: String,          // semver，pack 侧写版本要求
    pub display_name: String,
    /// 本模块声明的名词（provides 的名词必须在这里有定义）
    pub nouns: Vec<NounDecl>,
    pub interface: SystemInterface,
    /// 重度阶梯（见 §2），至少 1 档
    pub heaviness: HeavinessLadder,
    /// 模块自带的决策点集合。id 必须以 "<module_id>." 为前缀（加载器校验）。
    /// 每个点可声明 tier_gate：低于该档位时点不激活（见 §2.2）。
    pub decision_points: Vec<DecisionPoint>,
    /// 模块自带的基数期望与一致性规则（与 GenrePack 同型，实例化时并入）
    #[serde(default)]
    pub cardinality_expectations: BTreeMap<String, CardinalityRange>,
    #[serde(default)]
    pub consistency_rules: Vec<ConsistencyRule>,
}
```

`DecisionPoint` 增加一个字段（serde 默认 None，旧清单零影响）：

```rust
pub struct DecisionPoint {
    // ...既有字段全部不动...
    /// 所属系统模块档位门：项目对该模块选定的档位 rank 低于此档时，
    /// 本点不激活、不进完成度分母（激活语义复用 PointRequirement + unlocks 机制，见 §2.2）
    #[serde(default)]
    pub tier_gate: Option<TierId>,
}
```

### 1.3 跨包复用的加载与校验语义

pack 不再复制系统决策点，改为**引用 + 绑定**。`GenrePack` 新增字段：

```rust
pub struct GenrePack {
    // ...既有字段全部不动...
    /// 引用的系统模块（旧包无此键 → 空 → 行为不变）
    #[serde(default)]
    pub system_refs: Vec<SystemRef>,
}

#[derive(Serialize, Deserialize)]
pub struct SystemRef {
    /// 项目内实例 id（同一模块可多实例：主武器装备 + 时装装备）
    pub instance_id: String,
    pub module: SystemModuleId,
    pub version_req: String,             // "^1.0"
    /// 允许的档位范围（pack 作者收窄模块全谱系）
    pub allowed_tiers: Vec<TierId>,
    /// 名词绑定：模块 consumes/modifies 的抽象名词 → 本 pack 的具体名词。
    /// 例：sys.equipment 的 "combat.attack_power" → grid_strategy 的 "grid.unit_roster.atk"
    pub bindings: BTreeMap<NounId, NounId>,
    /// 组合角色声明：本实例与核心循环的关联强度（判据由提案 A 定义，见 §8）
    pub core_link: CoreLink,   // Strong | Support | Peripheral
}
```

**加载语义**（`adm4-space/loader.rs` 扩展）：

1. 装配 DesignSpace 时，对每个 `system_refs` 条目：解析模块文件 → 校验版本 → 把模块决策点**按实例命名空间重写 id**（`sys.equipment.gem_table` → `<instance_id>.gem_table`），并入决策图。重写是纯字符串前缀替换，同一模块 + 同一实例 id 装配结果逐字节确定。
2. 模块的一致性规则与基数期望做同样的 id 重写后并入 pack 规则集，喂给既有的冻结门第 2 道——**复用现有校验机器，不新造一套**。
3. **绑定校验**（space validate 时）：`consumes ∪ modifies` 中每个名词必须在 bindings 有映射，且映射目标必须是 pack 核心或其他实例 provides 的名词。悬空 → 加载失败（不是警告）。
4. 模块文件哈希进入 FrozenDesign 的 `content_hash` 计算范围（`pack_version` 旁新增 `module_versions: BTreeMap<SystemModuleId, String>`），保证冻结可追溯：模块升级不会静默改变已冻结项目的语义。

**为什么不复制**：复制的代价审计已经付过一次（v2 2575 点就是复制冲压的尸体）。引用 + 命名空间重写让"装备系统修一个 bug"只改一处，所有引用它的 pack 下次装配自动生效，而已冻结项目因模块版本锁定不受影响。

---

## 2. 重度（Heaviness）的类型建模

### 2.1 双轨制：声明档位 + 测得向量

红队会问："重度是拍脑袋的枚举还是可测的量？"回答是两者都要，各司其职：

- **声明档位（tier）**：设计师的意图输入，有限有序枚举，是决策不是测量。它决定激活哪些内容。
- **测得向量（measured weight）**：编译期从激活结果**确定性计算**出的证据，用于预算校验。它不由人填，所以不会说谎。

```rust
/// 重度阶梯：有序档位列表。档位数量与命名由模块作者定（装备系统可以 4 档，
/// 音游判定系统可以 2 档）——类型系统只保证有序与可比。
#[derive(Serialize, Deserialize)]
pub struct HeavinessLadder {
    /// 按 rank 升序；rank 从 0 开始连续（加载器校验）
    pub tiers: Vec<HeavinessTier>,
}

#[derive(Serialize, Deserialize)]
pub struct HeavinessTier {
    pub id: TierId,
    pub rank: u8,
    pub display_name: String,
    /// 本档位的设计语义描述（给人读；机器语义在 tier_gate 与 inductions）
    pub summary: String,
    /// 传导声明：本模块处于本档（含以上）时，对其他系统的最低档位要求。
    /// 例：装备 heavy → 背包 ≥ medium（宝石+材料需要分类收纳）
    #[serde(default)]
    pub inductions: Vec<Induction>,
}

#[derive(Serialize, Deserialize)]
pub struct Induction {
    /// 目标：另一个系统模块 id（组合校验时解析到项目内实例；
    /// 目标模块不在组合中 = 违例"传导缺口：需要引入 X 系统"）
    pub target_module: SystemModuleId,
    pub min_tier_rank: u8,
    /// 传导理由（进冻结门 finding 文案，让违例可读）
    pub reason: String,
}

/// 测得向量：编译期从冻结集计算，凡字段全部是计数，无人工输入。
#[derive(Serialize, Deserialize)]
pub struct MeasuredWeight {
    /// 该实例激活且已作答的决策点数
    pub active_points: usize,
    /// 激活的表/矩阵/图参数数
    pub active_schemas: usize,
    /// modifies 名词数（对外耦合面）
    pub coupling_width: usize,
    /// 激活的 L4 机制的效果总数
    pub effect_count: usize,
}

impl MeasuredWeight {
    /// 标量化：系数由通用层配置文件声明（数据驱动，不硬编码；
    /// 系数标定是提案 A 的活，见 §8 第 6 条）
    pub fn score(&self, coeff: &WeightCoefficients) -> f64 { /* 加权和 */ }
}
```

### 2.2 档位 = 决策点子集的激活条件（复用 unlocks 机器）

**关键工程决定：不新造激活机制。** 加载器为每个系统实例合成一个单选决策点：

```
id: "<instance_id>.tier"，level: L3，question: "「装备系统」采用什么重度档位？"
options: 每个 allowed_tier 一个选项，
  选项 unlocks = 该模块中所有 tier_gate.rank <= 本档 rank 的决策点 id（实例命名空间）
```

于是档位选择走的是**现有**的 select → unlocks 激活 → 完成度分母 → 冻结门第 1 道整条既有链路。`engine.rs`、`freeze.rs` 的激活/完备度代码一行不改。档位换挡（heavy 改回 light）自动等价于现有"改选导致下游点失活"的语义，已有测试覆盖这条路径。

**保确定性论证**：合成决策点由加载器从模块声明纯函数生成（模块+实例 id 相同 → 合成结果相同），选择由用户做，激活由既有确定性机器执行。全程无发明。

### 2.3 装备系统谱系：可编译示例（JSON schema 级）

`knowledge/systems/sys.equipment/module.json` 节选（可直接按 §1.2 类型反序列化）：

```json
{
  "module_id": "sys.equipment",
  "module_version": "1.0.0",
  "display_name": "装备系统",
  "nouns": [
    { "id": "sys.equipment.item",      "kind": { "kind": "entity_class" }, "display_name": "装备" },
    { "id": "sys.equipment.gem",       "kind": { "kind": "entity_class" }, "display_name": "宝石" },
    { "id": "sys.equipment.forge_mat", "kind": { "kind": "resource" },     "display_name": "强化材料" },
    { "id": "sys.equipment.equipped",  "kind": { "kind": "signal" },       "display_name": "穿戴变更信号" }
  ],
  "interface": {
    "provides": ["sys.equipment.item", "sys.equipment.equipped"],
    "consumes": ["ext.combat.attack_power", "ext.inventory.slot"],
    "modifies": ["ext.combat.attack_power"]
  },
  "heaviness": {
    "tiers": [
      { "id": "light",  "rank": 0, "display_name": "轻·属性加成",
        "summary": "装备只有一张属性表，穿上加数值", "inductions": [] },
      { "id": "medium", "rank": 1, "display_name": "中·构筑装备",
        "summary": "追加技能词条与套装效果，装备参与 BD 构筑",
        "inductions": [
          { "target_module": "sys.inventory", "min_tier_rank": 1,
            "reason": "词条装备需要背包支持比较与筛选" } ] },
      { "id": "heavy",  "rank": 2, "display_name": "重·装备养成",
        "summary": "追加宝石镶嵌、合成、强化曲线，装备本身成为长线养成目标",
        "inductions": [
          { "target_module": "sys.inventory", "min_tier_rank": 2,
            "reason": "宝石+材料+装备三类库存需要重型收纳" },
          { "target_module": "sys.economy", "min_tier_rank": 1,
            "reason": "强化材料需要产出与回收闭环" } ] }
    ]
  },
  "decision_points": [
    { "id": "sys.equipment.attr_table", "level": "L5", "domain": "equipment",
      "genre_scope": "universal", "question": "装备属性表的结构？",
      "tier_gate": "light",
      "options": [ { "id": "flat_bonus", "label": "平坦加成表",
        "parameter_schema": { "schema": "table", "columns": [
          { "key": "id", "kind": { "kind": "text" }, "required": true },
          { "key": "slot", "kind": { "kind": "enum", "variants": ["weapon","armor","accessory"] }, "required": true },
          { "key": "atk_bonus", "kind": { "kind": "int" }, "required": true } ],
          "row_key": "id", "cardinality_key": "equipment_items" },
        "compiler_tags": { "spec_role": "entity_table", "visual_form": "sprite2d" } } ] },

    { "id": "sys.equipment.gem_table", "level": "L5", "domain": "equipment",
      "genre_scope": "universal", "question": "宝石表的结构？",
      "tier_gate": "heavy",
      "options": [ "…（宝石表 schema，同上形态）" ] },

    { "id": "sys.equipment.synthesis_rule", "level": "L4", "domain": "equipment",
      "genre_scope": "universal", "question": "装备合成的机制规则？",
      "tier_gate": "heavy",
      "options": [ { "id": "n_to_one", "label": "N 合 1 同稀有度合成",
        "parameter_schema": { "schema": "scalar", "fields": [
          { "key": "merge_count", "kind": { "kind": "int" },
            "constraint": { "constraint": "range", "min": 2, "max": 5 }, "required": true } ] },
        "compiler_tags": { "spec_role": "mechanic", "system": "sys.equipment" },
        "effects_template": [
          { "effect": "consume_resource", "resource": "sys.equipment.item",
            "formula": "{param:merge_count} 件同稀有度装备" },
          { "effect": "spawn_entity", "entity": "sys.equipment.item" } ] } ] },

    { "id": "sys.equipment.enhance_curve", "level": "L6", "domain": "equipment",
      "genre_scope": "universal", "question": "强化成功率/成本曲线？",
      "tier_gate": "heavy",
      "options": [ { "id": "curve_decay", "label": "递减成功率曲线",
        "parameter_schema": { "schema": "curve",
          "x_axis": "enhance_level", "y_axis": "success_rate",
          "interpolation": "linear", "min_points": 5, "cardinality_key": "enhance_levels" },
        "compiler_tags": { "spec_role": "data_table" } } ] }
  ]
}
```

轻档项目：`<inst>.tier = light` → 只有 `attr_table` 激活，完成度分母 1 个点。重档项目：`gem_table`/`synthesis_rule`/`enhance_curve` 全部进入分母，且 inductions 对背包、经济系统的档位要求进入组合校验（§3）。**同一份模块文件，两种玩法，零复制。**

---

## 3. 组合规则的机器校验

### 3.1 为什么不用约束求解器

传导约束是"档位 ≥ rank"形态的单调约束，档位是有限全序集，所以传导闭包是**有限格上的单调不动点**，最多迭代 `Σ档位数` 轮必然终止，普通 worklist 就够。引入 SAT/SMT 求解器是把可判定的小问题升格成依赖黑箱的大问题，红队会打，我自己先打：不用。

### 3.2 类型与校验器

```rust
/// 项目侧组合状态（存 AuthoringState，随项目存档）
pub struct SystemComposition {
    pub instances: Vec<SystemInstance>,
}
pub struct SystemInstance {
    pub instance_id: String,
    pub module: SystemModuleId,
    pub module_version: String,
    /// 冻结时从 "<instance_id>.tier" 决策点读出（用户的选择是唯一真源，
    /// 这里只是解析缓存；两处不一致 = 内部错误，非用户违例）
    pub declared_tier_rank: u8,
    pub core_link: CoreLink,
}

/// 组合预算配置：通用层数据文件声明，不硬编码（标定值待提案 A，见 §8）
pub struct CompositionBudget {
    /// 允许的重度核心数量（默认 min=1, max=1："一个重度核心"从口号变成校验）
    pub heavy_core_count: CardinalityRange,
    /// 判为"重度核心"的档位 rank 门槛
    pub heavy_rank_threshold: u8,
    /// 全组合测得分总预算（按项目规模 u.target_scale 分档给值）
    pub total_score_budget: BTreeMap<String, f64>,
    pub weight_coefficients: WeightCoefficients,
}
```

校验器伪代码（纯函数：输入组合+模块库+预算配置，输出违例清单）：

```text
fn validate_composition(comp, modules, budget) -> Vec<Violation>:
  # 一、传导闭包（有限格不动点）
  required[i] := comp.instances[i].declared_tier_rank  for all i
  repeat until 不再变化:                      # 单调递增且 rank 有上界 → 必终止
    for i in instances:
      for ind in modules[i].tiers[required[i]] 及以下各档的 inductions:
        match 按 module id 找目标实例 j:
          found  => required[j] := max(required[j], ind.min_tier_rank)
          absent => 违例 V1: 传导缺口——组合缺少系统 ind.target_module
                    （文案带 ind.reason）
  for j: if required[j] > declared[j]:
    违例 V2: 传导不满足——实例 j 需 ≥ tier[required[j]]，当前 tier[declared[j]]
            （附完整传导链：谁在哪一档要求的）

  # 二、重度预算
  heavies := { i | declared[i] >= budget.heavy_rank_threshold }
  if |heavies| ∉ budget.heavy_core_count: 违例 V3: 重度核心数量违规
  for i in heavies:
    if comp.instances[i].core_link != Strong:
      违例 V4: 重度系统未与核心玩法强关联（"重的必须是核心"）
  total := Σ measured_weight(i).score(budget.weight_coefficients)
           # measured_weight 从激活决策点/表计数，见 §2.1，无人工输入
  if total > budget.total_score_budget[项目规模档]: 违例 V5: 总重度超预算（附逐系统分值表）

  # 三、接口完整性
  for i, for noun in modules[i].interface.consumes:
    if bindings[i][noun] 未被任何实例 provides 且非 pack 核心名词:
      违例 V6: 悬空消费——实例 i 需要 noun，组合中无人提供
```

### 3.3 与冻结门五道的合流

不加第六道门，不动门的数量与顺序：

- **V1/V2/V6（结构性违例）并入 gate2_consistency**：实现为 `ConsistencyRuleKind` 三个新枚举分支（`TierInduction`/`NounProvided` 等），由加载器从模块声明自动生成规则实例，喂现有 gate2 的 finding 机器。新枚举分支不影响旧包反序列化（`model.rs` 中 RowReference 的先例）。
- **V3/V4/V5（预算违例）同样进 gate2**，finding code 前缀 `composition_budget_*`。预算违例**默认 block**；但允许人工署名豁免走既有 `NaJustification` 通道（R3 可追责）——因为"两个重度系统"可能是刻意设计（如双核心 MMO），规则要能拦也要能签字放行，不能变成新枷锁。
- 校验同时挂在 authoring 期（`engine` 每次档位变更后即时反馈，不用等冻结）——同一个纯函数两处调用。

---

## 4. EffectSpec 扩展：从七封闭枚举到分层效果体系

### 4.1 设计：单平面枚举 + 三层确定性等级

审计确认七枚举是架构天花板（MOBA 位移、遗物、DoT 全部无表达位）。扩展方案：**序列化上保持单层 tag 枚举**（`#[serde(tag="effect")]`，旧 tag 一个不动，旧 JSON 逐字节兼容），**语义上分三层确定性等级**，每层有明确的 C4 投影承诺：

```rust
/// 第 1 层（封闭核心，既有 7 个，序列化 tag 与字段完全不动）——
/// C4 全语义投影：效果结构即需求结构。
/// 第 2 层（受控扩展，新增 8 个）——C4 模式投影：每个变体有固定渲染函数，
/// 字段全部由设计者/pack 作者填写，投影只做字符串组装。
/// 第 3 层（逃生舱口，1 个）——C4 转录投影：只誊写设计者提供的验收模板，
/// 模板缺失 = R2 阻塞。
#[derive(Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum EffectSpec {
    // ===== 第 1 层：封闭核心（不动）=====
    ModifyProperty { entity: String, property: String, formula: String },
    SpawnEntity { entity: String },
    DespawnEntity { entity: String },
    ChangeState { machine: String, to_state: String },
    GrantResource { resource: String, formula: String },
    ConsumeResource { resource: String, formula: String },
    EmitSignal { signal: String },

    // ===== 第 2 层：受控扩展 =====
    /// 位移：实体按谓词移动（MOBA 闪现/击退、战棋强制移动）
    Displace { entity: String, motion: MotionKind, distance_formula: String },
    /// 空间查询效果：对满足空间谓词的实体集施加内层效果（AoE、链式）
    AreaApply { center: String, shape: AreaShape, radius_formula: String,
                inner: Vec<EffectSpec> },
    /// 附着：实体间挂载关系（宝石镶嵌、装备穿戴、buff 附身）
    Attach { host: String, attachment: String, slot: Option<String> },
    Detach { host: String, attachment: String },
    /// 时序包装：内层效果延迟/持续/周期执行（DoT、延迟爆炸、波次 tick）
    Schedule { timing: TimingSpec, inner: Vec<EffectSpec> },
    /// 规则修改器：作用于另一条机制（遗物/思维阁/天赋的表达位）。
    /// target 必须是 spec 内真实 mechanic id 或系统声明的 RuleSlot 名词，
    /// 悬空 → GameSpec 校验错误（新增 rulemod_dangling_target）。
    ModifyRule { target_mechanic: String, patch: RulePatch },
    /// 抽取：从表/池中按声明的抽取规则取行（三选一 draft、抽卡、卡组抽牌）
    DrawFromPool { pool_table: String, count_formula: String,
                   draw_rule: DrawRule, destination: String },
    /// 判定掷点：声明式随机检定（2d6+技能 vs 难度、命中掷点）
    RollCheck { formula: String, on_success: Vec<EffectSpec>,
                on_failure: Vec<EffectSpec> },

    // ===== 第 3 层：逃生舱口 =====
    /// 自定义效果：动词 + 类型化操作数 + 设计者必填的验收模板。
    /// C4 对它只做占位符替换后誊写，不做任何语义展开。
    Custom {
        verb: String,                          // "重排规则语序"（Baba Is You）
        operands: BTreeMap<String, String>,    // 名词引用，进悬空校验
        /// GWT 模板（支持 {param:KEY} 与 {operand:KEY} 占位符）。
        /// 缺失或占位符解析失败 → C0 按 R2 阻塞。
        acceptance_given: Vec<String>,
        acceptance_when: Vec<String>,
        acceptance_then: Vec<String>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "patch", rename_all = "snake_case")]
pub enum RulePatch {
    /// 给目标机制公式中的命名系数乘一个因子（遗物"伤害 +25%"）
    ScaleCoefficient { coefficient: String, factor_formula: String },
    /// 整条替换目标机制公式（思维阁改写检定规则）
    ReplaceFormula { new_formula: String },
    /// 停用/启用目标机制（"你不再能反击"）
    Disable, Enable,
    /// 给目标机制追加前置条件
    AddPrecondition { subject: String, predicate: String },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "timing", rename_all = "snake_case")]
pub enum TimingSpec {
    Delayed { delay_formula: String, unit: TimeUnit },
    OverTime { duration_formula: String, tick_formula: String, unit: TimeUnit },
    Periodic { interval_formula: String, unit: TimeUnit },
}
// TimeUnit: Seconds | Turns | Ticks —— 回合制与实时共用一套时序类型
```

### 4.2 图与曲线参数：schema 与序列化

`ParameterSchema`/`ParameterValues` 各加两个 tag 分支（旧 tag 不动）：

```rust
#[serde(tag = "schema", rename_all = "snake_case")]
pub enum ParameterSchema {
    None, Scalar {...}, Table(TableSchema), Matrix(MatrixSchema),   // 不动
    /// 图：节点+边+双侧负载。覆盖对话树、肉鸽地图、技能树、关卡拓扑。
    Graph(GraphSchema),
    /// 曲线：单调 x 的采样点列。覆盖强化曲线、经验曲线、掉率曲线；
    /// 帧级手感曲线可承载采样近似（诚实声明：不承诺连续语义，见 §6 缺口）。
    Curve(CurveSchema),
}

pub struct GraphSchema {
    pub node_payload: Vec<ScalarField>,     // 节点负载列（复用 ScalarField，含 is_skin）
    pub edge_payload: Vec<ScalarField>,     // 边负载列（对话选项文本、通行条件）
    pub directed: bool,
    /// 声明无环则校验器跑拓扑排序，有环即冻结门 block（对话树/技能树设 true）
    pub acyclic: bool,
    /// 入口节点约束：Single（对话树单根）/ Multiple / Any
    pub entry: EntryConstraint,
    pub node_cardinality_key: String,
}

pub struct CurveSchema {
    pub x_axis: String, pub y_axis: String,
    pub x_kind: ValueKind, pub y_kind: ValueKind,
    pub interpolation: Interpolation,   // Step | Linear —— 只这两种，语义可判定
    pub min_points: usize,
    pub cardinality_key: String,
}

#[serde(tag = "values", rename_all = "snake_case")]
pub enum ParameterValues {
    None, Scalars {...}, Rows {...}, Cells {...},   // 不动
    Graph { nodes: Vec<GraphNode>, edges: Vec<GraphEdge> },
    Curve { points: Vec<CurvePoint> },              // 按 x 升序，校验器强制
}
pub struct GraphNode { pub id: String, pub payload: BTreeMap<String, TypedValue> }
pub struct GraphEdge { pub from: String, pub to: String,
                       pub payload: BTreeMap<String, TypedValue> }
```

GameSpec 侧新增对应产物类型（C0 逐字节转录）：

```rust
pub struct GraphSpec {
    pub id: String,
    pub node_columns: Vec<PropertySpec>, pub edge_columns: Vec<PropertySpec>,
    pub nodes: Vec<GraphNode>, pub edges: Vec<GraphEdge>,
    pub directed: bool, pub acyclic: bool,
}
// GameSpec 增加 pub graphs: Vec<GraphSpec>（serde default），
// contains_ref / all_ref_paths 增加 "graphs" 分支（R4 锚定闭合）。
// 曲线不加新 section：CurveSchema 编译成两列 TableSpec（x,y）+ 表级
// interpolation 注记——复用既有 TableSpec 通路，C3/C6 对表的处理零改动。
```

校验规则（`adm4-space/validate.rs` + GameSpec 校验双侧都拦，I3）：边端点必须是已声明节点；acyclic 声明则拓扑检查；entry 约束检查；节点数进基数期望。

### 4.3 保确定性论证（逐层）

红队最重的一拳是"开放效果类型 = C4 发明内容"。逐层回应：

- **第 1 层**：一个字节不改，既有论证继续成立。
- **第 2 层**：每个新变体在 C4 有唯一固定渲染函数（如 `Displace` → "实体 {entity} 按 {motion} 位移，距离 = {distance_formula}"），与现在 `ModifyProperty` 的渲染完全同构。所有字段来自 pack/模块作者写的 effects_template + 用户参数占位符替换（`substitute_placeholders` 既有函数，纯函数）。**变体是新增的，投影方式没变：仍是"枚举分支 → 固定文案模板"的模式匹配，穷尽匹配由 Rust 编译器保证（不写 `_` 分支），漏一个变体编译不过。**
- **`ModifyRule` 特别论证**：规则修改器不产生新规则，只引用已存在的 mechanic id + 封闭的 `RulePatch` 操作集。C4 投影为"机制 X 的系数 Y 乘以 Z"——这是转录不是发明。悬空 target 在 GameSpec 校验被拦（新校验码），不可能流到 C4。
- **第 3 层 `Custom`**：投影 = 誊写设计者自己写的 GWT 三段 + 占位符替换。C4 新增的代码路径里**没有任何生成逻辑**——设计者不写验收模板，C0 直接 R2 阻塞（与现在"缺 effects_template 则阻塞"完全同一纪律）。逃生舱口的代价是设计者多写三行 GWT，换来的是任何机制都有结构化落点而不是像审计 B 那样蒸发。
- **图/曲线**：C0 做的是 `ParameterValues::Graph` → `GraphSpec` 的逐字段拷贝（与现在 Rows → TableSpec 同构），无任何推导。

### 4.4 冲击面：C0-C6 逐编译器评估

| 编译器 | 改不改 | 具体改动 |
|---|---|---|
| **C0** (`c0_compile.rs`) | **改，中等** | ① `compile_data_table` 增加 Graph/Curve 两个匹配臂（Graph→GraphSpec、Curve→双列 TableSpec）；② `compile_mechanic` 不改逻辑——效果模板反序列化自动获得新变体；③ 新增 design_notes 收集（§5）；④ `default_role` 不动 |
| **C1** (`c1_validation.rs`) | **改，小** | 校验清单追加：graphs 段引用闭合、rulemod 目标存在、Custom 验收模板非空。红队提示词把 custom 机制列为必审项 |
| **C2** (`c2_gameplay.rs`) | **改，小** | `source_text` 拼接追加 design_notes 与图结构摘要（节点数/边数/入口）；AI 约束提示词不变（仍"不得引入规格外设计"） |
| **C3** (`c3_content.rs`) | **不动** | 视觉白名单仍走 EntitySpec.visual_form；图节点若需资产，走既有 entity_table 通路（图负载列可引用实体 id） |
| **C4** (`c4_capabilities.rs`) | **改，大（本次主战场）** | ① `project_scenario` 的效果 match 增加 8+1 个臂（每臂一个固定文案函数）；② `collect_data_structures` 覆盖新变体中的实体/表引用（修复审计发现的"数据结构为空"缺陷顺带做）；③ Schedule/AreaApply 递归展开内层效果（有限深度，模板写死，无循环风险——效果树不含引用）；④ Custom 臂 = 誊写 |
| **C5** (`c5_style.rs`) | **不动** | 风格确认与效果类型无关 |
| **C6** (`c6_plan.rs`) | **改，小** | ModifyRule 生成跨机制依赖边（target 机制的任务 → 修改器任务），顺带把审计 B 骂的"115 任务 3 条依赖边"补上一类真依赖 |

---

## 5. rationale / 自由文本进编译链 + custom_mechanic 一等入口

### 5.1 design_notes：C0 不再丢 rationale

审计 B 实锤：rationale 只在 `spec_role=promise` 时被消费，其余全丢。字段设计：

```rust
/// 设计注记：随 spec 元素流动的自由文本，来源可追溯。
/// 纪律：注记只被"携带与展示"，永不被编译成结构（效果/表/任务），
/// 所以它不威胁确定性——它是给 C2 的 AI 和 C4 的程序员看的上下文。
#[derive(Serialize, Deserialize)]
pub struct DesignNote {
    pub source_decision: String,
    pub source_option: String,
    pub role: NoteRole,        // Rationale | Statement（自由文本参数）
    pub text: String,
}
// SystemSpec / MechanicSpec / TableSpec / GraphSpec / ContentSpec
// 各增加 pub design_notes: Vec<DesignNote>（serde default，旧档兼容）
```

C0 改动：编译每个 spec 元素时，把该选择的非空 rationale（多选点逐选项）与 statement 类自由文本参数装入对应元素的 design_notes。**消费方式**：

- **C2**：`source_text` 追加 `设计注记：\n- {text}(来自 {source_decision})`。审计 B 摘录 4 的病根（"变异器的输入输出设计全在理由里，但不进 AI 上下文"）就此修复。
- **C4**：`CapabilityContract` 增加 `design_notes: Vec<String>`（渲染进 document 的"设计意图"小节）；接口命名的 user_prompt 附带注记。**投影逻辑不读注记**——GWT 仍只从结构化字段派生，保 I1。

### 5.2 custom_mechanic：一等入口 schema

审计确认原创机制零入口（engine 无 add_decision_point，2575 点零 custom）。方案：**项目内自定义机制**，不要求先写 pack：

```rust
// adm4-authoring/engine.rs 新 API（走既有 confirm 纪律，AI 永不代确认）
pub fn add_custom_mechanic(&mut self, draft: CustomMechanicDraft) -> Adm4Result<DecisionId>;

/// 结构化草案：schema 强制到与 pack 内建 L4 选项同等的信息密度——
/// 自由的是内容，不自由的是形态。
#[derive(Serialize, Deserialize)]
pub struct CustomMechanicDraft {
    pub id_hint: String,               // 引擎加 "custom." 前缀并查重
    /// 必填：归属系统（既有 L3 决策 id 或系统实例 id，悬空即拒绝）
    pub system: String,
    pub rule_text: String,             // 必填非空
    /// 必填：效果列表，走 §4 的 EffectSpec 全集（含 Custom 变体）。
    /// 引擎当场反序列化校验 + 名词/实体引用悬空校验，非法即拒绝——
    /// 不存在审计 B 的"未知列静默放行"双标。
    pub effects: Vec<serde_json::Value>,
    #[serde(default)]
    pub parameter_schema: ParameterSchema,
    #[serde(default)]
    pub preconditions: Vec<ConditionSpec>,
    pub rationale: String,             // 必填：为什么要这个机制（进 design_notes）
}
```

落地形态：引擎把草案转成一个 `is_custom: true` 的单选项决策点并入项目私有决策集（不写回 pack 文件），选择即成立，走全部既有链路（完成度/一致性/C0）。C0 对它零特殊分支——它就是一个普通 L4 点。

### 5.3 与红线协调

- **R5 换皮扫描覆盖 custom**：`SkinScanner` 扫描范围显式追加 custom 点的 `rule_text`/`rationale`/参数中 `is_skin` 字段；custom 机制的 id/label 进换皮比对词表。**加严而非放松**：冻结门报告已有 `custom_option_count`，扩展为 gate4 逐条强制处置——每个 custom 机制必须有一条红队 finding 的显式处置记录（accept/revise），没有则 gate4 block。离线 scripted 红队的自证问题是既有缺陷，本提案不扩大它（custom 强制处置在 scripted 通道下同样要求逐条记录，留下审计痕迹）。
- **reference_games ≥ 3**：该硬规则管的是 pack 认证，**继续保留**；custom_mechanic 是项目私有点不走 pack 认证，天然绕开"原创混合品类找不到 3 个参考"的逻辑死锁——但代价是 custom 点不进知识库复用。想沉淀进模块库？那时才走认证（可附"原创声明 + 人工署名"替代参考游戏，规则降级为警告，与审计 A 建议一致）。

---

## 6. 表达力覆盖证明（Q4，工程视角）

10 款审计游戏的 ❌ 项逐一对到本提案的类型设施（设计学层面的系统划分留给提案 A，此处只证"有无表达位"）：

| 游戏 | 原 ❌ 卡点 | 表达设施 |
|---|---|---|
| 幸存者 | 三选一 draft | `DrawFromPool { draw_rule: PickN{n:3, choose:1} }` |
| 幸存者 | 合成进化组合表 | 既有 Matrix + `Attach`/`SpawnEntity` 效果 |
| 星露谷 | 时间系统/作物周期 | `Schedule{Periodic, unit:Ticks}` + 世界时钟 = 系统模块（sys.world_clock provides 时间信号）|
| 杀戮尖塔 | 抽牌/弃牌/能量 | `DrawFromPool`(牌库→手牌) + `ConsumeResource`(能量) |
| 杀戮尖塔 | 遗物 | `ModifyRule{ScaleCoefficient/AddPrecondition}` |
| 杀戮尖塔 | 分支地图 | `ParameterSchema::Graph{acyclic:true, entry:Single}` |
| MOBA | 位移/技能组 | `Displace` + `AreaApply` + `Schedule`（冷却=`Schedule{Delayed}`+状态机）|
| BotW | 元素×材质 | 既有 Matrix + `AreaApply{inner:[ChangeState]}` 表达传播一跳 |
| 三消 | 连锁结算 | `AreaApply{shape:MatchGroup}` + `Schedule{Periodic, unit:Ticks}` 级联 |
| 棋牌 | 番型计分表 | 既有 Table；牌型判定见下方缺口 |
| 音游 | 谱面/判定窗 | 谱面=`ContentSpec`+Curve；判定窗=Table（分档±ms）|
| 极乐迪斯科 | 检定对话 | `RollCheck{on_success/on_failure}` + `Graph`(对话树，边负载=检定门) |
| 极乐迪斯科 | 思维阁 | `ModifyRule{ReplaceFormula}` |

**诚实缺口（红队不用找，我自首）**：① **算法体机制**（牌型合法性判定、三消消除检测的匹配算法）：类型系统只能用 `Custom{verb:"牌型合法性判定"}` + 设计者写 GWT 声明黑箱边界，算法本体不进 spec——这是"需求文档工具"的合理边界而非缺陷，但覆盖度口径必须如实标注为"声明式覆盖"。② **实时 netcode/回滚**：无表达位，明确宣布不在 scope。③ **涌现语义**：Matrix+AreaApply 表达单跳传播，多跳涌现（火→草→气流）只能靠多条机制组合声明，涌现结果不可静态推导——如实标注。④ **帧级手感**：Curve 是采样近似，不承诺连续/微分语义。

---

## 7. 迁移方案（Q7）

### 7.1 冲击面：逐 crate 点名

13 个 crate（`crates/` 12 个 + `apps/adm4-cli`；`adm4-desktop`、`tools/v2_migration` 一并列出）：

| crate | 冲击 | 内容 |
|---|---|---|
| adm4-foundation | 无 | 错误类型够用 |
| adm4-contracts | 小 | `values.rs` 不动；新增 GraphNode/GraphEdge/CurvePoint 基础类型 |
| adm4-decision | **大** | types.rs：ParameterSchema/ParameterValues 加 Graph/Curve 分支、DecisionPoint 加 tier_gate；新文件 system_module.rs（SystemModule/HeavinessLadder/NounDecl）|
| adm4-spec | **大** | model.rs：EffectSpec +9 变体、GameSpec 加 graphs 段、各 spec 加 design_notes；validate 加悬空/无环检查 |
| adm4-space | **大** | 模块加载器 + 实例化命名空间重写 + 绑定校验 + 合成 tier 决策点；validate.rs 加图 schema 校验 |
| adm4-authoring | **大** | engine.rs：add_custom_mechanic、组合状态维护；freeze.rs：gate2 并入组合校验/V 系列 finding、gate4 custom 强制处置 |
| adm4-pipeline | **大** | 按 §4.4 表逐编译器改（C4 最大，C3/C5 不动）|
| adm4-template | 小 | 逆向答卷可引用模块决策点（id 前缀适配）；25 份模板处置见 7.4 |
| adm4-archive | 小 | FrozenDesign 增加 module_versions 键（serde default）|
| adm4-ai | 无 | 请求/响应结构不变 |
| adm4-app | 中 | 服务层暴露组合面板/档位选择/custom 入口三组新服务 |
| adm4-build | 小 | 读 C4 契约的执行器适配新效果文案（结构未变仍是 GWT 字符串）|
| adm4-cli | 中 | `system add/tier set/composition check`、`mechanic add-custom` 子命令 |
| adm4-desktop | 中 | 组合视图（系统×档位矩阵 + 传导违例灯）；可后置 |
| tools/v2_migration | 小 | 增加 checklist→PromptLibrary 降级转换 |

### 7.2 分波计划（每波独立可验收）

**波 0：类型底座（纯加法）** — 约 3-4 人日
decision/spec/contracts 三个 crate 的新类型 + 校验器，不接任何编译器。
验收：① `cargo test` 全绿，既有 407 测试**零修改**通过（I2 的机器证明）；② 新增序列化往返测试：旧 pack.json/旧 FrozenDesign 样本反序列化 → 再序列化，逐字节相同；③ 新类型各有非法样本被校验拒绝的负测试。

**波 1：效果与图打通编译链** — 约 5-6 人日
C0/C1/C2/C4/C6 按 §4.4 改；design_notes 全链贯通。
验收：① lane_defense 金样项目重放：C0-C6 产物 diff 仅限新增字段（design_notes/graphs 空数组），既有字段逐字节不变；② 新建测试项目覆盖 9 个新效果变体各至少 1 条机制，C4 对每条产出非空 GWT，`ModifyRule` 悬空样本被 C0 拦截；③ 图参数项目：对话树样例经 C0 产出 GraphSpec 且 R4 锚定闭合。

**波 2：custom_mechanic 入口 + 红线扩展** — 约 3 人日
engine API + CLI 子命令 + R5 扫描扩展 + gate4 逐条处置。
验收：① 审计 B 的两个机制（可编程索敌/波次自适应变异）用 add_custom_mechanic 重录，跑全链后在 C4 各有一个能力契约、C6 各有程序任务——**以审计 B 的失败案例为回归测试**；② custom 机制未处置时 gate4 block 的负测试。

**波 3：系统模块 + 重度 + 组合校验** — 约 6-8 人日
system_module.rs 全量、加载器实例化、tier 合成决策点、组合校验器、gate2 合流；制作 sys.equipment（§2.3 全谱系）与 sys.inventory 两个标定模块。
验收：① 同一 sys.equipment 被两个测试 pack 引用，各选不同档位，激活点集合与完成度分母符合 tier_gate 声明；② 装备 heavy + 背包 light 触发 V2 传导违例、双重度触发 V3、签字豁免通过——三条校验器测试；③ 换挡（heavy→light）后已答的重档点按既有失活语义处理。

**波 4：存量资产迁移** — 约 4-5 人日
两包迁移 + v2 降级 + 模板隔离（映射规则见 7.3/7.4）。
验收：① 迁移后 lane_defense/grid_strategy 项目新建→冻结→C0-C6 全链绿；② 迁移前后冻结同一组选择，GameSpec 语义 diff 为零（id 允许有前缀差异，给出映射表）；③ 完成度分母变化有书面对账单。

**波 5（W8，不在本轮承诺）**：垂直样板——新模型重表达 lane_defense + 杀戮尖塔（审计 ❌ 最多且新设施覆盖最全：DrawFromPool/ModifyRule/Graph 三件全用上）。

总量估计：**波 0-4 约 21-26 人日**，关键路径是波 1 和波 3；波 2 可与波 3 并行。

### 7.3 两包迁移映射规则

机械规则（可写成迁移脚本，人工只审结果）：

1. pack 内 L3 系统点 + 其 unlocks 闭包内的 L4/L5/L6 点，凡语义属于可复用系统（grid_progression → sys.progression；ld/grid 共有的敌人表模式 → sys.roster）→ 提为模块，pack 留 `system_refs` + bindings；品类特有点（grid.terrain_effect_rule）留在 pack。首轮只提两包共通的 2-3 个模块，**宁少勿滥**——边界判据等提案 A（§8 第 1 条）。
2. 迁移点 id 加模块前缀，pack 内 `unlocks`/`conflicts`/`consistency_rules` 的 id 引用由脚本同步重写；脚本输出旧 id→新 id 对照表，作为波 4 验收材料。
3. 既有单档系统模块先声明单档 ladder（rank 0 一档），重度谱系逐模块补——**迁移不强迫一步到位**。

### 7.4 v2 2575 点与 25 份模板的处置

**2575 点 → 提示词库**（不进决策图、不进完成度分母、不进冻结门）：

```rust
// adm4-template 或独立 knowledge 数据，只读资产
#[derive(Serialize, Deserialize)]
pub struct PromptLibrary { pub version: String, pub entries: Vec<PromptEntry> }

#[derive(Serialize, Deserialize)]
pub struct PromptEntry {
    pub id: String,
    pub domain: DomainId, pub node_id: Option<NodeId>,
    pub question: String,
    pub option_labels: Vec<String>,   // 原 130 套选项保留为话术素材
    pub origin: String,               // "v2_checklist"
    /// 消费场景标签：interview_probe（AI 访谈追问）/ review_checklist（人工评审）
    pub usage: Vec<String>,
}
```

`tools/v2_migration` 加转换命令：2575 点冲压为 PromptEntry（去重后按 130 套选项聚类合并，预计收敛到 300 条以内）。`interview.rs` 在访谈追问时可引用 PromptEntry 作为提问素材——问卷的组织价值保留，"设计空间"的名分收回。

**25 份模板**：certification 批量改标 `smoke_test`（一条数据订正脚本），`library.rs` 加载时 smoke_test 模板默认排除在预填/校准之外（`--allow-smoke` 显式放行才可见）。不删除——它们是审计证据。

---

## 8. 对提案 A 的需求清单（缺这些，类型系统空转）

逐条列出：没有这些定义，本提案的对应类型只是有形无魂的容器。

1. **系统边界判据**：装备/背包/宠物是几个模块？给可操作的切分判据（例如"独立的玩家动词集 + 独立的名词命名空间 = 独立系统"），否则 §7.3 的模块提取无从下手。
2. **重度的语义维度与档位命名规范**：`HeavinessLadder` 允许任意档数——每档"该放什么"的设计学标准（子机制数量？心智负担？）由 A 给出，装备系统 §2.3 的三档划分请 A 校正或重标。
3. **core_link 的判定规则**：Strong/Support/Peripheral 目前是设计师自报。A 需给出可核验的判据（如"系统的 provides 名词出现在核心循环机制的 effects 中 → Strong"），我可以把它升级为机器判定替代自报。
4. **传导关系先验库**：哪些系统天然传导哪些（装备→背包、抽卡→货币）？`Induction` 是表达位，内容要 A 的系统库给。
5. **MDA 映射的词汇表**：SystemModule 是否要带 aesthetics/dynamics 标签、标签集是什么，A 定了我加字段（一个 `Vec<String>` 的事，但词汇表不能由工程侧发明）。
6. **重度预算的标定值**：`CompositionBudget` 的系数与各规模档总预算是数据文件，初始值需要 A 拿 10 款审计游戏反推标定（例如把星露谷、MOBA 按新模型摆出来，看总分落在哪）。
7. **10 款游戏的系统清单标定**：Q4 的设计学答卷（每款 = 哪些模块 × 什么档位）由 A 给出，我承诺其中结构化部分（§6 表内设施）全部可编译；对不上的就是融合阶段要谈的真分歧。

---

## 9. 自认风险（替红队先打两拳）

1. **"档位=激活点集"可能把重度降维成内容开关**。重度的设计学本质若含"数值深度/心智负担"这类非点集维度，tier_gate 表达不了——我的辩护是 MeasuredWeight 向量可扩维，但维度语义空缺（依赖 §8 第 2/6 条），红队若拿"两个系统激活点数相同但体感重度天差地别"的反例，本方案的预算校验会失真。
2. **Custom 效果可能成为垃圾场**。逃生舱口的纪律靠"必填 GWT + gate4 强制处置"兜底，但若设计师批量用 Custom 绕开受控变体，C4 产物退化为誊写机——与审计 B 批评的"复述"一步之遥。缓解手段（custom 占比进冻结报告、超阈值警告）治标；根治靠受控变体覆盖面足够广，而 8 个新变体够不够，要等 10 款压力测试回来才知道。
