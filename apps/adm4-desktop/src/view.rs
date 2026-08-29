//! 后端聚合结果 → UI 行模型的纯装配层。
//!
//! 边界约定（D14）：本模块只做「后端 DTO → 展示文本/行模型」的确定性转换，
//! 不判定任何业务规则（完成度口径、门禁通过、适用性都由后端给出，这里照实呈现）。
//! 之所以单独成模块：装配逻辑（领域卡片要含 0 点域、徽章取值、导出快照排版）
//! 是本任务里唯一有分支的逻辑，必须能被单元测试钉住，而 Slint 回调不便测试。

use crate::{CheckRow, DomainCard, LogItem, NodeCard, OptionRow, ProfileRow, StageItem, TextRow};
use adm4_app::{DecisionPointView, ProjectProfile, RunLogEntry, WorkbenchOverview};
use adm4_decision::{
    DesignDomain, OrganizationProgress, SelectionMode, UNASSIGNED_DOMAIN_ID, UNASSIGNED_NODE_ID,
};
use adm4_pipeline::{PipelineRunState, StageStatus, design_compile_registry, phase2_registry};
use slint::{SharedString, VecModel};

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

/// 流水线全版图：C0-C6（实跑，状态来自 runner）+ P0-P5（Phase 2 占位，不断头）。
pub fn stage_rows(run_state: Option<&PipelineRunState>) -> Vec<StageItem> {
    let mut rows: Vec<StageItem> = design_compile_registry()
        .into_iter()
        .map(|stage| {
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
            StageItem {
                id: stage.id.into(),
                name: stage.name.into(),
                status: status.into(),
                summary: stage.summary.into(),
                segment: "C 段".into(),
                waiting,
                placeholder: false,
            }
        })
        .collect();
    rows.extend(phase2_registry().into_iter().map(|stage| StageItem {
        id: stage.id.into(),
        name: stage.name.into(),
        status: "Phase 2 占位（数据模型已立，执行器另行立项）".into(),
        summary: stage.summary.into(),
        segment: "P 段".into(),
        waiting: false,
        placeholder: true,
    }));
    rows
}

/// 流水线视图的映射注记：二版 Step00-14 与四版 C 段/P 段的对应关系。
pub fn pipeline_note() -> String {
    "二版 Step00-14 → 四版分段映射：\n\
     · Step00-06（创意收集/玩法框架/设计冻结/程序需求/美术需求/程序评审/美术评审）→ 设计工作台冻结门 + C0-C4\n\
     · Step07 美术风格人工确认 → C5 风格段人工门（本视图「人工确认」按钮）\n\
     · Step08-10（程序计划/美术计划/资源对齐）→ C6 开发计划与签收\n\
     · Step11-14（程序执行/美术生产/场景组装/集成验证）→ P0-P5（Phase 2，本视图占位可见，不断头）\n\
     阶段详情/重跑/范围运行的完整交互由 T12 在本视图内补齐。"
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
}
