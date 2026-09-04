//! T-W7-3f 种子库结构自验：条数 20-30、id 唯一、必填字段非空、
//! follow_ups 每条 ≥2、首批四模块（装备/背包/掉落/货币）各 ≥3 条弹药。
//!
//! 与 `prompt_library.rs`（可溯源性校验）互补：本文件只管结构与覆盖面。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use adm4_decision::PromptLibrary;

fn seed_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
        .join("prompt_library")
        .join("seed.json")
}

fn load_seed() -> PromptLibrary {
    let path = seed_path();
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 应可读：{e}", path.display()));
    serde_json::from_str(&raw).expect("seed.json 应反序列化为 PromptLibrary")
}

#[test]
fn seed_has_20_to_30_entries_with_unique_ids() {
    let library = load_seed();
    let n = library.entries.len();
    assert!((20..=30).contains(&n), "种子库应为 20-30 条，实际 {n} 条");

    let mut seen = BTreeSet::new();
    for entry in &library.entries {
        assert!(seen.insert(entry.id.as_str()), "id 重复：{}", entry.id);
    }
}

#[test]
fn every_entry_has_required_fields_and_enough_follow_ups() {
    let library = load_seed();
    for entry in &library.entries {
        assert!(!entry.id.is_empty(), "存在空 id 的条目");
        assert!(!entry.domain.is_empty(), "{}：domain 为空", entry.id);
        assert!(
            !entry.question_zh.is_empty(),
            "{}：question_zh 为空",
            entry.id
        );
        assert!(
            !entry.source_ref.is_empty(),
            "{}：source_ref 为空",
            entry.id
        );
        assert!(
            entry.follow_ups.len() >= 2,
            "{}：follow_ups 应 ≥2 条，实际 {} 条",
            entry.id,
            entry.follow_ups.len()
        );
        for (i, follow_up) in entry.follow_ups.iter().enumerate() {
            assert!(!follow_up.is_empty(), "{}：第 {i} 条追问为空", entry.id);
        }
    }
}

#[test]
fn first_batch_modules_each_have_at_least_three_entries() {
    let library = load_seed();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &library.entries {
        *counts.entry(entry.domain.as_str()).or_default() += 1;
    }
    for domain in ["sys.equipment", "sys.inventory", "sys.loot", "sys.economy"] {
        let n = counts.get(domain).copied().unwrap_or(0);
        assert!(
            n >= 3,
            "首批模块 {domain} 应 ≥3 条弹药，实际 {n} 条（3d 访谈不能没子弹）"
        );
    }
}
