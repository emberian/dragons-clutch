#!/bin/sh
set -eu

# Compare a generated 3,360-row Lean-model corpus with the exact production
# Rust entry point, then require eight temporary production-source mutations to
# disagree.  This is the wide sibling of `run_bspline_refinement.sh`: that
# campaign runs eight rows whose `Split` literals were derived by hand, and its
# assumption manifest says so.  Here the splits come from the checked generic
# evaluator `DragonsClutch.BSplineCorpus.uniformSmoothBasis?`, whose exactness
# at every admitted uniform input is a theorem, so rows cost nothing and the
# corpus can sweep every integer value across each grid at eight scales.
#
# It is still a digest-bound executable refinement campaign, not a universal
# Verus or Lean theorem about Rust.  It never rewrites the checked-out source.

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO=$(CDPATH='' cd -- "$HERE/../.." && pwd)
CRATE="$REPO/crates/clutch-bspline"
SOURCE="$CRATE/src/lib.rs"
DRIVER="$CRATE/examples/oracle_driver.rs"
LEAN_MODEL="$REPO/lean/DragonsClutch/BSpline.lean"
LEAN_CORPUS="$REPO/lean/DragonsClutch/BSplineCorpus.lean"
LEAN_EMITTER="$HERE/emit_degree_corpus.lean"
LEAN_ARCHIVE="$HERE/evidence/lean_degree_corpus.txt"
PRODUCTION_ARCHIVE="$HERE/evidence/production_degree_corpus.txt"

EXPECTED_ROWS=3360

LEAN_VERSION_PIN='4.33.0'
LEAN_COMMIT_PIN='d8b18978322de05a8f3dba51ef03cf5461676c17'
RUST_RELEASE_PIN='1.98.0-nightly'
RUST_COMMIT_PIN='91fe22da8084a1c9e993d78d4a56f22ab8396236'

SOURCE_SHA256_PIN='220de128366a8311de6579c0ce334a64c97620159eaf9570f61fa10fabb6de92'
DRIVER_SHA256_PIN='c74ecec10c36fbebc3ab3335f2f933de6ecbaae4e061615f9e0822a917888dc7'
CARGO_SHA256_PIN='d993057affd6a9ba58a698e59e109ab882353456294ba57712c6cfac378b1c0d'
LOCK_SHA256_PIN='e49289a908b01a9032b096cfea0499f4a902714abf9475b91b55446d0ab43edd'
LEAN_MODEL_SHA256_PIN='3e2961e765cc0aeebe232bb2b4e9667b036fc06a22e5b0960873cc51a91d52bc'
LEAN_CORPUS_SHA256_PIN='f410e9fac2a9a7c6bfcfd4e4252ac7e6c5c6d4a98f4b9e09d02523237382832f'
LEAN_EMITTER_SHA256_PIN='56cde49550eb4be9e28b14bd771422ea4bbb93fb910b5dc2c50a4a25fea0522b'
LEAN_ARCHIVE_SHA256_PIN='0809fd47b5ac8c65c7bb20a896f67a6fd82916e6b5620d7435bea47b43e5307d'
PRODUCTION_ARCHIVE_SHA256_PIN='786baad85566c6badb5c07a8eba8883f25df90fa1120caf9a42f123fec0d1b92'

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
require_digest lean-model "$LEAN_MODEL" "$LEAN_MODEL_SHA256_PIN"
require_digest lean-corpus "$LEAN_CORPUS" "$LEAN_CORPUS_SHA256_PIN"
require_digest lean-emitter "$LEAN_EMITTER" "$LEAN_EMITTER_SHA256_PIN"
require_digest lean-transcript "$LEAN_ARCHIVE" "$LEAN_ARCHIVE_SHA256_PIN"
require_digest production-transcript "$PRODUCTION_ARCHIVE" "$PRODUCTION_ARCHIVE_SHA256_PIN"

