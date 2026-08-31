//! SDK 知识库：SDK 资源登记 + 三态审批流（待审 → 批准/拒绝，署名必填）。
//!
//! 归宿定位（`docs/plan/05` §2.2）：二版 SDK 知识库的「新增/审批队列/已索引资源」在四版的落点。
//! 审批产物（已批准资源）是 Phase 2 构建集成的前置——未审批的资源不得进入构建。
//! 本期落地数据模型 + 审批流；构建集成留 Phase 2。
//!
//! 落盘：全局 `data_root/config/sdk_knowledge.json`（跨项目共享，非单存档内容），
//! 与二版 `sdk_knowledge_service.rs` 的 data_root 全局语义一致；改用 serde JSON 而非自定义行格式。

use adm4_archive::DataRoot;
use adm4_foundation::{
    Adm4Error, Adm4Result, UtcTimestamp, new_id, read_json_file, write_json_file,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// SDK 资源审批状态：`Pending`（待审）→ `Approved`（已批准）/ `Rejected`（已拒绝）。
///
/// 审批是分叉终态而非线性链：只有 `Pending` 能被裁决，裁决后即终态，不可再改（重复审批 blocked）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SdkReviewStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
}

impl SdkReviewStatus {
    /// 中文展示名（UI 不必自己映射枚举）。
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Pending => "待审核",
            Self::Approved => "已批准",
            Self::Rejected => "已拒绝",
        }
    }

    /// 是否已裁决（终态）。
    pub fn is_decided(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

/// 一条 SDK 资源登记与审批记录。
///
/// 审批署名四要素（`status`/`reviewer`/`reviewed_at`/`review_note`）内联进本结构，
/// 不另开并行 map（参考 `NaJustification` 的 F3 合并教训：并行署名会留下幽灵记录）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkRecord {
    pub id: String,
    pub sdk_name: String,
    pub url: String,
    /// 资源类别（默认 `custom`）。
    #[serde(default = "default_category")]
    pub category: String,
    /// 目标引擎（默认 `Unity`；Phase 2 构建集成消费）。
    #[serde(default = "default_engines")]
    pub target_engines: String,
    /// 目标平台（默认 `windows-desktop`）。
    #[serde(default = "default_platforms")]
    pub target_platforms: String,
    /// 取用目的（人读说明）。
    #[serde(default)]
    pub purpose: String,
    pub status: SdkReviewStatus,
    /// 审批署名（待审时空串）。
    #[serde(default)]
    pub reviewer: String,
    /// 审批时间 ISO8601（待审时空串）。
    #[serde(default)]
    pub reviewed_at: String,
    /// 审批结论/拒绝理由（待审时空串）。
    #[serde(default)]
    pub review_note: String,
    /// 登记时间 ISO8601。
    #[serde(default)]
    pub created_at: String,
}

fn default_category() -> String {
    "custom".into()
}
fn default_engines() -> String {
    "Unity".into()
}
fn default_platforms() -> String {
    "windows-desktop".into()
}

/// 审批队列快照：记录清单 + 三态计数（UI 顶部计数条数据源）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkSnapshot {
    pub records: Vec<SdkRecord>,
    pub pending_count: usize,
    pub approved_count: usize,
    pub rejected_count: usize,
}

/// SDK 知识库：全局审批队列的权威状态（落 `config/sdk_knowledge.json`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SdkKnowledgeBase {
    #[serde(default)]
    pub records: Vec<SdkRecord>,
}

impl SdkKnowledgeBase {
    fn path(data_root: &DataRoot) -> PathBuf {
        data_root.config_dir().join("sdk_knowledge.json")
    }

    /// 读取知识库；文件缺失 = 空库（首次使用）。
    pub fn load(data_root: &DataRoot) -> Adm4Result<Self> {
        let path = Self::path(data_root);
        if !path.is_file() {
            return Ok(Self::default());
        }
        read_json_file(&path)
    }

    /// 原子落盘。
    pub fn save(&self, data_root: &DataRoot) -> Adm4Result<()> {
        write_json_file(&Self::path(data_root), self)
    }

    /// 登记一条待审 SDK 资源；`sdk_name`/`url` 非空必填，返回新记录 id。
    pub fn add_pending(
        &mut self,
        sdk_name: &str,
        url: &str,
        category: &str,
        purpose: &str,
    ) -> Adm4Result<String> {
        let sdk_name = sdk_name.trim();
        let url = url.trim();
        if sdk_name.is_empty() {
            return Err(Adm4Error::invalid_input("SDK 资源名不能为空"));
        }
        if url.is_empty() {
            return Err(Adm4Error::invalid_input("SDK 资源 URL/来源不能为空"));
        }
        let category = category.trim();
        let id = new_id("sdk");
        self.records.push(SdkRecord {
            id: id.clone(),
            sdk_name: sdk_name.to_string(),
            url: url.to_string(),
            category: if category.is_empty() {
                default_category()
            } else {
                category.to_string()
            },
            target_engines: default_engines(),
            target_platforms: default_platforms(),
            purpose: purpose.trim().to_string(),
            status: SdkReviewStatus::Pending,
            reviewer: String::new(),
            reviewed_at: String::new(),
            review_note: String::new(),
            created_at: UtcTimestamp::now().to_iso8601(),
        });
        Ok(id)
    }

