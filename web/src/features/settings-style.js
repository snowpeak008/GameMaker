import { enumLabel, getLanguageMode, t } from "../i18n.js";
import { setModalVisible } from "../modal-focus.js";

export const ENGINE_OPTIONS = [
  {
    id: "unity",
    label: "Unity",
    get developmentLabel() {
      return t("settings.project.field.unityProjectPath");
    },
    get editorLabel() {
      return t("settings.project.field.unityEditorPath");
    },
  },
  {
    id: "godot",
    label: "Godot",
    get developmentLabel() {
      return t("settings.project.field.godotProjectPath");
    },
    get editorLabel() {
      return t("settings.project.field.godotEditorPath");
    },
  },
  {
    id: "unreal",
    label: "Unreal Engine",
    get developmentLabel() {
      return t("settings.project.field.unrealProjectPath");
    },
    get editorLabel() {
      return t("settings.project.field.unrealEditorPath");
    },
  },
  {
    id: "custom",
    get label() {
      return t("settings.project.engine.custom");
    },
    get developmentLabel() {
      return t("settings.project.field.customProjectRoot");
    },
    get editorLabel() {
      return t("settings.project.field.editorPath");
    },
  },
];

export const DEFAULT_PROJECT_CONFIG = {
  schemaVersion: 2,
  bindingId: "",
  projectEngine: "unity",
  pipelineAdapter: "none",
  customEngineName: "",
  requiredEditorVersion: "",
  developmentPath: "",
  editorPath: "",
};

export const PROMPT_START = "PROMPT_START";
export const PROMPT_END = "PROMPT_END";

export const DEFAULT_STYLE_PROMPT_OPTIONS = [
  {
    styleId: "STYLE-01-readability",
    optionId: "STYLE-01-readability",
    title: "Stylized Readability",
    description: "Clear silhouettes and bold color groups.",
    imagePath: "outputs/stage_07/style_01.png",
    prompt: "stylized game art style reference, clear silhouettes, bold readable color groups",
    selected: true,
    promptRefined: false,
  },
];

export function createSettingsStyleApi(invokeCommand) {
  return {
    async loadProjectConfig() {
      try {
        return unwrapCommandResponse(await invokeCommand("load_project_config"));
      } catch {
        return DEFAULT_PROJECT_CONFIG;
      }
    },
    async saveProjectConfig(settings, options = {}) {
      return unwrapCommandResponse(
        await invokeCommand("save_project_config", {
          request: buildProjectConfigSaveRequest(settings, options),
        }),
      );
    },
    async runProjectPreflight(settings, options = {}) {
      return unwrapCommandResponse(
        await invokeCommand("run_project_preflight", {
          request: buildProjectPreflightRequest(settings, options),
        }),
      );
    },
    async relinkProjectBinding(settings, options = {}) {
      return unwrapCommandResponse(
        await invokeCommand("relink_project_binding", {
          request: buildProjectRelinkRequest(settings, options),
        }),
      );
    },
    async selectNativePath(request) {
      return unwrapCommandResponse(
        await invokeCommand("select_native_path", { request }),
      );
    },
    async inspectProjectEnvironment(request) {
      return unwrapCommandResponse(
        await invokeCommand("inspect_project_environment", { request }),
      );
    },
    async discoverUnityEditors(request) {
      return unwrapCommandResponse(
        await invokeCommand("discover_project_unity_editors", { request }),
      );
    },
    async validateProjectEditor(request) {
      return unwrapCommandResponse(
        await invokeCommand("validate_project_editor", { request }),
      );
    },
    async refineStylePrompts(request) {
      return unwrapCommandResponse(await invokeCommand("refine_style_prompts", { request }));
    },
  };
}

export function normalizeProjectConfig(input) {
  const value = input ?? DEFAULT_PROJECT_CONFIG;
  const engine = String(read(value, "projectEngine", "project_engine") ?? "unity")
    .trim()
    .toLowerCase();
  return {
    schemaVersion: Number(read(value, "schemaVersion", "schema_version") ?? 2),
    bindingId: read(value, "bindingId", "binding_id") ?? "",
    projectEngine: ENGINE_OPTIONS.some((option) => option.id === engine) ? engine : "unity",
    pipelineAdapter: read(value, "pipelineAdapter", "pipeline_adapter") ?? "none",
    customEngineName: read(value, "customEngineName", "custom_engine_name") ?? "",
    requiredEditorVersion:
      read(value, "requiredEditorVersion", "required_editor_version") ?? "",
    developmentPath: read(value, "developmentPath", "development_path") ?? "",
    editorPath: read(value, "editorPath", "editor_path") ?? "",
  };
}

