//! 人工豁免（N/A）进出与节点文本的行为验收。
//!
//! 覆盖：豁免必须带理由码 + 说明 + 署名（R3）；豁免点移出完成度分母但在案；
//! 冻结门第 1 道逐条列出豁免（不拦截）；解除豁免后该点恢复正常适用性。

use adm4_authoring::{AuthoringEngine, AuthoringState, evaluate_freeze_gates};
use adm4_contracts::SkinScanner;
use adm4_decision::{
    DecisionGraph, DecisionOption, DecisionPoint, DepthProfile, DesignDomain, DesignLevel,
    DesignNode, DesignOrganization, GenreScope, PointApplicability, PointRequirement, Provenance,
    SelectionMode,
};
use adm4_space::{DesignSpace, GenrePack};

fn option(id: &str) -> DecisionOption {
    DecisionOption {
        id: id.into(),
        label: id.into(),
        ..Default::default()
    }
}

fn point(id: &str, level: DesignLevel, node_id: Option<&str>) -> DecisionPoint {
    DecisionPoint {
        id: id.into(),
        domain: "core".into(),
        level,
        genre_scope: GenreScope::Universal,
        question: format!("{id}？"),
        mda_layer: None,
        design_question: None,
        node_id: node_id.map(Into::into),
        selection_mode: SelectionMode::Single,
        requirement: PointRequirement::Unlocked,
        options: vec![option("a"), option("b")],
        skin_fields: Vec::new(),
        evidence_slots: false,
    }
}

fn engine() -> AuthoringEngine {
    let organization = DesignOrganization::new(
        vec![DesignDomain {
            id: "positioning".into(),
            name: "立项与产品定位设计".into(),
            description: String::new(),
            order: 1,
        }],
        vec![DesignNode {
            id: "vision".into(),
            domain_id: "positioning".into(),
            name: "项目愿景".into(),
            description: String::new(),
            role_class: "strategic".into(),
        }],
    );
    let space = DesignSpace {
        universal_version: "test".into(),
        pack: GenrePack {
            pack_id: "na_test".into(),
            pack_version: "0.1.0".into(),
            display_name: "豁免测试包".into(),
            reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
            cardinality_expectations: Default::default(),
            consistency_rules: Vec::new(),
            nodes: Vec::new(),
            decision_points: Vec::new(),
        },
        graph: match DecisionGraph::new(vec![
            point("u.vision", DesignLevel::L0, Some("vision")),
            point("u.scope", DesignLevel::L1, Some("vision")),
        ]) {
            Ok(graph) => graph,
            Err(error) => panic!("测试图构造失败：{}", error.message),
        },
        organization,
    };
    let state = AuthoringState::new(
        "豁免项目",
        "na_test",
        "0.1.0",
        DepthProfile::new(DesignLevel::L4).unwrap(),
    );
    AuthoringEngine::new(space, state).unwrap()
}

#[test]
fn exemption_requires_reason_note_and_signature() {
    let mut engine = engine();
    assert!(
        engine
            .set_not_applicable("u.scope", "  ", "说明", "张三")
            .is_err(),
        "空理由码必须拒收"
    );
    assert!(
        engine
            .set_not_applicable("u.scope", "out_of_scope", "   ", "张三")
            .is_err(),
        "空说明必须拒收"
    );
    assert!(
        engine
            .set_not_applicable("u.scope", "out_of_scope", "本期不做多人", "  ")
            .is_err(),
        "无署名必须拒收（R3）"
    );
    assert!(
        engine
            .set_not_applicable("ghost.point", "out_of_scope", "说明", "张三")
            .is_err(),
        "清单外的决策点不能豁免"
    );
    // 三者齐备才落库，且没有任何一次失败尝试留下痕迹。
    assert!(engine.state().not_applicable.is_empty());
    assert!(engine.state().na_signoffs.is_empty());
}

