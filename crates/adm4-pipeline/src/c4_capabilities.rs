use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_ai::AiRequest;
use adm4_contracts::{
    CardinalityDeclaration, CardinalityRange, EvidencePointer, MeasuredMetric, SpecRef,
};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::{
    AcceptanceScenario, AreaKind, EffectSpec, GameSpec, MechanicSpec, RulePatch, ScheduleTiming,
    ScheduleUnit,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 嵌套效果（AreaApply.inner / Schedule.inner / RollCheck.on_success/on_failure）
/// 的递归深度上限；超限即结构化 Err 点名机制 id（W7 波 1 T-W7-1b）。
/// C6 收集 ModifyRule 依赖边时沿用同一上限（同一份 spec 的同一条纪律）。
pub(crate) const MAX_EFFECT_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityContract {
    pub id: String,
    /// AI 命名（仅命名；结构由机制确定性投影）。
    pub interface_name: String,
    pub data_structures: Vec<String>,
    pub source_refs: Vec<SpecRef>,
    pub scenarios: Vec<AcceptanceScenario>,
}

/// C4 契约：机制投影派生的能力契约 + GWT + 双向核对。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesContract {
    pub capabilities: Vec<CapabilityContract>,
    /// 覆盖率 = 真实命中数 / 机制总数（R1：带逐机制证据，禁止恒 1.0 硬编码）。
    pub coverage: MeasuredMetric,
    pub cardinality: CardinalityDeclaration,
    pub blockers: Vec<String>,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;
    if spec.mechanics.is_empty() {
        return Err(Adm4Error::validation("GameSpec 无任何机制，C4 无从派生"));
    }
    // 悬空引用复检（W7 1c，沿 C1 的 ModifyRule 悬空先例）：Attach/Detach 的 target
    // 与 Detach 的 modifier_id 在投影前整体核对，悬空即结构化 Err 点名机制。
    check_effect_references(&spec)?;

    // 确定性投影：每条机制 → 能力契约 + GWT 场景（枚举投影，无 AI 发明）。
    let mut capabilities = Vec::new();
    for mechanic in &spec.mechanics {
        let interface_name = name_interface(ctx, mechanic)?;
        let scenario = project_scenario(mechanic)?;
        let data_structures = collect_data_structures(&spec, mechanic)?;
        capabilities.push(CapabilityContract {
            id: format!("cap_{}", mechanic.id),
            interface_name,
            data_structures,
            source_refs: vec![SpecRef::new(format!("mechanics/{}", mechanic.id))],
            scenarios: vec![scenario],
        });
    }

    // 双向核对（真核对，非橡皮图章）。
    let mut blockers = Vec::new();
    // 正向：每条 source_ref 真实存在。
    for capability in &capabilities {
        for source_ref in &capability.source_refs {
            if !spec.contains_ref(source_ref) {
                blockers.push(format!(
                    "能力 {} 的 source_ref {} 不存在于 GameSpec",
                    capability.id, source_ref.0
                ));
            }
        }
        if capability.scenarios.is_empty() {
            blockers.push(format!("能力 {} 没有可判定验收场景", capability.id));
        }
    }
    // 反向：每条机制被 ≥1 能力命中。
    let mut evidence = Vec::new();
    let mut covered = 0usize;
    for mechanic in &spec.mechanics {
        let path = format!("mechanics/{}", mechanic.id);
        let hit = capabilities.iter().find(|capability| {
            capability
                .source_refs
                .iter()
                .any(|source_ref| source_ref.0 == path)
        });
        match hit {
            Some(capability) => {
                covered += 1;
                evidence.push(EvidencePointer {
                    file: "C4/contract.json".into(),
                    path: path.clone(),
                    observed: format!("被能力 {} 覆盖", capability.id),
                });
            }
            None => blockers.push(format!(
                "CORE_MECHANIC_NOT_PLANNED: {path} 未被任何能力覆盖"
            )),
        }
    }
    if !blockers.is_empty() {
        return Err(Adm4Error::validation(format!(
            "C4 双向核对 {} 项 blocker：{}",
            blockers.len(),
            blockers.join("; ")
        )));
    }
    let coverage = MeasuredMetric::new(covered as f64 / spec.mechanics.len() as f64, evidence)?;

    let cardinality = CardinalityDeclaration {
        rule: "每条 L4 机制确定性投影为 1 个能力契约与 ≥1 个 GWT 场景".into(),
        produced: capabilities.len(),
        expected: CardinalityRange {
            min: spec.mechanics.len(),
            max: spec.mechanics.len(),
        },
        dropped: Vec::new(),
    };

    let contract = CapabilitiesContract {
        capabilities: capabilities.clone(),
        coverage,
        cardinality,
        blockers: Vec::new(),
    };
    let mut document = format!(
        "# C4 程序需求与架构\n\n- 能力契约：{} 个\n- 机制覆盖率：{:.0}%（逐机制证据见 contract.json）\n\n",
        capabilities.len(),
        contract.coverage.value() * 100.0
    );
    for capability in &capabilities {
        let scenario = &capability.scenarios[0];
        document.push_str(&format!(
            "## {}（`{}`）\n\n- 来源：{}\n- 数据结构：{}\n- 验收场景：\n  - Given {}\n  - When {}\n  - Then {}\n\n",
            capability.interface_name,
            capability.id,
            capability
                .source_refs
                .iter()
                .map(|source_ref| format!("`{}`", source_ref.0))
                .collect::<Vec<_>>()
                .join(" "),
            capability.data_structures.join(", "),
            scenario.given.join("；"),
            scenario.when.join("；"),
            scenario.then.join("；")
        ));
    }
    document.push_str("> 本文档由 contract.json 渲染，请勿手改。\n");
    ctx.store.write_stage("C4", &contract, &document)?;
    Ok(StageStatus::Succeeded)
}

/// 确定性 GWT 投影：preconditions → Given；rule_text → When；effects（封闭枚举）→ Then。
///
/// 16 变体全交付（T-W7-1b 交付 12 臂 + T-W7-1c 交付 AreaApply/Attach/Detach/
/// RollCheck 四臂）：旧 7 变体全语义投影 + 第 2 层 8 变体模式投影（每变体一个
/// 固定渲染函数，字段全来自作者填写）+ Custom 转录投影（只誊写设计者自己写的
/// GWT 三段）。嵌套效果（AreaApply.inner / Schedule.inner / RollCheck 两分支）
/// 递归渲染，深度上限 [`MAX_EFFECT_DEPTH`]，超限结构化 Err 点名机制 id。
fn project_scenario(mechanic: &MechanicSpec) -> Adm4Result<AcceptanceScenario> {
    let given = if mechanic.preconditions.is_empty() {
        vec![format!("系统 {} 处于就绪状态", mechanic.system_id)]
    } else {
        mechanic
            .preconditions
            .iter()
            .map(|condition| format!("{} {}", condition.subject, condition.predicate))
            .collect()
    };
    let then = mechanic
        .effects
        .iter()
        .map(|effect| render_effect(&mechanic.id, effect, 1))
        .collect::<Adm4Result<Vec<String>>>()?;
    Ok(AcceptanceScenario {
        id: format!("scenario_{}", mechanic.id),
        capability_id: format!("cap_{}", mechanic.id),
        given,
        when: vec![mechanic.rule_text.clone()],
        then,
        source_refs: vec![SpecRef::new(format!("mechanics/{}", mechanic.id))],
    })
}

