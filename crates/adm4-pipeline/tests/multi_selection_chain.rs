//! 多选点（含主选）走完「创作 → 冻结门五道 → C0 编译」的正例。
//!
//! 现有两个品类包里没有多选点（T10 迁移二版 L4 选项组后才会有），因此本测试自建
//! 一份最小合成设计空间：领域/节点两级组织 + 多选 L0 画像点 + 多选 L3 系统点
//! （allow_primary）+ 多选 L4 机制点。验收的是「多选点不被任何一环静默跳过」：
//! 完成度算它、冻结门放它过、FrozenDesign 主选在前、GameSpec 每个已选选项都成元素。

use adm4_ai::ScriptedProvider;
use adm4_authoring::{
    AuthoringEngine, AuthoringState, evaluate_freeze_gates, execute_freeze, run_red_team,
};
use adm4_contracts::SkinScanner;
use adm4_decision::{
    DecisionGraph, DecisionOption, DecisionPoint, DepthProfile, DesignDomain, DesignLevel,
    DesignNode, DesignOrganization, GenreScope, PointRequirement, Provenance, SelectionMode,
    UNASSIGNED_DOMAIN_ID,
};
use adm4_pipeline::compile_frozen_design;
use adm4_space::{DesignSpace, GenrePack};

const RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"多选点逐条交叉","conclusion":"未发现矛盾"}]}"#;

fn option(id: &str, label: &str) -> DecisionOption {
    DecisionOption {
        id: id.into(),
        label: label.into(),
        ..Default::default()
    }
}

fn signal_option(id: &str, label: &str, signal: &str) -> DecisionOption {
    DecisionOption {
        id: id.into(),
        label: label.into(),
        summary: format!("{label}的规则说明"),
        effects_template: vec![serde_json::json!({"effect": "emit_signal", "signal": signal})],
        ..Default::default()
    }
}

fn point(
    id: &str,
    domain: &str,
    node_id: &str,
    level: DesignLevel,
    mode: SelectionMode,
    options: Vec<DecisionOption>,
) -> DecisionPoint {
    DecisionPoint {
        id: id.into(),
        domain: domain.into(),
        level,
        genre_scope: GenreScope::Universal,
        question: format!("{id}？"),
        mda_layer: None,
        design_question: Some(format!("{id} 的设计提问")),
        node_id: Some(node_id.into()),
        selection_mode: mode,
        requirement: PointRequirement::Unlocked,
        options,
        skin_fields: Vec::new(),
        evidence_slots: false,
    }
}

