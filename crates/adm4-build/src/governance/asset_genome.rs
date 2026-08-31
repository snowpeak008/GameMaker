//! AssetGenome 回填（册 07 §5）：设计资产 id ↔ 磁盘上的实际文件。
//!
//! 这里闭合的是 godogen 的那条硬约束——**「AssetGenome 记录的 path 必须等于运行时真正加载的
//! path」**。资产表（命名权威）说资产该在哪儿，基因表说资产实际在哪儿，两者对不上就是漂移；
//! 对账在 [`AssetGenome::verify_runtime_paths`] 里做，确定性、不经 AI。

use super::asset_registry::{AssetRegistry, lexical_variant};
use super::{AssetSize, require_non_blank};
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 一条回填记录。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GenomeEntry {
    /// 设计侧资产 id（如 `ART-ILL-0001-asset_001`）。
    pub id: String,
    /// 实际落盘文件（相对工程根；与运行时加载路径同一个值）。
    pub files: Vec<String>,
    pub created_at: String,
    /// 实际入游尺寸（未测得就是 None——不拿规格书上的尺寸冒充实测值，R1）。
    pub in_game_size: Option<AssetSize>,
    /// 谁在用它（程序线的 `dependency_id`）。
    pub used_by: Vec<String>,
}

/// 资产基因表（`asset_genome.json`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGenome {
    pub schema_version: String,
    pub assets: Vec<GenomeEntry>,
}

/// 一条基因-资产表对账差异。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenomeDrift {
    pub asset_id: String,
    pub kind: GenomeDriftKind,
    pub detail: String,
}

impl Default for GenomeDrift {
    fn default() -> Self {
        Self {
            asset_id: String::new(),
            kind: GenomeDriftKind::NotRegistered,
            detail: String::new(),
        }
    }
}

/// 对账差异的机器码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenomeDriftKind {
    /// 产出了一个资产表里没有的资产（生产端自己发明了资产，违铁律②）。
    NotRegistered,
    /// 实际文件里找不到资产表登记的运行时加载路径。
    RuntimePathMismatch,
    /// 资产表登记了但一次都没产出（生产未完成，不算漂移的反面：也必须可见）。
    NotProduced,
}

impl AssetGenome {
    pub fn entry(&self, id: &str) -> Option<&GenomeEntry> {
        self.assets.iter().find(|entry| entry.id == id)
    }

    /// 回填一条记录（生产端每完成一个资产调一次；同 id 覆盖，支持重跑）。
    ///
    /// 三条硬校验：id 非空、至少一个文件、文件名必须是 id 的词法变体。
    /// 最后一条是铁律②的收口——一路贯穿的命名链，最后一环断在这里就等于白治理。
    pub fn backfill(&mut self, entry: GenomeEntry) -> Adm4Result<()> {
        require_non_blank(&entry.id, "AssetGenome 记录 id")?;
        if entry.files.is_empty() {
            return Err(Adm4Error::validation(format!(
                "资产 {} 的回填记录没有任何文件：产出为空就不该回填（R2）",
                entry.id
            )));
        }
        let variant = lexical_variant(&entry.id);
        for file in &entry.files {
            let stem = file_stem(file);
            if !stem.starts_with(&variant) {
                return Err(Adm4Error::validation(format!(
                    "资产 {} 的文件 {file} 与 id 的词法变体 {variant} 对不上：\
                     id→文件名必须一路贯穿（铁律②）",
                    entry.id
                )));
            }
        }
        match self
            .assets
            .iter_mut()
            .find(|existing| existing.id == entry.id)
        {
            Some(existing) => *existing = entry,
            None => self.assets.push(entry),
        }
        Ok(())
    }

