"""Bounded session context and one backend-owned, expiring action."""
from collections import deque
from copy import deepcopy
import re
import threading
import time
import uuid

from memory.local_store import LocalMemory
try:
    from ..request_state.state import RequestState
except ImportError:
    from request_state.state import RequestState


def confirmation(text):
    clean = re.sub(r"[^\w\s]", "", text.casefold())
    clean = " ".join(clean.split())
    if clean in {"yes", "yeah", "yep", "yup", "yes please", "confirm", "go ahead", "do it"}:
        return True
    if clean in {"no", "nope", "no thanks", "cancel", "cancel it", "never mind", "nevermind"}:
        return False
    return None


class Conversation:
    def __init__(self, memory=None, clock=time.monotonic):
        self.memory = memory if memory is not None else LocalMemory()
        self.session_log = None
        self.current_turn_id = None
        self.current_input_source = "unknown"
        self.clock = clock
        self.history = deque(maxlen=6)
        self.pending = None
        self.active_request = RequestState()
        self.epoch = 0
        self.lock = threading.RLock()
        self.turn_lock = threading.RLock()

    def cancel(self):
        with self.lock:
            self.epoch += 1
            self.pending = None
            self.active_request.clear()

    def record(self, query, reply):
        self.history.extend([{"role": "user", "content": query[:400]},
                             {"role": "assistant", "content": reply[:400]}])

    def stage(self, intent, settings, origin_turn_id=None):
        self.pending = {"id": uuid.uuid4().hex, "intent": deepcopy(intent),
                        "settings": settings, "expires": self.clock() + 120,
                        "origin_turn_id": origin_turn_id}
        return self.pending["id"]

    def take(self, expected_id=None):
        if not self.pending:
            return None
        if self.clock() >= self.pending["expires"]:
            self.pending = None
            self.active_request.clear()
            return None
        if expected_id is not None and expected_id != self.pending["id"]:
            return None
        pending, self.pending = self.pending, None
        return pending
