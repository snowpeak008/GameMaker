use crate::state::{AuthoringState, TemplateMode};
use adm4_decision::{
    ApplicabilityMap, CompletenessReport, DecisionId, NaJustification, OptionId,
    OrganizationProgress, ParameterValues, PointRequirement, Provenance, SelectedOption, Selection,
    aggregate_progress, compute_applicability, compute_completeness, counts_toward_completeness,
    validate_option_parameters, validate_parameters,
};
use adm4_foundation::{Adm4Error, Adm4Result, UtcTimestamp};
use adm4_space::DesignSpace;
use adm4_template::Template;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 模板预填时被跳过的一条答卷内容（R2：不静默丢弃，逐条在案）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefillSkip {
    pub decision_id: String,
    /// 被跳过的选项 id（决策点整条跳过时是答卷的首选项）。
    pub option_id: String,
    pub reason: String,
}

/// 模板预填结果：写入了什么、跳过了什么。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PrefillReport {
    /// 写入项目的决策点数。
    pub applied: usize,
    /// 写入的附加已选选项数（多选点；不含首选项）。
    pub multi_options_applied: usize,
    /// 逐条跳过的答卷内容。
    pub skipped: Vec<PrefillSkip>,
}

impl PrefillReport {
    pub fn skipped_count(&self) -> usize {
        self.skipped.len()
    }

    /// 一行摘要（日志与 CLI/GUI 提示共用，保证「跳过多少」永远可见）。
    pub fn summary(&self) -> String {
        format!(
            "写入 {} 个决策点（含 {} 个附加多选选项），跳过 {} 条",
            self.applied,
            self.multi_options_applied,
            self.skipped_count()
        )
    }
}

/// 工作台重置的清空计数（二版 `reset-design-workbench` 的四版归宿）。
///
/// 破坏性操作必须能回答「到底清了什么」，否则用户点完只看到一片空白，
/// 分不清是重置生效了还是项目坏了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkbenchResetReport {
    /// 清空的决策点选择数（含其参数值、理由、附加多选选项与主选标记）。
    pub cleared_selections: usize,
    /// 其中带主选标记的决策点数（子集，便于说明多选点也回到未作答）。
    pub cleared_primary_marks: usize,
    /// 其中填过参数值的决策点数（子集）。
    pub cleared_parameter_values: usize,
    /// 清除的 N/A 人工豁免与理由码跳过条数。
    pub cleared_exemptions: usize,
    /// 清除的节点设计说明条数。
    pub cleared_node_design_notes: usize,
    /// 清除的节点风险说明条数。
    pub cleared_node_risk_notes: usize,
    /// 重置执行者署名（R3）。
    pub actor: String,
    pub at: String,
}

impl WorkbenchResetReport {
    /// 一行摘要（日志与 CLI/GUI 提示共用）。
    pub fn summary(&self) -> String {
        format!(
            "清空 {} 个决策点选择（含 {} 处主选、{} 处参数值）、{} 处不适用豁免、{} 处设计说明、{} 处风险说明",
            self.cleared_selections,
            self.cleared_primary_marks,
            self.cleared_parameter_values,
            self.cleared_exemptions,
            self.cleared_node_design_notes,
            self.cleared_node_risk_notes
        )
    }

    /// 什么都没清（重置一个本来就空白的项目）。
    pub fn is_noop(&self) -> bool {
        self.cleared_selections == 0
            && self.cleared_exemptions == 0
            && self.cleared_node_design_notes == 0
            && self.cleared_node_risk_notes == 0
    }
}

/// 创作引擎：装配设计空间 + 项目状态，提供全部经校验的变更操作。
///
/// 设计空间以 `Arc` 持有：它在运行期只读，一个包的空间被多个引擎实例共享是常态
/// （工作台一次交互要开好几个只读引擎）。迁移后通用层清单 4.7MB，按值持有意味着
/// 每开一个引擎就深拷一遍整张决策图。`new` 接受 `impl Into<Arc<DesignSpace>>`，
/// 因此既可以传 `DesignSpace`（自动装箱）也可以传门面缓存里的 `Arc`（零拷贝）。
pub struct AuthoringEngine {
    space: Arc<DesignSpace>,
    state: AuthoringState,
}