    /// 与资产表对账：产出的路径必须等于登记的运行时加载路径。
    ///
    /// 返回**全部**差异（不是遇到第一条就返回）：一次生产可能有几十个资产错位，
    /// 逐条列出来才能一次修完。
    pub fn verify_runtime_paths(&self, registry: &AssetRegistry) -> Vec<GenomeDrift> {
        let mut drifts = Vec::new();
        let mut produced = BTreeSet::new();
        for entry in &self.assets {
            produced.insert(entry.id.as_str());
            let Some(registered) = registry.entry(&entry.id) else {
                drifts.push(GenomeDrift {
                    asset_id: entry.id.clone(),
                    kind: GenomeDriftKind::NotRegistered,
                    detail: "产出的资产不在资产表内：生产端不得自行发明资产".to_string(),
                });
                continue;
            };
            let wanted = normalize_path(&registered.runtime_path);
            if !entry
                .files
                .iter()
                .any(|file| normalize_path(file) == wanted)
            {
                drifts.push(GenomeDrift {
                    asset_id: entry.id.clone(),
                    kind: GenomeDriftKind::RuntimePathMismatch,
                    detail: format!(
                        "资产表登记的运行时加载路径 {} 不在实际产出文件 [{}] 内",
                        registered.runtime_path,
                        entry.files.join(", ")
                    ),
                });
            }
        }
        for registered in &registry.entries {
            if !produced.contains(registered.asset_id.as_str()) {
                drifts.push(GenomeDrift {
                    asset_id: registered.asset_id.clone(),
                    kind: GenomeDriftKind::NotProduced,
                    detail: "资产表登记在案但从未回填产出记录".to_string(),
                });
            }
        }
        drifts
    }

    /// 一行摘要（R1：报计数而不是「基本对齐」这类说法）。
    pub fn summary(&self, registry: &AssetRegistry) -> String {
        let drifts = self.verify_runtime_paths(registry);
        format!(
            "回填 {} 个资产，资产表登记 {} 个，路径对账差异 {} 条",
            self.assets.len(),
            registry.entries.len(),
            drifts.len()
        )
    }
}

/// 取文件名主干（去目录与扩展名）：`a/b/ui_x_001.png` → `ui_x_001`。
fn file_stem(path: &str) -> String {
    let normalized = normalize_path(path);
    let file_name = match normalized.rsplit_once('/') {
        Some((_, name)) => name,
        None => normalized.as_str(),
    };
    match file_name.rsplit_once('.') {
        Some((stem, _)) => stem.to_ascii_lowercase(),
        None => file_name.to_ascii_lowercase(),
    }
}

