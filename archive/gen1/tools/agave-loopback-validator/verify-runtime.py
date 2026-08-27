#!/usr/bin/env python3
"""Refuse any validator except the repository's provenance-checked build."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys


PIN_TO_MANIFEST = {
    "AGAVE_UPSTREAM_URL": "upstream_url",
    "AGAVE_COMMIT": "upstream_commit",
    "AGAVE_VERSION": "upstream_version",
    "AGAVE_RUST_TOOLCHAIN": "rust_toolchain",
    "AGAVE_RUSTC_COMMIT": "rustc_commit",
    "AGAVE_CARGO_LOCK_SHA256": "cargo_lock_sha256",
    "AGAVE_LICENSE_SHA256": "upstream_license_sha256",
    "AGAVE_PATCH_SHA256": "patch_sha256",
    "AGAVE_PATCHED_QUIC_CLIENT_SHA256": "patched_quic_client_sha256",
    "AGAVE_PATCHED_UDP_CLIENT_SHA256": "patched_udp_client_sha256",
    "AGAVE_PATCHED_CLI_SHA256": "patched_cli_sha256",
    "AGAVE_PATCHED_LIBRARY_SHA256": "patched_library_sha256",
}
KEY = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")


class Refusal(Exception):
    """A fail-closed runtime selection failure."""


def parse_record(path: Path) -> dict[str, str]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise Refusal(f"required provenance record is unavailable: {path}: {error}") from error
    record: dict[str, str] = {}
    for number, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise Refusal(f"malformed record line {path}:{number}")
        key, value = line.split("=", 1)
        if not KEY.fullmatch(key) or not value:
            raise Refusal(f"malformed record line {path}:{number}")
        if key in record:
            raise Refusal(f"duplicate record key {key!r} in {path}")
        record[key] = value
    return record


def require(record: dict[str, str], key: str, path: Path) -> str:
    try:
        return record[key]
    except KeyError as error:
        raise Refusal(f"required key {key!r} is absent from {path}") from error


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as error:
        raise Refusal(f"cannot hash selected validator {path}: {error}") from error
    return digest.hexdigest()


def verify(repo: Path, cache_root: Path, pins_path: Path, binary: Path) -> str:
    repo = repo.resolve(strict=True)
    cache_root = cache_root.resolve(strict=False)
    expected_binary = (cache_root / "bin/solana-test-validator").resolve(strict=False)
    selected_binary = binary.resolve(strict=False)
    if selected_binary != expected_binary:
        raise Refusal(
            "refusing validator outside the repository's pinned loopback cache: "
            f"selected {selected_binary}, required {expected_binary}; stock Agave 4.0.2 "
            "can expose wildcard RPC, WebSocket, faucet, QUIC, or UDP sockets"
        )
    if not selected_binary.is_file() or not os.access(selected_binary, os.X_OK):
        raise Refusal(
            f"pinned loopback validator is not executable: {selected_binary}; "
            f"build it with {repo / 'tools/agave-loopback-validator/build.sh'}"
        )

    manifest_path = cache_root / "build-provenance.txt"
    pins = parse_record(pins_path)
    manifest = parse_record(manifest_path)
    if require(manifest, "format", manifest_path) != "dragons-clutch-agave-loopback-build-v1":
        raise Refusal("validator build record has an unsupported format")
    for pin_key, manifest_key in PIN_TO_MANIFEST.items():
        pinned = require(pins, pin_key, pins_path)
        recorded = require(manifest, manifest_key, manifest_path)
        if recorded != pinned:
            raise Refusal(
                f"validator build provenance drift for {manifest_key}: "
                f"recorded {recorded!r}, pinned {pinned!r}"
            )
    if require(manifest, "build_profile", manifest_path) != "release":
        raise Refusal("selected validator was not recorded as a release build")
    if require(manifest, "build_mode", manifest_path) not in {"offline", "network-enabled"}:
        raise Refusal("selected validator has an unknown build mode")

    recorded_binary = Path(require(manifest, "binary_path", manifest_path)).resolve(strict=False)
    if recorded_binary != selected_binary:
        raise Refusal(
            f"validator build record names {recorded_binary}, not selected {selected_binary}"
        )
    try:
        recorded_bytes = int(require(manifest, "binary_bytes", manifest_path), 10)
    except ValueError as error:
        raise Refusal("validator build record has a non-decimal binary size") from error
    if selected_binary.stat().st_size != recorded_bytes:
        raise Refusal("selected validator byte length differs from its build record")
    actual_sha256 = file_sha256(selected_binary)
    if actual_sha256 != require(manifest, "binary_sha256", manifest_path):
        raise Refusal("selected validator SHA-256 differs from its build record")

    try:
        result = subprocess.run(
            [str(selected_binary), "--version"],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Refusal(f"cannot query selected validator version: {error}") from error
    version = result.stdout.strip()
    if result.returncode != 0 or result.stderr or "\n" in version:
        raise Refusal("selected validator did not return one clean version line")
    if version != require(manifest, "binary_version", manifest_path):
        raise Refusal("selected validator version differs from its build record")
    expected_version = require(pins, "AGAVE_VERSION", pins_path)
    expected_source = require(pins, "AGAVE_COMMIT", pins_path)[:8]
    if not version.startswith(f"solana-test-validator {expected_version} ") or (
        f"src:{expected_source}" not in version
    ):
        raise Refusal("selected validator version does not name the pinned release and source")
    return actual_sha256


def main() -> int:
    tool_dir = Path(__file__).resolve().parent
    default_repo = tool_dir.parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=default_repo, help=argparse.SUPPRESS)
    parser.add_argument("--cache-root", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--pins", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()
    repo = args.repo
    cache_root = args.cache_root or Path(
        os.environ.get(
            "CLUTCH_AGAVE_LOOPBACK_CACHE",
            str(repo / ".cache/agave-loopback-validator"),
        )
    )
    pins = args.pins or repo / "tools/agave-loopback-validator/pins.env"
    binary = args.binary or cache_root / "bin/solana-test-validator"
    try:
        digest = verify(repo, cache_root, pins, binary)
    except Refusal as error:
        print(f"agave-loopback-validator: REFUSE: {error}", file=sys.stderr)
        return 1
    print("pinned loopback validator runtime: PASS")
    print(f"binary: {binary.resolve()}")
    print(f"sha256: {digest}")
    print(f"provenance: {(cache_root / 'build-provenance.txt').resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
