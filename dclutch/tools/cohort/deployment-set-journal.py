#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Author a cohort's deployment-set journal skeleton.

The journal is a DECLARATION -- which roles, which dispositions, which gate --
and every claim in it is then re-authenticated against the chain by
`devnet-deployment-set-already-current-v1`, by `devnet-deployment-set-journal-v2`
and by `prepare --deployment-set-journal`. Nothing here asserts a fact the
auditor will not re-read, which is why a skeleton is allowed to be authored at
all.

WHY IT LIVES HERE. Cohort-14 and cohort-15 both built this file with a
hand-written python script inside their job directories, and the `seal` row of
`steps.tsv` was written as `devnet-deployment-set-journal-v2 --init ...` -- a
mode no driver implements. Cohort-16 ran the row and it refused with "unknown
devnet-deployment-set-journal-v2 argument: --init", which is the runbook's own
producer-missing shape: the row named a producer that did not exist because the
producer that DID exist had no home in the tree. This is that producer, with the
job directory's absolute paths taken as arguments instead of typed in.

Registry and Rent are CarryForward: no baseline, and their dump is the built ELF
itself. The other five start as Upgrade rows with a baseline, and
`devnet-deployment-set-already-current-v1` converts each to AlreadyCurrent on
byte equality alone -- which is what a genesis cohort is, since it deployed the
checked candidate directly and has no upgrade to perform.

The ProgramData address of each role is read off the chain the key-free way, out
of the Program account's own bytes 4..36, for the same reason the emitted stage
scripts read it there: `solana program show` demands a default signer to perform
a read, and a job directory deliberately holds no key.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
import urllib.request

CARRY_FORWARD_ROLES = ("registry", "rent")
ROLE_ORDER = ("registry", "rent", "custody", "resolution", "claims", "trading", "core")
# `steps.tsv` spells `rent` as `rent-credit` wherever a flag names it, because
# that is the flag the driver takes. The journal's own `role` field is `rent`.
ROLE_FLAG = {"rent": "rent-credit"}
BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def refuse(message: str) -> None:
    raise SystemExit(f"deployment-set-journal: {message}")


def sha256_file(path: str) -> str | None:
    if not os.path.isfile(path):
        return None
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def base58_encode(raw: bytes) -> str:
    number = int.from_bytes(raw, "big")
    out = ""
    while number:
        number, remainder = divmod(number, 58)
        out = BASE58[remainder] + out
    for byte in raw:
        if byte:
            break
        out = "1" + out
    return out


