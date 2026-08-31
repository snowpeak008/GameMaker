//! Phase 2 治理契约（册 07）：一个真源 → 两条线 → 对齐合流 → 回填。
//!
//! 六个契约模块的分工：
//! - [`program_line`] / [`art_line`]：从 `GameSpec` 横向派生出的两条机器契约（下游只派生不发明，铁律①）；
//! - [`asset_registry`]：美术线的命名权威，稳定 `asset_id` 单点锚定（铁律②）；
//! - [`alignment`]：两条线的**确定性**合流核对（三要素 + orphan/conflict，不经 AI）；
//! - [`asset_genome`]：生产端回填，设计 id ↔ 实际文件，且 path = 运行时加载 path；
//! - [`authority_order`]：权威顺序校验插件，JSON 契约压过 Markdown（铁律③）。
//!
//! 全部契约结构带 `#[serde(default)]`：旧存档缺新字段必须照旧读得出来（D4 旧档兼容铁律）。
//! 缺字段读出来的是「空/未知」而不是编造值——判定环节遇到未知一律停（R2），不拿默认值冒充事实。

pub mod alignment;
pub mod art_line;
pub mod asset_genome;
pub mod asset_registry;
pub mod authority_order;
pub mod program_line;

use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

/// 程序线的线标识（写进 [`ContractEnvelope::consumer_line`]）。
pub const PROGRAM_LINE: &str = "program";

/// 美术线的线标识。
pub const ART_LINE: &str = "art";

/// 治理契约的 schema 版本（两条线 + 资产表 + 基因表共用一个版本号，同进同退）。
pub const GOVERNANCE_SCHEMA_VERSION: &str = "4.0.0";

/// 派生契约信封：两条线共用的头部，回答「这份契约派生自哪一版真源、按什么规则派生、缺了什么」。
///
/// `source_frozen_hash` 是**不新造第二真源**（D22）的机器保证：契约只是 `GameSpec` 的投影，
/// 换了冻结版本就必须重新派生，旧契约不得跨版本复用。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractEnvelope {
    pub schema_version: String,
    pub generated_at: String,
    /// 派生线标识：[`PROGRAM_LINE`] 或 [`ART_LINE`]。
    ///
    /// 空串 = 旧档未声明。这里**不给默认值**（不猜「大概是程序线」）：
    /// 校验时空串直接报错，让人去看那份契约到底是谁产的。
    pub consumer_line: String,
    /// 所派生真源的冻结哈希（`GameSpec::identity::frozen_hash`）。
    pub source_frozen_hash: String,
    /// 本线允许派生什么、禁止发明什么（铁律①的自述，随契约落盘可审计）。
    pub derivation_policy: Vec<String>,
    /// 真源里缺的事实（铁律①：标 gap，禁止用散文填补）。
    pub coverage_gaps: Vec<CoverageGap>,
}

impl ContractEnvelope {
    /// 新建信封（派生器用）。
    pub fn new(consumer_line: &str, generated_at: &str, source_frozen_hash: &str) -> Self {
        Self {
            schema_version: GOVERNANCE_SCHEMA_VERSION.to_string(),
            generated_at: generated_at.to_string(),
            consumer_line: consumer_line.to_string(),
            source_frozen_hash: source_frozen_hash.to_string(),
            derivation_policy: Vec::new(),
            coverage_gaps: Vec::new(),
        }
    }

    /// 校验信封：线标识必须与所属契约一致、真源哈希必须在案。
    ///
    /// 真源哈希为空即报错而不是放行：一份说不清自己派生自哪一版设计的契约，
    /// 下游拿去对齐得到的任何结论都没有意义（R2 未知即停）。
    pub fn validate(&self, expected_line: &str) -> Adm4Result<()> {
        if self.consumer_line.is_empty() {
            return Err(Adm4Error::validation(format!(
                "契约信封未声明派生线（应为 {expected_line}）：无法判断这份契约属于哪一条线"
            )));
        }
        if self.consumer_line != expected_line {
            return Err(Adm4Error::validation(format!(
                "契约信封的派生线为 {}，与所属契约 {expected_line} 不符",
                self.consumer_line
            )));
        }
        if self.source_frozen_hash.is_empty() {
            return Err(Adm4Error::validation(format!(
                "{expected_line} 契约未记录真源冻结哈希：派生契约必须锚定 GameSpec 版本（D22）"
            )));
        }
        Ok(())
    }

    /// 阻塞级 gap（非空即代表本线不可放行下游）。
    pub fn blocking_gaps(&self) -> Vec<&CoverageGap> {
        self.coverage_gaps
            .iter()
            .filter(|gap| gap.severity == GapSeverity::Blocking)
            .collect()
    }
}

/// 缺失事实登记：真源里没有、而本线又需要的事实。
///
/// 铁律①要求「缺失的事实必须标 gap，禁止用散文填补」——这个结构就是那张单子。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CoverageGap {
    pub gap_id: String,
    /// 缺的是什么事实（用真源侧的说法描述，便于回到设计工作台补）。
    pub missing_fact: String,
    /// 谁需要它（系统/资产/校验项 id）。
    pub required_by: String,
    pub severity: GapSeverity,
}

/// gap 的严重度。默认 [`GapSeverity::Blocking`]：旧档没写严重度时按**最保守**的一侧读，
/// 宁可多拦一次也不放一条未知事实过去（fail-closed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapSeverity {
    #[default]
    Blocking,
    Warning,
}

/// 资产像素尺寸（对齐三要素之一）。
///
/// 两个字段都**不带** `#[serde(default)]`：一份写了 `size` 却缺 `width` 的契约是坏数据，
/// 该解析失败；「没有尺寸信息」的正确表达是整个 `size` 缺席（`Option::None`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSize {
    pub width: u32,
    pub height: u32,
}

