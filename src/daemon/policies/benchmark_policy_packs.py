"""Offline benchmark: 11 declarative policy packs x 2 composition modes."""

import argparse
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from src.daemon.policies.policy_pack_loader import (  # noqa: E402
    PolicyPackEngine, build_context, load_config, load_packs,
)

CAPTURED = ROOT / "evaluations.jsonl"
REPORT = ROOT / "policy_pack_benchmark_report.json"
MODES = ("block_unless_allowed", "strict_allow_only")


def case(cid, url, title, text, positives, bua, strict, hard_safety=None, policy=None,
         note="", recorded=None):
    return {"id": cid, "url": url, "title": title, "text": text,
            "positives": set(positives), "expected": {"block_unless_allowed": bua, "strict_allow_only": strict},
            "hard_safety": hard_safety, "policy": policy, "note": note, "recorded": recorded}


TARGETED = [
    case("d2l_education_email",
         "https://kennesaw.view.usg.edu/d2l/le/content/123/viewContent/456/View",
         "D2L Brightspace - Kennesaw State University",
         "Course lecture notes and assignment instructions for students. Inbox announcements and unread messages.",
         ["education", "email"], "ALLOW", "ALLOW",
         note="D2L must hit education and email (D2L mail)"),
    case("wsj_news",
         "https://www.wsj.com/finance/markets", "The Wall Street Journal - Markets",
         "Financial news and current events reporting on the stock market. Headlines and analysis from journalists.",
         ["news"], "ALLOW", "ALLOW"),
    case("yahoo_finance_news",
         "https://finance.yahoo.com/quote/AAPL", "Yahoo Finance - Apple Inc",
         "Stock market news, financial data, investing analysis and coverage of current events.",
         ["news"], "ALLOW", "ALLOW"),
    case("github_work",
         "https://github.com/acme/repo", "GitHub - acme/repo: build tools",
         "Software development repository with source code, documentation, commits and pull requests.",
         ["work", "coding"], "ALLOW", "ALLOW"),
    case("huggingface_work",
         "https://huggingface.co/models/bert", "Hugging Face Models",
         "Machine learning model repository and documentation with code examples for developers.",
         ["work", "coding"], "ALLOW", "ALLOW"),
    case("math_theorems",
         "https://example.edu/math/theorem", "Introduction to Proofs",
         "Mathematics: theorem and proof. Statistics, equation and problem solving with worked examples and exercises.",
         ["math"], "ALLOW", "ALLOW"),
    case("language_arts_writing",
         "https://example.org/writing/essay", "Composition and Grammar Reference",
         "Literature and writing reference: grammar, composition, essay structure, close reading and poetry analysis.",
         ["language_arts"], "ALLOW", "ALLOW"),
    case("youtube_math_education",
         "https://www.youtube.com/watch?v=linear-algebra", "Linear Algebra and Neural Networks - MIT OCW Lecture",
         "Educational tutorial on mathematics: equation, proof and problem solving with examples. Learn AI fundamentals.",
         ["math", "education"], "ALLOW", "ALLOW",
         note="block_unless_allowed must allow educational math/AI video"),
    case("youtube_gaming_entertainment",
         "https://www.youtube.com/watch?v=minecraft", "Playing Minecraft for 8 Hours - Livestream",
         "Livestream of video game gameplay. Watch stream now, multiplayer session on twitch, play free and top up.",
         ["games"], "BLOCK", "BLOCK",
         note="general entertainment must be blocked"),
    case("suno_music",
         "https://suno.com/create", "Suno - AI Music Generator",
         "Generate music and create songs with our audio streaming playlist. Listen now and download song.",
         ["music"], "BLOCK", "BLOCK"),
    case("addictinggames",
         "https://www.addictinggames.com/game", "Addicting Games - Play Free",
         "Play now free video games and game store with multiplayer. Download game and play free.",
         ["games"], "BLOCK", "BLOCK"),
    case("casino_gambling",
         "https://luckybet.casino/slots", "LuckyBet Casino - Place Bet",
         "Online gambling and sports betting with slots and poker. Place bet now, deposit and claim sign up bonus.",
         ["gambling"], "BLOCK", "BLOCK"),
    case("amazon_shopping",
         "https://www.amazon.com/s?k=shoes", "Amazon.com - Online Shopping",
         "Marketplace product listing with price and discount. Buy now, add to cart and checkout your order online.",
         ["shopping"], "BLOCK", "BLOCK"),
    case("doordash_food",
         "https://www.doordash.com/store", "DoorDash - Food Delivery",
         "Order food delivery from restaurant menu with promo code and delivery time. Place order now.",
         ["food_delivery"], "BLOCK", "BLOCK"),
    case("game_dev_education",
         "https://www.youtube.com/watch?v=game-dev", "Game Development Tutorial - Intro to Engines",
         "Educational tutorial about game development and computer science course concepts. Learn to build with code and documentation.",
         ["education", "work"], "ALLOW", "ALLOW",
         note="boundary exemption keeps game-dev education out of games pack"),
    case("dev_music_exception",
         "https://www.youtube.com/watch?v=code-songs", "Coding Music Playlist - Developer",
         "Music streaming playlist of songs for developers. Documentation and code for software engineering projects.",
         ["music", "work"], "ALLOW", "ALLOW",
         note="work allow pack explicitly overrides matched music block pack"),
    case("hard_safety_non_overridable",
         "https://www.youtube.com/watch?v=linear-algebra", "Linear Algebra and Neural Networks - MIT OCW Lecture",
         "Educational tutorial on mathematics: equation, proof and problem solving with examples. Learn AI fundamentals.",
         ["math", "education"], "BLOCK", "BLOCK", hard_safety="BLOCK", policy="nsfw",
          note="hard safety BLOCK must beat allow packs in both modes"),
    case("v2_coding_github_gstack", "https://github.com/garrytan/gstack",
         "garrytan/gstack: Use Garry Tan's exact Claude Code setup",
         "GitHub repository with source code, tools and documentation for a developer setup. Browse code and commits.",
         ["coding", "work"], "ALLOW", "ALLOW", note="coding blocks repos; work exemption"),
    case("v2_coding_hf_model", "https://huggingface.co/LiquidAI/LFM3-1.7B",
         "LiquidAI/LFM3-1.7B - Hugging Face Model Card",
         "Hugging Face model card with model weights, documentation and code for LFM3-1.7B.",
         ["coding", "work"], "ALLOW", "ALLOW", note="model card; work exemption"),
    case("v2_coding_stackoverflow", "https://stackoverflow.com/questions/123/python",
         "Stack Overflow - Python Question",
         "Stack Overflow coding Q&A thread with code examples and answers from developers.",
         ["coding", "work"], "ALLOW", "ALLOW"),
    case("v2_coding_reddit_localllama", "https://www.reddit.com/r/LocalLLaMA/",
         "r/LocalLLaMA - local model discussion",
         "Reddit community discussion about local AI model fine-tuning, code and repositories. Browse posts and comments.",
         ["coding", "social_media"], "BLOCK", "BLOCK", note="coding blocks r/LocalLLaMA"),
    case("v2_ai_chatgpt", "https://chatgpt.com/", "ChatGPT",
         "ChatGPT interactive AI chatbot. Chat with the assistant, ask questions and get generated replies.",
         ["ai_chatbots"], "BLOCK", "BLOCK"),
    case("v2_ai_claude", "https://claude.ai/chat", "Claude - AI Chat",
         "Interactive AI chat interface for conversation and roleplay with an AI assistant.",
         ["ai_chatbots"], "BLOCK", "BLOCK"),
    case("v2_ai_static_docs", "https://docs.example.ai/models/lfm3",
         "LFM3-1.7B model documentation",
         "Static technical documentation and model weights reference page for a language model.",
         ["work"], "ALLOW", "ALLOW", note="ai_chatbots negative: static docs"),
    case("v2_social_instagram", "https://www.instagram.com/", "Instagram",
         "Social media feed with posts, photos and stories. Browse the timeline of updates.",
         ["social_media"], "BLOCK", "BLOCK"),
    case("v2_social_reddit_feed", "https://www.reddit.com/", "Reddit - The heart of the internet",
         "Reddit home feed with posts, comments and communities to browse and scroll.",
         ["social_media"], "BLOCK", "BLOCK"),
    case("v2_social_email_exception", "https://mail.google.com/mail/u/0/#inbox",
         "Gmail Inbox - social notifications",
         "Inbox with email messages and direct message threads, plus social media notifications.",
         ["email"], "ALLOW", "ALLOW",
         note="email allow pack exempts social_media inbox route"),
    case("v2_manga_asurascans", "https://asurascans.com/comics", "Asura Scans - Read Manga Online",
         "Read manga and manhwa comics online. Browse latest chapters in the comic reader.",
         ["manga_comics"], "BLOCK", "BLOCK"),
    case("v2_manga_news_reference", "https://en.wikipedia.org/wiki/Manga", "Manga - Wikipedia",
         "Reference article about manga history and the comics industry, with news and encyclopedia details.",
         ["news"], "ALLOW", "ALLOW", note="manga negative: reference/news page"),
]


