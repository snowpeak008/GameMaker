//! Phase 2 制品注册层（册 10 §1「强设计关联」）：每个阶段**显式声明**它产出什么、消费什么。
//!
//! 为什么要有这一层：插件之间不直接互相 import，只通过制品契约交换数据（弱代码耦合）。
//! 那就必须有一处说清「这份契约是谁产的、谁在用」，否则加一个插件没人知道它该排在哪儿，
//! 出了错也定位不到是谁的问题。这份声明就是那个「制品依赖图」，拓扑序由它推导，
//! 环、悬空消费、双产出者三类结构错误在这里被机器拦下（[`validate_artifact_graph`]）。

use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_pipeline::{StageSpec, phase2_registry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Phase 2 的制品种类：插件之间唯一的数据交换通道。
///
/// 命名只描述**制品是什么**，不带任何具体引擎/工具名——接缝纪律（D17）要求引擎相关的
/// 具体实现锁在后端插件里，注册层不认得任何一个具体引擎。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Phase 1 C0 产出的唯一真源（D22：Phase 2 只读它，不重造第二真源）。
    GameSpec,
    /// Phase 1 C0-C6 的文档编译契约集（C3 内容与资产 / C4 程序需求与架构，两条线的上游）。
    DesignContractSet,
    /// 设计阶段风格门锁定的风格锚点集（册 08 选项 A；Phase 2 只消费不重造）。
    StyleAnchorSet,

    /// 程序线机器契约。
    ProgramContract,
    /// 美术线机器契约。
    ArtContract,
    /// 资产表（命名权威）。
    AssetRegistry,
    /// 对齐合流报告。
    AlignmentReport,
    /// 引擎工程种子（隔离工程的初始骨架）。
    EngineProjectSeed,

    /// 可玩切片定义。
    PlayableSlice,
    /// 薄运行时清单。
    RuntimeManifest,
    /// 引擎指南（按引擎注入的「坑」清单）。
    EngineGuide,
    /// 现场开发轮次记录（可停可续的 durable 记录）。
    DevRoundLog,

    /// 资产生产台账（Name/Purpose/Runtime path/Size/Cost/Fallback/Used by）。
    AssetProductionLedger,
    /// 资产基因表（设计 id ↔ 实际文件）。
    AssetGenome,

    /// 装配与集成报告。
    AssemblyReport,
    /// 运行证据包。
    ProofBundle,
    /// 用户裁决。
    ProofVerdict,
    /// 缺陷回写队列。
    RepairQueue,
    /// 交付清单。
    DeliveryManifest,
}

impl ArtifactKind {
    /// 是否由 Phase 2 之外供给（Phase 1 产物或设计阶段产物）。
    ///
    /// 这三类制品在 Phase 2 里**只被消费、不被产出**，因此依赖图校验不该把它们
    /// 当成「消费了一个没人产的制品」而报错。
    pub fn is_external_input(self) -> bool {
        matches!(
            self,
            Self::GameSpec | Self::DesignContractSet | Self::StyleAnchorSet
        )
    }

    /// 中文展示名（CLI/桌面共用一份口径）。
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::GameSpec => "GameSpec 真源",
            Self::DesignContractSet => "设计文档契约集",
            Self::StyleAnchorSet => "风格锚点集",
            Self::ProgramContract => "程序线契约",
            Self::ArtContract => "美术线契约",
            Self::AssetRegistry => "资产表",
            Self::AlignmentReport => "对齐报告",
            Self::EngineProjectSeed => "引擎工程种子",
            Self::PlayableSlice => "可玩切片",
            Self::RuntimeManifest => "运行时清单",
            Self::EngineGuide => "引擎指南",
            Self::DevRoundLog => "开发轮次记录",
            Self::AssetProductionLedger => "资产生产台账",
            Self::AssetGenome => "资产基因表",
            Self::AssemblyReport => "装配集成报告",
            Self::ProofBundle => "运行证据包",
            Self::ProofVerdict => "用户裁决",
            Self::RepairQueue => "缺陷回写队列",
            Self::DeliveryManifest => "交付清单",
        }
    }
}

