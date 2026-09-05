#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Write a cohort's `sim-config.json` from the founding's OWN records.

The `admissions` row runs `simulator run --config $HERE/sim-config.json`, and
until this file existed NO row produced that config: every cohort hand-wrote
one in its job directory, and cohort-16.1 stopped dead at
`admission message compilation: PacketTooLarge` because the config it never had
is the only thing that supplies the routing address lookup table the admission
message needs to fit a packet. That is the producer-missing pattern, and this
is the producer.

NOTHING HERE IS TRANSCRIBED. Every address is read out of
`market/campaign-open.json`'s accounts map, and the ROUTING TABLE is derived
the one way the tree already trusts:
`simlife_drivers.frozen_routing_table_for` reads the founding's own
`create DCLTGMF3 frozen routing address lookup table` transaction out of the
evidence, asks `getTransaction` which table that `CreateLookupTable` made, and
then AUTHENTICATES the account -- owned by the Address Lookup Table program,
authority `None` (frozen, so its extension plan is complete), and routing this
founding's own market. A hand-carried list of six recovered addresses cannot be
authenticated and is exactly what this refuses to accept.

The endpoint credential is never written. `cluster.rpc_url` is stripped of every
credential parameter before it is stored, and the simulator resolves the key at
USE time from `~/.helius-key`; cohort-15's config carried a live key in
cleartext and the loader now refuses a config that does.

usage: build-sim-config.py --job <dir> --rpc-url <url>
                           [--participant <name>]...
                           [--fill-gross-atoms N --direct-fee-basis-points BPS]
                           [--output <path>]
