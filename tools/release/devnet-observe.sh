#!/usr/bin/env bash
# Bounded, read-only observation of Solana devnet for the dClutch demo deploy.
#
# THIS SCRIPT NEVER WRITES. It issues only JSON-RPC read methods, and an
# allowlist in `rpc()` refuses anything else. It does not sign, submit, fund,
# airdrop, deploy, or read any keypair; it takes no keypair argument and has no
# code path that could acquire one.
#
# It answers exactly two operational questions and logs every call it makes:
#
#   1. Is the devnet Pyth wiring this repo pins still the live wiring?
#      (genesis hash; the three Program accounts; the three ProgramData headers
#      -- deployment slot and upgrade authority; the receiver Config;
#      GuardianSet[0]; the bridge config; the SOL/USD PriceUpdateV2 and its age
#      right now.)
#
#   2. What does devnet actually charge, today, to rent-exempt the accounts a
#      deploy would create? Two probes fix the affine rent parameters and one
#      more confirms linearity at a megabyte; every artifact price is then
#      exact arithmetic rather than another round trip.
#
# Usage:
#   tools/release/devnet-observe.sh [--url URL] [--elf-dir DIR] [--out DIR]
#                                   [--cadence]
#
#   --url      RPC endpoint (default https://api.devnet.solana.com).
#              Refuses to run against mainnet-beta's genesis hash.
#   --elf-dir  directory of <role>.so artifacts to price
#   --out      write the read log here
#   --cadence  additionally sample SOL/USD posting cadence (3 extra reads)
set -euo pipefail

URL="https://api.devnet.solana.com"
ELF_DIR=""
OUT=""
CADENCE="false"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --url) URL="${2:?--url needs a value}"; shift 2 ;;
        --elf-dir) ELF_DIR="${2:?--elf-dir needs a value}"; shift 2 ;;
        --out) OUT="${2:?--out needs a value}"; shift 2 ;;
        --cadence) CADENCE="true"; shift ;;
        -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
CALLS=0
LOGFILE="$WORK/rpc-reads.log"
: > "$LOGFILE"

# The enforcement point for the "reads only" claim.
readonly_method() {
    case "$1" in
        getGenesisHash|getVersion|getAccountInfo|getMultipleAccounts|\
        getMinimumBalanceForRentExemption|getBlockHeight|getEpochInfo|\
        getRecentPrioritizationFees|getSignaturesForAddress|getSlot|getBlockTime|getFeeForMessage)
            return 0 ;;
        *) return 1 ;;
    esac
}

rpc() {
    local method="$1" params="$2"
    if ! readonly_method "$method"; then
        echo "REFUSED: $method is not a read method" >&2
        exit 3
    fi
    CALLS=$((CALLS + 1))
    printf '%s\t%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$method" "$params" >> "$LOGFILE"
    curl -sS -X POST -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":$CALLS,\"method\":\"$method\",\"params\":$params}" \
        "$URL" > "$WORK/last.json"
}

result() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["result"])' "$WORK/last.json"; }

echo "== cluster =="
rpc getGenesisHash '[]'
GENESIS="$(result)"
echo "genesis_hash                 $GENESIS"
case "$GENESIS" in
    EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG)
        echo "cluster                      devnet (matches the pinned bound fact)" ;;
    5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d)
        echo "cluster                      MAINNET-BETA -- refusing" >&2; exit 4 ;;
    *)  echo "cluster                      UNRECOGNISED genesis hash" ;;
esac

rpc getVersion '[]'
python3 - "$WORK/last.json" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))["result"]
print(f"solana-core                  {r.get('solana-core')}")
print(f"feature-set                  {r.get('feature-set')}")
PY

rpc getEpochInfo '[]'
python3 - "$WORK/last.json" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))["result"]
print(f"absoluteSlot                 {r['absoluteSlot']}")
print(f"epoch                        {r['epoch']}")
PY

ROUTER=HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL
RECEIVER=rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp
PUSH=pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou
ROUTER_PD=9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x
RECEIVER_PD=3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX
PUSH_PD=9nxngQjxBGUZ3ajfqoTrpiuDBVfztXCQVDuWDAw52Gew
CONFIG=H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye
GUARDIANS=CJHmJw4FuvLTUfPsYepyVCQkUR8qv1AtZbkwsS36hEcd
BRIDGE=GPhDjebMkciFeemuNGaUn5RsmxauQL7UZArqRDjCSZSW
SOLUSD=7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE

