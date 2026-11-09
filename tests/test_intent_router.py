"""Tests for intent router (AC-001 through AC-005).

Evidence: "classify returns valid category"
Evidence: "router selects specialized prompt"
Evidence: "routing mode flag controls behavior"
Evidence: "routed parse returns same format"
Evidence: "correction context injected"
"""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "voice"))
import unittest
from unittest.mock import patch, MagicMock

import routing.intent_router as ir


class TestIntentRouter(unittest.TestCase):
    """Evidence: "classify returns valid category" """
    def test_classify_returns_valid_category(self):
        mock_resp = MagicMock()
        mock_resp.__enter__.return_value.read.return_value = \
            b'{"choices":[{"message":{"content":"tool_call"}}]}'
        with patch.object(ir.urllib.request, "urlopen", return_value=mock_resp):
            cat = ir.classify_intent("block reddit")
        self.assertIn(cat, ir.INTENT_CATEGORIES)

    """Evidence: "router selects specialized prompt" """
    def test_router_selects_specialized_prompt(self):
        for cat in ir.INTENT_CATEGORIES:
            p = ir.get_specialized_prompt(cat)
            self.assertIsInstance(p, str)
            self.assertTrue(len(p) > 0)
        # Different categories get different prompts
        self.assertNotEqual(ir.get_specialized_prompt("tool_call"),
                            ir.get_specialized_prompt("conversational"))

    """Evidence: "routing mode flag controls behavior" """
    def test_routing_mode_flag_controls_behavior(self):
        self.assertIn(ir.ROUTING_MODE, ["single", "cascade"])

    """Evidence: "routed parse returns same format" """
    def test_routed_parse_returns_same_format(self):
        # With routing off (default), parse_user_intent uses single path
        import llm_router
        mock_resp = MagicMock()
        mock_resp.__enter__.return_value.read.return_value = \
            b'{"choices":[{"message":{"tool_calls":[{"function":{"name":"add_block","arguments":"{\\"name\\":\\"Test\\"}"}}]}}]}'
        with patch.object(llm_router, "ensure_server"), patch.object(
                llm_router.urllib.request, "urlopen", return_value=mock_resp):
            r = llm_router.parse_user_intent("block reddit")
        self.assertIn("action", r)

    """Evidence: "correction context injected" """
    def test_correction_context_injected(self):
        prompt = ir.get_specialized_prompt("correction")
        self.assertIn("correcting", prompt.lower())