//! 权威顺序校验插件（册 07 铁律③）：**JSON 契约永远压过 Markdown**。
//!
//! 校验器拦两类漂移，两类都是确定性判定：
//!
//! - **Markdown-only 事实**（py 协议 §8 硬阻塞 10）：叙述里出现、契约里查无此物的关键事实。
//!   Markdown 只是渲染层，它不许携带任何独有事实——一旦携带，那个事实就没有机器可读的出处，
//!   下游谁也拿不到它，最终变成「文档说有、程序里没有」。
//! - **无源锚点**（铁律①）：契约里的 `SpecRef` 在真源 `GameSpec` 里不存在，即下游发明了设计事实。
//!
//! ## Markdown 事实的识别规则（确定性，无 AI）
//!
//! 渲染层把事实标识一律写成**行内代码**（`` `UI_PlayerIdle` ``）。校验器只认能按
//! [`FactShape`] 归类的行内代码：类型前缀命名的资产 id、`SpecRef` 路径、派生 id 前缀
//! （`STATE-`/`UX-`/`DRIFT-`）。归不了类的行内代码（`contract.json`、普通词汇）是排版，
//! 不参与判定——宁可漏判一个奇形怪状的 id，也不要把「文件名」误判成事实把整份文档拦死。
//!
//! ### 已知覆盖边界（如实登记，别当它不存在）
//!
//! 程序线的 `system_id` / `capability_id` 等标识**没有形态特征**（`combat_system` 与普通
//! snake_case 词汇长得一样），因此 Markdown 里凭空多出一个系统名，本校验器检不出来。
//! 收窄办法是给程序线 id 定一组命名空间前缀再登记进 [`FactShape`]——那要改的是程序线的
//! id 约定，属派生器落地时（G4）连同 id 生成规则一起定，本波不擅自改约定。
//! 见 `crate::governance::art_line`：美术线因为有类型前缀与派生前缀，这条缝在美术线上不存在。

use super::art_line::{ArtContract, DRIFT_CHECK_PREFIX, UX_BINDING_PREFIX, VISUAL_STATE_PREFIX};
use super::asset_registry::{AssetRegistry, NamingRules};
use super::program_line::ProgramContract;
use adm4_contracts::SpecRef;
use adm4_spec::GameSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 权威顺序表（册 07 铁律③；数字越小权威越高）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoritySource {
    /// 1 · 唯一真源。
    #[default]
    GameSpec,
    /// 2 · 派生协议（册 07 本身）。
    DerivationProtocol,
    /// 3 · 程序线契约。
    ProgramContract,
    /// 4 · 美术线契约。
    ArtContract,
    /// 5 · 资产表（命名权威）。
    AssetRegistry,
    /// 6 · 溯源/校验证据。
    Trace,
    /// 7 · Markdown（仅人类可读渲染）。
    Markdown,
}

impl AuthoritySource {
    /// 权威序号（1 最高，7 最低）。
    pub fn priority(self) -> u8 {
        match self {
            Self::GameSpec => 1,
            Self::DerivationProtocol => 2,
            Self::ProgramContract => 3,
            Self::ArtContract => 4,
            Self::AssetRegistry => 5,
            Self::Trace => 6,
            Self::Markdown => 7,
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::GameSpec => "GameSpec（唯一真源）",
            Self::DerivationProtocol => "派生协议",
            Self::ProgramContract => "程序线契约",
            Self::ArtContract => "美术线契约",
            Self::AssetRegistry => "资产表（命名权威）",
            Self::Trace => "溯源证据",
            Self::Markdown => "Markdown（仅渲染）",
        }
    }

    /// 是否只是渲染层（渲染层不得携带独有事实）。
    pub fn is_rendering_only(self) -> bool {
        matches!(self, Self::Markdown)
    }
}

/// 权威顺序表全序（按权威从高到低）。
pub fn authority_order() -> [AuthoritySource; 7] {
    [
        AuthoritySource::GameSpec,
        AuthoritySource::DerivationProtocol,
        AuthoritySource::ProgramContract,
        AuthoritySource::ArtContract,
        AuthoritySource::AssetRegistry,
        AuthoritySource::Trace,
        AuthoritySource::Markdown,
    ]
}

