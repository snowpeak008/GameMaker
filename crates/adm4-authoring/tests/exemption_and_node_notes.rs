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
        tier_gate: None,
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
            profile_points: Vec::new(),
            cardinality_expectations: Default::default(),
            consistency_rules: Vec::new(),
            nodes: Vec::new(),
            decision_points: Vec::new(),
            system_refs: Vec::new(),
            core_nouns: Vec::new(),
        },
        graph: match DecisionGraph::new(vec![
            point("u.vision", DesignLevel::L0, Some("vision")),
            point("u.scope", DesignLevel::L1, Some("vision")),
        ]) {
            Ok(graph) => graph,
            Err(error) => panic!("测试图构造失败：{}", error.message),
        },
        organization,
        system_instances: Vec::new(),
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
    // F3：署名已并入 NaJustification（此前是 AuthoringState.na_signoffs 并行 map），
    // 因此适用性里的 NotApplicable 载荷同时带理由码、说明与署名。
    let applicability = engine.applicability();
    let Some(PointApplicability::NotApplicable(justification)) = applicability.get("u.scope")
    else {
        panic!("u.scope 应被判为不适用：{:?}", applicability.get("u.scope"));
    };
    assert_eq!(justification.reason_code, "out_of_scope");
    assert_eq!(justification.note, "本期不做范围扩展");
    assert_eq!(justification.actor, "张三");
    assert!(!justification.at.is_empty());
    assert!(justification.is_signed());
    let exempted = engine.completeness();
    assert_eq!(
        (exempted.done, exempted.total),
        (1, 1),
        "豁免点必须离开分母"
    );
    assert!(exempted.is_complete());
    // 在案：理由码计数 + 署名。
    assert_eq!(exempted.na_reason_counts.get("out_of_scope"), Some(&1));
    // 并行 map 已淘汰：新写入不再往 na_signoffs 落任何东西。
    assert!(engine.state().na_signoffs.is_empty());

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
    assert!(
        engine.state().not_applicable.is_empty(),
        "署名随豁免一并清除"
    );
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

