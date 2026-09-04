use adm4_authoring::FrozenDesign;
use adm4_contracts::{SpecRef, TypedValue, UnclassifiedItem, ValueKind};
use adm4_decision::{
    DesignLevel, GraphEntryConstraint, ParameterSchema, ParameterValues, SelectedOptionRef,
    Selection, validate_graph_value,
};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_space::DesignSpace;
use adm4_spec::{
    ContentSpec, CurveInterpolation, CurveSpec, DesignNote, DesignNoteRole, EffectSpec, EntitySpec,
    GameSpec, GraphEntry, GraphSpec, MechanicSpec, ProjectIntent, PropertySpec,
    SPEC_SCHEMA_VERSION, SpecIdentity, SpecSourceEntry, SystemSpec, TableSpec, VisualForm,
    validate_game_spec,
};
use std::collections::BTreeMap;

/// Statement 类自由文本的参数键约定（如 `u.experience` 的体验陈述字段）：
/// 选择参数里该键的非空文本作为 `DesignNoteRole::Statement` 注记进入 spec 元素。
const STATEMENT_PARAM_KEY: &str = "statement";

/// Curve 参数的标量键约定：`data_form=curve` 的选项在该键放 `CurveSpec` 的 JSON 文本。
/// schema 侧已有 `ParameterSchema::Curve` 变体（W7 3a），值形态沿波 1 先例以标量文本承载，
/// 编译成两列 TableSpec + 插值注记（W7 定稿 §5.4，复用既有表通路）。
const CURVE_PARAM_KEY: &str = "curve";

/// Graph 参数的标量键约定：`data_form=graph` 的选项在该键放 GraphSpec 形态的 JSON 文本
/// （nodes/edges；directed/acyclic/entry 以 schema 为真相覆盖，见 `compile_graph`）。
const GRAPH_PARAM_KEY: &str = "graph";

/// 数据形态标签键：`data_table` 角色的选项可声明 `data_form=curve|graph`。
const DATA_FORM_TAG: &str = "data_form";