echo
echo "== pyth Program accounts (Loader V3, 36 B, tag 2) =="
rpc getMultipleAccounts "[[\"$ROUTER\",\"$RECEIVER\",\"$PUSH\"],{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]"
python3 - "$WORK/last.json" "$ROUTER" "$RECEIVER" "$PUSH" <<'PY'
import base64, json, sys
names = ["router  ", "receiver", "push    "]
accts = json.load(open(sys.argv[1]))["result"]["value"]
for name, addr, a in zip(names, sys.argv[2:5], accts):
    if a is None:
        print(f"{name} {addr} ABSENT"); continue
    d = base64.b64decode(a["data"][0])
    tag = int.from_bytes(d[0:4], "little")
    print(f"{name} {addr}")
    print(f"           len={len(d)} tag={tag} exec={a['executable']} owner={a['owner']}")
PY

echo
echo "== pyth ProgramData headers (first 45 B) =="
B58PY='
B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
def b58(b):
    n = int.from_bytes(b, "big"); s = ""
    while n:
        n, r = divmod(n, 58); s = B58[r] + s
    return "1" * (len(b) - len(b.lstrip(b"\x00"))) + s
'
for pair in "router:$ROUTER_PD" "receiver:$RECEIVER_PD" "push:$PUSH_PD"; do
    role="${pair%%:*}"; addr="${pair##*:}"
    rpc getAccountInfo "[\"$addr\",{\"encoding\":\"base64\",\"commitment\":\"finalized\",\"dataSlice\":{\"offset\":0,\"length\":45}}]"
    python3 - "$WORK/last.json" "$role" "$addr" <<PY
import base64, json, sys
$B58PY
v = json.load(open(sys.argv[1]))["result"]["value"]
d = base64.b64decode(v["data"][0])
tag = int.from_bytes(d[0:4], "little")
slot = int.from_bytes(d[4:12], "little")
auth = b58(d[13:45]) if d[12] == 1 else "None (IMMUTABLE)"
print(f"{sys.argv[2]:9s} {sys.argv[3]}")
print(f"          tag={tag} space={v['space']} deploy_slot={slot}")
print(f"          upgrade_authority={auth}")
PY
done

echo
echo "== pyth trust root =="
rpc getMultipleAccounts "[[\"$CONFIG\",\"$GUARDIANS\",\"$BRIDGE\",\"$SOLUSD\"],{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]"
python3 - "$WORK/last.json" <<PY
import base64, hashlib, json, sys, time
$B58PY
r = json.load(open(sys.argv[1]))["result"]["value"]
cfg, gs, br, pu = (base64.b64decode(a["data"][0]) if a else None for a in r)

o = 8
gov = b58(cfg[o:o+32]); o += 32
o += 1 + (32 if cfg[o] == 1 else 0)
wh = b58(cfg[o:o+32]); o += 32
n = int.from_bytes(cfg[o:o+4], "little"); o += 4
srcs = []
for _ in range(n):
    chain = int.from_bytes(cfg[o:o+2], "little"); o += 2
    srcs.append((chain, b58(cfg[o:o+32]))); o += 32
fee = int.from_bytes(cfg[o:o+8], "little"); o += 8
minsig = cfg[o]
print(f"receiver Config        {len(cfg)} B  sha256={hashlib.sha256(cfg).hexdigest()}")
print(f"  governance_authority   {gov}")
print(f"  wormhole               {wh}")
for chain, em in srcs:
    print(f"  data_source            chain={chain} emitter={em}")
print(f"  single_update_fee      {fee}")
print(f"  minimum_signatures     {minsig}")

idx = int.from_bytes(gs[0:4], "little"); cnt = int.from_bytes(gs[4:8], "little")
tail = gs[8 + 20*cnt:]
print(f"GuardianSet[{idx}]         {len(gs)} B  sha256={hashlib.sha256(gs).hexdigest()}")
print(f"  cardinality            {cnt}  (strict majority {cnt//2+1})")
print(f"  creation_time          {int.from_bytes(tail[0:4],'little')}")
print(f"  expiration_time        {int.from_bytes(tail[4:8],'little')}")
for i in range(cnt):
    print(f"  [{i}]                    0x{gs[8+20*i:8+20*(i+1)].hex()}")
# Wormhole "Bridge" PDA, 24 B: guardian_set_index u32 | last_lamports u64
#                            | guardian_set_expiration_time u32 | fee u64
print(f"bridge config          {len(br)} B  sha256={hashlib.sha256(br).hexdigest()}")
print(f"  guardian_set_index     {int.from_bytes(br[0:4],'little')}")
print(f"  last_lamports          {int.from_bytes(br[4:12],'little')}")
print(f"  guardian_set_expiry    {int.from_bytes(br[12:16],'little')}")
print(f"  fee                    {int.from_bytes(br[16:24],'little')}")

o = 8
wa = b58(pu[o:o+32]); o += 32
vl = pu[o]; o += 1
if vl == 0:
    o += 1
