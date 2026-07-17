import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_SHELL_STATE,
  TASKS,
  createShellModel,
  createShellRefreshScheduler,
  formatProgress,
  getShellState,
  listenForShutdownErrors,
} from "../src/main.js";
import {
  SHELL_THEME,
  THEME_TOKENS,
  applyThemeTokens,
  normalizeShellThemeTokens,
} from "../src/theme.js";
import { setLanguageMode, t } from "../src/i18n.js";
import {
  buildAutosaveDesignRequest,
  buildChecklistRequest,
  buildDeleteTemplateRequest,
  buildDesignEntitiesRequest,
  buildDesignExportRequest,
  buildGameplaySystemUpdateRequest,
  buildNodeTextRequest,
  buildOptionRequest,
  buildResetDesignRequest,
  buildSaveTemplateRequest,
  buildTemplateListRequest,
  buildTemplateSelectionRequest,
  createDesignApi,
  createDesignModel,
  DesignWorkbenchController,
  formatL4MissingItem,
  formatQualityViolationMessage,
  localizedTemplateError,
  normalizeDesignView,
  normalizeTemplateList,
  parseDesignEntities,
  templatePresentation,
  unwrapCommandResponse,
} from "../src/features/design.js";
import {
  buildForceAiOutputRequest,
  buildMarkAiInaccurateRequest,
  buildSaveAiArchiveRequest,
  buildSubmitAiTurnRequest,
  createAiInterviewApi,
  createAiInterviewController,
  createAiInterviewModel,
  normalizeAiBackgroundJobs,
  normalizeAiInterviewState,
  normalizeAiStreamEvents,
} from "../src/features/ai-interview.js";
import {
  API_KEY_MASK,
  applyApiKeyEdit,
  buildAiConfigSaveRequest,
  buildNewApiEntry,
  configTypesForCategory,
  createAiConfigModel,
  entryFieldMode,
  maskApiKey,
  normalizeAiConfig,
  redactAiConfigForDisplay,
  selectedNativeFilePath,
  setAiConfigDescriptors,
  validateExtraJsonText,
} from "../src/features/ai-config.js";
import {
  buildConfirmStyleRequest,
  buildReadStep07PreviewRequest,
  buildResumePipelineRequest,
  buildRunPipelineRequest,
  createPipelineModel,
  normalizeBoundedCompletion,
  normalizePipelineIssues,
  normalizeSemanticQuality,
  normalizePipelineView,
  createPipelineImageObjectUrl,
  pipelineImageBlob,
} from "../src/features/pipeline.js";
import {
  buildEditorExecutablePickerRequest,
  buildProjectEditorValidationRequest,
  buildProjectConfigSaveRequest,
  buildProjectDirectoryPickerRequest,
  buildProjectInspectionRequest,
  buildProjectPreflightRequest,
  buildProjectRelinkRequest,
  buildUnityEditorDiscoveryRequest,
  buildStylePromptMessages,
  buildStylePromptOverrideRequest,
  engineLabel,
  editorCandidateMatchLabel,
  editorCandidateSourceLabel,
  normalizePreflightReport,
  normalizeEditorSelectionValidation,
  normalizeProjectConfig,
  normalizeProjectEnvironmentInspection,
  normalizeStylePromptOptions,
  normalizeUnityEditorCandidates,
  parseStylePromptResponse,
  preflightFixActions,
} from "../src/features/settings-style.js";
import {
  buildAddSdkRequest,
  buildAnalyzePatchRequest,
  buildCreateBlankSaveRequest,
  buildCreateSaveRequest,
  buildDeleteSaveRequest,
  buildLoadSaveRequest,
  buildOpenSaveDirectoryRequest,
  buildPackageRequestFromSources,
  buildProjectStateFromDesignView,
  buildReadLogEntriesRequest,
  buildRenameSaveRequest,
  buildSaveProjectRequest,
  buildUpdateSdkReviewStatusRequest,
  commandDiagnostics,
  createUtilityPanelApis,
  createUtilityPanelsController,
  filterLogEntries,
  formatSaveCommandError,
  formatLogJsonl,
  formatSaveBytes,
  formatSaveTimestamp,
  normalizeLogEntries,
  normalizePackageView,
  normalizePatchRecords,
  normalizeSaveIndex,
  normalizeSdkSpecs,
  refreshAfterCommittedSave,
  validatePatchRequest,
  validateSaveName,
  validateSdkName,
} from "../src/features/utility-panels.js";
import {
  sampleAiInterviewState,
  sampleAiConfig,
  sampleAiConfigDescriptors,
  sampleDesignView,
  sampleLogEntries,
  samplePackageViewBlocked,
  samplePatchRecords,
  samplePipelineView,
  sampleProjectConfig,
  sampleSaveIndex,
  sampleSdkSpecs,
  sampleStylePromptResponse,
  sampleTemplateList,
} from "./fixtures.mjs";
import {
  findSourceProjectRoot,
  openSourceProjectRoot,
  safeProjectJoin,
} from "./project-root.mjs";
import { clear, el } from "../src/shared/dom.js";
import { asArray, read } from "../src/shared/value.js";

const root = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const src = join(root, "src");
const target = process.argv[2] ?? "all";
const [html, css, pipelineJs, aiConfigJs, settingsStyleJs, uiGateJs] = await Promise.all([
  readFile(join(src, "index.html"), "utf8"),
  readFile(join(src, "styles.css"), "utf8"),
  readFile(join(src, "features", "pipeline.js"), "utf8"),
  readFile(join(src, "features", "ai-config.js"), "utf8"),
  readFile(join(src, "features", "settings-style.js"), "utf8"),
  readFile(join(root, "scripts", "ui-gate.mjs"), "utf8"),
]);

if (target === "all" || target === "project-root") {
  await testProjectRoot(root);
}

if (target === "all" || target === "shell") {
  await testShell(html, css, uiGateJs);
}

if (target === "all" || target === "design") {
  await testDesign(html, css);
}

if (target === "all" || target === "ai-interview") {
  await testAiInterview(html, css);
}

if (target === "all" || target === "pipeline") {
  testPipeline(html, css, pipelineJs);
}

if (target === "all" || target === "utility-panels") {
  await testUtilityPanels(html, css);
}

if (target === "all" || target === "ai-config") {
  testAiConfig(html, css, aiConfigJs);
}

if (target === "all" || target === "settings-style") {
  testSettingsStyle(html, css, pipelineJs, settingsStyleJs);
}

if (!["all", "project-root", "shell", "design", "ai-interview", "pipeline", "utility-panels", "ai-config", "settings-style"].includes(target)) {
  throw new Error(`unsupported test target: ${target}`);
}

console.log(`web ${target} unit checks passed`);

async function testProjectRoot(webRoot) {
  const sourceProject = await findSourceProjectRoot(webRoot);
  assert(sourceProject.manifest.projectId === "autodesignmaker-rust-v2", "source project id mismatch");
  await Promise.all([
    readFile(await safeProjectJoin(sourceProject.root, "knowledge/resource-manifest.json")),
    readFile(await safeProjectJoin(sourceProject.root, "pipeline/artifact_layer/registry.json")),
    readFile(await safeProjectJoin(sourceProject.root, "testdata/ui_baselines/index.json")),
  ]);
  await assertRejects(
    () => safeProjectJoin(sourceProject.root, "../parent-data.json"),
    "project root safe join must reject parent traversal",
  );

  const fixtureRoot = await mkdtemp(join(tmpdir(), "adm-newrust-project-root-"));
  try {
    const renamedRoot = join(fixtureRoot, "独立 项目-renamed");
    await writeProjectRootFixture(renamedRoot);
    const nested = join(renamedRoot, "web", "scripts");
    await mkdir(nested, { recursive: true });
    const discovered = await findSourceProjectRoot(nested);
    assert(discovered.root === renamedRoot, "renamed Unicode source root was not discovered");

    const missingLockRoot = join(fixtureRoot, "missing-lock");
    await writeProjectRootFixture(missingLockRoot);
    await rm(join(missingLockRoot, "web", "package-lock.json"));
    await assertRejects(
      () => openSourceProjectRoot(missingLockRoot),
      "source root must reject a missing declared lockfile",
    );

    const escapeRoot = join(fixtureRoot, "escape-marker");
    await writeProjectRootFixture(escapeRoot, { resourceManifest: "../outside.json" });
    await assertRejects(
      () => openSourceProjectRoot(escapeRoot),
      "source root must reject marker paths that escape the checkout",
    );

    const outerRoot = join(fixtureRoot, "valid-outer");
    await writeProjectRootFixture(outerRoot);
    const invalidChild = join(outerRoot, "invalid-child");
    const invalidStart = join(invalidChild, "crates", "demo");
    await mkdir(invalidStart, { recursive: true });
    await writeFile(join(invalidChild, ".project_root"), "{}\n", "utf8");
    await assertRejects(
      () => findSourceProjectRoot(invalidStart),
      "invalid nearest marker must not fall through to a parent project",
    );
  } finally {
    await rm(fixtureRoot, { recursive: true, force: true });
  }
}

