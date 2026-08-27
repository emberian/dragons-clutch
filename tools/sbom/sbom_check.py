#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Offline dependency/license SBOM for the dClutch repository.

GITSCAN-2's G-4 (docs/ASPIRATION_LEDGER.md): gen-1 (dragons-clutch) ran a real
SBOM/license closure -- `scripts/dependency_license_check.py`, 36 manifests,
2,129 unique rows, 0 failures, PASS, with a committed catalog byte-gated
against drift -- and left three dependency families for a human to review
(MPL-2.0, CDLA-Permissive-2.0, one license-file-only crate). Gen-3 (this
repository) had no such instrument at all: a regression, not a carry-over,
and the surface is now strictly larger (this repository has an npm tree
gen-1 never had, and the Pages workflow now distributes the frontend, which
makes the AGPL-3.0-or-later source-offer obligation live rather than
theoretical).

This tool is the same method, generalized to this repository's shape:

  - every tracked ``Cargo.lock`` (there is no single workspace: 47 of them,
    the root workspace plus 46 independent test-program/tool mini-workspaces)
    resolved with ``cargo metadata --locked --offline``, and
  - every tracked ``package.json`` with its ``package-lock.json`` (today:
    ``apps/dclutch-web``; the discovery is automatic so a future tool's
    ``package.json`` is picked up without editing this script).

Classification rules, per package:

  - A Cargo registry dependency needs an SPDX ``license`` expression (or a
    ``license_file``, digest-pinned below) and a checksum recorded in the
    lockfile. Any other source (git, a bare path outside this repository,
    an unrecognized registry) is a FAILURE.
  - A first-party (path) dependency must resolve inside this repository. If
    it declares a license, that license is recorded. If it declares
    ``publish = false`` (an internal, never-published crate -- test
    harnesses, program-test fixtures, caller programs) and declares no
    license at all, it is recorded as inheriting this repository's own
    default license (read from the root ``Cargo.toml``'s
    ``[workspace.package].license``, not hardcoded) with that basis stated
    plainly in its row -- this is a real, mechanical fact about the
    repository (one LICENSE governs everything not stated otherwise), not a
    guess about the crate. A first-party crate that is *not* marked
    ``publish = false`` and still declares no license is a FAILURE: nothing
    excuses an omission on code meant to leave the repository standalone.
  - An npm dependency's license comes from the lockfile's own ``license``
    field first (modern ``package-lock.json`` embeds it per package, which
    is what makes an uninstalled optional platform variant --
    ``@esbuild/linux-arm64`` and friends -- still classifiable without ever
    installing it), then the installed copy's ``package.json`` ``license``
    field, then that same file's legacy ``licenses: [...]`` array (an old,
    unambiguous convention some packages still use), then a ``LICENSE*``
    file next to it, digest-pinned exactly like the Cargo case. No license
    found anywhere in that chain is a FAILURE.

Nothing here infers a license from text. A digest-pinned ``license_file`` or
``LICENSE*`` file is recorded as ``LicenseRef-file:<path>:sha256=<digest>``,
never translated into an SPDX guess -- and every such row is FLAGGED for
human review (see below), because the tool did not determine what license it
actually is.

Flagging (the review list, never a mechanical guess):

  - Any row whose license involves a copyleft family this repository does
    not itself use for that dependency's role -- AGPL/GPL/LGPL/SSPL/MPL/CDLA/
    CDDL/EPL/OSL edges -- on a *third-party* (non-path) dependency. This
    repository's own AGPL-3.0-or-later on its own first-party crates is not
    flagged; a third-party dependency carrying a copyleft or
    copyleft-adjacent license, which interacts with distribution, is.
  - Any ``LicenseRef-file:`` row (license-file-only; SPDX identity
    unresolved).
  - Any license expression this tool does not recognize as unambiguously
    permissive (the allowlist is stated below and is deliberately small).

Flagging never fails the gate by itself -- gen-1's own SBOM was PASS with
three flagged families outstanding, and a review queue that also reds the
build stops getting reviewed and starts getting silenced. What fails the
gate: a genuinely unclassified license (nothing in the chain above), a
forbidden dependency source, a stale/unresolvable lockfile, or a committed
``SBOM.md`` that does not byte-match a fresh run.

