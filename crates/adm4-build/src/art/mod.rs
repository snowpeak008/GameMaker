//! 美术线（册 08）：设计阶段的风格锚点门 + （后续波次的）资产生产。
//!
//! 本波（G2）只做 [`style_anchor`]：**设计阶段**的风格门（册 08 §2，选项 A）。
//! 它不是 Phase 2 的一个 P 段——风格在冻结之前由人看真图定下来，Phase 2 只消费
//! 锁定的 `style_anchor_set`，绝不重造风格（制品注册表里
//! [`crate::ArtifactKind::StyleAnchorSet`] 声明为外部输入正是这个意思）。
//!
//! 资产生产（`asset_producer` / `budget` / `cache` / `genome_backfill`，册 08 §4）留 G3。

pub mod style_anchor;
