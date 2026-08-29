#!/usr/bin/env bash
# A deliberately small, disposable devnet activity rail.
#
# It has no default wallet and never searches a Solana config directory.  The
# only private files it reads are the fresh participant keys it created below,
# inside --state-dir.  A rerun derives progress from balances and recorded,
# finalized transaction signatures; it never blindly replays a trade.
set -euo pipefail

DEVNET_GENESIS=EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
URL=""
ACK=""
STATE=""
PAYER=""
SESSION=""
MARKET=""
MINT=""
COLLATERAL_SOURCE=""
TOKEN_PROGRAM=TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
PARTICIPANTS=2
WALLET_LAMPORTS=20000000
TOKEN_ATOMS=0
TRADES=""
REPLAY=false
EXECUTE=false

usage() {
  printf '%s\n' \
    'usage: devnet-activity.sh --rpc-url URL --i-mean-devnet GENESIS --state-dir DIR [options]' \
    '' \
    'Creates only campaign-local participant keys; no default wallet is read.' \
    'Funding/trading/replay require --execute and an explicit --payer-keypair.' \
    '' \
    '  --participants N             default 2' \
    '  --wallet-lamports N          exact SOL funding target per participant (default 20000000)' \
    '  --collateral-mint ADDRESS --collateral-source ACCOUNT --token-atoms N' \
    '                              create and fund one campaign token account per participant' \
    '  --session FILE --market ADDRESS --trades TSV' \
    '                              trade rows: seller buyer route outcome fill price (tab-separated)' \
    '  --replay                     run the terminal-only Claims replay step for every participant'
}

die() { echo "REFUSED: $*" >&2; exit 2; }
need_value() { [ "$#" -ge 2 ] || die "$1 needs a value"; }

while [ "$#" -gt 0 ]; do
  case "$1" in
    --rpc-url) need_value "$@"; URL="$2"; shift 2 ;;
    --i-mean-devnet) need_value "$@"; ACK="$2"; shift 2 ;;
    --state-dir) need_value "$@"; STATE="$2"; shift 2 ;;
    --payer-keypair) need_value "$@"; PAYER="$2"; shift 2 ;;
    --session) need_value "$@"; SESSION="$2"; shift 2 ;;
    --market) need_value "$@"; MARKET="$2"; shift 2 ;;
    --collateral-mint) need_value "$@"; MINT="$2"; shift 2 ;;
    --collateral-source) need_value "$@"; COLLATERAL_SOURCE="$2"; shift 2 ;;
    --token-program) need_value "$@"; TOKEN_PROGRAM="$2"; shift 2 ;;
    --participants) need_value "$@"; PARTICIPANTS="$2"; shift 2 ;;
    --wallet-lamports) need_value "$@"; WALLET_LAMPORTS="$2"; shift 2 ;;
    --token-atoms) need_value "$@"; TOKEN_ATOMS="$2"; shift 2 ;;
    --trades) need_value "$@"; TRADES="$2"; shift 2 ;;
    --replay) REPLAY=true; shift ;;
    --execute) EXECUTE=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument $1" ;;
  esac
done

[ -n "$URL" ] || die "pass --rpc-url"
[ -n "$STATE" ] || die "pass --state-dir (a new, dedicated directory)"
[ "$ACK" = "$DEVNET_GENESIS" ] || die "--i-mean-devnet must be $DEVNET_GENESIS exactly"
case "$URL" in https://*) ;; *) die "external RPC must be https" ;; esac
[[ "$PARTICIPANTS" =~ ^[2-9][0-9]*$ ]] || die "--participants must be an integer >= 2"
[[ "$WALLET_LAMPORTS" =~ ^[0-9]+$ ]] || die "--wallet-lamports must be an unsigned integer"
[[ "$TOKEN_ATOMS" =~ ^[0-9]+$ ]] || die "--token-atoms must be an unsigned integer"

# This is a bounded public read.  Refuse an endpoint that is not devnet before
# handling a payer or generating a participant key.
GENESIS="$(solana genesis-hash --url "$URL")"
[ "$GENESIS" = "$DEVNET_GENESIS" ] || die "RPC answered genesis $GENESIS, not acknowledged devnet"

