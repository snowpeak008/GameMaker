import { enumLabel, t } from "../i18n.js";
import { clear, el } from "../shared/dom.js";
import { asArray, read } from "../shared/value.js";

export const DEFAULT_AI_INTERVIEW_STATE = {
  schemaVersion: "1.0",
  status: "idle",
  backendStage: "idle",
  currentQuestionText: "",
  awaitingUserAnswer: false,
  messages: [],
  lastError: "",
  autoArchivePath: "",
  lastManualArchivePath: "",
  lastArchivedAt: "",
  streamEvents: [],
  backgroundJobs: {
    mappingStatus: "idle",
    summaryCorrectionStatus: "idle",
    activeJobCount: 0,
  },
};

export function createAiInterviewApi(invokeCommand) {
  return {
    async load() {
      return unwrapCommandResponse(await invokeCommand("load_ai_interview"));
    },
    async submitTurn(request) {
      return unwrapCommandResponse(await invokeCommand("submit_ai_turn", { request }));
    },
    async forceOutput(request) {
      return unwrapCommandResponse(await invokeCommand("force_ai_output", { request }));
    },
    async markInaccurate(request) {
      return unwrapCommandResponse(await invokeCommand("mark_ai_inaccurate", { request }));
    },
    async saveArchive(request) {
      return unwrapCommandResponse(await invokeCommand("save_ai_archive", { request }));
    },
  };
}

export function normalizeAiInterviewState(input) {
  const state = read(input, "state") ?? input ?? DEFAULT_AI_INTERVIEW_STATE;
  return {
    schemaVersion: read(state, "schemaVersion", "schema_version") ?? "1.0",
    status: read(state, "status") ?? "idle",
    backendStage: read(state, "backendStage", "backend_stage") ?? "idle",
    currentQuestionText: read(state, "currentQuestionText", "current_question_text") ?? "",
    awaitingUserAnswer: Boolean(read(state, "awaitingUserAnswer", "awaiting_user_answer")),
    messages: asArray(read(state, "messages")).map(normalizeMessage),
    lastError: read(state, "lastError", "last_error") ?? "",
    autoArchivePath: read(state, "autoArchivePath", "auto_archive_path") ?? "",
    lastManualArchivePath: read(state, "lastManualArchivePath", "last_manual_archive_path") ?? "",
    lastArchivedAt: read(state, "lastArchivedAt", "last_archived_at") ?? "",
    sessionTurnCount: Number(read(state, "sessionTurnCount", "session_turn_count") ?? 0),
    routeOverview: read(state, "routeOverview", "route_overview") ?? {},
    summary: read(state, "summary") ?? {},
    inferences: asArray(read(state, "inferences")),
    streamEvents: normalizeAiStreamEvents(
      read(input, "streamEvents", "stream_events") ?? read(state, "streamEvents", "stream_events"),
    ),
    backgroundJobs: normalizeAiBackgroundJobs(
      read(input, "backgroundJobs", "background_jobs") ?? read(state, "backgroundJobs", "background_jobs"),
    ),
  };
}

export function createAiInterviewModel(stateInput) {
  const state = normalizeAiInterviewState(stateInput);
  const running = ["running", "queued"].includes(state.status) || ["running", "queued"].includes(state.backendStage);
  return {
    state,
    running,
    actionsDisabled: running,
    currentQuestion: state.currentQuestionText || t("settings.aiInterview.waitingQuestion"),
    inputHint: t(
      state.awaitingUserAnswer
        ? "settings.aiInterview.inputHint.answer"
        : "settings.aiInterview.inputHint.supplement",
    ),
    statusText: statusText(state, running),
    archiveText: archiveText(state),
    streamText: streamText(state.streamEvents),
    mappingStatus: state.backgroundJobs.mappingStatus,
    summaryCorrectionStatus: state.backgroundJobs.summaryCorrectionStatus,
    routeText: routeText(state.routeOverview),
  };
}

export class AiInterviewController {
  constructor(api = {}, stateInput = DEFAULT_AI_INTERVIEW_STATE) {
    this.api = api;
    this.state = normalizeAiInterviewState(stateInput);
  }

  get model() {
    return createAiInterviewModel(this.state);
  }

  applyCommandResult(result) {
    this.state = normalizeAiInterviewState(result?.state ? result : (result ?? this.state));
    return this.state;
  }

  async submitTurn(userMessage, payloadJson = null) {
    const result = await this.api.submitTurn(buildSubmitAiTurnRequest(userMessage, payloadJson));
    return this.applyCommandResult(result);
  }

  async forceOutput(payload = null) {
    const result = await this.api.forceOutput(buildForceAiOutputRequest(payload));
    return this.applyCommandResult(result);
  }

  async markInaccurate(nodeId, reason) {
    const result = await this.api.markInaccurate(buildMarkAiInaccurateRequest(nodeId, reason));
    return this.applyCommandResult(result);
  }

