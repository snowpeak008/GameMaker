import { enumLabel, getLanguageMode, t } from "../i18n.js";

export const DEFAULT_PIPELINE_VIEW = {
  orderedStageIds: [],
  stages: [],
  state: {
    runId: "",
    attemptId: "",
    parentAttemptId: null,
    attemptNo: 0,
    stateVersion: 0,
    status: "idle",
    stopRequested: false,
    fromStageId: "",
    toStageId: "",
    stageIds: [],
    currentStageId: null,
    currentUnitId: null,
    stages: {},
  },
  currentStageId: null,
  running: false,
  waitingConfirmation: false,
  styleOptions: [],
  recovery: null,
};

const stylePreviewObjectUrls = new WeakMap();
let stylePreviewToken = 0;

export function createPipelineApi(invokeCommand) {
  return {
    async load() {
      return unwrapCommandResponse(await invokeCommand("load_pipeline_view"));
    },
    async runRange(request) {
      return unwrapCommandResponse(await invokeCommand("run_pipeline_range", { request }));
    },
    async stop() {
      return unwrapCommandResponse(await invokeCommand("stop_pipeline"));
    },
    async resume(request) {
      return unwrapCommandResponse(await invokeCommand("resume_pipeline", { request }));
    },
    async confirmStyle(request) {
      return unwrapCommandResponse(await invokeCommand("confirm_style", { request }));
    },
    async readStylePreview(request) {
      return unwrapCommandResponse(
        await invokeCommand("read_pipeline_artifact", { request }),
      );
    },
  };
}

export function normalizePipelineView(input) {
  const view = input ?? DEFAULT_PIPELINE_VIEW;
  const state = normalizePipelineState(read(view, "state") ?? DEFAULT_PIPELINE_VIEW.state);
  const orderedStageIds = asArray(read(view, "orderedStageIds", "ordered_stage_ids"));
  const stageViews = asArray(read(view, "stages")).map(normalizeStageView);
  return {
    orderedStageIds,
    stages: stageViews.length > 0 ? stageViews : orderedStageIds.map((stageId) => stageFromState(stageId, state)),
    state,
    currentStageId: read(view, "currentStageId", "current_stage_id") ?? state.currentStageId,
    running: Boolean(read(view, "running")),
    waitingConfirmation: Boolean(read(view, "waitingConfirmation", "waiting_confirmation")),
    styleOptions: asArray(read(view, "styleOptions", "style_options")).map(normalizeStyleOption),
    recovery: normalizePipelineRecovery(read(view, "recovery")),
    loadError: read(view, "loadError", "load_error") ?? "",
  };
}

export function createPipelineModel(viewInput) {
  const view = normalizePipelineView(viewInput);
  let selectedStageId = view.currentStageId ?? view.stages[0]?.stageId ?? "";
  return {
    view,
    get selectedStageId() {
      return selectedStageId;
    },
    selectStage(stageId) {
      selectedStageId = stageId;
      return selectedStageId;
    },
    selectedStage() {
      return view.stages.find((stage) => stage.stageId === selectedStageId) ?? view.stages[0] ?? null;
    },
    runtimeLines() {
      const lines = [
        t("pipeline.runtime.summary", {
          run: view.state.runId || t("pipeline.runtime.none"),
          status: enumLabel("pipelineStatus", view.state.status),
        }),
      ];
      if (view.state.attemptId) {
        lines.push(
          t("pipeline.runtime.attempt", {
            attempt: view.state.attemptId,
            number: view.state.attemptNo || 1,
            version: view.state.stateVersion,
          }),
        );
      }
      if (view.state.currentStageId || view.state.currentUnitId) {
        lines.push(
          t("pipeline.runtime.current", {
            stage: view.state.currentStageId || "-",
            unit: view.state.currentUnitId || "-",
          }),
        );
      }
      if (view.state.stopRequested) {
        lines.push(t("pipeline.runtime.stopPending"));
      }
      if (view.recovery) {
        lines.push(
          t("pipeline.runtime.recovery", {
            next: view.recovery.nextUnitId || "-",
            updated: view.recovery.updatedAt || "-",
          }),
        );
      }
      if (view.loadError) {
        lines.push(`LOAD ERROR ${view.loadError}`);
      }
      for (const stage of view.stages) {
        if (stage.status !== "pending") {
          lines.push(`${stage.stageId} ${stage.status} ${stage.message}`.trim());
        }
        for (const issue of stage.errors) {
          lines.push(`${stage.stageId} ERROR ${issueText(issue)}`);
        }
        for (const issue of stage.warnings) {
          lines.push(`${stage.stageId} WARNING ${issueText(issue)}`);
        }
      }
      return lines;
    },
  };
}

export function buildRunPipelineRequest(fromStageId, toStageId, options = {}) {
  return {
    from_stage_id: String(fromStageId ?? ""),
    to_stage_id: String(toStageId ?? ""),
    skip_manual_gates: Boolean(options.skipManualGates ?? options.skip_manual_gates),
    artifact_locale: getLanguageMode(),
  };
}