export function engineLabel(engineId) {
  return ENGINE_OPTIONS.find((option) => option.id === engineId)?.label ?? "Unity";
}

export function enginePathLabels(engineId) {
  const option = ENGINE_OPTIONS.find((item) => item.id === engineId) ?? ENGINE_OPTIONS[0];
  return {
    developmentLabel: option.developmentLabel,
    editorLabel: option.editorLabel,
  };
}

export function buildProjectConfigSaveRequest(settings, options = {}) {
  return {
    settings: projectConfigToSnake(settings),
    run_preflight: options.runPreflight ?? options.run_preflight ?? true,
  };
}

export function buildProjectPreflightRequest(settings = null, options = {}) {
  return {
    settings: settings ? projectConfigToSnake(settings) : null,
    write_report: options.writeReport ?? options.write_report ?? true,
    prefer_run_context: Boolean(options.preferRunContext ?? options.prefer_run_context),
  };
}

export function buildProjectRelinkRequest(settings, options = {}) {
  const config = normalizeProjectConfig(settings);
  return {
    settings: projectConfigToSnake(config),
    project_path: config.developmentPath.trim(),
    editor_path: config.projectEngine === "custom" ? "" : config.editorPath.trim(),
    run_preflight: options.runPreflight ?? options.run_preflight ?? true,
  };
}

export function buildProjectDirectoryPickerRequest(settings) {
  const config = normalizeProjectConfig(settings);
  return {
    kind: "folder",
    title: t("settings.project.picker.projectTitle", {
      engine: engineLabel(config.projectEngine),
    }),
    current_path: config.developmentPath.trim(),
    filters: [],
  };
}

export function buildEditorExecutablePickerRequest(settings) {
  const config = normalizeProjectConfig(settings);
  return {
    kind: "file",
    title: t("settings.project.picker.editorTitle", {
      engine: engineLabel(config.projectEngine),
    }),
    current_path: config.editorPath.trim(),
    filters: [
      {
        name: t("settings.project.picker.executableFilter"),
        extensions: ["exe"],
      },
    ],
  };
}

export function buildProjectInspectionRequest(settings) {
  const config = normalizeProjectConfig(settings);
  return {
    project_path: config.developmentPath.trim(),
    expected_engine: config.projectEngine,
  };
}

export function buildUnityEditorDiscoveryRequest(settings) {
  const config = normalizeProjectConfig(settings);
  return {
    project_path: config.developmentPath.trim(),
    configured_editor_path: config.editorPath.trim(),
  };
}

export function buildProjectEditorValidationRequest(settings) {
  const config = normalizeProjectConfig(settings);
  return {
    project_engine: config.projectEngine,
    project_path: config.developmentPath.trim(),
    editor_path: config.editorPath.trim(),
  };
}

export function normalizePreflightReport(input) {
  const value = input ?? {};
  const blockers = asArray(read(value, "blockers")).map((blocker) => ({
    code: read(blocker, "code") ?? "",
    field: read(blocker, "field") ?? "",
    message: read(blocker, "message") ?? "",
    fix: read(blocker, "fix") ?? "",
  }));
  const warnings = asArray(read(value, "warnings")).map(String);
  const structured = asArray(read(value, "diagnostics")).map((diagnostic) => ({
    severity: read(diagnostic, "severity") ?? "warning",
    errorCode: read(diagnostic, "errorCode", "error_code") ?? read(diagnostic, "code") ?? "",
    field: read(diagnostic, "field") ?? "",
    message: read(diagnostic, "message") ?? "",
    fixAction:
      read(diagnostic, "fixAction", "fix_action") ?? read(diagnostic, "fix") ?? "",
  }));
  const diagnostics = structured.length > 0
    ? structured
    : [
        ...blockers.map((blocker) => ({
          severity: "blocker",
          errorCode: blocker.code,
          field: blocker.field,
          message: blocker.message,
          fixAction: legacyFixActionForCode(blocker.code),
        })),
        ...warnings.map((message) => ({
          severity: "warning",
          errorCode: "legacy_warning",
          field: "",
          message,
          fixAction: "",
        })),
      ];
  return {
    schemaVersion: Number(read(value, "schemaVersion", "schema_version") ?? 1),
    timestamp: read(value, "timestamp") ?? "",
    status: read(value, "status") ?? "unknown",
    blockers,
    warnings,
    diagnostics,
    settings: normalizeProjectConfig(read(value, "settings")),
  };
}

