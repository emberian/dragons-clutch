#!/usr/bin/env python3
"""Deterministic hostile test double for lifecycle_chaos.py.

This is not protocol evidence.  It implements only the supervisor projection so
the process-kill, RPC-proxy, evidence-replacement, and snapshot comparisons can
be tested without a validator or keys.
"""

from __future__ import annotations

import base64
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
SIGNATURE = b"\0" * 64
RECENT_BLOCKHASH = b"\x07" * 32
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58_encode(value: bytes) -> str:
    zeroes = len(value) - len(value.lstrip(b"\0"))
    number = int.from_bytes(value, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = BASE58_ALPHABET[remainder] + encoded
    return "1" * zeroes + encoded


def fake_transaction() -> str:
    # One signature plus the smallest well-formed legacy message: one account,
    # one recent blockhash, and zero instructions.
    message = b"\x01\x00\x00\x01" + b"\0" * 32 + RECENT_BLOCKHASH + b"\x00"
    return base64.b64encode(b"\x01" + SIGNATURE + message).decode()


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

    if case in {"wallet-underfund", "wallet-surplus"}:
        wait_for(control / "FAULT.json")
        fault = json.loads((control / "FAULT.json").read_text())
        if fault.get("fault") != case:
            return 25
        write_atomic(
            control / "FAULT_ARMED.json",
            {"schema": CONTROL_SCHEMA, "state": "fault-armed", "fault": case},
        )
        wait_for(control / "GO.json")
        return 26

    wait_for(control / "GO.json")

    if case in {"corrupted-evidence", "replaced-evidence"}:
        if (case_work / "evidence.json").read_bytes() != EVIDENCE:
            return 23
        return 24

    journals = case_work / "journals"
    for stage in BOUNDARIES:
        path = journals / f"{stage}.json"
        if finalized(path, stage):
            continue
        try:
            current = json.loads(path.read_text())
        except (FileNotFoundError, json.JSONDecodeError):
            current = None
        resumed_submitted = current is not None and current.get("phase") == "submitted"
        if not resumed_submitted:
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

        if case == f"kill-{stage}":
            if resumed_submitted:
                try:
                    rpc(rpc_url, "getSignatureStatuses", [[base58_encode(SIGNATURE)]])
                except Exception:
                    return 27
            else:
                latest = rpc(
                    url=rpc_url,
                    method="getLatestBlockhash",
                    params=[{"commitment": "finalized"}],
                )
                if latest.get("result") is None:
                    return 28
                try:
                    rpc(
                        url=rpc_url,
                        method="sendTransaction",
                        params=[
                            fake_transaction(),
                            {"encoding": "base64", "maxRetries": 0},
                        ],
                    )
                except Exception:
                    return 29
                return 30

        if stage == "hot" and case in {"rpc-timeout", "duplicate-send", "blockhash-expiry"}:
            latest = rpc(
                url=rpc_url,
                method="getLatestBlockhash",
                params=[{"commitment": "finalized"}],
            )
            if latest.get("result") is None:
                return 31
            packet = fake_transaction()
            try:
                response = rpc(
                    url=rpc_url,
                    method="sendTransaction",
                    params=[packet, {"encoding": "base64", "maxRetries": 0}],
                )
            except Exception:
                if case != "rpc-timeout":
                    return 32
                # Unknown send result: recover by status polling only.
                try:
                    rpc(rpc_url, "getSignatureStatuses", [[base58_encode(SIGNATURE)]])
                except Exception:
                    return 33
                journal(path, stage, "finalized")
                continue
            if response.get("error") is not None:
                try:
                    rpc(rpc_url, "getSignatureStatuses", [[base58_encode(SIGNATURE)]])
                except Exception:
                    return 34
                return 35

        journal(path, stage, "finalized")
        if stage == "payout" and case == "late-child-refusal":
            write_atomic(
                control / "FAULT_ARMED.json",
                {"schema": CONTROL_SCHEMA, "state": "fault-armed", "fault": case},
            )
            wait_for(control / "FAULT_GO.json")
            return 36

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