async function writeProjectRootFixture(rootPath, markerOverrides = {}) {
  await mkdir(join(rootPath, "knowledge"), { recursive: true });
  await mkdir(join(rootPath, "web"), { recursive: true });
  await Promise.all([
    writeFile(join(rootPath, "Cargo.toml"), "[workspace]\nmembers = []\n", "utf8"),
    writeFile(join(rootPath, "Cargo.lock"), "# fixture\n", "utf8"),
    writeFile(join(rootPath, "web", "package-lock.json"), "{}\n", "utf8"),
    writeFile(
      join(rootPath, "knowledge", "resource-manifest.json"),
      `${JSON.stringify({ schemaVersion: 1, projectId: "autodesignmaker-rust-v2", groups: [] }, null, 2)}\n`,
      "utf8",
    ),
  ]);
  const marker = {
    schemaVersion: 1,
    kind: "source-project-root",
    projectId: "autodesignmaker-rust-v2",
    workspaceManifest: "Cargo.toml",
    lockfiles: ["Cargo.lock", "web/package-lock.json"],
    resourceManifest: "knowledge/resource-manifest.json",
    ...markerOverrides,
  };
  await writeFile(join(rootPath, ".project_root"), `${JSON.stringify(marker, null, 2)}\n`, "utf8");
}

async function testShell(html, css, uiGateJs) {
  assert(TASKS.length === 6, "shell must expose six task areas");
  for (const task of TASKS) {
    assert(html.includes(`data-route="${task.id}"`), `missing route button: ${task.id}`);
    assert(html.includes(`data-panel="${task.id}"`), `missing route panel: ${task.id}`);
  }

  for (const token of [
    "--bg: #f3f6f8",
    "--surface: #ffffff",
    "--primary: #2563eb",
    "--success: #0f8a5f",
    "--warning: #b45309",
    "--danger: #b42318",
    "--dark: #17212b",
  ]) {
    assert(css.includes(token), `missing CSS token ${token}`);
  }

  assert(!html.includes("Placeholder route"), "placeholder shell text must be removed");
  for (const action of [
    "close-template-browser",
    "close-save-template",
    "close-save-manager",
    "close-project-config",
    "close-ai-config",
    "close-style-prompt-editor",
  ]) {
    assert(!html.includes(`data-action="${action}"`), `duplicate modal header action remains: ${action}`);
  }
  for (const action of [
    "cancel-template-browser",
    "cancel-save-template",
    "cancel-save-manager",
    "cancel-project-config",
    "cancel-ai-config",
    "cancel-style-prompt-editor",
  ]) {
    assert(html.includes(`data-action="${action}"`), `modal footer action is missing: ${action}`);
  }
  assert(html.includes('data-theme="adm-light"'), "desktop shell theme marker is required");
  assert(html.includes("bottom-status-bar"), "bottom status bar is required");
  assert(css.includes(".secondary-button"), "dynamic secondary buttons need shared styling");
  assert(css.includes("button:focus-visible"), "keyboard focus styling is required");
  assert(css.includes("scrollbar-width: thin"), "task navigation must remain discoverable when narrow");
  assert(css.includes("flex: 0 0 auto"), "compact retry actions must not collapse on narrow screens");
  assert(
    uiGateJs.includes("Playwright is required for the UI gate"),
    "UI gate must fail closed when Playwright is unavailable",
  );
  assert(!uiGateJs.includes("spawnSync"), "UI gate must not use an unbounded system-browser fallback");
  assert(uiGateJs.includes("setDefaultNavigationTimeout"), "UI gate navigation needs a timeout");
  assert(formatProgress(DEFAULT_SHELL_STATE.progress) === t("shell.progress", { passed: 0, total: 15 }), "progress format changed");
  assert(DEFAULT_SHELL_STATE.window.minWidth === 1180, "minimum desktop width should match Python shell");
  assert(DEFAULT_SHELL_STATE.window.minHeight === 720, "minimum desktop height should match Python shell");
  assert(SHELL_THEME.defaultWindowWidth === 1280, "default desktop width changed");
  assert(THEME_TOKENS.primary === "#2563EB", "primary theme token changed");
  const applied = [];
  const fakeDocument = {
    documentElement: {
      style: {
        setProperty: (name, value) => applied.push([name, value]),
      },
    },
  };
  applyThemeTokens(fakeDocument);
  assert(applied.some(([name, value]) => name === "--primary" && value === "#2563eb"), "theme apply should set primary CSS token");
  const normalizedTheme = normalizeShellThemeTokens([{ name: "primary_soft", value: "#abc123" }]);
  assert(normalizedTheme.primarySoft === "#abc123", "shell theme tokens should normalize snake case names");

  const model = createShellModel();
  assert(model.activeRoute === "design", "default route should be design");
  assert(model.switchRoute("pipeline") === "pipeline", "route switch failed");
  assertThrows(() => model.switchRoute("missing"), "unknown route should throw");
  assert(read({ snake_value: 4 }, "snakeValue", "snake_value") === 4, "shared read should accept snake case");
  assert(asArray("not-an-array").length === 0, "shared array normalization should reject scalars");
  const children = [
    { remove: () => children.shift() },
    { remove: () => children.shift() },
  ];
  clear({ get firstChild() { return children[0] ?? null; } });
  assert(children.length === 0, "shared DOM clear should remove every child");
  const previousDocument = globalThis.document;
  globalThis.document = {
    createElement: (tag) => ({ tag, className: "", textContent: "" }),
  };
  const sharedElement = el("button", "shared-button", "Action");
  if (previousDocument === undefined) {
    delete globalThis.document;
  } else {
    globalThis.document = previousDocument;
  }
  assert(
    sharedElement.tag === "button" &&
      sharedElement.className === "shared-button" &&
      sharedElement.textContent === "Action",
    "shared DOM element creation changed",
  );

  const visibilityListeners = new Map();
  let scheduledRefresh = null;
  let activeRefreshes = 0;
  let maxActiveRefreshes = 0;
  const refreshResolvers = [];
  const schedulerDocument = {
    hidden: false,
    addEventListener: (event, listener) => visibilityListeners.set(event, listener),
    removeEventListener: (event) => visibilityListeners.delete(event),
  };
  const scheduler = createShellRefreshScheduler(
    schedulerDocument,
    () => {
      activeRefreshes += 1;
      maxActiveRefreshes = Math.max(maxActiveRefreshes, activeRefreshes);
      return new Promise((resolve) =>
        refreshResolvers.push(() => {
          activeRefreshes -= 1;
          resolve();
        }),
      );
    },
    {
      intervalMs: 100,
      setTimeout: (callback) => {
        scheduledRefresh = callback;
        return callback;
      },
      clearTimeout: (callback) => {
        if (scheduledRefresh === callback) {
          scheduledRefresh = null;
        }
      },
    },
  );
  scheduler.start();
  assert(refreshResolvers.length === 1, "shell scheduler should refresh immediately");
  visibilityListeners.get("visibilitychange")();
  assert(refreshResolvers.length === 1, "shell scheduler must not overlap an active refresh");
  refreshResolvers[0]();
  await Promise.resolve();
  await Promise.resolve();
  assert(typeof scheduledRefresh === "function", "shell scheduler should wait between refreshes");
  const runScheduledRefresh = scheduledRefresh;
  scheduledRefresh = null;
  runScheduledRefresh();
  assert(refreshResolvers.length === 2, "shell scheduler should run the next serial refresh");
  schedulerDocument.hidden = true;
  visibilityListeners.get("visibilitychange")();
  refreshResolvers[1]();
  await Promise.resolve();
  await Promise.resolve();
  assert(scheduledRefresh === null, "hidden documents must pause shell refresh scheduling");
  schedulerDocument.hidden = false;
  visibilityListeners.get("visibilitychange")();
  assert(refreshResolvers.length === 3, "visible documents should refresh immediately");
  scheduler.stop();
  refreshResolvers[2]();
  await Promise.resolve();
  assert(maxActiveRefreshes === 1, "shell refresh requests must remain non-overlapping");

  globalThis.__TAURI__ = {
    core: {
      invoke: async () => ({
        ok: true,
        data: {
          activeView: "design",
          aiStatus: { label: "AI: 测试", ok: true },
          progress: { passed: 2, total: 15 },
          systemStatus: "系统: 测试",
        },
      }),
    },
  };
  const shellState = await getShellState();
  assert(shellState.aiStatus.label === "AI: 测试", "shell command response should unwrap data");
  let shutdownListener = null;
  const shutdownErrors = [];
  globalThis.__TAURI__.event = {
    listen: async (event, listener) => {
      assert(event === "adm-shutdown-error", "shutdown listener event changed");
      shutdownListener = listener;
      return () => {};
    },
  };
  await listenForShutdownErrors((error) => shutdownErrors.push(error));
  shutdownListener({ payload: "disk full" });
  assert(shutdownErrors[0] === "disk full", "shutdown persistence errors must reach the shell");
  delete globalThis.__TAURI__;
}

