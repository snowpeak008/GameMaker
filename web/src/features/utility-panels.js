import { enumLabel, getLanguageMode, t } from "../i18n.js";
import { setModalVisible } from "../modal-focus.js";

export const DEFAULT_PACKAGE_VIEW = {
  step14Status: "unknown",
  canPackage: false,
  lastResult: null,
  blockingIssues: [],
};

export const DEFAULT_SAVE_INDEX = {
  schemaVersion: 1,
  currentSaveId: null,
  saves: [],
  updatedAt: "",
};

export function createUtilityPanelApis(invokeCommand) {
  return {
    patch: {
      async list() {
        try {
          return unwrapCommandResponse(
            await invokeCommand("list_patches", { request: { status: null } }),
          );
        } catch {
          return null;
        }
      },
      async analyze(request) {
        return unwrapCommandResponse(await invokeCommand("analyze_patch_request", { request }));
      },
    },
    package: {
      async load() {
        try {
          return unwrapCommandResponse(await invokeCommand("load_package_view"));
        } catch {
          return null;
        }
      },
      async packageCurrentProject(request) {
        return unwrapCommandResponse(await invokeCommand("package_current_project", { request }));
      },
      currentPackageRequest: () => null,
    },
    logs: {
      async read(request = buildReadLogEntriesRequest("ALL", 200)) {
        try {
          return unwrapCommandResponse(await invokeCommand("read_log_entries", { request }));
        } catch {
          return null;
        }
      },
      async clear() {
        return unwrapCommandResponse(await invokeCommand("clear_logs"));
      },
      async exportJsonl() {
        return unwrapCommandResponse(await invokeCommand("export_log_jsonl"));
      },
    },
    sdk: {
      async list() {
        try {
          return unwrapCommandResponse(await invokeCommand("list_sdks"));
        } catch {
          return null;
        }
      },
      async add(request) {
        return unwrapCommandResponse(await invokeCommand("add_sdk", { request }));
      },
      async updateStatus(request) {
        return unwrapCommandResponse(
          await invokeCommand("update_sdk_review_status", { request }),
        );
      },
      async approvedContext() {
        try {
          return unwrapCommandResponse(await invokeCommand("get_approved_sdk_context"));
        } catch {
          return "";
        }
      },
    },
    save: {
      async list() {
        return unwrapCommandResponse(await invokeCommand("list_saves"));
      },
      async create(request) {
        return unwrapCommandResponse(await invokeCommand("create_save", { request }));
      },
      async createBlank(request) {
        return unwrapCommandResponse(await invokeCommand("create_blank_save", { request }));
      },
      async saveProject(request) {
        return unwrapCommandResponse(await invokeCommand("save_project", { request }));
      },
      async load(request) {
        return unwrapCommandResponse(await invokeCommand("load_save", { request }));
      },
      async rename(request) {
        return unwrapCommandResponse(await invokeCommand("rename_save", { request }));
      },
      async delete(request) {
        return unwrapCommandResponse(await invokeCommand("delete_save", { request }));
      },
      async openDirectory(request) {
        return unwrapCommandResponse(await invokeCommand("open_save_directory", { request }));
      },
      async autosaveState() {
        try {
          return unwrapCommandResponse(await invokeCommand("get_autosave_state"));
        } catch {
          return null;
        }
      },
      currentState: null,
      applyLoadedState: null,
    },
  };
}

export function normalizePatchRecords(input) {
  return asArray(input).map((record) => ({
    patchId: read(record, "patchId", "patch_id") ?? "",
    request: read(record, "request") ?? "",
    status: normalizeStatus(read(record, "status") ?? "analyzed"),
    createdAt: read(record, "createdAt", "created_at") ?? "",
    updatedAt: read(record, "updatedAt", "updated_at") ?? "",
    tasks: asArray(read(record, "tasks")).map(normalizePatchTask),
    changedFiles: asArray(read(record, "changedFiles", "changed_files")),
    validationSummary: read(record, "validationSummary", "validation_summary") ?? {},
    promotedIterationSpec:
      read(record, "promotedIterationSpec", "promoted_iteration_spec") ?? "",
    errors: asArray(read(record, "errors")),
  }));
}

export function normalizePackageView(input) {
  const view = input ?? DEFAULT_PACKAGE_VIEW;
  return {
    step14Status: normalizeStatus(read(view, "step14Status", "step14_status") ?? "unknown"),
    canPackage: Boolean(read(view, "canPackage", "can_package")),
    lastResult: read(view, "lastResult", "last_result") ?? null,
    blockingIssues: asArray(read(view, "blockingIssues", "blocking_issues")),
  };
}

export function normalizePackageViewFromRunResult(result) {
  const validation = read(result, "validationReport", "validation_report") ?? {};
  const status = normalizeStatus(read(validation, "status") ?? "blocked");
  const issues = asArray(read(validation, "blockingIssues", "blocking_issues")).map((issue) => {
    const id = read(issue, "id") ?? "";
    const message = read(issue, "message") ?? "";
    return [id, message].filter(Boolean).join(": ");
  });
  return {
    step14Status: status,
    canPackage: status === "success",
    lastResult: result ?? null,
    blockingIssues: issues,
  };
}

export function normalizeLogEntries(input) {
  return asArray(input).map((entry) => ({
    timestamp: read(entry, "timestamp") ?? "",
    level: String(read(entry, "level") ?? "INFO").toUpperCase(),
    context: read(entry, "context") ?? "",
    message: read(entry, "message") ?? "",
    source: read(entry, "source") ?? "",
    metadata: read(entry, "metadata") ?? {},
  }));
}

export function normalizeSdkSpecs(input) {
  return asArray(input).map((spec) => ({
    sdkId: read(spec, "sdkId", "sdk_id") ?? "",
    name: read(spec, "name") ?? "",
    sourceUrl: read(spec, "sourceUrl", "source_url") ?? "",
    reviewStatus: normalizeStatus(read(spec, "reviewStatus", "review_status") ?? "draft"),
    summary: read(spec, "summary") ?? "",
    integrationNotes: asArray(read(spec, "integrationNotes", "integration_notes")),
    apiRequirements: asArray(read(spec, "apiRequirements", "api_requirements")),
    risks: asArray(read(spec, "risks")),
    lastSyncedAt: read(spec, "lastSyncedAt", "last_synced_at") ?? "",
    updatedAt: read(spec, "updatedAt", "updated_at") ?? "",
  }));
}

export function normalizeSaveIndex(input) {
  const source = Array.isArray(input) ? { saves: input } : input ?? DEFAULT_SAVE_INDEX;
  const currentSaveId = read(source, "currentSaveId", "current_save_id") ?? null;
  const saves = asArray(read(source, "saves")).map((entry) =>
    normalizeSaveEntry(entry, currentSaveId),
  );
  return {
    schemaVersion: Number(read(source, "schemaVersion", "schema_version") ?? 1),
    currentSaveId,
    saves,
    updatedAt: read(source, "updatedAt", "updated_at") ?? "",
    workspaceState: normalizeStatus(
      read(source, "workspaceState", "workspace_state") ??
        (currentSaveId ? "linked_save" : "unsaved"),
    ),
    draftUpdatedAt: read(source, "draftUpdatedAt", "draft_updated_at") ?? "",
    originDeletedSaveId:
      read(source, "originDeletedSaveId", "origin_deleted_save_id") ?? null,
    hasAutosave: Boolean(read(source, "hasAutosave", "has_autosave")),
  };
}

export function buildAnalyzePatchRequest(request, tasks = []) {
  return {
    request: String(request ?? "").trim(),
    tasks: asArray(tasks),
  };
}

export function validatePatchRequest(request) {
  return String(request ?? "").trim() ? "" : t("utility.patch.validation.emptyRequest");
}