/// 从 Markdown 行内代码里能识别出的事实形态。
///
/// `Unknown` 是旧档/未归类的落点，[`classify`] 永远不会产出它——归不了类的行内代码
/// 压根不会变成一条 claim。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactShape {
    #[default]
    Unknown,
    /// 带类型前缀的资产 id（`UI_…` / `VFX_…` / …）。
    AssetId,
    /// `GameSpec` 锚点路径（`systems/x`、`mechanics/y`、`intent` …）。
    SpecAnchor,
    /// 从 asset_id 派生的 id（`STATE-` / `UX-` / `DRIFT-`）。
    DerivedId,
}

/// Markdown 里的一条事实声明。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MarkdownFactClaim {
    pub source_file: String,
    pub shape: FactShape,
    pub token: String,
}

/// 校验发现的机器码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityFindingCode {
    /// Markdown 里有、JSON 契约里没有的事实（硬阻塞）。
    MarkdownOnlyFact,
    /// 契约锚点在真源里不存在（下游发明了设计事实，硬阻塞）。
    AnchorNotInSource,
    /// 契约里的资产没在命名权威登记（硬阻塞）。
    AssetNotRegistered,
}

/// 一条校验发现。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthorityFinding {
    pub code: AuthorityFindingCode,
    /// 事实出现在哪一层（用于把「谁压过谁」讲清楚）。
    pub source: AuthoritySource,
    /// 出处（文件名 / 契约段名）。
    pub location: String,
    /// 具体事实标识。
    pub subject: String,
    pub detail: String,
    /// 是否阻断（本校验器的三条码全部阻断；留字段是给后续波次加非阻断项用）。
    pub blocking: bool,
}

impl Default for AuthorityFinding {
    fn default() -> Self {
        Self {
            code: AuthorityFindingCode::MarkdownOnlyFact,
            source: AuthoritySource::Markdown,
            location: String::new(),
            subject: String::new(),
            detail: String::new(),
            blocking: true,
        }
    }
}

/// 权威顺序校验报告。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthorityOrderReport {
    pub findings: Vec<AuthorityFinding>,
    /// 实际核对过的 Markdown 事实声明条数（R1：报实测量，不报「已核对」三个字）。
    pub checked_markdown_claims: usize,
    /// 实际核对过的契约锚点条数。
    pub checked_contract_anchors: usize,
}

impl AuthorityOrderReport {
    pub fn passed(&self) -> bool {
        !self.findings.iter().any(|finding| finding.blocking)
    }

    pub fn blocking_findings(&self) -> Vec<&AuthorityFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.blocking)
            .collect()
    }

    pub fn summary(&self) -> String {
        format!(
            "核对 Markdown 事实 {} 条、契约锚点 {} 条，阻塞发现 {} 条",
            self.checked_markdown_claims,
            self.checked_contract_anchors,
            self.blocking_findings().len()
        )
    }
}

/// 一份参与校验的 Markdown 渲染件。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MarkdownDocument {
    pub path: String,
    pub body: String,
}

impl MarkdownDocument {
    pub fn new(path: &str, body: &str) -> Self {
        Self {
            path: path.to_string(),
            body: body.to_string(),
        }
    }
}

/// 校验器输入：真源 + 两条线 + 命名权威 + 渲染件。
pub struct AuthorityOrderInput<'a> {
    pub spec: &'a GameSpec,
    pub program: &'a ProgramContract,
    pub art: &'a ArtContract,
    pub registry: &'a AssetRegistry,
    pub markdown: &'a [MarkdownDocument],
    /// 资产 id 的类型前缀（识别 Markdown 里的资产声明用；由品类包供给，见册 07 §6）。
    pub naming: &'a NamingRules,
}

