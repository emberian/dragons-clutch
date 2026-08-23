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
#
# Default usage builds the production-inert ELF: it contains one unreachable
# off-curve fixture release and no production release. V1 mock source/value
# scenarios require the explicit, differently compiled laboratory profile:
#   ./run_svm_tests.sh --non-production-mock-source [test filters ...]
# The deployed-Pyth local campaign is a separate, explicit test-only ELF:
#   ./run_svm_tests.sh --non-production-real-pyth-lab real_pyth_router_verifies_then_post_update
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
program="$(cd "$here/.." && pwd)"

profile="default-production-inert"
build_features=()
test_features=()
build_default=()
test_default=()
if [ "${1:-}" = "--non-production-mock-source" ]; then
  shift
  profile="NON-PRODUCTION-non-production-mock-source"
  build_features=(--features non-production-mock-source)
  test_features=(--features non-production-mock-source)
elif [ "${1:-}" = "--non-production-real-pyth-lab" ]; then
  shift
  profile="NON-PRODUCTION-non-production-real-pyth-lab"
  build_features=(--features non-production-real-pyth-lab)
  test_features=(--features non-production-real-pyth-lab)
elif [ "${1:-}" = "--profile-non-production-dealer-policy-catalog-lab" ]; then
  shift
  profile="NON-PRODUCTION-dealer-policy-catalog-lab"
  build_default=(--no-default-features)
  test_default=(--no-default-features)
  build_features=(--features custom-heap,profile-non-production-dealer-policy-catalog-lab)
  test_features=(--features profile-non-production-dealer-policy-catalog-lab)
fi

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"

echo "== SVM profile: $profile =="
echo "== building the program ELF =="
mkdir -p "$here/tests/fixtures"
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="${SBF_TARGET_DIR:-$program/target/sbf-build}" \
  "$build_sbf" --manifest-path "$program/program/Cargo.toml" \
  "${build_default[@]}" "${build_features[@]}" --sbf-out-dir "$here/tests/fixtures"
# The laboratory receiver writes a canonical 134-byte update immediately
# before append. It is not a provider-proof model; it exists so the bank proves
# the real write/consume/rollback transaction seam rather than reading bytes
# installed by the host harness. See `r2_v2_wire.rs`.
echo "== building the laboratory receiver writer =="
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="${LAB_TARGET_DIR:-$program/target/lab-receiver-build}" \
  "$build_sbf" --manifest-path "$here/lab-receiver/Cargo.toml" \
  --sbf-out-dir "$here/tests/fixtures"

elf="$here/tests/fixtures/clutch_sbf.so"
elf_hash="$(shasum -a 256 "$elf" | awk '{print $1}')"
elf_size="$(wc -c < "$elf" | tr -d ' ')"
profile_file="$here/tests/fixtures/clutch_sbf.profile"
printf '%s\n' \
  "source_profile=$profile" \
  "elf_sha256=$elf_hash" \
  "elf_bytes=$elf_size" > "$profile_file"
echo "source_profile=$profile"
echo "elf_sha256=$elf_hash"
echo "elf_bytes=$elf_size"

# The feature selecting the tests and the just-built fixture come from the
# same branch above. Refuse a stale, replaced, or relabelled fixture before a
# bank starts rather than attributing one profile's result to the other.
grep -Fxq "source_profile=$profile" "$profile_file"
grep -Fxq "elf_sha256=$elf_hash" "$profile_file"
[ "$(shasum -a 256 "$elf" | awk '{print $1}')" = "$elf_hash" ]

echo
echo "== driving SVM profile: $profile =="
cd "$here"
export RUST_LOG="${RUST_LOG:-error}"
cargo test --locked "${test_default[@]}" "${test_features[@]}" -- --nocapture --test-threads=1 "$@"