export function buildPackageRequestFromSources(sources = {}) {
  return {
    integration: read(sources, "integration") ?? {},
    actual_project_file_audit:
      read(sources, "actualProjectFileAudit", "actual_project_file_audit") ?? {},
    unity_validation_summary:
      read(sources, "unityValidationSummary", "unity_validation_summary") ?? {},
  };
}

export function buildReadLogEntriesRequest(level = "ALL", limit = 200) {
  const normalized = String(level ?? "ALL").toUpperCase();
  return {
    level: normalized === "ALL" ? null : normalized,
    limit,
  };
}

export function filterLogEntries(entries, level = "ALL") {
  const normalized = String(level ?? "ALL").toUpperCase();
  const all = normalizeLogEntries(entries);
  return normalized === "ALL" ? all : all.filter((entry) => entry.level === normalized);
}

export function formatLogJsonl(entries) {
  return normalizeLogEntries(entries)
    .map((entry) => JSON.stringify(entry))
    .join("\n");
}

export function buildAddSdkRequest(name, sourceUrl = "") {
  const trimmedName = String(name ?? "").trim();
  return {
    sdk_id: slugSdkId(trimmedName),
    name: trimmedName,
    source_url: String(sourceUrl ?? "").trim(),
  };
}

export function validateSdkName(name) {
  return String(name ?? "").trim() ? "" : t("utility.sdk.validation.emptyName");
}

export function buildUpdateSdkReviewStatusRequest(sdkId, status) {
  return {
    sdk_id: String(sdkId ?? ""),
    status: normalizeStatus(status),
  };
}

export function validateSaveName(name) {
  return String(name ?? "").trim() ? "" : t("utility.save.validation.emptyName");
}

export function buildProjectStateFromDesignView(viewInput) {
  const view = read(viewInput, "view") ?? viewInput ?? {};
  const profile = {};
  const rawProfile = read(view, "profile");
  if (Array.isArray(rawProfile)) {
    for (const field of rawProfile) {
      const key = String(read(field, "key") ?? "").trim();
      if (key) {
        profile[key] = read(field, "value") ?? "";
      }
    }
  } else if (rawProfile && typeof rawProfile === "object") {
    Object.assign(profile, rawProfile);
  }

  const nodes = {};
  for (const node of asArray(read(view, "nodes"))) {
    const nodeId = read(node, "nodeId", "node_id") ?? "";
    if (!nodeId) {
      continue;
    }
    const checklist = {};
    const checklistOptions = {};
    for (const item of asArray(read(node, "checklistItems", "checklist_items"))) {
      const itemId = read(item, "itemId", "item_id") ?? "";
      if (!itemId) {
        continue;
      }
      checklist[itemId] = Boolean(read(item, "checked"));
      const groups = {};
      for (const group of asArray(read(item, "optionGroups", "option_groups"))) {
        const groupId = read(group, "groupId", "group_id") ?? "";
        if (!groupId) {
          continue;
        }
        const options = asArray(read(group, "options"));
        const selected = options
          .filter((option) => Boolean(read(option, "selected")))
          .map((option) => read(option, "optionId", "option_id") ?? "")
          .filter(Boolean);
        const primary =
          options.find((option) => Boolean(read(option, "primary")))?.optionId ??
          options.find((option) => Boolean(read(option, "primary")))?.option_id ??
          "";
        groups[groupId] = { selected, primary };
      }
      if (Object.keys(groups).length > 0) {
        checklistOptions[itemId] = groups;
      }
    }
    nodes[nodeId] = {
      decisionState: normalizeStatus(
        read(node, "decisionState", "decision_state") ??
          read(node, "effectiveState", "effective_state") ??
          "not_started",
      ),
      designNote: read(node, "designNote", "design_note") ?? "",
      riskNote: read(node, "riskNote", "risk_note") ?? "",
      notApplicableReason: read(node, "notApplicableReason", "not_applicable_reason") ?? "",
      designEntities: asArray(read(node, "designEntities", "design_entities")),
      entityValidationErrors: asArray(
        read(node, "entityValidationErrors", "entity_validation_errors"),
      ),
      checklist,
      checklistOptions,
      l4Progress: read(node, "l4Progress", "l4_progress") ?? null,
      l5Progress: read(node, "l5Progress", "l5_progress") ?? null,
      qualitySignals: read(node, "qualitySignals", "quality_signals") ?? null,
    };
  }

  return {
    projectName: read(view, "projectName", "project_name") ?? t("utility.save.defaultDisplayName"),
    profile,
    nodes,
    gameplaySystems: normalizeGameplaySystemsForProjectState(
      read(view, "gameplaySystems", "gameplay_systems"),
    ),
    aiInterview: read(view, "aiInterview", "ai_interview") ?? {},
  };
}

export function buildCreateSaveRequest(displayName, stateInput = {}) {
  return {
    display_name: String(displayName ?? "").trim() || t("utility.save.defaultDisplayName"),
    state: buildProjectStateFromDesignView(stateInput),
  };
}

export function buildCreateBlankSaveRequest(displayName, stateInput = {}) {
  return {
    display_name: String(displayName ?? "").trim() || t("utility.save.defaultDisplayName"),
    state: buildProjectStateFromDesignView(stateInput),
  };
}

export function buildSaveProjectRequest(stateInput = {}, reason = "manual_save") {
  return {
    state: buildProjectStateFromDesignView(stateInput),
    reason: String(reason ?? "").trim() || "manual_save",
  };
}

export function buildLoadSaveRequest(saveId, switchBehavior = null) {
  const request = { save_id: String(saveId ?? "").trim() };
  if (switchBehavior) {
    request.switch_behavior = String(switchBehavior);
  }
  return request;
}

export function buildRenameSaveRequest(saveId, displayName) {
  return {
    save_id: String(saveId ?? "").trim(),
    display_name: String(displayName ?? "").trim(),
  };
}

export function buildDeleteSaveRequest(saveId) {
  return { save_id: String(saveId ?? "").trim() };
}

export function buildOpenSaveDirectoryRequest(saveId) {
  return { save_id: String(saveId ?? "").trim() };
}

export async function initUtilityPanels(documentRef, apis) {
  if (!documentRef) {
    return null;
  }
  const [patches, packageView, logs, sdks, sdkContext, saveResult] = await Promise.all([
    apis.patch?.list?.() ?? null,
    apis.package?.load?.() ?? null,
    apis.logs?.read?.(buildReadLogEntriesRequest("ALL", 200)) ?? null,
    apis.sdk?.list?.() ?? null,
    apis.sdk?.approvedContext?.() ?? "",
    loadSaveIndexResult(apis.save),
  ]);
  const saveIndex = saveResult.error
    ? { __loadError: formatSaveCommandError(saveResult.error) }
    : saveResult.value;
  renderPatchPanel(documentRef, patches, apis.patch);
  renderPackagePanel(documentRef, packageView, apis.package);
  renderLogsPanel(documentRef, logs, apis.logs);
  renderSdkPanel(documentRef, sdks, sdkContext, apis.sdk);
  renderSaveManagerDialog(documentRef, saveIndex, apis.save);
  return { patches, packageView, logs, sdks, sdkContext, saveIndex };
}

async function loadSaveIndexResult(api) {
  if (!api?.list) {
    return { value: null, error: null };
  }
  try {
    return { value: await api.list(), error: null };
  } catch (error) {
    return { value: null, error };
  }
}

