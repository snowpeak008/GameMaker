import { enumLabel, t } from "../i18n.js";
import { setModalVisible } from "../modal-focus.js";

export const AI_CONFIG_CATEGORIES = [
  {
    id: "dev",
    get label() {
      return t("settings.aiConfig.category.dev");
    },
  },
  {
    id: "image",
    get label() {
      return t("settings.aiConfig.category.image");
    },
  },
  {
    id: "completion",
    get label() {
      return t("settings.aiConfig.category.completion");
    },
  },
];

let aiConfigDescriptors = [];

export const API_KEY_MASK = "********";

export const DEFAULT_AI_CONFIG = {
  schemaVersion: 3,
  dev: { categoryId: "dev", entries: [], activeEntryId: "" },
  image: { categoryId: "image", entries: [], activeEntryId: "" },
  completion: { categoryId: "completion", entries: [], activeEntryId: "" },
  activeProfileId: "",
  profiles: [],
};

export function createAiConfigApi(invokeCommand) {
  return {
    async load() {
      try {
        return unwrapCommandResponse(await invokeCommand("load_ai_config"));
      } catch {
        return DEFAULT_AI_CONFIG;
      }
    },
    async validate(config) {
      return unwrapCommandResponse(await invokeCommand("validate_ai_config", { config }));
    },
    async save(config) {
      return unwrapCommandResponse(
        await invokeCommand("save_ai_config", { request: buildAiConfigSaveRequest(config) }),
      );
    },
    async completionAdapterSpec(config) {
      return unwrapCommandResponse(await invokeCommand("completion_adapter_spec", { config }));
    },
    async descriptors() {
      try {
        return unwrapCommandResponse(await invokeCommand("list_ai_config_descriptors"));
      } catch {
        return [];
      }
    },
    async preview(config, categoryId) {
      return unwrapCommandResponse(
        await invokeCommand("preview_ai_resolution", {
          request: { config: normalizeAiConfig(config), categoryId },
        }),
      );
    },
    async probeCli(config, categoryId) {
      return unwrapCommandResponse(
        await invokeCommand("probe_ai_cli", {
          request: { config: normalizeAiConfig(config), categoryId },
        }),
      );
    },
    async probeApi(config, categoryId) {
      return unwrapCommandResponse(
        await invokeCommand("probe_ai_api", {
          request: { config: normalizeAiConfig(config), categoryId },
        }),
      );
    },
    async pickFile(request) {
      return unwrapCommandResponse(
        await invokeCommand("select_native_path", { request }),
      );
    },
  };
}

export function setAiConfigDescriptors(input) {
  aiConfigDescriptors = asArray(input)
    .map((descriptor) => ({
      configType: read(descriptor, "configType", "config_type") ?? "",
      category: read(descriptor, "category") ?? "",
      source: read(descriptor, "source") ?? "",
      adapter: read(descriptor, "adapter") ?? "",
      capabilities: asArray(read(descriptor, "capabilities")),
      requiredFields: asArray(read(descriptor, "requiredFields", "required_fields")),
      defaultProgram: read(descriptor, "defaultProgram", "default_program") ?? "",
    }))
    .filter((descriptor) => descriptor.configType && descriptor.category);
  return aiConfigDescriptors;
}

export function aiConfigDescriptor(configType) {
  return aiConfigDescriptors.find((descriptor) => descriptor.configType === configType) ?? null;
}

export function normalizeAiConfig(input) {
  const config = input ?? DEFAULT_AI_CONFIG;
  const legacyActiveProfileId = read(config, "activeProfileId", "active_profile_id") ?? "";
  const dev = normalizeApiCategory(read(config, "dev"), "dev", legacyActiveProfileId);
  const image = normalizeApiCategory(read(config, "image"), "image");
  const completion = normalizeApiCategory(read(config, "completion"), "completion");
  return {
    schemaVersion: Number(read(config, "schemaVersion", "schema_version") ?? 3),
    dev,
    image,
    completion,
    activeProfileId: dev.activeEntryId,
    profiles: asArray(read(config, "profiles")),
  };
}

