"""Explicit local facts with bounded lexical retrieval; no transcript storage."""
from contextlib import contextmanager
import os
import re
import sqlite3
from pathlib import Path

STOP_WORDS = set("i my me the a an is are to for of and what do does please remember that it you about".split())


def terms(text):
    return set(re.findall(r"[\w]+", text.casefold())) - STOP_WORDS


class LocalMemory:
    def __init__(self, path=None):
        self.path = Path(path or os.environ.get(
            "LEKO_MEMORY_PATH", "~/Library/Application Support/Leko/memory.sqlite3")).expanduser()

    @contextmanager
    def _connect(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        db = sqlite3.connect(self.path)
        os.chmod(self.path, 0o600)
        db.execute("PRAGMA secure_delete = ON")
        db.execute("CREATE TABLE IF NOT EXISTS facts (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        try:
            with db:
                yield db
        finally:
            db.close()

    def command(self, text):
        match = re.fullmatch(r"(remember|correct|forget)\s+(.+)", text.strip(), re.I)
        if not match:
            return None
        verb, body = match.groups()
        verb = verb.lower()
        if verb == "forget":
            key = body.strip().casefold()
            with self._connect() as db:
                if key == "all":
                    db.execute("DELETE FROM facts")
                    return "Forgot all saved memories."
                changed = db.execute("DELETE FROM facts WHERE key = ?", (key,)).rowcount
            return "Forgot that memory." if changed else "No saved memory has that key."
        parts = re.split(r"\s*(?:=|\s+(?:is|equals)\s+)\s*", body, maxsplit=1, flags=re.I)
        if len(parts) != 2 or not all(part.strip() for part in parts):
            return "Use remember key equals value, written as key = value, or key is value."
        key, value = parts[0].strip().casefold(), parts[1].strip()
        if len(key) > 80 or len(value) > 320:
            return "Memory keys must fit 80 characters and values 320 characters."
        with self._connect() as db:
            exists = db.execute("SELECT 1 FROM facts WHERE key = ?", (key,)).fetchone()
            if verb == "correct" and not exists:
                return "No saved memory has that key. Use remember to add it."
            if not exists and db.execute("SELECT COUNT(*) FROM facts").fetchone()[0] >= 200:
                return "Memory is full. Forget a saved memory first."
            db.execute("INSERT OR REPLACE INTO facts VALUES (?, ?)", (key, value))
        return "Memory corrected." if exists else "Memory saved."

    def retrieve(self, query):
        if not self.path.exists():
            return []
        wanted = terms(query)
        with self._connect() as db:
            rows = db.execute("SELECT key, value FROM facts ORDER BY key").fetchall()
        ranked = [(len(wanted & terms(key + " " + value)), key, value) for key, value in rows]
        ranked.sort(key=lambda row: (-row[0], row[1]))
        return [f"{key}: {value}" for score, key, value in ranked if score][:3]
