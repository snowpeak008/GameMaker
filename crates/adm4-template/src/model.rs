use adm4_decision::{DecisionId, DesignLevel, GenrePackId, OptionId, ParameterValues};
use adm4_foundation::{Adm4Error, Adm4Result, UtcTimestamp, sha256_hex};
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

/// 模板的来源：决定它要走哪条产线、认证与取用时查什么证据、以及是否登记换皮词表。
///
/// 三种来源的可信依据完全不同，混在一起会让「已认证」三个字失去含义：
/// - 逆向来源的依据是**外部证据链**（S1 检索 → S2 映射 → S3 独立二次核验）；
/// - 本项目导出的依据是**源项目里逐条被用户确认过**，没有外部语料可查。
///   为它伪造 S1-S3 的证据链等于造假，所以它直接落 S4 人工审核（署名 + 结论必填）；
/// - 批量迁移的依据是**离线迁移登记**（批次 + 工具版本 + 源引用 + 答卷指纹）。
///
/// 旧档没有这个键（`serde(default)`）→ 一律按逆向来源解读，默认值与扩展前的行为逐字一致；
/// 而逆向来源的证据关卡最严，因此「缺 origin 键的伪造模板」落在最严的分支上（fail-closed）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum TemplateOrigin {
    /// 逆向外部游戏（产线 S1-S5 全程）。
    #[default]
    Reverse,
    /// 从本项目导出（「另存模板」）：不走 S1-S3，认证前必须有人工审核署名。
    ProjectExport {
        /// 源项目存档 id（可追溯到底是哪个项目导出的）。
        source_archive_id: String,
        /// 导出当时的源项目名（项目可能改名，这里留导出时的快照）。
        source_project_name: String,
        exported_at: String,
    },
    /// 离线批量迁移（二版内置库 → 四版模板库）：不走 S1-S3，凭**迁移登记**准入。
    ///
    /// 为什么需要这个变体：25 份二版内置模板是迁移工具直接写死 `Certified` 落盘的，
    /// 既不经 [`crate::TemplateLibrary::certify`]，也就不受任何证据关卡约束。
    /// 若取用路径只看状态位，「手工往 references/ 里塞一份写着 certified 的 JSON」
    /// 就等于拿到了预填资格——认证流程形同虚设。所以批量迁移必须**自带可核对的登记**：
    /// 批次标识 + 工具版本 + 源引用（人可回溯到原始数据）加上答卷结构指纹（机器可重算）。
    BulkMigration {
        /// 迁移批次标识（同一次迁移的全部模板共享，可对账「哪一批、什么时候迁的」）。
        batch_id: String,
        /// 迁移工具版本（换了工具就换版本号，产物可归因到具体代码）。
        tool_version: String,
        /// 迁移源引用（二版侧的文件路径/标识，可回到原始数据逐条核对）。
        source_ref: String,
        /// 答卷结构指纹，形态与覆盖范围见 [`Template::answers_digest`]。
        answers_digest: String,
        migrated_at: String,
    },
}