export function buildConfirmStyleRequest(stageId, optionId, notes) {
  const note = [optionId ? `style=${optionId}` : "", notes ? `notes=${notes}` : ""]
    .filter(Boolean)
    .join("; ");
  return {
    stage_id: String(stageId ?? "07"),
    selected_style_id: String(optionId ?? ""),
    notes: String(notes ?? ""),
    message: note || "style confirmed",
  };
}

export function buildResumePipelineRequest(recovery) {
  return {
    run_id: String(read(recovery, "runId", "run_id") ?? "").trim(),
    expected_revision: Number(read(recovery, "revision") ?? 0),
  };
}

export function buildReadStep07PreviewRequest(relativePath, maxBytes = null) {
  const request = {
    stage_id: "07",
    relative_path: String(relativePath ?? "").trim(),
  };
  if (Number.isFinite(maxBytes) && maxBytes > 0) {
    request.max_bytes = Math.floor(maxBytes);
  }
  return request;
}

export function pipelineImageBlob(input) {
  const contentType = String(read(input, "contentType", "content_type") ?? "")
    .trim()
    .toLowerCase();
  const encoding = String(read(input, "encoding") ?? "").trim().toLowerCase();
  const content = String(read(input, "content") ?? "").trim();
  const allowedTypes = new Set(["image/png", "image/jpeg", "image/webp"]);
  if (
    !allowedTypes.has(contentType) ||
    encoding !== "base64" ||
    Boolean(read(input, "truncated")) ||
    !content ||
    content.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]*={0,2}$/u.test(content)
  ) {
    return null;
  }
  try {
    const binary = globalThis.atob(content);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }
    return new Blob([bytes], { type: contentType });
  } catch {
    return null;
  }
}

export function createPipelineImageObjectUrl(input, urlApi = globalThis.URL) {
  const blob = pipelineImageBlob(input);
  if (!blob || typeof urlApi?.createObjectURL !== "function") {
    return "";
  }
  return urlApi.createObjectURL(blob);
}

export async function initPipelinePanel(documentRef, api) {
  if (!documentRef) {
    return null;
  }
  const controller = new PipelinePanelController(documentRef, api);
  await controller.reload();
  return controller;
}

export class PipelinePanelController {
  constructor(documentRef, api = {}, renderer = renderPipelinePanel) {
    this.documentRef = documentRef;
    this.api = api;
    this.renderer = renderer;
    this.model = null;
    this.pollTimer = null;
    this.renderApi = {
      ...api,
      applyView: (view) => this.render(view),
      reloadView: () => this.reload(),
    };
  }

  get view() {
    return this.model?.view ?? null;
  }

  render(view) {
    this.model = this.renderer(this.documentRef, view, this.renderApi);
    this.scheduleReloadWhileRunning();
    return this.model;
  }

  scheduleReloadWhileRunning() {
    if (this.pollTimer) {
      clearTimeout(this.pollTimer);
      this.pollTimer = null;
    }
    if (!this.model?.view?.running || !this.api.load) {
      return;
    }
    this.pollTimer = setTimeout(() => {
      this.pollTimer = null;
      void this.reload();
    }, 750);
  }

  async reload() {
    try {
      return this.render(await this.api.load());
    } catch (error) {
      return this.render({
        ...DEFAULT_PIPELINE_VIEW,
        state: { ...DEFAULT_PIPELINE_VIEW.state, status: "failed" },
        loadError: error.message,
      });
    }
  }
}

export function renderPipelinePanel(documentRef, viewInput, api = {}) {
  const panel = documentRef.querySelector('[data-panel="pipeline"]');
  if (!panel) {
    return null;
  }
  const model = createPipelineModel(viewInput);
  renderStageSelects(panel, model.view.stages);
  renderStageList(panel.querySelector('[data-role="pipeline-step-list"]'), model, () => rerender());
  renderDetail(panel, model, api);
  renderRuntimeLog(panel, model);
  bindPipelineActions(panel, model, api);
  setPipelineRunning(panel, model.view.running);
  setPipelineRecovery(panel, model.view);
  return model;

  function rerender() {
    renderStageList(panel.querySelector('[data-role="pipeline-step-list"]'), model, rerender);
    renderDetail(panel, model, api);
    bindPipelineActions(panel, model, api);
    setPipelineRecovery(panel, model.view);
  }
}

function renderStageSelects(panel, stages) {
  for (const role of ["pipeline-from", "pipeline-to"]) {
    const input = panel.querySelector(`[data-role="${role}"]`);
    const options = panel.querySelector(`[data-role="${role}-options"]`);
    if (!input || !options) {
      continue;
    }
    const previous = input.value.trim();
    clear(options);
    for (const stage of stages) {
      const option = panel.ownerDocument.createElement("option");
      option.value = stage.stageId;
      option.label = t("pipeline.stage.selectLabel", {
        step: stageStepLabel(stage.stageId),
        title: stageDisplayTitle(stage),
      });
      markRuntimeWhen(option, !knownStageId(stage.stageId));
      options.append(option);
    }
    if (!previous && stages.length > 0) {
      input.value = role === "pipeline-to" ? stages[stages.length - 1].stageId : stages[0].stageId;
    }
  }
}

