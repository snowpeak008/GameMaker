#!/usr/bin/env python3
"""Update AI memory freshness hashes.

Usage:
    python tools/memory/update_freshness.py
"""

from __future__ import annotations

import hashlib
import json
from datetime import datetime
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[2]
MEMORY_ROOT = PROJECT_ROOT / "knowledge" / "ai_memory"
CONFIG_PATH = MEMORY_ROOT / "project_understanding" / "memory_config.json"
FRESHNESS_PATH = MEMORY_ROOT / "project_understanding" / "freshness.json"


DEFAULT_CONFIG = {
    "key_files": ["AI_README.md", "AGENTS.md"],
    "ignore_missing": True,
}


def read_config() -> dict[str, Any]:
    if not CONFIG_PATH.exists():
        return DEFAULT_CONFIG
    return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))


def compute_file_hash(path: Path) -> dict[str, str | int]:
    content = path.read_bytes()
    return {
        "sha256": hashlib.sha256(content).hexdigest(),
        "size": path.stat().st_size,
    }


def main() -> int:
    config = read_config()
    key_files = config.get("key_files", DEFAULT_CONFIG["key_files"])
    ignore_missing = bool(config.get("ignore_missing", True))

    files_data: dict[str, dict[str, str | int]] = {}
    missing: list[str] = []

    for rel_path in key_files:
        full_path = PROJECT_ROOT / rel_path
        if full_path.exists() and full_path.is_file():
            files_data[rel_path] = compute_file_hash(full_path)
        else:
            missing.append(rel_path)

    output = {
        "generated_at": datetime.now().isoformat(timespec="seconds"),
        "files": files_data,
    }

    FRESHNESS_PATH.parent.mkdir(parents=True, exist_ok=True)
    FRESHNESS_PATH.write_text(
        json.dumps(output, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    print(f"[OK] Updated freshness.json with {len(files_data)} files")
    if missing:
        print(f"[WARN] Missing files: {', '.join(missing)}")
        if not ignore_missing:
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