/// C0：FrozenDesign → GameSpec 的确定性编译（无 AI）。
///
/// 分层映射：L0-L1→intent；L2→genre_structure；L3→SystemSpec；L4→MechanicSpec；
/// L5→EntitySpec/TableSpec；L6→行数据/ContentSpec。
/// 无法映射的选择 → R2 阻塞，绝不跳过；每个 spec 元素登记 source_map。
///
/// design_notes（W7 定稿 §5.5）：每个选择的非空 rationale 与 `statement` 参数键的
/// 自由文本作为注记装入对应 spec 元素（SystemSpec/MechanicSpec/TableSpec/ContentSpec）。
/// **纪律：注记只被携带与展示，永不被编译成结构**（I1）。
pub fn compile_frozen_design(frozen: &FrozenDesign, space: &DesignSpace) -> Adm4Result<GameSpec> {
    let mut unknown: Vec<UnclassifiedItem> = Vec::new();
    let mut intent = ProjectIntent {
        title: frozen.project_name.clone(),
        ..Default::default()
    };
    let mut systems: Vec<SystemSpec> = Vec::new();
    let mut mechanics: Vec<MechanicSpec> = Vec::new();
    let mut entities: Vec<EntitySpec> = Vec::new();
    let mut tables: Vec<TableSpec> = Vec::new();
    let mut content: Vec<ContentSpec> = Vec::new();
    // W7 3a：`data_form=graph` 的选项由 compile_graph 编译进本集合（GameSpec.graphs）。
    let mut graphs: Vec<GraphSpec> = Vec::new();
    let mut source_map: Vec<SpecSourceEntry> = Vec::new();
    let mut promises: Vec<String> = Vec::new();
    let mut genre_parts: Vec<String> = Vec::new();
    // 同一决策点可能贡献多条画像文本（多选点），按决策 id 聚合后合并成一个键。
    let mut profile_parts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // 「L3 决策点 → 它实际产出的 SystemSpec id」预扫描：多选的 L3 会产出多个系统，
    // 机制归属不能再假设「系统 id == 决策 id」（主选在前，供缺省归属取用）。
    let system_ids_by_decision = scan_system_ids(frozen, space);

    for selection in &frozen.decisions {
        let Some(point) = space.graph.point(&selection.decision_id) else {
            unknown.push(UnclassifiedItem {
                item: selection.decision_id.clone(),
                reason: "清单中不存在该决策点".into(),
            });
            continue;
        };
        // 多选点的每个已选选项都编译（主选在前）：漏一个就等于把用户的设计吞了。
        let selected = selection.selected_options();
        let multi = selected.len() > 1;
        for item in selected {
            let Some(option) = point.option(item.option_id) else {
                unknown.push(UnclassifiedItem {
                    item: format!("{}/{}", selection.decision_id, item.option_id),
                    reason: "选项不存在".into(),
                });
                continue;
            };
            let role = option
                .compiler_tags
                .get("spec_role")
                .cloned()
                .unwrap_or_else(|| default_role(point.level).to_string());
            // spec 元素 id：单选点沿用决策 id（既有产物锚点不变），
            // 多选点按 `<决策id>#<选项id>` 拆开，避免同名元素互相覆盖。
            let element_id = if multi {
                format!("{}#{}", selection.decision_id, item.option_id)
            } else {
                selection.decision_id.clone()
            };
            let label = if item.is_primary {
                format!("{}（主选）", option.label)
            } else {
                option.label.clone()
            };
            // design_notes 五挂点收集（W7 定稿 §5.5）：非空 rationale + statement 自由文本，
            // 随 spec 元素落盘。注记只携带与展示，永不编译成结构（保 I1）。
            let notes = collect_design_notes(
                &selection.decision_id,
                item.option_id,
                item.rationale,
                item.parameters,
            );

            match role.as_str() {
                "title" => {
                    if let Some(title) = scalar_text(item.parameters, "title") {
                        intent.title = title;
                    }
                    profile_parts
                        .entry(selection.decision_id.clone())
                        .or_default()
                        .push(label);
                    source_map.push(entry("intent", &selection.decision_id));
                }
                "profile" => {
                    profile_parts
                        .entry(selection.decision_id.clone())
                        .or_default()
                        .push(format!("{label}{}", render_scalar_suffix(item.parameters)));
                    source_map.push(entry("intent", &selection.decision_id));
                }
                "promise" => {
                    promises.push(format!("{label}：{}", item.rationale));
                    source_map.push(entry("intent", &selection.decision_id));
                }
                "genre" => {
                    genre_parts.push(label);
                    source_map.push(entry("intent", &selection.decision_id));
                }
                "system" => {
                    systems.push(SystemSpec {
                        id: element_id.clone(),
                        name: label,
                        purpose: option.summary.clone(),
                        interfaces: option.implications.clone(),
                        design_notes: notes,
                    });
                    source_map.push(entry(
                        &format!("systems/{element_id}"),
                        &selection.decision_id,
                    ));
                }
                "mechanic" => {
                    match compile_mechanic(
                        space,
                        selection,
                        point,
                        option,
                        &item,
                        &element_id,
                        &system_ids_by_decision,
                    ) {
                        Ok(mut mechanic) => {
                            mechanic.design_notes = notes;
                            source_map.push(entry(
                                &format!("mechanics/{}", mechanic.id),
                                &selection.decision_id,
                            ));
                            mechanics.push(mechanic);
                        }
                        Err(item) => unknown.push(item),
                    }
                }
                "entity_table" => {
                    match compile_entity_table(option, item.parameters, &element_id) {
                        Ok((mut new_entities, mut table)) => {
                            table.design_notes = notes;
                            source_map.push(entry(
                                &format!("tables/{}", table.id),
                                &selection.decision_id,
                            ));
                            for entity in &new_entities {
                                source_map.push(entry(
                                    &format!("entities/{}", entity.id),
                                    &selection.decision_id,
                                ));
                            }
                            entities.append(&mut new_entities);
                            tables.push(table);
                        }
                        Err(item) => unknown.push(item),
                    }
                }
                // Graph 产 GraphSpec 进 GameSpec.graphs（不走表通路），先于 data_table 分流。
                "data_table"
                    if option.compiler_tags.get(DATA_FORM_TAG).map(String::as_str)
                        == Some("graph") =>
                {
                    match compile_graph(option, item.parameters, &element_id) {
                        Ok(mut graph) => {
                            graph.design_notes.extend(notes);
                            source_map.push(entry(
                                &format!("graphs/{}", graph.id),
                                &selection.decision_id,
                            ));
                            graphs.push(graph);
                        }
                        Err(item) => unknown.push(item),
                    }
                }
                "data_table" => match compile_data_table(option, item.parameters, &element_id) {
                    Ok(mut table) => {
                        table.design_notes.extend(notes);
                        source_map.push(entry(
                            &format!("tables/{}", table.id),
                            &selection.decision_id,
                        ));
                        tables.push(table);
                    }
                    Err(item) => unknown.push(item),
                },
                "content" => {
                    let data = serde_json::to_value(item.parameters).map_err(|error| {
                        Adm4Error::internal(format!("content serialize failed: {error}"))
                    })?;
                    content.push(ContentSpec {
                        id: element_id.clone(),
                        content_kind: option
                            .compiler_tags
                            .get("content_kind")
                            .cloned()
                            .unwrap_or_else(|| "generic".into()),
                        data,
                        design_notes: notes,
                    });
                    source_map.push(entry(
                        &format!("content/{element_id}"),
                        &selection.decision_id,
                    ));
                }
                other => {
                    unknown.push(UnclassifiedItem {
                        item: format!("{}/{}", selection.decision_id, item.option_id),
                        reason: format!("未知 spec_role: {other}"),
                    });
                }
            }
        }
    }
    for (decision_id, parts) in profile_parts {
        intent.profile.insert(decision_id, parts.join("、"));
    }

    if !unknown.is_empty() {
        let detail: Vec<String> = unknown
            .iter()
            .map(|item| format!("{}（{}）", item.item, item.reason))
            .collect();
        return Err(Adm4Error::blocked(format!(
            "R2: C0 编译阻塞，{} 项无法映射：{}",
            unknown.len(),
            detail.join("; ")
        )));
    }

    intent.experience_promise = promises.join("\n");
    intent.genre_structure = genre_parts.join(" + ");

    let spec = GameSpec {
        identity: SpecIdentity {
            schema_version: SPEC_SCHEMA_VERSION.into(),
            project_id: format!(
                "{}@{}",
                frozen.project_name,
                &frozen.content_hash[..frozen.content_hash.len().min(23)]
            ),
            frozen_hash: frozen.content_hash.clone(),
        },
        intent,
        systems,
        mechanics,
        entities,
        tables,
        content,
        graphs,
        acceptance: Vec::new(),
        source_map,
    };

    let violations = validate_game_spec(&spec);
    if !violations.is_empty() {
        let detail: Vec<String> = violations
            .iter()
            .map(|violation| format!("[{}] {}", violation.code, violation.message))
            .collect();
        return Err(Adm4Error::validation(format!(
            "C0 编译产物未通过 GameSpec 校验（{} 项）：{}",
            violations.len(),
            detail.join("; ")
        )));
    }
    Ok(spec)
}

/// 预扫描：每个决策点实际会产出哪些 `SystemSpec` id（主选在前）。
/// 单选点恒为 `[决策id]`；多选点为 `[决策id#选项id, ...]`。
fn scan_system_ids(frozen: &FrozenDesign, space: &DesignSpace) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for selection in &frozen.decisions {
        let Some(point) = space.graph.point(&selection.decision_id) else {
            continue;
        };
        let selected = selection.selected_options();
        let multi = selected.len() > 1;
        for item in selected {
            let Some(option) = point.option(item.option_id) else {
                continue;
            };
            let role = match option.compiler_tags.get("spec_role") {
                Some(role) => role.as_str(),
                None => default_role(point.level),
            };
            if role != "system" {
                continue;
            }
            let element_id = if multi {
                format!("{}#{}", selection.decision_id, item.option_id)
            } else {
                selection.decision_id.clone()
            };
            map.entry(selection.decision_id.clone())
                .or_default()
                .push(element_id);
        }
    }
    map
}

