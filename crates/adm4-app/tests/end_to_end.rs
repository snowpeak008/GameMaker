//! 端到端集成测试：设计空间校验 → 创建项目 → 手动创作 → 红队 → 冻结门五道 →
//! C0-C6 全链（确定性脚本 AI）→ 两个人工门确认 → 全绿。

use adm4_ai::{ScriptedImageProvider, ScriptedProvider};
use adm4_app::{
    AppConfig, AppServices, CONTRACT_FILE, DOCUMENT_FILE, InterviewTurnDto, StyleGenerationOptions,
    save_config,
};
use adm4_archive::DataRoot;
use adm4_authoring::TemplateMode;
use adm4_contracts::{MatrixCell, TypedValue};
use adm4_decision::{
    DesignLevel, NaJustification, ParameterValues, PointRequirement, Provenance, SelectionMode,
    UNASSIGNED_DOMAIN_ID,
};
use adm4_pipeline::{CancelSignal, StageStatus};
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

/// 二版十六领域检查单的领域入口点（W6 T10 迁移进通用层的内容）。
///
/// 迁移把二版 16 领域 / 103 节点 / 515 检查单项 × L4 选项组落成 2575 个通用层决策点，
/// 每个领域的入口点是 `requirement=baseline` 的根点（恒适用），域内其余点靠 unlocks
/// 顺序链激活。清单与 `scripts/cli_smoke.ps1` §5b 的 `$V2DomainEntryPoints` 逐字一致。
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