async function testDesign(html, css) {
  for (const required of [
    'data-role="domain-list"',
    'data-role="node-list"',
    'data-role="result-output"',
    'data-role="gameplay-systems"',
    'data-action="export-design"',
    'data-action="save-manager"',
    'data-action="template-browser"',
    'data-action="save-template"',
    'data-role="template-browser-modal"',
    'data-role="template-list"',
    'data-role="template-detail"',
    'data-action="apply-template"',
    'data-action="delete-template"',
    'data-role="save-template-modal"',
    'data-action="confirm-save-template"',
    'data-action="reset-design"',
    'data-i18n="state.waitingWorkbench"',
  ]) {
    assert(html.includes(required), `missing design UI marker: ${required}`);
  }
  for (const required of [
    ".domain-card",
    ".node-card",
    ".checklist-item",
    ".entity-editor",
    ".gameplay-systems-panel",
    ".gameplay-system-chip",
    ".design-status-bar",
    ".template-browser-dialog",
    ".template-list-item",
    ".template-detail-grid",
    ".save-template-dialog",
  ]) {
    assert(css.includes(required), `missing design CSS rule: ${required}`);
  }

  const view = normalizeDesignView(sampleDesignView());
  assert(view.projectName === "未命名游戏设计项目", "project name should normalize");
  assert(view.domains.length === 2, "domains should normalize");
  assert(view.nodes[0].checklistItems[0].optionGroups[0].options[0].selected, "options should normalize");
  assert(view.nodes[0].l5EntityCount === 1, "L5 count should normalize");
  assert(view.gameplaySystems.selected.length === 2, "gameplay systems should normalize");
  assert(view.gameplaySystems.custom[0].id === "custom_synergy", "custom gameplay system should normalize");

  const profileInput = {
    project_name: "Profile localization",
    profile: [
      { key: "targetScale", label: "Target Scale", value: "indie" },
      { key: "referenceGame", label: "Reference Game", value: "用户参考 / User reference" },
      { key: "custom_field", label: "自定义字段 / Custom Field", value: "自由文本 / Free text" },
    ],
  };
  setLanguageMode("zh-CN", { notify: false });
  const chineseProfile = normalizeDesignView(profileInput).profile;
  assert(chineseProfile[0].label === "目标规模", "Chinese mode must localize known profile labels");
  assert(chineseProfile[0].displayValue === "独立游戏", "Chinese mode must localize known profile enums");
  assert(chineseProfile[0].value === "indie", "profile enum localization must preserve the protocol value");
  assert(chineseProfile[0].labelIsSystem && chineseProfile[0].valueIsSystem, "known profile labels and enums must be system content");
  assert(chineseProfile[1].label === "参考游戏", "known free-text profile fields must localize their label");
  assert(chineseProfile[1].displayValue === "用户参考 / User reference", "profile free text must remain verbatim");
  assert(chineseProfile[1].labelIsSystem && !chineseProfile[1].valueIsSystem, "free-text values must remain project content");
  assert(!chineseProfile[2].labelIsSystem && !chineseProfile[2].valueIsSystem, "unknown profile fields must remain project content");

  const defaultProfileFields = [
    "businessModel",
    "operationModel",
    "socialModel",
    "platformScope",
    "primaryPlatform",
    "regionScope",
    "targetScale",
    "contentRating",
    "targetSessionBand",
  ];
  const defaultProfile = normalizeDesignView({
    profile: Object.fromEntries(defaultProfileFields.map((key) => [key, "unknown"])),
  }).profile;
  assert(defaultProfile.every((field) => field.displayValue === "未知"), "all default profile fields must localize the unknown sentinel in Chinese");
  assert(!defaultProfile.some((field) => /unknown/i.test(`${field.label} ${field.displayValue}`)), "Chinese default profile display must not leak unknown");

  setLanguageMode("en-US", { notify: false });
  const englishProfile = normalizeDesignView(profileInput).profile;
  assert(englishProfile[0].label === "Target Scale", "English mode must localize known profile labels");
  assert(englishProfile[0].displayValue === "Indie Game", "English mode must localize known profile enums");
  assert(englishProfile[1].displayValue === "用户参考 / User reference", "English mode must preserve profile free text");
  setLanguageMode("zh-CN", { notify: false });

  const catalogInput = {
    project_name: "Custom project",
    domains: [{ domain_id: "product_positioning_design", name: "mixed fallback" }],
    nodes: [{
      node_id: "product_vision_decision",
      domain_id: "product_positioning_design",
      name: "mixed fallback",
      description: "mixed fallback",
      design_note: "用户输入 / User note",
      checklist_items: [{
        item_id: "he_xin_ti_yan_cheng_nuo",
        label: "mixed fallback",
        option_groups: [{
          group_id: "core_feeling_type",
          options: [{ option_id: "tense_choice", label: "mixed fallback" }],
        }],
      }],
    }],
  };
  setLanguageMode("zh-CN", { notify: false });
  const chineseCatalog = normalizeDesignView(catalogInput);
  assert(chineseCatalog.domains[0].name === "立项与产品定位设计", "Chinese mode must localize built-in domains");
  assert(chineseCatalog.nodes[0].name === "项目愿景决策", "Chinese mode must localize built-in nodes");
  assert(chineseCatalog.nodes[0].checklistItems[0].label === "核心体验承诺", "Chinese mode must localize checklist labels");
  assert(chineseCatalog.nodes[0].checklistItems[0].optionGroups[0].label === "核心感受类型", "Chinese mode must localize inline option groups");
  assert(chineseCatalog.nodes[0].checklistItems[0].optionGroups[0].options[0].label === "紧张抉择", "Chinese mode must localize inline options");
  setLanguageMode("en-US", { notify: false });
  const englishCatalog = normalizeDesignView(catalogInput);
  const englishCatalogText = [
    englishCatalog.domains[0].name,
    englishCatalog.nodes[0].name,
    englishCatalog.nodes[0].description,
    englishCatalog.nodes[0].checklistItems[0].label,
    englishCatalog.nodes[0].checklistItems[0].optionGroups[0].label,
    englishCatalog.nodes[0].checklistItems[0].optionGroups[0].options[0].label,
  ].join(" ");
  assert(!/\p{Script=Han}/u.test(englishCatalogText), "English built-in design content must not contain Chinese text");
  assert(englishCatalog.nodes[0].designNote === "用户输入 / User note", "user-authored design text must remain unchanged");

  const knownMissing = "product_vision_decision:he_xin_ti_yan_cheng_nuo:core_feeling_type";
  setLanguageMode("zh-CN", { notify: false });
  assert(
    formatL4MissingItem(knownMissing) === "项目愿景决策 · 核心体验承诺：缺少 核心感受类型",
    "Chinese mode must localize known L4 missing paths",
  );
  assert(
    formatQualityViolationMessage({
      id: "missing_l5_entity_product_vision_decision",
      message: "concrete node is missing L5 designEntities",
    }) === "项目愿景决策：具体节点缺少第五层设计实体",
    "Chinese mode must localize known quality violation IDs",
  );
  assert(
    formatQualityViolationMessage({ message: "node has entity validation errors" }) ===
      "节点存在设计实体校验错误",
    "Chinese mode must localize known quality violation messages",
  );
  setLanguageMode("en-US", { notify: false });
  assert(
    formatL4MissingItem(knownMissing) ===
      "Product Vision Decision / Core Experience Promise: Missing Core Feeling Type",
    "English mode must localize known L4 missing paths",
  );
  assert(
    formatQualityViolationMessage({
      id: "entity_validation_errors_product_vision_decision",
      message: "node has entity validation errors",
    }) === "Product Vision Decision: Node has design entity validation errors",
    "English mode must localize known quality violation IDs",
  );
  const externalText = "用户提供 / vendor detail";
  assert(formatL4MissingItem("vendor:item:group") === "vendor:item:group", "unknown L4 paths must remain unchanged");
  assert(
    formatQualityViolationMessage({ id: "vendor_error", message: externalText }) === externalText,
    "unknown quality violations must remain unchanged",
  );
  setLanguageMode("zh-CN", { notify: false });

  const model = createDesignModel(sampleDesignView());
  assert(model.selectedDomainId === "mechanics", "first domain should be selected");
  assert(model.visibleNodes().length === 2, "domain filter should show mechanics nodes");
  model.setSearch("combat");
  assert(model.visibleNodes().length === 1, "search should narrow nodes");
  model.setSearch("");
  model.setFilter("l4_missing");
  assert(model.visibleNodes().some((node) => node.nodeId === "progression"), "L4 filter should find missing node");
  model.setFilter("risk");
  assert(model.visibleNodes()[0].nodeId === "progression", "risk filter should find risk node");
  assert(model.gameplaySummary().totalWeight === 100, "gameplay weights should summarize");

  assert(buildNodeTextRequest("combat_loop", { designNote: "Updated" }).design_note === "Updated", "text request mismatch");
  assert(buildChecklistRequest("combat_loop", "core_loop", true).checklist[0].checked, "checklist request mismatch");
  assert(buildOptionRequest("combat_loop", "core_loop", "loop_type", "real_time", true).option_updates[0].option_id === "real_time", "option request mismatch");
  assert(parseDesignEntities('{"kind":"loop"}').length === 1, "single entity JSON should wrap into array");
  assert(buildDesignEntitiesRequest("combat_loop", '[{"kind":"loop"}]').design_entities.length === 1, "entity request mismatch");
  assert(buildDesignExportRequest("markdown").include_gameplay_global_view === false, "export request default mismatch");
  assert(buildDesignExportRequest("markdown").artifact_locale === "zh-CN", "export request should use Chinese UI language");
  setLanguageMode("en-US", { notify: false });
  assert(buildDesignExportRequest("markdown").artifact_locale === "en-US", "export request should carry the selected UI language");
  setLanguageMode("zh-CN", { notify: false });
  assert(buildAutosaveDesignRequest().autosave_file.includes("autosave_state.json"), "autosave request mismatch");
  const templateList = normalizeTemplateList(sampleTemplateList());
  assert(templateList.templates.length === 3, "template summaries should normalize");
  assert(templateList.templates[0].templateId === "builtin_indie_ftl_faster_than_light", "template id should normalize");
  assert(templateList.templates[2].canDelete, "custom templates should be deletable");
  assert(!templateList.templates[0].canDelete, "built-in templates must not be deletable");
  setLanguageMode("zh-CN", { notify: false });
  assert(templatePresentation(templateList.templates[0]).name === "超越光速", "Chinese mode should use the built-in template's Chinese name");
  assert(templatePresentation(templateList.templates[0]).summary === templateList.templates[0].summary, "Chinese built-in template summary should use the Chinese source summary");
  setLanguageMode("en-US", { notify: false });
  assert(templatePresentation(templateList.templates[0]).name === "FTL: Faster Than Light", "English mode should use the authoritative game name");
  assert(templatePresentation(templateList.templates[0]).summary.startsWith("FTL"), "English mode should use the English analysis summary");
  setLanguageMode("zh-CN", { notify: false });
  assert(buildTemplateListRequest().include_internal, "template list should include internal records for parity");
  const templateSelection = buildTemplateSelectionRequest("hades", "Template: ");
  assert(templateSelection.template_id === "hades", "template selection request mismatch");
  assert(templateSelection.project_name_prefix === "Template: ", "template selection prefix mismatch");
  assert(!Object.hasOwn(templateSelection, "project_state"), "template selection must not trust client project state");
  assert(buildSaveTemplateRequest("Demo", "indie", true).overwrite, "save template overwrite request mismatch");
  assert(buildDeleteTemplateRequest("custom_demo").template_id === "custom_demo", "delete template request mismatch");
  setLanguageMode("zh-CN", { notify: false });
  assert(localizedTemplateError({ code: "TEMPLATE_NOT_FOUND", message: "raw backend text" }) === "模板不存在或已被删除", "template errors must localize in Chinese mode");
  setLanguageMode("en-US", { notify: false });
  assert(localizedTemplateError({ code: "TEMPLATE_DELETE_FORBIDDEN", message: "后端原文" }) === "Built-in templates cannot be deleted", "template errors must localize in English mode");
  setLanguageMode("zh-CN", { notify: false });
  assert(buildGameplaySystemUpdateRequest("combat", { selected: true, weight: 60, coreLoop: "loop" }).weight === 60, "gameplay update request mismatch");
  assert(buildResetDesignRequest().confirmed, "reset request mismatch");
  assert(unwrapCommandResponse({ ok: true, data: { id: 1 } }).id === 1, "command response unwrap mismatch");
  assertThrows(() => unwrapCommandResponse({ ok: false, error: { code: "FAILED" } }), "command errors should throw");

  const commandCalls = [];
  const designApi = createDesignApi(async (command, payload) => {
    commandCalls.push([command, payload]);
    return { ok: true, data: { ...sampleDesignView(), project_name: payload?.name ?? "Loaded" } };
  });
  await designApi.setProjectName("Edited Project");
  assert(commandCalls[0][0] === "set_project_name", "project name must use the state mutation command");
  assert(commandCalls[0][1].name === "Edited Project", "project name command payload mismatch");
  await designApi.listTemplates(true);
  await designApi.selectTemplate(buildTemplateSelectionRequest("builtin_demo"));
  await designApi.saveTemplate(buildSaveTemplateRequest("Demo", "indie"));
  await designApi.deleteTemplate(buildDeleteTemplateRequest("custom_demo"));
  assert(commandCalls.map(([command]) => command).slice(1).join(",") === "list_templates,select_template,save_template,delete_template", "template API command sequence mismatch");

  const failingDesignApi = createDesignApi(async () => {
    throw new Error("design backend unavailable");
  });
  await assertRejects(
    () => failingDesignApi.load(),
    "design load failures must not be downgraded to an empty project",
  );

  let authoritative = { ...sampleDesignView(), project_name: "First Model" };
  const renderedNames = [];
  const fakeDocument = {
    querySelector: (selector) =>
      selector === '[data-role="project-name"]' ? { value: "Edited Project" } : null,
  };
  const controller = new DesignWorkbenchController(
    fakeDocument,
    {
      load: async () => authoritative,
      setProjectName: async (name) => {
        authoritative = { ...authoritative, project_name: name };
        return authoritative;
      },
    },
    (_document, nextView) => {
      renderedNames.push(nextView.project_name);
      return createDesignModel(nextView);
    },
  );
  await controller.reload();
  const latestView = await controller.latestView({ reload: true });
  assert(latestView.projectName === "Edited Project", "save must read the latest authoritative model");
  assert(renderedNames.at(-1) === "Edited Project", "authoritative reload must redraw the workbench");
  assert(
    buildSaveProjectRequest(controller.view).state.projectName === "Edited Project",
    "save request must not retain the first design model",
  );

  let failedDesignModel = null;
  const failedController = new DesignWorkbenchController(
    fakeDocument,
    { load: async () => Promise.reject(new Error("design load failed")) },
    (_document, nextView) => {
      failedDesignModel = createDesignModel(nextView);
      return failedDesignModel;
    },
  );
  await failedController.reload();
  assert(
    failedDesignModel.view.loadError === "design load failed",
    "design controller must render an explicit load error model",
  );
}

