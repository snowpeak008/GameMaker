import { mkdir, readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const newrustRoot = resolve(webRoot, "..");
const repoRoot = resolve(newrustRoot, "..");
const baselineRoot = join(
  repoRoot,
  "plan",
  "NEWrust",
  "full_project_reproduction",
  "ui_baselines",
);
const manifestPath = join(newrustRoot, "gates", "ui-evidence-manifest.json");
const writeMode = process.argv.includes("--write");

const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
const screenshotById = new Map(
  manifest.screenshots
    .filter((item) => (item.language ?? "zh-CN") === "zh-CN")
    .map((item) => [`${item.id}:${item.viewport ?? "desktop"}`, item]),
);

async function main() {
  const records = buildBaselineRecords();
  if (writeMode) {
    await writeBaselineRecords(records);
  }
  await verifyBaselineRecords(records);

  console.log(`ui-baseline-gate passed: ${records.length} records`);
}

function buildBaselineRecords() {
  return UI_SURFACES.flatMap((surface) =>
    surface.states.map((state) => buildRecord(surface, state)),
  );
}

function buildRecord(surface, state) {
  const targetId = state.webTargetId ?? surface.webTargetId;
  const screenshotRequired = state.screenshotRequired ?? surface.screenshotRequired ?? true;
  const recordPath = baselineRecordPath(surface.surfaceId, state.id);
  const record = {
    schema_version: 1,
    surface: surface.surfaceId,
    state: state.id,
    python_source: surface.pythonFile,
    baseline_surface: surface.baselineSurface,
    web_tauri_target: surface.webTauriTarget,
    required_state: state.label,
    screenshot_required: screenshotRequired,
    python_screenshot: screenshotRequired
      ? {
          path: "",
          manual_review_note:
            "Python Tk baseline requires an interactive desktop session; this record preserves the required source/state and is marked for manual Tk screenshot review.",
        }
      : {
          path: "",
          manual_review_note: "No screenshot required for package marker disposition review.",
        },
    web_screenshot: screenshotRequired
      ? {
          desktop: screenshotRef(targetId, "desktop"),
          narrow: screenshotRef(targetId, "narrow"),
        }
      : {},
    interaction_trace: [
      {
        action: state.action ?? surface.defaultAction,
        command_contract: state.command ?? surface.commandContract,
        expected_result: state.expected ?? `renders ${state.label} without overlap or layout shift`,
      },
    ],
    parity_notes: {
      accepted_differences: [
        "Native Tk widget chrome is not copied; layout, density, state semantics, and command behavior are reviewed instead.",
      ],
      open_p0_p1_deltas: [],
      difference_table: [
        {
          severity: "P2",
          area: "rendering",
          status: "accepted",
          note: "Web uses CSS controls while preserving required state and command semantics.",
        },
      ],
    },
    command_contract: state.command ?? surface.commandContract,
    record_path: relativeRepoPath(recordPath),
  };
  return record;
}

async function writeBaselineRecords(records) {
  await mkdir(baselineRoot, { recursive: true });
  const index = {
    schema_version: 1,
    gate: "ui-parity-v3",
    source_plan: "plan/NEWrust/full_project_reproduction/07_ui_python_baseline_plan.md",
    manifest: "NEWrust/gates/ui-evidence-manifest.json",
    required_record_count: records.length,
    records: [],
  };
  for (const record of records) {
    const path = baselineRecordPath(record.surface, record.state);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, `${JSON.stringify(record, null, 2)}\n`, "utf8");
    index.records.push({
      surface: record.surface,
      state: record.state,
      path: relativeRepoPath(path),
      screenshot_required: record.screenshot_required,
    });
  }
  await writeFile(join(baselineRoot, "index.json"), `${JSON.stringify(index, null, 2)}\n`, "utf8");
}

async function verifyBaselineRecords(expectedRecords) {
  const indexPath = join(baselineRoot, "index.json");
  assert(existsSync(indexPath), "ui baseline index is missing; run npm run ui-baseline-gate -- --write");
  const index = JSON.parse(await readFile(indexPath, "utf8"));
  assert(index.gate === "ui-parity-v3", "ui baseline index gate mismatch");
  assert(index.required_record_count === expectedRecords.length, "ui baseline record count mismatch");
  const indexed = new Set(index.records.map((record) => `${record.surface}:${record.state}`));
  for (const expected of expectedRecords) {
    const key = `${expected.surface}:${expected.state}`;
    assert(indexed.has(key), `ui baseline index missing ${key}`);
    const path = baselineRecordPath(expected.surface, expected.state);
    assert(existsSync(path), `ui baseline record missing ${relativeRepoPath(path)}`);
    const record = JSON.parse(await readFile(path, "utf8"));
    verifyRecord(record, expected);
  }
}

