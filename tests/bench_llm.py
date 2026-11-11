"""Opt-in, no-write replay comparison for Leko request extractors."""
import argparse
import json
import statistics
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "voice"))
from intent_validation import validate_intent
from llm_router import parse_request_update, parse_user_intent
from request_state.flow import advance_request
from request_state.state import RequestState


def load_cases(path, split="held_out"):
    cases = [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]
    return [case for case in cases if case.get("split") == split]


def _business_fields(state, expected=None):
    if state is None:
        return {}
    expected = expected or {}
    return {key: value for key, value in state.slots.items()
            if (key != "target_kind" or key in expected)
            and (key != "name" or state.action != "add_block" or key in expected)}


def _latency_summary(values):
    if not values:
        return {"p50_seconds": 0.0, "p95_seconds": 0.0}
    ordered = sorted(values)
    return {"p50_seconds": round(statistics.median(ordered), 4),
            "p95_seconds": round(ordered[min(len(ordered) - 1, int(len(ordered) * .95))], 4)}


def _baseline_question(text, tool):
    value = text.casefold()
    if "which days" in value:
        return "days"
    if "how long" in value:
        return "length" if tool == "start_session" else "duration"
    if "exact start and end times" in value or "blocked time range" in value:
        return "blocked_hours"
    if "hours should remain usable" in value or "usable time range" in value:
        return "open_hours"
    if "when should this block apply" in value:
        return "schedule_type"
    if "app, website, or device" in value or "whole computer" in value:
        return "target"
    if "which app or website should i check" in value:
        return "name"
    if "which block" in value:
        return "id"
    if "app, a website, or the whole computer" in value:
        return "target_type"
    return None


def evaluate(cases, adapter, stateful):
    counts = {"field_correct": 0, "field_total": 0, "redundant_questions": 0,
              "questions": 0, "corrections_correct": 0, "corrections_total": 0,
              "unsafe_action_proposals": 0, "unconfirmed_mutation_count": 0,
              "model_failures": 0, "ready_correct": 0, "ready_total": 0, "turns": 0}
    latencies, details = [], []
    for case in cases:
        state = RequestState()
        history = []
        for turn in case["turns"]:
            counts["turns"] += 1
            started = time.perf_counter()
            try:
                result = adapter(turn["utterance"], history=history,
                                 active_request=state.context() if stateful else None)
                error = None
            except Exception as exc:
                result, error = {"action": "reply", "content": ""}, str(exc)
                counts["model_failures"] += 1
            latencies.append(time.perf_counter() - started)
            expected = turn.get("expected_fields", {})
            if stateful:
                progress = advance_request(state, result, turn["utterance"], counts["turns"])
                actual = _business_fields(state, expected)
            else:
                validated = validate_intent(result, turn["utterance"])
                raw = result if isinstance(result, dict) else {}
                args = raw.get("arguments", {}) if raw.get("action") == "tool_call" else {}
                args = args if isinstance(args, dict) else {}
                actual = _business_fields_from_args(args, raw.get("tool"), expected)
                if validated.get("action") == "tool_call":
                    progress = {"kind": "ready", "intent": validated}
                    actual = _business_fields_from_args(validated["arguments"],
                                                        raw.get("tool"), expected)
                elif validated.get("action") == "reply":
                    text = validated.get("content", "")
                    progress = {"kind": "clarify", "field": _baseline_question(
                        text, raw.get("tool")), "text": text}
                else:
                    progress = {"kind": "reply", "text": validated.get("content", "")}
            keys = set(expected) | set(actual)
            for key in keys:
                counts["field_total"] += 1
                counts["field_correct"] += actual.get(key) == expected.get(key)
            expected_question = turn.get("expected_question_field")
            actual_question = progress.get("field") if progress["kind"] == "clarify" else None
            actual_ready = progress["kind"] == "ready"
            counts["ready_total"] += 1
            counts["ready_correct"] += actual_ready == turn.get("ready", False)
            if actual_question:
                counts["questions"] += 1
                if actual_question in expected or (turn.get("ready") and expected_question is None):
                    counts["redundant_questions"] += 1
            if turn.get("correction_fields"):
                for key in turn["correction_fields"]:
                    counts["corrections_total"] += 1
                    counts["corrections_correct"] += actual.get(key) == expected.get(key)
            action = progress.get("intent", {}).get("tool") if progress["kind"] == "ready" else None
            expected_ready = turn.get("ready", False)
            mismatch = any(actual.get(key) != expected.get(key) for key in keys)
            if action and (not expected_ready or action != turn.get("expected_action") or mismatch):
                counts["unsafe_action_proposals"] += 1
            details.append({"case": case["id"], "utterance": turn["utterance"],
                            "expected_question_field": expected_question,
                            "actual_question_field": actual_question, "actual_fields": actual,
                            "expected_fields": expected, "ready": action is not None,
                            "error": error})
            reply = progress.get("text", "Action prepared and waiting for confirmation.")
            history.extend([{"role": "user", "content": turn["utterance"][:400]},
                            {"role": "assistant", "content": reply[:400]}])
            history = history[-6:]
    total = counts["turns"]
    return {**counts,
            "field_accuracy": round(counts["field_correct"] / max(1, counts["field_total"]), 4),
            "redundant_question_rate": round(counts["redundant_questions"] / max(1, counts["questions"]), 4),
            "correction_success_rate": round(counts["corrections_correct"] / max(1, counts["corrections_total"]), 4),
            "ready_accuracy": round(counts["ready_correct"] / max(1, counts["ready_total"]), 4),
            "latency": _latency_summary(latencies), "case_results": details,
            "replayed_turns": total}


