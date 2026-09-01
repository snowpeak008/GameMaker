//! 资产预算门（册 08 §4.2）：**首次付费生产前必须人工确认**（R3 署名 + 结论），
//! 生产中超出申报张数必须停下再确认（与 R6 基数申报同源）。
//!
//! 预算与实耗都落盘（R1：报实测计数）。状态迁移只有一条正路：
//! `Draft`（申报）→ `Approved`（署名确认）→ 生产扣减 → 超额自动回 `Exhausted`（再申再批）。

use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

/// 预算状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetStatus {
    /// 已申报未批准：一张都不许产。
    #[default]
    Draft,
    /// 已署名批准：可产 `approved_calls` 张。
    Approved,
    /// 批额已用尽：继续生产需重新申报批准。
    Exhausted,
}

/// 一次预算确认（R3：署名 + 结论双必填）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetApproval {
    pub actor: String,
    pub note: String,
    pub at: String,
    /// 本次批准的生成调用数上限。
    pub approved_calls: usize,
}

/// 资产生产预算（`asset_budget.json`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBudget {
    pub status: BudgetStatus,
    /// 申报：本轮要产的资产清单（人工门审的就是这张单子）。
    pub declared_assets: Vec<String>,
    /// 申报的预计生成调用数（缓存命中不耗额度，实耗 ≤ 申报是常态）。
    pub declared_calls: usize,
    /// 历次批准记录（只追加；最近一条是当前生效额度）。
    pub approvals: Vec<BudgetApproval>,
    /// 实耗：真实发出的图像生成调用数（R1 实测计数，缓存命中不计）。
    pub consumed_calls: usize,
}

impl AssetBudget {
    /// 申报一轮生产（覆盖草稿；已批准的预算不许改单——改单必须重批）。
    pub fn declare(assets: Vec<String>, calls: usize) -> Adm4Result<Self> {
        if assets.is_empty() {
            return Err(Adm4Error::invalid_input(
                "预算申报的资产清单为空：没有要产的东西就不需要预算门",
            ));
        }
        if calls == 0 {
            return Err(Adm4Error::invalid_input(
                "预算申报的调用数为 0：一张都不产的申报没有意义",
            ));
        }
        Ok(Self {
            status: BudgetStatus::Draft,
            declared_assets: assets,
            declared_calls: calls,
            approvals: Vec::new(),
            consumed_calls: 0,
        })
    }

    /// 人工批准（R3：署名 + 结论必填；只有 Draft/Exhausted 可批——重复批 Approved 是空章）。
    pub fn approve(&mut self, actor: &str, note: &str, at: &str) -> Adm4Result<()> {
        let actor = actor.trim();
        let note = note.trim();
        if actor.is_empty() {
            return Err(Adm4Error::red_line(
                "R3：预算批准必须署名（首次付费生产是人工门，禁止自动放行）",
            ));
        }
        if note.is_empty() {
            return Err(Adm4Error::red_line(
                "R3：预算批准必须写结论（署名而不给结论不构成评审）",
            ));
        }
        match self.status {
            BudgetStatus::Draft | BudgetStatus::Exhausted => {}
            BudgetStatus::Approved => {
                return Err(Adm4Error::conflict(
                    "预算已在批准状态：额度未用尽时重复批准是空章（用尽后会自动回到待批）",
                ));
            }
        }
        self.approvals.push(BudgetApproval {
            actor: actor.to_string(),
            note: note.to_string(),
            at: at.to_string(),
            approved_calls: self.declared_calls,
        });
        self.status = BudgetStatus::Approved;
        Ok(())
    }

    /// 当前生效的剩余额度。
    pub fn remaining_calls(&self) -> usize {
        let approved: usize = self
            .approvals
            .iter()
            .map(|approval| approval.approved_calls)
            .sum();
        approved.saturating_sub(self.consumed_calls)
    }