    fn record_mut(&mut self, id: &str) -> Adm4Result<&mut SdkRecord> {
        self.records
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| Adm4Error::not_found(format!("SDK 记录 {id} 不存在")))
    }

    /// 裁决一条记录（批准/拒绝）：署名 + 意见双必填（R3），仅 `Pending` 可裁决，重复裁决 blocked。
    fn decide(
        &mut self,
        id: &str,
        target: SdkReviewStatus,
        reviewer: &str,
        note: &str,
    ) -> Adm4Result<()> {
        let reviewer = reviewer.trim();
        let note = note.trim();
        if reviewer.is_empty() {
            return Err(Adm4Error::invalid_input(
                "SDK 审批必须署名评审人（R3 评审工作量证明）",
            ));
        }
        if note.is_empty() {
            return Err(Adm4Error::invalid_input(
                "SDK 审批必须填写审核结论（R3 评审工作量证明）",
            ));
        }
        let record = self.record_mut(id)?;
        if record.status.is_decided() {
            return Err(Adm4Error::blocked(format!(
                "SDK 记录 {id} 已是终态 {:?}，不能再次裁决",
                record.status
            )));
        }
        record.status = target;
        record.reviewer = reviewer.to_string();
        record.reviewed_at = UtcTimestamp::now().to_iso8601();
        record.review_note = note.to_string();
        Ok(())
    }

    /// 批准一条待审记录。
    pub fn approve(&mut self, id: &str, reviewer: &str, note: &str) -> Adm4Result<()> {
        self.decide(id, SdkReviewStatus::Approved, reviewer, note)
    }

    /// 拒绝一条待审记录。
    pub fn reject(&mut self, id: &str, reviewer: &str, note: &str) -> Adm4Result<()> {
        self.decide(id, SdkReviewStatus::Rejected, reviewer, note)
    }

    /// 队列快照（记录清单 + 三态计数）。
    pub fn snapshot(&self) -> SdkSnapshot {
        let mut pending = 0;
        let mut approved = 0;
        let mut rejected = 0;
        for record in &self.records {
            match record.status {
                SdkReviewStatus::Pending => pending += 1,
                SdkReviewStatus::Approved => approved += 1,
                SdkReviewStatus::Rejected => rejected += 1,
            }
        }
        SdkSnapshot {
            records: self.records.clone(),
            pending_count: pending,
            approved_count: approved,
            rejected_count: rejected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> (SdkKnowledgeBase, String) {
        let mut base = SdkKnowledgeBase::default();
        let id = base
            .add_pending(
                "DOTween",
                "https://dotween.demigiant.com",
                "animation",
                "补间动画",
            )
            .unwrap();
        (base, id)
    }

    #[test]
    fn add_then_approve_records_signature() {
        let (mut base, id) = seed();
        base.approve(&id, "策划A", "许可范围内使用").unwrap();
        let record = base.records.iter().find(|r| r.id == id).unwrap();
        assert_eq!(record.status, SdkReviewStatus::Approved);
        assert_eq!(record.reviewer, "策划A");
        assert!(!record.reviewed_at.is_empty());
        assert_eq!(record.review_note, "许可范围内使用");
    }

    #[test]
    fn add_then_reject_records_reason() {
        let (mut base, id) = seed();
        base.reject(&id, "法务B", "许可证不兼容").unwrap();
        let record = base.records.iter().find(|r| r.id == id).unwrap();
        assert_eq!(record.status, SdkReviewStatus::Rejected);
        assert_eq!(record.review_note, "许可证不兼容");
    }

    #[test]
    fn empty_signature_or_note_blocked() {
        let (mut base, id) = seed();
        assert!(base.approve(&id, "  ", "结论").is_err());
        assert!(base.approve(&id, "策划A", "  ").is_err());
        // 被拒后仍是 Pending，可再正常裁决。
        assert_eq!(base.records[0].status, SdkReviewStatus::Pending);
        assert!(base.approve(&id, "策划A", "结论").is_ok());
    }

    #[test]
    fn double_decision_blocked() {
        let (mut base, id) = seed();
        base.approve(&id, "策划A", "首次批准").unwrap();
        let err = base.reject(&id, "法务B", "想改判").unwrap_err();
        assert!(err.to_string().contains("终态"), "{err}");
    }

    #[test]
    fn add_requires_name_and_url() {
        let mut base = SdkKnowledgeBase::default();
        assert!(base.add_pending("  ", "https://x", "", "").is_err());
        assert!(base.add_pending("X", "  ", "", "").is_err());
    }

    #[test]
    fn snapshot_counts_by_status() {
        let mut base = SdkKnowledgeBase::default();
        let a = base.add_pending("A", "https://a", "", "").unwrap();
        let b = base.add_pending("B", "https://b", "", "").unwrap();
        base.add_pending("C", "https://c", "", "").unwrap();
        base.approve(&a, "r", "ok").unwrap();
        base.reject(&b, "r", "no").unwrap();
        let snap = base.snapshot();
        assert_eq!(snap.pending_count, 1);
        assert_eq!(snap.approved_count, 1);
        assert_eq!(snap.rejected_count, 1);
        assert_eq!(snap.records.len(), 3);
    }
}
