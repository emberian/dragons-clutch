#!/usr/bin/env bash
# Build one coherent real-SBF Direct Hot release plus three isolated child
# adversaries, then prove Trading refuses each exact post-child mismatch and
# rolls the whole transaction back.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
work="${DCLUTCH_POSTJOIN_WORK:-$(mktemp -d /private/tmp/dclutch-postjoin.XXXXXX)}"
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

run_case() {
  name="$1"
  SBF_OUT_DIR="$real_out" \
  POSTJOIN_SBF_OUT_DIR="$hostile_out" \
  DCLUTCH_FIXTURE_SUBSTRATE=slot-pinned \
    "${cargo_command[@]}" test --locked \
      --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
      --test registry_hot_continuation "$name" -- --exact --nocapture
}

run_case real_registry_executes_profile14_direct_hot_under_protocol_limit
run_case nonselected_claims_supply_corruption_after_real_child_commit_rolls_back
run_case omitted_token_close_authority_corruption_after_real_custody_commit_rolls_back
run_case omitted_custody_replay_lineage_corruption_after_real_child_commit_rolls_back

printf 'postjoin hostile evidence retained at %s\n' "$work"
