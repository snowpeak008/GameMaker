use crate::values::SpecRef;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// R1 指标即测量
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidencePointer {
    pub file: String,
    pub path: String,
    pub observed: String,
}

/// 带证据的指标。字段私有 + 构造校验：证据为空而值非零 → 构造即拒（R1）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredMetric {
    value: f64,
    evidence: Vec<EvidencePointer>,
}

impl MeasuredMetric {
    pub fn new(value: f64, evidence: Vec<EvidencePointer>) -> Adm4Result<Self> {
        if value != 0.0 && evidence.is_empty() {
            return Err(Adm4Error::red_line(
                "R1: non-zero metric without evidence pointers",
            ));
        }
        Ok(Self { value, evidence })
    }

    pub fn zero() -> Self {
        Self {
            value: 0.0,
            evidence: Vec::new(),
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn evidence(&self) -> &[EvidencePointer] {
        &self.evidence
    }
}

// ---------------------------------------------------------------------------
// R2 未知即停
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnclassifiedItem {
    pub item: String,
    pub reason: String,
}

/// 派生结果：要么可解析，要么携带待分类清单阻塞。禁止默认值兜底（R2）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "derive", rename_all = "snake_case")]
pub enum Derive<T> {
    Resolved(T),
    Blocked { unknown: Vec<UnclassifiedItem> },
}

impl<T> Derive<T> {
    pub fn into_result(self, context: &str) -> Adm4Result<T> {
        match self {
            Derive::Resolved(value) => Ok(value),
            Derive::Blocked { unknown } => Err(Adm4Error::blocked(format!(
                "R2: {context} blocked with {} unclassified items: {}",
                unknown.len(),
                unknown
                    .iter()
                    .map(|item| item.item.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// R3 评审最低工作量证明
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryEvidence {
    pub category: String,
    pub checked: String,
    pub conclusion: String,
    pub evidence: Vec<EvidencePointer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewProof {
    pub reviewer: String,
    pub reviewed_count: usize,
    pub upstream_count: usize,
    pub content_hash: String,
    pub per_category_evidence: Vec<CategoryEvidence>,
}

impl ReviewProof {
    pub fn verify(&self) -> Adm4Result<()> {
        if self.reviewed_count != self.upstream_count {
            return Err(Adm4Error::red_line(format!(
                "R3: reviewed_count {} != upstream_count {}",
                self.reviewed_count, self.upstream_count
            )));
        }
        if self.per_category_evidence.is_empty() {
            return Err(Adm4Error::red_line(
                "R3: review proof must carry per-category evidence even with zero findings",
            ));
        }
        Ok(())
    }
}

/// 同批多份评审：内容哈希全同 → 判定橡皮图章 → fail（R3）。
pub fn verify_review_batch(proofs: &[ReviewProof]) -> Adm4Result<()> {
    for proof in proofs {
        proof.verify()?;
    }
    if proofs.len() >= 2 {
        let first = &proofs[0].content_hash;
        if proofs.iter().all(|proof| &proof.content_hash == first) {
            return Err(Adm4Error::red_line(
                "R3: all review reports share the same content hash (rubber stamp)",
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// R4 AI 产出锚定
// ---------------------------------------------------------------------------

/// AI 叙述必须锚定 GameSpec 路径；空锚定即构造失败（R4）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchoredNarrative {
    pub text: String,
    pub anchors: Vec<SpecRef>,
}

impl AnchoredNarrative {
    pub fn new(text: impl Into<String>, anchors: Vec<SpecRef>) -> Adm4Result<Self> {
        if anchors.is_empty() {
            return Err(Adm4Error::red_line(
                "R4: AI narrative without spec anchors is invented design",
            ));
        }
        Ok(Self {
            text: text.into(),
            anchors,
        })
    }
}

// ---------------------------------------------------------------------------
// R5 参考名扫描
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkinHit {
    pub location: String,
    pub matched_word: String,
}

/// 换皮词表扫描器：词表 = 模板库 game_name + aliases + 品类包参考游戏名。
///
/// # 为什么需要「豁免词」
///
/// 词表是全局的：任何模板认证入库都会把它的 `game_name`/`aliases` 登记进去，其中包括
/// 「本项目导出」的模板——那登记的是**某个项目自己的名字**。对别的项目而言这是必须拦的
/// （B 的产物里出现 A 的名字 = 换皮抄 A）；对 A 自己而言这是必须放行的（C0 规格文档的
/// 标题就是项目名，拦了 A 就永远走不完流水线）。
///
/// 让 `ProjectExport` 干脆不登记词条不行：那样 A 的名字压根不在词表里，B 抄 A 也没人拦，
/// 换皮扫描对「抄另一个项目」直接失效。所以词表照常登记，**扫描时按当前项目豁免自身**。
///
/// # 本类型只是机制，作用域由调用方决定
///
/// 本类型对「豁免词从哪来」不作判断：它按**整词相等**（归一化后逐字相同）剔除调用方
/// 显式给出的词，不做前缀/子串豁免、不顺带豁免同一份模板的别名、不豁免其它任何词。
/// 因此 A 的名字是 "霜落" 时，词表里另一个项目的 "霜落峡谷" 照旧命中 A 的产物。
///
/// 「哪个词在什么条件下才配得上被豁免」是**策略**，不在这一层：它由
/// `adm4_app::AppServices::skin_scanner_for_project` 判定，口径是「该词在全库中的唯一
/// 登记来源是本存档导出的模板」。把策略放在这里做不到——本层看不见模板库，
/// 无从知道一个词面是项目自己的名字还是某个外部游戏的名字（两者可以逐字相同）。
#[derive(Debug, Clone, Default)]
pub struct SkinScanner {
    /// 生效词表（已剔除豁免词）。
    wordlist: Vec<String>,
    /// 实际被豁免的词（归一化形态），供报告/日志如实说明「放行了什么」。
    exempted: Vec<String>,
}

impl SkinScanner {
    /// 无豁免的扫描器（行为与豁免扩展前逐字一致）。
    pub fn new(wordlist: Vec<String>) -> Self {
        Self::with_exemptions(wordlist, Vec::new())
    }

    /// 带豁免词的扫描器：`exemptions` 一般只有一个元素——当前项目名（且已由调用方
    /// 确认该词条**只**由本存档导出的模板登记过，见类型文档的作用域说明）。
    ///
    /// 豁免在构造时就把词条从生效词表里剔掉，`scan` 因此没有第二套判定；
    /// `wordlist()` 返回的即真正会命中的词，不存在「看着在词表里却扫不出来」的错觉。
    pub fn with_exemptions(wordlist: Vec<String>, exemptions: Vec<String>) -> Self {
        let exempted: Vec<String> = normalize_words(exemptions);
        let wordlist = normalize_words(wordlist)
            .into_iter()
            .filter(|word| !exempted.contains(word))
            .collect();
        Self { wordlist, exempted }
    }

    pub fn wordlist(&self) -> &[String] {
        &self.wordlist
    }

    /// 被豁免的词（归一化后）：报告与日志据此说明「哪个名字被当作自身放行」。
    pub fn exempted(&self) -> &[String] {
        &self.exempted
    }

    pub fn scan(&self, location: &str, text: &str) -> Vec<SkinHit> {
        let lowered = text.to_lowercase();
        self.wordlist
            .iter()
            .filter(|word| lowered.contains(word.as_str()))
            .map(|word| SkinHit {
                location: location.to_string(),
                matched_word: word.clone(),
            })
            .collect()
    }
}

/// 单个换皮词的归一化口径：trim + 小写。
///
/// 词表、豁免词、以及「这个词条是谁登记的」溯源查询（`adm4-template` 的
/// `TemplateLibrary::skin_word_registrations`）三处共用本函数。口径一旦分叉，豁免就会
/// 对不上：要么该放的没放（项目被自己的名字拦住），要么该拦的放了（大小写不同的外部
/// 游戏名被当成项目名豁免）。
pub fn normalize_skin_word(word: &str) -> String {
    word.trim().to_lowercase()
}

/// 词表归一化：逐词归一化 + 丢空串（词表与豁免词共用同一口径，否则豁免会对不上）。
fn normalize_words(words: Vec<String>) -> Vec<String> {
    words
        .into_iter()
        .map(|word| normalize_skin_word(&word))
        .filter(|word| !word.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// R6 基数申报
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CardinalityRange {
    pub min: usize,
    pub max: usize,
}

impl CardinalityRange {
    pub fn contains(&self, count: usize) -> bool {
        count >= self.min && count <= self.max
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DroppedItem {
    pub item: String,
    pub reason: String,
}

/// 一对多派生的基数申报：映射规则 + 丢弃清单 + 期望区间对照（R6）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardinalityDeclaration {
    pub rule: String,
    pub produced: usize,
    pub expected: CardinalityRange,
    pub dropped: Vec<DroppedItem>,
}

impl CardinalityDeclaration {
    pub fn within_expectation(&self) -> bool {
        self.expected.contains(self.produced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r1_rejects_nonzero_metric_without_evidence() {
        assert!(MeasuredMetric::new(0.8, Vec::new()).is_err());
        assert!(MeasuredMetric::new(0.0, Vec::new()).is_ok());
        assert!(
            MeasuredMetric::new(
                0.8,
                vec![EvidencePointer {
                    file: "a.json".into(),
                    path: "mechanics/0".into(),
                    observed: "covered".into(),
                }]
            )
            .is_ok()
        );
    }

    #[test]
    fn r2_blocked_reports_items() {
        let derive: Derive<u32> = Derive::Blocked {
            unknown: vec![UnclassifiedItem {
                item: "mystery_entity".into(),
                reason: "no visual form".into(),
            }],
        };
        let error = derive.into_result("asset derivation").unwrap_err();
        assert!(error.message.contains("mystery_entity"));
    }

    #[test]
    fn r3_rubber_stamp_batch_fails() {
        let proof = |hash: &str| ReviewProof {
            reviewer: "ai".into(),
            reviewed_count: 3,
            upstream_count: 3,
            content_hash: hash.into(),
            per_category_evidence: vec![CategoryEvidence {
                category: "consistency".into(),
                checked: "all mechanics".into(),
                conclusion: "ok".into(),
                evidence: Vec::new(),
            }],
        };
        assert!(verify_review_batch(&[proof("h1"), proof("h1")]).is_err());
        assert!(verify_review_batch(&[proof("h1"), proof("h2")]).is_ok());
    }

    #[test]
    fn r4_empty_anchor_rejected() {
        assert!(AnchoredNarrative::new("text", Vec::new()).is_err());
        assert!(AnchoredNarrative::new("text", vec![SpecRef::new("intent/title")]).is_ok());
    }

    #[test]
    fn r5_scan_finds_reference_names_case_insensitive() {
        let scanner = SkinScanner::new(vec!["Plants vs Zombies".into(), "PvZ".into()]);
        let hits = scanner.scan("c2/document.md", "灵感来自 pvz 的克制循环");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched_word, "pvz");
        assert!(scanner.scan("c2/document.md", "原创克制循环").is_empty());
    }

    /// R5 豁免：同一份词表下，「项目自己的名字」放行，其它词一条不放松。
    ///
    /// 三条一起钉住，因为它们是同一个判定的三个方向，分开写会掩盖「豁免范围被放宽」：
    /// ① A 的产物含 A 不拦（否则 C0 文档标题过不去）；
    /// ② B 的产物含 A 要拦（否则抄另一个项目无人管）；
    /// ③ 逆向来源的外部游戏名，两种项目视角下都照旧拦。
    #[test]
    fn r5_exemption_covers_own_project_name_only() {
        // 词表：A 项目自己的名字（由 A 的「另存模板」认证时登记）+ 一款逆向的外部游戏名。
        let wordlist = || vec!["霜落峡谷".to_string(), "晨昏防线".to_string()];
        let own = SkinScanner::with_exemptions(wordlist(), vec!["霜落峡谷".into()]);
        let other = SkinScanner::with_exemptions(wordlist(), vec!["晨星台地".into()]);

        // ① A 自己的产物含 A 不被拦（C0 文档标题就是项目名）。
        assert!(own.scan("c0/document.md", "霜落峡谷设计规格").is_empty());
        // ② B 的产物含 A 被拦（B 必须改写理由完成换皮）。
        let hits = other.scan("c0/document.md", "模板预填自霜落峡谷");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].matched_word, "霜落峡谷");
        // ③ 逆向来源的外部游戏名，两种视角下拦截行为一条不变。
        for scanner in [&own, &other] {
            assert_eq!(
                scanner.scan("c2/document.md", "参考晨昏防线的波次").len(),
                1
            );
        }

        // 豁免只按整词相等：另一个项目名恰好是自身名字的子串时，它照旧命中
        // （代价是 A 会被「霜落」拦下，需人工处理；这是刻意不放宽的边界）。
        let with_substring = SkinScanner::with_exemptions(
            vec!["霜落峡谷".into(), "霜落".into()],
            vec!["霜落峡谷".into()],
        );
        assert_eq!(
            with_substring
                .scan("c0/document.md", "霜落峡谷设计规格")
                .len(),
            1
        );
        assert_eq!(own.wordlist(), ["晨昏防线".to_string()]);
        assert_eq!(own.exempted(), ["霜落峡谷".to_string()]);
        // 无豁免构造与扩展前逐字等价。
        assert_eq!(SkinScanner::new(wordlist()).wordlist().len(), 2);
        assert!(SkinScanner::new(wordlist()).exempted().is_empty());
    }

    #[test]
    fn r6_range_check() {
        let declaration = CardinalityDeclaration {
            rule: "entity->asset 1:2".into(),
            produced: 12,
            expected: CardinalityRange { min: 4, max: 10 },
            dropped: Vec::new(),
        };
        assert!(!declaration.within_expectation());
    }
}
