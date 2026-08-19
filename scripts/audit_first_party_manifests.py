#!/usr/bin/env python3
"""Refuse a first-party Cargo package without the repository license.

The audit includes tracked and non-ignored untracked manifests so a newly added
crate cannot evade the check before its first commit.  Vendored third-party
source is excluded deliberately: its upstream license must be preserved rather
than rewritten to the repository's outbound license.
"""

from __future__ import annotations

import subprocess
import sys
import tomllib
from pathlib import Path


EXPECTED_LICENSE = "AGPL-3.0-or-later"
EXCLUDED_PARTS = {".git", "target", "vendor", "node_modules"}


def repository_root() -> Path:
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(result.stdout.strip()).resolve()


def candidate_manifests(root: Path) -> list[Path]:
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "**/Cargo.toml",
            "Cargo.toml",
        ],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    paths: list[Path] = []
    for raw in result.stdout.splitlines():
        relative = Path(raw)
        if EXCLUDED_PARTS.intersection(relative.parts):
            continue
        paths.append(root / relative)
    return sorted(set(paths))


def parse_manifest(path: Path) -> dict[str, object]:
    try:
        with path.open("rb") as source:
            return tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"cannot parse {path}: {error}") from error


def inherited_license(path: Path, root: Path) -> str | None:
    parent = path.parent
    while parent == root or root in parent.parents:
        candidate = parent / "Cargo.toml"
        if candidate != path and candidate.is_file():
            manifest = parse_manifest(candidate)
            workspace = manifest.get("workspace")
            if isinstance(workspace, dict):
                package = workspace.get("package")
                if isinstance(package, dict):
                    license_value = package.get("license")
                    if isinstance(license_value, str):
                        return license_value
        if parent == root:
            break
        parent = parent.parent
    return None


def package_license(manifest: dict[str, object], path: Path, root: Path) -> str | None:
    package = manifest.get("package")
    if not isinstance(package, dict):
        return None
    value = package.get("license")
    if isinstance(value, str):
        return value
    if isinstance(value, dict) and value.get("workspace") is True:
        return inherited_license(path, root)
    return "<missing>"


def main() -> int:
    root = repository_root()
    manifests = candidate_manifests(root)
    package_count = 0
    failures: list[str] = []
    for path in manifests:
        try:
            manifest = parse_manifest(path)
            license_value = package_license(manifest, path, root)
        except ValueError as error:
            failures.append(str(error))
            continue
        if license_value is None:
            continue
        package_count += 1
        if license_value != EXPECTED_LICENSE:
            relative = path.relative_to(root)
            failures.append(
                f"{relative}: expected {EXPECTED_LICENSE!r}, found {license_value!r}"
            )

    if failures:
        for failure in failures:
            print(f"manifest-license error: {failure}", file=sys.stderr)
        return 1
    print(
        "manifest-license PASS: "
        f"{package_count} first-party packages declare or inherit {EXPECTED_LICENSE}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
