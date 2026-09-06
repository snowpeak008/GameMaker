//! T-W7-5d 塔防薄样板的 app 级验收（执行计划 §6 样板矩阵最后一件，范围经主开发
//! 调整：lane_defense 重表达对照因发现 B 推迟到量产波金样重固化窗口——见
//! `docs/memory/w7_wave4/4b_迁移方案.md`；本卡用独立新 pack 实证 **G1 建造放置
//! 轻档 BP0（预设槽位放置，塔防塔位口径）**，不碰 ld）。
//!
//! 两条测试线：
//! - **装配与组合判定**：真实 `knowledge/design_space/towerdef_thin/pack.json`
//!   三实例装配零悬空（allowed_tiers 收窄 [BP0,BP1] → BP2/BP3 谱系点按不可达剔除，
//!   模块一致性规则连带剔除）；R-C1′ 薄判定如实断言——|H|=0（BP0 W4 轻 / 经济 T0
//!   W5 中 / 计分 K0 W4 轻，无人 W≥9），零 block 零 advice 无署名确认；
//!   B(G)=9.75（4×1.0 + 5×0.75 + 4×0.5）≤ mid_core 42；
//!   反例：击破事件绑定改指幽灵名词 → 装配失败点名 V6。
//! - **全链**：建项 → tier 三档声明（放置 = **bp0_preset_slots，G1 轻档实战主角**）
//!   → 组合判定 → 冻结（五门全绿 + 三模块版本锁）→ C0-C6 全绿 → 断言：
//!   BP0 放置合法性机制到达 C4（occupancy_rule 的 claim_exclusive…fail_if_occupied
//!   槽位占用约束 + build_cost_timing 的即时扣费落成，GWT 非空）+ 塔实体表进
//!   GameSpec + C6 含放置程序任务 + C3 user_prompt 带属性值（5d C3 复核修复的
//!   调用现场证据，出处 `docs/memory/w7_wave5/5d_C3复核结论.md`）。
//!
//! 1c 翻正（T-W7-1c）：RollCheck 真渲染交付后，5d 上报的 BP0 正名点 slot_legality
//! 已改真作答（preset_slot_whitelist，schema 经 1c 归档为 scalar 表引用——Rows 参数
//! 不走占位符替换的基建缺口以数据形态绕开，见 1c 断点申报）；GWT 断言判定条件 +
//! 成功落成三拍 + 失败拒绝分支。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, save_config};
use adm4_archive::DataRoot;
use adm4_contracts::TypedValue;
use adm4_decision::{DesignLevel, ParameterValues, Provenance};
use adm4_pipeline::StageStatus;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

/// 最小通用层：`u.target_scale`（组合判定产品档数据源）+ L1/L2 各一点
/// （空间校验三层齐备），与音游微样板同构（towerdef_thin 无 nodes，无需领域声明）。
const UNIVERSAL_CORE: &str = r#"{
  "space_version": "towerdef-test-1",
  "decision_points": [
    { "id": "u.target_scale", "domain": "core", "level": "L0", "genre_scope": "universal",
      "question": "产品规模档位？",
      "options": [
        { "id": "iaa_hypercasual", "label": "超休闲" },
        { "id": "midcore", "label": "中核" },
        { "id": "triple_a", "label": "大制作" }
      ] },
    { "id": "u.promise", "domain": "core", "level": "L1", "genre_scope": "universal",
      "question": "体验承诺？",
      "options": [ { "id": "guardian_mastery", "label": "布防精进" }, { "id": "loot_fantasy", "label": "刷宝幻想" } ] },
    { "id": "u.genre", "domain": "core", "level": "L2", "genre_scope": "universal",
      "question": "品类？",
      "options": [ { "id": "tower_defense", "label": "塔防" }, { "id": "puzzle", "label": "解谜" } ] }
  ]
}"#;

const FREEZE_RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"塔防薄样板全部决策交叉复核","conclusion":"未发现矛盾"}]}"#;
const C1_RED_TEAM_ANSWER: &str = r#"{"findings":[{"id":"w1","severity":"warning","target":"mechanics/towerdef.kill_bounty_rule","text":"击破赏金与塔造价的回本节奏需逐波试玩验证"}],"per_category":[{"category":"feasibility","checked":"6 条机制逐条","conclusion":"均可实现"}]}"#;

