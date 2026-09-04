//! W7 3a 系统模块运行时接线的 app 级端到端验收。
//!
//! 覆盖（任务卡验收 3/4/5/6 + ⑥ 全链）：
//! - 含 `system_refs` 的 pack 走全链：建项 → tier 档位选择 → 确认 → 冻结
//!   （module_versions 进产物）→ C0（模块机制进 GameSpec）→ C1；
//! - 两 pack 引用同模块选不同档 → 激活点集与完成度分母各符合；
//! - 同 pack 双实例（loot_main + loot_alt）不冲突；
//! - 绑定悬空 / activates-tier_gate 矛盾 / 版本不满足 → 加载失败点名
//!   （system_loader 已有纯函数单测，这里补 app 级）；
//! - Graph 全链：schema 点 → confirm 图值 → 冻结 → C0 GameSpec.graphs 非空 → C1 闭合；
//! - 模块 semver 漂移 → 复演 fail-closed 点名两侧版本；
//! - 项目私有系统模块登记（系统级 custom）：合法入库参与装配，非法拒绝点名。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, save_config};
use adm4_archive::DataRoot;
use adm4_contracts::TypedValue;
use adm4_decision::DesignLevel;
use adm4_decision::system_module::SystemModule;
use adm4_decision::{ParameterValues, PointApplicability, Provenance};
use adm4_pipeline::StageStatus;
use adm4_space::SystemRef;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 夹具：最小合成设计空间 + 真实系统模块库副本
// ---------------------------------------------------------------------------

/// 最小通用层：L0/L1/L2 各一点（校验硬要求三层齐备）。
const UNIVERSAL_CORE: &str = r#"{
  "space_version": "modtest-1",
  "decision_points": [
    { "id": "u.audience", "domain": "core", "level": "L0", "genre_scope": "universal",
      "question": "目标受众？",
      "options": [ { "id": "core_players", "label": "核心玩家" }, { "id": "casual", "label": "休闲玩家" } ] },
    { "id": "u.promise", "domain": "core", "level": "L1", "genre_scope": "universal",
      "question": "体验承诺？",
      "options": [ { "id": "loot_fantasy", "label": "刷宝幻想" }, { "id": "mastery", "label": "技巧精进" } ] },
    { "id": "u.genre", "domain": "core", "level": "L2", "genre_scope": "universal",
      "question": "品类？",
      "options": [ { "id": "arpg", "label": "刷宝动作" }, { "id": "puzzle", "label": "解谜" } ] }
  ]
}"#;

/// 甲包：引用 sys.loot 收窄到前两档（tier 点须 ≥2 选项：非表格点校验）
/// + 一个 Graph schema 决策点（全链验证对象）。
const PACK_ALPHA: &str = r#"{
  "pack_id": "mod_alpha",
  "pack_version": "0.1.0",
  "display_name": "模块甲包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "decision_points": [
    {
      "id": "alpha.map_graph",
      "domain": "content",
      "level": "L5",
      "genre_scope": { "pack": "mod_alpha" },
      "question": "关卡地图结构？",
      "options": [
        {
          "id": "branching_map",
          "label": "分支地图",
          "summary": "肉鸽式分支路线图（有向无环、单入口）",
          "compiler_tags": { "spec_role": "data_table", "data_form": "graph" },
          "parameter_schema": { "schema": "graph", "directed": true, "acyclic": true, "entry": "single" }
        },
        {
          "id": "hub_map",
          "label": "枢纽地图",
          "summary": "中心枢纽放射式地图（可回环、多入口）",
          "compiler_tags": { "spec_role": "data_table", "data_form": "graph" },
          "parameter_schema": { "schema": "graph", "directed": false, "acyclic": false, "entry": "multiple" }
        }
      ]
    }
  ],
  "system_refs": [
    { "instance_id": "loot_main", "module_id": "sys.loot", "version_req": "^1.0.0",
      "allowed_tiers": ["basic_table", "quality_affix_weights"], "core_link": "core" }
  ]
}"#;

