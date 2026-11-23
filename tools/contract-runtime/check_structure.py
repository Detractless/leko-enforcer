#!/usr/bin/env python3
"""Fail on oversized files or directories in Leko source and test trees."""

from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
SCAN_ROOTS = ["core", "cli", "voice", "ui", "enforcers", "platform", "installer", "tests", "src/daemon"]
SKIP_DIRS = {"__pycache__", "node_modules", "target", ".git", "fixtures"}
CODE_SUFFIXES = {".py", ".rs", ".js", ".mjs", ".ts"}
MAX_LINES = 350
MAX_FILES = 8


def main() -> int:
    failures = []
    for rel in SCAN_ROOTS:
        base = ROOT / rel
        if not base.exists():
            continue
        for folder in [base, *[p for p in base.rglob("*") if p.is_dir()]]:
            if any(part in SKIP_DIRS for part in folder.parts):
                continue
            if folder.name == "tests" and folder.parent.name in {"core", "cli"}:
                continue
            files = [p for p in folder.iterdir() if p.is_file()]
            if len(files) > MAX_FILES:
                failures.append(f"directory {folder.relative_to(ROOT)} has {len(files)} files (max {MAX_FILES})")
            for path in files:
                if path.suffix not in CODE_SUFFIXES:
                    continue
                try:
                    lines = len(path.read_text(encoding="utf-8").splitlines())
                except UnicodeDecodeError:
                    continue
                if lines >= MAX_LINES:
                    failures.append(f"file {path.relative_to(ROOT)} has {lines} lines (max {MAX_LINES - 1})")
    if failures:
        print("Structure check failed:")
        for failure in failures:
            print(" -", failure)
        return 1
    print("Structure check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