fn repo_knowledge_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

const TOWERDEF_MODULES: [&str; 3] = ["sys.build_placement", "sys.economy", "sys.scoring_combo"];

fn copy_module(temp_systems: &Path, module_id: &str) {
    let target = temp_systems.join(module_id);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(
        repo_knowledge_root()
            .join("systems")
            .join(module_id)
            .join("module.json"),
        target.join("module.json"),
    )
    .unwrap();
}

fn real_pack_json() -> String {
    std::fs::read_to_string(
        repo_knowledge_root()
            .join("design_space")
            .join("towerdef_thin")
            .join("pack.json"),
    )
    .unwrap()
}

/// 隔离环境：真实 towerdef_thin pack 原文（不改字节复制）+ 三模块副本 +
/// 真预算表副本（入库数值改动必须让本测试跟着表态，与 5a/5b 同纪律）。
fn setup(tag: &str, packs: &[(&str, String)]) -> (PathBuf, AppServices) {
    let temp = std::env::temp_dir().join(format!("adm4_towerdef_e2e_{tag}_{}", std::process::id()));
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
    let knowledge = temp.join("knowledge");
    let systems_root = knowledge.join("systems");
    std::fs::create_dir_all(&systems_root).unwrap();
    for module_id in TOWERDEF_MODULES {
        copy_module(&systems_root, module_id);
    }
    let calibration = knowledge.join("calibration");
    std::fs::create_dir_all(&calibration).unwrap();
    std::fs::copy(
        repo_knowledge_root()
            .join("calibration")
            .join("budget.json"),
        calibration.join("budget.json"),
    )
    .unwrap();
    std::fs::create_dir_all(knowledge.join("prompt_library")).unwrap();
    std::fs::write(
        knowledge.join("prompt_library").join("seed.json"),
        r#"{"entries":[]}"#,
    )
    .unwrap();
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

fn text(value: &str) -> TypedValue {
    TypedValue::Text(value.into())
}

fn select_confirmed(services: &AppServices, archive_id: &str, decision: &str, option: &str) {
    services
        .with_project(archive_id, |engine| {
            engine.select_option(decision, option, Provenance::UserManual)?;
            engine.confirm_selection(decision)
        })
        .unwrap();
}

/// 三实例的档位声明（任务卡档位：放置 **BP0 预设槽位——G1 轻档实战主角** /
/// 经济 T0 轻-中档 / 计分 K0 轻档）。
const TIER_DECLARATIONS: [(&str, &str); 3] = [
    ("placement_main.tier", "bp0_preset_slots"),
    ("economy_main.tier", "basic_income"),
    ("score_main.tier", "k0_score_table"),
];

fn slot_row(id: &str, tag: &str, from_wave: i64) -> BTreeMap<String, TypedValue> {
    [
        ("slot_id".to_string(), text(id)),
        ("slot_tag".to_string(), text(tag)),
        ("enabled_from_wave".to_string(), TypedValue::Int(from_wave)),
    ]
    .into_iter()
    .collect()
}

fn tower_row(
    id: &str,
    label: &str,
    cost: i64,
    attack: i64,
    fire_interval: f64,
    range_cells: i64,
) -> BTreeMap<String, TypedValue> {
    [
        ("tower_id".to_string(), text(id)),
        ("label".to_string(), text(label)),
        ("cost".to_string(), TypedValue::Int(cost)),
        ("attack".to_string(), TypedValue::Int(attack)),
        (
            "fire_interval".to_string(),
            TypedValue::Float(fire_interval),
        ),
        ("range_cells".to_string(), TypedValue::Int(range_cells)),
    ]
    .into_iter()
    .collect()
}

fn enemy_row(
    id: &str,
    label: &str,
    hp: i64,
    speed: f64,
    bounty: i64,
) -> BTreeMap<String, TypedValue> {
    [
        ("enemy_id".to_string(), text(id)),
        ("label".to_string(), text(label)),
        ("hp".to_string(), TypedValue::Int(hp)),
        ("speed".to_string(), TypedValue::Float(speed)),
        ("bounty".to_string(), TypedValue::Int(bounty)),
    ]
    .into_iter()
    .collect()
}

fn wave_row(id: &str, enemy_id: &str, count: i64, interval: f64) -> BTreeMap<String, TypedValue> {
    [
        ("wave_id".to_string(), text(id)),
        ("enemy_id".to_string(), text(enemy_id)),
        ("count".to_string(), TypedValue::Int(count)),
        ("interval_seconds".to_string(), TypedValue::Float(interval)),
    ]
    .into_iter()
    .collect()
}

fn bounty_row(action: &str, amount: i64) -> BTreeMap<String, TypedValue> {
    [
        ("action_key".to_string(), text(action)),
        ("bounty_amount".to_string(), TypedValue::Int(amount)),
    ]
    .into_iter()
    .collect()
}

fn score_row(action: &str, label: &str, base_score: i64) -> BTreeMap<String, TypedValue> {
    [
        ("action_id".to_string(), text(action)),
        ("label".to_string(), text(label)),
        ("base_score".to_string(), TypedValue::Int(base_score)),
    ]
    .into_iter()
    .collect()
}

// ---------------------------------------------------------------------------
// 线一：真实 towerdef_thin pack 装配（含 allowed_tiers 收窄剔除）+ R-C1′ 薄判定
//        + V6 反例（击破事件绑定改指幽灵名词 → 装配失败点名）
// ---------------------------------------------------------------------------

#[test]
fn towerdef_thin_pack_assembles_and_bp0_light_composition_is_clean() {
    // 反例包：计分侧击破事件绑定改指幽灵名词——V6 必须点名拦截。
    // pack_id 同步替换（目录即 pack_id），genre_scope 引用一并跟随；
    // 只改 score_main 的一条绑定，核心名词表原样（幽灵目标既非核心名词也无人 provides）。
    let broken_pack = real_pack_json()
        .replace("towerdef_thin", "towerdef_broken")
        .replace(
            r#""sys.combat.hit_signal": "td_kill_signal""#,
            r#""sys.combat.hit_signal": "td_ghost_signal""#,
        );
    let (temp, services) = setup(
        "assembly",
        &[
            ("towerdef_thin", real_pack_json()),
            ("towerdef_broken", broken_pack),
        ],
    );

    // ---- 反例：V6 绑定悬空点名实例 / 名词 / 幽灵目标 ----
    let error = services
        .load_space("towerdef_broken")
        .expect_err("幽灵名词绑定应装配失败");
    assert!(error.message.contains("V6"), "{}", error.message);
    assert!(error.message.contains("score_main"), "{}", error.message);
    assert!(
        error.message.contains("td_ghost_signal"),
        "{}",
        error.message
    );

    // ---- 正例：三实例装配零悬空 ----
    let space = services.load_space("towerdef_thin").unwrap();
    assert_eq!(space.system_instances.len(), 3, "三实例全部装配");
    for (instance, module, semver) in [
        ("placement_main", "sys.build_placement", "1.0.0"),
        ("economy_main", "sys.economy", "1.0.0"),
        ("score_main", "sys.scoring_combo", "1.0.0"),
    ] {
        let info = space
            .system_instances
            .iter()
            .find(|info| info.instance_id == instance)
            .unwrap_or_else(|| panic!("缺实例 {instance}"));
        assert_eq!(info.module_id, module);
        assert_eq!(info.semver, semver);
    }
    // allowed_tiers 收窄 [BP0, BP1]：tier 合成点只剩两档，BP0 在场（G1 轻档主角）。
    let placement_tier = space
        .graph
        .point("placement_main.tier")
        .expect("缺放置 tier 合成点");
    let tier_ids: Vec<&str> = placement_tier
        .options
        .iter()
        .map(|option| option.id.as_str())
        .collect();
    assert_eq!(
        tier_ids,
        vec!["bp0_preset_slots", "bp1_grid_adjacency"],
        "allowed_tiers 收窄后只剩 BP0/BP1 两档"
    );
    for (tier_point, options) in [("economy_main.tier", 3), ("score_main.tier", 4)] {
        let point = space
            .graph
            .point(tier_point)
            .unwrap_or_else(|| panic!("缺 tier 合成点 {tier_point}"));
        assert_eq!(point.options.len(), options, "{tier_point} 档位数不符");
    }
    // BP0/BP1 谱系点在图上；BP2/BP3 谱系点按不可达剔除（不进完成度分母）。
    for id in [
        "placement_main.slot_legality",
        "placement_main.occupancy_rule",
        "placement_main.build_cost_timing",
        "placement_main.grid_topology",
        "placement_main.adjacency_rule",
        "placement_main.facing_rule",
        "economy_main.income_model",
        "economy_main.income_curve",
        "score_main.score_rule",
        "towerdef.wave_system",
        "towerdef.wave_spawn_rule",
        "towerdef.kill_bounty_rule",
        "towerdef.slot_roster",
        "towerdef.tower_roster",
        "towerdef.enemy_roster",
        "towerdef.wave_table",
    ] {
        assert!(space.graph.point(id).is_some(), "装配后缺决策点 {id}");
    }
    for id in [
        "placement_main.support_rule",
        "placement_main.blueprint_copy",
        "placement_main.demolish_refund",
        "placement_main.upkeep_model",
        "placement_main.terrain_destruct",
    ] {
        assert!(
            space.graph.point(id).is_none(),
            "BP2/BP3 谱系点 {id} 应按 allowed_tiers 不可达剔除"
        );
    }

    // ---- R-C1′ 薄判定：中核档 + 三档声明，|H|=0 全轻中组合 ----
    let archive_id = services
        .project_new("塔防装配判定", "towerdef_thin", DesignLevel::L6, None)
        .unwrap();
    select_confirmed(&services, &archive_id, "u.target_scale", "midcore");
    services
        .with_project(&archive_id, |engine| {
            for (decision, option) in TIER_DECLARATIONS {
                engine.select_option(decision, option, Provenance::UserManual)?;
            }
            Ok(())
        })
        .unwrap();

    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产组合报告");
    let report = &assessment.report;
    assert!(assessment.missing_tiers.is_empty(), "三档位已全部声明");
    // 薄判定实录：BP0 W4 轻（core）/ 经济 T0 W5 中（strong）/ 计分 K0 W4 轻（weak）
    // ——无人 W≥9，|H|=0（G1 轻档实战的形态如实声明，与音游微样板同判定路径）。
    assert!(report.h_set.is_empty(), "实际 H：{:?}", report.h_set);
    assert!(report.h_connected, "|H|=0 平凡连通");
    assert!(
        report.blocks.is_empty(),
        "薄组合零硬违例（V1/V2/V3a/V3b/V4/V6 全空），实际：{:?}",
        report.blocks
    );
    assert!(report.advices.is_empty(), "实际：{:?}", report.advices);
    assert!(!report.form_confirmation_required, "|H|=0 无署名确认义务");
    // B(G) = 4(BP0, core×1.0) + 5(T0, strong×0.75) + 4(K0, weak×0.5) = 9.75 ≤ mid_core 42。
    assert!(
        (report.budget_total - 9.75).abs() < 1e-9,
        "B(G) 应为 9.75，实际 {}",
        report.budget_total
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 线二：全链（建项 → tier → 组合判定 → 冻结 → C0-C6 → BP0 C4 证据 + C3 属性值）
// ---------------------------------------------------------------------------

#[test]
fn towerdef_full_chain_reaches_phase1_with_bp0_placement_at_c4() {
    let (temp, services) = setup("chain", &[("towerdef_thin", real_pack_json())]);
    let archive_id = services
        .project_new("薄暮要塞防线", "towerdef_thin", DesignLevel::L6, None)
        .unwrap();

    // ---- ① 通用层三点 + tier 三档声明 ----
    select_confirmed(&services, &archive_id, "u.target_scale", "midcore");
    select_confirmed(&services, &archive_id, "u.promise", "guardian_mastery");
    select_confirmed(&services, &archive_id, "u.genre", "tower_defense");
    for (decision, option) in TIER_DECLARATIONS {
        select_confirmed(&services, &archive_id, decision, option);
    }

    // ---- ② 组合判定确认：|H|=0 零 block，无署名确认 ----
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("确认档位后应有组合报告");
    assert!(
        assessment.report.blocks.is_empty(),
        "实际：{:?}",
        assessment.report.blocks
    );
    assert!(assessment.report.h_set.is_empty());
    assert!(!assessment.report.form_confirmation_required);

    // ---- ③ 激活点补齐：干净选项 + 占位符参数全部由作者填写（I1）----
    services
        .with_project(&archive_id, |engine| {
            let manual = Provenance::UserManual;
            for (decision, option) in [
                // G1 BP0：预设槽位白名单（正名点，1c 翻正真作答：RollCheck 判定
                // slot_open，成功落成三拍/失败拒绝）+ 一格一物占用约束 + 即时扣费落成
                //（塔防/RTS 微操口径）。
                ("placement_main.slot_legality", "preset_slot_whitelist"),
                ("placement_main.occupancy_rule", "exclusive_cell"),
                ("placement_main.build_cost_timing", "instant_build"),
                // 经济 T0：行为赏金表 + 线性产出曲线。
                ("economy_main.income_model", "action_bounty"),
                ("economy_main.income_curve", "linear_progression"),
                // 计分 K0：固定分值表（街机计分）。
                ("score_main.score_rule", "fixed_value_table"),
                // pack 薄点：波次结构 / 按表投放 / 击破赏金 / 四张表。
                ("towerdef.wave_system", "scripted_waves"),
                ("towerdef.wave_spawn_rule", "scripted_release"),
                ("towerdef.kill_bounty_rule", "table_bounty"),
                ("towerdef.slot_roster", "slot_table"),
                ("towerdef.tower_roster", "tower_table"),
                ("towerdef.enemy_roster", "enemy_table"),
                ("towerdef.wave_table", "wave_rows"),
            ] {
                engine.select_option(decision, option, manual.clone())?;
            }

            // 占位符参数（C1 拦裸名纪律：entity 引用一律 {param:xxx_table_id} + 作者填写）。
            for (decision, parameters) in [
                // 1c 翻正：判定读槽位表、落成生成塔实体、占位记回槽位表。
                (
                    "placement_main.slot_legality",
                    scalars(&[
                        ("slot_table_id", text("towerdef.slot_roster")),
                        ("structure_table_id", text("towerdef.tower_roster")),
                        ("occupancy_table_id", text("towerdef.slot_roster")),
                    ]),
                ),
                (
                    "placement_main.occupancy_rule",
                    scalars(&[("occupancy_table_id", text("towerdef.slot_roster"))]),
                ),
                (
                    "placement_main.build_cost_timing",
                    scalars(&[("tower_table_id", text("towerdef.tower_roster"))]),
                ),
                (
                    "economy_main.income_curve",
                    scalars(&[
                        ("base_income", TypedValue::Int(100)),
                        ("per_level_increment", TypedValue::Float(0.0)),
                        ("world_table_id", text("towerdef.slot_roster")),
                    ]),
                ),
                (
                    "towerdef.wave_spawn_rule",
                    scalars(&[
                        ("wave_table_id", text("towerdef.wave_table")),
                        ("enemy_table_id", text("towerdef.enemy_roster")),
                    ]),
                ),
                (
                    "towerdef.kill_bounty_rule",
                    scalars(&[("enemy_table_id", text("towerdef.enemy_roster"))]),
                ),
            ] {
                let problems = engine.set_parameters(decision, parameters)?;
                assert!(
                    problems.is_empty(),
                    "{decision} 参数应通过校验：{problems:?}"
                );
            }

            // 经济赏金表（模块基数 bounty_rows ≥5——非击破行为的经济投放）。
            let problems = engine.set_parameters(
                "economy_main.income_model",
                ParameterValues::Rows {
                    rows: vec![
                        bounty_row("wave_clear", 20),
                        bounty_row("boss_kill", 50),
                        bounty_row("perfect_defense", 30),
                        bounty_row("salvage_pickup", 5),
                        bounty_row("stage_clear", 100),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "赏金表应过校验：{problems:?}");
            // 计分分值表（模块基数 score_table_rows ≥3）。
            let problems = engine.set_parameters(
                "score_main.score_rule",
                ParameterValues::Rows {
                    rows: vec![
                        score_row("kill_normal", "常规击破", 10),
                        score_row("kill_boss", "首领击破", 100),
                        score_row("wave_clear", "整波无漏", 50),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "分值表应过校验：{problems:?}");
            // 塔位槽位表（BP0 预设槽位的实体锚：解锁波次列联动波次进度）。
            let problems = engine.set_parameters(
                "towerdef.slot_roster",
                ParameterValues::Rows {
                    rows: vec![
                        slot_row("s_front_a", "前排甲", 0),
                        slot_row("s_front_b", "前排乙", 0),
                        slot_row("s_mid_a", "中排甲", 2),
                        slot_row("s_back_a", "后排甲", 4),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "槽位表应过校验：{problems:?}");
            // 塔名单表（造价列=放置结算消费端）。
            let problems = engine.set_parameters(
                "towerdef.tower_roster",
                ParameterValues::Rows {
                    rows: vec![
                        tower_row("arrow_spire", "箭岭塔", 120, 18, 1.2, 3),
                        tower_row("frost_pylon", "霜柱塔", 160, 8, 2.0, 2),
                        tower_row("ember_mortar", "烬火臼炮", 220, 40, 3.0, 5),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "塔名单表应过校验：{problems:?}");
            // 敌人名单表（赏金列被击破赏金机制逐行消费）。
            let problems = engine.set_parameters(
                "towerdef.enemy_roster",
                ParameterValues::Rows {
                    rows: vec![
                        enemy_row("husk_walker", "枯壳行者", 60, 1.0, 8),
                        enemy_row("wing_darter", "掠翼者", 40, 2.2, 10),
                        enemy_row("shell_titan", "甲殻巨像", 600, 0.6, 60),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "敌人名单表应过校验：{problems:?}");
            // 波次表（enemy_id 外键由跨表一致性规则锁定）。
            let problems = engine.set_parameters(
                "towerdef.wave_table",
                ParameterValues::Rows {
                    rows: vec![
                        wave_row("w1", "husk_walker", 6, 2.0),
                        wave_row("w2", "wing_darter", 4, 1.5),
                        wave_row("w3", "husk_walker", 10, 1.2),
                        wave_row("w4", "shell_titan", 1, 5.0),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "波次表应过校验：{problems:?}");

            for decision in [
                "placement_main.slot_legality",
                "placement_main.occupancy_rule",
                "placement_main.build_cost_timing",
                "economy_main.income_model",
                "economy_main.income_curve",
                "score_main.score_rule",
                "towerdef.wave_system",
                "towerdef.wave_spawn_rule",
                "towerdef.kill_bounty_rule",
                "towerdef.slot_roster",
                "towerdef.tower_roster",
                "towerdef.enemy_roster",
                "towerdef.wave_table",
            ] {
                engine.confirm_selection(decision)?;
            }

            let completeness = engine.completeness();
            assert!(
                completeness.is_complete(),
                "blocking: {:?}",
                completeness.blocking
            );
            Ok(())
        })
        .unwrap();

    // ---- ⑤ 冻结：红队 → 五门全绿 → 冻结锁三模块版本 ----
    let ai = ScriptedProvider::new();
    ai.script("freeze_red_team", vec![FREEZE_RED_TEAM_ANSWER.into()]);
    ai.script("c1_redteam", vec![C1_RED_TEAM_ANSWER.into()]);
    ai.script(
        "c2_narrative",
        vec![r#"{"text":"基于规格的玩法叙述：敌人按波次表定时涌向防线，玩家在预设塔位上即时扣费落塔（一格一物），塔的输出击破敌人换取赏金滚动扩防；击破事件同时进计分表累积战绩，整波无漏与首领击破有额外行为赏金。"}"#.into()],
    );
    ai.script(
        "c3_asset_description",
        vec![
            r#"{"description":"明快战场卡通风格的塔与敌人立绘，正面站姿，边缘描边，适配 2D 序列帧。"}"#
                .into(),
        ],
    );
    ai.script(
        "c4_interface_naming",
        vec![r#"{"interface_name":"MechanicExecutionService"}"#.into()],
    );
    services.freeze_red_team_with(&archive_id, &ai).unwrap();
    let gate_report = services.freeze_check(&archive_id).unwrap();
    assert!(
        gate_report.all_passed(),
        "五门应全绿：{:?}",
        gate_report.gates
    );
    let frozen = services.freeze_run(&archive_id).unwrap();
    assert_eq!(frozen.version, 1);
    for (module_id, semver) in [
        ("sys.build_placement", "1.0.0"),
        ("sys.economy", "1.0.0"),
        ("sys.scoring_combo", "1.0.0"),
    ] {
        assert_eq!(
            frozen.module_versions.get(module_id).map(String::as_str),
            Some(semver),
            "冻结应锁定 {module_id} 版本"
        );
    }

    // ---- ⑥ C0-C6 全链（C5/C6 人工门）----
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    for stage in ["C0", "C1", "C2", "C3", "C4"] {
        assert!(
            matches!(state.stage_status(stage), StageStatus::Succeeded),
            "{stage}: {:?}",
            state.stage_status(stage)
        );
    }
    services
        .pipeline_confirm(&archive_id, "C5", "样板评审员", "风格方向确认")
        .unwrap();
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    assert!(matches!(
        state.stage_status("C6"),
        StageStatus::WaitingHuman { .. }
    ));
    let state = services
        .pipeline_confirm(&archive_id, "C6", "样板评审员", "Phase 1 文档集签收")
        .unwrap();
    for stage in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        assert!(matches!(state.stage_status(stage), StageStatus::Succeeded));
    }

    // ---- ⑦ BP0 C4 证据 + 塔实体表进 GameSpec + C6 放置任务 + C3 属性值 ----
    let content = services.archives.content_dir(&archive_id);
    let read_contract = |stage: &str| -> serde_json::Value {
        let raw =
            std::fs::read_to_string(content.join(format!("pipeline/v1/{stage}/contract.json")))
                .unwrap_or_else(|e| panic!("{stage} 契约应可读：{e}"));
        serde_json::from_str(&raw).unwrap()
    };

    // C0：塔实体表进 GameSpec（tables 含 towerdef.tower_roster，行实体逐一在场）。
    let c0 = read_contract("C0");
    let tables: Vec<&str> = c0["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|table| table["id"].as_str().unwrap())
        .collect();
    assert!(
        tables.contains(&"towerdef.tower_roster"),
        "GameSpec 应含塔实体表：{tables:?}"
    );
    let entity_ids: Vec<&str> = c0["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entity| entity["id"].as_str().unwrap())
        .collect();
    for id in [
        "towerdef.tower_roster.arrow_spire",
        "towerdef.tower_roster.frost_pylon",
        "towerdef.tower_roster.ember_mortar",
    ] {
        assert!(entity_ids.contains(&id), "GameSpec 缺塔实体 {id}");
    }

    let c4 = read_contract("C4");
    let capabilities = c4["capabilities"].as_array().unwrap();
    let scenario_then = |cap_id: &str| -> String {
        let capability = capabilities
            .iter()
            .find(|capability| capability["id"] == cap_id)
            .unwrap_or_else(|| panic!("C4 缺能力契约 {cap_id}"));
        let then: Vec<String> = capability["scenarios"][0]["then"]
            .as_array()
            .unwrap()
            .iter()
            .map(|then| then.as_str().unwrap().to_string())
            .collect();
        assert!(!then.is_empty(), "{cap_id} 的 GWT Then 不得为空");
        then.join("；")
    };

    // 1c 翻正证据③：G1 BP0 正名点 slot_legality（preset_slot_whitelist）——
    // RollCheck 真渲染：判定条件（槽位开放）+ 成功落成三拍 + 失败拒绝分支。
    let legality_then = scenario_then("cap_placement_main.slot_legality");
    assert!(
        legality_then.contains("按 slot_open(towerdef.slot_roster, target_slot) 对难度 0 判定"),
        "BP0 槽位判定条件：{legality_then}"
    );
    assert!(
        legality_then.contains(
            "成功→生成实体 towerdef.tower_roster；实体 towerdef.slot_roster 的 world_grid_occupancy 按公式 claim(target_slot, structure_id) 变化；发出信号 placement_done_signal"
        ),
        "BP0 槽位判定成功分支（落成三拍）：{legality_then}"
    );
    assert!(
        legality_then.contains("失败→发出信号 placement_rejected"),
        "BP0 槽位判定拒绝分支：{legality_then}"
    );

    // G1 BP0 放置合法性到达 C4：一格一物占用约束（fail_if_occupied 即槽位约束语义）
    // 落在槽位表实体上。
    let occupancy_then = scenario_then("cap_placement_main.occupancy_rule");
    assert!(
        occupancy_then.contains(
            "实体 towerdef.slot_roster 的 world_grid_occupancy 按公式 claim_exclusive(cell, structure_id) fail_if_occupied 变化"
        ),
        "BP0 占用约束能力契约：{occupancy_then}"
    );
    // G1 BP0 放置结算到达 C4：即时扣费 + 塔落成 + 落成事件（塔防塔位口径三拍）。
    let build_then = scenario_then("cap_placement_main.build_cost_timing");
    assert!(
        build_then.contains("资源 sys.economy.currency_main 按 build_cost(structure_type) 消耗"),
        "BP0 即时扣费契约：{build_then}"
    );
    assert!(
        build_then.contains("生成实体 towerdef.tower_roster"),
        "BP0 塔落成契约：{build_then}"
    );
    assert!(
        build_then.contains("发出信号 placement_done_signal"),
        "BP0 落成事件契约：{build_then}"
    );

    // 波次语义薄点到达 C4：Schedule 周期投放 + 波次事件。
    let spawn_then = scenario_then("cap_towerdef.wave_spawn_rule");
    assert!(
        spawn_then.contains(
            "每 next_spawn_interval(towerdef.wave_table) 秒 触发一次：生成实体 towerdef.enemy_roster；发出信号 td_wave_signal"
        ),
        "波次投放契约：{spawn_then}"
    );
    // 击破赏金薄点到达 C4：击破事件是经济与计分的共同事件源。
    let bounty_then = scenario_then("cap_towerdef.kill_bounty_rule");
    assert!(
        bounty_then.contains("资源 economy_main.currency_main 按 bounty_of(enemy_id) 增加"),
        "击破赏金契约：{bounty_then}"
    );
    assert!(
        bounty_then.contains("发出信号 td_kill_signal"),
        "击破事件契约：{bounty_then}"
    );

    // ---- C6：放置程序任务在场 + 任务 id 零同名重复 ----
    let c6 = read_contract("C6");
    let tasks = c6["tasks"].as_array().unwrap();
    for task_id in [
        "task_cap_placement_main.occupancy_rule",
        "task_cap_placement_main.build_cost_timing",
        "task_cap_towerdef.wave_spawn_rule",
    ] {
        assert!(
            tasks.iter().any(|task| task["id"] == task_id),
            "C6 缺程序任务 {task_id}"
        );
    }
    let task_ids: Vec<&str> = tasks
        .iter()
        .map(|task| task["id"].as_str().unwrap())
        .collect();
    let unique: BTreeSet<&str> = task_ids.iter().copied().collect();
    assert_eq!(
        task_ids.len(),
        unique.len(),
        "C6 任务 id 零同名重复：{task_ids:?}"
    );

    // ---- C3 复核修复的调用现场证据（5d）：user_prompt 带属性值（key=value），
    // 不再只有键名清单——塔的造价/攻击、敌人的生命/赏金逐值进画面描述请求。
    let c3_calls: Vec<_> = ai
        .calls()
        .into_iter()
        .filter(|call| call.purpose == "c3_asset_description")
        .collect();
    assert_eq!(
        c3_calls.len(),
        6,
        "塔 3 + 敌人 3 = 6 个 sprite2d 实体各一次画面描述请求"
    );
    assert!(
        c3_calls
            .iter()
            .any(|call| call.user_prompt.contains("arrow_spire")
                && call.user_prompt.contains("cost=120")
                && call.user_prompt.contains("attack=18")),
        "塔实体的 user_prompt 应带属性值：{:?}",
        c3_calls
            .iter()
            .map(|call| call.user_prompt.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        c3_calls
            .iter()
            .any(|call| call.user_prompt.contains("shell_titan")
                && call.user_prompt.contains("hp=600")
                && call.user_prompt.contains("bounty=60")),
        "敌人实体的 user_prompt 应带属性值"
    );

    std::fs::remove_dir_all(&temp).ok();
}
