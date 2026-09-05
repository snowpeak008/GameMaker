//! T-W7-4b：grid_strategy 四点迁入 sys.tactical_board 的语义等价自证（②a/②c）。
//!
//! 验收口径：同一固定选择集，迁移前后 C0 编译产物经 spec_diff（映射表 =
//! `knowledge/design_space/grid_strategy/migration_map.json`）比对为语义零 diff。
//! 迁移前基准以夹具形式冻结在 `tests/fixtures/grid_premigration_c0_spec.json`
//! ——它是迁移前代码 + 迁移前数据的历史产物，**不得重新生成**
//! （重生成得到的是迁移后产物，等价断言会退化为自比对）。
//!
//! 预期漂移（全部在映射表 ignore_paths 里留痕豁免，见发现 B 论证）：
//! - `identity.frozen_hash` / `identity.project_id`：迁移后 `module_versions`
//!   进冻结哈希载荷，哈希必然变化（project_id 内嵌哈希前缀，同源漂移）；
//! - `systems[grid.tier]` 与其 source_map 条目：tier 合成点是加载器合成的
//!   新增 L3 点，属迁移引入的结构性新增，不属语义漂移。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, save_config};
use adm4_archive::DataRoot;
use adm4_contracts::{MatrixCell, TypedValue};
use adm4_decision::{DesignLevel, ParameterValues, Provenance};
use adm4_pipeline::StageStatus;
use adm4_spec::GameSpec;
use adm4_spec_diff::{IdMapping, diff_specs};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("grid_premigration_c0_spec.json")
}

fn migration_map_path() -> PathBuf {
    repo_root()
        .join("knowledge")
        .join("design_space")
        .join("grid_strategy")
        .join("migration_map.json")
}

/// 二版十六领域入口点（与 end_to_end.rs / cli_smoke.ps1 §5b 逐字一致）：
/// 本测试只验证 grid 品类链路，入口点显式豁免、域内下游不激活。
const V2_DOMAIN_ENTRY_POINTS: [&str; 16] = [
    "v2.product_vision_decision.he_xin_ti_yan_cheng_nuo.core_feeling_type",
    "v2.core_fun_decision.zhu_yao_le_qu_lai_yuan.core_feeling_target",
    "v2.gameplay_system_scope",
    "v2.content_type_decision.he_xin_nei_rong.content_experience",
    "v2.economy_loop_decision.zi_yuan_chan_chu.economy_value_experience",
    "v2.ux_information_architecture_decision.zhu_jie_mian_jie_gou.ux_understanding_experience",
    "v2.art_direction_decision.feng_ge_ding_wei.presentation_feeling_target",
    "v2.balance_model_decision.shu_xing_ding_yi.balance_goal",
    "v2.social_relationship_decision.hao_you_guan_xi.social_relation_experience",
    "v2.retention_onboarding_decision.shou_ci_ti_yan_mu_biao.retention_experience",
    "v2.liveops_launch_content_decision.shou_fa_he_xin_nei_rong.liveops_version_experience",
    "v2.data_goal_metric_decision.liu_cun_zhi_biao.data_validation_goal",
    "v2.compliance_age_rating_decision.nei_rong_chi_du.compliance_protection_goal",
    "v2.documentation_core_doc_decision.xiang_mu_yuan_jing_wen_dang.documentation_alignment_goal",
    "v2.release_store_entry_decision.he_xin_mai_dian_biao_da.release_external_promise",
    "v2.launch_version_decision.shou_fa_ti_yan_bi_huan.launch_experience",
];

fn services_at(temp: &Path) -> AppServices {
    std::fs::remove_dir_all(temp).ok();
    let data_root = DataRoot::new(temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: repo_root()
                .join("knowledge")
                .join("design_space")
                .to_string_lossy()
                .into_owned(),
            system_modules_root: repo_root()
                .join("knowledge")
                .join("systems")
                .to_string_lossy()
                .into_owned(),
            ai_provider: None,
            image_provider: None,
            engine_backend: None,
        },
    )
    .unwrap();
    AppServices::open(Some(temp.to_path_buf())).unwrap()
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

fn row(pairs: &[(&str, TypedValue)]) -> BTreeMap<String, TypedValue> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}

/// 4 兵种（class_types min 4）。
const CLASSES: [(&str, &str, i64, &str); 4] = [
    ("blade", "infantry", 5, "melee"),
    ("pike", "armored", 4, "anti_cavalry"),
    ("bow", "infantry", 5, "ranged"),
    ("mystic", "cavalry", 7, "magic"),
];

