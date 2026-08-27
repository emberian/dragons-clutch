#!/usr/bin/env bash
# Resume the successor campaign's ledger as a LIVE localhost chain.
#
# The campaign's own supervisor kills the validator when the run returns
# (ValidatorChild::drop). The ledger it leaves behind holds the finalized
# post-campaign state, including the OPEN Market. This restarts a validator on
# that exact ledger with the launcher's pinned network profile so the browser
# has something to read.
#
# No --account-dir and no --upgradeable-program: those are genesis-time inputs
# and the genesis is already in the ledger. No --reset: that would erase it.
set -euo pipefail
LEDGER="${1:?usage: resume-validator.sh ABSOLUTE_LEDGER_DIR [RPC_PORT]}"
PORT="${2:-21890}"
[ -d "$LEDGER" ] || { echo "no such ledger: $LEDGER" >&2; exit 1; }
exec solana-test-validator \
  --config /dev/null \
  --ledger "$LEDGER" \
  --ticks-per-slot 16 \
  --bind-address 127.0.0.1 \
  --rpc-port "$PORT" \
  --faucet-port $((PORT + 2)) \
  --gossip-port $((PORT + 3)) \
  --dynamic-port-range $((PORT + 10))-$((PORT + 41))
