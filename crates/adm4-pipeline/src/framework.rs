use adm4_contracts::SkinScanner;
use adm4_foundation::{
    Adm4Error, Adm4Result, UtcTimestamp, atomic_write, read_json_file, sha256_hex, write_json_file,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    /// 确定性：无 AI、无人工。
    Deterministic,
    /// AI 必需：AI 不可用 = blocked（R7，无兜底）。
    AiRequired,
    /// 人工门：产物就绪后等待人工确认。
    HumanGate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageSpec {
    pub id: String,
    pub name: String,
    pub kind: StageKind,
    pub depends_on: Vec<String>,
    pub summary: String,
}

/// Phase 1：C0-C6 文档编译 registry（面板数据驱动的唯一来源）。
pub fn design_compile_registry() -> Vec<StageSpec> {
    let stage =
        |id: &str, name: &str, kind: StageKind, depends: &[&str], summary: &str| StageSpec {
            id: id.into(),
            name: name.into(),
            kind,
            depends_on: depends.iter().map(|item| item.to_string()).collect(),
            summary: summary.into(),
        };
    vec![
        stage(
            "C0",
            "规格编译",
            StageKind::Deterministic,
            &[],
            "FrozenDesign → GameSpec，绑定冻结哈希",
        ),
        stage(
            "C1",
            "验证与红队",
            StageKind::AiRequired,
            &["C0"],
            "静态规则零违例 + AI 红队（ReviewProof）",
        ),
        stage(
            "C2",
            "玩法设计文档",
            StageKind::AiRequired,
            &["C1"],
            "章节→Spec 锚定 100% + 渲染 MD",
        ),
        stage(
            "C3",
            "内容与资产需求",
            StageKind::AiRequired,
            &["C2"],
            "视觉白名单 + 基数申报门",
        ),
        stage(
            "C4",
            "程序需求与架构",
            StageKind::AiRequired,
            &["C2"],
            "机制投影派生能力契约 + GWT + 双向核对",
        ),
        stage(
            "C5",
            "美术方向与风格锚点",
            StageKind::HumanGate,
            &["C3"],
            "风格简报 + 人工确认",
        ),
        stage(
            "C6",
            "开发计划与签收",
            StageKind::HumanGate,
            &["C3", "C4", "C5"],
            "任务图真对账 + Phase1 人工签收",
        ),
    ]
}

/// Phase 2：P0-P5 边界占位（另行立项，不实现执行器）。
pub fn phase2_registry() -> Vec<StageSpec> {
    let stage = |id: &str, name: &str, summary: &str| StageSpec {
        id: id.into(),
        name: name.into(),
        kind: StageKind::Deterministic,
        depends_on: Vec::new(),
        summary: summary.into(),
    };
    vec![
        stage("P0", "引擎工程骨架", "按 engine_architecture 生成工程种子"),
        stage(
            "P1",
            "程序任务执行",
            "并行生成、串行合入；变更内核 + 受信测试",
        ),
        stage("P2", "资产批量生产", "生产前清单人工门 + 内容哈希缓存"),
        stage("P3", "装配与集成", "按 spec 执行装配"),
        stage("P4", "验收场景执行", "C4 的 GWT 真机运行判定"),
        stage("P5", "打包交付", "EXE + manifest + 确定性报告"),
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StageStatus {
    Pending,
    Running,
    Succeeded,
    Failed { reasons: Vec<String> },
    Blocked { reasons: Vec<String> },
    WaitingHuman { gate: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageRecord {
    pub stage_id: String,
    pub status: StageStatus,
    #[serde(default)]
    pub contract_hash: String,
    #[serde(default)]
    pub finished_at: String,
    #[serde(default)]
    pub human_confirmation: Option<HumanConfirmation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanConfirmation {
    pub actor: String,
    pub note: String,
    pub at: String,
}

/// 一次流水线运行的状态（断点续跑依据；随冻结版本存放）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PipelineRunState {
    pub frozen_hash: String,
    pub stages: BTreeMap<String, StageRecord>,
}

impl PipelineRunState {
    pub fn stage_status(&self, stage_id: &str) -> StageStatus {
        self.stages
            .get(stage_id)
            .map(|record| record.status.clone())
            .unwrap_or(StageStatus::Pending)
    }

    pub fn is_succeeded(&self, stage_id: &str) -> bool {
        matches!(self.stage_status(stage_id), StageStatus::Succeeded)
    }
}

/// 双格式产物仓：contract.json（机器真相）+ document.md（渲染，不可手改）。
/// 落盘前全部过换皮扫描（R5）。
pub struct ArtifactStore {
    pub root: PathBuf,
    pub scanner: SkinScanner,
}

impl ArtifactStore {
    pub fn new(root: PathBuf, scanner: SkinScanner) -> Self {
        Self { root, scanner }
    }

    fn stage_dir(&self, stage_id: &str) -> PathBuf {
        self.root.join(stage_id)
    }

    /// 写机器契约 + 渲染文档；命中参考名 → Err（step fail，R5）。
    pub fn write_stage<T: Serialize>(
        &self,
        stage_id: &str,
        contract: &T,
        document_markdown: &str,
    ) -> Adm4Result<String> {
        let contract_text = serde_json::to_string_pretty(contract)
            .map_err(|error| Adm4Error::internal(format!("contract serialize failed: {error}")))?;
        let mut hits = self
            .scanner
            .scan(&format!("{stage_id}/contract.json"), &contract_text);
        hits.extend(
            self.scanner
                .scan(&format!("{stage_id}/document.md"), document_markdown),
        );
        if !hits.is_empty() {
            let detail: Vec<String> = hits
                .iter()
                .map(|hit| format!("{} 命中 {}", hit.location, hit.matched_word))
                .collect();
            return Err(Adm4Error::red_line(format!(
                "R5: 产物命中参考名（{} 处）：{}",
                hits.len(),
                detail.join("; ")
            )));
        }
        let dir = self.stage_dir(stage_id);
        atomic_write(&dir.join("contract.json"), contract_text.as_bytes())?;
        atomic_write(&dir.join("document.md"), document_markdown.as_bytes())?;
        Ok(sha256_hex(contract_text.as_bytes()))
    }

    pub fn read_contract<T: DeserializeOwned>(&self, stage_id: &str) -> Adm4Result<T> {
        read_json_file(&self.stage_dir(stage_id).join("contract.json"))
    }

    pub fn save_run_state(&self, state: &PipelineRunState) -> Adm4Result<()> {
        write_json_file(&self.root.join("run_state.json"), state)
    }

    pub fn load_run_state(&self) -> Adm4Result<PipelineRunState> {
        let path = self.root.join("run_state.json");
        if !path.is_file() {
            return Ok(PipelineRunState::default());
        }
        read_json_file(&path)
    }
}

pub fn now_iso() -> String {
    UtcTimestamp::now().to_iso8601()
}
