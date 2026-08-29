//! adm4 桌面壳（Slint）：GUI 无业务规则（D14），只调 adm4-app 服务并渲染结果。
//! 四块面板：结构化表格编辑器（工作台）、AI 访谈、模板逆向产线、冻结门明细。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod convert;

use adm4_app::{AppServices, InterviewTurnDto};
use adm4_authoring::{AuthoringEngine, InterviewProposal};
use adm4_decision::{
    AxisRef, DesignLevel, ParameterSchema, ParameterValues, PointApplicability, Provenance,
};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_template::{CertificationStatus, Template};
use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// 结构化编辑器当前形态（按当前选项的 parameter_schema 派生，None = 走 JSON 编辑）。
#[derive(Default, Clone, Copy, PartialEq)]
enum EditorKind {
    #[default]
    None,
    Table {
        columns: usize,
    },
    Matrix,
}

#[derive(Default)]
struct UiState {
    current_archive: Option<String>,
    current_domain: Option<String>,
    current_decision: Option<String>,
    /// 结构化编辑缓冲（UI 行模型，字符串格）；保存时按 schema 转回 ParameterValues。
    table_buffer: Vec<Vec<String>>,
    editor: EditorKind,
    /// 待确认的访谈提案：interview_next 的返回原样暂存，
    /// confirm/reject 只能由用户点击触发并把提案原样传回（D11）。
    pending_turn: Option<InterviewTurnDto>,
    reverse_pack: Option<String>,
    reverse_template: Option<String>,
}

fn main() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    let services = match AppServices::open(std::env::var("ADM4_DATA_ROOT").ok().map(PathBuf::from))
    {
        Ok(services) => Rc::new(services),
        Err(error) => {
            eprintln!("启动失败：{error}");
            return Ok(());
        }
    };
    let state = Rc::new(RefCell::new(UiState::default()));

    refresh_projects(&window, &services);
    refresh_packs(&window, &services);
    refresh_logs(&window, &services);
    window.set_stages(stage_placeholder());

    hook_project_callbacks(&window, &services, &state);
    hook_workbench_callbacks(&window, &services, &state);
    hook_editor_callbacks(&window, &services, &state);
    hook_interview_callbacks(&window, &services, &state);
    hook_freeze_pipeline_callbacks(&window, &services, &state);
    hook_reverse_callbacks(&window, &services, &state);
    window.run()
}

// ---------------------------------------------------------------------------
// 项目与日志
// ---------------------------------------------------------------------------