export function formatPreflightReport(input) {
  const report = normalizePreflightReport(input);
  const lines = [`status=${report.status}`];
  for (const diagnostic of report.diagnostics) {
    const field = diagnostic.field ? ` ${diagnostic.field}` : "";
    lines.push(
      `${diagnostic.severity.toUpperCase()} ${diagnostic.errorCode}${field}: ${diagnostic.message}`,
    );
  }
  return lines.join("\n");
}

export function preflightFixActions(input) {
  return [...new Set(
    normalizePreflightReport(input)
      .diagnostics
      .map((diagnostic) => diagnostic.fixAction)
      .filter((action) => [
        "reselect_project_path",
        "reselect_editor_path",
        "rescan_editor",
      ].includes(action)),
  )];
}

export function normalizeProjectEnvironmentInspection(input) {
  const value = input ?? {};
  const version = read(value, "unityVersion", "unity_version") ?? null;
  return {
    status: read(value, "status") ?? "invalid",
    expectedEngine: read(value, "expectedEngine", "expected_engine") ?? "",
    detectedEngine: read(value, "detectedEngine", "detected_engine") ?? "",
    markers: asArray(read(value, "markers")).map(String),
    unityVersion: version
      ? {
          version: read(version, "version") ?? "",
          revision: read(version, "revision") ?? "",
        }
      : null,
    diagnostics: asArray(read(value, "diagnostics")).map((item) => ({
      severity: read(item, "severity") ?? "warning",
      code: read(item, "code") ?? "project_environment_warning",
      message: read(item, "message") ?? "",
    })),
  };
}

export function formatProjectEnvironmentInspection(input) {
  const inspection = normalizeProjectEnvironmentInspection(input);
  const lines = [
    t("settings.project.inspection.status", { status: inspection.status }),
    t("settings.project.inspection.engine", {
      engine: inspection.detectedEngine || t("settings.project.inspection.unknown"),
    }),
  ];
  if (inspection.unityVersion?.version) {
    lines.push(
      t("settings.project.inspection.unityVersion", {
        version: inspection.unityVersion.version,
      }),
    );
  }
  if (inspection.markers.length > 0) {
    lines.push(
      t("settings.project.inspection.markers", { markers: inspection.markers.join(", ") }),
    );
  }
  for (const diagnostic of inspection.diagnostics) {
    lines.push(`${diagnostic.severity.toUpperCase()} ${diagnostic.code}: ${diagnostic.message}`);
  }
  return lines.join("\n");
}

export function normalizeUnityEditorCandidates(input) {
  return asArray(input).map((candidate) => ({
    path: read(candidate, "path") ?? "",
    source: read(candidate, "source") ?? "",
    version: read(candidate, "version") ?? "",
    present: Boolean(read(candidate, "present")),
    validExecutable: Boolean(read(candidate, "validExecutable", "valid_executable")),
    configured: Boolean(read(candidate, "configured")),
    matchKind: read(candidate, "matchKind", "match_kind") ?? "unknown",
  }));
}

export function editorCandidateMatchLabel(value) {
  return enumLabel("editorMatch", value);
}

export function editorCandidateSourceLabel(value) {
  return enumLabel("editorCandidateSource", value);
}

export function normalizeEditorSelectionValidation(input) {
  const value = input ?? {};
  return {
    valid: Boolean(read(value, "valid")),
    engine: read(value, "engine") ?? "",
    path: read(value, "path") ?? "",
    version: read(value, "version") ?? "",
    matchKind: read(value, "matchKind", "match_kind") ?? "unknown",
    errorCode: read(value, "errorCode", "error_code") ?? "invalid_editor_path",
  };
}

export function normalizeStylePromptOptions(input) {
  const source = Array.isArray(input) ? input : asArray(read(input, "options"));
  const normalized = source.map(normalizeStylePromptOption).filter((option) => option.styleId);
  return normalized.length > 0 ? normalized : DEFAULT_STYLE_PROMPT_OPTIONS.map((option) => ({ ...option }));
}

export function normalizeStylePromptOption(input) {
  const styleId = read(input, "styleId", "style_id") ?? read(input, "optionId", "option_id") ?? "";
  return {
    styleId,
    optionId: read(input, "optionId", "option_id") ?? styleId,
    title: read(input, "title") ?? styleId,
    description: read(input, "description") ?? "",
    imagePath: read(input, "imagePath", "image_path") ?? "",
    prompt: read(input, "prompt") ?? "",
    selected: Boolean(read(input, "selected")),
    promptRefined: Boolean(read(input, "promptRefined", "prompt_refined")),
  };
}

