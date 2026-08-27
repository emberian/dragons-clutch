#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Offline dependency/license closure check with an attested default scope.

The default mode is the in-repo original of the checker the Persvati portable
attestation jobs run as ``dependency_license_check.py`` (fresh job
``dragons-clutch-final-portable-attest-6743b9d-20260819-TChWnu``): a fixed
twelve-manifest locked scope whose record grammar, iteration order, and
SUMMARY line are byte-stable.  At the ``6743b9d`` archive it reproduces the
attested ``SUMMARY manifests=12 unique_rows=888 failures=0 status=PASS``
byte-for-byte.  Do not reorder, extend, or reformat the default scope or its
output; the attestation transcript comparison depends on it.

``--complete`` is the release SBOM/license closure extension: it discovers
every tracked ``Cargo.lock`` in the repository (crates, programs, research,
tools, toolchain probes, the vendored crate) plus every tracked
``package.json``, applies the same rules per package — resolvable offline
from the lock, registry checksum present, license expression or declared
license file present, path dependencies inside the repository, no git or
unknown sources — and writes a deduplicated SBOM-style TSV
(name, version, source, checksum, license).  Failures are printed as
``FAILURE`` rows and turn the SUMMARY to STOP; they are never suppressed.

A lock under the vendored-source prefix is not checked standalone: cargo
refuses to process the vendored manifest outside its vendoring workspace, and
adding a ``[workspace]`` table to it would break the vendor byte-identity
audit in ``programs/clutch-sbf/audit/audit_artifact.sh``.  Its packages are
covered through the vendoring workspace's rows; the disposition is printed as
an explicit ``MANIFEST … VENDORED covered-by=…`` record, and it is a
``FAILURE`` if the covering workspace is not itself in the checked scope.

The complete mode is a declared baseline-manifest gate
(``python.dependency_license_complete``): it runs ``--complete`` and then
requires byte-equality against the committed catalog, so a crate added without
regenerating the catalog goes red. The attested twelve-manifest default mode
above is a separate byte-stable surface and is deliberately not that gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys
import tomllib


# The exact locked-manifest scope of the portable attestation methodology.
# Byte-stability contract: this list, its order, and ATTESTED_LOCK_OVERRIDES
# may only change together with a new attestation methodology revision.
ATTESTED_MANIFESTS = [
    "crates/clutch-bspline/Cargo.toml",
    "crates/clutch-bspline-accumulator/Cargo.toml",
    "research/bspline-shape-compiler/Cargo.toml",
    "programs/solana-layout/Cargo.toml",
    "research/batch-policy-identity/Cargo.toml",
    "research/resolution-work-v1/Cargo.toml",
    "research/liquidity-policy-model/Cargo.toml",
    "research/source-profile-v1/Cargo.toml",
    "crates/clutch-liveness/Cargo.toml",
    "research/liveness-policy-profile/Cargo.toml",
    "programs/clutch-sbf/program/Cargo.toml",
    "programs/clutch-sbf/svm-tests/Cargo.toml",
]
ATTESTED_LOCK_OVERRIDES = {
    "programs/clutch-sbf/program/Cargo.toml": "programs/clutch-sbf/Cargo.lock",
}

EXPECTED_FIRST_PARTY_LICENSE = "AGPL-3.0-or-later"
VENDOR_PREFIX = "programs/clutch-sbf/vendor/"
VENDOR_COVERING_WORKSPACE = "programs/clutch-sbf/Cargo.toml"
EXCLUDED_TREE_PARTS = {".git", "target", "node_modules", "vendor-cache"}
DEFAULT_SBOM_REL = "research/liveness-policy-profile/dependency_license_complete.tsv"
SBOM_HEADER = "name\tversion\tsource\tchecksum\tlicense"

Row = tuple[str, str, str, str, str, str]


def license_identity(
    package: dict, manifest: str, failures: list[str]
) -> str | None:
    """Return the package's SPDX expression or a digest-pinned license file id."""
    license_id = package.get("license")
    if license_id:
        return license_id
    license_file = package.get("license_file")
    if not license_file:
        failures.append(
            f"{manifest}: missing license and license_file: "
            f"{package['name']} {package['version']}"
        )
        return None
    license_path = (
        pathlib.Path(package["manifest_path"]).resolve().parent / license_file
    )
    if not license_path.is_file():
        failures.append(
            f"{manifest}: absent declared license_file: "
            f"{package['name']} {license_file}"
        )
        return None
    license_digest = hashlib.sha256(license_path.read_bytes()).hexdigest()
    return f"LicenseRef-file:{license_file}:sha256={license_digest}"


