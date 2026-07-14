import { TASKS, createShellModel } from "../src/main.js";
import {
  buildChecklistRequest,
  buildDeleteTemplateRequest,
  buildNodeTextRequest,
  buildOptionRequest,
  buildSaveTemplateRequest,
  buildTemplateListRequest,
  buildTemplateSelectionRequest,
  createDesignModel,
  DesignWorkbenchController,
  normalizeTemplateList,
  parseDesignEntities,
} from "../src/features/design.js";
import {
  buildForceAiOutputRequest,
  buildMarkAiInaccurateRequest,
  buildSaveAiArchiveRequest,
  buildSubmitAiTurnRequest,
  createAiInterviewModel,
} from "../src/features/ai-interview.js";
import {
  API_KEY_MASK,
  applyApiKeyEdit,
  buildAiConfigSaveRequest,
  createAiConfigModel,
  entryFieldMode,
  maskApiKey,
  normalizeAiConfig,
  redactAiConfigForDisplay,
  setAiConfigDescriptors,
  validateExtraJsonText,
} from "../src/features/ai-config.js";
import {
  buildConfirmStyleRequest,
  buildReadStep07PreviewRequest,
  buildResumePipelineRequest,
  buildRunPipelineRequest,
  createPipelineModel,
  styleGridVisibleForStage,
} from "../src/features/pipeline.js";
import {
  buildEditorExecutablePickerRequest,
  buildProjectConfigSaveRequest,
  buildProjectDirectoryPickerRequest,
  buildProjectPreflightRequest,
  buildStylePromptMessages,
  buildStylePromptOverrideRequest,
  normalizeProjectConfig,
  normalizeStylePromptOptions,
  parseStylePromptResponse,
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
  filterLogEntries,
  formatSaveBytes,
  formatSaveTimestamp,
  normalizePackageView,
  normalizePatchRecords,
  normalizeSaveIndex,
  normalizeSdkSpecs,
  validatePatchRequest,
  validateSaveName,
  validateSdkName,
} from "../src/features/utility-panels.js";
import { t } from "../src/i18n.js";
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

const target = process.argv[2] ?? "all";
if (target === "all") {
  testShellE2e();
  await testDesignE2e();
  testAiInterviewE2e();
  testPipelineE2e();
  testUtilityPanelsE2e();
  testAiConfigE2e();
  testSettingsStyleE2e();
  console.log("web all e2e checks passed");
} else if (target === "shell") {
  testShellE2e();
} else if (target === "design") {
  await testDesignE2e();
} else if (target === "ai-interview") {
  testAiInterviewE2e();
} else if (target === "pipeline") {
  testPipelineE2e();
} else if (target === "utility-panels") {
  testUtilityPanelsE2e();
} else if (target === "ai-config") {
  testAiConfigE2e();
} else if (target === "settings-style") {
  testSettingsStyleE2e();
} else {
  throw new Error(`unsupported e2e target: ${target}`);
}

function testShellE2e() {
  const model = createShellModel();
  const visited = [];
  for (const task of TASKS) {
    visited.push(model.switchRoute(task.id));
    if (model.activeRoute !== task.id) {
      throw new Error(`active route mismatch: ${task.id}`);
    }
  }

  if (visited.join(",") !== TASKS.map((task) => task.id).join(",")) {
    throw new Error("route visitation order changed");
  }

  console.log(`web shell e2e passed: ${visited.join(" -> ")}`);
}