export function renderPatchPanel(documentRef, recordsInput, api = {}) {
  const panel = documentRef.querySelector('[data-panel="patch"]');
  if (!panel) {
    return [];
  }
  const records = normalizePatchRecords(recordsInput);
  const status = panel.querySelector('[data-role="patch-status"]');
  const requestInput = panel.querySelector('[data-role="patch-request"]');
  markContentOrigin(requestInput, "user");
  setText(
    status,
    recordsInput
      ? t("utility.patch.status.records", { count: records.length })
      : t("utility.patch.status.waiting"),
  );
  renderPatchTable(panel.querySelector('[data-role="patch-table"]'), records);
  bindAction(panel, "analyze-patch", async () => {
    const requestText = requestInput?.value ?? "";
    const validation = validatePatchRequest(requestText);
    if (validation) {
      setText(status, validation);
      return;
    }
    setText(status, t("utility.patch.status.submitting"));
    try {
      const record = await api.analyze(buildAnalyzePatchRequest(requestText));
      const latest = api.list ? await api.list() : [record, ...records];
      renderPatchPanel(documentRef, latest ?? [record, ...records], api);
    } catch (error) {
      setText(status, t("utility.patch.status.failed", { error: error.message }));
    }
  });
  bindAction(panel, "refresh-patches", async () => {
    setText(status, t("utility.patch.status.refreshing"));
    try {
      renderPatchPanel(documentRef, await api.list(), api);
    } catch (error) {
      setText(status, t("utility.patch.status.refreshFailed", { error: error.message }));
    }
  });
  return records;
}

export function renderPackagePanel(documentRef, viewInput, api = {}) {
  const panel = documentRef.querySelector('[data-panel="package"]');
  if (!panel) {
    return DEFAULT_PACKAGE_VIEW;
  }
  const view = normalizePackageView(viewInput);
  const status = panel.querySelector('[data-role="package-status"]');
  const output = panel.querySelector('[data-role="package-output"]');
  const button = panel.querySelector('[data-action="run-package"]');
  button.disabled = typeof api.packageCurrentProject !== "function";
  setText(
    status,
    !viewInput
      ? t("utility.package.status.waiting")
      : view.canPackage
        ? t("utility.package.status.ready")
        : t("utility.package.status.blocked", { count: view.blockingIssues.length }),
  );
  output.textContent = packageOutputText(view, viewInput);
  markContentOrigin(output, "artifact");
  bindAction(panel, "refresh-package", async () => {
    setText(status, t("utility.package.status.refreshing"));
    try {
      renderPackagePanel(documentRef, await api.load(), api);
    } catch (error) {
      setText(status, t("utility.package.status.refreshFailed", { error: error.message }));
    }
  });
  bindAction(panel, "run-package", async () => {
    const request = api.currentPackageRequest?.() ?? null;
    setText(status, t("utility.package.status.running"));
    try {
      const result = await api.packageCurrentProject(request);
      renderPackagePanel(documentRef, normalizePackageViewFromRunResult(result), api);
    } catch (error) {
      setText(status, t("utility.package.status.failed", { error: error.message }));
    }
  });
  return view;
}

export function renderLogsPanel(documentRef, entriesInput, api = {}) {
  const panel = documentRef.querySelector('[data-panel="logs"]');
  if (!panel) {
    return [];
  }
  const entries = normalizeLogEntries(entriesInput);
  const level = panel.querySelector('[data-role="log-level"]')?.value ?? "ALL";
  const table = panel.querySelector('[data-role="log-table"]');
  const status = panel.querySelector('[data-role="logs-status"]');
  renderLogTable(table, filterLogEntries(entries, level), entriesInput);
  setText(
    status,
    entriesInput
      ? t("utility.logs.status.count", {
          visible: filterLogEntries(entries, level).length,
          total: entries.length,
        })
      : t("utility.logs.status.waiting"),
  );
  bindAction(panel, "export-logs", async () => {
    if (!api.exportJsonl) {
      clearContentOrigin(table);
      table.textContent = t("utility.logs.output.exportUnavailable");
      setText(status, t("utility.logs.status.exportUnavailable"));
      return;
    }
    try {
      const jsonl = await api.exportJsonl();
      table.textContent = jsonl || t("utility.logs.output.empty");
      markContentOrigin(table, "log");
      setText(status, t("utility.logs.status.exported"));
    } catch (error) {
      clearContentOrigin(table);
      table.textContent = t("utility.logs.output.exportFailed", { error: error.message });
      setText(status, t("utility.logs.status.exportFailed"));
    }
  });
  bindAction(panel, "clear-logs", async () => {
    if (!api.clear) {
      clearContentOrigin(table);
      table.textContent = t("utility.logs.output.clearUnavailable");
      setText(status, t("utility.logs.status.clearUnavailable"));
      return;
    }
    try {
      renderLogsPanel(documentRef, await api.clear(), api);
    } catch (error) {
      clearContentOrigin(table);
      table.textContent = t("utility.logs.output.clearFailed", { error: error.message });
      setText(status, t("utility.logs.status.clearFailed"));
    }
  });
  const select = panel.querySelector('[data-role="log-level"]');
  if (select) {
    select.onchange = async () => {
      const request = buildReadLogEntriesRequest(select.value, 200);
      const loaded = api.read ? await api.read(request) : entries;
      renderLogsPanel(documentRef, loaded ?? entries, api);
    };
  }
  return entries;
}

export function renderSdkPanel(documentRef, specsInput, contextInput = "", api = {}) {
  const panel = documentRef.querySelector('[data-panel="sdk"]');
  if (!panel) {
    return [];
  }
  const specs = normalizeSdkSpecs(specsInput);
  const status = panel.querySelector('[data-role="sdk-status"]');
  const context = panel.querySelector('[data-role="sdk-context"]');
  markContentOrigin(panel.querySelector('[data-role="sdk-name"]'), "user");
  markContentOrigin(panel.querySelector('[data-role="sdk-url"]'), "user");
  markContentOrigin(context, "sdk");
  let selectedSdkId = panel.dataset.selectedSdkId;
  if (!specs.some((spec) => spec.sdkId === selectedSdkId)) {
    selectedSdkId = specs[0]?.sdkId ?? "";
    panel.dataset.selectedSdkId = selectedSdkId;
  }
  setText(
    status,
    specsInput
      ? t("utility.sdk.status.records", { count: specs.length })
      : t("utility.sdk.status.waiting"),
  );
  context.value = contextInput ?? "";
  renderSdkTable(panel.querySelector('[data-role="sdk-table"]'), specs, selectedSdkId, (sdkId) => {
    panel.dataset.selectedSdkId = sdkId;
    renderSdkPanel(documentRef, specs, context.value, api);
  });
  bindAction(panel, "add-sdk", async () => {
    const name = panel.querySelector('[data-role="sdk-name"]')?.value ?? "";
    const sourceUrl = panel.querySelector('[data-role="sdk-url"]')?.value ?? "";
    const validation = validateSdkName(name);
    if (validation) {
      setText(status, validation);
      return;
    }
    setText(status, t("utility.sdk.status.adding"));
    try {
      const added = await api.add(buildAddSdkRequest(name, sourceUrl));
      const latest = api.list ? await api.list() : [added, ...specs];
      renderSdkPanel(documentRef, latest ?? [added, ...specs], await safeSdkContext(api), api);
    } catch (error) {
      setText(status, t("utility.sdk.status.addFailed", { error: error.message }));
    }
  });
  for (const [action, reviewStatus] of [
    ["approve-sdk", "approved"],
    ["pending-sdk", "pending_review"],
    ["reject-sdk", "rejected"],
  ]) {
    bindAction(panel, action, async () => {
      const sdkId = panel.dataset.selectedSdkId ?? "";
      if (!sdkId) {
        setText(status, t("utility.sdk.validation.selectFirst"));
        return;
      }
      setText(
        status,
        t("utility.sdk.status.updating", {
          status: enumLabel("sdk_review_status", reviewStatus),
        }),
      );
      try {
        const updated = await api.updateStatus(
          buildUpdateSdkReviewStatusRequest(sdkId, reviewStatus),
        );
        const merged = mergeSdkSpec(specs, normalizeSdkSpecs([updated])[0]);
        renderSdkPanel(documentRef, merged, await safeSdkContext(api), api);
      } catch (error) {
        setText(status, t("utility.sdk.status.updateFailed", { error: error.message }));
      }
    });
  }
  return specs;
}

