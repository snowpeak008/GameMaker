use adm4_decision::{DecisionId, DesignLevel, GenrePackId, OptionId, ParameterValues};
use adm4_foundation::{Adm4Error, Adm4Result, UtcTimestamp};
use serde::{Deserialize, Serialize};

/// 通用层模板的 `genre_pack` 取值（同时是设计空间通用层目录名）。
///
/// 这类模板不绑定任何品类包：它们逆向的是跨品类的通用层决策点，因此可以预填到**任何**
/// 品类包的项目里（品类专属点不在答卷内，自然不会被写入）。
pub const UNIVERSAL_GENRE_PACK: &str = "universal";

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

/// 逆向答卷上的一个附加已选选项（首个已选选项仍平铺在 `TemplateAnswer` 上，保持旧档兼容）。
///
/// 结构与项目态的 `adm4_decision::SelectedOption` 对齐，但不带 `rationale`/`template_original`：
/// 模板是「这个游戏怎么选的」的证据，理由与换皮原值属于项目态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TemplateSelectedOption {
    pub option_id: OptionId,
    #[serde(default)]
    pub parameters: ParameterValues,
}

/// 答卷上一个已选选项的只读视图（主选在前），让预填/对照按已选选项逐个处理。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateSelectedOptionRef<'a> {
    pub option_id: &'a str,
    pub parameters: &'a ParameterValues,
    /// 多选点的主选标记（单选答案恒 false）。
    pub is_primary: bool,
}

/// 逆向答卷条目：该游戏在某决策点「选了哪些选项、哪个是主选、填了什么参数」。
///
/// 多选是扩展出来的：`option_id` 仍是首个已选选项（旧档兼容锚点，单值文件原样可读），
/// 其余选项落 `additional_options`、主选落 `primary_option`——两个键都 `serde(default)`，
/// 因此二版迁移出的旧格式答卷文件一字节不改也能反序列化。
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
    /// 多选点的其余已选选项（单选点恒空）。
    #[serde(default)]
    pub additional_options: Vec<TemplateSelectedOption>,
    /// 多选点的主选选项 id；必须落在已选集合内（预填时校验）。
    #[serde(default)]
    pub primary_option: Option<OptionId>,
}

impl TemplateAnswer {
    /// 全部已选选项，**主选排在最前**，其余保持声明顺序；单选答案返回 1 条。
    pub fn selected_options(&self) -> Vec<TemplateSelectedOptionRef<'_>> {
        let is_primary = |option_id: &str| self.primary_option.as_deref() == Some(option_id);
        let mut refs = Vec::with_capacity(1 + self.additional_options.len());
        refs.push(TemplateSelectedOptionRef {
            option_id: self.option_id.as_str(),
            parameters: &self.parameters,
            is_primary: is_primary(&self.option_id),
        });
        for extra in &self.additional_options {
            refs.push(TemplateSelectedOptionRef {
                option_id: extra.option_id.as_str(),
                parameters: &extra.parameters,
                is_primary: is_primary(&extra.option_id),
            });
        }
        refs.sort_by_key(|item| !item.is_primary);
        refs
    }

    /// 已选选项 id（顺序同 `selected_options`）。
    pub fn selected_option_ids(&self) -> Vec<&str> {
        self.selected_options()
            .into_iter()
            .map(|item| item.option_id)
            .collect()
    }

    pub fn contains_option(&self, option_id: &str) -> bool {
        self.option_id == option_id
            || self
                .additional_options
                .iter()
                .any(|extra| extra.option_id == option_id)
    }

    /// 已选选项数量（≥1）。
    pub fn selected_count(&self) -> usize {
        1 + self.additional_options.len()
    }
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

    /// 通用层模板：不绑定品类包，可预填到任何包的项目。
    pub fn is_universal(&self) -> bool {
        self.genre_pack == UNIVERSAL_GENRE_PACK
    }

    /// 换皮词表贡献。
    pub fn skin_words(&self) -> Vec<String> {
        let mut words = vec![self.game_name.clone()];
        words.extend(self.aliases.clone());
        words
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 旧格式答卷（只有单个 `option_id`，没有多选键）必须原样反序列化，
    /// 且默认值与扩展前逐字节等价——T10 已落盘的 26 份模板文件不能因为加字段而读不出来。
    #[test]
    fn legacy_single_option_answer_still_parses() {
        let legacy = r#"{
          "decision_id": "u.genre",
          "option_id": "lane_defense",
          "evidence": [
            {"source_url": "adm4://v2-builtin/x.json", "source_type": "inference", "confidence": "low"}
          ],
          "notes": "旧档"
        }"#;
        let answer: TemplateAnswer = serde_json::from_str(legacy).expect("旧格式答卷应可解析");
        assert_eq!(answer.option_id, "lane_defense");
        assert!(answer.additional_options.is_empty());
        assert!(answer.primary_option.is_none());
        assert_eq!(answer.selected_count(), 1);
        assert_eq!(answer.selected_option_ids(), vec!["lane_defense"]);
        assert!(!answer.selected_options()[0].is_primary);
        // 回写后不含多选键（旧档往返不长胖）。
        let json = serde_json::to_string(&answer).expect("序列化");
        assert!(json.contains(r#""additional_options":[]"#), "{json}");
        assert!(json.contains(r#""primary_option":null"#), "{json}");
        let round_trip: TemplateAnswer = serde_json::from_str(&json).expect("往返");
        assert_eq!(round_trip, answer);
    }

    /// 多选答卷：全部已选选项可读，主选排在最前。
    #[test]
    fn multi_option_answer_orders_primary_first() {
        let multi = r#"{
          "decision_id": "v2.gameplay_system_scope",
          "option_id": "action_rule",
          "evidence": [
            {"source_url": "adm4://v2-builtin/x.json", "source_type": "inference", "confidence": "low"}
          ],
          "additional_options": [
            {"option_id": "objective"},
            {"option_id": "settlement"}
          ],
          "primary_option": "settlement"
        }"#;
        let answer: TemplateAnswer = serde_json::from_str(multi).expect("多选答卷应可解析");
        assert_eq!(answer.selected_count(), 3);
        assert_eq!(
            answer.selected_option_ids(),
            vec!["settlement", "action_rule", "objective"]
        );
        assert!(answer.selected_options()[0].is_primary);
        assert!(answer.contains_option("objective"));
        assert!(!answer.contains_option("liveops_event"));
    }

    #[test]
    fn universal_pack_is_recognized() {
        let mut template = Template {
            template_id: "tpl".into(),
            game_name: "虚构甲".into(),
            aliases: Vec::new(),
            genre_pack: UNIVERSAL_GENRE_PACK.into(),
            pack_version: "0.1.0".into(),
            depth_reached: crate::model::DesignLevel::L4,
            answers: Vec::new(),
            certification: Certification::default(),
            mapping_hash: String::new(),
            crosscheck_proof: None,
        };
        assert!(template.is_universal());
        template.genre_pack = "lane_defense".into();
        assert!(!template.is_universal());
    }
}
