#!/usr/bin/env bash
# Account the dClutch devnet deploy budget, and reclaim what is still
# reclaimable.
#
# THE ONE FACT THIS TOOL EXISTS TO ENFORCE
#
#   A Loader V3 program whose upgrade authority is `None` can never be closed.
#   `UpgradeableLoaderInstruction::Close` requires an authority signature, and
#   an immutable ProgramData has no authority to sign. Its rent -- for dClutch,
#   between 1.06 and 9.64 SOL per role -- is gone permanently.
#
#   dClutch REQUIRES that immutability: `CheckedInfrastructureV1::validate`
#   refuses a mutable Core/Registry/Rent, and release-set activation refuses a
#   role whose ProgramData still carries an authority. So the protocol's own
#   correctness condition is also the moment the money stops being recoverable.
#
#   Every role therefore has a recycle WINDOW that opens when its buffer is
#   created and closes the instant its authority is revoked. This script says,
#   for a given set of program ids, which windows are still open and exactly
#   how much is inside them.
#
# Default mode is PLAN: bounded read-only RPC, no signing, no writes, and it
# prints the exact commands a human would run. `--execute` is a separate, gated
# path -- see below.
#
# Usage:
#   tools/release/devnet-recycle.sh --program-ids FILE [options]
#   tools/release/devnet-recycle.sh --program-id ID [--program-id ID ...]
#
#   --program-ids FILE      newline-separated `<label> <program-id>` pairs
#   --program-id ID         a single program id (repeatable; label = the id)
#   --buffer-authority PK   also account buffers held by this authority
#   --url URL               RPC endpoint (default https://api.devnet.solana.com)
#   --recipient PK          where a close would send the lamports
#   --execute               actually close what is closeable. REQUIRES
#                           --authorization and --authority-keypair.
#   --authorization TEXT    free text naming the act the user authorized. There
#                           is no default and no way to skip it.
#   --authority-keypair P   passed straight through to `solana program close`.
#                           This script never opens it.
set -euo pipefail

URL="https://api.devnet.solana.com"
PROGRAM_IDS_FILE=""
BUFFER_AUTHORITY=""
RECIPIENT=""
EXECUTE="false"
AUTHORIZATION=""
AUTHORITY_KEYPAIR=""
SINGLE_IDS=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --program-ids) PROGRAM_IDS_FILE="${2:?--program-ids needs a value}"; shift 2 ;;
        --program-id) SINGLE_IDS+=("${2:?--program-id needs a value}"); shift 2 ;;
        --buffer-authority) BUFFER_AUTHORITY="${2:?--buffer-authority needs a value}"; shift 2 ;;
        --url) URL="${2:?--url needs a value}"; shift 2 ;;
        --recipient) RECIPIENT="${2:?--recipient needs a value}"; shift 2 ;;
        --execute) EXECUTE="true"; shift ;;
        --authorization) AUTHORIZATION="${2:?--authorization needs a value}"; shift 2 ;;
        --authority-keypair) AUTHORITY_KEYPAIR="${2:?--authority-keypair needs a value}"; shift 2 ;;
        -h|--help) sed -n '2,42p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

rpc() {
    local method="$1" params="$2"
    case "$method" in
        getGenesisHash|getAccountInfo|getMultipleAccounts|getBalance) ;;
        *) echo "REFUSED: $method is not a read method" >&2; exit 3 ;;
    esac
    curl -sS -X POST -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\",\"params\":$params}" \
        "$URL" > "$WORK/last.json"
}

