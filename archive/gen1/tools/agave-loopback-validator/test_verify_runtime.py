#!/usr/bin/env python3
"""Adversarial tests for the pinned validator runtime verifier."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("verify_runtime", HERE / "verify-runtime.py")
assert SPEC and SPEC.loader
VERIFY_RUNTIME = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY_RUNTIME)


PINS = {
    "AGAVE_UPSTREAM_URL": "https://example.invalid/agave.git",
    "AGAVE_COMMIT": "549805f3e85f345c9df98d59759691443eef57aa",
    "AGAVE_VERSION": "4.0.2",
    "AGAVE_RUST_TOOLCHAIN": "1.93.1",
    "AGAVE_RUSTC_COMMIT": "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
    "AGAVE_CARGO_LOCK_SHA256": "1" * 64,
    "AGAVE_LICENSE_SHA256": "2" * 64,
    "AGAVE_PATCH_SHA256": "3" * 64,
    "AGAVE_PATCHED_QUIC_CLIENT_SHA256": "4" * 64,
    "AGAVE_PATCHED_UDP_CLIENT_SHA256": "5" * 64,
    "AGAVE_PATCHED_CLI_SHA256": "6" * 64,
    "AGAVE_PATCHED_LIBRARY_SHA256": "7" * 64,
}
VERSION = "solana-test-validator 4.0.2 (src:549805f3; feat:test, client:Agave)"


class RuntimeFixture:
    def __init__(self, root: Path) -> None:
        self.repo = root / "repo"
        self.cache = self.repo / ".cache/agave-loopback-validator"
        self.binary = self.cache / "bin/solana-test-validator"
        self.pins = self.repo / "tools/agave-loopback-validator/pins.env"
        self.manifest = self.cache / "build-provenance.txt"
        self.binary.parent.mkdir(parents=True)
        self.pins.parent.mkdir(parents=True)
        self.binary.write_text(f"#!/bin/sh\nprintf '%s\\n' '{VERSION}'\n", encoding="utf-8")
        self.binary.chmod(0o755)
        self.pins.write_text(
            "".join(f"{key}={value}\n" for key, value in PINS.items()), encoding="utf-8"
        )
        binary_digest = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        fields = {
            "format": "dragons-clutch-agave-loopback-build-v1",
            **{
                manifest_key: PINS[pin_key]
                for pin_key, manifest_key in VERIFY_RUNTIME.PIN_TO_MANIFEST.items()
            },
            "build_mode": "offline",
            "build_profile": "release",
            "binary_path": str(self.binary),
            "binary_bytes": str(self.binary.stat().st_size),
            "binary_sha256": binary_digest,
            "binary_version": VERSION,
        }
        self.manifest.write_text(
            "".join(f"{key}={value}\n" for key, value in fields.items()), encoding="utf-8"
        )

    def verify(self, binary: Path | None = None) -> str:
        return VERIFY_RUNTIME.verify(
            self.repo, self.cache, self.pins, binary or self.binary
        )


class VerifyRuntimeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.fixture = RuntimeFixture(Path(self.temp.name))

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_accepts_exact_pinned_build(self) -> None:
        self.assertEqual(self.fixture.verify(), hashlib.sha256(self.fixture.binary.read_bytes()).hexdigest())

    def test_refuses_stock_or_other_path_even_with_same_bytes(self) -> None:
        stock = Path(self.temp.name) / "solana-test-validator"
        stock.write_bytes(self.fixture.binary.read_bytes())
        stock.chmod(0o755)
        with self.assertRaisesRegex(VERIFY_RUNTIME.Refusal, "outside the repository"):
            self.fixture.verify(stock)

    def test_refuses_tampered_binary(self) -> None:
        with self.fixture.binary.open("ab") as output:
            output.write(b"# tampered\n")
        with self.assertRaisesRegex(VERIFY_RUNTIME.Refusal, "byte length"):
            self.fixture.verify()

    def test_refuses_patch_provenance_drift(self) -> None:
        text = self.fixture.manifest.read_text(encoding="utf-8")
        self.fixture.manifest.write_text(
            text.replace(f"patch_sha256={PINS['AGAVE_PATCH_SHA256']}", "patch_sha256=" + "8" * 64),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VERIFY_RUNTIME.Refusal, "patch_sha256"):
            self.fixture.verify()

    def test_refuses_duplicate_manifest_key(self) -> None:
        with self.fixture.manifest.open("a", encoding="utf-8") as output:
            output.write("format=dragons-clutch-agave-loopback-build-v1\n")
        with self.assertRaisesRegex(VERIFY_RUNTIME.Refusal, "duplicate record key"):
            self.fixture.verify()

    def test_refuses_wrong_reported_version(self) -> None:
        self.fixture.binary.write_text("#!/bin/sh\nprintf 'wrong\\n'\n", encoding="utf-8")
        self.fixture.binary.chmod(0o755)
        digest = hashlib.sha256(self.fixture.binary.read_bytes()).hexdigest()
        text = self.fixture.manifest.read_text(encoding="utf-8")
        text = text.replace(
            next(line for line in text.splitlines() if line.startswith("binary_bytes=")),
            f"binary_bytes={self.fixture.binary.stat().st_size}",
        ).replace(
            next(line for line in text.splitlines() if line.startswith("binary_sha256=")),
            f"binary_sha256={digest}",
        )
        self.fixture.manifest.write_text(text, encoding="utf-8")
        with self.assertRaisesRegex(VERIFY_RUNTIME.Refusal, "version differs"):
            self.fixture.verify()


if __name__ == "__main__":
    unittest.main()
