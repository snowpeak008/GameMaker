//! PromptLibrary 种子库（T-W7-3f）的永久校验：形状可读、id 唯一、
//! 每条 source_ref 可溯源到 universal 层真实决策点——防止弹药库退化成编造问句。

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
/// 扫描即可，避免把种子库校验绑死在空间层类型演化上。
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

#[test]
fn seed_library_is_wellformed_and_traceable() {
    let seed_path = knowledge_root().join("prompt_library").join("seed.json");
    let raw = std::fs::read_to_string(&seed_path)
        .unwrap_or_else(|e| panic!("{} 应可读：{e}", seed_path.display()));
    let library: PromptLibrary =
        serde_json::from_str(&raw).expect("seed.json 应反序列化为 PromptLibrary");

    assert!(
        library.entries.len() >= 15,
        "种子库应至少 15 条，实际 {} 条",
        library.entries.len()
    );

    let mut seen = BTreeSet::new();
    let universal_ids = universal_decision_point_ids();
    for entry in &library.entries {
        assert!(!entry.id.is_empty(), "存在空 id 的条目");
        assert!(seen.insert(entry.id.as_str()), "id 重复：{}", entry.id);
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
