# Standalone Project Boundary

This repository is the authoritative source for AutoDesignMaker Rust V2. It does not
load build, test, protocol, or product data from its parent directory.

## Root contracts

- Source checkouts are identified by `.project_root`, `Cargo.toml`, and
  `knowledge/resource-manifest.json`.
- Portable builds are identified separately by `build-manifest.json`; they do not
  require Cargo workspace files.
- Runtime user data belongs in the portable `user_data` directory or the Tauri
  application-data directory. It never belongs in the source tree or Git history.

## Resource ownership

The tracked `knowledge` and `pipeline/artifact_layer` directories were imported once
when the V2 project was separated. They are now maintained only here. SDK and Skill
files in `knowledge` are read-only seeds; user additions belong to the runtime data
root.

## Supported relocation

A clean Git clone must build without any sibling project. A complete portable output
may be copied to any writable local Windows directory. Read-only locations, network
shares, cloud-synchronized folders, and reparse-point roots are not supported by the
portable data contract.

## Generated-file lifecycle

Cargo targets, Web output, dependency caches, gate screenshots/reports, packaging
staging directories, and test workspaces are generated data. Use the guarded cleanup
tool after each verification phase. The tool must never remove real `user_data`, the
source `.git`, tracked resources, or a portable recovery backup.

## Auditable inventories

- `source-boundary-inventory.json` freezes the imported authoritative resource trees
  and the protected local-data baseline without recording a machine-specific root.
- `persistent-path-registry.json` classifies every persisted path as project-owned,
  machine-bound, or display-only. Project-owned paths are portable relative paths;
  machine bindings must be revalidated after relocation.
