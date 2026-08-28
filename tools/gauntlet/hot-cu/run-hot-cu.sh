#!/usr/bin/env bash
# HOT-CU: sweep the SHIPPED-ELF Hot tail's compute at the protocol default heap.
#
#   build the ELFs -> run `hot_heap_frame_is_inert` once per fixture seed ->
#   report PASS n/N and, only at N/N, MEAN, MIN, MAX, and the trading ELF
#   sha256 they belong to.
#
# This is W2p/W2q/DIAG-82's measurement, promoted out of /tmp. It lived in
# `/private/tmp/w2q/sweep.sh` while several board entries quoted pass counts
# from it, which is one `rm -rf /tmp` away from a gate nobody can re-run.
#
# This tier is NOT a census campaign. It submits no transaction to a validator,
# binds no route, folds nothing into `out/ledger.json`, and carries neither
# `bindings.json` nor `witnesses.json`. It is an INSTRUMENT: it measures one
# number, twenty times, and prints the only two statistics that number supports.
# See README.md before quoting anything it prints.
#
# ============================================================================
# THE REPORTING RULE (ledger M-61, docs/ASPIRATION_LEDGER.md)
# ============================================================================
#
# The per-seed CU figure is a BUMP-SEARCH LOTTERY. Report the PASS COUNT and the
# MEAN. Never a worst margin, never one seed's number as a bound.
#
# The Hot path derives program addresses whose seeds include the ARTIFACT
# RELEASE IDENTITY, and that identity is `hash(elf)` -- see `hash(elf)` in
# `programs/dclutch-trading-sbf/program-test/direct-hot/src/waist.rs`.
# `try_find_program_address` costs 1,500 CU per rejected bump and walks up to 31
# of them, so each seed's total carries `n * 1,500` of pure draw: a swing of
# +/-46,000 CU against a 1,400,000 ceiling with no headroom to buy.
#
# The consequence is the thing lanes keep getting wrong: **changing one byte of
# the trading ELF redraws every seed's bump search.** DIAG-82 measured it across
# a pure out-of-line refactor whose real cost was one extra call (~50 CU): every
# per-seed delta decomposed as `n * 1,500 + ~50`. "Worst margin 8,238" was never
# a property of the code -- the same tip with a 440-byte-larger ELF measured
# 3,689, on a different seed, at 20/20 either way.
#
# So this script prints the pass count, the mean, and the digest the numbers
# belong to, and it prints them itself so the next lane cannot report it wrong.
# MIN and MAX are printed as the observed SPREAD, which is what they are; they
# are not a bound on anything and MIN in particular is not a margin.
#
# The pinning in `waist::fixture_keypair` makes a single seed's figure
# REPRODUCIBLE. It does not make it MEANINGFUL: on a real chain the makers are
# whoever they are, so the spread is a property of the protocol.
#
# ============================================================================
# THE SUBSTRATE ARMS (decision 0012)
# ============================================================================
#
# `--substrate` selects which release substrate the fixture stages under the
# SAME ELFs. It is `DCLUTCH_FIXTURE_SUBSTRATE`, whose four arms are declared in
# `program-test/direct-hot/src/waist.rs::FixtureSubstrateV1`.
#
# Decision 0012 admitted a MUTABLE substrate onto the cached-digest path.
# `slot_pinned_release_elf_digest_v1` branches on the release's upgrade policy,
# and until this option existed the fixture could only ever build `Immutable`
# releases over ProgramData with no authority -- so the ExactAuthority arm, the
# whole of what 0012 added, had never executed against a validator and this
# sweep could not say anything about its cost.
#
# THE THREE ARMS ARE NOT TWO. `slot-pinned` minus `immutable` is NOT 0012's
# cost: the policy byte, the bound authority and the bound slot all live inside
# `ArtifactReleaseV1::to_bytes`, so they move the artifact id, the release-set
# identity, and every PDA seeded by it -- which under M-61 is a REDRAWN LOTTERY
# worth tens of thousands of CU by itself. `immutable-pinned` is the control:
# same `Immutable` digest arm, same absent authority, but the same nonzero bound
# slot, so it has a DIFFERENT release identity and takes the SAME code path.
#
#   immutable-pinned - immutable   = REDRAW ALONE
#   slot-pinned      - immutable   = REDRAW + whatever 0012 costs or saves
#
# The difference between those two differences is the signal. Sweep all three
# against ONE ELF or the comparison means nothing, which is what the build reuse
# below exists to guarantee.
#
# usage:
#   tools/gauntlet/hot-cu/run-hot-cu.sh                      # build, sweep 20
#   tools/gauntlet/hot-cu/run-hot-cu.sh --seeds 40
#   tools/gauntlet/hot-cu/run-hot-cu.sh --substrate slot-pinned
#   tools/gauntlet/hot-cu/run-hot-cu.sh --elf-dir /path/to/deploy   # no build
#   tools/gauntlet/hot-cu/run-hot-cu.sh --trading-elf /path/to/final.so
#
# Outputs land under --work (default /private/tmp/dclutch-hot-cu), never under
# the shared `target/`: parallel lanes share this working tree, and the gauntlet
# README is explicit that a tier writing into it is a race.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
WORK="/private/tmp/dclutch-hot-cu"
ELF_DIR=""
TRADING_ELF_OVERRIDE=""
COMMIT=""
SEEDS=20
SUBSTRATE="immutable"
# The harness paths whose dirtiness the DIRTY flag cannot speak for: `--commit`
# archives the BUILD, but the fixture derivation -- keys, substrate, staged
# ProgramData -- is read out of --repo on every run, committed or not.
HARNESS_PATHS="programs/dclutch-trading-sbf/program-test"

