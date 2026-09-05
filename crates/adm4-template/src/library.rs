use crate::model::{CertificationStatus, Template, TemplateOrigin, UNIVERSAL_GENRE_PACK};
use adm4_contracts::normalize_skin_word;
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

    /// 某项目可取用的全部模板：本包模板 + 通用层模板（`genre_pack=universal`）。
    ///
    /// `list` 严格按目录取，通用层模板会被过滤掉——那对逆向产线是对的（产线要写回本包目录），
    /// 但对「选一份模板预填/对照」是错的：通用层模板本就跨包可用，滤掉等于在 UI 里既不可选
    /// 也不可预填。展示与预填一律用本方法。
    ///
    /// 排序：先本包（`list` 的文件名序），再通用层；`pack_id` 本身就是通用层时不重复列出。
    pub fn list_available(&self, pack_id: &str) -> Adm4Result<Vec<Template>> {
        let mut templates = self.list(pack_id)?;
        if pack_id != UNIVERSAL_GENRE_PACK {
            templates.extend(self.list(UNIVERSAL_GENRE_PACK)?);
        }
        Ok(templates)
    }

    pub fn get(&self, pack_id: &str, template_id: &str) -> Adm4Result<Template> {
        read_json_file(&self.template_path(pack_id, template_id))
    }

    /// 取用解析：先在 `pack_id` 目录里找，找不到再落到通用层目录。
    ///
    /// 预填/对照的入口用它（而不是 `get`），这样调用方不必知道一份模板是本包的还是通用的；
    /// 逆向产线仍用 `get`——产线要按 `template.genre_pack` 写回原目录，不能落错地方。
    pub fn resolve(&self, pack_id: &str, template_id: &str) -> Adm4Result<Template> {
        match self.get(pack_id, template_id) {
            Ok(template) => Ok(template),
            Err(error) if pack_id != UNIVERSAL_GENRE_PACK => self
                .get(UNIVERSAL_GENRE_PACK, template_id)
                .map_err(|_| error),
            Err(error) => Err(error),
        }
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
    ///
    /// 两道关卡的顺序不能反：先状态机（跳级/回退 → `Blocked`），再证据链
    /// （逆向来源缺 S2/S3 机器证据 → `RedLine`）。反过来会把「Draft 直接认证」
    /// 误报成缺证据，掩盖真正的问题。状态推进落在克隆上，任一关不过则模板一字不改。
    pub fn certify(&self, template: &mut Template, wordlist_path: &Path) -> Adm4Result<()> {
        let mut certification = template.certification.clone();
        certification.advance_to(CertificationStatus::Certified)?;
        template.require_certification_evidence()?;
        template.certification = certification;
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

    /// 模板取用的强制关卡：Certified **且证据可核**才可用于预填（决定 D8）。
    /// 模板按 `resolve` 查找——本包找不到就落通用层（universal 模板跨包可用）。
    ///
    /// 为什么状态位不够：认证状态只是一个字段，而模板是磁盘上的 JSON。
    /// `certify` 上的证据关卡管不到「不走 `certify`、直接落盘一份 `status=certified`」
    /// 的模板——迁移工具就是这么写的 25 份内置模板，手工伪造同理。取用侧因此必须
    /// **自己再查一遍证据**（[`Template::require_certification_evidence`]，与 S5 同一份判定）：
    /// 有据可查的认证放行，手工塞入的伪认证被拒。
    pub fn approved_for_prefill(&self, pack_id: &str, template_id: &str) -> Adm4Result<Template> {
        let template = self.resolve(pack_id, template_id)?;
        if !template.is_certified() {
            return Err(Adm4Error::blocked(format!(
                "模板 {template_id} 未完成认证（当前状态 {:?}），只有 Certified 模板可用于预填",
                template.certification.status
            )));
        }
        template.require_certification_evidence()?;
        Ok(template)
    }

    /// 设计空间根下全部模板文件路径（每个带 `references/` 的品类包目录，含通用层）。
    ///
    /// 词表是全局单表，只看当前包会漏掉别的包登记的同名词条；而漏掉的方向是
    /// 「把外部游戏名误判成本项目自己的名字并豁免」，正是 R5 最不能出的错。
    fn all_template_files(&self) -> Adm4Result<Vec<PathBuf>> {
        if !self.root.is_dir() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.root).map_err(|error| {
            Adm4Error::io(format!("read {} failed: {error}", self.root.display()))
        })?;
        let mut packs: Vec<String> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.join("references").is_dir())
            .filter_map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
            })
            .collect();
        packs.sort();
        let mut files = Vec::new();
        for pack in &packs {
            let dir = self.references_dir(pack);
            let entries = fs::read_dir(&dir).map_err(|error| {
                Adm4Error::io(format!("read {} failed: {error}", dir.display()))
            })?;
            let mut pack_files: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
                .collect();
            pack_files.sort();
            files.extend(pack_files);
        }
        Ok(files)
    }

    /// 某换皮词条的全部登记出处（归一化整词相等，口径同 [`normalize_skin_word`]）。
    ///
    /// 为什么要能回答这个问题：词表只存词面，同一个词面可以既是「某项目自己的名字」
    /// 又是「某外部游戏的名字」。扫描侧要按当前项目豁免自身，就必须先分清这两者，
    /// 否则「项目取名恰好等于某个外部游戏名」时那个外部名会对该项目整体失效（R5 的缝）。
    ///
    /// 来源的权威记录是模板文件自己的 `origin`——不在词表里再抄一份，避免双真相
    /// （词表说 A 登记的、模板说 B 登记的时，谁也说不清该信哪个）。
    ///
    /// 不按认证状态过滤是刻意的：要判定的是「这个词面有没有可能指某个外部游戏」，
    /// 一份**尚在产线中**的逆向草稿同样回答「有可能」。方向取 fail-closed——宁可让项目
    /// 被自己的名字拦下（人工可见、可改名或改模板），也不放过「抄一个同名外部游戏」。
    pub fn skin_word_registrations(&self, word: &str) -> Adm4Result<Vec<SkinWordRegistration>> {
        let needle = normalize_skin_word(word);
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let mut registrations = Vec::new();
        for path in self.all_template_files()? {
            let header: TemplateSkinHeader = read_json_file(&path)?;
            let registers = std::iter::once(&header.game_name)
                .chain(header.aliases.iter())
                .any(|candidate| normalize_skin_word(candidate) == needle);
            if registers {
                registrations.push(SkinWordRegistration {
                    genre_pack: header.genre_pack,
                    template_id: header.template_id,
                    origin: header.origin,
                });
            }
        }
        Ok(registrations)
    }
}

