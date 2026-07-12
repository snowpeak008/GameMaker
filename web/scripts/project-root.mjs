import { existsSync } from "node:fs";
import { lstat, readFile, realpath, stat } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";

export const ROOT_MARKER = ".project_root";
export const SOURCE_PROJECT_ID = "autodesignmaker-rust-v2";

const SOURCE_PROJECT_KIND = "source-project-root";
const SOURCE_PROJECT_SCHEMA_VERSION = 1;
const REQUIRED_LOCKFILES = ["Cargo.lock", "web/package-lock.json"];

export async function findSourceProjectRoot(startPath) {
  const normalizedStart = await realpath(startPath).catch((error) => {
    throw new Error(`unable to resolve source project search start ${startPath}: ${error.message}`);
  });
  const startStats = await stat(normalizedStart);
  let current = startStats.isFile() ? dirname(normalizedStart) : normalizedStart;
  for (;;) {
    if (existsSync(join(current, ROOT_MARKER))) {
      return openSourceProjectRoot(current);
    }
    const parent = dirname(current);
    if (parent === current) {
      throw new Error(`unable to locate source project root from ${startPath}`);
    }
    current = parent;
  }
}

export async function openSourceProjectRoot(rootPath) {
  const root = await realpath(rootPath).catch((error) => {
    throw new Error(`unable to resolve source project root ${rootPath}: ${error.message}`);
  });
  const markerPath = join(root, ROOT_MARKER);
  const markerStats = await lstat(markerPath).catch((error) => {
    throw new Error(`source project root marker is missing at ${markerPath}: ${error.message}`);
  });
  if (!markerStats.isFile() || markerStats.isSymbolicLink()) {
    throw new Error(`source project root marker must be a regular file: ${markerPath}`);
  }

  const manifest = await readJson(markerPath, "source project root marker");
  if (manifest.schemaVersion !== SOURCE_PROJECT_SCHEMA_VERSION) {
    throw new Error(`unsupported source project root schema version: ${manifest.schemaVersion}`);
  }
  if (manifest.kind !== SOURCE_PROJECT_KIND) {
    throw new Error(`invalid source project root kind: ${manifest.kind}`);
  }
  if (manifest.projectId !== SOURCE_PROJECT_ID) {
    throw new Error(`unexpected source project id: ${manifest.projectId}`);
  }

  const workspaceManifest = await requireSourceFile(root, manifest.workspaceManifest);
  const workspaceText = await readFile(workspaceManifest, "utf8");
  if (!workspaceText.split(/\r?\n/u).some((line) => line.trim() === "[workspace]")) {
    throw new Error(`workspace manifest does not declare [workspace]: ${workspaceManifest}`);
  }

  if (!Array.isArray(manifest.lockfiles) || manifest.lockfiles.length === 0) {
    throw new Error("source project root manifest must declare lockfiles");
  }
  const lockfiles = new Set();
  for (const lockfile of manifest.lockfiles) {
    if (lockfiles.has(lockfile)) {
      throw new Error(`source project root manifest contains duplicate lockfile: ${lockfile}`);
    }
    lockfiles.add(lockfile);
    await requireSourceFile(root, lockfile);
  }
  for (const required of REQUIRED_LOCKFILES) {
    if (!lockfiles.has(required)) {
      throw new Error(`source project root manifest is missing required lockfile: ${required}`);
    }
  }

  const resourceManifestPath = await requireSourceFile(root, manifest.resourceManifest);
  const resourceManifest = await readJson(resourceManifestPath, "source resource manifest");
  if (resourceManifest.schemaVersion !== SOURCE_PROJECT_SCHEMA_VERSION) {
    throw new Error(
      `unsupported source resource manifest schema version: ${resourceManifest.schemaVersion}`,
    );
  }
  if (resourceManifest.projectId !== manifest.projectId) {
    throw new Error(`source resource manifest project id mismatch: ${resourceManifest.projectId}`);
  }

  return Object.freeze({ root, manifest: Object.freeze(manifest) });
}

export async function safeProjectJoin(rootPath, relativePath) {
  const root = await realpath(rootPath).catch((error) => {
    throw new Error(`unable to resolve project root ${rootPath}: ${error.message}`);
  });
  if (
    typeof relativePath !== "string" ||
    relativePath.length === 0 ||
    isAbsolute(relativePath) ||
    relativePath.split(/[\\/]+/u).some((component) => component === "..")
  ) {
    throw new Error(`path must be a non-empty portable project-relative path: ${relativePath}`);
  }

  const candidate = resolve(root, relativePath);
  assertInside(root, candidate, `project path escapes root: ${relativePath}`);
  let existingAncestor = candidate;
  while (!existsSync(existingAncestor)) {
    const parent = dirname(existingAncestor);
    if (parent === existingAncestor) {
      throw new Error(`project path has no existing in-root ancestor: ${relativePath}`);
    }
    existingAncestor = parent;
  }
  const canonicalAncestor = await realpath(existingAncestor);
  assertInside(root, canonicalAncestor, `project path escapes through an external link: ${relativePath}`);
  if (existsSync(candidate)) {
    const canonicalCandidate = await realpath(candidate);
    assertInside(root, canonicalCandidate, `project path resolves outside root: ${relativePath}`);
  }
  return candidate;
}

async function requireSourceFile(root, relativePath) {
  const path = await safeProjectJoin(root, relativePath);
  const metadata = await lstat(path).catch((error) => {
    throw new Error(`required source project file is missing at ${path}: ${error.message}`);
  });
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`required source project path must be a regular file: ${path}`);
  }
  return path;
}

async function readJson(path, label) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    throw new Error(`invalid ${label} ${path}: ${error.message}`);
  }
}

function assertInside(root, candidate, message) {
  const relation = relative(root, candidate);
  if (relation === "" || (!relation.startsWith("..") && !isAbsolute(relation))) {
    return;
  }
  throw new Error(message);
}
