//! 组合校验器纯函数（W7 定稿 §4 全套 + 附录《极端组合形态测试》，T-W7-0c）。
//!
//! 为什么是纯函数：组合合法性要同时挂 gate2 冻结门与 authoring 期即时反馈
//! （定稿 §4.5 裁决 5），同一份判定逻辑两处调用——无 IO、无状态、输入决定输出，
//! 才能保证两处结论逐字节一致（I1 确定性守恒）。接线（gate2 合流、署名确认流、
//! R3 留痕）是波 3b 的事，本文件只产报告。
//!
//! block 与 advice 的分界（用户 2026-09-03 改制）：
//! - **block（不可豁免）**：R-C1′(a) 连通、(b) 强耦合——防"两个游戏钉在一起"的
//!   结构缺陷，与玩法大小无关；V1/V2 传导、V4 重而弱关联、V6 悬空消费同为结构缺陷。
//! - **advice（提示不拦）**：(c) |H| 参考线——超大玩法（EU4 型）是稀有但正当的设计，
//!   档位枚举会无穷倒退，故只产提示 + `form_confirmation_required` 标记（一次性署名
//!   形态确认的数据结构），确认流本身由波 3b 实现；R-C2 预算同理降为提示；
//!   双连通守卫（割点检出）为提示级——检出货币桥式钉接嫌疑时提醒，不拦。
//!
//! 确定性纪律：实例按 id 排序遍历、集合一律 BTreeMap/BTreeSet、预算表按贡献降序 +
//! id 字典序 tie-break——同一输入在任何平台产出完全相同的报告。

use crate::system_module::{CoreLink, FiveAxisRating, HeavinessBand, Induction, InductionTarget};
use crate::system_module::{NounId, TierId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// 三类端口（定稿 §2.2）：一条接口边 =（源实例, 端口, 名词, 目标实例）。
///
/// `#[default]` 取 Provides 仅为满足容器 `#[serde(default)]` 的旧档兼容——
/// provides 是三端口中唯一"只供给不索取"的方向，缺键时不会凭空制造 V6 悬空消费。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InterfacePort {
    #[default]
    Provides,
    Consumes,
    Modifies,
}

impl InterfacePort {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Provides => "供给",
            Self::Consumes => "消费",
            Self::Modifies => "修改",
        }
    }
}

/// 实例级接口边。系统实例图以它为边集，是 R-C1′/R-C3/V6 的唯一结构输入
/// （定稿 §2.2：模块只声明端口名词，成边与绑定由加载器完成后喂进本校验器）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InterfaceEdge {
    pub from_instance: String,
    pub port: InterfacePort,
    pub noun: NounId,
    pub to_instance: String,
}

/// 组合中的一个系统实例（加载器把 SystemModule × 声明档实例化后的产物）。
///
/// 波 3b 接线的映射关系（给加载器/gate2 的构造说明）：
/// - `module_id` ← `SystemModule.module_id`；`declared_tier` ← tier 合成点的选中档；
/// - `rating` ← 声明档 `HeavinessTier.rating`（定稿 §4.3：预算单轨制，一律取声明档
///   映射的五维分，测得分不进本校验器）；
/// - `core_link` ← pack `SystemRef` 的 κ 声明或 `derive_core_link` 推导结果；
/// - `interface_edges` ← 名词绑定表成边后归属本实例的边（from/to 可指向任意实例，
///   校验时取全组合边并集，归属仅为组织方便）；
/// - `inductions` ← 声明档及以下各档 `HeavinessTier.inductions` 的并集（定稿 §4.4
///   "本档（含以上）触发"语义：达到某档即背上该档与更低档的全部传导要求，
///   并集由调用方展开——本校验器不持有模块阶梯，保持输入自含）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SystemInstance {
    pub instance_id: String,
    pub module_id: String,
    pub declared_tier: TierId,
    /// 声明档映射的五维分（W = total()，档带 = band()）。
    pub rating: FiveAxisRating,
    /// κ 已由输入给定（pack 声明或推导回填）；本校验器信任该字段，
    /// `derive_core_link` 仅作调用方推导 κ 的辅助纯函数。
    pub core_link: CoreLink,
    /// 只在局外生效（derive_core_link 的 meta 分支判据——"局外性"是模块语义
    /// 声明不是图结构，机器无法从边推出，故由输入携带）。
    pub is_meta_only: bool,
    pub interface_edges: Vec<InterfaceEdge>,
    pub inductions: Vec<Induction>,
}

/// L0 产品档位（定稿 §4.2(c) 参考线表）。`#[default]` 取超休闲：参考线 0 最严，
/// 缺键的旧档宁可多出提示也不静默放行。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProductGrade {
    #[default]
    HyperCasual,
    Casual,
    MidCore,
    HardCore,
    Mmo,
}

impl ProductGrade {
    /// |H| 参考线（用户 2026-09-03 改制后为提示线不是硬上限）：
    /// 超休闲 0 / 休闲 1 / 中核 2 / 重核 3 / MMO 4。
    pub fn h_reference_line(&self) -> usize {
        match self {
            Self::HyperCasual => 0,
            Self::Casual => 1,
            Self::MidCore => 2,
            Self::HardCore => 3,
            Self::Mmo => 4,
        }
    }

    /// 预算表键（与 serde tag 同形，避免"枚举一套名、数据文件另一套名"两处真相）。
    pub fn key(&self) -> &'static str {
        match self {
            Self::HyperCasual => "hyper_casual",
            Self::Casual => "casual",
            Self::MidCore => "mid_core",
            Self::HardCore => "hard_core",
            Self::Mmo => "mmo",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::HyperCasual => "超休闲",
            Self::Casual => "休闲",
            Self::MidCore => "中核",
            Self::HardCore => "重核",
            Self::Mmo => "MMO",
        }
    }
}

/// R-C2 预算配置（定稿 §4.3：数值为占位，波 5 标定回归回填；键 = ProductGrade::key）。
/// 占位期允许为空——查无本档预算值时预算检查静默跳过（提示制，不阻塞）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompositionBudget {
    pub grade_budgets: BTreeMap<String, f64>,
}

/// 组合校验输入（纯数据，无模块库句柄——保持纯函数自含）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompositionInput {
    pub instances: Vec<SystemInstance>,
    /// core_loop 动词序列：（动词, 绑定实例 id）。本函数信任 `core_link` 字段不复算 κ，
    /// 该字段供 `derive_core_link` 与波 3b 接线（κ 推导回填）使用。
    pub core_loop_verbs: Vec<(String, String)>,
    pub product_grade: ProductGrade,
    pub budget: CompositionBudget,
    /// |H| 超参考线的一次性署名形态确认是否已存在（R3 留痕由波 3b 管理，
    /// 本校验器只消费其结果决定 `form_confirmation_required`）。
    pub form_confirmed: bool,
    /// pack 核心名词白名单：consumes 无人 provides 但在此清单内不算悬空（V6）。
    pub pack_core_nouns: Vec<NounId>,
    /// 模块 id → 档位 id 有序表（由轻到重，序号即 rank；加载器从
    /// `HeavinessLadder.tiers` 顺序导出）。V2 传导的档位比较需要 rank，
    /// 而 rank 是模块阶梯的局部序（0a 裁定不落字段），故随输入携带。
    pub module_tier_orders: BTreeMap<String, Vec<TierId>>,
}

/// 违例/提示代码。V1-V6 沿定稿 §4.5 违例清单；V3 按 R-C1′ 三查拆为 a/b/c；
/// BiconnectivityAdvice 为附录双连通守卫（用户改制后提示级）。
/// `#[default]` 仅为容器 `#[serde(default)]` 旧档兼容，无语义倾向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FindingCode {
    #[default]
    V1TransmissionGap,
    V2TransmissionUnmet,
    V3aDisconnected,
    V3bWeakCoupling,
    V3cCountAdvice,
    V4HeavyButLoose,
    V5BudgetAdvice,
    V6DanglingConsume,
    BiconnectivityAdvice,
}

