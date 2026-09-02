#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""The pinned digests of the two-sided wire vectors, and the escape hatch.

THE HOLE THIS CLOSES. Three checked-in fixtures are written by a Rust encoder
and re-derived independently by a TypeScript one, so a wire that moves goes red
on the authoring side first. Each of the three Rust tests also accepts
`DCLUTCH_WRITE_WIRE_VECTOR=1`, which OVERWRITES the fixture and returns
success. That is the correct way for a deliberate move to land -- and it is
also a single environment variable that turns "the wire moved and nobody
noticed" into a green run, on both sides at once: regenerate, and the fixture,
the Rust encoder and the browser mirror all agree again about bytes nobody
looked at. The web tier would go green too, because it compares the mirror
against the same regenerated file.

    tools/ci/wire-vector-pins.py            verify (the default)
    tools/ci/wire-vector-pins.py --update   re-pin from disk, after a move

So the digest is pinned HERE, in a file no test can write. A deliberate move is
two edits in one commit -- the regenerated fixture and its pin -- and the pin
is the one a human has to type, which is the whole point: it is the place a
reviewer sees the bytes changed. `--update` prints every digest it moves so
the numbers can go in the commit message.

Both halves are needed and neither is sufficient. The Rust test proves the
fixture matches the encoder TODAY; this proves the fixture is the one that was
reviewed. Regeneration alone now fails twice over: the writing test refuses
after it writes (see each test's write branch), and this compares the new bytes
against a pin that did not move with them.

WHY A DIGEST AND NOT A LEAN THEOREM. The natural pin for a generated artifact
in this tree is the emitter that printed it, and `tools/emission-guard` already
owns that for files Lean emits. These three are not Lean-emitted: the authority
is a Rust encoder, and there is no theorem to move. A digest a human must retype
is the cheapest thing that makes the regeneration visible in review; if one of
these vectors ever acquires a Lean-side emitter, the guard there supersedes the
row here and it should be deleted rather than kept as a second authority.

The SDK copies are pinned even though only `apps/` is read back by
`browser_bump_hint_vector`: the write branch writes BOTH, so an unpinned SDK
copy is a file this repository generates and never checks.
"""

from __future__ import annotations

import hashlib
import pathlib
import sys

PINS = 'tools/ci/wire-vector-pins.tsv'


def rows(root: pathlib.Path):
    for number, line in enumerate((root / PINS).read_text().splitlines(), start=1):
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        fields = line.split('\t')
        if len(fields) != 3:
            raise SystemExit(
                f'{PINS}:{number}: expected 3 tab-separated fields'
                f' (sha256, path, authoring test), got {len(fields)}'
            )
        yield number, fields[0], fields[1], fields[2]


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    root = pathlib.Path(__file__).resolve().parents[2]
    update = '--update' in sys.argv[1:]
    moved: list[str] = []
    missing: list[str] = []
    lines: list[str] = []
    for _, pinned, relative, test in rows(root):
        path = root / relative
        if not path.exists():
            missing.append(relative)
            lines.append(f'{pinned}\t{relative}\t{test}')
            continue
        actual = digest(path)
        if actual != pinned:
            moved.append(f'  {relative}\n    pinned {pinned}\n    on disk {actual}\n'
                         f'    authored by {test}')
        lines.append(f'{actual if update else pinned}\t{relative}\t{test}')

    if missing:
        print('wire-vector-pins: a pinned fixture is MISSING, which is not a'
              ' digest mismatch and is not fixed by re-pinning:')
        for relative in missing:
            print(f'  {relative}')
        return 1

    if update:
        if not moved:
            print('wire-vector-pins: nothing moved; the pins already match disk.')
            return 0
        header = [line for line in (root / PINS).read_text().splitlines()
                  if not line.strip() or line.lstrip().startswith('#')]
        (root / PINS).write_text('\n'.join(header + lines) + '\n')
        print(f'wire-vector-pins: RE-PINNED {len(moved)} fixture(s). Put these'
              ' digests in the commit message,')
        print('and commit the regenerated fixture and this file TOGETHER --'
              ' separately, each half')
        print('looks like an accident to whoever reads it next.')
        print('\n'.join(moved))
        return 0

    if moved:
        print(f'wire-vector-pins: {len(moved)} fixture(s) do not match their'
              ' pinned digest.')
        print('\n'.join(moved))
        print()
        print('If the wire deliberately moved: re-run the authoring test with'
              ' DCLUTCH_WRITE_WIRE_VECTOR=1,')
        print(f'then `python3 {PINS[:-4]}.py --update`, and commit the fixture'
              ' and the pin in ONE commit.')
        print('If it did not: something regenerated a fixture without meaning'
              ' to, and the encoder,')
        print('the fixture and the browser mirror are now agreed about bytes'
              ' nobody reviewed.')
        return 1

    print(f'wire-vector-pins: {len(lines)} pinned fixture(s) match.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