mkdir -p "$STATE/keys" "$STATE/logs" "$STATE/trades"
chmod 700 "$STATE" "$STATE/keys"
MANIFEST="$STATE/participants.tsv"
if [ ! -e "$MANIFEST" ]; then
  : > "$MANIFEST"
  chmod 600 "$MANIFEST"
fi

participant_key() { printf '%s/keys/%s.json' "$STATE" "$1"; }
participant_address() { awk -F '\t' -v name="$1" '$1 == name { print $2 }' "$MANIFEST"; }
participant_token() { awk -F '\t' -v name="$1" '$1 == name { print $3 }' "$MANIFEST"; }
require_execute() { [ "$EXECUTE" = true ] || die "$1 can create accounts or submit transactions; repeat with --execute"; }

for number in $(seq 1 "$PARTICIPANTS"); do
  name="wallet-$number"
  key="$(participant_key "$name")"
  address="$(participant_address "$name")"
  if [ -z "$address" ]; then
    require_execute "participant generation"
    [ ! -e "$key" ] || die "$key exists but is absent from the campaign manifest; do not adopt an unknown key"
    solana-keygen new --silent --no-bip39-passphrase --outfile "$key"
    chmod 600 "$key"
    address="$(solana-keygen pubkey "$key")"
    printf '%s\t%s\t\n' "$name" "$address" >> "$MANIFEST"
  fi
done

echo "devnet activity campaign"
echo "  state directory: $STATE"
echo "  participants:    $PARTICIPANTS (public addresses in $MANIFEST)"
echo "  wallet budget:   $WALLET_LAMPORTS lamports each"
echo "  token budget:    $TOKEN_ATOMS atoms each"

if [ -z "$PAYER" ]; then
  echo "prepared only: supply --payer-keypair plus --execute to fund; no external transaction was submitted"
  exit 0
fi
[ -f "$PAYER" ] || die "explicit payer keypair path does not exist"

fund_wallet() {
  local name="$1" address current delta
  address="$(participant_address "$name")"
  current="$(solana balance --lamports --url "$URL" "$address")"
  [[ "$current" =~ ^[0-9]+$ ]] || die "could not read a lamport balance for $name"
  if [ "$current" -lt "$WALLET_LAMPORTS" ]; then
    delta=$((WALLET_LAMPORTS - current))
    require_execute "funding $name"
    solana transfer --url "$URL" --from "$PAYER" --fee-payer "$PAYER" --allow-unfunded-recipient --lamports "$address" "$delta" | tee "$STATE/logs/fund-$name.log"
  fi
}

for number in $(seq 1 "$PARTICIPANTS"); do fund_wallet "wallet-$number"; done

# Token funding is optional because some routes use a pre-existing collateral
# account.  Its source is an explicit input: collateral principal is never
# guessed from the payer's SOL balance or from a default token account.
if [ "$TOKEN_ATOMS" -gt 0 ]; then
  [ -n "$MINT" ] || die "--token-atoms requires --collateral-mint"
  [ -n "$COLLATERAL_SOURCE" ] || die "--token-atoms requires --collateral-source (the explicit source account)"
  for number in $(seq 1 "$PARTICIPANTS"); do
    name="wallet-$number"; token="$(participant_token "$name")"
    if [ -z "$token" ]; then
      require_execute "creating a token account for $name"
      owner="$(participant_address "$name")"
      token="$(spl-token --url "$URL" --program-id "$TOKEN_PROGRAM" create-account "$MINT" --owner "$owner" --fee-payer "$PAYER" | awk '/Creating account/ {print $3}')"
      [ -n "$token" ] || die "spl-token did not report a created account for $name"
      awk -F '\t' -v name="$name" -v token="$token" 'BEGIN {OFS="\t"} $1 == name {$3=token} {print}' "$MANIFEST" > "$MANIFEST.tmp"
      mv "$MANIFEST.tmp" "$MANIFEST"
      chmod 600 "$MANIFEST"
    fi
    marker="$STATE/tokens/$name.signature"
    mkdir -p "$STATE/tokens"
    if [ -f "$marker" ]; then
      solana confirm --url "$URL" "$(cat "$marker")" >/dev/null || die "recorded token funding for $name is not confirmed; inspect it before retrying"
      continue
    fi
    log="$STATE/logs/token-$name.log"
    set +e
    spl-token --url "$URL" --program-id "$TOKEN_PROGRAM" transfer "$MINT" "$TOKEN_ATOMS" "$token" --from "$COLLATERAL_SOURCE" --owner "$PAYER" --fee-payer "$PAYER" 2>&1 | tee "$log"
    status=${PIPESTATUS[0]}
    set -e
    signature="$(awk '/^Signature: / {print $2; exit}' "$log")"
    if [ -n "$signature" ]; then printf '%s\n' "$signature" > "$marker"; chmod 600 "$marker"; fi
    [ "$status" -eq 0 ] || die "token funding for $name did not complete; its log is $log"
  done
