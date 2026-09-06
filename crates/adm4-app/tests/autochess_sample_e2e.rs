//! T-W7-5b 自走棋薄样板的 app 级验收（执行计划 §6 样板矩阵第 3 件）：
//! 单样板同时实证波 4 三个新模块——G2 对局赛制（sys.match_format）/
//! G5 战术棋盘（sys.tactical_board）/ G8 编队指挥（sys.squad_command）。
//!
//! 两条测试线：
//! - **装配与组合判定**：真实 `knowledge/design_space/autochess_thin/pack.json`
//!   五实例装配零悬空；R-C1′ 薄判定如实断言——H={format_main}（唯一 W≥9 且 κ=core），
//!   |H|=1 ≤ 中核参考线 2 → 零 block 零 advice、无署名确认要求（与尖塔 |H|=3 超线
//!   形成对照）；B(G)=34.75 ≤ mid_core 42；V1 传导正反例（mf3 的 currency_main
//!   析取由 economy 满足 / db1 的 turn_signal 无源即 V1 block）。
//! - **全链**：建项 → tier 五档声明 → 组合判定确认 → 冻结（五门全绿 + 五模块版本锁）
//!   → C0-C6 全绿 → 三新模块机制到达 C4（各 ≥1 条能力契约 GWT 非空）+
//!   羁绊 ModifyRule 叠加序文字 + DrawFromPool 抽取语义 + C6 跨机制依赖边与任务零重复。
//!
//! 1c 翻正（T-W7-1c）：Attach/RollCheck 真渲染交付后，5b 上报的两个撞臂正名点
//! 已改真作答——G8 阵型协同 = squad_main.synergy_bonus/tag_count_synergy（Attach
//! 叠加序 GWT）、G2 循环积分 = format_main.bracket_shape/round_robin_points
//! （RollCheck 成功/失败两分支 GWT）。其余 N/A 豁免均为纯 genre 理由保留。

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

/// 最小通用层：`u.target_scale`（组合判定产品档数据源，中核 → 参考线 2）+
/// L1/L2 各一点（空间校验三层齐备）+ `gameplay_system_design` 领域声明
/// （tactical_board 的 MG 迁移承接点带 node_id，pack 声明的节点归属该领域，
/// 缺领域即 node.dangling_domain 拦装配）。
const UNIVERSAL_CORE: &str = r#"{
  "space_version": "autochess-test-1",
  "domains": [
    { "id": "gameplay_system_design", "name": "玩法系统设计", "order": 3 }
  ],
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
      "options": [ { "id": "mastery", "label": "技巧精进" }, { "id": "social_rivalry", "label": "同场竞技" } ] },
    { "id": "u.genre", "domain": "core", "level": "L2", "genre_scope": "universal",
      "question": "品类？",
      "options": [ { "id": "auto_battler", "label": "自走棋" }, { "id": "puzzle", "label": "解谜" } ] }
  ]
}"#;

const FREEZE_RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"自走棋薄样板全部决策交叉复核","conclusion":"未发现矛盾"}]}"#;
const C1_RED_TEAM_ANSWER: &str = r#"{"findings":[{"id":"w1","severity":"warning","target":"mechanics/autochess.trait_synergy_rule","text":"羁绊台阶系数与利息曲线的联动强度需对局数据验证"}],"per_category":[{"category":"feasibility","checked":"16 条机制逐条","conclusion":"均可实现"}]}"#;

fn repo_knowledge_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

const AUTOCHESS_MODULES: [&str; 5] = [
    "sys.match_format",
    "sys.tactical_board",
    "sys.squad_command",
    "sys.run_deckbuild",
    "sys.economy",
];

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

