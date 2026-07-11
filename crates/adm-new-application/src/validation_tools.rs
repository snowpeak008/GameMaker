use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use adm_new_contracts::schema::{ContractValidationReport, validate_contract_file};
use adm_new_foundation::io::{now_iso, read_json, write_json};
use adm_new_foundation::structured_md::read_structured_or_text;
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const REQUIRED_CONTEXT_SECTIONS: &[&str] = &[
    "Core Pipeline And Save Boundaries",
    "Art Governance And Asset Contracts",
    "Execution Objects And GUI Gates",
    "Stage Sequence And Runtime Integration",
];

const REJECT_PHRASES: &[&str] = &[
    "抱歉",
    "我不能",
    "我无法",
    "对不起",
    "i cannot",
    "i apologize",
    "i'm unable",
    "i am unable",
    "sorry",
    "as an ai",
    "i can't",
    "i cannot fulfill",
];

const EXPECTED_PIPELINE_METRIC_FILES: &[(u32, &str)] = &[
    (0, "core_question_coverage_report.json"),
    (1, "core_loop.json"),
    (1, "system_definitions.json"),
    (2, "entity_coverage_report.json"),
    (3, "requirement_quality_report.json"),
    (4, "asset_registry.json"),
    (5, "intelligent_review_report.json"),
];
const PLAN_002_MIN_ENTITY_COVERAGE: f64 = 0.38;
const SEMANTIC_STAGES: std::ops::RangeInclusive<u32> = 0..=10;
const KEY_CONTRACT_FILES: &[&str] = &[
    "concept_profile.json",
    "project_identity_contract.json",
    "project_dna_seed.json",
    "design_freeze_contract.json",
    "project_dna_contract.json",
    "program_requirements_contract.json",
    "program_capability_contract.json",
    "art_requirements_contract.json",
    "art_taxonomy_contract.json",
    "asset_strategy_matrix.json",
    "program_plan_contract.json",
    "program_task_breakdown.json",
    "art_production_task_contract.json",
    "art_task_breakdown.json",
    "asset_alignment_report.json",
    "semantic_alignment_report.json",
    "semantic_coverage_matrix.json",
];
const PLACEHOLDER_MARKERS: &[&str] = &["placeholder", "fallback", "dummy", "stub"];
const TEMPLATE_LEAKAGE_MARKERS: &[&str] = &[
    "Hades reference template",
    "generated from a Hades",
    "reverse-engineering caveat",
    "范本反推",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigTableValidation {
    pub name: String,
    pub passed: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigValidationReport {
    pub status: String,
    pub tables: Vec<ConfigTableValidation>,
}

impl ConfigValidationReport {
    pub fn passed(&self) -> bool {
        self.status == "PASS"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextLintReport {
    pub path: String,
    pub valid: bool,
    pub terms: usize,
    pub terms_with_adr: usize,
    pub adr_coverage: f64,
    pub sections: BTreeMap<String, usize>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentOutputValidationReport {
    pub valid: bool,
    pub output_name: String,
    pub expected_format: String,
    pub parsed_type: String,
    pub errors: Vec<String>,
}

pub fn validate_config_tables(
    schema_path: impl AsRef<Path>,
    tables_dir: impl AsRef<Path>,
) -> AdmResult<ConfigValidationReport> {
    let schema = read_structured_or_text(schema_path.as_ref())?;
    let tables = schema
        .get("tables")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut all_passed = true;
    let mut results = Vec::new();
    for table in tables {
        let name = string_field(&table, "name");
        if name.is_empty() {
            continue;
        }
        let csv_path = tables_dir.as_ref().join(format!("{name}.csv"));
        if !csv_path.exists() {
            all_passed = false;
            results.push(ConfigTableValidation {
                name,
                passed: false,
                errors: vec![format!("CSV 文件不存在：{}", csv_path.display())],
                warnings: Vec::new(),
            });
            continue;
        }
        let validation = validate_config_table(&table, &csv_path)?;
        if !validation.passed {
            all_passed = false;
        }
        results.push(validation);
    }
    Ok(ConfigValidationReport {
        status: if all_passed { "PASS" } else { "FAIL" }.to_string(),
        tables: results,
    })
}

pub fn validate_config_table(
    table_def: &Value,
    csv_path: &Path,
) -> AdmResult<ConfigTableValidation> {
    let table_name = string_field(table_def, "name");
    let columns = table_def
        .get("columns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rows = parse_csv_table(csv_path)?;
    let headers = rows.headers;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for column in &columns {
        let name = string_field(column, "name");
        if bool_field(column, "required") && !headers.contains(&name) {
            errors.push(format!("缺少必填列：{name}"));
        }
        if column.get("deprecated").is_some() && headers.contains(&name) {
            warnings.push(format!(
                "使用了已废弃的列：{}（{}）",
                name,
                scalar_text(column.get("deprecated").unwrap_or(&Value::Null))
            ));
        }
    }

    let mut unique_values: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (row_index, row) in rows.records.iter().enumerate() {
        for column in &columns {
            let name = string_field(column, "name");
            if !row.contains_key(&name) {
                continue;
            }
            let value = row.get(&name).cloned().unwrap_or_default();
            let expected_type = string_field(column, "type")
                .if_empty("string")
                .to_ascii_lowercase();
            if !value.is_empty() {
                match expected_type.as_str() {
                    "int" => {
                        if value.parse::<i64>().is_err() {
                            errors.push(format!(
                                "表 {table_name} 行 {} 列 {name}：期望 int，实际 '{value}'",
                                row_index + 1
                            ));
                        }
                    }
                    "float" => {
                        if value.parse::<f64>().is_err() {
                            errors.push(format!(
                                "表 {table_name} 行 {} 列 {name}：期望 float，实际 '{value}'",
                                row_index + 1
                            ));
                        }
                    }
                    "bool" => {
                        if !matches!(
                            value.to_ascii_lowercase().as_str(),
                            "true" | "false" | "0" | "1"
                        ) {
                            errors.push(format!(
                                "表 {table_name} 行 {} 列 {name}：期望 bool，实际 '{value}'",
                                row_index + 1
                            ));
                        }
                    }
                    _ => {}
                }
            }
            if bool_field(column, "required") && value.is_empty() {
                errors.push(format!(
                    "表 {table_name} 行 {} 列 {name}：必填，但为空",
                    row_index + 1
                ));
            }
            if bool_field(column, "unique") {
                let values = unique_values.entry(name.clone()).or_default();
                if let Some(previous) = values.get(&value) {
                    errors.push(format!(
                        "表 {table_name} 行 {} 列 {name}：值 '{value}' 重复（唯一约束，首次出现行 {previous}）",
                        row_index + 1
                    ));
                }
                values.insert(value.clone(), row_index + 1);
            }
            if matches!(expected_type.as_str(), "int" | "float")
                && !value.is_empty()
                && let Ok(parsed) = value.parse::<f64>()
            {
                if let Some(min) = number_field(column, "min")
                    && parsed < min
                {
                    errors.push(format!(
                        "表 {table_name} 行 {} 列 {name}：{parsed} < min({min})",
                        row_index + 1
                    ));
                }
                if let Some(max) = number_field(column, "max")
                    && parsed > max
                {
                    errors.push(format!(
                        "表 {table_name} 行 {} 列 {name}：{parsed} > max({max})",
                        row_index + 1
                    ));
                }
            }
        }
    }

    Ok(ConfigTableValidation {
        name: table_name,
        passed: errors.is_empty(),
        errors,
        warnings,
    })
}

pub fn lint_context_file(path: impl AsRef<Path>) -> AdmResult<ContextLintReport> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let mut sections = REQUIRED_CONTEXT_SECTIONS
        .iter()
        .map(|section| ((*section).to_string(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut current_section = String::new();
    let mut terms = 0usize;
    let mut terms_with_adr = 0usize;
    let mut last_term_has_adr = false;
    let mut errors = Vec::new();
    for line in text.lines() {
        if let Some(section) = line.strip_prefix("## ") {
            current_section = section.trim().to_string();
        }
        if is_context_term_line(line) {
            if terms > 0 && !last_term_has_adr {
                errors.push("term missing _ADR_ before next term".to_string());
            }
            terms += 1;
            last_term_has_adr = false;
            if let Some(count) = sections.get_mut(&current_section) {
                *count += 1;
            } else {
                errors.push(format!("term outside required sections: {line}"));
            }
        } else if line.starts_with("_ADR_:") && terms > 0 {
            terms_with_adr += 1;
            last_term_has_adr = true;
        }
    }
    if terms > 0 && !last_term_has_adr {
        errors.push("last term missing _ADR_".to_string());
    }
    for section in REQUIRED_CONTEXT_SECTIONS {
        if !text.contains(section) {
            errors.push(format!("missing section: {section}"));
        }
        if sections.get(*section).copied().unwrap_or_default() == 0 {
            errors.push(format!("section has no terms: {section}"));
        }
    }
    let coverage = if terms == 0 {
        0.0
    } else {
        round3(terms_with_adr as f64 / terms as f64)
    };
    if coverage < 0.8 {
        errors.push(format!("ADR coverage below threshold: {coverage:.3}"));
    }
    Ok(ContextLintReport {
        path: path.display().to_string(),
        valid: errors.is_empty(),
        terms,
        terms_with_adr,
        adr_coverage: coverage,
        sections,
        errors,
    })
}

pub fn validate_contract_file_report(
    contract_path: impl AsRef<Path>,
    schema_path: impl AsRef<Path>,
    report_path: Option<impl AsRef<Path>>,
) -> AdmResult<ContractValidationReport> {
    let contract_path = contract_path.as_ref();
    let schema_path = schema_path.as_ref();
    let errors = validate_contract_file(contract_path, schema_path)?;
    let report = ContractValidationReport {
        contract: normalize_path(contract_path),
        schema: normalize_path(schema_path),
        valid: errors.is_empty(),
        errors,
    };
    if let Some(report_path) = report_path {
        let text = serde_json::to_string_pretty(&report)
            .map_err(|error| AdmError::new(format!("failed to serialize report: {error}")))?;
        if let Some(parent) = report_path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(report_path, format!("{text}\n"))?;
    }
    Ok(report)
}

pub fn validate_agent_output(
    text: &str,
    expected_format: &str,
    required_keys: &[String],
    output_name: &str,
) -> AgentOutputValidationReport {
    let mut errors = Vec::new();
    let content = text.trim();
    if content.is_empty() {
        errors.push(format!("{output_name} is empty."));
    }
    let content_lower = content.to_ascii_lowercase();
    for phrase in REJECT_PHRASES {
        if content_lower.contains(phrase) {
            errors.push(format!("{output_name} contains rejection phrase: {phrase}"));
        }
    }
    let mut parsed_type = String::new();
    if errors.is_empty() && expected_format == "json" {
        match parse_json_output(content, true) {
            Ok(value) => {
                parsed_type = json_type_name(&value).to_string();
                if !required_keys.is_empty() {
                    if let Some(object) = value.as_object() {
                        let missing = required_keys
                            .iter()
                            .filter(|key| !object.contains_key(key.as_str()))
                            .cloned()
                            .collect::<Vec<_>>();
                        if !missing.is_empty() {
                            errors.push(format!(
                                "{output_name} missing required fields: {}",
                                missing.join(", ")
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "{output_name} must be an object with keys {:?}.",
                            required_keys
                        ));
                    }
                }
            }
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() && parsed_type.is_empty() {
        parsed_type = "text".to_string();
    }
    AgentOutputValidationReport {
        valid: errors.is_empty(),
        output_name: output_name.to_string(),
        expected_format: expected_format.to_string(),
        parsed_type,
        errors,
    }
}

pub fn collect_pipeline_quality_metrics(artifacts_root: impl AsRef<Path>) -> Value {
    let root = artifacts_root.as_ref();
    let question_coverage = read_json(
        &stage_path(root, 0, "core_question_coverage_report.json"),
        json!({}),
    );
    let core_loop = read_json(&stage_path(root, 1, "core_loop.json"), json!({}));
    let systems = read_json(&stage_path(root, 1, "system_definitions.json"), json!({}));
    let entity_coverage = read_json(
        &stage_path(root, 2, "entity_coverage_report.json"),
        json!({}),
    );
    let requirement_quality = read_json(
        &stage_path(root, 3, "requirement_quality_report.json"),
        json!({}),
    );
    let assets = read_json(&stage_path(root, 4, "asset_registry.json"), json!({}));
    let program_review = read_json(
        &stage_path(root, 5, "intelligent_review_report.json"),
        json!({}),
    );
    let asset_count = assets
        .get("assets")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifacts_root": root.display().to_string(),
        "metric_file_count": EXPECTED_PIPELINE_METRIC_FILES.iter().filter(|(stage, file)| stage_path(root, *stage, file).is_file()).count(),
        "metrics": {
            "question_coverage_rate": number_or(&question_coverage, "coverage_rate", 0.0),
            "core_loop_output_rate": if core_loop.get("loop").is_some_and(|value| !is_nullish(value)) { 1.0 } else { 0.0 },
            "system_definition_rate": number_or(&systems, "definition_rate", 0.0),
            "design_entity_coverage_rate": number_or(&entity_coverage, "entity_coverage_rate", 0.0),
            "design_entity_count": number_or(&entity_coverage, "entity_count", 0.0),
            "requirement_system_binding_rate": number_or(&requirement_quality, "system_binding_rate", 0.0),
            "requirement_placeholder_rate": number_or(&requirement_quality, "placeholder_rate", 1.0),
            "asset_count": asset_count,
            "stage05_warning_count": number_or(&program_review, "warning_count", 0.0),
            "stage05_blocking_issue_count": number_or(&program_review, "blocking_issue_count", 0.0),
        },
        "targets": {
            "question_coverage_rate": ">= 0.55 for v5 Phase 1",
            "core_loop_output_rate": ">= 1.00",
            "design_entity_coverage_rate": ">= 0.38 for PLAN-002; >= 0.75 with real L5 entities",
            "requirement_system_binding_rate": ">= 0.90",
            "requirement_placeholder_rate": "<= 0.25",
            "asset_count": ">= 50 synthetic; >= 80 with real L5 entities",
            "stage05_warning_count": "<= 15 after full configured run",
        }
    })
}

pub fn check_pipeline_plan_002(artifacts_root: impl AsRef<Path>) -> Value {
    let mut payload = collect_pipeline_quality_metrics(artifacts_root);
    let actual = payload
        .get("metrics")
        .and_then(|metrics| metrics.get("design_entity_coverage_rate"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let passed = actual >= PLAN_002_MIN_ENTITY_COVERAGE;
    payload["checks"] = json!({
        "plan-002": {
            "passed": passed,
            "metric": "design_entity_coverage_rate",
            "actual": actual,
            "minimum": PLAN_002_MIN_ENTITY_COVERAGE,
        }
    });
    payload
}

pub fn collect_design_semantic_quality(artifacts_root: impl AsRef<Path>) -> AdmResult<Value> {
    let root = artifacts_root.as_ref();
    let mut stage_scores = Vec::new();
    let mut total_generic_tasks = 0usize;
    let mut total_tasks = 0usize;
    let mut total_placeholder_items = 0usize;
    let mut total_art_items = 0usize;
    let mut total_nullish = 0usize;
    let mut total_fields = 0usize;
    for stage in SEMANTIC_STAGES {
        let stage_dir = root.join(format!("stage_{stage:02}"));
        let mut stage_fields = 0usize;
        let mut stage_nullish = 0usize;
        let mut stage_tasks = 0usize;
        let mut stage_generic_tasks = 0usize;
        let mut stage_art_items = 0usize;
        let mut stage_placeholders = 0usize;
        let mut present_files = Vec::new();
        if stage_dir.is_dir() {
            for filename in KEY_CONTRACT_FILES {
                let path = stage_dir.join(filename);
                if !path.exists() {
                    continue;
                }
                present_files.push((*filename).to_string());
                let data = read_json(&path, json!({}));
                let (fields, nullish) = nullish_stats(&data);
                stage_fields += fields;
                stage_nullish += nullish;
                let tasks = list_items(&data, &["tasks", "capabilities"]);
                stage_tasks += tasks.len();
                stage_generic_tasks += tasks.iter().filter(|task| is_generic_task(task)).count();
                let art_items = list_items(
                    &data,
                    &[
                        "assets",
                        "asset_requirements",
                        "asset_groups",
                        "mount_items",
                        "alignment_items",
                    ],
                );
                stage_art_items += art_items.len();
                stage_placeholders += art_items
                    .iter()
                    .filter(|item| contains_marker(item, PLACEHOLDER_MARKERS))
                    .count();
            }
        }
        total_fields += stage_fields;
        total_nullish += stage_nullish;
        total_tasks += stage_tasks;
        total_generic_tasks += stage_generic_tasks;
        total_art_items += stage_art_items;
        total_placeholder_items += stage_placeholders;
        let nullish_ratio = ratio(stage_nullish, stage_fields);
        let generic_ratio = ratio(stage_generic_tasks, stage_tasks);
        let placeholder_ratio = ratio(stage_placeholders, stage_art_items);
        stage_scores.push(json!({
            "stage": stage,
            "present_contract_files": present_files,
            "field_count": stage_fields,
            "nullish_field_count": stage_nullish,
            "nullish_field_ratio": round4(nullish_ratio),
            "task_count": stage_tasks,
            "generic_task_count": stage_generic_tasks,
            "generic_task_ratio": round4(generic_ratio),
            "art_item_count": stage_art_items,
            "placeholder_item_count": stage_placeholders,
            "placeholder_asset_ratio": round4(placeholder_ratio),
            "stage_score": stage_score(nullish_ratio, generic_ratio, placeholder_ratio, 0),
        }));
    }
    let tokens = project_tokens(root);
    let leakage = template_leakage_count(root)?;
    let signature_report = project_signature_report(root)?;
    let generic_ratio = ratio(total_generic_tasks, total_tasks);
    let placeholder_ratio = ratio(total_placeholder_items, total_art_items);
    let nullish_ratio = ratio(total_nullish, total_fields);
    let keyword_rate = keyword_downstream_rate(root, &tokens)?;
    let mut blocking_issues = Vec::new();
    let mut warnings = Vec::new();
    if leakage > 0 {
        warnings.push(json!({
            "code": "TEMPLATE_LEAKAGE_DETECTED",
            "message": "Template leakage markers were found in Step00-10 artifacts.",
            "count": leakage,
        }));
    }
    if signature_report["cross_context_count"]
        .as_u64()
        .unwrap_or_default()
        > 0
    {
        blocking_issues.push(json!({
            "code": "PROJECT_SIGNATURE_MISMATCH",
            "message": "Multiple project_signature values were found under one artifacts root.",
            "count": signature_report["cross_context_count"],
        }));
    }
    if generic_ratio > 0.45 {
        warnings.push(json!({
            "code": "GENERIC_TASK_RATIO_HIGH",
            "message": "A high ratio of generic tasks was detected.",
            "ratio": round4(generic_ratio),
        }));
    }
    Ok(json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifacts_root": root.display().to_string(),
        "project_tokens": tokens.into_iter().collect::<Vec<_>>(),
        "stage_scores": stage_scores,
        "summary": {
            "nullish_field_ratio": round4(nullish_ratio),
            "generic_task_ratio": round4(generic_ratio),
            "placeholder_asset_ratio": round4(placeholder_ratio),
            "project_keyword_downstream_rate": round4(keyword_rate),
            "template_leakage_count": leakage,
            "source_ref_cross_context_count": signature_report["cross_context_count"],
        },
        "project_signature_report": signature_report,
        "blocking_issues": blocking_issues,
        "warnings": warnings,
    }))
}

pub fn render_design_semantic_markdown(report: &Value) -> String {
    let summary = report.get("summary").unwrap_or(&Value::Null);
    let mut lines = vec![
        "# Design Semantic Quality Report".to_string(),
        String::new(),
        format!(
            "- artifacts_root: `{}`",
            report
                .get("artifacts_root")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        format!(
            "- nullish_field_ratio: {}",
            summary
                .get("nullish_field_ratio")
                .and_then(Value::as_f64)
                .unwrap_or_default()
        ),
        format!(
            "- generic_task_ratio: {}",
            summary
                .get("generic_task_ratio")
                .and_then(Value::as_f64)
                .unwrap_or_default()
        ),
        format!(
            "- placeholder_asset_ratio: {}",
            summary
                .get("placeholder_asset_ratio")
                .and_then(Value::as_f64)
                .unwrap_or_default()
        ),
        format!(
            "- project_keyword_downstream_rate: {}",
            summary
                .get("project_keyword_downstream_rate")
                .and_then(Value::as_f64)
                .unwrap_or_default()
        ),
        format!(
            "- template_leakage_count: {}",
            summary
                .get("template_leakage_count")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ),
        format!(
            "- source_ref_cross_context_count: {}",
            summary
                .get("source_ref_cross_context_count")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ),
        String::new(),
        "| Stage | Score | Nullish | Generic Tasks | Placeholder Assets |".to_string(),
        "|---:|---:|---:|---:|---:|".to_string(),
    ];
    for item in report
        .get("stage_scores")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            item.get("stage")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            item.get("stage_score")
                .and_then(Value::as_f64)
                .unwrap_or_default(),
            item.get("nullish_field_ratio")
                .and_then(Value::as_f64)
                .unwrap_or_default(),
            item.get("generic_task_ratio")
                .and_then(Value::as_f64)
                .unwrap_or_default(),
            item.get("placeholder_asset_ratio")
                .and_then(Value::as_f64)
                .unwrap_or_default(),
        ));
    }
    format!("{}\n", lines.join("\n"))
}

pub fn write_design_semantic_quality_outputs(
    report: &Value,
    json_path: Option<impl AsRef<Path>>,
    markdown_path: Option<impl AsRef<Path>>,
) -> AdmResult<()> {
    if let Some(path) = json_path {
        write_json(path.as_ref(), report)?;
    }
    if let Some(path) = markdown_path {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, render_design_semantic_markdown(report))?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CsvRows {
    headers: Vec<String>,
    records: Vec<BTreeMap<String, String>>,
}

fn parse_csv_table(path: &Path) -> AdmResult<CsvRows> {
    let text = fs::read_to_string(path)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut lines = text.lines();
    let headers = lines
        .next()
        .map(parse_csv_line)
        .unwrap_or_default()
        .into_iter()
        .map(|item| item.trim().to_string())
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let values = parse_csv_line(line);
        let record = headers
            .iter()
            .enumerate()
            .map(|(index, header)| {
                (
                    header.clone(),
                    values.get(index).cloned().unwrap_or_default(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        records.push(record);
    }
    Ok(CsvRows { headers, records })
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                values.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    values.push(current.trim().to_string());
    values
}

fn is_context_term_line(line: &str) -> bool {
    line.starts_with("**") && line.contains("**:")
}

fn parse_json_output(content: &str, prefer_json_fence: bool) -> Result<Value, String> {
    let candidates = if prefer_json_fence {
        let fenced = fenced_blocks(content, Some("json"));
        if fenced.is_empty() {
            vec![content.to_string()]
        } else {
            fenced
        }
    } else {
        vec![content.to_string()]
    };
    let mut last_error = String::new();
    for candidate in candidates {
        match serde_json::from_str(candidate.trim()) {
            Ok(value) => return Ok(value),
            Err(error) => last_error = error.to_string(),
        }
    }
    let fallback = fenced_blocks(content, None);
    for candidate in fallback {
        match serde_json::from_str(candidate.trim()) {
            Ok(value) => return Ok(value),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(format!("Agent output is not valid JSON: {last_error}"))
}

fn fenced_blocks(content: &str, language: Option<&str>) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("```") {
        let after_start = &rest[start + 3..];
        let Some(line_end) = after_start.find('\n') else {
            break;
        };
        let lang = after_start[..line_end]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let after_lang = &after_start[line_end + 1..];
        let Some(end) = after_lang.find("```") else {
            break;
        };
        if language.is_none_or(|expected| expected.eq_ignore_ascii_case(&lang)) {
            blocks.push(after_lang[..end].trim().to_string());
        }
        rest = &after_lang[end + 3..];
    }
    blocks
}

fn stage_path(root: &Path, stage: u32, filename: &str) -> PathBuf {
    root.join(format!("stage_{stage:02}")).join(filename)
}

fn nullish_stats(value: &Value) -> (usize, usize) {
    let mut total = 0usize;
    let mut nullish = 0usize;
    fn walk(value: &Value, total: &mut usize, nullish: &mut usize) {
        match value {
            Value::Object(object) => {
                for child in object.values() {
                    *total += 1;
                    if is_nullish(child) {
                        *nullish += 1;
                    }
                    walk(child, total, nullish);
                }
            }
            Value::Array(items) => {
                for child in items {
                    walk(child, total, nullish);
                }
            }
            _ => {}
        }
    }
    walk(value, &mut total, &mut nullish);
    (total, nullish)
}

fn is_nullish(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(object) => object.is_empty(),
        _ => false,
    }
}

fn list_items(value: &Value, keys: &[&str]) -> Vec<Value> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

fn is_generic_task(task: &Value) -> bool {
    let text = task_label(task).to_ascii_lowercase();
    if (text.contains("dev") && text.contains("feature"))
        || text.contains("dev-") && text.contains("feature")
        || [
            "implement runtimebootstrap",
            "implement gamestate",
            "implement inputrouter",
            "implement uicontroller",
            "implement objectivetracker",
            "implement scenebootstrap",
            "implement camerarig",
        ]
        .iter()
        .any(|pattern| text.contains(pattern))
    {
        return true;
    }
    task.get("project_semantic_refs")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

fn task_label(task: &Value) -> String {
    let Some(object) = task.as_object() else {
        return task.to_string();
    };
    let mut parts = Vec::new();
    for key in [
        "task_id",
        "id",
        "title",
        "name",
        "description",
        "acceptance",
    ] {
        if let Some(value) = object.get(key) {
            parts.push(scalar_text(value));
        }
    }
    for key in ["output_files", "files", "changed_files"] {
        if let Some(items) = object.get(key).and_then(Value::as_array) {
            parts.extend(items.iter().map(scalar_text));
        }
    }
    parts.join(" ")
}

fn contains_marker(item: &Value, markers: &[&str]) -> bool {
    let text = item.to_string().to_ascii_lowercase();
    markers.iter().any(|marker| text.contains(marker))
}

fn project_tokens(root: &Path) -> BTreeSet<String> {
    let concept = read_json(&stage_path(root, 0, "concept_profile.json"), json!({}));
    let raw = ["project_id", "project_name", "title", "genre", "genre_key"]
        .iter()
        .filter_map(|key| concept.get(*key).map(scalar_text))
        .collect::<Vec<_>>()
        .join(" ");
    tokenize_project_terms(&raw)
        .into_iter()
        .filter(|token| {
            !matches!(
                token.as_str(),
                "game" | "project" | "template" | "generic" | "playable"
            )
        })
        .collect()
}

fn tokenize_project_terms(text: &str) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    let mut current = String::new();
    let mut current_is_cjk = false;
    for ch in text.chars() {
        let is_cjk = ('\u{4e00}'..='\u{9fff}').contains(&ch);
        if ch.is_ascii_alphanumeric() || ch == '_' || is_cjk {
            if !current.is_empty() && current_is_cjk != is_cjk {
                push_token(&mut tokens, &current);
                current.clear();
            }
            current_is_cjk = is_cjk;
            current.push(ch);
        } else {
            push_token(&mut tokens, &current);
            current.clear();
        }
    }
    push_token(&mut tokens, &current);
    tokens
}

fn push_token(tokens: &mut BTreeSet<String>, token: &str) {
    let token = token.trim().to_ascii_lowercase();
    if token.chars().count() >= 2 {
        tokens.insert(token);
    }
}

fn keyword_downstream_rate(root: &Path, tokens: &BTreeSet<String>) -> AdmResult<f64> {
    if tokens.is_empty() {
        return Ok(0.0);
    }
    let mut downstream = String::new();
    for stage in [3_u32, 4, 8, 9, 10] {
        let stage_dir = root.join(format!("stage_{stage:02}"));
        collect_text_under(&stage_dir, &mut downstream)?;
    }
    let downstream = downstream.to_ascii_lowercase();
    let hits = tokens
        .iter()
        .filter(|token| downstream.contains(token.as_str()))
        .count();
    Ok(hits as f64 / tokens.len() as f64)
}

fn template_leakage_count(root: &Path) -> AdmResult<usize> {
    let mut count = 0usize;
    for stage in SEMANTIC_STAGES {
        let stage_dir = root.join(format!("stage_{stage:02}"));
        count += count_markers_under(&stage_dir, TEMPLATE_LEAKAGE_MARKERS)?;
    }
    Ok(count)
}

fn project_signature_report(root: &Path) -> AdmResult<Value> {
    let mut signatures = BTreeMap::<String, Vec<String>>::new();
    for stage in SEMANTIC_STAGES {
        let stage_dir = root.join(format!("stage_{stage:02}"));
        collect_project_signatures(root, &stage_dir, &mut signatures)?;
    }
    Ok(json!({
        "signature_count": signatures.len(),
        "signatures": signatures,
        "cross_context_count": signatures.len().saturating_sub(1),
    }))
}

fn collect_project_signatures(
    root: &Path,
    current: &Path,
    signatures: &mut BTreeMap<String, Vec<String>>,
) -> AdmResult<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_project_signatures(root, &path, signatures)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let data = read_json(&path, json!({}));
        if let Some(signature) = data.get("project_signature").and_then(Value::as_str)
            && !signature.trim().is_empty()
        {
            signatures.entry(signature.to_string()).or_default().push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

fn collect_text_under(dir: &Path, output: &mut String) -> AdmResult<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_text_under(&path, output)?;
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("json" | "md" | "txt")
        ) {
            output.push(' ');
            output.push_str(&fs::read_to_string(path).unwrap_or_default());
        }
    }
    Ok(())
}

fn count_markers_under(dir: &Path, markers: &[&str]) -> AdmResult<usize> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut text = String::new();
    collect_text_under(dir, &mut text)?;
    Ok(markers
        .iter()
        .filter(|marker| text.contains(**marker))
        .count())
}

fn ratio(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

fn stage_score(
    nullish_ratio: f64,
    generic_ratio: f64,
    placeholder_ratio: f64,
    leakage: usize,
) -> f64 {
    let mut score = 10.0;
    score -= (nullish_ratio * 20.0).min(3.0);
    score -= (generic_ratio * 4.0).min(3.0);
    score -= (placeholder_ratio * 4.0).min(3.0);
    if leakage > 0 {
        score -= (leakage as f64 * 0.5).min(2.0);
    }
    round2(score.max(0.0))
}

fn number_or(value: &Value, key: &str, fallback: f64) -> f64 {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_i64().map(|item| item as f64))
        })
        .unwrap_or(fallback)
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|item| item as f64))
    })
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .map(scalar_text)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn round4(value: f64) -> f64 {
    (value * 10000.0).round() / 10000.0
}

trait EmptyDefault {
    fn if_empty(self, default: &str) -> String;
}

impl EmptyDefault for String {
    fn if_empty(self, default: &str) -> String {
        if self.trim().is_empty() {
            default.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn config_validator_matches_python_required_type_unique_and_range_rules() {
        let root = temp_root("config");
        let schema = root.join("schema.json");
        let tables = root.join("tables");
        fs::create_dir_all(&tables).unwrap();
        fs::write(
            &schema,
            r#"{"tables":[{"name":"items","columns":[{"name":"id","type":"int","required":true,"unique":true},{"name":"speed","type":"float","min":0,"max":10},{"name":"enabled","type":"bool"},{"name":"old","deprecated":"use id"}]}]}"#,
        )
        .unwrap();
        fs::write(
            tables.join("items.csv"),
            "id,speed,enabled,old\n1,4.5,true,x\n1,20,maybe,y\n",
        )
        .unwrap();

        let report = validate_config_tables(&schema, &tables).unwrap();

        assert_eq!(report.status, "FAIL");
        assert!(report.tables[0].warnings[0].contains("已废弃"));
        assert!(
            report.tables[0]
                .errors
                .iter()
                .any(|error| error.contains("唯一约束"))
        );
        assert!(
            report.tables[0]
                .errors
                .iter()
                .any(|error| error.contains("期望 bool"))
        );
        cleanup(root);
    }

    #[test]
    fn context_lint_requires_sections_terms_and_adr_coverage() {
        let root = temp_root("context");
        let path = root.join("CONTEXT.md");
        fs::write(
            &path,
            REQUIRED_CONTEXT_SECTIONS
                .iter()
                .map(|section| format!("## {section}\n**Term**: definition\n_ADR_: adr-1\n"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();

        let report = lint_context_file(&path).unwrap();

        assert!(report.valid, "{:?}", report.errors);
        assert_eq!(report.terms, REQUIRED_CONTEXT_SECTIONS.len());
        cleanup(root);
    }

    #[test]
    fn contract_and_output_validators_report_failures() {
        let root = temp_root("contract");
        let contract = root.join("contract.json");
        let schema = root.join("schema.json");
        let report_path = root.join("report.json");
        fs::write(&contract, r#"{"name":1}"#).unwrap();
        fs::write(
            &schema,
            r#"{"type":"object","required":["name","items"],"properties":{"name":{"type":"string"}}}"#,
        )
        .unwrap();

        let report = validate_contract_file_report(&contract, &schema, Some(&report_path)).unwrap();
        assert!(!report.valid);
        assert!(report_path.exists());

        let rejected = validate_agent_output("sorry", "markdown", &[], "Agent output");
        assert!(!rejected.valid);
        let parsed = validate_agent_output(
            "```json\n{\"ok\":true}\n```",
            "json",
            &["ok".to_string()],
            "Agent output",
        );
        assert!(parsed.valid, "{:?}", parsed.errors);
        cleanup(root);
    }

    #[test]
    fn pipeline_quality_plan_002_check_uses_stage_metrics() {
        let root = temp_root("pipeline_quality");
        fs::create_dir_all(root.join("stage_02")).unwrap();
        fs::write(
            root.join("stage_02/entity_coverage_report.json"),
            r#"{"entity_coverage_rate":0.4,"entity_count":4}"#,
        )
        .unwrap();

        let report = check_pipeline_plan_002(&root);

        assert_eq!(report["checks"]["plan-002"]["passed"], true);
        cleanup(root);
    }

    #[test]
    fn design_semantic_quality_detects_cross_context_and_generic_tasks() {
        let root = temp_root("semantic_quality");
        fs::create_dir_all(root.join("stage_00")).unwrap();
        fs::create_dir_all(root.join("stage_08")).unwrap();
        fs::write(
            root.join("stage_00/concept_profile.json"),
            r#"{"project_name":"Moon Tower","project_signature":"sig-a"}"#,
        )
        .unwrap();
        fs::write(
            root.join("stage_08/program_task_breakdown.json"),
            r#"{"project_signature":"sig-b","tasks":[{"task_id":"DEV-001","title":"Feature","project_semantic_refs":[]}]}"#,
        )
        .unwrap();

        let report = collect_design_semantic_quality(&root).unwrap();
        let markdown = render_design_semantic_markdown(&report);

        assert_eq!(report["project_signature_report"]["cross_context_count"], 1);
        assert!(!report["blocking_issues"].as_array().unwrap().is_empty());
        assert!(markdown.contains("Design Semantic Quality Report"));
        cleanup(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "adm_new_validation_{label}_{}",
            new_stable_id("root").unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
