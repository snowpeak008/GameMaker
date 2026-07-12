import { mkdir, readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { findSourceProjectRoot, safeProjectJoin } from "./project-root.mjs";

const webRoot = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const sourceProject = await findSourceProjectRoot(webRoot);
const projectRoot = sourceProject.root;
const baselineRoot = await safeProjectJoin(projectRoot, "testdata/ui_baselines");
const manifestPath = await safeProjectJoin(projectRoot, "gates/ui-evidence-manifest.json");
await verifyEvidencePathGuard();
if (process.argv.includes("--path-guard-self-test")) {
  console.log("ui-baseline path-guard self-test passed");
  process.exit(0);
}
const writeMode = process.argv.includes("--write");
const ACTIVE_UI_CONTRACT_ID = "adm-rust-v2-ui-contract-v1";

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
    schema_version: 2,
    ui_contract_id: `${ACTIVE_UI_CONTRACT_ID}:${surface.surfaceId}`,
    surface: surface.surfaceId,
    state: state.id,
    contract_surface: surface.contractSurface,
    web_tauri_target: surface.webTauriTarget,
    required_state: state.label,
    screenshot_required: screenshotRequired,
    reference_evidence: screenshotRequired
      ? {
          kind: "independent_ui_contract",
          review_note:
            "Review layout, density, state semantics, accessibility, and command behavior against this versioned UI contract.",
        }
      : {
          kind: "contract_only",
          review_note: "No screenshot is required for this contract disposition record.",
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
    contract_notes: {
      rendering_constraints: [
        "Platform rendering details may vary; required layout, density, state semantics, accessibility, and command behavior must remain stable.",
      ],
      open_blocking_deltas: [],
      difference_table: [
        {
          severity: "P2",
          area: "rendering",
          status: "accepted",
          note: "Platform control rendering may vary while preserving the independent UI contract.",
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
    schema_version: 2,
    gate: "ui-contract-v1",
    ui_contract_id: ACTIVE_UI_CONTRACT_ID,
    evidence_manifest: "gates/ui-evidence-manifest.json",
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
  assert(index.schema_version === 2, "ui baseline index schema mismatch");
  assert(index.gate === "ui-contract-v1", "ui baseline index gate mismatch");
  assert(index.ui_contract_id === ACTIVE_UI_CONTRACT_ID, "ui baseline contract id mismatch");
  assert(index.evidence_manifest === "gates/ui-evidence-manifest.json", "ui evidence path must be project-root-relative");
  assertExactKeys(
    index,
    ["schema_version", "gate", "ui_contract_id", "evidence_manifest", "required_record_count", "records"],
    "ui baseline index",
  );
  assert(index.required_record_count === expectedRecords.length, "ui baseline record count mismatch");
  const indexed = new Set(index.records.map((record) => `${record.surface}:${record.state}`));
  for (const expected of expectedRecords) {
    const key = `${expected.surface}:${expected.state}`;
    assert(indexed.has(key), `ui baseline index missing ${key}`);
    const path = baselineRecordPath(expected.surface, expected.state);
    assert(existsSync(path), `ui baseline record missing ${relativeRepoPath(path)}`);
    const record = JSON.parse(await readFile(path, "utf8"));
    await verifyRecord(record, expected);
  }
}

async function verifyRecord(record, expected) {
  assert(record.schema_version === 2, `${expected.surface}/${expected.state}: schema mismatch`);
  assert(record.ui_contract_id === expected.ui_contract_id, `${expected.surface}/${expected.state}: contract id mismatch`);
  assert(record.surface === expected.surface, `${expected.surface}/${expected.state}: surface mismatch`);
  assert(record.state === expected.state, `${expected.surface}/${expected.state}: state mismatch`);
  assert(record.contract_surface === expected.contract_surface, `${expected.surface}/${expected.state}: contract surface mismatch`);
  const recordPath = await resolveEvidencePath(
    record.record_path,
    "testdata/ui_baselines/",
    `${expected.surface}/${expected.state}: baseline path`,
  );
  assert(
    recordPath === baselineRecordPath(expected.surface, expected.state),
    `${expected.surface}/${expected.state}: baseline path does not match its contract record`,
  );
  assertExactKeys(
    record,
    [
      "schema_version", "ui_contract_id", "surface", "state", "contract_surface",
      "web_tauri_target", "required_state", "screenshot_required", "reference_evidence",
      "web_screenshot", "interaction_trace", "contract_notes", "command_contract", "record_path",
    ],
    `${expected.surface}/${expected.state}`,
  );
  assert(
    record.reference_evidence?.kind && record.reference_evidence?.review_note,
    `${expected.surface}/${expected.state}: missing contract reference evidence`,
  );
  if (expected.screenshot_required) {
    for (const viewport of ["desktop", "narrow"]) {
      const screenshot = record.web_screenshot?.[viewport];
      assert(screenshot?.path, `${expected.surface}/${expected.state}: missing ${viewport} screenshot path`);
      const screenshotPath = await resolveEvidencePath(
        screenshot.path,
        "gates/",
        `${expected.surface}/${expected.state}: ${viewport} screenshot path`,
      );
      assert(existsSync(screenshotPath), `${expected.surface}/${expected.state}: missing ${screenshot.path}`);
    }
  }
  assert(record.interaction_trace?.length > 0, `${expected.surface}/${expected.state}: missing interaction trace`);
  assert(record.command_contract, `${expected.surface}/${expected.state}: missing command contract`);
  assert(
    Array.isArray(record.contract_notes?.open_blocking_deltas) &&
      record.contract_notes.open_blocking_deltas.length === 0,
    `${expected.surface}/${expected.state}: unapproved blocking deltas`,
  );
}

async function resolveEvidencePath(relativePath, requiredPrefix, label) {
  assert(
    typeof relativePath === "string" && relativePath.startsWith(requiredPrefix),
    `${label} is not in ${requiredPrefix}`,
  );
  return safeProjectJoin(projectRoot, relativePath);
}

async function verifyEvidencePathGuard() {
  for (const maliciousPath of [
    "gates/../../outside.png",
    "gates/..\\..\\outside.png",
    "testdata/ui_baselines/../../../outside.json",
  ]) {
    let rejected = false;
    try {
      const prefix = maliciousPath.startsWith("gates/") ? "gates/" : "testdata/ui_baselines/";
      await resolveEvidencePath(maliciousPath, prefix, "path-guard self-test");
    } catch {
      rejected = true;
    }
    assert(rejected, `path-guard self-test accepted traversal: ${maliciousPath}`);
  }
}

function screenshotRef(targetId, viewport) {
  const item = screenshotById.get(`${targetId}:${viewport}`);
  if (!item) {
    throw new Error(`missing screenshot evidence for ${targetId}:${viewport}; run npm run ui-gate first`);
  }
  return {
    path: item.path,
    width: item.width,
    height: item.height,
    browser: item.browser,
  };
}

function baselineRecordPath(surface, state) {
  return join(baselineRoot, surface, `${state}.json`);
}

function relativeRepoPath(path) {
  return relative(projectRoot, path).replaceAll("\\", "/");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertExactKeys(value, expectedKeys, label) {
  const actual = Object.keys(value).sort();
  const expected = [...expectedKeys].sort();
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${label}: unexpected contract fields`);
}

const UI_SURFACES = [
  surface("main_window", "app shell, close lifecycle, pipeline/design/package nav", "adm-new-web::AppShell + adm-new-tauri-commands::lifecycle", "shell", "get_shell_state / close lifecycle", [
    state("normal", "normal"),
    state("startup_error", "startup error"),
    state("running_lock", "running lock"),
    state("close_while_running", "close while running", { command: "release lock at exit / stop guard" }),
  ]),
  surface("app_window", "16-domain design workbench, L4/L5, gameplay systems, templates, export/save", "adm-new-web::pages::design_workbench", "design", "design workbench commands", [
    state("empty_project", "empty project"),
    state("loaded_project", "loaded project"),
    state("long_domain_text", "long domain text"),
    state("template_selected", "template browser selection and apply confirmation", { webTargetId: "template_browser", command: "list_templates / select_template" }),
    state("save_pending", "save pending", { webTargetId: "save_manager", command: "save_project" }),
    state("export_blocked", "export blocked", { command: "export_design" }),
  ]),
  surface("ai_interview_window", "standalone AI interview", "adm-new-web::features::ai_interview", "design", "AI interview commands", [
    state("no_provider", "no provider", { webTargetId: "config" }),
    state("running_stream", "running stream"),
    state("invalid_payload", "invalid payload"),
    state("summary_correction", "summary correction"),
    state("saved_session", "saved session", { command: "save_ai_archive" }),
  ]),
  surface("embedded_interview", "embedded AI panel inside workbench", "shared AI interview controller/component", "design", "AI interview controller", [
    state("idle", "idle"),
    state("chunked_stream", "chunked stream"),
    state("mapping_pending", "mapping pending"),
    state("mapping_failed", "mapping failed"),
    state("accepted_summary", "accepted summary"),
  ]),
  surface("bottom_panel", "log/AI tabs and queue polling", "adm-new-web::components::bottom_panel", "design", "bottom panel tab state", [
    state("log_tab", "log tab"),
    state("ai_tab", "AI tab"),
    state("overflow_logs", "overflow logs", { webTargetId: "logs" }),
    state("polling_update", "polling update"),
  ]),
  surface("pipeline_panel", "Step00-Step14 run tree, range run, stop, semantic report return path", "adm-new-web::pages::pipeline", "pipeline", "pipeline commands", [
    state("all_pending", "all pending"),
    state("mixed_status", "mixed status"),
    state("running", "running", { command: "run_pipeline_range" }),
    state("stopped", "stopped", { command: "stop_pipeline" }),
    state("blocked", "blocked"),
    state("step07_confirmation_required", "Step07 confirmation required", { webTargetId: "step07" }),
  ]),
  surface("pipeline_step_card", "individual stage card", "adm-new-web::components::pipeline_step_card", "pipeline", "pipeline view state", [
    state("pending", "pending"),
    state("running", "running"),
    state("success", "success"),
    state("failed", "failed"),
    state("blocked", "blocked"),
    state("long_message", "long message"),
  ]),
  surface("style_confirmation_dialog", "manual style confirmation", "adm-new-web::modals::style_confirmation", "step07", "confirm_style", [
    state("needs_confirmation", "needs confirmation"),
    state("selected_style", "selected style"),
    state("rejection", "rejection"),
    state("resume_after_reload", "resume after reload"),
  ]),
  surface("style_prompt_editor", "style prompt override editor", "adm-new-web::modals::style_prompt_editor", "style_prompt_editor", "style prompt override request", [
    state("default_prompt", "default prompt"),
    state("edited_prompt", "edited prompt"),
    state("validation_error", "validation error"),
    state("long_prompt", "long prompt"),
  ]),
  surface("patch_panel", "quick patch management", "adm-new-web::pages::patches", "patch", "patch commands", [
    state("empty_list", "empty list"),
    state("patch_analyzed", "patch analyzed"),
    state("validation_failed", "validation failed"),
    state("apply_running", "apply running"),
    state("promoted", "promoted"),
  ]),
  surface("package_panel", "package/export readiness", "adm-new-web::pages::package", "package", "package commands", [
    state("ready", "ready"),
    state("blocked_by_validation", "blocked by validation"),
    state("notes_visible", "notes visible"),
    state("package_complete", "package complete"),
  ]),
  surface("log_panel", "runtime log list/filter", "adm-new-web::components::log_panel", "logs", "log commands", [
    state("empty", "empty"),
    state("long_list", "long list"),
    state("warning_error_filter", "warning/error filter"),
    state("autoscroll", "autoscroll"),
  ]),
  surface("log_entry", "single log row", "adm-new-web::components::log_entry", "logs", "log entry render", [
    state("info", "info"),
    state("warning", "warning"),
    state("error", "error"),
    state("wrapped_text", "wrapped text"),
  ]),
  surface("sdk_panel", "SDK knowledge manager", "adm-new-web::pages::sdk", "sdk", "sdk commands", [
    state("no_sdks", "no SDKs"),
    state("list", "list"),
    state("detail", "detail"),
    state("review_status", "review status"),
    state("sync_error", "sync error"),
  ]),
  surface("semantic_quality_panel", "semantic quality report viewer", "adm-new-web::components::semantic_quality", "pipeline", "pipeline semantic quality view", [
    state("missing_report", "missing report"),
    state("pass", "pass"),
    state("warnings", "warnings"),
    state("blocking_issues", "blocking issues"),
  ]),
  surface("save_manager_dialog", "save create/load/delete/current state", "adm-new-web::modals::save_manager", "save_manager", "save commands", [
    state("empty_saves", "empty saves"),
    state("selected_save", "selected save"),
    state("dirty_save", "dirty save"),
    state("delete_confirm", "delete confirm"),
    state("load_error", "load error"),
  ]),
  surface("ai_config_unified_dialog", "unified AI config/profile editing", "adm-new-web::modals::ai_config", "config", "AI config commands", [
    state("profile_list", "profile list"),
    state("missing_key", "missing key"),
    state("image_provider", "image provider"),
    state("test_failed", "test failed"),
    state("save_success", "save success"),
  ]),
  surface("unity_config_dialog", "development environment config", "adm-new-web::modals::unity_config", "project_config", "project config preflight commands", [
    state("no_path", "no path"),
    state("valid_path", "valid path"),
    state("invalid_command", "invalid command"),
    state("preflight_warnings", "preflight warnings"),
  ]),
  surface("workbench", "workbench facade and self-test commands", "adm-new-application::workbench_facade + Tauri commands", "pipeline", "workbench facade commands", [
    state("command_success", "command success"),
    state("command_failure", "command failure"),
    state("soft_stop", "soft stop"),
    state("range_run", "range run"),
  ]),
  surface("theme", "color/font/spacing token contract", "adm-new-web::theme", "shell", "theme tokens", [
    state("token_parity_review", "token parity review"),
    state("contrast_review", "contrast review"),
  ]),
  surface("gui_app", "desktop application entry", "Tauri desktop entry", "shell", "desktop startup smoke", [
    state("smoke_startup", "smoke startup"),
    state("fatal_init_error", "fatal init error"),
  ]),
  surface("ui_init", "package marker", "none", "shell", "disposition review only", [
    state("disposition_review_only", "no screenshot; disposition review only", { screenshotRequired: false }),
  ]),
];

function surface(surfaceId, contractSurface, webTauriTarget, webTargetId, commandContract, states) {
  return {
    surfaceId,
    contractSurface,
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