fn hook_project_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    {
        let services = services.clone();
        let weak = weak.clone();
        window.on_refresh_projects(move || {
            if let Some(window) = weak.upgrade() {
                refresh_projects(&window, &services);
            }
        });
    }
    {
        let services = services.clone();
        let weak = weak.clone();
        window.on_create_project(move |name, pack, depth| {
            if let Some(window) = weak.upgrade() {
                match parse_depth(depth.as_str()) {
                    Ok(level) => {
                        report(
                            &window,
                            services
                                .project_new(name.as_str(), pack.as_str(), level, None)
                                .map(|archive_id| format!("已创建项目 {archive_id}")),
                        );
                        refresh_projects(&window, &services);
                    }
                    Err(error) => report::<String>(&window, Err(error)),
                }
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_open_project(move |archive_id| {
            if let Some(window) = weak.upgrade() {
                {
                    let mut borrowed = state.borrow_mut();
                    borrowed.current_archive = Some(archive_id.to_string());
                    borrowed.current_domain = None;
                    borrowed.current_decision = None;
                    borrowed.pending_turn = None;
                }
                // 冻结门/访谈/对照展示都是项目相关状态，切项目时归零。
                window.set_freeze_ready(false);
                window.set_gate_rows(ModelRc::new(VecModel::from(Vec::<GateRow>::new())));
                window.set_structured_active(false);
                window.set_compare_rows(ModelRc::new(VecModel::from(Vec::<CompareRow>::new())));
                window.set_compare_title(SharedString::default());
                clear_interview_proposal(&window);
                report(&window, Ok(format!("已打开项目 {archive_id}")));
                refresh_workbench(&window, &services, &state);
                refresh_pipeline(&window, &services, &state);
                refresh_interview(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let weak = weak.clone();
        window.on_refresh_logs(move || {
            if let Some(window) = weak.upgrade() {
                refresh_logs(&window, &services);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// 设计工作台（领域/决策点/选项/参数）
// ---------------------------------------------------------------------------

fn hook_workbench_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_select_domain(move |domain| {
            if let Some(window) = weak.upgrade() {
                state.borrow_mut().current_domain = Some(domain.to_string());
                refresh_workbench(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_select_decision(move |decision| {
            if let Some(window) = weak.upgrade() {
                state.borrow_mut().current_decision = Some(decision.to_string());
                // 切换决策点时回到结构化默认视图（若适用）。
                window.set_advanced_mode(false);
                refresh_decision_panel(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_choose_option(move |option_id| {
            if let Some(window) = weak.upgrade() {
                let (archive, decision) = current_pair(&state);
                if let (Some(archive), Some(decision)) = (archive, decision) {
                    let result = services.with_project(&archive, |engine| {
                        engine.select_option(&decision, option_id.as_str(), Provenance::UserManual)
                    });
                    report(
                        &window,
                        result.map(|_| format!("已选择 {decision}/{option_id}")),
                    );
                    refresh_workbench(&window, &services, &state);
                    refresh_decision_panel(&window, &services, &state);
                }
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_save_params(move |json| {
            if let Some(window) = weak.upgrade() {
                let (archive, decision) = current_pair(&state);
                if let (Some(archive), Some(decision)) = (archive, decision) {
                    let result = services.with_project(&archive, |engine| {
                        let parameters: ParameterValues = serde_json::from_str(json.as_str())
                            .map_err(|error| {
                                Adm4Error::invalid_input(format!("参数 JSON 非法：{error}"))
                            })?;
                        engine.set_parameters(&decision, parameters)
                    });
                    report(&window, result.map(saved_message));
                    refresh_workbench(&window, &services, &state);
                    refresh_decision_panel(&window, &services, &state);
                }
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_confirm_decision(move || {
            if let Some(window) = weak.upgrade() {
                let (archive, decision) = current_pair(&state);
                if let (Some(archive), Some(decision)) = (archive, decision) {
                    let result = services
                        .with_project(&archive, |engine| engine.confirm_selection(&decision));
                    report(&window, result.map(|_| format!("已确认 {decision}")));
                    refresh_workbench(&window, &services, &state);
                    refresh_decision_panel(&window, &services, &state);
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// 结构化表格/矩阵编辑器（格编辑、行增删、保存、高级模式切换）
// ---------------------------------------------------------------------------

fn hook_editor_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    {
        let state = state.clone();
        window.on_edit_table_cell(move |row, col, text| {
            // 只写缓冲、不重建模型：保持正在输入的 LineEdit 焦点。
            let mut borrowed = state.borrow_mut();
            if let (Ok(row), Ok(col)) = (usize::try_from(row), usize::try_from(col))
                && let Some(row_buffer) = borrowed.table_buffer.get_mut(row)
                && let Some(cell) = row_buffer.get_mut(col)
            {
                *cell = text.to_string();
            }
        });
    }
    {
        let state = state.clone();
        let weak = weak.clone();
        window.on_add_table_row(move || {
            if let Some(window) = weak.upgrade() {
                let width = match state.borrow().editor {
                    EditorKind::Table { columns } => columns,
                    EditorKind::Matrix => 3,
                    EditorKind::None => return,
                };
                state
                    .borrow_mut()
                    .table_buffer
                    .push(vec![String::new(); width]);
                push_table_model(&window, &state.borrow().table_buffer);
            }
        });
    }
    {
        let state = state.clone();
        let weak = weak.clone();
        window.on_remove_table_row(move |index| {
            if let Some(window) = weak.upgrade() {
                if let Ok(index) = usize::try_from(index) {
                    let mut borrowed = state.borrow_mut();
                    if index < borrowed.table_buffer.len() {
                        borrowed.table_buffer.remove(index);
                    }
                }
                push_table_model(&window, &state.borrow().table_buffer);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_save_table(move || {
            if let Some(window) = weak.upgrade() {
                let (Some(archive), Some(decision)) = current_pair(&state) else {
                    report::<String>(
                        &window,
                        Err(Adm4Error::invalid_input("请先打开项目并选择决策点")),
                    );
                    return;
                };
                let buffer = state.borrow().table_buffer.clone();
                let result = services.with_project(&archive, |engine| {
                    let schema = lookup_selected_schema(engine, &decision)?;
                    let parameters = buffer_to_params(&schema, &buffer)?;
                    engine.set_parameters(&decision, parameters)
                });
                report(&window, result.map(saved_message));
                refresh_workbench(&window, &services, &state);
                refresh_decision_panel(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_set_advanced_mode(move |advanced| {
            if let Some(window) = weak.upgrade() {
                toggle_advanced_mode(&window, &services, &state, advanced);
            }
        });
    }
}

/// 表格 ↔ JSON 高级模式互切：两个方向都带着当前未保存的编辑走，
/// 转换失败（表格无法解析 / JSON 非法）则停留在原模式并报错——不丢数据不吞错。
fn toggle_advanced_mode(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
    advanced: bool,
) {
    let (Some(archive), Some(decision)) = current_pair(state) else {
        window.set_advanced_mode(advanced);
        return;
    };
    let schema = match services
        .open_engine(&archive)
        .and_then(|engine| lookup_selected_schema(&engine, &decision))
    {
        Ok(schema) => schema,
        Err(error) => {
            report::<String>(window, Err(error));
            return;
        }
    };
    if advanced {
        let buffer = state.borrow().table_buffer.clone();
        match buffer_to_params(&schema, &buffer)
            .and_then(|parameters| params_to_pretty_json(&parameters))
        {
            Ok(json) => {
                window.set_param_json(SharedString::from(json));
                window.set_advanced_mode(true);
            }
            Err(error) => report::<String>(
                window,
                Err(Adm4Error::invalid_input(format!(
                    "切换到高级模式失败，请先修正表格：{}",
                    error.message
                ))),
            ),
        }
    } else {
        let text = window.get_param_json();
        match serde_json::from_str::<ParameterValues>(text.as_str()) {
            Ok(parameters) => {
                let (buffer, warnings) = params_to_buffer(&schema, &parameters);
                push_table_model(window, &buffer);
                state.borrow_mut().table_buffer = buffer;
                window.set_advanced_mode(false);
                if !warnings.is_empty() {
                    report(window, Ok(warnings.join("；")));
                }
            }
            Err(error) => report::<String>(
                window,
                Err(Adm4Error::invalid_input(format!(
                    "JSON 非法，无法切回表格模式：{error}"
                ))),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// AI 访谈（下一提案 / 确认（可下钻）/ 拒绝；确认与拒绝只由按钮点击触发，D11）
// ---------------------------------------------------------------------------

fn hook_interview_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_interview_next(move || {
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    report::<String>(
                        &window,
                        Err(Adm4Error::invalid_input("请先在「项目」页打开项目")),
                    );
                    return;
                };
                match services.interview_next(&archive) {
                    Ok(turn) => show_interview_turn(&window, &services, &state, &archive, turn),
                    Err(error) => report::<String>(&window, Err(error)),
                }
                refresh_interview(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_interview_confirm(move |params_text| {
            if let Some(window) = weak.upgrade() {
                confirm_interview_proposal(&window, &services, &state, params_text.as_str());
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_interview_reject(move |note| {
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    return;
                };
                let Some(proposal) = pending_proposal(&state) else {
                    report::<String>(
                        &window,
                        Err(Adm4Error::invalid_input(
                            "没有待处理的提案，请先点「下一提案」",
                        )),
                    );
                    return;
                };
                match services.interview_reject(&archive, &proposal.decision_id, note.as_str()) {
                    Ok(()) => {
                        state.borrow_mut().pending_turn = None;
                        clear_interview_proposal(&window);
                        report(
                            &window,
                            Ok(format!(
                                "已拒绝 {}：该点排到同层末尾等待重提",
                                proposal.decision_id
                            )),
                        );
                        refresh_interview(&window, &services, &state);
                    }
                    Err(error) => report::<String>(&window, Err(error)),
                }
            }
        });
    }
}

/// 展示一条访谈回合：提案暂存进 UiState，等待用户手势处置。
fn show_interview_turn(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
    archive: &str,
    turn: InterviewTurnDto,
) {
    let is_table = matches!(turn, InterviewTurnDto::TableProposal { .. });
    let Some(proposal) = turn.proposal().cloned() else {
        state.borrow_mut().pending_turn = None;
        clear_interview_proposal(window);
        window.set_interview_turn_kind(SharedString::from("访谈完成：全部激活点已确认"));
        report(window, Ok("访谈完成：全部激活点已确认".to_string()));
        return;
    };
    window.set_interview_turn_kind(SharedString::from(if is_table {
        "整表提案（L5/L6，一表一个确认单元，可编辑参数做例外下钻）"
    } else {
        "结构层提案（单点）"
    }));
    window.set_interview_rationale(SharedString::from(proposal.rationale.clone()));
    window.set_interview_params_editable(is_table);
    window.set_interview_has_proposal(true);
    let json = match params_to_pretty_json(&proposal.parameters) {
        Ok(json) => json,
        Err(error) => {
            report::<String>(window, Err(error));
            String::new()
        }
    };
    window.set_interview_params_json(SharedString::from(json));
    // 富化展示：决策问题与选项 label（查询失败时退回 id 展示并报错）。
    match proposal_labels(services, archive, &proposal) {
        Ok((question, option_label)) => {
            window.set_interview_decision(SharedString::from(format!(
                "决策点：{}{}",
                proposal.decision_id, question
            )));
            window.set_interview_option(SharedString::from(format!(
                "提案选项：{}{}",
                proposal.option_id, option_label
            )));
            report(
                window,
                Ok(format!(
                    "已获取提案 {}/{}，请确认或拒绝",
                    proposal.decision_id, proposal.option_id
                )),
            );
        }
        Err(error) => {
            window.set_interview_decision(SharedString::from(format!(
                "决策点：{}",
                proposal.decision_id
            )));
            window.set_interview_option(SharedString::from(format!(
                "提案选项：{}",
                proposal.option_id
            )));
            report::<String>(window, Err(error));
        }
    }
    state.borrow_mut().pending_turn = Some(turn);
}

/// 用户点击「确认提案」：整表提案的参数编辑结果作为 overrides（例外下钻，D10）；
/// JSON 解析失败直接报错并保留提案，不提交、不吞错。
fn confirm_interview_proposal(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
    params_text: &str,
) {
    let Some(archive) = state.borrow().current_archive.clone() else {
        return;
    };
    let Some(turn) = state.borrow().pending_turn.clone() else {
        report::<String>(
            window,
            Err(Adm4Error::invalid_input(
                "没有待确认的提案，请先点「下一提案」",
            )),
        );
        return;
    };
    let Some(proposal) = turn.proposal().cloned() else {
        return;
    };
    let overrides = if matches!(turn, InterviewTurnDto::TableProposal { .. }) {
        match serde_json::from_str::<ParameterValues>(params_text) {
            Ok(edited) if edited == proposal.parameters => None,
            Ok(edited) => Some(edited),
            Err(error) => {
                report::<String>(
                    window,
                    Err(Adm4Error::invalid_input(format!(
                        "参数 JSON 非法，未提交确认：{error}"
                    ))),
                );
                return;
            }
        }
    } else {
        None
    };
    let drilled = overrides.is_some();
    match services.interview_confirm(&archive, &proposal, overrides) {
        Ok(problems) => {
            state.borrow_mut().pending_turn = None;
            clear_interview_proposal(window);
            let mut message = format!(
                "已确认 {}/{}{}",
                proposal.decision_id,
                proposal.option_id,
                if drilled { "（例外下钻）" } else { "" }
            );
            if !problems.is_empty() {
                message.push_str(&format!(
                    "，{} 项待填：{}",
                    problems.len(),
                    problems.join("；")
                ));
            }
            report(window, Ok(message));
            refresh_interview(window, services, state);
            refresh_workbench(window, services, state);
            refresh_decision_panel(window, services, state);
        }
        // 确认失败时提案保留在面板上，可修正后重试或拒绝。
        Err(error) => report::<String>(window, Err(error)),
    }
}

fn pending_proposal(state: &Rc<RefCell<UiState>>) -> Option<InterviewProposal> {
    state
        .borrow()
        .pending_turn
        .as_ref()
        .and_then(|turn| turn.proposal().cloned())
}

/// 查决策问题与选项 label 用于提案展示（只读，不改任何状态）。
fn proposal_labels(
    services: &AppServices,
    archive: &str,
    proposal: &InterviewProposal,
) -> Adm4Result<(String, String)> {
    let engine = services.open_engine(archive)?;
    let question = engine
        .space()
        .graph
        .point(&proposal.decision_id)
        .map(|point| format!("（{}）", point.question))
        .unwrap_or_default();
    let option_label = engine
        .space()
        .graph
        .point(&proposal.decision_id)
        .and_then(|point| point.option(&proposal.option_id))
        .map(|option| format!("（{}）", option.label))
        .unwrap_or_default();
    Ok((question, option_label))
}

fn clear_interview_proposal(window: &MainWindow) {
    window.set_interview_has_proposal(false);
    window.set_interview_params_editable(false);
    window.set_interview_turn_kind(SharedString::default());
    window.set_interview_decision(SharedString::default());
    window.set_interview_option(SharedString::default());
    window.set_interview_rationale(SharedString::default());
    window.set_interview_params_json(SharedString::default());
}

/// 顶部分层进度：interview_progress 的各层「已确认/适用」与当前层。
fn refresh_interview(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    let Some(archive) = state.borrow().current_archive.clone() else {
        window.set_interview_levels(ModelRc::new(VecModel::from(Vec::<LevelRow>::new())));
        window.set_interview_progress_text(SharedString::default());
        return;
    };
    match services.interview_progress(&archive) {
        Ok(progress) => {
            let rows: Vec<LevelRow> = progress
                .levels
                .iter()
                .map(|level| LevelRow {
                    label: level.level.label().into(),
                    text: format!("{}/{}", level.confirmed, level.applicable).into(),
                    current: Some(level.level) == progress.current_level,
                })
                .collect();
            window.set_interview_levels(ModelRc::new(VecModel::from(rows)));
            window.set_interview_progress_text(SharedString::from(match progress.current_level {
                Some(level) => format!("当前层：{}（低层未全确认不进高层）", level.label()),
                None => "全部适用点已确认".to_string(),
            }));
        }
        Err(error) => report::<String>(window, Err(error)),
    }
}

// ---------------------------------------------------------------------------
// 冻结门明细 + 流水线
// ---------------------------------------------------------------------------

fn hook_freeze_pipeline_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_run_red_team(move || {
            if let Some(window) = weak.upgrade()
                && let Some(archive) = state.borrow().current_archive.clone()
            {
                report(
                    &window,
                    services
                        .freeze_red_team(&archive)
                        .map(|count| format!("红队评审完成，发现 {count} 项")),
                );
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_freeze_check(move || {
            if let Some(window) = weak.upgrade()
                && let Some(archive) = state.borrow().current_archive.clone()
            {
                match services.freeze_check(&archive) {
                    Ok(freeze_report) => {
                        let mut rows = Vec::new();
                        for gate in &freeze_report.gates {
                            rows.push(GateRow {
                                gate: gate.gate.clone().into(),
                                is_header: true,
                                passed: gate.passed,
                                text: if gate.passed {
                                    SharedString::from("通过")
                                } else {
                                    SharedString::from(format!(
                                        "未通过（{} 项）",
                                        gate.findings.len()
                                    ))
                                },
                            });
                            for finding in &gate.findings {
                                rows.push(GateRow {
                                    gate: SharedString::default(),
                                    is_header: false,
                                    passed: gate.passed,
                                    text: format!("[{}] {}", finding.code, finding.message).into(),
                                });
                            }
                        }
                        window.set_gate_rows(ModelRc::new(VecModel::from(rows)));
                        // 「执行冻结」可用性由后端评估结果驱动，UI 不重算（D14）。
                        window.set_freeze_ready(freeze_report.all_passed());
                        let passed = freeze_report
                            .gates
                            .iter()
                            .filter(|gate| gate.passed)
                            .count();
                        report(
                            &window,
                            Ok(format!(
                                "五道门评估完成：通过 {passed}/{}",
                                freeze_report.gates.len()
                            )),
                        );
                    }
                    Err(error) => {
                        window.set_freeze_ready(false);
                        report::<String>(&window, Err(error));
                    }
                }
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_freeze_run(move || {
            if let Some(window) = weak.upgrade()
                && let Some(archive) = state.borrow().current_archive.clone()
            {
                report(
                    &window,
                    services.freeze_run(&archive).map(|frozen| {
                        format!("冻结成功 v{}：{}", frozen.version, frozen.content_hash)
                    }),
                );
                refresh_pipeline(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_pipeline_run(move || {
            if let Some(window) = weak.upgrade()
                && let Some(archive) = state.borrow().current_archive.clone()
            {
                report(
                    &window,
                    services
                        .pipeline_run(&archive, "C0", "C6")
                        .map(|_| "流水线运行完成（见各阶段状态）".to_string()),
                );
                refresh_pipeline(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_pipeline_confirm(move |stage, note| {
            if let Some(window) = weak.upgrade()
                && let Some(archive) = state.borrow().current_archive.clone()
            {
                report(
                    &window,
                    services
                        .pipeline_confirm(&archive, stage.as_str(), "desktop_user", note.as_str())
                        .map(|_| format!("阶段 {stage} 已人工确认")),
                );
                refresh_pipeline(&window, &services, &state);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// 模板逆向维护（产线五步 + 对照查询）
// ---------------------------------------------------------------------------

fn hook_reverse_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_select_pack(move |pack| {
            if let Some(window) = weak.upgrade() {
                {
                    let mut borrowed = state.borrow_mut();
                    borrowed.reverse_pack = Some(pack.to_string());
                    borrowed.reverse_template = None;
                }
                clear_reverse_results(&window);
                refresh_reverse(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_refresh(move || {
            if let Some(window) = weak.upgrade() {
                refresh_reverse(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_select_template(move |template_id| {
            if let Some(window) = weak.upgrade() {
                state.borrow_mut().reverse_template = Some(template_id.to_string());
                clear_reverse_results(&window);
                refresh_reverse(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_new_draft(move |template_id, game_name, aliases, depth| {
            if let Some(window) = weak.upgrade() {
                let Some(pack) = state.borrow().reverse_pack.clone() else {
                    report::<String>(
                        &window,
                        Err(Adm4Error::invalid_input("请先在左侧选择品类包")),
                    );
                    return;
                };
                let level = match parse_depth(depth.as_str()) {
                    Ok(level) => level,
                    Err(error) => {
                        report::<String>(&window, Err(error));
                        return;
                    }
                };
                let alias_list = split_list(aliases.as_str());
                match services.template_new_draft(
                    &pack,
                    template_id.as_str(),
                    game_name.as_str(),
                    &alias_list,
                    level,
                ) {
                    Ok(template) => {
                        state.borrow_mut().reverse_template = Some(template.template_id.clone());
                        report(
                            &window,
                            Ok(format!(
                                "已建草稿 {pack}/{}（Draft，逆向目标：{}）",
                                template.template_id, template.game_name
                            )),
                        );
                    }
                    Err(error) => report::<String>(&window, Err(error)),
                }
                refresh_reverse(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_search(move |corpus_dir, question, keywords| {
            if let Some(window) = weak.upgrade() {
                let Some((pack, template_id)) = reverse_pair(&window, &state) else {
                    return;
                };
                let keyword_list = split_list(keywords.as_str());
                report(
                    &window,
                    services
                        .template_search_corpus(
                            &pack,
                            &template_id,
                            Path::new(corpus_dir.trim()),
                            question.as_str(),
                            &keyword_list,
                        )
                        .map(|hits| format!("语料检索命中 {} 条（已并入候选池）", hits.len())),
                );
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_map(move || {
            if let Some(window) = weak.upgrade() {
                let Some((pack, template_id)) = reverse_pair(&window, &state) else {
                    return;
                };
                report(
                    &window,
                    services
                        .template_map(&pack, &template_id)
                        .map(|count| format!("AI 映射 {count} 条答案（Draft→Mapped）")),
                );
                refresh_reverse(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_crosscheck(move || {
            if let Some(window) = weak.upgrade() {
                let Some((pack, template_id)) = reverse_pair(&window, &state) else {
                    return;
                };
                let result = services.template_cross_check(&pack, &template_id);
                refresh_reverse(&window, &services, &state);
                match result {
                    Ok(check) => {
                        // 用本次核验报告（带 reason）覆盖冲突清单。
                        let conflicts: Vec<ConflictRow> = check
                            .entries
                            .iter()
                            .filter(|entry| {
                                entry.verdict == adm4_template::CrossCheckVerdict::Conflict
                            })
                            .map(|entry| ConflictRow {
                                decision: entry.decision_id.clone().into(),
                                reason: if entry.reason.is_empty() {
                                    SharedString::from("S3 判定冲突，待人工裁决")
                                } else {
                                    entry.reason.clone().into()
                                },
                            })
                            .collect();
                        let conflict_count = conflicts.len();
                        window.set_template_conflicts(ModelRc::new(VecModel::from(conflicts)));
                        report(
                            &window,
                            Ok(format!(
                                "交叉核验 {} 条，冲突待人工 {} 条（Mapped→CrossChecked）",
                                check.entries.len(),
                                conflict_count
                            )),
                        );
                    }
                    Err(error) => report::<String>(&window, Err(error)),
                }
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_review(move |reviewer, note| {
            if let Some(window) = weak.upgrade() {
                let Some((pack, template_id)) = reverse_pair(&window, &state) else {
                    return;
                };
                // 署名/结论必填由后端校验（R3），UI 只转发并展示拒绝原因。
                report(
                    &window,
                    services
                        .template_review(&pack, &template_id, reviewer.as_str(), note.as_str())
                        .map(|template| {
                            format!(
                                "人工审核通过（评审人：{}）（CrossChecked→HumanReviewed）",
                                template.certification.reviewed_by
                            )
                        }),
                );
                refresh_reverse(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_certify(move || {
            if let Some(window) = weak.upgrade() {
                let Some((pack, template_id)) = reverse_pair(&window, &state) else {
                    return;
                };
                report(
                    &window,
                    services
                        .template_certify(&pack, &template_id)
                        .map(|template| {
                            format!(
                                "认证入库，登记换皮词 {} 个（HumanReviewed→Certified）",
                                template.skin_words().len()
                            )
                        }),
                );
                refresh_reverse(&window, &services, &state);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_reverse_compare(move || {
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    report::<String>(
                        &window,
                        Err(Adm4Error::invalid_input("请先在「项目」页打开项目")),
                    );
                    return;
                };
                let Some(template_id) = state.borrow().reverse_template.clone() else {
                    report::<String>(&window, Err(Adm4Error::invalid_input("请先在左侧选择模板")));
                    return;
                };
                match services.template_compare(&archive, &template_id) {
                    Ok(comparison) => {
                        let rows: Vec<CompareRow> = comparison
                            .entries
                            .iter()
                            .map(|entry| CompareRow {
                                decision: entry.decision_id.clone().into(),
                                template_option: entry.template_option.clone().into(),
                                project_option: entry
                                    .project_option
                                    .clone()
                                    .unwrap_or_else(|| "（未选）".to_string())
                                    .into(),
                                same: entry.same_option,
                                params: params_summary(&entry.template_parameters).into(),
                            })
                            .collect();
                        let count = rows.len();
                        window.set_compare_rows(ModelRc::new(VecModel::from(rows)));
                        window.set_compare_title(SharedString::from(format!(
                            "对照：{}（{}） vs 当前项目，共 {count} 条",
                            comparison.game_name, comparison.template_id
                        )));
                        report(&window, Ok("对照查询完成".to_string()));
                    }
                    Err(error) => report::<String>(&window, Err(error)),
                }
            }
        });
    }
}

/// 逆向操作的前置：已选品类包与模板，缺一即报错并中止。
fn reverse_pair(window: &MainWindow, state: &Rc<RefCell<UiState>>) -> Option<(String, String)> {
    let borrowed = state.borrow();
    match (
        borrowed.reverse_pack.clone(),
        borrowed.reverse_template.clone(),
    ) {
        (Some(pack), Some(template)) => Some((pack, template)),
        _ => {
            report::<String>(
                window,
                Err(Adm4Error::invalid_input("请先选择品类包与模板")),
            );
            None
        }
    }
}

fn clear_reverse_results(window: &MainWindow) {
    window.set_template_conflicts(ModelRc::new(VecModel::from(Vec::<ConflictRow>::new())));
    window.set_compare_rows(ModelRc::new(VecModel::from(Vec::<CompareRow>::new())));
    window.set_compare_title(SharedString::default());
}

/// 刷新模板列表 + 选中模板的产线五步状态与已持久化的冲突清单。
fn refresh_reverse(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    let Some(pack) = state.borrow().reverse_pack.clone() else {
        window.set_template_list(ModelRc::new(VecModel::from(Vec::<TemplateRow>::new())));
        window.set_template_steps(ModelRc::new(VecModel::from(Vec::<StepRow>::new())));
        window.set_reverse_selected(SharedString::from("（请先选择品类包）"));
        return;
    };
    let templates = match services.templates().list(&pack) {
        Ok(templates) => templates,
        Err(error) => {
            window.set_template_list(ModelRc::new(VecModel::from(Vec::<TemplateRow>::new())));
            report::<String>(window, Err(error));
            return;
        }
    };
    let rows: Vec<TemplateRow> = templates
        .iter()
        .map(|template| {
            let conflicts = persisted_conflict_count(template);
            TemplateRow {
                id: template.template_id.clone().into(),
                game: template.game_name.clone().into(),
                status: status_label(template.certification.status).into(),
                depth: format!("{:?}", template.depth_reached).into(),
                answers: if conflicts == 0 {
                    format!("答卷 {} 条", template.answers.len()).into()
                } else {
                    format!("答卷 {} 条，冲突 {conflicts} 条", template.answers.len()).into()
                },
            }
        })
        .collect();
    window.set_template_list(ModelRc::new(VecModel::from(rows)));

    let selected_id = state.borrow().reverse_template.clone();
    let selected =
        selected_id.and_then(|id| templates.iter().find(|template| template.template_id == id));
    match selected {
        Some(template) => {
            window.set_reverse_selected(SharedString::from(format!(
                "{pack}/{} · {} · 状态：{}",
                template.template_id,
                template.game_name,
                status_label(template.certification.status)
            )));
            window.set_template_steps(step_rows(template.certification.status));
            window.set_template_conflicts(persisted_conflicts(template));
        }
        None => {
            window.set_reverse_selected(SharedString::from("（未选择模板）"));
            window.set_template_steps(ModelRc::new(VecModel::from(Vec::<StepRow>::new())));
        }
    }
}

/// 产线五步在状态链上的序号；遗留状态（Approved/Rejected）不在链上。
fn status_rank(status: CertificationStatus) -> Option<usize> {
    match status {
        CertificationStatus::Draft => Some(0),
        CertificationStatus::Mapped => Some(1),
        CertificationStatus::CrossChecked => Some(2),
        CertificationStatus::HumanReviewed => Some(3),
        CertificationStatus::Certified => Some(4),
        CertificationStatus::Approved | CertificationStatus::Rejected => None,
    }
}

fn status_label(status: CertificationStatus) -> &'static str {
    match status {
        CertificationStatus::Draft => "草稿",
        CertificationStatus::Mapped => "已映射",
        CertificationStatus::CrossChecked => "已核验",
        CertificationStatus::HumanReviewed => "已审核",
        CertificationStatus::Certified => "已认证",
        CertificationStatus::Approved | CertificationStatus::Rejected => "遗留状态（需重走产线）",
    }
}

/// 五步状态条：绿 = 已达成，橙 = 下一步，灰 = 未来步骤。
fn step_rows(status: CertificationStatus) -> ModelRc<StepRow> {
    let names = [
        "S1 草稿",
        "S2 已映射",
        "S3 已核验",
        "S4 已审核",
        "S5 已认证",
    ];
    let rank = status_rank(status);
    let rows: Vec<StepRow> = names
        .iter()
        .enumerate()
        .map(|(index, name)| StepRow {
            name: SharedString::from(*name),
            reached: rank.is_some_and(|reached| index <= reached),
            current: rank.is_some_and(|reached| index == reached + 1),
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

fn persisted_conflict_count(template: &Template) -> usize {
    template
        .answers
        .iter()
        .filter(|answer| answer.crosscheck_agreed == Some(false))
        .count()
}

/// 模板上已持久化的冲突标记（S3 落盘的 crosscheck_agreed=false；reason 只在核验运行时可得）。
fn persisted_conflicts(template: &Template) -> ModelRc<ConflictRow> {
    let rows: Vec<ConflictRow> = template
        .answers
        .iter()
        .filter(|answer| answer.crosscheck_agreed == Some(false))
        .map(|answer| ConflictRow {
            decision: answer.decision_id.clone().into(),
            reason: SharedString::from("S3 判定冲突，待人工裁决（详情见核验时输出）"),
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

/// 模板参数的紧凑展示；序列化失败时把错误呈现在格内（不吞错）。
fn params_summary(parameters: &ParameterValues) -> String {
    if matches!(parameters, ParameterValues::None) {
        return String::new();
    }
    match serde_json::to_string(parameters) {
        Ok(json) => format!("模板参数：{json}"),
        Err(error) => format!("模板参数序列化失败：{error}"),
    }
}

// ---------------------------------------------------------------------------
// 共用小工具
// ---------------------------------------------------------------------------

fn current_pair(state: &Rc<RefCell<UiState>>) -> (Option<String>, Option<String>) {
    let borrowed = state.borrow();
    (
        borrowed.current_archive.clone(),
        borrowed.current_decision.clone(),
    )
}

fn parse_depth(depth: &str) -> Adm4Result<DesignLevel> {
    match depth {
        "L4" => Ok(DesignLevel::L4),
        "L5" => Ok(DesignLevel::L5),
        "L6" => Ok(DesignLevel::L6),
        other => Err(Adm4Error::invalid_input(format!(
            "深度档「{other}」非法（只接受 L4/L5/L6）"
        ))),
    }
}

/// 逗号分隔（兼容中文逗号）→ 去空白非空列表。
fn split_list(text: &str) -> Vec<String> {
    text.split([',', '，'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn saved_message(problems: Vec<String>) -> String {
    if problems.is_empty() {
        "参数已保存并通过校验".to_string()
    } else {
        format!(
            "参数已保存，{} 项待修正：{}",
            problems.len(),
            problems.join("；")
        )
    }
}

fn params_to_pretty_json(parameters: &ParameterValues) -> Adm4Result<String> {
    serde_json::to_string_pretty(parameters)
        .map_err(|error| Adm4Error::internal(format!("参数序列化失败：{error}")))
}

/// 当前决策点已选选项的参数 schema（结构化编辑器与模式切换共用）。
fn lookup_selected_schema(
    engine: &AuthoringEngine,
    decision_id: &str,
) -> Adm4Result<ParameterSchema> {
    let point = engine
        .space()
        .graph
        .point(decision_id)
        .ok_or_else(|| Adm4Error::not_found(format!("决策点 {decision_id} 不存在")))?;
    let selection = engine
        .state()
        .selections
        .get(decision_id)
        .ok_or_else(|| Adm4Error::invalid_input("尚未选择选项，无法编辑参数"))?;
    let option = point
        .option(&selection.option_id)
        .ok_or_else(|| Adm4Error::not_found(format!("选项 {} 不存在", selection.option_id)))?;
    Ok(option.parameter_schema.clone())
}

/// UI 缓冲 → ParameterValues（按 schema 分派；非表/矩阵 schema 走 JSON 编辑，不该到这里）。
fn buffer_to_params(
    schema: &ParameterSchema,
    buffer: &[Vec<String>],
) -> Adm4Result<ParameterValues> {
    match schema {
        ParameterSchema::Table(table) => convert::table_buffer_to_params(table, buffer),
        ParameterSchema::Matrix(matrix) => convert::matrix_buffer_to_params(matrix, buffer),
        _ => Err(Adm4Error::invalid_input(
            "当前选项不是表/矩阵结构，请使用 JSON 参数编辑",
        )),
    }
}

/// ParameterValues → UI 缓冲（含警告：schema 外列、值变体不符等会随状态栏呈现）。
fn params_to_buffer(
    schema: &ParameterSchema,
    parameters: &ParameterValues,
) -> (Vec<Vec<String>>, Vec<String>) {
    match schema {
        ParameterSchema::Table(table) => convert::table_buffer_from_params(table, parameters),
        ParameterSchema::Matrix(_) => convert::matrix_buffer_from_params(parameters),
        _ => (Vec::new(), Vec::new()),
    }
}

fn axis_label(axis: &AxisRef) -> String {
    match axis {
        AxisRef::DecisionOptions { decision } => format!("{decision} 的选项"),
        AxisRef::TableRows { decision } => format!("{decision} 的表行"),
    }
}

fn push_table_model(window: &MainWindow, buffer: &[Vec<String>]) {
    let rows: Vec<ModelRc<SharedString>> = buffer
        .iter()
        .map(|row| {
            ModelRc::new(VecModel::from(
                row.iter().map(SharedString::from).collect::<Vec<_>>(),
            ))
        })
        .collect();
    window.set_table_rows_model(ModelRc::new(VecModel::from(rows)));
}

fn report<T: Into<String>>(window: &MainWindow, result: Adm4Result<T>) {
    match result {
        Ok(message) => window.set_status_message(SharedString::from(message.into())),
        Err(error) => window.set_status_message(SharedString::from(format!(
            "[{:?}] {}",
            error.kind, error.message
        ))),
    }
}

// ---------------------------------------------------------------------------
// 刷新逻辑
// ---------------------------------------------------------------------------

fn refresh_projects(window: &MainWindow, services: &AppServices) {
    let items: Vec<ProjectItem> = services
        .project_list()
        .unwrap_or_default()
        .into_iter()
        .map(|manifest| ProjectItem {
            id: manifest.archive_id.into(),
            name: manifest.project_name.into(),
            updated: manifest.updated_at.into(),
        })
        .collect();
    window.set_projects(ModelRc::new(VecModel::from(items)));
}

fn refresh_packs(window: &MainWindow, services: &AppServices) {
    let packs: Vec<SharedString> = services
        .list_packs()
        .unwrap_or_default()
        .into_iter()
        .map(SharedString::from)
        .collect();
    window.set_packs(ModelRc::new(VecModel::from(packs)));
}

fn refresh_logs(window: &MainWindow, services: &AppServices) {
    let items: Vec<LogItem> = services
        .log
        .tail(200)
        .unwrap_or_default()
        .into_iter()
        .rev()
        .map(|entry| LogItem {
            at: entry.at.into(),
            category: entry.category.into(),
            message: entry.message.into(),
        })
        .collect();
    window.set_logs(ModelRc::new(VecModel::from(items)));
}

fn refresh_workbench(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    let Some(archive) = state.borrow().current_archive.clone() else {
        return;
    };
    let Ok(engine) = services.open_engine(&archive) else {
        return;
    };
    window.set_current_project_name(SharedString::from(format!(
        "{}（{}）",
        engine.state().project_name,
        engine.state().genre_pack
    )));

    // 领域列表。
    let mut domains: Vec<String> = engine
        .space()
        .graph
        .points()
        .iter()
        .map(|point| point.domain.clone())
        .collect();
    domains.sort();
    domains.dedup();
    window.set_domains(ModelRc::new(VecModel::from(
        domains.iter().map(SharedString::from).collect::<Vec<_>>(),
    )));

    // 决策点列表（按选中领域过滤；未选则全部）。
    let applicability = engine.applicability();
    let filter = state.borrow().current_domain.clone();
    let items: Vec<DecisionItem> = engine
        .space()
        .graph
        .points()
        .iter()
        .filter(|point| {
            filter
                .as_deref()
                .is_none_or(|domain| point.domain == domain)
        })
        .map(|point| {
            let selection = engine.state().selections.get(&point.id);
            let status = match applicability.get(&point.id) {
                Some(PointApplicability::BeyondDepth) => "超出深度档".to_string(),
                Some(PointApplicability::Inactive) => "未激活".to_string(),
                Some(PointApplicability::NotApplicable(_)) => "不适用".to_string(),
                _ => match selection {
                    Some(selected) if selected.confirmed_by_user => "已确认".to_string(),
                    Some(_) => "待确认".to_string(),
                    None => "待选择".to_string(),
                },
            };
            DecisionItem {
                id: point.id.clone().into(),
                domain: point.domain.clone().into(),
                level: format!("{:?}", point.level).into(),
                question: point.question.clone().into(),
                status: status.into(),
                selected: selection
                    .map(|selected| selected.option_id.clone())
                    .unwrap_or_default()
                    .into(),
            }
        })
        .collect();
    window.set_decisions(ModelRc::new(VecModel::from(items)));

    // 完成度。
    let completeness = engine.completeness();
    window.set_completeness_text(SharedString::from(format!(
        "完成度 {}/{}（{}%），阻塞 {} 项",
        completeness.done,
        completeness.total,
        completeness.percent(),
        completeness.blocking.len()
    )));
    window.set_completeness_ratio(if completeness.total == 0 {
        0.0
    } else {
        completeness.done as f32 / completeness.total as f32
    });
}

fn refresh_decision_panel(
    window: &MainWindow,
    services: &AppServices,
    state: &Rc<RefCell<UiState>>,
) {
    let (Some(archive), Some(decision_id)) = current_pair(state) else {
        return;
    };
    let Ok(engine) = services.open_engine(&archive) else {
        return;
    };
    let Some(point) = engine.space().graph.point(&decision_id) else {
        return;
    };
    window.set_current_decision(SharedString::from(decision_id.clone()));
    window.set_current_decision_question(SharedString::from(format!(
        "[{}] {}",
        point.level.label(),
        point.question
    )));
    let selection = engine.state().selections.get(&decision_id);
    let options: Vec<OptionItem> = point
        .options
        .iter()
        .map(|option| OptionItem {
            id: option.id.clone().into(),
            label: option.label.clone().into(),
            summary: option.summary.clone().into(),
            selected: selection.is_some_and(|selected| selected.option_id == option.id),
        })
        .collect();
    window.set_options(ModelRc::new(VecModel::from(options)));
    let params = selection
        .map(|selected| serde_json::to_string_pretty(&selected.parameters).unwrap_or_default())
        .unwrap_or_default();
    window.set_param_json(SharedString::from(params));

    // 结构化编辑器：已选选项的 schema 为 Table/Matrix 时启用（列名来自 schema，D13）。
    let mut editor = EditorKind::None;
    let mut buffer: Vec<Vec<String>> = Vec::new();
    let mut columns: Vec<SharedString> = Vec::new();
    let mut title = String::new();
    let mut warnings: Vec<String> = Vec::new();
    if let Some(selected) = selection
        && let Some(option) = point.option(&selected.option_id)
    {
        match &option.parameter_schema {
            ParameterSchema::Table(table) => {
                editor = EditorKind::Table {
                    columns: table.columns.len(),
                };
                columns = table
                    .columns
                    .iter()
                    .map(|column| {
                        SharedString::from(format!(
                            "{}{}·{}",
                            column.key,
                            if column.required { "*" } else { "" },
                            convert::kind_label(&column.kind)
                        ))
                    })
                    .collect();
                title = format!(
                    "表结构编辑（行标识列 {}；留空格 = 未填，由后端校验报缺）",
                    table.row_key
                );
                let (initial, warns) =
                    convert::table_buffer_from_params(table, &selected.parameters);
                buffer = initial;
                warnings = warns;
            }
            ParameterSchema::Matrix(matrix) => {
                editor = EditorKind::Matrix;
                columns = vec![
                    SharedString::from("row·行标识"),
                    SharedString::from("col·列标识"),
                    SharedString::from(format!(
                        "{}·{}",
                        matrix.cell.key,
                        convert::kind_label(&matrix.cell.kind)
                    )),
                ];
                title = format!(
                    "矩阵格编辑（行轴：{}；列轴：{}；缺格由后端校验逐格列出）",
                    axis_label(&matrix.row_axis),
                    axis_label(&matrix.col_axis)
                );
                let (initial, warns) = convert::matrix_buffer_from_params(&selected.parameters);
                buffer = initial;
                warnings = warns;
            }
            _ => {}
        }
    }
    window.set_structured_active(editor != EditorKind::None);
    window.set_editor_title(SharedString::from(title));
    window.set_table_columns(ModelRc::new(VecModel::from(columns)));
    push_table_model(window, &buffer);
    {
        let mut borrowed = state.borrow_mut();
        borrowed.table_buffer = buffer;
        borrowed.editor = editor;
    }
    if !warnings.is_empty() {
        report(window, Ok(warnings.join("；")));
    }
}

fn refresh_pipeline(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    let Some(archive) = state.borrow().current_archive.clone() else {
        return;
    };
    let run_state = services.pipeline_status(&archive).unwrap_or_default();
    let items: Vec<StageItem> = adm4_pipeline::design_compile_registry()
        .into_iter()
        .map(|stage| {
            let status = run_state.stage_status(&stage.id);
            let (text, waiting) = match &status {
                adm4_pipeline::StageStatus::Pending => ("待运行".to_string(), false),
                adm4_pipeline::StageStatus::Running => ("运行中".to_string(), false),
                adm4_pipeline::StageStatus::Succeeded => ("成功".to_string(), false),
                adm4_pipeline::StageStatus::Failed { reasons } => {
                    (format!("失败：{}", reasons.join("；")), false)
                }
                adm4_pipeline::StageStatus::Blocked { reasons } => {
                    (format!("阻塞：{}", reasons.join("；")), false)
                }
                adm4_pipeline::StageStatus::WaitingHuman { gate } => {
                    (format!("等待人工确认（{gate}）"), true)
                }
            };
            StageItem {
                id: stage.id.into(),
                name: stage.name.into(),
                status: text.into(),
                waiting,
            }
        })
        .collect();
    window.set_stages(ModelRc::new(VecModel::from(items)));
}

fn stage_placeholder() -> ModelRc<StageItem> {
    let items: Vec<StageItem> = adm4_pipeline::design_compile_registry()
        .into_iter()
        .map(|stage| StageItem {
            id: stage.id.into(),
            name: stage.name.into(),
            status: "待运行".into(),
            waiting: false,
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}