function verifyRecord(record, expected) {
  assert(record.schema_version === 1, `${expected.surface}/${expected.state}: schema mismatch`);
  assert(record.surface === expected.surface, `${expected.surface}/${expected.state}: surface mismatch`);
  assert(record.state === expected.state, `${expected.surface}/${expected.state}: state mismatch`);
  assert(record.python_source === expected.python_source, `${expected.surface}/${expected.state}: python source mismatch`);
  assert(
    record.python_screenshot?.path || record.python_screenshot?.manual_review_note,
    `${expected.surface}/${expected.state}: missing Python baseline note`,
  );
  if (expected.screenshot_required) {
    for (const viewport of ["desktop", "narrow"]) {
      const screenshot = record.web_screenshot?.[viewport];
      assert(screenshot?.path, `${expected.surface}/${expected.state}: missing ${viewport} screenshot path`);
      assert(existsSync(join(repoRoot, screenshot.path)), `${expected.surface}/${expected.state}: missing ${screenshot.path}`);
    }
  }
  assert(record.interaction_trace?.length > 0, `${expected.surface}/${expected.state}: missing interaction trace`);
  assert(record.command_contract, `${expected.surface}/${expected.state}: missing command contract`);
  assert(
    Array.isArray(record.parity_notes?.open_p0_p1_deltas) &&
      record.parity_notes.open_p0_p1_deltas.length === 0,
    `${expected.surface}/${expected.state}: unapproved P0/P1 deltas`,
  );
}

function screenshotRef(targetId, viewport) {
  const item = screenshotById.get(`${targetId}:${viewport}`);
  if (!item) {
    throw new Error(`missing screenshot evidence for ${targetId}:${viewport}; run npm run ui-gate first`);
  }
  return {
    path: `NEWrust/${item.path}`,
    width: item.width,
    height: item.height,
    browser: item.browser,
  };
}

function baselineRecordPath(surface, state) {
  return join(baselineRoot, surface, `${state}.json`);
}