impl AssetSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl std::fmt::Display for AssetSize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}x{}", self.width, self.height)
    }
}

/// 对齐三要素：帧数 / 尺寸 / 格式（册 07 §4）。
///
/// 三项都是 `Option`：**没声明就是未知**。对齐层遇到未知一律判冲突交人工，
/// 绝不把「未知 vs 未知」当成一致（那正是 R2 禁止的默认值兜底）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SpecTriple {
    pub frames: Option<u32>,
    pub size: Option<AssetSize>,
    pub format: Option<String>,
}

impl SpecTriple {
    /// 三要素齐全的规格（派生器与测试的便捷构造）。
    pub fn full(frames: u32, size: AssetSize, format: &str) -> Self {
        Self {
            frames: Some(frames),
            size: Some(size),
            format: Some(format.to_string()),
        }
    }

    /// 一行人类可读描述（进冲突报告；未知项如实写「未声明」，不留空白）。
    pub fn describe(&self) -> String {
        let frames = match self.frames {
            Some(count) => format!("{count} 帧"),
            None => "帧数未声明".to_string(),
        };
        let size = match &self.size {
            Some(size) => size.to_string(),
            None => "尺寸未声明".to_string(),
        };
        let format = match &self.format {
            Some(format) => format.clone(),
            None => "格式未声明".to_string(),
        };
        format!("{frames} / {size} / {format}")
    }
}

/// 非空白校验的公用小工具：契约里的 id 类字段一律不许是空白串。
pub(crate) fn require_non_blank(value: &str, what: &str) -> Adm4Result<()> {
    if value.trim().is_empty() {
        return Err(Adm4Error::validation(format!("{what}不能为空")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_rejects_missing_line_and_hash() {
        let blank = ContractEnvelope::default();
        let error = blank.validate(PROGRAM_LINE).unwrap_err();
        assert!(error.message.contains("未声明派生线"), "{}", error.message);

        let mut wrong_line = ContractEnvelope::new(ART_LINE, "2026-08-31T00:00:00Z", "sha256:x");
        assert!(wrong_line.validate(PROGRAM_LINE).is_err());
        wrong_line.consumer_line = PROGRAM_LINE.to_string();
        assert!(wrong_line.validate(PROGRAM_LINE).is_ok());

        let mut no_hash = wrong_line.clone();
        no_hash.source_frozen_hash.clear();
        let error = no_hash.validate(PROGRAM_LINE).unwrap_err();
        assert!(error.message.contains("真源冻结哈希"), "{}", error.message);
    }

    /// 旧档兼容：只有信封三个老字段的 JSON 必须能读出来，新字段落成空集合。
    #[test]
    fn legacy_envelope_without_new_fields_parses() {
        let legacy = r#"{
          "schema_version": "4.0.0",
          "generated_at": "2026-08-30T10:00:00Z",
          "consumer_line": "program",
          "source_frozen_hash": "sha256:abc"
        }"#;
        let parsed: ContractEnvelope = serde_json::from_str(legacy).expect("旧档信封应可解析");
        assert!(parsed.derivation_policy.is_empty());
        assert!(parsed.coverage_gaps.is_empty());
        assert!(parsed.validate(PROGRAM_LINE).is_ok());
    }

    /// gap 缺 severity 时按最保守的 Blocking 读（fail-closed），不悄悄降级成 Warning。
    #[test]
    fn legacy_gap_without_severity_defaults_to_blocking() {
        let legacy = r#"{"gap_id":"g1","missing_fact":"缺帧数","required_by":"PlayerIdle"}"#;
        let parsed: CoverageGap = serde_json::from_str(legacy).expect("旧档 gap 应可解析");
        assert_eq!(parsed.severity, GapSeverity::Blocking);
    }

    #[test]
    fn spec_triple_round_trips_and_describes_unknowns() {
        let triple = SpecTriple::full(8, AssetSize::new(256, 256), "png");
        let json = serde_json::to_string(&triple).expect("序列化");
        let back: SpecTriple = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, triple);
        assert_eq!(back.describe(), "8 帧 / 256x256 / png");

        let partial: SpecTriple = serde_json::from_str(r#"{"frames":8}"#).expect("缺项应可解析");
        assert_eq!(partial.frames, Some(8));
        assert_eq!(partial.size, None);
        assert_eq!(partial.describe(), "8 帧 / 尺寸未声明 / 格式未声明");
    }

    /// 写了 size 却缺 width 是坏数据：必须解析失败，而不是补一个 0 宽度。
    #[test]
    fn asset_size_rejects_partial_payload() {
        assert!(serde_json::from_str::<SpecTriple>(r#"{"size":{"width":256}}"#).is_err());
    }

    #[test]
    fn blocking_gaps_filter_only_blocking_entries() {
        let envelope = ContractEnvelope {
            coverage_gaps: vec![
                CoverageGap {
                    gap_id: "g1".into(),
                    missing_fact: "缺帧数".into(),
                    required_by: "PlayerIdle".into(),
                    severity: GapSeverity::Blocking,
                },
                CoverageGap {
                    gap_id: "g2".into(),
                    missing_fact: "缺备注".into(),
                    required_by: "PlayerIdle".into(),
                    severity: GapSeverity::Warning,
                },
            ],
            ..ContractEnvelope::new(PROGRAM_LINE, "now", "sha256:x")
        };
        let blocking = envelope.blocking_gaps();
        assert_eq!(blocking.len(), 1);
        assert_eq!(blocking[0].gap_id, "g1");
    }
}
