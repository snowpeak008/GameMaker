use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use adm_new_contracts::{
    ArtifactLocale,
    project::{DecisionState, NodeState, OptionProvenanceEntry, ProjectState},
};
use adm_new_foundation::{AdmResult, io, sha256_hex};
use serde_json::{Map, Value, json};

use crate::{
    DesignEngineService, DesignNodeSpec,
    contracts::build_playable_contract_bundle_from_decisions_with_locale,
    semantic_pipeline::{ArchetypeCatalog, build_archetype_requirements_with_locale},
};

pub const EXPORT_SCHEMA_VERSION: &str = "0.5.0";
pub const DOCUMENT_VERSION: u32 = 1;
pub const TAXONOMY_VERSION: &str = "v1";
pub const PROVENANCE_AUTHOR: &str = "exporter_compatibility_layer";

const PLAYABLE_CONTRACT_REGISTRY: &[(&str, &str)] = &[
    (
        "core_playable_contract",
        "knowledge/schemas/playable_contracts/core_playable_contract.schema.json",
    ),
    (
        "demo_flow_contract",
        "knowledge/schemas/playable_contracts/demo_flow_contract.schema.json",
    ),
    (
        "runtime_data_contract",
        "knowledge/schemas/playable_contracts/runtime_data_contract.schema.json",
    ),
    (
        "ui_flow_contract",
        "knowledge/schemas/playable_contracts/ui_flow_contract.schema.json",
    ),
    (
        "scene_bootstrap_contract",
        "knowledge/schemas/playable_contracts/scene_bootstrap_contract.schema.json",
    ),
    (
        "asset_mount_contract",
        "knowledge/schemas/playable_contracts/asset_mount_contract.schema.json",
    ),
    (
        "audio_requirements_contract",
        "knowledge/schemas/playable_contracts/audio_requirements_contract.schema.json",
    ),
    (
        "playable_acceptance_contract",
        "knowledge/schemas/playable_contracts/playable_acceptance_contract.schema.json",
    ),
];