export function parseStylePromptResponse(response, validStyleIds = null) {
  const text = String(response ?? "").trim();
  const explanation = text.includes(PROMPT_START) ? text.split(PROMPT_START, 1)[0].trim() : text;
  const prompts = {};
  if (!text.includes(PROMPT_START) || !text.includes(PROMPT_END)) {
    return { explanation, prompts };
  }
  const valid = validStyleIds ? new Set(Array.from(validStyleIds).map(String)) : null;
  const block = text.split(PROMPT_START, 2)[1].split(PROMPT_END, 1)[0];
  for (const rawLine of block.split(/\r?\n/)) {
    const line = rawLine.trim().replace(/^`+|`+$/g, "");
    if (!line || line.startsWith("```") || !line.includes(":")) {
      continue;
    }
    const [rawStyleId, ...rest] = line.split(":");
    const styleId = rawStyleId.replace(/^[-*]\s*/, "").trim();
    const prompt = rest.join(":").trim();
    if (!styleId || !prompt) {
      continue;
    }
    if (valid && !valid.has(styleId)) {
      continue;
    }
    prompts[styleId] = prompt;
  }
  return { explanation, prompts };
}

export function buildStylePromptMessages(options, history = []) {
  const normalized = normalizeStylePromptOptions(options);
  const responseRule = t(
    getLanguageMode() === "zh-CN"
      ? "settings.stylePrompt.system.responseChinese"
      : "settings.stylePrompt.system.responseEnglish",
  );
  const context = normalized
    .map(
      (option) =>
        `${option.styleId}\n${t("settings.stylePrompt.context.title")}: ${option.title}\n${t("settings.stylePrompt.context.intent")}: ${option.description}\n${t("settings.stylePrompt.context.currentPrompt")}:\n${option.prompt}`,
    )
    .join("\n\n");
  const systemPrompt = [
    t("settings.stylePrompt.system.role"),
    t("settings.stylePrompt.system.task"),
    responseRule,
    t("settings.stylePrompt.system.constraints"),
    "",
    // Stable section marker keeps the prompt payload schema language-independent.
    "Current style options",
    `${t("settings.stylePrompt.system.currentOptions")}:\n${context}`,
  ].join("\n");
  return [{ role: "system", content: systemPrompt }, ...asArray(history)];
}

export function buildStylePromptOverrideRequest(options, refinedPrompts = {}, count = null) {
  const normalized = normalizeStylePromptOptions(options);
  const requestedCount = clampCount(count ?? normalized.length, normalized.length);
  const finalOptions = normalized.slice(0, requestedCount).map((option) => {
    const refined = String(refinedPrompts[option.styleId] ?? refinedPrompts[option.optionId] ?? "").trim();
    return {
      style_id: option.styleId,
      option_id: option.optionId,
      title: option.title,
      description: option.description,
      image_path: option.imagePath,
      prompt: refined || option.prompt,
      prompt_refined: Boolean(refined),
    };
  });
  return {
    schema_version: 1,
    source: "style_prompt_editor",
    requested_count: requestedCount,
    count: finalOptions.length,
    options: finalOptions,
  };
}

export function initSettingsStyleModals(documentRef, api = {}) {
  if (!documentRef) {
    return null;
  }
  const openProjectConfig = async () => {
    const config = api.loadProjectConfig ? await api.loadProjectConfig() : DEFAULT_PROJECT_CONFIG;
    renderProjectConfigDialog(documentRef, config, api);
    showModal(documentRef, "project-config-modal", true);
  };
  const openStylePromptEditor = (options = DEFAULT_STYLE_PROMPT_OPTIONS) => {
    renderStylePromptEditorDialog(documentRef, { options }, api);
    showModal(documentRef, "style-prompt-editor-modal", true);
  };

  for (const opener of documentRef.querySelectorAll('[data-action="project-config"]')) {
    opener.onclick = openProjectConfig;
  }
  for (const closer of documentRef.querySelectorAll(
    '[data-action="cancel-project-config"]',
  )) {
    closer.onclick = () => showModal(documentRef, "project-config-modal", false);
  }
  for (const closer of documentRef.querySelectorAll(
    '[data-action="cancel-style-prompt-editor"]',
  )) {
    closer.onclick = () => showModal(documentRef, "style-prompt-editor-modal", false);
  }
  documentRef.addEventListener("adm:open-style-prompt-editor", (event) => {
    openStylePromptEditor(event.detail?.styleOptions ?? DEFAULT_STYLE_PROMPT_OPTIONS);
  });
  return { openProjectConfig, openStylePromptEditor };
}

