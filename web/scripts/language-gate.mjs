import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const webRoot = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const distRoot = join(webRoot, "dist");
const indexPath = join(distRoot, "index.html");
const languages = ["zh-CN", "en-US"];
const browserUnsafePorts = new Set([
  1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95,
  101, 102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143,
  161, 179, 389, 427, 465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554,
  556, 563, 587, 601, 636, 989, 990, 993, 995, 1719, 1720, 1723, 2049, 3659, 4045,
  5060, 5061, 6000, 6566, 6697, 10080,
]);
const surfaces = [
  { id: "design", route: "design" },
  { id: "pipeline", route: "pipeline" },
  { id: "patch", route: "patch" },
  { id: "package", route: "package" },
  { id: "logs", route: "logs" },
  { id: "sdk", route: "sdk" },
  {
    id: "save-manager",
    route: "design",
    openAction: '[data-action="save-manager"]',
    visibleTarget: '[data-role="save-manager-dialog"]',
  },
  {
    id: "template-browser",
    route: "design",
    openAction: '[data-action="template-browser"]',
    visibleTarget: '[data-role="template-browser-modal"]',
  },
  {
    id: "save-template",
    route: "design",
    openAction: '[data-action="save-template"]',
    visibleTarget: '[data-role="save-template-modal"]',
  },
  {
    id: "project-settings",
    route: "pipeline",
    params: { projectConfig: "1" },
    visibleTarget: '[data-role="project-config-modal"]',
  },
  {
    id: "ai-settings",
    route: "design",
    params: { aiConfig: "1" },
    visibleTarget: '[data-role="ai-config-modal"]',
  },
  {
    id: "style-prompt",
    route: "pipeline",
    params: { stylePrompt: "1" },
    visibleTarget: '[data-role="style-prompt-editor-modal"]',
  },
];
const hanPattern = /\p{Script=Han}/u;
const englishActionPattern =
  /\b(?:add|apply|approve|cancel|clear|close|confirm|create|delete|edit|export|generate|import|load|manage|mark|open|refresh|rename|reset|retry|run|save|search|select|stop|submit|view)\b/iu;

assert(existsSync(indexPath), "web/dist/index.html is missing; run npm run build first");
assert(surfaces.length >= 6, "language gate must cover at least six application surfaces");

const chromium = await loadChromium();
const browser = await launchBrowser(chromium);
const staticServer = await startStaticServer(distRoot);
const baseUrl = `http://127.0.0.1:${staticServer.port}`;
const results = [];

try {
  for (const language of languages) {
    for (const surface of surfaces) {
      results.push(await inspectSurface(browser, baseUrl, language, surface));
    }
  }
} finally {
  await browser.close();
  await closeServer(staticServer.server);
}

console.log(
  `language gate passed: ${results.map((result) => `${result.language}/${result.surface}(${result.itemCount})`).join(", ")}`,
);

async function inspectSurface(browserInstance, serverUrl, language, surface) {
  const page = await browserInstance.newPage({ viewport: { width: 1366, height: 900 } });
  try {
    if (surface.id === "template-browser") {
      await installTemplateLanguageFixture(page);
    }
    const url = new URL(`${serverUrl}/index.html`);
    url.searchParams.set("route", surface.route);
    url.searchParams.set("lang", language);
    for (const [key, value] of Object.entries(surface.params ?? {})) {
      url.searchParams.set(key, value);
    }
    await page.goto(url.href, { waitUntil: "load" });
    await page.waitForSelector(`[data-panel="${surface.route}"].active`, { state: "visible" });
    if (surface.openAction) {
      await page.locator(surface.openAction).first().click();
    }
    if (surface.visibleTarget) {
      await page.waitForSelector(surface.visibleTarget, { state: "visible" });
    }
    await page.waitForTimeout(500);
    if (surface.id === "template-browser") {
      await page.waitForFunction(() => document.querySelectorAll(".template-list-item").length === 3);
      await assertTemplatePresentationLanguage(page, language);
    }

    const documentLanguage = await page.evaluate(() => document.documentElement.lang);
    assert(
      documentLanguage === language,
      `${language}/${surface.id}: expected html.lang=${language}, received ${documentLanguage || "<empty>"}`,
    );

    const items = await page.evaluate(collectVisibleChrome);
    assert(items.length >= 8, `${language}/${surface.id}: too few visible chrome strings (${items.length})`);
    const violations = items.filter((item) =>
      language === "en-US" ? containsHan(item.value) : containsEnglishAction(item.value),
    );
    assert(
      violations.length === 0,
      `${language}/${surface.id}: language purity violations:\n${formatViolations(violations)}`,
    );
    return { language, surface: surface.id, itemCount: items.length };
  } finally {
    await page.close();
  }
}