/// 一个阶段的制品声明。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StageArtifacts {
    pub stage_id: String,
    pub produces: Vec<ArtifactKind>,
    pub consumes: Vec<ArtifactKind>,
}

/// P0-P5 的制品依赖声明。
///
/// 阶段 id 与摘要沿用 `adm4_pipeline::phase2_registry()`（只增不改，D4）；本函数只补
/// 「产出什么 / 消费什么」这一层——它才是后续波次接活时要照着写的那张表。
pub fn phase2_artifacts() -> Vec<StageArtifacts> {
    let stage =
        |stage_id: &str, produces: &[ArtifactKind], consumes: &[ArtifactKind]| StageArtifacts {
            stage_id: stage_id.to_string(),
            produces: produces.to_vec(),
            consumes: consumes.to_vec(),
        };
    vec![
        stage(
            "P0",
            &[
                ArtifactKind::ProgramContract,
                ArtifactKind::ArtContract,
                ArtifactKind::AssetRegistry,
                ArtifactKind::AlignmentReport,
                ArtifactKind::EngineProjectSeed,
            ],
            &[ArtifactKind::GameSpec, ArtifactKind::DesignContractSet],
        ),
        stage(
            "P1",
            &[
                ArtifactKind::PlayableSlice,
                ArtifactKind::RuntimeManifest,
                ArtifactKind::EngineGuide,
                ArtifactKind::DevRoundLog,
            ],
            &[
                ArtifactKind::GameSpec,
                ArtifactKind::ProgramContract,
                ArtifactKind::AlignmentReport,
                ArtifactKind::EngineProjectSeed,
            ],
        ),
        stage(
            "P2",
            &[
                ArtifactKind::AssetProductionLedger,
                ArtifactKind::AssetGenome,
            ],
            &[
                ArtifactKind::ArtContract,
                ArtifactKind::AssetRegistry,
                ArtifactKind::AlignmentReport,
                ArtifactKind::StyleAnchorSet,
            ],
        ),
        stage(
            "P3",
            &[ArtifactKind::AssemblyReport],
            &[
                ArtifactKind::PlayableSlice,
                ArtifactKind::RuntimeManifest,
                ArtifactKind::DevRoundLog,
                ArtifactKind::AssetProductionLedger,
                ArtifactKind::AssetGenome,
            ],
        ),
        stage(
            "P4",
            &[
                ArtifactKind::ProofBundle,
                ArtifactKind::ProofVerdict,
                ArtifactKind::RepairQueue,
            ],
            &[
                ArtifactKind::AssemblyReport,
                ArtifactKind::PlayableSlice,
                ArtifactKind::EngineGuide,
            ],
        ),
        stage(
            "P5",
            &[ArtifactKind::DeliveryManifest],
            &[
                ArtifactKind::ProofVerdict,
                ArtifactKind::AssemblyReport,
                ArtifactKind::AssetGenome,
            ],
        ),
    ]
}

/// 按 `depends_on` 推导拓扑序（Kahn 算法，同层按 registry 顺序稳定输出）。
///
/// 稳定性是刻意的：同一份 registry 每次都得到逐字相同的顺序，报告与测试才能直接断言。
/// 成环时返回 `Err` 并**点名**还没解开的那几段——只说「有环」等于让人自己去找。
pub fn topological_order(registry: &[StageSpec]) -> Adm4Result<Vec<String>> {
    let known: BTreeSet<&str> = registry.iter().map(|stage| stage.id.as_str()).collect();
    if known.len() != registry.len() {
        return Err(Adm4Error::validation(
            "阶段 registry 存在重复 id：阶段 id 必须唯一",
        ));
    }
    let mut pending: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for stage in registry {
        let mut unmet = BTreeSet::new();
        for dependency in &stage.depends_on {
            if !known.contains(dependency.as_str()) {
                return Err(Adm4Error::not_found(format!(
                    "阶段 {} 依赖了不存在的阶段 {dependency}",
                    stage.id
                )));
            }
            unmet.insert(dependency.as_str());
        }
        pending.insert(stage.id.as_str(), unmet);
    }

    let mut ordered: Vec<String> = Vec::with_capacity(registry.len());
    let mut settled: BTreeSet<&str> = BTreeSet::new();
    while ordered.len() < registry.len() {
        // 按 registry 声明顺序取第一个依赖已全部就绪的段：结果稳定且贴近人读顺序。
        let ready = registry.iter().find(|stage| {
            if settled.contains(stage.id.as_str()) {
                return false;
            }
            match pending.get(stage.id.as_str()) {
                Some(unmet) => unmet.iter().all(|item| settled.contains(item)),
                None => false,
            }
        });
        let Some(stage) = ready else {
            let stuck: Vec<&str> = registry
                .iter()
                .map(|stage| stage.id.as_str())
                .filter(|id| !settled.contains(id))
                .collect();
            return Err(Adm4Error::validation(format!(
                "阶段依赖成环，无法推导执行顺序：{} 互相等待",
                stuck.join(" → ")
            )));
        };
        settled.insert(stage.id.as_str());
        ordered.push(stage.id.clone());
    }
    Ok(ordered)
}