    /// 生产前的放行判定：未批准 / 额度用尽 → Err（生产循环每次真调用前都问一遍）。
    pub fn authorize_call(&self) -> Adm4Result<()> {
        match self.status {
            BudgetStatus::Draft => Err(Adm4Error::blocked(format!(
                "资产预算未批准：申报了 {} 个资产（预计 {} 次生成调用），\
                 请人工署名批准后再生产（R3 首次付费确认）",
                self.declared_assets.len(),
                self.declared_calls
            ))),
            BudgetStatus::Exhausted => Err(Adm4Error::blocked(format!(
                "资产预算额度已用尽（批准 {} 次、实耗 {} 次）：超出申报必须重新人工确认（R6）",
                self.consumed_calls + self.remaining_calls(),
                self.consumed_calls
            ))),
            BudgetStatus::Approved => {
                if self.remaining_calls() == 0 {
                    return Err(Adm4Error::blocked(
                        "资产预算剩余额度为 0：请重新批准后继续（不静默超支）",
                    ));
                }
                Ok(())
            }
        }
    }

    /// 记一次真实生成调用（缓存命中**不要**调它——缓存不花钱，不占额度）。
    pub fn consume_call(&mut self) -> Adm4Result<()> {
        self.authorize_call()?;
        self.consumed_calls += 1;
        if self.remaining_calls() == 0 {
            self.status = BudgetStatus::Exhausted;
        }
        Ok(())
    }

    /// 一行摘要（R1：计数说话）。
    pub fn summary(&self) -> String {
        format!(
            "预算状态 {:?}：申报 {} 资产 / {} 次调用，批准 {} 轮，实耗 {} 次，剩余 {} 次",
            self.status,
            self.declared_assets.len(),
            self.declared_calls,
            self.approvals.len(),
            self.consumed_calls,
            self.remaining_calls()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> AssetBudget {
        AssetBudget::declare(vec!["T_Guard".into(), "UI_HudPanel".into()], 2).expect("申报")
    }

    /// 主链：申报 → 未批不许产 → 批准 → 扣减 → 用尽自动回待批 → 再批可续。
    #[test]
    fn budget_gate_blocks_until_approved_and_stops_at_the_cap() {
        let mut budget = declared();
        let error = budget.authorize_call().expect_err("未批准必须拦");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
        assert!(error.message.contains("R3"), "{}", error.message);

        budget
            .approve("制作人甲", "首轮 2 张，成本可接受", "2026-08-31T00:00:00Z")
            .expect("批准");
        budget.consume_call().expect("第 1 次");
        budget.consume_call().expect("第 2 次");
        assert_eq!(budget.status, BudgetStatus::Exhausted);
        let error = budget.consume_call().expect_err("超出申报必须停");
        assert!(error.message.contains("重新人工确认"), "{}", error.message);

        // 再批一轮（Exhausted 可批）：额度累加，历史只追加。
        budget
            .approve("制作人甲", "追加 2 张返工额度", "2026-08-31T01:00:00Z")
            .expect("再批");
        assert_eq!(budget.approvals.len(), 2);
        assert_eq!(budget.remaining_calls(), 2);
        budget.consume_call().expect("续产");
        assert_eq!(budget.consumed_calls, 3);
    }

    /// R3：匿名 / 无结论 / 重复批 Approved 全拒。
    #[test]
    fn approval_requires_signature_and_is_not_a_rubber_stamp() {
        let mut budget = declared();
        assert!(budget.approve("  ", "结论", "now").is_err());
        assert!(budget.approve("甲", "  ", "now").is_err());
        budget.approve("甲", "结论", "now").expect("批准");
        let error = budget.approve("乙", "再盖一章", "now").expect_err("空章");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Conflict);
    }

    /// 申报自检 + serde 往返 + 旧档兼容。
    #[test]
    fn declaration_validates_and_round_trips() {
        assert!(AssetBudget::declare(Vec::new(), 3).is_err());
        assert!(AssetBudget::declare(vec!["T_Guard".into()], 0).is_err());

        let budget = declared();
        let json = serde_json::to_string(&budget).expect("序列化");
        let back: AssetBudget = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, budget);

        // 旧档缺字段：全默认（Draft / 空清单 / 0 额度）——最保守的一侧，产不了任何东西。
        let legacy: AssetBudget = serde_json::from_str("{}").expect("旧档");
        assert_eq!(legacy.status, BudgetStatus::Draft);
        assert!(legacy.authorize_call().is_err());
    }
}