export function renderSaveManagerDialog(documentRef, indexInput, api = {}) {
  const modal = documentRef.querySelector('[data-role="save-manager-dialog"]');
  if (!modal) {
    return DEFAULT_SAVE_INDEX;
  }
  const loadError = String(indexInput?.__loadError ?? "");
  const index = normalizeSaveIndex(indexInput);
  modal.__saveIndex = index;
  modal.__saveApi = api;
  const status = modal.querySelector('[data-role="save-manager-status"]');
  const nameInput = modal.querySelector('[data-role="save-name"]');
  const renameInput = modal.querySelector('[data-role="save-rename-name"]');
  const list = modal.querySelector('[data-role="save-table"]');
  const detail = modal.querySelector('[data-role="save-detail"]');
  markContentOrigin(nameInput, "user");
  markContentOrigin(renameInput, "user");
  let selectedSaveId = modal.dataset.selectedSaveId;
  if (!index.saves.some((save) => save.saveId === selectedSaveId)) {
    selectedSaveId = index.currentSaveId ?? index.saves[0]?.saveId ?? "";
    modal.dataset.selectedSaveId = selectedSaveId;
  }
  const selectedSave = index.saves.find((save) => save.saveId === selectedSaveId) ?? null;
  const selectSave = (saveId, focus = false) => {
    modal.dataset.selectedSaveId = saveId;
    renderSaveManagerDialog(documentRef, index, api);
    if (focus) {
      const schedule = documentRef.defaultView?.queueMicrotask ?? globalThis.queueMicrotask;
      schedule?.(() => {
        modal.querySelector(`[data-save-id="${cssEscape(saveId)}"]`)?.focus();
      });
    }
  };
  if (loadError) {
    renderSaveLoadError(list, loadError);
    renderSaveDetail(detail, null);
    setText(status, t("utility.save.status.refreshFailed", { error: loadError }));
  } else {
    renderSaveList(list, index, selectedSaveId, selectSave);
    renderSaveDetail(detail, selectedSave);
    setText(status, indexInput ? formatSaveManagerStatus(index) : t("utility.save.status.waiting"));
  }
  setText(modal.querySelector('[data-role="save-count"]'), loadError ? "!" : String(index.saves.length));
  if (renameInput && documentRef.activeElement !== renameInput) {
    renameInput.value = selectedSave?.displayName ?? "";
  }
  updateSaveActionAvailability(modal, index, api);

  const openButton = documentRef.querySelector('[data-action="save-manager"]');
  if (openButton && !openButton.dataset.saveManagerBound) {
    openButton.dataset.saveManagerBound = "true";
    openButton.addEventListener("click", async () => {
      setModalVisible(modal, true);
      if (api.list) {
        const result = await runSaveOperation(
          modal,
          status,
          "utility.save.status.refreshing",
          () => api.list(),
          "utility.save.status.refreshFailed",
        );
        if (result.ok) {
          renderSaveManagerDialog(documentRef, result.value, api);
        }
      }
    });
  }

  bindAction(modal, "cancel-save-manager", () => {
    hideSaveConfirmation(modal);
    setModalVisible(modal, false);
  });
  bindAction(modal, "refresh-saves", async () => {
    const result = await runSaveOperation(
      modal,
      status,
      "utility.save.status.refreshing",
      () => api.list(),
      "utility.save.status.refreshFailed",
    );
    if (result.ok) {
      renderSaveManagerDialog(documentRef, result.value, api);
    }
  });
  bindAction(modal, "create-blank-save", async () => {
    const displayName = nameInput?.value ?? "";
    const validation = validateSaveName(displayName);
    if (validation) {
      setText(status, validation);
      return;
    }
    let state;
    try {
      state = await currentSaveState(api);
    } catch (error) {
      setText(status, t("utility.save.status.createBlankFailed", {
        error: formatSaveCommandError(error),
      }));
      return;
    }
    let committedWithWarning = false;
    let refreshFailed = false;
    const result = await runSaveOperation(
      modal,
      status,
      "utility.save.status.creatingBlank",
      async () => {
        const report = await api.createBlank(buildCreateBlankSaveRequest(displayName, state));
        committedWithWarning = commandDiagnostics(report).some(
          (diagnostic) => String(diagnostic?.level ?? "").toUpperCase() !== "INFO",
        );
        if (!(await refreshAfterCommittedSave(() => api.applyLoadedState?.(null)))) {
          refreshFailed = true;
        }
        return resolveSaveIndex(report, api);
      },
      "utility.save.status.createBlankFailed",
    );
    if (result.ok) {
      nameInput.value = "";
      renderSaveManagerDialog(documentRef, result.value, api);
      setText(
        status,
        t(
          committedWithWarning
            ? "utility.save.status.createdBlankWarning"
            : refreshFailed
              ? "utility.save.status.createdBlankRefreshWarning"
            : "utility.save.status.createdBlank",
        ),
      );
    }
  });
  bindAction(modal, "create-save-copy", async () => {
    const displayName = nameInput?.value ?? "";
    const validation = validateSaveName(displayName);
    if (validation) {
      setText(status, validation);
      return;
    }
    let state;
    try {
      state = await currentSaveState(api);
    } catch (error) {
      setText(status, t("utility.save.status.createCopyFailed", { error: error.message }));
      return;
    }
    if (!state) {
      setText(status, t("utility.save.status.designUnavailableCopy"));
      return;
    }
    let committedWithWarning = false;
    const result = await runSaveOperation(
      modal,
      status,
      "utility.save.status.creatingCopy",
      async () => {
        const report = await api.create(buildCreateSaveRequest(displayName, state));
        committedWithWarning = commandDiagnostics(report).some(
          (diagnostic) => String(diagnostic?.level ?? "").toUpperCase() !== "INFO",
        );
        return resolveSaveIndex(report, api);
      },
      "utility.save.status.createCopyFailed",
    );
    if (result.ok) {
      nameInput.value = "";
      renderSaveManagerDialog(documentRef, result.value, api);
      setText(
        status,
        t(
          committedWithWarning
            ? "utility.save.status.createdCopyWarning"
            : "utility.save.status.createdCopy",
        ),
      );
    }
  });
  bindAction(modal, "save-selected-save", async () => {
    const saveId = modal.dataset.selectedSaveId ?? "";
    if (!saveId) {
      setText(status, t("utility.save.validation.selectFirst"));
      return;
    }
    if (index.currentSaveId && saveId !== index.currentSaveId) {
      setText(status, t("utility.save.status.currentOnly"));
      return;
    }
    let state;
    try {
      state = await currentSaveState(api);
    } catch (error) {
      setText(status, t("utility.save.status.saveFailed", { error: error.message }));
      return;
    }
    if (!state) {
      setText(status, t("utility.save.status.designUnavailableSave"));
      return;
    }
    let committedWithWarning = false;
    const result = await runSaveOperation(
      modal,
      status,
      "utility.save.status.saving",
      async () => {
        const report = await api.saveProject(buildSaveProjectRequest(state, "manual_save"));
        committedWithWarning = commandDiagnostics(report).some(
          (diagnostic) => String(diagnostic?.level ?? "").toUpperCase() !== "INFO",
        );
        return resolveSaveIndex(report, api);
      },
      "utility.save.status.saveFailed",
    );
    if (result.ok) {
      renderSaveManagerDialog(documentRef, result.value, api);
      setText(
        status,
        t(
          committedWithWarning
            ? "utility.save.status.savedWarning"
            : "utility.save.status.saved",
        ),
      );
    }
  });
  bindAction(modal, "load-save", () => {
    const saveId = modal.dataset.selectedSaveId ?? "";
    if (!saveId) {
      setText(status, t("utility.save.validation.selectFirst"));
      return;
    }
    showSaveConfirmation(modal, "load", selectedSave);
  });
  bindAction(modal, "rename-save", async () => {
    const saveId = modal.dataset.selectedSaveId ?? "";
    const displayName = renameInput?.value ?? "";
    if (!saveId) {
      setText(status, t("utility.save.validation.selectFirst"));
      return;
    }
    const validation = validateSaveName(displayName);
    if (validation) {
      setText(status, validation);
      return;
    }
    const result = await runSaveOperation(
      modal,
      status,
      "utility.save.status.renaming",
      async () => resolveSaveIndex(await api.rename(buildRenameSaveRequest(saveId, displayName)), api),
      "utility.save.status.renameFailed",
    );
    if (result.ok) {
      renderSaveManagerDialog(documentRef, result.value, api);
      setText(status, t("utility.save.status.renamed"));
    }
  });
  bindAction(modal, "delete-save", () => {
    const saveId = modal.dataset.selectedSaveId ?? "";
    if (!saveId) {
      setText(status, t("utility.save.validation.selectFirst"));
      return;
    }
    showSaveConfirmation(modal, "delete", selectedSave);
  });
  bindAction(modal, "open-save-directory", async () => {
    const saveId = modal.dataset.selectedSaveId ?? "";
    if (!saveId) {
      setText(status, t("utility.save.validation.selectFirst"));
      return;
    }
    const result = await runSaveOperation(
      modal,
      status,
      "utility.save.status.openingDirectory",
      () => api.openDirectory(buildOpenSaveDirectoryRequest(saveId)),
      "utility.save.status.openDirectoryFailed",
    );
    if (result.ok) {
      setText(status, t("utility.save.status.directoryOpened"));
    }
  });
  bindAction(modal, "confirm-load-save-current", () =>
    confirmLoadSave(documentRef, modal, api, "save_current"),
  );
  bindAction(modal, "confirm-load-discard", () =>
    confirmLoadSave(documentRef, modal, api, "discard_draft"),
  );
  bindAction(modal, "confirm-delete-save", () => confirmDeleteSave(documentRef, modal, api));
  bindAction(modal, "cancel-save-confirmation", () => {
    hideSaveConfirmation(modal);
  });
  return index;
}