usage() {
    cat <<'USAGE'
usage: tools/gauntlet/hot-cu/run-hot-cu.sh [options]

  --repo PATH      source repository (default: this script's repository)
  --work PATH      scratch + output root (default: /private/tmp/dclutch-hot-cu)
  --elf-dir PATH   use these already-built .so artifacts instead of building.
                   The digest is reported either way, and per M-61 the digest
                   is what the numbers belong to -- so an --elf-dir from
                   another revision produces a valid, differently-drawn sweep.
  --trading-elf PATH
                   replace only dclutch_trading_sbf.so after the base artifact
                   set is built or supplied. The file must be absolute, regular,
                   and not a symlink. Its digest is reported explicitly. This is
                   the bounded handoff for a final Direct ELF; all seven fixture
                   support ELFs remain byte-identical to the base set.
  --commit REV     build the ELFs from a clean `git archive` of REV instead of
                   from the working tree. Use this whenever the number is going
                   to be quoted at a revision: this is a SHARED checkout and a
                   concurrent lane's uncommitted edit to any program in the
                   fixture redraws every seed (M-61).
  --seeds N        how many fixture seeds to sweep, 0..N-1 (default 20)
  --substrate NAME which release substrate the fixture stages, one of
                   immutable (default), immutable-pinned, slot-pinned,
                   slot-pinned-superseded. Sets DCLUTCH_FIXTURE_SUBSTRATE.
                   `immutable-pinned` is the REDRAW CONTROL for `slot-pinned`;
                   see the substrate block at the top of this file. Each
                   substrate keeps its own logs and summary under --work.
  -h, --help       show this message

Prints PASS n/N and the trading ELF sha256. MEAN, MIN, and MAX are emitted only
when every requested seed completed; a partial run has no sweep mean. Exits
nonzero if any seed failed. Read the M-61 block at the top of this file, or
README.md, before quoting any number it prints.
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --elf-dir) ELF_DIR="${2:?--elf-dir needs a value}"; shift 2 ;;
        --trading-elf) TRADING_ELF_OVERRIDE="${2:?--trading-elf needs a value}"; shift 2 ;;
        --commit) COMMIT="${2:?--commit needs a value}"; shift 2 ;;
        --seeds) SEEDS="${2:?--seeds needs a value}"; shift 2 ;;
        --substrate) SUBSTRATE="${2:?--substrate needs a value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "hot-cu: unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

