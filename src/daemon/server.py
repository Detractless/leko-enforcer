"""Stage 0 / WAE Local Pipeline Daemon HTTP Server on 127.0.0.1:8765."""

from http.server import HTTPServer, ThreadingHTTPServer, BaseHTTPRequestHandler
import json
from pathlib import Path
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

try:
    from .pipeline_service import DaemonService
except ImportError:
    from src.daemon.pipeline_service import DaemonService

SERVICE = DaemonService()


def warmup_daemon(service: DaemonService):
    """Pre-warm LFM3, GLiDE MPS shaders, and VisionEvaluator to avoid cold-start delays."""
    print("[WARMUP] Starting pipeline model warmup...", flush=True)
    t_start = time.time()
    try:
        service.start_llama_server()
        test_summary = service.summarize_lfm3("Warmup test content", "Warmup", "example.com", "/")
        print(f"[WARMUP] LFM3 ready ({round((time.time() - t_start) * 1000, 1)}ms): {test_summary[:45]}...", flush=True)
    except Exception as e:
        print(f"[WARMUP] LFM3 warmup error: {e}", flush=True)

    t_glide = time.time()
    try:
        d_res = service.decision_engine.evaluate("[Warmup] This webpage is a test search engine.")
        print(f"[WARMUP] GLiDE MPS shaders compiled ({round((time.time() - t_glide) * 1000, 1)}ms): verdict={d_res.verdict}", flush=True)
    except Exception as e:
        print(f"[WARMUP] GLiDE warmup error: {e}", flush=True)

    t_vision = time.time()
    try:
        dummy_png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII="
        v_res, _ = service.vision_evaluator.evaluate_sparse_media(dummy_png, 5)
        print(f"[WARMUP] VisionEvaluator ready ({round((time.time() - t_vision) * 1000, 1)}ms): score={v_res}", flush=True)
    except Exception as e:
        print(f"[WARMUP] VisionEvaluator warmup error: {e}", flush=True)

    print(f"[WARMUP] Daemon ready in {round((time.time() - t_start) * 1000, 1)}ms", flush=True)


class DaemonHTTPHandler(BaseHTTPRequestHandler):
    def _send_json(self, status: int, data: dict):
        body = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("content-length", str(len(body)))
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-headers", "content-type, x-wae-bridge")
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET, POST, OPTIONS")
        self.send_header("access-control-allow-headers", "content-type, x-wae-bridge")
        self.end_headers()

    def do_GET(self):
        if self.path in ("/health", "/api/status"):
            self._send_json(200, {"status": "ok", "service": "wae-daemon", "port": 8765})
        elif self.path == "/api/history":
            history = []
            hist_file = ROOT / "evaluations.jsonl"
            if hist_file.exists():
                for line in hist_file.read_text(encoding="utf-8").splitlines():
                    if line.strip():
                        try:
                            history.append(json.loads(line))
                        except Exception:
                            pass
            self._send_json(200, {"count": len(history), "history": history[-50:]})
        elif self.path == "/bridge/next":
            self.send_response(204)
            self.end_headers()
        else:
            self._send_json(404, {"error": "not_found", "path": self.path})

    def do_POST(self):
        clen = int(self.headers.get("content-length", 0))
        raw_body = self.rfile.read(clen).decode("utf-8") if clen > 0 else "{}"
        try:
            req = json.loads(raw_body)
        except Exception:
            return self._send_json(400, {"error": "invalid_json"})

        if self.path == "/api/evaluate":
            url = req.get("url", "https://example.com")
            html = req.get("html", "")
            title = req.get("title", "")
            text = req.get("text")
            dom_method = req.get("dom_method")
            image_b64 = req.get("image_b64")
            force_evaluate = bool(req.get("force_evaluate", False))
            is_test = bool(req.get("is_test", False))
            chunks = req.get("chunks")
            total_extracted = int(req.get("total_extracted", 0))
            print(f"[EVAL REQUEST] url: {url} | title: {title[:40]} | method={dom_method} | chunks={len(chunks) if chunks else 0}", flush=True)
            res = SERVICE.evaluate_page(url, html, title, image_b64, force_evaluate=force_evaluate, is_test=is_test, text=text, dom_method=dom_method, chunks=chunks, total_extracted=total_extracted)
            print(f"[EVAL RESULT] verdict: {res.get('verdict')} | policy: {res.get('policy_triggered')} ({res.get('latency_ms')}ms)", flush=True)
            self._send_json(200, res)
        elif self.path == "/api/evaluate-chunks":
            chunks = req.get("chunks", [])
            total_extracted = int(req.get("total_extracted", len(chunks)))
            c_res = SERVICE.chunk_evaluator.evaluate_chunks(chunks, total_extracted=total_extracted)
            self._send_json(200, {
                "flagged_ids": c_res.flagged_ids, "flagged_count": c_res.flagged_count,
                "flag_ratio": c_res.flag_ratio, "escalate_to_block": c_res.escalate_to_block,
                "details": c_res.details
            })
        elif self.path == "/bridge/hello":
            self._send_json(200, {"ok": True, "bridge": 3, "service": "wae-stage0"})
        elif self.path == "/bridge/result":
            self._send_json(200, {"ok": True})
        else:
            self._send_json(404, {"error": "not_found", "path": self.path})

    def log_message(self, format, *args):
        pass


def create_server(host: str = "127.0.0.1", port: int = 8765) -> HTTPServer:
    return ThreadingHTTPServer((host, port), DaemonHTTPHandler)


if __name__ == "__main__":
    warmup_daemon(SERVICE)
    server = create_server()
    print("Starting WAE daemon on http://127.0.0.1:8765...", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping daemon...", flush=True)
    finally:
        SERVICE.stop_llama_server()