function renderPatchTable(container, records) {
  clearContentOrigin(container);
  clear(container);
  if (records.length === 0) {
    container.append(el("div", "empty-list", t("utility.patch.empty")));
    return;
  }
  const table = utilityTable([
    "utility.patch.table.id",
    "utility.table.status",
    "utility.patch.table.tasks",
    "utility.table.updated",
  ]);
  const body = table.querySelector("tbody");
  for (const record of records) {
    const row = document.createElement("tr");
    row.append(
      cell(record.patchId, "artifact"),
      cell(enumLabel("patch_status", record.status)),
      cell(String(record.tasks.length)),
      cell(record.updatedAt || record.createdAt, "artifact"),
    );
    body.append(row);
  }
  container.append(table);
}

function renderLogTable(container, entries, entriesInput = entries) {
  clearContentOrigin(container);
  clear(container);
  if (!entriesInput) {
    container.append(el("div", "empty-list", t("utility.logs.empty.waiting")));
    return;
  }
  if (entries.length === 0) {
    container.append(el("div", "empty-list", t("utility.logs.empty.level")));
    return;
  }
  const table = utilityTable([
    "utility.logs.table.time",
    "utility.logs.table.level",
    "utility.logs.table.context",
    "utility.logs.table.message",
  ]);
  const body = table.querySelector("tbody");
  for (const entry of entries) {
    const row = document.createElement("tr");
    row.className = `log-row level-${entry.level.toLowerCase()}`;
    row.append(
      cell(entry.timestamp, "log"),
      cell(enumLabel("log_level", entry.level)),
      cell(entry.context, "log"),
      cell(entry.message, "log"),
    );
    body.append(row);
  }
  container.append(table);
}

function renderSdkTable(container, specs, selectedSdkId, onSelect) {
  clearContentOrigin(container);
  clear(container);
  if (specs.length === 0) {
    container.append(el("div", "empty-list", t("utility.sdk.empty")));
    return;
  }
  const table = utilityTable([
    "utility.sdk.table.name",
    "utility.table.status",
    "utility.sdk.table.source",
    "utility.table.updated",
  ]);
  const body = table.querySelector("tbody");
  for (const spec of specs) {
    const row = document.createElement("tr");
    row.classList.toggle("selected-row", spec.sdkId === selectedSdkId);
    row.dataset.sdkId = spec.sdkId;
    row.addEventListener("click", () => onSelect(spec.sdkId));
    row.append(
      cell(`${spec.name}\n${spec.sdkId}`, "sdk"),
      cell(enumLabel("sdk_review_status", spec.reviewStatus)),
      cell(spec.sourceUrl, "sdk"),
      cell(spec.updatedAt, "sdk"),
    );
    body.append(row);
  }
  container.append(table);
}

function renderSaveList(container, index, selectedSaveId, onSelect) {
  clear(container);
  markContentOrigin(container, "save");
  if (!index.saves.length) {
    container.append(el("div", "empty-list", t("utility.save.empty")));
    return;
  }
  for (const save of index.saves) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "save-list-item";
    item.classList.toggle("current-save", save.isCurrent);
    item.dataset.saveId = save.saveId;
    item.setAttribute("role", "option");
    item.setAttribute("aria-selected", String(save.saveId === selectedSaveId));
    item.tabIndex = save.saveId === selectedSaveId ? 0 : -1;

    const nameRow = el("span", "save-list-name-row");
    nameRow.append(el("span", "save-list-name", save.displayName));
    if (save.isCurrent) {
      nameRow.append(saveBadge("current", t("utility.save.badge.current")));
    }
    if (save.lockedByOther) {
      nameRow.append(saveBadge("locked", t("utility.save.badge.locked")));
    }
    if (save.integrityStatus !== "unknown") {
      nameRow.append(
        saveBadge(
          `integrity-${save.integrityStatus}`,
          integrityStatusLabel(save.integrityStatus),
        ),
      );
    }
    const meta = el(
      "span",
      "save-list-meta",
      formatSaveTimestamp(save.lastWorkedAt || save.createdAt),
    );
    const progress = el("span", "save-list-progress");
    progress.append(
      el("span", "", save.progress.pipelineLabel),
      el("span", "", save.progress.designLabel),
    );
    item.append(nameRow, meta, progress);
    item.addEventListener("click", () => onSelect(save.saveId));
    container.append(item);
  }
  container.onkeydown = (event) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End", "Enter", " "].includes(event.key)) {
      return;
    }
    const items = Array.from(container.querySelectorAll(".save-list-item"));
    if (!items.length) {
      return;
    }
    const active = document.activeElement?.closest?.(".save-list-item");
    let position = Math.max(0, items.indexOf(active));
    if (event.key === "ArrowDown") {
      position = Math.min(items.length - 1, position + 1);
    } else if (event.key === "ArrowUp") {
      position = Math.max(0, position - 1);
    } else if (event.key === "Home") {
      position = 0;
    } else if (event.key === "End") {
      position = items.length - 1;
    }
    event.preventDefault();
    onSelect(items[position].dataset.saveId, true);
  };
}

function renderSaveLoadError(container, message) {
  clear(container);
  clearContentOrigin(container);
  container.append(
    el("div", "save-load-error", t("utility.save.listError", { error: message })),
  );
}

