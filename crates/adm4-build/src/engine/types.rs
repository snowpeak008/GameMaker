//! 引擎无关的后端契约类型：种子、切片任务、开发上下文、轮次、运行结果、证据包。
//!
//! 这些类型是 [`super::EngineBackend`] 四个「干活」方法的入参与返回值，也是 P0/P1 落盘的
//! 制品形态。它们**不认得任何具体引擎**：`engine_id` 只是字符串标识，由配置决定；具体引擎
//! 的后端实现放在本模块子目录里，治理与运行骨架只通过这些契约交换数据（接缝纪律 D17）。
//!
//! 全部结构 `#[serde(default)]`：契约字段只增不改，旧档缺键按默认值读（D4）。默认值只用于
//! 「读旧档不炸」，**不代表事实**——凡是缺事实就不能继续的地方，调用方必须显式校验并 `Err`。

use adm4_contracts::SpecRef;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 引擎工程种子：P0 契约 `engine_seed` 要落的真实内容。
///
/// 它回答「P1 要在哪个引擎里、以什么方式、开一个叫什么的工程」，但不携带任何引擎专有语义——
/// `seed_kind` 与 `required_tools` 的取值由具体后端解释，这里只是把事实原样记下来以便追溯。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineProjectSeed {
    /// 后端标识（与 [`super::EngineBackend::id`] 对应）。来自配置；未配置时由调用方决定如何报告。
    pub engine_id: String,
    /// 目标工程目录名（相对于调用方给的父目录）。工程与本仓库隔离，靠清单/证据契约交换数据。
    pub project_dir_name: String,
    /// 种子类型（如「空工程」「模板工程」），具体取值由后端解释。
    pub seed_kind: String,
    /// 后端声明需要的外部工具；预检据此报告缺什么，而不是笼统说「环境不满足」。
    pub required_tools: Vec<String>,
    /// 给后端/人看的备注（不参与派生）。
    pub notes: String,
    /// 种子来源锚点：它是从 GameSpec 哪些事实派生出来的（R4）。
    pub anchors: Vec<SpecRef>,
}

/// 一轮现场开发的任务：让后端围绕某个可玩切片推进一轮。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SliceTask {
    /// 指向可玩切片制品的引用（制品路径或 id），后端据此读切片事实，而不是靠口头描述。
    pub slice_ref: String,
    /// 轮次序号（从 0 起）。回放型后端按它取对应结果，真实后端把它写进轮次记录以便续跑。
    pub round_index: u32,
    /// 本轮目标（一句话，来自清单，不由后端发明）。
    pub objective: String,
    /// 本轮约束（不许碰什么、必须保住什么）。
    pub constraints: Vec<String>,
}

/// 现场开发的上下文路径：后端在哪个工程里干、读哪份清单与指南、把耐久文档写到哪。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DevContext {
    /// 引擎工程根目录（隔离于本仓库）。
    pub project_dir: PathBuf,
    /// 运行时清单文件路径。
    pub manifest_path: PathBuf,
    /// 引擎指南文件路径。
    pub guide_path: PathBuf,
    /// 耐久文档目录（可停可续的记录落在这里）。
    pub durable_dir: PathBuf,
}

/// 一轮开发的结局。
///
/// 默认为「进行中」：旧档缺该字段时不能被读成「成功」（fail-closed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevRoundStatus {
    #[default]
    InProgress,
    Succeeded,
    Failed,
    Aborted,
}

/// 一轮现场开发的记录（后端返回）。
///
/// 字段名与轮次日志制品的轮次结构保持一致（`index/commands/failures/repair_summary/status`），
/// 以便调用方直接追加进日志而不做字段翻译；两者的合流由接线波次负责。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DevRound {
    /// 轮次序号（与 [`SliceTask::round_index`] 对应）。
    pub index: u32,
    /// 本轮实际执行过的命令/工具调用（原样记录，不做摘要）。
    pub commands: Vec<String>,
    /// 本轮遇到的失败（原文，不吞）。
    pub failures: Vec<String>,
    /// 本轮修复了什么（人可读摘要）。
    pub repair_summary: String,
    pub status: DevRoundStatus,
}

/// 「跑起来」的结果。
///
/// `launched=false` 是一条正常结论而非错误：引擎给出了明确的未启动原因就写进 `exit_detail`。
/// `Err` 只用于「跑的动作本身没做成」。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RunResult {
    pub launched: bool,
    pub duration_ms: u64,
    /// 运行日志落盘位置（证据，供预检与人工核对）。
    pub log_path: PathBuf,
    /// 退出/未启动的机器原因原文。
    pub exit_detail: String,
}

