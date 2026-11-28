"""Reverse spoken-to-written normalization (ITN) for speech-to-text transcripts."""
from __future__ import annotations

import calendar
import re
from typing import List, Optional, Tuple

_UNITS = {
    "zero": 0, "one": 1, "two": 2, "three": 3, "four": 4, "five": 5,
    "six": 6, "seven": 7, "eight": 8, "nine": 9, "ten": 10,
    "eleven": 11, "twelve": 12, "thirteen": 13, "fourteen": 14, "fifteen": 15,
    "sixteen": 16, "seventeen": 17, "eighteen": 18, "nineteen": 19,
}
_TENS = {
    "twenty": 20, "thirty": 30, "forty": 40, "fifty": 50,
    "sixty": 60, "seventy": 70, "eighty": 80, "ninety": 90,
}
_SCALES = {"hundred": 100, "thousand": 1_000, "million": 1_000_000, "billion": 1_000_000_000}
_ORDINALS = {
    "first": 1, "second": 2, "third": 3, "fourth": 4, "fifth": 5,
    "sixth": 6, "seventh": 7, "eighth": 8, "ninth": 9, "tenth": 10,
    "eleventh": 11, "twelfth": 12, "thirteenth": 13, "fourteenth": 14,
    "fifteenth": 15, "sixteenth": 16, "seventeenth": 17, "eighteenth": 18,
    "nineteenth": 19, "twentieth": 20, "thirtieth": 30,
}
_MONTHS = (
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
)
_MONTH_MAP = {name.lower(): idx for idx, name in enumerate(_MONTHS, 1)}
_NUM_WORDS = set(_UNITS) | set(_TENS) | set(_SCALES)
_SINGLE_DIGIT = {**_UNITS, "oh": 0, "o": 0}
_UNIT_NOUNS = (
    "seconds?|secs?|minutes?|mins?|hours?|hrs?|weeks?|months?|years?|"
    "blocks?|rules?|apps?|sites?|passes?|percent"
)


def _split_tokens(raw: str) -> List[str]:
    return [tok for part in raw.lower().split() for tok in part.split("-") if tok]


def _words_to_int(tokens: List[str]) -> Optional[int]:
    """Parse canonical English cardinal number tokens into an integer."""
    clean = [t for t in tokens if t != "and"]
    if not clean or any(t not in _NUM_WORDS for t in clean):
        return None
    if not any(t in _UNITS or t in _TENS for t in clean):
        return None
    total, current = 0, 0
    for tok in clean:
        if tok in _UNITS:
            current += _UNITS[tok]
        elif tok in _TENS:
            current += _TENS[tok]
        elif tok == "hundred":
            current = (current or 1) * 100
        else:
            scale = _SCALES[tok]
            total += (current or 1) * scale
            current = 0
    return total + current


def _ordinal_to_int(raw: str) -> Optional[int]:
    """Parse spoken ordinal or cardinal day words (`third`, `twenty-first`, `21st`)."""
    text = raw.strip().lower()
    digits = re.fullmatch(r"(\d{1,2})(?:st|nd|rd|th)?", text)
    if digits:
        return int(digits.group(1))
    tokens = _split_tokens(text)
    if not tokens:
        return None
    if len(tokens) == 1:
        return _ORDINALS.get(tokens[0], _UNITS.get(tokens[0], _TENS.get(tokens[0])))
    if len(tokens) == 2 and tokens[0] in _TENS:
        tail = _ORDINALS.get(tokens[1], _UNITS.get(tokens[1]))
        if tail is not None and 1 <= tail <= 9:
            return _TENS[tokens[0]] + tail
    return None


def _parse_year_prefix(text: str) -> Optional[Tuple[int, int]]:
    """Parse a leading 4-digit spoken year expression and return `(year, chars_consumed)`."""
    numeric = re.match(r"(\d{4})\b", text)
    if numeric:
        return int(numeric.group(1)), numeric.end()

    word_matches = list(re.finditer(r"[A-Za-z]+(?:-[A-Za-z]+)*", text))
    valid_prefix: List[re.Match] = []
    for match in word_matches:
        sub = _split_tokens(match.group(0))
        if not sub or any(t not in (_NUM_WORDS | {"and", "oh", "o"}) for t in sub):
            break
        if valid_prefix and any(w in text[valid_prefix[-1].end():match.start()] for w in ("\n", ",", ".")):
            break
        valid_prefix.append(match)

    for end_idx in range(len(valid_prefix), 0, -1):
        span_end = valid_prefix[end_idx - 1].end()
        tokens = [t for m in valid_prefix[:end_idx] for t in _split_tokens(m.group(0))]
        year = _tokens_to_year(tokens)
        if year is not None:
            return year, span_end
    return None