/// 逐个豁免 16 个领域入口点，与冒烟脚本 §5b 同一做法：全链场景只验证「品类最小链路」
/// 的工具链闭环，不做全域设计巡视。豁免点移出完成度分母，在冻结门第 1 道逐条在案且
/// 不拦截，且因为入口点无选择，域内下游链不激活——既有断言一条不改就能继续成立。
fn exempt_v2_domain_entry_points(services: &AppServices, archive_id: &str) {
    for entry in V2_DOMAIN_ENTRY_POINTS {
        services
            .authoring_set_not_applicable(
                archive_id,
                entry,
                "e2e_scope_minimal_chain",
                "本测试只验证品类最小链路",
                "e2e",
            )
            .unwrap();
    }
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
            image_provider: None,
            engine_backend: None,
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

    // 二版十六领域入口点显式豁免：本测试只验证品类最小链路（见常量处说明）。
    exempt_v2_domain_entry_points(&services, &archive_id);

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
    // 双真相修复核验：导入后 manifest 名与创作态名归一，工作台摘要报的是传入名（非导出方名）。
    assert_eq!(
        services
            .workbench_overview(&imported_id)
            .unwrap()
            .summary
            .project_name,
        "导入副本"
    );

    // 10. 文档集交付打包：C0-C6 全跑通 → 清单完整、无缺段、每段带非空 sha256。
    let manifest = services.deliverable_package(&archive_id, 1).unwrap();
    assert!(manifest.complete, "缺段：{:?}", manifest.missing_segments);
    assert!(manifest.missing_segments.is_empty());
    assert_eq!(manifest.segments.len(), 7);
    assert!(
        manifest
            .segments
            .iter()
            .all(|s| s.present && !s.document_sha256.is_empty())
    );
    // 落盘产物存在，且只读 status 与打包结果一致（段数一致）。
    assert!(content.join("deliverable/v1/manifest.json").is_file());
    let status = services.deliverable_status(&archive_id, 1).unwrap();
    assert_eq!(status.segments.len(), manifest.segments.len());
    assert!(status.complete);

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
            image_provider: None,
            engine_backend: None,
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
            let bad = engine.mark_not_applicable("u.business_model", NaJustification::default());
            assert!(bad.is_err());
            // baseline 跳过通道不接受署名（署名走 set_not_applicable，F3 合并后同一结构承载）。
            let signed = engine.mark_not_applicable(
                "u.business_model",
                NaJustification {
                    reason_code: "out_of_scope".into(),
                    note: "本期不做".into(),
                    actor: "主策划".into(),
                    at: String::new(),
                },
            );
            assert!(signed.is_err());
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
            image_provider: None,
            engine_backend: None,
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

    // 二版十六领域入口点显式豁免：本测试只验证品类最小链路（见常量处说明）。
    // 位置与冒烟脚本 §5b 一致——换皮改写之后、访谈开始之前，因此访谈只在品类点上推进。
    exempt_v2_domain_entry_points(&services, &archive_id);

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
            image_provider: None,
            engine_backend: None,
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

// ---------------------------------------------------------------------------
// T9 场景（W6 T10 迁移后跟随更新）：工作台聚合查询（领域进度 / 项目画像 / 右栏四页签）
// 在**已迁移**的仓内设计空间上工作——每个决策点都声明 node_id，按二版 16 个通用领域
// 分组，保留领域/节点「未分域」因无点落入而不出现在聚合结果里。
//
// 「未声明 node_id 全部落进未分域」的过渡形态由 adm4-decision::organization 与
// adm4-space::validate 的单元测试覆盖（保留项是代码内置的，不依赖仓库数据形态）。
// ---------------------------------------------------------------------------

#[test]
fn workbench_aggregates_work_on_migrated_space() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_t9_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let data_root = DataRoot::new(&temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: design_space_root().to_string_lossy().into_owned(),
            ai_provider: None,
            image_provider: None,
            engine_backend: None,
        },
    )
    .unwrap();
    let services = AppServices::open(Some(temp.clone())).unwrap();
    let archive_id = services
        .project_new("工作台聚合验证", "lane_defense", DesignLevel::L4, None)
        .unwrap();

    // L0/L1 各确认一条（画像卡片的数据源），L2 留空（缺失项的数据源）。
    services
        .with_project(&archive_id, |engine| {
            engine.select_option("u.platform", "pc_single", Provenance::UserManual)?;
            engine.confirm_selection("u.platform")?;
            engine.select_option("u.experience", "guardian_underdog", Provenance::UserManual)?;
            engine.set_parameters(
                "u.experience",
                ParameterValues::Scalars {
                    entries: [(
                        "statement".to_string(),
                        TypedValue::Text("以有限守卫资源保卫家园，从被动防御走向全面掌控".into()),
                    )]
                    .into_iter()
                    .collect(),
                },
            )?;
            engine.confirm_selection("u.experience")?;
            Ok(())
        })
        .unwrap();

    // 领域聚合：迁移后 16 个通用领域各自有决策点，保留领域为空（不出现在结果里）。
    let progress = services.organization_progress(&archive_id).unwrap();
    assert_eq!(
        progress.domains.len(),
        16,
        "迁移后应聚合出二版 16 个领域：{:?}",
        progress
            .domains
            .iter()
            .map(|domain| domain.domain_id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        progress.domain(UNASSIGNED_DOMAIN_ID).is_none(),
        "全部决策点都声明了 node_id，保留领域应为空"
    );
    // 两条已确认的点分属两个领域（u.platform → 立项定位，u.experience → 核心体验）。
    let positioning = progress
        .domain("product_positioning_design")
        .expect("u.platform 所在领域应上榜");
    assert_eq!(positioning.name, "立项与产品定位设计");
    assert_eq!(positioning.counts.confirmed, 1);
    assert!(positioning.counts.applicable >= 3);
    let core_experience = progress
        .domain("core_experience_design")
        .expect("u.experience 所在领域应上榜");
    assert_eq!(core_experience.counts.confirmed, 1);
    assert_eq!(progress.total.confirmed, 2);
    assert_eq!(progress.total.not_applicable, 0, "此时还没有任何人工豁免");

    // 节点文本挂在迁移后的真实节点上（u.experience 归「核心乐趣决策」节点）。
    services
        .authoring_set_node_risk_note(&archive_id, "core_fun_decision", "体验参数尚未经试玩验证")
        .unwrap();

    // 人工豁免一个适用点：分母 -1，且在案。
    services
        .authoring_set_not_applicable(
            &archive_id,
            "u.business_model",
            "out_of_scope",
            "本期只做单机买断，商业模式不再分化",
            "主策划",
        )
        .unwrap();

    // 项目画像：字段来自 L0/L1 已确认决策点，没有硬编码字段名。
    let profile = services.project_profile(&archive_id).unwrap();
    assert_eq!(profile.project_name, "工作台聚合验证");
    assert_eq!(profile.depth_target, DesignLevel::L4);
    let platform = profile
        .fields
        .iter()
        .find(|field| field.decision_id == "u.platform")
        .expect("已确认的 L0 点应上画像卡");
    assert_eq!(platform.level, DesignLevel::L0);
    assert_eq!(platform.label, "主平台是什么？");
    assert_eq!(platform.node_id, "platform_play_context_decision");
    assert_eq!(platform.domain_id, "product_positioning_design");
    assert_eq!(platform.selected.len(), 1);
    assert_eq!(platform.selected[0].label, "PC 单机");
    assert!(!platform.selected[0].is_primary, "单选点没有主选概念");
    assert!(
        profile
            .fields
            .iter()
            .any(|field| field.decision_id == "u.experience"),
        "L1 点也应上画像卡"
    );
    assert!(
        profile
            .fields
            .iter()
            .all(|field| field.decision_id != "u.business_model"),
        "被豁免的点不应上画像卡"
    );

    // 决策点视图：新字段（node_id / selection_mode / MDA / 设计提问）透传到 UI DTO，
    // 豁免记录带署名，逐选项已选状态可见。
    let views = services.decision_points(&archive_id).unwrap();
    let platform_view = views
        .iter()
        .find(|view| view.decision_id == "u.platform")
        .expect("决策点视图应覆盖全图");
    assert_eq!(platform_view.node_id, "platform_play_context_decision");
    assert_eq!(platform_view.domain_id, "product_positioning_design");
    assert_eq!(platform_view.applicability, "active");
    assert!(platform_view.confirmed);
    assert_eq!(platform_view.selection_mode, SelectionMode::Single);
    assert!(
        platform_view.design_question.is_none(),
        "u.platform 未声明设计提问（二版检查单点才有）"
    );
    assert!(
        platform_view
            .options
            .iter()
            .any(|option| option.option_id == "pc_single" && option.selected && !option.is_primary)
    );
    let exempted_view = views
        .iter()
        .find(|view| view.decision_id == "u.business_model")
        .expect("被豁免的点仍在视图里");
    assert_eq!(exempted_view.applicability, "not_applicable");
    let exemption = exempted_view.exemption.as_ref().expect("豁免记录应在案");
    assert_eq!(exemption.reason_code, "out_of_scope");
    assert_eq!(exemption.actor.as_deref(), Some("主策划"));
    assert!(exemption.at.is_some());

    // 右栏四页签一次取齐。
    let overview = services.workbench_overview(&archive_id).unwrap();

    // 1. 摘要：领域 × 进度 + 总完成度。
    assert_eq!(overview.summary.project_name, "工作台聚合验证");
    assert_eq!(overview.summary.genre_pack, "lane_defense");
    assert_eq!(overview.summary.done, 2);
    assert!(overview.summary.total > overview.summary.done);
    assert_eq!(overview.summary.percent, overview.summary.percent.min(100));
    assert_eq!(overview.summary.domains.len(), 16);
    assert_eq!(overview.summary.counts.not_applicable, 1);
    // 总完成度口径：领域聚合的分子分母与完成度报告同源同值（迁移后仍成立）。
    assert_eq!(overview.summary.counts.confirmed, overview.summary.done);
    assert_eq!(overview.summary.counts.applicable, overview.summary.total);
    assert!(!overview.summary.nodes.is_empty());

    // 2. 缺失项：未确认且适用的点，按领域分组（领域序同左栏，保留领域不出现）。
    assert_eq!(
        overview.missing.len(),
        16,
        "16 个领域各有未确认的适用点：{:?}",
        overview
            .missing
            .iter()
            .map(|group| group.domain_id.as_str())
            .collect::<Vec<_>>()
    );
    let missing_group = &overview.missing[0];
    assert_eq!(missing_group.domain_id, "product_positioning_design");
    assert!(
        missing_group
            .items
            .iter()
            .any(|item| item.decision_id == "u.genre" && item.reasons.contains(&"未选择".into())),
        "{:?}",
        missing_group.items
    );
    assert!(
        missing_group
            .items
            .iter()
            .all(|item| item.decision_id != "u.business_model"),
        "豁免点不算缺失项"
    );

    // 3. 风险：节点风险说明汇总；未跑红队时 red_team 为 None。
    assert_eq!(overview.risk.node_risks.len(), 1);
    assert_eq!(overview.risk.node_risks[0].node_id, "core_fun_decision");
    assert_eq!(overview.risk.node_risks[0].node_name, "核心乐趣决策");
    assert_eq!(
        overview.risk.node_risks[0].domain_id,
        "core_experience_design"
    );
    assert!(overview.risk.node_risks[0].note.contains("尚未经试玩验证"));
    assert!(overview.risk.red_team.is_none());

    // 4. 校验：外键违规 + 冻结门预检各门 pass/block。
    assert!(overview.validation.row_reference_violations.is_empty());
    assert_eq!(overview.validation.gates.len(), 4);
    let gate_names: Vec<&str> = overview
        .validation
        .gates
        .iter()
        .map(|gate| gate.gate.as_str())
        .collect();
    assert_eq!(
        gate_names,
        vec![
            "gate1_completeness",
            "gate2_consistency",
            "gate3_skin",
            "gate4_red_team"
        ]
    );
    assert!(!overview.validation.all_gates_passed, "设计未完成不该全绿");
    let gate1 = &overview.validation.gates[0];
    assert!(!gate1.passed);
    assert_eq!(gate1.finding_count, gate1.findings.len());
    // 豁免在门 1 明细里可见（带署名），且不参与通过判定。
    assert!(
        gate1
            .findings
            .iter()
            .any(|finding| finding.code == "not_applicable_exemption"
                && finding.message.contains("主策划")),
        "{:?}",
        gate1.findings
    );

    // 解除豁免 → 该点回到缺失项。
    assert!(
        services
            .authoring_clear_not_applicable(&archive_id, "u.business_model")
            .unwrap()
    );
    let after = services.workbench_overview(&archive_id).unwrap();
    assert_eq!(after.summary.counts.not_applicable, 0);
    assert_eq!(after.missing[0].domain_id, "product_positioning_design");
    assert!(
        after.missing[0]
            .items
            .iter()
            .any(|item| item.decision_id == "u.business_model"),
        "{:?}",
        after.missing[0].items
    );

    // F3-1：仓内通用层有 8 个 requirement=optional 的画像点（u.platform_scope 等）。
    // 它们恒适用（L0 根点）但未作答，因此不进分母、不进缺失项，只在 optional_skipped 计数。
    assert!(
        after.summary.optional_skipped >= 8,
        "迁移后的画像点应作为非必做点被移出分母，实际 {}",
        after.summary.optional_skipped
    );
    let optional_views: Vec<&adm4_app::DecisionPointView> = views
        .iter()
        .filter(|view| view.optional && view.applicability == "active")
        .collect();
    assert!(
        optional_views
            .iter()
            .any(|view| view.decision_id == "u.dimension"),
        "u.dimension 应是非必做点：{:?}",
        optional_views
            .iter()
            .map(|view| view.decision_id.as_str())
            .collect::<Vec<_>>()
    );
    for view in &optional_views {
        assert_eq!(view.requirement, PointRequirement::Optional);
        assert_eq!(view.requirement_label, "非必做");
        assert!(
            after
                .missing
                .iter()
                .flat_map(|group| group.items.iter())
                .all(|item| item.decision_id != view.decision_id),
            "未作答的非必做点不该进缺失项：{}",
            view.decision_id
        );
    }

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// F3 场景：通用层模板跨包预填（含跳过计数）+ 项目重命名 + 设计空间缓存等价性。
//
// 三项都走门面 `AppServices`（GUI/CLI 的唯一入口），用仓内真实设计空间的隔离副本，
// 从而同时验证「T10 迁入的 26 份 universal 模板确实可用」。
// ---------------------------------------------------------------------------

#[test]
fn universal_template_prefills_across_packs_via_services() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f3_prefill_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);

    // 取一份 T10 迁入的通用层内置模板（Certified，答卷含 u.* 画像点与 v2.* 检查单点）。
    let universal = services.templates().list("universal").unwrap();
    assert!(
        universal.len() >= 25,
        "T10 应迁入 25+ 份通用层模板，实际 {}",
        universal.len()
    );
    let template = universal
        .iter()
        .find(|template| template.template_id == "builtin_midcore_arknights")
        .expect("应能取到内置模板");
    assert!(template.is_universal());
    assert_eq!(
        template.certification.status,
        CertificationStatus::Certified
    );
    let template_id = template.template_id.clone();

    // 列表接口：lane_defense 的可取用集合必须包含通用层模板（此前被按包过滤掉）。
    let available = services.templates().list_available("lane_defense").unwrap();
    assert!(
        available
            .iter()
            .any(|item| item.template_id == template_id && item.is_universal()),
        "通用层模板必须出现在品类包的可取用列表里"
    );
    // 严格按目录的 list 里不该有它（逆向产线仍按 genre_pack 写回原目录）。
    assert!(
        services
            .templates()
            .list("lane_defense")
            .unwrap()
            .iter()
            .all(|item| item.template_id != template_id)
    );

    // 跨包预填：模板包是 universal，项目包是 lane_defense。
    let archive_id = services
        .project_new("通用模板预填验证", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    let report = services
        .project_prefill_template(&archive_id, &template_id)
        .unwrap();

    // 通用层答卷全是通用层决策点，而通用层对每个品类包都装配在内，因此整卷可用、零跳过。
    assert_eq!(report.applied, template.answers.len());
    assert_eq!(report.skipped_count(), 0, "{:?}", report.skipped);
    assert!(
        report.multi_options_applied > 0,
        "v2.gameplay_system_scope 是 multi+allow_primary 点，附加系统应随模板写入"
    );

    let state = services.load_authoring_state(&archive_id).unwrap();
    assert!(matches!(
        state.template_mode,
        TemplateMode::Prefilled { .. }
    ));
    // 画像点（F3-4 新补）确实被写入：证明补选项后 V2 画像答案不再全军覆没。
    let profile_written: Vec<&String> = state
        .selections
        .keys()
        .filter(|id| id.starts_with("u."))
        .collect();
    assert!(
        profile_written.len() >= 5,
        "补齐选项后画像答案应能落地，实际写入 {:?}",
        profile_written
    );
    // 多选点：附加选项与主选按模板落库。
    let scope = state
        .selections
        .get("v2.gameplay_system_scope")
        .expect("玩法系统范围点应被预填");
    assert!(scope.selected_count() > 1);
    assert!(scope.primary_option.is_some());
    assert!(scope.contains_option(scope.primary_option.as_deref().unwrap()));
    // 预填一律未确认（逐条确认 + 换皮门照旧）。
    assert!(
        state
            .selections
            .values()
            .all(|item| !item.confirmed_by_user)
    );

    // ---- 跳过计数：造一份含「本包装配空间外条目」的通用层模板，验证逐条跳过 + 落日志 ----
    // 真实迁移模板整卷可用（上面已断言零跳过），所以这条路径必须专门造数据来覆盖：
    // 静默丢弃是 R2 红线，不能只靠单元测试保。
    let mut patched = template.clone();
    patched.template_id = "tpl_f3_skip_probe".into();
    patched.game_name = "虚构探针甲".into();
    patched.answers.truncate(2);
    let evidence = patched.answers[0].evidence.clone();
    // F4d：探针改了答卷，批量迁移登记里的答卷指纹必须一起重算——否则取用关卡会
    // 判定「登记不为当前答卷背书」而拒绝预填（那正是它该做的事，见 T4d 负例）。
    let restamp = |template: &mut adm4_template::Template| {
        template.origin = adm4_template::TemplateOrigin::BulkMigration {
            batch_id: "e2e-probe".into(),
            tool_version: "e2e/1.0.0".into(),
            source_ref: "tests/end_to_end.rs".into(),
            answers_digest: template.answers_digest(),
            migrated_at: "2026-08-31T00:00:00Z".into(),
        };
    };
    let probe = |decision_id: &str, option_id: &str| adm4_template::TemplateAnswer {
        decision_id: decision_id.into(),
        option_id: option_id.into(),
        parameters: ParameterValues::None,
        evidence: evidence.clone(),
        notes: String::new(),
        crosscheck_agreed: None,
        additional_options: Vec::new(),
        primary_option: None,
    };
    // 1. 决策点不存在（其它品类包的专属点）；2. 选项不存在。
    patched.answers.push(probe("gs.grid_shape", "hex_grid"));
    patched.answers.push(probe("u.platform", "ghost_option"));
    restamp(&mut patched);
    services.templates().save_draft(&patched).unwrap();

    let probe_archive = services
        .project_new("跳过计数验证", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    let probe_report = services
        .project_prefill_template(&probe_archive, "tpl_f3_skip_probe")
        .unwrap();
    assert_eq!(probe_report.applied, 2);
    assert_eq!(
        probe_report.skipped_count(),
        2,
        "{:?}",
        probe_report.skipped
    );
    let skipped: Vec<(&str, &str)> = probe_report
        .skipped
        .iter()
        .map(|skip| (skip.decision_id.as_str(), skip.option_id.as_str()))
        .collect();
    assert!(
        skipped.contains(&("gs.grid_shape", "hex_grid")),
        "{skipped:?}"
    );
    assert!(
        skipped.contains(&("u.platform", "ghost_option")),
        "{skipped:?}"
    );
    assert!(
        probe_report
            .skipped
            .iter()
            .all(|skip| !skip.reason.trim().is_empty()),
        "每条跳过都必须给出原因"
    );
    assert!(probe_report.summary().contains("跳过 2 条"));
    // 被跳过的点不能留下任何选择（禁止半写入）。
    let probe_state = services.load_authoring_state(&probe_archive).unwrap();
    assert!(!probe_state.selections.contains_key("gs.grid_shape"));
    assert!(!probe_state.selections.contains_key("u.platform"));

    // 跳过明细进了运行日志（不只在返回值里）。
    let logs = services.log.tail(400).unwrap();
    assert!(
        logs.iter().any(|entry| entry.message.contains("跳过")
            && entry.message.contains("tpl_f3_skip_probe")
            && entry.message.contains("gs.grid_shape")),
        "跳过明细必须落运行日志"
    );

    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn project_rename_validates_and_logs() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f3_rename_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let data_root = DataRoot::new(&temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: design_space_root().to_string_lossy().into_owned(),
            ai_provider: None,
            image_provider: None,
            engine_backend: None,
        },
    )
    .unwrap();
    let services = AppServices::open(Some(temp.clone())).unwrap();
    let archive_id = services
        .project_new("旧名字", "lane_defense", DesignLevel::L4, None)
        .unwrap();

    // 空白名称被拒，且状态不变。
    assert!(services.project_rename(&archive_id, "   ").is_err());
    assert_eq!(
        services
            .load_authoring_state(&archive_id)
            .unwrap()
            .project_name,
        "旧名字"
    );

    services
        .project_rename(&archive_id, "  霜落峡谷防卫计划 ")
        .unwrap();
    let state = services.load_authoring_state(&archive_id).unwrap();
    assert_eq!(state.project_name, "霜落峡谷防卫计划");
    // 工作台摘要/画像读的都是创作状态，重命名即刻可见。
    assert_eq!(
        services.project_profile(&archive_id).unwrap().project_name,
        "霜落峡谷防卫计划"
    );
    assert_eq!(
        services
            .workbench_overview(&archive_id)
            .unwrap()
            .summary
            .project_name,
        "霜落峡谷防卫计划"
    );
    // 存档 manifest 的展示名同步跟随（project list 与工作台不会出现两个名字）。
    assert_eq!(
        services
            .project_list()
            .unwrap()
            .iter()
            .find(|manifest| manifest.archive_id == archive_id)
            .map(|manifest| manifest.project_name.as_str()),
        Some("霜落峡谷防卫计划")
    );
    assert!(
        services
            .log
            .tail(50)
            .unwrap()
            .iter()
            .any(|entry| entry.message.contains("重命名") && entry.message.contains("旧名字")),
        "重命名必须落运行日志"
    );

    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn design_space_cache_is_hit_and_behaviourally_equivalent() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f3_cache_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let data_root = DataRoot::new(&temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: design_space_root().to_string_lossy().into_owned(),
            ai_provider: None,
            image_provider: None,
            engine_backend: None,
        },
    )
    .unwrap();
    let services = AppServices::open(Some(temp.clone())).unwrap();

    // 命中判定不看耗时（会 flaky），看是不是同一份 Arc。
    let first = services.load_space_shared("lane_defense").unwrap();
    let second = services.load_space_shared("lane_defense").unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "同一包的第二次装载必须命中缓存"
    );
    let other = services.load_space_shared("grid_strategy").unwrap();
    assert!(!std::sync::Arc::ptr_eq(&first, &other), "不同包各自一份");

    // 行为等价：缓存返回的空间与直接装载的完全一致（决策点集合、组织维度、包元数据）。
    let owned = services.load_space("lane_defense").unwrap();
    assert_eq!(owned.pack, first.pack);
    assert_eq!(owned.universal_version, first.universal_version);
    assert_eq!(owned.graph.points().len(), first.graph.points().len());
    assert_eq!(
        owned
            .graph
            .points()
            .iter()
            .map(|point| point.id.as_str())
            .collect::<Vec<_>>(),
        first
            .graph
            .points()
            .iter()
            .map(|point| point.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        owned
            .organization
            .domains()
            .iter()
            .map(|domain| domain.id.as_str())
            .collect::<Vec<_>>(),
        first
            .organization
            .domains()
            .iter()
            .map(|domain| domain.id.as_str())
            .collect::<Vec<_>>()
    );

    // 不存在的包仍旧 fail-closed，且失败不进缓存（重复调用照旧报错）。
    assert!(services.load_space_shared("no_such_pack").is_err());
    assert!(services.load_space("no_such_pack").is_err());

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// F4a 新增场景：流水线控制三项（强制重跑 + 协作式取消 + 阶段产物查询）。
// ---------------------------------------------------------------------------

/// 造一个「品类最小链路已冻结（v1）」的 lane_defense 项目，返回存档 id。
///
/// 与 `full_chain_from_space_to_signed_phase1` 的创作块同源（同一组决策点与参数），
/// 但只到冻结为止——流水线怎么跑由各测试自己决定。
fn frozen_minimal_lane_defense_project(services: &AppServices, ai: &ScriptedProvider) -> String {
    frozen_lane_defense_project_named(services, ai, "重跑与取消验证项目")
}

/// [`frozen_minimal_lane_defense_project`] 的具名版：项目名参数化（其余逐字相同）。
/// F4d 需要一个**项目名可指定**的已冻结项目来验证换皮豁免（豁免词就是项目名）。
fn frozen_lane_defense_project_named(
    services: &AppServices,
    ai: &ScriptedProvider,
    project_name: &str,
) -> String {
    let archive_id = services
        .project_new(project_name, "lane_defense", DesignLevel::L6, None)
        .unwrap();
    exempt_v2_domain_entry_points(services, &archive_id);

    const STRUCTURAL: [(&str, &str); 15] = [
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
    ];

    services
        .with_project(&archive_id, |engine| {
            for (decision, option) in STRUCTURAL {
                engine
                    .select_option(decision, option, Provenance::UserManual)
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
            for (decision, _) in STRUCTURAL {
                engine.confirm_selection(decision).unwrap();
            }
            let report = engine.completeness();
            assert!(report.is_complete(), "blocking: {:?}", report.blocking);
            Ok(())
        })
        .unwrap();

    services.freeze_red_team_with(&archive_id, ai).unwrap();
    let frozen = services.freeze_run(&archive_id).unwrap();
    assert_eq!(frozen.version, 1);
    archive_id
}

fn assert_stage_statuses(state: &adm4_pipeline::PipelineRunState, expected: &[(&str, &str)]) {
    for (stage_id, wanted) in expected {
        let actual = match state.stage_status(stage_id) {
            StageStatus::Pending => "pending".to_string(),
            StageStatus::Running => "running".to_string(),
            StageStatus::Succeeded => "succeeded".to_string(),
            StageStatus::Failed { reasons } => format!("failed({})", reasons.join("; ")),
            StageStatus::Blocked { reasons } => format!("blocked({})", reasons.join("; ")),
            StageStatus::WaitingHuman { .. } => "waiting_human".to_string(),
        };
        assert_eq!(&actual, wanted, "阶段 {stage_id} 状态");
    }
}

#[test]
fn pipeline_cancellation_stops_at_stage_boundary_without_marking_failure() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4a_cancel_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let archive_id = frozen_minimal_lane_defense_project(&services, &ai);

    // 1. 运行前已取消：第一段就停，且记为「未运行」而不是 Failed。
    let cancel = CancelSignal::new();
    cancel.cancel();
    let outcome = services
        .pipeline_run_with_cancel(&archive_id, "C0", "C6", &ai, &cancel)
        .unwrap();
    assert_eq!(outcome.cancelled_at.as_deref(), Some("C0"));
    assert_stage_statuses(
        &outcome.state,
        &[("C0", "pending"), ("C1", "pending"), ("C6", "pending")],
    );
    // 一段都没跑 → 产物一份都不该在，且查询如实报缺失（不是空文档）。
    let c0 = services.pipeline_artifact(&archive_id, 1, "C0").unwrap();
    assert!(!c0.complete);
    assert_eq!(c0.missing, vec![DOCUMENT_FILE, CONTRACT_FILE]);
    assert_eq!(c0.document_text, None);

    // 2. 取消发生在半途：先正常跑到 C2，再带已取消的信号跑全程。
    services
        .pipeline_run_with(&archive_id, "C0", "C2", &ai)
        .unwrap();
    let mid_cancel = CancelSignal::new();
    mid_cancel.cancel();
    let outcome = services
        .pipeline_run_with_cancel(&archive_id, "C0", "C6", &ai, &mid_cancel)
        .unwrap();
    assert_eq!(
        outcome.cancelled_at.as_deref(),
        Some("C3"),
        "取消应在下一个未完成段的边界生效"
    );
    assert_stage_statuses(
        &outcome.state,
        &[
            ("C0", "succeeded"),
            ("C1", "succeeded"),
            ("C2", "succeeded"),
            ("C3", "pending"),
            ("C4", "pending"),
        ],
    );
    // 已完成段的产物必须原样保留（协作式取消不回滚已完成的工作）。
    assert!(
        services
            .pipeline_artifact(&archive_id, 1, "C2")
            .unwrap()
            .complete
    );
    assert!(
        !services
            .pipeline_artifact(&archive_id, 1, "C3")
            .unwrap()
            .complete
    );

    // 3. 复位同一个信号即可继续断点续跑，一路推进到 C5 人工门。
    mid_cancel.reset();
    let outcome = services
        .pipeline_run_with_cancel(&archive_id, "C0", "C6", &ai, &mid_cancel)
        .unwrap();
    assert_eq!(outcome.cancelled_at, None);
    assert_stage_statuses(
        &outcome.state,
        &[
            ("C3", "succeeded"),
            ("C4", "succeeded"),
            ("C5", "waiting_human"),
            ("C6", "pending"),
        ],
    );

    // 4. 取消在审计流里留痕（"为什么停在 C3" 必须可追查）。
    let messages: Vec<String> = services
        .log
        .tail(2000)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.category == "pipeline")
        .map(|entry| entry.message)
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("被用户取消") && message.contains("阶段 C3")),
        "取消必须落日志：{messages:?}"
    );

    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn pipeline_rerun_invalidates_downstream_stages_artifacts_and_human_gates() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4a_rerun_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let archive_id = frozen_minimal_lane_defense_project(&services, &ai);

    // 先把 C0-C6 跑成全绿（两道人工门都署名通过）。
    services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    services
        .pipeline_confirm(&archive_id, "C5", "初审评审员", "风格方向确认")
        .unwrap();
    services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    let state = services
        .pipeline_confirm(&archive_id, "C6", "初审评审员", "Phase 1 文档集签收")
        .unwrap();
    for stage_id in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        assert!(state.is_succeeded(stage_id), "{stage_id} 应先全绿");
        assert!(
            services
                .pipeline_artifact(&archive_id, 1, stage_id)
                .unwrap()
                .complete
        );
    }
    let c4_before = services.pipeline_artifact(&archive_id, 1, "C4").unwrap();

    // --- 强制重跑 C2：C2 及其全部下游（C3-C6）的状态、产物、人工门署名一并失效 ---
    let rerun = services
        .pipeline_rerun_with(&archive_id, "C2", "C6", &ai)
        .unwrap();
    assert_eq!(rerun.reset.target, "C2");
    assert_eq!(rerun.reset.reset_stages, ["C2", "C3", "C4", "C5", "C6"]);
    assert_eq!(
        rerun.reset.cleared_artifacts,
        ["C2", "C3", "C4", "C5", "C6"]
    );
    let revoked: Vec<(&str, &str)> = rerun
        .reset
        .revoked_confirmations
        .iter()
        .map(|item| (item.stage_id.as_str(), item.actor.as_str()))
        .collect();
    assert_eq!(
        revoked,
        vec![("C5", "初审评审员"), ("C6", "初审评审员")],
        "R3：重置范围内的旧署名必须作废，不许为新产物背书"
    );
    assert_eq!(rerun.cancelled_at, None);

    // 重跑后：C2/C3/C4 重新产出，C5 回到等待人工确认，C6 停在未运行。
    assert_stage_statuses(
        &rerun.state,
        &[
            ("C0", "succeeded"),
            ("C1", "succeeded"),
            ("C2", "succeeded"),
            ("C3", "succeeded"),
            ("C4", "succeeded"),
            ("C5", "waiting_human"),
            ("C6", "pending"),
        ],
    );

    // C6 产物确实失效（缺段如实报，不是空文档兜底）。
    let c6_after = services.pipeline_artifact(&archive_id, 1, "C6").unwrap();
    assert!(!c6_after.complete);
    assert_eq!(c6_after.missing, vec![DOCUMENT_FILE, CONTRACT_FILE]);
    assert_eq!(c6_after.document_text, None);
    assert!(c6_after.document.sha256.is_empty());
    assert_eq!(c6_after.document.bytes, 0);
    // C4 是重跑范围内的确定性段：产物被删后重新生成，内容哈希与上一版一致（确定性投影）。
    let c4_after = services.pipeline_artifact(&archive_id, 1, "C4").unwrap();
    assert!(c4_after.complete);
    assert_eq!(c4_after.contract.sha256, c4_before.contract.sha256);

    // 人工门确认确实失效：C6 未运行不能确认，C5 必须重新署名。
    let stale = services
        .pipeline_confirm(&archive_id, "C6", "初审评审员", "沿用上次签收")
        .unwrap_err();
    assert_eq!(stale.kind, adm4_foundation::Adm4ErrorKind::NotFound);
    services
        .pipeline_confirm(&archive_id, "C5", "复审评审员", "重跑后重新确认风格")
        .unwrap();
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    assert_stage_statuses(&state, &[("C6", "waiting_human")]);
    let state = services
        .pipeline_confirm(&archive_id, "C6", "复审评审员", "重跑后重新签收")
        .unwrap();
    for stage_id in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        assert!(state.is_succeeded(stage_id), "{stage_id} 重跑后应重新全绿");
    }
    for gate in ["C5", "C6"] {
        assert_eq!(
            state.stages[gate]
                .human_confirmation
                .as_ref()
                .map(|item| item.actor.as_str()),
            Some("复审评审员"),
            "{gate} 应由重跑后的新署名背书"
        );
    }

    // 重跑与作废逐条落日志。
    let messages: Vec<String> = services
        .log
        .tail(2000)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.category == "pipeline")
        .map(|entry| entry.message)
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("强制重跑 C2..C6") && message.contains("重置 5 段")),
        "重跑必须落日志：{messages:?}"
    );
    assert!(
        messages
            .iter()
            .filter(|message| message.contains("随重跑作废"))
            .count()
            >= 2,
        "两处作废的人工门确认应各留一条日志：{messages:?}"
    );

    // --- 负例：参数不合法时一份产物都不许被动 ---
    assert!(
        services
            .pipeline_rerun_with(&archive_id, "C4", "C2", &ai)
            .is_err(),
        "区间非法（from > to）必须被拒"
    );
    let unknown = services
        .pipeline_rerun_with(&archive_id, "C9", "C6", &ai)
        .unwrap_err();
    assert_eq!(unknown.kind, adm4_foundation::Adm4ErrorKind::NotFound);
    for stage_id in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
        assert!(
            services
                .pipeline_artifact(&archive_id, 1, stage_id)
                .unwrap()
                .complete,
            "{stage_id} 的产物不该被失败的重跑请求误伤"
        );
    }

    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn stage_artifact_query_exposes_documents_and_reports_gaps() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4a_artifact_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let archive_id = frozen_minimal_lane_defense_project(&services, &ai);
    services
        .pipeline_run_with(&archive_id, "C0", "C2", &ai)
        .unwrap();

    // 已产出段：双格式产物齐备 + 路径/摘要/字节数/预览文本全部可用。
    let c2 = services.pipeline_artifact(&archive_id, 1, "C2").unwrap();
    assert_eq!(c2.archive_id, archive_id);
    assert_eq!((c2.frozen_version, c2.stage_id.as_str()), (1, "C2"));
    assert!(c2.complete && c2.missing.is_empty());
    assert!(c2.document.present && c2.contract.present);
    let document_path = std::path::Path::new(&c2.document.path);
    assert!(
        document_path.is_file(),
        "路径必须真能打开：{}",
        c2.document.path
    );
    let on_disk = std::fs::read(document_path).unwrap();
    assert_eq!(c2.document.bytes as usize, on_disk.len());
    let text = c2.document_text.clone().expect("预览文本");
    assert_eq!(text.as_bytes(), on_disk.as_slice(), "小文档应给全文");
    assert!(text.contains("玩法设计文档"), "C2 渲染文档正文：{text}");
    assert!(!c2.document_truncated);
    assert!(c2.preview_limit_bytes > 0);

    // 未产出段：如实报缺失（present=false + missing 列名 + 文本 None）。
    let c5 = services.pipeline_artifact(&archive_id, 1, "C5").unwrap();
    assert!(!c5.complete);
    assert_eq!(c5.missing, vec![DOCUMENT_FILE, CONTRACT_FILE]);
    assert_eq!(c5.document_text, None);
    assert!(c5.document.path.contains("C5"), "{}", c5.document.path);

    // 未知阶段 id → 报错，不能伪装成「该段没跑」。
    for bad in ["C7", "P0", "c2"] {
        assert_eq!(
            services
                .pipeline_artifact(&archive_id, 1, bad)
                .unwrap_err()
                .kind,
            adm4_foundation::Adm4ErrorKind::NotFound,
            "阶段 id {bad}"
        );
    }
    // 不存在的冻结版本：整版目录缺失 → 七段全缺，不报错也不兜底。
    let ghost = services.pipeline_artifact(&archive_id, 9, "C0").unwrap();
    assert!(!ghost.complete);
    assert_eq!(ghost.missing, vec![DOCUMENT_FILE, CONTRACT_FILE]);
    // 不存在的存档 → not_found。
    assert_eq!(
        services
            .pipeline_artifact("arc-not-there", 1, "C0")
            .unwrap_err()
            .kind,
        adm4_foundation::Adm4ErrorKind::NotFound
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// F4b 新增场景：另存模板 / 工作台重置 / 画像取点 / 体检上提 / 阶段耗时与运行中状态。
// ---------------------------------------------------------------------------

/// 二版「风格定位·感受目标」：L3、baseline（恒适用）、multi + allow_primary。
/// 既是另存模板里验证多选/主选导出的样本，也是画像卡「美术风格」字段的归宿。
const ART_STYLE_POINT: &str =
    "v2.art_direction_decision.feng_ge_ding_wei.presentation_feeling_target";

/// 造一个「答了几个点、其中一个故意不确认」的 lane_defense 项目（不冻结，够快）。
///
/// 已确认：`u.business_model`（单选）/ `u.platform`（单选）/ `ART_STYLE_POINT`（多选 + 主选）。
/// 未确认：`u.genre` —— 另存模板必须把它挡在门外。
fn project_with_one_unconfirmed_point(services: &AppServices) -> String {
    let archive_id = services
        .project_new("霜落峡谷防卫计划", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    services
        .with_project(&archive_id, |engine| {
            engine.select_option("u.business_model", "premium", Provenance::UserManual)?;
            engine.set_rationale("u.business_model", "单机塔防以一次性交付内容为宜")?;
            engine.confirm_selection("u.business_model")?;
            engine.select_option("u.platform", "pc_single", Provenance::UserManual)?;
            engine.confirm_selection("u.platform")?;
            engine.select_option(ART_STYLE_POINT, "clear_readable", Provenance::UserManual)?;
            engine.add_option(ART_STYLE_POINT, "immersive_mood")?;
            engine.set_primary_option(ART_STYLE_POINT, "immersive_mood")?;
            engine.confirm_selection(ART_STYLE_POINT)?;
            // 选了但没确认：预填/提案的半成品，不该被当成定论传播出去。
            engine.select_option(
                "u.genre",
                "lane_defense",
                Provenance::Template {
                    template_id: "someone_else".into(),
                },
            )?;
            Ok(())
        })
        .unwrap();
    archive_id
}

/// 另存模板（H1）：只导出已确认的点 → 落 HumanReviewed → certify → 可预填且仍需逐条确认；
/// 逆向来源缺 S2/S3 证据在门面层同样被拒；落盘前过换皮扫描（R5）。
#[test]
fn template_save_as_exports_only_confirmed_selections_then_certifies_and_prefills() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4b_saveas_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let archive_id = project_with_one_unconfirmed_point(&services);

    // --- R5 落盘钩子：项目里残留参考游戏名 → 另存被拒（不许随模板扩散出去） ---
    services
        .with_project(&archive_id, |engine| {
            engine.set_rationale("u.platform", "照 Kingdom Rush 的布防节奏来")
        })
        .unwrap();
    let blocked = services
        .template_export_from_project(
            &archive_id,
            "tpl_from_project",
            "",
            &[],
            "评审员甲",
            "逐条复核已确认选择",
        )
        .unwrap_err();
    assert_eq!(blocked.kind, adm4_foundation::Adm4ErrorKind::RedLine);
    assert!(
        blocked.message.contains("kingdom rush"),
        "{}",
        blocked.message
    );
    assert!(
        services
            .templates()
            .get("lane_defense", "tpl_from_project")
            .is_err(),
        "被换皮门拦下时一份模板都不许落盘"
    );

    // --- 正例：改写理由后另存 ---
    services
        .with_project(&archive_id, |engine| {
            engine.set_rationale("u.platform", "键鼠精确布防是本作的核心操作前提")
        })
        .unwrap();
    let report = services
        .template_export_from_project(
            &archive_id,
            "tpl_from_project",
            "",
            &["霜落定稿".to_string()],
            " 评审员甲 ",
            " 逐条复核已确认选择 ",
        )
        .unwrap();

    // 钉子 ①：未确认的点不进模板，且跳过数如实在案（R2）。
    assert_eq!(report.exported_points, 3, "{}", report.summary());
    assert_eq!(
        report.skipped_unconfirmed, 1,
        "u.genre 未确认，必须被挡在门外"
    );
    assert!(report.skipped_unknown.is_empty());
    assert_eq!(report.exported_additional_options, 1);
    assert_eq!(report.exported_primary_marks, 1);
    assert_eq!(report.status, "HumanReviewed");
    assert_eq!(report.reviewed_by, "评审员甲");
    assert_eq!(report.source_project_name, "霜落峡谷防卫计划");
    assert_eq!(report.depth_reached, DesignLevel::L4);

    let saved = services
        .templates()
        .get("lane_defense", "tpl_from_project")
        .unwrap();
    assert!(saved.is_project_export());
    assert_eq!(
        saved.origin,
        adm4_template::TemplateOrigin::ProjectExport {
            source_archive_id: archive_id.clone(),
            source_project_name: "霜落峡谷防卫计划".into(),
            exported_at: match &saved.origin {
                adm4_template::TemplateOrigin::ProjectExport { exported_at, .. } =>
                    exported_at.clone(),
                other => panic!("来源应为本项目导出，实际 {other:?}"),
            },
        }
    );
    assert_eq!(saved.game_name, "霜落峡谷防卫计划", "缺 --game 时用项目名");
    let ids: Vec<&str> = saved
        .answers
        .iter()
        .map(|answer| answer.decision_id.as_str())
        .collect();
    assert!(
        !ids.contains(&"u.genre"),
        "未确认的点不许出现在答卷里：{ids:?}"
    );
    let art = saved
        .answers
        .iter()
        .find(|answer| answer.decision_id == ART_STYLE_POINT)
        .expect("多选点应导出");
    assert_eq!(art.primary_option.as_deref(), Some("immersive_mood"));
    assert_eq!(art.selected_count(), 2);
    assert_eq!(
        art.selected_option_ids(),
        vec!["immersive_mood", "clear_readable"],
        "主选排在最前"
    );
    let business = saved
        .answers
        .iter()
        .find(|answer| answer.decision_id == "u.business_model")
        .expect("单选点应导出");
    assert_eq!(
        business.notes, "单机塔防以一次性交付内容为宜",
        "选择理由落答卷备注"
    );
    assert!(
        business.evidence.is_empty(),
        "本项目导出没有外部来源可引，宁缺勿造（不许塞假 URL）"
    );

    // 同名模板不许覆盖（另存不该悄悄毁掉别人的产线进度）。
    assert_eq!(
        services
            .template_export_from_project(
                &archive_id,
                "tpl_from_project",
                "",
                &[],
                "评审员甲",
                "再来一次"
            )
            .unwrap_err()
            .kind,
        adm4_foundation::Adm4ErrorKind::Conflict
    );

    // 认证前不可预填（取用关卡对本项目导出来源一视同仁）。
    let target = services
        .project_new("接收另存模板的项目", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    assert_eq!(
        services
            .project_prefill_template(&target, "tpl_from_project")
            .unwrap_err()
            .kind,
        adm4_foundation::Adm4ErrorKind::Blocked
    );

    // --- S5 认证：本项目导出来源不要求 S2/S3 机器证据，但**照常登记换皮词表** ---
    //
    // F4d 修红线：曾经「本项目导出不登记词表」，理由是源项目自己的名字进了词表后
    // 它自己过不了换皮门。代价是 B 项目预填 A 的模板、带着 A 的名字通过冻结门——
    // 换皮扫描对「抄另一个项目」彻底失效。现在登记照做，源项目自身的放行改由
    // 扫描侧按当前项目名豁免（`skin_scanner_for_project`）。
    let wordlist_before = load_skin_wordlist(&services.skin_wordlist_path())
        .unwrap()
        .words;
    let certified = services
        .template_certify("lane_defense", "tpl_from_project")
        .unwrap();
    assert_eq!(
        certified.certification.status,
        CertificationStatus::Certified
    );
    let wordlist_after = load_skin_wordlist(&services.skin_wordlist_path())
        .unwrap()
        .words;
    for word in ["霜落峡谷防卫计划", "霜落定稿"] {
        assert!(
            !wordlist_before.contains(&word.to_string())
                && wordlist_after.contains(&word.to_string()),
            "另存模板认证后 {word} 必须进词表（否则别的项目抄它没人拦）：{wordlist_after:?}"
        );
    }

    // 钉子（R5 豁免作用域）①：源项目自己的名字已在词表里，它自己照旧过得了换皮门。
    // 用「再另存一份」验证：另存前整份模板过换皮扫描，而模板的 game_name 与
    // origin.source_project_name 就是项目名——没有豁免的话这一步必被 RedLine 拦下。
    services
        .template_export_from_project(
            &archive_id,
            "tpl_from_project_again",
            "",
            &[],
            "评审员甲",
            "自身名字已在词表里，本项目仍应可导出",
        )
        .expect("源项目自己的名字不该拦住源项目自己（R5 豁免只放行当前项目名）");

    // 钉子 ③：认证后可预填，且预填条目一条都不算已确认。
    let prefill = services
        .project_prefill_template(&target, "tpl_from_project")
        .unwrap();
    assert_eq!(prefill.applied, 3, "{}", prefill.summary());
    assert_eq!(prefill.multi_options_applied, 1);
    assert!(prefill.skipped.is_empty(), "{:?}", prefill.skipped);
    let views = services.decision_points(&target).unwrap();
    for decision_id in ["u.business_model", "u.platform", ART_STYLE_POINT] {
        let view = views
            .iter()
            .find(|view| view.decision_id == decision_id)
            .unwrap_or_else(|| panic!("{decision_id} 应已预填"));
        assert!(
            !view.confirmed,
            "{decision_id} 预填后必须仍待用户逐条确认（AI/模板永不代提交）"
        );
    }
    assert!(
        services.project_profile(&target).unwrap().fields.is_empty(),
        "未确认的预填条目不上画像卡"
    );
    services
        .with_project(&target, |engine| engine.confirm_selection("u.platform"))
        .unwrap();
    assert!(
        services
            .project_profile(&target)
            .unwrap()
            .fields
            .iter()
            .any(|field| field.decision_id == "u.platform"),
        "逐条确认后才计入"
    );

    // 钉子（R5 豁免作用域）②：换到别的项目，A 的名字照旧被拦。
    // `target` 刚用 tpl_from_project 预填，预填理由是「模板预填自 霜落峡谷防卫计划」。
    let target_gates = services.freeze_check(&target).unwrap();
    let skin_hits: Vec<String> = target_gates
        .gates
        .iter()
        .flat_map(|gate| gate.findings.iter())
        .filter(|finding| finding.code == "reference_name_hit")
        .map(|finding| finding.message.clone())
        .collect();
    assert!(
        skin_hits
            .iter()
            .any(|message| message.contains("霜落峡谷防卫计划")),
        "B 项目的产物带着 A 的项目名必须被换皮门拦下：{skin_hits:?}"
    );
    // 钉子（R5 豁免作用域）③：同一份词表下，逆向来源的外部游戏名拦截行为一条不变。
    services
        .with_project(&target, |engine| {
            engine.set_rationale("u.platform", "参考 Kingdom Rush 的布防节奏")
        })
        .unwrap();
    assert!(
        services
            .freeze_check(&target)
            .unwrap()
            .gates
            .iter()
            .flat_map(|gate| gate.findings.iter())
            .any(|finding| finding.code == "reference_name_hit"
                && finding.message.contains("kingdom rush")),
        "逆向来源的外部游戏名照旧被拦"
    );

    // --- 钉子 ②：逆向来源缺 S2/S3 证据，门面层的 certify 同样拒绝 ---
    services
        .template_new_draft(
            "lane_defense",
            "tpl_forged",
            "虚构逆向甲",
            &[],
            DesignLevel::L4,
        )
        .unwrap();
    let mut forged = services
        .templates()
        .get("lane_defense", "tpl_forged")
        .unwrap();
    forged.certification = adm4_template::Certification {
        status: CertificationStatus::HumanReviewed,
        reviewed_by: "评审员乙".into(),
        reviewed_at: "2026-08-31T00:00:00Z".into(),
        review_note: "手改状态字段伪造的审核".into(),
    };
    services.templates().save_draft(&forged).unwrap();
    let refused = services
        .template_certify("lane_defense", "tpl_forged")
        .unwrap_err();
    assert_eq!(refused.kind, adm4_foundation::Adm4ErrorKind::RedLine);
    assert!(refused.message.contains("逆向模板"), "{}", refused.message);
    assert_eq!(
        services
            .templates()
            .get("lane_defense", "tpl_forged")
            .unwrap()
            .certification
            .status,
        CertificationStatus::HumanReviewed,
        "被拒时磁盘上的模板一字不改"
    );
    assert_eq!(
        services
            .project_prefill_template(&target, "tpl_forged")
            .unwrap_err()
            .kind,
        adm4_foundation::Adm4ErrorKind::Blocked
    );

    std::fs::remove_dir_all(&temp).ok();
}

/// 工作台重置（H2）：清空创作态，保留项目、已冻结版本与流水线产物；actor/note 双必填。
#[test]
fn workbench_reset_clears_authoring_state_but_keeps_frozen_history() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4b_reset_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let archive_id = frozen_minimal_lane_defense_project(&services, &ai);
    services
        .pipeline_run_with(&archive_id, "C0", "C2", &ai)
        .unwrap();
    services
        .authoring_set_node_design_note(&archive_id, "core_fun_decision", "核心乐趣以布防抉择为轴")
        .unwrap();
    services
        .authoring_set_node_risk_note(&archive_id, "core_fun_decision", "体验参数尚未经试玩验证")
        .unwrap();

    let before = services.load_authoring_state(&archive_id).unwrap();
    assert_eq!(before.frozen_versions, 1);
    assert!(before.selections.len() >= 15);
    assert!(!before.not_applicable.is_empty());

    // 钉子 ⑤：破坏性操作缺署名或缺理由一律被拒（R3），且什么都没被清掉。
    for (actor, note) in [("   ", "返工重来"), ("主策划", "  ")] {
        let error = services
            .project_reset_workbench(&archive_id, actor, note)
            .unwrap_err();
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::InvalidInput);
    }
    assert_eq!(
        services
            .load_authoring_state(&archive_id)
            .unwrap()
            .selections
            .len(),
        before.selections.len(),
        "被拒的重置不许动任何数据"
    );

    let report = services
        .project_reset_workbench(&archive_id, " 主策划 ", " 品类方向推翻，创作重来 ")
        .unwrap();
    assert_eq!(report.actor, "主策划");
    assert_eq!(report.cleared_selections, before.selections.len());
    assert_eq!(report.cleared_exemptions, before.not_applicable.len());
    assert_eq!(report.cleared_node_design_notes, 1);
    assert_eq!(report.cleared_node_risk_notes, 1);
    assert!(report.cleared_parameter_values >= 7, "{}", report.summary());
    assert!(!report.is_noop());

    // 钉子 ④a：创作态回到初始未作答状态。
    let after = services.load_authoring_state(&archive_id).unwrap();
    assert!(after.selections.is_empty());
    assert!(after.not_applicable.is_empty());
    assert!(after.node_design_notes.is_empty());
    assert!(after.node_risk_notes.is_empty());
    assert_eq!(after.template_mode, TemplateMode::None);
    assert!(after.revision > before.revision, "重置也是一次变更");
    let completeness = services.open_engine(&archive_id).unwrap().completeness();
    assert_eq!(completeness.done, 0);
    assert!(completeness.total > 0, "分母仍在，只是一个都没答");
    assert!(
        services
            .project_profile(&archive_id)
            .unwrap()
            .fields
            .is_empty()
    );

    // 钉子 ④b：项目身份、已冻结版本与流水线产物全部保留。
    assert_eq!(after.project_name, before.project_name);
    assert_eq!(after.frozen_versions, 1, "冻结版本是只增不改的历史（D4）");
    assert_eq!(
        services.load_frozen(&archive_id, 1).unwrap().version,
        1,
        "冻结产物必须还能读出来"
    );
    assert_eq!(services.latest_frozen_version(&archive_id).unwrap(), 1);
    let state = services.pipeline_status(&archive_id).unwrap();
    for stage_id in ["C0", "C1", "C2"] {
        assert!(state.is_succeeded(stage_id), "{stage_id} 的运行状态应保留");
        assert!(
            services
                .pipeline_artifact(&archive_id, 1, stage_id)
                .unwrap()
                .complete,
            "{stage_id} 的产物应保留"
        );
    }
    assert!(
        services
            .project_list()
            .unwrap()
            .iter()
            .any(|manifest| manifest.archive_id == archive_id),
        "项目本身不该消失"
    );
    // 体检仍一致（重置走的是 with_project 事务，指纹与内容同步刷新）。
    let doctor = services.project_doctor(&archive_id).unwrap();
    assert!(doctor.healthy, "{:?}", doctor.problems);

    // 重置在审计流里留痕（清了什么、谁按的、为什么按）。
    let messages: Vec<String> = services
        .log
        .tail(2000)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.category == "project")
        .map(|entry| entry.message)
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("工作台重置") && message.contains("主策划")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("品类方向推翻，创作重来")),
        "重置理由必须落日志：{messages:?}"
    );

    // 重置一个已经空白的项目：幂等且如实报告「无可清空」。
    let again = services
        .project_reset_workbench(&archive_id, "主策划", "确认已清空")
        .unwrap();
    assert!(again.is_noop());
    assert_eq!(again.cleared_selections, 0);

    std::fs::remove_dir_all(&temp).ok();
}

