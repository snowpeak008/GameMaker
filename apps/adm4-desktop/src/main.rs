//! adm4 桌面壳（Slint）：布局与交互沿用二版（顶部六视图导航 + 设计工作台三栏 + 底部状态栏），
//! 语义按四版设计承载（决策点 / 多选主选 / 人工豁免 / 事务自动保存 / 冻结门）。
//!
//! GUI 无业务规则（D14）：全部数据来自 `adm4-app` 的只读聚合查询，全部变更走 `AppServices`
//! 的写入方法；按钮可用性一律由后端结果推出（例如「执行冻结」只看 `all_gates_passed`）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod convert;
mod view;

use adm4_app::{
    AppServices, DecisionPointView, InterviewTurnDto, ProjectProfile, WorkbenchOverview,
};
use adm4_authoring::{AuthoringEngine, AuthoringState, InterviewProposal};
use adm4_decision::{
    AxisRef, DesignDomain, DesignLevel, OrganizationProgress, ParameterSchema, ParameterValues,
    ParameterValues as Params, Provenance,
};
use adm4_foundation::{Adm4Error, Adm4Result, atomic_write};
use adm4_template::{CertificationStatus, Template};
use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// 结构化编辑器当前形态（按当前聚焦选项的 parameter_schema 派生，None = 走 JSON 编辑）。
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
    current_node: Option<String>,
    current_decision: Option<String>,
    /// 参数编辑聚焦的选项（多选点可有多个已选选项，各自一份参数）。
    current_option: Option<String>,
    /// 当前项目的决策点视图缓存：回调据此判断单选/多选、已选集合、适用性，
    /// 每次变更后随刷新重建（不是第二份真相，只是本次渲染的快照）。
    points: Vec<DecisionPointView>,
    /// 结构化编辑缓冲（UI 行模型，字符串格）；保存时按 schema 转回 ParameterValues。
    table_buffer: Vec<Vec<String>>,
    editor: EditorKind,
    /// 待确认的访谈提案：interview_next 的返回原样暂存，
    /// confirm/reject 只能由用户点击触发并把提案原样传回（D11）。
    pending_turn: Option<InterviewTurnDto>,
    reverse_pack: Option<String>,
    reverse_template: Option<String>,
    /// 选中模板的**所属包**：通用层模板为 `universal`，与左栏选中的包可以不同。
    /// 逆向产线按它读写模板文件（`refresh_reverse` 每次刷新时同步）。
    reverse_template_pack: Option<String>,
}

impl UiState {
    fn point(&self, decision_id: &str) -> Option<&DecisionPointView> {
        self.points
            .iter()
            .find(|point| point.decision_id == decision_id)
    }
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

    refresh_projects(&window, &services, &state);
    refresh_packs(&window, &services);
    refresh_logs(&window, &services, "");
    window.set_stages(ModelRc::new(VecModel::from(view::stage_rows(None))));
    window.set_pipeline_note(SharedString::from(view::pipeline_note()));
    window.set_level_brief(SharedString::from(view::level_brief(None, "")));
    window.set_autosave_text(SharedString::from(
        "事务自动保存：每次变更原子提交（无手动保存按钮）",
    ));
    report(&window, Ok("就绪：请在「存档管理」新建或载入项目"));

    hook_project_callbacks(&window, &services, &state);
    hook_workbench_callbacks(&window, &services, &state);
    hook_authoring_callbacks(&window, &services, &state);
    hook_editor_callbacks(&window, &services, &state);
    hook_interview_callbacks(&window, &services, &state);
    hook_freeze_pipeline_callbacks(&window, &services, &state);
    hook_reverse_callbacks(&window, &services, &state);
    window.run()
}

// ---------------------------------------------------------------------------
// 项目与存档
// ---------------------------------------------------------------------------

