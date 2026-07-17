import { mkdir, readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { extname } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer } from "node:http";
import {
  sampleAiConfig,
  sampleAiConfigDescriptors,
  sampleDesignView,
  samplePipelineView,
  sampleSaveIndex,
  sampleTemplateList,
} from "./fixtures.mjs";

const webRoot = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const newrustRoot = resolve(webRoot, "..");
const distRoot = join(webRoot, "dist");
const indexPath = join(distRoot, "index.html");
const gateDir = process.env.ADM_UI_GATE_DIR
  ? resolve(process.env.ADM_UI_GATE_DIR)
  : join(newrustRoot, "gates");
const step07ImageBase64 = (
  await readFile(join(newrustRoot, "testdata", "fixplan", "fixtures", "step07", "images", "visible-640x384.png"))
).toString("base64");
const playwrightChromium = await loadPlaywrightChromium();
const viewports = [
  { id: "desktop", width: 1280, height: 820 },
  { id: "compact", width: 1180, height: 720 },
  { id: "narrow", width: 390, height: 844 },
];
const languages = ["zh-CN", "en-US"];
const browserUnsafePorts = new Set([
  1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95,
  101, 102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143,
  161, 179, 389, 427, 465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554,
  556, 563, 587, 601, 636, 989, 990, 993, 995, 1719, 1720, 1723, 2049, 3659, 4045,
  5060, 5061, 6000, 6566, 6697, 10080,
]);

const targets = [
  { id: "shell", query: "?route=design" },
  { id: "design", query: "?route=design" },
  { id: "pipeline", query: "?route=pipeline" },
  { id: "step07", query: "?route=pipeline&step07=1" },
  { id: "recovery", query: "?route=pipeline&recovery=1" },
  { id: "patch", query: "?route=patch" },
  { id: "package", query: "?route=package" },
  { id: "logs", query: "?route=logs" },
  { id: "sdk", query: "?route=sdk" },
  { id: "save_manager", query: "?route=design" },
  { id: "template_browser", query: "?route=design" },
  { id: "save_template", query: "?route=design" },
  { id: "project_config", query: "?route=pipeline&projectConfig=1" },
  { id: "style_prompt_editor", query: "?route=pipeline&stylePrompt=1" },
  { id: "config", query: "?route=design&aiConfig=1" },
];

if (!existsSync(indexPath)) {
  throw new Error("web/dist/index.html is missing; run npm run build first");
}
if (!playwrightChromium) {
  throw new Error("Playwright is required for the UI gate; run npm ci before npm run ui-gate");
}

await mkdir(gateDir, { recursive: true });
const staticServer = await startStaticServer(distRoot);
const baseUrl = `http://127.0.0.1:${staticServer.port}`;

const evidence = [];
let playwrightBrowser = null;
try {
  playwrightBrowser = await playwrightChromium.launch({ headless: true, timeout: 15_000 });
  for (const language of languages) {
    for (const target of targets) {
      for (const viewport of viewports) {
        const viewportSuffix = viewport.id === "desktop" ? "" : `-${viewport.id}`;
        const languagePrefix = language === "zh-CN" ? "" : `${language}-`;
        const filename = `ui-${languagePrefix}${target.id}${viewportSuffix}.png`;
        const screenshotPath = join(gateDir, filename);
        const url = new URL(`${baseUrl}/index.html`);
        url.search = target.query;
        url.searchParams.set("lang", language);
        const browser = await captureWithPlaywright(
          playwrightBrowser,
          target,
          viewport,
          url.href,
          screenshotPath,
        );
        const png = await readFile(screenshotPath);
        const check = inspectPng(png, viewport);
        if (!check.ok) {
          throw new Error(
            `invalid or blank screenshot for ${language}/${target.id}/${viewport.id}: ${check.reason}`,
          );
        }
        evidence.push({
          id: target.id,
          language,
          query: target.query,
          viewport: viewport.id,
          path: `gates/${filename}`,
          width: check.width,
          height: check.height,
          byteLength: png.length,
          uniqueByteCount: check.uniqueByteCount,
          browser,
        });
      }
    }
  }
} finally {
  await playwrightBrowser?.close().catch(() => {});
  await closeServer(staticServer.server);
}

const manifest = {
  gate: "ui-parity",
  playwright: true,
  languages,
  viewports,
  server: baseUrl,
  screenshots: evidence,
};
await writeFile(join(gateDir, "ui-evidence-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);

console.log(`ui gate screenshots passed: ${evidence.map((item) => `${item.language}/${item.id}@${item.viewport}`).join(", ")}`);

async function startStaticServer(root) {
  for (let attempt = 0; attempt < 32; attempt += 1) {
    const server = createStaticFileServer(root);
    const port = await listenOnLoopback(server);
    if (!isBrowserUnsafePort(port)) {
      return { server, port };
    }
    await closeServer(server);
  }
  throw new Error("unable to allocate a browser-safe local port for ui-gate");
}

function createStaticFileServer(root) {
  return createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://127.0.0.1");
      const relative = url.pathname === "/" ? "/index.html" : url.pathname;
      const clean = relative.replace(/^\/+/, "");
      if (clean.includes("..")) {
        response.writeHead(403);
        response.end("forbidden");
        return;
      }
      const filePath = join(root, clean);
      const body = await readFile(filePath);
      response.writeHead(200, { "content-type": contentType(filePath) });
      response.end(body);
    } catch {
      response.writeHead(404);
      response.end("not found");
    }
  });
}

