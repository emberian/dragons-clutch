#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Root-workspace integration tests that CI compiles and never executes.

`tools/release/check-all-workspaces.py` builds with `--all-targets`, so every
`tests/*.rs` in the root workspace COMPILES on every run. Nothing runs them.
Every `cargo test` in this repository points somewhere else:

    tools/ci/run.sh:729   the program-test workspace, which is not this one
    tools/ci/run.sh:845   the journey workspace, and `--bins` at that
    tools/ci/run.sh:871   `cargo check`, deliberately, for the tools tier
    tools/gauntlet/run.sh the census workspace, which is not this one

A compiled-never-run test is the "green over something that never executed"
shape, and it is worse than no test because it looks like coverage. It was
found on 2026-09-02 through one instance: a two-sided fixture whose TypeScript
half ran in the web tier and whose Rust half -- the side that AUTHORS the bytes
-- did not, so an encoder that moved without regenerating the fixture left the
mirror and the fixture consistent with each other and CI green.

This enumerates the class rather than that instance. It is CHEAP: one
`cargo metadata --no-deps` and a read of each target's source text. It runs
nothing.

CLASSIFICATION, and it is deliberately conservative in the direction that
costs. A target is EXPENSIVE if its source mentions `lake` (it re-runs a Lean
emitter) or `ProgramTest`/`SBF_OUT_DIR` (it needs a built ELF). Everything else
is CHEAP. A false "cheap" is a slow CI row; a false "expensive" is a test that
goes on never running, so the greps are broad on purpose.

THE MEASUREMENT HAS SINCE BEEN MADE, and it refuted the reason this tool gave
for not wiring the cheap ones. The ten-minute figure quoted here was a COLD
target directory: each `cargo test -p X --test Y` was paying for a build, not
for a re-resolve. Warm, one execution each of all 80 at `0bac7f001` is about
three minutes of execution, no target over 2 GB and none near the 120 s
timeout. `tools/ci/root-targets.tsv` records every target's measured time and
what CI does with it; `tools/ci/run.sh root-targets` is the tier.

So this file has a second job now: `--check` is that tier's CONTROL. The tier
can only run what somebody listed, and a list is exactly the kind of thing that
agrees with reality right up until a new target lands beside it. `--check`
compares the enumeration above against the tsv and fails on either direction --
a cheap target with no row (it would go on never running, which is the whole
defect) or a row naming a target that is no longer cheap or no longer exists.

    tools/ci/never-run-tests.py            counts and the excluded lists
    tools/ci/never-run-tests.py --cheap    the runnable targets, one per line
    tools/ci/never-run-tests.py --check    every cheap target has a tsv row
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

LAKE = ('"lake"',)
ELF = ('solana_program_test', 'ProgramTest', 'SBF_OUT_DIR')

# The wiring decision for every cheap target, and the ONLY copy of it: the
# `root-targets` tier reads these rows to know what to run, and `--check` below
# reads them to know whether the list still covers what this file enumerates.
# Two readers, one file, because the alternative -- the tier holding its own
# list -- is a value duplicated instead of read, which is this project's named
# signature defect.
ROOT_TARGETS = 'tools/ci/root-targets.tsv'
STATUSES = ('run', 'quarantine', 'slow')

# The per-target admission budget, in seconds, and it is checked against the
# number RECORDED IN THE TSV rather than against a wall clock. That is the
# whole point: a wall-clock gate on a machine a dozen lanes share fails for
# reasons unrelated to what it gates -- one target here measured 39.95s, 9.80s
# and 6.48s in three rounds -- and a gate whose red is usually somebody else's
# build teaches everyone to ignore it. Checking the committed number instead is
# deterministic, and it puts the obligation where it belongs: a lane adding a
# target has to MEASURE it and write the number down.
#
# 8.00 rather than 5.00 because that is where the measured gap is. See the
# tsv's own header for the three-target cluster it would otherwise split.
BUDGET_SECONDS = 8.00


def read_root_targets(root: pathlib.Path):
    """Every non-comment row of the tsv as (status, package, target, secs, note).

    Raises rather than returning a partial list: a malformed row would
    otherwise silently drop a target out of the tier, which is the exact
    never-executed shape this whole file exists to detect.
    """
    path = root / ROOT_TARGETS
    rows = []
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        fields = line.split('\t')
        if len(fields) < 4:
            raise SystemExit(
                f'{ROOT_TARGETS}:{number}: expected at least 4 tab-separated'
                f' fields (status, package, target, secs), got {len(fields)}'
            )
        status, package, target, secs = fields[:4]
        note = fields[4] if len(fields) > 4 else ''
        if status not in STATUSES:
            raise SystemExit(
                f'{ROOT_TARGETS}:{number}: unknown status {status!r};'
                f' expected one of {", ".join(STATUSES)}'
            )
        rows.append((status, package, target, secs, note))
    return rows