function renderStageList(container, model, rerender) {
  clear(container);
  if (model.view.stages.length === 0) {
    container.append(el("div", "empty-list", t("pipeline.empty.waitingView")));
    return;
  }
  let currentGroup = "";
  for (const stage of model.view.stages) {
    const group = groupForStage(stage.stageId);
    if (group !== currentGroup) {
      currentGroup = group;
      container.append(el("div", "step-group-label", currentGroup));
    }
    const button = el("button", `step-card status-${stage.status}`);
    button.type = "button";
    button.classList.toggle("active", stage.stageId === model.selectedStageId);
    button.dataset.stageId = stage.stageId;
    button.append(el("strong", "step-id", stageStepLabel(stage.stageId)));
    const title = el("span", "step-title", stageDisplayTitle(stage));
    markRuntimeWhen(title, !knownStageId(stage.stageId));
    button.append(title, protocolLabel("span", "step-status", "pipelineStatus", stage.status));
    button.addEventListener("click", () => {
      model.selectStage(stage.stageId);
      rerender();
    });
    container.append(button);
  }
}

function renderDetail(panel, model, api = {}) {
  const detail = panel.querySelector('[data-role="pipeline-detail"]');
  clear(detail);
  const stage = model.selectedStage();
  if (!stage) {
    detail.textContent = t("pipeline.detail.selectStage");
    return;
  }
  detail.append(
    markRuntimeWhen(
      el(
        "h2",
        "",
        t("pipeline.detail.title", {
          step: stageStepLabel(stage.stageId),
          title: stageDisplayTitle(stage),
        }),
      ),
      !knownStageId(stage.stageId),
    ),
  );
  detail.append(labeledProtocolLine("pipeline.detail.status", "pipelineStatus", stage.status));
  detail.append(el("div", "detail-line", t("pipeline.detail.engine")));
  detail.append(el("div", "detail-line", t("pipeline.detail.aiAdapter")));
  if (stage.message) {
    detail.append(labeledRuntimeLine("pipeline.detail.message", stage.message));
  }
  renderStageIssues(detail, stage);
  renderSemanticQuality(detail, stage.semanticQuality);
  const action = el(
    "button",
    "command-button",
    t(stage.status === "pending" ? "pipeline.action.runStage" : "pipeline.action.rerunStage"),
  );
  action.type = "button";
  action.dataset.action = "run-selected-stage";
  detail.append(action);
  renderStyleGrid(panel, model, stage, api);
}

function renderStageIssues(container, stage) {
  if (stage.errors.length === 0 && stage.warnings.length === 0) {
    return;
  }
  const section = el("section", "pipeline-evidence-section pipeline-issues");
  section.dataset.role = "pipeline-issues";
  section.append(
    el(
      "h3",
      "",
      t("pipeline.issues.summary", {
        errors: stage.errors.length,
        warnings: stage.warnings.length,
      }),
    ),
  );
  for (const issue of [...stage.errors, ...stage.warnings]) {
    const item = el("article", `pipeline-issue severity-${issue.severity}`);
    const heading = issue.code
      ? markRuntime(el("strong", "pipeline-issue-heading", issue.code))
      : protocolLabel("strong", "pipeline-issue-heading", "pipelineSeverity", issue.severity);
    item.append(heading, markRuntime(el("span", "pipeline-issue-message", issue.message)));
    section.append(item);
  }
  container.append(section);
}

function renderSemanticQuality(container, semanticQuality) {
  const quality = normalizeSemanticQuality(semanticQuality);
  const section = el("section", `semantic-quality-panel status-${quality.status}`);
  section.setAttribute("data-role", "semantic-quality-panel");
  section.append(el("h3", "", t("pipeline.semantic.title")));
  const metricLine = [
    t("pipeline.semantic.status", { status: enumLabel("pipelineStatus", quality.status) }),
    t("pipeline.semantic.projectSpecificity", {
      value: metricText(quality.projectSpecificityScore),
    }),
    t("pipeline.semantic.requiredCoverage", {
      value: metricText(quality.requiredSemanticCoverage),
    }),
    t("pipeline.semantic.genericTemplate", { value: metricText(quality.genericTemplateRatio) }),
    t("pipeline.semantic.placeholder", { value: metricText(quality.placeholderRatio) }),
  ].join(t("pipeline.separator"));
  section.append(el("div", "semantic-metrics", metricLine));
  if (quality.returnTargets.length === 0) {
    section.append(el("div", "empty-inline", t("pipeline.semantic.waiting")));
  }
  for (const target of quality.returnTargets.slice(0, 6)) {
    const issue = el("div", `semantic-issue severity-${target.severity}`);
    issue.append(markRuntime(el("strong", "", target.code)));
    issue.append(
      target.message
        ? markRuntime(el("span", "", target.message))
        : el("span", "", t("pipeline.semantic.viewReport")),
    );
    const returnTarget = localizedReturnTarget(target.code);
    issue.append(
      returnTarget
        ? el("em", "", returnTarget)
        : markRuntime(el("em", "", target.returnTarget)),
    );
    section.append(issue);
  }
  container.append(section);
}

