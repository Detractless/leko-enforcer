"""Local, append-only session diagnostics stored as dated JSONL files."""
from __future__ import annotations

from datetime import datetime
import json
import os
from pathlib import Path
import sys
import threading
import uuid


SCHEMA_VERSION = 1


class SessionLog:
    """Store local voice events without feeding archived data back to the model."""

    def __init__(self, root: Path | str | None = None, session_id: str | None = None,
                 started_at: datetime | None = None):
        base = Path(root) if root is not None else (
            Path.home() / "Library" / "Application Support" / "Leko" / "session-logs"
        )
        self.session_id = session_id or uuid.uuid4().hex
        self.started_at = started_at or datetime.now().astimezone()
        self.directory = base / self.started_at.strftime("%Y") / self.started_at.strftime("%m") / self.started_at.strftime("%d")
        self.path = self.directory / f"session-{self.session_id}.jsonl"
        self._lock = threading.Lock()
        self._sequence = 0
        self._enabled = True
        self._report_latest_incomplete(base)
        try:
            self.directory.mkdir(parents=True, exist_ok=True)
            os.chmod(self.directory, 0o700)
            fd = os.open(self.path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            os.close(fd)
            os.chmod(self.path, 0o600)
        except OSError as exc:
            self._enabled = False
            self._warn(f"cannot create session file {self.path}: {exc}")

    @staticmethod
    def new_turn_id() -> str:
        return uuid.uuid4().hex

    def playback_sink(self, turn_id: str | None):
        return lambda event: self.append("playback", turn_id, **event)

    @staticmethod
    def read_records(path: Path | str) -> tuple[list[dict], str | None]:
        """Read valid JSONL records, reporting but never changing damaged data."""
        records = []
        try:
            lines = Path(path).read_bytes().splitlines(keepends=True)
        except OSError as exc:
            return records, f"cannot read session file: {exc}"
        warning = None
        for index, line in enumerate(lines, start=1):
            if not line.endswith(b"\n"):
                warning = f"incomplete final record at line {index}"
                break
            try:
                record = json.loads(line)
            except (UnicodeDecodeError, json.JSONDecodeError):
                warning = f"invalid JSON record at line {index}"
                break
            if record.get("schema_version") != SCHEMA_VERSION:
                warning = f"unsupported schema version at line {index}"
                break
            records.append(record)
        return records, warning

    def _report_latest_incomplete(self, base: Path) -> None:
        try:
            latest = max(base.rglob("session-*.jsonl"), key=lambda item: item.stat().st_mtime)
        except (OSError, ValueError):
            return
        _, warning = self.read_records(latest)
        if warning:
            self._warn(f"{latest}: {warning}; earlier complete records remain readable")

    @staticmethod
    def _warn(message: str) -> None:
        print(f"[Session log warning] {message}", file=sys.stderr, flush=True)

    def append(self, event_type: str, turn_id: str | None = None, **details) -> bool:
        """Append one durable record; roll back a short/failed write to its prior length."""
        if not self._enabled:
            return False
        with self._lock:
            self._sequence += 1
            record = {
                "schema_version": SCHEMA_VERSION,
                "session_id": self.session_id,
                "sequence": self._sequence,
                "timestamp": datetime.now().astimezone().isoformat(timespec="milliseconds"),
                "event": event_type,
                "details": details,
            }
            if turn_id:
                record["turn_id"] = turn_id
            encoded = (json.dumps(record, ensure_ascii=False, default=str, separators=(",", ":")) + "\n").encode()
            fd = None
            old_size = 0
            try:
                fd = os.open(self.path, os.O_WRONLY | os.O_APPEND, 0o600)
                os.fchmod(fd, 0o600)
                old_size = os.fstat(fd).st_size
                remaining = memoryview(encoded)
                while remaining:
                    written = os.write(fd, remaining)
                    if written <= 0:
                        raise OSError("session write made no progress")
                    remaining = remaining[written:]
                os.fsync(fd)
                return True
            except OSError as exc:
                if fd is not None:
                    try:
                        os.ftruncate(fd, old_size)
                        os.fsync(fd)
                    except OSError:
                        pass
                self._warn(f"cannot append to {self.path}: {exc}")
                return False
            finally:
                if fd is not None:
                    os.close(fd)
