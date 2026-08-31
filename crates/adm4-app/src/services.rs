use crate::change::{ChangeLog, ChangeRequest, ChangeStatus};
use crate::config::{AppConfig, load_config, load_named_secrets, save_config, save_named_secret};
use crate::deliverable::DeliverableManifest;
use crate::pipeline_artifact::StageArtifactView;
use crate::runlog::RunLog;
use crate::sdk::{SdkKnowledgeBase, SdkSnapshot};
use adm4_ai::{
    AiProvider, AiRequest, HttpImageProviderConfig, HttpProviderConfig, ImageProvider,
    OpenAiCompatibleImageProvider, OpenAiCompatibleProvider, SecretRef,
};
use adm4_archive::{
    ArchiveLock, ArchiveManifest, ArchiveStore, DataRoot, export_package, import_package,
};
use adm4_authoring::{
    AuthoringEngine, AuthoringState, FreezeGateReport, FrozenDesign, GateFinding,
    InterviewProgress, InterviewProposal, InterviewService, InterviewTurn, PrefillReport,
    WorkbenchResetReport, evaluate_freeze_gates, execute_freeze, run_red_team,
};
use adm4_build::art::style_anchor::{
    STYLE_SECTION, StyleAnchorSet, StyleAnchorStore, StyleApplicationContract, StyleFitReport,
    StyleGate, StyleGateStatus, StyleGenerationOptions, StyleLockOutcome, StyleReadiness,
    StyleSession, StyleSourceFact, StyleSourceFacts,
};
use adm4_build::{
    ArtifactKind, BuildContext, PendingStage, Phase2Runner, pending_stage, phase2_artifacts,
    phase2_execution_order,
};
use adm4_contracts::{SkinScanner, normalize_skin_word};
use adm4_decision::{
    DecisionPoint, DepthProfile, DesignLevel, DomainProgress, NodeProgress, OrganizationProgress,
    ParameterValues, PointApplicability, PointRequirement, ProgressCounts, SelectionMode,
    check_row_references, counts_toward_completeness,
};
use adm4_foundation::{
    Adm4Error, Adm4Result, UtcTimestamp, ensure_dir, ensure_within_root, new_id, read_json_file,
    write_json_file,
};
use adm4_pipeline::{
    ArtifactStore, CancelSignal, PipelineRerunOutcome, PipelineRunOutcome, PipelineRunState,
    PipelineRunner, RunnerContext, phase2_registry,
};
use adm4_space::{DesignSpace, DesignSpaceRoot, load_design_space};
use adm4_spec::GameSpec;
use adm4_template::{
    Certification, CrossCheckReport, CrossCheckService, EvidenceCandidate, EvidenceQuery,
    EvidenceSearchChannel, FileCorpusChannel, MappingService, Template, TemplateAnswer,
    TemplateLibrary, TemplateOrigin, TemplateSelectedOption, load_skin_wordlist,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// 应用服务门面：GUI/CLI 的唯一入口。
pub struct AppServices {
    pub data_root: DataRoot,
    pub archives: ArchiveStore,
    pub log: RunLog,
    /// 运行期可热更新的配置（内部 `RwLock`）。
    ///
    /// 为什么要热更新：桌面端允许运行期改 AI 配置，而 `open` 时的快照改不了，
    /// 用户会看到「已保存」却仍旧 blocked——那正是 R7 禁止的「显示成功而实际没生效」。
    /// F4c 的绕过办法是每次 AI 动作重开一份门面（顺带丢掉设计空间缓存），
    /// 这里给出正规通道：[`AppServices::reload_config`] / [`AppServices::set_ai_provider`]。
    ///
    /// 为什么选 `RwLock` 而不是「每次从磁盘现读」：现读会让每次 AI 动作都多一次文件 IO
    /// 与一次 JSON 解析，且「当前生效配置」变成没有单一真相（两次现读之间可能不同，
    /// 一次运行里前后不一致却查不出来）。锁内一份快照 + 显式 reload，语义清楚可测。
    config: RwLock<AppConfig>,
    /// 设计空间根：**进程期不变量**，不参与热更新。
    ///
    /// 它是 `space_root` / `templates` / `space_cache` 三者的锚，运行期换掉等于把
    /// 已打开的项目悄悄指到另一份清单上（缓存里还留着旧空间）。因此 `reload_config`
    /// 遇到它被改动会显式报错要求重启，而不是装作生效。
    design_space_root: String,
    space_root: DesignSpaceRoot,
    templates: TemplateLibrary,
    /// 已装配设计空间的进程内缓存，键 = `pack_id`。
    ///
    /// 迁移后通用层清单达 4.7MB，一次 `load_design_space` 的解析 + 校验在 100-150ms 量级，
    /// 而工作台的一次交互要装载 4 次（`decision_points` / `project_profile` /
    /// `workbench_overview` / `open_engine`）。设计空间运行期只读（唯一写入者是离线迁移
    /// 工具与手改清单，都要求重启），所以缓存**没有失效问题**：进程存活期内一装到底。
    ///
    /// 用 `Mutex` 而不是 `RefCell`：`AppServices` 现状是 `Send + Sync`（桌面端 `Rc` 单线程用，
    /// 测试里直接用），换成 `RefCell` 会静默摘掉 `Sync`；`Mutex` 保住原有约束又足够简单。
    /// 缓存 `Arc<DesignSpace>` 让「命中 → 克隆出所有权」只付一次结构克隆，不再付解析与校验。
    space_cache: Mutex<BTreeMap<String, Arc<DesignSpace>>>,
}

impl AppServices {
    pub fn open(data_root_path: Option<PathBuf>) -> Adm4Result<Self> {
        let data_root = match data_root_path {
            Some(path) => DataRoot::new(path)?,
            None => DataRoot::default_at_cwd()?,
        };
        let config = load_config(&data_root)?;
        let design_space_root = config.design_space_root.clone();
        let space_root = DesignSpaceRoot::new(&design_space_root);
        let templates = TemplateLibrary::new(&design_space_root);
        let log = RunLog::new(&data_root);
        Ok(Self {
            archives: ArchiveStore::new(data_root.clone()),
            data_root,
            config: RwLock::new(config),
            log,
            design_space_root,
            space_root,
            templates,
            space_cache: Mutex::new(BTreeMap::new()),
        })
    }

    // ------------------------------------------------------------------
    // 配置（运行期热更新）
    // ------------------------------------------------------------------

    /// 当前生效配置的快照（克隆，调用方随便持有多久都不会看到半改状态）。
    pub fn config(&self) -> Adm4Result<AppConfig> {
        Ok(self.read_config()?.clone())
    }

    /// 设计空间根（进程期不变量，无需加锁）。
    pub fn design_space_root(&self) -> &str {
        &self.design_space_root
    }

    /// 从磁盘重读 `config/app.json` 并替换当前生效配置。
    ///
    /// 用于「用户在别处（手改文件 / 另一个进程）改了配置，界面点刷新」。
    /// `design_space_root` 被改动时**显式报错**而不静默沿用旧值：那是进程期不变量，
    /// 装作生效会让人以为切换成功（而缓存里还是旧空间），需要重启才能生效。
    pub fn reload_config(&self) -> Adm4Result<AppConfig> {
        let fresh = load_config(&self.data_root)?;
        if fresh.design_space_root != self.design_space_root {
            return Err(Adm4Error::blocked(format!(
                "config/app.json 的 design_space_root 已从 {} 改为 {}：\
                 设计空间根是进程期不变量（决策图与模板库缓存都锚在它上面），请重启应用后生效",
                self.design_space_root, fresh.design_space_root
            )));
        }
        let mut guard = self.write_config()?;
        *guard = fresh.clone();
        Ok(fresh)
    }

    /// 设置（或以 `None` 清空）激活的 AI Provider：落盘 `config/app.json` **并**更新内存配置。
    ///
    /// 桌面端与 CLI 一律走这里，不再各自 `load_config` → 改 → `save_config`：
    /// 那样磁盘变了而门面里的快照没变，紧接着的 AI 动作仍按旧配置跑。
    pub fn set_ai_provider(&self, provider: Option<HttpProviderConfig>) -> Adm4Result<()> {
        let mut guard = self.write_config()?;
        let mut updated = guard.clone();
        updated.ai_provider = provider;
        save_config(&self.data_root, &updated)?;
        let message = match &updated.ai_provider {
            Some(config) => format!(
                "AI Provider 配置更新：{}（模型 {}，密钥引用 {}）",
                config.provider_id, config.model, config.api_key_ref
            ),
            None => "AI Provider 配置已清空：AI 相关功能将显式 blocked（无兜底）".to_string(),
        };
        *guard = updated;
        drop(guard);
        self.log.append("ai", &message)
    }

    fn read_config(&self) -> Adm4Result<RwLockReadGuard<'_, AppConfig>> {
        self.config
            .read()
            .map_err(|error| Adm4Error::internal(format!("配置锁被污染：{error}")))
    }

    fn write_config(&self) -> Adm4Result<RwLockWriteGuard<'_, AppConfig>> {
        self.config
            .write()
            .map_err(|error| Adm4Error::internal(format!("配置锁被污染：{error}")))
    }

    // ------------------------------------------------------------------
    // 设计空间
    // ------------------------------------------------------------------

    pub fn list_packs(&self) -> Adm4Result<Vec<String>> {
        self.space_root.list_packs()
    }

    /// 装载设计空间（进程内缓存；语义与直接 `load_design_space` 完全等价）。
    pub fn load_space(&self, pack_id: &str) -> Adm4Result<DesignSpace> {
        Ok((*self.load_space_shared(pack_id)?).clone())
    }

    /// 装载设计空间并共享所有权：只读用途（不需要 `DesignSpace` 所有权）走这里，连结构克隆都免了。
    pub fn load_space_shared(&self, pack_id: &str) -> Adm4Result<Arc<DesignSpace>> {
        {
            let cache = self.lock_space_cache()?;
            if let Some(space) = cache.get(pack_id) {
                return Ok(Arc::clone(space));
            }
        }
        // 校验失败照旧上抛（fail-closed）：只有装配成功的空间进缓存，不缓存错误。
        let space = Arc::new(load_design_space(&self.space_root, pack_id)?);
        let mut cache = self.lock_space_cache()?;
        Ok(Arc::clone(
            cache.entry(pack_id.to_string()).or_insert(space),
        ))
    }

    fn lock_space_cache(&self) -> Adm4Result<MutexGuard<'_, BTreeMap<String, Arc<DesignSpace>>>> {
        self.space_cache.lock().map_err(|error| {
            Adm4Error::internal(format!("设计空间缓存锁被污染（前一次持锁 panic）：{error}"))
        })
    }

    pub fn templates(&self) -> &TemplateLibrary {
        &self.templates
    }

    /// 换皮扫描词表 + **当前项目自身名的豁免**（R5）。
    ///
    /// 词表是全局的：任何模板认证入库都会登记它的 `game_name`/`aliases`，其中包括
    /// 「本项目导出」的模板——那登记的是某个项目自己的名字。对别的项目而言必须拦
    /// （B 的产物出现 A 的名字 = 抄 A），对 A 自己必须放行（C0 文档标题就是项目名）。
    ///
    /// # 豁免作用域（F4e 收窄后的完整口径）
    ///
    /// 一个词被豁免，必须**同时**满足：
    /// 1. 归一化（trim + 小写）后与 `project_name` **整词相等**；
    /// 2. 它在全库模板中的登记来源**有且只有** `TemplateOrigin::ProjectExport`
    ///    且 `source_archive_id == archive_id`——也就是「本存档自己导出的模板」。
    ///
    /// 因此下列情形一律**不豁免**：词条另有逆向（`Reverse`）或批量迁移（`BulkMigration`）
    /// 来源登记（即使字面与项目名逐字相同）；词条来自品类包 `reference_games`；词条只由
    /// **别的**存档导出的模板登记；词条压根不在词表里（无可豁免）。
    ///
    /// F4d 的旧口径是「按词面无条件剔除当前项目名」，留下一条缝：项目取名恰好等于某个
    /// 逆向外部游戏名（如项目就叫「Kingdom Rush」）时，那个外部名对该项目整体失效。
    /// 收窄后这条缝关闭，代价是同名项目会被自己的名字拦下——那是可见、可改名的
    /// fail-closed 方向，而误豁免是静默放过换皮。
    ///
    /// 别名同样不豁免（别名是作者另起的词，出现在自己产物里说明词表登记过宽，应当被看见），
    /// 也不做前缀/子串豁免。
    pub fn skin_scanner_for_project(
        &self,
        space: &DesignSpace,
        archive_id: &str,
        project_name: &str,
    ) -> Adm4Result<SkinScanner> {
        let words = self.skin_words(space)?;
        let exemptions =
            self.skin_exemptions_for_project(space, archive_id, project_name, &words)?;
        Ok(SkinScanner::with_exemptions(words, exemptions))
    }

    /// 豁免集合的唯一判定点（口径见 [`Self::skin_scanner_for_project`]）。
    ///
    /// 先做「项目名压根不在生效词表里」的快路径：这是绝大多数项目的常态（没导出过模板），
    /// 此时无词可豁免，也就不必去翻模板库——溯源要读全部模板文件，内置库有十几 MB。
    fn skin_exemptions_for_project(
        &self,
        space: &DesignSpace,
        archive_id: &str,
        project_name: &str,
        words: &[String],
    ) -> Adm4Result<Vec<String>> {
        let normalized = normalize_skin_word(project_name);
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        if !words
            .iter()
            .any(|word| normalize_skin_word(word) == normalized)
        {
            return Ok(Vec::new());
        }
        // 品类包 reference_games 登记的词是外部游戏名，项目叫什么都不豁免
        // （它们不来自模板库，溯源查不到登记，必须在这里单独拦住）。
        if space
            .skin_words()
            .iter()
            .any(|word| normalize_skin_word(word) == normalized)
        {
            return Ok(Vec::new());
        }
        let registrations = self.templates.skin_word_registrations(&normalized)?;
        // 空登记 = 词在表里但查不到出处（登记它的模板已被删）→ 不豁免（fail-closed）。
        if !registrations.is_empty()
            && registrations
                .iter()
                .all(|registration| registration.is_export_of(archive_id))
        {
            return Ok(vec![project_name.to_string()]);
        }
        Ok(Vec::new())
    }

    /// 生效换皮词表（全局词表文件 + 品类包参考游戏名），未做任何豁免。
    ///
    /// 只用于**查看**词表（体检/调试）。没有对应的「无豁免扫描器」构造入口是故意的：
    /// 每个扫描点都必须说清自己在扫谁的产物，否则项目会被自己的名字拦住（F4d 之前的坑）。
    pub fn skin_words(&self, space: &DesignSpace) -> Adm4Result<Vec<String>> {
        let mut words = load_skin_wordlist(&self.skin_wordlist_path())?.words;
        words.extend(space.skin_words());
        Ok(words)
    }

    pub fn skin_wordlist_path(&self) -> PathBuf {
        Path::new(&self.design_space_root).join("skin_wordlist.json")
    }

    // ------------------------------------------------------------------
    // 项目生命周期
    // ------------------------------------------------------------------

    /// 创建项目：草稿 → 初始 authoring_state → 原子提交为正式存档。
    pub fn project_new(
        &self,
        project_name: &str,
        pack_id: &str,
        depth: DesignLevel,
        template_id: Option<&str>,
    ) -> Adm4Result<String> {
        let space = self.load_space_shared(pack_id)?;
        let depth_profile = DepthProfile::new(depth).map_err(Adm4Error::invalid_input)?;
        let mut state = AuthoringState::new(
            project_name,
            pack_id,
            space.pack.pack_version.clone(),
            depth_profile,
        );
        if let Some(template_id) = template_id {
            let template = self.templates.approved_for_prefill(pack_id, template_id)?;
            let mut engine = AuthoringEngine::new(space, state)?;
            let report = engine.prefill_from_template(&template)?;
            self.log.append(
                "project",
                &format!(
                    "模板 {template_id}（包 {}）预填：{}（需换皮与逐条确认）",
                    template.genre_pack,
                    report.summary()
                ),
            )?;
            for skip in &report.skipped {
                self.log.append(
                    "project",
                    &format!(
                        "模板 {template_id} 跳过 {}/{}：{}",
                        skip.decision_id, skip.option_id, skip.reason
                    ),
                )?;
            }
            state = engine.into_state();
        }
        let session_id = new_id("session");
        self.archives.create_draft(&session_id, None)?;
        let content = self.archives.draft_content_dir(&session_id);
        write_json_file(&content.join("authoring_state.json"), &state)?;
        let archive_id = self
            .archives
            .commit_draft(&session_id, project_name, None)?;
        self.log.append(
            "project",
            &format!("创建项目 {project_name} → {archive_id}"),
        )?;
        Ok(archive_id)
    }

    pub fn project_list(&self) -> Adm4Result<Vec<ArchiveManifest>> {
        self.archives.list_archives()
    }

    /// 存档体检（只诊断不修复）：manifest 可读、内容指纹与实际内容一致。
    ///
    /// 逻辑本来只长在 CLI 里，桌面端因此没有体检入口。上提到门面后两端共用同一份判定，
    /// 呈现层只负责把 `problems` 逐条画出来、按 `healthy` 决定退出码/提示色。
    /// 存档不存在时 manifest 不可读 → 也是一条 problem（而不是抛错），
    /// 与「体检要报告问题、不要自己失败」的语义一致。
    pub fn project_doctor(&self, archive_id: &str) -> Adm4Result<ProjectDoctorReport> {
        let problems = self.archives.doctor(archive_id)?;
        Ok(ProjectDoctorReport {
            archive_id: archive_id.to_string(),
            healthy: problems.is_empty(),
            problems,
        })
    }

    pub fn load_authoring_state(&self, archive_id: &str) -> Adm4Result<AuthoringState> {
        let content = self.archives.content_dir(archive_id);
        if !content.is_dir() {
            return Err(Adm4Error::not_found(format!(
                "存档 {archive_id} 不存在（可用 project list 查看现有项目）"
            )));
        }
        let mut state: AuthoringState = read_json_file(&content.join("authoring_state.json"))?;
        // 旧存档的豁免署名并行 map 就地合并（与 AuthoringEngine::new 同一处理，只读路径也生效）。
        state.adopt_legacy_na_signoffs();
        Ok(state)
    }

    /// 打开项目为创作引擎（加载状态 + 设计空间）。
    /// 设计空间取缓存里的共享句柄——引擎按 `Arc` 持有，因此这里连结构克隆都不发生。
    pub fn open_engine(&self, archive_id: &str) -> Adm4Result<AuthoringEngine> {
        let state = self.load_authoring_state(archive_id)?;
        let space = self.load_space_shared(&state.genre_pack)?;
        AuthoringEngine::new(space, state)
    }

    /// 修改并事务性保存项目（持锁 → 草稿 → 变更 → 原子提交）。
    pub fn with_project<T>(
        &self,
        archive_id: &str,
        operation: impl FnOnce(&mut AuthoringEngine) -> Adm4Result<T>,
    ) -> Adm4Result<T> {
        self.with_project_named(archive_id, None, operation)
    }

    /// `with_project` 的内部形态：`manifest_name` 非 None 时同时更新存档 manifest 的展示名。
    ///
    /// 只有重命名需要动 manifest；其余变更沿用原名（避免把 `import_project` 等场景下
    /// manifest 名与创作状态名不一致的历史数据在无关操作里被悄悄改写）。
    fn with_project_named<T>(
        &self,
        archive_id: &str,
        manifest_name: Option<&str>,
        operation: impl FnOnce(&mut AuthoringEngine) -> Adm4Result<T>,
    ) -> Adm4Result<T> {
        let manifest = self.archives.manifest(archive_id)?;
        let commit_name = manifest_name.unwrap_or(manifest.project_name.as_str());
        let session_id = new_id("session");
        let archive_dir = self.data_root.archive_dir(archive_id);
        let lock = ArchiveLock::acquire(&archive_dir, &session_id)?;
        let result = (|| {
            self.archives.create_draft(&session_id, Some(archive_id))?;
            let content = self.archives.draft_content_dir(&session_id);
            let state: AuthoringState = read_json_file(&content.join("authoring_state.json"))?;
            let space = self.load_space_shared(&state.genre_pack)?;
            let mut engine = AuthoringEngine::new(space, state)?;
            let value = operation(&mut engine)?;
            write_json_file(&content.join("authoring_state.json"), engine.state())?;
            self.archives
                .commit_draft(&session_id, commit_name, Some(archive_id))?;
            Ok(value)
        })();
        lock.release()?;
        result
    }

    /// 项目重命名（校验非空白）：创作状态与存档 manifest 的展示名一起改，并落运行日志。
    ///
    /// 名称校验（非空白 + trim）在引擎里做，manifest 用引擎 trim 后的结果，
    /// 因此 `project list` 与工作台摘要不会出现两个不同的项目名。
    pub fn project_rename(&self, archive_id: &str, project_name: &str) -> Adm4Result<()> {
        // 先规范化（空白名在开事务前就被拒），再一次事务同时写创作状态与 manifest 展示名。
        let renamed = AuthoringEngine::normalize_project_name(project_name)?;
        let previous = self.load_authoring_state(archive_id)?.project_name;
        self.with_project_named(archive_id, Some(&renamed), |engine| {
            engine.set_project_name(&renamed)
        })?;
        self.log.append(
            "project",
            &format!("项目 {archive_id} 重命名：{previous} → {renamed}"),
        )
    }

    /// 认证模板预填到已有项目：走取用关卡（`approved_for_prefill`，含 universal 跨包解析）
    /// + 引擎预填，跳过的答卷条目逐条进运行日志（R2：不静默丢弃）。
    pub fn project_prefill_template(
        &self,
        archive_id: &str,
        template_id: &str,
    ) -> Adm4Result<PrefillReport> {
        let state = self.load_authoring_state(archive_id)?;
        let template = self
            .templates
            .approved_for_prefill(&state.genre_pack, template_id)?;
        let report =
            self.with_project(archive_id, |engine| engine.prefill_from_template(&template))?;
        self.log.append(
            "project",
            &format!(
                "项目 {archive_id} 用模板 {template_id}（包 {}）预填：{}",
                template.genre_pack,
                report.summary()
            ),
        )?;
        for skip in &report.skipped {
            self.log.append(
                "project",
                &format!(
                    "模板 {template_id} 跳过 {}/{}：{}",
                    skip.decision_id, skip.option_id, skip.reason
                ),
            )?;
        }
        Ok(report)
    }

    /// 工作台重置：清空创作选择，保留项目与已冻结历史（二版 `reset-design-workbench`）。
    ///
    /// 清空/保留的确切范围见 [`AuthoringEngine::reset_workbench`]。走既有 `with_project`
    /// 事务（持锁 → 草稿 → 变更 → 原子提交），因此中途失败不会留下半清空的项目。
    /// `actor` 与 `note` 双必填（R3）；清空计数与署名逐条进运行日志。
    pub fn project_reset_workbench(
        &self,
        archive_id: &str,
        actor: &str,
        note: &str,
    ) -> Adm4Result<WorkbenchResetReport> {
        let report = self.with_project(archive_id, |engine| engine.reset_workbench(actor, note))?;
        self.log.append(
            "project",
            &format!(
                "项目 {archive_id} 工作台重置（署名 {}）：{}；已冻结版本与流水线产物保留",
                report.actor,
                report.summary()
            ),
        )?;
        self.log.append(
            "project",
            &format!("项目 {archive_id} 工作台重置理由：{}", note.trim()),
        )?;
        Ok(report)
    }

    pub fn export_project(&self, archive_id: &str, output: &Path) -> Adm4Result<usize> {
        export_package(&self.archives.content_dir(archive_id), output)
    }

    pub fn import_project(&self, package: &Path, project_name: &str) -> Adm4Result<String> {
        // 归一化名先于建档：空白名在导入前就被拒，manifest 与创作态用同一份规范名。
        let normalized = AuthoringEngine::normalize_project_name(project_name)?;
        let session_id = new_id("session");
        self.archives.create_draft(&session_id, None)?;
        let content = self.archives.draft_content_dir(&session_id);
        import_package(package, &content)?;
        let archive_id = self.archives.commit_draft(&session_id, &normalized, None)?;
        // 包内 authoring_state.json 带的是导出方项目名；这里把创作态项目名归一回写，
        // 消除「manifest 名 vs 创作态名」双真相（否则 workbench 摘要与 project list 会各说各话）。
        self.with_project_named(&archive_id, Some(&normalized), |engine| {
            engine.set_project_name(&normalized)
        })?;
        self.log
            .append("project", &format!("导入项目 {normalized} → {archive_id}"))?;
        Ok(archive_id)
    }

    // ------------------------------------------------------------------
    // 创作：多选点、人工豁免、节点文本（CLI 子命令由 T11/T12 统一补）
    // ------------------------------------------------------------------

    /// 多选点追加一个已选选项。
    pub fn authoring_add_option(
        &self,
        archive_id: &str,
        decision_id: &str,
        option_id: &str,
    ) -> Adm4Result<()> {
        self.with_project(archive_id, |engine| {
            engine.add_option(decision_id, option_id)
        })?;
        self.log.append(
            "authoring",
            &format!("项目 {archive_id} 多选点 {decision_id} 追加选项 {option_id}"),
        )
    }

    /// 多选点移除一个已选选项（整点撤销请用 `authoring_clear_selection` 语义的引擎方法）。
    pub fn authoring_remove_option(
        &self,
        archive_id: &str,
        decision_id: &str,
        option_id: &str,
    ) -> Adm4Result<()> {
        self.with_project(archive_id, |engine| {
            engine.remove_option(decision_id, option_id)
        })?;
        self.log.append(
            "authoring",
            &format!("项目 {archive_id} 多选点 {decision_id} 移除选项 {option_id}"),
        )
    }

    /// 标记多选点主选。
    pub fn authoring_set_primary_option(
        &self,
        archive_id: &str,
        decision_id: &str,
        option_id: &str,
    ) -> Adm4Result<()> {
        self.with_project(archive_id, |engine| {
            engine.set_primary_option(decision_id, option_id)
        })?;
        self.log.append(
            "authoring",
            &format!("项目 {archive_id} 决策点 {decision_id} 主选设为 {option_id}"),
        )
    }

    /// 为多选点的某个已选选项填参数；返回参数校验问题清单（非空进待填清单，不阻断保存）。
    pub fn authoring_set_option_parameters(
        &self,
        archive_id: &str,
        decision_id: &str,
        option_id: &str,
        parameters: ParameterValues,
    ) -> Adm4Result<Vec<String>> {
        self.with_project(archive_id, |engine| {
            engine.set_option_parameters(decision_id, option_id, parameters)
        })
    }

    /// 人工豁免：把决策点标记为不适用（理由码 + 说明 + 署名三者必填，R3）。
    pub fn authoring_set_not_applicable(
        &self,
        archive_id: &str,
        decision_id: &str,
        reason_code: &str,
        note: &str,
        actor: &str,
    ) -> Adm4Result<()> {
        self.with_project(archive_id, |engine| {
            engine.set_not_applicable(decision_id, reason_code, note, actor)
        })?;
        self.log.append(
            "authoring",
            &format!(
                "项目 {archive_id} 决策点 {decision_id} 人工豁免为不适用[{reason_code}]（署名 {actor}）"
            ),
        )
    }

    /// 解除不适用；返回 false 表示该点本来就不是 N/A（幂等）。
    pub fn authoring_clear_not_applicable(
        &self,
        archive_id: &str,
        decision_id: &str,
    ) -> Adm4Result<bool> {
        let cleared = self.with_project(archive_id, |engine| {
            engine.clear_not_applicable(decision_id)
        })?;
        if cleared {
            self.log.append(
                "authoring",
                &format!("项目 {archive_id} 决策点 {decision_id} 解除不适用"),
            )?;
        }
        Ok(cleared)
    }

    /// 节点级设计说明（空串 = 清除）。
    pub fn authoring_set_node_design_note(
        &self,
        archive_id: &str,
        node_id: &str,
        note: &str,
    ) -> Adm4Result<()> {
        self.with_project(archive_id, |engine| {
            engine.set_node_design_note(node_id, note)
        })
    }

    /// 节点级风险说明（空串 = 清除）。
    pub fn authoring_set_node_risk_note(
        &self,
        archive_id: &str,
        node_id: &str,
        note: &str,
    ) -> Adm4Result<()> {
        self.with_project(archive_id, |engine| {
            engine.set_node_risk_note(node_id, note)
        })
    }

    // ------------------------------------------------------------------
    // 工作台只读聚合查询（左栏领域卡片 / 画像卡片 / 右栏四页签）
    // ------------------------------------------------------------------

    /// 按领域/节点聚合的进度（左栏领域卡片 + 中栏节点列表）。
    pub fn organization_progress(&self, archive_id: &str) -> Adm4Result<OrganizationProgress> {
        let engine = self.open_engine(archive_id)?;
        Ok(engine.organization_progress())
    }

    /// 全部决策点的 UI 视图（中栏检查单数据源）：带领域/节点归属、MDA 标注、设计提问、
    /// 选择基数、逐选项已选/主选状态、适用性与豁免记录。
    ///
    /// 一次返回全图（现有包 20-30 个点，量级可忽略），由 UI 按节点/层级过滤——
    /// 避免为每种筛选条件各开一个查询。
    pub fn decision_points(&self, archive_id: &str) -> Adm4Result<Vec<DecisionPointView>> {
        let engine = self.open_engine(archive_id)?;
        let state = engine.state();
        let space = engine.space();
        let applicability = engine.applicability();
        let mut views = Vec::with_capacity(space.graph.points().len());
        for point in space.graph.points() {
            let selection = state.selections.get(&point.id);
            let node_id = space
                .organization
                .effective_node_id(point.node_id.as_deref())
                .to_string();
            let (status, exemption) = match applicability.get(&point.id) {
                Some(PointApplicability::Active) => ("active", None),
                Some(PointApplicability::Inactive) => ("inactive", None),
                Some(PointApplicability::BeyondDepth) => ("beyond_depth", None),
                Some(PointApplicability::NotApplicable(justification)) => (
                    "not_applicable",
                    Some(ExemptionView {
                        reason_code: justification.reason_code.clone(),
                        note: justification.note.clone(),
                        // 无署名（baseline 理由码跳过 / 旧存档条目）→ None，UI 照实标注。
                        actor: justification
                            .is_signed()
                            .then(|| justification.actor.clone()),
                        at: justification.is_signed().then(|| justification.at.clone()),
                    }),
                ),
                None => ("unknown", None),
            };
            let options = point
                .options
                .iter()
                .map(|option| DecisionOptionView {
                    option_id: option.id.clone(),
                    label: option.label.clone(),
                    summary: option.summary.clone(),
                    selected: selection.is_some_and(|item| item.contains_option(&option.id)),
                    is_primary: selection
                        .and_then(|item| item.primary_option.as_deref())
                        .is_some_and(|primary| primary == option.id),
                })
                .collect();
            views.push(DecisionPointView {
                decision_id: point.id.clone(),
                level: point.level,
                domain_id: space.organization.domain_of_node(&node_id).to_string(),
                node_id,
                question: point.question.clone(),
                design_question: point.design_question.clone(),
                mda_layer: point.mda_layer.map(|layer| layer.label().to_string()),
                selection_mode: point.selection_mode,
                requirement: point.requirement,
                requirement_label: point.requirement.label().to_string(),
                optional: point.requirement.is_optional(),
                applicability: status.to_string(),
                confirmed: selection.is_some_and(|item| item.confirmed_by_user),
                options,
                exemption,
            });
        }
        Ok(views)
    }

    /// 项目画像卡片：取点清单里的已确认决策点 → 「决策点提问 + 已选选项名」条目。
    ///
    /// 二版的画像六字段在四版不再单独存一份数据（那会与决策点重复存储、双向同步），
    /// 而是从决策点聚合出来：字段集合由清单决定，代码里没有任何硬编码字段名。
    ///
    /// 取点口径：品类包声明了 `profile_points` 就按它取（顺序即展示顺序），否则回退
    /// 「L0/L1 全取」。为什么需要显式清单——二版画像六字段里的「美术风格」「目标用户」
    /// 落在 L3/L4 的检查单点上，按层级过滤永远上不了卡；而改那些点的层级会连带动完成度
    /// 分母。清单是**纯展示层**的取点，不参与 `requirement`/适用性/完备度的任何判定。
    ///
    /// 未确认的提案/预填一律不上卡（与完成度口径一致）；清单里尚未作答的点自然缺席。
    pub fn project_profile(&self, archive_id: &str) -> Adm4Result<ProjectProfile> {
        let engine = self.open_engine(archive_id)?;
        let state = engine.state();
        let space = engine.space();
        let declared = &space.pack.profile_points;
        // 清单非空 → 按清单顺序；为空 → 回退决策图顺序上的 L0/L1 点（旧数据零影响）。
        let selected_points: Vec<&DecisionPoint> = if declared.is_empty() {
            space
                .graph
                .points()
                .iter()
                .filter(|point| matches!(point.level, DesignLevel::L0 | DesignLevel::L1))
                .collect()
        } else {
            // 清单 id 的存在性由 `space validate` 保证（写错的 id 装载即 fail-closed），
            // 这里对残余的未知 id 只是跳过：它到不了运行期。
            declared
                .iter()
                .filter_map(|decision_id| space.graph.point(decision_id))
                .collect()
        };
        let mut fields = Vec::new();
        for point in selected_points {
            let Some(selection) = state.selections.get(&point.id) else {
                continue;
            };
            if !selection.confirmed_by_user {
                continue; // 未确认的提案/预填不上画像卡（与完成度口径一致）。
            }
            let mut selected = Vec::new();
            for item in selection.selected_options() {
                let Some(option) = point.option(item.option_id) else {
                    continue;
                };
                selected.push(ProfileOption {
                    option_id: item.option_id.to_string(),
                    label: option.label.clone(),
                    is_primary: item.is_primary,
                });
            }
            let node_id = space
                .organization
                .effective_node_id(point.node_id.as_deref())
                .to_string();
            fields.push(ProfileField {
                decision_id: point.id.clone(),
                level: point.level,
                label: point.question.clone(),
                design_question: point.design_question.clone(),
                mda_layer: point.mda_layer.map(|layer| layer.label().to_string()),
                domain_id: space.organization.domain_of_node(&node_id).to_string(),
                node_id,
                selected,
            });
        }
        Ok(ProjectProfile {
            project_name: state.project_name.clone(),
            genre_pack: state.genre_pack.clone(),
            depth_target: state.depth_profile.target,
            fields,
        })
    }

    /// 工作台右栏四页签一次取齐：摘要 / 缺失项 / 风险 / 校验。
    ///
    /// 四块全部复用既有引擎查询（完成度、组织聚合、一致性引擎、冻结门预检），
    /// 不引入第二套算法、不落任何派生存储——UI 每次拿到的都是当前状态的实时投影。
    pub fn workbench_overview(&self, archive_id: &str) -> Adm4Result<WorkbenchOverview> {
        let engine = self.open_engine(archive_id)?;
        let state = engine.state();
        let space = engine.space();
        let applicability = engine.applicability();
        let completeness = engine.completeness();
        let progress = engine.organization_progress();

        // --- 摘要 ---
        let summary = WorkbenchSummary {
            project_name: state.project_name.clone(),
            genre_pack: state.genre_pack.clone(),
            pack_version: state.pack_version.clone(),
            depth_target: state.depth_profile.target,
            revision: state.revision,
            frozen_versions: state.frozen_versions,
            done: completeness.done,
            total: completeness.total,
            percent: completeness.percent(),
            optional_skipped: completeness.optional_skipped,
            domains: progress.domains.clone(),
            nodes: progress.nodes.clone(),
            counts: progress.total,
        };

        // --- 缺失项：未确认且适用的决策点，按领域分组 ---
        let mut blocking_by_decision: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for item in &completeness.blocking {
            blocking_by_decision
                .entry(item.decision_id.as_str())
                .or_default()
                .push(item.detail.as_str());
        }
        let mut missing_by_domain: BTreeMap<String, Vec<MissingEntry>> = BTreeMap::new();
        for point in space.graph.points() {
            // 与完成度分母同口径：未作答的非必做点不算缺失项（它们的计数走 summary.optional_skipped）。
            if !counts_toward_completeness(point, &applicability, &state.selections) {
                continue;
            }
            let selection = state.selections.get(&point.id);
            let details = blocking_by_decision.get(point.id.as_str());
            let confirmed = selection.is_some_and(|item| item.confirmed_by_user);
            if confirmed && details.is_none() {
                continue;
            }
            let reasons = match details {
                Some(items) => items.iter().map(|detail| (*detail).to_string()).collect(),
                None if selection.is_none() => vec!["未选择".to_string()],
                None => vec!["未经用户确认".to_string()],
            };
            let node_id = space
                .organization
                .effective_node_id(point.node_id.as_deref())
                .to_string();
            let domain_id = space.organization.domain_of_node(&node_id).to_string();
            missing_by_domain
                .entry(domain_id)
                .or_default()
                .push(MissingEntry {
                    decision_id: point.id.clone(),
                    node_id,
                    level: point.level,
                    question: point.question.clone(),
                    reasons,
                });
        }
        let mut missing = Vec::new();
        for domain in space.organization.domains() {
            if let Some(items) = missing_by_domain.remove(&domain.id) {
                missing.push(MissingByDomain {
                    domain_id: domain.id.clone(),
                    domain_name: domain.name.clone(),
                    items,
                });
            }
        }

        // --- 风险：节点风险说明 + 最近一次红队发现 ---
        let node_risks = state
            .node_risk_notes
            .iter()
            .map(|(node_id, note)| NodeRiskNote {
                node_id: node_id.clone(),
                node_name: match space.organization.node(node_id) {
                    Some(node) => node.name.clone(),
                    None => node_id.clone(),
                },
                domain_id: space.organization.domain_of_node(node_id).to_string(),
                note: note.clone(),
            })
            .collect();
        let red_team = state.red_team.as_ref().map(|record| RedTeamSummary {
            reviewed_revision: record.reviewed_revision,
            stale: record.reviewed_revision != state.revision,
            reviewer: record.proof.reviewer.clone(),
            findings: record
                .findings
                .iter()
                .map(|finding| RedTeamFinding {
                    id: finding.id.clone(),
                    severity: finding.severity.clone(),
                    target: finding.target.clone(),
                    text: finding.text.clone(),
                    disposed: finding.disposition.is_some(),
                })
                .collect(),
        });
        let risk = WorkbenchRisk {
            node_risks,
            red_team,
        };

        // --- 校验：外键违规 + 冻结门预检 ---
        let scanner = self.skin_scanner_for_project(space, archive_id, &state.project_name)?;
        let gate_report = evaluate_freeze_gates(&engine, &scanner);
        let validation = WorkbenchValidation {
            row_reference_violations: check_row_references(
                &space.graph,
                &state.selections,
                &space.row_references(),
            )
            .into_iter()
            .map(|violation| RowReferenceIssue {
                rule_id: violation.rule_id,
                decision_id: violation.decision_id,
                detail: violation.detail,
            })
            .collect(),
            all_gates_passed: gate_report.all_passed(),
            gates: gate_report
                .gates
                .iter()
                .map(|gate| GateSummary {
                    gate: gate.gate.clone(),
                    passed: gate.passed,
                    finding_count: gate.findings.len(),
                    findings: gate.findings.clone(),
                })
                .collect(),
        };

        Ok(WorkbenchOverview {
            summary,
            missing,
            risk,
            validation,
        })
    }

    // ------------------------------------------------------------------
    // 冻结
    // ------------------------------------------------------------------

    pub fn freeze_check(&self, archive_id: &str) -> Adm4Result<FreezeGateReport> {
        let engine = self.open_engine(archive_id)?;
        let scanner = self.skin_scanner_for_project(
            engine.space(),
            archive_id,
            &engine.state().project_name,
        )?;
        Ok(evaluate_freeze_gates(&engine, &scanner))
    }

    /// 冻结门第 4 道：运行 AI 红队（结果持久化到项目状态）。
    pub fn freeze_red_team(&self, archive_id: &str) -> Adm4Result<usize> {
        let provider = self.build_provider()?;
        self.freeze_red_team_with(archive_id, provider.as_ref())
    }

    pub fn freeze_red_team_with(
        &self,
        archive_id: &str,
        provider: &dyn AiProvider,
    ) -> Adm4Result<usize> {
        self.with_project(archive_id, |engine| {
            let record = run_red_team(engine, provider)?;
            Ok(record.findings.len())
        })
    }

    /// 执行冻结（全门通过才成功）；冻结产物写入 frozen/v{N}/。
    pub fn freeze_run(&self, archive_id: &str) -> Adm4Result<FrozenDesign> {
        let frozen = self.with_project(archive_id, |engine| {
            let scanner = self.skin_scanner_for_project(
                engine.space(),
                archive_id,
                &engine.state().project_name,
            )?;
            execute_freeze(engine, &scanner)
        })?;
        // 写冻结产物（在事务外补写：冻结文件本身只读追加，不影响 authoring_state 一致性）。
        let frozen_dir = self
            .archives
            .content_dir(archive_id)
            .join("frozen")
            .join(format!("v{}", frozen.version));
        ensure_dir(&frozen_dir)?;
        write_json_file(&frozen_dir.join("frozen_design.json"), &frozen)?;
        write_json_file(&frozen_dir.join("gate_report.json"), &frozen.gate_report)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "freeze",
            &format!(
                "项目 {archive_id} 冻结 v{}（{}）",
                frozen.version, frozen.content_hash
            ),
        )?;
        Ok(frozen)
    }

    pub fn load_frozen(&self, archive_id: &str, version: u32) -> Adm4Result<FrozenDesign> {
        read_json_file(
            &self
                .archives
                .content_dir(archive_id)
                .join("frozen")
                .join(format!("v{version}"))
                .join("frozen_design.json"),
        )
    }

    pub fn latest_frozen_version(&self, archive_id: &str) -> Adm4Result<u32> {
        let state = self.load_authoring_state(archive_id)?;
        if state.frozen_versions == 0 {
            return Err(Adm4Error::not_found("项目尚未冻结"));
        }
        Ok(state.frozen_versions)
    }

    // ------------------------------------------------------------------
    // 流水线
    // ------------------------------------------------------------------

    fn artifact_store(&self, archive_id: &str, version: u32) -> Adm4Result<ArtifactStore> {
        self.versioned_store(archive_id, version, PIPELINE_SECTION)
    }

    /// 某个冻结版本下的产物仓（`section` 区分 Phase 1 文档编译与 Phase 2 构建产线）。
    ///
    /// 两段各占一个子目录（`pipeline/v{N}` 与 `build/v{N}`），因此各有一份自己的
    /// `run_state.json`——否则 Phase 2 一开跑就会把 Phase 1 的运行状态覆盖掉。
    /// 换皮扫描器与冻结版本的解析逻辑两段共用，不复制第二份。
    fn versioned_store(
        &self,
        archive_id: &str,
        version: u32,
        section: &str,
    ) -> Adm4Result<ArtifactStore> {
        let state = self.load_authoring_state(archive_id)?;
        let space = self.load_space_shared(&state.genre_pack)?;
        // C0 文档标题就是项目名：产物落盘钩子必须豁免本项目自身名，否则「另存模板 →
        // 认证」把项目名登记进词表后，该项目自己的流水线立刻走不通（R5 豁免作用域见
        // `skin_scanner_for_project`）。
        //
        // 豁免的是**冻结当时**的项目名（`frozen.project_name`）而不是创作态的当前名：
        // 产物由冻结版本渲染，冻结后改名不影响已冻结版本，文档里写的仍是旧名。
        // 拿当前名去豁免，改过名的项目重跑流水线就会被自己的旧名拦住。
        let frozen_name = self.load_frozen(archive_id, version)?.project_name;
        let scanner = self.skin_scanner_for_project(&space, archive_id, &frozen_name)?;
        let root = self
            .archives
            .content_dir(archive_id)
            .join(section)
            .join(format!("v{version}"));
        ensure_dir(&root)?;
        Ok(ArtifactStore::new(root, scanner))
    }

    /// 运行 C0-C6（区间）。AI 未配置 → AiUnavailable（blocked，无兜底）。
    pub fn pipeline_run(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
    ) -> Adm4Result<PipelineRunState> {
        let provider = self.build_provider()?;
        self.pipeline_run_with(archive_id, from, to, provider.as_ref())
    }

    pub fn pipeline_run_with(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
        provider: &dyn AiProvider,
    ) -> Adm4Result<PipelineRunState> {
        Ok(self
            .pipeline_run_with_cancel(archive_id, from, to, provider, &CancelSignal::never())?
            .state)
    }

    /// `pipeline_run_with` 的可取消变体：调用方（GUI 主线程）持 `CancelSignal` 的克隆，
    /// 工作线程把它传进来；运行器在**每个阶段开始前**检查，命中即停止推进。
    ///
    /// 协作式取消不打断段内正在进行的 AI 调用——被取消的那一段记为「未运行」而非失败，
    /// 已完成段的产物与成功状态原样保留，下次照常断点续跑。
    pub fn pipeline_run_with_cancel(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
        provider: &dyn AiProvider,
        cancel: &CancelSignal,
    ) -> Adm4Result<PipelineRunOutcome> {
        let version = self.latest_frozen_version(archive_id)?;
        let frozen = self.load_frozen(archive_id, version)?;
        let space = self.load_space_shared(&frozen.genre_pack)?;
        let store = self.artifact_store(archive_id, version)?;
        let runner = PipelineRunner::new();
        let ctx = RunnerContext {
            frozen: &frozen,
            space: &space,
            ai: provider,
            store: &store,
        };
        let outcome = runner.run_range_with_cancel(&ctx, from, to, cancel)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "pipeline",
            &format!("项目 {archive_id} v{version} 运行 {from}..{to}"),
        )?;
        self.log_cancellation(archive_id, version, outcome.cancelled_at.as_deref())?;
        Ok(outcome)
    }

    /// 强制重跑（激活 Provider 版）：重置 `from` 及其全部下游后从 `from` 跑到 `to`。
    pub fn pipeline_rerun(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
    ) -> Adm4Result<PipelineRerunOutcome> {
        let provider = self.build_provider()?;
        self.pipeline_rerun_with(archive_id, from, to, provider.as_ref())
    }

    pub fn pipeline_rerun_with(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
        provider: &dyn AiProvider,
    ) -> Adm4Result<PipelineRerunOutcome> {
        self.pipeline_rerun_with_cancel(archive_id, from, to, provider, &CancelSignal::never())
    }

    /// 强制重跑指定阶段（可取消）：**连带重置该段及其全部下游**的运行状态与已落盘产物，
    /// 然后从该段正常向后跑到 `to`。
    ///
    /// 为什么不能只重跑单段：下游产物是按旧契约渲染的，保留它们的「已成功」会产出
    /// 「C2 新版 + C4 旧版」的错版文档集。重置范围内已通过的人工门（C5/C6）一并作废，
    /// 需重新署名确认（R3：旧署名不为新产物背书）。作废明细逐条进运行日志。
    pub fn pipeline_rerun_with_cancel(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
        provider: &dyn AiProvider,
        cancel: &CancelSignal,
    ) -> Adm4Result<PipelineRerunOutcome> {
        let version = self.latest_frozen_version(archive_id)?;
        let frozen = self.load_frozen(archive_id, version)?;
        let space = self.load_space_shared(&frozen.genre_pack)?;
        let store = self.artifact_store(archive_id, version)?;
        let runner = PipelineRunner::new();
        let ctx = RunnerContext {
            frozen: &frozen,
            space: &space,
            ai: provider,
            store: &store,
        };
        let outcome = runner.rerun_from(&ctx, from, to, cancel)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "pipeline",
            &format!(
                "项目 {archive_id} v{version} 强制重跑 {from}..{to}：{}",
                outcome.reset.summary()
            ),
        )?;
        for revoked in &outcome.reset.revoked_confirmations {
            self.log.append(
                "pipeline",
                &format!(
                    "项目 {archive_id} v{version} 阶段 {} 的人工确认（{} 于 {}）随重跑作废，需重新确认（R3）",
                    revoked.stage_id, revoked.actor, revoked.at
                ),
            )?;
        }
        self.log_cancellation(archive_id, version, outcome.cancelled_at.as_deref())?;
        Ok(outcome)
    }

    /// 用户取消落日志：取消不是失败，但必须在审计流里留痕，否则「为什么停在 C3」无从追查。
    fn log_cancellation(
        &self,
        archive_id: &str,
        version: u32,
        cancelled_at: Option<&str>,
    ) -> Adm4Result<()> {
        let Some(stage_id) = cancelled_at else {
            return Ok(());
        };
        self.log.append(
            "pipeline",
            &format!(
                "项目 {archive_id} v{version} 被用户取消：停在阶段 {stage_id} 之前，该段记为未运行（非失败），已完成段的产物保留"
            ),
        )
    }

    /// 阶段产物查询（只读，流水线视图的「产物入口」）：给定冻结版本与阶段 id，
    /// 返回双格式产物的存在性/路径/sha256/字节数与 `document.md` 预览文本。
    ///
    /// 缺文件如实标缺失（`complete=false` + `missing` 列名），不用空串兜底（R2）；
    /// 超大文档只回传前 `DOCUMENT_PREVIEW_LIMIT_BYTES` 字节并置 `document_truncated=true`，
    /// 而 sha256/字节数恒为整份文件的真值。
    pub fn pipeline_artifact(
        &self,
        archive_id: &str,
        version: u32,
        stage_id: &str,
    ) -> Adm4Result<StageArtifactView> {
        let content = self.require_content_dir(archive_id)?;
        let version_dir = content.join("pipeline").join(format!("v{version}"));
        StageArtifactView::build(&version_dir, archive_id, version, stage_id)
    }

    pub fn pipeline_status(&self, archive_id: &str) -> Adm4Result<PipelineRunState> {
        let version = self.latest_frozen_version(archive_id)?;
        let store = self.artifact_store(archive_id, version)?;
        store.load_run_state()
    }

    pub fn pipeline_confirm(
        &self,
        archive_id: &str,
        stage_id: &str,
        actor: &str,
        note: &str,
    ) -> Adm4Result<PipelineRunState> {
        let version = self.latest_frozen_version(archive_id)?;
        let store = self.artifact_store(archive_id, version)?;
        let runner = PipelineRunner::new();
        let state = runner.confirm_human_gate(&store, stage_id, actor, note)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "pipeline",
            &format!("项目 {archive_id} 阶段 {stage_id} 人工确认（{actor}）"),
        )?;
        Ok(state)
    }

    // ------------------------------------------------------------------
    // Phase 2 构建产线（P0-P5；本波执行器全是诚实空实现，如实 Blocked）
    // ------------------------------------------------------------------

    /// Phase 2 版图（只读）：阶段 id / 名称 / 依赖 / 产出与消费的制品 / 待哪一波实现。
    ///
    /// 呈现层（CLI 与桌面流水线视图的 P 段）拿它画状态行，**不在 UI 里写死任何阶段文案**——
    /// 后续波次把某段实现掉，界面上的说明会跟着注册表一起变。
    pub fn build_plan(&self) -> Adm4Result<Vec<BuildStageView>> {
        let order = phase2_execution_order()?;
        let artifacts = phase2_artifacts();
        let mut views = Vec::with_capacity(order.len());
        for stage in phase2_registry() {
            let declared = artifacts
                .iter()
                .find(|item| item.stage_id == stage.id)
                .ok_or_else(|| Adm4Error::internal(format!("阶段 {} 缺制品声明", stage.id)))?;
            let label = |kinds: &[ArtifactKind]| -> Vec<String> {
                kinds
                    .iter()
                    .map(|kind| kind.label_zh().to_string())
                    .collect()
            };
            views.push(BuildStageView {
                stage_id: stage.id.clone(),
                name: stage.name.clone(),
                summary: stage.summary.clone(),
                depends_on: stage.depends_on.clone(),
                produces: label(&declared.produces),
                consumes: label(&declared.consumes),
                pending_note: pending_stage(&stage.id).map(PendingStage::blocked_reason),
            });
        }
        Ok(views)
    }

    /// Phase 2 的产物仓（`content/build/v{N}`）。
    fn build_store(&self, archive_id: &str, version: u32) -> Adm4Result<ArtifactStore> {
        self.versioned_store(archive_id, version, BUILD_SECTION)
    }

    /// Phase 2 的唯一真源：Phase 1 C0 落盘的 `GameSpec`。
    ///
    /// C0 没跑过就**直接报错**而不是就地重编一份：重编等于在 Phase 2 里造第二个真源（D22），
    /// 而且会绕开 C1-C6 的验证与人工门。缺就说缺，让人回去把 C0 跑掉。
    fn build_source_spec(&self, archive_id: &str, version: u32) -> Adm4Result<GameSpec> {
        let store = self.artifact_store(archive_id, version)?;
        store.read_contract::<GameSpec>("C0").map_err(|error| {
            Adm4Error::blocked(format!(
                "读不到冻结版本 v{version} 的 C0 规格编译产物（{}）：\
                 Phase 2 一切派生自 GameSpec，请先跑 pipeline run <项目> --from C0",
                error.message
            ))
        })
    }

    /// 运行 P0-P5（区间）。本波每段都会如实 Blocked 并说清在等哪一波。
    pub fn build_run(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
    ) -> Adm4Result<PipelineRunState> {
        Ok(self
            .build_run_with_cancel(archive_id, from, to, &CancelSignal::never())?
            .state)
    }

    /// `build_run` 的可取消变体（语义同 [`AppServices::pipeline_run_with_cancel`]：
    /// 段边界粒度的协作式取消，被取消的段记为未运行而非失败）。
    pub fn build_run_with_cancel(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
        cancel: &CancelSignal,
    ) -> Adm4Result<PipelineRunOutcome> {
        let version = self.latest_frozen_version(archive_id)?;
        let spec = self.build_source_spec(archive_id, version)?;
        let store = self.build_store(archive_id, version)?;
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let outcome = Phase2Runner::new().run_range_with_cancel(&ctx, from, to, cancel)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "build",
            &format!("项目 {archive_id} v{version} 构建运行 {from}..{to}"),
        )?;
        self.log_build_cancellation(archive_id, version, outcome.cancelled_at.as_deref())?;
        Ok(outcome)
    }

    /// 强制重跑 P 段：重置 `from` 及其全部下游（状态 + 产物 + 人工门署名）后再跑。
    pub fn build_rerun(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
    ) -> Adm4Result<PipelineRerunOutcome> {
        self.build_rerun_with_cancel(archive_id, from, to, &CancelSignal::never())
    }

    /// `build_rerun` 的可取消变体。作废明细（含被作废的人工门署名）逐条进运行日志（R3）。
    pub fn build_rerun_with_cancel(
        &self,
        archive_id: &str,
        from: &str,
        to: &str,
        cancel: &CancelSignal,
    ) -> Adm4Result<PipelineRerunOutcome> {
        let version = self.latest_frozen_version(archive_id)?;
        let spec = self.build_source_spec(archive_id, version)?;
        let store = self.build_store(archive_id, version)?;
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let outcome = Phase2Runner::new().rerun_from(&ctx, from, to, cancel)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "build",
            &format!(
                "项目 {archive_id} v{version} 构建强制重跑 {from}..{to}：{}",
                outcome.reset.summary()
            ),
        )?;
        for revoked in &outcome.reset.revoked_confirmations {
            self.log.append(
                "build",
                &format!(
                    "项目 {archive_id} v{version} 构建阶段 {} 的人工确认（{} 于 {}）随重跑作废，需重新确认（R3）",
                    revoked.stage_id, revoked.actor, revoked.at
                ),
            )?;
        }
        self.log_build_cancellation(archive_id, version, outcome.cancelled_at.as_deref())?;
        Ok(outcome)
    }

    /// 构建运行状态（只读）。
    pub fn build_status(&self, archive_id: &str) -> Adm4Result<PipelineRunState> {
        let version = self.latest_frozen_version(archive_id)?;
        self.build_store(archive_id, version)?.load_run_state()
    }

    /// 构建段的人工门确认（署名 + 结论必填，R3）。
    pub fn build_confirm(
        &self,
        archive_id: &str,
        stage_id: &str,
        actor: &str,
        note: &str,
    ) -> Adm4Result<PipelineRunState> {
        let version = self.latest_frozen_version(archive_id)?;
        let store = self.build_store(archive_id, version)?;
        let state = Phase2Runner::new().confirm_human_gate(&store, stage_id, actor, note)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "build",
            &format!("项目 {archive_id} 构建阶段 {stage_id} 人工确认（{actor}）"),
        )?;
        Ok(state)
    }

    /// 构建段的取消落日志（与 [`AppServices::log_cancellation`] 同一口径，只是分类不同）。
    fn log_build_cancellation(
        &self,
        archive_id: &str,
        version: u32,
        cancelled_at: Option<&str>,
    ) -> Adm4Result<()> {
        let Some(stage_id) = cancelled_at else {
            return Ok(());
        };
        self.log.append(
            "build",
            &format!(
                "项目 {archive_id} v{version} 构建被用户取消：停在阶段 {stage_id} 之前，该段记为未运行（非失败），已完成段的产物保留"
            ),
        )
    }

    // ------------------------------------------------------------------
    // 设计阶段美术风格锚点门（册 08 §2，选项 A）
    //
    // 这道门在**冻结之前**跑：用户看真图、改词、反复重出图、署名确认，锁定
    // `style_anchor_set` + `style_application_contract`。Phase 2 的 P2 资产生产只消费
    // 锁定产物（G1 的制品注册表把「风格锚点集」声明为 P2 的外部输入），因此本组还提供
    // 「锚点是否就绪」的可判定查询给 runner/呈现层用。
    //
    // 门面在这里只做四件事：取真源、装配换皮扫描器、注入图像通道、落日志与指纹。
    // 一切状态迁移与拒绝判定在 `adm4_build::art::style_anchor`（D14：GUI/CLI 无业务规则）。
    // ------------------------------------------------------------------

    /// 风格门的产物仓（`content/style`），**建目录**：写入路径用。
    fn style_store(&self, archive_id: &str) -> Adm4Result<StyleAnchorStore> {
        let store = self.style_store_read(archive_id)?;
        ensure_dir(store.root())?;
        Ok(store)
    }

    /// 风格门的产物仓（只读路径用，**不建目录**）。
    ///
    /// 只读查询不该改动存档：建一个空 `style/` 目录会让内容指纹变化，
    /// 于是「只查了个状态」之后存档体检报不一致——那是一条查不出来源的假警报。
    fn style_store_read(&self, archive_id: &str) -> Adm4Result<StyleAnchorStore> {
        Ok(StyleAnchorStore::new(
            self.require_content_dir(archive_id)?.join(STYLE_SECTION),
        ))
    }

    /// 风格门的真源事实：创作态里**已确认**的画像决策点（册 08 §2.1 的「L0-L2 画像」）。
    ///
    /// 取点口径与 [`AppServices::project_profile`] **完全同源**（品类包的 `profile_points`
    /// 清单，缺清单回退 L0/L1；未确认的一律不上卡）。这里刻意不另开一套取点规则：
    /// 那就是第二真源（D22），而且会让「界面上的画像」与「提示词锚的事实」各说各话。
    ///
    /// 风格门在冻结前跑，所以真源**不是** `GameSpec`（那时还没有），而是创作态。
    /// `source_revision` 随锚点集落盘，用来判定「锁风格之后设计又变了没有」。
    fn style_source_facts(&self, archive_id: &str) -> Adm4Result<StyleSourceFacts> {
        let profile = self.project_profile(archive_id)?;
        let revision = self.load_authoring_state(archive_id)?.revision;
        let entries = profile
            .fields
            .into_iter()
            .map(|field| {
                // 主选排在最前：它是这个多选点的代表答案，提示词里也该先出现。
                let mut options = field.selected;
                options.sort_by_key(|option| !option.is_primary);
                StyleSourceFact {
                    decision_id: field.decision_id,
                    question: field.label,
                    option_labels: options.into_iter().map(|option| option.label).collect(),
                }
            })
            .collect();
        Ok(StyleSourceFacts {
            project_name: profile.project_name,
            genre_pack: profile.genre_pack,
            source_revision: revision,
            entries,
        })
    }

    /// 风格提示词的换皮扫描器（R5，册 08 §5 把提示词列为强制扫描点）。
    ///
    /// 用**当前**项目名做豁免（提示词里必然含项目名）；口径与流水线落盘钩子同源，
    /// 见 [`AppServices::skin_scanner_for_project`] 的作用域说明。
    fn style_skin_scanner(&self, archive_id: &str) -> Adm4Result<SkinScanner> {
        let state = self.load_authoring_state(archive_id)?;
        let space = self.load_space_shared(&state.genre_pack)?;
        self.skin_scanner_for_project(&space, archive_id, &state.project_name)
    }

    /// 生成参数：预览尺寸取图像通道配置里的 `size`，没配图像通道则用默认值。
    ///
    /// 为什么尺寸不由调用方决定：同一个图像模型往往只接受几种固定尺寸，那是**通道**的
    /// 属性。让界面自己填尺寸的结果是用户填了个模型不接受的值，等一次超时才知道。
    pub fn style_options(
        &self,
        direction_count: usize,
        force: bool,
    ) -> Adm4Result<StyleGenerationOptions> {
        let mut options = StyleGenerationOptions {
            direction_count,
            force,
            ..StyleGenerationOptions::default()
        };
        if let Some(config) = self.read_config()?.image_provider.clone() {
            let (width, height) = config.parse_size()?;
            options.preview_width = width;
            options.preview_height = height;
        }
        options.validate()?;
        Ok(options)
    }

    /// 生成风格方向 + 预览图（走配置的真实图像通道；未配置 → 显式 blocked）。
    pub fn style_generate(
        &self,
        archive_id: &str,
        direction_count: usize,
        force: bool,
    ) -> Adm4Result<StyleSession> {
        let images = self.build_image_provider()?;
        let options = self.style_options(direction_count, force)?;
        self.style_generate_with(archive_id, images.as_ref(), &options)
    }

    /// [`AppServices::style_generate`] 的注入版：测试与冒烟传 `ScriptedImageProvider`（零网络）。
    pub fn style_generate_with(
        &self,
        archive_id: &str,
        images: &dyn ImageProvider,
        options: &StyleGenerationOptions,
    ) -> Adm4Result<StyleSession> {
        let facts = self.style_source_facts(archive_id)?;
        let scanner = self.style_skin_scanner(archive_id)?;
        let store = self.style_store(archive_id)?;
        let now = UtcTimestamp::now().to_iso8601();
        let outcome = StyleGate::new(&store).generate(&facts, &scanner, images, options, &now);
        // 成败都刷指纹与日志：失败那一轮的生成记录也落了盘（可停可续），
        // 不刷指纹会让存档体检报「内容与指纹不一致」。
        self.archives.refresh_fingerprint(archive_id)?;
        match &outcome {
            Ok(session) => self.log.append(
                "style",
                &format!(
                    "项目 {archive_id} 风格方向生成：{} 个方向、{}x{} 预览、共 {} 轮记录（图像通道 {}）",
                    session.directions.len(),
                    session.preview_width,
                    session.preview_height,
                    session.rounds.len(),
                    images.id()
                ),
            )?,
            Err(error) => self.log.append(
                "style",
                &format!(
                    "项目 {archive_id} 风格方向生成失败（原样上抛，不产占位图）：{}",
                    error.message
                ),
            )?,
        }
        outcome
    }

    /// 对某方向提交改词并重生成预览（册 08 §2.3，次数不限）。
    ///
    /// `prompt_override` 传空串 = 清掉改词、回到锚定真源的派生提示词。
    pub fn style_regenerate(
        &self,
        archive_id: &str,
        style_id: &str,
        prompt_override: &str,
    ) -> Adm4Result<StyleSession> {
        let images = self.build_image_provider()?;
        self.style_regenerate_with(archive_id, style_id, prompt_override, images.as_ref())
    }

    /// [`AppServices::style_regenerate`] 的注入版。
    pub fn style_regenerate_with(
        &self,
        archive_id: &str,
        style_id: &str,
        prompt_override: &str,
        images: &dyn ImageProvider,
    ) -> Adm4Result<StyleSession> {
        let scanner = self.style_skin_scanner(archive_id)?;
        let store = self.style_store(archive_id)?;
        let now = UtcTimestamp::now().to_iso8601();
        let outcome =
            StyleGate::new(&store).regenerate(style_id, prompt_override, &scanner, images, &now);
        self.archives.refresh_fingerprint(archive_id)?;
        let note = if prompt_override.trim().is_empty() {
            "清掉改词，回到派生提示词".to_string()
        } else {
            format!("改词「{}」", truncate_chars(prompt_override.trim(), 80))
        };
        match &outcome {
            Ok(session) => self.log.append(
                "style",
                &format!(
                    "项目 {archive_id} 风格方向 {style_id} 重生成（{note}）：第 {} 轮（图像通道 {}）",
                    session.rounds.len(),
                    images.id()
                ),
            )?,
            Err(error) => self.log.append(
                "style",
                &format!(
                    "项目 {archive_id} 风格方向 {style_id} 重生成失败（{note}）：{}",
                    error.message
                ),
            )?,
        }
        outcome
    }

    /// attended 确认并锁定风格锚点（R3：署名 + 结论双必填，拒绝在服务层）。
    ///
    /// 落**新版本** `style/anchors/v{N}`；旧版本一个字节都不动（D4 不可变历史）。
    /// 重选风格就是再确认一次 → v{N+1}，因此「这版游戏当时锁的什么风格」永远查得到。
    pub fn style_confirm(
        &self,
        archive_id: &str,
        style_id: &str,
        actor: &str,
        note: &str,
    ) -> Adm4Result<StyleLockOutcome> {
        let store = self.style_store(archive_id)?;
        let now = UtcTimestamp::now().to_iso8601();
        let outcome = StyleGate::new(&store).confirm(style_id, actor, note, &now)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "style",
            &format!(
                "项目 {archive_id} 风格锚点 v{} 已确认：方向 {}（{}），署名 {}",
                outcome.anchor_set.anchor_version,
                outcome.anchor_set.selected_style_id,
                outcome.anchor_set.selected_title,
                outcome.anchor_set.confirmation.actor
            ),
        )?;
        self.log.append(
            "style",
            &format!(
                "项目 {archive_id} 风格锚点 v{} 确认结论：{}",
                outcome.anchor_set.anchor_version, outcome.anchor_set.confirmation.notes
            ),
        )?;
        if let Some(superseded) = outcome.superseded_version {
            self.log.append(
                "style",
                &format!(
                    "项目 {archive_id} 风格锚点 v{superseded} 被 v{} 取代：旧版不改不删，仍是可回溯的历史事实（D4）",
                    outcome.anchor_set.anchor_version
                ),
            )?;
        }
        Ok(outcome)
    }

    /// 风格门状态（只读投影，CLI 与桌面共用一份口径）。
    pub fn style_status(&self, archive_id: &str) -> Adm4Result<StyleGateStatus> {
        let revision = self.load_authoring_state(archive_id)?.revision;
        let store = self.style_store_read(archive_id)?;
        StyleGate::new(&store).status(revision)
    }

    /// 风格门工作态原样读出（尚未生成过 → `Ok(None)`）。
    ///
    /// 呈现层需要完整的提示词（状态投影里的是截断摘要）：改词编辑框要把当前提示词
    /// 原样填进去，截断过的文本一保存就把用户的提示词截短了。
    pub fn style_session(&self, archive_id: &str) -> Adm4Result<Option<StyleSession>> {
        self.style_store_read(archive_id)?.load_session()
    }

    /// **风格锚点是否就绪**：Phase 2 的 P2 资产生产把「风格锚点集」当外部输入消费，
    /// 这就是那个「外部输入到位了没有」的查询。
    ///
    /// 未就绪不是错误（设计阶段本来就有没定风格的时刻），它是一条要显示给人看的结论；
    /// 要真正阻断下游时调 [`StyleReadiness::require_ready`]。
    pub fn style_readiness(&self, archive_id: &str) -> Adm4Result<StyleReadiness> {
        self.style_store_read(archive_id)?.readiness()
    }

    pub fn style_anchor_set(&self, archive_id: &str, version: u32) -> Adm4Result<StyleAnchorSet> {
        self.style_store_read(archive_id)?.load_anchor_set(version)
    }

    pub fn style_application_contract(
        &self,
        archive_id: &str,
        version: u32,
    ) -> Adm4Result<StyleApplicationContract> {
        self.style_store_read(archive_id)?
            .load_application_contract(version)
    }

    pub fn style_fit_report(&self, archive_id: &str, version: u32) -> Adm4Result<StyleFitReport> {
        self.style_store_read(archive_id)?.load_fit_report(version)
    }

    /// 风格图（预览图/锚图）的绝对路径：呈现层按它加载图片。
    ///
    /// 只接受**相对路径**（锚点集与工作态里记的就是相对路径），越界组件一律拒；
    /// 文件不存在也如实报错——返回一个不存在的路径会让界面显示空白而查不出原因。
    pub fn style_image_path(&self, archive_id: &str, relative: &str) -> Adm4Result<PathBuf> {
        let store = self.style_store_read(archive_id)?;
        let safe = ensure_within_root(Path::new(relative))?;
        let path = store.absolute(&safe.to_string_lossy());
        if !path.is_file() {
            return Err(Adm4Error::not_found(format!(
                "风格图 {relative} 不在案（{}）：请重新生成该方向的预览图",
                path.display()
            )));
        }
        Ok(path)
    }

    // ------------------------------------------------------------------
    // 逆向产线（模板五步：S1 检索 → S2 映射 → S3 交叉核验 → S4 人工审核 → S5 认证）
    // ------------------------------------------------------------------

    /// 产线起点：新建模板草稿（状态 Draft）。
    /// game_name 必填——它与 aliases 是认证时换皮词表的登记来源（R5）。
    pub fn template_new_draft(
        &self,
        pack_id: &str,
        template_id: &str,
        game_name: &str,
        aliases: &[String],
        depth: DesignLevel,
    ) -> Adm4Result<Template> {
        check_template_ref(pack_id, template_id)?;
        let template_id = template_id.trim();
        let game_name = game_name.trim();
        if template_id.is_empty() {
            return Err(Adm4Error::invalid_input("模板 id 不能为空"));
        }
        if game_name.is_empty() {
            return Err(Adm4Error::invalid_input(
                "逆向目标游戏名不能为空（认证时登记换皮词表的来源）",
            ));
        }
        let space = self.load_space_shared(pack_id)?;
        if self.templates.get(pack_id, template_id).is_ok() {
            return Err(Adm4Error::conflict(format!(
                "模板 {pack_id}/{template_id} 已存在，不能重复建草稿（防止覆盖产线进度）"
            )));
        }
        let template = Template {
            template_id: template_id.to_string(),
            game_name: game_name.to_string(),
            aliases: aliases.to_vec(),
            genre_pack: pack_id.to_string(),
            pack_version: space.pack.pack_version.clone(),
            depth_reached: depth,
            answers: Vec::new(),
            certification: Certification::default(),
            origin: TemplateOrigin::Reverse,
            mapping_hash: String::new(),
            crosscheck_proof: None,
        };
        self.templates.save_draft(&template)?;
        self.log.append(
            "template",
            &format!("新建模板草稿 {pack_id}/{template_id}（逆向目标：{game_name}）"),
        )?;
        Ok(template)
    }

    /// 另存模板（项目 → 模板）：把当前项目**已确认**的决策点选择导出为一份模板。
    ///
    /// 与逆向产线的关系：本方法**不走** S1 检索 / S2 映射 / S3 交叉核验。那三步的产出是
    /// 「外部语料 → 证据链 → 独立二次核验」，而本项目导出根本没有外部语料，跑一遍只能
    /// 产出编造的证据。它的可信依据是「答卷里每一条都在源项目里被用户确认过」，
    /// 因此直接落 S4 人工审核态（`reviewer` + `note` 双必填，R3），
    /// 之后仍须由 [`AppServices::template_certify`] 认证（S5）才能预填。
    ///
    /// 只导出已确认的点：未确认的选择是提案/预填的半成品，把没定的东西当定论传播给
    /// 别的项目，比这份模板缺几条更糟。多选点的全部已选选项、主选标记与参数值一并导出；
    /// 决策点的选择理由落在答卷 `notes` 上。
    ///
    /// 证据字段一律留空：本项目导出没有外部来源可引，`origin` 已如实记录它来自哪个存档，
    /// 硬塞一条 `adm4://…` 的假 URL 只会让证据链失去意义（宁缺勿造）。
    ///
    /// 落盘前整份模板过换皮扫描（R5）：项目里残留的参考游戏名（典型是预填未改写的
    /// 「模板预填自 X」理由）会在这里被拦下，而不是随模板扩散到下一个项目。
    pub fn template_export_from_project(
        &self,
        archive_id: &str,
        template_id: &str,
        game_name: &str,
        aliases: &[String],
        reviewer: &str,
        note: &str,
    ) -> Adm4Result<TemplateExportReport> {
        let state = self.load_authoring_state(archive_id)?;
        check_template_ref(&state.genre_pack, template_id)?;
        let template_id = template_id.trim();
        if template_id.is_empty() {
            return Err(Adm4Error::invalid_input("模板 id 不能为空"));
        }
        // 模板展示名默认取项目名：另存出来的模板在列表里得能认出是哪个项目的定稿。
        let game_name = match game_name.trim() {
            "" => state.project_name.trim(),
            explicit => explicit,
        };
        if game_name.is_empty() {
            return Err(Adm4Error::invalid_input(
                "模板展示名不能为空（项目名也为空时请显式给出）",
            ));
        }
        if self.templates.get(&state.genre_pack, template_id).is_ok() {
            return Err(Adm4Error::conflict(format!(
                "模板 {}/{template_id} 已存在，不能覆盖（另存请换一个模板 id）",
                state.genre_pack
            )));
        }
        let space = self.load_space_shared(&state.genre_pack)?;

        let mut answers = Vec::new();
        let mut skipped_unconfirmed = 0usize;
        let mut skipped_unknown = Vec::new();
        let mut additional_option_count = 0usize;
        let mut primary_marks = 0usize;
        // 按决策图顺序导出，使同一项目两次另存出来的答卷逐字节一致（可对比、可复核）。
        for point in space.graph.points() {
            let Some(selection) = state.selections.get(&point.id) else {
                continue;
            };
            if !selection.confirmed_by_user {
                skipped_unconfirmed += 1;
                continue;
            }
            if point.option(&selection.option_id).is_none() {
                // 清单改过而项目里还留着旧选项：如实记账，不静默塞进模板（R2）。
                skipped_unknown.push(format!("{}/{}", point.id, selection.option_id));
                continue;
            }
            let mut additional_options = Vec::new();
            for extra in &selection.additional_options {
                if point.option(&extra.option_id).is_none() {
                    skipped_unknown.push(format!("{}/{}", point.id, extra.option_id));
                    continue;
                }
                additional_options.push(TemplateSelectedOption {
                    option_id: extra.option_id.clone(),
                    parameters: extra.parameters.clone(),
                });
            }
            additional_option_count += additional_options.len();
            let primary_option = selection.primary_option.clone();
            if primary_option.is_some() {
                primary_marks += 1;
            }
            answers.push(TemplateAnswer {
                decision_id: point.id.clone(),
                option_id: selection.option_id.clone(),
                parameters: selection.parameters.clone(),
                evidence: Vec::new(),
                notes: selection.rationale.clone(),
                crosscheck_agreed: None,
                additional_options,
                primary_option,
            });
        }
        if answers.is_empty() {
            return Err(Adm4Error::blocked(format!(
                "项目 {archive_id} 没有任何已确认的决策点，另存模板会产出一份空答卷（跳过 {skipped_unconfirmed} 个未确认点）"
            )));
        }

        let mut template = Template {
            template_id: template_id.to_string(),
            game_name: game_name.to_string(),
            aliases: aliases.to_vec(),
            genre_pack: state.genre_pack.clone(),
            pack_version: space.pack.pack_version.clone(),
            depth_reached: state.depth_profile.target,
            answers,
            certification: Certification::default(),
            origin: TemplateOrigin::ProjectExport {
                source_archive_id: archive_id.to_string(),
                source_project_name: state.project_name.clone(),
                exported_at: UtcTimestamp::now().to_iso8601(),
            },
            mapping_hash: String::new(),
            crosscheck_proof: None,
        };
        template.record_project_export_review(reviewer, note)?;

        // R5：落盘钩子。整份模板（含理由文本与参数）过换皮扫描，命中即拒绝落盘。
        // 豁免本项目自身名：模板的 game_name 与 origin.source_project_name 就是项目名，
        // 而它可能已被上一次「另存 + 认证」登记进词表——那不该拦住源项目再导一份。
        // 豁免的成立条件见 `skin_scanner_for_project`：该词的登记来源只能是本存档的导出。
        let scanner = self.skin_scanner_for_project(&space, archive_id, &state.project_name)?;
        let serialized = serde_json::to_string(&template)
            .map_err(|error| Adm4Error::internal(format!("序列化另存模板失败：{error}")))?;
        let hits = scanner.scan(
            &format!("{}/references/{template_id}.json", state.genre_pack),
            &serialized,
        );
        if !hits.is_empty() {
            let detail: Vec<String> = hits
                .iter()
                .map(|hit| format!("{} 命中 {}", hit.location, hit.matched_word))
                .collect();
            return Err(Adm4Error::red_line(format!(
                "R5: 另存模板命中参考名（{} 处）：{}——请先改写项目里的相关表述再另存",
                hits.len(),
                detail.join("; ")
            )));
        }
        self.templates.save_draft(&template)?;

        let report = TemplateExportReport {
            template_id: template.template_id.clone(),
            genre_pack: template.genre_pack.clone(),
            game_name: template.game_name.clone(),
            source_archive_id: archive_id.to_string(),
            source_project_name: state.project_name.clone(),
            depth_reached: template.depth_reached,
            status: format!("{:?}", template.certification.status),
            reviewed_by: template.certification.reviewed_by.clone(),
            exported_points: template.answers.len(),
            exported_additional_options: additional_option_count,
            exported_primary_marks: primary_marks,
            skipped_unconfirmed,
            skipped_unknown,
        };
        self.log.append(
            "template",
            &format!(
                "项目 {archive_id} 另存模板 {}/{}：{}",
                report.genre_pack,
                report.template_id,
                report.summary()
            ),
        )?;
        for skipped in &report.skipped_unknown {
            self.log.append(
                "template",
                &format!(
                    "另存模板 {}/{} 跳过 {skipped}：选项已不在当前装配空间内",
                    report.genre_pack, report.template_id
                ),
            )?;
        }
        Ok(report)
    }

    /// S1 语料检索：本地语料通道（D5，零网络）检索证据候选并累积进模板候选池。
    /// 候选池按 source_url 去重排序持久化——S2 映射只认池内来源（禁止 AI 编造来源）。
    pub fn template_search_corpus(
        &self,
        pack_id: &str,
        template_id: &str,
        corpus_root: &Path,
        decision_question: &str,
        keywords: &[String],
    ) -> Adm4Result<Vec<EvidenceCandidate>> {
        check_template_ref(pack_id, template_id)?;
        let template = self.templates.get(pack_id, template_id)?;
        let channel = FileCorpusChannel::new(corpus_root);
        let hits = channel.search(&EvidenceQuery {
            game_name: template.game_name.clone(),
            decision_question: decision_question.to_string(),
            keywords: keywords.to_vec(),
        })?;
        let pool_path = self.template_candidates_path(pack_id, template_id);
        let mut pool: Vec<EvidenceCandidate> = if pool_path.is_file() {
            read_json_file(&pool_path)?
        } else {
            Vec::new()
        };
        for hit in &hits {
            if !pool
                .iter()
                .any(|candidate| candidate.source_url == hit.source_url)
            {
                pool.push(hit.clone());
            }
        }
        pool.sort_by(|left, right| left.source_url.cmp(&right.source_url));
        write_json_file(&pool_path, &pool)?;
        self.log.append(
            "template",
            &format!(
                "模板 {pack_id}/{template_id} 语料检索命中 {} 条（累计候选 {} 条）",
                hits.len(),
                pool.len()
            ),
        )?;
        Ok(hits)
    }

    /// S2 AI 映射（激活 Provider 版）。
    pub fn template_map(&self, pack_id: &str, template_id: &str) -> Adm4Result<usize> {
        let provider = self.build_provider()?;
        self.template_map_with(pack_id, template_id, provider.as_ref())
    }

    /// S2 AI 映射：候选证据 → 逆向答卷，Draft→Mapped。
    /// 无证据答案整卷拒收（R1）、AI 非法输出直接 Err（R7），失败时模板保持原状。
    pub fn template_map_with(
        &self,
        pack_id: &str,
        template_id: &str,
        provider: &dyn AiProvider,
    ) -> Adm4Result<usize> {
        check_template_ref(pack_id, template_id)?;
        let mut template = self.templates.get(pack_id, template_id)?;
        let pool_path = self.template_candidates_path(pack_id, template_id);
        let candidates: Vec<EvidenceCandidate> = if pool_path.is_file() {
            read_json_file(&pool_path)?
        } else {
            Vec::new()
        };
        if candidates.is_empty() {
            return Err(Adm4Error::blocked(format!(
                "模板 {pack_id}/{template_id} 没有任何证据候选：请先执行语料检索（S1）——无证据不可映射（R1）"
            )));
        }
        let space = self.load_space_shared(pack_id)?;
        let mapped = MappingService::map_answers(
            provider,
            &mut template,
            space.graph.points(),
            &candidates,
        )?;
        self.templates.save_draft(&template)?;
        self.log.append(
            "template",
            &format!("模板 {pack_id}/{template_id} AI 映射 {mapped} 条答案（Draft→Mapped）"),
        )?;
        Ok(mapped)
    }

    /// S3 交叉核验（激活 Provider 版）。
    pub fn template_cross_check(
        &self,
        pack_id: &str,
        template_id: &str,
    ) -> Adm4Result<CrossCheckReport> {
        let provider = self.build_provider()?;
        self.template_cross_check_with(pack_id, template_id, provider.as_ref())
    }

    /// S3 交叉核验：独立二次 AI 会话对照映射结果，Mapped→CrossChecked（D7）。
    /// 冲突条目降级为待人工（不采信任一方），由 S4 人工审核裁决。
    pub fn template_cross_check_with(
        &self,
        pack_id: &str,
        template_id: &str,
        provider: &dyn AiProvider,
    ) -> Adm4Result<CrossCheckReport> {
        check_template_ref(pack_id, template_id)?;
        let mut template = self.templates.get(pack_id, template_id)?;
        let space = self.load_space_shared(pack_id)?;
        let report = CrossCheckService::cross_check(provider, &mut template, space.graph.points())?;
        self.templates.save_draft(&template)?;
        self.log.append(
            "template",
            &format!(
                "模板 {pack_id}/{template_id} 交叉核验 {} 条，冲突待人工 {} 条（Mapped→CrossChecked）",
                report.entries.len(),
                report.conflict_ids().len()
            ),
        )?;
        Ok(report)
    }

    /// S4 人工审核：署名 + 结论必填（R3 评审工作量证明），CrossChecked→HumanReviewed。
    pub fn template_review(
        &self,
        pack_id: &str,
        template_id: &str,
        reviewer: &str,
        note: &str,
    ) -> Adm4Result<Template> {
        check_template_ref(pack_id, template_id)?;
        let mut template = self.templates.get(pack_id, template_id)?;
        self.templates.human_review(&mut template, reviewer, note)?;
        self.log.append(
            "template",
            &format!(
                "模板 {pack_id}/{template_id} 人工审核通过（评审人：{reviewer}）（CrossChecked→HumanReviewed）"
            ),
        )?;
        Ok(template)
    }

    /// S5 认证入库：HumanReviewed→Certified（跳级/回退由状态机拒绝），
    /// game_name + aliases 自动登记进全局换皮词表（R5）。只有 Certified 模板可预填/对照。
    pub fn template_certify(&self, pack_id: &str, template_id: &str) -> Adm4Result<Template> {
        check_template_ref(pack_id, template_id)?;
        let mut template = self.templates.get(pack_id, template_id)?;
        self.templates
            .certify(&mut template, &self.skin_wordlist_path())?;
        self.log.append(
            "template",
            &format!(
                "模板 {pack_id}/{template_id} 认证入库，登记换皮词 {} 个（HumanReviewed→Certified）",
                template.skin_words().len()
            ),
        )?;
        Ok(template)
    }

    /// 模板对照（只读查询，对照模式的侧栏数据源）：认证模板答卷 vs 项目当前选择。
    /// 模板不进项目；未认证模板与预填一样被拒（走同一取用关卡）。
    pub fn template_compare(
        &self,
        archive_id: &str,
        template_id: &str,
    ) -> Adm4Result<TemplateComparison> {
        let state = self.load_authoring_state(archive_id)?;
        let template = self
            .templates
            .approved_for_prefill(&state.genre_pack, template_id)?;
        let entries = template
            .answers
            .iter()
            .map(|answer| {
                let selection = state.selections.get(&answer.decision_id);
                TemplateCompareEntry {
                    decision_id: answer.decision_id.clone(),
                    template_option: answer.option_id.clone(),
                    template_options: answer
                        .selected_option_ids()
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                    template_primary: answer.primary_option.clone(),
                    template_parameters: answer.parameters.clone(),
                    template_notes: answer.notes.clone(),
                    project_option: selection.map(|current| current.option_id.clone()),
                    project_confirmed: selection.is_some_and(|current| current.confirmed_by_user),
                    same_option: selection
                        .is_some_and(|current| current.option_id == answer.option_id),
                }
            })
            .collect();
        Ok(TemplateComparison {
            template_id: template.template_id.clone(),
            game_name: template.game_name.clone(),
            entries,
        })
    }

    /// 模板候选证据池的持久化位置：`<design_space_root>/<pack>/references/.candidates/<id>.json`。
    /// 用 `.candidates` 点前缀目录避开 TemplateLibrary::list 对 references/*.json 的扫描。
    fn template_candidates_path(&self, pack_id: &str, template_id: &str) -> PathBuf {
        Path::new(&self.design_space_root)
            .join(pack_id)
            .join("references")
            .join(".candidates")
            .join(format!("{template_id}.json"))
    }

    // ------------------------------------------------------------------
    // AI 访谈（分层确认；confirm/reject 只能由 UI/CLI 的用户动作触发，AI 永不代提交）
    // ------------------------------------------------------------------

    /// 生成下一个访谈提案（激活 Provider 版）。
    pub fn interview_next(&self, archive_id: &str) -> Adm4Result<InterviewTurnDto> {
        let provider = self.build_provider()?;
        self.interview_next_with(archive_id, provider.as_ref())
    }

    /// 生成下一个访谈提案：L 层升序 + 同层拓扑序 + 被拒点排同层末尾（D9/D11）。
    /// 提案与 transcript 走 with_project 事务持久化；AI 非法输出 → Err 且不留痕（R7）。
    pub fn interview_next_with(
        &self,
        archive_id: &str,
        provider: &dyn AiProvider,
    ) -> Adm4Result<InterviewTurnDto> {
        let turn = self.with_project(archive_id, |engine| {
            InterviewService::propose_next(engine, provider)
        })?;
        let dto = InterviewTurnDto::from_turn(turn);
        let message = match &dto {
            InterviewTurnDto::StructuralPoint { proposal } => format!(
                "项目 {archive_id} 访谈提案（结构层）{}/{}",
                proposal.decision_id, proposal.option_id
            ),
            InterviewTurnDto::TableProposal { proposal } => format!(
                "项目 {archive_id} 访谈提案（整表）{}/{}",
                proposal.decision_id, proposal.option_id
            ),
            InterviewTurnDto::Complete => {
                format!("项目 {archive_id} 访谈完成：全部激活点已确认")
            }
        };
        self.log.append("interview", &message)?;
        Ok(dto)
    }

    /// 用户确认提案（唯一提交入口，D11）；`overrides` = 例外下钻（整表确认时替换若干行/格）。
    /// 返回参数校验问题清单（非空时进待填清单，不阻断确认）。
    pub fn interview_confirm(
        &self,
        archive_id: &str,
        proposal: &InterviewProposal,
        overrides: Option<ParameterValues>,
    ) -> Adm4Result<Vec<String>> {
        let drilled = overrides.is_some();
        let problems = self.with_project(archive_id, |engine| {
            InterviewService::confirm_proposal(engine, proposal, overrides)
        })?;
        self.log.append(
            "interview",
            &format!(
                "项目 {archive_id} 用户确认 {}/{}{}",
                proposal.decision_id,
                proposal.option_id,
                if drilled { "（例外下钻）" } else { "" }
            ),
        )?;
        Ok(problems)
    }

    /// 用户拒绝提案：不产生任何选择，决策点留在待办并排同层末尾（D11）。
    pub fn interview_reject(
        &self,
        archive_id: &str,
        decision_id: &str,
        note: &str,
    ) -> Adm4Result<()> {
        self.with_project(archive_id, |engine| {
            InterviewService::reject_proposal(engine, decision_id, note);
            Ok(())
        })?;
        self.log.append(
            "interview",
            &format!("项目 {archive_id} 用户拒绝 {decision_id}：{note}"),
        )
    }

    /// 查询分层访谈进度（只读）：当前层与各层「已确认/适用」计数，供 UI/CLI 展示。
    pub fn interview_progress(&self, archive_id: &str) -> Adm4Result<InterviewProgress> {
        let engine = self.open_engine(archive_id)?;
        Ok(InterviewService::progress(&engine))
    }

    // ------------------------------------------------------------------
    // AI
    // ------------------------------------------------------------------

    /// AI 体检（只诊断不修复）：能否构建出激活的 Provider、密钥能否解析。
    ///
    /// 本方法**不返回 Err**：「AI 不可用」是它要报告的结论，不是它自己的失败。
    /// 判定完全复用 [`AppServices::build_provider`]（不另写一套探测逻辑，否则体检说
    /// 可用而真调用报错，比不体检更糟）。
    ///
    /// **零网络**：只校验配置与密钥可解析性。因此 base_url 写错、密钥失效、模型名不存在
    /// 时它照旧报「可用」——要判定这些必须真打一次，见 [`AppServices::ai_invoke_check`]。
    pub fn ai_doctor(&self) -> AiDoctorReport {
        match self.build_provider() {
            Ok(provider) => AiDoctorReport {
                available: true,
                provider_id: provider.id().to_string(),
                detail: "已配置且密钥可解析".to_string(),
            },
            Err(error) => AiDoctorReport {
                available: false,
                provider_id: String::new(),
                detail: error.message,
            },
        }
    }

    /// 构建激活的 AI Provider；未配置 = AiUnavailable（R7：显式失败）。
    ///
    /// 读的是**当前生效配置**（含运行期 `reload_config` / `set_ai_provider` 的结果），
    /// 因此桌面端不必为了「刚改的配置能生效」重开门面。
    pub fn build_provider(&self) -> Adm4Result<Box<dyn AiProvider>> {
        let Some(config) = self.read_config()?.ai_provider.clone() else {
            return Err(Adm4Error::ai_unavailable(
                "未配置 AI Provider（config/app.json 的 ai_provider）",
            ));
        };
        let secret_ref = SecretRef::parse(&config.api_key_ref)?;
        let named = load_named_secrets(&self.data_root)?;
        let api_key = secret_ref.resolve(&named)?;
        Ok(Box::new(OpenAiCompatibleProvider::new(config, api_key)?))
    }

    /// 构建激活的**图像**Provider；未配置 = AiUnavailable（R7：显式失败，不产占位图）。
    ///
    /// 错误消息把「缺什么配置」说全（段名 + 字段名），因为这是绝大多数人第一次用风格门
    /// 时会撞上的那道墙：只说「图像通道不可用」等于让人自己去猜配置长什么样。
    pub fn build_image_provider(&self) -> Adm4Result<Box<dyn ImageProvider>> {
        let Some(config) = self.read_config()?.image_provider.clone() else {
            return Err(Adm4Error::ai_unavailable(
                "未配置图像 Provider：请在 config/app.json 补一段 image_provider\
                 （provider_id / base_url / model / api_key_ref，可选 size 如 1024x1024 与 timeout_secs）。\
                 风格门必须看真图，没有图像通道就是 blocked——绝不用占位图冒充真图（R7）",
            ));
        };
        let secret_ref = SecretRef::parse(&config.api_key_ref)?;
        let named = load_named_secrets(&self.data_root)?;
        let api_key = secret_ref.resolve(&named)?;
        Ok(Box::new(OpenAiCompatibleImageProvider::new(
            config, api_key,
        )?))
    }

    /// 图像通道体检（零网络）：只查配置在不在、尺寸解析得出来、密钥解析得出来。
    ///
    /// 与 [`AppServices::ai_doctor`] 同款语义与同款回执结构（呈现层复用一套行模型）：
    /// base_url 写错、密钥失效、模型名不存在时它照旧报「可用」——那些只能靠真生成一张图
    /// 才能判定，而生成图是要花钱的，因此本方法不代劳。
    pub fn image_doctor(&self) -> AiDoctorReport {
        match self.build_image_provider() {
            Ok(provider) => AiDoctorReport {
                available: true,
                provider_id: provider.id().to_string(),
                detail: "图像通道已配置，尺寸与密钥均可解析（本检查零网络，不代表真能出图）"
                    .to_string(),
            },
            Err(error) => AiDoctorReport {
                available: false,
                provider_id: String::new(),
                detail: error.message,
            },
        }
    }

    /// 设置（或以 `None` 清空）激活的图像 Provider：落盘 `config/app.json` **并**更新内存配置。
    ///
    /// 与 [`AppServices::set_ai_provider`] 同一套热更新语义（两端一律走门面，
    /// 不各自 load → 改 → save，否则磁盘变了而运行期快照没变）。
    pub fn set_image_provider(&self, provider: Option<HttpImageProviderConfig>) -> Adm4Result<()> {
        // 尺寸写错在保存时就拦下：等到生成时才报错，用户已经等了一次超时。
        if let Some(config) = &provider {
            config.parse_size()?;
        }
        let mut guard = self.write_config()?;
        let mut updated = guard.clone();
        updated.image_provider = provider;
        save_config(&self.data_root, &updated)?;
        let message = match &updated.image_provider {
            Some(config) => format!(
                "图像 Provider 配置更新：{}（模型 {}，尺寸 {}，密钥引用 {}）",
                config.provider_id, config.model, config.size, config.api_key_ref
            ),
            None => {
                "图像 Provider 配置已清空：风格门的生成入口将显式 blocked（无占位兜底）".to_string()
            }
        };
        *guard = updated;
        drop(guard);
        self.log.append("style", &message)
    }

    /// 写入一条 named secret（`config/secrets.json`），供配置里的 `named:<名字>` 引用。
    ///
    /// **返回值与运行日志都不含密钥值**：回执只说名字与字符数，日志只记名字。
    /// 密钥落点是数据根的 `config/`，不进存档内容树，因此不进导出包、不进内容指纹、
    /// 不进任何报告。
    pub fn ai_save_secret(&self, name: &str, value: &str) -> Adm4Result<String> {
        save_named_secret(&self.data_root, name, value)?;
        let name = name.trim();
        self.log.append(
            "ai",
            &format!("写入 named secret「{name}」（值已脱敏，不入日志）"),
        )?;
        Ok(format!(
            "已写入 named secret「{name}」（{} 字符，值不落日志/不进存档/不进报告）；\
             配置里用 named:{name} 引用它",
            value.chars().count()
        ))
    }

    /// 已登记的 named secret 名字（不含值）。
    pub fn ai_secret_names(&self) -> Adm4Result<Vec<String>> {
        crate::config::list_named_secret_names(&self.data_root)
    }

    /// AI **实调用**连通性检查：真发一次最小请求，如实报告成功/失败与原因。
    ///
    /// 与 [`AppServices::ai_doctor`] 的分工必须说清（两端的 `--help`/UI 文案同样要写）：
    /// - `ai_doctor` 只查「配置在不在、密钥解析得出来」，零网络。base_url 写错、
    ///   密钥失效、模型名不存在，它一律报「可用」——这正是 R7 关心的误报；
    /// - 本方法真打一次，因此上面那些错都会现形。
    ///
    /// 不返回 `Err`：「打不通」是它要报告的结论。但**绝不美化**——失败时 `succeeded=false`
    /// 且 `detail` 是原始错误消息（不改写、不吞、不重试成功）。空应答也算失败：
    /// 一个返回空文本的 provider 对下游没有任何用处，报「可用」等于误报。
    pub fn ai_invoke_check(&self) -> AiInvokeCheckReport {
        match self.build_provider() {
            Ok(provider) => self.ai_invoke_check_with(provider.as_ref()),
            Err(error) => AiInvokeCheckReport {
                succeeded: false,
                provider_id: String::new(),
                model: String::new(),
                response_chars: 0,
                elapsed_ms: 0,
                detail: error.message,
                at: UtcTimestamp::now().to_iso8601(),
            },
        }
    }

    /// [`AppServices::ai_invoke_check`] 的注入版：测试传 `ScriptedProvider`（零网络）。
    pub fn ai_invoke_check_with(&self, provider: &dyn AiProvider) -> AiInvokeCheckReport {
        let request = AiRequest {
            purpose: AI_INVOKE_CHECK_PURPOSE.to_string(),
            system_prompt: "你是连通性探针。只回复两个字符：OK".to_string(),
            user_prompt: "OK".to_string(),
            expect_json: false,
        };
        let started = std::time::Instant::now();
        let outcome = provider.invoke(&request);
        let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let at = UtcTimestamp::now().to_iso8601();
        let report = match outcome {
            Ok(response) if response.text.trim().is_empty() => AiInvokeCheckReport {
                succeeded: false,
                provider_id: response.provider_id,
                model: response.model,
                response_chars: 0,
                elapsed_ms,
                detail: "Provider 返回了空文本：调用链通但产出不可用，按失败报告（不美化）"
                    .to_string(),
                at,
            },
            Ok(response) => AiInvokeCheckReport {
                succeeded: true,
                provider_id: response.provider_id,
                model: response.model,
                response_chars: response.text.chars().count(),
                elapsed_ms,
                detail: truncate_chars(response.text.trim(), 120),
                at,
            },
            Err(error) => AiInvokeCheckReport {
                succeeded: false,
                provider_id: provider.id().to_string(),
                model: String::new(),
                response_chars: 0,
                elapsed_ms,
                detail: error.message,
                at,
            },
        };
        // 失败也落日志：连通性检查的历史是排查「什么时候开始打不通」的唯一线索。
        let mut report = report;
        if let Err(error) = self.log.append(
            "ai",
            &format!(
                "AI 实调用检查（{}）：{}",
                if report.succeeded { "成功" } else { "失败" },
                report.summary()
            ),
        ) {
            // 结论已经拿到，写日志失败不该把它变成错误；但也不许静默——附在回执里说出来。
            report
                .detail
                .push_str(&format!("；（注：运行日志写入失败：{}）", error.message));
        }
        report
    }

    // ------------------------------------------------------------------
    // SDK 知识库（全局审批队列；构建集成留 Phase 2）
    // ------------------------------------------------------------------

    /// 审批队列快照（记录清单 + 三态计数）。
    pub fn sdk_list(&self) -> Adm4Result<SdkSnapshot> {
        Ok(SdkKnowledgeBase::load(&self.data_root)?.snapshot())
    }

    /// 登记一条待审 SDK 资源，返回新记录 id。
    pub fn sdk_add(
        &self,
        sdk_name: &str,
        url: &str,
        category: &str,
        purpose: &str,
    ) -> Adm4Result<String> {
        let mut base = SdkKnowledgeBase::load(&self.data_root)?;
        let id = base.add_pending(sdk_name, url, category, purpose)?;
        base.save(&self.data_root)?;
        self.log
            .append("sdk", &format!("登记 SDK 资源 {sdk_name} → {id}（待审）"))?;
        Ok(id)
    }

    /// 批准一条待审 SDK 资源（署名 + 结论必填）。
    pub fn sdk_approve(&self, id: &str, reviewer: &str, note: &str) -> Adm4Result<()> {
        let mut base = SdkKnowledgeBase::load(&self.data_root)?;
        base.approve(id, reviewer, note)?;
        base.save(&self.data_root)?;
        self.log
            .append("sdk", &format!("SDK 资源 {id} 批准（署名 {reviewer}）"))
    }

    /// 拒绝一条待审 SDK 资源（署名 + 理由必填）。
    pub fn sdk_reject(&self, id: &str, reviewer: &str, note: &str) -> Adm4Result<()> {
        let mut base = SdkKnowledgeBase::load(&self.data_root)?;
        base.reject(id, reviewer, note)?;
        base.save(&self.data_root)?;
        self.log
            .append("sdk", &format!("SDK 资源 {id} 拒绝（署名 {reviewer}）"))
    }

    // ------------------------------------------------------------------
    // 补充开发变更流（项目内 content/change_requests.json）
    // ------------------------------------------------------------------

    /// 解析并校验项目内容目录（不存在即 not_found，给可读错误）。
    fn require_content_dir(&self, archive_id: &str) -> Adm4Result<PathBuf> {
        let content = self.archives.content_dir(archive_id);
        if !content.is_dir() {
            return Err(Adm4Error::not_found(format!(
                "存档 {archive_id} 不存在（可用 project list 查看现有项目）"
            )));
        }
        Ok(content)
    }

    /// 变更请求清单（按登记顺序）。
    pub fn change_list(&self, archive_id: &str) -> Adm4Result<Vec<ChangeRequest>> {
        let content = self.require_content_dir(archive_id)?;
        Ok(ChangeLog::load(&content)?.requests)
    }

    /// 登记一条补充开发变更请求（起草态），返回新 id。
    pub fn change_add(
        &self,
        archive_id: &str,
        title: &str,
        description: &str,
        requested_by: &str,
        target_frozen_version: u32,
    ) -> Adm4Result<String> {
        let content = self.require_content_dir(archive_id)?;
        let mut log = ChangeLog::load(&content)?;
        let id = log.add(title, description, requested_by, target_frozen_version)?;
        log.save(&content)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "change",
            &format!("项目 {archive_id} 登记变更请求 {id}：{}", title.trim()),
        )?;
        Ok(id)
    }

    /// 记录影响分析：填受影响段（C0..C6）并推到「已影响分析」。
    pub fn change_set_impact(
        &self,
        archive_id: &str,
        id: &str,
        affected_segments: &[String],
    ) -> Adm4Result<()> {
        let content = self.require_content_dir(archive_id)?;
        let mut log = ChangeLog::load(&content)?;
        log.set_impact(id, affected_segments)?;
        log.save(&content)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "change",
            &format!(
                "项目 {archive_id} 变更 {id} 影响分析：{}",
                affected_segments.join("/")
            ),
        )
    }

    /// 推进变更请求状态（署名 + 结论必填，线性下一步或拒绝）。
    pub fn change_advance(
        &self,
        archive_id: &str,
        id: &str,
        target: ChangeStatus,
        actor: &str,
        note: &str,
    ) -> Adm4Result<()> {
        let content = self.require_content_dir(archive_id)?;
        let mut log = ChangeLog::load(&content)?;
        log.advance(id, target, actor, note)?;
        log.save(&content)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "change",
            &format!(
                "项目 {archive_id} 变更 {id} 推进至 {}（署名 {}）",
                target.label_zh(),
                actor.trim()
            ),
        )
    }

    // ------------------------------------------------------------------
    // 文档集交付打包（读 C0-C6 产物 → content/deliverable/v{N}/manifest.json）
    // ------------------------------------------------------------------

    /// 打包指定冻结版本的设计文档集：清点 C0-C6 产物、算 sha256、写交付清单。
    /// 缺段不报错——清单标 `complete=false` 并列出缺失段。
    pub fn deliverable_package(
        &self,
        archive_id: &str,
        version: u32,
    ) -> Adm4Result<DeliverableManifest> {
        let content = self.require_content_dir(archive_id)?;
        let version_dir = content.join("pipeline").join(format!("v{version}"));
        let manifest = DeliverableManifest::build(
            &version_dir,
            archive_id,
            version,
            &UtcTimestamp::now().to_iso8601(),
        )?;
        let out_dir = content.join("deliverable").join(format!("v{version}"));
        ensure_dir(&out_dir)?;
        write_json_file(&out_dir.join("manifest.json"), &manifest)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "deliverable",
            &format!(
                "项目 {archive_id} v{version} 文档集打包：{}（{}/7 段齐备）",
                if manifest.complete {
                    "完整"
                } else {
                    "缺段"
                },
                DESIGN_STAGE_COUNT - manifest.missing_segments.len()
            ),
        )?;
        Ok(manifest)
    }

    /// 只读清点：按当前流水线产物重算交付清单，不落盘（视图刷新用）。
    pub fn deliverable_status(
        &self,
        archive_id: &str,
        version: u32,
    ) -> Adm4Result<DeliverableManifest> {
        let content = self.require_content_dir(archive_id)?;
        let version_dir = content.join("pipeline").join(format!("v{version}"));
        DeliverableManifest::build(
            &version_dir,
            archive_id,
            version,
            &UtcTimestamp::now().to_iso8601(),
        )
    }
}