  async saveArchive(archivePath = null) {
    const result = await this.api.saveArchive(buildSaveAiArchiveRequest(archivePath));
    return this.applyCommandResult(result);
  }
}

export function createAiInterviewController(stateInput, api = {}) {
  return new AiInterviewController(api, stateInput);
}

export function buildSubmitAiTurnRequest(userMessage, payloadJson = null) {
  return {
    user_message: String(userMessage ?? ""),
    schema_mode: "turn",
    payload_json: payloadJson,
  };
}

export function buildForceAiOutputRequest(payload = null) {
  return {
    schema_mode: "full_output",
    payload,
  };
}

export function buildMarkAiInaccurateRequest(nodeId, reason) {
  return {
    node_id: String(nodeId ?? ""),
    reason: String(reason ?? ""),
  };
}

export function buildSaveAiArchiveRequest(archivePath = null) {
  return {
    archive_path: archivePath ? String(archivePath) : null,
  };
}

export async function initAiInterviewPanel(documentRef, api) {
  if (!documentRef) {
    return null;
  }
  let state;
  try {
    state = await api.load();
  } catch (error) {
    state = {
      ...DEFAULT_AI_INTERVIEW_STATE,
      status: "failed",
      backendStage: "failed",
      lastError: String(error?.message ?? error),
    };
  }
  return renderAiInterviewPanel(documentRef, state, api);
}

export function renderAiInterviewPanel(documentRef, stateInput, api = {}) {
  const panel = documentRef.querySelector('[data-role="ai-interview-panel"]');
  if (!panel) {
    return null;
  }
  const model = createAiInterviewModel(stateInput);
  markRuntime(panel.querySelector('[data-role="ai-answer-input"]'));
  markRuntime(panel.querySelector('[data-role="ai-node-id"]'));
  const question = panel.querySelector('[data-role="ai-current-question"]');
  question.value = model.currentQuestion;
  markRuntimeWhen(question, Boolean(model.state.currentQuestionText));
  panel.querySelector('[data-role="ai-input-hint"]').textContent = model.inputHint;
  const status = panel.querySelector('[data-role="ai-panel-status"]');
  status.textContent = model.statusText;
  markRuntimeWhen(status, statusHasRuntimeContent(model.state));
  const stream = panel.querySelector('[data-role="ai-stream-timeline"]');
  stream.textContent = model.streamText;
  markRuntime(stream);
  const background = panel.querySelector('[data-role="ai-background-status"]');
  background.textContent = t("settings.aiInterview.backgroundStatus", {
    mapping: enumLabel("aiJobStatus", model.mappingStatus),
    summary: enumLabel("aiJobStatus", model.summaryCorrectionStatus),
    route: model.routeText,
  });
  markRuntime(background);
  renderTranscript(panel.querySelector('[data-role="ai-transcript"]'), model.state.messages);
  setActionDisabled(panel, model.actionsDisabled);
  bindAiActions(panel, api);
  return model;
}

export function normalizeAiStreamEvents(input) {
  const events = asArray(input).map((event) => ({
    stage: read(event, "stage") ?? "idle",
    turnId: read(event, "turnId", "turn_id") ?? "",
    message: read(event, "message") ?? "",
    running: Boolean(read(event, "running")),
  }));
  if (events.length > 0) {
    return events;
  }
  return [{ stage: "idle", turnId: "", message: t("settings.aiInterview.stream.idle"), running: false }];
}

export function normalizeAiBackgroundJobs(input) {
  const jobs = input && typeof input === "object" ? input : {};
  return {
    mappingStatus: read(jobs, "mappingStatus", "mapping_status") ?? "idle",
    summaryCorrectionStatus: read(jobs, "summaryCorrectionStatus", "summary_correction_status") ?? "idle",
    activeJobCount: Number(read(jobs, "activeJobCount", "active_job_count") ?? 0),
  };
}

function renderTranscript(container, messages) {
  clear(container);
  if (messages.length === 0) {
    container.append(el("div", "empty-inline", t("settings.aiInterview.emptyTranscript")));
    return;
  }
  for (const message of messages) {
    const item = el("article", `ai-message ${message.role}`);
    item.append(
      markRuntimeWhen(
        el("header", "ai-message-role", message.roleLabel),
        message.roleLabel === message.role,
      ),
    );
    item.append(markRuntime(el("div", "ai-message-body", message.content)));
    container.append(item);
  }
}