fn space() -> DesignSpace {
    let mut genre = point(
        "u.genre",
        "core",
        "positioning_scope",
        DesignLevel::L2,
        SelectionMode::Single,
        vec![option("lane", "通道防守"), option("grid", "网格调度")],
    );
    genre.options[0].unlocks = vec!["sys.core".into()];

    let mut systems = point(
        "sys.core",
        "gameplay",
        "gameplay_systems",
        DesignLevel::L3,
        SelectionMode::Multi {
            allow_primary: true,
        },
        vec![option("combat", "战斗系统"), option("economy", "经济系统")],
    );
    systems.options[0].unlocks = vec!["mech.damage".into(), "mech.modifiers".into()];

    let points = vec![
        point(
            "u.audience",
            "core",
            "positioning_scope",
            DesignLevel::L0,
            SelectionMode::Multi {
                allow_primary: true,
            },
            vec![
                option("core_players", "核心玩家"),
                option("casual_players", "泛用户"),
            ],
        ),
        point(
            "u.promise",
            "core",
            "positioning_scope",
            DesignLevel::L1,
            SelectionMode::Single,
            vec![option("underdog", "以小博大"), option("power", "力量成长")],
        ),
        genre,
        systems,
        // 显式 system 标签写的是 L3 决策 id：多选 L3 上解析到主选系统。
        {
            let mut mechanic = point(
                "mech.damage",
                "gameplay",
                "gameplay_systems",
                DesignLevel::L4,
                SelectionMode::Single,
                vec![
                    signal_option("linear", "线性伤害", "damage_applied"),
                    signal_option("curved", "曲线伤害", "damage_applied"),
                ],
            );
            for candidate in &mut mechanic.options {
                candidate
                    .compiler_tags
                    .insert("system".into(), "sys.core".into());
            }
            mechanic
        },
        // 无 system 标签的多选 L4：走「同域已选 L3 系统」缺省归属。
        point(
            "mech.modifiers",
            "gameplay",
            "gameplay_systems",
            DesignLevel::L4,
            SelectionMode::Multi {
                allow_primary: false,
            },
            vec![
                signal_option("slow", "减速词条", "modifier_slow"),
                signal_option("burn", "点燃词条", "modifier_burn"),
            ],
        ),
    ];

    let organization = DesignOrganization::new(
        vec![
            DesignDomain {
                id: "positioning".into(),
                name: "立项与产品定位设计".into(),
                description: String::new(),
                order: 1,
            },
            DesignDomain {
                id: "gameplay".into(),
                name: "玩法系统设计".into(),
                description: String::new(),
                order: 3,
            },
        ],
        vec![
            DesignNode {
                id: "positioning_scope".into(),
                domain_id: "positioning".into(),
                name: "定位与范围".into(),
                description: String::new(),
                role_class: "strategic".into(),
            },
            DesignNode {
                id: "gameplay_systems".into(),
                domain_id: "gameplay".into(),
                name: "玩法系统组成".into(),
                description: String::new(),
                role_class: "system_concrete".into(),
            },
        ],
    );

    DesignSpace {
        universal_version: "test".into(),
        pack: GenrePack {
            pack_id: "multi_test".into(),
            pack_version: "0.1.0".into(),
            display_name: "多选测试包".into(),
            reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
            profile_points: Vec::new(),
            cardinality_expectations: Default::default(),
            consistency_rules: Vec::new(),
            nodes: Vec::new(),
            decision_points: Vec::new(),
        },
        graph: match DecisionGraph::new(points) {
            Ok(graph) => graph,
            Err(error) => panic!("测试图构造失败：{}", error.message),
        },
        organization,
    }
}

fn engine() -> AuthoringEngine {
    let space = space();
    let state = AuthoringState::new(
        "多选链路项目",
        "multi_test",
        "0.1.0",
        DepthProfile::new(DesignLevel::L4).unwrap(),
    );
    AuthoringEngine::new(space, state).unwrap()
}