/// 画像取点（M4）：清单驱动取点让 L2/L3 点上画像卡，但**不动**完备度分母；
/// 写错 id 被装载校验拦下；去掉清单即回退 L0/L1（旧数据零影响）。
#[test]
fn profile_points_drive_profile_card_without_touching_completeness_denominator() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4b_profile_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let pack_path = temp
        .join("design_space")
        .join("lane_defense")
        .join("pack.json");

    // 仓内数据的清单覆盖二版画像六字段，且每个 id 都真实存在。
    let declared = services
        .load_space("lane_defense")
        .unwrap()
        .pack
        .profile_points
        .clone();
    for wanted in [
        "u.genre",                                              // 品类
        "u.platform",                                           // 平台
        "v2.target_player_decision.wan_jia_hua_xiang.age_band", // 目标用户
        "u.business_model",                                     // 商业模式
        ART_STYLE_POINT,                                        // 美术风格
        "u.target_scale",                                       // 内容规模
        "u.operation_model",                                    // 上线节奏
    ] {
        assert!(
            declared.iter().any(|id| id == wanted),
            "画像清单应覆盖 {wanted}"
        );
    }

    let archive_id = services
        .project_new("画像取点验证", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    services
        .with_project(&archive_id, |engine| {
            engine.select_option("u.platform", "pc_single", Provenance::UserManual)?;
            engine.confirm_selection("u.platform")?;
            // u.genre 是 L2、ART_STYLE_POINT 是 L3：按老的 L0/L1 过滤这两个永远上不了画像卡。
            engine.select_option("u.genre", "lane_defense", Provenance::UserManual)?;
            engine.confirm_selection("u.genre")?;
            engine.select_option(ART_STYLE_POINT, "immersive_mood", Provenance::UserManual)?;
            engine.confirm_selection(ART_STYLE_POINT)?;
            Ok(())
        })
        .unwrap();

    let profile = services.project_profile(&archive_id).unwrap();
    let field_ids: Vec<&str> = profile
        .fields
        .iter()
        .map(|field| field.decision_id.as_str())
        .collect();
    assert_eq!(
        field_ids,
        vec!["u.genre", "u.platform", ART_STYLE_POINT],
        "取点与展示顺序都由清单决定"
    );
    let art = &profile.fields[2];
    assert_eq!(art.level, DesignLevel::L3);
    assert_eq!(art.selected.len(), 1);
    assert!(art.label.contains("风格定位"), "{}", art.label);
    let with_list = services.open_engine(&archive_id).unwrap().completeness();
    let views_with_list = services.decision_points(&archive_id).unwrap();

    // --- 钉子 ⑥：清单里写错 id → 装载即 fail-closed（不静默忽略） ---
    let original: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pack_path).unwrap()).unwrap();
    let rewrite = |pack: &serde_json::Value| {
        std::fs::write(&pack_path, serde_json::to_string_pretty(pack).unwrap()).unwrap();
    };
    let mut typo = original.clone();
    typo["profile_points"] = serde_json::json!(["u.platform", "u.target_scale_typo"]);
    rewrite(&typo);
    let reopened = AppServices::open(Some(temp.clone())).unwrap();
    let error = reopened.load_space("lane_defense").unwrap_err();
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    assert!(
        error.message.contains("profile.unknown_point"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains("u.target_scale_typo"),
        "{}",
        error.message
    );

    // --- 钉子 ⑦：去掉清单后完备度分母/分子/非必做计数一字不变，只有画像卡变少 ---
    let mut stripped = original.clone();
    stripped
        .as_object_mut()
        .expect("pack.json 是对象")
        .remove("profile_points")
        .expect("清单键应存在");
    rewrite(&stripped);
    let without_list = AppServices::open(Some(temp.clone())).unwrap();
    assert!(
        without_list
            .load_space("lane_defense")
            .unwrap()
            .pack
            .profile_points
            .is_empty(),
        "清单应已移除（模拟旧数据）"
    );
    let fallback = without_list
        .open_engine(&archive_id)
        .unwrap()
        .completeness();
    assert_eq!(
        (fallback.total, fallback.done, fallback.optional_skipped),
        (with_list.total, with_list.done, with_list.optional_skipped),
        "画像取点是纯展示层，不许动完备度分母"
    );
    assert_eq!(fallback.blocking.len(), with_list.blocking.len());
    let views_without_list = without_list.decision_points(&archive_id).unwrap();
    assert_eq!(views_with_list.len(), views_without_list.len());
    for (with, without) in views_with_list.iter().zip(views_without_list.iter()) {
        assert_eq!(
            (
                with.decision_id.as_str(),
                with.level,
                with.requirement,
                with.applicability.as_str()
            ),
            (
                without.decision_id.as_str(),
                without.level,
                without.requirement,
                without.applicability.as_str()
            ),
            "清单不得改变任何决策点的层级/必填性/适用性"
        );
    }
    let fallback_profile = without_list.project_profile(&archive_id).unwrap();
    assert_eq!(
        fallback_profile
            .fields
            .iter()
            .map(|field| field.decision_id.as_str())
            .collect::<Vec<_>>(),
        vec!["u.platform"],
        "回退到 L0/L1 过滤：L2 的品类与 L3 的美术风格都上不了卡（正是本项要修的老毛病）"
    );

    std::fs::remove_dir_all(&temp).ok();
}

