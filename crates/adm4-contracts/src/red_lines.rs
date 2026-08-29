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
#[derive(Debug, Clone, Default)]
pub struct SkinScanner {
    wordlist: Vec<String>,
}

impl SkinScanner {
    pub fn new(wordlist: Vec<String>) -> Self {
        Self {
            wordlist: wordlist
                .into_iter()
                .map(|word| word.trim().to_lowercase())
                .filter(|word| !word.is_empty())
                .collect(),
        }
    }

    pub fn wordlist(&self) -> &[String] {
        &self.wordlist
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