export function normalizeApiCategory(input, categoryId, fallbackActiveEntryId = "") {
  const entries = asArray(read(input, "entries")).map((entry) =>
    normalizeApiEntry(entry, categoryId),
  );
  const requestedActiveEntryId =
    read(input, "activeEntryId", "active_entry_id") || fallbackActiveEntryId;
  return {
    categoryId: read(input, "categoryId", "category_id") ?? categoryId,
    entries,
    activeEntryId: entries.some((entry) => entry.id === requestedActiveEntryId)
      ? requestedActiveEntryId
      : entries[0]?.id ?? "",
  };
}

export function normalizeApiEntry(input, categoryId = "dev") {
  const fallbackType = configTypesForCategory(categoryId)[0] ?? "";
  return {
    id: read(input, "id") ?? "",
    label: read(input, "label") ?? read(input, "id") ?? "",
    configType: read(input, "configType", "config_type") ?? fallbackType,
    apiUrl: read(input, "apiUrl", "api_url") ?? "",
    apiKey: read(input, "apiKey", "api_key") ?? "",
    extraJson: read(input, "extraJson", "extra_json") ?? null,
    codexTomlPath: read(input, "codexTomlPath", "codex_toml_path") ?? "",
    codexJsonPath: read(input, "codexJsonPath", "codex_json_path") ?? "",
  };
}

export function createAiConfigModel(input) {
  let config = normalizeAiConfig(input);
  let activeCategoryId = "dev";
  let selectedEntryId = firstSelectedId(config[activeCategoryId]);
  return {
    get config() {
      return config;
    },
    get activeCategoryId() {
      return activeCategoryId;
    },
    get selectedEntryId() {
      return selectedEntryId;
    },
    selectCategory(categoryId) {
      if (!AI_CONFIG_CATEGORIES.some((category) => category.id === categoryId)) {
        return activeCategoryId;
      }
      activeCategoryId = categoryId;
      selectedEntryId = firstSelectedId(config[activeCategoryId]);
      return activeCategoryId;
    },
    selectEntry(entryId) {
      selectedEntryId = entryId;
      return selectedEntryId;
    },
    selectedCategory() {
      return config[activeCategoryId];
    },
    selectedEntry() {
      return this.selectedCategory().entries.find((entry) => entry.id === selectedEntryId) ?? null;
    },
    addEntry() {
      const category = this.selectedCategory();
      const entry = buildNewApiEntry(activeCategoryId, category.entries.length + 1);
      category.entries.push(entry);
      selectedEntryId = entry.id;
      if (!category.activeEntryId) {
        category.activeEntryId = entry.id;
      }
      return entry;
    },
    removeSelected() {
      const category = this.selectedCategory();
      if (category.entries.length <= 1) {
        return selectedEntryId;
      }
      const removedIndex = category.entries.findIndex((entry) => entry.id === selectedEntryId);
      if (removedIndex < 0) {
        return selectedEntryId;
      }
      const removedActiveEntry = category.activeEntryId === selectedEntryId;
      category.entries.splice(removedIndex, 1);
      selectedEntryId =
        category.entries[removedIndex]?.id ??
        category.entries[removedIndex - 1]?.id ??
        firstSelectedId(category);
      if (removedActiveEntry) {
        category.activeEntryId = selectedEntryId;
        if (activeCategoryId === "dev") {
          config.activeProfileId = selectedEntryId;
        }
      }
      return selectedEntryId;
    },
    updateSelected(updatedEntry) {
      const category = this.selectedCategory();
      const index = category.entries.findIndex((entry) => entry.id === selectedEntryId);
      if (index >= 0) {
        const wasActiveEntry = category.activeEntryId === selectedEntryId;
        category.entries[index] = normalizeApiEntry(updatedEntry, activeCategoryId);
        selectedEntryId = category.entries[index].id;
        if (wasActiveEntry) {
          category.activeEntryId = selectedEntryId;
        }
        if (activeCategoryId === "dev" && wasActiveEntry) {
          config.activeProfileId = selectedEntryId;
        }
      }
      return this.selectedEntry();
    },
    setActive(categoryId, entryId) {
      if (!AI_CONFIG_CATEGORIES.some((category) => category.id === categoryId)) {
        return "";
      }
      const category = config[categoryId];
      if (!category.entries.some((entry) => entry.id === entryId)) {
        return "";
      }
      category.activeEntryId = entryId;
      if (categoryId === "dev") {
        config.activeProfileId = entryId;
      }
      return entryId;
    },
    setActiveSelected() {
      return this.setActive(activeCategoryId, selectedEntryId);
    },
    toConfig() {
      return normalizeAiConfig(config);
    },
  };
}