/// 单条判定结果。detail 为中文完整叙述（含判据数字与修复指向），
/// related 为涉事实例/名词 id 列表——机器接线（authoring 高亮）用 related，人读 detail。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompositionFinding {
    pub code: FindingCode,
    pub subject: String,
    pub detail: String,
    pub related: Vec<String>,
}

/// 组合校验报告。blocks 非空 = 组合不可冻结（gate2 拦截）；advices 只提示。
/// h_set/h_connected/budget_total 是无论有无违例都产出的结构事实，
/// 供报告层（F 形态标签、逐重核访谈）复用，不必重跑图算法。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompositionReport {
    pub blocks: Vec<CompositionFinding>,
    pub advices: Vec<CompositionFinding>,
    /// H = {W≥9 且 κ∈{core,strong}}，按 id 字典序。
    pub h_set: Vec<String>,
    pub h_connected: bool,
    /// B(G) = Σ W(S) × weight(κ)，按 id 字典序求和（f64 加法顺序固定保确定性）。
    pub budget_total: f64,
    /// |H| 超参考线且尚无署名确认时为 true——波 3b 据此弹一次性形态确认。
    pub form_confirmation_required: bool,
}

/// 按定稿 §4.1 纯结构判据推导 κ（按序取首个命中）：
/// 1. **core**：实例被 core_loop 动词绑定；
/// 2. **strong**：provides 的名词被某 core 实例消费（存在 core 实例的 consumes 边
///    指向同名词），或 provides/modifies 边直接指向 core 实例（modifies core 属性）；
/// 3. **meta**：`is_meta_only`（局外性是模块语义声明，图结构推不出来，故看字段）；
/// 4. 否则 **weak**（保守计权：未命中判据的系统按"可绕开"处理，宁可低估贡献）。
///
/// `edges` 应传全组合边并集——strong 判定要看别的实例（core）消费了什么，
/// 单看本实例的边不够。
pub fn derive_core_link(
    instance: &SystemInstance,
    core_loop_verbs: &[(String, String)],
    edges: &[InterfaceEdge],
) -> CoreLink {
    let core_instances: BTreeSet<&str> = core_loop_verbs
        .iter()
        .map(|(_, instance_id)| instance_id.as_str())
        .collect();
    if core_instances.contains(instance.instance_id.as_str()) {
        return CoreLink::Core;
    }
    let core_consumed: BTreeSet<&str> = edges
        .iter()
        .filter(|edge| {
            edge.port == InterfacePort::Consumes
                && core_instances.contains(edge.from_instance.as_str())
        })
        .map(|edge| edge.noun.as_str())
        .collect();
    let strong = edges.iter().any(|edge| {
        if edge.from_instance != instance.instance_id {
            return false;
        }
        match edge.port {
            InterfacePort::Provides => {
                core_consumed.contains(edge.noun.as_str())
                    || core_instances.contains(edge.to_instance.as_str())
            }
            InterfacePort::Modifies => core_instances.contains(edge.to_instance.as_str()),
            InterfacePort::Consumes => false,
        }
    });
    if strong {
        return CoreLink::Strong;
    }
    if instance.is_meta_only {
        return CoreLink::Meta;
    }
    CoreLink::Weak
}

/// W≥9（全局档带 重/极重）即入重核判定口径（定稿裁决 1：废除 rank 门槛）。
fn is_heavy(rating: &FiveAxisRating) -> bool {
    matches!(
        rating.band(),
        HeavinessBand::Heavy | HeavinessBand::UltraHeavy
    )
}