fn hook_project_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_refresh_projects(move || {
            if let Some(window) = weak.upgrade() {
                refresh_projects(&window, &services, &state);
                refresh_packs(&window, &services);
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_create_project(move |name, pack, depth| {
            if let Some(window) = weak.upgrade() {
                match parse_depth(depth.as_str()) {
                    Ok(level) => {
                        report(
                            &window,
                            services
                                .project_new(name.as_str(), pack.as_str(), level, None)
                                .map(|archive_id| {
                                    format!("已创建项目 {archive_id}（点列表载入即可开始设计）")
                                }),
                        );
                        refresh_projects(&window, &services, &state);
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
                    borrowed.current_node = None;
                    borrowed.current_decision = None;
                    borrowed.current_option = None;
                    borrowed.pending_turn = None;
                    borrowed.points.clear();
                }
                // 项目相关的展示状态切项目时归零。
                window.set_freeze_ready(false);
                window.set_freeze_hint(SharedString::default());
                window.set_structured_active(false);
                window.set_compare_rows(ModelRc::new(VecModel::from(Vec::<CompareRow>::new())));
                window.set_compare_title(SharedString::default());
                window.set_archive_panel_open(false);
                clear_interview_proposal(&window);
                refresh_all(&window, &services, &state);
                refresh_pipeline(&window, &services, &state);
                refresh_interview(&window, &services, &state);
                refresh_projects(&window, &services, &state);
                report(&window, Ok(format!("已载入项目 {archive_id}")));
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_refresh_workbench(move || {
            if let Some(window) = weak.upgrade() {
                refresh_all(&window, &services, &state);
                refresh_pipeline(&window, &services, &state);
                refresh_interview(&window, &services, &state);
                report(&window, Ok("工作台已刷新"));
            }
        });
    }
}

// ---------------------------------------------------------------------------
// 设计工作台：领域 / 节点 / 决策点导航
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
                {
                    let mut borrowed = state.borrow_mut();
                    borrowed.current_domain = Some(domain.to_string());
                    borrowed.current_node = None;
                }
                refresh_all(&window, &services, &state);
                report(&window, Ok(format!("已过滤到领域 {domain}")));
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_select_node(move |node| {
            if let Some(window) = weak.upgrade() {
                state.borrow_mut().current_node = Some(node.to_string());
                refresh_all(&window, &services, &state);
                report(&window, Ok(format!("已选择节点 {node}")));
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_select_decision(move |decision| {
            if let Some(window) = weak.upgrade() {
                {
                    let mut borrowed = state.borrow_mut();
                    let decision = decision.to_string();
                    // 从缺失项跳转过来时，同步把中栏的领域/节点切到该点所在处。
                    if let Some(point) = borrowed.point(&decision) {
                        let domain = point.domain_id.clone();
                        let node = point.node_id.clone();
                        let option = point
                            .options
                            .iter()
                            .find(|option| option.is_primary)
                            .or_else(|| point.options.iter().find(|option| option.selected))
                            .map(|option| option.option_id.clone());
                        borrowed.current_domain = Some(domain);
                        borrowed.current_node = Some(node);
                        borrowed.current_option = option;
                    }
                    borrowed.current_decision = Some(decision);
                }
                window.set_advanced_mode(false);
                refresh_all(&window, &services, &state);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// 创作动作：选项（单/多选）/ 主选 / 确认 / N/A 豁免 / 节点文本 / 导出
// ---------------------------------------------------------------------------

fn hook_authoring_callbacks(
    window: &MainWindow,
    services: &Rc<AppServices>,
    state: &Rc<RefCell<UiState>>,
) {
    let weak = window.as_weak();

    // 单选点：选定（覆盖既有已选集合）。
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_choose_option(move |option_id| {
            if let Some(window) = weak.upgrade() {
                let Some((archive, decision)) = current_pair(&window, &state) else {
                    return;
                };
                let result = services.with_project(&archive, |engine| {
                    engine.select_option(&decision, option_id.as_str(), Provenance::UserManual)
                });
                state.borrow_mut().current_option = Some(option_id.to_string());
                refresh_all(&window, &services, &state);
                report(
                    &window,
                    result.map(|()| format!("已选择 {decision} / {option_id}")),
                );
            }
        });
    }

    // 多选点：勾选 = 首个走 select_option、其后走 add_option；取消勾选 = remove_option，
    // 只剩一个已选选项时改为整点撤销（clear_selection）——两者语义不同，不能混。
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_toggle_option(move |option_id, turn_on| {
            if let Some(window) = weak.upgrade() {
                let Some((archive, decision)) = current_pair(&window, &state) else {
                    return;
                };
                let selected_count = state
                    .borrow()
                    .point(&decision)
                    .map(|point| point.options.iter().filter(|item| item.selected).count())
                    .unwrap_or_default();
                let option = option_id.to_string();
                let result: Adm4Result<String> = if turn_on {
                    if selected_count == 0 {
                        services
                            .with_project(&archive, |engine| {
                                engine.select_option(&decision, &option, Provenance::UserManual)
                            })
                            .map(|()| format!("已勾选首个选项 {decision} / {option}"))
                    } else {
                        services
                            .authoring_add_option(&archive, &decision, &option)
                            .map(|()| format!("已追加勾选 {decision} / {option}（需重新确认）"))
                    }
                } else if selected_count > 1 {
                    services
                        .authoring_remove_option(&archive, &decision, &option)
                        .map(|()| format!("已取消勾选 {decision} / {option}（需重新确认）"))
                } else {
                    services
                        .with_project(&archive, |engine| engine.clear_selection(&decision))
                        .map(|()| format!("已撤销 {decision} 的全部选择"))
                };
                if turn_on {
                    state.borrow_mut().current_option = Some(option);
                } else {
                    state.borrow_mut().current_option = None;
                }
                refresh_all(&window, &services, &state);
                report(&window, result);
            }
        });
    }

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_set_primary_option(move |option_id| {
            if let Some(window) = weak.upgrade() {
                let Some((archive, decision)) = current_pair(&window, &state) else {
                    return;
                };
                let result =
                    services.authoring_set_primary_option(&archive, &decision, option_id.as_str());
                refresh_all(&window, &services, &state);
                report(
                    &window,
                    result.map(|()| format!("{decision} 的主选已设为 {option_id}")),
                );
            }
        });
    }

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_focus_option(move |option_id| {
            if let Some(window) = weak.upgrade() {
                state.borrow_mut().current_option = Some(option_id.to_string());
                window.set_advanced_mode(false);
                refresh_decision_panel(&window, &services, &state);
                report(&window, Ok(format!("参数编辑目标：{option_id}")));
            }
        });
    }

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_confirm_decision(move || {
            if let Some(window) = weak.upgrade() {
                let Some((archive, decision)) = current_pair(&window, &state) else {
                    return;
                };
                let result =
                    services.with_project(&archive, |engine| engine.confirm_selection(&decision));
                refresh_all(&window, &services, &state);
                let message = result.map(|()| {
                    let mut text = format!("已确认 {decision}（= 二版检查单勾选）");
                    if state
                        .borrow()
                        .point(&decision)
                        .is_some_and(view::primary_missing)
                    {
                        text.push_str("；但该多选点尚未设主选，请点选项行的「设主」");
                    }
                    text
                });
                report(&window, message);
            }
        });
    }

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_set_not_applicable(move |reason_code, note, actor| {
            if let Some(window) = weak.upgrade() {
                let Some((archive, decision)) = current_pair(&window, &state) else {
                    return;
                };
                // 理由码/说明/署名三者必填由后端校验（R3），UI 只转发并展示拒绝原因。
                let result = services.authoring_set_not_applicable(
                    &archive,
                    &decision,
                    reason_code.as_str(),
                    note.as_str(),
                    actor.as_str(),
                );
                refresh_all(&window, &services, &state);
                report(
                    &window,
                    result.map(|()| {
                        format!("{decision} 已标记不适用：移出完成度分母，冻结门 1 逐条在案")
                    }),
                );
            }
        });
    }

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_clear_not_applicable(move || {
            if let Some(window) = weak.upgrade() {
                let Some((archive, decision)) = current_pair(&window, &state) else {
                    return;
                };
                let result = services.authoring_clear_not_applicable(&archive, &decision);
                refresh_all(&window, &services, &state);
                report(
                    &window,
                    result.map(|cleared| {
                        if cleared {
                            format!("{decision} 已解除不适用，重新进入完成度分母")
                        } else {
                            format!("{decision} 本来就不是不适用（无需解除）")
                        }
                    }),
                );
            }
        });
    }

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_save_node_notes(move |design_note, risk_note| {
            if let Some(window) = weak.upgrade() {
                let (archive, node) = {
                    let borrowed = state.borrow();
                    (
                        borrowed.current_archive.clone(),
                        borrowed.current_node.clone(),
                    )
                };
                let (Some(archive), Some(node)) = (archive, node) else {
                    report::<String>(
                        &window,
                        Err(Adm4Error::invalid_input("请先打开项目并在中栏选择节点")),
                    );
                    return;
                };
                let result = services
                    .authoring_set_node_design_note(&archive, &node, design_note.as_str())
                    .and_then(|()| {
                        services.authoring_set_node_risk_note(&archive, &node, risk_note.as_str())
                    });
                refresh_all(&window, &services, &state);
                report(
                    &window,
                    result.map(|()| format!("节点 {node} 的设计/风险说明已保存")),
                );
            }
        });
    }

    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_export_workbench(move |path| {
            if let Some(window) = weak.upgrade() {
                report(&window, export_workbench(&services, &state, path.as_str()));
            }
        });
    }
}

/// 工作台快照导出：三个只读聚合拼装 markdown 并原子落盘。
fn export_workbench(
    services: &AppServices,
    state: &Rc<RefCell<UiState>>,
    path: &str,
) -> Adm4Result<String> {
    let Some(archive) = state.borrow().current_archive.clone() else {
        return Err(Adm4Error::invalid_input("请先打开项目"));
    };
    let target = path.trim();
    if target.is_empty() {
        return Err(Adm4Error::invalid_input(
            "请先填写导出路径（例如 D:\\snapshot.md）",
        ));
    }
    let overview = services.workbench_overview(&archive)?;
    let profile = services.project_profile(&archive)?;
    let points = services.decision_points(&archive)?;
    let text = view::workbench_markdown(&overview, &profile, &points);
    atomic_write(Path::new(target), text.as_bytes())?;
    Ok(format!(
        "工作台快照已导出到 {target}（{} 字节）",
        text.len()
    ))
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
                let Some((archive, decision, option)) = current_triple(&window, &state) else {
                    return;
                };
                let buffer = state.borrow().table_buffer.clone();
                let result = services
                    .open_engine(&archive)
                    .and_then(|engine| option_schema(&engine, &decision, &option))
                    .and_then(|schema| buffer_to_params(&schema, &buffer))
                    .and_then(|parameters| {
                        services.authoring_set_option_parameters(
                            &archive, &decision, &option, parameters,
                        )
                    });
                refresh_all(&window, &services, &state);
                report(&window, result.map(saved_message));
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_save_params(move |json| {
            if let Some(window) = weak.upgrade() {
                let Some((archive, decision, option)) = current_triple(&window, &state) else {
                    return;
                };
                let result = serde_json::from_str::<ParameterValues>(json.as_str())
                    .map_err(|error| Adm4Error::invalid_input(format!("参数 JSON 非法：{error}")))
                    .and_then(|parameters| {
                        services.authoring_set_option_parameters(
                            &archive, &decision, &option, parameters,
                        )
                    });
                refresh_all(&window, &services, &state);
                report(&window, result.map(saved_message));
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
    let Some((archive, decision, option)) = current_triple(window, state) else {
        window.set_advanced_mode(advanced);
        return;
    };
    let schema = match services
        .open_engine(&archive)
        .and_then(|engine| option_schema(&engine, &decision, &option))
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
                        Err(Adm4Error::invalid_input("请先在「存档管理」打开项目")),
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
///
/// 多选点确认后若仍缺主选，这里补一条可见提示（T9 已知缺口：访谈不代设主选）。
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
            refresh_interview(window, services, state);
            refresh_all(window, services, state);
            if state
                .borrow()
                .point(&proposal.decision_id)
                .is_some_and(view::primary_missing)
            {
                message.push_str("；该点是多选点且缺主选，请在中栏决策点详情里点「设主」");
            }
            report(window, Ok(message));
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
    let point = engine.space().graph.point(&proposal.decision_id);
    let question = point
        .map(|point| format!("（{}）", point.question))
        .unwrap_or_default();
    let option_label = point
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

/// 访谈分层进度：interview_progress 的各层「已确认/适用」与当前层。
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
// 冻结门 + 流水线 + 运行日志
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
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    report::<String>(&window, Err(Adm4Error::invalid_input("请先打开项目")));
                    return;
                };
                let result = services.freeze_red_team(&archive);
                refresh_all(&window, &services, &state);
                report(
                    &window,
                    result.map(|count| format!("红队评审完成，发现 {count} 项（见「风险」页签）")),
                );
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_freeze_check(move || {
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    report::<String>(&window, Err(Adm4Error::invalid_input("请先打开项目")));
                    return;
                };
                let result = services.freeze_check(&archive);
                refresh_all(&window, &services, &state);
                report(
                    &window,
                    result.map(|freeze_report| {
                        let passed = freeze_report
                            .gates
                            .iter()
                            .filter(|gate| gate.passed)
                            .count();
                        format!(
                            "四门预检完成：通过 {passed}/{}（明细见「校验」页签）",
                            freeze_report.gates.len()
                        )
                    }),
                );
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_freeze_run(move || {
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    return;
                };
                let result = services.freeze_run(&archive);
                if let Ok(frozen) = &result {
                    window.set_freeze_hint(SharedString::from(format!(
                        "已冻结 v{}：下一步到「开发流水线」运行 C0-C6",
                        frozen.version
                    )));
                }
                refresh_all(&window, &services, &state);
                refresh_pipeline(&window, &services, &state);
                report(
                    &window,
                    result.map(|frozen| {
                        format!(
                            "冻结成功 v{}（{}）：请前往「开发流水线」视图运行 C0-C6",
                            frozen.version, frozen.content_hash
                        )
                    }),
                );
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_pipeline_run(move || {
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    report::<String>(&window, Err(Adm4Error::invalid_input("请先打开项目")));
                    return;
                };
                let result = services.pipeline_run(&archive, "C0", "C6");
                refresh_pipeline(&window, &services, &state);
                refresh_logs(&window, &services, "");
                report(
                    &window,
                    result.map(|_| "C0-C6 运行结束（逐段状态见列表）".to_string()),
                );
            }
        });
    }
    {
        let services = services.clone();
        let state = state.clone();
        let weak = weak.clone();
        window.on_pipeline_confirm(move |stage, note| {
            if let Some(window) = weak.upgrade() {
                let Some(archive) = state.borrow().current_archive.clone() else {
                    return;
                };
                let result = services.pipeline_confirm(
                    &archive,
                    stage.as_str(),
                    "desktop_user",
                    note.as_str(),
                );
                refresh_pipeline(&window, &services, &state);
                report(
                    &window,
                    result.map(|_| format!("阶段 {stage} 已人工确认（署名 desktop_user）")),
                );
            }
        });
    }
    {
        let services = services.clone();
        let weak = weak.clone();
        window.on_refresh_logs(move |filter| {
            if let Some(window) = weak.upgrade() {
                refresh_logs(&window, &services, filter.as_str());
                report(&window, Ok("运行日志已刷新"));
            }
        });
    }
    {
        let weak = weak.clone();
        window.on_export_logs(move |path| {
            if let Some(window) = weak.upgrade() {
                let target = path.trim().to_string();
                let result = if target.is_empty() {
                    Err(Adm4Error::invalid_input("请先填写导出路径"))
                } else {
                    let rows: Vec<LogItem> = window.get_logs().iter().collect();
                    let text = view::log_markdown(&rows);
                    atomic_write(Path::new(&target), text.as_bytes())
                        .map(|()| format!("已导出 {} 条日志到 {target}", rows.len()))
                };
                report(&window, result);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// 模板：查看 / 预填 / 逆向维护产线五步 / 对照
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
                // 未选包时默认跟随当前项目的品类包（模板与项目必须同包）。
                // 注意：条件链里的 borrow() 会活到 if 语句结束，取值必须先落地再写回，
                // 否则 borrow_mut 与之重叠会 panic。
                let follow_project = {
                    let borrowed = state.borrow();
                    borrowed
                        .reverse_pack
                        .is_none()
                        .then(|| borrowed.current_archive.clone())
                        .flatten()
                };
                if let Some(archive) = follow_project {
                    match services.load_authoring_state(&archive) {
                        Ok(project) => state.borrow_mut().reverse_pack = Some(project.genre_pack),
                        Err(error) => report::<String>(&window, Err(error)),
                    }
                }
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
                    report::<String>(&window, Err(Adm4Error::invalid_input("请先打开项目")));
                    return;
                };
                let Some(template_id) = state.borrow().reverse_template.clone() else {
                    report::<String>(&window, Err(Adm4Error::invalid_input("请先选择模板")));
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
                        report(&window, Ok("对照查询完成（模板不进项目）".to_string()));
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
        window.on_template_prefill(move || {
            if let Some(window) = weak.upgrade() {
                let result = template_prefill(&services, &state);
                refresh_all(&window, &services, &state);
                report(&window, result);
            }
        });
    }
}

/// 认证模板预填到当前项目：走门面的 `project_prefill_template`（取用关卡 + 跳过计数 + 日志）。
/// 预填结果 provenance=Template、confirmed=false——必须逐条确认，且换皮门会拦截含原游戏名的理由。
fn template_prefill(services: &AppServices, state: &Rc<RefCell<UiState>>) -> Adm4Result<String> {
    let (archive, template_id) = {
        let borrowed = state.borrow();
        (
            borrowed.current_archive.clone(),
            borrowed.reverse_template.clone(),
        )
    };
    let Some(archive) = archive else {
        return Err(Adm4Error::invalid_input("请先打开项目"));
    };
    let Some(template_id) = template_id else {
        return Err(Adm4Error::invalid_input("请先在左侧选择模板"));
    };
    let report = services.project_prefill_template(&archive, &template_id)?;
    Ok(format!(
        "模板 {template_id} 预填：{}（未确认状态：请逐条确认，并重写含原游戏名的理由以过换皮门）",
        report.summary()
    ))
}

/// 逆向操作的前置：已选品类包与模板，缺一即报错并中止。
///
/// 返回的包是**模板所属包**（通用层模板即 `universal`），不是左栏选中的包——
/// 产线五步都要按模板所在目录读写。
fn reverse_pair(window: &MainWindow, state: &Rc<RefCell<UiState>>) -> Option<(String, String)> {
    let borrowed = state.borrow();
    match (
        borrowed
            .reverse_template_pack
            .clone()
            .or_else(|| borrowed.reverse_pack.clone()),
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
///
/// 列表用 `list_available`：本包模板 + 通用层模板（`genre_pack=universal`）。后者跨包可用，
/// 按包过滤会让 T10 迁入的 26 份内置模板在 UI 里既看不见也选不中。
/// 选中模板的**所属包**另记在 `reverse_template_pack`——逆向产线要写回原目录，不能按左栏包走。
fn refresh_reverse(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    let Some(pack) = state.borrow().reverse_pack.clone() else {
        window.set_template_list(ModelRc::new(VecModel::from(Vec::<TemplateRow>::new())));
        window.set_template_steps(ModelRc::new(VecModel::from(Vec::<StepRow>::new())));
        window.set_reverse_selected(SharedString::from("（请先选择品类包）"));
        return;
    };
    let templates = match services.templates().list_available(&pack) {
        Ok(templates) => templates,
        Err(error) => {
            window.set_template_list(ModelRc::new(VecModel::from(Vec::<TemplateRow>::new())));
            report::<String>(window, Err(error));
            return;
        }
    };
    let selected_id = state.borrow().reverse_template.clone();
    let rows: Vec<TemplateRow> = templates
        .iter()
        .map(|template| {
            let conflicts = persisted_conflict_count(template);
            let scope = if template.is_universal() {
                "通用层"
            } else {
                "本包"
            };
            TemplateRow {
                id: template.template_id.clone().into(),
                game: template.game_name.clone().into(),
                status: status_label(template.certification.status).into(),
                depth: format!("{:?}", template.depth_reached).into(),
                answers: if conflicts == 0 {
                    format!("{scope} · 答卷 {} 条", template.answers.len()).into()
                } else {
                    format!(
                        "{scope} · 答卷 {} 条，冲突 {conflicts} 条",
                        template.answers.len()
                    )
                    .into()
                },
                active: selected_id.as_deref() == Some(template.template_id.as_str()),
            }
        })
        .collect();
    window.set_template_list(ModelRc::new(VecModel::from(rows)));

    let selected = selected_id
        .and_then(|id| templates.iter().find(|template| template.template_id == id))
        .cloned();
    state.borrow_mut().reverse_template_pack = selected
        .as_ref()
        .map(|template| template.genre_pack.clone());
    match selected {
        Some(template) => {
            window.set_reverse_selected(SharedString::from(format!(
                "{}/{} · {} · 状态：{}",
                template.genre_pack,
                template.template_id,
                template.game_name,
                status_label(template.certification.status)
            )));
            window.set_template_steps(step_rows(template.certification.status));
            window.set_template_conflicts(persisted_conflicts(&template));
        }
        None => {
            window.set_reverse_selected(SharedString::from(format!(
                "{pack}（未选择模板：只有「已认证」模板可预填/对照）"
            )));
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

/// 「项目 + 决策点」前置；缺一即报错并中止（写入类回调统一入口）。
fn current_pair(window: &MainWindow, state: &Rc<RefCell<UiState>>) -> Option<(String, String)> {
    let borrowed = state.borrow();
    match (
        borrowed.current_archive.clone(),
        borrowed.current_decision.clone(),
    ) {
        (Some(archive), Some(decision)) => Some((archive, decision)),
        _ => {
            report::<String>(
                window,
                Err(Adm4Error::invalid_input("请先打开项目并在中栏选择决策点")),
            );
            None
        }
    }
}

/// 「项目 + 决策点 + 参数编辑目标选项」前置（多选点每个已选选项各有一份参数）。
fn current_triple(
    window: &MainWindow,
    state: &Rc<RefCell<UiState>>,
) -> Option<(String, String, String)> {
    let borrowed = state.borrow();
    match (
        borrowed.current_archive.clone(),
        borrowed.current_decision.clone(),
        borrowed.current_option.clone(),
    ) {
        (Some(archive), Some(decision), Some(option)) => Some((archive, decision, option)),
        _ => {
            report::<String>(
                window,
                Err(Adm4Error::invalid_input(
                    "请先选定选项（多选点请点选项行的「编辑参数」指定目标）",
                )),
            );
            None
        }
    }
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

/// 指定选项的参数 schema（结构化编辑器与模式切换共用）。
///
/// 注意：这里按**指定的选项**取 schema，而不是 `selection.option_id`——
/// 多选点的每个已选选项各有一份参数，只看首选项会编辑错对象。
fn option_schema(
    engine: &AuthoringEngine,
    decision_id: &str,
    option_id: &str,
) -> Adm4Result<ParameterSchema> {
    let point = engine
        .space()
        .graph
        .point(decision_id)
        .ok_or_else(|| Adm4Error::not_found(format!("决策点 {decision_id} 不存在")))?;
    let option = point
        .option(option_id)
        .ok_or_else(|| Adm4Error::not_found(format!("选项 {option_id} 不存在")))?;
    Ok(option.parameter_schema.clone())
}

/// 指定选项当前已存的参数（未选中该选项时返回 None）。
fn option_parameters(
    engine: &AuthoringEngine,
    decision_id: &str,
    option_id: &str,
) -> Option<ParameterValues> {
    engine
        .state()
        .selections
        .get(decision_id)?
        .selected_options()
        .into_iter()
        .find(|item| item.option_id == option_id)
        .map(|item| item.parameters.clone())
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
    window.set_table_rows_model(ModelRc::new(VecModel::from(view::table_model(buffer))));
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

/// 一次刷新工作台全部三栏（左/中列表 + 决策点详情 + 右栏四页签）。
/// 每个写入回调之后调用：UI 永远显示后端刚落盘的事实，不做本地推演。
///
/// 聚合查询只取一轮：迁移后的品类包清单有数 MB，每个查询内部都要重装设计空间，
/// 所以这里一次取齐后分发给三栏——右栏的领域/节点进度直接复用摘要里的同一份数据，
/// 不再单独调 `organization_progress`（同源同值，多调一次只是白装一遍清单）。
fn refresh_all(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    let Some(archive) = state.borrow().current_archive.clone() else {
        clear_workbench(window);
        return;
    };
    match load_workbench_data(services, &archive) {
        Ok(data) => {
            apply_workbench(window, state, &data);
            refresh_decision_panel(window, services, state);
            apply_overview(window, &data.overview);
        }
        Err(error) => report::<String>(window, Err(error)),
    }
}

/// 一轮工作台数据：全部来自 `adm4-app` 的只读聚合查询。
struct WorkbenchData {
    project: AuthoringState,
    /// 设计空间声明的全部领域（含 0 点域与保留领域），左栏卡片的骨架。
    domains: Vec<DesignDomain>,
    /// 领域/节点进度：取自 `workbench_overview` 的摘要（与 `organization_progress` 同源）。
    progress: OrganizationProgress,
    points: Vec<DecisionPointView>,
    profile: ProjectProfile,
    overview: WorkbenchOverview,
}

fn load_workbench_data(services: &AppServices, archive: &str) -> Adm4Result<WorkbenchData> {
    let project = services.load_authoring_state(archive)?;
    let overview = services.workbench_overview(archive)?;
    let points = services.decision_points(archive)?;
    let profile = services.project_profile(archive)?;
    let domains = services
        .load_space_shared(&project.genre_pack)?
        .organization
        .domains()
        .to_vec();
    let progress = OrganizationProgress {
        domains: overview.summary.domains.clone(),
        nodes: overview.summary.nodes.clone(),
        total: overview.summary.counts,
    };
    Ok(WorkbenchData {
        project,
        domains,
        progress,
        points,
        profile,
        overview,
    })
}

fn refresh_projects(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    // 高亮当前项目按存档 id 判定（项目名可重复，不能当身份用）。
    let current = state.borrow().current_archive.clone();
    let items: Vec<ProjectItem> = match services.project_list() {
        Ok(list) => list
            .into_iter()
            .map(|manifest| ProjectItem {
                active: current.as_deref() == Some(manifest.archive_id.as_str()),
                id: manifest.archive_id.into(),
                name: manifest.project_name.into(),
                updated: manifest.updated_at.into(),
            })
            .collect(),
        Err(error) => {
            report::<String>(window, Err(error));
            Vec::new()
        }
    };
    window.set_projects(ModelRc::new(VecModel::from(items)));
}

fn refresh_packs(window: &MainWindow, services: &AppServices) {
    match services.list_packs() {
        Ok(packs) => {
            let packs: Vec<SharedString> = packs.into_iter().map(SharedString::from).collect();
            window.set_packs(ModelRc::new(VecModel::from(packs)));
        }
        Err(error) => report::<String>(window, Err(error)),
    }
}

fn refresh_logs(window: &MainWindow, services: &AppServices, filter: &str) {
    match services.log.tail(400) {
        Ok(entries) => {
            window.set_logs(ModelRc::new(VecModel::from(view::log_rows(
                entries, filter,
            ))));
        }
        Err(error) => report::<String>(window, Err(error)),
    }
}

/// 左栏（领域卡片 + 画像）与中栏（节点列表 + 检查单 + 节点文本）。
fn apply_workbench(window: &MainWindow, state: &Rc<RefCell<UiState>>, data: &WorkbenchData) {
    let WorkbenchData {
        project,
        domains,
        progress,
        points,
        profile,
        ..
    } = data;

    // 归一化当前选择：领域/节点/决策点若已不存在（切项目、清单变更），回落为未选。
    {
        let mut borrowed = state.borrow_mut();
        borrowed.points = points.clone();
        if let Some(domain) = borrowed.current_domain.clone()
            && !domains.iter().any(|item| item.id == domain)
        {
            borrowed.current_domain = None;
        }
        let domain_filter = borrowed.current_domain.clone();
        if let Some(node) = borrowed.current_node.clone()
            && !progress.nodes.iter().any(|item| {
                item.node_id == node
                    && domain_filter
                        .as_deref()
                        .is_none_or(|domain| item.domain_id == domain)
            })
        {
            borrowed.current_node = None;
        }
        if let Some(decision) = borrowed.current_decision.clone()
            && !points.iter().any(|point| point.decision_id == decision)
        {
            borrowed.current_decision = None;
            borrowed.current_option = None;
        }
    }

    let (domain_filter, node_filter, decision_filter) = {
        let borrowed = state.borrow();
        (
            borrowed.current_domain.clone(),
            borrowed.current_node.clone(),
            borrowed.current_decision.clone(),
        )
    };

    window.set_project_title(SharedString::from(project.project_name.clone()));
    window.set_project_pack(SharedString::from(format!(
        "{} @ {} · 深度档 {:?} · 修订 {}",
        project.genre_pack, project.pack_version, project.depth_profile.target, project.revision
    )));
    window.set_domain_cards(ModelRc::new(VecModel::from(view::domain_cards(
        domains,
        progress,
        domain_filter.as_deref(),
    ))));
    window.set_node_cards(ModelRc::new(VecModel::from(view::node_cards(
        progress,
        domain_filter.as_deref(),
        node_filter.as_deref(),
    ))));
    window.set_check_rows(ModelRc::new(VecModel::from(view::check_rows(
        points,
        domain_filter.as_deref(),
        node_filter.as_deref(),
        decision_filter.as_deref(),
    ))));
    window.set_profile_rows(ModelRc::new(VecModel::from(view::profile_rows(profile))));
    window.set_profile_title(SharedString::from(format!(
        "项目画像（L0/L1 已确认 {} 项）",
        profile.fields.len()
    )));

    let domain_name = domain_filter
        .as_deref()
        .and_then(|id| domains.iter().find(|domain| domain.id == id))
        .map(|domain| domain.name.clone());
    window.set_center_title(SharedString::from(match &domain_name {
        Some(name) => format!("节点列表 · 领域「{name}」"),
        None => "节点列表 · 全部领域（点击左栏领域卡片可过滤）".to_string(),
    }));

    // 节点详情：名称/角色 + 节点级设计说明与风险说明（按节点保存）。
    match node_filter
        .as_deref()
        .and_then(|node_id| progress.node(node_id))
    {
        Some(node) => {
            window.set_node_title(SharedString::from(format!(
                "节点：{}（{}）",
                node.name, node.node_id
            )));
            window.set_node_meta(SharedString::from(format!(
                "角色分类 {} · 已确认 {}/{} · 决策点 {}",
                if node.role_class.is_empty() {
                    "未标注"
                } else {
                    node.role_class.as_str()
                },
                node.counts.confirmed,
                node.counts.applicable,
                node.counts.total_points
            )));
            window.set_node_design_note(SharedString::from(
                project
                    .node_design_notes
                    .get(&node.node_id)
                    .cloned()
                    .unwrap_or_default(),
            ));
            window.set_node_risk_note(SharedString::from(
                project
                    .node_risk_notes
                    .get(&node.node_id)
                    .cloned()
                    .unwrap_or_default(),
            ));
        }
        None => {
            window.set_node_title(SharedString::from("（未选择节点）"));
            window.set_node_meta(SharedString::from(
                "点击上方节点卡片以填写节点级设计说明/风险说明",
            ));
            window.set_node_design_note(SharedString::default());
            window.set_node_risk_note(SharedString::default());
        }
    }

    window.set_completeness_text(SharedString::from(format!(
        "完成度 {}/{}（{}%）· N/A {}",
        progress.total.confirmed,
        progress.total.applicable,
        progress.total.percent(),
        progress.total.not_applicable
    )));
    window.set_completeness_ratio(if progress.total.applicable == 0 {
        0.0
    } else {
        progress.total.confirmed as f32 / progress.total.applicable as f32
    });
}

fn clear_workbench(window: &MainWindow) {
    window.set_domain_cards(ModelRc::new(VecModel::from(Vec::<DomainCard>::new())));
    window.set_node_cards(ModelRc::new(VecModel::from(Vec::<NodeCard>::new())));
    window.set_check_rows(ModelRc::new(VecModel::from(Vec::<CheckRow>::new())));
    window.set_profile_rows(ModelRc::new(VecModel::from(Vec::<ProfileRow>::new())));
    window.set_options(ModelRc::new(VecModel::from(Vec::<OptionRow>::new())));
    window.set_summary_rows(ModelRc::new(VecModel::from(Vec::<TextRow>::new())));
    window.set_missing_rows(ModelRc::new(VecModel::from(Vec::<TextRow>::new())));
    window.set_risk_rows(ModelRc::new(VecModel::from(Vec::<TextRow>::new())));
    window.set_validation_rows(ModelRc::new(VecModel::from(Vec::<TextRow>::new())));
    window.set_project_title(SharedString::from("（未打开项目）"));
    window.set_project_pack(SharedString::default());
    window.set_completeness_text(SharedString::default());
    window.set_completeness_ratio(0.0);
    window.set_decision_title(SharedString::from("（未选择决策点）"));
    window.set_decision_meta(SharedString::default());
    window.set_decision_question(SharedString::default());
    window.set_decision_active(false);
    window.set_structured_active(false);
    window.set_level_brief(SharedString::from(view::level_brief(None, "")));
}

/// 中栏决策点详情：选项行（单/多选 + 主选）、参数编辑器、豁免记录、可见拦截。
fn refresh_decision_panel(
    window: &MainWindow,
    services: &AppServices,
    state: &Rc<RefCell<UiState>>,
) {
    let (archive, decision, focused) = {
        let borrowed = state.borrow();
        (
            borrowed.current_archive.clone(),
            borrowed.current_decision.clone(),
            borrowed.current_option.clone(),
        )
    };
    let (Some(archive), Some(decision)) = (archive, decision) else {
        window.set_decision_title(SharedString::from("（未选择决策点）"));
        window.set_decision_meta(SharedString::default());
        window.set_decision_question(SharedString::default());
        window.set_decision_active(false);
        window.set_decision_exempted(false);
        window.set_decision_primary_missing(false);
        window.set_options(ModelRc::new(VecModel::from(Vec::<OptionRow>::new())));
        window.set_structured_active(false);
        window.set_focused_option_text(SharedString::default());
        window.set_level_brief(SharedString::from(view::level_brief(None, "")));
        return;
    };
    let point = match state.borrow().point(&decision).cloned() {
        Some(point) => point,
        None => return,
    };

    window.set_decision_title(SharedString::from(view::decision_title(&point)));
    window.set_decision_meta(SharedString::from(view::decision_meta(&point)));
    window.set_decision_question(SharedString::from(view::design_question_text(&point)));
    window.set_decision_active(point.applicability == "active");
    window.set_decision_multi(point.selection_mode.is_multi());
    window.set_decision_allow_primary(point.selection_mode.requires_primary());
    window.set_decision_primary_missing(view::primary_missing(&point));
    window.set_decision_exempted(point.exemption.is_some());
    window.set_decision_exemption_text(SharedString::from(view::exemption_text(&point)));
    window.set_options(ModelRc::new(VecModel::from(view::option_rows(
        &point,
        focused.as_deref(),
    ))));

    // 参数编辑目标：未指定时取主选，其次首个已选选项。
    let focused = focused.filter(|id| point.options.iter().any(|option| &option.option_id == id));
    let focused = focused.or_else(|| {
        point
            .options
            .iter()
            .find(|option| option.is_primary && option.selected)
            .or_else(|| point.options.iter().find(|option| option.selected))
            .map(|option| option.option_id.clone())
    });
    state.borrow_mut().current_option = focused.clone();

    let Some(option_id) = focused else {
        window.set_focused_option_text(SharedString::from(
            "参数编辑：请先选定选项（选定后按 schema 自动出现表格/矩阵或 JSON 编辑区）",
        ));
        window.set_structured_active(false);
        window.set_param_json(SharedString::default());
        push_table_model(window, &[]);
        {
            let mut borrowed = state.borrow_mut();
            borrowed.table_buffer.clear();
            borrowed.editor = EditorKind::None;
        }
        window.set_level_brief(SharedString::from(view::level_brief(
            Some(&point),
            "未选定选项（无参数结构）",
        )));
        return;
    };

    let engine = match services.open_engine(&archive) {
        Ok(engine) => engine,
        Err(error) => {
            report::<String>(window, Err(error));
            return;
        }
    };
    let schema = match option_schema(&engine, &decision, &option_id) {
        Ok(schema) => schema,
        Err(error) => {
            report::<String>(window, Err(error));
            return;
        }
    };
    let parameters = option_parameters(&engine, &decision, &option_id).unwrap_or(Params::None);
    let params_json = match params_to_pretty_json(&parameters) {
        Ok(json) => json,
        Err(error) => {
            report::<String>(window, Err(error));
            String::new()
        }
    };
    window.set_param_json(SharedString::from(params_json));

    let mut editor = EditorKind::None;
    let mut buffer: Vec<Vec<String>> = Vec::new();
    let mut columns: Vec<SharedString> = Vec::new();
    let mut title = String::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut schema_label = "标量/无参数（JSON 编辑）".to_string();
    match &schema {
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
            schema_label = format!("表结构（行标识 {}）", table.row_key);
            let (initial, warns) = convert::table_buffer_from_params(table, &parameters);
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
            schema_label = "矩阵结构（行轴 × 列轴）".to_string();
            let (initial, warns) = convert::matrix_buffer_from_params(&parameters);
            buffer = initial;
            warnings = warns;
        }
        ParameterSchema::Scalar { fields } => {
            schema_label = format!("标量字段 {} 个（JSON 编辑）", fields.len());
        }
        ParameterSchema::None => {}
    }
    window.set_focused_option_text(SharedString::from(format!(
        "参数编辑目标：{option_id} · {schema_label}"
    )));
    window.set_structured_active(editor != EditorKind::None);
    window.set_editor_title(SharedString::from(title));
    window.set_table_columns(ModelRc::new(VecModel::from(columns)));
    push_table_model(window, &buffer);
    {
        let mut borrowed = state.borrow_mut();
        borrowed.table_buffer = buffer;
        borrowed.editor = editor;
    }
    window.set_level_brief(SharedString::from(view::level_brief(
        Some(&point),
        &schema_label,
    )));
    if !warnings.is_empty() {
        report(window, Ok(warnings.join("；")));
    }
}

/// 右栏四页签 + 「执行冻结」可用性（只看后端 all_gates_passed）。
fn apply_overview(window: &MainWindow, overview: &WorkbenchOverview) {
    window.set_summary_rows(ModelRc::new(VecModel::from(view::summary_rows(overview))));
    window.set_missing_rows(ModelRc::new(VecModel::from(view::missing_rows(overview))));
    window.set_risk_rows(ModelRc::new(VecModel::from(view::risk_rows(overview))));
    window.set_validation_rows(ModelRc::new(VecModel::from(view::validation_rows(
        overview,
    ))));
    window.set_freeze_ready(overview.validation.all_gates_passed);
}

/// 流水线全版图：C0-C6 状态来自 runner；未冻结时列表照样呈现（提示先冻结）。
fn refresh_pipeline(window: &MainWindow, services: &AppServices, state: &Rc<RefCell<UiState>>) {
    let Some(archive) = state.borrow().current_archive.clone() else {
        window.set_stages(ModelRc::new(VecModel::from(view::stage_rows(None))));
        return;
    };
    // 未冻结时 pipeline_status 返回 not_found：这是正常前置状态，不当错误报。
    let run_state = services.pipeline_status(&archive).ok();
    window.set_stages(ModelRc::new(VecModel::from(view::stage_rows(
        run_state.as_ref(),
    ))));
}