"""
import argparse
import hashlib
import json
import os
import subprocess
import sys
import urllib.parse
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "load-simulator"))

import simlife_drivers  # noqa: E402

CREDENTIAL_PARAMETERS = ("api-key", "api_key", "apikey", "access-token", "token")
DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"


def credential_free(url: str) -> str:
    parts = urllib.parse.urlsplit(url)
    query = [
        (name, value)
        for name, value in urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
        if name.lower() not in CREDENTIAL_PARAMETERS
    ]
    return urllib.parse.urlunsplit(
        (parts.scheme, parts.netloc, parts.path or "/", urllib.parse.urlencode(query), "")
    )


def pubkey(path: Path) -> str:
    return subprocess.run(
        ["solana-keygen", "pubkey", str(path)], capture_output=True, text=True, check=True
    ).stdout.strip()


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def finalized_slot(url: str) -> int:
    return int(simlife_drivers.rpc(url, "getSlot", [{"commitment": "finalized"}]))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--job", required=True, type=Path)
    ap.add_argument("--rpc-url", required=True)
    ap.add_argument("--work-dir", default=None)
    ap.add_argument("--participant", action="append", default=None)
    # The buyer's delegated allowance is DERIVED, never stated. It must EQUAL
    # the trade's `required_buyer_collateral` exactly -- the allowance
    # authorizes one trade and is spent to zero, so more is as refused as less
    # -- and that number is `gross + buyer_fee`, both of which the manifest's
    # economics already carry. Restating 201 in a third place is how the two
    # would drift.
    ap.add_argument("--fill-gross-atoms", type=int, default=0)
    ap.add_argument("--direct-fee-basis-points", type=int, default=0)
    ap.add_argument("--claim-unit-atoms", type=int, default=1)
    ap.add_argument("--output", type=Path, default=None)
    args = ap.parse_args()

    job = args.job.resolve()
    gross = args.fill_gross_atoms
    required_buyer_collateral = gross + (gross * args.direct_fee_basis_points) // 10_000
    participants = args.participant or ["participant-1", "participant-2"]
    evidence_path = job / "market" / "campaign-open.json"
    evidence = json.loads(evidence_path.read_text())
    accounts = evidence["execution"]["market"]["accounts"]

    def address(label: str) -> str:
        return accounts[label]["address"]

    # The LIVE Core Market is the founding's `founding_market`, not `market`:
    # both are Core Market accounts and the OPEN one is this one (cohort-11),
    # and it is the one the frozen table routes.
    market_address = address("founding_market")

    # THE ROUTING TABLE, DERIVED. Not a flag, not a list, not a scan.
    table = simlife_drivers.frozen_routing_table_for(args.rpc_url, evidence, market_address)
    if table is None:
        raise SystemExit(
            f"{evidence_path} records no "
            f"'{simlife_drivers.FROZEN_ROUTING_TABLE_CREATE_LABEL_V1}' transaction; "
            "this founding predates the label and its table cannot be derived"
        )

    payer = pubkey(job / "keys" / "campaign-payer.json")
    slot = finalized_slot(args.rpc_url)

    admissions = []
    for name in participants:
        entry = {
            "name": name,
            # plan-seal.json, not plan.json: the admission authenticates the
            # campaign report's own `plan_sha256`, and every cohort from 14 on
            # is founded from the SEALED plan.
            "plan": str(job / "plan-seal.json"),
            "campaign_evidence": str(evidence_path),
            "position_owner": pubkey(job / "keys" / f"{name}.json"),
            "position_owner_keypair": str(job / "keys" / f"{name}.json"),
            # The owner signs READONLY and a fee payer is writable
            # unconditionally, so the two can never be the same key.
            "fee_payer": payer,
            "fee_payer_keypair": str(job / "keys" / "campaign-payer.json"),
            "minimum_finalized_slot": slot,
            "output": str(job / "sim" / "admissions" / f"{name}.json"),
            "collateral": None,
        }
        if name == participants[-1] and required_buyer_collateral:
            # The buyer's delegated allowance must EQUAL the trade's
            # `required_buyer_collateral` exactly: it authorizes one trade and
            # is spent to zero, so more is as refused as less.
            entry["collateral"] = {
                "source_owner": payer,
                "source_owner_keypair": str(job / "keys" / "campaign-payer.json"),
                "source_account": address("collateral_wallet"),
                "quantity_atoms": required_buyer_collateral,
            }
        admissions.append(entry)

    # THE CENSUS BINDINGS ARE COMPLETE FROM THE FIRST CENSUS. A law asked to
    # total over a set missing a member reports a conservation breach that is
    # really a naming gap: cohort-12's first census stopped on
    # `VIOLATED L1: tracked 999999799 atoms != Mint supply 1000000000` because
    # the delegated collateral account had no name here. Neither the Position
    # PDA nor the delegated account can be named before the admission that
    # creates them, so the landed admission reports are read back when present.
    census_tokens = {"founder_collateral_wallet": address("collateral_wallet")}
    census_positions = {"founder": address("founder_position")}
    for name in participants:
        report = job / "sim" / "admissions" / f"{name}.json"
        if not report.is_file():
            continue
        landed = json.loads(report.read_text())
        census_positions[name] = landed["intent"]["position"]
        delegated = landed.get("collateral")
        if delegated:
            census_tokens[f"{name}_delegated_collateral"] = delegated["intent"][
                "participantTokenAccount"
            ]

    driver = job / "bin" / "dclutch-local-successor-bootstrap"
    digest = job / "bin" / "dclutch-local-successor-bootstrap.sha256"
    if digest.is_file():
        stated = digest.read_text().split()[0]
        actual = sha256(driver)
        if stated != actual:
            raise SystemExit(
                f"{driver} hashes {actual} and {digest} states {stated}; refusing to "
                "write a config naming a driver that is not the staged one"
            )

    config = {
        "schema": "dclutch-load-simulator-config-v1",
        "cluster": {
            "label": "devnet",
            "rpc_url": credential_free(args.rpc_url),
            "devnet_genesis": DEVNET_GENESIS,
        },
        "bootstrap_bin": str(driver),
        # A work dir belongs to ONE config: the journal recomputes the plan
        # digest and refuses to resume when the config changed. A rebuild that
        # changes the config gets its own dir.
        "work_dir": args.work_dir or os.environ.get("SIM_WORK_DIR", str(job / "sim")),
        "market_address": market_address,
        "cadence": {"period_seconds": 20.0, "jitter_fraction": 0.25},
        "trade": {"mode": "none", "max_steps_per_session": 32, "step_pause_seconds": 5.0},
        "admissions": admissions,
        "census": {
            "mint": address("collateral_mint"),
            "payer": payer,
            "hoard": address("founding_hoard_vault"),
            "aggregate": address("claims_aggregate"),
            "claim_unit_atoms": args.claim_unit_atoms,
            "tokens": census_tokens,
            "positions": census_positions,
            "watch": {},
        },
        "routing_table": table,
        "budget": {"max_lamports_spent": 500000000},
    }

    out = args.output or (job / "sim-config.json")
    body = json.dumps(config, indent=2, sort_keys=True) + "\n"
    # The value test is on the CREDENTIAL SHAPE, not on the word: `census.tokens`
    # is a legitimate key and a substring scan for "token" refuses it. What may
    # never appear is a query assignment -- `api-key=...` -- anywhere in the file.
    for parameter in CREDENTIAL_PARAMETERS:
        if f"{parameter}=" in body:
            raise SystemExit(f"refusing to write a config carrying '{parameter}='")
    scratch = out.with_suffix(out.suffix + ".partial")
    scratch.write_text(body)
    os.chmod(scratch, 0o600)
    scratch.replace(out)
    print(f"wrote {out}")
    print(f"  market_address: {market_address}")
    print(f"  routing_table:  {table}   (derived from the founding's create transaction)")
    if required_buyer_collateral:
        print(f"  required_buyer_collateral: {required_buyer_collateral} atoms "
              f"= gross {gross} + floor({gross} x {args.direct_fee_basis_points} / 10000)")
    for key, value in config["census"].items():
        print(f"  census.{key}: {value}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