fn default_role(level: DesignLevel) -> &'static str {
    match level {
        DesignLevel::L0 => "profile",
        DesignLevel::L1 => "promise",
        DesignLevel::L2 => "genre",
        DesignLevel::L3 => "system",
        DesignLevel::L4 => "mechanic",
        DesignLevel::L5 => "data_table",
        DesignLevel::L6 => "content",
    }
}

fn entry(spec_path: &str, decision_id: &str) -> SpecSourceEntry {
    SpecSourceEntry {
        spec_path: SpecRef::new(spec_path),
        decision_id: decision_id.to_string(),
    }
}

fn scalar_text(parameters: &ParameterValues, key: &str) -> Option<String> {
    match parameters {
        ParameterValues::Scalars { entries } => entries.get(key).map(TypedValue::render),
        _ => None,
    }
}

/// design_notes 收集（W7 定稿 §5.5）：选项的非空 rationale → Rationale 注记；
/// 标量参数里 `statement` 键的非空文本 → Statement 注记。两者皆空 → 空集（不产注记）。
fn collect_design_notes(
    decision_id: &str,
    option_id: &str,
    rationale: &str,
    parameters: &ParameterValues,
) -> Vec<DesignNote> {
    let mut notes = Vec::new();
    if !rationale.trim().is_empty() {
        notes.push(DesignNote {
            source_decision: decision_id.to_string(),
            source_option: option_id.to_string(),
            role: DesignNoteRole::Rationale,
            text: rationale.trim().to_string(),
        });
    }
    if let Some(statement) = scalar_text(parameters, STATEMENT_PARAM_KEY)
        && !statement.trim().is_empty()
    {
        notes.push(DesignNote {
            source_decision: decision_id.to_string(),
            source_option: option_id.to_string(),
            role: DesignNoteRole::Statement,
            text: statement.trim().to_string(),
        });
    }
    notes
}

fn render_scalar_suffix(parameters: &ParameterValues) -> String {
    match parameters {
        ParameterValues::Scalars { entries } if !entries.is_empty() => {
            let rendered: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{key}={}", value.render()))
                .collect();
            format!("（{}）", rendered.join(", "))
        }
        _ => String::new(),
    }
}

/// L4 机制编译：效果模板占位符替换；缺模板/缺系统归属 → R2 阻塞。
fn compile_mechanic(
    space: &DesignSpace,
    selection: &Selection,
    point: &adm4_decision::DecisionPoint,
    option: &adm4_decision::DecisionOption,
    selected: &SelectedOptionRef<'_>,
    element_id: &str,
    system_ids_by_decision: &BTreeMap<String, Vec<String>>,
) -> Result<MechanicSpec, UnclassifiedItem> {
    // 系统归属：tags["system"] 显式指定 > 同域 L3 系统决策。
    // 标签既可以写系统 spec id，也可以写 L3 决策 id——后者在多选 L3 上解析到主选系统。
    let resolve = |candidate: &str| -> Option<String> {
        match system_ids_by_decision.get(candidate) {
            Some(ids) => ids.first().cloned(),
            None => None,
        }
    };
    let system_id = match option.compiler_tags.get("system") {
        Some(tag) => match resolve(tag) {
            Some(resolved) => resolved,
            // 标签直接写的就是系统 spec id（多选点的 `决策id#选项id` 形态）：原样采用，
            // 悬空引用由 GameSpec 校验的 mechanic_dangling_system 兜住。
            None => tag.clone(),
        },
        None => {
            let same_domain = space
                .graph
                .points()
                .iter()
                .find(|candidate| {
                    candidate.domain == point.domain
                        && candidate.level == DesignLevel::L3
                        && system_ids_by_decision.contains_key(&candidate.id)
                })
                .and_then(|candidate| resolve(&candidate.id));
            match same_domain {
                Some(resolved) => resolved,
                None => {
                    return Err(UnclassifiedItem {
                        item: selection.decision_id.clone(),
                        reason: "机制无法归属到任何系统（无 system 标签且同域无已选 L3 系统决策）"
                            .into(),
                    });
                }
            }
        }
    };

    if option.effects_template.is_empty() {
        return Err(UnclassifiedItem {
            item: format!("{}/{}", selection.decision_id, selected.option_id),
            reason: "L4 机制选项缺少 effects_template，流水线不发明效果（R2）".into(),
        });
    }

    let mut effects = Vec::new();
    for template in &option.effects_template {
        let substituted = substitute_placeholders(template, selected.parameters);
        match serde_json::from_value::<EffectSpec>(substituted) {
            Ok(effect) => effects.push(effect),
            Err(error) => {
                return Err(UnclassifiedItem {
                    item: format!("{}/{}", selection.decision_id, selected.option_id),
                    reason: format!("效果模板解析失败：{error}"),
                });
            }
        }
    }

    // 第 3 层 Custom 缺 GWT → R2 阻塞（W7 定稿 §5.3）：转录投影只誊写设计者
    // 自己写的验收模板，模板缺段与「缺 effects_template 即阻塞」同一纪律。
    // 点名机制 id 与缺段，递归覆盖嵌套效果内层的 Custom。
    let mut missing_gwt = Vec::new();
    for effect in &effects {
        collect_missing_custom_gwt(effect, &mut missing_gwt);
    }
    if !missing_gwt.is_empty() {
        return Err(UnclassifiedItem {
            item: format!("{}/{}", selection.decision_id, selected.option_id),
            reason: format!(
                "机制 {element_id} 的 Custom 效果 GWT 三段模板缺段（R2 阻塞）：{}",
                missing_gwt.join("; ")
            ),
        });
    }

    // 多选点的主选在规则文本里带标记，下游（C2 叙述/C6 计划）能看出主次。
    let rule_text = format!(
        "{}{}。{}{}",
        option.label,
        if selected.is_primary {
            "（主选）"
        } else {
            ""
        },
        option.summary,
        render_scalar_suffix(selected.parameters)
    );

    Ok(MechanicSpec {
        id: element_id.to_string(),
        system_id,
        rule_text,
        preconditions: Vec::new(),
        effects,
        state_machine: None,
        design_notes: Vec::new(),
    })
}

