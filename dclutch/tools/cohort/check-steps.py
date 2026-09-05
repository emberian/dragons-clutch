#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Gate THE cohort runbook, for one cohort named by its manifest.

There is one `steps.tsv` and one manifest per cohort. A new cohort is a file in
`cohorts/` and nothing else -- no forked table, no forked checker, because a
forked checker is a checker that stops being run.

What it checks:

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
     silently keeps its own placeholder is a row with no author. In `args` the
     generator's own placeholders (`{market.x}`, `{role}`, `{pubkey:..}`, ...)
     are checked for shape here and resolved at emission, per market.
  5. `replaces` and `until` agree -- a replaced row's `until` is the
     replacement's `since` minus one -- so the two ways of retiring a row
     cannot drift apart.
  6. Every row's `shape` is one the generator has, and every invocation in
     `args` starts with a driver the generator can emit. A `*` act is only
     legal under a shape that loops, and a looping shape needs one.

`--prove-frozen` is the migration's own gate: the cohort-14 view must reproduce
`frozen/cohort-14.tsv` and the cohort-15 delta view `frozen/cohort-15.tsv`, byte
for byte in their six-column form. Those two files are the tables cohort-14 and
cohort-15 actually ran from, kept as fixtures so that adding a column or a
since-16 row can be proved to have changed nothing about what already ran.

It reads files and exits. No cargo, no chain, no keys.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
STEPS = HERE / "steps.tsv"
README = HERE / "README.md"
COHORTS = HERE / "cohorts"
FROZEN = HERE / "frozen"

FIELDS = ("key", "stage", "since", "until", "replaces", "shape",
          "command", "args", "verifier", "cost", "blocks")
LEGACY = ("id", "stage", "command", "verifier", "cost", "blocks")

# Phrases that are an exit code wearing a verifier's clothes.
HOLLOW = ("it succeeds", "it succeeded", "exit code", "exits zero",
          "no error", "reports success")

# The shapes the generator has. A shape outside this vocabulary is a row the
# generator cannot emit, which is the same as a row nobody can run.
SHAPES = ("once", "per-role", "attempts", "wait:capture", "wait:settle",
          "journal", "commit", "-")
LOOPING = ("attempts", "wait:capture", "wait:settle", "journal")

# The programs an invocation may name.
DRIVER_KINDS = ("bootstrap", "bootstrap-public", "bootstrap-offline", "solana",
                "script", "simulator", "sh")
LOOP_PREFIXES = ("@roles", "@owned_roles", "@participants")

PLACEHOLDER = re.compile(r"\{([a-z0-9_]+(?:[.-][a-z0-9_-]+)*)\}")
# The placeholders the GENERATOR resolves, per market or per loop, and so are
# not the manifest's to answer. Their shape is checked here; their value is not.
EMIT_TIME = re.compile(
    r"\{(?:market\.[a-z0-9_.]+|role|role_flag|participant|item|prior\.[a-z0-9_]+"
    r"|pubkey:[^{}]+|execute(?::[^{}]*)?|mode|attempt|stage:[a-z0-9-]+)\}")
INLINE_LOOP = re.compile(r"@(?:roles|owned_roles|list:[a-z0-9_.]+)\{")
ACT = re.compile(r"^\*(?:\[[^\]]*\])?\s*")


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
    """Resolve `group.field`, and the counts derived from `roles`.

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


def resolve(text: str, document: dict, where: str, problems: list[str],
            emit_time_ok: bool = False) -> str:
    def one(match):
        if emit_time_ok and EMIT_TIME.fullmatch(match.group(0)):
            return match.group(0)
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


def invocations(args: str) -> list[str]:
    return [piece.strip() for piece in args.split(" ;; ")] if args != "-" else []


def invocation_parts(text: str) -> tuple[str | None, bool, bool, str]:
    """(loop prefix, is-act, skip-if-output-exists, rest starting at the driver)."""
    loop = None
    for prefix in LOOP_PREFIXES:
        if text.startswith(prefix + " "):
            loop, text = prefix, text[len(prefix) + 1:]
            break
    act = bool(ACT.match(text))
    text = ACT.sub("", text, count=1)
    skip = text.startswith("?")
    if skip:
        text = text[1:]
    return loop, act, skip, text


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
        # The args are validated for SHAPE against this manifest: a manifest
        # field they name must exist; the generator's own placeholders pass.
        resolve(row["args"], document, where + " args", problems, emit_time_ok=True)
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
        shape = row["shape"]
        if shape not in SHAPES:
            problems.append(f"{row['key']} names shape {shape!r}, which the generator does not have")
        acts = 0
        for text in invocations(row["args"]):
            loop, act, _skip, rest = invocation_parts(text)
            acts += act
            kind = rest.split(" ", 1)[0]
            if kind not in DRIVER_KINDS:
                problems.append(f"{row['key']} names driver {kind!r}, which the generator cannot emit")
            if loop and shape == "per-role":
                problems.append(f"{row['key']} is per-role and also loops {loop}; one loop per row")
            for match in re.finditer(r"\{stage:([a-z0-9-]+)\}", rest):
                if match.group(1) not in known:
                    problems.append(f"{row['key']} names {{stage:{match.group(1)}}}, which is not a row")
        if shape in LOOPING and acts == 0:
            problems.append(f"{row['key']} has shape {shape} and no `*` act to loop")
        if shape not in LOOPING and acts:
            problems.append(f"{row['key']} marks a `*` act under shape {shape}, which does not loop")
        if shape == "commit" and row["args"] != "-":
            problems.append(f"{row['key']} is a commit and carries args; a commit has nothing to run")
        if shape == "-" and row["args"] != "-":
            problems.append(f"{row['key']} carries args under shape `-`; name the shape")


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


def prove_frozen() -> int:
    table = rows()
    structural(table, problems := [])
    checks = [("14", False, FROZEN / "cohort-14.tsv"),
              ("15", True, FROZEN / "cohort-15.tsv")]
    failures = 0
    for name, delta, path in checks:
        if not path.exists():
            print(f"  no frozen table at {path}")
            failures += 1
            continue
        document = manifest(name)
        view = render(table, select(table, int(name), delta), document, problems)
        ours, theirs = legacy_text(view), frozen_text(path)
        label = f"cohort-{name}{' delta' if delta else ''}"
        if ours == theirs:
            print(f"  {label}: {len(view)} rows reproduce {path.name} exactly")
            continue
        failures += 1
        print(f"  {label}: DOES NOT reproduce {path.name}")
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
                        help="print the six-column form the frozen tables use")
    parser.add_argument("--prove-frozen", action="store_true",
                        help="prove the union reproduces both frozen tables under frozen/")
    arguments = parser.parse_args()

    try:
        if arguments.prove_frozen:
            print("cohort runbook: reproducing the frozen tables")
            return prove_frozen()
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
    unemitted = [row["key"] for row in chosen if row["shape"] not in ("commit",) and row["args"] == "-"]
    print(f"{label} runbook: {len(view)} steps, each documented, each naming a "
          f"verifier, every field resolved from {document['schema']}; "
          f"{len(chosen) - len(unemitted)} carry their args, {len(unemitted)} do not"
          + (f" ({', '.join(unemitted)})" if unemitted else "") + ".")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
