#!/bin/sh
# Boot a fresh local validator with the real upgraded Pyth devnet programs and
# Config account cloned from canonical devnet. This performs public RPC reads
# only. It never reads a wallet, requests an airdrop, signs, or submits to the
# public cluster.
set -eu

RPC_URL="https://api.devnet.solana.com"
EXPECTED_GENESIS="EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
RECEIVER="rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp"
ROUTER="HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL"
PUSH_ORACLE="pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou"
CONFIG="H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye"

RPC_PORT="${DC_PYTH_CLONE_RPC_PORT:-9147}"
# Agave reserves RPC_PORT + 1 (9148) for RPC WebSocket.
FAUCET_PORT="${DC_PYTH_CLONE_FAUCET_PORT:-9149}"
GOSSIP_PORT="${DC_PYTH_CLONE_GOSSIP_PORT:-9150}"
DYNAMIC_PORT_RANGE="${DC_PYTH_CLONE_DYNAMIC_PORT_RANGE:-9151-9199}"

for required_tool in curl jq solana-test-validator; do
    if ! command -v "$required_tool" >/dev/null 2>&1; then
        echo "missing required tool: $required_tool" >&2
        exit 1
    fi
done

genesis_hash=$(
    curl --fail --silent --show-error --max-time 30 \
        -X POST -H 'Content-Type: application/json' \
        --data '{"jsonrpc":"2.0","id":1,"method":"getGenesisHash"}' \
        "$RPC_URL" \
        | jq -er '.result'
)

if [ "$genesis_hash" != "$EXPECTED_GENESIS" ]; then
    echo "refusing non-canonical devnet genesis: $genesis_hash" >&2
    exit 1
fi

clone_ledger=$(mktemp -d "${TMPDIR:-/tmp}/dragons-clutch-pyth-clone.XXXXXX")

echo "canonical devnet genesis: $genesis_hash"
echo "fresh clone ledger: $clone_ledger"
echo "local RPC after boot: http://127.0.0.1:$RPC_PORT"
echo "local RPC WebSocket port: $((RPC_PORT + 1))"
echo "the ledger is retained after exit for inspection"

# Stock Agave applies --bind-address to gossip/node sockets, not RPC/faucet.
exec solana-test-validator \
    --ledger "$clone_ledger" \
    --quiet \
    --bind-address 127.0.0.1 \
    --url "$RPC_URL" \
    --clone-feature-set \
    --clone-upgradeable-program "$RECEIVER" \
    --clone-upgradeable-program "$ROUTER" \
    --clone-upgradeable-program "$PUSH_ORACLE" \
    --clone "$CONFIG" \
    --rpc-port "$RPC_PORT" \
    --faucet-port "$FAUCET_PORT" \
    --gossip-port "$GOSSIP_PORT" \
    --dynamic-port-range "$DYNAMIC_PORT_RANGE"