# ---------------------------------------------------------------- guardrails --
rpc getGenesisHash '[]'
GENESIS="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["result"])' "$WORK/last.json")"
if [ "$GENESIS" = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d" ]; then
    echo "REFUSED: that endpoint is mainnet-beta. This tool is for the devnet demo budget." >&2
    exit 4
fi
echo "cluster genesis: $GENESIS"

if [ "$EXECUTE" = "true" ]; then
    if [ -z "$AUTHORIZATION" ]; then
        cat >&2 <<'REFUSAL'
REFUSED: --execute without --authorization.

Closing a program is irreversible and moves real lamports. This tool will not
infer authorization from the presence of a flag, from a config file, from an
environment variable, or from the fact that someone typed --execute. Pass
--authorization with text naming the act the user actually authorized, in their
words, and keep it in the run log.
REFUSAL
        exit 5
    fi
    if [ -z "$AUTHORITY_KEYPAIR" ]; then
        echo "REFUSED: --execute needs --authority-keypair" >&2
        exit 5
    fi
    echo "EXECUTE MODE. Authorization recorded as:"
    echo "  $AUTHORIZATION"
fi

# ------------------------------------------------------------------- targets --
: > "$WORK/targets"
if [ -n "$PROGRAM_IDS_FILE" ]; then
    while read -r label id; do
        [ -z "${id:-}" ] && continue
        case "$label" in \#*) continue ;; esac
        printf '%s\t%s\n' "$label" "$id" >> "$WORK/targets"
    done < "$PROGRAM_IDS_FILE"
fi
for id in ${SINGLE_IDS+"${SINGLE_IDS[@]}"}; do
    printf '%s\t%s\n' "$id" "$id" >> "$WORK/targets"
done
[ -s "$WORK/targets" ] || { echo "no program ids given" >&2; exit 2; }

# ProgramData address is find_program_address([program_id], loader) -- derived
# offline so a wrong or absent Program account cannot misdirect the close.
derive_programdata() {
    python3 - "$1" <<'PY'
import hashlib, sys
B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
def dec(s):
    n = 0
    for ch in s:
        n = n * 58 + B58.index(ch)
    b = n.to_bytes(32, "big") if n.bit_length() <= 256 else None
    pad = len(s) - len(s.lstrip("1"))
    return (b"\x00" * pad + n.to_bytes((n.bit_length() + 7) // 8, "big")).rjust(32, b"\x00")
def enc(b):
    n = int.from_bytes(b, "big"); s = ""
    while n:
        n, r = divmod(n, 58); s = B58[r] + s
    return "1" * (len(b) - len(b.lstrip(b"\x00"))) + s
loader = dec("BPFLoaderUpgradeab1e11111111111111111111111")
seed = dec(sys.argv[1])
for bump in range(255, -1, -1):
    h = hashlib.sha256(seed + bytes([bump]) + loader + b"ProgramDerivedAddress").digest()
    # Off-curve check: a PDA must not be a valid Ed25519 point.
    y = int.from_bytes(h, "little") & ((1 << 255) - 1)
    p = 2**255 - 19
    if y >= p:
        continue
    d = (-121665 * pow(121666, p - 2, p)) % p
    u = (y * y - 1) % p
    v = (d * y * y + 1) % p
    x2 = (u * pow(v, p - 2, p)) % p
    x = pow(x2, (p + 3) // 8, p)
    if (v * x * x - u) % p != 0:
        x = (x * pow(2, (p - 1) // 4, p)) % p
    on_curve = (v * x * x - u) % p == 0 and not (x == 0 and h[31] & 0x80)
    if not on_curve:
        print(enc(h)); break
PY
}

R0=890880
PER_BYTE=6960

TOTAL_OPEN=0
TOTAL_LOCKED=0
: > "$WORK/commands"

printf '\n%-14s %-46s %-12s %14s  %s\n' label programdata state SOL note
printf '%s\n' "--------------------------------------------------------------------------------------------------------"
while IFS=$'\t' read -r label id; do
    pd="$(derive_programdata "$id")"
    rpc getMultipleAccounts "[[\"$id\",\"$pd\"],{\"encoding\":\"base64\",\"commitment\":\"finalized\",\"dataSlice\":{\"offset\":0,\"length\":45}}]"
    read -r state lamports note <<<"$(python3 - "$WORK/last.json" <<'PY'
import base64, json, sys
v = json.load(open(sys.argv[1]))["result"]["value"]
prog, pdata = v[0], v[1]
if prog is None and pdata is None:
    print("absent 0 nothing-deployed-at-this-id"); raise SystemExit
total = (prog["lamports"] if prog else 0) + (pdata["lamports"] if pdata else 0)
if pdata is None:
    print(f"orphan-program {total} program-account-without-programdata"); raise SystemExit
d = base64.b64decode(pdata["data"][0])
tag = int.from_bytes(d[0:4], "little")
if tag != 3:
    print(f"not-programdata {total} tag={tag}"); raise SystemExit
if d[12] == 1:
    print(f"MUTABLE {total} closeable-by-its-upgrade-authority")
else:
    print(f"IMMUTABLE {total} rent-is-permanently-burned")
PY
)"
    sol="$(python3 -c "print(f'{$lamports/1e9:.9f}')")"
    printf '%-14s %-46s %-12s %14s  %s\n' "$label" "$pd" "$state" "$sol" "$note"
    case "$state" in
        MUTABLE)
            TOTAL_OPEN=$((TOTAL_OPEN + lamports))
            {
                printf 'solana program close %s --url %s --authority <AUTHORITY_KEYPAIR>' "$id" "$URL"
                [ -n "$RECIPIENT" ] && printf ' --recipient %s' "$RECIPIENT"
                printf ' --bypass-warning\n'
            } >> "$WORK/commands"
            ;;
        IMMUTABLE) TOTAL_LOCKED=$((TOTAL_LOCKED + lamports)) ;;
    esac
done < "$WORK/targets"

echo
printf 'still reclaimable (mutable programs) : %.9f SOL\n' "$(python3 -c "print($TOTAL_OPEN/1e9)")"
printf 'permanently burned (immutable)       : %.9f SOL\n' "$(python3 -c "print($TOTAL_LOCKED/1e9)")"

if [ -n "$BUFFER_AUTHORITY" ]; then
    echo
    echo "buffers held by $BUFFER_AUTHORITY:"
    # `solana program show --buffers` is a read. An orphan buffer is the single
    # most common way to lose SOL on a deploy that failed partway.
    solana program show --buffers --buffer-authority "$BUFFER_AUTHORITY" --url "$URL" 2>&1 | sed 's/^/  /'
    {
        printf 'solana program close --buffers --url %s --authority <AUTHORITY_KEYPAIR>' "$URL"
        [ -n "$RECIPIENT" ] && printf ' --recipient %s' "$RECIPIENT"
        printf '\n'
    } >> "$WORK/commands"
fi

echo
if [ -s "$WORK/commands" ]; then
    echo "commands that would reclaim the open windows:"
    sed 's/^/  /' "$WORK/commands"
else
    echo "nothing is reclaimable at these ids."
fi

if [ "$EXECUTE" != "true" ]; then
    echo
    echo "PLAN MODE. Nothing was signed or submitted."
    exit 0
fi

echo
echo "executing under the recorded authorization"
while read -r line; do
    cmd="${line//<AUTHORITY_KEYPAIR>/$AUTHORITY_KEYPAIR}"
    echo "+ $cmd"
    eval "$cmd"
done < "$WORK/commands"
