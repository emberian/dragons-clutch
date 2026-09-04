#!/usr/bin/env python3
"""One Rust `&str` constant, read from the file that declares it.

WHY THIS IS ITS OWN FILE. Two Python tools in this tree state the same wire
schema strings that the successor crate writes into the artifacts they read
back: `tools/release/private-validator-lifecycle/run.py`, which drives the
private-validator lifecycle, and `tools/devnet-reconcile/reconcile.py`, which
reconciles the evidence that lifecycle leaves behind. The runner stopped
restating them in `c04465f9`; the reconciler had not, and by 2026-09-04 two of
its literals were stale -- the terminal-sequence session at `-v1` against the
crate's `-v3`, and the private-lifecycle chaos session at `-v1` against `-v2`.
Both refused every session the current driver writes, and nothing went red,
because nothing in CI ran the reconciler at all.

So the derivation lives here, with one author and two readers, rather than
being copied into the second tool along with whatever the copy would drift
into. The VALUE has one author too, and it is neither of them: it is the Rust
`const`.

This is deliberately not the only reader of a Rust constant in the tree.
`preflight.py` keeps its own `rust_str_const` because its whole job is to
check the runner's WIRING independently -- a second reader there is the point.
A second reader in a tool that is merely consuming the value is not.
"""

from __future__ import annotations

import re
from pathlib import Path

# `tools/lib/rust_schema.py` -> `tools/lib` -> `tools` -> the tree root. Callers
# execute from their own directories and this file is the only one that has to
# know how far up the root is.
REPO_ROOT = Path(__file__).resolve().parents[2]


class SchemaOwnerRefusal(RuntimeError):
    """A schema owner is absent, unreadable, or has stopped having one answer."""


def rust_schema_constant(
    directory: str,
    file_name: str,
    constant: str,
    repo_root: Path | str | None = None,
) -> str:
    """The value of one `const NAME: &str = "...";` in one Rust file.

    Resolved against `repo_root`, which defaults to the tree this module is
    committed in. The release runner pins its own tree to `--repo` before it
    gets here -- `authenticate_offline_preflight` refuses with "executing
    lifecycle runner is outside the clean target source" when the two differ --
    so there is no second tree for this read to disagree with.

    Exactly one declaration is accepted. Zero means the constant was renamed or
    the owner moved; two means the file has stopped having one answer, and a
    caller that guessed between them would be back to holding an opinion about a
    value it does not own.
    """
    root = REPO_ROOT if repo_root is None else Path(repo_root)
    path = root / directory / file_name
    try:
        source = path.read_text(encoding="utf-8")
    except OSError as error:
        raise SchemaOwnerRefusal(
            f"cannot read schema owner {directory}/{file_name}: {error}"
        ) from error
    matches = re.findall(
        r"(?m)^\s*(?:pub(?:\([a-z]+\))?\s+)?const\s+"
        + re.escape(constant)
        + r"\s*:\s*&(?:'static\s+)?str\s*=\s*(?:\r?\n\s*)?\"([^\"]*)\"\s*;",
        source,
    )
    if len(matches) != 1 or not matches[0]:
        raise SchemaOwnerRefusal(
            f"{directory}/{file_name} must declare exactly one non-empty &str "
            f"{constant}; found {len(matches)}"
        )
    return matches[0]
