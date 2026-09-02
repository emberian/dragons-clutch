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

    tools/ci/never-run-tests.py            counts and the excluded lists
    tools/ci/never-run-tests.py --cheap    the runnable targets, one per line

CLASSIFICATION, and it is deliberately conservative in the direction that
costs. A target is EXPENSIVE if its source mentions `lake` (it re-runs a Lean
emitter) or `ProgramTest`/`SBF_OUT_DIR` (it needs a built ELF). Everything else
is CHEAP. A false "cheap" is a slow CI row; a false "expensive" is a test that
goes on never running, so the greps are broad on purpose.

What this tool does NOT do is wire the cheap ones into a tier. Doing that needs
a MEASURED runtime for the set, and the measurement is not the sum of the
parts: a per-target `cargo test -p X --test Y` loop over the 80 exceeded ten
minutes locally because each invocation re-resolves, while one invocation
sharing a build would not. Wiring an unmeasured multi-minute row into a shared
tier is how a tier gets reverted. The number below is the argument for doing
the measurement; it is not the measurement.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

LAKE = ('"lake"',)
ELF = ('solana_program_test', 'ProgramTest', 'SBF_OUT_DIR')


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
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