function listenOnLoopback(server) {
  return new Promise((resolveServer, rejectServer) => {
    const onError = (error) => rejectServer(error);
    server.once("error", onError);
    server.listen(0, "127.0.0.1", () => {
      server.removeListener("error", onError);
      resolveServer(server.address().port);
    });
  });
}

function isBrowserUnsafePort(port) {
  return browserUnsafePorts.has(port) || (port >= 6665 && port <= 6669);
}

function closeServer(server) {
  return new Promise((resolveClose) => server.close(resolveClose));
}

function contentType(filePath) {
  return {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".json": "application/json; charset=utf-8",
  }[extname(filePath)] ?? "application/octet-stream";
}

async function loadPlaywrightChromium() {
  try {
    const { chromium } = await import("playwright");
    return chromium;
  } catch {
    return null;
  }
}

async function captureWithPlaywright(browser, target, viewport, url, screenshotPath) {
  const page = await browser.newPage({ viewport: { width: viewport.width, height: viewport.height } });
  try {
    page.setDefaultTimeout(10_000);
    page.setDefaultNavigationTimeout(15_000);
    if (target.id === "save_manager") {
      await installSaveManagerFixture(page, sampleSaveIndex());
    }
    if (["template_browser", "save_template"].includes(target.id)) {
      await installTemplateFixture(page, sampleDesignView(), sampleTemplateList());
    }
    if (target.id === "project_config") {
      await installProjectConfigFixture(page);
    }
    if (target.id === "recovery") {
      await installPipelineRecoveryFixture(page, samplePipelineView());
    }
    if (target.id === "step07") {
      await installPipelineStep07Fixture(page, samplePipelineView(), step07ImageBase64);
    }
    if (target.id === "config") {
      await installAiConfigFixture(page, sampleAiConfig(), sampleAiConfigDescriptors());
    }
    await page.goto(url, { waitUntil: "load", timeout: 15_000 });
    await page.waitForTimeout(700);
    await assertDomState(page, target);
    await assertResponsiveLayout(page, target, viewport);
    await page.screenshot({ path: screenshotPath, fullPage: false, timeout: 15_000 });
    return "playwright:chromium";
  } finally {
    await page.close();
  }
}

async function installTemplateFixture(page, designView, templateReport) {
  await page.addInitScript(({ view, report }) => {
    let templates = structuredClone(report.templates);
    globalThis.__ADM_TEMPLATE_GATE_CALLS = [];
    globalThis.__TAURI__ = {
      core: {
        invoke: async (command, payload = {}) => {
          globalThis.__ADM_TEMPLATE_GATE_CALLS.push({ command, payload });
          if (command === "get_shell_state") {
            return {
              ok: true,
              data: {
                currentProjectName: view.project_name,
                systemStatus: "ready",
                aiStatus: "ready",
                progress: { passed: 0, total: 15 },
              },
            };
          }
          if (command === "load_design_workbench") {
            return { ok: true, data: view };
          }
          if (command === "list_templates") {
            return { ok: true, data: { templates: structuredClone(templates), warnings: [] } };
          }
          if (command === "select_template") {
            await new Promise((resolve) => setTimeout(resolve, 350));
            const template = templates.find(
              (item) => (item.template_id ?? item.templateId) === payload?.request?.template_id,
            );
            const name = template?.name ?? "Template";
            return {
              ok: true,
              data: {
                templateId: payload?.request?.template_id,
                projectName: `${payload?.request?.project_name_prefix ?? ""}${name}`,
                view: { ...view, project_name: `${payload?.request?.project_name_prefix ?? ""}${name}` },
              },
            };
          }
          if (command === "save_template") {
            await new Promise((resolve) => setTimeout(resolve, 350));
            return {
              ok: true,
              data: {
                templateId: `custom_${payload?.request?.target_scale}_fixture`,
                templateName: payload?.request?.template_name,
                targetScale: payload?.request?.target_scale,
                targetFileName: `custom_${payload?.request?.target_scale}_fixture.json`,
                stateHash: "fixture-hash",
                overwritten: Boolean(payload?.request?.overwrite),
              },
            };
          }
          if (command === "delete_template") {
            templates = templates.filter(
              (item) => (item.template_id ?? item.templateId) !== payload?.request?.template_id,
            );
            return {
              ok: true,
              data: {
                templateId: payload?.request?.template_id,
                deleted: true,
              },
            };
          }
          return { ok: true, data: null };
        },
      },
    };
  }, { view: designView, report: templateReport });
}

async function installSaveManagerFixture(page, fixture) {
  await page.addInitScript((saveIndex) => {
    globalThis.__ADM_SAVE_GATE_CALLS = [];
    globalThis.__TAURI__ = {
      core: {
        invoke: async (command, payload = {}) => {
          globalThis.__ADM_SAVE_GATE_CALLS.push({ command, payload });
          if (command === "list_saves") {
            if (globalThis.__ADM_FAIL_SAVE_LIST) {
              throw new Error("fixture list failure");
            }
            return { ok: true, data: saveIndex };
          }
          if (command === "get_shell_state") {
            return {
              ok: true,
              data: {
                currentProjectName: "Gate Project",
                systemStatus: "ready",
                aiStatus: "ready",
                progress: { passed: 0, total: 15 },
              },
            };
          }
          if (command === "load_save") {
            await new Promise((resolve) => setTimeout(resolve, 500));
            if (globalThis.__ADM_FAIL_SAVE_LOAD) {
              return {
                ok: false,
                error: {
                  code: "save_as_required",
                  message: "fixture backend detail",
                  recoverable: true,
                  evidence: [],
                },
              };
            }
            return { ok: true, data: { state: null, index: saveIndex } };
          }
          if ([
            "create_save",
            "create_blank_save",
            "save_project",
            "rename_save",
            "delete_save",
          ].includes(command)) {
            return { ok: true, data: { index: saveIndex } };
          }
          return { ok: true, data: null };
        },
      },
    };
  }, fixture);
}