/// 抽取一份 Markdown 里的事实声明（公开，便于单独测「识别规则」本身）。
pub fn extract_fact_claims(
    document: &MarkdownDocument,
    naming: &NamingRules,
) -> Vec<MarkdownFactClaim> {
    inline_code_spans(&document.body)
        .into_iter()
        .filter_map(|token| {
            classify(&token, naming).map(|shape| MarkdownFactClaim {
                source_file: document.path.clone(),
                shape,
                token,
            })
        })
        .collect()
}

/// 取出所有行内代码片段（成对反引号之间的内容；未闭合的反引号忽略）。
fn inline_code_spans(body: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut current: Option<String> = None;
    for character in body.chars() {
        match (character, &mut current) {
            ('`', None) => current = Some(String::new()),
            ('`', Some(buffer)) => {
                let token = buffer.trim().to_string();
                if !token.is_empty() {
                    spans.push(token);
                }
                current = None;
            }
            // 行内代码不跨行：遇到换行说明反引号没闭合，丢掉这段。
            ('\n', Some(_)) => current = None,
            (other, Some(buffer)) => buffer.push(other),
            (_, None) => {}
        }
    }
    spans
}

/// 把一个行内代码片段归类成事实形态；归不了类返回 None（那是排版，不是事实）。
fn classify(token: &str, naming: &NamingRules) -> Option<FactShape> {
    for prefix in [VISUAL_STATE_PREFIX, UX_BINDING_PREFIX, DRIFT_CHECK_PREFIX] {
        if token.starts_with(prefix) {
            return Some(FactShape::DerivedId);
        }
    }
    if naming
        .type_prefixes
        .iter()
        .any(|prefix| token.starts_with(prefix.as_str()))
    {
        return Some(FactShape::AssetId);
    }
    if is_spec_anchor(token) {
        return Some(FactShape::SpecAnchor);
    }
    None
}

/// `SpecRef` 的形态：`intent` / `identity`，或 `<已知段>/<id>`。
///
/// 段名白名单与 `GameSpec::contains_ref` 认的那一组保持一致——两处认的东西不一样，
/// 就会出现「校验器说这是锚点、真源说这不是路径」的鬼打墙。
fn is_spec_anchor(token: &str) -> bool {
    if matches!(token, "intent" | "identity") {
        return true;
    }
    let Some((section, id)) = token.split_once('/') else {
        return false;
    };
    if id.is_empty() || id.contains('/') {
        return false;
    }
    matches!(
        section,
        "intent" | "systems" | "mechanics" | "entities" | "tables" | "content" | "acceptance"
    )
}