function bindAiActions(panel, api) {
  const submit = async () => {
    const input = panel.querySelector('[data-role="ai-answer-input"]');
    const message = input.value.trim();
    if (!message) {
      setStatus(panel, t("settings.aiInterview.validation.answerRequired"));
      return;
    }
    await runAiAction(panel, () => api.submitTurn(buildSubmitAiTurnRequest(message)), api, input);
  };
  bindOnce(panel, "send-ai-turn", submit);
  const input = panel.querySelector('[data-role="ai-answer-input"]');
  if (input && !input.dataset.ctrlEnterBound) {
    input.dataset.ctrlEnterBound = "true";
    input.addEventListener("keydown", (event) => {
      if (event.ctrlKey && event.key === "Enter") {
        event.preventDefault();
        submit();
      }
    });
  }
  bindOnce(panel, "force-ai-output", async () => {
    await runAiAction(panel, () => api.forceOutput(buildForceAiOutputRequest()), api);
  });
  bindOnce(panel, "mark-ai-inaccurate", async () => {
    const nodeId = panel.querySelector('[data-role="ai-node-id"]').value.trim();
    const reason = panel.querySelector('[data-role="ai-answer-input"]').value.trim();
    if (!nodeId || !reason) {
      setStatus(panel, t("settings.aiInterview.validation.nodeAndReasonRequired"));
      return;
    }
    await runAiStateAction(panel, () => api.markInaccurate(buildMarkAiInaccurateRequest(nodeId, reason)), api);
  });
  bindOnce(panel, "save-ai-archive", async () => {
    await runAiStateAction(panel, () => api.saveArchive(buildSaveAiArchiveRequest()), api);
  });
}

async function runAiAction(panel, operation, api = {}, input = null) {
  setRunning(panel, true);
  try {
    const result = await operation();
    renderAiInterviewPanel(panel.ownerDocument, result, api);
    if (input) {
      input.value = "";
    }
  } catch (error) {
    setRunning(panel, false);
    setStatus(panel, t("settings.aiInterview.actionFailed", { error: error.message }), true);
  }
}

async function runAiStateAction(panel, operation, api = {}) {
  setRunning(panel, true);
  try {
    const state = await operation();
    renderAiInterviewPanel(panel.ownerDocument, state, api);
  } catch (error) {
    setRunning(panel, false);
    setStatus(panel, t("settings.aiInterview.actionFailed", { error: error.message }), true);
  }
}

function setActionDisabled(panel, disabled) {
  for (const button of panel.querySelectorAll("[data-action]")) {
    button.disabled = disabled;
  }
}

function setRunning(panel, running) {
  setActionDisabled(panel, running);
  setStatus(
    panel,
    t(running ? "settings.aiInterview.running" : "settings.aiInterview.waitingStatus"),
  );
}

function setStatus(panel, message, runtime = false) {
  const status = panel.querySelector('[data-role="ai-panel-status"]');
  if (status) {
    status.textContent = message;
    markRuntimeWhen(status, runtime);
  }
}

function bindOnce(panel, action, handler) {
  const button = panel.querySelector(`[data-action="${action}"]`);
  if (!button || button.dataset.bound) {
    return;
  }
  button.dataset.bound = "true";
  button.addEventListener("click", handler);
}

function normalizeMessage(message) {
  const role = read(message, "role") ?? "system";
  return {
    role,
    roleLabel: enumLabel("aiRole", role),
    content: String(read(message, "content") ?? read(message, "text") ?? JSON.stringify(message)),
  };
}

function statusText(state, running) {
  if (running) {
    return t("settings.aiInterview.running");
  }
  if (state.lastError) {
    return t("settings.aiInterview.errorStatus", { error: state.lastError });
  }
  return t("settings.aiInterview.status", {
    status: enumLabel("aiInterviewStatus", state.status),
    stage: enumLabel("aiInterviewStage", state.backendStage),
    archive: archiveText(state),
  });
}

function streamText(events) {
  return normalizeAiStreamEvents(events)
    .map((event) => `${event.stage}${event.turnId ? `:${event.turnId}` : ""}${event.running ? ":running" : ""}`)
    .join(" | ");
}

function routeText(routeOverview) {
  const stage = read(routeOverview, "currentMdaStage", "current_mda_stage") ?? "aesthetics";
  const expected = asArray(read(routeOverview, "expectedDomains", "expected_domains"));
  return expected.length > 0 ? `${stage}:${expected.join(",")}` : stage;
}

function archiveText(state) {
  return (
    state.lastManualArchivePath ||
    state.autoArchivePath ||
    t("settings.aiInterview.notArchived")
  );
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

function statusHasRuntimeContent(state) {
  return Boolean(
    state.lastError ||
      state.lastManualArchivePath ||
      state.autoArchivePath ||
      enumLabel("aiInterviewStatus", state.status) === String(state.status ?? "") ||
      enumLabel("aiInterviewStage", state.backendStage) === String(state.backendStage ?? ""),
  );
}

function markRuntime(element) {
  if (element) {
    element.dataset.contentOrigin = "runtime";
  }
  return element;
}

function markRuntimeWhen(element, condition) {
  if (!condition && element) {
    delete element.dataset.contentOrigin;
    return element;
  }
  return condition ? markRuntime(element) : element;
}
