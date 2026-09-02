#!/usr/bin/env bash
# Fetch the credential-free sponsored SOL/USD PriceUpdateV2 from Solana devnet.
#
# usage: tools/release/devnet-price-update.sh --out ABSOLUTE_NEW_FILE [--url RPC]
#
#   --out PATH   absolute path of the file to create; required, never overwritten
#   --url URL    JSON-RPC endpoint (default: https://api.devnet.solana.com)
#
# This is a read-only input producer for `devnet-sponsored-market`: it never
# reads a keypair, signs, submits, funds, or calls Hermes / Pyth Price Service.
set -euo pipefail

URL="https://api.devnet.solana.com"
OUT=""
ACCOUNT="7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"
RECEIVER="rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
DEVNET_GENESIS="EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --url) URL="${2:?--url needs a value}"; shift 2 ;;
        --out) OUT="${2:?--out needs a value}"; shift 2 ;;
        # The whole leading comment block, found rather than counted: a
        # hardcoded line range is a second author for where the help ends, and
        # it was already wrong -- it stopped two lines above the flags a reader
        # is told to pass, so `--help` did not admit --url or --out existed.
        -h|--help) sed -n '2,/^[^#]/p' "$0" | sed '$d'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$OUT" ]; then
    echo "--out ABSOLUTE_NEW_FILE is required" >&2
    exit 2
fi
case "$OUT" in
    /*) ;;
    *) echo "--out must be absolute" >&2; exit 2 ;;
esac
if [ -e "$OUT" ]; then
    echo "refusing to overwrite existing output: $OUT" >&2
    exit 2
fi

WORK="$(mktemp -d)"
TMP="$(mktemp "${OUT}.tmp.XXXXXX")"
trap 'rm -rf "$WORK"; rm -f "$TMP"' EXIT

rpc() {
    curl -sS -X POST -H 'Content-Type: application/json' -d "$1" "$URL" > "$WORK/result.json"
}

rpc '{"jsonrpc":"2.0","id":1,"method":"getGenesisHash","params":[]}'
GENESIS="$(python3 - "$WORK/result.json" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["result"])
PY
)"
if [ "$GENESIS" != "$DEVNET_GENESIS" ]; then
    echo "refusing non-devnet genesis: $GENESIS" >&2
    exit 3
fi

rpc "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"getAccountInfo\",\"params\":[\"$ACCOUNT\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}"
python3 - "$WORK/result.json" "$TMP" "$RECEIVER" <<'PY'
import base64, json, sys

account = json.load(open(sys.argv[1])).get("result", {}).get("value")
if account is None:
    raise SystemExit("sponsored PriceUpdateV2 account is absent")
if account.get("owner") != sys.argv[3] or account.get("executable"):
    raise SystemExit("sponsored PriceUpdateV2 does not have the pinned Receiver ownership")
data = account.get("data")
if not isinstance(data, list) or len(data) != 2 or data[1] != "base64":
    raise SystemExit("sponsored PriceUpdateV2 response is not base64 account data")
body = base64.b64decode(data[0], validate=True)
if len(body) != 134:
    raise SystemExit(f"sponsored PriceUpdateV2 body is {len(body)} bytes, expected 134")
open(sys.argv[2], "wb").write(body)
PY
mv "$TMP" "$OUT"
trap - EXIT
rm -rf "$WORK"
printf 'wrote %s (134-byte finalized sponsored SOL/USD PriceUpdateV2)\n' "$OUT"
