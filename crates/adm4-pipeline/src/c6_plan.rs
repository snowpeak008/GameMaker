use crate::c3_content::ContentInventoryContract;
use crate::c4_capabilities::{CapabilitiesContract, MAX_EFFECT_DEPTH};
use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_contracts::{EvidencePointer, MeasuredMetric, SpecRef};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::{EffectSpec, GameSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub kind: String, // "program" | "asset" | "assembly"
    pub title: String,
    pub source_refs: Vec<SpecRef>,
    pub depends_on: Vec<String>,
}

/// C6 契约：任务图 + 与 C3/C4 的真实全量对账 + Phase1 签收（人工门）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskGraphContract {
    pub tasks: Vec<TaskNode>,
    /// 对账覆盖率（R1：逐条证据，禁止硬编码分数）。
    pub reconciliation: MeasuredMetric,
    pub missing: Vec<String>,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;
    let content: ContentInventoryContract = ctx.store.read_contract("C3")?;
    let capabilities: CapabilitiesContract = ctx.store.read_contract("C4")?;

    // 系统 → 编译期 domain（决策图查询；多选点系统 id 形如 `决策id#选项id`，
    // 按 `#` 前缀归属同一决策点的 domain）。查不到的系统各自成组（不猜归属）。
    let domain_of = |system_id: &str| -> Option<String> {
        let decision_id = system_id.split('#').next().unwrap_or(system_id);
        ctx.space
            .graph
            .points()
            .iter()
            .find(|point| point.id == decision_id)
            .map(|point| point.domain.clone())
    };
    let system_domains: Vec<Option<String>> = spec
        .systems
        .iter()
        .map(|system| domain_of(&system.id))
        .collect();

    let tasks = build_tasks(&spec, &content, &capabilities, &system_domains)?;

    // 真实对账：每个能力、每条资产需求必须有对应任务；缺一列一。
    let mut evidence = Vec::new();
    let mut missing = Vec::new();
    let mut checked = 0usize;
    let mut covered = 0usize;
    for capability in &capabilities.capabilities {
        checked += 1;
        let task_id = format!("task_{}", capability.id);
        if tasks.iter().any(|task| task.id == task_id) {
            covered += 1;
            evidence.push(EvidencePointer {
                file: "C6/contract.json".into(),
                path: format!("capabilities/{}", capability.id),
                observed: format!("任务 {task_id} 存在"),
            });
        } else {
            missing.push(format!("能力 {} 无对应程序任务", capability.id));
        }
    }
    for asset in &content.assets {
        checked += 1;
        let task_id = format!("task_{}", asset.id);
        if tasks.iter().any(|task| task.id == task_id) {
            covered += 1;
            evidence.push(EvidencePointer {
                file: "C6/contract.json".into(),
                path: format!("assets/{}", asset.id),
                observed: format!("任务 {task_id} 存在"),
            });
        } else {
            missing.push(format!("资产需求 {} 无对应任务", asset.id));
        }
    }
    if !missing.is_empty() {
        return Err(Adm4Error::validation(format!(
            "C6 对账缺口 {} 项：{}",
            missing.len(),
            missing.join("; ")
        )));
    }
    let reconciliation = MeasuredMetric::new(
        if checked == 0 {
            0.0
        } else {
            covered as f64 / checked as f64
        },
        evidence,
    )?;

    let contract = TaskGraphContract {
        tasks: tasks.clone(),
        reconciliation,
        missing: Vec::new(),
    };
    let mut document = format!(
        "# C6 开发计划\n\n- 任务总数：{}（程序 {} / 资产 {} / 装配 {}）\n- 对账覆盖率：{:.0}%（逐条证据见 contract.json）\n\n| 任务 | 类型 | 依赖 |\n|------|------|------|\n",
        tasks.len(),
        tasks.iter().filter(|task| task.kind == "program").count(),
        tasks.iter().filter(|task| task.kind == "asset").count(),
        tasks.iter().filter(|task| task.kind == "assembly").count(),
        contract.reconciliation.value() * 100.0
    );
    for task in &tasks {
        document.push_str(&format!(
            "| {} | {} | {} |\n",
            task.title,
            task.kind,
            if task.depends_on.is_empty() {
                "-".to_string()
            } else {
                task.depends_on.join(", ")
            }
        ));
    }
    document
        .push_str("\n**等待 Phase 1 人工签收。**\n\n> 本文档由 contract.json 渲染，请勿手改。\n");
    ctx.store.write_stage("C6", &contract, &document)?;
    Ok(StageStatus::WaitingHuman {
        gate: "phase1_signoff".into(),
    })
}