/// 递归收集 Custom 效果的 GWT 缺段描述（如 `custom(verb=merge) 缺 then`），
/// 嵌套容器（AreaApply/Schedule/RollCheck）内层一并检查。
fn collect_missing_custom_gwt(effect: &EffectSpec, missing: &mut Vec<String>) {
    match effect {
        EffectSpec::Custom {
            verb,
            given,
            when_,
            then,
            ..
        } => {
            let mut absent = Vec::new();
            if given.trim().is_empty() {
                absent.push("given");
            }
            if when_.trim().is_empty() {
                absent.push("when");
            }
            if then.trim().is_empty() {
                absent.push("then");
            }
            if !absent.is_empty() {
                missing.push(format!("custom(verb={verb}) 缺 {}", absent.join("/")));
            }
        }
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            for nested in inner {
                collect_missing_custom_gwt(nested, missing);
            }
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => {
            for nested in on_success.iter().chain(on_failure) {
                collect_missing_custom_gwt(nested, missing);
            }
        }
        _ => {}
    }
}

/// 效果模板占位符：字符串值中的 `{param:KEY}` 用标量参数替换。
fn substitute_placeholders(
    template: &serde_json::Value,
    parameters: &ParameterValues,
) -> serde_json::Value {
    match template {
        serde_json::Value::String(text) => {
            let mut result = text.clone();
            if let ParameterValues::Scalars { entries } = parameters {
                for (key, value) in entries {
                    result = result.replace(&format!("{{param:{key}}}"), &value.render());
                }
            }
            serde_json::Value::String(result)
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), substitute_placeholders(value, parameters)))
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| substitute_placeholders(item, parameters))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// L5 实体表：每行 → EntitySpec；列 → PropertySpec；visual_form 由标签声明（C3 白名单依据）。
///
/// `element_id` 是本条 spec 元素的 id（单选点 = 决策 id；多选点 = `决策id#选项id`）。
fn compile_entity_table(
    option: &adm4_decision::DecisionOption,
    parameters: &ParameterValues,
    element_id: &str,
) -> Result<(Vec<EntitySpec>, TableSpec), UnclassifiedItem> {
    let ParameterSchema::Table(schema) = &option.parameter_schema else {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: "entity_table 角色要求 Table 参数结构".into(),
        });
    };
    let ParameterValues::Rows { rows } = parameters else {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: "entity_table 选择缺少行数据".into(),
        });
    };
    let visual_form = match option.compiler_tags.get("visual_form").map(String::as_str) {
        Some("sprite2d") => Some(VisualForm::Sprite2d),
        Some("model3d") => Some(VisualForm::Model3d),
        Some("ui_only") => Some(VisualForm::UiOnly),
        Some("invisible") => Some(VisualForm::Invisible),
        None => None,
        Some(other) => {
            return Err(UnclassifiedItem {
                item: element_id.to_string(),
                reason: format!("未知 visual_form 标签：{other}"),
            });
        }
    };
    let properties: Vec<PropertySpec> = schema
        .columns
        .iter()
        .map(|column| PropertySpec {
            key: column.key.clone(),
            kind: column.kind.clone(),
            constraint: column.constraint.clone(),
        })
        .collect();
    let mut entities = Vec::new();
    for row in rows {
        let Some(row_id) = row.get(&schema.row_key).map(TypedValue::render) else {
            return Err(UnclassifiedItem {
                item: element_id.to_string(),
                reason: format!("行缺少标识列 {}", schema.row_key),
            });
        };
        entities.push(EntitySpec {
            id: format!("{element_id}.{row_id}"),
            name: row_id,
            visual_form: visual_form.clone(),
            properties: properties.clone(),
        });
    }
    let table = TableSpec {
        id: element_id.to_string(),
        columns: properties,
        row_key: schema.row_key.clone(),
        rows: rows.clone(),
        cells: Vec::new(),
        design_notes: Vec::new(),
    };
    Ok((entities, table))
}

/// L5/L6 数据表：Table 行数据或 Matrix 格数据 → TableSpec；
/// `data_form=curve` 走 Curve→两列 Table+插值注记通路（W7 定稿 §5.4）。
fn compile_data_table(
    option: &adm4_decision::DecisionOption,
    parameters: &ParameterValues,
    element_id: &str,
) -> Result<TableSpec, UnclassifiedItem> {
    match option.compiler_tags.get(DATA_FORM_TAG).map(String::as_str) {
        Some("curve") => return compile_curve_table(option, parameters, element_id),
        // Graph 在主循环里先于 data_table 分流到 compile_graph（产 GraphSpec 不产表）；
        // 走到这里说明调用路径接错，按 R2 显式申报而不是静默当普通表编译。
        Some("graph") => {
            return Err(UnclassifiedItem {
                item: element_id.to_string(),
                reason: "内部路由错误：data_form=graph 应由 compile_graph 编译进 GameSpec.graphs，\
                         不走表通路（R2）"
                    .into(),
            });
        }
        Some(other) => {
            return Err(UnclassifiedItem {
                item: element_id.to_string(),
                reason: format!("未知 data_form 标签：{other}"),
            });
        }
        None => {}
    }
    match (&option.parameter_schema, parameters) {
        (ParameterSchema::Table(schema), ParameterValues::Rows { rows }) => Ok(TableSpec {
            id: element_id.to_string(),
            columns: schema
                .columns
                .iter()
                .map(|column| PropertySpec {
                    key: column.key.clone(),
                    kind: column.kind.clone(),
                    constraint: column.constraint.clone(),
                })
                .collect(),
            row_key: schema.row_key.clone(),
            rows: rows.clone(),
            cells: Vec::new(),
            design_notes: Vec::new(),
        }),
        (ParameterSchema::Matrix(schema), ParameterValues::Cells { cells }) => Ok(TableSpec {
            id: element_id.to_string(),
            columns: vec![PropertySpec {
                key: schema.cell.key.clone(),
                kind: schema.cell.kind.clone(),
                constraint: schema.cell.constraint.clone(),
            }],
            row_key: "row".into(),
            rows: Vec::new(),
            cells: cells.clone(),
            design_notes: Vec::new(),
        }),
        (ParameterSchema::Scalar { .. }, ParameterValues::Scalars { entries }) => Ok(TableSpec {
            id: element_id.to_string(),
            columns: Vec::new(),
            row_key: "key".into(),
            rows: vec![
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ],
            cells: Vec::new(),
            design_notes: Vec::new(),
        }),
        _ => Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: "data_table 角色的参数结构与数据形态不匹配".into(),
        }),
    }
}