def captured_cases(limit: int = 4, all_rows: bool = False):
    if not CAPTURED.exists():
        return []
    wanted = ("cachyos.org", "reddit.com", "lubuntu.net", "arxiv.org")
    out = []
    for line in CAPTURED.read_text(encoding="utf-8").splitlines():
        if not line.strip() or (not all_rows and len(out) >= limit):
            continue
        try:
            rec = json.loads(line)
        except Exception:
            continue
        url = rec.get("url", "")
        if not all_rows and not any(w in url for w in wanted):
            continue
        verdict = rec.get("verdict", "ALLOW")
        out.append(case("captured_" + str(len(out)), url, rec.get("title", ""),
                        rec.get("summary", ""), [], "ALLOW", "BLOCK",
                        note="captured Variant 17 URL (no per-pack ground truth)",
                        recorded=verdict))
    return out


def fmt(value):
    return "n/a" if value is None else f"{value:.2f}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--real", action="store_true",
                        help="use real GLiDE-0.4B inference in the torch environment")
    args = parser.parse_args()
    real = args.real

    packs = load_packs()
    config = load_config()
    config["enabled_packs"] = sorted(packs)
    engine = PolicyPackEngine(config=config)
    glide_engine = None
    if real:
        from src.daemon.decision_engine import DecisionEngine
        glide_engine = DecisionEngine(device="mps")
    cases = TARGETED + captured_cases(all_rows=real)
    ids = sorted(packs)
    labeled_ids = {c["id"] for c in TARGETED}

    stats = {pid: {"tp": 0, "fp": 0, "fn": 0, "tn": 0} for pid in ids}
    mode_stats = {m: {"correct": 0, "total": 0} for m in MODES}
    details, borderline, tuning, score_rows = [], [], [], []
    captured_hits, captured_parity = {pid: 0 for pid in ids}, {"match": 0, "total": 0}

    for c in cases:
        input_text = f"[{c['title']} | {c['url']}] {c['text']}".strip()
        results = {}
        for m in MODES:
            if real:
                results[m] = engine.evaluate_with_glide(
                    glide_engine, input_text, mode=m,
                    hard_safety_verdict=c["hard_safety"],
                    hard_safety_policy=c["policy"])
            else:
                results[m] = engine.evaluate(
                    context=build_context(url=c["url"], title=c["title"], text=c["text"]),
                    mode=m, hard_safety_verdict=c["hard_safety"],
                    hard_safety_policy=c["policy"])
        primary = results[MODES[0]]
        matched = {p["id"] for p in primary["packs"] if p["matched"]}
        is_labeled = c["id"] in labeled_ids

        if is_labeled:
            for pid in ids:
                got, want = pid in matched, pid in c["positives"]
                key = "tp" if got and want else "fp" if got else "fn" if want else "tn"
                stats[pid][key] += 1
            for m in MODES:
                mode_stats[m]["total"] += 1
                if results[m]["verdict"] == c["expected"][m]:
                    mode_stats[m]["correct"] += 1
        else:
            for pid in matched:
                captured_hits[pid] += 1
            if c["recorded"]:
                captured_parity["total"] += 1
                if primary["verdict"] == c["recorded"]:
                    captured_parity["match"] += 1

        for p in primary["packs"]:
            if real:
                score_rows.append({"case": c["id"], "pack": p["id"],
                                   "scores": [p["gate"], p["intent"], p["boundary"]],
                                   "matched": p["matched"]})
                if is_labeled and p["matched"] != (p["id"] in c["positives"]):
                    want = p["id"] in c["positives"]
                    if want and p["gate"] < 0.5:
                        fix = "lower Gate threshold or reword Gate instructions"
                    elif want and p["intent"] < 0.5:
                        fix = "lower Intent threshold or reword Intent criteria"
                    elif want:
                        fix = "lower Boundary threshold or reword in-scope phrasing"
                    else:
                        fix = "raise threshold or tighten Gate/Intent phrasing"
                    tuning.append({"case": c["id"], "pack": p["id"], "expected": want,
                                   "got": p["matched"], "scores": [p["gate"], p["intent"], p["boundary"]],
                                   "suggestion": fix})
            elif p["gate"] >= 1.0 and (p["intent"] < 1.0 or p["exempted"]):
                borderline.append({"case": c["id"], "pack": p["id"],
                                   "gate": p["gate"], "intent": p["intent"],
                                   "exempted": p["exempted"], "reason": "near_miss"})
        for m in MODES:
            r = results[m]
            if r["allow_exception"] or (r["policy_triggered"] or "").startswith("hard_safety:"):
                borderline.append({"case": c["id"], "mode": m, "verdict": r["verdict"],
                                   "policy": r["policy_triggered"],
                                   "exception": r["allow_exception"], "reason": "composition_override"})
        details.append({"case": c["id"], "positives": sorted(c["positives"]),
                        "matched": sorted(matched), "note": c["note"],
                        "verdicts": {m: results[m]["verdict"] for m in MODES},
                        "expected": {m: c["expected"][m] for m in MODES}})

    backend = "GLiDE-0.4B (real)" if real else "lexical offline"
    print("=" * 88)
    print(f"POLICY PACK BENCHMARK [{backend}]  |  {len(cases)} cases "
          f"({len(labeled_ids)} labeled, {len(cases)-len(labeled_ids)} captured)  |  "
          f"{len(ids)} packs  |  2 modes  |  shadow_mode={config['shadow_mode']}")
    print("=" * 88)

    if real:
        print("\nREAL GLiDE [gate, intent, boundary] SCORES — labeled cases")
        print("format: pack=G/I/B")
        for c in TARGETED:
            row = [r for r in score_rows if r["case"] == c["id"]]
            line = " ".join(f"{r['pack'][:4]}={r['scores'][0]:.2f}/{r['scores'][1]:.2f}/{r['scores'][2]:.2f}"
                            for r in row)
            print(f"  {c['id']:<30} {line}")
        print("\nCAPTURED VARIANT 17 MATCH COUNTS (no per-pack ground truth):")
        print("  " + ", ".join(f"{pid}:{captured_hits[pid]}" for pid in ids))
        if captured_parity["total"]:
            print(f"  composition parity with recorded verdicts: "
                  f"{captured_parity['match']}/{captured_parity['total']}")

    print("\nPRECISION / RECALL (labeled cases only)")
    print(f"{'Pack':<16}{'Effect':<8}{'TP':>4}{'FP':>4}{'FN':>4}{'TN':>5}"
          f"{'Precision':>11}{'Recall':>9}{'F1':>8}")
    print("-" * 88)
    per_pack = {}
    for pid in ids:
        s = stats[pid]
        tp, fp, fn = s["tp"], s["fp"], s["fn"]
        precision = tp / (tp + fp) if tp + fp else None
        recall = tp / (tp + fn) if tp + fn else 1.0 if fp == 0 else None
        f1 = (2 * precision * recall / (precision + recall)
              if precision is not None and recall is not None and precision + recall else 0.0)
        per_pack[pid] = {"effect": packs[pid]["effect"], **s,
                         "precision": precision, "recall": recall, "f1": f1}
        print(f"{pid:<16}{packs[pid]['effect']:<8}{tp:>4}{fp:>4}{fn:>4}{s['tn']:>5}"
              f"{fmt(precision):>11}{fmt(recall):>9}{fmt(f1):>8}")
    print("-" * 88)
    for m in MODES:
        s = mode_stats[m]
        print(f"{m:<32}{s['correct']}/{s['total']}  accuracy="
              f"{s['correct']/s['total']:.3f}" if s["total"] else f"{m}: n/a")

    if real:
        print(f"\nTHRESHOLD / PROMPT TUNING FLAGS ({len(tuning)} mismatches)")
        for t in tuning[:30]:
            print("  -", json.dumps(t, sort_keys=True))
    print("\nSAMPLE BORDERLINE DECISIONS")
    for b in borderline[:10]:
        print("  -", json.dumps(b, sort_keys=True))

    report_path = ROOT / "policy_pack_glide_benchmark_report.json" if real else REPORT
    report = {"backend": backend.strip("()"), "cases": len(cases),
              "labeled_cases": len(labeled_ids), "packs": len(ids),
              "shadow_mode": config["shadow_mode"], "per_pack": per_pack,
              "composition": {m: {**mode_stats[m],
                                  "accuracy": mode_stats[m]["correct"] / mode_stats[m]["total"]
                                  if mode_stats[m]["total"] else None} for m in MODES},
              "captured_hits": captured_hits, "captured_parity": captured_parity,
              "tuning_flags": tuning, "borderline": borderline[:20],
              "score_rows": score_rows, "details": details}
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"\nReport written to {report_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