#[test]
fn multi_select_point_passes_freeze_gates_and_compiles_every_selected_option() {
    let mut engine = engine();

    // L0 多选 + 主选。
    engine
        .select_option("u.audience", "core_players", Provenance::UserManual)
        .unwrap();
    engine.add_option("u.audience", "casual_players").unwrap();
    engine
        .set_primary_option("u.audience", "core_players")
        .unwrap();
    engine.confirm_selection("u.audience").unwrap();

    engine
        .select_option("u.promise", "underdog", Provenance::UserManual)
        .unwrap();
    engine.confirm_selection("u.promise").unwrap();
    engine
        .select_option("u.genre", "lane", Provenance::UserManual)
        .unwrap();
    engine.confirm_selection("u.genre").unwrap();

    // L3 多选 + 主选：两个选项各自 unlock 下游（economy 无下游）。
    engine
        .select_option("sys.core", "combat", Provenance::UserManual)
        .unwrap();
    engine.add_option("sys.core", "economy").unwrap();

    // 反例先验：allow_primary 但未标主选 → 完成度拦截、冻结门第 1 道 block。
    engine.confirm_selection("sys.core").unwrap();
    let before_primary = engine.completeness();
    assert!(
        before_primary
            .blocking
            .iter()
            .any(|item| item.decision_id == "sys.core" && item.detail.contains("未指定主选")),
        "{:?}",
        before_primary.blocking
    );

    engine.set_primary_option("sys.core", "economy").unwrap();
    engine.confirm_selection("sys.core").unwrap();

    engine
        .select_option("mech.damage", "linear", Provenance::UserManual)
        .unwrap();
    engine.confirm_selection("mech.damage").unwrap();

    // L4 多选（不设主选）。
    engine
        .select_option("mech.modifiers", "slow", Provenance::UserManual)
        .unwrap();
    engine.add_option("mech.modifiers", "burn").unwrap();
    engine.confirm_selection("mech.modifiers").unwrap();

    // 领域/节点聚合：两个领域各自完成，没有点落进「未分域」。
    let progress = engine.organization_progress();
    assert!(
        progress.domain(UNASSIGNED_DOMAIN_ID).is_none(),
        "所有点都声明了 node_id，保留领域应为空：{:?}",
        progress.domains
    );
    let gameplay = progress.domain("gameplay").expect("玩法领域应有决策点");
    assert_eq!(gameplay.counts.applicable, 3);
    assert_eq!(gameplay.counts.confirmed, 3);
    assert_eq!(gameplay.percent, 100);

    let completeness = engine.completeness();
    assert!(completeness.is_complete(), "{:?}", completeness.blocking);

    // 冻结门第 4 道：脚本 AI 红队（零发现 + 逐类证据）。
    let provider = ScriptedProvider::new();
    provider.script("freeze_red_team", vec![RED_TEAM_ANSWER.into()]);
    run_red_team(&mut engine, &provider).unwrap();

    let scanner = SkinScanner::new(engine.space().skin_words());
    let report = evaluate_freeze_gates(&engine, &scanner);
    assert!(
        report.all_passed(),
        "多选点应能过五门：{:?}",
        report
            .gates
            .iter()
            .filter(|gate| !gate.passed)
            .collect::<Vec<_>>()
    );

    let frozen = execute_freeze(&mut engine, &scanner).unwrap();

    // FrozenDesign：多选点全部已选选项在案，主选排在 option_id 位。
    let audience = frozen
        .decisions
        .iter()
        .find(|selection| selection.decision_id == "u.audience")
        .expect("画像多选点应进冻结集");
    assert_eq!(audience.option_id, "core_players");
    assert_eq!(audience.primary_option.as_deref(), Some("core_players"));
    assert_eq!(audience.selected_count(), 2);
    let systems = frozen
        .decisions
        .iter()
        .find(|selection| selection.decision_id == "sys.core")
        .expect("系统多选点应进冻结集");
    assert_eq!(systems.option_id, "economy", "主选应被搬到首位");
    assert_eq!(
        systems.selected_option_ids(),
        vec!["economy", "combat"],
        "主选排序在前"
    );

    // C0：每个已选选项都产出 spec 元素，主选带标记。
    let spec = compile_frozen_design(&frozen, engine.space()).unwrap();
    let system_ids: Vec<&str> = spec.systems.iter().map(|item| item.id.as_str()).collect();
    assert!(system_ids.contains(&"sys.core#economy"), "{system_ids:?}");
    assert!(system_ids.contains(&"sys.core#combat"), "{system_ids:?}");
    let primary_system = spec
        .systems
        .iter()
        .find(|item| item.id == "sys.core#economy")
        .expect("主选系统");
    assert!(
        primary_system.name.contains("（主选）"),
        "{}",
        primary_system.name
    );

    let mechanic_ids: Vec<&str> = spec.mechanics.iter().map(|item| item.id.as_str()).collect();
    assert!(mechanic_ids.contains(&"mech.damage"), "{mechanic_ids:?}");
    assert!(
        mechanic_ids.contains(&"mech.modifiers#slow"),
        "{mechanic_ids:?}"
    );
    assert!(
        mechanic_ids.contains(&"mech.modifiers#burn"),
        "{mechanic_ids:?}"
    );
    // 归属解析：显式标签写决策 id → 主选系统；缺省归属 → 同域已选 L3 的主选系统。
    for mechanic in &spec.mechanics {
        assert_eq!(
            mechanic.system_id, "sys.core#economy",
            "机制 {} 的系统归属",
            mechanic.id
        );
    }

    // 画像点：同一决策点的多个已选选项合并成一个键，主选在前。
    let audience_profile = spec.intent.profile.get("u.audience").expect("画像键应存在");
    assert_eq!(audience_profile, "核心玩家（主选）、泛用户");
}