export function configTypesForCategory(categoryId) {
  return aiConfigDescriptors
    .filter((descriptor) => descriptor.category === categoryId)
    .map((descriptor) => descriptor.configType);
}

export function buildNewApiEntry(categoryId, index = 1) {
  const id = `${categoryId}_${index}`;
  return {
    id,
    label: t("settings.aiConfig.newEntryLabel", {
      category: enumLabel("aiConfigCategory", categoryId),
    }),
    configType: configTypesForCategory(categoryId)[0] ?? "",
    apiUrl: "",
    apiKey: "",
    extraJson: null,
    codexTomlPath: "",
    codexJsonPath: "",
  };
}

export function buildAiConfigSaveRequest(config) {
  return {
    config: normalizeAiConfig(config),
  };
}

export function maskApiKey(apiKey) {
  return String(apiKey ?? "").trim() ? API_KEY_MASK : "";
}

export function applyApiKeyEdit(existingKey, editedValue) {
  const value = String(editedValue ?? "");
  if (!value || value === API_KEY_MASK) {
    return existingKey ?? "";
  }
  return value;
}

export function entryFieldMode(configType) {
  const descriptor = aiConfigDescriptor(String(configType ?? ""));
  const source = descriptor?.source ?? "";
  return {
    requiresApi: source === "api",
    usesCodexFiles:
      (source === "cli" || source === "cli_builtin") && descriptor?.adapter === "codex",
    usesLocalCli: source === "cli" || source === "cli_builtin",
    usesCustomJson: source === "api",
    capabilities: descriptor?.capabilities ?? [],
    requiredFields: descriptor?.requiredFields ?? [],
  };
}

export function validateExtraJsonText(text) {
  const trimmed = String(text ?? "").trim();
  if (!trimmed) {
    return null;
  }
  const parsed = JSON.parse(trimmed);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(t("settings.aiConfig.validation.extraJsonObject"));
  }
  return parsed;
}

export function redactAiConfigForDisplay(config) {
  const redacted = normalizeAiConfig(config);
  for (const category of AI_CONFIG_CATEGORIES) {
    redacted[category.id].entries = redacted[category.id].entries.map((entry) => ({
      ...entry,
      apiKey: maskApiKey(entry.apiKey),
    }));
  }
  return redacted;
}

export function initAiConfigDialog(documentRef, api) {
  if (!documentRef) {
    return null;
  }
  const open = async () => {
    const [config, descriptors] = await Promise.all([
      api.load(),
      api.descriptors ? api.descriptors() : [],
    ]);
    setAiConfigDescriptors(descriptors);
    renderAiConfigDialog(documentRef, config, api);
    showAiConfigModal(documentRef, true);
  };
  for (const opener of documentRef.querySelectorAll('[data-action="ai-config"], .ai-status')) {
    opener.onclick = open;
  }
  for (const closer of documentRef.querySelectorAll(
    '[data-action="cancel-ai-config"]',
  )) {
    closer.onclick = () => showAiConfigModal(documentRef, false);
  }
  return open;
}

