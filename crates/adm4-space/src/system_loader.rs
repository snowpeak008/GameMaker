//! 系统模块加载器（T-W7-3a，定稿 §2.5/§5.1/§5.2）：把 `GenrePack.system_refs`
//! 引用的 `SystemModule` 实例化进设计空间。
//!
//! 分层：`instantiate_system_refs` 是纯函数（接「已加载模块表」，产实例化产物），
//! `load_modules_from_dirs` 单独做 fs 解析——测试用自建模块表喂纯函数，不碰磁盘。
//!
//! 实例化语义（全部 fail-closed，违例 Err 点名）：
//! 1. 版本要求（手写 semver 匹配：空=任意，精确=相等，`^x.y.z`=同主版本且≥基准）；
//! 2. 名词绑定 V6：consumes∪modifies 每名词必须有绑定，目标 = pack 核心名词 或
//!    `<提供方实例>.<名词>`（该实例模块 provides 该名词）；
//! 3. 决策点按 `<module_id>.` → `<instance_id>.` 前缀重写（选项 unlocks、一致性规则、
//!    基数键同步重写），genre_scope 重写为 Pack(pack_id) 过既有校验；
//! 4. tier 合成点 `<instance_id>.tier`（L3 单选、Unlocked）：每允许档一个选项，
//!    unlocks = 该档 activates（累计口径）∪ 所有 tier_gate.rank ≤ 本档 rank 的点
//!    （重写后 id，BTreeSet 确定序）；
//! 5. activates 与 tier_gate 矛盾（tier_gate=T 的点不在 T 档及以上的 activates 里）
//!    → 加载失败：档位承诺与门控两处真相必须一致，漂移即标定笔误。

use crate::model::{ConsistencyRule, GenrePack, SystemInstanceInfo, SystemRef};
use adm4_decision::system_module::SystemModule;
use adm4_decision::{
    DecisionOption, DecisionPoint, DesignLevel, GenreScope, PointRequirement, SelectionMode,
};
use adm4_foundation::{Adm4Error, Adm4Result, read_json_file};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// 一次实例化的全部产物（并进 pack 的内存态，不落盘）。
#[derive(Debug, Clone, Default)]
pub struct SystemInstantiation {
    /// 重写后的模块决策点 + 每实例一个 tier 合成点。
    pub decision_points: Vec<DecisionPoint>,
    /// 重写后的基数期望（键加 `<instance_id>.` 前缀，与决策点 schema 内的键一致）。
    pub cardinality_expectations: BTreeMap<String, adm4_contracts::CardinalityRange>,
    /// serde 级转换 + id/引用重写后的一致性规则。
    pub consistency_rules: Vec<ConsistencyRule>,
    /// 实例信息（module_versions 冻结键与复演比对的源）。
    pub instances: Vec<SystemInstanceInfo>,
}

/// 手写 semver 匹配（设计决定 1，零第三方依赖）。
///
/// - 空要求 = 任意版本；
/// - `^x.y.z` = 主版本相同且实际版本 ≥ 基准（字段逐级数值比较）；
/// - 其余 = 字面精确相等。
///
/// 版本号解析失败（非 `x.y.z` 数字三段）→ Err：静默当不匹配会把标定笔误
/// 报成「版本不满足」，误导修复方向。
pub fn semver_matches(requirement: &str, actual: &str) -> Adm4Result<bool> {
    let requirement = requirement.trim();
    if requirement.is_empty() {
        return Ok(true);
    }
    if let Some(base) = requirement.strip_prefix('^') {
        let base = parse_semver(base)?;
        let actual = parse_semver(actual)?;
        return Ok(actual.0 == base.0 && actual >= base);
    }
    Ok(requirement == actual)
}

fn parse_semver(text: &str) -> Adm4Result<(u64, u64, u64)> {
    let parts: Vec<&str> = text.trim().split('.').collect();
    if parts.len() != 3 {
        return Err(Adm4Error::validation(format!(
            "semver {text} 不是 x.y.z 三段数字形式"
        )));
    }
    let mut numbers = [0u64; 3];
    for (slot, part) in numbers.iter_mut().zip(&parts) {
        *slot = part.parse::<u64>().map_err(|_| {
            Adm4Error::validation(format!("semver {text} 的段 {part} 不是非负整数"))
        })?;
    }
    Ok((numbers[0], numbers[1], numbers[2]))
}