async function installProjectConfigFixture(page) {
  await page.addInitScript(() => {
    globalThis.__ADM_PROJECT_CONFIG_GATE_CALLS = [];
    let projectPickerCount = 0;
    globalThis.__TAURI__ = {
      core: {
        invoke: async (command, payload = {}) => {
          globalThis.__ADM_PROJECT_CONFIG_GATE_CALLS.push({ command, payload });
          if (command === "get_shell_state") {
            return {
              ok: true,
              data: {
                currentProjectName: "Gate Project",
                systemStatus: "ready",
                aiStatus: "ready",
                progress: { passed: 0, total: 15 },
              },
            };
          }
          if (command === "load_project_config") {
            return {
              ok: true,
              data: {
                schema_version: 1,
                project_engine: "unity",
                development_path: "C:/Fixture/UnityProject",
                editor_path: "C:/Fixture/Unity/Editor/Unity.exe",
              },
            };
          }
          if (command === "select_native_path") {
            if (payload?.request?.kind === "folder") {
              projectPickerCount += 1;
              return projectPickerCount === 1
                ? { ok: true, data: { status: "cancelled", path: null, message: "" } }
                : {
                    ok: true,
                    data: {
                      status: "selected",
                      path: "C:/Fixture/UnityProjectChosen",
                      message: "",
                    },
                  };
            }
            return {
              ok: true,
              data: {
                status: "selected",
                path: "C:/Fixture/Unity/Editor/Unity.exe",
                message: "",
              },
            };
          }
          if (command === "inspect_project_environment") {
            return {
              ok: true,
              data: {
                status: "valid",
                expected_engine: "unity",
                detected_engine: "unity",
                markers: ["Assets", "ProjectSettings", "Packages/manifest.json"],
                unity_version: { version: "2022.3.21f1", revision: "fixture" },
                diagnostics: [],
              },
            };
          }
          if (command === "discover_project_unity_editors") {
            return {
              ok: true,
              data: [
                {
                  path: "C:/Fixture/Unity/2022.3.21f1/Editor/Unity.exe",
                  source: "unity_hub",
                  version: "2022.3.21f1",
                  present: true,
                  valid_executable: true,
                  configured: false,
                  match_kind: "exact",
                },
                {
                  path: "C:/Fixture/Unity/2022.3.9f1/Editor/Unity.exe",
                  source: "unity_hub",
                  version: "2022.3.9f1",
                  present: true,
                  valid_executable: true,
                  configured: false,
                  match_kind: "compatible",
                },
              ],
            };
          }
          return { ok: true, data: null };
        },
      },
    };
  });
}

async function installAiConfigFixture(page, configFixture, descriptorFixture) {
  await page.addInitScript(({ config, descriptors }) => {
    let pickerCount = 0;
    globalThis.__ADM_AI_CONFIG_GATE_CALLS = [];
    globalThis.__TAURI__ = {
      core: {
        invoke: async (command, payload = {}) => {
          globalThis.__ADM_AI_CONFIG_GATE_CALLS.push({ command, payload });
          if (command === "get_shell_state") {
            return {
              ok: true,
              data: {
                currentProjectName: "AI Config Gate",
                systemStatus: "ready",
                aiStatus: { ok: true, label: "AI: codex (codex)" },
                progress: { passed: 0, total: 15 },
              },
            };
          }
          if (command === "load_ai_config") {
            return { ok: true, data: structuredClone(config) };
          }
          if (command === "list_ai_config_descriptors") {
            return { ok: true, data: structuredClone(descriptors) };
          }
          if (command === "select_native_path") {
            pickerCount += 1;
            await new Promise((resolvePicker) => setTimeout(resolvePicker, 120));
            return pickerCount === 1
              ? { ok: true, data: { status: "cancelled", path: null } }
              : { ok: true, data: { status: "selected", path: "" } };
          }
          if (command === "validate_ai_config" || command === "save_ai_config") {
            return { ok: true, data: { ok: true, errors: [] } };
          }
          return { ok: true, data: null };
        },
      },
    };
  }, { config: configFixture, descriptors: descriptorFixture });
}

async function installPipelineStep07Fixture(page, pipelineFixture, imageBase64) {
  await page.addInitScript(({ fixture, content }) => {
    const originalRevokeObjectUrl = URL.revokeObjectURL.bind(URL);
    globalThis.__ADM_REVOKED_PREVIEW_URLS = [];
    URL.revokeObjectURL = (url) => {
      globalThis.__ADM_REVOKED_PREVIEW_URLS.push(url);
      originalRevokeObjectUrl(url);
    };
    globalThis.__TAURI__ = {
      core: {
        invoke: async (command) => {
          if (command === "get_shell_state") {
            return {
              ok: true,
              data: {
                currentProjectName: "Step07 Gate",
                systemStatus: "waiting_confirmation",
                aiStatus: { ok: true, label: "AI: image_api (openai_image)" },
                progress: { passed: 7, total: 15 },
              },
            };
          }
          if (command === "load_pipeline_view") {
            return { ok: true, data: structuredClone(fixture) };
          }
          if (command === "read_pipeline_artifact") {
            return {
              ok: true,
              data: {
                content_type: "image/png",
                encoding: "base64",
                content,
                truncated: false,
              },
            };
          }
          return { ok: true, data: null };
        },
      },
    };
  }, { fixture: pipelineFixture, content: imageBase64 });
}