export function renderAiConfigDialog(documentRef, configInput, api = {}) {
  const modal = documentRef.querySelector('[data-role="ai-config-modal"]');
  if (!modal) {
    return null;
  }
  const model = createAiConfigModel(configInput);
  const status = modal.querySelector('[data-role="ai-config-status"]');
  const rerender = () => {
    renderCategoryTabs(modal, model, rerender);
    renderEntryList(modal, model, rerender);
    renderEntryFields(modal, model);
    bindDialogActions(modal, model, api, status, rerender);
  };
  rerender();
  setText(status, t("settings.aiConfig.status.loaded"));
  return model;
}

function renderCategoryTabs(modal, model, rerender) {
  const status = modal.querySelector('[data-role="ai-config-status"]');
  const tabs = [...modal.querySelectorAll("[data-ai-config-tab]")];
  const activateTab = (tab) => {
    if (!syncDialogForm(modal, model, status)) {
      return;
    }
    model.selectCategory(tab.dataset.aiConfigTab);
    rerender();
    tab.focus();
  };
  for (const [index, tab] of tabs.entries()) {
    const active = tab.dataset.aiConfigTab === model.activeCategoryId;
    tab.classList.toggle("active", active);
    tab.setAttribute("aria-selected", String(active));
    tab.tabIndex = active ? 0 : -1;
    if (active) {
      modal.querySelector('[data-role="ai-config-content"]')?.setAttribute("aria-labelledby", tab.id);
    }
    tab.onclick = () => activateTab(tab);
    tab.onkeydown = (event) => {
      let nextIndex = null;
      if (["ArrowRight", "ArrowDown"].includes(event.key)) {
        nextIndex = (index + 1) % tabs.length;
      } else if (["ArrowLeft", "ArrowUp"].includes(event.key)) {
        nextIndex = (index - 1 + tabs.length) % tabs.length;
      } else if (event.key === "Home") {
        nextIndex = 0;
      } else if (event.key === "End") {
        nextIndex = tabs.length - 1;
      }
      if (nextIndex !== null) {
        event.preventDefault();
        activateTab(tabs[nextIndex]);
      }
    };
  }
}

function renderEntryList(modal, model, rerender) {
  const container = modal.querySelector('[data-role="ai-entry-list"]');
  const status = modal.querySelector('[data-role="ai-config-status"]');
  clear(container);
  const category = model.selectedCategory();
  if (category.entries.length === 0) {
    container.append(el("div", "empty-list", t("settings.aiConfig.empty.waiting")));
    return;
  }
  for (const entry of category.entries) {
    const item = el("article", "ai-entry-item");
    item.classList.toggle("active", entry.id === model.selectedEntryId);
    item.dataset.entryId = entry.id;
    const selectButton = el("button", "ai-entry-select");
    selectButton.type = "button";
    selectButton.append(markRuntime(el("strong", "", entry.label || entry.id)));
    selectButton.append(markRuntime(el("span", "", `${entry.id} / ${entry.configType}`)));
    selectButton.onclick = () => {
      const wasSelected = model.selectedEntryId === entry.id;
      if (!syncDialogForm(modal, model, status)) {
        return;
      }
      const targetEntryId = wasSelected ? model.selectedEntryId : entry.id;
      model.selectEntry(targetEntryId);
      rerender();
      focusEntryControl(modal, targetEntryId, ".ai-entry-select");
    };
    const isCurrent = entry.id === category.activeEntryId;
    const activeButton = el(
      "button",
      `ai-entry-active-action${isCurrent ? " is-current" : ""}`,
      t(
        isCurrent
          ? "settings.aiConfig.entry.active"
          : "settings.aiConfig.field.activeEntry",
      ),
    );
    activeButton.type = "button";
    activeButton.setAttribute("aria-pressed", String(isCurrent));
    activeButton.onclick = () => {
      const wasSelected = model.selectedEntryId === entry.id;
      if (!syncDialogForm(modal, model, status)) {
        return;
      }
      const targetEntryId = wasSelected ? model.selectedEntryId : entry.id;
      model.selectEntry(targetEntryId);
      model.setActive(model.activeCategoryId, targetEntryId);
      const activeEntry = model.selectedEntry();
      rerender();
      focusEntryControl(modal, targetEntryId, ".ai-entry-active-action");
      setText(
        status,
        t("settings.aiConfig.status.activated", {
          label: activeEntry?.label || targetEntryId,
        }),
      );
    };
    item.append(selectButton, activeButton);
    container.append(item);
  }
}

