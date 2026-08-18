#!/usr/bin/env bash
# Build the Dragon's Clutch program ELF and drive it against real Token-2022.
#
# Everything this touches is local: `cargo-build-sbf` produces the ELF, and an
# in-process Agave bank started by `solana-program-test` executes it with the
# Token-2022 program `solana-program-binaries` installs at genesis.  No RPC, no
# cluster, no key material, no submission, no port.
#
# The ELF is staged into `tests/fixtures/` and is deliberately NOT committed: a
# checked-in binary is a second copy of the program that goes stale silently.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
program="$(cd "$here/.." && pwd)"

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"

echo "== building the program ELF =="
mkdir -p "$here/tests/fixtures"
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="${SBF_TARGET_DIR:-$program/target/sbf-build}" \
  "$build_sbf" --manifest-path "$program/program/Cargo.toml" \
  --sbf-out-dir "$here/tests/fixtures"
shasum -a 256 "$here/tests/fixtures/clutch_sbf.so"

echo
echo "== driving it =="
cd "$here"
export RUST_LOG="${RUST_LOG:-error}"
cargo test --locked -- --nocapture --test-threads=1 "$@"