function renderSaveDetail(container, save) {
  clear(container);
  markContentOrigin(container, "save");
  if (!save) {
    container.append(el("div", "empty-list", t("utility.save.detailEmpty")));
    return;
  }
  const heading = el("div", "save-detail-heading");
  heading.append(el("strong", "", save.displayName));
  const badges = el("div", "save-detail-badges");
  if (save.isCurrent) {
    badges.append(saveBadge("current", t("utility.save.badge.current")));
  }
  if (save.lockedByOther) {
    badges.append(saveBadge("locked", t("utility.save.badge.locked")));
  }
  badges.append(
    saveBadge(
      `integrity-${save.integrityStatus}`,
      integrityStatusLabel(save.integrityStatus),
    ),
  );
  heading.append(badges);

  const fields = document.createElement("dl");
  fields.className = "save-detail-grid";
  appendSaveDetail(fields, "utility.save.detail.id", save.saveId);
  appendSaveDetail(fields, "utility.save.detail.path", save.path || t("utility.save.notAvailable"));
  appendSaveDetail(fields, "utility.save.detail.type", saveTypeLabel(save.saveType));
  appendSaveDetail(fields, "utility.save.detail.created", formatSaveTimestamp(save.createdAt));
  appendSaveDetail(
    fields,
    "utility.save.detail.lastWorked",
    formatSaveTimestamp(save.lastWorkedAt || save.createdAt),
  );
  appendSaveDetail(fields, "utility.save.detail.pipeline", save.progress.pipelineLabel);
  appendSaveDetail(fields, "utility.save.detail.design", save.progress.designLabel);
  appendSaveDetail(
    fields,
    "utility.save.detail.transaction",
    t("utility.save.transactionValue", { seq: save.lastTransactionSeq }),
  );
  appendSaveDetail(
    fields,
    "utility.save.detail.workspace",
    t("utility.save.workspaceValue", {
      count: save.workspaceFileCount,
      size: formatSaveBytes(save.workspaceBytes),
    }),
  );
  appendSaveDetail(
    fields,
    "utility.save.detail.integrity",
    save.integrityMessage
      ? `${integrityStatusLabel(save.integrityStatus)} · ${save.integrityMessage}`
      : integrityStatusLabel(save.integrityStatus),
  );
  appendSaveDetail(fields, "utility.save.detail.lock", saveLockLabel(save));
  appendSaveDetail(
    fields,
    "utility.save.detail.reason",
    save.reason || t("utility.save.notAvailable"),
  );
  container.append(heading, fields);
}

function appendSaveDetail(container, labelKey, value) {
  const term = document.createElement("dt");
  term.textContent = t(labelKey);
  const detail = document.createElement("dd");
  detail.textContent = value ?? t("utility.save.notAvailable");
  container.append(term, detail);
}

function saveBadge(className, label) {
  return el("span", `save-status-badge ${className}`, label);
}

function integrityStatusLabel(status) {
  return t(`utility.save.integrity.${normalizeIntegrityStatus(status)}`);
}

function saveTypeLabel(value) {
  const type = normalizeStatus(value || "unknown");
  const key = ["manual", "iteration", "auto", "unknown"].includes(type)
    ? type
    : "unknown";
  return t(`utility.save.type.${key}`);
}

function saveLockLabel(save) {
  if (!save.lockedByOther) {
    return t("utility.save.lock.available");
  }
  const owner = [
    save.lockOwnerPid ? `PID ${save.lockOwnerPid}` : "",
    save.lockOwnerSession,
  ].filter(Boolean).join(" · ");
  return owner
    ? t("utility.save.lock.owner", { owner })
    : t("utility.save.lock.other");
}

function showSaveConfirmation(modal, kind, save) {
  if (!save || modal.dataset.busy === "true") {
    return;
  }
  const layer = modal.querySelector('[data-role="save-confirmation"]');
  if (!layer) {
    return;
  }
  layer.dataset.kind = kind;
  layer.dataset.saveId = save.saveId;
  const title = layer.querySelector('[data-role="save-confirmation-title"]');
  const message = layer.querySelector('[data-role="save-confirmation-message"]');
  const isLoad = kind === "load";
  setText(title, t(isLoad ? "utility.save.confirm.loadTitle" : "utility.save.confirm.deleteTitle"));
  setText(
    message,
    t(isLoad ? "utility.save.confirm.loadMessage" : "utility.save.confirm.deleteMessage", {
      name: save.displayName,
    }),
  );
  markContentOrigin(message, "save");
  const error = layer.querySelector('[data-role="save-confirmation-error"]');
  if (error) {
    error.hidden = true;
    error.textContent = "";
  }
  for (const action of ["confirm-load-save-current", "confirm-load-discard"]) {
    const button = layer.querySelector(`[data-action="${action}"]`);
    if (button) {
      button.hidden = !isLoad;
    }
  }
  const deleteButton = layer.querySelector('[data-action="confirm-delete-save"]');
  if (deleteButton) {
    deleteButton.hidden = isLoad;
  }
  layer.hidden = false;
  layer.querySelector('[data-action="cancel-save-confirmation"]')?.focus();
}

function hideSaveConfirmation(modal) {
  const layer = modal.querySelector('[data-role="save-confirmation"]');
  if (!layer || modal.dataset.busy === "true") {
    return;
  }
  layer.hidden = true;
  const error = layer.querySelector('[data-role="save-confirmation-error"]');
  if (error) {
    error.hidden = true;
    error.textContent = "";
  }
  delete layer.dataset.kind;
  delete layer.dataset.saveId;
}

async function confirmLoadSave(documentRef, modal, api, switchBehavior) {
  const layer = modal.querySelector('[data-role="save-confirmation"]');
  const saveId = layer?.dataset.saveId ?? "";
  if (!saveId || layer?.dataset.kind !== "load") {
    return;
  }
  const status = modal.querySelector('[data-role="save-manager-status"]');
  let committedWithWarning = false;
  let refreshFailed = false;
  const result = await runSaveOperation(
    modal,
    status,
    "utility.save.status.loading",
    async () => {
      await currentSaveState(api);
      const loaded = await api.load(buildLoadSaveRequest(saveId, switchBehavior));
      committedWithWarning = commandDiagnostics(loaded).some(
        (diagnostic) => String(diagnostic?.level ?? "").toUpperCase() !== "INFO",
      );
      if (!(await refreshAfterCommittedSave(
        () => api.applyLoadedState?.(loaded?.state ?? null),
      ))) {
        refreshFailed = true;
      }
      try {
        return await resolveSaveIndex(loaded, api);
      } catch {
        refreshFailed = true;
        return modal.__saveIndex ?? DEFAULT_SAVE_INDEX;
      }
    },
    "utility.save.status.loadFailed",
  );
  if (result.ok) {
    hideSaveConfirmation(modal);
    renderSaveManagerDialog(documentRef, result.value, api);
    setText(
      status,
      t(
        committedWithWarning
          ? "utility.save.status.loadedWarning"
          : refreshFailed
            ? "utility.save.status.loadedRefreshWarning"
            : "utility.save.status.loaded",
      ),
    );
  } else if (result.error) {
    renderSaveConfirmationError(
      layer,
      t("utility.save.status.loadFailed", {
        error: formatSaveCommandError(result.error),
      }),
    );
  }
}

async function confirmDeleteSave(documentRef, modal, api) {
  const layer = modal.querySelector('[data-role="save-confirmation"]');
  const saveId = layer?.dataset.saveId ?? "";
  if (!saveId || layer?.dataset.kind !== "delete") {
    return;
  }
  const status = modal.querySelector('[data-role="save-manager-status"]');
  const result = await runSaveOperation(
    modal,
    status,
    "utility.save.status.deleting",
    async () => resolveSaveIndex(await api.delete(buildDeleteSaveRequest(saveId)), api),
    "utility.save.status.deleteFailed",
  );
  if (result.ok) {
    hideSaveConfirmation(modal);
    renderSaveManagerDialog(documentRef, result.value, api);
    setText(status, t("utility.save.status.deleted"));
  } else if (result.error) {
    renderSaveConfirmationError(
      layer,
      t("utility.save.status.deleteFailed", {
        error: formatSaveCommandError(result.error),
      }),
    );
  }
}

