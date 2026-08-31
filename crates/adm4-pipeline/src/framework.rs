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
    /// 本段开始执行的时刻（ISO-8601 UTC）；未开始执行过的段为空串。
    ///
    /// 「耗时」= `finished_at - started_at`，两个时刻都得在案才算得出来。旧存档没有
    /// 这个键（`serde(default)` → 空串），[`StageRecord::duration_seconds`] 对它返回 None——
    /// 缺输入就如实说不知道，不拿 0 秒冒充（R2）。
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub finished_at: String,
    #[serde(default)]
    pub human_confirmation: Option<HumanConfirmation>,
}

impl StageRecord {
    /// 本段耗时（秒）：`started_at` 与 `finished_at` 都在案且顺序合理时才有值。
    ///
    /// 返回 None 的情形：旧存档（无 `started_at`）、正在运行（无 `finished_at`）、
    /// 从未开始执行（依赖未满足而 Blocked 的段）、时刻不可解析或倒序。
    /// 人工门的 `finished_at` 是**确认时刻**，因此这里算出来的是「产物就绪 + 等人签字」
    /// 的总时长——那正是流水线面板要显示的东西（卡在人工门多久也是耗时）。
    pub fn duration_seconds(&self) -> Option<i64> {
        let started = parse_iso8601_seconds(&self.started_at)?;
        let finished = parse_iso8601_seconds(&self.finished_at)?;
        (finished >= started).then_some(finished - started)
    }
}

/// 解析 [`now_iso`] 产出的秒级 ISO-8601 UTC 时刻为 Unix 秒。
///
/// 只认自家写出去的格式 `YYYY-MM-DDTHH:MM:SSZ`（[`UtcTimestamp::to_iso8601`] 的逆运算）；
/// 认不出就返回 None，由调用方按「耗时未知」处理，不猜不补。
fn parse_iso8601_seconds(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' || bytes[19] != b'Z' {
        return None;
    }
    let field = |from: usize, to: usize| text.get(from..to)?.parse::<i64>().ok();
    let year = field(0, 4)?;
    let month = field(5, 7)?;
    let day = field(8, 10)?;
    let hour = field(11, 13)?;
    let minute = field(14, 16)?;
    let second = field(17, 19)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

