#!/usr/bin/env python3
"""Canonicalize and ratchet complete per-function SBF frame reports.

`sbf-frame-sizes.py --format json` reports the raw symbols in one fresh object.
This tool removes only compiler-generated identity hashes, preserves colliding
monomorphizations as a sorted multiset, and joins all twelve program objects
into one canonical manifest. `check` requires that manifest to equal the
committed baseline exactly: growth is red, and shrinkage is also red until the
baseline is ratcheted down, so recovered headroom cannot silently be spent
again.

A baseline is accepted only from two independently captured manifests, both of
which must name the SAME source commit, which the accepted baseline then
records. Object hashes are intentionally absent: equality of the complete
canonical function map, not a lucky build artifact hash, is the repeatability
witness -- but WHICH SOURCE was measured is not a build detail, and an exact
ratchet whose base is "whatever the tree held that minute" cannot be reviewed
after the fact. `owed` reads that recorded commit back and names the commits
since it that moved program sources without carrying baseline rows.

Exit 0 means the comparison ran and agreed. Exit 1 means measured frames or
the two acceptance runs disagree. Exit 2 means an input/prerequisite was
missing or malformed, so no frame claim was made.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
import concurrent.futures
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from typing import Any


REPORT_SCHEMA = "dclutch-sbf-frame-sizes-v1"
MANIFEST_SCHEMA = "dclutch-sbf-frame-manifest-v1"
BASELINE_SCHEMA = "dclutch-sbf-frame-baseline-v1"
# Twelve since `e6b7bf1a` deleted `dclutch-dealer-sbf`, a standalone measurement
# prototype its own header disclaimed. `6590f042` moved the runner's copy of
# this count and left the checker's behind, so every capture since has built all
# twelve links clean and then been refused by the assembler -- which is why the
# committed baseline was still the thirteen-link one, and why nothing had
# ratcheted this gate since `8c050751`. The count is pinned rather than
# discovered on purpose (a link silently dropping out of the measurement is the
# failure this guard exists to catch), so it moves by hand, with a reason, in
# BOTH halves. Ten since 2026-09-04, when `dclutch-direct-aot-sbf` and
# `dclutch-product-runtime-v2-sbf` -- both `false` in SHIPPED_LINKS, in no
# cohort, their bands retired -- were deleted; eight from the same day, when
# `dclutch-general-accelerator-sbf`, `dclutch-dealer-accelerator-sbf` and
# `dclutch-series-shadow-sbf` became the one `dclutch-accelerator-sbf`.
EXPECTED_LINK_COUNT = 8
SBPF_V0_FRAME_BYTES = 4096
# A manifest names the commit whose sources it measured. It is metadata, not
# frame content: `check` compares the function map with this field REMOVED, so
# re-measuring the same frames at a later commit is green and only the frames
# ratchet. `accept` refuses captures that name nothing or name different
# commits, because a baseline whose base is unnamed cannot be diffed against
# anything -- which is how three correct recaptures were invalidated in one
# night, each by a program commit that landed while the four-minute double
# build was still running.
COMMIT_FIELD = "commit"
FULL_COMMIT = re.compile(r"[0-9a-f]{40}")
# What `owed` prints for a commit made without `tools/lane.sh`, which is the
# only way a commit here reaches HEAD with no `Lane:` trailer. It is named
# rather than blank so the ledger distinguishes "no lane claimed this" from a
# formatting hole.
UNATTRIBUTED = "unattributed"


# The attribution predicate for `owed`. A link's frames are a function of every
# crate compiled INTO it, not only of the program crate: the codegen that moves
# a frame is as often three edges down the path-dependency closure. That closure
# is read from `cargo metadata`, which already answers the question and answers
# it the way the build does -- deriving it by hand from Cargo.toml would make
# this file a second author for the dependency graph. Within a crate, only what
# the compiler reads can move a frame: sources, the manifest that selects
# features and dependencies, the lock that pins them, and a build script.
CRATE_SOURCE_DIRECTORY = "src"
CRATE_BUILD_INPUTS = ("Cargo.toml", "Cargo.lock", "build.rs")
BASELINE_ROWS = "tools/frameguard/baseline.json"

# Legacy Rust mangling ends in a crate/codegen hash. Rust v0 mangling carries
# crate disambiguators as `Cs<base62>_`. Neither names a source-level function,
# and both can move when an unrelated unit changes. LLVM's clone suffix is the
# same kind of non-semantic identity. Do not strip any other suffix: closure,
# shim and monomorphization identity remains load-bearing, and collisions are
# retained as a multiset rather than silently merged into one frame.
LEGACY_RUST_HASH = re.compile(r"17h[0-9a-f]{16}E$")
V0_CRATE_HASH = re.compile(r"Cs[0-9A-Za-z]+_")
LLVM_HASH = re.compile(r"\.llvm\.[0-9A-Fa-f]+$")


class MissingOrMalformed(RuntimeError):
    """No comparison can be made from the supplied inputs."""


class Disagreement(RuntimeError):
    """The measurement ran and disagrees with its admitted predecessor."""


def canonical_symbol(symbol: str) -> str:
    if not symbol or "\n" in symbol or "\0" in symbol:
        raise MissingOrMalformed("frame report carries an empty or unsafe symbol")
    value = LLVM_HASH.sub(".llvm.<hash>", symbol)
    value = V0_CRATE_HASH.sub("Cs<hash>_", value)
    value = LEGACY_RUST_HASH.sub("17h<hash>E", value)
    return value


def checked_commit(value: Any, label: str) -> str:
    if not isinstance(value, str) or not FULL_COMMIT.fullmatch(value):
        raise MissingOrMalformed(f"{label} is not a full 40-character commit id")
    return value


def read_json(path: Path, label: str) -> Any:
    try:
        if not path.is_file() or path.is_symlink():
            raise MissingOrMalformed(f"{label} is missing or not a regular file: {path}")
        return json.loads(path.read_text())
    except json.JSONDecodeError as error:
        raise MissingOrMalformed(f"{label} is not JSON: {path}: {error}") from error
    except OSError as error:
        raise MissingOrMalformed(f"cannot read {label}: {path}: {error}") from error


def natural_number(value: Any, label: str, *, positive: bool = False) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise MissingOrMalformed(f"{label} is not an integer")
    if value < (1 if positive else 0):
        raise MissingOrMalformed(f"{label} is outside its admitted range")
    return value


def canonicalize_report(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or value.get("schema") != REPORT_SCHEMA:
        raise MissingOrMalformed(f"{label} does not use {REPORT_SCHEMA}")
    bound = natural_number(value.get("bound_bytes"), f"{label} bound", positive=True)
    if bound != SBPF_V0_FRAME_BYTES:
        raise MissingOrMalformed(
            f"{label} measures bound {bound}, not SBPF v0's {SBPF_V0_FRAME_BYTES}"
        )
    frames = value.get("frames")
    if not isinstance(frames, list) or not frames:
        raise MissingOrMalformed(f"{label} carries no measured frames")
    if natural_number(value.get("frame_count"), f"{label} frame_count") != len(frames):
        raise MissingOrMalformed(f"{label} frame_count differs from its frame rows")

    grouped: defaultdict[str, list[int]] = defaultdict(list)
    for index, row in enumerate(frames):
        if not isinstance(row, dict) or set(row) != {"bytes", "symbol"}:
            raise MissingOrMalformed(f"{label} frame row {index} is malformed")
        size = natural_number(row["bytes"], f"{label} frame row {index} bytes")
        symbol = row["symbol"]
        if not isinstance(symbol, str):
            raise MissingOrMalformed(f"{label} frame row {index} symbol is not text")
        grouped[canonical_symbol(symbol)].append(size)

    functions = [
        {"symbol": symbol, "frames_bytes": sorted(sizes, reverse=True)}
        for symbol, sizes in sorted(grouped.items())
    ]
    return {"frame_count": len(frames), "functions": functions}


def read_inventory(path: Path) -> list[str]:
    try:
        if not path.is_file() or path.is_symlink():
            raise MissingOrMalformed(f"inventory is missing or not regular: {path}")
        lines = path.read_text().splitlines()
    except OSError as error:
        raise MissingOrMalformed(f"cannot read inventory {path}: {error}") from error
    packages: list[str] = []
    for number, line in enumerate(lines, 1):
        fields = line.split("\t")
        if len(fields) != 1 or not re.fullmatch(r"[a-z0-9][a-z0-9-]*", fields[0]):
            raise MissingOrMalformed(f"inventory row {number} is malformed")
        packages.append(fields[0])
    if len(packages) != EXPECTED_LINK_COUNT:
        raise MissingOrMalformed(
            f"inventory is not the exact {EXPECTED_LINK_COUNT}-link set: {len(packages)}"
        )
    if len(set(packages)) != len(packages) or packages != sorted(packages):
        raise MissingOrMalformed("inventory packages are duplicated or not canonical")
    return packages


def atomic_write(path: Path, value: dict[str, Any]) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        encoded = json.dumps(value, indent=2, sort_keys=True) + "\n"
        descriptor, temporary = tempfile.mkstemp(
            prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
        )
        try:
            with os.fdopen(descriptor, "w") as output:
                output.write(encoded)
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, path)
        except BaseException:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass
            raise
    except OSError as error:
        raise MissingOrMalformed(f"cannot atomically write {path}: {error}") from error


def assemble(inventory: Path, reports: Path, commit: str | None) -> dict[str, Any]:
    packages = read_inventory(inventory)
    if not reports.is_dir():
        raise MissingOrMalformed(f"report directory is missing: {reports}")
    links = []
    for package in packages:
        value = canonicalize_report(
            read_json(reports / f"{package}.json", f"{package} frame report"),
            f"{package} frame report",
        )
        links.append({"package": package, **value})
    manifest: dict[str, Any] = {
        "schema": MANIFEST_SCHEMA,
        "bound_bytes": SBPF_V0_FRAME_BYTES,
        "link_count": len(links),
        "links": links,
    }
    if commit is not None:
        manifest[COMMIT_FIELD] = checked_commit(commit, "measured commit")
    return manifest


def validate_manifest(value: Any, label: str, schemas: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict) or value.get("schema") not in schemas:
        raise MissingOrMalformed(f"{label} has no admitted frame manifest schema")
    fields = set(value)
    if fields - {COMMIT_FIELD} != {"schema", "bound_bytes", "link_count", "links"}:
        raise MissingOrMalformed(f"{label} has missing or unknown fields")
    if COMMIT_FIELD in value:
        checked_commit(value[COMMIT_FIELD], f"{label} commit")
    if value["bound_bytes"] != SBPF_V0_FRAME_BYTES:
        raise MissingOrMalformed(f"{label} does not bind the SBPF v0 frame wall")
    links = value["links"]
    if not isinstance(links, list):
        raise MissingOrMalformed(f"{label} links are not a list")
    if value["link_count"] != EXPECTED_LINK_COUNT or len(links) != EXPECTED_LINK_COUNT:
        raise MissingOrMalformed(
            f"{label} is not the exact {EXPECTED_LINK_COUNT}-link manifest"
        )
    packages: list[str] = []
    for link_index, link in enumerate(links):
        if not isinstance(link, dict) or set(link) != {
            "package",
            "frame_count",
            "functions",
        }:
            raise MissingOrMalformed(f"{label} link row {link_index} is malformed")
        package = link["package"]
        if not isinstance(package, str) or not re.fullmatch(
            r"[a-z0-9][a-z0-9-]*", package
        ):
            raise MissingOrMalformed(f"{label} link row {link_index} package is unsafe")
        packages.append(package)
        functions = link["functions"]
        if not isinstance(functions, list) or not functions:
            raise MissingOrMalformed(f"{label} {package} has no function rows")
        symbols: list[str] = []
        total = 0
        for function_index, function in enumerate(functions):
            if not isinstance(function, dict) or set(function) != {
                "symbol",
                "frames_bytes",
            }:
                raise MissingOrMalformed(
                    f"{label} {package} function row {function_index} is malformed"
                )
            symbol = function["symbol"]
            if not isinstance(symbol, str) or canonical_symbol(symbol) != symbol:
                raise MissingOrMalformed(
                    f"{label} {package} function row {function_index} is not canonical"
                )
            symbols.append(symbol)
            sizes = function["frames_bytes"]
            if not isinstance(sizes, list) or not sizes:
                raise MissingOrMalformed(f"{label} {package} {symbol} has no frames")
            checked_sizes = [
                natural_number(size, f"{label} {package} {symbol} frame")
                for size in sizes
            ]
            if checked_sizes != sorted(checked_sizes, reverse=True):
                raise MissingOrMalformed(f"{label} {package} {symbol} frames are unsorted")
            total += len(checked_sizes)
        if symbols != sorted(symbols) or len(symbols) != len(set(symbols)):
            raise MissingOrMalformed(f"{label} {package} symbols are not canonical")
        if natural_number(link["frame_count"], f"{label} {package} frame_count") != total:
            raise MissingOrMalformed(f"{label} {package} frame_count differs from rows")
    if packages != sorted(packages) or len(packages) != len(set(packages)):
        raise MissingOrMalformed(f"{label} package order or identity is not canonical")
    return value


def frames_only(value: dict[str, Any]) -> dict[str, Any]:
    """The comparable content: every measured frame, and nothing about its base."""

    return {key: item for key, item in value.items() if key != COMMIT_FIELD}


def named_base(value: dict[str, Any]) -> str:
    commit = value.get(COMMIT_FIELD)
    return commit if isinstance(commit, str) else "an unnamed source tree"


def differences(before: dict[str, Any], after: dict[str, Any]) -> list[str]:
    """Describe enough exact deltas to diagnose red without flooding a log."""

    messages: list[str] = []
    before_links = {link["package"]: link for link in before["links"]}
    after_links = {link["package"]: link for link in after["links"]}
    for package in sorted(set(before_links) | set(after_links)):
        if package not in before_links:
            messages.append(f"{package}: new unbaselined link")
            continue
        if package not in after_links:
            messages.append(f"{package}: admitted link disappeared")
            continue
        old_functions = {
            row["symbol"]: row["frames_bytes"]
            for row in before_links[package]["functions"]
        }
        new_functions = {
            row["symbol"]: row["frames_bytes"]
            for row in after_links[package]["functions"]
        }
        for symbol in sorted(set(old_functions) | set(new_functions)):
            if symbol not in old_functions:
                messages.append(f"{package}: new function {symbol}")
                continue
            if symbol not in new_functions:
                messages.append(f"{package}: function disappeared {symbol}")
                continue
            old = old_functions[symbol]
            new = new_functions[symbol]
            if old == new:
                continue
            grew = len(new) > len(old) or any(
                current > admitted for current, admitted in zip(new, old)
            )
            direction = "GREW" if grew else "changed/ratcheted"
            messages.append(f"{package}: {direction} {symbol}: {old} -> {new}")
    return messages


def check(baseline_path: Path, candidate_path: Path) -> str:
    baseline = validate_manifest(
        read_json(baseline_path, "frame baseline"), "frame baseline", {BASELINE_SCHEMA}
    )
    candidate = validate_manifest(
        read_json(candidate_path, "candidate manifest"),
        "candidate manifest",
        {MANIFEST_SCHEMA},
    )
    projected = frames_only({**baseline, "schema": MANIFEST_SCHEMA})
    measured = frames_only(candidate)
    if projected != measured:
        delta = differences(projected, measured)
        detail = "\n".join(f"  {line}" for line in delta[:20])
        if len(delta) > 20:
            detail += f"\n  ... and {len(delta) - 20} more differences"
        raise Disagreement(
            "per-function frame manifest differs from the ratchet admitted at "
            f"{named_base(baseline)}"
            + (f":\n{detail}" if detail else "")
        )
    return named_base(baseline)


def accept(first_path: Path, second_path: Path, output: Path) -> str:
    first = validate_manifest(
        read_json(first_path, "first capture"), "first capture", {MANIFEST_SCHEMA}
    )
    second = validate_manifest(
        read_json(second_path, "second capture"), "second capture", {MANIFEST_SCHEMA}
    )
    if first != second:
        delta = differences(frames_only(first), frames_only(second))
        detail = "\n".join(f"  {line}" for line in delta[:20])
        if first.get(COMMIT_FIELD) != second.get(COMMIT_FIELD):
            delta.insert(
                0,
                f"captured at different commits: {named_base(first)}"
                f" then {named_base(second)}",
            )
            detail = "\n".join(f"  {line}" for line in delta[:20])
        raise Disagreement(
            "independent captures disagree; no baseline accepted"
            + (f":\n{detail}" if detail else "")
        )
    commit = first.get(COMMIT_FIELD)
    if commit is None:
        raise MissingOrMalformed(
            "neither capture names the commit it measured; recapture with"
            " `tools/frameguard/run.sh --at <commit> --capture <file>` so the"
            " admitted ratchet has a reviewable base"
        )
    atomic_write(output, {**first, "schema": BASELINE_SCHEMA})
    return checked_commit(commit, "accepted commit")


def git(repo: Path, *arguments: str) -> str:
    try:
        finished = subprocess.run(
            ["git", "-C", str(repo), *arguments],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as error:
        raise MissingOrMalformed(f"cannot run git in {repo}: {error}") from error
    if finished.returncode != 0:
        detail = finished.stderr.strip().splitlines()
        raise MissingOrMalformed(
            "git " + " ".join(arguments) + f" failed in {repo}"
            + (f": {detail[0]}" if detail else "")
        )
    return finished.stdout


def discover_links(repo: Path) -> list[str]:
    """The link set, by the same rule `run.sh` inventories it: a manifest.

    A bare directory is not a link. `programs/dclutch-dealer-sbf` survived its
    crate's deletion as an empty directory full of build leavings, and counting
    it made this report claim thirteen links and one unresolved closure.
    """

    programs = repo / "programs"
    if not programs.is_dir():
        raise MissingOrMalformed(f"no programs directory under {repo}")
    return sorted(
        entry.name
        for entry in programs.iterdir()
        if entry.is_dir() and not entry.is_symlink()
        and (entry / "Cargo.toml").is_file()
    )


def path_dependency_closure(repo: Path, link: str) -> tuple[set[str], str | None]:
    """The in-repo crates compiled into one link, as repository-relative dirs.

    Returns the closure and, when it could not be resolved, the reason -- never
    a silent narrowing. A link whose closure is unknown falls back to its own
    directory, which is the pre-closure predicate: strictly weaker, and said out
    loud rather than presented as an answer.
    """

    fallback = {f"programs/{link}"}
    manifest = repo / "programs" / link / "Cargo.toml"
    if not manifest.is_file():
        return fallback, f"{link} has no Cargo.toml"
    try:
        finished = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--offline",
             "--manifest-path", str(manifest)],
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
    except OSError as error:
        return fallback, f"cargo is not runnable ({error})"
    if finished.returncode != 0:
        detail = finished.stderr.strip().splitlines()
        return fallback, f"cargo metadata failed for {link}" + (
            f": {detail[0]}" if detail else ""
        )
    try:
        metadata = json.loads(finished.stdout)
        resolve = metadata["resolve"]
        nodes = {node["id"]: node for node in resolve["nodes"]}
        manifests = {
            package["id"]: Path(package["manifest_path"])
            for package in metadata["packages"]
        }
        root = resolve["root"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        return fallback, f"cargo metadata for {link} is unreadable: {error}"
    if root is None:
        return fallback, f"cargo metadata for {link} resolves no root package"

    # Walk only THROUGH in-repo packages: a registry crate cannot depend on one
    # of ours, so stopping there is both correct and what keeps this a walk over
    # ~100 packages instead of ~850.
    root_directory = repo.resolve()
    closure: set[str] = set()
    seen: set[str] = set()
    stack = [root]
    while stack:
        identifier = stack.pop()
        if identifier in seen or identifier not in nodes:
            continue
        seen.add(identifier)
        manifest_path = manifests.get(identifier)
        if manifest_path is None:
            continue
        try:
            relative = manifest_path.parent.resolve().relative_to(root_directory)
        except (ValueError, OSError):
            continue
        closure.add(relative.as_posix())
        for dependency in nodes[identifier]["deps"]:
            # DEV-DEPENDENCIES ARE NOT IN THE LINK. `cargo metadata` resolves
            # them anyway, and following them put `programs/dclutch-trading-sbf`
            # and two of its program-test crates inside the CLAIMS closure --
            # which would have accused every trading commit of owing claims
            # rows. Only what the cdylib compiles counts: normal edges, and
            # build edges because a build script's output is a compiler input.
            kinds = dependency.get("dep_kinds")
            if kinds is not None and all(
                kind.get("kind") == "dev" for kind in kinds
            ):
                continue
            stack.append(dependency["pkg"])
    if not closure:
        return fallback, f"{link} resolved an empty closure"
    return closure, None


def reaches(name: str, crate: str) -> bool:
    """Does this changed path feed the compiler for that crate?"""

    prefix = f"{crate}/"
    if not name.startswith(prefix):
        return False
    rest = name[len(prefix):]
    return rest.startswith(f"{CRATE_SOURCE_DIRECTORY}/") or rest in CRATE_BUILD_INPUTS


def owed(repo: Path, since: str | None, baseline_path: Path | None, until: str) -> None:
    """Name the commits that moved a link's sources without carrying frame rows.

    An exact ratchet cannot be recaptured after the fact by a bystander in a
    busy tree: the four-minute double build is longer than the interval between
    program commits, so a correct capture is invalidated before it can be
    reviewed and committed. The rule that fixes it -- a commit that moves a
    frame carries its baseline rows -- is only enforceable if a reader can see
    WHO owes them. That is this mode: the range is named by the baseline's own
    recorded commit, so the gate answers "red, and here is the ledger" instead
    of "red".

    Each debtor is printed with the `Lane:` trailer `tools/lane.sh` writes, so
    the ledger names a LANE and not only a commit. Every lane here commits as
    the same git author; before the trailer existed this output said "who owes
    them" in the docstring and printed the identical name on every row, and on
    2026-09-02 three lanes mis-attributed each other's commits in one
    afternoon. A commit made without the wrapper has no trailer and is printed
    as unattributed rather than guessed at.

    Attribution follows each link's PATH-DEPENDENCY CLOSURE, not just its own
    program crate. A frame moves wherever the compiler's input changed, and in
    this tree that is usually a crate two or three edges down: the +832 bytes on
    claims `prepare_and_execute` came with a codec change, and a program-only
    predicate would have let every such row hide behind a crate boundary.
    """

    if since is None:
        if baseline_path is None:
            raise MissingOrMalformed("owed needs --since or --baseline")
        baseline = validate_manifest(
            read_json(baseline_path, "frame baseline"),
            "frame baseline",
            {BASELINE_SCHEMA},
        )
        since = baseline.get(COMMIT_FIELD)
        if since is None:
            raise MissingOrMalformed(
                f"{baseline_path} names no captured commit, so no range can be"
                " read from it; pass --since <commit> or recapture with --at"
            )
    base = git(repo, "rev-parse", "--verify", "--quiet", f"{since}^{{commit}}").strip()
    head = git(repo, "rev-parse", "--verify", "--quiet", f"{until}^{{commit}}").strip()
    if not base or not head:
        raise MissingOrMalformed(f"{since}..{until} does not name two commits")
    revisions = git(repo, "rev-list", "--reverse", f"{base}..{head}").split()

    # One `cargo metadata` per link, all at once: each is a subprocess that
    # spends its time waiting, and twelve in series is the difference between a
    # red gate that explains itself immediately and one that pauses first.
    links = discover_links(repo)
    if len(links) != EXPECTED_LINK_COUNT:
        print(
            f"frameguard: NOTE -- {len(links)} program manifests, not the"
            f" admitted {EXPECTED_LINK_COUNT}; this ledger covers what it found",
            file=sys.stderr,
        )
    closures: dict[str, set[str]] = {}
    degraded: list[str] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        resolved = list(pool.map(
            lambda link: (link, *path_dependency_closure(repo, link)), links
        ))
    for link, closure, reason in resolved:
        closures[link] = closure
        if reason is not None:
            degraded.append(reason)
    crates = sorted({crate for closure in closures.values() for crate in closure})

    debtors: list[str] = []
    for revision in revisions:
        # No `-m`: a merge commit gets no accusation, because whatever sources
        # it brought in are attributed to the commits that made them, which
        # `rev-list` has already put in this range.
        names = git(
            repo, "diff-tree", "--no-commit-id", "--name-only", "-r", revision
        ).splitlines()
        if BASELINE_ROWS in names:
            continue
        moved = sorted({
            crate for crate in crates
            if any(reaches(name, crate) for name in names)
        })
        if not moved:
            continue
        reached = sorted(
            link for link, closure in closures.items() if closure & set(moved)
        )
        # Subject and lane in ONE `git log`, separated by a NUL that neither
        # can contain. Every lane in this tree commits as the same author, so
        # the author field cannot answer "who owes this" and this ledger --
        # whose whole purpose is naming the debtor -- would otherwise print one
        # identical name beside every row. `tools/lane.sh` writes the trailer.
        recorded = git(
            repo, "log", "-1",
            "--format=%s%x00%(trailers:key=Lane,valueonly,separator=%x2C)",
            revision,
        ).strip("\n")
        subject, _, lane = recorded.partition("\0")
        lane = " ".join(lane.split()) or UNATTRIBUTED
        shown = moved[:4] + ([f"and {len(moved) - 4} more"] if len(moved) > 4 else [])
        debtors.append(
            f"{revision[:8]} [lane {lane}] {subject}\n"
            f"      reaches {', '.join(reached)}\n"
            f"      via {', '.join(shown)}"
        )

    scope = f"{base[:8]}..{head[:8]} ({len(revisions)} commits)"
    coverage = (
        f"{len(links)} links over {len(crates)} first-party crates"
        + (f"; {len(degraded)} closure(s) UNRESOLVED, falling back to the"
           f" program crate alone: {'; '.join(degraded[:3])}" if degraded else "")
    )
    if not debtors:
        print(
            f"frameguard: {scope} -- no commit changed a link's compiled sources"
            f" without carrying its baseline rows ({coverage})"
        )
        return
    raise Disagreement(
        f"{len(debtors)} commit(s) in {scope} changed sources compiled into a"
        f" link and left the frame ratchet to someone else ({coverage}):\n"
        + "\n".join(f"  {line}" for line in debtors)
        + "\n  Each owes a `tools/frameguard/run.sh --at <its commit>` recapture"
        " in its own commit, or an explicit statement that it leaves the ratchet red."
        f"\n  `lane` is the commit's `Lane:` trailer; `{UNATTRIBUTED}` means it was"
        " committed without `tools/lane.sh`,"
        "\n  which every commit before 2026-09-02 was, so an old range says it about"
        " every row."
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    assemble_parser = subparsers.add_parser("assemble")
    assemble_parser.add_argument("--inventory", type=Path, required=True)
    assemble_parser.add_argument("--reports", type=Path, required=True)
    assemble_parser.add_argument("--output", type=Path, required=True)
    assemble_parser.add_argument("--commit", default=None)

    owed_parser = subparsers.add_parser("owed")
    owed_parser.add_argument("--repo", type=Path, required=True)
    owed_parser.add_argument("--since", default=None)
    owed_parser.add_argument("--until", default="HEAD")
    owed_parser.add_argument("--baseline", type=Path, default=None)

    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("--baseline", type=Path, required=True)
    check_parser.add_argument("--candidate", type=Path, required=True)

    accept_parser = subparsers.add_parser("accept")
    accept_parser.add_argument("--first", type=Path, required=True)
    accept_parser.add_argument("--second", type=Path, required=True)
    accept_parser.add_argument("--output", type=Path, required=True)

    arguments = parser.parse_args()
    try:
        if arguments.command == "assemble":
            atomic_write(
                arguments.output,
                assemble(arguments.inventory, arguments.reports, arguments.commit),
            )
        elif arguments.command == "check":
            base = check(arguments.baseline, arguments.candidate)
            print(
                "frameguard: complete per-function frame manifest matches the"
                f" ratchet admitted at {base}"
            )
        elif arguments.command == "owed":
            owed(arguments.repo, arguments.since, arguments.baseline, arguments.until)
        else:
            base = accept(arguments.first, arguments.second, arguments.output)
            print(
                "frameguard: accepted two identical independent captures of"
                f" {base} at {arguments.output}"
            )
        return 0
    except Disagreement as error:
        print(f"frameguard: REFUSING -- {error}", file=sys.stderr)
        return 1
    except MissingOrMalformed as error:
        print(f"frameguard: COULD NOT RUN -- {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
