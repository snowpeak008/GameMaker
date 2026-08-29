//! F3 模型缺口的行为验收（不含 UI/门面）：
//!
//! 1. `PointRequirement::Optional`：未作答不进完成度分母、不构成冻结门 1 阻塞项，
//!    也不进访谈待办；作答后与普通点一视同仁（未确认照样拦）。
//! 2. 通用层模板跨包预填：`genre_pack=universal` 的模板可预填到任何包的项目；
//!    答卷里引用装配空间之外的决策点/选项时逐条跳过并计数（R2：禁止静默丢弃）。
//! 3. 多选答卷预填：附加选项与主选按模板写入项目。
//! 4. 项目重命名：空白名称被拒。

use adm4_authoring::{AuthoringEngine, AuthoringState, evaluate_freeze_gates};
use adm4_contracts::SkinScanner;
use adm4_decision::{
    DecisionGraph, DecisionOption, DecisionPoint, DepthProfile, DesignLevel, DesignOrganization,
    GenreScope, PointRequirement, Provenance, SelectionMode,
};
use adm4_space::{DesignSpace, GenrePack};
use adm4_template::{
    Certification, CertificationStatus, Confidence, Evidence, SourceType, Template, TemplateAnswer,
    TemplateSelectedOption,
};

fn option(id: &str) -> DecisionOption {
    DecisionOption {
        id: id.into(),
        label: id.into(),
        ..Default::default()
    }
}

fn point(id: &str, level: DesignLevel, options: Vec<DecisionOption>) -> DecisionPoint {
    DecisionPoint {
        id: id.into(),
        domain: "core".into(),
        level,
        genre_scope: GenreScope::Universal,
        question: format!("{id}？"),
        mda_layer: None,
        design_question: None,
        node_id: None,
        selection_mode: SelectionMode::Single,
        requirement: PointRequirement::Unlocked,
        options,
        skin_fields: Vec::new(),
        evidence_slots: false,
    }
}

/// 测试空间：一个必做点 + 一个非必做点 + 一个 multi/allow_primary 点。
fn engine(pack_id: &str) -> AuthoringEngine {
    let mut optional = point(
        "u.dimension",
        DesignLevel::L0,
        vec![option("two_d"), option("three_d")],
    );
    optional.requirement = PointRequirement::Optional;
    let mut multi = point(
        "u.systems",
        DesignLevel::L2,
        vec![option("combat"), option("economy"), option("social")],
    );
    multi.selection_mode = SelectionMode::Multi {
        allow_primary: true,
    };
    let space = DesignSpace {
        universal_version: "test".into(),
        pack: GenrePack {
            pack_id: pack_id.into(),
            pack_version: "0.1.0".into(),
            display_name: "F3 测试包".into(),
            reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
            cardinality_expectations: Default::default(),
            consistency_rules: Vec::new(),
            nodes: Vec::new(),
            decision_points: Vec::new(),
        },
        graph: match DecisionGraph::new(vec![
            point(
                "u.platform",
                DesignLevel::L0,
                vec![option("pc_single"), option("mobile")],
            ),
            optional,
            multi,
        ]) {
            Ok(graph) => graph,
            Err(error) => panic!("测试图构造失败：{}", error.message),
        },
        organization: DesignOrganization::default(),
    };
    let state = AuthoringState::new(
        "F3 项目",
        pack_id,
        "0.1.0",
        DepthProfile::new(DesignLevel::L4).unwrap(),
    );
    match AuthoringEngine::new(space, state) {
        Ok(engine) => engine,
        Err(error) => panic!("引擎装配失败：{}", error.message),
    }
}

fn evidence() -> Vec<Evidence> {
    vec![Evidence {
        source_url: "adm4://v2-builtin/fixture.json".into(),
        quote: String::new(),
        source_type: SourceType::Inference,
        confidence: Confidence::Low,
    }]
}

fn answer(decision_id: &str, option_id: &str) -> TemplateAnswer {
    TemplateAnswer {
        decision_id: decision_id.into(),
        option_id: option_id.into(),
        parameters: Default::default(),
        evidence: evidence(),
        notes: String::new(),
        crosscheck_agreed: None,
        additional_options: Vec::new(),
        primary_option: None,
    }
}

