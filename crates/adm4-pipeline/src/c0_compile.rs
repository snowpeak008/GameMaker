use adm4_authoring::FrozenDesign;
use adm4_contracts::{SpecRef, TypedValue, UnclassifiedItem};
use adm4_decision::{DesignLevel, ParameterSchema, ParameterValues, SelectedOptionRef, Selection};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_space::DesignSpace;
use adm4_spec::{
    ContentSpec, EffectSpec, EntitySpec, GameSpec, MechanicSpec, ProjectIntent, PropertySpec,
    SPEC_SCHEMA_VERSION, SpecIdentity, SpecSourceEntry, SystemSpec, TableSpec, VisualForm,
    validate_game_spec,
};
use std::collections::BTreeMap;

/// C0：FrozenDesign → GameSpec 的确定性编译（无 AI）。
///
/// 分层映射：L0-L1→intent；L2→genre_structure；L3→SystemSpec；L4→MechanicSpec；
/// L5→EntitySpec/TableSpec；L6→行数据/ContentSpec。
/// 无法映射的选择 → R2 阻塞，绝不跳过；每个 spec 元素登记 source_map。
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
                        Ok(mechanic) => {
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
                        Ok((mut new_entities, table)) => {
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
                "data_table" => match compile_data_table(option, item.parameters, &element_id) {
                    Ok(table) => {
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
    })
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
    };
    Ok((entities, table))
}

/// L5/L6 数据表：Table 行数据或 Matrix 格数据 → TableSpec。
fn compile_data_table(
    option: &adm4_decision::DecisionOption,
    parameters: &ParameterValues,
    element_id: &str,
) -> Result<TableSpec, UnclassifiedItem> {
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
        }),
        _ => Err(UnclassifiedItem {
            item: element_id.to_string(),
            reason: "data_table 角色的参数结构与数据形态不匹配".into(),
        }),
    }
}
