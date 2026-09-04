//! W7 3b 组合校验接线的 app 级端到端验收（任务计划 §4 三测 + 负测试）。
//!
//! 覆盖：
//! - **V2 传导**（真实四模块 sys.equipment/sys.inventory/sys.loot/sys.economy）：
//!   装备声明 e3_socket（要求背包 ≥ classify）+ 背包声明 basic_capacity →
//!   gate2 block 点名传导链；升背包档后放行；
//! - **连通性硬 block 不可豁免**：两个 W≥9 重核实例零接口边 → V3a/V3b block，
//!   署名形态确认后仍 block；
//! - **|H| 确认流**：超参考线 → advice + form_confirmation_required → 未确认可继续
//!   创作但报告持续提示 → confirm 署名 → 不再要求、R3 留痕可查 → 新增重核
//!   （h_set 变化）→ 确认失效重新要求；
//! - **旧项目负测试**：无 system_refs 的项目 gate 报告与扩展前逐字节一致
//!   （composition_report 返回 None，gate2 无任何 composition 前缀 finding）；
//! - **authoring 即时反馈与 gate2 逐字节一致**：composition_report 的 blocks 与
//!   gate2 的 composition.* finding 一一对应。

use adm4_app::{AppConfig, AppServices, save_config};
use adm4_archive::DataRoot;
use adm4_decision::{DesignLevel, Provenance};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 夹具：最小通用层 + 真实四模块库副本
// ---------------------------------------------------------------------------

const UNIVERSAL_CORE: &str = r#"{
  "space_version": "composetest-1",
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

/// 传导包：真实四模块各一实例，绑定按各模块 consumes∪modifies 全覆盖。
/// 装备 κ=strong、背包/掉落/货币 κ=weak——装备重档时传导链是唯一考点，
/// 不让 |H|/连通检查掺进来（装备 e3 档 W12 入 H 但 |H|=1 无 (b) 要求）。
const PACK_CHAIN: &str = r#"{
  "pack_id": "chain_pack",
  "pack_version": "0.1.0",
  "display_name": "传导链测试包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "core_nouns": ["combat_unit_slot"],
  "decision_points": [],
  "system_refs": [
    { "instance_id": "equip_main", "module_id": "sys.equipment", "version_req": "^1.0.0",
      "core_link": "strong",
      "noun_bindings": {
        "sys.loot.drop_table": "loot_main.drop_table",
        "sys.loot.gem_entity": "loot_main.gem_entity",
        "sys.loot.material_entity": "loot_main.material_entity",
        "sys.economy.currency_main": "econ_main.currency_main",
        "combat_attribute": "combat_unit_slot"
      } },
    { "instance_id": "bag_main", "module_id": "sys.inventory", "version_req": "^1.0.0",
      "core_link": "weak",
      "noun_bindings": {
        "sys.equipment.equipment_entity": "equip_main.equipment_entity",
        "sys.loot.material_entity": "loot_main.material_entity",
        "sys.loot.gem_entity": "loot_main.gem_entity",
        "storage_capacity": "bag_main.storage_capacity"
      } },
    { "instance_id": "loot_main", "module_id": "sys.loot", "version_req": "^1.0.0",
      "core_link": "strong", "noun_bindings": {} },
    { "instance_id": "econ_main", "module_id": "sys.economy", "version_req": "^1.0.0",
      "core_link": "weak",
      "noun_bindings": {
        "sys.loot.material_entity": "loot_main.material_entity",
        "money_supply": "combat_unit_slot"
      } }
  ]
}"#;

/// 钉接包：两个装备实例互不相连（W≥9 重核 × 2，零接口边）——
/// V3a 不连通 + V3b 边数不足，双双硬 block；同时 |H|=2 超默认超休闲参考线 0，
/// 确认流与"确认不豁免硬 block"用同一个包验证。
/// 绑定全部指向 pack 核心名词（成边不产生实例间边）。
const PACK_STAPLED: &str = r#"{
  "pack_id": "stapled_pack",
  "pack_version": "0.1.0",
  "display_name": "钉接测试包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "core_nouns": ["drop_table", "gem_entity", "material_entity", "currency_main", "combat_attribute"],
  "decision_points": [],
  "system_refs": [
    { "instance_id": "equip_alpha", "module_id": "sys.equipment", "version_req": "",
      "core_link": "core",
      "noun_bindings": {
        "sys.loot.drop_table": "drop_table",
        "sys.loot.gem_entity": "gem_entity",
        "sys.loot.material_entity": "material_entity",
        "sys.economy.currency_main": "currency_main",
        "combat_attribute": "combat_attribute"
      } },
    { "instance_id": "equip_beta", "module_id": "sys.equipment", "version_req": "",
      "core_link": "strong",
      "noun_bindings": {
        "sys.loot.drop_table": "drop_table",
        "sys.loot.gem_entity": "gem_entity",
        "sys.loot.material_entity": "material_entity",
        "sys.economy.currency_main": "currency_main",
        "combat_attribute": "combat_attribute"
      } },
    { "instance_id": "equip_gamma", "module_id": "sys.equipment", "version_req": "",
      "core_link": "strong",
      "noun_bindings": {
        "sys.loot.drop_table": "drop_table",
        "sys.loot.gem_entity": "gem_entity",
        "sys.loot.material_entity": "material_entity",
        "sys.economy.currency_main": "currency_main",
        "combat_attribute": "combat_attribute"
      } }
  ]
}"#;