/// 从模块目录集合加载模块表（fs 解析层）：每个目录下按
/// `<module_id>/module.json` 布局扫描；模块结构自校验失败即 Err（入库门槛）。
pub fn load_modules_from_dirs(dirs: &[PathBuf]) -> Adm4Result<BTreeMap<String, SystemModule>> {
    let mut modules = BTreeMap::new();
    for dir in dirs {
        if !dir.is_dir() {
            return Err(Adm4Error::not_found(format!(
                "系统模块目录 {} 不存在",
                dir.display()
            )));
        }
        let entries = fs::read_dir(dir).map_err(|error| {
            Adm4Error::io(format!("read module dir {} failed: {error}", dir.display()))
        })?;
        for entry in entries.flatten() {
            let manifest = entry.path().join("module.json");
            if !manifest.is_file() {
                continue;
            }
            let module = load_module_file(&manifest)?;
            if let Some(previous) = modules.insert(module.module_id.clone(), module) {
                return Err(Adm4Error::validation(format!(
                    "模块 id {} 在多个目录中重复声明（模块 id 是全局命名空间，重复即歧义）",
                    previous.module_id
                )));
            }
        }
    }
    Ok(modules)
}

/// 加载并结构自校验单个 module.json。
pub fn load_module_file(path: &Path) -> Adm4Result<SystemModule> {
    let module: SystemModule = read_json_file(path)?;
    module.validate().map_err(|error| {
        Adm4Error::validation(format!(
            "模块文件 {} 未通过自校验：{}",
            path.display(),
            error.message
        ))
    })?;
    Ok(module)
}

/// 纯函数：把 pack 的 system_refs 按模块表实例化（设计决定 2）。
///
/// 不修改 pack；产物由调用方并进 pack 内存态。`pack_id` 用于 genre_scope 重写。
pub fn instantiate_system_refs(
    pack: &GenrePack,
    modules: &BTreeMap<String, SystemModule>,
) -> Adm4Result<SystemInstantiation> {
    let mut result = SystemInstantiation::default();
    if pack.system_refs.is_empty() {
        return Ok(result);
    }

    // 实例 id 唯一性（决策点前缀即命名空间，重复实例互相踩 id）。
    let mut seen_instances = BTreeSet::new();
    for system_ref in &pack.system_refs {
        if system_ref.instance_id.trim().is_empty() {
            return Err(Adm4Error::validation(format!(
                "品类包 {} 的系统引用（模块 {}）缺少 instance_id",
                pack.pack_id, system_ref.module_id
            )));
        }
        if !seen_instances.insert(system_ref.instance_id.as_str()) {
            return Err(Adm4Error::validation(format!(
                "品类包 {} 的系统实例 id 重复：{}",
                pack.pack_id, system_ref.instance_id
            )));
        }
    }

    // 「实例 → 模块」解析 + 版本检查。
    let mut resolved: Vec<(&SystemRef, &SystemModule)> = Vec::new();
    for system_ref in &pack.system_refs {
        let module = modules.get(&system_ref.module_id).ok_or_else(|| {
            Adm4Error::not_found(format!(
                "系统实例 {} 引用的模块 {} 未加载（检查模块目录与 module_id）",
                system_ref.instance_id, system_ref.module_id
            ))
        })?;
        if !semver_matches(&system_ref.version_req, &module.semver)? {
            return Err(Adm4Error::validation(format!(
                "系统实例 {} 的版本要求 {} 与模块 {} 的实际版本 {} 不匹配",
                system_ref.instance_id, system_ref.version_req, system_ref.module_id, module.semver
            )));
        }
        resolved.push((system_ref, module));
    }

    // 名词绑定 V6（设计决定 3）。合法目标集合先算好：
    // pack 核心名词 + 每个实例 provides 的 `<instance_id>.<名词>`。
    let core_nouns: BTreeSet<&str> = pack.core_nouns.iter().map(String::as_str).collect();
    let mut provided: BTreeSet<String> = BTreeSet::new();
    for (system_ref, module) in &resolved {
        for noun in &module.interface.provides {
            provided.insert(format!("{}.{}", system_ref.instance_id, local_noun(noun)));
        }
    }
    for (system_ref, module) in &resolved {
        let consumed_or_modified = module
            .interface
            .consumes
            .iter()
            .map(|noun| ("consumes", noun))
            .chain(
                module
                    .interface
                    .modifies
                    .iter()
                    .map(|noun| ("modifies", noun)),
            );
        for (port, noun) in consumed_or_modified {
            let Some(target) = system_ref.noun_bindings.get(noun) else {
                return Err(Adm4Error::validation(format!(
                    "V6 绑定悬空：实例 {} 的 {port} 名词 {noun} 没有绑定目标\
                     （noun_bindings 必须覆盖 consumes∪modifies 的每个名词）",
                    system_ref.instance_id
                )));
            };
            if core_nouns.contains(target.as_str()) || provided.contains(target) {
                continue;
            }
            return Err(Adm4Error::validation(format!(
                "V6 绑定悬空：实例 {} 的 {port} 名词 {noun} 绑定到 {target}，\
                 但它既不是 pack 核心名词，也没有任何实例 provides 它\
                 （合法目标 = 核心名词 或 <提供方实例>.<名词>）",
                system_ref.instance_id
            )));
        }
    }

    // 逐实例实例化：决策点重写 + tier 合成点 + 基数/一致性规则重写。
    for (system_ref, module) in &resolved {
        instantiate_one(pack, system_ref, module, &mut result)?;
        result.instances.push(SystemInstanceInfo {
            instance_id: system_ref.instance_id.clone(),
            module_id: system_ref.module_id.clone(),
            semver: module.semver.clone(),
        });
    }
    Ok(result)
}

