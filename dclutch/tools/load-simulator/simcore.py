#!/usr/bin/env python3
"""Core primitives for the dClutch load simulator.

This module owns everything about the simulator that does not touch a
cluster: the cycle journal (resume-never-resend), rate control with jitter
and refusal-aware backoff, the atomically written status artifact, and the
halt discipline.  It deliberately imports nothing from the driver layer so
each piece is testable without a validator.

Journal doctrine (house standard):
  * every cycle gets its own directory, ``cycle-NNNNNN``;
  * a cycle journal moves through phases ``planned -> executing -> finalized``
    (or ``halted``), each written to a temp file and renamed into place;
  * rerunning over a ``finalized`` cycle journal is a byte-identical no-op:
    the orchestrator recomputes the plan digest and refuses loudly if the
    journal on disk describes a different plan.
"""

from __future__ import annotations

import dataclasses
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import random
import signal
import time
from typing import Any, Callable, Optional

SCHEMA_STATUS = "dclutch-load-simulator-status-v1"
SCHEMA_CYCLE = "dclutch-load-simulator-cycle-v1"
SCHEMA_HALT = "dclutch-load-simulator-halt-v1"

PHASE_PLANNED = "planned"
PHASE_EXECUTING = "executing"
PHASE_FINALIZED = "finalized"
PHASE_HALTED = "halted"


def utc_now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def canonical_json_bytes(value: Any) -> bytes:
    """One canonical byte encoding so digests are stable across reruns."""
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def digest_of(value: Any) -> str:
    return hashlib.sha256(canonical_json_bytes(value)).hexdigest()


def write_atomic(path: Path, payload: bytes) -> None:
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_bytes(payload)
    os.replace(tmp, path)


def write_json_atomic(path: Path, value: Any) -> None:
    write_atomic(path, json.dumps(value, sort_keys=True, indent=2).encode("utf-8") + b"\n")


class JournalConflict(RuntimeError):
    """The journal already on disk describes a different plan."""


class Halt(RuntimeError):
    """A conservation divergence or structural refusal: stop loudly."""


@dataclasses.dataclass
class CycleJournal:
    """Write-ahead journal for one simulator cycle."""

    directory: Path
    index: int

    @property
    def path(self) -> Path:
        return self.directory / "cycle.json"

    @staticmethod
    def cycle_dir(journal_root: Path, index: int) -> Path:
        return journal_root / f"cycle-{index:06d}"

    @classmethod
    def open(cls, journal_root: Path, index: int) -> "CycleJournal":
        d = cls.cycle_dir(journal_root, index)
        d.mkdir(parents=True, exist_ok=True)
        return cls(directory=d, index=index)

    def read(self) -> Optional[dict]:
        if not self.path.exists():
            return None
        return json.loads(self.path.read_text())

    def is_finalized(self) -> bool:
        body = self.read()
        return bool(body) and body.get("phase") == PHASE_FINALIZED

    def assert_same_plan_or_absent(self, plan: dict) -> Optional[dict]:
        """Resume guard: an existing journal must describe this exact plan.

        Returns the existing journal body (any phase) or None.  Raises
        JournalConflict when the journal on disk was written for a different
        plan -- resuming over it would be resuming somebody else's run.
        """
        body = self.read()
        if body is None:
            return None
        if body.get("plan_digest") != digest_of(plan):
            raise JournalConflict(
                f"the journal already in {self.directory} describes a different "
                f"plan (theirs {body.get('plan_digest')!r}, ours {digest_of(plan)!r}); "
                "refusing to resume over it"
            )
        return body

    def record(self, phase: str, plan: dict, **extra: Any) -> dict:
        body = {
            "schema": SCHEMA_CYCLE,
            "cycle": self.index,
            "phase": phase,
            "plan": plan,
            "plan_digest": digest_of(plan),
            "recorded_at": utc_now_iso(),
        }
        body.update(extra)
        write_json_atomic(self.path, body)
        return body