async function testAiInterview(html, css) {
  for (const required of [
    'data-role="ai-interview-panel"',
    'data-role="ai-current-question"',
    'data-action="send-ai-turn"',
    'data-action="force-ai-output"',
    'data-action="mark-ai-inaccurate"',
    'data-action="save-ai-archive"',
    'data-role="ai-stream-timeline"',
    'data-role="ai-background-status"',
    'data-bottom-tab="ai"',
  ]) {
    assert(html.includes(required), `missing AI interview UI marker: ${required}`);
  }
  for (const required of [
    ".ai-message.user",
    ".ai-message.assistant",
    ".ai-message.system",
    ".ai-action-row",
    ".ai-panel-status",
    ".ai-stream-timeline",
    ".ai-background-status",
    ".bottom-tab",
  ]) {
    assert(css.includes(required), `missing AI interview CSS rule: ${required}`);
  }

  const state = normalizeAiInterviewState(sampleAiInterviewState());
  assert(state.currentQuestionText.includes("player promise"), "current question should normalize");
  assert(state.messages.length === 3, "messages should normalize");
  assert(state.streamEvents[0].stage === "completed", "stream events should normalize");
  assert(state.backgroundJobs.mappingStatus === "idle", "background jobs should normalize");
  const model = createAiInterviewModel(sampleAiInterviewState());
  assert(model.currentQuestion.includes("player promise"), "model should expose question text");
  assert(model.inputHint === t("settings.aiInterview.inputHint.answer"), "answer hint should use the active language");
  assert(!model.actionsDisabled, "completed state should not disable actions");
  assert(model.streamText.includes("completed:turn-2"), "model should expose stream timeline");
  assert(model.routeText.includes("mechanics"), "model should expose route overview");
  const running = createAiInterviewModel(sampleAiInterviewState({ status: "running", backendStage: "queued" }));
  assert(running.actionsDisabled, "running state should disable actions");
  const failed = createAiInterviewModel({ lastError: "interview load failed" });
  assert(failed.statusText.includes("interview load failed"), "AI load errors must be visible in status");
  const failingInterviewApi = createAiInterviewApi(async () => {
    throw new Error("interview backend unavailable");
  });
  await assertRejects(
    () => failingInterviewApi.load(),
    "AI interview load failures must not be downgraded to idle state",
  );
  const commandViewState = normalizeAiInterviewState({
    state: sampleAiInterviewState({ status: "completed" }),
    stream_events: [{ stage: "calling_codex", turn_id: "turn-3", message: "calling", running: true }],
    background_jobs: { mapping_status: "pending", summary_correction_status: "needs_revision", active_job_count: 2 },
  });
  assert(commandViewState.streamEvents[0].turnId === "turn-3", "command-view stream events should normalize");
  assert(commandViewState.backgroundJobs.activeJobCount === 2, "command-view background jobs should normalize");
  assert(normalizeAiStreamEvents([{ stage: "calling_codex", turn_id: "turn-1", running: true }])[0].turnId === "turn-1", "stream normalizer should accept snake case");
  assert(normalizeAiBackgroundJobs({ mapping_status: "pending", summary_correction_status: "needs_revision", active_job_count: 2 }).summaryCorrectionStatus === "needs_revision", "background normalizer should accept snake case");
  const controller = createAiInterviewController(sampleAiInterviewState(), {});
  controller.applyCommandResult({
    state: sampleAiInterviewState({ lastManualArchivePath: "ai_archives/manual/test.json" }),
    stream_events: [{ stage: "completed", turn_id: "turn-4", message: "done", running: false }],
  });
  assert(controller.state.lastManualArchivePath.endsWith("test.json"), "controller should apply command state");

  assert(buildSubmitAiTurnRequest("answer").schema_mode === "turn", "submit request schema mismatch");
  assert(buildSubmitAiTurnRequest("answer").user_message === "answer", "submit request message mismatch");
  assert(buildForceAiOutputRequest().schema_mode === "full_output", "force request schema mismatch");
  assert(buildMarkAiInaccurateRequest("combat_loop", "wrong").node_id === "combat_loop", "mark request mismatch");
  assert(buildSaveAiArchiveRequest().archive_path === null, "archive request should default to null path");
}

