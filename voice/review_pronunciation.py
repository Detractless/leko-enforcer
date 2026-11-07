"""Pronunciation and Verbalization Review Tool.

Allows developers and users to inspect before-and-after text transformations
and listen to audio synthesis takes to verify natural human-ear delivery.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import List, Dict

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))

try:
    from voice.verbalizer import verbalize_for_speech
except ImportError:
    from verbalizer import verbalize_for_speech

DEFAULT_SAMPLES = [
    "Blocked distracting sites, e.g. reddit.com, twitter.com, etc.",
    "Focus session active for 25m vs 2h with 45s cooldown.",
    "Visiting https://www.reddit.com/r/all for study materials.",
    "Status report:\n| Target | Type | Rule |\n| :--- | :--- | :--- |\n| Discord | App | Blocked for 2h |\n| reddit.com | Site | Always blocked |",
    "Command executed with `leko block --app Slack` and verified **successfully**."
]

def generate_pronunciation_review(samples: List[str]) -> List[Dict[str, str]]:
    """Generates comparison pairs between raw text and verbalized speech text."""
    results = []
    for s in samples:
        v = verbalize_for_speech(s)
        results.append({
            "original": s,
            "verbalized": v
        })
    return results

def main():
    parser = argparse.ArgumentParser(description="Pronunciation & Spoken Verbalization Review Tool")
    parser.add_argument("--text", type=str, help="Custom text sample to review")
    parser.add_argument("--listen", action="store_true", help="Play audio takes through Maeve TTS")
    args = parser.parse_args()

    samples = [args.text] if args.text else DEFAULT_SAMPLES
    reviews = generate_pronunciation_review(samples)

    print("\n" + "=" * 70)
    print(" Leko Pronunciation & Verbalization Review")
    print("=" * 70)

    for idx, r in enumerate(reviews, 1):
        print(f"\n[Sample #{idx}]")
        print("  RAW MODEL / UI TEXT:")
        for line in r["original"].split("\n"):
            print(f"    {line}")
        print("  SPOKEN VERBALIZATION:")
        print(f"    --> \"{r['verbalized']}\"")

        if args.listen:
            try:
                from voice.assistant import speak
                print("  [Playing audio through Maeve...]")
                speak(r["verbalized"])
            except Exception as e:
                print(f"  [Audio playback error]: {e}", file=sys.stderr)

    print("\n" + "=" * 70)
    print(" Review complete. Pass --listen to hear audio synthesis.")
    print("=" * 70 + "\n")

if __name__ == "__main__":
    main()
