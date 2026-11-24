"""Build fixed canonical spoken summaries for validated action previews."""

import json

from ..routing.canonical_speech import canonical_name, empty_notice, status_sentence


def _count(stdout):
    try:
        value = json.loads(stdout)
    except (TypeError, ValueError):
        return 0
    if isinstance(value, dict):
        rules = value.get("rules")
        if isinstance(rules, list):
            return len(rules)
    return 0


def draft_summary(tool: str, args: dict, stdout: str, format_status_summary):
    """Return a spoken preview and three short labels for one validated tool."""
    if tool == "create_rule":
        target = args.get("target") or {}
        kind = target.get("kind")
        if not kind:
            if args.get("device"):
                kind = "whole_device"
            elif args.get("sites"):
                kind = "specific_websites"
            else:
                kind = "specific_apps"
        summary = "Prepared a Technology Management Rule for {}.".format(canonical_name(kind))
    elif tool == "list_rules":
        count = _count(stdout)
        summary = empty_notice() if count == 0 else "{} Technology Management Rules are stored.".format(count)
    elif tool == "show_rule":
        summary = "Showing Technology Management Rule {}.".format(args.get("id", ""))
    elif tool == "edit_rule":
        summary = "Prepared an edit to the Technology Management Rule, subject to its Seal."
    elif tool == "delete_rule":
        summary = "Prepared to delete the Technology Management Rule."
    elif tool == "queue_edit":
        summary = "Prepared a Queued Delay Seal edit."
    elif tool == "cancel_edit":
        summary = "Prepared to cancel the queued edit."
    elif tool == "request_pass":
        summary = "Prepared a Pass request. The rule will not be edited."
    elif tool in ("check_status", "evaluate_rule"):
        summary = format_status_summary(stdout)
    else:
        summary = "Action prepared."

    if tool in ("check_status", "evaluate_rule"):
        return summary, "Status Report", "Status", summary
    return summary, "Rule Preview", "Rule", summary
