#!/usr/bin/env python3
"""Materialize finite Direct private sessions before Activity V4 authorization.

This is deliberately a preparation-only adapter.  It invokes the accepted
successor's key-free session producer, never opens a key file, calls no RPC,
and leaves V3's manifest/session semantic owner unchanged.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
from typing import Any, Mapping, Sequence

ACTIVITY_PATH = Path(__file__).with_name("activity.py")
SCHEMA = "dclutch-devnet-activity-direct-session-preparation-manifest-v1"
JOURNAL_SCHEMA = "dclutch-devnet-direct-trade-session-producer-journal-v1"
COMMAND = "devnet-direct-trade-session-produce-v1"


class Refusal(RuntimeError):
    pass


def load_activity() -> Any:
    spec = importlib.util.spec_from_file_location("dclutch_activity_direct_prepare", ACTIVITY_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


activity = load_activity()


def exact_object(value: Any, label: str) -> dict[str, Any]:
    try:
        return activity.exact_object(value, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def exact_list(value: Any, label: str) -> list[Any]:
    try:
        return activity.exact_list(value, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def exact_keys(value: Mapping[str, Any], keys: set[str], label: str) -> None:
    try:
        activity.exact_keys(value, keys, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def digest(value: Any, label: str) -> str:
    try:
        return activity.digest_text(value, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def accepted_file(value: Any, label: str) -> tuple[Path, str]:
    row = exact_object(value, label)
    exact_keys(row, {"path", "sha256"}, label)
    expected = digest(row["sha256"], f"{label} digest")
    try:
        path = activity.canonical_existing_file(row["path"], label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    if activity.sha256_file(path) != expected:
        raise Refusal(f"{label} differs from its accepted SHA-256")
    return path, expected


def output_path(value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value or value.strip() != value:
        raise Refusal(f"{label} must be canonical text")
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or path.parent.is_symlink() or not path.parent.is_dir():
        raise Refusal(f"{label} must have one existing absolute non-symlink parent")
    if path.exists() and (path.is_symlink() or not path.is_file()):
        raise Refusal(f"{label} exists with another kind")
    return path.resolve(strict=False)


def producer_state_sha256(value: Mapping[str, Any]) -> str:
    copy = dict(value)
    copy["stateSha256"] = ""
    return hashlib.sha256(json.dumps(copy, separators=(",", ":")).encode()).hexdigest()


def finalize(producer: Mapping[str, Any], label: str) -> None:
    journal_path, journal_sha = accepted_file(producer["producerJournal"], f"{label} producer journal")
    value = exact_object(activity.read_exact_json(journal_path, f"{label} producer journal"), f"{label} producer journal")
    if value.get("schema") != JOURNAL_SCHEMA or value.get("phase") != "finalized":
        raise Refusal(f"{label} producer journal is not Finalized")
    if digest(value.get("stateSha256"), f"{label} producer state") != producer_state_sha256(value):
        raise Refusal(f"{label} producer journal state changed")
    session = output_path(producer["session"], f"{label} session")
    session_sha = activity.sha256_file(session)
    if value.get("privateSession") != str(session) or value.get("privateSessionSha256") != session_sha:
        raise Refusal(f"{label} Finalized journal does not bind its produced session")
    if journal_sha != activity.sha256_file(journal_path):
        raise Refusal(f"{label} producer journal changed after acceptance")


def prepare(path: Path, expected_sha256: str) -> None:
    manifest_path, _ = accepted_file({"path": str(path), "sha256": expected_sha256}, "preparation manifest")
    value = exact_object(activity.read_exact_json(manifest_path, "preparation manifest"), "preparation manifest")
    exact_keys(value, {"schema", "successor", "cycles"}, "preparation manifest")
    if value["schema"] != SCHEMA:
        raise Refusal("preparation manifest schema changed")
    successor, _ = accepted_file(value["successor"], "preparation successor")
    if not successor.stat().st_mode & 0o111:
        raise Refusal("preparation successor is not executable")
    seen_outputs: set[Path] = set()
    for cycle_index, raw_cycle in enumerate(exact_list(value["cycles"], "preparation cycles")):
        cycle = exact_object(raw_cycle, f"preparation cycle {cycle_index}")
        exact_keys(cycle, {"cycleId", "producers"}, f"preparation cycle {cycle_index}")
        activity.stable_id(cycle["cycleId"], f"preparation cycle {cycle_index} id")
        for index, raw in enumerate(exact_list(cycle["producers"], f"preparation cycle {cycle_index} producers")):
            producer = exact_object(raw, f"preparation producer {cycle_index}/{index}")
            exact_keys(producer, {"publicManifest", "plan", "marketInput", "sellerParticipant", "buyerParticipant", "payerKeypair", "journalDir", "evidenceFile", "session", "producerJournal"}, f"preparation producer {cycle_index}/{index}")
            files = {key: accepted_file(producer[key], f"preparation {key}") for key in ("publicManifest", "plan", "marketInput", "sellerParticipant", "buyerParticipant")}
            payer = output_path(producer["payerKeypair"], "preparation payer keypair")
            # The producer authenticates the runtime key path but does not open it.
            journal_dir = Path(producer["journalDir"])
            if not journal_dir.is_absolute() or journal_dir.is_symlink() or not journal_dir.parent.is_dir():
                raise Refusal("preparation journalDir is not an absolute safe target")
            session = output_path(producer["session"], "preparation session")
            journal = output_path(producer["producerJournal"], "preparation producer journal")
            evidence = output_path(producer["evidenceFile"], "preparation evidence")
            if len({session, journal, evidence}) != 3 or any(item in seen_outputs for item in (session, journal, evidence)):
                raise Refusal("preparation producer outputs alias or repeat across cycles")
            seen_outputs.update((session, journal, evidence))
            argv = [str(successor), COMMAND, "--i-mean-devnet", activity.DEVNET_GENESIS_HASH]
            for flag, key in (("--public-manifest", "publicManifest"), ("--plan", "plan"), ("--market-input", "marketInput"), ("--seller-participant", "sellerParticipant"), ("--buyer-participant", "buyerParticipant")):
                argv += [flag, str(files[key][0]), f"--expected-{flag[2:]}-sha256", files[key][1]]
            argv += ["--payer-keypair", str(payer), "--journal-dir", str(journal_dir), "--evidence-file", str(evidence), "--session", str(session), "--producer-journal", str(journal)]
            result = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60, check=False)
            if result.returncode != 0:
                raise Refusal(f"preparation producer {cycle_index}/{index} refused")
            finalize(producer, f"preparation producer {cycle_index}/{index}")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    try:
        arguments = parser.parse_args(argv)
        prepare(Path(arguments.manifest), arguments.manifest_sha256)
        return 0
    except Refusal as error:
        print(f"direct session preparation refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
