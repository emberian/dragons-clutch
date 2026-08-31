#!/bin/sh
set -eu

# Resolved from THIS SCRIPT's location, not from `git rev-parse --show-toplevel`.
# The two agree in this repository and disagree in the one that actually runs
# CI: `dragons-clutch` vendors this tree as a `dclutch/` SUBTREE, so the git
# toplevel there is the OUTER root and every path below silently lost its
# `dclutch/` segment --
#
#   Failed to obtain package metadata: manifest path
#   `/home/runner/work/dragons-clutch/dragons-clutch/programs/dclutch-custody-sbf/Cargo.toml`
#   does not exist
#
# which is true, and the manifest exists one directory further in. This was the
# only one of the five suite runners resolving its root from git, which is why
# claims and dealer compiled in the same job while this row never reached a
# single test.
repository=$(cd "$(dirname "$0")/../.." && pwd)
output=$(mktemp -d "${TMPDIR:-/tmp}/dclutch-custody-sbf.XXXXXX")

cleanup() {
  rm -rf -- "$output"
}
trap cleanup EXIT HUP INT TERM

cargo build-sbf \
  --manifest-path "$repository/programs/dclutch-custody-sbf/Cargo.toml" \
  --sbf-out-dir "$output"
cargo build-sbf \
  --manifest-path "$repository/programs/dclutch-custody-sbf/test-programs/caller/Cargo.toml" \
  --sbf-out-dir "$output"
cargo build-sbf \
  --manifest-path "$repository/programs/dclutch-registry-sbf/Cargo.toml" \
  --sbf-out-dir "$output"
SBF_OUT_DIR="$output" cargo test \
  --manifest-path "$repository/programs/dclutch-custody-sbf/Cargo.toml" \
  --test program_test -- --nocapture
