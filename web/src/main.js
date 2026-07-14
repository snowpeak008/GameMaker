import { createAiInterviewApi, initAiInterviewPanel } from "./features/ai-interview.js";
import { createAiConfigApi, initAiConfigDialog } from "./features/ai-config.js";
import {
  buildAutosaveDesignRequest,
  createDesignApi,
  initDesignWorkbench,
} from "./features/design.js";
import { createPipelineApi, initPipelinePanel } from "./features/pipeline.js";
import { createSettingsStyleApi, initSettingsStyleModals } from "./features/settings-style.js";
import {
  createUtilityPanelApis,
  createUtilityPanelsController,
} from "./features/utility-panels.js";
import {
  enumLabel,
  initializeLanguageMode,
  t,
} from "./i18n.js";
import { SHELL_THEME, applyThemeTokens, normalizeShellThemeTokens } from "./theme.js";

export const TASKS = [
  { id: "design", labelKey: "nav.design" },
  { id: "pipeline", labelKey: "nav.pipeline" },
  { id: "patch", labelKey: "nav.patch" },
  { id: "package", labelKey: "nav.package" },
  { id: "logs", labelKey: "nav.logs" },
  { id: "sdk", labelKey: "nav.sdk" },
];

export const DEFAULT_SHELL_STATE = {
  activeView: "design",
  uiLanguage: "zh-CN",
  aiStatus: {
    label: "not_configured",
    ok: false,
  },
  progress: {
    passed: 0,
    total: 15,
  },
  systemStatus: "ready",
  theme: [],
  window: {
    title: "AutoDesignMaker NEWrust",
    width: SHELL_THEME.defaultWindowWidth,
    height: SHELL_THEME.defaultWindowHeight,
    minWidth: SHELL_THEME.minWindowWidth,
    minHeight: SHELL_THEME.minWindowHeight,
    geometryFile: "settings/window_geometry.json",
    resizable: true,
  },
  startup: {
    validateDataIntegrity: true,
    autoRestoreCurrentSave: false,
    releaseLockAtExit: true,
    pruneDraftsKeepCount: 0,
    startupStatus: "ready",
    startupStatusColor: "#C8D3DF",
  },
};

export function createShellModel(initialRoute = DEFAULT_SHELL_STATE.activeView) {
  let activeRoute = taskExists(initialRoute) ? initialRoute : DEFAULT_SHELL_STATE.activeView;
  return {
    get activeRoute() {
      return activeRoute;
    },
    switchRoute(route) {
      if (!taskExists(route)) {
        throw new Error(t("shell.error.unknownRoute", { route }));
      }
      activeRoute = route;
      return activeRoute;
    },
  };
}

export function formatProgress(progress = DEFAULT_SHELL_STATE.progress) {
  return t("shell.progress", progress);
}

export async function invokeCommand(command, payload = {}) {
  const tauriInvoke = globalThis.__TAURI__?.core?.invoke;
  if (!tauriInvoke) {
    throw new Error(t("shell.error.commandUnavailable", { command }));
  }
  return tauriInvoke(command, payload);
}

export async function getShellState(options = {}) {
  try {
    return unwrapCommandResponse(await invokeCommand("get_shell_state"));
  } catch (error) {
    if (options.fallback !== false) {
      return DEFAULT_SHELL_STATE;
    }
    throw error;
  }
}

export function unwrapCommandResponse(response) {
  if (response && typeof response.ok === "boolean") {
    if (response.ok) {
      return response.data ?? null;
    }
    const detail = response.error?.message ?? response.error?.code ?? t("shell.error.commandFailed");
    throw new Error(detail);
  }
  return response ?? null;
}

export function applyRoute(documentRef, route) {
  const app = documentRef.querySelector("#app");
  if (!app || !taskExists(route)) {
    return;
  }
  app.dataset.route = route;
  for (const tab of documentRef.querySelectorAll(".task-tab")) {
    const active = tab.dataset.route === route;
    tab.classList.toggle("active", active);
    tab.setAttribute("aria-pressed", String(active));
  }
  for (const panel of documentRef.querySelectorAll(".task-panel")) {
    const active = panel.dataset.panel === route;
    panel.classList.toggle("active", active);
    panel.hidden = !active;
  }
}

