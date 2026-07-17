use adm_new_foundation::new_stable_id;
use adm_new_pipeline::cross_genre_evaluation::{
    A09EvaluationStatus, A09MeasurementOptions, A09ProductionScope, run_a09_cross_genre_evaluation,
    run_a09_cross_genre_evaluation_with_options,
};

#[test]
fn a09_runs_eight_spec_level_samples_one_r1_reference_and_three_r2_full_production_slices() {
    let root = temp_root("a09_cross_genre");

    let report = run_a09_cross_genre_evaluation(&root).unwrap();

    assert_eq!(report.status, A09EvaluationStatus::Passed);
    assert_eq!(report.spec_level_results.len(), 8);
    assert_eq!(
        report
            .spec_level_results
            .iter()
            .filter(|result| result.production_scope == A09ProductionScope::FullProduction)
            .count(),
        3
    );
    assert!(
        report
            .spec_level_results
            .iter()
            .any(
                |result| result.production_scope == A09ProductionScope::ArchitectureOnly
                    && result.sample_id == "network_coop_sample"
            )
    );
    assert_eq!(report.full_production_results.len(), 4);
    assert!(
        report
            .spec_level_results
            .iter()
            .all(|result| result.blockers.is_empty())
    );
    assert!(
        report
            .full_production_results
            .iter()
            .all(|result| result.blockers.is_empty())
    );
    assert!(
        report
            .full_production_results
            .iter()
            .all(|result| result.manual_signature_required)
    );
    assert!(
        report
            .full_production_results
            .iter()
            .all(|result| result.step14_status == "Blocked")
    );
    assert_eq!(report.third_layer_anti_overfit.mutation_rejection_count, 8);
    assert!(report.third_layer_anti_overfit.repeated_runs_stable);
    assert!(report.third_layer_anti_overfit.fault_injection_blocked);
    assert!(report.source_scan.hits.is_empty());
    assert!(root.join("a09_cross_genre_evaluation_report.json").exists());
    assert!(root.join("a09_spec_level_matrix.json").exists());
    assert!(root.join("a09_full_production_matrix.json").exists());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a09_blocks_when_required_third_layer_measurements_are_disabled() {
    let root = temp_root("a09_measurement_disabled");

    let report = run_a09_cross_genre_evaluation_with_options(
        &root,
        A09MeasurementOptions {
            run_no_ai_path: false,
            run_bounded_ai_repetition: false,
            run_mutation_rejection: true,
        },
    )
    .unwrap();

    assert_eq!(report.status, A09EvaluationStatus::Blocked);
    assert!(!report.third_layer_anti_overfit.no_ai_mode_supported);
    assert!(!report.third_layer_anti_overfit.bounded_ai_repeat_stable);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a09_blocks_when_mutation_rejection_measurement_is_disabled() {
    let root = temp_root("a09_mutation_rejection_disabled");

    let report = run_a09_cross_genre_evaluation_with_options(
        &root,
        A09MeasurementOptions {
            run_no_ai_path: true,
            run_bounded_ai_repetition: true,
            run_mutation_rejection: false,
        },
    )
    .unwrap();

    assert_eq!(report.status, A09EvaluationStatus::Blocked);
    assert_eq!(report.third_layer_anti_overfit.mutation_rejection_count, 0);
    assert_eq!(
        report.third_layer_anti_overfit.mutation_rejections_required,
        8
    );

    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    std::fs::create_dir_all(&root).unwrap();
    root
}
