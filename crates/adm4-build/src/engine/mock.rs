//! 确定性回放后端：没有真机时把 P1 全链跑通用的替身。
//!
//! 它不假装是引擎：`preflight` 就是构造时给的那个布尔，每轮开发返回脚本里第 N 条记录，
//! 运行结果与证据包原样回放。它**唯一真做**的事是建工程目录并写 `seed.json`——接线层要靠
//! 「目录真的出现了」断言 P0 种子到 P1 工程的交接，而不是靠一个 `Ok(())`。
//!
//! 全部调用记进 [`MockEngineBackend::calls`]，测试用它断言「后端没就绪时没有跑开发轮」这类
//! **不该发生的事确实没发生**（R7 的另一面）。
//!
//! 回放后端不做预检门控：`preflight_ready=false` 时四个方法仍按脚本回放。门控是执行器的责任，
//! 混进替身里会让「执行器忘了查预检」这种缺陷被替身掩盖。

use super::types::{DevContext, DevRound, EngineProjectSeed, ProofBundle, RunResult, SliceTask};
use super::{EngineBackend, EnginePreflight};
use adm4_foundation::{Adm4Error, Adm4Result, ensure_dir, ensure_within_root, write_json_file};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// 工程目录里种子文件的名字；接线层与冒烟脚本按它找文件。
pub const SEED_FILE_NAME: &str = "seed.json";

/// 回放脚本：构造时一次给全，之后不可变，保证同一脚本多次运行结果逐字节一致。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MockEngineScript {
    /// `preflight().ready` 直接回放这个值；默认 `false`，缺字段的旧脚本按未就绪读（fail-closed）。
    pub preflight_ready: bool,
    /// 第 N 轮 `agent_develop` 返回 `rounds[N]`；越界即 `Err`。
    pub rounds: Vec<DevRound>,
    /// `run_playmode` 原样回放。
    pub run: RunResult,
    /// `capture_proof` 原样回放。
    pub proof: ProofBundle,
}

/// 一次被记录的调用。只记能用来断言的事实字段，不记整份入参，避免记录本身成为第二真源。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MockCall {
    Preflight,
    OpenOrCreateProject {
        engine_id: String,
        project_dir_name: String,
        dir: PathBuf,
    },
    AgentDevelop {
        round_index: u32,
        slice_ref: String,
        project_dir: PathBuf,
    },
    RunPlaymode {
        project: PathBuf,
    },
    CaptureProof {
        project: PathBuf,
    },
}

/// 确定性回放后端。
pub struct MockEngineBackend {
    id: String,
    script: MockEngineScript,
    calls: Mutex<Vec<MockCall>>,
}

impl MockEngineBackend {
    /// 以后端 id 与一份完整脚本构造；脚本之后不可变，所以同一构造参数的两个实例回放结果相同。
    pub fn new(id: &str, script: MockEngineScript) -> Self {
        Self {
            id: id.to_string(),
            script,
            calls: Mutex::new(Vec::new()),
        }
    }

    /// 迄今全部调用（按发生顺序），含被拒的调用——「被拒」也是要断言的事实。
    pub fn calls(&self) -> Vec<MockCall> {
        self.lock_calls().clone()
    }

    /// 构造时给的脚本，供测试对照「回放结果 == 脚本内容」。
    pub fn script(&self) -> &MockEngineScript {
        &self.script
    }

    /// 记录只是测试证据，某个线程 panic 导致锁中毒时里面的记录依然完整，取回继续用。
    fn lock_calls(&self) -> MutexGuard<'_, Vec<MockCall>> {
        match self.calls.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn record(&self, call: MockCall) {
        self.lock_calls().push(call);
    }
}