export function renderProjectConfigDialog(documentRef, configInput, api = {}) {
  const modal = documentRef.querySelector('[data-role="project-config-modal"]');
  if (!modal) {
    return null;
  }
  const config = normalizeProjectConfig(configInput);
  modal.__projectConfigBase = config;
  setFieldValue(modal, "project-engine", config.projectEngine);
  setFieldValue(modal, "custom-engine-name", config.customEngineName);
  setFieldValue(modal, "development-project-path", config.developmentPath);
  setFieldValue(modal, "editor-path", config.editorPath);
  updateProjectConfigLabels(modal, config.projectEngine);
  const editorCandidates = modal.querySelector('[data-role="unity-editor-candidates"]');
  if (editorCandidates) {
    editorCandidates.hidden = true;
    editorCandidates.replaceChildren();
  }
  const status = modal.querySelector('[data-role="project-config-status"]');
  const output = modal.querySelector('[data-role="project-preflight-output"]');
  setText(status, t("settings.project.status.loaded"));
  setText(output, t("settings.project.preflight.waiting"));

  bindAction(modal, "project-engine", "change", () => {
    updateProjectConfigLabels(modal, fieldValue(modal, "project-engine"));
  });
  bindNativePathAction(
    modal,
    "pick-development-project-path",
    "development-project-path",
    buildProjectDirectoryPickerRequest,
    api,
    status,
    async () => inspectProjectForm(modal, api, status, output),
  );
  bindNativePathAction(
    modal,
    "pick-editor-path",
    "editor-path",
    buildEditorExecutablePickerRequest,
    api,
    status,
    async () => validateEditorForm(modal, api, status),
  );
  bindAction(modal, "discover-unity-editors", "click", async () => {
    const button = modal.querySelector('[data-action="discover-unity-editors"]');
    if (!api.discoverUnityEditors) {
      setText(status, t("settings.project.status.discoveryUnavailable"));
      return;
    }
    if (button) button.disabled = true;
    try {
      const candidates = normalizeUnityEditorCandidates(
        await api.discoverUnityEditors(
          buildUnityEditorDiscoveryRequest(readProjectConfigForm(modal)),
        ),
      );
      renderUnityEditorCandidates(modal, candidates, status);
    } catch (error) {
      setRuntimeText(
        status,
        t("settings.project.status.discoveryFailed", { error: error.message }),
      );
    } finally {
      if (button) button.disabled = false;
    }
  });
  bindAction(modal, "save-project-config", "click", async () => {
    const next = readProjectConfigForm(modal);
    try {
      const result = api.saveProjectConfig
        ? await api.saveProjectConfig(next, { runPreflight: true })
        : { settings: next, preflight: normalizePreflightReport({ status: "passed", settings: next }) };
      setText(status, t("settings.project.status.saved"));
      if (result?.settings) {
        modal.__projectConfigBase = normalizeProjectConfig(result.settings);
      }
      if (result?.preflight) {
        renderPreflightResult(modal, result.preflight, output);
      }
    } catch (error) {
      setRuntimeText(status, t("settings.project.status.saveFailed", { error: error.message }));
    }
  });
  bindAction(modal, "relink-project-binding", "click", async () => {
    const next = readProjectConfigForm(modal);
    try {
      if (!api.relinkProjectBinding) {
        throw new Error(t("settings.project.status.relinkUnavailable"));
      }
      const result = await api.relinkProjectBinding(next, { runPreflight: true });
      if (result?.settings) {
        const relinked = normalizeProjectConfig(result.settings);
        modal.__projectConfigBase = relinked;
        setFieldValue(modal, "development-project-path", relinked.developmentPath);
        setFieldValue(modal, "editor-path", relinked.editorPath);
      }
      if (result?.preflight) {
        renderPreflightResult(modal, result.preflight, output);
      }
      setText(status, t("settings.project.status.relinked"));
    } catch (error) {
      setRuntimeText(status, t("settings.project.status.relinkFailed", { error: error.message }));
    }
  });
  bindAction(modal, "validate-project-config", "click", async () => {
    const next = readProjectConfigForm(modal);
    try {
      const report = api.runProjectPreflight
        ? await api.runProjectPreflight(next, { writeReport: true })
        : normalizePreflightReport({ status: "passed", settings: next });
      const rawStatus = String(normalizePreflightReport(report).status ?? "");
      const statusLabel = enumLabel("preflightStatus", rawStatus);
      const statusMessage = t("settings.project.status.preflight", { status: statusLabel });
      if (statusLabel === rawStatus && rawStatus) {
        setRuntimeText(status, statusMessage);
      } else {
        setText(status, statusMessage);
      }
      renderPreflightResult(modal, report, output);
    } catch (error) {
      setRuntimeText(status, t("settings.project.status.validationFailed", { error: error.message }));
    }
  });
  bindAction(modal, "preflight-reselect-project", "click", () => {
    modal.querySelector('[data-action="pick-development-project-path"]')?.click();
  });
  bindAction(modal, "preflight-reselect-editor", "click", () => {
    modal.querySelector('[data-action="pick-editor-path"]')?.click();
  });
  bindAction(modal, "preflight-rescan-editor", "click", () => {
    modal.querySelector('[data-action="discover-unity-editors"]')?.click();
  });
  renderPreflightFixActions(modal, []);
  return config;
}

