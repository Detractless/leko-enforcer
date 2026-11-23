"""Mirror Guard: CTA / redirect / mirror domain verdict inheritance helper."""

import re
from typing import Dict, List, Optional, Tuple
import urllib.parse


CTA_PATTERNS = [
    re.compile(r'(?:continue\s+to|redirecting\s+to|proceed\s+to|watch\s+on|visit\s+our\s+new\s+domain|new\s+address|mirror|alternative\s+link|click\s+here\s+to\s+proceed)\s*[:—\s]*([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})', re.IGNORECASE),
    re.compile(r'https?://([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})[^\s"\']*', re.IGNORECASE)
]


def extract_candidate_target_domains(url: str, html: str, text: str, get_root_domain_fn) -> List[str]:
    """Extract candidate external target domains from CTAs, links, and text."""
    current_root = get_root_domain_fn(url)
    candidates = []

    # 1. Inspect meta refresh tags
    m_refresh = re.search(r'<meta[^>]+http-equiv=["\']refresh["\'][^>]+content=["\'][^"\']*url=([^"\'>]+)["\']', html, re.I)
    if m_refresh:
        ref_url = m_refresh.group(1).strip()
        rd = get_root_domain_fn(ref_url)
        if rd and rd != current_root and rd not in candidates:
            candidates.append(rd)

    # 2. Inspect anchor hrefs with prominent classes or CTA texts
    href_matches = re.findall(r'<a[^>]+href=["\'](https?://[^"\'>]+)["\'][^>]*>(.*?)</a>', html, re.I | re.DOTALL)
    for href, anchor_text in href_matches:
        rd = get_root_domain_fn(href)
        if rd and rd != current_root and rd not in candidates:
            combined = f"{anchor_text} {href}".lower()
            if any(k in combined for k in ["continue", "proceed", "watch", "mirror", "enter", "click", "portal", "movie", "stream", "shuttle"]):
                candidates.append(rd)

    # 3. Inspect CTA regex patterns in text / summary facts
    for pat in CTA_PATTERNS:
        for match in pat.finditer(text):
            candidate_raw = match.group(1)
            rd = get_root_domain_fn(candidate_raw if candidate_raw.startswith("http") else f"https://{candidate_raw}")
            if rd and rd != current_root and rd not in candidates:
                candidates.append(rd)

    return candidates


def check_mirror_verdict_inheritance(
    url: str,
    html: str,
    text: str,
    domain_cache: Dict[str, dict],
    get_root_domain_fn,
    evaluate_domain_purpose_fn
) -> Optional[dict]:
    """Check if the page is a mirror/portal redirecting to an evaluated blocked domain."""
    parsed_path = urllib.parse.urlparse(url).path.lower()
    if "/search" in parsed_path or re.search(r"\b(?:search|web) results\b", text[:3000], re.I):
        return None

    candidates = extract_candidate_target_domains(url, html, text, get_root_domain_fn)
    current_root = get_root_domain_fn(url)

    for target_domain in candidates:
        if target_domain == current_root:
            continue

        # Check existing domain cache
        cached = domain_cache.get(target_domain)
        if cached:
            cached_data = cached.get("data", {})
            if cached_data.get("verdict") == "BLOCK":
                return {
                    "inherited": True,
                    "target_domain": target_domain,
                    "verdict": "BLOCK",
                    "policy_triggered": cached_data.get("policy_triggered") or "streaming",
                    "summary": f"Mirror portal redirecting to blocked target domain {target_domain} ({cached_data.get('summary', '')}).",
                    "scores_streaming": cached_data.get("scores_streaming", [0.9, 0.9, 0.9]),
                    "scores_nsfw": cached_data.get("scores_nsfw", [0.0, 0.0, 0.0])
                }

        # If domain matches brand stem (e.g. shuttletv.su -> shuttletv.pk) or explicit CTA
        current_stem = current_root.split(".")[0]
        target_stem = target_domain.split(".")[0]
        if current_stem == target_stem or any(k in text.lower() for k in ["continue", "shuttle", "stream"]):
            try:
                target_url = f"https://{target_domain}/"
                evaluated = evaluate_domain_purpose_fn(target_domain, target_url, "", "")
                if evaluated.get("verdict") == "BLOCK":
                    return {
                        "inherited": True,
                        "target_domain": target_domain,
                        "verdict": "BLOCK",
                        "policy_triggered": evaluated.get("policy_triggered") or "streaming",
                        "summary": f"Mirror portal redirecting to blocked target domain {target_domain} ({evaluated.get('summary', '')}).",
                        "scores_streaming": evaluated.get("scores_streaming", [0.9, 0.9, 0.9]),
                        "scores_nsfw": evaluated.get("scores_nsfw", [0.0, 0.0, 0.0])
                    }
            except Exception:
                pass

    return None