def programdata_of(rpc_url: str, program: str) -> str:
    """The ProgramData address a Loader-v3 Program account names at offset 4."""
    request = urllib.request.Request(
        rpc_url,
        data=json.dumps(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getAccountInfo",
                "params": [
                    program,
                    {
                        "encoding": "base64",
                        "commitment": "finalized",
                        "dataSlice": {"offset": 4, "length": 32},
                    },
                ],
            }
        ).encode(),
        headers={"content-type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=30) as answer:
        value = json.load(answer).get("result", {}).get("value")
    if not value:
        refuse(f"no finalized account at {program}")
    raw = base64.b64decode(value["data"][0])
    if len(raw) != 32:
        refuse(f"{program} is not a 36-byte Loader-v3 Program account")
    return base58_encode(raw)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--output", required=True)
    parser.add_argument("--checked-release-gate", required=True)
    parser.add_argument("--solana-cli", required=True)
    parser.add_argument("--retained-upgrade-authority", required=True)
    parser.add_argument("--fee-payer", required=True)
    parser.add_argument("--carry-forward", required=True)
    parser.add_argument("--elf-dir", required=True)
    parser.add_argument("--baseline-dir", required=True)
    parser.add_argument("--receipt-dir", required=True)
    parser.add_argument("--dump-dir", required=True)
    parser.add_argument("--devnet-genesis", required=True)
    parser.add_argument("--rpc-url", default=os.environ.get("DCLUTCH_RPC_URL", ""))
    for role in ROLE_ORDER:
        parser.add_argument(f"--{ROLE_FLAG.get(role, role)}-program-id", required=True)
    arguments = parser.parse_args()

    if not arguments.rpc_url:
        refuse("--rpc-url or DCLUTCH_RPC_URL is required to read each Program account")
    for path in (arguments.checked_release_gate, arguments.carry_forward):
        if not os.path.isfile(path):
            refuse(f"{path} does not exist; the seal's earlier invocations write it")

    cli_version = subprocess.run(
        [arguments.solana_cli, "--version"], capture_output=True, text=True, check=True
    ).stdout.strip()
    gate = json.loads(open(arguments.checked_release_gate).read())
    os.makedirs(arguments.receipt_dir, exist_ok=True)
    os.makedirs(arguments.dump_dir, exist_ok=True)

    roles = []
    for role in ROLE_ORDER:
        program = getattr(arguments, f"{ROLE_FLAG.get(role, role)}_program_id".replace("-", "_"))
        programdata = programdata_of(arguments.rpc_url, program)
        disposition = "carry-forward" if role in CARRY_FORWARD_ROLES else "upgrade"
        receipt = os.path.join(arguments.receipt_dir, f"{role}.json")
        dump = os.path.join(arguments.dump_dir, f"{role}.so")
        baseline = None
        if disposition == "upgrade":
            baseline_path = os.path.join(arguments.baseline_dir, f"{role}.baseline.json")
            if not os.path.isfile(baseline_path):
                refuse(f"{role} has no upgrade baseline at {baseline_path}")
            baseline = {"canonical_path": baseline_path, "sha256": sha256_file(baseline_path)}
        else:
            source = os.path.join(arguments.elf_dir, f"{role}.so")
            if not os.path.isfile(source):
                refuse(f"{role} has no built ELF at {source}")
            if not os.path.isfile(dump):
                with open(dump, "wb") as out, open(source, "rb") as src:
                    out.write(src.read())
        roles.append(
            {
                "role": role,
                "disposition": disposition,
                "program_id": program,
                "programdata_id": programdata,
                "baseline": baseline,
                "receipt": {"canonical_path": receipt, "sha256": None},
                "dump": {"canonical_path": dump, "sha256": sha256_file(dump)},
                "already_current": None,
            }
        )

    journal = {
        "schema": "dclutch-devnet-deployment-set-journal-v3",
        "checked_release_gate": {
            "canonical_path": arguments.checked_release_gate,
            "sha256": sha256_file(arguments.checked_release_gate),
        },
        "source_revision": gate["source_revision"],
        "source_tree_sha256": gate["source_tree_sha256"],
        "devnet_genesis_hash": arguments.devnet_genesis,
        "solana_cli_version": cli_version,
        "retained_upgrade_authority": arguments.retained_upgrade_authority,
        "fee_payer": arguments.fee_payer,
        "infrastructure_carry_forward": {
            "canonical_path": arguments.carry_forward,
            "sha256": sha256_file(arguments.carry_forward),
        },
        "roles": roles,
    }
    # Never redirect a generator into its canonical output: write beside it,
    # then replace atomically, so a failed run leaves the last accepted journal
    # byte-for-byte intact.
    temporary = arguments.output + ".partial"
    with open(temporary, "w") as handle:
        handle.write(json.dumps(journal, indent=1) + "\n")
    os.replace(temporary, arguments.output)
    print("wrote", arguments.output)
    print("  cli:", cli_version)
    print("  source_revision:", gate["source_revision"])
    for row in roles:
        print(
            f"  {row['role']:<11}{row['disposition']:<15}"
            f"baseline={'yes' if row['baseline'] else '-':<4} "
            f"dump={(row['dump']['sha256'] or '-')[:12]}  {row['program_id']}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
