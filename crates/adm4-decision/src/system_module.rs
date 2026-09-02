//! 系统模块类型底座（W7 定稿 §2.2 / §3.1-3.5 / §4.1 / §4.4 / §5.1，T-W7-0a）。
//!
//! 为什么单独成文件：`SystemModule` 是 W7 引入的知识层一等资产
//! （`knowledge/systems/<module_id>/module.json`），它与 types.rs 里的决策点清单是
//! 「库 → 项目」的供给关系——模块声明名词接口、重度阶梯与决策点，加载器
//! （T-W7-3a）把它实例化进设计空间，组合校验器（T-W7-0c）消费它的接口与档位。
//! 本卡只落类型与结构自校验，不做加载器、不做组合校验。
//!
//! serde 纪律（I2 旧档守恒）：所有结构体 `#[serde(default)]`，缺键的旧 JSON
//! （乃至 `{}`）必须能反序列化为默认值；枚举 snake_case tag，与既有
//! `ParameterSchema`/`SelectionMode` 风格一致。

use crate::types::{DecisionId, DecisionPoint};
use adm4_contracts::CardinalityRange;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// 名词 id。带点号（如 `sys.loot.drop_table`）视为外部命名空间引用，
/// 裸 id 必须在本模块 `nouns` 里声明——见 `SystemModule::validate`。
pub type NounId = String;
/// 重度档 id。档位的高低（rank）由其在 `HeavinessLadder.tiers` 中的序号决定，
/// 不单独存储——避免序号与声明顺序两处真相（定稿 §3.1：跨模块比较走全局档带，
/// rank 只在模块内部有意义）。
pub type TierId = String;

/// 四类名词 + 规则挂点（定稿 §2.2 类型落点）。
///
/// 为什么是五分而不是表格四分：`RuleSlot` 是允许其他系统 `ModifyRule` 作用的
/// 规则挂点，它不是「事件/修饰器」的子类而是接口位——没有它，跨系统改规则
/// （遗物改抽牌规则）就没有可声明的作用目标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NounKind {
    /// 资源：可计数、可增减、可转移，有余额语义（金币、体力、行动点）。
    #[default]
    Resource,
    /// 实体类：有行标识 + 属性列的对象实例类（一件装备、一张卡、一个敌人）。
    EntityClass,
    /// 属性：挂在某实体/系统上的可读写量；`of` 指明宿主（如战斗属性挂在角色上）。
    Property { of: String },
    /// 事件信号：时点信号带载荷，有发生时刻、无余额（击杀、升级、波次结束）。
    Signal,
    /// 规则挂点：允许他系统 `ModifyRule` 作用的规则位（抽牌规则、掉落规则）。
    RuleSlot,
}

/// 名词声明：系统间交换的最小语义单元（定稿 §2.2）。
///
/// 接口边、名词绑定、悬空消费检查（V6）都以名词 id 为锚——名词必须先声明后引用，
/// 否则「探针列静默放行」类缺陷无从拦截。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NounDecl {
    pub id: NounId,
    pub kind: NounKind,
    pub label_zh: String,
    pub summary: String,
}

/// 三类端口（定稿 §2.2）：一条接口边 =（源系统, 端口, 名词, 目标系统）。
///
/// 系统实例图以它为边集，是组合校验器（R-C1′/R-C3/V6）的唯一输入——
/// 这里只声明本模块的端口名词，成边与绑定在加载器（3a）完成。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SystemInterface {
    /// 本系统对外供给的名词（供他系统 consumes）。
    pub provides: Vec<NounId>,
    /// 本系统消费的名词（无人 provides 且非 pack 核心 → V6 加载失败，3a 实现）。
    pub consumes: Vec<NounId>,
    /// 本系统修改的名词（他系统属性/规则挂点）。
    pub modifies: Vec<NounId>,
}

/// 全局档带（定稿 §3.2）：轻 0-4 / 中 5-8 / 重 9-12 / 极重 13-15。
///
/// 为什么用档带而不用档位 rank 跨模块比较：rank 是模块局部序号（装备 4 档、
/// 音游 2 档），跨模块不可比——红队已证 `heavy_rank_threshold` 为坏类型并废除，
/// 重核判定一律走全局档带（W≥9 即重）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HeavinessBand {
    #[default]
    Light,
    Medium,
    Heavy,
    UltraHeavy,
}

impl HeavinessBand {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Light => "轻",
            Self::Medium => "中",
            Self::Heavy => "重",
            Self::UltraHeavy => "极重",
        }
    }
}

/// 五维标定 W(S) = M + D + C + P + O，各维 0-3（定稿 §3.2 量表）。
///
/// M 机制深度 / D 数据深度 / C 耦合宽度 / P 决策形态 / O 内容供给。
/// 字段用单字母：与定稿量表、谱系表逐格对应（"M2 D2 C2 P3 O2"），改成长名
/// 反而切断与标定文档的可核对性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FiveAxisRating {
    pub m: u8,
    pub d: u8,
    pub c: u8,
    pub p: u8,
    pub o: u8,
}

impl FiveAxisRating {
    /// 总分 W。返回 u16 而非 u8：未过校验的数据可能各维 >3，u8 求和会溢出
    /// ——库代码禁 panic，宁可放宽返回类型也不做截断兜底。
    pub fn total(&self) -> u16 {
        u16::from(self.m)
            + u16::from(self.d)
            + u16::from(self.c)
            + u16::from(self.p)
            + u16::from(self.o)
    }

    /// 全局档带（定稿 §3.2：轻 0-4 / 中 5-8 / 重 9-12 / 极重 ≥13）。
    pub fn band(&self) -> HeavinessBand {
        match self.total() {
            0..=4 => HeavinessBand::Light,
            5..=8 => HeavinessBand::Medium,
            9..=12 => HeavinessBand::Heavy,
            _ => HeavinessBand::UltraHeavy,
        }
    }

