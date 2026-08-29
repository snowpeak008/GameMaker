use crate::model::{CertificationStatus, Template};
use adm4_foundation::{Adm4Error, Adm4Result, read_json_file, write_json_file};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 模板库：`knowledge/design_space/<pack>/references/*.json`。
#[derive(Debug, Clone)]
pub struct TemplateLibrary {
    pub root: PathBuf,
}

impl TemplateLibrary {
    pub fn new(design_space_root: impl Into<PathBuf>) -> Self {
        Self {
            root: design_space_root.into(),
        }
    }

    fn references_dir(&self, pack_id: &str) -> PathBuf {
        self.root.join(pack_id).join("references")
    }

    fn template_path(&self, pack_id: &str, template_id: &str) -> PathBuf {
        self.references_dir(pack_id)
            .join(format!("{template_id}.json"))
    }

    pub fn list(&self, pack_id: &str) -> Adm4Result<Vec<Template>> {
        let dir = self.references_dir(pack_id);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut templates = Vec::new();
        let entries = fs::read_dir(&dir)
            .map_err(|error| Adm4Error::io(format!("read {} failed: {error}", dir.display())))?;
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        files.sort();
        for file in files {
            templates.push(read_json_file::<Template>(&file)?);
        }
        Ok(templates)
    }

    pub fn get(&self, pack_id: &str, template_id: &str) -> Adm4Result<Template> {
        read_json_file(&self.template_path(pack_id, template_id))
    }

    /// 保存草稿或审核中的答卷。
    pub fn save_draft(&self, template: &Template) -> Adm4Result<()> {
        write_json_file(
            &self.template_path(&template.genre_pack, &template.template_id),
            template,
        )
    }

    /// S4 人工审核：`CrossChecked→HumanReviewed`，落评审证明（R3）并保存答卷。
    pub fn human_review(
        &self,
        template: &mut Template,
        reviewer: &str,
        note: &str,
    ) -> Adm4Result<()> {
        template.certification.record_human_review(reviewer, note)?;
        self.save_draft(template)
    }

    /// S5 认证入库：`HumanReviewed→Certified`（跳级/回退由状态机拒绝），
    /// 自动把 game_name + aliases 登记进换皮词表（R5），再落盘模板。
    /// 先写词表后写模板：宁可多登记（多扫描是安全方向），不可漏登记。
    pub fn certify(&self, template: &mut Template, wordlist_path: &Path) -> Adm4Result<()> {
        template
            .certification
            .advance_to(CertificationStatus::Certified)?;
        let mut wordlist = load_skin_wordlist(wordlist_path)?;
        for word in template.skin_words() {
            let normalized = word.trim().to_string();
            if !normalized.is_empty() && !wordlist.words.contains(&normalized) {
                wordlist.words.push(normalized);
            }
        }
        wordlist.words.sort();
        save_skin_wordlist(wordlist_path, &wordlist)?;
        self.save_draft(template)
    }

    /// 模板取用的强制关卡：只有 Certified 模板可用于预填（决定 D8）。
    pub fn approved_for_prefill(&self, pack_id: &str, template_id: &str) -> Adm4Result<Template> {
        let template = self.get(pack_id, template_id)?;
        if !template.is_certified() {
            return Err(Adm4Error::blocked(format!(
                "模板 {template_id} 未完成认证（当前状态 {:?}），只有 Certified 模板可用于预填",
                template.certification.status
            )));
        }
        Ok(template)
    }
}

/// 全局换皮词表（`knowledge/design_space/skin_wordlist.json`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SkinWordlist {
    pub words: Vec<String>,
}

pub fn load_skin_wordlist(path: &Path) -> Adm4Result<SkinWordlist> {
    if !path.is_file() {
        return Ok(SkinWordlist::default());
    }
    read_json_file(path)
}

pub fn save_skin_wordlist(path: &Path, wordlist: &SkinWordlist) -> Adm4Result<()> {
    write_json_file(path, wordlist)
}