fi

# Each trade row is: seller<TAB>buyer<TAB>route.json<TAB>outcome<TAB>fill<TAB>price.
# A route is an authenticated, market-specific frame; it cannot be fabricated
# from a mint or from this campaign's wallet names.  The normal Direct trade
# admits the participants' canonical Position/admission records as part of its
# own checked transition.  A marker is written only after the CLI reports a
# submitted signature; a rerun confirms that exact signature before proceeding.
if [ -n "$TRADES" ]; then
  [ -n "$SESSION" ] && [ -n "$MARKET" ] || die "--trades requires --session and --market"
  [ -r "$TRADES" ] || die "trade plan is not readable"
  index=0
  while IFS=$'\t' read -r seller buyer route outcome fill price; do
    [ -z "${seller:-}" ] && continue
    index=$((index + 1)); marker="$STATE/trades/$index.signature"
    [ -n "$buyer" ] && [ -n "$route" ] && [ -n "$outcome" ] && [ -n "$fill" ] && [ -n "$price" ] || die "trade row $index needs six tab-separated fields"
    seller_key="$(participant_key "$seller")"; buyer_key="$(participant_key "$buyer")"
    seller_token="$(participant_token "$seller")"; buyer_token="$(participant_token "$buyer")"
    [ -f "$seller_key" ] && [ -f "$buyer_key" ] || die "trade row $index names a non-campaign participant"
    [ -n "$seller_token" ] && [ -n "$buyer_token" ] || die "trade row $index needs token account addresses in the campaign manifest"
    if [ -f "$marker" ]; then
      signature="$(cat "$marker")"
      echo "checking recorded trade $index: $signature"
      solana confirm --url "$URL" "$signature" >/dev/null || die "recorded trade $index is not confirmed; inspect it before retrying"
      continue
    fi
    require_execute "trade row $index"
    log="$STATE/logs/trade-$index.log"
    set +e
    dclutch --rpc "$URL" --session "$SESSION" sell --route "$route" --outcome "$outcome" --fill "$fill" --price "$price" --collateral "$seller_token" --keypair "$seller_key" --counter-keypair "$buyer_key" --counter-collateral "$buyer_token" --payer "$PAYER" 2>&1 | tee "$log"
    status=${PIPESTATUS[0]}
    set -e
    signature="$(awk '/^submitted / {print $2; exit}' "$log")"
    if [ -n "$signature" ]; then printf '%s\n' "$signature" > "$marker"; chmod 600 "$marker"; fi
    [ "$status" -eq 0 ] || die "trade row $index did not report a successful confirmation; its log is $log"
  done < "$TRADES"
fi

if [ "$REPLAY" = true ]; then
  [ -n "$SESSION" ] && [ -n "$MARKET" ] || die "--replay requires --session and --market"
  for number in $(seq 1 "$PARTICIPANTS"); do
    name="wallet-$number"; key="$(participant_key "$name")"
    require_execute "Claims replay for $name"
    dclutch --rpc "$URL" --session "$SESSION" redeem --market "$MARKET" --keypair "$key"
  done
fi

echo "== reconciliation (public chain facts) =="
for number in $(seq 1 "$PARTICIPANTS"); do
  name="wallet-$number"; address="$(participant_address "$name")"
  echo "$name $address $(solana balance --lamports --url "$URL" "$address") lamports"
  [ -n "$SESSION" ] && dclutch --rpc "$URL" --session "$SESSION" portfolio "$address"
done