function renderStyleGrid(panel, model, stage, api = {}) {
  const grid = panel.querySelector('[data-role="style-grid"]');
  revokeStylePreviewObjectUrls(grid);
  clear(grid);
  const shouldShow = stage?.isStep07 || model.view.waitingConfirmation || model.view.styleOptions.length > 0;
  grid.hidden = !shouldShow;
  if (!shouldShow) {
    return;
  }
  const confirmed = model.view.state.status === "style_confirmed";
  grid.append(
    el("h3", "", t(confirmed ? "pipeline.style.confirmedTitle" : "pipeline.style.title")),
  );
  if (confirmed) {
    grid.append(el("div", "style-confirmed", t("pipeline.style.confirmedDescription")));
  }
  const options = model.view.styleOptions;
  if (options.length === 0) {
    grid.append(el("div", "empty-inline", t("pipeline.style.waitingOptions")));
  }
  const optionWrap = el("div", "style-option-grid");
  for (const option of options) {
    const card = el("label", "style-option-card");
    card.classList.toggle("selected", option.selected);
    const radio = document.createElement("input");
    radio.type = "radio";
    radio.name = "style-option";
    radio.value = option.optionId;
    radio.checked = option.selected;
    card.append(radio);
    card.append(markRuntime(el("strong", "", option.title)));
    card.append(
      option.description
        ? markRuntime(el("span", "", option.description))
        : el("span", "", t("pipeline.style.noDescription")),
    );
    if (option.imageStatus) {
      card.append(
        el(
          "span",
          `style-image-status status-${option.imageStatus}`,
          t(`pipeline.style.imageStatus.${option.imageStatus}`),
        ),
      );
    }
    const preview = el("div", "style-image-preview");
    const previewStatus = el(
      "span",
      "style-image-preview-status",
      option.imagePath
        ? t("pipeline.style.imageLoading")
        : t("pipeline.style.imageUnavailable"),
    );
    preview.append(previewStatus);
    card.append(preview);
    if (option.imagePath && api.readStylePreview) {
      const image = document.createElement("img");
      image.className = "style-image-preview-image";
      image.alt = option.title || t("pipeline.style.imageAlt");
      image.hidden = true;
      const token = `preview-${++stylePreviewToken}`;
      preview.dataset.previewToken = token;
      preview.append(image);
      image.onload = () => {
        if (preview.dataset.previewToken !== token) {
          return;
        }
        if (image.naturalWidth <= 1 || image.naturalHeight <= 1) {
          revokeStylePreviewObjectUrl(grid, image.src);
          image.hidden = true;
          previewStatus.hidden = false;
          previewStatus.textContent = t("pipeline.style.legacyPlaceholder");
          return;
        }
        image.hidden = false;
        previewStatus.hidden = true;
        revokeStylePreviewObjectUrl(grid, image.src);
      };
      image.onerror = () => {
        revokeStylePreviewObjectUrl(grid, image.src);
        if (preview.dataset.previewToken === token) {
          image.hidden = true;
          previewStatus.hidden = false;
          previewStatus.textContent = t("pipeline.style.imageUnavailable");
        }
      };
      Promise.resolve(
        api.readStylePreview(buildReadStep07PreviewRequest(option.imagePath, 8 * 1024 * 1024)),
      )
        .then((result) => {
          if (!preview.isConnected || preview.dataset.previewToken !== token) {
            return;
          }
          const objectUrl = createPipelineImageObjectUrl(result);
          if (!objectUrl) {
            previewStatus.textContent = t("pipeline.style.imageUnavailable");
            return;
          }
          rememberStylePreviewObjectUrl(grid, objectUrl);
          image.src = objectUrl;
        })
        .catch(() => {
          if (preview.isConnected && preview.dataset.previewToken === token) {
            previewStatus.textContent = t("pipeline.style.imageUnavailable");
          }
        });
    }
    optionWrap.append(card);
  }
  grid.append(optionWrap);
  const notes = document.createElement("textarea");
  notes.className = "text-area style-notes";
  notes.rows = 3;
  notes.placeholder = t("pipeline.style.notesPlaceholder");
  notes.dataset.role = "style-notes";
  grid.append(notes);
  const row = el("div", "style-action-row");
  const confirm = el("button", "command-button", t("pipeline.style.confirm"));
  confirm.type = "button";
  confirm.dataset.action = "confirm-style";
  const regenerate = el("button", "command-button secondary", t("pipeline.style.regenerate"));
  regenerate.type = "button";
  regenerate.dataset.action = "regenerate-style";
  const promptEditor = el("button", "command-button secondary", t("pipeline.style.editPrompt"));
  promptEditor.type = "button";
  promptEditor.dataset.action = "open-style-prompt-editor";
  row.append(confirm, regenerate, promptEditor);
  grid.append(row);
}