fn universal_template(answers: Vec<TemplateAnswer>) -> Template {
    Template {
        template_id: "builtin_fixture".into(),
        game_name: "虚构通用甲".into(),
        aliases: Vec::new(),
        genre_pack: "universal".into(),
        pack_version: "0.1.0".into(),
        depth_reached: DesignLevel::L4,
        answers,
        certification: Certification {
            status: CertificationStatus::Certified,
            reviewed_by: "批量迁移".into(),
            reviewed_at: "2026-08-29T00:00:00Z".into(),
            review_note: "fixture".into(),
        },
        mapping_hash: String::new(),
        crosscheck_proof: None,
    }
}

// ---------------------------------------------------------------------------
// 1. 非必做点
// ---------------------------------------------------------------------------

#[test]
fn optional_point_stays_out_of_gate1_denominator_until_answered() {
    let mut engine = engine("f3_test");
    for (decision, option_id) in [("u.platform", "pc_single"), ("u.systems", "combat")] {
        engine
            .select_option(decision, option_id, Provenance::UserManual)
            .unwrap();
    }
    engine.set_primary_option("u.systems", "combat").unwrap();
    engine.confirm_selection("u.platform").unwrap();
    engine.confirm_selection("u.systems").unwrap();

    // 非必做点未作答：分母 2（不含它），完成度满，门 1 通过。
    let report = engine.completeness();
    assert_eq!((report.done, report.total), (2, 2));
    assert_eq!(report.optional_skipped, 1);
    assert!(report.is_complete(), "{:?}", report.blocking);

    // 也不进访谈/UI 待办（不是欠着的活）。
    let pending = engine.pending_decisions().unwrap();
    assert!(
        !pending.iter().any(|id| id == "u.dimension"),
        "非必做点不应进待办：{pending:?}"
    );

    let scanner = SkinScanner::new(engine.space().skin_words());
    let gates = evaluate_freeze_gates(&engine, &scanner);
    let gate1 = gates
        .gates
        .iter()
        .find(|gate| gate.gate == "gate1_completeness")
        .expect("门 1 应存在");
    assert!(
        gate1.passed,
        "非必做点未作答不得拦冻结：{:?}",
        gate1.findings
    );
    // 但必须数得出来（否则「100%」会掩盖「有点根本没看」）。
    assert_eq!(gates.optional_skipped, 1);
    let visible = gate1
        .findings
        .iter()
        .find(|finding| finding.code == "optional_not_answered")
        .expect("非必做未作答必须在门 1 明细里可见");
    assert!(visible.message.contains('1'), "{visible:?}");

    // 一旦作答：进分母，未确认照常拦（作答即纳入设计）。
    engine
        .select_option("u.dimension", "two_d", Provenance::AiInterviewConfirmed)
        .unwrap();
    let answered = engine.completeness();
    assert_eq!((answered.done, answered.total), (2, 3));
    assert_eq!(answered.optional_skipped, 0);
    assert!(
        answered
            .blocking
            .iter()
            .any(|item| item.decision_id == "u.dimension"),
        "{:?}",
        answered.blocking
    );
    let gate1_after = evaluate_freeze_gates(&engine, &scanner);
    assert!(
        !gate1_after
            .gates
            .iter()
            .find(|gate| gate.gate == "gate1_completeness")
            .expect("门 1 应存在")
            .passed
    );

    // 确认后恢复全绿。
    engine.confirm_selection("u.dimension").unwrap();
    let confirmed = engine.completeness();
    assert_eq!((confirmed.done, confirmed.total), (3, 3));
    assert!(confirmed.is_complete(), "{:?}", confirmed.blocking);
}

// ---------------------------------------------------------------------------
// 2 + 3. 通用层模板跨包预填、跳过计数、多选写入
// ---------------------------------------------------------------------------