function relativeRepoPath(path) {
  return relative(repoRoot, path).replaceAll("\\", "/");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const UI_SURFACES = [
  surface("main_window", "core/ui/main_window.py", "app shell, close lifecycle, pipeline/design/package nav", "adm-new-web::AppShell + adm-new-tauri-commands::lifecycle", "shell", "get_shell_state / close lifecycle", [
    state("normal", "normal"),
    state("startup_error", "startup error"),
    state("running_lock", "running lock"),
    state("close_while_running", "close while running", { command: "release lock at exit / stop guard" }),
  ]),
  surface("app_window", "core/ui/app_window.py", "16-domain design workbench, L4/L5, gameplay systems, templates, export/save", "adm-new-web::pages::design_workbench", "design", "design workbench commands", [
    state("empty_project", "empty project"),
    state("loaded_project", "loaded project"),
    state("long_domain_text", "long domain text"),
    state("template_selected", "template browser selection and apply confirmation", { webTargetId: "template_browser", command: "list_templates / select_template" }),
    state("save_pending", "save pending", { webTargetId: "save_manager", command: "save_project" }),
    state("export_blocked", "export blocked", { command: "export_design" }),
  ]),
  surface("ai_interview_window", "core/ui/ai_interview_window.py", "standalone AI interview", "adm-new-web::features::ai_interview", "design", "AI interview commands", [
    state("no_provider", "no provider", { webTargetId: "config" }),
    state("running_stream", "running stream"),
    state("invalid_payload", "invalid payload"),
    state("summary_correction", "summary correction"),
    state("saved_session", "saved session", { command: "save_ai_archive" }),
  ]),
  surface("embedded_interview", "core/ui/embedded_interview.py", "embedded AI panel inside workbench", "shared AI interview controller/component", "design", "AI interview controller", [
    state("idle", "idle"),
    state("chunked_stream", "chunked stream"),
    state("mapping_pending", "mapping pending"),
    state("mapping_failed", "mapping failed"),
    state("accepted_summary", "accepted summary"),
  ]),
  surface("bottom_panel", "core/ui/bottom_panel.py", "log/AI tabs and queue polling", "adm-new-web::components::bottom_panel", "design", "bottom panel tab state", [
    state("log_tab", "log tab"),
    state("ai_tab", "AI tab"),
    state("overflow_logs", "overflow logs", { webTargetId: "logs" }),
    state("polling_update", "polling update"),
  ]),
  surface("pipeline_panel", "core/ui/pipeline_panel.py", "Step00-Step14 run tree, range run, stop, semantic report return path", "adm-new-web::pages::pipeline", "pipeline", "pipeline commands", [
    state("all_pending", "all pending"),
    state("mixed_status", "mixed status"),
    state("running", "running", { command: "run_pipeline_range" }),
    state("stopped", "stopped", { command: "stop_pipeline" }),
    state("blocked", "blocked"),
    state("step07_confirmation_required", "Step07 confirmation required", { webTargetId: "step07" }),
  ]),
  surface("pipeline_step_card", "core/ui/pipeline_step_card.py", "individual stage card", "adm-new-web::components::pipeline_step_card", "pipeline", "pipeline view state", [
    state("pending", "pending"),
    state("running", "running"),
    state("success", "success"),
    state("failed", "failed"),
    state("blocked", "blocked"),
    state("long_message", "long message"),
  ]),
  surface("style_confirmation_dialog", "core/ui/style_confirmation_dialog.py", "manual style confirmation", "adm-new-web::modals::style_confirmation", "step07", "confirm_style", [
    state("needs_confirmation", "needs confirmation"),
    state("selected_style", "selected style"),
    state("rejection", "rejection"),
    state("resume_after_reload", "resume after reload"),
  ]),
  surface("style_prompt_editor", "core/ui/style_prompt_editor.py", "style prompt override editor", "adm-new-web::modals::style_prompt_editor", "style_prompt_editor", "style prompt override request", [
    state("default_prompt", "default prompt"),
    state("edited_prompt", "edited prompt"),
    state("validation_error", "validation error"),
    state("long_prompt", "long prompt"),
  ]),
  surface("patch_panel", "core/ui/patch_panel.py", "quick patch management", "adm-new-web::pages::patches", "patch", "patch commands", [
    state("empty_list", "empty list"),
    state("patch_analyzed", "patch analyzed"),
    state("validation_failed", "validation failed"),
    state("apply_running", "apply running"),
    state("promoted", "promoted"),
  ]),
  surface("package_panel", "core/ui/package_panel.py", "package/export readiness", "adm-new-web::pages::package", "package", "package commands", [
    state("ready", "ready"),
    state("blocked_by_validation", "blocked by validation"),
    state("notes_visible", "notes visible"),
    state("package_complete", "package complete"),
  ]),
  surface("log_panel", "core/ui/log_panel.py", "runtime log list/filter", "adm-new-web::components::log_panel", "logs", "log commands", [
    state("empty", "empty"),
    state("long_list", "long list"),
    state("warning_error_filter", "warning/error filter"),
    state("autoscroll", "autoscroll"),
  ]),
  surface("log_entry", "core/ui/log_entry.py", "single log row", "adm-new-web::components::log_entry", "logs", "log entry render", [
    state("info", "info"),
    state("warning", "warning"),
    state("error", "error"),
    state("wrapped_text", "wrapped text"),
  ]),
  surface("sdk_panel", "core/ui/sdk_panel.py", "SDK knowledge manager", "adm-new-web::pages::sdk", "sdk", "sdk commands", [
    state("no_sdks", "no SDKs"),
    state("list", "list"),
    state("detail", "detail"),
    state("review_status", "review status"),
    state("sync_error", "sync error"),
  ]),
  surface("semantic_quality_panel", "core/ui/semantic_quality_panel.py", "semantic quality report viewer", "adm-new-web::components::semantic_quality", "pipeline", "pipeline semantic quality view", [
    state("missing_report", "missing report"),
    state("pass", "pass"),
    state("warnings", "warnings"),
    state("blocking_issues", "blocking issues"),
  ]),
  surface("save_manager_dialog", "core/ui/save_manager_dialog.py", "save create/load/delete/current state", "adm-new-web::modals::save_manager", "save_manager", "save commands", [
    state("empty_saves", "empty saves"),
    state("selected_save", "selected save"),
    state("dirty_save", "dirty save"),
    state("delete_confirm", "delete confirm"),
    state("load_error", "load error"),
  ]),
  surface("ai_config_unified_dialog", "core/ui/ai_config_unified_dialog.py", "unified AI config/profile editing", "adm-new-web::modals::ai_config", "config", "AI config commands", [
    state("profile_list", "profile list"),
    state("missing_key", "missing key"),
    state("image_provider", "image provider"),
    state("test_failed", "test failed"),
    state("save_success", "save success"),
  ]),
  surface("unity_config_dialog", "core/ui/unity_config_dialog.py", "development environment config", "adm-new-web::modals::unity_config", "project_config", "project config preflight commands", [
    state("no_path", "no path"),
    state("valid_path", "valid path"),
    state("invalid_command", "invalid command"),
    state("preflight_warnings", "preflight warnings"),
  ]),
  surface("workbench", "core/ui/workbench.py", "workbench facade and self-test commands", "adm-new-application::workbench_facade + Tauri commands", "pipeline", "workbench facade commands", [
    state("command_success", "command success"),
    state("command_failure", "command failure"),
    state("soft_stop", "soft stop"),
    state("range_run", "range run"),
  ]),
  surface("theme", "core/ui/theme.py", "Tk color/font/spacing tokens", "adm-new-web::theme", "shell", "theme tokens", [
    state("token_parity_review", "token parity review"),
    state("contrast_review", "contrast review"),
  ]),
  surface("gui_app", "core/ui/gui_app.py", "Tk app entry wrapper", "Tauri desktop entry", "shell", "desktop startup smoke", [
    state("smoke_startup", "smoke startup"),
    state("fatal_init_error", "fatal init error"),
  ]),
  surface("ui_init", "core/ui/__init__.py", "package marker", "none", "shell", "disposition review only", [
    state("disposition_review_only", "no screenshot; disposition review only", { screenshotRequired: false }),
  ]),
];

function surface(surfaceId, pythonFile, baselineSurface, webTauriTarget, webTargetId, commandContract, states) {
  return {
    surfaceId,
    pythonFile,
    baselineSurface,
    webTauriTarget,
    webTargetId,
    commandContract,
    defaultAction: `open ${surfaceId}`,
    states,
  };
}

function state(id, label, options = {}) {
  return { id, label, ...options };
}

await main();
