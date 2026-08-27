"""Focused declaration and classification tests for dependency_license_check.

Standard library only, offline, no cargo invocation.  The attested-scope pin
is the byte-stability tripwire: the default mode's manifest list may only
change together with a new portable-attestation methodology revision.
"""

from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import dependency_license_check as dlc  # noqa: E402


class AttestedScopeDeclarationTests(unittest.TestCase):
    def test_attested_manifest_list_is_exactly_the_sealed_twelve(self) -> None:
        self.assertEqual(
            dlc.ATTESTED_MANIFESTS,
            [
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
            ],
        )
        self.assertEqual(
            dlc.ATTESTED_LOCK_OVERRIDES,
            {"programs/clutch-sbf/program/Cargo.toml": "programs/clutch-sbf/Cargo.lock"},
        )

    def test_expected_first_party_license_is_the_repository_license(self) -> None:
        self.assertEqual(dlc.EXPECTED_FIRST_PARTY_LICENSE, "AGPL-3.0-or-later")


class LicenseIdentityTests(unittest.TestCase):
    def test_missing_license_and_license_file_is_a_recorded_failure(self) -> None:
        failures: list[str] = []
        package = {"name": "mystery", "version": "1.0.0", "manifest_path": "/x/Cargo.toml"}
        self.assertIsNone(dlc.license_identity(package, "m/Cargo.toml", failures))
        self.assertEqual(
            failures, ["m/Cargo.toml: missing license and license_file: mystery 1.0.0"]
        )

    def test_license_file_becomes_digest_pinned_license_ref(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "LICENSE-CUSTOM").write_bytes(b"custom terms\n")
            digest = hashlib.sha256(b"custom terms\n").hexdigest()
            failures: list[str] = []
            package = {
                "name": "filed",
                "version": "2.0.0",
                "manifest_path": str(root / "Cargo.toml"),
                "license_file": "LICENSE-CUSTOM",
            }
            self.assertEqual(
                dlc.license_identity(package, "m/Cargo.toml", failures),
                f"LicenseRef-file:LICENSE-CUSTOM:sha256={digest}",
            )
            self.assertEqual(failures, [])

    def test_absent_declared_license_file_is_a_recorded_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            failures: list[str] = []
            package = {
                "name": "ghost",
                "version": "3.0.0",
                "manifest_path": str(Path(temp) / "Cargo.toml"),
                "license_file": "NOPE.txt",
            }
            self.assertIsNone(dlc.license_identity(package, "m/Cargo.toml", failures))
            self.assertEqual(
                failures, ["m/Cargo.toml: absent declared license_file: ghost NOPE.txt"]
            )


