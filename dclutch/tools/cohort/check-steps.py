#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Gate THE cohort runbook, for one cohort named by its manifest.

There is one `steps.tsv` and one manifest per cohort. A new cohort is a file in
`cohorts/` and nothing else -- no forked table, no forked checker, because a
forked checker is a checker that stops being run.

What it checks, which is cohort-14's three plus what a parameterized table adds:

  1. Every selected row appears in `README.md` under its KEY, and every key the
     README claims exists in the table. A step described in one and not the
     other is a step nobody owns.
  2. Every row NAMES A VERIFIER, and the verifier is not "it succeeded". A step
     whose check is its own exit code is the failure this repository calls
     silent success, and cohort-13 met it three times in one day.
  3. Every `blocks` edge names a real key that comes later in the selection, so
     "what cannot start until this is green" is answerable. An edge pointing at
     a row this cohort replaced follows the replacement forward rather than
     dangling.
  4. Every `{field}` in a selected row RESOLVES against the manifest. An
     unresolved field is refused rather than rendered as itself: a row that
     silently keeps its own placeholder is a row with no author.
  5. `replaces` and `until` agree -- a replaced row's `until` is the
     replacement's `since` minus one -- so the two ways of retiring a row
     cannot drift apart.

`--prove-frozen` is the migration's own gate: the cohort-14 view must reproduce
`tools/cohort14/steps.tsv` and the cohort-15 delta view
`tools/cohort15/steps.tsv`, byte for byte in their six-column form. Those two
files stay until the live cohort-15 lane closes, and until then this is what
says the union lost nothing.

It reads files and exits. No cargo, no chain, no keys.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
STEPS = HERE / "steps.tsv"
README = HERE / "README.md"
COHORTS = HERE / "cohorts"

FIELDS = ("key", "stage", "since", "until", "replaces",
          "driver", "command", "verifier", "cost", "blocks")
LEGACY = ("id", "stage", "command", "verifier", "cost", "blocks")

# Phrases that are an exit code wearing a verifier's clothes.
HOLLOW = ("it succeeds", "it succeeded", "exit code", "exits zero",
          "no error", "reports success")

# The programs a row may name. A driver outside this vocabulary is a row the
# generator cannot emit, which is the same as a row nobody can run.
DRIVER_KINDS = ("bootstrap", "solana-cli", "script", "simulator", "commit")

PLACEHOLDER = re.compile(r"\{([a-z0-9_]+(?:[.-][a-z0-9_-]+)*)\}")


class Refusal(Exception):
    pass


