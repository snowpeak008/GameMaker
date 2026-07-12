use std::fs;
use std::path::PathBuf;

use adm_new_application::{SdkKnowledgeApplicationService, SkillOverlayApplicationService};
use adm_new_contracts::sdk::SdkReviewStatus;
use adm_new_foundation::new_stable_id;
use adm_new_sdk::SdkKnowledgeBase;

#[test]
fn desktop_service_and_cli_repository_share_the_same_sdk_overlay_across_restart() {
    let root = temp_root("application_sdk_shared");
    let resource_root = root.join("resources");
    let data_root = root.join("data");
    let seed = SdkKnowledgeBase::from_project_root(&resource_root);
    seed.initialize().unwrap();

    let mut desktop = SdkKnowledgeApplicationService::open(&resource_root, &data_root).unwrap();
    desktop
        .add_placeholder_with_source_url("steamworks", "Steamworks", "https://ui.test")
        .unwrap();
    desktop.persist().unwrap();

    let cli = SdkKnowledgeBase::from_project_and_data_roots(&resource_root, &data_root);
    assert_eq!(
        cli.read_spec("steamworks").unwrap().unwrap().source_url,
        "https://ui.test"
    );
    cli.add_placeholder("EOS", "https://cli.test").unwrap();
    cli.update_review_status("eos", SdkReviewStatus::Approved)
        .unwrap();

    let restarted = SdkKnowledgeApplicationService::open(&resource_root, &data_root).unwrap();
    assert_eq!(restarted.list_specs().len(), 2);
    assert!(restarted.approved_context().contains("EOS"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn skill_application_service_creates_real_extension_overlay_and_reads_seed() {
    let root = temp_root("application_skill_shared");
    let resource_root = root.join("resources");
    let data_root = root.join("data");
    let seed = resource_root.join("knowledge/skills/dev/run.json");
    fs::create_dir_all(seed.parent().unwrap()).unwrap();
    fs::write(&seed, r#"{"name":"run"}"#).unwrap();

    let service = SkillOverlayApplicationService::open(&resource_root, &data_root).unwrap();

    assert!(service.overlay_root().is_dir());
    assert_eq!(service.list().unwrap().len(), 1);
    let _ = fs::remove_dir_all(root);
}

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    fs::create_dir_all(&root).unwrap();
    root
}