export function applyShellState(documentRef, shellState) {
  applyThemeTokens(documentRef, normalizeShellThemeTokens(shellState.theme));
  const ai = documentRef.querySelector(".ai-status");
  const progress = documentRef.querySelector(".progress-status");
  const system = documentRef.querySelector(".system-status");
  if (ai) {
    ai.textContent = formatAiStatus(shellState.aiStatus?.label);
    ai.dataset.status = shellState.aiStatus?.ok ? "ready" : "error";
  }
  if (progress) {
    progress.textContent = formatProgress(shellState.progress);
  }
  if (system) {
    system.textContent = formatSystemStatus(shellState.systemStatus);
  }
}

export async function listenForShutdownErrors(onError) {
  const listen = globalThis.__TAURI__?.event?.listen;
  if (!listen || typeof onError !== "function") {
    return null;
  }
  return listen("adm-shutdown-error", (event) => {
    onError(String(event?.payload ?? t("shell.error.unknownShutdownFailure")));
  });
}

export function createShellRefreshScheduler(documentRef, refresh, options = {}) {
  const windowRef = documentRef?.defaultView ?? globalThis;
  const setTimer = options.setTimeout ?? windowRef?.setTimeout?.bind(windowRef) ?? globalThis.setTimeout;
  const clearTimer =
    options.clearTimeout ?? windowRef?.clearTimeout?.bind(windowRef) ?? globalThis.clearTimeout;
  const intervalMs = Math.max(100, Number(options.intervalMs ?? 2500));
  let timer = null;
  let running = false;
  let stopped = true;

  const clearScheduled = () => {
    if (timer !== null) {
      clearTimer(timer);
      timer = null;
    }
  };
  const schedule = () => {
    if (stopped || running || documentRef?.hidden) {
      return;
    }
    clearScheduled();
    timer = setTimer(() => {
      timer = null;
      void run();
    }, intervalMs);
  };
  const run = async () => {
    if (stopped || running || documentRef?.hidden) {
      return;
    }
    running = true;
    try {
      await refresh();
    } catch (error) {
      options.onError?.(error);
    } finally {
      running = false;
      schedule();
    }
  };
  const onVisibilityChange = () => {
    if (documentRef?.hidden) {
      clearScheduled();
    } else if (!stopped && !running) {
      clearScheduled();
      void run();
    }
  };

  documentRef?.addEventListener?.("visibilitychange", onVisibilityChange);
  return {
    start() {
      if (!stopped) {
        return;
      }
      stopped = false;
      if (options.immediate === false) {
        schedule();
      } else {
        void run();
      }
    },
    stop() {
      stopped = true;
      clearScheduled();
      documentRef?.removeEventListener?.("visibilitychange", onVisibilityChange);
    },
    refreshNow() {
      clearScheduled();
      return run();
    },
  };
}

export async function initApp(documentRef = globalThis.document) {
  if (!documentRef) {
    return null;
  }
  const initialShellState = await getShellState({ fallback: true });
  initializeLanguageMode(documentRef, initialShellState?.uiLanguage);
  const model = createShellModel();
  let utilityApis = null;
  let utilityPanelsController = null;
  for (const tab of documentRef.querySelectorAll(".task-tab")) {
    tab.addEventListener("click", () => {
      const route = model.switchRoute(tab.dataset.route);
      applyRoute(documentRef, route);
      if (["patch", "package", "logs", "sdk"].includes(route) && utilityPanelsController) {
        void utilityPanelsController.refresh(route);
      }
    });
  }
  applyRoute(documentRef, model.activeRoute);
  let lastShellState = initialShellState ?? DEFAULT_SHELL_STATE;
  let shutdownPersistenceError = "";
  const refreshShellState = async () => {
    try {
      const state = await getShellState({
        fallback: !globalThis.__TAURI__?.core?.invoke,
      });
      lastShellState = state;
      applyShellState(
        documentRef,
        shutdownPersistenceError
          ? {
              ...state,
              systemStatus: t("shell.shutdownSaveFailed", {
                error: shutdownPersistenceError,
              }),
            }
          : state,
      );
      return state;
    } catch (error) {
      const failed = {
        ...lastShellState,
        systemStatus: t("shell.stateReadFailed", { error: error.message }),
      };
      applyShellState(documentRef, failed);
      return failed;
    }
  };
  applyShellState(documentRef, lastShellState);
  void listenForShutdownErrors((error) => {
    shutdownPersistenceError = error;
    applyShellState(documentRef, {
      ...lastShellState,
      systemStatus: t("shell.shutdownSaveFailed", { error }),
    });
  });
  const shellRefreshScheduler = createShellRefreshScheduler(documentRef, refreshShellState);
  shellRefreshScheduler.start();
  documentRef.defaultView?.addEventListener?.("beforeunload", () => shellRefreshScheduler.stop(), {
    once: true,
  });
  const designApi = createDesignApi(invokeCommand);
  const designControllerPromise = initDesignWorkbench(documentRef, designApi);
  const aiApi = createAiInterviewApi(invokeCommand);
  initAiInterviewPanel(documentRef, aiApi);
  const pipelineApi = createPipelineApi(invokeCommand);
  let pipelineControllerPromise;
  pipelineApi.exportToPipeline = async () => {
    const designController = await designControllerPromise;
    const latestView = await designController?.latestView({ reload: true });
    if (!latestView) {
      throw new Error(t("shell.exportUnavailable"));
    }
    await designApi.autosave(buildAutosaveDesignRequest());
    await designApi.exportDesign("json");
    applyRoute(documentRef, model.switchRoute("pipeline"));
    const pipelineController = await pipelineControllerPromise;
    const pipelineModel = await pipelineController?.reload();
    return { view: pipelineModel?.view ?? pipelineController?.view ?? null };
  };
  pipelineControllerPromise = initPipelinePanel(documentRef, pipelineApi);
  utilityApis = createUtilityPanelApis(invokeCommand);
  utilityPanelsController = createUtilityPanelsController(documentRef, utilityApis);
  utilityApis.save.currentState = async () => {
    const controller = await designControllerPromise;
    return controller?.latestView({ reload: true }) ?? null;
  };
  utilityApis.save.applyLoadedState = async () => {
    const [designController, pipelineController] = await Promise.all([
      designControllerPromise,
      pipelineControllerPromise,
    ]);
    await Promise.all([
      designController?.reload(),
      pipelineController?.reload(),
      initAiInterviewPanel(documentRef, aiApi),
      utilityPanelsController.refreshAll(),
      refreshShellState(),
    ]);
  };
  void utilityPanelsController.refreshAll();
  const openAiConfig = initAiConfigDialog(documentRef, createAiConfigApi(invokeCommand));
  const settingsStyle = initSettingsStyleModals(documentRef, createSettingsStyleApi(invokeCommand));
  applyInitialUrlState(documentRef, model, openAiConfig, settingsStyle);
  return model;
}

