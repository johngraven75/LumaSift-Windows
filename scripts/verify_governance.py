#!/usr/bin/env python3
"""Fail closed when required LumaSift engineering-governance artifacts are missing."""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_FILES = (
    "AGENTS.md",
    ".github/AUTOMATION_POLICY.md",
    ".github/copilot-instructions.md",
    ".github/MASTER_ENGINEER_STANDARD.md",
    "README.md",
    "docs/ARCHITECTURE.md",
    "docs/SECURITY.md",
    "RELEASE_NOTES.md",
)
MASTER_STANDARD_TOKENS = (
    "exact-content proof",
    "full SHA-256 hashing",
    "recoverable quarantine",
    "permanent purge",
    "User-selected file categories",
    "Remote or companion clients must not disclose raw local source paths",
    "Mandatory Evidence Before Publication",
    "Completion Report",
)


def main() -> int:
    errors: list[str] = []
    for relative_path in REQUIRED_FILES:
        if not (ROOT / relative_path).is_file():
            errors.append(f"missing required governance artifact: {relative_path}")

    master_path = ROOT / ".github/MASTER_ENGINEER_STANDARD.md"
    if master_path.is_file():
        standard = master_path.read_text(encoding="utf-8")
        for token in MASTER_STANDARD_TOKENS:
            if token not in standard:
                errors.append(f"master engineering standard missing required policy: {token}")

    if errors:
        print("LumaSift governance verification failed:")
        for error in errors:
            print(f" - {error}")
        return 1

    print("LumaSift governance verification passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
