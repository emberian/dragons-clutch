#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$here/common.sh"

usage() {
  echo "usage: $0 [--allow-network]" >&2
  exit 2
}

cargo_network_args=(--offline)
build_mode=offline
case "${1:-}" in
  "") ;;
  --allow-network)
    cargo_network_args=()
    build_mode=network-enabled
    ;;
  *) usage ;;
esac
[ "$#" -le 1 ] || usage

require_command git
require_command install
require_command shasum
require_command awk
require_command rg
verify_checkout
require_pinned_rust

if [ -n "${CLUTCH_AGAVE_LIBCLANG_PATH:-}" ]; then
  libclang_dir="$CLUTCH_AGAVE_LIBCLANG_PATH"
elif command -v brew >/dev/null 2>&1 &&
     [ -f "$(brew --prefix llvm 2>/dev/null)/lib/libclang.dylib" ]; then
  libclang_dir="$(brew --prefix llvm)/lib"
elif command -v xcrun >/dev/null 2>&1 &&
     [ -f "$(xcrun --show-toolchain-path)/usr/lib/libclang.dylib" ]; then
  libclang_dir="$(xcrun --show-toolchain-path)/usr/lib"
else
  die "libclang.dylib not found; set CLUTCH_AGAVE_LIBCLANG_PATH to its directory"
fi
[ -f "$libclang_dir/libclang.dylib" ] ||
  die "libclang.dylib missing from $libclang_dir"
libclang_sha256="$(sha256_file "$libclang_dir/libclang.dylib")"

mkdir -p "$cargo_home" "$target_dir" "$bin_dir"
start_epoch="$(date +%s)"
start_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
source_date_epoch="$(git -C "$source_dir" show -s --format=%ct "$AGAVE_COMMIT")"

unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER
export CARGO_HOME="$cargo_home"
export CARGO_TARGET_DIR="$target_dir"
export CARGO_INCREMENTAL=0
export SOURCE_DATE_EPOCH="$source_date_epoch"
export LIBCLANG_PATH="$libclang_dir"
# clang-sys links its build helper to @rpath/libclang.dylib on macOS.
export DYLD_LIBRARY_PATH="$libclang_dir"

rustup run "$AGAVE_RUST_TOOLCHAIN" cargo build \
  "${cargo_network_args[@]}" \
  --locked \
  --release \
  --manifest-path "$source_dir/Cargo.toml" \
  -p agave-validator \
  --bin solana-test-validator

built_binary="$target_dir/release/solana-test-validator"
[ -x "$built_binary" ] || die "Cargo completed without $built_binary"
binary_version="$($built_binary --version)"
printf '%s\n' "$binary_version" | rg -q "solana-test-validator $AGAVE_VERSION" ||
  die "built binary has unexpected version: $binary_version"
printf '%s\n' "$binary_version" | rg -q 'src:549805f3' ||
  die "built binary does not report the pinned source prefix: $binary_version"

install -m 0755 "$built_binary" "$binary_path"
binary_sha256="$(sha256_file "$binary_path")"
binary_bytes="$(stat -f '%z' "$binary_path")"
finish_epoch="$(date +%s)"
finish_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
elapsed_seconds="$((finish_epoch - start_epoch))"
manifest_tmp="$manifest_path.tmp.$$"

{
  echo "format=dragons-clutch-agave-loopback-build-v1"
  echo "upstream_url=$AGAVE_UPSTREAM_URL"
  echo "upstream_commit=$AGAVE_COMMIT"
  echo "upstream_version=$AGAVE_VERSION"
  echo "rust_toolchain=$AGAVE_RUST_TOOLCHAIN"
  echo "rustc_commit=$AGAVE_RUSTC_COMMIT"
  echo "cargo_lock_sha256=$AGAVE_CARGO_LOCK_SHA256"
  echo "upstream_license_sha256=$AGAVE_LICENSE_SHA256"
  echo "patch_sha256=$AGAVE_PATCH_SHA256"
  echo "patched_quic_client_sha256=$AGAVE_PATCHED_QUIC_CLIENT_SHA256"
  echo "patched_udp_client_sha256=$AGAVE_PATCHED_UDP_CLIENT_SHA256"
  echo "patched_cli_sha256=$AGAVE_PATCHED_CLI_SHA256"
  echo "patched_library_sha256=$AGAVE_PATCHED_LIBRARY_SHA256"
  echo "build_mode=$build_mode"
  echo "build_profile=release"
  echo "source_date_epoch=$SOURCE_DATE_EPOCH"
  echo "libclang_path=$libclang_dir/libclang.dylib"
  echo "libclang_sha256=$libclang_sha256"
  echo "started_utc=$start_utc"
  echo "finished_utc=$finish_utc"
  echo "elapsed_seconds=$elapsed_seconds"
  echo "host=$(uname -m)-$(uname -s)"
  echo "binary_path=$binary_path"
  echo "binary_bytes=$binary_bytes"
  echo "binary_sha256=$binary_sha256"
  echo "binary_version=$binary_version"
} >"$manifest_tmp"
mv "$manifest_tmp" "$manifest_path"

echo "binary: $binary_path"
echo "sha256: $binary_sha256"
echo "provenance: $manifest_path"
echo "elapsed_seconds: $elapsed_seconds"