    /// 各维必须落在 0-3 取值域（量表定义域，超出即标定数据笔误）。
    fn ensure_dimensions_in_range(&self, context: &str) -> Adm4Result<()> {
        let dimensions = [
            ("M", self.m),
            ("D", self.d),
            ("C", self.c),
            ("P", self.p),
            ("O", self.o),
        ];
        for (name, value) in dimensions {
            if value > 3 {
                return Err(Adm4Error::validation(format!(
                    "{context}：五维评分 {name}={value} 超出量表取值域 0-3"
                )));
            }
        }
        Ok(())
    }
}

/// 重度传导目标（定稿 §4.4，裁决 4：名词析取）。
///
/// 为什么不只点名模块：「装备 heavy 强制点名 sys.economy」会对无经济系统但有
/// 掉落回收的游戏误报 V1——`NounProvided` 表达「任一实例 provides 该名词即满足」
/// 的析取语义（宝石从掉落**或**商店任一来源提供都算有源）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InductionTarget {
    /// 点名模块：组合中该模块实例必须存在且档位不低于 `Induction.min_tier`。
    Module(String),
    /// 点名名词：组合中任一实例 provides 该名词即满足（析取语义）。
    NounProvided(NounId),
}

/// `#[default]` 只支持单元变体，这里两个变体都带数据，手写 Default——
/// 空模块名仅为满足容器 `#[serde(default)]` 的旧档兼容，语义上等于「未填」，
/// 由 0c 组合校验按 V1 拦截。
impl Default for InductionTarget {
    fn default() -> Self {
        Self::Module(String::new())
    }
}

/// 重度传导（定稿 §4.4，R-C3 的声明载体）：本模块处于 `when_tier`（含以上）时，
/// 对目标的最低档位要求。传导走接口边不是玄学感应——`reason` 进 finding 文案，
/// 让被拦的人看得懂「为什么装备 E4 要求经济系统有回收回路」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Induction {
    /// 本模块达到哪个档位时触发本条传导。
    pub when_tier: TierId,
    pub target: InductionTarget,
    /// 目标模块须达到的最低档位；目标为 `NounProvided` 时无档位语义，留空。
    pub min_tier: TierId,
    pub reason: String,
}

/// 重度阶梯中的一档（定稿 §3.1/§3.4：每档 = 五维标定 + P 下限承诺 +
/// C 维接口边下限 + 激活点集合 + 传导要求）。
///
/// rank 不落字段：档位高低由其在 `HeavinessLadder.tiers` 中的序号决定，
/// 存两处必然漂移。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HeavinessTier {
    pub id: TierId,
    pub label_zh: String,
    /// 本档五维标定（定稿 §3.4 谱系表的"五维"列）。
    pub rating: FiveAxisRating,
    /// P 维下限承诺（指令 5：每档写死 P 下限，把心智负担从测量问题转为档位定义问题）。
    pub p_floor: u8,
    /// C 维接口边下限（指令 5：C 是组合相对量，谱系写 C≥n 下限、实例图上实测）。
    pub interface_floor: u8,
    /// 本档激活的决策点 id（累计口径：高档 ⊇ 低档，tier 合成点 unlocks 的数据源）。
    pub activates: Vec<DecisionId>,
    /// 本档（含以上）对其他系统的传导要求（定稿 §4.4）。
    pub inductions: Vec<Induction>,
    pub summary: String,
}

/// C 维声明档对应的接口边数上界（量表：0→≤1 / 1→2-3 / 2→4-6 / 3→≥7 无上界）。
///
/// 用途：`interface_floor` 是"边数下限承诺"，它不得超出声明 C 档的边数区间上界
/// ——声明 C1（2-3 边）却承诺下限 9 条边是自相矛盾的标定。
fn c_dimension_edge_cap(c: u8) -> Option<u8> {
    match c {
        0 => Some(1),
        1 => Some(3),
        2 => Some(6),
        _ => None,
    }
}

/// 模块局部重度阶梯（定稿 §3.1：档数与命名由模块作者定，装备 4+ 档、
/// 音游判定 2 档均合法）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HeavinessLadder {
    /// 由轻到重排列；序号即 rank。
    pub tiers: Vec<HeavinessTier>,
}

impl HeavinessLadder {
    /// 档位 rank（在阶梯中的序号，0 为最轻档）；未知档 id 返回 None，
    /// 由调用方决定报错口径（R2：不得默认成 0 档兜底）。
    pub fn tier_rank(&self, id: &str) -> Option<usize> {
        self.tiers.iter().position(|tier| tier.id == id)
    }