/// 外部命名空间名词（带点号）取末段做绑定名；裸名词原样。
///
/// provides 声明的是本模块名词（裸 id），绑定目标形如 `<实例>.<名词>`——
/// 这里统一取名词本名，防止「provides 里误写带点号形式」造成目标永远拼不上。
fn local_noun(noun: &str) -> &str {
    noun.rsplit('.').next().unwrap_or(noun)
}

/// 单实例实例化（调用前版本与绑定已验）。
fn instantiate_one(
    pack: &GenrePack,
    system_ref: &SystemRef,
    module: &SystemModule,
    result: &mut SystemInstantiation,
) -> Adm4Result<()> {
    let instance = system_ref.instance_id.as_str();
    let module_prefix = format!("{}.", module.module_id);
    let instance_prefix = format!("{instance}.");
    let tier_point_id = format!("{instance}.tier");
    // id 重写是纯字符串前缀替换（§2.5：逐字节确定）。
    let rewrite = |id: &str| -> String {
        match id.strip_prefix(&module_prefix) {
            Some(rest) => format!("{instance_prefix}{rest}"),
            None => id.to_string(),
        }
    };

    // 允许档位集合（空 = 全部），并验证每个允许档真实存在。
    let allowed: Vec<usize> = if system_ref.allowed_tiers.is_empty() {
        (0..module.heaviness.tiers.len()).collect()
    } else {
        let mut ranks = Vec::with_capacity(system_ref.allowed_tiers.len());
        for tier_id in &system_ref.allowed_tiers {
            let rank = module.heaviness.tier_rank(tier_id).ok_or_else(|| {
                Adm4Error::validation(format!(
                    "实例 {instance} 的 allowed_tiers 引用了模块 {} 不存在的档位 {tier_id}",
                    module.module_id
                ))
            })?;
            ranks.push(rank);
        }
        ranks.sort_unstable();
        ranks.dedup();
        ranks
    };
    if allowed.is_empty() {
        return Err(Adm4Error::validation(format!(
            "实例 {instance} 无可选档位（模块 {} 的重度阶梯为空或 allowed_tiers 为空集）",
            module.module_id
        )));
    }
    // allowed_tiers 收窄后的不可达点：tier_gate 高于最高允许档的点永远不会被任何
    // tier 选项 unlock，而「无人声明 unlock 的点」在适用性规则里是根点（恒 Active）
    // ——不剔除它们会把永远打不开的设计问题塞进完成度分母。连同引用它们的一致性
    // 规则一起剔除（规则管辖的点不存在，留着必然被 space 校验判悬空）。
    let max_allowed_rank = *allowed.iter().max().unwrap_or(&0);
    let reachable = |point: &DecisionPoint| -> bool {
        match &point.tier_gate {
            None => true,
            Some(gate) => module
                .heaviness
                .tier_rank(gate)
                .is_some_and(|rank| rank <= max_allowed_rank),
        }
    };
    let excluded: BTreeSet<String> = module
        .decision_points
        .iter()
        .filter(|point| !reachable(point))
        .map(|point| rewrite(&point.id))
        .collect();

    // activates 与 tier_gate 矛盾检查（设计决定 4）：tier_gate=T 的点必须出现在
    // T 档（及以上，累计口径下等价于检查 T 档本档）的 activates 里。
    for point in &module.decision_points {
        let Some(gate) = &point.tier_gate else {
            continue;
        };
        let Some(gate_rank) = module.heaviness.tier_rank(gate) else {
            // module.validate 已拦，这里防御性重述。
            return Err(Adm4Error::validation(format!(
                "实例 {instance}：决策点 {} 的 tier_gate={gate} 不在模块 {} 的阶梯中",
                point.id, module.module_id
            )));
        };
        let covered = module
            .heaviness
            .tiers
            .iter()
            .enumerate()
            .filter(|(rank, _)| *rank >= gate_rank)
            .all(|(_, tier)| tier.activates.iter().any(|id| id == &point.id));
        if !covered {
            return Err(Adm4Error::validation(format!(
                "实例 {instance}：模块 {} 的决策点 {} 声明 tier_gate={gate}，\
                 但该档（含以上）的 activates 未全部包含它——档位承诺与门控矛盾，标定笔误",
                module.module_id, point.id
            )));
        }
    }

    // 模块决策点重写并入（genre_scope 重写为 Pack(pack_id) 过既有校验；
    // allowed_tiers 不可达点剔除，见上）。
    for point in &module.decision_points {
        let mut rewritten = point.clone();
        rewritten.id = rewrite(&point.id);
        if excluded.contains(&rewritten.id) {
            continue;
        }
        rewritten.genre_scope = GenreScope::Pack(pack.pack_id.clone());
        for option in &mut rewritten.options {
            for unlocked in &mut option.unlocks {
                *unlocked = rewrite(unlocked);
            }
            // 被剔除的不可达点不能留在任何 unlocks 里（dangling_unlock 会拦装配）。
            option.unlocks.retain(|target| !excluded.contains(target));
            // C0 机制归属重写：模块点的 `system` 标签写的是模块 id（模块作者只知道
            // 自己），但 spec 里的 SystemSpec 由 tier 合成点（L3）产出、id 是
            // `<instance>.tier`——不重写则 C0 按 mechanic_dangling_system 拦死全链。
            if let Some(tag) = option.compiler_tags.get_mut("system")
                && tag == &module.module_id
            {
                *tag = tier_point_id.clone();
            }
            rewrite_schema_references(&mut option.parameter_schema, instance, &rewrite);
        }
        result.decision_points.push(rewritten);
    }

    // tier 合成点（设计决定 4）：L3 单选、Unlocked；每允许档一个选项；
    // unlocks = 该档 activates（累计） ∪ tier_gate.rank ≤ 本档 rank 的点（BTreeSet 确定序）。
    let mut tier_options = Vec::with_capacity(allowed.len());
    for rank in &allowed {
        let tier = &module.heaviness.tiers[*rank];
        let mut unlocks: BTreeSet<String> = tier
            .activates
            .iter()
            .map(|id| rewrite(id))
            .filter(|id| !excluded.contains(id))
            .collect();
        for point in &module.decision_points {
            if let Some(gate) = &point.tier_gate
                && let Some(gate_rank) = module.heaviness.tier_rank(gate)
                && gate_rank <= *rank
            {
                unlocks.insert(rewrite(&point.id));
            }
        }
        tier_options.push(DecisionOption {
            id: tier.id.clone(),
            label: if tier.label_zh.is_empty() {
                tier.id.clone()
            } else {
                tier.label_zh.clone()
            },
            summary: tier.summary.clone(),
            unlocks: unlocks.into_iter().collect(),
            ..Default::default()
        });
    }
    result.decision_points.push(DecisionPoint {
        id: tier_point_id,
        domain: module.module_id.clone(),
        level: DesignLevel::L3,
        genre_scope: GenreScope::Pack(pack.pack_id.clone()),
        question: format!("{}（实例 {instance}）做到哪个重度档？", module.label_zh),
        mda_layer: None,
        design_question: None,
        node_id: None,
        selection_mode: SelectionMode::Single,
        requirement: PointRequirement::Unlocked,
        tier_gate: None,
        options: tier_options,
        skin_fields: Vec::new(),
        evidence_slots: false,
    });

    // 基数期望重写并入（键加实例前缀，与上面 schema 内键的重写一致）。
    for (key, range) in &module.cardinality_expectations {
        result
            .cardinality_expectations
            .insert(format!("{instance_prefix}{key}"), *range);
    }

    // 一致性规则：serde 级转换（模块侧 ConsistencyRule 与 adm4-space 版 JSON 同形），
    // 规则 id 与决策引用同步重写；引用了不可达点的规则连带剔除（点已不在图上，
    // 留着必被 space 校验判悬空）。
    for rule in &module.consistency_rules {
        let value = serde_json::to_value(rule)
            .map_err(|error| Adm4Error::internal(format!("模块一致性规则序列化失败：{error}")))?;
        let mut converted: ConsistencyRule = serde_json::from_value(value).map_err(|error| {
            Adm4Error::validation(format!(
                "实例 {instance}：模块 {} 的一致性规则 {} 不符合 pack 规则形态：{error}",
                module.module_id, rule.id
            ))
        })?;
        converted.id = format!("{instance_prefix}{}", converted.id);
        rewrite_rule_references(&mut converted, &rewrite);
        if rule_references(&converted)
            .iter()
            .any(|id| excluded.contains(*id))
        {
            continue;
        }
        result.consistency_rules.push(converted);
    }
    Ok(())
}