function renderRuntimeLog(panel, model) {
  const output = panel.querySelector('[data-role="pipeline-runtime-log"]');
  if (output) {
    markRuntime(output);
    output.textContent = model.runtimeLines().join("\n");
  }
}

function bindPipelineActions(panel, model, api) {
  bindOnce(panel, "run-pipeline", async () => {
    const from = panel.querySelector('[data-role="pipeline-from"]').value;
    const to = panel.querySelector('[data-role="pipeline-to"]').value;
    const skipManualGates = panel.querySelector('[data-role="pipeline-skip-gate"]')?.checked ?? false;
    await runPipelineOperation(
      panel,
      () => api.runRange(buildRunPipelineRequest(from, to, { skipManualGates })),
      api,
    );
  });
  bindOnce(panel, "stop-pipeline", async () => {
    await runPipelineOperation(panel, () => api.stop(), api);
  });
  bindOnce(panel, "resume-pipeline", async () => {
    if (!model.view.recovery || !api.resume) {
      setPipelineStatus(panel, t("pipeline.operation.resumeUnavailable"));
      return;
    }
    await runPipelineOperation(
      panel,
      () => api.resume(buildResumePipelineRequest(model.view.recovery)),
      api,
    );
  });
  bindOnce(panel, "confirm-style", async () => {
    const selected = panel.querySelector('input[name="style-option"]:checked')?.value ?? "";
    const notes = panel.querySelector('[data-role="style-notes"]')?.value ?? "";
    await runPipelineOperation(
      panel,
      () => api.confirmStyle(buildConfirmStyleRequest("07", selected, notes)),
      api,
    );
  });
  bindOnce(panel, "regenerate-style", async () => {
    const skipManualGates = panel.querySelector('[data-role="pipeline-skip-gate"]')?.checked ?? false;
    await runPipelineOperation(
      panel,
      () => api.runRange(buildRunPipelineRequest("07", "07", { skipManualGates })),
      api,
    );
  });
  bindOnce(panel, "export-to-pipeline", async () => {
    if (!api.exportToPipeline) {
      setPipelineStatus(panel, t("pipeline.operation.exportUnavailable"));
      return;
    }
    await runPipelineOperation(panel, () => api.exportToPipeline(), api);
  });
  bindOnce(panel, "open-style-prompt-editor", () => {
    panel.ownerDocument.dispatchEvent(
      new CustomEvent("adm:open-style-prompt-editor", {
        detail: { stageId: "07", styleOptions: model.view.styleOptions },
      }),
    );
  });
  bindOnce(panel, "run-selected-stage", async () => {
    const stage = model.selectedStage();
    const skipManualGates = panel.querySelector('[data-role="pipeline-skip-gate"]')?.checked ?? false;
    await runPipelineOperation(panel, () => api.runRange(buildRunPipelineRequest(stage.stageId, stage.stageId, { skipManualGates })), api);
  });
}

async function runPipelineOperation(panel, operation, api = {}) {
  setPipelineStatus(panel, t("pipeline.operation.running"));
  setPipelineRunning(panel, true);
  try {
    const result = await operation();
    const canonicalFrom = read(result?.report, "fromStageId", "from_stage_id") ?? "";
    const canonicalTo = read(result?.report, "toStageId", "to_stage_id") ?? "";
    let view = result?.view ?? null;
    if (!view && api.load) {
      view = await api.load();
    }
    view ??= result;
    if (view) {
      if (api.applyView) {
        api.applyView(view);
      } else {
        renderPipelinePanel(panel.ownerDocument, view, api);
      }
    } else {
      setPipelineRunning(panel, false);
      setPipelineStatus(panel, t("pipeline.operation.missingView"));
    }
    if (canonicalFrom) {
      panel.querySelector('[data-role="pipeline-from"]')?.setAttribute("value", canonicalFrom);
      const input = panel.querySelector('[data-role="pipeline-from"]');
      if (input) input.value = canonicalFrom;
    }
    if (canonicalTo) {
      panel.querySelector('[data-role="pipeline-to"]')?.setAttribute("value", canonicalTo);
      const input = panel.querySelector('[data-role="pipeline-to"]');
      if (input) input.value = canonicalTo;
    }
  } catch (error) {
    setPipelineRunning(panel, false);
    setPipelineStatus(panel, t("pipeline.operation.failed", { error: error.message }));
  }
}