@dataclasses.dataclass
class RateController:
    """Cadence with jitter, plus refusal-aware exponential backoff.

    ``period_seconds`` is the target time between mutation cycles.  Jitter is
    uniform in ``[-jitter_fraction, +jitter_fraction]`` of the period so a
    sustained run does not phase-lock with anything else hitting the same RPC.
    ``on_backpressure`` doubles the wait (capped) each consecutive time the
    driver surfaces rate limiting (HTTP 429 et al.); one clean cycle resets it.
    """

    period_seconds: float
    jitter_fraction: float = 0.25
    backoff_initial: float = 5.0
    backoff_max: float = 120.0
    _backoff: float = 0.0
    rng: random.Random = dataclasses.field(default_factory=random.Random)

    def next_delay(self) -> float:
        jitter = 1.0 + self.rng.uniform(-self.jitter_fraction, self.jitter_fraction)
        return max(0.0, self.period_seconds * jitter + self._backoff)

    def on_backpressure(self) -> float:
        self._backoff = min(
            self.backoff_max, self.backoff_initial if self._backoff == 0 else self._backoff * 2
        )
        return self._backoff

    def on_clean_cycle(self) -> None:
        self._backoff = 0.0


BACKPRESSURE_MARKERS = (
    "429",
    "Too Many Requests",
    "rate limit",
    "rate-limited",
    "blockhash not found",
)


def looks_like_backpressure(text: str) -> bool:
    lowered = text.lower()
    return any(marker.lower() in lowered for marker in BACKPRESSURE_MARKERS)


@dataclasses.dataclass
class StopFlag:
    """SIGTERM/SIGINT set the flag; the loop finishes the in-flight cycle."""

    requested: bool = False
    signal_name: Optional[str] = None

    def install(self) -> None:
        for sig in (signal.SIGTERM, signal.SIGINT):
            signal.signal(sig, self._handle)

    def _handle(self, signum: int, _frame: Any) -> None:
        self.requested = True
        self.signal_name = signal.Signals(signum).name

    def sleep_interruptibly(self, seconds: float, step: float = 0.5) -> None:
        deadline = time.monotonic() + seconds
        while not self.requested and time.monotonic() < deadline:
            time.sleep(min(step, max(0.0, deadline - time.monotonic())))


@dataclasses.dataclass
class StatusWriter:
    """The status artifact: one JSON file, rewritten atomically every cycle.

    Shaped so a web surface can render it without knowing the simulator:
    everything is plain data, signatures are capped, secrets never enter it.
    """

    path: Path
    cluster_label: str
    rpc_url: str
    mode: str
    market_address: Optional[str] = None
    started_at: str = dataclasses.field(default_factory=utc_now_iso)
    max_signatures: int = 50

    def write(
        self,
        *,
        cycles_run: int,
        cycles_target: Optional[int],
        trades_landed: int,
        signatures: list,
        wallets: list,
        last_reconciliation: Optional[dict],
        halted: bool = False,
        halt_reason: Optional[str] = None,
        stopping: bool = False,
        extra: Optional[dict] = None,
    ) -> dict:
        body = {
            "schema": SCHEMA_STATUS,
            "cluster": {"label": self.cluster_label, "rpc_url": self.rpc_url},
            "market": {"address": self.market_address},
            "mode": self.mode,
            "started_at": self.started_at,
            "updated_at": utc_now_iso(),
            "cycles": {"run": cycles_run, "target": cycles_target},
            "trades": {"landed": trades_landed, "signatures": signatures[-self.max_signatures:]},
            "wallets": wallets,
            "last_reconciliation": last_reconciliation,
            "halted": halted,
            "halt_reason": halt_reason,
            "stopping": stopping,
        }
        if extra:
            body.update(extra)
        write_json_atomic(self.path, body)
        return body


def halt_loudly(work_dir: Path, reason: str, details: Optional[dict] = None) -> None:
    """Record the halt durably, then raise.  A halted work dir refuses restart
    until the HALT file is deliberately removed by a human."""
    body = {
        "schema": SCHEMA_HALT,
        "reason": reason,
        "details": details or {},
        "halted_at": utc_now_iso(),
    }
    write_json_atomic(work_dir / "HALT.json", body)
    raise Halt(reason)


def refuse_if_halted(work_dir: Path) -> None:
    halt_file = work_dir / "HALT.json"
    if halt_file.exists():
        body = json.loads(halt_file.read_text())
        raise Halt(
            f"work dir {work_dir} is halted ({body.get('reason')!r} at "
            f"{body.get('halted_at')}); remove HALT.json deliberately to resume"
        )
