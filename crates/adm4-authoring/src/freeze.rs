use crate::engine::AuthoringEngine;
use crate::state::{Finding, RedTeamRecord};
use adm4_ai::{AiProvider, AiRequest};
use adm4_contracts::{CategoryEvidence, ReviewProof, SkinScanner, TypedValue, verify_review_batch};
use adm4_decision::{
    DecisionId, DepthProfile, GenrePackId, NaJustification, ParameterValues, Provenance, Selection,
    check_row_references, enumerate_axis,
};
use adm4_foundation::{Adm4Error, Adm4Result, ContentHash, UtcTimestamp, sha256_hex};
use adm4_space::ConsistencyRuleKind;
use serde::{Deserialize, Serialize};

/// 红队发现的合法严重度枚举；缺失或超出枚举一律拒收（不得默认降级为 warning）。
const FINDING_SEVERITIES: [&str; 2] = ["blocker", "warning"];

/// custom 占比警告阈值（W7 §5.6，试用制：advisory 不 block；数值交开放问题 4 标定）。
const CUSTOM_RATIO_ADVISORY_THRESHOLD: f64 = 0.4;

// ---------------------------------------------------------------------------
// 门禁结果结构
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateFinding {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateResult {
    pub gate: String,
    pub passed: bool,
    pub findings: Vec<GateFinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreezeGateReport {
    pub gates: Vec<GateResult>,
    pub custom_option_count: usize,
    pub na_counts: Vec<(String, usize)>,
    /// 非必做（`requirement=Optional`）且未作答、因此未进完成度分母的适用点数。
    /// 旧存档没有该字段（`serde(default)` → 0）。
    #[serde(default)]
    pub optional_skipped: usize,
    pub evaluated_at: String,
}

impl FreezeGateReport {
    pub fn all_passed(&self) -> bool {
        self.gates.iter().all(|gate| gate.passed)
    }
}

/// 冻结产物：唯一内容真相源（只读；修改 = 新冻结版本）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrozenDesign {
    pub version: u32,
    pub project_name: String,
    pub decisions: Vec<Selection>,
    pub not_applicable: Vec<(DecisionId, NaJustification)>,
    pub genre_pack: GenrePackId,
    pub pack_version: String,
    pub depth_profile: DepthProfile,
    pub content_hash: String,
    pub frozen_at: String,
    pub gate_report: FreezeGateReport,
    /// 冻结门第 4 道采信的红队工作量证明；C0-C6 用它与 C1 红队合批做 R3 哈希互异校验。
    /// 旧存档没有该字段（`serde(default)` → None），此时 C1 退化为单份证明校验。
    #[serde(default)]
    pub red_team_proof: Option<ReviewProof>,
}

// ---------------------------------------------------------------------------
// 五道门评估
// ---------------------------------------------------------------------------

/// 评估冻结门 1-4（门 5 = execute_freeze 本身）。
/// 门 4 需要红队记录已存在且针对当前 revision，且全部发现已处置。
pub fn evaluate_freeze_gates(engine: &AuthoringEngine, scanner: &SkinScanner) -> FreezeGateReport {
    let state = engine.state();
    let mut gates = Vec::new();

    // 门 1：完备度 / 拆分就绪度。
    let completeness = engine.completeness();
    let mut gate1_findings: Vec<GateFinding> = completeness
        .blocking
        .iter()
        .map(|item| GateFinding {
            code: "incomplete".into(),
            message: format!("{}：{}", item.decision_id, item.detail),
        })
        .collect();
    // 多选点：任一已选选项是 custom 就计入（少算会低报自定义比例）。
    let custom_option_count = state
        .selections
        .values()
        .filter(|selection| {
            engine
                .space()
                .graph
                .point(&selection.decision_id)
                .is_some_and(|point| {
                    selection.selected_options().into_iter().any(|item| {
                        point
                            .option(item.option_id)
                            .is_some_and(|option| option.is_custom)
                    })
                })
        })
        .count();
    if completeness.total == 0 {
        gate1_findings.push(GateFinding {
            code: "empty_design".into(),
            message: "没有任何适用决策点（设计空间或深度档异常）".into(),
        });
    }
    // 不适用（N/A）豁免逐条在案：不拦截冻结，但必须在门报告里看得见谁以什么理由豁免了什么。
    let mut na_findings: Vec<GateFinding> = state
        .not_applicable
        .iter()
        .map(|(decision_id, justification)| {
            let signature = justification.signature_label();
            let note = if justification.note.trim().is_empty() {
                String::new()
            } else {
                format!("：{}", justification.note)
            };
            GateFinding {
                code: "not_applicable_exemption".into(),
                message: format!(
                    "{decision_id} 已标记不适用[{}]{note}，{signature}",
                    justification.reason_code
                ),
            }
        })
        .collect();
    let gate1_passed = gate1_findings.is_empty();
    gate1_findings.append(&mut na_findings);
    // 非必做点未作答：同样是可见性条目（不进分母、不拦截），但必须在门报告里数得出来，
    // 否则「完成度 100%」会掩盖「有一批点根本没看」。
    if completeness.optional_skipped > 0 {
        gate1_findings.push(GateFinding {
            code: "optional_not_answered".into(),
            message: format!(
                "{} 个非必做点未作答（requirement=optional，不进完成度分母也不拦截冻结）",
                completeness.optional_skipped
            ),
        });
    }
    // custom 占比 >40% 警告（W7 §5.6，试用制）：advisory 不拦截、不参与通过判定。
    // 口径：已确认且任一已选选项 is_custom 的点数 / 已确认点数。
    let confirmed_total = state
        .selections
        .values()
        .filter(|selection| selection.confirmed_by_user)
        .count();
    let confirmed_custom = state
        .selections
        .values()
        .filter(|selection| selection.confirmed_by_user)
        .filter(|selection| {
            engine
                .space()
                .graph
                .point(&selection.decision_id)
                .is_some_and(|point| {
                    selection.selected_options().into_iter().any(|item| {
                        point
                            .option(item.option_id)
                            .is_some_and(|option| option.is_custom)
                    })
                })
        })
        .count();
    if confirmed_total > 0 {
        let ratio = confirmed_custom as f64 / confirmed_total as f64;
        if ratio > CUSTOM_RATIO_ADVISORY_THRESHOLD {
            gate1_findings.push(GateFinding {
                code: "custom_ratio_advisory".into(),
                message: format!(
                    "已确认选择中自定义（custom）占比 {:.0}%（{confirmed_custom}/{confirmed_total}）超过 {:.0}% 参考线：\
                     偏离预设选项过多可能意味着选错了品类包或该沉淀新系统模块（试用制警告，不拦截冻结）",
                    ratio * 100.0,
                    CUSTOM_RATIO_ADVISORY_THRESHOLD * 100.0
                ),
            });
        }
    }
    gates.push(GateResult {
        gate: "gate1_completeness".into(),
        // N/A 与非必做明细是可见性条目，不参与通过判定。
        passed: gate1_passed,
        findings: gate1_findings,
    });

    // 门 2：一致性（决策图约束已在 select 时硬拦截；此处复核 + 品类包跨决策规则）。
    let mut gate2_findings = Vec::new();
    for selection in state.selections.values() {
        let Some(point) = engine.space().graph.point(&selection.decision_id) else {
            gate2_findings.push(GateFinding {
                code: "unknown_decision".into(),
                message: format!("选择引用了清单外的决策点 {}", selection.decision_id),
            });
            continue;
        };
        // 选择基数复核：单选点被塞多选、多选点主选缺失/越界，都不许进 FrozenDesign。
        for problem in adm4_decision::validate_selection_mode(point, selection) {
            gate2_findings.push(GateFinding {
                code: "selection_mode_violated".into(),
                message: format!("{}：{problem}", selection.decision_id),
            });
        }
        // 多选点逐个已选选项复核 requires/conflicts（判定按「对方已选集合是否包含」）。
        for item in selection.selected_options() {
            let Some(option) = point.option(item.option_id) else {
                gate2_findings.push(GateFinding {
                    code: "unknown_option".into(),
                    message: format!(
                        "{} 选择了不存在的选项 {}",
                        selection.decision_id, item.option_id
                    ),
                });
                continue;
            };
            for required in &option.requires {
                let satisfied = state
                    .selections
                    .get(&required.decision)
                    .is_some_and(|other| other.contains_option(&required.option));
                if !satisfied {
                    gate2_findings.push(GateFinding {
                        code: "requires_violated".into(),
                        message: format!(
                            "{}/{} 的前置 {}/{} 未满足",
                            selection.decision_id,
                            item.option_id,
                            required.decision,
                            required.option
                        ),
                    });
                }
            }
            for conflict in &option.conflicts {
                let conflicted = state
                    .selections
                    .get(&conflict.decision)
                    .is_some_and(|other| other.contains_option(&conflict.option));
                if conflicted {
                    gate2_findings.push(GateFinding {
                        code: "conflict_violated".into(),
                        message: format!(
                            "{}/{} 与 {}/{} 冲突",
                            selection.decision_id,
                            item.option_id,
                            conflict.decision,
                            conflict.option
                        ),
                    });
                }
            }
        }
    }
    for rule in &engine.space().pack.consistency_rules {
        match &rule.kind {
            ConsistencyRuleKind::MatrixAxisMatchesTableRows {
                matrix_decision,
                table_decision,
            } => {
                // 多选点：全部已选选项的格数据合并后取行集合。
                let matrix_rows: Vec<String> = match state.selections.get(matrix_decision) {
                    None => Vec::new(),
                    Some(selection) => {
                        let mut rows: Vec<String> = selection
                            .selected_options()
                            .into_iter()
                            .flat_map(|item| match item.parameters {
                                ParameterValues::Cells { cells } => {
                                    cells.iter().map(|cell| cell.row.clone()).collect()
                                }
                                _ => Vec::new(),
                            })
                            .collect();
                        rows.sort();
                        rows.dedup();
                        rows
                    }
                };
                let table_rows = {
                    let axis = adm4_decision::AxisRef::TableRows {
                        decision: table_decision.clone(),
                    };
                    let mut rows = enumerate_axis(&engine.space().graph, &state.selections, &axis);
                    rows.sort();
                    rows
                };
                if !matrix_rows.is_empty() && matrix_rows != table_rows {
                    gate2_findings.push(GateFinding {
                        code: format!("rule.{}", rule.id),
                        message: format!(
                            "矩阵 {matrix_decision} 的行集合与表 {table_decision} 的行集合不一致"
                        ),
                    });
                }
            }
            ConsistencyRuleKind::AnsweredTogether { first, second } => {
                let first_answered = state.selections.contains_key(first);
                let second_answered = state.selections.contains_key(second);
                if first_answered != second_answered {
                    gate2_findings.push(GateFinding {
                        code: format!("rule.{}", rule.id),
                        message: format!("{first} 与 {second} 必须同时回答或同时不适用"),
                    });
                }
            }
            // 跨表外键：悬空行引用不得进 FrozenDesign。
            ConsistencyRuleKind::RowReference { .. } => {
                let Some(reference) = rule.as_row_reference() else {
                    continue;
                };
                for violation in check_row_references(
                    &engine.space().graph,
                    &state.selections,
                    std::slice::from_ref(&reference),
                ) {
                    gate2_findings.push(GateFinding {
                        code: format!("rule.{}", rule.id),
                        message: violation.detail,
                    });
                }
            }
        }
    }
    gates.push(GateResult {
        gate: "gate2_consistency".into(),
        passed: gate2_findings.is_empty(),
        findings: gate2_findings,
    });

    // 门 3：换皮门。
    let mut gate3_findings = Vec::new();
    for selection in state.selections.values() {
        let is_template = matches!(selection.provenance, Provenance::Template { .. });
        let point = engine.space().graph.point(&selection.decision_id);
        // 多选点逐个已选选项过换皮门：皮字段比对与参考名扫描都不许漏掉附加选项。
        for item in selection.selected_options() {
            if is_template
                && let Some(original) = item.template_original
                && let Some(point) = point
            {
                for skin_field in &point.skin_fields {
                    let current = extract_field(item.parameters, skin_field);
                    let template_value = extract_field(original, skin_field);
                    if let (Some(current_value), Some(template_value)) = (current, template_value)
                        && current_value == template_value
                    {
                        gate3_findings.push(GateFinding {
                            code: "skin_field_unchanged".into(),
                            message: format!(
                                "{} 的皮字段 {skin_field} 与模板原值相同，必须换皮",
                                selection.decision_id
                            ),
                        });
                    }
                }
            }
            // 参考名扫描：理由与全部文本参数。
            for hit in scanner.scan(
                &format!("selection:{}", selection.decision_id),
                item.rationale,
            ) {
                gate3_findings.push(GateFinding {
                    code: "reference_name_hit".into(),
                    message: format!("{} 命中参考名 {}", hit.location, hit.matched_word),
                });
            }
            for text in collect_text_values(item.parameters) {
                for hit in scanner.scan(&format!("selection:{}", selection.decision_id), &text) {
                    gate3_findings.push(GateFinding {
                        code: "reference_name_hit".into(),
                        message: format!("{} 参数命中参考名 {}", hit.location, hit.matched_word),
                    });
                }
            }
            // custom 选项不豁免 R5（W7 §5.6，A 的豁免论作废）：custom 是整段抄袭现成机制
            // 的最大通道，其 id/label（换皮比对词表面）与 rule_text 全部进扫描——内建选项
            // 的 label/summary 出自 pack 作者（认证时已查），custom 的出自项目作者，必须
            // 在这里设防。rationale 与 is_skin 文本参数已由上方通用扫描覆盖（custom 的
            // 参数值同样走 collect_text_values，不存在豁免通道）。
            if let Some(option) = point.and_then(|point| point.option(item.option_id))
                && option.is_custom
            {
                for hit in scanner.scan_fields(
                    &format!("custom:{}", selection.decision_id),
                    &[
                        ("id", selection.decision_id.as_str()),
                        ("label", option.label.as_str()),
                        ("rule_text", option.summary.as_str()),
                    ],
                ) {
                    gate3_findings.push(GateFinding {
                        code: "reference_name_hit".into(),
                        message: format!(
                            "{} 命中参考名 {}（custom 机制不豁免换皮门）",
                            hit.location, hit.matched_word
                        ),
                    });
                }
            }
        }
    }
    gates.push(GateResult {
        gate: "gate3_skin".into(),
        passed: gate3_findings.is_empty(),
        findings: gate3_findings,
    });

    // 门 4：AI 红队 + ReviewProof + 逐条处置。
    let mut gate4_findings = Vec::new();
    match &state.red_team {
        None => gate4_findings.push(GateFinding {
            code: "red_team_missing".into(),
            message: "尚未执行 AI 红队评审".into(),
        }),
        Some(record) => {
            if record.reviewed_revision != state.revision {
                gate4_findings.push(GateFinding {
                    code: "red_team_stale".into(),
                    message: format!(
                        "红队评审针对 revision {}，当前已是 {}，设计变更后需重跑",
                        record.reviewed_revision, state.revision
                    ),
                });
            }
            // R3：走批次校验（数量 + 逐类证据 + 同批哈希互异）；本门只有一份红队
            // 证明，互异检查在 C1 与本证明合批时生效（见 c1_validation）。
            if let Err(error) = verify_review_batch(std::slice::from_ref(&record.proof)) {
                gate4_findings.push(GateFinding {
                    code: "red_team_proof_invalid".into(),
                    message: error.message,
                });
            }
            for finding in &record.findings {
                if finding.severity == "blocker" && finding.disposition.is_none() {
                    gate4_findings.push(GateFinding {
                        code: "finding_unresolved".into(),
                        message: format!(
                            "红队发现 {}（{}）未处置：{}",
                            finding.id, finding.target, finding.text
                        ),
                    });
                }
            }
            // custom 逐条强制处置（W7 §5.6）：每个 custom 机制必须有指向它的红队
            // finding 且已显式处置（accept/revise + 署名），缺失即 block 点名机制 id。
            // scripted 通道走同一条路（红队应答无论出自谁，处置留痕一条不能少）。
            for decision_id in state.custom_mechanics.keys() {
                let targeted: Vec<&Finding> = record
                    .findings
                    .iter()
                    .filter(|finding| finding.target.contains(decision_id.as_str()))
                    .collect();
                if targeted.is_empty() {
                    gate4_findings.push(GateFinding {
                        code: "custom_finding_missing".into(),
                        message: format!(
                            "自定义机制 {decision_id} 没有任何红队发现指向它：custom 是红队必审项，\
                             红队应答必须对每个 custom 机制出具至少一条 finding（哪怕是 warning 级的审查结论）"
                        ),
                    });
                    continue;
                }
                for finding in targeted {
                    if finding.disposition.is_none() {
                        gate4_findings.push(GateFinding {
                            code: "custom_finding_undisposed".into(),
                            message: format!(
                                "自定义机制 {decision_id} 的红队发现 {} 未处置：custom 机制的每条 finding \
                                 都必须显式处置（freeze dispose <发现id> accept|revise --actor <署名>）",
                                finding.id
                            ),
                        });
                    }
                }
            }
        }
    }
    gates.push(GateResult {
        gate: "gate4_red_team".into(),
        passed: gate4_findings.is_empty(),
        findings: gate4_findings,
    });

    FreezeGateReport {
        gates,
        custom_option_count,
        na_counts: completeness
            .na_reason_counts
            .iter()
            .map(|(code, count)| (code.clone(), *count))
            .collect(),
        optional_skipped: completeness.optional_skipped,
        evaluated_at: UtcTimestamp::now().to_iso8601(),
    }
}

/// 门 5：执行冻结（前四道全绿才可调用）。
pub fn execute_freeze(
    engine: &mut AuthoringEngine,
    scanner: &SkinScanner,
) -> Adm4Result<FrozenDesign> {
    let report = evaluate_freeze_gates(engine, scanner);
    if !report.all_passed() {
        let blocked: Vec<String> = report
            .gates
            .iter()
            .filter(|gate| !gate.passed)
            .map(|gate| format!("{}({} 项)", gate.gate, gate.findings.len()))
            .collect();
        return Err(Adm4Error::blocked(format!(
            "冻结门未全部通过：{}",
            blocked.join(", ")
        )));
    }
    let state = engine.state();
    // 多选点主选落在首位（产物里字面可见主次），单选点等价于原样克隆。
    let decisions: Vec<Selection> = state
        .selections
        .values()
        .map(Selection::with_primary_first)
        .collect();
    let not_applicable: Vec<(DecisionId, NaJustification)> = state
        .not_applicable
        .iter()
        .map(|(id, justification)| (id.clone(), justification.clone()))
        .collect();
    // custom 合成点进哈希载荷（冻结内容真相必须覆盖 custom 机制的全部结构），
    // 但**不进 FrozenDesign 结构**：合成点由门面层落盘为 frozen/v{N}/custom_points.json，
    // 流水线运行前据此增广设计空间——pipeline crate 因此零改动、零特殊分支。
    // 无 custom 时哈希载荷不带该键，产物与扩展前逐字节一致（金样零漂移）。
    let custom_points = engine.custom_points();
    let payload = if custom_points.is_empty() {
        serde_json::json!({
            "project_name": state.project_name,
            "decisions": decisions,
            "not_applicable": not_applicable,
            "genre_pack": state.genre_pack,
            "pack_version": state.pack_version,
            "depth_profile": state.depth_profile,
        })
    } else {
        serde_json::json!({
            "project_name": state.project_name,
            "decisions": decisions,
            "not_applicable": not_applicable,
            "genre_pack": state.genre_pack,
            "pack_version": state.pack_version,
            "depth_profile": state.depth_profile,
            "custom_points": custom_points,
        })
    };
    let content_hash = ContentHash::of_canonical_json(&payload)?.0;
    let red_team_proof = state.red_team.as_ref().map(|record| record.proof.clone());
    let frozen = FrozenDesign {
        version: state.frozen_versions + 1,
        project_name: state.project_name.clone(),
        decisions,
        not_applicable,
        genre_pack: state.genre_pack.clone(),
        pack_version: state.pack_version.clone(),
        depth_profile: state.depth_profile,
        content_hash,
        frozen_at: UtcTimestamp::now().to_iso8601(),
        gate_report: report,
        red_team_proof,
    };
    engine.mark_frozen();
    Ok(frozen)
}

// ---------------------------------------------------------------------------
// 红队评审执行（AI 必需；失败 = Err，无兜底）
// ---------------------------------------------------------------------------

/// 运行 AI 红队评审并记录到创作状态。产出必须可解析且带工作量证明，否则 Err。
pub fn run_red_team(
    engine: &mut AuthoringEngine,
    provider: &dyn AiProvider,
) -> Adm4Result<RedTeamRecord> {
    let state = engine.state();
    let upstream_count = state.selections.len();
    let design_dump = serde_json::to_string_pretty(&state.selections)
        .map_err(|error| Adm4Error::internal(format!("serialize selections failed: {error}")))?;
    let request = AiRequest {
        purpose: "freeze_red_team".into(),
        system_prompt: "你是对抗性设计评审员。逐条检查给出的全部决策，找出矛盾、不可实现点、\
                        体验断裂。输出 JSON：{\"findings\":[{\"id\":...,\"severity\":\"blocker|warning\",\
                        \"target\":\"决策点id\",\"text\":...}],\"per_category\":[{\"category\":...,\
                        \"checked\":...,\"conclusion\":...}]}。id 与 target 每条必填（缺一即拒收）；\
                        severity=blocker 的 text 必须写明理由，不得留空。\
                        零发现也必须填 per_category 说明查了什么。"
            .into(),
        user_prompt: format!("共 {upstream_count} 条决策：\n{design_dump}"),
        expect_json: true,
    };
    let response = provider.invoke(&request)?;
    let value: serde_json::Value = serde_json::from_str(response.text.trim())
        .map_err(|error| Adm4Error::validation(format!("红队产出不是合法 JSON：{error}")))?;
    let findings = parse_findings(&value)?;
    let per_category: Vec<CategoryEvidence> = value
        .get("per_category")
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| CategoryEvidence {
                    category: item
                        .get("category")
                        .and_then(|category| category.as_str())
                        .unwrap_or("")
                        .to_string(),
                    checked: item
                        .get("checked")
                        .and_then(|checked| checked.as_str())
                        .unwrap_or("")
                        .to_string(),
                    conclusion: item
                        .get("conclusion")
                        .and_then(|conclusion| conclusion.as_str())
                        .unwrap_or("")
                        .to_string(),
                    evidence: Vec::new(),
                })
                .collect()
        })
        .unwrap_or_default();
    let proof = ReviewProof {
        reviewer: format!("{}:{}", response.provider_id, response.model),
        reviewed_count: upstream_count,
        upstream_count,
        content_hash: sha256_hex(response.text.as_bytes()),
        per_category_evidence: per_category,
    };
    proof.verify()?;
    let record = RedTeamRecord {
        findings,
        proof,
        reviewed_revision: engine.state().revision,
    };
    engine.record_red_team(record.clone());
    Ok(record)
}

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

