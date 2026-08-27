#!/usr/bin/env bash
# HOT-CU: sweep the SHIPPED-ELF Hot tail's compute at the protocol default heap.
#
#   build the ELFs -> run `hot_heap_frame_is_inert` once per fixture seed ->
#   report PASS n/N, MEAN, MIN, MAX, and the trading ELF sha256 they belong to.
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
#
# usage:
#   tools/gauntlet/hot-cu/run-hot-cu.sh                      # build, sweep 20
#   tools/gauntlet/hot-cu/run-hot-cu.sh --seeds 40
#   tools/gauntlet/hot-cu/run-hot-cu.sh --elf-dir /path/to/deploy   # no build
#
# Outputs land under --work (default /private/tmp/dclutch-hot-cu), never under
# the shared `target/`: parallel lanes share this working tree, and the gauntlet
# README is explicit that a tier writing into it is a race.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
WORK="/private/tmp/dclutch-hot-cu"
ELF_DIR=""
COMMIT=""
SEEDS=20

usage() {
    cat <<'USAGE'
usage: tools/gauntlet/hot-cu/run-hot-cu.sh [options]

  --repo PATH      source repository (default: this script's repository)
  --work PATH      scratch + output root (default: /private/tmp/dclutch-hot-cu)
  --elf-dir PATH   use these already-built .so artifacts instead of building.
                   The digest is reported either way, and per M-61 the digest
                   is what the numbers belong to -- so an --elf-dir from
                   another revision produces a valid, differently-drawn sweep.
  --commit REV     build the ELFs from a clean `git archive` of REV instead of
                   from the working tree. Use this whenever the number is going
                   to be quoted at a revision: this is a SHARED checkout and a
                   concurrent lane's uncommitted edit to any program in the
                   fixture redraws every seed (M-61).
  --seeds N        how many fixture seeds to sweep, 0..N-1 (default 20)
  -h, --help       show this message

Prints PASS n/N, MEAN, MIN, MAX and the trading ELF sha256. Exits nonzero if
any seed failed. Read the M-61 block at the top of this file, or README.md,
before quoting any single number it prints.
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --elf-dir) ELF_DIR="${2:?--elf-dir needs a value}"; shift 2 ;;
        --commit) COMMIT="${2:?--commit needs a value}"; shift 2 ;;
        --seeds) SEEDS="${2:?--seeds needs a value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "hot-cu: unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

