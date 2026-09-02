#!/usr/bin/env bash
# Run one ticket board locally, on loopback, with a snapshot beside it.
#
#   bash tools/ticket-board/run-local.sh
#
# Any argument is passed through to the binary, so the flags in `--help` all
# work here: `run-local.sh --bind 0.0.0.0:8787`, `--market <PUBKEY>`, and so on.
#
# It reads no chain, holds no key, and takes no credential. Nothing it does can
# move a lamport.
set -euo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

# Debug build on purpose: this is devnet-grade infrastructure and a board is
# bounded by its network, not by its parser. `--release` if you disagree.
cargo build --quiet

exec ./target/debug/dclutch-ticket-board \
  --snapshot "${DCLUTCH_TICKET_BOARD_SNAPSHOT:-$here/ticket-board-snapshot.json}" \
  "$@"
