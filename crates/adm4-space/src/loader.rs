use crate::model::{DesignSpace, GenrePack, SystemRef, UniversalLayer};
use crate::system_loader::{instantiate_system_refs, load_modules_from_dirs};
use crate::validate::validate_design_space;
use adm4_decision::system_module::SystemModule;
use adm4_decision::{DecisionGraph, DesignOrganization};
use adm4_foundation::{Adm4Error, Adm4Result, read_json_file};
use std::collections::BTreeMap;
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
///
/// 不带模块目录：pack 声明了 system_refs 但没给模块目录 → 引用的模块必然未加载
/// → `instantiate_system_refs` 按 not_found 报错（fail-closed，不静默装配残缺空间）；
/// 无 system_refs 的旧包行为与扩展前逐字节一致。
pub fn load_design_space(root: &DesignSpaceRoot, pack_id: &str) -> Adm4Result<DesignSpace> {
    load_design_space_with_modules(root, pack_id, &[])
}

/// 加载「通用层 + 品类包 + 系统模块实例化」（W7 3a，定稿 §2.5）。
///
/// `module_dirs`：系统模块库目录集合（每目录按 `<module_id>/module.json` 布局）。
/// pack 的 system_refs 实例化产物（重写决策点 + tier 合成点 + 基数/一致性规则）
/// 并进 pack **内存态**（不落盘），genre_scope 重写为 Pack(pack_id) 过既有校验。
pub fn load_design_space_with_modules(
    root: &DesignSpaceRoot,
    pack_id: &str,
    module_dirs: &[PathBuf],
) -> Adm4Result<DesignSpace> {
    load_design_space_customized(root, pack_id, module_dirs, &BTreeMap::new(), &[])
}

/// 加载「通用层 + 品类包 + 库内模块 + **项目私有模块与引用**」（W7 §8 可拓宽通道，
/// 3a 系统级 custom module 的装配入口）。
///
/// `extra_modules`：项目私有 SystemModule 表（门面层从项目存档读出，已过结构自校验）；
/// 与库内模块 id 冲突即 Err——模块 id 是全局命名空间，私有模块遮蔽库模块会让
/// 同一 pack 在不同项目里装出不同语义（静默换语义，R2 禁止）。
/// `extra_refs`：项目私有系统实例引用，追加在 pack.system_refs 之后一起实例化
/// （instance_id 冲突由 `instantiate_system_refs` 统一拦截）。
pub fn load_design_space_customized(
    root: &DesignSpaceRoot,
    pack_id: &str,
    module_dirs: &[PathBuf],
    extra_modules: &BTreeMap<String, SystemModule>,
    extra_refs: &[SystemRef],
) -> Adm4Result<DesignSpace> {
    let universal = load_universal(root)?;
    let pack_path = root.pack_dir(pack_id).join("pack.json");
    if !pack_path.is_file() {
        return Err(Adm4Error::not_found(format!(
            "品类包 {pack_id} 不存在（未找到 {}，可用 space validate 查看可用包）",
            pack_path.display()
        )));
    }
    let mut pack = load_pack_file(&pack_path)?;
    if pack.pack_id != pack_id {
        return Err(Adm4Error::validation(format!(
            "pack.json 声明的 pack_id={} 与目录名 {pack_id} 不一致",
            pack.pack_id
        )));
    }
    // 模块库只在真的需要时才读：无 system_refs 的旧包加载路径完全不碰模块目录，
    // 库里某个坏 module.json 不能把不相关的包一并拖垮（fail-closed 只落在引用方）。
    let needs_modules =
        !pack.system_refs.is_empty() || !extra_refs.is_empty() || !extra_modules.is_empty();
    let mut modules = if needs_modules {
        load_modules_from_dirs(module_dirs)?
    } else {
        BTreeMap::new()
    };
    for (module_id, module) in extra_modules {
        if modules.contains_key(module_id) {
            return Err(Adm4Error::validation(format!(
                "项目私有模块 {module_id} 与系统模块库内的同名模块冲突\
                 （模块 id 是全局命名空间，私有遮蔽会静默换语义）"
            )));
        }
        modules.insert(module_id.clone(), module.clone());
    }
    pack.system_refs.extend(extra_refs.iter().cloned());
    assemble_design_space(universal, pack, &modules)
}

/// 装配纯函数：通用层 + pack + 已加载模块表 → 校验通过的设计空间。
/// fs 无关，测试可自建三者直接喂（模块实例化的单元测试走这里，不碰磁盘）。
pub fn assemble_design_space(
    universal: UniversalLayer,
    mut pack: GenrePack,
    modules: &std::collections::BTreeMap<String, adm4_decision::system_module::SystemModule>,
) -> Adm4Result<DesignSpace> {
    // 系统模块实例化（无 system_refs 时产物为空，行为与扩展前一致）。
    let instantiation = instantiate_system_refs(&pack, modules)?;
    pack.decision_points
        .extend(instantiation.decision_points.clone());
    pack.cardinality_expectations
        .extend(instantiation.cardinality_expectations.clone());
    pack.consistency_rules
        .extend(instantiation.consistency_rules.clone());

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
        system_instances: instantiation.instances,
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