async function installTemplateLanguageFixture(page) {
  await page.addInitScript((templates) => {
    globalThis.__TAURI__ = {
      core: {
        invoke: async (command) => {
          if (command === "get_shell_state") {
            return {
              ok: true,
              data: {
                currentProjectName: "Language Gate",
                systemStatus: "ready",
                aiStatus: "ready",
                progress: { passed: 0, total: 15 },
              },
            };
          }
          if (command === "load_design_workbench") {
            return { ok: true, data: null };
          }
          if (command === "list_templates") {
            return { ok: true, data: { templates: structuredClone(templates), warnings: [] } };
          }
          return { ok: true, data: null };
        },
      },
    };
  }, [
    {
      templateId: "builtin_indie_ftl_faster_than_light",
      source: "builtin",
      name: "FTL: Faster Than Light（超越光速）",
      gameName: "FTL: Faster Than Light",
      targetScale: "indie",
      qualityTier: "B",
      summary: "飞船管理、船员调度与危机取舍组成轻量循环。",
      analysis: [
        "FTL: Faster Than Light is used as an indie spaceship-management reference.",
        "The template emphasizes crew, ship systems, power routing, and crisis tradeoffs.",
      ],
      verification: { mode: "offline_reference", runtimeNetwork: "none" },
    },
    {
      templateId: "builtin_midcore_arknights",
      source: "builtin",
      name: "Arknights（明日方舟）",
      gameName: "Arknights",
      targetScale: "midcore",
      qualityTier: "A",
      summary: "干员编队、部署时机和路线防守构成核心策略。",
      analysis: ["Arknights is used as a squad tower-defense reference."],
      verification: { mode: "offline_reference", runtimeNetwork: "none" },
    },
    {
      templateId: "custom_indie_user_content",
      source: "custom",
      name: "用户内容 / User Content",
      gameName: "用户内容 / User Content",
      targetScale: "indie",
      qualityTier: "custom",
      summary: "User-authored content remains verbatim.",
      analysis: [],
      verification: { mode: "user_saved", runtimeNetwork: "none" },
    },
  ]);
}

async function assertTemplatePresentationLanguage(page, language) {
  const projection = await page.evaluate(() => ({
    names: [...document.querySelectorAll('.template-list-item[data-template-id^="builtin_"] .template-list-name')]
      .map((element) => element.textContent.trim()),
    detailName: document.querySelector('[data-role="template-detail"] h3')?.textContent.trim() ?? "",
    summary: document.querySelector('[data-role="template-detail"] .template-summary')?.textContent.trim() ?? "",
    analysis: [...document.querySelectorAll('[data-role="template-detail"] .template-analysis li')]
      .map((element) => element.textContent.trim()),
  }));
  const builtinText = [...projection.names, projection.detailName, projection.summary, ...projection.analysis]
    .join("\n");
  if (language === "en-US") {
    assert(!hanPattern.test(builtinText), `en-US/template-browser: localized built-in metadata contains Han text:\n${builtinText}`);
    return;
  }
  assert(
    projection.names.join("|") === "超越光速|明日方舟",
    `zh-CN/template-browser: built-in names were not localized: ${projection.names.join("|")}`,
  );
  assert(hanPattern.test(projection.summary), "zh-CN/template-browser: built-in summary is not Chinese");
  assert(
    projection.analysis.length > 0 && projection.analysis.every((item) => hanPattern.test(item)),
    "zh-CN/template-browser: built-in analysis is not Chinese",
  );
}

