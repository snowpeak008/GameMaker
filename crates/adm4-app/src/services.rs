use crate::config::{AppConfig, load_config, load_named_secrets};
use crate::runlog::RunLog;
use adm4_ai::{AiProvider, OpenAiCompatibleProvider, SecretRef};
use adm4_archive::{
    ArchiveLock, ArchiveManifest, ArchiveStore, DataRoot, export_package, import_package,
};
use adm4_authoring::{
    AuthoringEngine, AuthoringState, FreezeGateReport, FrozenDesign, InterviewProgress,
    InterviewProposal, InterviewService, InterviewTurn, evaluate_freeze_gates, execute_freeze,
    run_red_team,
};
use adm4_contracts::SkinScanner;
use adm4_decision::{DepthProfile, DesignLevel, ParameterValues};
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
use std::path::{Path, PathBuf};

/// 应用服务门面：GUI/CLI 的唯一入口。
pub struct AppServices {
    pub data_root: DataRoot,
    pub archives: ArchiveStore,
    pub config: AppConfig,
    pub log: RunLog,
    space_root: DesignSpaceRoot,
    templates: TemplateLibrary,
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
        })
    }

    // ------------------------------------------------------------------
    // 设计空间
    // ------------------------------------------------------------------

    pub fn list_packs(&self) -> Adm4Result<Vec<String>> {
        self.space_root.list_packs()
    }

    pub fn load_space(&self, pack_id: &str) -> Adm4Result<DesignSpace> {
        load_design_space(&self.space_root, pack_id)
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
        let space = self.load_space(pack_id)?;
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
            let applied = engine.prefill_from_template(&template)?;
            self.log.append(
                "project",
                &format!("模板 {template_id} 预填 {applied} 条（需换皮与逐条确认）"),
            )?;
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
        read_json_file(&content.join("authoring_state.json"))
    }

    /// 打开项目为创作引擎（加载状态 + 设计空间）。
    pub fn open_engine(&self, archive_id: &str) -> Adm4Result<AuthoringEngine> {
        let state = self.load_authoring_state(archive_id)?;
        let space = self.load_space(&state.genre_pack)?;
        AuthoringEngine::new(space, state)
    }

    /// 修改并事务性保存项目（持锁 → 草稿 → 变更 → 原子提交）。
    pub fn with_project<T>(
        &self,
        archive_id: &str,
        operation: impl FnOnce(&mut AuthoringEngine) -> Adm4Result<T>,
    ) -> Adm4Result<T> {
        let manifest = self.archives.manifest(archive_id)?;
        let session_id = new_id("session");
        let archive_dir = self.data_root.archive_dir(archive_id);
        let lock = ArchiveLock::acquire(&archive_dir, &session_id)?;
        let result = (|| {
            self.archives.create_draft(&session_id, Some(archive_id))?;
            let content = self.archives.draft_content_dir(&session_id);
            let state: AuthoringState = read_json_file(&content.join("authoring_state.json"))?;
            let space = self.load_space(&state.genre_pack)?;
            let mut engine = AuthoringEngine::new(space, state)?;
            let value = operation(&mut engine)?;
            write_json_file(&content.join("authoring_state.json"), engine.state())?;
            self.archives
                .commit_draft(&session_id, &manifest.project_name, Some(archive_id))?;
            Ok(value)
        })();
        lock.release()?;
        result
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
        let space = self.load_space(&state.genre_pack)?;
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
        let space = self.load_space(&frozen.genre_pack)?;
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
        let space = self.load_space(pack_id)?;
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
        let space = self.load_space(pack_id)?;
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
        let space = self.load_space(pack_id)?;
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

/// 模板对照条目：某决策点上「模板怎么选」与「项目当前怎么选」的并排视图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateCompareEntry {
    pub decision_id: String,
    pub template_option: String,
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
