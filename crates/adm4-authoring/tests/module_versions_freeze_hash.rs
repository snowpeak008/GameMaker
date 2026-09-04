//! W7 3a：`module_versions` 进冻结哈希的行为验收。
//!
//! 覆盖：无模块项目的冻结哈希与扩展前算法**逐字节一致**（条件键负测试，金样零漂移）；
//! FrozenDesign 序列化对空表不写 `module_versions` 键（旧档零漂移）；
//! 有模块时同选择同模块两次冻结哈希一致；模块 semver 变则哈希变。

use adm4_ai::ScriptedProvider;
use adm4_authoring::{AuthoringEngine, AuthoringState, FrozenDesign, execute_freeze, run_red_team};
use adm4_contracts::SkinScanner;
use adm4_decision::{
    DecisionGraph, DecisionOption, DecisionPoint, DepthProfile, DesignLevel, DesignOrganization,
    GenreScope, PointRequirement, Provenance, SelectionMode,
};
use adm4_foundation::ContentHash;
use adm4_space::{DesignSpace, GenrePack, SystemInstanceInfo};

const RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"全部决策交叉","conclusion":"未发现矛盾"}]}"#;

fn option(id: &str) -> DecisionOption {
    DecisionOption {
        id: id.into(),
        label: id.into(),
        ..Default::default()
    }
}

fn point(id: &str) -> DecisionPoint {
    DecisionPoint {
        id: id.into(),
        domain: "core".into(),
        level: DesignLevel::L0,
        genre_scope: GenreScope::Universal,
        question: format!("{id}？"),
        mda_layer: None,
        design_question: None,
        node_id: None,
        selection_mode: SelectionMode::Single,
        requirement: PointRequirement::Unlocked,
        tier_gate: None,
        options: vec![option("a"), option("b")],
        skin_fields: Vec::new(),
        evidence_slots: false,
    }
}

/// 最小可冻结空间；`instances` 模拟加载器产出的系统实例信息。
fn space(instances: Vec<SystemInstanceInfo>) -> DesignSpace {
    DesignSpace {
        universal_version: "test".into(),
        pack: GenrePack {
            pack_id: "mv_test".into(),
            pack_version: "0.1.0".into(),
            display_name: "模块版本哈希测试包".into(),
            reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
            profile_points: Vec::new(),
            cardinality_expectations: Default::default(),
            consistency_rules: Vec::new(),
            nodes: Vec::new(),
            decision_points: Vec::new(),
            system_refs: Vec::new(),
            core_nouns: Vec::new(),
        },
        graph: match DecisionGraph::new(vec![point("u.core")]) {
            Ok(graph) => graph,
            Err(error) => panic!("测试图构造失败：{}", error.message),
        },
        organization: DesignOrganization::new(Vec::new(), Vec::new()),
        system_instances: instances,
    }
}

/// 选中确认唯一决策点 + 脚本红队 + 冻结，返回冻结产物。
fn freeze_with(instances: Vec<SystemInstanceInfo>) -> FrozenDesign {
    let state = AuthoringState::new(
        "模块版本项目",
        "mv_test",
        "0.1.0",
        DepthProfile::new(DesignLevel::L4).unwrap(),
    );
    let mut engine = AuthoringEngine::new(space(instances), state).unwrap();
    engine
        .select_option("u.core", "a", Provenance::UserManual)
        .unwrap();
    engine.confirm_selection("u.core").unwrap();
    let provider = ScriptedProvider::new();
    provider.script("freeze_red_team", vec![RED_TEAM_ANSWER.into()]);
    run_red_team(&mut engine, &provider).unwrap();
    let scanner = SkinScanner::new(engine.space().skin_words());
    execute_freeze(&mut engine, &scanner).unwrap()
}

fn instance(instance_id: &str, module_id: &str, semver: &str) -> SystemInstanceInfo {
    SystemInstanceInfo {
        instance_id: instance_id.into(),
        module_id: module_id.into(),
        semver: semver.into(),
    }
}

