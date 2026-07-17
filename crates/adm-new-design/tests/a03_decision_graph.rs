use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use adm_new_design::anti_overfit::{
    CapabilityMutation, apply_capability_mutation, capability_mutation_suite,
    permute_display_labels,
};
use adm_new_design::data_loader::{DesignDataLoader, DomainDocument};
use adm_new_design::decision_graph::{
    CapabilityDecisionGraph, CapabilityDecisionGraphCompiler, DecisionCoverage,
};
use adm_new_game_spec::{
    ConditionExpr, ConditionSpec, GameSpec, ParticipantMode, canonicalize_game_spec,
    parse_game_spec,
};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("design crate must be located under the workspace crates directory")
        .to_path_buf()
}

fn domains() -> Vec<DomainDocument> {
    DesignDataLoader::new(project_root())
        .load_domains()
        .expect("real knowledge domains must load")
}

fn fixture(name: &str) -> GameSpec {
    let source = std::fs::read_to_string(
        project_root()
            .join("testdata/game_spec")
            .join(format!("{name}.json")),
    )
    .expect("fixture must exist");
    parse_game_spec(&source).expect("fixture must pass strict parsing")
}

fn compile(name: &str, domains: &[DomainDocument]) -> CapabilityDecisionGraph {
    CapabilityDecisionGraphCompiler
        .compile(&fixture(name), domains, &DecisionCoverage::default())
        .expect("decision graph must compile")
}

#[test]
fn real_knowledge_inventory_is_activation_aware() {
    let domains = domains();
    let graph = compile("lane_guard", &domains);
    assert_eq!(graph.coverage.knowledge_domain_count, 16);
    assert_eq!(graph.coverage.knowledge_node_count, 103);
    assert_eq!(graph.coverage.knowledge_checklist_count, 515);
    assert!(graph.coverage.relevant_node_count < 103);
    assert_eq!(graph.coverage.active_node_count, graph.active_nodes.len());
    graph
        .validate_activation_evidence()
        .expect("every activation must carry complete evidence");
}

