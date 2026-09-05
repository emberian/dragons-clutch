"""tools/gate reference -- docs/reference/ and its client mirrors, at their fixpoint.

  tools/gate reference                     regenerate docs/reference/ (tools/genref/generate.mjs)
  tools/gate reference --check             byte-compare docs/reference/, write nothing
  tools/gate reference --converge          regenerate the reference AND the client emitters that read it,
                                           until a pass moves nothing (at most three passes)
  tools/gate reference --check --converge  is the COMMITTED revision already that fixpoint (--commit REV, default HEAD)
  --allow-dirty (or GENREF_ALLOW_DIRTY=1)  regenerate from a dirty working tree anyway

Refuses: a dirty working tree (a regeneration would emit reference docs for code
no commit holds -- regenerate from a detached worktree at HEAD); a committed
revision that is not at its fixpoint; a generator pair that still moves a file on
the third pass; an emitter under apps/*/scripts or packages/*/scripts that reads
docs/reference and is not declared reference-coupled here.

`docs/reference/refusals.md` feeds `generate-refusal-registry.mjs`, whose module
this generator mirrors back into `docs/reference/abi/`, so one pass is never
enough. Exit 0 at the fixpoint (written or not), 1 stale or non-convergent, 2 dirty.
"""

from __future__ import annotations

import hashlib
import os
import subprocess
import sys
from pathlib import Path

from .common import EXIT_FAIL, EXIT_PASS, EXIT_PREREQ, REPO, Failed, Prereq, archived, dirty, have, note, scratch, sh

GENERATE = REPO / "tools" / "genref" / "generate.mjs"
MAX_PASSES = 3
# The emitters that close the loop: they read docs/reference and are mirrored back into it.
REFERENCE_COUPLED = (
    "packages/dclutch-sdk/scripts/generate-refusal-registry.mjs",
    "packages/dclutch-sdk/scripts/generate-route-census.mjs",
    "packages/dclutch-sdk/scripts/generate-market-phase-admission.mjs",
)
# The reference and the one client mirror of it: the web app imports the SDK's
# generated modules and carries no reference-coupled copy of its own.
CONVERGE_TREES = ("docs/reference", "packages/dclutch-sdk/lib/generated")


def inventory_for(root: Path, work: Path) -> Path:
    """The census inventory of `root`, built by this tree's census crate into `work`."""
    from . import census

    binary = census.binary(work, run_tests=False)
    out = work / "inventory.json"
    if census.inventory(binary, root, out, revision=None):
        raise Failed("the census refused: a refusal-code, magic or identity collision")
    return out


def discovered_coupled(root: Path) -> list[str]:
    found = []
    for scripts in ("apps/dclutch-web/scripts", "packages/dclutch-sdk/scripts"):
        directory = root / scripts
        if not directory.is_dir():
            continue
        for path in sorted(directory.iterdir()):
            if path.is_file() and "docs/reference" in path.read_text(errors="replace"):
                found.append(str(path.relative_to(root)))
    return sorted(found)


