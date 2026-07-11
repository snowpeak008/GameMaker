use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use adm_new_foundation::io::{write_json_serializable, write_text};
use adm_new_foundation::structured_md::{read_structured_or_text, write_data};
use adm_new_foundation::{AdmError, AdmResult, sanitize_identifier, unix_timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const LIFECYCLE_METHODS: [&str; 7] = [
    "void Update()",
    "void FixedUpdate()",
    "void LateUpdate()",
    "void Awake()",
    "void Start()",
    "void OnEnable()",
    "void OnDisable()",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledTable {
    pub table: String,
    pub data_file: PathBuf,
    pub code_file: PathBuf,
    pub row_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenReport {
    pub generated_files: Vec<PathBuf>,
    pub modified_files: Vec<PathBuf>,
    pub skipped_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectScaffoldReport {
    pub root: PathBuf,
    pub created_dirs: Vec<PathBuf>,
    pub created_files: Vec<PathBuf>,
    pub skipped_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepScaffoldReport {
    pub step_dir: PathBuf,
    pub created_files: Vec<PathBuf>,
    pub skipped_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestGenerationReport {
    pub modules: Vec<String>,
    pub generated_files: Vec<PathBuf>,
    pub results_path: PathBuf,
    pub summary: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiStateGenerationReport {
    pub manager_path: PathBuf,
    pub state_doc_path: PathBuf,
    pub panel_count: usize,
    pub state_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalGitCommandSpec {
    pub allowed: bool,
    pub program: String,
    pub args: Vec<String>,
    pub work_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileCheckPlan {
    pub status: String,
    pub command: String,
    pub work_dir: PathBuf,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileCheckEvaluation {
    pub status: String,
    pub exit_code: i32,
    pub error_detected: bool,
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentCheckResult {
    pub name: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentCheckReport {
    pub ok: bool,
    pub results: Vec<EnvironmentCheckResult>,
}

pub fn compile_all_config(
    schema_path: impl AsRef<Path>,
    tables_dir: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> AdmResult<Vec<CompiledTable>> {
    let schema = read_structured_or_text(schema_path.as_ref())?;
    let tables = schema
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| AdmError::new("config schema must contain a tables array"))?;
    let mut compiled = Vec::new();
    for table in tables {
        let name = string_field(table, "name");
        if name.is_empty() {
            continue;
        }
        let csv_path = tables_dir.as_ref().join(format!("{name}.csv"));
        if !csv_path.exists() {
            continue;
        }
        compiled.push(compile_table(table, &csv_path, output_dir.as_ref())?);
    }
    Ok(compiled)
}

pub fn compile_table(
    table_def: &Value,
    csv_path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> AdmResult<CompiledTable> {
    let table_name = string_field(table_def, "name");
    if table_name.is_empty() {
        return Err(AdmError::new("table name is required"));
    }
    let columns = table_def
        .get("columns")
        .and_then(Value::as_array)
        .ok_or_else(|| AdmError::new("table columns are required"))?;
    let rows = parse_csv_table(csv_path.as_ref())?;
    let mut data = Vec::new();
    for row in rows {
        let mut item = serde_json::Map::new();
        for column in columns {
            let name = string_field(column, "name");
            if name.is_empty() {
                continue;
            }
            let column_type = string_field(column, "type")
                .if_empty("string")
                .to_ascii_lowercase();
            let raw = row.get(&name).cloned().unwrap_or_default();
            item.insert(name, typed_csv_value(&raw, &column_type)?);
        }
        data.push(Value::Object(item));
    }
    fs::create_dir_all(output_dir.as_ref())?;
    let data_file = output_dir.as_ref().join(format!("{table_name}.json"));
    write_json_serializable(&data_file, &data)?;
    let code_file = output_dir
        .as_ref()
        .join(format!("{}Data.cs", capitalize_identifier(&table_name)));
    write_text(
        &code_file,
        &render_csharp_table_struct(&table_name, columns),
    )?;
    Ok(CompiledTable {
        table: table_name,
        data_file,
        code_file,
        row_count: data.len(),
    })
}

pub fn generate_error_logger(output_dir: impl AsRef<Path>) -> AdmResult<PathBuf> {
    let path = output_dir.as_ref().join("ErrorLogger.cs");
    write_text(&path, error_logger_source())?;
    Ok(path)
}

pub fn run_error_logger_pipeline(
    source_dir: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> AdmResult<CodegenReport> {
    let logger = generate_error_logger(output_dir)?;
    let (modified_files, skipped_files) = wrap_lifecycle_methods(
        source_dir.as_ref(),
        "ErrorLogger.Log",
        WrapperKind::ErrorLogger,
    )?;
    Ok(CodegenReport {
        generated_files: vec![logger],
        modified_files,
        skipped_files,
    })
}

pub fn run_perf_pipeline(
    source_dir: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> AdmResult<CodegenReport> {
    let monitor = output_dir.as_ref().join("PerfMonitor.cs");
    let hud = output_dir.as_ref().join("PerfHUD.cs");
    write_text(&monitor, perf_monitor_source())?;
    write_text(&hud, perf_hud_source())?;
    let (modified_files, skipped_files) = wrap_lifecycle_methods(
        source_dir.as_ref(),
        "PerfMonitor.BeginSample",
        WrapperKind::PerfMonitor,
    )?;
    Ok(CodegenReport {
        generated_files: vec![monitor, hud],
        modified_files,
        skipped_files,
    })
}

pub fn scaffold_project(root: impl AsRef<Path>) -> AdmResult<ProjectScaffoldReport> {
    let root = root.as_ref();
    let dirs = [
        "Docs/governance",
        "tools",
        "source_artifacts",
        "source_artifacts/.snapshots",
        "outputs/artifacts",
    ];
    let mut created_dirs = Vec::new();
    for dir in dirs {
        let path = root.join(dir);
        if !path.exists() {
            fs::create_dir_all(&path)?;
            created_dirs.push(path);
        }
    }
    let templates = project_templates();
    let mut created_files = Vec::new();
    let mut skipped_files = Vec::new();
    for (relative, content) in templates {
        let path = root.join(relative);
        if path.exists() {
            skipped_files.push(path);
            continue;
        }
        write_text(&path, content)?;
        created_files.push(path);
    }
    Ok(ProjectScaffoldReport {
        root: root.to_path_buf(),
        created_dirs,
        created_files,
        skipped_files,
    })
}

pub fn scaffold_step(
    project_root: impl AsRef<Path>,
    step: u32,
    name: &str,
    force: bool,
) -> AdmResult<StepScaffoldReport> {
    let slug = slugify(name)?;
    let step_dir = project_root
        .as_ref()
        .join("pipeline")
        .join(format!("step_{step:02}_{slug}"));
    if step_dir.exists() && !force {
        return Err(AdmError::new(format!(
            "{} already exists; pass force to add missing files",
            step_dir.display()
        )));
    }
    fs::create_dir_all(step_dir.join("data"))?;
    fs::create_dir_all(step_dir.join("prompts"))?;
    let files = [
        (step_dir.join("__init__.py"), String::new()),
        (step_dir.join("plugin.py"), plugin_template(step)),
        (step_dir.join("helpers.py"), helpers_template().to_string()),
        (
            step_dir.join("prompts/README.md"),
            "# Prompts\n".to_string(),
        ),
        (step_dir.join("data/README.md"), "# Data\n".to_string()),
    ];
    let mut created_files = Vec::new();
    let mut skipped_files = Vec::new();
    for (path, content) in files {
        if path.exists() && !force {
            skipped_files.push(path);
            continue;
        }
        write_text(&path, &content)?;
        created_files.push(path);
    }
    Ok(StepScaffoldReport {
        step_dir,
        created_files,
        skipped_files,
    })
}

pub fn run_test_generation_pipeline(
    plans_dir: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> AdmResult<TestGenerationReport> {
    let modules = load_modules_from_plans(plans_dir.as_ref())?;
    if modules.is_empty() {
        return Err(AdmError::new("no modules found for test generation"));
    }
    fs::create_dir_all(output_dir.as_ref())?;
    let mut generated_files = Vec::new();
    let mut results = Vec::new();
    for module in &modules {
        let safe = sanitize_identifier(module).unwrap_or_else(|_| "module".to_string());
        let path = output_dir.as_ref().join(format!("test_{safe}.cs"));
        write_text(&path, &test_source(module))?;
        generated_files.push(path);
        results.push(json!({
            "module": module,
            "tests_total": 0,
            "passed": 0,
            "failed": 0,
            "status": "SKIPPED",
            "message": "Test cases generated; real Unity test runner is not configured.",
        }));
    }
    let summary = json!({
        "total_modules": modules.len(),
        "passed_modules": 0,
        "failed_modules": 0,
        "skipped_modules": modules.len(),
        "overall_status": "SKIPPED",
    });
    let report = json!({
        "results": results,
        "summary": summary,
    });
    let results_path = output_dir.as_ref().join("test_results.md");
    write_data(&results_path, &report, "Test Results")?;
    Ok(TestGenerationReport {
        modules,
        generated_files,
        results_path,
        summary,
    })
}

pub fn generate_ui_state_artifacts(
    graph_path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> AdmResult<UiStateGenerationReport> {
    let graph = read_structured_or_text(graph_path.as_ref())?;
    let panels = graph
        .get("registry")
        .and_then(|registry| registry.get("panels"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let states = graph
        .get("graph")
        .and_then(|graph| graph.get("states"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    fs::create_dir_all(output_dir.as_ref())?;
    let manager_path = output_dir.as_ref().join("UIManager.cs");
    let state_doc_path = output_dir.as_ref().join("UIStateMachine.md");
    write_text(&manager_path, &ui_manager_source(&panels, &states))?;
    write_text(&state_doc_path, &ui_state_doc(&graph, &states))?;
    Ok(UiStateGenerationReport {
        manager_path,
        state_doc_path,
        panel_count: panels.len(),
        state_count: states.len(),
    })
}

pub fn local_git_command_spec(
    command: &[String],
    work_dir: impl AsRef<Path>,
) -> AdmResult<LocalGitCommandSpec> {
    if command.len() < 2 || command.first().map(String::as_str) != Some("git") {
        return Err(AdmError::new("git command must start with git"));
    }
    let allowed = ["init", "add", "commit", "tag", "status", "log"];
    let subcommand = command[1].as_str();
    if !allowed.contains(&subcommand) {
        return Err(AdmError::new(
            "forbidden git command; only local init/add/commit/tag/status/log are allowed",
        ));
    }
    Ok(LocalGitCommandSpec {
        allowed: true,
        program: "git".to_string(),
        args: command.iter().skip(1).cloned().collect(),
        work_dir: work_dir.as_ref().to_path_buf(),
    })
}

pub fn compile_check_plan(work_dir: impl AsRef<Path>, command: Option<&str>) -> CompileCheckPlan {
    let work_dir = work_dir.as_ref();
    let command = command
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "Unity -batchmode -quit -projectPath \"{}\" -executeMethod BuildPipeline.BuildPlayer",
                work_dir.display()
            )
        });
    CompileCheckPlan {
        status: "planned".to_string(),
        command,
        work_dir: work_dir.to_path_buf(),
        timeout_seconds: 300,
    }
}

pub fn evaluate_compile_output(
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> CompileCheckEvaluation {
    let error_detected = stdout.to_ascii_lowercase().contains("error")
        || stderr.to_ascii_lowercase().contains("error");
    CompileCheckEvaluation {
        status: if exit_code == 0 && !error_detected {
            "PASS".to_string()
        } else {
            "FAIL".to_string()
        },
        exit_code,
        error_detected,
        stdout_excerpt: excerpt(stdout, 1200),
        stderr_excerpt: excerpt(stderr, 1200),
    }
}

pub fn check_environment_config(path: impl AsRef<Path>) -> AdmResult<EnvironmentCheckReport> {
    let config = read_structured_or_text(path.as_ref())?;
    let mut results = Vec::new();
    if let Some(engine) = config.get("engine").and_then(Value::as_object) {
        let name = engine
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unity");
        if name == "Unity" {
            let available = command_available("Unity");
            results.push(EnvironmentCheckResult {
                name: "Unity".to_string(),
                status: if available { "PASS" } else { "FAIL" }.to_string(),
                message: if available {
                    "Unity command is available".to_string()
                } else {
                    "Unity command was not found on PATH".to_string()
                },
            });
        }
    }
    for sdk in string_array(config.get("sdks")) {
        if sdk.to_ascii_lowercase().contains("net") {
            let available = command_available("dotnet");
            results.push(EnvironmentCheckResult {
                name: ".NET SDK".to_string(),
                status: if available { "PASS" } else { "FAIL" }.to_string(),
                message: if available {
                    "dotnet command is available".to_string()
                } else {
                    "dotnet command was not found on PATH".to_string()
                },
            });
        }
    }
    for package in string_array(config.get("python")) {
        results.push(EnvironmentCheckResult {
            name: format!("python:{package}"),
            status: "SKIPPED".to_string(),
            message: "Python package installation is reported but not auto-installed by Rust gate."
                .to_string(),
        });
    }
    for tool in string_array(config.get("tools")) {
        let available = command_available(&tool);
        results.push(EnvironmentCheckResult {
            name: format!("tool:{tool}"),
            status: if available { "PASS" } else { "FAIL" }.to_string(),
            message: if available {
                format!("{tool} is available")
            } else {
                format!("{tool} was not found on PATH")
            },
        });
    }
    let ok = results
        .iter()
        .all(|result| result.status == "PASS" || result.status == "SKIPPED");
    Ok(EnvironmentCheckReport { ok, results })
}

fn parse_csv_table(path: &Path) -> AdmResult<Vec<BTreeMap<String, String>>> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Ok(Vec::new());
    };
    let headers = parse_csv_line(header)
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            if index == 0 {
                value.trim_start_matches('\u{feff}').to_string()
            } else {
                value
            }
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let cells = parse_csv_line(line);
        let mut row = BTreeMap::new();
        for (index, header) in headers.iter().enumerate() {
            row.insert(
                header.clone(),
                cells.get(index).cloned().unwrap_or_default(),
            );
        }
        rows.push(row);
    }
    Ok(rows)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                cells.push(cell.trim().to_string());
                cell.clear();
            }
            _ => cell.push(ch),
        }
    }
    cells.push(cell.trim().to_string());
    cells
}

fn typed_csv_value(raw: &str, column_type: &str) -> AdmResult<Value> {
    match column_type {
        "int" => Ok(json!(raw.parse::<i64>().unwrap_or(0))),
        "float" => Ok(json!(raw.parse::<f64>().unwrap_or(0.0))),
        "bool" => Ok(json!(matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "true" | "1"
        ))),
        "string" | "" => Ok(json!(raw)),
        other => Err(AdmError::new(format!("unsupported column type: {other}"))),
    }
}

fn render_csharp_table_struct(table_name: &str, columns: &[Value]) -> String {
    let class_name = format!("{}Data", capitalize_identifier(table_name));
    let mut code = String::new();
    code.push_str("// Auto-generated. Do not edit by hand.\n");
    code.push_str("using System;\n\n");
    code.push_str("[Serializable]\n");
    code.push_str(&format!("public struct {class_name}\n{{\n"));
    for column in columns {
        let name = string_field(column, "name");
        if name.is_empty() {
            continue;
        }
        let ty = match string_field(column, "type").as_str() {
            "int" => "int",
            "float" => "float",
            "bool" => "bool",
            _ => "string",
        };
        code.push_str(&format!("    public {ty} {name};\n"));
    }
    code.push_str("}\n");
    code
}

#[derive(Debug, Clone, Copy)]
enum WrapperKind {
    ErrorLogger,
    PerfMonitor,
}

fn wrap_lifecycle_methods(
    source_dir: &Path,
    existing_marker: &str,
    kind: WrapperKind,
) -> AdmResult<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut files = Vec::new();
    collect_cs_files(source_dir, &mut files)?;
    files.sort();
    let mut modified = Vec::new();
    let mut skipped = Vec::new();
    for path in files {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if matches!(
            name,
            "ErrorLogger.cs" | "PerfMonitor.cs" | "PerfHUD.cs" | "UIManager.cs"
        ) {
            skipped.push(path);
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if content.contains(existing_marker) {
            skipped.push(path);
            continue;
        }
        let ranges = method_ranges(&content);
        if ranges.is_empty() {
            skipped.push(path);
            continue;
        }
        let source_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Script");
        let mut next = content.clone();
        for range in ranges.iter().rev() {
            let replacement = wrapped_method(&content, range, source_name, kind);
            next.replace_range(range.open_brace..=range.close_brace, &replacement);
        }
        if matches!(kind, WrapperKind::ErrorLogger) && !next.contains("using System;") {
            next = format!("using System;\n{next}");
        }
        fs::write(&path, next)?;
        modified.push(path);
    }
    Ok((modified, skipped))
}

#[derive(Debug, Clone)]
struct MethodRange {
    signature: String,
    open_brace: usize,
    close_brace: usize,
}

fn method_ranges(content: &str) -> Vec<MethodRange> {
    let mut ranges = Vec::new();
    for signature in LIFECYCLE_METHODS {
        let mut offset = 0usize;
        while let Some(relative) = content[offset..].find(signature) {
            let start = offset + relative;
            let Some(open_relative) = content[start..].find('{') else {
                break;
            };
            let open_brace = start + open_relative;
            let Some(close_brace) = find_matching_brace(content, open_brace) else {
                break;
            };
            ranges.push(MethodRange {
                signature: signature.to_string(),
                open_brace,
                close_brace,
            });
            offset = close_brace + 1;
        }
    }
    ranges.sort_by_key(|range| range.open_brace);
    ranges
}

fn find_matching_brace(content: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in content[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_brace + index);
                }
            }
            _ => {}
        }
    }
    None
}

fn wrapped_method(
    content: &str,
    range: &MethodRange,
    source_name: &str,
    kind: WrapperKind,
) -> String {
    let body = &content[range.open_brace + 1..range.close_brace];
    let method = range
        .signature
        .trim_start_matches("void ")
        .trim_end_matches("()")
        .to_string();
    match kind {
        WrapperKind::ErrorLogger => format!(
            "{{\n        try {{{body}\n        }}\n        catch (Exception ex) {{\n            ErrorLogger.LogFatal(\"{source_name}.{method}\", ex.ToString());\n            throw;\n        }}\n    }}"
        ),
        WrapperKind::PerfMonitor => format!(
            "{{\n        using (PerfMonitor.BeginSample(\"{source_name}.{method}\")) {{{body}\n        }}\n    }}"
        ),
    }
}

fn collect_cs_files(root: &Path, files: &mut Vec<PathBuf>) -> AdmResult<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_cs_files(&path, files)?;
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("cs"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn load_modules_from_plans(plans_dir: &Path) -> AdmResult<Vec<String>> {
    if !plans_dir.exists() {
        return Ok(Vec::new());
    }
    let mut modules = Vec::new();
    collect_markdown_stems(plans_dir, &mut modules)?;
    modules.sort();
    modules.dedup();
    Ok(modules)
}

fn collect_markdown_stems(root: &Path, modules: &mut Vec<String>) -> AdmResult<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_stems(&path, modules)?;
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            && let Some(stem) = path.file_stem().and_then(|value| value.to_str())
        {
            modules.push(stem.to_string());
        }
    }
    Ok(())
}

fn command_available(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions = if cfg!(windows) {
        vec![".exe", ".cmd", ".bat", ""]
    } else {
        vec![""]
    };
    std::env::split_paths(&paths).any(|dir| {
        extensions
            .iter()
            .any(|extension| dir.join(format!("{command}{extension}")).is_file())
    })
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn project_templates() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        (
            ".gitignore",
            "# API secrets\napi_config.md\n\n# Pipeline artifacts\nsource_artifacts/\n\n# Python\n__pycache__/\n*.pyc\nvenv/\n\n# Logs\nlogs/\n*.log\n\n# Build artifacts\nBuild/\n",
        ),
        (
            "api_config.template.md",
            "# API Configuration Template\n\n```yaml\nproviders:\n  llm:\n    provider: openai\n    api_key: \"\"\n    base_url: https://vip.auto-code.net/v1\n    default_model: gpt-5.5\n  image2:\n    api_key: \"\"\n    base_url: https://vip.auto-code.net/v1\n    default_model: gpt-image-2\nproject:\n  dev_work_dir: D:/YourGame/Project\n```\n",
        ),
        (
            "my_game_idea.txt",
            "# Describe the core game idea here.\n# Include player action, emotional target, world constraints, and forbidden styles.\n",
        ),
    ])
}

fn plugin_template(step: u32) -> String {
    format!(
        r#"from __future__ import annotations

from core.context import StageContext, StageResult
from core.engines.generation import apply_development_plan_outputs
from core.source.groups import SourceGroup
from core.source.importer import run_import_step
from core.stage_plugin import StagePlugin


class Plugin(StagePlugin):
    stage_id = "{step:02}"
    _source_groups = [
        SourceGroup("design", ("devflow_*",), "latest", True, ("Concept", "Design"))
    ]

    def execute(self, ctx: StageContext) -> StageResult:
        if ctx.test_mode:
            return StageResult(status="success", outputs={{"stage_id": self.stage_id}})
        report = run_import_step(int(self.stage_id), self._source_groups, context=ctx)
        result = apply_development_plan_outputs(int(self.stage_id), report)
        return StageResult(status=result.get("status", "success"), outputs=result)
"#
    )
}

fn helpers_template() -> &'static str {
    r#"from __future__ import annotations

from typing import Any


def build_report(parsed: dict[str, Any]) -> dict[str, Any]:
    """Build this step's structured report."""
    return {"schema_version": 1, "source": str(parsed.get("source", ""))}
"#
}

fn test_source(module: &str) -> String {
    let name = sanitize_identifier(module).unwrap_or_else(|_| "Module".to_string());
    format!(
        r#"// Auto-generated Unity test placeholder for {module}.
using NUnit.Framework;
using UnityEngine;
using UnityEngine.TestTools;

public class Test{name}
{{
    [SetUp]
    public void Setup()
    {{
    }}

    [TearDown]
    public void Teardown()
    {{
    }}

    [Test]
    public void Test_NormalCase_01()
    {{
        Assert.Inconclusive("Real assertion is not connected yet.");
    }}

    [Test]
    public void Test_Boundary_ZeroInput()
    {{
        Assert.Inconclusive("Real assertion is not connected yet.");
    }}

    [Test]
    public void Test_Exception_NullInput()
    {{
        Assert.Inconclusive("Real assertion is not connected yet.");
    }}
}}
"#
    )
}

fn ui_manager_source(panels: &[Value], states: &serde_json::Map<String, Value>) -> String {
    let known_panels = panels
        .iter()
        .filter_map(Value::as_str)
        .map(|panel| format!("        \"{panel}\","))
        .collect::<Vec<_>>()
        .join("\n");
    let known_states = states
        .keys()
        .map(|state| format!("        \"{state}\","))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"// Auto-generated. Do not edit by hand.
using System.Collections.Generic;
using UnityEngine;

public class UIManager : MonoBehaviour
{{
    private static UIManager _instance;
    public static UIManager Instance => _instance;

    private Dictionary<string, GameObject> _panels = new Dictionary<string, GameObject>();
    private Stack<string> _popupStack = new Stack<string>();
    private string _currentScreen = null;

    public static readonly string[] KnownPanels = new string[]
    {{
{known_panels}
    }};

    public static readonly string[] KnownStates = new string[]
    {{
{known_states}
    }};

    void Awake()
    {{
        if (_instance != null)
        {{
            Destroy(gameObject);
            return;
        }}
        _instance = this;
        DontDestroyOnLoad(gameObject);
    }}

    public void RegisterPanel(string id, GameObject panel)
    {{
        if (!_panels.ContainsKey(id))
        {{
            _panels[id] = panel;
            panel.SetActive(false);
        }}
    }}

    public void OpenPanel(string id)
    {{
        if (!_panels.ContainsKey(id))
        {{
            Debug.LogError("UIManager: unregistered panel " + id);
            return;
        }}
        _panels[id].SetActive(true);
        _currentScreen = id;
    }}

    public void ClosePanel(string id)
    {{
        if (_panels.ContainsKey(id))
            _panels[id].SetActive(false);
    }}
}}
"#
    )
}

fn ui_state_doc(graph: &Value, states: &serde_json::Map<String, Value>) -> String {
    let mut output = format!(
        "# UI State Machine\n\nGenerated at: unix:{}\n\n",
        unix_timestamp()
    );
    output.push_str(
        "## States\n\n| State ID | Layer | Input Mode | Transitions |\n| --- | --- | --- | --- |\n",
    );
    for (state_id, state) in states {
        let layer = string_field(state, "layer").if_empty("-");
        let input_mode = string_field(state, "input_mode").if_empty("-");
        let transitions = state
            .get("transitions")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        let to = string_field(item, "to");
                        let kind = string_field(item, "type");
                        if kind.is_empty() {
                            to
                        } else {
                            format!("{to}({kind})")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "-".to_string());
        output.push_str(&format!(
            "| {state_id} | {layer} | {input_mode} | {transitions} |\n"
        ));
    }
    if let Some(layers) = graph.get("layers").and_then(Value::as_array) {
        output.push_str("\n## Layers\n\n");
        for layer in layers {
            output.push_str(&format!("- {}\n", string_field(layer, "id")));
        }
    }
    output
}

fn error_logger_source() -> &'static str {
    r#"// Auto-generated error logger.
using System;
using System.IO;

public enum ErrorLevel
{
    Info,
    Warning,
    Error,
    Fatal
}

public static class ErrorLogger
{
    private static string _logDir = "logs";
    private static string _currentLogFile = Path.Combine(_logDir, "error.log");

    static ErrorLogger()
    {
        Directory.CreateDirectory(_logDir);
    }

    public static void LogFatal(string source, string message)
    {
        WriteLog(ErrorLevel.Fatal, source, message);
    }

    public static void LogError(string source, string message)
    {
        WriteLog(ErrorLevel.Error, source, message);
    }

    public static void LogWarning(string source, string message)
    {
        WriteLog(ErrorLevel.Warning, source, message);
    }

    public static void LogInfo(string source, string message)
    {
        WriteLog(ErrorLevel.Info, source, message);
    }

    private static void WriteLog(ErrorLevel level, string source, string message)
    {
        string line = "[" + DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss") + "] " + level.ToString().ToUpper() + " | " + source + ": " + message;
        try
        {
            File.AppendAllText(_currentLogFile, line + Environment.NewLine);
        }
        catch
        {
        }
    }
}
"#
}

fn perf_monitor_source() -> &'static str {
    r#"// Auto-generated performance monitor.
using System;
using System.Diagnostics;
using System.IO;
using System.Collections.Generic;

public static class PerfMonitor
{
    private static Dictionary<string, Stopwatch> _watches = new Dictionary<string, Stopwatch>();
    private static string _reportPath = "logs/perf_report.csv";

    static PerfMonitor()
    {
        Directory.CreateDirectory("logs");
        if (!File.Exists(_reportPath))
            File.WriteAllText(_reportPath, "Timestamp,Metric,Value,Unit\n");
    }

    public static IDisposable BeginSample(string name)
    {
        if (!_watches.ContainsKey(name))
            _watches[name] = new Stopwatch();
        _watches[name].Start();
        return new SampleDisposable(name);
    }

    private class SampleDisposable : IDisposable
    {
        private string _name;
        public SampleDisposable(string name) { _name = name; }
        public void Dispose()
        {
            if (PerfMonitor._watches.ContainsKey(_name))
            {
                var watch = PerfMonitor._watches[_name];
                watch.Stop();
                PerfMonitor.RecordMetric(_name + "_ms", watch.ElapsedMilliseconds, "ms");
                watch.Reset();
            }
        }
    }

    public static void RecordMetric(string name, long value, string unit)
    {
        string line = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss") + "," + name + "," + value + "," + unit;
        File.AppendAllText(_reportPath, line + "\n");
    }
}
"#
}

