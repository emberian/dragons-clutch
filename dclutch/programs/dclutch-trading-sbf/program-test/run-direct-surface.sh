#!/usr/bin/env bash
# Every binary of the Trading program-test surface, on one coherent ELF set.
#
# WHY THIS EXISTS. This workspace has nineteen integration binaries and, until
# 2026-09-03, no runner that executed them together. `run-program-test.sh` runs
# `activation`; `run-postjoin-hostiles.sh` runs three named cases of
# `registry_hot_continuation`; `run-fee-pair.sh`, `run-fee-second-transaction.sh`,
# `run-begin-retiring.sh` and `run-close-maker.sh` each run one. `tools/gate`'s
# `programs` tier runs the package unfiltered, which is the closest thing there
# was -- and it did not build every ELF the package reads, so two binaries
# failed on a MISSING PREREQUISITE while looking exactly like a failing test:
#
#   * `direct_registered_creation_hot` reads `dclutch_rent_sbf.so`, and
#     `PROGRAM_MANIFESTS` did not build `programs/dclutch-rent-sbf`. Every run
#     panicked on `expect("real Rent ELF")`.
#   * the three `registry_hot_continuation` postjoin hostiles read
#     `POSTJOIN_SBF_OUT_DIR`, which nothing set, so they panicked on an unset
#     variable while proving nothing. The `programs` tier skips them by name and
#     says why -- the Hot continuation is decision 0030's demoted route and must
#     not gate the production tier -- so they run HERE, where the hostile links
#     are built beside the real ones and their prerequisite exists.
#
# So: one out directory, twelve links, all nineteen binaries, EVERY ROW RUN AND
# EVERY ROW REPORTED. A bare `set -e` loop stops at the first red and reports one
# number for a surface that was never executed (`run-postjoin-hostiles.sh` paid
# for that lesson: it reported one failing case when the true figure was ten), so
# the accounting below is the point of the script, not decoration.
#
# usage: run-direct-surface.sh [--out DIR] [--keep]
set -uo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
out=""
keep=false
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --keep) keep=true; shift ;;
    *) echo "run-direct-surface.sh: unknown argument: $1" >&2; exit 64 ;;
  esac
done
if [ -z "$out" ]; then
  out="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-direct-surface.XXXXXX")"
  [ "$keep" = true ] || trap 'rm -rf -- "$out"' EXIT HUP INT TERM
fi
logs="$out/logs"
mkdir -p "$out" "$logs"
cd "$repo_root"

printf 'tree      %s\n' "$(git rev-parse --show-toplevel)"
printf 'commit    %s\n' "$(git rev-parse HEAD)"
printf 'elf out   %s\n\n' "$out"

# ---------------------------------------------------------------- 1. the links
#
# `cargo build-sbf` exits ZERO when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution", so the log is the only signal there is. Count them and refuse:
# an artifact the toolchain calls potentially-undefined has no business
# producing evidence.
#
# The three postjoin adversaries land in the SAME directory as the real set on
# purpose. Their file names are distinct
# (`dclutch_postjoin_*_hostile_sbf.so`), so there is no collision, and
# `POSTJOIN_SBF_OUT_DIR` can then be this one directory rather than a second one
# whose contents have to be kept in step with it.
diagnostics=0
for manifest in \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-custody-sbf/Cargo.toml \
  programs/dclutch-rent-sbf/Cargo.toml \
  programs/dclutch-trading-sbf/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/trading-outer/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/core-caller/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/registry/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/postjoin-claims/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/postjoin-custody/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/postjoin-token/Cargo.toml
do
  link="$(basename "$(dirname "$manifest")")"
  log="$logs/build-$link.log"
  if ! cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$out" >"$log" 2>&1; then
    echo "run-direct-surface.sh: SBF build failed: $manifest" >&2
    tail -n 40 "$log" >&2
    exit 1
  fi
  count="$(grep -c 'overwrites values in the frame' "$log" || true)"
  printf '  built %-18s %s frame diagnostics\n' "$link" "${count:-0}"
  if [ "${count:-0}" != "0" ]; then
    grep 'overwrites values in the frame' "$log" | sort -u >&2
  fi
  diagnostics=$((diagnostics + count))
done
if [ "$diagnostics" -ne 0 ]; then
  echo "run-direct-surface.sh: refusing -- $diagnostics SBF stack-frame-overwrite" \
       "diagnostics. The toolchain says these calls may cause undefined behavior" \
       "during execution; fix the frame, do not measure on top of it." >&2
  exit 1
fi

# ----------------------------------------------------------- 2. every binary
#
# One `cargo test --test <name>` per binary rather than one unfiltered run, so a
# binary that fails to LINK is distinguishable from one whose tests fail, and so
# the per-binary counts below are read from that binary's own summary line
# instead of a single package-wide total.
names=(
  activation
  capability_seal_close
  direct_begin_retiring_on_chain
  direct_close_maker_on_chain
  direct_hot_bump_hints
  direct_hot_fee_bearing_margin_gate
  direct_hot_fee_pair
  direct_hot_pda_depth_census
  direct_hot_record_depth_census
  direct_hot_top_level
  direct_hot_top_level_margin_gate
  direct_registered_creation_hot
  hot_heap_frame_is_inert
  hot_tail_profile
  registry_hot_continuation
  series_permit_expiry_hot_wall
  series_pre_market_expiry_program_test
  slot_pin_supersession
  ticket_authored_intents_execute_top_level
)
printf '\n'
total_passed=0
total_failed=0
never_ran=()
for name in "${names[@]}"; do
  log="$logs/test-$name.log"
  SBF_OUT_DIR="$out" POSTJOIN_SBF_OUT_DIR="$out" cargo test \
    --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    --test "$name" -- --nocapture >"$log" 2>&1
  summary="$(grep -E '^test result:' "$log" | tail -n 1)"
  if [ -z "$summary" ]; then
    # No summary line at all means the binary never ran -- a compile or link
    # failure. "did not run" is not "failed", and conflating them is how a
    # silent red becomes invisible from outside.
    printf '  %-42s DID NOT RUN\n' "$name"
    never_ran+=("$name")
    tail -n 20 "$log" >&2
    continue
  fi
  passed="$(printf '%s' "$summary" | sed -n 's/.* \([0-9]*\) passed.*/\1/p')"
  failed="$(printf '%s' "$summary" | sed -n 's/.* \([0-9]*\) failed.*/\1/p')"
  printf '  %-42s %3s passed / %3s failed\n' "$name" "${passed:-0}" "${failed:-0}"
  total_passed=$((total_passed + ${passed:-0}))
  total_failed=$((total_failed + ${failed:-0}))
done

printf '\nDirect surface: %s passed / %s failed across %s binaries' \
  "$total_passed" "$total_failed" "${#names[@]}"
if [ "${#never_ran[@]}" -ne 0 ]; then
  printf ', %s DID NOT RUN (%s)' "${#never_ran[@]}" "${never_ran[*]}"
fi
printf '\nlogs: %s\n' "$logs"
[ "$total_failed" -eq 0 ] && [ "${#never_ran[@]}" -eq 0 ]
