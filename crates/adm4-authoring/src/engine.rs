use crate::state::{AuthoringState, TemplateMode};
use adm4_decision::{
    ApplicabilityMap, CompletenessReport, DecisionId, NaJustification, OptionId, ParameterValues,
    PointApplicability, PointRequirement, Provenance, Selection, compute_applicability,
    compute_completeness, validate_parameters,
};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_space::DesignSpace;
use adm4_template::Template;

/// 创作引擎：装配设计空间 + 项目状态，提供全部经校验的变更操作。
pub struct AuthoringEngine {
    space: DesignSpace,
    state: AuthoringState,
}

impl AuthoringEngine {
    pub fn new(space: DesignSpace, state: AuthoringState) -> Adm4Result<Self> {
        if state.genre_pack != space.pack.pack_id {
            return Err(Adm4Error::conflict(format!(
                "项目品类包 {} 与加载的设计空间 {} 不一致",
                state.genre_pack, space.pack.pack_id
            )));
        }
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

    /// 选择选项：requires 满足性 + conflicts 硬拦截。
    pub fn select_option(
        &mut self,
        decision_id: &str,
        option_id: &str,
        provenance: Provenance,
    ) -> Adm4Result<()> {
        let point = self.space.graph.require_point(decision_id)?;
        let option = point.option(option_id).ok_or_else(|| {
            Adm4Error::not_found(format!("决策点 {decision_id} 无选项 {option_id}"))
        })?;

        for required in &option.requires {
            let satisfied = self
                .state
                .selections
                .get(&required.decision)
                .is_some_and(|selection| selection.option_id == required.option);
            if !satisfied {
                return Err(Adm4Error::conflict(format!(
                    "选择 {decision_id}/{option_id} 需要先选 {}/{}",
                    required.decision, required.option
                )));
            }
        }
        for conflict in &option.conflicts {
            let conflicted = self
                .state
                .selections
                .get(&conflict.decision)
                .is_some_and(|selection| selection.option_id == conflict.option);
            if conflicted {
                return Err(Adm4Error::conflict(format!(
                    "选择 {decision_id}/{option_id} 与已选 {}/{} 冲突",
                    conflict.decision, conflict.option
                )));
            }
        }

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
            },
        );
        self.state.not_applicable.remove(decision_id);
        self.invalidate_red_team();
        Ok(())
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

    /// baseline 点显式 N/A（结构化理由码）。
    pub fn mark_not_applicable(
        &mut self,
        decision_id: &str,
        justification: NaJustification,
    ) -> Adm4Result<()> {
        let point = self.space.graph.require_point(decision_id)?;
        if point.requirement != PointRequirement::Baseline {
            return Err(Adm4Error::invalid_input(format!(
                "决策点 {decision_id} 不是 baseline 点，不支持显式 N/A（未激活的点自然不进分母）"
            )));
        }
        if justification.reason_code.trim().is_empty() {
            return Err(Adm4Error::invalid_input("N/A 必须提供结构化理由码"));
        }
        self.state.selections.remove(decision_id);
        self.state
            .not_applicable
            .insert(decision_id.to_string(), justification);
        self.invalidate_red_team();
        Ok(())
    }

    /// 清除选择：级联检查依赖本选项的其它选择。
    pub fn clear_selection(&mut self, decision_id: &str) -> Adm4Result<()> {
        let Some(removed) = self.state.selections.get(decision_id).cloned() else {
            return Ok(());
        };
        let dependents: Vec<String> = self
            .state
            .selections
            .values()
            .filter(|selection| selection.decision_id != decision_id)
            .filter_map(|selection| {
                let point = self.space.graph.point(&selection.decision_id)?;
                let option = point.option(&selection.option_id)?;
                option
                    .requires
                    .iter()
                    .any(|required| {
                        required.decision == decision_id && required.option == removed.option_id
                    })
                    .then(|| selection.decision_id.clone())
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

    /// 模板预填（仅 Approved 模板；调用方通过 TemplateLibrary::approved_for_prefill 取得）。
    pub fn prefill_from_template(&mut self, template: &Template) -> Adm4Result<usize> {
        if !template.is_approved() {
            return Err(Adm4Error::blocked(format!(
                "模板 {} 未认证，不能预填",
                template.template_id
            )));
        }
        if template.genre_pack != self.state.genre_pack {
            return Err(Adm4Error::conflict(format!(
                "模板品类包 {} 与项目 {} 不一致",
                template.genre_pack, self.state.genre_pack
            )));
        }
        let mut applied = 0;
        for answer in &template.answers {
            let Some(point) = self.space.graph.point(&answer.decision_id) else {
                continue; // 答卷针对旧版清单的条目：跳过并由 coverage 呈现。
            };
            if point.option(&answer.option_id).is_none() {
                continue;
            }
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
                },
            );
            applied += 1;
        }
        self.state.template_mode = TemplateMode::Prefilled {
            template_id: template.template_id.clone(),
        };
        self.invalidate_red_team();
        Ok(applied)
    }

    /// 当前激活但未完成的决策点（拓扑序），访谈游标与 UI 待办共用。
    pub fn pending_decisions(&self) -> Adm4Result<Vec<DecisionId>> {
        let applicability = self.applicability();
        let order = self.space.graph.topological_order()?;
        Ok(order
            .into_iter()
            .filter(|id| {
                matches!(applicability.get(id), Some(PointApplicability::Active))
                    && !self
                        .state
                        .selections
                        .get(id)
                        .is_some_and(|selection| selection.confirmed_by_user)
            })
            .collect())
    }

    pub fn selected_option_id(&self, decision_id: &str) -> Option<&OptionId> {
        self.state
            .selections
            .get(decision_id)
            .map(|selection| &selection.option_id)
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