case "$WORK" in /*) ;; *) echo "hot-cu: --work must be absolute" >&2; exit 2 ;; esac
case "$SEEDS" in ''|*[!0-9]*) echo "hot-cu: --seeds must be a decimal count" >&2; exit 2 ;; esac
[ "$SEEDS" -gt 0 ] || { echo "hot-cu: --seeds must be positive" >&2; exit 2; }

die() { echo "hot-cu: $*" >&2; exit 1; }
say() { printf '\n== %s\n' "$*"; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }

command -v cargo >/dev/null 2>&1 || die "cargo not found"
[ -d "$REPO/.git" ] || die "not a repository: $REPO"

LOGS="$WORK/logs"
SWEEP="$WORK/sweep"
BUILT_ELF="$WORK/elf"
mkdir -p "$WORK" "$LOGS" "$SWEEP"
# A re-run must not blend its pass count with a previous shape's logs.
rm -f "$SWEEP"/seed*.log

REVISION="$(git -C "$REPO" rev-parse "${COMMIT:-HEAD}")"
DIRTY="clean"
git -C "$REPO" diff --quiet HEAD -- programs crates Cargo.toml Cargo.lock 2>/dev/null || DIRTY="DIRTY"

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
else
    if [ -n "$COMMIT" ]; then
        BUILD_ROOT="$WORK/source-${REVISION:0:12}"
        ELF_DIR="$WORK/elf-${REVISION:0:12}"
        say "stage archive: $REVISION (clean, from git archive)"
        rm -rf "$BUILD_ROOT"
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
        DIRTY="clean"
        PROVENANCE="$REVISION (clean git archive)"
    else
        ELF_DIR="$BUILT_ELF"
        say "stage elf: $REVISION ($DIRTY working tree)"
        PROVENANCE="$REVISION ($DIRTY working tree)"
    fi
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
            "$WRAP" cargo build-sbf --manifest-path "$1" --sbf-out-dir "$ELF_DIR"
        else
            cargo build-sbf --manifest-path "$1" --sbf-out-dir "$ELF_DIR"
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
fi

TRADING_ELF="$ELF_DIR/dclutch_trading_sbf.so"
[ -f "$TRADING_ELF" ] || die "no trading ELF at $TRADING_ELF"
TRADING_SHA="$(sha256 "$TRADING_ELF")"

# --------------------------------------------------------------- 2. the sweep
say "sweep: $SEEDS fixture seeds against trading ELF ${TRADING_SHA:0:16}..."
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
        cargo test \
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
# MEAN/MIN/MAX are over the seeds that COMPLETED and printed a figure. A seed
# that exhausted the meter has no figure to average -- the pass count is what
# carries it, which is the other half of why M-61 asks for both numbers and not
# for a margin.
say "result"
if [ "$pass" -gt 0 ]; then
    read -r MEAN MIN MAX <<EOF
$(awk '{ t += $1; if (n++ == 0 || $1 < lo) lo = $1; if ($1 > hi) hi = $1 }
       END { printf "%d %d %d\n", int(t / n + 0.5), lo, hi }' "$OBSERVED")
EOF
else
    MEAN=0; MIN=0; MAX=0
fi

printf 'PASS %d/%d\n' "$pass" "$SEEDS"
if [ "$pass" -gt 0 ]; then
    printf "MEAN %'d CU   (over the %d seeds that completed, of 1,400,000)\n" "$MEAN" "$pass"
    printf "MIN  %'d CU\n" "$MIN"
    printf "MAX  %'d CU\n" "$MAX"
    # Rounded to NEAREST, not floored. Per M-61 a delta decomposes as
    # `n * 1,500 + ~50`, so a spread of 49,499 is 33 draws less a small residual,
    # and flooring would report 32 and invite someone to hunt the missing one.
    printf "SPREAD %'d CU  ~ %d bump-search iterations at 1,500 CU each\n" \
        "$((MAX - MIN))" "$(( (MAX - MIN + 750) / 1500 ))"
else
    echo "MEAN -   MIN -   MAX -   (no seed completed)"
fi
printf 'ELF  %s  dclutch_trading_sbf.so\n' "$TRADING_SHA"
printf 'SRC  %s\n' "$PROVENANCE"
echo
echo "M-61: quote PASS and MEAN. MIN is not a margin and MAX is not a bound --"
echo "      both are draws, and any one-byte change to the ELF above redraws"
echo "      every seed. A number quoted without that digest means nothing."

cat > "$WORK/summary.json" <<EOF
{
  "schema": "dclutch-hot-cu-sweep-v1",
  "artifact_provenance": "$PROVENANCE",
  "trading_elf_sha256": "$TRADING_SHA",
  "elf_dir": "$ELF_DIR",
  "seeds": $SEEDS,
  "pass": $pass,
  "fail": $fail,
  "mean_cu": $MEAN,
  "min_cu": $MIN,
  "max_cu": $MAX,
  "ceiling_cu": 1400000,
  "observed_cu": [$(paste -sd, - < "$OBSERVED")],
  "statistic": "PASS and MEAN. min/max are the observed spread of a bump-search lottery (M-61), not bounds; they are keyed to trading_elf_sha256 and a one-byte ELF change redraws every seed."
}
EOF
echo "logs:    $SWEEP"
echo "summary: $WORK/summary.json"

[ "$fail" -eq 0 ] || exit 1
