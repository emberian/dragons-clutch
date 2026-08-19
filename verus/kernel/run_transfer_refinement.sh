#!/bin/sh
set -eu

# Verify one exact production arithmetic subset, then demonstrate that two
# semantic mutations make the contract go red.  This script never rewrites the
# checked-out source and leaves every generated file in a private temp dir.

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH='' cd -- "$HERE/../.." && pwd)
SOURCE="$REPO/crates/clutch-kernel/src/transfer_arithmetic.rs"
KERNEL="$REPO/crates/clutch-kernel/src/lib.rs"

VERUS_PINNED_VERSION='0.2026.08.15.7d4628a'
VERUS_PINNED_TOOLCHAIN='1.97.1-aarch64-apple-darwin'
VERUS_PREFIX_DEFAULT="$HOME/toolchains/verus-0.2026.08.15.7d4628a/verus-arm64-macos"
VERUS_PREFIX=${VERUS_PREFIX:-$VERUS_PREFIX_DEFAULT}
VERUS="$VERUS_PREFIX/verus"

SOURCE_SHA256_PIN='01a04ce8b6cf94680d0451e883a142bc580e358dd06088314f945e5ebaa72df3'
CALL_SITE_SHA256_PIN='2fe06410feeb72ce7322a57ae9efd92632c2cd167ba69dce388ace235951791e'

if [ ! -x "$VERUS" ]; then
    printf '%s\n' 'BLOCKED: pinned Verus binary is unavailable.'
    exit 2
fi

OBSERVED_VERSION=$(
    "$VERUS" --version 2>&1 | awk '/Version:/ { print $2; exit }'
)
OBSERVED_TOOLCHAIN=$(
    "$VERUS" --version 2>&1 | awk '/Toolchain:/ { print $2; exit }'
)
if [ "$OBSERVED_VERSION" != "$VERUS_PINNED_VERSION" ] ||
    [ "$OBSERVED_TOOLCHAIN" != "$VERUS_PINNED_TOOLCHAIN" ]; then
    printf '%s\n' 'BLOCKED: Verus version or frontend toolchain differs from the reviewed pin.'
    exit 3
fi

SOURCE_SHA256=$(shasum -a 256 "$SOURCE" | awk '{print $1}')
if [ "$SOURCE_SHA256" != "$SOURCE_SHA256_PIN" ]; then
    printf 'BLOCKED: production helper digest drifted: expected %s, observed %s\n' \
        "$SOURCE_SHA256_PIN" "$SOURCE_SHA256"
    exit 4
fi

if [ "$(grep -c 'VERUS-CONTRACT-ANCHOR' "$SOURCE")" -ne 1 ] ||
    [ "$(grep -Ec '^[[:space:]]*// VERUS-TRANSFER-CALLSITE-BEGIN' "$KERNEL")" -ne 1 ] ||
    [ "$(grep -Ec '^[[:space:]]*// VERUS-TRANSFER-CALLSITE-END' "$KERNEL")" -ne 1 ]; then
    printf '%s\n' 'BLOCKED: refinement anchors are missing or ambiguous.'
    exit 4
fi

CALL_SITE_SHA256=$(
    sed -n \
        '/VERUS-TRANSFER-CALLSITE-BEGIN/,/VERUS-TRANSFER-CALLSITE-END/p' \
        "$KERNEL" | shasum -a 256 | awk '{print $1}'
)
if [ "$CALL_SITE_SHA256" != "$CALL_SITE_SHA256_PIN" ]; then
    printf 'BLOCKED: production call-site digest drifted: expected %s, observed %s\n' \
        "$CALL_SITE_SHA256_PIN" "$CALL_SITE_SHA256"
    exit 4
fi

# The first-party proof artifact may not smuggle any of the repository's
# forbidden shortcuts into the exact source being verified.
if grep -En \
    'unsafe|assume|admit|axiom|external_body|assume_specification|cfg\(verus_only\)' \
    "$SOURCE"; then
    printf '%s\n' 'BLOCKED: forbidden proof or executable construct in production helper.'
    exit 5
fi