/// 体检上提（D）：`project doctor` / `ai doctor` 的判定进门面，两端共用同一份结论。
#[test]
fn doctor_reports_are_structured_at_the_service_layer() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4b_doctor_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let archive_id = services
        .project_new("体检验证", "lane_defense", DesignLevel::L4, None)
        .unwrap();

    let healthy = services.project_doctor(&archive_id).unwrap();
    assert_eq!(healthy.archive_id, archive_id);
    assert!(healthy.healthy);
    assert!(healthy.problems.is_empty());

    // 内容被外部改动 → 指纹不一致，逐条报问题（体检报告问题，不是自己失败）。
    let content = services.archives.content_dir(&archive_id);
    std::fs::write(content.join("tampered.txt"), b"outside edit").unwrap();
    let broken = services.project_doctor(&archive_id).unwrap();
    assert!(!broken.healthy);
    assert_eq!(broken.problems.len(), 1);
    assert!(
        broken.problems[0].contains("内容指纹不一致"),
        "{:?}",
        broken.problems
    );

    // 存档不存在 → manifest 不可读也是一条问题（而不是抛错）。
    let missing = services.project_doctor("archive-not-there").unwrap();
    assert!(!missing.healthy);
    assert!(
        missing.problems[0].contains("manifest 不可读"),
        "{:?}",
        missing.problems
    );

    // AI 体检：本测试的配置里 ai_provider=None → 不可用，并如实带出原始原因。
    let ai = services.ai_doctor();
    assert!(!ai.available);
    assert!(ai.provider_id.is_empty());
    assert!(ai.detail.contains("未配置 AI Provider"), "{}", ai.detail);

    std::fs::remove_dir_all(&temp).ok();
}

