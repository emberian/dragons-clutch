#!/usr/bin/env bash
# Execute the Direct fee leg in a transaction of its own, against real Custody.
#
# FEE_SECOND_TRANSACTION_V1 §1 argues from source that Custody admits the fee
# request in a LATER transaction. The shipped Direct route cannot produce such a
# transaction -- its transition co-enables the seller and fee legs from one
# register, and the two-leg execution is over the compute ceiling -- so this
# probe stages a release set whose TRADING ROLE is
# `test-programs/custody-leg-caller`, a program that forwards one projected
# Custody request under the caller authority that request derives.
#
# Everything else is real: real Custody, Core, Claims and Registry ELFs, the
# fixture's own byte-exact projected requests, the real Realm and token program.
# The Trading ELF is the ONLY substitution and it is what makes the probe
# possible at all; see the test file's header for what that does and does not
# buy.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
work="${DCLUTCH_FEE2TX_WORK:-$(mktemp -d /private/tmp/dclutch-fee2tx.XXXXXX)}"
probe="$work/probe"
caller="$work/caller"
mkdir -p "$probe" "$caller"

cd "$repo_root"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
export SWARM_MEM_MAX="${SWARM_MEM_MAX:-32G}"

cargo_command=(cargo)
if command -v swarm-build >/dev/null 2>&1; then
  cargo_command=(swarm-build cargo)
fi

build_sbf() {
  manifest="$1"
  output="$2"
  label="$(basename "$(dirname "$manifest")")"
  log="$work/build-$label.log"
  "${cargo_command[@]}" build-sbf --manifest-path "$manifest" --sbf-out-dir "$output" \
    >"$log" 2>&1 || { tail -n 60 "$log" >&2; exit 1; }
  count="$(grep -c 'overwrites values in the frame' "$log" || true)"
  printf '  built %-72s %s frame diagnostics\n' "$manifest" "${count:-0}"
  if [ "${count:-0}" != "0" ]; then
    grep 'overwrites values in the frame' "$log" | sort -u >&2
    echo "run-fee-second-transaction.sh: refusing $manifest" >&2
    exit 1
  fi
}

# The four roles this probe executes for real. Trading is deliberately absent:
# its slot in the release set is the caller below, and building the real one
# would only stage bytes nothing in this probe runs.
for manifest in \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-custody-sbf/Cargo.toml
do
  build_sbf "$manifest" "$probe"
done

build_sbf programs/dclutch-trading-sbf/program-test/test-programs/custody-leg-caller/Cargo.toml \
  "$caller"

caller_elf="$caller/dclutch_custody_leg_caller_test_program.so"
cp "$caller_elf" "$probe/dclutch_trading_sbf.so"

SBF_OUT_DIR="$probe" \
DCLUTCH_CUSTODY_LEG_CALLER_ELF="$caller_elf" \
  "${cargo_command[@]}" test \
    --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    --test direct_hot_fee_second_transaction \
    -- --nocapture

printf 'fee second-transaction evidence retained at %s\n' "$work"
