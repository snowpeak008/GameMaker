//! T-W7-5a 杀戮尖塔全量样板的 app 级验收（定稿 §6.2 / §7.1 波 5 / 指令 7）。
//!
//! 两条测试线（前任断点申报的 e2e 结构裁量）：
//! - **M2**：真实 `knowledge/design_space/spire_like/pack.json` + 三新模块的装配
//!   与 R-C1′ 判定——装配零悬空绑定；组合判定与定稿 §6.2 逐条一致
//!   （H={构筑,战斗,遗物}、(a) 连通、(b) 每节点 H 内边 ≥2、|H|=3 超中核参考线 2
//!   → advice + form_confirmation_required；B(G)=31.5 ≤ 中核预算 42 不触发预算提示）；
//! - **M3**：概念访谈全链样板——scripted 概念访谈（口述「爬塔卡牌肉鸽」→ 提案含
//!   三新模块 → 确认落盘）→ tier 声明 → 组合报告（|H|=3 超线 → 署名确认）→
//!   机制访谈逐点（PromptLibrary 弹药注入）→ 冻结（gate2 绿）→ C0-C6 →
//!   四伤疤断言：C4 含 DrawFromPool 能力契约、两个不同 priority 的 ModifyRule
//!   遗物 GWT 渲染出叠加序文字（指令 7 正式验收）、GameSpec.graphs 含 acyclic
//!   地图图、C6 含 ModifyRule 跨机制依赖边。
//!
//! 1c 纪律：全链只使用旧 7 变体 + Schedule/ModifyRule/DrawFromPool——
//! C0-C6 全绿本身就是「未触 AreaApply/Attach/Detach/RollCheck 四个诚实 Err 臂」的证明。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, InterviewTurnDto, save_config};
use adm4_archive::DataRoot;
use adm4_contracts::TypedValue;
use adm4_decision::FindingCode;
use adm4_decision::{DesignLevel, ParameterValues, Provenance};
use adm4_pipeline::StageStatus;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

/// 最小通用层：带 `u.target_scale`（组合判定的产品档数据源，选中核 → 参考线 2）
/// + L1/L2 各一点（空间校验硬要求三层齐备）。
const UNIVERSAL_CORE: &str = r#"{
  "space_version": "spiretest-1",
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
      "options": [ { "id": "mastery", "label": "技巧精进" }, { "id": "loot_fantasy", "label": "刷宝幻想" } ] },
    { "id": "u.genre", "domain": "core", "level": "L2", "genre_scope": "universal",
      "question": "品类？",
      "options": [ { "id": "roguelike_deckbuilder", "label": "爬塔卡牌肉鸽" }, { "id": "puzzle", "label": "解谜" } ] }
  ]
}"#;