impl AuthoringEngine {
    pub fn new(space: impl Into<Arc<DesignSpace>>, mut state: AuthoringState) -> Adm4Result<Self> {
        let space = space.into();
        if state.genre_pack != space.pack.pack_id {
            return Err(Adm4Error::conflict(format!(
                "项目品类包 {} 与加载的设计空间 {} 不一致",
                state.genre_pack, space.pack.pack_id
            )));
        }
        // 旧存档的豁免署名并行 map 就地合并进 NaJustification（署名只留一个真相源）。
        state.adopt_legacy_na_signoffs();
        Ok(Self { space, state })
    }

    pub fn space(&self) -> &DesignSpace {
        &self.space
    }

    pub fn state(&self) -> &AuthoringState {
        &self.state
    }

    pub fn into_state(self) -> AuthoringState {
        self.state
    }

    pub fn applicability(&self) -> ApplicabilityMap {
        compute_applicability(
            &self.space.graph,
            &self.state.selections,
            &self.state.not_applicable,
            self.state.depth_profile,
        )
    }

    pub fn completeness(&self) -> CompletenessReport {
        let applicability = self.applicability();
        compute_completeness(
            &self.space.graph,
            &self.state.selections,
            &self.state.not_applicable,
            &applicability,
            self.space.cardinality(),
            &self.space.row_references(),
        )
    }

    /// 按领域/节点聚合进度（左栏领域卡片 + 中栏节点列表的数据源）。
    pub fn organization_progress(&self) -> OrganizationProgress {
        let applicability = self.applicability();
        aggregate_progress(
            &self.space.graph,
            &self.space.organization,
            &self.state.selections,
            &applicability,
        )
    }

    /// 选择选项：requires 满足性 + conflicts 硬拦截。
    ///
    /// 无论单选还是多选点，本方法都把已选集合**重置**为这一个选项（清空附加选项与主选）；
    /// 多选点追加选项走 `add_option`。
    pub fn select_option(
        &mut self,
        decision_id: &str,
        option_id: &str,
        provenance: Provenance,
    ) -> Adm4Result<()> {
        self.check_option_dependencies(decision_id, option_id)?;
        let confirmed = matches!(provenance, Provenance::UserManual);
        self.state.selections.insert(
            decision_id.to_string(),
            Selection {
                decision_id: decision_id.to_string(),
                option_id: option_id.to_string(),
                parameters: ParameterValues::None,
                rationale: String::new(),
                provenance,
                confirmed_by_user: confirmed,
                template_original: None,
                additional_options: Vec::new(),
                primary_option: None,
            },
        );
        self.clear_na_records(decision_id);
        self.invalidate_red_team();
        Ok(())
    }

    /// 多选点追加一个已选选项（单选点拒绝）。首个选项仍走 `select_option`。
    pub fn add_option(&mut self, decision_id: &str, option_id: &str) -> Adm4Result<()> {
        let point = self.space.graph.require_point(decision_id)?;
        if !point.is_multi() {
            return Err(Adm4Error::invalid_input(format!(
                "决策点 {decision_id} 是单选点，不能追加选项（改选请用 select_option）"
            )));
        }
        if !self.state.selections.contains_key(decision_id) {
            return Err(Adm4Error::conflict(format!(
                "决策点 {decision_id} 尚无任何已选选项，请先 select_option 选定第一个"
            )));
        }
        self.check_option_dependencies(decision_id, option_id)?;
        let selection = self.state.selections.get_mut(decision_id).ok_or_else(|| {
            Adm4Error::internal(format!("决策点 {decision_id} 的选择记录在追加过程中消失"))
        })?;
        if selection.contains_option(option_id) {
            return Err(Adm4Error::conflict(format!(
                "决策点 {decision_id} 已选中 {option_id}"
            )));
        }
        selection.additional_options.push(SelectedOption {
            option_id: option_id.to_string(),
            parameters: ParameterValues::None,
            rationale: String::new(),
            template_original: None,
        });
        // 已选集合变化 = 需要重新确认（多选点的确认覆盖整组选项）。
        selection.confirmed_by_user = false;
        self.invalidate_red_team();
        Ok(())
    }