function setPipelineRunning(panel, running) {
  for (const action of [
    "run-pipeline",
    "run-selected-stage",
    "confirm-style",
    "regenerate-style",
    "export-to-pipeline",
    "resume-pipeline",
  ]) {
    const button = panel.querySelector(`[data-action="${action}"]`);
    if (button) {
      button.disabled = running;
    }
  }
  const stop = panel.querySelector('[data-action="stop-pipeline"]');
  if (stop) {
    stop.disabled = !running;
  }
}

function setPipelineRecovery(panel, view) {
  const recoverable = ["recoverable", "recovery_blocked"].includes(view.state.status)
    && Boolean(view.recovery);
  const resume = panel.querySelector('[data-action="resume-pipeline"]');
  if (resume) {
    resume.hidden = !recoverable;
    resume.disabled = view.running || !recoverable;
  }
  const run = panel.querySelector('[data-action="run-pipeline"]');
  if (run && recoverable) {
    run.disabled = true;
  }
}

function setPipelineStatus(panel, message) {
  const output = panel.querySelector('[data-role="pipeline-runtime-log"]');
  if (output) {
    output.textContent = message;
  }
}

function normalizePipelineState(input) {
  const runtimeMap = read(input, "stages") ?? {};
  return {
    runId: read(input, "runId", "run_id") ?? "",
    attemptId: read(input, "attemptId", "attempt_id") ?? "",
    parentAttemptId: read(input, "parentAttemptId", "parent_attempt_id") ?? null,
    attemptNo: Number(read(input, "attemptNo", "attempt_no") ?? 0),
    stateVersion: Number(read(input, "stateVersion", "state_version") ?? 0),
    status: read(input, "status") ?? "idle",
    stopRequested: Boolean(read(input, "stopRequested", "stop_requested")),
    fromStageId: read(input, "fromStageId", "from_stage_id") ?? "",
    toStageId: read(input, "toStageId", "to_stage_id") ?? "",
    stageIds: asArray(read(input, "stageIds", "stage_ids")),
    currentStageId: read(input, "currentStageId", "current_stage_id") ?? null,
    currentUnitId: read(input, "currentUnitId", "current_unit_id") ?? null,
    stages: Object.fromEntries(
      Object.entries(runtimeMap).map(([stageId, runtimeInput]) => {
        const runtime = runtimeInput && typeof runtimeInput === "object" ? runtimeInput : {};
        const result = read(runtime, "result") ?? {};
        return [
          stageId,
          {
            status: read(runtime, "status") ?? "pending",
            result: {
              message: read(result, "message") ?? "",
              errors: read(result, "errors") ?? [],
              warnings: read(result, "warnings") ?? [],
            },
          },
        ];
      }),
    ),
  };
}

function normalizePipelineRecovery(input) {
  if (!input) {
    return null;
  }
  const runId = read(input, "runId", "run_id") ?? "";
  const revision = Number(read(input, "revision") ?? 0);
  if (!runId || !Number.isSafeInteger(revision) || revision <= 0) {
    return null;
  }
  return {
    runId,
    attemptId: read(input, "attemptId", "attempt_id") ?? "",
    revision,
    status: read(input, "status") ?? "recoverable",
    fromStageId: read(input, "fromStageId", "from_stage_id") ?? "",
    toStageId: read(input, "toStageId", "to_stage_id") ?? "",
    currentStageId: read(input, "currentStageId", "current_stage_id") ?? null,
    nextUnitId: read(input, "nextUnitId", "next_unit_id") ?? null,
    updatedAt: read(input, "updatedAt", "updated_at") ?? "",
  };
}

function normalizeStageView(stage) {
  const result = read(stage, "result") ?? {};
  return {
    stageId: read(stage, "stageId", "stage_id") ?? "",
    title: read(stage, "title") ?? "",
    kind: read(stage, "kind") ?? "development",
    status: read(stage, "status") ?? "pending",
    message: read(stage, "message") ?? read(result, "message") ?? "",
    isStep07: Boolean(read(stage, "isStep07", "is_step07")),
    semanticQuality: normalizeSemanticQuality(read(stage, "semanticQuality", "semantic_quality")),
    errors: normalizePipelineIssues(
      read(stage, "errors") ?? read(result, "errors"),
      "error",
    ),
    warnings: normalizePipelineIssues(
      read(stage, "warnings") ?? read(result, "warnings"),
      "warning",
    ),
  };
}

function normalizeStyleOption(option) {
  const optionId = read(option, "optionId", "option_id") ?? read(option, "styleId", "style_id") ?? "";
  return {
    optionId,
    styleId: read(option, "styleId", "style_id") ?? optionId,
    title: read(option, "title") ?? "",
    description: read(option, "description") ?? "",
    imagePath: read(option, "imagePath", "image_path") ?? "",
    imageStatus: read(option, "imageStatus", "image_status") ?? "",
    imageMessage: read(option, "imageMessage", "image_message") ?? "",
    prompt: read(option, "prompt") ?? "",
    promptRefined: Boolean(read(option, "promptRefined", "prompt_refined")),
    selected: Boolean(read(option, "selected")),
  };
}

