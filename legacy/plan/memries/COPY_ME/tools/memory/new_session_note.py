#!/usr/bin/env python3
"""Create the next AI memory session note from a template.

Usage:
    python tools/memory/new_session_note.py
"""

from __future__ import annotations

import json
from datetime import date
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[2]
SESSION_DIR = PROJECT_ROOT / "knowledge" / "ai_memory" / "session_history"
SESSION_INDEX = SESSION_DIR / "index.json"


def next_session_id() -> str:
    today = date.today().isoformat()
    existing = sorted(SESSION_DIR.glob(f"{today}-*.md"))
    next_number = len(existing) + 1
    return f"{today}-{next_number:03d}"


def main() -> int:
    SESSION_DIR.mkdir(parents=True, exist_ok=True)
    session_id = next_session_id()
    path = SESSION_DIR / f"{session_id}.md"

    if path.exists():
        print(f"[ERROR] Session note already exists: {path}")
        return 1

    path.write_text(
        f"""# {session_id}

## 摘要

待填写。

## 完成内容

- [ ] 待填写

## 验证

- [ ] 待填写

## 后续关注

- [ ] 待填写
""",
        encoding="utf-8",
    )

    index_data = {"sessions": []}
    if SESSION_INDEX.exists():
        index_data = json.loads(SESSION_INDEX.read_text(encoding="utf-8"))
    sessions = index_data.setdefault("sessions", [])
    sessions.append({"id": session_id, "file": f"{session_id}.md", "summary": "待填写"})
    SESSION_INDEX.write_text(
        json.dumps(index_data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

