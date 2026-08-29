# v3 实施子计划 · 07 · 七条红线的机器化落地

> 上位：[00 §5.4](00_master_design.md) · [01 总纲](01_overview_and_milestones.md)
> 性质：横切约束，贯穿 R3–R9 每一步。这是 00 号「三代病灶」的防复发硬约束。
> 落点：证据指针类型进 `adm-new-contracts`；各红线在对应步骤强制。

---

## 0. 为什么单列一文档

三代病灶（00 §1）——指标非测量、fail-open 默认、评审橡皮图章、参考污染——都是**约束缺失**导致，不是功能缺失。红线若只写在文字里会被绕过。本文档把 7 条红线落成**类型 + 编译期/运行期可检查的机制**，让违反红线在代码层就困难。

**这些不是假想威胁——2026-07-26 代码调查逐条坐实了它们仍活在当前活跃路径**（design→plan 链，详见文档 06 §4.3/§4.4）：

| 病灶 | 坐实位置 | 对应红线 |
|------|---------|---------|
| 指标硬编码 | `step08_14.rs:3548` score 恒 100/60；`generic_content_ratio:0.0`/`template_leakage_count:0` 直接写死 | R1 |
| 覆盖率橡皮图章 | `semantic_pipeline.rs:512/517` coverage 恒 = 1.0（`unbound_semantic_items` 硬编码 `[]`） | R1/R6 |
| review 恒 passed | `step08_14.rs:528` 每任务 review 硬编码 `"passed"`；`:3500/3515` blockers 恒 `[]` | R3 |
| 通用 stub 兜底 | `step08_10_v2.rs:635` machine_checks 固定桩、`:405` 任务图硬编码模板 | R2/R7 |
| 真核对被旁路 | `semantic_pipeline.rs:1025` 反向覆盖核对只在 `mod tests` 调用、且取模轮询假覆盖 `:998` | R1/R4 |

v3 的红线机器化就是要让上述每一处**在类型层无法再写出来**。

---

## 1. 七条红线 → 机制映射

### R1 指标即测量

> 任何报告中的分数/计数必须携带证据指针列表（文件+路径+值）；证据为空而指标非零 → 步骤 fail。禁止硬编码指标字面量。

**机制**：所有指标用带证据的类型，不用裸 `f64`/`u32`：

```rust
pub struct MeasuredMetric {
    pub value: MetricValue,
    pub evidence: Vec<EvidencePointer>,  // 空 + value≠0 → 构造即拒或 validate 时 fail
}
pub struct EvidencePointer { pub file: String, pub path: String, pub observed: String }
```

- 落 `adm-new-contracts/src/measured.rs`。
- 报告结构里**不允许出现裸数值指标**（code review 约束 + 一个 lint 测试扫报告类型）。
- `MeasuredMetric::new` 校验：`value != 0 && evidence.is_empty()` → `Err`。

### R2 未知即停

> 派生规则遇未知类型/无法分类输入 → 产出 `blocked` + 待分类清单，绝不给默认值。

**机制**：派生函数返回 `Derive<T>` 而非 `T`：

```rust
pub enum Derive<T> { Resolved(T), Blocked { unknown: Vec<UnclassifiedItem> } }
```

- 禁止 `unwrap_or(default)` 式兜底（如「认不出→art_asset/2048/P0」）。
- C3 视觉白名单、基数分类等所有一对多派生用此类型。
- code review 红线：搜 `unwrap_or`/`unwrap_or_else`/`unwrap_or_default` 在派生路径出现即审查。

### R3 评审最低工作量证明

> 评审步骤必须：reviewed_count = 上游实际条目数（或显式抽样清单）；各评审报告内容哈希互不相同；零发现时逐类别给「查过什么、为什么没问题」的证据指针。

**机制**：

```rust
pub struct ReviewProof {
    pub reviewed_count: usize,        // 必须 == upstream_count 或带 SamplingManifest
    pub upstream_count: usize,
    pub content_hash: String,         // 与同批其它评审比对，全同→fail（橡皮图章）
    pub per_category_evidence: Vec<CategoryEvidence>, // 零发现也要填
}
```

- 用于 C1 红队、C4 评审、逆向 S3 交叉核验、冻结门第 4 道。
- **哈希互异检查**：三份评审内容哈希全同 → 判定橡皮图章 → fail（直击 00 号「三份同哈希+审0条PASS」病灶）。