/// C0-C6 设计文档段数（交付清单完整度分母）。
const DESIGN_STAGE_COUNT: usize = 7;

/// Phase 1 文档编译产物在存档内容树里的子目录名（存档兼容：不得更名）。
const PIPELINE_SECTION: &str = "pipeline";

/// Phase 2 构建产物的子目录名（与 Phase 1 分开，各持一份 run_state）。
const BUILD_SECTION: &str = "build";

/// Phase 2 版图的一行（只读投影：注册表 + 制品声明 + 待实现登记）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildStageView {
    pub stage_id: String,
    pub name: String,
    pub summary: String,
    pub depends_on: Vec<String>,
    /// 本段产出的制品（中文展示名）。
    pub produces: Vec<String>,
    /// 本段消费的制品（中文展示名）。
    pub consumes: Vec<String>,
    /// 执行器尚未实现时的诚实说明（已实现的段为 None）。
    pub pending_note: Option<String>,
}

/// 访谈回合 DTO：`InterviewTurn` 未派生 serde，门面层包装为可序列化结构，
/// 供 CLI/GUI 在「提案 → 用户确认」两次调用之间无损转发提案载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "turn", rename_all = "snake_case")]
pub enum InterviewTurnDto {
    /// 结构层（L0-L4）单点提案。
    StructuralPoint { proposal: InterviewProposal },
    /// L5/L6 整表提案：确认整表为一个决策单元，可带 overrides 例外下钻（D10）。
    TableProposal { proposal: InterviewProposal },
    /// 全部激活点已确认。
    Complete,
}

