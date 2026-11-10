"""Deterministic, conservative English normalization for TTS-only text."""
from __future__ import annotations

import calendar
import re

_PROTECTED = [
    re.compile(r"```[\s\S]*?```"), re.compile(r"`[^`\n]*`"),
    re.compile(r"(?i)\b(?:https?://|www\.)[^\s<>\"']+"),
    re.compile(r"(?<![\w.+-])[\w.+-]+@[\w-]+(?:\.[\w-]+)+(?![\w.-])"),
    re.compile(r"(?<!\w)(?:[A-Za-z]:\\|\\)[^\s,;!?<>\"]+"),
    re.compile(r"(?<!\w)(?:\.\.?/)?(?:[A-Za-z0-9._~+-]+/)+[A-Za-z0-9._~+-]+\.[A-Za-z0-9]{1,8}(?!\w)"),
    re.compile(r"(?<!\w)/(?:[A-Za-z0-9._~+-]+/)*[A-Za-z0-9._~+-]+"),
    re.compile(r"(?i)(?<![\w])(?:sha1|sha256|sha512|md5):[a-f0-9]{4,}\b"),
    re.compile(r"(?<![\w])v?\d+(?:\.\d+){2,}(?![\w])"),
    re.compile(r"\b[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)+\b"),
    re.compile(r"(?<!\w)(?:[A-Z]{2,}-[A-Z0-9-]+|(?=[A-Za-z0-9-]*\d)[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)+)(?!\w)"),
    re.compile(r"(?<!\w)\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?(?!\w)"),
    re.compile(r"(?<!\w)\d{1,2}:\d{2}:\d{2}(?:\.\d+)?(?!\w)"),
    re.compile(r"(?i)(?<![\w.-])(?:localhost|[A-Za-z][\w.-]*|\d{1,3}(?:\.\d{1,3}){3}):\d{1,5}(?!\w)"),
]


def protect_speech_literals(text: str) -> tuple[str, list[str]]:
    """Mask technical spans until prose rules finish, then restore them exactly."""
    saved = []
    for pattern in _PROTECTED:
        def hide(match):
            if len(saved) >= 0xF8FF - 0xE100:
                return match.group(0)
            saved.append(match.group(0))
            return chr(0xE100 + len(saved) - 1)
        text = pattern.sub(hide, text)
    return text, saved


def restore_speech_literals(text: str, saved: list[str]) -> str:
    return re.sub(r"[\ue100-\uf8ff]", lambda m: saved[ord(m.group(0)) - 0xE100], text)


_SMALL = ("zero one two three four five six seven eight nine ten eleven twelve thirteen "
          "fourteen fifteen sixteen seventeen eighteen nineteen").split()
