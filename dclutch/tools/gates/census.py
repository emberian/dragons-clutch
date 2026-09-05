"""tools/gate census -- the route census, enumerated from the Rust AST and checked unique.

  tools/gate census [--commit REV | --source DIR] [--work DIR] [--revision SHA] [--no-tests]
      build tools/gauntlet/census (release, into <work>/census-target), run its own tests,
      write <work>/out/inventory.json with --check-unique, render <work>/out/CENSUS.md
      from the shared <work>/out/ledger.json and tools/gauntlet/blocked.json.
  tools/gate census observe --bindings F --programs F --evidence F [--work DIR]
      fold one campaign's chain evidence into the ledger, under the ledger lock.

Refuses: a refusal code outside its registered band or claimed twice, an 8-byte
magic claimed by two names without an adjudicated entry in magic-collisions.json,
a schema identity that is not the SHA-256 of the label it documents, a program
directory missing from the census target list, and a campaign transaction the
chain's own logs do not corroborate.

The default --work is the gauntlet's own (/private/tmp/dclutch-gauntlet, or
DCLUTCH_GAUNTLET_WORK), so the campaign runners and this census share one ledger.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

from .common import (
    EXIT_FAIL, EXIT_PASS, EXIT_USAGE, REPO, Prereq, archived, dirty, have, ledger_lock, note, sh,
)

WORK_DEFAULT = Path(os.environ.get("DCLUTCH_GAUNTLET_WORK", "/private/tmp/dclutch-gauntlet"))
CRATE = REPO / "tools" / "gauntlet" / "census"
BLOCKED = REPO / "tools" / "gauntlet" / "blocked.json"


def binary(work: Path, *, run_tests: bool = True, jobs: str = "4", dry_run: bool = False) -> Path:
    """The census binary, built from THIS tree's crate (the instrument) into <work>/census-target.

    DCLUTCH_GATE_CENSUS_BIN names an already-built binary instead (the campaign runners, and the tests).
    """
    if os.environ.get("DCLUTCH_GATE_CENSUS_BIN"):
        return Path(os.environ["DCLUTCH_GATE_CENSUS_BIN"])
    if not have("cargo"):
        raise Prereq("cargo is not on PATH")
    if not (CRATE / "Cargo.toml").is_file():
        raise Prereq("tools/gauntlet/census is absent")
    target = work / "census-target"
    env = {**os.environ, "CARGO_TARGET_DIR": str(target), "CARGO_BUILD_JOBS": jobs}
    if dry_run:
        note(f"$ cd {CRATE} && CARGO_TARGET_DIR={target} cargo build --release" + (" && cargo test --release" if run_tests else ""))
        return target / "release" / "dclutch-route-census"
    if sh(["cargo", "build", "--release", "--quiet"], cwd=CRATE, env=env).returncode:
        raise Prereq("the census crate did not build")
    if run_tests and sh(["cargo", "test", "--release", "--quiet"], cwd=CRATE, env=env).returncode:
        raise Prereq("the census crate's own adversarial tests failed; its verdicts are not evidence")
    return target / "release" / "dclutch-route-census"


def inventory(binary_path: Path, root: Path, out: Path, *, revision: str | None, dry_run: bool = False) -> int:
    args = [binary_path, "inventory", "--root", root, "--out", out, "--check-unique"]
    if revision:
        args += ["--revision", revision]
    if dry_run:
        note("$ " + " ".join(str(a) for a in args))
        return 0
    out.parent.mkdir(parents=True, exist_ok=True)
    return sh(args).returncode


def report(binary_path: Path, work: Path, *, dry_run: bool = False) -> int:
    out = work / "out"
    args = [binary_path, "report", "--inventory", out / "inventory.json", "--ledger", out / "ledger.json",
            "--blocked", BLOCKED, "--out", out / "CENSUS.md"]
    if dry_run:
        note("$ " + " ".join(str(a) for a in args))
        return 0
    if not BLOCKED.is_file():
        raise Prereq("tools/gauntlet/blocked.json is absent")
    with ledger_lock(out / "ledger.json"):
        return sh(args).returncode


def run(*, commit: str | None = None, source: Path | None = None, work: Path = WORK_DEFAULT,
        revision: str | None = None, run_tests: bool = True, jobs: str = "4", dry_run: bool = False):
    if commit:
        with archived(commit) as (root, sha):
            note(f"enumerating commit {sha} (clean export)")
            return _run_over(root, sha, work, run_tests, jobs, dry_run)
    root = source or REPO
    if source is None:
        count = dirty("programs", "crates")
        if count:
            note(f"enumerating the WORKING TREE: {count} uncommitted file(s) under programs/ and crates/")
    return _run_over(root, revision, work, run_tests, jobs, dry_run)


def _run_over(root: Path, revision: str | None, work: Path, run_tests: bool, jobs: str, dry_run: bool):
    binary_path = binary(work, run_tests=run_tests, jobs=jobs, dry_run=dry_run)
    out = work / "out"
    if inventory(binary_path, root, out / "inventory.json", revision=revision, dry_run=dry_run):
        return EXIT_FAIL, "a refusal-code, magic or schema-identity collision, or the enumeration failed"
    if report(binary_path, work, dry_run=dry_run):
        return EXIT_FAIL, "the census report could not be rendered"
    note(f"inventory: {out / 'inventory.json'}")
    note(f"report:    {out / 'CENSUS.md'}")
    return EXIT_PASS, ""


def observe(*, work: Path, bindings: Path, programs: Path, evidence: Path, jobs: str = "4") -> int:
    binary_path = binary(work, run_tests=False, jobs=jobs)
    out = work / "out"
    with ledger_lock(out / "ledger.json"):
        code = sh([binary_path, "observe", "--inventory", out / "inventory.json", "--ledger", out / "ledger.json",
                   "--bindings", bindings, "--programs", programs, "--evidence", evidence]).returncode
    return EXIT_FAIL if code else EXIT_PASS


def main(argv: list[str]) -> int:
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return EXIT_PASS
    options: dict[str, str] = {}
    flags: set[str] = set()
    positional: list[str] = []
    tokens = list(argv)
    while tokens:
        token = tokens.pop(0)
        if token in ("--no-tests",):
            flags.add(token)
        elif token.startswith("--"):
            if not tokens:
                print(f"census: {token} needs a value", file=sys.stderr)
                return EXIT_USAGE
            options[token] = tokens.pop(0)
        else:
            positional.append(token)
    work = Path(options.get("--work", WORK_DEFAULT))
    if positional == ["observe"]:
        try:
            return observe(work=work, bindings=Path(options["--bindings"]), programs=Path(options["--programs"]),
                           evidence=Path(options["--evidence"]))
        except KeyError as missing:
            print(f"census observe needs {missing.args[0]}", file=sys.stderr)
            return EXIT_USAGE
    if positional:
        print(f"census: unknown argument {positional[0]}", file=sys.stderr)
        return EXIT_USAGE
    code, detail = run(commit=options.get("--commit"),
                       source=Path(options["--source"]).resolve() if "--source" in options else None,
                       work=work, revision=options.get("--revision"), run_tests="--no-tests" not in flags)
    if detail:
        print(f"census: {detail}", file=sys.stderr)
    return code
