//! 逆向工具链产线全链测试（T3）：本地语料检索 → AI 映射 → 交叉核验 → 人工审核 →
//! 认证入库 → 换皮词表登记；负例覆盖跳级被拒（D8）、无证据答案被拒（R1）、
//! 非法 AI 输出即停（R7）。AI 一律用 ScriptedProvider，零网络。

use adm4_ai::ScriptedProvider;
use adm4_decision::{DecisionOption, DecisionPoint, DesignLevel, GenreScope, ParameterValues};
use adm4_foundation::Adm4ErrorKind;
use adm4_template::{
    CROSSCHECK_PURPOSE, Certification, CertificationStatus, Confidence, CrossCheckService,
    CrossCheckVerdict, EvidenceQuery, EvidenceSearchChannel, FileCorpusChannel, MAPPING_PURPOSE,
    MappingService, SourceType, Template, TemplateLibrary, TemplateOrigin, load_skin_wordlist,
};
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 测试基座
// ---------------------------------------------------------------------------

/// 唯一临时目录（进程号 + 用例标签），用完自清理。
fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("adm4_t3_{tag}_{}", std::process::id()));
    fs::remove_dir_all(&root).ok();
    root
}

const SNAPSHOT_JSON: &str = r#"[
  {
    "source_url": "https://wiki.example/combat",
    "title": "Combat overview",
    "snippet": "克制伤害倍率 counter damage multiplier 2.0",
    "source_type": "wiki",
    "fetched_hash": "sha256:combat"
  },
  {
    "source_url": "https://official.example/deploy",
    "title": "Deploy guide",
    "snippet": "grid deploy 部署费用与网格规则",
    "source_type": "official",
    "fetched_hash": "sha256:deploy"
  },
  {
    "source_url": "https://blog.example/story",
    "title": "Story background",
    "snippet": "剧情背景设定",
    "source_type": "inference",
    "fetched_hash": "sha256:story"
  }
]"#;

fn write_corpus(root: &Path, game: &str) -> PathBuf {
    let corpus = root.join("corpus");
    let game_dir = corpus.join(game);
    fs::create_dir_all(&game_dir).unwrap();
    fs::write(game_dir.join("snapshot.json"), SNAPSHOT_JSON).unwrap();
    corpus
}

fn point(id: &str, question: &str, options: &[(&str, &str)]) -> DecisionPoint {
    DecisionPoint {
        id: id.into(),
        domain: "test".into(),
        level: DesignLevel::L4,
        genre_scope: GenreScope::Pack("lane_test".into()),
        question: question.into(),
        mda_layer: None,
        design_question: None,
        node_id: None,
        selection_mode: Default::default(),
        requirement: Default::default(),
        tier_gate: None,
        options: options
            .iter()
            .map(|(option_id, label)| DecisionOption {
                id: (*option_id).into(),
                label: (*label).into(),
                ..Default::default()
            })
            .collect(),
        skin_fields: vec![],
        evidence_slots: true,
    }
}

fn test_points() -> Vec<DecisionPoint> {
    vec![
        point(
            "t.combat",
            "战斗核心机制是什么？",
            &[("counter_combat", "克制战斗"), ("aura_combat", "光环战斗")],
        ),
        point(
            "t.deploy",
            "部署规则是什么？",
            &[("grid_deploy", "网格部署"), ("free_deploy", "自由部署")],
        ),
    ]
}

fn draft_template() -> Template {
    Template {
        template_id: "tpl_galaxy".into(),
        game_name: "galaxy_guard".into(),
        aliases: vec!["Galaxy Guard".into()],
        genre_pack: "lane_test".into(),
        pack_version: "1.0.0".into(),
        depth_reached: DesignLevel::L4,
        answers: vec![],
        certification: Certification::default(),
        origin: TemplateOrigin::Reverse,
        mapping_hash: String::new(),
        crosscheck_proof: None,
        smoke_test: false,
    }
}

