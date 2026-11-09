"""Verify that local model startup output is retained for diagnosis."""
import io
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "voice"))
import llm_router


class LlmServerDiagnosticsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.runtime = root / "llama-server"
        self.model = root / "model.gguf"
        self.log = root / "llama-server.log"
        self.runtime.touch()
        self.model.touch()

    def server_patches(self, proc, running):
        stack = patch.multiple(
            llm_router,
            RUNTIME_BIN=self.runtime,
            MODEL_PATH=self.model,
            SERVER_LOG_PATH=self.log,
        )
        stack.start()
        self.addCleanup(stack.stop)
        running_patch = patch.object(llm_router, "is_server_running", side_effect=running)
        running_patch.start()
        self.addCleanup(running_patch.stop)
        popen_patch = patch.object(llm_router.subprocess, "Popen", return_value=proc)
        popen_mock = popen_patch.start()
        self.addCleanup(popen_patch.stop)
        return popen_mock

    def test_startup_captures_output_and_reports_log_path(self):
        proc = Mock(pid=42)
        proc.poll.return_value = None
        popen = self.server_patches(proc, [False, True])
        diagnostic = io.StringIO()

        with redirect_stderr(diagnostic):
            self.assertIs(llm_router.ensure_server(timeout_sec=1), proc)

        _, options = popen.call_args
        self.assertEqual(options["stderr"], subprocess.STDOUT)
        self.assertEqual(Path(options["stdout"].name), self.log)
        self.assertIn("runtime=", self.log.read_text())
        self.assertIn("pid=42", diagnostic.getvalue())
        self.assertIn(str(self.log), diagnostic.getvalue())

    def test_early_exit_reports_code_and_log_path(self):
        proc = Mock(pid=7)
        proc.poll.return_value = 9
        proc.returncode = 9
        self.server_patches(proc, [False])
        diagnostic = io.StringIO()

        with redirect_stderr(diagnostic), self.assertRaises(RuntimeError):
            llm_router.ensure_server(timeout_sec=1)

        self.assertIn("code=9", diagnostic.getvalue())
        self.assertIn(str(self.log), diagnostic.getvalue())
        self.assertTrue(self.log.exists())


if __name__ == "__main__":
    unittest.main()
