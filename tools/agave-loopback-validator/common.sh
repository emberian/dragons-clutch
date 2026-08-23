#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Shared paths are consumed by sourcing scripts.
# shellcheck disable=SC2034

set -euo pipefail

tool_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(git -C "$tool_dir" rev-parse --show-toplevel)"
# pins.env is repository-owned data, not user configuration.
# shellcheck source-path=SCRIPTDIR
# shellcheck source=pins.env
source "$tool_dir/pins.env"

cache_root="${CLUTCH_AGAVE_LOOPBACK_CACHE:-$repo_root/.cache/agave-loopback-validator}"
source_dir="$cache_root/source"
cargo_home="$cache_root/cargo-home"
target_dir="$cache_root/target"
bin_dir="$cache_root/bin"
binary_path="$bin_dir/solana-test-validator"
manifest_path="$cache_root/build-provenance.txt"
patch_path="$tool_dir/agave-4.0.2-loopback.patch"

die() {
  echo "agave-loopback-validator: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

sha256_file() {
  shasum -a 256 "$1" | awk '{print $1}'
}

require_sha256() {
  local path="$1"
  local expected="$2"
  local actual
  [ -f "$path" ] || die "required file missing: $path"
  actual="$(sha256_file "$path")"
  [ "$actual" = "$expected" ] ||
    die "SHA-256 mismatch for $path: expected $expected, got $actual"
}

require_pinned_rust() {
  local rustc_verbose
  require_command rustup
  rustc_verbose="$(rustup run "$AGAVE_RUST_TOOLCHAIN" rustc --version --verbose)" ||
    die "Rust $AGAVE_RUST_TOOLCHAIN is not installed"
  printf '%s\n' "$rustc_verbose" | rg -qx "release: $AGAVE_RUST_TOOLCHAIN" ||
    die "rustc release is not $AGAVE_RUST_TOOLCHAIN"
  printf '%s\n' "$rustc_verbose" | rg -qx "commit-hash: $AGAVE_RUSTC_COMMIT" ||
    die "rustc commit does not match pinned $AGAVE_RUSTC_COMMIT"
}

verify_checkout() {
  local head origin changed untracked
  [ -d "$source_dir/.git" ] || die "source checkout missing; run fetch-source.sh"
  head="$(git -C "$source_dir" rev-parse HEAD)"
  [ "$head" = "$AGAVE_COMMIT" ] ||
    die "source HEAD is $head, expected $AGAVE_COMMIT"
  origin="$(git -C "$source_dir" config --get remote.origin.url || true)"
  [ "$origin" = "$AGAVE_UPSTREAM_URL" ] ||
    die "source origin is $origin, expected $AGAVE_UPSTREAM_URL"

  require_sha256 "$patch_path" "$AGAVE_PATCH_SHA256"
  require_sha256 "$source_dir/Cargo.lock" "$AGAVE_CARGO_LOCK_SHA256"
  require_sha256 "$source_dir/LICENSE" "$AGAVE_LICENSE_SHA256"
  require_sha256 \
    "$source_dir/quic-client/src/nonblocking/quic_client.rs" \
    "$AGAVE_PATCHED_QUIC_CLIENT_SHA256"
  require_sha256 \
    "$source_dir/udp-client/src/lib.rs" \
    "$AGAVE_PATCHED_UDP_CLIENT_SHA256"
  require_sha256 \
    "$source_dir/validator/src/bin/solana-test-validator.rs" \
    "$AGAVE_PATCHED_CLI_SHA256"
  require_sha256 \
    "$source_dir/test-validator/src/lib.rs" \
    "$AGAVE_PATCHED_LIBRARY_SHA256"

  git -C "$source_dir" diff --cached --quiet -- ||
    die "source checkout has staged changes"
  git -C "$source_dir" diff --check || die "source patch has whitespace errors"
  untracked="$(git -C "$source_dir" ls-files --others --exclude-standard)"
  [ -z "$untracked" ] || die "source checkout has untracked files: $untracked"
  changed="$(git -C "$source_dir" diff --name-only -- | LC_ALL=C sort)"
  [ "$changed" = $'quic-client/src/nonblocking/quic_client.rs\ntest-validator/src/lib.rs\nudp-client/src/lib.rs\nvalidator/src/bin/solana-test-validator.rs' ] ||
    die "source checkout has unexpected modified paths: $changed"
  git -C "$source_dir" apply --unidiff-zero --reverse --check "$patch_path" ||
    die "source checkout is not exactly reversible by the pinned patch"
}
