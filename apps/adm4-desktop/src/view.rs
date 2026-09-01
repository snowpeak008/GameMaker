//! 后端聚合结果 → UI 行模型的纯装配层。
//!
//! 边界约定（D14）：本模块只做「后端 DTO → 展示文本/行模型」的确定性转换，
//! 不判定任何业务规则（完成度口径、门禁通过、适用性都由后端给出，这里照实呈现）。
//! 之所以单独成模块：装配逻辑（领域卡片要含 0 点域、徽章取值、导出快照排版）
//! 是本任务里唯一有分支的逻辑，必须能被单元测试钉住，而 Slint 回调不便测试。

use crate::{
    ChangeRow, CheckRow, DeliverRow, DomainCard, LogItem, NodeCard, OptionRow, ProfileRow, SdkRow,
    StageItem, StyleCard, TextRow,
};
use adm4_ai::{HttpImageProviderConfig, HttpProviderConfig};
use adm4_app::{
    AiDoctorReport, AiInvokeCheckReport, ChangeRequest, ChangeStatus, DecisionPointView,
    DeliverableManifest, ProjectDoctorReport, ProjectProfile, RunLogEntry, SdkReviewStatus,
    SdkSnapshot, StageArtifactView, StyleAnchorSet, StyleApplicationContract, StyleDirection,
    StyleDirectionStatus, StyleFitRisk, StyleGateStatus, TemplateExportReport, WorkbenchOverview,
};
use adm4_authoring::WorkbenchResetReport;
use adm4_build::pending_stage;
use adm4_decision::{
    DesignDomain, OrganizationProgress, SelectionMode, UNASSIGNED_DOMAIN_ID, UNASSIGNED_NODE_ID,
};
use adm4_pipeline::{
    PipelineRunState, StageRecord, StageResetReport, StageStatus, design_compile_registry,
    phase2_registry,
};
use slint::{Image, SharedString, VecModel};

/// 左栏领域卡片：**全部**领域（含 0 决策点的域）+ 进度 join。
///
/// `OrganizationProgress.domains` 只含有决策点的域（T9 口径），因此这里以设计空间
/// 声明的领域清单为骨架，把进度 join 进去；无点的域照实显示「0 点」而不是不出现——
/// 否则用户在 16 领域巡视时会以为领域丢了。保留领域「未分域」只在真有点时才显示。
pub fn domain_cards(
    domains: &[DesignDomain],
    progress: &OrganizationProgress,
    active: Option<&str>,
) -> Vec<DomainCard> {
    let mut cards = Vec::with_capacity(domains.len());
    for domain in domains {
        let matched = progress.domain(&domain.id);
        if matched.is_none() && domain.id == UNASSIGNED_DOMAIN_ID {
            continue;
        }
        let (progress_text, summary, empty) = match matched {
            Some(item) => {
                let mut text = format!(
                    "{}/{} 已确认 · {}%",
                    item.counts.confirmed, item.counts.applicable, item.percent
                );
                if item.counts.not_applicable > 0 {
                    text.push_str(&format!(" · N/A {}", item.counts.not_applicable));
                }
                let summary = if domain.description.trim().is_empty() {
                    format!(
                        "节点 {} · 决策点 {}",
                        item.node_count, item.counts.total_points
                    )
                } else {
                    domain.description.clone()
                };
                (text, summary, false)
            }
            None => (
                "0/0 · 本域暂无决策点".to_string(),
                if domain.description.trim().is_empty() {
                    "等待清单为该领域挂载节点与决策点".to_string()
                } else {
                    domain.description.clone()
                },
                true,
            ),
        };
        cards.push(DomainCard {
            id: domain.id.clone().into(),
            name: domain.name.clone().into(),
            summary: summary.into(),
            progress: progress_text.into(),
            percent: matched.map_or(0, |item| i32::from(item.percent)),
            active: active == Some(domain.id.as_str()),
            empty,
        });
    }
    cards
}

/// 中栏节点列表：当前领域下的节点（未选领域 = 全部节点）。
pub fn node_cards(
    progress: &OrganizationProgress,
    domain: Option<&str>,
    active: Option<&str>,
) -> Vec<NodeCard> {
    progress
        .nodes
        .iter()
        .filter(|node| domain.is_none_or(|id| node.domain_id == id))
        .map(|node| {
            let mut progress_text = format!(
                "{}/{} · {}%",
                node.counts.confirmed, node.counts.applicable, node.percent
            );
            if node.counts.not_applicable > 0 {
                progress_text.push_str(&format!(" · N/A {}", node.counts.not_applicable));
            }
            let role = if node.role_class.trim().is_empty() {
                "角色分类：未标注".to_string()
            } else {
                format!("角色分类：{}", node.role_class)
            };
            let detail = if node.description.trim().is_empty() {
                format!("决策点 {} 个（含未激活/超深度）", node.counts.total_points)
            } else {
                node.description.clone()
            };
            NodeCard {
                id: node.node_id.clone().into(),
                name: if node.node_id == UNASSIGNED_NODE_ID {
                    format!("{}（清单尚未挂载节点）", node.name).into()
                } else {
                    node.name.clone().into()
                },
                role: role.into(),
                progress: progress_text.into(),
                detail: detail.into(),
                active: active == Some(node.node_id.as_str()),
            }
        })
        .collect()
}

/// 决策点检查单：优先按节点过滤，其次按领域，二者皆无 = 全图。
pub fn check_rows(
    points: &[DecisionPointView],
    domain: Option<&str>,
    node: Option<&str>,
    active: Option<&str>,
) -> Vec<CheckRow> {
    points
        .iter()
        .filter(|point| match (node, domain) {
            (Some(node_id), _) => point.node_id == node_id,
            (None, Some(domain_id)) => point.domain_id == domain_id,
            (None, None) => true,
        })
        .map(|point| {
            let (badge, kind) = badge_of(point);
            CheckRow {
                id: point.decision_id.clone().into(),
                question: point.question.clone().into(),
                meta: decision_meta(point).into(),
                badge: badge.into(),
                badge_kind: kind.into(),
                active: active == Some(point.decision_id.as_str()),
            }
        })
        .collect()
}

/// 状态徽章：文案 + 配色分类（done/warn/off/bad）。
///
/// 判定只读后端给出的 `applicability` 与 `confirmed`，UI 不自行推断适用性。
pub fn badge_of(point: &DecisionPointView) -> (&'static str, &'static str) {
    match point.applicability.as_str() {
        "beyond_depth" => ("超深度", "off"),
        "inactive" => ("未激活", "off"),
        "not_applicable" => ("N/A 豁免", "warn"),
        _ if point.confirmed => ("已确认", "done"),
        _ if point.options.iter().any(|option| option.selected) => ("待确认", "warn"),
        _ => ("未选", "bad"),
    }
}

/// 决策点副标题：层级 · MDA 层 · 选择模式 · 必做性标记 · id。
pub fn decision_meta(point: &DecisionPointView) -> String {
    let mut parts = vec![point.level.label().to_string()];
    if let Some(layer) = &point.mda_layer {
        parts.push(format!("MDA {layer}"));
    }
    parts.push(selection_mode_label(point.selection_mode).to_string());
    match point.requirement {
        adm4_decision::PointRequirement::Baseline => {
            parts.push("基线点（可理由码跳过）".to_string());
        }
        adm4_decision::PointRequirement::Optional => {
            parts.push("非必做（不进完成度分母）".to_string());
        }
        adm4_decision::PointRequirement::Unlocked => {}
    }
    parts.push(point.decision_id.clone());
    parts.join(" · ")
}

pub fn selection_mode_label(mode: SelectionMode) -> &'static str {
    match mode {
        SelectionMode::Single => "单选",
        SelectionMode::Multi {
            allow_primary: true,
        } => "多选（需设主选）",
        SelectionMode::Multi {
            allow_primary: false,
        } => "多选",
    }
}

/// 决策点详情标题（问题 + 已选摘要）。
pub fn decision_title(point: &DecisionPointView) -> String {
    let selected: Vec<String> = point
        .options
        .iter()
        .filter(|option| option.selected)
        .map(|option| {
            if option.is_primary {
                format!("★{}", option.label)
            } else {
                option.label.clone()
            }
        })
        .collect();
    if selected.is_empty() {
        point.question.clone()
    } else {
        format!("{}    已选：{}", point.question, selected.join("、"))
    }
}

/// 设计提问（二版 designQuestion）；清单未声明时给出说明而不是空白。
pub fn design_question_text(point: &DecisionPointView) -> String {
    match &point.design_question {
        Some(text) => format!("设计提问：{text}"),
        None => "设计提问：清单未声明（迁移后由 design_question 字段提供）".to_string(),
    }
}

/// N/A 豁免在案记录（理由码 + 说明 + 署名 + 时间）。
pub fn exemption_text(point: &DecisionPointView) -> String {
    match &point.exemption {
        Some(exemption) => {
            let signature = match (&exemption.actor, &exemption.at) {
                (Some(actor), Some(at)) => format!("署名 {actor}（{at}）"),
                (Some(actor), None) => format!("署名 {actor}"),
                _ => "无署名（baseline 理由码跳过）".to_string(),
            };
            let note = if exemption.note.trim().is_empty() {
                String::new()
            } else {
                format!("：{}", exemption.note)
            };
            format!("已豁免[{}]{note}，{signature}", exemption.reason_code)
        }
        None => String::new(),
    }
}

/// 多选点已选但缺主选 —— 需要在详情里可见拦截（T9 已知缺口：访谈确认不代设主选）。
pub fn primary_missing(point: &DecisionPointView) -> bool {
    point.selection_mode.requires_primary()
        && point.options.iter().any(|option| option.selected)
        && !point.options.iter().any(|option| option.is_primary)
}

pub fn option_rows(point: &DecisionPointView, focused: Option<&str>) -> Vec<OptionRow> {
    point
        .options
        .iter()
        .map(|option| OptionRow {
            id: option.option_id.clone().into(),
            label: option.label.clone().into(),
            summary: option.summary.clone().into(),
            selected: option.selected,
            is_primary: option.is_primary,
            focused: focused == Some(option.option_id.as_str()),
        })
        .collect()
}

/// 左栏「L5/L6 编辑入口」速览：当前点的层级/模式/参数结构与编辑落位说明。
pub fn level_brief(point: Option<&DecisionPointView>, editor_kind: &str) -> String {
    match point {
        None => "尚未选择决策点。\n二版的 L5 JSON 编辑框在四版被结构化表格/矩阵编辑覆盖，\
编辑区位于中栏「决策点详情」内（选项选定后按 schema 自动出现）。"
            .to_string(),
        Some(point) => format!(
            "当前点：{}\n层级：{}\n模式：{}\n领域/节点：{} / {}\n参数结构：{}\n（表格/矩阵编辑在中栏决策点详情内；高级 JSON 模式可互切）",
            point.decision_id,
            point.level.label(),
            selection_mode_label(point.selection_mode),
            point.domain_id,
            point.node_id,
            editor_kind
        ),
    }
}