/// M3 概念变体 pack（前任裁量：概念访谈路径用无 system_refs 的变体包，
/// 三实例由访谈确认落盘——与 3d 先例 PACK_EMPTY 同构；真实 spire_like pack
/// 的装配判定由 M2 测试独立锁定）。pack 层三决策点与真实 pack 同 id 同结构
/// （Graph 地图 / 意图预告 / 敌人名单），保证 C0 产物断言对得上定稿 §6.2 表达位。
const PACK_SPIRE_CONCEPT: &str = r#"{
  "pack_id": "spire_concept",
  "pack_version": "0.1.0",
  "display_name": "尖塔概念访谈变体包",
  "reference_games": ["虚构爬塔甲", "虚构爬塔乙", "虚构爬塔丙"],
  "core_nouns": ["player_command_intent"],
  "cardinality_expectations": {
    "map_nodes": { "min": 3, "max": 80 },
    "enemy_roster_rows": { "min": 2, "max": 60 },
    "asset_specs": { "min": 1, "max": 60 }
  },
  "decision_points": [
    {
      "id": "spire.map_graph",
      "domain": "content",
      "level": "L5",
      "genre_scope": { "pack": "spire_concept" },
      "question": "爬塔地图的节点路线结构是什么？",
      "options": [
        {
          "id": "acyclic_branching",
          "label": "有向无环分支塔",
          "summary": "单入口有向无环图：楼层向上分支/汇合，走过的节点不可回访（定稿 §6.2 尖塔口径）",
          "compiler_tags": { "spec_role": "data_table", "data_form": "graph" },
          "parameter_schema": { "schema": "graph", "node_payload": [
            { "key": "node_kind", "kind": { "kind": "enum", "variants": ["combat", "elite", "event", "shop", "rest", "boss"] }, "required": true }
          ], "edge_payload": [], "directed": true, "acyclic": true, "entry": "single", "cardinality_key": "map_nodes" }
        },
        {
          "id": "hub_revisit_web",
          "label": "枢纽可回访网",
          "summary": "中心枢纽放射式地图，节点可回访（可回环、多入口）",
          "compiler_tags": { "spec_role": "data_table", "data_form": "graph" },
          "parameter_schema": { "schema": "graph", "node_payload": [
            { "key": "node_kind", "kind": { "kind": "enum", "variants": ["combat", "elite", "event", "shop", "rest", "boss"] }, "required": true }
          ], "edge_payload": [], "directed": false, "acyclic": false, "entry": "multiple", "cardinality_key": "map_nodes" }
        }
      ]
    },
    {
      "id": "spire.intent_telegraph",
      "domain": "combat_information",
      "level": "L4",
      "genre_scope": { "pack": "spire_concept" },
      "question": "敌人意图按什么口径预告？",
      "options": [
        {
          "id": "full_disclosure",
          "label": "数值明牌预告",
          "summary": "敌人下回合行动与数值提前落表并完整展示——尖塔口径（披露时序为已知轻缺口，如实标注）",
          "compiler_tags": { "spec_role": "mechanic", "system": "combat_main.tier" },
          "parameter_schema": { "schema": "scalar", "fields": [
            { "key": "enemy_table_id", "kind": { "kind": "text" }, "required": true }
          ] },
          "effects_template": [
            { "effect": "modify_property", "entity": "{param:enemy_table_id}", "property": "intent_state", "formula": "next_action_with_values(action_id, computed_value) 提前落表" },
            { "effect": "emit_signal", "signal": "combat_main.turn_signal" }
          ]
        },
        {
          "id": "category_hint",
          "label": "类别模糊提示",
          "summary": "只预告行动类别图标，不给数值",
          "compiler_tags": { "spec_role": "mechanic", "system": "combat_main.tier" },
          "parameter_schema": { "schema": "scalar", "fields": [
            { "key": "enemy_table_id", "kind": { "kind": "text" }, "required": true }
          ] },
          "effects_template": [
            { "effect": "modify_property", "entity": "{param:enemy_table_id}", "property": "intent_state", "formula": "next_action_category_only(action_category) 提前落表" },
            { "effect": "emit_signal", "signal": "combat_main.turn_signal" }
          ]
        }
      ]
    },
    {
      "id": "spire.enemy_roster",
      "domain": "content",
      "level": "L5",
      "genre_scope": { "pack": "spire_concept" },
      "question": "敌人名单表包含哪些敌人？",
      "options": [
        {
          "id": "enemy_table",
          "label": "敌人名单表",
          "summary": "每种敌人一行：标识、生命、层级、行动模式脚本引用",
          "compiler_tags": { "spec_role": "entity_table", "visual_form": "sprite2d" },
          "parameter_schema": { "schema": "table", "columns": [
            { "key": "id", "kind": { "kind": "text" }, "required": true, "is_skin": true },
            { "key": "hp", "kind": { "kind": "int" }, "constraint": { "constraint": "range", "min": 1.0, "max": 9999.0 }, "required": true },
            { "key": "tier", "kind": { "kind": "enum", "variants": ["normal", "elite", "boss"] }, "required": true },
            { "key": "action_pattern", "kind": { "kind": "text" }, "required": true }
          ], "row_key": "id", "cardinality_key": "enemy_roster_rows" }
        },
        {
          "id": "enemy_table_with_scaling",
          "label": "带成长列敌人名单表",
          "summary": "在基础列之上增加楼层成长系数列",
          "compiler_tags": { "spec_role": "entity_table", "visual_form": "sprite2d" },
          "parameter_schema": { "schema": "table", "columns": [
            { "key": "id", "kind": { "kind": "text" }, "required": true, "is_skin": true },
            { "key": "hp", "kind": { "kind": "int" }, "constraint": { "constraint": "range", "min": 1.0, "max": 9999.0 }, "required": true },
            { "key": "tier", "kind": { "kind": "enum", "variants": ["normal", "elite", "boss"] }, "required": true },
            { "key": "action_pattern", "kind": { "kind": "text" }, "required": true },
            { "key": "floor_scaling", "kind": { "kind": "float" }, "constraint": { "constraint": "range", "min": 1.0, "max": 5.0 }, "required": true }
          ], "row_key": "id", "cardinality_key": "enemy_roster_rows" }
        }
      ]
    }
  ]
}"#;

/// e2e 专用弹药库（前任裁量：仓库 seed.json 的三新模块条目留给波 4 量产，
/// 本测试自造含 sys.run_deckbuild domain 的弹药副本，锁「弹药注入」断言）。
const PROMPT_AMMO: &str = r#"{
  "entries": [
    {
      "id": "pl.run_deckbuild.e2e",
      "domain": "sys.run_deckbuild",
      "question_zh": "抽牌节奏与手牌上限的张力取舍拉多紧",
      "follow_ups": ["0 费卡与连抽的组合上限要不要封顶？", "洗牌时点公开吗——数牌是技巧还是负担？"],
      "source_ref": "v2:e2e_fixture"
    }
  ]
}"#;