#[test]
fn universal_template_prefills_any_pack_and_counts_every_skip() {
    let mut engine = engine("f3_test");
    let mut multi_answer = answer("u.systems", "combat");
    multi_answer.additional_options = vec![
        TemplateSelectedOption {
            option_id: "economy".into(),
            parameters: Default::default(),
        },
        // 选项不存在 → 跳过并计数。
        TemplateSelectedOption {
            option_id: "ghost_system".into(),
            parameters: Default::default(),
        },
    ];
    multi_answer.primary_option = Some("economy".into());

    // 单选点被塞附加选项 → 附加项跳过，首选项照常写入。
    let mut single_with_extras = answer("u.platform", "pc_single");
    single_with_extras.additional_options = vec![TemplateSelectedOption {
        option_id: "mobile".into(),
        parameters: Default::default(),
    }];

    let template = universal_template(vec![
        single_with_extras,
        multi_answer,
        // 决策点不在装配空间内（品类专属点/旧清单条目）→ 跳过并计数。
        answer("ld.wave_system", "scripted_waves"),
        // 选项不在选项集内 → 跳过并计数。
        answer("u.dimension", "four_d"),
    ]);

    let report = engine.prefill_from_template(&template).unwrap();
    assert_eq!(report.applied, 2, "两条可用答案应写入");
    assert_eq!(report.multi_options_applied, 1, "只有 economy 是合法附加项");
    assert_eq!(report.skipped_count(), 4);
    let reasons: Vec<(&str, &str)> = report
        .skipped
        .iter()
        .map(|skip| (skip.decision_id.as_str(), skip.option_id.as_str()))
        .collect();
    assert!(reasons.contains(&("u.platform", "mobile")), "{reasons:?}");
    assert!(
        reasons.contains(&("u.systems", "ghost_system")),
        "{reasons:?}"
    );
    assert!(
        reasons.contains(&("ld.wave_system", "scripted_waves")),
        "{reasons:?}"
    );
    assert!(reasons.contains(&("u.dimension", "four_d")), "{reasons:?}");
    assert!(
        report.summary().contains("跳过 4 条"),
        "{}",
        report.summary()
    );

    // 多选点：全部合法已选选项 + 主选按模板写入，且主选排最前。
    let selection = engine
        .state()
        .selections
        .get("u.systems")
        .expect("多选点应已写入");
    assert_eq!(selection.selected_count(), 2);
    assert_eq!(selection.primary_option.as_deref(), Some("economy"));
    assert_eq!(selection.selected_option_ids(), vec!["economy", "combat"]);
    // 预填 = 未确认 + provenance=Template（逐条确认与换皮门照旧生效）。
    assert!(!selection.confirmed_by_user);
    assert!(matches!(selection.provenance, Provenance::Template { .. }));

    // 单选点只写首选项，附加项没有偷偷落进去。
    let single = engine
        .state()
        .selections
        .get("u.platform")
        .expect("单选点应已写入");
    assert_eq!(single.selected_count(), 1);
    assert!(single.primary_option.is_none());

    // 预填后完成度：两个已写入点待确认，非必做点仍未作答（模板那条被跳过了）。
    let completeness = engine.completeness();
    assert_eq!(completeness.optional_skipped, 1);
    assert_eq!(completeness.total, 2);
}

#[test]
fn non_universal_template_from_other_pack_is_still_rejected() {
    let mut engine = engine("f3_test");
    let mut foreign = universal_template(vec![answer("u.platform", "pc_single")]);
    foreign.genre_pack = "other_pack".into();
    let error = engine.prefill_from_template(&foreign).unwrap_err();
    assert!(error.message.contains("universal"), "{}", error.message);
    assert!(engine.state().selections.is_empty());
}

#[test]
fn uncertified_universal_template_is_rejected_at_the_engine_too() {
    let mut engine = engine("f3_test");
    let mut draft = universal_template(vec![answer("u.platform", "pc_single")]);
    draft.certification.status = CertificationStatus::Draft;
    assert!(engine.prefill_from_template(&draft).is_err());
    assert!(engine.state().selections.is_empty());
}

#[test]
fn template_primary_outside_written_set_is_skipped_not_stored() {
    let mut engine = engine("f3_test");
    let mut answer_with_bad_primary = answer("u.systems", "combat");
    answer_with_bad_primary.primary_option = Some("social".into());
    let report = engine
        .prefill_from_template(&universal_template(vec![answer_with_bad_primary]))
        .unwrap();
    assert_eq!(report.applied, 1);
    assert_eq!(report.skipped_count(), 1);
    assert!(
        report.skipped[0].reason.contains("主选"),
        "{:?}",
        report.skipped
    );
    let selection = engine.state().selections.get("u.systems").unwrap();
    assert!(
        selection.primary_option.is_none(),
        "越界主选不得落库（否则完成度门会永久拦住这个点）"
    );
}

// ---------------------------------------------------------------------------
// 4. 项目重命名
// ---------------------------------------------------------------------------

#[test]
fn project_rename_trims_and_rejects_blank() {
    let mut engine = engine("f3_test");
    let before = engine.state().revision;
    assert!(engine.set_project_name("   ").is_err());
    assert!(engine.set_project_name("\t\n").is_err());
    assert_eq!(engine.state().project_name, "F3 项目");
    assert_eq!(engine.state().revision, before, "失败尝试不得改动状态");

    engine.set_project_name("  霜落峡谷防卫计划  ").unwrap();
    assert_eq!(engine.state().project_name, "霜落峡谷防卫计划");
    assert_eq!(engine.state().revision, before + 1);
}
