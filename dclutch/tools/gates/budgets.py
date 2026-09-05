"""tools/gate budgets -- tools/gauntlet/CU_BUDGETS.json, well-formed and under the ceiling.

Refuses: a budget that is not `measured + tolerance`; a budget above Solana's
1,400,000 CU ceiling (the transaction has stopped fitting and no tolerance can
be written for it); an enforced entry whose scope is neither `transaction` nor
`stage`, or a stage entry with no index/name; an unenforced entry with no
`unenforced_reason`; a duplicated id; a campaign no bindings file or
substrates.json row names, so the budget could never be evaluated.

Evaluation against a campaign's evidence stays where the campaigns run it:
tools/gauntlet/tier1/check-witnesses.sh, the one evaluator every runner calls.
"""

from __future__ import annotations

import json
from pathlib import Path

from .common import EXIT_FAIL, EXIT_PASS, REPO, Prereq, note

GAUNTLET = REPO / "tools" / "gauntlet"
BUDGETS = GAUNTLET / "CU_BUDGETS.json"
SUBSTRATES = GAUNTLET / "substrates.json"
CEILING = 1_400_000  # Solana's per-transaction MAX_COMPUTE_UNIT_LIMIT


def known_campaigns() -> set[str]:
    names: set[str] = set()
    if SUBSTRATES.is_file():
        names |= {row["campaign"] for row in json.loads(SUBSTRATES.read_text()).get("campaigns", [])}
    for bindings in sorted(GAUNTLET.glob("*/bindings.json")) + sorted(GAUNTLET.glob("*/*-bindings.json")):
        try:
            names.add(json.loads(bindings.read_text())["campaign"])
        except (OSError, ValueError, KeyError):
            continue
    return names


def problems(document: dict, campaigns: set[str]) -> list[str]:
    found: list[str] = []
    if document.get("schema") != "dclutch-cu-budgets-v1":
        found.append(f"schema is {document.get('schema')!r}, not dclutch-cu-budgets-v1")
    ceiling = (document.get("ceiling") or {}).get("compute_units")
    if ceiling != CEILING:
        found.append(f"ceiling.compute_units is {ceiling}, not Solana's {CEILING}")
    seen: set[str] = set()
    for entry in document.get("budgets", []):
        ident = entry.get("id", "<no id>")
        if ident in seen:
            found.append(f"{ident}: duplicated id")
        seen.add(ident)
        if not entry.get("enforced"):
            if not entry.get("unenforced_reason"):
                found.append(f"{ident}: unenforced with no unenforced_reason")
            continue
        if entry.get("campaign") not in campaigns:
            found.append(f"{ident}: enforced for campaign {entry.get('campaign')!r}, which no bindings file and no substrates.json row names")
        measured, tolerance, budget = entry.get("measured"), entry.get("tolerance"), entry.get("budget")
        if not all(isinstance(v, int) for v in (measured, tolerance, budget)):
            found.append(f"{ident}: measured, tolerance and budget must be integers")
            continue
        if budget != measured + tolerance:
            found.append(f"{ident}: budget {budget} is not measured+tolerance ({measured}+{tolerance}={measured + tolerance})")
        if budget > CEILING:
            found.append(f"{ident}: budget {budget} is ABOVE the {CEILING} ceiling; the transaction has stopped fitting")
        scope = entry.get("scope")
        if scope == "stage":
            stage = entry.get("stage") or {}
            if not isinstance(stage.get("index"), int) or not stage.get("name"):
                found.append(f"{ident}: a stage budget needs stage.index and stage.name")
        elif scope != "transaction":
            found.append(f"{ident}: an enforced budget needs scope transaction or stage, not {scope!r}")
    return found


def check(*, dry_run: bool = False):
    if dry_run:
        note("$ tools/gate budgets")
        return EXIT_PASS, ""
    if not BUDGETS.is_file():
        raise Prereq("tools/gauntlet/CU_BUDGETS.json is absent")
    document = json.loads(BUDGETS.read_text())
    found = problems(document, known_campaigns())
    for line in found:
        note(line)
    if found:
        return EXIT_FAIL, f"{len(found)} budget row(s) malformed, over the ceiling, or naming an unknown campaign"
    note(f"{len(document.get('budgets', []))} budget rows: every enforced one is measured+tolerance and under {CEILING}")
    return EXIT_PASS, ""


def main(argv: list[str]) -> int:
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return EXIT_PASS
    code, detail = check()
    if detail:
        print(f"budgets: {detail}")
    return code
