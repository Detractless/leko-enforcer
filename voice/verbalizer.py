"""Text normalization and verbalizer for Maeve speech synthesis.

Transforms Markdown, tables, abbreviations, and symbols into natural,
fluent conversational English without modifying the underlying raw model text.
"""
from __future__ import annotations

import re
from typing import List

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

def verbalize_for_speech(text: str) -> str:
    """Transforms raw LLM output into fluent, human-ear friendly spoken English."""
    if not text:
        return ""

    t = text

    # 1. Tables and structure
    t = _flatten_markdown_tables(t)

    # 2. Markdown artifacts
    t = re.sub(r"```[\w-]*\n?", "", t)
    t = t.replace("```", "")
    t = re.sub(r"`([^`]+)`", r"\1", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"\1", t)
    t = re.sub(r"\*([^*]+)\*", r"\1", t)
    t = re.sub(r"__([^_]+)__", r"\1", t)
    t = re.sub(r"_([^_]+)_", r"\1", t)
    # Strip markdown headers (e.g. #, ##, ###)
    t = re.sub(r"^\s*#{1,6}\s*", "", t, flags=re.MULTILINE)
    t = re.sub(r"^[-*+]\s+", "", t, flags=re.MULTILINE)
    # Replace symbols and brackets that vocoders speak aloud
    t = t.replace("→", " meaning ").replace("->", " meaning ")
    t = re.sub(r"[\[\]{}<>]", " ", t)
    t = re.sub(r"#+", "", t)
    t = t.replace("\\", " ")
    t = t.replace("|", " ")

    # 3. URLs
    t = re.sub(r"https?://(?:www\.)?([a-zA-Z0-9.-]+)(?:/[^\s]*)?", r"\1", t)
    t = re.sub(r"\bwww\.([a-zA-Z0-9.-]+)", r"\1", t)

    # 4. Domain names (e.g. reddit.com -> reddit dot com)
    t = re.sub(
        r"\b([a-zA-Z0-9-]+)\.(com|org|net|io|edu|gov|app|dev)\b",
        r"\1 dot \2",
        t,
        flags=re.IGNORECASE
    )

    # 5. Latin abbreviations and conversational acronyms
    t = re.sub(r"\be\.g\.,?\s*", "for example, ", t, flags=re.IGNORECASE)
    t = re.sub(r"\bi\.e\.,?\s*", "that is, ", t, flags=re.IGNORECASE)
    t = re.sub(r"\betc\b\.?", "and more", t, flags=re.IGNORECASE)
    t = re.sub(r"\bvs\b\.?", "versus", t, flags=re.IGNORECASE)
    t = re.sub(r"\bw/o\b", "without", t, flags=re.IGNORECASE)
    t = re.sub(r"\bw/(?=\s|$)", "with", t, flags=re.IGNORECASE)
    t = re.sub(r"\band\s*/\s*or\b", "and or", t, flags=re.IGNORECASE)
    # A single slash between words commonly means alternatives. Leave paths,
    # dates, fractions, and multi-part slash expressions intact.
    t = re.sub(r"(?<![\w/])([A-Za-z]+)\s*/\s*([A-Za-z]+)(?![\w/])",
               r"\1 and or \2", t)

    # 6. Units and durations
    def _hours(m):
        n = int(m.group(1))
        return f"{n} hour" if n == 1 else f"{n} hours"
    t = re.sub(r"\b(\d+)\s*(?:h|hr|hrs)\b", _hours, t, flags=re.IGNORECASE)

    def _minutes(m):
        n = int(m.group(1))
        return f"{n} minute" if n == 1 else f"{n} minutes"
    t = re.sub(r"\b(\d+)\s*(?:m|min|mins)\b", _minutes, t, flags=re.IGNORECASE)

    def _seconds(m):
        n = int(m.group(1))
        return f"{n} second" if n == 1 else f"{n} seconds"
    t = re.sub(r"\b(\d+)\s*(?:s|sec|secs)\b", _seconds, t, flags=re.IGNORECASE)

    # 7. Em-dashes and acoustic breathing pauses
    t = re.sub(r"\s*[—–]\s*", "... ", t)
    t = re.sub(r"\s*--\s*", "... ", t)

    # 8. Strip emojis that confuse the acoustic vocoder
    t = re.sub(r"[\U00010000-\U0010ffff]", "", t)

    # 9. Cadence and sentence boundary spacing
    t = re.sub(r"([!?,.:;])([A-Za-z])", r"\1 \2", t)

    # 10. Clean up whitespace and punctuation
    t = re.sub(r"[ \t]+", " ", t)
    t = re.sub(r"\s+([.,;:!?])", r"\1", t)
    t = re.sub(r"\n+", " ", t)
    return t.strip()
