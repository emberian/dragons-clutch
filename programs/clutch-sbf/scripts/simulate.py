#!/usr/bin/env python3
"""Differential check: run the bring-up plan against a local SVM and compare.

This script talks only to a loopback JSON-RPC endpoint served by a local
`solana-test-validator`.  It never contacts a public cluster, never signs, and
never sends a committing transaction: every call is `simulateTransaction` with
`sigVerify: false` and `replaceRecentBlockhash: true`.

What that establishes and what it does not is written up in
`docs/implementation/SBF_BRINGUP.md`.  In short: the SBF program really is
loaded and executed by an Agave bank, the account data really is serialized
into the VM and written back, and the `is_signer` bits the program reads really
do come from the transaction message header -- but no Ed25519 signature is
verified, and nothing is committed to a ledger.
"""

from __future__ import annotations

import argparse
import base64
import json
import sys
import urllib.request
from pathlib import Path

STATE_ROLES = ["market", "hoard", "position", "kernel", "external", "replay"]


def rpc(url: str, method: str, params: list) -> dict:
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    request = urllib.request.Request(
        url, data=payload, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        body = json.loads(response.read().decode())
    if "error" in body:
        raise SystemExit(f"RPC error for {method}: {body['error']}")
    return body["result"]


def read_manifest(plan: Path) -> dict:
    values = {}
    for line in (plan / "manifest.txt").read_text().splitlines():
        if line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key] = value
    return values


def simulate(url: str, plan: Path, name: str, addresses: list[str]) -> dict:
    encoded = (plan / "tx" / f"{name}.b64").read_text().strip()
    config = {
        "encoding": "base64",
        "sigVerify": False,
        "replaceRecentBlockhash": True,
        "commitment": "processed",
        "accounts": {"encoding": "base64", "addresses": addresses},
    }
    return rpc(url, "simulateTransaction", [encoded, config])["value"]


def custom_error_code(err) -> int | None:
    """Extract `InstructionError::Custom(code)` from a simulation error."""
    if not isinstance(err, dict):
        return None
    instruction_error = err.get("InstructionError")
    if not isinstance(instruction_error, list) or len(instruction_error) != 2:
        return None
    detail = instruction_error[1]
    if isinstance(detail, dict) and "Custom" in detail:
        return int(detail["Custom"])
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:18899")
    parser.add_argument("--plan", required=True, type=Path)
    args = parser.parse_args()

    manifest = read_manifest(args.plan)
    addresses = [manifest[f"account.{role}"] for role in STATE_ROLES]
    failures: list[str] = []
    report: list[str] = []

    accept = simulate(args.url, args.plan, "accept", addresses)
    if accept["err"] is not None:
        failures.append(f"accept: expected success, got err={accept['err']}")
        for line in accept.get("logs") or []:
            report.append(f"    log {line}")
    else:
        report.append(f"  accept: executed, unitsConsumed={accept.get('unitsConsumed')}")

    returned = accept.get("accounts") or []
    if len(returned) != len(STATE_ROLES):
        failures.append(
            f"accept: expected {len(STATE_ROLES)} accounts back, got {len(returned)}"
        )
    else:
        for role, account in zip(STATE_ROLES, returned):
            observed = base64.b64decode(account["data"][0]).hex()
            expected = (args.plan / "expected" / f"{role}.hex").read_text().strip()
            pre = (args.plan / "expected" / f"{role}.pre.hex").read_text().strip()
            if observed == expected:
                changed = "changed" if expected != pre else "unchanged"
                report.append(f"  differential {role:<9} MATCH ({changed} by Split)")
            else:
                failures.append(f"differential {role}: on-chain bytes != reference bytes")
                report.append(f"    reference {expected}")
                report.append(f"    on-chain  {observed}")

    for name in ["refuse-unsigned", "refuse-stranger", "refuse-imposter"]:
        expected_code = int(manifest[f"expect.{name}"], 16)
        result = simulate(args.url, args.plan, name, [])
        code = custom_error_code(result["err"])
        reference = manifest.get(f"reference.{name}", "?")
        if code == expected_code:
            report.append(
                f"  refusal {name:<16} Custom(0x{code:04x}) "
                f"(offline reference: {reference})"
            )
        else:
            failures.append(
                f"refusal {name}: expected Custom(0x{expected_code:04x}), "
                f"got err={result['err']}"
            )
            for line in result.get("logs") or []:
                report.append(f"    log {line}")

    print("\n".join(report))
    if failures:
        print("\nFAIL")
        for failure in failures:
            print(f"  {failure}")
        return 1
    print("\nPASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