/// 阶段耗时与运行中状态（E）：段执行期间落 `Running` + `started_at`，完成后可算耗时；
/// 取消不留 `Running` 残留；旧存档（无 `started_at`）照旧可读。
#[test]
fn pipeline_records_running_state_and_stage_durations() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4b_running_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let archive_id = frozen_minimal_lane_defense_project(&services, &ai);
    let run_state_path = services
        .archives
        .content_dir(&archive_id)
        .join("pipeline")
        .join("v1")
        .join("run_state.json");

    // 在 C1 的 AI 调用时刻窥一眼磁盘上的运行状态：本段必须已经是「运行中」。
    // 这是桌面端把流水线放进工作线程后显示「C1 正在跑」的唯一依据。
    let spy = RunStateSpy {
        inner: &ai,
        run_state_path: run_state_path.clone(),
        observed: std::sync::Mutex::new(Vec::new()),
    };
    let state = services
        .pipeline_run_with(&archive_id, "C0", "C2", &spy)
        .unwrap();
    let observed = spy.observed.into_inner().expect("观测锁");
    assert!(
        observed
            .iter()
            .any(|(stage_id, status)| stage_id == "C1" && status == "running"),
        "C1 执行期间磁盘上的状态应为 running，实测 {observed:?}"
    );
    assert!(
        observed
            .iter()
            .any(|(stage_id, status)| stage_id == "C0" && status == "succeeded"),
        "上一段照旧是 succeeded：{observed:?}"
    );

    // 跑完之后：三段都是成功，开始/结束时刻俱在，耗时算得出来。
    for stage_id in ["C0", "C1", "C2"] {
        let record = state
            .stages
            .get(stage_id)
            .unwrap_or_else(|| panic!("{stage_id} 记录应在案"));
        assert_eq!(record.status, StageStatus::Succeeded);
        assert!(!record.started_at.is_empty(), "{stage_id} 缺开始时刻");
        assert!(!record.finished_at.is_empty(), "{stage_id} 缺结束时刻");
        assert!(
            record.duration_seconds().is_some(),
            "{stage_id} 的耗时必须算得出来"
        );
    }

    // 取消：被取消的段记为未运行、无开始时刻，全表不留任何 Running 残留。
    let cancel = CancelSignal::new();
    cancel.cancel();
    let outcome = services
        .pipeline_run_with_cancel(&archive_id, "C0", "C6", &ai, &cancel)
        .unwrap();
    assert_eq!(outcome.cancelled_at.as_deref(), Some("C3"));
    let cancelled = &outcome.state.stages["C3"];
    assert_eq!(cancelled.status, StageStatus::Pending);
    assert!(
        cancelled.started_at.is_empty(),
        "一步都没执行的段不该有开始时刻"
    );
    assert_eq!(cancelled.duration_seconds(), None);
    assert!(
        outcome
            .state
            .stages
            .values()
            .all(|record| record.status != StageStatus::Running),
        "取消后不许留下「正在跑」的幽灵段：{:?}",
        outcome.state.stages
    );

    // 人工门：产物就绪后等人签字，确认时刻即结束时刻，等待时长也算进耗时。
    services
        .pipeline_run_with(&archive_id, "C0", "C6", &ai)
        .unwrap();
    let state = services
        .pipeline_confirm(&archive_id, "C5", "评审员甲", "风格方向确认")
        .unwrap();
    let gate = &state.stages["C5"];
    assert!(!gate.started_at.is_empty());
    assert!(gate.duration_seconds().is_some());

    // 钉子 ⑧：旧存档（run_state.json 里没有 started_at 键）照旧可读，耗时如实为未知。
    let legacy = r#"{
      "frozen_hash": "PLACEHOLDER",
      "stages": {
        "C0": {"stage_id":"C0","status":{"status":"succeeded"},"contract_hash":"","finished_at":"2026-08-31T10:00:30Z","human_confirmation":null}
      }
    }"#
    .replace(
        "PLACEHOLDER",
        &services.load_frozen(&archive_id, 1).unwrap().content_hash,
    );
    std::fs::write(&run_state_path, legacy).unwrap();
    let reloaded = services.pipeline_status(&archive_id).unwrap();
    let record = &reloaded.stages["C0"];
    assert!(record.started_at.is_empty());
    assert_eq!(record.duration_seconds(), None, "缺开始时刻就说不知道");
    assert!(reloaded.is_succeeded("C0"));

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// F4d 场景 1：换皮词表按项目排除（R5 跨项目漏洞）——A 的产物放行、B 的产物被拦
//
// 与 `template_save_as_...` 里的钉子互补：那条走「另存 + 冻结门」，这条走
// **流水线产物落盘钩子**（C0 文档标题就是项目名，最容易被自己的名字拦住的地方）。
// ---------------------------------------------------------------------------

#[test]
fn skin_wordlist_exempts_only_the_current_project_name() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4d_skin_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);

    // A 项目：手动答满 → 冻结 → C0 产物落盘成功（此时词表里还没有 A 的名字）。
    let ai = scripted_ai();
    let archive_a = frozen_lane_defense_project_named(&services, &ai, "霜落峡谷防卫计划");
    let state = services
        .pipeline_run_with(&archive_a, "C0", "C0", &ai)
        .unwrap();
    assert!(state.is_succeeded("C0"));

    // A 另存模板并认证 → A 的项目名进全局词表（R5 照常登记）。
    services
        .template_export_from_project(
            &archive_a,
            "tpl_frostfall",
            "",
            &[],
            "评审员甲",
            "逐条复核已确认选择",
        )
        .unwrap();
    services
        .template_certify("lane_defense", "tpl_frostfall")
        .unwrap();
    let words = load_skin_wordlist(&services.skin_wordlist_path())
        .unwrap()
        .words;
    assert!(
        words.contains(&"霜落峡谷防卫计划".to_string()),
        "另存模板认证必须登记源项目名（否则别的项目抄它没人拦）：{words:?}"
    );

    // ① A 自己重跑 C0：文档标题就是「霜落峡谷防卫计划」，落盘钩子必须放行。
    let rerun = services
        .pipeline_rerun_with(&archive_a, "C0", "C0", &ai)
        .unwrap();
    assert!(
        rerun.state.is_succeeded("C0"),
        "自身名字在词表里不该拦住自己：{:?}",
        rerun.state.stage_status("C0")
    );
    let document = services
        .pipeline_artifact(&archive_a, 1, "C0")
        .unwrap()
        .document_text
        .expect("C0 文档应可读");
    assert!(
        document.contains("霜落峡谷防卫计划"),
        "C0 文档标题确实带项目名（否则本用例证明不了什么）"
    );

    // ② B 项目预填 A 的模板 → 理由里带 A 的名字 → 冻结门必须拦。
    let archive_b = services
        .project_new("晨星台地防线", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    services
        .project_prefill_template(&archive_b, "tpl_frostfall")
        .unwrap();
    let hits: Vec<String> = services
        .freeze_check(&archive_b)
        .unwrap()
        .gates
        .iter()
        .flat_map(|gate| gate.findings.iter())
        .filter(|finding| finding.code == "reference_name_hit")
        .map(|finding| finding.message.clone())
        .collect();
    assert!(
        hits.iter()
            .any(|message| message.contains("霜落峡谷防卫计划")),
        "B 的产物带着 A 的项目名必须被拦：{hits:?}"
    );

    // ③ 逆向来源的外部游戏名：两个项目视角下都照旧拦，一条没放松。
    for archive in [&archive_a, &archive_b] {
        services
            .with_project(archive, |engine| {
                engine.set_rationale("u.platform", "参考 Kingdom Rush 的布防节奏")
            })
            .unwrap();
        assert!(
            services
                .freeze_check(archive)
                .unwrap()
                .gates
                .iter()
                .flat_map(|gate| gate.findings.iter())
                .any(|finding| finding.code == "reference_name_hit"
                    && finding.message.contains("kingdom rush")),
            "逆向来源的外部游戏名拦截行为必须一条不变"
        );
    }

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// F4e：换皮豁免的作用域收窄到「由本存档 ProjectExport 模板登记的词」
//
// F4d 的豁免是按**词面**剔除当前项目名，不问该词条是谁登记的，于是留下一条缝：
// 项目取名恰好等于词表里某个逆向外部游戏名（如项目就叫「Kingdom Rush」）时，
// 那个外部名对这个项目整体失效——该项目可以随便抄它而不被 R5 拦住。
//
// 四条钉子一起写，因为它们是同一个判定的四个方向，分开写会掩盖作用域被放宽：
// ① 本存档导出登记的自身名放行；② 换到别的存档视角照旧拦；
// ③ 逆向外部游戏名一条不放松；④ 项目名与外部游戏名同名时，外部名**依然生效**。
// ---------------------------------------------------------------------------

#[test]
fn skin_exemption_only_covers_words_registered_by_this_archives_own_export() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4e_exemption_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let space = services.load_space("lane_defense").unwrap();

    // A 项目（名为「霜落峡谷防卫计划」）另存模板并认证 → 它的名字由 A 的 ProjectExport 登记。
    let archive_a = project_with_one_unconfirmed_point(&services);
    let archive_b = services
        .project_new("晨星台地防线", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    services
        .template_export_from_project(
            &archive_a,
            "tpl_frostfall",
            "",
            &[],
            "评审员甲",
            "逐条复核已确认选择",
        )
        .unwrap();
    services
        .template_certify("lane_defense", "tpl_frostfall")
        .unwrap();
    let words = load_skin_wordlist(&services.skin_wordlist_path())
        .unwrap()
        .words;
    assert!(
        words.contains(&"霜落峡谷防卫计划".to_string()),
        "另存模板认证必须登记源项目名：{words:?}"
    );

    // ① 唯一登记来源是本存档的另存模板 → 豁免成立，A 自己的产物含 A 名不被拦。
    let scanner_a = services
        .skin_scanner_for_project(&space, &archive_a, "霜落峡谷防卫计划")
        .unwrap();
    assert_eq!(scanner_a.exempted(), ["霜落峡谷防卫计划".to_string()]);
    assert!(
        scanner_a
            .scan("c0/document.md", "霜落峡谷防卫计划设计规格")
            .is_empty(),
        "C0 文档标题就是项目名，拦了它这个项目永远走不完流水线"
    );

    // ② 同一个词换到 B 的视角：B 没导出过它 → 一个词都不豁免，照旧命中。
    let scanner_b = services
        .skin_scanner_for_project(&space, &archive_b, "晨星台地防线")
        .unwrap();
    assert!(scanner_b.exempted().is_empty());
    assert_eq!(
        scanner_b
            .scan("c0/document.md", "模板预填自霜落峡谷防卫计划")
            .len(),
        1,
        "B 的产物带着 A 的项目名必须被拦"
    );
    // 反向也钉住：拿 A 的名字当 B 的项目名去要豁免同样拿不到（豁免看登记的存档，不看词面）。
    assert!(
        services
            .skin_scanner_for_project(&space, &archive_b, "霜落峡谷防卫计划")
            .unwrap()
            .exempted()
            .is_empty(),
        "词条登记在 A 名下，B 冒用同一个词面不得获得豁免"
    );

    // ③ 逆向外部游戏名（品类包 reference_games）：两种视角下拦截行为一条不变。
    for scanner in [&scanner_a, &scanner_b] {
        assert_eq!(
            scanner
                .scan("c2/document.md", "参考 Kingdom Rush 的布防节奏")
                .len(),
            1
        );
    }

    // ④a 本次要修的缝：项目取名恰好等于品类包里的外部游戏名，该外部名**依然生效**。
    let archive_kingdom = services
        .project_new("Kingdom Rush", "lane_defense", DesignLevel::L4, None)
        .unwrap();
    let scanner_kingdom = services
        .skin_scanner_for_project(&space, &archive_kingdom, "Kingdom Rush")
        .unwrap();
    assert!(
        scanner_kingdom.exempted().is_empty(),
        "项目取名 Kingdom Rush 不得让这个外部游戏名对本项目整体失效"
    );
    assert!(
        scanner_kingdom
            .wordlist()
            .contains(&"kingdom rush".to_string()),
        "外部名必须仍在生效词表里：{:?}",
        scanner_kingdom.wordlist()
    );
    // 行为层同款：该项目的产物带这个名字照旧被冻结门拦下。
    services
        .with_project(&archive_kingdom, |engine| {
            engine.select_option("u.platform", "pc_single", Provenance::UserManual)?;
            engine.set_rationale("u.platform", "沿用 Kingdom Rush 的布防节奏")
        })
        .unwrap();
    assert!(
        services
            .freeze_check(&archive_kingdom)
            .unwrap()
            .gates
            .iter()
            .flat_map(|gate| gate.findings.iter())
            .any(|finding| finding.code == "reference_name_hit"
                && finding.message.contains("kingdom rush")),
        "同名项目的换皮门必须照常拦住那个外部游戏名"
    );

    // ④b 同一条缝的另一半：词条**同时**有逆向来源登记时，豁免立即撤销。
    // 造一份 game_name 与 A 逐字相同的逆向模板（外部游戏恰好与本项目同名），
    // 落盘即可——判定的问题是「这个词面有没有可能指某个外部游戏」，不看认证状态。
    let rival = adm4_template::Template {
        template_id: "tpl_rival_same_name".into(),
        game_name: "霜落峡谷防卫计划".into(),
        aliases: Vec::new(),
        genre_pack: "lane_defense".into(),
        pack_version: space.pack.pack_version.clone(),
        depth_reached: DesignLevel::L4,
        answers: Vec::new(),
        certification: adm4_template::Certification::default(),
        origin: adm4_template::TemplateOrigin::Reverse,
        mapping_hash: String::new(),
        crosscheck_proof: None,
    };
    services.templates().save_draft(&rival).unwrap();
    let scanner_a_after = services
        .skin_scanner_for_project(&space, &archive_a, "霜落峡谷防卫计划")
        .unwrap();
    assert!(
        scanner_a_after.exempted().is_empty(),
        "词条一旦另有逆向来源登记，豁免必须撤销（宁可拦住 A 自己，也不放过抄同名外部游戏）"
    );
    assert_eq!(
        scanner_a_after
            .scan("c0/document.md", "霜落峡谷防卫计划设计规格")
            .len(),
        1
    );
    // 该模板对别的存档一样是外部名（不因为它与某个项目同名而对谁松一档）。
    assert!(
        services
            .skin_scanner_for_project(&space, &archive_b, "霜落峡谷防卫计划")
            .unwrap()
            .exempted()
            .is_empty()
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// F4d 场景 2：认证证据旁路收口（R1/R3）——伪认证被拒、25 份内置模板照旧可预填
// ---------------------------------------------------------------------------

#[test]
fn prefill_requires_verifiable_evidence_while_builtin_templates_still_work() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4d_evidence_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let archive_id = services
        .project_new("证据关卡验证", "lane_defense", DesignLevel::L4, None)
        .unwrap();

    // ① 25 份批量迁移的内置模板：登记齐备且答卷指纹对得上 → 照旧可预填。
    let builtins = services.templates().list("universal").unwrap();
    assert!(
        builtins.len() >= 25,
        "内置模板应有 25+ 份，实际 {}",
        builtins.len()
    );
    for template in &builtins {
        assert!(
            template.is_bulk_migration(),
            "内置模板 {} 应带批量迁移登记（迁移工具 --stamp-origin 补的）",
            template.template_id
        );
        template
            .require_certification_evidence()
            .unwrap_or_else(|error| {
                panic!(
                    "内置模板 {} 的迁移登记应可核对：{}",
                    template.template_id, error.message
                )
            });
    }
    let report = services
        .project_prefill_template(&archive_id, "builtin_midcore_arknights")
        .unwrap();
    assert!(report.applied > 0);

    // ② 手工往 references/ 里塞一份 status=certified、无任何证据的 JSON → 预填被拒。
    let forged = r#"{
      "template_id": "tpl_forged_certified",
      "game_name": "伪造甲",
      "genre_pack": "lane_defense",
      "pack_version": "0.1.0",
      "depth_reached": "L4",
      "certification": {"status": "certified", "reviewed_by": "我自己", "reviewed_at": "2026-08-31T00:00:00Z", "review_note": "手改的"},
      "answers": [{"decision_id": "u.platform", "option_id": "pc_single", "evidence": []}]
    }"#;
    let references = Path::new(services.design_space_root())
        .join("lane_defense")
        .join("references");
    std::fs::create_dir_all(&references).unwrap();
    std::fs::write(references.join("tpl_forged_certified.json"), forged).unwrap();
    assert!(
        services
            .templates()
            .get("lane_defense", "tpl_forged_certified")
            .unwrap()
            .is_certified(),
        "状态位确实是 Certified（这正是旁路的形态）"
    );
    let refused = services
        .project_prefill_template(&archive_id, "tpl_forged_certified")
        .unwrap_err();
    assert_eq!(refused.kind, adm4_foundation::Adm4ErrorKind::RedLine);
    assert!(refused.message.contains("机器证据"), "{}", refused.message);
    // 对照也走同一关卡（不能只堵预填，留个只读侧门把答卷抄出来）。
    assert_eq!(
        services
            .template_compare(&archive_id, "tpl_forged_certified")
            .unwrap_err()
            .kind,
        adm4_foundation::Adm4ErrorKind::RedLine
    );

    // ③ 篡改内置模板的答卷而不更新登记 → 指纹失配 → 取用被拒。
    let mut tampered = services
        .templates()
        .get("universal", "builtin_midcore_arknights")
        .unwrap();
    tampered.template_id = "builtin_tampered".into();
    tampered.answers.truncate(3);
    services.templates().save_draft(&tampered).unwrap();
    let refused = services
        .project_prefill_template(&archive_id, "builtin_tampered")
        .unwrap_err();
    assert_eq!(refused.kind, adm4_foundation::Adm4ErrorKind::RedLine);
    assert!(refused.message.contains("指纹"), "{}", refused.message);

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// F4d 场景 3：配置热更新 + 密钥写入 + AI 实调用检查
//
// 全程零网络：实调用检查走 `ai_invoke_check_with(ScriptedProvider)`；
// 「未配置 Provider」路径本就不发请求。
// ---------------------------------------------------------------------------

#[test]
fn config_hot_reload_secret_write_and_invoke_check() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_f4d_config_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);

    // --- 热更新：open 时没有 ai_provider，运行期设上即刻生效（不必重开门面） ---
    assert!(services.config().unwrap().ai_provider.is_none());
    assert!(!services.ai_doctor().available);
    assert_eq!(
        services
            .build_provider()
            .err()
            .expect("未配置时必须显式失败")
            .kind,
        adm4_foundation::Adm4ErrorKind::AiUnavailable
    );

    services
        .set_ai_provider(Some(adm4_ai::HttpProviderConfig {
            provider_id: "smoke_local".into(),
            base_url: "http://127.0.0.1:9/v1".into(),
            model: "test-model".into(),
            api_key_ref: "named:smoke_key".into(),
            timeout_secs: 5,
        }))
        .unwrap();
    // 密钥还没写 → 体检如实报不可用（配置在、密钥解析不出来）。
    let doctor = services.ai_doctor();
    assert!(!doctor.available, "{}", doctor.detail);
    assert!(doctor.detail.contains("smoke_key"), "{}", doctor.detail);

    // --- 密钥写入：值不进回执、不进运行日志 ---
    const SECRET: &str = "sk-f4d-DO-NOT-LOG-0123456789";
    assert!(
        services.ai_save_secret("  ", SECRET).is_err(),
        "空密钥名必须被拒"
    );
    assert!(
        services.ai_save_secret("smoke_key", "").is_err(),
        "空密钥值必须被拒（配置看着可用而调用必然 401）"
    );
    let receipt = services.ai_save_secret("smoke_key", SECRET).unwrap();
    assert!(!receipt.contains(SECRET), "回执不得包含密钥值：{receipt}");
    assert!(receipt.contains("smoke_key"));
    let log_text =
        std::fs::read_to_string(temp.join("logs").join("run_log.jsonl")).unwrap_or_default();
    assert!(!log_text.contains(SECRET), "运行日志不得包含密钥值");
    assert!(log_text.contains("smoke_key"), "日志应记下密钥名（可审计）");
    // 落点是数据根 config/，不是项目存档内容树 → 不进存档、不进导出包、不进内容指纹。
    let secrets_path = temp.join("config").join("secrets.json");
    assert!(secrets_path.is_file());
    assert!(
        std::fs::read_to_string(&secrets_path)
            .unwrap()
            .contains(SECRET),
        "密钥本身当然要落在 secrets.json 里（它就是密钥库）"
    );
    assert_eq!(services.ai_secret_names().unwrap(), vec!["smoke_key"]);

    // 密钥齐备 → 体检转为可用（仍然零网络）。
    let doctor = services.ai_doctor();
    assert!(doctor.available, "{}", doctor.detail);
    assert_eq!(doctor.provider_id, "smoke_local");

    // --- 实调用检查：成功 / 失败 / 空应答三条路径（一律 ScriptedProvider，零网络） ---
    let ok = ScriptedProvider::new();
    ok.script(adm4_app::AI_INVOKE_CHECK_PURPOSE, vec!["OK".into()]);
    let report = services.ai_invoke_check_with(&ok);
    assert!(report.succeeded, "{}", report.summary());
    assert_eq!(report.response_chars, 2);
    assert_eq!(report.provider_id, "scripted");
    assert!(report.summary().contains("实调用成功"));

    // 没有脚本应答 = provider 报错 → 如实失败，原始原因原样保留（R7：不美化、不重试）。
    let failing = ScriptedProvider::new();
    let report = services.ai_invoke_check_with(&failing);
    assert!(!report.succeeded);
    assert!(
        report.detail.contains("ai_invoke_check"),
        "失败原因必须是后端原文：{}",
        report.detail
    );

    // 空应答也算失败：调用链通但产出不可用，报「可用」等于误报。
    let empty = ScriptedProvider::new();
    empty.script(adm4_app::AI_INVOKE_CHECK_PURPOSE, vec!["   ".into()]);
    let report = services.ai_invoke_check_with(&empty);
    assert!(!report.succeeded);
    assert!(report.detail.contains("空文本"), "{}", report.detail);

    // --- reload_config：磁盘被别处改动后重载即生效；设计空间根被改则显式报错 ---
    let mut on_disk = adm4_app::load_config(&services.data_root).unwrap();
    on_disk
        .ai_provider
        .as_mut()
        .expect("provider 应已落盘")
        .model = "hand-edited-model".into();
    save_config(&services.data_root, &on_disk).unwrap();
    assert_eq!(
        services.config().unwrap().ai_provider.unwrap().model,
        "test-model",
        "重载之前，运行期生效配置不该被磁盘悄悄改掉"
    );
    assert_eq!(
        services.reload_config().unwrap().ai_provider.unwrap().model,
        "hand-edited-model"
    );

    let mut moved = adm4_app::load_config(&services.data_root).unwrap();
    moved.design_space_root = temp.join("another_space").to_string_lossy().into_owned();
    save_config(&services.data_root, &moved).unwrap();
    let error = services
        .reload_config()
        .expect_err("设计空间根是进程期不变量，改了必须显式报错而不是装作生效");
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    assert!(error.message.contains("重启"), "{}", error.message);

    // 清空 Provider 也走同一通道：落盘 + 运行期同时生效。
    services.set_ai_provider(None).unwrap();
    assert!(services.config().unwrap().ai_provider.is_none());
    assert!(
        adm4_app::load_config(&services.data_root)
            .unwrap()
            .ai_provider
            .is_none()
    );
    let report = services.ai_invoke_check();
    assert!(!report.succeeded);
    assert!(
        report.provider_id.is_empty(),
        "没构建出 Provider 就没发请求"
    );
    assert!(report.summary().contains("未能构建 Provider"));

    std::fs::remove_dir_all(&temp).ok();
}