/// 画像卡：逐字段「问题 → 已选选项（主选标记）」。
pub fn profile_rows(profile: &ProjectProfile) -> Vec<ProfileRow> {
    if profile.fields.is_empty() {
        return vec![ProfileRow {
            label: "画像尚未成形".into(),
            value: "L0/L1 决策点确认后自动汇总".into(),
            hint: "画像不单独存储，数据源就是 L0/L1 已确认决策点".into(),
        }];
    }
    profile
        .fields
        .iter()
        .map(|field| {
            let value = field
                .selected
                .iter()
                .map(|option| {
                    if option.is_primary {
                        format!("★{}", option.label)
                    } else {
                        option.label.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("、");
            let mut hint = vec![field.level.label().to_string()];
            if let Some(layer) = &field.mda_layer {
                hint.push(format!("MDA {layer}"));
            }
            if let Some(question) = &field.design_question {
                hint.push(question.clone());
            }
            ProfileRow {
                label: field.label.clone().into(),
                value: value.into(),
                hint: hint.join(" · ").into(),
            }
        })
        .collect()
}

fn header_row(title: impl Into<String>) -> TextRow {
    TextRow {
        title: title.into().into(),
        text: SharedString::default(),
        kind: "header".into(),
        target: SharedString::default(),
    }
}

fn info_row(title: impl Into<String>, text: impl Into<String>, kind: &str) -> TextRow {
    TextRow {
        title: title.into().into(),
        text: text.into().into(),
        kind: kind.into(),
        target: SharedString::default(),
    }
}

/// 右栏「摘要」：项目档案 + 总完成度 + 领域 × 进度。
pub fn summary_rows(overview: &WorkbenchOverview) -> Vec<TextRow> {
    let summary = &overview.summary;
    let mut rows = vec![
        header_row("项目档案"),
        info_row(
            format!("{}（{}）", summary.project_name, summary.genre_pack),
            format!(
                "包版本 {} · 深度档 {:?} · 修订 {} · 已冻结 {} 版",
                summary.pack_version,
                summary.depth_target,
                summary.revision,
                summary.frozen_versions
            ),
            "info",
        ),
        header_row("总完成度"),
        info_row(
            format!("{}/{}（{}%）", summary.done, summary.total, summary.percent),
            format!(
                "适用 {} · 已确认 {} · N/A {} · 决策点总数 {}",
                summary.counts.applicable,
                summary.counts.confirmed,
                summary.counts.not_applicable,
                summary.counts.total_points
            ),
            if summary.done == summary.total {
                "ok"
            } else {
                "info"
            },
        ),
        header_row("领域 × 进度"),
    ];
    if summary.domains.is_empty() {
        rows.push(info_row(
            "暂无领域进度",
            "设计空间尚未挂载领域（迁移前全部点归「未分域」）",
            "info",
        ));
    }
    for domain in &summary.domains {
        rows.push(info_row(
            format!("{}  {}%", domain.name, domain.percent),
            format!(
                "已确认 {}/{} · 节点 {} · N/A {}",
                domain.counts.confirmed,
                domain.counts.applicable,
                domain.node_count,
                domain.counts.not_applicable
            ),
            if domain.counts.is_complete() {
                "ok"
            } else {
                "info"
            },
        ));
    }
    rows.push(header_row("节点进度"));
    for node in &summary.nodes {
        rows.push(info_row(
            format!("{}  {}%", node.name, node.percent),
            format!(
                "已确认 {}/{} · 角色 {}",
                node.counts.confirmed,
                node.counts.applicable,
                if node.role_class.is_empty() {
                    "未标注"
                } else {
                    node.role_class.as_str()
                }
            ),
            if node.counts.is_complete() {
                "ok"
            } else {
                "info"
            },
        ));
    }
    rows
}

/// 右栏「缺失项」：按领域分组的未确认清单；每条带 target 供点击跳转中栏。
pub fn missing_rows(overview: &WorkbenchOverview) -> Vec<TextRow> {
    if overview.missing.is_empty() {
        return vec![info_row(
            "没有缺失项",
            "全部适用决策点已确认且参数校验通过",
            "ok",
        )];
    }
    let mut rows = Vec::new();
    for group in &overview.missing {
        rows.push(header_row(format!(
            "{}（{} 项待办）",
            group.domain_name,
            group.items.len()
        )));
        for item in &group.items {
            rows.push(TextRow {
                title: format!("[{}] {}", item.level.label(), item.question).into(),
                text: format!("{} · {}", item.decision_id, item.reasons.join("；")).into(),
                kind: "bad".into(),
                target: item.decision_id.clone().into(),
            });
        }
    }
    rows
}

/// 右栏「风险」：节点风险说明汇总 + 红队摘要（含过期标记）。
pub fn risk_rows(overview: &WorkbenchOverview) -> Vec<TextRow> {
    let mut rows = vec![header_row("节点风险说明")];
    if overview.risk.node_risks.is_empty() {
        rows.push(info_row(
            "暂无节点风险说明",
            "在中栏节点详情填写「风险说明」后汇总到此",
            "info",
        ));
    }
    for note in &overview.risk.node_risks {
        rows.push(info_row(
            format!("{}（{}）", note.node_name, note.domain_id),
            note.note.clone(),
            "info",
        ));
    }
    rows.push(header_row("AI 红队"));
    match &overview.risk.red_team {
        None => rows.push(info_row(
            "尚未运行红队",
            "冻结门第 4 道要求红队记录针对当前修订",
            "info",
        )),
        Some(red_team) => {
            rows.push(info_row(
                if red_team.stale {
                    format!(
                        "红队记录已过期（评审于修订 {}，设计已再变更）",
                        red_team.reviewed_revision
                    )
                } else {
                    format!("红队记录有效（修订 {}）", red_team.reviewed_revision)
                },
                format!(
                    "评审人 {} · 发现 {} 项 · 已处置 {} 项",
                    red_team.reviewer,
                    red_team.findings.len(),
                    red_team
                        .findings
                        .iter()
                        .filter(|finding| finding.disposed)
                        .count()
                ),
                if red_team.stale { "bad" } else { "ok" },
            ));
            for finding in &red_team.findings {
                rows.push(info_row(
                    format!(
                        "[{}] {}{}",
                        finding.severity,
                        finding.target,
                        if finding.disposed {
                            "（已处置）"
                        } else {
                            ""
                        }
                    ),
                    finding.text.clone(),
                    if finding.disposed { "info" } else { "bad" },
                ));
            }
        }
    }
    rows
}

/// 右栏「校验」：外键违规 + 四门预检 pass/block 明细。
///
/// 门 1 即使 `passed=true` 也可能带 findings（N/A 豁免的可见性条目），
/// 因此通过与否只看 `passed`，明细照实列出——不拿「findings 非空」当失败。
pub fn validation_rows(overview: &WorkbenchOverview) -> Vec<TextRow> {
    let validation = &overview.validation;
    let mut rows = vec![header_row("跨表外键")];
    if validation.row_reference_violations.is_empty() {
        rows.push(info_row("无外键违规", "跨表引用全部命中目标行键", "ok"));
    }
    for issue in &validation.row_reference_violations {
        rows.push(info_row(
            format!("{} · {}", issue.rule_id, issue.decision_id),
            issue.detail.clone(),
            "bad",
        ));
    }
    rows.push(header_row(format!(
        "冻结门预检（{}）",
        if validation.all_gates_passed {
            "全绿，可执行冻结"
        } else {
            "未全绿，冻结被拦"
        }
    )));
    for gate in &validation.gates {
        rows.push(info_row(
            format!(
                "{} {} · {}",
                if gate.passed { "✔" } else { "✘" },
                gate.gate,
                if gate.passed { "通过" } else { "未通过" }
            ),
            format!(
                "明细 {} 条（门 1 的 N/A 豁免条目属可见性记录，不参与通过判定）",
                gate.finding_count
            ),
            if gate.passed { "ok" } else { "bad" },
        ));
        for finding in &gate.findings {
            rows.push(info_row(
                format!("    [{}]", finding.code),
                finding.message.clone(),
                if gate.passed { "info" } else { "bad" },
            ));
        }
    }
    rows
}

/// 流水线全版图：C0-C6（实跑，状态来自 runner）+ P0-P5（G3 起 P0/P2 实跑，其余待实现）。
///
/// 每行按 `docs/design/06` §4 要求带齐「状态 / 耗时 / 产物入口」：耗时来自
/// `StageRecord::duration_seconds`，产物入口与重跑按钮的可用性一律由后端状态推出
/// （从未开始执行的段没有产物可看、也没有产物需要作废，因此两个按钮都不出）。
///
/// 兼容入口：P 段无构建状态（等价 `stage_rows_with_build(run_state, None)`）。
pub fn stage_rows(run_state: Option<&PipelineRunState>) -> Vec<StageItem> {
    stage_rows_with_build(run_state, None)
}

/// 全版图 + Phase 2 构建运行状态：P 段有记录显示真状态（含耗时），没跑过的段
/// 按待实现登记表给文案（已实现但未跑 = 「待运行」）。
pub fn stage_rows_with_build(
    run_state: Option<&PipelineRunState>,
    build_state: Option<&PipelineRunState>,
) -> Vec<StageItem> {
    let mut rows: Vec<StageItem> = design_compile_registry()
        .into_iter()
        .map(|stage| {
            let record = run_state.and_then(|state| state.stages.get(&stage.id));
            let (status, waiting) = match run_state.map(|state| state.stage_status(&stage.id)) {
                None => ("待运行（先完成设计冻结）".to_string(), false),
                Some(StageStatus::Pending) => ("待运行".to_string(), false),
                Some(StageStatus::Running) => ("运行中".to_string(), false),
                Some(StageStatus::Succeeded) => ("成功".to_string(), false),
                Some(StageStatus::Failed { reasons }) => {
                    (format!("失败：{}", reasons.join("；")), false)
                }
                Some(StageStatus::Blocked { reasons }) => {
                    (format!("阻塞：{}", reasons.join("；")), false)
                }
                Some(StageStatus::WaitingHuman { gate }) => {
                    (format!("等待人工确认（{gate}）"), true)
                }
            };
            let running = matches!(record.map(|item| &item.status), Some(StageStatus::Running));
            // 「跑过」= 后端给这段留了非 Pending 的记录；没跑过就没有产物入口与重跑入口。
            let touched = record.is_some_and(|item| !matches!(item.status, StageStatus::Pending));
            StageItem {
                id: stage.id.into(),
                name: stage.name.into(),
                status: status.into(),
                summary: stage.summary.into(),
                segment: "C 段".into(),
                duration: duration_text(record.and_then(StageRecord::duration_seconds)).into(),
                waiting,
                running,
                can_rerun: touched,
                can_inspect: touched,
                placeholder: false,
            }
        })
        .collect();
    rows.extend(phase2_registry().into_iter().map(|stage| {
        let record = build_state.and_then(|state| state.stages.get(&stage.id));
        // 有真实运行记录 → 与 C 段同一套状态渲染；没有 → 待实现文案（未实现段）
        // 或「待运行」（已实现但还没跑）。文案不在 UI 里写死，跟着注册表与登记表变。
        let (status, waiting, placeholder) = match record.map(|item| &item.status) {
            Some(StageStatus::Pending) => ("待运行".to_string(), false, false),
            Some(StageStatus::Running) => ("运行中".to_string(), false, false),
            Some(StageStatus::Succeeded) => ("成功".to_string(), false, false),
            Some(StageStatus::Failed { reasons }) => {
                (format!("失败：{}", reasons.join("；")), false, false)
            }
            Some(StageStatus::Blocked { reasons }) => {
                (format!("阻塞：{}", reasons.join("；")), false, false)
            }
            Some(StageStatus::WaitingHuman { gate }) => {
                (format!("等待人工确认（{gate}）"), true, false)
            }
            None => match pending_stage(&stage.id) {
                Some(pending) => (pending.blocked_reason(), false, true),
                None => ("待运行".to_string(), false, true),
            },
        };
        let running = matches!(record.map(|item| &item.status), Some(StageStatus::Running));
        let touched = record.is_some_and(|item| !matches!(item.status, StageStatus::Pending));
        StageItem {
            id: stage.id.into(),
            name: stage.name.into(),
            status: status.into(),
            summary: stage.summary.into(),
            segment: "P 段".into(),
            duration: duration_text(record.and_then(StageRecord::duration_seconds)).into(),
            waiting,
            running,
            can_rerun: touched,
            can_inspect: false,
            placeholder,
        }
    }));
    rows
}

/// 范围运行下拉的可选段（C0-C6）：registry 是唯一真相源，段名不在 UI 里硬编码。
pub fn stage_ids() -> Vec<SharedString> {
    design_compile_registry()
        .into_iter()
        .map(|stage| SharedString::from(stage.id))
        .collect()
}

/// 阶段耗时文案：缺任一时刻（旧存档 / 运行中 / 从未开始）就如实说「未在案」，
/// 不拿 0 秒冒充一次瞬时完成的运行（R2）。
pub fn duration_text(seconds: Option<i64>) -> String {
    match seconds {
        None => "耗时 —（未在案）".to_string(),
        Some(total) => format!("耗时 {}", human_duration(total)),
    }
}

fn human_duration(total: i64) -> String {
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}h{minutes:02}m{seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m{seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

/// 运行中/停止按钮旁的状态提示。
///
/// 取消是**段边界粒度**的协作式取消：点「停止」后当前段的 AI 调用仍会跑完，
/// 文案必须说清这一点，否则用户会以为点完立刻断开（然后误判为卡死）。
pub fn pipeline_run_hint(running: bool, from: &str, to: &str) -> String {
    if running {
        format!(
            "正在运行 {from} → {to}：点「停止运行」会在「当前阶段结束后」停止（当前段的 AI 调用仍会跑完），已完成段的产物保留，下次可断点续跑。"
        )
    } else {
        "选择起止段后点「运行选定范围」；已成功的段会被跳过（断点续跑），要重做某段请用该段的「重跑」。".to_string()
    }
}

/// 强制重跑的二次确认警示语：把「连带作废什么」逐条摆到用户眼前再让他点确认。
pub fn rerun_warning(stage_id: &str, downstream: &[String]) -> String {
    let scope = if downstream.is_empty() {
        stage_id.to_string()
    } else {
        format!("{stage_id} → {}", downstream.join("、"))
    };
    format!(
        "危险操作：强制重跑 {stage_id}\n将连带作废这些段的产物与运行状态：{scope}\n这些段里已通过的人工门（C5 风格 / C6 签收）署名一并作废，需要重新确认（R3：旧署名不为新产物背书）。\n确认要继续吗？"
    )
}

/// 强制重跑回执：重置了哪些段、清空了哪些产物、作废了谁的署名——逐条列出。
pub fn reset_report_rows(report: &StageResetReport) -> Vec<TextRow> {
    // 表头用报告自带的一行摘要（与状态栏、CLI、运行日志同一份口径，不再各拼一遍）。
    let mut rows = vec![header_row(format!(
        "强制重跑回执 · 目标段 {} · {}",
        report.target,
        report.summary()
    ))];
    rows.push(info_row(
        format!("重置阶段 {} 个", report.reset_stages.len()),
        if report.reset_stages.is_empty() {
            "（无）".to_string()
        } else {
            report.reset_stages.join("、")
        },
        "bad",
    ));
    rows.push(info_row(
        format!("清空产物 {} 段", report.cleared_artifacts.len()),
        if report.cleared_artifacts.is_empty() {
            "（这些段此前没有落盘产物）".to_string()
        } else {
            report.cleared_artifacts.join("、")
        },
        "info",
    ));
    if report.revoked_confirmations.is_empty() {
        rows.push(info_row(
            "无人工门署名被作废",
            "重置范围内没有已通过的人工确认",
            "info",
        ));
    }
    for revoked in &report.revoked_confirmations {
        rows.push(info_row(
            format!("人工确认已作废 · {}", revoked.stage_id),
            format!(
                "原署名 {}（{}）不再有效，重跑到该段后需重新确认",
                revoked.actor, revoked.at
            ),
            "bad",
        ));
    }
    rows
}

/// 阶段产物详情标题（含缺失产物的显式提示）。
pub fn artifact_title(artifact: &StageArtifactView) -> String {
    if artifact.complete {
        format!(
            "阶段产物 · {} · 冻结版 v{}（contract.json + document.md 齐备）",
            artifact.stage_id, artifact.frozen_version
        )
    } else {
        format!(
            "阶段产物 · {} · 冻结版 v{}（缺 {}）",
            artifact.stage_id,
            artifact.frozen_version,
            artifact.missing.join("、")
        )
    }
}

/// 阶段产物逐文件行：路径 + sha256 + 字节数；缺文件如实标缺（不显示成空白详情）。
pub fn artifact_rows(artifact: &StageArtifactView) -> Vec<TextRow> {
    [&artifact.document, &artifact.contract]
        .into_iter()
        .map(|file| {
            if file.present {
                info_row(
                    format!(
                        "✔ {} · {} 字节 · sha256 {}",
                        file.file_name,
                        file.bytes,
                        short_sha(&file.sha256)
                    ),
                    file.path.clone(),
                    "ok",
                )
            } else {
                info_row(
                    format!("✘ {} 未生成", file.file_name),
                    format!("预期路径：{}", file.path),
                    "bad",
                )
            }
        })
        .collect()
}

/// `document.md` 预览正文；文件缺失时给出说明而不是空白（空白会被当成「文档是空的」）。
pub fn artifact_document(artifact: &StageArtifactView) -> String {
    match &artifact.document_text {
        Some(text) => text.clone(),
        None => format!(
            "（{} 未生成：该段尚未产出渲染文档，先运行或重跑 {}）",
            adm4_app::DOCUMENT_FILE,
            artifact.stage_id
        ),
    }
}

/// 预览提示条：截断时必须显式说「你看到的不是全文」。
pub fn artifact_hint(artifact: &StageArtifactView) -> String {
    let mut parts = Vec::new();
    if artifact.document_truncated {
        parts.push(format!(
            "预览已截断：只显示前 {} KiB，非全文；sha256 与字节数是整份文件的真值，核对请打开上方路径",
            artifact.preview_limit_bytes / 1024
        ));
    }
    if !artifact.complete {
        parts.push(format!("缺产物：{}", artifact.missing.join("、")));
    }
    parts.join(" · ")
}

/// 存档体检结果（`project_doctor`）：healthy 与否一眼可辨，problems 逐条可见。
pub fn doctor_rows(report: &ProjectDoctorReport) -> Vec<TextRow> {
    let mut rows = vec![header_row(format!("存档体检 · {}", report.archive_id))];
    if report.healthy {
        rows.push(info_row(
            "✔ 健康：未发现问题",
            "manifest 可读，且内容指纹与实际内容一致",
            "ok",
        ));
        return rows;
    }
    rows.push(info_row(
        format!("✘ 发现 {} 个问题", report.problems.len()),
        "逐条见下（体检只诊断不修复）",
        "bad",
    ));
    for (index, problem) in report.problems.iter().enumerate() {
        rows.push(info_row(
            format!("[问题 {}]", index + 1),
            problem.clone(),
            "bad",
        ));
    }
    rows
}

/// AI 体检结果：不可用就如实显示不可用，并原样呈现后端给的原因（R7：不许画成成功）。
pub fn ai_doctor_rows(report: &AiDoctorReport) -> Vec<TextRow> {
    let mut rows = vec![header_row("AI 体检（只诊断不修复，零网络请求）")];
    if report.available {
        rows.push(info_row(
            format!("✔ 可用 · Provider {}", report.provider_id),
            report.detail.clone(),
            "ok",
        ));
        rows.push(info_row(
            "提示",
            "体检只校验配置与密钥可解析性，不代表远端服务一定连得上——请跑「实调用检查」",
            "info",
        ));
    } else {
        rows.push(info_row("✘ 不可用", report.detail.clone(), "bad"));
        rows.push(info_row(
            "影响范围",
            "AI 访谈 / 冻结门红队 / C1-C5 流水线段会直接 blocked（无模板兜底，R7）",
            "bad",
        ));
    }
    rows
}

/// AI 实调用检查结果：成功给可核对的事实（模型/字符数/耗时），失败原样呈现原因。
///
/// 与 [`ai_doctor_rows`] 分开呈现，因为两者结论可以相反且都对：配置齐备（doctor 可用）
/// 而 base_url 写错（invoke-check 失败）正是最常见的情形。把它们混在一张表里，
/// 用户会以为其中一个是过期数据。
pub fn ai_invoke_rows(report: &AiInvokeCheckReport) -> Vec<TextRow> {
    let mut rows = vec![header_row("AI 实调用检查（真发一次最小请求，走网络）")];
    if report.succeeded {
        rows.push(info_row(
            format!(
                "✔ 打通 · Provider {} · 模型 {}",
                report.provider_id, report.model
            ),
            format!(
                "应答 {} 字符，耗时 {} ms（{}）",
                report.response_chars, report.elapsed_ms, report.at
            ),
            "ok",
        ));
        rows.push(info_row("应答摘要", report.detail.clone(), "info"));
    } else {
        rows.push(info_row(
            if report.provider_id.is_empty() {
                "✘ 未发出请求".to_string()
            } else {
                format!("✘ 调用失败 · Provider {}", report.provider_id)
            },
            report.detail.clone(),
            "bad",
        ));
        rows.push(info_row(
            "如实说明",
            format!(
                "失败原因原样来自后端，不重试、不降级、不改写（R7）；耗时 {} ms",
                report.elapsed_ms
            ),
            "bad",
        ));
    }
    rows
}

/// 实调用检查的一行状态栏文案。
pub fn ai_invoke_status_text(report: &AiInvokeCheckReport) -> String {
    if report.succeeded {
        format!("AI 实调用检查通过：{}", report.summary())
    } else {
        format!("AI 实调用检查失败：{}", report.summary())
    }
}

/// 已登记 named secret 的名字一行（**只列名字，不列值**）。
pub fn secret_names_text(names: &[String]) -> String {
    if names.is_empty() {
        return "config/secrets.json 尚无 named secret（配置里用 named:<名字> 引用它们）"
            .to_string();
    }
    format!(
        "已登记 named secret {} 条：{}（只列名字，值不展示）",
        names.len(),
        names
            .iter()
            .map(|name| format!("named:{name}"))
            .collect::<Vec<_>>()
            .join("、")
    )
}

/// 底栏与面板顶部的一行 AI 状态。
pub fn ai_status_text(report: &AiDoctorReport) -> String {
    if report.available {
        format!("AI：可用（{}）", report.provider_id)
    } else {
        format!("AI：不可用 — {}", report.detail)
    }
}

/// AI 配置面板的当前配置摘要（读 `config/app.json`）。
pub fn provider_summary(config: Option<&HttpProviderConfig>) -> String {
    match config {
        None => "config/app.json 尚未配置 ai_provider：AI 访谈 / 红队 / C1-C5 全部会被 blocked"
            .to_string(),
        Some(config) => format!(
            "当前配置：{} · {} · 模型 {} · 密钥引用 {} · 超时 {}s",
            config.provider_id,
            config.base_url,
            config.model,
            config.api_key_ref,
            config.timeout_secs
        ),
    }
}

/// AI 配置表单的初值（`None` = 未配置，全部留空由用户填或套用 preset）。
pub struct ProviderForm {
    pub provider_id: String,
    pub base_url: String,
    pub model: String,
    pub api_key_ref: String,
    pub timeout_secs: String,
}

pub fn provider_form(config: Option<&HttpProviderConfig>) -> ProviderForm {
    match config {
        None => ProviderForm {
            provider_id: String::new(),
            base_url: String::new(),
            model: String::new(),
            api_key_ref: String::new(),
            timeout_secs: String::new(),
        },
        Some(config) => ProviderForm {
            provider_id: config.provider_id.clone(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            api_key_ref: config.api_key_ref.clone(),
            timeout_secs: config.timeout_secs.to_string(),
        },
    }
}

/// 表单必填项缺失检查（只看「填没填」，值是否有效由后端 `build_provider` 判定）。
///
/// 与既有的「请先填写导出路径」同类：这是输入完整性，不是业务规则——
/// 密钥引用的格式、base_url 能否连通，一概不在 UI 里判断。
pub fn missing_provider_fields(
    provider_id: &str,
    base_url: &str,
    model: &str,
    api_key_ref: &str,
    timeout_secs: &str,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    for (value, label) in [
        (provider_id, "Provider id"),
        (base_url, "Base URL"),
        (model, "模型名"),
        (api_key_ref, "密钥引用（env:NAME 或 named:NAME）"),
        (timeout_secs, "超时秒数"),
    ] {
        if value.trim().is_empty() {
            missing.push(label);
        }
    }
    missing
}

// ---------------------------------------------------------------------------
// G2 设计阶段风格锚点门：右栏「风格」页签的装配
//
// 这里只做「后端只读投影 → 行模型/文案」的确定性转换。能不能生成、能不能确认、
// 未确认要不要阻断下游，一律由服务层给出（`StyleGateStatus.readiness` /
// `AiDoctorReport.available`），本层照实呈现（D14）。
// ---------------------------------------------------------------------------

/// 图像通道体检的状态条文案。
pub fn image_status_text(report: &AiDoctorReport) -> String {
    if report.available {
        format!("图像通道：可用（{}）", report.provider_id)
    } else {
        format!("图像通道：不可用 — {}", report.detail)
    }
}

/// 图像通道配置表单的初值（`None` = 未配置，留空由用户填或套用 preset）。
pub struct ImageProviderForm {
    pub provider_id: String,
    pub base_url: String,
    pub model: String,
    pub api_key_ref: String,
    pub size: String,
    pub timeout_secs: String,
}

pub fn image_provider_form(config: Option<&HttpImageProviderConfig>) -> ImageProviderForm {
    match config {
        None => ImageProviderForm {
            provider_id: String::new(),
            base_url: String::new(),
            model: String::new(),
            api_key_ref: String::new(),
            size: String::new(),
            timeout_secs: String::new(),
        },
        Some(config) => ImageProviderForm {
            provider_id: config.provider_id.clone(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            api_key_ref: config.api_key_ref.clone(),
            size: config.size.clone(),
            timeout_secs: config.timeout_secs.to_string(),
        },
    }
}

/// 图像通道表单的必填项缺失检查（只看「填没填」；尺寸格式与密钥可解析性由后端判）。
pub fn missing_image_provider_fields(
    provider_id: &str,
    base_url: &str,
    model: &str,
    api_key_ref: &str,
    size: &str,
    timeout_secs: &str,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    for (value, label) in [
        (provider_id, "Provider id"),
        (base_url, "图像 Base URL"),
        (model, "图像模型名"),
        (api_key_ref, "密钥引用（env:NAME 或 named:NAME）"),
        (size, "生成尺寸（如 1024x1024）"),
        (timeout_secs, "超时秒数"),
    ] {
        if value.trim().is_empty() {
            missing.push(label);
        }
    }
    missing
}

/// 适配风险 → 卡片配色键（slint 侧按它选色）。
fn style_fit_kind(risk: StyleFitRisk) -> &'static str {
    match risk {
        StyleFitRisk::Ok => "ok",
        StyleFitRisk::Caution => "caution",
        StyleFitRisk::Unknown => "unknown",
    }
}

/// 风格网格的卡片行模型。
///
/// `image_of` 是图片加载器：给相对路径，换一张 `slint::Image` 或一句失败原因。
/// 做成回调而不是在这里直接读盘，为的是两件事：① 本层要能单测，而单测里没有存档也没有图；
/// ② 「路径怎么解析成绝对路径」是服务层的判定（`AppServices::style_image_path` 会拦越界
/// 路径与不存在的文件），装配层不该复制一份。
///
/// 三态如实区分（这是这段装配唯一有分支的地方，也是最容易糊掉的地方）：
/// 记录里没有图 → 画「无预览图」并显示最近失败原因；有图且加载成功 → 画图；
/// 有图但加载失败 → 也画「无预览图」，但失败原因换成加载错误（不是生成错误）。
pub fn style_cards(
    status: &StyleGateStatus,
    focused: Option<&str>,
    image_of: &dyn Fn(&str) -> Result<Image, String>,
) -> Vec<StyleCard> {
    status
        .directions
        .iter()
        .map(|row| {
            let (image, has_image, failure) = if row.image_path.is_empty() {
                (Image::default(), false, row.last_failure.clone())
            } else {
                match image_of(&row.image_path) {
                    Ok(image) => (image, true, row.last_failure.clone()),
                    Err(error) => (Image::default(), false, error),
                }
            };
            StyleCard {
                id: row.style_id.clone().into(),
                title: row.title.clone().into(),
                description: row.description.clone().into(),
                prompt_summary: row.prompt_summary.clone().into(),
                prompt_origin: if row.prompt_overridden {
                    "用户改词".into()
                } else {
                    "派生自真源".into()
                },
                palette: format!("配色 {}", row.palette.join(" ")).into(),
                fit: format!("适配 {} · {}", row.fit_risk.label_zh(), row.fit_reason).into(),
                fit_kind: style_fit_kind(row.fit_risk).into(),
                badge: if row.is_selected {
                    "★已确认 ".into()
                } else if row.recommended {
                    "◎推荐 ".into()
                } else {
                    SharedString::default()
                },
                failure: failure.into(),
                image,
                has_image,
                selected: row.is_selected,
                active: focused == Some(row.style_id.as_str()),
            }
        })
        .collect()
}

/// 「风格」页签顶部的状态摘要。
pub fn style_summary(status: &StyleGateStatus) -> String {
    if !status.session_present {
        return "本项目尚未生成风格方向。点「生成方向」派生 3-5 个候选——提示词锚定已确认的画像决策点（品类/平台/体验/美术风格定位），一条都没确认时会如实报错（R4）。".to_string();
    }
    let mut text = format!(
        "{} · 品类包 {} · {} 个方向 · {} 轮生成记录",
        status.project_name,
        status.genre_pack,
        status.directions.len(),
        status.round_count
    );
    if !status.latest_round_id.is_empty() {
        text.push_str(&format!("（最近 {}）", status.latest_round_id));
    }
    text.push_str(&format!(
        " · 真源 revision 当前 {} / 工作态 {}",
        status.current_revision, status.session_revision
    ));
    if status.session_stale {
        text.push_str("｜设计已变，建议「推翻重生成」重新派生提示词（提示不阻断）");
    }
    text
}

/// 尚未打开项目时的摘要（工作台没有项目就没有风格门可谈）。
pub fn style_summary_without_project() -> String {
    "尚未打开项目：请先在「存档管理」新建或载入项目，再来定风格。".to_string()
}

/// 就绪结论（就绪 = 下游 P2 资产生产可开跑）。
pub fn style_readiness_text(status: &StyleGateStatus) -> String {
    let mut text = if status.readiness.ready {
        format!("[就绪] {}", status.readiness.detail)
    } else {
        format!(
            "[阻断] {}｜下游 P2 资产生产被阻断（风格锚点集是它声明消费的外部输入）",
            status.readiness.detail
        )
    };
    if status.readiness.ready && status.anchor_stale {
        text.push_str("｜提醒：锚点锚的设计版本落后于当前设计，可「重新选择」另立新版");
    }
    if !status.anchor_versions.is_empty() {
        text.push_str(&format!(
            "｜锚点历史 {}",
            status
                .anchor_versions
                .iter()
                .map(|version| format!("v{version}"))
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    text
}

/// 生成入口不可用时的指路文案（缺什么说什么，不只说「不可用」）。
pub fn style_gate_hint(image_doctor: &AiDoctorReport, project_open: bool) -> String {
    if !project_open {
        return String::new();
    }
    if image_doctor.available {
        return String::new();
    }
    format!(
        "生成入口已停用：{}。请在顶栏「AI配置/诊断」面板的「图像通道」一段填 provider id / base_url / 模型 / 密钥引用 / 尺寸后保存。风格门必须看真图，没有图像通道就是 blocked——绝不用占位图冒充（R7）。",
        image_doctor.detail
    )
}

/// 改词编辑框上方的来源说明。
pub fn style_prompt_origin(direction: Option<&StyleDirection>) -> String {
    match direction {
        None => "选中一个方向后可在此改词重生成（清空后重生成 = 回到派生提示词）".to_string(),
        Some(direction) if direction.prompt_override.trim().is_empty() => {
            format!(
                "当前提示词派生自真源（{} 条锚点）——改写后重生成即成为该方向的最终提示词",
                direction.prompt_anchors.len()
            )
        }
        Some(_) => {
            "当前提示词是你的改词（点「回到派生提示词」可还原为锚定真源的那一版）".to_string()
        }
    }
}

/// 已锁定锚点的摘要行（下游要照它消费，所以关键字段都得看得见）。
pub fn style_lock_rows(
    anchor_set: &StyleAnchorSet,
    contract: &StyleApplicationContract,
) -> Vec<TextRow> {
    let mut rows = vec![header_row(format!(
        "已锁定风格锚点 v{}",
        anchor_set.anchor_version
    ))];
    rows.push(info_row(
        format!(
            "方向 {}（{}）",
            anchor_set.selected_title, anchor_set.selected_style_id
        ),
        format!(
            "预设 {} · 配色 {} · 真源 revision {}",
            anchor_set.preset_key,
            anchor_set.palette.join(" "),
            anchor_set.source_revision
        ),
        "ok",
    ));
    rows.push(info_row(
        format!(
            "署名 {} 于 {}",
            anchor_set.confirmation.actor, anchor_set.confirmation.at
        ),
        anchor_set.confirmation.notes.clone(),
        "info",
    ));
    rows.push(info_row(
        format!(
            "最终提示词（{}）",
            if anchor_set.prompt_overridden {
                "用户改词"
            } else {
                "派生自真源"
            }
        ),
        anchor_set.final_prompt.clone(),
        "info",
    ));
    for anchor in &anchor_set.anchors {
        rows.push(info_row(
            format!("锚图 {}", anchor.role),
            format!(
                "{} · {}",
                anchor.image_path,
                short_sha(&anchor.image_sha256)
            ),
            "info",
        ));
    }
    rows.push(info_row(
        "风格应用契约".to_string(),
        format!(
            "锚点哈希 {} · 分用途约束 {} 条（{}）",
            short_sha(&contract.source_anchor_hash),
            contract.style_constraints.len(),
            contract
                .style_constraints
                .iter()
                .map(|constraint| constraint.usage.label_zh())
                .collect::<Vec<_>>()
                .join("/")
        ),
        "ok",
    ));
    rows
}

/// 大图覆盖层的标题与提示词全文。
pub fn style_viewer_texts(row: &StyleDirectionStatus, prompt: &str) -> (String, String) {
    let title = format!(
        "{}（{}）· 适配 {}",
        row.title,
        row.style_id,
        row.fit_risk.label_zh()
    );
    let mut detail = format!(
        "方向说明：{}\n提示词（{}）：{}",
        row.description,
        if row.prompt_overridden {
            "用户改词"
        } else {
            "派生自真源"
        },
        prompt
    );
    if !row.image_sha256.is_empty() {
        detail.push_str(&format!("\n锚图指纹：{}", row.image_sha256));
    }
    if !row.last_failure.is_empty() {
        detail.push_str(&format!("\n最近失败：{}", row.last_failure));
    }
    detail.push_str("\n（点遮罩或「关闭」退出；不满意就回右栏改词重生成，次数不限）");
    (title, detail)
}

/// 另存模板回执：导出条数与**跳过的未确认点数**同样显眼（后者最容易被误当成整卷定稿）。
pub fn template_export_text(report: &TemplateExportReport) -> String {
    format!(
        "已另存模板 {}/{}（{}）\n{}\n跳过未确认决策点 {} 个 · 失效选项 {} 条\n注意：另存出来的模板落在「已审核」状态，还需在本面板走 S5「认证入库」才能预填到别的项目。",
        report.genre_pack,
        report.template_id,
        report.game_name,
        report.summary(),
        report.skipped_unconfirmed,
        report.skipped_unknown.len()
    )
}

/// 工作台重置的二次确认警示语（破坏性操作，必须先说清清空什么、保留什么）。
pub fn reset_workbench_warning() -> String {
    "危险操作：重置设计工作台\n将清空当前项目的全部决策点选择（含参数值、理由、多选与主选标记）、不适用豁免、节点设计说明与风险说明。\n已冻结版本（frozen/v{N}）与流水线产物（pipeline/v{N}）不受影响，项目本身也不会被删除。\n操作人与理由必填并进审计日志（R3）。确认要继续吗？".to_string()
}

/// 工作台重置回执：清空计数逐项列出 + 明示未受影响的范围。
pub fn reset_workbench_text(report: &WorkbenchResetReport) -> String {
    let head = if report.is_noop() {
        "重置完成：该项目本来就没有任何创作内容可清空"
    } else {
        "重置完成"
    };
    format!(
        "{head}（署名 {} · {}）\n{}\n已冻结版本与流水线产物不受影响。",
        report.actor,
        report.at,
        report.summary()
    )
}

/// 流水线视图的映射注记：二版 Step00-14 与四版 C 段/P 段的对应关系。
pub fn pipeline_note() -> String {
    "二版 Step00-14 → 四版分段映射：\n\
     · Step00-06（创意收集/玩法框架/设计冻结/程序需求/美术需求/程序评审/美术评审）→ 设计工作台冻结门 + C0-C4\n\
     · Step07 美术风格人工确认 → C5 风格段人工门（本视图「人工确认」按钮）\n\
     · Step08-10（程序计划/美术计划/资源对齐）→ C6 开发计划与签收\n\
     · Step11-14（程序执行/美术生产/场景组装/集成验证）→ P0-P5（Phase 2，本视图占位可见，不断头）\n\
     本视图可操作：范围运行（选起止段）/ 停止运行（段边界生效）/ 单段强制重跑（二次确认后连带作废下游）/ 阶段详情（状态·耗时·产物入口）。"
        .to_string()
}

/// 运行日志过滤：类别或消息命中即保留（大小写不敏感）；最新在前。
pub fn log_rows(entries: Vec<RunLogEntry>, filter: &str) -> Vec<LogItem> {
    let needle = filter.trim().to_lowercase();
    entries
        .into_iter()
        .rev()
        .filter(|entry| {
            needle.is_empty()
                || entry.category.to_lowercase().contains(&needle)
                || entry.message.to_lowercase().contains(&needle)
        })
        .map(|entry| LogItem {
            at: entry.at.into(),
            category: entry.category.into(),
            message: entry.message.into(),
        })
        .collect()
}

/// 运行日志导出文本（markdown 列表，最新在前）。
pub fn log_markdown(rows: &[LogItem]) -> String {
    let mut text = String::from("# 运行日志导出\n\n");
    for row in rows {
        text.push_str(&format!(
            "- `{}` **[{}]** {}\n",
            row.at, row.category, row.message
        ));
    }
    text
}

/// 工作台快照导出（markdown）：档案 + 画像 + 领域进度 + 决策点清单 + 缺失/风险/校验。
///
/// 导出内容全部来自 `workbench_overview` / `project_profile` / `decision_points`
/// 三个只读聚合，不二次计算任何进度——快照与界面看到的是同一份事实。
pub fn workbench_markdown(
    overview: &WorkbenchOverview,
    profile: &ProjectProfile,
    points: &[DecisionPointView],
) -> String {
    let summary = &overview.summary;
    let mut text = format!("# 设计工作台快照 · {}\n\n", summary.project_name);
    text.push_str(&format!(
        "- 品类包：{} @ {}\n- 深度档：{:?}\n- 修订：{}\n- 已冻结版本数：{}\n- 完成度：{}/{}（{}%）\n- N/A 豁免：{} 项\n\n",
        summary.genre_pack,
        summary.pack_version,
        summary.depth_target,
        summary.revision,
        summary.frozen_versions,
        summary.done,
        summary.total,
        summary.percent,
        summary.counts.not_applicable
    ));

    text.push_str("## 项目画像（L0/L1 已确认决策点聚合）\n\n");
    if profile.fields.is_empty() {
        text.push_str("（尚无已确认的 L0/L1 决策点）\n\n");
    } else {
        for field in &profile.fields {
            let selected = field
                .selected
                .iter()
                .map(|option| {
                    if option.is_primary {
                        format!("{}（主选）", option.label)
                    } else {
                        option.label.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("、");
            text.push_str(&format!(
                "- **{}** → {}（{}，{}）\n",
                field.label,
                selected,
                field.level.label(),
                field.decision_id
            ));
        }
        text.push('\n');
    }

    text.push_str(
        "## 领域进度\n\n| 领域 | 已确认/适用 | 百分比 | 节点 | N/A |\n|---|---|---|---|---|\n",
    );
    if summary.domains.is_empty() {
        text.push_str("| （无领域进度） | 0/0 | - | 0 | 0 |\n");
    }
    for domain in &summary.domains {
        text.push_str(&format!(
            "| {} | {}/{} | {}% | {} | {} |\n",
            domain.name,
            domain.counts.confirmed,
            domain.counts.applicable,
            domain.percent,
            domain.node_count,
            domain.counts.not_applicable
        ));
    }
    text.push('\n');

    text.push_str("## 决策点清单\n\n");
    for domain in &summary.domains {
        text.push_str(&format!("### {}\n\n", domain.name));
        for point in points
            .iter()
            .filter(|point| point.domain_id == domain.domain_id)
        {
            let (badge, _) = badge_of(point);
            text.push_str(&format!(
                "- [{}] `{}` {}（{}）\n",
                badge,
                point.decision_id,
                point.question,
                point.level.label()
            ));
            let selected: Vec<String> = point
                .options
                .iter()
                .filter(|option| option.selected)
                .map(|option| {
                    if option.is_primary {
                        format!("{}（主选）", option.label)
                    } else {
                        option.label.clone()
                    }
                })
                .collect();
            if !selected.is_empty() {
                text.push_str(&format!("  - 已选：{}\n", selected.join("、")));
            }
            if point.exemption.is_some() {
                text.push_str(&format!("  - 豁免：{}\n", exemption_text(point)));
            }
        }
        text.push('\n');
    }

    text.push_str("## 缺失项\n\n");
    if overview.missing.is_empty() {
        text.push_str("（无）\n\n");
    }
    for group in &overview.missing {
        text.push_str(&format!("### {}\n\n", group.domain_name));
        for item in &group.items {
            text.push_str(&format!(
                "- `{}` {}：{}\n",
                item.decision_id,
                item.question,
                item.reasons.join("；")
            ));
        }
        text.push('\n');
    }

    text.push_str("## 风险\n\n");
    if overview.risk.node_risks.is_empty() {
        text.push_str("- 节点风险说明：（无）\n");
    }
    for note in &overview.risk.node_risks {
        text.push_str(&format!("- {}：{}\n", note.node_name, note.note));
    }
    match &overview.risk.red_team {
        None => text.push_str("- AI 红队：尚未运行\n\n"),
        Some(red_team) => {
            text.push_str(&format!(
                "- AI 红队：评审人 {}，修订 {}{}，发现 {} 项\n\n",
                red_team.reviewer,
                red_team.reviewed_revision,
                if red_team.stale {
                    "（已过期）"
                } else {
                    ""
                },
                red_team.findings.len()
            ));
        }
    }

    text.push_str("## 校验\n\n");
    text.push_str(&format!(
        "- 外键违规：{} 条\n- 冻结门预检：{}\n\n",
        overview.validation.row_reference_violations.len(),
        if overview.validation.all_gates_passed {
            "全绿"
        } else {
            "未全绿"
        }
    ));
    for gate in &overview.validation.gates {
        text.push_str(&format!(
            "### {} · {}\n\n",
            gate.gate,
            if gate.passed { "通过" } else { "未通过" }
        ));
        for finding in &gate.findings {
            text.push_str(&format!("- [{}] {}\n", finding.code, finding.message));
        }
        text.push('\n');
    }
    text
}

/// 表格行模型（外层行 → 内层格）的构造：Slint 侧是 `[[string]]`。
pub fn table_model(buffer: &[Vec<String>]) -> Vec<slint::ModelRc<SharedString>> {
    buffer
        .iter()
        .map(|row| {
            slint::ModelRc::new(VecModel::from(
                row.iter().map(SharedString::from).collect::<Vec<_>>(),
            ))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// T12 三视图行模型装配（SDK 审批队列 / 补充开发变更 / 文档集交付）
// ---------------------------------------------------------------------------

/// SDK 审批队列行模型。
pub fn sdk_rows(snapshot: &SdkSnapshot) -> Vec<SdkRow> {
    snapshot
        .records
        .iter()
        .map(|record| {
            let (status_kind, signed) = match record.status {
                SdkReviewStatus::Pending => ("warn", "待审核".to_string()),
                SdkReviewStatus::Approved => (
                    "good",
                    format!(
                        "批准 · 评审 {} · {} · {}",
                        record.reviewer, record.review_note, record.reviewed_at
                    ),
                ),
                SdkReviewStatus::Rejected => (
                    "bad",
                    format!(
                        "拒绝 · 评审 {} · {} · {}",
                        record.reviewer, record.review_note, record.reviewed_at
                    ),
                ),
            };
            SdkRow {
                id: record.id.clone().into(),
                name: record.sdk_name.clone().into(),
                url: record.url.clone().into(),
                category: record.category.clone().into(),
                purpose: record.purpose.clone().into(),
                status: record.status.label_zh().into(),
                status_kind: status_kind.into(),
                signed: signed.into(),
                pending: matches!(record.status, SdkReviewStatus::Pending),
            }
        })
        .collect()
}

/// SDK 计数条文本（顶部三态计数）。
pub fn sdk_counts(snapshot: &SdkSnapshot) -> String {
    format!(
        "待审 {} · 已批准 {} · 已拒绝 {}",
        snapshot.pending_count, snapshot.approved_count, snapshot.rejected_count
    )
}

/// 补充开发变更请求行模型。
///
/// 按钮可用性由后端状态机推出（GUI 不判规则）：`Drafted`/`ImpactAnalyzed` 可影响分析，
/// `ImpactAnalyzed`/`Scheduled` 可推进；终态两个按钮都不出。
pub fn change_rows(requests: &[ChangeRequest]) -> Vec<ChangeRow> {
    requests
        .iter()
        .map(|req| {
            let status_kind = match req.status {
                ChangeStatus::Applied => "good",
                ChangeStatus::Rejected => "bad",
                _ => "",
            };
            let can_impact = matches!(
                req.status,
                ChangeStatus::Drafted | ChangeStatus::ImpactAnalyzed
            );
            let can_advance = matches!(
                req.status,
                ChangeStatus::ImpactAnalyzed | ChangeStatus::Scheduled
            );
            let next = req.status.next();
            let signed = if req.last_actor.is_empty() {
                "尚未推进".to_string()
            } else {
                format!(
                    "推进署名 {} · {} · {}",
                    req.last_actor, req.last_note, req.updated_at
                )
            };
            let segments = if req.affected_segments.is_empty() {
                "（未分析）".to_string()
            } else {
                req.affected_segments.join(",")
            };
            ChangeRow {
                id: req.id.clone().into(),
                title: req.title.clone().into(),
                requester: req.requested_by.clone().into(),
                status: req.status.label_zh().into(),
                status_kind: status_kind.into(),
                segments: segments.into(),
                signed: signed.into(),
                next_token: next.map(|status| status.as_token()).unwrap_or("").into(),
                next_label: next
                    .map(|status| format!("推进→{}", status.label_zh()))
                    .unwrap_or_default()
                    .into(),
                can_impact,
                can_advance,
            }
        })
        .collect()
}

/// 变更请求汇总条文本。
pub fn change_summary(requests: &[ChangeRequest]) -> String {
    if requests.is_empty() {
        return "尚无变更请求".to_string();
    }
    let applied = requests
        .iter()
        .filter(|req| matches!(req.status, ChangeStatus::Applied))
        .count();
    let rejected = requests
        .iter()
        .filter(|req| matches!(req.status, ChangeStatus::Rejected))
        .count();
    let open = requests.len() - applied - rejected;
    format!(
        "共 {} 项 · 进行中 {} · 已应用 {} · 已拒绝 {}",
        requests.len(),
        open,
        applied,
        rejected
    )
}

/// 文档集交付逐段行模型（C0-C6）。
pub fn deliverable_rows(manifest: &DeliverableManifest) -> Vec<DeliverRow> {
    manifest
        .segments
        .iter()
        .map(|segment| {
            let detail = if segment.present {
                format!(
                    "doc {} · {}B · contract {}",
                    short_sha(&segment.document_sha256),
                    segment.document_bytes,
                    short_sha(&segment.contract_sha256)
                )
            } else {
                "未生成（先运行流水线该段）".to_string()
            };
            DeliverRow {
                stage: segment.stage_id.clone().into(),
                present: segment.present,
                detail: detail.into(),
            }
        })
        .collect()
}

/// 交付清单汇总条文本。
pub fn deliverable_summary(manifest: &DeliverableManifest) -> String {
    if manifest.complete {
        format!(
            "v{} · 完整：7/7 段齐备 · 生成于 {}",
            manifest.frozen_version, manifest.generated_at
        )
    } else {
        format!(
            "v{} · 缺段：{} · 生成于 {}",
            manifest.frozen_version,
            manifest.missing_segments.join(","),
            manifest.generated_at
        )
    }
}

/// sha256 短展示（前 12 位；空串照原样）。
fn short_sha(sha: &str) -> String {
    if sha.len() > 12 {
        sha[..12].to_string()
    } else {
        sha.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_app::{
        DecisionOptionView, ExemptionView, GateSummary, MissingByDomain, MissingEntry,
        NodeRiskNote, ProfileField, ProfileOption, WorkbenchRisk, WorkbenchSummary,
        WorkbenchValidation,
    };
    use adm4_authoring::GateFinding;
    use adm4_decision::{
        DesignLevel, DomainProgress, NodeProgress, PointRequirement, ProgressCounts,
    };

    fn counts(confirmed: usize, applicable: usize, na: usize, total: usize) -> ProgressCounts {
        ProgressCounts {
            confirmed,
            applicable,
            not_applicable: na,
            optional_skipped: 0,
            total_points: total,
        }
    }

    fn domain(id: &str, name: &str, order: u32) -> DesignDomain {
        DesignDomain {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            order,
        }
    }

    fn point(id: &str, applicability: &str, confirmed: bool) -> DecisionPointView {
        DecisionPointView {
            decision_id: id.into(),
            level: DesignLevel::L4,
            domain_id: "gameplay".into(),
            node_id: "input".into(),
            question: "问题？".into(),
            design_question: None,
            mda_layer: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            requirement_label: PointRequirement::Unlocked.label().into(),
            optional: false,
            applicability: applicability.into(),
            confirmed,
            options: vec![DecisionOptionView {
                option_id: "a".into(),
                label: "甲".into(),
                summary: "摘要".into(),
                selected: confirmed,
                is_primary: false,
            }],
            exemption: None,
        }
    }

    fn overview() -> WorkbenchOverview {
        WorkbenchOverview {
            summary: WorkbenchSummary {
                project_name: "验证项目".into(),
                genre_pack: "lane_defense".into(),
                pack_version: "0.1.0".into(),
                depth_target: DesignLevel::L5,
                revision: 7,
                frozen_versions: 0,
                done: 1,
                total: 3,
                percent: 33,
                optional_skipped: 0,
                domains: vec![DomainProgress {
                    domain_id: "gameplay".into(),
                    name: "玩法系统设计".into(),
                    description: String::new(),
                    order: 3,
                    node_count: 1,
                    counts: counts(1, 3, 1, 5),
                    percent: 33,
                }],
                nodes: vec![NodeProgress {
                    node_id: "input".into(),
                    domain_id: "gameplay".into(),
                    name: "输入与控制".into(),
                    description: String::new(),
                    role_class: "system_concrete".into(),
                    counts: counts(1, 3, 1, 5),
                    percent: 33,
                    decision_ids: vec!["ld.input".into()],
                }],
                counts: counts(1, 3, 1, 5),
            },
            missing: vec![MissingByDomain {
                domain_id: "gameplay".into(),
                domain_name: "玩法系统设计".into(),
                items: vec![MissingEntry {
                    decision_id: "ld.wave".into(),
                    node_id: "input".into(),
                    level: DesignLevel::L4,
                    question: "波次如何编排？".into(),
                    reasons: vec!["未选择".into()],
                }],
            }],
            risk: WorkbenchRisk {
                node_risks: vec![NodeRiskNote {
                    node_id: "input".into(),
                    node_name: "输入与控制".into(),
                    domain_id: "gameplay".into(),
                    note: "手感未经试玩验证".into(),
                }],
                red_team: None,
            },
            validation: WorkbenchValidation {
                row_reference_violations: Vec::new(),
                gates: vec![GateSummary {
                    gate: "gate1_completeness".into(),
                    passed: true,
                    finding_count: 1,
                    findings: vec![GateFinding {
                        code: "not_applicable_exemption".into(),
                        message: "ld.boss 已标记不适用[out_of_scope]，署名 主策划".into(),
                    }],
                }],
                all_gates_passed: false,
            },
        }
    }

    /// 左栏必须显示全部领域（含 0 点域）；「未分域」无点时不显示。
    #[test]
    fn domain_cards_include_empty_domains_and_hide_empty_reserved() {
        let domains = vec![
            domain("gameplay", "玩法系统设计", 3),
            domain("economy", "经济商业化设计", 5),
            domain(UNASSIGNED_DOMAIN_ID, "未分域", u32::MAX),
        ];
        let progress = OrganizationProgress {
            domains: vec![DomainProgress {
                domain_id: "gameplay".into(),
                name: "玩法系统设计".into(),
                description: String::new(),
                order: 3,
                node_count: 2,
                counts: counts(2, 4, 1, 6),
                percent: 50,
            }],
            nodes: Vec::new(),
            total: counts(2, 4, 1, 6),
        };
        let cards = domain_cards(&domains, &progress, Some("gameplay"));
        assert_eq!(cards.len(), 2, "0 点域要显示，未分域无点不显示");
        assert!(cards[0].active);
        assert!(cards[0].progress.contains("2/4"));
        assert!(cards[0].progress.contains("N/A 1"));
        assert!(cards[1].empty, "无点领域标记为 empty");
        assert!(cards[1].progress.contains("本域暂无决策点"));
        assert_eq!(cards[1].percent, 0);
    }

    /// 未迁移形态：所有点归「未分域」，此时保留领域必须显示。
    #[test]
    fn reserved_domain_is_visible_when_it_has_points() {
        let domains = vec![domain(UNASSIGNED_DOMAIN_ID, "未分域", u32::MAX)];
        let progress = OrganizationProgress {
            domains: vec![DomainProgress {
                domain_id: UNASSIGNED_DOMAIN_ID.into(),
                name: "未分域".into(),
                description: String::new(),
                order: u32::MAX,
                node_count: 1,
                counts: counts(2, 5, 0, 5),
                percent: 40,
            }],
            nodes: vec![NodeProgress {
                node_id: UNASSIGNED_NODE_ID.into(),
                domain_id: UNASSIGNED_DOMAIN_ID.into(),
                name: "未分节点".into(),
                description: String::new(),
                role_class: "reserved".into(),
                counts: counts(2, 5, 0, 5),
                percent: 40,
                decision_ids: vec!["u.platform".into()],
            }],
            total: counts(2, 5, 0, 5),
        };
        let cards = domain_cards(&domains, &progress, None);
        assert_eq!(cards.len(), 1);
        assert!(cards[0].progress.contains("2/5"));
        let nodes = node_cards(&progress, Some(UNASSIGNED_DOMAIN_ID), None);
        assert_eq!(nodes.len(), 1);
        assert!(
            nodes[0].name.contains("清单尚未挂载节点"),
            "{}",
            nodes[0].name
        );
    }

    #[test]
    fn badges_cover_every_applicability_state() {
        assert_eq!(badge_of(&point("a", "active", true)), ("已确认", "done"));
        assert_eq!(badge_of(&point("a", "active", false)), ("未选", "bad"));
        assert_eq!(badge_of(&point("a", "beyond_depth", false)).0, "超深度");
        assert_eq!(badge_of(&point("a", "inactive", false)).0, "未激活");
        assert_eq!(badge_of(&point("a", "not_applicable", false)).0, "N/A 豁免");

        // 已选未确认 → 待确认（不能与「未选」混为一谈）。
        let mut selected = point("a", "active", false);
        selected.options[0].selected = true;
        assert_eq!(badge_of(&selected), ("待确认", "warn"));
    }

    /// 多选点：缺主选要能被 UI 可见拦截（T9 已知缺口）。
    #[test]
    fn primary_missing_only_when_multi_selected_without_primary() {
        let mut multi = point("a", "active", true);
        multi.selection_mode = SelectionMode::Multi {
            allow_primary: true,
        };
        multi.options[0].selected = true;
        assert!(primary_missing(&multi));
        multi.options[0].is_primary = true;
        assert!(!primary_missing(&multi));

        // 单选点没有主选概念，永不拦截。
        let single = point("a", "active", true);
        assert!(!primary_missing(&single));
        assert!(decision_meta(&single).contains("单选"));
        assert!(decision_meta(&multi).contains("多选（需设主选）"));
    }

    #[test]
    fn check_rows_filter_by_node_then_domain() {
        let mut other = point("b", "active", false);
        other.node_id = "waves".into();
        other.domain_id = "content".into();
        let points = vec![point("a", "active", true), other];

        assert_eq!(check_rows(&points, None, None, None).len(), 2);
        assert_eq!(check_rows(&points, None, Some("input"), None).len(), 1);
        assert_eq!(check_rows(&points, Some("content"), None, None).len(), 1);
        // 节点过滤优先于领域过滤（节点更细）。
        let rows = check_rows(&points, Some("gameplay"), Some("waves"), Some("b"));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "b");
        assert!(rows[0].active);
    }

    #[test]
    fn exemption_text_renders_signature_and_baseline_case() {
        let mut exempted = point("a", "not_applicable", false);
        exempted.exemption = Some(ExemptionView {
            reason_code: "out_of_scope".into(),
            note: "本期不做".into(),
            actor: Some("主策划".into()),
            at: Some("2026-08-29T00:00:00Z".into()),
        });
        let text = exemption_text(&exempted);
        assert!(text.contains("out_of_scope"), "{text}");
        assert!(text.contains("主策划"), "{text}");

        let baseline = ExemptionView {
            reason_code: "baseline_skip".into(),
            note: String::new(),
            actor: None,
            at: None,
        };
        exempted.exemption = Some(baseline);
        assert!(exemption_text(&exempted).contains("无署名"));
        assert!(exemption_text(&point("a", "active", true)).is_empty());
    }

    #[test]
    fn profile_rows_mark_primary_and_handle_empty() {
        let empty = ProjectProfile {
            project_name: "p".into(),
            genre_pack: "lane_defense".into(),
            depth_target: DesignLevel::L4,
            fields: Vec::new(),
        };
        assert_eq!(profile_rows(&empty).len(), 1);
        assert!(profile_rows(&empty)[0].label.contains("尚未成形"));

        let profile = ProjectProfile {
            project_name: "p".into(),
            genre_pack: "lane_defense".into(),
            depth_target: DesignLevel::L4,
            fields: vec![ProfileField {
                decision_id: "u.experience".into(),
                level: DesignLevel::L1,
                label: "核心体验是什么？".into(),
                design_question: Some("玩家会反复获得什么感受？".into()),
                mda_layer: Some("体验目标".into()),
                domain_id: "unassigned".into(),
                node_id: "unassigned".into(),
                selected: vec![
                    ProfileOption {
                        option_id: "guardian".into(),
                        label: "守护逆转".into(),
                        is_primary: true,
                    },
                    ProfileOption {
                        option_id: "power".into(),
                        label: "力量释放".into(),
                        is_primary: false,
                    },
                ],
            }],
        };
        let rows = profile_rows(&profile);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].value, "★守护逆转、力量释放");
        assert!(rows[0].hint.contains("MDA 体验目标"), "{}", rows[0].hint);
    }

    /// 门 1 通过但带 N/A 明细：不能被渲染成失败。
    #[test]
    fn validation_rows_keep_passed_gate_with_findings_green() {
        let rows = validation_rows(&overview());
        let gate_row = rows
            .iter()
            .find(|row| row.title.contains("gate1_completeness"))
            .expect("门 1 应在校验页签");
        assert_eq!(gate_row.kind, "ok", "passed=true 的门不得渲染为 bad");
        assert!(gate_row.title.contains("通过"));
        let finding = rows
            .iter()
            .find(|row| row.title.contains("not_applicable_exemption"))
            .expect("豁免明细应可见");
        assert_eq!(finding.kind, "info");
        assert!(
            rows.iter().any(|row| row.title.contains("未全绿")),
            "总判定来自 all_gates_passed"
        );
    }

    #[test]
    fn missing_rows_carry_jump_target() {
        let rows = missing_rows(&overview());
        assert_eq!(rows[0].kind, "header");
        assert_eq!(rows[1].target, "ld.wave");
        assert!(rows[1].text.contains("未选择"));
    }

    #[test]
    fn risk_rows_flag_missing_red_team() {
        let rows = risk_rows(&overview());
        assert!(rows.iter().any(|row| row.title.contains("输入与控制")));
        assert!(rows.iter().any(|row| row.title.contains("尚未运行红队")));
    }

    #[test]
    fn summary_rows_report_totals_and_domains() {
        let rows = summary_rows(&overview());
        assert!(rows.iter().any(|row| row.title.contains("1/3（33%）")));
        assert!(rows.iter().any(|row| row.title.contains("玩法系统设计")));
    }

    /// 流水线全版图：C 段 7 个 + P 段 6 个，未冻结时 C 段提示先冻结。
    #[test]
    fn stage_rows_cover_c_and_p_segments() {
        let rows = stage_rows(None);
        assert_eq!(rows.len(), 13);
        assert_eq!(rows[0].id, "C0");
        assert!(rows[0].status.contains("先完成设计冻结"));
        assert!(!rows[0].placeholder);
        let phase2: Vec<&StageItem> = rows.iter().filter(|row| row.placeholder).collect();
        assert_eq!(phase2.len(), 6);
        assert_eq!(phase2[0].id, "P0");
        assert_eq!(phase2[0].segment, "P 段");
    }

    #[test]
    fn log_rows_filter_case_insensitively_and_newest_first() {
        let entries = vec![
            RunLogEntry {
                at: "2026-08-29T00:00:00Z".into(),
                category: "project".into(),
                message: "创建项目".into(),
            },
            RunLogEntry {
                at: "2026-08-29T00:00:01Z".into(),
                category: "Authoring".into(),
                message: "标记不适用".into(),
            },
        ];
        let rows = log_rows(entries.clone(), "");
        assert_eq!(rows[0].category, "Authoring", "最新在前");
        assert_eq!(log_rows(entries.clone(), "authoring").len(), 1);
        assert_eq!(log_rows(entries.clone(), "不适用").len(), 1);
        assert_eq!(log_rows(entries, "nothing").len(), 0);
    }

    #[test]
    fn workbench_markdown_contains_all_sections() {
        let profile = ProjectProfile {
            project_name: "验证项目".into(),
            genre_pack: "lane_defense".into(),
            depth_target: DesignLevel::L5,
            fields: Vec::new(),
        };
        let points = vec![point("ld.input", "active", true)];
        let text = workbench_markdown(&overview(), &profile, &points);
        for needle in [
            "# 设计工作台快照 · 验证项目",
            "## 项目画像",
            "## 领域进度",
            "## 决策点清单",
            "### 玩法系统设计",
            "ld.input",
            "## 缺失项",
            "ld.wave",
            "## 风险",
            "手感未经试玩验证",
            "## 校验",
            "gate1_completeness",
        ] {
            assert!(text.contains(needle), "缺少「{needle}」：\n{text}");
        }
    }

    #[test]
    fn level_brief_explains_editor_location() {
        let brief = level_brief(None, "");
        assert!(brief.contains("中栏"), "{brief}");
        let selected = level_brief(Some(&point("ld.input", "active", true)), "表结构");
        assert!(selected.contains("ld.input"));
        assert!(selected.contains("表结构"));
    }

    // ---- T12 三视图行模型 ----

    fn change_req(status: ChangeStatus, segments: &[&str]) -> ChangeRequest {
        ChangeRequest {
            id: "chg-1".into(),
            title: "加精英怪".into(),
            description: String::new(),
            requested_by: "策划".into(),
            created_at: "t0".into(),
            status,
            affected_segments: segments.iter().map(|s| s.to_string()).collect(),
            target_frozen_version: 0,
            last_actor: String::new(),
            last_note: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn change_rows_gate_buttons_by_status() {
        let drafted = change_rows(&[change_req(ChangeStatus::Drafted, &[])]);
        assert!(drafted[0].can_impact && !drafted[0].can_advance);

        let analyzed = change_rows(&[change_req(ChangeStatus::ImpactAnalyzed, &["C2", "C3"])]);
        assert!(analyzed[0].can_impact && analyzed[0].can_advance);
        assert_eq!(analyzed[0].next_token.as_str(), "scheduled");
        assert_eq!(analyzed[0].segments.as_str(), "C2,C3");

        let scheduled = change_rows(&[change_req(ChangeStatus::Scheduled, &["C0"])]);
        assert!(!scheduled[0].can_impact && scheduled[0].can_advance);
        assert_eq!(scheduled[0].next_token.as_str(), "applied");

        let applied = change_rows(&[change_req(ChangeStatus::Applied, &["C0"])]);
        assert!(!applied[0].can_impact && !applied[0].can_advance);
        assert_eq!(applied[0].status_kind.as_str(), "good");

        let rejected = change_rows(&[change_req(ChangeStatus::Rejected, &[])]);
        assert_eq!(rejected[0].status_kind.as_str(), "bad");
    }

    #[test]
    fn deliverable_rows_and_summary_reflect_completeness() {
        use adm4_app::DeliverableSegment;
        let seg = |stage: &str, present: bool| DeliverableSegment {
            stage_id: stage.into(),
            present,
            document_sha256: if present {
                "abcdef0123456789".into()
            } else {
                String::new()
            },
            contract_sha256: if present {
                "fedcba9876543210".into()
            } else {
                String::new()
            },
            document_bytes: if present { 128 } else { 0 },
        };
        let manifest = DeliverableManifest {
            archive_id: "a".into(),
            frozen_version: 2,
            generated_at: "t".into(),
            complete: false,
            missing_segments: vec!["C6".into()],
            segments: vec![seg("C0", true), seg("C6", false)],
        };
        let rows = deliverable_rows(&manifest);
        assert!(rows[0].present && rows[0].detail.as_str().contains("abcdef012345"));
        assert!(!rows[1].present && rows[1].detail.as_str().contains("未生成"));
        assert!(deliverable_summary(&manifest).contains("缺段：C6"));
    }

    #[test]
    fn sdk_rows_mark_pending_and_signed() {
        use adm4_app::SdkRecord;
        let record = |id: &str, status: SdkReviewStatus, reviewer: &str| SdkRecord {
            id: id.into(),
            sdk_name: "DOTween".into(),
            url: "u".into(),
            category: "anim".into(),
            target_engines: "Unity".into(),
            target_platforms: "windows-desktop".into(),
            purpose: "p".into(),
            status,
            reviewer: reviewer.into(),
            reviewed_at: if reviewer.is_empty() { "" } else { "t" }.into(),
            review_note: if reviewer.is_empty() { "" } else { "ok" }.into(),
            created_at: "t".into(),
        };
        let snapshot = SdkSnapshot {
            records: vec![
                record("s1", SdkReviewStatus::Pending, ""),
                record("s2", SdkReviewStatus::Approved, "评审员"),
            ],
            pending_count: 1,
            approved_count: 1,
            rejected_count: 0,
        };
        let rows = sdk_rows(&snapshot);
        assert!(rows[0].pending && rows[0].status_kind.as_str() == "warn");
        assert!(!rows[1].pending && rows[1].status_kind.as_str() == "good");
        assert!(rows[1].signed.as_str().contains("批准"));
        assert!(sdk_counts(&snapshot).contains("待审 1"));
    }

    // ---- F4c 装配：流水线控制 / 阶段详情 / 体检 / AI 配置 / 另存模板 / 重置 ----

    fn stage_record(
        stage: &str,
        status: StageStatus,
        started: &str,
        finished: &str,
    ) -> StageRecord {
        StageRecord {
            stage_id: stage.into(),
            status,
            contract_hash: String::new(),
            started_at: started.into(),
            finished_at: finished.into(),
            human_confirmation: None,
        }
    }

    fn run_state(records: Vec<StageRecord>) -> PipelineRunState {
        PipelineRunState {
            frozen_hash: "hash".into(),
            stages: records
                .into_iter()
                .map(|record| (record.stage_id.clone(), record))
                .collect(),
        }
    }

    /// 耗时：两端时刻齐备才有值；缺一（旧存档 / 运行中）就说「未在案」，不显示 0 秒。
    #[test]
    fn duration_text_reports_unknown_instead_of_zero() {
        assert!(duration_text(None).contains("未在案"));
        assert_eq!(duration_text(Some(0)), "耗时 0s");
        assert_eq!(duration_text(Some(45)), "耗时 45s");
        assert_eq!(duration_text(Some(63)), "耗时 1m03s");
        assert_eq!(duration_text(Some(3723)), "耗时 1h02m03s");
    }

    /// 阶段行：耗时/运行中/产物入口/重跑按钮全部由后端状态推出，UI 不自行判定。
    #[test]
    fn stage_rows_carry_duration_and_gate_buttons_by_backend_status() {
        // 未冻结（无运行状态）：没有任何段可看产物或重跑。
        let fresh = stage_rows(None);
        assert!(fresh.iter().all(|row| !row.can_rerun && !row.can_inspect));
        assert!(fresh[0].duration.as_str().contains("未在案"));

        let state = run_state(vec![
            stage_record(
                "C0",
                StageStatus::Succeeded,
                "2026-08-31T10:00:00Z",
                "2026-08-31T10:00:12Z",
            ),
            stage_record("C1", StageStatus::Running, "2026-08-31T10:00:12Z", ""),
            stage_record("C2", StageStatus::Pending, "", ""),
        ]);
        let rows = stage_rows(Some(&state));
        let row = |id: &str| {
            rows.iter()
                .find(|row| row.id == id)
                .unwrap_or_else(|| panic!("缺少阶段 {id}"))
        };

        let c0 = row("C0");
        assert_eq!(c0.status.as_str(), "成功");
        assert_eq!(c0.duration.as_str(), "耗时 12s");
        assert!(c0.can_rerun && c0.can_inspect, "跑过的段要给重跑与产物入口");
        assert!(!c0.running);

        let c1 = row("C1");
        assert!(c1.running, "Running 状态要能在行上显示成运行中");
        assert_eq!(c1.status.as_str(), "运行中");
        assert!(
            c1.duration.as_str().contains("未在案"),
            "运行中没有结束时刻"
        );
        assert!(c1.can_rerun && c1.can_inspect);

        let c2 = row("C2");
        assert!(
            !c2.can_rerun && !c2.can_inspect,
            "Pending 段没跑过：既无产物可看也无产物需作废"
        );

        // C3-C6 无记录 = 未跑过；P 段永远不给这两个按钮。
        assert!(!row("C6").can_rerun);
        let phase2 = row("P0");
        assert!(phase2.placeholder && !phase2.can_rerun && !phase2.can_inspect);
    }

    #[test]
    fn stage_ids_follow_the_registry() {
        let ids: Vec<String> = stage_ids().into_iter().map(|id| id.to_string()).collect();
        assert_eq!(ids, vec!["C0", "C1", "C2", "C3", "C4", "C5", "C6"]);
    }

    /// 「停止」的文案必须说清是段边界粒度，不能写成「立即中止」。
    #[test]
    fn pipeline_run_hint_explains_stage_boundary_cancellation() {
        let running = pipeline_run_hint(true, "C0", "C6");
        assert!(running.contains("当前阶段结束后"), "{running}");
        assert!(!running.contains("立即"), "{running}");
        assert!(pipeline_run_hint(false, "C0", "C6").contains("断点续跑"));
    }

    /// 重跑警示必须点名下游段与人工门署名作废。
    #[test]
    fn rerun_warning_names_downstream_and_signature_revocation() {
        let text = rerun_warning("C2", &["C3".into(), "C4".into(), "C5".into(), "C6".into()]);
        assert!(text.contains("强制重跑 C2"), "{text}");
        assert!(text.contains("C3、C4、C5、C6"), "{text}");
        assert!(text.contains("人工门"), "{text}");
        assert!(text.contains("作废"), "{text}");

        // 末段无下游时也要照样给出确认语，不能变成空白提示。
        let last = rerun_warning("C6", &[]);
        assert!(last.contains("C6") && last.contains("确认"), "{last}");
    }

    #[test]
    fn reset_report_rows_list_every_revoked_confirmation() {
        use adm4_pipeline::RevokedConfirmation;
        let report = StageResetReport {
            target: "C2".into(),
            reset_stages: vec!["C2".into(), "C5".into()],
            revoked_confirmations: vec![RevokedConfirmation {
                stage_id: "C5".into(),
                actor: "主美".into(),
                at: "2026-08-30T09:00:00Z".into(),
            }],
            cleared_artifacts: vec!["C2".into()],
        };
        let rows = reset_report_rows(&report);
        assert_eq!(rows[0].kind, "header");
        assert!(rows.iter().any(|row| row.text.contains("C2、C5")));
        assert!(
            rows.iter()
                .any(|row| row.title.contains("C5") && row.text.contains("主美")),
            "作废署名要逐条可见"
        );

        // 没有署名被作废时，也要有一条明确的「无」而不是空列表。
        let empty = StageResetReport {
            target: "C6".into(),
            reset_stages: vec!["C6".into()],
            revoked_confirmations: Vec::new(),
            cleared_artifacts: Vec::new(),
        };
        assert!(
            reset_report_rows(&empty)
                .iter()
                .any(|row| row.title.contains("无人工门署名被作废"))
        );
    }

    fn artifact_file(name: &str, present: bool) -> adm4_app::ArtifactFileView {
        adm4_app::ArtifactFileView {
            file_name: name.into(),
            present,
            path: format!("D:\\data\\C2\\{name}"),
            sha256: if present {
                "0123456789abcdef0123".into()
            } else {
                String::new()
            },
            bytes: if present { 2048 } else { 0 },
        }
    }

    fn artifact(complete: bool, truncated: bool) -> StageArtifactView {
        StageArtifactView {
            archive_id: "arc".into(),
            frozen_version: 2,
            stage_id: "C2".into(),
            complete,
            missing: if complete {
                Vec::new()
            } else {
                vec!["document.md".into()]
            },
            document: artifact_file("document.md", complete),
            contract: artifact_file("contract.json", true),
            document_text: complete.then(|| "# C2 玩法文档".to_string()),
            document_truncated: truncated,
            preview_limit_bytes: 256 * 1024,
        }
    }

    #[test]
    fn artifact_view_exposes_paths_digests_and_missing_state() {
        let complete = artifact(true, false);
        assert!(artifact_title(&complete).contains("齐备"));
        let rows = artifact_rows(&complete);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, "ok");
        assert!(
            rows[0].title.contains("sha256 0123456789ab"),
            "{}",
            rows[0].title
        );
        assert!(rows[0].text.contains("document.md"));
        assert_eq!(artifact_document(&complete), "# C2 玩法文档");
        assert!(artifact_hint(&complete).is_empty());

        let missing = artifact(false, false);
        assert!(artifact_title(&missing).contains("缺 document.md"));
        assert_eq!(artifact_rows(&missing)[0].kind, "bad");
        assert!(
            artifact_document(&missing).contains("未生成"),
            "缺文件不能渲染成空白正文"
        );
        assert!(artifact_hint(&missing).contains("缺产物"));
    }

    /// 截断预览必须显式说「非全文」，否则会被拿去当完整产物核对。
    #[test]
    fn artifact_hint_flags_truncated_preview_as_not_full_text() {
        let hint = artifact_hint(&artifact(true, true));
        assert!(hint.contains("非全文"), "{hint}");
        assert!(hint.contains("256 KiB"), "{hint}");
    }

    #[test]
    fn doctor_rows_make_health_obvious() {
        let healthy = doctor_rows(&ProjectDoctorReport {
            archive_id: "arc-1".into(),
            healthy: true,
            problems: Vec::new(),
        });
        assert!(healthy[1].kind == "ok" && healthy[1].title.contains("健康"));

        let broken = doctor_rows(&ProjectDoctorReport {
            archive_id: "arc-1".into(),
            healthy: false,
            problems: vec!["内容指纹不一致".into(), "manifest 不可读".into()],
        });
        assert!(broken[1].title.contains("2 个问题") && broken[1].kind == "bad");
        assert!(broken.iter().any(|row| row.text.contains("内容指纹不一致")));
        assert!(broken.iter().filter(|row| row.kind == "bad").count() >= 3);
    }

    /// R7：AI 不可用必须显示成不可用，并原样带出后端给的原因。
    #[test]
    fn ai_doctor_rows_never_paint_failure_as_success() {
        let blocked = AiDoctorReport {
            available: false,
            provider_id: String::new(),
            detail: "未配置 AI Provider（config/app.json 的 ai_provider）".into(),
        };
        let rows = ai_doctor_rows(&blocked);
        assert!(rows.iter().any(|row| row.title.contains("不可用")));
        assert!(
            rows.iter()
                .any(|row| row.text.contains("未配置 AI Provider"))
        );
        assert!(rows.iter().all(|row| row.kind != "ok"));
        assert!(ai_status_text(&blocked).starts_with("AI：不可用"));

        let ready = AiDoctorReport {
            available: true,
            provider_id: "openai".into(),
            detail: "已配置且密钥可解析".into(),
        };
        let rows = ai_doctor_rows(&ready);
        assert!(
            rows.iter()
                .any(|row| row.kind == "ok" && row.title.contains("openai"))
        );
        assert_eq!(ai_status_text(&ready), "AI：可用（openai）");
    }

    /// R7：实调用失败必须画成失败并原样带出原因；成功则给可核对的事实。
    #[test]
    fn ai_invoke_rows_report_failure_verbatim() {
        let failed = AiInvokeCheckReport {
            succeeded: false,
            provider_id: "openai".into(),
            model: String::new(),
            response_chars: 0,
            elapsed_ms: 1234,
            detail: "ai provider returned status 401: invalid api key".into(),
            at: "2026-08-31T00:00:00Z".into(),
        };
        let rows = ai_invoke_rows(&failed);
        assert!(rows.iter().all(|row| row.kind != "ok"));
        assert!(rows.iter().any(|row| row.text.contains("status 401")));
        assert!(ai_invoke_status_text(&failed).starts_with("AI 实调用检查失败"));

        let ok = AiInvokeCheckReport {
            succeeded: true,
            provider_id: "openai".into(),
            model: "gpt-4o-mini".into(),
            response_chars: 2,
            elapsed_ms: 380,
            detail: "OK".into(),
            at: "2026-08-31T00:00:00Z".into(),
        };
        let rows = ai_invoke_rows(&ok);
        assert!(
            rows.iter()
                .any(|row| row.kind == "ok" && row.title.contains("gpt-4o-mini"))
        );
        assert!(rows.iter().any(|row| row.text.contains("380 ms")));
        assert!(ai_invoke_status_text(&ok).contains("实调用成功"));
    }

    /// 密钥面板只列名字，绝不列值。
    #[test]
    fn secret_names_text_lists_names_only() {
        assert!(secret_names_text(&[]).contains("尚无 named secret"));
        let text = secret_names_text(&["my_key".to_string(), "other".to_string()]);
        assert!(text.contains("named:my_key") && text.contains("named:other"));
        assert!(text.contains("值不展示"));
    }

    #[test]
    fn provider_form_round_trips_config_and_flags_missing_fields() {
        assert!(provider_summary(None).contains("尚未配置"));
        let empty = provider_form(None);
        assert!(empty.provider_id.is_empty() && empty.timeout_secs.is_empty());

        let config = HttpProviderConfig {
            provider_id: "deepseek".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            model: "deepseek-chat".into(),
            api_key_ref: "env:DEEPSEEK_API_KEY".into(),
            timeout_secs: 90,
        };
        let form = provider_form(Some(&config));
        assert_eq!(form.provider_id, "deepseek");
        assert_eq!(form.timeout_secs, "90");
        assert!(provider_summary(Some(&config)).contains("env:DEEPSEEK_API_KEY"));

        assert!(
            missing_provider_fields(
                &form.provider_id,
                &form.base_url,
                &form.model,
                &form.api_key_ref,
                &form.timeout_secs
            )
            .is_empty()
        );
        let missing = missing_provider_fields("openai", " ", "", "env:K", "120");
        assert_eq!(missing, vec!["Base URL", "模型名"]);
    }

    #[test]
    fn template_export_text_shows_exported_and_skipped_counts() {
        let report = TemplateExportReport {
            template_id: "tpl_from_project".into(),
            genre_pack: "lane_defense".into(),
            game_name: "验证项目".into(),
            source_archive_id: "arc".into(),
            source_project_name: "验证项目".into(),
            depth_reached: DesignLevel::L5,
            status: "HumanReviewed".into(),
            reviewed_by: "主策划".into(),
            exported_points: 2571,
            exported_additional_options: 7,
            exported_primary_marks: 3,
            skipped_unconfirmed: 14,
            skipped_unknown: vec!["ld.wave/legacy".into()],
        };
        let text = template_export_text(&report);
        assert!(text.contains("2571"), "{text}");
        assert!(text.contains("跳过未确认决策点 14 个"), "{text}");
        assert!(text.contains("失效选项 1 条"), "{text}");
        assert!(text.contains("认证入库"), "另存后还需 S5 认证才能预填");
    }

    #[test]
    fn workbench_reset_texts_state_scope_and_counts() {
        let warning = reset_workbench_warning();
        assert!(warning.contains("清空"), "{warning}");
        assert!(warning.contains("已冻结版本"), "{warning}");
        assert!(warning.contains("必填"), "{warning}");

        let report = WorkbenchResetReport {
            cleared_selections: 120,
            cleared_primary_marks: 4,
            cleared_parameter_values: 9,
            cleared_exemptions: 3,
            cleared_node_design_notes: 2,
            cleared_node_risk_notes: 1,
            actor: "主策划".into(),
            at: "2026-08-31T12:00:00Z".into(),
        };
        let text = reset_workbench_text(&report);
        assert!(text.contains("120"), "{text}");
        assert!(text.contains("主策划"), "{text}");
        assert!(text.contains("已冻结版本与流水线产物不受影响"), "{text}");

        let noop = WorkbenchResetReport {
            cleared_selections: 0,
            cleared_primary_marks: 0,
            cleared_parameter_values: 0,
            cleared_exemptions: 0,
            cleared_node_design_notes: 0,
            cleared_node_risk_notes: 0,
            actor: "主策划".into(),
            at: "t".into(),
        };
        assert!(reset_workbench_text(&noop).contains("本来就没有任何创作内容"));
    }

    // -----------------------------------------------------------------------
    // G2 风格门装配层
    // -----------------------------------------------------------------------

    fn style_row(style_id: &str, risk: StyleFitRisk) -> StyleDirectionStatus {
        StyleDirectionStatus {
            style_id: style_id.into(),
            title: "清晰量产".into(),
            description: "以可读性与量产效率优先".into(),
            prompt_summary: "flat clean vector-like game art…".into(),
            prompt_overridden: false,
            recommended: false,
            recommended_reason: String::new(),
            palette: vec!["#2F4858".into(), "#F6C445".into()],
            fit_risk: risk,
            fit_reason: "信息密度代价低，与已确认方向适配".into(),
            image_path: String::new(),
            image_sha256: String::new(),
            last_failure: String::new(),
            is_selected: false,
        }
    }

    fn style_status(rows: Vec<StyleDirectionStatus>) -> StyleGateStatus {
        StyleGateStatus {
            project_name: "霜落峡谷防卫计划".into(),
            genre_pack: "lane_defense".into(),
            session_present: true,
            session_revision: 12,
            current_revision: 12,
            session_stale: false,
            directions: rows,
            round_count: 2,
            latest_round_id: "r0002".into(),
            anchor_versions: Vec::new(),
            readiness: adm4_app::StyleReadiness::not_ready("从未确认过风格锚点"),
            anchor_stale: false,
            confirmed_actor: String::new(),
            confirmed_at: String::new(),
            confirmed_notes: String::new(),
        }
    }

    /// 卡片三态：无图 / 有图加载成功 / 有图加载失败——三条分支的呈现必须区分得开，
    /// 否则用户看到一个空框却不知道是「还没生成」还是「图坏了」。
    #[test]
    fn style_cards_distinguish_missing_loaded_and_broken_images() {
        let mut rows = vec![
            style_row("STYLE-01-readable_production", StyleFitRisk::Ok),
            style_row("STYLE-02-concept_painting", StyleFitRisk::Caution),
            style_row("STYLE-03-high_contrast_arcade", StyleFitRisk::Unknown),
        ];
        // ① 没图，且上一轮生成失败过。
        rows[0].last_failure = "图像 API 返回 503".into();
        // ② 有图（加载器会成功）。
        rows[1].image_path = "previews/r0002/STYLE-02-concept_painting.png".into();
        rows[1].image_sha256 = "sha256:abcdef0123456789".into();
        rows[1].prompt_overridden = true;
        rows[1].recommended = true;
        // ③ 有图但加载失败。
        rows[2].image_path = "previews/r0001/broken.png".into();
        let status = style_status(rows);

        let cards = style_cards(&status, Some("STYLE-02-concept_painting"), &|relative| {
            if relative.contains("broken") {
                Err("预览图 broken.png 加载失败（文件在但不是可读的图像）".to_string())
            } else {
                Ok(Image::default())
            }
        });
        assert_eq!(cards.len(), 3);

        assert!(!cards[0].has_image);
        assert_eq!(cards[0].failure, "图像 API 返回 503");
        assert_eq!(cards[0].fit_kind, "ok");
        assert!(!cards[0].active);
        assert_eq!(cards[0].prompt_origin, "派生自真源");

        assert!(cards[1].has_image, "加载成功的图必须画出来");
        assert!(cards[1].failure.is_empty());
        assert_eq!(cards[1].fit_kind, "caution");
        assert!(cards[1].active, "聚焦方向要高亮");
        assert_eq!(cards[1].badge, "◎推荐 ");
        assert_eq!(cards[1].prompt_origin, "用户改词");
        assert!(cards[1].palette.contains("#2F4858"));
        assert!(cards[1].fit.contains("需注意"), "{}", cards[1].fit);

        assert!(!cards[2].has_image, "加载失败不许画一张空图冒充有图");
        assert!(
            cards[2].failure.contains("加载失败"),
            "{}",
            cards[2].failure
        );
        assert_eq!(cards[2].fit_kind, "unknown");
    }

    /// 已确认的方向徽章压过推荐徽章（用户要先看到「这就是当前锚点」）。
    #[test]
    fn confirmed_direction_badge_wins_over_recommended() {
        let mut row = style_row("STYLE-01-readable_production", StyleFitRisk::Ok);
        row.recommended = true;
        row.is_selected = true;
        let cards = style_cards(&style_status(vec![row]), None, &|_| Ok(Image::default()));
        assert_eq!(cards[0].badge, "★已确认 ");
        assert!(cards[0].selected);
    }

    /// 摘要三态：未打开项目 / 未生成 / 已生成（含真源已变的提醒）。
    #[test]
    fn style_summary_covers_all_phases() {
        assert!(style_summary_without_project().contains("尚未打开项目"));

        let mut status = style_status(Vec::new());
        status.session_present = false;
        let text = style_summary(&status);
        assert!(text.contains("尚未生成风格方向"), "{text}");
        assert!(text.contains("R4"), "要说清无锚会报错：{text}");

        let mut status = style_status(vec![style_row("STYLE-01-x", StyleFitRisk::Ok)]);
        let text = style_summary(&status);
        assert!(text.contains("1 个方向"), "{text}");
        assert!(text.contains("最近 r0002"), "{text}");
        assert!(!text.contains("设计已变"), "{text}");

        status.session_stale = true;
        status.current_revision = 15;
        let text = style_summary(&status);
        assert!(text.contains("设计已变"), "{text}");
        assert!(text.contains("提示不阻断"), "{text}");
    }

    /// 就绪文案：未确认必须点明下游被阻断；已确认要带版本历史与落后提醒。
    #[test]
    fn style_readiness_text_states_the_downstream_verdict() {
        let status = style_status(Vec::new());
        let text = style_readiness_text(&status);
        assert!(text.starts_with("[阻断]"), "{text}");
        assert!(
            text.contains(adm4_app::STYLE_APPLICATION_CONTRACT_NOT_APPROVED),
            "{text}"
        );
        assert!(text.contains("P2 资产生产被阻断"), "{text}");

        let mut ready = style_status(Vec::new());
        ready.anchor_versions = vec![1, 2];
        ready.anchor_stale = true;
        ready.readiness = adm4_app::StyleReadiness {
            ready: true,
            anchor_version: 2,
            selected_style_id: "STYLE-02-concept_painting".into(),
            anchor_hash: "sha256:x".into(),
            detail: "风格锚点 v2 已确认（方向 概念绘画，署名 主美甲 于 t）".into(),
        };
        let text = style_readiness_text(&ready);
        assert!(text.starts_with("[就绪]"), "{text}");
        assert!(text.contains("锚点历史 v1/v2"), "{text}");
        assert!(text.contains("落后于当前设计"), "{text}");
    }

    /// 门控指路：图像通道不可用时必须说清缺什么、去哪配（不只说「不可用」）。
    #[test]
    fn style_gate_hint_points_at_the_missing_configuration() {
        let unavailable = AiDoctorReport {
            available: false,
            provider_id: String::new(),
            detail: "未配置图像 Provider：请在 config/app.json 补一段 image_provider…".into(),
        };
        let hint = style_gate_hint(&unavailable, true);
        assert!(hint.contains("image_provider"), "{hint}");
        assert!(hint.contains("图像通道"), "{hint}");
        assert!(hint.contains("R7"), "{hint}");
        // 没打开项目时不该先怪配置（先让人载入项目）。
        assert!(style_gate_hint(&unavailable, false).is_empty());
        // 可用时不出提示。
        let available = AiDoctorReport {
            available: true,
            provider_id: "openai_images".into(),
            detail: "已配置".into(),
        };
        assert!(style_gate_hint(&available, true).is_empty());
        assert!(image_status_text(&available).contains("可用（openai_images）"));
        assert!(image_status_text(&unavailable).contains("不可用"));
    }

    /// 改词框的来源说明三态（没选方向 / 派生提示词 / 用户改词）。
    #[test]
    fn style_prompt_origin_states_where_the_prompt_came_from() {
        assert!(style_prompt_origin(None).contains("选中一个方向"));

        let derived = StyleDirection {
            derived_prompt: "flat clean vector-like game art".into(),
            prompt_anchors: vec![
                adm4_contracts::SpecRef::new("profile/u.genre"),
                adm4_contracts::SpecRef::new("profile/u.platform"),
            ],
            ..StyleDirection::default()
        };
        let text = style_prompt_origin(Some(&derived));
        assert!(text.contains("派生自真源"), "{text}");
        assert!(text.contains("2 条锚点"), "{text}");

        let overridden = StyleDirection {
            prompt_override: "colder palette".into(),
            ..derived.clone()
        };
        assert!(style_prompt_origin(Some(&overridden)).contains("你的改词"));
    }

    /// 已锁定摘要：下游要照它消费，关键字段（署名/结论/最终提示词/锚图指纹/契约哈希）
    /// 一条都不能缺。
    #[test]
    fn style_lock_rows_expose_every_field_downstream_consumes() {
        let anchor_set = StyleAnchorSet {
            schema_version: "4.0.0".into(),
            anchor_version: 3,
            generated_at: "2026-09-01T00:00:00Z".into(),
            project_name: "霜落峡谷防卫计划".into(),
            genre_pack: "lane_defense".into(),
            source_revision: 12,
            source_anchors: vec![adm4_contracts::SpecRef::new("profile/u.genre")],
            selected_style_id: "STYLE-01-readable_production".into(),
            selected_title: "清晰量产".into(),
            preset_key: "readable_production".into(),
            final_prompt: "colder palette, thicker outlines".into(),
            prompt_overridden: true,
            palette: vec!["#2F4858".into(), "#F6C445".into()],
            anchors: vec![adm4_app::StyleAnchorImage {
                anchor_id: "ANCHOR-STYLE-01-readable_production-selected_preview".into(),
                role: "selected_preview".into(),
                image_path: "anchors/v3/STYLE-01-readable_production.png".into(),
                image_sha256: "sha256:0123456789abcdef".into(),
                image_bytes: 4096,
                media_type: "image/png".into(),
                requested_width: 512,
                requested_height: 512,
                prompt: "colder palette, thicker outlines".into(),
                provider_id: "openai_images".into(),
                model: "gpt-image-1".into(),
            }],
            confirmation: adm4_app::StyleConfirmation {
                selected_style_id: "STYLE-01-readable_production".into(),
                selected_title: "清晰量产".into(),
                selected_image_path: "anchors/v3/STYLE-01-readable_production.png".into(),
                notes: "四个方向都看过大图，选它".into(),
                actor: "主美甲".into(),
                at: "2026-09-01T00:00:00Z".into(),
                anchor_version: 3,
                ..adm4_app::StyleConfirmation::default()
            },
        };
        let contract = StyleApplicationContract::derive(&anchor_set, "2026-09-01T00:00:00Z")
            .expect("派生应用契约");
        let rows = style_lock_rows(&anchor_set, &contract);
        let dump = rows
            .iter()
            .map(|row| format!("{}|{}", row.title, row.text))
            .collect::<Vec<_>>()
            .join("\n");
        for needle in [
            "已锁定风格锚点 v3",
            "清晰量产",
            "readable_production",
            "主美甲",
            "四个方向都看过大图",
            "colder palette, thicker outlines",
            "anchors/v3/STYLE-01-readable_production.png",
            "sha256:01234",
            "分用途约束 5 条",
            "地块/图标/界面/背景/特效",
        ] {
            assert!(dump.contains(needle), "锁定摘要缺「{needle}」：\n{dump}");
        }
        assert_eq!(rows[0].kind, "header");
    }

    /// 大图覆盖层文案：方向说明 + 提示词全文 + 指纹 + 失败原因（看图时要知道要求的是什么）。
    #[test]
    fn style_viewer_texts_carry_the_full_prompt_not_the_summary() {
        let mut row = style_row("STYLE-02-concept_painting", StyleFitRisk::Caution);
        row.title = "概念绘画".into();
        row.image_sha256 = "sha256:deadbeef".into();
        row.prompt_overridden = true;
        row.last_failure = "图像 API 返回 429".into();
        let full_prompt = "painterly concept art, soft brushwork, atmospheric depth, layered value composition, game art style board";
        let (title, detail) = style_viewer_texts(&row, full_prompt);
        assert!(title.contains("概念绘画"), "{title}");
        assert!(title.contains("需注意"), "{title}");
        assert!(detail.contains(full_prompt), "必须是提示词全文而不是摘要");
        assert!(detail.contains("用户改词"), "{detail}");
        assert!(detail.contains("sha256:deadbeef"), "{detail}");
        assert!(detail.contains("图像 API 返回 429"), "{detail}");
        assert!(detail.contains("次数不限"), "{detail}");
    }

    /// 图像通道表单：初值往返 + 必填缺失检查（值是否有效由后端判）。
    #[test]
    fn image_provider_form_round_trips_and_reports_missing_fields() {
        let empty = image_provider_form(None);
        assert!(empty.provider_id.is_empty() && empty.size.is_empty());

        let config = HttpImageProviderConfig {
            provider_id: "openai_images".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-image-1".into(),
            api_key_ref: "env:OPENAI_API_KEY".into(),
            timeout_secs: 300,
            size: "1024x1024".into(),
        };
        let form = image_provider_form(Some(&config));
        assert_eq!(form.provider_id, "openai_images");
        assert_eq!(form.size, "1024x1024");
        assert_eq!(form.timeout_secs, "300");
        assert!(
            missing_image_provider_fields(
                &form.provider_id,
                &form.base_url,
                &form.model,
                &form.api_key_ref,
                &form.size,
                &form.timeout_secs
            )
            .is_empty()
        );
        let missing = missing_image_provider_fields("openai_images", " ", "", "env:K", "  ", "300");
        assert_eq!(
            missing,
            vec!["图像 Base URL", "图像模型名", "生成尺寸（如 1024x1024）"]
        );
    }
}
