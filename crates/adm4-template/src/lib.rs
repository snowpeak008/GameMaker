//! V4 模板体系与逆向工具链。
//!
//! 模板 = 知名成熟游戏在当前选项体系下的逆向答卷（参考点，非复刻目标）。
//! 产线五步：S1 检索（[`FileCorpusChannel`]/[`ManualEvidenceChannel`]）→
//! S2 AI 映射（[`MappingService`]）→ S3 交叉核验（[`CrossCheckService`]）→
//! S4 人工审核 → S5 认证入库（[`TemplateLibrary`]）。
//! 认证状态机固定 `Draft→Mapped→CrossChecked→HumanReviewed→Certified` 只进不跳；
//! 只有 Certified 模板可入库预填。

mod corpus;
mod coverage;
mod crosscheck;
mod evidence;
mod library;
mod mapping;
mod model;

pub use corpus::FileCorpusChannel;
pub use coverage::{CoverageReport, LevelCoverage, compute_coverage};
pub use crosscheck::{
    CROSSCHECK_PURPOSE, CrossCheckEntry, CrossCheckReport, CrossCheckService, CrossCheckVerdict,
};
pub use evidence::{
    EvidenceCandidate, EvidenceQuery, EvidenceSearchChannel, ManualEvidenceChannel,
};
pub use library::{
    SkinWordRegistration, SkinWordlist, TemplateLibrary, load_skin_wordlist, save_skin_wordlist,
};
pub use mapping::{MAPPING_PURPOSE, MappingService};
pub use model::{
    Certification, CertificationStatus, Confidence, CrossCheckProof, Evidence, SourceType,
    Template, TemplateAnswer, TemplateOrigin, TemplateSelectedOption, TemplateSelectedOptionRef,
};
