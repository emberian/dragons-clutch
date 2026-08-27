#!/bin/sh
set -eu

# Compare live Lean-model rows with the exact production Rust entry point,
# then require five temporary production-source mutations to disagree.  This
# is a digest-bound executable refinement campaign, not a universal Verus or
# Lean theorem about Rust.  It never rewrites the checked-out source.

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH='' cd -- "$HERE/../.." && pwd)
CRATE="$REPO/crates/clutch-bspline"
SOURCE="$CRATE/src/lib.rs"
DRIVER="$CRATE/examples/oracle_driver.rs"
LEAN_SOURCE="$REPO/lean/DragonsClutch/BSpline.lean"
LEAN_EMITTER="$HERE/emit_fixtures.lean"
LEAN_ARCHIVE="$HERE/evidence/lean_fixtures.txt"
PRODUCTION_ARCHIVE="$HERE/evidence/production_outputs.txt"

LEAN_VERSION_PIN='4.33.0'
LEAN_COMMIT_PIN='d8b18978322de05a8f3dba51ef03cf5461676c17'
RUST_RELEASE_PIN='1.98.0-nightly'
RUST_COMMIT_PIN='91fe22da8084a1c9e993d78d4a56f22ab8396236'

SOURCE_SHA256_PIN='220de128366a8311de6579c0ce334a64c97620159eaf9570f61fa10fabb6de92'
DRIVER_SHA256_PIN='c74ecec10c36fbebc3ab3335f2f933de6ecbaae4e061615f9e0822a917888dc7'
CARGO_SHA256_PIN='d993057affd6a9ba58a698e59e109ab882353456294ba57712c6cfac378b1c0d'
LOCK_SHA256_PIN='e49289a908b01a9032b096cfea0499f4a902714abf9475b91b55446d0ab43edd'
LEAN_SOURCE_SHA256_PIN='3e2961e765cc0aeebe232bb2b4e9667b036fc06a22e5b0960873cc51a91d52bc'
LEAN_EMITTER_SHA256_PIN='5f732e09b232c2bedb665f2670530b76de21a14f162fa508d999d9efe0288337'
LEAN_ARCHIVE_SHA256_PIN='017afe06dfed89e45a802060b701daf2a00e4e6fc28aecd73ecbdd108c1274f0'
PRODUCTION_ARCHIVE_SHA256_PIN='eae31ce1e369e25a60883fd2d5206b56a14acc0cff072d7d2c3de1acb7da3814'

digest() {
    shasum -a 256 "$1" | awk '{print $1}'
}

require_digest() {
    label=$1
    path=$2
    expected=$3
    observed=$(digest "$path")
    if [ "$observed" != "$expected" ]; then
        printf 'BLOCKED: %s digest drifted: expected %s, observed %s\n' \
            "$label" "$expected" "$observed"
        exit 4
    fi
}

require_digest production-source "$SOURCE" "$SOURCE_SHA256_PIN"
require_digest production-driver "$DRIVER" "$DRIVER_SHA256_PIN"
require_digest crate-manifest "$CRATE/Cargo.toml" "$CARGO_SHA256_PIN"
require_digest dependency-lock "$CRATE/Cargo.lock" "$LOCK_SHA256_PIN"
require_digest lean-model "$LEAN_SOURCE" "$LEAN_SOURCE_SHA256_PIN"
require_digest lean-emitter "$LEAN_EMITTER" "$LEAN_EMITTER_SHA256_PIN"
require_digest lean-transcript "$LEAN_ARCHIVE" "$LEAN_ARCHIVE_SHA256_PIN"
require_digest production-transcript "$PRODUCTION_ARCHIVE" "$PRODUCTION_ARCHIVE_SHA256_PIN"

if [ "$(grep -Ec '^[[:space:]]*pub fn evaluate\(&self, value: u128\)' "$SOURCE")" -ne 1 ] ||
    [ "$(grep -Ec '^def bsplineRefinementFixtures' "$LEAN_SOURCE")" -ne 1 ]; then
    printf '%s\n' 'BLOCKED: the production or Lean refinement seam is missing or ambiguous.'
    exit 4
fi

if grep -En '^[[:space:]]*(sorry|admit|axiom)([[:space:]]|$)|native_decide' \
    "$LEAN_SOURCE" "$LEAN_EMITTER"; then
    printf '%s\n' 'BLOCKED: forbidden proof shortcut in the checked Lean refinement closure.'
    exit 5
fi

LEAN_VERSION=$(cd "$REPO/lean" && lake env lean --version)
case "$LEAN_VERSION" in
    *"version $LEAN_VERSION_PIN,"*"commit $LEAN_COMMIT_PIN"*) ;;
    *)
        printf 'BLOCKED: Lean toolchain differs from the reviewed pin: %s\n' "$LEAN_VERSION"
        exit 3
        ;;
esac

RUST_RELEASE=$(rustc --version --verbose | awk '/^release:/ {print $2}')
RUST_COMMIT=$(rustc --version --verbose | awk '/^commit-hash:/ {print $2}')
if [ "$RUST_RELEASE" != "$RUST_RELEASE_PIN" ] || [ "$RUST_COMMIT" != "$RUST_COMMIT_PIN" ]; then
    printf '%s\n' 'BLOCKED: host Rust toolchain differs from the reviewed pin.'
    exit 3
fi

TMP=$(mktemp -d "${TMPDIR:-/tmp}/clutch-bspline-refinement.XXXXXX")
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

(cd "$REPO/lean" && lake build)
(cd "$REPO/lean" && lake env lean "$LEAN_EMITTER") > "$TMP/fixtures.txt"

