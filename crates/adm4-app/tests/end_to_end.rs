//! 端到端集成测试：设计空间校验 → 创建项目 → 手动创作 → 红队 → 冻结门五道 →
//! C0-C6 全链（确定性脚本 AI）→ 两个人工门确认 → 全绿。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, InterviewTurnDto, save_config};
use adm4_archive::DataRoot;
use adm4_authoring::TemplateMode;
use adm4_contracts::{MatrixCell, TypedValue};
use adm4_decision::{DesignLevel, NaJustification, ParameterValues, Provenance};
use adm4_pipeline::StageStatus;
use adm4_template::{CROSSCHECK_PURPOSE, CertificationStatus, MAPPING_PURPOSE, load_skin_wordlist};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn design_space_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
        .join("design_space")
}

/// 冻结门红队应答；C1 红队若原样复读这段文本，两份 ReviewProof 哈希全同 → R3 橡皮图章。
const FREEZE_RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"全部 15 条决策交叉","conclusion":"未发现矛盾"}]}"#;

fn scripted_ai() -> ScriptedProvider {
    let provider = ScriptedProvider::new();
    provider.script("freeze_red_team", vec![FREEZE_RED_TEAM_ANSWER.into()]);
    provider.script(
        "c1_redteam",
        vec![
            r#"{"findings":[{"id":"w1","severity":"warning","target":"mechanics/ld.income_rule","text":"回复节奏与部署成本的匹配需要试玩验证"}],"per_category":[{"category":"feasibility","checked":"3 条机制逐条","conclusion":"均可实现"}]}"#.into(),
        ],
    );
    provider.script(
        "c2_narrative",
        vec![r#"{"text":"基于规格的玩法叙述：玩家在通道上部署守卫，利用克制系数放大伤害，抵御脚本化波次。"}"#.into()],
    );
    provider.script(
        "c3_asset_description",
        vec![
            r#"{"description":"扁平卡通风格的角色立绘，正面站姿，边缘描边，适配 2D 序列帧。"}"#
                .into(),
        ],
    );
    provider.script(
        "c4_interface_naming",
        vec![r#"{"interface_name":"MechanicExecutionService"}"#.into()],
    );
    provider
}

fn scalars(pairs: &[(&str, TypedValue)]) -> ParameterValues {
    ParameterValues::Scalars {
        entries: pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect(),
    }
}

fn guard_row(id: &str, cost: i64, attack: i64, interval: f64) -> BTreeMap<String, TypedValue> {
    [
        ("id".to_string(), TypedValue::Text(id.into())),
        ("cost".to_string(), TypedValue::Int(cost)),
        ("attack".to_string(), TypedValue::Int(attack)),
        ("attack_interval".to_string(), TypedValue::Float(interval)),
    ]
    .into_iter()
    .collect()
}

fn enemy_row(id: &str, hp: i64, speed: f64) -> BTreeMap<String, TypedValue> {
    [
        ("id".to_string(), TypedValue::Text(id.into())),
        ("hp".to_string(), TypedValue::Int(hp)),
        ("speed".to_string(), TypedValue::Float(speed)),
    ]
    .into_iter()
    .collect()
}

fn wave_row(wave: i64, enemy: &str, count: i64, interval: f64) -> BTreeMap<String, TypedValue> {
    [
        ("wave".to_string(), TypedValue::Int(wave)),
        ("enemy_id".to_string(), TypedValue::Text(enemy.into())),
        ("count".to_string(), TypedValue::Int(count)),
        ("interval_seconds".to_string(), TypedValue::Float(interval)),
    ]
    .into_iter()
    .collect()
}

#[test]
fn full_chain_from_space_to_signed_phase1() {
    // 隔离数据根 + 指向仓内设计空间。
    let temp = std::env::temp_dir().join(format!("adm4_e2e_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let data_root = DataRoot::new(&temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: design_space_root().to_string_lossy().into_owned(),
            ai_provider: None,
        },
    )
    .unwrap();
    let services = AppServices::open(Some(temp.clone())).unwrap();

    // 1. 设计空间加载即校验（违例会直接 Err）。
    let space = services.load_space("lane_defense").unwrap();
    assert!(space.graph.points().len() >= 14);

    // 2. 创建 L6 深度档项目。
    let archive_id = services
        .project_new("生态穹顶防线", "lane_defense", DesignLevel::L6, None)
        .unwrap();

    // 3. 手动创作：结构层 + 参数表层全部填齐。
    services
        .with_project(&archive_id, |engine| {
            let manual = Provenance::UserManual;
            for (decision, option) in [
                ("u.business_model", "premium"),
                ("u.platform", "pc_single"),
                ("u.experience", "guardian_underdog"),
                ("u.genre", "lane_defense"),
                ("ld.combat_system", "counter_combat"),
                ("ld.deploy_system", "grid_deploy"),
                ("ld.wave_system", "scripted_waves"),
                ("ld.economy_system", "regen_resource"),
                ("ld.counter_damage", "multiplier_formula"),
                ("ld.deploy_cost", "cost_gate"),
                ("ld.income_rule", "periodic_income"),
                ("ld.guard_roster", "guard_table"),
                ("ld.enemy_roster", "enemy_table"),
                ("ld.counter_matrix", "matrix_full"),
                ("ld.wave_table", "wave_rows"),
            ] {
                engine
                    .select_option(decision, option, manual.clone())
                    .unwrap();
            }

            engine
                .set_parameters(
                    "u.experience",
                    scalars(&[(
                        "statement",
                        TypedValue::Text("以有限资源守护脆弱的生态穹顶，从濒危走向掌控".into()),
                    )]),
                )
                .unwrap();
            engine
                .set_parameters(
                    "ld.counter_damage",
                    scalars(&[("base_multiplier", TypedValue::Float(2.0))]),
                )
                .unwrap();
            engine
                .set_parameters(
                    "ld.deploy_cost",
                    scalars(&[("refund_ratio", TypedValue::Float(0.8))]),
                )
                .unwrap();
            engine
                .set_parameters(
                    "ld.income_rule",
                    scalars(&[
                        ("interval_seconds", TypedValue::Float(5.0)),
                        ("amount", TypedValue::Int(25)),
                    ]),
                )
                .unwrap();
            engine
                .set_parameters(
                    "ld.guard_roster",
                    ParameterValues::Rows {
                        rows: vec![
                            guard_row("thorn_archer", 100, 12, 1.2),
                            guard_row("mist_mage", 150, 20, 1.8),
                            guard_row("stone_ward", 75, 4, 2.0),
                            guard_row("sun_harvester", 50, 0, 3.0),
                        ],
                    },
                )
                .unwrap();
            engine
                .set_parameters(
                    "ld.enemy_roster",
                    ParameterValues::Rows {
                        rows: vec![enemy_row("crawler", 60, 1.0), enemy_row("glider", 40, 2.2)],
                    },
                )
                .unwrap();
            let mut cells = Vec::new();
            for guard in ["thorn_archer", "mist_mage", "stone_ward", "sun_harvester"] {
                for enemy in ["crawler", "glider"] {
                    cells.push(MatrixCell {
                        row: guard.into(),
                        col: enemy.into(),
                        value: TypedValue::Float(if guard == "mist_mage" && enemy == "glider" {
                            2.5
                        } else {
                            1.0
                        }),
                    });
                }
            }
            engine
                .set_parameters("ld.counter_matrix", ParameterValues::Cells { cells })
                .unwrap();
            engine
                .set_parameters(
                    "ld.wave_table",
                    ParameterValues::Rows {
                        rows: vec![
                            wave_row(1, "crawler", 5, 2.0),
                            wave_row(2, "crawler", 8, 1.6),
                            wave_row(3, "glider", 4, 1.5),
                            wave_row(4, "crawler", 10, 1.2),
                            wave_row(5, "glider", 8, 1.0),
                        ],
                    },
                )
                .unwrap();

            for decision in [
                "u.business_model",
                "u.platform",
                "u.experience",
                "u.genre",
                "ld.combat_system",
                "ld.deploy_system",
                "ld.wave_system",
                "ld.economy_system",
                "ld.counter_damage",
                "ld.deploy_cost",
                "ld.income_rule",
                "ld.guard_roster",
                "ld.enemy_roster",
                "ld.counter_matrix",
                "ld.wave_table",
            ] {
                engine.confirm_selection(decision).unwrap();
            }
            let report = engine.completeness();
            assert!(report.is_complete(), "blocking: {:?}", report.blocking);
            Ok(())
        })
        .unwrap();

    // 4. 未红队前冻结必须被拒（门 4）。
    let premature = services.freeze_run(&archive_id);
    assert!(premature.is_err());

    // 5. 红队 + 冻结门五道。
    let ai = scripted_ai();
    services.freeze_red_team_with(&archive_id, &ai).unwrap();
    let report = services.freeze_check(&archive_id).unwrap();
    assert!(report.all_passed(), "gates: {:?}", report.gates);
    let frozen = services.freeze_run(&archive_id).unwrap();
    assert_eq!(frozen.version, 1);
    assert!(frozen.content_hash.starts_with("sha256:"));

    // 6. R3：C1 红队若原样复读冻结门红队的应答（同批内容哈希全同）→ 判橡皮图章，C1 失败。
    let rubber_stamp = scripted_ai();
    rubber_stamp.script("c1_redteam", vec![FREEZE_RED_TEAM_ANSWER.into()]);
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &rubber_stamp)
        .unwrap();
    match state.stage_status("C1") {
        StageStatus::Failed { reasons } => assert!(
            reasons.iter().any(|reason| reason.contains("rubber stamp")),
            "C1 失败原因应指出橡皮图章：{reasons:?}"
        ),
        other => panic!("C1 应因 R3 橡皮图章失败，实际 {other:?}"),
    }

    // 7. C0-C6：换回互异的红队应答，第一次完整运行停在 C5 人工门。
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
    assert!(matches!(
        state.stage_status("C5"),
        StageStatus::WaitingHuman { .. }
    ));

    // 7. 确认 C5，跑到 C6 人工签收，确认后全绿。
    services
        .pipeline_confirm(&archive_id, "C5", "测试评审员", "风格方向确认")
        .unwrap();
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    assert!(matches!(
        state.stage_status("C6"),
        StageStatus::WaitingHuman { .. }
    ));
    let state = services
        .pipeline_confirm(&archive_id, "C6", "测试评审员", "Phase 1 文档集签收")
        .unwrap();
    for stage in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        assert!(matches!(state.stage_status(stage), StageStatus::Succeeded));
    }

    // 8. 产物落盘核验：双格式 + 冻结哈希绑定。
    let content = services.archives.content_dir(&archive_id);
    let spec_text = std::fs::read_to_string(content.join("pipeline/v1/C0/contract.json")).unwrap();
    assert!(spec_text.contains(&frozen.content_hash));
    assert!(content.join("pipeline/v1/C6/document.md").is_file());

    // 9. 存档体检 + 导出导入回路。
    assert!(services.archives.doctor(&archive_id).unwrap().is_empty());
    let package = temp.join("export.adm4proj");
    let exported = services.export_project(&archive_id, &package).unwrap();
    assert!(exported >= 10);
    let imported_id = services.import_project(&package, "导入副本").unwrap();
    assert!(services.archives.doctor(&imported_id).unwrap().is_empty());

    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn red_lines_hold_in_end_to_end_paths() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_rl_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let data_root = DataRoot::new(&temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: design_space_root().to_string_lossy().into_owned(),
            ai_provider: None,
        },
    )
    .unwrap();
    let services = AppServices::open(Some(temp.clone())).unwrap();
    let archive_id = services
        .project_new("红线验证", "lane_defense", DesignLevel::L4, None)
        .unwrap();

    // R7：AI 未配置 → 红队显式失败（AiUnavailable），不静默兜底。
    let result = services.freeze_red_team(&archive_id);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind,
        adm4_foundation::Adm4ErrorKind::AiUnavailable
    );

    // 完成度：未确认的选择不计入（AI 提案/模板预填必须用户确认）。
    services
        .with_project(&archive_id, |engine| {
            engine
                .select_option("u.genre", "lane_defense", Provenance::UserManual)
                .unwrap();
            // baseline 点显式 N/A 需要结构化理由码。
            let bad = engine.mark_not_applicable(
                "u.business_model",
                NaJustification {
                    reason_code: "  ".into(),
                    note: String::new(),
                },
            );
            assert!(bad.is_err());
            Ok(())
        })
        .unwrap();

    // 换皮门：理由文本命中参考游戏名 → 冻结检查 block。
    services
        .with_project(&archive_id, |engine| {
            engine
                .select_option("u.experience", "guardian_underdog", Provenance::UserManual)
                .unwrap();
            engine
                .set_parameters(
                    "u.experience",
                    ParameterValues::Scalars {
                        entries: [(
                            "statement".to_string(),
                            TypedValue::Text("像 Plants vs Zombies 一样守护后院".into()),
                        )]
                        .into_iter()
                        .collect(),
                    },
                )
                .unwrap();
            engine.confirm_selection("u.experience").unwrap();
            Ok(())
        })
        .unwrap();
    let report = services.freeze_check(&archive_id).unwrap();
    let skin_gate = report
        .gates
        .iter()
        .find(|gate| gate.gate == "gate3_skin")
        .unwrap();
    assert!(!skin_gate.passed);
    assert!(
        skin_gate
            .findings
            .iter()
            .any(|finding| finding.code == "reference_name_hit")
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// T5 新增场景：逆向五步产出 Certified 模板（词表登记）→ 模板预填新项目 →
// 访谈补齐剩余决策（整表提案 + 例外下钻 + 拒绝重提）→ 冻结门全绿 → C0-C6 全绿。
// ---------------------------------------------------------------------------

/// 设计空间复制到临时目录：模板落库与词表登记均写临时副本，不污染仓库数据。
fn copy_dir_recursive(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap().flatten() {
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &destination);
        } else {
            std::fs::copy(&path, &destination).unwrap();
        }
    }
}