/// 解析红队发现清单（R2 未知即停）。
///
/// `findings` 键缺失 → Err：无法区分「AI 没查」与「查完零发现」，默认零发现会让
/// 冻结门第 4 道形同虚设；AI 确认无发现时必须显式输出空数组。
/// 单条发现的 `severity` 缺失或不在枚举内 → Err：默认成 warning 会把 blocker
/// 静默降级，绕过「blocker 未处置不得冻结」的硬门。
/// `id`/`target` 缺失 → Err：定位不到设计位置的发现无法逐条处置，门 4 会变成走过场。
/// blocker 的 `text` 缺失或空白 → Err：拦下冻结却说不出理由，用户无从修改。
fn parse_findings(value: &serde_json::Value) -> Adm4Result<Vec<Finding>> {
    let items = value
        .get("findings")
        .ok_or_else(|| {
            Adm4Error::validation(
                "红队产出缺少 findings 键（R2：缺输入即停；确认零发现请显式输出 \"findings\": []）",
            )
        })?
        .as_array()
        .ok_or_else(|| Adm4Error::validation("红队产出的 findings 必须是数组"))?;
    let mut findings = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let severity = item
            .get("severity")
            .and_then(|severity| severity.as_str())
            .map(str::trim)
            .filter(|severity| FINDING_SEVERITIES.contains(severity))
            .ok_or_else(|| {
                Adm4Error::validation(format!(
                    "红队第 {} 条发现的 severity 缺失或非法（只接受 {}；禁止默认降级为 warning）",
                    index + 1,
                    FINDING_SEVERITIES.join("|")
                ))
            })?;
        let id = required_field(item, "id", index)?;
        let target = required_field(item, "target", index)?;
        let text = item
            .get("text")
            .and_then(|text| text.as_str())
            .map(str::trim)
            .unwrap_or("");
        if severity == "blocker" && text.is_empty() {
            return Err(Adm4Error::validation(format!(
                "红队第 {} 条发现（{id}）是 blocker 但缺少 text：blocker 会拦下冻结，必须给出可读理由",
                index + 1
            )));
        }
        findings.push(Finding {
            id,
            severity: severity.to_string(),
            target,
            text: text.to_string(),
            disposition: None,
            disposition_note: String::new(),
        });
    }
    Ok(findings)
}