function testPipeline(html, css, pipelineJs) {
  for (const required of [
    'data-role="pipeline-step-list"',
    'data-role="pipeline-detail"',
    'data-role="style-grid"',
    'data-role="pipeline-runtime-log"',
    'data-role="pipeline-skip-gate"',
    'data-role="pipeline-from-options"',
    'data-role="pipeline-to-options"',
    'data-action="stop-pipeline"',
    'data-action="run-pipeline"',
    'data-action="resume-pipeline"',
    'data-action="export-to-pipeline"',
  ]) {
    assert(html.includes(required), `missing pipeline UI marker: ${required}`);
  }
  assert(
    /<input[^>]+data-role="pipeline-from"[^>]+list="pipeline-from-options"/.test(html),
    "pipeline start stage must be editable with canonical suggestions",
  );
  assert(
    !/<select[^>]+data-role="pipeline-(?:from|to)"/.test(html),
    "pipeline range must not be limited to fixed select controls",
  );
  assert(pipelineJs.includes("confirm-style"), "missing dynamic confirm-style action");
  for (const required of [
    ".step-card",
    ".pipeline-detail",
    ".style-option-grid",
    ".style-image-preview",
    ".runtime-log-pane",
    ".style-confirmed",
    ".bounded-completion-panel",
    ".semantic-quality-panel",
    ".semantic-issue",
    ".pipeline-issue",
  ]) {
    assert(css.includes(required), `missing pipeline CSS rule: ${required}`);
  }
  assert(
    /\.pipeline-shell\s*\{[^}]*grid-template-columns:\s*330px\s+minmax\(0,\s*1fr\)/su.test(css),
    "pipeline step column must be exactly 50% wider than the original 220px column",
  );
  assert(
    /\.step-card\s*\{[^}]*min-height:\s*124px/su.test(css),
    "pipeline step card must be exactly twice the original 62px minimum height",
  );
  assert(
    pipelineJs.includes("const shouldShow = styleGridVisibleForStage(stage)"),
    "Step07 images must be gated by the selected stage",
  );

  const view = normalizePipelineView(samplePipelineView());
  assert(view.stages.length === 15, "pipeline stages should normalize full Step00-14 tree");
  assert(view.currentStageId === "07", "current stage should normalize");
  assert(view.waitingConfirmation, "waiting confirmation should normalize");
  assert(view.styleOptions.length === 3, "style options should normalize");
  const semanticStage = view.stages.find((stage) => stage.stageId === "10");
  assert(semanticStage.semanticQuality.status === "blocked", "semantic quality should normalize");
  assert(semanticStage.semanticQuality.returnTargets[0].returnTarget.includes("Step10"), "semantic return targets should normalize");
  assert(view.stages[0].boundedCompletion.status === "not_called", "completion state should default to not_called");
  assert(!Object.hasOwn(semanticStage, "artifacts"), "pipeline view must discard artifact records");
  assert(!Object.hasOwn(semanticStage, "outputs"), "pipeline view must discard raw outputs");
  assert(
    !JSON.stringify(view).includes("semantic_alignment_report.json"),
    "pipeline view must not retain internal artifact paths",
  );
  assert(semanticStage.errors[0].code === "SEMANTIC_ALIGNMENT_GAP", "pipeline errors should normalize");
  assert(semanticStage.warnings[0].severity === "warning", "pipeline warnings should normalize");
  const model = createPipelineModel(samplePipelineView());
  assert(model.selectedStage().stageId === "07", "model should select current stage");
  assert(
    model.runtimeLines().some((line) => line.includes("等待确认") || line.includes("Waiting for confirmation")),
    "runtime lines should include the localized status",
  );
  assert(buildRunPipelineRequest("07", "10").from_stage_id === "07", "run request from mismatch");
  assert(buildRunPipelineRequest("07", "10").to_stage_id === "10", "run request to mismatch");
  assert(buildRunPipelineRequest("07", "10").artifact_locale === "zh-CN", "run request should use Chinese UI language");
  setLanguageMode("en-US", { notify: false });
  assert(buildRunPipelineRequest("07", "10").artifact_locale === "en-US", "run request should use English UI language");
  setLanguageMode("zh-CN", { notify: false });
  assert(buildRunPipelineRequest("07", "10", { skipManualGates: true }).skip_manual_gates, "run request should include skip manual gates");
  const recoverable = normalizePipelineView({
    ...samplePipelineView(),
    state: {
      ...samplePipelineView().state,
      status: "recoverable",
      attempt_id: "attempt_1",
      attempt_no: 2,
      state_version: 7,
      current_unit_id: "11:program:TASK-002",
    },
    running: false,
    recovery: { run_id: "run_1", attempt_id: "attempt_1", revision: 3, status: "recoverable" },
  });
  assert(recoverable.recovery.revision === 3, "pipeline recovery summary should normalize");
  assert(recoverable.state.currentUnitId === "11:program:TASK-002", "current safe unit should normalize");
  assert(buildResumePipelineRequest(recoverable.recovery).expected_revision === 3, "resume request revision mismatch");
  const completionStage = normalizePipelineView({
    ordered_stage_ids: ["05"],
    stages: [
      {
        stage_id: "05",
        title: "Program Review",
        status: "completed_with_review",
        bounded_completion: {
          status: "confirmed",
          model_config_id: "local-codex",
          attempts: 2,
          risk: "medium",
          confirmation_mode: "attended",
          confirmation_actor: "reviewer",
          confirmation_accepted: "false",
          errors: ["needs review"],
        },
        outputs: { secret: "raw output" },
        artifacts: [{ relative_path: "stage_05/private.json" }],
      },
    ],
  }).stages[0];
  assert(completionStage.boundedCompletion.status === "confirmed", "completion status should normalize");
  assert(completionStage.boundedCompletion.modelConfigId === "local-codex", "completion model should normalize");
  assert(completionStage.boundedCompletion.confirmationAccepted === false, "completion boolean should parse false strings");
  assert(completionStage.boundedCompletion.errors[0] === "needs review", "completion errors should normalize");
  assert(!Object.hasOwn(completionStage, "outputs"), "completion view must still discard raw outputs");
  assert(normalizeBoundedCompletion({ status: "unexpected" }).status === "failed", "unknown completion status should fail closed");
  assert(normalizeSemanticQuality({ return_targets: [{ code: "SEMANTIC_ALIGNMENT_GAP" }] }).returnTargets[0].returnTarget.includes("Step10"), "semantic return fallback should resolve");
  assert(normalizePipelineIssues(["failed"], "error")[0].severity === "error", "issue string should use fallback severity");
  const previewRequest = buildReadStep07PreviewRequest("generated_images/a.png", 1024);
  assert(previewRequest.stage_id === "07", "preview request must be fixed to Step07");
  assert(previewRequest.max_bytes === 1024, "preview byte limit missing");
  const imageBlob = pipelineImageBlob({
      content_type: "image/png",
      encoding: "base64",
      content: "AQID",
      truncated: false,
    });
  assert(imageBlob?.type === "image/png" && imageBlob.size === 3, "valid preview must decode to a PNG Blob");
  const objectUrls = [];
  assert(
    createPipelineImageObjectUrl(
      { content_type: "image/png", encoding: "base64", content: "AQID" },
      { createObjectURL: (blob) => (objectUrls.push(blob), "blob:opaque-preview") },
    ) === "blob:opaque-preview" && objectUrls[0]?.size === 3,
    "valid image preview should produce an opaque object URL",
  );
  assert(
    pipelineImageBlob({
      content_type: "text/plain",
      encoding: "base64",
      content: "aGVsbG8=",
    }) === null,
    "non-image artifact must not produce a preview",
  );
  assert(
    pipelineImageBlob({
      content_type: "image/png",
      encoding: "base64",
      content: "AQID",
      truncated: true,
    }) === null,
    "truncated image artifact must not produce a preview",
  );
  assert(
    pipelineImageBlob({ content_type: "image/png", encoding: "base64", content: "not base64" }) === null,
    "invalid Base64 must not produce a preview",
  );
  assert(!pipelineJs.includes("data:${contentType};base64"), "Step07 must not create a Base64 data URL");
  assert(!pipelineJs.includes("normalizePipelineArtifacts"), "Web must not retain a raw artifact normalizer");
  assert(!pipelineJs.includes('dataset.role = "pipeline-artifacts"'), "pipeline detail must hide internal artifact lists");
  assert(!pipelineJs.includes('dataset.action = "read-pipeline-artifact"'), "pipeline detail must not expose artifact readers");
  assert(!pipelineJs.includes('dataset.role = "pipeline-outputs"'), "pipeline detail must hide raw outputs");
  assert(pipelineJs.includes('dataset.role = "pipeline-issues"'), "pipeline detail must render errors and warnings");
  const confirm = buildConfirmStyleRequest("07", "stylized", "use readable silhouettes");
  assert(confirm.stage_id === "07", "confirm request stage mismatch");
  assert(confirm.message.includes("stylized"), "confirm request should include option id");
}