export function normalizePipelineIssues(input, fallbackSeverity = "warning") {
  return listFromUnknown(input).map((issue) => {
    if (typeof issue === "string") {
      return { severity: fallbackSeverity, code: "", message: issue, detail: "" };
    }
    const value = issue && typeof issue === "object" ? issue : {};
    const message =
      read(value, "message") ??
      read(value, "error") ??
      read(value, "reason") ??
      read(value, "summary") ??
      structuredText(value);
    const detail = read(value, "detail") ?? read(value, "details") ?? read(value, "trace") ?? "";
    return {
      severity: String(read(value, "severity") ?? read(value, "level") ?? fallbackSeverity).toLowerCase(),
      code: String(read(value, "code") ?? read(value, "id") ?? ""),
      message: structuredText(message),
      detail: structuredText(detail),
    };
  });
}

export function normalizeSemanticQuality(input) {
  const value = input && typeof input === "object" ? input : {};
  return {
    status: read(value, "status") ?? "missing",
    projectSpecificityScore: ratioOrNull(read(value, "projectSpecificityScore", "project_specificity_score")),
    requiredSemanticCoverage: ratioOrNull(read(value, "requiredSemanticCoverage", "required_semantic_coverage")),
    genericTemplateRatio: ratioOrNull(read(value, "genericTemplateRatio", "generic_template_ratio")),
    placeholderRatio: ratioOrNull(read(value, "placeholderRatio", "placeholder_ratio")),
    returnTargets: asArray(read(value, "returnTargets", "return_targets")).map(normalizeReturnTarget),
  };
}

function normalizeReturnTarget(input) {
  const code = read(input, "code") ?? "UNCLASSIFIED_ISSUE";
  return {
    severity: read(input, "severity") ?? "warning",
    code,
    message: read(input, "message") ?? read(input, "messageZh", "message_zh") ?? "",
    returnTarget: read(input, "returnTarget", "return_target") ?? returnTargetForCode(code),
  };
}

function stageFromState(stageId, state) {
  const runtime = state.stages[stageId] ?? {};
  const result = runtime.result ?? {};
  return {
    stageId,
    title: `Step ${stageId}`,
    kind: stageId === "07" ? "human_gate" : "development",
    status: read(runtime, "status") ?? "pending",
    message: read(result, "message") ?? "",
    isStep07: stageId === "07",
    semanticQuality: normalizeSemanticQuality(null),
    errors: normalizePipelineIssues(read(result, "errors"), "error"),
    warnings: normalizePipelineIssues(read(result, "warnings"), "warning"),
  };
}

function groupForStage(stageId) {
  const number = Number(stageId);
  if (number <= 2) return t("pipeline.group.designHandoff");
  if (number <= 7) return t("pipeline.group.requirementsReview");
  if (number <= 10) return t("pipeline.group.planningAlignment");
  return t("pipeline.group.executionValidation");
}

function unwrapCommandResponse(response) {
  if (response && typeof response.ok === "boolean") {
    if (response.ok) {
      return response.data ?? null;
    }
    const detail = response.error?.message ?? response.error?.code ?? t("pipeline.error.commandFailed");
    throw new Error(detail);
  }
  return response ?? null;
}