/// 单个效果的确定性 Then 渲染（I1：纯函数投影，字段全部来自作者填写，无 AI、无发明）。
///
/// `depth` 从 1 起计；嵌套效果每下一层 +1，超过 [`MAX_EFFECT_DEPTH`] 即结构化 Err
/// 点名机制 id（防作者档递归炸栈，也防无界展开产出不可读需求）。
fn render_effect(mechanic_id: &str, effect: &EffectSpec, depth: usize) -> Adm4Result<String> {
    if depth > MAX_EFFECT_DEPTH {
        return Err(Adm4Error::validation(format!(
            "机制 {mechanic_id} 的效果嵌套深度超过上限 {MAX_EFFECT_DEPTH}（AreaApply/Schedule/RollCheck 的内层效果请拍平或拆分机制）"
        )));
    }
    match effect {
        EffectSpec::ModifyProperty {
            entity,
            property,
            formula,
        } => Ok(format!("实体 {entity} 的 {property} 按公式 {formula} 变化")),
        EffectSpec::SpawnEntity { entity } => Ok(format!("生成实体 {entity}")),
        EffectSpec::DespawnEntity { entity } => Ok(format!("移除实体 {entity}")),
        EffectSpec::ChangeState { machine, to_state } => {
            Ok(format!("状态机 {machine} 进入 {to_state}"))
        }
        EffectSpec::GrantResource { resource, formula } => {
            Ok(format!("资源 {resource} 按 {formula} 增加"))
        }
        EffectSpec::ConsumeResource { resource, formula } => {
            Ok(format!("资源 {resource} 按 {formula} 消耗"))
        }
        EffectSpec::EmitSignal { signal } => Ok(format!("发出信号 {signal}")),
        EffectSpec::Displace {
            subject,
            direction_expr,
            distance_expr,
            duration_expr,
        } => Ok(format!(
            "{subject} 沿 {direction_expr} 位移 {distance_expr}（时长 {duration_expr}）"
        )),
        EffectSpec::Schedule {
            timing,
            amount_expr,
            unit,
            inner,
        } => {
            let inner_rendered = render_nested(mechanic_id, inner, depth)?;
            let unit_text = schedule_unit_text(*unit);
            Ok(match timing {
                ScheduleTiming::Delayed => {
                    format!("延迟 {amount_expr} {unit_text} 后一次性执行：{inner_rendered}")
                }
                ScheduleTiming::OverTime => {
                    format!("在 {amount_expr} {unit_text} 内持续生效：{inner_rendered}")
                }
                ScheduleTiming::Periodic => {
                    format!("每 {amount_expr} {unit_text} 触发一次：{inner_rendered}")
                }
            })
        }
        EffectSpec::ModifyRule {
            target_rule,
            patch,
            priority,
        } => {
            let patch_text = match patch {
                RulePatch::ScaleCoefficient { expr } => {
                    format!("规则 {target_rule} 的系数按 {expr} 缩放")
                }
                RulePatch::ReplaceFormula { formula } => {
                    format!("规则 {target_rule} 的公式整体替换为 {formula}")
                }
                RulePatch::Disable => format!("规则 {target_rule} 被禁用"),
                RulePatch::Enable => format!("规则 {target_rule} 被启用"),
                RulePatch::AddPrecondition { condition } => {
                    format!("规则 {target_rule} 追加前置条件 {condition}")
                }
            };
            // 叠加序渲染进 GWT（W7 定稿 §5.3：同目标多修饰器按 priority 结算，
            // 同序冲突按机制 id 字典序确定性 tie-break）。
            Ok(format!(
                "{patch_text}（按 priority={priority} 结算，同序按机制 id 字典序）"
            ))
        }
        EffectSpec::DrawFromPool {
            pool_table,
            draw_count_expr,
            draw_rule,
            destination,
        } => Ok(format!(
            "从池表 {pool_table} 按规则 {draw_rule} 抽取 {draw_count_expr} 个到 {destination}"
        )),
        // 转录投影：只誊写设计者自己写的 GWT 三段（三段非空由 GameSpec 校验
        // 与 C1 复检拦截，这里照抄不加工）。
        EffectSpec::Custom {
            verb,
            given,
            when_,
            then,
            ..
        } => Ok(format!(
            "（Custom {verb} 转录）Given {given}；When {when_}；Then {then}"
        )),
        EffectSpec::AreaApply {
            area_kind,
            area_params,
            inner,
            target_filter,
        } => {
            let inner_rendered = render_nested(mechanic_id, inner, depth)?;
            let kind_text = area_kind_text(*area_kind);
            // 空间参数如实逐键渲染（BTreeMap 键序确定，I1）；空表如实写明。
            let params_text = if area_params.is_empty() {
                "（无空间参数）".to_string()
            } else {
                area_params
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join("，")
            };
            let filter_text = if target_filter.trim().is_empty() {
                "全部目标".to_string()
            } else {
                format!("满足 {target_filter} 的目标")
            };
            Ok(format!(
                "对 {kind_text} 范围（{params_text}）内{filter_text}逐个应用：{inner_rendered}"
            ))
        }
        EffectSpec::Attach {
            modifier_id,
            target,
            duration_expr,
            priority,
        } => {
            if modifier_id.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "机制 {mechanic_id} 的 Attach 效果缺少 modifier_id（挂载什么修饰器必须由作者声明，投影不发明）"
                )));
            }
            // 生效期字段有什么渲染什么：空 = 作者未声明，如实写明不发明默认值。
            let duration_text = if duration_expr.trim().is_empty() {
                "（生效期未声明）".to_string()
            } else {
                format!("生效期 {duration_expr}")
            };
            // 叠加序渲染进 GWT（W7 定稿 §5.3 指令 7：同目标多修饰器按 priority 结算，
            // 同序冲突按机制 id 字典序确定性 tie-break——与 ModifyRule 同款文案）。
            Ok(format!(
                "把修饰器 {modifier_id} 挂载到 {target}（{duration_text}；按 priority={priority} 结算，同序按机制 id 字典序）"
            ))
        }
        EffectSpec::Detach {
            modifier_id,
            target,
        } => {
            if modifier_id.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "机制 {mechanic_id} 的 Detach 效果缺少 modifier_id（卸下什么修饰器必须由作者声明，投影不发明）"
                )));
            }
            Ok(format!("从 {target} 卸下修饰器 {modifier_id}"))
        }
        EffectSpec::RollCheck {
            formula,
            difficulty_expr,
            on_success,
            on_failure,
        } => {
            if formula.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "机制 {mechanic_id} 的 RollCheck 效果缺少判定公式 formula（判什么必须由作者声明，投影不发明）"
                )));
            }
            // 两分支都可能为空列表，render_nested 如实渲染「（无内层效果）」。
            let success_rendered = render_nested(mechanic_id, on_success, depth)?;
            let failure_rendered = render_nested(mechanic_id, on_failure, depth)?;
            let difficulty_text = if difficulty_expr.trim().is_empty() {
                "（难度未声明）".to_string()
            } else {
                difficulty_expr.clone()
            };
            Ok(format!(
                "按 {formula} 对难度 {difficulty_text} 判定：成功→{success_rendered}；失败→{failure_rendered}"
            ))
        }
    }
}