function renderEntryFields(modal, model) {
  const container = modal.querySelector('[data-role="ai-config-fields"]');
  clear(container);
  const entry = model.selectedEntry();
  if (!entry) {
    container.append(el("div", "empty-list", t("settings.aiConfig.empty.selectEntry")));
    return;
  }
  const mode = entryFieldMode(entry.configType);
  container.append(inputField(t("settings.aiConfig.field.entryId"), "entry-id", entry.id));
  container.append(inputField(t("settings.aiConfig.field.label"), "entry-label", entry.label));
  container.append(
    selectField(
      t("settings.aiConfig.field.configType"),
      "entry-config-type",
      configTypesForCategory(model.activeCategoryId),
      entry.configType,
    ),
  );
  if (mode.requiresApi) {
    container.append(inputField(t("settings.aiConfig.field.apiUrl"), "entry-api-url", entry.apiUrl));
    container.append(
      inputField(
        t("settings.aiConfig.field.apiKey"),
        "entry-api-key",
        maskApiKey(entry.apiKey),
        "password",
      ),
    );
  }
  if (mode.usesCodexFiles) {
    container.append(
      pathInputField(
        t("settings.aiConfig.field.codexTomlPath"),
        "entry-codex-toml",
        entry.codexTomlPath,
        "pick-ai-codex-toml",
      ),
    );
    container.append(
      pathInputField(
        t("settings.aiConfig.field.codexJsonPath"),
        "entry-codex-json",
        entry.codexJsonPath,
        "pick-ai-codex-json",
      ),
    );
  }
  if (mode.usesLocalCli) {
    container.append(
      pathInputField(
        t("settings.aiConfig.field.cliPath"),
        "entry-cli-path",
        entry.extraJson?.cli_path ?? "",
        "pick-ai-cli-path",
      ),
    );
    container.append(el("div", "ai-config-local-status", t("settings.aiConfig.cli.waiting")));
  }
  if (mode.usesCustomJson) {
    container.append(
      textareaField(t("settings.aiConfig.field.extraJson"), "entry-extra-json", extraJsonText(entry.extraJson)),
    );
  }
  const diagnostics = el("div", "ai-config-diagnostics");
  const previewButton = el(
    "button",
    "secondary-button",
    t("settings.aiConfig.action.previewResolution"),
  );
  previewButton.type = "button";
  previewButton.dataset.action = "preview-ai-resolution";
  diagnostics.append(previewButton);
  if (mode.usesLocalCli) {
    const probeButton = el(
      "button",
      "secondary-button",
      t("settings.aiConfig.action.probeCli"),
    );
    probeButton.type = "button";
    probeButton.dataset.action = "probe-ai-cli";
    diagnostics.append(probeButton);
  }
  if (mode.requiresApi) {
    const probeButton = el(
      "button",
      "secondary-button",
      t("settings.aiConfig.action.probeApi"),
    );
    probeButton.type = "button";
    probeButton.dataset.action = "probe-ai-api";
    diagnostics.append(probeButton);
  }
  const result = el("div", "ai-config-local-status", t("settings.aiConfig.preview.waiting"));
  result.dataset.role = "ai-resolution-status";
  diagnostics.append(result);
  container.append(diagnostics);
}

