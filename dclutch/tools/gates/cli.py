"""Dispatch: a gate name runs a tier with its defaults; an instrument name with options runs it alone."""

from __future__ import annotations

import importlib
import sys
import time

from .common import EXIT_PASS, EXIT_PREREQ, EXIT_USAGE, Context, Failed, Prereq, Verdict, say
from .tiers import ALL, CHEAP, TIERS

# Instruments a caller can drive with their own options. Each module exposes
# `main(argv) -> int`. The tier of the same name is the instrument at its
# defaults, so `tools/gate emission` and `tools/gate emission --verify` agree.
INSTRUMENTS = {
    "census": "gates.census",
    "emission": "gates.emission",
    "frames": "gates.frames",
    "reference": "gates.reference",
    "witness": "gates.witness",
    "budgets": "gates.budgets",
    "commands": "gates.commands",
    "citations": "gates.citations",
    "lane": "gates.lane",
    "archive": "gates.archive",
    "selftest": "gates.selftest",
}

GLOBAL_FLAGS = {"--commit", "--require", "--dry-run"}
TIER_NAMES = [tier.name for tier in TIERS]


def usage(stream=sys.stdout) -> None:
    from pathlib import Path

    print(Path(__file__).resolve().parents[1].joinpath("gate").read_text().split('"""')[1], file=stream)


def list_tiers() -> None:
    print(f"{'gate':<14}{"cost":<38}{'needs':<30}refuses")
    for tier in TIERS:
        print(f"{tier.name:<14}{tier.cost:<38}{tier.needs:<30}{tier.gates}")
    print()
    print(f"cheap = {' '.join(CHEAP)}")
    print(f"all   = {' '.join(ALL)}")
    print("        (workspaces is deliberately outside all: fresh target directory per workspace, the cut's price)")
    print()
    print("--commit REV   measure a clean export of REV instead of the working tree (any number you QUOTE)")
    print("--require      a missing prerequisite is a failure (the cut)")
    print("--dry-run      print each gate's commands instead of running them")
    print("CARGO_BUILD_JOBS=4  DCLUTCH_GATE_SUITES=\"core custody\"  DCLUTCH_GATE_SUITE_DRAWS=3  DCLUTCH_GATE_TIME_SLACK=4  DCLUTCH_GATE_BUILD_ROOT=<dir>")


def run_tiers(names: list[str], ctx: Context) -> int:
    verdict = Verdict()
    by_name = {tier.name: tier for tier in TIERS}
    for name in names:
        tier = by_name[name]
        say(f"{name} -- {tier.gates}")
        started = time.time()
        try:
            code, detail = tier.run(ctx)
        except Prereq as error:
            code, detail = EXIT_PREREQ, str(error)
        except Failed as error:
            code, detail = 1, str(error)
        verdict.record(f"{name} ({int(time.time() - started)}s)", code, detail, require=ctx.require)
    say("verdict")
    print(verdict.render(), end="")
    return verdict.worst


def main(argv: list[str]) -> int:
    if not argv or argv[0] in ("-h", "--help", "help"):
        usage()
        list_tiers()
        return EXIT_PASS if argv else EXIT_USAGE
    if argv[0] in ("--list", "-l"):
        list_tiers()
        return EXIT_PASS

    head, rest = argv[0], argv[1:]
    instrument_mode = head in INSTRUMENTS and any(
        token not in TIER_NAMES and token not in GLOBAL_FLAGS and token not in ("cheap", "all")
        and not (index > 0 and rest[index - 1] == "--commit")
        for index, token in enumerate(rest)
    )
    if instrument_mode:
        module = importlib.import_module(INSTRUMENTS[head])
        try:
            return module.main(rest)
        except Prereq as error:
            print(f"{head}: COULD NOT RUN -- {error}", file=sys.stderr)
            return EXIT_PREREQ
        except Failed as error:
            print(f"{head}: REFUSING -- {error}", file=sys.stderr)
            return 1

    ctx = Context()
    names: list[str] = []
    tokens = list(argv)
    while tokens:
        token = tokens.pop(0)
        if token == "--require":
            ctx.require = True
        elif token == "--dry-run":
            ctx.dry_run = True
        elif token == "--commit":
            if not tokens:
                print("tools/gate: --commit needs a revision", file=sys.stderr)
                return EXIT_USAGE
            ctx.commit = tokens.pop(0)
        elif token.startswith("--commit="):
            ctx.commit = token.split("=", 1)[1]
        elif token == "cheap":
            names.extend(CHEAP)
        elif token == "all":
            names.extend(ALL)
        elif token in TIER_NAMES:
            names.append(token)
        else:
            print(f"tools/gate: unknown gate {token}\n", file=sys.stderr)
            list_tiers()
            return EXIT_USAGE
    if not names:
        list_tiers()
        return EXIT_USAGE
    return run_tiers(names, ctx)
