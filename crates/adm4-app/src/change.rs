//! 补充开发变更流：冻结后的设计变更请求登记 + 线性推进状态机（署名必填）。
//!
//! 归宿定位（`docs/plan/05` §2.2）：二版「补充开发（增量重跑受影响段）」在四版的落点。
//! 本期落地数据模型 + 变更请求生命周期（起草 → 影响分析 → 排期 → 已应用，可拒绝）；
//! 「增量重跑受影响段」不新造引擎——`affected_segments`（C0..C6）映射为对既有
//! `AppServices::pipeline_run(archive_id, from, to)` 的调用参数（视图侧按钮接线）。
//!
//! 落盘：项目内 `content/change_requests.json`（authoring_state.json 的兄弟文件，纳入存档指纹）。
//! 采用与 `frozen/`、`pipeline/` 产物一致的「事务外补写 + refresh_fingerprint」范式
//! （变更清单不属于创作态，不经 AuthoringEngine）。

use adm4_foundation::{
    Adm4Error, Adm4Result, UtcTimestamp, new_id, read_json_file, write_json_file,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 合法的受影响段（对齐流水线 C0-C6 阶段 id）。
const DESIGN_SEGMENTS: [&str; 7] = ["C0", "C1", "C2", "C3", "C4", "C5", "C6"];

/// 变更请求状态机。
///
/// 线性主链：`Drafted` → `ImpactAnalyzed` → `Scheduled` → `Applied`；
/// 任意非终态可分叉到 `Rejected`。`Applied`/`Rejected` 为终态，不可再推进。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    #[default]
    Drafted,
    ImpactAnalyzed,
    Scheduled,
    Applied,
    Rejected,
}

impl ChangeStatus {
    /// 中文展示名（UI 不必自己映射枚举）。
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Drafted => "已起草",
            Self::ImpactAnalyzed => "已影响分析",
            Self::Scheduled => "已排期",
            Self::Applied => "已应用",
            Self::Rejected => "已拒绝",
        }
    }

    /// 序列化令牌（与 serde 的 snake_case 一致）；UI 回调用字符串往返状态。
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Drafted => "drafted",
            Self::ImpactAnalyzed => "impact_analyzed",
            Self::Scheduled => "scheduled",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }

    /// 从令牌解析状态（未知令牌 = None）。
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "drafted" => Some(Self::Drafted),
            "impact_analyzed" => Some(Self::ImpactAnalyzed),
            "scheduled" => Some(Self::Scheduled),
            "applied" => Some(Self::Applied),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }

    /// 线性主链的下一步（`None` 表示无后继）；公开供 UI 计算「推进」按钮目标。
    pub fn next(self) -> Option<Self> {
        self.linear_next()
    }

    /// 是否终态（不可再推进）。
    fn is_terminal(self) -> bool {
        matches!(self, Self::Applied | Self::Rejected)
    }

    /// 线性主链的下一步（`None` 表示无后继）。
    fn linear_next(self) -> Option<Self> {
        match self {
            Self::Drafted => Some(Self::ImpactAnalyzed),
            Self::ImpactAnalyzed => Some(Self::Scheduled),
            Self::Scheduled => Some(Self::Applied),
            Self::Applied | Self::Rejected => None,
        }
    }
}

/// 一条补充开发变更请求。
///
/// 推进署名（`last_actor`/`last_note`/`updated_at`）内联，不另开并行 map
/// （参考 `NaJustification` 的 F3 合并教训）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeRequest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub requested_by: String,
    pub created_at: String,
    pub status: ChangeStatus,
    /// 受影响的设计段（C0..C6 子集）；影响分析后填。
    #[serde(default)]
    pub affected_segments: Vec<String>,
    /// 关联的冻结版本（0 = 未绑定）。
    #[serde(default)]
    pub target_frozen_version: u32,
    /// 最近一次推进的署名评审人（起草时空串）。
    #[serde(default)]
    pub last_actor: String,
    /// 最近一次推进的结论/理由（起草时空串）。
    #[serde(default)]
    pub last_note: String,
    /// 最近一次状态变更时间 ISO8601。
    #[serde(default)]
    pub updated_at: String,
}

/// 项目内变更请求清单（`content/change_requests.json` 的权威状态）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChangeLog {
    #[serde(default)]
    pub requests: Vec<ChangeRequest>,
}

impl ChangeLog {
    fn path(content_dir: &Path) -> PathBuf {
        content_dir.join("change_requests.json")
    }

