use crate::evidence::{EvidenceCandidate, EvidenceQuery, EvidenceSearchChannel};
use adm4_foundation::{Adm4Error, Adm4Result, ensure_within_root, read_json_file};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 本地语料通道（设计决定 D5）：S1 检索档从目录读取抓取快照做关键词检索，**零网络访问**。
///
/// 快照约定：`<root>/<game_name>/*.json`，每个文件是 `EvidenceCandidate` 的 JSON 数组：
///
/// ```json
/// [{
///   "source_url": "https://wiki.example/combat",
///   "title": "Combat overview",
///   "snippet": "克制伤害倍率……",
///   "source_type": "wiki",
///   "fetched_hash": "sha256:…"
/// }]
/// ```
///
/// `source_type` 取 `official|wiki|datamine|inference`。检索规则：按
/// `EvidenceQuery::keywords` 对 title/snippet 做大小写不敏感的包含匹配，命中任一关键词
/// 即入选；同一 `source_url` 去重，结果按 `source_url` 排序保证确定性。
/// 游戏无快照目录 = 无证据（宁缺勿造，缺口由 coverage 如实呈现）。
pub struct FileCorpusChannel {
    root: PathBuf,
}

impl FileCorpusChannel {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl EvidenceSearchChannel for FileCorpusChannel {
    fn channel_id(&self) -> &str {
        "file_corpus"
    }

    fn search(&self, query: &EvidenceQuery) -> Adm4Result<Vec<EvidenceCandidate>> {
        let game_name = query.game_name.trim();
        if game_name.is_empty() {
            return Err(Adm4Error::invalid_input("语料检索必须指定游戏名"));
        }
        let keywords: Vec<String> = query
            .keywords
            .iter()
            .map(|keyword| keyword.trim().to_lowercase())
            .filter(|keyword| !keyword.is_empty())
            .collect();
        if keywords.is_empty() {
            return Err(Adm4Error::invalid_input(
                "语料检索至少需要一个非空关键词（无关键词无法界定检索范围）",
            ));
        }
        // 游戏名用作子目录名，须防越界（如包含 `..`）。
        let game_dir = self.root.join(ensure_within_root(Path::new(game_name))?);
        if !game_dir.is_dir() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&game_dir).map_err(|error| {
            Adm4Error::io(format!("读取语料目录 {} 失败：{error}", game_dir.display()))
        })?;
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        files.sort();

        let mut seen_urls = BTreeSet::new();
        let mut results = Vec::new();
        for file in files {
            let candidates: Vec<EvidenceCandidate> = read_json_file(&file)?;
            for candidate in candidates {
                if candidate.source_url.trim().is_empty() {
                    return Err(Adm4Error::validation(format!(
                        "语料快照 {} 存在缺 source_url 的候选（宁缺勿造：无源候选不得入库）",
                        file.display()
                    )));
                }
                let haystack = format!("{}\n{}", candidate.title, candidate.snippet).to_lowercase();
                if keywords.iter().any(|keyword| haystack.contains(keyword))
                    && seen_urls.insert(candidate.source_url.clone())
                {
                    results.push(candidate);
                }
            }
        }
        results.sort_by(|left, right| left.source_url.cmp(&right.source_url));
        Ok(results)
    }
}