/// 隔离环境：真实 autochess_thin pack 原文（不改字节复制）+ 五模块副本 +
/// 真预算表副本（入库数值改动必须让本测试跟着表态，与 5a 同纪律）。
fn setup(tag: &str) -> (PathBuf, AppServices) {
    let temp =
        std::env::temp_dir().join(format!("adm4_autochess_e2e_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let space_root = temp.join("design_space");
    std::fs::create_dir_all(space_root.join("universal")).unwrap();
    std::fs::write(
        space_root.join("universal").join("core.json"),
        UNIVERSAL_CORE,
    )
    .unwrap();
    let real_pack = std::fs::read_to_string(
        repo_knowledge_root()
            .join("design_space")
            .join("autochess_thin")
            .join("pack.json"),
    )
    .unwrap();
    std::fs::create_dir_all(space_root.join("autochess_thin")).unwrap();
    std::fs::write(
        space_root.join("autochess_thin").join("pack.json"),
        real_pack,
    )
    .unwrap();
    let knowledge = temp.join("knowledge");
    let systems_root = knowledge.join("systems");
    std::fs::create_dir_all(&systems_root).unwrap();
    for module_id in AUTOCHESS_MODULES {
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

/// 五实例的档位声明（任务卡档位：赛制重 mf2 / 棋盘中 tb1 / 编队中 s1 /
/// 构筑 db0（db1 的 turn_signal 传导在本组合必 V1，draft 语义 db0 完整成立）/
/// 经济 T0（T1 W9 入重核但与赛制零接口边必产 V3a/V3b 假 block——档位裁量见断点申报）。
const TIER_DECLARATIONS: [(&str, &str); 5] = [
    ("format_main.tier", "mf2_elimination_bracket"),
    ("board_main.tier", "tb1_grid_movement"),
    ("squad_main.tier", "s1_formation"),
    ("shop_main.tier", "db0_simple_draft"),
    ("economy_main.tier", "basic_income"),
];

fn participant_row(id: &str, hp: i64, seat: i64) -> BTreeMap<String, TypedValue> {
    [
        ("id".to_string(), text(id)),
        ("hp".to_string(), TypedValue::Int(hp)),
        ("seat".to_string(), TypedValue::Int(seat)),
    ]
    .into_iter()
    .collect()
}

fn unit_row(
    id: &str,
    label: &str,
    cost: i64,
    trait_tag: &str,
    hp: i64,
    attack: i64,
) -> BTreeMap<String, TypedValue> {
    [
        ("unit_id".to_string(), text(id)),
        ("label".to_string(), text(label)),
        ("cost".to_string(), TypedValue::Int(cost)),
        ("trait_tag".to_string(), text(trait_tag)),
        ("hp".to_string(), TypedValue::Int(hp)),
        ("attack".to_string(), TypedValue::Int(attack)),
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

// ---------------------------------------------------------------------------
// 线一：真实 autochess_thin pack 装配 + R-C1′ 薄判定 + V1 传导正反例
// ---------------------------------------------------------------------------

#[test]
fn autochess_thin_pack_assembles_and_thin_composition_passes_reference_line() {
    let (temp, services) = setup("assembly");

    // ---- 装配成功零悬空绑定（fail-closed：任何 V6/版本/门控矛盾都会 Err）----
    let space = services.load_space("autochess_thin").unwrap();
    assert_eq!(space.system_instances.len(), 5, "五实例全部装配");
    for (instance, module, semver) in [
        ("format_main", "sys.match_format", "1.0.0"),
        ("board_main", "sys.tactical_board", "1.1.0"),
        ("squad_main", "sys.squad_command", "1.0.0"),
        ("shop_main", "sys.run_deckbuild", "1.0.0"),
        ("economy_main", "sys.economy", "1.0.0"),
    ] {
        let info = space
            .system_instances
            .iter()
            .find(|info| info.instance_id == instance)
            .unwrap_or_else(|| panic!("缺实例 {instance}"));
        assert_eq!(info.module_id, module);
        assert_eq!(info.semver, semver);
    }
    // tier 合成点齐备且档位数 = 模块阶梯档数（allowed_tiers 未收窄）。
    for (tier_point, options) in [
        ("format_main.tier", 4),
        ("board_main.tier", 6),
        ("squad_main.tier", 4),
        ("shop_main.tier", 3),
        ("economy_main.tier", 3),
    ] {
        let point = space
            .graph
            .point(tier_point)
            .unwrap_or_else(|| panic!("缺 tier 合成点 {tier_point}"));
        assert_eq!(point.options.len(), options, "{tier_point} 档位数不符");
    }
    // 命名空间重写后的模块点与 pack 层六决策点都在图上。
    for id in [
        "format_main.placement_rule",
        "format_main.bracket_shape",
        "board_main.battlefield_system",
        "board_main.formation_slots",
        "board_main.grid_shape",
        "squad_main.roster_capacity",
        "squad_main.synergy_bonus",
        "shop_main.draft_pick_rule",
        "economy_main.income_model",
        "autochess.auto_battle_resolution",
        "autochess.trait_synergy_rule",
        "autochess.economy_interest",
        "autochess.streak_bonus",
        "autochess.player_roster",
        "autochess.unit_pool",
    ] {
        assert!(space.graph.point(id).is_some(), "装配后缺决策点 {id}");
    }

    // ---- R-C1′ 薄判定：中核档 + 五档声明 ----
    let archive_id = services
        .project_new("自走棋装配判定", "autochess_thin", DesignLevel::L6, None)
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
    assert!(assessment.missing_tiers.is_empty(), "五档位已全部声明");
    // 薄判定实录：唯一重核 = 赛制（mf2 W11 重 × κ core）；棋盘 tb1/编队 s1 各 W8 中、
    // 构筑 db0 W4 轻、经济 T0 W5 中——全部不入 H。
    assert_eq!(report.h_set, vec!["format_main"], "H 集应只有赛制实例");
    assert!(report.h_connected, "|H|=1 平凡连通");
    assert!(
        report.blocks.is_empty(),
        "薄组合零硬违例（V1/V2/V3a/V3b/V4/V6 全空），实际：{:?}",
        report.blocks
    );
    // |H|=1 ≤ 中核参考线 2 → 零提示、无署名确认要求（与尖塔 |H|=3 超线形成对照）。
    assert!(
        report.advices.is_empty(),
        "|H|=1 不超线且 B(G) 不超预算，应零提示，实际：{:?}",
        report.advices
    );
    assert!(
        !report.form_confirmation_required,
        "|H|=1 ≤ 参考线 2 不需要署名形态确认"
    );
    // B(G) = 11(mf2, core) + 8(tb1, core) + 8(s1, core) + 4(db0, core) + 5×0.75(T0, strong)
    //      = 34.75 ≤ mid_core 42。
    assert!(
        (report.budget_total - 34.75).abs() < 1e-9,
        "B(G) 应为 34.75，实际 {}",
        report.budget_total
    );

    // ---- V1 传导正例：赛制升 mf3 的 currency_main 析取由 economy provides 满足 ----
    services
        .with_project(&archive_id, |engine| {
            engine.select_option(
                "format_main.tier",
                "mf3_league_season",
                Provenance::UserManual,
            )
        })
        .unwrap();
    let mf3 = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("应产报告");
    assert!(
        !mf3.report
            .blocks
            .iter()
            .any(|finding| finding.subject == "currency_main"),
        "mf3 的联赛奖金传导（NounProvided currency_main）应由 economy_main provides 满足：{:?}",
        mf3.report.blocks
    );

    // ---- V1 传导负例：构筑升 db1 的 turn_signal 无源即 V1 block ----
    services
        .with_project(&archive_id, |engine| {
            engine.select_option(
                "format_main.tier",
                "mf2_elimination_bracket",
                Provenance::UserManual,
            )?;
            engine.select_option("shop_main.tier", "db1_run_loop", Provenance::UserManual)
        })
        .unwrap();
    let db1 = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("应产报告");
    let v1 = db1
        .report
        .blocks
        .iter()
        .find(|finding| finding.subject == "turn_signal")
        .expect("db1 的抽弃洗循环锚定回合边界，本组合无回合战斗模块应产 V1");
    assert!(
        v1.detail.contains("V1 违例"),
        "V1 传导缺口应点名 turn_signal 无源：{}",
        v1.detail
    );
    // 回落 db0 → 恢复零违例（draft 语义在 db0 完整成立——档位裁量的结构证据）。
    services
        .with_project(&archive_id, |engine| {
            engine.select_option("shop_main.tier", "db0_simple_draft", Provenance::UserManual)
        })
        .unwrap();
    let restored = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("应产报告");
    assert!(
        restored.report.blocks.is_empty(),
        "回落 db0 应恢复零违例：{:?}",
        restored.report.blocks
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 线二：全链（建项 → tier → 组合判定 → 冻结 → C0-C6 → 三模块 C4/C6 断言）
// ---------------------------------------------------------------------------

#[test]
fn autochess_full_chain_reaches_phase1_with_three_new_modules_at_c4() {
    let (temp, services) = setup("chain");
    let archive_id = services
        .project_new("八人棋会薄样板", "autochess_thin", DesignLevel::L6, None)
        .unwrap();

    // ---- ① 通用层三点 + tier 五档声明 ----
    select_confirmed(&services, &archive_id, "u.target_scale", "midcore");
    select_confirmed(&services, &archive_id, "u.promise", "social_rivalry");
    select_confirmed(&services, &archive_id, "u.genre", "auto_battler");
    for (decision, option) in TIER_DECLARATIONS {
        select_confirmed(&services, &archive_id, decision, option);
    }

    // ---- ② 组合判定确认：薄组合零 block、|H|=1 不要求署名确认 ----
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("确认档位后应有组合报告");
    assert!(
        assessment.report.blocks.is_empty(),
        "实际：{:?}",
        assessment.report.blocks
    );
    assert_eq!(assessment.report.h_set, vec!["format_main"]);
    assert!(!assessment.report.form_confirmation_required);

    // ---- ③ 激活点补齐：干净选项 + 占位符参数全部由作者填写（I1）----
    services
        .with_project(&archive_id, |engine| {
            let manual = Provenance::UserManual;
            for (decision, option) in [
                // G2 赛制：淘汰顺序倒排（自走棋名次口径）+ 平滑曲线全名次发放 +
                // 全开放抽签（每轮随机对阵，DrawFromPool）+ 循环积分晋级
                //（1c 翻正：round_robin_points 正名选项，RollCheck 真渲染）。
                ("format_main.placement_rule", "elimination_order_rank"),
                ("format_main.reward_mapping", "smooth_curve_payout"),
                ("format_main.seeding_rule", "open_random_draw"),
                ("format_main.bracket_shape", "round_robin_points"),
                // G5 棋盘：方格 + 格阵站位 + 网格几何 + 恒定步长。
                ("board_main.battlefield_system", "square_grid"),
                ("board_main.formation_slots", "cell_matrix_formation"),
                ("board_main.grid_shape", "square_grid"),
                ("board_main.move_budget", "uniform_step_cost"),
                // G8 编队：人口预算（自走棋人口上限）+ 回合间热换 + 自由格位布置 +
                // 标签计数协同（1c 翻正：tag_count_synergy 正名选项，Attach 真渲染）。
                ("squad_main.roster_capacity", "population_budget"),
                ("squad_main.swap_policy", "in_session_swap"),
                ("squad_main.formation_structure", "free_slot_grid"),
                ("squad_main.synergy_bonus", "tag_count_synergy"),
                // 构筑：商店定数候选 draft（DrawFromPool）。
                ("shop_main.draft_pick_rule", "fixed_choice_count"),
                // 经济：行为赏金（回合工资表）+ 线性产出曲线。
                ("economy_main.income_model", "action_bounty"),
                ("economy_main.income_curve", "linear_progression"),
                // pack 薄点：自动战斗结算 / 羁绊台阶 / 利息 / 连胜连败 / 两张实体表。
                ("autochess.auto_battle_resolution", "survivor_count_settle"),
                ("autochess.trait_synergy_rule", "threshold_step_synergy"),
                ("autochess.economy_interest", "tiered_interest"),
                ("autochess.streak_bonus", "symmetric_streak"),
                ("autochess.player_roster", "participant_table"),
                ("autochess.unit_pool", "unit_pool_table"),
            ] {
                engine.select_option(decision, option, manual.clone())?;
            }

            let scalar_params: [(&str, ParameterValues); 17] = [
                (
                    "format_main.placement_rule",
                    scalars(&[("participant_table_id", text("autochess.player_roster"))]),
                ),
                // 1c 翻正：循环积分晋级（8 人小组前 4 晋级，积分落参战玩家表）。
                (
                    "format_main.bracket_shape",
                    scalars(&[
                        ("group_size", TypedValue::Int(8)),
                        ("advance_count", TypedValue::Int(4)),
                        ("participant_table_id", text("autochess.player_roster")),
                    ]),
                ),
                (
                    "format_main.reward_mapping",
                    scalars(&[
                        ("top_payout", TypedValue::Float(10.0)),
                        ("decay_rate", TypedValue::Float(0.5)),
                    ]),
                ),
                (
                    "board_main.formation_slots",
                    scalars(&[
                        ("formation_rows", TypedValue::Int(4)),
                        ("formation_cols", TypedValue::Int(5)),
                        ("unit_table_id", text("autochess.unit_pool")),
                        ("occupancy_table_id", text("autochess.player_roster")),
                    ]),
                ),
                (
                    "board_main.grid_shape",
                    scalars(&[
                        ("board_width", TypedValue::Int(8)),
                        ("board_height", TypedValue::Int(8)),
                        ("occupancy_table_id", text("autochess.player_roster")),
                    ]),
                ),
                (
                    "board_main.move_budget",
                    scalars(&[
                        ("base_move_points", TypedValue::Int(1)),
                        ("unit_table_id", text("autochess.unit_pool")),
                    ]),
                ),
                (
                    "squad_main.roster_capacity",
                    scalars(&[
                        ("population_cap", TypedValue::Int(10)),
                        ("roster_table_id", text("autochess.unit_pool")),
                    ]),
                ),
                (
                    "squad_main.swap_policy",
                    scalars(&[
                        ("swap_cooldown_seconds", TypedValue::Float(0.0)),
                        ("swap_charges_per_session", TypedValue::Int(99)),
                        ("roster_table_id", text("autochess.unit_pool")),
                    ]),
                ),
                (
                    "squad_main.formation_structure",
                    scalars(&[
                        ("grid_width", TypedValue::Int(7)),
                        ("grid_height", TypedValue::Int(4)),
                        ("assignment_table_id", text("autochess.unit_pool")),
                    ]),
                ),
                // 1c 翻正：标签计数协同（Attach 挂到棋子单位池，叠加序进 GWT）。
                (
                    "squad_main.synergy_bonus",
                    scalars(&[
                        ("per_tag_scaling", TypedValue::Float(0.15)),
                        ("roster_table_id", text("autochess.unit_pool")),
                    ]),
                ),
                (
                    "shop_main.draft_pick_rule",
                    scalars(&[
                        ("pool_table_id", text("autochess.unit_pool")),
                        ("choice_count", TypedValue::Int(5)),
                    ]),
                ),
                (
                    "economy_main.income_curve",
                    scalars(&[
                        ("base_income", TypedValue::Int(5)),
                        ("per_level_increment", TypedValue::Float(0.0)),
                        ("world_table_id", text("autochess.player_roster")),
                    ]),
                ),
                (
                    "autochess.auto_battle_resolution",
                    scalars(&[
                        ("participant_table_id", text("autochess.player_roster")),
                        ("damage_per_unit", TypedValue::Int(1)),
                    ]),
                ),
                // 羁绊 ModifyRule 靶 = 自动战斗结算机制（真实机制 id，C1 复检咬合），
                // priority=20 进叠加序渲染。
                (
                    "autochess.trait_synergy_rule",
                    scalars(&[
                        ("target_rule_id", text("autochess.auto_battle_resolution")),
                        ("per_step_bonus", TypedValue::Float(0.25)),
                    ]),
                ),
                (
                    "autochess.economy_interest",
                    scalars(&[("interest_cap", TypedValue::Int(5))]),
                ),
                (
                    "autochess.streak_bonus",
                    scalars(&[
                        ("streak_cap", TypedValue::Int(5)),
                        ("bonus_per_step", TypedValue::Int(1)),
                    ]),
                ),
                (
                    "board_main.battlefield_system",
                    ParameterValues::None,
                ),
            ];
            for (decision, parameters) in scalar_params {
                if matches!(parameters, ParameterValues::None) {
                    continue;
                }
                let problems = engine.set_parameters(decision, parameters)?;
                assert!(
                    problems.is_empty(),
                    "{decision} 参数应通过校验：{problems:?}"
                );
            }

            // 经济赏金表（模块基数 bounty_rows ≥5）。
            let problems = engine.set_parameters(
                "economy_main.income_model",
                ParameterValues::Rows {
                    rows: vec![
                        bounty_row("round_end", 5),
                        bounty_row("round_win", 1),
                        bounty_row("stage_clear", 2),
                        bounty_row("elimination_bonus", 3),
                        bounty_row("final_podium", 10),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "赏金表应过校验：{problems:?}");
            // 参战玩家表（8 人循环淘汰的实体面）。
            let problems = engine.set_parameters(
                "autochess.player_roster",
                ParameterValues::Rows {
                    rows: (1..=8)
                        .map(|seat| participant_row(&format!("p{seat}"), 100, seat))
                        .collect(),
                },
            )?;
            assert!(problems.is_empty(), "参战玩家表应过校验：{problems:?}");
            // 棋子单位池（两羁绊 × 三棋子）。
            let problems = engine.set_parameters(
                "autochess.unit_pool",
                ParameterValues::Rows {
                    rows: vec![
                        unit_row("shield_bearer", "执盾卫", 1, "warrior", 700, 55),
                        unit_row("blade_dancer", "刃舞者", 2, "warrior", 800, 70),
                        unit_row("axe_warlord", "斧王", 3, "warrior", 950, 85),
                        unit_row("frost_mage", "霜法", 2, "mage", 550, 90),
                        unit_row("storm_caller", "唤雷者", 3, "mage", 600, 105),
                        unit_row("void_seer", "虚空先知", 5, "mage", 750, 140),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "棋子单位池应过校验：{problems:?}");

            for decision in [
                "format_main.placement_rule",
                "format_main.reward_mapping",
                "format_main.seeding_rule",
                "format_main.bracket_shape",
                "board_main.battlefield_system",
                "board_main.formation_slots",
                "board_main.grid_shape",
                "board_main.move_budget",
                "squad_main.roster_capacity",
                "squad_main.swap_policy",
                "squad_main.formation_structure",
                "squad_main.synergy_bonus",
                "shop_main.draft_pick_rule",
                "economy_main.income_model",
                "economy_main.income_curve",
                "autochess.auto_battle_resolution",
                "autochess.trait_synergy_rule",
                "autochess.economy_interest",
                "autochess.streak_bonus",
                "autochess.player_roster",
                "autochess.unit_pool",
            ] {
                engine.confirm_selection(decision)?;
            }

            // ---- ④ 不适用点的署名 N/A 豁免（1c 翻正后余下豁免全为真实 genre/
            // 迁移理由；原 bracket_shape/synergy_bonus 两条撞臂豁免已解除改真作答）----
            for (decision, reason_code, note) in [
                (
                    "format_main.series_length",
                    "genre_not_applicable",
                    "自走棋淘汰制单局无 BO 局分",
                ),
                (
                    "format_main.tiebreak_rule",
                    "genre_not_applicable",
                    "与 series_length 同组（answered_together）：无局分即无平局判定",
                ),
                (
                    "format_main.side_swap",
                    "genre_not_applicable",
                    "自走棋无阵营换边语义（对局双方对称）",
                ),
                (
                    "board_main.move_rule",
                    "migration_carrier_only",
                    "MG 迁移承接点硬编码 grid_strategy 实体锚（grid.unit_roster），非本 pack 可用——上报",
                ),
                (
                    "board_main.terrain_effect_rule",
                    "genre_not_applicable",
                    "自走棋均质棋盘无地形（且迁移点硬编码 grid_strategy 锚）",
                ),
                (
                    "board_main.terrain_table",
                    "genre_not_applicable",
                    "自走棋均质棋盘无地形表",
                ),
                (
                    "board_main.row_effect",
                    "genre_not_applicable",
                    "站位效果由 pack 羁绊薄点承载（自由格位摆位语义在 formation_structure 已答）",
                ),
                (
                    "board_main.range_los",
                    "genre_not_applicable",
                    "攻击射程属自动战斗涌现层（射程外声明）",
                ),
            ] {
                engine.set_not_applicable(decision, reason_code, note, "样板设计师")?;
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

    // ---- ⑤ 冻结：红队 → 五门全绿（gate2 组合段绿）→ 冻结锁五模块版本 ----
    let ai = ScriptedProvider::new();
    ai.script("freeze_red_team", vec![FREEZE_RED_TEAM_ANSWER.into()]);
    ai.script("c1_redteam", vec![C1_RED_TEAM_ANSWER.into()]);
    ai.script(
        "c2_narrative",
        vec![r#"{"text":"基于规格的玩法叙述：八名玩家每回合从商店按稀有度权重抽取候选棋子，用回合工资与存款利息滚动构筑，把棋子摆上方格棋盘的格阵站位；自动战斗按残存棋子数结算扣血，羁绊台阶常驻改写结算规则，血量归零即淘汰，名次按淘汰顺序倒排、奖励沿平滑曲线全名次发放。"}"#.into()],
    );
    ai.script(
        "c3_asset_description",
        vec![
            r#"{"description":"明快棋盘卡通风格的棋子立绘，正面站姿，边缘描边，适配 2D 序列帧。"}"#
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
    let gate2 = gate_report
        .gates
        .iter()
        .find(|gate| gate.gate == "gate2_consistency")
        .expect("gate2 应存在");
    assert!(gate2.passed, "gate2 组合段应绿：{:?}", gate2.findings);
    let frozen = services.freeze_run(&archive_id).unwrap();
    assert_eq!(frozen.version, 1);
    for (module_id, semver) in [
        ("sys.match_format", "1.0.0"),
        ("sys.tactical_board", "1.1.0"),
        ("sys.squad_command", "1.0.0"),
        ("sys.run_deckbuild", "1.0.0"),
        ("sys.economy", "1.0.0"),
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

    // ---- ⑦ 三新模块 C4 证据 + DrawFromPool + 叠加序 + C6 依赖边与零重复 ----
    let content = services.archives.content_dir(&archive_id);
    let read_contract = |stage: &str| -> serde_json::Value {
        let raw =
            std::fs::read_to_string(content.join(format!("pipeline/v1/{stage}/contract.json")))
                .unwrap_or_else(|e| panic!("{stage} 契约应可读：{e}"));
        serde_json::from_str(&raw).unwrap()
    };

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

    // G2 赛制到达 C4：名次判定（淘汰顺序倒排落在参战玩家实体表上）。
    let placement_then = scenario_then("cap_format_main.placement_rule");
    assert!(
        placement_then.contains(
            "实体 autochess.player_roster 的 advancement_state 按公式 rank = remaining_count_at(elimination_time) 变化"
        ),
        "G2 名次判定能力契约：{placement_then}"
    );
    // G2 附加：随机对阵抽签的 DrawFromPool 语义。
    let seeding_then = scenario_then("cap_format_main.seeding_rule");
    assert!(
        seeding_then.contains("从池表 participant_pool 按规则 uniform_without_replacement 抽取"),
        "G2 随机对阵抽签契约：{seeding_then}"
    );

    // G5 棋盘到达 C4：网格几何初始化 + 格阵站位（占位符参数全部作者填写）。
    let grid_then = scenario_then("cap_board_main.grid_shape");
    assert!(
        grid_then.contains(
            "实体 autochess.player_roster 的 board_occupancy 按公式 init_grid(square, 8, 8, neighbors=4) 变化"
        ),
        "G5 网格几何能力契约：{grid_then}"
    );
    let formation_then = scenario_then("cap_board_main.formation_slots");
    assert!(
        formation_then.contains("实体 autochess.unit_pool 的 unit_position_state 按公式 cell = chosen(row < 4, col < 5) 变化"),
        "G5 格阵站位能力契约：{formation_then}"
    );

    // G8 编队到达 C4：人口预算编成。
    let roster_then = scenario_then("cap_squad_main.roster_capacity");
    assert!(
        roster_then.contains(
            "实体 autochess.unit_pool 的 active_roster 按公式 chosen_units where sum(unit_population) <= 10 变化"
        ),
        "G8 人口预算能力契约：{roster_then}"
    );

    // 1c 翻正证据①：G8 阵型协同正名点（tag_count_synergy）——Attach 真渲染，
    // 叠加序文字与 ModifyRule 同款（W7 定稿 §5.3 指令 7）。
    let synergy_bonus_then = scenario_then("cap_squad_main.synergy_bonus");
    assert!(
        synergy_bonus_then.contains(
            "把修饰器 tag_synergy_bonus 挂载到 autochess.unit_pool（生效期 while(tag_count >= tag_threshold)"
        ),
        "G8 阵型协同 Attach 渲染：{synergy_bonus_then}"
    );
    assert!(
        synergy_bonus_then.contains("按 priority=0 结算，同序按机制 id 字典序"),
        "G8 阵型协同叠加序文字（与 ModifyRule 同款）：{synergy_bonus_then}"
    );
    assert!(
        synergy_bonus_then.contains("combat_stat 按公式 stat * (1 + roster_tag_count * 0.15) 变化"),
        "G8 阵型协同加成公式（作者填写系数）：{synergy_bonus_then}"
    );

    // 1c 翻正证据②：G2 循环积分正名点（round_robin_points）——RollCheck 真渲染，
    // 成功/失败两分支齐备（晋级/淘汰）。
    let bracket_then = scenario_then("cap_format_main.bracket_shape");
    assert!(
        bracket_then.contains(
            "实体 autochess.player_roster 的 advancement_state 按公式 group_points += points(win_or_draw) 变化"
        ),
        "G2 循环积分累计：{bracket_then}"
    );
    assert!(
        bracket_then.contains(
            "按 group_rank <= 4 at group_end 对难度 0 判定：成功→状态机 match_participant.bracket_state 进入 advanced；失败→状态机 match_participant.bracket_state 进入 eliminated"
        ),
        "G2 循环积分 RollCheck 两分支：{bracket_then}"
    );

    // 拿牌 = DrawFromPool（任务卡预判兑现：商店 5 选 1）。
    let draft_then = scenario_then("cap_shop_main.draft_pick_rule");
    assert!(
        draft_then.contains(
            "从池表 autochess.unit_pool 按规则 weighted_by_rarity_no_duplicate 抽取 5 个到 draft_offer"
        ),
        "DrawFromPool 能力契约：{draft_then}"
    );

    // 羁绊 = ModifyRule 常驻改写自动战斗结算，叠加序文字进 GWT。
    let synergy_then = scenario_then("cap_autochess.trait_synergy_rule");
    assert!(
        synergy_then.contains(
            "规则 autochess.auto_battle_resolution 的系数按 result * (1 + 0.25 * floor(trait_count / 2)) 缩放"
        ),
        "羁绊 ModifyRule 渲染：{synergy_then}"
    );
    assert!(
        synergy_then.contains("（按 priority=20 结算，同序按机制 id 字典序）"),
        "羁绊叠加序文字：{synergy_then}"
    );

    // 利息与连胜连败薄点到达 C4（经济表达位缺口的 pack 薄点补法实证）。
    let interest_then = scenario_then("cap_autochess.economy_interest");
    assert!(
        interest_then.contains(
            "资源 economy_main.currency_main 按 floor(min(gold_balance, 5 * 10) / 10) 增加"
        ),
        "利息薄点契约：{interest_then}"
    );

    // ---- C6：含三模块任务、羁绊跨机制依赖边、任务 id 零同名重复 ----
    let c6 = read_contract("C6");
    let tasks = c6["tasks"].as_array().unwrap();
    for task_id in [
        "task_cap_format_main.placement_rule",
        "task_cap_board_main.grid_shape",
        "task_cap_squad_main.roster_capacity",
    ] {
        assert!(
            tasks.iter().any(|task| task["id"] == task_id),
            "C6 缺三模块程序任务 {task_id}"
        );
    }
    let synergy_task = tasks
        .iter()
        .find(|task| task["id"] == "task_cap_autochess.trait_synergy_rule")
        .expect("C6 缺羁绊程序任务");
    let depends: Vec<&str> = synergy_task["depends_on"]
        .as_array()
        .unwrap()
        .iter()
        .map(|dep| dep.as_str().unwrap())
        .collect();
    assert!(
        depends.contains(&"task_cap_autochess.auto_battle_resolution"),
        "羁绊任务应依赖自动战斗结算机制的程序任务（ModifyRule 跨机制边）：{depends:?}"
    );
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

    std::fs::remove_dir_all(&temp).ok();
}
