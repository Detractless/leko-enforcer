"""Build fixed spoken summaries for validated action previews."""


def draft_summary(tool: str, args: dict, stdout: str, format_status_summary) -> tuple[str, str, str, str]:
    if tool == "start_session":
        length = args["length"]
        return (f"Prepared a focus session of {length}.", "Focus Session", "Session",
                f"Unskippable session for {length}.")
    if tool == "add_block":
        device = args.get("device")
        targets = list(args.get("apps", [])) + list(args.get("sites", []))
        target = f"the whole computer ({device})" if device else ", ".join(targets)
        if args["schedule_type"] == "blocked_hours":
            start, end = args["blocked_hours"].split("-")
            window = f"{_spoken_clock(start)} to {_spoken_clock(end)} {_spoken_days(args['days'])}"
            return (f"Drafted a block for {target} from {window}.", "Block Rule", "Block",
                    f"Blocks {target} from {window}.")
        schedule = (f"usable during {args['open_hours']}" if args["schedule_type"] == "open_hours"
                    else args.get("duration") or "all day")
        return (f"Drafted a block for {target}, {schedule}.", "Block Rule", "Block",
                f"Blocks {target}, {schedule}.")
    if tool == "switch_off":
        block_id = args["id"]
        return (f"Prepared to turn off block {block_id}.", f"Turn Off Block {block_id}",
                "Switch Off", f"Turns off rule {block_id}.")
    if tool == "check_status":
        summary = format_status_summary(stdout, args["target_type"], args.get("name", ""))
        return summary, "Status Report", "Status", summary
    return "Action prepared.", "Action", "Command", "Prepared action."


def _spoken_clock(value: str) -> str:
    hour, minute = map(int, value.split(":"))
    if hour == 0 and minute == 0:
        return "midnight"
    if hour == 12 and minute == 0:
        return "noon"
    suffix = "a.m." if hour < 12 else "p.m."
    shown_hour = hour % 12 or 12
    return f"{shown_hour} {suffix}" if minute == 0 else f"{shown_hour}:{minute:02d} {suffix}"


def _spoken_days(value: str) -> str:
    days = [day.strip() for day in value.split(",") if day.strip()]
    if days == ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]:
        return "every day"
    if days == ["mon", "tue", "wed", "thu", "fri"]:
        return "every weekday"
    if days == ["sat", "sun"]:
        return "every weekend"
    names = {"mon": "Monday", "tue": "Tuesday", "wed": "Wednesday", "thu": "Thursday",
             "fri": "Friday", "sat": "Saturday", "sun": "Sunday"}
    return "on " + ", ".join(names.get(day, day) for day in days)