case "$WORK" in /*) ;; *) echo "hot-cu: --work must be absolute" >&2; exit 2 ;; esac
case "$SEEDS" in ''|*[!0-9]*) echo "hot-cu: --seeds must be a decimal count" >&2; exit 2 ;; esac
[ "$SEEDS" -gt 0 ] || { echo "hot-cu: --seeds must be positive" >&2; exit 2; }
# Refused here as well as in the fixture. `FixtureSubstrateV1::from_env` panics
# on an unknown name, so a typo would already fail -- but it would fail twenty
# times, after a full ELF build, with the reason buried in a per-seed log.
case "$SUBSTRATE" in
    immutable|immutable-pinned|slot-pinned|slot-pinned-superseded) ;;
    *) echo "hot-cu: --substrate must be immutable, immutable-pinned, slot-pinned or slot-pinned-superseded" >&2; exit 2 ;;
esac

die() { echo "hot-cu: $*" >&2; exit 1; }
say() { printf '\n== %s\n' "$*"; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }

command -v cargo >/dev/null 2>&1 || die "cargo not found"
git -C "$REPO" rev-parse --git-dir >/dev/null 2>&1 \
    || die "not a repository: $REPO"
if [ -n "$TRADING_ELF_OVERRIDE" ]; then
    case "$TRADING_ELF_OVERRIDE" in
        /*) ;;
        *) die "--trading-elf must be absolute" ;;
    esac
    [ -f "$TRADING_ELF_OVERRIDE" ] \
        || die "--trading-elf is not a regular file: $TRADING_ELF_OVERRIDE"
    [ ! -L "$TRADING_ELF_OVERRIDE" ] \
        || die "--trading-elf must not be a symlink: $TRADING_ELF_OVERRIDE"
fi

LOGS="$WORK/logs"
# Per substrate, not shared. Three arms swept into one directory would blend
# their pass counts through the `rm -f` below, and the whole point of the
# `immutable-pinned` control is that its twenty figures stay separable from the
# twenty they are the control FOR.
SWEEP="$WORK/sweep/$SUBSTRATE"
SUMMARY="$WORK/summary-$SUBSTRATE.json"
BUILT_ELF="$WORK/elf"
mkdir -p "$WORK" "$LOGS" "$SWEEP"
# A re-run must not blend its pass count with a previous shape's logs.
rm -f "$SWEEP"/seed*.log

REVISION="$(git -C "$REPO" rev-parse "${COMMIT:-HEAD}")"
DIRTY="clean"
git -C "$REPO" diff --quiet HEAD -- programs crates Cargo.toml Cargo.lock 2>/dev/null || DIRTY="DIRTY"
# The HARNESS is a separate fact from the artifacts, and `--commit` does not
# speak for it: the archive supplies the ELFs, but the fixture keys, the staged
# ProgramData and the substrate arms are compiled out of --repo on every run.
# Reporting only the build's cleanliness would let a figure drawn from an
# uncommitted fixture edit be quoted as a clean-revision number.
#
# It is reported against --repo's OWN HEAD, never against `--commit`'s revision:
# the harness is whatever is checked out here, and labelling it with the
# revision the ELFs were archived from would be the same mispairing again, one
# level down.
HARNESS_REVISION="$(git -C "$REPO" rev-parse HEAD)"
HARNESS_DIRTY="clean"
git -C "$REPO" diff --quiet HEAD -- $HARNESS_PATHS 2>/dev/null || HARNESS_DIRTY="DIRTY"
if [ -n "$(git -C "$REPO" ls-files --others --exclude-standard -- $HARNESS_PATHS 2>/dev/null)" ]; then
    HARNESS_DIRTY="DIRTY"
fi

# ------------------------------------------------------------------ 1. the ELFs
#
# By DEFAULT this builds from the WORKING TREE, unlike the campaign tiers, which
# always archive a revision first. That default is the honest shape for this
# tier: a CU lane's question is usually "what does the code in front of me
# cost", asked about an edit that is not committed yet. The cost is that a
# concurrent lane's edit lands in the measurement, so the run reports the
# revision AND whether the compiled inputs were dirty.
#
# `--commit REV` is the other half, and it is not optional politeness. This is a
# SHARED checkout: on 2026-08-27 a default run here found five other lanes'
# uncommitted edits in the tree, two of them (`core-sbf/src/resolution.rs` and a
# Cargo.lock dependency addition) inside the fixture's own artifacts. Under
# M-61 that is not a small contamination -- every changed byte redraws every
# seed's bump search. Anything that will be QUOTED at a revision has to be built
# from a clean archive of it.
#
# The BUILD is archived; the harness still runs from --repo, because the host
# side does not consume compute. What the figure depends on is the ELFs plus the
# fixture keys, and the fixture derivation lives in
# `program-test/direct-hot/src/waist.rs` -- so check that the harness paths are
# clean when quoting, which the DIRTY flag lets a reader do.
BUILD_ROOT="$REPO"
SBF_TARGET_DIR=""
PROVENANCE=""
STAMP=""
NEEDS_BUILD="yes"
if [ -n "$ELF_DIR" ]; then
    case "$ELF_DIR" in /*) ;; *) die "--elf-dir must be absolute" ;; esac
    [ -d "$ELF_DIR" ] || die "--elf-dir does not exist: $ELF_DIR"
    say "stage elf: using the artifacts already at $ELF_DIR"
    # These artifacts came from somewhere this run cannot see, so it must NOT
    # report --repo's HEAD beside them. Printing the checkout's revision next to
    # a figure drawn from someone else's ELF is precisely the mispairing M-61
    # exists to stop -- and it bites immediately, because another lane can
    # commit between the build and the sweep. The digest is the provenance here.
    PROVENANCE="artifacts supplied via --elf-dir; revision unknown to this run"
    NEEDS_BUILD=""
elif [ -n "$COMMIT" ]; then
    BUILD_ROOT="$WORK/source-${REVISION:0:12}"
    ELF_DIR="$WORK/elf-${REVISION:0:12}"
    DIRTY="clean"
    PROVENANCE="$REVISION (clean git archive)"
    # A COMPLETED build of this exact revision is REUSED, and that is a
    # requirement rather than an optimization. Two substrate arms can only be
    # compared if both drew against the same ELF byte for byte: under M-61 a
    # one-byte difference redraws every seed by up to +/-46,000 CU, which is
    # larger than any effect a substrate arm could have. Two separate builds of
    # one revision SHOULD agree; reuse means the comparison does not depend on
    # that holding. The stamp is written only after all eight artifacts build,
    # so a partial build is never reused -- and every run still recomputes and
    # prints the digest, which is what a reader checks, not this stamp.
    STAMP="$ELF_DIR/.hot-cu-built"
    if [ "$(cat "$STAMP" 2>/dev/null || true)" = "$REVISION" ] \
        && [ -f "$ELF_DIR/dclutch_trading_sbf.so" ]; then
        say "stage archive: $REVISION already built at $ELF_DIR -- reused"
        NEEDS_BUILD=""
    else
        say "stage archive: $REVISION (clean, from git archive)"
        rm -rf "$BUILD_ROOT"
        rm -f "$STAMP"
        mkdir -p "$BUILD_ROOT"
        git -C "$REPO" archive "$REVISION" | tar -x -C "$BUILD_ROOT"
        # The archive is a fresh tree, so its default target/ would be a cold
        # build every time. One shared, work-root-local target dir across
        # revisions keeps a bisect from paying full price per step -- and it is
        # still not the checkout's `target/`, which parallel lanes share.
        #
        # NOT exported: the sweep stage below runs `cargo test` out of --repo,
        # and an inherited CARGO_TARGET_DIR would point that build at the
        # archive's artifacts. It is applied to the SBF builds only.
        SBF_TARGET_DIR="$WORK/build-target"
    fi
else
    ELF_DIR="$BUILT_ELF"
    say "stage elf: $REVISION ($DIRTY working tree)"
    PROVENANCE="$REVISION ($DIRTY working tree)"
fi

if [ -n "$NEEDS_BUILD" ]; then
    mkdir -p "$ELF_DIR"

    # hbox is co-tenant with codex's HOL build. Containment is structural.
    if command -v swarm-build >/dev/null 2>&1; then WRAP="swarm-build"; else WRAP=""; fi
    # Run from the build root so workspace resolution and any .cargo/config.toml
    # are the ones belonging to the tree being measured, not the invoker's.
    build() (
        cd "$BUILD_ROOT"
        # An `&& export` one-liner here is the classic `set -e` footgun: the
        # AND-list returns nonzero whenever the test is false, and the list is a
        # complete command, so an unset variable would abort the run.
        if [ -n "$SBF_TARGET_DIR" ]; then export CARGO_TARGET_DIR="$SBF_TARGET_DIR"; fi
        if [ -n "$WRAP" ]; then
            "$WRAP" cargo build-sbf --manifest-path "$1" --sbf-out-dir "$ELF_DIR" -- --locked --offline
        else
            cargo build-sbf --manifest-path "$1" --sbf-out-dir "$ELF_DIR" -- --locked --offline
        fi
    )

    command -v cargo-build-sbf >/dev/null 2>&1 || die "cargo-build-sbf not found"

    # The five protocol programs the fixture installs, then the three test-only
    # callers `waist::elves` loads beside them. All eight, or the harness panics
    # looking for one and every seed reports FAIL for a reason that is not CU.
    MANIFESTS="programs/dclutch-trading-sbf/Cargo.toml
programs/dclutch-registry-sbf/Cargo.toml
programs/dclutch-core-sbf/Cargo.toml
programs/dclutch-claims-sbf/Cargo.toml
programs/dclutch-custody-sbf/Cargo.toml
programs/dclutch-trading-sbf/program-test/test-programs/trading-outer/Cargo.toml
programs/dclutch-trading-sbf/program-test/test-programs/core-caller/Cargo.toml
programs/dclutch-trading-sbf/program-test/test-programs/registry/Cargo.toml"

    # `cargo build-sbf` exits ZERO when the SBF backend reports that a call
    # overwrites its own stack frame. Every campaign tier REFUSES on a nonzero
    # count, because an artifact the toolchain calls potentially-undefined has
    # no business producing evidence. This tier COUNTS AND WARNS instead, and
    # the distinction is not laxity: DIAG-82 was an 82-diagnostic regression
    # whose CU cost is exactly what someone would come here to measure, and a
    # measuring instrument that refuses to measure the regression is useless on
    # the one day it matters. Nothing here is admitted as evidence of
    # correctness; the count is printed so a reader knows what was measured.
    diagnostics=0
    for manifest in $MANIFESTS; do
        name="$(basename "$(dirname "$manifest")")"
        log="$LOGS/build-$name.log"
        build "$BUILD_ROOT/$manifest" > "$log" 2>&1 \
            || { tail -n 40 "$log" >&2; die "SBF build failed: $name (see $log)"; }
        count="$(grep -c 'overwrites values in the frame' "$log" || true)"
        printf '  %-24s %s frame diagnostics\n' "$name" "${count:-0}"
        diagnostics=$((diagnostics + count))
    done
    if [ "$diagnostics" -ne 0 ]; then
        echo "hot-cu: WARNING: $diagnostics SBF stack-frame-overwrite diagnostics across these artifacts." >&2
        echo "hot-cu: the sweep still runs -- measuring a frame regression's CU cost is a reason this" >&2
        echo "hot-cu: tier exists -- but nothing it prints is evidence that these artifacts are sound." >&2
    fi
    # Last, and only on the archived path: the stamp claims a COMPLETE build of
    # a named revision, so it must not exist until every manifest above has
    # succeeded. The working-tree build writes none -- an uncommitted tree is
    # not a revision and nothing about it is reusable.
    if [ -n "$STAMP" ]; then printf '%s\n' "$REVISION" > "$STAMP"; fi
fi

# Keep the base artifact directory immutable. A Direct lane may finish its
# Trading link after the other seven fixture ELFs are already checked; stage an
# overlay and replace exactly one canonical filename. Copy the override to a
# temporary regular file first so a re-run cannot delete its own input when the
# caller points back into a prior overlay.
TRADING_ELF_OVERRIDE_SHA=""
TRADING_ELF_OVERRIDE_JSON=null
if [ -n "$TRADING_ELF_OVERRIDE" ]; then
    TRADING_ELF_OVERRIDE_SHA="$(sha256 "$TRADING_ELF_OVERRIDE")"
    TRADING_ELF_OVERRIDE_JSON="\"$TRADING_ELF_OVERRIDE_SHA\""
    override_copy="$(mktemp "$WORK/trading-elf-override.XXXXXX.so")"
    cp "$TRADING_ELF_OVERRIDE" "$override_copy"
    overlay="$WORK/elf-with-trading-override"
    rm -rf "$overlay"
    mkdir -p "$overlay"
    cp "$ELF_DIR"/*.so "$overlay/"
    cp "$override_copy" "$overlay/dclutch_trading_sbf.so"
    rm -f "$override_copy"
    ELF_DIR="$overlay"
    PROVENANCE="$PROVENANCE; Trading ELF supplied via --trading-elf ($TRADING_ELF_OVERRIDE_SHA)"
fi

TRADING_ELF="$ELF_DIR/dclutch_trading_sbf.so"
[ -f "$TRADING_ELF" ] || die "no trading ELF at $TRADING_ELF"
TRADING_SHA="$(sha256 "$TRADING_ELF")"

# --------------------------------------------------------------- 2. the sweep
say "sweep: $SEEDS fixture seeds on the $SUBSTRATE substrate, trading ELF ${TRADING_SHA:0:16}..."
echo "seed  exit  CU          result"

pass=0
fail=0
OBSERVED="$SWEEP/observed-cu.txt"
: > "$OBSERVED"
for s in $(seq 0 $((SEEDS - 1))); do
    log="$SWEEP/seed$s.log"
    # Non-fatal by construction: a seed that exhausts the meter is a DRAW this
    # sweep exists to count, not an error that should abort the run. `|| status=`
    # keeps `set -e` from taking the failure.
    status=0
    ( cd "$REPO" && DCLUTCH_FIXTURE_SEED="$s" SBF_OUT_DIR="$ELF_DIR" \
        DCLUTCH_FIXTURE_SUBSTRATE="$SUBSTRATE" \
        cargo test --locked --offline \
            --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
            --test hot_heap_frame_is_inert -- --nocapture ) \
        > "$log" 2>&1 || status=$?

    # ONE capture group, not `grep -oE '[0-9]+'` over the matched line: the line
    # reads "... fixture seed 7: 1376260 CU ...", so a bare digit grep returns
    # the SEED as well as the figure. The /tmp driver this replaces did exactly
    # that and printed a two-line CU column for every seed.
    cu="$(sed -n 's/.*protocol default heap[^:]*: \([0-9][0-9]*\) CU .*/\1/p' "$log" | tail -1)"
    res="$(grep -E '^test result' "$log" | tail -1 | cut -c1-40 || true)"

    if [ "$status" -eq 0 ] && [ -n "$cu" ]; then
        pass=$((pass + 1))
        printf '%s\n' "$cu" >> "$OBSERVED"
    else
        fail=$((fail + 1))
    fi
    printf 'seed %2d  %4d  %-11s %s\n' "$s" "$status" "${cu:-FAIL}" "${res:-no test result line}"
