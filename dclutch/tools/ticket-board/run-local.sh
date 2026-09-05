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

# `--help` is answered HERE, before the build, and it answers for the WRAPPER
# only. The board's own flag list belongs to the board and is printed by the
# board; restating it here would be a second author for it, and a wrapper that
# cargo-builds a binary in order to forward `--help` costs a minute to answer a
# question about a shell script. `tools/doc-commands` declines to probe any
# program whose source does not handle a help flag, so before this arm the
# command `docs/operators/author-a-ticket.md` publishes was unprobed.
case "${1:-}" in
-h | --help)
  cat <<'USAGE'
usage: bash tools/ticket-board/run-local.sh [board flag ...]

Builds dclutch-ticket-board from this checkout and runs it on loopback with a
snapshot file beside it. Every argument is passed through to the board, whose
own flags -- --bind, --market, and the rest -- are printed by:

    cargo run --quiet --manifest-path tools/ticket-board/Cargo.toml -- --help

  -h, --help   this page
  DCLUTCH_TICKET_BOARD_SNAPSHOT   snapshot path (default: tools/ticket-board/ticket-board-snapshot.json)

The board reads no chain, holds no key, and takes no credential.
USAGE
  exit 0
  ;;
esac

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

# Debug build on purpose: this is devnet-grade infrastructure and a board is
# bounded by its network, not by its parser. `--release` if you disagree.
# One workspace, one target directory: build by package name from the root.
repo="$(cd -- "$here/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$repo/target}"
(cd "$repo" && cargo build --quiet -p dclutch-ticket-board)

exec "$target/debug/dclutch-ticket-board" \
  --snapshot "${DCLUTCH_TICKET_BOARD_SNAPSHOT:-$here/ticket-board-snapshot.json}" \
  "$@"
