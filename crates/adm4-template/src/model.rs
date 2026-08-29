use adm4_decision::{DecisionId, DesignLevel, GenrePackId, OptionId, ParameterValues};
use adm4_foundation::{Adm4Error, Adm4Result, UtcTimestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Official,
    Wiki,
    Datamine,
    Inference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Med,
    High,
}

/// 逆向证据：source_url 必填（宁缺勿造）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub source_url: String,
    #[serde(default)]
    pub quote: String,
    pub source_type: SourceType,
    pub confidence: Confidence,
}

/// 逆向答卷条目：该游戏在某决策点「选了哪个选项、填了什么参数」。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateAnswer {
    pub decision_id: DecisionId,
    pub option_id: OptionId,
    #[serde(default)]
    pub parameters: ParameterValues,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub notes: String,
    /// S3 交叉核验结论（None=未核验；true=一致；false=冲突待人工）。
    #[serde(default)]
    pub crosscheck_agreed: Option<bool>,
}

/// 认证状态：产线固定流转 `Draft→Mapped→CrossChecked→HumanReviewed→Certified`（决定 D8）。
///
/// `Approved`/`Rejected` 是产线状态机建立前的遗留变体，仅为旧存档反序列化兼容保留；
/// 它们不在流转链上——处于遗留状态的模板既不能推进也不能预填，须重走产线。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CertificationStatus {
    #[default]
    Draft,
    Mapped,
    CrossChecked,
    HumanReviewed,
    Certified,
    Approved,
    Rejected,
}

impl CertificationStatus {
    /// 产线流转的下一步；终态（Certified）与遗留状态（Approved/Rejected）没有下一步。
    pub fn pipeline_next(self) -> Option<Self> {
        match self {
            Self::Draft => Some(Self::Mapped),
            Self::Mapped => Some(Self::CrossChecked),
            Self::CrossChecked => Some(Self::HumanReviewed),
            Self::HumanReviewed => Some(Self::Certified),
            Self::Certified | Self::Approved | Self::Rejected => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Certification {
    pub status: CertificationStatus,
    #[serde(default)]
    pub reviewed_by: String,
    #[serde(default)]
    pub reviewed_at: String,
    #[serde(default)]
    pub review_note: String,
}

impl Certification {
    /// 逐级推进认证状态：`target` 必须恰好是当前状态的下一步，跳级/回退/原地一律 Err。
    pub fn advance_to(&mut self, target: CertificationStatus) -> Adm4Result<()> {
        match self.status.pipeline_next() {
            Some(next) if next == target => {
                self.status = target;
                Ok(())
            }
            Some(next) => Err(Adm4Error::blocked(format!(
                "认证状态只能逐级前进：当前 {:?} 的下一步是 {next:?}，不能进入 {target:?}（禁止跳级与回退）",
                self.status
            ))),
            None => Err(Adm4Error::blocked(format!(
                "认证状态 {:?} 不在产线流转链上或已是终态，不能进入 {target:?}",
                self.status
            ))),
        }
    }

    /// S4 人工审核：`CrossChecked→HumanReviewed`，同时落评审证明（R3：署名 + 结论必填）。
    pub fn record_human_review(&mut self, reviewer: &str, note: &str) -> Adm4Result<()> {
        let reviewer = reviewer.trim();
        let note = note.trim();
        if reviewer.is_empty() {
            return Err(Adm4Error::invalid_input(
                "人工审核必须署名评审人（R3 评审工作量证明）",
            ));
        }
        if note.is_empty() {
            return Err(Adm4Error::invalid_input(
                "人工审核必须填写审核结论（R3 评审工作量证明）",
            ));
        }
        self.advance_to(CertificationStatus::HumanReviewed)?;
        self.reviewed_by = reviewer.to_string();
        self.reviewed_at = UtcTimestamp::now().to_iso8601();
        self.review_note = note.to_string();
        Ok(())
    }
}

/// S2 映射与 S3 核验两次**独立** AI 会话的机器证据（R3）。
///
/// 记录两次会话应答的内容哈希：相同即「第二会话复读第一会话」，核验不成立；
/// 落在模板上是为了让证据可审计——事后能复核这两会话确实互异。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossCheckProof {
    /// S2 映射会话应答的内容哈希。
    pub mapping_hash: String,
    /// S3 核验会话应答的内容哈希。
    pub crosscheck_hash: String,
    /// 逐条核验覆盖的答卷条数（= 答卷总条数，不多不少）。
    pub checked_count: usize,
    pub checked_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub template_id: String,
    pub game_name: String,
    /// 别名（换皮词表来源：game_name + aliases）。
    #[serde(default)]
    pub aliases: Vec<String>,
    pub genre_pack: GenrePackId,
    pub pack_version: String,
    /// 逆向到哪层（可低于 L6，按 coverage 如实呈现）。
    pub depth_reached: DesignLevel,
    pub answers: Vec<TemplateAnswer>,
    #[serde(default)]
    pub certification: Certification,
    /// S2 映射会话应答的内容哈希（S3 据此做两会话互异校验）。旧档为空串。
    #[serde(default)]
    pub mapping_hash: String,
    /// S3 核验通过后留下的两会话机器证据。旧档为 None。
    #[serde(default)]
    pub crosscheck_proof: Option<CrossCheckProof>,
}

impl Template {
    /// 是否已认证入库（Certified）：只有认证模板可入库与预填（决定 D8）。
    pub fn is_certified(&self) -> bool {
        self.certification.status == CertificationStatus::Certified
    }

    /// 兼容旧调用名（adm4-authoring 预填路径使用）；语义等同 [`Template::is_certified`]。
    pub fn is_approved(&self) -> bool {
        self.is_certified()
    }

    /// 换皮词表贡献。
    pub fn skin_words(&self) -> Vec<String> {
        let mut words = vec![self.game_name.clone()];
        words.extend(self.aliases.clone());
        words
    }
}