async function testDesignE2e() {
  const model = createDesignModel(sampleDesignView());
  if (model.currentDomain().domainId !== "mechanics") {
    throw new Error("default design domain mismatch");
  }
  if (model.visibleNodes().map((node) => node.nodeId).join(",") !== "combat_loop,progression") {
    throw new Error("mechanics node order mismatch");
  }
  model.selectDomain("narrative");
  if (model.visibleNodes()[0].nodeId !== "tone") {
    throw new Error("domain switch did not show narrative node");
  }
  model.selectDomain("mechanics");
  model.setSearch("combat");
  if (model.visibleNodes()[0].nodeId !== "combat_loop") {
    throw new Error("design search did not focus combat loop");
  }
  model.setSearch("");
  model.setFilter("risk");
  if (model.visibleNodes()[0].nodeId !== "progression") {
    throw new Error("risk filter did not find progression");
  }
  model.setFilter("l4_missing");
  if (!model.visibleNodes().some((node) => node.nodeId === "progression")) {
    throw new Error("L4 missing filter did not include progression");
  }

  const noteRequest = buildNodeTextRequest("combat_loop", { designNote: "Readable update" });
  if (noteRequest.node_id !== "combat_loop" || noteRequest.design_note !== "Readable update") {
    throw new Error("design note request mismatch");
  }
  const checklistRequest = buildChecklistRequest("combat_loop", "core_loop", false);
  if (checklistRequest.checklist[0].checked !== false) {
    throw new Error("checklist request mismatch");
  }
  const optionRequest = buildOptionRequest("combat_loop", "core_loop", "loop_type", "real_time", true);
  if (optionRequest.option_updates[0].option_id !== "real_time") {
    throw new Error("option request mismatch");
  }
  if (parseDesignEntities('[{"kind":"loop"}]')[0].kind !== "loop") {
    throw new Error("design entity parsing mismatch");
  }
  const templates = normalizeTemplateList(sampleTemplateList());
  if (templates.templates.length !== 3 || !templates.templates[2].canDelete) {
    throw new Error("template list/detail normalization mismatch");
  }
  if (!buildTemplateListRequest(true).include_internal) {
    throw new Error("template list request mismatch");
  }
  if (Object.hasOwn(buildTemplateSelectionRequest("builtin_demo"), "project_state")) {
    throw new Error("template selection still trusts client project state");
  }
  if (!buildSaveTemplateRequest("Demo", "indie", true).overwrite) {
    throw new Error("template overwrite request mismatch");
  }
  if (buildDeleteTemplateRequest("custom_demo").template_id !== "custom_demo") {
    throw new Error("template delete request mismatch");
  }

  let authoritative = { ...sampleDesignView(), project_name: "First Load" };
  const input = { value: "Loaded Save" };
  const redraws = [];
  const controller = new DesignWorkbenchController(
    { querySelector: () => input },
    {
      load: async () => authoritative,
      setProjectName: async (name) => {
        authoritative = { ...authoritative, project_name: name };
        return authoritative;
      },
    },
    (_document, view) => {
      redraws.push(view.project_name);
      return createDesignModel(view);
    },
  );
  await controller.reload();
  await controller.latestView({ reload: true });
  if (controller.view.projectName !== "Loaded Save" || redraws.at(-1) !== "Loaded Save") {
    throw new Error("save/load journey retained the first design model");
  }

  console.log("web design e2e passed: domain -> search -> risk -> l4 -> templates -> update requests");
}

function testAiInterviewE2e() {
  const model = createAiInterviewModel(sampleAiInterviewState());
  if (!model.currentQuestion.includes("player promise")) {
    throw new Error("AI current question mismatch");
  }
  if (model.actionsDisabled) {
    throw new Error("completed AI state should allow actions");
  }
  const running = createAiInterviewModel(sampleAiInterviewState({ status: "running" }));
  if (!running.actionsDisabled || running.statusText !== t("settings.aiInterview.running")) {
    throw new Error("running AI state should disable actions");
  }

  const submit = buildSubmitAiTurnRequest("Readable tactical mastery.");
  if (submit.schema_mode !== "turn" || !submit.user_message.includes("tactical")) {
    throw new Error("submit AI turn request mismatch");
  }
  const force = buildForceAiOutputRequest({
    schemaVersion: "1.0",
    mode: "maintenance",
  });
  if (force.schema_mode !== "full_output" || force.payload.mode !== "maintenance") {
    throw new Error("force AI output request mismatch");
  }
  const mark = buildMarkAiInaccurateRequest("combat_loop", "ignores constraints");
  if (mark.node_id !== "combat_loop" || !mark.reason.includes("constraints")) {
    throw new Error("mark inaccurate request mismatch");
  }
  const archive = buildSaveAiArchiveRequest("ai_archives/manual/test.json");
  if (archive.archive_path !== "ai_archives/manual/test.json") {
    throw new Error("archive request mismatch");
  }

  console.log("web ai-interview e2e passed: question -> running -> send -> force -> mark -> archive");
}

