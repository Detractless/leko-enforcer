"""Tests for lazy model loading (AC-001).

Evidence: "model not loaded at import"
"""
import unittest
from unittest.mock import Mock

from voice.stt_lifecycle import SttLifecycle


class TestLazyLoad(unittest.TestCase):
    def test_model_not_loaded_at_import(self):
        loader = Mock()
        mgr = SttLifecycle("/tmp/fake_model", loader, idle_timeout_s=300.0)
        self.assertFalse(mgr.is_loaded())
        loader.assert_not_called()

    def test_loads_on_first_get_model(self):
        loader = Mock(return_value="loaded_model")
        mgr = SttLifecycle("/tmp/fake_model", loader)
        self.assertFalse(mgr.is_loaded())
        result = mgr.get_model()
        self.assertEqual(result, "loaded_model")
        self.assertTrue(mgr.is_loaded())
        loader.assert_called_once()

    def test_second_get_does_not_reload(self):
        loader = Mock(return_value="loaded_model")
        mgr = SttLifecycle("/tmp/fake_model", loader)
        mgr.get_model()
        mgr.get_model()
        loader.assert_called_once()

    def test_get_model_updates_last_used(self):
        clock = Mock(side_effect=[100.0, 200.0])
        loader = Mock(return_value="m")
        mgr = SttLifecycle("/tmp/fake_model", loader, clock=clock)
        mgr.get_model()  # last_used=100.0
        clock.side_effect = [200.0]
        self.assertAlmostEqual(mgr.idle_seconds(), 100.0)