/// 一条换皮词条的登记出处：哪份模板、什么来源把它登记进词表的。
#[derive(Debug, Clone, PartialEq)]
pub struct SkinWordRegistration {
    /// 登记该词的模板所属品类包。
    pub genre_pack: String,
    pub template_id: String,
    /// 登记来源；`ProjectExport` 带源存档 id，据此可判断「是不是本项目自己的名字」。
    pub origin: TemplateOrigin,
}

impl SkinWordRegistration {
    /// 该登记是否来自 `archive_id` 这个存档的「另存模板」。
    ///
    /// 只有这一种登记才可能是「项目自己的名字」；逆向来源与批量迁移来源登记的词条
    /// 一律是外部游戏名，即使字面与当前项目名逐字相同也不得豁免。
    pub fn is_export_of(&self, archive_id: &str) -> bool {
        matches!(
            &self.origin,
            TemplateOrigin::ProjectExport {
                source_archive_id,
                ..
            } if source_archive_id == archive_id
        )
    }
}

/// 溯源用的模板头部：只反序列化词表相关字段。
///
/// 不直接用 [`Template`] 是性能考虑：内置模板库 25 份答卷合计十几 MB，而溯源只需要
/// `game_name`/`aliases`/`origin`。serde 默认忽略未声明字段，因此 `answers` 只被解析器
/// 跳过、不被建成结构。
#[derive(Debug, Clone, Deserialize)]
struct TemplateSkinHeader {
    template_id: String,
    game_name: String,
    #[serde(default)]
    aliases: Vec<String>,
    genre_pack: String,
    /// 旧档缺 `origin` 键 → 逆向来源（与 [`Template`] 的默认值一致，最严的分支）。
    #[serde(default)]
    origin: TemplateOrigin,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Certification;
    use adm4_decision::DesignLevel;

