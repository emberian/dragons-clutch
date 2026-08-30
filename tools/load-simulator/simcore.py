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
import re
import shlex
import shutil
import signal
import sys
import time
from typing import Any, Callable, Optional
import urllib.parse

SCHEMA_STATUS = "dclutch-load-simulator-status-v1"
SCHEMA_CYCLE = "dclutch-load-simulator-cycle-v1"
SCHEMA_HALT = "dclutch-load-simulator-halt-v1"
SCHEMA_EXIT = "dclutch-load-simulator-exit-v1"

PHASE_PLANNED = "planned"
PHASE_EXECUTING = "executing"
PHASE_FINALIZED = "finalized"
PHASE_HALTED = "halted"

# How a run ended, as recorded in EXIT.json.  ABSENCE of that file after the
# heartbeat deadline has passed is itself a reading -- see `record_exit`.
EXIT_COMPLETED = "completed"
EXIT_PREFLIGHT = "preflight"
EXIT_SIGNALLED = "signalled"
EXIT_HALTED = "halted"
EXIT_LOW_DISK = "low-disk"
EXIT_CRASHED = "crashed"

# Census retention defaults.  See CensusRetention for why these two numbers
# are the whole storage bound.
DEFAULT_CENSUS_WINDOW = 480
DEFAULT_CENSUS_KEEP_FILES = 2

# A run refuses to start, and stops between cycles, when the volume holding
# its work directory has less than this much room.  On 2026-08-30 an unbounded
# census filled the shared data volume to 100% and every lane on the machine
# lost its shell to ENOSPC -- including the lane that would have diagnosed it,
# because the harness writes each command's output to that same volume before
# anyone can read it.  A writer that keeps writing into the last gigabyte is
# not resilient, it is the cause; so this one stops while there is still room
# to record that it stopped.
DEFAULT_DISK_FLOOR_BYTES = 2 * 1024 * 1024 * 1024


def utc_now_iso() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def canonical_json_bytes(value: Any) -> bytes:
    """One canonical byte encoding so digests are stable across reruns."""
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def redact_endpoint(url: str) -> str:
    """An RPC endpoint with any credential removed.

    Provider keys ride in the query string (Helius sends ``?api-key=...``) and
    for some providers in the path itself. Everything this module writes is
    read by a web surface or handed to whoever is looking at a run, so what it
    records is the endpoint's identity - scheme and host - and never the
    credential that reaches it. Redaction happens where the value is STORED,
    not where it is passed in, so no caller can forget.
    """
    if not url:
        return url
    parts = urllib.parse.urlsplit(url)
    if not parts.scheme or not parts.netloc:
        return "<redacted>"
    path = parts.path if parts.path in ("", "/") else "/<redacted>"
    query = "?<redacted>" if parts.query else ""
    return f"{parts.scheme}://{parts.netloc}{path}{query}"


_URL_IN_TEXT = re.compile(r"\b[a-zA-Z][a-zA-Z0-9+.-]*://[^\s'\"]+")


def redact_text(text: str) -> str:
    """Free text with every endpoint credential in it redacted.

    For the strings this module records that nobody composed field by field:
    a child's error output, an exception message, a command line.  The live
    RPC URL is passed to every driver as `--rpc-url <url>`, so anything that
    quotes a command or echoes a driver's complaint can carry the provider key
    into a file -- which is how it survived being removed from status.json and
    the cycle plan (d17aa1a4) and stayed in HALT.json's recorded command.
    Same doctrine as `redact_endpoint`: redact where the value is STORED, so
    no caller can forget.
    """
    if not text:
        return text
    return _URL_IN_TEXT.sub(lambda match: redact_endpoint(match.group(0)), text)


