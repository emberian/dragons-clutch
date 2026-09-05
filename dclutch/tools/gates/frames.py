"""tools/gate frames -- the exact per-function SBF frame ratchet over the twelve program links.

  tools/gate frames [--commit REV]                  build every link with LLVM stack-size sections
                                                    and compare the canonical manifest with frames-baseline.json
  tools/gate frames --at REV --capture FILE         measure a detached worktree at REV; the manifest names it
  tools/gate frames --source DIR [--repo DIR] [--tools DIR] [--baseline FILE] [--capture FILE]
  tools/gate frames accept --first A --second B --output FILE
                                                    admit a baseline from two identical captures of ONE commit
  tools/gate frames owed [--repo DIR] [--since REV | --baseline FILE] [--until REV]
                                                    the commits since the baseline's commit that changed sources
                                                    compiled into a link and carried no baseline rows

Refuses: a link that did not freshly compile; any `overwrites values in the
frame` diagnostic; a function whose frames differ from the admitted multiset in
EITHER direction (shrinkage is red until the ratchet is lowered, so recovered
headroom cannot be spent again); a capture from a dirty tree with no --at (an
exact ratchet must name its base); two captures naming different commits.

A red comparison prints the `owed` ledger: each debtor with its `Lane:` trailer.
The unit of attribution is the link's path-dependency closure from `cargo metadata`,
normal and build edges only.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

from .common import (
    EXIT_FAIL, EXIT_PASS, EXIT_PREREQ, FRAME_DIAGNOSTIC, GATES, REPO, Failed, Prereq, archived,
    atomic_write, checked_out, dirty, have, note, repo_top, resolve_commit, scratch, sh,
)

REPORT_SCHEMA = "dclutch-sbf-frame-sizes-v1"
MANIFEST_SCHEMA = "dclutch-sbf-frame-manifest-v1"
BASELINE_SCHEMA = "dclutch-sbf-frame-baseline-v1"
BASELINE = GATES / "frames-baseline.json"
# Pinned, not discovered: a link silently dropping out of the measurement is the failure this guard exists to catch.
EXPECTED_LINK_COUNT = 8
SBPF_V0_FRAME_BYTES = 4096
COMMIT_FIELD = "commit"
FULL_COMMIT = re.compile(r"[0-9a-f]{40}")
UNATTRIBUTED = "unattributed"
CRATE_SOURCE_DIRECTORY = "src"
CRATE_BUILD_INPUTS = ("Cargo.toml", "Cargo.lock", "build.rs")
# A commit that carries either path carries its rows (the file moved on 2026-09-04).
BASELINE_ROWS = ("tools/gates/frames-baseline.json", "tools/frameguard/baseline.json")
# Compiler identity hashes are not source-level identity; everything else in a symbol is load-bearing.
LEGACY_RUST_HASH = re.compile(r"17h[0-9a-f]{16}E$")
V0_CRATE_HASH = re.compile(r"Cs[0-9A-Za-z]+_")
LLVM_HASH = re.compile(r"\.llvm\.[0-9A-Fa-f]+$")


# ------------------------------------------------------------------ manifests

def canonical_symbol(symbol: str) -> str:
    if not symbol or "\n" in symbol or "\0" in symbol:
        raise Prereq("frame report carries an empty or unsafe symbol")
    value = LLVM_HASH.sub(".llvm.<hash>", symbol)
    value = V0_CRATE_HASH.sub("Cs<hash>_", value)
    return LEGACY_RUST_HASH.sub("17h<hash>E", value)


def checked_commit(value: Any, label: str) -> str:
    if not isinstance(value, str) or not FULL_COMMIT.fullmatch(value):
        raise Prereq(f"{label} is not a full 40-character commit id")
    return value


def read_json(path: Path, label: str) -> Any:
    try:
        if not path.is_file() or path.is_symlink():
            raise Prereq(f"{label} is missing or not a regular file: {path}")
        return json.loads(path.read_text())
    except json.JSONDecodeError as error:
        raise Prereq(f"{label} is not JSON: {path}: {error}") from error
    except OSError as error:
        raise Prereq(f"cannot read {label}: {path}: {error}") from error


def natural_number(value: Any, label: str, *, positive: bool = False) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < (1 if positive else 0):
        raise Prereq(f"{label} is not a natural number")
    return value


def canonicalize_report(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or value.get("schema") != REPORT_SCHEMA:
        raise Prereq(f"{label} does not use {REPORT_SCHEMA}")
    if natural_number(value.get("bound_bytes"), f"{label} bound", positive=True) != SBPF_V0_FRAME_BYTES:
        raise Prereq(f"{label} measures a bound other than SBPF v0's {SBPF_V0_FRAME_BYTES}")
    frames = value.get("frames")
    if not isinstance(frames, list) or not frames:
        raise Prereq(f"{label} carries no measured frames")
    if natural_number(value.get("frame_count"), f"{label} frame_count") != len(frames):
        raise Prereq(f"{label} frame_count differs from its frame rows")
    grouped: defaultdict[str, list[int]] = defaultdict(list)
    for index, row in enumerate(frames):
        if not isinstance(row, dict) or set(row) != {"bytes", "symbol"} or not isinstance(row["symbol"], str):
            raise Prereq(f"{label} frame row {index} is malformed")
        grouped[canonical_symbol(row["symbol"])].append(natural_number(row["bytes"], f"{label} row {index} bytes"))
    functions = [{"symbol": s, "frames_bytes": sorted(sizes, reverse=True)} for s, sizes in sorted(grouped.items())]
    return {"frame_count": len(frames), "functions": functions}


def read_inventory(path: Path) -> list[str]:
    if not path.is_file() or path.is_symlink():
        raise Prereq(f"inventory is missing or not regular: {path}")
    packages = []
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", line):
            raise Prereq(f"inventory row {number} is malformed")
        packages.append(line)
    if len(packages) != EXPECTED_LINK_COUNT:
        raise Prereq(f"inventory is not the exact {EXPECTED_LINK_COUNT}-link set: {len(packages)}")
    if len(set(packages)) != len(packages) or packages != sorted(packages):
        raise Prereq("inventory packages are duplicated or not canonical")
    return packages


def write_manifest(path: Path, value: dict[str, Any]) -> None:
    try:
        atomic_write(path, json.dumps(value, indent=2, sort_keys=True) + "\n")
    except OSError as error:
        raise Prereq(f"cannot write {path}: {error}") from error


def assemble(inventory: Path, reports: Path, commit: str | None) -> dict[str, Any]:
    packages = read_inventory(inventory)
    if not reports.is_dir():
        raise Prereq(f"report directory is missing: {reports}")
    links = [{"package": p, **canonicalize_report(read_json(reports / f"{p}.json", f"{p} frame report"), f"{p} frame report")}
             for p in packages]
    manifest: dict[str, Any] = {"schema": MANIFEST_SCHEMA, "bound_bytes": SBPF_V0_FRAME_BYTES,
                                "link_count": len(links), "links": links}
    if commit is not None:
        manifest[COMMIT_FIELD] = checked_commit(commit, "measured commit")
    return manifest


def validate_manifest(value: Any, label: str, schemas: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict) or value.get("schema") not in schemas:
        raise Prereq(f"{label} has no admitted frame manifest schema")
    if set(value) - {COMMIT_FIELD} != {"schema", "bound_bytes", "link_count", "links"}:
        raise Prereq(f"{label} has missing or unknown fields")
    if COMMIT_FIELD in value:
        checked_commit(value[COMMIT_FIELD], f"{label} commit")
    if value["bound_bytes"] != SBPF_V0_FRAME_BYTES:
        raise Prereq(f"{label} does not bind the SBPF v0 frame wall")
    links = value["links"]
    if not isinstance(links, list) or value["link_count"] != EXPECTED_LINK_COUNT or len(links) != EXPECTED_LINK_COUNT:
        raise Prereq(f"{label} is not the exact {EXPECTED_LINK_COUNT}-link manifest")
    packages = []
    for link in links:
        if not isinstance(link, dict) or set(link) != {"package", "frame_count", "functions"}:
            raise Prereq(f"{label} link row is malformed")
        package = link["package"]
        if not isinstance(package, str) or not re.fullmatch(r"[a-z0-9][a-z0-9-]*", package):
            raise Prereq(f"{label} link package is unsafe")
        packages.append(package)
        functions = link["functions"]
        if not isinstance(functions, list) or not functions:
            raise Prereq(f"{label} {package} has no function rows")
        symbols, total = [], 0
        for function in functions:
            if not isinstance(function, dict) or set(function) != {"symbol", "frames_bytes"}:
                raise Prereq(f"{label} {package} function row is malformed")
            symbol = function["symbol"]
            if not isinstance(symbol, str) or canonical_symbol(symbol) != symbol:
                raise Prereq(f"{label} {package} function row is not canonical")
            symbols.append(symbol)
            sizes = function["frames_bytes"]
            if not isinstance(sizes, list) or not sizes:
                raise Prereq(f"{label} {package} {symbol} has no frames")
            checked = [natural_number(s, f"{label} {package} {symbol} frame") for s in sizes]
            if checked != sorted(checked, reverse=True):
                raise Prereq(f"{label} {package} {symbol} frames are unsorted")
            total += len(checked)
        if symbols != sorted(symbols) or len(symbols) != len(set(symbols)):
            raise Prereq(f"{label} {package} symbols are not canonical")
        if natural_number(link["frame_count"], f"{label} {package} frame_count") != total:
            raise Prereq(f"{label} {package} frame_count differs from rows")
    if packages != sorted(packages) or len(packages) != len(set(packages)):
        raise Prereq(f"{label} package order or identity is not canonical")
    return value


def frames_only(value: dict[str, Any]) -> dict[str, Any]:
    return {k: v for k, v in value.items() if k != COMMIT_FIELD}


def named_base(value: dict[str, Any]) -> str:
    commit = value.get(COMMIT_FIELD)
    return commit if isinstance(commit, str) else "an unnamed source tree"


def differences(before: dict[str, Any], after: dict[str, Any]) -> list[str]:
    messages: list[str] = []
    old_links = {link["package"]: link for link in before["links"]}
    new_links = {link["package"]: link for link in after["links"]}
    for package in sorted(set(old_links) | set(new_links)):
        if package not in old_links:
            messages.append(f"{package}: new unbaselined link")
            continue
        if package not in new_links:
            messages.append(f"{package}: admitted link disappeared")
            continue
        old = {r["symbol"]: r["frames_bytes"] for r in old_links[package]["functions"]}
        new = {r["symbol"]: r["frames_bytes"] for r in new_links[package]["functions"]}
        for symbol in sorted(set(old) | set(new)):
            if symbol not in old:
                messages.append(f"{package}: new function {symbol}")
            elif symbol not in new:
                messages.append(f"{package}: function disappeared {symbol}")
            elif old[symbol] != new[symbol]:
                grew = len(new[symbol]) > len(old[symbol]) or any(c > a for c, a in zip(new[symbol], old[symbol]))
                messages.append(f"{package}: {'GREW' if grew else 'changed/ratcheted'} {symbol}: {old[symbol]} -> {new[symbol]}")
    return messages


def check(baseline_path: Path, candidate_path: Path) -> str:
    baseline = validate_manifest(read_json(baseline_path, "frame baseline"), "frame baseline", {BASELINE_SCHEMA})
    candidate = validate_manifest(read_json(candidate_path, "candidate manifest"), "candidate manifest", {MANIFEST_SCHEMA})
    projected = frames_only({**baseline, "schema": MANIFEST_SCHEMA})
    measured = frames_only(candidate)
    if projected != measured:
        delta = differences(projected, measured)
        detail = "\n".join(f"  {line}" for line in delta[:20]) + (f"\n  ... and {len(delta) - 20} more" if len(delta) > 20 else "")
        raise Failed(f"per-function frame manifest differs from the ratchet admitted at {named_base(baseline)}:\n{detail}")
    return named_base(baseline)


def accept(first_path: Path, second_path: Path, output: Path) -> str:
    first = validate_manifest(read_json(first_path, "first capture"), "first capture", {MANIFEST_SCHEMA})
    second = validate_manifest(read_json(second_path, "second capture"), "second capture", {MANIFEST_SCHEMA})
    if first != second:
        delta = differences(frames_only(first), frames_only(second))
        if first.get(COMMIT_FIELD) != second.get(COMMIT_FIELD):
            delta.insert(0, f"captured at different commits: {named_base(first)} then {named_base(second)}")
        detail = "\n".join(f"  {line}" for line in delta[:20])
        raise Failed(f"independent captures disagree; no baseline accepted:\n{detail}")
    commit = first.get(COMMIT_FIELD)
    if commit is None:
        raise Prereq("neither capture names the commit it measured; recapture with --at <commit> --capture <file>")
    write_manifest(output, {**first, "schema": BASELINE_SCHEMA})
    return checked_commit(commit, "accepted commit")


# ------------------------------------------------------------------ owed

def _git(repo: Path, *arguments: str) -> str:
    result = sh(["git", "-C", repo, *arguments], capture=True)
    if result.returncode != 0:
        first = (result.stderr.strip().splitlines() or [""])[0]
        raise Prereq(f"git {' '.join(arguments)} failed in {repo}: {first}")
    return result.stdout


def discover_links(repo: Path) -> list[str]:
    programs = repo / "programs"
    if not programs.is_dir():
        raise Prereq(f"no programs directory under {repo}")
    return sorted(e.name for e in programs.iterdir() if e.is_dir() and not e.is_symlink() and (e / "Cargo.toml").is_file())


def path_dependency_closure(repo: Path, link: str) -> tuple[set[str], str | None]:
    """In-repo crates compiled into one link (normal + build edges), or the program crate alone, SAID OUT LOUD."""
    fallback = {f"programs/{link}"}
    manifest = repo / "programs" / link / "Cargo.toml"
    if not manifest.is_file():
        return fallback, f"{link} has no Cargo.toml"
    try:
        finished = subprocess.run(["cargo", "metadata", "--format-version", "1", "--offline", "--manifest-path", str(manifest)],
                                  text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    except OSError as error:
        return fallback, f"cargo is not runnable ({error})"
    if finished.returncode != 0:
        return fallback, f"cargo metadata failed for {link}"
    try:
        metadata = json.loads(finished.stdout)
        nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
        manifests = {p["id"]: Path(p["manifest_path"]) for p in metadata["packages"]}
        root = metadata["resolve"]["root"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        return fallback, f"cargo metadata for {link} is unreadable: {error}"
    if root is None:
        return fallback, f"cargo metadata for {link} resolves no root package"
    top, closure, seen, stack = repo.resolve(), set(), set(), [root]
    while stack:
        identifier = stack.pop()
        if identifier in seen or identifier not in nodes:
            continue
        seen.add(identifier)
        manifest_path = manifests.get(identifier)
        if manifest_path is None:
            continue
        try:
            closure.add(manifest_path.parent.resolve().relative_to(top).as_posix())
        except (ValueError, OSError):
            continue
        for dependency in nodes[identifier]["deps"]:
            kinds = dependency.get("dep_kinds")
            if kinds is not None and all(k.get("kind") == "dev" for k in kinds):
                continue  # dev-dependencies are not in the link
            stack.append(dependency["pkg"])
    return (closure, None) if closure else (fallback, f"{link} resolved an empty closure")


def reaches(name: str, crate: str) -> bool:
    prefix = f"{crate}/"
    if not name.startswith(prefix):
        return False
    rest = name[len(prefix):]
    return rest.startswith(f"{CRATE_SOURCE_DIRECTORY}/") or rest in CRATE_BUILD_INPUTS


def owed(repo: Path, since: str | None, baseline_path: Path | None, until: str) -> None:
    if since is None:
        if baseline_path is None:
            raise Prereq("owed needs --since or --baseline")
        baseline = validate_manifest(read_json(baseline_path, "frame baseline"), "frame baseline", {BASELINE_SCHEMA})
        since = baseline.get(COMMIT_FIELD)
        if since is None:
            raise Prereq(f"{baseline_path} names no captured commit; pass --since or recapture with --at")
    base = _git(repo, "rev-parse", "--verify", "--quiet", f"{since}^{{commit}}").strip()
    head = _git(repo, "rev-parse", "--verify", "--quiet", f"{until}^{{commit}}").strip()
    revisions = _git(repo, "rev-list", "--reverse", f"{base}..{head}").split()
    links = discover_links(repo)
    if len(links) != EXPECTED_LINK_COUNT:
        print(f"frames: NOTE -- {len(links)} program manifests, not the admitted {EXPECTED_LINK_COUNT}", file=sys.stderr)
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        resolved = list(pool.map(lambda link: (link, *path_dependency_closure(repo, link)), links))
    closures = {link: closure for link, closure, _ in resolved}
    degraded = [reason for _, _, reason in resolved if reason]
    crates = sorted({crate for closure in closures.values() for crate in closure})
    debtors = []
    for revision in revisions:
        names = _git(repo, "diff-tree", "--no-commit-id", "--name-only", "-r", revision).splitlines()
        if any(row in names for row in BASELINE_ROWS):
            continue
        moved = sorted({crate for crate in crates if any(reaches(n, crate) for n in names)})
        if not moved:
            continue
        reached = sorted(link for link, closure in closures.items() if closure & set(moved))
        recorded = _git(repo, "log", "-1", "--format=%s%x00%(trailers:key=Lane,valueonly,separator=%x2C)", revision).strip("\n")
        subject, _, lane = recorded.partition("\0")
        lane = " ".join(lane.split()) or UNATTRIBUTED
        shown = moved[:4] + ([f"and {len(moved) - 4} more"] if len(moved) > 4 else [])
        debtors.append(f"{revision[:8]} [lane {lane}] {subject}\n      reaches {', '.join(reached)}\n      via {', '.join(shown)}")
    scope = f"{base[:8]}..{head[:8]} ({len(revisions)} commits)"
    coverage = f"{len(links)} links over {len(crates)} first-party crates" + (
        f"; {len(degraded)} closure(s) UNRESOLVED, falling back to the program crate alone: {'; '.join(degraded[:3])}" if degraded else "")
    if not debtors:
        print(f"frames: {scope} -- no commit changed a link's compiled sources without carrying its baseline rows ({coverage})")
        return
    raise Failed(
        f"{len(debtors)} commit(s) in {scope} changed sources compiled into a link and left the frame ratchet to someone else ({coverage}):\n"
        + "\n".join(f"  {line}" for line in debtors)
        + "\n  Each owes a `tools/gate frames --at <its commit> --capture` in its own commit, or a statement that it leaves the ratchet red."
        f"\n  `lane` is the commit's `Lane:` trailer; `{UNATTRIBUTED}` means it was committed without tools/lane.sh.")


# ------------------------------------------------------------------ measuring

def measure(*, source: Path, repo: Path | None, tools: Path, measured_commit: str | None,
            baseline: Path | None, capture: Path | None, dry_run: bool = False) -> int:
    """Build every link fresh with stack-size sections, assemble the manifest, then capture or check."""
    for command in ("cargo-build-sbf", "cargo", "python3"):
        if not have(command):
            raise Prereq(f"{command} is not on PATH")
    parser = tools / "tools" / "sbf-frame-sizes.py"
    if not parser.is_file() or parser.is_symlink():
        raise Prereq(f"frame parser is missing from the measuring tools: {parser}")
    if tools != source:
        note(f"measuring with the tools in {tools}")
    baseline = baseline or BASELINE
    if capture is None and (not baseline.is_file() or baseline.is_symlink()):
        raise Prereq(f"baseline is missing or not regular: {baseline}")
    manifests = sorted(p for p in source.glob("programs/*/Cargo.toml") if p.is_file())
    packages = sorted(p.parent.name for p in manifests)
    if len(packages) != EXPECTED_LINK_COUNT or len(set(packages)) != EXPECTED_LINK_COUNT:
        raise Prereq(f"program inventory is not the exact {EXPECTED_LINK_COUNT}-link set: {len(packages)}")
    with scratch("frames") as tmp:
        inventory, reports = tmp / "inventory.tsv", tmp / "reports"
        inventory.write_text("".join(f"{p}\n" for p in packages))
        reports.mkdir()
        for package in packages:
            target, log = tmp / f"target-{package}", tmp / f"build-{package}.log"
            manifest = source / "programs" / package / "Cargo.toml"
            note(f"build {package}")
            env = {**__import__("os").environ, "RUSTC_BOOTSTRAP": "1", "RUSTFLAGS": "-Zemit-stack-sizes --emit=obj,link",
                   "CARGO_TERM_COLOR": "never", "CARGO_TARGET_DIR": str(target)}
            if dry_run:
                note(f"$ cd {source} && RUSTFLAGS='-Zemit-stack-sizes --emit=obj,link' cargo build-sbf --manifest-path {manifest} -- --locked")
                continue
            with open(log, "w") as handle:
                built = subprocess.run(["cargo", "build-sbf", "--manifest-path", str(manifest), "--", "--locked"],
                                       cwd=source, env=env, stdout=handle, stderr=subprocess.STDOUT)
            text = log.read_text(errors="replace")
            if built.returncode:
                print(text[-3000:], file=sys.stderr)
                raise Failed(f"{package} measurement build failed")
            if not re.search(rf"^\s*Compiling\s+{re.escape(package)}\s+v\S+", text, re.M):
                raise Prereq(f"{package} has no fresh top-package compile marker; no measurement")
            diagnostics = text.count(FRAME_DIAGNOSTIC)
            if diagnostics:
                print("\n".join(sorted({l for l in text.splitlines() if FRAME_DIAGNOSTIC in l})), file=sys.stderr)
                raise Failed(f"{package} emitted {diagnostics} stack-frame overwrite diagnostics")
            triple = next((t for t in ("sbpf-solana-solana", "sbf-solana-solana") if (target / t).is_dir()), None)
            if triple is None:
                raise Prereq(f"{package} emitted no recognizable SBF target directory")
            obj = target / triple / "release" / "deps" / f"{package.replace('-', '_')}.o"
            if not obj.is_file() or obj.is_symlink():
                raise Prereq(f"{package} fresh measurement object is missing: {obj}")
            parsed = sh(["python3", parser, "--format", "json", obj], capture=True)
            if parsed.returncode == 1:
                print(parsed.stderr, file=sys.stderr)
                raise Failed(f"{package}: a frame is at or over the {SBPF_V0_FRAME_BYTES}-byte wall")
            if parsed.returncode:
                raise Prereq(f"{package}: the frame parser could not run: {parsed.stderr.strip()[:200]}")
            (reports / f"{package}.json").write_text(parsed.stdout)
        if dry_run:
            return EXIT_PASS
        manifest_value = assemble(inventory, reports, measured_commit)
        if capture is not None:
            write_manifest(capture, manifest_value)
            print(f"frames: captured the complete {EXPECTED_LINK_COUNT}-link manifest of {measured_commit} at {capture}")
            return EXIT_PASS
        candidate = tmp / "candidate.json"
        write_manifest(candidate, manifest_value)
        try:
            base = check(baseline, candidate)
        except Failed as disagreement:
            print(f"frames: REFUSING -- {disagreement}", file=sys.stderr)
            if repo is not None:
                try:
                    owed(repo, None, baseline, measured_commit or "HEAD")
                except (Failed, Prereq) as ledger:
                    print(f"frames: {ledger}", file=sys.stderr)
            return EXIT_FAIL
        print(f"frames: complete per-function frame manifest matches the ratchet admitted at {base}")
        return EXIT_PASS


def resolve_source(*, source: Path | None, at: str | None, repo: Path | None, capture: Path | None):
    """Which tree is measured and under what name: a named commit, a clean checkout's HEAD, or an unnamed tree.

    Returns a context manager yielding (source_root, repo_or_None, measured_commit_or_None).
    """
    import contextlib

    source = (source or REPO).resolve()
    repo = (repo or source).resolve()
    top = repo_top(repo)

    @contextlib.contextmanager
    def named():
        if at is not None:
            if top is None:
                raise Prereq(f"--at {at} needs a git repository; {repo} is not one")
            with checked_out(at, top) as (root, sha):
                print(f"frames: measuring commit {sha} in a detached worktree")
                yield root, top, sha
            return
        if top is not None and source.resolve() == top.resolve():
            count = dirty(repo=top)
            if count:
                if capture is not None:
                    raise Prereq(f"REFUSING to capture from a dirty tree: {count} tracked path(s) differ from HEAD; "
                                 "an exact ratchet must name its base (use --at HEAD)")
                print("frames: measuring a DIRTY tree; the comparison names no commit")
                yield source, top, None
                return
            sha = resolve_commit("HEAD", top)
            print(f"frames: measuring clean HEAD {sha}")
            yield source, top, sha
            return
        if capture is not None:
            raise Prereq(f"REFUSING to capture from {source}, which no repository names; use --repo DIR --at <commit>")
        print(f"frames: measuring {source}, which no repository names; the comparison names no commit")
        yield source, top, None

    return named()


def tier(ctx):
    """The tier: a clean export of --commit measured by THIS tree's instrument, else the working tree."""
    if not have("cargo-build-sbf"):
        raise Prereq("cargo-build-sbf is not on PATH")
    if ctx.commit:
        with archived(ctx.commit) as (root, sha):
            note(f"measuring commit {sha} (clean export)")
            code = measure(source=root, repo=REPO, tools=REPO, measured_commit=sha, baseline=None, capture=None, dry_run=ctx.dry_run)
    else:
        note("measuring the working tree; use --commit HEAD for a quoteable ratchet run")
        with resolve_source(source=REPO, at=None, repo=REPO, capture=None) as (root, repo, sha):
            code = measure(source=root, repo=repo, tools=REPO, measured_commit=sha, baseline=None, capture=None, dry_run=ctx.dry_run)
    return code, "the per-function frame ratchet disagrees" if code == EXIT_FAIL else ""


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="tools/gate frames", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command")
    parser.add_argument("--source", type=Path)
    parser.add_argument("--repo", type=Path)
    parser.add_argument("--tools", type=Path)
    parser.add_argument("--at")
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--capture", type=Path)
    parser.add_argument("--commit")
    p = sub.add_parser("assemble")
    p.add_argument("--inventory", type=Path, required=True)
    p.add_argument("--reports", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--commit", default=None)
    p = sub.add_parser("check")
    p.add_argument("--baseline", type=Path, required=True)
    p.add_argument("--candidate", type=Path, required=True)
    p = sub.add_parser("accept")
    p.add_argument("--first", type=Path, required=True)
    p.add_argument("--second", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p = sub.add_parser("owed")
    p.add_argument("--repo", type=Path, default=REPO)
    p.add_argument("--since")
    p.add_argument("--until", default="HEAD")
    p.add_argument("--baseline", type=Path, default=None)
    args = parser.parse_args(argv)
    try:
        if args.command == "assemble":
            write_manifest(args.output, assemble(args.inventory, args.reports, args.commit))
        elif args.command == "check":
            print(f"frames: complete per-function frame manifest matches the ratchet admitted at {check(args.baseline, args.candidate)}")
        elif args.command == "accept":
            print(f"frames: accepted two identical independent captures of {accept(args.first, args.second, args.output)} at {args.output}")
        elif args.command == "owed":
            owed(args.repo, args.since, args.baseline or (BASELINE if args.since is None else None), args.until)
        else:
            if args.commit:
                with archived(args.commit) as (root, sha):
                    return measure(source=root, repo=REPO, tools=(args.tools or REPO).resolve(), measured_commit=sha,
                                   baseline=args.baseline, capture=args.capture)
            with resolve_source(source=args.source, at=args.at, repo=args.repo, capture=args.capture) as (root, repo, sha):
                return measure(source=root, repo=repo, tools=(args.tools or REPO).resolve(), measured_commit=sha,
                               baseline=args.baseline, capture=args.capture)
        return EXIT_PASS
    except Failed as error:
        print(f"frames: REFUSING -- {error}", file=sys.stderr)
        return EXIT_FAIL
    except Prereq as error:
        print(f"frames: COULD NOT RUN -- {error}", file=sys.stderr)
        return EXIT_PREREQ
