//! W7 3d 三段访谈（概念/组合/机制）的 app 级端到端验收。
//!
//! 覆盖（对应任务卡验收 2/3/4/5/6）：
//! - **计划 §4 硬断言**：scripted 脚本模拟 5 重核提案 → 断言访谈切逐重核模式、
//!   逐系统档位声明 + rationale 落盘（Selection.rationale 与 transcript 留痕）；
//! - **三段全链**：概念（口述 → 系统清单 + 档位 + core_loop 确认落盘）→
//!   组合（真实 V2 违例 → AI 解释 + 修复选项 → 选升档 → 违例消除）→
//!   机制（弹药注入 prompt 可断言 + 逐点提案确认）；
//! - **融合型嗅探**："X+Y" 口述 → 双核并集分解提案结构（CLI 级断言在冒烟脚本）；
//! - **AI 越界负测试**：发明模块 id / 档位 id → Err 不吞；
//! - **core_loop 回填**：确认落盘后 CompositionInput.core_loop_verbs 非空
//!   （通过组合评估的 h_set/κ 行为间接验证 + 状态直查）。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, InterviewTurnDto, save_config};
use adm4_archive::DataRoot;
use adm4_decision::DesignLevel;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 夹具：最小通用层 + 真实四模块库副本 + 空 pack（系统实例全部由概念访谈落盘）
// ---------------------------------------------------------------------------

const UNIVERSAL_CORE: &str = r#"{
  "space_version": "interviewtest-1",
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

/// 空 pack：无 system_refs——概念访谈确认落盘 content/system_refs.json 后
/// 实例才进装配（这正是 3d 的落盘通道验收）。
const PACK_EMPTY: &str = r#"{
  "pack_id": "concept_pack",
  "pack_version": "0.1.0",
  "display_name": "概念访谈测试包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "core_nouns": ["combat_attribute", "money_supply"],
  "decision_points": []
}"#;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn copy_module(temp_systems: &Path, module_id: &str) {
    let target = temp_systems.join(module_id);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(
        repo_root()
            .join("knowledge")
            .join("systems")
            .join(module_id)
            .join("module.json"),
        target.join("module.json"),
    )
    .unwrap();
}

