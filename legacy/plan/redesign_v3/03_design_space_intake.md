# v3 实施子计划 · 03 · 设计空间清单输入门

> 上位：[00 §3.4](00_master_design.md) · [01 总纲](01_overview_and_milestones.md)
> 里程碑：R2（清单 schema + 空白模板 + 加载校验）、R4（用逆向工具产出数据）
> 落点：`knowledge/design_data/`（数据）+ `adm-new-design/data_loader/`（加载）
> **核心：这是 E4「设计空间由用户提供」的落地门。我建格式与校验，用户填内容。**

---

## 1. 输入门的形态（开放问题 O1）

E4 定：决策点/选项枚举由用户提供。两种交接方式：

- **O1-甲（模板格式先行，建议）**：R2 先产出**空白清单 schema + 填写指南 + 一个最小样例**；用户按格式填「通用层 + 通道塔防」清单；我加载校验。优点：格式一次对齐，后续战棋包同格式复用；用户填写有明确约束不会跑偏。
- **O1-乙（先给塔防实例）**：用户先口述/给一版塔防清单，我反推格式。缺点：格式随实例走样，战棋包可能要返工。

> **本计划按甲推进**，把「空白清单 schema」作为 R2 的首要交付。若你选乙，R2 顺序对调（先收你的实例，再抽象 schema），交付物不变。

---

## 2. 设计空间清单是什么

它是**决策点与选项的定义源**（不是某个项目的选择）。一份清单 = 一个层集（通用层）或一个品类包（塔防）。填的是：有哪些决策点、每点有哪些选项、每选项的 implications/requires/conflicts、L5/L6 的表结构。

对应文档 02 的 `DecisionPoint`/`DecisionOption`/`ParameterSchema` —— 清单就是这些类型的**外部 JSON 序列化**，用户（或逆向工具）填，加载器读成 Rust 类型。

```jsonc
// design_space/lane_defense/pack.json（示意，字段对齐文档 02 类型）
{
  "pack_id": "lane_defense",
  "pack_version": "0.1.0",
  "reference_games": ["<游戏A>", "<游戏B>", "<游戏C>"],   // ≥3，交叉验证来源
  "cardinality_expectations": {                            // 基数期望表，接 C3 基数门
    "guard_types": { "min": 4, "max": 8 },
    "wave_count": { "min": 8, "max": 20 }
  },
  "decision_points": [
    {
      "id": "ld.counter_matrix",
      "domain": "gameplay_system",
      "level": "L5",
      "genre_scope": { "pack": "lane_defense" },
      "question": "克制关系如何组织？",
      "options": [
        {
          "id": "matrix_full",
          "label": "全克制矩阵",
          "summary": "每种守卫对每种敌人有独立系数",
          "implications": ["数值表规模 = 守卫数 × 敌人数", "平衡成本高"],
          "requires": [], "conflicts": [],
          "parameter_schema": {
            "matrix": {
              "row_axis": "ld.guard_types", "col_axis": "ld.enemy_types",
              "cell": { "key": "coeff", "kind": "float", "constraint": {"range":[0.0,3.0]} }
            }
          }
        }
      ],
      "skin_fields": [],
      "evidence_slots": true,
      "requirement": "unlocked"      // unlocked(默认,纯激活驱动) | baseline(基线,可被显式 N/A 跳过)
    }
  ]
}
```

> `requirement` 字段落地文档 02 §3.5 的适用性判定：默认 `unlocked`（只有父选项 unlock 才需回答，简单玩法天然少填）；少数真正基线的点标 `baseline`（建议回答，但项目可给结构化理由码显式 N/A）。品类包设计时**慎用 baseline**——绝大多数点应靠激活驱动，避免给简单项目强加负担。

---

## 3. 品类包架构（00 §3.4 落地）

三层组织，都用同一清单格式：