impl InterviewTurnDto {
    fn from_turn(turn: InterviewTurn) -> Self {
        match turn {
            InterviewTurn::StructuralPoint(proposal) => Self::StructuralPoint { proposal },
            InterviewTurn::TableProposal(proposal) => Self::TableProposal { proposal },
            InterviewTurn::Complete => Self::Complete,
        }
    }

    /// 提案载荷（Complete 回合为 None），免去调用方逐变体解构。
    pub fn proposal(&self) -> Option<&InterviewProposal> {
        match self {
            Self::StructuralPoint { proposal } | Self::TableProposal { proposal } => Some(proposal),
            Self::Complete => None,
        }
    }
}

// ---------------------------------------------------------------------------
// 工作台 DTO（左栏画像卡片 + 右栏四页签；全部只读投影，不落存储）
// ---------------------------------------------------------------------------

/// 决策点视图上的一个选项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionOptionView {
    pub option_id: String,
    pub label: String,
    pub summary: String,
    pub selected: bool,
    /// 多选点的主选标记。
    pub is_primary: bool,
}

/// 人工豁免/理由码跳过的在案记录（`actor` 为 None = baseline 理由码跳过，无署名）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExemptionView {
    pub reason_code: String,
    pub note: String,
    pub actor: Option<String>,
    pub at: Option<String>,
}