/// 负测试（金样零漂移）：无模块项目的冻结哈希必须与扩展前的 canonical payload
/// **逐字节一致**——payload 不带 `module_versions` 键（条件键纪律，custom_points 同款）。
#[test]
fn freeze_hash_without_modules_matches_pre_extension_payload_byte_for_byte() {
    let frozen = freeze_with(Vec::new());
    assert!(frozen.module_versions.is_empty());
    // 用扩展前的 payload 形状重算哈希：decisions/not_applicable 取冻结产物里的
    // 同一份数据（execute_freeze 进 payload 的正是它们），六键之外无任何新键。
    let expected_payload = serde_json::json!({
        "project_name": frozen.project_name,
        "decisions": frozen.decisions,
        "not_applicable": frozen.not_applicable,
        "genre_pack": frozen.genre_pack,
        "pack_version": frozen.pack_version,
        "depth_profile": frozen.depth_profile,
    });
    let expected = ContentHash::of_canonical_json(&expected_payload).unwrap().0;
    assert_eq!(
        frozen.content_hash, expected,
        "无模块项目的冻结哈希必须与扩展前算法逐字节一致（module_versions 是条件键）"
    );
}

/// 旧档零漂移：空 `module_versions` 不序列化该键；带模块的产物往返无损。
#[test]
fn frozen_design_serde_omits_empty_module_versions_and_roundtrips() {
    let without = freeze_with(Vec::new());
    let json = serde_json::to_string(&without).unwrap();
    assert!(
        !json.contains("module_versions"),
        "空表必须跳过序列化（旧档与产物零漂移）"
    );
    // 旧档（无该键）可读，反序列化为空表。
    let back: FrozenDesign = serde_json::from_str(&json).unwrap();
    assert!(back.module_versions.is_empty());

    let with = freeze_with(vec![instance("equipment_main", "sys.equipment", "1.0.0")]);
    let json = serde_json::to_string(&with).unwrap();
    assert!(json.contains("module_versions"), "{json}");
    let back: FrozenDesign = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.module_versions
            .get("sys.equipment")
            .map(String::as_str),
        Some("1.0.0")
    );
}

/// 同选择 + 同模块版本 → 两次冻结哈希一致（frozen_at 等时间戳不进哈希）；
/// 模块 semver 变 → 哈希必须变（同选择不同模块版本 = 不同语义）。
#[test]
fn same_modules_hash_stable_and_semver_change_changes_hash() {
    let first = freeze_with(vec![instance("equipment_main", "sys.equipment", "1.0.0")]);
    let second = freeze_with(vec![instance("equipment_main", "sys.equipment", "1.0.0")]);
    assert_eq!(
        first.content_hash, second.content_hash,
        "同选择同模块版本的冻结哈希必须确定性一致"
    );
    assert_eq!(
        first
            .module_versions
            .get("sys.equipment")
            .map(String::as_str),
        Some("1.0.0")
    );

    let bumped = freeze_with(vec![instance("equipment_main", "sys.equipment", "1.1.0")]);
    assert_ne!(
        first.content_hash, bumped.content_hash,
        "模块 semver 变化必须反映进冻结哈希（版本漂移不得静默）"
    );

    // 无模块 vs 有模块：哈希也必须不同（条件键在场即改变载荷）。
    let without = freeze_with(Vec::new());
    assert_ne!(first.content_hash, without.content_hash);
}

/// 同一模块多实例：`module_versions` 是 module_id → semver 的单值映射（登记一次）。
#[test]
fn multiple_instances_of_same_module_register_single_version_entry() {
    let frozen = freeze_with(vec![
        instance("equipment_main", "sys.equipment", "1.0.0"),
        instance("equipment_fashion", "sys.equipment", "1.0.0"),
    ]);
    assert_eq!(frozen.module_versions.len(), 1);
    assert_eq!(
        frozen
            .module_versions
            .get("sys.equipment")
            .map(String::as_str),
        Some("1.0.0")
    );
}