#[test]
fn repeated_compilation_is_byte_deterministic() {
    let domains = domains();
    let first = compile("lane_guard", &domains);
    let second = compile("lane_guard", &domains);
    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn structurally_distinct_fixtures_produce_distinct_activation_sets() {
    let domains = domains();
    let names = [
        "lane_guard",
        "match_grid",
        "branching_story",
        "turn_tactics",
    ];
    let sets = names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                compile(name, &domains).activation_set(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for left in names {
        for right in names {
            if left < right {
                assert_ne!(sets[left], sets[right], "{left} and {right} collapsed");
            }
        }
    }
    assert!(sets.values().all(|set| !set.is_empty()));
}

#[test]
fn display_label_permutation_preserves_graph_semantics() {
    let domains = domains();
    let source = fixture("lane_guard");
    let permuted = permute_display_labels(&source, "opaque_7391").unwrap();
    let compiler = CapabilityDecisionGraphCompiler;
    let baseline = compiler
        .compile(&source, &domains, &DecisionCoverage::default())
        .unwrap();
    let changed = compiler
        .compile(&permuted, &domains, &DecisionCoverage::default())
        .unwrap();
    assert_eq!(baseline.semantics(), changed.semantics());
    assert_eq!(baseline, changed);
}

#[test]
fn display_label_permutation_covers_tags_without_breaking_tag_conditions() {
    let mut source = fixture("lane_guard");
    let (entity_id, original_tag) = source
        .entities
        .iter()
        .find_map(|(entity_id, entity)| {
            entity
                .tags
                .iter()
                .next()
                .map(|tag| (entity_id.clone(), tag.clone()))
        })
        .expect("fixture must contain at least one tagged entity");
    source
        .acceptance_scenarios
        .values_mut()
        .next()
        .expect("fixture must contain a scenario")
        .given
        .push(ConditionSpec {
            description: "Tagged entity remains semantically linked after label permutation."
                .to_string(),
            reads: Vec::new(),
            expression: ConditionExpr::HasTag {
                entity: entity_id.clone(),
                tag: original_tag.clone(),
            },
        });

    let permuted = permute_display_labels(&source, "opaque_tags").unwrap();
    let permuted_entity = permuted.entities.get(&entity_id).unwrap();
    assert!(!permuted_entity.tags.contains(&original_tag));
    let replacement_tag = permuted_entity
        .tags
        .iter()
        .find(|tag| tag.starts_with("opaque_tags_entity_"))
        .expect("entity tags must be replaced")
        .clone();
    assert!(permuted.acceptance_scenarios.values().any(|scenario| {
        scenario.given.iter().any(|condition| {
            matches!(
                &condition.expression,
                ConditionExpr::HasTag { entity, tag }
                    if entity == &entity_id && tag == &replacement_tag
            )
        })
    }));
    assert!(permuted.spaces.values().any(|space| {
        space.regions.values().any(|region| {
            !region.tags.is_empty()
                && region
                    .tags
                    .iter()
                    .all(|tag| tag.starts_with("opaque_tags_region_"))
        })
    }));
}

#[test]
fn one_capability_mutation_changes_the_activation_graph() {
    let domains = domains();
    let source = fixture("lane_guard");
    let mutated = apply_capability_mutation(
        &source,
        CapabilityMutation::ParticipantMode(ParticipantMode::Networked),
    )
    .unwrap();
    let compiler = CapabilityDecisionGraphCompiler;
    let baseline = compiler
        .compile(&source, &domains, &DecisionCoverage::default())
        .unwrap();
    let changed = compiler
        .compile(&mutated, &domains, &DecisionCoverage::default())
        .unwrap();
    assert_ne!(baseline.activation_set(), changed.activation_set());
    assert!(
        changed
            .active_nodes
            .iter()
            .any(|node| node.activation_reasons.iter().any(|reason| {
                reason.source_path == "/capabilities/participants/mode"
                    && reason.actual == "networked"
            }))
    );
}

#[test]
fn capability_mutation_suite_covers_all_declared_axes() {
    let source = fixture("lane_guard");
    let baseline_hash = canonicalize_game_spec(&source).unwrap().content_hash;
    let suite = capability_mutation_suite(&source);

    assert_eq!(suite.len(), 16);
    assert_eq!(
        suite
            .iter()
            .map(|case| case.axis)
            .collect::<BTreeSet<_>>()
            .len(),
        16
    );
    for case in suite {
        let mutated = apply_capability_mutation(&source, case.mutation).unwrap();
        let mutated_hash = canonicalize_game_spec(&mutated).unwrap().content_hash;
        assert_ne!(
            baseline_hash, mutated_hash,
            "{} mutation must change the canonical GameSpec hash",
            case.axis
        );
    }
}

#[test]
fn satisfied_relevant_constraints_leave_the_active_graph() {
    let domains = domains();
    let spec = fixture("lane_guard");
    let compiler = CapabilityDecisionGraphCompiler;
    let baseline = compiler
        .compile(&spec, &domains, &DecisionCoverage::default())
        .unwrap();
    let target = baseline
        .active_nodes
        .first()
        .expect("fixture must activate a node");
    let mut coverage = DecisionCoverage::default();
    for constraint in &target.unresolved_constraints {
        coverage.satisfy(&target.node_id, &constraint.checklist_id);
    }

    let changed = compiler.compile(&spec, &domains, &coverage).unwrap();
    assert!(!changed.activation_set().contains(&target.node_id));
    assert_eq!(
        changed.coverage.relevant_node_count,
        baseline.coverage.relevant_node_count
    );
    assert_eq!(changed.coverage.resolved_node_count, 1);
    assert!(changed.coverage.completion_percent > 0);
}

#[test]
fn unknown_coverage_is_rejected_fail_closed() {
    let domains = domains();
    let mut coverage = DecisionCoverage::default();
    coverage.satisfy("missing_node", "missing_constraint");
    let error = CapabilityDecisionGraphCompiler
        .compile(&fixture("lane_guard"), &domains, &coverage)
        .unwrap_err();
    assert_eq!(error.code, "decision_graph.unknown_coverage_constraint");
    assert_eq!(error.path, "/coverage/missing_node/missing_constraint");
}

#[test]
fn requires_any_cycles_are_rejected_like_hard_dependency_cycles() {
    let mut domains = domains();
    let baseline = compile("lane_guard", &domains);
    let first = baseline.active_nodes[0].node_id.clone();
    let second = baseline.active_nodes[1].node_id.clone();
    for domain in &mut domains {
        for node in &mut domain.nodes {
            if node.id == first {
                node.requires_any.push(second.clone());
            }
            if node.id == second {
                node.requires_any.push(first.clone());
            }
        }
    }

    let error = CapabilityDecisionGraphCompiler
        .compile(
            &fixture("lane_guard"),
            &domains,
            &DecisionCoverage::default(),
        )
        .unwrap_err();

    assert_eq!(error.code, "decision_graph.dependency_cycle");
}

#[test]
fn compiler_sources_contain_no_sample_or_type_branches() {
    let source_root = project_root().join("crates/adm-new-design/src");
    let sources = [
        source_root.join("decision_graph/mod.rs"),
        source_root.join("decision_graph/policy.rs"),
        source_root.join("anti_overfit.rs"),
    ];
    let forbidden = [
        "lane_guard",
        "match_grid",
        "branching_story",
        "turn_tactics",
        "tower_defense",
        "match_three",
        "visual_novel",
        "turn_tactics",
    ];
    let mut hits = BTreeSet::new();
    for path in sources {
        let source = std::fs::read_to_string(&path).unwrap();
        for token in forbidden {
            if source.to_ascii_lowercase().contains(token) {
                hits.insert(format!("{}:{token}", path.display()));
            }
        }
    }
    assert!(hits.is_empty(), "type-specific compiler tokens: {hits:?}");
}
