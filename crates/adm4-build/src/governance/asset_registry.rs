//! 资产表（`asset_registry.json`）：美术线的**命名权威**（册 07 铁律②）。
//!
//! 这是「稳定 `asset_id` → 文件名 → 运行时加载路径」这条链的单点锚定处：生产端不得自由起名，
//! 装配端不得另立路径。命名规范的**机制**（类型前缀 + 骨架分段 + 禁止词根扫描）写在代码里，
//! 具体**内容**（阵营/磨损等词表）由品类包提供——册 07 §6 的「保留机制、下放内容」。

use super::require_non_blank;
use adm4_contracts::SpecRef;
use adm4_foundation::{Adm4Error, Adm4Result, ensure_within_root};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// 资产在流水线中的生命周期（py `AlignmentSchema.asset_state`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetLifecycleState {
    #[default]
    Draft,
    Generated,
    Reviewed,
    Approved,
    Integrated,
    Deprecated,
    Archived,
}

impl AssetLifecycleState {
    /// 是否可被程序/装配端正式使用（只有 approved / integrated）。
    pub fn usable_downstream(self) -> bool {
        matches!(self, Self::Approved | Self::Integrated)
    }
}

/// 稳定性档位（py `AlignmentSchema.stability_levels`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StabilityLevel {
    #[default]
    Experimental,
    Stable,
    Frozen,
}

/// 命名规范的**机制**部分。
///
/// `forbidden_roots` 默认空表：禁止词根是某个品类包的内容（塔防的阵营词、磨损词……），
/// 写死在工程里会让别的品类莫名其妙被拦（册 07 §6 的下放内容）。调用方从品类包读进来传入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NamingRules {
    /// 类型前缀白名单（`SM_` 静态模型 / `SK_` 骨骼 / `T_` 贴图 / `M_` 材质 / `UI_` / `VFX_`）。
    pub type_prefixes: Vec<String>,
    /// 禁止词根（接 R5 换皮词表，由品类包供给）。
    pub forbidden_roots: Vec<String>,
    /// `{AssetType}_{Subject}[_{Qualifier}...]` 骨架的最小分段数。
    pub min_segments: usize,
}

impl Default for NamingRules {
    fn default() -> Self {
        Self {
            type_prefixes: ["SM_", "SK_", "T_", "M_", "UI_", "VFX_"]
                .iter()
                .map(|prefix| (*prefix).to_string())
                .collect(),
            forbidden_roots: Vec::new(),
            min_segments: 2,
        }
    }
}

/// 命名违规的机器码（呈现层按码分类，不靠解析中文文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamingViolationCode {
    /// 缺类型前缀。
    MissingTypePrefix,
    /// 骨架分段不足。
    TooFewSegments,
    /// 命中禁止词根。
    ForbiddenRoot,
    /// 文件名模式与 `asset_id` 的词法变体对不上（链断在命名这一环）。
    PatternNotDerivedFromId,
}

/// 一条命名违规。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NamingViolation {
    pub asset_id: String,
    pub code: NamingViolationCode,
    pub detail: String,
}

impl Default for NamingViolation {
    fn default() -> Self {
        Self {
            asset_id: String::new(),
            code: NamingViolationCode::MissingTypePrefix,
            detail: String::new(),
        }
    }
}

