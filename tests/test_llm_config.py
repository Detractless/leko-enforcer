"""Tests for LLM config optimization (AC-001 through AC-006).

Evidence: "reasoning enabled and ctx expanded"
Evidence: "sampling params present"
Evidence: "correction awareness in prompt"
Evidence: "uncertainty rule in prompt"
Evidence: "history and memory caps"
Evidence: "preserve thinking enabled"
"""
import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "voice"))
import unittest
from unittest.mock import patch, MagicMock

import llm_router


class TestLlmConfig(unittest.TestCase):
    """Evidence: "reasoning enabled and ctx expanded" """

    def test_reasoning_enabled_and_ctx_expanded(self):
        # Check ensure_server command has no --reasoning off and ctx >= 8192
        import inspect
        src = inspect.getsource(llm_router.ensure_server)
        self.assertNotIn('"--reasoning", "off"', src)
        self.assertIn('"--ctx-size", "8192"', src)
        self.assertIn('"--reasoning-budget", "1024"', src)

    """Evidence: "sampling params present" """
    def test_sampling_params_present(self):
        import inspect
        src = inspect.getsource(llm_router.parse_user_intent)
        self.assertIn('"top_k": 50', src)
        self.assertIn('"repetition_penalty": 1.1', src)
        self.assertIn('"max_tokens": 1624', src)

    """Evidence: "correction awareness in prompt" """
    def test_correction_awareness_in_prompt(self):
        prompt = llm_router.get_system_prompt()
        self.assertIn("corrects or clarifies", prompt)

    def test_blocked_and_allowed_windows_have_distinct_meanings(self):
        prompt = llm_router.get_system_prompt()
        self.assertIn("Never put a blocked interval into `open_hours`", prompt)
        self.assertIn("never turn that window into a duration", prompt)

    """Evidence: "uncertainty rule in prompt" """
    def test_uncertainty_rule_in_prompt(self):
        prompt = llm_router.get_system_prompt()
        self.assertIn("clarifying question", prompt)

    def test_unclear_block_target_names_apps_sites_and_whole_computer(self):
        prompt = llm_router.get_system_prompt()
        self.assertIn("apps, websites, or the whole computer", prompt)
        self.assertIn("Do not leave out whole-computer blocking", prompt)

    """Evidence: "history and memory caps" """
    def test_history_and_memory_caps(self):
        import inspect
        src = inspect.getsource(llm_router.parse_user_intent)
        self.assertIn("memories[:2]", src)
        self.assertIn("[-4:]", src)

    """Evidence: "preserve thinking enabled" """
    def test_preserve_thinking_enabled(self):
        import inspect
        src = inspect.getsource(llm_router.parse_user_intent)
        self.assertIn("preserve_thinking", src)