function testPipelineE2e() {
  const model = createPipelineModel(samplePipelineView());
  if (model.selectedStage().stageId !== "07") {
    throw new Error("pipeline should default to current Step07");
  }
  model.selectStage("10");
  if (model.selectedStage().title !== "Asset Alignment") {
    throw new Error("pipeline stage selection mismatch");
  }
  if (styleGridVisibleForStage(model.selectedStage())) {
    throw new Error("Step07 style images must be hidden when another stage is selected");
  }
  model.selectStage("07");
  if (!styleGridVisibleForStage(model.selectedStage())) {
    throw new Error("Step07 style images must be visible on the selected Step07 stage");
  }
  const run = buildRunPipelineRequest("07", "10");
  if (run.from_stage_id !== "07" || run.to_stage_id !== "10" || run.artifact_locale !== "zh-CN") {
    throw new Error("pipeline run request mismatch");
  }
  const resume = buildResumePipelineRequest({ runId: "run_1", revision: 4 });
  if (resume.run_id !== "run_1" || resume.expected_revision !== 4) {
    throw new Error("pipeline resume request mismatch");
  }
  const confirm = buildConfirmStyleRequest("07", "stylized", "use readable silhouettes");
  if (!confirm.message.includes("style=stylized") || !confirm.message.includes("readable")) {
    throw new Error("style confirmation request mismatch");
  }
  if (model.view.styleOptions.filter((option) => option.selected).length !== 1) {
    throw new Error("style option selection mismatch");
  }
  if (!model.runtimeLines().join("\n").includes("waiting_confirmation")) {
    throw new Error("pipeline runtime log summary missing waiting confirmation");
  }
  const evidenceStage = model.view.stages.find((stage) => stage.stageId === "10");
  if (Object.hasOwn(evidenceStage, "artifacts") || Object.hasOwn(evidenceStage, "outputs")) {
    throw new Error("pipeline view retained internal artifacts or raw outputs");
  }
  if (evidenceStage.errors.length !== 1 || evidenceStage.warnings.length !== 1) {
    throw new Error("pipeline issue fields did not survive normalization");
  }
  const readPreview = buildReadStep07PreviewRequest("generated_images/style.png", 4096);
  if (readPreview.stage_id !== "07" || readPreview.max_bytes !== 4096) {
    throw new Error("Step07 preview request boundary mismatch");
  }

  console.log("web pipeline e2e passed: select -> run -> style -> issues (internal artifacts hidden)");
}

function testUtilityPanelsE2e() {
  const patchValidation = validatePatchRequest("");
  if (patchValidation !== t("utility.patch.validation.emptyRequest")) {
    throw new Error("empty patch validation mismatch");
  }
  const patchRequest = buildAnalyzePatchRequest("Add SDK approval filter");
  const patches = normalizePatchRecords(samplePatchRecords());
  if (patchRequest.request !== "Add SDK approval filter" || patches[0].status !== "validated") {
    throw new Error("patch analyze flow mismatch");
  }

  const blockedPackage = normalizePackageView(samplePackageViewBlocked());
  if (blockedPackage.canPackage || blockedPackage.blockingIssues.length !== 1) {
    throw new Error("blocked package flow mismatch");
  }
  const blockedRequest = buildPackageRequestFromSources({
    integration: { status: "blocked" },
    actualProjectFileAudit: { actual_changed_files: [] },
    unityValidationSummary: { valid: false },
  });
  if (blockedRequest.actual_project_file_audit.actual_changed_files.length !== 0) {
    throw new Error("package source request should keep empty changed-files evidence");
  }

  const errorLogs = filterLogEntries(sampleLogEntries(), "ERROR");
  if (errorLogs.length !== 1 || !errorLogs[0].message.includes("blocked")) {
    throw new Error("log ERROR filter mismatch");
  }
  if (buildReadLogEntriesRequest("ERROR", 25).limit !== 25) {
    throw new Error("log read request limit mismatch");
  }

  const sdks = normalizeSdkSpecs(sampleSdkSpecs());
  if (validateSdkName("") !== t("utility.sdk.validation.emptyName")) {
    throw new Error("empty sdk name validation mismatch");
  }
  const add = buildAddSdkRequest("Steamworks SDK", "https://partner.steamgames.com/doc/sdk");
  const approve = buildUpdateSdkReviewStatusRequest(sdks[0].sdkId, "approved");
  const reject = buildUpdateSdkReviewStatusRequest(sdks[1].sdkId, "rejected");
  if (add.sdk_id !== "steamworks_sdk" || approve.status !== "approved" || reject.status !== "rejected") {
    throw new Error("sdk status flow mismatch");
  }

  const saveIndex = normalizeSaveIndex(sampleSaveIndex());
  const saveState = buildProjectStateFromDesignView(sampleDesignView());
  const createSave = buildCreateSaveRequest("Combat Save", sampleDesignView());
  const createBlank = buildCreateBlankSaveRequest("New Project");
  const saveProject = buildSaveProjectRequest(sampleDesignView());
  if (!saveIndex.saves[0].isCurrent || validateSaveName("") !== t("utility.save.validation.emptyName")) {
    throw new Error("save manager validation mismatch");
  }
  if (!saveState.nodes.combat_loop.checklist.core_loop || createSave.display_name !== "Combat Save") {
    throw new Error("save ProjectState conversion mismatch");
  }
  if (
    createBlank.display_name !== "New Project" ||
    saveIndex.saves[0].progress.designPassed !== 68 ||
    saveIndex.saves[1].lockOwnerPid !== 44520 ||
    formatSaveTimestamp("unix:20").includes("unix:") ||
    !formatSaveBytes(saveIndex.saves[0].workspaceBytes).includes("MB")
  ) {
    throw new Error("save metadata journey mismatch");
  }
  if (
    saveProject.reason !== "manual_save" ||
    buildLoadSaveRequest("save_combat", "save_current").switch_behavior !== "save_current" ||
    buildRenameSaveRequest("save_combat", "Renamed").display_name !== "Renamed" ||
    buildDeleteSaveRequest("save_combat").save_id !== "save_combat" ||
    buildOpenSaveDirectoryRequest("save_combat").save_id !== "save_combat"
  ) {
    throw new Error("save command request flow mismatch");
  }

  console.log("web utility-panels e2e passed: patch -> package -> logs -> sdk -> save manager");
}