# The two seams this campaign is about: the production entry point, and the
# theorem that makes a generated row non-vacuous.  Ambiguity is a refusal.
if [ "$(grep -Ec '^[[:space:]]*pub fn evaluate\(&self, value: u128\)' "$SOURCE")" -ne 1 ] ||
    [ "$(grep -Ec '^theorem uniformSmoothBasis\?_exact' "$LEAN_CORPUS")" -ne 1 ] ||
    [ "$(grep -Ec '^def corpusRows' "$LEAN_CORPUS")" -ne 1 ]; then
    printf '%s\n' 'BLOCKED: the production or Lean corpus seam is missing or ambiguous.'
    exit 4
fi

if grep -En '^[[:space:]]*(sorry|admit|axiom)([[:space:]]|$)|native_decide' \
    "$LEAN_MODEL" "$LEAN_CORPUS" "$LEAN_EMITTER"; then
    printf '%s\n' 'BLOCKED: forbidden proof shortcut in the checked Lean corpus closure.'
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

TMP=$(mktemp -d "${TMPDIR:-/tmp}/clutch-bspline-degree-corpus.XXXXXX")
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

(cd "$REPO/lean" && lake build DragonsClutch.BSplineCorpus)
(cd "$REPO/lean" && lake env lean "$LEAN_EMITTER") > "$TMP/corpus.txt"

if [ "$(wc -l < "$TMP/corpus.txt" | tr -d ' ')" -ne "$EXPECTED_ROWS" ] ||
    grep -Ev '^[0-9]+,[0-9]+,[0-9]+,c,[0-9]+,[0-9]+(,[0-9]+)+\|[0-9]+(,[0-9]+)*$' \
        "$TMP/corpus.txt"; then
    printf '%s\n' 'BLOCKED: Lean emitter produced a malformed or incomplete corpus.'
    exit 7
fi
# Every implemented smooth degree must actually be present; a corpus that
# silently lost degree three would still be 3,360 well-formed lines.
for degree in 1 2 3; do
    present=$(awk -F, -v d="$degree" '$1 == d' "$TMP/corpus.txt" | wc -l | tr -d ' ')
    if [ "$present" -eq 0 ]; then
        printf 'BLOCKED: the corpus contains no degree-%s rows.\n' "$degree"
        exit 7
    fi
    printf 'degree_%s_rows=%s\n' "$degree" "$present"
done
if ! cmp -s "$LEAN_ARCHIVE" "$TMP/corpus.txt"; then
    printf '%s\n' 'FAIL: live Lean transcript differs from the reviewed archive.'
    diff -u "$LEAN_ARCHIVE" "$TMP/corpus.txt" | head -n 40 || true
    exit 8
fi

awk -F '|' '{ print $1 }' "$TMP/corpus.txt" > "$TMP/inputs.txt"
awk -F '|' '{ print "ok," $2 }' "$TMP/corpus.txt" > "$TMP/expected.txt"

cargo run --quiet --manifest-path "$CRATE/Cargo.toml" --example oracle_driver \
    < "$TMP/inputs.txt" > "$TMP/actual.txt"

# No row may refuse: every corpus input is inside the freeze-time bounds and
# the admitted shape, so an `err,` line is a disagreement, not an expectation.
if grep -q '^err,' "$TMP/actual.txt"; then
    printf '%s\n' 'FAIL: the production evaluator refused an admitted corpus row.'
    grep -n '^err,' "$TMP/actual.txt" | head -n 10
    exit 8
fi
if ! cmp -s "$TMP/expected.txt" "$TMP/actual.txt"; then
    printf '%s\n' 'FAIL: production evaluator disagrees with live Lean model rows.'
    diff -u "$TMP/expected.txt" "$TMP/actual.txt" | head -n 40 || true
    exit 8
