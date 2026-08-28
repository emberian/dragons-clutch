#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Focused, offline, stdlib-only tests for sbom_check's classification logic.

No cargo, no network, no git: these exercise exactly the decisions a human
should be able to argue with -- what counts as permissive, what gets a
license-file digest instead of an SPDX id, how a first-party undeclared
crate is classified, and that "the SBOM lists a dependency once" survives
the same package resolving through several manifests. The end-to-end
behavior (real cargo/npm trees, drift detection) is exercised by running
``sbom_check.py`` itself against this repository, not reproduced here.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import sbom_check as sc  # noqa: E402


def make_row(**kwargs) -> sc.Row:
    base = dict(
        ecosystem="cargo",
        manifest="m/Cargo.toml",
        name="pkg",
        version="1.0.0",
        source="registry+https://github.com/rust-lang/crates.io-index",
        checksum="abc123",
        license="MIT",
        basis="declared",
    )
    base.update(kwargs)
    return sc.Row(**base)


class FlagRowTests(unittest.TestCase):
    def test_permissive_single_license_is_unflagged(self) -> None:
        self.assertIsNone(sc.flag_row(make_row(license="MIT")))
        self.assertIsNone(sc.flag_row(make_row(license="Apache-2.0")))
        self.assertIsNone(sc.flag_row(make_row(license="Unlicense")))

    def test_permissive_or_expression_is_unflagged(self) -> None:
        self.assertIsNone(sc.flag_row(make_row(license="MIT OR Apache-2.0")))
        self.assertIsNone(sc.flag_row(make_row(license="Unlicense OR MIT")))

    def test_legacy_slash_separator_is_treated_as_or(self) -> None:
        # Cargo's own pre-SPDX convention: "MIT/Apache-2.0" means either, at
        # the licensee's choice -- not an unrecognized opaque token.
        self.assertIsNone(sc.flag_row(make_row(license="MIT/Apache-2.0")))
        self.assertIsNone(sc.flag_row(make_row(license="Apache-2.0 / MIT")))

    def test_with_exception_is_one_atom_not_split(self) -> None:
        self.assertIsNone(
            sc.flag_row(make_row(license="Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"))
        )
        # "LLVM-exception" alone must never be checked against the allowlist.
        self.assertNotIn("LLVM-exception", sc.PERMISSIVE_ALLOWLIST)

    def test_copyleft_marker_flags_third_party_row(self) -> None:
        row = make_row(license="MPL-2.0+")
        self.assertEqual(
            sc.flag_row(row), "copyleft or copyleft-adjacent license on a third-party dependency"
        )
        row = make_row(license="LGPL-3.0-or-later")
        self.assertIsNotNone(sc.flag_row(row))
        row = make_row(license="CDLA-Permissive-2.0")
        self.assertIsNotNone(sc.flag_row(row))

    def test_copyleft_inside_or_expression_still_flagged(self) -> None:
        # An escape hatch to a permissive branch does not resolve the
        # question mechanically -- a human still has to confirm which branch
        # is actually being relied on.
        row = make_row(license="MIT OR Apache-2.0 OR LGPL-2.1-or-later")
        self.assertIsNotNone(sc.flag_row(row))

    def test_first_party_path_row_is_never_flagged_even_if_copyleft(self) -> None:
        row = make_row(license="AGPL-3.0-or-later", source="path+crates/dclutch-x")
        self.assertIsNone(sc.flag_row(row))

    def test_license_file_only_is_always_flagged(self) -> None:
        row = make_row(license="LicenseRef-file:LICENSE:sha256=deadbeef")
        self.assertEqual(
            sc.flag_row(row), "license-file-only: SPDX identity unresolved, needs human eyes"
        )
        # Even for a first-party row: we still don't know what it actually is.
        row = make_row(license="LicenseRef-file:LICENSE:sha256=deadbeef", source="path+x")
        self.assertIsNotNone(sc.flag_row(row))

    def test_unrecognized_expression_is_flagged(self) -> None:
        row = make_row(license="bzip2-1.0.6")
        self.assertEqual(
            sc.flag_row(row), "unrecognized license expression, not on the permissive allowlist"
        )