function bindOnce(panel, action, handler) {
  const button = panel.querySelector(`[data-action="${action}"]`);
  if (button) {
    button.onclick = handler;
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

function listFromUnknown(value) {
  if (value === null || value === undefined || value === "") {
    return [];
  }
  if (Array.isArray(value)) {
    return value;
  }
  if (typeof value !== "object") {
    return [value];
  }
  const descriptorKeys = [
    "relativePath",
    "relative_path",
    "path",
    "file",
    "content",
    "message",
    "error",
    "reason",
    "severity",
    "code",
  ];
  if (descriptorKeys.some((key) => Object.hasOwn(value, key))) {
    return [value];
  }
  return Object.entries(value).map(([key, item]) => {
    if (item && typeof item === "object" && !Array.isArray(item)) {
      return { label: key, ...item };
    }
    return item;
  });
}

function structuredText(value) {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function finiteNumberOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function rememberStylePreviewObjectUrl(grid, objectUrl) {
  const urls = stylePreviewObjectUrls.get(grid) ?? new Set();
  urls.add(objectUrl);
  stylePreviewObjectUrls.set(grid, urls);
}

function revokeStylePreviewObjectUrl(grid, objectUrl) {
  if (!objectUrl?.startsWith?.("blob:")) {
    return;
  }
  globalThis.URL?.revokeObjectURL?.(objectUrl);
  const urls = stylePreviewObjectUrls.get(grid);
  urls?.delete(objectUrl);
  if (urls?.size === 0) {
    stylePreviewObjectUrls.delete(grid);
  }
}

function revokeStylePreviewObjectUrls(grid) {
  const urls = stylePreviewObjectUrls.get(grid);
  if (!urls) {
    return;
  }
  for (const objectUrl of urls) {
    globalThis.URL?.revokeObjectURL?.(objectUrl);
  }
  stylePreviewObjectUrls.delete(grid);
}

function issueText(issue) {
  return [issue.code, issue.message].filter(Boolean).join(": ") || issue.severity;
}

function ratioOrNull(value) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return null;
  }
  return number > 1 ? number / 100 : number;
}

function metricText(value) {
  return value === null || value === undefined
    ? t("pipeline.semantic.notGenerated")
    : `${Math.round(value * 100)}%`;
}

function returnTargetForCode(code) {
  return {
    PROGRAM_CAPABILITY_NOT_BOUND: "Step03 program capability contract",
    CORE_ENTITY_WITHOUT_ASSET_STRATEGY: "Step04 art asset strategy",
    STYLE_ARCHETYPE_MISMATCH: "Step07 style confirmation",
    STYLE_OVERRIDE_REASON_MISSING: "Step07 style confirmation",
    GENERIC_PLAN_DOMINANCE: "Step08, then Step02/03 if needed",
    PROGRAM_TASK_WITHOUT_PROJECT_REF: "Step08, then Step02/03 if needed",
    CORE_ASSET_NOT_PRODUCED: "Step09 or Step04",
    PLACEHOLDER_ONLY_CORE_ASSET: "Step09 or Step04",
    SEMANTIC_ALIGNMENT_GAP: "Step10, then the source stage for the gap",
    PLACEHOLDER_ONLY_ALIGNMENT: "Step10, then the source stage for the gap",
    PLACEHOLDER_TOKEN_REMAINS: "Source stage report, usually Step03/04/09",
    SOURCE_TRACE_MISSING: "Source stage source_refs",
    L5_ENTITY_COVERAGE_BELOW_TARGET: "Step02 L5 entity-node mapping",
    INVALID_L5_ENTITY: "Step02 L5 entity definitions",
    ENTITY_GRAPH_CYCLE: "Step02 entity dependency graph",
    PLAYABLE_CONTRACT_INCOMPLETE: "Step02 playable contracts",
  }[code] ?? t("pipeline.returnTarget.default");
}

const PIPELINE_STAGE_IDS = new Set(
  Array.from({ length: 15 }, (_, index) => String(index).padStart(2, "0")),
);

const LOCALIZED_RETURN_TARGET_CODES = new Set([
  "PROGRAM_CAPABILITY_NOT_BOUND",
  "CORE_ENTITY_WITHOUT_ASSET_STRATEGY",
  "STYLE_ARCHETYPE_MISMATCH",
  "STYLE_OVERRIDE_REASON_MISSING",
  "GENERIC_PLAN_DOMINANCE",
  "PROGRAM_TASK_WITHOUT_PROJECT_REF",
  "CORE_ASSET_NOT_PRODUCED",
  "PLACEHOLDER_ONLY_CORE_ASSET",
  "SEMANTIC_ALIGNMENT_GAP",
  "PLACEHOLDER_ONLY_ALIGNMENT",
  "PLACEHOLDER_TOKEN_REMAINS",
  "SOURCE_TRACE_MISSING",
  "L5_ENTITY_COVERAGE_BELOW_TARGET",
  "INVALID_L5_ENTITY",
  "ENTITY_GRAPH_CYCLE",
  "PLAYABLE_CONTRACT_INCOMPLETE",
]);

function knownStageId(stageId) {
  return PIPELINE_STAGE_IDS.has(String(stageId ?? ""));
}

function stageStepLabel(stageId) {
  return t("pipeline.stage.step", { id: stageId });
}

function stageDisplayTitle(stage) {
  if (knownStageId(stage?.stageId)) {
    return t(`pipeline.stage.${stage.stageId}.title`);
  }
  return stage?.title || t("pipeline.stage.unknownTitle");
}

function localizedReturnTarget(code) {
  const normalized = String(code ?? "");
  return LOCALIZED_RETURN_TARGET_CODES.has(normalized)
    ? t(`pipeline.returnTarget.${normalized}`)
    : "";
}

function protocolLabel(tag, className, group, value) {
  const raw = String(value ?? "");
  const translated = enumLabel(group, raw);
  return markRuntimeWhen(el(tag, className, translated), translated === raw && raw !== "");
}

function labeledProtocolLine(labelKey, group, value) {
  const line = el("div", "detail-line");
  line.append(`${t(labelKey)} `, protocolLabel("span", "", group, value));
  return line;
}

function labeledRuntimeLine(labelKey, value) {
  const line = el("div", "detail-line");
  line.append(`${t(labelKey)} `, markRuntime(el("span", "", value)));
  return line;
}

function markRuntime(element) {
  if (element) {
    element.dataset.contentOrigin = "runtime";
  }
  return element;
}

function markRuntimeWhen(element, condition) {
  return condition ? markRuntime(element) : element;
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
