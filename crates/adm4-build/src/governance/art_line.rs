//! 美术线契约（册 07 §1）：视觉语言 / 资产 / 视觉状态 / UX 信号绑定 / 漂移检查。
//!
//! 铁律②「稳定 `asset_id` 单点锚定」在这里落成机器规则：视觉状态、UX 绑定、漂移检查的 id
//! 全部从 `asset_id` 派生（`STATE-` / `UX-` / `DRIFT-` 前缀），[`ArtContract::validate`]
//! 逐条核对前缀与归属——起错名字当场被拒，而不是等到生产端找不到文件才发现。

use super::{ART_LINE, ContractEnvelope, SpecTriple, require_non_blank};
use adm4_contracts::SpecRef;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 视觉状态 id 前缀（铁律②派生规则）。
pub const VISUAL_STATE_PREFIX: &str = "STATE-";
/// UX 信号绑定 id 前缀。
pub const UX_BINDING_PREFIX: &str = "UX-";
/// 漂移检查 id 前缀。
pub const DRIFT_CHECK_PREFIX: &str = "DRIFT-";

/// 从 `asset_id` 派生视觉状态 id（`STATE-{asset_id}-{state}`）。
pub fn visual_state_id(asset_id: &str, state: &str) -> String {
    format!("{VISUAL_STATE_PREFIX}{asset_id}-{state}")
}

/// 从 `asset_id` 派生 UX 信号绑定 id（`UX-{asset_id}-{signal}`）。
pub fn ux_binding_id(asset_id: &str, signal: &str) -> String {
    format!("{UX_BINDING_PREFIX}{asset_id}-{signal}")
}

/// 从 `asset_id` 派生漂移检查 id（`DRIFT-{asset_id}`）。
pub fn drift_check_id(asset_id: &str) -> String {
    format!("{DRIFT_CHECK_PREFIX}{asset_id}")
}

/// 资产大类（py `ArtSchema` 的 category 枚举 V4 化）。
///
/// `Unknown` 只是旧档/漏填的落点，校验时报错——见 [`crate::governance::program_line::ContractMethod`]
/// 的同款理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetCategory {
    #[default]
    Unknown,
    Illustration,
    Ui,
    Vfx,
    Animation,
    Model,
    Audio,
}

/// 漂移检查的严重度（py 协议 §5.5：OK / WARNING / BLOCK / UNKNOWN）。
///
/// 这里的 `Unknown` 是**合法结论**（「查不出来」也是一种如实结论），与其它枚举的
/// 「漏填落点」语义不同：它不会让校验失败，但也绝不算通过。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    #[default]
    Unknown,
    Ok,
    Warning,
    Block,
}

impl DriftSeverity {
    /// 是否阻断下游（只有 `Ok` 放行；`Unknown` 不算通过——查不出来不等于没问题）。
    pub fn blocks_downstream(self) -> bool {
        !matches!(self, DriftSeverity::Ok)
    }
}

/// 视觉语言：从真源与风格锚点派生的约束集合（不得发明世界观与玩法含义）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualLanguage {
    pub tokens: Vec<String>,
    pub palette: Vec<String>,
    pub forbidden_motifs: Vec<String>,
    /// 设计阶段锁定的风格锚点集标识（册 08，Phase 2 只消费不重造）。
    pub style_anchor_ref: String,
}

/// 一个稳定的美术生产目标。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtAsset {
    pub asset_id: String,
    pub name: String,
    pub category: AssetCategory,
    pub purpose: String,
    /// 美术侧提供的三要素（帧/尺寸/格式），对齐层拿它与程序侧要求核对。
    pub production_spec: SpecTriple,
    /// 文件名模式（如 `player_idle_{frame:03d}.png`）；命名权威在 `asset_registry`。
    pub naming_pattern: String,
    pub required_readability: String,
    pub forbidden_visuals: Vec<String>,
    pub acceptance_checks: Vec<String>,
    /// 真源锚点。
    pub source_refs: Vec<SpecRef>,
    /// 无真源锚点时的显式美术规则出处（两者不能同时为空，py 协议 §8 硬阻塞 9）。
    pub art_rule: String,
}

/// 资产的有状态变体。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualState {
    pub visual_state_id: String,
    pub asset_id: String,
    pub source_state_id: String,
    pub state_name: String,
    pub required_difference: String,
}

/// UX 信号 → 资产/视觉状态的绑定（保证美术真的把玩法决策传达出去）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UxSignalBinding {
    pub binding_id: String,
    pub ux_signal_id: String,
    pub asset_id: String,
    pub required_feedback: String,
    pub timing: String,
}

/// 视觉漂移检查（比对基准 = 设计阶段锁定的风格锚点集）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DriftCheck {
    pub check_id: String,
    pub asset_id: String,
    pub severity: DriftSeverity,
    pub detail: String,
    /// 判定证据（R1：结论必须带证据，不接受裸结论）。
    pub evidence: Vec<String>,
}