fn area_kind_text(kind: AreaKind) -> &'static str {
    match kind {
        AreaKind::Circle => "圆形",
        AreaKind::Cone => "锥形",
        AreaKind::Line => "直线",
        AreaKind::Grid => "网格",
    }
}

/// 内层效果列表的递归渲染（子层深度 +1；空列表如实写明，不发明内容）。
fn render_nested(mechanic_id: &str, inner: &[EffectSpec], depth: usize) -> Adm4Result<String> {
    if inner.is_empty() {
        return Ok("（无内层效果）".into());
    }
    let rendered = inner
        .iter()
        .map(|nested| render_effect(mechanic_id, nested, depth + 1))
        .collect::<Adm4Result<Vec<String>>>()?;
    Ok(rendered.join("；"))
}

fn schedule_unit_text(unit: ScheduleUnit) -> &'static str {
    match unit {
        ScheduleUnit::Seconds => "秒",
        ScheduleUnit::Turns => "回合",
        ScheduleUnit::Ticks => "tick",
    }
}

/// 收集机制引用到的数据结构（实体 → `XxxData`，表 → `XxxTable`），
/// 覆盖全部 16 变体并递归嵌套效果（T-W7-1b 根治审计实锤的"数据结构为空"缺陷）。
///
/// 各变体的引用面（穷尽核对）：
/// - 旧 7：ModifyProperty/SpawnEntity/DespawnEntity 引用实体；ChangeState 引用
///   状态机、Grant/ConsumeResource 引用资源、EmitSignal 引用信号——均非实体/表，
///   无数据结构引用（核对确认无漏）。
/// - Displace 的 subject、Attach/Detach 的 target 引用实体；
/// - DrawFromPool 的 pool_table 引用表、destination 引用实体或表；
/// - AreaApply/Schedule 的 inner、RollCheck 的 on_success/on_failure 递归收集；
/// - ModifyRule 引用机制（跨机制依赖属 C6 的边，非数据结构）；
/// - Custom 的 operands 值逐个比对实体/表 id。
///
/// 未在 spec 中声明的名字（如 Attach target="self" 这类语义占位）不产结构引用。
/// 递归深度上限与渲染同款，超限结构化 Err 点名机制 id。
fn collect_data_structures(spec: &GameSpec, mechanic: &MechanicSpec) -> Adm4Result<Vec<String>> {
    let mut structures = Vec::new();
    for effect in &mechanic.effects {
        collect_effect_structures(spec, &mechanic.id, effect, 1, &mut structures)?;
    }
    Ok(structures)
}

fn collect_effect_structures(
    spec: &GameSpec,
    mechanic_id: &str,
    effect: &EffectSpec,
    depth: usize,
    structures: &mut Vec<String>,
) -> Adm4Result<()> {
    if depth > MAX_EFFECT_DEPTH {
        return Err(Adm4Error::validation(format!(
            "机制 {mechanic_id} 的效果嵌套深度超过上限 {MAX_EFFECT_DEPTH}（数据结构收集中止）"
        )));
    }
    let push_entity = |id: &str, structures: &mut Vec<String>| {
        if let Some(found) = spec.entities.iter().find(|candidate| candidate.id == id) {
            let name = format!("{}Data", camel(&found.name));
            if !structures.contains(&name) {
                structures.push(name);
            }
        }
    };
    let push_table = |id: &str, structures: &mut Vec<String>| {
        if spec.tables.iter().any(|candidate| candidate.id == id) {
            let name = format!("{}Table", camel(id));
            if !structures.contains(&name) {
                structures.push(name);
            }
        }
    };
    match effect {
        EffectSpec::ModifyProperty { entity, .. }
        | EffectSpec::SpawnEntity { entity }
        | EffectSpec::DespawnEntity { entity } => push_entity(entity, structures),
        EffectSpec::ChangeState { .. }
        | EffectSpec::GrantResource { .. }
        | EffectSpec::ConsumeResource { .. }
        | EffectSpec::EmitSignal { .. } => {}
        EffectSpec::Displace { subject, .. } => push_entity(subject, structures),
        EffectSpec::Attach { target, .. } | EffectSpec::Detach { target, .. } => {
            push_entity(target, structures)
        }
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            for nested in inner {
                collect_effect_structures(spec, mechanic_id, nested, depth + 1, structures)?;
            }
        }
        // ModifyRule 引用的是机制（规则），不是数据结构；跨机制依赖由 C6 建边。
        EffectSpec::ModifyRule { .. } => {}
        EffectSpec::DrawFromPool {
            pool_table,
            destination,
            ..
        } => {
            push_table(pool_table, structures);
            push_entity(destination, structures);
            push_table(destination, structures);
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => {
            for nested in on_success.iter().chain(on_failure.iter()) {
                collect_effect_structures(spec, mechanic_id, nested, depth + 1, structures)?;
            }
        }
        EffectSpec::Custom { operands, .. } => {
            for value in operands.values() {
                push_entity(value, structures);
                push_table(value, structures);
            }
        }
    }
    Ok(())
}