done

# ------------------------------------------------------------- 3. the statistic
#
# A sweep mean exists only when EVERY requested seed completed and printed a
# figure. Averaging the survivors of a failed sweep changes the sample and can
# make PASS 19/20 look like it carries a 20-seed mean. The pass count carries a
# partial run; the observed per-seed figures remain in the logs for diagnosis.
say "result"
if [ "$pass" -eq "$SEEDS" ]; then
    read -r MEAN MIN MAX <<EOF
$(awk '{ t += $1; if (n++ == 0 || $1 < lo) lo = $1; if ($1 > hi) hi = $1 }
       END { printf "%d %d %d\n", int(t / n + 0.5), lo, hi }' "$OBSERVED")
EOF
    MEAN_JSON="$MEAN"
    MIN_JSON="$MIN"
    MAX_JSON="$MAX"
    ALL_SEEDS_COMPLETED=true
else
    MEAN_JSON=null
    MIN_JSON=null
    MAX_JSON=null
    ALL_SEEDS_COMPLETED=false
fi

printf 'PASS %d/%d\n' "$pass" "$SEEDS"
if [ "$pass" -eq "$SEEDS" ]; then
    printf "MEAN %'d CU   (over all %d requested seeds, of 1,400,000)\n" "$MEAN" "$SEEDS"
    printf "MIN  %'d CU\n" "$MIN"
    printf "MAX  %'d CU\n" "$MAX"
    # Rounded to NEAREST, not floored. Per M-61 a delta decomposes as
    # `n * 1,500 + ~50`, so a spread of 49,499 is 33 draws less a small residual,
    # and flooring would report 32 and invite someone to hunt the missing one.
    printf "SPREAD %'d CU  ~ %d bump-search iterations at 1,500 CU each\n" \
        "$((MAX - MIN))" "$(( (MAX - MIN + 750) / 1500 ))"
else
    printf 'MEAN -   MIN -   MAX -   (requires PASS %d/%d; %d completed)\n' \
        "$SEEDS" "$SEEDS" "$pass"
fi
printf 'ELF  %s  dclutch_trading_sbf.so\n' "$TRADING_SHA"
if [ -n "$TRADING_ELF_OVERRIDE_SHA" ]; then
    printf 'OVRD %s  --trading-elf\n' "$TRADING_ELF_OVERRIDE_SHA"
fi
printf 'SRC  %s\n' "$PROVENANCE"
printf 'SUB  %s  (DCLUTCH_FIXTURE_SUBSTRATE)\n' "$SUBSTRATE"
# The harness is compiled out of --repo whatever --commit says about the ELFs,
# and the fixture derivation is what turns a seed into a set of keys. A DIRTY
# harness beside a clean-archive SRC is not automatically a wrong measurement --
# it is the ordinary state of a lane measuring its own uncommitted fixture --
# but it is a fact the number has to be quoted with.
printf 'HARN %s (%s) %s\n' "${HARNESS_REVISION:0:12}" "$HARNESS_DIRTY" "$HARNESS_PATHS"
echo
echo "M-61: quote PASS and MEAN. MIN is not a margin and MAX is not a bound --"
echo "      both are draws, and any one-byte change to the ELF above redraws"
echo "      every seed. A number quoted without that digest means nothing."
if [ "$SUBSTRATE" != "immutable" ]; then
    echo
    echo "0012: this arm's distance from \`--substrate immutable\` is NOT its cost."
    echo "      Changing the substrate changes the release identity, which redraws"
    echo "      every seed's bump search too. Sweep \`--substrate immutable-pinned\`"
    echo "      against this same ELF: it takes the same digest arm as immutable"
    echo "      with a different identity, so it measures the REDRAW ALONE, and"
    echo "      only the difference of the two differences is a signal."
fi

cat > "$SUMMARY" <<EOF
{
  "schema": "dclutch-hot-cu-sweep-v2",
  "artifact_provenance": "$PROVENANCE",
  "trading_elf_sha256": "$TRADING_SHA",
  "trading_elf_override_sha256": $TRADING_ELF_OVERRIDE_JSON,
  "elf_dir": "$ELF_DIR",
  "substrate": "$SUBSTRATE",
  "harness_revision": "$HARNESS_REVISION",
  "harness_state": "$HARNESS_DIRTY",
  "seeds": $SEEDS,
  "pass": $pass,
  "fail": $fail,
  "all_seeds_completed": $ALL_SEEDS_COMPLETED,
  "mean_cu": $MEAN_JSON,
  "min_cu": $MIN_JSON,
  "max_cu": $MAX_JSON,
  "ceiling_cu": 1400000,
  "observed_cu": [$(paste -sd, - < "$OBSERVED")],
  "statistic": "PASS and an all-requested-seed MEAN. A partial run has null mean/min/max. Complete-run min/max are the observed spread of a bump-search lottery (M-61), not bounds; they are keyed to trading_elf_sha256 and a one-byte ELF change redraws every seed.",
  "cross_substrate": "A difference between two substrates' means is redraw PLUS effect: the substrate moves the release identity and so redraws every seed. Subtract the immutable-pinned arm, which is the same digest arm at a different identity, to separate them."
}
EOF
echo "logs:    $SWEEP"
echo "summary: $SUMMARY"

[ "$fail" -eq 0 ] || exit 1