/// 决策点 schema 内引用的前缀重写：基数键加实例前缀（与
/// `cardinality_expectations` 的键重写同步，保证 space validate 的键匹配不断）；
/// 矩阵轴引用的模块内决策 id 按同一前缀替换规则重写。
fn rewrite_schema_references(
    schema: &mut adm4_decision::ParameterSchema,
    instance: &str,
    rewrite: &impl Fn(&str) -> String,
) {
    use adm4_decision::ParameterSchema;
    let rewrite_key = |key: &mut String| {
        if !key.is_empty() {
            *key = format!("{instance}.{key}");
        }
    };
    match schema {
        ParameterSchema::Table(table) => rewrite_key(&mut table.cardinality_key),
        ParameterSchema::Matrix(matrix) => {
            rewrite_key(&mut matrix.cardinality_key);
            for axis in [&mut matrix.row_axis, &mut matrix.col_axis] {
                match axis {
                    adm4_decision::AxisRef::DecisionOptions { decision }
                    | adm4_decision::AxisRef::TableRows { decision } => {
                        *decision = rewrite(decision);
                    }
                }
            }
        }
        ParameterSchema::Graph(graph) => rewrite_key(&mut graph.cardinality_key),
        ParameterSchema::Curve(curve) => rewrite_key(&mut curve.cardinality_key),
        ParameterSchema::None | ParameterSchema::Scalar { .. } => {}
    }
}

