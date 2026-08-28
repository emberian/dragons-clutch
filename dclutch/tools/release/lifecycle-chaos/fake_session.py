#!/usr/bin/env python3
"""Deterministic hostile test double for lifecycle_chaos.py.

This is not protocol evidence.  It implements only the supervisor projection so
the process-kill, RPC-proxy, evidence-replacement, and snapshot comparisons can
be tested without a validator or keys.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys
import time
from urllib import request as urlrequest


CONTROL_SCHEMA = "dclutch-lifecycle-chaos-control-v1"
JOURNAL_SCHEMA = "dclutch-lifecycle-chaos-stage-projection-v1"
SESSION_SCHEMA = "dclutch-fake-owned-loopback-lifecycle-v1"
SNAPSHOT_SCHEMA = "dclutch-lifecycle-chaos-snapshot-v1"
BOUNDARIES = (
    "founding",
    "participant",
    "alt",
    "seal",
    "hot",
    "resolution",
    "payout",
    "retire",
)
EVIDENCE = b'{"schema":"fake-canonical-evidence-v1","market":"one"}\n'


def write_atomic(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    with temporary.open("x") as target:
        json.dump(value, target, indent=2, sort_keys=True)
        target.write("\n")
        target.flush()
        os.fsync(target.fileno())
    os.replace(temporary, path)


def wait_for(path: Path) -> None:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if path.is_file():
            return
        time.sleep(0.01)
    raise RuntimeError(f"missing control file {path}")


def intent(stage: str) -> str:
    return hashlib.sha256(f"fake-intent:{stage}".encode()).hexdigest()


def journal(path: Path, stage: str, phase: str) -> None:
    write_atomic(
        path,
        {
            "schema": JOURNAL_SCHEMA,
            "stage": stage,
            "phase": phase,
            "intentSha256": intent(stage),
        },
    )


def rpc(url: str, method: str, params: list[object]) -> dict[str, object]:
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    request = urlrequest.Request(url, data=body, headers={"content-type": "application/json"})
    with urlrequest.urlopen(request, timeout=0.25) as response:  # noqa: S310 test loopback
        return json.load(response)


def finalized(path: Path, stage: str) -> bool:
    try:
        value = json.loads(path.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return False
    return value.get("stage") == stage and value.get("phase") == "finalized"


def run_session(case_work: Path, rpc_url: str) -> int:
    control = Path(os.environ["DCLUTCH_LIFECYCLE_CHAOS_CONTROL"])
    case = os.environ["DCLUTCH_LIFECYCLE_CHAOS_CASE"]
    prepared = control / "PREPARED.json"
    if not prepared.exists():
        write_atomic(prepared, {"schema": CONTROL_SCHEMA, "state": "prepared"})
    wait_for(control / "GO.json")

    if case in {"corrupted-evidence", "replaced-evidence"}:
        if (case_work / "evidence.json").read_bytes() != EVIDENCE:
            return 23
        return 24
    if case in {"wallet-underfund", "wallet-surplus"}:
        fault = json.loads((control / "FAULT.json").read_text())
        return 25 if fault.get("fault") == case else 26

    journals = case_work / "journals"
    for stage in BOUNDARIES:
        path = journals / f"{stage}.json"
        if finalized(path, stage):
            continue
        try:
            current = json.loads(path.read_text())
        except (FileNotFoundError, json.JSONDecodeError):
            current = None
        if current is None or current.get("phase") not in {"submitted", "finalized"}:
            journal(path, stage, "planned")
            journal(path, stage, "signed-not-submitted")
            journal(path, stage, "submitted")
            # The real commands remain in Submitted while polling finality.  This
            # window makes the process-death test deterministic without adding a
            # supervisor-specific pause to a semantic owner.
            time.sleep(0.15)
        else:
            # A restart at Submitted polls the frozen intent.  It never sends it.
            time.sleep(0.01)

        if stage == "hot" and case in {"rpc-timeout", "duplicate-send", "blockhash-expiry"}:
            packet = "ZmFrZS1mcm96ZW4tdHJhbnNhY3Rpb24="
            try:
                response = rpc(url=rpc_url, method="sendTransaction", params=[packet])
            except Exception:
                if case != "rpc-timeout":
                    return 31
                # Unknown send result: recover by status polling only.
                try:
                    rpc(rpc_url, "getSignatureStatuses", [["fake-signature"]])
                except Exception:
                    return 32
                journal(path, stage, "finalized")
                continue
            if response.get("error") is not None:
                try:
                    rpc(rpc_url, "getSignatureStatuses", [["fake-signature"]])
                except Exception:
                    return 35
                return 33

        journal(path, stage, "finalized")
        if stage == "payout" and case == "late-child-refusal":
            return 34

    final_state = {
        "schema": SNAPSHOT_SCHEMA,
        "accounts": [
            {
                "address": "Account111111111111111111111111111111111",
                "owner": "Owner11111111111111111111111111111111111",
                "lamports": 97,
                "executable": False,
                "dataBase64": "ZmluYWw=",
                "dataSha256": hashlib.sha256(b"final").hexdigest(),
            }
        ],
        "totals": {"accountCount": 1, "lamports": 97},
    }
    write_atomic(case_work / "state.json", final_state)
    write_atomic(
        case_work / "session.json",
        {
            "schema": SESSION_SCHEMA,
            "status": "finalized",
            "stages": [
                {
                    "stage": stage,
                    "status": "finalized",
                    "intentSha256": intent(stage),
                }
                for stage in BOUNDARIES
            ],
        },
    )
    return 0


def observe(case_work: Path) -> int:
    sys.stdout.buffer.write((case_work / "state.json").read_bytes())
    return 0


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[0] not in {"session", "observe"}:
        print("usage: fake_session.py session|observe CASE_WORK RPC_URL", file=sys.stderr)
        return 2
    case_work = Path(argv[1])
    if argv[0] == "observe":
        return observe(case_work)
    return run_session(case_work, argv[2])


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