class ClassifyCargoPackageTests(unittest.TestCase):
    def test_git_source_is_forbidden(self) -> None:
        failures: list[str] = []
        package = {
            "name": "sneaky",
            "version": "0.1.0",
            "license": "MIT",
            "source": "git+https://example.com/sneaky.git#abc",
            "manifest_path": "/x/Cargo.toml",
        }
        row = dlc.classify_cargo_package(package, {}, Path("/repo"), "m", failures)
        self.assertIsNone(row)
        self.assertEqual(
            failures,
            ["m: forbidden dependency source: sneaky git+https://example.com/sneaky.git#abc"],
        )

    def test_registry_package_without_locked_checksum_is_a_failure(self) -> None:
        failures: list[str] = []
        package = {
            "name": "floaty",
            "version": "0.2.0",
            "license": "MIT",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "manifest_path": "/x/Cargo.toml",
        }
        row = dlc.classify_cargo_package(package, {}, Path("/repo"), "m", failures)
        self.assertIsNone(row)
        self.assertEqual(failures, ["m: registry lock lacks checksum: floaty 0.2.0"])

    def test_registry_package_with_checksum_yields_row(self) -> None:
        failures: list[str] = []
        source = "registry+https://github.com/rust-lang/crates.io-index"
        locked = {("solid", "1.2.3", source): "aa" * 32}
        package = {
            "name": "solid",
            "version": "1.2.3",
            "license": "Apache-2.0 OR MIT",
            "source": source,
            "manifest_path": "/x/Cargo.toml",
        }
        row = dlc.classify_cargo_package(package, locked, Path("/repo"), "m", failures)
        self.assertEqual(
            row, ("m", "solid", "1.2.3", source, "aa" * 32, "Apache-2.0 OR MIT")
        )
        self.assertEqual(failures, [])

    def test_path_dependency_outside_repo_is_a_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "repo"
            outside = Path(temp) / "elsewhere"
            outside.mkdir()
            root.mkdir()
            failures: list[str] = []
            package = {
                "name": "escapee",
                "version": "0.1.0",
                "license": "AGPL-3.0-or-later",
                "source": None,
                "manifest_path": str(outside / "Cargo.toml"),
            }
            row = dlc.classify_cargo_package(package, {}, root, "m", failures)
            self.assertIsNone(row)
            self.assertEqual(len(failures), 1)
            self.assertIn("path dependency outside archive", failures[0])

    def test_first_party_path_dependency_must_be_agpl(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            crate = root / "crates" / "oops"
            crate.mkdir(parents=True)
            failures: list[str] = []
            package = {
                "name": "oops",
                "version": "0.1.0",
                "license": "MIT",
                "source": None,
                "manifest_path": str(crate / "Cargo.toml"),
            }
            row = dlc.classify_cargo_package(package, {}, root, "m", failures)
            self.assertIsNone(row)
            self.assertEqual(
                failures, ["m: unexpected first-party license: crates/oops: MIT"]
            )

    def test_vendored_path_dependency_may_keep_upstream_license(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            vendored = root / "programs" / "clutch-sbf" / "vendor" / "up-1.0.0"
            vendored.mkdir(parents=True)
            failures: list[str] = []
            package = {
                "name": "up",
                "version": "1.0.0",
                "license": "Apache-2.0",
                "source": None,
                "manifest_path": str(vendored / "Cargo.toml"),
            }
            row = dlc.classify_cargo_package(package, {}, root, "m", failures)
            self.assertEqual(
                row,
                (
                    "m",
                    "up",
                    "1.0.0",
                    "vendored-path+programs/clutch-sbf/vendor/up-1.0.0",
                    "-",
                    "Apache-2.0",
                ),
            )
            self.assertEqual(failures, [])


class NpmManifestTests(unittest.TestCase):
    def _write_manifest(self, root: Path, body: str) -> str:
        package_dir = root / "apps" / "thing"
        package_dir.mkdir(parents=True)
        (package_dir / "package.json").write_text(body)
        return "apps/thing/package.json"

    def test_dependency_free_agpl_package_yields_single_row(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            manifest = self._write_manifest(
                root,
                '{"name": "thing", "version": "0.1.0", "license": "AGPL-3.0-or-later"}',
            )
            rows: list[dlc.Row] = []
            failures: list[str] = []
            dlc.check_npm_manifest(root, manifest, rows, failures)
            self.assertEqual(
                rows,
                [
                    (
                        manifest,
                        "thing",
                        "0.1.0",
                        "path+apps/thing",
                        "-",
                        "AGPL-3.0-or-later",
                    )
                ],
            )
            self.assertEqual(failures, [])

    def test_declared_dependency_without_lockfile_is_a_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            manifest = self._write_manifest(
                root,
                '{"name": "thing", "version": "0.1.0", '
                '"license": "AGPL-3.0-or-later", '
                '"dependencies": {"left-pad": "^1.0.0"}}',
            )
            rows: list[dlc.Row] = []
            failures: list[str] = []
            dlc.check_npm_manifest(root, manifest, rows, failures)
            self.assertEqual(
                failures,
                [f"{manifest}: npm dependency has no lockfile: left-pad ^1.0.0"],
            )

    def test_missing_npm_license_is_a_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            manifest = self._write_manifest(
                root, '{"name": "thing", "version": "0.1.0"}'
            )
            rows: list[dlc.Row] = []
            failures: list[str] = []
            dlc.check_npm_manifest(root, manifest, rows, failures)
            self.assertEqual(rows, [])
            self.assertEqual(failures, [f"{manifest}: missing license: thing 0.1.0"])


class VendoredLockDispositionTests(unittest.TestCase):
    VENDORED_MANIFEST = (
        "programs/clutch-sbf/vendor/solana-define-syscall-5.1.0/Cargo.toml"
    )

    def test_vendored_lock_with_covering_workspace_is_recorded_not_failed(self) -> None:
        specs = [
            ("programs/clutch-sbf/Cargo.toml", "programs/clutch-sbf/Cargo.lock", "cargo"),
            (self.VENDORED_MANIFEST, self.VENDORED_MANIFEST[:-4] + "lock", "vendored"),
        ]
        failures: list[str] = []
        dlc.record_vendored_manifest(self.VENDORED_MANIFEST, specs, failures)
        self.assertEqual(failures, [])

    def test_vendored_lock_without_covering_workspace_is_a_failure(self) -> None:
        specs = [
            (self.VENDORED_MANIFEST, self.VENDORED_MANIFEST[:-4] + "lock", "vendored"),
        ]
        failures: list[str] = []
        dlc.record_vendored_manifest(self.VENDORED_MANIFEST, specs, failures)
        self.assertEqual(
            failures,
            [
                f"{self.VENDORED_MANIFEST}: vendored lock has no covering "
                "workspace in scope: programs/clutch-sbf/Cargo.toml"
            ],
        )


class SbomTests(unittest.TestCase):
    def test_sbom_rows_are_deduplicated_sorted_and_manifest_free(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / "sbom.tsv"
            rows = [
                ("m2", "b", "1.0.0", "src", "cc", "MIT"),
                ("m1", "b", "1.0.0", "src", "cc", "MIT"),
                ("m1", "a", "2.0.0", "src", "dd", "Apache-2.0"),
            ]
            count = dlc.write_sbom(out, rows)
            self.assertEqual(count, 2)
            self.assertEqual(
                out.read_text(),
                "name\tversion\tsource\tchecksum\tlicense\n"
                "a\t2.0.0\tsrc\tdd\tApache-2.0\n"
                "b\t1.0.0\tsrc\tcc\tMIT\n",
            )


if __name__ == "__main__":
    unittest.main()