/// 转发型 Provider：每次被调用时窥一眼磁盘上的 `run_state.json`，记录各段状态。
///
/// 只读文件、不改任何东西；AI 应答完全转发给内层的 [`ScriptedProvider`]（零网络、确定性）。
/// 「段执行期间状态是 Running」这件事只有在段执行**中间**才观察得到，
/// 而 AI 调用恰好就在段中间——所以探针挂在这里。
struct RunStateSpy<'a> {
    inner: &'a ScriptedProvider,
    run_state_path: PathBuf,
    /// `AiProvider` 要求 `Sync`，所以用 `Mutex` 而不是 `RefCell` 存观测结果。
    observed: std::sync::Mutex<Vec<(String, String)>>,
}

impl adm4_ai::AiProvider for RunStateSpy<'_> {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn capabilities(&self) -> &[adm4_ai::AiCapability] {
        self.inner.capabilities()
    }

    fn invoke(
        &self,
        request: &adm4_ai::AiRequest,
    ) -> adm4_foundation::Adm4Result<adm4_ai::AiResponse> {
        if let Ok(text) = std::fs::read_to_string(&self.run_state_path)
            && let Ok(state) = serde_json::from_str::<adm4_pipeline::PipelineRunState>(&text)
        {
            let mut observed = self.observed.lock().expect("观测锁");
            for (stage_id, record) in &state.stages {
                let label = match &record.status {
                    StageStatus::Pending => "pending",
                    StageStatus::Running => "running",
                    StageStatus::Succeeded => "succeeded",
                    StageStatus::Failed { .. } => "failed",
                    StageStatus::Blocked { .. } => "blocked",
                    StageStatus::WaitingHuman { .. } => "waiting_human",
                };
                observed.push((stage_id.clone(), label.to_string()));
            }
        }
        self.inner.invoke(request)
    }
}

// ---------------------------------------------------------------------------
// G1 场景：Phase 2 构建产线门面（build_*）全链
//
// 本波 P0-P5 全是诚实空执行器，因此这条链验的是**骨架**而不是产线：
// 真源前置、版图自洽、区间/续跑/重跑/人工门语义、与 Phase 1 运行状态互不干扰，
// 以及最要紧的一条——每段如实 Blocked 并说清在等谁，绝不出现假成功（R7）。
// ---------------------------------------------------------------------------

