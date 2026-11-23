"""3-Tier Branching Decision Tree Engine using Fastino GLiDE-0.4B."""

from dataclasses import dataclass
import os
from pathlib import Path
import sys
from typing import Dict, List, Optional, Tuple

GLIDE_PATH = Path(
    os.path.expanduser(
        "~/.cache/huggingface/hub/models--fastino--GLiDE-0.4B/snapshots/6bda7ab91aeb36dcad6d27828a8871e548c7fc71"
    )
)
if str(GLIDE_PATH) not in sys.path:
    sys.path.insert(0, str(GLIDE_PATH))

import torch
from glide.model import Glide

try:
    from .policies.policy_pack_loader import PACKS_DIR, load_packs
except ImportError:
    from src.daemon.policies.policy_pack_loader import PACKS_DIR, load_packs

SHADOW_MODE = True

TUNED_TREE = {
    "streaming": {
        "n1": {
            "type": "noul",
            "instructions": (
                "Is this page specifically about watching movies, films, television series, "
                "or full video episodes, excluding radio talk shows, audio podcasts, music albums, "
                "and general discussions?"
            ),
            "threshold": 0.50,
        },
        "n2": {
            "type": "choice",
            "instructions": "What is the primary function of this webpage regarding video viewing?",
            "criteria": {
                "movie_tv_video_service": (
                    "A dedicated video streaming service, film catalog, or full TV episode viewing platform."
                ),
                "audio_shopping_news_clip": (
                    "Music or audio streaming (songs/albums/podcasts), physical product or "
                    "comic book sales, news/tech articles, or individual video clips. "
                    "Also exempt search engine results pages, listings of links, and "
                    "informational queries about shows."
                ),
            },
            "target": "movie_tv_video_service",
            "threshold": 0.50,
        },
        "n3": {
            "type": "choice",
            "instructions": "Adversarial shield: Which statement accurately classifies this entity?",
            "criteria": {
                "movie_tv_platform": (
                    "A commercial television network or movie video streaming platform "
                    "(e.g. Netflix, Tubi, iQIYI, Prime Video, Hulu, full TV/movie portal)."
                ),
                "exempt_content": (
                    "Music streaming platform, audio playlist, bookstore/comic shop, YouTube educational "
                    "clip, tech/gaming news media, or personal forum. Also exempt a Google or Bing "
                    "search results page that lists links about a show rather than hosting video playback."
                ),
            },
            "target": "movie_tv_platform",
            "threshold": 0.50,
        },
    },
    "nsfw": {
        "n1": {
            "type": "noul",
            "instructions": (
                "Does this page contain sexually explicit, pornographic, or adult entertainment content, "
                "excluding mainstream music, celebrity news, awards shows, and pop-culture articles?"
            ),
            "threshold": 0.50,
        },
        "n2": {
            "type": "choice",
            "instructions": "Which statement best describes this webpage regarding adult material?",
            "criteria": {
                "adult_explicit": (
                    "The webpage is dedicated sexually explicit, pornographic, or adult erotic content "
                    "intended for sexual entertainment."
                ),
                "safe_context": (
                    "The webpage is mainstream entertainment, celebrity profile or music news, awards "
                    "show coverage, pop culture, political satire, general biography, or safe media."
                ),
            },
            "target": "adult_explicit",
            "threshold": 0.50,
        },
        "n3": {
            "type": "choice",
            "instructions": "Adversarial shield: Which statement accurately classifies this content?",
            "criteria": {
                "nsfw_platform": (
                    "A dedicated adult pornographic website, erotic publication, or adult streaming service."
                ),
                "safe_educational": (
                    "Mainstream music/celebrity news article, pop culture commentary, awards show performance, "
                    "historical biography, editorial cartoon, or general website."
                ),
            },
            "target": "nsfw_platform",
            "threshold": 0.50,
        },
    },
}


def _criteria_from_signals(node: dict, positive: bool) -> dict:
    signals = node.get("signals") or node.get("exempt_signals") or []
    label = "match" if positive else "exempt"
    return {
        label: f"Page matches these criteria: {', '.join(signals)}." if signals
               else "No matching signal.",
        "opposite": "Page does not match the stated criteria.",
    }