/// 取单条发现的必填定位字段；缺失或全空白即拒收（无法定位的发现不可处置）。
fn required_field(item: &serde_json::Value, key: &str, index: usize) -> Adm4Result<String> {
    item.get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            Adm4Error::validation(format!(
                "红队第 {} 条发现缺少 {key}：无法定位到具体设计位置的发现不可逐条处置",
                index + 1
            ))
        })
}

/// 从参数中提取皮字段的值（标量键 / 表列：全行拼接）。
fn extract_field(parameters: &ParameterValues, field: &str) -> Option<String> {
    match parameters {
        ParameterValues::Scalars { entries } => entries.get(field).map(TypedValue::render),
        ParameterValues::Rows { rows } => {
            let joined: Vec<String> = rows
                .iter()
                .filter_map(|row| row.get(field).map(TypedValue::render))
                .collect();
            if joined.is_empty() {
                None
            } else {
                Some(joined.join("|"))
            }
        }
        _ => None,
    }
}

fn collect_text_values(parameters: &ParameterValues) -> Vec<String> {
    match parameters {
        ParameterValues::Scalars { entries } => entries
            .values()
            .filter_map(|value| match value {
                TypedValue::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        ParameterValues::Rows { rows } => rows
            .iter()
            .flat_map(|row| row.values())
            .filter_map(|value| match value {
                TypedValue::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_findings_key_is_rejected() {
        let value = serde_json::json!({ "per_category": [] });
        let error = parse_findings(&value).unwrap_err();
        assert!(error.message.contains("findings"), "{}", error.message);
    }

    #[test]
    fn explicit_empty_findings_is_accepted() {
        let value = serde_json::json!({ "findings": [] });
        assert!(parse_findings(&value).unwrap().is_empty());
    }

    #[test]
    fn missing_or_unknown_severity_is_rejected() {
        let missing = serde_json::json!({ "findings": [{ "id": "f1", "target": "ld.income_rule", "text": "体验断裂" }] });
        assert!(
            parse_findings(&missing)
                .unwrap_err()
                .message
                .contains("severity")
        );
        let unknown = serde_json::json!({ "findings": [{ "id": "f1", "severity": "note", "target": "ld.income_rule", "text": "体验断裂" }] });
        assert!(
            parse_findings(&unknown)
                .unwrap_err()
                .message
                .contains("severity")
        );
    }

    #[test]
    fn blocker_severity_survives_parsing() {
        let value = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "blocker", "target": "ld.counter_damage", "text": "规则矛盾" }]
        });
        let findings = parse_findings(&value).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, "blocker");
        assert_eq!(findings[0].target, "ld.counter_damage");
        assert!(findings[0].disposition.is_none());
    }

    #[test]
    fn missing_id_or_target_is_rejected_for_any_severity() {
        let no_id = serde_json::json!({
            "findings": [{ "severity": "warning", "target": "ld.income_rule", "text": "节奏偏慢" }]
        });
        assert!(parse_findings(&no_id).unwrap_err().message.contains("id"));
        let no_target = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "warning", "text": "节奏偏慢" }]
        });
        assert!(
            parse_findings(&no_target)
                .unwrap_err()
                .message
                .contains("target")
        );
        let blank_id = serde_json::json!({
            "findings": [{ "id": "  ", "severity": "blocker", "target": "ld.income_rule", "text": "矛盾" }]
        });
        assert!(
            parse_findings(&blank_id)
                .unwrap_err()
                .message
                .contains("id")
        );
    }

    #[test]
    fn blocker_without_readable_text_is_rejected() {
        let missing = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "blocker", "target": "ld.counter_damage" }]
        });
        let error = parse_findings(&missing).unwrap_err();
        assert!(error.message.contains("text"), "{}", error.message);
        let blank = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "blocker", "target": "ld.counter_damage", "text": " \t " }]
        });
        assert!(parse_findings(&blank).unwrap_err().message.contains("text"));
    }

    #[test]
    fn warning_may_omit_text() {
        let value = serde_json::json!({
            "findings": [{ "id": "w1", "severity": "warning", "target": "ld.income_rule" }]
        });
        let findings = parse_findings(&value).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].text.is_empty());
        assert_eq!(findings[0].target, "ld.income_rule");
    }
}