feed = pu[o:o+32].hex(); o += 32
price = int.from_bytes(pu[o:o+8], "little", signed=True); o += 8
conf = int.from_bytes(pu[o:o+8], "little"); o += 8
expo = int.from_bytes(pu[o:o+4], "little", signed=True); o += 4
pt = int.from_bytes(pu[o:o+8], "little", signed=True); o += 8
now = int(time.time())
print(f"SOL/USD PriceUpdateV2  {len(pu)} B  sha256={hashlib.sha256(pu).hexdigest()}")
print(f"  owner                  (see Program accounts above -- receiver-owned)")
print(f"  write_authority        {wa}")
print(f"  verification_level     {'Full' if vl == 1 else 'Partial'}")
print(f"  feed_id                0x{feed}")
print(f"  price                  {price} x 10^{expo} = {price * (10 ** expo):.6f}")
print(f"  conf                   {conf}")
print(f"  publish_time           {pt} ({time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime(pt))})")
print(f"  AGE AT READ TIME       {now - pt} s")
PY
python3 -c '
import json, sys
r = json.load(open(sys.argv[1]))["result"]["value"]
print(f"  price account owner    {r[3][chr(111)+chr(119)+chr(110)+chr(101)+chr(114)]}")
' "$WORK/last.json"

echo
echo "== devnet rent, read from the cluster =="
rpc getMinimumBalanceForRentExemption '[0]'; R0="$(result)"
rpc getMinimumBalanceForRentExemption '[1]'; R1="$(result)"
PER_BYTE=$((R1 - R0))
echo "min_balance(0)               $R0 lamports"
echo "min_balance(1)               $R1 lamports"
echo "lamports per data byte       $PER_BYTE"
rpc getMinimumBalanceForRentExemption '[1000000]'; RBIG="$(result)"
PRED=$((R0 + PER_BYTE * 1000000))
if [ "$RBIG" = "$PRED" ]; then
    echo "affine check at 1e6 B        EXACT ($RBIG)"
else
    echo "affine check at 1e6 B        MISMATCH observed=$RBIG predicted=$PRED" >&2
    exit 5
fi

if [ -n "$ELF_DIR" ]; then
    echo
    python3 - "$R0" "$PER_BYTE" "$ELF_DIR" <<'PY'
import os, sys
r0, per, d = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
def rent(n): return r0 + per * n
roles = sorted(f for f in os.listdir(d) if f.endswith(".so"))
print(f"{'role':<22}{'elf B':>10}{'buffer B':>11}{'pdata B':>11}"
      f"{'program SOL':>14}{'pdata SOL':>13}{'buffer SOL':>13}{'final SOL':>12}")
tot_final = tot_buf = 0
for f in roles:
    n = os.path.getsize(os.path.join(d, f))
    prog, pdata, buf = rent(36), rent(45 + n), rent(37 + n)
    tot_final += prog + pdata
    tot_buf = max(tot_buf, buf)
    print(f"{f[:-3]:<22}{n:>10}{37+n:>11}{45+n:>11}"
          f"{prog/1e9:>14.9f}{pdata/1e9:>13.9f}{buf/1e9:>13.9f}{(prog+pdata)/1e9:>12.6f}")
print()
print(f"final resident rent, all listed artifacts : {tot_final/1e9:.6f} SOL ({tot_final} lamports)")
print(f"largest single buffer (serial peak adder)  : {tot_buf/1e9:.6f} SOL")
print(f"SERIAL peak  ~ final + one buffer - that pdata: bounded above by {(tot_final+tot_buf)/1e9:.6f} SOL")
PY
fi

if [ "$CADENCE" = "true" ]; then
    echo
    echo "== SOL/USD posting cadence (bounded: one page of 1000) =="
    rpc getSignaturesForAddress "[\"$SOLUSD\",{\"limit\":1000,\"commitment\":\"finalized\"}]"
    python3 - "$WORK/last.json" <<'PY'
import json, sys, time
r = json.load(open(sys.argv[1]))["result"]
ts = sorted({s["blockTime"] for s in r if s.get("err") is None and s.get("blockTime")})
gaps = sorted(b - a for a, b in zip(ts, ts[1:]))
if gaps:
    n = len(gaps)
    print(f"  window   {ts[0]} .. {ts[-1]}  ({(ts[-1]-ts[0])/3600:.2f} h, {n} gaps)")
    print(f"  p50      {gaps[n//2]} s")
    print(f"  p90      {gaps[min(n-1, (9*n)//10)]} s")
    print(f"  p99      {gaps[min(n-1, (99*n)//100)]} s")
    print(f"  MAX      {gaps[-1]} s   <-- the bound is the max, never the median")
PY
fi

echo
echo "reads issued: $CALLS  (all read-only)"
if [ -n "$OUT" ]; then
    mkdir -p "$OUT"
    cp "$LOGFILE" "$OUT/rpc-reads.log"
    echo "read log: $OUT/rpc-reads.log"
fi