    /// 多选点移除一个已选选项。
    ///
    /// 移除首选项时把下一个已选选项提为首选项（`option_id` 字段）；移除的是主选则清空主选；
    /// 只剩一个选项时拒绝（整点撤销请用 `clear_selection`，语义不同）。
    pub fn remove_option(&mut self, decision_id: &str, option_id: &str) -> Adm4Result<()> {
        let point = self.space.graph.require_point(decision_id)?;
        if !point.is_multi() {
            return Err(Adm4Error::invalid_input(format!(
                "决策点 {decision_id} 是单选点，不能移除单个选项（整点撤销请用 clear_selection）"
            )));
        }
        let selection = self
            .state
            .selections
            .get_mut(decision_id)
            .ok_or_else(|| Adm4Error::not_found(format!("决策点 {decision_id} 未选择")))?;
        if !selection.contains_option(option_id) {
            return Err(Adm4Error::not_found(format!(
                "决策点 {decision_id} 未选中 {option_id}"
            )));
        }
        if selection.selected_count() == 1 {
            return Err(Adm4Error::conflict(format!(
                "决策点 {decision_id} 只剩一个已选选项：整点撤销请用 clear_selection"
            )));
        }
        if selection.option_id == option_id {
            let promoted = selection.additional_options.remove(0);
            selection.option_id = promoted.option_id;
            selection.parameters = promoted.parameters;
            selection.rationale = promoted.rationale;
            selection.template_original = promoted.template_original;
        } else {
            selection
                .additional_options
                .retain(|extra| extra.option_id != option_id);
        }
        if selection.primary_option.as_deref() == Some(option_id) {
            selection.primary_option = None;
        }
        selection.confirmed_by_user = false;
        self.invalidate_red_team();
        Ok(())
    }

    /// 标记多选点的主选（必须是 `allow_primary` 的多选点，且该选项已在已选集合内）。
    pub fn set_primary_option(&mut self, decision_id: &str, option_id: &str) -> Adm4Result<()> {
        let point = self.space.graph.require_point(decision_id)?;
        if !point.requires_primary() {
            return Err(Adm4Error::invalid_input(format!(
                "决策点 {decision_id} 未开启 allow_primary，不接受主选标记"
            )));
        }
        let selection = self
            .state
            .selections
            .get_mut(decision_id)
            .ok_or_else(|| Adm4Error::not_found(format!("决策点 {decision_id} 未选择")))?;
        if !selection.contains_option(option_id) {
            return Err(Adm4Error::conflict(format!(
                "主选 {option_id} 必须先进入 {decision_id} 的已选集合"
            )));
        }
        selection.primary_option = Some(option_id.to_string());
        self.invalidate_red_team();
        Ok(())
    }

    /// 设置多选点上**某个已选选项**的参数（单选点等价于 `set_parameters`）。
    /// 与 `set_parameters` 一致：校验失败仍保存并返回问题清单（进待填清单）。
    pub fn set_option_parameters(
        &mut self,
        decision_id: &str,
        option_id: &str,
        parameters: ParameterValues,
    ) -> Adm4Result<Vec<String>> {
        let point = self.space.graph.require_point(decision_id)?;
        let selection = self
            .state
            .selections
            .get(decision_id)
            .ok_or_else(|| {
                Adm4Error::conflict(format!("决策点 {decision_id} 尚未选择选项，不能填参数"))
            })?
            .clone();
        if !selection.contains_option(option_id) {
            return Err(Adm4Error::not_found(format!(
                "决策点 {decision_id} 未选中 {option_id}，不能填它的参数"
            )));
        }
        let problems = validate_option_parameters(
            &self.space.graph,
            &self.state.selections,
            point,
            option_id,
            &parameters,
            self.space.cardinality(),
        );
        let stored = self.state.selections.get_mut(decision_id).ok_or_else(|| {
            Adm4Error::internal(format!("决策点 {decision_id} 的选择记录在写参数过程中消失"))
        })?;
        if stored.option_id == option_id {
            stored.parameters = parameters;
        } else {
            for extra in &mut stored.additional_options {
                if extra.option_id == option_id {
                    extra.parameters = parameters;
                    break;
                }
            }
        }
        self.invalidate_red_team();
        Ok(problems)
    }

