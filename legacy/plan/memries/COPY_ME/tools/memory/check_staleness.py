#!/usr/bin/env python3
"""Check whether AI memory cached files are stale.

Usage:
    python tools/memory/check_staleness.py
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[2]
FRESHNESS_PATH = (
    PROJECT_ROOT / "knowledge" / "ai_memory" / "project_understanding" / "freshness.json"
)


def compute_file_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    if not FRESHNESS_PATH.exists():
        print(json.dumps({"error": "freshness.json not found"}, ensure_ascii=False))
        return 1

    data = json.loads(FRESHNESS_PATH.read_text(encoding="utf-8"))
    cached_files = data.get("files", {})

    stale: list[str] = []
    fresh: list[str] = []
    missing: list[str] = []

    for rel_path, cached_info in cached_files.items():
        full_path = PROJECT_ROOT / rel_path
        if not full_path.exists():
            missing.append(rel_path)
            continue

        current_hash = compute_file_hash(full_path)
        cached_hash = cached_info.get("sha256")
        if current_hash != cached_hash:
            stale.append(rel_path)
        else:
            fresh.append(rel_path)

    result = {
        "stale": stale,
        "fresh": fresh,
        "missing": missing,
        "generated_at": data.get("generated_at"),
    }

    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 1 if stale or missing else 0


if __name__ == "__main__":
    raise SystemExit(main())