    /// 结构自校验：档 id 唯一、五维落域、双下限合法、activates 递增包含。
    ///
    /// 为什么 activates 必须递增包含：档位语义是「累计谱系」（E2 = E0+E1 的全部
    /// 内容再加构筑层），高档丢低档的点意味着升档反而关闭已有设计问题——
    /// 这只可能是标定笔误，必须在入库时拦下而不是等换挡失活时暴露。
    pub fn validate(&self) -> Adm4Result<()> {
        let mut seen_ids = BTreeSet::new();
        for tier in &self.tiers {
            if tier.id.trim().is_empty() {
                return Err(Adm4Error::validation(
                    "重度阶梯存在空档位 id（每档必须有可引用的 id）".to_string(),
                ));
            }
            if !seen_ids.insert(tier.id.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "重度档 id 重复：{}（tier_gate 与传导都按 id 引用，重复即歧义）",
                    tier.id
                )));
            }
            let context = format!("重度档 {}", tier.id);
            tier.rating.ensure_dimensions_in_range(&context)?;
            if tier.p_floor > 3 {
                return Err(Adm4Error::validation(format!(
                    "{context}：p_floor={} 超出 P 维取值域 0-3",
                    tier.p_floor
                )));
            }
            if tier.rating.p < tier.p_floor {
                return Err(Adm4Error::validation(format!(
                    "{context}：P 维评分 {} 低于本档承诺下限 p_floor={}（档位承诺必须被自身标定兑现）",
                    tier.rating.p, tier.p_floor
                )));
            }
            if let Some(cap) = c_dimension_edge_cap(tier.rating.c)
                && tier.interface_floor > cap
            {
                return Err(Adm4Error::validation(format!(
                    "{context}：接口边下限 interface_floor={} 超出 C{} 档的边数上界 {cap}（下限承诺与 C 维声明自相矛盾）",
                    tier.interface_floor, tier.rating.c
                )));
            }
        }
        let mut previous: Option<(&HeavinessTier, BTreeSet<&str>)> = None;
        for tier in &self.tiers {
            if tier.activates.is_empty() {
                continue;
            }
            let current: BTreeSet<&str> = tier.activates.iter().map(String::as_str).collect();
            if let Some((previous_tier, previous_set)) = &previous
                && let Some(missing) = previous_set.iter().find(|id| !current.contains(*id))
            {
                return Err(Adm4Error::validation(format!(
                    "重度档 {} 的 activates 缺少低档 {} 的激活点 {missing}（高档必须递增包含低档）",
                    tier.id, previous_tier.id
                )));
            }
            previous = Some((tier, current));
        }
        Ok(())
    }
}

/// MDA 三层映射（定稿 §2.3）。D 层按裁决 12 降为文档纪律（谓词可观测性不可
/// 静态判定，假装机检会制造覆盖假象），故 `dynamics_notes` 只存不校验；
/// A 层保留机检位：主项 ≤2（八分类限选，超了说明作者没做取舍）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MdaMapping {
    /// M 层摘要：本系统 L4 决策点承载什么机制（点清单本身在 `decision_points`）。
    pub mechanics_summary: String,
    /// D 层：预期运行时动态，"谓词 + 可观测信号"书写（写作纪律，人审不机检）。
    pub dynamics_notes: Vec<String>,
    /// A 层：MDA 八分类主项，≤2（`validate` 拦截）。
    pub aesthetics_primary: Vec<String>,
}

impl MdaMapping {
    pub fn validate(&self) -> Adm4Result<()> {
        if self.aesthetics_primary.len() > 2 {
            return Err(Adm4Error::validation(format!(
                "MDA A 层主项 {} 个超过上限 2（八分类限选，主项过多等于没有取舍）",
                self.aesthetics_primary.len()
            )));
        }
        Ok(())
    }
}

/// 跨决策一致性规则（与 `adm4-space::ConsistencyRule` JSON 同形）。
///
/// 为什么在这里再落一份类型：任务卡预期复用既有类型，但 `ConsistencyRule` 实际
/// 定义在 `adm4-space`——它依赖本 crate，反向引用会成环。模块作为知识层资产又
/// 必须能声明这类规则，故按**字段名与 serde tag 逐字节同形**镜像一份：module.json
/// 里的规则值可原样搬进 GenrePack 合并（加载器 3a 只做 serde 级转换），后续若把
/// 类型上移到本 crate、adm4-space 改 re-export，数据零迁移。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsistencyRule {
    pub id: String,
    #[serde(flatten)]
    pub kind: ConsistencyRuleKind,
}

/// 规则种类（与 `adm4-space::ConsistencyRuleKind` 同形，见 `ConsistencyRule` 说明）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsistencyRuleKind {
    /// 矩阵行轴集合必须与某表的行集合一致。
    MatrixAxisMatchesTableRows {
        matrix_decision: DecisionId,
        table_decision: DecisionId,
    },
    /// 两个决策点必须同时被回答或同时不适用。
    AnsweredTogether {
        first: DecisionId,
        second: DecisionId,
    },
    /// 跨表外键：源表某列的取值必须落在目标表行键列的取值集合内。
    RowReference {
        source_decision: DecisionId,
        source_column: String,
        target_decision: DecisionId,
        target_key_column: String,
    },
}

/// 系统模块：知识层一等资产（定稿 §5.1 字段清单）。
///
/// 一个模块 = 名词接口 + MDA 映射 + 重度阶梯 + 决策点包 + 基数/一致性声明。
/// 无痛接入纪律（§9.2b 第 3 条）：后续新增系统 = 新增一个 module.json，
/// 不改类型系统、不改校验器、不改编译器——本类型是该纪律的结构保证。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SystemModule {
    pub module_id: String,
    /// 语义化版本；进冻结哈希（module_versions，3a 实现），保证复演可追溯。
    pub semver: String,
    pub label_zh: String,
    pub summary: String,
    /// 本模块声明的名词（接口引用的裸 id 必须在此声明）。
    pub nouns: Vec<NounDecl>,
    pub interface: SystemInterface,
    pub mda: MdaMapping,
    pub heaviness: HeavinessLadder,
    /// 模块自带的决策点包；id 强制 `<module_id>.` 前缀（加载器按命名空间重写实例化）。
    pub decision_points: Vec<DecisionPoint>,
    /// 表/矩阵行数期望（与 GenrePack 同键语义，加载时合并）。
    pub cardinality_expectations: BTreeMap<String, CardinalityRange>,
    /// 跨决策一致性规则（加载时并入 pack 规则集）。
    pub consistency_rules: Vec<ConsistencyRule>,
    /// 属于「皮」的字段路径（换皮门比对粒度；幻化这类纯皮内容落这里而非机制点）。
    pub skin_fields: Vec<String>,
}

