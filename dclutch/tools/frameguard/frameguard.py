#!/usr/bin/env python3
"""Canonicalize and ratchet complete per-function SBF frame reports.

`sbf-frame-sizes.py --format json` reports the raw symbols in one fresh object.
This tool removes only compiler-generated identity hashes, preserves colliding
monomorphizations as a sorted multiset, and joins all twelve program objects
into one canonical manifest. `check` requires that manifest to equal the
committed baseline exactly: growth is red, and shrinkage is also red until the
baseline is ratcheted down, so recovered headroom cannot silently be spent
again.

A baseline is accepted only from two independently captured manifests. Object
hashes are intentionally absent: equality of the complete canonical function
map, not a lucky build artifact hash, is the repeatability witness.

Exit 0 means the comparison ran and agreed. Exit 1 means measured frames or
the two acceptance runs disagree. Exit 2 means an input/prerequisite was
missing or malformed, so no frame claim was made.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
import json
import os
from pathlib import Path
import re
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
# BOTH halves.
EXPECTED_LINK_COUNT = 12
SBPF_V0_FRAME_BYTES = 4096

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


def assemble(inventory: Path, reports: Path) -> dict[str, Any]:
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
    return {
        "schema": MANIFEST_SCHEMA,
        "bound_bytes": SBPF_V0_FRAME_BYTES,
        "link_count": len(links),
        "links": links,
    }


def validate_manifest(value: Any, label: str, schemas: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict) or value.get("schema") not in schemas:
        raise MissingOrMalformed(f"{label} has no admitted frame manifest schema")
    if set(value) != {"schema", "bound_bytes", "link_count", "links"}:
        raise MissingOrMalformed(f"{label} has missing or unknown fields")
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


def check(baseline_path: Path, candidate_path: Path) -> None:
    baseline = validate_manifest(
        read_json(baseline_path, "frame baseline"), "frame baseline", {BASELINE_SCHEMA}
    )
    candidate = validate_manifest(
        read_json(candidate_path, "candidate manifest"),
        "candidate manifest",
        {MANIFEST_SCHEMA},
    )
    projected = {**baseline, "schema": MANIFEST_SCHEMA}
    if projected != candidate:
        delta = differences(projected, candidate)
        detail = "\n".join(f"  {line}" for line in delta[:20])
        if len(delta) > 20:
            detail += f"\n  ... and {len(delta) - 20} more differences"
        raise Disagreement(
            "per-function frame manifest differs from the admitted ratchet"
            + (f":\n{detail}" if detail else "")
        )


def accept(first_path: Path, second_path: Path, output: Path) -> None:
    first = validate_manifest(
        read_json(first_path, "first capture"), "first capture", {MANIFEST_SCHEMA}
    )
    second = validate_manifest(
        read_json(second_path, "second capture"), "second capture", {MANIFEST_SCHEMA}
    )
    if first != second:
        delta = differences(first, second)
        detail = "\n".join(f"  {line}" for line in delta[:20])
        raise Disagreement(
            "independent captures disagree; no baseline accepted"
            + (f":\n{detail}" if detail else "")
        )
    atomic_write(output, {**first, "schema": BASELINE_SCHEMA})


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    assemble_parser = subparsers.add_parser("assemble")
    assemble_parser.add_argument("--inventory", type=Path, required=True)
    assemble_parser.add_argument("--reports", type=Path, required=True)
    assemble_parser.add_argument("--output", type=Path, required=True)

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
            atomic_write(arguments.output, assemble(arguments.inventory, arguments.reports))
        elif arguments.command == "check":
            check(arguments.baseline, arguments.candidate)
            print("frameguard: complete per-function frame manifest matches the ratchet")
        else:
            accept(arguments.first, arguments.second, arguments.output)
            print(f"frameguard: accepted two identical independent captures at {arguments.output}")
        return 0
    except Disagreement as error:
        print(f"frameguard: REFUSING -- {error}", file=sys.stderr)
        return 1
    except MissingOrMalformed as error:
        print(f"frameguard: COULD NOT RUN -- {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