/// 任务图构造（T-W7-1b，审计 B 整改 ③）：
/// - 程序任务：每个能力一条（id 锚定机制）；机制内 ModifyRule（含嵌套）产
///   跨机制依赖边——target_rule 所属机制的程序任务 → 本机制的程序任务；
/// - 资产任务：每条资产需求一条；
/// - 装配任务：按编译期 domain 聚合去重——同 domain 的多个系统合并成一条，
///   标题说明覆盖 N 个决策（同名装配任务重复 6 次的病根）；查不到 domain 的
///   系统各自成条（不猜归属），单系统组保持既有 id/标题形状（金样零漂移）。
fn build_tasks(
    spec: &GameSpec,
    content: &ContentInventoryContract,
    capabilities: &CapabilitiesContract,
    system_domains: &[Option<String>],
) -> Adm4Result<Vec<TaskNode>> {
    let mechanic_ids: BTreeSet<&str> = spec
        .mechanics
        .iter()
        .map(|mechanic| mechanic.id.as_str())
        .collect();

    let mut tasks = Vec::new();
    for capability in &capabilities.capabilities {
        // 能力 id 形如 `cap_<机制id>`（C4 投影不变量）；据此回查机制拿 ModifyRule 边。
        let mechanic_id = capability
            .id
            .strip_prefix("cap_")
            .unwrap_or(capability.id.as_str());
        let mut depends_on = Vec::new();
        if let Some(mechanic) = spec
            .mechanics
            .iter()
            .find(|mechanic| mechanic.id == mechanic_id)
        {
            let mut rule_targets = BTreeSet::new();
            for effect in &mechanic.effects {
                collect_rule_targets(&mechanic.id, effect, 1, &mut rule_targets)?;
            }
            for target in rule_targets {
                // 悬空目标由 C1 复检拦截，这里不产边；自指不成边（不构成开发顺序约束）。
                if target != mechanic.id && mechanic_ids.contains(target.as_str()) {
                    let edge = format!("task_cap_{target}");
                    if !depends_on.contains(&edge) {
                        depends_on.push(edge);
                    }
                }
            }
        }
        tasks.push(TaskNode {
            id: format!("task_{}", capability.id),
            kind: "program".into(),
            title: format!("实现 {}", capability.interface_name),
            source_refs: capability.source_refs.clone(),
            depends_on,
        });
    }

    for asset in &content.assets {
        tasks.push(TaskNode {
            id: format!("task_{}", asset.id),
            kind: "asset".into(),
            title: format!("制作资产 {}", asset.id),
            source_refs: vec![SpecRef::new(format!("entities/{}", asset.entity_id))],
            depends_on: Vec::new(),
        });
    }

    // 装配分组：同 domain 合并（首见序稳定）；无 domain 的系统各自成组。
    let mut groups: Vec<(Option<&str>, Vec<usize>)> = Vec::new();
    for (index, domain) in system_domains.iter().enumerate() {
        match domain.as_deref() {
            Some(domain_id) => match groups.iter_mut().find(|(key, _)| *key == Some(domain_id)) {
                Some((_, members)) => members.push(index),
                None => groups.push((Some(domain_id), vec![index])),
            },
            None => groups.push((None, vec![index])),
        }
    }
    for (domain, members) in groups {
        let systems: Vec<&adm4_spec::SystemSpec> =
            members.iter().map(|index| &spec.systems[*index]).collect();
        let mut depends_on = Vec::new();
        for system in &systems {
            for mechanic in &spec.mechanics {
                if mechanic.system_id == system.id {
                    let task_id = format!("task_cap_{}", mechanic.id);
                    if !depends_on.contains(&task_id) {
                        depends_on.push(task_id);
                    }
                }
            }
        }
        let source_refs: Vec<SpecRef> = systems
            .iter()
            .map(|system| SpecRef::new(format!("systems/{}", system.id)))
            .collect();
        let (id, title) = match (domain, systems.as_slice()) {
            // 单系统组：保持既有 id 与标题形状（金样既有键值零漂移）。
            (_, [only]) => (
                format!("task_assemble_{}", only.id),
                format!("装配系统 {}", only.name),
            ),
            (Some(domain_id), _) => {
                // 多选点系统 id 形如 `决策id#选项id`：按 `#` 前缀数覆盖的决策点。
                let decisions: BTreeSet<&str> = systems
                    .iter()
                    .map(|system| system.id.split('#').next().unwrap_or(&system.id))
                    .collect();
                (
                    format!("task_assemble_domain_{domain_id}"),
                    format!(
                        "装配 {domain_id} 领域系统（覆盖 {} 个决策）",
                        decisions.len()
                    ),
                )
            }
            // 分组规则保证 None 组恒为单系统，穷尽匹配兜底沿用单系统形状。
            (None, _) => (
                format!("task_assemble_{}", systems[0].id),
                format!("装配系统 {}", systems[0].name),
            ),
        };
        tasks.push(TaskNode {
            id,
            kind: "assembly".into(),
            title,
            source_refs,
            depends_on,
        });
    }
    Ok(tasks)
}

