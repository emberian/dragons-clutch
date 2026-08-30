#!/usr/bin/env python3
"""Build a load-simulator config from a HELD private-validator probe.

The probe is tools/release/private-validator-lifecycle/run.py run with
``--through participant --seeds 1 --hold-after-participant``.  At the hold it
writes ``runs/seed-01/participant-handoff.json`` and SIGSTOPs itself, leaving
the validator alive.  This adapter reads the handoff plus the founding and
participant evidence and emits one dclutch-load-simulator-config-v1 JSON.

It never reads key bytes; it only names paths the accepted drivers open
themselves.  Missing facts refuse with the exact field named rather than
guessing.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys


class Refusal(RuntimeError):
    pass


def need(mapping: dict, key: str, where: str):
    if key not in mapping:
        raise Refusal(f"{where} lacks required field {key!r}")
    return mapping[key]


def load(path: Path, what: str) -> dict:
    if not path.is_file():
        raise Refusal(f"{what} is absent: {path}")
    return json.loads(path.read_text())


def find_first(body, predicate, path=""):
    """Depth-first search for the first value satisfying predicate; returns
    (json-pointer-ish path, value) or None."""
    if predicate(body):
        return path, body
    if isinstance(body, dict):
        for key, value in body.items():
            hit = find_first(value, predicate, f"{path}/{key}")
            if hit:
                return hit
    elif isinstance(body, list):
        for index, value in enumerate(body):
            hit = find_first(value, predicate, f"{path}/{index}")
            if hit:
                return hit
    return None


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--probe-work", required=True, help="the probe --work dir")
    parser.add_argument("--sim-work", required=True, help="fresh simulator work dir (absolute)")
    parser.add_argument("--bootstrap-bin", default=None,
                        help="successor binary (default: probe host-target build)")
    parser.add_argument("--output", required=True, help="config JSON to write (absolute)")
    parser.add_argument("--period-seconds", type=float, default=8.0)
    parser.add_argument("--no-census", action="store_true",
                        help="omit the census block (NOT for real runs; the "
                             "reconciliation loop is part of the deliverable)")
    args = parser.parse_args(argv)

    probe = Path(args.probe_work)
    seed = probe / "runs" / "seed-01"
    handoff = load(seed / "participant-handoff.json", "participant handoff")
    if handoff.get("schema") != "dclutch-private-validator-participant-handoff-v1":
        raise Refusal(f"unexpected handoff schema {handoff.get('schema')!r}")

    rpc_url = need(handoff, "rpcUrl", "handoff")
    plan = need(handoff, "plan", "handoff")
    market_input = need(handoff, "marketInput", "handoff")
    founding_evidence = need(handoff, "foundingEvidence", "handoff")
    participant_evidence = need(handoff, "participantEvidence", "handoff")
    key_directory = need(handoff, "keyDirectory", "handoff")

    founding = load(Path(founding_evidence), "founding evidence")
    targets = need(founding, "founding_targets", "founding evidence")
    market_address = need(targets, "open_market", "founding_targets")
    checkpoint = need(founding, "foundingCheckpoint", "founding evidence")
    accounts = need(checkpoint, "accounts", "foundingCheckpoint")
    payer = need(founding, "payer", "founding evidence")

    def account_address(label: str) -> str:
        entry = need(accounts, label, "foundingCheckpoint.accounts")
        if isinstance(entry, dict):
            return need(entry, "address", f"accounts[{label}]")
        return str(entry)

    market = load(Path(market_input), "market input")
    participant = load(Path(participant_evidence), "participant evidence")

    # Census facts.  Mint and hoard come from founding; the aggregate and the
    # participant's token/position accounts come from the admission report.
    census = None
    if not args.no_census:
        aggregate_hit = find_first(
            participant,
            lambda v: isinstance(v, str) and len(v) in range(32, 45)
            and v not in (market_address,),
            "",
        )
        # The aggregate must be named explicitly, not guessed: look for a key
        # literally containing "aggregate" in either evidence document.
        def keyed(body, needle):
            found = {}
            def walk(node, path=""):
                if isinstance(node, dict):
                    for key, value in node.items():
                        if needle in key.lower() and isinstance(value, str):
                            found[f"{path}/{key}"] = value
                        elif needle in key.lower() and isinstance(value, dict) and "address" in value:
                            found[f"{path}/{key}"] = value["address"]
                        walk(value, f"{path}/{key}")
                elif isinstance(node, list):
                    for index, value in enumerate(node):
                        walk(value, f"{path}/{index}")
            walk(body)
            return found

        aggregates = {**keyed(founding, "aggregate"), **keyed(participant, "aggregate")}
        if len(set(aggregates.values())) != 1:
            raise Refusal(
                "could not resolve exactly one aggregate address from evidence; "
                f"candidates: {aggregates!r}; pass --no-census only for a smoke "
                "run and file the gap"
            )
        aggregate = next(iter(set(aggregates.values())))

        positions = keyed(participant, "position")
        tokens = keyed(participant, "token_account")
        claim_unit = None
        for source, key in ((market, "claim_unit_atoms"), (market, "local_participant_fixture_liquidity_atoms")):
            if key in source:
                claim_unit = int(source[key])
                break
        if claim_unit is None:
            raise Refusal("no claim unit quantity in market input")
        census = {
            "mint": account_address("collateral_mint"),
            "payer": payer,
            "hoard": account_address("founding_hoard_vault"),
            "aggregate": aggregate,
            "claim_unit_atoms": claim_unit,
            "tokens": {
                label.rsplit("/", 1)[-1] or f"t{i}": addr
                for i, (label, addr) in enumerate(sorted(tokens.items()))
            },
            "positions": {
                label.rsplit("/", 1)[-1] or f"p{i}": addr
                for i, (label, addr) in enumerate(sorted(positions.items()))
            },
            "watch": {},
        }

    boot = args.bootstrap_bin or str(probe / "host-target" / "release" / "dclutch-local-successor-bootstrap")
    if not Path(boot).is_file():
        raise Refusal(f"bootstrap binary absent: {boot}")

    config = {
        "schema": "dclutch-load-simulator-config-v1",
        "cluster": {"label": "local", "rpc_url": rpc_url},
        "bootstrap_bin": boot,
        "work_dir": args.sim_work,
        "market_address": market_address,
        "cadence": {"period_seconds": args.period_seconds, "jitter_fraction": 0.25},
        "trade": {
            "mode": "local",
            "max_steps_per_session": 32,
            "step_pause_seconds": 1.0,
            "local": {
                "plan": plan,
                "market_input": market_input,
                "campaign_report": founding_evidence,
                "participant_report": participant_evidence,
                "key_dir": key_directory,
            },
        },
        "census": census,
        "wallets": [],
        "admissions": [],
        "probe": {
            "work": str(probe),
            "validator_pid": handoff.get("validatorPid"),
            "note": "teardown: SIGCONT the (stopped) run.py supervisor; never kill the validator directly",
        },
    }
    out = Path(args.output)
    out.write_text(json.dumps(config, sort_keys=True, indent=2) + "\n")
    print(f"config written: {out}")
    print(f"market: {market_address}")
    print(f"rpc: {rpc_url}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Refusal as refusal:
        print(f"REFUSED: {refusal}", file=sys.stderr)
        sys.exit(2)