if [ "$(wc -l < "$TMP/fixtures.txt" | tr -d ' ')" -ne 8 ] ||
    grep -Ev '^[^|]+\|[0-9]+(,[0-9]+)*$' "$TMP/fixtures.txt"; then
    printf '%s\n' 'BLOCKED: Lean emitter produced a malformed or incomplete fixture set.'
    exit 7
fi
if ! cmp -s "$LEAN_ARCHIVE" "$TMP/fixtures.txt"; then
    printf '%s\n' 'FAIL: live Lean transcript differs from the reviewed archive.'
    diff -u "$LEAN_ARCHIVE" "$TMP/fixtures.txt" || true
    exit 8
fi

awk -F '|' '{ print $1 }' "$TMP/fixtures.txt" > "$TMP/inputs.txt"
awk -F '|' '{ print "ok," $2 }' "$TMP/fixtures.txt" > "$TMP/expected.txt"

cargo test --quiet --manifest-path "$CRATE/Cargo.toml"
cargo run --quiet --manifest-path "$CRATE/Cargo.toml" --example oracle_driver \
    < "$TMP/inputs.txt" > "$TMP/actual.txt"

if ! cmp -s "$TMP/expected.txt" "$TMP/actual.txt"; then
    printf '%s\n' 'FAIL: production evaluator disagrees with live Lean model rows.'
    diff -u "$TMP/expected.txt" "$TMP/actual.txt" || true
    exit 8
fi
if ! cmp -s "$PRODUCTION_ARCHIVE" "$TMP/actual.txt"; then
    printf '%s\n' 'FAIL: live production transcript differs from the reviewed archive.'
    diff -u "$PRODUCTION_ARCHIVE" "$TMP/actual.txt" || true
    exit 8
fi

make_mutation() {
    label=$1
    output=$2
    case "$label" in
        tie-direction)
            awk '
                !done && /Some\(current\) => remainders\[index\] > remainders\[current\]/ {
                    sub(/ > /, " >= ")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
        residual-awards)
            awk '
                /Fixed-denominator specialization/ { production = 1 }
                production && !done && /while remaining > 0/ {
                    sub(/while remaining > 0/, "while remaining > residual")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
        pane-placement)
            awk '
                /Fixed-denominator specialization/ { production = 1 }
                production && !done && /weights\[pane \+ local\] = floor/ {
                    sub(/weights\[pane \+ local\]/, "weights[local]")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
        span-index)
            awk '
                /Fixed-denominator specialization/ { production = 1 }
                production && !done && /checked_add\(pane\)/ {
                    sub(/checked_add\(pane\)/, "checked_add(pane + 1)")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
        closed-top)
            awk '
                !done && /if handled >= last/ {
                    sub(/if handled >= last/, "if handled > last")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
        *) exit 9 ;;
    esac
}

run_expected_red() {
    label=$1
    mutant="$TMP/mutant-$label"
    mkdir -p "$mutant/src" "$mutant/examples"
    cp "$CRATE/Cargo.toml" "$CRATE/Cargo.lock" "$mutant/"
    cp "$DRIVER" "$mutant/examples/oracle_driver.rs"
    make_mutation "$label" "$mutant/src/lib.rs"
    if ! CARGO_TARGET_DIR="$mutant/target" cargo run --quiet \
        --manifest-path "$mutant/Cargo.toml" --example oracle_driver \
        < "$TMP/inputs.txt" > "$mutant/actual.txt"; then
        printf 'FAIL mutation=%s did not execute successfully\n' "$label"
        exit 9
    fi
    if cmp -s "$TMP/expected.txt" "$mutant/actual.txt"; then
        printf 'FAIL mutation=%s survived the Lean/Rust comparison\n' "$label"
        exit 9
    fi
    if [ "$(wc -l < "$mutant/actual.txt" | tr -d ' ')" -ne 8 ]; then
        printf 'FAIL mutation=%s produced an incomplete executable transcript\n' "$label"
        exit 9
    fi
    printf 'mutation=%s status=EXPECTED_RED reason=semantic-disagreement\n' "$label"
}

printf 'lean_version=%s\n' "$LEAN_VERSION_PIN"
printf 'lean_commit=%s\n' "$LEAN_COMMIT_PIN"
printf 'rust_release=%s\n' "$RUST_RELEASE"
printf 'rust_commit=%s\n' "$RUST_COMMIT"
printf 'production_source_sha256=%s\n' "$SOURCE_SHA256_PIN"
printf 'production_driver_sha256=%s\n' "$DRIVER_SHA256_PIN"
printf 'lean_source_sha256=%s\n' "$LEAN_SOURCE_SHA256_PIN"
printf 'lean_emitter_sha256=%s\n' "$LEAN_EMITTER_SHA256_PIN"
printf 'lean_fixture_transcript_sha256=%s\n' "$(digest "$TMP/fixtures.txt")"
printf 'production_transcript_sha256=%s\n' "$(digest "$TMP/actual.txt")"
printf '%s\n' 'baseline=PASS fixtures=8 seam=BasisSpec::evaluate'

run_expected_red tie-direction
run_expected_red residual-awards
run_expected_red pane-placement
run_expected_red span-index
run_expected_red closed-top

printf '%s\n' 'status=PASS'
printf '%s\n' 'claim=finite live Lean-model/production-Rust agreement at pinned source digests'
printf '%s\n' 'boundary=no universal source refinement; parser bounds, fixed-denominator correctness outside rows, compilers, SBF, Solana, and deployment remain unverified'
