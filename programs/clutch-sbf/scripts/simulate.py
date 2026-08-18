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

Two gates live here.  Without `--lifecycle` this runs the per-family plan:
every implemented instruction family, its accepting transactions and its
refusals, grouped by family.  With `--lifecycle` it runs the PROJECT.md
section-10 walk instead -- one market taken end to end as ONE ordered gate,
step by step, closing with the section-10 item-10 accounting identity read
out of the bytes the bank returned.  See
`docs/implementation/LIFECYCLE_WALK.md`.
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
) -> dict:
    """Run one accepting transaction and compare every writable account.

    Returns the measurement the lifecycle walk tabulates -- compute units, the
    transaction size, how many accounts the step actually wrote -- together
    with the raw on-chain bytes the SVM returned, so a later step can read a
    field out of them rather than out of an expectation file.
    """
    entries = case["compare"]
    addresses = [entry["address"] for entry in entries]
    result = simulate(url, plan, case, addresses)
    metrics = {
        "units": None,
        "per_instruction": [],
        "bytes": case["bytes"],
        "written": 0,
        "verdict": "FAIL",
        "observed": {},
    }
    if result["err"] is not None:
        failures.append(f"{case['name']}: expected success, got err={result['err']}")
        for line in result.get("logs") or []:
            report.append(f"      log {line}")
        return metrics

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

    metrics["units"] = program_units
    metrics["per_instruction"] = per

    returned = result.get("accounts") or []
    if len(returned) != len(entries):
        failures.append(
            f"{case['name']}: expected {len(entries)} accounts back, got {len(returned)}"
        )
        return metrics

    identical = set(case.get("identical_to_pre") or [])
    matched = 0
    for entry, account in zip(entries, returned):
        raw = base64.b64decode(account["data"][0])
        metrics["observed"][entry["role"]] = raw
        observed = raw.hex()
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
        if role not in identical:
            metrics["written"] += 1
        matched += 1
        report.append(f"    differential {role:<22} MATCH ({state})")
    if matched == len(entries):
        metrics["verdict"] = "MATCH"
    return metrics


def check_refuse(
    url: str, plan: Path, program_id: str, case: dict, report: list[str], failures: list[str]
) -> dict:
    result = simulate(url, plan, case, [])
    expected = int(case["expect_code"])
    code = custom_error_code(result["err"])
    pair = granted_and_consumed(result.get("logs"), program_id)
    metrics = {
        "units": pair[0] if pair else None,
        "per_instruction": [],
        "bytes": case["bytes"],
        "written": 0,
        "verdict": "FAIL",
        "observed": {},
    }
    if code == expected:
        metrics["verdict"] = f"REFUSED Custom({case['expect_code_hex']})"
        report.append(
            f"  refuse {case['name']:<28} Custom({case['expect_code_hex']})  "
            f"offline reference: {case['reference']}"
        )
        return metrics
    failures.append(
        f"{case['name']}: expected Custom({case['expect_code_hex']}), got err={result['err']}"
    )
    for line in result.get("logs") or []:
        report.append(f"      log {line}")
    return metrics



def evaluate_terms(terms: list[dict], observed: dict[str, int]) -> tuple[int, str]:
    """Sum one side of an accounting identity from values read off the chain."""
    total = 0
    parts = []
    for term in terms:
        if "constant" in term:
            total += int(term["value"])
            parts.append(f"{term['constant']}={term['value']}")
            continue
        label = term["label"]
        scale = int(term["scale"])
        value = observed[label]
        total += value * scale
        parts.append(f"{scale}*{label}({value})" if scale != 1 else f"{label}({value})")
    return total, " + ".join(parts)


def check_terminal(
    terminal: dict,
    observed_bytes: dict[str, dict[str, bytes]],
    report: list[str],
    failures: list[str],
) -> None:
    """Close PROJECT.md section 10 item 10 over the bytes the SVM returned.

    Every number below is read out of the account data the bank handed back
    for the walk's last step, at an offset the harness located by probing the
    frozen codec rather than by hard-coding it.  Nothing here consults an
    expectation file: the byte differential already established that the
    on-chain bytes and the oracle's bytes are the same bytes, and this is the
    separate question of whether those bytes *close*.
    """
    case_name = terminal["case"]
    accounts = observed_bytes.get(case_name)
    if not accounts:
        failures.append(f"terminal identity: no on-chain bytes for {case_name}")
        return

    observed: dict[str, int] = {}
    report.append("")
    report.append("  terminal state, read out of the on-chain bytes:")
    for value in terminal["values"]:
        data = accounts.get(value["role"])
        if data is None:
            failures.append(
                f"terminal identity: {case_name} returned no {value['role']}"
            )
            return
        offset, width = int(value["offset"]), int(value["width"])
        if len(data) < offset + width:
            failures.append(
                f"terminal identity: {value['role']} is {len(data)} bytes, "
                f"too short for {value['label']} at {offset}"
            )
            return
        read = int.from_bytes(data[offset : offset + width], "little")
        observed[value["label"]] = read
        mark = "ok" if read == int(value["expected"]) else "MISMATCH"
        if read != int(value["expected"]):
            failures.append(
                f"terminal identity: {value['label']} is {read} on chain, "
                f"expected {value['expected']}"
            )
        report.append(
            f"    {value['label']:<24} {read:<12} ({mark}, {value['role']} +{offset})"
        )

    report.append("")
    report.append("  accounting identities:")
    for identity in terminal["identities"]:
        left, left_text = evaluate_terms(identity["left"], observed)
        right, right_text = evaluate_terms(identity["right"], observed)
        if left == right:
            report.append(f"    CLOSED  {identity['equation']}")
            report.append(f"              {left_text} = {left} = {right_text}")
            continue
        failures.append(
            f"terminal identity `{identity['equation']}` does not close: "
            f"{left_text} = {left} != {right} = {right_text}"
        )