impl EngineBackend for MockEngineBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn preflight(&self) -> Adm4Result<EnginePreflight> {
        self.record(MockCall::Preflight);
        let detail = if self.script.preflight_ready {
            format!(
                "回放后端 {} 按脚本就绪（预置 {} 轮开发记录）",
                self.id,
                self.script.rounds.len()
            )
        } else {
            format!("回放后端 {} 按脚本设为未就绪", self.id)
        };
        Ok(EnginePreflight {
            backend_id: self.id.clone(),
            ready: self.script.preflight_ready,
            detail,
        })
    }

    fn open_or_create_project(&self, seed: &EngineProjectSeed, dir: &Path) -> Adm4Result<()> {
        self.record(MockCall::OpenOrCreateProject {
            engine_id: seed.engine_id.clone(),
            project_dir_name: seed.project_dir_name.clone(),
            dir: dir.to_path_buf(),
        });
        if seed.project_dir_name.trim().is_empty() {
            return Err(Adm4Error::validation(format!(
                "种子 project_dir_name 为空，无法在 {} 下建工程目录",
                dir.display()
            )));
        }
        let relative = ensure_within_root(Path::new(&seed.project_dir_name)).map_err(|error| {
            Adm4Error::new(
                error.kind,
                format!(
                    "种子 project_dir_name={} 不是安全的相对目录名：{}",
                    seed.project_dir_name, error.message
                ),
            )
        })?;
        let project_dir = dir.join(relative);
        ensure_dir(&project_dir)?;
        write_json_file(&project_dir.join(SEED_FILE_NAME), seed)
    }

    fn agent_develop(&self, task: &SliceTask, ctx: &DevContext) -> Adm4Result<DevRound> {
        self.record(MockCall::AgentDevelop {
            round_index: task.round_index,
            slice_ref: task.slice_ref.clone(),
            project_dir: ctx.project_dir.clone(),
        });
        self.script
            .rounds
            .get(task.round_index as usize)
            .cloned()
            .ok_or_else(|| {
                Adm4Error::not_found(format!(
                    "回放后端 {} 没有第 {} 轮的预置记录（脚本共 {} 轮），切片 {}",
                    self.id,
                    task.round_index,
                    self.script.rounds.len(),
                    task.slice_ref
                ))
            })
    }

    fn run_playmode(&self, project: &Path) -> Adm4Result<RunResult> {
        self.record(MockCall::RunPlaymode {
            project: project.to_path_buf(),
        });
        Ok(self.script.run.clone())
    }

    fn capture_proof(&self, project: &Path) -> Adm4Result<ProofBundle> {
        self.record(MockCall::CaptureProof {
            project: project.to_path_buf(),
        });
        Ok(self.script.proof.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::types::DevRoundStatus;
    use adm4_foundation::read_json_file;

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(case: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("adm4_engine_mock_{case}_{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("建临时目录");
            Self(root)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn script() -> MockEngineScript {
        MockEngineScript {
            preflight_ready: true,
            rounds: vec![
                DevRound {
                    index: 0,
                    commands: vec!["create_scene".into()],
                    failures: vec![],
                    repair_summary: String::new(),
                    status: DevRoundStatus::Succeeded,
                },
                DevRound {
                    index: 1,
                    commands: vec!["attach_script".into()],
                    failures: vec!["编译失败一次".into()],
                    repair_summary: "补了引用".into(),
                    status: DevRoundStatus::Succeeded,
                },
            ],
            run: RunResult {
                launched: true,
                duration_ms: 42,
                log_path: PathBuf::from("proof/run_log.txt"),
                exit_detail: "回放：正常退出".into(),
            },
            proof: ProofBundle {
                build_log: PathBuf::from("proof/build_log.txt"),
                run_log: PathBuf::from("proof/run_log.txt"),
                screenshots: vec![PathBuf::from("proof/screenshots/0001.png")],
                video: None,
                captured_at: "2026-09-02T00:00:00Z".into(),
                capture_detail: "回放：无视频".into(),
            },
        }
    }

    fn seed() -> EngineProjectSeed {
        EngineProjectSeed {
            engine_id: "mock".into(),
            project_dir_name: "playable_proj".into(),
            seed_kind: "empty".into(),
            required_tools: vec![],
            notes: String::new(),
            anchors: vec![],
        }
    }

    #[test]
    fn open_or_create_project_really_creates_dir_and_writes_seed_idempotently() {
        let root = TempRoot::new("open");
        let backend = MockEngineBackend::new("mock", script());
        let seed = seed();
        backend
            .open_or_create_project(&seed, &root.0)
            .expect("建工程");
        let project_dir = root.0.join("playable_proj");
        assert!(project_dir.is_dir(), "工程目录必须真实存在");
        let written: EngineProjectSeed =
            read_json_file(&project_dir.join(SEED_FILE_NAME)).expect("读 seed.json");
        assert_eq!(written, seed);

        std::fs::write(project_dir.join("keep.txt"), b"x").expect("写现场文件");
        backend
            .open_or_create_project(&seed, &root.0)
            .expect("再次打开不得失败");
        assert!(
            project_dir.join("keep.txt").exists(),
            "再次打开不得清空已有现场"
        );
        assert_eq!(
            backend.calls(),
            vec![
                MockCall::OpenOrCreateProject {
                    engine_id: "mock".into(),
                    project_dir_name: "playable_proj".into(),
                    dir: root.0.clone(),
                };
                2
            ]
        );
    }

    #[test]
    fn open_or_create_project_rejects_empty_or_escaping_dir_name() {
        let root = TempRoot::new("reject");
        let backend = MockEngineBackend::new("mock", script());
        let empty = EngineProjectSeed {
            project_dir_name: "  ".into(),
            ..seed()
        };
        let error = backend
            .open_or_create_project(&empty, &root.0)
            .expect_err("空目录名应 Err");
        assert!(
            error.message.contains("project_dir_name"),
            "{}",
            error.message
        );

        let escaping = EngineProjectSeed {
            project_dir_name: "../outside".into(),
            ..seed()
        };
        let error = backend
            .open_or_create_project(&escaping, &root.0)
            .expect_err("越界目录名应 Err");
        assert!(error.message.contains("../outside"), "{}", error.message);
        assert!(!root.0.join("outside").exists());
        assert!(!root.0.parent().expect("父目录").join("outside").exists());
        assert_eq!(backend.calls().len(), 2, "被拒的调用同样要记录");
    }

    #[test]
    fn agent_develop_replays_by_round_index_deterministically_and_errors_out_of_range() {
        let backend = MockEngineBackend::new("mock", script());
        let ctx = DevContext {
            project_dir: PathBuf::from("proj"),
            ..DevContext::default()
        };
        let task = |round_index: u32| SliceTask {
            slice_ref: "P1/slice.json".into(),
            round_index,
            objective: "目标".into(),
            constraints: vec![],
        };
        let first = backend.agent_develop(&task(1), &ctx).expect("第 1 轮");
        let second = backend.agent_develop(&task(1), &ctx).expect("再取第 1 轮");
        assert_eq!(first, second, "同一轮次两次回放必须相同");
        assert_eq!(first, script().rounds[1]);
        let zero = backend.agent_develop(&task(0), &ctx).expect("第 0 轮");
        assert_eq!(zero.commands, vec!["create_scene".to_string()]);

        let error = backend
            .agent_develop(&task(2), &ctx)
            .expect_err("越界应 Err");
        assert!(error.message.contains("第 2 轮"), "{}", error.message);
        assert!(error.message.contains("共 2 轮"), "{}", error.message);
        assert!(error.message.contains("P1/slice.json"), "{}", error.message);

        assert_eq!(
            backend.calls(),
            vec![
                MockCall::AgentDevelop {
                    round_index: 1,
                    slice_ref: "P1/slice.json".into(),
                    project_dir: PathBuf::from("proj"),
                },
                MockCall::AgentDevelop {
                    round_index: 1,
                    slice_ref: "P1/slice.json".into(),
                    project_dir: PathBuf::from("proj"),
                },
                MockCall::AgentDevelop {
                    round_index: 0,
                    slice_ref: "P1/slice.json".into(),
                    project_dir: PathBuf::from("proj"),
                },
                MockCall::AgentDevelop {
                    round_index: 2,
                    slice_ref: "P1/slice.json".into(),
                    project_dir: PathBuf::from("proj"),
                },
            ]
        );
    }

    #[test]
    fn preflight_run_and_proof_replay_script_and_calls_keep_full_order() {
        let backend = MockEngineBackend::new("mock", script());
        assert_eq!(backend.id(), "mock");
        let preflight = backend.preflight().expect("预检");
        assert!(preflight.ready);
        assert_eq!(preflight.backend_id, "mock");
        assert!(preflight.detail.contains("2 轮"), "{}", preflight.detail);

        let project = Path::new("proj");
        assert_eq!(backend.run_playmode(project).expect("运行"), script().run);
        assert_eq!(
            backend.capture_proof(project).expect("证据"),
            script().proof
        );
        assert_eq!(
            backend.calls(),
            vec![
                MockCall::Preflight,
                MockCall::RunPlaymode {
                    project: PathBuf::from("proj"),
                },
                MockCall::CaptureProof {
                    project: PathBuf::from("proj"),
                },
            ]
        );

        let not_ready = MockEngineBackend::new(
            "mock_off",
            MockEngineScript {
                preflight_ready: false,
                ..MockEngineScript::default()
            },
        );
        let preflight = not_ready.preflight().expect("预检");
        assert!(!preflight.ready);
        assert!(preflight.detail.contains("未就绪"));
        assert!(
            not_ready
                .agent_develop(&SliceTask::default(), &DevContext::default())
                .is_err(),
            "空脚本任何轮次都越界"
        );
    }

    #[test]
    fn script_and_calls_round_trip_and_read_legacy() {
        let script = script();
        let json = serde_json::to_string(&script).expect("序列化");
        assert_eq!(
            serde_json::from_str::<MockEngineScript>(&json).expect("反序列化"),
            script
        );
        let legacy: MockEngineScript = serde_json::from_str("{}").expect("空对象");
        assert!(!legacy.preflight_ready, "缺字段按未就绪读");
        assert!(legacy.rounds.is_empty());

        let calls = vec![
            MockCall::Preflight,
            MockCall::RunPlaymode {
                project: PathBuf::from("p"),
            },
        ];
        let json = serde_json::to_string(&calls).expect("序列化");
        assert!(json.contains(r#""kind":"run_playmode""#), "{json}");
        assert_eq!(
            serde_json::from_str::<Vec<MockCall>>(&json).expect("反序列化"),
            calls
        );
    }
}