class CargoClassificationTests(unittest.TestCase):
    def test_registry_package_missing_license_and_license_file_fails(self) -> None:
        package = {
            "name": "mystery", "version": "1.0.0",
            "manifest_path": "/x/Cargo.toml", "source": "registry+https://x",
        }
        failures: list[str] = []
        row = sc.classify_cargo_package(package, {}, Path("/root"), "m", "AGPL-3.0-or-later", failures)
        self.assertIsNone(row)
        self.assertEqual(failures, ["m: missing license and license_file: mystery 1.0.0"])

    def test_registry_package_missing_checksum_fails(self) -> None:
        package = {
            "name": "pkg", "version": "1.0.0", "license": "MIT",
            "manifest_path": "/x/Cargo.toml", "source": "registry+https://x",
        }
        failures: list[str] = []
        row = sc.classify_cargo_package(package, {}, Path("/root"), "m", "AGPL-3.0-or-later", failures)
        self.assertIsNone(row)
        self.assertIn("registry lock lacks checksum", failures[0])

    def test_forbidden_source_fails(self) -> None:
        package = {
            "name": "pkg", "version": "1.0.0", "license": "MIT",
            "manifest_path": "/x/Cargo.toml", "source": "git+https://example.com/pkg",
        }
        failures: list[str] = []
        row = sc.classify_cargo_package(package, {}, Path("/root"), "m", "AGPL-3.0-or-later", failures)
        self.assertIsNone(row)
        self.assertIn("forbidden dependency source", failures[0])

    def test_first_party_publish_false_undeclared_inherits_repo_default(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            pkg_dir = root / "crates" / "internal-only"
            pkg_dir.mkdir(parents=True)
            manifest_path = pkg_dir / "Cargo.toml"
            manifest_path.write_text("[package]\nname='internal-only'\n")
            package = {
                "name": "internal-only", "version": "0.1.0", "source": None,
                "manifest_path": str(manifest_path), "publish": [],
            }
            failures: list[str] = []
            row = sc.classify_cargo_package(package, {}, root, "m", "AGPL-3.0-or-later", failures)
            self.assertIsNotNone(row)
            self.assertEqual(row.license, "AGPL-3.0-or-later")
            self.assertIn("inherited-default", row.basis)
            self.assertEqual(failures, [])

    def test_first_party_publishable_undeclared_is_a_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            pkg_dir = root / "crates" / "publishable"
            pkg_dir.mkdir(parents=True)
            manifest_path = pkg_dir / "Cargo.toml"
            manifest_path.write_text("[package]\nname='publishable'\n")
            package = {
                "name": "publishable", "version": "0.1.0", "source": None,
                "manifest_path": str(manifest_path), "publish": None,
            }
            failures: list[str] = []
            row = sc.classify_cargo_package(package, {}, root, "m", "AGPL-3.0-or-later", failures)
            self.assertIsNone(row)
            self.assertEqual(failures, ["m: missing license and license_file: publishable 0.1.0"])

    def test_license_file_is_digest_pinned_not_guessed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            pkg_dir = root / "vendor" / "oldcrate"
            pkg_dir.mkdir(parents=True)
            manifest_path = pkg_dir / "Cargo.toml"
            manifest_path.write_text("[package]\n")
            (pkg_dir / "LICENSE").write_text("some license text\n")
            package = {
                "name": "oldcrate", "version": "1.0.0", "source": None,
                "manifest_path": str(manifest_path), "license_file": "LICENSE",
                "publish": None,
            }
            failures: list[str] = []
            row = sc.classify_cargo_package(package, {}, root, "m", "AGPL-3.0-or-later", failures)
            self.assertIsNotNone(row)
            self.assertTrue(row.license.startswith("LicenseRef-file:LICENSE:sha256="))
            self.assertEqual(row.basis, "license_file")

    def test_path_dependency_outside_repository_fails(self) -> None:
        package = {
            "name": "outside", "version": "1.0.0", "source": None,
            "manifest_path": "/completely/elsewhere/Cargo.toml", "publish": None,
        }
        with tempfile.TemporaryDirectory() as tmp:
            failures: list[str] = []
            row = sc.classify_cargo_package(package, {}, Path(tmp), "m", "AGPL-3.0-or-later", failures)
            self.assertIsNone(row)
            self.assertIn("path dependency outside repository", failures[0])


class NpmLicenseChainTests(unittest.TestCase):
    def test_lock_embedded_license_field_is_preferred(self) -> None:
        entry = {"license": "MIT"}
        with tempfile.TemporaryDirectory() as tmp:
            lic, basis = sc.npm_package_license("node_modules/x", entry, Path(tmp), Path(tmp))
            self.assertEqual((lic, basis), ("MIT", "npm-lock-field"))

    def test_legacy_licenses_string_array(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pkg_dir = root / "node_modules" / "eyes"
            pkg_dir.mkdir(parents=True)
            (pkg_dir / "package.json").write_text('{"licenses": ["MIT"]}')
            lic, basis = sc.npm_package_license("node_modules/eyes", {}, root, root)
            self.assertEqual((lic, basis), ("MIT", "npm-legacy-licenses-array"))

    def test_legacy_licenses_dict_array(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pkg_dir = root / "node_modules" / "old"
            pkg_dir.mkdir(parents=True)
            (pkg_dir / "package.json").write_text('{"licenses": [{"type": "BSD-3-Clause"}]}')
            lic, basis = sc.npm_package_license("node_modules/old", {}, root, root)
            self.assertEqual((lic, basis), ("BSD-3-Clause", "npm-legacy-licenses-array"))

    def test_license_file_fallback_is_digest_pinned(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pkg_dir = root / "node_modules" / "text-encoding-utf-8"
            pkg_dir.mkdir(parents=True)
            (pkg_dir / "package.json").write_text('{"name": "text-encoding-utf-8"}')
            (pkg_dir / "LICENSE.md").write_text("This is free and unencumbered software...\n")
            lic, basis = sc.npm_package_license(
                "node_modules/text-encoding-utf-8", {}, root, root
            )
            self.assertTrue(lic.startswith("LicenseRef-file:LICENSE.md:sha256="))
            self.assertEqual(basis, "npm-license-file")

    def test_nothing_found_returns_none(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lic, basis = sc.npm_package_license("node_modules/ghost", {}, Path(tmp), Path(tmp))
            self.assertEqual((lic, basis), (None, None))


class DedupeRowsTests(unittest.TestCase):
    def test_same_dependency_across_manifests_is_one_row(self) -> None:
        rows = [
            make_row(manifest="a/Cargo.toml", name="serde", version="1.0.0"),
            make_row(manifest="b/Cargo.toml", name="serde", version="1.0.0"),
            make_row(manifest="c/Cargo.toml", name="serde", version="1.0.0"),
        ]
        unique = sc.dedupe_rows(rows)
        self.assertEqual(len(unique), 1)

    def test_tie_break_is_deterministic(self) -> None:
        rows = [
            make_row(manifest="z/Cargo.toml"),
            make_row(manifest="a/Cargo.toml"),
        ]
        unique = sc.dedupe_rows(rows)
        self.assertEqual(unique[0].manifest, "a/Cargo.toml")

    def test_different_licenses_for_the_same_package_stay_distinct(self) -> None:
        # Should not happen for one real (name, version, source), but the
        # identity is explicit about what makes two rows "the same" rather
        # than silently merging a license disagreement away.
        rows = [
            make_row(license="MIT"),
            make_row(license="Apache-2.0"),
        ]
        self.assertEqual(len(sc.dedupe_rows(rows)), 2)


class ReportShapeTests(unittest.TestCase):
    def test_report_owns_the_discovered_manifest_set_without_a_fixed_count(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = Path(tmp) / "SBOM.md"
            content = sc.write_sbom(
                report,
                [make_row()],
                [("Cargo.toml", 1), ("app/package.json", 1)],
                [],
                [],
            )

            self.assertIn(
                "every tracked Cargo workspace and npm package tree discovered",
                content,
            )
            self.assertIn("**2 manifests,", content)
            self.assertIn("## npm dependencies (tracked package trees)", content)
            self.assertNotIn("independent lockfiles", content)


if __name__ == "__main__":
    unittest.main()