/// Howard Hinnant 的 civil→days 算法（`adm4_foundation::UtcTimestamp` 内那份的逆运算）。
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
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

    /// 阶段产物目录（校验版）：`clear_stage` 会递归删除该目录，所以先挡住任何
    /// 带分隔符/相对成分/空白的 id——否则一个畸形 id 就能把整个流水线版本目录删掉。
    fn stage_dir_checked(&self, stage_id: &str) -> Adm4Result<PathBuf> {
        let trimmed = stage_id.trim();
        if trimmed.is_empty()
            || trimmed == "."
            || trimmed == ".."
            || trimmed.contains(['/', '\\', ':'])
        {
            return Err(Adm4Error::invalid_input(format!(
                "非法阶段 id「{stage_id}」：产物目录名必须是单段、无路径分隔符的标识"
            )));
        }
        Ok(self.root.join(trimmed))
    }

    /// 作废一段的产物：删除该段目录（contract.json + document.md 一并消失）。
    ///
    /// 返回 `true` = 目录原本存在且已删除，`false` = 本来就没有产物（不是错误）。
    /// 重跑必须调用它：留着旧产物会让下游读到上一版契约，产出「错版文档」。
    pub fn clear_stage(&self, stage_id: &str) -> Adm4Result<bool> {
        let dir = self.stage_dir_checked(stage_id)?;
        if !dir.exists() {
            return Ok(false);
        }
        std::fs::remove_dir_all(&dir).map_err(|error| {
            Adm4Error::io(format!("删除阶段产物目录 {} 失败：{error}", dir.display()))
        })?;
        Ok(true)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_store(case: &str) -> ArtifactStore {
        let root = std::env::temp_dir().join(format!(
            "adm4_store_{case}_{}_{}",
            std::process::id(),
            now_iso().replace([':', '.', '-'], "")
        ));
        let _ = std::fs::remove_dir_all(&root);
        ArtifactStore::new(root, SkinScanner::default())
    }

    #[test]
    fn clear_stage_removes_both_artifacts_and_is_idempotent() {
        let store = scratch_store("clear");
        store
            .write_stage("C2", &serde_json::json!({"ok": true}), "# C2\n正文")
            .expect("write stage");
        let dir = store.root.join("C2");
        assert!(dir.join("contract.json").is_file());
        assert!(dir.join("document.md").is_file());

        assert!(
            store.clear_stage("C2").expect("clear stage"),
            "首次清理应报告确有产物被作废"
        );
        assert!(!dir.exists(), "产物目录必须整体消失，不能留半份契约");
        assert!(
            !store.clear_stage("C2").expect("clear again"),
            "无产物时清理是幂等的 false，而不是错误"
        );

        std::fs::remove_dir_all(&store.root).ok();
    }

    fn record(started_at: &str, finished_at: &str) -> StageRecord {
        StageRecord {
            stage_id: "C2".into(),
            status: StageStatus::Succeeded,
            contract_hash: String::new(),
            started_at: started_at.into(),
            finished_at: finished_at.into(),
            human_confirmation: None,
        }
    }

    /// 旧存档（无 `started_at`）必须原样反序列化，且耗时如实为「未知」。
    #[test]
    fn legacy_stage_record_without_started_at_parses_with_unknown_duration() {
        let legacy = r#"{
          "stage_id": "C2",
          "status": {"status": "succeeded"},
          "contract_hash": "",
          "finished_at": "2026-08-31T10:00:30Z",
          "human_confirmation": null
        }"#;
        let parsed: StageRecord = serde_json::from_str(legacy).expect("旧存档记录应可解析");
        assert!(parsed.started_at.is_empty());
        assert_eq!(parsed.finished_at, "2026-08-31T10:00:30Z");
        assert_eq!(
            parsed.duration_seconds(),
            None,
            "缺开始时刻就说不知道，不许拿 0 秒冒充"
        );
        let json = serde_json::to_string(&parsed).expect("序列化");
        assert!(json.contains(r#""started_at":"""#), "{json}");
        let round_trip: StageRecord = serde_json::from_str(&json).expect("往返");
        assert_eq!(round_trip, parsed);
    }

    /// 耗时按两个时刻相减；缺任一时刻、倒序或格式不认识 → None。
    #[test]
    fn stage_duration_needs_both_timestamps_in_order() {
        assert_eq!(
            record("2026-08-31T10:00:00Z", "2026-08-31T10:00:30Z").duration_seconds(),
            Some(30)
        );
        // 跨日、跨年也要对（civil↔days 换算是逆运算，不是近似）。
        assert_eq!(
            record("2026-12-31T23:59:00Z", "2027-01-01T00:01:00Z").duration_seconds(),
            Some(120)
        );
        assert_eq!(record("", "2026-08-31T10:00:30Z").duration_seconds(), None);
        assert_eq!(record("2026-08-31T10:00:00Z", "").duration_seconds(), None);
        assert_eq!(
            record("2026-08-31T10:00:30Z", "2026-08-31T10:00:00Z").duration_seconds(),
            None,
            "倒序时刻是坏数据，不返回负耗时"
        );
        for bad in ["2026-08-31 10:00:00Z", "2026-08-31T10:00:00", "not a time"] {
            assert_eq!(record(bad, "2026-08-31T10:00:30Z").duration_seconds(), None);
        }
        // 与 now_iso 的输出格式对齐（自家写出去的时刻必须自家认得）。
        let now = now_iso();
        assert_eq!(record(&now, &now).duration_seconds(), Some(0));
    }

    #[test]
    fn clear_stage_rejects_malformed_stage_ids() {
        let store = scratch_store("guard");
        for bad in ["", "  ", ".", "..", "../C0", "C0/nested", "C:\\evil"] {
            let error = store
                .clear_stage(bad)
                .expect_err("畸形阶段 id 必须被拒，否则递归删除会越界");
            assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::InvalidInput);
        }
        std::fs::remove_dir_all(&store.root).ok();
    }
}
