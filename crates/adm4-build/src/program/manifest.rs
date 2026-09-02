//! 薄运行时清单（册 09 §3）：目标、durable 状态文件、引擎指南引用、proof 要求、失败修复循环。
//!
//! 清单**极短**且只转述可玩切片与风险计划里已有的事实：它是给开发轮次读的"任务卡"，
//! 不是第二份设计文档。Markdown 渲染与 JSON 契约字段一一对应——Markdown 里每一条非空事实
//! 都能在 [`RuntimeManifest::facts`] 里找到，测试以此为断言，防止渲染层夹带私货。

use super::slice::{PlayableSlice, RiskSlicePlan};
use adm4_contracts::SpecRef;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

/// 可玩切片契约的落盘文件名（清单的 `slice_ref` 指向它）。
pub const PLAYABLE_SLICE_FILE: &str = "playable_slice.json";
/// 引擎指南的落盘文件名（清单的 `engine_guide_ref` 指向它；内容由引擎插件提供，本波只有骨架）。
pub const ENGINE_GUIDE_FILE: &str = "engine_guide.json";
/// 运行时清单自身的落盘文件名。
pub const RUNTIME_MANIFEST_FILE: &str = "runtime_manifest.json";
/// durable docs 的四份文件名（册 09 §3）：抵抗上下文丢失、可断点续跑。
pub const DURABLE_PLAN_FILE: &str = "PLAYABLE_PLAN.md";
pub const DURABLE_STRUCTURE_FILE: &str = "PLAYABLE_STRUCTURE.md";
pub const DURABLE_ASSETS_FILE: &str = "PLAYABLE_ASSETS.md";
pub const DURABLE_PROOF_FILE: &str = "PLAYABLE_PROOF.md";

/// 运行时清单（`runtime_manifest.json`）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeManifest {
    /// 本轮可玩目标：由切片的场景 / 主操作 / 成败状态 / 核心循环拼成，不含切片以外的事实。
    pub goal: String,
    /// durable 状态文件名（相对开发目录）。
    pub durable_files: Vec<String>,
    /// 引擎指南文件引用（相对开发目录）。
    pub engine_guide_ref: String,
    /// proof 要求：每条风险切片对应一条"验什么 + 怎么验"。
    pub proof_requirements: Vec<String>,
    /// 失败修复循环的步骤（R7：失败必须落记录，不许伪装成功）。
    pub repair_loop: Vec<String>,
    /// 可玩切片文件引用（相对开发目录）。
    pub slice_ref: String,
    /// 真源锚点：切片锚点 ∪ 风险锚点。
    pub anchors: Vec<SpecRef>,
}

impl RuntimeManifest {
    /// 清单的硬性检查：目标 / 切片引用 / 指南引用 / 锚点不得为空，proof 要求不得为空。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.goal.trim().is_empty() {
            return Err(Adm4Error::validation("运行时清单没有目标（goal）"));
        }
        if self.slice_ref.trim().is_empty() {
            return Err(Adm4Error::validation("运行时清单没有切片引用（slice_ref）"));
        }
        if self.engine_guide_ref.trim().is_empty() {
            return Err(Adm4Error::validation(
                "运行时清单没有引擎指南引用（engine_guide_ref）",
            ));
        }
        if self.proof_requirements.is_empty() {
            return Err(Adm4Error::validation(
                "运行时清单没有任何 proof 要求：无 proof 的可玩产出不可验收（R7）",
            ));
        }
        if self.anchors.iter().all(|anchor| anchor.0.trim().is_empty()) {
            return Err(Adm4Error::validation("运行时清单没有任何真源锚点（R4）"));
        }
        Ok(())
    }

    /// 清单里的全部字符串事实（Markdown 渲染的唯一素材；测试用它核对 Markdown 无夹带）。
    pub fn facts(&self) -> Vec<String> {
        let mut facts = vec![
            self.goal.clone(),
            self.engine_guide_ref.clone(),
            self.slice_ref.clone(),
        ];
        facts.extend(self.durable_files.iter().cloned());
        facts.extend(self.proof_requirements.iter().cloned());
        facts.extend(self.repair_loop.iter().cloned());
        facts.extend(self.anchors.iter().map(|anchor| anchor.0.clone()));
        facts
    }

    /// 一页内的 Markdown：标题是固定标签，每条 `- ` 行都是契约字段的原文。
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# 运行时清单\n\n");
        section(&mut out, "目标", std::slice::from_ref(&self.goal));
        section(&mut out, "可玩切片", std::slice::from_ref(&self.slice_ref));
        section(
            &mut out,
            "引擎指南",
            std::slice::from_ref(&self.engine_guide_ref),
        );
        section(&mut out, "durable 状态文件", &self.durable_files);
        section(&mut out, "proof 要求", &self.proof_requirements);
        section(&mut out, "失败修复循环", &self.repair_loop);
        let anchors: Vec<String> = self.anchors.iter().map(|anchor| anchor.0.clone()).collect();
        section(&mut out, "真源锚点", &anchors);
        out
    }
}