async function installPipelineRecoveryFixture(page, fixture) {
  await page.addInitScript((pipelineFixture) => {
    const view = structuredClone(pipelineFixture);
    view.running = false;
    view.state = {
      ...(view.state ?? {}),
      run_id: "recoverable_run",
      status: "recoverable",
      stop_requested: false,
    };
    view.recovery = {
      run_id: "recoverable_run",
      attempt_id: "attempt_1",
      revision: 7,
      status: "recoverable",
      from_stage_id: "00",
      to_stage_id: "10",
      current_stage_id: "08",
      next_unit_id: "08:stage",
      updated_at: "unix:1",
    };
    globalThis.__TAURI__ = {
      core: {
        invoke: async (command) => {
          if (command === "get_shell_state") {
            return {
              ok: true,
              data: {
                currentProjectName: "Recovery Gate",
                systemStatus: "recoverable",
                aiStatus: "ready",
                progress: { passed: 7, total: 15 },
              },
            };
          }
          if (command === "load_pipeline_view") {
            return { ok: true, data: structuredClone(view) };
          }
          return { ok: true, data: null };
        },
      },
    };
  }, fixture);
}

async function assertResponsiveLayout(page, target, viewport) {
  const failures = await page.evaluate(() => {
    const selectors = [
      "[data-panel].active .panel-header",
      "[data-panel].active .panel-actions",
      "[data-panel].active .toolbar-row",
      ".modal-backdrop:not([hidden]) .modal-dialog",
      ".modal-backdrop:not([hidden]) .modal-header",
      ".modal-backdrop:not([hidden]) .modal-footer",
      ".modal-backdrop:not([hidden]) button",
      ".modal-backdrop:not([hidden]) .save-create-bar",
      ".modal-backdrop:not([hidden]) .save-manager-workspace",
      ".modal-backdrop:not([hidden]) .save-detail-actions",
      ".modal-backdrop:not([hidden]) .save-confirmation-panel",
      ".modal-backdrop:not([hidden]) .template-browser-body",
      ".modal-backdrop:not([hidden]) .template-pane-header",
      ".modal-backdrop:not([hidden]) .template-detail",
      ".modal-backdrop:not([hidden]) .save-template-body",
      ".modal-backdrop:not([hidden]) .template-confirmation-panel",
    ];
    const problems = [];
    const visible = (element) => {
      const style = getComputedStyle(element);
      return style.display !== "none" && style.visibility !== "hidden" && element.getClientRects().length > 0;
    };
    const rootElements = [
      ["document", document.documentElement],
      ["body", document.body],
      ["app shell", document.querySelector("#app")],
    ];
    if (scrollX !== 0 || scrollY !== 0) {
      problems.push(`window is scrolled (${scrollX}, ${scrollY})`);
    }
    for (const [label, element] of rootElements) {
      if (!element) {
        problems.push(`${label} is missing`);
        continue;
      }
      const style = getComputedStyle(element);
      if (style.overflowY !== "hidden") {
        problems.push(`${label} must hide root vertical overflow (got ${style.overflowY})`);
      }
      if (element.scrollHeight > element.clientHeight + 1) {
        problems.push(`${label} scrolls vertically (${element.scrollHeight} > ${element.clientHeight})`);
      }
    }
    const appRect = document.querySelector("#app")?.getBoundingClientRect();
    if (appRect && (Math.abs(appRect.top) > 1 || Math.abs(appRect.bottom - innerHeight) > 1)) {
      problems.push(`app shell does not match viewport height (${appRect.top.toFixed(1)}..${appRect.bottom.toFixed(1)} / ${innerHeight})`);
    }
    for (const selector of selectors) {
      for (const element of document.querySelectorAll(selector)) {
        if (!visible(element)) {
          continue;
        }
        const rect = element.getBoundingClientRect();
        if (rect.left < -1 || rect.right > innerWidth + 1) {
          problems.push(`${selector} leaves viewport (${rect.left.toFixed(1)}..${rect.right.toFixed(1)} / ${innerWidth})`);
        }
        if (element.scrollWidth > element.clientWidth + 1) {
          problems.push(`${selector} overflows horizontally (${element.scrollWidth} > ${element.clientWidth})`);
        }
      }
    }
    for (const [label, selector, maxHeight] of [
      ["template name input", '[data-role="save-template-modal"] [data-role="template-name"]', 64],
      ["template scale select", '[data-role="save-template-modal"] [data-role="template-scale"]', 64],
    ]) {
      const element = document.querySelector(selector);
      if (element && visible(element) && element.getBoundingClientRect().height > maxHeight) {
        problems.push(`${label} is too tall (${element.getBoundingClientRect().height.toFixed(1)} > ${maxHeight})`);
      }
    }
    return problems;
  });
  if (failures.length > 0) {
    throw new Error(`${target.id}/${viewport.id}: responsive layout failures:\n${failures.join("\n")}`);
  }
}