async function testUtilityPanels(html, css) {
  for (const required of [
    'data-role="patch-request"',
    'data-role="patch-table"',
    'data-role="package-output"',
    'data-role="log-level"',
    'data-role="log-table"',
    'data-role="sdk-table"',
    'data-role="save-manager-dialog"',
    'data-role="save-table"',
    'data-role="save-detail"',
    'data-role="save-confirmation"',
    'data-role="save-confirmation-error"',
    'data-action="clear-logs"',
    'data-action="approve-sdk"',
    'data-action="create-blank-save"',
    'data-action="create-save-copy"',
    'data-action="save-selected-save"',
    'data-action="load-save"',
    'data-action="open-save-directory"',
    'data-action="rename-save"',
    'data-action="delete-save"',
    'data-action="confirm-load-save-current"',
    'data-action="confirm-load-discard"',
    'data-action="confirm-delete-save"',
  ]) {
    assert(html.includes(required), `missing utility UI marker: ${required}`);
  }
  for (const required of [
    ".utility-table",
    ".panel-status",
    ".log-row.level-error",
    ".sdk-name-input",
    ".sdk-url-input",
    ".save-manager-dialog",
    ".save-name-input",
    ".save-manager-workspace",
    ".save-list-item",
    ".save-detail-grid",
    ".save-confirmation-layer",
    ".save-confirmation-error",
  ]) {
    assert(css.includes(required), `missing utility CSS rule: ${required}`);
  }

  const patches = normalizePatchRecords(samplePatchRecords());
  assert(patches[0].patchId === "patch_001", "patch id should normalize");
  assert(patches[0].tasks[0].affectedSystems[0] === "package", "patch tasks should normalize");
  assert(validatePatchRequest("") === t("utility.patch.validation.emptyRequest"), "empty patch validation changed");
  assert(buildAnalyzePatchRequest("  Add task  ").request === "Add task", "patch request should trim");

  const blocked = normalizePackageView(samplePackageViewBlocked());
  assert(!blocked.canPackage, "blocked package must not be packageable");
  assert(blocked.blockingIssues[0].includes("PACKAGE-NO-ACTUAL-PROJECT-CHANGES"), "blocking issue should normalize");
  const packageRequest = buildPackageRequestFromSources({
    integration: { status: "blocked" },
    actualProjectFileAudit: { actual_changed_files: [] },
    unityValidationSummary: { valid: false },
  });
  assert(packageRequest.actual_project_file_audit.actual_changed_files.length === 0, "package request audit mismatch");

  const logs = normalizeLogEntries(sampleLogEntries());
  assert(filterLogEntries(logs, "ERROR").length === 1, "log filter should isolate errors");
  assert(buildReadLogEntriesRequest("ERROR", 20).level === "ERROR", "log read request mismatch");
  assert(formatLogJsonl(logs).includes('"level":"ERROR"'), "log jsonl should include error level");

  const sdks = normalizeSdkSpecs(sampleSdkSpecs());
  assert(sdks[0].sourceUrl.includes("steamgames"), "sdk source URL should normalize");
  assert(validateSdkName("") === t("utility.sdk.validation.emptyName"), "empty SDK name validation changed");
  const add = buildAddSdkRequest("Steamworks SDK", "https://partner.steamgames.com/doc/sdk");
  assert(add.sdk_id === "steamworks_sdk", "sdk id slug mismatch");
  assert(add.source_url.includes("steamgames"), "sdk add request should preserve source URL");
  const approve = buildUpdateSdkReviewStatusRequest("steamworks", "approved");
  assert(approve.status === "approved", "sdk review status request mismatch");

  const saveIndex = normalizeSaveIndex(sampleSaveIndex());
  assert(saveIndex.currentSaveId === "save_combat", "save current id should normalize");
  assert(saveIndex.saves[0].isCurrent, "current save should be marked");
  assert(saveIndex.workspaceState === "linked_save", "draft workspace state should normalize");
  assert(saveIndex.hasAutosave, "draft autosave availability should normalize");
  assert(saveIndex.saves[0].progress.pipelinePassed === 2, "pipeline progress should normalize");
  assert(saveIndex.saves[0].progress.designPassed === 68, "design progress should normalize");
  assert(saveIndex.saves[0].lastTransactionSeq === 42, "transaction sequence should normalize");
  assert(saveIndex.saves[0].workspaceBytes === 5242880, "workspace bytes should normalize");
  assert(saveIndex.saves[1].lockedByOther, "save lock state should normalize");
  assert(saveIndex.saves[1].integrityStatus === "warning", "integrity status should normalize");
  assert(saveIndex.saves[3].integrityStatus === "error", "corrupt integrity should normalize");
  assert(validateSaveName("") === t("utility.save.validation.emptyName"), "empty save name validation changed");
  assert(!formatSaveTimestamp("unix:20").includes("unix:"), "unix time should be formatted");
  assert(formatSaveBytes(5242880).includes("MB"), "workspace bytes should be formatted");
  const saveDiagnostics = commandDiagnostics({
    __commandDiagnostics: [{ level: "INFO", message: "committed" }],
    warnings: ["cleanup failed", "committed"],
  });
  assert(saveDiagnostics.length === 2, "save diagnostics should merge and deduplicate warnings");
  assert(
    saveDiagnostics.some(
      (diagnostic) => diagnostic.level === "WARNING" && diagnostic.message === "cleanup failed",
    ),
    "save warnings should become visible warning diagnostics",
  );
  assert(
    await refreshAfterCommittedSave(async () => {}),
    "successful post-commit refresh should stay successful",
  );
  assert(
    !(await refreshAfterCommittedSave(async () => {
      throw new Error("fixture refresh failure");
    })),
    "post-commit refresh errors must not turn the committed save into a failure",
  );
  const projectState = buildProjectStateFromDesignView(sampleDesignView());
  assert(projectState.projectName === "未命名游戏设计项目", "project state name should normalize");
  assert(projectState.profile.genre === "Action RPG", "project state profile should become a map");
  assert(projectState.nodes.combat_loop.checklist.core_loop, "checklist should map by item id");
  assert(
    projectState.nodes.combat_loop.checklistOptions.core_loop.loop_type.primary === "turn_based",
    "primary option should survive save conversion",
  );
  const createSave = buildCreateSaveRequest(" Combat Save ", sampleDesignView());
  const createBlank = buildCreateBlankSaveRequest(" Blank Save ");
  assert(createSave.display_name === "Combat Save", "create save request should trim display name");
  assert(createBlank.display_name === "Blank Save", "blank save request should trim display name");
  assert(createBlank.state.nodes && typeof createBlank.state.nodes === "object", "blank save request should include a deserializable state");
  assert(createSave.state.nodes.combat_loop.decisionState === "completed", "save state should include decision state");
  assert(buildSaveProjectRequest(sampleDesignView()).reason === "manual_save", "save project reason default changed");
  assert(
    buildLoadSaveRequest("save_combat", "discard_draft").switch_behavior === "discard_draft",
    "load save switch behavior mismatch",
  );
  assert(buildRenameSaveRequest("save_combat", "Renamed").display_name === "Renamed", "rename save request mismatch");
  assert(buildDeleteSaveRequest("save_combat").save_id === "save_combat", "delete save request mismatch");
  assert(buildOpenSaveDirectoryRequest("save_combat").save_id === "save_combat", "open save directory request mismatch");
  const saveLocked = new Error("backend detail must not leak into localized UI");
  saveLocked.code = "SAVE_LOCKED";
  assert(
    formatSaveCommandError(saveLocked) === t("utility.save.error.locked"),
    "stable save errors should localize by code",
  );

  const failingApis = createUtilityPanelApis(async () => {
    throw new Error("utility backend unavailable");
  });
  await assertRejects(
    () => failingApis.logs.read(),
    "utility API load failures must reach the panel controller",
  );

  const calls = { patch: 0, package: 0, logs: 0, sdk: 0, context: 0, save: 0 };
  const noPanelDocument = { querySelector: () => null };
  const controller = createUtilityPanelsController(noPanelDocument, {
    patch: { list: async () => (calls.patch += 1, []) },
    package: { load: async () => (calls.package += 1, null) },
    logs: { read: async () => (calls.logs += 1, []) },
    sdk: {
      list: async () => (calls.sdk += 1, []),
      approvedContext: async () => (calls.context += 1, ""),
    },
    save: { list: async () => (calls.save += 1, null) },
  });
  await controller.refresh("logs");
  assert(calls.logs === 1, "logs route must refresh the logs panel");
  assert(
    calls.patch + calls.package + calls.sdk + calls.context + calls.save === 0,
    "single-panel refresh must not fan out to unrelated utility APIs",
  );
  const allUtilityValues = await controller.refreshAll();
  for (const field of ["patches", "packageView", "logs", "sdks", "sdkContext", "saveIndex"]) {
    assert(Object.hasOwn(allUtilityValues, field), `utility refreshAll result is missing ${field}`);
  }
  assert(
    calls.patch === 1 &&
      calls.package === 1 &&
      calls.logs === 2 &&
      calls.sdk === 1 &&
      calls.context === 1 &&
      calls.save === 1,
    "utility refreshAll must load each target exactly once",
  );

  const pending = [];
  const racingController = createUtilityPanelsController(noPanelDocument, {
    logs: {
      read: () => new Promise((resolve) => pending.push(resolve)),
    },
  });
  const oldRefresh = racingController.refresh("logs");
  const newRefresh = racingController.refresh("logs");
  pending[1]([]);
  const newResult = await newRefresh;
  pending[0]([]);
  const oldResult = await oldRefresh;
  assert(!newResult.stale, "latest utility refresh must be accepted");
  assert(oldResult.stale, "late utility refresh must not overwrite newer panel state");

  const failedController = createUtilityPanelsController(noPanelDocument, {
    package: { load: async () => Promise.reject(new Error("package read failed")) },
  });
  const failedResult = await failedController.refresh("package");
  assert(
    failedResult.value.__loadError === "package read failed",
    "utility controller must preserve explicit load failures",
  );
}

