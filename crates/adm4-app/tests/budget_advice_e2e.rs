//! T-W7-5-0 预算提示接线的 app 级端到端验收。
//!
//! 覆盖：
//! - **MOBA 级组合在 mid_core 档触发预算提示、在 mmo 档不触发**（任务卡验收原文）：
//!   真实四模块重档组合 B(G)=43.5，落在中核预算 42 与 MMO 预算 68 之间——
//!   V5BudgetAdvice 是提示不 block，gate2 不因它拦冻结；
//! - **预算数据流**：`knowledge/calibration/budget.json`（占位试用态数值）经
//!   services → engine → assess_composition 真实装载，提示文案含贡献降序表；
//! - **占位期负测试**：budget.json 缺失时预算检查静默跳过（0c 空表语义），
//!   同一组合零预算提示——提示制不阻塞的护栏。

use adm4_app::{AppConfig, AppServices, save_config};
use adm4_archive::DataRoot;
use adm4_decision::FindingCode;
use adm4_decision::{DesignLevel, Provenance};
use std::path::{Path, PathBuf};

/// 通用层带 u.target_scale（L0 画像点：compose.rs 的产品档数据源）。
const UNIVERSAL_CORE: &str = r#"{
  "space_version": "budgettest-1",
  "decision_points": [
    { "id": "u.target_scale", "domain": "core", "level": "L0", "genre_scope": "universal",
      "question": "产品规模档位？",
      "options": [
        { "id": "iaa_hypercasual", "label": "超休闲" },
        { "id": "indie", "label": "独立" },
        { "id": "midcore", "label": "中核" },
        { "id": "triple_a", "label": "大制作" },
        { "id": "large_service", "label": "大型长线服务" }
      ] },
    { "id": "u.promise", "domain": "core", "level": "L1", "genre_scope": "universal",
      "question": "体验承诺？",
      "options": [ { "id": "loot_fantasy", "label": "刷宝幻想" }, { "id": "mastery", "label": "技巧精进" } ] },
    { "id": "u.genre", "domain": "core", "level": "L2", "genre_scope": "universal",
      "question": "品类？",
      "options": [ { "id": "arpg", "label": "刷宝动作" }, { "id": "puzzle", "label": "解谜" } ] }
  ]
}"#;

/// MOBA 级组合：真实四模块全取重档（装备 e4 / 掉落 pity / 背包 batch / 经济 exchange），
/// H = 全部四实例（W 15/12/11/11，κ core/core/strong/strong），B(G) = 27 + 16.5 = 43.5
/// ——与 MOBA 锚点（50.25）同档带（中核预算 42 < B ≤ MMO 预算 68）。
/// 绑定与 composition_gate_e2e 的 chain 包同口径，四实例互相成边（连通、无割点）。
const PACK_MOBA_SCALE: &str = r#"{
  "pack_id": "moba_scale_pack",
  "pack_version": "0.1.0",
  "display_name": "MOBA 级预算测试包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "core_nouns": ["combat_unit_slot"],
  "decision_points": [],
  "system_refs": [
    { "instance_id": "equip_main", "module_id": "sys.equipment", "version_req": "^1.0.0",
      "core_link": "core",
      "noun_bindings": {
        "sys.loot.drop_table": "loot_main.drop_table",
        "sys.loot.gem_entity": "loot_main.gem_entity",
        "sys.loot.material_entity": "loot_main.material_entity",
        "sys.economy.currency_main": "econ_main.currency_main",
        "combat_attribute": "combat_unit_slot"
      } },
    { "instance_id": "loot_main", "module_id": "sys.loot", "version_req": "^1.0.0",
      "core_link": "core", "noun_bindings": {} },
    { "instance_id": "bag_main", "module_id": "sys.inventory", "version_req": "^1.0.0",
      "core_link": "strong",
      "noun_bindings": {
        "sys.equipment.equipment_entity": "equip_main.equipment_entity",
        "sys.loot.material_entity": "loot_main.material_entity",
        "sys.loot.gem_entity": "loot_main.gem_entity",
        "storage_capacity": "combat_unit_slot"
      } },
    { "instance_id": "econ_main", "module_id": "sys.economy", "version_req": "^1.0.0",
      "core_link": "strong",
      "noun_bindings": {
        "sys.loot.material_entity": "loot_main.material_entity",
        "money_supply": "combat_unit_slot"
      } }
  ]
}"#;

fn repo_knowledge_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

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

