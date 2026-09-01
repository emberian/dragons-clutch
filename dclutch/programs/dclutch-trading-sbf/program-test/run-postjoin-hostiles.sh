#!/usr/bin/env bash
# Build one coherent real-SBF Direct Hot release plus three isolated child
# adversaries, then prove Trading refuses each exact post-child mismatch and
# rolls the whole transaction back.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
# TMPDIR, not a literal /private/tmp: that path is macOS-specific and does not
# exist on the ubuntu-24.04 runner, so `mktemp -d` failed before the first ELF
# was built. Pin an exact directory with DCLUTCH_POSTJOIN_WORK when you want
# the hostile evidence kept somewhere specific.
work="${DCLUTCH_POSTJOIN_WORK:-$(mktemp -d "${TMPDIR:-/tmp}/dclutch-postjoin.XXXXXX")}"
real_out="$work/real"
hostile_out="$work/hostile"
target="$work/target"
mkdir -p "$real_out" "$hostile_out" "$target"

cd "$repo_root"
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR="$target"
export SWARM_MEM_MAX="${SWARM_MEM_MAX:-32G}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

cargo_command=(cargo)
if command -v swarm-build >/dev/null 2>&1; then
  cargo_command=(swarm-build cargo)
fi

build_sbf() {
  manifest="$1"
  output="$2"
  label="$(basename "$(dirname "$manifest")")"
  log="$work/build-$label.log"
  "${cargo_command[@]}" build-sbf --manifest-path "$manifest" --sbf-out-dir "$output" -- --locked \
    >"$log" 2>&1 || { tail -n 60 "$log" >&2; exit 1; }
  count="$(grep -c 'overwrites values in the frame' "$log" || true)"
  printf '  built %-74s %s frame diagnostics\n' "$manifest" "${count:-0}"
  if [ "${count:-0}" != "0" ]; then
    grep 'overwrites values in the frame' "$log" | sort -u >&2
    echo "run-postjoin-hostiles.sh: refusing $manifest" >&2
    exit 1
  fi
}

for manifest in \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-custody-sbf/Cargo.toml \
  programs/dclutch-trading-sbf/Cargo.toml
do
  build_sbf "$manifest" "$real_out"
done

for manifest in \
  programs/dclutch-trading-sbf/program-test/test-programs/postjoin-claims/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/postjoin-custody/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/postjoin-token/Cargo.toml
do
  build_sbf "$manifest" "$hostile_out"
done

# EVERY case runs, and every case is reported. This used to be four bare
# invocations under `set -e`, so the first failure aborted the script and the
# remaining rows never ran -- and nothing said so. Measured 2026-09-01: the
# control was failing, CI reported exactly one failing case, and the true
# figure in that test file was TEN. Three of the rows below had not been
# executed at all, and "did not run" had been presented as though it were the
# whole result. Keep the accounting; it is the only reason a silent red here
# is visible from outside.
declare -a passed=() failed=()

run_case() {
  name="$1"
  if SBF_OUT_DIR="$real_out" \
     POSTJOIN_SBF_OUT_DIR="$hostile_out" \
     DCLUTCH_FIXTURE_SUBSTRATE=slot-pinned \
       "${cargo_command[@]}" test --locked \
         --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
         --test registry_hot_continuation "$name" -- --exact --nocapture
  then
    passed+=("$name")
  else
    failed+=("$name")
  fi
}

# The first row is the CONTROL: the same bundle with nothing hostile about it.
# The three that follow are hostiles, and each one is only meaningful while the
# control executes -- a hostile that "passes" because the honest path already
# refused is a test of nothing (`AGENTS.md`, ledger M-38).
control=real_registry_executes_profile14_direct_hot_under_protocol_limit

set +e
run_case "$control"
run_case nonselected_claims_supply_corruption_after_real_child_commit_rolls_back
run_case omitted_token_close_authority_corruption_after_real_custody_commit_rolls_back
run_case omitted_custody_replay_lineage_corruption_after_real_child_commit_rolls_back
set -e

printf '\n=== postjoin ===\n'
for name in "${passed[@]}"; do printf '  passed  %s\n' "$name"; done
for name in "${failed[@]}"; do printf '  FAILED  %s\n' "$name"; done

control_failed=0
for name in "${failed[@]}"; do
  [ "$name" = "$control" ] && control_failed=1
done
if [ "$control_failed" = "1" ]; then
  echo "  the CONTROL failed, so every hostile verdict above proves NOTHING:" >&2
  echo "  each of them can refuse for the honest path's reason instead of its own." >&2
fi

printf 'postjoin hostile evidence retained at %s\n' "$work"

if [ "${#failed[@]}" -ne 0 ]; then
  printf '%s of %s postjoin rows failed\n' "${#failed[@]}" "$(( ${#passed[@]} + ${#failed[@]} ))" >&2
  exit 1
fi