fn services_with_isolated_space(temp: &Path) -> AppServices {
    std::fs::remove_dir_all(temp).ok();
    let space_root = temp.join("design_space");
    copy_dir_recursive(&design_space_root(), &space_root);
    let data_root = DataRoot::new(temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: space_root.to_string_lossy().into_owned(),
            ai_provider: None,
        },
    )
    .unwrap();
    AppServices::open(Some(temp.to_path_buf())).unwrap()
}

/// 本地语料快照：虚构逆向目标「晨昏防线」的四份抓取候选。
/// 关键词设计：克制/网格 命中前两条，波次/回复 命中后两条（验证两轮检索累积去重）。
const CORPUS_SNAPSHOT: &str = r#"[
  {
    "source_url": "https://wiki.example/dawnline/combat",
    "title": "战斗机制综述",
    "snippet": "克制系数与基础伤害倍率 2.0，克制矩阵决定守卫与敌人的强弱关系",
    "source_type": "wiki",
    "fetched_hash": "sha256:combat"
  },
  {
    "source_url": "https://official.example/dawnline/deploy",
    "title": "官方指南：守卫放置",
    "snippet": "守卫放置于固定网格位，消耗资源，撤除返还比例 0.8",
    "source_type": "official",
    "fetched_hash": "sha256:deploy"
  },
  {
    "source_url": "https://wiki.example/dawnline/waves",
    "title": "出怪规则",
    "snippet": "脚本化波次表，敌人按预设顺序与间隔出场",
    "source_type": "wiki",
    "fetched_hash": "sha256:waves"
  },
  {
    "source_url": "https://official.example/dawnline/economy",
    "title": "官方指南：资源系统",
    "snippet": "资源每 5 秒周期回复 25 点，用于换取守卫",
    "source_type": "official",
    "fetched_hash": "sha256:economy"
  }
]"#;