/// 在给定节点子集内做 BFS 连通块划分（邻接表可含子集外的键/邻居，一律过滤）。
/// 返回的每个连通块内部按字典序、块间按首元素字典序——输出确定。
fn connected_components(
    nodes: &BTreeSet<&str>,
    adjacency: &BTreeMap<&str, BTreeMap<&str, usize>>,
) -> Vec<Vec<String>> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut components = Vec::new();
    for start in nodes {
        if seen.contains(start) {
            continue;
        }
        seen.insert(start);
        let mut queue: VecDeque<&str> = VecDeque::new();
        queue.push_back(start);
        let mut component = vec![(*start).to_string()];
        while let Some(current) = queue.pop_front() {
            if let Some(neighbors) = adjacency.get(current) {
                for neighbor in neighbors.keys() {
                    if nodes.contains(neighbor) && seen.insert(neighbor) {
                        component.push((*neighbor).to_string());
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components
}

fn format_components(components: &[Vec<String>]) -> String {
    components
        .iter()
        .map(|component| format!("[{}]", component.join("、")))
        .collect::<Vec<_>>()
        .join(" 与 ")
}

/// 组合校验主入口：输入组合快照，输出 block/advice 两类判定与结构事实。
///
/// 判定顺序固定（R-C1′(a)(b) → (c) → 双连通守卫 → V4 → R-C3(V1/V2) → V6 → R-C2），
/// 同类判定内按实例 id 字典序——报告可逐字节复演。
pub fn check_composition(input: &CompositionInput) -> CompositionReport {
    let mut blocks: Vec<CompositionFinding> = Vec::new();
    let mut advices: Vec<CompositionFinding> = Vec::new();

    let mut sorted_instances: Vec<&SystemInstance> = input.instances.iter().collect();
    sorted_instances.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));

    let all_edges: Vec<&InterfaceEdge> = sorted_instances
        .iter()
        .flat_map(|instance| instance.interface_edges.iter())
        .collect();

    // ---------- H 集与诱导子图 ----------
    let h_members: Vec<&SystemInstance> = sorted_instances
        .iter()
        .copied()
        .filter(|instance| {
            is_heavy(&instance.rating)
                && matches!(instance.core_link, CoreLink::Core | CoreLink::Strong)
        })
        .collect();
    let h_ids: BTreeSet<&str> = h_members
        .iter()
        .map(|instance| instance.instance_id.as_str())
        .collect();
    let h_set: Vec<String> = h_ids.iter().map(|id| (*id).to_string()).collect();

    // 无向邻接 + 边数计数：一条接口边记一条无向边（(b) 数的是"接口边合计"，
    // 平行边各自计数——两条不同名词的边就是两重耦合证据）。
    let mut h_adjacency: BTreeMap<&str, BTreeMap<&str, usize>> =
        h_ids.iter().map(|id| (*id, BTreeMap::new())).collect();
    for edge in &all_edges {
        let from = edge.from_instance.as_str();
        let to = edge.to_instance.as_str();
        if from == to || !h_ids.contains(from) || !h_ids.contains(to) {
            continue;
        }
        if let Some(neighbors) = h_adjacency.get_mut(from) {
            *neighbors.entry(to).or_insert(0) += 1;
        }
        if let Some(neighbors) = h_adjacency.get_mut(to) {
            *neighbors.entry(from).or_insert(0) += 1;
        }
    }

    // ---------- R-C1′(a) 连通（硬违例，不可豁免） ----------
    let components = connected_components(&h_ids, &h_adjacency);
    let h_connected = components.len() <= 1;
    if !h_connected {
        blocks.push(CompositionFinding {
            code: FindingCode::V3aDisconnected,
            subject: "重核集合H".to_string(),
            detail: format!(
                "R-C1′(a) 违例：重核（W≥9 且 κ∈{{core,strong}}）诱导子图不连通，分裂为 {} 个连通块：{}。这是\"两个游戏钉在一起\"的结构特征，与玩法大小无关，不可署名豁免；修复方向：为连通块之间补真实接口边（名词流），或将其中一侧降档退出重核。",
                components.len(),
                format_components(&components)
            ),
            related: h_set.clone(),
        });
    }

    // ---------- R-C1′(b) 强耦合（硬违例，不可豁免） ----------
    if h_ids.len() >= 2 {
        for member in &h_members {
            let id = member.instance_id.as_str();
            let (degree, neighbors): (usize, Vec<String>) = match h_adjacency.get(id) {
                Some(adjacent) => (
                    adjacent.values().sum(),
                    adjacent.keys().map(|key| (*key).to_string()).collect(),
                ),
                None => (0, Vec::new()),
            };
            if degree < 2 {
                blocks.push(CompositionFinding {
                    code: FindingCode::V3bWeakCoupling,
                    subject: id.to_string(),
                    detail: format!(
                        "R-C1′(b) 违例：重核 {id} 与 H 内其他重核的接口边合计 {degree} 条，低于强耦合下限 2。重系统只靠单边（或零边）挂在核心网上是钉接嫌疑；修复方向：补第二条真实耦合边，或将该系统降档退出重核。"
                    ),
                    related: neighbors,
                });
            }
        }
    }

    // ---------- R-C1′(c) 数量参考线（提示义务，不 block） ----------
    let reference_line = input.product_grade.h_reference_line();
    let over_line = h_ids.len() > reference_line;
    let form_confirmation_required = over_line && !input.form_confirmed;
    if over_line {
        advices.push(CompositionFinding {
            code: FindingCode::V3cCountAdvice,
            subject: input.product_grade.label().to_string(),
            detail: format!(
                "R-C1′(c) 提示：重核数量 |H|={} 超过 {} 档参考线 {}。超线不 block——超大玩法是稀有但正当的设计；需一次性署名形态确认（确认后 AI 转入逐重核系统的轻重需求访谈，不再劝减总体形态）。当前确认状态：{}。",
                h_ids.len(),
                input.product_grade.label(),
                reference_line,
                if input.form_confirmed {
                    "已署名确认"
                } else {
                    "待确认"
                }
            ),
            related: h_set.clone(),
        });
    }

    // ---------- 双连通守卫（提示级）：H 连通但存在割点 → 钉接嫌疑 ----------
    // 割点判法用"逐点摘除 + 连通复查"而不是 Tarjan：H 规模是个位数到十位数，
    // O(n·(n+m)) 足够，且摘除语义与判据原文（"删除任意单节点后其余仍连通"）逐字对应，
    // 正确性可目视核对。
    if h_connected && h_ids.len() >= 3 {
        for candidate in &h_ids {
            let remaining: BTreeSet<&str> =
                h_ids.iter().copied().filter(|id| id != candidate).collect();
            let sub_components = connected_components(&remaining, &h_adjacency);
            if sub_components.len() > 1 {
                blocks_or_advice_cut_vertex(&mut advices, candidate, &sub_components);
            }
        }
    }

    // ---------- V4 重而弱关联（block，定稿综合者注 ②） ----------
    for instance in &sorted_instances {
        if is_heavy(&instance.rating)
            && matches!(instance.core_link, CoreLink::Weak | CoreLink::Meta)
        {
            blocks.push(CompositionFinding {
                code: FindingCode::V4HeavyButLoose,
                subject: instance.instance_id.clone(),
                detail: format!(
                    "V4 违例：实例 {}（W={}，{}）达到重核档带却与核心循环弱关联（κ={}）。重系统游离在核心循环外是堆料特征，R-C1′ 的 H 集管不到它；修复方向：接入核心循环（补 provides/modifies 边升 κ），或降档到 W<9。",
                    instance.instance_id,
                    instance.rating.total(),
                    instance.rating.band().label(),
                    instance.core_link.label()
                ),
                related: vec![instance.instance_id.clone()],
            });
        }
    }

    // ---------- R-C3 传导 worklist 不动点（V1/V2 block） ----------
    // 声明档单轨制下全部实例的声明档自始生效，故初始 worklist 已含全部传导项，
    // 首轮即达不动点；仍用 worklist + visited 结构：一是循环传导链（A 要求 B、
    // B 要求 A）在 visited 护栏下必然终止，二是将来若扩展为需求传播
    // （被要求的更高档触发目标自己的更高档传导），结构不必推倒。
    let mut providers_by_noun: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in &all_edges {
        if edge.port == InterfacePort::Provides {
            providers_by_noun
                .entry(edge.noun.as_str())
                .or_default()
                .insert(edge.from_instance.as_str());
        }
    }
    let mut instances_by_module: BTreeMap<&str, Vec<&SystemInstance>> = BTreeMap::new();
    for instance in &sorted_instances {
        instances_by_module
            .entry(instance.module_id.as_str())
            .or_default()
            .push(instance);
    }

    let mut worklist: VecDeque<(usize, usize)> = VecDeque::new();
    for (instance_index, instance) in sorted_instances.iter().enumerate() {
        for induction_index in 0..instance.inductions.len() {
            worklist.push_back((instance_index, induction_index));
        }
    }
    let mut visited: BTreeSet<(usize, usize)> = BTreeSet::new();
    while let Some(item) = worklist.pop_front() {
        if !visited.insert(item) {
            continue;
        }
        let Some(source) = sorted_instances.get(item.0) else {
            continue;
        };
        let Some(induction) = source.inductions.get(item.1) else {
            continue;
        };
        check_induction(
            &mut blocks,
            source,
            induction,
            &instances_by_module,
            &providers_by_noun,
            &input.module_tier_orders,
        );
    }

    // ---------- V6 悬空消费（block） ----------
    let whitelist: BTreeSet<&str> = input
        .pack_core_nouns
        .iter()
        .map(|noun| noun.as_str())
        .collect();
    let mut consumers_by_noun: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in &all_edges {
        if edge.port == InterfacePort::Consumes {
            consumers_by_noun
                .entry(edge.noun.as_str())
                .or_default()
                .insert(edge.from_instance.as_str());
        }
    }
    for (noun, consumers) in &consumers_by_noun {
        if providers_by_noun.contains_key(noun) || whitelist.contains(noun) {
            continue;
        }
        let consumer_list: Vec<String> = consumers.iter().map(|id| (*id).to_string()).collect();
        blocks.push(CompositionFinding {
            code: FindingCode::V6DanglingConsume,
            subject: (*noun).to_string(),
            detail: format!(
                "V6 违例：名词 {noun} 被 {} 消费，但组合内无任何实例 provides 它，也不在 pack 核心名词白名单——悬空消费即探针列静默放行的缺陷源。修复方向：补一个 provides 该名词的系统实例，或将其登记进 pack_core_nouns。",
                consumer_list.join("、")
            ),
            related: consumer_list,
        });
    }

    // ---------- R-C2 预算（提示） ----------
    // 按 id 字典序求和固定 f64 加法顺序（浮点加法不满足结合律，乱序求和会破坏确定性）。
    let contributions: Vec<(f64, &SystemInstance)> = sorted_instances
        .iter()
        .map(|instance| {
            (
                f64::from(instance.rating.total()) * instance.core_link.weight(),
                *instance,
            )
        })
        .collect();
    let budget_total: f64 = contributions
        .iter()
        .map(|(contribution, _)| *contribution)
        .sum();
    if let Some(limit) = input.budget.grade_budgets.get(input.product_grade.key())
        && budget_total > *limit
    {
        let mut table = contributions.clone();
        table.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(a.1.instance_id.cmp(&b.1.instance_id))
        });
        let rows: Vec<String> = table
            .iter()
            .map(|(contribution, instance)| {
                format!(
                    "{}（κ={} W={} 贡献 {contribution:.2}）",
                    instance.instance_id,
                    instance.core_link.label(),
                    instance.rating.total()
                )
            })
            .collect();
        advices.push(CompositionFinding {
            code: FindingCode::V5BudgetAdvice,
            subject: input.product_grade.label().to_string(),
            detail: format!(
                "R-C2 提示：B(G)={budget_total:.2} 超过 {} 档预算 {limit:.2}（占位值，待波 5 标定）。逐系统分值表（按贡献降序，供自查最贵的系统）：{}。指向减重度档而非删系统。",
                input.product_grade.label(),
                rows.join("；")
            ),
            related: table
                .iter()
                .map(|(_, instance)| instance.instance_id.clone())
                .collect(),
        });
    }

    CompositionReport {
        blocks,
        advices,
        h_set,
        h_connected,
        budget_total,
        form_confirmation_required,
    }
}