function testAiConfig(html, css, aiConfigJs) {
  setAiConfigDescriptors(sampleAiConfigDescriptors());
  for (const required of [
    'data-role="ai-config-modal"',
    'data-ai-config-tab="dev"',
    'data-ai-config-tab="image"',
    'data-ai-config-tab="completion"',
    'data-role="ai-entry-list"',
    'data-role="ai-config-fields"',
    'data-action="apply-ai-config"',
    'data-action="save-ai-config"',
    'role="tab"',
    'role="tabpanel"',
    'aria-selected="true"',
  ]) {
    assert(html.includes(required), `missing AI config UI marker: ${required}`);
  }
  for (const required of [
    ".ai-config-dialog",
    ".ai-config-body",
    ".ai-entry-item",
    ".ai-entry-active-action",
    ".ai-config-field",
    ".modal-backdrop",
  ]) {
    assert(css.includes(required), `missing AI config CSS rule: ${required}`);
  }
  assert(
    /\.ai-entry-active-action\s*\{[^}]*min-height:\s*30px/su.test(css),
    "AI current-entry actions must use the compact height",
  );
  assert(
    !aiConfigJs.includes("${entry.id} / ${entry.configType}"),
    "AI entry list must not expose internal IDs or config types",
  );

  const config = normalizeAiConfig(sampleAiConfig());
  assert(config.schemaVersion === 3, "AI config schema should normalize");
  assert(config.dev.entries.length === 2, "dev entries should normalize");
  assert(config.completion.activeEntryId === "completion_api", "completion active entry mismatch");
  assert(configTypesForCategory("image").includes("openai_image_api"), "image types missing");

  const model = createAiConfigModel(sampleAiConfig());
  assert(model.selectedEntry().id === "codex", "default selected dev entry mismatch");
  const originalDevActive = model.config.dev.activeEntryId;
  model.selectCategory("completion");
  assert(model.selectedEntry().id === "completion_api", "completion selection mismatch");
  model.setActiveSelected();
  assert(model.config.completion.activeEntryId === "completion_api", "active completion mismatch");
  assert(model.config.dev.activeEntryId === originalDevActive, "completion activation must not change dev");
  model.selectCategory("image");
  const imageEntries = model.config.image.entries;
  if (imageEntries.length > 1) {
    model.selectEntry(imageEntries[1].id);
    model.setActiveSelected();
    assert(model.config.image.activeEntryId === imageEntries[1].id, "image activation mismatch");
    assert(model.config.dev.activeEntryId === originalDevActive, "image activation must not change dev");
  }
  model.selectCategory("completion");
  const added = model.addEntry();
  assert(added.id === "completion_2", "new entry id mismatch");
  const countBeforeProtectedDelete = model.config.completion.entries.length;
  model.removeSelected();
  assert(model.config.completion.entries.length === countBeforeProtectedDelete - 1, "entry deletion mismatch");

  assert(entryFieldMode("openai_completion_api").requiresApi, "API type should require API fields");
  assert(entryFieldMode("openai_completion_api").usesCustomJson, "OpenAI completion must expose model JSON");
  assert(entryFieldMode("local_codex_cli").usesCodexFiles, "Codex CLI type should show file fields");
  assert(entryFieldMode("custom_dev_api").usesCustomJson, "custom type should show extra JSON");
  assert(maskApiKey("secret") === API_KEY_MASK, "API key mask mismatch");
  assert(applyApiKeyEdit("secret", API_KEY_MASK) === "secret", "masked key should preserve existing value");
  assert(applyApiKeyEdit("secret", "new-secret") === "new-secret", "edited key should replace existing value");
  assert(validateExtraJsonText('{"model":"gpt-test"}').model === "gpt-test", "extra JSON parse mismatch");
  assertThrows(() => validateExtraJsonText("[1]"), "extra JSON array should be rejected");
  assert(selectedNativeFilePath({ status: "cancelled", path: null }) === null, "cancelled picker must preserve the current path");
  assert(selectedNativeFilePath({ status: "selected", path: " C:/Tools/codex.exe " }) === "C:/Tools/codex.exe", "selected picker path should normalize");
  assertThrows(
    () => selectedNativeFilePath({ status: "selected", path: "" }),
    "selected picker result must reject an empty path",
  );

  const redacted = JSON.stringify(redactAiConfigForDisplay(sampleAiConfig()));
  assert(redacted.includes(API_KEY_MASK), "redacted config should show mask");
  assert(!redacted.includes("completion-secret"), "redacted config must not expose full API key");
  const save = buildAiConfigSaveRequest(sampleAiConfig());
  assert(save.config.completion.entries[0].apiKey === "completion-secret", "save request should preserve actual key in payload");
  assert(buildNewApiEntry("dev", 3).configType === "local_codex_cli", "new dev entry type mismatch");
}