    /// requires 满足性 + conflicts 硬拦截（多选点按「已选集合是否包含」判定）。
    fn check_option_dependencies(&self, decision_id: &str, option_id: &str) -> Adm4Result<()> {
        let point = self.space.graph.require_point(decision_id)?;
        let option = point.option(option_id).ok_or_else(|| {
            Adm4Error::not_found(format!("决策点 {decision_id} 无选项 {option_id}"))
        })?;
        for required in &option.requires {
            if !self.option_selected(&required.decision, &required.option) {
                return Err(Adm4Error::conflict(format!(
                    "选择 {decision_id}/{option_id} 需要先选 {}/{}",
                    required.decision, required.option
                )));
            }
        }
        for conflict in &option.conflicts {
            if self.option_selected(&conflict.decision, &conflict.option) {
                return Err(Adm4Error::conflict(format!(
                    "选择 {decision_id}/{option_id} 与已选 {}/{} 冲突",
                    conflict.decision, conflict.option
                )));
            }
        }
        Ok(())
    }

    /// 某决策点的已选集合是否包含指定选项（多选点看全集，不只看首选项）。
    fn option_selected(&self, decision_id: &str, option_id: &str) -> bool {
        self.state
            .selections
            .get(decision_id)
            .is_some_and(|selection| selection.contains_option(option_id))
    }

    /// 设置参数（整表/整组替换），按 schema 校验。校验失败仍保存但返回问题清单（进待填清单）。
    pub fn set_parameters(
        &mut self,
        decision_id: &str,
        parameters: ParameterValues,
    ) -> Adm4Result<Vec<String>> {
        let point = self.space.graph.require_point(decision_id)?;
        let selection = self.state.selections.get(decision_id).ok_or_else(|| {
            Adm4Error::conflict(format!("决策点 {decision_id} 尚未选择选项，不能填参数"))
        })?;
        let mut updated = selection.clone();
        updated.parameters = parameters;
        let problems = validate_parameters(
            &self.space.graph,
            &self.state.selections,
            point,
            &updated,
            self.space.cardinality(),
        );
        self.state
            .selections
            .insert(decision_id.to_string(), updated);
        self.invalidate_red_team();
        Ok(problems)
    }

    pub fn set_rationale(&mut self, decision_id: &str, rationale: &str) -> Adm4Result<()> {
        let selection = self
            .state
            .selections
            .get_mut(decision_id)
            .ok_or_else(|| Adm4Error::not_found(format!("决策点 {decision_id} 未选择")))?;
        selection.rationale = rationale.to_string();
        self.state.bump_revision();
        Ok(())
    }

    /// 用户确认（AI 提案/模板预填 → 计入完成度的唯一途径）。
    pub fn confirm_selection(&mut self, decision_id: &str) -> Adm4Result<()> {
        let selection = self
            .state
            .selections
            .get_mut(decision_id)
            .ok_or_else(|| Adm4Error::not_found(format!("决策点 {decision_id} 未选择")))?;
        selection.confirmed_by_user = true;
        self.state.bump_revision();
        Ok(())
    }