pub fn safe_file_name(value: &str, fallback: &str) -> String {
    let cleaned = value
        .trim()
        .chars()
        .map(|ch| {
            if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .replace(' ', "_")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

pub fn stable_json_hash(payload: &Value) -> String {
    let data = serde_json::to_vec(payload).unwrap_or_default();
    sha256_hex(&data)
}

pub fn taxonomy_fingerprint(specs: &[DesignNodeSpec]) -> String {
    let canonical = specs
        .iter()
        .map(|spec| {
            json!({
                "domain": spec.domain_id,
                "node": spec.node_id,
                "roleClass": spec.role_class,
                "checklist": spec.checklist.iter().map(|item| {
                    json!({
                        "id": item.item_id,
                        "optionGroups": item.option_groups.iter().map(|group| {
                            json!({
                                "id": group.group_id,
                                "selectionMode": group.selection_mode,
                                "allowPrimary": group.allow_primary,
                                "options": group.options,
                            })
                        }).collect::<Vec<_>>(),
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    stable_json_hash(&json!(canonical))
}

pub fn build_payload(engine: &DesignEngineService, project_state: &ProjectState) -> Value {
    let normalized = engine.normalize_state(project_state.clone());
    let view = engine.view_model(&normalized);
    let exported_at = io::now_iso();
    let taxonomy_hash = taxonomy_fingerprint(engine.specs());
    let metadata = build_document_metadata(&normalized, &taxonomy_hash, &exported_at);
    let domains = build_domain_payloads(engine, &normalized);
    let coverage_metrics = build_coverage_metrics(&view, &domains);
    json!({
        "schemaVersion": EXPORT_SCHEMA_VERSION,
        "exportedAt": exported_at,
        "projectName": normalized.project_name,
        "documentMetadata": metadata,
        "taxonomy": build_taxonomy_payload(engine.specs(), &taxonomy_hash),
        "profile": normalized.profile,
        "profileDisplay": profile_display(&normalized.profile),
        "projectCoverage": {
            "nodePercent": view.project_coverage.node_percent,
            "checklistPercent": view.project_coverage.checklist_percent,
            "doneNodes": view.project_coverage.done_nodes,
            "totalNodes": view.project_coverage.total_nodes,
            "doneChecklist": view.project_coverage.done_checklist,
            "totalChecklist": view.project_coverage.total_checklist,
        },
        "coverage": {
            "nodePercent": view.project_coverage.node_percent,
            "checklistPercent": view.project_coverage.checklist_percent,
        },
        "coverageMetrics": coverage_metrics,
        "qualityBadge": view.quality_metrics.quality_badge,
        "structureCoverage": {"nodePercent": view.project_coverage.node_percent, "checklistPercent": view.project_coverage.checklist_percent},
        "concretenessCoverage": {"l4Done": view.project_l4_progress.done, "l4Total": view.project_l4_progress.total},
        "consistencyScore": {"score": if view.quality_metrics.quality_critical_count == 0 { 1.0 } else { 0.0 }},
        "qualityViolations": view.quality_metrics.quality_violations,
        "qualityCriticalCount": view.quality_metrics.quality_critical_count,
        "crossLayerViolations": [],
        "gameplaySystems": {
            "schemaVersion": normalized.gameplay_systems.schema_version,
            "selected": normalized.gameplay_systems.selected,
            "custom": normalized.gameplay_systems.custom,
            "weights": normalized.gameplay_systems.weights,
            "coreLoops": normalized.gameplay_systems.core_loops,
            "interview": normalized.gameplay_systems.interview,
            "weightSummary": gameplay_weight_summary(&project_state.gameplay_systems.weights),
            "validationMessages": [],
        },
        "gameplaySystemGlobalView": gameplay_global_view(project_state),
        "domains": domains,
    })
}

pub fn export_preview_lines(
    engine: &DesignEngineService,
    project_state: &ProjectState,
    export_format: &str,
    export_scope: &str,
    include_gameplay_global_view: bool,
) -> Vec<String> {
    let payload = build_payload(engine, project_state);
    let fmt = export_format.to_ascii_lowercase();
    let scope = if fmt == "json" {
        "archive".to_string()
    } else {
        export_scope.to_ascii_lowercase()
    };
    let gameplay_count = payload["gameplaySystems"]["selected"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let appendix = if include_gameplay_global_view && fmt != "json" && gameplay_count > 0 {
        ", with gameplay global view"
    } else {
        ""
    };
    if fmt == "json" || scope == "archive" {
        let totals = payload_totals(&payload);
        return vec![
            "Scope: full export".to_string(),
            format!(
                "Contains {} domains, {} nodes, {} checklist items",
                totals["domains"], totals["nodes"], totals["checklist"]
            ),
            format!(
                "L4: {} option groups and {} options",
                totals["groups"], totals["options"]
            ),
            format!("Gameplay systems: {gameplay_count}{appendix}"),
            format!("Active conflicts: {}", totals["activeConflicts"]),
            "Use: archive, audit, and downstream machine reading".to_string(),
        ];
    }
    let totals = decision_totals(&payload);
    vec![
        "Scope: decision export".to_string(),
        format!(
            "Contains {} decided nodes, {} decided checklist items, {} selected L4 groups",
            totals["nodes"], totals["checklist"], totals["groups"]
        ),
        format!("Not applicable nodes: {}", totals["notApplicable"]),
        format!(
            "Pending overview: {} nodes and {} items",
            totals["pendingNodes"], totals["pendingItems"]
        ),
        format!("Gameplay systems: {gameplay_count}{appendix}"),
        format!("Active conflicts: {}", totals["activeConflicts"]),
        "Use: reading, review, and continued design completion".to_string(),
    ]
}

pub fn render_markdown(payload: &Value, include_gameplay_global_view: bool) -> String {
    let mut lines = Vec::new();
    lines.push(format!("# {}", str_field(payload, "projectName")));
    lines.push(String::new());
    lines.push("## Export Summary".to_string());
    lines.push(String::new());
    lines.push(format!(
        "- Exported at: `{}`",
        str_field(payload, "exportedAt")
    ));
    lines.extend(markdown_quality_lines(payload));
    lines.push(String::new());
    lines.push("## Project Profile".to_string());
    lines.push(String::new());
    for (key, value) in object(payload, "profile") {
        lines.push(format!("- {}: {}", humanize_id(&key), value_label(&value)));
    }
    append_markdown_gameplay_systems(&mut lines, payload);
    lines.push(String::new());
    lines.push("## Domain Overview".to_string());
    lines.push(String::new());
    lines.push("| Domain | Node Coverage | Checklist Coverage | Decided Nodes |".to_string());
    lines.push("| --- | ---: | ---: | ---: |".to_string());
    for domain in list(payload, "domains") {
        let domain_info = domain.get("domain").unwrap_or(&Value::Null);
        let coverage = domain.get("coverage").unwrap_or(&Value::Null);
        let decided = decision_nodes(&domain).len();
        lines.push(format!(
            "| {} | {}% | {}% | {} |",
            str_field(domain_info, "name"),
            number_text(coverage.get("nodePercent")),
            number_text(coverage.get("checklistPercent")),
            decided
        ));
    }
    lines.push(String::new());
    lines.push("## Pending Overview".to_string());
    let mut pending_lines = Vec::new();
    for domain in list(payload, "domains") {
        let (nodes, items) = pending_counts(&domain);
        if nodes > 0 || items > 0 {
            pending_lines.push(format!(
                "- {}: pending nodes {}, pending items {}",
                str_field(domain.get("domain").unwrap_or(&Value::Null), "name"),
                nodes,
                items
            ));
        }
    }
    if pending_lines.is_empty() {
        lines.push("No pending items.".to_string());
    } else {
        lines.extend(pending_lines);
    }
    lines.push(String::new());
    lines.push("## Decided Content".to_string());
    let mut has_decisions = false;
    for domain in list(payload, "domains") {
        let nodes = decision_nodes(&domain);
        if nodes.is_empty() {
            continue;
        }
        has_decisions = true;
        let domain_info = domain.get("domain").unwrap_or(&Value::Null);
        lines.push(String::new());
        lines.push(format!("### {}", str_field(domain_info, "name")));
        for node in nodes {
            lines.push(String::new());
            lines.push(format!(
                "#### {} - {}",
                str_field(&node, "name"),
                str_field(&node, "decisionState")
            ));
            lines.push(format!("- id: `{}`", str_field(&node, "id")));
            if !str_field(&node, "designNote").is_empty() {
                lines.push(format!("- design: {}", str_field(&node, "designNote")));
            }
            if !str_field(&node, "riskNote").is_empty() {
                lines.push(format!("- risk: {}", str_field(&node, "riskNote")));
            }
            for item in decision_items(&node) {
                append_markdown_decision_item(&mut lines, &item);
            }
            append_markdown_design_entities(&mut lines, &node);
        }
    }
    if !has_decisions {
        lines.push("No decided content.".to_string());
    }
    if include_gameplay_global_view {
        append_markdown_gameplay_global_view(&mut lines, payload);
    }
    append_markdown_quality_violations(&mut lines, payload);
    append_markdown_cross_layer_violations(&mut lines, payload);
    lines.join("\n")
}

pub fn render_text(payload: &Value, include_gameplay_global_view: bool) -> String {
    let mut lines = Vec::new();
    let name = str_field(payload, "projectName");
    lines.push(name.clone());
    lines.push("=".repeat(name.chars().count().max(8)));
    lines.push(String::new());
    lines.push(format!("Exported at: {}", str_field(payload, "exportedAt")));
    lines.extend(text_quality_lines(payload));
    lines.push(String::new());
    lines.push("Project profile:".to_string());
    for (key, value) in object(payload, "profile") {
        lines.push(format!("  {}: {}", humanize_id(&key), value_label(&value)));
    }
    append_text_gameplay_systems(&mut lines, payload);
    lines.push(String::new());
    lines.push("Domains:".to_string());
    for domain in list(payload, "domains") {
        let domain_info = domain.get("domain").unwrap_or(&Value::Null);
        let coverage = domain.get("coverage").unwrap_or(&Value::Null);
        lines.push(format!(
            "  {}: node {}%, checklist {}%, decided {}",
            str_field(domain_info, "name"),
            number_text(coverage.get("nodePercent")),
            number_text(coverage.get("checklistPercent")),
            decision_nodes(&domain).len()
        ));
    }
    lines.push(String::new());
    lines.push("Decided content:".to_string());
    for domain in list(payload, "domains") {
        for node in decision_nodes(&domain) {
            lines.push(format!(
                "- {} ({})",
                str_field(&node, "name"),
                str_field(&node, "id")
            ));
            if !str_field(&node, "designNote").is_empty() {
                lines.push(format!("  design: {}", str_field(&node, "designNote")));
            }
            for item in decision_items(&node) {
                append_text_decision_item(&mut lines, &item);
            }
        }
    }
    if include_gameplay_global_view {
        append_text_gameplay_global_view(&mut lines, payload);
    }
    append_text_quality_violations(&mut lines, payload);
    append_text_cross_layer_violations(&mut lines, payload);
    lines.join("\n")
}

pub fn render_prompt(payload: &Value, include_gameplay_global_view: bool) -> String {
    let mut lines = Vec::new();
    lines.push("Continue the design from the confirmed decisions below. Preserve existing decisions and only fill missing areas.".to_string());
    lines.push(String::new());
    lines.push(format!("Project: {}", str_field(payload, "projectName")));
    lines.push(format!("Exported at: {}", str_field(payload, "exportedAt")));
    lines.extend(text_quality_lines(payload));
    if include_gameplay_global_view {
        append_text_gameplay_global_view(&mut lines, payload);
    }
    lines.push(String::new());
    lines.push("Suggested next clarifications:".to_string());
    let mut missing_count = 0usize;
    for domain in list(payload, "domains") {
        for node in list(&domain, "nodes") {
            if node_has_decision(&node) || str_field(&node, "decisionState") == "not_applicable" {
                continue;
            }
            for item in list(&node, "checklist") {
                if !bool_field(&item, "done") {
                    missing_count += 1;
                    lines.push(format!(
                        "- {} / {} / {}",
                        str_field(domain.get("domain").unwrap_or(&Value::Null), "name"),
                        str_field(&node, "name"),
                        str_field(&item, "label")
                    ));
                }
                if missing_count >= 30 {
                    break;
                }
            }
            if missing_count >= 30 {
                break;
            }
        }
        if missing_count >= 30 {
            lines.push("- More pending items exist; use full export for the rest.".to_string());
            break;
        }
    }
    if missing_count == 0 {
        lines.push(
            "- No obvious pending items. Continue with risks, conflicts, and detail expansion."
                .to_string(),
        );
    }
    lines.push(String::new());
    lines.push("Confirmed decision summary:".to_string());
    lines.push(render_text(payload, include_gameplay_global_view));
    lines.join("\n")
}

pub fn render_archive_markdown(payload: &Value, include_gameplay_global_view: bool) -> String {
    let mut text = render_markdown(payload, include_gameplay_global_view);
    text.push_str("\n\n## Machine Metadata\n\n");
    text.push_str(&format!(
        "- taxonomy_hash: `{}`\n",
        payload["documentMetadata"]["taxonomy_hash"]
            .as_str()
            .unwrap_or("")
    ));
    text.push_str(&format!(
        "- coverage: `{}`\n",
        serde_json::to_string(payload.get("coverageMetrics").unwrap_or(&Value::Null))
            .unwrap_or_else(|_| "{}".to_string())
    ));
    text
}

pub fn render_archive_text(payload: &Value, include_gameplay_global_view: bool) -> String {
    let mut text = render_text(payload, include_gameplay_global_view);
    text.push_str("\n\nMachine metadata:\n");
    text.push_str(&format!(
        "  taxonomy_hash: {}\n",
        payload["documentMetadata"]["taxonomy_hash"]
            .as_str()
            .unwrap_or("")
    ));
    text
}

pub fn profile_payload(payload: &Value) -> Value {
    let metadata = payload.get("documentMetadata").unwrap_or(&Value::Null);
    json!({
        "schemaVersion": EXPORT_SCHEMA_VERSION,
        "projectName": str_field(payload, "projectName"),
        "exportedAt": str_field(payload, "exportedAt"),
        "document_type": str_field(metadata, "document_type"),
        "taxonomy_version": str_field(metadata, "taxonomy_version"),
        "taxonomy_hash": str_field(metadata, "taxonomy_hash"),
        "profile": payload.get("profile").cloned().unwrap_or_else(|| json!({})),
        "profileDisplay": payload.get("profileDisplay").cloned().unwrap_or_else(|| json!({})),
        "case_genre": metadata.get("case_genre").cloned().unwrap_or_else(|| json!([])),
        "case_applicability": metadata.get("case_applicability").cloned().unwrap_or_else(|| json!([])),
        "not_applicable_to": metadata.get("not_applicable_to").cloned().unwrap_or_else(|| json!([])),
    })
}

pub fn coverage_payload(payload: &Value) -> Value {
    let metadata = payload.get("documentMetadata").unwrap_or(&Value::Null);
    json!({
        "schemaVersion": EXPORT_SCHEMA_VERSION,
        "projectName": str_field(payload, "projectName"),
        "exportedAt": str_field(payload, "exportedAt"),
        "document_type": str_field(metadata, "document_type"),
        "taxonomy_version": str_field(metadata, "taxonomy_version"),
        "taxonomy_hash": str_field(metadata, "taxonomy_hash"),
        "coverageMetrics": payload.get("coverageMetrics").cloned().unwrap_or_else(|| json!({})),
        "projectCoverage": payload.get("projectCoverage").cloned().unwrap_or_else(|| json!({})),
        "structureCoverage": payload.get("structureCoverage").cloned().unwrap_or_else(|| json!({})),
        "concretenessCoverage": payload.get("concretenessCoverage").cloned().unwrap_or_else(|| json!({})),
        "consistencyScore": payload.get("consistencyScore").cloned().unwrap_or_else(|| json!({})),
        "qualityBadge": str_field(payload, "qualityBadge"),
        "qualityCriticalCount": payload.get("qualityCriticalCount").cloned().unwrap_or(json!(0)),
    })
}

pub fn write_archive_sidecars(payload: &Value, target_dir: &Path) -> AdmResult<Vec<PathBuf>> {
    let base = safe_file_name(&str_field(payload, "projectName"), "commercial-game-design");
    let sidecars = vec![
        (format!("{base}.full.json"), payload.clone()),
        (format!("{base}.profile.json"), profile_payload(payload)),
        (format!("{base}.coverage.json"), coverage_payload(payload)),
    ];
    let mut written = Vec::new();
    for (file_name, data) in sidecars {
        written.push(io::write_json(&target_dir.join(file_name), &data)?);
    }
    Ok(written)
}

pub fn write_export(
    engine: &DesignEngineService,
    project_state: &ProjectState,
    target_dir: &Path,
    export_format: &str,
    export_scope: &str,
    include_gameplay_global_view: bool,
) -> AdmResult<PathBuf> {
    let payload = build_payload(engine, project_state);
    fs::create_dir_all(target_dir)?;
    let fmt = export_format.to_ascii_lowercase();
    let scope = export_scope.to_ascii_lowercase();
    let suffix = match fmt.as_str() {
        "markdown" => "md",
        "json" => "json",
        "txt" => "txt",
        "text" => "text",
        "prompt" => "prompt.txt",
        _ => "txt",
    };
    let scope_suffix = if scope == "archive" && fmt != "json" {
        "full"
    } else {
        "decision"
    };
    let base = safe_file_name(
        &str_field(&payload, "projectName"),
        "commercial-game-design",
    );
    let path = if fmt == "json" {
        target_dir.join(format!("{base}.{suffix}"))
    } else {
        target_dir.join(format!("{base}.{scope_suffix}.{suffix}"))
    };
    match fmt.as_str() {
        "json" => {
            io::write_json(&path, &payload)?;
        }
        "markdown" => {
            let text = if scope == "archive" {
                render_archive_markdown(&payload, include_gameplay_global_view)
            } else {
                render_markdown(&payload, include_gameplay_global_view)
            };
            io::write_text(&path, &text)?;
            if scope == "archive" {
                write_archive_sidecars(&payload, target_dir)?;
            }
        }
        "prompt" => {
            let text = if scope == "archive" {
                render_archive_text(&payload, include_gameplay_global_view)
            } else {
                render_prompt(&payload, include_gameplay_global_view)
            };
            io::write_text(&path, &text)?;
        }
        _ => {
            let text = if scope == "archive" {
                render_archive_text(&payload, include_gameplay_global_view)
            } else {
                render_text(&payload, include_gameplay_global_view)
            };
            io::write_text(&path, &text)?;
        }
    }
    Ok(path)
}

pub fn build_structured_decisions(specs: &[DesignNodeSpec], project_state: &ProjectState) -> Value {
    let mut decisions = Vec::new();
    let mut confirmed_text = Vec::new();
    for spec in specs {
        let node_state = project_state
            .nodes
            .get(&spec.node_id)
            .cloned()
            .unwrap_or_default();
        let (selected, primary) = selected_option_records(spec, &node_state);
        for option in &selected {
            let provenance = option.get("optionProvenance").unwrap_or(&Value::Null);
            if matches!(
                provenance.get("source").and_then(Value::as_str),
                Some("user_selected" | "user_confirmed_ai")
            ) && provenance
                .get("confirmed")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                confirmed_text.push(format!(
                    "{} {}",
                    spec.name,
                    option.get("label").and_then(Value::as_str).unwrap_or("")
                ));
            }
        }
        decisions.push(json!({
            "node_id": spec.node_id,
            "domain": spec.domain_id,
            "priority": "",
            "requirement_level": "",
            "contract_targets": [],
            "decision_state": effective_state_name(&node_state),
            "checklistOptions": node_state.checklist_options,
            "optionProvenance": node_state.option_provenance,
            "selected_options": selected,
            "primary_options": primary,
            "notes": {
                "designNote": node_state.design_note,
                "riskNote": node_state.risk_note,
                "notApplicableReason": node_state.not_applicable_reason,
            },
            "design_entities": node_state.design_entities,
            "conflicts": [],
            "source_refs": [spec.node_id],
        }));
    }
    json!({
        "schema_version": "1.0",
        "source": "structured/decisions.json",
        "project_id": project_state.project_name,
        "decisions": decisions,
        "confirmed_option_text": confirmed_text,
    })
}

pub fn build_structured_design_entities(project_state: &ProjectState) -> Value {
    let nodes = project_state
        .nodes
        .iter()
        .filter(|(_, node)| !node.design_entities.is_empty())
        .map(|(node_id, node)| {
            json!({
                "node_id": node_id,
                "entities": node.design_entities,
                "source_refs": [node_id],
            })
        })
        .collect::<Vec<_>>();
    let entity_count = nodes
        .iter()
        .filter_map(|node| node.get("entities").and_then(Value::as_array))
        .map(Vec::len)
        .sum::<usize>();
    json!({
        "schema_version": "1.0",
        "source": "structured/design_entities.json",
        "project_id": project_state.project_name,
        "node_count": nodes.len(),
        "entity_count": entity_count,
        "nodes": nodes,
    })
}

pub fn build_traceability(contracts: &Value, decisions: &Value) -> Value {
    let mut decision_refs = BTreeSet::new();
    for decision in list(decisions, "decisions") {
        for source_ref in value_list(decision.get("source_refs")) {
            decision_refs.insert(source_ref);
        }
        for option in list(&decision, "selected_options") {
            for source_ref in value_list(option.get("source_refs")) {
                decision_refs.insert(source_ref);
            }
        }
    }
    let mut field_refs = Vec::new();
    let mut mismatches = Vec::new();
    for (contract_id, payload) in object(contracts, "") {
        if contract_id == "design_completeness_report" || !payload.is_object() {
            continue;
        }
        for source_ref in value_list(payload.get("source_refs")) {
            if source_ref != "decisions.json" && !decision_refs.contains(&source_ref) {
                mismatches.push(json!({
                    "code": "TRACEABILITY_MISMATCH",
                    "contract_id": contract_id,
                    "source_ref": source_ref,
                }));
            }
        }
        field_refs.push(json!({
            "contract_id": contract_id,
            "path": "$.source_refs",
            "source_refs": payload.get("source_refs").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    json!({
        "schema_version": "1.0",
        "field_refs": field_refs,
        "mismatches": mismatches,
        "valid": mismatches.is_empty(),
    })
}

pub fn write_structured_handoff(
    package_dir: &Path,
    engine: &DesignEngineService,
    project_state: &ProjectState,
) -> AdmResult<Value> {
    write_structured_handoff_with_locale(
        package_dir,
        engine,
        project_state,
        ArtifactLocale::default(),
    )
}

pub fn write_structured_handoff_with_locale(
    package_dir: &Path,
    engine: &DesignEngineService,
    project_state: &ProjectState,
    artifact_locale: ArtifactLocale,
) -> AdmResult<Value> {
    let structured_dir = package_dir.join("structured");
    let contracts_dir = structured_dir.join("playable_contract_candidates");
    fs::create_dir_all(&contracts_dir)?;

    let mut decisions = build_structured_decisions(engine.specs(), project_state);
    decisions["artifact_locale"] = json!(artifact_locale);
    let mut design_entities = build_structured_design_entities(project_state);
    design_entities["artifact_locale"] = json!(artifact_locale);
    let mut profile = profile_handoff_payload(project_state);
    profile["artifact_locale"] = json!(artifact_locale);
    let confirmed = decisions
        .get("confirmed_option_text")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    let mut archetype = build_archetype_requirements_with_locale(
        &profile,
        &confirmed,
        &ArchetypeCatalog::builtin(),
        artifact_locale,
    );
    archetype["source"] = json!("archetype_requirements.json");
    archetype["artifact_locale"] = json!(artifact_locale);
    let contracts = build_playable_contract_bundle_from_decisions_with_locale(
        &decisions,
        &profile,
        &archetype,
        artifact_locale,
    );
    let mut traceability = build_traceability(&contracts, &decisions);
    traceability["artifact_locale"] = json!(artifact_locale);

    io::write_json(&structured_dir.join("decisions.json"), &decisions)?;
    io::write_json(
        &structured_dir.join("design_entities.json"),
        &design_entities,
    )?;
    io::write_json(&structured_dir.join("profile.json"), &profile)?;
    io::write_json(
        &structured_dir.join("archetype_requirements.json"),
        &archetype,
    )?;
    io::write_json(&structured_dir.join("traceability.json"), &traceability)?;
    for (contract_id, payload) in object(&contracts, "") {
        if matches!(
            contract_id.as_str(),
            "artifact_locale" | "design_completeness_report"
        ) {
            continue;
        }
        io::write_json(&contracts_dir.join(format!("{contract_id}.json")), &payload)?;
    }

    let completeness = contracts
        .get("design_completeness_report")
        .and_then(|report| report.get("playable_completeness"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut blocking_issues = list(&completeness, "blocking_issues");
    if !traceability
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        blocking_issues.extend(list(&traceability, "mismatches"));
    }
    let mut review_items = list(&completeness, "review_items");
    review_items.extend(list(&archetype, "warnings"));
    let validation = json!({
        "status": if blocking_issues.is_empty() { "passed" } else { "blocked" },
        "blocking_issues": blocking_issues,
        "review_items": review_items,
    });
    let required = value_set(archetype.get("required_contracts"));
    let manifest = json!({
        "schema_version": "1.0",
        "handoff_type": "structured_design_handoff",
        "generated_at": io::now_iso(),
        "project_id": project_state.project_name,
        "artifact_locale": artifact_locale,
        "package_manifest_path": "../package_manifest.json",
        "structured_dir": "structured/",
        "decisions_path": "structured/decisions.json",
        "design_entities_path": "structured/design_entities.json",
        "profile_path": "structured/profile.json",
        "archetype_path": "structured/archetype_requirements.json",
        "traceability_path": "structured/traceability.json",
        "contracts": PLAYABLE_CONTRACT_REGISTRY.iter().map(|(contract_id, schema)| {
            json!({
                "contract_id": contract_id,
                "path": format!("structured/playable_contract_candidates/{contract_id}.json"),
                "schema": schema,
                "schema_version": contracts.get(*contract_id).and_then(|item| item.get("schema_version")).and_then(Value::as_str).unwrap_or(""),
                "required": required.contains(*contract_id),
            })
        }).collect::<Vec<_>>(),
        "validation": validation,
    });
    io::write_json(&structured_dir.join("handoff_manifest.json"), &manifest)?;
    Ok(manifest)
}

pub fn export_concept_package_from_state(
    target_dir: &Path,
    engine: &DesignEngineService,
    project_state: &ProjectState,
) -> AdmResult<Value> {
    export_concept_package_from_state_with_locale(
        target_dir,
        engine,
        project_state,
        ArtifactLocale::default(),
    )
}

pub fn export_concept_package_from_state_with_locale(
    target_dir: &Path,
    engine: &DesignEngineService,
    project_state: &ProjectState,
    artifact_locale: ArtifactLocale,
) -> AdmResult<Value> {
    fs::create_dir_all(target_dir)?;
    let summary = design_summary(engine, project_state);
    let packages = [
        (
            "Concept",
            "devflow_Concept_v2",
            "concept.md",
            concept_markdown(project_state, artifact_locale),
        ),
        (
            "GameplayFramework",
            "devflow_GameplayFramework_v2",
            "framework.md",
            framework_markdown(project_state, artifact_locale),
        ),
        (
            "Design",
            "devflow_Design_v2",
            "design.md",
            design_markdown(engine, project_state, artifact_locale),
        ),
    ];
    let mut results = Map::new();
    for (source_type, package_name, attachment_name, markdown) in packages {
        let package_dir = target_dir.join(package_name);
        write_layer_package(
            &package_dir,
            source_type,
            attachment_name,
            &markdown,
            &summary,
            project_state,
            artifact_locale,
        )?;
        results.insert(
            source_type.to_string(),
            Value::String(package_dir.to_string_lossy().to_string()),
        );
    }
    let design_dir = target_dir.join("devflow_Design_v2");
    let structured_handoff =
        write_structured_handoff_with_locale(&design_dir, engine, project_state, artifact_locale)?;
    Ok(json!({
        "package_dir": target_dir.join("devflow_Concept_v2").to_string_lossy().to_string(),
        "packages": results,
        "structured_handoff": structured_handoff,
        "design_summary": summary,
        "artifact_locale": artifact_locale,
    }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructuredContextError {
    pub issue: Value,
}

impl std::fmt::Display for StructuredContextError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", str_field(&self.issue, "message"))
    }
}

impl std::error::Error for StructuredContextError {}

#[derive(Debug, Clone, PartialEq)]
pub struct StructuredDesignContext {
    pub output_base_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub handoff_dir: Option<PathBuf>,
    pub warnings: Vec<Value>,
}

impl StructuredDesignContext {
    pub fn from_output_base(output_base_dir: impl AsRef<Path>) -> Self {
        let root = output_base_dir.as_ref().to_path_buf();
        let artifacts = resolve_artifacts_dir(&root);
        let handoff_dir = latest_structured_handoff_package_for_root(&root)
            .map(|package| package.join("structured"));
        Self {
            output_base_dir: root,
            artifacts_dir: artifacts,
            handoff_dir,
            warnings: Vec::new(),
        }
    }

    pub fn from_draft_session(draft_session_id: &str, workspace_root: impl AsRef<Path>) -> Self {
        Self::from_output_base(
            workspace_root
                .as_ref()
                .join("drafts")
                .join(draft_session_id),
        )
    }

    pub fn stage_dir(&self, stage: u8) -> PathBuf {
        self.artifacts_dir.join(format!("stage_{stage:02}"))
    }

    pub fn stage_artifact(&self, stage: u8, artifact_name: &str) -> PathBuf {
        self.stage_dir(stage).join(artifact_name)
    }

    pub fn load_stage_artifact(&self, stage: u8, artifact_name: &str) -> Value {
        let payload = io::read_json(&self.stage_artifact(stage, artifact_name), json!({}));
        if payload.is_object() {
            payload
        } else {
            json!({})
        }
    }

    pub fn require_handoff(&self, name: &str) -> Result<Value, StructuredContextError> {
        let Some(handoff_dir) = &self.handoff_dir else {
            return Err(StructuredContextError {
                issue: missing_issue(name, "structured/handoff_manifest.json", "D4"),
            });
        };
        let filename = if name.ends_with(".json") {
            name.to_string()
        } else {
            format!("{name}.json")
        };
        let path = handoff_dir.join(filename);
        let payload = io::read_json(&path, json!({}));
        if payload.is_object() && !payload.as_object().unwrap().is_empty() {
            Ok(payload)
        } else {
            Err(StructuredContextError {
                issue: missing_issue(name, &path.to_string_lossy(), "D4"),
            })
        }
    }

    pub fn require_playable_contract(
        &mut self,
        contract_id: &str,
        required_by_step: &str,
    ) -> Result<Value, StructuredContextError> {
        let payload = self.optional_playable_contract(contract_id);
        if payload.is_object() && !payload.as_object().unwrap().is_empty() {
            return Ok(payload);
        }
        Err(StructuredContextError {
            issue: missing_issue(
                contract_id,
                &self.contract_artifact_hint(contract_id),
                required_by_step,
            ),
        })
    }

    pub fn optional_playable_contract(&mut self, contract_id: &str) -> Value {
        let filename = format!("{contract_id}.json");
        let stage2_path = self.stage_dir(2).join("playable_contracts").join(&filename);
        let payload = io::read_json(&stage2_path, json!({}));
        if payload.is_object() && !payload.as_object().unwrap().is_empty() {
            return payload;
        }
        if let Some(handoff_dir) = &self.handoff_dir {
            let candidate_path = handoff_dir
                .join("playable_contract_candidates")
                .join(&filename);
            let payload = io::read_json(&candidate_path, json!({}));
            if payload.is_object() && !payload.as_object().unwrap().is_empty() {
                self.warnings.push(json!({
                    "code": "USING_D4_CONTRACT_CANDIDATE",
                    "contract_id": contract_id,
                    "artifact_path": candidate_path.to_string_lossy(),
                }));
                return payload;
            }
        }
        json!({})
    }

    pub fn trace(&self, contract_id: &str, field_path: &str) -> Vec<Value> {
        let Some(handoff_dir) = &self.handoff_dir else {
            return Vec::new();
        };
        let traceability = io::read_json(&handoff_dir.join("traceability.json"), json!({}));
        list(&traceability, "field_refs")
            .into_iter()
            .filter(|item| {
                str_field(item, "contract_id") == contract_id
                    && str_field(item, "path") == field_path
            })
            .collect()
    }

    fn contract_artifact_hint(&self, contract_id: &str) -> String {
        format!(
            "{} or structured/playable_contract_candidates/{contract_id}.json",
            self.stage_dir(2)
                .join("playable_contracts")
                .join(format!("{contract_id}.json"))
                .to_string_lossy()
        )
    }
}

pub fn resolve_artifacts_dir(root: &Path) -> PathBuf {
    if root.join("outputs").join("artifacts").exists() {
        return root.join("outputs").join("artifacts");
    }
    if root.join("artifacts").exists() {
        return root.join("artifacts");
    }
    if root.file_name().and_then(|name| name.to_str()) == Some("artifacts") {
        return root.to_path_buf();
    }
    root.join("outputs").join("artifacts")
}

pub fn latest_structured_handoff_package_for_root(root: &Path) -> Option<PathBuf> {
    let source_dir = root.join("source_artifacts");
    if !source_dir.exists() {
        return None;
    }
    let mut candidates = fs::read_dir(source_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            path.join("structured")
                .join("handoff_manifest.json")
                .exists()
        })
        .filter(|path| package_matches_type(path, "Design") || name_starts(path, "devflow_Design_"))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    candidates.pop()
}

pub fn package_matches_type(package_dir: &Path, source_type: &str) -> bool {
    let manifest = io::read_json(&package_dir.join("package_manifest.json"), json!({}));
    str_field(&manifest, "package_type") == source_type
        || str_field(&manifest, "source_id") == source_type
        || value_list(manifest.get("source_ids"))
            .iter()
            .any(|item| item == source_type)
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrossLayerRuleSet {
    pub path: Option<PathBuf>,
    pub payload: Value,
    pub rules: Vec<Value>,
}

impl CrossLayerRuleSet {
    pub fn from_payload(payload: Value) -> Self {
        let rules = list(&payload, "rules");
        Self {
            path: None,
            payload,
            rules,
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let payload = load_cross_layer_rules(Some(&path));
        let rules = list(&payload, "rules");
        Self {
            path: Some(path),
            payload,
            rules,
        }
    }

    pub fn lint(&self, specs: &[DesignNodeSpec], project_state: &ProjectState) -> Vec<Value> {
        let selected = selected_option_contexts(specs, project_state);
        let mut violations = Vec::new();
        for rule in &self.rules {
            if !rule_matches_profile(rule, &project_state.profile) {
                continue;
            }
            let mut hit_options = Vec::new();
            for option_id in value_list(rule.get("forbidsOptionId")) {
                if let Some(items) = selected.get(&option_id) {
                    hit_options.extend(items.clone());
                }
            }
            if hit_options.is_empty() {
                continue;
            }
            let hit_ids = hit_options
                .iter()
                .filter_map(|item| item.get("optionId").and_then(Value::as_str))
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(Value::String)
                .collect::<Vec<_>>();
            violations.push(json!({
                "ruleId": str_field(rule, "id"),
                "severity": rule.get("severity").and_then(Value::as_str).unwrap_or("WARNING"),
                "reason": str_field(rule, "reason"),
                "condition": rule.get("if").cloned().unwrap_or_else(|| json!({})),
                "hitOptionIds": hit_ids,
                "hitOptions": hit_options,
            }));
        }
        violations
    }
}

pub fn load_cross_layer_rules(path: Option<&Path>) -> Value {
    path.map(|target| io::read_json(target, json!({"schemaVersion":"1.0","rules":[]})))
        .unwrap_or_else(|| json!({"schemaVersion":"1.0","rules":[]}))
}

pub fn rule_matches_profile(rule: &Value, profile: &BTreeMap<String, Value>) -> bool {
    let conditions = rule
        .get("if")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, expected) in conditions {
        let Some(field) = key.strip_prefix("profile.") else {
            return false;
        };
        let actual = profile
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if !value_matches(actual, &expected) {
            return false;
        }
    }
    true
}

pub fn value_matches(actual: &str, expected_values: &Value) -> bool {
    match expected_values {
        Value::Array(items) => items.iter().any(|item| item.as_str() == Some(actual)),
        Value::String(value) => value == actual,
        other => other.to_string().trim_matches('"') == actual,
    }
}

pub fn selected_option_contexts(
    specs: &[DesignNodeSpec],
    project_state: &ProjectState,
) -> BTreeMap<String, Vec<Value>> {
    let mut selected = BTreeMap::<String, Vec<Value>>::new();
    for spec in specs {
        let node_state = project_state
            .nodes
            .get(&spec.node_id)
            .cloned()
            .unwrap_or_default();
        for item in &spec.checklist {
            let Some(item_options) = node_state.checklist_options.get(&item.item_id) else {
                continue;
            };
            for group in &item.option_groups {
                let Some(group_state) = item_options.get(&group.group_id) else {
                    continue;
                };
                for option_id in &group_state.selected {
                    selected.entry(option_id.clone()).or_default().push(json!({
                        "domainId": spec.domain_id,
                        "domainName": humanize_id(&spec.domain_id),
                        "nodeId": spec.node_id,
                        "nodeName": spec.name,
                        "itemId": item.item_id,
                        "itemLabel": item.label,
                        "groupId": group.group_id,
                        "groupLabel": humanize_id(&group.group_id),
                        "optionId": option_id,
                        "optionLabel": humanize_id(option_id),
                    }));
                }
            }
        }
    }
    selected
}

fn build_document_metadata(
    project_state: &ProjectState,
    taxonomy_hash: &str,
    exported_at: &str,
) -> Value {
    let document_type = infer_document_type(project_state);
    json!({
        "document_type": document_type,
        "document_version": DOCUMENT_VERSION,
        "taxonomy_version": TAXONOMY_VERSION,
        "taxonomy_hash": taxonomy_hash,
        "case_name": project_state.project_name,
        "case_genre": profile_case_genre(&project_state.profile),
        "case_applicability": profile_applicability(&project_state.profile),
        "not_applicable_to": profile_not_applicable(&project_state.profile),
        "authoring_source": if document_type == "template_reverse_inferred" { "reverse_inference_from_public_info" } else { "internal_design" },
        "authoring_confidence_overall": if document_type == "template_reverse_inferred" { "mid" } else { "high" },
        "exported_at": exported_at,
    })
}

fn build_taxonomy_payload(specs: &[DesignNodeSpec], taxonomy_hash: &str) -> Value {
    json!({
        "taxonomy_version": TAXONOMY_VERSION,
        "taxonomy_hash": taxonomy_hash,
        "previous_version": Value::Null,
        "deprecated_codes": [],
        "renamed_codes": [],
        "added_codes": [],
        "naming_convention": {
            "domain": "snake_case",
            "decision_id": "snake_case",
            "option_code": "snake_case_id",
            "profile_value": "snake_case",
        },
        "derives_to_stage": specs.iter().map(|spec| (spec.node_id.clone(), json!(derives_to_stage(&spec.domain_id)))).collect::<Map<_, _>>(),
        "decision_dependency_graph": {"edges": []},
        "option_compatibility_matrix": {"incompatibilities": [], "co_requirements": []},
    })
}

fn build_domain_payloads(engine: &DesignEngineService, project_state: &ProjectState) -> Vec<Value> {
    let mut by_domain = BTreeMap::<String, Vec<&DesignNodeSpec>>::new();
    for spec in engine.specs() {
        by_domain
            .entry(spec.domain_id.clone())
            .or_default()
            .push(spec);
    }
    by_domain
        .into_iter()
        .map(|(domain_id, specs)| {
            let nodes = specs.iter().map(|spec| build_node_payload(spec, project_state)).collect::<Vec<_>>();
            let coverage = domain_coverage(&nodes);
            json!({
                "domain": {
                    "id": domain_id,
                    "name": humanize_id(&domain_id),
                    "description": format!("{} domain contains {} design nodes.", humanize_id(&domain_id), nodes.len()),
                },
                "coverage": coverage,
                "nodes": nodes,
            })
        })
        .collect()
}

fn build_node_payload(spec: &DesignNodeSpec, project_state: &ProjectState) -> Value {
    let node_state = project_state
        .nodes
        .get(&spec.node_id)
        .cloned()
        .unwrap_or_default();
    let decision_metadata = build_decision_metadata(spec, &node_state, project_state);
    let checklist = spec
        .checklist
        .iter()
        .map(|item| {
            let groups = item
                .option_groups
                .iter()
                .map(|group| {
                    let group_state = node_state
                        .checklist_options
                        .get(&item.item_id)
                        .and_then(|groups| groups.get(&group.group_id))
                        .cloned()
                        .unwrap_or_default();
                    let provenance = node_state
                        .option_provenance
                        .get(&item.item_id)
                        .and_then(|groups| groups.get(&group.group_id))
                        .cloned()
                        .unwrap_or_default();
                    json!({
                        "id": group.group_id,
                        "label": humanize_id(&group.group_id),
                        "selectionMode": group.selection_mode,
                        "required": !group.options.is_empty(),
                        "allowPrimary": group.allow_primary,
                        "selected": group_state.selected,
                        "selectedLabels": group_state.selected.iter().map(|id| humanize_id(id)).collect::<Vec<_>>(),
                        "primary": group_state.primary,
                        "primaryLabel": if group_state.primary.is_empty() { String::new() } else { humanize_id(&group_state.primary) },
                        "activeConflicts": [],
                        "options": group.options.iter().map(|option_id| {
                            let selected = group_state.selected.iter().any(|selected| selected == option_id);
                            json!({
                                "id": option_id,
                                "label": humanize_id(option_id),
                                "description": "",
                                "outputKey": option_id,
                                "selected": selected,
                                "primary": group_state.primary == *option_id,
                                "strength": if group.options.is_empty() { "derived" } else { "soft_preference" },
                                "treatment": if selected { "selected" } else if !bool_value(node_state.checklist.get(&item.item_id)) { "deferred" } else { "not_evaluated" },
                                "selectionProvenance": if selected { Some(selection_provenance(&decision_metadata)) } else { None },
                                "optionProvenance": if selected { provenance.get(option_id).and_then(option_provenance_value) } else { None },
                            })
                        }).collect::<Vec<_>>(),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "id": item.item_id,
                "label": item.label,
                "description": "",
                "outputKey": item.item_id,
                "templateRef": "",
                "path": format!("{}.{}.{}", spec.domain_id, spec.node_id, item.item_id),
                "done": bool_value(node_state.checklist.get(&item.item_id)),
                "optionGroups": groups,
                "optionRelations": [],
                "activeOptionConflicts": [],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "id": spec.node_id,
        "name": spec.name,
        "description": spec.description,
        "requires": [],
        "unlocks": [],
        "designEntities": node_state.design_entities,
        "entityValidationErrors": node_state.entity_validation_errors,
        "decisionState": node_state.decision_state.as_str(),
        "designNote": node_state.design_note,
        "riskNote": node_state.risk_note,
        "notApplicableReason": node_state.not_applicable_reason,
        "decisionMetadata": decision_metadata,
        "selection_state": decision_metadata["selection_state"],
        "confidence": decision_metadata["confidence"],
        "derives_to_stage": decision_metadata["derives_to_stage"],
        "depends_on_decisions": decision_metadata["depends_on_decisions"],
        "selection_rationale": decision_metadata["selection_rationale"],
        "checklist": checklist,
    })
}

fn build_decision_metadata(
    spec: &DesignNodeSpec,
    node_state: &NodeState,
    project_state: &ProjectState,
) -> Value {
    let selection_state = selection_state_for_node(node_state, project_state);
    json!({
        "id": spec.node_id,
        "selection_state": selection_state,
        "confidence": confidence_for_node(node_state, &selection_state),
        "derives_to_stage": derives_to_stage(&spec.domain_id),
        "depends_on_decisions": [],
        "selection_rationale": selection_rationale(node_state, &selection_state),
    })
}

fn build_coverage_metrics(view: &crate::DesignWorkbenchView, domains: &[Value]) -> Value {
    let totals = count_payload_items(domains);
    json!({
        "structural_coverage": ratio(view.project_coverage.done_nodes, view.project_coverage.total_nodes),
        "checklist_tick_coverage": ratio(view.project_coverage.done_checklist, view.project_coverage.total_checklist),
        "selection_state_resolution": ratio(
            totals.get("resolvedNodes").and_then(Value::as_u64).unwrap_or(0) as usize,
            totals.get("nodes").and_then(Value::as_u64).unwrap_or(0) as usize,
        ),
        "rationale_density": ratio(
            totals.get("rationaleNodes").and_then(Value::as_u64).unwrap_or(0) as usize,
            totals.get("nodes").and_then(Value::as_u64).unwrap_or(0) as usize,
        ),
        "downstream_readiness": ratio(
            totals.get("downstreamReadyNodes").and_then(Value::as_u64).unwrap_or(0) as usize,
            totals.get("nodes").and_then(Value::as_u64).unwrap_or(0) as usize,
        ),
        "option_treatment_resolution": ratio(
            totals.get("handledOptions").and_then(Value::as_u64).unwrap_or(0) as usize,
            totals.get("options").and_then(Value::as_u64).unwrap_or(0) as usize,
        ),
        "counts": totals,
    })
}

fn count_payload_items(domains: &[Value]) -> Value {
    let mut totals = BTreeMap::from([
        ("nodes", 0u64),
        ("resolvedNodes", 0),
        ("rationaleNodes", 0),
        ("downstreamReadyNodes", 0),
        ("checklist", 0),
        ("selectedOrHandledChecklist", 0),
        ("options", 0),
        ("handledOptions", 0),
    ]);
    for domain in domains {
        for node in list(domain, "nodes") {
            add_total(&mut totals, "nodes", 1);
            let metadata = node.get("decisionMetadata").unwrap_or(&Value::Null);
            if matches!(
                str_field(metadata, "selection_state").as_str(),
                "answered" | "not_applicable"
            ) {
                add_total(&mut totals, "resolvedNodes", 1);
            }
            if !str_field(&metadata["selection_rationale"], "intent").is_empty() {
                add_total(&mut totals, "rationaleNodes", 1);
            }
            if metadata.get("derives_to_stage").is_some()
                && metadata.get("depends_on_decisions").is_some()
            {
                add_total(&mut totals, "downstreamReadyNodes", 1);
            }
            for item in list(&node, "checklist") {
                add_total(&mut totals, "checklist", 1);
                if bool_field(&item, "done")
                    || list(&item, "optionGroups")
                        .iter()
                        .any(|group| !list(group, "selected").is_empty())
                {
                    add_total(&mut totals, "selectedOrHandledChecklist", 1);
                }
                for group in list(&item, "optionGroups") {
                    for option in list(&group, "options") {
                        add_total(&mut totals, "options", 1);
                        if matches!(
                            str_field(&option, "treatment").as_str(),
                            "selected" | "rejected_with_reason" | "deferred"
                        ) {
                            add_total(&mut totals, "handledOptions", 1);
                        }
                    }
                }
            }
        }
    }
    serde_json::to_value(totals).unwrap_or_else(|_| json!({}))
}

fn selected_option_records(
    spec: &DesignNodeSpec,
    node_state: &NodeState,
) -> (Vec<Value>, Vec<Value>) {
    let mut selected = Vec::new();
    let mut primary = Vec::new();
    for item in &spec.checklist {
        let Some(item_options) = node_state.checklist_options.get(&item.item_id) else {
            continue;
        };
        let item_provenance = node_state.option_provenance.get(&item.item_id);
        for group in &item.option_groups {
            let Some(group_state) = item_options.get(&group.group_id) else {
                continue;
            };
            let group_provenance = item_provenance.and_then(|groups| groups.get(&group.group_id));
            for option_id in &group.options {
                if !group_state
                    .selected
                    .iter()
                    .any(|selected| selected == option_id)
                {
                    continue;
                }
                let record = json!({
                    "node_id": spec.node_id,
                    "item_id": item.item_id,
                    "group_id": group.group_id,
                    "option_id": option_id,
                    "label": humanize_id(option_id),
                    "description": "",
                    "source_refs": [format!("{}.{}.{}.{}", spec.node_id, item.item_id, group.group_id, option_id)],
                    "optionProvenance": group_provenance.and_then(|entries| entries.get(option_id)).and_then(option_provenance_value).unwrap_or_else(|| json!({})),
                });
                if group_state.primary == *option_id {
                    primary.push(record.clone());
                }
                selected.push(record);
            }
        }
    }
    (selected, primary)
}

fn profile_handoff_payload(project_state: &ProjectState) -> Value {
    let mut profile = Map::new();
    profile.insert("schema_version".to_string(), json!("1.0"));
    profile.insert("source".to_string(), json!("profile.json"));
    profile.insert("project_id".to_string(), json!(project_state.project_name));
    for (key, value) in &project_state.profile {
        profile.insert(key.clone(), value.clone());
    }
    Value::Object(profile)
}

fn write_layer_package(
    package_dir: &Path,
    source_type: &str,
    attachment_name: &str,
    markdown_content: &str,
    summary: &Value,
    project_state: &ProjectState,
    artifact_locale: ArtifactLocale,
) -> AdmResult<()> {
    let attachments_dir = package_dir.join("attachments");
    fs::create_dir_all(&attachments_dir)?;
    let attachment_rel = format!("attachments/{attachment_name}");
    io::write_text(&attachments_dir.join(attachment_name), markdown_content)?;
    let created_at = io::now_iso();
    let project_name = if project_state.project_name.trim().is_empty() {
        localized_text(artifact_locale, "未命名游戏项目", "Untitled Game Project")
    } else {
        &project_state.project_name
    };
    let stage_title = localized_text(artifact_locale, "初始创意录入", "Initial Idea Intake");
    io::write_json(
        &package_dir.join("package_manifest.json"),
        &json!({
            "schema_version": 1,
            "project": project_name,
            "project_id": "devflow",
            "package_id": format!("source:{source_type}"),
            "package_type": source_type,
            "package_type_id": source_type.to_ascii_lowercase(),
            "source_id": source_type,
            "source_ids": [source_type],
            "stage": 0,
            "stage_slug": "idea_intake",
            "stage_title": stage_title,
            "artifact_locale": artifact_locale,
            "version": 2,
            "generated_by": "autodesignmaker.export_adapter",
            "design_summary": summary,
        }),
    )?;
    io::write_json(
        &package_dir.join("operator_submission.json"),
        &json!({
            "schema_version": 1,
            "project": project_name,
            "step": 0,
            "slug": "idea_intake",
            "title": stage_title,
            "artifact_locale": artifact_locale,
            "created_at": created_at,
            "approved": true,
            "notes": if artifact_locale == ArtifactLocale::ZhCn {
                format!("AutoDesignMaker D4 已导出 {source_type} 交接包。")
            } else {
                format!("AutoDesignMaker D4 exported {source_type} package.")
            },
            "attachments": [attachment_rel],
            "primary_attachment": attachment_rel,
            "package_type": source_type,
            "source_id": source_type,
            "source_ids": [source_type],
        }),
    )?;
    io::write_json(
        &package_dir.join("human_approval.json"),
        &json!({
            "schema_version": 1,
            "approved": true,
            "artifact_locale": artifact_locale,
            "approved_at": created_at,
            "reviewer": "AutoDesignMaker D4",
            "source_attachment": attachment_rel,
        }),
    )?;
    io::write_json(
        &package_dir.join("selected_play_prototype.json"),
        &json!({
            "schema_version": 1,
            "id": format!("ADM-{}-001", source_type.to_ascii_uppercase()),
            "selected": true,
            "artifact_locale": artifact_locale,
            "description": project_name,
            "source_attachment": attachment_rel,
        }),
    )?;
    io::write_text(
        &package_dir.join("selected_play_prototype.md"),
        &format!(
            "<!-- artifact_locale: {} -->\n# {}\n\n{project_name}\n",
            artifact_locale.as_str(),
            localized_text(artifact_locale, "已选原型", "Selected Prototype")
        ),
    )?;
    io::write_text(
        &package_dir.join("human_review.md"),
        &format!(
            "<!-- artifact_locale: {} -->\n# {}\n\n{}\n",
            artifact_locale.as_str(),
            localized_text(artifact_locale, "人工复核", "Human Review"),
            localized_text(artifact_locale, "已批准。", "Approved.")
        ),
    )?;
    io::write_text(&package_dir.join("stage_input.md"), markdown_content)?;
    Ok(())
}

fn localized_text<'a>(locale: ArtifactLocale, zh_cn: &'a str, en_us: &'a str) -> &'a str {
    match locale {
        ArtifactLocale::ZhCn => zh_cn,
        ArtifactLocale::EnUs => en_us,
    }
}

fn profile_key_label(key: &str, locale: ArtifactLocale) -> String {
    if locale == ArtifactLocale::EnUs {
        return humanize_id(key);
    }
    match key {
        "targetScale" => "项目规模".to_string(),
        "businessModel" => "商业模式".to_string(),
        "platformScope" => "平台范围".to_string(),
        "regionScope" => "发行地区".to_string(),
        "socialModel" => "社交模式".to_string(),
        "operationModel" => "运营模式".to_string(),
        _ => humanize_id(key),
    }
}

fn concept_markdown(project_state: &ProjectState, artifact_locale: ArtifactLocale) -> String {
    let mut lines = vec![
        format!("<!-- artifact_locale: {} -->", artifact_locale.as_str()),
        format!(
            "# {} - {}",
            project_state.project_name,
            localized_text(artifact_locale, "游戏设计概念", "Design Concept")
        ),
        String::new(),
        localized_text(
            artifact_locale,
            "由 AutoDesignMaker D4 DevFlow 交接流程生成。",
            "Generated by AutoDesignMaker D4 DevFlow handoff.",
        )
        .to_string(),
        String::new(),
        localized_text(
            artifact_locale,
            "## 第 1 层 项目愿景",
            "## Layer 1 Project Vision",
        )
        .to_string(),
        localized_text(artifact_locale, "已提交 / 已接受", "Submitted / accepted").to_string(),
    ];
    for key in [
        "targetScale",
        "businessModel",
        "platformScope",
        "regionScope",
        "socialModel",
        "operationModel",
    ] {
        if let Some(value) = project_state
            .profile
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && *value != "unknown")
        {
            lines.push(format!(
                "- {}: {}",
                profile_key_label(key, artifact_locale),
                value
            ));
            lines.push(if artifact_locale == ArtifactLocale::ZhCn {
                format!("  目的: {} 的项目配置。", project_state.project_name)
            } else {
                format!(
                    "  Purpose: {} project configuration.",
                    project_state.project_name
                )
            });
        }
    }
    lines.push(String::new());
    lines.push(
        localized_text(
            artifact_locale,
            "## 第 2 层 核心体验",
            "## Layer 2 Core Experience",
        )
        .to_string(),
    );
    lines.push(
        localized_text(artifact_locale, "已提交 / 已接受", "Submitted / accepted").to_string(),
    );
    if project_state.gameplay_systems.selected.is_empty() {
        lines.push(format!(
            "- {}: {}",
            localized_text(artifact_locale, "游戏类型", "Game type"),
            project_state.project_name
        ));
        lines.push(
            localized_text(
                artifact_locale,
                "  目的: 等待补充游戏设计。",
                "  Purpose: pending game design.",
            )
            .to_string(),
        );
    } else {
        for system_id in project_state.gameplay_systems.selected.iter().take(5) {
            let loop_text = project_state
                .gameplay_systems
                .core_loops
                .get(system_id)
                .cloned()
                .unwrap_or_default();
            lines.push(format!(
                "- {}: {system_id}{}",
                localized_text(artifact_locale, "核心循环", "Core loop"),
                if loop_text.is_empty() {
                    String::new()
                } else {
                    format!(" -> {loop_text}")
                }
            ));
            lines.push(if artifact_locale == ArtifactLocale::ZhCn {
                format!("  目的: {} 的核心玩法循环。", project_state.project_name)
            } else {
                format!(
                    "  Purpose: {} core gameplay loop.",
                    project_state.project_name
                )
            });
        }
    }
    lines.join("\n") + "\n"
}

fn framework_markdown(project_state: &ProjectState, artifact_locale: ArtifactLocale) -> String {
    let mut lines = vec![
        format!("<!-- artifact_locale: {} -->", artifact_locale.as_str()),
        format!(
            "# {} - {}",
            project_state.project_name,
            localized_text(artifact_locale, "玩法框架", "Gameplay Framework")
        ),
        String::new(),
        localized_text(
            artifact_locale,
            "由 AutoDesignMaker D4 DevFlow 交接流程生成。",
            "Generated by AutoDesignMaker D4 DevFlow handoff.",
        )
        .to_string(),
        String::new(),
        localized_text(
            artifact_locale,
            "## 第 3 层 系统图",
            "## Layer 3 System Map",
        )
        .to_string(),
        localized_text(artifact_locale, "已提交 / 已接受", "Submitted / accepted").to_string(),
    ];
    if project_state.gameplay_systems.selected.is_empty() {
        lines.push(format!(
            "- {}: {}{}",
            localized_text(artifact_locale, "玩法系统", "system_layer"),
            project_state.project_name,
            localized_text(artifact_locale, "基础玩法系统", " base gameplay system")
        ));
        lines.push(
            localized_text(
                artifact_locale,
                "  目的: 等待补充定义。",
                "  Purpose: pending definition.",
            )
            .to_string(),
        );
    } else {
        for system_id in &project_state.gameplay_systems.selected {
            let weight = project_state
                .gameplay_systems
                .weights
                .get(system_id)
                .map(|item| item.weight.to_string())
                .unwrap_or_default();
            lines.push(format!(
                "- {}: {system_id}{}",
                localized_text(artifact_locale, "玩法系统", "system_layer"),
                if weight.is_empty() {
                    String::new()
                } else {
                    format!(" weight={weight}%")
                }
            ));
            lines.push(if artifact_locale == ArtifactLocale::ZhCn {
                format!("  目的: {} 的玩法系统模块。", project_state.project_name)
            } else {
                format!(
                    "  Purpose: {} gameplay system module.",
                    project_state.project_name
                )
            });
            lines.push(
                localized_text(
                    artifact_locale,
                    "  解锁: gameplay_requirements",
                    "  Unlocks: gameplay_requirements",
                )
                .to_string(),
            );
        }
    }
    lines.join("\n") + "\n"
}

fn design_markdown(
    engine: &DesignEngineService,
    project_state: &ProjectState,
    artifact_locale: ArtifactLocale,
) -> String {
    let mut lines = vec![
        format!("<!-- artifact_locale: {} -->", artifact_locale.as_str()),
        format!(
            "# {} - {}",
            project_state.project_name,
            localized_text(
                artifact_locale,
                "完整游戏设计规格",
                "Full Design Specification"
            )
        ),
        String::new(),
        localized_text(
            artifact_locale,
            "由 AutoDesignMaker D4 DevFlow 交接流程生成。",
            "Generated by AutoDesignMaker D4 DevFlow handoff.",
        )
        .to_string(),
        String::new(),
        localized_text(
            artifact_locale,
            "## 第 4 层 设计决策",
            "## Layer 4 Design Decisions",
        )
        .to_string(),
        localized_text(artifact_locale, "已提交 / 已接受", "Submitted / accepted").to_string(),
    ];
    for spec in engine.specs() {
        let node_state = project_state
            .nodes
            .get(&spec.node_id)
            .cloned()
            .unwrap_or_default();
        if matches!(
            node_state.decision_state,
            DecisionState::Completed | DecisionState::Selected
        ) {
            lines.push(format!(
                "- {}: {}",
                spec.node_id,
                if node_state.design_note.is_empty() {
                    localized_text(artifact_locale, "已完成", "completed")
                } else {
                    &node_state.design_note
                }
            ));
            lines.push(if artifact_locale == ArtifactLocale::ZhCn {
                format!("  目的: {} 的设计决策节点。", project_state.project_name)
            } else {
                format!(
                    "  Purpose: {} design decision node.",
                    project_state.project_name
                )
            });
            if !node_state.risk_note.is_empty() {
                lines.push(format!(
                    "  {}: {}",
                    localized_text(artifact_locale, "约束", "Constraint"),
                    node_state.risk_note
                ));
            }
        }
        if !node_state.design_entities.is_empty() {
            lines.push(String::new());
            lines.push(
                localized_text(
                    artifact_locale,
                    "## 第 5 层 L5 实体",
                    "## Layer 5 L5 Entities",
                )
                .to_string(),
            );
            _append_l5_design_entities(&mut lines, &spec.node_id, &node_state, artifact_locale);
        }
    }
    lines.join("\n") + "\n"
}

fn _append_l5_design_entities(
    lines: &mut Vec<String>,
    node_id: &str,
    node_state: &NodeState,
    artifact_locale: ArtifactLocale,
) {
    if node_state.design_entities.is_empty() {
        return;
    }
    lines.push(format!(
        "- {}: {node_id}",
        localized_text(artifact_locale, "L5 节点", "L5 node")
    ));
    lines.push(if artifact_locale == ArtifactLocale::ZhCn {
        format!(
            "  目的: 该具体设计节点包含 {} 个可追溯的 L5 实体。",
            node_state.design_entities.len()
        )
    } else {
        format!(
            "  Purpose: this concrete design node contains {} traceable L5 entities.",
            node_state.design_entities.len()
        )
    });
    for entity in &node_state.design_entities {
        let label = first_non_empty(
            entity,
            &["label", "id", "kind"],
            localized_text(artifact_locale, "未命名实体", "unnamed entity"),
        );
        let entity_id = str_field(entity, "id");
        let suffix = if !entity_id.is_empty() && entity_id != label {
            format!(" ({entity_id})")
        } else {
            String::new()
        };
        lines.push(format!(
            "- {}: {label}{suffix}",
            localized_text(artifact_locale, "L5 实体", "L5 entity")
        ));
        lines.push(format!(
            "  {}: {}",
            localized_text(artifact_locale, "目的", "Purpose"),
            entity_summary_fields(entity)
        ));
        lines.push(format!(
            "  {}: {node_id}",
            localized_text(artifact_locale, "依赖", "Depends")
        ));
        lines.push(format!(
            "  {}: program_requirements, art_requirements",
            localized_text(artifact_locale, "解锁", "Unlocks")
        ));
    }
}

fn design_summary(engine: &DesignEngineService, project_state: &ProjectState) -> Value {
    let node_count = engine.specs().len();
    let checklist_count = engine
        .specs()
        .iter()
        .map(|spec| spec.checklist.len())
        .sum::<usize>();
    let option_group_count = engine
        .specs()
        .iter()
        .flat_map(|spec| spec.checklist.iter())
        .map(|item| item.option_groups.len())
        .sum::<usize>();
    let entity_count = project_state
        .nodes
        .values()
        .map(|node| node.design_entities.len())
        .sum::<usize>();
    json!({
        "domain_count": engine.specs().iter().map(|spec| &spec.domain_id).collect::<BTreeSet<_>>().len(),
        "domain_names": engine.specs().iter().map(|spec| humanize_id(&spec.domain_id)).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
        "node_count": node_count,
        "checklist_count": checklist_count,
        "option_group_count": option_group_count,
        "validation_errors": 0,
        "validation_warnings": 0,
        "data_source": "adm-new-design::DesignEngineService",
        "design_entity_node_count": project_state.nodes.values().filter(|node| !node.design_entities.is_empty()).count(),
        "design_entity_count": entity_count,
    })
}

fn selected_groups(item: &Value) -> Vec<Value> {
    list(item, "optionGroups")
        .into_iter()
        .filter(|group| !list(group, "selected").is_empty())
        .collect()
}

fn empty_groups(item: &Value) -> Vec<Value> {
    list(item, "optionGroups")
        .into_iter()
        .filter(|group| list(group, "selected").is_empty())
        .collect()
}

fn decision_items(node: &Value) -> Vec<Value> {
    list(node, "checklist")
        .into_iter()
        .filter(|item| bool_field(item, "done") || !selected_groups(item).is_empty())
        .collect()
}

fn decision_nodes(domain: &Value) -> Vec<Value> {
    list(domain, "nodes")
        .into_iter()
        .filter(|node| node_has_decision(node))
        .collect()
}

fn node_has_decision(node: &Value) -> bool {
    matches!(
        str_field(node, "decisionState").as_str(),
        "completed" | "selected" | "risk"
    ) || !str_field(node, "designNote").is_empty()
        || !list(node, "designEntities").is_empty()
        || list(node, "checklist")
            .iter()
            .any(|item| bool_field(item, "done") || !selected_groups(item).is_empty())
}

fn pending_counts(domain: &Value) -> (usize, usize) {
    let mut pending_nodes = 0usize;
    let mut pending_items = 0usize;
    for node in list(domain, "nodes") {
        if !node_has_decision(&node) && str_field(&node, "decisionState") != "not_applicable" {
            pending_nodes += 1;
        }
        for item in list(&node, "checklist") {
            if !bool_field(&item, "done") && selected_groups(&item).is_empty() {
                pending_items += 1;
            }
        }
    }
    (pending_nodes, pending_items)
}

fn append_markdown_decision_item(lines: &mut Vec<String>, item: &Value) {
    lines.push(format!("- **{}**", str_field(item, "label")));
    for group in selected_groups(item) {
        let labels = value_list(group.get("selectedLabels")).join(", ");
        lines.push(format!("  - {}: {}", str_field(&group, "label"), labels));
    }
    let missing = empty_groups(item);
    if !missing.is_empty() {
        lines.push(format!(
            "  - Missing optional/required groups: {}",
            missing
                .iter()
                .map(|group| str_field(group, "label"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn append_text_decision_item(lines: &mut Vec<String>, item: &Value) {
    lines.push(format!("  - {}", str_field(item, "label")));
    for group in selected_groups(item) {
        let labels = value_list(group.get("selectedLabels")).join(", ");
        lines.push(format!("    {}: {}", str_field(&group, "label"), labels));
    }
}

fn append_markdown_design_entities(lines: &mut Vec<String>, node: &Value) {
    let entities = list(node, "designEntities");
    if entities.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("**L5 entities**".to_string());
    for entity in entities {
        let label = first_non_empty(&entity, &["label", "id", "kind"], "unnamed entity");
        lines.push(format!("- {}: {}", label, entity_summary_fields(&entity)));
    }
}

fn append_markdown_gameplay_systems(lines: &mut Vec<String>, payload: &Value) {
    let systems = value_list(payload["gameplaySystems"].get("selected"));
    if systems.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("## Gameplay Systems".to_string());
    for system in systems {
        lines.push(format!("- {system}"));
    }
}

fn append_text_gameplay_systems(lines: &mut Vec<String>, payload: &Value) {
    let systems = value_list(payload["gameplaySystems"].get("selected"));
    if systems.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("Gameplay systems:".to_string());
    for system in systems {
        lines.push(format!("  - {system}"));
    }
}

fn append_markdown_gameplay_global_view(lines: &mut Vec<String>, payload: &Value) {
    let systems = list(payload, "gameplaySystemGlobalView");
    if systems.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("## Gameplay Global View".to_string());
    for system in systems {
        lines.push(format!(
            "- {}: {}",
            str_field(&system, "id"),
            str_field(&system, "coreLoop")
        ));
    }
}

fn append_text_gameplay_global_view(lines: &mut Vec<String>, payload: &Value) {
    let systems = list(payload, "gameplaySystemGlobalView");
    if systems.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("Gameplay global view:".to_string());
    for system in systems {
        lines.push(format!(
            "  - {}: {}",
            str_field(&system, "id"),
            str_field(&system, "coreLoop")
        ));
    }
}

fn append_markdown_quality_violations(lines: &mut Vec<String>, payload: &Value) {
    let violations = list(payload, "qualityViolations");
    if violations.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("## Quality Issues".to_string());
    for violation in violations {
        lines.push(format!(
            "- [{}] {}",
            str_field(&violation, "severity"),
            str_field(&violation, "message")
        ));
    }
}

fn append_text_quality_violations(lines: &mut Vec<String>, payload: &Value) {
    let violations = list(payload, "qualityViolations");
    if violations.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("Quality issues:".to_string());
    for violation in violations {
        lines.push(format!(
            "  - [{}] {}",
            str_field(&violation, "severity"),
            str_field(&violation, "message")
        ));
    }
}

fn append_markdown_cross_layer_violations(lines: &mut Vec<String>, payload: &Value) {
    let violations = list(payload, "crossLayerViolations");
    if violations.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("## Cross-layer Violations".to_string());
    for violation in violations {
        lines.push(format!(
            "- [{}] {}",
            str_field(&violation, "severity"),
            str_field(&violation, "reason")
        ));
    }
}

fn append_text_cross_layer_violations(lines: &mut Vec<String>, payload: &Value) {
    let violations = list(payload, "crossLayerViolations");
    if violations.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push("Cross-layer violations:".to_string());
    for violation in violations {
        lines.push(format!(
            "  - [{}] {}",
            str_field(&violation, "severity"),
            str_field(&violation, "reason")
        ));
    }
}

fn markdown_quality_lines(payload: &Value) -> Vec<String> {
    vec![
        format!("- Quality badge: `{}`", str_field(payload, "qualityBadge")),
        format!(
            "- Critical issues: `{}`",
            number_text(payload.get("qualityCriticalCount"))
        ),
    ]
}

fn text_quality_lines(payload: &Value) -> Vec<String> {
    vec![
        format!("Quality badge: {}", str_field(payload, "qualityBadge")),
        format!(
            "Critical issues: {}",
            number_text(payload.get("qualityCriticalCount"))
        ),
    ]
}

fn payload_totals(payload: &Value) -> BTreeMap<&'static str, usize> {
    let mut totals = BTreeMap::from([
        ("domains", list(payload, "domains").len()),
        ("nodes", 0),
        ("checklist", 0),
        ("groups", 0),
        ("options", 0),
        ("activeConflicts", 0),
    ]);
    for domain in list(payload, "domains") {
        let nodes = list(&domain, "nodes");
        *totals.get_mut("nodes").unwrap() += nodes.len();
        for node in nodes {
            let checklist = list(&node, "checklist");
            *totals.get_mut("checklist").unwrap() += checklist.len();
            for item in checklist {
                let groups = list(&item, "optionGroups");
                *totals.get_mut("groups").unwrap() += groups.len();
                *totals.get_mut("activeConflicts").unwrap() +=
                    list(&item, "activeOptionConflicts").len();
                for group in groups {
                    *totals.get_mut("options").unwrap() += list(&group, "options").len();
                }
            }
        }
    }
    totals
}

fn decision_totals(payload: &Value) -> BTreeMap<&'static str, usize> {
    let mut totals = BTreeMap::from([
        ("nodes", 0),
        ("checklist", 0),
        ("groups", 0),
        ("notApplicable", 0),
        ("activeConflicts", 0),
        ("pendingNodes", 0),
        ("pendingItems", 0),
    ]);
    for domain in list(payload, "domains") {
        let (pending_nodes, pending_items) = pending_counts(&domain);
        *totals.get_mut("pendingNodes").unwrap() += pending_nodes;
        *totals.get_mut("pendingItems").unwrap() += pending_items;
        for node in decision_nodes(&domain) {
            *totals.get_mut("nodes").unwrap() += 1;
            if str_field(&node, "decisionState") == "not_applicable" {
                *totals.get_mut("notApplicable").unwrap() += 1;
            }
            for item in decision_items(&node) {
                *totals.get_mut("checklist").unwrap() += 1;
                *totals.get_mut("groups").unwrap() += selected_groups(&item).len();
                *totals.get_mut("activeConflicts").unwrap() +=
                    list(&item, "activeOptionConflicts").len();
            }
        }
    }
    totals
}

fn profile_display(profile: &BTreeMap<String, Value>) -> Value {
    Value::Object(
        profile
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    json!({"label": humanize_id(key), "value": value_label(value)}),
                )
            })
            .collect(),
    )
}

fn profile_case_genre(profile: &BTreeMap<String, Value>) -> Vec<String> {
    [
        "targetScale",
        "businessModel",
        "operationModel",
        "socialModel",
        "targetSessionBand",
    ]
    .iter()
    .filter_map(|key| profile.get(*key).and_then(Value::as_str))
    .filter(|value| !value.is_empty() && *value != "unknown")
    .map(ToString::to_string)
    .collect()
}

fn profile_applicability(profile: &BTreeMap<String, Value>) -> Vec<String> {
    [
        "targetScale",
        "primaryPlatform",
        "targetSessionBand",
        "businessModel",
        "socialModel",
    ]
    .iter()
    .filter_map(|key| {
        profile
            .get(*key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && *value != "unknown")
            .map(|value| format!("{} applies: {value}", humanize_id(key)))
    })
    .collect()
}

fn profile_not_applicable(profile: &BTreeMap<String, Value>) -> Vec<String> {
    let mut items = Vec::new();
    match profile.get("targetScale").and_then(Value::as_str) {
        Some("iaa_hypercasual") => items.extend([
            "heavy progression projects".to_string(),
            "large live-service projects".to_string(),
        ]),
        Some("3a") => items.extend([
            "short IAA hypercasual projects".to_string(),
            "low-cost rapid iteration projects".to_string(),
        ]),
        Some("large_service") => items.push("offline single-release projects".to_string()),
        _ => {}
    }
    match profile.get("primaryPlatform").and_then(Value::as_str) {
        Some("mobile") => items.push("PC/console input-first projects".to_string()),
        Some("pc_console") => items.push("touch-first mobile projects".to_string()),
        _ => {}
    }
    if profile.get("socialModel").and_then(Value::as_str) == Some("none") {
        items.push("real-time multiplayer or community-driven projects".to_string());
    }
    items
}

fn infer_document_type(project_state: &ProjectState) -> String {
    if let Some(value) = project_state
        .profile
        .get("documentType")
        .and_then(Value::as_str)
    {
        return value.to_string();
    }
    if project_state.project_name.starts_with("范本")
        || project_state.nodes.values().any(|node| {
            ["范本反推", "反推", "非官方配置", "公开信息"]
                .iter()
                .any(|hint| node.design_note.contains(hint))
        })
    {
        "template_reverse_inferred".to_string()
    } else {
        "project_answer".to_string()
    }
}

fn selection_state_for_node(node_state: &NodeState, project_state: &ProjectState) -> String {
    if matches!(node_state.decision_state, DecisionState::NotApplicable) {
        return "not_applicable".to_string();
    }
    if matches!(node_state.decision_state, DecisionState::NotStarted) {
        return "open".to_string();
    }
    if infer_document_type(project_state) == "template_reverse_inferred" {
        return "reverse_inferred".to_string();
    }
    "answered".to_string()
}

fn confidence_for_node(node_state: &NodeState, selection_state: &str) -> String {
    match selection_state {
        "open" => "low",
        "not_applicable" | "answered" => "high",
        "reverse_inferred"
            if node_state.design_note.contains("部分")
                || node_state.design_note.contains("合理推断") =>
        {
            "mid"
        }
        "reverse_inferred" => "mid",
        _ => "mid",
    }
    .to_string()
}

fn selection_rationale(node_state: &NodeState, selection_state: &str) -> Value {
    let (intent, evidence) = match selection_state {
        "open" => (
            "Current node has no exportable design answer.",
            "Not filled.",
        ),
        "not_applicable" => (
            "This node was explicitly marked not applicable.",
            node_state.not_applicable_reason.as_str(),
        ),
        "reverse_inferred" => (
            node_state.design_note.as_str(),
            "Inferred from public information and design structure.",
        ),
        _ => (
            node_state.design_note.as_str(),
            "Exported from current project state.",
        ),
    };
    json!({
        "intent": if intent.is_empty() { "Confirmed by current project state." } else { intent },
        "evidence": if evidence.is_empty() { "No extra evidence text." } else { evidence },
        "locked_downstream": "See derives_to_stage and depends_on_decisions.",
        "risks_and_alternatives": node_state.risk_note,
    })
}

fn derives_to_stage(domain_id: &str) -> Vec<u8> {
    match domain_id {
        "product_positioning_design" => vec![0, 1, 2, 7, 13],
        "core_experience_design" | "gameplay_system_design" => vec![1, 2, 3, 7, 10],
        "content_design" => vec![2, 3, 5, 7, 10],
        "balance_design" => vec![2, 7, 8, 10],
        "economy_monetization_design" => vec![2, 7, 8, 13],
        "ux_interface_design" | "presentation_feel_design" => vec![5, 8, 11],
        "retention_lifecycle_design"
        | "liveops_version_design"
        | "release_growth_design"
        | "launch_readiness_design" => vec![7, 13, 14, 15],
        _ => vec![2, 7, 13],
    }
}

fn domain_coverage(nodes: &[Value]) -> Value {
    let total_nodes = nodes.len();
    let done_nodes = nodes.iter().filter(|node| node_has_decision(node)).count();
    let total_checklist = nodes
        .iter()
        .map(|node| list(node, "checklist").len())
        .sum::<usize>();
    let done_checklist = nodes
        .iter()
        .flat_map(|node| list(node, "checklist"))
        .filter(|item| bool_field(item, "done"))
        .count();
    json!({
        "nodePercent": percent(done_nodes, total_nodes),
        "checklistPercent": percent(done_checklist, total_checklist),
    })
}

fn gameplay_weight_summary(
    weights: &BTreeMap<String, adm_new_contracts::project::GameplaySystemWeight>,
) -> Value {
    let total = weights
        .values()
        .map(|item| numeric_weight(&item.weight))
        .sum::<u32>();
    json!({"totalWeight": total, "configured": weights.len()})
}

fn gameplay_global_view(project_state: &ProjectState) -> Vec<Value> {
    project_state
        .gameplay_systems
        .selected
        .iter()
        .map(|system_id| {
            json!({
                "id": system_id,
                "coreLoop": project_state.gameplay_systems.core_loops.get(system_id).cloned().unwrap_or_default(),
                "weight": project_state.gameplay_systems.weights.get(system_id).map(|item| numeric_weight(&item.weight)).unwrap_or(0),
            })
        })
        .collect()
}

fn numeric_weight(value: &Value) -> u32 {
    match value {
        Value::Number(number) => number.as_u64().unwrap_or(0) as u32,
        Value::String(text) => text.parse::<u32>().unwrap_or(0),
        _ => 0,
    }
}

fn effective_state_name(node_state: &NodeState) -> &'static str {
    if matches!(node_state.decision_state, DecisionState::NotStarted)
        && (!node_state.design_note.is_empty()
            || node_state.checklist.values().any(|done| *done)
            || !node_state.design_entities.is_empty())
    {
        "selected"
    } else {
        node_state.decision_state.as_str()
    }
}

fn selection_provenance(decision_metadata: &Value) -> Value {
    json!({
        "source": if str_field(decision_metadata, "selection_state") == "reverse_inferred" { "reverse_inference" } else { "operator_decision" },
        "evidence": "Exported from current project state.",
        "evidence_url": Value::Null,
        "confidence": str_field(decision_metadata, "confidence"),
        "author": PROVENANCE_AUTHOR,
        "timestamp": io::now_iso(),
    })
}

fn option_provenance_value(entry: &OptionProvenanceEntry) -> Option<Value> {
    serde_json::to_value(entry).ok()
}

fn entity_summary_fields(entity: &Value) -> String {
    let parts = [
        "device", "mapping", "role", "behavior", "resource", "output", "trigger", "effect", "kind",
        "schema",
    ]
    .iter()
    .filter_map(|key| {
        entity.get(*key).and_then(|value| {
            if value.is_null() || value == "" || value == &json!([]) || value == &json!({}) {
                None
            } else {
                Some(format!("{key}={}", value_label(value)))
            }
        })
    })
    .collect::<Vec<_>>();
    if parts.is_empty() {
        "L5 concrete design entity".to_string()
    } else {
        parts.join("; ")
    }
}

fn missing_issue(contract_id: &str, artifact_path: &str, required_by_step: &str) -> Value {
    json!({
        "code": "REQUIRED_CONTRACT_MISSING",
        "contract_id": contract_id,
        "required_by_step": required_by_step,
        "artifact_path": artifact_path,
        "repair_hint": "Run D4 structured handoff and Step02 design freeze before this step.",
        "message": format!("Required contract `{contract_id}` is missing."),
    })
}

fn name_starts(path: &Path, prefix: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with(prefix))
        .unwrap_or(false)
}

fn ratio(done: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        ((done as f64 / total as f64) * 10000.0).round() / 10000.0
    }
}

fn percent(done: usize, total: usize) -> u32 {
    if total == 0 {
        0
    } else {
        ((done as f64 / total as f64) * 100.0).round() as u32
    }
}

fn add_total(totals: &mut BTreeMap<&'static str, u64>, key: &'static str, amount: u64) {
    *totals.entry(key).or_default() += amount;
}

fn object(value: &Value, key: &str) -> Map<String, Value> {
    let target = if key.is_empty() {
        value
    } else {
        value.get(key).unwrap_or(&Value::Null)
    };
    target.as_object().cloned().unwrap_or_default()
}

fn list(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn value_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(ToString::to_string))
            .collect(),
        Some(Value::String(value)) if !value.is_empty() => vec![value.clone()],
        _ => Vec::new(),
    }
}

fn value_set(value: Option<&Value>) -> BTreeSet<String> {
    value_list(value).into_iter().collect()
}

fn str_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn bool_value(value: Option<&bool>) -> bool {
    value.copied().unwrap_or(false)
}

fn value_label(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn number_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => "0".to_string(),
    }
}

fn first_non_empty(value: &Value, keys: &[&str], fallback: &str) -> String {
    keys.iter()
        .find_map(|key| {
            let text = str_field(value, key);
            (!text.trim().is_empty()).then_some(text)
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn humanize_id(id: &str) -> String {
    let words = id
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut label = String::new();
                    label.extend(first.to_uppercase());
                    label.push_str(chars.as_str());
                    label
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();
    if words.is_empty() {
        id.to_string()
    } else {
        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::project::ChecklistOptionGroupState;

    #[test]
    fn structured_handoff_writes_manifest_decisions_and_candidates() {
        let service = sample_service();
        let mut state = service.empty_state();
        state.project_name = "Structured Demo".to_string();
        state
            .profile
            .insert("archetype".to_string(), json!("generic_playable"));
        service
            .set_option_group_option(
                &mut state,
                "core_loop_decision",
                "core_loop",
                "loop_type",
                "action_loop",
                true,
            )
            .unwrap();
        service
            .set_option_group_primary(
                &mut state,
                "core_loop_decision",
                "core_loop",
                "loop_type",
                "action_loop",
            )
            .unwrap();
        let root = temp_root("structured_handoff");
        let manifest = write_structured_handoff(&root, &service, &state).unwrap();

        assert_eq!(manifest["handoff_type"], json!("structured_design_handoff"));
        assert_eq!(
            manifest["package_manifest_path"],
            json!("../package_manifest.json")
        );
        assert!(root.join("structured/handoff_manifest.json").exists());
        assert!(root.join("structured/decisions.json").exists());
        assert!(root.join("structured/profile.json").exists());
        assert!(root.join("structured/archetype_requirements.json").exists());
        assert!(root.join("structured/traceability.json").exists());
        assert!(
            root.join("structured/playable_contract_candidates/ui_flow_contract.json")
                .exists()
        );

        let decisions = io::read_json(&root.join("structured/decisions.json"), json!({}));
        let first = decisions["decisions"][0]["selected_options"][0].clone();
        assert_eq!(first["optionProvenance"]["source"], json!("user_selected"));
        assert_eq!(
            first["source_refs"][0],
            json!("core_loop_decision.core_loop.loop_type.action_loop")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn structured_context_prefers_stage2_and_falls_back_to_d4_candidate() {
        let root = temp_root("structured_context");
        let stage2 = root.join("outputs/artifacts/stage_02/playable_contracts");
        fs::create_dir_all(&stage2).unwrap();
        io::write_json(
            &stage2.join("ui_flow_contract.json"),
            &json!({"schema_version":"2.0","screens":[]}),
        )
        .unwrap();
        let mut context = StructuredDesignContext::from_output_base(&root);
        assert_eq!(
            context
                .require_playable_contract("ui_flow_contract", "Step13")
                .unwrap()["schema_version"],
            json!("2.0")
        );

        let handoff =
            root.join("source_artifacts/devflow_Design_v2/structured/playable_contract_candidates");
        fs::create_dir_all(&handoff).unwrap();
        io::write_json(
            &root.join("source_artifacts/devflow_Design_v2/package_manifest.json"),
            &json!({"package_type":"Design","source_ids":["Design"]}),
        )
        .unwrap();
        io::write_json(
            &root.join("source_artifacts/devflow_Design_v2/structured/handoff_manifest.json"),
            &json!({"handoff_type":"structured_design_handoff"}),
        )
        .unwrap();
        io::write_json(
            &handoff.join("scene_bootstrap_contract.json"),
            &json!({"schema_version":"2.0","start_scene":"Assets/Scenes/DemoScene.unity"}),
        )
        .unwrap();
        let mut context = StructuredDesignContext::from_output_base(&root);
        let payload = context
            .require_playable_contract("scene_bootstrap_contract", "Step13")
            .unwrap();
        assert_eq!(
            payload["start_scene"],
            json!("Assets/Scenes/DemoScene.unity")
        );
        assert_eq!(
            context.warnings[0]["code"],
            json!("USING_D4_CONTRACT_CANDIDATE")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn structured_context_missing_contract_reports_standard_issue() {
        let root = temp_root("structured_missing");
        let mut context = StructuredDesignContext::from_output_base(&root);
        let error = context
            .require_playable_contract("ui_flow_contract", "Step13")
            .unwrap_err();
        assert_eq!(error.issue["code"], json!("REQUIRED_CONTRACT_MISSING"));
        assert_eq!(error.issue["contract_id"], json!("ui_flow_contract"));
        assert_eq!(error.issue["required_by_step"], json!("Step13"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_adapter_writes_three_source_packages_and_sidecars() {
        let service = sample_service();
        let mut state = service.empty_state();
        state.project_name = "Export Demo".to_string();
        state.gameplay_systems.selected = vec!["combat".to_string()];
        state
            .gameplay_systems
            .core_loops
            .insert("combat".to_string(), "enter -> fight -> reward".to_string());
        service
            .set_option_group_option(
                &mut state,
                "core_loop_decision",
                "core_loop",
                "loop_type",
                "action_loop",
                true,
            )
            .unwrap();
        let root = temp_root("source_package");
        let result = export_concept_package_from_state(&root, &service, &state).unwrap();
        assert!(
            root.join("devflow_Concept_v2/package_manifest.json")
                .exists()
        );
        assert!(
            root.join("devflow_GameplayFramework_v2/stage_input.md")
                .exists()
        );
        assert!(
            root.join("devflow_Design_v2/structured/handoff_manifest.json")
                .exists()
        );
        assert!(
            root.join("devflow_Design_v2/structured/design_entities.json")
                .exists()
        );
        assert!(
            !root
                .join(
                    "devflow_Design_v2/structured/playable_contract_candidates/artifact_locale.json"
                )
                .exists()
        );
        assert_eq!(
            result["packages"]["Design"],
            json!(root.join("devflow_Design_v2").to_string_lossy().to_string())
        );
        let zh_manifest = io::read_json(
            &root.join("devflow_Concept_v2/package_manifest.json"),
            json!({}),
        );
        let zh_concept =
            fs::read_to_string(root.join("devflow_Concept_v2/attachments/concept.md")).unwrap();
        assert_eq!(zh_manifest["artifact_locale"], json!("zh-CN"));
        assert!(zh_concept.contains("## 第 1 层 项目愿景"));
        assert!(zh_concept.contains("核心循环"));

        let en_root = root.join("en");
        export_concept_package_from_state_with_locale(
            &en_root,
            &service,
            &state,
            ArtifactLocale::EnUs,
        )
        .unwrap();
        let en_concept =
            fs::read_to_string(en_root.join("devflow_Concept_v2/attachments/concept.md")).unwrap();
        assert!(en_concept.contains("## Layer 1 Project Vision"));
        assert!(en_concept.contains("Core loop"));

        let export_dir = root.join("exports");
        let markdown =
            write_export(&service, &state, &export_dir, "markdown", "archive", true).unwrap();
        assert!(markdown.ends_with("Export_Demo.full.md"));
        assert!(export_dir.join("Export_Demo.full.json").exists());
        assert!(export_dir.join("Export_Demo.profile.json").exists());
        assert!(export_dir.join("Export_Demo.coverage.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cross_layer_lint_flags_profile_forbidden_option() {
        let service = sample_service();
        let mut state = service.empty_state();
        state
            .profile
            .insert("targetScale".to_string(), json!("iaa_hypercasual"));
        let node = state.nodes.get_mut("core_loop_decision").unwrap();
        node.checklist_options
            .entry("core_loop".to_string())
            .or_default()
            .insert(
                "loop_type".to_string(),
                ChecklistOptionGroupState {
                    selected: vec!["deep_liveops".to_string()],
                    primary: String::new(),
                    extra: BTreeMap::new(),
                },
            );
        let rules = CrossLayerRuleSet::from_payload(json!({
            "schemaVersion": "1.0",
            "rules": [{
                "id": "rule.fast_scope",
                "severity": "WARNING",
                "reason": "IAA scope cannot select deep liveops.",
                "if": {"profile.targetScale": ["iaa_hypercasual"]},
                "forbidsOptionId": ["deep_liveops"]
            }]
        }));
        let violations = rules.lint(service.specs(), &state);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0]["hitOptionIds"][0], json!("deep_liveops"));
    }

    #[test]
    fn traceability_reads_handoff_field_refs() {
        let root = temp_root("traceability");
        let handoff = root.join("structured");
        fs::create_dir_all(&handoff).unwrap();
        io::write_json(
            &handoff.join("traceability.json"),
            &json!({"field_refs":[{"contract_id":"ui_flow_contract","path":"$.source_refs","source_refs":["node:ui"]}]}),
        )
        .unwrap();
        let context = StructuredDesignContext {
            output_base_dir: root.clone(),
            artifacts_dir: root.join("outputs/artifacts"),
            handoff_dir: Some(handoff),
            warnings: Vec::new(),
        };
        assert_eq!(
            context.trace("ui_flow_contract", "$.source_refs")[0]["source_refs"][0],
            json!("node:ui")
        );
        let _ = fs::remove_dir_all(root);
    }

    fn sample_service() -> DesignEngineService {
        DesignEngineService::new(vec![DesignNodeSpec {
            node_id: "core_loop_decision".to_string(),
            domain_id: "core_experience_design".to_string(),
            name: "Core Loop".to_string(),
            description: "Define the playable core loop.".to_string(),
            role_class: "system_concrete".to_string(),
            checklist: vec![crate::DesignChecklistItemSpec {
                item_id: "core_loop".to_string(),
                label: "Core Loop".to_string(),
                option_groups: vec![crate::DesignOptionGroupSpec {
                    group_id: "loop_type".to_string(),
                    selection_mode: "single".to_string(),
                    allow_primary: true,
                    options: vec![
                        "action_loop".to_string(),
                        "puzzle_loop".to_string(),
                        "deep_liveops".to_string(),
                    ],
                }],
            }],
        }])
    }

    fn temp_root(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(adm_new_foundation::new_stable_id(prefix).unwrap())
    }
}
