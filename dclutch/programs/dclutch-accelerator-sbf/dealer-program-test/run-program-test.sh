#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-accelerator-dealer.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"
# Exactly the two ELFs the surviving targets load. Trading, Custody, Claims and
# Core were built here for tests/accepted.rs, which drove the real Trading ELF
# over the lock-bounded checkpoint routes; the programs merge (3bee5f3f1)
# deleted that campaign with the routes it drove, and four SBF links kept being
# built for a suite that no longer reads them. tests/frontier.rs loads no ELF at
# all and tests/physical.rs adds these two.
for manifest in \
  programs/dclutch-accelerator-sbf/Cargo.toml \
  programs/dclutch-accelerator-sbf/test-programs/dealer-caller/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-accelerator-sbf/dealer-program-test/Cargo.toml \
  --tests \
  -- --nocapture
