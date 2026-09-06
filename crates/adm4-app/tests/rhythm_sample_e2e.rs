//! T-W7-5c 音游微样板的 app 级验收（执行计划 §6 样板矩阵第 4 件·最小规模）。
//!
//! 实证目标（缺口扫描音游行）：#29 谱面时间轴产判定事件 + G11 计分连击消费成
//! 分数——「音游成立」的最小组合链。两条测试线：
//! - **装配线**：真实 `knowledge/design_space/rhythm_micro/pack.json` + 两新旧模块
//!   （sys.beatmap_timeline 本卡入库 + sys.scoring_combo 4a2 已入库）装配零悬空；
//!   判定事件供需边的机器证据 = 正例装配成功 + 组合报告零 block（V6 通过即
//!   consumes 有源）+ 反例（绑定改指幽灵名词）装配失败点名 V6 与 judgement_signal；
//! - **全链线**：建项 → tier 声明（谱面 BT1 中档 / 计分 K1 中档=连击+倍率衰减）→
//!   全点作答 → 冻结（五门全绿 + module_versions 锁两模块）→ C0-C6 全绿 →
//!   断言：计分连击到达 C4（连击窗/倍率成长/衰减归零三条 GWT 非空且含作者公式）、
//!   判定窗 Table 进 GameSpec、Curve 通路实证（accuracy_curve 编译成两列 (x,y)
//!   Table + 插值注记，波 1 Curve 先例的库内首次真实模块过链）。
//!
//! Curve 裁量（定稿 §5.4 逐案核）：偏差→精度换算是单值 y=f(x)，Curve 表达位成立；
//! 变速 SV 段是区间语义（start/end/factor），单值曲线装不下，模块内申报退化四列
//! Table——本链选 BT1 档不触 BT2 点，退化申报见模块 JSON 与 5c 验收单。
//!
//! 计分侧选型纪律：combo_window 选 timed_window（unbroken_chain 含 RollCheck，
//! 属 C4 未交付渲染臂，选之即全链 Err——1c 纪律下如实绕行并在验收单申报）。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, save_config};
use adm4_archive::DataRoot;
use adm4_contracts::TypedValue;
use adm4_decision::{DesignLevel, ParameterValues, Provenance};
use adm4_pipeline::StageStatus;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

/// 最小通用层：`u.target_scale`（组合判定产品档数据源）+ L1/L2 各一点
/// （空间校验硬要求三层齐备），与 spire 样板同构。
const UNIVERSAL_CORE: &str = r#"{
  "space_version": "rhythmtest-1",
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
      "options": [ { "id": "flow_mastery", "label": "节奏心流精进" }, { "id": "loot_fantasy", "label": "刷宝幻想" } ] },
    { "id": "u.genre", "domain": "core", "level": "L2", "genre_scope": "universal",
      "question": "品类？",
      "options": [ { "id": "rhythm_game", "label": "音乐节奏" }, { "id": "puzzle", "label": "解谜" } ] }
  ]
}"#;

const FREEZE_RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"全部决策交叉复核","conclusion":"未发现矛盾"}]}"#;
const C1_RED_TEAM_ANSWER: &str = r#"{"findings":[{"id":"w1","severity":"warning","target":"mechanics/scoring_main.multiplier_growth","text":"连续倍率封顶值需与判定事件峰值密度联调"}],"per_category":[{"category":"feasibility","checked":"7 条机制逐条","conclusion":"均可实现"}]}"#;

fn repo_knowledge_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

const RHYTHM_MODULES: [&str; 2] = ["sys.beatmap_timeline", "sys.scoring_combo"];

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
            .join("rhythm_micro")
            .join("pack.json"),
    )
    .unwrap()
}