    /// 读取变更清单；文件缺失 = 空清单（尚无补充开发）。
    pub fn load(content_dir: &Path) -> Adm4Result<Self> {
        let path = Self::path(content_dir);
        if !path.is_file() {
            return Ok(Self::default());
        }
        read_json_file(&path)
    }

    /// 落盘（调用方负责 refresh_fingerprint）。
    pub fn save(&self, content_dir: &Path) -> Adm4Result<()> {
        write_json_file(&Self::path(content_dir), self)
    }

    /// 登记一条变更请求（起草态）；`title`/`requested_by` 非空必填，返回新 id。
    pub fn add(
        &mut self,
        title: &str,
        description: &str,
        requested_by: &str,
        target_frozen_version: u32,
    ) -> Adm4Result<String> {
        let title = title.trim();
        let requested_by = requested_by.trim();
        if title.is_empty() {
            return Err(Adm4Error::invalid_input("变更请求标题不能为空"));
        }
        if requested_by.is_empty() {
            return Err(Adm4Error::invalid_input("变更请求必须署名申请人"));
        }
        let id = new_id("chg");
        self.requests.push(ChangeRequest {
            id: id.clone(),
            title: title.to_string(),
            description: description.trim().to_string(),
            requested_by: requested_by.to_string(),
            created_at: UtcTimestamp::now().to_iso8601(),
            status: ChangeStatus::Drafted,
            affected_segments: Vec::new(),
            target_frozen_version,
            last_actor: String::new(),
            last_note: String::new(),
            updated_at: String::new(),
        });
        Ok(id)
    }

    fn request_mut(&mut self, id: &str) -> Adm4Result<&mut ChangeRequest> {
        self.requests
            .iter_mut()
            .find(|request| request.id == id)
            .ok_or_else(|| Adm4Error::not_found(format!("变更请求 {id} 不存在")))
    }

    /// 记录影响分析：填受影响段并把状态推到 `ImpactAnalyzed`。
    ///
    /// 段必须是 C0..C6 子集且非空；仅 `Drafted`/`ImpactAnalyzed`（复评）可设，其余状态 blocked。
    pub fn set_impact(&mut self, id: &str, affected_segments: &[String]) -> Adm4Result<()> {
        let normalized = normalize_segments(affected_segments)?;
        let request = self.request_mut(id)?;
        if !matches!(
            request.status,
            ChangeStatus::Drafted | ChangeStatus::ImpactAnalyzed
        ) {
            return Err(Adm4Error::blocked(format!(
                "变更请求 {id} 处于 {:?}，不能再做影响分析",
                request.status
            )));
        }
        request.affected_segments = normalized;
        request.status = ChangeStatus::ImpactAnalyzed;
        request.updated_at = UtcTimestamp::now().to_iso8601();
        Ok(())
    }

    /// 推进状态：署名 + 结论双必填（R3），只允许线性下一步或分叉到 `Rejected`，跳级 blocked。
    pub fn advance(
        &mut self,
        id: &str,
        target: ChangeStatus,
        actor: &str,
        note: &str,
    ) -> Adm4Result<()> {
        let actor = actor.trim();
        let note = note.trim();
        if actor.is_empty() {
            return Err(Adm4Error::invalid_input(
                "推进变更请求必须署名评审人（R3 评审工作量证明）",
            ));
        }
        if note.is_empty() {
            return Err(Adm4Error::invalid_input(
                "推进变更请求必须填写结论（R3 评审工作量证明）",
            ));
        }
        let request = self.request_mut(id)?;
        if request.status.is_terminal() {
            return Err(Adm4Error::blocked(format!(
                "变更请求 {id} 已是终态 {:?}，不能再推进",
                request.status
            )));
        }
        let allowed =
            target == ChangeStatus::Rejected || Some(target) == request.status.linear_next();
        if !allowed {
            return Err(Adm4Error::blocked(format!(
                "变更请求 {id} 不能从 {:?} 跳到 {:?}（只能推进到下一步或拒绝）",
                request.status, target
            )));
        }
        request.status = target;
        request.last_actor = actor.to_string();
        request.last_note = note.to_string();
        request.updated_at = UtcTimestamp::now().to_iso8601();
        Ok(())
    }
}