fn perf_hud_source() -> &'static str {
    r#"// Auto-generated performance HUD.
using UnityEngine;

public class PerfHUD : MonoBehaviour
{
    private bool _show = false;
    private float _fps = 0;
    private long _memory = 0;

    void Update()
    {
        if (Input.GetKeyDown(KeyCode.F3))
            _show = !_show;
        _fps = 1.0f / Time.unscaledDeltaTime;
        _memory = System.GC.GetTotalMemory(false) / (1024 * 1024);
    }

    void OnGUI()
    {
        if (!_show)
            return;
        GUILayout.BeginArea(new Rect(10, 10, 220, 100));
        GUILayout.Label("FPS: " + _fps.ToString("0.0"));
        GUILayout.Label("Memory: " + _memory + " MB");
        GUILayout.EndArea();
    }
}
"#
}

fn slugify(value: &str) -> AdmResult<String> {
    sanitize_identifier(&value.trim().to_lowercase().replace([' ', '-'], "_"))
}

fn capitalize_identifier(value: &str) -> String {
    let clean = sanitize_identifier(value).unwrap_or_else(|_| "Data".to_string());
    let mut chars = clean.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => "Data".to_string(),
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
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

fn excerpt(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::{new_stable_id, unix_timestamp_millis};

    #[test]
    fn config_compiler_writes_typed_json_and_csharp_struct() {
        let root = temp_root("config");
        let schema = root.join("schema.json");
        let tables = root.join("tables");
        let out = root.join("out");
        fs::create_dir_all(&tables).unwrap();
        fs::write(
            &schema,
            r#"{"tables":[{"name":"items","columns":[{"name":"id","type":"int"},{"name":"speed","type":"float"},{"name":"enabled","type":"bool"},{"name":"label","type":"string"}]}]}"#,
        )
        .unwrap();
        fs::write(
            tables.join("items.csv"),
            "id,speed,enabled,label\n7,1.5,true,Sword",
        )
        .unwrap();

        let compiled = compile_all_config(&schema, &tables, &out).unwrap();

        assert_eq!(compiled.len(), 1);
        assert_eq!(compiled[0].row_count, 1);
        let data = fs::read_to_string(out.join("items.json")).unwrap();
        assert!(data.contains("\"id\": 7"));
        assert!(
            fs::read_to_string(out.join("ItemsData.cs"))
                .unwrap()
                .contains("public int id;")
        );
        cleanup(root);
    }

    #[test]
    fn error_and_perf_generators_write_tools_and_wrap_lifecycle_methods() {
        let root = temp_root("codegen");
        let source = root.join("Assets/Scripts");
        let generated = root.join("Generated");
        fs::create_dir_all(&source).unwrap();
        let script = source.join("Mover.cs");
        fs::write(
            &script,
            "public class Mover { void Update() { Tick(); } void Start() { Init(); } }",
        )
        .unwrap();

        let error = run_error_logger_pipeline(&source, &generated).unwrap();
        assert!(error.generated_files[0].exists());
        assert_eq!(error.modified_files, vec![script.clone()]);
        let wrapped = fs::read_to_string(&script).unwrap();
        assert!(wrapped.contains("ErrorLogger.LogFatal"));

        let perf = run_perf_pipeline(&source, &generated).unwrap();
        assert_eq!(perf.generated_files.len(), 2);
        let monitored = fs::read_to_string(&script).unwrap();
        assert!(monitored.contains("PerfMonitor.BeginSample"));
        cleanup(root);
    }

    #[test]
    fn scaffolds_project_step_tests_and_ui_state_artifacts() {
        let root = temp_root("scaffold");
        let project = root.join("project");
        let scaffold = scaffold_project(&project).unwrap();
        assert!(
            scaffold
                .created_dirs
                .iter()
                .any(|path| path.ends_with("outputs/artifacts"))
        );
        assert!(project.join("api_config.template.md").exists());

        let step = scaffold_step(&project, 16, "new stage", false).unwrap();
        assert!(step.step_dir.join("plugin.py").exists());

        let plans = root.join("plans");
        fs::create_dir_all(&plans).unwrap();
        fs::write(plans.join("combat.md"), "# Combat").unwrap();
        let tests = run_test_generation_pipeline(&plans, root.join("tests")).unwrap();
        assert_eq!(tests.modules, vec!["combat".to_string()]);
        assert!(tests.results_path.exists());

        let graph = root.join("ui_graph.json");
        fs::write(
            &graph,
            r#"{"registry":{"panels":["main"]},"graph":{"states":{"main":{"layer":"Screen","input_mode":"ui_only","transitions":[{"to":"pause","type":"open"}]}}},"layers":[{"id":"Screen"}]}"#,
        )
        .unwrap();
        let ui = generate_ui_state_artifacts(&graph, root.join("ui")).unwrap();
        assert_eq!(ui.panel_count, 1);
        assert_eq!(ui.state_count, 1);
        assert!(ui.manager_path.exists());
        assert!(ui.state_doc_path.exists());
        cleanup(root);
    }

    #[test]
    fn git_compile_and_environment_helpers_match_python_tool_policy() {
        let root = temp_root("validate");
        let git =
            local_git_command_spec(&["git".to_string(), "status".to_string()], &root).unwrap();
        assert_eq!(git.args, vec!["status".to_string()]);
        assert!(local_git_command_spec(&["git".to_string(), "push".to_string()], &root).is_err());

        let plan = compile_check_plan(&root, None);
        assert!(plan.command.contains("-batchmode"));
        assert_eq!(evaluate_compile_output(0, "Compiled", "").status, "PASS");
        assert_eq!(
            evaluate_compile_output(0, "error CS1000", "").status,
            "FAIL"
        );

        let config = root.join("env.json");
        fs::write(&config, r#"{"tools":[],"python":["pytest"]}"#).unwrap();
        let report = check_environment_config(&config).unwrap();
        assert!(report.ok);
        assert_eq!(report.results[0].status, "SKIPPED");
        cleanup(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "adm_new_dev_tools_{label}_{}_{}",
            unix_timestamp_millis(),
            new_stable_id("root").unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
