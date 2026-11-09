"""Benchmark LLM routing: latency + correctness across utterance types."""
import sys, os, time, json
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "voice"))
from llm_router import parse_user_intent

CASES = [
    # (utterance, history, expected_tool_or_action)
    ("block reddit for 2 hours", None, "add_block"),
    ("focus for 30 minutes", None, "start_session"),
    ("turn off the block named freeze", None, "switch_off"),
    ("is reddit blocked", None, "check_status"),
    ("what time is it", None, "reply"),
    ("lock the whole computer for 1 hour", None, "add_block"),
    ("unblock twitter", None, "switch_off"),
    ("block youtube and twitter", None, "add_block"),
]

results = []
for utt, hist, expected in CASES:
    t0 = time.time()
    try:
        r = parse_user_intent(utt, history=hist)
        lat = time.time() - t0
        got = r.get("tool") or r.get("action", "?")
        ok = got == expected
        results.append((utt, got, expected, lat, ok, None))
    except Exception as e:
        lat = time.time() - t0
        results.append((utt, "ERROR", expected, lat, False, str(e)))

print(f"{'Utterance':<40} {'Got':<15} {'Expected':<15} {'Lat(s)':>7} {'OK'}")
print("-" * 85)
for utt, got, exp, lat, ok, err in results:
    print(f"{utt:<40} {got:<15} {exp:<15} {lat:>7.1f} {'✓' if ok else '✗'}")
    if err:
        print(f"  ERROR: {err}")

lats = [r[3] for r in results]
oks = [r[4] for r in results]
print(f"\nAvg latency: {sum(lats)/len(lats):.1f}s  Min: {min(lats):.1f}  Max: {max(lats):.1f}")
print(f"Accuracy: {sum(oks)}/{len(oks)}")
# Correction scenarios
print("\n--- Correction scenarios ---")
corr_cases = [
    ("actually I meant full device lock not reddit",
     [{"role":"user","content":"block reddit for 2 hours"},{"role":"assistant","content":"Blocked reddit for 2h."}],
     "add_block"),
    ("no, 30 minutes not 2 hours",
     [{"role":"user","content":"focus for 2 hours"},{"role":"assistant","content":"Started 2h session."}],
     "start_session"),
]
for utt, hist, expected in corr_cases:
    t0 = time.time()
    try:
        r = parse_user_intent(utt, history=hist)
        lat = time.time() - t0
        got = r.get("tool") or r.get("action","?")
        print(f"{utt:<50} {got:<15} {lat:>6.1f}s {'✓' if got==expected else '✗'}  args={r.get('arguments')}")
    except Exception as e:
        print(f"{utt:<50} ERROR {str(e)[:40]}")
