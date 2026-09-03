#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-rational-lifecycle.XXXXXX)"
# EXIT 2, not 1: without the pinned crate archive this suite DID NOT RUN, and
# `tools/ci/run.sh` distinguishes that from a gate that failed. A `:?` expansion
# exits 1 under `set -u`, which reads as a protocol finding about a tree that
# was never measured.
#
# `TOKEN_2022_V11_CRATE` is required either way and is NOT interchangeable with
# `TOKEN_2022_V11_ELF`: the fixture builder verifies the crate archive's digest
# and the whole toolchain manifest BEFORE it reaches the prepared-ELF
# short-circuit, so the archive is a prerequisite of the provenance check
# itself. Setting `TOKEN_2022_V11_ELF` as well is what lets this suite run off
# Linux-x86_64 -- the builder then copies that artifact instead of compiling
# one, and still verifies its digest and length against the provenance.
if [ -z "${TOKEN_2022_V11_CRATE:-}" ]; then
  echo "TOKEN_2022_V11_CRATE is unset, so this suite DID NOT RUN. See" >&2
  echo "programs/dclutch-claims-sbf/fixtures/README.md; off Linux-x86_64 set" >&2
  echo "TOKEN_2022_V11_ELF to a canonical artifact beside it." >&2
  exit 2
fi
fixture_builder="$repo_root/programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"
for manifest in \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/test-programs/rational-lifecycle-caller/Cargo.toml \
  programs/dclutch-rent-sbf/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done
"$fixture_builder" "$TOKEN_2022_V11_CRATE" "$sbf_out"

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/program-test/rational-lifecycle/Cargo.toml \
  --test lifecycle \
  -- --nocapture
