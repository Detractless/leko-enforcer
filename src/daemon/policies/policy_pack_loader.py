"""Declarative bidirectional Policy Pack loader and composition engine.

Pure standard library so the offline benchmark runs without torch/GLiDE.
Each pack is a JSON config with effect=block|allow and a 3-tier tree:
Gate -> Intent -> Boundary. Hard safety packs are never overridden by allow
packs unless config.json explicitly sets allow_overrides_hard_safety=true.
"""

from dataclasses import dataclass, field
import json
from pathlib import Path
import re
from typing import Dict, List, Optional

PACKS_DIR = Path(__file__).resolve().parents[1] / "policy_packs"
CONFIG_PATH = PACKS_DIR / "config.json"
MODES = ("block_unless_allowed", "strict_allow_only")


def normalize(text: str) -> str:
    return re.sub(r"\s+", " ", re.sub(r"[^a-z0-9]+", " ", (text or "").lower())).strip()


def build_context(url: str = "", title: str = "", text: str = "",
                  summary: str = "", meta: str = "") -> str:
    return normalize(" ".join([url, title, meta, summary, text]))


def load_config(packs_dir: Path = PACKS_DIR) -> dict:
    cfg = json.loads(CONFIG_PATH.read_text(encoding="utf-8")) if CONFIG_PATH.exists() else {}
    cfg.setdefault("mode", "block_unless_allowed")
    cfg.setdefault("default_deny", False)
    cfg.setdefault("shadow_mode", True)
    cfg.setdefault("hard_safety", ["nsfw"])
    cfg.setdefault("hard_safety_packs", ["nsfw", "streaming"])
    cfg.setdefault("allow_overrides_hard_safety", False)
    cfg.setdefault("enabled_packs", ["nsfw", "streaming"])
    cfg.setdefault("disabled_packs", [])
    return cfg


def load_packs(packs_dir: Path = PACKS_DIR) -> Dict[str, dict]:
    packs = {}
    for path in sorted(packs_dir.rglob("*.json")):
        if path.name == "config.json":
            continue
        cfg = json.loads(path.read_text(encoding="utf-8"))
        if cfg.get("effect") not in ("block", "allow"):
            raise ValueError(f"{path.name}: effect must be 'block' or 'allow'")
        tree = cfg.get("tree", {})
        if not all(tier in tree for tier in ("gate", "intent", "boundary")):
            raise ValueError(f"{path.name}: tree needs gate, intent, boundary")
        packs[cfg["id"]] = cfg
    return packs


@dataclass
class PackResult:
    id: str
    name: str
    effect: str
    matched: bool
    score: float
    gate: float
    intent: float
    exempted: bool
    evidence: dict = field(default_factory=dict)
    overrides: List[str] = field(default_factory=list)
    hard_safety: bool = False
    boundary: float = 0.0
    allow_exemptions: List[str] = field(default_factory=list)
    exemption_blocked_by: List[str] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "id": self.id, "effect": self.effect, "matched": self.matched,
            "score": self.score, "gate": self.gate, "intent": self.intent,
            "boundary": self.boundary, "exempted": self.exempted,
            "evidence": self.evidence,
        }


def _hits(ctx: str, signals: List[str]) -> List[str]:
    return [s for s in signals if s and s in ctx]


def evaluate_pack(cfg: dict, ctx: str) -> PackResult:
    tree = cfg["tree"]
    gate_node, intent_node = tree["gate"], tree["intent"]
    boundary = tree["boundary"]
    gate_hits = _hits(ctx, gate_node.get("signals", []))
    intent_hits = _hits(ctx, intent_node.get("signals", []))
    exempt_hits = _hits(ctx, boundary.get("exempt_signals", []))
    gate = min(1.0, round(len(gate_hits) / max(1, int(gate_node.get("min_hits", 1))), 3))
    intent = min(1.0, round(len(intent_hits) / max(1, int(intent_node.get("min_hits", 1))), 3))
    exempted = bool(exempt_hits) and bool(boundary.get("veto_on_exempt", True))
    matched = gate >= 1.0 and intent >= 1.0 and not exempted
    return PackResult(
        id=cfg["id"], name=cfg.get("name", cfg["id"]), effect=cfg["effect"],
        matched=matched, score=round((gate + intent) / 2, 3), gate=gate,
        intent=intent, exempted=exempted,
        evidence={"gate": gate_hits, "intent": intent_hits, "boundary": exempt_hits},
        overrides=list(cfg.get("overrides", [])),
        hard_safety=bool(cfg.get("hard_safety", False)),
        allow_exemptions=list(cfg.get("allow_exemptions", [])),
        exemption_blocked_by=list(cfg.get("exemption_blocked_by", [])),
    )