/// 校验制品依赖图与阶段 registry 是否自洽。
///
/// 四类结构错误一次拦住：
/// 1. 声明表与 registry 的阶段集合对不上（有段没声明制品，或声明了不存在的段）；
/// 2. 同一个制品有两个产出者（制品是单点锚定的，两个产出者等于两个真源）；
/// 3. 消费了一个 Phase 2 里没人产、又不是外部输入的制品（R2：来源不明就停）；
/// 4. 制品的产出者排在消费者之后（拓扑序上够不着 = 运行时必然拿不到）。
pub fn validate_artifact_graph(
    registry: &[StageSpec],
    artifacts: &[StageArtifacts],
) -> Adm4Result<Vec<String>> {
    let order = topological_order(registry)?;
    let position: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect();

    let declared: BTreeSet<&str> = artifacts
        .iter()
        .map(|item| item.stage_id.as_str())
        .collect();
    if declared.len() != artifacts.len() {
        return Err(Adm4Error::validation(
            "制品声明表里有重复阶段：每段只能声明一次产出与消费",
        ));
    }
    for stage in registry {
        if !declared.contains(stage.id.as_str()) {
            return Err(Adm4Error::validation(format!(
                "阶段 {} 没有制品声明：强设计关联要求每段显式声明产出与依赖（D24）",
                stage.id
            )));
        }
    }
    for item in artifacts {
        if !position.contains_key(item.stage_id.as_str()) {
            return Err(Adm4Error::not_found(format!(
                "制品声明表里的阶段 {} 不在阶段 registry 内",
                item.stage_id
            )));
        }
    }

    let mut producer: BTreeMap<ArtifactKind, &str> = BTreeMap::new();
    for item in artifacts {
        for kind in &item.produces {
            if kind.is_external_input() {
                return Err(Adm4Error::validation(format!(
                    "阶段 {} 声称产出 {}：它由 Phase 2 之外供给，重造它就是第二真源（D22）",
                    item.stage_id,
                    kind.label_zh()
                )));
            }
            if let Some(existing) = producer.insert(*kind, item.stage_id.as_str()) {
                return Err(Adm4Error::conflict(format!(
                    "制品 {} 有两个产出者（{} 与 {}）：制品必须单点产出",
                    kind.label_zh(),
                    existing,
                    item.stage_id
                )));
            }
        }
    }

    for item in artifacts {
        let Some(consumer_position) = position.get(item.stage_id.as_str()) else {
            continue;
        };
        for kind in &item.consumes {
            if kind.is_external_input() {
                continue;
            }
            let Some(producer_stage) = producer.get(kind) else {
                return Err(Adm4Error::validation(format!(
                    "阶段 {} 消费的制品 {} 在 Phase 2 里没有产出者：来源不明就不能开跑（R2）",
                    item.stage_id,
                    kind.label_zh()
                )));
            };
            let Some(producer_position) = position.get(producer_stage) else {
                continue;
            };
            if producer_position >= consumer_position {
                return Err(Adm4Error::validation(format!(
                    "阶段 {} 消费的制品 {} 由 {} 产出，但 {} 在执行顺序上不早于它：\
                     依赖声明与制品图对不上",
                    item.stage_id,
                    kind.label_zh(),
                    producer_stage,
                    producer_stage
                )));
            }
        }
    }
    Ok(order)
}