/// Curve → 两列 TableSpec + 插值注记（W7 定稿 §5.4：不加新 section，复用既有表通路）。
///
/// Curve 数据以标量参数 `curve` 键承载 `CurveSpec` JSON 文本（schema 侧尚无 Curve
/// 变体）。采样点必须非空且按 x 严格升序，否则 R2 阻塞。插值方式以 Statement
/// 注记落在表上——注记只携带不编译成结构（保 I1），下游查表仍是纯表语义。
fn compile_curve_table(
    _option: &adm4_decision::DecisionOption,
    parameters: &ParameterValues,
    element_id: &str,
) -> Result<TableSpec, UnclassifiedItem> {
    let Some(raw) = scalar_text(parameters, CURVE_PARAM_KEY) else {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: format!(
                "data_form=curve 要求标量参数含 {CURVE_PARAM_KEY} 键（CurveSpec JSON）"
            ),
        });
    };
    let curve: CurveSpec = serde_json::from_str(&raw).map_err(|error| UnclassifiedItem {
        item: element_id.to_string(),
        reason: format!("Curve 参数解析失败：{error}"),
    })?;
    if curve.points.is_empty() {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: "Curve 采样点为空（至少 1 个 (x, y) 点）".into(),
        });
    }
    if curve.points.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: "Curve 采样点必须按 x 严格升序".into(),
        });
    }
    let interpolation = match curve.interpolation {
        CurveInterpolation::Linear => "linear",
        CurveInterpolation::Step => "step",
        CurveInterpolation::Cubic => "cubic",
    };
    Ok(TableSpec {
        id: element_id.to_string(),
        columns: vec![
            PropertySpec {
                key: "x".into(),
                kind: ValueKind::Float,
                constraint: None,
            },
            PropertySpec {
                key: "y".into(),
                kind: ValueKind::Float,
                constraint: None,
            },
        ],
        row_key: "x".into(),
        rows: curve
            .points
            .iter()
            .map(|(x, y)| {
                BTreeMap::from([
                    ("x".to_string(), TypedValue::Float(*x)),
                    ("y".to_string(), TypedValue::Float(*y)),
                ])
            })
            .collect(),
        cells: Vec::new(),
        design_notes: vec![DesignNote {
            source_decision: element_id.to_string(),
            source_option: curve.id.clone(),
            role: DesignNoteRole::Statement,
            text: format!(
                "插值注记：本表由 Curve「{}」编译为两列 (x, y) 采样表，插值方式 {interpolation}；\
                 点间取值按该插值方式求值（W7 §5.4，注记只携带不编译成结构）",
                curve.id
            ),
        }],
    })
}

