import { readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { findSourceProjectRoot, safeProjectJoin } from "./project-root.mjs";

const webRoot = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const sourceProject = await findSourceProjectRoot(webRoot);
const designDataRoot = await safeProjectJoin(sourceProject.root, "knowledge/design_data");
const localesRoot = join(webRoot, "src", "locales");
const outputPath = join(localesRoot, "design-content.generated.js");
const checkOnly = process.argv.includes("--check");
const hanPattern = /\p{Script=Han}/u;

const ACRONYMS = new Map([
  ["ai", "AI"], ["api", "API"], ["ar", "AR"], ["dlc", "DLC"], ["fps", "FPS"],
  ["hud", "HUD"], ["kpi", "KPI"], ["mmo", "MMO"], ["npc", "NPC"], ["pve", "PvE"],
  ["pvp", "PvP"], ["qa", "QA"], ["sdk", "SDK"], ["ui", "UI"], ["ugc", "UGC"],
  ["ux", "UX"], ["vr", "VR"], ["xr", "XR"],
]);

const GAMEPLAY_ENGLISH = {
  input_control: { name: "Input and Control System", description: "Defines how players issue input, how the game responds, and the boundaries for tolerance, pacing, and feedback." },
  action_rule: { name: "Action Rules System", description: "Defines the conditions, costs, rewards, limits, and failure outcomes of player actions." },
  objective: { name: "Objective System", description: "Defines how primary, secondary, optional, hidden, and failure objectives drive player behavior." },
  settlement: { name: "Resolution System", description: "Defines victory, defeat, scoring, rewards, penalties, retries, and the close of each session loop." },
  progression: { name: "Progression System", description: "Defines how levels, abilities, equipment, collections, or account growth create long-term goals." },
  buildcraft: { name: "Buildcraft System", description: "Defines build components, constraints, combinations, counters, and update cadence to create strategic space." },
  randomness: { name: "Randomness System", description: "Defines random entry points, weights, guarantees, controllable randomness, and result feedback." },
  meta_structure: { name: "Meta Structure System", description: "Defines preparation, persistent progression, collections, settlement carryover, and transitions between runs." },
  resource_economy: { name: "Resource and Economy System", description: "Defines resource production, consumption, exchange, scarcity, and economic feedback that supports play decisions." },
  social_competition: { name: "Social and Competition System", description: "Defines the role of cooperation, competition, ranking, matchmaking, community interaction, and asynchronous rivalry." },
  content_delivery: { name: "Content Delivery System", description: "Defines how levels, quests, events, challenges, and seasonal content enter the player loop." },
  liveops_event: { name: "Live Events and Version System", description: "Defines limited-time events, release cadence, operational goals, and long-term retention support." },
};

// Mirrors apps/desktop-tauri/src/design_specs.rs::fallback_design_specs.
// These entries keep the workbench language-pure when packaged design data is unavailable.
const FALLBACK_TAXONOMY = [
  { domainId: "product_positioning_design", zhName: "产品定位", enName: "Product Positioning", zhDescription: "明确目标用户、平台与产品承诺", enDescription: "Defines target users, platforms, and the product promise." },
  { domainId: "core_experience_design", zhName: "核心体验", enName: "Core Experience", zhDescription: "定义玩家体验目标与核心循环", enDescription: "Defines player experience goals and the core loop." },
  { domainId: "gameplay_system_design", zhName: "玩法系统", enName: "Gameplay Systems", zhDescription: "建立可执行的玩法与系统规则", enDescription: "Establishes executable gameplay and system rules." },
  { domainId: "content_design", zhName: "内容设计", enName: "Content Design", zhDescription: "规划关卡、角色、叙事与内容节奏", enDescription: "Plans levels, characters, narrative, and content pacing." },
  { domainId: "economy_monetization_design", zhName: "经济与商业化", enName: "Economy and Monetization", zhDescription: "定义资源循环、成长与商业边界", enDescription: "Defines resource loops, progression, and commercial boundaries." },
  { domainId: "ux_interface_design", zhName: "交互界面", enName: "UX and Interface", zhDescription: "定义信息架构、输入与反馈", enDescription: "Defines information architecture, input, and feedback." },
  { domainId: "presentation_feel_design", zhName: "表现与手感", enName: "Presentation and Feel", zhDescription: "定义视听表现和操作手感", enDescription: "Defines audiovisual presentation and control feel." },
  { domainId: "balance_design", zhName: "数值平衡", enName: "Balance", zhDescription: "定义数值模型与平衡验证", enDescription: "Defines numerical models and balance validation." },
  { domainId: "social_community_design", zhName: "社交社区", enName: "Social and Community", zhDescription: "定义社交关系与社区机制", enDescription: "Defines social relationships and community systems." },
  { domainId: "retention_lifecycle_design", zhName: "留存生命周期", enName: "Retention and Lifecycle", zhDescription: "定义长期目标与回流路径", enDescription: "Defines long-term goals and return paths." },
  { domainId: "liveops_version_design", zhName: "运营版本", enName: "Live Operations and Releases", zhDescription: "定义版本节奏与活动框架", enDescription: "Defines release cadence and event frameworks." },
  { domainId: "data_validation_design", zhName: "数据验证", enName: "Data and Validation", zhDescription: "定义指标、埋点与验证方式", enDescription: "Defines metrics, instrumentation, and validation methods." },
  { domainId: "compliance_risk_design", zhName: "合规风险", enName: "Compliance and Risk", zhDescription: "识别合规、平台与制作风险", enDescription: "Identifies compliance, platform, and production risks." },
  { domainId: "documentation_collaboration_design", zhName: "文档协作", enName: "Documentation and Collaboration", zhDescription: "定义交接、评审与变更规则", enDescription: "Defines handoff, review, and change rules." },
  { domainId: "release_growth_design", zhName: "发布增长", enName: "Release and Growth", zhDescription: "定义发布渠道与增长策略", enDescription: "Defines release channels and growth strategies." },
  { domainId: "launch_readiness_design", zhName: "上线准备", enName: "Launch Readiness", zhDescription: "定义上线门禁与应急预案", enDescription: "Defines launch gates and incident response plans." },
];

const FALLBACK_CHECKLIST = [
  { itemId: "goal", zhLabel: "目标与边界", enLabel: "Goals and Boundaries" },
  { itemId: "decision", zhLabel: "关键方案", enLabel: "Key Approach" },
  { itemId: "acceptance", zhLabel: "验收信号", enLabel: "Acceptance Signals" },
];

const FALLBACK_DEPTH_OPTIONS = [
  { optionId: "focused", zhLabel: "聚焦", enLabel: "Focused" },
  { optionId: "balanced", zhLabel: "平衡", enLabel: "Balanced" },
  { optionId: "deep", zhLabel: "深入", enLabel: "Deep" },
];

const chinese = {};
const english = {};
const requiredEnglishKeys = new Set();
const consumedOverrides = new Set();

const overrides = await loadEnglishOverrides();
await loadDomains();
await loadTemplates();
await loadGameplaySystems();
loadFallbackTaxonomy();

const missing = [...requiredEnglishKeys].filter((key) => !Object.hasOwn(overrides, key));
if (missing.length > 0) {
  throw new Error(`English design-content translations are missing:\n${missing.join("\n")}`);
}
for (const key of requiredEnglishKeys) {
  const value = String(overrides[key] ?? "").trim();
  if (!value || hanPattern.test(value)) {
    throw new Error(`Invalid English design-content translation: ${key}=${JSON.stringify(value)}`);
  }
  setMessage(english, key, value);
  consumedOverrides.add(key);
}
const extra = Object.keys(overrides).filter((key) => !consumedOverrides.has(key));
if (extra.length > 0) {
  throw new Error(`Unused English design-content translations:\n${extra.join("\n")}`);
}
assertFallbackTaxonomyCoverage();

const output = `// Generated by scripts/generate-design-content.mjs. Do not edit manually.\nexport const designContentMessages = ${JSON.stringify(
  { "zh-CN": sortObject(chinese), "en-US": sortObject(english) },
  null,
  2,
)};\n`;

if (checkOnly) {
  const current = await readFile(outputPath, "utf8").catch(() => "");
  if (current !== output) {
    throw new Error("design-content.generated.js is stale; run npm run design-content");
  }
  console.log(`design content check passed: ${Object.keys(chinese).length} localized values`);
} else {
  await writeFile(outputPath, output, "utf8");
  console.log(`generated design content: ${Object.keys(chinese).length} localized values`);
}

async function loadEnglishOverrides() {
  const files = (await readdir(localesRoot))
    .filter((name) => /^design-content\.en-US\.part\d+\.json$/u.test(name))
    .sort();
  const merged = {};
  for (const name of files) {
    const payload = JSON.parse(await readFile(join(localesRoot, name), "utf8"));
    for (const [key, value] of Object.entries(payload)) {
      if (Object.hasOwn(merged, key)) {
        throw new Error(`Duplicate English design-content key: ${key}`);
      }
      merged[key] = value;
    }
  }
  return merged;
}

async function loadDomains() {
  const root = join(designDataRoot, "domains");
  const files = (await readdir(root)).filter((name) => name.endsWith(".json")).sort();
  for (const name of files) {
    const payload = JSON.parse(await readFile(join(root, name), "utf8"));
    const domainId = String(payload.domain?.id ?? "").trim();
    addRequired(
      `content.domain.${domainId}.name`,
      payload.domain?.name,
      `content.domain.${domainId}.description`,
      payload.domain?.description,
    );
    for (const node of payload.nodes ?? []) {
      const nodeId = String(node.id ?? "").trim();
      addRequired(
        `content.node.${nodeId}.name`,
        node.name,
        `content.node.${nodeId}.description`,
        node.description,
      );
      for (const item of node.checklist ?? []) {
        const itemId = String(item.id ?? "").trim();
        addRequired(`content.checklist.${nodeId}.${itemId}.label`, item.label);
        addOptionGroups(item.optionGroups);
      }
    }
  }
}

async function loadTemplates() {
  const root = join(designDataRoot, "templates");
  const files = (await readdir(root)).filter((name) => name.endsWith(".json")).sort();
  for (const name of files) {
    const payload = JSON.parse(await readFile(join(root, name), "utf8"));
    addOptionGroups(payload.optionGroups);
  }
}

function addOptionGroups(groups = []) {
  for (const group of groups ?? []) {
    const groupId = String(group.id ?? "").trim();
    const groupKey = `content.group.${groupId}.label`;
    setMessage(chinese, groupKey, group.label);
    setMessage(english, groupKey, humanizeId(groupId));
    for (const option of group.options ?? []) {
      const optionId = String(option.id ?? "").trim();
      const optionKey = `content.option.${groupId}.${optionId}.label`;
      setMessage(chinese, optionKey, option.label);
      setMessage(english, optionKey, humanizeId(optionId));
    }
  }
}

async function loadGameplaySystems() {
  const path = join(designDataRoot, "gameplay_system_options.json");
  const payload = JSON.parse(await readFile(path, "utf8"));
  for (const option of payload.options ?? []) {
    const id = String(option.id ?? "").trim();
    setMessage(chinese, `content.gameplay.${id}.name`, option.name);
    setMessage(chinese, `content.gameplay.${id}.description`, option.mapping_desc);
    setMessage(english, `content.gameplay.${id}.name`, GAMEPLAY_ENGLISH[id]?.name ?? humanizeId(id));
    setMessage(
      english,
      `content.gameplay.${id}.description`,
      GAMEPLAY_ENGLISH[id]?.description ?? `Defines the project's ${humanizeId(id).toLowerCase()}.`,
    );
  }
}

function loadFallbackTaxonomy() {
  for (const entry of FALLBACK_TAXONOMY) {
    const nodeId = `${entry.domainId}_core`;
    addLocalized(`content.node.${nodeId}.name`, entry.zhName, entry.enName);
    addLocalized(
      `content.node.${nodeId}.description`,
      entry.zhDescription,
      entry.enDescription,
    );
    for (const item of FALLBACK_CHECKLIST) {
      addLocalized(
        `content.checklist.${nodeId}.${item.itemId}.label`,
        item.zhLabel,
        item.enLabel,
      );
    }
  }
  addLocalized("content.group.depth.label", "设计深度", "Design Depth");
  for (const option of FALLBACK_DEPTH_OPTIONS) {
    addLocalized(
      `content.option.depth.${option.optionId}.label`,
      option.zhLabel,
      option.enLabel,
    );
  }
}

function assertFallbackTaxonomyCoverage() {
  const expectedDomainIds = new Set(FALLBACK_TAXONOMY.map((entry) => entry.domainId));
  if (expectedDomainIds.size !== 16) {
    throw new Error(`Fallback taxonomy must contain 16 unique domains, found ${expectedDomainIds.size}`);
  }

  const expectedKeys = ["content.group.depth.label"];
  for (const entry of FALLBACK_TAXONOMY) {
    expectedKeys.push(
      `content.domain.${entry.domainId}.name`,
      `content.domain.${entry.domainId}.description`,
      `content.node.${entry.domainId}_core.name`,
      `content.node.${entry.domainId}_core.description`,
      ...FALLBACK_CHECKLIST.map(
        (item) => `content.checklist.${entry.domainId}_core.${item.itemId}.label`,
      ),
    );
  }
  expectedKeys.push(
    ...FALLBACK_DEPTH_OPTIONS.map(
      (option) => `content.option.depth.${option.optionId}.label`,
    ),
  );

  const missing = expectedKeys.filter(
    (key) => !Object.hasOwn(chinese, key) || !Object.hasOwn(english, key),
  );
  if (missing.length > 0) {
    throw new Error(`Fallback taxonomy localization is incomplete:\n${missing.join("\n")}`);
  }
  for (const key of expectedKeys) {
    if (hanPattern.test(english[key])) {
      throw new Error(`Fallback English localization contains Han characters: ${key}`);
    }
  }
}

function addLocalized(key, chineseValue, englishValue) {
  setMessage(chinese, key, chineseValue);
  setMessage(english, key, englishValue);
}

function addRequired(...pairs) {
  for (let index = 0; index < pairs.length; index += 2) {
    const key = pairs[index];
    const value = pairs[index + 1];
    if (!key || key.includes("..")) {
      throw new Error(`Invalid design-content key: ${key}`);
    }
    setMessage(chinese, key, value);
    requiredEnglishKeys.add(key);
  }
}

function setMessage(target, key, value) {
  const normalized = String(value ?? "").trim();
  if (!key || !normalized) {
    throw new Error(`Empty design-content value: ${key}`);
  }
  if (Object.hasOwn(target, key) && target[key] !== normalized) {
    throw new Error(`Conflicting design-content value: ${key}`);
  }
  target[key] = normalized;
}

function humanizeId(value) {
  return String(value ?? "")
    .split(/[_-]+/u)
    .filter(Boolean)
    .map((word) => ACRONYMS.get(word.toLowerCase()) ?? `${word[0]?.toUpperCase() ?? ""}${word.slice(1)}`)
    .join(" ");
}

function sortObject(value) {
  return Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right)));
}