/// 6 单位（unit_types min 6），class_id 全部落在兵种表行键集合内（row_reference）。
const UNITS: [(&str, &str); 6] = [
    ("unit_blade_a", "blade"),
    ("unit_blade_b", "blade"),
    ("unit_pike_a", "pike"),
    ("unit_bow_a", "bow"),
    ("unit_bow_b", "bow"),
    ("unit_mystic_a", "mystic"),
];

/// 固定选择集：迁移前后完全相同的一组 grid 决策 + 参数
/// （迁移后新增 tier 合成点 grid.tier，由 helper 按「点在图上才选」自动补选）。
fn author_fixed_selection_set(services: &AppServices, archive_id: &str) {
    for entry in V2_DOMAIN_ENTRY_POINTS {
        services
            .authoring_set_not_applicable(
                archive_id,
                entry,
                "migration_equivalence_scope",
                "本测试只验证 grid 品类链路的迁移等价",
                "4b_test",
            )
            .unwrap();
    }
    services
        .with_project(archive_id, |engine| {
            let manual = Provenance::UserManual;
            let mut decisions: Vec<(&str, &str)> = Vec::new();
            // 迁移后才存在的 tier 合成点：先选它把模块点解锁（迁移前的空间没有该点，跳过）。
            let has_tier = engine.space().graph.point("grid.tier").is_some();
            if has_tier {
                decisions.push(("grid.tier", "mg1_terrain_carrier"));
            }
            decisions.extend([
                ("u.business_model", "premium"),
                ("u.platform", "pc_single"),
                ("u.experience", "guardian_underdog"),
                ("u.genre", "grid_strategy"),
                ("grid.battlefield_system", "square_grid"),
                ("grid.move_rule", "move_point_cost"),
                ("grid.terrain_effect_rule", "terrain_modifier_stack"),
                ("grid.terrain_table", "terrain_effect_table"),
                ("grid.turn_system", "phase_alternate"),
                ("grid.action_point_rule", "move_act_split"),
                ("grid.combat_system", "deterministic_resolve"),
                ("grid.damage_formula", "atk_minus_def"),
                ("grid.counter_attack_rule", "range_check_counter"),
                ("grid.unit_system", "class_based_units"),
                ("grid.unit_roster", "unit_stat_table"),
                ("grid.class_roster", "class_table"),
                ("grid.class_counter_matrix", "class_counter_full"),
                ("grid.progression_system", "exp_level_growth"),
                ("grid.exp_gain_rule", "action_exp"),
                ("grid.progression_table", "progression_rows"),
                ("grid.stage_system", "linear_campaign"),
                ("grid.victory_condition_rule", "objective_victory"),
                ("grid.stage_table", "stage_rows"),
                ("grid.enemy_deploy_table", "enemy_deploy_rows"),
            ]);
            for (decision, option) in &decisions {
                engine.select_option(decision, option, manual.clone())?;
            }

            engine.set_parameters(
                "u.experience",
                scalars(&[("statement", text("以走位与地形博弈换取以少胜多的战场掌控"))]),
            )?;
            engine.set_parameters(
                "grid.move_rule",
                scalars(&[("zoc_enabled", TypedValue::Bool(true))]),
            )?;
            engine.set_parameters(
                "grid.terrain_effect_rule",
                scalars(&[("flier_ignore_terrain", TypedValue::Bool(true))]),
            )?;
            engine.set_parameters(
                "grid.action_point_rule",
                scalars(&[("allow_move_after_act", TypedValue::Bool(false))]),
            )?;
            engine.set_parameters(
                "grid.damage_formula",
                scalars(&[("min_damage", TypedValue::Int(1))]),
            )?;
            engine.set_parameters(
                "grid.counter_attack_rule",
                scalars(&[("counter_damage_ratio", TypedValue::Float(1.0))]),
            )?;
            engine.set_parameters(
                "grid.exp_gain_rule",
                scalars(&[
                    ("exp_per_level", TypedValue::Int(100)),
                    ("kill_bonus_multiplier", TypedValue::Float(3.0)),
                ]),
            )?;
            engine.set_parameters(
                "grid.victory_condition_rule",
                scalars(&[("default_turn_limit", TypedValue::Int(20))]),
            )?;

            // 地形表（terrain_types min 4）。
            engine.set_parameters(
                "grid.terrain_table",
                ParameterValues::Rows {
                    rows: vec![
                        row(&[
                            ("id", text("plain")),
                            ("move_cost", TypedValue::Int(1)),
                            ("evade_bonus", TypedValue::Int(0)),
                            ("def_bonus", TypedValue::Int(0)),
                            ("passable", TypedValue::Bool(true)),
                        ]),
                        row(&[
                            ("id", text("forest")),
                            ("move_cost", TypedValue::Int(2)),
                            ("evade_bonus", TypedValue::Int(20)),
                            ("def_bonus", TypedValue::Int(1)),
                            ("passable", TypedValue::Bool(true)),
                        ]),
                        row(&[
                            ("id", text("mountain")),
                            ("move_cost", TypedValue::Int(3)),
                            ("evade_bonus", TypedValue::Int(30)),
                            ("def_bonus", TypedValue::Int(2)),
                            ("passable", TypedValue::Bool(true)),
                        ]),
                        row(&[
                            ("id", text("river")),
                            ("move_cost", TypedValue::Int(9)),
                            ("evade_bonus", TypedValue::Int(10)),
                            ("def_bonus", TypedValue::Int(0)),
                            ("passable", TypedValue::Bool(false)),
                        ]),
                    ],
                },
            )?;

            // 兵种表 + 单位表 + 克制矩阵（4×4=16 格 ≥ class_counter_cells min 16）。
            engine.set_parameters(
                "grid.class_roster",
                ParameterValues::Rows {
                    rows: CLASSES
                        .iter()
                        .map(|(id, mobility, base_move, tag)| {
                            row(&[
                                ("id", text(id)),
                                ("mobility", text(mobility)),
                                ("base_move", TypedValue::Int(*base_move)),
                                ("counter_tag", text(tag)),
                            ])
                        })
                        .collect(),
                },
            )?;
            engine.set_parameters(
                "grid.unit_roster",
                ParameterValues::Rows {
                    rows: UNITS
                        .iter()
                        .enumerate()
                        .map(|(index, (id, class_id))| {
                            let base = index as i64;
                            row(&[
                                ("id", text(id)),
                                ("class_id", text(class_id)),
                                ("hp", TypedValue::Int(20 + base * 2)),
                                ("atk", TypedValue::Int(6 + base)),
                                ("def", TypedValue::Int(3 + base % 3)),
                                ("skill", TypedValue::Int(5 + base % 4)),
                                ("speed", TypedValue::Int(4 + base % 5)),
                                ("move", TypedValue::Int(4 + base % 3)),
                                ("range_min", TypedValue::Int(1)),
                                ("range_max", TypedValue::Int(1 + base % 2)),
                            ])
                        })
                        .collect(),
                },
            )?;
            let mut cells = Vec::new();
            for (attacker, ..) in &CLASSES {
                for (defender, ..) in &CLASSES {
                    cells.push(MatrixCell {
                        row: (*attacker).to_string(),
                        col: (*defender).to_string(),
                        value: TypedValue::Float(if *attacker == "pike" && *defender == "mystic" {
                            2.0
                        } else if *attacker == "bow" && *defender == "blade" {
                            1.5
                        } else {
                            1.0
                        }),
                    });
                }
            }
            engine.set_parameters(
                "grid.class_counter_matrix",
                ParameterValues::Cells { cells },
            )?;

            // 养成表（progression_rows min 4，class_id 外键闭合）。
            engine.set_parameters(
                "grid.progression_table",
                ParameterValues::Rows {
                    rows: CLASSES
                        .iter()
                        .map(|(id, ..)| {
                            row(&[
                                ("class_id", text(id)),
                                ("hp_gain", TypedValue::Float(2.0)),
                                ("atk_gain", TypedValue::Float(1.0)),
                                ("def_gain", TypedValue::Float(0.8)),
                                ("skill_gain", TypedValue::Float(1.0)),
                                ("speed_gain", TypedValue::Float(0.7)),
                            ])
                        })
                        .collect(),
                },
            )?;

            // 关卡表（stage_count min 8）+ 敌方配置（enemy_deploy_rows min 24 = 8 关 × 3）。
            engine.set_parameters(
                "grid.stage_table",
                ParameterValues::Rows {
                    rows: (1..=8)
                        .map(|stage_no| {
                            row(&[
                                ("stage_no", TypedValue::Int(stage_no)),
                                ("map_width", TypedValue::Int(10 + stage_no % 3)),
                                ("map_height", TypedValue::Int(10)),
                                (
                                    "objective",
                                    text(match stage_no % 4 {
                                        0 => "escort",
                                        1 => "rout",
                                        2 => "seize",
                                        _ => "survive",
                                    }),
                                ),
                                ("turn_limit", TypedValue::Int(20)),
                                ("deploy_limit", TypedValue::Int(6)),
                            ])
                        })
                        .collect(),
                },
            )?;
            let unit_ids: Vec<&str> = UNITS.iter().map(|(id, _)| *id).collect();
            let stances = ["aggressive", "hold_position", "patrol", "guard_objective"];
            let mut deploy_rows = Vec::new();
            for stage_no in 1..=8i64 {
                for slot in 0..3i64 {
                    let ordinal = ((stage_no - 1) * 3 + slot) as usize;
                    deploy_rows.push(row(&[
                        ("slot_id", text(&format!("slot_s{stage_no}_{slot}"))),
                        ("stage_no", TypedValue::Int(stage_no)),
                        ("unit_id", text(unit_ids[ordinal % unit_ids.len()])),
                        ("grid_x", TypedValue::Int(2 + slot * 3)),
                        ("grid_y", TypedValue::Int(1 + stage_no % 8)),
                        ("ai_stance", text(stances[ordinal % stances.len()])),
                    ]));
                }
            }
            engine.set_parameters(
                "grid.enemy_deploy_table",
                ParameterValues::Rows { rows: deploy_rows },
            )?;

            for (decision, _) in &decisions {
                engine.confirm_selection(decision)?;
            }
            let report = engine.completeness();
            assert!(report.is_complete(), "blocking: {:?}", report.blocking);
            Ok(())
        })
        .unwrap();
}