impl SystemModule {
    /// 结构自校验（入库门槛，加载器与 CLI 校验共同复用）：
    /// module_id 非空、决策点带模块前缀、接口名词有声明、档位激活点存在、
    /// tier_gate 指向真实档位，外加 MDA 与阶梯的自校验。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.module_id.trim().is_empty() {
            return Err(Adm4Error::validation(
                "系统模块缺少 module_id（模块 id 是名词命名空间与决策点前缀的锚）".to_string(),
            ));
        }
        let context = format!("系统模块 {}", self.module_id);
        self.mda
            .validate()
            .map_err(|error| Adm4Error::validation(format!("{context}：{}", error.message)))?;
        self.heaviness
            .validate()
            .map_err(|error| Adm4Error::validation(format!("{context}：{}", error.message)))?;

        let prefix = format!("{}.", self.module_id);
        let mut point_ids = BTreeSet::new();
        for point in &self.decision_points {
            if !point.id.starts_with(&prefix) {
                return Err(Adm4Error::validation(format!(
                    "{context}：决策点 {} 未带模块前缀 {prefix}（前缀即命名空间，缺了多实例会互相踩 id）",
                    point.id
                )));
            }
            point_ids.insert(point.id.as_str());
        }

        let declared_nouns: BTreeSet<&str> =
            self.nouns.iter().map(|noun| noun.id.as_str()).collect();
        let ports = [
            ("provides", &self.interface.provides),
            ("consumes", &self.interface.consumes),
            ("modifies", &self.interface.modifies),
        ];
        for (port, noun_ids) in ports {
            for noun_id in noun_ids {
                // 带点号 = 外部命名空间名词（绑定悬空与否由加载器 V6 判），裸 id 必须本地声明。
                if noun_id.contains('.') {
                    continue;
                }
                if !declared_nouns.contains(noun_id.as_str()) {
                    return Err(Adm4Error::validation(format!(
                        "{context}：接口 {port} 引用的名词 {noun_id} 未在 nouns 声明（外部名词须写成带点号的命名空间形式）"
                    )));
                }
            }
        }

        for tier in &self.heaviness.tiers {
            for activated in &tier.activates {
                if !point_ids.contains(activated.as_str()) {
                    return Err(Adm4Error::validation(format!(
                        "{context}：重度档 {} 激活的决策点 {activated} 不存在（悬空激活会让档位承诺落空）",
                        tier.id
                    )));
                }
            }
        }

        let tier_ids: BTreeSet<&str> = self
            .heaviness
            .tiers
            .iter()
            .map(|tier| tier.id.as_str())
            .collect();
        for point in &self.decision_points {
            if let Some(gate) = &point.tier_gate
                && !tier_ids.contains(gate.as_str())
            {
                return Err(Adm4Error::validation(format!(
                    "{context}：决策点 {} 的 tier_gate={gate} 不在重度阶梯档位中（门控指向不存在的档等于永不激活）",
                    point.id
                )));
            }
        }
        Ok(())
    }
}

/// 追问弹药条目（v2 2575 点降级而成的取舍问句，波 4 全量填充）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PromptEntry {
    pub id: String,
    /// 领域/系统归属（机制访谈按当前系统取弹药）。
    pub domain: String,
    pub question_zh: String,
    pub follow_ups: Vec<String>,
    /// 来源引用（v2 点 id 或文档锚），保证问句可溯源不是编的。
    pub source_ref: String,
}

/// 追问弹药库（执行计划：类型本卡前移，内容波 4 填充；3f 先种子化 20-30 条）。
///
/// 为什么现在就落类型：3d 机制访谈的接口要在波 3 定型，若类型等内容一起来，
/// 访谈卡会被迫自造临时结构再返工。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PromptLibrary {
    pub entries: Vec<PromptEntry>,
}

/// 与核心循环的关联强度 κ 四级（定稿 §4.1，裁决 9）。
///
/// 判定全部是纯结构判据（core_loop 动词绑定 / 接口边），机器可判无自由裁量；
/// `#[default]` 取 Weak：未判定的系统按"可绕开"保守计权，宁可低估贡献
/// 也不虚增预算占用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CoreLink {
    /// 系统机制出现在 core_loop 动词序列内。
    Core,
    /// provides 核心循环消费的名词，或 modifies core 系统属性。
    Strong,
    /// 可绕开或影响间接（摘除后核心循环仍完整转动）。
    #[default]
    Weak,
    /// 只在局外生效。
    Meta,
}

