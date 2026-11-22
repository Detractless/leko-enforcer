"""Stage 0 / WAE Local Pipeline Execution Service."""

import json
from pathlib import Path
import re
import subprocess
import sys
import threading
import time
from typing import Dict, List, Optional, Tuple
import urllib.parse
import urllib.request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
EXTRACT_MJS = ROOT / "src/analysis/capture-evaluation/page-regions/extract-single-page.mjs"
LLAMA_SERVER = Path("/Users/calibro/.t3/scratch/2026-10-29-based-on-these-documents-what-d76b9ec3/voice-beta/.runtime/llama-b11980/llama-server")
LFM3_MODEL = Path("/Users/calibro/.t3/scratch/2026-10-29-based-on-these-documents-what-d76b9ec3/voice-beta/models/LFM3-1.7B-Q4_K_M.gguf")

if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

try:
    from .challenge_detector import detect_challenge, CHALLENGE_TITLE_PATTERNS
    from .chunk_evaluator import ChunkEvaluator
    from .decision_engine import DecisionEngine
    from .mirror_guard import check_mirror_verdict_inheritance
    from .policies.policy_pack_loader import PolicyPackEngine
    from .vision_evaluator import VisionEvaluator
except ImportError:
    from src.daemon.challenge_detector import detect_challenge, CHALLENGE_TITLE_PATTERNS
    from src.daemon.chunk_evaluator import ChunkEvaluator
    from src.daemon.decision_engine import DecisionEngine
    from src.daemon.mirror_guard import check_mirror_verdict_inheritance
    from src.daemon.policies.policy_pack_loader import PolicyPackEngine
    from src.daemon.vision_evaluator import VisionEvaluator

FEW_SHOT_COMPLETION_PROMPT = """Webpage text:
[Python programming language - Google Search | www.google.com | /search]
Search query: "Python programming language"
Knowledge panel reference: Python is a programming language.
Top search results:
- Welcome to Python.org
- Python Tutorial - W3Schools
Summary: This webpage is a Google search results page listing search engine links, web articles, and general discussions for "Python programming language".

Webpage text:
[Favorite features in Python? : r/Python | www.reddit.com | /r/Python/comments/xyz]
Subreddit: r/Python
Post Title: Favorite features in Python?
Post Body: What are your favorite improvements in Python?
Summary: This webpage is a Reddit discussion thread where users discuss programming features and community opinions.

Webpage text:
[Chess: Play Chess Online | www.chess.com | /]
Chess.com is an online chess platform where users can play games and solve tactics puzzles.
Summary: This webpage is an online chess platform for playing games and solving tactics puzzles with global players.

Webpage text:
[{doc_title} | {domain} | {path}]
{facts}
Summary: This webpage """