export function renderStylePromptEditorDialog(documentRef, input, api = {}) {
  const modal = documentRef.querySelector('[data-role="style-prompt-editor-modal"]');
  if (!modal) {
    return null;
  }
  const options = normalizeStylePromptOptions(input?.options ?? input);
  const state = {
    options,
    history: [],
    refinedPrompts: {},
  };
  modal.__stylePromptState = state;
  markRuntime(modal.querySelector('[data-role="style-prompt-input"]'));
  refreshStylePromptPreview(modal, state);
  setRuntimeText(
    modal.querySelector('[data-role="style-prompt-conversation"]'),
    greetingForStyleOptions(options),
  );
  setText(
    modal.querySelector('[data-role="style-prompt-status"]'),
    t("settings.stylePrompt.status.waiting"),
  );

  bindAction(modal, "send-style-prompt", "click", async () => {
    const inputBox = modal.querySelector('[data-role="style-prompt-input"]');
    const userText = inputBox?.value.trim() ?? "";
    if (!userText) {
      return;
    }
    if (inputBox) {
      inputBox.value = "";
    }
    state.history.push({ role: "user", content: userText });
    appendConversation(
      modal,
      t("settings.stylePrompt.conversation.user", { message: userText }),
    );
    try {
      const request = {
        messages: buildStylePromptMessages(currentPromptOptions(state), state.history),
      };
      const response = api.refineStylePrompts ? await api.refineStylePrompts(request) : { text: userText };
      const text = response?.text ?? response?.content ?? userText;
      state.history.push({ role: "assistant", content: text });
      const validIds = new Set(state.options.map((option) => option.styleId));
      const parsed = parseStylePromptResponse(text, validIds);
      Object.assign(state.refinedPrompts, parsed.prompts);
      appendConversation(
        modal,
        t("settings.stylePrompt.conversation.assistant", {
          message: parsed.explanation || text,
        }),
      );
      refreshStylePromptPreview(modal, state);
      setText(
        modal.querySelector('[data-role="style-prompt-status"]'),
        t("settings.stylePrompt.status.updated", {
          count: Object.keys(parsed.prompts).length,
        }),
      );
    } catch (error) {
      setRuntimeText(
        modal.querySelector('[data-role="style-prompt-status"]'),
        t("settings.stylePrompt.status.failed", { error: error.message }),
      );
    }
  });
  bindAction(modal, "confirm-style-prompts", "click", async () => {
    const count = Number(fieldValue(modal, "style-prompt-count") || state.options.length);
    const request = buildStylePromptOverrideRequest(state.options, state.refinedPrompts, count);
    try {
      if (api.confirmStylePrompts) {
        await api.confirmStylePrompts(request);
      }
      setText(
        modal.querySelector('[data-role="style-prompt-status"]'),
        t("settings.stylePrompt.status.generated", { count: request.count }),
      );
    } catch (error) {
      setRuntimeText(
        modal.querySelector('[data-role="style-prompt-status"]'),
        t("settings.stylePrompt.status.confirmFailed", { error: error.message }),
      );
    }
  });
  return state;
}

function projectConfigToSnake(settings) {
  const config = normalizeProjectConfig(settings);
  return {
    schema_version: Math.max(2, config.schemaVersion),
    binding_id: config.bindingId,
    project_engine: config.projectEngine,
    pipeline_adapter: config.pipelineAdapter,
    custom_engine_name: config.projectEngine === "custom" ? config.customEngineName.trim() : "",
    required_editor_version: config.requiredEditorVersion.trim(),
    development_path: config.developmentPath.trim(),
    editor_path: config.projectEngine === "custom" ? "" : config.editorPath.trim(),
  };
}

function readProjectConfigForm(modal) {
  return normalizeProjectConfig({
    ...(modal.__projectConfigBase ?? DEFAULT_PROJECT_CONFIG),
    projectEngine: fieldValue(modal, "project-engine"),
    customEngineName: fieldValue(modal, "custom-engine-name"),
    developmentPath: fieldValue(modal, "development-project-path"),
    editorPath: fieldValue(modal, "editor-path"),
  });
}