const FREEZE_RED_TEAM_ANSWER: &str = r#"{"findings":[],"per_category":[{"category":"consistency","checked":"全部决策交叉复核","conclusion":"未发现矛盾"}]}"#;
const C1_RED_TEAM_ANSWER: &str = r#"{"findings":[{"id":"w1","severity":"warning","target":"mechanics/relic_main.multiplicative_modifier_rule","text":"乘算件叠乘上界需在投放前算清"}],"per_category":[{"category":"feasibility","checked":"14 条机制逐条","conclusion":"均可实现"}]}"#;

fn repo_knowledge_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

const SPIRE_MODULES: [&str; 3] = [
    "sys.turn_combat",
    "sys.run_deckbuild",
    "sys.rule_modifier_collect",
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

/// 隔离环境：`knowledge/` 布局副本（systems + calibration/budget.json +
/// prompt_library）+ 设计空间（合成通用层 + 指定 pack 集）。
/// 预算表用真文件副本——入库数值改动必须让本测试跟着表态（与 budget_advice_e2e 同纪律）。
fn setup(tag: &str, packs: &[(&str, String)]) -> (PathBuf, AppServices) {
    let temp = std::env::temp_dir().join(format!("adm4_spire_e2e_{tag}_{}", std::process::id()));
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
    for module_id in SPIRE_MODULES {
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
        PROMPT_AMMO,
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
// M2：真实 spire_like pack 装配 + R-C1′ 判定与定稿 §6.2 逐条一致
// ---------------------------------------------------------------------------

#[test]
fn spire_like_pack_assembles_and_composition_matches_finalized_ruling() {
    // 真实 pack.json 原文（不改字节复制），装配用与产线同一条加载链路。
    let real_pack = std::fs::read_to_string(
        repo_knowledge_root()
            .join("design_space")
            .join("spire_like")
            .join("pack.json"),
    )
    .unwrap();
    let (temp, services) = setup("assembly", &[("spire_like", real_pack)]);

    // ---- 装配成功零悬空绑定（fail-closed：任何 V6/版本/门控矛盾都会 Err）----
    let space = services.load_space("spire_like").unwrap();
    assert_eq!(space.system_instances.len(), 3, "三实例全部装配");
    for (instance, module) in [
        ("combat_main", "sys.turn_combat"),
        ("deck_main", "sys.run_deckbuild"),
        ("relic_main", "sys.rule_modifier_collect"),
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
    for (tier_point, options) in [
        ("combat_main.tier", 4),
        ("deck_main.tier", 3),
        ("relic_main.tier", 3),
    ] {
        let point = space
            .graph
            .point(tier_point)
            .unwrap_or_else(|| panic!("缺 tier 合成点 {tier_point}"));
        assert_eq!(point.options.len(), options, "{tier_point} 档位数不符");
    }
    // 命名空间重写后的模块点与 pack 层三决策点都在图上。
    for id in [
        "combat_main.damage_formula",
        "combat_main.status_effect_rule",
        "deck_main.draw_discard_cycle",
        "relic_main.additive_modifier_rule",
        "spire.map_graph",
        "spire.intent_telegraph",
        "spire.enemy_roster",
    ] {
        assert!(space.graph.point(id).is_some(), "装配后缺决策点 {id}");
    }

    // ---- R-C1′ 判定（定稿 §6.2）：中核档 + 尖塔三档位声明 ----
    let archive_id = services
        .project_new("尖塔装配判定", "spire_like", DesignLevel::L6, None)
        .unwrap();
    select_confirmed(&services, &archive_id, "u.target_scale", "midcore");
    select_confirmed(
        &services,
        &archive_id,
        "combat_main.tier",
        "tc2_status_stack",
    );
    select_confirmed(
        &services,
        &archive_id,
        "deck_main.tier",
        "db2_full_deckbuild",
    );
    select_confirmed(&services, &archive_id, "relic_main.tier", "rm1_rule_patch");

    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产组合报告");
    let report = &assessment.report;
    assert!(assessment.missing_tiers.is_empty(), "三档位已全部声明");
    // §6.2：H = {构筑, 战斗, 遗物} 三实例（W14/W10/W10，κ core/core/strong），按 id 字典序。
    assert_eq!(report.h_set, vec!["combat_main", "deck_main", "relic_main"]);
    // (a) 连通：构筑↔战斗双边（action_point/turn_signal）+ 遗物 modifies 战斗与构筑。
    assert!(report.h_connected, "R-C1′(a) 连通应通过");
    // (b) 每节点 H 内边 ≥2：blocks 为空即证明——V3b 有任何一条都会进 blocks；
    // V1 传导（db 档要求 turn_signal 有源 / rm 档要求 combat_rule_slot 有源）同样零违例。
    assert!(
        report.blocks.is_empty(),
        "定稿 §6.2：尖塔组合无硬违例，实际：{:?}",
        report.blocks
    );
    // (c) |H|=3 > 中核参考线 2 → 恰好一条数量提示（三角形无割点，无双连通提示；
    // B(G) 未超中核预算 42，无预算提示——5-0 预算表入库后的联动断言）。
    let advice_codes: Vec<FindingCode> = report.advices.iter().map(|f| f.code).collect();
    assert_eq!(
        advice_codes,
        vec![FindingCode::V3cCountAdvice],
        "应只有 |H| 数量提示，实际：{:?}",
        report.advices
    );
    assert!(
        report.form_confirmation_required,
        "|H|=3 超线且未署名 → 需要一次性形态确认"
    );
    // B(G) = 14(deck, core) + 10(combat, core) + 10×0.75(relic, strong) = 31.5 ≤ 42。
    // （标定锚 37.75 含 pack 外的地图/局外解锁两件；本 pack 组合是三件套，31.5。）
    assert!(
        (report.budget_total - 31.5).abs() < 1e-9,
        "B(G) 应为 31.5，实际 {}（若 >42 需复核 pack 的 κ 声明）",
        report.budget_total
    );
    assert!(
        !advice_codes.contains(&FindingCode::V5BudgetAdvice),
        "B(G)=31.5 ≤ mid_core=42 不得触发预算提示"
    );

    // ---- 署名形态确认流：确认后不再要求，数量提示保持（提示义务不消失）----
    let record = services
        .compose_confirm_form(&archive_id, "样板设计师", "接受尖塔三重核形态（定稿 §6.2）")
        .unwrap();
    assert_eq!(record.h_set, vec!["combat_main", "deck_main", "relic_main"]);
    let confirmed = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert!(!confirmed.report.form_confirmation_required);
    assert!(
        confirmed
            .report
            .advices
            .iter()
            .any(|finding| finding.code == FindingCode::V3cCountAdvice
                && finding.detail.contains("已署名确认")),
        "确认后数量提示保持并标注已署名：{:?}",
        confirmed.report.advices
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// M3：概念访谈全链样板（口述 → 三模块提案落盘 → 组合确认 → 机制访谈 → 冻结 → C0-C6）
// ---------------------------------------------------------------------------

/// 概念提案：三新模块各一实例（档位=定稿 §6.2 尖塔档），绑定与真实 pack 同口径。
fn spire_concept_json() -> String {
    r#"{
      "systems": [
        { "instance_id": "combat_main", "module_id": "sys.turn_combat", "suggested_tier": "tc2_status_stack",
          "core_link": "core", "rationale": "回合战斗是爬塔对局的结算本体（W10 重）",
          "noun_bindings": { "sys.player_input.command_intent": "player_command_intent" } },
        { "instance_id": "deck_main", "module_id": "sys.run_deckbuild", "suggested_tier": "db2_full_deckbuild",
          "core_link": "core", "rationale": "全构筑循环是运行主循环（W14 极重）",
          "noun_bindings": {
            "sys.turn_combat.action_point": "combat_main.action_point",
            "sys.turn_combat.turn_signal": "combat_main.turn_signal"
          } },
        { "instance_id": "relic_main", "module_id": "sys.rule_modifier_collect", "suggested_tier": "rm1_rule_patch",
          "core_link": "strong", "rationale": "遗物按叠加序常驻改写战斗与构筑规则（W10 重）",
          "noun_bindings": {
            "sys.turn_combat.turn_signal": "combat_main.turn_signal",
            "sys.turn_combat.combat_rule_slot": "combat_main.combat_rule_slot",
            "sys.run_deckbuild.deck_rule_slot": "deck_main.deck_rule_slot"
          } }
      ],
      "core_loop": [
        { "verb": "打出卡牌", "instance_id": "deck_main" },
        { "verb": "结算战斗", "instance_id": "combat_main" }
      ],
      "notes": "爬塔卡牌肉鸽：构筑×战斗×遗物三重核"
    }"#
    .to_string()
}

fn enemy_row(id: &str, hp: i64, tier: &str, pattern: &str) -> BTreeMap<String, TypedValue> {
    [
        ("id".to_string(), text(id)),
        ("hp".to_string(), TypedValue::Int(hp)),
        ("tier".to_string(), text(tier)),
        ("action_pattern".to_string(), text(pattern)),
    ]
    .into_iter()
    .collect()
}

fn card_row(
    id: &str,
    label: &str,
    cost: i64,
    rarity: &str,
    kind: &str,
) -> BTreeMap<String, TypedValue> {
    [
        ("card_id".to_string(), text(id)),
        ("label".to_string(), text(label)),
        ("cost".to_string(), TypedValue::Int(cost)),
        ("rarity".to_string(), text(rarity)),
        ("card_type".to_string(), text(kind)),
    ]
    .into_iter()
    .collect()
}

fn relic_row(id: &str, label: &str, rarity: &str, class: &str) -> BTreeMap<String, TypedValue> {
    [
        ("relic_id".to_string(), text(id)),
        ("label".to_string(), text(label)),
        ("rarity".to_string(), text(rarity)),
        ("effect_class".to_string(), text(class)),
    ]
    .into_iter()
    .collect()
}

const MAP_GRAPH_VALUE: &str = r#"{"nodes":[{"id":"floor_start","payload":{"node_kind":"combat"}},{"id":"floor_elite","payload":{"node_kind":"elite"}},{"id":"floor_boss","payload":{"node_kind":"boss"}}],"edges":[{"from":"floor_start","to":"floor_elite"},{"from":"floor_elite","to":"floor_boss"}]}"#;

#[test]
fn spire_full_chain_from_concept_interview_to_phase1_artifacts() {
    let (temp, services) = setup(
        "chain",
        &[("spire_concept", PACK_SPIRE_CONCEPT.to_string())],
    );
    let archive_id = services
        .project_new("爬塔卡牌肉鸽样板", "spire_concept", DesignLevel::L6, None)
        .unwrap();

    // L0 画像先落（组合判定的产品档数据源：中核 → 参考线 2）。
    select_confirmed(&services, &archive_id, "u.target_scale", "midcore");

    // ---- ① 概念访谈：口述「爬塔卡牌肉鸽」→ 提案含三新模块 → 确认落盘 ----
    let provider = ScriptedProvider::new();
    provider.script("interview_concept", vec![spire_concept_json()]);
    let proposal = services
        .interview_concept_with(
            &archive_id,
            &provider,
            "爬塔卡牌肉鸽：打牌打怪捡遗物，一路爬塔到 Boss",
        )
        .unwrap();
    assert_eq!(proposal.systems.len(), 3, "提案应含三新模块实例");
    assert_eq!(
        proposal.heavy_core_candidates,
        vec!["combat_main", "deck_main", "relic_main"],
        "三档位建议全部入重核候选（W10/W14/W10 × core/core/strong）"
    );
    assert!(!proposal.per_heavy_core_mode, "3 个候选 ≤4 不切逐重核模式");
    let report = services
        .interview_concept_confirm(&archive_id, &proposal)
        .unwrap();
    assert_eq!(report.instances.len(), 3);
    assert_eq!(report.core_loop_len, 2);

    // ---- ② tier 声明落盘核验（概念确认即声明，rationale 在案）----
    let state = services.load_authoring_state(&archive_id).unwrap();
    for (point, tier) in [
        ("combat_main.tier", "tc2_status_stack"),
        ("deck_main.tier", "db2_full_deckbuild"),
        ("relic_main.tier", "rm1_rule_patch"),
    ] {
        let selection = &state.selections[point];
        assert_eq!(selection.option_id, tier);
        assert!(selection.confirmed_by_user);
        assert!(!selection.rationale.is_empty());
    }

    // ---- ③ 组合报告：|H|=3 超中核参考线 2 → 提示 + 署名确认 ----
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("确认落盘后应有组合报告");
    assert_eq!(
        assessment.report.h_set,
        vec!["combat_main", "deck_main", "relic_main"]
    );
    assert!(assessment.report.h_connected);
    assert!(
        assessment.report.blocks.is_empty(),
        "实际：{:?}",
        assessment.report.blocks
    );
    assert!(assessment.report.form_confirmation_required);
    services
        .compose_confirm_form(&archive_id, "样板设计师", "接受三重核形态")
        .unwrap();

    // ---- ④ 手动补齐除 deck_main.draft_pick_rule 之外的全部激活点
    //（占位符参数全部由作者填写，I1）——留恰好一个待办点给机制访谈，
    // 使访谈选点确定（选点纪律是层升序+拓扑序，留单点即无歧义）。 ----
    services
        .with_project(&archive_id, |engine| {
            let manual = Provenance::UserManual;
            for (decision, option) in [
                ("u.promise", "mastery"),
                ("u.genre", "roguelike_deckbuilder"),
                ("combat_main.turn_structure", "player_enemy_alternate"),
                ("combat_main.action_economy", "per_turn_refill"),
                ("combat_main.damage_formula", "flat_minus_block"),
                ("combat_main.status_effect_rule", "stack_intensity"),
                ("combat_main.status_timing", "turn_start_settle"),
                ("deck_main.card_pool", "card_pool_table"),
                ("deck_main.draw_discard_cycle", "full_refresh_cycle"),
                ("deck_main.energy_cost_rule", "fixed_cost_per_card"),
                ("deck_main.card_removal", "paid_removal_service"),
                ("deck_main.upgrade_rule", "single_step_upgrade"),
                ("relic_main.acquisition_channel", "milestone_drop"),
                ("relic_main.relic_pool", "relic_pool_table"),
                ("relic_main.additive_modifier_rule", "flat_additive_patch"),
                (
                    "relic_main.multiplicative_modifier_rule",
                    "global_multiplier_patch",
                ),
                ("spire.map_graph", "acyclic_branching"),
                ("spire.intent_telegraph", "full_disclosure"),
                ("spire.enemy_roster", "enemy_table"),
            ] {
                engine.select_option(decision, option, manual.clone())?;
            }

            let scalar_params: [(&str, ParameterValues); 12] = [
                (
                    "combat_main.action_economy",
                    scalars(&[("points_per_turn", TypedValue::Int(3))]),
                ),
                (
                    "combat_main.damage_formula",
                    scalars(&[("unit_table_id", text("spire.enemy_roster"))]),
                ),
                (
                    "combat_main.status_effect_rule",
                    scalars(&[
                        ("unit_table_id", text("spire.enemy_roster")),
                        ("decay_per_turn", TypedValue::Int(1)),
                    ]),
                ),
                (
                    "combat_main.status_timing",
                    scalars(&[("unit_table_id", text("spire.enemy_roster"))]),
                ),
                (
                    "deck_main.draw_discard_cycle",
                    scalars(&[
                        ("pool_table_id", text("deck_main.card_pool")),
                        ("hand_size", TypedValue::Int(5)),
                    ]),
                ),
                (
                    "deck_main.energy_cost_rule",
                    scalars(&[("card_table_id", text("deck_main.card_pool"))]),
                ),
                (
                    "deck_main.card_removal",
                    scalars(&[
                        ("card_table_id", text("deck_main.card_pool")),
                        ("base_removal_cost", TypedValue::Int(75)),
                        ("cost_escalation", TypedValue::Int(25)),
                    ]),
                ),
                (
                    "deck_main.upgrade_rule",
                    scalars(&[("card_table_id", text("deck_main.card_pool"))]),
                ),
                (
                    "relic_main.acquisition_channel",
                    scalars(&[("pool_table_id", text("relic_main.relic_pool"))]),
                ),
                // 叠加序验收对（指令 7）：同靶 combat_main.damage_formula，
                // 加法件 priority=10 先结算、乘算件 priority=100 后结算。
                (
                    "relic_main.additive_modifier_rule",
                    scalars(&[
                        ("target_rule_id", text("combat_main.damage_formula")),
                        ("flat_bonus", TypedValue::Float(6.0)),
                    ]),
                ),
                (
                    "relic_main.multiplicative_modifier_rule",
                    scalars(&[
                        ("target_rule_id", text("combat_main.damage_formula")),
                        ("multiplier", TypedValue::Float(1.5)),
                    ]),
                ),
                (
                    "spire.intent_telegraph",
                    scalars(&[("enemy_table_id", text("spire.enemy_roster"))]),
                ),
            ];
            for (decision, parameters) in scalar_params {
                let problems = engine.set_parameters(decision, parameters)?;
                assert!(
                    problems.is_empty(),
                    "{decision} 参数应通过校验：{problems:?}"
                );
            }

            let problems = engine.set_parameters(
                "deck_main.card_pool",
                ParameterValues::Rows {
                    rows: vec![
                        card_row("strike", "打击", 1, "common", "attack"),
                        card_row("defend", "防御", 1, "common", "skill"),
                        card_row("heavy_blade", "重刃", 2, "common", "attack"),
                        card_row("inflame", "燃心", 1, "uncommon", "power"),
                        card_row("bludgeon", "钝击", 3, "rare", "attack"),
                        card_row("impervious", "固若金汤", 2, "rare", "skill"),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "卡池表应过校验：{problems:?}");
            let problems = engine.set_parameters(
                "relic_main.relic_pool",
                ParameterValues::Rows {
                    rows: vec![
                        relic_row("iron_talisman", "铁符", "common", "additive"),
                        relic_row("prism_lens", "棱镜", "boss", "multiplicative"),
                        relic_row("echo_bell", "回响铃", "rare", "trigger"),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "遗物池表应过校验：{problems:?}");
            let problems = engine.set_parameters(
                "spire.enemy_roster",
                ParameterValues::Rows {
                    rows: vec![
                        enemy_row("acolyte", 48, "normal", "ritual_then_attack"),
                        enemy_row("pit_bruiser", 82, "elite", "enrage_on_skill"),
                        enemy_row("tower_warden", 240, "boss", "mode_shift_cycle"),
                    ],
                },
            )?;
            assert!(problems.is_empty(), "敌人名单表应过校验：{problems:?}");
            let problems = engine.set_parameters(
                "spire.map_graph",
                scalars(&[("graph", text(MAP_GRAPH_VALUE))]),
            )?;
            assert!(problems.is_empty(), "地图图值应过校验：{problems:?}");

            for decision in [
                "u.promise",
                "u.genre",
                "combat_main.turn_structure",
                "combat_main.action_economy",
                "combat_main.damage_formula",
                "combat_main.status_effect_rule",
                "combat_main.status_timing",
                "deck_main.card_pool",
                "deck_main.draw_discard_cycle",
                "deck_main.energy_cost_rule",
                "deck_main.card_removal",
                "deck_main.upgrade_rule",
                "relic_main.acquisition_channel",
                "relic_main.relic_pool",
                "relic_main.additive_modifier_rule",
                "relic_main.multiplicative_modifier_rule",
                "spire.map_graph",
                "spire.intent_telegraph",
                "spire.enemy_roster",
            ] {
                engine.confirm_selection(decision)?;
            }
            // 唯一待办 = deck_main.draft_pick_rule（留给机制访谈）。
            let pending = engine.pending_decisions()?;
            assert_eq!(
                pending,
                vec!["deck_main.draft_pick_rule".to_string()],
                "机制访谈前应恰好剩 draft 规则一个待办"
            );
            Ok(())
        })
        .unwrap();

    // ---- ⑤ 机制访谈逐点：PromptLibrary 弹药注入 + 逐点提案确认 ----
    let provider = ScriptedProvider::new();
    provider.script(
        "interview_mechanism",
        vec![
            r#"{"option_id":"fixed_choice_count","rationale":"三选一制造没得选的取舍——直接回应抽牌节奏张力的追问。","parameters":{"pool_table_id":"deck_main.card_pool","choice_count":3}}"#
                .into(),
        ],
    );
    let turn = services
        .interview_mechanism_next_with(&archive_id, &provider, "deck_main")
        .unwrap();
    let InterviewTurnDto::StructuralPoint {
        proposal: point_proposal,
    } = &turn
    else {
        panic!("deck_main 应有待确认激活点，得到 {turn:?}");
    };
    assert_eq!(
        point_proposal.decision_id, "deck_main.draft_pick_rule",
        "机制访谈范围限定实例命名空间且应选中唯一待办"
    );
    // 弹药注入断言：自造弹药库的 sys.run_deckbuild 域问句进了 AI 上下文。
    let calls = provider.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].user_prompt.contains("追问弹药"), "弹药段应存在");
    assert!(
        calls[0]
            .user_prompt
            .contains("抽牌节奏与手牌上限的张力取舍拉多紧"),
        "sys.run_deckbuild 域弹药问句应注入：{}",
        calls[0].user_prompt
    );
    services
        .interview_confirm(&archive_id, point_proposal, None)
        .unwrap();
    services
        .with_project(&archive_id, |engine| {
            let completeness = engine.completeness();
            assert!(
                completeness.is_complete(),
                "blocking: {:?}",
                completeness.blocking
            );
            Ok(())
        })
        .unwrap();

    // ---- ⑤ 冻结：红队 → 五门全绿（gate2 组合段绿 + 署名确认留痕可见）→ 冻结 ----
    let ai = ScriptedProvider::new();
    ai.script("freeze_red_team", vec![FREEZE_RED_TEAM_ANSWER.into()]);
    ai.script("c1_redteam", vec![C1_RED_TEAM_ANSWER.into()]);
    ai.script(
        "c2_narrative",
        vec![r#"{"text":"基于规格的玩法叙述：玩家按能量打出卡牌驱动回合战斗，敌人意图明牌预告，遗物按叠加序常驻改写伤害规则，一路沿有向无环分支塔爬向 Boss。"}"#.into()],
    );
    ai.script(
        "c3_asset_description",
        vec![r#"{"description":"暗色调哥特卡通风格的敌人立绘，正面站姿，边缘描边，适配 2D 序列帧。"}"#.into()],
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
    assert!(
        gate2
            .findings
            .iter()
            .any(|finding| finding.code == "composition_form_confirmed"),
        "署名形态确认留痕应在门报告可见（R3）：{:?}",
        gate2.findings
    );
    let frozen = services.freeze_run(&archive_id).unwrap();
    assert_eq!(frozen.version, 1);
    for module_id in SPIRE_MODULES {
        assert_eq!(
            frozen.module_versions.get(module_id).map(String::as_str),
            Some("1.0.0"),
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

    // ---- ⑦ 四伤疤断言 ----
    let content = services.archives.content_dir(&archive_id);
    let read_contract = |stage: &str| -> serde_json::Value {
        let raw =
            std::fs::read_to_string(content.join(format!("pipeline/v1/{stage}/contract.json")))
                .unwrap_or_else(|e| panic!("{stage} 契约应可读：{e}"));
        serde_json::from_str(&raw).unwrap()
    };

    // 伤疤 3（Graph）：GameSpec.graphs 含 acyclic 单入口有向地图图（定稿 §6.2 #26）。
    let spec = read_contract("C0");
    let graphs = spec["graphs"].as_array().unwrap();
    assert_eq!(graphs.len(), 1, "GameSpec.graphs 应恰有地图一图");
    assert_eq!(graphs[0]["id"], "spire.map_graph");
    assert_eq!(graphs[0]["directed"], true);
    assert_eq!(
        graphs[0]["acyclic"], true,
        "acyclic:true 显式声明（可校验结构约束）"
    );
    assert_eq!(graphs[0]["entry"], "single");
    assert_eq!(graphs[0]["nodes"].as_array().unwrap().len(), 3);

    // 伤疤 1（DrawFromPool）+ 伤疤 2（叠加序，指令 7 正式验收）。
    let c4 = read_contract("C4");
    let capabilities = c4["capabilities"].as_array().unwrap();
    let scenario_then = |cap_id: &str| -> String {
        let capability = capabilities
            .iter()
            .find(|capability| capability["id"] == cap_id)
            .unwrap_or_else(|| panic!("C4 缺能力契约 {cap_id}"));
        capability["scenarios"][0]["then"]
            .as_array()
            .unwrap()
            .iter()
            .map(|then| then.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
            .join("；")
    };
    // DrawFromPool：draft 三选一的抽取契约，池表/规则/数量/目的地全来自作者填写。
    let draft_then = scenario_then("cap_deck_main.draft_pick_rule");
    assert!(
        draft_then.contains(
            "从池表 deck_main.card_pool 按规则 weighted_by_rarity_no_duplicate 抽取 3 个到 draft_offer"
        ),
        "DrawFromPool 能力契约应含完整抽取语义：{draft_then}"
    );
    // 叠加序：两个不同 priority 的 ModifyRule 遗物，GWT 渲染出叠加序文字——
    // 加法件（priority=10）先结算、乘算件（priority=100）后结算，同靶 damage_formula。
    let additive_then = scenario_then("cap_relic_main.additive_modifier_rule");
    assert!(
        additive_then.contains("规则 combat_main.damage_formula 的系数按 base + 6 缩放"),
        "加法遗物 patch 渲染：{additive_then}"
    );
    assert!(
        additive_then.contains("（按 priority=10 结算，同序按机制 id 字典序）"),
        "加法遗物叠加序文字（指令 7）：{additive_then}"
    );
    let multiplicative_then = scenario_then("cap_relic_main.multiplicative_modifier_rule");
    assert!(
        multiplicative_then.contains("规则 combat_main.damage_formula 的系数按 result * 1.5 缩放"),
        "乘算遗物 patch 渲染：{multiplicative_then}"
    );
    assert!(
        multiplicative_then.contains("（按 priority=100 结算，同序按机制 id 字典序）"),
        "乘算遗物叠加序文字（指令 7）：{multiplicative_then}"
    );

    // 伤疤 4（R-C1′）在 M2 测试与本链 ③ 段已锁；此处补 C6 跨机制依赖边：
    // 两件遗物的程序任务都依赖 target_rule 所属机制（战斗伤害公式）的程序任务。
    let c6 = read_contract("C6");
    let tasks = c6["tasks"].as_array().unwrap();
    for relic_task_id in [
        "task_cap_relic_main.additive_modifier_rule",
        "task_cap_relic_main.multiplicative_modifier_rule",
    ] {
        let task = tasks
            .iter()
            .find(|task| task["id"] == relic_task_id)
            .unwrap_or_else(|| panic!("C6 缺程序任务 {relic_task_id}"));
        let depends: Vec<&str> = task["depends_on"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dep| dep.as_str().unwrap())
            .collect();
        assert!(
            depends.contains(&"task_cap_combat_main.damage_formula"),
            "{relic_task_id} 应依赖伤害公式机制的程序任务（ModifyRule 跨机制边）：{depends:?}"
        );
    }
    // 被引用方不产反向边。
    let base = tasks
        .iter()
        .find(|task| task["id"] == "task_cap_combat_main.damage_formula")
        .expect("伤害公式程序任务应在");
    assert!(
        base["depends_on"].as_array().unwrap().is_empty(),
        "被 ModifyRule 引用的机制不产反向依赖"
    );

    std::fs::remove_dir_all(&temp).ok();
}
