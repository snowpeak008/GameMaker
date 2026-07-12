# Artifact Layer

This directory defines the upper runtime contract for the migrated 0-14 pipeline.

- `registry.json` declares stage artifacts, tasks, reviewers, validators, dependencies, and knowledge references.
- `dependency_graph.json` is generated from the registry and kept deterministic.

The layer wraps the existing `steps/` modules. It does not replace their `run(context)` API.