    /// baseline 点显式 N/A（结构化理由码；不带署名）。
    ///
    /// 这是设计 01 号文档 §2.4 的 baseline 跳过通道，只对 `requirement=Baseline` 的点开放。
    /// 把任意适用点判为不适用请用 `set_not_applicable`（强制理由 + 说明 + 署名）。
    pub fn mark_not_applicable(
        &mut self,
        decision_id: &str,
        justification: NaJustification,
    ) -> Adm4Result<()> {
        let point = self.space.graph.require_point(decision_id)?;
        if point.requirement != PointRequirement::Baseline {
            return Err(Adm4Error::invalid_input(format!(
                "决策点 {decision_id} 不是 baseline 点，不支持理由码跳过；人工豁免请用 set_not_applicable（需理由与署名）"
            )));
        }
        if justification.reason_code.trim().is_empty() {
            return Err(Adm4Error::invalid_input("N/A 必须提供结构化理由码"));
        }
        if justification.is_signed() {
            return Err(Adm4Error::invalid_input(
                "baseline 理由码跳过不带署名；需要署名的人工豁免请用 set_not_applicable",
            ));
        }
        self.state.selections.remove(decision_id);
        self.state
            .not_applicable
            .insert(decision_id.to_string(), justification);
        self.invalidate_red_team();
        Ok(())
    }

    /// 人工豁免：把任意决策点（含普通适用点）标记为不适用。
    ///
    /// R3 要求可追责，因此理由码、说明、署名三者都必须非空——「不适用」是把一个点
    /// 移出完成度分母的唯一人工通道，说不出理由/找不到责任人的豁免不予受理。
    /// 豁免点不进分母但在案：完成度报告按理由码计数，冻结门第 1 道逐条列出（不拦截）。
    pub fn set_not_applicable(
        &mut self,
        decision_id: &str,
        reason_code: &str,
        note: &str,
        actor: &str,
    ) -> Adm4Result<()> {
        self.space.graph.require_point(decision_id)?;
        if reason_code.trim().is_empty() {
            return Err(Adm4Error::invalid_input(
                "人工豁免必须提供结构化理由码（reason_code）",
            ));
        }
        if note.trim().is_empty() {
            return Err(Adm4Error::invalid_input(
                "人工豁免必须写明理由说明（note）：不适用的判断要能被复核",
            ));
        }
        if actor.trim().is_empty() {
            return Err(Adm4Error::invalid_input(
                "人工豁免必须署名（actor）：R3 要求人工判断可追责",
            ));
        }
        self.state.selections.remove(decision_id);
        self.state.not_applicable.insert(
            decision_id.to_string(),
            NaJustification {
                reason_code: reason_code.trim().to_string(),
                note: note.trim().to_string(),
                actor: actor.trim().to_string(),
                at: UtcTimestamp::now().to_iso8601(),
            },
        );
        self.invalidate_red_team();
        Ok(())
    }

    /// 解除不适用：该点恢复正常适用性判定（重新进分母，等待作答）。
    /// 未被标记时返回 false（幂等，不报错）。
    pub fn clear_not_applicable(&mut self, decision_id: &str) -> Adm4Result<bool> {
        self.space.graph.require_point(decision_id)?;
        if !self.state.not_applicable.contains_key(decision_id) {
            return Ok(false);
        }
        self.clear_na_records(decision_id);
        self.invalidate_red_team();
        Ok(true)
    }

    /// 节点级设计说明（二版节点文本）；空串 = 删除该条。
    pub fn set_node_design_note(&mut self, node_id: &str, note: &str) -> Adm4Result<()> {
        self.set_node_text(node_id, note, true)
    }

    /// 节点级风险说明（右栏「风险」页签数据源）；空串 = 删除该条。
    pub fn set_node_risk_note(&mut self, node_id: &str, note: &str) -> Adm4Result<()> {
        self.set_node_text(node_id, note, false)
    }

    fn set_node_text(&mut self, node_id: &str, note: &str, design: bool) -> Adm4Result<()> {
        if self.space.organization.node(node_id).is_none() {
            return Err(Adm4Error::not_found(format!(
                "节点 {node_id} 不在当前设计空间的节点清单内"
            )));
        }
        let target = if design {
            &mut self.state.node_design_notes
        } else {
            &mut self.state.node_risk_notes
        };
        if note.trim().is_empty() {
            target.remove(node_id);
        } else {
            target.insert(node_id.to_string(), note.to_string());
        }
        self.state.bump_revision();
        Ok(())
    }