fn scripted_ai() -> ScriptedProvider {
    let provider = ScriptedProvider::new();
    provider.script(
        "freeze_red_team",
        vec![
            r#"{"findings":[],"per_category":[{"category":"consistency","checked":"全部决策交叉复核","conclusion":"未发现矛盾"}]}"#.into(),
        ],
    );
    provider.script(
        "c1_redteam",
        vec![
            r#"{"findings":[{"id":"w1","severity":"warning","target":"mechanics/grid.damage_formula","text":"最低伤害保底与高防单位的交换比需要试玩验证"}],"per_category":[{"category":"feasibility","checked":"机制逐条","conclusion":"均可实现"}]}"#.into(),
        ],
    );
    provider.script(
        "c2_narrative",
        vec![
            r#"{"text":"基于规格的玩法叙述：玩家在方形网格上按地形消耗调度单位，以克制与地形加成赢下逐关战役。"}"#.into(),
        ],
    );
    provider.script(
        "c3_asset_description",
        vec![
            r#"{"description":"像素战棋风格的单位立绘，四方向站姿，边缘清晰，适配 2D 网格。"}"#
                .into(),
        ],
    );
    provider.script(
        "c4_interface_naming",
        vec![r#"{"interface_name":"MechanicExecutionService"}"#.into()],
    );
    provider
}

/// 建项 → 固定选择集 → 冻结 → C0，返回 C0 契约 JSON 文本。
fn compile_grid_c0(temp: &Path) -> String {
    let services = services_at(temp);
    let archive_id = services
        .project_new("网格迁移基准", "grid_strategy", DesignLevel::L6, None)
        .unwrap();
    author_fixed_selection_set(&services, &archive_id);
    let ai = scripted_ai();
    services.freeze_red_team_with(&archive_id, &ai).unwrap();
    let report = services.freeze_check(&archive_id).unwrap();
    assert!(report.all_passed(), "gates: {:?}", report.gates);
    services.freeze_run(&archive_id).unwrap();
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C0", &ai)
        .unwrap();
    assert!(
        matches!(state.stage_status("C0"), StageStatus::Succeeded),
        "C0: {:?}",
        state.stage_status("C0")
    );
    std::fs::read_to_string(
        services
            .archives
            .content_dir(&archive_id)
            .join("pipeline/v1/C0/contract.json"),
    )
    .unwrap()
}

/// 一次性夹具捕获（迁移前手动跑 `cargo test ... -- --ignored` 生成）：
/// 夹具是「迁移前代码 + 迁移前数据」的历史证据，已存在时拒绝覆盖。
#[test]
#[ignore = "迁移前基准捕获：夹具已固化后不得重生成（重生成得到的是迁移后产物）"]
fn capture_premigration_fixture() {
    let path = fixture_path();
    assert!(
        !path.is_file(),
        "夹具 {} 已存在：它是迁移前历史证据，禁止覆盖",
        path.display()
    );
    let temp = std::env::temp_dir().join(format!("adm4_4b_capture_{}", std::process::id()));
    let spec_text = compile_grid_c0(&temp);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &spec_text).unwrap();
    std::fs::remove_dir_all(&temp).ok();
}

