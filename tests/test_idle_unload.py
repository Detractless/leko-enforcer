"""Tests for idle model unload (AC-002).

Evidence: "unloads after timeout"
"""
import unittest
from unittest.mock import Mock

from voice.stt_lifecycle import SttLifecycle


class TestIdleUnload(unittest.TestCase):
    def test_unloads_after_timeout(self):
        clock = Mock(side_effect=[10.0, 320.0])
        loader = Mock(return_value="m")
        mgr = SttLifecycle("/tmp/fake_model", loader, idle_timeout_s=300.0, clock=clock)
        mgr.get_model()  # last_used=10.0
        unloaded = mgr.unload_if_idle()  # clock=320.0, idle=310 >= 300
        self.assertTrue(unloaded)
        self.assertFalse(mgr.is_loaded())

    def test_does_not_unload_before_timeout(self):
        clock_values = [0.0, 100.0]
        clock = Mock(side_effect=clock_values)
        loader = Mock(return_value="m")
        mgr = SttLifecycle("/tmp/fake_model", loader, idle_timeout_s=300.0, clock=clock)
        mgr.get_model()  # last_used=0.0
        clock.side_effect = [100.0]
        unloaded = mgr.unload_if_idle()
        self.assertFalse(unloaded)
        self.assertTrue(mgr.is_loaded())

    def test_deferred_during_transcription(self):
        clock_values = [0.0, 400.0]
        clock = Mock(side_effect=clock_values)
        loader = Mock(return_value="m")
        mgr = SttLifecycle("/tmp/fake_model", loader, idle_timeout_s=300.0, clock=clock)
        mgr.get_model()
        mgr.set_transcribing(True)
        clock.side_effect = [400.0]
        unloaded = mgr.unload_if_idle()
        self.assertFalse(unloaded)
        self.assertTrue(mgr.is_loaded())

    def test_unload_not_loaded_returns_false(self):
        clock = Mock(return_value=0.0)
        loader = Mock()
        mgr = SttLifecycle("/tmp/fake_model", loader, clock=clock)
        self.assertFalse(mgr.unload_if_idle())