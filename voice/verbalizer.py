"""Text normalization and verbalizer for Maeve speech synthesis.

Transforms Markdown, tables, abbreviations, and symbols into natural,
fluent conversational English without modifying the underlying raw model text.
"""
from __future__ import annotations

import re
from typing import List

try:
    from .normalization.speech_text import normalize_spoken_text, protect_speech_literals, restore_speech_literals
except ImportError:
    from normalization.speech_text import normalize_spoken_text, protect_speech_literals, restore_speech_literals

def _flatten_markdown_tables(text: str) -> str:
    """Converts markdown table lines into fluent spoken sentences."""
    lines = text.split("\n")
    cleaned_lines: List[str] = []
    table_rows: List[List[str]] = []

    def flush_table():
        nonlocal table_rows
        if not table_rows:
            return
        # If header + separator + content
        if len(table_rows) >= 2:
            headers = [h.strip() for h in table_rows[0]]
            data_rows = table_rows[1:]
            # Filter out separator row e.g. |---|---|
            if all(re.match(r"^:?-+:?$", cell.strip()) for cell in data_rows[0]):
                data_rows = data_rows[1:]

            for row in data_rows:
                items = []
                for h, val in zip(headers, row):
                    v = val.strip()
                    if v:
                        items.append(f"{h}: {v}" if h else v)
                if items:
                    cleaned_lines.append(", ".join(items) + ".")
        table_rows = []

    for line in lines:
        stripped = line.strip()
        if stripped.startswith("|") and stripped.endswith("|"):
            cells = [c.strip() for c in stripped.strip("|").split("|")]
            table_rows.append(cells)
        else:
            flush_table()
            cleaned_lines.append(line)

    flush_table()
    return "\n".join(cleaned_lines)

def _speech_structure(text: str) -> str:
    """Tables, markdown artifacts, URLs and domain names."""
    t = _flatten_markdown_tables(text)
    t = re.sub(r"```[\w-]*\n?", "", t)
    t = t.replace("```", "")
    t = re.sub(r"`([^`]+)`", r"\1", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"\1", t)
    t = re.sub(r"\*([^*]+)\*", r"\1", t)
    t = re.sub(r"__([^_]+)__", r"\1", t)
    t = re.sub(r"_([^_]+)_", r"\1", t)
    t = re.sub(r"^\s*#{1,6}\s*", "", t, flags=re.MULTILINE)
    t = re.sub(r"^[-*+]\s+", "", t, flags=re.MULTILINE)
    t = t.replace("→", " meaning ").replace("->", " meaning ")
    t = re.sub(r"[\[\]{}<>]", " ", t)
    t = re.sub(r"#+", "", t)
    t = t.replace("\\", " ")
    t = t.replace("|", " ")
    t = re.sub(r"https?://(?:www\.)?([a-zA-Z0-9.-]+)(?:/[^\s]*)?", r"\1", t)
    t = re.sub(r"\bwww\.([a-zA-Z0-9.-]+)", r"\1", t)
    t = re.sub(r"\b([a-zA-Z0-9-]+)\.(com|org|net|io|edu|gov|app|dev)\b", r"\1 dot \2", t, flags=re.IGNORECASE)
    return t


def _speech_cadence(text: str) -> str:
    """Em-dashes, emojis, spacing and punctuation cadence."""
    t = re.sub(r"\s*[—–]\s*", "... ", text)
    t = re.sub(r"\s*--\s*", "... ", t)
    t = re.sub(r"(?<=[A-Za-z]):\s+", "... ", t)
    t = re.sub(r"(?i)\ba\.m\.", "\ue300", t)
    t = re.sub(r"(?i)\bp\.m\.", "\ue301", t)
    t = re.sub(r"[\U00010000-\U0010ffff]", "", t)
    t = re.sub(r"([!?,.:;])([A-Za-z])", r"\1 \2", t)
    t = re.sub(r"[ \t]+", " ", t)
    t = re.sub(r"\s+([.,;:!?])", r"\1", t)
    t = re.sub(r"\n+", " ", t)
    t = t.replace("\ue300", "a.m.").replace("\ue301", "p.m.")
    return t


def verbalize_for_speech(text: str) -> str:
    """Transforms raw LLM output into fluent, human-ear friendly spoken English."""
    if not text:
        return ""
    try:
        masked, saved = protect_speech_literals(text)
        spoken = _speech_structure(masked)
        spoken = normalize_spoken_text(spoken)
        return restore_speech_literals(_speech_cadence(spoken), saved).strip()
    except Exception:
        return text
