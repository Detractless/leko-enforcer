"""Serial worker for speech turns, with newer turns superseding stale work."""

import queue
import sys
import threading
from typing import Callable


class SerializedTurnWorker:
    """Runs one turn at a time and cancels older turns when newer work arrives."""

    def __init__(self):
        self.q = queue.Queue()
        self.seq = 0
        self.current_seq = 0
        self.alive = True
        self.lock = threading.Lock()
        self.thread = threading.Thread(target=self._run, daemon=True, name="serialized-turn-worker")
        self.thread.start()

    def submit(self, fn: Callable[[int], None], on_done=None) -> int:
        with self.lock:
            if not self.alive:
                raise RuntimeError("Turn worker is stopped")
            self.seq += 1
            seq = self.seq
            self.q.put((seq, fn, on_done))
            return seq

    def is_superseded(self, seq: int) -> bool:
        with self.lock:
            return not self.alive or seq < self.seq

    def cancel_current(self):
        with self.lock:
            self.seq += 1

    def stop(self):
        with self.lock:
            if not self.alive:
                return
            self.alive = False
            self.seq += 1
            self.q.put(None)

    def _run(self):
        while True:
            item = self.q.get()
            if item is None:
                self.q.task_done()
                break
            seq, fn, on_done = item
            with self.lock:
                self.current_seq = seq
            try:
                if not self.is_superseded(seq):
                    fn(seq)
            except Exception as exc:
                print(f"[Turn Worker Error]: {exc}", file=sys.stderr)
            finally:
                try:
                    if on_done:
                        on_done()
                except Exception as exc:
                    print(f"[Turn Cleanup Error]: {exc}", file=sys.stderr)
                finally:
                    self.q.task_done()