/// 运行证据包（册 09 §5.1 的机器形态）。
///
/// 证据是路径而不是内容：文件留在工程/证据目录里，预检与人工核对直接看文件。
/// 捕获不到就把原因写进 `capture_detail`，绝不伪装成功（R7）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProofBundle {
    pub build_log: PathBuf,
    pub run_log: PathBuf,
    pub screenshots: Vec<PathBuf>,
    pub video: Option<PathBuf>,
    /// 捕获时刻（ISO-8601 UTC）。
    pub captured_at: String,
    /// 捕获过程说明：用了什么手段、缺了什么、为什么缺。
    pub capture_detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_fixture() -> EngineProjectSeed {
        EngineProjectSeed {
            engine_id: "engine_a".into(),
            project_dir_name: "playable_proj".into(),
            seed_kind: "empty".into(),
            required_tools: vec!["tool_x".into(), "tool_y".into()],
            notes: "备注".into(),
            anchors: vec![SpecRef::new("intent"), SpecRef::new("mechanics/jump")],
        }
    }

    #[test]
    fn engine_project_seed_round_trips_and_reads_legacy() {
        let seed = seed_fixture();
        let json = serde_json::to_string(&seed).expect("序列化");
        assert_eq!(
            serde_json::from_str::<EngineProjectSeed>(&json).expect("反序列化"),
            seed
        );
        let legacy: EngineProjectSeed =
            serde_json::from_str(r#"{"engine_id":"engine_a"}"#).expect("旧档应可解析");
        assert_eq!(legacy.engine_id, "engine_a");
        assert!(legacy.project_dir_name.is_empty());
        assert!(legacy.anchors.is_empty());
    }

    #[test]
    fn slice_task_and_dev_context_round_trip() {
        let task = SliceTask {
            slice_ref: "P1/playable_slice.json".into(),
            round_index: 3,
            objective: "让主操作可见".into(),
            constraints: vec!["不改清单".into()],
        };
        let json = serde_json::to_string(&task).expect("序列化");
        assert_eq!(
            serde_json::from_str::<SliceTask>(&json).expect("反序列化"),
            task
        );
        let legacy: SliceTask = serde_json::from_str(r#"{"round_index":7}"#).expect("旧档");
        assert_eq!(legacy.round_index, 7);

        let ctx = DevContext {
            project_dir: PathBuf::from("proj"),
            manifest_path: PathBuf::from("proj/manifest.json"),
            guide_path: PathBuf::from("proj/guide.json"),
            durable_dir: PathBuf::from("proj/durable"),
        };
        let json = serde_json::to_string(&ctx).expect("序列化");
        assert_eq!(
            serde_json::from_str::<DevContext>(&json).expect("反序列化"),
            ctx
        );
        let legacy: DevContext = serde_json::from_str("{}").expect("空对象应可解析");
        assert_eq!(legacy, DevContext::default());
    }

    #[test]
    fn dev_round_round_trips_and_legacy_status_is_in_progress() {
        let round = DevRound {
            index: 2,
            commands: vec!["tool_call a".into()],
            failures: vec!["编译失败一次".into()],
            repair_summary: "修了引用".into(),
            status: DevRoundStatus::Succeeded,
        };
        let json = serde_json::to_string(&round).expect("序列化");
        assert!(
            json.contains(r#""status":"succeeded""#),
            "状态应为 snake_case"
        );
        assert_eq!(
            serde_json::from_str::<DevRound>(&json).expect("反序列化"),
            round
        );
        let legacy: DevRound = serde_json::from_str(r#"{"index":1}"#).expect("旧档");
        assert_eq!(
            legacy.status,
            DevRoundStatus::InProgress,
            "缺状态不得读成成功"
        );
    }

    #[test]
    fn run_result_and_proof_bundle_round_trip() {
        let run = RunResult {
            launched: true,
            duration_ms: 1234,
            log_path: PathBuf::from("proof/run_log.txt"),
            exit_detail: "正常退出".into(),
        };
        let json = serde_json::to_string(&run).expect("序列化");
        assert_eq!(
            serde_json::from_str::<RunResult>(&json).expect("反序列化"),
            run
        );
        let legacy: RunResult = serde_json::from_str("{}").expect("空对象");
        assert!(!legacy.launched, "缺字段按未启动读");

        let proof = ProofBundle {
            build_log: PathBuf::from("proof/build_log.txt"),
            run_log: PathBuf::from("proof/run_log.txt"),
            screenshots: vec![PathBuf::from("proof/screenshots/0001.png")],
            video: Some(PathBuf::from("proof/video.mp4")),
            captured_at: "2026-09-02T00:00:00Z".into(),
            capture_detail: "回放".into(),
        };
        let json = serde_json::to_string(&proof).expect("序列化");
        assert_eq!(
            serde_json::from_str::<ProofBundle>(&json).expect("反序列化"),
            proof
        );
        let legacy: ProofBundle =
            serde_json::from_str(r#"{"capture_detail":"缺视频"}"#).expect("旧档");
        assert!(legacy.video.is_none());
        assert!(legacy.screenshots.is_empty());
    }
}
