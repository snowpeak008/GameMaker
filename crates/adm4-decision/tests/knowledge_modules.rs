//! 系统模块库（knowledge/systems/*/module.json）的永久门禁（T-W7-3c）。
//!
//! 无痛接入纪律（定稿 §9.2b）：第 5 个及以后的模块只要落一个 module.json
//! 就自动纳入本门禁——遍历目录、逐个反序列化 + validate + 内容红线检查，
//! 零代码改动。本文件是一次性通用基建，不随模块数增长。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use adm4_decision::{DesignLevel, FiveAxisRating, HeavinessBand, InductionTarget, SystemModule};

fn systems_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
        .join("systems")
}

/// 遍历全部 <module_id>/module.json，反序列化为 SystemModule。
/// 目录名与 module_id 一致性也在此核对——目录即命名空间，漂移即歧义。
fn load_all_modules() -> BTreeMap<String, SystemModule> {
    let root = systems_root();
    let mut modules = BTreeMap::new();
    for entry in
        std::fs::read_dir(&root).unwrap_or_else(|e| panic!("{} 应可读：{e}", root.display()))
    {
        let dir = entry.expect("目录项应可读").path();
        if !dir.is_dir() {
            continue;
        }
        let path = dir.join("module.json");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} 应可读：{e}", path.display()));
        let module: SystemModule = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("{} 应反序列化为 SystemModule：{e}", path.display()));
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .expect("目录名应为合法 UTF-8")
            .to_string();
        assert_eq!(
            module.module_id,
            dir_name,
            "{}：module_id 与目录名不一致",
            path.display()
        );
        modules.insert(module.module_id.clone(), module);
    }
    assert!(
        modules.len() >= 4,
        "首批应至少 4 个系统模块，实际 {} 个",
        modules.len()
    );
    modules
}

/// 每个模块必须通过结构自校验（validate 是入库门槛）。
#[test]
fn every_module_deserializes_and_validates() {
    for (id, module) in load_all_modules() {
        module
            .validate()
            .unwrap_or_else(|e| panic!("模块 {id} 未过 validate：{}", e.message));
    }
}

/// 装备阶梯档带与定稿 §3.4 逐档一致（总分用 FiveAxisRating::total、档带用 band）。
#[test]
fn equipment_ladder_bands_match_finalized_calibration() {
    let modules = load_all_modules();
    let equipment = modules
        .get("sys.equipment")
        .expect("库内应有 sys.equipment");
    let expectations: [(&str, u16, HeavinessBand); 7] = [
        ("e0_stat_bonus", 3, HeavinessBand::Light),
        ("e1_quality_affix", 7, HeavinessBand::Medium),
        ("e2_skill_build", 11, HeavinessBand::Heavy),
        ("e3_socket", 12, HeavinessBand::Heavy),
        ("e4_craft", 15, HeavinessBand::UltraHeavy),
        ("e5_enhance", 15, HeavinessBand::UltraHeavy),
        ("e6_set_transmog", 15, HeavinessBand::UltraHeavy),
    ];
    assert_eq!(
        equipment.heaviness.tiers.len(),
        expectations.len(),
        "装备阶梯档数与定稿 §3.4 不符"
    );
    for (index, (id, total, band)) in expectations.iter().enumerate() {
        let tier = &equipment.heaviness.tiers[index];
        assert_eq!(&tier.id, id, "第 {index} 档 id 不符");
        let rating: FiveAxisRating = tier.rating;
        assert_eq!(rating.total(), *total, "档 {id} 总分不符");
        assert_eq!(rating.band(), *band, "档 {id} 档带不符");
    }
    // 定稿 §4.4 口径修正：E4 传导须 3 条（材料名词 + inventory 批量档 + economy 回收档），
    // 0a 夹具的 2 条是骨架非全集。
    let e4 = &equipment.heaviness.tiers[4];
    assert_eq!(
        e4.inductions.len(),
        3,
        "装备 E4 应按定稿 §4.4 声明 3 条传导，实际 {} 条",
        e4.inductions.len()
    );
}

/// 传导咬合：Module 目标在库内存在且 min_tier 是目标模块真实档 id；
/// NounProvided 名词被库内某模块 provides（析取语义的最低要求：至少一个源）。
#[test]
fn inductions_interlock_across_module_library() {
    let modules = load_all_modules();
    let provided_nouns: BTreeSet<&str> = modules
        .values()
        .flat_map(|module| module.interface.provides.iter().map(String::as_str))
        .collect();
    for (id, module) in &modules {
        for tier in &module.heaviness.tiers {
            for induction in &tier.inductions {
                match &induction.target {
                    InductionTarget::Module(target_id) => {
                        let target = modules.get(target_id).unwrap_or_else(|| {
                            panic!("{id} 档 {} 传导点名的模块 {target_id} 不在库内", tier.id)
                        });
                        assert!(
                            target.heaviness.tier_rank(&induction.min_tier).is_some(),
                            "{id} 档 {} 传导要求 {target_id} ≥ {}，但该档不在目标模块阶梯中",
                            tier.id,
                            induction.min_tier
                        );
                    }
                    InductionTarget::NounProvided(noun) => {
                        assert!(
                            provided_nouns.contains(noun.as_str()),
                            "{id} 档 {} 传导点名的名词 {noun} 无任何模块 provides",
                            tier.id
                        );
                    }
                }
            }
        }
    }
}

/// 跨模块名词引用闭合：接口里带点号的外部名词（sys.<mod>.<noun>）——
/// 提供方模块在库内时，其裸名词必须真的在该模块 provides 里。
#[test]
fn dotted_interface_nouns_resolve_to_real_providers() {
    let modules = load_all_modules();
    for (id, module) in &modules {
        let ports = [
            ("provides", &module.interface.provides),
            ("consumes", &module.interface.consumes),
            ("modifies", &module.interface.modifies),
        ];
        for (port, nouns) in ports {
            for noun in nouns {
                let Some((provider_id, bare)) = noun.rsplit_once('.') else {
                    continue;
                };
                // 提供方不在首批库内（如未来的 sys.shop）不拦——加载器 V6 的职责；
                // 在库内则裸名词必须真实存在，拦下点号后段拼写漂移。
                if let Some(provider) = modules.get(provider_id) {
                    assert!(
                        provider.interface.provides.iter().any(|p| p == bare),
                        "{id} 接口 {port} 引用 {noun}，但 {provider_id} 未 provides {bare}"
                    );
                }
            }
        }
    }
}

/// 内容红线：每个 L4 机制点的每个选项必须带非空 effects_template
/// （C0 纪律「缺模板即阻塞」的入库前置），且选项 ≥2、summary 非空。
#[test]
fn every_l4_option_carries_effects_template() {
    for (id, module) in load_all_modules() {
        for point in &module.decision_points {
            if point.level != DesignLevel::L4 {
                continue;
            }
            assert!(
                point.options.len() >= 2,
                "{id} 决策点 {} 选项少于 2 个（真设计问题必须有取舍）",
                point.id
            );
            for option in &point.options {
                assert!(
                    !option.summary.trim().is_empty(),
                    "{id} 点 {} 选项 {} 缺 summary",
                    point.id,
                    option.id
                );
                assert!(
                    !option.effects_template.is_empty(),
                    "{id} 点 {} 选项 {} 的 effects_template 为空（L4 机制点必带效果模板）",
                    point.id,
                    option.id
                );
            }
        }
    }
}