/// ②a 主断言：迁移后同一选择集重编 C0，与迁移前夹具做 spec_diff → 语义零 diff。
///
/// 映射表就是交付物 `migration_map.json`（基本恒等 + ignore_paths 豁免留痕），
/// 测试直接消费它——映射表错了这里会红，不存在两处真相。
#[test]
fn grid_migration_c0_semantics_are_equivalent() {
    let fixture = std::fs::read_to_string(fixture_path())
        .expect("迁移前夹具应已固化（capture_premigration_fixture 在迁移前跑过一次）");
    let old_spec: GameSpec = serde_json::from_str(&fixture).expect("夹具应解析为 GameSpec");

    let temp = std::env::temp_dir().join(format!("adm4_4b_equiv_{}", std::process::id()));
    let new_text = compile_grid_c0(&temp);
    let new_spec: GameSpec = serde_json::from_str(&new_text).expect("新产物应解析为 GameSpec");
    std::fs::remove_dir_all(&temp).ok();
    // 顺手把迁移后产物落到固定临时路径，供 spec_diff CLI 独立复跑同一比对：
    // cargo run -p adm4-spec-diff -- --old <夹具> --new <该文件> --map <migration_map.json>
    std::fs::write(
        std::env::temp_dir().join("adm4_4b_grid_postmigration_c0.json"),
        &new_text,
    )
    .ok();

    let map_raw = std::fs::read_to_string(migration_map_path()).expect("migration_map.json 应存在");
    let mapping: IdMapping = serde_json::from_str(&map_raw).expect("映射表应可解析");
    let report = diff_specs(&old_spec, &new_spec, &mapping).expect("diff 应成功");
    assert!(
        report.is_clean(),
        "迁移前后 C0 语义必须零 diff：\n{}",
        report.render()
    );
}

