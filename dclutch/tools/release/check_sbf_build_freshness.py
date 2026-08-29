#!/usr/bin/env python3
"""Refuse SBF diagnostic evidence that did not freshly compile every link."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import stat
import sys
from typing import NoReturn


SAFE_NAME = re.compile(r"^[a-z0-9][a-z0-9_-]*$")
RUN_ID = re.compile(r"^[0-9a-f]{64}$")


def refuse(message: str) -> NoReturn:
    print(f"SBF BUILD FRESHNESS REFUSED: {message}", file=sys.stderr)
    raise SystemExit(1)


def regular_file(path: Path, label: str) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        refuse(f"missing {label}: {path}")
    if not stat.S_ISREG(mode):
        refuse(f"{label} is not a regular file: {path}")


def read_lines(path: Path, label: str) -> list[str]:
    regular_file(path, label)
    try:
        data = path.read_bytes()
        text = data.decode("utf-8")
    except (OSError, UnicodeDecodeError) as error:
        refuse(f"could not read {label} as UTF-8: {path}: {error}")
    if not data or not text.endswith("\n"):
        refuse(f"{label} must be nonempty and newline-terminated: {path}")
    return text.splitlines()


def parse_expected(path: Path) -> list[tuple[str, str]]:
    rows: list[tuple[str, str]] = []
    labels: set[str] = set()
    packages: set[str] = set()
    for number, line in enumerate(read_lines(path, "expected-link manifest"), 1):
        fields = line.split("\t")
        if len(fields) != 2:
            refuse(f"expected-link row {number} is not label<TAB>package")
        label, package = fields
        if not SAFE_NAME.fullmatch(label) or not SAFE_NAME.fullmatch(package):
            refuse(f"expected-link row {number} has an unsafe label or package")
        if label in labels:
            refuse(f"duplicate expected label: {label}")
        if package in packages:
            refuse(f"duplicate expected package: {package}")
        labels.add(label)
        packages.add(package)
        rows.append((label, package))
    if not rows:
        refuse("expected-link manifest is empty")
    return rows


def parse_diagnostics(path: Path) -> dict[str, int]:
    diagnostics: dict[str, int] = {}
    for number, line in enumerate(read_lines(path, "diagnostics manifest"), 1):
        if line.count("=") != 1:
            refuse(f"diagnostics row {number} is not label=count")
        label, count_text = line.split("=", 1)
        if not SAFE_NAME.fullmatch(label) or not count_text.isascii() or not count_text.isdigit():
            refuse(f"diagnostics row {number} has an unsafe label or count")
        if label in diagnostics:
            refuse(f"duplicate diagnostics label: {label}")
        diagnostics[label] = int(count_text)
    return diagnostics


def has_top_package_compile(lines: list[str], package: str) -> bool:
    marker = re.compile(rf"^\s*Compiling\s+{re.escape(package)}\s+v\S+(?:\s|$)")
    return any(marker.search(line) is not None for line in lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", required=True)
    parser.add_argument("--expected", required=True)
    parser.add_argument("--diagnostics", required=True)
    parser.add_argument("--run-id", required=True)
    args = parser.parse_args()

    if not os.path.isabs(args.work):
        refuse("--work must be absolute")
    if not RUN_ID.fullmatch(args.run_id):
        refuse("--run-id must be 64 lowercase hexadecimal characters")

    work = Path(args.work)
    expected = parse_expected(Path(args.expected))
    diagnostics = parse_diagnostics(Path(args.diagnostics))
    expected_labels = {label for label, _ in expected}
    if set(diagnostics) != expected_labels:
        missing = sorted(expected_labels - set(diagnostics))
        extra = sorted(set(diagnostics) - expected_labels)
        refuse(f"diagnostics labels differ: missing={missing} extra={extra}")

    header = f"dclutch-sbf-build-run-v1={args.run_id}"
    invocation_prefix = "dclutch-sbf-build-invocation-v1="
    for label, package in expected:
        log = work / f"build-{label}.log"
        lines = read_lines(log, f"build log for {label}")
        if lines[0] != header:
            refuse(f"build log for {label} belongs to a different or unstamped run")
        if len(lines) < 3 or not lines[1].startswith(invocation_prefix) or len(lines[1]) == len(invocation_prefix):
            refuse(f"build log for {label} omitted its exact invocation stamp")
        if not has_top_package_compile(lines[2:], package):
            refuse(f"build log for {label} has no fresh top-package compile marker for {package}")

    print(f"SBF build freshness PASS links={len(expected)}")


if __name__ == "__main__":
    main()