/// 决策点的 UI 视图（中栏检查单一行）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionPointView {
    pub decision_id: String,
    pub level: DesignLevel,
    pub domain_id: String,
    pub node_id: String,
    pub question: String,
    /// 二版「设计提问」。
    pub design_question: Option<String>,
    /// MDA 层展示标签（已本地化）。
    pub mda_layer: Option<String>,
    pub selection_mode: SelectionMode,
    pub requirement: PointRequirement,
    /// `requirement` 的中文展示名（UI 不必自己映射枚举）。
    pub requirement_label: String,
    /// 非必做点（`requirement=optional`）：未作答不进完成度分母、不拦冻结。
    pub optional: bool,
    /// `active` / `inactive` / `not_applicable` / `beyond_depth`。
    pub applicability: String,
    pub confirmed: bool,
    pub options: Vec<DecisionOptionView>,
    pub exemption: Option<ExemptionView>,
}

/// 画像字段上的一个已选选项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileOption {
    pub option_id: String,
    pub label: String,
    /// 多选点的主选标记（单选点恒 false）。
    pub is_primary: bool,
}

/// 画像卡片的一行：一个 L0/L1 决策点及其已选选项（主选在前）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileField {
    pub decision_id: String,
    pub level: DesignLevel,
    /// 展示名 = 决策点的 `question`（决策点没有独立 label 字段，question 即 UI 展示文本）。
    pub label: String,
    /// 二版「设计提问」（清单未声明时为 None）。
    pub design_question: Option<String>,
    /// MDA 层展示标签（已本地化）。
    pub mda_layer: Option<String>,
    pub domain_id: String,
    pub node_id: String,
    pub selected: Vec<ProfileOption>,
}