def digest_trees(root: Path) -> dict[str, str]:
    out = {}
    for tree in CONVERGE_TREES:
        base = root / tree
        if not base.is_dir():
            continue
        for path in sorted(p for p in base.rglob("*") if p.is_file()):
            out[str(path.relative_to(root))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return out


def generate(root: Path, inventory: Path, *extra: str) -> int:
    return sh(["node", root / "tools/genref/generate.mjs", "--inventory", inventory, *extra], cwd=root).returncode


def converge(root: Path, work: Path) -> int:
    """Run the generator and the reference-coupled emitters until nothing moves. Exit 0 at the fixpoint."""
    declared = sorted(REFERENCE_COUPLED)
    found = discovered_coupled(root)
    if found != declared:
        note("the reference-coupled emitter list is no longer the truth; declared: " + ", ".join(declared))
        note("found reading docs/reference: " + (", ".join(found) or "(none)"))
        raise Failed("an emitter reading docs/reference is not declared in tools/gates/reference.py; the reference would stay one pass behind")
    inventory = inventory_for(root, work)
    wrote = False
    for number in range(1, MAX_PASSES + 1):
        print(f"\n=== reference converge: pass {number} of {MAX_PASSES} ===")
        before = digest_trees(root)
        if generate(root, inventory):
            raise Failed(f"generate.mjs failed on pass {number}")
        for emitter in REFERENCE_COUPLED:
            emitter_dir = root / Path(emitter).parents[1]
            result = sh(["node", root / emitter], cwd=emitter_dir, capture=True)
            if result.returncode:
                print(result.stdout + result.stderr, file=sys.stderr)
                raise Failed(f"{emitter} failed")
            print(f"    ran {emitter}")
        after = digest_trees(root)
        moved = sorted(p for p in set(before) | set(after) if before.get(p) != after.get(p))
        if not moved:
            print(f"    pass {number} moved nothing.")
            if wrote:
                print(f"reference converge: fixpoint proved by pass {number}; commit the reference and the client mirrors together.")
            else:
                print("reference converge: fixpoint on the first pass -- nothing moved.")
            return EXIT_PASS
        wrote = True
        print(f"    pass {number} moved:")
        for path in moved:
            print(f"      {path}")
    raise Failed(f"NO FIXPOINT after {MAX_PASSES} passes; the generators do not agree under repetition. Do not commit this output.")


def check_committed(rev: str, repo: Path = REPO) -> int:
    """Is the committed revision already the fixpoint? Exported, converged there, and compared."""
    with archived(rev, repo) as (root, sha):
        print(f"reference check: measuring {sha}, not this working tree.")
        with scratch("reference") as work:
            before = digest_trees(root)
            converge(root, work)
            after = digest_trees(root)
    moved = sorted(p for p in set(before) | set(after) if before.get(p) != after.get(p))
    if moved:
        for path in moved:
            print(f"  stale at {sha[:12]}: {path}", file=sys.stderr)
        raise Failed(f"{sha[:12]} is NOT the fixpoint; run `tools/gate reference --converge` from a detached worktree at HEAD")
    print(f"reference check: {sha[:12]} is already the fixpoint.")
    return EXIT_PASS


def tier(ctx):
    if not have("cargo") or not have("node"):
        raise Prereq("cargo and node are both required")
    if not GENERATE.is_file():
        raise Prereq("tools/genref/generate.mjs is absent")
    if ctx.dry_run:
        note(f"$ tools/gate reference --check --converge --commit {ctx.commit or 'HEAD'}")
        return EXIT_PASS, ""
    try:
        return check_committed(ctx.commit or "HEAD"), ""
    except Failed as error:
        return EXIT_FAIL, str(error)


def main(argv: list[str], root: Path = REPO) -> int:
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return EXIT_PASS
    flags = {a for a in argv if a.startswith("--")}
    commit = next((argv[i + 1] for i, a in enumerate(argv) if a == "--commit" and i + 1 < len(argv)), None)
    unknown = flags - {"--check", "--converge", "--allow-dirty", "--commit"}
    if unknown:
        print(f"reference: unknown option {sorted(unknown)[0]}", file=sys.stderr)
        return 64
    if not have("node") or not have("cargo"):
        raise Prereq("node and cargo are both required")
    try:
        if "--check" in flags and "--converge" in flags:
            return check_committed(commit or os.environ.get("GENREF_CONVERGE_REV", "HEAD"), root)
        allow_dirty = "--allow-dirty" in flags or os.environ.get("GENREF_ALLOW_DIRTY") == "1"
        if not allow_dirty and dirty(repo=root):
            print("reference: refusing to regenerate from a dirty tree; use a detached worktree at HEAD, or --allow-dirty", file=sys.stderr)
            return EXIT_PREREQ
        with scratch("reference") as work:
            if "--converge" in flags:
                return converge(root, work)
            inventory = inventory_for(root, work)
            return EXIT_FAIL if generate(root, inventory, *(["--check"] if "--check" in flags else [])) else EXIT_PASS
    except Failed as error:
        print(f"reference: REFUSING -- {error}", file=sys.stderr)
        return EXIT_FAIL
