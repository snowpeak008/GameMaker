//! 概念访谈（T-W7-3d ①）：用户口述游戏想法 → AI 产出结构化提案 →
//! 用户确认后落盘（项目私有 SystemRef 集合 + tier 声明 + core_loop 动词序列）。
//!
//! 与既有逐点访谈同一纪律：**AI 只提案，确认是用户手势**（D11）——本文件全部是
//! 纯函数（提案构造/解析/校验/理清），不写任何状态；落盘由 `AppServices`
//! 的确认入口做（用户触发）。AI 输出必须结构化可校验，越界（发明模块 id/
//! 档位 id/κ 值/悬空动词绑定）即 Err 不吞（R7）。
//!
//! 三个嗅探（定稿 §9.2b 第 2 条 + §4.2(c) 改制）：
//! - **大战略嗅探**：提案的重核候选（建议档 W≥9 且 κ∈{core,strong}）数量 >4 →
//!   切「逐重核轻重理清」模式：只提示不设阻（总体形态由用户自行设计，署名确认走
//!   既有 `compose_confirm_form`），但设计火力转入逐重核系统——每个重核候选必须
//!   经 `clarify_tier` 落档位声明与 rationale 后才允许确认（宁多问一句，不落半案）；
//! - **融合型嗅探**："X+Y"口述 → AI 给双核并集分解（两核系统清单并集 + 嵌套
//!   core_loop + 跨核转换说明），`fusion` 字段承载，实例引用逐一校验；
//! - **库外系统**：模块库覆盖不到的系统如实标注进 `library_external`（名称+说明），
//!   不发明模块 id、不静默丢弃（R2）。

use crate::state::CoreLoopVerb;
use adm4_ai::{AiProvider, AiRequest};
use adm4_decision::system_module::{CoreLink, SystemModule};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_space::{DesignSpace, SystemRef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 概念访谈 AI 调用的 purpose 键（Scripted 脚本按此回放）。
pub const PURPOSE_CONCEPT: &str = "interview_concept";
/// 逐重核档位理清 AI 调用的 purpose 键。
pub const PURPOSE_CONCEPT_TIER: &str = "interview_concept_tier";

/// 大战略嗅探的重核候选数量阈值（定稿 §4.2(c)：|H|>4 切逐重核理清模式）。
pub const HEAVY_CORE_THRESHOLD: usize = 4;

/// 逐重核理清的规范问句（定稿 §9.2b 用户裁决原话的落地形态；CLI/桌面展示用）。
pub fn tier_question(instance_label: &str) -> String {
    format!("{instance_label} 这个系统你要轻度还是重度？轻重的判断依据（对标哪款游戏的哪个系统）？")
}

/// 提案中的一个系统（从模块库选出的实例草案）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConceptSystem {
    /// 实例 id（`[a-z0-9_]`，确认后成为决策点命名空间锚）。
    pub instance_id: String,
    /// 引用的模块 id（必须在模块表内——发明即 Err）。
    pub module_id: String,
    /// AI 建议的重度档（必须在该模块阶梯内——发明即 Err）。
    pub suggested_tier: String,
    /// 与核心循环的关联强度 κ 建议。
    pub core_link: CoreLink,
    /// 建议理由（落 tier 合成点的 rationale，除非理清覆盖）。
    pub rationale: String,
    /// 名词绑定（AI 可给可不给；缺省由 `resolve_bindings` 确定性推导补全）。
    pub noun_bindings: BTreeMap<String, String>,
}

/// 库外系统的如实标注（模块库覆盖不到，不发明模块 id）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ExternalSystemNote {
    pub name: String,
    /// 说明（该系统做什么、为何库内无对应模块、后续走系统级 custom 的方向提示）。
    pub note: String,
}

/// 融合型分解的一个核（"X+Y"口述里的 X 或 Y）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FusionCore {
    /// 核名（如「合成大西瓜」「塔防」）。
    pub label: String,
    /// 归属本核的系统实例 id（须是 `systems` 内的实例——悬空即 Err）。
    pub instance_ids: Vec<String>,
}

/// 融合型嗅探产物：双核并集分解（两核系统清单并集 + 跨核转换说明）。
///
/// 嵌套 core_loop 由外层 `ConceptProposal.core_loop` 表达（动词按核交替排列），
/// 本结构记录核的划分与转换语义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FusionDecomposition {
    pub cores: Vec<FusionCore>,
    /// 跨核转换说明（玩家何时从 X 核切到 Y 核、什么资源/信号衔接）。
    pub transition: String,
}

/// 逐重核理清的一条记录：AI 按用户回答落的档位建议 + rationale。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TierClarification {
    /// 理清后的档位 id（必须在该实例模块的阶梯内）。
    pub tier_id: String,
    /// 档位理由（对标游戏与轻重判断依据，落 tier 合成点 rationale）。
    pub rationale: String,
    /// 用户的原话回答（R3 留痕素材，随提案走、确认时进 transcript）。
    pub user_answer: String,
}

/// 概念访谈提案（AI 产出 + 解析校验后的结构化形态）。
///
/// 与逐点访谈的 `InterviewProposal` 同一传递纪律：提案由调用方持有并
/// **原样传回**确认入口——服务端不缓存提案，防止「确认的不是用户看到的」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConceptProposal {
    /// 系统清单（模块库实例草案）。
    pub systems: Vec<ConceptSystem>,
    /// 库外系统如实标注（不发明模块 id）。
    pub library_external: Vec<ExternalSystemNote>,
    /// core_loop 动词序列草案（动词 → 实例绑定；确认后落 AuthoringState.core_loop）。
    pub core_loop: Vec<CoreLoopVerb>,
    /// 融合型分解（"X+Y"口述时非空）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fusion: Option<FusionDecomposition>,
    /// AI 的整体说明。
    pub notes: String,
    /// 大战略嗅探：重核候选（建议档 W≥9 且 κ∈{core,strong} 的实例 id，字典序）。
    /// 在提案时刻按 `suggested_tier` 计算并冻结——理清降档不缩减本清单
    /// （模式是否触发是提案时刻的事实，理清义务覆盖全部候选）。
    pub heavy_core_candidates: Vec<String>,
    /// 重核候选 > 4 → 逐重核轻重理清模式（提示不设阻；确认前候选必须全部理清）。
    pub per_heavy_core_mode: bool,
    /// 逐重核理清记录（键 = 实例 id）。
    pub tier_clarifications: BTreeMap<String, TierClarification>,
    /// 提示清单（超大玩法规模事实、署名确认指路等——只提示，不拦）。
    pub hints: Vec<String>,
}