/// 隔离环境：`knowledge/` 布局副本（两模块 + calibration/budget.json 真文件副本
/// + prompt_library 空种子）+ 设计空间（合成通用层 + 指定 pack 集）。
fn setup(tag: &str, packs: &[(&str, String)]) -> (PathBuf, AppServices) {
    let temp = std::env::temp_dir().join(format!("adm4_rhythm_e2e_{tag}_{}", std::process::id()));
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
    for module_id in RHYTHM_MODULES {
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

// ---------------------------------------------------------------------------
// 装配线：零悬空 + 判定事件供需边（正例 + 反例）+ 组合判定
// ---------------------------------------------------------------------------

#[test]
fn rhythm_micro_pack_assembles_with_judgement_supply_chain() {
    // 反例包：计分侧的判定事件绑定改指幽灵名词——V6 必须点名拦截。
    // pack_id 同步替换（目录即 pack_id），genre_scope 引用一并跟随。
    let broken_pack = real_pack_json()
        .replace("rhythm_micro", "rhythm_broken")
        .replace("chart_main.judgement_signal", "chart_main.ghost_signal");
    let (temp, services) = setup(
        "assembly",
        &[
            ("rhythm_micro", real_pack_json()),
            ("rhythm_broken", broken_pack),
        ],
    );

    // ---- 正例：装配成功零悬空（fail-closed：任何 V6/版本矛盾都会 Err）----
    let space = services.load_space("rhythm_micro").unwrap();
    assert_eq!(space.system_instances.len(), 2, "两实例全部装配");
    for (instance, module) in [
        ("chart_main", "sys.beatmap_timeline"),
        ("scoring_main", "sys.scoring_combo"),
    ] {
        let info = space
            .system_instances
            .iter()
            .find(|info| info.instance_id == instance)
            .unwrap_or_else(|| panic!("缺实例 {instance}"));
        assert_eq!(info.module_id, module);
        assert_eq!(info.semver, "1.0.0");
    }
    // tier 合成点齐备且档位数 = 模块阶梯档数（allowed_tiers 未收窄）。
    for (tier_point, options) in [("chart_main.tier", 3), ("scoring_main.tier", 4)] {
        let point = space
            .graph
            .point(tier_point)
            .unwrap_or_else(|| panic!("缺 tier 合成点 {tier_point}"));
        assert_eq!(point.options.len(), options, "{tier_point} 档位数不符");
    }
    // 命名空间重写后的模块点与 pack 层两决策点都在图上（含 BT2 变速点——全档装配）。
    for id in [
        "chart_main.judgement_window_table",
        "chart_main.note_chart_table",
        "chart_main.hit_matching_rule",
        "chart_main.long_note_rule",
        "chart_main.lane_layout",
        "chart_main.accuracy_curve",
        "chart_main.speed_change_rule",
        "scoring_main.score_rule",
        "scoring_main.combo_window",
        "scoring_main.decay_rule",
        "rhythm.song_roster",
        "scoring_session",
    ] {
        assert!(space.graph.point(id).is_some(), "装配后缺决策点 {id}");
    }

    // ---- 反例：绑定悬空即装配失败，V6 点名判定事件（供需边被门禁强制的机器证据）----
    let error = services
        .load_space("rhythm_broken")
        .expect_err("幽灵绑定必须装配失败");
    assert!(error.message.contains("V6"), "{}", error.message);
    assert!(
        error.message.contains("judgement_signal"),
        "应点名判定事件名词：{}",
        error.message
    );

    // ---- 组合判定：中核档 + 双中档声明（BT1 W=8 / K1 W=7 均低于重核线 9）----
    let archive_id = services
        .project_new("音游装配判定", "rhythm_micro", DesignLevel::L6, None)
        .unwrap();
    select_confirmed(&services, &archive_id, "u.target_scale", "midcore");
    select_confirmed(
        &services,
        &archive_id,
        "chart_main.tier",
        "bt1_multi_lane_holds",
    );
    select_confirmed(
        &services,
        &archive_id,
        "scoring_main.tier",
        "k1_combo_window",
    );

    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产组合报告");
    let report = &assessment.report;
    assert!(assessment.missing_tiers.is_empty(), "两档位已全部声明");
    // 零 block = V6 悬空消费未触发：scoring_main 消费的判定事件由 chart_main
    // provides 供给（供需链闭合的组合期证据）；V1/V2/V3/V4 同样零违例。
    assert!(
        report.blocks.is_empty(),
        "微样板组合应零硬违例，实际：{:?}",
        report.blocks
    );
    // 微样板刻意不进重核：|H|=0（BT1=8、K1=7 均为全局档带「中」），
    // 无数量提示也无署名确认义务——最小规模样板的形态如实声明。
    assert!(report.h_set.is_empty(), "实际 H：{:?}", report.h_set);
    assert!(report.advices.is_empty(), "实际：{:?}", report.advices);
    assert!(!report.form_confirmation_required);
    // B(G) = 8(chart, core×1.0) + 7(scoring, core×1.0) = 15 ≤ 中核预算 42。
    assert!(
        (report.budget_total - 15.0).abs() < 1e-9,
        "B(G) 应为 15.0，实际 {}",
        report.budget_total
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 全链线：建项 → tier → 全点作答 → 冻结 → C0-C6 → 供需链/Curve/判定窗断言
// ---------------------------------------------------------------------------

fn note_row(
    id: &str,
    time_ms: i64,
    lane: i64,
    note_type: &str,
    duration_ms: i64,
) -> BTreeMap<String, TypedValue> {
    [
        ("note_id".to_string(), text(id)),
        ("time_ms".to_string(), TypedValue::Int(time_ms)),
        ("lane".to_string(), TypedValue::Int(lane)),
        ("note_type".to_string(), text(note_type)),
        ("duration_ms".to_string(), TypedValue::Int(duration_ms)),
    ]
    .into_iter()
    .collect()
}

fn window_row(
    grade: &str,
    label: &str,
    window_ms: i64,
    weight: f64,
) -> BTreeMap<String, TypedValue> {
    [
        ("grade_key".to_string(), text(grade)),
        ("label".to_string(), text(label)),
        ("window_ms".to_string(), TypedValue::Int(window_ms)),
        ("accuracy_weight".to_string(), TypedValue::Float(weight)),
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

fn song_row(id: &str, title: &str, bpm: f64, duration: i64) -> BTreeMap<String, TypedValue> {
    [
        ("song_id".to_string(), text(id)),
        ("title".to_string(), text(title)),
        ("bpm".to_string(), TypedValue::Float(bpm)),
        ("duration_seconds".to_string(), TypedValue::Int(duration)),
    ]
    .into_iter()
    .collect()
}

/// 偏差→精度曲线（Curve 通路实证载荷）：x=|偏差 ms| 严格升序，linear 插值。
const ACCURACY_CURVE_VALUE: &str = r#"{"id":"chart_main.accuracy_curve","interpolation":"linear","points":[[0.0,1.0],[40.0,0.85],[80.0,0.6],[120.0,0.2]]}"#;

#[test]
fn rhythm_full_chain_scores_judgement_events_through_phase1() {
    let (temp, services) = setup("chain", &[("rhythm_micro", real_pack_json())]);
    let archive_id = services
        .project_new("音游微样板", "rhythm_micro", DesignLevel::L6, None)
        .unwrap();

    // ---- ① 画像 + tier 声明（谱面 BT1 中档 / 计分 K1 中档=连击+倍率衰减）----
    select_confirmed(&services, &archive_id, "u.target_scale", "midcore");
    select_confirmed(&services, &archive_id, "u.promise", "flow_mastery");
    select_confirmed(&services, &archive_id, "u.genre", "rhythm_game");
    select_confirmed(
        &services,
        &archive_id,
        "chart_main.tier",
        "bt1_multi_lane_holds",
    );
    select_confirmed(
        &services,
        &archive_id,
        "scoring_main.tier",
        "k1_combo_window",
    );

    // ---- ② 全部激活点作答（占位符参数全部由作者填写，I1）----
    services
        .with_project(&archive_id, |engine| {
            let manual = Provenance::UserManual;
            for (decision, option) in [
                // 谱面 BT1 六点。
                ("chart_main.judgement_window_table", "fixed_window_table"),
                ("chart_main.note_chart_table", "timestamp_notation"),
                ("chart_main.hit_matching_rule", "nearest_note_match"),
                ("chart_main.long_note_rule", "head_tail_double_judgement"),
                ("chart_main.lane_layout", "fixed_lane_count"),
                ("chart_main.accuracy_curve", "offset_accuracy_curve"),
                // 计分 K1 四点（timed_window 选型申报见文件头）。
                ("scoring_main.score_rule", "fixed_value_table"),
                ("scoring_main.combo_window", "timed_window"),
                ("scoring_main.multiplier_growth", "continuous_scaling"),
                ("scoring_main.decay_rule", "hard_reset"),
                // pack 两点。
                ("rhythm.song_roster", "basic_song_table"),
                ("scoring_session", "single_profile_table"),
            ] {
                engine.select_option(decision, option, manual.clone())?;
            }

            // 判定窗表（进 GameSpec 的断言对象）。
            let problems = engine.set_parameters(
                "chart_main.judgement_window_table",
                ParameterValues::Rows {
                    rows: vec![
                        window_row("perfect", "完美", 25, 1.0),
                        window_row("great", "很好", 60, 0.7),
                        window_row("good", "尚可", 110, 0.35),
                        window_row("miss", "错过", 180, 0.0),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "判定窗表应过校验：{problems:?}");
            // 音符表（谱面实体源）。
            let problems = engine.set_parameters(
                "chart_main.note_chart_table",
                ParameterValues::Rows {
                    rows: vec![
                        note_row("n001", 1000, 1, "tap", 0),
                        note_row("n002", 1500, 2, "tap", 0),
                        note_row("n003", 2000, 3, "hold", 800),
                        note_row("n004", 2500, 4, "tap", 0),
                        note_row("n005", 3000, 2, "slide", 400),
                        note_row("n006", 3500, 1, "tap", 0),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "音符表应过校验：{problems:?}");
            // Curve 值（波 1 通路：标量 curve 键装 CurveSpec JSON）。
            let problems = engine.set_parameters(
                "chart_main.accuracy_curve",
                scalars(&[("curve", text(ACCURACY_CURVE_VALUE))]),
            )?;
            assert!(problems.is_empty(), "曲线值应过校验：{problems:?}");

            let scalar_params: [(&str, ParameterValues); 5] = [
                (
                    "chart_main.hit_matching_rule",
                    scalars(&[
                        ("chart_table_id", text("chart_main.note_chart_table")),
                        ("window_table_id", text("chart_main.judgement_window_table")),
                    ]),
                ),
                (
                    "chart_main.long_note_rule",
                    scalars(&[
                        ("chart_table_id", text("chart_main.note_chart_table")),
                        ("window_table_id", text("chart_main.judgement_window_table")),
                    ]),
                ),
                (
                    "chart_main.lane_layout",
                    scalars(&[
                        ("chart_table_id", text("chart_main.note_chart_table")),
                        ("lane_count", TypedValue::Int(4)),
                    ]),
                ),
                (
                    "scoring_main.combo_window",
                    scalars(&[("window_seconds", TypedValue::Float(5.0))]),
                ),
                (
                    "scoring_main.multiplier_growth",
                    scalars(&[
                        ("growth_per_combo", TypedValue::Float(0.02)),
                        ("multiplier_cap", TypedValue::Float(8.0)),
                    ]),
                ),
            ];
            for (decision, parameters) in scalar_params {
                let problems = engine.set_parameters(decision, parameters)?;
                assert!(
                    problems.is_empty(),
                    "{decision} 参数应通过校验：{problems:?}"
                );
            }

            // 行为计分表：行键对齐判定等级（判定事件→分数的语义咬合在数据层可见）。
            let problems = engine.set_parameters(
                "scoring_main.score_rule",
                ParameterValues::Rows {
                    rows: vec![
                        score_row("perfect", "完美判定", 500),
                        score_row("great", "很好判定", 300),
                        score_row("good", "尚可判定", 100),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "计分表应过校验：{problems:?}");
            let problems = engine.set_parameters(
                "rhythm.song_roster",
                ParameterValues::Rows {
                    rows: vec![
                        song_row("tutorial_bop", "入门曲", 120.0, 95),
                        song_row("midnight_rush", "午夜疾走", 174.0, 128),
                        song_row("stardust_finale", "星屑终章", 200.0, 150),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "曲目表应过校验：{problems:?}");
            let problems = engine.set_parameters(
                "scoring_session",
                ParameterValues::Rows {
                    rows: vec![
                        [
                            ("session_id".to_string(), text("standard")),
                            ("combo_scope".to_string(), text("per_song")),
                        ]
                        .into_iter()
                        .collect(),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "会话档案表应过校验：{problems:?}");

            for decision in [
                "chart_main.judgement_window_table",
                "chart_main.note_chart_table",
                "chart_main.hit_matching_rule",
                "chart_main.long_note_rule",
                "chart_main.lane_layout",
                "chart_main.accuracy_curve",
                "scoring_main.score_rule",
                "scoring_main.combo_window",
                "scoring_main.multiplier_growth",
                "scoring_main.decay_rule",
                "rhythm.song_roster",
                "scoring_session",
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

    // ---- ③ 冻结：红队 → 五门全绿 → module_versions 锁两模块 ----
    let ai = ScriptedProvider::new();
    ai.script("freeze_red_team", vec![FREEZE_RED_TEAM_ANSWER.into()]);
    ai.script("c1_redteam", vec![C1_RED_TEAM_ANSWER.into()]);
    ai.script(
        "c2_narrative",
        vec![r#"{"text":"基于规格的玩法叙述：音符沿时间轴下落，输入在判定窗内配对产出判定事件；计分系统消费判定事件累积分数，时间窗内续连抬升倍率，断连即归零。"}"#.into()],
    );
    ai.script(
        "c3_asset_description",
        vec![r#"{"description":"高饱和霓虹风格的下落音符贴图，四轨布局，边缘发光，适配 2D 序列帧。"}"#.into()],
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
    for module_id in RHYTHM_MODULES {
        assert_eq!(
            frozen.module_versions.get(module_id).map(String::as_str),
            Some("1.0.0"),
            "冻结应锁定 {module_id} 版本"
        );
    }

    // ---- ④ C0-C6 全链（C5/C6 人工门）----
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

    // ---- ⑤ 产物断言 ----
    let content = services.archives.content_dir(&archive_id);
    let read_contract = |stage: &str| -> serde_json::Value {
        let raw =
            std::fs::read_to_string(content.join(format!("pipeline/v1/{stage}/contract.json")))
                .unwrap_or_else(|e| panic!("{stage} 契约应可读：{e}"));
        serde_json::from_str(&raw).unwrap()
    };

    // 断言 A：判定窗 Table 进 GameSpec（#29 的独占状态之一落为一等 spec 元素）。
    let spec = read_contract("C0");
    let tables = spec["tables"].as_array().unwrap();
    let window_table = tables
        .iter()
        .find(|table| table["id"] == "chart_main.judgement_window_table")
        .expect("判定窗表应进 GameSpec.tables");
    assert_eq!(window_table["row_key"], "grade_key");
    assert_eq!(window_table["rows"].as_array().unwrap().len(), 4);

    // 断言 B：Curve 通路实证（波 1）——两列 (x, y) Table + 插值注记。
    let curve_table = tables
        .iter()
        .find(|table| table["id"] == "chart_main.accuracy_curve")
        .expect("精度曲线应编译进 GameSpec.tables");
    let columns: Vec<&str> = curve_table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| column["key"].as_str().unwrap())
        .collect();
    assert_eq!(columns, vec!["x", "y"], "Curve 应编译为两列采样表");
    assert_eq!(curve_table["rows"].as_array().unwrap().len(), 4);
    let curve_note = curve_table["design_notes"][0]["text"].as_str().unwrap();
    assert!(curve_note.contains("插值"), "{curve_note}");
    assert!(curve_note.contains("linear"), "{curve_note}");

    // 断言 C：音符实体（sprite2d）与会话宿主实体（invisible 类前缀解析）都在。
    let entities = spec["entities"].as_array().unwrap();
    for entity_id in [
        "chart_main.note_chart_table.n001",
        "scoring_session.standard",
    ] {
        assert!(
            entities.iter().any(|entity| entity["id"] == entity_id),
            "GameSpec 缺实体 {entity_id}"
        );
    }

    // 断言 D：判定事件→分数供需链的机制面证据——谱面判定机制 emit judgement_signal，
    // 计分机制群到达 C4 且 GWT 非空（连击窗 + 倍率成长 + 衰减归零，作者公式在场）。
    let c4 = read_contract("C4");
    let capabilities = c4["capabilities"].as_array().unwrap();
    let scenario_then = |cap_id: &str| -> String {
        let capability = capabilities
            .iter()
            .find(|capability| capability["id"] == cap_id)
            .unwrap_or_else(|| panic!("C4 缺能力契约 {cap_id}"));
        let then = capability["scenarios"][0]["then"].as_array().unwrap();
        assert!(!then.is_empty(), "{cap_id} 的 GWT Then 不得为空");
        then.iter()
            .map(|item| item.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
            .join("；")
    };
    // 供给端：判定配对机制读音符表/判定窗表并发出判定事件。
    let matching_then = scenario_then("cap_chart_main.hit_matching_rule");
    assert!(
        matching_then.contains("chart_main.judgement_window_table"),
        "判定机制应引用作者填写的判定窗表：{matching_then}"
    );
    assert!(
        matching_then.contains("发出信号 judgement_signal"),
        "判定机制应发出判定事件（供需链供给端）：{matching_then}"
    );
    // 消费端：计分三机制的 GWT 非空且含作者公式（K1 连击+倍率+衰减齐备）。
    let combo_then = scenario_then("cap_scoring_main.combo_window");
    assert!(
        combo_then.contains("combo + 1 if within(5)"),
        "连击窗公式应含作者窗长：{combo_then}"
    );
    assert!(
        combo_then.contains("延迟 5 秒"),
        "断连倒计时的 Schedule 渲染应在场：{combo_then}"
    );
    let multiplier_then = scenario_then("cap_scoring_main.multiplier_growth");
    assert!(
        multiplier_then.contains("min(1 + combo_count * 0.02, 8)"),
        "倍率成长公式应含作者系数与封顶：{multiplier_then}"
    );
    let decay_then = scenario_then("cap_scoring_main.decay_rule");
    assert!(
        decay_then.contains("0 on break_condition"),
        "衰减归零公式应在场：{decay_then}"
    );
    let score_then = scenario_then("cap_scoring_main.score_rule");
    assert!(
        score_then.contains("score_pool"),
        "计分机制应向分数资源入账（供需链消费端落点）：{score_then}"
    );

    std::fs::remove_dir_all(&temp).ok();
}