def classify_cargo_package(
    package: dict,
    locked: dict[tuple[str, str, str | None], str | None],
    root: pathlib.Path,
    manifest: str,
    failures: list[str],
) -> Row | None:
    """Classify one resolved package into an SBOM row or a recorded failure."""
    license_id = license_identity(package, manifest, failures)
    if license_id is None:
        return None
    source = package.get("source")
    if source is None:
        package_root = pathlib.Path(package["manifest_path"]).resolve().parent
        try:
            rel = package_root.relative_to(root).as_posix()
        except ValueError:
            failures.append(
                f"{manifest}: path dependency outside archive: {package_root}"
            )
            return None
        checksum = "-"
        if rel.startswith(VENDOR_PREFIX):
            source_norm = f"vendored-path+{rel}"
        else:
            source_norm = f"path+{rel}"
            if license_id != EXPECTED_FIRST_PARTY_LICENSE:
                failures.append(
                    f"{manifest}: unexpected first-party license: {rel}: {license_id}"
                )
                return None
    elif source.startswith("registry+"):
        source_norm = source
        checksum = locked.get((package["name"], package["version"], source))
        if not checksum:
            failures.append(
                f"{manifest}: registry lock lacks checksum: "
                f"{package['name']} {package['version']}"
            )
            return None
    else:
        failures.append(
            f"{manifest}: forbidden dependency source: {package['name']} {source}"
        )
        return None
    return (
        manifest,
        package["name"],
        package["version"],
        source_norm,
        checksum,
        license_id,
    )