class DaemonService:
    def __init__(self, llama_port: int = 18766):
        self.llama_port = llama_port
        self.llama_proc: Optional[subprocess.Popen] = None
        self.decision_engine = DecisionEngine(device="mps")
        self.policy_packs = PolicyPackEngine()
        self.chunk_evaluator = ChunkEvaluator(self.decision_engine)
        self.vision_evaluator = VisionEvaluator()
        self.domain_cache: Dict[str, dict] = {}
        self.domain_cache_ttl = 3600
        self.in_flight_lock = threading.Lock()
        self.in_flight_events: Dict[str, threading.Event] = {}
        self.in_flight_results: Dict[str, dict] = {}

    @staticmethod
    def get_root_domain(url: str) -> str:
        netloc = urllib.parse.urlparse(url).netloc.lower().split(":")[0]
        parts = netloc.split(".") if netloc else []
        if len(parts) <= 2: return netloc
        if parts[-2] in ("co", "com", "org", "net", "edu", "gov") and len(parts) >= 3: return ".".join(parts[-3:])
        return ".".join(parts[-2:])

    def _save_record(self, record: dict, is_test: bool = False) -> None:
        target_file = ROOT / "test_evaluations.jsonl" if is_test else ROOT / "evaluations.jsonl"
        try:
            with open(target_file, "a", encoding="utf-8") as f:
                f.write(json.dumps(record, ensure_ascii=False) + "\n")
        except Exception: pass

    def evaluate_domain_purpose(self, root_domain: str, url: str, html: str, title: str) -> dict:
        now = time.time()
        cached = self.domain_cache.get(root_domain)
        if cached and (now - cached.get("timestamp", 0) < self.domain_cache_ttl):
            return cached["data"]

        path = urllib.parse.urlparse(url).path
        m = re.search(r'<meta[^>]+(?:name|property)=["\'](?:description|og:description)["\'][^>]+content=["\']([^"\']+)["\']', html, re.I)
        meta_desc = m.group(1).strip() if m else ""
        if path in ("", "/"):
            domain_summary = f"{title}. {meta_desc}".strip() if meta_desc else (title or root_domain)
        else:
            try:
                scheme = urllib.parse.urlparse(url).scheme or "https"
                root_req = urllib.request.Request(f"{scheme}://{root_domain}/", headers={"User-Agent": "Mozilla/5.0"})
                with urllib.request.urlopen(root_req, timeout=3) as resp:
                    root_html = resp.read()[:50000].decode("utf-8", errors="ignore")
                    m_root = re.search(r'<meta[^>]+(?:name|property)=["\'](?:description|og:description)["\'][^>]+content=["\']([^"\']+)["\']', root_html, re.I)
                    m_title = re.search(r'<title[^>]*>([^<]+)</title>', root_html, re.I)
                    root_title = m_title.group(1).strip() if m_title else root_domain
                    root_desc = m_root.group(1).strip() if m_root else ""
                    domain_summary = f"{root_title}. {root_desc}".strip()
            except Exception:
                domain_summary = f"{title}. {meta_desc}".strip() if meta_desc else (title or root_domain)

        domain_input = f"[{root_domain}] {domain_summary}"
        decision = self.decision_engine.evaluate(domain_input)
        res_data = {
            "root_domain": root_domain, "verdict": decision.verdict, "policy_triggered": decision.policy_triggered,
            "summary": domain_summary, "scores_streaming": decision.scores_streaming, "scores_nsfw": decision.scores_nsfw,
        }
        self.domain_cache[root_domain] = {"data": res_data, "timestamp": now}
        return res_data

    def start_llama_server(self):
        if self.llama_proc is not None and self.llama_proc.poll() is None:
            return
        try:
            with urllib.request.urlopen("http://127.0.0.1:18766/health", timeout=1) as resp:
                if resp.status == 200:
                    self.llama_port = 18766
                    return
        except Exception: pass

        self.llama_port = 18766
        cmd = [str(LLAMA_SERVER), "--device", "none", "-m", str(LFM3_MODEL), "--port", str(self.llama_port), "--ctx-size", "2048", "-s", "20261119"]
        self.llama_proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(60):
            try:
                urllib.request.urlopen(f"http://127.0.0.1:{self.llama_port}/health", timeout=1).close()
                return
            except Exception: time.sleep(0.5)
        raise RuntimeError(f"llama-server failed to start on port {self.llama_port}")

    def stop_llama_server(self):
        if self.llama_proc is not None:
            self.llama_proc.terminate()
            try: self.llama_proc.wait(timeout=5)
            except subprocess.TimeoutExpired: self.llama_proc.kill()
            self.llama_proc = None

    def extract_dom(self, url: str, html: str) -> dict:
        req = json.dumps({"url": url, "html": html})
        res = subprocess.run(["node", str(EXTRACT_MJS)], input=req, text=True, capture_output=True, cwd=str(ROOT), check=True, timeout=10)
        return json.loads(res.stdout)

    def summarize_lfm3(self, facts: str, doc_title: str = "", domain: str = "", path: str = "/") -> str:
        self.start_llama_server()
        prompt = FEW_SHOT_COMPLETION_PROMPT.format(doc_title=doc_title, domain=domain, path=path or "/", facts=facts[:1800])
        payload = {"prompt": prompt, "temperature": 0.1, "n_predict": 45, "stop": ["\n", "\n\n", "Webpage text:", "Summary:"]}
        req = urllib.request.Request(f"http://127.0.0.1:{self.llama_port}/completion", data=json.dumps(payload).encode("utf-8"), headers={"content-type": "application/json"})
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            comp = data.get("content", "").strip().split("\n")[0].strip()
            return f"This webpage {comp}".strip()

    def evaluate_page(self, url: str, html: str, title: str = "", image_b64: Optional[str] = None, force_evaluate: bool = False, is_test: bool = False, text: Optional[str] = None, dom_method: Optional[str] = None, chunks: Optional[List[dict]] = None, total_extracted: int = 0) -> dict:
        if not force_evaluate and not is_test:
            with self.in_flight_lock:
                if url in self.in_flight_events:
                    event = self.in_flight_events[url]
                    wait_needed = True
                else:
                    event = threading.Event()
                    self.in_flight_events[url] = event
                    wait_needed = False
            if wait_needed:
                event.wait(timeout=30)
                with self.in_flight_lock:
                    if url in self.in_flight_results: return self.in_flight_results[url]

        try:
            record = self._do_evaluate_page(url, html, title, image_b64, force_evaluate=force_evaluate, is_test=is_test, text=text, dom_method=dom_method, chunks=chunks, total_extracted=total_extracted)
            if not force_evaluate and not is_test:
                with self.in_flight_lock:
                    if len(self.in_flight_results) > 200: self.in_flight_results.clear()
                    self.in_flight_results[url] = record
            return record
        finally:
            if not force_evaluate and not is_test:
                with self.in_flight_lock:
                    ev = self.in_flight_events.pop(url, None)
                    if ev is not None: ev.set()

    def _do_evaluate_page(self, url: str, html: str, title: str = "", image_b64: Optional[str] = None, force_evaluate: bool = False, is_test: bool = False, text: Optional[str] = None, dom_method: Optional[str] = None, chunks: Optional[List[dict]] = None, total_extracted: int = 0) -> dict:
        t0 = time.time()
        root_domain = self.get_root_domain(url)

        # STAGE 1: Website / Root Domain Purpose Evaluation
        domain_info = self.evaluate_domain_purpose(root_domain, url, html, title)
        domain_verdict = domain_info.get("verdict", "ALLOW")
        domain_policy = domain_info.get("policy_triggered")
        domain_summary = domain_info.get("summary", "")

        if domain_verdict == "BLOCK":
            elapsed_ms = round((time.time() - t0) * 1000, 1)
            vision_score = self.vision_evaluator.evaluate_sparse_media(image_b64, 0)[0] if image_b64 else 0.0
            record = {
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "url": url, "title": title or root_domain,
                "verdict": "BLOCK", "policy_triggered": domain_policy, "decision_stage": "domain",
                "domain_verdict": domain_verdict, "domain_summary": domain_summary,
                "page_verdict": None, "summary": domain_summary,
                "scores": {"streaming": domain_info.get("scores_streaming", [0.0, 0.0, 0.0]), "nsfw": domain_info.get("scores_nsfw", [0.0, 0.0, 0.0]), "vision_nsfw": vision_score},
                "vision_nsfw": vision_score, "dom_method": "domain_purpose_fastpath", "item_count": 0, "latency_ms": elapsed_ms,
            }
            self._save_record(record, is_test=is_test)
            return record

        # Check Mirror / Redirect Target Domain Inheritance
        mirror = check_mirror_verdict_inheritance(url, html, text or "", self.domain_cache, self.get_root_domain, self.evaluate_domain_purpose)
        if mirror and mirror.get("verdict") == "BLOCK":
            elapsed_ms = round((time.time() - t0) * 1000, 1)
            record = {
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "url": url, "title": title or root_domain, "verdict": "BLOCK",
                "policy_triggered": mirror.get("policy_triggered"), "decision_stage": "domain",
                "domain_verdict": "BLOCK", "domain_summary": mirror.get("summary"),
                "page_verdict": None, "summary": mirror.get("summary"),
                "scores": {"streaming": mirror.get("scores_streaming", [0.9, 0.9, 0.9]), "nsfw": mirror.get("scores_nsfw", [0.0, 0.0, 0.0]), "vision_nsfw": 0.0},
                "vision_nsfw": 0.0, "dom_method": "domain_purpose_fastpath", "item_count": 0, "latency_ms": elapsed_ms,
            }
            self._save_record(record, is_test=is_test)
            return record

        # STAGE 2: Specific Page Purpose Evaluation
        if not force_evaluate and any(pat.search(title or "") for pat in CHALLENGE_TITLE_PATTERNS):
            pre_check = detect_challenge(title, html, "")
            if pre_check.is_challenge:
                return {"verdict": "PENDING", "reason": pre_check.reason, "retry_in_ms": pre_check.retry_in_ms, "latency_ms": round((time.time() - t0) * 1000, 1)}

        if dom_method and dom_method.startswith("adapter:") and text:
            doc_title = title or root_domain
            facts = text
            word_count = len(facts.split())
            method = dom_method
            item_count = len(re.findall(r'^- ', text, re.M)) if "search" in dom_method else 1
        else:
            dom_data = self.extract_dom(url, html)
            doc_title = dom_data.get("title") or title
            facts = dom_data.get("facts") or ""
            word_count = len(facts.split())
            method = dom_data.get("method") or "fallback_text"
            item_count = dom_data.get("item_count", 0)

        if word_count < 25 and not (dom_method and dom_method.startswith("adapter:")):
            meta_desc = ""
            m = re.search(r'<meta[^>]+(?:name|property)=["\'](?:description|og:description)["\'][^>]+content=["\']([^"\']+)["\']', html, re.I)
            if m: meta_desc = m.group(1).strip()
            m_title = re.search(r"""<meta[^>]+(?:property|name)=['"]og:title['"][^>]+content=['"]([^'"]+)['"]""", html, re.I)
            if m_title and len(m_title.group(1).strip()) > len(doc_title): doc_title = m_title.group(1).strip()
            if meta_desc:
                facts = f"Title: {doc_title}\nDescription: {meta_desc}\n{facts}".strip()
                word_count = len(facts.split())

        if not force_evaluate:
            post_check = detect_challenge(doc_title, html, facts)
            if post_check.is_challenge:
                return {"verdict": "PENDING", "reason": post_check.reason, "retry_in_ms": post_check.retry_in_ms, "latency_ms": round((time.time() - t0) * 1000, 1)}

        vision_score, ocr_text = self.vision_evaluator.evaluate_sparse_media(image_b64, word_count)
        if ocr_text: facts = f"{facts}\nOCR Text: {ocr_text}"

        domain = urllib.parse.urlparse(url).netloc.lower()
        path = urllib.parse.urlparse(url).path or "/"

        if (len(facts.strip()) < 100 or word_count < 10) and not ocr_text:
            summary = f"This webpage is {doc_title}" if doc_title else "This webpage is an unrendered or empty page."
        else:
            summary = self.summarize_lfm3(facts, doc_title=doc_title, domain=domain, path=path)

        glide_input = f"[{doc_title} | {domain}] {summary}"
        decision = self.decision_engine.evaluate(glide_input)
        page_verdict = decision.verdict
        verdict = page_verdict
        policy = decision.policy_triggered
        pack_result = self.policy_packs.evaluate_with_glide(
            self.decision_engine, glide_input, url=url, title=doc_title,
            summary=summary, hard_safety_verdict=decision.verdict,
            hard_safety_policy=decision.policy_triggered or "glide")
        if pack_result.get("enforce") and verdict != "BLOCK":
            verdict, policy = pack_result["verdict"], pack_result["policy_triggered"]

        if vision_score >= 0.70 and verdict != "BLOCK":
            verdict, policy = "BLOCK", "nsfw_visual"

        flagged_blocks = []
        chunk_eval_meta = None
        if chunks:
            c_res = self.chunk_evaluator.evaluate_chunks(chunks, total_extracted=total_extracted)
            flagged_blocks = c_res.flagged_ids
            chunk_eval_meta = {"flagged_ids": c_res.flagged_ids, "flagged_count": c_res.flagged_count, "flag_ratio": c_res.flag_ratio, "escalate_to_block": c_res.escalate_to_block}
            if c_res.escalate_to_block and verdict != "BLOCK":
                verdict, policy = "BLOCK", "nsfw_chunk_density"

        elapsed_ms = round((time.time() - t0) * 1000, 1)
        record = {
            "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "url": url, "title": doc_title, "verdict": verdict,
            "policy_triggered": policy, "decision_stage": "page",
            "domain_verdict": domain_verdict, "domain_summary": domain_summary,
            "page_verdict": page_verdict, "summary": summary,
            "scores": {"streaming": decision.scores_streaming, "nsfw": decision.scores_nsfw, "vision_nsfw": vision_score},
            "vision_nsfw": vision_score, "dom_method": method, "flagged_blocks": flagged_blocks,
            "chunk_eval": chunk_eval_meta, "item_count": item_count, "latency_ms": elapsed_ms,
            "shadow_mode": pack_result["shadow_mode"],
            "policy_packs": {
                "verdict": pack_result["verdict"], "mode": pack_result["mode"],
                "policy_triggered": pack_result["policy_triggered"],
                "allow_exception": pack_result["allow_exception"],
                "shadow_mode": pack_result["shadow_mode"],
                "packs": {p["id"]: {"matched": p["matched"], "score": p["score"],
                                    "glide_scores": p.get("glide_scores")}
                          for p in pack_result["packs"]},
            },
        }
        self._save_record(record, is_test=is_test)
        return record