/// 项目画像卡片：字段集合完全由清单的 L0/L1 决策点决定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectProfile {
    pub project_name: String,
    pub genre_pack: String,
    pub depth_target: DesignLevel,
    pub fields: Vec<ProfileField>,
}

/// 摘要页签：领域 × 进度总览 + 总完成度。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchSummary {
    pub project_name: String,
    pub genre_pack: String,
    pub pack_version: String,
    pub depth_target: DesignLevel,
    pub revision: u64,
    pub frozen_versions: u32,
    /// 总完成度分子/分母/百分比（口径同 `CompletenessReport`）。
    pub done: usize,
    pub total: usize,
    pub percent: u8,
    /// 非必做且未作答的适用点数（不进 total，仅在案可见）。
    pub optional_skipped: usize,
    pub domains: Vec<DomainProgress>,
    pub nodes: Vec<NodeProgress>,
    /// 各领域计数求和（与 done/total 同源）。
    pub counts: ProgressCounts,
}

/// 缺失项页签的一条：未确认或参数未填齐的适用决策点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissingEntry {
    pub decision_id: String,
    pub node_id: String,
    pub level: DesignLevel,
    pub question: String,
    /// 缺什么（未选择 / 未确认 / 参数与外键明细，逐条列出）。
    pub reasons: Vec<String>,
}

