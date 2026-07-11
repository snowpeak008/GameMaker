import { designMessages } from "./locales/design.js";
import { designContentMessages } from "./locales/design-content.generated.js";
import { pipelineMessages } from "./locales/pipeline.js";
import { settingsMessages } from "./locales/settings.js";
import { shellMessages } from "./locales/shell.js";
import { utilityMessages } from "./locales/utility.js";

export const LANGUAGE_MODES = Object.freeze({
  CHINESE: "zh-CN",
  ENGLISH: "en-US",
});

export const DEFAULT_LANGUAGE_MODE = LANGUAGE_MODES.CHINESE;
export const LANGUAGE_STORAGE_KEY = "adm-newrust.language-mode";

const messageGroups = [
  shellMessages,
  designMessages,
  designContentMessages,
  pipelineMessages,
  utilityMessages,
  settingsMessages,
];

const catalogs = Object.freeze(
  Object.fromEntries(
    Object.values(LANGUAGE_MODES).map((language) => [
      language,
      Object.freeze(Object.assign({}, ...messageGroups.map((group) => group[language] ?? {}))),
    ]),
  ),
);

let activeLanguageMode = DEFAULT_LANGUAGE_MODE;

export function normalizeLanguageMode(value, fallback = null) {
  const normalized = String(value ?? "").trim().toLowerCase();
  if (["zh", "zh-cn", "zh_cn", "chinese"].includes(normalized)) {
    return LANGUAGE_MODES.CHINESE;
  }
  if (["en", "en-us", "en_us", "english"].includes(normalized)) {
    return LANGUAGE_MODES.ENGLISH;
  }
  return fallback;
}

export function getLanguageMode() {
  return activeLanguageMode;
}

export function setLanguageMode(language, options = {}) {
  const normalized = normalizeLanguageMode(language);
  if (!normalized) {
    throw new Error(`Unsupported language mode: ${language}`);
  }
  const changed = activeLanguageMode !== normalized;
  activeLanguageMode = normalized;
  if (options.persist) {
    safeStorage(options.documentRef)?.setItem(LANGUAGE_STORAGE_KEY, normalized);
  }
  const documentRef = options.documentRef ?? globalThis.document;
  if (documentRef) {
    applyDocumentTranslations(documentRef);
  }
  if (changed && options.notify !== false) {
    const windowRef = documentRef?.defaultView;
    const EventConstructor = windowRef?.CustomEvent ?? globalThis.CustomEvent;
    if (windowRef?.dispatchEvent && EventConstructor) {
      windowRef.dispatchEvent(
        new EventConstructor("adm:languagechange", { detail: { language: normalized } }),
      );
    }
  }
  return normalized;
}

export function initializeLanguageMode(documentRef = globalThis.document, preferredLanguage = null) {
  const location = documentRef?.defaultView?.location ?? globalThis.location;
  const queryLanguage = location
    ? normalizeLanguageMode(new URL(location.href).searchParams.get("lang"))
    : null;
  const injectedLanguage = normalizeLanguageMode(globalThis.__ADM_NEWRUST_LANGUAGE__);
  const storedLanguage = normalizeLanguageMode(safeStorage(documentRef)?.getItem(LANGUAGE_STORAGE_KEY));
  const documentLanguage = normalizeLanguageMode(documentRef?.documentElement?.dataset?.languageMode);
  const language =
    queryLanguage ??
    injectedLanguage ??
    storedLanguage ??
    normalizeLanguageMode(preferredLanguage) ??
    documentLanguage ??
    DEFAULT_LANGUAGE_MODE;
  return setLanguageMode(language, { documentRef, notify: false });
}

export function replaceLanguageMode(language, documentRef = globalThis.document) {
  const normalized = setLanguageMode(language, {
    documentRef,
    persist: true,
    notify: false,
  });
  documentRef?.defaultView?.location?.reload?.();
  return normalized;
}

export function t(key, variables = {}, language = activeLanguageMode) {
  const normalized = normalizeLanguageMode(language, DEFAULT_LANGUAGE_MODE);
  const template = catalogs[normalized]?.[key] ?? catalogs[DEFAULT_LANGUAGE_MODE]?.[key];
  if (template === undefined) {
    return key;
  }
  return String(template).replace(/\{([A-Za-z0-9_]+)\}/g, (match, name) =>
    Object.hasOwn(variables, name) ? String(variables[name]) : match,
  );
}

export function hasTranslation(key, language = activeLanguageMode) {
  const normalized = normalizeLanguageMode(language, DEFAULT_LANGUAGE_MODE);
  return Object.hasOwn(catalogs[normalized], key);
}

export function translationKeys(language = activeLanguageMode) {
  const normalized = normalizeLanguageMode(language, DEFAULT_LANGUAGE_MODE);
  return Object.keys(catalogs[normalized]).sort();
}

export function catalogFor(language) {
  const normalized = normalizeLanguageMode(language, DEFAULT_LANGUAGE_MODE);
  return catalogs[normalized];
}

export function enumLabel(group, value, variables = {}) {
  const normalizedValue = String(value ?? "").trim().toLowerCase().replaceAll("-", "_");
  const key = `enum.${group}.${normalizedValue}`;
  return hasTranslation(key) ? t(key, { value, ...variables }) : String(value ?? "");
}

export function applyDocumentTranslations(documentRef = globalThis.document) {
  if (!documentRef?.documentElement) {
    return;
  }
  documentRef.documentElement.lang = activeLanguageMode;
  documentRef.documentElement.dataset.languageMode = activeLanguageMode;
  if (hasTranslation("app.documentTitle")) {
    documentRef.title = t("app.documentTitle");
  }
  applyAttributeTranslations(documentRef, "data-i18n", "textContent");
  applyAttributeTranslations(documentRef, "data-i18n-placeholder", "placeholder");
  applyAttributeTranslations(documentRef, "data-i18n-aria-label", "aria-label");
  applyAttributeTranslations(documentRef, "data-i18n-title", "title");
  applyAttributeTranslations(documentRef, "data-i18n-value", "value");
}

function applyAttributeTranslations(documentRef, attribute, target) {
  for (const element of documentRef.querySelectorAll(`[${attribute}]`)) {
    const key = element.getAttribute(attribute);
    if (!key || !hasTranslation(key)) {
      continue;
    }
    if (target === "textContent" || target === "value") {
      if (
        element.hasAttribute("data-content-origin") &&
        !isCatalogOwnedValue(element[target], key)
      ) {
        continue;
      }
      element[target] = t(key);
    } else {
      element.setAttribute(target, t(key));
    }
  }
}

function isCatalogOwnedValue(value, key) {
  const current = String(value ?? "").trim();
  return Object.values(LANGUAGE_MODES).some(
    (language) => String(catalogs[language]?.[key] ?? "").trim() === current,
  );
}

function safeStorage(documentRef) {
  try {
    return documentRef?.defaultView?.localStorage ?? globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}