const MAPPING_OK: &str = r#"[
  {
    "decision_id": "t.combat",
    "option_id": "counter_combat",
    "evidence": [
      {"source_url": "https://wiki.example/combat", "quote": "克制伤害倍率 2.0", "confidence": "high"}
    ],
    "parameters": {"base_multiplier": 2.0},
    "notes": "wiki 明确记载"
  },
  {
    "decision_id": "t.deploy",
    "option_id": "grid_deploy",
    "evidence": [
      {"source_url": "https://official.example/deploy", "confidence": "med"}
    ]
  }
]"#;

const CROSSCHECK_MIXED: &str = r#"[
  {"decision_id": "t.combat", "verdict": "consistent", "reason": "证据直接支撑所选选项"},
  {"decision_id": "t.deploy", "verdict": "conflict", "reason": "两处来源对部署规则描述矛盾"}
]"#;

// ---------------------------------------------------------------------------
// S1 本地语料检索
// ---------------------------------------------------------------------------

#[test]
fn file_corpus_channel_matches_keywords_case_insensitively() {
    let root = temp_root("corpus");
    let corpus = write_corpus(&root, "galaxy_guard");
    let channel = FileCorpusChannel::new(&corpus);
    assert_eq!(channel.channel_id(), "file_corpus");

    // 大小写不敏感命中 title/snippet；不相关候选被过滤。
    let hits = channel
        .search(&EvidenceQuery {
            game_name: "galaxy_guard".into(),
            decision_question: "战斗机制".into(),
            keywords: vec!["COUNTER".into()],
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source_url, "https://wiki.example/combat");
    assert_eq!(hits[0].source_type, SourceType::Wiki);

    // 多关键词并集，结果按 source_url 排序、去重。
    let hits = channel
        .search(&EvidenceQuery {
            game_name: "galaxy_guard".into(),
            decision_question: String::new(),
            keywords: vec!["counter".into(), "deploy".into()],
        })
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].source_url, "https://official.example/deploy");

    // 无快照的游戏 = 空结果（宁缺勿造，不报错）。
    let hits = channel
        .search(&EvidenceQuery {
            game_name: "unknown_game".into(),
            decision_question: String::new(),
            keywords: vec!["counter".into()],
        })
        .unwrap();
    assert!(hits.is_empty());

    // 空关键词 → 显式报错；游戏名越界 → 路径护栏报错。
    assert!(
        channel
            .search(&EvidenceQuery {
                game_name: "galaxy_guard".into(),
                decision_question: String::new(),
                keywords: vec!["  ".into()],
            })
            .is_err()
    );
    let escape = channel.search(&EvidenceQuery {
        game_name: "../evil".into(),
        decision_question: String::new(),
        keywords: vec!["counter".into()],
    });
    assert_eq!(escape.unwrap_err().kind, Adm4ErrorKind::PathEscape);

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// 全链正例：检索 → 映射 → 交叉核验 → 人工审核 → 认证 → 词表出现别名
// ---------------------------------------------------------------------------

