#!/usr/bin/env python3
"""Plan one exact all-link SBF batch before spending hbox build time.

The plan compares the local dependency closure of every `programs/*` package
between two committed source trees.  It predicts which of the exact shipped
links need new content-addressed artifacts; it never builds an ELF and is not
release evidence by itself.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
from typing import Any, Iterable


SCHEMA = "dclutch-sbf-release-batch-plan-v1"
# The shipped set is defined ONCE, in artifact_provenance, and this module used
# to restate its SIZE as the literal 13. `e6b7bf1a` deleted `dclutch-dealer-sbf`
# and took the set to twelve, and from that commit until this one the pre-freeze
# forecast the release runbook tells an operator to run refused every tree with
# "program manifest inventory is not exact 13-link set: 12". Count the set from
# the set: this module already names every artifact-producing role, and the
# frame-gate-only packages are named here rather than counted, so adding or
# deleting a program changes one obvious line instead of going stale in silence.
ARTIFACT_ROLES = {
    "core": ("dclutch-core-sbf", "dclutch_core_sbf"),
    "claims": ("dclutch-claims-sbf", "dclutch_claims_sbf"),
    "trading": ("dclutch-trading-sbf", "dclutch_trading_sbf"),
    "resolution": ("dclutch-resolution-proof-sbf", "dclutch_resolution_proof_sbf"),
    "custody": ("dclutch-custody-sbf", "dclutch_custody_sbf"),
    "registry": ("dclutch-registry-sbf", "dclutch_registry_sbf"),
    "rent": ("dclutch-rent-sbf", "dclutch_rent_sbf"),
    "accelerator": ("dclutch-accelerator-sbf", "dclutch_accelerator_sbf"),
}
# Empty since 2026-09-04: the two frame-gate-only links (`dclutch-direct-aot-sbf`,
# `dclutch-product-runtime-v2-sbf`) were deleted with their bands retired.
FRAME_GATE_ONLY_PACKAGES: tuple[str, ...] = ()
EXPECTED_LINK_COUNT = len(ARTIFACT_ROLES) + len(FRAME_GATE_ONLY_PACKAGES)
GLOBAL_INPUTS = {
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rust-toolchain",
}
GLOBAL_PREFIXES = (".cargo/",)


class Refusal(RuntimeError):
    pass


def run(*arguments: str, cwd: Path | None = None) -> bytes:
    try:
        return subprocess.run(
            arguments,
            cwd=cwd,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ).stdout
    except subprocess.CalledProcessError as error:
        detail = error.stderr.decode(errors="replace").strip()
        raise Refusal(f"command refused ({' '.join(arguments)}): {detail}") from error


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def resolve(repo: Path, revision: str) -> str:
    value = (
        run("git", "-C", str(repo), "rev-parse", f"{revision}^{{commit}}")
        .decode()
        .strip()
    )
    if len(value) != 40:
        raise Refusal(f"revision did not resolve to a full commit: {revision}")
    return value


def source_tree(repo: Path, revision: str) -> tuple[str, list[str]]:
    listing = run("git", "-C", str(repo), "ls-tree", "-r", "--full-tree", revision)
    names = (
        run(
            "git",
            "-C",
            str(repo),
            "ls-tree",
            "-r",
            "--name-only",
            "--full-tree",
            revision,
        )
        .decode()
        .splitlines()
    )
    return sha256(listing), names


def archive(repo: Path, revision: str, destination: Path) -> None:
    raw = run("git", "-C", str(repo), "archive", revision)
    with tarfile.open(fileobj=io.BytesIO(raw)) as source:
        source.extractall(destination, filter="data")


def metadata(source: Path) -> dict[str, Any]:
    raw = run(
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--locked",
        "--offline",
        "--manifest-path",
        str(source / "Cargo.toml"),
        cwd=source,
    )
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        raise Refusal(f"cargo metadata was not JSON: {error}") from error
    if not isinstance(value, dict) or not isinstance(value.get("packages"), list):
        raise Refusal("cargo metadata package inventory is malformed")
    return value


def package_map(source: Path, value: dict[str, Any]) -> dict[str, dict[str, Any]]:
    packages: dict[str, dict[str, Any]] = {}
    for package in value["packages"]:
        name = package.get("name")
        manifest = Path(package.get("manifest_path", ""))
        try:
            manifest.relative_to(source)
        except ValueError:
            continue
        if not isinstance(name, str) or name in packages:
            raise Refusal(
                f"local Cargo package name is missing or duplicated: {name!r}"
            )
        packages[name] = package
    return packages


def link_inventory(source: Path) -> list[tuple[str, str, str | None]]:
    by_package = {
        package: (role, stem) for role, (package, stem) in ARTIFACT_ROLES.items()
    }
    rows: list[tuple[str, str, str | None]] = []
    for manifest in sorted((source / "programs").glob("*/Cargo.toml")):
        package = manifest.parent.name
        role = by_package.get(package)
        rows.append((role[0] if role else package, package, role[1] if role else None))
    if len(rows) != EXPECTED_LINK_COUNT or len({row[0] for row in rows}) != len(rows):
        raise Refusal(
            f"program manifest inventory is not exact {EXPECTED_LINK_COUNT}-link set: {len(rows)}"
        )
    missing = sorted(
        package
        for package, _ in ARTIFACT_ROLES.values()
        if package not in {r[1] for r in rows}
    )
    if missing:
        raise Refusal(
            f"permanent release roles are absent from link inventory: {missing}"
        )
    return rows


def dependency_closure(packages: dict[str, dict[str, Any]], root: str) -> set[str]:
    if root not in packages:
        raise Refusal(f"program package absent from Cargo metadata: {root}")
    closure: set[str] = set()
    pending = [root]
    while pending:
        name = pending.pop()
        if name in closure:
            continue
        closure.add(name)
        for dependency in packages[name].get("dependencies", []):
            if dependency.get("kind") == "dev":
                continue
            target = dependency.get("name")
            if target in packages and target not in closure:
                pending.append(target)
    return closure


def closure_paths(
    source: Path,
    tracked: Iterable[str],
    packages: dict[str, dict[str, Any]],
    root: str,
) -> list[str]:
    directories: list[str] = []
    for name in dependency_closure(packages, root):
        manifest = Path(packages[name]["manifest_path"])
        relative = manifest.parent.relative_to(source).as_posix()
        directories.append(relative + "/")
    selected = []
    for path in tracked:
        if (
            path in GLOBAL_INPUTS
            or path.startswith(GLOBAL_PREFIXES)
            or path.startswith(tuple(directories))
        ):
            selected.append(path)
    return sorted(set(selected))


def closure_digest(source: Path, paths: Iterable[str]) -> str:
    digest = hashlib.sha256()
    for path in paths:
        raw = (source / path).read_bytes()
        encoded = path.encode()
        digest.update(len(encoded).to_bytes(8, "little"))
        digest.update(encoded)
        digest.update(len(raw).to_bytes(8, "little"))
        digest.update(raw)
    return digest.hexdigest()


def changed_paths(
    base_root: Path,
    candidate_root: Path,
    base_paths: set[str],
    candidate_paths: set[str],
) -> list[str]:
    changed = []
    for path in sorted(base_paths | candidate_paths):
        before = (base_root / path).read_bytes() if path in base_paths else None
        after = (
            (candidate_root / path).read_bytes() if path in candidate_paths else None
        )
        if before != after:
            changed.append(path)
    return changed


def plan(repo: Path, base: str, candidate: str) -> dict[str, Any]:
    repo = repo.resolve(strict=True)
    base_revision = resolve(repo, base)
    candidate_revision = resolve(repo, candidate)
    base_tree, base_tracked = source_tree(repo, base_revision)
    candidate_tree, candidate_tracked = source_tree(repo, candidate_revision)
    with tempfile.TemporaryDirectory(prefix="dclutch-sbf-batch-plan.") as temporary:
        root = Path(temporary)
        base_root = root / "base"
        candidate_root = root / "candidate"
        base_root.mkdir()
        candidate_root.mkdir()
        archive(repo, base_revision, base_root)
        archive(repo, candidate_revision, candidate_root)
        base_packages = package_map(base_root, metadata(base_root))
        candidate_packages = package_map(candidate_root, metadata(candidate_root))
        base_inventory = link_inventory(base_root)
        inventory = link_inventory(candidate_root)
        if [(row[0], row[1]) for row in base_inventory] != [
            (row[0], row[1]) for row in inventory
        ]:
            raise Refusal(
                "base and candidate do not share one exact shipped-link identity set"
            )
        links = []
        for label, package, artifact_stem in inventory:
            before_paths = set(
                closure_paths(base_root, base_tracked, base_packages, package)
            )
            after_paths = set(
                closure_paths(
                    candidate_root, candidate_tracked, candidate_packages, package
                )
            )
            changes = changed_paths(
                base_root, candidate_root, before_paths, after_paths
            )
            links.append(
                {
                    "label": label,
                    "package": package,
                    "artifact_stem": artifact_stem,
                    "base_input_digest": closure_digest(
                        base_root, sorted(before_paths)
                    ),
                    "candidate_input_digest": closure_digest(
                        candidate_root, sorted(after_paths)
                    ),
                    "requires_new_artifact": bool(changes),
                    "changed_inputs": changes,
                    "consumers": [
                        "plain-sbf-diagnostics",
                        "frame-diagnostics",
                        "import-symbol-audit",
                        "caller-proof",
                        "cu-20-seed"
                        if label in {"trading", "resolution"}
                        else "not-applicable",
                    ],
                }
            )
    return {
        "schema": SCHEMA,
        "base_revision": base_revision,
        "base_source_tree_sha256": base_tree,
        "candidate_revision": candidate_revision,
        "candidate_source_tree_sha256": candidate_tree,
        "link_count": len(links),
        "changed_link_count": sum(
            bool(link["requires_new_artifact"]) for link in links
        ),
        "links": links,
        "qualification": (
            "Static source-closure forecast only. Build each changed link once at candidate_revision; "
            "bind every downstream consumer to its exact artifact provenance descriptor."
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=".")
    parser.add_argument("--base", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output")
    arguments = parser.parse_args()
    try:
        value = plan(Path(arguments.repo), arguments.base, arguments.candidate)
        encoded = json.dumps(value, indent=2, sort_keys=True) + "\n"
        if arguments.output:
            output = Path(arguments.output)
            if output.exists() or output.is_symlink():
                raise Refusal(f"output already exists: {output}")
            output.write_text(encoded)
        else:
            print(encoded, end="")
        return 0
    except (OSError, Refusal) as error:
        print(f"SBF BATCH PLAN REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