function bindDialogActions(modal, model, api, status, rerender) {
  const deleteButton = modal.querySelector('[data-action="delete-ai-entry"]');
  if (deleteButton) {
    deleteButton.disabled = model.selectedCategory().entries.length <= 1;
  }
  bindAction(modal, "new-ai-entry", () => {
    if (!syncDialogForm(modal, model, status)) {
      return;
    }
    model.addEntry();
    rerender();
    setText(status, t("settings.aiConfig.status.created"));
  });
  bindAction(modal, "delete-ai-entry", () => {
    model.removeSelected();
    rerender();
    setText(status, t("settings.aiConfig.status.deleted"));
  });
  bindAction(modal, "apply-ai-config", async () => {
    if (!syncDialogForm(modal, model, status)) {
      return;
    }
    try {
      const report = api.validate ? await api.validate(model.toConfig()) : { ok: true, errors: [] };
      setValidationStatus(status, report, "apply");
    } catch (error) {
      setRuntimeText(status, t("settings.aiConfig.status.applyFailed", { error: error.message }));
    }
  });
  bindAction(modal, "save-ai-config", async () => {
    if (!syncDialogForm(modal, model, status)) {
      return;
    }
    try {
      const report = api.save ? await api.save(model.toConfig()) : { ok: true, errors: [] };
      setValidationStatus(status, report, "save");
      rerender();
    } catch (error) {
      setRuntimeText(status, t("settings.aiConfig.status.saveFailed", { error: error.message }));
    }
  });
  bindAction(modal, "preview-ai-resolution", async () => {
    if (!syncDialogForm(modal, model, status)) {
      return;
    }
    const target = modal.querySelector('[data-role="ai-resolution-status"]');
    setText(target, t("settings.aiConfig.preview.running"));
    try {
      const view = api.preview
        ? await api.preview(model.toConfig(), model.activeCategoryId)
        : null;
      setRuntimeText(target, resolutionSummary(view));
    } catch (error) {
      setRuntimeText(
        target,
        t("settings.aiConfig.preview.failed", { error: error.message }),
      );
    }
  });
  bindAction(modal, "probe-ai-cli", async () => {
    if (!syncDialogForm(modal, model, status)) {
      return;
    }
    const target = modal.querySelector('[data-role="ai-resolution-status"]');
    setText(target, t("settings.aiConfig.probe.running"));
    try {
      const view = api.probeCli
        ? await api.probeCli(model.toConfig(), model.activeCategoryId)
        : null;
      setRuntimeText(target, cliProbeSummary(view));
    } catch (error) {
      setRuntimeText(target, t("settings.aiConfig.probe.failed", { error: error.message }));
    }
  });
  bindAction(modal, "probe-ai-api", async () => {
    if (!syncDialogForm(modal, model, status)) {
      return;
    }
    const target = modal.querySelector('[data-role="ai-resolution-status"]');
    setText(target, t("settings.aiConfig.probeApi.running"));
    try {
      const view = api.probeApi
        ? await api.probeApi(model.toConfig(), model.activeCategoryId)
        : null;
      setRuntimeText(target, apiProbeSummary(view));
    } catch (error) {
      setRuntimeText(target, t("settings.aiConfig.probeApi.failed", { error: error.message }));
    }
  });
  bindAiPathPicker(
    modal,
    api,
    status,
    "pick-ai-cli-path",
    "entry-cli-path",
    t("settings.aiConfig.picker.cliTitle"),
    [{ name: "Programs", extensions: ["exe", "cmd", "bat"] }],
  );
  bindAiPathPicker(
    modal,
    api,
    status,
    "pick-ai-codex-toml",
    "entry-codex-toml",
    t("settings.aiConfig.picker.tomlTitle"),
    [{ name: "TOML", extensions: ["toml"] }],
  );
  bindAiPathPicker(
    modal,
    api,
    status,
    "pick-ai-codex-json",
    "entry-codex-json",
    t("settings.aiConfig.picker.jsonTitle"),
    [{ name: "JSON", extensions: ["json"] }],
  );
}

function resolutionSummary(view) {
  if (!view) {
    return t("settings.aiConfig.preview.unavailable");
  }
  const available = Boolean(read(view, "available"));
  const source = read(view, "source") ?? "";
  const adapter = read(view, "adapter") ?? "";
  const destination =
    read(view, "program") ??
    read(view, "maskedUrl", "masked_url") ??
    t("settings.aiConfig.preview.noDestination");
  const diagnostics = asArray(read(view, "diagnostics"));
  const issue = diagnostics[0]?.message ? " · " + diagnostics[0].message : "";
  return t("settings.aiConfig.preview.result", {
    status: t(
      available
        ? "settings.aiConfig.preview.available"
        : "settings.aiConfig.preview.blocked",
    ),
    source,
    adapter,
    destination,
    issue,
  });
}

