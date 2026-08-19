#!/bin/sh
set -eu

# Verify the mathematical scalar-batch shadow, refuse production-seam drift,
# and require four semantic mutants to fail. Generated mutants live only in a
# private temporary directory; this script never rewrites production source.

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH='' cd -- "$HERE/../.." && pwd)
PROOF="$HERE/batch.rs"
PRODUCTION="$REPO/crates/clutch-batch/src/lib.rs"
COUPLED="$REPO/crates/clutch-batch/src/relation_v1.rs"
STREAM="$REPO/crates/clutch-batch/src/relation_v1_stream.rs"

VERUS_PINNED_VERSION='0.2026.08.15.7d4628a'
VERUS_PINNED_COMMIT='7d4628a8543d3e51e6e314c52032c9bab43f0f53'
VERUS_PINNED_TOOLCHAIN='1.97.1-aarch64-apple-darwin'
VERUS_PINNED_SHA256='07b3859fc335fd9bf803323baf82584dcefb62d74329561875353f5fde93fe8b'
Z3_PINNED_SHA256='edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb'
VSTD_RLIB_PINNED_SHA256='91f842643674660a1da8799eda3eceaa37e5816682d026b3d860bc59a81be61c'
VSTD_VIR_PINNED_SHA256='3061321483841360c760fa3fb38b22be57533052cf2100eb51956aa31205fa7d'
VERUS_PREFIX_DEFAULT="$HOME/toolchains/verus-0.2026.08.15.7d4628a/verus-arm64-macos"
VERUS_PREFIX=${VERUS_PREFIX:-$VERUS_PREFIX_DEFAULT}
VERUS="$VERUS_PREFIX/verus"
Z3="$VERUS_PREFIX/z3"
VSTD_RLIB="$VERUS_PREFIX/libvstd.rlib"
VSTD_VIR="$VERUS_PREFIX/vstd.vir"

PROOF_SHA256_PIN='6dce0d76961a74fe3433edf5b2830366280b2393fa5c013d43e95d5b4355e052'
PRODUCTION_SHA256_PIN='f25ce5524a71f9e8ad5200992bb69290444865243f26040906d7aa6798013249'
PRICE_GRID_IMPL_SHA256_PIN='216ad4b7db7967c71206043a1b7775ef0a25df09e8c64329d8a8ff163fb11ae9'
FIXED_BOOK_IMPL_SHA256_PIN='6d41c75a1218aa4485730fdf4526143b9b370d364d14535bb5d52c74c51b3c16'
CANDIDATE_SHA256_PIN='7dcacb6d06f6702e282f18ab152c7989df635f1c182a5606a4913026e863ce00'
COUPLED_SHA256_PIN='f95b4931414386f109ef52b844616f86f11e21121d6f9ef8901f18b77eafc490'
STREAM_SHA256_PIN='c196c096c75adfb85397eda5e5d905dde89349ab06512e5ad02d345d75fbf358'

sha256_file() {
    shasum -a 256 "$1" | awk '{print $1}'
}

refuse_drift() {
    label=$1
    expected=$2
    observed=$3
    if [ "$observed" != "$expected" ]; then
        printf 'BLOCKED: %s digest drifted: expected %s, observed %s\n' \
            "$label" "$expected" "$observed"
        exit 4
    fi
}

if [ ! -x "$VERUS" ] || [ ! -x "$Z3" ] || \
    [ ! -f "$VSTD_RLIB" ] || [ ! -f "$VSTD_VIR" ]; then
    printf '%s\n' 'BLOCKED: pinned local Verus, bundled Z3, or pinned vstd artifacts are unavailable.'
    exit 2
fi

OBSERVED_VERSION=$(
    "$VERUS" --version 2>&1 | awk '/Version:/ { print $2; exit }'
)
OBSERVED_TOOLCHAIN=$(
    "$VERUS" --version 2>&1 | awk '/Toolchain:/ { print $2; exit }'
)
if [ "$OBSERVED_VERSION" != "$VERUS_PINNED_VERSION" ] || \
    [ "$OBSERVED_TOOLCHAIN" != "$VERUS_PINNED_TOOLCHAIN" ]; then
    printf '%s\n' 'BLOCKED: Verus version or frontend toolchain differs from the reviewed pin.'
    exit 3
fi

VERUS_SHA256=$(sha256_file "$VERUS")
Z3_SHA256=$(sha256_file "$Z3")
VSTD_RLIB_SHA256=$(sha256_file "$VSTD_RLIB")
VSTD_VIR_SHA256=$(sha256_file "$VSTD_VIR")
refuse_drift verus-binary "$VERUS_PINNED_SHA256" "$VERUS_SHA256"
refuse_drift z3-binary "$Z3_PINNED_SHA256" "$Z3_SHA256"
refuse_drift vstd-rlib "$VSTD_RLIB_PINNED_SHA256" "$VSTD_RLIB_SHA256"
refuse_drift vstd-vir "$VSTD_VIR_PINNED_SHA256" "$VSTD_VIR_SHA256"

PROOF_SHA256=$(sha256_file "$PROOF")
PRODUCTION_SHA256=$(sha256_file "$PRODUCTION")
COUPLED_SHA256=$(sha256_file "$COUPLED")
STREAM_SHA256=$(sha256_file "$STREAM")
PRICE_GRID_IMPL_SHA256=$(
    awk '/^impl PriceGrid \{/,/^\}/' "$PRODUCTION" | shasum -a 256 | awk '{print $1}'
)
FIXED_BOOK_IMPL_SHA256=$(
    awk '/^impl FixedBook \{/,/^\}/' "$PRODUCTION" | shasum -a 256 | awk '{print $1}'
)
CANDIDATE_SHA256=$(
    awk '/^pub struct Candidate \{/,/^\}/' "$PRODUCTION" | shasum -a 256 | awk '{print $1}'
)

