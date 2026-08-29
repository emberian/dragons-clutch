#!/usr/bin/env bash
# Fetch one fully verified Pyth PriceUpdateV2 body from Solana devnet.
#
# This is deliberately a read-only input producer for
# `dclutch-local-successor-bootstrap devnet-market`.  It never reads a
# keypair, signs, submits, funds, or calls a Pyth HTTP service.
set -euo pipefail

URL="https://api.devnet.solana.com"
OUT=""
ACCOUNT="7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE"
RECEIVER="rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp"
DEVNET_GENESIS="EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --url) URL="${2:?--url needs a value}"; shift 2 ;;
        --out) OUT="${2:?--out needs a value}"; shift 2 ;;
        --account) ACCOUNT="${2:?--account needs a value}"; shift 2 ;;
        -h|--help)
            sed -n '2,11p' "$0"
            exit 0
            ;;
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
    curl -sS -X POST -H 'Content-Type: application/json' \
        -d "$1" "$URL" > "$WORK/result.json"
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

reply = json.load(open(sys.argv[1]))
account = reply.get("result", {}).get("value")
if account is None:
    raise SystemExit("PriceUpdateV2 account is absent")
if account.get("owner") != sys.argv[3]:
    raise SystemExit(f"PriceUpdateV2 owner is {account.get('owner')}, not the pinned receiver")
data = account.get("data")
if not isinstance(data, list) or len(data) != 2 or data[1] != "base64":
    raise SystemExit("PriceUpdateV2 response is not base64 account data")
body = base64.b64decode(data[0], validate=True)
if len(body) != 134:
    raise SystemExit(f"PriceUpdateV2 body is {len(body)} bytes, expected 134")
open(sys.argv[2], "wb").write(body)
PY
mv "$TMP" "$OUT"
trap - EXIT
rm -rf "$WORK"
printf 'wrote %s (134-byte finalized devnet PriceUpdateV2, account %s)\n' "$OUT" "$ACCOUNT"