/// 一致性规则引用的全部决策 id（不可达点连带剔除规则时用）。
fn rule_references(rule: &ConsistencyRule) -> Vec<&str> {
    use crate::model::ConsistencyRuleKind;
    match &rule.kind {
        ConsistencyRuleKind::MatrixAxisMatchesTableRows {
            matrix_decision,
            table_decision,
        } => vec![matrix_decision, table_decision],
        ConsistencyRuleKind::AnsweredTogether { first, second } => vec![first, second],
        ConsistencyRuleKind::RowReference {
            source_decision,
            target_decision,
            ..
        } => vec![source_decision, target_decision],
    }
}

/// 一致性规则内决策引用的重写。
fn rewrite_rule_references(rule: &mut ConsistencyRule, rewrite: &impl Fn(&str) -> String) {
    use crate::model::ConsistencyRuleKind;
    match &mut rule.kind {
        ConsistencyRuleKind::MatrixAxisMatchesTableRows {
            matrix_decision,
            table_decision,
        } => {
            *matrix_decision = rewrite(matrix_decision);
            *table_decision = rewrite(table_decision);
        }
        ConsistencyRuleKind::AnsweredTogether { first, second } => {
            *first = rewrite(first);
            *second = rewrite(second);
        }
        ConsistencyRuleKind::RowReference {
            source_decision,
            target_decision,
            ..
        } => {
            *source_decision = rewrite(source_decision);
            *target_decision = rewrite(target_decision);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ConsistencyRuleKind;
    use adm4_decision::system_module::{
        FiveAxisRating, HeavinessLadder, HeavinessTier, MdaMapping, NounDecl, NounKind,
        SystemInterface,
    };
    use adm4_decision::{DecisionOption, DecisionPoint, DesignLevel, PointRequirement};

    fn option(id: &str) -> DecisionOption {
        DecisionOption {
            id: id.into(),
            label: id.into(),
            ..Default::default()
        }
    }

    fn module_point(id: &str, tier_gate: &str) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "meter".into(),
            level: DesignLevel::L4,
            genre_scope: GenreScope::Universal,
            question: format!("{id}？"),
            mda_layer: None,
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: Some(tier_gate.into()),
            options: vec![option("a"), option("b")],
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    fn tier(id: &str, activates: &[&str]) -> HeavinessTier {
        HeavinessTier {
            id: id.into(),
            label_zh: id.into(),
            rating: FiveAxisRating {
                m: 1,
                d: 0,
                c: 0,
                p: 1,
                o: 0,
            },
            p_floor: 1,
            interface_floor: 0,
            activates: activates.iter().map(|point| (*point).to_string()).collect(),
            inductions: Vec::new(),
            summary: String::new(),
        }
    }

    /// 两档三点的合成模块：t0 激活 fill_rule，t1 追加 decay_rule/cap_rule。
    fn meter_module() -> SystemModule {
        let mut cap_point = module_point("sys.meter.cap_rule", "t1");
        // 选项 unlocks 指向同档另一点 + `system` 标签指向模块 id（C0 归属重写的对象）。
        cap_point.options[0].unlocks = vec!["sys.meter.decay_rule".into()];
        cap_point.options[0]
            .compiler_tags
            .insert("system".into(), "sys.meter".into());
        SystemModule {
            module_id: "sys.meter".into(),
            semver: "1.2.0".into(),
            label_zh: "计量".into(),
            summary: "测试用计量系统".into(),
            nouns: vec![NounDecl {
                id: "meter_value".into(),
                kind: NounKind::Resource,
                label_zh: "计量值".into(),
                summary: String::new(),
            }],
            interface: SystemInterface {
                provides: vec!["meter_value".into()],
                consumes: vec!["sys.energy.charge".into()],
                modifies: Vec::new(),
            },
            mda: MdaMapping {
                mechanics_summary: "计量机制".into(),
                dynamics_notes: Vec::new(),
                aesthetics_primary: vec!["挑战".into()],
            },
            heaviness: HeavinessLadder {
                tiers: vec![
                    tier("t0", &["sys.meter.fill_rule"]),
                    tier(
                        "t1",
                        &[
                            "sys.meter.fill_rule",
                            "sys.meter.decay_rule",
                            "sys.meter.cap_rule",
                        ],
                    ),
                ],
            },
            decision_points: vec![
                module_point("sys.meter.fill_rule", "t0"),
                module_point("sys.meter.decay_rule", "t1"),
                cap_point,
            ],
            cardinality_expectations: [(
                "meter_rows".to_string(),
                adm4_contracts::CardinalityRange { min: 1, max: 9 },
            )]
            .into_iter()
            .collect(),
            consistency_rules: vec![adm4_decision::system_module::ConsistencyRule {
                id: "decay_and_cap_together".into(),
                kind: adm4_decision::system_module::ConsistencyRuleKind::AnsweredTogether {
                    first: "sys.meter.decay_rule".into(),
                    second: "sys.meter.cap_rule".into(),
                },
            }],
            skin_fields: Vec::new(),
        }
    }

    fn pack_with_refs(refs: Vec<SystemRef>) -> GenrePack {
        GenrePack {
            pack_id: "loader_test".into(),
            pack_version: "0.1.0".into(),
            display_name: "加载器测试包".into(),
            reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
            profile_points: Vec::new(),
            cardinality_expectations: Default::default(),
            consistency_rules: Vec::new(),
            nodes: Vec::new(),
            decision_points: Vec::new(),
            system_refs: refs,
            core_nouns: vec!["energy".into()],
        }
    }

    fn meter_ref(instance_id: &str) -> SystemRef {
        SystemRef {
            instance_id: instance_id.into(),
            module_id: "sys.meter".into(),
            version_req: String::new(),
            allowed_tiers: Vec::new(),
            noun_bindings: [("sys.energy.charge".to_string(), "energy".to_string())]
                .into_iter()
                .collect(),
            core_link: Default::default(),
        }
    }

    fn modules() -> BTreeMap<String, SystemModule> {
        [("sys.meter".to_string(), meter_module())]
            .into_iter()
            .collect()
    }

    // ---------------- semver ----------------

    #[test]
    fn semver_matching_covers_empty_exact_and_caret() {
        assert!(semver_matches("", "9.9.9").unwrap());
        assert!(semver_matches("1.2.0", "1.2.0").unwrap());
        assert!(!semver_matches("1.2.0", "1.2.1").unwrap());
        assert!(semver_matches("^1.2.0", "1.3.5").unwrap());
        assert!(!semver_matches("^1.2.0", "1.1.9").unwrap());
        assert!(!semver_matches("^1.2.0", "2.0.0").unwrap());
        assert!(semver_matches("^0.1.0", "0.1.0").unwrap());
        // 非 x.y.z 形式 → Err（不静默当不匹配）。
        assert!(semver_matches("^1.2", "1.2.0").is_err());
        assert!(semver_matches("^1.2.0", "1.2").is_err());
    }

    // ---------------- 命名空间重写与 tier 合成点 ----------------

    #[test]
    fn instantiation_rewrites_namespace_and_synthesizes_tier_point() {
        let pack = pack_with_refs(vec![meter_ref("meter_main")]);
        let result = instantiate_system_refs(&pack, &modules()).unwrap();

        let ids: Vec<&str> = result
            .decision_points
            .iter()
            .map(|point| point.id.as_str())
            .collect();
        assert!(ids.contains(&"meter_main.fill_rule"), "{ids:?}");
        assert!(ids.contains(&"meter_main.tier"), "{ids:?}");

        // 选项 unlocks 与 `system` 归属标签都重写（后者指向 tier 合成点）。
        let cap = result
            .decision_points
            .iter()
            .find(|point| point.id == "meter_main.cap_rule")
            .unwrap();
        assert_eq!(cap.options[0].unlocks, vec!["meter_main.decay_rule"]);
        assert_eq!(
            cap.options[0]
                .compiler_tags
                .get("system")
                .map(String::as_str),
            Some("meter_main.tier")
        );

        // tier 合成点：L3 单选 Unlocked，每档一个选项；t1 的 unlocks 覆盖全部三点。
        let tier_point = result
            .decision_points
            .iter()
            .find(|point| point.id == "meter_main.tier")
            .unwrap();
        assert_eq!(tier_point.level, DesignLevel::L3);
        assert_eq!(tier_point.options.len(), 2);
        let t1 = tier_point.options.iter().find(|o| o.id == "t1").unwrap();
        assert_eq!(
            t1.unlocks,
            vec![
                "meter_main.cap_rule",
                "meter_main.decay_rule",
                "meter_main.fill_rule"
            ]
        );

        // 基数键与一致性规则同步重写。
        assert!(
            result
                .cardinality_expectations
                .contains_key("meter_main.meter_rows")
        );
        assert_eq!(result.consistency_rules.len(), 1);
        match &result.consistency_rules[0].kind {
            ConsistencyRuleKind::AnsweredTogether { first, second } => {
                assert_eq!(first, "meter_main.decay_rule");
                assert_eq!(second, "meter_main.cap_rule");
            }
            other => panic!("规则种类不应改变：{other:?}"),
        }

        // 实例信息（冻结 module_versions 的源）。
        assert_eq!(result.instances.len(), 1);
        assert_eq!(result.instances[0].module_id, "sys.meter");
        assert_eq!(result.instances[0].semver, "1.2.0");
    }

    /// 同模块双实例互不冲突（命名空间隔离）；实例 id 重复即 Err。
    #[test]
    fn dual_instances_coexist_and_duplicate_instance_id_is_rejected() {
        let pack = pack_with_refs(vec![meter_ref("meter_main"), meter_ref("meter_alt")]);
        let result = instantiate_system_refs(&pack, &modules()).unwrap();
        let ids: Vec<&str> = result
            .decision_points
            .iter()
            .map(|point| point.id.as_str())
            .collect();
        assert!(ids.contains(&"meter_main.tier"));
        assert!(ids.contains(&"meter_alt.tier"));
        assert!(ids.contains(&"meter_main.fill_rule"));
        assert!(ids.contains(&"meter_alt.fill_rule"));

        let duplicated = pack_with_refs(vec![meter_ref("meter_main"), meter_ref("meter_main")]);
        let error = instantiate_system_refs(&duplicated, &modules()).unwrap_err();
        assert!(error.message.contains("meter_main"), "{}", error.message);
        assert!(error.message.contains("重复"), "{}", error.message);
    }

    // ---------------- allowed_tiers 收窄：不可达点剔除 ----------------

    #[test]
    fn narrowed_allowed_tiers_prune_unreachable_points_and_their_rules() {
        let mut narrowed = meter_ref("meter_main");
        narrowed.allowed_tiers = vec!["t0".into()];
        let pack = pack_with_refs(vec![narrowed]);
        let result = instantiate_system_refs(&pack, &modules()).unwrap();

        let ids: Vec<&str> = result
            .decision_points
            .iter()
            .map(|point| point.id.as_str())
            .collect();
        assert!(ids.contains(&"meter_main.fill_rule"));
        assert!(
            !ids.contains(&"meter_main.decay_rule"),
            "tier_gate=t1 的点在只允许 t0 时必须剔除（否则成恒 Active 根点进分母）：{ids:?}"
        );
        assert!(!ids.contains(&"meter_main.cap_rule"));

        // tier 合成点只剩 t0 一个选项。
        let tier_point = result
            .decision_points
            .iter()
            .find(|point| point.id == "meter_main.tier")
            .unwrap();
        assert_eq!(tier_point.options.len(), 1);
        assert_eq!(tier_point.options[0].id, "t0");

        // 引用被剔除点的一致性规则连带剔除。
        assert!(result.consistency_rules.is_empty());

        // 不存在的档位 → Err 点名。
        let mut ghost = meter_ref("meter_main");
        ghost.allowed_tiers = vec!["t9".into()];
        let error = instantiate_system_refs(&pack_with_refs(vec![ghost]), &modules()).unwrap_err();
        assert!(error.message.contains("t9"), "{}", error.message);
    }

    // ---------------- 失败点名：版本 / 绑定 / 门控矛盾 / 模块缺失 ----------------

    #[test]
    fn version_requirement_mismatch_names_instance_and_versions() {
        let mut too_new = meter_ref("meter_main");
        too_new.version_req = "^2.0.0".into();
        let error =
            instantiate_system_refs(&pack_with_refs(vec![too_new]), &modules()).unwrap_err();
        assert!(error.message.contains("meter_main"), "{}", error.message);
        assert!(error.message.contains("^2.0.0"), "{}", error.message);
        assert!(error.message.contains("1.2.0"), "{}", error.message);
    }

    #[test]
    fn dangling_noun_binding_names_instance_port_and_noun() {
        // 缺绑定。
        let mut unbound = meter_ref("meter_main");
        unbound.noun_bindings.clear();
        let error =
            instantiate_system_refs(&pack_with_refs(vec![unbound]), &modules()).unwrap_err();
        assert!(error.message.contains("V6"), "{}", error.message);
        assert!(error.message.contains("meter_main"), "{}", error.message);
        assert!(
            error.message.contains("sys.energy.charge"),
            "{}",
            error.message
        );

        // 绑定到既非核心名词也无人 provides 的目标。
        let mut dangling = meter_ref("meter_main");
        dangling
            .noun_bindings
            .insert("sys.energy.charge".into(), "ghost_target".into());
        let error =
            instantiate_system_refs(&pack_with_refs(vec![dangling]), &modules()).unwrap_err();
        assert!(error.message.contains("ghost_target"), "{}", error.message);

        // 绑定到另一实例 provides 的名词 → 放行（<提供方实例>.<名词> 形态）。
        let mut provided = meter_ref("meter_consumer");
        provided
            .noun_bindings
            .insert("sys.energy.charge".into(), "meter_main.meter_value".into());
        let pack = pack_with_refs(vec![meter_ref("meter_main"), provided]);
        instantiate_system_refs(&pack, &modules()).expect("实例间供给绑定应放行");
    }

    #[test]
    fn tier_gate_activates_contradiction_is_rejected_with_point_name() {
        let mut module = meter_module();
        // orphan_rule 声明 tier_gate=t0，但任何档的 activates 都不含它——矛盾。
        module
            .decision_points
            .push(module_point("sys.meter.orphan_rule", "t0"));
        let modules: BTreeMap<String, SystemModule> =
            [("sys.meter".to_string(), module)].into_iter().collect();
        let error =
            instantiate_system_refs(&pack_with_refs(vec![meter_ref("meter_main")]), &modules)
                .unwrap_err();
        assert!(error.message.contains("orphan_rule"), "{}", error.message);
        assert!(error.message.contains("矛盾"), "{}", error.message);
    }

    #[test]
    fn missing_module_is_named() {
        let error = instantiate_system_refs(
            &pack_with_refs(vec![meter_ref("meter_main")]),
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(error.message.contains("sys.meter"), "{}", error.message);
        assert!(error.message.contains("meter_main"), "{}", error.message);
    }
}