/// S2 映射脚本：8 条答案（结构层到 L4），每条挂候选池内来源（R1/R4）。
const MAPPING_ANSWERS: &str = r#"[
  {"decision_id":"u.genre","option_id":"lane_defense","evidence":[{"source_url":"https://wiki.example/dawnline/combat","quote":"克制矩阵决定守卫与敌人的强弱关系","confidence":"med"}],"notes":"整体结构为通道塔防"},
  {"decision_id":"ld.combat_system","option_id":"counter_combat","evidence":[{"source_url":"https://wiki.example/dawnline/combat","confidence":"high"}]},
  {"decision_id":"ld.deploy_system","option_id":"grid_deploy","evidence":[{"source_url":"https://official.example/dawnline/deploy","confidence":"high"}]},
  {"decision_id":"ld.wave_system","option_id":"scripted_waves","evidence":[{"source_url":"https://wiki.example/dawnline/waves","confidence":"high"}]},
  {"decision_id":"ld.economy_system","option_id":"regen_resource","evidence":[{"source_url":"https://official.example/dawnline/economy","confidence":"high"}]},
  {"decision_id":"ld.counter_damage","option_id":"multiplier_formula","evidence":[{"source_url":"https://wiki.example/dawnline/combat","quote":"基础伤害倍率 2.0","confidence":"med"}],"parameters":{"base_multiplier":2.0}},
  {"decision_id":"ld.deploy_cost","option_id":"cost_gate","evidence":[{"source_url":"https://official.example/dawnline/deploy","quote":"撤除返还比例 0.8","confidence":"med"}],"parameters":{"refund_ratio":0.8}},
  {"decision_id":"ld.income_rule","option_id":"periodic_income","evidence":[{"source_url":"https://official.example/dawnline/economy","quote":"每 5 秒周期回复 25 点","confidence":"med"}],"parameters":{"interval_seconds":5.0,"amount":25}}
]"#;