function bindNativePathAction(
  modal,
  action,
  fieldRole,
  requestBuilder,
  api,
  status,
  onSelected = null,
) {
  const button = modal.querySelector(`[data-action="${action}"]`);
  if (!button) {
    return;
  }
  button.onclick = async () => {
    if (!api.selectNativePath) {
      setText(status, t("settings.project.status.pickerUnavailable"));
      return;
    }
    button.disabled = true;
    const previousValue = fieldValue(modal, fieldRole);
    try {
      const result = await api.selectNativePath(requestBuilder(readProjectConfigForm(modal)));
      const selectionStatus = String(read(result, "status") ?? "");
      const selectedPath = String(read(result, "path") ?? "").trim();
      if (selectionStatus === "selected" && selectedPath) {
        setFieldValue(modal, fieldRole, selectedPath);
        setText(status, t("settings.project.status.pathSelected"));
        if (onSelected) {
          await onSelected(selectedPath);
        }
      } else if (selectionStatus === "cancelled") {
        setText(status, t("settings.project.status.pathSelectionCancelled"));
      } else {
        throw new Error(t("settings.project.status.pathSelectionInvalid"));
      }
    } catch (error) {
      setFieldValue(modal, fieldRole, previousValue);
      setRuntimeText(
        status,
        t("settings.project.status.pathSelectionFailed", { error: error.message }),
      );
    } finally {
      button.disabled = false;
    }
  };
}

async function inspectProjectForm(modal, api, status, output) {
  if (!api.inspectProjectEnvironment) {
    return;
  }
  const inspection = await api.inspectProjectEnvironment(
    buildProjectInspectionRequest(readProjectConfigForm(modal)),
  );
  setRuntimeText(output, formatProjectEnvironmentInspection(inspection));
  const normalized = normalizeProjectEnvironmentInspection(inspection);
  if (normalized.unityVersion?.version) {
    modal.__projectConfigBase = normalizeProjectConfig({
      ...(modal.__projectConfigBase ?? DEFAULT_PROJECT_CONFIG),
      requiredEditorVersion: normalized.unityVersion.version,
    });
  }
  setText(
    status,
    t("settings.project.status.inspected", { status: normalized.status }),
  );
}

async function validateEditorForm(modal, api, status) {
  if (!api.validateProjectEditor) {
    throw new Error(t("settings.project.status.editorValidationUnavailable"));
  }
  const validation = normalizeEditorSelectionValidation(
    await api.validateProjectEditor(
      buildProjectEditorValidationRequest(readProjectConfigForm(modal)),
    ),
  );
  if (!validation.valid) {
    throw new Error(
      t("settings.project.status.editorInvalid", { code: validation.errorCode }),
    );
  }
  setText(status, t("settings.project.status.editorValidated"));
  return validation;
}

function renderUnityEditorCandidates(modal, candidates, status) {
  const select = modal.querySelector('[data-role="unity-editor-candidates"]');
  if (!select) {
    return;
  }
  select.replaceChildren();
  const valid = candidates.filter(
    (candidate) =>
      candidate.present
      && candidate.validExecutable
      && candidate.matchKind !== "mismatch",
  );
  if (valid.length === 0) {
    select.hidden = true;
    setText(status, t("settings.project.status.noEditorsFound"));
    return;
  }
  const placeholder = modal.ownerDocument.createElement("option");
  placeholder.value = "";
  placeholder.textContent = t("settings.project.discovery.chooseCandidate");
  placeholder.selected = true;
  placeholder.disabled = true;
  select.append(placeholder);
  for (const candidate of valid) {
    const option = modal.ownerDocument.createElement("option");
    option.value = candidate.path;
    option.textContent = t("settings.project.discovery.candidate", {
      version: candidate.version || t("settings.project.inspection.unknown"),
      match: editorCandidateMatchLabel(candidate.matchKind),
      source: editorCandidateSourceLabel(candidate.source),
    });
    select.append(option);
  }
  select.hidden = false;
  select.onchange = () => {
    if (!select.value) {
      return;
    }
    setFieldValue(modal, "editor-path", select.value);
    setText(status, t("settings.project.status.editorCandidateSelected"));
  };
  setText(status, t("settings.project.status.editorsFound", { count: valid.length }));
}

function renderPreflightResult(modal, report, output) {
  setRuntimeText(output, formatPreflightReport(report));
  renderPreflightFixActions(modal, preflightFixActions(report));
}

function renderPreflightFixActions(modal, actions) {
  const container = modal.querySelector('[data-role="project-preflight-actions"]');
  if (!container) {
    return;
  }
  const actionSet = new Set(actions);
  const mapping = [
    ["preflight-reselect-project", "reselect_project_path"],
    ["preflight-reselect-editor", "reselect_editor_path"],
    ["preflight-rescan-editor", "rescan_editor"],
  ];
  for (const [action, fixAction] of mapping) {
    const button = container.querySelector(`[data-action="${action}"]`);
    if (button) {
      button.hidden = !actionSet.has(fixAction);
    }
  }
  container.hidden = actionSet.size === 0;
}

