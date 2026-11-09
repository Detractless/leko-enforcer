"""Guard LFM3 HTTP requests and report failures with elapsed time."""
from __future__ import annotations

import json
import sys
import time
import urllib.request
from typing import Optional


def parse_user_intent_safe(user_text: str, endpoint_url: Optional[str] = None,
                           history=None, memories=None):
    """Call the intent router and turn model/network errors into spoken replies."""
    started_at = time.monotonic()
    try:
        if endpoint_url:
            req = urllib.request.Request(
                endpoint_url,
                data=json.dumps({"model": "lfm3", "messages": [{"role": "user", "content": user_text}]}).encode("utf-8"),
                headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=5) as resp:
                data = json.loads(resp.read().decode("utf-8"))
                return {"action": "reply", "content": data["choices"][0]["message"].get("content", "")}
        try:
            from .llm_router import parse_user_intent
        except ImportError:
            from llm_router import parse_user_intent
        return parse_user_intent(user_text, history=history, memories=memories)
    except Exception as exc:
        elapsed = time.monotonic() - started_at
        print(f"[LLM Router Warning after {elapsed:.1f}s; {type(exc).__name__}]: {exc}", file=sys.stderr)
        return {"action": "reply", "content":
                "Sorry, I ran into an issue communicating with the local model. Please try again."}
