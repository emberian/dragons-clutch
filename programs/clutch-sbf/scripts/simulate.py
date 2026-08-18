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

The plan is `plan.json`, written by `clutch-sbf-harness`.  Every accepting case
names the exact writable accounts to compare and the oracle that produced the
expectation; every refusing case names the numeric `ProgramError::Custom` code
the program must return and the offline reference adapter's own refusal for the
same situation.
"""

from __future__ import annotations

import argparse
import base64
import json
import re
import sys
import urllib.request
from pathlib import Path

CONSUMED = re.compile(r"^Program (\S+) consumed (\d+) of (\d+) compute units$")


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


def simulate(url: str, plan: Path, case: dict, addresses: list[str]) -> dict:
    encoded = (plan / case["tx"]).read_text().strip()
    config = {
        "encoding": "base64",
        "sigVerify": False,
        "replaceRecentBlockhash": True,
        "commitment": "processed",
    }
    if addresses:
        config["accounts"] = {"encoding": "base64", "addresses": addresses}
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


def per_instruction_units(logs, program_id: str) -> list[int]:
    """Compute units the *program under test* consumed, one entry per invocation.

    Filtered by program id so that a `SetComputeUnitLimit` instruction ahead of
    the program instruction does not show up as a second measurement.
    """
    units = []
    for line in logs or []:
        match = CONSUMED.match(line)
        if match and match.group(1) == program_id:
            units.append(int(match.group(2)))
    return units


def granted_and_consumed(logs, program_id: str) -> tuple[int, int] | None:
    """The `consumed X of Y` pair the runtime logged for the program."""
    for line in logs or []:
        match = CONSUMED.match(line)
        if match and match.group(1) == program_id:
            return int(match.group(2)), int(match.group(3))
    return None


def check_exhausted(
    url: str,
    plan: Path,
    program_id: str,
    case: dict,
    report: list[str],
    failures: list[str],
) -> None:
    """Assert a documented compute-ceiling exhaustion.

    This is a claim that can go red: an instruction that became cheap enough to
    finish fails this check, which is the signal to re-measure it and rewrite
    the evidence rather than leave a stale "does not fit" in the docs.
    """
    result = simulate(url, plan, case, [])
    err = result["err"]
    detail = None
    if isinstance(err, dict) and isinstance(err.get("InstructionError"), list):
        detail = err["InstructionError"][1]
    pair = granted_and_consumed(result.get("logs"), program_id)
    if detail != "ProgramFailedToComplete" or pair is None or pair[0] != pair[1]:
        failures.append(
            f"{case['name']}: expected a compute-ceiling exhaustion, got err={err} "
            f"units={pair}"
        )
        for line in result.get("logs") or []:
            report.append(f"      log {line}")
        return
    report.append(
        f"  UNDRIVABLE {case['name']:<24} consumed {pair[0]} of {pair[1]} granted "
        f"and was aborted: does not fit one transaction"
    )


def read_hex(plan: Path, relative: str) -> str:
    return (plan / relative).read_text().strip()


def check_accept(
    url: str,
    plan: Path,
    program_id: str,
    case: dict,
    report: list[str],
    failures: list[str],
) -> None:
    entries = case["compare"]
    addresses = [entry["address"] for entry in entries]
    result = simulate(url, plan, case, addresses)
    if result["err"] is not None:
        failures.append(f"{case['name']}: expected success, got err={result['err']}")
        for line in result.get("logs") or []:
            report.append(f"      log {line}")
        return

    per = per_instruction_units(result.get("logs"), program_id)
    program_units = sum(per)
    detail = f" per-instruction {per}" if len(per) > 1 else ""
    limit = case.get("compute_limit")
    budget = f" limit={limit}" if limit else ""
    report.append(
        f"  accept {case['name']:<28} program_units={program_units}{detail} "
        f"tx_units={result.get('unitsConsumed')}{budget} "
        f"bytes={case['bytes']} oracle={case['oracle']}"
    )

    returned = result.get("accounts") or []
    if len(returned) != len(entries):
        failures.append(
            f"{case['name']}: expected {len(entries)} accounts back, got {len(returned)}"
        )
        return

    identical = set(case.get("identical_to_pre") or [])
    for entry, account in zip(entries, returned):
        observed = base64.b64decode(account["data"][0]).hex()
        expected = read_hex(plan, entry["expected"])
        pre = read_hex(plan, entry["pre"])
        role = entry["role"]
        if observed != expected:
            failures.append(
                f"{case['name']} / {role}: on-chain bytes != oracle bytes"
            )
            report.append(f"      oracle   {expected}")
            report.append(f"      on-chain {observed}")
            continue
        if role in identical and observed != pre:
            failures.append(
                f"{case['name']} / {role}: expected to be untouched, but it moved"
            )
            continue
        if role not in identical and observed == pre:
            failures.append(
                f"{case['name']} / {role}: expected to move, but it is unchanged"
            )
            continue
        state = "unchanged" if role in identical else "changed"
        report.append(f"    differential {role:<22} MATCH ({state})")


def check_refuse(url: str, plan: Path, case: dict, report: list[str], failures: list[str]) -> None:
    result = simulate(url, plan, case, [])
    expected = int(case["expect_code"])
    code = custom_error_code(result["err"])
    if code == expected:
        report.append(
            f"  refuse {case['name']:<28} Custom({case['expect_code_hex']})  "
            f"offline reference: {case['reference']}"
        )
        return
    failures.append(
        f"{case['name']}: expected Custom({case['expect_code_hex']}), got err={result['err']}"
    )
    for line in result.get("logs") or []:
        report.append(f"      log {line}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:18899")
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--only", default=None, help="run one case by name")
    args = parser.parse_args()

    plan = json.loads((args.plan / "plan.json").read_text())
    failures: list[str] = []
    report: list[str] = []

    families: dict[str, list[dict]] = {}
    for case in plan["cases"]:
        if args.only and case["name"] != args.only:
            continue
        families.setdefault(case["family"], []).append(case)

    for family, cases in families.items():
        report.append(f"\n== {family} ==")
        for case in cases:
            if case["kind"] == "exhausted":
                check_exhausted(
                    args.url, args.plan, plan["program_id"], case, report, failures
                )
            elif case["kind"] == "accept":
                check_accept(
                    args.url, args.plan, plan["program_id"], case, report, failures
                )
            else:
                check_refuse(args.url, args.plan, case, report, failures)

    print("\n".join(report))
    accepts = sum(1 for case in plan["cases"] if case["kind"] == "accept")
    refuses = sum(1 for case in plan["cases"] if case["kind"] == "refuse")
    exhausted = [case["name"] for case in plan["cases"] if case["kind"] == "exhausted"]
    print(
        f"\n{accepts} accepting transactions, {refuses} refusals, "
        f"{len(exhausted)} undrivable, {len(plan['genesis'])} genesis accounts"
    )
    if exhausted:
        print(
            "  UNDRIVABLE on this runtime (compute ceiling, documented in "
            "SBF_BRINGUP.md): " + ", ".join(exhausted)
        )
    if failures:
        print("\nFAIL")
        for failure in failures:
            print(f"  {failure}")
        return 1
    print("\nPASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