fn section(out: &mut String, title: &str, lines: &[String]) {
    out.push_str("## ");
    out.push_str(title);
    out.push('\n');
    for line in lines {
        if !line.trim().is_empty() {
            out.push_str("- ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
}

/// 四份 durable docs 文件名（清单 `durable_files` 与 [`super::dev_round::DurableDocs`] 共用）。
pub fn durable_file_names() -> Vec<String> {
    vec![
        DURABLE_PLAN_FILE.to_string(),
        DURABLE_STRUCTURE_FILE.to_string(),
        DURABLE_ASSETS_FILE.to_string(),
        DURABLE_PROOF_FILE.to_string(),
    ]
}

/// 从切片与风险计划渲染运行时清单。
///
/// 不返回 `Result`：输入已是通过校验的契约，这里只是重排字段；调用方仍可用
/// [`RuntimeManifest::validate`] 复核（例如切片是旧档读回来未校验的情况）。
pub fn render_runtime_manifest(
    slice: &PlayableSlice,
    risk_plan: &RiskSlicePlan,
) -> RuntimeManifest {
    let primary = slice
        .primary_input
        .first()
        .map(String::as_str)
        .filter(|item| !item.trim().is_empty());
    let goal = match primary {
        Some(primary) => format!(
            "在场景 {} 内通过主操作 {} 达成「{}」；核心循环：{}",
            slice.scene, primary, slice.success_or_fail_state, slice.core_loop
        ),
        None => format!(
            "在场景 {} 内达成「{}」；核心循环：{}",
            slice.scene, slice.success_or_fail_state, slice.core_loop
        ),
    };

    let proof_requirements = risk_plan
        .items
        .iter()
        .map(|item| {
            format!(
                "{}风险：{}（验证方式：{}）",
                item.risk.label(),
                item.description,
                item.verify_by.label()
            )
        })
        .collect();

    let mut anchors = slice.anchors.clone();
    for item in &risk_plan.items {
        for anchor in &item.anchors {
            if !anchors.contains(anchor) {
                anchors.push(anchor.clone());
            }
        }
    }

    RuntimeManifest {
        goal,
        durable_files: durable_file_names(),
        engine_guide_ref: ENGINE_GUIDE_FILE.to_string(),
        proof_requirements,
        repair_loop: vec![
            "执行构建与运行命令，采集日志与 proof".to_string(),
            "失败：原样写入本轮 failures，不伪装成功".to_string(),
            "按失败修复，写 repair_summary，开下一轮".to_string(),
            "成功：proof 要求全部有证据后才可宣称完成".to_string(),
        ],
        slice_ref: PLAYABLE_SLICE_FILE.to_string(),
        anchors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::slice::extract_playable_slice;
    use crate::program::slice::test_fixtures::{program, spec};

    fn manifest() -> RuntimeManifest {
        let extraction = extract_playable_slice(&spec(), &program()).expect("抽取");
        render_runtime_manifest(&extraction.slice, &extraction.risk_plan)
    }

    /// 清单字段全部源于切片/风险计划：目标含场景/主操作/成败状态，proof 条数 = 风险条数。
    #[test]
    fn manifest_mirrors_slice_and_risk_plan() {
        let extraction = extract_playable_slice(&spec(), &program()).expect("抽取");
        let manifest = render_runtime_manifest(&extraction.slice, &extraction.risk_plan);
        assert!(manifest.validate().is_ok());
        assert!(manifest.goal.contains("wave_1"));
        assert!(manifest.goal.contains("cap_place_guard"));
        assert!(manifest.goal.contains("基地存活即胜利"));
        assert_eq!(
            manifest.proof_requirements.len(),
            extraction.risk_plan.items.len()
        );
        assert!(manifest.proof_requirements[0].contains("验证方式：视频"));
        assert_eq!(manifest.slice_ref, PLAYABLE_SLICE_FILE);
        assert_eq!(manifest.engine_guide_ref, ENGINE_GUIDE_FILE);
        assert_eq!(manifest.durable_files, durable_file_names());
        for anchor in &extraction.slice.anchors {
            assert!(manifest.anchors.contains(anchor));
        }
    }

    /// 验收 d：Markdown 里每条非空事实都能在契约字段里找到；且一页内。
    #[test]
    fn markdown_facts_all_trace_back_to_contract() {
        let manifest = manifest();
        let markdown = manifest.to_markdown();
        let facts = manifest.facts();
        let mut fact_lines = 0;
        for line in markdown.lines() {
            if let Some(fact) = line.strip_prefix("- ") {
                fact_lines += 1;
                assert!(
                    facts.iter().any(|known| known == fact),
                    "Markdown 事实「{fact}」在契约里找不到"
                );
            } else {
                assert!(
                    line.is_empty() || line.starts_with('#'),
                    "非事实行只能是标题或空行：{line:?}"
                );
            }
        }
        // 反向：契约里的每条非空事实都进了 Markdown（不漏）。
        let non_blank = facts.iter().filter(|f| !f.trim().is_empty()).count();
        assert_eq!(fact_lines, non_blank);
        assert!(markdown.lines().count() <= 60, "清单必须一页内");
    }

    /// 确定性 + serde 往返 + 旧档缺键可读。
    #[test]
    fn manifest_is_deterministic_and_round_trips() {
        let first = serde_json::to_string(&manifest()).expect("序列化");
        let second = serde_json::to_string(&manifest()).expect("序列化");
        assert_eq!(first, second);
        let back: RuntimeManifest = serde_json::from_str(&first).expect("反序列化");
        assert_eq!(back, manifest());

        let legacy: RuntimeManifest =
            serde_json::from_str(r#"{"goal":"旧目标"}"#).expect("旧档可读");
        assert!(legacy.durable_files.is_empty());
        assert!(legacy.validate().is_err(), "缺切片引用/锚点的清单不可放行");
    }

    /// 空切片渲染出的清单过不了校验：渲染不兜底，校验兜底出口是 Err。
    #[test]
    fn empty_slice_yields_invalid_manifest() {
        let manifest =
            render_runtime_manifest(&PlayableSlice::default(), &RiskSlicePlan::default());
        let error = manifest.validate().expect_err("空清单必须被拒");
        assert!(error.message.contains("proof"), "{}", error.message);
    }
}