impl ConceptProposal {
    /// 某实例确认时生效的档位：理清记录优先，否则 AI 建议档。
    pub fn effective_tier(&self, instance_id: &str) -> Option<(&str, &str)> {
        if let Some(clarification) = self.tier_clarifications.get(instance_id) {
            return Some((&clarification.tier_id, &clarification.rationale));
        }
        self.systems
            .iter()
            .find(|system| system.instance_id == instance_id)
            .map(|system| (system.suggested_tier.as_str(), system.rationale.as_str()))
    }

    /// 确认前置校验：逐重核模式下全部重核候选必须已理清（档位 + rationale 在案）。
    pub fn validate_for_confirm(&self) -> Adm4Result<()> {
        if self.systems.is_empty() {
            return Err(Adm4Error::invalid_input(
                "概念提案不含任何系统实例，无可确认内容",
            ));
        }
        if !self.per_heavy_core_mode {
            return Ok(());
        }
        let unclarified: Vec<&str> = self
            .heavy_core_candidates
            .iter()
            .filter(|id| !self.tier_clarifications.contains_key(*id))
            .map(String::as_str)
            .collect();
        if unclarified.is_empty() {
            Ok(())
        } else {
            Err(Adm4Error::blocked(format!(
                "逐重核轻重理清模式：还有 {} 个重核系统未理清档位（{}）。\
                 请对每个重核系统回答「轻度还是重度？对标哪款游戏的哪个系统？」\
                 （interview concept-clarify），落档位声明与理由后再确认。",
                unclarified.len(),
                unclarified.join("、")
            )))
        }
    }
}

/// 概念访谈提案入口（纯函数）：口述想法 + 模块库 + 当前空间 → 结构化提案。
///
/// AI 输出解析失败/发明模块 id/发明档位/动词绑定悬空 → Err 即停（R7 不吞）。
pub fn propose_concept(
    space: &DesignSpace,
    modules: &BTreeMap<String, SystemModule>,
    provider: &dyn AiProvider,
    pitch: &str,
) -> Adm4Result<ConceptProposal> {
    if pitch.trim().is_empty() {
        return Err(Adm4Error::invalid_input(
            "概念访谈需要用户口述游戏想法（pitch 不可为空）",
        ));
    }
    let request = build_concept_request(space, modules, pitch);
    let response = provider.invoke(&request)?;
    parse_concept_proposal(space, modules, &response.text)
}