function cliProbeSummary(view) {
  if (!view) {
    return t("settings.aiConfig.preview.unavailable");
  }
  const success = Boolean(read(view, "success"));
  const version = read(view, "version") ?? t("settings.aiConfig.probe.noVersion");
  const program = read(view, "program") ?? "";
  const diagnostics = asArray(read(view, "diagnostics"));
  const issue = diagnostics[0]?.message ? " · " + diagnostics[0].message : "";
  return t("settings.aiConfig.probe.result", {
    status: t(
      success ? "settings.aiConfig.preview.available" : "settings.aiConfig.preview.blocked",
    ),
    program,
    version,
    issue,
  });
}

function apiProbeSummary(view) {
  if (!view) {
    return t("settings.aiConfig.preview.unavailable");
  }
  const available = Boolean(read(view, "available"));
  const endpoint = read(view, "endpoint") ?? "";
  const statusCode = read(view, "statusCode", "status_code") ?? "-";
  const diagnostics = asArray(read(view, "diagnostics"));
  const issue = diagnostics[0]?.message ? " · " + diagnostics[0].message : "";
  return t("settings.aiConfig.probeApi.result", {
    status: t(
      available ? "settings.aiConfig.preview.available" : "settings.aiConfig.preview.blocked",
    ),
    endpoint,
    statusCode,
    issue,
  });
}

function bindAiPathPicker(modal, api, status, action, role, title, filters) {
  const button = modal.querySelector(`[data-action="${action}"]`);
  if (!button) {
    return;
  }
  button.onclick = async () => {
    const input = modal.querySelector('[data-role="' + role + '"]');
    if (!input || !api.pickFile) {
      return;
    }
    const previous = input.value;
    button.disabled = true;
    try {
      const result = await api.pickFile({
        kind: "file",
        title,
        current_path: previous,
        filters,
      });
      const selectedPath = selectedNativeFilePath(result);
      if (selectedPath !== null) {
        input.value = selectedPath;
      }
    } catch (error) {
      input.value = previous;
      setRuntimeText(status, t("settings.aiConfig.picker.failed", { error: error.message }));
    } finally {
      button.disabled = false;
    }
  };
}

export function selectedNativeFilePath(result) {
  const selectionStatus = String(read(result, "status") ?? "");
  if (selectionStatus === "cancelled") {
    return null;
  }
  const selectedPath = String(read(result, "path") ?? "").trim();
  if (selectionStatus === "selected" && selectedPath) {
    return selectedPath;
  }
  throw new Error(t("settings.aiConfig.picker.invalidSelection"));
}

function syncDialogForm(modal, model, status = null) {
  const entry = model.selectedEntry();
  const fields = modal.querySelector('[data-role="ai-config-fields"]');
  if (!entry || !fields || fields.querySelector(".empty-list")) {
    return true;
  }
  try {
    const extraJsonField = fields.querySelector('[data-role="entry-extra-json"]');
    const parsedExtra = extraJsonField
      ? validateExtraJsonText(extraJsonField.value)
      : entry.extraJson;
    const cliPath = fieldValue(fields, "entry-cli-path").trim();
    const updatedExtra =
      cliPath || parsedExtra
        ? { ...(parsedExtra ?? {}), ...(cliPath ? { cli_path: cliPath } : {}) }
        : null;
    if (!cliPath && updatedExtra && Object.hasOwn(updatedExtra, "cli_path")) {
      delete updatedExtra.cli_path;
    }
    const updated = {
      ...entry,
      id: fieldValue(fields, "entry-id").trim(),
      label: fieldValue(fields, "entry-label").trim(),
      configType: fieldValue(fields, "entry-config-type"),
      apiUrl: fieldValue(fields, "entry-api-url").trim(),
      apiKey: applyApiKeyEdit(entry.apiKey, fieldValue(fields, "entry-api-key")),
      codexTomlPath: fieldValue(fields, "entry-codex-toml").trim(),
      codexJsonPath: fieldValue(fields, "entry-codex-json").trim(),
      extraJson: updatedExtra,
    };
    if (!updated.id) {
      throw new Error(t("settings.aiConfig.validation.entryIdRequired"));
    }
    model.updateSelected(updated);
    return true;
  } catch (error) {
    setRuntimeText(status, t("settings.aiConfig.status.invalid", { error: error.message }));
    return false;
  }
}

