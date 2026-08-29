//! 逆向工具链产线全链测试（T3）：本地语料检索 → AI 映射 → 交叉核验 → 人工审核 →
//! 认证入库 → 换皮词表登记；负例覆盖跳级被拒（D8）、无证据答案被拒（R1）、
//! 非法 AI 输出即停（R7）。AI 一律用 ScriptedProvider，零网络。

use adm4_ai::ScriptedProvider;
use adm4_decision::{DecisionOption, DecisionPoint, DesignLevel, GenreScope, ParameterValues};
use adm4_foundation::Adm4ErrorKind;
use adm4_template::{
    CROSSCHECK_PURPOSE, Certification, CertificationStatus, Confidence, CrossCheckService,
    CrossCheckVerdict, EvidenceQuery, EvidenceSearchChannel, FileCorpusChannel, MAPPING_PURPOSE,
    MappingService, SourceType, Template, TemplateLibrary, load_skin_wordlist,
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
        requirement: Default::default(),
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
        mapping_hash: String::new(),
        crosscheck_proof: None,
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