### R4 AI 产出锚定

> AI 生成的每段叙述/设计声明必须锚定到 GameSpec 决策路径；无锚定内容视为 AI 发明设计，fail。

**机制**：AI 叙述产物用锚定包裹类型，复用 game-spec `TraceLink`：

```rust
pub struct AnchoredNarrative {
    pub text: String,
    pub anchors: Vec<SpecRef>,       // 复用 game-spec::SpecRef，指向 GameSpec 路径
}
```

- C2/C3/C4 的 AI 产出必须是 `AnchoredNarrative`；`anchors.is_empty()` → fail。
- C2 门禁「锚定覆盖率 100%」即对全部声明检查 anchors 非空且路径存在于 GameSpec。

### R5 参考名扫描全程在线

> 换皮词表扫描不只在冻结门，Phase1/2 每步产物都扫，命中即 block。

**机制**：一个横切扫描器，每步产物落盘前过一遍：

```rust
pub struct SkinScanner { wordlist: Vec<String> } // 来自文档 04 §5 词表
impl SkinScanner { pub fn scan(&self, artifact: &str) -> Vec<SkinHit>; }
```

- 词表来源：文档 04 §5 登记的 game_name+aliases（升级自 `forbidden_source_tokens.json`）。
- 挂在 `StageExecutor` 的产物落盘钩子，成本极低。命中→step fail + 列位置。

### R6 基数申报

> 任何一对多派生（实体→资产、机制→需求）必须声明映射规则与丢弃清单；产出数量对照品类包期望区间，超界必须人工确认。

**机制**：

```rust
pub struct CardinalityDeclaration {
    pub rule: String,                 // 映射规则（人读+机器记）
    pub produced: usize,
    pub expected: Range,              // 来自品类包 cardinality_expectations
    pub dropped: Vec<DroppedItem>,    // 丢弃清单（不能静默丢）
}
```

- C3（实体→资产）、C4（机制→需求）强制返回此声明。
- `produced` 超 `expected` → 人工确认门（非 fail，但 block 到确认）。

### R7 fallback 禁令

> 全系统不存在「AI 失败→模板兜底→报 success」路径。AI 失败 = blocked + 明确原因。

**机制**：

- AI 调用返回 `AdmResult<...>`；失败路径**只能**产 `Blocked{reason}`，**不得**转模板兜底后标 success。
- code review 红线：AI 调用的 `Err`/失败分支若接「读模板/取默认」再返回成功状态 → 违规。
- 直击 00 号二代「退化成静默兜底」病灶。**这条与 E4「AI 必需即停」一体**。

---

## 2. 落地节奏

红线不是单独里程碑，而是**每个里程碑的验收项**：

| 里程碑 | 必须强制的红线 |
|--------|--------------|
| R3 逆向工具链 | R3（S3 交叉核验哈希异）、R7（检索/映射失败不造假）、宁缺勿造（源空不填） |
| R5 冻结门 | R2（缺格→blocked）、R3（红队工作量证明）、R5（换皮扫描） |
| R6 C0–C2 | R1（validation/redteam 带证据）、R3、R4（C2 锚定）、R5 |
| R7 C3–C6 | R1、R2（视觉白名单）、R4、R6（基数申报）、R5 |

## 3. R 红线交付清单

| 交付 | 文件 | 验证 |
|------|------|------|
| `MeasuredMetric`/`EvidencePointer` | `adm-new-contracts/src/measured.rs` | 空证据非零→拒的测试 |
| `Derive<T>` | `adm-new-contracts/src/derive.rs` | Blocked 路径测试 |
| `ReviewProof` + 哈希互异检查 | `adm-new-contracts/src/review_proof.rs` | 全同哈希→fail 测试 |
| `AnchoredNarrative` | `adm-new-contracts/src/anchored.rs` | 空锚定→fail 测试 |
| `SkinScanner` + 落盘钩子 | `adm-new-pipeline`（横切） | 命中→block 测试 |
| `CardinalityDeclaration` | `adm-new-contracts/src/cardinality.rs` | 超界→人工门测试 |
| R7 code-review 约束 | 文档化 + CI grep | AI 失败分支无兜底 |

> 这些类型**在 R1 阶段就应先建**（哪怕空实现），让后续每步「只能」用带约束的类型，而不是事后补检查。建议把 §3 的 contracts 类型提前到 R1 一并交付。