/// 逐重核档位理清入口（纯函数）：用户对轻重问句的回答 → AI 落档位建议 + rationale。
///
/// 返回**更新后的提案**（原提案 + 该实例的理清记录）——提案原样传回的纪律下，
/// 更新由调用方持有，确认时一并进落盘入口。
pub fn clarify_tier(
    modules: &BTreeMap<String, SystemModule>,
    provider: &dyn AiProvider,
    mut proposal: ConceptProposal,
    instance_id: &str,
    user_answer: &str,
) -> Adm4Result<ConceptProposal> {
    let system = proposal
        .systems
        .iter()
        .find(|system| system.instance_id == instance_id)
        .ok_or_else(|| {
            Adm4Error::not_found(format!(
                "提案中没有实例 {instance_id}（可理清的实例：{}）",
                proposal
                    .systems
                    .iter()
                    .map(|system| system.instance_id.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            ))
        })?;
    if user_answer.trim().is_empty() {
        return Err(Adm4Error::invalid_input(
            "档位理清需要用户回答（轻重需求与对标游戏），不可为空",
        ));
    }
    let module = require_module(modules, &system.module_id)?;
    let ladder_text = module
        .heaviness
        .tiers
        .iter()
        .map(|tier| {
            format!(
                "- id={} label={} W={}：{}",
                tier.id,
                tier.label_zh,
                tier.rating.total(),
                tier.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let request = AiRequest {
        purpose: PURPOSE_CONCEPT_TIER.into(),
        system_prompt: "你是游戏设计访谈助手，正在做逐重核系统的轻重需求理清。\
                        根据用户对「轻度还是重度？对标哪款游戏？」的回答，从给出的档位阶梯中\
                        选一个档位并给出理由（理由须引用用户的对标与判断依据）。\
                        只能选给出的档位 id，不得发明。输出 JSON：{\"tier_id\":..., \"rationale\":...}。\
                        你提出的是建议，最终决定权在用户。"
            .into(),
        user_prompt: format!(
            "系统实例 {instance_id}（模块 {}：{}）。\n问句：{}\n用户回答：{}\n\n可选档位阶梯：\n{ladder_text}",
            module.module_id,
            module.label_zh,
            tier_question(&module.label_zh),
            user_answer.trim()
        ),
        expect_json: true,
    };
    let response = provider.invoke(&request)?;
    let value: serde_json::Value = serde_json::from_str(response.text.trim()).map_err(|error| {
        Adm4Error::validation(format!(
            "档位理清的 AI 应答不是合法 JSON：{error}；原文：{}",
            response.text
        ))
    })?;
    let tier_id = value
        .get("tier_id")
        .and_then(|item| item.as_str())
        .ok_or_else(|| Adm4Error::validation("档位理清的 AI 应答缺少 tier_id"))?
        .to_string();
    if module.heaviness.tier_rank(&tier_id).is_none() {
        return Err(Adm4Error::validation(format!(
            "档位理清的 AI 应答发明了档位 {tier_id}（不在模块 {} 的阶梯内，非法输出即停）",
            module.module_id
        )));
    }
    let rationale = value
        .get("rationale")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if rationale.is_empty() {
        return Err(Adm4Error::validation(
            "档位理清必须携带 rationale（轻重判断依据与对标是理清的目的，缺失即无效理清）",
        ));
    }
    proposal.tier_clarifications.insert(
        instance_id.to_string(),
        TierClarification {
            tier_id,
            rationale,
            user_answer: user_answer.trim().to_string(),
        },
    );
    Ok(proposal)
}

/// 确认落盘用：把提案系统转成 `SystemRef` 序列（绑定已在解析期补全校验）。
pub fn proposal_to_refs(proposal: &ConceptProposal) -> Vec<SystemRef> {
    proposal
        .systems
        .iter()
        .map(|system| SystemRef {
            instance_id: system.instance_id.clone(),
            module_id: system.module_id.clone(),
            version_req: String::new(),
            allowed_tiers: Vec::new(),
            noun_bindings: system.noun_bindings.clone(),
            core_link: system.core_link,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 请求构造与解析（内部）
// ---------------------------------------------------------------------------

/// 已占用实例 id（字典序）：pack 既有引用 + 项目私有引用（装配时已并入 pack）。
/// 提示词注入与拒收文案共用同一数据源，保证 AI 看到的与校验拦的完全一致。
fn occupied_instance_ids(space: &DesignSpace) -> Vec<String> {
    space
        .pack
        .system_refs
        .iter()
        .map(|reference| reference.instance_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn require_module<'a>(
    modules: &'a BTreeMap<String, SystemModule>,
    module_id: &str,
) -> Adm4Result<&'a SystemModule> {
    modules.get(module_id).ok_or_else(|| {
        Adm4Error::validation(format!(
            "AI 提案引用的模块 {module_id} 不在模块库内（发明模块 id 被拒绝；\
             库外系统应如实标注进 library_external）"
        ))
    })
}

fn build_concept_request(
    space: &DesignSpace,
    modules: &BTreeMap<String, SystemModule>,
    pitch: &str,
) -> AiRequest {
    let catalog = modules
        .values()
        .map(|module| {
            let tiers = module
                .heaviness
                .tiers
                .iter()
                .map(|tier| format!("{}(W{})", tier.id, tier.rating.total()))
                .collect::<Vec<_>>()
                .join("/");
            format!(
                "- module_id={} label={} 档位：{tiers} provides={} consumes={} modifies={}",
                module.module_id,
                module.label_zh,
                module.interface.provides.join(","),
                module.interface.consumes.join(","),
                module.interface.modifies.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    // 已占用实例 id 清单（pack 既有 + 概念确认落盘的项目私有引用——loader 装配时
    // 已把私有引用 extend 进 pack.system_refs，此处取 pack 即全覆盖）。
    // 波 6 前置缺口 A：AI 自主命名实例时看不到这份清单必然撞 id（撞了校验必拒），
    // 注入清单 + 硬约束让 AI 一次给出可落盘的命名（校验防线原样保留）。
    let occupied = occupied_instance_ids(space);
    let existing = space
        .pack
        .system_refs
        .iter()
        .map(|reference| format!("{}（模块 {}）", reference.instance_id, reference.module_id))
        .collect::<Vec<_>>()
        .join("、");
    AiRequest {
        purpose: PURPOSE_CONCEPT.into(),
        system_prompt: "你是游戏设计访谈助手，做概念访谈：把用户口述的游戏想法分解为\
             系统清单 + 每系统建议重度档 + core_loop 动词序列草案。\
             系统只能从给出的模块库选（module_id/档位 id 不得发明）；库内没有的系统\
             如实写进 library_external（名称+说明），不要硬套。\
             硬约束：systems 只列**新增**实例——「既有系统实例」已在项目里，不要把\
             它们重新列进 systems（想法里已被既有实例覆盖的部分直接省略）；新实例的\
             instance_id 不得与「已占用实例 id 清单」里的任何 id 重复（重复会被系统\
             直接拒收）——撞名时换一个语义等价的名字（如加用途后缀）。\
             core_loop 动词可以绑定既有实例的 id（引用既有实例是允许的，重新定义才不允许）。\
             名词绑定：模块 consumes 的名词若提案内没有实例 provides（尤其玩家输入类，\
             如 sys.player_input.command_intent），必须在 noun_bindings 里显式绑定到\
             「pack 核心名词」清单中的名词（如 player_command_intent），否则会被拒收。\
             识别到「X+Y」融合型口述时，给出双核并集分解（fusion 字段：两核系统清单\
             并集 + 跨核转换说明），core_loop 按嵌套循环排列动词。\
             输出 JSON：{\"systems\":[{\"instance_id\":小写下划线,\"module_id\":...,\
             \"suggested_tier\":...,\"core_link\":\"core|strong|weak|meta\",\
             \"rationale\":...,\"noun_bindings\":{可选}}],\
             \"library_external\":[{\"name\":...,\"note\":...}],\
             \"core_loop\":[{\"verb\":...,\"instance_id\":...}],\
             \"fusion\":{\"cores\":[{\"label\":...,\"instance_ids\":[...]}],\
             \"transition\":...}（非融合型省略）,\"notes\":...}。\
             你提出的是建议，最终决定权在用户。"
            .into(),
        user_prompt: format!(
            "用户口述的游戏想法：\n{pitch}\n\n可选模块库：\n{catalog}\n\n\
             pack 核心名词（绑定可指向）：{}\n{}既有系统实例：{}\n\
             已占用实例 id 清单（新实例 id 不得与其中任何一个重复）：{}",
            space.pack.core_nouns.join("、"),
            // 输入类名词的绑定示例用 pack 实际核心名词生成（提示词层修复，缺口 B 语料侧）。
            space
                .pack
                .core_nouns
                .first()
                .map(|noun| {
                    format!(
                        "绑定示例：所选模块 consumes 的名词若提案内无实例 provides\
                         （如 sys.player_input.command_intent 这类玩家输入），必须在该实例的\
                         noun_bindings 里显式绑到核心名词，例如 \
                         {{\"sys.player_input.command_intent\":\"{noun}\"}}。\n"
                    )
                })
                .unwrap_or_default(),
            if existing.is_empty() {
                "（无）".to_string()
            } else {
                existing
            },
            if occupied.is_empty() {
                "（无）".to_string()
            } else {
                occupied.join("、")
            }
        ),
        expect_json: true,
    }
}

fn parse_concept_proposal(
    space: &DesignSpace,
    modules: &BTreeMap<String, SystemModule>,
    text: &str,
) -> Adm4Result<ConceptProposal> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|error| {
        Adm4Error::validation(format!("概念提案不是合法 JSON：{error}；原文：{text}"))
    })?;
    let mut proposal: ConceptProposal = serde_json::from_value(value).map_err(|error| {
        Adm4Error::validation(format!(
            "概念提案 JSON 结构不符（字段形态见 schema 文档 08 续档）：{error}"
        ))
    })?;

    // 实例 id 合法且唯一（与既有实例也不冲突——确认落盘会追加进装配）。
    let mut seen: BTreeSet<&str> = space
        .pack
        .system_refs
        .iter()
        .map(|reference| reference.instance_id.as_str())
        .collect();
    for system in &proposal.systems {
        let id_ok = !system.instance_id.trim().is_empty()
            && system.instance_id.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
            });
        if !id_ok {
            return Err(Adm4Error::validation(format!(
                "概念提案的实例 id 非法（只接受小写字母/数字/下划线）：{:?}",
                system.instance_id
            )));
        }
        if !seen.insert(system.instance_id.as_str()) {
            // 拒收判定不变（防线不拆）；文案增补已占用清单方便人读改名（波 6 前置缺口 A）。
            let occupied = occupied_instance_ids(space);
            return Err(Adm4Error::validation(format!(
                "概念提案的实例 id {} 重复（或与既有实例冲突）——实例 id 是命名空间锚，必须唯一。\
                 已占用清单：{}",
                system.instance_id,
                if occupied.is_empty() {
                    "（无既有实例，重复发生在提案内部）".to_string()
                } else {
                    occupied.join("、")
                }
            )));
        }
        // 模块与档位存在性（发明即 Err——验收 5 的负测试锚点）。
        let module = require_module(modules, &system.module_id)?;
        if module.heaviness.tier_rank(&system.suggested_tier).is_none() {
            return Err(Adm4Error::validation(format!(
                "概念提案给实例 {} 建议的档位 {} 不在模块 {} 的阶梯内（发明档位被拒绝）",
                system.instance_id, system.suggested_tier, system.module_id
            )));
        }
    }

    // 名词绑定：AI 给的先校验，缺的确定性推导补全（推导失败即 Err 点名）。
    let instance_modules: BTreeMap<&str, &SystemModule> = proposal
        .systems
        .iter()
        .map(|system| {
            (
                system.instance_id.as_str(),
                &modules[&system.module_id] as &SystemModule,
            )
        })
        .collect();
    let mut resolved_bindings = Vec::with_capacity(proposal.systems.len());
    for system in &proposal.systems {
        resolved_bindings.push(resolve_bindings(space, &instance_modules, system)?);
    }
    for (system, bindings) in proposal.systems.iter_mut().zip(resolved_bindings) {
        system.noun_bindings = bindings;
    }

    // core_loop 动词绑定：实例必须在提案或既有实例内（悬空即 Err）。
    let known: BTreeSet<&str> = proposal
        .systems
        .iter()
        .map(|system| system.instance_id.as_str())
        .chain(
            space
                .pack
                .system_refs
                .iter()
                .map(|reference| reference.instance_id.as_str()),
        )
        .collect();
    for entry in &proposal.core_loop {
        if entry.verb.trim().is_empty() {
            return Err(Adm4Error::validation(
                "core_loop 动词不可为空（动词序列是 κ 推导与叙述的数据源）",
            ));
        }
        if !known.contains(entry.instance_id.as_str()) {
            return Err(Adm4Error::validation(format!(
                "core_loop 动词「{}」绑定的实例 {} 不在提案系统清单内（悬空绑定被拒绝）",
                entry.verb, entry.instance_id
            )));
        }
    }

    // 融合分解的实例引用校验。
    if let Some(fusion) = &proposal.fusion {
        if fusion.cores.len() < 2 {
            return Err(Adm4Error::validation(
                "融合型分解至少要有两个核（单核口述不该出 fusion 字段）",
            ));
        }
        for core in &fusion.cores {
            for instance_id in &core.instance_ids {
                if !known.contains(instance_id.as_str()) {
                    return Err(Adm4Error::validation(format!(
                        "融合核「{}」引用的实例 {instance_id} 不在提案系统清单内",
                        core.label
                    )));
                }
            }
        }
    }

    // 大战略嗅探：重核候选按建议档计算（提案时刻冻结）。
    let mut heavy: Vec<String> = proposal
        .systems
        .iter()
        .filter(|system| {
            let module = &modules[&system.module_id];
            let heavy_rating = module
                .heaviness
                .tier_rank(&system.suggested_tier)
                .map(|rank| module.heaviness.tiers[rank].rating.total() >= 9)
                .unwrap_or(false);
            heavy_rating && matches!(system.core_link, CoreLink::Core | CoreLink::Strong)
        })
        .map(|system| system.instance_id.clone())
        .collect();
    heavy.sort();
    proposal.per_heavy_core_mode = heavy.len() > HEAVY_CORE_THRESHOLD;
    proposal.heavy_core_candidates = heavy;
    proposal.tier_clarifications.clear(); // 理清记录只能由 clarify_tier 产生。
    proposal.hints.clear();
    if proposal.per_heavy_core_mode {
        proposal.hints.push(format!(
            "超大玩法提示（不设阻）：本提案含 {} 个重核候选（{}），超出 |H| 参考线上限 4。\
             访谈切入逐重核轻重理清模式——每个重核系统须回答「轻度还是重度？对标哪款\
             游戏的哪个系统？」并落档位声明与理由（interview concept-clarify）。\
             总体形态由你自行设计；落盘后 |H| 超参考线的一次性署名确认走 compose confirm-form。",
            proposal.heavy_core_candidates.len(),
            proposal.heavy_core_candidates.join("、")
        ));
    }
    Ok(proposal)
}

/// 名词绑定的确定性推导（非 AI）：显式绑定先校验，缺失的按规则补全。
///
/// 规则（按序取首个命中，全部不中 → Err 点名名词与修复方向）：
/// - 目标已显式给出：必须是 pack 核心名词，或 `<提案内提供方实例>.<名词>` 且提供方
///   模块确实 provides 该名词；
/// - 带命名空间名词 `sys.X.n`：提案内恰有一个模块 `sys.X` 的实例 → 绑
///   `<该实例>.n`；多于一个 → 歧义 Err（AI/用户须显式指定）；没有但 `n` 在
///   pack 核心名词内 → 绑核心名词；`n` 不精确命中时按尾段后缀找核心名词的
///   全名变体（`command_intent` → `player_command_intent`），唯一才绑、
///   多候选歧义 Err（不许静默绑错）；
/// - 裸名词 `n`：本模块自身 provides → 自绑 `<self>.n`；否则 pack 核心名词；
///   否则提案内恰有一个别的实例 provides → 绑它。
fn resolve_bindings(
    space: &DesignSpace,
    instance_modules: &BTreeMap<&str, &SystemModule>,
    system: &ConceptSystem,
) -> Adm4Result<BTreeMap<String, String>> {
    let core_nouns: BTreeSet<&str> = space.pack.core_nouns.iter().map(String::as_str).collect();
    let module = instance_modules[system.instance_id.as_str()];
    let mut bindings = BTreeMap::new();
    let bound_nouns = module
        .interface
        .consumes
        .iter()
        .chain(&module.interface.modifies);
    for noun in bound_nouns {
        if let Some(target) = system.noun_bindings.get(noun) {
            validate_explicit_binding(
                &core_nouns,
                instance_modules,
                &system.instance_id,
                noun,
                target,
            )?;
            bindings.insert(noun.clone(), target.clone());
            continue;
        }
        let target = derive_binding(&core_nouns, instance_modules, system, module, noun)?;
        bindings.insert(noun.clone(), target);
    }
    Ok(bindings)
}

fn validate_explicit_binding(
    core_nouns: &BTreeSet<&str>,
    instance_modules: &BTreeMap<&str, &SystemModule>,
    instance_id: &str,
    noun: &str,
    target: &str,
) -> Adm4Result<()> {
    if core_nouns.contains(target) {
        return Ok(());
    }
    if let Some((provider_instance, provided_noun)) = target.rsplit_once('.')
        && let Some(provider_module) = instance_modules.get(provider_instance)
        && provider_module
            .interface
            .provides
            .iter()
            .any(|provided| local_noun(provided) == provided_noun)
    {
        return Ok(());
    }
    Err(Adm4Error::validation(format!(
        "概念提案里实例 {instance_id} 的名词 {noun} 绑定目标 {target} 悬空：\
         既不是 pack 核心名词，也不是提案内某实例 provides 的 <实例>.<名词>（发明绑定被拒绝）"
    )))
}

fn derive_binding(
    core_nouns: &BTreeSet<&str>,
    instance_modules: &BTreeMap<&str, &SystemModule>,
    system: &ConceptSystem,
    module: &SystemModule,
    noun: &str,
) -> Adm4Result<String> {
    // 带命名空间名词：sys.X.n → 找提案内模块 sys.X 的实例。
    if let Some((module_id, bare)) = noun.rsplit_once('.') {
        let providers: Vec<&str> = instance_modules
            .iter()
            .filter(|(_, m)| m.module_id == module_id)
            .map(|(id, _)| *id)
            .collect();
        match providers.as_slice() {
            [only] => return Ok(format!("{only}.{bare}")),
            [] => {
                if core_nouns.contains(bare) {
                    return Ok(bare.to_string());
                }
                // 命名口径兜底：六包核心名词统一带语义前缀（如 player_command_intent），
                // 模块 consumes 声明用裸尾段（sys.player_input.command_intent）——
                // 按尾段后缀匹配找全名变体。唯一候选才绑；多候选歧义 Err 不静默绑错
                // （宁可不兜底，留给 AI/用户显式 noun_bindings）。
                let suffix = format!("_{bare}");
                let variants: Vec<&str> = core_nouns
                    .iter()
                    .filter(|candidate| candidate.ends_with(suffix.as_str()))
                    .copied()
                    .collect();
                match variants.as_slice() {
                    [only] => return Ok((*only).to_string()),
                    [] => {
                        return Err(Adm4Error::validation(format!(
                            "实例 {} 的名词 {noun} 无法绑定：提案内没有模块 {module_id} 的实例，\
                             {bare} 也不是 pack 核心名词（含尾段变体）。修复方向：把 {module_id} \
                             加进系统清单，或在提案里显式给 noun_bindings",
                            system.instance_id
                        )));
                    }
                    multiple => {
                        return Err(Adm4Error::validation(format!(
                            "实例 {} 的名词 {noun} 绑定歧义：pack 核心名词里有多个 {bare} 的\
                             全名变体（{}），须在提案 noun_bindings 里显式指定",
                            system.instance_id,
                            multiple.join("、")
                        )));
                    }
                }
            }
            multiple => {
                return Err(Adm4Error::validation(format!(
                    "实例 {} 的名词 {noun} 绑定歧义：提案内有多个模块 {module_id} 的实例（{}），\
                     须在提案 noun_bindings 里显式指定",
                    system.instance_id,
                    multiple.join("、")
                )));
            }
        }
    }
    // 裸名词：自身 provides → 自绑；否则核心名词；否则唯一外部提供方。
    if module
        .interface
        .provides
        .iter()
        .any(|provided| local_noun(provided) == noun)
    {
        return Ok(format!("{}.{noun}", system.instance_id));
    }
    if core_nouns.contains(noun) {
        return Ok(noun.to_string());
    }
    let providers: Vec<&str> = instance_modules
        .iter()
        .filter(|(id, m)| {
            **id != system.instance_id
                && m.interface
                    .provides
                    .iter()
                    .any(|provided| local_noun(provided) == noun)
        })
        .map(|(id, _)| *id)
        .collect();
    match providers.as_slice() {
        [only] => Ok(format!("{only}.{noun}")),
        [] => Err(Adm4Error::validation(format!(
            "实例 {} 的名词 {noun} 无法绑定：模块自身不 provides、不是 pack 核心名词、\
             提案内也没有实例 provides 它。修复方向：补充提供方系统，或在 pack 声明核心名词",
            system.instance_id
        ))),
        multiple => Err(Adm4Error::validation(format!(
            "实例 {} 的名词 {noun} 绑定歧义：多个实例都 provides 它（{}），\
             须在提案 noun_bindings 里显式指定",
            system.instance_id,
            multiple.join("、")
        ))),
    }
}

/// 与加载器/组合层同口径：带点号取末段。
fn local_noun(noun: &str) -> &str {
    noun.rsplit('.').next().unwrap_or(noun)
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_ai::ScriptedProvider;
    use adm4_decision::system_module::{
        FiveAxisRating, HeavinessLadder, HeavinessTier, MdaMapping, NounDecl, NounKind,
        SystemInterface,
    };
    use adm4_decision::{DecisionGraph, DesignOrganization};
    use adm4_space::GenrePack;

    fn tier(id: &str, weight: u8) -> HeavinessTier {
        HeavinessTier {
            id: id.into(),
            label_zh: id.into(),
            rating: FiveAxisRating {
                m: weight,
                d: weight,
                c: weight,
                p: weight,
                o: weight,
            },
            p_floor: 0,
            interface_floor: 0,
            activates: Vec::new(),
            inductions: Vec::new(),
            summary: format!("{id} 档"),
        }
    }

    /// 造一个两档模块（light W5 / heavy W10），provides 一个名词。
    fn module(module_id: &str, provides: &str) -> SystemModule {
        SystemModule {
            module_id: module_id.into(),
            semver: "1.0.0".into(),
            label_zh: module_id.into(),
            summary: String::new(),
            nouns: vec![NounDecl {
                id: provides.into(),
                kind: NounKind::Resource,
                label_zh: provides.into(),
                summary: String::new(),
            }],
            interface: SystemInterface {
                provides: vec![provides.into()],
                consumes: Vec::new(),
                modifies: Vec::new(),
            },
            mda: MdaMapping::default(),
            heaviness: HeavinessLadder {
                tiers: vec![tier("light", 1), tier("heavy", 2)],
            },
            decision_points: Vec::new(),
            cardinality_expectations: BTreeMap::new(),
            consistency_rules: Vec::new(),
            skin_fields: Vec::new(),
        }
    }

    fn module_table(count: usize) -> BTreeMap<String, SystemModule> {
        (0..count)
            .map(|index| {
                let id = format!("sys.mod{index}");
                (id.clone(), module(&id, &format!("noun{index}")))
            })
            .collect()
    }

    fn empty_space() -> DesignSpace {
        DesignSpace {
            universal_version: "test".into(),
            pack: GenrePack {
                pack_id: "concept_test".into(),
                pack_version: "0.1.0".into(),
                display_name: "概念访谈测试包".into(),
                reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
                profile_points: Vec::new(),
                cardinality_expectations: Default::default(),
                consistency_rules: Vec::new(),
                nodes: Vec::new(),
                decision_points: Vec::new(),
                system_refs: Vec::new(),
                core_nouns: vec!["mana".into()],
            },
            graph: DecisionGraph::new(Vec::new()).expect("空图应可构造"),
            organization: DesignOrganization::new(Vec::new(), Vec::new()),
            system_instances: Vec::new(),
        }
    }

    /// 带既有实例的空间（缺口 A 测试用）：occupied_id 已占用 sys.mod0 的实例位。
    fn space_with_existing(occupied_id: &str) -> DesignSpace {
        let mut space = empty_space();
        space.pack.system_refs.push(SystemRef {
            instance_id: occupied_id.into(),
            module_id: "sys.mod0".into(),
            version_req: String::new(),
            allowed_tiers: Vec::new(),
            noun_bindings: BTreeMap::new(),
            core_link: CoreLink::Core,
        });
        space
    }

    fn system_json(index: usize, tier: &str, link: &str) -> String {
        format!(
            r#"{{"instance_id":"inst{index}","module_id":"sys.mod{index}","suggested_tier":"{tier}","core_link":"{link}","rationale":"理由{index}"}}"#
        )
    }

    fn scripted(purpose: &str, response: &str) -> ScriptedProvider {
        let provider = ScriptedProvider::new();
        provider.script(purpose, vec![response.to_string()]);
        provider
    }

    /// 正常提案：解析 + 绑定补全 + core_loop 校验。
    #[test]
    fn propose_parses_and_resolves() {
        let modules = module_table(2);
        let response = format!(
            r#"{{"systems":[{},{}],"core_loop":[{{"verb":"采集","instance_id":"inst0"}},{{"verb":"消耗","instance_id":"inst1"}}],"library_external":[{{"name":"天气系统","note":"库内暂无"}}],"notes":"双系统循环"}}"#,
            system_json(0, "light", "core"),
            system_json(1, "light", "weak")
        );
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let proposal =
            propose_concept(&empty_space(), &modules, &provider, "采集与消耗的小游戏").unwrap();
        assert_eq!(proposal.systems.len(), 2);
        assert_eq!(proposal.core_loop.len(), 2);
        assert_eq!(proposal.library_external.len(), 1);
        assert!(!proposal.per_heavy_core_mode);
        assert!(proposal.heavy_core_candidates.is_empty());
        // scripted 通道可断言请求形态：purpose 与模块目录进了提示词。
        let calls = provider.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].purpose, PURPOSE_CONCEPT);
        assert!(
            calls[0].user_prompt.contains("sys.mod0"),
            "模块目录应进提示词"
        );
    }

    /// 缺口 A 注入：提示词携带已占用实例 id 清单与「不得重复」硬约束
    /// （AI 提案、用户确认纪律不变——注入只是让 AI 看得见接缝）。
    #[test]
    fn prompt_carries_occupied_instance_ids_and_constraint() {
        let modules = module_table(1);
        let response = format!(r#"{{"systems":[{}]}}"#, system_json(0, "light", "core"));
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let space = space_with_existing("combat_main");
        propose_concept(&space, &modules, &provider, "口述").unwrap();
        let calls = provider.calls();
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0].user_prompt.contains("已占用实例 id 清单"),
            "user prompt 应携带已占用清单标题：{}",
            calls[0].user_prompt
        );
        assert!(
            calls[0].user_prompt.contains("combat_main"),
            "已占用实例 id 应进提示词：{}",
            calls[0].user_prompt
        );
        assert!(
            calls[0]
                .system_prompt
                .contains("不得与「已占用实例 id 清单」"),
            "system prompt 应携带硬约束：{}",
            calls[0].system_prompt
        );
    }

    /// 缺口 A 反测试：脚本模拟 AI 无视约束返回撞名实例 → 仍被拒（防线不拆），
    /// 且拒收文案携带已占用清单（人读改名指引）。
    #[test]
    fn colliding_instance_id_still_rejected_with_occupied_list() {
        let modules = module_table(1);
        let response = r#"{"systems":[{"instance_id":"combat_main","module_id":"sys.mod0","suggested_tier":"light","core_link":"core","rationale":"撞名"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let space = space_with_existing("combat_main");
        let error = propose_concept(&space, &modules, &provider, "口述").unwrap_err();
        assert!(error.message.contains("combat_main"), "{}", error.message);
        assert!(error.message.contains("必须唯一"), "{}", error.message);
        assert!(
            error.message.contains("已占用清单"),
            "拒收文案应携带已占用清单：{}",
            error.message
        );
    }

    /// 缺口 A 正测试：脚本返回避开清单的提案 → 通过（新名不与既有实例冲突）。
    #[test]
    fn non_colliding_instance_id_passes_with_existing_refs() {
        let modules = module_table(1);
        let response = r#"{"systems":[{"instance_id":"combat_secondary","module_id":"sys.mod0","suggested_tier":"light","core_link":"core","rationale":"避开清单"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let space = space_with_existing("combat_main");
        let proposal = propose_concept(&space, &modules, &provider, "口述").unwrap();
        assert_eq!(proposal.systems.len(), 1);
        assert_eq!(proposal.systems[0].instance_id, "combat_secondary");
    }

    /// 提案内部重复（无既有实例）：拒收文案如实说明重复发生在提案内部。
    #[test]
    fn internal_duplicate_gets_honest_message_without_pack_refs() {
        let modules = module_table(1);
        let response = r#"{"systems":[
            {"instance_id":"twin","module_id":"sys.mod0","suggested_tier":"light","core_link":"core","rationale":"a"},
            {"instance_id":"twin","module_id":"sys.mod0","suggested_tier":"light","core_link":"weak","rationale":"b"}
        ]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let error = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap_err();
        assert!(error.message.contains("twin"), "{}", error.message);
        assert!(
            error.message.contains("提案内部"),
            "空清单时文案应说明重复来自提案内部：{}",
            error.message
        );
    }

    /// 验收 5 负测试：发明模块 id → Err。
    #[test]
    fn invented_module_id_is_rejected() {
        let modules = module_table(1);
        let response = r#"{"systems":[{"instance_id":"ghost","module_id":"sys.ghost","suggested_tier":"light","core_link":"core","rationale":"x"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let error = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap_err();
        assert!(error.message.contains("sys.ghost"), "{}", error.message);
        assert!(error.message.contains("发明"), "{}", error.message);
    }

    /// 验收 5 负测试：发明档位 id → Err。
    #[test]
    fn invented_tier_id_is_rejected() {
        let modules = module_table(1);
        let response = r#"{"systems":[{"instance_id":"inst0","module_id":"sys.mod0","suggested_tier":"ultra","core_link":"core","rationale":"x"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let error = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap_err();
        assert!(error.message.contains("ultra"), "{}", error.message);
        assert!(error.message.contains("发明档位"), "{}", error.message);
    }

    /// core_loop 悬空绑定 → Err。
    #[test]
    fn dangling_core_loop_binding_is_rejected() {
        let modules = module_table(1);
        let response = format!(
            r#"{{"systems":[{}],"core_loop":[{{"verb":"漂移","instance_id":"nobody"}}]}}"#,
            system_json(0, "light", "core")
        );
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let error = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap_err();
        assert!(error.message.contains("nobody"), "{}", error.message);
    }

    /// 大战略嗅探：5 个重核候选（heavy W10 + core/strong）→ 切逐重核模式 + 提示不设阻。
    #[test]
    fn heavy_core_sniff_switches_mode_over_threshold() {
        let modules = module_table(5);
        let systems: Vec<String> = (0..5).map(|i| system_json(i, "heavy", "core")).collect();
        let response = format!(r#"{{"systems":[{}]}}"#, systems.join(","));
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let proposal = propose_concept(&empty_space(), &modules, &provider, "大战略").unwrap();
        assert!(proposal.per_heavy_core_mode, "5 重核候选必须切逐重核模式");
        assert_eq!(proposal.heavy_core_candidates.len(), 5);
        assert!(
            proposal.hints.iter().any(|hint| hint.contains("不设阻")),
            "提示义务：{:?}",
            proposal.hints
        );
        // 未理清 → 确认被拒（模式的硬性设计义务）。
        let error = proposal.validate_for_confirm().unwrap_err();
        assert!(error.message.contains("未理清"), "{}", error.message);
    }

    /// 4 个重核候选（= 阈值）不切模式。
    #[test]
    fn heavy_core_sniff_at_threshold_does_not_switch() {
        let modules = module_table(4);
        let systems: Vec<String> = (0..4).map(|i| system_json(i, "heavy", "strong")).collect();
        let response = format!(r#"{{"systems":[{}]}}"#, systems.join(","));
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let proposal = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap();
        assert!(!proposal.per_heavy_core_mode);
        proposal.validate_for_confirm().unwrap();
    }

    /// 逐重核理清：AI 落档位 + rationale；全部理清后确认放行；发明档位被拒。
    #[test]
    fn clarify_tier_records_and_gates_confirm() {
        let modules = module_table(5);
        let systems: Vec<String> = (0..5).map(|i| system_json(i, "heavy", "core")).collect();
        let response = format!(r#"{{"systems":[{}]}}"#, systems.join(","));
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let mut proposal = propose_concept(&empty_space(), &modules, &provider, "大战略").unwrap();

        // 逐个理清：inst0..inst4，脚本按序回放（前 4 个 heavy、最后 1 个降 light）。
        let clarifier = ScriptedProvider::new();
        clarifier.script(
            PURPOSE_CONCEPT_TIER,
            vec![
                r#"{"tier_id":"heavy","rationale":"对标 EU4 外交：全谈判栈"}"#.into(),
                r#"{"tier_id":"heavy","rationale":"对标 EU4 战争"}"#.into(),
                r#"{"tier_id":"heavy","rationale":"对标 EU4 贸易"}"#.into(),
                r#"{"tier_id":"heavy","rationale":"对标 EU4 宗教"}"#.into(),
                r#"{"tier_id":"light","rationale":"对标文明 6：只要轻度"}"#.into(),
            ],
        );
        for index in 0..5 {
            proposal = clarify_tier(
                &modules,
                &clarifier,
                proposal,
                &format!("inst{index}"),
                "要重度，对标 EU4",
            )
            .unwrap();
        }
        assert_eq!(proposal.tier_clarifications.len(), 5);
        // 理清覆盖建议档：inst4 生效档位为 light、带理清 rationale。
        let (tier, rationale) = proposal.effective_tier("inst4").unwrap();
        assert_eq!(tier, "light");
        assert!(rationale.contains("文明"), "{rationale}");
        proposal.validate_for_confirm().unwrap();

        // 理清发明档位 → Err。
        let bad = ScriptedProvider::new();
        bad.script(
            PURPOSE_CONCEPT_TIER,
            vec![r#"{"tier_id":"colossal","rationale":"x"}"#.into()],
        );
        let error = clarify_tier(&modules, &bad, proposal.clone(), "inst0", "重").unwrap_err();
        assert!(error.message.contains("colossal"), "{}", error.message);

        // 理清缺 rationale → Err（理清的目的就是理由）。
        let no_reason = ScriptedProvider::new();
        no_reason.script(
            PURPOSE_CONCEPT_TIER,
            vec![r#"{"tier_id":"heavy","rationale":""}"#.into()],
        );
        let error = clarify_tier(&modules, &no_reason, proposal, "inst0", "重").unwrap_err();
        assert!(error.message.contains("rationale"), "{}", error.message);
    }

    /// 融合型嗅探（验收 4）："X+Y"口述 → 双核并集分解提案结构。
    #[test]
    fn fusion_sniff_parses_dual_core_decomposition() {
        let modules = module_table(3);
        let response = format!(
            r#"{{"systems":[{},{},{}],"core_loop":[{{"verb":"合成","instance_id":"inst0"}},{{"verb":"布防","instance_id":"inst1"}},{{"verb":"结算","instance_id":"inst2"}}],"fusion":{{"cores":[{{"label":"合成大西瓜","instance_ids":["inst0"]}},{{"label":"塔防","instance_ids":["inst1","inst2"]}}],"transition":"合成产物作为塔防单位入场，波次结算返还合成素材"}},"notes":"融合型"}}"#,
            system_json(0, "light", "core"),
            system_json(1, "light", "core"),
            system_json(2, "light", "weak")
        );
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let proposal =
            propose_concept(&empty_space(), &modules, &provider, "合成大西瓜+塔防").unwrap();
        let fusion = proposal.fusion.as_ref().expect("融合口述应产 fusion 分解");
        assert_eq!(fusion.cores.len(), 2);
        assert_eq!(fusion.cores[0].label, "合成大西瓜");
        assert_eq!(fusion.cores[1].instance_ids, vec!["inst1", "inst2"]);
        assert!(fusion.transition.contains("塔防单位"));
        // 两核系统清单并集 = 提案系统清单；嵌套 core_loop 覆盖两核。
        assert_eq!(proposal.systems.len(), 3);
        assert_eq!(proposal.core_loop.len(), 3);

        // 融合核引用悬空实例 → Err。
        let bad = format!(
            r#"{{"systems":[{}],"fusion":{{"cores":[{{"label":"甲","instance_ids":["inst0"]}},{{"label":"乙","instance_ids":["ghost"]}}],"transition":"x"}}}}"#,
            system_json(0, "light", "core")
        );
        let provider = scripted(PURPOSE_CONCEPT, &bad);
        let error = propose_concept(&empty_space(), &modules, &provider, "X+Y").unwrap_err();
        assert!(error.message.contains("ghost"), "{}", error.message);
    }

    /// 名词绑定推导：命名空间名词绑唯一提供方实例；裸名词自绑/核心名词。
    #[test]
    fn binding_resolution_covers_namespace_and_bare_nouns() {
        let mut consumer = module("sys.consumer", "own_thing");
        consumer.interface.consumes = vec!["sys.mod0.noun0".into()];
        consumer.interface.modifies = vec!["own_thing".into(), "mana".into()];
        let mut modules = module_table(1);
        modules.insert("sys.consumer".into(), consumer);
        let response = format!(
            r#"{{"systems":[{},{{"instance_id":"eater","module_id":"sys.consumer","suggested_tier":"light","core_link":"weak","rationale":"x"}}]}}"#,
            system_json(0, "light", "core")
        );
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let proposal = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap();
        let eater = proposal
            .systems
            .iter()
            .find(|system| system.instance_id == "eater")
            .unwrap();
        assert_eq!(eater.noun_bindings["sys.mod0.noun0"], "inst0.noun0");
        assert_eq!(eater.noun_bindings["own_thing"], "eater.own_thing");
        assert_eq!(eater.noun_bindings["mana"], "mana");

        // 无提供方且非核心名词 → Err 点名。
        let orphan_only = r#"{"systems":[{"instance_id":"eater","module_id":"sys.consumer","suggested_tier":"light","core_link":"weak","rationale":"x"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, orphan_only);
        let error = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap_err();
        assert!(
            error.message.contains("sys.mod0.noun0"),
            "{}",
            error.message
        );
    }

    /// 命名口径兜底正测试：`sys.player_input.command_intent` 在提案无该模块实例、
    /// 裸段不精确命中时，按尾段后缀唯一命中核心名词全名变体 `player_command_intent`。
    #[test]
    fn namespaced_noun_falls_back_to_core_noun_suffix_variant() {
        let mut consumer = module("sys.consumer", "own_thing");
        consumer.interface.consumes = vec!["sys.player_input.command_intent".into()];
        let mut modules = BTreeMap::new();
        modules.insert("sys.consumer".into(), consumer);
        let mut space = empty_space();
        space.pack.core_nouns = vec!["player_command_intent".into()];
        let response = r#"{"systems":[{"instance_id":"eater","module_id":"sys.consumer","suggested_tier":"light","core_link":"core","rationale":"x"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let proposal = propose_concept(&space, &modules, &provider, "口述").unwrap();
        assert_eq!(
            proposal.systems[0].noun_bindings["sys.player_input.command_intent"],
            "player_command_intent",
            "尾段后缀唯一命中应兜底到核心名词全名变体"
        );
    }

    /// 命名口径兜底反测试：多个全名变体（player_/enemy_command_intent）→ 歧义 Err
    /// 点名候选（红线：不许静默绑错，留给 AI/用户显式 noun_bindings）。
    #[test]
    fn ambiguous_core_noun_suffix_variants_are_rejected_not_silently_bound() {
        let mut consumer = module("sys.consumer", "own_thing");
        consumer.interface.consumes = vec!["sys.player_input.command_intent".into()];
        let mut modules = BTreeMap::new();
        modules.insert("sys.consumer".into(), consumer);
        let mut space = empty_space();
        space.pack.core_nouns = vec![
            "player_command_intent".into(),
            "enemy_command_intent".into(),
        ];
        let response = r#"{"systems":[{"instance_id":"eater","module_id":"sys.consumer","suggested_tier":"light","core_link":"core","rationale":"x"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let error = propose_concept(&space, &modules, &provider, "口述").unwrap_err();
        assert!(error.message.contains("歧义"), "{}", error.message);
        assert!(
            error.message.contains("player_command_intent")
                && error.message.contains("enemy_command_intent"),
            "歧义文案应点名全部候选：{}",
            error.message
        );
        assert!(
            error.message.contains("noun_bindings"),
            "歧义文案应指路显式绑定：{}",
            error.message
        );
    }

    /// 精确命中优先于后缀变体：裸段本身就是核心名词时直接绑，不受变体干扰。
    #[test]
    fn exact_core_noun_match_wins_over_suffix_variant() {
        let mut consumer = module("sys.consumer", "own_thing");
        consumer.interface.consumes = vec!["sys.player_input.command_intent".into()];
        let mut modules = BTreeMap::new();
        modules.insert("sys.consumer".into(), consumer);
        let mut space = empty_space();
        space.pack.core_nouns = vec!["command_intent".into(), "player_command_intent".into()];
        let response = r#"{"systems":[{"instance_id":"eater","module_id":"sys.consumer","suggested_tier":"light","core_link":"core","rationale":"x"}]}"#;
        let provider = scripted(PURPOSE_CONCEPT, response);
        let proposal = propose_concept(&space, &modules, &provider, "口述").unwrap();
        assert_eq!(
            proposal.systems[0].noun_bindings["sys.player_input.command_intent"], "command_intent",
            "精确命中必须优先，不进后缀变体分支"
        );
    }

    /// proposal_to_refs：绑定/κ/档位如实转 SystemRef。
    #[test]
    fn proposal_converts_to_refs() {
        let modules = module_table(1);
        let response = format!(r#"{{"systems":[{}]}}"#, system_json(0, "heavy", "strong"));
        let provider = scripted(PURPOSE_CONCEPT, &response);
        let proposal = propose_concept(&empty_space(), &modules, &provider, "口述").unwrap();
        let refs = proposal_to_refs(&proposal);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].instance_id, "inst0");
        assert_eq!(refs[0].module_id, "sys.mod0");
        assert_eq!(refs[0].core_link, CoreLink::Strong);
    }
}