/// S3 交叉核验脚本：逐条覆盖全部 8 个决策点，全部一致。
const CROSSCHECK_ALL_CONSISTENT: &str = r#"[
  {"decision_id":"u.genre","verdict":"consistent","reason":"品类结构与来源一致"},
  {"decision_id":"ld.combat_system","verdict":"consistent","reason":"克制战斗有直接来源"},
  {"decision_id":"ld.deploy_system","verdict":"consistent","reason":"网格部署有官方来源"},
  {"decision_id":"ld.wave_system","verdict":"consistent","reason":"脚本化波次有来源"},
  {"decision_id":"ld.economy_system","verdict":"consistent","reason":"周期回复有官方来源"},
  {"decision_id":"ld.counter_damage","verdict":"consistent","reason":"倍率数值与引文一致"},
  {"decision_id":"ld.deploy_cost","verdict":"consistent","reason":"返还比例与引文一致"},
  {"decision_id":"ld.income_rule","verdict":"consistent","reason":"回复节奏与引文一致"}
]"#;

/// 访谈脚本：按「L 层升序 + 同层拓扑序 + 被拒点排同层末尾」的确定性顺序逐条应答。
/// 顺序：business_model(被拒) → platform → business_model(重提) → experience →
/// counter_matrix(被拒) → guard_roster → enemy_roster → counter_matrix(重提) → wave_table。
fn interview_scripts() -> Vec<String> {
    let premium = r#"{"option_id":"premium","rationale":"单机塔防以一次性交付内容为宜"}"#;
    let matrix = r#"{"option_id":"matrix_full","rationale":"全量矩阵便于精细调平","parameters":{"cells":[
        {"row":"thorn_archer","col":"crawler","value":1.0},{"row":"thorn_archer","col":"glider","value":1.0},
        {"row":"mist_mage","col":"crawler","value":1.0},{"row":"mist_mage","col":"glider","value":2.5},
        {"row":"stone_ward","col":"crawler","value":1.0},{"row":"stone_ward","col":"glider","value":1.0},
        {"row":"sun_harvester","col":"crawler","value":1.0},{"row":"sun_harvester","col":"glider","value":1.0},
        {"row":"bramble_guard","col":"crawler","value":1.5},{"row":"bramble_guard","col":"glider","value":1.0}
    ]}}"#;
    vec![
        premium.into(),
        r#"{"option_id":"pc_single","rationale":"键鼠精确操作适合布防"}"#.into(),
        premium.into(),
        r#"{"option_id":"guardian_underdog","rationale":"守护脆弱目标的压力曲线契合品类体验","parameters":{"statement":"以有限守卫资源保卫家园，从被动防御走向全面掌控"}}"#.into(),
        matrix.into(),
        r#"{"option_id":"guard_table","rationale":"四类守卫覆盖输出、控制、经济与肉盾","parameters":{"rows":[
            {"id":"thorn_archer","cost":100,"attack":12,"attack_interval":1.2},
            {"id":"mist_mage","cost":150,"attack":20,"attack_interval":1.8},
            {"id":"stone_ward","cost":75,"attack":4,"attack_interval":2.0},
            {"id":"sun_harvester","cost":50,"attack":0,"attack_interval":3.0}
        ]}}"#.into(),
        r#"{"option_id":"enemy_table","rationale":"先以双敌人验证攻防节奏","parameters":{"rows":[
            {"id":"crawler","hp":60,"speed":1.0},{"id":"glider","hp":40,"speed":2.2}
        ]}}"#.into(),
        matrix.into(),
        r#"{"option_id":"wave_rows","rationale":"五波由浅入深压测防线","parameters":{"rows":[
            {"wave":1,"enemy_id":"crawler","count":5,"interval_seconds":2.0},
            {"wave":2,"enemy_id":"crawler","count":8,"interval_seconds":1.6},
            {"wave":3,"enemy_id":"glider","count":4,"interval_seconds":1.5},
            {"wave":4,"enemy_id":"crawler","count":10,"interval_seconds":1.2},
            {"wave":5,"enemy_id":"glider","count":8,"interval_seconds":1.0}
        ]}}"#.into(),
    ]
}