/// 规范化受影响段：去空白、大写、去重（保序）、必须是 C0..C6 且非空。
fn normalize_segments(segments: &[String]) -> Adm4Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for raw in segments {
        let seg = raw.trim().to_ascii_uppercase();
        if seg.is_empty() {
            continue;
        }
        if !DESIGN_SEGMENTS.contains(&seg.as_str()) {
            return Err(Adm4Error::invalid_input(format!(
                "受影响段 {seg} 非法（合法值：C0..C6）"
            )));
        }
        if !out.contains(&seg) {
            out.push(seg);
        }
    }
    if out.is_empty() {
        return Err(Adm4Error::invalid_input(
            "影响分析至少需要指定一个受影响段（C0..C6）",
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn seed() -> (ChangeLog, String) {
        let mut log = ChangeLog::default();
        let id = log
            .add("新增精英怪波次", "第 8 关加入精英单位", "策划A", 2)
            .unwrap();
        (log, id)
    }

    #[test]
    fn add_set_impact_advance_full_chain() {
        let (mut log, id) = seed();
        assert_eq!(log.requests[0].status, ChangeStatus::Drafted);

        log.set_impact(&id, &seg(&["C2", "C3"])).unwrap();
        let r = log.requests.iter().find(|r| r.id == id).unwrap();
        assert_eq!(r.status, ChangeStatus::ImpactAnalyzed);
        assert_eq!(r.affected_segments, vec!["C2", "C3"]);

        log.advance(&id, ChangeStatus::Scheduled, "主程B", "排入 v3 迭代")
            .unwrap();
        log.advance(&id, ChangeStatus::Applied, "主程B", "已重跑 C2..C3")
            .unwrap();
        let r = log.requests.iter().find(|r| r.id == id).unwrap();
        assert_eq!(r.status, ChangeStatus::Applied);
        assert_eq!(r.last_actor, "主程B");
    }

    #[test]
    fn skip_level_blocked() {
        let (mut log, id) = seed();
        log.set_impact(&id, &seg(&["C0"])).unwrap();
        // ImpactAnalyzed 直接跳到 Applied（越过 Scheduled）应被拦。
        let err = log
            .advance(&id, ChangeStatus::Applied, "x", "y")
            .unwrap_err();
        assert!(err.to_string().contains("跳"), "{err}");
    }

    #[test]
    fn advance_requires_signature_and_note() {
        let (mut log, id) = seed();
        log.set_impact(&id, &seg(&["C1"])).unwrap();
        assert!(
            log.advance(&id, ChangeStatus::Scheduled, "  ", "结论")
                .is_err()
        );
        assert!(
            log.advance(&id, ChangeStatus::Scheduled, "评审", "  ")
                .is_err()
        );
        // 被拒后仍是 ImpactAnalyzed，可再正常推进。
        assert_eq!(log.requests[0].status, ChangeStatus::ImpactAnalyzed);
        assert!(
            log.advance(&id, ChangeStatus::Scheduled, "评审", "结论")
                .is_ok()
        );
    }

    #[test]
    fn reject_from_any_non_terminal() {
        let (mut log, id) = seed();
        // 起草态直接拒绝（分叉终态）。
        log.advance(&id, ChangeStatus::Rejected, "负责人", "需求撤回")
            .unwrap();
        assert_eq!(log.requests[0].status, ChangeStatus::Rejected);
        // 终态不可再推进。
        let err = log
            .advance(&id, ChangeStatus::Scheduled, "x", "y")
            .unwrap_err();
        assert!(err.to_string().contains("终态"), "{err}");
    }

    #[test]
    fn add_requires_title_and_requester() {
        let mut log = ChangeLog::default();
        assert!(log.add("  ", "d", "策划", 0).is_err());
        assert!(log.add("标题", "d", "  ", 0).is_err());
    }

    #[test]
    fn impact_rejects_empty_or_invalid_segments() {
        let (mut log, id) = seed();
        assert!(log.set_impact(&id, &seg(&[])).is_err());
        assert!(log.set_impact(&id, &seg(&["C9"])).is_err());
        // 大小写与去重规范化。
        log.set_impact(&id, &seg(&["c1", "C1", "c2"])).unwrap();
        assert_eq!(log.requests[0].affected_segments, vec!["C1", "C2"]);
    }

    #[test]
    fn impact_blocked_after_applied() {
        let (mut log, id) = seed();
        log.set_impact(&id, &seg(&["C0"])).unwrap();
        log.advance(&id, ChangeStatus::Scheduled, "a", "n").unwrap();
        log.advance(&id, ChangeStatus::Applied, "a", "n").unwrap();
        assert!(log.set_impact(&id, &seg(&["C1"])).is_err());
    }
}