#[test]
fn exemption_leaves_and_reenters_the_denominator() {
    let mut engine = engine();
    engine
        .select_option("u.vision", "a", Provenance::UserManual)
        .unwrap();
    engine.confirm_selection("u.vision").unwrap();

    // 两个适用点，一个已确认 → 1/2。
    let before = engine.completeness();
    assert_eq!((before.done, before.total), (1, 2));

    // 人工豁免 u.scope（普通适用点，不是 baseline）。
    engine
        .set_not_applicable("u.scope", "out_of_scope", "本期不做范围扩展", "张三")
        .unwrap();
    assert_eq!(
        engine.applicability().get("u.scope"),
        Some(&PointApplicability::NotApplicable(
            adm4_decision::NaJustification {
                reason_code: "out_of_scope".into(),
                note: "本期不做范围扩展".into(),
            }
        ))
    );
    let exempted = engine.completeness();
    assert_eq!(
        (exempted.done, exempted.total),
        (1, 1),
        "豁免点必须离开分母"
    );
    assert!(exempted.is_complete());
    // 在案：理由码计数 + 署名。
    assert_eq!(exempted.na_reason_counts.get("out_of_scope"), Some(&1));
    let signoff = engine
        .state()
        .na_signoffs
        .get("u.scope")
        .expect("署名应落库");
    assert_eq!(signoff.actor, "张三");
    assert!(!signoff.at.is_empty());

    // 领域聚合：豁免点单列，不进分母。
    let progress = engine.organization_progress();
    let domain = progress.domain("positioning").expect("领域应有决策点");
    assert_eq!(domain.counts.applicable, 1);
    assert_eq!(domain.counts.not_applicable, 1);
    assert_eq!(domain.counts.total_points, 2);

    // 冻结门第 1 道：豁免逐条可见但不拦截。
    let scanner = SkinScanner::new(engine.space().skin_words());
    let report = evaluate_freeze_gates(&engine, &scanner);
    let gate1 = report
        .gates
        .iter()
        .find(|gate| gate.gate == "gate1_completeness")
        .expect("门 1 应存在");
    assert!(gate1.passed, "豁免不该拦截冻结：{:?}", gate1.findings);
    let exemption = gate1
        .findings
        .iter()
        .find(|finding| finding.code == "not_applicable_exemption")
        .expect("豁免必须出现在门 1 明细里");
    assert!(exemption.message.contains("u.scope"), "{exemption:?}");
    assert!(exemption.message.contains("out_of_scope"), "{exemption:?}");
    assert!(exemption.message.contains("张三"), "{exemption:?}");

    // 解除豁免 → 恢复正常适用性，重新进分母。
    assert!(engine.clear_not_applicable("u.scope").unwrap());
    assert!(
        !engine.clear_not_applicable("u.scope").unwrap(),
        "重复解除是幂等的"
    );
    let restored = engine.completeness();
    assert_eq!((restored.done, restored.total), (1, 2));
    assert!(engine.state().na_signoffs.is_empty());
    assert!(
        restored
            .blocking
            .iter()
            .any(|item| item.decision_id == "u.scope")
    );
}

#[test]
fn selecting_an_option_clears_the_exemption_and_its_signature() {
    let mut engine = engine();
    engine
        .set_not_applicable("u.scope", "out_of_scope", "本期不做", "李四")
        .unwrap();
    engine
        .select_option("u.scope", "a", Provenance::UserManual)
        .unwrap();
    assert!(engine.state().not_applicable.is_empty());
    assert!(engine.state().na_signoffs.is_empty());
}

#[test]
fn node_notes_are_keyed_by_declared_nodes_only() {
    let mut engine = engine();
    assert!(
        engine.set_node_risk_note("ghost_node", "风险").is_err(),
        "未声明的节点不能挂文本"
    );
    engine
        .set_node_design_note("vision", "愿景以守护叙事为主")
        .unwrap();
    engine
        .set_node_risk_note("vision", "叙事资源投入尚未评估")
        .unwrap();
    assert_eq!(
        engine
            .state()
            .node_design_notes
            .get("vision")
            .map(String::as_str),
        Some("愿景以守护叙事为主")
    );
    assert_eq!(
        engine
            .state()
            .node_risk_notes
            .get("vision")
            .map(String::as_str),
        Some("叙事资源投入尚未评估")
    );
    // 空串 = 清除该条（不留空字符串垃圾）。
    engine.set_node_risk_note("vision", "  ").unwrap();
    assert!(engine.state().node_risk_notes.is_empty());
}