/// 四臂悬空引用复检（T-W7-1c，沿 ModifyRule 悬空先例 c1_modify_rule_dangling 的
/// 纪律；判据与 spec 级 effect_dangling_entity 同款：实体 id 或实体类前缀）：
/// - Attach/Detach 的 target 必须解析到 spec 内真实实体（或实体表行实体的类前缀）；
/// - Detach 的 modifier_id 必须被 spec 内某机制的 Attach 声明过（卸一个从未挂载的
///   修饰器 = 悬空引用，转录不发明）；
/// - 嵌套效果（AreaApply.inner / Schedule.inner / RollCheck 两分支）递归下探，
///   深度上限与渲染同款。
///
/// 为什么落 C4 而非 C1：C1 的扩展复检清单属波 1 交付面（本卡禁改），且该检查
/// 是渲染的前置条件（悬空 target 渲染出来的 GWT 指向不存在的对象 = 语义丢失），
/// 与投影同处一站保证「能渲染必可追溯」。
fn check_effect_references(spec: &GameSpec) -> Adm4Result<()> {
    let entity_ids: BTreeSet<&str> = spec.entities.iter().map(|item| item.id.as_str()).collect();
    let entity_known = |id: &str| -> bool {
        let class_prefix = format!("{id}.");
        entity_ids.contains(id)
            || entity_ids
                .iter()
                .any(|candidate| candidate.starts_with(&class_prefix))
    };
    // 先收全 spec 的已挂载修饰器名（含嵌套），再核对 Detach 引用。
    let mut attached_modifiers = BTreeSet::new();
    for mechanic in &spec.mechanics {
        for effect in &mechanic.effects {
            collect_attached_modifiers(&mechanic.id, effect, 1, &mut attached_modifiers)?;
        }
    }
    let mut violations = Vec::new();
    for mechanic in &spec.mechanics {
        for effect in &mechanic.effects {
            check_reference_recursively(
                &mechanic.id,
                effect,
                1,
                &entity_known,
                &attached_modifiers,
                &mut violations,
            )?;
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(Adm4Error::validation(format!(
            "C4 效果悬空引用 {} 项：{}",
            violations.len(),
            violations.join("; ")
        )))
    }
}

fn collect_attached_modifiers(
    mechanic_id: &str,
    effect: &EffectSpec,
    depth: usize,
    attached: &mut BTreeSet<String>,
) -> Adm4Result<()> {
    if depth > MAX_EFFECT_DEPTH {
        return Err(Adm4Error::validation(format!(
            "机制 {mechanic_id} 的效果嵌套深度超过上限 {MAX_EFFECT_DEPTH}（修饰器收集中止）"
        )));
    }
    match effect {
        EffectSpec::Attach { modifier_id, .. } => {
            attached.insert(modifier_id.clone());
        }
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            for nested in inner {
                collect_attached_modifiers(mechanic_id, nested, depth + 1, attached)?;
            }
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => {
            for nested in on_success.iter().chain(on_failure.iter()) {
                collect_attached_modifiers(mechanic_id, nested, depth + 1, attached)?;
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
        | EffectSpec::Detach { .. }
        | EffectSpec::ModifyRule { .. }
        | EffectSpec::DrawFromPool { .. }
        | EffectSpec::Custom { .. } => {}
    }
    Ok(())
}

fn check_reference_recursively(
    mechanic_id: &str,
    effect: &EffectSpec,
    depth: usize,
    entity_known: &dyn Fn(&str) -> bool,
    attached_modifiers: &BTreeSet<String>,
    violations: &mut Vec<String>,
) -> Adm4Result<()> {
    if depth > MAX_EFFECT_DEPTH {
        return Err(Adm4Error::validation(format!(
            "机制 {mechanic_id} 的效果嵌套深度超过上限 {MAX_EFFECT_DEPTH}（悬空引用复检中止）"
        )));
    }
    match effect {
        EffectSpec::Attach { target, .. } => {
            if !entity_known(target) {
                violations.push(format!(
                    "机制 {mechanic_id} 的 Attach 目标 {target} 不存在（须为 spec 内真实实体 id 或实体类前缀）"
                ));
            }
        }
        EffectSpec::Detach {
            modifier_id,
            target,
        } => {
            if !entity_known(target) {
                violations.push(format!(
                    "机制 {mechanic_id} 的 Detach 目标 {target} 不存在（须为 spec 内真实实体 id 或实体类前缀）"
                ));
            }
            if !modifier_id.trim().is_empty() && !attached_modifiers.contains(modifier_id) {
                violations.push(format!(
                    "机制 {mechanic_id} 的 Detach 修饰器 {modifier_id} 未被 spec 内任何机制挂载（卸载引用悬空）"
                ));
            }
        }
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            for nested in inner {
                check_reference_recursively(
                    mechanic_id,
                    nested,
                    depth + 1,
                    entity_known,
                    attached_modifiers,
                    violations,
                )?;
            }
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => {
            for nested in on_success.iter().chain(on_failure.iter()) {
                check_reference_recursively(
                    mechanic_id,
                    nested,
                    depth + 1,
                    entity_known,
                    attached_modifiers,
                    violations,
                )?;
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
        | EffectSpec::ModifyRule { .. }
        | EffectSpec::DrawFromPool { .. }
        | EffectSpec::Custom { .. } => {}
    }
    Ok(())
}

/// AI 只负责接口命名（锚定机制；不可用 = Err，R7）。
fn name_interface(ctx: &RunnerContext<'_>, mechanic: &MechanicSpec) -> Adm4Result<String> {
    let request = AiRequest {
        purpose: "c4_interface_naming".into(),
        system_prompt: "你是程序接口命名者。给机制起一个英文 PascalCase 接口名，\
                        输出 JSON：{\"interface_name\": ...}。"
            .into(),
        user_prompt: format!("机制 {}：{}", mechanic.id, mechanic.rule_text),
        expect_json: true,
    };
    let response = ctx.ai.invoke(&request)?;
    let value: serde_json::Value = serde_json::from_str(response.text.trim())
        .map_err(|error| Adm4Error::validation(format!("C4 命名产出不是合法 JSON：{error}")))?;
    let name = value
        .get("interface_name")
        .and_then(|name| name.as_str())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| Adm4Error::validation("C4 命名缺少 interface_name"))?;
    Ok(name.to_string())
}

fn camel(text: &str) -> String {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_foundation::Adm4ErrorKind;
    use std::collections::BTreeMap;

    fn mechanic_with(effects: Vec<EffectSpec>) -> MechanicSpec {
        MechanicSpec {
            id: "m1".into(),
            system_id: "s1".into(),
            rule_text: "规则".into(),
            preconditions: Vec::new(),
            effects,
            state_machine: None,
            design_notes: Vec::new(),
        }
    }

    /// 构造带实体/表声明的最小 spec（数据结构收集测试用）。
    fn spec_with_entities_and_tables() -> GameSpec {
        use adm4_spec::{
            EntitySpec, ProjectIntent, SPEC_SCHEMA_VERSION, SpecIdentity, TableSpec, VisualForm,
        };
        GameSpec {
            identity: SpecIdentity {
                schema_version: SPEC_SCHEMA_VERSION.into(),
                project_id: "p1".into(),
                frozen_hash: "sha256:abc".into(),
            },
            intent: ProjectIntent::default(),
            systems: Vec::new(),
            mechanics: Vec::new(),
            entities: vec![
                EntitySpec {
                    id: "enemy".into(),
                    name: "敌人".into(),
                    visual_form: Some(VisualForm::Sprite2d),
                    properties: Vec::new(),
                },
                EntitySpec {
                    id: "hand".into(),
                    name: "手牌区".into(),
                    visual_form: Some(VisualForm::UiOnly),
                    properties: Vec::new(),
                },
            ],
            tables: vec![TableSpec {
                id: "card_pool".into(),
                columns: Vec::new(),
                row_key: "id".into(),
                rows: Vec::new(),
                cells: Vec::new(),
                design_notes: Vec::new(),
            }],
            content: Vec::new(),
            graphs: Vec::new(),
            acceptance: Vec::new(),
            source_map: Vec::new(),
        }
    }

    /// 把机制装进最小 spec（悬空引用复检测试用：复检以整份 spec 为单位）。
    fn spec_with_mechanic(base: &GameSpec, mechanic: MechanicSpec) -> GameSpec {
        let mut spec = base.clone();
        spec.mechanics = vec![mechanic];
        spec
    }

    // ===== 4+1 臂真渲染正例（T-W7-1b 交付）=====

    /// Displace 真渲染：GWT 含作者填写的方向/距离/时长文案。
    #[test]
    fn displace_projection_renders_authored_fields() {
        let mechanic = mechanic_with(vec![EffectSpec::Displace {
            subject: "enemy".into(),
            direction_expr: "away_from(caster)".into(),
            distance_expr: "3".into(),
            duration_expr: "0.5".into(),
        }]);
        let scenario = project_scenario(&mechanic).expect("Displace 真渲染已交付");
        assert_eq!(
            scenario.then,
            vec!["enemy 沿 away_from(caster) 位移 3（时长 0.5）".to_string()]
        );
    }

    /// Schedule 真渲染：三种时序模式文案各不相同，内层效果递归渲染。
    #[test]
    fn schedule_projection_renders_timing_and_inner() {
        let inner = vec![EffectSpec::GrantResource {
            resource: "gold".into(),
            formula: "5".into(),
        }];
        let cases = [
            (ScheduleTiming::Delayed, "延迟 2 回合 后一次性执行"),
            (ScheduleTiming::OverTime, "在 2 回合 内持续生效"),
            (ScheduleTiming::Periodic, "每 2 回合 触发一次"),
        ];
        for (timing, expected_prefix) in cases {
            let mechanic = mechanic_with(vec![EffectSpec::Schedule {
                timing,
                amount_expr: "2".into(),
                unit: ScheduleUnit::Turns,
                inner: inner.clone(),
            }]);
            let scenario = project_scenario(&mechanic).expect("Schedule 真渲染已交付");
            assert_eq!(scenario.then.len(), 1);
            assert!(
                scenario.then[0].contains(expected_prefix)
                    && scenario.then[0].contains("资源 gold 按 5 增加"),
                "{}",
                scenario.then[0]
            );
        }
    }

    /// ModifyRule 真渲染：五种 patch 全渲染，且叠加序说明进 GWT
    /// （按 priority=N 结算，同序按机制 id 字典序）。
    #[test]
    fn modify_rule_projection_renders_patch_and_priority_order() {
        let cases: [(RulePatch, &str); 5] = [
            (
                RulePatch::ScaleCoefficient {
                    expr: "x * 2".into(),
                },
                "规则 damage_formula 的系数按 x * 2 缩放",
            ),
            (
                RulePatch::ReplaceFormula {
                    formula: "base + bonus".into(),
                },
                "规则 damage_formula 的公式整体替换为 base + bonus",
            ),
            (RulePatch::Disable, "规则 damage_formula 被禁用"),
            (RulePatch::Enable, "规则 damage_formula 被启用"),
            (
                RulePatch::AddPrecondition {
                    condition: "hp > 0".into(),
                },
                "规则 damage_formula 追加前置条件 hp > 0",
            ),
        ];
        for (patch, expected) in cases {
            let mechanic = mechanic_with(vec![EffectSpec::ModifyRule {
                target_rule: "damage_formula".into(),
                patch,
                priority: 7,
            }]);
            let scenario = project_scenario(&mechanic).expect("ModifyRule 真渲染已交付");
            assert_eq!(scenario.then.len(), 1);
            assert!(scenario.then[0].contains(expected), "{}", scenario.then[0]);
            assert!(
                scenario.then[0].contains("按 priority=7 结算，同序按机制 id 字典序"),
                "叠加序说明必须渲染进 GWT：{}",
                scenario.then[0]
            );
        }
    }

    /// DrawFromPool 真渲染：池表/规则/数量/目的地全部来自作者填写。
    #[test]
    fn draw_from_pool_projection_renders_authored_fields() {
        let mechanic = mechanic_with(vec![EffectSpec::DrawFromPool {
            pool_table: "card_pool".into(),
            draw_count_expr: "3".into(),
            draw_rule: "weighted_by_rarity".into(),
            destination: "hand".into(),
        }]);
        let scenario = project_scenario(&mechanic).expect("DrawFromPool 真渲染已交付");
        assert_eq!(
            scenario.then,
            vec!["从池表 card_pool 按规则 weighted_by_rarity 抽取 3 个到 hand".to_string()]
        );
    }

    /// Custom 转录投影：只誊写设计者自己写的 GWT 三段，一字不改不加工。
    #[test]
    fn custom_projection_transcribes_authored_gwt() {
        let mechanic = mechanic_with(vec![EffectSpec::Custom {
            verb: "merge".into(),
            operands: BTreeMap::new(),
            given: "两个同级单位相邻".into(),
            when_: "玩家拖拽其一到另一之上".into(),
            then: "合成一个高一级单位".into(),
        }]);
        let scenario = project_scenario(&mechanic).expect("Custom 转录投影已交付");
        assert_eq!(
            scenario.then,
            vec![
                "（Custom merge 转录）Given 两个同级单位相邻；When 玩家拖拽其一到另一之上；Then 合成一个高一级单位"
                    .to_string()
            ]
        );
    }

    // ===== 四臂真渲染（T-W7-1c 翻转：原"未交付必 Err"锁定测试按机动卡使命
    // 翻转为正路径断言；非法字段/悬空引用的负测试保留在后段）=====

    /// AreaApply 真渲染：形状/空间参数/过滤器全部来自作者填写，inner 递归展开；
    /// 空参数/空过滤器/空内层如实渲染，不发明内容。
    #[test]
    fn area_apply_projection_renders_shape_params_and_inner() {
        let mechanic = mechanic_with(vec![EffectSpec::AreaApply {
            area_kind: AreaKind::Cone,
            area_params: BTreeMap::from([
                ("angle".to_string(), "60".to_string()),
                ("radius".to_string(), "5".to_string()),
            ]),
            inner: vec![EffectSpec::ModifyProperty {
                entity: "enemy".into(),
                property: "hp".into(),
                formula: "hp - burst".into(),
            }],
            target_filter: "faction != caster.faction".into(),
        }]);
        let scenario = project_scenario(&mechanic).expect("AreaApply 真渲染已交付");
        assert_eq!(
            scenario.then,
            vec![
                "对 锥形 范围（angle=60，radius=5）内满足 faction != caster.faction 的目标逐个应用：实体 enemy 的 hp 按公式 hp - burst 变化"
                    .to_string()
            ]
        );

        // 裸 AreaApply（缺参数/过滤器/内层）如实渲染空语义，不 Err 不发明。
        let bare = mechanic_with(vec![EffectSpec::AreaApply {
            area_kind: Default::default(),
            area_params: BTreeMap::new(),
            inner: Vec::new(),
            target_filter: String::new(),
        }]);
        let scenario = project_scenario(&bare).expect("裸 AreaApply 应如实渲染");
        assert_eq!(
            scenario.then,
            vec!["对 圆形 范围（（无空间参数））内全部目标逐个应用：（无内层效果）".to_string()]
        );
    }

    /// Attach 真渲染：修饰器/目标/生效期全来自作者填写，叠加序文字与 ModifyRule
    /// 同款（W7 定稿 §5.3 指令 7）。
    #[test]
    fn attach_projection_renders_modifier_duration_and_priority_order() {
        let mechanic = mechanic_with(vec![EffectSpec::Attach {
            modifier_id: "tag_synergy_bonus".into(),
            target: "squad".into(),
            duration_expr: "while(tag_count >= tag_threshold)".into(),
            priority: 10,
        }]);
        let scenario = project_scenario(&mechanic).expect("Attach 真渲染已交付");
        assert_eq!(
            scenario.then,
            vec![
                "把修饰器 tag_synergy_bonus 挂载到 squad（生效期 while(tag_count >= tag_threshold)；按 priority=10 结算，同序按机制 id 字典序）"
                    .to_string()
            ]
        );

        // 生效期未声明时如实写明，不发明默认值。
        let no_duration = mechanic_with(vec![EffectSpec::Attach {
            modifier_id: "buff".into(),
            target: "enemy".into(),
            duration_expr: String::new(),
            priority: 0,
        }]);
        let scenario = project_scenario(&no_duration).expect("缺生效期仍应渲染");
        assert!(
            scenario.then[0].contains("（生效期未声明）"),
            "{}",
            scenario.then[0]
        );
    }

    /// Detach 真渲染：目标与修饰器全来自作者填写。
    #[test]
    fn detach_projection_renders_target_and_modifier() {
        let mechanic = mechanic_with(vec![EffectSpec::Detach {
            modifier_id: "gem_effect".into(),
            target: "equipment_entity".into(),
        }]);
        let scenario = project_scenario(&mechanic).expect("Detach 真渲染已交付");
        assert_eq!(
            scenario.then,
            vec!["从 equipment_entity 卸下修饰器 gem_effect".to_string()]
        );
    }

    /// RollCheck 真渲染：公式/难度全来自作者填写，成功/失败两分支递归展开；
    /// 空分支如实渲染「（无内层效果）」。
    #[test]
    fn roll_check_projection_renders_formula_and_both_branches() {
        let mechanic = mechanic_with(vec![EffectSpec::RollCheck {
            formula: "slot_open(target_slot)".into(),
            difficulty_expr: "0".into(),
            on_success: vec![EffectSpec::SpawnEntity {
                entity: "placed_structure".into(),
            }],
            on_failure: vec![EffectSpec::EmitSignal {
                signal: "placement_rejected".into(),
            }],
        }]);
        let scenario = project_scenario(&mechanic).expect("RollCheck 真渲染已交付");
        assert_eq!(
            scenario.then,
            vec![
                "按 slot_open(target_slot) 对难度 0 判定：成功→生成实体 placed_structure；失败→发出信号 placement_rejected"
                    .to_string()
            ]
        );

        // 两分支均空：如实渲染空语义。
        let empty_branches = mechanic_with(vec![EffectSpec::RollCheck {
            formula: "d20 + perception".into(),
            difficulty_expr: "12".into(),
            on_success: Vec::new(),
            on_failure: Vec::new(),
        }]);
        let scenario = project_scenario(&empty_branches).expect("空分支应如实渲染");
        assert_eq!(
            scenario.then,
            vec![
                "按 d20 + perception 对难度 12 判定：成功→（无内层效果）；失败→（无内层效果）"
                    .to_string()
            ]
        );
    }

    /// 混合机制整体投影：旧变体与新臂同机制共存，逐效果各产一条 Then。
    #[test]
    fn mixed_effects_project_each_effect() {
        let mechanic = mechanic_with(vec![
            EffectSpec::SpawnEntity {
                entity: "guard".into(),
            },
            EffectSpec::RollCheck {
                formula: "d20".into(),
                difficulty_expr: "12".into(),
                on_success: Vec::new(),
                on_failure: Vec::new(),
            },
        ]);
        let scenario = project_scenario(&mechanic).expect("混合机制应整体投影");
        assert_eq!(scenario.then.len(), 2);
        assert!(scenario.then[0].contains("guard"));
        assert!(scenario.then[1].contains("d20"), "{}", scenario.then[1]);
    }

    // ===== 四臂负测试：非法字段（缺必填语义字段即结构化 Err，点名机制）=====

    /// Attach/Detach 缺 modifier_id、RollCheck 缺 formula：投影拒绝，不糊空文案。
    #[test]
    fn arms_with_missing_semantic_fields_are_rejected() {
        let cases: [(EffectSpec, &str); 3] = [
            (
                EffectSpec::Attach {
                    modifier_id: "  ".into(),
                    target: "enemy".into(),
                    duration_expr: "3".into(),
                    priority: 1,
                },
                "Attach",
            ),
            (
                EffectSpec::Detach {
                    modifier_id: String::new(),
                    target: "enemy".into(),
                },
                "Detach",
            ),
            (
                EffectSpec::RollCheck {
                    formula: String::new(),
                    difficulty_expr: "12".into(),
                    on_success: Vec::new(),
                    on_failure: Vec::new(),
                },
                "RollCheck",
            ),
        ];
        for (effect, variant) in cases {
            let mechanic = mechanic_with(vec![effect]);
            let error =
                project_scenario(&mechanic).expect_err(&format!("{variant} 缺必填字段应 Err"));
            assert_eq!(error.kind, Adm4ErrorKind::Validation);
            assert!(
                error.message.contains("m1") && error.message.contains(variant),
                "{}",
                error.message
            );
        }
    }

    // ===== 四臂负测试：悬空引用（沿 ModifyRule 悬空先例）=====

    /// Attach/Detach 的 target 悬空（spec 无该实体也无该类前缀）→ 结构化 Err 点名；
    /// 指向真实实体或实体类前缀则放行。
    #[test]
    fn dangling_attach_detach_target_is_rejected() {
        let spec = spec_with_entities_and_tables();

        let dangling_attach = spec_with_mechanic(
            &spec,
            mechanic_with(vec![EffectSpec::Attach {
                modifier_id: "buff".into(),
                target: "ghost_target".into(),
                duration_expr: "3".into(),
                priority: 0,
            }]),
        );
        let error = check_effect_references(&dangling_attach).expect_err("悬空 Attach 目标应 Err");
        assert_eq!(error.kind, Adm4ErrorKind::Validation);
        assert!(
            error.message.contains("m1")
                && error.message.contains("Attach")
                && error.message.contains("ghost_target"),
            "{}",
            error.message
        );

        let dangling_detach = spec_with_mechanic(
            &spec,
            mechanic_with(vec![
                EffectSpec::Attach {
                    modifier_id: "buff".into(),
                    target: "enemy".into(),
                    duration_expr: "3".into(),
                    priority: 0,
                },
                EffectSpec::Detach {
                    modifier_id: "buff".into(),
                    target: "ghost_target".into(),
                },
            ]),
        );
        let error = check_effect_references(&dangling_detach).expect_err("悬空 Detach 目标应 Err");
        assert!(
            error.message.contains("Detach") && error.message.contains("ghost_target"),
            "{}",
            error.message
        );

        // 正例：实体 id 与实体类前缀（enemy.xxx 行实体）都放行；嵌套内同查。
        let ok = spec_with_mechanic(
            &spec,
            mechanic_with(vec![EffectSpec::Schedule {
                timing: ScheduleTiming::Delayed,
                amount_expr: "1".into(),
                unit: ScheduleUnit::Seconds,
                inner: vec![
                    EffectSpec::Attach {
                        modifier_id: "buff".into(),
                        target: "enemy".into(),
                        duration_expr: "3".into(),
                        priority: 0,
                    },
                    EffectSpec::Detach {
                        modifier_id: "buff".into(),
                        target: "enemy".into(),
                    },
                ],
            }]),
        );
        assert!(check_effect_references(&ok).is_ok());
    }

    /// Detach 引用从未被任何 Attach 挂载过的修饰器 → 悬空 Err；跨机制挂载可解。
    #[test]
    fn detach_of_never_attached_modifier_is_rejected() {
        let spec = spec_with_entities_and_tables();
        let dangling = spec_with_mechanic(
            &spec,
            mechanic_with(vec![EffectSpec::Detach {
                modifier_id: "phantom_modifier".into(),
                target: "enemy".into(),
            }]),
        );
        let error = check_effect_references(&dangling).expect_err("卸载未挂载的修饰器应 Err");
        assert_eq!(error.kind, Adm4ErrorKind::Validation);
        assert!(
            error.message.contains("phantom_modifier"),
            "{}",
            error.message
        );

        // 另一机制挂载了同名修饰器 → 引用闭合放行（嵌套内的 Attach 声明也算）。
        let mut resolved = spec_with_mechanic(
            &spec,
            mechanic_with(vec![EffectSpec::Detach {
                modifier_id: "phantom_modifier".into(),
                target: "enemy".into(),
            }]),
        );
        let mut attacher = mechanic_with(vec![EffectSpec::Schedule {
            timing: ScheduleTiming::Delayed,
            amount_expr: "1".into(),
            unit: ScheduleUnit::Seconds,
            inner: vec![EffectSpec::Attach {
                modifier_id: "phantom_modifier".into(),
                target: "enemy".into(),
                duration_expr: "3".into(),
                priority: 0,
            }],
        }]);
        attacher.id = "m2".into();
        resolved.mechanics.push(attacher);
        assert!(check_effect_references(&resolved).is_ok());
    }

    /// AreaApply 内层的悬空引用同样被递归复检拦下（嵌套不豁免）。
    #[test]
    fn nested_dangling_reference_inside_area_apply_is_rejected() {
        let spec = spec_with_entities_and_tables();
        let nested_dangling = spec_with_mechanic(
            &spec,
            mechanic_with(vec![EffectSpec::AreaApply {
                area_kind: AreaKind::Grid,
                area_params: BTreeMap::new(),
                inner: vec![EffectSpec::Attach {
                    modifier_id: "buff".into(),
                    target: "ghost_target".into(),
                    duration_expr: "3".into(),
                    priority: 0,
                }],
                target_filter: String::new(),
            }]),
        );
        let error = check_effect_references(&nested_dangling).expect_err("嵌套内悬空引用应被拦下");
        assert!(error.message.contains("ghost_target"), "{}", error.message);
    }

    /// 旧 7 变体投影不受本卡影响（基线守恒）。
    #[test]
    fn legacy_seven_variants_still_project() {
        let mechanic = mechanic_with(vec![
            EffectSpec::ModifyProperty {
                entity: "enemy".into(),
                property: "hp".into(),
                formula: "hp - damage".into(),
            },
            EffectSpec::EmitSignal {
                signal: "hit".into(),
            },
        ]);
        let scenario = project_scenario(&mechanic).expect("旧变体应正常投影");
        assert_eq!(scenario.then.len(), 2);
        assert!(scenario.then[0].contains("hp"));
        assert!(scenario.then[1].contains("hit"));
    }

    // ===== 递归化：嵌套渲染与深度上限 =====

    /// 三层嵌套正例：Schedule 套 Schedule 套 Displace，逐层文案都在 Then 里。
    #[test]
    fn three_level_nested_projection_renders_all_layers() {
        let mechanic = mechanic_with(vec![EffectSpec::Schedule {
            timing: ScheduleTiming::Periodic,
            amount_expr: "1".into(),
            unit: ScheduleUnit::Seconds,
            inner: vec![EffectSpec::Schedule {
                timing: ScheduleTiming::Delayed,
                amount_expr: "2".into(),
                unit: ScheduleUnit::Ticks,
                inner: vec![EffectSpec::Displace {
                    subject: "enemy".into(),
                    direction_expr: "back".into(),
                    distance_expr: "1".into(),
                    duration_expr: "0.2".into(),
                }],
            }],
        }]);
        let scenario = project_scenario(&mechanic).expect("三层嵌套应正常投影");
        assert_eq!(scenario.then.len(), 1);
        let rendered = &scenario.then[0];
        assert!(rendered.contains("每 1 秒 触发一次"), "{rendered}");
        assert!(rendered.contains("延迟 2 tick 后一次性执行"), "{rendered}");
        assert!(
            rendered.contains("enemy 沿 back 位移 1（时长 0.2）"),
            "{rendered}"
        );
    }

    /// 深度超限（9 层 > 上限 8）：结构化 Err 且点名机制 id。
    #[test]
    fn nesting_beyond_depth_limit_is_structured_err() {
        let mut effect = EffectSpec::Displace {
            subject: "enemy".into(),
            direction_expr: "back".into(),
            distance_expr: "1".into(),
            duration_expr: "0.1".into(),
        };
        for _ in 0..MAX_EFFECT_DEPTH {
            effect = EffectSpec::Schedule {
                timing: ScheduleTiming::Delayed,
                amount_expr: "1".into(),
                unit: ScheduleUnit::Seconds,
                inner: vec![effect],
            };
        }
        let mechanic = mechanic_with(vec![effect.clone()]);
        let error = project_scenario(&mechanic).expect_err("9 层嵌套应超限 Err");
        assert_eq!(error.kind, Adm4ErrorKind::Validation);
        assert!(
            error.message.contains("m1") && error.message.contains("深度超过上限 8"),
            "{}",
            error.message
        );
        // 收集函数同一深度纪律。
        let spec = spec_with_entities_and_tables();
        let collect_error =
            collect_data_structures(&spec, &mechanic).expect_err("收集函数对 9 层嵌套同样超限 Err");
        assert!(
            collect_error.message.contains("m1"),
            "{}",
            collect_error.message
        );

        // 恰好 8 层（上限内）正常通过。
        let mut ok_effect = EffectSpec::Displace {
            subject: "enemy".into(),
            direction_expr: "back".into(),
            distance_expr: "1".into(),
            duration_expr: "0.1".into(),
        };
        for _ in 0..(MAX_EFFECT_DEPTH - 1) {
            ok_effect = EffectSpec::Schedule {
                timing: ScheduleTiming::Delayed,
                amount_expr: "1".into(),
                unit: ScheduleUnit::Seconds,
                inner: vec![ok_effect],
            };
        }
        let ok_mechanic = mechanic_with(vec![ok_effect]);
        assert!(project_scenario(&ok_mechanic).is_ok(), "8 层应在上限内");
    }

    // ===== collect_data_structures：全变体引用面 =====

    /// DrawFromPool 收集 pool_table（表）与 destination（实体）；
    /// Displace 收集 subject（实体）。
    #[test]
    fn collect_covers_draw_from_pool_and_displace() {
        let spec = spec_with_entities_and_tables();
        let mechanic = mechanic_with(vec![
            EffectSpec::DrawFromPool {
                pool_table: "card_pool".into(),
                draw_count_expr: "3".into(),
                draw_rule: "uniform".into(),
                destination: "hand".into(),
            },
            EffectSpec::Displace {
                subject: "enemy".into(),
                direction_expr: "away".into(),
                distance_expr: "2".into(),
                duration_expr: "0.3".into(),
            },
        ]);
        let structures = collect_data_structures(&spec, &mechanic).expect("收集应成功");
        assert_eq!(
            structures,
            vec![
                "CardPoolTable".to_string(),
                "手牌区Data".to_string(),
                "敌人Data".to_string()
            ]
        );
    }

    /// 嵌套内层的引用被递归收集（Schedule 内层 ModifyProperty 引用实体）；
    /// Attach/Detach 的 target、Custom 的 operands 值也进收集面；
    /// spec 未声明的名字（如 target="self"）不产引用；重复引用去重。
    #[test]
    fn collect_recurses_and_covers_new_variants() {
        let spec = spec_with_entities_and_tables();
        let mechanic = mechanic_with(vec![
            EffectSpec::Schedule {
                timing: ScheduleTiming::Periodic,
                amount_expr: "2".into(),
                unit: ScheduleUnit::Seconds,
                inner: vec![EffectSpec::ModifyProperty {
                    entity: "enemy".into(),
                    property: "hp".into(),
                    formula: "hp - burn".into(),
                }],
            },
            EffectSpec::Attach {
                modifier_id: "slow".into(),
                target: "enemy".into(),
                duration_expr: "3".into(),
                priority: 1,
            },
            EffectSpec::Detach {
                modifier_id: "shield".into(),
                target: "self".into(),
            },
            EffectSpec::Custom {
                verb: "shuffle".into(),
                operands: BTreeMap::from([("pool".to_string(), "card_pool".to_string())]),
                given: "g".into(),
                when_: "w".into(),
                then: "t".into(),
            },
        ]);
        let structures = collect_data_structures(&spec, &mechanic).expect("收集应成功");
        // enemy 出现两次（嵌套内层 + Attach target）但去重为一条；
        // "self" 未在 spec 声明，不产引用；Custom operands 命中 card_pool 表。
        assert_eq!(
            structures,
            vec!["敌人Data".to_string(), "CardPoolTable".to_string()]
        );
    }

    /// 旧 7 变体引用面核对：实体三兄弟收集、资源/信号/状态机不产数据结构引用。
    #[test]
    fn collect_legacy_variants_reference_surface() {
        let spec = spec_with_entities_and_tables();
        let mechanic = mechanic_with(vec![
            EffectSpec::ModifyProperty {
                entity: "enemy".into(),
                property: "hp".into(),
                formula: "hp - 1".into(),
            },
            EffectSpec::SpawnEntity {
                entity: "hand".into(),
            },
            EffectSpec::DespawnEntity {
                entity: "enemy".into(),
            },
            EffectSpec::ChangeState {
                machine: "door".into(),
                to_state: "open".into(),
            },
            EffectSpec::GrantResource {
                resource: "gold".into(),
                formula: "5".into(),
            },
            EffectSpec::ConsumeResource {
                resource: "mana".into(),
                formula: "2".into(),
            },
            EffectSpec::EmitSignal {
                signal: "hit".into(),
            },
        ]);
        let structures = collect_data_structures(&spec, &mechanic).expect("收集应成功");
        assert_eq!(
            structures,
            vec!["敌人Data".to_string(), "手牌区Data".to_string()]
        );
    }
}
