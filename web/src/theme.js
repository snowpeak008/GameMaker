export const THEME_TOKENS = Object.freeze({
  bg: "#F3F6F8",
  surface: "#FFFFFF",
  surfaceAlt: "#F8FAFC",
  border: "#D7E0E8",
  borderStrong: "#A8B7C5",
  text: "#15202B",
  muted: "#657486",
  primary: "#2563EB",
  primarySoft: "#EAF1FF",
  success: "#0F8A5F",
  successSoft: "#E7F7EF",
  warning: "#B45309",
  warningSoft: "#FFF4DE",
  danger: "#B42318",
  dangerSoft: "#FDEBEA",
  dark: "#17212B",
  userMessageBg: "#EFF6FF",
  userMessageBorder: "#2563EB",
  aiMessageBg: "#ECFDF5",
  aiMessageBorder: "#0F8A5F",
  systemMessageBg: "#FFF7ED",
  systemMessageBorder: "#B45309",
});

export const SHELL_THEME = Object.freeze({
  fontBody: '"Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif',
  fontMono: 'Consolas, "Courier New", monospace',
  minWindowWidth: 1180,
  minWindowHeight: 720,
  defaultWindowWidth: 1280,
  defaultWindowHeight: 820,
  statusReady: "ready",
});

const CSS_TOKEN_NAMES = Object.freeze({
  bg: "--bg",
  surface: "--surface",
  surfaceAlt: "--surface-alt",
  border: "--border",
  borderStrong: "--border-strong",
  text: "--text",
  muted: "--muted",
  primary: "--primary",
  primarySoft: "--primary-soft",
  success: "--success",
  successSoft: "--success-soft",
  warning: "--warning",
  warningSoft: "--warning-soft",
  danger: "--danger",
  dangerSoft: "--danger-soft",
  dark: "--dark",
  userMessageBg: "--user-message-bg",
  userMessageBorder: "--user-message-border",
  aiMessageBg: "--ai-message-bg",
  aiMessageBorder: "--ai-message-border",
  systemMessageBg: "--system-message-bg",
  systemMessageBorder: "--system-message-border",
});

export function applyThemeTokens(documentRef = globalThis.document, tokens = THEME_TOKENS) {
  const root = documentRef?.documentElement;
  if (!root?.style) {
    return [];
  }
  const applied = [];
  for (const [name, cssName] of Object.entries(CSS_TOKEN_NAMES)) {
    const value = tokens[name];
    if (!value) {
      continue;
    }
    root.style.setProperty(cssName, value.toLowerCase());
    applied.push(cssName);
  }
  return applied;
}

export function normalizeShellThemeTokens(tokens = []) {
  const normalized = { ...THEME_TOKENS };
  for (const token of tokens) {
    if (!token?.name || !token?.value) {
      continue;
    }
    const key = snakeToCamel(token.name);
    if (Object.hasOwn(normalized, key)) {
      normalized[key] = String(token.value);
    }
  }
  return normalized;
}

function snakeToCamel(value) {
  return String(value).replace(/_([a-z])/g, (_, ch) => ch.toUpperCase());
}