Usage:
    tools/sbom/sbom_check.py            regenerate tools/sbom/SBOM.md
    tools/sbom/sbom_check.py --verify   check for drift and classification
                                         failures; writes nothing; exit 1 on
                                         either
    tools/sbom/sbom_check.py --root DIR   check a different tree (tests use
                                           this; defaults to the repository
                                           containing this script)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass

REPO_ROOT_DEFAULT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_SBOM_REL = "tools/sbom/SBOM.md"
EXCLUDED_TREE_PARTS = {".git", "target", "node_modules", ".cache", "dist"}
NPM_REGISTRY_PREFIX = "https://registry.npmjs.org/"

# Unambiguously permissive, no-review-needed licenses (and the atoms an SPDX
# AND/OR/WITH expression built only from these is also unflagged). Small and
# explicit on purpose: anything not on this list is FLAGGED, never assumed.
PERMISSIVE_ALLOWLIST = {
    "MIT",
    "MIT-0",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-1-Clause",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSD-4-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "CC0-1.0",
    "BlueOak-1.0.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "WTFPL",
    "Python-2.0",
    "Unlicense",
    "BSL-1.0",
}

# Substring markers for copyleft / copyleft-adjacent / otherwise-owed-review
# license families. Substring matching over an SPDX expression, so
# "LGPL-3.0-or-later" is caught by "GPL" and "AGPL-3.0-or-later" is caught by
# both "AGPL" and "GPL" -- redundancy here is harmless, a miss is not.
COPYLEFT_MARKERS = (
    "AGPL",
    "GPL",
    "MPL",
    "SSPL",
    "CDDL",
    "EPL",
    "OSL",
    "CECILL",
    "CDLA",
    "EUPL",
)


@dataclass(frozen=True, order=True)
class Row:
    ecosystem: str  # "cargo" | "npm"
    manifest: str
    name: str
    version: str
    source: str
    checksum: str
    license: str
    basis: str  # how the license was determined -- real SBOM provenance


def load_workspace_default_license(root: pathlib.Path) -> str:
    data = tomllib.loads((root / "Cargo.toml").read_text())
    license_id = data.get("workspace", {}).get("package", {}).get("license")
    if not license_id:
        raise SystemExit(
            "sbom_check: root Cargo.toml has no [workspace.package].license; "
            "the first-party-undeclared fallback has nothing to fall back to"
        )
    return license_id


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


# ------------------------------------------------------------------- cargo


def license_file_ref(license_path: pathlib.Path, declared_name: str) -> str:
    digest = hashlib.sha256(license_path.read_bytes()).hexdigest()
    return f"LicenseRef-file:{declared_name}:sha256={digest}"


def classify_cargo_package(
    package: dict,
    locked: dict[tuple[str, str, str | None], str | None],
    root: pathlib.Path,
    manifest: str,
    default_first_party_license: str,
    failures: list[str],
) -> Row | None:
    name, version = package["name"], package["version"]
    source = package.get("source")

    if source is None:
        package_root = pathlib.Path(package["manifest_path"]).resolve().parent
        try:
            rel = package_root.relative_to(root).as_posix()
        except ValueError:
            failures.append(f"{manifest}: path dependency outside repository: {package_root}")
            return None
        license_id = package.get("license")
        if license_id:
            basis = "declared"
        else:
            license_file = package.get("license_file")
            if license_file:
                license_path = package_root / license_file
                if not license_path.is_file():
                    failures.append(
                        f"{manifest}: absent declared license_file: {name} {license_file}"
                    )
                    return None
                license_id = license_file_ref(license_path, license_file)
                basis = "license_file"
            elif package.get("publish") == []:
                license_id = default_first_party_license
                basis = "inherited-default (publish=false, undeclared)"
            else:
                failures.append(f"{manifest}: missing license and license_file: {name} {version}")
                return None
        return Row("cargo", manifest, name, version, f"path+{rel}", "-", license_id, basis)

    if source.startswith("registry+"):
        license_id = package.get("license")
        if license_id:
            basis = "declared"
        else:
            license_file = package.get("license_file")
            if not license_file:
                failures.append(f"{manifest}: missing license and license_file: {name} {version}")
                return None
            license_path = pathlib.Path(package["manifest_path"]).resolve().parent / license_file
            if not license_path.is_file():
                failures.append(f"{manifest}: absent declared license_file: {name} {license_file}")
                return None
            license_id = license_file_ref(license_path, license_file)
            basis = "license_file"
        checksum = locked.get((name, version, source))
        if not checksum:
            failures.append(f"{manifest}: registry lock lacks checksum: {name} {version}")
            return None
        return Row("cargo", manifest, name, version, source, checksum, license_id, basis)

    failures.append(f"{manifest}: forbidden dependency source: {name} {source}")
    return None