/// Graph → GraphSpec（W7 定稿 §5.4，T-W7-3a 翻转波 1 的结构化 Err 降级）。
///
/// 值形态：标量参数 `graph` 键装 GraphSpec 形态 JSON 文本（nodes/edges）。
/// **schema 为真相**：directed/acyclic/entry 一律取 `ParameterSchema::Graph`
/// 的声明覆盖，值里写什么都不算数——同一份值换 schema 语义即变，真相必须唯一。
/// 结构校验（端点已声明 / acyclic 拓扑 / 单入口 / 必填负载）走 adm4-decision 的
/// `validate_graph_value` 纯函数，问题清单逐条进 R2 阻塞文案。
fn compile_graph(
    option: &adm4_decision::DecisionOption,
    parameters: &ParameterValues,
    element_id: &str,
) -> Result<GraphSpec, UnclassifiedItem> {
    let ParameterSchema::Graph(schema) = &option.parameter_schema else {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: "data_form=graph 要求选项声明 ParameterSchema Graph 分支\
                     （directed/acyclic/entry 以 schema 为真相，缺 schema 即无真相，R2）"
                .into(),
        });
    };
    let Some(raw) = scalar_text(parameters, GRAPH_PARAM_KEY) else {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: format!(
                "data_form=graph 要求标量参数含 {GRAPH_PARAM_KEY} 键（GraphSpec 形态 JSON）"
            ),
        });
    };
    let mut value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|error| UnclassifiedItem {
            item: element_id.to_string(),
            reason: format!("Graph 参数解析失败：{error}"),
        })?;
    let problems = validate_graph_value(schema, &value);
    if !problems.is_empty() {
        return Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: format!("Graph 参数结构校验失败：{}", problems.join("; ")),
        });
    }
    // id 一律取决策 id（值内不承载 id：spec 元素锚点由编译器统一命名，与表通路一致）。
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "id".to_string(),
            serde_json::Value::String(element_id.to_string()),
        );
    }
    let mut graph: GraphSpec = serde_json::from_value(value).map_err(|error| UnclassifiedItem {
        item: element_id.to_string(),
        reason: format!("Graph 参数不符合 GraphSpec 形态：{error}"),
    })?;
    graph.directed = schema.directed;
    graph.acyclic = schema.acyclic;
    graph.entry = match schema.entry {
        GraphEntryConstraint::Single => GraphEntry::Single,
        GraphEntryConstraint::Multiple => GraphEntry::Multiple,
    };
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_authoring::{FreezeGateReport, FrozenDesign};
    use adm4_decision::{
        DecisionGraph, DecisionOption, DecisionPoint, DepthProfile, DesignOrganization, GenreScope,
        GraphSchema, PointRequirement, Provenance, SelectionMode,
    };
    use adm4_space::GenrePack;

    fn point(id: &str, level: DesignLevel, options: Vec<DecisionOption>) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "gameplay".into(),
            level,
            genre_scope: GenreScope::Universal,
            question: format!("{id}？"),
            mda_layer: None,
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: None,
            options,
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    /// 每个测试空间都带一个 L0 画像点：validate_game_spec 要求 `intent`
    /// 在 source_map 有落点，画像选择是最小成本的合法来源。
    fn space_with(mut points: Vec<DecisionPoint>) -> DesignSpace {
        points.insert(
            0,
            point(
                "u.audience",
                DesignLevel::L0,
                vec![DecisionOption {
                    id: "core_players".into(),
                    label: "核心玩家".into(),
                    ..Default::default()
                }],
            ),
        );
        DesignSpace {
            universal_version: "test".into(),
            pack: GenrePack {
                pack_id: "c0_test".into(),
                pack_version: "0.1.0".into(),
                display_name: "C0 单元测试包".into(),
                reference_games: vec!["甲".into(), "乙".into(), "丙".into()],
                profile_points: Vec::new(),
                cardinality_expectations: Default::default(),
                consistency_rules: Vec::new(),
                nodes: Vec::new(),
                decision_points: Vec::new(),
                system_refs: Vec::new(),
                core_nouns: Vec::new(),
            },
            graph: DecisionGraph::new(points).expect("测试图构造失败"),
            organization: DesignOrganization::new(Vec::new(), Vec::new()),
            system_instances: Vec::new(),
        }
    }

    fn frozen_with(decisions: Vec<Selection>) -> FrozenDesign {
        let mut all = vec![selection("u.audience", "core_players", "")];
        all.extend(decisions);
        frozen_raw(all)
    }

    fn frozen_raw(decisions: Vec<Selection>) -> FrozenDesign {
        FrozenDesign {
            version: 1,
            project_name: "C0 收尾测试".into(),
            decisions,
            not_applicable: Vec::new(),
            genre_pack: "c0_test".into(),
            pack_version: "0.1.0".into(),
            depth_profile: DepthProfile::new(DesignLevel::L4).expect("depth"),
            content_hash: "sha256:c0test".into(),
            frozen_at: "2026-09-03T00:00:00Z".into(),
            gate_report: FreezeGateReport {
                gates: Vec::new(),
                custom_option_count: 0,
                na_counts: Vec::new(),
                optional_skipped: 0,
                evaluated_at: "2026-09-03T00:00:00Z".into(),
            },
            red_team_proof: None,
            module_versions: Default::default(),
        }
    }

    fn selection(decision_id: &str, option_id: &str, rationale: &str) -> Selection {
        Selection {
            decision_id: decision_id.into(),
            option_id: option_id.into(),
            parameters: ParameterValues::None,
            rationale: rationale.into(),
            provenance: Provenance::UserManual,
            confirmed_by_user: true,
            template_original: None,
            additional_options: Vec::new(),
            primary_option: None,
        }
    }

    fn system_option(id: &str) -> DecisionOption {
        DecisionOption {
            id: id.into(),
            label: format!("{id} 系统"),
            summary: format!("{id} 的职责"),
            compiler_tags: BTreeMap::from([("spec_role".to_string(), "system".to_string())]),
            ..Default::default()
        }
    }

    fn mechanic_option(id: &str, effects: Vec<serde_json::Value>) -> DecisionOption {
        DecisionOption {
            id: id.into(),
            label: format!("{id} 机制"),
            summary: format!("{id} 的规则"),
            compiler_tags: BTreeMap::from([
                ("spec_role".to_string(), "mechanic".to_string()),
                ("system".to_string(), "sys.core".to_string()),
            ]),
            effects_template: effects,
            ..Default::default()
        }
    }

    fn emit_effect() -> serde_json::Value {
        serde_json::json!({"effect": "emit_signal", "signal": "ping"})
    }

    /// 一个最小可编译空间：L3 系统点 + L4 机制点（机制效果由调用方指定）。
    fn minimal_space(mechanic_effects: Vec<serde_json::Value>) -> DesignSpace {
        space_with(vec![
            point("sys.core", DesignLevel::L3, vec![system_option("core")]),
            point(
                "mech.main",
                DesignLevel::L4,
                vec![mechanic_option("main", mechanic_effects)],
            ),
        ])
    }

    // ===== design_notes 五挂点收集（W7 §5.5）=====

    /// 正例：非空 rationale 流入 SystemSpec/MechanicSpec 的 design_notes；
    /// statement 参数键产 Statement 注记。
    #[test]
    fn design_notes_flow_into_system_and_mechanic() {
        let space = minimal_space(vec![emit_effect()]);
        let mut sys_selection = selection("sys.core", "core", "克制循环是本作乐趣核心");
        sys_selection.parameters = ParameterValues::Scalars {
            entries: BTreeMap::from([(
                STATEMENT_PARAM_KEY.to_string(),
                TypedValue::Text("玩家应始终感到以小博大".into()),
            )]),
        };
        let frozen = frozen_with(vec![
            sys_selection,
            selection("mech.main", "main", "线性公式便于新手理解"),
        ]);
        let spec = compile_frozen_design(&frozen, &space).expect("应编译成功");

        let system = &spec.systems[0];
        assert_eq!(system.design_notes.len(), 2, "{:?}", system.design_notes);
        assert_eq!(system.design_notes[0].role, DesignNoteRole::Rationale);
        assert_eq!(system.design_notes[0].text, "克制循环是本作乐趣核心");
        assert_eq!(system.design_notes[0].source_decision, "sys.core");
        assert_eq!(system.design_notes[0].source_option, "core");
        assert_eq!(system.design_notes[1].role, DesignNoteRole::Statement);
        assert_eq!(system.design_notes[1].text, "玩家应始终感到以小博大");

        let mechanic = &spec.mechanics[0];
        assert_eq!(mechanic.design_notes.len(), 1);
        assert_eq!(mechanic.design_notes[0].role, DesignNoteRole::Rationale);
        assert_eq!(mechanic.design_notes[0].text, "线性公式便于新手理解");
    }

    /// 反例：rationale 空白（含纯空格）且无 statement → 不产注记（空数组，不造键值噪音）。
    #[test]
    fn blank_rationale_produces_no_notes() {
        let space = minimal_space(vec![emit_effect()]);
        let frozen = frozen_with(vec![
            selection("sys.core", "core", "   "),
            selection("mech.main", "main", ""),
        ]);
        let spec = compile_frozen_design(&frozen, &space).expect("应编译成功");
        assert!(spec.systems[0].design_notes.is_empty());
        assert!(spec.mechanics[0].design_notes.is_empty());
    }

    /// data_table（Scalar 形态）的 rationale 也流入 TableSpec.design_notes（五挂点之表）。
    #[test]
    fn design_notes_flow_into_data_table() {
        let mut table_option = DecisionOption {
            id: "tbl".into(),
            label: "数值表".into(),
            parameter_schema: ParameterSchema::Scalar { fields: Vec::new() },
            ..Default::default()
        };
        table_option
            .compiler_tags
            .insert("spec_role".into(), "data_table".into());
        let space = space_with(vec![point(
            "data.tuning",
            DesignLevel::L5,
            vec![table_option],
        )]);
        let mut chosen = selection("data.tuning", "tbl", "节奏参数以 5 秒为锚");
        chosen.parameters = ParameterValues::Scalars {
            entries: BTreeMap::from([("interval".to_string(), TypedValue::Float(5.0))]),
        };
        let frozen = frozen_with(vec![chosen]);
        let spec = compile_frozen_design(&frozen, &space).expect("应编译成功");
        assert_eq!(spec.tables[0].design_notes.len(), 1);
        assert_eq!(spec.tables[0].design_notes[0].text, "节奏参数以 5 秒为锚");
    }

    // ===== Custom 缺 GWT → R2 阻塞（W7 §5.3 第 3 层）=====

    /// Custom 缺 then → R2 阻塞，错误点名机制 id 与缺段。
    #[test]
    fn custom_missing_then_blocks_with_mechanic_id_and_segment() {
        let space = minimal_space(vec![serde_json::json!({
            "effect": "custom", "verb": "merge",
            "given": "两个同级单位相邻", "when": "玩家拖拽合成", "then": ""
        })]);
        let frozen = frozen_with(vec![
            selection("sys.core", "core", ""),
            selection("mech.main", "main", ""),
        ]);
        let error = compile_frozen_design(&frozen, &space).expect_err("缺 then 必须 R2 阻塞");
        assert!(error.message.contains("R2"), "{}", error.message);
        assert!(error.message.contains("mech.main"), "{}", error.message);
        assert!(error.message.contains("verb=merge"), "{}", error.message);
        assert!(error.message.contains("缺 then"), "{}", error.message);
    }

    /// 嵌套容器（Schedule 内层）的 Custom 缺段同样被拦——递归不漏。
    #[test]
    fn nested_custom_missing_given_blocks() {
        let space = minimal_space(vec![serde_json::json!({
            "effect": "schedule", "timing": "delayed", "amount_expr": "1", "unit": "turns",
            "inner": [{"effect": "custom", "verb": "spawn_rift", "given": "", "when": "w", "then": "t"}]
        })]);
        let frozen = frozen_with(vec![
            selection("sys.core", "core", ""),
            selection("mech.main", "main", ""),
        ]);
        let error = compile_frozen_design(&frozen, &space).expect_err("嵌套缺 given 必须阻塞");
        assert!(error.message.contains("缺 given"), "{}", error.message);
        assert!(
            error.message.contains("verb=spawn_rift"),
            "{}",
            error.message
        );
    }

    /// Custom GWT 三段齐全 → 放行，效果原样转录进 MechanicSpec。
    #[test]
    fn custom_with_full_gwt_passes() {
        let space = minimal_space(vec![serde_json::json!({
            "effect": "custom", "verb": "merge",
            "given": "两个同级单位相邻", "when": "玩家拖拽合成", "then": "生成高一级单位"
        })]);
        let frozen = frozen_with(vec![
            selection("sys.core", "core", ""),
            selection("mech.main", "main", ""),
        ]);
        let spec = compile_frozen_design(&frozen, &space).expect("GWT 齐全应放行");
        match &spec.mechanics[0].effects[0] {
            EffectSpec::Custom { verb, then, .. } => {
                assert_eq!(verb, "merge");
                assert_eq!(then, "生成高一级单位");
            }
            other => panic!("期望 Custom，得到 {other:?}"),
        }
    }

    // ===== Curve → 两列 Table + 插值注记（W7 §5.4）=====

    fn curve_option() -> DecisionOption {
        DecisionOption {
            id: "xp".into(),
            label: "经验曲线".into(),
            parameter_schema: ParameterSchema::Scalar { fields: Vec::new() },
            compiler_tags: BTreeMap::from([
                ("spec_role".to_string(), "data_table".to_string()),
                (DATA_FORM_TAG.to_string(), "curve".to_string()),
            ]),
            ..Default::default()
        }
    }

    fn curve_selection(curve_json: &str) -> Selection {
        let mut chosen = selection("data.xp_curve", "xp", "");
        chosen.parameters = ParameterValues::Scalars {
            entries: BTreeMap::from([(
                CURVE_PARAM_KEY.to_string(),
                TypedValue::Text(curve_json.into()),
            )]),
        };
        chosen
    }

    /// Curve 编译成两列 (x, y) TableSpec，行序即采样点序，插值方式落 Statement 注记。
    #[test]
    fn curve_compiles_to_two_column_table_with_interpolation_note() {
        let space = space_with(vec![point(
            "data.xp_curve",
            DesignLevel::L5,
            vec![curve_option()],
        )]);
        let frozen = frozen_with(vec![curve_selection(
            r#"{"id":"xp","interpolation":"cubic","points":[[0.0,0.0],[1.0,100.0],[2.0,350.0]]}"#,
        )]);
        let spec = compile_frozen_design(&frozen, &space).expect("Curve 应编译成功");
        let table = &spec.tables[0];
        assert_eq!(table.id, "data.xp_curve");
        let keys: Vec<&str> = table.columns.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, vec!["x", "y"], "两列 (x, y)");
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[2].get("y"), Some(&TypedValue::Float(350.0)));
        assert_eq!(table.design_notes.len(), 1, "插值注记恰一条");
        let note = &table.design_notes[0];
        assert_eq!(note.role, DesignNoteRole::Statement);
        assert!(note.text.contains("cubic"), "{}", note.text);
        assert!(note.text.contains("插值"), "{}", note.text);
    }

    /// Curve 采样点非严格升序 → R2 阻塞（不静默重排——重排等于替设计者做决定）。
    #[test]
    fn curve_unsorted_points_block() {
        let space = space_with(vec![point(
            "data.xp_curve",
            DesignLevel::L5,
            vec![curve_option()],
        )]);
        let frozen = frozen_with(vec![curve_selection(
            r#"{"id":"xp","points":[[1.0,100.0],[0.0,0.0]]}"#,
        )]);
        let error = compile_frozen_design(&frozen, &space).expect_err("乱序采样点必须阻塞");
        assert!(error.message.contains("严格升序"), "{}", error.message);
    }

    // ===== Graph → GraphSpec（W7 §5.4，T-W7-3a 翻转波 1 的结构化 Err 降级）=====

    fn graph_option(schema: GraphSchema) -> DecisionOption {
        DecisionOption {
            id: "map".into(),
            label: "地图图".into(),
            parameter_schema: ParameterSchema::Graph(schema),
            compiler_tags: BTreeMap::from([
                ("spec_role".to_string(), "data_table".to_string()),
                (DATA_FORM_TAG.to_string(), "graph".to_string()),
            ]),
            ..Default::default()
        }
    }

    fn graph_schema(acyclic: bool, entry: GraphEntryConstraint) -> GraphSchema {
        GraphSchema {
            node_payload: Vec::new(),
            edge_payload: Vec::new(),
            directed: true,
            acyclic,
            entry,
            cardinality_key: String::new(),
        }
    }

    fn graph_selection(graph_json: &str) -> Selection {
        let mut chosen = selection("data.map", "map", "");
        chosen.parameters = ParameterValues::Scalars {
            entries: BTreeMap::from([(
                GRAPH_PARAM_KEY.to_string(),
                TypedValue::Text(graph_json.into()),
            )]),
        };
        chosen
    }

    /// Graph 正路径（T-W7-3a 翻转豁免）：schema 点 + 图值 → GraphSpec 进 GameSpec.graphs，
    /// directed/acyclic/entry 以 schema 为真相覆盖，source_map 登记 graphs/ 路径（R4）。
    #[test]
    fn graph_compiles_into_game_spec_graphs_with_schema_as_truth() {
        let space = space_with(vec![point(
            "data.map",
            DesignLevel::L5,
            vec![graph_option(graph_schema(
                true,
                GraphEntryConstraint::Single,
            ))],
        )]);
        // 值里故意写反 directed/acyclic/entry：schema 为真相，值内声明不算数。
        let frozen = frozen_with(vec![graph_selection(
            r#"{"directed":false,"acyclic":false,"entry":"multiple",
                "nodes":[{"id":"start"},{"id":"mid"},{"id":"boss"}],
                "edges":[{"from":"start","to":"mid"},{"from":"mid","to":"boss"}]}"#,
        )]);
        let spec = compile_frozen_design(&frozen, &space).expect("合法图应编译成功");
        assert_eq!(spec.graphs.len(), 1, "GameSpec.graphs 应非空");
        let graph = &spec.graphs[0];
        assert_eq!(graph.id, "data.map");
        assert!(graph.directed && graph.acyclic, "schema 为真相覆盖值内声明");
        assert_eq!(graph.entry, adm4_spec::GraphEntry::Single);
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        // R4 锚定闭合：source_map 里有 graphs/ 路径。
        assert!(
            spec.source_map
                .iter()
                .any(|entry| entry.spec_path.0 == "graphs/data.map"),
            "source_map 应登记 graphs/data.map"
        );
    }

    /// 负测试（豁免条款保留）：悬空边端点必须拒绝并点名。
    #[test]
    fn graph_dangling_edge_endpoint_blocks() {
        let space = space_with(vec![point(
            "data.map",
            DesignLevel::L5,
            vec![graph_option(graph_schema(
                false,
                GraphEntryConstraint::Multiple,
            ))],
        )]);
        let frozen = frozen_with(vec![graph_selection(
            r#"{"nodes":[{"id":"a"}],"edges":[{"from":"a","to":"ghost"}]}"#,
        )]);
        let error = compile_frozen_design(&frozen, &space).expect_err("悬空端点必须阻塞");
        assert!(error.message.contains("ghost"), "{}", error.message);
        assert!(error.message.contains("data.map"), "{}", error.message);
    }

    /// 负测试（豁免条款保留）：schema 声明 acyclic 但图值有环必须拒绝。
    #[test]
    fn graph_cycle_under_acyclic_schema_blocks() {
        let space = space_with(vec![point(
            "data.map",
            DesignLevel::L5,
            vec![graph_option(graph_schema(
                true,
                GraphEntryConstraint::Multiple,
            ))],
        )]);
        let frozen = frozen_with(vec![graph_selection(
            r#"{"nodes":[{"id":"a"},{"id":"b"}],
                "edges":[{"from":"a","to":"b"},{"from":"b","to":"a"}]}"#,
        )]);
        let error = compile_frozen_design(&frozen, &space).expect_err("acyclic 下的环必须阻塞");
        assert!(error.message.contains("环"), "{}", error.message);
    }

    /// 负测试：缺 graph 键 / 缺 Graph schema 声明都必须结构化申报。
    #[test]
    fn graph_missing_key_or_schema_blocks() {
        let space = space_with(vec![point(
            "data.map",
            DesignLevel::L5,
            vec![graph_option(graph_schema(
                false,
                GraphEntryConstraint::Multiple,
            ))],
        )]);
        let frozen = frozen_with(vec![selection("data.map", "map", "")]);
        let error = compile_frozen_design(&frozen, &space).expect_err("缺 graph 键必须阻塞");
        assert!(error.message.contains(GRAPH_PARAM_KEY), "{}", error.message);

        // data_form=graph 但选项没声明 Graph schema：无真相 → 阻塞。
        let mut option = curve_option();
        option
            .compiler_tags
            .insert(DATA_FORM_TAG.into(), "graph".into());
        let space = space_with(vec![point("data.map", DesignLevel::L5, vec![option])]);
        let mut chosen = selection("data.map", "xp", "");
        chosen.parameters = ParameterValues::Scalars {
            entries: BTreeMap::from([(
                GRAPH_PARAM_KEY.to_string(),
                TypedValue::Text(r#"{"nodes":[]}"#.into()),
            )]),
        };
        let frozen = frozen_with(vec![chosen]);
        let error = compile_frozen_design(&frozen, &space).expect_err("缺 schema 必须阻塞");
        assert!(error.message.contains("schema"), "{}", error.message);
        assert!(error.message.contains("data.map"), "{}", error.message);
    }
}