#[test]
fn reverse_template_prefill_interview_freeze_and_pipeline_full_chain() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_t5_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);

    // 本地语料：虚构逆向目标「晨昏防线」的抓取快照。
    let corpus_root = temp.join("corpus");
    let game_dir = corpus_root.join("晨昏防线");
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("snapshot.json"), CORPUS_SNAPSHOT).unwrap();

    let ai = scripted_ai();
    ai.script(MAPPING_PURPOSE, vec![MAPPING_ANSWERS.into()]);
    ai.script(CROSSCHECK_PURPOSE, vec![CROSSCHECK_ALL_CONSISTENT.into()]);
    ai.script("interview_proposal", interview_scripts());

    // === 逆向产线五步 ===
    // S0 建草稿。
    let draft = services
        .template_new_draft(
            "lane_defense",
            "tpl_dawnline",
            "晨昏防线",
            &["Dawnline Defense".to_string()],
            DesignLevel::L4,
        )
        .unwrap();
    assert_eq!(draft.certification.status, CertificationStatus::Draft);

    // S1 两轮语料检索，候选池累积去重。
    let first = services
        .template_search_corpus(
            "lane_defense",
            "tpl_dawnline",
            &corpus_root,
            "战斗与部署结构",
            &["克制".to_string(), "网格".to_string()],
        )
        .unwrap();
    assert_eq!(first.len(), 2);
    let second = services
        .template_search_corpus(
            "lane_defense",
            "tpl_dawnline",
            &corpus_root,
            "波次与经济",
            &["波次".to_string(), "回复".to_string()],
        )
        .unwrap();
    assert_eq!(second.len(), 2);

    // S2 AI 映射：8 条答案，Draft→Mapped。
    let mapped = services
        .template_map_with("lane_defense", "tpl_dawnline", &ai)
        .unwrap();
    assert_eq!(mapped, 8);

    // S3 独立二次会话交叉核验，Mapped→CrossChecked。
    let report = services
        .template_cross_check_with("lane_defense", "tpl_dawnline", &ai)
        .unwrap();
    assert_eq!(report.entries.len(), 8);
    assert!(report.conflict_ids().is_empty());

    // 负例（认证前）：未认证模板预填必须被拒（取用关卡 approved_for_prefill）。
    let premature = services.project_new(
        "偷跑项目",
        "lane_defense",
        DesignLevel::L6,
        Some("tpl_dawnline"),
    );
    assert_eq!(
        premature.unwrap_err().kind,
        adm4_foundation::Adm4ErrorKind::Blocked
    );

    // S4 人工审核（署名 + 结论，R3）。
    let reviewed = services
        .template_review(
            "lane_defense",
            "tpl_dawnline",
            "评审员甲",
            "抽查证据链与核验结论，全部一致，可入库",
        )
        .unwrap();
    assert_eq!(
        reviewed.certification.status,
        CertificationStatus::HumanReviewed
    );

    // S5 认证入库：Certified + 换皮词表自动登记 game_name 与 aliases（R5）。
    let certified = services
        .template_certify("lane_defense", "tpl_dawnline")
        .unwrap();
    assert!(certified.is_certified());
    let words = load_skin_wordlist(&services.skin_wordlist_path())
        .unwrap()
        .words;
    assert!(words.contains(&"晨昏防线".to_string()), "{words:?}");
    assert!(words.contains(&"Dawnline Defense".to_string()), "{words:?}");

    // === 认证模板预填新项目 ===
    let archive_id = services
        .project_new(
            "霜落峡谷防卫计划",
            "lane_defense",
            DesignLevel::L6,
            Some("tpl_dawnline"),
        )
        .unwrap();
    let state = services.load_authoring_state(&archive_id).unwrap();
    assert_eq!(state.selections.len(), 8);
    assert!(
        state
            .selections
            .values()
            .all(|selection| !selection.confirmed_by_user),
        "预填条目必须待用户逐条确认"
    );
    assert!(matches!(
        state.template_mode,
        TemplateMode::Prefilled { .. }
    ));

    // 对照查询（对照模式数据源）：模板答卷与项目当前选择并排。
    let comparison = services
        .template_compare(&archive_id, "tpl_dawnline")
        .unwrap();
    assert_eq!(comparison.entries.len(), 8);
    assert!(comparison.entries.iter().all(|entry| entry.same_option));

    // 用户过卷确认预填条目（保留模板理由）→ 换皮门必须命中模板游戏名（R5）。
    let prefilled: Vec<String> = state.selections.keys().cloned().collect();
    services
        .with_project(&archive_id, |engine| {
            for id in &prefilled {
                engine.confirm_selection(id)?;
            }
            Ok(())
        })
        .unwrap();
    let gate_report = services.freeze_check(&archive_id).unwrap();
    let skin_gate = gate_report
        .gates
        .iter()
        .find(|gate| gate.gate == "gate3_skin")
        .unwrap();
    assert!(
        skin_gate
            .findings
            .iter()
            .any(|finding| finding.code == "reference_name_hit"),
        "预填理由带模板游戏名，换皮门必须拦截：{:?}",
        skin_gate.findings
    );
    // 换皮：重写理由后换皮门放行。
    services
        .with_project(&archive_id, |engine| {
            for id in &prefilled {
                engine.set_rationale(id, "沿用成熟结构，参数已按本作节奏重新校准")?;
            }
            Ok(())
        })
        .unwrap();

    // === AI 访谈补齐剩余决策 ===
    let progress = services.interview_progress(&archive_id).unwrap();
    assert_eq!(progress.current_level, Some(DesignLevel::L0));

    // 回合 1：L0 拓扑序首点 u.business_model，用户拒绝（AI 永不代提交）。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    assert!(matches!(turn, InterviewTurnDto::StructuralPoint { .. }));
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "u.business_model");
    services
        .interview_reject(&archive_id, &proposal.decision_id, "先看平台再定商业模式")
        .unwrap();
    assert!(
        !services
            .load_authoring_state(&archive_id)
            .unwrap()
            .selections
            .contains_key("u.business_model"),
        "拒绝不产生任何选择"
    );

    // 回合 2：被拒点排同层末尾 → 轮到 u.platform，确认。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "u.platform");
    assert!(
        services
            .interview_confirm(&archive_id, &proposal, None)
            .unwrap()
            .is_empty()
    );

    // 回合 3：同层只剩被拒点 → 重提 u.business_model，确认。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "u.business_model");
    assert!(
        services
            .interview_confirm(&archive_id, &proposal, None)
            .unwrap()
            .is_empty()
    );

    // 回合 4：L0 全确认后进 L1（体验幻想，标量参数）。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "u.experience");
    assert!(
        services
            .interview_confirm(&archive_id, &proposal, None)
            .unwrap()
            .is_empty()
    );

    // 回合 5：L5 拓扑序首点是克制矩阵——先拒绝，等守卫/敌人名单就绪。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    assert!(matches!(turn, InterviewTurnDto::TableProposal { .. }));
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "ld.counter_matrix");
    services
        .interview_reject(&archive_id, &proposal.decision_id, "先定名单表再定矩阵")
        .unwrap();

    // 回合 6：守卫表整表提案 → 例外下钻确认（改 stone_ward 造价 + 新增 bramble_guard）。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    assert!(matches!(turn, InterviewTurnDto::TableProposal { .. }));
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "ld.guard_roster");
    let overrides = ParameterValues::Rows {
        rows: vec![
            guard_row("thorn_archer", 100, 12, 1.2),
            guard_row("mist_mage", 150, 20, 1.8),
            guard_row("stone_ward", 60, 4, 2.0),
            guard_row("sun_harvester", 50, 0, 3.0),
            guard_row("bramble_guard", 90, 8, 1.6),
        ],
    };
    assert!(
        services
            .interview_confirm(&archive_id, &proposal, Some(overrides))
            .unwrap()
            .is_empty()
    );

    // 回合 7：敌人表整表确认。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "ld.enemy_roster");
    assert!(
        services
            .interview_confirm(&archive_id, &proposal, None)
            .unwrap()
            .is_empty()
    );

    // 回合 8：同层只剩被拒的矩阵 → 重提并确认（行集与下钻后的守卫表一致）。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "ld.counter_matrix");
    assert!(
        services
            .interview_confirm(&archive_id, &proposal, None)
            .unwrap()
            .is_empty()
    );

    // 回合 9：L6 波次表整表确认。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    assert!(matches!(turn, InterviewTurnDto::TableProposal { .. }));
    let proposal = turn.proposal().unwrap().clone();
    assert_eq!(proposal.decision_id, "ld.wave_table");
    assert!(
        services
            .interview_confirm(&archive_id, &proposal, None)
            .unwrap()
            .is_empty()
    );

    // 回合 10：全部激活点确认完毕。
    let turn = services.interview_next_with(&archive_id, &ai).unwrap();
    assert!(matches!(turn, InterviewTurnDto::Complete));
    assert!(
        services
            .interview_progress(&archive_id)
            .unwrap()
            .is_complete()
    );

    // 访谈状态随 authoring_state 持久化：拒绝记录 + 例外下钻摘要都在 transcript。
    let state = services.load_authoring_state(&archive_id).unwrap();
    assert!(
        state
            .interview
            .transcript
            .iter()
            .any(|entry| entry.role == "user_reject")
    );
    let drill = state
        .interview
        .transcript
        .iter()
        .rev()
        .find(|entry| entry.role == "user_confirm" && entry.content.contains("例外下钻"))
        .unwrap();
    assert!(
        drill.content.contains("新增行 bramble_guard"),
        "{}",
        drill.content
    );
    assert!(
        drill.content.contains("行 stone_ward 列 cost：75 → 60"),
        "{}",
        drill.content
    );

    // === 冻结门五道全绿 → 冻结 ===
    services.freeze_red_team_with(&archive_id, &ai).unwrap();
    let gate_report = services.freeze_check(&archive_id).unwrap();
    assert!(gate_report.all_passed(), "gates: {:?}", gate_report.gates);
    let frozen = services.freeze_run(&archive_id).unwrap();
    assert_eq!(frozen.version, 1);

    // === C0-C6 全绿（C5/C6 人工门确认） ===
    let run_state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    assert!(matches!(
        run_state.stage_status("C5"),
        StageStatus::WaitingHuman { .. }
    ));
    services
        .pipeline_confirm(&archive_id, "C5", "测试评审员", "风格方向确认")
        .unwrap();
    let run_state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    assert!(matches!(
        run_state.stage_status("C6"),
        StageStatus::WaitingHuman { .. }
    ));
    let run_state = services
        .pipeline_confirm(&archive_id, "C6", "测试评审员", "Phase 1 文档集签收")
        .unwrap();
    for stage in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        assert!(
            matches!(run_state.stage_status(stage), StageStatus::Succeeded),
            "{stage}: {:?}",
            run_state.stage_status(stage)
        );
    }

    // 新命令的 RunLog 类别落盘（template / interview）。
    let entries = services.log.tail(500).unwrap();
    assert!(entries.iter().any(|entry| entry.category == "template"));
    assert!(entries.iter().any(|entry| entry.category == "interview"));

    std::fs::remove_dir_all(&temp).ok();
}

