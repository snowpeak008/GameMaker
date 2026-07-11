import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { formatFindings, scanCatalog } from "../../testdata/fixplan/scripts/terminology-scan.mjs";

const webRoot = dirname(fileURLToPath(new URL("../package.json", import.meta.url)));
const i18nPath = join(webRoot, "src", "i18n.js");
const htmlPath = join(webRoot, "src", "index.html");
const terminologyRulesPath = join(webRoot, "..", "testdata", "fixplan", "terminology", "rules.json");
const languages = ["zh-CN", "en-US"];
const hanPattern = /\p{Script=Han}/u;
const placeholderPattern = /\{([A-Za-z0-9_]+)\}/g;
const parenthesizedPattern = /[（(]([^()（）]*)[)）]/gu;
const englishActionPattern =
  /\b(?:add|apply|approve|cancel|clear|close|confirm|create|delete|edit|export|generate|import|load|manage|mark|open|refresh|rename|reset|retry|run|save|search|select|stop|submit|view)\b/iu;

const {
  catalogFor,
  getLanguageMode,
  normalizeLanguageMode,
  replaceLanguageMode,
  setLanguageMode,
  t,
  translationKeys,
} = await import(pathToFileURL(i18nPath).href);
const catalogs = Object.fromEntries(languages.map((language) => [language, catalogFor(language)]));

verifyCatalogKeySets();
verifyMessages();
await verifyTerminology();
verifyLanguageReplacement();
await verifyHtmlKeys();

console.log(`i18n test passed: ${translationKeys("zh-CN").length} keys across ${languages.join(" / ")}`);

function verifyCatalogKeySets() {
  const expected = translationKeys(languages[0]);
  assert(expected.length > 0, `${languages[0]} catalog is empty`);
  for (const language of languages.slice(1)) {
    const actual = translationKeys(language);
    const missing = expected.filter((key) => !actual.includes(key));
    const extra = actual.filter((key) => !expected.includes(key));
    assert(
      missing.length === 0 && extra.length === 0,
      `${language} key set differs from ${languages[0]}\nmissing: ${formatList(missing)}\nextra: ${formatList(extra)}`,
    );
  }
}

function verifyMessages() {
  const keys = translationKeys(languages[0]);
  for (const key of keys) {
    const zhMessage = catalogs["zh-CN"][key];
    const enMessage = catalogs["en-US"][key];
    verifyNonEmptyString("zh-CN", key, zhMessage);
    verifyNonEmptyString("en-US", key, enMessage);
    assert(
      sameArray(placeholders(zhMessage), placeholders(enMessage)),
      `${key}: placeholder mismatch\nzh-CN: ${formatList(placeholders(zhMessage))}\nen-US: ${formatList(placeholders(enMessage))}`,
    );
    assert(!hanPattern.test(enMessage), `${key}: en-US message contains Han characters: ${JSON.stringify(enMessage)}`);
    for (const match of zhMessage.matchAll(parenthesizedPattern)) {
      const content = match[1].trim();
      assert(
        !englishActionPattern.test(content),
        `${key}: zh-CN message contains an English action in parentheses: ${JSON.stringify(match[0])}`,
      );
    }
  }
}

async function verifyTerminology() {
  const rules = JSON.parse(await readFile(terminologyRulesPath, "utf8"));
  const findings = scanCatalog(catalogs["zh-CN"], rules);
  assert(findings.length === 0, formatFindings(findings));
}

function verifyLanguageReplacement() {
  assert(normalizeLanguageMode("zh") === "zh-CN", "zh alias must normalize to zh-CN");
  assert(normalizeLanguageMode("EN_us") === "en-US", "en alias must normalize to en-US");
  assert(normalizeLanguageMode("missing") === null, "unknown language must be rejected");

  setLanguageMode("en-US", { notify: false });
  assert(getLanguageMode() === "en-US", "setLanguageMode must activate English");
  assert(t("shell.progress", { passed: 2, total: 15 }) === "Progress: 2/15", "English interpolation failed");

  const writes = [];
  let reloadCount = 0;
  const documentRef = {
    title: "",
    documentElement: { lang: "", dataset: {} },
    querySelectorAll: () => [],
    defaultView: {
      localStorage: { setItem: (key, value) => writes.push([key, value]) },
      location: { reload: () => reloadCount += 1 },
    },
  };
  replaceLanguageMode("zh-CN", documentRef);
  assert(getLanguageMode() === "zh-CN", "replaceLanguageMode must activate Chinese");
  assert(writes.at(-1)?.[1] === "zh-CN", "replaceLanguageMode must persist the selection");
  assert(reloadCount === 1, "replaceLanguageMode must reload dynamic views once");

  let rejected = false;
  try {
    setLanguageMode("unsupported", { notify: false });
  } catch {
    rejected = true;
  }
  assert(rejected, "unsupported language modes must throw");
}

