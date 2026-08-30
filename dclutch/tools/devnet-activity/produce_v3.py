#!/usr/bin/env python3
"""Produce the canonical Activity-v3 scenario and manifest without keys or RPC.

Economic values are projected from the checked lifecycle fixture and the exact
flagship operation ensemble.  The bindings file supplies only deployment and
caller artifacts; it cannot redefine wallet funding or expected mutations.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[2]
ACTIVITY_PATH = Path(__file__).with_name("activity.py")
LEDGER_PATH = ROOT / "tools" / "economic-lifecycle-ledger" / "ledger.py"
CANONICAL_ECONOMIC_FIXTURE = (
    ROOT / "tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json"
)
CANONICAL_BASE_SCENARIO = ROOT / "tools/devnet-scenarios/fixtures/flagship.json"
BINDINGS_SCHEMA = "dclutch-devnet-activity-v3-producer-bindings-v1"
SCENARIO_ID = "activity-v3-canonical-four-outcome"
CALLER_SCHEMA_BY_KIND = {
    "found": "dclutch-successor-campaign-report-v1",
    "participant": "dclutch-devnet-user-position-admission-execution-v1",
    "direct": "dclutch-devnet-direct-trade-finalized-v1",
    "resolve": "dclutch-devnet-terminal-sequence-completion-v1",
    "redeem": "dclutch-devnet-terminal-sequence-completion-v1",
    "retire": "dclutch-devnet-terminal-sequence-completion-v1",
}


class Refusal(RuntimeError):
    pass


def load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise Refusal(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_body_sha256(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


def read_json(path: Path, label: str) -> Any:
    def pairs(rows: list[tuple[str, Any]]) -> dict[str, Any]:
        output: dict[str, Any] = {}
        for key, value in rows:
            if key in output:
                raise Refusal(f"{label} repeats JSON key {key!r}")
            output[key] = value
        return output

    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=pairs)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise Refusal(f"{label} is not exact JSON: {error}") from error


def exact_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise Refusal(f"{label} must be one JSON object")
    return value


def exact_keys(value: Mapping[str, Any], keys: set[str], label: str) -> None:
    if set(value) != keys:
        raise Refusal(
            f"{label} has missing {sorted(keys - set(value))} or unknown {sorted(set(value) - keys)} fields"
        )


def accepted_file(path_value: str, digest_value: str, label: str) -> Path:
    path = Path(path_value)
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise Refusal(f"{label} must be one existing absolute non-symlink file")
    if sha256_file(path) != digest_value:
        raise Refusal(f"{label} changed from its accepted SHA-256")
    return path.resolve(strict=True)


def canonical_scenario(base: Any, economic_fixture: Any, ledger: Any) -> dict[str, Any]:
    fixture = exact_object(economic_fixture, "economic fixture")
    evaluated = ledger.derive_fixture(fixture)
    authority = exact_object(evaluated.get("activityV3Authority"), "Activity-v3 authority")
    envelope = exact_object(base, "base scenario")
    if (
        envelope.get("schema") != "dclutch-devnet-economic-scenario-v1"
        or envelope.get("scenarioId") != "flagship-four-outcome"
    ):
        raise Refusal("base scenario is not the flagship economic operation ensemble")
    body = copy.deepcopy(exact_object(envelope.get("body"), "base scenario body"))
    if envelope.get("bodySha256") != canonical_body_sha256(body):
        raise Refusal("base scenario compact body digest changed")
    operations = body.get("operations")
    if not isinstance(operations, list) or len(operations) != 25:
        raise Refusal("flagship operation ensemble is not exactly 25 operations")

    base_wallets = {
        row.get("id"): copy.deepcopy(row)
        for row in body.get("wallets", [])
        if isinstance(row, dict)
    }
    wallets: list[dict[str, Any]] = []
    fresh_roles = {
        "collateral-mint",
        "collateral-wallet",
        "founding-beneficiary",
        "founding-projection-witness",
        "founding-source-funder",
    }
    for expected in authority["wallets"]:
        wallet_id = expected["id"]
        role = expected["role"]
        if wallet_id in base_wallets:
            row = base_wallets[wallet_id]
        elif role in fresh_roles:
            row = {
                "id": wallet_id,
                "roles": [role],
                "fundingLamports": "0",
                "collateralAccountRef": "token.flagship-four-outcome.deployer.collateral",
                "claimAccountRefs": [],
                "positionAccountRef": None,
            }
        else:
            raise Refusal(f"economic authority wallet {wallet_id} has no canonical projection")
        projected_role = "participant" if role.startswith("participant-") else role
        row["roles"] = list(dict.fromkeys([projected_role, *row["roles"]]))
        row["fundingLamports"] = (
            expected["initialFundingLamports"]
            if wallet_id == authority["payerWallet"]
            else expected["postInitFundingLamports"]
        )
        wallets.append(row)

    existing_accounts = {
        row.get("id")
        for row in body.get("accounts", [])
        if isinstance(row, dict)
    }
    accounts = copy.deepcopy(body["accounts"])
    for expected in authority["wallets"]:
        account_id = f"wallet.{expected['id']}"
        if account_id not in existing_accounts:
            accounts.append(
                {
                    "id": account_id,
                    "kind": "wallet",
                    "address": None,
                    "expectedOwnerRef": "solana-system-program",
                    "mintRef": None,
                    "tokenAuthorityWalletRef": None,
                }
            )

    for operation in operations:
        kind = operation.get("kind")
        if kind not in CALLER_SCHEMA_BY_KIND:
            raise Refusal("flagship operation has another lifecycle kind")
        operation["callerTarget"] = (
            "tools/local-validator/bootstrap/successor/"
            + ("campaign" if kind == "found" else "devnet-activity-v3")
        )
        operation["callerSchema"] = CALLER_SCHEMA_BY_KIND[kind]
        operation["callerAvailability"] = "public-executable"
        operation["mutationExpected"] = True
        operation["expectedObservedDelta"] = copy.deepcopy(
            operation["projectedAcceptedDelta"]
        )

    body.update(
        {
            "scenarioId": SCENARIO_ID,
            "title": "Canonical Activity-v3 four-outcome lifecycle",
            "description": (
                "Authenticated devnet Activity-v3 projection of the exact flagship "
                "economic operation ensemble and corrected ten-wallet funding envelope."
            ),
            "evidenceLevel": "authenticated-activity-v3",
            "wallets": wallets,
            "accounts": accounts,
            "operations": operations,
        }
    )
    scenario = {
        "schema": "dclutch-devnet-economic-scenario-v1",
        "version": 1,
        "scenarioId": SCENARIO_ID,
        "bodyDigestScope": "canonical-compact-scenario-body-json-v1",
        "bodySha256": canonical_body_sha256(body),
        "body": body,
    }
    ledger.authenticate_activity_v3_scenario(
        scenario, fixture["activityV3Authority"]
    )
    return scenario


def canonical_manifest(
    scenario_path: Path,
    scenario: Mapping[str, Any],
    economic_fixture: Mapping[str, Any],
    bindings_value: Any,
    ledger: Any,
) -> dict[str, Any]:
    bindings = exact_object(bindings_value, "producer bindings")
    exact_keys(
        bindings,
        {
            "schema",
            "target",
            "inputs",
            "addressBindings",
            "adapters",
            "campaignIdentities",
            "permanentAuthorityRef",
            "foundingAdapter",
        },
        "producer bindings",
    )
    if bindings["schema"] != BINDINGS_SCHEMA:
        raise Refusal("producer bindings schema changed")
    authority = ledger.authenticate_activity_v3_authority(
        economic_fixture["activityV3Authority"]
    )
    if authority is None:
        raise Refusal("economic fixture omitted Activity-v3 authority")
    participant_rows = [
        row for row in authority["wallets"] if row["role"].startswith("participant-")
    ]
    manifest = {
        "schema": "dclutch-devnet-activity-manifest-v3",
        "scenario": {
            "path": str(scenario_path),
            "sha256": sha256_file(scenario_path),
        },
        "target": copy.deepcopy(bindings["target"]),
        "inputs": copy.deepcopy(bindings["inputs"]),
        "addressBindings": copy.deepcopy(bindings["addressBindings"]),
        "adapters": copy.deepcopy(bindings["adapters"]),
        "campaign": {
            "identities": copy.deepcopy(bindings["campaignIdentities"]),
            "permanentAuthorityRef": bindings["permanentAuthorityRef"],
            "foundingAdapter": bindings["foundingAdapter"],
            "initialFunding": {
                "walletRef": authority["payerWallet"],
                "transferLamports": authority["authorization"]["initialFundingLamports"],
            },
            "postInitFunding": [
                {
                    "id": f"fund-{row['id']}",
                    "walletRef": row["id"],
                    "transferLamports": row["postInitFundingLamports"],
                    "afterAdapter": bindings["foundingAdapter"],
                }
                for row in participant_rows
            ],
        },
    }
    if manifest["target"] != {
        "kind": "devnet",
        "rpcUrl": "https://api.devnet.solana.com:443/",
        "devnetGenesisHash": "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
    }:
        raise Refusal("canonical Activity-v3 target changed from exact public devnet")
    return manifest


def authenticate_canonical_manifest_shape(manifest: Any) -> None:
    operations = list(manifest.scenario.operations)
    founding = [row for row in manifest.adapters if row.argv[0] == "campaign"]
    participants = [
        row
        for row in manifest.adapters
        if row.argv[0] == "devnet-user-position-admission-v1"
    ]
    directs = [
        row for row in manifest.adapters if row.argv[0] == "devnet-direct-trade-v1"
    ]
    terminals = [
        row
        for row in manifest.adapters
        if row.argv[0] == "devnet-terminal-sequence-v1"
    ]
    if (
        len(manifest.adapters) != 10
        or len(founding) != 1
        or len(participants) != 4
        or len(directs) != 4
        or len(terminals) != 1
    ):
        raise Refusal(
            "canonical Activity-v3 manifest must be one founding, four participant, four Direct, and one terminal adapter"
        )
    expected_found = [row.operation_id for row in operations if row.kind == "found"]
    expected_participants = [
        row.operation_id for row in operations if row.kind == "participant"
    ]
    expected_directs = [row.operation_id for row in operations if row.kind == "direct"]
    expected_terminal = [
        row.operation_id
        for row in operations
        if row.kind in {"resolve", "redeem", "retire"}
    ]
    if list(founding[0].covers) != expected_found:
        raise Refusal("canonical Activity-v3 founding coverage changed")
    if [list(row.covers) for row in participants] != [
        [operation_id] for operation_id in expected_participants
    ]:
        raise Refusal("canonical Activity-v3 participant coverage changed")
    if [list(row.covers) for row in directs] != [
        [operation_id] for operation_id in expected_directs
    ]:
        raise Refusal("canonical Activity-v3 Direct coverage changed")
    if list(terminals[0].covers) != expected_terminal:
        raise Refusal("canonical Activity-v3 terminal coverage changed")
    for row in [*directs, *terminals]:
        if row.progressive is None or not row.mutation:
            raise Refusal("canonical Activity-v3 progressive adapter contract is absent")
        expected_schema = (
            "dclutch-devnet-direct-trade-finalized-v1"
            if row.argv[0] == "devnet-direct-trade-v1"
            else "dclutch-devnet-terminal-sequence-completion-v1"
        )
        expected_list = "/mutations" if row in directs else "/journals"
        expected_label = "/kind" if row in directs else "/mutation/kind"
        completion = row.completion
        if (
            completion.schema != expected_schema
            or completion.transaction_list_pointer != expected_list
            or completion.transaction_label_pointer != expected_label
            or completion.transaction_signature_pointer != "/signature"
            or not completion.require_all_transactions_successful
        ):
            raise Refusal(
                "canonical Activity-v3 progressive completion list changed"
            )


def write_new_json(path: Path, value: Mapping[str, Any], label: str) -> None:
    if not path.is_absolute() or path.is_symlink() or path.exists():
        raise Refusal(f"{label} must be one absent absolute non-symlink path")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8") as output:
        # Scenario body digests intentionally preserve the field order owned by
        # the canonical envelope; sorting would change those authenticated bytes.
        json.dump(value, output, sort_keys=False, indent=2, ensure_ascii=True)
        output.write("\n")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--economic-fixture", required=True)
    result.add_argument("--economic-fixture-sha256", required=True)
    result.add_argument("--base-scenario", required=True)
    result.add_argument("--base-scenario-sha256", required=True)
    result.add_argument("--bindings", required=True)
    result.add_argument("--scenario-out", required=True)
    result.add_argument("--manifest-out", required=True)
    return result


def produce(arguments: argparse.Namespace) -> None:
    fixture_path = accepted_file(
        arguments.economic_fixture,
        arguments.economic_fixture_sha256,
        "economic fixture",
    )
    base_path = accepted_file(
        arguments.base_scenario,
        arguments.base_scenario_sha256,
        "base scenario",
    )
    if fixture_path != CANONICAL_ECONOMIC_FIXTURE.resolve(strict=True):
        raise Refusal("economic fixture is not the repository Activity-v3 authority")
    if base_path != CANONICAL_BASE_SCENARIO.resolve(strict=True):
        raise Refusal("base scenario is not the repository flagship fixture")
    bindings_path = Path(arguments.bindings)
    if not bindings_path.is_absolute() or bindings_path.is_symlink() or not bindings_path.is_file():
        raise Refusal("bindings must be one existing absolute non-symlink file")
    scenario_out = Path(arguments.scenario_out)
    manifest_out = Path(arguments.manifest_out)
    if scenario_out == manifest_out:
        raise Refusal("scenario and manifest outputs must be distinct")
    ledger = load_module("dclutch_economic_lifecycle_ledger", LEDGER_PATH)
    activity = load_module("dclutch_devnet_activity_producer_check", ACTIVITY_PATH)
    fixture = exact_object(read_json(fixture_path, "economic fixture"), "economic fixture")
    scenario = canonical_scenario(
        read_json(base_path, "base scenario"), fixture, ledger
    )
    write_new_json(scenario_out, scenario, "scenario output")
    manifest = canonical_manifest(
        scenario_out,
        scenario,
        fixture,
        read_json(bindings_path, "producer bindings"),
        ledger,
    )
    write_new_json(manifest_out, manifest, "manifest output")
    parsed = activity.parse_manifest(manifest_out)
    authenticate_canonical_manifest_shape(parsed)


def main(argv: Sequence[str] | None = None) -> int:
    try:
        produce(parser().parse_args(argv))
        return 0
    except Exception as error:
        if not isinstance(error, Refusal) and error.__class__.__name__ != "Refusal":
            raise
        print(f"Activity-v3 producer refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
