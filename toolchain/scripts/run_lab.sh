#!/bin/sh
set -eu

# Offline E0 compatibility lab. This script never contacts a cluster, reads a
# key, signs, deploys, or modifies repository files. Build outputs go to a
# temporary directory and are intentionally left behind for inspection.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
VERSIONS_FILE="$REPO_DIR/toolchain/versions.env"
PROBE_MANIFEST="$REPO_DIR/toolchain/probes/no_std_core/Cargo.toml"
HOST_MANIFEST="$REPO_DIR/toolchain/probes/host_harness/Cargo.toml"
SOURCE_FILE="$REPO_DIR/toolchain/probes/no_std_core/src/lib.rs"

# shellcheck disable=SC1090
. "$VERSIONS_FILE"

LAB_TMP=$(mktemp -d "${TMPDIR:-/tmp}/dragon-clutch-toolchain.XXXXXX")
HOST_TARGET="$LAB_TMP/host-target"
SBF_TARGET="$LAB_TMP/sbf-target"
SBF_TARGET_2="$LAB_TMP/sbf-target-2"
HOST_LOG="$LAB_TMP/host.log"
SBF_LOG="$LAB_TMP/sbf.log"
SBF_LOG_2="$LAB_TMP/sbf-2.log"

say() { printf '%s\n' "$*"; }
fail() { say "FAIL: $*" >&2; exit 1; }

command -v rustup >/dev/null 2>&1 || fail "rustup is required"
command -v cargo-build-sbf >/dev/null 2>&1 || fail "cargo-build-sbf is required"
command -v shasum >/dev/null 2>&1 || fail "shasum is required"

rustup toolchain list | grep -F "$HOST_RUST_TOOLCHAIN" >/dev/null 2>&1 \
  || fail "pinned host toolchain is not installed: $HOST_RUST_TOOLCHAIN"

host_rustc() {
    RUSTUP_SKIP_UPDATE_CHECK=1 rustup run "$HOST_RUST_TOOLCHAIN" rustc "$@"
}
host_cargo() {
    RUSTUP_SKIP_UPDATE_CHECK=1 CARGO_NET_OFFLINE=true \
      rustup run "$HOST_RUST_TOOLCHAIN" cargo "$@"
}

source_hash=$(shasum -a 256 "$SOURCE_FILE" | awk '{print $1}')
say "lab_schema=$LAB_SCHEMA"
say "source_sha256=$source_hash"
say "source=$SOURCE_FILE"
say "host_toolchain=$HOST_RUST_TOOLCHAIN"
say "host_rustc=$(host_rustc --version)"
say "host_rustc_verbose=$(host_rustc --version --verbose | tr '\n' ';')"
say "sbf_build=$(cargo-build-sbf --version 2>&1 | head -n 1)"
if command -v solana >/dev/null 2>&1; then
    say "solana=$(solana --version 2>&1 | head -n 1)"
else
    say "solana=UNAVAILABLE (cargo-build-sbf is present)"
fi
if command -v z3 >/dev/null 2>&1; then
    say "z3=$(z3 --version 2>&1 | head -n 1)"
else
    say "z3=UNAVAILABLE"
fi

say "host_build=START"
if ! CARGO_TARGET_DIR="$HOST_TARGET" host_cargo run \
    --manifest-path "$HOST_MANIFEST" --offline >"$HOST_LOG" 2>&1; then
    sed -n '1,160p' "$HOST_LOG" >&2
    fail "host probe failed"
fi
grep -Fx 'probe-ok' "$HOST_LOG" >/dev/null \
  || fail "host probe did not produce its assertion marker"
say "host_build=PASS"
host_rlib=$(find "$HOST_TARGET" -type f -name 'libdragon_clutch_toolchain_probe-*.rlib' | head -n 1)
test -n "$host_rlib" || fail "host rlib not found"
say "host_rlib_sha256=$(shasum -a 256 "$host_rlib" | awk '{print $1}')"
say "host_rlib_bytes=$(wc -c < "$host_rlib" | tr -d ' ')"

say "sbf_build=START"
if ! CARGO_TARGET_DIR="$SBF_TARGET" CARGO_NET_OFFLINE=true \
    cargo-build-sbf --manifest-path "$PROBE_MANIFEST" --no-default-features \
    >"$SBF_LOG" 2>&1; then
    sed -n '1,200p' "$SBF_LOG" >&2
    fail "SBF probe failed"
fi
say "sbf_build=PASS"
sbf_rlib=$(find "$SBF_TARGET" -type f -name 'libdragon_clutch_toolchain_probe.rlib' | head -n 1)
test -n "$sbf_rlib" || fail "SBF rlib not found"
sbf_hash=$(shasum -a 256 "$sbf_rlib" | awk '{print $1}')
say "sbf_rlib_sha256=$sbf_hash"
say "sbf_rlib_bytes=$(wc -c < "$sbf_rlib" | tr -d ' ')"

say "sbf_reproducibility=START"
if ! CARGO_TARGET_DIR="$SBF_TARGET_2" CARGO_NET_OFFLINE=true \
    cargo-build-sbf --manifest-path "$PROBE_MANIFEST" --no-default-features \
    >"$SBF_LOG_2" 2>&1; then
    sed -n '1,200p' "$SBF_LOG_2" >&2
    fail "second SBF probe failed"
fi
sbf_rlib_2=$(find "$SBF_TARGET_2" -type f -name 'libdragon_clutch_toolchain_probe.rlib' | head -n 1)
test -n "$sbf_rlib_2" || fail "second SBF rlib not found"
sbf_hash_2=$(shasum -a 256 "$sbf_rlib_2" | awk '{print $1}')
say "sbf_reproducibility=$(test "$sbf_hash" = "$sbf_hash_2" && printf PASS || printf FAIL)"
say "sbf_rlib_sha256_second=$sbf_hash_2"

say "prohibited_source_scan=START"
if grep -nE 'unsafe|extern[[:space:]]+"|cfg[[:space:]]*\([^)]*verus_only|assume|admit|external_body|assume_specification' "$SOURCE_FILE"; then
    fail "prohibited construct found in probe source"
fi
say "prohibited_source_scan=PASS"

if command -v verus >/dev/null 2>&1; then
    verus_state=NOT_RUN
    say "verus=AVAILABLE"
    say "verus_version=$(verus --version 2>&1 | head -n 1)"
    say "verus_probe=NOT_RUN (invocation is pinned in the report only after the exact local Verus release is selected)"
else
    verus_state=BLOCKED
    say "verus=UNAVAILABLE"
    say "verus_probe=BLOCKED"
fi

say "compatibility=HOST_AND_SBF_PASS_VERUS_$verus_state"
say "temporary_outputs=$LAB_TMP"
