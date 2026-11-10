"""Intent router: classify utterance, select specialized prompt, route.

Experimental cascade classifier behind ROUTING_MODE config flag.
"""
from __future__ import annotations

import json
import os
import time
import urllib.request

BASE_URL = "http://127.0.0.1:18080"

INTENT_CATEGORIES = ["tool_call", "conversational", "correction", "status_check"]

CLASSIFIER_PROMPT = (
    "Classify the user's utterance into exactly one category: "
    "tool_call, conversational, correction, or status_check. "
    "Reply with ONLY the category name."
)

SPECIALIZED_PROMPTS = {
    "tool_call": (
        "You are Leko Voice Assistant. Translate the request into a structured tool call. "
        "Pick the most specific tool. Extract clean domains and app names. "
        "Durations must be lengths with units (30m, 2h)."
    ),
    "conversational": (
        "You are Leko Voice Assistant. Answer concisely in natural spoken English. "
        "No Markdown. Keep under 2 sentences."
    ),
    "correction": (
        "You are Leko Voice Assistant. The user is correcting a prior request. "
        "Acknowledge briefly, then act on the corrected request. "
        "Extract clean domains. Durations must be lengths with units."
    ),
    "status_check": (
        "You are Leko Voice Assistant. Use check_status. Reply concisely."
    ),
}

ROUTING_MODE = os.environ.get("LEKO_ROUTING_MODE", "single")


def _emit_trace(callback, trace):
    if callback is not None:
        try:
            callback(trace)
        except Exception:
            pass


def classify_intent(user_text, history=None, ensure_server_fn=None, trace_callback=None):
    messages = [{"role": "system", "content": CLASSIFIER_PROMPT}]
    if history:
        messages.extend(list(history)[-2:])
    messages.append({"role": "user", "content": user_text})
    payload = {"model": "lfm3", "messages": messages, "temperature": 0.0, "max_tokens": 10}
    started = time.perf_counter()
    try:
        if ensure_server_fn:
            ensure_server_fn()
        req = urllib.request.Request(
            f"{BASE_URL}/v1/chat/completions",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=180) as resp:
            data = json.loads(resp.read().decode("utf-8"))
        _emit_trace(trace_callback, {"status": "ok", "request": payload, "response": data,
                                    "elapsed_seconds": round(time.perf_counter() - started, 3)})
        content = data["choices"][0]["message"].get("content", "").strip().lower()
        for cat in INTENT_CATEGORIES:
            if cat in content:
                return cat
        return "tool_call"
    except Exception as exc:
        _emit_trace(trace_callback, {"status": "error", "request": payload,
                                    "error": {"type": type(exc).__name__, "message": str(exc)},
                                    "elapsed_seconds": round(time.perf_counter() - started, 3)})
        return "tool_call"


def get_specialized_prompt(category):
    return SPECIALIZED_PROMPTS.get(category, SPECIALIZED_PROMPTS["tool_call"])