function testAiConfigE2e() {
  setAiConfigDescriptors(sampleAiConfigDescriptors());
  const config = normalizeAiConfig(sampleAiConfig());
  const model = createAiConfigModel(config);
  if (model.selectedEntry().id !== "codex") {
    throw new Error("AI config should start on active dev entry");
  }
  model.selectEntry("dev_api");
  model.setActiveSelected();
  if (model.config.dev.activeEntryId !== "dev_api" || model.config.activeProfileId !== "dev_api") {
    throw new Error("dev active entry should sync activeProfileId");
  }
  model.selectCategory("image");
  if (!entryFieldMode(model.selectedEntry().configType).requiresApi) {
    throw new Error("image API entry should require API fields");
  }
  model.selectCategory("completion");
  if (model.selectedEntry().id !== "completion_api") {
    throw new Error("completion tab should select active completion entry");
  }
  if (maskApiKey(model.selectedEntry().apiKey) !== API_KEY_MASK) {
    throw new Error("completion API key should render as mask");
  }
  if (applyApiKeyEdit(model.selectedEntry().apiKey, API_KEY_MASK) !== "completion-secret") {
    throw new Error("masked API key edit should preserve existing key");
  }
  if (validateExtraJsonText('{"model":"gpt-test"}').model !== "gpt-test") {
    throw new Error("custom extra JSON object parse mismatch");
  }
  const redacted = JSON.stringify(redactAiConfigForDisplay(model.toConfig()));
  if (redacted.includes("completion-secret") || !redacted.includes(API_KEY_MASK)) {
    throw new Error("redacted display config leaked API key");
  }
  const save = buildAiConfigSaveRequest(model.toConfig());
  if (save.config.completion.entries[0].apiKey !== "completion-secret") {
    throw new Error("save payload should preserve full key for backend only");
  }

  console.log("web ai-config e2e passed: tabs -> active entry -> conditional fields -> masked key save");
}

function testSettingsStyleE2e() {
  const projectConfig = normalizeProjectConfig(sampleProjectConfig());
  if (projectConfig.projectEngine !== "unity" || !projectConfig.developmentPath.includes("UnityProject")) {
    throw new Error("project config normalization mismatch");
  }
  if (buildProjectDirectoryPickerRequest(projectConfig).kind !== "folder") {
    throw new Error("project path picker should select a folder");
  }
  if (!buildEditorExecutablePickerRequest(projectConfig).filters[0].extensions.includes("exe")) {
    throw new Error("editor picker executable filter mismatch");
  }
  const save = buildProjectConfigSaveRequest(projectConfig, { runPreflight: true });
  if (!save.run_preflight || save.settings.project_engine !== "unity") {
    throw new Error("project config save request mismatch");
  }
  const preflight = buildProjectPreflightRequest(projectConfig, { writeReport: false });
  if (preflight.write_report || preflight.settings.development_path !== "UnityProject") {
    throw new Error("project preflight request mismatch");
  }

  const styleOptions = normalizeStylePromptOptions(samplePipelineView().style_options);
  const messages = buildStylePromptMessages(styleOptions, [{ role: "user", content: "Make silhouettes stronger" }]);
  if (!messages[0].content.includes("Current style options") || messages[1].role !== "user") {
    throw new Error("style prompt message flow mismatch");
  }
  const parsed = parseStylePromptResponse(sampleStylePromptResponse(), new Set(["stylized", "minimal"]));
  if (!parsed.prompts.stylized || parsed.prompts.realistic) {
    throw new Error("style prompt parser valid-id filtering mismatch");
  }
  const override = buildStylePromptOverrideRequest(styleOptions, parsed.prompts, 2);
  if (override.count !== 2 || !override.options[0].prompt_refined) {
    throw new Error("style prompt override request mismatch");
  }

  console.log("web settings-style e2e passed: project config -> preflight -> Step07 prompt editor -> override");
}
