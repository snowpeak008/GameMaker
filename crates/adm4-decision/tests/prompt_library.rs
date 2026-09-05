//! PromptLibrary 全库校验（T-W7-3f 种子化，T-W7-4c 全量填充）：
//! 遍历 prompt_library 目录全部 JSON——形状可读、总量 ≤300、全库 id 唯一、
//! source_ref 可溯源到 universal 层真实决策点、domain 非空；
//! 另加去重断言：question_zh 前 12 字符全库无重复，防同题微调刷量。

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use adm4_decision::PromptLibrary;

fn knowledge_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

/// 裸读 universal 层全部 JSON，收集 decision_points[].id 集合。
/// 故意不依赖 adm4-space：本测试只关心「id 是否存在」，用 serde_json::Value
/// 扫描即可，避免把提示词库校验绑死在空间层类型演化上。
fn universal_decision_point_ids() -> BTreeSet<String> {
    let dir = knowledge_root().join("design_space").join("universal");
    let mut ids = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("universal 目录应可读") {
        let path = entry.expect("目录项应可读").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} 应可读：{e}", path.display()));
        let value: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("{} 应为合法 JSON：{e}", path.display()));
        let Some(points) = value.get("decision_points").and_then(|p| p.as_array()) else {
            continue;
        };
        for point in points {
            if let Some(id) = point.get("id").and_then(|i| i.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }
    assert!(!ids.is_empty(), "universal 层应至少扫出一个决策点 id");
    ids
}

/// 遍历 prompt_library 目录全部 JSON，按文件名排序合并加载。
/// 返回 (文件名, 库) 列表，供各断言共用同一份加载逻辑。
fn load_all_libraries() -> Vec<(String, PromptLibrary)> {
    let dir = knowledge_root().join("prompt_library");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("prompt_library 目录应可读")
        .map(|e| e.expect("目录项应可读").path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "prompt_library 目录应至少有一个 JSON");

    files
        .into_iter()
        .map(|path| {
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{} 应可读：{e}", path.display()));
            let library: PromptLibrary = serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("{} 应反序列化为 PromptLibrary：{e}", path.display()));
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("文件名应可读")
                .to_string();
            (name, library)
        })
        .collect()
}

/// W7 定稿 §7.2：v2 2575 点降级为访谈提示词库，去重聚类后全库上限 300 条。
/// 宁缺勿滥是硬红线——上限断死，不满不罚。
#[test]
fn whole_library_stays_within_300_entries() {
    let total: usize = load_all_libraries()
        .iter()
        .map(|(_, l)| l.entries.len())
        .sum();
    assert!(
        total <= 300,
        "全库应 ≤300 条（含 seed），实际 {total} 条——超量说明去重聚类没做干净"
    );
    assert!(
        total >= 30,
        "全库应至少覆盖种子库规模（30 条），实际 {total} 条"
    );
}

/// 全库结构与可溯源性：id 唯一、domain/question 非空、follow_ups 1-3 条、
/// source_ref 必须 `v2:` 前缀且指向 universal 层真实决策点——问句不是编的。
#[test]
fn every_entry_is_wellformed_and_traceable() {
    let universal_ids = universal_decision_point_ids();
    let mut seen_ids: BTreeMap<String, String> = BTreeMap::new();

    for (file, library) in load_all_libraries() {
        for entry in &library.entries {
            assert!(!entry.id.is_empty(), "{file}：存在空 id 的条目");
            if let Some(prev) = seen_ids.insert(entry.id.clone(), file.clone()) {
                panic!("id 跨库重复：{}（{prev} 与 {file}）", entry.id);
            }
            assert!(!entry.domain.is_empty(), "{}：domain 为空", entry.id);
            assert!(
                !entry.question_zh.is_empty(),
                "{}：question_zh 为空",
                entry.id
            );
            assert!(
                (1..=3).contains(&entry.follow_ups.len()),
                "{}：follow_ups 应为 1-3 条，实际 {} 条",
                entry.id,
                entry.follow_ups.len()
            );
            for (i, follow_up) in entry.follow_ups.iter().enumerate() {
                assert!(!follow_up.is_empty(), "{}：第 {i} 条追问为空", entry.id);
            }

            let point_id = entry.source_ref.strip_prefix("v2:").unwrap_or_else(|| {
                panic!(
                    "{}：source_ref 应以 v2: 开头，实际 {}",
                    entry.id, entry.source_ref
                )
            });
            assert!(
                universal_ids.contains(point_id),
                "{}：source_ref 指向的决策点 {point_id} 不存在于 universal 层（问句必须可溯源）",
                entry.id
            );
        }
    }
}

/// 去重断言：question_zh 前 12 字符全库无重复。
/// 同题微调（换个量词再交一条）是 v2 模板冲压的老病，这里断死。
#[test]
fn question_prefixes_are_unique_across_library() {
    let mut prefixes: BTreeMap<String, String> = BTreeMap::new();
    for (_, library) in load_all_libraries() {
        for entry in &library.entries {
            let prefix: String = entry.question_zh.chars().take(12).collect();
            if let Some(prev) = prefixes.insert(prefix.clone(), entry.id.clone()) {
                panic!(
                    "question_zh 前 12 字符重复：{prev} 与 {}（『{prefix}…』）——疑似同题微调刷量",
                    entry.id
                );
            }
        }
    }
}

/// 覆盖断言：11 个系统模块域各 ≥5 条——机制访谈（3d）按 domain 取弹药，
/// 任何一个在库模块弹药不足都会让访谈空转。
#[test]
fn every_system_module_domain_has_at_least_five_entries() {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (_, library) in load_all_libraries() {
        for entry in &library.entries {
            *counts.entry(entry.domain.clone()).or_default() += 1;
        }
    }
    for domain in [
        "sys.equipment",
        "sys.inventory",
        "sys.loot",
        "sys.economy",
        "sys.build_placement",
        "sys.match_format",
        "sys.tactical_board",
        "sys.squad_command",
        "sys.scoring_combo",
        "sys.class_archetype",
        "sys.onboarding",
    ] {
        let n = counts.get(domain).copied().unwrap_or(0);
        assert!(n >= 5, "系统模块域 {domain} 应 ≥5 条弹药，实际 {n} 条");
    }
}