async function assertDomState(page, target) {
  if (target.id === "config") {
    await assertVisible(page, '[data-role="ai-config-modal"]');
    await assertVisible(page, '[data-ai-config-tab="dev"]');
    await page.waitForFunction(() => document.querySelectorAll(".ai-entry-active-action").length >= 2);
    const activeActions = page.locator(".ai-entry-active-action");
    for (let index = 0; index < await activeActions.count(); index += 1) {
      const box = await activeActions.nth(index).boundingBox();
      if (!box || box.height < 28 || box.height > 34) {
        throw new Error(`AI current-entry action should stay compact (height ${box?.height ?? 0})`);
      }
    }
    const entryLabelAudit = await page.locator(".ai-entry-select").evaluateAll((buttons) =>
      buttons.map((button) => ({
        text: button.textContent.trim(),
        label: button.querySelector("strong")?.textContent.trim() ?? "",
        childCount: button.children.length,
      })),
    );
    if (entryLabelAudit.some((entry) => entry.childCount !== 1 || entry.text !== entry.label)) {
      throw new Error("AI entry list exposed an internal ID or configuration type");
    }
    const previousCurrent = page.locator(".ai-entry-active-action.is-current").first();
    const nextCurrent = page.locator(".ai-entry-active-action:not(.is-current)").first();
    const previousEntryId = await previousCurrent.locator("xpath=..").getAttribute("data-entry-id");
    const nextEntryId = await nextCurrent.locator("xpath=..").getAttribute("data-entry-id");
    await nextCurrent.click();
    await page.waitForFunction(
      (entryId) => document.querySelector(`[data-entry-id="${entryId}"] .ai-entry-active-action`)?.classList.contains("is-current"),
      nextEntryId,
    );
    if (await page.locator(`[data-entry-id="${previousEntryId}"] .ai-entry-active-action`).evaluate((button) => button.classList.contains("is-current"))) {
      throw new Error("previous AI entry remained current after the immediate redraw");
    }
    const devTab = page.locator('[data-ai-config-tab="dev"]');
    await devTab.focus();
    await devTab.press("ArrowRight");
    const imageTab = page.locator('[data-ai-config-tab="image"]');
    if (await imageTab.getAttribute("aria-selected") !== "true" || !(await imageTab.evaluate((tab) => tab === document.activeElement))) {
      throw new Error("AI category tabs do not support ARIA arrow-key activation");
    }
    await imageTab.press("ArrowLeft");
    await page.locator(`[data-entry-id="${previousEntryId}"] .ai-entry-select`).click();
    const cliPath = page.locator('[data-role="entry-cli-path"]');
    const originalCliPath = await cliPath.inputValue();
    const cliPicker = page.locator('[data-action="pick-ai-cli-path"]');
    await cliPicker.click();
    if (!(await cliPicker.isDisabled())) {
      throw new Error("AI native picker did not enter its busy state");
    }
    await page.waitForFunction(() => document.querySelector('[data-action="pick-ai-cli-path"]')?.disabled === false);
    if (await cliPath.inputValue() !== originalCliPath) {
      throw new Error("cancelling the AI native picker changed the previous path");
    }
    await cliPicker.click();
    await page.waitForFunction(() => document.querySelector('[data-action="pick-ai-cli-path"]')?.disabled === false);
    if (await cliPath.inputValue() !== originalCliPath) {
      throw new Error("an empty selected AI path replaced the previous path");
    }
    await page.locator('[data-action="cancel-ai-config"]').click();
    const opener = page.locator(".ai-status");
    await opener.focus();
    await opener.click();
    await assertVisible(page, '[data-role="ai-config-modal"]');
    const footerCancel = page.locator('[data-role="ai-config-modal"] .modal-footer [data-action="cancel-ai-config"]');
    await footerCancel.focus();
    await footerCancel.press("Tab");
    if (!(await devTab.evaluate((tab) => tab === document.activeElement))) {
      throw new Error("AI modal did not trap forward Tab focus");
    }
    await devTab.press("Shift+Tab");
    if (!(await footerCancel.evaluate((button) => button === document.activeElement))) {
      throw new Error("AI modal did not trap reverse Tab focus");
    }
    await page.keyboard.press("Escape");
    await page.waitForFunction(() => document.querySelector('[data-role="ai-config-modal"]')?.hidden === true);
    if (!(await opener.evaluate((button) => button === document.activeElement))) {
      throw new Error("closing the AI modal with Escape did not return focus to its opener");
    }
    await opener.click();
    await assertVisible(page, '[data-role="ai-config-modal"]');
    return;
  }
  if (target.id === "project_config") {
    await assertVisible(page, '[data-panel="pipeline"].active');
    await assertVisible(page, '[data-role="project-config-modal"]');
    await assertVisible(page, '[data-role="project-preflight-output"]');
    const projectPath = page.locator('[data-role="development-project-path"]');
    const originalPath = await projectPath.inputValue();
    await page.locator('[data-action="pick-development-project-path"]').click();
    await page.waitForFunction(
      () => document.querySelector('[data-action="pick-development-project-path"]')?.disabled === false,
    );
    if ((await projectPath.inputValue()) !== originalPath) {
      throw new Error("cancelling the project folder picker changed the existing path");
    }
    await page.locator('[data-action="pick-development-project-path"]').click();
    await page.waitForFunction(
      () => document.querySelector('[data-role="development-project-path"]')?.value.endsWith("UnityProjectChosen"),
    );
    await page.locator('[data-action="discover-unity-editors"]').click();
    await assertVisible(page, '[data-role="unity-editor-candidates"]');
    await page
      .locator('[data-role="unity-editor-candidates"]')
      .selectOption("C:/Fixture/Unity/2022.3.9f1/Editor/Unity.exe");
    if (!(await page.locator('[data-role="editor-path"]').inputValue()).includes("2022.3.9f1")) {
      throw new Error("explicit Unity editor candidate selection did not update the draft path");
    }
    const candidateText = await page.locator('[data-role="unity-editor-candidates"] option').allTextContents();
    const language = await page.locator("html").getAttribute("lang");
    if (language === "zh-CN" && candidateText.some((text) => /\b(?:exact|compatible|unity_hub)\b/iu.test(text))) {
      throw new Error(`Unity candidate protocol values were not localized: ${candidateText.join(" | ")}`);
    }
    const saveCalls = await page.evaluate(() =>
      globalThis.__ADM_PROJECT_CONFIG_GATE_CALLS.filter(
        (call) => call.command === "save_project_config",
      ).length,
    );
    if (saveCalls !== 0) {
      throw new Error("path selection saved project configuration without user confirmation");
    }
    return;
  }
  if (target.id === "style_prompt_editor") {
    await assertVisible(page, '[data-panel="pipeline"].active');
    await assertVisible(page, '[data-role="style-prompt-editor-modal"]');
    await assertVisible(page, '[data-role="style-prompt-preview"]');
    return;
  }
  if (target.id === "recovery") {
    await assertVisible(page, '[data-panel="pipeline"].active');
    await assertVisible(page, '[data-action="resume-pipeline"]');
    if (!(await page.locator('[data-action="run-pipeline"]').isDisabled())) {
      throw new Error("a recoverable run still allows a conflicting new range");
    }
    const visibleText = await page.locator('[data-panel="pipeline"]').innerText();
    if (visibleText.includes("current.json") || visibleText.includes("checkpoints/")) {
      throw new Error("pipeline recovery UI exposed an internal checkpoint path");
    }
    return;
  }
  const route = new URL(`http://local/${target.query}`).searchParams.get("route") ?? "design";
  await assertVisible(page, `[data-panel="${route}"].active`);
  if (target.id === "template_browser") {
    await page.locator('[data-action="template-browser"]').click();
    await assertVisible(page, '[data-role="template-browser-modal"]');
    await page.waitForFunction(() => document.querySelectorAll(".template-list-item").length === 3);
    const initialListCall = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.find((call) => call.command === "list_templates"),
    );
    if (initialListCall?.payload?.request?.include_internal !== true) {
      throw new Error("template browser must request internal templates for Python parity");
    }
    if (!(await page.locator('[data-role="template-detail"]').innerText()).includes("builtin_indie_ftl_faster_than_light")) {
      throw new Error("template browser did not render the selected template detail");
    }
    await page.locator('[data-role="template-list"]').focus();
    await page.keyboard.press("End");
    const selectedId = await page
      .locator('.template-list-item[aria-selected="true"]')
      .getAttribute("data-template-id");
    if (selectedId !== "custom_indie_tactical_demo") {
      throw new Error(`template keyboard selection mismatch: ${selectedId}`);
    }
    if (await page.locator('[data-action="delete-template"]').isDisabled()) {
      throw new Error("custom template should be deletable");
    }
    await page.locator('[data-action="delete-template"]').click();
    await assertVisible(page, '[data-role="template-confirmation"]');
    await page.locator('[data-action="cancel-template-confirmation"]').click();
    const deletesAfterCancel = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.filter((call) => call.command === "delete_template").length,
    );
    if (deletesAfterCancel !== 0) {
      throw new Error("cancelled template delete invoked the backend");
    }
    await page.locator('[data-action="apply-template"]').click();
    await assertVisible(page, '[data-role="template-confirmation"]');
    await page.locator('[data-action="cancel-template-confirmation"]').click();
    const callsAfterCancel = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.filter((call) => call.command === "select_template").length,
    );
    if (callsAfterCancel !== 0) {
      throw new Error("cancelled template load invoked the backend");
    }
    await page.locator('[data-action="apply-template"]').click();
    await page.locator('[data-action="confirm-template-apply"]').click();
    if ((await page.locator('[data-role="template-browser-modal"]').getAttribute("aria-busy")) !== "true") {
      throw new Error("template apply did not enter busy state");
    }
    await page.waitForFunction(() => document.querySelector('[data-role="template-browser-modal"]')?.hidden === true);
    const applyCall = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.findLast((call) => call.command === "select_template"),
    );
    if (applyCall?.payload?.request?.template_id !== "custom_indie_tactical_demo") {
      throw new Error("template apply did not send the selected template id");
    }
    if (Object.hasOwn(applyCall?.payload?.request ?? {}, "project_state")) {
      throw new Error("template apply sent an untrusted client project state");
    }
    if (!String(applyCall?.payload?.request?.project_name_prefix ?? "").trim()) {
      throw new Error("template apply did not send the localized project-name prefix");
    }
    await page.locator('[data-action="template-browser"]').click();
    await page.waitForFunction(() => document.querySelectorAll(".template-list-item").length === 3);
    await page.locator('[data-template-id="builtin_indie_ftl_faster_than_light"]').click();
    if (!(await page.locator('[data-action="delete-template"]').isDisabled())) {
      throw new Error("built-in template delete must be disabled");
    }
    await page.locator('[data-template-id="custom_indie_tactical_demo"]').click();
    await page.locator('[data-action="delete-template"]').click();
    await page.locator('[data-action="confirm-template-delete"]').click();
    await page.waitForFunction(() => document.querySelectorAll(".template-list-item").length === 2);
    const deleteCall = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.findLast((call) => call.command === "delete_template"),
    );
    if (deleteCall?.payload?.request?.template_id !== "custom_indie_tactical_demo") {
      throw new Error("confirmed template delete request mismatch");
    }
    await page.reload({ waitUntil: "load" });
    await page.waitForSelector('[data-panel="design"].active', { state: "visible" });
    await page.locator('[data-action="template-browser"]').click();
    await page.waitForFunction(() => document.querySelectorAll(".template-list-item").length === 3);
    await page.locator('[data-template-id="builtin_indie_ftl_faster_than_light"]').click();
    return;
  }
  if (target.id === "save_template") {
    await page.locator('[data-action="save-template"]').click();
    await assertVisible(page, '[data-role="save-template-modal"]');
    await page.waitForFunction(() => document.querySelector('[data-role="save-template-modal"]')?.getAttribute("aria-busy") === "false");
    const saveDialogListCall = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.find((call) => call.command === "list_templates"),
    );
    if (saveDialogListCall?.payload?.request?.include_internal !== false) {
      throw new Error("save-template dialog should check visible template collisions only");
    }
    const nameInput = page.locator('[data-role="template-name"]');
    await nameInput.fill("Tactical Demo");
    await page.locator('[data-action="confirm-save-template"]').click();
    await assertVisible(page, '[data-role="template-overwrite-confirmation"]');
    await page.locator('[data-action="cancel-template-overwrite"]').click();
    const savesAfterCancel = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.filter((call) => call.command === "save_template").length,
    );
    if (savesAfterCancel !== 0) {
      throw new Error("cancelled template overwrite invoked the backend");
    }
    await page.locator('[data-action="confirm-save-template"]').click();
    await page.locator('[data-action="confirm-template-overwrite"]').click();
    if ((await page.locator('[data-role="save-template-modal"]').getAttribute("aria-busy")) !== "true") {
      throw new Error("template overwrite did not enter busy state");
    }
    await page.waitForFunction(() => document.querySelector('[data-role="save-template-modal"]')?.hidden === true);
    const overwriteCall = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.findLast((call) => call.command === "save_template"),
    );
    if (overwriteCall?.payload?.request?.overwrite !== true
      || overwriteCall?.payload?.request?.target_scale !== "indie") {
      throw new Error("confirmed template overwrite request mismatch");
    }
    await page.locator('[data-action="save-template"]').click();
    await page.waitForFunction(() => document.querySelector('[data-role="save-template-modal"]')?.getAttribute("aria-busy") === "false");
    await nameInput.fill("");
    await page.locator('[data-action="confirm-save-template"]').click();
    await assertVisible(page, '[data-role="save-template-error"]');
    await nameInput.fill("Gate Template");
    await page.locator('[data-action="confirm-save-template"]').click();
    if ((await page.locator('[data-role="save-template-modal"]').getAttribute("aria-busy")) !== "true") {
      throw new Error("save template did not enter busy state");
    }
    await page.waitForFunction(() => document.querySelector('[data-role="save-template-modal"]')?.hidden === true);
    const saveCall = await page.evaluate(() =>
      globalThis.__ADM_TEMPLATE_GATE_CALLS.findLast((call) => call.command === "save_template"),
    );
    if (saveCall?.payload?.request?.template_name !== "Gate Template"
      || saveCall?.payload?.request?.overwrite !== false
      || saveCall?.payload?.request?.target_scale !== "indie") {
      throw new Error("save template request mismatch");
    }
    await page.locator('[data-action="save-template"]').click();
    await assertVisible(page, '[data-role="save-template-modal"]');
    return;
  }
  if (target.id === "save_manager") {
    await page.locator('[data-action="save-manager"]').first().click();
    await page.waitForTimeout(200);
    await assertVisible(page, '[data-role="save-manager-dialog"]');
    await assertVisible(page, '[data-role="save-table"]');
    await assertVisible(page, '[data-role="save-detail"]');
    const saveItems = page.locator(".save-list-item");
    if ((await saveItems.count()) !== 4) {
      throw new Error("save manager fixture did not render all save records");
    }
    if (!(await page.locator('[data-role="save-detail"]').innerText()).includes("42")) {
      throw new Error("save detail did not render transaction metadata");
    }

    await page.locator('[data-role="save-table"]').focus();
    await page.keyboard.press("End");
    const selectedId = await page.locator('.save-list-item[aria-selected="true"]').getAttribute("data-save-id");
    if (selectedId !== "save_corrupt") {
      throw new Error(`keyboard save selection mismatch: ${selectedId}`);
    }

    await page.locator('[data-save-id="save_branch"]').click();
    await page.locator('[data-action="load-save"]').click();
    await assertVisible(page, '[data-role="save-confirmation"]');
    await assertVisible(page, '[data-action="confirm-load-save-current"]');
    await assertVisible(page, '[data-action="confirm-load-discard"]');
    await page.locator('[data-action="confirm-load-save-current"]').click();
    if ((await page.locator('[data-role="save-manager-dialog"]').getAttribute("aria-busy")) !== "true") {
      throw new Error("save manager did not enter busy state during load");
    }
    if (!(await page.locator('[data-action="confirm-load-save-current"]').isDisabled())) {
      throw new Error("save manager did not disable duplicate load submission");
    }
    await page.waitForFunction(() => document.querySelector('[data-role="save-manager-dialog"]')?.getAttribute("aria-busy") === "false");
    const loadCall = await page.evaluate(() =>
      globalThis.__ADM_SAVE_GATE_CALLS.findLast((call) => call.command === "load_save"),
    );
    if (loadCall?.payload?.request?.switch_behavior !== "save_current") {
      throw new Error("save-and-switch did not send the explicit switch behavior");
    }

    await page.locator('[data-save-id="save_branch"]').click();
    await page.locator('[data-action="delete-save"]').click();
    await assertVisible(page, '[data-action="confirm-delete-save"]');
    await page.locator('[data-action="cancel-save-confirmation"]').click();
    if (await page.locator('[data-role="save-confirmation"]').isVisible()) {
      throw new Error("save delete confirmation did not cancel cleanly");
    }

    await page.locator('[data-save-id="save_archive"]').click();
    if (!(await page.locator('[data-action="load-save"]').isDisabled())) {
      throw new Error("locked save should not be loadable");
    }
    if (!(await page.locator('[data-action="delete-save"]').isDisabled())) {
      throw new Error("locked save should not be deletable");
    }

    await page.locator('[data-save-id="save_corrupt"]').click();
    if (!(await page.locator('[data-action="load-save"]').isDisabled())) {
      throw new Error("corrupt save should not be loadable");
    }
    if (!(await page.locator('[data-action="rename-save"]').isDisabled())) {
      throw new Error("corrupt save should not be renameable");
    }
    if (await page.locator('[data-action="delete-save"]').isDisabled()) {
      throw new Error("corrupt save must remain deletable for recovery");
    }
    if (await page.locator('[data-action="open-save-directory"]').isDisabled()) {
      throw new Error("corrupt save directory must remain inspectable");
    }

    await page.evaluate(() => {
      globalThis.__ADM_FAIL_SAVE_LIST = true;
    });
    await page.locator('[data-action="refresh-saves"]').click();
    await page.waitForFunction(() =>
      document.querySelector('[data-role="save-manager-status"]')?.textContent.includes("fixture list failure"),
    );
    if ((await saveItems.count()) !== 4) {
      throw new Error("save list failure was incorrectly rendered as an empty list");
    }
    await page.locator('[data-save-id="save_branch"]').click();
    await page.locator('[data-action="load-save"]').click();
    await assertVisible(page, '[data-role="save-confirmation"]');
    await page.evaluate(() => {
      globalThis.__ADM_FAIL_SAVE_LOAD = true;
    });
    await page.locator('[data-action="confirm-load-save-current"]').click();
    await page.waitForFunction(() => {
      const error = document.querySelector('[data-role="save-confirmation-error"]');
      return error && !error.hidden && error.textContent.trim().length > 0;
    });
    await assertVisible(page, '[data-role="save-confirmation-error"]');
  }
  if (target.id === "step07") {
    await assertVisible(page, '[data-role="style-grid"]');
    const layoutAudit = await page.evaluate(() => {
      const shell = document.querySelector(".pipeline-shell");
      const card = document.querySelector(".step-card");
      return {
        firstTrack: Number.parseFloat(getComputedStyle(shell).gridTemplateColumns),
        cardHeight: card?.getBoundingClientRect().height ?? 0,
      };
    });
    if (page.viewportSize().width >= 900 && Math.abs(layoutAudit.firstTrack - 330) > 1) {
      throw new Error(`pipeline step column is not 330px (${layoutAudit.firstTrack})`);
    }
    if (layoutAudit.cardHeight < 123.5) {
      throw new Error(`pipeline step card height is below 124px (${layoutAudit.cardHeight})`);
    }
    await page.waitForFunction(() => {
      const image = document.querySelector(".style-image-preview-image:not([hidden])");
      return image?.src.startsWith("blob:") && image.naturalWidth > 1 && image.naturalHeight > 1;
    });
    const previewAudit = await page.evaluate(() => {
      const image = document.querySelector(".style-image-preview-image:not([hidden])");
      const html = document.documentElement.outerHTML;
      return {
        src: image?.src ?? "",
        revoked: globalThis.__ADM_REVOKED_PREVIEW_URLS?.includes(image?.src) ?? false,
        leakedBase64: html.includes("iVBOR") || document.body.innerText.includes("[编码=base64]"),
        leakedPath: html.includes("outputs/stage_07") || html.includes("generated_images/"),
      };
    });
    if (!previewAudit.src.startsWith("blob:") || !previewAudit.revoked) {
      throw new Error("Step07 preview did not use and revoke an opaque Blob URL");
    }
    if (previewAudit.leakedBase64 || previewAudit.leakedPath) {
      throw new Error("Step07 preview leaked Base64 or an internal path into the DOM");
    }
    await page.locator('.step-card[data-stage-id="06"]').click();
    if (await page.locator('[data-role="style-grid"]').isVisible()) {
      throw new Error("Step07 images remained visible after selecting Step06");
    }
    const detailHeading = await page.locator('[data-role="pipeline-detail"] h2').textContent();
    if (!detailHeading?.includes("06")) {
      throw new Error("pipeline detail did not switch to the selected Step06 content");
    }
    await page.locator('.step-card[data-stage-id="07"]').click();
    await assertVisible(page, '[data-role="style-grid"]');
  }
}