/// 旧项目负测试用：无 system_refs 的纯决策点包（gate 报告零变化锁定对象）。
const PACK_LEGACY: &str = r#"{
  "pack_id": "legacy_pack",
  "pack_version": "0.1.0",
  "display_name": "旧项目负测试包",
  "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
  "decision_points": []
}"#;

fn repo_systems_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
        .join("systems")
}

fn copy_module(temp_systems: &Path, module_id: &str) {
    let target = temp_systems.join(module_id);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(
        repo_systems_root().join(module_id).join("module.json"),
        target.join("module.json"),
    )
    .unwrap();
}

fn setup(tag: &str, packs: &[(&str, &str)]) -> (PathBuf, AppServices) {
    let temp = std::env::temp_dir().join(format!("adm4_compose_e2e_{tag}_{}", std::process::id()));
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
    let systems_root = temp.join("systems");
    std::fs::create_dir_all(&systems_root).unwrap();
    for module_id in ["sys.equipment", "sys.inventory", "sys.loot", "sys.economy"] {
        copy_module(&systems_root, module_id);
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

/// gate2 的 findings（组合校验合流的落点）。
fn gate2_findings(services: &AppServices, archive_id: &str) -> Vec<(String, String)> {
    let report = services.freeze_check(archive_id).unwrap();
    report
        .gates
        .iter()
        .find(|gate| gate.gate == "gate2_consistency")
        .expect("gate2 应存在")
        .findings
        .iter()
        .map(|finding| (finding.code.clone(), finding.message.clone()))
        .collect()
}

fn gate2_passed(services: &AppServices, archive_id: &str) -> bool {
    services
        .freeze_check(archive_id)
        .unwrap()
        .gates
        .iter()
        .find(|gate| gate.gate == "gate2_consistency")
        .expect("gate2 应存在")
        .passed
}

fn select_tier(services: &AppServices, archive_id: &str, decision: &str, option: &str) {
    services
        .with_project(archive_id, |engine| {
            engine.select_option(decision, option, Provenance::UserManual)?;
            engine.confirm_selection(decision)
        })
        .unwrap();
}

// ---------------------------------------------------------------------------
// 验收 2：V2 传导（真实四模块）——装备重档 + 背包轻档 block，升档放行
// ---------------------------------------------------------------------------

#[test]
fn v2_transmission_blocks_then_clears_after_tier_upgrade() {
    let (temp, services) = setup("chain", &[("chain_pack", PACK_CHAIN)]);
    let archive_id = services
        .project_new("传导链项目", "chain_pack", DesignLevel::L6, None)
        .unwrap();

    // 四实例先各声明档位：装备 e3_socket（要求 inventory ≥ classify、loot 供给
    // gem_entity）、背包 basic_capacity（rank 0 < classify rank 1）、
    // 掉落 quality_affix_weights（供给档）、货币 basic_income。
    select_tier(&services, &archive_id, "equip_main.tier", "e3_socket");
    select_tier(&services, &archive_id, "bag_main.tier", "basic_capacity");
    select_tier(
        &services,
        &archive_id,
        "loot_main.tier",
        "quality_affix_weights",
    );
    select_tier(&services, &archive_id, "econ_main.tier", "basic_income");

    // gate2：V2 block 点名传导链（equip_main → sys.inventory ≥ classify，
    // 组合内最高实例 bag_main 声明档 basic_capacity 未达标）。
    let findings = gate2_findings(&services, &archive_id);
    let v2: Vec<&(String, String)> = findings
        .iter()
        .filter(|(code, _)| code == "composition.v2_transmission_unmet")
        .collect();
    assert_eq!(v2.len(), 1, "findings：{findings:?}");
    let message = &v2[0].1;
    assert!(message.contains("传导链"), "{message}");
    assert!(message.contains("equip_main"), "{message}");
    assert!(message.contains("bag_main"), "{message}");
    assert!(message.contains("sys.inventory"), "{message}");
    assert!(message.contains("classify"), "{message}");
    assert!(!gate2_passed(&services, &archive_id));

    // authoring 即时反馈与 gate2 结论逐字节一致（同一纯函数）。
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert_eq!(assessment.report.blocks.len(), 1);
    assert_eq!(
        format!(
            "{}：{}",
            assessment.report.blocks[0].subject, assessment.report.blocks[0].detail
        ),
        *message,
        "authoring 报告与 gate2 finding 必须逐字节一致"
    );

    // 升背包档到 classify → V2 放行，gate2 组合段零 block。
    select_tier(&services, &archive_id, "bag_main.tier", "classify");
    let findings = gate2_findings(&services, &archive_id);
    assert!(
        !findings
            .iter()
            .any(|(code, _)| code.starts_with("composition.")),
        "升档后不应再有组合硬违例：{findings:?}"
    );
    assert!(gate2_passed(&services, &archive_id));
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert!(assessment.report.blocks.is_empty());
    assert!(assessment.missing_tiers.is_empty());

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// tier 未选择：不默认兜底，gate2 block 点名实例
// ---------------------------------------------------------------------------

#[test]
fn unselected_tier_blocks_gate2_naming_instance() {
    let (temp, services) = setup("notier", &[("chain_pack", PACK_CHAIN)]);
    let archive_id = services
        .project_new("缺档项目", "chain_pack", DesignLevel::L6, None)
        .unwrap();
    // 只选一个实例的档，其余三个缺档。
    select_tier(&services, &archive_id, "loot_main.tier", "basic_table");
    let findings = gate2_findings(&services, &archive_id);
    let missing: Vec<&(String, String)> = findings
        .iter()
        .filter(|(code, _)| code == "composition_tier_unselected")
        .collect();
    assert_eq!(missing.len(), 3, "findings：{findings:?}");
    for instance in ["equip_main", "bag_main", "econ_main"] {
        assert!(
            missing
                .iter()
                .any(|(_, message)| message.contains(instance)),
            "缺 {instance} 的点名：{missing:?}"
        );
    }
    assert!(!gate2_passed(&services, &archive_id));
    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 验收 3+4：连通性硬 block 不可豁免 + |H| 署名形态确认流全链
// ---------------------------------------------------------------------------

#[test]
fn disconnected_heavy_cores_block_unforgivably_and_confirmation_flow_works() {
    let (temp, services) = setup("stapled", &[("stapled_pack", PACK_STAPLED)]);
    let archive_id = services
        .project_new("钉接项目", "stapled_pack", DesignLevel::L6, None)
        .unwrap();

    // 两个装备实例都声明 e3_socket（W12 重核；κ=core/strong 入 H），零实例间边。
    // 第三个实例（equip_gamma）先不选档——它是后面"h_set 变化"的道具。
    // 但缺档会 block，先只用两实例验证：gamma 暂选最轻档（W3 不入 H）。
    select_tier(&services, &archive_id, "equip_alpha.tier", "e3_socket");
    select_tier(&services, &archive_id, "equip_beta.tier", "e3_socket");
    select_tier(&services, &archive_id, "equip_gamma.tier", "e0_stat_bonus");

    // 阶段 1：V3a 不连通 + V3b 强耦合双硬 block；|H|=2 超默认（超休闲）参考线 0
    // → advice + form_confirmation_required。
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert_eq!(assessment.report.h_set, vec!["equip_alpha", "equip_beta"]);
    assert!(!assessment.report.h_connected);
    assert!(assessment.report.form_confirmation_required);
    let findings = gate2_findings(&services, &archive_id);
    assert!(
        findings
            .iter()
            .any(|(code, _)| code == "composition.v3a_disconnected"),
        "{findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|(code, _)| code == "composition.v3b_weak_coupling"),
        "{findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|(code, _)| code == "composition_form_confirmation_required"),
        "{findings:?}"
    );
    assert!(!gate2_passed(&services, &archive_id));

    // 未确认时创作可继续（select 等操作不被组合报告拦截）——报告只是持续提示。
    select_tier(&services, &archive_id, "u.audience", "core_players");

    // 阶段 2：署名确认 → form_confirmation_required 消失、R3 留痕可查；
    // **硬 block 仍在场（不可豁免）**。
    let record = services
        .compose_confirm_form(&archive_id, "设计师甲", "接受双核钉接实验形态")
        .unwrap();
    assert_eq!(record.signer, "设计师甲");
    assert_eq!(record.h_set, vec!["equip_alpha", "equip_beta"]);
    assert!(!record.at.is_empty(), "R3 留痕必须带时间戳");
    // 留痕进项目存档（重开门面仍可查）。
    let state = services.load_authoring_state(&archive_id).unwrap();
    let stored = state
        .composition_form_confirmation
        .as_ref()
        .expect("确认留痕应持久化");
    assert_eq!(stored.signer, "设计师甲");
    assert_eq!(stored.note, "接受双核钉接实验形态");

    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert!(!assessment.report.form_confirmation_required);
    assert_eq!(
        assessment.confirmation.as_ref().map(|c| c.signer.as_str()),
        Some("设计师甲")
    );
    // |H| 数量提示仍产出（提示义务不消失，只是不再要求确认）。
    assert!(
        assessment
            .report
            .advices
            .iter()
            .any(|finding| finding.detail.contains("已署名确认")),
        "advices：{:?}",
        assessment.report.advices
    );
    let findings = gate2_findings(&services, &archive_id);
    assert!(
        findings
            .iter()
            .any(|(code, _)| code == "composition.v3a_disconnected"),
        "署名确认不得豁免连通硬违例：{findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|(code, _)| code == "composition.v3b_weak_coupling"),
        "署名确认不得豁免强耦合硬违例：{findings:?}"
    );
    assert!(
        !findings
            .iter()
            .any(|(code, _)| code == "composition_form_confirmation_required"),
        "已确认后不应再要求：{findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|(code, _)| code == "composition_form_confirmed"),
        "生效确认应在门报告里可见（R3）：{findings:?}"
    );
    assert!(
        !gate2_passed(&services, &archive_id),
        "硬违例在场，gate2 不得因确认而放行"
    );

    // 阶段 3：第三个实例升到重核档（h_set 变化）→ 确认失效重新要求。
    select_tier(&services, &archive_id, "equip_gamma.tier", "e3_socket");
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert_eq!(
        assessment.report.h_set,
        vec!["equip_alpha", "equip_beta", "equip_gamma"]
    );
    assert!(
        assessment.report.form_confirmation_required,
        "h_set 变化后确认必须失效重新要求"
    );
    assert!(assessment.confirmation_stale);
    assert!(assessment.confirmation.is_none());
    let findings = gate2_findings(&services, &archive_id);
    let required = findings
        .iter()
        .find(|(code, _)| code == "composition_form_confirmation_required")
        .expect("失效后应重新要求确认");
    assert!(
        required.1.contains("已失效"),
        "提示应说明是失效重签：{}",
        required.1
    );

    // 重新署名 → 再次生效（快照更新为三重核）。
    let renewed = services
        .compose_confirm_form(&archive_id, "设计师甲", "三核形态再确认")
        .unwrap();
    assert_eq!(renewed.h_set.len(), 3);
    let assessment = services
        .composition_report(&archive_id)
        .unwrap()
        .expect("有 system_refs 应产报告");
    assert!(!assessment.report.form_confirmation_required);

    // 未被要求时拒签（防预防性签名）：立刻再签一次应被拒。
    let error = services
        .compose_confirm_form(&archive_id, "设计师乙", "")
        .unwrap_err();
    assert!(error.message.contains("不要求"), "{}", error.message);
    // 空署名拒收（R3）。
    // 先制造需要确认的场景不值得——空署名校验在前，任意项目即可验。
    let error = services
        .compose_confirm_form(&archive_id, "  ", "")
        .unwrap_err();
    assert!(error.message.contains("署名"), "{}", error.message);

    std::fs::remove_dir_all(&temp).ok();
}

// ---------------------------------------------------------------------------
// 验收 5：旧项目（无 system_refs）gate 报告与门禁行为零变化（负测试）
// ---------------------------------------------------------------------------

#[test]
fn legacy_project_without_system_refs_sees_no_composition_findings() {
    let (temp, services) = setup("legacy", &[("legacy_pack", PACK_LEGACY)]);
    let archive_id = services
        .project_new("旧项目", "legacy_pack", DesignLevel::L6, None)
        .unwrap();

    // composition_report：None（零开销路径）。
    assert!(
        services.composition_report(&archive_id).unwrap().is_none(),
        "无 system_refs 的项目必须返回 None"
    );

    // gate 报告：任何门里都不得出现 composition 前缀的 finding——
    // 组合段对旧项目是不存在的，不是"空结果"。
    let report = services.freeze_check(&archive_id).unwrap();
    for gate in &report.gates {
        for finding in &gate.findings {
            assert!(
                !finding.code.starts_with("composition"),
                "旧项目 gate 报告不得出现组合 finding：{} {}",
                gate.gate,
                finding.code
            );
        }
    }
    // gate2 findings 为空（与扩展前一致：三个通用点无约束违例）。
    let gate2 = report
        .gates
        .iter()
        .find(|gate| gate.gate == "gate2_consistency")
        .unwrap();
    assert!(gate2.findings.is_empty(), "{:?}", gate2.findings);
    assert!(gate2.passed);

    // 确认流对旧项目显式拒绝（无形态可确认）。
    let error = services
        .compose_confirm_form(&archive_id, "设计师甲", "")
        .unwrap_err();
    assert!(error.message.contains("无形态可确认"), "{}", error.message);

    std::fs::remove_dir_all(&temp).ok();
}