function collectVisibleChrome() {
  const output = [];
  const excluded = (element) => Boolean(element?.closest?.("[data-content-origin]"));
  const visible = (element) => {
    if (!(element instanceof Element) || !element.isConnected || element.hidden) {
      return false;
    }
    for (let current = element; current; current = current.parentElement) {
      const style = getComputedStyle(current);
      if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") {
        return false;
      }
    }
    return element.getClientRects().length > 0;
  };
  const add = (source, value) => {
    const normalized = String(value ?? "").replace(/\s+/g, " ").trim();
    if (normalized) {
      output.push({ source, value: normalized });
    }
  };

  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const parent = node.parentElement;
    if (!parent || excluded(parent) || !visible(parent) || ["SCRIPT", "STYLE", "NOSCRIPT"].includes(parent.tagName)) {
      continue;
    }
    add(`text:${parent.tagName.toLowerCase()}${parent.id ? `#${parent.id}` : ""}`, node.nodeValue);
  }

  for (const element of document.querySelectorAll("[placeholder], [aria-label], [title]")) {
    if (excluded(element) || !visible(element)) {
      continue;
    }
    for (const attribute of ["placeholder", "aria-label", "title"]) {
      if (element.hasAttribute(attribute)) {
        add(`${attribute}:${element.tagName.toLowerCase()}`, element.getAttribute(attribute));
      }
    }
  }
  for (const select of document.querySelectorAll("select")) {
    if (!excluded(select) && visible(select)) {
      add("text:select", select.selectedOptions[0]?.textContent);
    }
  }
  add("document:title", document.title);
  return output;
}

function containsHan(value) {
  return hanPattern.test(value);
}

function containsEnglishAction(value) {
  return englishActionPattern.test(value);
}

function formatViolations(violations) {
  const unique = new Map();
  for (const violation of violations) {
    unique.set(`${violation.source}\u0000${violation.value}`, violation);
  }
  return [...unique.values()]
    .slice(0, 30)
    .map((item) => `- ${item.source}: ${JSON.stringify(item.value)}`)
    .join("\n");
}

async function loadChromium() {
  try {
    return (await import("playwright")).chromium;
  } catch (error) {
    throw new Error(`Playwright is required for language-gate: ${error.message}`);
  }
}

async function launchBrowser(chromiumType) {
  const candidates = [
    null,
    process.env.ADM_BROWSER,
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  ].filter(
    (candidate, index, values) =>
      candidate === null ||
      (typeof candidate === "string" &&
        candidate.length > 0 &&
        existsSync(candidate) &&
        values.indexOf(candidate) === index),
  );
  const failures = [];
  for (const executablePath of candidates) {
    try {
      return await chromiumType.launch({
        headless: true,
        executablePath: executablePath ?? undefined,
        args: ["--disable-gpu", "--disable-dev-shm-usage"],
      });
    } catch (error) {
      failures.push(`${executablePath ?? "playwright chromium"}: ${error.message}`);
    }
  }
  throw new Error(`unable to launch a browser for language-gate:\n${failures.join("\n")}`);
}

async function startStaticServer(root) {
  for (let attempt = 0; attempt < 32; attempt += 1) {
    const server = createStaticFileServer(root);
    const port = await listenOnLoopback(server);
    if (!isBrowserUnsafePort(port)) {
      return { server, port };
    }
    await closeServer(server);
  }
  throw new Error("unable to allocate a browser-safe local port for language-gate");
}

function createStaticFileServer(root) {
  return createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://127.0.0.1");
      const relativePath = url.pathname === "/" ? "index.html" : url.pathname.replace(/^\/+/, "");
      if (relativePath.includes("..")) {
        response.writeHead(403);
        response.end("forbidden");
        return;
      }
      const filePath = join(root, relativePath);
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

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}