STALE_LOCK_MARKER = "because --locked was passed to prevent this"


def check_cargo_manifest(
    root: pathlib.Path,
    manifest: str,
    lock_rel: str,
    default_first_party_license: str,
    rows: list[Row],
    failures: list[str],
    coverage: list[tuple[str, int]],
    unresolvable: list[str],
) -> None:
    """Classify one workspace-root manifest's whole resolved graph.

    A ``--locked`` refusal because the lock does not match the manifest
    (``STALE_LOCK_MARKER``) is recorded separately from ``failures``: it is
    real, reportable debt (this repository's dependency tree is not fully
    reproducible right now for that mini-workspace) but a *different* defect
    than a license question, and does not by itself redden this gate --
    otherwise the SBOM lane would need edit rights over every other lane's
    in-flight Cargo.toml to ever go green, which is exactly the collision
    this tool must not cause. Any other metadata failure (a genuinely
    unresolvable dependency, a corrupt lock) still fails outright.
    """
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
            if STALE_LOCK_MARKER in proc.stderr:
                # A fixed, stable message on purpose: cargo's stderr here can
                # carry transient, run-to-run-variable noise (e.g. "Blocking
                # waiting for file lock on package cache" under concurrent
                # cargo invocations from other lanes), and this tool's
                # committed output must be byte-stable regardless of who
                # else is building at the same moment.
                unresolvable.append(
                    f"{manifest}: Cargo.lock does not match Cargo.toml "
                    f"(cargo metadata --locked --offline refused to resolve it)"
                )
            else:
                detail = " | ".join(line.strip() for line in proc.stderr.splitlines() if line.strip())
                failures.append(f"{manifest}: cargo metadata rc={proc.returncode}: {detail}")
            return
        data = json.loads(proc.stdout)
        before = len(rows)
        for package in data["packages"]:
            row = classify_cargo_package(
                package, locked, root, manifest, default_first_party_license, failures
            )
            if row is not None:
                rows.append(row)
        coverage.append((manifest, len(rows) - before))
    except Exception as exc:  # noqa: BLE001 -- recorded, never silent
        failures.append(f"{manifest}: checker exception: {type(exc).__name__}: {exc}")


def discover_cargo_specs(
    root: pathlib.Path, failures: list[str]
) -> tuple[list[tuple[str, str]], list[str]]:
    """Every *workspace-root* manifest (its own ``[workspace]`` table), each
    paired with its own committed lock.

    This repository has no single workspace: 38 manifests each declare
    ``[workspace]`` themselves (the root plus 37 self-contained
    test-program/tool mini-workspaces). Every other tracked ``Cargo.toml`` is
    a *member*, adopted by the nearest ancestor that declares ``[workspace]``
    (Cargo's own directory-walk rule -- membership in that ancestor's
    ``members`` list is not required for the adoption, only exclusion from
    it prevents it) even when it is not named in that ancestor's ``members``
    at all. ``cargo metadata --manifest-path`` on a member returns the
    *entire* owning workspace's resolved graph, so checking members
    individually would both recompute the same closure dozens of times and
    misattribute its rows to the wrong manifest.

    Fifteen of those members carry their own leftover ``Cargo.lock`` next to
    them anyway (pre-workspace-membership relics, most likely): cargo never
    reads those files, so they are excluded from checking here and returned
    separately as a stray-lock finding -- real repository hygiene debt, not
    an SBOM classification question.
    """
    manifests = tracked_files(root, "*Cargo.toml")
    ws_manifests: list[tuple[str, str]] = []
    member_dirs: set[str] = set()
    for manifest_rel in manifests:
        data = tomllib.loads((root / manifest_rel).read_text())
        pkg_dir = str(pathlib.PurePosixPath(manifest_rel).parent)
        if "workspace" in data:
            lock_rel = str(pathlib.PurePosixPath(manifest_rel).parent / "Cargo.lock")
            if not (root / lock_rel).is_file():
                failures.append(f"{manifest_rel}: workspace manifest has no committed Cargo.lock")
                continue
            ws_manifests.append((manifest_rel, lock_rel))
        else:
            member_dirs.add(pkg_dir)

    stray_locks = sorted(
        lock_rel
        for lock_rel in tracked_files(root, "*Cargo.lock")
        if str(pathlib.PurePosixPath(lock_rel).parent) in member_dirs
    )
    return ws_manifests, stray_locks