/// 乙包：同模块双实例（loot_main 全档 + loot_alt 收窄到前两档），选不同档的对照包。
const PACK_BETA: &str = r#"{
  "pack_id": "mod_beta",
  "pack_version": "0.1.0",
  "display_name": "模块乙包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "decision_points": [],
  "system_refs": [
    { "instance_id": "loot_main", "module_id": "sys.loot", "version_req": "" },
    { "instance_id": "loot_alt", "module_id": "sys.loot", "version_req": "",
      "allowed_tiers": ["basic_table", "quality_affix_weights"] }
  ]
}"#;

/// 丙包：版本要求不可满足（加载失败点名用）。
const PACK_GAMMA: &str = r#"{
  "pack_id": "mod_gamma",
  "pack_version": "0.1.0",
  "display_name": "版本失配包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "decision_points": [],
  "system_refs": [
    { "instance_id": "loot_future", "module_id": "sys.loot", "version_req": "^9.0.0" }
  ]
}"#;

/// 丁包：引用 sys.equipment 但零绑定（V6 悬空点名用）。
const PACK_DELTA: &str = r#"{
  "pack_id": "mod_delta",
  "pack_version": "0.1.0",
  "display_name": "绑定悬空包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "decision_points": [],
  "system_refs": [
    { "instance_id": "equip_main", "module_id": "sys.equipment", "version_req": "" }
  ]
}"#;

/// 戊包：引用带 activates/tier_gate 矛盾的模块（合成坏模块，见 CONTRADICT_MODULE）。
const PACK_EPSILON: &str = r#"{
  "pack_id": "mod_epsilon",
  "pack_version": "0.1.0",
  "display_name": "门控矛盾包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "decision_points": [],
  "system_refs": [
    { "instance_id": "bad_main", "module_id": "sys.contradict", "version_req": "" }
  ]
}"#;

/// 结构自校验能过（前缀/名词/阶梯全合法）、但 sys.contradict.c 声明 tier_gate=t0
/// 却不在任何档的 activates 里——档位承诺与门控矛盾，实例化必须点名拒绝。
const CONTRADICT_MODULE: &str = r#"{
  "module_id": "sys.contradict",
  "semver": "1.0.0",
  "label_zh": "矛盾模块",
  "summary": "测试用：门控与档位承诺矛盾",
  "nouns": [],
  "interface": { "provides": [], "consumes": [], "modifies": [] },
  "mda": { "mechanics_summary": "测试", "dynamics_notes": [], "aesthetics_primary": ["挑战"] },
  "heaviness": { "tiers": [
    { "id": "t0", "label_zh": "低", "rating": { "m": 1, "d": 0, "c": 0, "p": 1, "o": 0 },
      "p_floor": 1, "interface_floor": 0, "activates": ["sys.contradict.a"], "inductions": [], "summary": "" },
    { "id": "t1", "label_zh": "高", "rating": { "m": 2, "d": 1, "c": 0, "p": 1, "o": 0 },
      "p_floor": 1, "interface_floor": 0, "activates": ["sys.contradict.a", "sys.contradict.b"], "inductions": [], "summary": "" }
  ] },
  "decision_points": [
    { "id": "sys.contradict.a", "domain": "test", "level": "L4", "genre_scope": "universal",
      "question": "a？", "tier_gate": "t0",
      "options": [ { "id": "x", "label": "X" }, { "id": "y", "label": "Y" } ] },
    { "id": "sys.contradict.b", "domain": "test", "level": "L4", "genre_scope": "universal",
      "question": "b？", "tier_gate": "t1",
      "options": [ { "id": "x", "label": "X" }, { "id": "y", "label": "Y" } ] },
    { "id": "sys.contradict.c", "domain": "test", "level": "L4", "genre_scope": "universal",
      "question": "c？", "tier_gate": "t0",
      "options": [ { "id": "x", "label": "X" }, { "id": "y", "label": "Y" } ] }
  ]
}"#;