/// ②c：迁移后 grid 项目跑通 C0-C6，C6 装配任务零同名重复（波 1 断言在 grid 上复跑）。
#[test]
fn grid_post_migration_c6_assembly_tasks_have_no_duplicate_titles() {
    let temp = std::env::temp_dir().join(format!("adm4_4b_c6_{}", std::process::id()));
    let services = services_at(&temp);
    let archive_id = services
        .project_new("网格迁移C6", "grid_strategy", DesignLevel::L6, None)
        .unwrap();
    author_fixed_selection_set(&services, &archive_id);
    let ai = scripted_ai();
    services.freeze_red_team_with(&archive_id, &ai).unwrap();
    services.freeze_run(&archive_id).unwrap();

    let state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    assert!(matches!(
        state.stage_status("C5"),
        StageStatus::WaitingHuman { .. }
    ));
    services
        .pipeline_confirm(&archive_id, "C5", "4b_test", "风格方向确认")
        .unwrap();
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    assert!(matches!(
        state.stage_status("C6"),
        StageStatus::WaitingHuman { .. }
    ));
    let state = services
        .pipeline_confirm(&archive_id, "C6", "4b_test", "文档集签收")
        .unwrap();
    for stage in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        assert!(
            matches!(state.stage_status(stage), StageStatus::Succeeded),
            "{stage}: {:?}",
            state.stage_status(stage)
        );
    }

    let contract_text = std::fs::read_to_string(
        services
            .archives
            .content_dir(&archive_id)
            .join("pipeline/v1/C6/contract.json"),
    )
    .unwrap();
    let contract: serde_json::Value = serde_json::from_str(&contract_text).unwrap();
    let assembly_titles: Vec<&str> = contract["tasks"]
        .as_array()
        .expect("C6 契约应有 tasks 数组")
        .iter()
        .filter(|task| task["kind"] == "assembly")
        .map(|task| task["title"].as_str().unwrap_or_default())
        .collect();
    let unique: std::collections::BTreeSet<&str> = assembly_titles.iter().copied().collect();
    assert_eq!(
        assembly_titles.len(),
        unique.len(),
        "迁移后 grid 项目的 C6 装配任务出现同名重复：{assembly_titles:?}"
    );
    assert!(
        !assembly_titles.is_empty(),
        "C6 装配任务不应为空（断言对象存在）"
    );

    std::fs::remove_dir_all(&temp).ok();
}