    fn clear_na_records(&mut self, decision_id: &str) {
        // 署名已并入 NaJustification，删一处即全清（不会再留下幽灵署名）。
        self.state.not_applicable.remove(decision_id);
    }

    /// 清除选择：级联检查依赖本选项的其它选择。
    pub fn clear_selection(&mut self, decision_id: &str) -> Adm4Result<()> {
        let Some(removed) = self.state.selections.get(decision_id).cloned() else {
            return Ok(());
        };
        // 多选点两侧都要看全集：被清除点的任一已选选项，若是某个已选选项的前置，就不能清。
        let dependents: Vec<String> = self
            .state
            .selections
            .values()
            .filter(|selection| selection.decision_id != decision_id)
            .filter_map(|selection| {
                let point = self.space.graph.point(&selection.decision_id)?;
                let depends = selection.selected_options().into_iter().any(|item| {
                    point.option(item.option_id).is_some_and(|option| {
                        option.requires.iter().any(|required| {
                            required.decision == decision_id
                                && removed.contains_option(&required.option)
                        })
                    })
                });
                depends.then(|| selection.decision_id.clone())
            })
            .collect();
        if !dependents.is_empty() {
            return Err(Adm4Error::conflict(format!(
                "不能清除 {decision_id}：以下选择依赖它：{}",
                dependents.join(", ")
            )));
        }
        self.state.selections.remove(decision_id);
        self.invalidate_red_team();
        Ok(())
    }

