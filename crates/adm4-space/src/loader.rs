use crate::model::{DesignSpace, GenrePack, UniversalLayer};
use crate::validate::validate_design_space;
use adm4_decision::{DecisionGraph, DesignOrganization};
use adm4_foundation::{Adm4Error, Adm4Result, read_json_file};
use std::fs;
use std::path::{Path, PathBuf};

/// 设计空间根目录（`knowledge/design_space/`）。
#[derive(Debug, Clone)]
pub struct DesignSpaceRoot {
    pub root: PathBuf,
}

impl DesignSpaceRoot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn universal_dir(&self) -> PathBuf {
        self.root.join("universal")
    }

    pub fn pack_dir(&self, pack_id: &str) -> PathBuf {
        self.root.join(pack_id)
    }

    /// 列出可用品类包（含 pack.json 的子目录）。
    pub fn list_packs(&self) -> Adm4Result<Vec<String>> {
        let mut packs = Vec::new();
        let entries = fs::read_dir(&self.root).map_err(|error| {
            Adm4Error::io(format!(
                "read design space root {} failed: {error}",
                self.root.display()
            ))
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path.join("pack.json").is_file()
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
                && !name.starts_with('_')
                && name != "universal"
            {
                packs.push(name.to_string());
            }
        }
        packs.sort();
        Ok(packs)
    }
}

pub fn load_pack_file(path: &Path) -> Adm4Result<GenrePack> {
    read_json_file(path)
}

fn load_universal(root: &DesignSpaceRoot) -> Adm4Result<UniversalLayer> {
    let dir = root.universal_dir();
    let mut merged: Option<UniversalLayer> = None;
    let entries = fs::read_dir(&dir).map_err(|error| {
        Adm4Error::io(format!(
            "read universal layer dir {} failed: {error}",
            dir.display()
        ))
    })?;
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(Adm4Error::not_found(format!(
            "no universal layer json under {}",
            dir.display()
        )));
    }
    for file in files {
        let layer: UniversalLayer = read_json_file(&file)?;
        match &mut merged {
            None => merged = Some(layer),
            Some(existing) => {
                if existing.space_version != layer.space_version {
                    return Err(Adm4Error::validation(format!(
                        "universal layer version mismatch: {} vs {}",
                        existing.space_version, layer.space_version
                    )));
                }
                existing.decision_points.extend(layer.decision_points);
                existing.domains.extend(layer.domains);
                existing.nodes.extend(layer.nodes);
            }
        }
    }
    merged.ok_or_else(|| {
        Adm4Error::not_found(format!(
            "通用层目录 {} 未加载到任何决策点清单",
            dir.display()
        ))
    })
}

/// 加载「通用层 + 指定品类包」并完成全部校验；违例即返回错误（fail-closed）。
pub fn load_design_space(root: &DesignSpaceRoot, pack_id: &str) -> Adm4Result<DesignSpace> {
    let universal = load_universal(root)?;
    let pack_path = root.pack_dir(pack_id).join("pack.json");
    if !pack_path.is_file() {
        return Err(Adm4Error::not_found(format!(
            "品类包 {pack_id} 不存在（未找到 {}，可用 space validate 查看可用包）",
            pack_path.display()
        )));
    }
    let pack = load_pack_file(&pack_path)?;
    if pack.pack_id != pack_id {
        return Err(Adm4Error::validation(format!(
            "pack.json 声明的 pack_id={} 与目录名 {pack_id} 不一致",
            pack.pack_id
        )));
    }
    let mut points = universal.decision_points.clone();
    points.extend(pack.decision_points.clone());
    let graph = DecisionGraph::new(points)?;
    // 组织维度：通用层领域 + 通用层节点 + 本包节点（保留领域/节点由装配内置）。
    let declared_domains = universal.domains.clone();
    let mut declared_nodes = universal.nodes.clone();
    declared_nodes.extend(pack.nodes.clone());
    let space = DesignSpace {
        universal_version: universal.space_version,
        pack,
        graph,
        organization: DesignOrganization::new(declared_domains.clone(), declared_nodes.clone()),
    };
    let violations = validate_design_space(
        &space,
        &universal.decision_points,
        &declared_domains,
        &declared_nodes,
    );
    if !violations.is_empty() {
        let summary = violations
            .iter()
            .map(|violation| format!("[{}] {}", violation.code, violation.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Adm4Error::blocked(format!(
            "design space validation failed with {} violations: {summary}",
            violations.len()
        )));
    }
    Ok(space)
}