/// F3 顺带项：`NaJustification` 吸收 actor/at 后，F3 之前的存档（署名在
/// `na_signoffs` 并行 map 里）必须仍可读，且署名不丢——读入后就地合并进 not_applicable。
#[test]
fn legacy_archive_signoff_map_is_adopted_on_load() {
    let legacy = r#"{
      "project_name": "旧存档",
      "genre_pack": "na_test",
      "pack_version": "0.1.0",
      "depth_profile": { "target": "L4" },
      "selections": {},
      "not_applicable": {
        "u.scope": { "reason_code": "out_of_scope", "note": "本期不做" },
        "u.vision": { "reason_code": "no_persistence", "note": "理由码跳过" }
      },
      "na_signoffs": {
        "u.scope": { "actor": "王五", "at": "2026-01-01T00:00:00Z" },
        "u.ghost": { "actor": "幽灵", "at": "2026-01-01T00:00:00Z" }
      }
    }"#;
    let mut state: AuthoringState =
        serde_json::from_str(legacy).expect("F3 之前的存档必须仍可反序列化");
    assert_eq!(state.na_signoffs.len(), 2);
    assert_eq!(state.adopt_legacy_na_signoffs(), 1, "只合并仍在案的豁免");
    assert!(state.na_signoffs.is_empty(), "并行 map 合并后清空");

    let signed = state.not_applicable.get("u.scope").expect("豁免应在案");
    assert_eq!(signed.actor, "王五");
    assert_eq!(signed.at, "2026-01-01T00:00:00Z");
    assert!(signed.is_signed());
    // 理由码跳过条目没有署名，照实标注（不编造署名）。
    let unsigned = state.not_applicable.get("u.vision").expect("跳过应在案");
    assert!(!unsigned.is_signed());
    assert!(unsigned.signature_label().contains("无署名"));
    // 幽灵署名（豁免早已解除）被丢弃，不复活成新记录。
    assert!(!state.not_applicable.contains_key("u.ghost"));

    // 回写后不再包含 na_signoffs 键（新存档格式只有一个真相源）。
    let json = serde_json::to_string(&state).expect("序列化");
    assert!(!json.contains("na_signoffs"), "{json}");
    assert!(json.contains(r#""actor":"王五""#), "{json}");

    // 引擎装配路径同样自动合并（上层不必知道曾有过并行 map）。
    let with_legacy: AuthoringState = serde_json::from_str(legacy).expect("反序列化");
    let space = engine().space().clone();
    let assembled = AuthoringEngine::new(space, with_legacy).expect("装配");
    assert!(assembled.state().na_signoffs.is_empty());
    assert_eq!(
        assembled
            .state()
            .not_applicable
            .get("u.scope")
            .map(|item| item.actor.as_str()),
        Some("王五")
    );
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

/// 工作台重置（F4b-B）：清空创作态并逐项计数；署名与理由双必填（R3）；
/// 已冻结版本计数与项目身份原样保留。
#[test]
fn workbench_reset_clears_authoring_state_and_counts_what_it_cleared() {
    let mut engine = engine();
    engine
        .select_option("u.vision", "a", Provenance::UserManual)
        .unwrap();
    engine
        .set_parameters("u.vision", adm4_decision::ParameterValues::None)
        .unwrap();
    engine.confirm_selection("u.vision").unwrap();
    engine
        .set_not_applicable("u.scope", "out_of_scope", "本期不做", "张三")
        .unwrap();
    engine
        .set_node_design_note("vision", "愿景以守护叙事为主")
        .unwrap();
    engine
        .set_node_risk_note("vision", "叙事资源投入尚未评估")
        .unwrap();
    engine.mark_frozen();
    let revision_before = engine.state().revision;

    // R3：署名或理由缺一，重置一律不执行（数据一字不动）。
    for (actor, note) in [("   ", "返工"), ("张三", "\t ")] {
        let error = engine
            .reset_workbench(actor, note)
            .expect_err("缺署名或缺理由必须被拒");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::InvalidInput);
    }
    assert_eq!(engine.state().selections.len(), 1, "被拒的重置不许动数据");
    assert_eq!(engine.state().not_applicable.len(), 1);

    let report = engine
        .reset_workbench(" 张三 ", " 方向推翻，创作重来 ")
        .expect("署名与理由齐备应通过");
    assert_eq!(report.actor, "张三");
    assert!(!report.at.is_empty());
    assert_eq!(report.cleared_selections, 1);
    assert_eq!(report.cleared_exemptions, 1);
    assert_eq!(report.cleared_node_design_notes, 1);
    assert_eq!(report.cleared_node_risk_notes, 1);
    assert!(!report.is_noop());
    assert!(
        report.summary().contains("1 个决策点选择"),
        "{}",
        report.summary()
    );

    let state = engine.state();
    assert!(state.selections.is_empty());
    assert!(state.not_applicable.is_empty());
    assert!(state.node_design_notes.is_empty());
    assert!(state.node_risk_notes.is_empty());
    assert_eq!(state.project_name, "豁免项目", "项目身份不变");
    assert_eq!(
        state.frozen_versions, 1,
        "已冻结版本是只增不改的历史，重置不得抹掉（D4）"
    );
    assert!(state.revision > revision_before, "重置也是一次变更");
    let completeness = engine.completeness();
    assert_eq!(completeness.done, 0, "回到一个都没答");
    assert_eq!(
        completeness.total, 2,
        "分母仍是两个适用点：豁免被清除后 u.scope 重回分母，重置不改变分母口径"
    );

    // 幂等：再重置一次什么都没有可清。
    let again = engine
        .reset_workbench("张三", "确认已清空")
        .expect("重复重置合法");
    assert!(again.is_noop());
}
