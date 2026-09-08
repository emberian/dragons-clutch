#!/usr/bin/env python3
"""Reproduce the dated binding-claim comparison without promoting it to evidence."""

import json
import pathlib
import sys


def load(path: pathlib.Path):
    with path.open() as stream:
        return json.load(stream)


def matching_block(blocked, route):
    matches = []
    for entry in blocked:
        pattern = entry["route"]
        if pattern == route or (pattern.endswith("*") and route.startswith(pattern[:-1])):
            matches.append(entry)
    return max(matches, key=lambda entry: len(entry["route"].rstrip("*")), default=None)


UNRECORDED_OWNERS = {
    "accelerator/dealer::process": "Dealer accelerator program owner; gauntlet evidence owner",
    "accelerator/dealer::process_scoring_row_v1": (
        "Dealer accelerator program owner; gauntlet evidence owner"
    ),
    "resolution/provider_instruction_v3::process_provider_resolution_v3#count": (
        "Source/provider owner; gauntlet evidence owner"
    ),
}


def main():
    if len(sys.argv) != 5:
        raise SystemExit("usage: classify.py REPO INVENTORY LEDGER OUT")
    repo, inventory_path, ledger_path, out_path = map(pathlib.Path, sys.argv[1:])
    inventory = load(inventory_path)
    ledger = load(ledger_path)
    substrates = {
        row["campaign"]: row["substrate"]
        for row in load(repo / "tools/gauntlet/substrates.json")["campaigns"]
    }
    route_ids = {
        route["id"] for program in inventory["programs"] for route in program["routes"]
    }
    accepted = {name: set() for name in ("devnet", "local-validator", "program-test")}
    refused = set()
    dealer_routes = set()

    for path in sorted((repo / "tools/gauntlet").glob("*/*bindings.json")):
        document = load(path)
        campaign = document.get("campaign", str(path.relative_to(repo)))
        substrate = substrates.get(campaign)
        if substrate not in accepted:
            continue
        for binding in document.get("bindings", []):
            bound = set(binding.get("routes", [])) & route_ids
            if binding.get("outcome") == "executed":
                accepted[substrate].update(bound)
                if campaign == "scoring-dealer-local-validator":
                    dealer_routes.update(bound)
            elif binding.get("outcome") == "refused":
                refused.update(bound)

    for path in sorted((repo / "docs/evidence/witnesses").glob("*.json")):
        for record in load(path).get("records", []):
            bound = set(record.get("routes_corroborated", [])) & route_ids
            if record.get("outcome") == "executed":
                accepted["devnet"].update(bound)
            elif record.get("outcome") == "refused":
                refused.update(bound)

    exact_local = {
        row["route"]
        for row in ledger["observations"]
        if row["outcome"] == "executed"
        and row["evidence_level"] == "finalized-instruction"
    }
    exact_agave = accepted["devnet"] | exact_local
    historical_success = set().union(*accepted.values())
    historical_success_before_dealer = historical_success - dealer_routes
    refusal_only = refused - historical_success
    no_success = route_ids - historical_success - refusal_only
    local_claim_only = accepted["local-validator"] - accepted["devnet"] - exact_local
    program_test_only = accepted["program-test"] - (
        accepted["devnet"] | accepted["local-validator"]
    )
    blocked = load(repo / "tools/gauntlet/blocked.json")["blocked"]

    rows = []
    for route in sorted(no_success):
        entry = matching_block(blocked, route)
        rows.append(
            {
                "route": route,
                "class": entry["class"] if entry else "unrecorded",
                "owner": entry["owner"] if entry else UNRECORDED_OWNERS.get(route),
            }
        )

    result = {
        "schema": "dclutch-execution-coverage-analysis-2026-09-08-v1",
        "source_revision": inventory.get("source_revision"),
        "warning": (
            "Historical binding claims are authored route labels. Only exact_agave_routes "
            "come from checked finalized native instruction evidence in this analysis."
        ),
        "totals": {
            "inventory_routes": len(route_ids),
            "inventory_refusals": sum(
                len(program["refusals"]) for program in inventory["programs"]
            ),
            "inventory_unclassified": sum(
                len(program["unclassified"]) for program in inventory["programs"]
            ),
            "exact_devnet_accepted_routes": len(accepted["devnet"]),
            "exact_local_validator_accepted_routes": len(exact_local),
            "exact_historical_agave_union_routes": len(exact_agave),
            "historical_success_claim_union_before_dealer": len(
                historical_success_before_dealer
            ),
            "historical_success_claim_union": len(historical_success),
            "historical_refusal_only_claims": len(refusal_only),
            "no_historical_success_claim": len(no_success),
            "local_validator_claim_only_no_exact_checked_evidence": len(local_claim_only),
            "program_test_only_success_claim": len(program_test_only),
        },
        "non_disjoint_success_claims": {
            key: len(value) for key, value in accepted.items()
        },
        "strongest_disjoint_success_claims": {
            "devnet": len(accepted["devnet"]),
            "local-validator": len(accepted["local-validator"] - accepted["devnet"]),
            "program-test": len(program_test_only),
        },
        "exact_local_validator_routes": sorted(exact_local),
        "exact_agave_routes": sorted(exact_agave),
        "local_validator_claim_only_routes": sorted(local_claim_only),
        "program_test_only_routes": sorted(program_test_only),
        "refusal_only_claim_routes": sorted(refusal_only),
        "no_successful_claim_routes": rows,
    }
    with out_path.open("w") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")


if __name__ == "__main__":
    main()
