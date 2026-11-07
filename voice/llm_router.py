from __future__ import annotations
import sys
"""Local LLM tool caller using LiquidAI LFM3-1.7B.

Parses natural speech or typed requests into structured Leko CLI commands.
"""

import datetime
import json
import os
import re
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Any, Dict, List, Optional

SERVER_PORT = 18080
BASE_URL = f"http://127.0.0.1:{SERVER_PORT}"
RUNTIME_BIN = Path("/Users/calibro/.t3/scratch/2026-10-29-based-on-these-documents-what-d76b9ec3/voice-beta/.runtime/llama-b11980/llama-server")
MODEL_PATH = Path("/Users/calibro/.t3/scratch/2026-10-29-based-on-these-documents-what-d76b9ec3/voice-beta/models/LFM3-1.7B-Q4_K_M.gguf")

TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "add_block",
            "description": "Create a new distraction blocking rule for apps or websites.",
            "parameters": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Short clean descriptive name for the block, e.g. 'Social Media' or 'Distractions'"
                    },
                    "apps": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "App bundle identifiers or application names to block, e.g. ['com.apple.Chess', 'com.apple.Calculator']"
                    },
                    "sites": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Website domains or hosts to block, e.g. ['reddit.com', 'twitter.com']"
                    },
                    "schedule_type": {
                        "type": "string",
                        "enum": ["always", "for_duration", "open_hours"],
                        "description": "Type of schedule: 'always' (blocked all day), 'for_duration' (blocked for a duration length like 2h, 45m), or 'open_hours' (allowed during specific window like 17:00-21:00)"
                    },
                    "duration": {
                        "type": "string",
                        "description": "Duration length for 'for_duration': must be a duration such as '30m', '2h', '4h', '90m' (never 'until ...')"
                    },
                    "open_hours": {
                        "type": "string",
                        "description": "Allowed open hours window for 'open_hours', e.g. '12:00-13:00' or '17:00-21:00'"
                    },
                    "days": {
                        "type": "string",
                        "description": "Comma separated days of week if not everyday, e.g. 'mon,tue,wed,thu,fri'"
                    }
                },
                "required": ["name", "schedule_type"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "start_session",
            "description": "Start an immediate, unskippable focus session for a specific duration.",
            "parameters": {
                "type": "object",
                "properties": {
                    "length": {
                        "type": "string",
                        "description": "Length of focus session in minutes or with units, e.g. '25m', '45m', '2h'"
                    }
                },
                "required": ["length"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "switch_off",
            "description": "Turn off or unlock an existing block.",
            "parameters": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The identifier of the block to switch off"
                    }
                },
                "required": ["id"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "check_status",
            "description": "Check whether an app or website is currently blocked, and when it opens or closes next.",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_type": {
                        "type": "string",
                        "enum": ["app", "site"],
                        "description": "Whether checking an application or a website"
                    },
                    "name": {
                        "type": "string",
                        "description": "App name/bundle id or website domain"
                    }
                },
                "required": ["target_type", "name"]
            }
        }
    }
]

SYSTEM_PROMPT = """You are Leko Voice Assistant, an on-device privacy-first focus and distraction enforcer.
You translate the user's spoken voice requests into concrete, structured tool calls.
Always pick the most specific tool:
- If the user wants to focus for a period, use `start_session`.
- If the user wants to block certain apps or sites, use `add_block`.
- If the user wants to remove or turn off a block, use `switch_off`.
- If the user asks whether something is blocked or allowed, use `check_status`.
Extract clean domain names (e.g. reddit.com) and clean app identifiers.
CRITICAL: All durations must be expressed as lengths with units, such as '30m', '2h', '90m'. If the user says 'until 5pm', convert it to a duration length (e.g. '2h' or '3h').
"""

def is_server_running() -> bool:
    try:
        with urllib.request.urlopen(f"{BASE_URL}/health", timeout=1) as resp:
            return resp.status == 200
    except Exception:
        return False