def check_cargo_manifest(
    root: pathlib.Path,
    manifest: str,
    lock_rel: str,
    rows: list[Row],
    failures: list[str],
) -> None:
    """One attestation-grammar manifest record; identical to the attested loop body."""
    manifest_path = root / manifest
    lock_path = root / lock_rel
    try:
        lock = tomllib.loads(lock_path.read_text())
        locked = {
            (p["name"], p["version"], p.get("source")): p.get("checksum")
            for p in lock["package"]
        }
        proc = subprocess.run(
            [
                "cargo", "metadata", "--format-version", "1", "--locked", "--offline",
                "--manifest-path", str(manifest_path),
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if proc.returncode != 0:
            detail = " | ".join(
                line.strip() for line in proc.stderr.splitlines() if line.strip()
            )
            failures.append(
                f"{manifest}: cargo metadata rc={proc.returncode}: {detail}"
            )
            print(f"MANIFEST\t{manifest}\tSTOP\tmetadata-locked")
            return
        data = json.loads(proc.stdout)
        manifest_rows = 0
        for package in data["packages"]:
            row = classify_cargo_package(package, locked, root, manifest, failures)
            if row is None:
                continue
            rows.append(row)
            manifest_rows += 1
        print(
            f"MANIFEST\t{manifest}\tPASS\tpackages={manifest_rows}"
            f"\tlock={lock_path.relative_to(root)}"
        )
    except Exception as exc:
        failures.append(f"{manifest}: checker exception: {type(exc).__name__}: {exc}")
        print(f"MANIFEST\t{manifest}\tSTOP\tchecker-exception")


def npm_declared_dependencies(data: dict) -> dict[str, str]:
    merged: dict[str, str] = {}
    for section in (
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
    ):
        table = data.get(section)
        if isinstance(table, dict):
            merged.update(table)
    return merged


def check_npm_manifest(
    root: pathlib.Path,
    manifest: str,
    rows: list[Row],
    failures: list[str],
) -> None:
    """Mirror the cargo rules for one package.json: locked, checksummed, licensed."""
    manifest_path = root / manifest
    try:
        data = json.loads(manifest_path.read_text())
        package_dir = pathlib.PurePosixPath(manifest).parent
        name = data.get("name") or str(package_dir)
        version = data.get("version", "0.0.0")
        license_id = data.get("license")
        manifest_rows = 0
        if not license_id:
            failures.append(f"{manifest}: missing license: {name} {version}")
        else:
            if license_id != EXPECTED_FIRST_PARTY_LICENSE:
                failures.append(
                    f"{manifest}: unexpected first-party license: "
                    f"{package_dir}: {license_id}"
                )
            else:
                rows.append(
                    (manifest, name, version, f"path+{package_dir}", "-", license_id)
                )
                manifest_rows += 1
        declared = npm_declared_dependencies(data)
        lock_path = manifest_path.parent / "package-lock.json"
        if declared and not lock_path.is_file():
            for dep_name, requirement in sorted(declared.items()):
                failures.append(
                    f"{manifest}: npm dependency has no lockfile: "
                    f"{dep_name} {requirement}"
                )
            print(f"MANIFEST\t{manifest}\tSTOP\tnpm-unlocked")
            return
        if declared:
            lock = json.loads(lock_path.read_text())
            packages = lock.get("packages")
            if not isinstance(packages, dict):
                failures.append(
                    f"{manifest}: unsupported npm lockfile (no packages map)"
                )
                print(f"MANIFEST\t{manifest}\tSTOP\tnpm-lock-schema")
                return
            for entry_path, entry in sorted(packages.items()):
                if entry_path == "":
                    continue
                dep_name = entry.get("name") or entry_path.rpartition(
                    "node_modules/"
                )[2]
                dep_version = entry.get("version", "?")
                resolved = entry.get("resolved", "")
                integrity = entry.get("integrity")
                if entry.get("link"):
                    continue
                if not resolved.startswith("https://registry.npmjs.org/"):
                    failures.append(
                        f"{manifest}: forbidden npm source: {dep_name} "
                        f"{resolved or '<unresolved>'}"
                    )
                    continue
                if not integrity:
                    failures.append(
                        f"{manifest}: npm lock lacks integrity: "
                        f"{dep_name} {dep_version}"
                    )
                    continue
                dep_license = None
                dep_manifest = (
                    manifest_path.parent / entry_path / "package.json"
                )
                if dep_manifest.is_file():
                    dep_license = json.loads(dep_manifest.read_text()).get("license")
                if not dep_license:
                    failures.append(
                        f"{manifest}: npm license unresolvable offline: "
                        f"{dep_name} {dep_version}"
                    )
                    continue
                rows.append(
                    (
                        manifest,
                        dep_name,
                        dep_version,
                        "npm+https://registry.npmjs.org",
                        integrity,
                        dep_license,
                    )
                )
                manifest_rows += 1
        print(f"MANIFEST\t{manifest}\tPASS\tpackages={manifest_rows}\tlock=-")
    except Exception as exc:
        failures.append(f"{manifest}: checker exception: {type(exc).__name__}: {exc}")
        print(f"MANIFEST\t{manifest}\tSTOP\tchecker-exception")


def tracked_files(root: pathlib.Path, pattern: str) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--", pattern],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    kept = []
    for line in result.stdout.splitlines():
        if EXCLUDED_TREE_PARTS.intersection(pathlib.PurePosixPath(line).parts):
            continue
        kept.append(line)
    return sorted(kept)


def discover_complete_specs(root: pathlib.Path) -> list[tuple[str, str, str]]:
    """Every tracked Cargo.lock (paired with its sibling manifest) plus package.json."""
    specs: list[tuple[str, str, str]] = []
    for lock_rel in tracked_files(root, "*Cargo.lock"):
        manifest_rel = str(pathlib.PurePosixPath(lock_rel).parent / "Cargo.toml")
        kind = "vendored" if lock_rel.startswith(VENDOR_PREFIX) else "cargo"
        specs.append((manifest_rel, lock_rel, kind))
    for manifest_rel in tracked_files(root, "*package.json"):
        specs.append((manifest_rel, "", "npm"))
    return specs


def record_vendored_manifest(
    manifest: str,
    specs: list[tuple[str, str, str]],
    failures: list[str],
) -> None:
    """A vendored lock is covered through its workspace, never checked standalone."""
    covered = any(
        spec_manifest == VENDOR_COVERING_WORKSPACE and kind == "cargo"
        for spec_manifest, _, kind in specs
    )
    if not covered:
        failures.append(
            f"{manifest}: vendored lock has no covering workspace in scope: "
            f"{VENDOR_COVERING_WORKSPACE}"
        )
        print(f"MANIFEST\t{manifest}\tSTOP\tvendored-uncovered")
        return
    print(f"MANIFEST\t{manifest}\tVENDORED\tcovered-by={VENDOR_COVERING_WORKSPACE}")


def write_sbom(path: pathlib.Path, rows: list[Row]) -> int:
    unique = sorted({row[1:] for row in rows})
    path.write_text(
        SBOM_HEADER + "\n" + "".join("\t".join(entry) + "\n" for entry in unique)
    )
    return len(unique)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "root",
        nargs="?",
        default=str(pathlib.Path(__file__).resolve().parents[1]),
        help="tree to check (defaults to the repository containing this script)",
    )
    parser.add_argument(
        "--complete",
        action="store_true",
        help="check every tracked Cargo.lock and package.json and write the SBOM TSV",
    )
    parser.add_argument(
        "--sbom-out",
        default=None,
        help=f"complete-mode SBOM TSV path (default: <root>/{DEFAULT_SBOM_REL})",
    )
    args = parser.parse_args(argv)
    root = pathlib.Path(args.root).resolve()

    if args.complete:
        specs = discover_complete_specs(root)
    else:
        specs = [
            (
                manifest,
                ATTESTED_LOCK_OVERRIDES.get(
                    manifest,
                    str(pathlib.PurePosixPath(manifest).parent / "Cargo.lock"),
                ),
                "cargo",
            )
            for manifest in ATTESTED_MANIFESTS
        ]

    rows: list[Row] = []
    failures: list[str] = []
    for manifest, lock_rel, kind in specs:
        if kind == "cargo":
            check_cargo_manifest(root, manifest, lock_rel, rows, failures)
        elif kind == "vendored":
            record_vendored_manifest(manifest, specs, failures)
        else:
            check_npm_manifest(root, manifest, rows, failures)

    for row in sorted(set(rows)):
        print("PACKAGE\t" + "\t".join(row))
    for failure in failures:
        print("FAILURE\t" + failure)
    if args.complete:
        sbom_path = (
            pathlib.Path(args.sbom_out) if args.sbom_out else root / DEFAULT_SBOM_REL
        )
        sbom_rows = write_sbom(sbom_path, rows)
        print(f"SBOM\t{sbom_path}\trows={sbom_rows}")
    print(
        f"SUMMARY\tmanifests={len(specs)}\tunique_rows={len(set(rows))}"
        f"\tfailures={len(failures)}\tstatus={'STOP' if failures else 'PASS'}"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