#[test]
fn build_facade_runs_the_honest_empty_plan_without_faking_success() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_g1_build_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let archive_id = frozen_minimal_lane_defense_project(&services, &ai);

    // --- 版图：注册表 + 制品声明自洽，且每段都说得清自己在等谁 ---
    let plan = services.build_plan().expect("Phase 2 版图必须自洽");
    let ids: Vec<&str> = plan.iter().map(|stage| stage.stage_id.as_str()).collect();
    assert_eq!(ids, vec!["P0", "P1", "P2", "P3", "P4", "P5"]);
    assert!(plan[0].depends_on.is_empty(), "P0 是版图起点");
    assert_eq!(plan[3].depends_on, vec!["P1", "P2"], "P3 合流两条线");
    assert!(
        plan[0].produces.iter().any(|item| item == "对齐报告"),
        "对齐报告由 P0 产出：{:?}",
        plan[0].produces
    );
    assert!(
        plan[2].consumes.iter().any(|item| item == "风格锚点集"),
        "资产生产消费设计阶段锁定的风格锚点：{:?}",
        plan[2].consumes
    );
    // G4a 起 P0/P1/P2 已实现（无待实现说明），P3/P4/P5 仍有诚实登记。
    for stage in &plan {
        match stage.stage_id.as_str() {
            "P0" | "P1" | "P2" => assert!(
                stage.pending_note.is_none(),
                "{} 已实现，不该再挂待实现说明",
                stage.stage_id
            ),
            _ => {
                let note = stage
                    .pending_note
                    .as_deref()
                    .expect("未实现段必须有诚实说明");
                assert!(note.starts_with("待 G"), "{note}");
            }
        }
    }

    // --- 真源前置：C0 没跑过就不许开跑（不就地重编一份规格 = 不造第二真源）---
    let error = services
        .build_run(&archive_id, "P0", "P5")
        .expect_err("缺 C0 产物必须显式失败");
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    assert!(error.message.contains("C0"), "{}", error.message);

    // --- 跑 Phase 1 的 C0，Phase 2 才有真源可派生 ---
    let phase1 = services
        .pipeline_run_with(&archive_id, "C0", "C0", &ai)
        .unwrap();
    assert!(phase1.is_succeeded("C0"));

    // --- G3 起 P0 真跑：只有 C0 时缺 C3/C4，P0 如实 Blocked 指路（不就地重算 C3）---
    let state = services.build_run(&archive_id, "P0", "P5").unwrap();
    match state.stage_status("P0") {
        StageStatus::Blocked { reasons } => {
            assert_eq!(reasons.len(), 1);
            assert!(reasons[0].contains("C3"), "{}", reasons[0]);
            assert!(reasons[0].contains("pipeline run"), "{}", reasons[0]);
        }
        other => panic!("P0 缺上游契约应如实 Blocked，实际 {other:?}"),
    }
    assert_stage_statuses(
        &state,
        &[
            ("P1", "pending"),
            ("P2", "pending"),
            ("P3", "pending"),
            ("P4", "pending"),
            ("P5", "pending"),
        ],
    );
    assert!(
        !state.frozen_hash.is_empty(),
        "构建运行状态必须绑定冻结版本"
    );

    // --- status 只读回放同一份结论 ---
    let status = services.build_status(&archive_id).unwrap();
    assert_eq!(status, state);

    // --- 未知阶段与非法区间：显式报错，不伪装成「那段没跑」---
    assert_eq!(
        services
            .build_run(&archive_id, "P0", "P9")
            .expect_err("未知阶段必须被拒")
            .kind,
        adm4_foundation::Adm4ErrorKind::NotFound
    );
    assert_eq!(
        services
            .build_run(&archive_id, "C0", "P5")
            .expect_err("C 段不在构建版图内")
            .kind,
        adm4_foundation::Adm4ErrorKind::NotFound
    );
    assert_eq!(
        services
            .build_run(&archive_id, "P3", "P1")
            .expect_err("倒序区间必须被拒")
            .kind,
        adm4_foundation::Adm4ErrorKind::InvalidInput
    );

    // --- 人工门：没停在等待态就不接受确认；空署名一律拒（R3）---
    assert_eq!(
        services
            .build_confirm(&archive_id, "P0", "评审员甲", "想直接放行")
            .expect_err("阻塞的段不是人工门")
            .kind,
        adm4_foundation::Adm4ErrorKind::Conflict
    );
    assert_eq!(
        services
            .build_confirm(&archive_id, "P0", "   ", "匿名放行")
            .expect_err("匿名确认等于没有评审")
            .kind,
        adm4_foundation::Adm4ErrorKind::InvalidInput
    );

    // --- 协作式取消：停在段边界，被取消的段记为未运行而非失败 ---
    let cancel = CancelSignal::new();
    cancel.cancel();
    let outcome = services
        .build_run_with_cancel(&archive_id, "P0", "P5", &cancel)
        .expect("取消是正常结束");
    assert_eq!(outcome.cancelled_at.as_deref(), Some("P0"));
    assert_eq!(outcome.state.stage_status("P0"), StageStatus::Pending);

    // --- 强制重跑：重置目标段及全部下游（此处无产物可清，如实报空）---
    let rerun = services.build_rerun(&archive_id, "P0", "P5").unwrap();
    assert_eq!(
        rerun.reset.reset_stages,
        vec!["P0", "P1", "P2", "P3", "P4", "P5"]
    );
    assert!(
        rerun.reset.cleared_artifacts.is_empty(),
        "诚实空执行器一份产物都没写过，不许虚报清空"
    );
    assert!(rerun.reset.revoked_confirmations.is_empty());
    assert!(matches!(
        rerun.state.stage_status("P0"),
        StageStatus::Blocked { .. }
    ));

    // --- 两段流水线互不干扰：Phase 2 跑了这么多轮，C0 的成功与产物原样还在 ---
    let phase1 = services.pipeline_status(&archive_id).unwrap();
    assert!(
        phase1.is_succeeded("C0"),
        "构建段的运行状态不得覆盖文档编译段"
    );
    let c0 = services.pipeline_artifact(&archive_id, 1, "C0").unwrap();
    assert!(c0.complete, "C0 产物应仍然齐备");

    // --- 审计：构建动作进运行日志（分类 build，与 pipeline 分开）---
    let logs = services.log.tail(200).unwrap();
    assert!(
        logs.iter()
            .any(|entry| entry.category == "build" && entry.message.contains("构建运行")),
        "构建运行必须留痕"
    );
    assert!(
        logs.iter()
            .any(|entry| entry.category == "build" && entry.message.contains("被用户取消")),
        "取消不是失败，但必须可追查"
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// G2 场景：设计阶段美术风格锚点门（册 08 §2，选项 A）全链
//
// 走完 未配图像通道诚实 blocked → 生成 3-5 方向（提示词锚定真源）→ 改词重生成 →
// attended 署名确认 → 锁定 style_anchor_set / style_application_contract →
// P2 就绪查询转绿 → 重选风格另立新版（旧版不动）。
// 全程零网络：图像走 `ScriptedImageProvider`（确定性占位 PNG，provider id 落盘可辨）。
// ---------------------------------------------------------------------------

#[test]
fn style_anchor_gate_runs_the_full_design_stage_chain() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_g2_style_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    // 风格门在冻结**之前**也能跑；这里复用已冻结夹具是为了拿一个画像点齐备的项目，
    // 并顺带验证「冻结之后重选风格」照旧另立新版。
    let archive_id = frozen_lane_defense_project_named(&services, &ai, "风格门验证项目");

    // --- ① 没配图像通道：生成入口诚实 blocked，且必须说清缺什么配置 ---
    let doctor = services.image_doctor();
    assert!(!doctor.available, "本用例不配置图像通道");
    assert!(
        doctor.detail.contains("image_provider"),
        "体检要指名缺哪一段配置：{}",
        doctor.detail
    );
    let error = services
        .style_generate(&archive_id, 3, false)
        .expect_err("没有图像通道就是 blocked，不许产占位图冒充（R7）");
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::AiUnavailable);
    for needle in ["image_provider", "base_url", "占位图"] {
        assert!(error.message.contains(needle), "{}", error.message);
    }

    // --- ② 还没生成：状态可查，下游可判定被阻断 ---
    let status = services.style_status(&archive_id).unwrap();
    assert!(!status.session_present);
    assert!(status.directions.is_empty());
    assert!(status.anchor_versions.is_empty());
    assert!(!status.readiness.ready);
    assert!(
        status
            .readiness
            .detail
            .contains(adm4_app::STYLE_APPLICATION_CONTRACT_NOT_APPROVED),
        "阻断结论必须带册 08 §3 的阻断码：{}",
        status.readiness.detail
    );
    let readiness = services.style_readiness(&archive_id).unwrap();
    let blocked = readiness.require_ready().expect_err("未确认必须阻断下游");
    assert_eq!(blocked.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    assert!(services.style_session(&archive_id).unwrap().is_none());

    // --- ③ 生成 4 个方向（零网络的确定性图像通道）---
    let images = ScriptedImageProvider::new();
    let options = StyleGenerationOptions {
        direction_count: 4,
        preview_width: 32,
        preview_height: 24,
        force: false,
    };
    let session = services
        .style_generate_with(&archive_id, &images, &options)
        .expect("生成风格方向");
    assert_eq!(session.directions.len(), 4);
    assert_eq!(session.rounds.len(), 1);
    assert_eq!(session.recommended_count(), 1, "恰好标一个推荐方向");
    // 提示词锚定真源：夹具确认了 4 个画像点（u.genre / u.platform / u.business_model / u.experience）。
    assert_eq!(session.source_anchors.len(), 4);
    assert!(
        session
            .source_summary
            .iter()
            .any(|line| line.starts_with("u.genre")),
        "真源摘要要指得出具体决策点：{:?}",
        session.source_summary
    );
    for direction in &session.directions {
        assert!(direction.style_id.starts_with("STYLE-"));
        assert!(
            direction.derived_prompt.contains("风格门验证项目"),
            "提示词里要有项目名：{}",
            direction.derived_prompt
        );
        assert!(
            direction.derived_prompt.contains("lane_defense"),
            "提示词里要有品类包：{}",
            direction.derived_prompt
        );
        assert_eq!(direction.prompt_anchors.len(), 4);
        let preview = direction.preview.as_ref().expect("每个方向都要有预览图");
        assert_eq!(preview.provider_id, "scripted_image");
        // 呈现层按相对路径拿绝对路径加载图片；文件必须真在。
        let path = services
            .style_image_path(&archive_id, &preview.image_path)
            .expect("预览图应可定位");
        assert!(path.is_file());
        let bytes = std::fs::read(&path).expect("读预览图");
        assert_eq!(&bytes[..4], b"\x89PNG", "落盘的是真 PNG，界面才画得出来");
    }
    assert_eq!(images.calls().len(), 4, "一个方向一次图像调用");

    // 越界路径与不存在的图一律显式报错（不返回一个让界面显示空白的路径）。
    assert_eq!(
        services
            .style_image_path(&archive_id, "../../secrets.json")
            .expect_err("越界路径必须被拒")
            .kind,
        adm4_foundation::Adm4ErrorKind::PathEscape
    );
    assert!(
        services
            .style_image_path(&archive_id, "previews/r0001/STYLE-99-nope.png")
            .is_err()
    );

    // --- ④ 对话式改词重生成（次数不限，每轮留痕）---
    let target = session.directions[1].style_id.clone();
    let derived = session.directions[1].derived_prompt.clone();
    let updated = services
        .style_regenerate_with(
            &archive_id,
            &target,
            "colder palette, dusk lighting, thicker outlines",
            &images,
        )
        .expect("改词重生成");
    let direction = updated.direction(&target).expect("方向仍在");
    assert_eq!(
        direction.effective_prompt(),
        "colder palette, dusk lighting, thicker outlines"
    );
    assert_eq!(direction.derived_prompt, derived, "派生提示词不被改词覆盖");
    assert_eq!(updated.rounds.len(), 2);
    assert_eq!(
        direction
            .preview
            .as_ref()
            .map(|item| item.round_id.as_str()),
        Some("r0002")
    );

    // R5：提示词里写参考游戏名一律被拒（册 08 §5 把提示词列为强制扫描点）。
    let error = services
        .style_regenerate_with(
            &archive_id,
            &target,
            "make it look like Kingdom Rush",
            &images,
        )
        .expect_err("提示词命中换皮词必须被拒");
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
    assert!(error.message.contains("kingdom rush"), "{}", error.message);
    // 被拒之后工作态没被污染。
    assert_eq!(
        services
            .style_session(&archive_id)
            .unwrap()
            .and_then(|session| session.direction(&target).cloned())
            .map(|direction| direction.prompt_override)
            .unwrap_or_default(),
        "colder palette, dusk lighting, thicker outlines"
    );

    // --- ⑤ attended 确认：署名与结论双必填（R3），拒绝在服务层 ---
    for (actor, note) in [("   ", "就它了"), ("主美甲", "  ")] {
        let error = services
            .style_confirm(&archive_id, &target, actor, note)
            .expect_err("署名与结论缺一不可");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
        assert!(error.message.contains("R3"), "{}", error.message);
    }
    assert!(
        !services.style_readiness(&archive_id).unwrap().ready,
        "被拒之后仍旧未确认"
    );

    // --- ⑥ 确认锁定：四件产物齐备且互相对得上 ---
    let outcome = services
        .style_confirm(
            &archive_id,
            &target,
            "主美甲",
            "四个方向都看过大图，选它兼顾可读性与氛围",
        )
        .expect("确认");
    let anchor_set = &outcome.anchor_set;
    assert_eq!(anchor_set.anchor_version, 1);
    assert_eq!(anchor_set.selected_style_id, target);
    assert_eq!(
        anchor_set.final_prompt,
        "colder palette, dusk lighting, thicker outlines"
    );
    assert!(anchor_set.prompt_overridden);
    assert_eq!(anchor_set.project_name, "风格门验证项目");
    assert_eq!(anchor_set.source_anchors.len(), 4);
    assert_eq!(anchor_set.confirmation.actor, "主美甲");
    assert_eq!(anchor_set.anchors.len(), 1);
    let anchor = anchor_set.selected_anchor().expect("选中锚图");
    assert_eq!(anchor.image_path, format!("anchors/v1/{target}.png"));
    assert!(
        services
            .style_image_path(&archive_id, &anchor.image_path)
            .unwrap()
            .is_file()
    );
    let contract = &outcome.application_contract;
    assert_eq!(contract.style_constraints.len(), 5, "五类用途全覆盖");
    assert_eq!(contract.prompt_prefix, anchor_set.final_prompt);
    assert!(contract.matches(anchor_set).is_ok());
    // 回读与内存里的一致。
    assert_eq!(
        services.style_anchor_set(&archive_id, 1).unwrap(),
        *anchor_set
    );
    assert_eq!(
        services.style_application_contract(&archive_id, 1).unwrap(),
        *contract
    );
    assert_eq!(
        services
            .style_fit_report(&archive_id, 1)
            .unwrap()
            .entries
            .len(),
        4
    );

    // --- ⑦ P2 就绪查询转绿（G1 把「风格锚点集」声明为 P2 的外部输入）---
    let readiness = services.style_readiness(&archive_id).unwrap();
    assert!(readiness.ready);
    assert_eq!(readiness.anchor_version, 1);
    assert_eq!(readiness.selected_style_id, target);
    assert_eq!(readiness.anchor_hash, contract.source_anchor_hash);
    assert!(readiness.require_ready().is_ok());
    let status = services.style_status(&archive_id).unwrap();
    assert_eq!(status.anchor_versions, vec![1]);
    assert_eq!(status.confirmed_actor, "主美甲");
    assert_eq!(
        status
            .directions
            .iter()
            .filter(|row| row.is_selected)
            .count(),
        1
    );
    assert!(!status.anchor_stale, "锚点锚的就是当前 revision");

    // --- ⑧ 重选风格：另立 v2，v1 一个字节都不动（D4 不可变历史）---
    let anchors_dir = services
        .archives
        .content_dir(&archive_id)
        .join(adm4_app::STYLE_SECTION)
        .join("anchors");
    let v1_anchor_set = anchors_dir.join("v1").join(adm4_app::ANCHOR_SET_FILE);
    let v1_bytes = std::fs::read(&v1_anchor_set).expect("读 v1");
    let other = session.directions[0].style_id.clone();
    let second = services
        .style_confirm(&archive_id, &other, "主美乙", "试玩后改走清晰量产")
        .expect("重选风格");
    assert_eq!(second.anchor_set.anchor_version, 2);
    assert_eq!(second.superseded_version, Some(1));
    assert_eq!(
        std::fs::read(&v1_anchor_set).expect("重读 v1"),
        v1_bytes,
        "旧版锚点集必须逐字节不变"
    );
    assert_eq!(
        services
            .style_anchor_set(&archive_id, 1)
            .unwrap()
            .selected_style_id,
        target,
        "v1 仍记着当时选的方向"
    );
    assert_eq!(
        services
            .style_readiness(&archive_id)
            .unwrap()
            .anchor_version,
        2
    );

    // --- ⑨ 图像通道失败：原样上抛且记录在案，下一次可续跑（R7）---
    let broken = ScriptedImageProvider::new();
    broken.fail_with("图像 API 返回 503：上游不可用");
    let mut forced = options.clone();
    forced.force = true;
    let error = services
        .style_generate_with(&archive_id, &broken, &forced)
        .expect_err("图像失败必须原样上抛");
    assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::AiUnavailable);
    assert!(error.message.contains("上游不可用"), "{}", error.message);
    let after = services
        .style_session(&archive_id)
        .unwrap()
        .expect("工作态");
    assert_eq!(after.pending_style_ids().len(), 4, "失败的方向如实标缺图");
    // 已锁定的两版历史一概不受影响：下游照旧按 v2 生产。
    let readiness = services.style_readiness(&archive_id).unwrap();
    assert!(readiness.ready);
    assert_eq!(readiness.anchor_version, 2);

    let recovered = services
        .style_generate_with(&archive_id, &images, &options)
        .expect("续跑补齐");
    assert!(recovered.pending_style_ids().is_empty());

    // --- ⑩ 审计：风格动作进运行日志（分类 style）---
    let logs = services.log.tail(400).unwrap();
    for needle in ["风格方向生成", "风格锚点 v1 已确认", "被 v2 取代"] {
        assert!(
            logs.iter()
                .any(|entry| entry.category == "style" && entry.message.contains(needle)),
            "运行日志缺「{needle}」"
        );
    }
    assert!(
        logs.iter().any(|entry| entry.category == "style"
            && entry.message.contains("生成失败")
            && entry.message.contains("不产占位图")),
        "生成失败也必须留痕（R7）"
    );

    // --- ⑪ 存档体检仍一致：风格产物纳入内容指纹 ---
    let doctor = services.project_doctor(&archive_id).unwrap();
    assert!(doctor.healthy, "{:?}", doctor.problems);

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// G3 场景：Phase 2 资产生产线（P0 两条线派生 + P2 资产批量生产）全链
//
// 走完 冻结→C0-C4→风格确认→P0 派生成功→P2 预算门申报停下→署名批准→Scripted 图像
// 批量生产→台账/基因表/一致性断言→重跑 P2 缓存全命中（零图像调用）→代表资产锚图追加。
// 全程零网络：文本走 ScriptedProvider、图像走 ScriptedImageProvider。
// ---------------------------------------------------------------------------