def ensure_server(timeout_sec: int = 20) -> subprocess.Popen | None:
    if is_server_running():
        return None
    
    if not RUNTIME_BIN.exists():
        raise FileNotFoundError(f"llama-server binary not found at {RUNTIME_BIN}")
    if not MODEL_PATH.exists():
        raise FileNotFoundError(f"LFM3 GGUF model not found at {MODEL_PATH}")

    cmd = [
        str(RUNTIME_BIN),
        "-m", str(MODEL_PATH),
        "--host", "127.0.0.1",
        "--port", str(SERVER_PORT),
        "--alias", "lfm3",
        "--ctx-size", "2048",
        "--jinja",
        "--no-webui",
        "--device", "none",
        "-ngl", "0",
        "--threads", "4"
    ]
    proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    
    start = time.time()
    while time.time() - start < timeout_sec:
        if proc.poll() is not None:
            raise RuntimeError(f"llama-server exited prematurely with code {proc.returncode}")
        if is_server_running():
            return proc
        time.sleep(0.5)
    
    proc.terminate()
    raise TimeoutError("LFM3 server did not become ready in time")

def _normalize_duration(d: str) -> str:
    d = d.strip().lower()
    m = re.search(r"(\d+)\s*(hour|hr|h)", d)
    if m:
        return f"{m.group(1)}h"
    m = re.search(r"(\d+)\s*(minute|min|m)", d)
    if m:
        return f"{m.group(1)}m"
    if "until" in d:
        return "2h"
    return d

def parse_user_intent(user_text: str) -> Dict[str, Any]:
    """Sends user text to LFM3 and returns the chosen tool name and arguments."""
    ensure_server()

    payload = {
        "model": "lfm3",
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_text}
        ],
        "tools": TOOLS,
        "temperature": 0.1
    }

    req = urllib.request.Request(
        f"{BASE_URL}/v1/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"}
    )

    with urllib.request.urlopen(req, timeout=30) as resp:
        data = json.loads(resp.read().decode("utf-8"))
        msg = data["choices"][0]["message"]
        tool_calls = msg.get("tool_calls", [])
        if not tool_calls:
            return {"action": "reply", "content": msg.get("content", "")}
        
        tc = tool_calls[0]
        name = tc["function"]["name"]
        args = json.loads(tc["function"]["arguments"])
        if "duration" in args:
            args["duration"] = _normalize_duration(args["duration"])
        if "length" in args:
            args["length"] = _normalize_duration(args["length"])
        return {"action": "tool_call", "tool": name, "arguments": args}

def tool_to_cli_args(settings_zip: str, tool_name: str, args: Dict[str, Any], write: bool = False) -> List[str]:
    """Maps structured tool call into `leko` CLI arguments."""
    cli_args = []
    
    if tool_name == "start_session":
        cli_args.extend(["session", args["length"]])
    elif tool_name == "add_block":
        cli_args.extend(["add-block", settings_zip, "--name", args["name"]])
        for app in args.get("apps", []):
            cli_args.extend(["--app", app])
        for site in args.get("sites", []):
            cli_args.extend(["--site", site])
        
        sched = args.get("schedule_type", "always")
        if sched == "always":
            cli_args.append("--always")
        elif sched == "for_duration" and args.get("duration"):
            cli_args.extend(["--for", args["duration"]])
        elif sched == "open_hours" and args.get("open_hours"):
            cli_args.extend(["--open-hours", args["open_hours"]])
        else:
            cli_args.append("--always")
        
        if args.get("days"):
            cli_args.extend(["--days", args["days"]])
        if write:
            cli_args.append("--write")
    elif tool_name == "switch_off":
        cli_args.extend(["switch-off", settings_zip, "--id", args["id"]])
        if write:
            cli_args.append("--write")
    elif tool_name == "check_status":
        cli_args.extend(["status", settings_zip])
        if args["target_type"] == "app":
            cli_args.extend(["--app", args["name"]])
        else:
            cli_args.extend(["--site", args["name"]])
    else:
        raise ValueError(f"Unknown tool: {tool_name}")
    
    return cli_args

def parse_user_intent_safe(user_text: str, endpoint_url: Optional[str] = None) -> Dict[str, Any]:
    """Safe wrapper around parse_user_intent handling HTTP and connection errors gracefully."""
    try:
        if endpoint_url:
            req = urllib.request.Request(
                endpoint_url,
                data=json.dumps({"model": "lfm3", "messages": [{"role": "user", "content": user_text}]}).encode("utf-8"),
                headers={"Content-Type": "application/json"}
            )
            with urllib.request.urlopen(req, timeout=5) as resp:
                data = json.loads(resp.read().decode("utf-8"))
                return {"action": "reply", "content": data["choices"][0]["message"].get("content", "")}
        return parse_user_intent(user_text)
    except Exception as e:
        print(f"[LLM Router Warning]: {e}", file=sys.stderr)
        return {
            "action": "reply",
            "content": "Sorry, I ran into an issue communicating with the local model. Please try again."
        }