export function applyInitialUrlState(documentRef, model, openAiConfig = null, settingsStyle = null) {
  const location = documentRef.defaultView?.location ?? globalThis.location;
  if (!location) {
    return;
  }
  const params = new URL(location.href).searchParams;
  const route = params.get("route");
  if (route && taskExists(route)) {
    applyRoute(documentRef, model.switchRoute(route));
  }
  if (params.get("step07") === "1") {
    const styleGrid = documentRef.querySelector('[data-role="style-grid"]');
    if (styleGrid) {
      styleGrid.hidden = false;
      if (!styleGrid.textContent.trim()) {
        styleGrid.textContent = t("shell.waitingStyleOptions");
      }
    }
  }
  if (params.get("aiConfig") === "1" && openAiConfig) {
    openAiConfig();
  }
  if (params.get("projectConfig") === "1" && settingsStyle?.openProjectConfig) {
    settingsStyle.openProjectConfig();
  }
  if (params.get("stylePrompt") === "1" && settingsStyle?.openStylePromptEditor) {
    settingsStyle.openStylePromptEditor();
  }
}

function taskExists(route) {
  return TASKS.some((task) => task.id === route);
}

export function formatAiStatus(label) {
  const raw = String(label ?? "").trim();
  if (!raw || raw === "not_configured" || /未配置|not configured/i.test(raw)) {
    return t("ai.notConfigured");
  }
  const incomplete = raw.match(/^AI:\s*(.*?)\s*(?:配置不完整|configuration incomplete)$/i);
  if (incomplete) {
    return t("ai.configIncomplete", { entry: incomplete[1] });
  }
  const configured = raw.match(/^AI:\s*(.*?)\s*\((.*?)\)$/i);
  if (configured) {
    return t("ai.configured", { entry: configured[1], adapter: configured[2] });
  }
  return t("ai.status", { detail: raw.replace(/^AI\s*:\s*/i, "") });
}

export function formatSystemStatus(status) {
  const raw = String(status ?? "").trim();
  if (!raw || raw === "ready" || /^(系统\s*:\s*就绪|system\s*:\s*ready)$/i.test(raw)) {
    return t("shell.ready");
  }
  const pipeline = raw.match(/^(?:系统|system)\s*:\s*(?:流水线|pipeline)\s+(.+)$/i);
  if (pipeline) {
    return t("shell.pipelineStatus", { status: enumLabel("status", pipeline[1]) });
  }
  return raw;
}

if (typeof document !== "undefined") {
  void initApp(document);
}