function setValidationStatus(status, report, action) {
  if (report?.ok) {
    setText(status, t(`settings.aiConfig.status.${action}Passed`));
    return;
  }
  const errors = asArray(report?.errors);
  setRuntimeText(
    status,
    t(`settings.aiConfig.status.${action}Rejected`, {
      error: errors[0] ?? t("settings.aiConfig.validation.unknown"),
    }),
  );
}

function inputField(labelText, role, value, type = "text") {
  const label = el("label", "ai-config-field");
  label.append(el("span", "editor-label", labelText));
  const input = document.createElement("input");
  input.className = "text-input";
  input.type = type;
  input.dataset.role = role;
  input.value = value ?? "";
  markRuntime(input);
  label.append(input);
  return label;
}

function pathInputField(labelText, role, value, action) {
  const label = el("label", "ai-config-field");
  label.append(el("span", "editor-label", labelText));
  const row = el("div", "path-input-row");
  const input = document.createElement("input");
  input.className = "text-input";
  input.type = "text";
  input.dataset.role = role;
  input.value = value ?? "";
  markRuntime(input);
  const button = el("button", "secondary-button", t("settings.aiConfig.action.browse"));
  button.type = "button";
  button.dataset.action = action;
  row.append(input, button);
  label.append(row);
  return label;
}

function selectField(labelText, role, options, value) {
  const label = el("label", "ai-config-field");
  label.append(el("span", "editor-label", labelText));
  const select = document.createElement("select");
  select.className = "select-input";
  select.dataset.role = role;
  for (const optionValue of options) {
    const option = document.createElement("option");
    option.value = optionValue;
    const display = enumLabel("aiConfigType", optionValue);
    option.textContent = display;
    if (display === optionValue) {
      markRuntime(option);
    }
    select.append(option);
  }
  select.value = value;
  label.append(select);
  return label;
}

function textareaField(labelText, role, value) {
  const label = el("label", "ai-config-field");
  label.append(el("span", "editor-label", labelText));
  const textarea = document.createElement("textarea");
  textarea.className = "text-area";
  textarea.rows = 6;
  textarea.dataset.role = role;
  textarea.value = value;
  markRuntime(textarea);
  label.append(textarea);
  return label;
}

function fieldValue(container, role) {
  return container.querySelector(`[data-role="${role}"]`)?.value ?? "";
}

function focusEntryControl(modal, entryId, selector) {
  const item = [...modal.querySelectorAll(".ai-entry-item")].find(
    (candidate) => candidate.dataset.entryId === entryId,
  );
  item?.querySelector(selector)?.focus();
}

function extraJsonText(value) {
  if (!value) {
    return "";
  }
  return JSON.stringify(value, null, 2);
}

function firstSelectedId(category) {
  return category.activeEntryId || category.entries[0]?.id || "";
}

function showAiConfigModal(documentRef, visible) {
  const modal = documentRef.querySelector('[data-role="ai-config-modal"]');
  if (modal) {
    setModalVisible(modal, visible);
  }
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

function bindAction(container, action, handler) {
  const button = container.querySelector(`[data-action="${action}"]`);
  if (button) {
    button.onclick = handler;
  }
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

function clear(element) {
  if (!element) {
    return;
  }
  while (element.firstChild) {
    element.firstChild.remove();
  }
}

function el(tag, className, text) {
  const element = document.createElement(tag);
  if (className) {
    element.className = className;
  }
  if (text !== undefined) {
    element.textContent = text;
  }
  return element;
}