def _tokens_to_year(tokens: List[str]) -> Optional[int]:
    if not tokens or tokens[0] in ("and", "oh", "o"):
        return None
    val = _words_to_int(tokens)
    if val is not None and 1000 <= val <= 2999:
        return val
    # Split century + decade forms: `twenty twenty-six` -> 2026, `twenty oh six` -> 2006.
    century = _UNITS.get(tokens[0], _TENS.get(tokens[0]))
    if century is None or not 10 <= century <= 29 or len(tokens) < 2:
        return None
    rest = tokens[1:]
    if rest[0] in ("oh", "o"):
        sub = _words_to_int(rest[1:])
        if sub is not None and 1 <= sub <= 9:
            return century * 100 + sub
        return None
    sub = _words_to_int(rest)
    if sub is not None and 10 <= sub <= 99:
        return century * 100 + sub
    return None


def _dates_to_written(text: str) -> str:
    """Convert spoken `<Month> <day>[, <year>]` phrases into `<Month> <D>[, <YYYY>]`."""
    names = "|".join(_MONTHS)
    day_pat = (
        r"(?:twenty|thirty)[-\s]+(?:first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|"
        r"one|two|three|four|five|six|seven|eight|nine)|"
        r"(?:twent|thirt)ieth|"
        r"(?:first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth|"
        r"eleventh|twelfth|thirteenth|fourteenth|fifteenth|sixteenth|seventeenth|eighteenth|nineteenth)|"
        r"thirty|twenty|ten|"
        r"one|two|three|four|five|six|seven|eight|nine|"
        r"\d{1,2}(?:st|nd|rd|th)"
    )
    pattern = re.compile(rf"(?i)\b(?P<month>{names})\s+(?P<day>{day_pat})\b")
    out: List[str] = []
    cursor = 0
    for match in pattern.finditer(text):
        out.append(text[cursor:match.start()])
        raw_month = match["month"]
        raw_day = match["day"].strip().lower()
        # Avoid false positives on verbs/adjectives (`we may two`, `march four miles`) unless ordinal or Title Case.
        is_ordinal_day = bool(re.search(r"(?:st|nd|rd|th)$", raw_day)) or any(
            t in _ORDINALS for t in _split_tokens(raw_day)
        )
        if raw_month.lower() in ("may", "march", "august") and raw_month[0].islower() and not is_ordinal_day:
            out.append(match.group(0))
            cursor = match.end()
            continue
        month = _MONTH_MAP[raw_month.lower()]
        day = _ordinal_to_int(match["day"])
        if day is None or not 1 <= day <= 31:
            out.append(match.group(0))
            cursor = match.end()
            continue

        tail = text[match.end():]
        sep = re.match(r"(\s*,\s*|\s+)", tail)
        year_match: Optional[Tuple[int, int]] = None
        if sep:
            parsed = _parse_year_prefix(tail[sep.end():])
            if parsed is not None:
                year, consumed = parsed
                max_day = calendar.monthrange(year, month)[1]
                if 1 <= day <= max_day:
                    out.append(f"{_MONTHS[month - 1]} {day}, {year}")
                    cursor = match.end() + sep.end() + consumed
                    continue
        if year_match is None and day <= calendar.monthrange(2026, month)[1]:
            out.append(f"{_MONTHS[month - 1]} {day}")
            cursor = match.end()
            continue
        out.append(match.group(0))
        cursor = match.end()
    out.append(text[cursor:])
    return "".join(out)


def _minute_words_to_int(raw: Optional[str]) -> Optional[int]:
    if not raw:
        return None
    tokens = _split_tokens(raw)
    if len(tokens) == 2 and tokens[0] in ("oh", "o") and tokens[1] in _UNITS and 1 <= _UNITS[tokens[1]] <= 9:
        return _UNITS[tokens[1]]
    val = _words_to_int(tokens)
    return val if val is not None and 10 <= val <= 59 else None


def _format_meridiem(raw: Optional[str]) -> str:
    if not raw:
        return ""
    return " a.m." if raw.lower().startswith("a") else " p.m."