@dataclass
class CompositionResult:
    verdict: str
    policy_triggered: Optional[str]
    mode: str
    shadow_mode: bool
    enforce: bool
    packs: List[dict]
    allow_exception: Optional[str] = None
    overrode: List[str] = field(default_factory=list)
    reason: str = ""

    def to_dict(self) -> dict:
        return {
            "verdict": self.verdict, "policy_triggered": self.policy_triggered,
            "mode": self.mode, "shadow_mode": self.shadow_mode,
            "enforce": self.enforce, "allow_exception": self.allow_exception,
            "overrode": self.overrode, "reason": self.reason, "packs": self.packs,
        }


def compose(results: List[PackResult], config: dict, mode: str,
            hard_safety_verdict: Optional[str] = None,
            hard_safety_policy: Optional[str] = None) -> CompositionResult:
    shadow = bool(config.get("shadow_mode", True))
    base = dict(mode=mode, shadow_mode=shadow, enforce=not shadow,
                packs=[r.to_dict() for r in results])
    hs_block = hard_safety_verdict == "BLOCK"

    if hs_block and not config.get("allow_overrides_hard_safety", False):
        return CompositionResult("BLOCK", f"hard_safety:{hard_safety_policy or 'safety'}",
                                 reason="hard_safety_non_overridable", **base)

    block_matches = [r for r in results if r.effect == "block" and r.matched]
    allow_matches = [r for r in results if r.effect == "allow" and r.matched]

    if mode == "strict_allow_only" or config.get("default_deny", False):
        if allow_matches:
            return CompositionResult("ALLOW", allow_matches[0].id,
                                     reason="allow_pack_match", **base)
        return CompositionResult("BLOCK", "default_deny",
                                 reason="no_active_allow_pack_matched", **base)

    if not block_matches:
        if allow_matches:
            return CompositionResult("ALLOW", allow_matches[0].id,
                                     reason="allow_pack_match", **base)
        return CompositionResult("ALLOW", None, reason="no_block_pack_matched", **base)

    all_block_ids = {r.id for r in block_matches}
    neutralized = {}
    for block in block_matches:
        if any(pid in all_block_ids for pid in block.exemption_blocked_by):
            continue
        for allow in allow_matches:
            if allow.id in block.allow_exemptions or block.id in allow.overrides:
                neutralized[block.id] = allow.id
                break
    effective = [r for r in block_matches if r.id not in neutralized]

    if not effective:
        first_allow = next(iter(neutralized.values()),
                           allow_matches[0].id if allow_matches else None)
        return CompositionResult("ALLOW", first_allow, allow_exception=first_allow,
                                 overrode=sorted(neutralized),
                                 reason="explicit_allow_exception", **base)
    return CompositionResult("BLOCK", sorted(r.id for r in effective)[0],
                             reason="block_pack_matched_no_exception", **base)


class PolicyPackEngine:
    def __init__(self, packs_dir: Path = PACKS_DIR, config: Optional[dict] = None):
        self.packs = load_packs(packs_dir)
        self.config = config or load_config()
        enabled = self.config.get("enabled_packs")
        if enabled is None:
            enabled = self.config.get("active_packs", list(self.packs))
        self.active = [i for i in enabled if i in self.packs]

    def evaluate(self, url: str = "", title: str = "", text: str = "",
                 summary: str = "", meta: str = "", context: Optional[str] = None,
                 mode: Optional[str] = None,
                 hard_safety_verdict: Optional[str] = None,
                 hard_safety_policy: Optional[str] = None) -> dict:
        ctx = context if context is not None else build_context(
            url=url, title=title, text=text, summary=summary, meta=meta)
        results = [evaluate_pack(self.packs[i], ctx) for i in self.active]
        return compose(results, self.config, mode or self.config["mode"],
                       hard_safety_verdict, hard_safety_policy).to_dict()

    def evaluate_with_glide(self, glide_engine, text: str, url: str = "", title: str = "",
                            summary: str = "", mode: Optional[str] = None,
                            hard_safety_verdict: Optional[str] = None,
                            hard_safety_policy: Optional[str] = None) -> dict:
        results = []
        for pid in self.active:
            cfg = self.packs[pid]
            g = glide_engine.evaluate_pack(text, pid) or {}
            scores = g.get("scores", [0.0, 0.0, 0.0])
            results.append(PackResult(
                id=pid, name=cfg.get("name", pid), effect=cfg["effect"],
                matched=bool(g.get("matched", False)),
                score=round(sum(scores) / 3, 3), gate=scores[0], intent=scores[1],
                exempted=False,
                evidence={"glide": [round(s, 3) for s in scores]},
                overrides=list(cfg.get("overrides", [])),
                hard_safety=bool(cfg.get("hard_safety", False)),
                boundary=scores[2],
                allow_exemptions=list(cfg.get("allow_exemptions", [])),
                exemption_blocked_by=list(cfg.get("exemption_blocked_by", [])),
            ))
        comp = compose(results, self.config, mode or self.config["mode"],
                       hard_safety_verdict, hard_safety_policy)
        out = comp.to_dict()
        for pack, result in zip(out["packs"], results):
            pack["glide_scores"] = [result.gate, result.intent, result.boundary]
        return out