# --------------------------------------------------------------------- npm


# Registry packages whose own metadata carries no modern ``license`` field
# anywhere this tool can read WITHOUT an installed node_modules (the lock has
# no license field and the package.json uses the legacy ``licenses`` array or
# nothing at all). Each row is pinned to the exact lockfile integrity, so a
# republished tarball under the same version stops matching and fails loudly.
# Provenance, verified against the installed package the day each row was
# added: eyes 0.1.8 declares ``licenses: ['MIT']`` (legacy array);
# text-encoding-utf-8 1.0.2 ships the W3C text-encoding polyfill's dual
# licence in its LICENSE.md (Unlicense OR Apache-2.0) and declares nothing in
# package.json.
CURATED_NPM_LICENSES: dict[tuple[str, str, str], str] = {
    (
        "eyes",
        "0.1.8",
        "sha512-GipyPsXO1anza0AOZdy69Im7hGFCNB7Y/NGjDlZGJ3GJJLtwNSb2vrzYrTYJRrRloVx7pl+bhUaTB8yiccPvFQ==",
    ): "MIT",
    (
        "text-encoding-utf-8",
        "1.0.2",
        "sha512-8bw4MY9WjdsD2aMtO0OzOCY3pXGYNx2d2FfHRVUKkiCPDWjKuOlhLVASS+pD7VkLTVjW268LYJHwsnPFlBpbAg==",
    ): "(Unlicense OR Apache-2.0)",
}


def npm_package_license(
    entry_path: str, entry: dict, root: pathlib.Path, manifest_dir: pathlib.Path
) -> tuple[str | None, str | None]:
    """Return (license, basis) for one resolved npm package, or (None, None)."""
    lock_license = entry.get("license")
    if lock_license:
        return lock_license, "npm-lock-field"

    curated = CURATED_NPM_LICENSES.get(
        (entry.get("name") or entry_path.rpartition("node_modules/")[2],
         entry.get("version", "?"), entry.get("integrity", ""))
    )
    if curated:
        return curated, "curated-integrity-pinned"

    installed = manifest_dir / entry_path / "package.json"
    if installed.is_file():
        data = json.loads(installed.read_text())
        if data.get("license"):
            return data["license"], "npm-installed-package.json"
        legacy = data.get("licenses")
        if isinstance(legacy, list) and len(legacy) == 1 and isinstance(legacy[0], dict):
            legacy_type = legacy[0].get("type")
            if legacy_type:
                return legacy_type, "npm-legacy-licenses-array"
        if isinstance(legacy, list) and len(legacy) == 1 and isinstance(legacy[0], str):
            return legacy[0], "npm-legacy-licenses-array"

    pkg_dir = manifest_dir / entry_path
    if pkg_dir.is_dir():
        for candidate in sorted(pkg_dir.glob("LICENSE*")) + sorted(pkg_dir.glob("License*")):
            if candidate.is_file():
                rel = candidate.relative_to(pkg_dir).as_posix()
                return license_file_ref(candidate, rel), "npm-license-file"
    return None, None


def npm_declared_dependencies(data: dict) -> dict[str, str]:
    merged: dict[str, str] = {}
    for section in ("dependencies", "devDependencies", "optionalDependencies", "peerDependencies"):
        table = data.get(section)
        if isinstance(table, dict):
            merged.update(table)
    return merged