fi
if ! cmp -s "$PRODUCTION_ARCHIVE" "$TMP/actual.txt"; then
    printf '%s\n' 'FAIL: live production transcript differs from the reviewed archive.'
    diff -u "$PRODUCTION_ARCHIVE" "$TMP/actual.txt" | head -n 40 || true
    exit 8
fi

make_mutation() {
    label=$1
    output=$2
    case "$label" in
        # The two smooth common denominators.  `6*h^3` is the value
        # DISTRIBUTIONAL_CLAIMS_DESIGN.md section 2.2 still prints for the
        # cubic; the shipped evaluator uses `12*h^3`, so this mutant is the
        # executable form of that documented divergence.
        cubic-denominator)
            awk '
                !done && /12 => \(u16::from\(spacing_shift\) \* 3 \+ 2, 3_u8\)/ {
                    sub(/\* 3 \+ 2/, "* 3 + 1")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
        quadratic-denominator)
            awk '
                !done && /2 => \(u16::from\(spacing_shift\) \* 2 \+ 1, 1_u8\)/ {
                    sub(/\* 2 \+ 1/, "* 2 + 0")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
        # Open-clamped endpoint multiplicity: the last stored knot is what
        # every expanded index at or past the interior end must denote, which
        # is what makes the top pane correct without an interior-formula
        # substitution.  (The symmetric low-end guard `index <= degree` is
        # deliberately not mutated: flipping it to `index < degree` is a
        # semantic no-op, because the interior branch then computes
        # `knots[degree - degree] = knots[0]` anyway.)
        high-endpoint-repeat)
            awk '
                /fn expanded_knot/ { production = 1 }
                production && !done && /Ok\(spec\.knots\[knot_count - 1\]\)/ {
                    sub(/knot_count - 1/, "knot_count - 2")
                    done = 1
                }
                { print }
                END { if (!done) exit 9 }
            ' "$SOURCE" > "$output"
            ;;
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
    if [ "$(wc -l < "$mutant/actual.txt" | tr -d ' ')" -ne "$EXPECTED_ROWS" ]; then
        printf 'FAIL mutation=%s produced an incomplete executable transcript\n' "$label"
        exit 9
    fi
    disagreements=$(diff "$TMP/expected.txt" "$mutant/actual.txt" | grep -c '^<' || true)
    printf 'mutation=%s status=EXPECTED_RED rows_disagreeing=%s\n' \
        "$label" "$disagreements"
}

printf 'lean_version=%s\n' "$LEAN_VERSION_PIN"
printf 'lean_commit=%s\n' "$LEAN_COMMIT_PIN"
printf 'rust_release=%s\n' "$RUST_RELEASE"
printf 'rust_commit=%s\n' "$RUST_COMMIT"
printf 'production_source_sha256=%s\n' "$SOURCE_SHA256_PIN"
printf 'lean_corpus_sha256=%s\n' "$LEAN_CORPUS_SHA256_PIN"
printf 'lean_emitter_sha256=%s\n' "$LEAN_EMITTER_SHA256_PIN"
printf 'lean_transcript_sha256=%s\n' "$(digest "$TMP/corpus.txt")"
printf 'production_transcript_sha256=%s\n' "$(digest "$TMP/actual.txt")"
printf 'baseline=PASS rows=%s seam=BasisSpec::evaluate\n' "$EXPECTED_ROWS"

run_expected_red cubic-denominator
run_expected_red quadratic-denominator
run_expected_red high-endpoint-repeat
run_expected_red tie-direction
run_expected_red residual-awards
run_expected_red pane-placement
run_expected_red span-index
run_expected_red closed-top

printf '%s\n' 'status=PASS'
printf '%s\n' 'claim=finite live Lean-model/production-Rust agreement on 3360 generated uniform rows at pinned source digests'
printf '%s\n' 'boundary=uniform stored grids only; no universal source refinement; nonuniform panes, degree zero, hostile-input refusal order, compilers, SBF, Solana, and deployment remain unverified'
