#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Gate the cohort-14 runbook against its own step table.

A runbook is prose and prose drifts. This checks the three things that make
this one usable when somebody is four hours into a deploy and tired:

  1. Every row in `steps.tsv` appears in `README.md`, and every step id the
     README claims exists in `steps.tsv`. A step described in one and not the
     other is a step nobody owns.
  2. Every row NAMES A VERIFIER, and the verifier is not "it succeeded". A
     step whose check is its own exit code is the failure this repository calls
     silent success, and cohort-13 met it three times in one day.
  3. Every `blocks` reference names a real step id, and the graph has no cycle,
     so "what cannot start until this is green" is answerable.

It reads two files and exits. No cargo, no chain, no keys.
"""

from __future__ import annotations

import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
STEPS = HERE / "steps.tsv"
README = HERE / "README.md"

# Phrases that are an exit code wearing a verifier's clothes.
HOLLOW = (
    "it succeeds",
    "it succeeded",
    "exit code",
    "exits zero",
    "no error",
    "reports success",
)


def rows() -> list[dict[str, str]]:
    parsed = []
    for number, line in enumerate(STEPS.read_text().splitlines(), start=1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 6:
            sys.exit(f"steps.tsv:{number}: {len(fields)} tab-separated fields, expected 6")
        parsed.append(
            dict(zip(("id", "stage", "command", "verifier", "cost", "blocks"), fields))
        )
    return parsed


def main() -> int:
    steps = rows()
    ids = [step["id"] for step in steps]
    problems: list[str] = []

    if len(set(ids)) != len(ids):
        problems.append("steps.tsv has duplicate ids")
    if ids != sorted(ids):
        problems.append("steps.tsv rows are not in id order")

    readme = README.read_text()
    # The README's own step headings, `### 07 activate-direct` and so on.
    documented = dict(re.findall(r"^### (\d\d) ([a-z0-9-]+)$", readme, re.MULTILINE))
    for step in steps:
        if step["id"] not in documented:
            problems.append(f"step {step['id']} {step['stage']} has no README heading")
        elif documented[step["id"]] != step["stage"]:
            problems.append(
                f"step {step['id']}: README says {documented[step['id']]!r}, "
                f"steps.tsv says {step['stage']!r}"
            )
        verifier = step["verifier"].strip()
        if len(verifier) < 20:
            problems.append(f"step {step['id']} names no real verifier: {verifier!r}")
        lowered = verifier.lower()
        for hollow in HOLLOW:
            if hollow in lowered:
                problems.append(
                    f"step {step['id']}'s verifier is an exit code in disguise: {verifier!r}"
                )
        for blocked in step["blocks"].split(","):
            blocked = blocked.strip()
            if blocked in ("", "-"):
                continue
            if blocked not in ids:
                problems.append(f"step {step['id']} blocks {blocked!r}, which is not a step")
            elif blocked <= step["id"]:
                problems.append(
                    f"step {step['id']} blocks {blocked}, which does not come after it"
                )
    for extra in sorted(set(documented) - set(ids)):
        problems.append(f"README documents step {extra}, which steps.tsv does not have")

    if problems:
        print("cohort14 runbook: the README and the step table disagree.")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print(f"cohort14 runbook: {len(steps)} steps, each documented and each naming a verifier.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