refuse_drift proof-source "$PROOF_SHA256_PIN" "$PROOF_SHA256"
refuse_drift scalar-production-source "$PRODUCTION_SHA256_PIN" "$PRODUCTION_SHA256"
refuse_drift price-grid-impl "$PRICE_GRID_IMPL_SHA256_PIN" "$PRICE_GRID_IMPL_SHA256"
refuse_drift fixed-book-impl "$FIXED_BOOK_IMPL_SHA256_PIN" "$FIXED_BOOK_IMPL_SHA256"
refuse_drift candidate-struct "$CANDIDATE_SHA256_PIN" "$CANDIDATE_SHA256"
refuse_drift coupled-relation-excluded-source "$COUPLED_SHA256_PIN" "$COUPLED_SHA256"
refuse_drift streaming-relation-excluded-source "$STREAM_SHA256_PIN" "$STREAM_SHA256"

if grep -En \
    'unsafe|assume[[:space:]]*\(|admit|axiom|external_body|assume_specification|cfg\(verus_only\)' \
    "$PROOF"; then
    printf '%s\n' 'BLOCKED: forbidden proof or executable construct in batch proof source.'
    exit 5
fi

TMP=$(mktemp -d "${TMPDIR:-/tmp}/clutch-batch-proof.XXXXXX")
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

run_expected_red() {
    label=$1
    mutant=$2
    expected=$3
    reason=$4
    log="$TMP/$label.log"
    if "$VERUS" --crate-name clutch_batch_mutant --edition 2021 \
        --crate-type=lib "$mutant" > "$log" 2>&1; then
        printf 'FAIL mutation=%s unexpectedly verified\n' "$label"
        exit 6
    fi
    if ! grep -Eq "$expected" "$log"; then
        printf 'FAIL mutation=%s went red for the wrong reason\n' "$label"
        tail -n 30 "$log"
        exit 6
    fi
    printf 'mutation=%s status=EXPECTED_RED reason=%s\n' "$label" "$reason"
}

awk '
    !done && /if selected\[index\] \{ 1int \} else \{ 0int \}/ {
        sub(/\{ 1int \}/, "{ 2int }")
        done = 1
    }
    { print }
    END { if (!done) exit 7 }
' "$PROOF" > "$TMP/allocation-mutant.rs"

awk '
    !done && /if at_least_as_good\(volumes, imbalances, challenger, previous\)/ {
        sub(/challenger, previous/, "previous, challenger")
        done = 1
    }
    { print }
    END { if (!done) exit 7 }
' "$PROOF" > "$TMP/tick-mutant.rs"

awk '
    !done && /if buy_side\[index\] == want_buy \{ fills\[index\] \} else \{ 0 \}/ {
        sub(/else \{ 0 \}/, "else { fills[index] }")
        done = 1
    }
    { print }
    END { if (!done) exit 7 }
' "$PROOF" > "$TMP/relation-mutant.rs"

awk '
    /pub open spec fn canonical_padding\(/ { in_padding = 1 }
    in_padding && !done && /values\[i\] == 0/ {
        sub(/values\[i\] == 0/, "values[i] >= 0")
        done = 1
    }
    { print }
    END { if (!done) exit 7 }
' "$PROOF" > "$TMP/padding-mutant.rs"

printf 'verus_version=%s\n' "$OBSERVED_VERSION"
printf 'verus_commit=%s\n' "$VERUS_PINNED_COMMIT"
printf 'verus_toolchain=%s\n' "$OBSERVED_TOOLCHAIN"
printf 'verus_binary_sha256=%s\n' "$VERUS_SHA256"
printf 'z3_binary_sha256=%s\n' "$Z3_SHA256"
printf 'vstd_rlib_sha256=%s\n' "$VSTD_RLIB_SHA256"
printf 'vstd_vir_sha256=%s\n' "$VSTD_VIR_SHA256"
printf 'proof_source_sha256=%s\n' "$PROOF_SHA256"
printf 'scalar_production_source_sha256=%s\n' "$PRODUCTION_SHA256"
printf 'price_grid_impl_sha256=%s\n' "$PRICE_GRID_IMPL_SHA256"
printf 'fixed_book_impl_sha256=%s\n' "$FIXED_BOOK_IMPL_SHA256"
printf 'candidate_struct_sha256=%s\n' "$CANDIDATE_SHA256"
printf 'excluded_coupled_relation_sha256=%s\n' "$COUPLED_SHA256"
printf 'excluded_stream_relation_sha256=%s\n' "$STREAM_SHA256"
printf '%s\n' 'command=verus --crate-name clutch_batch_shadow --edition 2021 --crate-type=lib verus/batch/batch.rs'

"$VERUS" --crate-name clutch_batch_shadow --edition 2021 --crate-type=lib "$PROOF"

run_expected_red allocation-double-selected-atom "$TMP/allocation-mutant.rs" \
    'postcondition not satisfied' postcondition
run_expected_red tick-select-worse "$TMP/tick-mutant.rs" \
    'precondition not satisfied|assertion failed' invariant-obligation
run_expected_red relation-double-count "$TMP/relation-mutant.rs" \
    'postcondition not satisfied' postcondition
run_expected_red padding-admit-nonzero "$TMP/padding-mutant.rs" \
    'assertion failed' zero-premise-obligation

printf '%s\n' 'status=PASS'
printf '%s\n' 'claim=scalar shadow: allocation decomposition/bounds, unique tick, accepted-side partition, zero-suffix fold identity'
printf '%s\n' 'boundary=digest-pinned correspondence review only; production body is not imported by Verus'
printf '%s\n' 'excluded=relation_v1 relation_v1_stream Solana SBF accounts serialization deployment'