#[test]
fn full_reverse_pipeline_certifies_template_and_registers_aliases() {
    let root = temp_root("full");
    let corpus = write_corpus(&root, "galaxy_guard");
    let library = TemplateLibrary::new(root.join("design_space"));
    let wordlist_path = root.join("design_space").join("skin_wordlist.json");
    let points = test_points();
    let mut template = draft_template();

    // S1：语料检索出证据候选。
    let channel = FileCorpusChannel::new(&corpus);
    let candidates = channel
        .search(&EvidenceQuery {
            game_name: "galaxy_guard".into(),
            decision_question: String::new(),
            keywords: vec!["counter".into(), "deploy".into()],
        })
        .unwrap();
    assert_eq!(candidates.len(), 2);

    // S2：AI 映射填答，Draft→Mapped。
    let provider = ScriptedProvider::new();
    provider.script(MAPPING_PURPOSE, vec![MAPPING_OK.into()]);
    provider.script(CROSSCHECK_PURPOSE, vec![CROSSCHECK_MIXED.into()]);
    let mapped =
        MappingService::map_answers(&provider, &mut template, &points, &candidates).unwrap();
    assert_eq!(mapped, 2);
    assert_eq!(template.certification.status, CertificationStatus::Mapped);
    let combat = &template.answers[0];
    assert_eq!(combat.option_id, "counter_combat");
    assert_eq!(combat.evidence[0].confidence, Confidence::High);
    // 来源类型以检索候选为准。
    assert_eq!(combat.evidence[0].source_type, SourceType::Wiki);
    assert!(matches!(combat.parameters, ParameterValues::Scalars { .. }));
    // AI 未给 quote 时回落到候选 snippet（同源内容，非编造）。
    assert_eq!(
        template.answers[1].evidence[0].quote,
        "grid deploy 部署费用与网格规则"
    );

    // S3：独立二次会话交叉核验，Mapped→CrossChecked；冲突条目标记待人工。
    let report = CrossCheckService::cross_check(&provider, &mut template, &points).unwrap();
    assert_eq!(report.entries.len(), 2);
    assert_eq!(report.conflict_ids(), vec!["t.deploy".to_string()]);
    assert_eq!(report.entries[0].verdict, CrossCheckVerdict::Consistent);
    assert_eq!(
        template.certification.status,
        CertificationStatus::CrossChecked
    );
    assert_eq!(template.answers[0].crosscheck_agreed, Some(true));
    assert_eq!(template.answers[1].crosscheck_agreed, Some(false));
    // R3 机器证据：两会话哈希留档且互异（S3 不是在复读 S2）。
    let proof = template
        .crosscheck_proof
        .clone()
        .expect("S3 应留下两会话机器证据");
    assert_eq!(proof.mapping_hash, template.mapping_hash);
    assert_ne!(proof.crosscheck_hash, proof.mapping_hash);
    assert_eq!(proof.checked_count, 2);
    assert!(!proof.checked_at.is_empty());

    // 认证前不可预填（取用路径强制）。
    library.save_draft(&template).unwrap();
    let premature = library.approved_for_prefill("lane_test", "tpl_galaxy");
    assert_eq!(premature.unwrap_err().kind, Adm4ErrorKind::Blocked);

    // S4：人工审核（署名 + 结论），CrossChecked→HumanReviewed。
    library
        .human_review(
            &mut template,
            "评审员甲",
            "抽查证据链与冲突条目，结论可入库",
        )
        .unwrap();
    assert_eq!(
        template.certification.status,
        CertificationStatus::HumanReviewed
    );
    assert_eq!(template.certification.reviewed_by, "评审员甲");
    assert!(!template.certification.reviewed_at.is_empty());
    assert!(
        library
            .approved_for_prefill("lane_test", "tpl_galaxy")
            .is_err()
    );

    // S5：认证入库，HumanReviewed→Certified；词表自动出现游戏名与别名。
    library.certify(&mut template, &wordlist_path).unwrap();
    assert!(template.is_certified());
    let words = load_skin_wordlist(&wordlist_path).unwrap().words;
    assert!(words.contains(&"galaxy_guard".to_string()));
    assert!(words.contains(&"Galaxy Guard".to_string()));

    // 认证后取用路径放行，且存档状态序列化往返一致。
    let stored = library
        .approved_for_prefill("lane_test", "tpl_galaxy")
        .unwrap();
    assert_eq!(stored.certification.status, CertificationStatus::Certified);
    assert_eq!(stored.answers[1].crosscheck_agreed, Some(false));

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// 负例 1：无证据答案被拒（R1）
// ---------------------------------------------------------------------------

#[test]
fn mapping_rejects_answers_without_evidence() {
    let points = test_points();
    let candidates = snapshot_candidates();
    for bad in [
        // 完全没有 evidence 字段。
        r#"[{"decision_id": "t.combat", "option_id": "counter_combat"}]"#,
        // evidence 是空数组。
        r#"[{"decision_id": "t.combat", "option_id": "counter_combat", "evidence": []}]"#,
    ] {
        let provider = ScriptedProvider::new();
        provider.script(MAPPING_PURPOSE, vec![bad.into()]);
        let mut template = draft_template();
        let result = MappingService::map_answers(&provider, &mut template, &points, &candidates);
        let error = result.unwrap_err();
        assert_eq!(error.kind, Adm4ErrorKind::RedLine);
        assert!(error.message.contains("证据"), "{}", error.message);
        // 整卷拒收：模板保持 Draft 原状，不留半成品。
        assert_eq!(template.certification.status, CertificationStatus::Draft);
        assert!(template.answers.is_empty());
    }

    // 编造候选集之外的来源同样拒收。
    let provider = ScriptedProvider::new();
    provider.script(
        MAPPING_PURPOSE,
        vec![
            r#"[{"decision_id": "t.combat", "option_id": "counter_combat", "evidence": [{"source_url": "https://fake.example/made-up", "confidence": "high"}]}]"#.into(),
        ],
    );
    let mut template = draft_template();
    let error =
        MappingService::map_answers(&provider, &mut template, &points, &candidates).unwrap_err();
    assert!(
        error.message.contains("不在检索候选集内"),
        "{}",
        error.message
    );
    assert_eq!(template.certification.status, CertificationStatus::Draft);
}

fn snapshot_candidates() -> Vec<adm4_template::EvidenceCandidate> {
    serde_json::from_str(SNAPSHOT_JSON).unwrap()
}

// ---------------------------------------------------------------------------
// 负例 2：认证跳级/回退被拒（D8）
// ---------------------------------------------------------------------------

#[test]
fn certification_rejects_skip_and_regression() {
    // 跳级：Draft 直接进 CrossChecked / Certified 均被拒。
    let mut certification = Certification::default();
    assert!(
        certification
            .advance_to(CertificationStatus::CrossChecked)
            .is_err()
    );
    assert!(
        certification
            .advance_to(CertificationStatus::Certified)
            .is_err()
    );
    assert_eq!(certification.status, CertificationStatus::Draft);

    // 逐级前进合法；原地与回退被拒。
    certification
        .advance_to(CertificationStatus::Mapped)
        .unwrap();
    assert!(
        certification
            .advance_to(CertificationStatus::Mapped)
            .is_err()
    );
    assert!(
        certification
            .advance_to(CertificationStatus::Draft)
            .is_err()
    );
    assert_eq!(certification.status, CertificationStatus::Mapped);

    // 终态之后不能再动。
    certification
        .advance_to(CertificationStatus::CrossChecked)
        .unwrap();
    certification
        .record_human_review("评审员乙", "复核通过")
        .unwrap();
    certification
        .advance_to(CertificationStatus::Certified)
        .unwrap();
    assert!(
        certification
            .advance_to(CertificationStatus::Draft)
            .is_err()
    );

    // 库入口同样拦截跳级：Draft 模板直接认证被拒，且词表未被污染。
    let root = temp_root("skip");
    let library = TemplateLibrary::new(root.join("design_space"));
    let wordlist_path = root.join("design_space").join("skin_wordlist.json");
    let mut template = draft_template();
    let error = library.certify(&mut template, &wordlist_path).unwrap_err();
    assert_eq!(error.kind, Adm4ErrorKind::Blocked);
    assert_eq!(template.certification.status, CertificationStatus::Draft);
    assert!(!wordlist_path.exists());

    // 产线服务的状态门：Draft 不能核验，Mapped 不能重复映射。
    let provider = ScriptedProvider::new();
    let points = test_points();
    let mut template = draft_template();
    assert!(CrossCheckService::cross_check(&provider, &mut template, &points).is_err());
    template
        .certification
        .advance_to(CertificationStatus::Mapped)
        .unwrap();
    assert!(MappingService::map_answers(&provider, &mut template, &points, &[]).is_err());

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// 负例 3：AI 非法输出直接 Err（R7）
// ---------------------------------------------------------------------------

#[test]
fn mapping_rejects_illegal_ai_output() {
    let points = test_points();
    let candidates = snapshot_candidates();
    for (bad, needle) in [
        // 非 JSON。
        ("这不是 JSON", "JSON"),
        // 引用不存在的决策点。
        (
            r#"[{"decision_id": "t.ghost", "option_id": "x", "evidence": [{"source_url": "https://wiki.example/combat", "confidence": "high"}]}]"#,
            "不存在的决策点",
        ),
        // 发明选项。
        (
            r#"[{"decision_id": "t.combat", "option_id": "invented", "evidence": [{"source_url": "https://wiki.example/combat", "confidence": "high"}]}]"#,
            "发明选项",
        ),
        // 非法置信度。
        (
            r#"[{"decision_id": "t.combat", "option_id": "counter_combat", "evidence": [{"source_url": "https://wiki.example/combat", "confidence": "maybe"}]}]"#,
            "confidence",
        ),
    ] {
        let provider = ScriptedProvider::new();
        provider.script(MAPPING_PURPOSE, vec![bad.into()]);
        let mut template = draft_template();
        let error = MappingService::map_answers(&provider, &mut template, &points, &candidates)
            .unwrap_err();
        assert!(error.message.contains(needle), "{}", error.message);
        assert_eq!(template.certification.status, CertificationStatus::Draft);
    }
}

#[test]
fn crosscheck_rejects_incomplete_or_invented_reports() {
    let points = test_points();
    let candidates = snapshot_candidates();

    for (bad, needle) in [
        // 漏掉 t.deploy 的结论。
        (
            r#"[{"decision_id": "t.combat", "verdict": "consistent"}]"#,
            "缺少决策点",
        ),
        // 报告了答卷之外的决策点。
        (
            r#"[{"decision_id": "t.combat", "verdict": "consistent"}, {"decision_id": "t.deploy", "verdict": "conflict"}, {"decision_id": "t.ghost", "verdict": "conflict"}]"#,
            "答卷之外",
        ),
        // 非法结论值。
        (
            r#"[{"decision_id": "t.combat", "verdict": "unsure"}, {"decision_id": "t.deploy", "verdict": "conflict"}]"#,
            "核验结论非法",
        ),
    ] {
        let provider = ScriptedProvider::new();
        provider.script(MAPPING_PURPOSE, vec![MAPPING_OK.into()]);
        provider.script(CROSSCHECK_PURPOSE, vec![bad.into()]);
        let mut template = draft_template();
        MappingService::map_answers(&provider, &mut template, &points, &candidates).unwrap();
        let error = CrossCheckService::cross_check(&provider, &mut template, &points).unwrap_err();
        assert!(error.message.contains(needle), "{}", error.message);
        // 核验失败：状态停在 Mapped，逐条结论保持未核验。
        assert_eq!(template.certification.status, CertificationStatus::Mapped);
        assert!(
            template
                .answers
                .iter()
                .all(|answer| answer.crosscheck_agreed.is_none())
        );
    }
}

// ---------------------------------------------------------------------------
// 负例 4：S3 复读 S2 会话（橡皮图章）被拒（R3）
// ---------------------------------------------------------------------------

#[test]
fn crosscheck_rejects_replay_of_mapping_session() {
    let points = test_points();
    let candidates = snapshot_candidates();
    let provider = ScriptedProvider::new();
    provider.script(MAPPING_PURPOSE, vec![MAPPING_OK.into()]);
    // 第二会话原样吐回第一会话的应答 → 两会话内容哈希全同。
    provider.script(CROSSCHECK_PURPOSE, vec![MAPPING_OK.into()]);
    let mut template = draft_template();
    MappingService::map_answers(&provider, &mut template, &points, &candidates).unwrap();

    let error = CrossCheckService::cross_check(&provider, &mut template, &points).unwrap_err();
    assert_eq!(error.kind, Adm4ErrorKind::RedLine);
    assert!(error.message.contains("复读"), "{}", error.message);
    // 拒绝后模板保持 Mapped 原状，不留核验证据。
    assert_eq!(template.certification.status, CertificationStatus::Mapped);
    assert!(template.crosscheck_proof.is_none());
}

// ---------------------------------------------------------------------------
// 通用层模板的跨包取用（F3-3）：列表可见 + 解析可取 + 认证关卡照旧生效
// ---------------------------------------------------------------------------

#[test]
fn universal_templates_are_listed_and_resolvable_from_any_pack() {
    let root = temp_root("universal");
    let space = root.join("design_space");
    let library = TemplateLibrary::new(&space);

    // 本包一份（Certified）+ 通用层一份（Certified）+ 通用层一份草稿。
    //
    // F4d：取用关卡除状态位外还查证据（`require_certification_evidence`），
    // 因此这两份 Certified 夹具补上逆向来源该有的 S2/S3 机器证据——本测试要验证的是
    // 「跨包解析」，不是「无证据也能预填」，断言一条未改。
    let certified_evidence = |template: &mut adm4_template::Template| {
        template.certification.status = CertificationStatus::Certified;
        template.mapping_hash = "sha256:mapping".into();
        template.crosscheck_proof = Some(adm4_template::CrossCheckProof {
            mapping_hash: "sha256:mapping".into(),
            crosscheck_hash: "sha256:crosscheck".into(),
            checked_count: template.answers.len(),
            checked_at: "2026-08-31T00:00:00Z".into(),
        });
    };

    let mut own = draft_template();
    certified_evidence(&mut own);
    library.save_draft(&own).unwrap();

    let mut universal = draft_template();
    universal.template_id = "builtin_universal_ok".into();
    universal.game_name = "虚构通用甲".into();
    universal.genre_pack = "universal".into();
    certified_evidence(&mut universal);
    library.save_draft(&universal).unwrap();

    let mut universal_draft = draft_template();
    universal_draft.template_id = "builtin_universal_draft".into();
    universal_draft.game_name = "虚构通用乙".into();
    universal_draft.genre_pack = "universal".into();
    library.save_draft(&universal_draft).unwrap();

    // list 严格按目录（逆向产线要写回本包目录，不能混进通用层的）。
    let own_only = library.list("lane_test").unwrap();
    assert_eq!(own_only.len(), 1);
    assert_eq!(own_only[0].template_id, "tpl_galaxy");

    // list_available = 本包 + 通用层（这才是 UI 选模板/预填该看的集合）。
    let available = library.list_available("lane_test").unwrap();
    let ids: Vec<&str> = available
        .iter()
        .map(|template| template.template_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            "tpl_galaxy",
            "builtin_universal_draft",
            "builtin_universal_ok"
        ]
    );
    assert!(available[1].is_universal());
    // pack_id 本身是通用层时不重复列出。
    assert_eq!(library.list_available("universal").unwrap().len(), 2);

    // resolve：本包找不到就落通用层；两处都没有 → 报本包的 not-found（不掩盖真实路径）。
    assert_eq!(
        library
            .resolve("lane_test", "builtin_universal_ok")
            .unwrap()
            .genre_pack,
        "universal"
    );
    assert!(library.get("lane_test", "builtin_universal_ok").is_err());
    assert!(library.resolve("lane_test", "no_such_template").is_err());

    // 取用关卡照旧：通用层未认证模板同样被拒（跨包不等于放宽认证）。
    assert_eq!(
        library
            .approved_for_prefill("lane_test", "builtin_universal_ok")
            .unwrap()
            .template_id,
        "builtin_universal_ok"
    );
    let blocked = library
        .approved_for_prefill("lane_test", "builtin_universal_draft")
        .unwrap_err();
    assert_eq!(blocked.kind, Adm4ErrorKind::Blocked);

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// 负例 5（F4b）：逆向来源缺 S2/S3 机器证据，即便状态是 HumanReviewed 也不许认证
// ---------------------------------------------------------------------------

/// 「另存模板」放宽了本项目导出来源的证据要求，逆向来源**一条都不许放松**。
///
/// 造出「状态已到 HumanReviewed 但没有证据」的模板有两条现实路径：手改落盘 json、
/// 以及 S2 重跑后 S3 没跟上（核验证据里的 mapping_hash 与当前映射哈希不一致）。
/// 两条都必须在 S5 被拦下——否则「已认证」三个字失去机器可核的依据。
#[test]
fn certify_rejects_reverse_template_without_crosscheck_evidence() {
    let root = temp_root("reverse_no_evidence");
    let library = TemplateLibrary::new(root.join("design_space"));
    let wordlist_path = root.join("design_space").join("skin_wordlist.json");

    let mut reviewed = draft_template();
    reviewed.certification = Certification {
        status: CertificationStatus::HumanReviewed,
        reviewed_by: "评审员甲".into(),
        reviewed_at: "2026-08-31T00:00:00Z".into(),
        review_note: "手改状态字段伪造的审核".into(),
    };
    assert_eq!(reviewed.origin, TemplateOrigin::Reverse);

    // 1. 完全没有证据。
    let error = library
        .certify(&mut reviewed, &wordlist_path)
        .expect_err("逆向来源缺证据必须被拒");
    assert_eq!(error.kind, Adm4ErrorKind::RedLine);
    assert!(error.message.contains("映射会话哈希"), "{}", error.message);
    assert_eq!(
        reviewed.certification.status,
        CertificationStatus::HumanReviewed,
        "被拒时模板状态一字不改"
    );
    assert!(!wordlist_path.exists(), "被拒时词表不许被污染");

    // 2. 只有 S2 哈希、没有 S3 证据。
    reviewed.mapping_hash = "sha256:mapping".into();
    let error = library
        .certify(&mut reviewed, &wordlist_path)
        .expect_err("缺 S3 两会话证据必须被拒");
    assert_eq!(error.kind, Adm4ErrorKind::RedLine);
    assert!(error.message.contains("交叉核验证据"), "{}", error.message);

    // 3. S3 证据存在但对应的是上一版映射（映射重跑而核验没跟上）。
    reviewed.crosscheck_proof = Some(adm4_template::CrossCheckProof {
        mapping_hash: "sha256:previous".into(),
        crosscheck_hash: "sha256:crosscheck".into(),
        checked_count: 2,
        checked_at: "2026-08-31T00:00:00Z".into(),
    });
    let error = library
        .certify(&mut reviewed, &wordlist_path)
        .expect_err("证据与当前映射哈希不对应必须被拒");
    assert_eq!(error.kind, Adm4ErrorKind::RedLine);
    assert!(error.message.contains("不对应"), "{}", error.message);
    assert!(!wordlist_path.exists());

    // 4. 证据补齐后放行（说明拦的是缺证据，不是把逆向来源整体锁死）。
    reviewed.crosscheck_proof = Some(adm4_template::CrossCheckProof {
        mapping_hash: "sha256:mapping".into(),
        crosscheck_hash: "sha256:crosscheck".into(),
        checked_count: 2,
        checked_at: "2026-08-31T00:00:00Z".into(),
    });
    library
        .certify(&mut reviewed, &wordlist_path)
        .expect("证据齐备应认证成功");
    assert!(reviewed.is_certified());
    let words = load_skin_wordlist(&wordlist_path).unwrap().words;
    assert!(words.contains(&"galaxy_guard".to_string()));

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// 负例 6（F4d）：认证证据旁路 —— 手工塞入的伪认证模板不得获得预填资格
// ---------------------------------------------------------------------------

/// `certify` 上的证据关卡管不到「不走 certify、直接把 JSON 落进 references/」这条路。
/// 迁移工具就是这么写的 25 份内置模板，手工伪造同理。所以取用侧必须自己再查一遍证据：
/// 有据可查的认证放行（批量迁移登记 / S2-S3 机器证据 / 人工审核署名），伪认证被拒。
#[test]
fn prefill_rejects_forged_certified_template_without_evidence() {
    let root = temp_root("forged_certified");
    let space = root.join("design_space");
    let library = TemplateLibrary::new(&space);

    // ① 手工伪造：状态写成 certified，什么证据都没有（连 origin 键都没有 → 按逆向解读）。
    let forged_json = r#"{
      "template_id": "tpl_forged",
      "game_name": "伪造甲",
      "genre_pack": "lane_test",
      "pack_version": "1.0.0",
      "depth_reached": "L4",
      "certification": {"status": "certified", "reviewed_by": "我自己", "reviewed_at": "2026-08-31T00:00:00Z", "review_note": "手改的"},
      "answers": [
        {"decision_id": "t.combat", "option_id": "counter_combat", "evidence": []}
      ]
    }"#;
    let references = space.join("lane_test").join("references");
    fs::create_dir_all(&references).unwrap();
    fs::write(references.join("tpl_forged.json"), forged_json).unwrap();

    let stored = library.get("lane_test", "tpl_forged").unwrap();
    assert!(
        stored.is_certified(),
        "状态位确实是 Certified（这正是漏洞形态）"
    );
    let error = library
        .approved_for_prefill("lane_test", "tpl_forged")
        .expect_err("无证据的伪认证模板不得取用");
    assert_eq!(error.kind, Adm4ErrorKind::RedLine);
    assert!(error.message.contains("机器证据"), "{}", error.message);

    // ② 批量迁移登记齐备且指纹对得上 → 照旧可取用（25 份内置模板走的就是这条）。
    let mut migrated = library.get("lane_test", "tpl_forged").unwrap();
    migrated.template_id = "tpl_migrated".into();
    migrated.origin = TemplateOrigin::BulkMigration {
        batch_id: "v2-builtin-2026-08-29".into(),
        tool_version: "v2_migration/1.1.0".into(),
        source_ref: "knowledge/design_data/project_templates/x.json".into(),
        answers_digest: migrated.answers_digest(),
        migrated_at: "2026-08-29T00:00:00Z".into(),
    };
    library.save_draft(&migrated).unwrap();
    assert_eq!(
        library
            .approved_for_prefill("lane_test", "tpl_migrated")
            .expect("登记可核对的批量迁移模板应可取用")
            .template_id,
        "tpl_migrated"
    );

    // ③ 篡改答卷（多加一条）而不更新登记 → 指纹失配，登记不再背书。
    let mut tampered = library.get("lane_test", "tpl_migrated").unwrap();
    tampered.template_id = "tpl_tampered".into();
    tampered.answers.push(adm4_template::TemplateAnswer {
        decision_id: "t.deploy".into(),
        option_id: "grid_deploy".into(),
        parameters: ParameterValues::default(),
        evidence: Vec::new(),
        notes: String::new(),
        crosscheck_agreed: None,
        additional_options: Vec::new(),
        primary_option: None,
    });
    library.save_draft(&tampered).unwrap();
    let error = library
        .approved_for_prefill("lane_test", "tpl_tampered")
        .expect_err("答卷被改而登记未更新 → 指纹失配");
    assert_eq!(error.kind, Adm4ErrorKind::RedLine);
    assert!(error.message.contains("指纹"), "{}", error.message);

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// 人工审核证明（R3）
// ---------------------------------------------------------------------------

#[test]
fn human_review_requires_reviewer_and_note() {
    let mut certification = Certification::default();
    certification
        .advance_to(CertificationStatus::Mapped)
        .unwrap();
    certification
        .advance_to(CertificationStatus::CrossChecked)
        .unwrap();
    assert!(certification.record_human_review("  ", "结论").is_err());
    assert!(certification.record_human_review("评审员丙", " ").is_err());
    assert_eq!(certification.status, CertificationStatus::CrossChecked);
    certification
        .record_human_review("评审员丙", "证据链完整")
        .unwrap();
    assert_eq!(certification.status, CertificationStatus::HumanReviewed);
}
