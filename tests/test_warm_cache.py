"""Tests for page-cache warming (AC-003).

Evidence: "warm cache reduces cold start"
"""
import os
import tempfile
import unittest
from unittest.mock import Mock

from voice.stt_lifecycle import SttLifecycle


class TestWarmCache(unittest.TestCase):
    def _warm_manager(self):
        """A lifecycle over a 1 KB temp file, with a fixed clock and a recorded loader."""
        handle = tempfile.NamedTemporaryFile(delete=False)
        handle.write(b"x" * 1024)
        handle.close()
        clock = Mock(return_value=0.0)
        loader = Mock()
        mgr = SttLifecycle(handle.name, loader, clock=clock)
        return handle.name, loader, mgr

    def test_warm_cache_reads_file(self):
        path, _, mgr = self._warm_manager()
        try:
            result = mgr.warm_cache()
            self.assertTrue(result)
        finally:
            os.unlink(path)

    def test_warm_cache_missing_path_returns_false(self):
        clock = Mock(return_value=0.0)
        loader = Mock()
        mgr = SttLifecycle("/tmp/nonexistent_model_xyz", loader, clock=clock)
        result = mgr.warm_cache()
        self.assertFalse(result)

    def test_warm_cache_directory(self):
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, "a.bin"), "wb") as f:
                f.write(b"hello")
            with open(os.path.join(d, "b.bin"), "wb") as f:
                f.write(b"world")
            clock = Mock(return_value=0.0)
            loader = Mock()
            mgr = SttLifecycle(d, loader, clock=clock)
            result = mgr.warm_cache()
            self.assertTrue(result)

    def test_warm_cache_does_not_load_model(self):
        path, loader, mgr = self._warm_manager()
        try:
            mgr.warm_cache()
            self.assertFalse(mgr.is_loaded())
            loader.assert_not_called()
        finally:
            os.unlink(path)