/// 项目私有系统模块草案（合法）：计分连击最小形态（两档：tier 点须 ≥2 选项）。
const PRIVATE_COMBO_MODULE: &str = r#"{
  "module_id": "sys.combo",
  "semver": "0.1.0",
  "label_zh": "连击",
  "summary": "计分连击（项目私有草案）",
  "nouns": [ { "id": "combo_meter", "kind": { "kind": "resource" }, "label_zh": "连击计量", "summary": "连击累积值" } ],
  "interface": { "provides": ["combo_meter"], "consumes": [], "modifies": [] },
  "mda": { "mechanics_summary": "连击累积与衰减", "dynamics_notes": [], "aesthetics_primary": ["挑战"] },
  "heaviness": { "tiers": [
    { "id": "c0", "label_zh": "基础连击", "rating": { "m": 1, "d": 0, "c": 0, "p": 1, "o": 0 },
      "p_floor": 1, "interface_floor": 0, "activates": ["sys.combo.decay"], "inductions": [], "summary": "" },
    { "id": "c1", "label_zh": "连击上限", "rating": { "m": 2, "d": 1, "c": 0, "p": 1, "o": 0 },
      "p_floor": 1, "interface_floor": 0, "activates": ["sys.combo.decay", "sys.combo.cap"], "inductions": [], "summary": "" }
  ] },
  "decision_points": [
    { "id": "sys.combo.decay", "domain": "combo", "level": "L4", "genre_scope": "universal",
      "question": "连击如何衰减？", "tier_gate": "c0",
      "options": [ { "id": "timer_decay", "label": "计时衰减" }, { "id": "hit_reset", "label": "受击清零" } ] },
    { "id": "sys.combo.cap", "domain": "combo", "level": "L4", "genre_scope": "universal",
      "question": "连击上限如何封顶？", "tier_gate": "c1",
      "options": [ { "id": "hard_cap", "label": "硬上限" }, { "id": "soft_decay_cap", "label": "软衰减" } ] }
  ]
}"#;

const FREEZE_RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"全部决策交叉","conclusion":"未发现矛盾"}]}"#;
const C1_RED_TEAM_ANSWER: &str = r#"{"findings":[{"id":"w1","severity":"warning","target":"mechanics/loot_main.rate_model","text":"权重份额下新增条目会稀释旧条目，建议监控"}],"per_category":[{"category":"feasibility","checked":"系统与机制逐条","conclusion":"均可实现"}]}"#;

fn repo_systems_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
        .join("systems")
}

fn copy_module(temp_systems: &Path, module_id: &str) {
    let target = temp_systems.join(module_id);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(
        repo_systems_root().join(module_id).join("module.json"),
        target.join("module.json"),
    )
    .unwrap();
}

/// 搭隔离环境：合成设计空间 + 真实模块库副本 + 数据根，回传门面。
fn setup(tag: &str, packs: &[(&str, &str)], with_contradict: bool) -> (PathBuf, AppServices) {
    let temp = std::env::temp_dir().join(format!("adm4_mod_e2e_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let space_root = temp.join("design_space");
    std::fs::create_dir_all(space_root.join("universal")).unwrap();
    std::fs::write(
        space_root.join("universal").join("core.json"),
        UNIVERSAL_CORE,
    )
    .unwrap();
    for (pack_id, json) in packs {
        std::fs::create_dir_all(space_root.join(pack_id)).unwrap();
        std::fs::write(space_root.join(pack_id).join("pack.json"), json).unwrap();
    }
    let systems_root = temp.join("systems");
    std::fs::create_dir_all(&systems_root).unwrap();
    copy_module(&systems_root, "sys.loot");
    copy_module(&systems_root, "sys.equipment");
    if with_contradict {
        let bad = systems_root.join("sys.contradict");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("module.json"), CONTRADICT_MODULE).unwrap();
    }
    let data_root = DataRoot::new(&temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: space_root.to_string_lossy().into_owned(),
            system_modules_root: systems_root.to_string_lossy().into_owned(),
            ai_provider: None,
            image_provider: None,
            engine_backend: None,
        },
    )
    .unwrap();
    let services = AppServices::open(Some(temp.clone())).unwrap();
    (temp, services)
}