function testSettingsStyle(html, css, pipelineJs, settingsStyleJs) {
  for (const required of [
    'data-role="project-config-modal"',
    'data-role="project-engine"',
    'data-role="game-spec-v2-enabled"',
    'data-role="development-project-path"',
    'data-action="pick-development-project-path"',
    'data-action="pick-editor-path"',
    'data-action="discover-unity-editors"',
    'data-role="unity-editor-candidates"',
    'data-role="project-preflight-output"',
    'data-role="project-preflight-actions"',
    'data-action="preflight-reselect-project"',
    'data-action="preflight-reselect-editor"',
    'data-action="preflight-rescan-editor"',
    'data-action="validate-project-config"',
    'data-action="relink-project-binding"',
    'data-action="save-project-config"',
    'data-role="style-prompt-editor-modal"',
    'data-role="style-prompt-preview"',
    'data-role="style-prompt-conversation"',
    'data-action="send-style-prompt"',
    'data-action="confirm-style-prompts"',
  ]) {
    assert(html.includes(required), `missing settings/style UI marker: ${required}`);
  }
  for (const required of [
    ".project-config-dialog",
    ".project-config-grid",
    ".project-config-toggle",
    ".native-path-field",
    ".editor-discovery-controls",
    ".project-preflight-section",
    ".project-preflight-actions",
    ".style-prompt-dialog",
    ".style-prompt-preview",
    ".style-prompt-conversation",
  ]) {
    assert(css.includes(required), `missing settings/style CSS rule: ${required}`);
  }
  assert(settingsStyleJs.includes("export function buildProjectConfigSaveRequest"), "missing project config save builder");
  assert(settingsStyleJs.includes("export function parseStylePromptResponse"), "missing style prompt parser");
  assert(pipelineJs.includes("open-style-prompt-editor"), "missing Step07 prompt editor action");

  const config = normalizeProjectConfig(sampleProjectConfig());
  assert(config.projectEngine === "unity", "project engine should normalize");
  assert(!config.gameSpecV2Enabled, "GameSpec v2 should default off");
  assert(config.developmentPath === "UnityProject", "development path should normalize");
  assert(normalizeProjectConfig({ project_engine: "bad-engine" }).projectEngine === "unity", "invalid engine should fallback");
  assert(
    normalizeProjectConfig({ game_spec_v2: { enabled: true } }).gameSpecV2Enabled,
    "nested GameSpec v2 switch should normalize",
  );
  assert(
    normalizeProjectConfig({ game_spec_v2_enabled: true }).gameSpecV2Enabled,
    "flat GameSpec v2 switch should normalize",
  );
  assert(engineLabel("godot") === "Godot", "engine label mismatch");
  const projectPicker = buildProjectDirectoryPickerRequest(config);
  assert(projectPicker.kind === "folder", "project picker must request a folder");
  assert(projectPicker.current_path === "UnityProject", "project picker should preserve the current path");
  const editorPicker = buildEditorExecutablePickerRequest(config);
  assert(editorPicker.kind === "file", "editor picker must request a file");
  assert(editorPicker.filters[0].extensions.includes("exe"), "editor picker executable filter missing");
  assert(buildProjectInspectionRequest(config).expected_engine === "unity", "project inspection engine mismatch");
  assert(buildUnityEditorDiscoveryRequest(config).project_path === "UnityProject", "editor discovery project mismatch");
  assert(
    buildProjectEditorValidationRequest(config).editor_path === config.editorPath,
    "editor validation request mismatch",
  );
  const inspection = normalizeProjectEnvironmentInspection({
    status: "valid",
    detected_engine: "unity",
    unity_version: { version: "2022.3.21f1", revision: "abc" },
    diagnostics: [],
  });
  assert(inspection.unityVersion.version === "2022.3.21f1", "Unity version should normalize");
  const candidates = normalizeUnityEditorCandidates([
    { path: "C:/Unity.exe", valid_executable: true, match_kind: "exact" },
  ]);
  assert(candidates[0].validExecutable && candidates[0].matchKind === "exact", "Unity editor candidate should normalize");
  setLanguageMode("zh-CN", { notify: false });
  assert(editorCandidateMatchLabel("exact") === "精确匹配", "Chinese editor match label mismatch");
  assert(editorCandidateSourceLabel("unity_hub") === "Unity Hub", "Chinese editor source label mismatch");
  setLanguageMode("en-US", { notify: false });
  assert(editorCandidateMatchLabel("compatible") === "Compatible version", "English editor match label mismatch");
  assert(editorCandidateSourceLabel("configured") === "Current configuration", "English editor source label mismatch");
  setLanguageMode("zh-CN", { notify: false });
  assert(
    normalizeEditorSelectionValidation({ valid: true, error_code: "" }).valid,
    "editor selection validation should normalize",
  );

  const customSave = buildProjectConfigSaveRequest(
    sampleProjectConfig({
      project_engine: "custom",
      game_spec_v2_enabled: true,
      custom_engine_name: "Internal",
      editor_path: "ignored.exe",
    }),
  );
  assert(customSave.settings.project_engine === "custom", "custom engine save mismatch");
  assert(customSave.settings.game_spec_v2_enabled, "GameSpec v2 switch should persist through project config save");
  assert(customSave.settings.custom_engine_name === "Internal", "custom engine name should persist");
  assert(customSave.settings.editor_path === "", "custom engine should not send editor path");
  const preflight = buildProjectPreflightRequest(sampleProjectConfig(), { writeReport: false });
  assert(preflight.write_report === false, "preflight write report option mismatch");
  const relink = buildProjectRelinkRequest(config, { runPreflight: false });
  assert(relink.project_path === "UnityProject", "relink project path mismatch");
  assert(relink.run_preflight === false, "relink preflight option mismatch");
  const report = normalizePreflightReport({
    status: "blocked",
    blockers: [{ code: "missing_development_path", field: "development_path", message: "missing", fix: "set path" }],
    warnings: ["editor_path is not set"],
    settings: sampleProjectConfig(),
  });
  assert(report.blockers[0].field === "development_path", "preflight blockers should normalize");
  assert(report.warnings.length === 1, "preflight warnings should normalize");
  assert(
    preflightFixActions(report).includes("reselect_project_path"),
    "legacy preflight code should map to a stable path action",
  );
  const structuredReport = normalizePreflightReport({
    status: "blocked",
    diagnostics: [{
      severity: "blocker",
      error_code: "opaque_code",
      field: "editor_path",
      message: "arbitrary localized backend text",
      fix_action: "rescan_editor",
    }],
    settings: sampleProjectConfig(),
  });
  assert(
    preflightFixActions(structuredReport)[0] === "rescan_editor",
    "preflight actions must use fix_action instead of parsing message text",
  );

  const styleOptions = normalizeStylePromptOptions(samplePipelineView().style_options);
  assert(styleOptions[0].styleId === "stylized", "pipeline style option should normalize to style id");
  const parsed = parseStylePromptResponse(sampleStylePromptResponse(), new Set(["stylized", "minimal"]));
  assert(parsed.explanation.includes("轮廓"), "style prompt explanation should parse");
  assert(parsed.prompts.stylized.includes("silhouette"), "style prompt parser should honor valid ids");
  assert(!Object.hasOwn(parsed.prompts, "realistic"), "style prompt parser should reject invalid ids");
  const messages = buildStylePromptMessages(styleOptions, [{ role: "user", content: "more readable" }]);
  assert(messages[0].role === "system", "style prompt messages should include system prompt");
  const override = buildStylePromptOverrideRequest(styleOptions, { stylized: "new readable prompt" }, 2);
  assert(override.source === "style_prompt_editor", "style prompt override source mismatch");
  assert(override.options[0].prompt_refined, "style prompt override should mark refined prompts");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertThrows(fn, message) {
  try {
    fn();
  } catch {
    return;
  }
  throw new Error(message);
}

async function assertRejects(fn, message) {
  try {
    await fn();
  } catch {
    return;
  }
  throw new Error(message);
}