impl TemplateOrigin {
    /// 中文展示名（CLI/GUI 列表直接用，不必各自映射枚举）。
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Reverse => "逆向外部游戏",
            Self::ProjectExport { .. } => "本项目导出",
            Self::BulkMigration { .. } => "批量迁移",
        }
    }
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
    /// 来源标记（旧档缺该键 = 逆向外部游戏）。
    #[serde(default)]
    pub origin: TemplateOrigin,
    /// S2 映射会话应答的内容哈希（S3 据此做两会话互异校验）。旧档为空串。
    #[serde(default)]
    pub mapping_hash: String,
    /// S3 核验通过后留下的两会话机器证据。旧档为 None。
    #[serde(default)]
    pub crosscheck_proof: Option<CrossCheckProof>,
    /// 冒烟测试模板标记（T-W7-4b）：标 true 的模板只服务于工具链冒烟/回归，
    /// 答卷数据未经产线校准，预填路径默认拒绝（`--allow-smoke` 显式放行）。
    ///
    /// 旧档缺键 = false（正常模板），行为与扩展前逐字节一致；`answers_digest`
    /// 只覆盖答卷结构，不含本字段——25 份 BulkMigration 模板加标记不破坏迁移登记指纹。
    #[serde(default)]
    pub smoke_test: bool,
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

    /// 本项目导出的模板（「另存模板」的产物）。
    pub fn is_project_export(&self) -> bool {
        matches!(self.origin, TemplateOrigin::ProjectExport { .. })
    }

    /// 离线批量迁移落盘的模板（二版内置库迁入的 25 份）。
    pub fn is_bulk_migration(&self) -> bool {
        matches!(self.origin, TemplateOrigin::BulkMigration { .. })
    }

    /// 换皮词表贡献：`game_name` + `aliases`，**不分来源**（R5）。
    ///
    /// 「本项目导出的模板不登记词条」曾被当成解法，因为源项目自己的名字进了词表后，
    /// 它自己的 C0 文档标题（就是项目名）会被换皮门拦住。但那留下一个更大的洞：
    /// A 项目导出模板 → B 项目预填 → A 的项目名（典型出现在「模板预填自 A」的选择理由里）
    /// 进了 B 的创作态，而词表里没有 A，**B 的产物可以带着 A 的名字通过冻结门**——
    /// 换皮扫描对「抄另一个项目」彻底失效。
    ///
    /// 所以词表照常登记，改由扫描侧按当前项目豁免自身：见
    /// [`adm4_contracts::SkinScanner::with_exemptions`]。登记是全局的、豁免是逐项目的，
    /// 这才对得上「谁是参考、谁是自己」随视角变化的事实。
    pub fn skin_words(&self) -> Vec<String> {
        let mut words = vec![self.game_name.clone()];
        words.extend(self.aliases.clone());
        words
    }

    /// 答卷结构指纹：批量迁移登记的机器可核对项。
    ///
    /// canonical 形态（逐条一行，制表符分隔，行尾换行）：
    /// `decision_id \t option_id \t 附加选项 id（逗号连接，声明序） \t 主选 id（无则空）`
    /// 再对整段 UTF-8 字节取 sha256（带 `sha256:` 前缀）。
    ///
    /// 刻意只覆盖「答了哪些点、选了哪些选项、谁是主选」这层结构，**不覆盖** `parameters`
    /// 与 `evidence`：那两者的 canonical 形态依赖 serde 的枚举表示，离线迁移工具（Python）
    /// 要逐字节复现就成了跨语言的隐式契约，一改序列化就静默失配。因此这里给的是**结构指纹**
    /// 而非全量内容哈希，名字如实叫 digest，能核对的范围写在这里，不多许诺。
    pub fn answers_digest(&self) -> String {
        let mut canonical = String::new();
        for answer in &self.answers {
            let additional: Vec<&str> = answer
                .additional_options
                .iter()
                .map(|extra| extra.option_id.as_str())
                .collect();
            canonical.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                answer.decision_id,
                answer.option_id,
                additional.join(","),
                answer.primary_option.as_deref().unwrap_or("")
            ));
        }
        sha256_hex(canonical.as_bytes())
    }

    /// 「另存模板」的人工审核落地：从 `Draft` 直接落 `HumanReviewed`（跳过 S1-S3）。
    ///
    /// 为什么可以跳：S1-S3 的产出是「外部语料 → 证据链 → 独立二次核验」，本项目导出
    /// 根本没有外部语料，跑一遍只会产出编造的证据。它的可信依据是答卷里的每一条都
    /// 在源项目里被用户确认过，再加上这里的人工审核署名（R3：署名 + 结论双必填）。
    ///
    /// 只对本项目导出来源开放；逆向来源仍必须逐级走完产线（否则等于给逆向开后门）。
    pub fn record_project_export_review(&mut self, reviewer: &str, note: &str) -> Adm4Result<()> {
        if !self.is_project_export() {
            return Err(Adm4Error::invalid_input(format!(
                "模板 {} 是{}来源，人工审核必须走产线 S1-S3 之后的 S4（不得跳过证据链）",
                self.template_id,
                self.origin.label_zh()
            )));
        }
        if self.certification.status != CertificationStatus::Draft {
            return Err(Adm4Error::blocked(format!(
                "模板 {} 当前认证状态 {:?}，「另存模板」的人工审核只能从 Draft 落地",
                self.template_id, self.certification.status
            )));
        }
        let reviewer = reviewer.trim();
        let note = note.trim();
        if reviewer.is_empty() {
            return Err(Adm4Error::invalid_input(
                "另存模板必须署名评审人（R3 评审工作量证明）",
            ));
        }
        if note.is_empty() {
            return Err(Adm4Error::invalid_input(
                "另存模板必须填写审核结论（R3 评审工作量证明）",
            ));
        }
        self.certification.status = CertificationStatus::HumanReviewed;
        self.certification.reviewed_by = reviewer.to_string();
        self.certification.reviewed_at = UtcTimestamp::now().to_iso8601();
        self.certification.review_note = note.to_string();
        Ok(())
    }

    /// 认证与取用的共用证据关卡：按来源查它该有的证据。
    ///
    /// 两个消费点共用同一份判定（S5 [`crate::TemplateLibrary::certify`] 与取用
    /// [`crate::TemplateLibrary::approved_for_prefill`]），否则「认证时查、取用时不查」
    /// 就留下旁路：绕过认证流程直接落盘一份 `status=certified` 的 JSON 即可获得预填资格。
    ///
    /// 逐来源要求：
    /// - 逆向来源必须同时有 S2 映射哈希与 S3 两会话证据，且证据里的 `mapping_hash`
    ///   必须与模板当前的映射哈希对得上——否则「已认证」没有任何机器可核的依据
    ///   （手改状态字段、或映射被重跑而核验没跟上，都能造出这种模板）；
    /// - 本项目导出来源不查这两项（它不走 S1-S3），改查人工审核署名与结论；
    /// - 批量迁移来源查迁移登记：批次/工具版本/源引用三项非空（人可回溯），
    ///   且 `answers_digest` 与当前答卷重算结果逐字相同（机器可核）。
    pub fn require_certification_evidence(&self) -> Adm4Result<()> {
        if let TemplateOrigin::BulkMigration {
            batch_id,
            tool_version,
            source_ref,
            answers_digest,
            ..
        } = &self.origin
        {
            for (label, value) in [
                ("迁移批次标识", batch_id),
                ("迁移工具版本", tool_version),
                ("迁移源引用", source_ref),
            ] {
                if value.trim().is_empty() {
                    return Err(Adm4Error::red_line(format!(
                        "R3: 批量迁移模板 {} 的迁移登记缺{label}，登记无法核对，不得取用",
                        self.template_id
                    )));
                }
            }
            let actual = self.answers_digest();
            if answers_digest.trim() != actual {
                return Err(Adm4Error::red_line(format!(
                    "R3: 批量迁移模板 {} 的答卷结构指纹与登记不符（登记 {answers_digest}，重算 {actual}），\
                     该登记不为当前答卷背书，不得取用",
                    self.template_id
                )));
            }
            return Ok(());
        }
        if self.is_project_export() {
            if self.certification.reviewed_by.trim().is_empty()
                || self.certification.review_note.trim().is_empty()
            {
                return Err(Adm4Error::red_line(format!(
                    "R3: 本项目导出模板 {} 缺人工审核署名或结论，不得认证入库",
                    self.template_id
                )));
            }
            return Ok(());
        }
        if self.mapping_hash.trim().is_empty() {
            return Err(Adm4Error::red_line(format!(
                "R3: 逆向模板 {} 没有 S2 映射会话哈希，认证缺机器证据（请重走 S2/S3）",
                self.template_id
            )));
        }
        let Some(proof) = &self.crosscheck_proof else {
            return Err(Adm4Error::red_line(format!(
                "R3: 逆向模板 {} 没有 S3 两会话交叉核验证据，认证缺机器证据（请重走 S3）",
                self.template_id
            )));
        };
        if proof.crosscheck_hash.trim().is_empty() || proof.mapping_hash != self.mapping_hash {
            return Err(Adm4Error::red_line(format!(
                "R3: 逆向模板 {} 的核验证据与当前映射哈希不对应（证据 {} vs 映射 {}），认证被拒",
                self.template_id, proof.mapping_hash, self.mapping_hash
            )));
        }
        Ok(())
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

    fn sample_template() -> Template {
        Template {
            template_id: "tpl".into(),
            game_name: "虚构甲".into(),
            aliases: vec!["Fictional A".into()],
            genre_pack: UNIVERSAL_GENRE_PACK.into(),
            pack_version: "0.1.0".into(),
            depth_reached: crate::model::DesignLevel::L4,
            answers: Vec::new(),
            certification: Certification::default(),
            origin: TemplateOrigin::Reverse,
            mapping_hash: String::new(),
            crosscheck_proof: None,
            smoke_test: false,
        }
    }

    fn project_export_origin() -> TemplateOrigin {
        TemplateOrigin::ProjectExport {
            source_archive_id: "archive-1".into(),
            source_project_name: "霜落峡谷".into(),
            exported_at: "2026-08-31T00:00:00Z".into(),
        }
    }

    #[test]
    fn universal_pack_is_recognized() {
        let mut template = sample_template();
        assert!(template.is_universal());
        template.genre_pack = "lane_defense".into();
        assert!(!template.is_universal());
    }

    /// 旧档没有 `origin` 键 → 逆向来源（默认值与扩展前行为一致），且回写后可往返。
    #[test]
    fn legacy_template_without_origin_reads_as_reverse() {
        let legacy = r#"{
          "template_id": "builtin_x",
          "game_name": "虚构甲",
          "genre_pack": "universal",
          "pack_version": "0.1.0",
          "depth_reached": "L4",
          "answers": []
        }"#;
        let template: Template = serde_json::from_str(legacy).expect("旧档模板应可解析");
        assert_eq!(template.origin, TemplateOrigin::Reverse);
        assert!(!template.is_project_export());
        // 逆向来源照旧贡献换皮词条。
        assert_eq!(template.skin_words(), vec!["虚构甲".to_string()]);
        let json = serde_json::to_string(&template).expect("序列化");
        assert!(json.contains(r#""origin":"reverse""#), "{json}");
        let round_trip: Template = serde_json::from_str(&json).expect("往返");
        assert_eq!(round_trip, template);
    }

    /// 本项目导出模板**照常**登记换皮词表：源项目的名字对别的项目就是参考名。
    ///
    /// 「不登记」曾是为了让源项目自己过得了换皮门，代价是 B 抄 A 无人拦（跨项目漏洞）。
    /// 现在登记照做，源项目自身的放行改由扫描侧豁免（`SkinScanner::with_exemptions`）。
    #[test]
    fn project_export_contributes_skin_words_like_any_other_origin() {
        let mut template = sample_template();
        template.origin = project_export_origin();
        assert!(template.is_project_export());
        assert_eq!(
            template.skin_words(),
            vec!["虚构甲".to_string(), "Fictional A".to_string()],
            "源项目名必须进词表，否则别的项目抄它没人拦"
        );
    }

    fn bulk_migration_origin(answers_digest: &str) -> TemplateOrigin {
        TemplateOrigin::BulkMigration {
            batch_id: "v2-builtin-2026-08-29".into(),
            tool_version: "v2_migration/1.1.0".into(),
            source_ref: "knowledge/design_data/project_templates/x.json".into(),
            answers_digest: answers_digest.into(),
            migrated_at: "2026-08-29T00:00:00Z".into(),
        }
    }

    /// 答卷结构指纹随答卷变化，且对同一答卷稳定（迁移工具据此产出可核对的登记）。
    #[test]
    fn answers_digest_is_stable_and_content_sensitive() {
        let mut template = sample_template();
        let empty = template.answers_digest();
        assert!(empty.starts_with("sha256:"), "{empty}");
        assert_eq!(empty, template.answers_digest());

        template.answers.push(TemplateAnswer {
            decision_id: "u.genre".into(),
            option_id: "lane_defense".into(),
            parameters: ParameterValues::default(),
            evidence: Vec::new(),
            notes: String::new(),
            crosscheck_agreed: None,
            additional_options: vec![TemplateSelectedOption {
                option_id: "objective".into(),
                parameters: ParameterValues::default(),
            }],
            primary_option: Some("objective".into()),
        });
        let one = template.answers_digest();
        assert_ne!(one, empty);
        // 主选变化改变指纹（结构层面的改动逃不掉）。
        template.answers[0].primary_option = None;
        assert_ne!(template.answers_digest(), one);
    }

    /// 批量迁移来源的证据关卡：登记三项必填 + 指纹必须与当前答卷对得上。
    #[test]
    fn bulk_migration_requires_verifiable_registration() {
        let mut template = sample_template();
        template.certification.status = CertificationStatus::Certified;

        // 缺 origin 键（旧档/手工伪造）→ 落在最严的逆向分支上，直接被拒。
        assert_eq!(
            template
                .require_certification_evidence()
                .expect_err("伪造的 certified 模板必须被拒")
                .kind,
            adm4_foundation::Adm4ErrorKind::RedLine
        );

        // 指纹对不上 → 登记不为当前答卷背书。
        template.origin = bulk_migration_origin("sha256:not-the-right-one");
        let error = template
            .require_certification_evidence()
            .expect_err("指纹不符必须被拒");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
        assert!(error.message.contains("指纹"), "{}", error.message);

        // 登记项留空 → 无法核对。
        template.origin = bulk_migration_origin(&template.answers_digest());
        if let TemplateOrigin::BulkMigration { batch_id, .. } = &mut template.origin {
            *batch_id = "  ".into();
        }
        assert!(template.require_certification_evidence().is_err());

        // 登记齐备 + 指纹对上 → 放行（且照旧不要求 S2/S3 机器证据）。
        template.origin = bulk_migration_origin(&template.answers_digest());
        template
            .require_certification_evidence()
            .expect("登记可核对应放行");
        assert!(template.is_bulk_migration());
        assert!(template.mapping_hash.is_empty());
        assert!(template.crosscheck_proof.is_none());
    }

    /// 批量迁移来源的 JSON 形态：内部 tag `origin` + 平铺登记字段，且可往返。
    #[test]
    fn bulk_migration_origin_round_trips_as_flat_json() {
        let raw = r#"{
          "template_id": "builtin_x",
          "game_name": "Arknights",
          "genre_pack": "universal",
          "pack_version": "0.1.0",
          "depth_reached": "L4",
          "origin": {
            "origin": "bulk_migration",
            "batch_id": "v2-builtin-2026-08-29",
            "tool_version": "v2_migration/1.1.0",
            "source_ref": "knowledge/design_data/project_templates/builtin_x.json",
            "answers_digest": "sha256:abc",
            "migrated_at": "2026-08-29T00:00:00Z"
          },
          "answers": []
        }"#;
        let template: Template = serde_json::from_str(raw).expect("批量迁移模板应可解析");
        assert!(template.is_bulk_migration());
        assert_eq!(template.origin.label_zh(), "批量迁移");
        let json = serde_json::to_string(&template).expect("序列化");
        assert!(json.contains(r#""origin":"bulk_migration""#), "{json}");
        assert!(json.contains(r#""answers_digest":"sha256:abc""#), "{json}");
        let round_trip: Template = serde_json::from_str(&json).expect("往返");
        assert_eq!(round_trip, template);
    }

    /// 「另存模板」的人工审核只对本项目导出来源开放，且署名与结论双必填。
    #[test]
    fn project_export_review_requires_signature_and_is_origin_scoped() {
        let mut reverse = sample_template();
        let error = reverse
            .record_project_export_review("评审员甲", "结论")
            .expect_err("逆向来源不得跳过 S1-S3");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::InvalidInput);
        assert_eq!(reverse.certification.status, CertificationStatus::Draft);

        let mut export = sample_template();
        export.origin = project_export_origin();
        assert!(export.record_project_export_review("  ", "结论").is_err());
        assert!(
            export
                .record_project_export_review("评审员甲", " ")
                .is_err()
        );
        assert_eq!(export.certification.status, CertificationStatus::Draft);
        export
            .record_project_export_review(" 评审员甲 ", " 已逐条复核 ")
            .expect("署名与结论齐备应通过");
        assert_eq!(
            export.certification.status,
            CertificationStatus::HumanReviewed
        );
        assert_eq!(export.certification.reviewed_by, "评审员甲");
        assert_eq!(export.certification.review_note, "已逐条复核");
        // 幂等重复落地被拒（只能从 Draft 落）。
        assert!(
            export
                .record_project_export_review("评审员甲", "再来一次")
                .is_err()
        );
    }

    /// 认证证据关卡：逆向来源缺 mapping_hash / crosscheck_proof 一律被拒；
    /// 本项目导出来源不查这两项，只查人工审核署名。
    #[test]
    fn certification_evidence_gate_is_strict_for_reverse_only() {
        let mut reverse = sample_template();
        assert_eq!(
            reverse
                .require_certification_evidence()
                .expect_err("缺映射哈希")
                .kind,
            adm4_foundation::Adm4ErrorKind::RedLine
        );
        reverse.mapping_hash = "sha256:map".into();
        assert!(
            reverse.require_certification_evidence().is_err(),
            "只有映射哈希、没有 S3 证据同样不许认证"
        );
        reverse.crosscheck_proof = Some(CrossCheckProof {
            mapping_hash: "sha256:stale".into(),
            crosscheck_hash: "sha256:check".into(),
            checked_count: 1,
            checked_at: "2026-08-31T00:00:00Z".into(),
        });
        assert!(
            reverse.require_certification_evidence().is_err(),
            "核验证据对不上当前映射哈希 → 证据失效"
        );
        reverse.crosscheck_proof = Some(CrossCheckProof {
            mapping_hash: "sha256:map".into(),
            crosscheck_hash: "sha256:check".into(),
            checked_count: 1,
            checked_at: "2026-08-31T00:00:00Z".into(),
        });
        reverse
            .require_certification_evidence()
            .expect("证据齐备应放行");

        let mut export = sample_template();
        export.origin = project_export_origin();
        assert!(
            export.require_certification_evidence().is_err(),
            "本项目导出缺人工审核署名同样被拒"
        );
        export
            .record_project_export_review("评审员甲", "已逐条复核")
            .expect("人工审核");
        export
            .require_certification_evidence()
            .expect("本项目导出不要求 S2/S3 机器证据");
        assert!(export.mapping_hash.is_empty());
        assert!(export.crosscheck_proof.is_none());
    }
}