    /// 模板预填（仅 Certified 模板；调用方通过 `TemplateLibrary::approved_for_prefill` 取得）。
    ///
    /// 包匹配规则：模板包与项目包相同，**或**模板是通用层模板（`genre_pack=universal`）——
    /// 通用层模板逆向的全是跨品类决策点，绑定单一品类包只会让 26 份内置模板全体不可用。
    ///
    /// 答卷里引用了装配空间中不存在的决策点/选项时**逐条跳过并计数在案**（R2：禁止静默丢弃），
    /// 结果落在 [`PrefillReport::skipped`] 里，由调用方展示/记日志。
    pub fn prefill_from_template(&mut self, template: &Template) -> Adm4Result<PrefillReport> {
        if !template.is_approved() {
            return Err(Adm4Error::blocked(format!(
                "模板 {} 未认证，不能预填",
                template.template_id
            )));
        }
        if template.genre_pack != self.state.genre_pack && !template.is_universal() {
            return Err(Adm4Error::conflict(format!(
                "模板品类包 {} 与项目 {} 不一致（只有 universal 模板可跨包预填）",
                template.genre_pack, self.state.genre_pack
            )));
        }
        let mut report = PrefillReport::default();
        for answer in &template.answers {
            let Some(point) = self.space.graph.point(&answer.decision_id) else {
                // 答卷针对旧版清单、或通用层模板在本包装配里找不到该点：跳过并计数。
                report.skipped.push(PrefillSkip {
                    decision_id: answer.decision_id.clone(),
                    option_id: answer.option_id.clone(),
                    reason: "决策点不在当前装配空间（通用层 + 本品类包）内".to_string(),
                });
                continue;
            };
            if point.option(&answer.option_id).is_none() {
                report.skipped.push(PrefillSkip {
                    decision_id: answer.decision_id.clone(),
                    option_id: answer.option_id.clone(),
                    reason: "首选项不在该决策点的选项集内".to_string(),
                });
                continue;
            }
            // 附加选项：单选点不接受，选项不存在则逐条跳过——两种情形都不静默丢弃。
            let mut additional_options = Vec::new();
            for extra in &answer.additional_options {
                if !point.is_multi() {
                    report.skipped.push(PrefillSkip {
                        decision_id: answer.decision_id.clone(),
                        option_id: extra.option_id.clone(),
                        reason: "决策点是单选点，附加选项无处安放".to_string(),
                    });
                    continue;
                }
                if point.option(&extra.option_id).is_none() {
                    report.skipped.push(PrefillSkip {
                        decision_id: answer.decision_id.clone(),
                        option_id: extra.option_id.clone(),
                        reason: "附加选项不在该决策点的选项集内".to_string(),
                    });
                    continue;
                }
                if extra.option_id == answer.option_id
                    || additional_options
                        .iter()
                        .any(|item: &SelectedOption| item.option_id == extra.option_id)
                {
                    report.skipped.push(PrefillSkip {
                        decision_id: answer.decision_id.clone(),
                        option_id: extra.option_id.clone(),
                        reason: "附加选项与已写入的选项重复".to_string(),
                    });
                    continue;
                }
                additional_options.push(SelectedOption {
                    option_id: extra.option_id.clone(),
                    parameters: extra.parameters.clone(),
                    rationale: String::new(),
                    template_original: Some(extra.parameters.clone()),
                });
            }
            // 主选：只在开了 allow_primary 且该选项确实被写入时才落，否则跳过并计数。
            let primary_option = match answer.primary_option.as_deref() {
                None => None,
                Some(primary) => {
                    let written = primary == answer.option_id
                        || additional_options
                            .iter()
                            .any(|item| item.option_id == primary);
                    if point.requires_primary() && written {
                        Some(primary.to_string())
                    } else {
                        report.skipped.push(PrefillSkip {
                            decision_id: answer.decision_id.clone(),
                            option_id: primary.to_string(),
                            reason: if written {
                                "该决策点未开启 allow_primary，主选标记无处安放".to_string()
                            } else {
                                "主选不在写入项目的已选集合内".to_string()
                            },
                        });
                        None
                    }
                }
            };
            report.multi_options_applied += additional_options.len();
            self.state.selections.insert(
                answer.decision_id.clone(),
                Selection {
                    decision_id: answer.decision_id.clone(),
                    option_id: answer.option_id.clone(),
                    parameters: answer.parameters.clone(),
                    rationale: format!("模板预填自 {}", template.game_name),
                    provenance: Provenance::Template {
                        template_id: template.template_id.clone(),
                    },
                    confirmed_by_user: false,
                    template_original: Some(answer.parameters.clone()),
                    additional_options,
                    primary_option,
                },
            );
            report.applied += 1;
        }
        self.state.template_mode = TemplateMode::Prefilled {
            template_id: template.template_id.clone(),
        };
        self.invalidate_red_team();
        Ok(report)
    }

    /// 工作台重置：清空创作选择，保留项目与已冻结历史（二版 `reset-design-workbench`）。
    ///
    /// **清空**：全部决策点选择（连带参数值、选择理由、附加多选选项与主选标记）、
    /// N/A 豁免与理由码跳过、节点设计说明与风险说明、模板模式标记——创作态回到初始
    /// 未作答状态。模板模式一并清掉是因为它描述的是「本项目由模板 X 预填而来」，
    /// 而那些预填选择此刻已经不存在了，留着就是一句假话。
    ///
    /// **保留**：项目 id 与名称、`frozen_versions`（已冻结版本是只增不改的历史，D4：
    /// 重置抹掉它等于伪造项目从没冻结过）、流水线产物、模板库、运行日志。
    /// 访谈 transcript 与红队记录也保留：前者是审计流，后者随 `revision` 前移自动标记过期，
    /// 冻结门第 4 道会要求重跑，不会被误当作对新设计的评审。
    ///
    /// R3：`actor` 与 `note` 双必填——重置是破坏性操作，说不出是谁按的、为什么按，不予受理。
    pub fn reset_workbench(&mut self, actor: &str, note: &str) -> Adm4Result<WorkbenchResetReport> {
        let actor = actor.trim();
        let note = note.trim();
        if actor.is_empty() {
            return Err(Adm4Error::invalid_input(
                "工作台重置必须署名（actor）：R3 要求破坏性操作可追责",
            ));
        }
        if note.is_empty() {
            return Err(Adm4Error::invalid_input(
                "工作台重置必须写明理由（note）：清空全部选择的决定要能被复核",
            ));
        }
        let mut report = WorkbenchResetReport {
            cleared_selections: self.state.selections.len(),
            cleared_exemptions: self.state.not_applicable.len(),
            cleared_node_design_notes: self.state.node_design_notes.len(),
            cleared_node_risk_notes: self.state.node_risk_notes.len(),
            actor: actor.to_string(),
            at: UtcTimestamp::now().to_iso8601(),
            ..Default::default()
        };
        for selection in self.state.selections.values() {
            if selection.primary_option.is_some() {
                report.cleared_primary_marks += 1;
            }
            if !matches!(selection.parameters, ParameterValues::None) {
                report.cleared_parameter_values += 1;
            }
        }
        self.state.selections.clear();
        self.state.not_applicable.clear();
        self.state.node_design_notes.clear();
        self.state.node_risk_notes.clear();
        self.state.template_mode = TemplateMode::None;
        self.invalidate_red_team();
        Ok(report)
    }