async function verifyHtmlKeys() {
  const html = await readFile(htmlPath, "utf8");
  const declaredAttributes = [
    ...html.matchAll(/\bdata-i18n(?:-[a-z0-9-]+)?\b/giu),
  ];
  const attributes = [
    ...html.matchAll(
      /\b(data-i18n(?:-[a-z0-9-]+)?)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))/giu,
    ),
  ];
  assert(attributes.length > 0, "index.html has no data-i18n attributes");
  assert(
    attributes.length === declaredAttributes.length,
    `index.html contains malformed data-i18n attributes (${attributes.length}/${declaredAttributes.length} parsed)`,
  );
  const missing = [];
  for (const match of attributes) {
    const attribute = match[1];
    const key = (match[2] ?? match[3] ?? match[4] ?? "").trim();
    if (!key) {
      missing.push(`${attribute}=<empty>`);
      continue;
    }
    for (const language of languages) {
      if (!Object.hasOwn(catalogs[language], key)) {
        missing.push(`${attribute}=${key} (${language})`);
      }
    }
  }
  assert(missing.length === 0, `index.html references missing translation keys:\n${missing.join("\n")}`);
  verifyHtmlFallbacks(html);
}

function verifyHtmlFallbacks(html) {
  const mismatches = [];
  const plainTextElements = html.matchAll(
    /<([a-z][a-z0-9-]*)\b([^>]*\bdata-i18n\s*=\s*(?:"([^"]+)"|'([^']+)')[^>]*)>([^<]*)<\/\1>/giu,
  );
  for (const match of plainTextElements) {
    const key = (match[3] ?? match[4] ?? "").trim();
    const fallback = normalizeHtmlFallback(match[5]);
    const expected = normalizeHtmlFallback(catalogs["zh-CN"][key]);
    if (fallback !== expected) {
      mismatches.push(`text ${key}: ${JSON.stringify(fallback)} != ${JSON.stringify(expected)}`);
    }
  }

  for (const match of html.matchAll(/<[a-z][a-z0-9-]*\b[^>]*>/giu)) {
    const attributes = parseHtmlAttributes(match[0]);
    for (const [attribute, key] of attributes) {
      if (!attribute.startsWith("data-i18n-") || attribute === "data-i18n") continue;
      const target = attribute.slice("data-i18n-".length);
      const fallback = attributes.get(target);
      const expected = catalogs["zh-CN"][key];
      if (fallback === undefined) {
        mismatches.push(`${target} ${key}: missing fallback attribute`);
      } else if (normalizeHtmlFallback(fallback) !== normalizeHtmlFallback(expected)) {
        mismatches.push(
          `${target} ${key}: ${JSON.stringify(normalizeHtmlFallback(fallback))} != ${JSON.stringify(normalizeHtmlFallback(expected))}`,
        );
      }
    }
  }

  assert(mismatches.length === 0, `index.html fallback text differs from zh-CN catalog:\n${mismatches.join("\n")}`);
}

function parseHtmlAttributes(tag) {
  const attributes = new Map();
  for (const match of tag.matchAll(/\b([a-z][a-z0-9-]*)\s*=\s*(?:"([^"]*)"|'([^']*)')/giu)) {
    attributes.set(match[1].toLowerCase(), match[2] ?? match[3] ?? "");
  }
  return attributes;
}

function normalizeHtmlFallback(value) {
  return String(value ?? "")
    .replace(/&nbsp;/giu, " ")
    .replace(/&hellip;/giu, "…")
    .replace(/&quot;/giu, '"')
    .replace(/&#(?:39|x27);/giu, "'")
    .replace(/&lt;/giu, "<")
    .replace(/&gt;/giu, ">")
    .replace(/&amp;/giu, "&")
    .replace(/\s+/gu, " ")
    .trim();
}

function verifyNonEmptyString(language, key, value) {
  assert(typeof value === "string", `${key}: ${language} value must be a string`);
  assert(value.trim().length > 0, `${key}: ${language} value must not be empty`);
}

function placeholders(value) {
  return [...value.matchAll(placeholderPattern)].map((match) => match[1]).sort();
}

function sameArray(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function formatList(items) {
  return items.length > 0 ? items.join(", ") : "<none>";
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}
