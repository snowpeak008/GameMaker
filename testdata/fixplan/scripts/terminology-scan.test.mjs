import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { scanCatalog } from "./terminology-scan.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const rules = JSON.parse(await readFile(resolve(scriptDir, "..", "terminology", "rules.json"), "utf8"));

const findings = scanCatalog({
  "settings.aiConfig.category.dev": "开发接口",
  "enum.aiConfigType.openai_dev_api": "OpenAI 开发接口",
  "enum.aiConfigType.local_codex_cli": "本地 Codex 命令行",
  "settings.aiConfig.field.apiKey": "接口密钥",
  "settings.aiConfig.field.entryId": "配置项标识",
  "format.markdown": "标记文本",
  "description.backend": "请检查后端接口是否可用。",
  "settings.stylePrompt.preview.waiting": "等待提示词",
  "utility.save.table.id": "存档编号",
}, rules);

assert(findings.some((item) => item.ruleId === "TERM-CATEGORY-CONFIG"), "category misuse was not detected");
assert(findings.some((item) => item.ruleId === "TERM-API-TYPE"), "API type misuse was not detected");
assert(findings.some((item) => item.ruleId === "TERM-CLI-TYPE"), "CLI type misuse was not detected");
assert(findings.some((item) => item.ruleId === "TERM-API-KEY"), "API Key misuse was not detected");
assert(findings.some((item) => item.ruleId === "TERM-ID"), "ID misuse was not detected");
assert(findings.some((item) => item.ruleId === "TERM-MARKDOWN-SHELL"), "Markdown misuse was not detected");
assert(findings.every((item) => item.severity === "error"), "terminology findings must be blocking errors");
assert(!findings.some((item) => item.key === "description.backend"), "ordinary 接口 prose must not be flagged");
assert(!findings.some((item) => item.key === "settings.stylePrompt.preview.waiting"), "natural 提示词 copy must not be flagged");
assert(!findings.some((item) => item.key === "utility.save.table.id"), "product-facing 编号 must not be flagged");

const clean = scanCatalog({
  "settings.aiConfig.category.dev": "开发配置",
  "enum.aiConfigType.openai_dev_api": "OpenAI 开发 API",
  "enum.aiConfigType.local_codex_cli": "本地 Codex CLI",
  "settings.aiConfig.field.apiKey": "API Key",
  "settings.aiConfig.field.entryId": "配置项 ID",
  "format.markdown": "Markdown",
}, rules);
assert(clean.length === 0, `clean terminology examples produced ${clean.length} finding(s)`);

console.log(`terminology scan tests passed: ${findings.length} positive findings and scoped natural-language exceptions`);

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