function renderSaveConfirmationError(layer, message) {
  const error = layer?.querySelector('[data-role="save-confirmation-error"]');
  if (!error) {
    return;
  }
  error.textContent = message;
  error.hidden = false;
}

async function resolveSaveIndex(report, api) {
  if (report?.index) {
    return report.index;
  }
  if (Array.isArray(report) || Array.isArray(report?.saves)) {
    return report;
  }
  if (api?.list) {
    return api.list();
  }
  return DEFAULT_SAVE_INDEX;
}

async function runSaveOperation(modal, status, statusKey, operation, errorKey) {
  if (modal.dataset.busy === "true") {
    return { ok: false, busy: true };
  }
  setSaveManagerBusy(modal, true);
  setText(status, t(statusKey));
  try {
    return { ok: true, value: await operation() };
  } catch (error) {
    setText(status, t(errorKey, { error: formatSaveCommandError(error) }));
    return { ok: false, error };
  } finally {
    setSaveManagerBusy(modal, false);
  }
}

function setSaveManagerBusy(modal, busy) {
  modal.dataset.busy = String(Boolean(busy));
  modal.setAttribute("aria-busy", String(Boolean(busy)));
  for (const element of modal.querySelectorAll("[data-save-action], [data-action='cancel-save-manager'], [data-action='cancel-save-confirmation'], [data-role='save-name'], [data-role='save-rename-name']")) {
    element.disabled = Boolean(busy);
  }
  if (!busy) {
    updateSaveActionAvailability(modal, modal.__saveIndex ?? DEFAULT_SAVE_INDEX, modal.__saveApi ?? {});
  }
}

function updateSaveActionAvailability(modal, index, api) {
  const busy = modal.dataset.busy === "true";
  const selected = index.saves.find((save) => save.saveId === modal.dataset.selectedSaveId) ?? null;
  const locked = Boolean(selected?.lockedByOther);
  const corrupt = selected?.integrityStatus === "error";
  setActionDisabled(modal, "refresh-saves", busy || !api.list);
  setActionDisabled(modal, "create-blank-save", busy || !api.createBlank);
  setActionDisabled(modal, "create-save-copy", busy || !api.create);
  setActionDisabled(modal, "rename-save", busy || !selected || locked || corrupt || !api.rename);
  setActionDisabled(
    modal,
    "save-selected-save",
    busy || !selected?.isCurrent || locked || corrupt || !api.saveProject,
  );
  setActionDisabled(
    modal,
    "load-save",
    busy || !selected || selected.isCurrent || locked || corrupt || !api.load,
  );
  setActionDisabled(modal, "open-save-directory", busy || !selected || !api.openDirectory);
  setActionDisabled(modal, "delete-save", busy || !selected || locked || !api.delete);
  setActionDisabled(modal, "confirm-load-save-current", busy || !api.load);
  setActionDisabled(modal, "confirm-load-discard", busy || !api.load);
  setActionDisabled(modal, "confirm-delete-save", busy || !api.delete);
}

function setActionDisabled(modal, action, disabled) {
  const button = modal.querySelector(`[data-action="${action}"]`);
  if (button) {
    button.disabled = Boolean(disabled);
  }
}