/// 路径归一：反斜杠统一成正斜杠（Windows 产出的路径与契约里的写法要能对上）。
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::asset_registry::{
        AssetLifecycleState, AssetRegistryEntry, StabilityLevel,
    };

    fn registry() -> AssetRegistry {
        AssetRegistry {
            schema_version: "4.0.0".into(),
            entries: vec![AssetRegistryEntry {
                asset_id: "ART-ILL-0001".into(),
                naming_pattern: "art_ill_0001.png".into(),
                runtime_path: "ArtAssets/Illustrations/art_ill_0001.png".into(),
                state: AssetLifecycleState::Integrated,
                stability: StabilityLevel::Stable,
                source_refs: Vec::new(),
            }],
        }
    }

    fn produced() -> GenomeEntry {
        GenomeEntry {
            id: "ART-ILL-0001".into(),
            files: vec!["ArtAssets/Illustrations/art_ill_0001.png".into()],
            created_at: "2026-08-31T00:00:00Z".into(),
            in_game_size: Some(AssetSize::new(512, 512)),
            used_by: vec!["hero_controller.portrait".into()],
        }
    }

    #[test]
    fn genome_round_trips_and_backfill_is_idempotent_per_id() {
        let mut genome = AssetGenome {
            schema_version: "4.0.0".into(),
            assets: Vec::new(),
        };
        genome.backfill(produced()).expect("首次回填");
        let mut second = produced();
        second.used_by.push("hud.portrait".into());
        genome.backfill(second.clone()).expect("重跑覆盖同 id");
        assert_eq!(genome.assets.len(), 1, "同 id 覆盖而不是追加第二条");
        assert_eq!(genome.entry("ART-ILL-0001"), Some(&second));

        let json = serde_json::to_string_pretty(&genome).expect("序列化");
        let back: AssetGenome = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, genome);
    }

    /// 旧档兼容：只有 id/files/created_at 的历史记录（正是 py 的 AssetGenome 形态）必须能读。
    #[test]
    fn legacy_genome_without_new_fields_parses() {
        let legacy = r#"{
          "assets": [{
            "id": "ART-ILL-0001-asset_001",
            "files": ["outputs/runtime/ArtAssets/Illustrations/art_ill_0001_asset_001.png"],
            "created_at": "2026-06-07T20:40:38"
          }]
        }"#;
        let parsed: AssetGenome = serde_json::from_str(legacy).expect("旧档应可解析");
        assert_eq!(parsed.assets.len(), 1);
        assert_eq!(
            parsed.assets[0].in_game_size, None,
            "缺实测尺寸就是未知，不补默认值"
        );
        assert!(parsed.assets[0].used_by.is_empty());
    }

    #[test]
    fn backfill_rejects_files_that_break_the_naming_chain() {
        let mut genome = AssetGenome::default();
        let mut wrong = produced();
        wrong.files = vec!["ArtAssets/whatever.png".into()];
        let error = genome.backfill(wrong).unwrap_err();
        assert!(error.message.contains("词法变体"), "{}", error.message);

        let mut empty = produced();
        empty.files.clear();
        assert!(genome.backfill(empty).is_err(), "空产出不许回填");

        let mut blank = produced();
        blank.id = "  ".into();
        assert!(genome.backfill(blank).is_err());
    }

    #[test]
    fn runtime_path_mismatch_and_missing_production_are_both_reported() {
        let registry = registry();

        let mut genome = AssetGenome::default();
        genome.backfill(produced()).expect("回填");
        assert!(
            genome.verify_runtime_paths(&registry).is_empty(),
            "路径一致时不该报漂移"
        );
        assert_eq!(
            genome.summary(&registry),
            "回填 1 个资产，资产表登记 1 个，路径对账差异 0 条"
        );

        // 产到了别的目录：运行时按资产表的路径去加载会加载不到 → 漂移。
        let mut moved = AssetGenome::default();
        let mut entry = produced();
        entry.files = vec!["ArtAssets/Elsewhere/art_ill_0001.png".into()];
        moved.backfill(entry).expect("回填");
        let drifts = moved.verify_runtime_paths(&registry);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].kind, GenomeDriftKind::RuntimePathMismatch);

        // 一次都没产：也必须可见（不是「没漂移」）。
        let drifts = AssetGenome::default().verify_runtime_paths(&registry);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].kind, GenomeDriftKind::NotProduced);

        // 产了资产表里没有的东西：生产端发明资产，同样拦下。
        let mut invented = AssetGenome::default();
        invented
            .backfill(GenomeEntry {
                id: "ART-ILL-9999".into(),
                files: vec!["ArtAssets/art_ill_9999.png".into()],
                created_at: "2026-08-31T00:00:00Z".into(),
                in_game_size: None,
                used_by: Vec::new(),
            })
            .expect("回填");
        let kinds: Vec<GenomeDriftKind> = invented
            .verify_runtime_paths(&registry)
            .into_iter()
            .map(|drift| drift.kind)
            .collect();
        assert!(kinds.contains(&GenomeDriftKind::NotRegistered));
        assert!(kinds.contains(&GenomeDriftKind::NotProduced));
    }

    /// Windows 反斜杠与契约里的正斜杠必须对得上，否则每个资产都会被误判成漂移。
    #[test]
    fn path_comparison_normalizes_separators() {
        let mut genome = AssetGenome::default();
        let mut entry = produced();
        entry.files = vec![r"ArtAssets\Illustrations\art_ill_0001.png".into()];
        genome.backfill(entry).expect("回填");
        assert!(genome.verify_runtime_paths(&registry()).is_empty());
    }
}