def _times_to_written(text: str) -> str:
    """Convert spoken clock times (`nine thirty AM`, `at nine oh five`, `nine o'clock`) to digits."""
    hour_words = "one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve"
    min_tens = "ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|" \
               "twenty|thirty|forty|fifty"
    min_ones = "one|two|three|four|five|six|seven|eight|nine"
    min_pat = rf"(?:(?:oh|o)\s+(?:{min_ones})|(?:twenty|thirty|forty|fifty)[-\s]+(?:{min_ones})|(?:{min_tens}))"
    meridiem_pat = r"(?P<mer>[ap]\.?\s*m\.?)"

    with_mer = re.compile(
        rf"(?i)\b(?P<hour>{hour_words})(?:\s+(?P<min>{min_pat}))?\s+{meridiem_pat}(?!\w)"
    )
    def replace_mer(match: re.Match) -> str:
        hour = _UNITS[match["hour"].lower()]
        minute = _minute_words_to_int(match["min"])
        if match["min"] and minute is None:
            return match.group(0)
        suffix = _format_meridiem(match["mer"])
        if minute is None:
            return f"{hour}{suffix}"
        return f"{hour}:{minute:02d}{suffix}"
    text = with_mer.sub(replace_mer, text)

    oclock = re.compile(rf"(?i)\b(?P<hour>{hour_words})\s+o['’]?clock\b(?:\s*{meridiem_pat})?")
    text = oclock.sub(
        lambda m: f"{_UNITS[m['hour'].lower()]}:00{_format_meridiem(m['mer'])}",
        text,
    )

    cued_hm = re.compile(
        rf"(?i)(?P<cue>\b(?:at|from|to|until|till|before|after|around|by|between)\s+)"
        rf"(?P<hour>{hour_words})\s+(?P<min>{min_pat})\b"
    )
    def replace_cued(match: re.Match) -> str:
        hour = _UNITS[match["hour"].lower()]
        minute = _minute_words_to_int(match["min"])
        if minute is None:
            return match.group(0)
        return f"{match['cue']}{hour}:{minute:02d}"
    return cued_hm.sub(replace_cued, text)


def _numbers_to_written(text: str) -> str:
    """Convert spoken decimals, unit-bound numbers, and multi-digit cardinals to digits."""
    num_tok = (
        r"(?:zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|"
        r"thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|"
        r"twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety|hundred|thousand|million|billion)"
    )
    phrase = rf"{num_tok}(?:(?:[-\s]+(?:and\s+)?){num_tok})*"

    # Decimals: `three point five` -> `3.5`
    dec = re.compile(
        rf"(?i)\b(?P<whole>{phrase})\s+point\s+(?P<frac>(?:(?:zero|oh|o|one|two|three|four|five|six|seven|eight|nine)\b\s*)+)"
    )
    def replace_dec(match: re.Match) -> str:
        whole = _words_to_int(_split_tokens(match["whole"]))
        frac_tokens = _split_tokens(match["frac"])
        if whole is None or not frac_tokens or any(t not in _SINGLE_DIGIT for t in frac_tokens):
            return match.group(0)
        digits = "".join(str(_SINGLE_DIGIT[t]) for t in frac_tokens)
        return f"{whole}.{digits}" + (" " if match.group(0).endswith(" ") else "")
    text = dec.sub(replace_dec, text)

    # Cardinals with optional trailing unit noun
    card = re.compile(rf"(?i)\b(?P<num>{phrase})(?P<unit>\s+(?:{_UNIT_NOUNS})\b)?")
    def replace_card(match: re.Match) -> str:
        raw_num = match["num"]
        unit = match["unit"] or ""
        tokens = _split_tokens(raw_num)
        val = _words_to_int(tokens)
        if val is None:
            return match.group(0)
        # Leave ambiguous single small words (`one`, `two`, ..., `nine`) alone without a unit noun.
        if not unit and val < 10 and len(tokens) == 1:
            return match.group(0)
        return f"{val}{unit}"
    return card.sub(replace_card, text)


def _capitalize_sentences(text: str) -> str:
    """Uppercase lowercase sentence starts while leaving domains, abbreviations, and mixed case intact."""
    pattern = re.compile(r"(^|(?<![ap]\.m)(?<![ap]\.M)[.!?]\s+)([a-z][a-zA-Z0-9'-]*)(\.[a-z]{2,})?")
    def fix_case(match: re.Match) -> str:
        prefix, word, tld = match.group(1), match.group(2), match.group(3)
        if tld:
            return match.group(0)
        return prefix + word[0].upper() + word[1:]
    return pattern.sub(fix_case, text)


def normalize_written_text(text: str) -> str:
    """Convert spoken-form dates, times, numbers, and sentence starts into written form."""
    if not text or not text.strip():
        return text or ""
    cleaned = _dates_to_written(text.strip())
    cleaned = _times_to_written(cleaned)
    cleaned = _numbers_to_written(cleaned)
    return _capitalize_sentences(cleaned)