function cssEscape(value) {
  return String(value ?? "").replace(/([\\"'])/g, "\\$1");
}

function packageOutputText(view, rawInput) {
  const lines = [
    t("utility.package.output.step14Status", {
      status: enumLabel("package_status", view.step14Status),
    }),
    t("utility.package.output.canPackage", {
      value: t(view.canPackage ? "utility.boolean.yes" : "utility.boolean.no"),
    }),
    t("utility.package.output.blockingIssues", { count: view.blockingIssues.length }),
  ];
  for (const issue of view.blockingIssues) {
    lines.push(`- ${issue}`);
  }
  if (rawInput) {
    lines.push("", JSON.stringify(rawInput, null, 2));
  }
  return lines.join("\n");
}

async function safeSdkContext(api) {
  return api.approvedContext ? await api.approvedContext() : "";
}

function mergeSdkSpec(specs, updated) {
  if (!updated) {
    return specs;
  }
  const merged = specs.filter((spec) => spec.sdkId !== updated.sdkId);
  merged.push(updated);
  return merged.sort((left, right) => left.name.localeCompare(right.name));
}

async function currentSaveState(api) {
  const direct = await api.currentState?.();
  if (direct) {
    return direct;
  }
  return (await api.autosaveState?.()) ?? null;
}

function normalizeSaveEntry(entry, currentSaveId = null) {
  const progress = normalizeSaveProgress(read(entry, "progress"));
  const saveId = read(entry, "saveId", "save_id") ?? "";
  const integrityStatus = normalizeIntegrityStatus(
    read(entry, "integrityStatus", "integrity_status") ?? "unknown",
  );
  return {
    saveId,
    displayName:
      read(entry, "displayName", "display_name") ?? t("utility.save.defaultDisplayName"),
    saveType: read(entry, "saveType", "save_type") ?? "manual",
    createdBy: read(entry, "createdBy", "created_by") ?? "",
    reason: read(entry, "reason") ?? "",
    path: read(entry, "path") ?? "",
    createdAt: read(entry, "createdAt", "created_at") ?? "",
    lastWorkedAt: read(entry, "lastWorkedAt", "last_worked_at") ?? "",
    lastTransactionSeq: Number(
      read(entry, "lastTransactionSeq", "last_transaction_seq") ?? 0,
    ),
    lockedByOther: Boolean(read(entry, "lockedByOther", "locked_by_other")),
    lockOwnerPid: read(entry, "lockOwnerPid", "lock_owner_pid") ?? null,
    lockOwnerSession: read(entry, "lockOwnerSession", "lock_owner_session") ?? "",
    integrityStatus,
    integrityMessage: read(entry, "integrityMessage", "integrity_message") ?? "",
    workspaceFileCount: Number(
      read(entry, "workspaceFileCount", "workspace_file_count") ?? 0,
    ),
    workspaceBytes: Number(read(entry, "workspaceBytes", "workspace_bytes") ?? 0),
    progress,
    progressLabel: progress.pipelineLabel,
    isCurrent: currentSaveId === saveId,
  };
}

function formatSaveManagerStatus(index) {
  const records = t(
    index.currentSaveId
      ? "utility.save.status.recordsWithCurrent"
      : "utility.save.status.records",
    { count: index.saves.length },
  );
  const knownStates = ["linked_save", "unsaved", "unsaved_copy_of_deleted_save"];
  const state = knownStates.includes(index.workspaceState) ? index.workspaceState : "unknown";
  const stateLabel = t(`utility.save.draftState.${state}`, {
    saveId: index.originDeletedSaveId ?? "-",
  });
  return `${records} · ${t("utility.save.draftStatus", {
    state: stateLabel,
    autosave: t(
      index.hasAutosave
        ? "utility.save.autosave.available"
        : "utility.save.autosave.unavailable",
    ),
    updated: formatSaveTimestamp(index.draftUpdatedAt),
  })}`;
}

export function commandDiagnostics(value) {
  const diagnostics = Array.isArray(value?.__commandDiagnostics)
    ? [...value.__commandDiagnostics]
    : [];
  const messages = new Set(diagnostics.map((diagnostic) => String(diagnostic?.message ?? "")));
  for (const warning of Array.isArray(value?.warnings) ? value.warnings : []) {
    const message = String(warning ?? "").trim();
    if (message && !messages.has(message)) {
      diagnostics.push({ level: "WARNING", message });
      messages.add(message);
    }
  }
  return diagnostics;
}

export async function refreshAfterCommittedSave(refresh) {
  try {
    await refresh?.();
    return true;
  } catch {
    return false;
  }
}

export function formatSaveCommandError(error) {
  const code = String(error?.code ?? "").trim();
  const keyByCode = {
    SAVE_LOCKED: "locked",
    SAVE_CORRUPT: "corrupt",
    PATH_GUARD_FAILED: "pathGuard",
    NOT_FOUND: "notFound",
    VALIDATION_FAILED: "validation",
    BACKEND_UNAVAILABLE: "backendUnavailable",
    COMMAND_FAILED: "commandFailed",
    pipeline_save_conflict: "pipelineRunning",
    save_as_required: "saveAsRequired",
    save_index_read_failed: "indexRead",
    save_sync_before_load_failed: "syncBeforeLoad",
    save_path_invalid: "pathGuard",
    save_not_found: "notFound",
    open_save_directory_failed: "openDirectory",
  };
  const key = keyByCode[code];
  if (key) {
    return t(`utility.save.error.${key}`);
  }
  if (code) {
    return t("utility.save.error.genericCode", { code });
  }
  const message = String(error?.message ?? error ?? "").trim();
  return message || t("utility.save.error.commandFailed");
}

export function normalizeSaveProgress(progressInput) {
  const progress = progressInput && typeof progressInput === "object" ? progressInput : {};
  const pipelinePassed = finiteNumber(
    read(progress, "pipelinePassed", "pipeline_passed") ?? read(progress, "passed"),
  );
  const pipelineTotal = finiteNumber(
    read(progress, "pipelineTotal", "pipeline_total") ?? read(progress, "total"),
  );
  const designPassed = finiteNumber(read(progress, "designPassed", "design_passed"));
  const designTotal = finiteNumber(read(progress, "designTotal", "design_total"));
  return {
    passed: pipelinePassed,
    total: pipelineTotal,
    label: read(progress, "label") ?? "",
    pipelinePassed,
    pipelineTotal,
    pipelineSourceLabel: read(progress, "pipelineLabel", "pipeline_label") ?? "",
    pipelineLabel: t("utility.save.pipelineProgress", {
      passed: pipelinePassed,
      total: pipelineTotal,
    }),
    designPassed,
    designTotal,
    designSourceLabel: read(progress, "designLabel", "design_label") ?? "",
    designLabel: t("utility.save.designProgress", {
      passed: designPassed,
      total: designTotal,
    }),
  };
}

export function formatSaveTimestamp(value) {
  const raw = String(value ?? "").trim();
  if (!raw) {
    return t("utility.save.notAvailable");
  }
  let milliseconds;
  if (raw.startsWith("unix:")) {
    milliseconds = Number(raw.slice(5)) * 1000;
  } else if (/^\d+(?:\.\d+)?$/.test(raw)) {
    const numeric = Number(raw);
    milliseconds = numeric < 100_000_000_000 ? numeric * 1000 : numeric;
  } else {
    milliseconds = Date.parse(raw);
  }
  if (!Number.isFinite(milliseconds)) {
    return raw;
  }
  return new Intl.DateTimeFormat(getLanguageMode(), {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(milliseconds));
}

export function formatSaveBytes(value) {
  const bytes = Math.max(0, finiteNumber(value));
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let amount = bytes;
  let unit = "B";
  for (const candidate of units) {
    amount /= 1024;
    unit = candidate;
    if (amount < 1024 || candidate === units.at(-1)) {
      break;
    }
  }
  return `${new Intl.NumberFormat(getLanguageMode(), {
    maximumFractionDigits: amount < 10 ? 1 : 0,
  }).format(amount)} ${unit}`;
}

function finiteNumber(value) {
  const numeric = Number(value ?? 0);
  return Number.isFinite(numeric) ? numeric : 0;
}

function normalizeIntegrityStatus(value) {
  const status = normalizeStatus(value || "unknown");
  if (["ok", "valid", "healthy", "passed"].includes(status)) {
    return "ok";
  }
  if (["warning", "degraded", "missing_files"].includes(status)) {
    return "warning";
  }
  if (["error", "failed", "corrupt", "invalid"].includes(status)) {
    return "error";
  }
  return "unknown";
}

function normalizeGameplaySystemsForProjectState(input) {
  const value = input && typeof input === "object" ? input : {};
  const weights = {};
  for (const [systemId, rawWeight] of Object.entries(read(value, "weights") ?? {})) {
    if (rawWeight && typeof rawWeight === "object" && !Array.isArray(rawWeight)) {
      weights[systemId] = {
        weight: read(rawWeight, "weight") ?? "",
        weight_type: read(rawWeight, "weightType", "weight_type") ?? "percent",
      };
    } else {
      weights[systemId] = { weight: rawWeight, weight_type: "percent" };
    }
  }
  const interview = read(value, "interview") ?? {};
  return {
    schemaVersion: read(value, "schemaVersion", "schema_version") ?? "1.0",
    selected: asArray(read(value, "selected")).map(String),
    custom: asArray(read(value, "custom")).map((item) => ({
      id: read(item, "id") ?? "",
      name: read(item, "name") ?? read(item, "id") ?? "",
      category: read(item, "category") ?? "custom",
      mapping_desc: read(item, "mappingDesc", "mapping_desc") ?? "",
    })),
    weights,
    coreLoops: read(value, "coreLoops", "core_loops") ?? {},
    interview: {
      questions: asArray(read(interview, "questions")).map(String),
      answers: asArray(read(interview, "answers")).map(String),
      parsedSystemIds: asArray(read(interview, "parsedSystemIds", "parsed_system_ids")).map(String),
    },
  };
}

function normalizePatchTask(task) {
  return {
    taskId: read(task, "taskId", "task_id") ?? "",
    title: read(task, "title") ?? "",
    description: read(task, "description") ?? "",
    affectedSystems: asArray(read(task, "affectedSystems", "affected_systems")),
    expectedFiles: asArray(read(task, "expectedFiles", "expected_files")),
    validationRoute: asArray(read(task, "validationRoute", "validation_route")),
    requiresIteration: Boolean(read(task, "requiresIteration", "requires_iteration")),
  };
}

function utilityTable(headers) {
  const table = document.createElement("table");
  table.className = "utility-table";
  const thead = document.createElement("thead");
  const headRow = document.createElement("tr");
  for (const header of headers) {
    const th = document.createElement("th");
    th.textContent = t(header);
    headRow.append(th);
  }
  thead.append(headRow);
  table.append(thead, document.createElement("tbody"));
  return table;
}

function cell(text, contentOrigin = null) {
  const td = document.createElement("td");
  td.textContent = text ?? "";
  markContentOrigin(td, contentOrigin);
  return td;
}

function slugSdkId(name) {
  return String(name ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 64);
}

function normalizeStatus(value) {
  return String(value ?? "")
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/\s+/g, "_")
    .toLowerCase();
}

function unwrapCommandResponse(response) {
  if (response && typeof response.ok === "boolean") {
    if (response.ok) {
      const data = response.data ?? null;
      if (data && typeof data === "object" && Array.isArray(response.diagnostics)) {
        Object.defineProperty(data, "__commandDiagnostics", {
          value: response.diagnostics,
          configurable: true,
        });
      }
      return data;
    }
    const detail = response.error?.message ?? response.error?.code ?? t("utility.error.commandFailed");
    const error = new Error(detail);
    error.code = response.error?.code ?? "";
    error.recoverable = response.error?.recoverable !== false;
    throw error;
  }
  return response ?? null;
}

function bindAction(panel, action, handler) {
  const button = panel.querySelector(`[data-action="${action}"]`);
  if (button) {
    button.onclick = handler;
  }
}

function setText(element, text) {
  if (element) {
    element.textContent = text;
  }
}

function markContentOrigin(element, origin) {
  if (element?.dataset && origin) {
    element.dataset.contentOrigin = origin;
  }
  return element;
}

function clearContentOrigin(element) {
  if (element?.dataset) {
    delete element.dataset.contentOrigin;
  }
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