    /// 项目名规范化：去首尾空白，全空白即拒。
    ///
    /// 单独暴露是为了让门面在开事务**之前**就拿到规范化后的名字（存档 manifest 的展示名
    /// 要与创作状态里的名字逐字一致，不能一个 trim 过一个没 trim）。
    pub fn normalize_project_name(project_name: &str) -> Adm4Result<String> {
        let trimmed = project_name.trim();
        if trimmed.is_empty() {
            return Err(Adm4Error::invalid_input(
                "项目名不能为空白（重命名必须给出可读名称）",
            ));
        }
        Ok(trimmed.to_string())
    }

    /// 项目重命名：非空白校验后写入创作状态（存档 manifest 的展示名由调用方同步）。
    pub fn set_project_name(&mut self, project_name: &str) -> Adm4Result<()> {
        self.state.project_name = Self::normalize_project_name(project_name)?;
        self.state.bump_revision();
        Ok(())
    }

    /// 当前激活但未完成的决策点（拓扑序），访谈游标与 UI 待办共用。
    ///
    /// 未作答的非必做点不进待办（口径与完成度分母一致）：它们不是「欠着的活」，
    /// 访谈不追问，用户想填就在工作台检查单里直接选。
    pub fn pending_decisions(&self) -> Adm4Result<Vec<DecisionId>> {
        let applicability = self.applicability();
        let order = self.space.graph.topological_order()?;
        Ok(order
            .into_iter()
            .filter(|id| {
                let Some(point) = self.space.graph.point(id) else {
                    return false;
                };
                counts_toward_completeness(point, &applicability, &self.state.selections)
                    && !self
                        .state
                        .selections
                        .get(id)
                        .is_some_and(|selection| selection.confirmed_by_user)
            })
            .collect())
    }

    /// 首选项 id（多选点为「主选优先」序列的第一项以外的语义请用 `selected_option_ids`）。
    pub fn selected_option_id(&self, decision_id: &str) -> Option<&OptionId> {
        self.state
            .selections
            .get(decision_id)
            .map(|selection| &selection.option_id)
    }

    /// 全部已选选项 id（主选在前）；未选择时返回空。
    pub fn selected_option_ids(&self, decision_id: &str) -> Vec<String> {
        match self.state.selections.get(decision_id) {
            None => Vec::new(),
            Some(selection) => selection
                .selected_option_ids()
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    /// 访谈状态可变访问（访谈记录不参与完成度/门禁判定，不影响 revision）。
    pub(crate) fn interview_mut(&mut self) -> &mut crate::state::InterviewState {
        &mut self.state.interview
    }

    pub fn record_red_team(&mut self, record: crate::state::RedTeamRecord) {
        self.state.red_team = Some(record);
    }

    pub fn mark_frozen(&mut self) {
        self.state.frozen_versions += 1;
    }

    /// 设计变更使红队记录过期（revision 前移）。
    fn invalidate_red_team(&mut self) {
        self.state.bump_revision();
    }
}
