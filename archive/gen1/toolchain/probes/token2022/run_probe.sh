#!/usr/bin/env bash
# Run the Token-2022 probe and print only its own findings.
#
# Everything this touches is local: an in-process Agave bank started by
# `solana-program-test`, with the Token-2022 ELF that `solana-program-binaries`
# installs at genesis.  No RPC, no cluster, no key material, no submission.
set -euo pipefail
cd "$(dirname "$0")"
export RUST_LOG="${RUST_LOG:-error}"
cargo test --locked -- --nocapture --test-threads=1 "$@"
