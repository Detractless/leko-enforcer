"""Vision and OCR evaluator for the WAE daemon using local ONNX weights."""

import base64
import io
from pathlib import Path
import subprocess
from typing import Optional, Tuple

import numpy as np
import onnxruntime as ort
from PIL import Image

MODEL_PATH = Path("/Users/calibro/atlas-nsfw-eval/models/marqo-384-int8.onnx")
TESSERACT_BIN = Path("/opt/homebrew/bin/tesseract")


class VisionEvaluator:
    def __init__(self, model_path: Optional[Path] = None):
        self.model_path = model_path or MODEL_PATH
        self._session: Optional[ort.InferenceSession] = None
        self._input_name: Optional[str] = None

    def _get_session(self) -> ort.InferenceSession:
        if self._session is None:
            opts = ort.SessionOptions()
            opts.intra_op_num_threads = 2
            opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            self._session = ort.InferenceSession(str(self.model_path), sess_options=opts)
            self._input_name = self._session.get_inputs()[0].name
        return self._session

    def decode_image(self, image_data: str | bytes) -> Image.Image:
        """Decode base64 or raw image bytes to RGB PIL Image."""
        if isinstance(image_data, str):
            if "," in image_data:
                image_data = image_data.split(",", 1)[1]
            raw_bytes = base64.b64decode(image_data)
        else:
            raw_bytes = image_data
        img = Image.open(io.BytesIO(raw_bytes))
        return img.convert("RGB")

    def score_image(self, img: Image.Image) -> float:
        """Score image with Marqo ONNX model, returning NSFW probability [0.0, 1.0]."""
        session = self._get_session()
        resized = img.resize((384, 384), Image.Resampling.BILINEAR)
        arr = np.array(resized).astype(np.float32) / 255.0
        arr = (arr - 0.5) / 0.5  # Mean 0.5, Std 0.5
        arr = np.transpose(arr, (2, 0, 1))  # [3, 384, 384]
        arr = np.expand_dims(arr, 0)  # [1, 3, 384, 384]

        logits = session.run(None, {self._input_name: arr})[0][0]
        # Softmax over [NSFW, SFW]
        exps = np.exp(logits - np.max(logits))
        probs = exps / np.sum(exps)
        return float(probs[0])  # Index 0 is NSFW

    def run_ocr(self, img: Image.Image) -> str:
        """Extract text from image using local tesseract binary if available."""
        if not TESSERACT_BIN.exists():
            return ""
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        try:
            res = subprocess.run(
                [str(TESSERACT_BIN), "stdin", "stdout", "--oem", "1", "-l", "eng"],
                input=buf.getvalue(),
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                check=True,
                timeout=5,
            )
            return res.stdout.decode("utf-8", errors="ignore").strip()
        except Exception:
            return ""

    def evaluate_sparse_media(
        self, image_b64: Optional[str], text_word_count: int
    ) -> Tuple[float, str]:
        """Conditional evaluation: only score image and run OCR if image exists or text is sparse."""
        if not image_b64:
            return 0.0, ""

        try:
            img = self.decode_image(image_b64)
            nsfw_score = self.score_image(img)
            ocr_text = ""
            if text_word_count < 30:
                ocr_text = self.run_ocr(img)
            return round(nsfw_score, 3), ocr_text
        except Exception:
            return 0.0, ""
