//! V4 跨层契约：共享值类型 + 七条红线的机器化类型。
//!
//! 红线（redesign 设计 05 号文档）：
//! - R1 指标即测量：`MeasuredMetric`
//! - R2 未知即停：`Derive<T>`
//! - R3 评审最低工作量证明：`ReviewProof`
//! - R4 AI 产出锚定：`AnchoredNarrative`
//! - R5 参考名扫描全程在线：`SkinScanner`
//! - R6 基数申报：`CardinalityDeclaration`
//! - R7 fallback 禁令：由 AI 层与流水线状态机强制（无类型兜底路径）

mod red_lines;
mod values;

pub use red_lines::{
    AnchoredNarrative, CardinalityDeclaration, CardinalityRange, CategoryEvidence, Derive,
    DroppedItem, EvidencePointer, MeasuredMetric, ReviewProof, SkinHit, SkinScanner,
    UnclassifiedItem, verify_review_batch,
};
pub use values::{MatrixCell, SpecRef, TypedValue, ValueConstraint, ValueKind};