/// 夹具：`with_budget=false` 即占位期形态（无 budget.json，空表静默跳过）。
fn setup(tag: &str, with_budget: bool) -> (PathBuf, AppServices) {
    let temp = std::env::temp_dir().join(format!("adm4_budget_e2e_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&temp).ok();
    let space_root = temp.join("design_space");
    std::fs::create_dir_all(space_root.join("universal")).unwrap();
    std::fs::write(
        space_root.join("universal").join("core.json"),
        UNIVERSAL_CORE,
    )
    .unwrap();
    std::fs::create_dir_all(space_root.join("moba_scale_pack")).unwrap();
    std::fs::write(
        space_root.join("moba_scale_pack").join("pack.json"),
        PACK_MOBA_SCALE,
    )
    .unwrap();
    let systems_root = temp.join("systems");
    std::fs::create_dir_all(&systems_root).unwrap();
    for module_id in ["sys.equipment", "sys.inventory", "sys.loot", "sys.economy"] {
        copy_module(&systems_root, module_id);
    }
    if with_budget {
        // 消费真预算文件（knowledge/calibration/budget.json）而非测试私拟数值——
        // 入库数值改动必须让本测试跟着表态。
        let calibration = temp.join("calibration");
        std::fs::create_dir_all(&calibration).unwrap();
        std::fs::copy(
            repo_knowledge_root()
                .join("calibration")
                .join("budget.json"),
            calibration.join("budget.json"),
        )
        .unwrap();
    }
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

fn select(services: &AppServices, archive_id: &str, decision: &str, option: &str) {
    services
        .with_project(archive_id, |engine| {
            engine.select_option(decision, option, Provenance::UserManual)?;
            engine.confirm_selection(decision)
        })
        .unwrap();
}

/// 建 MOBA 级组合项目并声明四实例重档。
fn new_moba_scale_project(services: &AppServices) -> String {
    let archive_id = services
        .project_new("MOBA级预算项目", "moba_scale_pack", DesignLevel::L6, None)
        .unwrap();
    select(services, &archive_id, "equip_main.tier", "e4_craft");
    select(services, &archive_id, "loot_main.tier", "pity_directed");
    select(services, &archive_id, "bag_main.tier", "batch_ops");
    select(
        services,
        &archive_id,
        "econ_main.tier",
        "exchange_reservoir",
    );
    archive_id
}

#[test]
fn moba_scale_budget_advice_fires_at_midcore_and_not_at_mmo() {
    let (temp, services) = setup("fires", true);
    let archive_id = new_moba_scale_project(&services);

    // 中核档：B(G)=43.5 > 42 → V5 提示产出，附贡献降序表（最贵 = 装备 15.0）。
    select(&services, &archive_id, "u.target_scale", "midcore");
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert!(
        (assessment.report.budget_total - 43.5).abs() < 1e-9,
        "B(G) 应为 43.5（15+12 核心 + (11+11)×0.75），实际 {}",
        assessment.report.budget_total
    );
    assert!(
        assessment.report.blocks.is_empty(),
        "组合本体无硬违例（传导/连通全过）：{:?}",
        assessment.report.blocks
    );
    let v5 = assessment
        .report
        .advices
        .iter()
        .find(|finding| finding.code == FindingCode::V5BudgetAdvice)
        .expect("中核档必须触发预算提示");
    assert!(v5.detail.contains("43.50"), "{}", v5.detail);
    assert!(v5.detail.contains("42.00"), "{}", v5.detail);
    assert!(
        v5.detail.contains("指向减重度档而非删系统"),
        "{}",
        v5.detail
    );
    assert_eq!(
        v5.related[0], "equip_main",
        "贡献降序表最贵的应是装备（15.0）：{:?}",
        v5.related
    );

    // 提示不 block：gate2 不因预算提示拦冻结（|H|=4 超中核参考线 2 的形态确认
    // 是另一条独立提示义务，与预算无关）。
    let report = services.freeze_check(&archive_id).unwrap();
    let gate2 = report
        .gates
        .iter()
        .find(|gate| gate.gate == "gate2_consistency")
        .expect("gate2 应存在");
    assert!(
        !gate2
            .findings
            .iter()
            .any(|finding| finding.code.starts_with("composition.v5")),
        "预算是提示不是违例，不得进 gate2 block：{:?}",
        gate2.findings
    );

    // MMO 档：B(G)=43.5 ≤ 68 且 |H|=4 ≤ 参考线 4 → 零提示。
    select(&services, &archive_id, "u.target_scale", "large_service");
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert!(
        assessment.report.advices.is_empty(),
        "MMO 档不应有任何提示：{:?}",
        assessment.report.advices
    );
    assert!(assessment.report.blocks.is_empty());

    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn missing_budget_file_keeps_placeholder_silence() {
    let (temp, services) = setup("silent", false);
    let archive_id = new_moba_scale_project(&services);
    select(&services, &archive_id, "u.target_scale", "midcore");
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    // 占位期语义（0c）：查无本档预算值时预算检查静默跳过——B(G) 结构事实照出，
    // 但零 V5 提示。这是"提示制不阻塞"的回滚护栏：删掉 budget.json 即回到标定前状态。
    assert!(
        (assessment.report.budget_total - 43.5).abs() < 1e-9,
        "budget_total 是结构事实，与预算表有无无关"
    );
    assert!(
        !assessment
            .report
            .advices
            .iter()
            .any(|finding| finding.code == FindingCode::V5BudgetAdvice),
        "无预算表时不得产预算提示：{:?}",
        assessment.report.advices
    );
    std::fs::remove_dir_all(&temp).ok();
}