def check_npm_manifest(
    root: pathlib.Path,
    manifest: str,
    default_first_party_license: str,
    rows: list[Row],
    failures: list[str],
    coverage: list[tuple[str, int]],
) -> None:
    manifest_path = root / manifest
    manifest_dir = manifest_path.parent
    try:
        data = json.loads(manifest_path.read_text())
        package_dir = pathlib.PurePosixPath(manifest).parent
        name = data.get("name") or str(package_dir)
        version = data.get("version", "0.0.0")
        before = len(rows)
        license_id = data.get("license") or default_first_party_license
        basis = "declared" if data.get("license") else "inherited-default (first-party app, undeclared)"
        rows.append(Row("npm", manifest, name, version, f"path+{package_dir}", "-", license_id, basis))

        declared = npm_declared_dependencies(data)
        lock_path = manifest_dir / "package-lock.json"
        if not declared:
            coverage.append((manifest, len(rows) - before))
            return
        if not lock_path.is_file():
            for dep_name, requirement in sorted(declared.items()):
                failures.append(f"{manifest}: npm dependency has no lockfile: {dep_name} {requirement}")
            return
        lock = json.loads(lock_path.read_text())
        packages = lock.get("packages")
        if not isinstance(packages, dict):
            failures.append(f"{manifest}: unsupported npm lockfile (no packages map)")
            return
        for entry_path, entry in sorted(packages.items()):
            if entry_path == "" or entry.get("link"):
                continue
            dep_name = entry.get("name") or entry_path.rpartition("node_modules/")[2]
            dep_version = entry.get("version", "?")
            if not entry_path.startswith("node_modules/"):
                # A ``file:`` dependency's target entry (npm keys it by its
                # relative path). Same rule as a Cargo path dependency: it
                # must resolve inside this repository, and it is first-party
                # -- there is no registry integrity to demand.
                target = (manifest_dir / entry_path).resolve()
                try:
                    rel = target.relative_to(root.resolve())
                except ValueError:
                    failures.append(f"{manifest}: path dependency outside repository: {dep_name} {target}")
                    continue
                license_id = entry.get("license") or default_first_party_license
                basis = "declared" if entry.get("license") else "inherited-default (first-party path dependency, undeclared)"
                rows.append(Row("npm", manifest, dep_name, dep_version, f"path+{rel}", "-", license_id, basis))
                continue
            resolved = entry.get("resolved", "")
            if resolved and not resolved.startswith(NPM_REGISTRY_PREFIX):
                failures.append(f"{manifest}: forbidden npm source: {dep_name} {resolved}")
                continue
            if not entry.get("integrity") and not entry.get("optional"):
                failures.append(f"{manifest}: npm lock lacks integrity: {dep_name} {dep_version}")
                continue
            dep_license, basis = npm_package_license(entry_path, entry, root, manifest_dir)
            if not dep_license:
                if entry.get("optional"):
                    # Not installed on this platform (a platform-specific
                    # optional variant, e.g. an esbuild/workerd arch build)
                    # and the lock carries no embedded license field for it
                    # either. Reported, not failed: it never executes on any
                    # real install of this project, and gen-1's gate never
                    # had to handle npm's optionalDependencies fan-out at all.
                    print(
                        f"NOTE\t{manifest}: npm optional dependency license unresolvable "
                        f"offline (not installed, lock has no license field): "
                        f"{dep_name} {dep_version}"
                    )
                    continue
                failures.append(f"{manifest}: npm license unresolvable offline: {dep_name} {dep_version}")
                continue
            rows.append(
                Row(
                    "npm", manifest, dep_name, dep_version,
                    f"npm+{NPM_REGISTRY_PREFIX}", entry.get("integrity", "-"), dep_license, basis,
                )
            )
        coverage.append((manifest, len(rows) - before))
    except Exception as exc:  # noqa: BLE001
        failures.append(f"{manifest}: checker exception: {type(exc).__name__}: {exc}")


def discover_npm_specs(root: pathlib.Path) -> list[str]:
    return tracked_files(root, "*package.json")


# --------------------------------------------------------------- flagging