    /// 唯一临时目录（进程 id + 用例标签，同二进制内多用例互不踩踏）。
    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("adm4_tpl_library_{}_{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn template(
        pack: &str,
        id: &str,
        game: &str,
        aliases: &[&str],
        origin: TemplateOrigin,
    ) -> Template {
        Template {
            template_id: id.into(),
            game_name: game.into(),
            aliases: aliases.iter().map(|alias| (*alias).to_string()).collect(),
            genre_pack: pack.into(),
            pack_version: "0.1.0".into(),
            depth_reached: DesignLevel::L4,
            answers: Vec::new(),
            certification: Certification::default(),
            origin,
            mapping_hash: String::new(),
            crosscheck_proof: None,
            smoke_test: false,
        }
    }

    fn project_export(archive_id: &str) -> TemplateOrigin {
        TemplateOrigin::ProjectExport {
            source_archive_id: archive_id.into(),
            source_project_name: "霜落峡谷".into(),
            exported_at: "2026-08-31T00:00:00Z".into(),
        }
    }

    /// 词条溯源：跨包遍历、区分来源、归一化整词相等、别名同样算登记。
    ///
    /// 这是「豁免只认本存档的另存模板」的判定依据，因此三件事必须同时成立：
    /// ① 本存档导出登记的词能被认出来；② 逆向来源登记的**同一个词面**照旧被认成外部来源；
    /// ③ 别的包里的模板也在溯源范围内（词表是全局单表，漏一个包就等于误豁免）。
    #[test]
    fn skin_word_registrations_distinguish_origin_across_packs() {
        let root = scratch("registrations");
        let library = TemplateLibrary::new(&root);
        // 根目录还不存在 → 空结果而不是报错（首次使用的设计空间没有 references 目录）。
        assert!(
            library
                .skin_word_registrations("霜落峡谷")
                .unwrap()
                .is_empty()
        );

        library
            .save_draft(&template(
                "lane_defense",
                "tpl_export",
                "霜落峡谷",
                &["霜落定稿"],
                project_export("arc-1"),
            ))
            .unwrap();
        library
            .save_draft(&template(
                "grid_strategy",
                "tpl_rival",
                "霜落峡谷",
                &[],
                TemplateOrigin::Reverse,
            ))
            .unwrap();
        library
            .save_draft(&template(
                "universal",
                "tpl_other",
                "晨昏防线",
                &[],
                TemplateOrigin::Reverse,
            ))
            .unwrap();

        let hits = library.skin_word_registrations("  霜落峡谷  ").unwrap();
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!(hits.iter().any(|hit| hit.template_id == "tpl_export"
            && hit.genre_pack == "lane_defense"
            && hit.is_export_of("arc-1")));
        assert!(
            hits.iter()
                .any(|hit| hit.template_id == "tpl_rival" && !hit.is_export_of("arc-1")),
            "别的包里的同名逆向模板必须一起被认出来：{hits:?}"
        );
        // 别的存档导出的同名模板同样不算「本存档自己的名字」。
        assert!(hits.iter().all(|hit| !hit.is_export_of("arc-2")));

        // 别名照样算登记（认证时 game_name + aliases 一并进词表）。
        let by_alias = library.skin_word_registrations("霜落定稿").unwrap();
        assert_eq!(by_alias.len(), 1);
        assert_eq!(by_alias[0].template_id, "tpl_export");

        // 子串不算登记（整词相等口径，与扫描器一致）。
        assert!(library.skin_word_registrations("霜落").unwrap().is_empty());
        // 空词与未登记词都返回空表。
        assert!(library.skin_word_registrations("   ").unwrap().is_empty());
        assert!(
            library
                .skin_word_registrations("从未登记过")
                .unwrap()
                .is_empty()
        );

        fs::remove_dir_all(&root).ok();
    }

    /// 缺 `origin` 键的旧档/伪造档在溯源时按逆向来源解读（fail-closed：不予豁免）。
    #[test]
    fn legacy_template_without_origin_is_traced_as_reverse() {
        let root = scratch("legacy_origin");
        let dir = root.join("lane_defense").join("references");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("tpl_legacy.json"),
            r#"{
              "template_id": "tpl_legacy",
              "game_name": "晨昏防线",
              "genre_pack": "lane_defense",
              "pack_version": "0.1.0",
              "depth_reached": "L4",
              "answers": []
            }"#,
        )
        .unwrap();
        let hits = TemplateLibrary::new(&root)
            .skin_word_registrations("晨昏防线")
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].origin, TemplateOrigin::Reverse);
        assert!(!hits[0].is_export_of("arc-1"));
        fs::remove_dir_all(&root).ok();
    }
}
