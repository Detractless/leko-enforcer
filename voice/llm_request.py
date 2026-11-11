"""Guard LFM3 HTTP requests and report failures with elapsed time."""
from __future__ import annotations

import json
import sys
import time
import urllib.request
from typing import Optional


def parse_user_intent_safe(user_text: str, endpoint_url: Optional[str] = None,
                           history=None, memories=None, trace_callback=None):
    """Call the intent router and turn model/network errors into spoken replies."""
    started_at = time.monotonic()
    payload = None
    try:
        if endpoint_url:
            payload = {"model": "lfm3", "messages": [{"role": "user", "content": user_text}]}
            req = urllib.request.Request(
                endpoint_url,
                data=json.dumps(payload).encode("utf-8"),
                headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=5) as resp:
                data = json.loads(resp.read().decode("utf-8"))
            message = data["choices"][0]["message"]
            if trace_callback is not None:
                try:
                    trace_callback({"status": "ok", "request": payload, "response": data,
                                    "elapsed_seconds": round(time.monotonic() - started_at, 3)})
                except Exception:
                    pass
            result = {"action": "reply", "content": message.get("content", "")}
            if "reasoning_content" in message:
                result["reasoning_content"] = message["reasoning_content"]
            return result
        try:
            from .llm_router import parse_user_intent
        except ImportError:
            from llm_router import parse_user_intent
        return parse_user_intent(user_text, history=history, memories=memories,
                                 trace_callback=trace_callback)
    except Exception as exc:
        elapsed = time.monotonic() - started_at
        print(f"[LLM Router Warning after {elapsed:.1f}s; {type(exc).__name__}]: {exc}", file=sys.stderr)
        if endpoint_url and trace_callback is not None:
            try:
                trace_callback({"status": "error", "request": payload,
                                "error": {"type": type(exc).__name__, "message": str(exc)},
                                "elapsed_seconds": round(elapsed, 3)})
            except Exception:
                pass
        return {"action": "reply", "content":
                "Sorry, I ran into an issue communicating with the local model. Please try again."}


def parse_request_update_safe(user_text: str, history=None, memories=None, active_request=None,
                              trace_callback=None):
    """Safely request a structured field patch for the current user turn."""
    started_at = time.monotonic()
    try:
        try:
            from .llm_router import parse_request_update
        except ImportError:
            from llm_router import parse_request_update
        return parse_request_update(user_text, history=history, memories=memories,
                                    active_request=active_request, trace_callback=trace_callback)
    except Exception as exc:
        elapsed = time.monotonic() - started_at
        print(f"[LLM Router Warning after {elapsed:.1f}s; {type(exc).__name__}]: {exc}", file=sys.stderr)
        if trace_callback is not None:
            try:
                trace_callback({"status": "error", "request": {"user_text": user_text},
                                "error": {"type": type(exc).__name__, "message": str(exc)},
                                "elapsed_seconds": round(elapsed, 3)})
            except Exception:
                pass
        return {"action": "reply", "content":
                "Sorry, I ran into an issue communicating with the local model. Please try again."}