/// 割点提示（守卫本体在 check_composition，此处只负责措辞与装配）。
fn blocks_or_advice_cut_vertex(
    advices: &mut Vec<CompositionFinding>,
    cut_vertex: &str,
    sub_components: &[Vec<String>],
) {
    advices.push(CompositionFinding {
        code: FindingCode::BiconnectivityAdvice,
        subject: cut_vertex.to_string(),
        detail: format!(
            "双连通守卫（提示级）：删除重核 {cut_vertex} 后，其余重核分裂为 {}——{cut_vertex} 是割点，呈\"单桥黏合多重核\"的钉接嫌疑拓扑。真互锁网无割点（EU4 型删任意单系统仍互通）；建议为两侧簇补直接耦合边，或确认此形态符合设计意图。",
            format_components(sub_components)
        ),
        related: sub_components.iter().flatten().cloned().collect(),
    });
}

/// 单条传导的判定（R-C3）：目标不存在 → V1；存在但声明档 rank 低于要求 → V2。
/// 反向超配（目标档高于要求）不报——背包比装备需要的重是浪费不是错误（定稿 §4.4）。
fn check_induction(
    blocks: &mut Vec<CompositionFinding>,
    source: &SystemInstance,
    induction: &Induction,
    instances_by_module: &BTreeMap<&str, Vec<&SystemInstance>>,
    providers_by_noun: &BTreeMap<&str, BTreeSet<&str>>,
    module_tier_orders: &BTreeMap<String, Vec<TierId>>,
) {
    match &induction.target {
        InductionTarget::NounProvided(noun) => {
            // 名词析取语义（裁决 4）：任一实例 provides 即满足，不点名模块。
            if !providers_by_noun.contains_key(noun.as_str()) {
                blocks.push(CompositionFinding {
                    code: FindingCode::V1TransmissionGap,
                    subject: noun.clone(),
                    detail: format!(
                        "V1 违例（R-C3 传导缺口）：{}（声明档 {}）要求组合内任一实例 provides 名词 {noun}，当前无人提供。理由：{}。修复方向：为任一系统补 provides 该名词的接口边（析取语义——掉落或商店任一来源即满足）。",
                        source.instance_id, source.declared_tier, induction.reason
                    ),
                    related: vec![source.instance_id.clone()],
                });
            }
        }
        InductionTarget::Module(module_id) => {
            let Some(candidates) = instances_by_module.get(module_id.as_str()) else {
                blocks.push(CompositionFinding {
                    code: FindingCode::V1TransmissionGap,
                    subject: module_id.clone(),
                    detail: format!(
                        "V1 违例（R-C3 传导缺口）：{}（声明档 {}）要求组合内存在模块 {module_id} 的实例（最低档 {}），当前组合没有。理由：{}。",
                        source.instance_id,
                        source.declared_tier,
                        induction.min_tier,
                        induction.reason
                    ),
                    related: vec![source.instance_id.clone()],
                });
                return;
            };
            if induction.min_tier.is_empty() {
                // 无档位要求 = 存在性传导，实例已找到即满足。
                return;
            }
            let order = module_tier_orders.get(module_id.as_str());
            let required_rank =
                order.and_then(|tiers| tiers.iter().position(|tier| tier == &induction.min_tier));
            let Some(required_rank) = required_rank else {
                // 定位不了要求档的 rank（档位序缺失或档 id 未登记）：R2 精神下
                // 不得默认放行——按 V2 拦下并在 detail 说明是数据缺口不是档位不足。
                blocks.push(CompositionFinding {
                    code: FindingCode::V2TransmissionUnmet,
                    subject: module_id.clone(),
                    detail: format!(
                        "V2 违例（R-C3 无法证明满足）：{} 要求模块 {module_id} ≥ 档 {}，但该档在 module_tier_orders 的档位序中无法定位（档位序缺失或档 id 未登记），无法比较 rank——校验器不做默认放行。理由：{}。",
                        source.instance_id, induction.min_tier, induction.reason
                    ),
                    related: vec![source.instance_id.clone()],
                });
                return;
            };
            // 多实例语义：同模块任一实例达标即满足（与名词析取同一精神——
            // 传导要的是"组合里有人接得住"，不指定谁接）。
            let mut best: Option<(&SystemInstance, usize)> = None;
            for candidate in candidates {
                let candidate_rank = order.and_then(|tiers| {
                    tiers
                        .iter()
                        .position(|tier| tier == &candidate.declared_tier)
                });
                if let Some(rank) = candidate_rank {
                    if rank >= required_rank {
                        return;
                    }
                    let better = match best {
                        Some((_, best_rank)) => rank > best_rank,
                        None => true,
                    };
                    if better {
                        best = Some((candidate, rank));
                    }
                }
            }
            let (chain_tail, mut related) = match best {
                Some((candidate, rank)) => (
                    format!(
                        "组合内最高实例 {}（声明档 {}，rank {rank}）未达标",
                        candidate.instance_id, candidate.declared_tier
                    ),
                    vec![source.instance_id.clone(), candidate.instance_id.clone()],
                ),
                None => (
                    "该模块实例的声明档均无法在档位序中定位".to_string(),
                    vec![source.instance_id.clone()],
                ),
            };
            related.sort();
            blocks.push(CompositionFinding {
                code: FindingCode::V2TransmissionUnmet,
                subject: module_id.clone(),
                detail: format!(
                    "V2 违例（R-C3 传导不满足）：传导链 {}（声明档 {}）→ 模块 {module_id} 要求 ≥ 档 {}（rank {required_rank}），{chain_tail}。理由：{}。修复方向：升目标档，或降源系统档解除传导。",
                    source.instance_id, source.declared_tier, induction.min_tier, induction.reason
                ),
                related,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rating(m: u8, d: u8, c: u8, p: u8, o: u8) -> FiveAxisRating {
        FiveAxisRating { m, d, c, p, o }
    }

    fn instance(id: &str, module: &str, rating: FiveAxisRating, link: CoreLink) -> SystemInstance {
        SystemInstance {
            instance_id: id.to_string(),
            module_id: module.to_string(),
            declared_tier: "heavy".to_string(),
            rating,
            core_link: link,
            is_meta_only: false,
            interface_edges: Vec::new(),
            inductions: Vec::new(),
        }
    }

    fn edge(from: &str, port: InterfacePort, noun: &str, to: &str) -> InterfaceEdge {
        InterfaceEdge {
            from_instance: from.to_string(),
            port,
            noun: noun.to_string(),
            to_instance: to.to_string(),
        }
    }

    fn input_of(instances: Vec<SystemInstance>, grade: ProductGrade) -> CompositionInput {
        CompositionInput {
            instances,
            core_loop_verbs: Vec::new(),
            product_grade: grade,
            budget: CompositionBudget::default(),
            form_confirmed: false,
            pack_core_nouns: Vec::new(),
            module_tier_orders: BTreeMap::new(),
        }
    }

    fn codes(findings: &[CompositionFinding]) -> Vec<FindingCode> {
        findings.iter().map(|finding| finding.code).collect()
    }

    // ---------------- 撞墙案例一：杀戮尖塔（定稿 §6.2） ----------------

    /// H={构筑, 战斗, 遗物}：遗物 modifies 战斗+构筑、构筑↔战斗双边。
    fn spire_instances() -> Vec<SystemInstance> {
        let mut deck = instance(
            "deck_building",
            "sys.in_run_build",
            rating(3, 3, 2, 3, 3),
            CoreLink::Core,
        ); // W14
        deck.interface_edges = vec![edge(
            "deck_building",
            InterfacePort::Provides,
            "card_modifier",
            "turn_combat",
        )];
        let mut combat = instance(
            "turn_combat",
            "sys.turn_combat",
            rating(2, 2, 2, 2, 2),
            CoreLink::Core,
        ); // W10
        combat.interface_edges = vec![edge(
            "turn_combat",
            InterfacePort::Provides,
            "energy",
            "deck_building",
        )];
        let mut relics = instance(
            "relics",
            "sys.rule_modifier",
            rating(2, 2, 2, 2, 2),
            CoreLink::Strong,
        ); // W10
        relics.interface_edges = vec![
            edge(
                "relics",
                InterfacePort::Modifies,
                "draw_rule",
                "deck_building",
            ),
            edge(
                "relics",
                InterfacePort::Modifies,
                "combat_attribute",
                "turn_combat",
            ),
        ];
        let map = instance(
            "map_route",
            "sys.map_route",
            rating(2, 2, 1, 1, 1),
            CoreLink::Strong,
        ); // W7 中，不入 H
        let meta = instance(
            "meta_unlock",
            "sys.meta_unlock",
            rating(1, 1, 1, 1, 0),
            CoreLink::Meta,
        ); // W4 轻
        vec![deck, combat, relics, map, meta]
    }

    #[test]
    fn spire_midcore_over_line_is_advice_only_and_requires_confirmation() {
        let report = check_composition(&input_of(spire_instances(), ProductGrade::MidCore));
        assert_eq!(
            report.h_set,
            vec!["deck_building", "relics", "turn_combat"],
            "H 集应为三重核且按 id 排序"
        );
        assert!(report.h_connected, "遗物双 modifies + 构筑↔战斗双边应连通");
        assert!(
            report.blocks.is_empty(),
            "定稿 §6.2：尖塔无硬违例，实际：{:?}",
            report.blocks
        );
        assert_eq!(
            codes(&report.advices),
            vec![FindingCode::V3cCountAdvice],
            "|H|=3 > 中核参考线 2 应只产一条数量提示（三角形无割点，无双连通提示）"
        );
        assert!(
            report.form_confirmation_required,
            "超线且未署名确认 → 需要一次性形态确认"
        );
    }

    #[test]
    fn spire_hardcore_grade_clears_advice_and_confirmation() {
        let report = check_composition(&input_of(spire_instances(), ProductGrade::HardCore));
        assert!(report.blocks.is_empty());
        assert!(
            report.advices.is_empty(),
            "|H|=3 ≤ 重核参考线 3 应无提示，实际：{:?}",
            report.advices
        );
        assert!(!report.form_confirmation_required);
    }

    // ---------------- 撞墙案例二：MOBA（定稿 §6.3） ----------------

    #[test]
    fn moba_four_heavy_cores_pass_under_mmo_grade() {
        // 环形四耦合：技能-战斗-目标-装备-技能，每节点 H 内边 2。
        let mut skill = instance("skill", "sys.skill", rating(3, 3, 3, 3, 3), CoreLink::Core); // W15
        skill.interface_edges = vec![edge(
            "skill",
            InterfacePort::Modifies,
            "combat_attribute",
            "combat",
        )];
        let mut combat = instance(
            "combat",
            "sys.realtime_combat",
            rating(2, 2, 2, 2, 2),
            CoreLink::Core,
        ); // W10
        combat.interface_edges = vec![edge(
            "combat",
            InterfacePort::Provides,
            "kill_event",
            "objective",
        )];
        let mut objective = instance(
            "objective",
            "sys.objective",
            rating(2, 2, 2, 2, 1),
            CoreLink::Strong,
        ); // W9
        objective.interface_edges = vec![edge(
            "objective",
            InterfacePort::Provides,
            "tower_gold",
            "gear",
        )];
        let mut gear = instance(
            "gear",
            "sys.equipment",
            rating(2, 2, 2, 2, 1),
            CoreLink::Strong,
        ); // W9
        gear.interface_edges = vec![edge(
            "gear",
            InterfacePort::Provides,
            "stat_modifier",
            "skill",
        )];
        let report = check_composition(&input_of(
            vec![skill, combat, objective, gear],
            ProductGrade::Mmo,
        ));
        assert_eq!(report.h_set.len(), 4);
        assert!(report.h_connected);
        assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
        assert!(
            report.advices.is_empty(),
            "|H|=4 ≤ MMO 参考线 4 且环形无割点，应零提示，实际：{:?}",
            report.advices
        );
        assert!(!report.form_confirmation_required);
    }

    // ---------------- 撞墙案例三：EU4 型（附录 §1，|H|=8） ----------------

    /// 附录 §1.2 的边集：时钟 tick 供给全体，其余按外交↔战斗↔领土↔贸易↔货币↔科技↔宗教互锁。
    fn eu4_instances() -> Vec<SystemInstance> {
        let mut clock = instance(
            "clock",
            "sys.world_clock",
            rating(2, 2, 3, 0, 2),
            CoreLink::Core,
        ); // W9
        clock.interface_edges = [
            "diplomacy",
            "warfare",
            "territory",
            "trade",
            "currency",
            "tech",
            "religion",
        ]
        .iter()
        .map(|target| edge("clock", InterfacePort::Provides, "tick", target))
        .collect();
        let mut diplomacy = instance(
            "diplomacy",
            "sys.diplomacy",
            rating(3, 3, 3, 3, 2),
            CoreLink::Core,
        ); // W14
        diplomacy.interface_edges = vec![edge(
            "diplomacy",
            InterfacePort::Provides,
            "war_declaration",
            "warfare",
        )];
        let mut warfare = instance(
            "warfare",
            "sys.warfare",
            rating(3, 2, 3, 2, 1),
            CoreLink::Core,
        ); // W11
        warfare.interface_edges = vec![edge(
            "warfare",
            InterfacePort::Modifies,
            "province_control",
            "territory",
        )];
        let mut territory = instance(
            "territory",
            "sys.territory",
            rating(3, 3, 3, 2, 1),
            CoreLink::Core,
        ); // W12
        territory.interface_edges =
            vec![edge("territory", InterfacePort::Provides, "goods", "trade")];
        let mut trade = instance(
            "trade",
            "sys.trade_network",
            rating(2, 3, 2, 3, 1),
            CoreLink::Strong,
        ); // W11
        trade.interface_edges = vec![edge(
            "trade",
            InterfacePort::Provides,
            "trade_income",
            "currency",
        )];
        let mut currency = instance(
            "currency",
            "sys.currency",
            rating(2, 2, 3, 2, 1),
            CoreLink::Strong,
        ); // W10
        currency.interface_edges = vec![
            edge("currency", InterfacePort::Provides, "war_funds", "warfare"),
            edge(
                "currency",
                InterfacePort::Provides,
                "monarch_points",
                "tech",
            ),
        ];
        let mut tech = instance(
            "tech",
            "sys.tech_ideas",
            rating(2, 2, 2, 2, 1),
            CoreLink::Strong,
        ); // W9
        tech.interface_edges = vec![
            edge("tech", InterfacePort::Modifies, "combat_rule", "warfare"),
            edge(
                "tech",
                InterfacePort::Modifies,
                "development_rule",
                "territory",
            ),
            edge("tech", InterfacePort::Modifies, "trade_rule", "trade"),
        ];
        let mut religion = instance(
            "religion",
            "sys.religion",
            rating(2, 2, 2, 2, 1),
            CoreLink::Strong,
        ); // W9
        religion.interface_edges = vec![
            edge("religion", InterfacePort::Modifies, "unrest", "territory"),
            edge(
                "religion",
                InterfacePort::Modifies,
                "relations",
                "diplomacy",
            ),
        ];
        vec![
            clock, diplomacy, warfare, territory, trade, currency, tech, religion,
        ]
    }

    #[test]
    fn eu4_eight_heavy_cores_advice_not_block() {
        let mut input = input_of(eu4_instances(), ProductGrade::Mmo);
        input.form_confirmed = true;
        let report = check_composition(&input);
        assert_eq!(report.h_set.len(), 8, "附录 §1.2：|H|=8");
        assert!(report.h_connected, "(a) 连通应通过");
        assert!(
            report.blocks.is_empty(),
            "附录判定：EU4 (a)(b) 全过、不 block，实际：{:?}",
            report.blocks
        );
        assert_eq!(
            codes(&report.advices),
            vec![FindingCode::V3cCountAdvice],
            "只应有 |H| 超线提示（真互锁网无割点，无双连通提示），实际：{:?}",
            report.advices
        );
        assert!(
            !report.form_confirmation_required,
            "已署名确认（form_confirmed=true）后不再要求确认"
        );
    }

    // ---------------- 撞墙案例四：卡牌+农场钉接反例（附录 §1.4） ----------------

    /// 卡牌簇（构筑/战斗/遗物三角）+ 农场簇（时钟↔栽培双边），可选货币桥。
    fn stapled_instances(bridge_edges: Vec<InterfaceEdge>) -> Vec<SystemInstance> {
        let mut spire = spire_instances();
        spire.truncate(3); // 只留三重核三角
        let mut clock = instance(
            "farm_clock",
            "sys.world_clock",
            rating(2, 2, 2, 2, 1),
            CoreLink::Core,
        ); // W9
        clock.interface_edges = vec![edge(
            "farm_clock",
            InterfacePort::Provides,
            "tick",
            "farming",
        )];
        let mut farming = instance(
            "farming",
            "sys.farming",
            rating(2, 2, 2, 2, 2),
            CoreLink::Core,
        ); // W10
        farming.interface_edges = vec![edge(
            "farming",
            InterfacePort::Provides,
            "harvest_event",
            "farm_clock",
        )];
        let mut currency = instance(
            "bridge_currency",
            "sys.currency",
            rating(2, 2, 2, 2, 2),
            CoreLink::Strong,
        ); // W10
        currency.interface_edges = bridge_edges;
        spire.extend([clock, farming, currency]);
        spire
    }

    #[test]
    fn stapled_single_edge_bridge_blocks_on_v3a_and_v3b() {
        // 货币只有 1 条边挂在卡牌侧：H 不连通（农场簇孤立）且桥节点边数 1 < 2。
        let instances = stapled_instances(vec![edge(
            "bridge_currency",
            InterfacePort::Provides,
            "gold",
            "turn_combat",
        )]);
        let report = check_composition(&input_of(instances, ProductGrade::Mmo));
        assert!(!report.h_connected);
        let block_codes = codes(&report.blocks);
        assert!(
            block_codes.contains(&FindingCode::V3aDisconnected),
            "农场簇与卡牌簇分家 → V3a 硬违例，实际：{block_codes:?}"
        );
        assert!(
            block_codes.contains(&FindingCode::V3bWeakCoupling),
            "货币 H 内边数 1 < 2 → V3b 硬违例，实际：{block_codes:?}"
        );
        let weak = report
            .blocks
            .iter()
            .find(|finding| finding.code == FindingCode::V3bWeakCoupling)
            .map(|finding| finding.subject.clone());
        assert_eq!(weak.as_deref(), Some("bridge_currency"), "V3b 应点名桥节点");
    }

    #[test]
    fn stapled_full_bridge_passes_hard_checks_but_flags_cut_vertex() {
        // 附录 §1.4 最强钉接：货币与两簇各 2 条边——旧三查全过，双连通守卫点名割点。
        let instances = stapled_instances(vec![
            edge(
                "bridge_currency",
                InterfacePort::Provides,
                "gold",
                "turn_combat",
            ),
            edge(
                "turn_combat",
                InterfacePort::Provides,
                "loot_gold",
                "bridge_currency",
            ),
            edge(
                "bridge_currency",
                InterfacePort::Provides,
                "seed_budget",
                "farming",
            ),
            edge(
                "farming",
                InterfacePort::Provides,
                "crop_income",
                "bridge_currency",
            ),
        ]);
        let report = check_composition(&input_of(instances, ProductGrade::Mmo));
        assert!(report.h_connected);
        assert!(
            report.blocks.is_empty(),
            "全桥变体 (a)(b) 均过——这正是双连通守卫存在的理由，实际：{:?}",
            report.blocks
        );
        let cut = report
            .advices
            .iter()
            .find(|finding| finding.code == FindingCode::BiconnectivityAdvice);
        assert_eq!(
            cut.map(|finding| finding.subject.as_str()),
            Some("bridge_currency"),
            "守卫应点名货币为割点（提示级，不 block），实际提示：{:?}",
            report.advices
        );
        assert!(
            codes(&report.advices).contains(&FindingCode::V3cCountAdvice),
            "|H|=6 > MMO 参考线 4 的数量提示应同时在场"
        );
    }

    // ---------------- R-C3 传导：V1/V2 正反例与析取 ----------------

    fn induction_module(min_tier: &str, module: &str, reason: &str) -> Induction {
        Induction {
            when_tier: "heavy".to_string(),
            target: InductionTarget::Module(module.to_string()),
            min_tier: min_tier.to_string(),
            reason: reason.to_string(),
        }
    }

    #[test]
    fn v1_module_target_missing_blocks() {
        let mut equipment = instance(
            "equipment",
            "sys.equipment",
            rating(2, 2, 2, 2, 1),
            CoreLink::Strong,
        );
        equipment.inductions = vec![induction_module(
            "recycle_loop",
            "sys.economy",
            "材料经济成环",
        )];
        // 补一条自反边避免 V3b 干扰？单实例 H={equipment}，|H|=1 无 (b) 要求。
        let report = check_composition(&input_of(vec![equipment], ProductGrade::HardCore));
        let v1: Vec<&CompositionFinding> = report
            .blocks
            .iter()
            .filter(|finding| finding.code == FindingCode::V1TransmissionGap)
            .collect();
        assert_eq!(
            v1.len(),
            1,
            "缺经济模块应产一条 V1，实际 blocks：{:?}",
            report.blocks
        );
        assert_eq!(v1[0].subject, "sys.economy");
        assert!(v1[0].detail.contains("材料经济成环"), "reason 应进文案");
    }

    #[test]
    fn v1_noun_disjunction_either_provider_satisfies() {
        let make = |provider: Option<&str>| -> CompositionInput {
            let mut equipment = instance(
                "equipment",
                "sys.equipment",
                rating(2, 2, 2, 2, 1),
                CoreLink::Strong,
            );
            equipment.inductions = vec![Induction {
                when_tier: "e3_socket".to_string(),
                target: InductionTarget::NounProvided("gem_entity".to_string()),
                min_tier: String::new(),
                reason: "宝石必须有源".to_string(),
            }];
            let mut instances = vec![equipment];
            if let Some(provider_id) = provider {
                let mut source = instance(
                    provider_id,
                    "sys.loot",
                    rating(1, 1, 1, 1, 1),
                    CoreLink::Weak,
                );
                source.interface_edges = vec![edge(
                    provider_id,
                    InterfacePort::Provides,
                    "gem_entity",
                    "equipment",
                )];
                instances.push(source);
            }
            input_of(instances, ProductGrade::HardCore)
        };
        // 无人提供 → V1。
        let missing = check_composition(&make(None));
        assert!(codes(&missing.blocks).contains(&FindingCode::V1TransmissionGap));
        // 掉落或商店任一提供即满足（析取语义，裁决 4）。
        for provider in ["loot_drop", "shop"] {
            let satisfied = check_composition(&make(Some(provider)));
            assert!(
                satisfied.blocks.is_empty(),
                "{provider} 提供 gem_entity 后不应再有 V1，实际：{:?}",
                satisfied.blocks
            );
        }
    }

    #[test]
    fn v2_under_tier_blocks_with_chain_and_overprovision_passes() {
        let make = |inventory_tier: &str| -> CompositionInput {
            let mut equipment = instance(
                "equipment",
                "sys.equipment",
                rating(2, 2, 2, 2, 1),
                CoreLink::Strong,
            );
            equipment.declared_tier = "e3_socket".to_string();
            equipment.inductions = vec![induction_module(
                "classify",
                "sys.inventory",
                "新名词必须有存放",
            )];
            let mut inventory = instance(
                "inventory",
                "sys.inventory",
                rating(1, 1, 1, 1, 1),
                CoreLink::Weak,
            );
            inventory.declared_tier = inventory_tier.to_string();
            let mut input = input_of(vec![equipment, inventory], ProductGrade::HardCore);
            input.module_tier_orders.insert(
                "sys.inventory".to_string(),
                vec![
                    "basic".to_string(),
                    "classify".to_string(),
                    "batch".to_string(),
                ],
            );
            input
        };
        // 声明档 basic(rank 0) < 要求 classify(rank 1) → V2 附传导链。
        let under = check_composition(&make("basic"));
        let v2: Vec<&CompositionFinding> = under
            .blocks
            .iter()
            .filter(|finding| finding.code == FindingCode::V2TransmissionUnmet)
            .collect();
        assert_eq!(v2.len(), 1, "实际 blocks：{:?}", under.blocks);
        assert!(
            v2[0].detail.contains("传导链"),
            "V2 应附传导链：{}",
            v2[0].detail
        );
        assert!(v2[0].detail.contains("equipment") && v2[0].detail.contains("inventory"));
        assert_eq!(v2[0].related, vec!["equipment", "inventory"]);
        // 反向超配（batch rank 2 > 要求 rank 1）不报（定稿 §4.4：浪费不是错误）。
        let over = check_composition(&make("batch"));
        assert!(
            over.blocks.is_empty(),
            "超配不应报违例，实际：{:?}",
            over.blocks
        );
        // 恰好达标同样通过。
        assert!(check_composition(&make("classify")).blocks.is_empty());
    }

    #[test]
    fn v2_unresolvable_tier_rank_blocks_instead_of_silently_passing() {
        // 档位序缺失时不得默认放行（R2 精神）：按 V2 拦下并说明是数据缺口。
        let mut equipment = instance(
            "equipment",
            "sys.equipment",
            rating(2, 2, 2, 2, 1),
            CoreLink::Strong,
        );
        equipment.inductions = vec![induction_module(
            "classify",
            "sys.inventory",
            "新名词必须有存放",
        )];
        let inventory = instance(
            "inventory",
            "sys.inventory",
            rating(1, 1, 1, 1, 1),
            CoreLink::Weak,
        );
        let report = check_composition(&input_of(
            vec![equipment, inventory],
            ProductGrade::HardCore,
        ));
        let v2: Vec<&CompositionFinding> = report
            .blocks
            .iter()
            .filter(|finding| finding.code == FindingCode::V2TransmissionUnmet)
            .collect();
        assert_eq!(v2.len(), 1);
        assert!(
            v2[0].detail.contains("无法定位"),
            "应说明档位序缺口：{}",
            v2[0].detail
        );
    }

    #[test]
    fn induction_cycle_terminates_without_false_findings() {
        // 循环传导链 A↔B（各自要求对方 high 档，双方都已达标）：worklist 必须终止且零违例。
        let orders: Vec<TierId> = vec!["low".to_string(), "high".to_string()];
        let mut alpha = instance("alpha", "sys.alpha", rating(2, 2, 2, 2, 1), CoreLink::Core);
        alpha.declared_tier = "high".to_string();
        alpha.inductions = vec![induction_module("high", "sys.beta", "互为约束")];
        let mut beta = instance("beta", "sys.beta", rating(2, 2, 2, 2, 1), CoreLink::Strong);
        beta.declared_tier = "high".to_string();
        beta.inductions = vec![induction_module("high", "sys.alpha", "互为约束")];
        // 两重核补双边满足 (a)(b)。
        alpha.interface_edges = vec![
            edge("alpha", InterfacePort::Provides, "pulse", "beta"),
            edge("alpha", InterfacePort::Modifies, "beta_rule", "beta"),
        ];
        let mut input = input_of(vec![alpha, beta], ProductGrade::MidCore);
        input
            .module_tier_orders
            .insert("sys.alpha".to_string(), orders.clone());
        input
            .module_tier_orders
            .insert("sys.beta".to_string(), orders);
        let report = check_composition(&input);
        assert!(
            report.blocks.is_empty(),
            "循环链双方达标应零违例：{:?}",
            report.blocks
        );
    }

    // ---------------- V4 / V6 / V5 ----------------

    #[test]
    fn v4_heavy_but_loose_blocks_weak_and_meta() {
        let heavy_weak = instance(
            "idle_farm",
            "sys.farming",
            rating(2, 2, 2, 2, 2),
            CoreLink::Weak,
        ); // W10
        let heavy_meta = instance(
            "season_pass",
            "sys.season_pass",
            rating(2, 2, 2, 2, 1),
            CoreLink::Meta,
        ); // W9
        let medium_weak = instance(
            "gallery",
            "sys.gallery",
            rating(2, 2, 2, 1, 1),
            CoreLink::Weak,
        ); // W8 不违例
        let report = check_composition(&input_of(
            vec![heavy_weak, heavy_meta, medium_weak],
            ProductGrade::HardCore,
        ));
        let v4_subjects: Vec<&str> = report
            .blocks
            .iter()
            .filter(|finding| finding.code == FindingCode::V4HeavyButLoose)
            .map(|finding| finding.subject.as_str())
            .collect();
        assert_eq!(
            v4_subjects,
            vec!["idle_farm", "season_pass"],
            "W≥9 且 κ∈{{weak,meta}} 各一条、按 id 排序；W8 不报。实际 blocks：{:?}",
            report.blocks
        );
    }

    #[test]
    fn v6_dangling_consume_blocks_unless_whitelisted() {
        let mut shop = instance("shop", "sys.shop", rating(1, 1, 1, 1, 1), CoreLink::Weak);
        shop.interface_edges = vec![edge("shop", InterfacePort::Consumes, "mana_crystal", "")];
        let mut input = input_of(vec![shop.clone()], ProductGrade::Casual);
        let report = check_composition(&input);
        let v6: Vec<&CompositionFinding> = report
            .blocks
            .iter()
            .filter(|finding| finding.code == FindingCode::V6DanglingConsume)
            .collect();
        assert_eq!(v6.len(), 1, "实际 blocks：{:?}", report.blocks);
        assert_eq!(v6[0].subject, "mana_crystal");
        assert_eq!(v6[0].related, vec!["shop"]);
        // 白名单放行（pack 核心名词由 pack 自身供给，组合内无边是正常的）。
        input.pack_core_nouns = vec!["mana_crystal".to_string()];
        assert!(check_composition(&input).blocks.is_empty());
        // 有人 provides 也放行。
        let mut mine = instance("mine", "sys.mine", rating(1, 1, 1, 1, 1), CoreLink::Weak);
        mine.interface_edges = vec![edge(
            "mine",
            InterfacePort::Provides,
            "mana_crystal",
            "shop",
        )];
        let provided = input_of(vec![shop, mine], ProductGrade::Casual);
        assert!(check_composition(&provided).blocks.is_empty());
    }

    #[test]
    fn v5_budget_advice_lists_systems_in_descending_contribution() {
        let core = instance(
            "combat",
            "sys.combat",
            rating(2, 2, 2, 2, 2),
            CoreLink::Core,
        ); // 10.0
        let strong = instance(
            "gear",
            "sys.equipment",
            rating(2, 2, 2, 2, 2),
            CoreLink::Strong,
        ); // 7.5
        let weak = instance(
            "gallery",
            "sys.gallery",
            rating(2, 2, 2, 1, 1),
            CoreLink::Weak,
        ); // 4.0
        let mut input = input_of(vec![weak, core, strong], ProductGrade::MidCore);
        input
            .budget
            .grade_budgets
            .insert("mid_core".to_string(), 20.0);
        let report = check_composition(&input);
        assert!(
            (report.budget_total - 21.5).abs() < 1e-9,
            "10.0+7.5+4.0=21.5"
        );
        let v5 = report
            .advices
            .iter()
            .find(|finding| finding.code == FindingCode::V5BudgetAdvice)
            .map(|finding| finding.related.clone());
        assert_eq!(
            v5,
            Some(vec![
                "combat".to_string(),
                "gear".to_string(),
                "gallery".to_string()
            ]),
            "分值表应按 weight×W 降序。实际 advices：{:?}",
            report.advices
        );
        // 预算未配置（占位期）不产提示：把上限抬高即无 V5。
        input
            .budget
            .grade_budgets
            .insert("mid_core".to_string(), 50.0);
        let relaxed = check_composition(&input);
        assert!(
            !codes(&relaxed.advices).contains(&FindingCode::V5BudgetAdvice),
            "未超限不应有预算提示"
        );
    }

    // ---------------- derive_core_link 四分支 ----------------

    #[test]
    fn derive_core_link_covers_all_four_branches() {
        let verbs = vec![("作战".to_string(), "combat".to_string())];
        let combat = instance(
            "combat",
            "sys.combat",
            rating(2, 2, 2, 2, 2),
            CoreLink::Weak,
        );
        let loot = instance("loot", "sys.loot", rating(1, 1, 1, 1, 1), CoreLink::Weak);
        let relics = instance(
            "relics",
            "sys.rule_modifier",
            rating(1, 1, 1, 1, 1),
            CoreLink::Weak,
        );
        let mut gallery = instance(
            "gallery",
            "sys.gallery",
            rating(1, 1, 1, 1, 1),
            CoreLink::Weak,
        );
        gallery.is_meta_only = true;
        let pet = instance("pet", "sys.pet", rating(1, 1, 1, 1, 1), CoreLink::Weak);
        let edges = vec![
            // core 实例 combat 消费 potion；loot provides potion → strong（provides 被 core 消费）。
            edge("combat", InterfacePort::Consumes, "potion", "loot"),
            edge("loot", InterfacePort::Provides, "potion", "combat"),
            // relics modifies core 实例属性 → strong（modifies core 属性）。
            edge(
                "relics",
                InterfacePort::Modifies,
                "combat_attribute",
                "combat",
            ),
            // pet 的边不指向 core 也不供给 core 消费的名词 → weak。
            edge(
                "pet",
                InterfacePort::Provides,
                "companion_visual",
                "gallery",
            ),
        ];
        assert_eq!(
            derive_core_link(&combat, &verbs, &edges),
            CoreLink::Core,
            "动词绑定 → core"
        );
        assert_eq!(
            derive_core_link(&loot, &verbs, &edges),
            CoreLink::Strong,
            "provides 被 core 消费 → strong"
        );
        assert_eq!(
            derive_core_link(&relics, &verbs, &edges),
            CoreLink::Strong,
            "modifies core 属性 → strong"
        );
        assert_eq!(
            derive_core_link(&gallery, &verbs, &edges),
            CoreLink::Meta,
            "is_meta_only → meta"
        );
        assert_eq!(
            derive_core_link(&pet, &verbs, &edges),
            CoreLink::Weak,
            "无判据命中 → weak（保守默认）"
        );
    }

    // ---------------- 结构事实与 serde 纪律 ----------------

    #[test]
    fn empty_composition_is_trivially_clean() {
        let report = check_composition(&input_of(Vec::new(), ProductGrade::HyperCasual));
        assert!(report.blocks.is_empty());
        assert!(report.advices.is_empty());
        assert!(report.h_set.is_empty());
        assert!(report.h_connected, "|H|≤1 平凡连通");
        assert_eq!(report.budget_total, 0.0);
        assert!(!report.form_confirmation_required, "|H|=0 不超参考线 0");
    }

    #[test]
    fn single_heavy_core_passes_all_structural_checks() {
        // 融合案例（附录 §2.2）：H={堆叠合成} 单核，(a) 平凡连通、(b) 无边数要求。
        let merge = instance(
            "merge_stack",
            "custom.merge_stack",
            rating(2, 2, 2, 2, 1),
            CoreLink::Core,
        ); // W9
        let report = check_composition(&input_of(vec![merge], ProductGrade::Casual));
        assert_eq!(report.h_set, vec!["merge_stack"]);
        assert!(report.h_connected);
        assert!(report.blocks.is_empty());
        assert!(report.advices.is_empty(), "|H|=1 ≤ 休闲参考线 1");
    }

    #[test]
    fn composition_types_serde_roundtrip_and_defaults() {
        // I2 旧档守恒：空对象可读为默认值；完整输入往返不丢字段。
        assert_eq!(
            serde_json::from_str::<CompositionInput>("{}").expect("CompositionInput 空对象应可读"),
            CompositionInput::default()
        );
        assert_eq!(
            serde_json::from_str::<CompositionReport>("{}")
                .expect("CompositionReport 空对象应可读"),
            CompositionReport::default()
        );
        assert_eq!(
            serde_json::from_str::<SystemInstance>("{}").expect("SystemInstance 空对象应可读"),
            SystemInstance::default()
        );
        let mut input = input_of(spire_instances(), ProductGrade::MidCore);
        input.core_loop_verbs = vec![("打出卡牌".to_string(), "deck_building".to_string())];
        input.pack_core_nouns = vec!["energy".to_string()];
        input
            .budget
            .grade_budgets
            .insert("mid_core".to_string(), 60.0);
        input.module_tier_orders.insert(
            "sys.equipment".to_string(),
            vec!["e0".to_string(), "e1".to_string()],
        );
        let json = serde_json::to_string(&input).expect("序列化应成功");
        let back: CompositionInput = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(back, input);
        assert_eq!(
            serde_json::to_value(FindingCode::V3aDisconnected).expect("序列化应成功"),
            serde_json::json!("v3a_disconnected")
        );
        assert_eq!(
            serde_json::to_value(ProductGrade::HyperCasual).expect("序列化应成功"),
            serde_json::json!("hyper_casual")
        );
    }

    #[test]
    fn report_is_deterministic_across_instance_orderings() {
        // 确定性守恒：实例乱序输入产出逐字段相同的报告。
        let forward = check_composition(&input_of(spire_instances(), ProductGrade::MidCore));
        let mut reversed_instances = spire_instances();
        reversed_instances.reverse();
        let reversed = check_composition(&input_of(reversed_instances, ProductGrade::MidCore));
        assert_eq!(forward, reversed);
    }
}