/// 缺失项按领域分组（领域顺序同左栏卡片）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissingByDomain {
    pub domain_id: String,
    pub domain_name: String,
    pub items: Vec<MissingEntry>,
}

/// 风险页签：节点风险说明。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRiskNote {
    pub node_id: String,
    pub node_name: String,
    pub domain_id: String,
    pub note: String,
}

/// 风险页签：最近一次红队发现摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedTeamFinding {
    pub id: String,
    pub severity: String,
    pub target: String,
    pub text: String,
    pub disposed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedTeamSummary {
    pub reviewed_revision: u64,
    /// 设计已变更 → 该次评审过期，冻结门第 4 道会要求重跑。
    pub stale: bool,
    pub reviewer: String,
    pub findings: Vec<RedTeamFinding>,
}

/// 风险页签：无红队记录时 `red_team` 为 None，`node_risks` 为空表。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchRisk {
    pub node_risks: Vec<NodeRiskNote>,
    pub red_team: Option<RedTeamSummary>,
}

/// 校验页签：跨表外键违规。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowReferenceIssue {
    pub rule_id: String,
    pub decision_id: String,
    pub detail: String,
}

/// 校验页签：冻结门预检单门摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateSummary {
    pub gate: String,
    pub passed: bool,
    pub finding_count: usize,
    pub findings: Vec<GateFinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchValidation {
    pub row_reference_violations: Vec<RowReferenceIssue>,
    pub gates: Vec<GateSummary>,
    pub all_gates_passed: bool,
}