/// 把 `asset_id` 折算成文件名侧的词法变体：小写 + `-`/空格 → `_`。
///
/// 册 07 §5 的原话是「id 与文件名是同一标识的词法变体（`ART-ILL-0001` ↔ `art_ill_0001`）」，
/// 这个函数就是那条折算规则的唯一实现——资产表与 `AssetGenome` 共用它，避免两处各折一套。
pub fn lexical_variant(asset_id: &str) -> String {
    asset_id
        .chars()
        .map(|character| match character {
            '-' | ' ' => '_',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

/// 资产表条目。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetRegistryEntry {
    pub asset_id: String,
    /// 文件名模式（如 `ui_playeridle_{frame:03d}.png`）。
    pub naming_pattern: String,
    /// 运行时加载路径（`AssetGenome` 回填时必须与实际落盘路径一致，否则即 drift）。
    pub runtime_path: String,
    pub state: AssetLifecycleState,
    pub stability: StabilityLevel,
    pub source_refs: Vec<SpecRef>,
}

/// 资产表（命名权威的中央清单）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetRegistry {
    pub schema_version: String,
    pub entries: Vec<AssetRegistryEntry>,
}

impl AssetRegistry {
    pub fn entry(&self, asset_id: &str) -> Option<&AssetRegistryEntry> {
        self.entries.iter().find(|entry| entry.asset_id == asset_id)
    }

    /// 取条目，缺登记即 `Err`。
    ///
    /// 命名权威的意义就在这里：一个没在中央清单登记过的 `asset_id`，下游拿不到文件名也拿不到
    /// 运行时路径——此时**必须停**，而不是就地编一个名字接着跑（R2）。
    pub fn require_entry(&self, asset_id: &str) -> Adm4Result<&AssetRegistryEntry> {
        self.entry(asset_id).ok_or_else(|| {
            Adm4Error::not_found(format!(
                "资产 {asset_id} 未在资产表登记：命名权威缺登记，下游不得自行起名（铁律②）"
            ))
        })
    }

    /// 结构校验：id 唯一非空、文件名模式与运行时路径在案且不越界。
    pub fn validate(&self) -> Adm4Result<()> {
        let mut seen = BTreeSet::new();
        for entry in &self.entries {
            require_non_blank(&entry.asset_id, "资产表条目 asset_id")?;
            if !seen.insert(entry.asset_id.as_str()) {
                return Err(Adm4Error::conflict(format!(
                    "资产表登记了重复的 asset_id {}：命名权威必须单点锚定",
                    entry.asset_id
                )));
            }
            require_non_blank(
                &entry.naming_pattern,
                &format!("资产 {} 的文件名模式", entry.asset_id),
            )?;
            require_non_blank(
                &entry.runtime_path,
                &format!("资产 {} 的运行时加载路径", entry.asset_id),
            )?;
            // 运行时路径最终会被拼到工程目录下：带 `..`/盘符的路径能写出目录之外，先拦住。
            ensure_within_root(Path::new(&entry.runtime_path))?;
        }
        Ok(())
    }

    /// 命名规范核对（结构化报告，不直接失败）。
    ///
    /// 为什么是报告而不是 `Err`：命名违规要**逐条**摆给人看（哪个资产错在哪一条），
    /// 一个 `Err` 只能带走第一条。放行与否由调用方的门决定（G3 的生产前清单人工门）。
    pub fn naming_violations(&self, rules: &NamingRules) -> Vec<NamingViolation> {
        let mut violations = Vec::new();
        for entry in &self.entries {
            let asset_id = entry.asset_id.as_str();
            if !rules
                .type_prefixes
                .iter()
                .any(|prefix| asset_id.starts_with(prefix.as_str()))
            {
                violations.push(NamingViolation {
                    asset_id: asset_id.to_string(),
                    code: NamingViolationCode::MissingTypePrefix,
                    detail: format!("缺类型前缀（允许：{}）", rules.type_prefixes.join(" / ")),
                });
            }
            if asset_id.split('_').filter(|part| !part.is_empty()).count() < rules.min_segments {
                violations.push(NamingViolation {
                    asset_id: asset_id.to_string(),
                    code: NamingViolationCode::TooFewSegments,
                    detail: format!(
                        "命名骨架至少 {} 段（{{AssetType}}_{{Subject}}…）",
                        rules.min_segments
                    ),
                });
            }
            let lowered = asset_id.to_ascii_lowercase();
            for root in &rules.forbidden_roots {
                let needle = root.to_ascii_lowercase();
                if !needle.is_empty() && lowered.contains(&needle) {
                    violations.push(NamingViolation {
                        asset_id: asset_id.to_string(),
                        code: NamingViolationCode::ForbiddenRoot,
                        detail: format!("命中禁止词根 {root}"),
                    });
                }
            }
            let variant = lexical_variant(asset_id);
            if !entry.naming_pattern.to_ascii_lowercase().contains(&variant) {
                violations.push(NamingViolation {
                    asset_id: asset_id.to_string(),
                    code: NamingViolationCode::PatternNotDerivedFromId,
                    detail: format!(
                        "文件名模式 {} 不含 asset_id 的词法变体 {variant}（id→文件名的链断了）",
                        entry.naming_pattern
                    ),
                });
            }
        }
        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(asset_id: &str, pattern: &str, path: &str) -> AssetRegistryEntry {
        AssetRegistryEntry {
            asset_id: asset_id.into(),
            naming_pattern: pattern.into(),
            runtime_path: path.into(),
            state: AssetLifecycleState::Approved,
            stability: StabilityLevel::Stable,
            source_refs: vec![SpecRef::new("entities/guard")],
        }
    }

    fn sample() -> AssetRegistry {
        AssetRegistry {
            schema_version: "4.0.0".into(),
            entries: vec![entry(
                "UI_PlayerIdle",
                "ui_playeridle_{frame:03d}.png",
                "Assets/Art/UI/ui_playeridle_001.png",
            )],
        }
    }

    #[test]
    fn asset_registry_round_trips_through_json() {
        let registry = sample();
        let json = serde_json::to_string_pretty(&registry).expect("序列化");
        let back: AssetRegistry = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, registry);
        assert!(back.validate().is_ok());
    }

    /// 旧档兼容：没有 state/stability/source_refs 的历史条目照旧可读（落最保守的默认档）。
    #[test]
    fn legacy_registry_entry_without_new_fields_parses() {
        let legacy = r#"{
          "schema_version": "3.0",
          "entries": [{
            "asset_id": "UI_PlayerIdle",
            "naming_pattern": "ui_playeridle_{frame:03d}.png",
            "runtime_path": "Assets/Art/UI/ui_playeridle_001.png"
          }]
        }"#;
        let parsed: AssetRegistry = serde_json::from_str(legacy).expect("旧档应可解析");
        let entry = parsed.entry("UI_PlayerIdle").expect("条目在案");
        assert_eq!(entry.state, AssetLifecycleState::Draft);
        assert!(
            !entry.state.usable_downstream(),
            "旧档没写状态时按草稿读，不许被当成已批准直接用"
        );
        assert_eq!(entry.stability, StabilityLevel::Experimental);
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn missing_registration_is_an_error_not_a_guess() {
        let registry = sample();
        let error = registry.require_entry("UI_Nowhere").unwrap_err();
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::NotFound);
        assert!(error.message.contains("命名权威"), "{}", error.message);
    }

    #[test]
    fn duplicate_ids_and_escaping_paths_are_rejected() {
        let mut registry = sample();
        registry.entries.push(entry(
            "UI_PlayerIdle",
            "ui_playeridle_{frame:03d}.png",
            "Assets/other.png",
        ));
        assert_eq!(
            registry.validate().unwrap_err().kind,
            adm4_foundation::Adm4ErrorKind::Conflict
        );

        let mut registry = sample();
        registry.entries[0].runtime_path = "../outside/ui.png".into();
        assert_eq!(
            registry.validate().unwrap_err().kind,
            adm4_foundation::Adm4ErrorKind::PathEscape
        );
    }

    #[test]
    fn naming_violations_report_mechanism_not_pack_content() {
        // 默认规则只管机制：类型前缀 + 分段 + id→文件名链，禁止词根表为空。
        let rules = NamingRules::default();
        assert!(sample().naming_violations(&rules).is_empty());

        let bad = AssetRegistry {
            schema_version: "4.0.0".into(),
            entries: vec![entry("hero", "sprite.png", "Assets/sprite.png")],
        };
        let codes: Vec<NamingViolationCode> = bad
            .naming_violations(&rules)
            .into_iter()
            .map(|violation| violation.code)
            .collect();
        assert!(codes.contains(&NamingViolationCode::MissingTypePrefix));
        assert!(codes.contains(&NamingViolationCode::TooFewSegments));
        assert!(codes.contains(&NamingViolationCode::PatternNotDerivedFromId));

        // 禁止词根来自品类包：不传就不拦，传了才拦（内容下放，不写死在工程里）。
        let pack_rules = NamingRules {
            forbidden_roots: vec!["Holo".into()],
            ..NamingRules::default()
        };
        let holo = AssetRegistry {
            schema_version: "4.0.0".into(),
            entries: vec![entry(
                "UI_HoloPanel",
                "ui_holopanel.png",
                "Assets/ui_holopanel.png",
            )],
        };
        assert!(holo.naming_violations(&NamingRules::default()).is_empty());
        let hits = holo.naming_violations(&pack_rules);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].code, NamingViolationCode::ForbiddenRoot);
    }

    #[test]
    fn lexical_variant_matches_the_documented_rule() {
        assert_eq!(lexical_variant("ART-ILL-0001"), "art_ill_0001");
        assert_eq!(lexical_variant("UI_PlayerIdle"), "ui_playeridle");
    }
}
