use crate::config::{AppConfig, load_config, load_named_secrets};
use crate::runlog::RunLog;
use adm4_ai::{AiProvider, OpenAiCompatibleProvider, SecretRef};
use adm4_archive::{
    ArchiveLock, ArchiveManifest, ArchiveStore, DataRoot, export_package, import_package,
};
use adm4_authoring::{
    AuthoringEngine, AuthoringState, FreezeGateReport, FrozenDesign, GateFinding,
    InterviewProgress, InterviewProposal, InterviewService, InterviewTurn, PrefillReport,
    evaluate_freeze_gates, execute_freeze, run_red_team,
};
use adm4_contracts::SkinScanner;
use adm4_decision::{
    DepthProfile, DesignLevel, DomainProgress, NodeProgress, OrganizationProgress, ParameterValues,
    PointApplicability, PointRequirement, ProgressCounts, SelectionMode, check_row_references,
    counts_toward_completeness,
};
use adm4_foundation::{
    Adm4Error, Adm4Result, ensure_dir, ensure_within_root, new_id, read_json_file, write_json_file,
};
use adm4_pipeline::{ArtifactStore, PipelineRunState, PipelineRunner, RunnerContext};
use adm4_space::{DesignSpace, DesignSpaceRoot, load_design_space};
use adm4_template::{
    Certification, CrossCheckReport, CrossCheckService, EvidenceCandidate, EvidenceQuery,
    EvidenceSearchChannel, FileCorpusChannel, MappingService, Template, TemplateLibrary,
    load_skin_wordlist,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// 应用服务门面：GUI/CLI 的唯一入口。
pub struct AppServices {
    pub data_root: DataRoot,
    pub archives: ArchiveStore,
    pub config: AppConfig,
    pub log: RunLog,
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
        let space_root = DesignSpaceRoot::new(&config.design_space_root);
        let templates = TemplateLibrary::new(&config.design_space_root);
        let log = RunLog::new(&data_root);
        Ok(Self {
            archives: ArchiveStore::new(data_root.clone()),
            data_root,
            config,
            log,
            space_root,
            templates,
            space_cache: Mutex::new(BTreeMap::new()),
        })
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

    /// 换皮扫描词表：全局词表文件 + 品类包参考游戏名。
    pub fn skin_scanner(&self, space: &DesignSpace) -> Adm4Result<SkinScanner> {
        let wordlist_path = self.skin_wordlist_path();
        let mut words = load_skin_wordlist(&wordlist_path)?.words;
        words.extend(space.skin_words());
        Ok(SkinScanner::new(words))
    }

    pub fn skin_wordlist_path(&self) -> PathBuf {
        Path::new(&self.config.design_space_root).join("skin_wordlist.json")
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

    pub fn export_project(&self, archive_id: &str, output: &Path) -> Adm4Result<usize> {
        export_package(&self.archives.content_dir(archive_id), output)
    }

    pub fn import_project(&self, package: &Path, project_name: &str) -> Adm4Result<String> {
        let session_id = new_id("session");
        self.archives.create_draft(&session_id, None)?;
        let content = self.archives.draft_content_dir(&session_id);
        import_package(package, &content)?;
        let archive_id = self
            .archives
            .commit_draft(&session_id, project_name, None)?;
        self.log.append(
            "project",
            &format!("导入项目 {project_name} → {archive_id}"),
        )?;
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

    /// 项目画像卡片：L0/L1 已确认决策点 → 「决策点提问 + 已选选项名」条目。
    ///
    /// 二版的画像六字段在四版不再单独存一份数据（那会与决策点重复存储、双向同步），
    /// 而是从 L0/L1 决策点聚合出来：字段集合由清单决定，代码里没有任何硬编码字段名。
    pub fn project_profile(&self, archive_id: &str) -> Adm4Result<ProjectProfile> {
        let engine = self.open_engine(archive_id)?;
        let state = engine.state();
        let space = engine.space();
        let mut fields = Vec::new();
        for point in space.graph.points() {
            if !matches!(point.level, DesignLevel::L0 | DesignLevel::L1) {
                continue;
            }
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
        let scanner = self.skin_scanner(space)?;
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
        let scanner = self.skin_scanner(engine.space())?;
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
            let scanner_words = {
                let mut words = load_skin_wordlist(&self.skin_wordlist_path())?.words;
                words.extend(engine.space().skin_words());
                words
            };
            let scanner = SkinScanner::new(scanner_words);
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
        let state = self.load_authoring_state(archive_id)?;
        let space = self.load_space_shared(&state.genre_pack)?;
        let scanner = self.skin_scanner(&space)?;
        let root = self
            .archives
            .content_dir(archive_id)
            .join("pipeline")
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
        let state = runner.run_range(&ctx, from, to)?;
        self.archives.refresh_fingerprint(archive_id)?;
        self.log.append(
            "pipeline",
            &format!("项目 {archive_id} v{version} 运行 {from}..{to}"),
        )?;
        Ok(state)
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
        Path::new(&self.config.design_space_root)
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

    /// 构建激活的 AI Provider；未配置 = AiUnavailable（R7：显式失败）。
    pub fn build_provider(&self) -> Adm4Result<Box<dyn AiProvider>> {
        let Some(config) = &self.config.ai_provider else {
            return Err(Adm4Error::ai_unavailable(
                "未配置 AI Provider（config/app.json 的 ai_provider）",
            ));
        };
        let secret_ref = SecretRef::parse(&config.api_key_ref)?;
        let named = load_named_secrets(&self.data_root)?;
        let api_key = secret_ref.resolve(&named)?;
        Ok(Box::new(OpenAiCompatibleProvider::new(
            config.clone(),
            api_key,
        )?))
    }
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