def rows() -> list[dict[str, str]]:
    parsed = []
    for number, line in enumerate(STEPS.read_text().splitlines(), start=1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != len(FIELDS):
            raise Refusal(f"steps.tsv:{number}: {len(fields)} tab-separated fields, expected {len(FIELDS)}")
        parsed.append(dict(zip(FIELDS, fields)))
    return parsed


def manifest(name: str) -> dict:
    path = pathlib.Path(name)
    if not path.exists():
        path = COHORTS / f"{name}.json"
    if not path.exists():
        raise Refusal(f"no cohort manifest at {name} or {COHORTS / (name + '.json')}")
    document = json.loads(path.read_text())
    if document.get("schema") != "dclutch-cohort-manifest-v1":
        raise Refusal(f"{path} is not a dclutch-cohort-manifest-v1")
    return document


def lookup(document: dict, dotted: str):
    """Resolve `group.field`, and the two counts derived from `roles`.

    The word forms exist because the rows are prose an operator reads at 3am,
    and "seven solana program deploy" is what that sentence has always said.
    Deriving them from the role list is what keeps the sentence and the loop
    the generator emits from ever disagreeing.
    """
    WORDS = {1: "one", 2: "two", 3: "three", 4: "four", 5: "five",
             6: "six", 7: "seven", 8: "eight", 9: "nine", 10: "ten"}
    if dotted == "role_count":
        return str(len(document["roles"]))
    if dotted == "role_count_word":
        return WORDS[len(document["roles"])]
    if dotted == "owned_role_count":
        return str(len(document["owned_roles"]))
    node = document
    for part in dotted.split("."):
        if isinstance(node, dict) and part in node:
            node = node[part]
        elif isinstance(node, dict) and "economics" in document and part in document["economics"]:
            node = document["economics"][part]
        else:
            return None
    return node


def resolve(text: str, document: dict, where: str, problems: list[str]) -> str:
    def one(match):
        dotted = match.group(1)
        value = lookup(document, dotted)
        if value is None:
            value = lookup(document.get("economics", {}), dotted)
        if value is None:
            problems.append(f"{where}: the manifest has no field {dotted!r}")
            return match.group(0)
        if not isinstance(value, (str, int)):
            problems.append(f"{where}: manifest field {dotted!r} is a {type(value).__name__}, not a value a sentence can carry")
            return match.group(0)
        return str(value)
    return PLACEHOLDER.sub(one, text)


def select(table: list[dict], cohort: int, delta: bool) -> list[dict]:
    chosen = []
    for row in table:
        since = int(row["since"])
        until = None if row["until"] == "-" else int(row["until"])
        if delta:
            if since == cohort:
                chosen.append(row)
            continue
        if since <= cohort and (until is None or until >= cohort):
            chosen.append(row)
    return chosen


def render(table: list[dict], chosen: list[dict], document: dict,
           problems: list[str]) -> list[dict[str, str]]:
    """The six-column view: positional ids, blocks by id, fields resolved."""
    position = {row["key"]: index for index, row in enumerate(chosen)}
    replaced_by: dict[str, str] = {}
    for row in table:
        if row["replaces"] != "-":
            replaced_by[row["replaces"]] = row["key"]

    def forward(key: str) -> str | None:
        seen = set()
        while key not in position:
            if key in seen or key not in replaced_by:
                return None
            seen.add(key)
            key = replaced_by[key]
        return key

    out = []
    for index, row in enumerate(chosen):
        where = f"step {row['key']}"
        blocks = []
        if row["blocks"] != "-":
            for raw in row["blocks"].split(","):
                target = forward(raw.strip())
                if target is None:
                    problems.append(f"{where} blocks {raw.strip()!r}, which this cohort neither runs nor replaces")
                    continue
                if position[target] <= index:
                    problems.append(f"{where} blocks {target}, which does not come after it")
                blocks.append(f"{position[target]:02d}")
        out.append({
            "id": f"{index:02d}",
            "stage": resolve(row["stage"], document, where, problems),
            "command": resolve(row["command"], document, where, problems),
            "verifier": resolve(row["verifier"], document, where, problems),
            "cost": row["cost"],
            "blocks": ",".join(blocks) if blocks else "-",
        })
    return out


def structural(table: list[dict], problems: list[str]) -> None:
    keys = [row["key"] for row in table]
    if len(set(keys)) != len(keys):
        problems.append("steps.tsv has duplicate keys")
    known = set(keys)
    for row in table:
        if row["replaces"] != "-":
            if row["replaces"] not in known:
                problems.append(f"{row['key']} replaces {row['replaces']!r}, which is not a row")
                continue
            replaced = next(r for r in table if r["key"] == row["replaces"])
            if replaced["until"] == "-":
                problems.append(f"{row['key']} replaces {replaced['key']}, which has no `until`; a replaced row does not stay current")
            elif int(replaced["until"]) != int(row["since"]) - 1:
                problems.append(
                    f"{row['key']} arrives at {row['since']} but {replaced['key']} runs until "
                    f"{replaced['until']}; a cohort would run both or neither")
        kind = row["driver"].split(":", 1)[0]
        if row["driver"] != "-" and kind not in DRIVER_KINDS:
            problems.append(f"{row['key']} names driver kind {kind!r}, which the generator cannot emit")


def verifiers(view: list[dict], problems: list[str]) -> None:
    for step in view:
        verifier = step["verifier"].strip()
        if len(verifier) < 20:
            problems.append(f"step {step['id']} names no real verifier: {verifier!r}")
        lowered = verifier.lower()
        for hollow in HOLLOW:
            if hollow in lowered:
                problems.append(f"step {step['id']}'s verifier is an exit code in disguise: {verifier!r}")


def documented(chosen: list[dict], problems: list[str]) -> None:
    text = README.read_text()
    headings = set(re.findall(r"^### ([a-z0-9-]+)$", text, re.MULTILINE))
    for row in chosen:
        if row["key"] not in headings:
            problems.append(f"row {row['key']} has no README heading")
    table_keys = {row["key"] for row in rows()}
    for extra in sorted(headings - table_keys):
        problems.append(f"README documents {extra}, which steps.tsv does not have")


def legacy_text(view: list[dict]) -> str:
    return "".join("\t".join(step[field] for field in LEGACY) + "\n" for step in view)


def frozen_text(path: pathlib.Path) -> str:
    keep = [line for line in path.read_text().splitlines()
            if line.strip() and not line.startswith("#")]
    return "".join(line + "\n" for line in keep)


def prove_frozen(frozen_root: pathlib.Path) -> int:
    table = rows()
    structural(table, problems := [])
    checks = [
        ("14", False, frozen_root / "tools/cohort14/steps.tsv"),
        ("15", True, frozen_root / "tools/cohort15/steps.tsv"),
    ]
    failures = 0
    for name, delta, path in checks:
        # A missing frozen file is a refusal with a sentence, never a traceback.
        # This runs from copies -- that is how the red proofs work -- so the
        # frozen runbooks are an EXTERNAL input to the proof and are named as
        # one rather than guessed from this file's own grandparent.
        if not path.exists():
            print(f"  no frozen runbook at {path}; pass --frozen-root")
            failures += 1
            continue
        document = manifest(name)
        view = render(table, select(table, int(name), delta), document, problems)
        ours, theirs = legacy_text(view), frozen_text(path)
        label = f"cohort-{name}{' delta' if delta else ''}"
        if ours == theirs:
            print(f"  {label}: {len(view)} rows reproduce {path} exactly")
            continue
        failures += 1
        print(f"  {label}: DOES NOT reproduce {path}")
        import difflib
        for line in list(difflib.unified_diff(theirs.splitlines(), ours.splitlines(),
                                              "frozen", "unified", lineterm=""))[:40]:
            print(f"    {line[:200]}")
    for problem in problems:
        print(f"  {problem}")
        failures += 1
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cohort", help="a cohort number or a manifest path")
    parser.add_argument("--delta", action="store_true",
                        help="only the rows this cohort is the first to run")
    parser.add_argument("--emit-legacy", action="store_true",
                        help="print the six-column form the frozen runbooks use")
    parser.add_argument("--prove-frozen", action="store_true",
                        help="prove the union reproduces both frozen runbooks")
    parser.add_argument("--frozen-root", default=str(REPO),
                        help="the tree holding tools/cohort14 and tools/cohort15")
    arguments = parser.parse_args()

    try:
        if arguments.prove_frozen:
            print("cohort runbook: reproducing the frozen tables")
            return prove_frozen(pathlib.Path(arguments.frozen_root))
        if not arguments.cohort:
            parser.error("--cohort is required (a number, or a path to a manifest)")
        document = manifest(arguments.cohort)
        table = rows()
        problems: list[str] = []
        structural(table, problems)
        chosen = select(table, int(document["cohort"]), arguments.delta)
        if not chosen:
            problems.append(f"cohort {document['cohort']} selects no rows at all")
        view = render(table, chosen, document, problems)
        verifiers(view, problems)
        if not arguments.delta:
            documented(chosen, problems)
    except Refusal as refusal:
        print(f"cohort runbook: {refusal}", file=sys.stderr)
        return 2

    if arguments.emit_legacy:
        sys.stdout.write(legacy_text(view))

    label = f"cohort-{document['cohort']}{' delta' if arguments.delta else ''}"
    if problems:
        print(f"{label} runbook: the table, the README and the manifest disagree.")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print(f"{label} runbook: {len(view)} steps, each documented, each naming a "
          f"verifier, every field resolved from {document['schema']}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
