import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = dirname(scriptPath);
const rulesPath = resolve(scriptDir, "..", "terminology", "rules.json");
const webRoot = resolve(scriptDir, "..", "..", "..", "web");

export function scanCatalog(catalog, rulesDocument) {
  const findings = [];
  const severity = rulesDocument.default_severity ?? "warning";
  for (const rule of rulesDocument.rules ?? []) {
    const patterns = (rule.key_patterns ?? []).map((pattern) => new RegExp(pattern, "u"));
    for (const [key, value] of Object.entries(catalog ?? {})) {
      if (typeof value !== "string" || !matchesRuleKey(key, rule, patterns)) continue;
      for (const required of rule.required_all ?? []) {
        if (!value.includes(required)) {
          findings.push(finding(rule, key, value, "missing", required, severity));
        }
      }
      for (const forbidden of rule.forbidden ?? []) {
        if (forbidden && value.includes(forbidden)) {
          findings.push(finding(rule, key, value, "forbidden", forbidden, severity));
        }
      }
    }
  }
  return findings;
}

export function formatFindings(findings) {
  if (findings.length === 0) return "terminology scan: no scoped findings";
  const lines = [`terminology scan: ${findings.length} finding(s)`];
  for (const item of findings) {
    lines.push(`- [${item.ruleId}] ${item.key}: ${item.kind} ${JSON.stringify(item.match)} in ${JSON.stringify(item.value)}`);
  }
  return lines.join("\n");
}

function matchesRuleKey(key, rule, patterns) {
  return (rule.exact_keys ?? []).includes(key)
    || (rule.key_prefixes ?? []).some((prefix) => key.startsWith(prefix))
    || patterns.some((pattern) => pattern.test(key));
}

function finding(rule, key, value, kind, match, severity) {
  return {
    severity,
    ruleId: rule.id,
    token: rule.token,
    key,
    value,
    kind,
    match,
    rationale: rule.rationale ?? "",
  };
}

async function runCli() {
  const strict = process.argv.includes("--strict");
  const rules = JSON.parse(await readFile(rulesPath, "utf8"));
  const i18nModule = await import(pathToFileURL(join(webRoot, "src", "i18n.js")).href);
  const catalog = i18nModule.catalogFor("zh-CN");
  const findings = scanCatalog(catalog, rules);
  console.log(formatFindings(findings));
  const hasBlockingFinding = findings.some((item) => item.severity === "error");
  if ((strict || hasBlockingFinding) && findings.length > 0) process.exitCode = 1;
}

if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  await runCli();
}