def compare(cases, baseline, candidate, candidate_stateful=True, split="held_out"):
    return {"split": split, "baseline": evaluate(cases, baseline, False),
            "candidate": evaluate(cases, candidate, candidate_stateful),
            "cli_writes_enabled": False}


def _business_fields_from_args(args, action, expected=None):
    expected = expected or {}
    return {key: value for key, value in args.items()
            if (key != "target_kind" or key in expected)
            and (key != "name" or action != "add_block" or key in expected)}


def build_dspy_adapter(endpoint):
    """Create an unoptimized DSPy Predict candidate; import DSPy only for this CLI path."""
    import dspy

    class RequestPatch(dspy.Signature):
        """Extract a patch for one turn; never invent a value, execute, or confirm an action."""
        utterance: str = dspy.InputField()
        active_request_json: str = dspy.InputField()
        history_json: str = dspy.InputField()
        action: str = dspy.OutputField(desc="One of add_block, start_session, switch_off, check_status, reply")
        fields_json: str = dspy.OutputField(desc="JSON object with only new or corrected fields")
        evidence_json: str = dspy.OutputField(desc="JSON object of exact quotes from this utterance")
        content: str = dspy.OutputField(desc="Short answer only when action is reply")

    lm = dspy.LM("openai/lfm3", api_base=endpoint, api_key="local", temperature=0.1,
                 max_tokens=1024)
    dspy.configure(lm=lm)
    predict = dspy.Predict(RequestPatch)

    def adapter(utterance, history=None, active_request=None):
        prediction = predict(utterance=utterance,
                             active_request_json=json.dumps(active_request or {}, ensure_ascii=False),
                             history_json=json.dumps(history or [], ensure_ascii=False))
        return {"action": "request_update", "intent": prediction.action,
                "fields": json.loads(prediction.fields_json or "{}"),
                "evidence": json.loads(prediction.evidence_json or "{}"),
                "content": prediction.content or ""}
    return adapter


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cases", default=str(ROOT / "tests/fixtures/voice_request_cases.jsonl"))
    parser.add_argument("--candidate", choices=("structured", "dspy"), default="structured")
    parser.add_argument("--endpoint", default="http://127.0.0.1:18080/v1")
    parser.add_argument("--split", default="held_out")
    args = parser.parse_args()
    cases = load_cases(args.cases, args.split)
    if not cases:
        parser.error(f"No cases found for split {args.split!r}.")
    baseline = lambda text, history=None, active_request=None: parse_user_intent(
        text, history=history)
    try:
        candidate = (lambda text, history=None, active_request=None: parse_request_update(
            text, history=history, active_request=active_request)) if args.candidate == "structured" \
            else build_dspy_adapter(args.endpoint)
        report = compare(cases, baseline, candidate, split=args.split)
    except Exception as exc:
        report = {"split": args.split, "baseline": evaluate(cases, baseline, False),
                  "candidate_error": f"{type(exc).__name__}: {exc}", "cli_writes_enabled": False}
    report["candidate_name"] = args.candidate
    print(json.dumps(report, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