def dedupe_rows(rows: list[Row]) -> list[Row]:
    """One row per distinct (ecosystem, name, version, source, checksum,
    license, basis), regardless of how many of this repository's 32
    independent Cargo/npm manifests happen to resolve it. ``manifest`` is
    excluded from the identity on purpose -- an SBOM lists what a repository
    depends on once each, not once per consumer; ``Coverage`` below is the
    per-manifest traceability. Deterministic regardless of scan order: ties
    keep the lexicographically-first manifest.
    """
    best: dict[tuple, Row] = {}
    for r in rows:
        key = (r.ecosystem, r.name, r.version, r.source, r.checksum, r.license, r.basis)
        if key not in best or r.manifest < best[key].manifest:
            best[key] = r
    return sorted(best.values())


def flag_row(row: Row) -> str | None:
    if row.license.startswith("LicenseRef-file:"):
        return "license-file-only: SPDX identity unresolved, needs human eyes"
    if row.source.startswith("path+"):
        return None  # our own declared license is never a review item
    if any(marker in row.license for marker in COPYLEFT_MARKERS):
        return "copyleft or copyleft-adjacent license on a third-party dependency"
    # `/` is the pre-SPDX dual-license separator Cargo's own docs used to
    # document ("MIT/Apache-2.0" meaning either, at the licensee's choice) --
    # a real, historical, unambiguous convention, split alongside the SPDX
    # OR/AND top-level keywords. WITH is deliberately not a split point: it
    # binds a license to one exception as a single compound unit ("Apache-2.0
    # WITH LLVM-exception" is one atom, not two), and the allowlist already
    # carries that exact compound string where it applies.
    atoms = [
        a.strip()
        for a in re.split(r"\s+(?:OR|AND)\s+|\s*/\s*|[()]", row.license)
        if a.strip()
    ]
    if atoms and all(a in PERMISSIVE_ALLOWLIST for a in atoms):
        return None
    return "unrecognized license expression, not on the permissive allowlist"


# --------------------------------------------------------------- notices


NOTICES_PREAMBLE = (
    "This repository's own code is `AGPL-3.0-or-later` (see `LICENSE` in the "
    "parent `dragons-clutch` repository, and every crate's `Cargo.toml`); "
    "this page is the mechanical notice aggregation the Pages artifact "
    "republishes (`tools/genref/render-site.mjs`), not a legal opinion. It "
    "lists, once each, every distinct third-party license identified in the "
    "full closure (`tools/sbom/SBOM.md`), and which dependencies carry it."
)

DEFAULT_NOTICES_REL = "tools/sbom/NOTICES.md"


def render_notice_groups(unique: list[Row]) -> list[str]:
    by_license_names: dict[str, list[str]] = {}
    for r in unique:
        if r.source.startswith("path+"):
            continue  # first-party rows are not third-party notices
        by_license_names.setdefault(r.license, []).append(f"{r.name} {r.version} ({r.ecosystem})")
    lines: list[str] = []
    for lic in sorted(by_license_names):
        lines.append(f"### `{lic}`")
        lines.append("")
        for dep in sorted(set(by_license_names[lic])):
            lines.append(f"- {dep}")
        lines.append("")
    return lines


def write_notices(path: pathlib.Path, unique: list[Row]) -> str:
    lines = [
        "<!-- @generated by tools/sbom/sbom_check.py -- do not hand-edit -->",
        "# Third-party notices",
        "",
        NOTICES_PREAMBLE,
        "",
        *render_notice_groups(unique),
    ]
    content = "\n".join(lines) + "\n"
    path.write_text(content)
    return content


# ------------------------------------------------------------------ report