/// 美术线机器契约（`art_contract.json`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtContract {
    pub envelope: ContractEnvelope,
    pub visual_language: VisualLanguage,
    pub assets: Vec<ArtAsset>,
    pub visual_states: Vec<VisualState>,
    pub ux_signal_bindings: Vec<UxSignalBinding>,
    pub drift_checks: Vec<DriftCheck>,
}

impl ArtContract {
    /// 确定性自校验：asset_id 唯一、派生 id 前缀正确、无悬空归属、每个资产有出处。
    pub fn validate(&self) -> Adm4Result<()> {
        self.envelope.validate(ART_LINE)?;
        let mut asset_ids = BTreeSet::new();
        for asset in &self.assets {
            require_non_blank(&asset.asset_id, "美术资产 asset_id")?;
            if !asset_ids.insert(asset.asset_id.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "美术资产 id 重复：{}（asset_id 是全线单点锚定标识）",
                    asset.asset_id
                )));
            }
            if asset.category == AssetCategory::Unknown {
                return Err(Adm4Error::validation(format!(
                    "美术资产 {} 未声明大类（illustration/ui/vfx/animation/model/audio）",
                    asset.asset_id
                )));
            }
            if asset.source_refs.is_empty() && asset.art_rule.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "美术资产 {} 既无真源锚点也无显式美术规则出处：下游不得凭空发明资产（铁律①）",
                    asset.asset_id
                )));
            }
        }
        for state in &self.visual_states {
            Self::require_known_asset(&asset_ids, &state.asset_id, "视觉状态")?;
            let expected = format!("{VISUAL_STATE_PREFIX}{}-", state.asset_id);
            if !state.visual_state_id.starts_with(&expected) {
                return Err(Adm4Error::validation(format!(
                    "视觉状态 id {} 未从 asset_id 派生（应以 {expected} 开头，铁律②）",
                    state.visual_state_id
                )));
            }
        }
        for binding in &self.ux_signal_bindings {
            Self::require_known_asset(&asset_ids, &binding.asset_id, "UX 信号绑定")?;
            let expected = format!("{UX_BINDING_PREFIX}{}-", binding.asset_id);
            if !binding.binding_id.starts_with(&expected) {
                return Err(Adm4Error::validation(format!(
                    "UX 绑定 id {} 未从 asset_id 派生（应以 {expected} 开头，铁律②）",
                    binding.binding_id
                )));
            }
        }
        for check in &self.drift_checks {
            Self::require_known_asset(&asset_ids, &check.asset_id, "漂移检查")?;
            if check.check_id != drift_check_id(&check.asset_id) {
                return Err(Adm4Error::validation(format!(
                    "漂移检查 id {} 未从 asset_id 派生（应为 {}，铁律②）",
                    check.check_id,
                    drift_check_id(&check.asset_id)
                )));
            }
        }
        Ok(())
    }

    fn require_known_asset(known: &BTreeSet<&str>, asset_id: &str, what: &str) -> Adm4Result<()> {
        if known.contains(asset_id) {
            return Ok(());
        }
        Err(Adm4Error::validation(format!(
            "{what} 挂在未注册的资产 {asset_id} 上"
        )))
    }

    pub fn asset(&self, asset_id: &str) -> Option<&ArtAsset> {
        self.assets.iter().find(|asset| asset.asset_id == asset_id)
    }

    /// 本契约声明的全部标识（权威顺序校验器用）。
    pub fn declared_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        ids.extend(self.assets.iter().map(|item| item.asset_id.clone()));
        ids.extend(
            self.visual_states
                .iter()
                .map(|item| item.visual_state_id.clone()),
        );
        ids.extend(
            self.ux_signal_bindings
                .iter()
                .map(|item| item.binding_id.clone()),
        );
        ids.extend(self.drift_checks.iter().map(|item| item.check_id.clone()));
        ids
    }

    pub fn source_refs(&self) -> Vec<SpecRef> {
        self.assets
            .iter()
            .flat_map(|asset| asset.source_refs.iter().cloned())
            .collect()
    }

    /// 阻断级漂移（`Block`）清单：BLOCK 必须路由回美术评审/风格门，不得进后续装配。
    pub fn blocking_drifts(&self) -> Vec<&DriftCheck> {
        self.drift_checks
            .iter()
            .filter(|check| check.severity == DriftSeverity::Block)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::AssetSize;

    fn sample() -> ArtContract {
        ArtContract {
            envelope: ContractEnvelope::new(ART_LINE, "2026-08-31T00:00:00Z", "sha256:frozen"),
            visual_language: VisualLanguage {
                tokens: vec!["扁平卡通".into()],
                palette: vec!["#2b2b2b".into()],
                forbidden_motifs: vec!["高魔奇幻".into()],
                style_anchor_ref: "style/anchors/v1".into(),
            },
            assets: vec![ArtAsset {
                asset_id: "UI_PlayerIdle".into(),
                name: "玩家待机".into(),
                category: AssetCategory::Animation,
                purpose: "待机状态反馈".into(),
                production_spec: SpecTriple::full(8, AssetSize::new(256, 256), "png"),
                naming_pattern: "ui_playeridle_{frame:03d}.png".into(),
                required_readability: "轮廓可辨".into(),
                forbidden_visuals: vec!["镜面高光".into()],
                acceptance_checks: vec!["八帧循环无跳变".into()],
                source_refs: vec![SpecRef::new("entities/guard")],
                art_rule: String::new(),
            }],
            visual_states: vec![VisualState {
                visual_state_id: visual_state_id("UI_PlayerIdle", "damaged"),
                asset_id: "UI_PlayerIdle".into(),
                source_state_id: "guard_damaged".into(),
                state_name: "受创".into(),
                required_difference: "描边转红".into(),
            }],
            ux_signal_bindings: vec![UxSignalBinding {
                binding_id: ux_binding_id("UI_PlayerIdle", "hurt"),
                ux_signal_id: "hurt".into(),
                asset_id: "UI_PlayerIdle".into(),
                required_feedback: "受击闪烁".into(),
                timing: "命中后 0.1s".into(),
            }],
            drift_checks: vec![DriftCheck {
                check_id: drift_check_id("UI_PlayerIdle"),
                asset_id: "UI_PlayerIdle".into(),
                severity: DriftSeverity::Ok,
                detail: "与风格锚点一致".into(),
                evidence: vec!["anchors/v1/style_01.png".into()],
            }],
        }
    }

    #[test]
    fn art_contract_round_trips_through_json() {
        let contract = sample();
        let json = serde_json::to_string_pretty(&contract).expect("序列化");
        let back: ArtContract = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, contract);
        assert!(back.validate().is_ok());
    }

    /// 旧档兼容：没有 drift_checks / art_rule 的历史契约照旧可读。
    #[test]
    fn legacy_art_contract_without_new_sections_parses() {
        let legacy = r#"{
          "envelope": {"consumer_line": "art", "source_frozen_hash": "sha256:old"},
          "assets": [{
            "asset_id": "UI_PlayerIdle",
            "name": "玩家待机",
            "category": "animation",
            "source_refs": ["entities/guard"]
          }]
        }"#;
        let parsed: ArtContract = serde_json::from_str(legacy).expect("旧档应可解析");
        assert_eq!(parsed.assets.len(), 1);
        assert!(parsed.assets[0].art_rule.is_empty());
        assert_eq!(parsed.assets[0].production_spec, SpecTriple::default());
        assert!(parsed.drift_checks.is_empty());
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn derived_ids_must_come_from_asset_id() {
        let mut contract = sample();
        contract.visual_states[0].visual_state_id = "STATE-Other-damaged".into();
        assert!(
            contract
                .validate()
                .unwrap_err()
                .message
                .contains("未从 asset_id 派生")
        );

        let mut contract = sample();
        contract.ux_signal_bindings[0].binding_id = "hurt_binding".into();
        assert!(contract.validate().is_err(), "UX 绑定 id 必须带派生前缀");

        let mut contract = sample();
        contract.drift_checks[0].check_id = "DRIFT-wrong".into();
        assert!(contract.validate().is_err(), "漂移检查 id 必须等于派生值");

        let mut contract = sample();
        contract.visual_states[0].asset_id = "UI_Missing".into();
        contract.visual_states[0].visual_state_id = visual_state_id("UI_Missing", "damaged");
        assert!(
            contract.validate().is_err(),
            "挂在未注册资产上的状态必须被拒"
        );
    }

    #[test]
    fn asset_without_source_or_art_rule_is_rejected() {
        let mut contract = sample();
        contract.assets[0].source_refs.clear();
        let error = contract.validate().unwrap_err();
        assert!(error.message.contains("凭空发明"), "{}", error.message);

        contract.assets[0].art_rule = "ArtRules 1.1 磨损原则".into();
        assert!(
            contract.validate().is_ok(),
            "有显式美术规则出处时应放行（协议允许的第二条出处）"
        );
    }

    #[test]
    fn drift_severity_only_ok_passes_downstream() {
        assert!(!DriftSeverity::Ok.blocks_downstream());
        for severity in [
            DriftSeverity::Unknown,
            DriftSeverity::Warning,
            DriftSeverity::Block,
        ] {
            assert!(
                severity.blocks_downstream(),
                "{severity:?} 不该被当成通过（查不出来不等于没问题）"
            );
        }
        let mut contract = sample();
        contract.drift_checks[0].severity = DriftSeverity::Block;
        assert_eq!(contract.blocking_drifts().len(), 1);
    }
}
