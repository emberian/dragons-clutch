#!/usr/bin/env python3
"""Evaluate SBF stack-frame-overwrite diagnostics against this tier's exemptions.

`cargo build-sbf` exits ZERO when the backend reports that a call overwrites its
own stack frame and "may cause undefined behavior during execution". `run.sh`
counts them and warns. This tier refuses, because the journey's whole claim is
about state surviving a long chain of transactions, and undefined behaviour
anywhere in that chain voids the claim silently.

`frame-diagnostics.json` is the narrow exception and is shaped like
`blocked.json`: every entry names the exact mangled symbol fragment, the
measured count, why this campaign does not reach it, and who owns the fix.

Exit 0 only when every observed diagnostic matches an entry AND no entry's count
grew. A count that shrank is reported loudly as stale and does not fail: the
person who lands the fix should not be met with a red run.

usage: check-frame-diagnostics.py ALLOWED.json OBSERVED.tsv
  OBSERVED.tsv is one diagnostic line per row, prefixed by "<role>\\t".
"""

import collections
import json
import sys


def main() -> int:
    allowed_path, observed_path = sys.argv[1], sys.argv[2]
    with open(allowed_path, encoding="utf-8") as handle:
        allowed = json.load(handle)["allowed"]
    with open(observed_path, encoding="utf-8") as handle:
        rows = [line.rstrip("\n") for line in handle if line.strip()]

    matched = collections.Counter()
    unmatched = []
    for row in rows:
        role, _, text = row.partition("\t")
        for index, entry in enumerate(allowed):
            if entry["symbol_fragment"] in text and entry["role"] == role:
                matched[index] += 1
                break
        else:
            unmatched.append(row)

    problems = 0
    for index, entry in enumerate(allowed):
        observed, expected = matched[index], entry["measured"]
        label = f'{entry["role"]}: {entry["symbol_fragment"]}'
        if observed == 0:
            print(
                f"frame-diagnostics: STALE ENTRY — {label} produced none. "
                f"The fix landed; delete the entry rather than letting it outlive its reason."
            )
        elif observed > expected:
            problems += 1
            print(
                f"frame-diagnostics: GREW — {label} measured {expected}, now {observed}. "
                f"A growing count is a new defect wearing an old exemption. Re-measure and "
                f"re-justify, or fix it."
            )
        elif observed < expected:
            print(
                f"frame-diagnostics: shrank — {label} measured {expected}, now {observed}. "
                f"Lower the recorded count so the exemption stays exactly as wide as the defect."
            )
        else:
            print(
                f"frame-diagnostics: EXEMPT ({observed}) — {label}\n"
                f"  not reached: {entry['unreached_by_this_tier']}\n"
                f"  owner: {entry['owner']}"
            )

    if unmatched:
        problems += 1
        print(
            f"frame-diagnostics: {len(unmatched)} diagnostic(s) match NO entry. The artifact the "
            f"toolchain calls potentially-undefined is not one this tier has accounted for:"
        )
        for row in sorted(set(unmatched)):
            print(f"  {row}")

    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