def pack_tree_to_glide(tree: dict) -> dict:
    gate, intent, boundary = tree["gate"], tree["intent"], tree["boundary"]
    exempt = ", ".join(boundary.get("exempt_signals", [])) or "none"
    return {
        "n1": {"type": "noul", "instructions": gate["instructions"],
               "threshold": gate.get("glide_threshold", 0.50)},
        "n2": {"type": "choice", "instructions": intent["instructions"],
               "criteria": _criteria_from_signals(intent, True),
               "target": "match", "threshold": intent.get("glide_threshold", 0.50)},
        "n3": {"type": "choice",
               "instructions": "Is this page inside the policy scope, with no exception applying?",
               "criteria": {
                   "in_scope": "Yes, this page is in scope with no exception.",
                   "out_of_scope": f"No, a boundary exception applies: {exempt}.",
               },
               "target": "in_scope",
               "threshold": boundary.get("glide_threshold", 0.50)},
    }


def load_pack_tree(pack_id: str) -> Optional[dict]:
    cfg = load_packs().get(pack_id)
    return pack_tree_to_glide(cfg["tree"]) if cfg else None


@dataclass
class DecisionResult:
    verdict: str  # "ALLOW" or "BLOCK"
    policy_triggered: Optional[str]  # "streaming", "nsfw", or None
    scores_streaming: List[float]
    scores_nsfw: List[float]


class DecisionEngine:
    def __init__(self, device: str = "mps"):
        self.device = device
        self._model: Optional[Glide] = None
        self.shadow_mode = SHADOW_MODE

    def _run_tree(self, text: str, tree: dict) -> Tuple[bool, List[float]]:
        p1 = self.eval_node(text, tree["n1"])
        p2, p3 = 0.0, 0.0
        blocked = False
        if p1 >= tree["n1"]["threshold"]:
            p2 = self.eval_node(text, tree["n2"])
            if p2 >= tree["n2"]["threshold"]:
                p3 = self.eval_node(text, tree["n3"])
                if p3 >= tree["n3"]["threshold"]:
                    blocked = True
        return blocked, [round(p1, 3), round(p2, 3), round(p3, 3)]

    def _get_model(self) -> Glide:
        if self._model is None:
            self._model = Glide.from_pretrained(
                str(GLIDE_PATH), device=self.device, vision=False, dtype=torch.float32
            )
        return self._model

    def eval_node(self, text: str, node_cfg: dict) -> float:
        model = self._get_model()
        ntype = node_cfg["type"]
        if ntype == "noul":
            q = {"q": {"type": "noul", "instructions": node_cfg["instructions"]}}
            res = model.decide(text, q)
            return float(res["answers"]["q"]["noul"])
        else:
            q = {
                "q": {
                    "type": "choice",
                    "instructions": node_cfg["instructions"],
                    "criteria": node_cfg["criteria"],
                }
            }
            res = model.decide(text, q)
            probs = res["answers"]["q"]["probabilities"]
            return float(probs.get(node_cfg["target"], 0.0))

    def evaluate(self, text: str) -> DecisionResult:
        block_s, scores_s = self._run_tree(text, TUNED_TREE["streaming"])
        block_n, scores_n = self._run_tree(text, TUNED_TREE["nsfw"])

        if block_s:
            return DecisionResult("BLOCK", "streaming", scores_s, scores_n)
        if block_n:
            return DecisionResult("BLOCK", "nsfw", scores_s, scores_n)
        return DecisionResult("ALLOW", None, scores_s, scores_n)

    def evaluate_pack(self, text: str, pack_id: str) -> Optional[dict]:
        tree = load_pack_tree(pack_id)
        if tree is None:
            return None
        matched, scores = self._run_tree(text, tree)
        cfg = load_packs().get(pack_id, {})
        return {"pack_id": pack_id, "effect": cfg.get("effect"),
                "matched": matched, "scores": scores}