/// 递归收集效果树里全部 ModifyRule 的 target_rule（穷尽匹配无 `_` 臂）；
/// 深度纪律与 C4 渲染/收集同款：超过 [`MAX_EFFECT_DEPTH`] 即结构化 Err 点名机制 id。
fn collect_rule_targets(
    mechanic_id: &str,
    effect: &EffectSpec,
    depth: usize,
    targets: &mut BTreeSet<String>,
) -> Adm4Result<()> {
    if depth > MAX_EFFECT_DEPTH {
        return Err(Adm4Error::validation(format!(
            "机制 {mechanic_id} 的效果嵌套深度超过上限 {MAX_EFFECT_DEPTH}（C6 依赖边收集中止）"
        )));
    }
    match effect {
        EffectSpec::ModifyRule { target_rule, .. } => {
            targets.insert(target_rule.clone());
        }
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            for nested in inner {
                collect_rule_targets(mechanic_id, nested, depth + 1, targets)?;
            }
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => {
            for nested in on_success.iter().chain(on_failure.iter()) {
                collect_rule_targets(mechanic_id, nested, depth + 1, targets)?;
            }
        }
        EffectSpec::ModifyProperty { .. }
        | EffectSpec::SpawnEntity { .. }
        | EffectSpec::DespawnEntity { .. }
        | EffectSpec::ChangeState { .. }
        | EffectSpec::GrantResource { .. }
        | EffectSpec::ConsumeResource { .. }
        | EffectSpec::EmitSignal { .. }
        | EffectSpec::Displace { .. }
        | EffectSpec::Attach { .. }
        | EffectSpec::Detach { .. }
        | EffectSpec::DrawFromPool { .. }
        | EffectSpec::Custom { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c4_capabilities::CapabilityContract;
    use adm4_contracts::{CardinalityDeclaration, CardinalityRange};
    use adm4_spec::{
        MechanicSpec, ProjectIntent, RulePatch, SPEC_SCHEMA_VERSION, ScheduleTiming, ScheduleUnit,
        SpecIdentity, SystemSpec,
    };

    fn system(id: &str, name: &str) -> SystemSpec {
        SystemSpec {
            id: id.into(),
            name: name.into(),
            purpose: String::new(),
            interfaces: Vec::new(),
            design_notes: Vec::new(),
        }
    }

    fn mechanic(id: &str, system_id: &str, effects: Vec<EffectSpec>) -> MechanicSpec {
        MechanicSpec {
            id: id.into(),
            system_id: system_id.into(),
            rule_text: format!("{id} 规则"),
            preconditions: Vec::new(),
            effects,
            state_machine: None,
            design_notes: Vec::new(),
        }
    }

    fn spec_with(systems: Vec<SystemSpec>, mechanics: Vec<MechanicSpec>) -> GameSpec {
        GameSpec {
            identity: SpecIdentity {
                schema_version: SPEC_SCHEMA_VERSION.into(),
                project_id: "p1".into(),
                frozen_hash: "sha256:abc".into(),
            },
            intent: ProjectIntent::default(),
            systems,
            mechanics,
            entities: Vec::new(),
            tables: Vec::new(),
            content: Vec::new(),
            graphs: Vec::new(),
            acceptance: Vec::new(),
            source_map: Vec::new(),
        }
    }

    fn capabilities_for(spec: &GameSpec) -> CapabilitiesContract {
        CapabilitiesContract {
            capabilities: spec
                .mechanics
                .iter()
                .map(|mechanic| CapabilityContract {
                    id: format!("cap_{}", mechanic.id),
                    interface_name: "MechanicExecutionService".into(),
                    data_structures: Vec::new(),
                    source_refs: vec![SpecRef::new(format!("mechanics/{}", mechanic.id))],
                    scenarios: Vec::new(),
                })
                .collect(),
            coverage: MeasuredMetric::zero(),
            cardinality: CardinalityDeclaration {
                rule: "测试".into(),
                produced: spec.mechanics.len(),
                expected: CardinalityRange {
                    min: spec.mechanics.len(),
                    max: spec.mechanics.len(),
                },
                dropped: Vec::new(),
            },
            blockers: Vec::new(),
        }
    }

    fn empty_content() -> ContentInventoryContract {
        ContentInventoryContract {
            assets: Vec::new(),
            ui_entries: Vec::new(),
            non_visual_entities: Vec::new(),
            cardinality: CardinalityDeclaration {
                rule: "测试".into(),
                produced: 0,
                expected: CardinalityRange { min: 0, max: 0 },
                dropped: Vec::new(),
            },
        }
    }

    // ===== 装配任务按 domain 聚合去重（审计 B 整改 ③，二轮必改 5）=====

    /// 同 domain 的多个系统合并为一条装配任务：零同名重复，标题说明覆盖 N 个决策，
    /// source_refs 与依赖边全并入；不同 domain 各自成条。
    #[test]
    fn assembly_tasks_deduplicate_by_domain() {
        let spec = spec_with(
            vec![
                system("balance.pace#slow", "慢节奏平衡（主选）"),
                system("balance.pace#fast", "快节奏平衡"),
                system("balance.curve", "成长曲线平衡"),
                system("combat.core", "战斗核心"),
            ],
            vec![
                mechanic(
                    "m_pace",
                    "balance.pace#slow",
                    vec![EffectSpec::EmitSignal {
                        signal: "paced".into(),
                    }],
                ),
                mechanic(
                    "m_curve",
                    "balance.curve",
                    vec![EffectSpec::EmitSignal {
                        signal: "curved".into(),
                    }],
                ),
                mechanic(
                    "m_hit",
                    "combat.core",
                    vec![EffectSpec::EmitSignal {
                        signal: "hit".into(),
                    }],
                ),
            ],
        );
        let capabilities = capabilities_for(&spec);
        // 前三个系统同属 balance 领域（含同决策 `#` 双选项），第四个属 combat。
        let domains = vec![
            Some("balance".to_string()),
            Some("balance".to_string()),
            Some("balance".to_string()),
            Some("combat".to_string()),
        ];
        let tasks =
            build_tasks(&spec, &empty_content(), &capabilities, &domains).expect("聚合构造应成功");

        let assemblies: Vec<&TaskNode> = tasks
            .iter()
            .filter(|task| task.kind == "assembly")
            .collect();
        // 去重后：4 个系统 → 2 条装配任务（聚合前逐系统会是 4 条）。
        assert_eq!(assemblies.len(), 2, "{assemblies:?}");
        let titles: BTreeSet<&str> = assemblies.iter().map(|task| task.title.as_str()).collect();
        assert_eq!(titles.len(), assemblies.len(), "装配任务标题零同名重复");

        let balance = assemblies
            .iter()
            .find(|task| task.id == "task_assemble_domain_balance")
            .expect("balance 领域聚合任务");
        // `#` 前缀归并：pace 双选项算 1 个决策，加 curve 共 2 个。
        assert_eq!(balance.title, "装配 balance 领域系统（覆盖 2 个决策）");
        assert_eq!(
            balance
                .source_refs
                .iter()
                .map(|source_ref| source_ref.0.as_str())
                .collect::<Vec<_>>(),
            vec![
                "systems/balance.pace#slow",
                "systems/balance.pace#fast",
                "systems/balance.curve"
            ]
        );
        assert_eq!(
            balance.depends_on,
            vec![
                "task_cap_m_pace".to_string(),
                "task_cap_m_curve".to_string()
            ]
        );

        let combat = assemblies
            .iter()
            .find(|task| task.id == "task_assemble_combat.core")
            .expect("combat 单系统组");
        assert_eq!(combat.title, "装配系统 战斗核心");
    }

    /// 单系统组保持既有 id/标题形状（金样零漂移）；查不到 domain 的系统
    /// 各自成条、不猜归属、不与有 domain 的组合并。
    #[test]
    fn single_system_groups_keep_golden_shape() {
        let spec = spec_with(
            vec![
                system("ld.wave_system", "脚本化波次"),
                system("orphan.sys", "无归属系统"),
            ],
            Vec::new(),
        );
        let capabilities = capabilities_for(&spec);
        let domains = vec![Some("wave".to_string()), None];
        let tasks =
            build_tasks(&spec, &empty_content(), &capabilities, &domains).expect("构造应成功");
        let assemblies: Vec<&TaskNode> = tasks
            .iter()
            .filter(|task| task.kind == "assembly")
            .collect();
        assert_eq!(assemblies.len(), 2);
        assert_eq!(assemblies[0].id, "task_assemble_ld.wave_system");
        assert_eq!(assemblies[0].title, "装配系统 脚本化波次");
        assert_eq!(assemblies[1].id, "task_assemble_orphan.sys");
        assert_eq!(assemblies[1].title, "装配系统 无归属系统");
    }

    // ===== ModifyRule 跨机制依赖边 =====

    /// ModifyRule 产依赖边：target_rule 所属机制的程序任务 → 本机制的程序任务；
    /// 嵌套（Schedule 内层）的 ModifyRule 同样收集；自指不产边。
    #[test]
    fn modify_rule_produces_cross_mechanic_dependency_edges() {
        let spec = spec_with(
            vec![system("s1", "系统一")],
            vec![
                mechanic(
                    "base_damage",
                    "s1",
                    vec![EffectSpec::EmitSignal {
                        signal: "dmg".into(),
                    }],
                ),
                mechanic(
                    "rage_buff",
                    "s1",
                    vec![EffectSpec::Schedule {
                        timing: ScheduleTiming::OverTime,
                        amount_expr: "3".into(),
                        unit: ScheduleUnit::Turns,
                        inner: vec![EffectSpec::ModifyRule {
                            target_rule: "base_damage".into(),
                            patch: RulePatch::ScaleCoefficient {
                                expr: "x * 2".into(),
                            },
                            priority: 5,
                        }],
                    }],
                ),
                // 自指 ModifyRule：合法（C1 允许自指）但不构成开发顺序边。
                mechanic(
                    "self_tuner",
                    "s1",
                    vec![EffectSpec::ModifyRule {
                        target_rule: "self_tuner".into(),
                        patch: RulePatch::Disable,
                        priority: 0,
                    }],
                ),
            ],
        );
        let capabilities = capabilities_for(&spec);
        let domains = vec![Some("d1".to_string())];
        let tasks =
            build_tasks(&spec, &empty_content(), &capabilities, &domains).expect("构造应成功");

        let rage = tasks
            .iter()
            .find(|task| task.id == "task_cap_rage_buff")
            .expect("rage_buff 程序任务");
        assert_eq!(
            rage.depends_on,
            vec!["task_cap_base_damage".to_string()],
            "嵌套 ModifyRule 应产 target 所属机制 → 本机制 的边"
        );
        let base = tasks
            .iter()
            .find(|task| task.id == "task_cap_base_damage")
            .expect("base_damage 程序任务");
        assert!(base.depends_on.is_empty(), "被引用方不产反向边");
        let selfish = tasks
            .iter()
            .find(|task| task.id == "task_cap_self_tuner")
            .expect("self_tuner 程序任务");
        assert!(selfish.depends_on.is_empty(), "自指不产边");
    }

    /// 金样项目级断言（T-W7-1b 验收 4：装配任务零同名重复、依赖边非平凡）：
    /// 直接读 tests/golden/lane_defense/C6/contract.json——金样即真实项目产物。
    #[test]
    fn golden_lane_defense_assemblies_dedup_and_edges_nontrivial() {
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/lane_defense/C6/contract.json");
        let raw = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|error| panic!("金样 C6 契约应可读（{golden_path:?}）：{error}"));
        let contract: TaskGraphContract =
            serde_json::from_str(&raw).expect("金样 C6 契约应反序列化为 TaskGraphContract");

        // 装配任务零同名重复（去重实做的项目级证据）。
        let assembly_titles: Vec<&str> = contract
            .tasks
            .iter()
            .filter(|task| task.kind == "assembly")
            .map(|task| task.title.as_str())
            .collect();
        let unique_titles: BTreeSet<&str> = assembly_titles.iter().copied().collect();
        assert_eq!(
            assembly_titles.len(),
            unique_titles.len(),
            "金样装配任务出现同名重复：{assembly_titles:?}"
        );

        // 依赖边非平凡：边数 > 任务数的 10%（波 1 验收口径）。
        let edge_count: usize = contract
            .tasks
            .iter()
            .map(|task| task.depends_on.len())
            .sum();
        assert!(
            edge_count as f64 > contract.tasks.len() as f64 * 0.1,
            "依赖边 {} 条，不足任务数 {} 的 10%",
            edge_count,
            contract.tasks.len()
        );
    }

    /// 依赖边收集与 C4 同一深度纪律：9 层嵌套结构化 Err 点名机制 id。
    #[test]
    fn rule_target_collection_respects_depth_limit() {
        let mut effect = EffectSpec::ModifyRule {
            target_rule: "base".into(),
            patch: RulePatch::Disable,
            priority: 0,
        };
        for _ in 0..MAX_EFFECT_DEPTH {
            effect = EffectSpec::Schedule {
                timing: ScheduleTiming::Delayed,
                amount_expr: "1".into(),
                unit: ScheduleUnit::Seconds,
                inner: vec![effect],
            };
        }
        let mut targets = BTreeSet::new();
        let error =
            collect_rule_targets("m9", &effect, 1, &mut targets).expect_err("9 层嵌套应超限 Err");
        assert!(
            error.message.contains("m9")
                && error.message.contains(&format!("上限 {MAX_EFFECT_DEPTH}")),
            "{}",
            error.message
        );
    }
}