function updateProjectConfigLabels(modal, engineId) {
  const labels = enginePathLabels(engineId);
  setText(modal.querySelector('[data-role="development-path-label"]'), labels.developmentLabel);
  setText(modal.querySelector('[data-role="editor-path-label"]'), labels.editorLabel);
  const customField = modal.querySelector('[data-role="custom-engine-field"]');
  if (customField) {
    customField.hidden = engineId !== "custom";
  }
  const editorField = modal.querySelector('[data-role="editor-path-field"]');
  if (editorField) {
    editorField.hidden = engineId === "custom";
  }
  const discoveryControls = modal.querySelector('[data-role="editor-discovery-controls"]');
  if (discoveryControls) {
    discoveryControls.hidden = engineId !== "unity";
  }
}

function currentPromptOptions(state) {
  return state.options.map((option) => ({
    ...option,
    prompt: state.refinedPrompts[option.styleId] ?? option.prompt,
  }));
}

function refreshStylePromptPreview(modal, state) {
  const preview = modal.querySelector('[data-role="style-prompt-preview"]');
  setText(
    preview,
    currentPromptOptions(state)
      .map(
        (option) =>
          `${option.styleId}\n${option.title}\n${option.prompt || option.description || t("settings.stylePrompt.preview.waiting")}`,
      )
      .join("\n\n"),
  );
  markRuntime(preview);
}

function greetingForStyleOptions(options) {
  const lines = options.map((option) => `- ${option.styleId}: ${option.title}`);
  return t("settings.stylePrompt.greeting", {
    count: options.length,
    options: lines.join("\n"),
  });
}

function appendConversation(modal, text) {
  const conversation = modal.querySelector('[data-role="style-prompt-conversation"]');
  if (!conversation) {
    return;
  }
  conversation.textContent = `${conversation.textContent.trim()}\n\n${text}`.trim();
}

function showModal(documentRef, role, visible) {
  const modal = documentRef.querySelector(`[data-role="${role}"]`);
  if (modal) {
    setModalVisible(modal, visible);
  }
}

function setFieldValue(container, role, value) {
  const field = container.querySelector(`[data-role="${role}"]`);
  if (field) {
    field.value = value ?? "";
    if (role !== "project-engine") {
      markRuntime(field);
    }
  }
}

function fieldValue(container, role) {
  return container.querySelector(`[data-role="${role}"]`)?.value ?? "";
}

function bindAction(container, action, eventName, handler) {
  const target = container.querySelector(`[data-action="${action}"], [data-role="${action}"]`);
  if (!target) {
    return;
  }
  target[`on${eventName}`] = handler;
}

function setText(element, text) {
  if (element) {
    delete element.dataset.contentOrigin;
    element.textContent = text;
  }
}

function setRuntimeText(element, text) {
  setText(element, text);
  markRuntime(element);
}

function markRuntime(element) {
  if (element) {
    element.dataset.contentOrigin = "runtime";
  }
  return element;
}

function clampCount(value, available) {
  const number = Number(value);
  const fallback = available > 0 ? available : 1;
  if (!Number.isFinite(number)) {
    return fallback;
  }
  return Math.max(1, Math.min(5, Math.min(fallback, Math.trunc(number))));
}

function unwrapCommandResponse(response) {
  if (response && typeof response.ok === "boolean") {
    if (response.ok) {
      return response.data ?? null;
    }
    const detail =
      response.error?.message ?? response.error?.code ?? t("settings.error.commandFailed");
    throw new Error(detail);
  }
  return response ?? null;
}

function read(object, camelKey, snakeKey = camelKey) {
  if (!object || typeof object !== "object") {
    return undefined;
  }
  if (Object.hasOwn(object, camelKey)) {
    return object[camelKey];
  }
  if (Object.hasOwn(object, snakeKey)) {
    return object[snakeKey];
  }
  return undefined;
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function legacyFixActionForCode(code) {
  const actions = {
    missing_development_path: "reselect_project_path",
    development_path_not_found: "reselect_project_path",
    project_engine_conflict: "reselect_project_path",
    missing_editor_path: "rescan_editor",
    editor_path_not_found: "reselect_editor_path",
    invalid_editor_path: "reselect_editor_path",
    invalid_unity_editor_executable: "reselect_editor_path",
    unity_editor_version_conflict: "rescan_editor",
  };
  return actions[String(code ?? "")] ?? "";
}