/// 执行权威顺序校验。
pub fn validate_authority_order(input: &AuthorityOrderInput<'_>) -> AuthorityOrderReport {
    let mut report = AuthorityOrderReport::default();

    // ---- 契约锚点 ⊆ 真源（铁律①：下游只派生不发明）----
    let mut anchors: Vec<(AuthoritySource, &'static str, SpecRef)> = Vec::new();
    anchors.extend(
        input
            .program
            .source_refs()
            .into_iter()
            .map(|item| (AuthoritySource::ProgramContract, "program_contract", item)),
    );
    anchors.extend(
        input
            .art
            .source_refs()
            .into_iter()
            .map(|item| (AuthoritySource::ArtContract, "art_contract", item)),
    );
    for entry in &input.registry.entries {
        anchors.extend(
            entry
                .source_refs
                .iter()
                .cloned()
                .map(|item| (AuthoritySource::AssetRegistry, "asset_registry", item)),
        );
    }
    report.checked_contract_anchors = anchors.len();
    for (source, location, anchor) in anchors {
        if !input.spec.contains_ref(&anchor) {
            report.findings.push(AuthorityFinding {
                code: AuthorityFindingCode::AnchorNotInSource,
                source,
                location: location.to_string(),
                subject: anchor.0.clone(),
                detail: format!(
                    "锚点 {} 在 GameSpec 里不存在：{} 只能派生真源已有的事实（铁律①）",
                    anchor.0,
                    source.label_zh()
                ),
                blocking: true,
            });
        }
    }

    // ---- 美术线资产 ⊆ 命名权威（第 4 层不得压过第 5 层的命名判定）----
    for asset in &input.art.assets {
        if input.registry.entry(&asset.asset_id).is_none() {
            report.findings.push(AuthorityFinding {
                code: AuthorityFindingCode::AssetNotRegistered,
                source: AuthoritySource::ArtContract,
                location: "art_contract.assets".to_string(),
                subject: asset.asset_id.clone(),
                detail: "资产未在资产表登记：命名权威是 asset_id 的单点锚定处（铁律②）".to_string(),
                blocking: true,
            });
        }
    }

    // ---- Markdown 事实 ⊆ JSON 契约（铁律③：渲染层不得携带独有事实）----
    let known = known_facts(input);
    for document in input.markdown {
        for claim in extract_fact_claims(document, input.naming) {
            report.checked_markdown_claims += 1;
            if known.contains(&claim.token) {
                continue;
            }
            report.findings.push(AuthorityFinding {
                code: AuthorityFindingCode::MarkdownOnlyFact,
                source: AuthoritySource::Markdown,
                location: document.path.clone(),
                subject: claim.token.clone(),
                detail: format!(
                    "Markdown 声明了 {}，JSON 契约里查无此事实：Markdown 只是渲染层，\
                     不得携带独有事实（铁律③，JSON 压过 Markdown）",
                    claim.token
                ),
                blocking: true,
            });
        }
    }

    report
}

/// JSON 契约侧的全部事实标识（Markdown 里出现的事实必须落在这个集合里）。
fn known_facts(input: &AuthorityOrderInput<'_>) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    known.extend(input.program.declared_ids());
    known.extend(input.art.declared_ids());
    known.extend(
        input
            .registry
            .entries
            .iter()
            .map(|entry| entry.asset_id.clone()),
    );
    known.extend(
        input
            .spec
            .all_ref_paths()
            .into_iter()
            .map(|anchor| anchor.0),
    );
    // GameSpec 的 `acceptance` 不进 `all_ref_paths`（那份清单只到 content 为止），
    // 但验收场景确实是真源事实，Markdown 引用它必须被认；这里补齐。
    known.extend(
        input
            .spec
            .acceptance
            .iter()
            .map(|scenario| format!("acceptance/{}", scenario.id)),
    );
    known.insert("identity".to_string());
    known
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::art_line::{ArtAsset, AssetCategory};
    use crate::governance::asset_registry::{
        AssetLifecycleState, AssetRegistryEntry, StabilityLevel,
    };
    use crate::governance::program_line::ProgramSystem;
    use crate::governance::{ART_LINE, ContractEnvelope, PROGRAM_LINE};
    use adm4_spec::{EntitySpec, GameSpec, ProjectIntent, SpecIdentity, SystemSpec};

    fn spec() -> GameSpec {
        GameSpec {
            identity: SpecIdentity {
                schema_version: "4.0.0".into(),
                project_id: "demo".into(),
                frozen_hash: "sha256:frozen".into(),
            },
            intent: ProjectIntent::default(),
            systems: vec![SystemSpec {
                id: "combat".into(),
                name: "战斗".into(),
                purpose: String::new(),
                interfaces: Vec::new(),
                design_notes: Vec::new(),
            }],
            mechanics: Vec::new(),
            entities: vec![EntitySpec {
                id: "guard".into(),
                name: "守卫".into(),
                visual_form: None,
                properties: Vec::new(),
            }],
            tables: Vec::new(),
            content: Vec::new(),
            graphs: Vec::new(),
            acceptance: Vec::new(),
            source_map: Vec::new(),
        }
    }

    fn program() -> ProgramContract {
        ProgramContract {
            envelope: ContractEnvelope::new(PROGRAM_LINE, "now", "sha256:frozen"),
            systems: vec![ProgramSystem {
                system_id: "combat_system".into(),
                name: "战斗系统".into(),
                responsibility: "结算".into(),
                source_refs: vec![SpecRef::new("systems/combat")],
            }],
            ..ProgramContract::default()
        }
    }

    fn art() -> ArtContract {
        ArtContract {
            envelope: ContractEnvelope::new(ART_LINE, "now", "sha256:frozen"),
            assets: vec![ArtAsset {
                asset_id: "UI_PlayerIdle".into(),
                name: "玩家待机".into(),
                category: AssetCategory::Animation,
                source_refs: vec![SpecRef::new("entities/guard")],
                ..ArtAsset::default()
            }],
            ..ArtContract::default()
        }
    }

    fn registry() -> AssetRegistry {
        AssetRegistry {
            schema_version: "4.0.0".into(),
            entries: vec![AssetRegistryEntry {
                asset_id: "UI_PlayerIdle".into(),
                naming_pattern: "ui_playeridle.png".into(),
                runtime_path: "Assets/ui_playeridle.png".into(),
                state: AssetLifecycleState::Approved,
                stability: StabilityLevel::Stable,
                source_refs: vec![SpecRef::new("entities/guard")],
            }],
        }
    }

    fn run(markdown: &[MarkdownDocument]) -> AuthorityOrderReport {
        let rules = NamingRules::default();
        validate_authority_order(&AuthorityOrderInput {
            spec: &spec(),
            program: &program(),
            art: &art(),
            registry: &registry(),
            markdown,
            naming: &rules,
        })
    }

    #[test]
    fn authority_table_is_ordered_from_source_to_rendering() {
        let order = authority_order();
        assert_eq!(order[0], AuthoritySource::GameSpec);
        assert_eq!(order[6], AuthoritySource::Markdown);
        for window in order.windows(2) {
            assert!(
                window[0].priority() < window[1].priority(),
                "{:?} 应比 {:?} 权威",
                window[0],
                window[1]
            );
        }
        assert!(AuthoritySource::Markdown.is_rendering_only());
        assert!(!AuthoritySource::GameSpec.is_rendering_only());
    }

    #[test]
    fn markdown_that_only_renders_contract_facts_passes() {
        let document = MarkdownDocument::new(
            "art_requirements.md",
            "本段渲染自 `contract.json`，请勿手改。\n\n资产 `UI_PlayerIdle` 锚定 `entities/guard`，\
             归属系统 `combat_system`。",
        );
        let report = run(&[document]);
        assert!(report.passed(), "{:?}", report.findings);
        // `contract.json` 与 `combat_system` 归不了类（前者是文件名，后者没有形态特征），
        // 真正被核对的是 `UI_PlayerIdle` 与 `entities/guard` 两条。
        assert_eq!(report.checked_markdown_claims, 2);
        assert!(report.summary().contains("阻塞发现 0 条"));
    }

    /// 覆盖边界的**显式**记账：程序线 id 没有形态特征，Markdown 里凭空多一个系统名检不出来。
    ///
    /// 写成测试而不是只写进注释，是为了让这条缝在收窄时（给程序线 id 定命名空间前缀）
    /// 必然有人回到这里改断言——而不是悄悄地一直漏下去。
    #[test]
    fn program_side_ids_without_a_shape_are_a_known_blind_spot() {
        let report = run(&[MarkdownDocument::new(
            "program_requirements.md",
            "另有 `ghost_system` 负责收尾（契约里并没有这个系统）。",
        )]);
        assert_eq!(
            report.checked_markdown_claims, 0,
            "`ghost_system` 与普通 snake_case 词汇无法区分，当前规则不把它当事实声明"
        );
        assert!(
            report.passed(),
            "因此它现在不会被拦下——这是已登记的覆盖边界"
        );
    }

    /// 负例：叙述里凭空多出一个资产，契约里查无此物 → 必须被拦下。
    #[test]
    fn markdown_only_fact_is_blocked() {
        let document = MarkdownDocument::new(
            "art_requirements.md",
            "另需 `UI_HudTimer` 作为倒计时组件（本条只写在文档里）。",
        );
        let report = run(&[document]);
        assert!(!report.passed());
        let finding = report
            .blocking_findings()
            .into_iter()
            .find(|item| item.code == AuthorityFindingCode::MarkdownOnlyFact)
            .expect("必须报出 Markdown-only 事实");
        assert_eq!(finding.subject, "UI_HudTimer");
        assert_eq!(finding.source, AuthoritySource::Markdown);
        assert!(finding.blocking);
        assert!(finding.detail.contains("JSON"), "{}", finding.detail);
    }

    /// 负例：Markdown 引用了真源里没有的锚点，同样是 Markdown-only 事实。
    #[test]
    fn markdown_only_spec_anchor_is_blocked() {
        let report = run(&[MarkdownDocument::new(
            "program_requirements.md",
            "本能力锚定 `mechanics/ghost_rule`。",
        )]);
        assert!(!report.passed());
        assert_eq!(
            report.blocking_findings()[0].subject,
            "mechanics/ghost_rule"
        );
    }

    /// 负例：契约锚点在真源里不存在 = 下游发明设计事实（铁律①）。
    #[test]
    fn contract_anchor_outside_the_source_is_blocked() {
        let mut invented = program();
        invented.systems[0].source_refs = vec![SpecRef::new("systems/not_in_spec")];
        let rules = NamingRules::default();
        let report = validate_authority_order(&AuthorityOrderInput {
            spec: &spec(),
            program: &invented,
            art: &art(),
            registry: &registry(),
            markdown: &[],
            naming: &rules,
        });
        assert!(!report.passed());
        let finding = &report.blocking_findings()[0];
        assert_eq!(finding.code, AuthorityFindingCode::AnchorNotInSource);
        assert_eq!(finding.source, AuthoritySource::ProgramContract);
        assert_eq!(report.checked_contract_anchors, 3);
    }

    /// 负例：美术线有资产而命名权威没登记（第 4 层不得压过第 5 层）。
    #[test]
    fn asset_missing_from_registry_is_blocked() {
        let rules = NamingRules::default();
        let report = validate_authority_order(&AuthorityOrderInput {
            spec: &spec(),
            program: &program(),
            art: &art(),
            registry: &AssetRegistry::default(),
            markdown: &[],
            naming: &rules,
        });
        assert!(!report.passed());
        assert_eq!(
            report.blocking_findings()[0].code,
            AuthorityFindingCode::AssetNotRegistered
        );
    }

    #[test]
    fn claim_extraction_only_picks_recognisable_shapes() {
        let rules = NamingRules::default();
        let document = MarkdownDocument::new(
            "doc.md",
            "文件 `document.md` 由 `contract.json` 渲染；资产 `VFX_Boom`、状态 \
             `STATE-VFX_Boom-idle`、锚点 `systems/combat`、说明 `请勿手改`。",
        );
        let tokens: Vec<String> = extract_fact_claims(&document, &rules)
            .into_iter()
            .map(|claim| claim.token)
            .collect();
        assert_eq!(
            tokens,
            vec!["VFX_Boom", "STATE-VFX_Boom-idle", "systems/combat"]
        );
    }

    /// 未闭合的反引号不许把后面整段吞成一个「事实」。
    #[test]
    fn unclosed_backtick_does_not_swallow_the_rest_of_the_line() {
        let rules = NamingRules::default();
        let document = MarkdownDocument::new("doc.md", "残缺的 `UI_Broken\n下一行 `UI_PlayerIdle`");
        let tokens: Vec<String> = extract_fact_claims(&document, &rules)
            .into_iter()
            .map(|claim| claim.token)
            .collect();
        assert_eq!(tokens, vec!["UI_PlayerIdle"]);
    }

    #[test]
    fn report_round_trips_through_json() {
        let report = run(&[MarkdownDocument::new("doc.md", "`UI_HudTimer`")]);
        let json = serde_json::to_string_pretty(&report).expect("序列化");
        let back: AuthorityOrderReport = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, report);
    }

    /// 旧档兼容：只有 findings 的历史报告照旧可读，计数落 0。
    #[test]
    fn legacy_report_without_counters_parses() {
        let legacy = r#"{"findings":[]}"#;
        let parsed: AuthorityOrderReport = serde_json::from_str(legacy).expect("旧档应可解析");
        assert_eq!(parsed.checked_markdown_claims, 0);
        assert!(parsed.passed());
    }
}
