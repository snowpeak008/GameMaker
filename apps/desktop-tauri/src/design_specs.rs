use std::path::{Path, PathBuf};

use adm_new_application::{DesignChecklistItemSpec, DesignNodeSpec, DesignOptionGroupSpec};
use adm_new_design::data_loader::DesignDataLoader;
use adm_new_foundation::{AdmError, AdmResult};
use adm_new_packaging::verify_portable_resource_root;

#[cfg(debug_assertions)]
use adm_new_foundation::source_root::{ROOT_MARKER, SourceProjectRoot};
#[cfg(debug_assertions)]
use adm_new_packaging::PORTABLE_BUILD_MANIFEST;

#[derive(Debug)]
pub struct LoadedDesignSpecs {
    pub specs: Vec<DesignNodeSpec>,
    pub resource_root: PathBuf,
}

pub fn load_design_specs() -> AdmResult<LoadedDesignSpecs> {
    #[cfg(not(debug_assertions))]
    {
        let executable = std::env::current_exe().map_err(|error| {
            AdmError::new(format!(
                "unable to locate the release executable for portable resources: {error}"
            ))
        })?;
        let root = executable.parent().ok_or_else(|| {
            AdmError::new("release executable has no parent portable resource directory")
        })?;
        return load_design_specs_from_portable_root(root);
    }

    #[cfg(debug_assertions)]
    load_debug_design_specs()
}

pub fn load_design_specs_from_portable_root(
    root: impl AsRef<Path>,
) -> AdmResult<LoadedDesignSpecs> {
    let root = root.as_ref();
    let report = verify_portable_resource_root(root);
    if !report.passed() {
        let blockers = report
            .blockers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(AdmError::new(format!(
            "portable resource root failed integrity verification at {}: {blockers}",
            root.display()
        )));
    }
    load_design_specs_from_validated_root(root)
}

#[cfg(debug_assertions)]
fn load_debug_design_specs() -> AdmResult<LoadedDesignSpecs> {
    if let Some(root) = std::env::var_os("ADM_NEWRUST_SOURCE_ROOT") {
        let source_root = SourceProjectRoot::open(PathBuf::from(root))?;
        return load_design_specs_from_validated_root(source_root.path());
    }

    if let Ok(executable) = std::env::current_exe()
        && let Some(executable_root) = executable.parent()
    {
        if path_entry_exists(&executable_root.join(PORTABLE_BUILD_MANIFEST))? {
            return load_design_specs_from_portable_root(executable_root);
        }
    }

    if let Ok(current) = std::env::current_dir()
        && nearest_marker_exists(&current)?
    {
        let source_root = SourceProjectRoot::discover(current)?;
        return load_design_specs_from_validated_root(source_root.path());
    }

    let compiled_source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source_root = SourceProjectRoot::open(compiled_source_root)?;
    load_design_specs_from_validated_root(source_root.path())
}

fn load_design_specs_from_validated_root(root: &Path) -> AdmResult<LoadedDesignSpecs> {
    let loader = DesignDataLoader::new(root);
    let domains = loader.load_domains().map_err(|error| {
        AdmError::new(format!(
            "failed to load required design data from {}: {error}",
            loader.design_data_dir().display()
        ))
    })?;
    let specs = domains
        .into_iter()
        .flat_map(|domain| {
            let domain_id = domain.domain.id;
            domain.nodes.into_iter().map(move |node| DesignNodeSpec {
                node_id: node.id,
                domain_id: if node.domain.trim().is_empty() {
                    domain_id.clone()
                } else {
                    node.domain
                },
                name: node.name,
                description: node.description,
                role_class: node.role_class,
                checklist: node
                    .checklist
                    .into_iter()
                    .map(|item| DesignChecklistItemSpec {
                        item_id: item.id,
                        label: item.label,
                        option_groups: item
                            .option_groups
                            .into_iter()
                            .map(|group| DesignOptionGroupSpec {
                                group_id: group.id,
                                selection_mode: group.selection_mode,
                                allow_primary: group.allow_primary,
                                options: group
                                    .options
                                    .into_iter()
                                    .map(|option| option.id)
                                    .filter(|id| !id.trim().is_empty())
                                    .collect(),
                            })
                            .collect(),
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    if specs.is_empty() {
        return Err(AdmError::new(format!(
            "required design data contained no nodes: {}",
            loader.design_data_dir().display()
        )));
    }
    let resource_root = root.canonicalize().map_err(|error| {
        AdmError::new(format!(
            "unable to canonicalize validated resource root {}: {error}",
            root.display()
        ))
    })?;
    Ok(LoadedDesignSpecs {
        specs,
        resource_root,
    })
}

#[cfg(debug_assertions)]
fn nearest_marker_exists(start: &Path) -> AdmResult<bool> {
    let mut current = start.canonicalize().map_err(|error| {
        AdmError::new(format!(
            "unable to inspect debug source root from {}: {error}",
            start.display()
        ))
    })?;
    if current.is_file() {
        current = current
            .parent()
            .ok_or_else(|| AdmError::new("debug source root search start has no parent"))?
            .to_path_buf();
    }
    loop {
        if path_entry_exists(&current.join(ROOT_MARKER))? {
            return Ok(true);
        }
        let Some(parent) = current.parent() else {
            return Ok(false);
        };
        if parent == current {
            return Ok(false);
        }
        current = parent.to_path_buf();
    }
}

#[cfg(debug_assertions)]
fn path_entry_exists(path: &Path) -> AdmResult<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AdmError::new(format!(
            "unable to inspect debug resource path {}: {error}",
            path.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_design_specs_load_from_a_fully_validated_source_root() {
        let loaded = load_design_specs().unwrap();
        assert!(!loaded.specs.is_empty());
        let domain_count = loaded
            .specs
            .iter()
            .map(|spec| spec.domain_id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert!(domain_count >= 16);
        assert!(loaded.resource_root.join(".project_root").is_file());
    }

    #[test]
    fn portable_design_specs_fail_closed_for_an_incomplete_root() {
        let root =
            std::env::temp_dir().join(adm_new_foundation::new_stable_id("resource-root").unwrap());
        std::fs::create_dir_all(root.join("knowledge/design_data/domains")).unwrap();

        let error = load_design_specs_from_portable_root(&root).unwrap_err();

        assert!(error.message().contains("failed integrity verification"));
        let _ = std::fs::remove_dir_all(root);
    }
}