/// 夹具目录：设计空间 + 真实四模块库 + 真实弹药库副本。
fn setup(tag: &str) -> (PathBuf, AppServices) {
    let temp =
        std::env::temp_dir().join(format!("adm4_interview3_e2e_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let space_root = temp.join("design_space");
    std::fs::create_dir_all(space_root.join("universal")).unwrap();
    std::fs::write(
        space_root.join("universal").join("core.json"),
        UNIVERSAL_CORE,
    )
    .unwrap();
    std::fs::create_dir_all(space_root.join("concept_pack")).unwrap();
    std::fs::write(
        space_root.join("concept_pack").join("pack.json"),
        PACK_EMPTY,
    )
    .unwrap();
    // knowledge/ 布局副本：systems/ 与兄弟目录 prompt_library/（弹药路径口径）。
    let knowledge = temp.join("knowledge");
    let systems_root = knowledge.join("systems");
    std::fs::create_dir_all(&systems_root).unwrap();
    for module_id in ["sys.equipment", "sys.inventory", "sys.loot", "sys.economy"] {
        copy_module(&systems_root, module_id);
    }
    std::fs::create_dir_all(knowledge.join("prompt_library")).unwrap();
    std::fs::copy(
        repo_root()
            .join("knowledge")
            .join("prompt_library")
            .join("seed.json"),
        knowledge.join("prompt_library").join("seed.json"),
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

fn new_project(services: &AppServices, name: &str) -> String {
    services
        .project_new(name, "concept_pack", DesignLevel::L6, None)
        .unwrap()
}

/// 四件套概念提案（equip/loot/bag/econ 各一实例，绑定显式给全——四模块的
/// consumes 交叉引用较多，显式绑定比推导更贴近真实 AI 输出）。
fn four_system_concept_json() -> String {
    r#"{
      "systems": [
        { "instance_id": "equip_main", "module_id": "sys.equipment", "suggested_tier": "e3_socket",
          "core_link": "strong", "rationale": "刷宝主循环的装备承诺",
          "noun_bindings": {
            "sys.loot.drop_table": "loot_main.drop_table",
            "sys.loot.gem_entity": "loot_main.gem_entity",
            "sys.loot.material_entity": "loot_main.material_entity",
            "sys.economy.currency_main": "econ_main.currency_main",
            "combat_attribute": "combat_attribute"
          } },
        { "instance_id": "loot_main", "module_id": "sys.loot", "suggested_tier": "quality_affix_weights",
          "core_link": "strong", "rationale": "掉落是装备的供给源", "noun_bindings": {} },
        { "instance_id": "bag_main", "module_id": "sys.inventory", "suggested_tier": "basic_capacity",
          "core_link": "weak", "rationale": "先做最小背包",
          "noun_bindings": {
            "sys.equipment.equipment_entity": "equip_main.equipment_entity",
            "sys.loot.material_entity": "loot_main.material_entity",
            "sys.loot.gem_entity": "loot_main.gem_entity",
            "storage_capacity": "bag_main.storage_capacity"
          } },
        { "instance_id": "econ_main", "module_id": "sys.economy", "suggested_tier": "basic_income",
          "core_link": "weak", "rationale": "货币兜底",
          "noun_bindings": {
            "sys.loot.material_entity": "loot_main.material_entity",
            "money_supply": "money_supply"
          } }
      ],
      "library_external": [ { "name": "天气系统", "note": "模块库暂无对应，后续走系统级 custom" } ],
      "core_loop": [
        { "verb": "击杀拾取", "instance_id": "loot_main" },
        { "verb": "穿戴强化", "instance_id": "equip_main" }
      ],
      "notes": "刷宝闭环"
    }"#
    .to_string()
}

// ---------------------------------------------------------------------------
// 三段全链（验收 3 + 6）：概念确认落盘 → 组合修复 → 机制逐点 + 弹药断言
// ---------------------------------------------------------------------------

#[test]
fn full_three_stage_interview_chain() {
    let (temp, services) = setup("chain");
    let archive_id = new_project(&services, "三段访谈项目");

    // ---- 概念访谈：提案 → 确认落盘 ----
    let provider = ScriptedProvider::new();
    provider.script("interview_concept", vec![four_system_concept_json()]);
    let proposal = services
        .interview_concept_with(&archive_id, &provider, "刷宝 ARPG：打怪掉装备镶宝石")
        .unwrap();
    assert_eq!(proposal.systems.len(), 4);
    assert_eq!(proposal.library_external.len(), 1, "库外系统如实标注");
    assert!(!proposal.per_heavy_core_mode, "2 个重核候选不切模式");
    // e3_socket W12 + strong、quality_affix_weights W9 + strong → 候选 2 个。
    assert_eq!(
        proposal.heavy_core_candidates,
        vec!["equip_main", "loot_main"]
    );

    let report = services
        .interview_concept_confirm(&archive_id, &proposal)
        .unwrap();
    assert_eq!(report.instances.len(), 4);
    assert_eq!(report.core_loop_len, 2);

    // 落盘验证 1：system_refs 等价物（项目私有引用）进装配——tier 合成点已确认。
    let state = services.load_authoring_state(&archive_id).unwrap();
    for (point, tier) in [
        ("equip_main.tier", "e3_socket"),
        ("loot_main.tier", "quality_affix_weights"),
        ("bag_main.tier", "basic_capacity"),
        ("econ_main.tier", "basic_income"),
    ] {
        let selection = state
            .selections
            .get(point)
            .unwrap_or_else(|| panic!("概念确认后 {point} 应有选择"));
        assert_eq!(selection.option_id, tier);
        assert!(selection.confirmed_by_user, "{point} 应已确认");
        assert!(!selection.rationale.is_empty(), "{point} 应带 rationale");
    }
    // 落盘验证 2：core_loop 进创作状态（3b 遗留 ① 回填的数据源）。
    assert_eq!(state.core_loop.len(), 2);
    assert_eq!(state.core_loop[0].verb, "击杀拾取");
    assert_eq!(state.core_loop[0].instance_id, "loot_main");
    // 落盘验证 3：确认留痕进 transcript（R3）。
    assert!(
        state.interview.transcript.iter().any(|entry| {
            entry.decision_id == "concept"
                && entry.role == "user_confirm"
                && entry.content.contains("击杀拾取→loot_main")
        }),
        "概念确认应留痕"
    );

    // 落盘验证 4（验收 6）：组合评估可跑且 core_loop_verbs 非空生效——
    // e3_socket 传导要求背包 ≥ classify，bag 是 basic_capacity → V2 违例在场。
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("确认落盘后应有组合报告");
    assert!(
        assessment
            .report
            .blocks
            .iter()
            .any(|finding| finding.detail.contains("classify")),
        "e3_socket + basic_capacity 应产 V2 传导违例：{:?}",
        assessment.report.blocks
    );

    // ---- 组合访谈：AI 解释 + 修复选项 → 选升档 → 违例消除 ----
    let provider = ScriptedProvider::new();
    provider.script(
        "interview_composition",
        vec![
            r#"{"explanation":"装备开到镶嵌档，宝石作为独立实体入包，但背包还是基础容量档没有分类页签——传导链在 bag_main 断了。","options":[{"option_id":"upgrade_bag","kind":"tier_change","instance_id":"bag_main","to_tier":"classify","detail":"背包升到分类堆叠档，宝石有分类可承接；代价是背包 UI 复杂度上升。"},{"option_id":"downgrade_equip","kind":"tier_change","instance_id":"equip_main","to_tier":"e2_skill_build","detail":"装备退回技能 BD 档，不引入宝石实体；代价是失去镶嵌构筑维度。"}]}"#
                .into(),
        ],
    );
    let fix = services
        .interview_compose_fix_with(&archive_id, &provider)
        .unwrap();
    assert!(fix.explanation.contains("传导链"), "{}", fix.explanation);
    assert_eq!(fix.options.len(), 2);
    // 违例文本进了 AI 上下文（组合访谈有据可讲）。
    let calls = provider.calls();
    assert!(
        calls[0].user_prompt.contains("classify"),
        "违例明细应进提示词"
    );

    // 用户选升档（手势）→ 执行走既有 select 链路。
    let message = services
        .interview_compose_fix_apply(&archive_id, &fix, "upgrade_bag", None)
        .unwrap();
    assert!(message.contains("classify"), "{message}");
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("应有组合报告");
    assert!(
        assessment.report.blocks.is_empty(),
        "升档后 V2 违例应消除：{:?}",
        assessment.report.blocks
    );
    let state = services.load_authoring_state(&archive_id).unwrap();
    let bag_tier = &state.selections["bag_main.tier"];
    assert_eq!(bag_tier.option_id, "classify");
    assert!(
        bag_tier.rationale.contains("组合访谈修复"),
        "{}",
        bag_tier.rationale
    );

    // 执行不存在的选项 → Err 点名。
    let error = services
        .interview_compose_fix_apply(&archive_id, &fix, "ghost_option", None)
        .unwrap_err();
    assert!(error.message.contains("ghost_option"), "{}", error.message);

    // ---- 机制访谈：实例范围逐点提案 + 弹药注入断言 ----
    let provider = ScriptedProvider::new();
    provider.script(
        "interview_mechanism",
        vec![r#"{"option_id":"slot_grid","rationale":"格子制让宝石与材料的取舍可见，回应了容量张力的取舍。","parameters":{"slot_count":40}}"#.into()],
    );
    let turn = services
        .interview_mechanism_next_with(&archive_id, &provider, "bag_main")
        .unwrap();
    let InterviewTurnDto::StructuralPoint {
        proposal: point_proposal,
    } = &turn
    else {
        panic!("背包实例应有待确认激活点，得到 {turn:?}");
    };
    assert!(
        point_proposal.decision_id.starts_with("bag_main."),
        "机制访谈范围必须限定实例命名空间：{}",
        point_proposal.decision_id
    );
    // 弹药注入可断言（验收 3）：seed.json 的 sys.inventory 域问句进了 prompt。
    let calls = provider.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].user_prompt.contains("追问弹药"), "弹药段应存在");
    assert!(
        calls[0].user_prompt.contains("背包容量给多紧"),
        "sys.inventory 域的弹药问句应注入：{}",
        calls[0].user_prompt
    );
    // 逐点提案确认（复用既有 confirm 链路）。
    services
        .interview_confirm(&archive_id, point_proposal, None)
        .unwrap();
    let state = services.load_authoring_state(&archive_id).unwrap();
    let selection = &state.selections[&point_proposal.decision_id];
    assert!(selection.confirmed_by_user);
    assert_eq!(selection.option_id, "slot_grid");

    // 不存在的实例 → Err 点名。
    let error = services
        .interview_mechanism_next_with(&archive_id, &provider, "ghost_instance")
        .unwrap_err();
    assert!(
        error.message.contains("ghost_instance"),
        "{}",
        error.message
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 计划 §4 硬断言（验收 2）：|H|>4 → 切逐重核模式 → 逐系统档位 + rationale 落盘
// ---------------------------------------------------------------------------

#[test]
fn heavy_core_mode_clarifies_each_system_and_persists_tier_with_rationale() {
    let (temp, services) = setup("heavy");
    let archive_id = new_project(&services, "大战略项目");

    // 5 个重核候选：装备实例 ×5，全部建议 e3_socket（W12）+ core/strong。
    // 绑定全指向 pack 核心名词与同伴 loot 实例——为控制变量，这里改为
    // 每实例自带一个 loot（供给）不现实；直接用核心名词绑定（装配可过）。
    // 注意：本测试考「模式切换 + 理清落盘」，不考组合违例。
    let systems: Vec<String> = (0..5)
        .map(|index| {
            format!(
                r#"{{ "instance_id": "war_sys_{index}", "module_id": "sys.equipment",
                     "suggested_tier": "e3_socket", "core_link": "core",
                     "rationale": "第 {index} 个重核",
                     "noun_bindings": {{
                       "sys.loot.drop_table": "combat_attribute",
                       "sys.loot.gem_entity": "combat_attribute",
                       "sys.loot.material_entity": "combat_attribute",
                       "sys.economy.currency_main": "combat_attribute",
                       "combat_attribute": "combat_attribute"
                     }} }}"#
            )
        })
        .collect();
    let concept = format!(
        r#"{{"systems":[{}],"core_loop":[{{"verb":"征战","instance_id":"war_sys_0"}}],"notes":"大战略"}}"#,
        systems.join(",")
    );
    let provider = ScriptedProvider::new();
    provider.script("interview_concept", vec![concept]);
    let proposal = services
        .interview_concept_with(
            &archive_id,
            &provider,
            "EU4 型大战略：外交战争贸易宗教科技全都要重",
        )
        .unwrap();

    // 硬断言 1：|H| 候选 >4 → 模式切换发生。
    assert_eq!(proposal.heavy_core_candidates.len(), 5);
    assert!(
        proposal.per_heavy_core_mode,
        "5 个重核候选必须切逐重核轻重理清模式"
    );
    // 只提示不设阻：提示在案，提案本身可继续（不是 Err）。
    assert!(
        proposal.hints.iter().any(|hint| hint.contains("逐重核")),
        "{:?}",
        proposal.hints
    );

    // 硬断言 2：未理清 → 确认被拒并点名。
    let error = services
        .interview_concept_confirm(&archive_id, &proposal)
        .unwrap_err();
    assert!(error.message.contains("未理清"), "{}", error.message);
    assert!(error.message.contains("war_sys_0"), "{}", error.message);

    // 逐重核理清：5 个系统逐一问答（4 重 1 轻——理清允许降档）。
    let clarifier = ScriptedProvider::new();
    clarifier.script(
        "interview_concept_tier",
        vec![
            r#"{"tier_id":"e3_socket","rationale":"对标 EU4 外交：全谈判栈要重度"}"#.into(),
            r#"{"tier_id":"e3_socket","rationale":"对标 EU4 战争：会战围城全要"}"#.into(),
            r#"{"tier_id":"e3_socket","rationale":"对标 EU4 贸易：贸易节点流向"}"#.into(),
            r#"{"tier_id":"e3_socket","rationale":"对标 EU4 宗教：改宗传教"}"#.into(),
            r#"{"tier_id":"e0_stat_bonus","rationale":"对标文明 6 科技：只要轻度线性树"}"#.into(),
        ],
    );
    let mut proposal = proposal;
    for index in 0..5 {
        proposal = services
            .interview_concept_clarify_with(
                &archive_id,
                &clarifier,
                proposal,
                &format!("war_sys_{index}"),
                if index == 4 {
                    "这个要轻度，对标文明 6 的科技树"
                } else {
                    "要重度，对标 EU4"
                },
            )
            .unwrap();
    }
    assert_eq!(proposal.tier_clarifications.len(), 5, "每答落一条理清记录");

    // 理清后确认放行 → 逐系统档位声明 + rationale 落盘。
    let report = services
        .interview_concept_confirm(&archive_id, &proposal)
        .unwrap();
    assert_eq!(report.instances.len(), 5);
    let state = services.load_authoring_state(&archive_id).unwrap();
    // 硬断言 3：每个重核系统落了档位声明与 rationale（理清档覆盖建议档）。
    for index in 0..4 {
        let point = format!("war_sys_{index}.tier");
        let selection = &state.selections[&point];
        assert_eq!(selection.option_id, "e3_socket");
        assert!(selection.confirmed_by_user);
        assert!(
            selection.rationale.contains("EU4"),
            "{point} 的 rationale 应带对标：{}",
            selection.rationale
        );
    }
    let light = &state.selections["war_sys_4.tier"];
    assert_eq!(light.option_id, "e0_stat_bonus", "理清降档应覆盖建议档");
    assert!(light.rationale.contains("文明"), "{}", light.rationale);
    // 理清确认留痕（用户原话进 transcript，R3）。
    assert!(
        state.interview.transcript.iter().any(|entry| {
            entry.decision_id == "war_sys_4.tier"
                && entry.role == "user_confirm"
                && entry.content.contains("文明 6")
        }),
        "理清留痕应含用户原话"
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 融合型嗅探（验收 4）："X+Y" 口述 → 双核并集分解提案结构
// ---------------------------------------------------------------------------

#[test]
fn fusion_sniff_produces_dual_core_decomposition() {
    let (temp, services) = setup("fusion");
    let archive_id = new_project(&services, "融合项目");
    let provider = ScriptedProvider::new();
    provider.script(
        "interview_concept",
        vec![
            r#"{
              "systems": [
                { "instance_id": "merge_loot", "module_id": "sys.loot", "suggested_tier": "basic_table",
                  "core_link": "core", "rationale": "合成核的产出源", "noun_bindings": {} },
                { "instance_id": "defense_econ", "module_id": "sys.economy", "suggested_tier": "basic_income",
                  "core_link": "core", "rationale": "塔防核的建造货币",
                  "noun_bindings": { "sys.loot.material_entity": "merge_loot.material_entity", "money_supply": "money_supply" } }
              ],
              "core_loop": [
                { "verb": "合成", "instance_id": "merge_loot" },
                { "verb": "布防", "instance_id": "defense_econ" }
              ],
              "fusion": {
                "cores": [
                  { "label": "合成大西瓜", "instance_ids": ["merge_loot"] },
                  { "label": "塔防", "instance_ids": ["defense_econ"] }
                ],
                "transition": "合成产物折算为建造资金进入塔防波次，波次结算返还合成素材"
              },
              "notes": "融合型双核"
            }"#
            .into(),
        ],
    );
    let proposal = services
        .interview_concept_with(&archive_id, &provider, "合成大西瓜+塔防")
        .unwrap();
    let fusion = proposal.fusion.as_ref().expect("X+Y 口述应产融合分解");
    assert_eq!(fusion.cores.len(), 2, "双核并集分解");
    assert_eq!(fusion.cores[0].label, "合成大西瓜");
    assert_eq!(fusion.cores[1].label, "塔防");
    assert!(
        fusion.transition.contains("波次"),
        "跨核转换说明：{}",
        fusion.transition
    );
    // 两核系统清单并集 = 提案系统清单；嵌套 core_loop 双核动词都在。
    assert_eq!(proposal.systems.len(), 2);
    assert_eq!(proposal.core_loop.len(), 2);
    // 融合提案照常可确认落盘（融合不是特殊通道）。
    let report = services
        .interview_concept_confirm(&archive_id, &proposal)
        .unwrap();
    assert_eq!(report.instances, vec!["merge_loot", "defense_econ"]);
    // 确认留痕带融合分解说明。
    let state = services.load_authoring_state(&archive_id).unwrap();
    assert!(
        state.interview.transcript.iter().any(|entry| {
            entry.decision_id == "concept" && entry.content.contains("合成大西瓜 + 塔防")
        }),
        "融合分解应进留痕"
    );

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// AI 越界负测试（验收 5）：发明模块 id / 档位 id → Err 不吞、零落盘
// ---------------------------------------------------------------------------

#[test]
fn ai_overstep_is_rejected_and_nothing_persists() {
    let (temp, services) = setup("overstep");
    let archive_id = new_project(&services, "越界负测试");

    // 发明模块 id。
    let provider = ScriptedProvider::new();
    provider.script(
        "interview_concept",
        vec![
            r#"{"systems":[{"instance_id":"ghost","module_id":"sys.mind_control","suggested_tier":"basic","core_link":"core","rationale":"x"}]}"#
                .into(),
        ],
    );
    let error = services
        .interview_concept_with(&archive_id, &provider, "口述")
        .unwrap_err();
    assert!(
        error.message.contains("sys.mind_control"),
        "{}",
        error.message
    );

    // 发明档位 id。
    let provider = ScriptedProvider::new();
    provider.script(
        "interview_concept",
        vec![
            r#"{"systems":[{"instance_id":"loot_main","module_id":"sys.loot","suggested_tier":"mythic_plus","core_link":"core","rationale":"x"}]}"#
                .into(),
        ],
    );
    let error = services
        .interview_concept_with(&archive_id, &provider, "口述")
        .unwrap_err();
    assert!(error.message.contains("mythic_plus"), "{}", error.message);

    // 越界后零落盘：项目无 system_refs、无 tier 选择、无 core_loop。
    let state = services.load_authoring_state(&archive_id).unwrap();
    assert!(state.selections.is_empty(), "越界提案不得留下任何选择");
    assert!(state.core_loop.is_empty());
    assert!(
        services.composition_report(&archive_id).unwrap().is_none(),
        "越界提案不得落盘任何实例引用"
    );

    // custom 草案越界（GWT 缺段）也不吞——机制访谈的起草防线。
    // 先落一个正常概念（复用四件套），再对其系统起草。
    let provider = ScriptedProvider::new();
    provider.script("interview_concept", vec![four_system_concept_json()]);
    let proposal = services
        .interview_concept_with(&archive_id, &provider, "刷宝")
        .unwrap();
    services
        .interview_concept_confirm(&archive_id, &proposal)
        .unwrap();
    let provider = ScriptedProvider::new();
    provider.script(
        "interview_mechanism_custom",
        vec![
            r#"{"host_system_id":"x","slug":"half_baked","label_zh":"半成品","rule_text":"规则","effects":[{"effect":"custom","verb":"v","given":"g","when":"","then":"t"}],"rationale":"理由"}"#
                .into(),
        ],
    );
    let error = services
        .interview_mechanism_draft_custom_with(
            &archive_id,
            &provider,
            "bag_main.capacity_model",
            "想要个机制",
        )
        .unwrap_err();
    assert!(error.message.contains("when"), "{}", error.message);

    std::fs::remove_dir_all(&temp).ok();
}