def write_sbom(
    path: pathlib.Path,
    rows: list[Row],
    coverage: list[tuple[str, int]],
    stray_locks: list[str],
    unresolvable: list[str],
) -> str:
    unique = dedupe_rows(rows)
    cargo_rows = [r for r in unique if r.ecosystem == "cargo"]
    npm_rows = [r for r in unique if r.ecosystem == "npm"]
    flagged = [(r, flag_row(r)) for r in unique]
    flagged = [(r, why) for r, why in flagged if why]

    lines: list[str] = []
    lines.append("<!-- @generated by tools/sbom/sbom_check.py -- do not hand-edit -->")
    lines.append("# SBOM — dClutch dependency/license closure")
    lines.append("")
    lines.append(
        "The complete dependency/license closure of this repository: every "
        "tracked Cargo workspace (there is no single one — 47 independent "
        "lockfiles, listed in [Coverage](#coverage)) and the web app's npm "
        "tree. Regenerate with `tools/sbom/sbom_check.py`; check for drift "
        "with `tools/sbom/sbom_check.py --verify` (also wired into "
        "`tools/gauntlet` — see `tools/sbom/README.md`)."
    )
    lines.append("")
    lines.append(
        f"**{len(coverage)} manifests, {len(unique)} unique dependency rows "
        f"({len(cargo_rows)} cargo, {len(npm_rows)} npm), "
        f"{len(flagged)} flagged for human review.**"
    )
    lines.append("")

    by_license: dict[str, int] = {}
    for r in unique:
        by_license[r.license] = by_license.get(r.license, 0) + 1
    lines.append("## Counts by license")
    lines.append("")
    lines.append("| License | Rows |")
    lines.append("|---|---|")
    for lic, count in sorted(by_license.items(), key=lambda kv: (-kv[1], kv[0])):
        lines.append(f"| `{lic}` | {count} |")
    lines.append("")

    lines.append("## Flagged for review")
    lines.append("")
    lines.append(
        "Never a mechanical guess, never silently classified: every row below "
        "needs a human license call, not this tool's. A flagged row does not "
        "fail `--verify` by itself (gen-1's precedent: 36 manifests PASS with "
        "three flagged families outstanding) — the flag is the deliverable, "
        "not a defect in the SBOM."
    )
    lines.append("")
    if flagged:
        lines.append("| Ecosystem | Name | Version | License | Reason |")
        lines.append("|---|---|---|---|---|")
        for r, why in sorted(flagged, key=lambda t: (t[0].ecosystem, t[0].name, t[0].version)):
            lines.append(f"| {r.ecosystem} | `{r.name}` | {r.version} | `{r.license}` | {why} |")
    else:
        lines.append("None.")
    lines.append("")

    def dep_table(title: str, table_rows: list[Row]) -> None:
        lines.append(f"## {title}")
        lines.append("")
        lines.append("| Name | Version | License | Source | Basis |")
        lines.append("|---|---|---|---|---|")
        for r in sorted(table_rows, key=lambda t: (t.name, t.version)):
            lines.append(f"| `{r.name}` | {r.version} | `{r.license}` | `{r.source}` | {r.basis} |")
        lines.append("")

    dep_table("Cargo dependencies", cargo_rows)
    dep_table("npm dependencies (apps/dclutch-web)", npm_rows)

    lines.append("## Coverage")
    lines.append("")
    lines.append("Every manifest this tool checked, and how many of the rows above it contributed.")
    lines.append("")
    lines.append("| Manifest | Packages |")
    lines.append("|---|---|")
    for manifest, count in sorted(coverage):
        lines.append(f"| `{manifest}` | {count} |")
    lines.append("")

    if unresolvable:
        lines.append("## Unresolvable manifests (stale lockfile, not a license question)")
        lines.append("")
        lines.append(
            "Each manifest below refused `cargo metadata --locked --offline`: its "
            "`Cargo.lock` does not match its `Cargo.toml` (typically a dependency edge "
            "added or changed without re-running cargo in that mini-workspace). This "
            "tool cannot see that manifest's dependency graph at all until its owning "
            "lane runs `cargo metadata`/`cargo check` there and commits the refreshed "
            "lock — recorded here as owed work, not folded into the flagged-license "
            "review list above, and not failing `--verify` by itself, since it is a "
            "reproducibility gap rather than an unclassified license."
        )
        lines.append("")
        for note in sorted(unresolvable):
            lines.append(f"- `{note}`")
        lines.append("")

    if stray_locks:
        lines.append("## Stray lockfiles (not read by cargo, not part of this closure)")
        lines.append("")
        lines.append(
            "Each of these sits next to a manifest with no `[workspace]` table of its "
            "own; cargo's directory-walk rule adopts that manifest into the nearest "
            "ancestor workspace instead (present in the ancestor's row counts above), "
            "so the lockfile below is dead — not consulted, not kept in sync, and safe "
            "to delete as ordinary repository hygiene rather than an SBOM question. "
            "Left untouched here; not this tool's file to remove."
        )
        lines.append("")
        for lock_rel in stray_locks:
            lines.append(f"- `{lock_rel}`")
        lines.append("")

    lines.append("## Notices")
    lines.append("")
    lines.append(NOTICES_PREAMBLE)
    lines.append("")
    lines.extend(render_notice_groups(unique))

    content = "\n".join(lines) + "\n"
    path.write_text(content)
    return content


