"""Challenge and interstitial page detector for the WAE daemon."""

from dataclasses import dataclass
import re
from typing import Optional


CHALLENGE_TITLE_PATTERNS = [
    re.compile(r"just a moment\.{3}", re.I),
    re.compile(r"attention required!?", re.I),
    re.compile(r"security check", re.I),
    re.compile(r"checking your browser", re.I),
    re.compile(r"verify you are human", re.I),
    re.compile(r"access denied\s*\|", re.I),
    re.compile(r"ddos protection by cloudflare", re.I),
    re.compile(r"are you a human", re.I),
    re.compile(r"^loading\b", re.I),
]

CHALLENGE_BODY_SIGNATURES = [
    "cf-chl-",
    "cf-turnstile",
    "cf_challenge",
    "challenge-running",
    "cf-browser-verification",
    "checking if the site connection is secure",
    "enable javascript and cookies to continue",
    "ray id:",
    "g-recaptcha",
    "h-captcha",
    "turnstile.cloudflare.com",
    "cf-error-details",
]


@dataclass
class ChallengeResult:
    is_challenge: bool
    reason: Optional[str] = None
    retry_in_ms: int = 1500


def detect_challenge(title: str, html: str, text: str) -> ChallengeResult:
    """Detect whether a page is an interstitial challenge or bot-check."""
    clean_title = (title or "").strip()
    lower_html = (html or "").lower()
    lower_text = (text or "").lower()
    word_count = len(text.split()) if text else 0

    # 1. Exact or strong title match
    for pat in CHALLENGE_TITLE_PATTERNS:
        if pat.search(clean_title):
            # Verify body has challenge cue or is short
            if any(sig in lower_html for sig in CHALLENGE_BODY_SIGNATURES) or word_count < 100:
                return ChallengeResult(
                    is_challenge=True,
                    reason=f"title_challenge_match:{clean_title[:40]}",
                    retry_in_ms=1500,
                )

    # 2. Strong body verification widget match
    for sig in CHALLENGE_BODY_SIGNATURES:
        if sig in lower_html:
            # Explicit verification widget detected
            if word_count < 60 or "ray id:" in lower_html:
                return ChallengeResult(
                    is_challenge=True,
                    reason=f"body_signature_match:{sig}",
                    retry_in_ms=2000,
                )

    # 3. Interstitial loading text (check visible text, not HTML attribute loading="lazy")
    if text and word_count < 10:
        if "please wait" in lower_text or "verifying your browser" in lower_text:
            return ChallengeResult(
                is_challenge=True,
                reason="sparse_loading_page",
                retry_in_ms=1200,
            )

    return ChallengeResult(is_challenge=False)