impl CoreLink {
    /// R-C2 预算权重（定稿 §4.1 表）：B(G) = Σ W(S) × weight(κ)。
    pub fn weight(&self) -> f64 {
        match self {
            Self::Core => 1.0,
            Self::Strong => 0.75,
            Self::Weak => 0.5,
            Self::Meta => 0.25,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Core => "核心",
            Self::Strong => "强关联",
            Self::Weak => "弱关联",
            Self::Meta => "局外",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DecisionOption, DecisionPoint, DesignLevel, GenreScope, PointRequirement, SelectionMode,
    };

    fn rating(m: u8, d: u8, c: u8, p: u8, o: u8) -> FiveAxisRating {
        FiveAxisRating { m, d, c, p, o }
    }

    fn tier(
        id: &str,
        rating: FiveAxisRating,
        p_floor: u8,
        interface_floor: u8,
        activates: &[&str],
    ) -> HeavinessTier {
        HeavinessTier {
            id: id.into(),
            label_zh: id.into(),
            rating,
            p_floor,
            interface_floor,
            activates: activates.iter().map(|point| (*point).to_string()).collect(),
            inductions: Vec::new(),
            summary: String::new(),
        }
    }

    fn point(id: &str, tier_gate: Option<&str>) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "equipment".into(),
            level: DesignLevel::L4,
            genre_scope: GenreScope::Universal,
            question: format!("{id}？"),
            mda_layer: None,
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: tier_gate.map(Into::into),
            options: vec![
                DecisionOption {
                    id: "option_a".into(),
                    label: "甲".into(),
                    ..Default::default()
                },
                DecisionOption {
                    id: "option_b".into(),
                    label: "乙".into(),
                    ..Default::default()
                },
            ],
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    /// 定稿 §3.4 装备 E0-E6 谱系真实标定数据（含累计 activates 与 §4.4 传导）。
    fn equipment_ladder() -> HeavinessLadder {
        let e0_points = ["sys.equipment.slot_rule"];
        let e1_points = [
            "sys.equipment.slot_rule",
            "sys.equipment.quality_tier",
            "sys.equipment.affix_roll",
        ];
        let e2_points = [
            "sys.equipment.slot_rule",
            "sys.equipment.quality_tier",
            "sys.equipment.affix_roll",
            "sys.equipment.skill_trigger",
            "sys.equipment.skill_bind",
        ];
        let e3_points = [
            "sys.equipment.slot_rule",
            "sys.equipment.quality_tier",
            "sys.equipment.affix_roll",
            "sys.equipment.skill_trigger",
            "sys.equipment.skill_bind",
            "sys.equipment.socket_slots",
            "sys.equipment.socket_ops",
        ];
        let e4_points = [
            "sys.equipment.slot_rule",
            "sys.equipment.quality_tier",
            "sys.equipment.affix_roll",
            "sys.equipment.skill_trigger",
            "sys.equipment.skill_bind",
            "sys.equipment.socket_slots",
            "sys.equipment.socket_ops",
            "sys.equipment.craft_recipe",
            "sys.equipment.salvage",
        ];
        let e5_points = [
            "sys.equipment.slot_rule",
            "sys.equipment.quality_tier",
            "sys.equipment.affix_roll",
            "sys.equipment.skill_trigger",
            "sys.equipment.skill_bind",
            "sys.equipment.socket_slots",
            "sys.equipment.socket_ops",
            "sys.equipment.craft_recipe",
            "sys.equipment.salvage",
            "sys.equipment.enhance_odds",
            "sys.equipment.enhance_guard",
            "sys.equipment.enhance_cost",
        ];
        let e6_points = [
            "sys.equipment.slot_rule",
            "sys.equipment.quality_tier",
            "sys.equipment.affix_roll",
            "sys.equipment.skill_trigger",
            "sys.equipment.skill_bind",
            "sys.equipment.socket_slots",
            "sys.equipment.socket_ops",
            "sys.equipment.craft_recipe",
            "sys.equipment.salvage",
            "sys.equipment.enhance_odds",
            "sys.equipment.enhance_guard",
            "sys.equipment.enhance_cost",
            "sys.equipment.set_bonus",
        ];
        let mut e1 = tier("e1_quality_affix", rating(1, 1, 1, 2, 2), 2, 3, &e1_points);
        e1.inductions = vec![Induction {
            when_tier: "e1_quality_affix".into(),
            target: InductionTarget::Module("sys.loot".into()),
            min_tier: "quality_affix_weights".into(),
            reason: "词条从哪来必须有规则（掉落表含权重列）".into(),
        }];
        let mut e2 = tier("e2_skill_build", rating(2, 2, 2, 3, 2), 3, 5, &e2_points);
        e2.inductions = vec![Induction {
            when_tier: "e2_skill_build".into(),
            target: InductionTarget::NounProvided("skill_effect_def".into()),
            min_tier: String::new(),
            reason: "任一实例 provides 技能效果定义；无则 E2 档非法".into(),
        }];
        let mut e3 = tier("e3_socket", rating(3, 2, 2, 3, 2), 3, 6, &e3_points);
        e3.inductions = vec![
            Induction {
                when_tier: "e3_socket".into(),
                target: InductionTarget::NounProvided("gem_entity".into()),
                min_tier: String::new(),
                reason: "宝石必须有源（掉落或商店任一提供即满足——析取语义）".into(),
            },
            Induction {
                when_tier: "e3_socket".into(),
                target: InductionTarget::Module("sys.inventory".into()),
                min_tier: "classify".into(),
                reason: "新名词必须有存放".into(),
            },
        ];
        let mut e4 = tier("e4_craft", rating(3, 3, 3, 3, 3), 3, 9, &e4_points);
        e4.inductions = vec![
            Induction {
                when_tier: "e4_craft".into(),
                target: InductionTarget::NounProvided("material_entity".into()),
                min_tier: String::new(),
                reason: "材料必须有源".into(),
            },
            Induction {
                when_tier: "e4_craft".into(),
                target: InductionTarget::Module("sys.economy".into()),
                min_tier: "recycle_loop".into(),
                reason: "材料经济成环".into(),
            },
        ];
        HeavinessLadder {
            tiers: vec![
                tier("e0_stat_bonus", rating(0, 0, 1, 1, 1), 1, 2, &e0_points),
                e1,
                e2,
                e3,
                e4,
                tier("e5_enhance", rating(3, 3, 3, 3, 3), 3, 9, &e5_points),
                tier("e6_set_transmog", rating(3, 3, 3, 3, 3), 3, 9, &e6_points),
            ],
        }
    }

    fn equipment_module() -> SystemModule {
        let ladder = equipment_ladder();
        let all_point_ids: Vec<String> = ladder
            .tiers
            .last()
            .map(|last| last.activates.clone())
            .expect("装备谱系夹具至少有一档");
        let decision_points = all_point_ids
            .iter()
            .map(|id| point(id, Some("e0_stat_bonus")))
            .collect();
        SystemModule {
            module_id: "sys.equipment".into(),
            semver: "1.0.0".into(),
            label_zh: "装备".into(),
            summary: "属性加成 → 技能BD → 宝石 → 合成/冶炼 → 强化 → 套装/幻化".into(),
            nouns: vec![
                NounDecl {
                    id: "equipment_entity".into(),
                    kind: NounKind::EntityClass,
                    label_zh: "装备实体".into(),
                    summary: "有行标识与属性列的装备实例".into(),
                },
                NounDecl {
                    id: "combat_attribute".into(),
                    kind: NounKind::Property {
                        of: "combat_unit".into(),
                    },
                    label_zh: "战斗属性".into(),
                    summary: "装备修改的角色属性".into(),
                },
                NounDecl {
                    id: "equip_signal".into(),
                    kind: NounKind::Signal,
                    label_zh: "穿戴事件".into(),
                    summary: "穿脱装备时点信号".into(),
                },
                NounDecl {
                    id: "drop_rule_slot".into(),
                    kind: NounKind::RuleSlot,
                    label_zh: "掉落规则挂点".into(),
                    summary: "允许他系统 ModifyRule 的规则位".into(),
                },
            ],
            interface: SystemInterface {
                provides: vec!["equipment_entity".into(), "equip_signal".into()],
                consumes: vec!["sys.loot.drop_table".into()],
                modifies: vec!["combat_attribute".into()],
            },
            mda: MdaMapping {
                mechanics_summary: "穿戴/词条/镶嵌/合成/强化/套装机制群".into(),
                dynamics_notes: vec!["玩家在装备对比中反复触发替换决策（信号：替换率）".into()],
                aesthetics_primary: vec!["挑战".into(), "表达".into()],
            },
            heaviness: ladder,
            decision_points,
            cardinality_expectations: BTreeMap::new(),
            consistency_rules: vec![ConsistencyRule {
                id: "affix_rows_reference_quality".into(),
                kind: ConsistencyRuleKind::RowReference {
                    source_decision: "sys.equipment.affix_roll".into(),
                    source_column: "quality".into(),
                    target_decision: "sys.equipment.quality_tier".into(),
                    target_key_column: "tier_key".into(),
                },
            }],
            skin_fields: vec!["transmog_appearance".into()],
        }
    }

    // ---------------- serde 往返与旧档可读 ----------------

    #[test]
    fn noun_kind_serde_shapes_and_roundtrip() {
        let kinds = vec![
            NounKind::Resource,
            NounKind::EntityClass,
            NounKind::Property {
                of: "combat_unit".into(),
            },
            NounKind::Signal,
            NounKind::RuleSlot,
        ];
        for kind in &kinds {
            let json = serde_json::to_string(kind).expect("序列化应成功");
            let back: NounKind = serde_json::from_str(&json).expect("反序列化应成功");
            assert_eq!(&back, kind);
        }
        let property = serde_json::to_value(&kinds[2]).expect("序列化应成功");
        assert_eq!(
            property,
            serde_json::json!({ "kind": "property", "of": "combat_unit" })
        );
        let rule_slot = serde_json::to_value(&kinds[4]).expect("序列化应成功");
        assert_eq!(rule_slot, serde_json::json!({ "kind": "rule_slot" }));
    }

    #[test]
    fn induction_target_serde_shapes_and_roundtrip() {
        let module = InductionTarget::Module("sys.loot".into());
        let noun = InductionTarget::NounProvided("gem_entity".into());
        assert_eq!(
            serde_json::to_value(&module).expect("序列化应成功"),
            serde_json::json!({ "module": "sys.loot" })
        );
        assert_eq!(
            serde_json::to_value(&noun).expect("序列化应成功"),
            serde_json::json!({ "noun_provided": "gem_entity" })
        );
        for target in [module, noun] {
            let json = serde_json::to_string(&target).expect("序列化应成功");
            let back: InductionTarget = serde_json::from_str(&json).expect("反序列化应成功");
            assert_eq!(back, target);
        }
        assert_eq!(
            InductionTarget::default(),
            InductionTarget::Module(String::new())
        );
    }

    #[test]
    fn empty_object_deserializes_to_defaults_for_all_structs() {
        // 旧档可读性：缺全部键（乃至 {}）必须能反序列化为默认值。
        assert_eq!(
            serde_json::from_str::<NounDecl>("{}").expect("NounDecl 空对象应可读"),
            NounDecl::default()
        );
        assert_eq!(
            serde_json::from_str::<SystemInterface>("{}").expect("SystemInterface 空对象应可读"),
            SystemInterface::default()
        );
        assert_eq!(
            serde_json::from_str::<FiveAxisRating>("{}").expect("FiveAxisRating 空对象应可读"),
            FiveAxisRating::default()
        );
        assert_eq!(
            serde_json::from_str::<Induction>("{}").expect("Induction 空对象应可读"),
            Induction::default()
        );
        assert_eq!(
            serde_json::from_str::<HeavinessTier>("{}").expect("HeavinessTier 空对象应可读"),
            HeavinessTier::default()
        );
        assert_eq!(
            serde_json::from_str::<HeavinessLadder>("{}").expect("HeavinessLadder 空对象应可读"),
            HeavinessLadder::default()
        );
        assert_eq!(
            serde_json::from_str::<MdaMapping>("{}").expect("MdaMapping 空对象应可读"),
            MdaMapping::default()
        );
        assert_eq!(
            serde_json::from_str::<SystemModule>("{}").expect("SystemModule 空对象应可读"),
            SystemModule::default()
        );
        assert_eq!(
            serde_json::from_str::<PromptEntry>("{}").expect("PromptEntry 空对象应可读"),
            PromptEntry::default()
        );
        assert_eq!(
            serde_json::from_str::<PromptLibrary>("{}").expect("PromptLibrary 空对象应可读"),
            PromptLibrary::default()
        );
    }

    #[test]
    fn system_module_full_roundtrip_preserves_everything() {
        let module = equipment_module();
        let json = serde_json::to_string_pretty(&module).expect("序列化应成功");
        let back: SystemModule = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(back, module);
    }

    #[test]
    fn prompt_library_roundtrip() {
        let library = PromptLibrary {
            entries: vec![PromptEntry {
                id: "equipment_tradeoff_1".into(),
                domain: "sys.equipment".into(),
                question_zh: "词条随机的上下限差距拉多大才既有惊喜又不毁平衡？".into(),
                follow_ups: vec!["极品词条允许翻倍吗？".into()],
                source_ref: "v2:equipment.affix_range".into(),
            }],
        };
        let json = serde_json::to_string(&library).expect("序列化应成功");
        let back: PromptLibrary = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(back, library);
    }

    // ---------------- 五维量表与档带 ----------------

    #[test]
    fn five_axis_total_sums_all_dimensions() {
        assert_eq!(rating(1, 2, 3, 2, 1).total(), 9);
        assert_eq!(FiveAxisRating::default().total(), 0);
        assert_eq!(rating(3, 3, 3, 3, 3).total(), 15);
    }

    #[test]
    fn heaviness_band_boundaries_match_global_scale() {
        // 边界对（定稿 §3.2）：4/5、8/9、12/13。
        assert_eq!(rating(1, 1, 1, 1, 0).band(), HeavinessBand::Light); // 4
        assert_eq!(rating(1, 1, 1, 1, 1).band(), HeavinessBand::Medium); // 5
        assert_eq!(rating(2, 2, 2, 2, 0).band(), HeavinessBand::Medium); // 8
        assert_eq!(rating(2, 2, 2, 2, 1).band(), HeavinessBand::Heavy); // 9
        assert_eq!(rating(3, 3, 3, 3, 0).band(), HeavinessBand::Heavy); // 12
        assert_eq!(rating(3, 3, 3, 3, 1).band(), HeavinessBand::UltraHeavy); // 13
        assert_eq!(FiveAxisRating::default().band(), HeavinessBand::Light); // 0
        assert_eq!(rating(3, 3, 3, 3, 3).band(), HeavinessBand::UltraHeavy); // 15
        assert_eq!(HeavinessBand::UltraHeavy.label(), "极重");
    }

    // ---------------- 阶梯校验 ----------------

    #[test]
    fn ladder_validate_accepts_wellformed_and_ranks_by_position() {
        let ladder = equipment_ladder();
        ladder.validate().expect("装备谱系应通过校验");
        assert_eq!(ladder.tier_rank("e0_stat_bonus"), Some(0));
        assert_eq!(ladder.tier_rank("e4_craft"), Some(4));
        assert_eq!(ladder.tier_rank("e9_missing"), None);
    }

    #[test]
    fn ladder_rejects_duplicate_tier_id() {
        let ladder = HeavinessLadder {
            tiers: vec![
                tier("light", rating(0, 0, 0, 0, 0), 0, 0, &[]),
                tier("light", rating(1, 1, 1, 1, 1), 0, 0, &[]),
            ],
        };
        let error = ladder.validate().expect_err("重复档 id 应被拒绝");
        assert!(
            error.message.contains("重复"),
            "实际消息：{}",
            error.message
        );
    }

    #[test]
    fn ladder_rejects_rating_dimension_over_three() {
        let ladder = HeavinessLadder {
            tiers: vec![tier("bad", rating(4, 0, 0, 0, 0), 0, 0, &[])],
        };
        let error = ladder.validate().expect_err("M=4 应被拒绝");
        assert!(error.message.contains("M=4"), "实际消息：{}", error.message);
    }

    #[test]
    fn ladder_rejects_p_floor_violations() {
        let over_domain = HeavinessLadder {
            tiers: vec![tier("bad", rating(0, 0, 0, 3, 0), 4, 0, &[])],
        };
        assert!(
            over_domain.validate().is_err(),
            "p_floor=4 超取值域应被拒绝"
        );

        let unfulfilled = HeavinessLadder {
            tiers: vec![tier("bad", rating(0, 0, 0, 1, 0), 2, 0, &[])],
        };
        let error = unfulfilled
            .validate()
            .expect_err("P 评分低于自身承诺下限应被拒绝");
        assert!(
            error.message.contains("p_floor"),
            "实际消息：{}",
            error.message
        );
    }

    #[test]
    fn ladder_rejects_interface_floor_contradicting_c_rating() {
        // 声明 C1（2-3 边）却承诺下限 9 条边：自相矛盾的标定。
        let ladder = HeavinessLadder {
            tiers: vec![tier("bad", rating(0, 0, 1, 0, 0), 0, 9, &[])],
        };
        let error = ladder.validate().expect_err("下限超 C 档上界应被拒绝");
        assert!(
            error.message.contains("interface_floor"),
            "实际消息：{}",
            error.message
        );
        // C3 档无上界：下限 9 合法。
        let unbounded = HeavinessLadder {
            tiers: vec![tier("ok", rating(0, 0, 3, 0, 0), 0, 9, &[])],
        };
        unbounded.validate().expect("C3 档下限 9 应合法");
    }

    #[test]
    fn ladder_rejects_non_monotonic_activates() {
        // 高档丢了低档的激活点（乱序/漏抄）：升档反而关闭已有设计问题，必须拒绝。
        let ladder = HeavinessLadder {
            tiers: vec![
                tier("low", rating(1, 0, 0, 1, 0), 0, 0, &["m.a", "m.b"]),
                tier("high", rating(2, 1, 1, 2, 1), 0, 0, &["m.b", "m.c"]),
            ],
        };
        let error = ladder.validate().expect_err("非递增包含应被拒绝");
        assert!(
            error.message.contains("m.a"),
            "应点名缺失的点，实际消息：{}",
            error.message
        );
        // 空档（未声明 activates）不参与包含链。
        let with_gap = HeavinessLadder {
            tiers: vec![
                tier("low", rating(1, 0, 0, 1, 0), 0, 0, &["m.a"]),
                tier("mid_undeclared", rating(1, 1, 0, 1, 0), 0, 0, &[]),
                tier("high", rating(2, 1, 1, 2, 1), 0, 0, &["m.a", "m.b"]),
            ],
        };
        with_gap.validate().expect("空档跳过包含检查应通过");
    }

    // ---------------- 模块校验 ----------------

    #[test]
    fn module_validate_accepts_equipment_fixture() {
        equipment_module()
            .validate()
            .expect("装备模块夹具应通过校验");
    }

    #[test]
    fn module_rejects_empty_module_id() {
        let mut module = equipment_module();
        module.module_id = "  ".into();
        assert!(module.validate().is_err(), "空 module_id 应被拒绝");
    }

    #[test]
    fn module_rejects_point_without_module_prefix() {
        let mut module = equipment_module();
        module.decision_points[0].id = "rogue.point".into();
        let error = module.validate().expect_err("前缀违规应被拒绝");
        assert!(
            error.message.contains("前缀"),
            "实际消息：{}",
            error.message
        );
    }

    #[test]
    fn module_rejects_dangling_local_noun() {
        let mut module = equipment_module();
        module.interface.modifies.push("undeclared_noun".into());
        let error = module.validate().expect_err("悬空本地名词应被拒绝");
        assert!(
            error.message.contains("undeclared_noun"),
            "实际消息：{}",
            error.message
        );
        // 带点号的外部名词不在本卡检查范围（绑定悬空由加载器 V6 判）。
        let mut external = equipment_module();
        external.interface.consumes.push("sys.shop.goods".into());
        external.validate().expect("外部命名空间名词应放行");
    }

    #[test]
    fn module_rejects_dangling_tier_activates() {
        let mut module = equipment_module();
        // 让每一档都追加同一个不存在的点，维持递增包含、只暴露悬空引用。
        for tier in &mut module.heaviness.tiers {
            tier.activates.push("sys.equipment.ghost_point".into());
        }
        let error = module.validate().expect_err("悬空 activates 应被拒绝");
        assert!(
            error.message.contains("ghost_point"),
            "实际消息：{}",
            error.message
        );
    }

    #[test]
    fn module_rejects_dangling_tier_gate() {
        let mut module = equipment_module();
        module.decision_points[0].tier_gate = Some("e99_missing".into());
        let error = module
            .validate()
            .expect_err("tier_gate 指向不存在档应被拒绝");
        assert!(
            error.message.contains("e99_missing"),
            "实际消息：{}",
            error.message
        );
    }

    #[test]
    fn mda_rejects_more_than_two_primary_aesthetics() {
        let mda = MdaMapping {
            mechanics_summary: String::new(),
            dynamics_notes: Vec::new(),
            aesthetics_primary: vec!["挑战".into(), "表达".into(), "幻想".into()],
        };
        assert!(mda.validate().is_err(), "A 层主项 3 个应被拒绝");
        let mut module = equipment_module();
        module.mda.aesthetics_primary.push("幻想".into());
        assert!(module.validate().is_err(), "模块级校验应连带拦截");
    }

    // ---------------- 装备谱系档带断言（定稿 §3.4 真实数据） ----------------

    #[test]
    fn equipment_spectrum_bands_match_calibration_doc() {
        let ladder = equipment_ladder();
        ladder.validate().expect("谱系应通过校验");
        let expectations = [
            ("e0_stat_bonus", 3, HeavinessBand::Light),
            ("e1_quality_affix", 7, HeavinessBand::Medium),
            ("e2_skill_build", 11, HeavinessBand::Heavy),
            ("e3_socket", 12, HeavinessBand::Heavy),
            ("e4_craft", 15, HeavinessBand::UltraHeavy),
            ("e5_enhance", 15, HeavinessBand::UltraHeavy),
            ("e6_set_transmog", 15, HeavinessBand::UltraHeavy),
        ];
        for (id, total, band) in expectations {
            let rank = ladder.tier_rank(id).expect("档位应存在");
            let tier = &ladder.tiers[rank];
            assert_eq!(tier.rating.total(), total, "档 {id} 总分不符");
            assert_eq!(tier.rating.band(), band, "档 {id} 档带不符");
        }
        // 谱系拐点 E1→E2：P 由 2 跳 3（构筑级承诺写死进档位定义）。
        let e1 = &ladder.tiers[1];
        let e2 = &ladder.tiers[2];
        assert_eq!((e1.rating.p, e1.p_floor), (2, 2));
        assert_eq!((e2.rating.p, e2.p_floor), (3, 3));
    }

    // ---------------- κ 四级 ----------------

    #[test]
    fn core_link_weights_and_default() {
        assert_eq!(CoreLink::Core.weight(), 1.0);
        assert_eq!(CoreLink::Strong.weight(), 0.75);
        assert_eq!(CoreLink::Weak.weight(), 0.5);
        assert_eq!(CoreLink::Meta.weight(), 0.25);
        assert_eq!(CoreLink::default(), CoreLink::Weak);
        assert_eq!(
            serde_json::to_value(CoreLink::Strong).expect("序列化应成功"),
            serde_json::json!("strong")
        );
        let back: CoreLink = serde_json::from_str("\"meta\"").expect("反序列化应成功");
        assert_eq!(back, CoreLink::Meta);
    }

    // ---------------- DecisionPoint.tier_gate 旧档兼容 ----------------

    #[test]
    fn decision_point_tier_gate_defaults_to_none_and_roundtrips() {
        // 旧清单（无 tier_gate 键）必须原样解析且不受档位门控。
        let legacy = r#"{
          "id": "u.platform", "domain": "profile", "level": "L0",
          "genre_scope": "universal", "question": "主平台是什么？",
          "options": [ { "id": "pc", "label": "PC" }, { "id": "mobile", "label": "移动端" } ]
        }"#;
        let legacy_point: DecisionPoint = serde_json::from_str(legacy).expect("旧决策点应可解析");
        assert_eq!(legacy_point.tier_gate, None);

        let gated = point("sys.equipment.skill_trigger", Some("e2_skill_build"));
        let json = serde_json::to_string(&gated).expect("序列化应成功");
        assert!(json.contains("tier_gate"));
        let back: DecisionPoint = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(back.tier_gate.as_deref(), Some("e2_skill_build"));
        assert_eq!(back, gated);
    }
}
