import { readFile, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { readPngDimensions } from "./generate-png-fixtures.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, "..", "fixtures");

const jsonPaths = [
  "manifest.json",
  "ai/ai-config-v3.json",
  "ai/ai-config-legacy-v1.json",
  "project/project-config-v1.json",
  "project/project-config-v2.json",
  "project/project-bindings.json",
  "project/unity-project/Packages/manifest.json",
  "pipeline/pipeline-state-legacy-idle.json",
  "pipeline/pipeline-state-legacy-running.json",
  "pipeline/checkpoint-recoverable-v1.json",
  "step07/style-options.json",
  "step07/generation-log.json",
  "ui/long-ui-data.json",
];

const documents = Object.fromEntries(
  await Promise.all(jsonPaths.map(async (relativePath) => {
    const text = await readFile(join(root, relativePath), "utf8");
    assertNoSecretOrUserPath(relativePath, text);
    return [relativePath, JSON.parse(text)];
  })),
);

verifyAiV3(documents["ai/ai-config-v3.json"]);
verifyLegacyAi(documents["ai/ai-config-legacy-v1.json"]);
await verifyProjectFixtures(documents);
verifyPipelineFixtures(documents);
await verifyStep07Fixtures(documents);
verifyLongUiData(documents["ui/long-ui-data.json"]);

console.log(`fixplan fixtures verified: ${jsonPaths.length} JSON documents, 2 PNG files, no credentials or absolute user paths`);

function verifyAiV3(config) {
  assert(config.schema_version === 3, "AI v3 fixture must use schema 3");
  for (const categoryId of ["dev", "image", "completion"]) {
    const category = config[categoryId];
    assert(category?.category_id === categoryId, `${categoryId}: category_id mismatch`);
    assert(Array.isArray(category.entries) && category.entries.length >= 2, `${categoryId}: expected CLI/API alternatives`);
    assert(category.entries.some((entry) => entry.id === category.active_entry_id), `${categoryId}: active entry is missing`);
    for (const entry of category.entries) {
      assert(entry.api_key === "", `${categoryId}/${entry.id}: credential must be empty`);
      if (entry.extra_json) {
        JSON.parse(entry.extra_json);
      }
    }
  }
  const extension = JSON.parse(config.dev.entries[0].extra_json).fixture_extension;
  const roundTripped = JSON.parse(JSON.stringify(extension));
  assert(roundTripped.keep === true && roundTripped.revision === 7, "extra_json fixture extension did not round-trip");
}

function verifyLegacyAi(config) {
  assert(config.schema_version === 1, "legacy AI fixture must use schema 1");
  assert(config.active_profile === "fixture_legacy_cli", "legacy active profile mismatch");
  assert(config.profiles?.[0]?.llm?.source === "cli", "legacy fixture must exercise CLI migration");
  assert(config.profiles[0].llm.api_key === "", "legacy credential must be empty");
  assert(config.profiles[0].metadata.fixture_extension.keep === true, "legacy metadata extension missing");
}

async function verifyProjectFixtures(all) {
  const logical = all["project/project-config-v2.json"];
  assert(logical.schema_version === 2, "logical project fixture must use schema 2");
  assert(!Object.hasOwn(logical, "development_path"), "logical project fixture leaks development_path");
  assert(!Object.hasOwn(logical, "editor_path"), "logical project fixture leaks editor_path");
  assert(logical.future_field.keep === true, "logical project unknown field missing");
  const bindings = all["project/project-bindings.json"];
  assert(bindings.bindings[logical.binding_id], "logical project binding is missing");
  for (const marker of [
    "project/unity-project/Assets/.fixture-marker",
    "project/unity-project/ProjectSettings/ProjectVersion.txt",
    "project/unity-project/Packages/manifest.json",
  ]) {
    assert((await stat(join(root, marker))).isFile(), `Unity marker is missing: ${marker}`);
  }
}

function verifyPipelineFixtures(all) {
  const legacy = all["pipeline/pipeline-state-legacy-running.json"];
  assert(!Object.hasOwn(legacy, "schema_version"), "legacy state must remain unversioned input");
  assert(legacy.status === "running", "legacy interrupted state must exercise recovery inspection");
  const checkpoint = all["pipeline/checkpoint-recoverable-v1.json"];
  assert(checkpoint.schema_version === 1 && checkpoint.revision > 0, "checkpoint schema/revision mismatch");
  assert(checkpoint.resume_policy === "explicit_only", "checkpoint must require explicit resume");
  assert(checkpoint.range.stage_ids.at(0) === checkpoint.range.from_stage_id, "checkpoint range start mismatch");
  assert(checkpoint.range.stage_ids.at(-1) === checkpoint.range.to_stage_id, "checkpoint range end mismatch");
}

async function verifyStep07Fixtures(all) {
  const visible = readPngDimensions(await readFile(join(root, "step07/images/visible-640x384.png")));
  const legacy = readPngDimensions(await readFile(join(root, "step07/images/legacy-1x1.png")));
  assert(visible.width === 640 && visible.height === 384, "visible Step07 PNG must be 640x384");
  assert(legacy.width === 1 && legacy.height === 1, "legacy Step07 PNG must be 1x1");
  const log = all["step07/generation-log.json"];
  assert(log.provider_generated_count === 0 && log.fallback_count === 1, "fallback must not count as provider generated");
  const options = all["step07/style-options.json"].options;
  assert(options.some((option) => option.image_status === "fallback"), "visible fallback option missing");
  assert(options.some((option) => option.image_status === "legacy_placeholder"), "legacy placeholder option missing");
}

function verifyLongUiData(data) {
  assert(data.project_name.length > 60, "long UI project name is too short");
  assert(data.stages.length === 15, "long UI fixture must contain all 15 stages");
  assert(data.logs.length >= 18, "long UI fixture needs enough log rows");
  assert(data.table_rows.length >= 24, "long UI fixture needs enough table rows");
  assert(data.interview_messages.length >= 4, "long UI fixture needs interview history");
}

function assertNoSecretOrUserPath(relativePath, text) {
  const secretPatterns = [
    /\bsk-[A-Za-z0-9_-]{8,}\b/u,
    /\bghp_[A-Za-z0-9]{8,}\b/u,
    /\bBearer\s+[A-Za-z0-9._-]{8,}/u,
  ];
  const absoluteUserPathPatterns = [
    /(?:^|["'\s])[A-Za-z]:[\\/]/mu,
    /\\\\[^\\\s]+\\[^\\\s]+/mu,
    /\/(?:Users|home)\/[^/\s]+/mu,
  ];
  for (const pattern of [...secretPatterns, ...absoluteUserPathPatterns]) {
    assert(!pattern.test(text), `${relativePath}: forbidden secret or absolute user path matched ${pattern}`);
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