async function assertVisible(page, selector) {
  const locator = page.locator(selector);
  if ((await locator.count()) === 0) {
    throw new Error(`missing DOM selector: ${selector}`);
  }
  if (!(await locator.first().isVisible())) {
    throw new Error(`DOM selector is not visible: ${selector}`);
  }
}

function inspectPng(buffer, viewport) {
  const signature = "89504e470d0a1a0a";
  if (buffer.subarray(0, 8).toString("hex") !== signature) {
    return { ok: false, reason: "missing PNG signature" };
  }
  const width = buffer.readUInt32BE(16);
  const height = buffer.readUInt32BE(20);
  if (width < viewport.width - 10 || height < viewport.height - 10) {
    return { ok: false, reason: `unexpected dimensions ${width}x${height}` };
  }
  const minimumBytes = viewport.id === "desktop" ? 20_000 : 10_000;
  if (buffer.length < minimumBytes) {
    return { ok: false, reason: `unexpectedly small PNG ${buffer.length}` };
  }
  const uniqueByteCount = new Set(buffer.subarray(128, Math.min(buffer.length, 40_000))).size;
  if (uniqueByteCount < 24) {
    return { ok: false, reason: `low byte variation ${uniqueByteCount}` };
  }
  return { ok: true, width, height, uniqueByteCount };
}