/// 跨表外键（row_reference）：波次行的 enemy_id 必须落在敌人名单的行键集合内。
/// 悬空引用要同时被完成度（待填清单）与冻结门第 2 道（一致性门）拦下，
/// 且信息必须点名「哪张表哪一行哪个值」。
#[test]
fn dangling_row_reference_blocks_completeness_and_consistency_gate() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_fk_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let data_root = DataRoot::new(&temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: design_space_root().to_string_lossy().into_owned(),
            ai_provider: None,
        },
    )
    .unwrap();
    let services = AppServices::open(Some(temp.clone())).unwrap();
    let archive_id = services
        .project_new("外键校验项目", "lane_defense", DesignLevel::L6, None)
        .unwrap();

    // 波次表引用了敌人名单里没有的 ghost_swarm。
    services
        .with_project(&archive_id, |engine| {
            for (decision, option) in [
                ("ld.wave_system", "scripted_waves"),
                ("ld.enemy_roster", "enemy_table"),
                ("ld.wave_table", "wave_rows"),
            ] {
                engine.select_option(decision, option, Provenance::UserManual)?;
            }
            engine.set_parameters(
                "ld.enemy_roster",
                ParameterValues::Rows {
                    rows: vec![enemy_row("crawler", 60, 1.0), enemy_row("glider", 40, 2.2)],
                },
            )?;
            engine.set_parameters(
                "ld.wave_table",
                ParameterValues::Rows {
                    rows: vec![
                        wave_row(1, "crawler", 5, 2.0),
                        wave_row(2, "ghost_swarm", 8, 1.6),
                        wave_row(3, "glider", 4, 1.5),
                        wave_row(4, "crawler", 10, 1.2),
                        wave_row(5, "glider", 8, 1.0),
                    ],
                },
            )?;
            Ok(())
        })
        .unwrap();

    let engine = services.open_engine(&archive_id).unwrap();
    let blocking = engine.completeness().blocking;
    let dangling = blocking
        .iter()
        .find(|item| item.detail.contains("ghost_swarm"))
        .unwrap_or_else(|| panic!("完成度必须列出悬空外键：{blocking:?}"));
    assert_eq!(dangling.decision_id, "ld.wave_table");
    assert!(dangling.detail.contains("第 2 行"), "{}", dangling.detail);
    assert!(
        dangling.detail.contains("ld.enemy_roster"),
        "{}",
        dangling.detail
    );

    let gate2 = services
        .freeze_check(&archive_id)
        .unwrap()
        .gates
        .into_iter()
        .find(|gate| gate.gate == "gate2_consistency")
        .unwrap();
    assert!(!gate2.passed, "悬空外键必须拦下一致性门");
    assert!(
        gate2
            .findings
            .iter()
            .any(|finding| finding.code == "rule.wave_rows_reference_enemies"
                && finding.message.contains("ghost_swarm")),
        "{:?}",
        gate2.findings
    );

    // 改回真实敌人 id 后，一致性门放行（规则不误伤合法引用）。
    services
        .with_project(&archive_id, |engine| {
            engine.set_parameters(
                "ld.wave_table",
                ParameterValues::Rows {
                    rows: vec![
                        wave_row(1, "crawler", 5, 2.0),
                        wave_row(2, "crawler", 8, 1.6),
                        wave_row(3, "glider", 4, 1.5),
                        wave_row(4, "crawler", 10, 1.2),
                        wave_row(5, "glider", 8, 1.0),
                    ],
                },
            )?;
            Ok(())
        })
        .unwrap();
    let gate2 = services
        .freeze_check(&archive_id)
        .unwrap()
        .gates
        .into_iter()
        .find(|gate| gate.gate == "gate2_consistency")
        .unwrap();
    assert!(gate2.passed, "合法引用不应被拦：{:?}", gate2.findings);

    std::fs::remove_dir_all(&temp).ok();
}

/// 负例：Draft 模板预填被拒（D8 只有 Certified 可预填）；未检索先映射被拒（S1 前置）；
/// 重复建草稿被拒（防覆盖产线进度）。
#[test]
fn uncertified_template_prefill_is_rejected() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_t5neg_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);

    services
        .template_new_draft("lane_defense", "tpl_wip", "未名防线", &[], DesignLevel::L4)
        .unwrap();

    let error = services
        .project_new("负例项目", "lane_defense", DesignLevel::L6, Some("tpl_wip"))
        .unwrap_err();
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    assert!(error.message.contains("未完成认证"), "{}", error.message);

    let provider = ScriptedProvider::new();
    let error = services
        .template_map_with("lane_defense", "tpl_wip", &provider)
        .unwrap_err();
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    assert!(error.message.contains("语料检索"), "{}", error.message);

    assert!(
        services
            .template_new_draft("lane_defense", "tpl_wip", "未名防线", &[], DesignLevel::L4)
            .is_err()
    );

    std::fs::remove_dir_all(&temp).ok();
}