def redact_command(argv) -> str:
    """One printable command line with no credential in it.

    Every element is redacted as an endpoint if it parses as one, so it does
    not matter which flag carried the URL or whether a future driver invents
    another.
    """
    parts = []
    for element in argv:
        element = str(element)
        parts.append(redact_text(element) if "://" in element else element)
    return " ".join(shlex.quote(part) for part in parts)


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
class CensusRetention:
    """Bounded storage for a census series whose newest file is the whole series.

    THE PROPERTY WORTH KEEPING.  `ledger-census --prior P` reloads P's
    observation array, appends the observation it just took, and re-serializes
    the whole chain.  So the NEWEST census file alone is the complete series,
    and no reader has to accumulate history -- which is exactly what
    `apps/dclutch-web/scripts/simulator-series.mjs` mines to draw the run.
    Nothing here breaks that.

    THE COST NOBODY BOUNDED.  Because every file is the whole series so far,
    the DIRECTORY is the sum of its files: with `b` bytes per observation, N
    cycles cost `b * N(N+1)/2`.  Measured on the market18 devnet run:
    b = 3,871 bytes, cycle-000001.json = 3,793 B, cycle-000123.json =
    476,055 B, directory = 28 MB at 123 cycles -- and 1.94 GB by cycle 1000.
    That run filled the machine's data volume and took every lane's shell down
    with it.

    TWO BOUNDS, both applied after each census is written.

      * FILE COUNT.  Every file older than the newest is a strict PREFIX of
        the newest: it holds no observation the newest does not.  Superseded
        files are therefore redundancy, not record, and `keep_files` of them
        is a rollback margin rather than history.  This alone turns O(N^2)
        into O(N).

      * SERIES LENGTH.  The newest file is truncated to its last `window`
        observations before it becomes the next cycle's `--prior`.  This turns
        O(N) into O(1), and it is LOSSLESS for every conservation law -- a
        property of the ledger, not a hope.  Each delta law reads exactly one
        predecessor, `self.observations.last()`:
        L2 at tools/gauntlet/journey/src/ledger.rs:463, L5 at :551, L6 at :577
        and L7 at :653; and the census's own verdicts and exit code are
        computed from `observations.last()` alone
        (tools/local-validator/bootstrap/successor/src/main.rs:502-522).
        No law reads the prefix.  Truncation drops whole array ELEMENTS and
        never edits one: the retained observations are re-serialized to the
        same bytes the census wrote them as, which `test_simcore.py` proves
        against the real 123-file market18 series.

    WORST CASE ON DISK is therefore `keep_files * window * b`, CONSTANT in the
    number of cycles run.  At the defaults and market18's measured b that is
    2 * 480 * 3,871 = 3,716,160 bytes, about 3.7 MB.  `b` depends on how many
    accounts a config tracks, so `apply` MEASURES it each cycle and reports
    the bound it actually implies rather than quoting this paragraph.
    """

    window: int = DEFAULT_CENSUS_WINDOW
    keep_files: int = DEFAULT_CENSUS_KEEP_FILES

    def __post_init__(self) -> None:
        if self.window < 1:
            raise ValueError("census window must keep at least one observation")
        if self.keep_files < 1:
            raise ValueError("census retention must keep at least the newest file")

    @staticmethod
    def series_files(census_dir: Path) -> list:
        """Every census file, oldest first.  The names are zero-padded to six
        digits, so lexicographic order is cycle order up to cycle 999999."""
        if not census_dir.is_dir():
            return []
        return sorted(census_dir.glob("cycle-*.json"))

    @staticmethod
    def serialize(observations: list) -> bytes:
        """The census's own encoding: `serde_json::to_vec_pretty`, which is a
        two-space indent and no trailing newline.  Key order is preserved from
        the parse, so a round trip is byte-identical and truncation cannot
        silently rewrite an observation while claiming to drop one."""
        return json.dumps(observations, indent=2).encode("utf-8")

    def apply(self, census_dir: Path) -> dict:
        """Enforce both bounds.  Returns what it did, in bytes, for the status
        artifact -- a bound nobody can read is not a bound."""
        files = self.series_files(census_dir)
        if not files:
            return {
                "window": self.window,
                "keep_files": self.keep_files,
                "files": 0,
                "removed_files": 0,
                "observations": 0,
                "dropped_observations": 0,
                "bytes_on_disk": 0,
                "bytes_per_observation": None,
                "bytes_bound": None,
            }

        newest = files[-1]
        observations = json.loads(newest.read_bytes())
        if not isinstance(observations, list):
            raise ValueError(f"{newest} is not an observation array")
        before = len(observations)
        # Measured from the newest file, which is the only one whose length we
        # know exactly; an empty series has no per-observation size to report.
        # ROUNDED UP, because this number is multiplied into a claimed ceiling:
        # the file also carries its brackets and separators, so a floor here
        # would state a bound the directory then sits a few bytes above -- and
        # a bound that is wrong by four bytes is not a bound, it is an
        # estimate wearing a bound's name.
        per_observation = (
            -(-newest.stat().st_size // before) if before else None
        )

        dropped = 0
        if before > self.window:
            observations = observations[-self.window:]
            write_atomic(newest, self.serialize(observations))
            dropped = before - len(observations)

        removed = 0
        for stale in files[:-self.keep_files] if self.keep_files < len(files) else []:
            stale.unlink()
            removed += 1

        remaining = self.series_files(census_dir)
        bytes_on_disk = sum(path.stat().st_size for path in remaining)
        return {
            "window": self.window,
            "keep_files": self.keep_files,
            "files": len(remaining),
            "removed_files": removed,
            "observations": len(observations),
            "dropped_observations": dropped,
            "bytes_on_disk": bytes_on_disk,
            "bytes_per_observation": per_observation,
            "bytes_bound": (
                None if per_observation is None
                else self.keep_files * self.window * per_observation
            ),
        }


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

    @property
    def current_backoff(self) -> float:
        """The backoff in force right now, so the status artifact's liveness
        deadline widens by exactly as much as a throttled run legitimately
        needs rather than by a guess."""
        return self._backoff


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

    IT ALSO CARRIES ITS OWN EXPIRY, and that is the part paid for in an
    outage.  On 2026-08-30 this run was killed mid-cycle by a full disk; it
    could not write anything on the way down, so the last status it had
    written stayed on disk saying `halted: false, stopping: false` -- an
    artifact still claiming health with no process behind it.  Nothing in the
    file itself contradicted that.  The reader caught it only because the page
    happened to apply its own fifteen-minute rule, which is a guess about a
    cadence the page cannot see.

    So every write stamps `heartbeat.expected_next_update_by`: the instant by
    which a LIVING run must have written again.  A reader compares it to their
    own clock and needs to know nothing about the simulator.  Past that
    instant the run is not writing, whatever `halted` says, and the artifact
    said so itself.

    The deadline is derived, not picked -- it is the longest gap a healthy run
    can leave between two writes:

        one jittered period            period * (1 + jitter_fraction)
      + the backoff currently in force (0, or up to the 120s cap under 429s)
      + one cycle of child processes   grace_seconds

    A death that CANNOT be recorded -- SIGKILL, or ENOSPC on the status write
    itself -- writes no halt record and no exit record, by construction.  That
    is the hard case and it is not solvable by writing more; it is solvable by
    making ABSENCE legible, which is what this deadline does.
    """

    path: Path
    cluster_label: str
    rpc_url: str
    mode: str
    market_address: Optional[str] = None
    started_at: str = dataclasses.field(default_factory=utc_now_iso)
    max_signatures: int = 50
    cadence_seconds: float = 0.0
    jitter_fraction: float = 0.25
    grace_seconds: float = 300.0

    def __post_init__(self) -> None:
        # The writer never holds the credential, so no write path can leak it.
        self.rpc_url = redact_endpoint(self.rpc_url)

    def heartbeat(self, now: dt.datetime, backoff_seconds: float = 0.0) -> dict:
        budget = (
            self.cadence_seconds * (1.0 + self.jitter_fraction)
            + max(0.0, backoff_seconds)
            + self.grace_seconds
        )
        deadline = now + dt.timedelta(seconds=budget)
        return {
            "cadence_seconds": self.cadence_seconds,
            "jitter_fraction": self.jitter_fraction,
            "grace_seconds": self.grace_seconds,
            "backoff_seconds": max(0.0, backoff_seconds),
            "budget_seconds": budget,
            "expected_next_update_by": deadline.isoformat(timespec="seconds"),
            "note": (
                "A living run rewrites this file every cycle. If your clock is past "
                "expected_next_update_by, no process is writing here any more -- read that "
                "and not the halted flag, which a run killed outright never gets to set. "
                "EXIT.json records how a run ended when it was able to; its absence past "
                "this instant means the run could not say."
            ),
        }

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
        backoff_seconds: float = 0.0,
        extra: Optional[dict] = None,
    ) -> dict:
        now = dt.datetime.now(dt.timezone.utc)
        body = {
            "schema": SCHEMA_STATUS,
            "cluster": {"label": self.cluster_label, "rpc_url": self.rpc_url},
            "market": {"address": self.market_address},
            "mode": self.mode,
            "started_at": self.started_at,
            "updated_at": now.isoformat(timespec="seconds"),
            "cycles": {"run": cycles_run, "target": cycles_target},
            "trades": {"landed": trades_landed, "signatures": signatures[-self.max_signatures:]},
            "wallets": wallets,
            "last_reconciliation": last_reconciliation,
            "halted": halted,
            "halt_reason": halt_reason,
            "stopping": stopping,
            "heartbeat": self.heartbeat(now, backoff_seconds),
        }
        if extra:
            body.update(extra)
        write_json_atomic(self.path, body)
        return body


def halt_loudly(work_dir: Path, reason: str, details: Optional[dict] = None) -> None:
    """Record the halt durably, then raise.  A halted work dir refuses restart
    until the HALT file is deliberately removed by a human.

    Every string it stores is redacted on the way in.  A halt record quotes
    the command that failed and the child's complaint, both of which carry
    `--rpc-url <credential>`; and HALT.json is precisely the file someone
    pastes into a report when a run stops, so it is the last place a provider
    key should survive.
    """
    body = {
        "schema": SCHEMA_HALT,
        "reason": redact_text(reason),
        "details": {
            key: redact_text(value) if isinstance(value, str) else value
            for key, value in (details or {}).items()
        },
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


EXIT_FILE = "EXIT.json"


def clear_exit_record(work_dir: Path) -> None:
    """Remove any previous run's exit record at startup.

    So that EXIT.json, whenever present, describes how THIS process ended --
    and so that its ABSENCE is a live claim rather than an artifact of never
    having been cleaned up.  Absence is the reading that matters: a run whose
    heartbeat deadline has passed with no exit record beside it did not get to
    say how it ended, which is what a SIGKILL or an ENOSPC death looks like
    from the outside.
    """
    try:
        (work_dir / EXIT_FILE).unlink()
    except FileNotFoundError:
        pass


def record_exit(
    work_dir: Path,
    outcome: str,
    *,
    detail: Optional[str] = None,
    cycles_run: Optional[int] = None,
    exit_code: Optional[int] = None,
) -> Optional[dict]:
    """Record how this run ended, durably and unconditionally.

    Written from a `finally`, so a crash gets a record as readily as a clean
    stop.  Unlike HALT.json this NEVER refuses a restart: how a process ended
    is a fact about the process, and only a conservation divergence is a fact
    about the ledger that a human must clear.

    BEST-EFFORT ON PURPOSE.  The deaths worth worrying about are exactly the
    ones that can defeat this: SIGKILL runs no handler, and a full volume
    fails the write.  Rather than pretend, it swallows its own write failure
    and says so on stderr -- the heartbeat deadline in status.json is what
    makes those deaths legible, and this record is what makes every other one
    precise.
    """
    body = {
        "schema": SCHEMA_EXIT,
        "outcome": outcome,
        # A crash detail is an exception message nobody composed field by
        # field, so it is redacted like any other stored free text.
        "detail": None if detail is None else redact_text(detail),
        "cycles_run": cycles_run,
        "exit_code": exit_code,
        "recorded_at": utc_now_iso(),
    }
    try:
        write_json_atomic(work_dir / EXIT_FILE, body)
    except OSError as error:
        print(
            f"could not record the exit ({outcome}): {error}. The heartbeat deadline in "
            "status.json is what remains honest here.",
            file=sys.stderr,
        )
        return None
    return body


@dataclasses.dataclass
class DiskFloor:
    """Refuse to be the writer that consumes the last of a shared volume.

    The 2026-08-30 outage was not survived by anybody; it was CAUSED by an
    unbounded writer, and every other lane on the machine lost its shell to
    ENOSPC because the harness stages command output on the same volume.
    Bounding the census (see CensusRetention) removes this run's own quadratic
    contribution.  This is the second half: whatever else is filling the disk,
    this process stops between cycles while there is still room to write down
    that it stopped, instead of dying mid-write and leaving an artifact that
    claims health.
    """

    floor_bytes: int = DEFAULT_DISK_FLOOR_BYTES

    def free_bytes(self, path: Path) -> int:
        return shutil.disk_usage(path).free

    def check(self, path: Path) -> Optional[str]:
        """None when there is room; otherwise the sentence to stop with."""
        free = self.free_bytes(path)
        if free >= self.floor_bytes:
            return None
        return (
            f"the volume holding {path} has {free} bytes free, under this run's "
            f"floor of {self.floor_bytes}; stopping between cycles while there is "
            "still room to record it rather than dying mid-write"
        )