def run_lifecycle(url: str, plan_dir: Path, plan: dict, only: str | None) -> int:
    """Run the PROJECT.md section-10 walk as ONE gate.

    The walk is one market taken end to end.  It is not ten independent
    checks that happen to be printed together: any step that diverges, refuses
    when it should accept, or accepts when it should refuse fails the walk as
    a unit, and the terminal accounting identity is part of the same gate.
    """
    walk = plan["lifecycle"]
    program_id = plan["program_id"]
    cases = {case["name"]: case for case in walk["cases"]}
    failures: list[str] = []
    report: list[str] = []
    rows: list[tuple] = []
    observed_bytes: dict[str, dict[str, bytes]] = {}

    for step in walk["steps"]:
        if only and step["case"] != only:
            continue
        case = cases[step["case"]]
        report.append("")
        report.append(f"== step {step['ordinal']}: {step['title']} ==")
        report.append(f"  section 10 item(s): {step['project_item']}")
        report.append(f"  {step['narrative']}")
        before = len(failures)
        if case["kind"] == "accept":
            metrics = check_accept(url, plan_dir, program_id, case, report, failures)
        else:
            metrics = check_refuse(url, plan_dir, program_id, case, report, failures)
        observed_bytes[case["name"]] = metrics["observed"]
        if len(failures) != before:
            metrics["verdict"] = "DIVERGED"
        rows.append(
            (
                step["ordinal"],
                step["case"],
                metrics["units"],
                metrics["per_instruction"],
                metrics["bytes"],
                metrics["written"],
                metrics["verdict"],
            )
        )

    if not only:
        check_terminal(walk["terminal"], observed_bytes, report, failures)

    print("\n".join(report))

    print("")
    print("== the walk, step by step ==")
    print(
        f"  {'#':>2}  {'step':<28} {'CU':>10}  {'tx bytes':>8}  "
        f"{'written':>7}  differential"
    )
    for ordinal, name, units, per, size, written, verdict in rows:
        cu = "-" if units is None else f"{units}"
        if len(per) > 1:
            cu = f"{units} {per}"
        print(f"  {ordinal:>2}  {name:<28} {cu:>10}  {size:>8}  {written:>7}  {verdict}")

    print("")
    print("== section 10 items this walk does not drive ==")
    for skip in walk["skipped"]:
        print(f"  item {skip['project_item']}: {skip['title']}")
        for line in wrap(skip["reason"], 4):
            print(line)

    print("")
    print("== what this walk is and is not ==")
    for note in walk["notes"]:
        for line in wrap(note, 2):
            print(line)

    if failures:
        print("")
        print("FAIL")
        for failure in failures:
            print(f"  {failure}")
        print("")
        print(
            "The walk is one gate: a single diverging step fails the whole walk, "
            "because a lifecycle that closes nine tenths of the way closes nothing."
        )
        return 1
    print("")
    print("PASS")
    return 0


def wrap(text: str, indent: int, width: int = 76) -> list[str]:
    """Wrap one prose paragraph for the terminal, at a fixed indent."""
    pad = " " * indent
    lines: list[str] = []
    current = pad
    for word in text.split():
        if len(current) + len(word) + 1 > width and current.strip():
            lines.append(current)
            current = pad
        current += ("" if current == pad else " ") + word
    if current.strip():
        lines.append(current)
    return lines


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:18899")
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--only", default=None, help="run one case by name")
    parser.add_argument(
        "--lifecycle",
        action="store_true",
        help="run the PROJECT.md section-10 walk as one gate instead of the per-family plan",
    )
    args = parser.parse_args()

    plan = json.loads((args.plan / "plan.json").read_text())
    if args.lifecycle:
        return run_lifecycle(args.url, args.plan, plan, args.only)
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
                check_refuse(
                    args.url, args.plan, plan["program_id"], case, report, failures
                )

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