/// Phase 2 版图的一次完整自检（registry + 制品声明），返回拓扑序。
pub fn phase2_execution_order() -> Adm4Result<Vec<String>> {
    validate_artifact_graph(&phase2_registry(), &phase2_artifacts())
}

/// 某个制品的产出阶段（后续波次接活时用来定位「我要的东西谁给我」）。
pub fn producer_of(kind: ArtifactKind) -> Option<String> {
    phase2_artifacts()
        .into_iter()
        .find(|item| item.produces.contains(&kind))
        .map(|item| item.stage_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_pipeline::StageKind;

    fn stage(id: &str, depends: &[&str]) -> StageSpec {
        StageSpec {
            id: id.into(),
            name: id.into(),
            kind: StageKind::Deterministic,
            depends_on: depends.iter().map(|item| (*item).to_string()).collect(),
            summary: String::new(),
        }
    }

    #[test]
    fn phase2_graph_is_consistent_and_topologically_ordered() {
        let order = phase2_execution_order().expect("Phase 2 版图必须自洽");
        assert_eq!(order, vec!["P0", "P1", "P2", "P3", "P4", "P5"]);
    }

    /// 每个 P 段都必须有制品声明，且声明的 id 与 registry 完全一致。
    #[test]
    fn every_registry_stage_declares_artifacts() {
        let registry = phase2_registry();
        let artifacts = phase2_artifacts();
        assert_eq!(registry.len(), artifacts.len());
        for (stage, declared) in registry.iter().zip(artifacts.iter()) {
            assert_eq!(stage.id, declared.stage_id);
            assert!(
                !declared.produces.is_empty(),
                "{} 必须至少产出一件制品，否则它在版图里没有存在意义",
                stage.id
            );
        }
    }

    #[test]
    fn producer_lookup_points_at_the_single_owner() {
        assert_eq!(
            producer_of(ArtifactKind::AlignmentReport).as_deref(),
            Some("P0")
        );
        assert_eq!(
            producer_of(ArtifactKind::AssetGenome).as_deref(),
            Some("P2")
        );
        assert_eq!(
            producer_of(ArtifactKind::DeliveryManifest).as_deref(),
            Some("P5")
        );
        // 外部输入在 Phase 2 内没有产出者：查得到「没有」本身就是结论。
        assert_eq!(producer_of(ArtifactKind::GameSpec), None);
        assert!(ArtifactKind::StyleAnchorSet.is_external_input());
        assert!(!ArtifactKind::ProofBundle.is_external_input());
    }

    /// 负例：依赖成环必须被检出并点名，不许悄悄少排一段。
    #[test]
    fn cyclic_dependencies_are_detected_and_named() {
        let cyclic = vec![
            stage("X0", &["X2"]),
            stage("X1", &["X0"]),
            stage("X2", &["X1"]),
        ];
        let error = topological_order(&cyclic).expect_err("成环必须报错");
        assert!(error.message.contains("成环"), "{}", error.message);
        for id in ["X0", "X1", "X2"] {
            assert!(error.message.contains(id), "报错应点名 {id}");
        }

        // 自环也是环。
        let self_loop = vec![stage("X0", &["X0"])];
        assert!(topological_order(&self_loop).is_err());
    }

    #[test]
    fn unknown_and_duplicate_stage_ids_are_rejected() {
        let dangling = vec![stage("X0", &["X9"])];
        assert_eq!(
            topological_order(&dangling).unwrap_err().kind,
            adm4_foundation::Adm4ErrorKind::NotFound
        );
        let duplicated = vec![stage("X0", &[]), stage("X0", &[])];
        assert!(topological_order(&duplicated).is_err());
    }

    #[test]
    fn topological_order_is_stable_for_independent_stages() {
        let independent = vec![stage("B", &[]), stage("A", &[]), stage("C", &["A"])];
        let first = topological_order(&independent).expect("无环");
        let second = topological_order(&independent).expect("无环");
        assert_eq!(first, second);
        // 同层按 registry 声明顺序，而不是字典序（B 先声明就先排）。
        assert_eq!(first, vec!["B", "A", "C"]);
    }

    /// 负例：消费一个没人产的制品 → 来源不明，直接报错。
    #[test]
    fn consuming_an_unproduced_artifact_is_rejected() {
        let registry = vec![stage("X0", &[]), stage("X1", &["X0"])];
        let artifacts = vec![
            StageArtifacts {
                stage_id: "X0".into(),
                produces: vec![ArtifactKind::ProgramContract],
                consumes: vec![ArtifactKind::GameSpec],
            },
            StageArtifacts {
                stage_id: "X1".into(),
                produces: vec![ArtifactKind::ProofBundle],
                consumes: vec![ArtifactKind::AssetGenome],
            },
        ];
        let error = validate_artifact_graph(&registry, &artifacts).unwrap_err();
        assert!(error.message.contains("没有产出者"), "{}", error.message);
    }

    /// 负例：两个阶段产出同一个制品 → 单点锚定被破坏。
    #[test]
    fn duplicate_producers_are_rejected() {
        let registry = vec![stage("X0", &[]), stage("X1", &["X0"])];
        let artifacts = vec![
            StageArtifacts {
                stage_id: "X0".into(),
                produces: vec![ArtifactKind::ArtContract],
                consumes: Vec::new(),
            },
            StageArtifacts {
                stage_id: "X1".into(),
                produces: vec![ArtifactKind::ArtContract],
                consumes: Vec::new(),
            },
        ];
        assert_eq!(
            validate_artifact_graph(&registry, &artifacts)
                .unwrap_err()
                .kind,
            adm4_foundation::Adm4ErrorKind::Conflict
        );
    }

    /// 负例：产出者排在消费者之后 → 运行时必然拿不到，声明阶段就要拦。
    #[test]
    fn producer_after_consumer_is_rejected() {
        let registry = vec![stage("X0", &[]), stage("X1", &["X0"])];
        let artifacts = vec![
            StageArtifacts {
                stage_id: "X0".into(),
                produces: vec![ArtifactKind::AlignmentReport],
                consumes: vec![ArtifactKind::ProgramContract],
            },
            StageArtifacts {
                stage_id: "X1".into(),
                produces: vec![ArtifactKind::ProgramContract],
                consumes: Vec::new(),
            },
        ];
        let error = validate_artifact_graph(&registry, &artifacts).unwrap_err();
        assert!(error.message.contains("不早于"), "{}", error.message);
    }

    /// 负例：声称产出外部输入 = 重造第二真源。
    #[test]
    fn producing_an_external_input_is_rejected() {
        let registry = vec![stage("X0", &[])];
        let artifacts = vec![StageArtifacts {
            stage_id: "X0".into(),
            produces: vec![ArtifactKind::GameSpec],
            consumes: Vec::new(),
        }];
        let error = validate_artifact_graph(&registry, &artifacts).unwrap_err();
        assert!(error.message.contains("第二真源"), "{}", error.message);
    }

    /// 负例：有段没声明制品 → 强设计关联不成立。
    #[test]
    fn stage_without_artifact_declaration_is_rejected() {
        let registry = vec![stage("X0", &[]), stage("X1", &["X0"])];
        let artifacts = vec![StageArtifacts {
            stage_id: "X0".into(),
            produces: vec![ArtifactKind::ProgramContract],
            consumes: Vec::new(),
        }];
        let error = validate_artifact_graph(&registry, &artifacts).unwrap_err();
        assert!(error.message.contains("没有制品声明"), "{}", error.message);
    }

    #[test]
    fn stage_artifacts_round_trip_through_json() {
        let artifacts = phase2_artifacts();
        let json = serde_json::to_string_pretty(&artifacts).expect("序列化");
        let back: Vec<StageArtifacts> = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, artifacts);
        // 旧档：只有 stage_id 的历史声明照旧可读（产出/消费落空表）。
        let legacy: StageArtifacts =
            serde_json::from_str(r#"{"stage_id":"P0"}"#).expect("旧档应可解析");
        assert!(legacy.produces.is_empty());
        assert!(legacy.consumes.is_empty());
    }
}
