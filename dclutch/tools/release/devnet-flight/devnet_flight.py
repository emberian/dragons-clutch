#!/usr/bin/env python3
"""Resumable orchestration for the already-owned devnet commands.

This is deliberately a process coordinator, not a release, market, activity,
or deployment validator.  Each command remains the semantic owner of its
arguments and evidence.  The flight journal only records the command digest
and durable dispatch state, never command output or key material.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any, Callable


DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
SCHEMA = "dclutch-devnet-flight-v1"
JOURNAL_SCHEMA = "dclutch-devnet-flight-journal-v1"
ROLES = ("custody", "resolution", "claims", "trading", "core")
REQUIRED = (
    "candidate",
    "buffer:custody", "upgrade:custody",
    "buffer:resolution", "upgrade:resolution",
    "buffer:claims", "upgrade:claims",
    "buffer:trading", "upgrade:trading",
    "buffer:core", "upgrade:core",
    "sponsored-market-open", "participant-lifecycle", "direct-lifecycle", "terminal-lifecycle",
    "finite-activity", "reconcile", "site-refresh", "wrapper-pages-checkpoint",
)


class FlightError(ValueError):
    pass


def canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def digest(argv: list[str]) -> str:
    return hashlib.sha256(canonical(argv)).hexdigest()


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def load_json(path: Path) -> dict[str, Any]:
    try:
        raw = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise FlightError(f"cannot read strict JSON {path}: {error}") from error
    if not isinstance(raw, dict):
        raise FlightError("flight document must be a JSON object")
    return raw


def absolute_regular(path: Path, label: str, *, existing: bool) -> Path:
    if not path.is_absolute():
        raise FlightError(f"{label} must be absolute")
    if path.is_symlink():
        raise FlightError(f"{label} must not be a symlink")
    if existing and not path.is_file():
        raise FlightError(f"{label} must be an existing regular file")
    if not existing:
        parent = path.parent
        if not parent.is_dir() or parent.is_symlink():
            raise FlightError(f"{label} parent must be an existing non-symlink directory")
    return path


def argv_of(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
        raise FlightError(f"{label} argv must be a non-empty string array")
    return value


def command_name(argv: list[str]) -> str:
    return " ".join(argv[:2])


def validate_flight(flight: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    expected = {"schema", "target", "bufferStrategy", "commands"}
    unknown = set(flight) - expected
    if unknown or set(flight) != expected:
        raise FlightError("flight has exactly schema, target, and commands")
    if flight["schema"] != SCHEMA:
        raise FlightError(f"flight schema must be {SCHEMA}")
    target = flight["target"]
    if target != {"cluster": "devnet", "genesis": DEVNET_GENESIS}:
        raise FlightError("flight target must be the exact acknowledged devnet genesis")
    buffer_strategy = flight["bufferStrategy"]
    if buffer_strategy not in {"batched", "interleaved"}:
        raise FlightError("bufferStrategy must be batched or interleaved")
    commands = flight["commands"]
    if not isinstance(commands, list):
        raise FlightError("commands must be an array")
    seen: set[str] = set()
    parsed: list[dict[str, Any]] = []
    for index, item in enumerate(commands):
        if not isinstance(item, dict) or set(item) != {"id", "mutation", "argv"}:
            raise FlightError(f"commands[{index}] has exactly id, mutation, and argv")
        ident, mutation, argv = item["id"], item["mutation"], item["argv"]
        if not isinstance(ident, str) or not ident or ident in seen:
            raise FlightError(f"commands[{index}] id must be unique and non-empty")
        if not isinstance(mutation, bool):
            raise FlightError(f"commands[{index}] mutation must be boolean")
        seen.add(ident)
        parsed.append({"id": ident, "mutation": mutation, "argv": argv_of(argv, ident)})
    required_missing = set(REQUIRED) - seen
    if required_missing:
        raise FlightError("missing required commands: " + ", ".join(sorted(required_missing)))
    allowed = set(REQUIRED) | {f"extend:{role}" for role in ROLES}
    extra = seen - allowed
    if extra:
        raise FlightError("unknown command ids: " + ", ".join(sorted(extra)))
    order = [item["id"] for item in parsed]
    expected_order = ["candidate"]
    if buffer_strategy == "batched":
        expected_order.extend(f"extend:{role}" for role in ROLES if f"extend:{role}" in seen)
        expected_order.extend(f"buffer:{role}" for role in ROLES)
        expected_order.extend(f"upgrade:{role}" for role in ROLES)
    else:
        for role in ROLES:
            if f"extend:{role}" in seen:
                expected_order.append(f"extend:{role}")
            expected_order.extend((f"buffer:{role}", f"upgrade:{role}"))
    expected_order.extend(REQUIRED[11:])
    if order != expected_order:
        raise FlightError(f"commands do not match declared {buffer_strategy} bufferStrategy order")
    for item in parsed:
        ident, argv = item["id"], item["argv"]
        if ident.startswith("buffer:") and "--stop-after-buffer-ready" not in argv:
            raise FlightError(f"{ident} must use the existing --stop-after-buffer-ready boundary")
        if ident.startswith("upgrade:") and "--stop-after-buffer-ready" in argv:
            raise FlightError(f"{ident} must be the post-buffer Upgrade invocation")
    return parsed, target


def has_existing_devnet_ack(argv: list[str]) -> bool:
    return any(argv[index] == "--i-mean-devnet" and argv[index + 1] == DEVNET_GENESIS for index in range(len(argv) - 1))


def has_existing_mutation_phrase(argv: list[str]) -> bool:
    return any(token == "--execute" or token.startswith("--i-accept-") or token.startswith("--i-kept-") for token in argv)


def initial_journal(flight_path: Path, flight: dict[str, Any], commands: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "schema": JOURNAL_SCHEMA,
        "flightPath": str(flight_path),
        "flightSha256": hashlib.sha256(canonical(flight)).hexdigest(),
        "target": flight["target"],
        "commands": [{"id": item["id"], "mutation": item["mutation"], "argvSha256": digest(item["argv"]), "state": "pending"} for item in commands],
        "events": [],
    }


def write_journal(path: Path, journal: dict[str, Any]) -> None:
    temp = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    encoded = json.dumps(journal, indent=2, sort_keys=True) + "\n"
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, "w") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp, path)
        os.chmod(path, 0o600)
    finally:
        if temp.exists():
            temp.unlink()


def journal_for(path: Path, flight_path: Path, flight: dict[str, Any], commands: list[dict[str, Any]]) -> dict[str, Any]:
    if not path.exists():
        journal = initial_journal(flight_path, flight, commands)
        write_journal(path, journal)
        return journal
    journal = load_json(path)
    expected = initial_journal(flight_path, flight, commands)
    for key in ("schema", "flightPath", "flightSha256", "target"):
        if journal.get(key) != expected[key]:
            raise FlightError(f"existing journal {key} does not match this immutable flight")
    actual_commands = journal.get("commands")
    expected_commands = expected["commands"]
    if not isinstance(actual_commands, list) or len(actual_commands) != len(expected_commands):
        raise FlightError("existing journal command set does not match this immutable flight")
    for actual, expected_row in zip(actual_commands, expected_commands):
        if not isinstance(actual, dict) or any(actual.get(key) != expected_row[key] for key in ("id", "mutation", "argvSha256")):
            raise FlightError("existing journal command binding does not match this immutable flight")
        if actual.get("state") not in {"pending", "dispatching", "failed", "finalized"}:
            raise FlightError("existing journal has an invalid command state")
    if not isinstance(journal.get("events"), list):
        raise FlightError("journal events must be an array")
    return journal


def execute(commands: list[dict[str, Any]], journal_path: Path, journal: dict[str, Any], runner: Callable[..., Any]) -> None:
    rows = {row["id"]: row for row in journal["commands"]}
    for command in commands:
        row = rows[command["id"]]
        if row["state"] == "finalized":
            continue
        argv = command["argv"]
        if command["mutation"]:
            if not has_existing_devnet_ack(argv):
                raise FlightError(f"{command['id']} mutation lacks existing --i-mean-devnet acknowledgement")
            if not has_existing_mutation_phrase(argv):
                raise FlightError(f"{command['id']} mutation lacks its existing authorization phrase")
            row["state"] = "dispatching"
            journal["events"].append({"at": now(), "id": command["id"], "event": "before-external-mutation", "argvSha256": digest(argv)})
            write_journal(journal_path, journal)
        result = runner(argv, check=False, shell=False, capture_output=True, text=True)
        if result.returncode != 0:
            row["state"] = "failed"
            journal["events"].append({"at": now(), "id": command["id"], "event": "failed", "returncode": result.returncode})
            write_journal(journal_path, journal)
            raise FlightError(f"{command['id']} failed with exit status {result.returncode}; inspect the command's own evidence")
        row["state"] = "finalized"
        journal["events"].append({"at": now(), "id": command["id"], "event": "finalized"})
        write_journal(journal_path, journal)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--flight", required=True, type=Path)
    parser.add_argument("--journal", required=True, type=Path)
    parser.add_argument("--execute", action="store_true", help="dispatch the already-authorized command argv arrays")
    args = parser.parse_args(argv)
    try:
        flight_path = absolute_regular(args.flight, "--flight", existing=True)
        journal_path = absolute_regular(args.journal, "--journal", existing=False)
        flight = load_json(flight_path)
        commands, _target = validate_flight(flight)
        if not args.execute:
            print(json.dumps({"schema": SCHEMA, "mode": "plan", "mutationPermitted": False,
                              "commands": [{"id": row["id"], "mutation": row["mutation"], "argvSha256": digest(row["argv"])} for row in commands]}, indent=2))
            return 0
        journal = journal_for(journal_path, flight_path, flight, commands)
        execute(commands, journal_path, journal, subprocess.run)
        print(json.dumps({"schema": JOURNAL_SCHEMA, "journal": str(journal_path), "status": "finalized"}, indent=2))
        return 0
    except FlightError as error:
        print(f"devnet-flight: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