def check(root: pathlib.Path, cheap) -> int:
    """The tier's control: the cheap set and the tsv name the same targets.

    Both directions are failures and they are different defects. A cheap target
    with NO row is the original finding recurring -- a test that compiles on
    every CI run and executes on none. A row with no target is a tier running
    (or excusing) something that is gone, which is how an exclusion outlives
    the reason for it.
    """
    rows = read_root_targets(root)
    listed = {(package, target): (status, secs)
              for status, package, target, secs, _ in rows}
    enumerated = {(package, target) for package, target, _ in cheap}
    unwired = sorted(enumerated - set(listed))
    orphaned = sorted(set(listed) - enumerated)
    for package, target in unwired:
        print(f'UNWIRED   cargo test -p {package} --test {target}')
    for package, target in orphaned:
        print(f'ORPHANED  {ROOT_TARGETS} names {package} --test {target},'
              f' which is no longer a cheap root-workspace target')

    # The budget, on the committed number. Both directions again, and again
    # they are different defects: a `run` row over budget makes the tier slower
    # than anybody agreed to, and a `slow` row UNDER budget is an exclusion
    # that outlived its reason -- a target sitting out of CI because it used to
    # be slow is the same never-executed shape wearing a label.
    overweight = []
    undeserved = []
    for status, package, target, secs, _ in rows:
        try:
            seconds = float(secs)
        except ValueError:
            print(f'MALFORMED {package} --test {target}: seconds {secs!r} is'
                  ' not a number')
            overweight.append((package, target, secs))
            continue
        if status in ('run', 'quarantine') and seconds > BUDGET_SECONDS:
            overweight.append((package, target, secs))
            print(f'OVER      {package} --test {target} records {secs}s,'
                  f' over the {BUDGET_SECONDS:.2f}s budget')
        if status == 'slow' and seconds <= BUDGET_SECONDS:
            undeserved.append((package, target, secs))
            print(f'EXCUSED   {package} --test {target} is marked `slow` at'
                  f' {secs}s, which is within the {BUDGET_SECONDS:.2f}s budget')

    if unwired or orphaned or overweight or undeserved:
        print()
        print(f'{len(unwired)} cheap target(s) outside the tier,'
              f' {len(orphaned)} tsv row(s) naming nothing,')
        print(f'{len(overweight)} over budget, {len(undeserved)} excluded'
              ' without needing to be.')
        print(f'Every cheap target needs a row in {ROOT_TARGETS}: `run` if it'
              ' passes, `quarantine` if it')
        print('is red today (measure it, then say so), `slow` if it exceeds the'
              ' per-target budget')
        print('(give its measured seconds). A target with no row is a target'
              ' CI compiles and never runs.')
        return 1
    print(f'{len(enumerated)} cheap targets, {len(enumerated)} wired:'
          f' zero outside the tier ({ROOT_TARGETS})')
    print(f'every row within the {BUDGET_SECONDS:.2f}s per-target budget,'
          ' or excluded and over it')
    return 0


def targets(root: pathlib.Path):
    raw = subprocess.run(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1'],
        cwd=root, capture_output=True, text=True, check=True,
    ).stdout
    for pkg in json.loads(raw)['packages']:
        for target in pkg['targets']:
            if 'test' not in target['kind']:
                continue
            source = pathlib.Path(target['src_path'])
            text = source.read_text(errors='replace') if source.exists() else ''
            needs = []
            if any(marker in text for marker in LAKE):
                needs.append('lake')
            if any(marker in text for marker in ELF):
                needs.append('elf')
            yield pkg['name'], target['name'], needs


def main() -> int:
    root = pathlib.Path(__file__).resolve().parents[2]
    rows = sorted(targets(root))
    cheap = [row for row in rows if not row[2]]
    lake = [row for row in rows if 'lake' in row[2]]
    elf = [row for row in rows if 'elf' in row[2] and 'lake' not in row[2]]

    if '--cheap' in sys.argv:
        for package, target, _ in cheap:
            print(f'{package} {target}')
        return 0

    if '--check' in sys.argv:
        return check(root, cheap)

    packages = len({row[0] for row in rows})
    print(f'root-workspace integration test targets: {len(rows)} across {packages} packages')
    print(f'  runnable without lake or an ELF: {len(cheap)}')
    print(f'  excluded, re-runs a Lean emitter: {len(lake)}')
    print(f'  excluded, needs a built ELF:      {len(elf)}')
    print()
    print('EXCLUDED -- re-runs a Lean emitter (needs `lake`):')
    for package, target, _ in lake:
        print(f'  cargo test -p {package} --test {target}')
    print('EXCLUDED -- needs a built ELF:')
    for package, target, _ in elf:
        print(f'  cargo test -p {package} --test {target}')
    print()
    counts: dict[str, int] = {}
    for status, _, _, _, _ in read_root_targets(root):
        counts[status] = counts.get(status, 0) + 1
    print(f'WIRING ({ROOT_TARGETS}), for the cheap set only:')
    for status in STATUSES:
        print(f'  {status:<12} {counts.get(status, 0)}')
    print('  `--check` is the control: it fails if any cheap target has no row.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