TMP=$(mktemp -d "${TMPDIR:-/tmp}/clutch-transfer-refinement.XXXXXX")
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

generate_proof() {
    input=$1
    output=$2
    {
        printf '%s\n' \
            'use vstd::prelude::*;' \
            '' \
            'verus! {' \
            'pub mod production {' \
            'use vstd::prelude::*;'
        awk '
            /^\) -> TransferArithmeticResult<\(u64, u64\)>$/ {
                print ") -> (result: TransferArithmeticResult<(u64, u64)>)"
                next
            }
            /^\/\/!/ {
                sub(/^\/\/!/, "//")
                print
                next
            }
            { print }
            /VERUS-CONTRACT-ANCHOR/ {
                print "    requires"
                print "        quantity <= from,"
                print "    ensures"
                print "        match result {"
                print "            Ok((new_from, new_to)) =>"
                print "                new_from as int == from as int - quantity as int"
                print "                && new_to as int == to as int + quantity as int"
                print "                && new_from as int + new_to as int == from as int + to as int,"
                print "            Err(TransferArithmeticError::Overflow) =>"
                print "                to as int + quantity as int > u64::MAX as int,"
                print "            Err(_) => false,"
                print "        },"
            }
        ' "$input"
        printf '%s\n' '}' '}'
    } > "$output"
}

run_expected_red() {
    label=$1
    proof=$2
    log=$3
    if "$VERUS" --crate-name clutch_transfer_mutation --edition 2021 \
        --crate-type=lib "$proof" > "$log" 2>&1; then
        printf 'FAIL mutation=%s unexpectedly verified\n' "$label"
        exit 6
    fi
    if ! grep -q 'postcondition not satisfied' "$log"; then
        printf 'FAIL mutation=%s went red for the wrong reason\n' "$label"
        tail -n 20 "$log"
        exit 6
    fi
    printf 'mutation=%s status=EXPECTED_RED reason=postcondition\n' "$label"
}

PROOF="$TMP/transfer_refinement.rs"
generate_proof "$SOURCE" "$PROOF"

if grep -En \
    'unsafe|assume|admit|axiom|external_body|assume_specification|cfg\(verus_only\)' \
    "$PROOF"; then
    printf '%s\n' 'BLOCKED: forbidden construct in generated proof unit.'
    exit 5
fi

printf 'verus_version=%s\n' "$OBSERVED_VERSION"
printf 'verus_toolchain=%s\n' "$OBSERVED_TOOLCHAIN"
printf 'production_source_sha256=%s\n' "$SOURCE_SHA256"
printf 'production_call_site_sha256=%s\n' "$CALL_SITE_SHA256"
printf 'generated_proof_sha256=%s\n' \
    "$(shasum -a 256 "$PROOF" | awk '{print $1}')"

"$VERUS" --crate-name clutch_transfer_refinement --edition 2021 \
    --crate-type=lib "$PROOF"

awk '
    !done && /checked_add\(quantity\)/ {
        sub(/checked_add\(quantity\)/, "checked_sub(quantity)")
        done = 1
    }
    { print }
    END { if (!done) exit 7 }
' "$SOURCE" > "$TMP/delta_mutation.rs"
generate_proof "$TMP/delta_mutation.rs" "$TMP/delta_mutation_proof.rs"
run_expected_red delta-direction "$TMP/delta_mutation_proof.rs" "$TMP/delta.log"

awk '
    !done && /before != after/ {
        sub(/before != after/, "before == after")
        done = 1
    }
    { print }
    END { if (!done) exit 7 }
' "$SOURCE" > "$TMP/conservation_mutation.rs"
generate_proof "$TMP/conservation_mutation.rs" "$TMP/conservation_mutation_proof.rs"
run_expected_red conservation-guard "$TMP/conservation_mutation_proof.rs" "$TMP/conservation.log"

printf '%s\n' 'status=PASS'
printf '%s\n' 'claim=exact production transfer arithmetic under quantity<=from'
printf '%s\n' 'boundary=call-site digest reviewed; MarketState/Position/Solana/SBF are not verified here'