#[test]
fn asset_production_line_runs_p0_and_p2_end_to_end() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_g3_assets_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let archive_id = frozen_lane_defense_project_named(&services, &ai, "资产产线验证项目");

    // --- 前置：跑 Phase 1 到 C4（P0 消费 C3/C4）+ 风格门确认（P2 消费锚点集）---
    let phase1 = services
        .pipeline_run_with(&archive_id, "C0", "C4", &ai)
        .unwrap();
    assert!(phase1.is_succeeded("C4"), "C4 应成功：{phase1:?}");
    let images = ScriptedImageProvider::new();
    let options = services.style_options(3, false).unwrap();
    services
        .style_generate_with(&archive_id, &images, &options)
        .unwrap();
    let style_id = services
        .style_session(&archive_id)
        .unwrap()
        .expect("工作态在案")
        .directions[0]
        .style_id
        .clone();
    services
        .style_confirm(&archive_id, &style_id, "主美甲", "产线验证选定方向")
        .unwrap();
    let calls_after_style = images.calls().len();

    // --- P0：两条线派生 + 对齐合流，真跑成功 ---
    let state = services.build_run(&archive_id, "P0", "P0").unwrap();
    assert!(
        state.is_succeeded("P0"),
        "P0 应派生成功：{:?}",
        state.stage_status("P0")
    );
    // P0 契约落盘且结构可读：两条线 + 资产表 + 对齐报告 + 引擎种子（G4a 真产；未配引擎 → engine_id none）。
    let version = services.latest_frozen_version(&archive_id).unwrap();
    let p0: adm4_build::TwoLineContract = {
        let content_dir = services.archives.content_dir(&archive_id);
        let path = content_dir
            .join("build")
            .join(format!("v{version}"))
            .join("P0")
            .join("contract.json");
        serde_json::from_str(&std::fs::read_to_string(&path).expect("P0 契约在盘")).expect("可解析")
    };
    assert_eq!(p0.engine_seed.status, "produced", "引擎种子由真源派生");
    let seed = p0.engine_seed.seed.as_ref().expect("真种子在案");
    assert_eq!(seed.engine_id, "none", "未配引擎时种子如实标 none");
    assert!(!seed.project_dir_name.is_empty());
    assert!(!p0.program.systems.is_empty());
    assert!(!p0.art.assets.is_empty());
    assert!(
        p0.alignment.is_clean(),
        "{:?}",
        p0.alignment.unresolved_conflicts
    );
    assert!(p0.authority.passed());

    // --- 区间语义：P0..P2 会先撞上 P1——lane_defense 经 P0 派生 3 个主操作候选未收敛，
    // 切片抽取按 R2「未知即停」落 Blocked（设计侧问题，非程序错误），P2 不被推进 ---
    let state = services.build_run(&archive_id, "P0", "P2").unwrap();
    assert!(
        matches!(state.stage_status("P1"), StageStatus::Blocked { .. }),
        "P1 因主操作候选未收敛必须诚实 Blocked：{:?}",
        state.stage_status("P1")
    );
    assert_eq!(state.stage_status("P2"), StageStatus::Pending);

    // --- P2 首跑（单段区间；依赖检查只看 P0 已成功）：预算门申报并停下（R3）---
    let state = services.build_run(&archive_id, "P2", "P2").unwrap();
    match state.stage_status("P2") {
        StageStatus::Blocked { reasons } => {
            assert!(reasons[0].contains("预算"), "{}", reasons[0]);
            assert!(reasons[0].contains("budget-confirm"), "{}", reasons[0]);
        }
        other => panic!("P2 首跑应停在预算门，实际 {other:?}"),
    }
    assert_eq!(
        images.calls().len(),
        calls_after_style,
        "预算未批一张图都不许生成"
    );
    let budget = services
        .build_budget(&archive_id)
        .unwrap()
        .expect("预算已申报");
    assert_eq!(budget.declared_assets.len(), p0.art.assets.len());

    // 匿名批准被拒（R3）。
    assert!(
        services
            .build_budget_confirm(&archive_id, "  ", "放行")
            .is_err()
    );
    services
        .build_budget_confirm(&archive_id, "制作人甲", "首轮产线验证，成本可接受")
        .unwrap();

    // --- 未配图像通道：P2 如实 Blocked 报通道不可用（不产占位图，R7）---
    let state = services.build_run(&archive_id, "P2", "P2").unwrap();
    match state.stage_status("P2") {
        StageStatus::Blocked { reasons } => {
            assert!(reasons[0].contains("图像生成通道不可用"), "{}", reasons[0]);
        }
        other => panic!("未配图像通道 P2 应诚实 Blocked，实际 {other:?}"),
    }

    // --- P2 生产（注入 Scripted 图像通道）：批量生产 + 台账 + 基因表 + 一致性 ---
    let production_images = ScriptedImageProvider::new();
    let outcome = services
        .build_run_with_images(
            &archive_id,
            "P2",
            "P2",
            &adm4_pipeline::CancelSignal::never(),
            Some(Box::new(production_images.clone())),
        )
        .unwrap();
    assert!(
        outcome.state.is_succeeded("P2"),
        "P2 应生产成功：{:?}",
        outcome.state.stage_status("P2")
    );
    let asset_count = p0.art.assets.len();
    assert_eq!(
        production_images.calls().len(),
        asset_count,
        "每个资产恰好一次生成调用"
    );

    // P2 契约：台账七字段 + 基因表对账零差异 + 确定性比对全 Ok + 修复队列空。
    let record: adm4_build::art::genome_backfill::AssetProductionRecord = {
        let path = services
            .archives
            .content_dir(&archive_id)
            .join("build")
            .join(format!("v{version}"))
            .join("P2")
            .join("contract.json");
        serde_json::from_str(&std::fs::read_to_string(&path).expect("P2 契约在盘")).expect("可解析")
    };
    assert!(
        record.clean(),
        "对账差异 {:?} / 修复队列 {:?}",
        record.genome_drifts,
        record.repair_queue
    );
    assert_eq!(record.ledger.entries.len(), asset_count);
    assert_eq!(record.ledger.generation_calls, asset_count);
    assert_eq!(record.anchor_version, 1);
    for entry in &record.ledger.entries {
        assert!(!entry.prompt.is_empty(), "台账必须记完整提示词");
        assert!(
            !entry.fallback.is_empty(),
            "Fallback 字段必须被回答（godogen 七字段）"
        );
        assert!(
            entry.in_game_size.is_none(),
            "未实测的入游尺寸如实 None（R1）"
        );
        // 资产文件真实落盘在暂存资产根，路径 = 资产表登记的运行时加载路径。
        let on_disk = services
            .archives
            .content_dir(&archive_id)
            .join("build")
            .join(format!("v{version}"))
            .join("assets")
            .join(&entry.runtime_path);
        assert!(on_disk.is_file(), "资产未落盘：{}", on_disk.display());
    }
    // 预算实耗如实入账。
    let budget = services
        .build_budget(&archive_id)
        .unwrap()
        .expect("预算在案");
    assert_eq!(budget.consumed_calls, asset_count);

    // --- 重跑 P2：内容哈希缓存全命中，一次图像调用都不发（省钱），预算实耗不涨 ---
    let calls_before_rerun = production_images.calls().len();
    let rerun = services
        .build_rerun_with_images(
            &archive_id,
            "P2",
            "P2",
            &adm4_pipeline::CancelSignal::never(),
            Some(Box::new(production_images.clone())),
        )
        .unwrap();
    assert!(
        rerun.state.is_succeeded("P2"),
        "缓存命中的重跑应成功：{:?}",
        rerun.state.stage_status("P2")
    );
    assert_eq!(
        production_images.calls().len(),
        calls_before_rerun,
        "缓存全命中：零新增图像调用"
    );
    let budget = services
        .build_budget(&archive_id)
        .unwrap()
        .expect("预算在案");
    assert_eq!(budget.consumed_calls, asset_count, "缓存命中不占预算额度");
    let record: adm4_build::art::genome_backfill::AssetProductionRecord = {
        let path = services
            .archives
            .content_dir(&archive_id)
            .join("build")
            .join(format!("v{version}"))
            .join("P2")
            .join("contract.json");
        serde_json::from_str(&std::fs::read_to_string(&path).expect("P2 契约在盘")).expect("可解析")
    };
    assert_eq!(
        record.ledger.cache_hits, asset_count,
        "重跑的台账如实记缓存来源"
    );
    assert_eq!(record.ledger.generation_calls, 0);

    // --- 代表资产锚图：以新锚点版本追加，旧版逐字节不变 ---
    let v1_bytes = {
        let path = services
            .archives
            .content_dir(&archive_id)
            .join("style")
            .join("anchors")
            .join("v1")
            .join("anchor_set.json");
        std::fs::read(&path).expect("v1 锚点集在盘")
    };
    let appended = services
        .style_append_representatives_with(&archive_id, &production_images)
        .expect("追加代表锚图");
    assert_eq!(appended.anchor_version, 2, "追加 = 新版本");
    assert!(
        appended.anchors.len() > 1,
        "新版含选中锚图 + 代表资产锚图：{:?}",
        appended.anchors.len()
    );
    assert!(
        appended
            .anchors
            .iter()
            .any(|anchor| anchor.role == "representative_asset"),
        "必须有代表资产角色的锚图"
    );
    let v1_after = {
        let path = services
            .archives
            .content_dir(&archive_id)
            .join("style")
            .join("anchors")
            .join("v1")
            .join("anchor_set.json");
        std::fs::read(&path).expect("v1 仍在盘")
    };
    assert_eq!(v1_bytes, v1_after, "旧锚点版本逐字节不变（不可变历史）");
    // 就绪查询指向新版本。
    let readiness = services.style_readiness(&archive_id).unwrap();
    assert!(readiness.ready);
    assert_eq!(readiness.anchor_version, 2);

    // --- 审计：生产与预算动作都留痕 ---
    let logs = services.log.tail(300).unwrap();
    assert!(
        logs.iter()
            .any(|entry| entry.category == "build" && entry.message.contains("资产预算批准")),
        "预算批准必须留痕（R3）"
    );
    assert!(
        logs.iter()
            .any(|entry| entry.category == "style" && entry.message.contains("代表资产锚图")),
        "锚图追加必须留痕"
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// G4a 场景：P0 引擎工程种子真产 + P1 可玩切片现场开发（Mock 引擎后端）
//
// 四条路径：(a) Mock 就绪 → P0→P1 Succeeded，四份 durable docs + 轮次日志落盘，
// 后端被调 OpenOrCreateProject + AgentDevelop；(b) Mock 未就绪 → P1 Blocked、detail 透传、
// 不调 AgentDevelop；(c) 无引擎配置 → P1 Blocked 含未配置原因；(d) P0 旧档 pending_g4 可读。
// ---------------------------------------------------------------------------

/// 测试用共享后端：`MockEngineBackend` 移交给门面后仍要能读 `calls()`，
/// 用 `Arc` 包一层并转发全部接口（只在测试里存在，不进库代码）。
struct SharedMockEngine(std::sync::Arc<adm4_app::MockEngineBackend>);

impl adm4_app::EngineBackend for SharedMockEngine {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn preflight(&self) -> adm4_foundation::Adm4Result<adm4_build::engine::EnginePreflight> {
        self.0.preflight()
    }
    fn open_or_create_project(
        &self,
        seed: &adm4_build::engine::EngineProjectSeed,
        dir: &Path,
    ) -> adm4_foundation::Adm4Result<()> {
        self.0.open_or_create_project(seed, dir)
    }
    fn agent_develop(
        &self,
        task: &adm4_build::engine::SliceTask,
        ctx: &adm4_build::engine::DevContext,
    ) -> adm4_foundation::Adm4Result<adm4_app::EngineDevRound> {
        self.0.agent_develop(task, ctx)
    }
    fn run_playmode(
        &self,
        project: &Path,
    ) -> adm4_foundation::Adm4Result<adm4_build::engine::RunResult> {
        self.0.run_playmode(project)
    }
    fn capture_proof(
        &self,
        project: &Path,
    ) -> adm4_foundation::Adm4Result<adm4_build::engine::ProofBundle> {
        self.0.capture_proof(project)
    }
}

fn mock_engine(ready: bool) -> std::sync::Arc<adm4_app::MockEngineBackend> {
    std::sync::Arc::new(adm4_app::MockEngineBackend::new(
        "mock_engine",
        adm4_app::MockEngineScript {
            preflight_ready: ready,
            rounds: vec![adm4_app::EngineDevRound {
                index: 0,
                commands: vec!["mock: build".into()],
                failures: Vec::new(),
                repair_summary: "一轮成功".into(),
                status: adm4_app::EngineDevRoundStatus::Succeeded,
            }],
            ..adm4_app::MockEngineScript::default()
        },
    ))
}

/// 跑 Phase 1 到 C4 并返回 (archive_id, 构建仓 v{N} 目录)。
fn prepared_for_build(
    services: &AppServices,
    ai: &ScriptedProvider,
    name: &str,
) -> (String, PathBuf) {
    let archive_id = frozen_lane_defense_project_named(services, ai, name);
    let phase1 = services
        .pipeline_run_with(&archive_id, "C0", "C4", ai)
        .unwrap();
    assert!(phase1.is_succeeded("C4"), "C4 应成功：{phase1:?}");
    let version = services.latest_frozen_version(&archive_id).unwrap();
    let build_root = services
        .archives
        .content_dir(&archive_id)
        .join("build")
        .join(format!("v{version}"));
    (archive_id, build_root)
}

// P1 三条后端门控用例（Mock 成功链 / 预检未就绪 / 未配引擎）已按 T-G4a-3 裁决 2 移到
// dm4-build/src/executors.rs 的 p1_integration 集成测试：lane_defense 全链夹具经 P0 派生
// 3 个主操作候选（设计侧未收敛），在 e2e 里靶它只能验到切片阻塞而验不到后端门控。
// 下面这条只验门面注入接缝 + R2 阻断本身：即便注入就绪的 Mock，P1 也在切片抽取处 Blocked
// 点名三个候选，且后端连预检都不该被调用（切片在预检之前）。

#[test]
fn p1_with_injected_mock_engine_blocks_on_unconverged_primary_input_before_preflight() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_g4a_p1_r2_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let (archive_id, build_root) = prepared_for_build(&services, &ai, "主操作未收敛验证项目");

    let engine = mock_engine(true);
    let outcome = services
        .build_run_with_engine(
            &archive_id,
            "P0",
            "P1",
            &CancelSignal::never(),
            None,
            Some(Box::new(SharedMockEngine(std::sync::Arc::clone(&engine)))),
        )
        .unwrap();
    assert!(
        outcome.state.is_succeeded("P0"),
        "{:?}",
        outcome.state.stage_status("P0")
    );
    let p0: adm4_build::TwoLineContract = serde_json::from_str(
        &std::fs::read_to_string(build_root.join("P0").join("contract.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        p0.engine_seed.seed.as_ref().expect("真种子").engine_id,
        "mock_engine"
    );
    match outcome.state.stage_status("P1") {
        StageStatus::Blocked { reasons } => {
            let joined = reasons.join("\n");
            assert!(joined.contains("主操作"), "{joined}");
            for candidate in [
                "cap_ld.counter_damage",
                "cap_ld.deploy_cost",
                "cap_ld.income_rule",
            ] {
                assert!(
                    joined.contains(candidate),
                    "应点名候选 {candidate}：{joined}"
                );
            }
        }
        other => panic!("P1 应因主操作候选未收敛 Blocked（R2），实际 {other:?}"),
    }
    assert!(
        engine.calls().is_empty(),
        "切片抽取在预检之前阻塞，后端不应被调用：{:?}",
        engine.calls()
    );
    // P1 未落契约但运行状态已 Blocked：摘要如实标「契约不在盘」并带阻塞原因，切片字段留空。
    let summary = services.build_p1_summary(&archive_id).unwrap();
    assert!(!summary.contract_present);
    assert!(summary.scene.is_empty() && summary.primary_input.is_empty());
    assert_eq!(summary.engine_id, "mock_engine");
    assert!(
        summary
            .blocked_reasons_hint
            .iter()
            .any(|r| r.contains("主操作")),
        "{:?}",
        summary.blocked_reasons_hint
    );

    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn legacy_p0_contract_with_pending_g4_seed_is_still_readable_and_p1_points_to_rerun() {
    let temp = std::env::temp_dir().join(format!("adm4_e2e_g4a_p0_legacy_{}", std::process::id()));
    let services = services_with_isolated_space(&temp);
    let ai = scripted_ai();
    let (archive_id, build_root) = prepared_for_build(&services, &ai, "旧档兼容验证项目");

    let state = services.build_run(&archive_id, "P0", "P0").unwrap();
    assert!(state.is_succeeded("P0"));
    // 把 P0 契约改写成 G3 旧档形态（status=pending_g4、无 seed 键），模拟升级前产物。
    let path = build_root.join("P0").join("contract.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json["engine_seed"] = serde_json::json!({
        "status": "pending_g4",
        "note": "引擎工程种子归 G4（册 09）"
    });
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    let legacy: adm4_build::TwoLineContract =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).expect("旧档必须可读");
    assert_eq!(legacy.engine_seed.status, "pending_g4");
    assert!(legacy.engine_seed.seed.is_none());
    assert!(!legacy.program.systems.is_empty(), "其余字段不受影响");

    // P1 读到旧档：如实阻塞并指路重跑 P0，不凭空造种子。
    let state = services.build_run(&archive_id, "P1", "P1").unwrap();
    match state.stage_status("P1") {
        StageStatus::Blocked { reasons } => {
            assert!(reasons[0].contains("pending_g4"), "{}", reasons[0]);
            assert!(reasons[0].contains("build rerun"), "{}", reasons[0]);
        }
        other => panic!("P1 读旧档应 Blocked 指路，实际 {other:?}"),
    }

    std::fs::remove_dir_all(&temp).ok();
}
