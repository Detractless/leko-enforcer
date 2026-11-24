"""Build native leko CLI argument lists and read its stable JSON output."""

import json
import os
import subprocess

CANONICAL_TOOLS = {
    "create_rule": "create-rule",
    "list_rules": "list-rules",
    "show_rule": "show-rule",
    "edit_rule": "edit-rule",
    "delete_rule": "delete-rule",
    "queue_edit": "queue-edit",
    "cancel_edit": "cancel-edit",
    "request_pass": "request-pass",
    "check_status": "check-status",
    "evaluate_rule": "evaluate-rule",
}

PLANNED_TOOLS = {
    "pomodoro_schedule",
    "community_linked_apps",
    "community_linked_websites",
    "engagement_pass",
    "restart_seal",
}

def tool_to_command(tool):
    """Return the kebab-case command for one canonical tool."""
    if tool in PLANNED_TOOLS:
        return None
    if tool not in CANONICAL_TOOLS:
        raise ValueError("Unknown tool: {}".format(tool))
    return CANONICAL_TOOLS[tool]


def cli_binary(default):
    """Allow tests to point at a temporary binary without changing production."""
    return os.environ.get("LEKO_CLI_BIN", str(default))


def _target_json(args, store):
    if args.get("device"):
        return {"kind": "whole_device", "action": args["device"]}
    if args.get("apps"):
        return {"kind": "specific_apps", "apps": list(args["apps"])}
    if args.get("sites"):
        return {"kind": "specific_websites", "sites": list(args["sites"])}
    return {"kind": "specific_apps", "apps": []}


def build_argv(store, tool, args, at=None, json_output=True):
    """Build one canonical argv list. No legacy command or flag is produced."""
    command = tool_to_command(tool)
    if command is None:
        return []
    store_path = str(args.get("store") or store or "leko-rules.json")
    argv = [command, "--store", store_path]
    if json_output:
        argv.append("--json")
    if at:
        argv.extend(["--at", str(at)])

    if tool == "create_rule":
        argv.extend(["--name", str(args.get("name", "")),
                     "--target", json.dumps(_target_json(args, store))])
        if args.get("penalty") is not None:
            argv.extend(["--penalty", json.dumps(args["penalty"])])
    elif tool in ("show_rule", "edit_rule", "delete_rule", "queue_edit",
                  "cancel_edit", "request_pass", "check_status", "evaluate_rule"):
        if args.get("id"):
            argv.extend(["--id", str(args["id"])])
        if tool == "edit_rule":
            argv.extend(["--direction", str(args.get("direction", "weaken"))])
        if tool in ("edit_rule", "queue_edit"):
            if args.get("field"):
                argv.extend(["--field", str(args["field"])])
            if args.get("value") is not None:
                argv.extend(["--value", json.dumps(args["value"])])
    return argv


def parse_json(stdout):
    """Return the CLI result object or raise a plain error."""
    try:
        value = json.loads(stdout)
    except (TypeError, ValueError):
        raise ValueError("the CLI result was not readable")
    if not isinstance(value, dict):
        raise ValueError("the CLI result was not an object")
    return value


def run_cli(argv, binary):
    """Run one native CLI call with a configurable binary path."""
    if not argv:
        raise ValueError("there is no command to run")
    return subprocess.run([cli_binary(binary)] + list(argv),
                          capture_output=True, text=True)