_TENS = ("", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety")
_ORDINAL = dict(zip(_SMALL, ("zeroth", "first", "second", "third", "fourth", "fifth", "sixth", "seventh",
                             "eighth", "ninth", "tenth", "eleventh", "twelfth", "thirteenth", "fourteenth",
                             "fifteenth", "sixteenth", "seventeenth", "eighteenth", "nineteenth")))
_ORDINAL.update(dict(zip(_TENS[2:], ("twentieth", "thirtieth", "fortieth", "fiftieth", "sixtieth",
                                    "seventieth", "eightieth", "ninetieth"))))
_ORDINAL.update({"hundred": "hundredth", "thousand": "thousandth", "million": "millionth", "billion": "billionth"})


def _under_thousand(value: int) -> str:
    parts = []
    if value >= 100:
        parts.extend((_SMALL[value // 100], "hundred"))
        value %= 100
    if value >= 20:
        tens, ones = divmod(value, 10)
        parts.append(_TENS[tens] + (f"-{_SMALL[ones]}" if ones else ""))
    elif value:
        parts.append(_SMALL[value])
    return " ".join(parts) or "zero"


def _integer_words(value: int) -> str:
    if value < 0 or value >= 1_000_000_000_000:
        return str(value)
    if value < 1000:
        return _under_thousand(value)
    chunks, names = [], ("", "thousand", "million", "billion")
    while value:
        value, chunk = divmod(value, 1000)
        chunks.append(chunk)
    return " ".join(f"{_under_thousand(n)} {names[i]}".strip()
                    for i, n in reversed(list(enumerate(chunks))) if n)


def _number_words(raw: str) -> str:
    value = raw.replace(",", "")
    sign = ""
    if value.startswith(("-", "+")):
        sign, value = ("minus " if value[0] == "-" else "plus "), value[1:]
    whole, dot, fraction = value.partition(".")
    if len(whole) > 1 and whole.startswith("0"):
        return raw
    try:
        result = _integer_words(int(whole or "0"))
    except ValueError:
        return raw
    if len(whole) > 12 and result == whole:
        return raw
    if dot:
        result += " point " + " ".join(_SMALL[int(d)] for d in fraction)
    return sign + result


def _ordinal_words(value: int) -> str:
    words = _integer_words(value).split()
    if not words or words[0].isdigit():
        return str(value)
    tail = words.pop().split("-")
    tail[-1] = _ORDINAL.get(tail[-1], tail[-1] + "th")
    words.append("-".join(tail))
    return " ".join(words)


def _singular(raw: str) -> bool:
    return "." not in raw and abs(int(raw.replace(",", ""))) == 1


_NUM = r"[+-]?(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?"
_MONTHS = "January February March April May June July August September October November December".split()
_WEEKDAYS = {"mon": "Monday", "tue": "Tuesday", "tues": "Tuesday", "wed": "Wednesday",
             "thu": "Thursday", "thur": "Thursday", "thurs": "Thursday", "fri": "Friday",
             "sat": "Saturday", "sun": "Sunday"}


def _year_words(year: int) -> str:
    if year == 2000:
        return "two thousand"
    if 2001 <= year <= 2099:
        return "twenty " + _under_thousand(year - 2000)
    if 1900 <= year <= 1999:
        return "nineteen hundred" if year % 100 == 0 else "nineteen " + _under_thousand(year % 100)
    return _integer_words(year)


def _date_words(month: int, day: int, year: int, original: str) -> str:
    if not 1 <= month <= 12 or not 1 <= day <= calendar.monthrange(year, month)[1]:
        return original
    return f"{_MONTHS[month - 1]} {_ordinal_words(day)}, {_year_words(year)}"


def _dates(text: str) -> str:
    names = "|".join(_MONTHS)
    named = re.compile(rf"(?i)\b(?P<month>{names})\s+(?P<day>\d{{1,2}})(?:st|nd|rd|th)?[,]?\s+(?P<year>\d{{4}})\b")
    def named_date(match):
        month = next(i for i, name in enumerate(_MONTHS, 1) if name.lower() == match["month"].lower())
        return _date_words(month, int(match["day"]), int(match["year"]), match.group(0))
    text = named.sub(named_date, text)
    iso = re.compile(r"(?<![\w/])(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})(?![\w/])")
    text = iso.sub(lambda m: _date_words(int(m["month"]), int(m["day"]), int(m["year"]), m.group(0)), text)
    numeric = re.compile(r"(?<![\w/])(?P<month>\d{1,2})/(?P<day>\d{1,2})/(?P<year>\d{4})(?![\w/])")
    return numeric.sub(lambda m: _date_words(int(m["month"]), int(m["day"]), int(m["year"]), m.group(0)), text)


_CLOCK_EXPR = r"(?:(?:0?[1-9]|1[0-2])(?::[0-5]\d)?\s*[ap]\.?\s*m\.?|(?:[01]?\d|2[0-3]):[0-5]\d)"


def _clock_ranges(text: str) -> str:
    pattern = re.compile(rf"(?<![\w:])({_CLOCK_EXPR})\s*[-–—]\s*({_CLOCK_EXPR})(?!\w)", re.I)
    return pattern.sub(r"\1 to \2", text)


def _times(text: str) -> str:
    pattern = re.compile(r"(?<![\w:])(?P<hour>\d{1,2}):(?P<minute>[0-5]\d)(?:\s*(?P<suffix>[ap])\.?m\.?)?(?!\w)", re.I)
    def speak(match):
        hour, minute, suffix = int(match["hour"]), int(match["minute"]), match["suffix"]
        if hour > (12 if suffix else 23) or (suffix and hour == 0):
            return match.group(0)
        if not suffix and hour == 0 and minute == 0:
            return "midnight"
        if not suffix and hour == 12 and minute == 0:
            return "noon"
        spoken_hour = _integer_words(hour % 12 or 12) if suffix else _integer_words(hour)
        spoken_minute = "" if minute == 0 else (" oh " + _SMALL[minute] if minute < 10 else " " + _under_thousand(minute))
        meridiem = " " + ("a.m." if suffix.lower() == "a" else "p.m.") if suffix else (" o'clock" if minute == 0 else "")
        return spoken_hour + spoken_minute + meridiem
    text = pattern.sub(speak, text)
    bare = re.compile(r"(?<![\w:])(?P<hour>0?[1-9]|1[0-2])\s*(?P<suffix>[ap])\.?\s*m\.?(?!\w)", re.I)
    return bare.sub(lambda m: f"{_integer_words(int(m['hour']))} {'a.m.' if m['suffix'].lower() == 'a' else 'p.m.'}", text)


def _weekdays(text: str) -> str:
    names = "|".join(sorted(_WEEKDAYS, key=len, reverse=True))
    day = rf"(?:{names})"
    day_range = re.compile(rf"(?i)\b(?P<start>{day})\.?\s*[-–]\s*(?P<end>{day})\.?(?![A-Za-z])")
    text = day_range.sub(lambda m: f"{_WEEKDAYS[m['start'].lower()]} through {_WEEKDAYS[m['end'].lower()]}", text)
    chain = re.compile(rf"(?i)\b(?P<days>{day}\.?(?:\s*(?:,|and|&)\s*{day}\.?){{1,6}})(?![A-Za-z])")
    def expand_chain(match):
        days = re.findall(names, match["days"], re.I)
        return ", ".join(_WEEKDAYS[d.lower()] for d in days)
    text = chain.sub(expand_chain, text)
    cues = re.compile(r"(?i)\b(?:on|every|each|weekday|weekdays|weekend|schedule|from|through|until|daily|meeting|appointment|January|February|March|April|May|June|July|August|September|October|November|December)\b")
    def expand_one(match):
        left, right = text[max(0, match.start() - 32):match.start()], text[match.end():match.end() + 32]
        if cues.search(left) or cues.search(right) or re.search(r"\b\d{1,2}:\d{2}\b", right):
            return _WEEKDAYS[match["day"].lower()]
        return match.group(0)
    return re.sub(rf"(?i)\b(?P<day>{day})\.?(?![A-Za-z])", expand_one, text)


def _quantity(raw: str, singular: str, plural: str) -> str:
    return f"{_number_words(raw)} {singular if _singular(raw) else plural}"


_UNITS = [
    (r"fl\s*oz", "fluid ounce", "fluid ounces", "volume"), (r"microseconds?|µs|μs", "microsecond", "microseconds", "clear"),
    (r"milliseconds?|msecs?|ms", "millisecond", "milliseconds", "clear"), (r"seconds?|secs?|s", "second", "seconds", "clear"),
    (r"minutes?|mins?", "minute", "minutes", "clear"), (r"hours?|hrs?|h", "hour", "hours", "clear"),
    (r"millimeters?|mm", "millimeter", "millimeters", "clear"), (r"centimeters?|cm", "centimeter", "centimeters", "clear"),
    (r"kilometers?|km", "kilometer", "kilometers", "clear"), (r"meters?", "meter", "meters", "clear"),
    (r"inches|inch", "inch", "inches", "clear"), (r"feet|foot|ft", "foot", "feet", "clear"),
    (r"yards?|yd", "yard", "yards", "clear"), (r"miles?|mi", "mile", "miles", "clear"),
    (r"milligrams?|mg", "milligram", "milligrams", "clear"), (r"kilograms?|kg", "kilogram", "kilograms", "clear"),
    (r"grams?|g", "gram", "grams", "clear"), (r"pounds?|lbs?", "pound", "pounds", "clear"),
    (r"milliliters?|millilitres?|ml", "milliliter", "milliliters", "clear"), (r"liters?|litres?|l", "liter", "liters", "clear"),
    (r"cups?", "cup", "cups", "clear"), (r"pints?|pt", "pint", "pints", "clear"),
    (r"quarts?|qt", "quart", "quarts", "clear"), (r"gallons?|gal", "gallon", "gallons", "clear"),
    (r"ounces?", "ounce", "ounces", "clear"), (r"oz", "ounce", "ounces", "mass"), (r"in", "inch", "inches", "ambiguous_in"),
    (r"m", "meter", "meters", "ambiguous_m"),
]


def _unit_context(text: str, start: int, end: int, kind: str) -> bool | str:
    before, after = text[max(0, start - 88):start].lower(), text[end:end + 64].lower()
    if kind == "ambiguous_m":
        nearby = text[max(0, start - 64):min(len(text), end + 64)].lower()
        minutes = 3 * bool(re.search(r"\b(?:duration|timer|wait|rest|session|focus|lasts?|within|for)\b", before[-64:]))
        minutes += bool(re.search(r"\b(?:open hours|blocked hours|schedule|hours?|hrs?)\b|\b[ap]\.m\.", nearby))
        meters = 3 * bool(re.search(r"\b(?:long|wide|tall|high|deep|away|distance|length|height|width|thickness|depth|diameter|radius)\b", after[:40] + " " + before[-40:]))
        meters += 3 * bool(re.search(r"\d+(?:\.\d+)?\s*m\s*(?:x|×|by)\s*\d|\d+(?:\.\d+)?\s*(?:x|×|by)\s*\d+(?:\.\d+)?\s*m\b", nearby))
        meters += bool(re.search(r"\b(?:mm|cm|km|ft|feet|inches|inch|yd|mi)\b", nearby))
        if minutes >= 3 and minutes >= meters + 2:
            return "minutes"
        if meters >= 3 and meters >= minutes + 2:
            return "meters"
        return ""
    if kind == "ambiguous_in":
        return bool(re.match(r"\s+(?:long|wide|tall|deep|thick|across|diameter)\b", after)
                    or re.search(r"\b(?:width|height|diameter|measures?|measured)\s*$", before))
    if kind == "mass":
        return bool(re.match(r"\s+of\s+(?:flour|sugar|cheese|meat|coffee|food|butter|rice|protein)\b", after)
                    or re.search(r"\b(?:weight|weighs?|weighing|package|mass)\s*$", before))
    return True


def _units(text: str) -> str:
    speed = re.compile(rf"(?<![\w/])(?P<num>{_NUM})\s*(?P<unit>km\s*/\s*hr?|m\s*/\s*s|mph)(?!\w)", re.I)
    def speed_words(match):
        unit = re.sub(r"\s+", "", match["unit"]).lower()
        if unit.startswith("km"):
            return _quantity(match["num"], "kilometer", "kilometers") + " per hour"
        if unit == "m/s":
            return _quantity(match["num"], "meter", "meters") + " per second"
        return _quantity(match["num"], "mile", "miles") + " per hour"
    text = speed.sub(speed_words, text)
    area = re.compile(rf"(?<![\w/])(?P<num>{_NUM})\s*(?P<unit>sq\.?\s*(?:ft|mi)|m²|ft²)(?![A-Za-z])", re.I)
    areas = {"sqft": ("square foot", "square feet"), "ft²": ("square foot", "square feet"),
             "sqmi": ("square mile", "square miles"), "m²": ("square meter", "square meters")}
    def area_words(match):
        key = match["unit"].lower().replace(".", "").replace(" ", "")
        return _quantity(match["num"], *areas[key])
    text = area.sub(area_words, text)
    temp = re.compile(rf"(?<![\w/])(?P<num>{_NUM})\s*°\s*(?P<unit>[CF])\b", re.I)
    text = temp.sub(lambda m: f"{_number_words(m['num'])} {'degree' if _singular(m['num']) else 'degrees'} "
                    + ("Celsius" if m["unit"].upper() == "C" else "Fahrenheit"), text)
    for expression, singular, plural, kind in _UNITS:
        pattern = re.compile(rf"(?<![\w/])(?P<num>{_NUM})\s*(?P<unit>{expression})(?!\w)", re.I)
        def replace(match):
            context = _unit_context(text, match.start(), match.end(), kind) if kind != "clear" else True
            if not context:
                return match.group(0)
            unit_singular, unit_plural = singular, plural
            if kind == "ambiguous_m":
                unit_singular, unit_plural = ("minute", "minutes") if context == "minutes" else ("meter", "meters")
            return _quantity(match["num"], unit_singular, unit_plural)
        text = pattern.sub(replace, text)
    return text


def _fractions(text: str) -> str:
    denominators = {2: ("half", "halves"), 3: ("third", "thirds"), 4: ("quarter", "quarters"),
                    **{n: (_ordinal_words(n), _ordinal_words(n) + "s") for n in range(5, 17)}}
    def speak(numerator: str, denominator: str) -> str:
        n, d = int(numerator), int(denominator)
        return f"{_integer_words(n)} {denominators[d][0 if n == 1 else 1]}" if n <= 99 and d in denominators else ""
    mixed = re.compile(r"(?<![\w/])(?P<whole>\d+)\s+(?P<num>\d{1,2})/(?P<den>\d{1,2})(?![\w/])")
    text = mixed.sub(lambda m: _integer_words(int(m["whole"])) + " and " + speak(m["num"], m["den"])
                     if speak(m["num"], m["den"]) else m.group(0), text)
    simple = re.compile(r"(?<![\w/])(?P<num>\d{1,2})/(?P<den>\d{1,2})(?![\w/])")
    return simple.sub(lambda m: speak(m["num"], m["den"]) or m.group(0), text)


def _slash(text: str) -> str:
    explicit = re.compile(r"(?i)\b([A-Za-z][A-Za-z'-]*)\s+and\s*/\s*or\s+([A-Za-z][A-Za-z'-]*)\b")
    text = explicit.sub(r"\1, \2, or both", text)
    text = re.sub(r"(?i)\band\s*/\s*or\b", "and or", text)
    text = re.sub(r"(?i)\bw\s*/\s*o\b", "without", text)
    text = re.sub(r"(?i)\bw\s*/(?=\s|[A-Za-z])", "with ", text)
    io = re.compile(r"(?i)\bI\s*/\s*O(?=\s+(?:device|port|interface|operation|stream|system|controller)\b)")
    text = io.sub("input and output", text)
    pair = re.compile(r"(?<![\w/])([A-Za-z][A-Za-z-]{1,}(?:\s*/\s*[A-Za-z][A-Za-z-]{1,})+)(?![\w/])")
    def list_words(match):
        items = re.split(r"\s*/\s*", match.group(1))
        return f"{items[0]} or {items[1]}" if len(items) == 2 else ", ".join(items[:-1]) + f", or {items[-1]}"
    return pair.sub(list_words, text)


def _numbers(text: str) -> str:
    ordinal = re.compile(r"(?<![\w/])(?P<num>\d{1,6})(?P<suffix>st|nd|rd|th)\b", re.I)
    def ordinal_words(match):
        value = int(match["num"])
        correct = "th" if 10 <= value % 100 <= 20 else {1: "st", 2: "nd", 3: "rd"}.get(value % 10, "th")
        return _ordinal_words(value) if match["suffix"].lower() == correct else match.group(0)
    text = ordinal.sub(ordinal_words, text)
    span = re.compile(r"(?<![\w/])(?P<a>-?\d+(?:\.\d+)?)\s*(?:–|(?<!-)-(?!-))\s*(?P<b>\d+(?:\.\d+)?)(?![\w/])")
    text = span.sub(lambda m: _number_words(m["a"]) + " to " + _number_words(m["b"]), text)
    number = re.compile(r"(?<![\w/])(?P<num>[+-]?(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?)(?![\w/])")
    common = {"app", "apps", "site", "sites", "day", "days", "week", "weeks", "month", "months", "year", "years",
              "hour", "hours", "minute", "minutes", "second", "seconds", "item", "items", "block", "blocks",
              "person", "people", "thing", "things", "file", "files", "time", "times", "option", "options",
              "user", "users", "dollar", "dollars", "cat", "cats"}
    def number_words(match):
        tail = re.match(r"\s+([a-z]{3})\b", text[match.end():match.end() + 12])
        ambiguous = re.match(r"\s+(m|in|oz)\b", text[match.end():match.end() + 8])
        return match.group(0) if ambiguous or (tail and tail[1] not in common) else _number_words(match["num"])
    return number.sub(number_words, text)


def normalize_spoken_text(text: str) -> str:
    """Run exact mappings, typed formats, units, slash rules, then plain numbers."""
    text = re.sub(r"(?i)\be\.g\.,?\s*", "for example, ", text)
    text = re.sub(r"(?i)\bi\.e\.,?\s*", "that is, ", text)
    text = re.sub(r"(?i)\betc\.?\b", "and more", text)
    text = re.sub(r"(?i)\bvs\.?\b", "versus", text)
    return _numbers(_slash(_fractions(_units(_weekdays(_times(_dates(_clock_ranges(text))))))))