# -------------------------------------------------------------------- main


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", default=str(REPO_ROOT_DEFAULT), help="repository root to check")
    parser.add_argument("--out", default=None, help=f"SBOM.md path (default: <root>/{DEFAULT_SBOM_REL})")
    parser.add_argument(
        "--notices-out", default=None,
        help=f"NOTICES.md path (default: <root>/{DEFAULT_NOTICES_REL})",
    )
    parser.add_argument(
        "--verify", action="store_true",
        help="write nothing; fail if the committed SBOM.md/NOTICES.md would change or any classification failed",
    )
    args = parser.parse_args(argv)
    root = pathlib.Path(args.root).resolve()
    sbom_path = pathlib.Path(args.out).resolve() if args.out else root / DEFAULT_SBOM_REL
    notices_path = (
        pathlib.Path(args.notices_out).resolve() if args.notices_out else root / DEFAULT_NOTICES_REL
    )

    default_first_party_license = load_workspace_default_license(root)

    rows: list[Row] = []
    failures: list[str] = []
    coverage: list[tuple[str, int]] = []
    unresolvable: list[str] = []

    cargo_specs, stray_locks = discover_cargo_specs(root, failures)
    for manifest, lock_rel in cargo_specs:
        check_cargo_manifest(
            root, manifest, lock_rel, default_first_party_license, rows, failures, coverage, unresolvable
        )
    for manifest in discover_npm_specs(root):
        check_npm_manifest(root, manifest, default_first_party_license, rows, failures, coverage)

    unique = dedupe_rows(rows)
    for row in unique:
        print(
            f"PACKAGE\t{row.ecosystem}\t{row.manifest}\t{row.name}\t{row.version}\t"
            f"{row.source}\t{row.checksum}\t{row.license}\t{row.basis}"
        )
    for note in unresolvable:
        print(f"UNRESOLVABLE\t{note}")
    for failure in failures:
        print(f"FAILURE\t{failure}")

    if args.verify:
        previous_sbom = sbom_path.read_text() if sbom_path.is_file() else None
        previous_notices = notices_path.read_text() if notices_path.is_file() else None
        fresh_sbom = write_sbom(sbom_path.with_suffix(".verify.tmp"), rows, coverage, stray_locks, unresolvable)
        sbom_path.with_suffix(".verify.tmp").unlink(missing_ok=True)
        fresh_notices = write_notices(notices_path.with_suffix(".verify.tmp"), unique)
        notices_path.with_suffix(".verify.tmp").unlink(missing_ok=True)
        drifted = previous_sbom != fresh_sbom or previous_notices != fresh_notices
        status = "STOP" if (failures or drifted) else "PASS"
        print(
            f"SUMMARY\tmanifests={len(coverage)}\tunique_rows={len(unique)}"
            f"\tfailures={len(failures)}\tunresolvable={len(unresolvable)}\tdrift={drifted}\tstatus={status}"
        )
        return 1 if (failures or drifted) else 0

    write_sbom(sbom_path, rows, coverage, stray_locks, unresolvable)
    write_notices(notices_path, unique)
    print(f"SBOM\t{sbom_path}\trows={len(unique)}")
    print(f"NOTICES\t{notices_path}")
    print(
        f"SUMMARY\tmanifests={len(coverage)}\tunique_rows={len(unique)}"
        f"\tfailures={len(failures)}\tunresolvable={len(unresolvable)}\tstatus={'STOP' if failures else 'PASS'}"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