fn scalars(pairs: &[(&str, TypedValue)]) -> ParameterValues {
    ParameterValues::Scalars {
        entries: pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect(),
    }
}

fn drop_row(index: usize) -> BTreeMap<String, TypedValue> {
    [
        (
            "entry_id".to_string(),
            TypedValue::Text(format!("entry_{index:02}")),
        ),
        ("source_id".to_string(), TypedValue::Text("wolf".into())),
        (
            "output_ref".to_string(),
            TypedValue::Text("loot_gold".into()),
        ),
        ("drop_chance".to_string(), TypedValue::Float(0.5)),
    ]
    .into_iter()
    .collect()
}

fn scripted_ai() -> ScriptedProvider {
    let provider = ScriptedProvider::new();
    provider.script("freeze_red_team", vec![FREEZE_RED_TEAM_ANSWER.into()]);
    provider.script("c1_redteam", vec![C1_RED_TEAM_ANSWER.into()]);
    provider
}

// ---------------------------------------------------------------------------
// 验收 3+4：两 pack 同模块不同档的激活点集与分母；同 pack 双实例不冲突
// ---------------------------------------------------------------------------

#[test]
fn dual_packs_and_dual_instances_shape_activation_sets_and_denominators() {
    let (temp, services) = setup(
        "tiers",
        &[("mod_alpha", PACK_ALPHA), ("mod_beta", PACK_BETA)],
        false,
    );

    // 甲包（收窄到 basic_table）：t1/t2 门控的点整体不在图上（不可达点剔除）。
    let alpha_space = services.load_space("mod_alpha").unwrap();
    assert!(alpha_space.graph.point("loot_main.tier").is_some());
    assert!(
        alpha_space
            .graph
            .point("loot_main.table_structure")
            .is_some()
    );
    assert!(
        alpha_space.graph.point("loot_main.pity_rule").is_none(),
        "收窄 allowed_tiers 后 tier_gate 更高的点必须剔除"
    );
    assert_eq!(alpha_space.system_instances.len(), 1);
    assert_eq!(alpha_space.system_instances[0].semver, "1.0.0");

    // 乙包：同模块双实例互不冲突，各自命名空间齐备；收窄实例的高档点剔除。
    let beta_space = services.load_space("mod_beta").unwrap();
    for id in [
        "loot_main.tier",
        "loot_alt.tier",
        "loot_main.table_structure",
        "loot_alt.table_structure",
        "loot_main.pity_rule",
    ] {
        assert!(beta_space.graph.point(id).is_some(), "缺 {id}");
    }
    assert!(beta_space.graph.point("loot_alt.pity_rule").is_none());
    assert_eq!(beta_space.system_instances.len(), 2);

    // 甲包项目：tier 未选时模块点 Inactive、分母 = 3 通用 + tier + graph 点 = 5。
    let alpha_project = services
        .project_new("甲包项目", "mod_alpha", DesignLevel::L6, None)
        .unwrap();
    let engine = services.open_engine(&alpha_project).unwrap();
    let map = engine.applicability();
    assert_eq!(map["loot_main.tier"], PointApplicability::Active);
    assert_eq!(
        map["loot_main.table_structure"],
        PointApplicability::Inactive
    );
    assert_eq!(engine.completeness().total, 5);

    // 选 basic_table 档 → 3 个 t0 点激活，分母 5 → 8。
    services
        .with_project(&alpha_project, |engine| {
            engine.select_option("loot_main.tier", "basic_table", Provenance::UserManual)?;
            engine.confirm_selection("loot_main.tier")
        })
        .unwrap();
    let engine = services.open_engine(&alpha_project).unwrap();
    let map = engine.applicability();
    for id in [
        "loot_main.table_structure",
        "loot_main.roll_timing",
        "loot_main.rate_model",
    ] {
        assert_eq!(map[id], PointApplicability::Active, "{id} 应被 t0 激活");
    }
    assert_eq!(engine.completeness().total, 8);

    // 乙包项目：loot_main 选最重档、loot_alt 选最轻档 → 分母 = 3 + 2 tier + 7 + 3 = 15。
    let beta_project = services
        .project_new("乙包项目", "mod_beta", DesignLevel::L6, None)
        .unwrap();
    services
        .with_project(&beta_project, |engine| {
            engine.select_option("loot_main.tier", "pity_directed", Provenance::UserManual)?;
            engine.confirm_selection("loot_main.tier")?;
            engine.select_option("loot_alt.tier", "basic_table", Provenance::UserManual)?;
            engine.confirm_selection("loot_alt.tier")
        })
        .unwrap();
    let engine = services.open_engine(&beta_project).unwrap();
    let map = engine.applicability();
    assert_eq!(map["loot_main.pity_rule"], PointApplicability::Active);
    assert_eq!(map["loot_main.smart_bias"], PointApplicability::Active);
    assert_eq!(map["loot_alt.table_structure"], PointApplicability::Active);
    assert_eq!(engine.completeness().total, 15);

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 验收 6 + ⑥ 全链：建项 → tier 选择 → 确认 → 冻结（module_versions）→ C0 → C1；
// Graph schema 点 → 图值 → GameSpec.graphs → C1 闭合；semver 漂移复演 fail-closed
// ---------------------------------------------------------------------------

#[test]
fn full_chain_with_system_module_graph_and_version_drift_guard() {
    let (temp, services) = setup("chain", &[("mod_alpha", PACK_ALPHA)], false);
    let archive_id = services
        .project_new("模块全链项目", "mod_alpha", DesignLevel::L6, None)
        .unwrap();

    // 创作：通用三点 + tier 档位 + 三个模块机制点（真实参数）+ Graph 图值。
    services
        .with_project(&archive_id, |engine| {
            let manual = Provenance::UserManual;
            for (decision, option) in [
                ("u.audience", "core_players"),
                ("u.promise", "loot_fantasy"),
                ("u.genre", "arpg"),
                ("loot_main.tier", "basic_table"),
                ("loot_main.table_structure", "flat_per_source"),
                ("loot_main.roll_timing", "settlement_batch"),
                ("loot_main.rate_model", "weighted_share"),
                ("alpha.map_graph", "branching_map"),
            ] {
                engine.select_option(decision, option, manual.clone())?;
            }
            engine.set_parameters(
                "loot_main.table_structure",
                ParameterValues::Rows {
                    rows: (1..=10).map(drop_row).collect(),
                },
            )?;
            engine.set_parameters(
                "loot_main.roll_timing",
                scalars(&[
                    ("clear_multiplier", TypedValue::Float(1.5)),
                    ("fail_retain_ratio", TypedValue::Float(0.5)),
                ]),
            )?;
            engine.set_parameters(
                "loot_main.rate_model",
                scalars(&[("draws_per_roll", TypedValue::Int(1))]),
            )?;
            engine.set_parameters(
                "alpha.map_graph",
                scalars(&[(
                    "graph",
                    TypedValue::Text(
                        r#"{"nodes":[{"id":"start"},{"id":"elite"},{"id":"boss"}],"edges":[{"from":"start","to":"elite"},{"from":"elite","to":"boss"}]}"#
                            .into(),
                    ),
                )]),
            )?;
            for decision in [
                "u.audience",
                "u.promise",
                "u.genre",
                "loot_main.tier",
                "loot_main.table_structure",
                "loot_main.roll_timing",
                "loot_main.rate_model",
                "alpha.map_graph",
            ] {
                engine.confirm_selection(decision)?;
            }
            let report = engine.completeness();
            assert!(report.is_complete(), "blocking: {:?}", report.blocking);
            Ok(())
        })
        .unwrap();

    // 冻结：module_versions 进产物（sys.loot@1.0.0）。
    let ai = scripted_ai();
    services.freeze_red_team_with(&archive_id, &ai).unwrap();
    let report = services.freeze_check(&archive_id).unwrap();
    assert!(report.all_passed(), "gates: {:?}", report.gates);
    let frozen = services.freeze_run(&archive_id).unwrap();
    assert_eq!(
        frozen.module_versions.get("sys.loot").map(String::as_str),
        Some("1.0.0"),
        "冻结产物必须锁定模块版本"
    );
    // 落盘的 frozen_design.json 里字面可见 module_versions 键。
    let frozen_text = std::fs::read_to_string(
        services
            .archives
            .content_dir(&archive_id)
            .join("frozen/v1/frozen_design.json"),
    )
    .unwrap();
    assert!(frozen_text.contains("module_versions"), "{frozen_text}");

    // C0 → C1：模块机制进 GameSpec、Graph 进 graphs、C1 静态闭合 + 红队全绿。
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C1", &ai)
        .unwrap();
    for stage in ["C0", "C1"] {
        assert!(
            matches!(state.stage_status(stage), StageStatus::Succeeded),
            "{stage}: {:?}",
            state.stage_status(stage)
        );
    }
    let spec_text = std::fs::read_to_string(
        services
            .archives
            .content_dir(&archive_id)
            .join("pipeline/v1/C0/contract.json"),
    )
    .unwrap();
    let spec: serde_json::Value = serde_json::from_str(&spec_text).unwrap();
    // 系统 = tier 合成点；机制归属它；graphs 非空且为 schema 声明的有向无环单入口。
    let systems = spec["systems"].as_array().unwrap();
    assert!(
        systems
            .iter()
            .any(|system| system["id"] == "loot_main.tier"),
        "{systems:?}"
    );
    let mechanics = spec["mechanics"].as_array().unwrap();
    assert_eq!(mechanics.len(), 3);
    assert!(
        mechanics
            .iter()
            .all(|mechanic| mechanic["system_id"] == "loot_main.tier"),
        "模块机制必须归属 tier 合成系统：{mechanics:?}"
    );
    let graphs = spec["graphs"].as_array().unwrap();
    assert_eq!(graphs.len(), 1, "GameSpec.graphs 应非空");
    assert_eq!(graphs[0]["id"], "alpha.map_graph");
    assert_eq!(graphs[0]["directed"], true);
    assert_eq!(graphs[0]["acyclic"], true);
    assert_eq!(graphs[0]["entry"], "single");
    assert_eq!(graphs[0]["nodes"].as_array().unwrap().len(), 3);
    // R4 锚定：source_map 有 graphs/ 路径。
    assert!(spec_text.contains("graphs/alpha.map_graph"), "R4 锚定缺失");

    // 模块 semver 漂移（1.0.0 → 1.0.1，仍满足 ^1.0.0 所以装配成功）→
    // 复演比对 fail-closed：点名模块与两侧版本。
    let module_path = temp.join("systems").join("sys.loot").join("module.json");
    let bumped = std::fs::read_to_string(&module_path)
        .unwrap()
        .replace("\"semver\": \"1.0.0\"", "\"semver\": \"1.0.1\"");
    std::fs::write(&module_path, bumped).unwrap();
    // 新门面实例（装配缓存是进程期的，重开才会读到新版本）。
    let reopened = AppServices::open(Some(temp.clone())).unwrap();
    let error = reopened
        .pipeline_run_with(&archive_id, "C0", "C0", &ai)
        .unwrap_err();
    assert!(error.message.contains("sys.loot"), "{}", error.message);
    assert!(error.message.contains("1.0.0"), "{}", error.message);
    assert!(error.message.contains("1.0.1"), "{}", error.message);

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 验收 5（app 级）：版本不满足 / 绑定悬空 / activates-tier_gate 矛盾 → 加载失败点名
// ---------------------------------------------------------------------------

#[test]
fn load_failures_name_version_binding_and_gate_contradiction() {
    let (temp, services) = setup(
        "failures",
        &[
            ("mod_gamma", PACK_GAMMA),
            ("mod_delta", PACK_DELTA),
            ("mod_epsilon", PACK_EPSILON),
        ],
        true,
    );

    // 版本不满足：点名实例、要求与实际版本。
    let error = services.load_space("mod_gamma").unwrap_err();
    assert!(error.message.contains("loot_future"), "{}", error.message);
    assert!(error.message.contains("^9.0.0"), "{}", error.message);
    assert!(error.message.contains("1.0.0"), "{}", error.message);

    // 绑定悬空（V6）：点名实例、端口与名词。
    let error = services.load_space("mod_delta").unwrap_err();
    assert!(error.message.contains("V6"), "{}", error.message);
    assert!(error.message.contains("equip_main"), "{}", error.message);
    assert!(
        error.message.contains("sys.loot.drop_table"),
        "{}",
        error.message
    );

    // activates 与 tier_gate 矛盾：点名决策点。
    let error = services.load_space("mod_epsilon").unwrap_err();
    assert!(
        error.message.contains("sys.contradict.c"),
        "{}",
        error.message
    );
    assert!(error.message.contains("矛盾"), "{}", error.message);

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// ⑤ 系统级 custom：项目私有系统模块登记（合法入库参与装配 / 非法拒绝点名）
// ---------------------------------------------------------------------------

#[test]
fn project_private_system_module_registers_and_rejects_invalid_drafts() {
    let (temp, services) = setup("private", &[("mod_beta", PACK_BETA)], false);
    let archive_id = services
        .project_new("私有模块项目", "mod_beta", DesignLevel::L6, None)
        .unwrap();

    // 合法草案：入库 + 即刻参与本项目装配（instance.module_id 留空由入口补全）。
    let module: SystemModule = serde_json::from_str(PRIVATE_COMBO_MODULE).unwrap();
    let instance = SystemRef {
        instance_id: "combo_main".into(),
        ..Default::default()
    };
    let instance_id = services
        .system_module_add(&archive_id, module, instance)
        .unwrap();
    assert_eq!(instance_id, "combo_main");
    let records = services.system_module_list(&archive_id).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].module.module_id, "sys.combo");

    let engine = services.open_engine(&archive_id).unwrap();
    assert!(engine.space().graph.point("combo_main.tier").is_some());
    assert!(engine.space().graph.point("combo_main.decay").is_some());
    // 私有模块实例进 system_instances（冻结时随 module_versions 锁版本）。
    assert!(
        engine
            .space()
            .system_instances
            .iter()
            .any(|info| info.module_id == "sys.combo" && info.semver == "0.1.0")
    );

    // 非法草案 1：决策点缺模块前缀 → SystemModule::validate 点名拒绝，存档零改动。
    let mut rogue: SystemModule = serde_json::from_str(PRIVATE_COMBO_MODULE).unwrap();
    rogue.module_id = "sys.rogue".into();
    // 决策点仍是 sys.combo.* 前缀 → 前缀违规。
    let error = services
        .system_module_add(
            &archive_id,
            rogue,
            SystemRef {
                instance_id: "rogue_main".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(error.message.contains("前缀"), "{}", error.message);

    // 非法草案 2：与库内模块同名（sys.loot）→ 冲突点名拒绝。
    let mut shadow: SystemModule = serde_json::from_str(PRIVATE_COMBO_MODULE).unwrap();
    shadow.module_id = "sys.loot".into();
    for point in &mut shadow.decision_points {
        point.id = point.id.replace("sys.combo.", "sys.loot.");
    }
    for tier in &mut shadow.heaviness.tiers {
        for activated in &mut tier.activates {
            *activated = activated.replace("sys.combo.", "sys.loot.");
        }
    }
    let error = services
        .system_module_add(
            &archive_id,
            shadow,
            SystemRef {
                instance_id: "loot_shadow".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(error.message.contains("sys.loot"), "{}", error.message);
    assert!(error.message.contains("冲突"), "{}", error.message);

    // 非法草案 3：缺 instance_id → 拒绝。
    let module: SystemModule = serde_json::from_str(PRIVATE_COMBO_MODULE).unwrap();
    let error = services
        .system_module_add(&archive_id, module, SystemRef::default())
        .unwrap_err();
    assert!(error.message.contains("instance_id"), "{}", error.message);

    // 三次非法登记之后清单仍只有 combo（失败不落盘）。
    assert_eq!(services.system_module_list(&archive_id).unwrap().len(), 1);

    std::fs::remove_dir_all(&temp).ok();
}