/// 工作台右栏四页签的一次性投影。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchOverview {
    pub summary: WorkbenchSummary,
    pub missing: Vec<MissingByDomain>,
    pub risk: WorkbenchRisk,
    pub validation: WorkbenchValidation,
}

/// 模板对照条目：某决策点上「模板怎么选」与「项目当前怎么选」的并排视图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateCompareEntry {
    pub decision_id: String,
    /// 答卷首选项（单选答案即唯一选项；多选答案为主选优先序列的第一项）。
    pub template_option: String,
    /// 答卷的全部已选选项（主选在前）；单选答案只有一项。
    #[serde(default)]
    pub template_options: Vec<String>,
    /// 答卷标记的主选（单选答案为 None）。
    #[serde(default)]
    pub template_primary: Option<String>,
    #[serde(default)]
    pub template_parameters: ParameterValues,
    #[serde(default)]
    pub template_notes: String,
    /// 项目当前选项（None = 项目尚未选择该点）。
    pub project_option: Option<String>,
    pub project_confirmed: bool,
    /// 项目选项与模板一致（项目未选时为 false）。
    pub same_option: bool,
}

/// 存档体检报告（`project doctor` 的结构化形态，CLI 与桌面端共用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDoctorReport {
    pub archive_id: String,
    /// 无任何问题（呈现层据此决定 `[OK]` 与退出码）。
    pub healthy: bool,
    /// 逐条问题描述（中文，可直接展示）。
    pub problems: Vec<String>,
}

