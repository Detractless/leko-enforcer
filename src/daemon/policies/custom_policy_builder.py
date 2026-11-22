"""Compile a plain-English boundary into a declarative 3-tier GLiDE pack config."""

import json
from pathlib import Path
import re
import sys
from typing import Optional

try:
    from .policy_pack_loader import PACKS_DIR
except ImportError:
    from src.daemon.policies.policy_pack_loader import PACKS_DIR

STOPWORDS = {
    "a", "an", "and", "are", "but", "for", "from", "into", "not", "only", "or",
    "that", "the", "their", "this", "to", "with", "you", "your", "just", "about",
    "allow", "block", "site", "sites", "page", "pages", "when", "unless",
}
ALLOW_INTENT = ["tutorial", "guide", "learn", "study", "read", "documentation", "reference", "course"]
BLOCK_INTENT = ["play", "buy", "stream", "download", "watch", "shop", "order", "browse", "sign up"]
ALLOW_OVERRIDES = ["games", "music", "shopping", "food_delivery"]


def slugify(text: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", (text or "").lower()).strip("-")
    return (slug or "policy")[:40].rstrip("-")


def extract_signals(boundary: str, limit: int = 8) -> list:
    tokens = re.findall(r"[a-z][a-z0-9+]{2,}", (boundary or "").lower())
    signals = []
    for token in tokens:
        if token in STOPWORDS or token in signals:
            continue
        signals.append(token)
        if len(signals) >= limit:
            break
    return signals


def build_pack(boundary: str, effect: str, name: Optional[str] = None) -> dict:
    if effect not in ("allow", "block"):
        raise ValueError("effect must be 'allow' or 'block'")
    signals = extract_signals(boundary)
    if not signals:
        raise ValueError("boundary has no usable terms")
    intent_signals = signals + (ALLOW_INTENT if effect == "allow" else BLOCK_INTENT)
    return {
        "id": f"custom_{slugify(boundary)}",
        "name": name or boundary[:80],
        "effect": effect,
        "hard_safety": False,
        "version": 1,
        "overrides": ALLOW_OVERRIDES if effect == "allow" else [],
        "tree": {
            "gate": {
                "node": "gate",
                "instructions": f"Is this page about: {boundary.strip()}?",
                "signals": signals,
                "min_hits": 1,
            },
            "intent": {
                "node": "intent",
                "instructions": f"Confirm the page's purpose matches: {boundary.strip()}.",
                "signals": intent_signals,
                "min_hits": 1,
            },
            "boundary": {
                "node": "boundary",
                "instructions": "Do not override hard safety policies (nsfw, streaming).",
                "exempt_signals": [],
                "veto_on_exempt": False,
            },
        },
    }


def save_pack(cfg: dict, packs_dir: Path = PACKS_DIR) -> Path:
    packs_dir.mkdir(parents=True, exist_ok=True)
    path = packs_dir / f"{cfg['id']}.json"
    path.write_text(json.dumps(cfg, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return path


def main(argv=None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("boundary", help="plain-English boundary, e.g. 'Block impulse shopping sites'")
    parser.add_argument("--effect", choices=("allow", "block"), required=True)
    parser.add_argument("--name", default=None)
    args = parser.parse_args(argv)
    cfg = build_pack(args.boundary, args.effect, args.name)
    path = save_pack(cfg)
    print(json.dumps({"saved": str(path), "id": cfg["id"], "effect": cfg["effect"]}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
