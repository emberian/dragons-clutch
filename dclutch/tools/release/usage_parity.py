#!/usr/bin/env python3
"""Every flag a tool TEACHES must be a flag that tool PARSES.

C-13's sharp sentence is "runbooks contain only commands actually replayed by
their campaigns." A `usage()` string is a runbook: `--help` prints it, guides
copy from it, and operators type it. When it names a flag the parser will
reject, the tool documents a command that cannot run, and nothing goes red --
the drift is invisible until somebody at a keyboard hits it.

Measured 2026-09-01, the reasons this is a gate and not a lint:

  - `tools/ci/run.sh`'s own header comment, which `--help` printed, listed six
    tiers when the dispatch ran nine and described `cheap` as two tiers when it
    ran three. Its `--list` was correct the whole time; the duplicate rotted.
  - Five commands taught in `docs/` were stale in exactly this way, including
    `run.py --through participant` in the two most-read guides, missing five
    required arguments.

WHICH DIRECTION IS LOAD-BEARING, because only one of them is.

TAUGHT BUT NOT PARSED is a defect: the tool promises something it refuses.
PARSED BUT NOT TAUGHT is usually deliberate -- diagnostic flags, arms of a
subcommand a given `usage()` does not cover -- so it is reported for the reader
and never fails the gate. A gate that fires on undocumented flags would cry
wolf, and a gate that cries wolf gets ignored, which is worse than no gate.

SCOPE IS THE CRATE, NOT THE FILE, and this distinction is the whole
correctness of the tool. A first cut of this check scanned each file against
its own parser and reported 24 of 34 files disagreeing. Every one was a false
positive: `--i-mean-devnet` is taught in nine files and parsed in the shared
devnet-acknowledgement arms of others, `--unknown` and `--invented` are words
inside refusal-message strings, and `--keypair-` is a `strip_prefix` stem
rather than a flag. Scanned crate-wide with those three shapes understood, the
true figure is zero.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

# Exit codes are this repository's one convention, adopted from
# `tools/seam-audit/seam_audit.py`: 0 green, 1 this tree has the defect, 2 the
# checker could not run and nothing was proven either way.
EXIT_PASS = 0
EXIT_GATE_FAILED = 1
EXIT_PREREQ_MISSING = 2

USAGE_FN = re.compile(r"fn usage\(\)[^{]*\{(.*?)\n\}", re.S)
FLAG = re.compile(r"--[a-z0-9][a-z0-9-]*")
QUOTED_FLAG = re.compile(r"\"(--[a-z0-9][a-z0-9-]*)\"")
STRIP_PREFIX = re.compile(r"strip_prefix\(\"(--[a-z0-9][a-z0-9-]*)\"")


def audit(crate_src: pathlib.Path) -> tuple[list[str], list[str], int]:
    """Return (failures, notes, usage_function_count) for one crate's sources."""
    sources = sorted(crate_src.glob("*.rs"))
    if not sources:
        raise FileNotFoundError(f"no Rust sources under {crate_src}")

    everything = "".join(path.read_text() for path in sources)
    # A flag is "parsed" if it appears as a quoted literal ANYWHERE in the
    # crate: match arms, comparisons and the shared acknowledgement helpers
    # other modules delegate to.
    parsed = set(QUOTED_FLAG.findall(everything))
    # `--keypair-<role>` is read with strip_prefix, so every flag under a
    # declared stem is parsed even though no literal spells it out.
    stems = tuple(STRIP_PREFIX.findall(everything))

    failures: list[str] = []
    notes: list[str] = []
    usage_functions = 0

    for path in sources:
        source = path.read_text()
        found = USAGE_FN.search(source)
        if found is None:
            continue
        usage_functions += 1
        taught = set(FLAG.findall(found.group(1)))
        missing = sorted(
            flag
            for flag in taught - parsed
            if not any(flag.startswith(stem) for stem in stems)
        )
        for flag in missing:
            failures.append(
                f"{path.relative_to(crate_src.parents[0])}: usage() teaches "
                f"{flag}, which no parser in this crate accepts"
            )
        undocumented = sorted(parsed - taught) if taught else []
        if undocumented:
            notes.append(f"{path.name}: {len(undocumented)} flags parsed but not in its usage()")

    if usage_functions == 0:
        # An empty derivation is a broken checker, never a clean tree.
        raise ValueError(f"no `fn usage()` found under {crate_src}; the scan is broken")

    return failures, notes, usage_functions


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--crate-src",
        action="append",
        required=True,
        help="absolute path to a crate's src/ directory; repeatable",
    )
    parser.add_argument(
        "--verbose", action="store_true", help="also list undocumented flags"
    )
    arguments = parser.parse_args(argv)

    total_failures: list[str] = []
    total_usage = 0
    for raw in arguments.crate_src:
        path = pathlib.Path(raw)
        if not path.is_dir():
            print(f"usage-parity: not a directory: {path}", file=sys.stderr)
            return EXIT_PREREQ_MISSING
        try:
            failures, notes, count = audit(path)
        except (FileNotFoundError, ValueError) as error:
            print(f"usage-parity: {error}", file=sys.stderr)
            return EXIT_PREREQ_MISSING
        total_failures.extend(failures)
        total_usage += count
        if arguments.verbose:
            for note in notes:
                print(f"note\t{note}")

    for failure in total_failures:
        print(f"TAUGHT-NOT-PARSED\t{failure}")
    status = "STOP" if total_failures else "PASS"
    print(
        f"SUMMARY\tusage_functions={total_usage}\t"
        f"taught_not_parsed={len(total_failures)}\tstatus={status}"
    )
    return EXIT_GATE_FAILED if total_failures else EXIT_PASS


if __name__ == "__main__":
    sys.exit(main())