| 层 | 内容 | genre_scope | 文件 |
|----|------|------------|------|
| 通用层 | 跨品类决策点（节奏/视角/会话长度/经济骨架/难度哲学）+ L0–L2 全部 | `universal` | `design_space/universal/*.json` |
| 品类包 | L3–L6 品类专属决策点与选项 + 基数期望表 | `pack:<id>` | `design_space/<pack>/pack.json` |
| 参考答卷 | ≥3 款该品类成熟游戏的逆向答卷（校准用，非项目目标） | — | `design_space/<pack>/references/*.json`（逆向工具产，文档 04） |

**≥3 款参考交叉验证的机制**（00 §3.4）：

- 品类包的**选项空间**与**基数区间**必须由 ≥3 款参考游戏横向对照校准，防止被单一游戏带偏。
- 落地为一个**交叉验证报告**：对每个决策点，列出 3 款参考各自选了哪个选项（来自文档 04 的逆向答卷）；若某选项只有 1 款覆盖 → 标记「弱证据选项」，提示品类包维护者复核是否是「那一款的形状」。
- 基数区间取 3 款参考的实际值域并集 ± 合理外扩，写入 `cardinality_expectations`。

> 参考游戏是**校准来源**不是复刻目标——清单里 `reference_games` 只记名字用于换皮词表登记（文档 04 §5），选项空间是三者的**并集抽象**而非任一的照抄。

---

## 4. 加载与校验（R2 交付）

在 `adm-new-design/data_loader/` 旁加一个 `design_space/` 加载器（与现 `DesignDataLoader` 并存，不改旧路径）：

```rust
pub struct DesignSpaceLoader { root: PathBuf } // knowledge/design_data/design_space

impl DesignSpaceLoader {
    pub fn load_universal(&self) -> AdmResult<Vec<DecisionPoint>>;
    pub fn load_pack(&self, id: &GenrePackId) -> AdmResult<GenrePack>;
    pub fn validate(&self, pack: &GenrePack) -> DesignSpaceReport; // 结构 + 交叉验证
}
```

校验规则（R2 必须实现）：
1. **schema 合法**：字段齐、level/genre_scope/kind 枚举合法（serde + serde_path_to_error，复用 game-spec 的错误定位风格）。
2. **DAG 合法**：复用文档 02 §4 校验器（无环、requires 可达、conflicts 对称）。
3. **参考覆盖**：`reference_games.len() >= 3`，否则 `blocked`（00 §3.4 硬要求）。
4. **矩阵轴引用合法**：`MatrixSchema.row_axis/col_axis` 指向的决策点存在且其选项集可枚举。
5. **基数期望完整**：品类包的每个 `Table`/`Matrix` 参数在 `cardinality_expectations` 有对应区间（供 C3 基数门，红线 R6）。

---

## 5. R2 / R4 交付清单

**R2（格式 + 校验，不含真实内容）**：

| 交付 | 文件 | 验证 |
|------|------|------|
| 清单 JSON schema（正式） | `knowledge/schemas/design_space.schema.json` | schema 自检 |
| 空白模板 + 填写指南 | `design_space/_template/pack.blank.json` + `README.md` | 人读可填 |
| 最小样例（1 域 2 决策点） | `design_space/_example/mini.json` | 加载校验通过 |
| `DesignSpaceLoader` + 校验 | `adm-new-design/src/data_loader/design_space.rs` | 单元测试（含负例：<3 参考→blocked） |
| ⛳ **用户填一版塔防清单** | `design_space/lane_defense/pack.json` | 加载校验通过 = R2 完成 |

**R4（用文档 04 工具产出参考数据）**：

| 交付 | 文件 | 验证 |
|------|------|------|
| 塔防 ≥3 参考答卷 | `design_space/lane_defense/references/*.json` | 来源真实性抽查 |
| 交叉验证报告 | `design_space/lane_defense/cross_validation.md` | 每决策点 ≥ 覆盖数标注；弱证据选项列出 |
| 基数期望回填 | `pack.json.cardinality_expectations` | 区间来自 3 款实测 |

> **R2 与 R4 之间隔着 R3（逆向工具链）**：R2 的塔防清单是**用户手工填的选项空间骨架**；R4 用逆向工具产出的参考答卷**校准/补全**这个骨架。二者不是同一份数据，先后关系见 01 号 §2 里程碑表。
