"""Chunk-Level Content Evaluation and Density Escalation Module."""

from dataclasses import dataclass
import re
from typing import Dict, List, Optional, Tuple

EROTIC_SUGGESTIVE_TREE = {
    "n1": {
        "type": "noul",
        "instructions": (
            "Does this text snippet contain sexually explicit, pornographic, erotic, "
            "or sexually suggestive language, excluding technical terms or medical discussions?"
        ),
        "threshold": 0.50,
    },
    "n2": {
        "type": "choice",
        "instructions": "Which statement best classifies this text chunk?",
        "criteria": {
            "erotic_or_suggestive": (
                "The text snippet describes sexual acts, adult content, erotic roleplay, "
                "or sexually suggestive/explicit commentary."
            ),
            "benign_commentary": (
                "The text snippet is normal discussion, technical conversation, mainstream news, "
                "or general non-erotic discourse."
            ),
        },
        "target": "erotic_or_suggestive",
        "threshold": 0.50,
    },
    "n3": {
        "type": "choice",
        "instructions": "Adversarial verification: Is this text intended as adult/erotic material?",
        "criteria": {
            "adult_material": "Intended as sexual, pornographic, or adult erotic content.",
            "safe_general": "Safe discourse, software engineering, science, or general topic."
        },
        "target": "adult_material",
        "threshold": 0.50,
    }
}


@dataclass
class ChunkEvalResult:
    flagged_ids: List[str]
    escalate_to_block: bool
    total_evaluated: int
    flagged_count: int
    flag_ratio: float
    details: Dict[str, dict]


class ChunkEvaluator:
    def __init__(self, decision_engine, min_flag_count: int = 3, min_flag_ratio: float = 0.20):
        self.engine = decision_engine
        self.min_flag_count = min_flag_count
        self.min_flag_ratio = min_flag_ratio

    def evaluate_single_chunk(self, text: str) -> Tuple[bool, List[float]]:
        p1 = self.engine.eval_node(text, EROTIC_SUGGESTIVE_TREE["n1"])
        p2, p3 = 0.0, 0.0
        flagged = False
        if p1 >= EROTIC_SUGGESTIVE_TREE["n1"]["threshold"]:
            p2 = self.engine.eval_node(text, EROTIC_SUGGESTIVE_TREE["n2"])
            if p2 >= EROTIC_SUGGESTIVE_TREE["n2"]["threshold"]:
                p3 = self.engine.eval_node(text, EROTIC_SUGGESTIVE_TREE["n3"])
                if p3 >= EROTIC_SUGGESTIVE_TREE["n3"]["threshold"]:
                    flagged = True
        scores = [round(p1, 3), round(p2, 3), round(p3, 3)]
        return flagged, scores

    def evaluate_chunks(self, chunks: List[dict], total_extracted: int = 0) -> ChunkEvalResult:
        flagged_ids = []
        details = {}
        for c in chunks:
            cid = c.get("id")
            text = c.get("text", "").strip()
            if not cid or not text:
                continue
            is_flagged, scores = self.evaluate_single_chunk(text)
            details[cid] = {"flagged": is_flagged, "scores": scores}
            if is_flagged:
                flagged_ids.append(cid)

        flagged_count = len(flagged_ids)
        base_count = total_extracted if total_extracted > 0 else len(chunks)
        flag_ratio = round(flagged_count / max(base_count, 1), 3)

        escalate = (flagged_count >= self.min_flag_count) or (base_count > 0 and flag_ratio > self.min_flag_ratio)
        return ChunkEvalResult(
            flagged_ids=flagged_ids,
            escalate_to_block=escalate,
            total_evaluated=len(chunks),
            flagged_count=flagged_count,
            flag_ratio=flag_ratio,
            details=details,
        )