/// AI 实调用连通性检查的调用意图（进 journal / ScriptedProvider 的脚本键）。
pub const AI_INVOKE_CHECK_PURPOSE: &str = "ai_invoke_check";

/// 截断到 `limit` 个字符（按字符而非字节，避免切坏多字节字符）。
fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit).collect();
    format!("{head}…（已截断）")
}

/// AI 体检报告（`ai doctor` 的结构化形态）。
///
/// `provider_id` 在不可用时为空串而不是 `Option`：呈现层直接插值即可，
/// 不必为了打印一行字去解包（也就不会有人在那里写 `unwrap_or`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiDoctorReport {
    pub available: bool,
    pub provider_id: String,
    /// 可用时说明结论，不可用时是原始错误消息（不改写、不吞掉）。
    pub detail: String,
}

/// AI 实调用连通性检查报告（`ai invoke-check` 的结构化形态）。
///
/// 与 [`AiDoctorReport`] 是两件事：`doctor` 查配置（零网络），本报告是「真打了一次」的结果。
/// 失败时 `detail` 是原始错误消息，`succeeded=false`——不存在「重试到成功」或「降级为可用」
/// 的路径（R7）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiInvokeCheckReport {
    pub succeeded: bool,
    /// 失败在「Provider 都没构建出来」这一步时为空串。
    pub provider_id: String,
    /// 成功时为实际应答的模型名；失败时可能为空（还没走到应答）。
    pub model: String,
    /// 应答字符数（成功时 > 0；空应答按失败报告）。
    pub response_chars: usize,
    /// 本次调用耗时（毫秒，含网络往返）——超时与「慢但通」靠它区分。
    pub elapsed_ms: u64,
    /// 成功：应答文本摘要（截断）；失败：原始错误消息。
    pub detail: String,
    pub at: String,
}

impl AiInvokeCheckReport {
    /// 一行摘要（CLI/GUI/日志共用）。
    pub fn summary(&self) -> String {
        if self.succeeded {
            format!(
                "Provider {} 模型 {} 实调用成功，{} 字符应答，耗时 {} ms：{}",
                self.provider_id, self.model, self.response_chars, self.elapsed_ms, self.detail
            )
        } else if self.provider_id.is_empty() {
            format!("未能构建 Provider，未发出请求：{}", self.detail)
        } else {
            format!(
                "Provider {} 实调用失败（耗时 {} ms）：{}",
                self.provider_id, self.elapsed_ms, self.detail
            )
        }
    }
}

/// 另存模板的落地回执：导出了什么、跳过了什么、落成了什么状态。
///
/// 「跳过了什么」必须与「导出了什么」一样显眼：一份模板缺了 30 个未确认的点，
/// 而使用者以为它是整卷定稿，是最容易出错的地方。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateExportReport {
    pub template_id: String,
    pub genre_pack: String,
    pub game_name: String,
    pub source_archive_id: String,
    pub source_project_name: String,
    pub depth_reached: DesignLevel,
    /// 落地后的认证状态（本项目导出恒为 `HumanReviewed`，还需 S5 认证才能预填）。
    pub status: String,
    pub reviewed_by: String,
    /// 导出的决策点数（= 答卷条数）。
    pub exported_points: usize,
    /// 导出的附加已选选项数（多选点，不含首选项）。
    pub exported_additional_options: usize,
    /// 带主选标记的决策点数。
    pub exported_primary_marks: usize,
    /// 因未确认而不进模板的决策点数。
    pub skipped_unconfirmed: usize,
    /// 因选项已不在当前装配空间而跳过的 `决策点/选项`（R2：不静默丢弃）。
    pub skipped_unknown: Vec<String>,
}

impl TemplateExportReport {
    /// 一行摘要（CLI/GUI/日志共用）。
    pub fn summary(&self) -> String {
        format!(
            "导出 {} 个已确认决策点（含 {} 个附加多选选项、{} 处主选），跳过未确认 {} 个、失效选项 {} 条，状态 {}",
            self.exported_points,
            self.exported_additional_options,
            self.exported_primary_marks,
            self.skipped_unconfirmed,
            self.skipped_unknown.len(),
            self.status
        )
    }
}

/// 模板对照报告（对照模式，D 侧栏数据）：模板不进项目，仅供展示参考。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateComparison {
    pub template_id: String,
    pub game_name: String,
    pub entries: Vec<TemplateCompareEntry>,
}

/// 模板/包 id 进入路径拼接前的护栏：拒绝 `..`、根、盘符等越界成分。
fn check_template_ref(pack_id: &str, template_id: &str) -> Adm4Result<()> {
    ensure_within_root(Path::new(pack_id))?;
    ensure_within_root(Path::new(template_id))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interview_turn_dto_round_trips_and_exposes_proposal() {
        let proposal = InterviewProposal {
            decision_id: "u.genre".into(),
            option_id: "lane_defense".into(),
            rationale: "结构契合".into(),
            parameters: ParameterValues::None,
        };
        let dto = InterviewTurnDto::TableProposal {
            proposal: proposal.clone(),
        };
        let json = serde_json::to_string(&dto).unwrap();
        let parsed: InterviewTurnDto = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, dto);
        assert_eq!(parsed.proposal(), Some(&proposal));
        assert_eq!(InterviewTurnDto::Complete.proposal(), None);
        // tag 字段固定为 turn（CLI/GUI 依赖该判别键）。
        assert!(json.contains(r#""turn":"table_proposal""#), "{json}");
    }

    #[test]
    fn template_ref_guard_rejects_path_escape() {
        assert!(check_template_ref("lane_defense", "tpl_ok").is_ok());
        assert!(check_template_ref("../evil", "tpl_ok").is_err());
        assert!(check_template_ref("lane_defense", "..\\evil").is_err());
    }
}
