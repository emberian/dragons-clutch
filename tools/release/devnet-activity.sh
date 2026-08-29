#!/usr/bin/env bash
# Disposable, resumable devnet activity rail.  It has no default wallet: it
# reads only an explicit payer and keys it freshly created in --state-dir.
set -euo pipefail

DEVNET_GENESIS=EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
URL=""; ACK=""; STATE=""; PAYER=""; SESSION=""; MARKET=""; MINT=""; SOURCE=""
PAYOUT_PLAN=""; PAYOUT_EVIDENCE=""; PAYOUT_INPUT_DIR=""; BOOTSTRAP_BIN=""; REDEEM_MAX_STEPS=12
TOKEN_PROGRAM=TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
PARTICIPANTS=2; WALLET_LAMPORTS=20000000; TOKEN_ATOMS=0; TRADES=""; REPLAY=false; EXECUTE=false

usage() { printf '%s\n' \
  'usage: devnet-activity.sh --rpc-url URL --i-mean-devnet GENESIS --state-dir DIR [options]' '' \
  'Creates only campaign-local participant keys; no default wallet is read.' \
  'Funding/trading/replay require --execute and an explicit --payer-keypair.' '' \
  '  --participants N             default 2' \
  '  --wallet-lamports N          exact SOL funding target per participant (default 20000000)' \
  '  --collateral-mint ADDRESS --collateral-source ACCOUNT --token-atoms N' \
  '  --session FILE --market ADDRESS --trades TSV   rows: seller buyer route outcome fill price' \
  '  --replay --payout-input-dir DIR' \
  '                              one explicit Rust-projected wallet-N.json per participant' \
  '  --payout-plan FILE --payout-evidence FILE' \
  '                              alternative completed-campaign projection input (requires its checked ALT plan)'; }
die() { echo "REFUSED: $*" >&2; exit 2; }
need_value() { [ "$#" -ge 2 ] || die "$1 needs a value"; }
while [ "$#" -gt 0 ]; do case "$1" in
  --rpc-url) need_value "$@"; URL="$2"; shift 2 ;; --i-mean-devnet) need_value "$@"; ACK="$2"; shift 2 ;;
  --state-dir) need_value "$@"; STATE="$2"; shift 2 ;; --payer-keypair) need_value "$@"; PAYER="$2"; shift 2 ;;
  --session) need_value "$@"; SESSION="$2"; shift 2 ;; --market) need_value "$@"; MARKET="$2"; shift 2 ;;
  --payout-plan) need_value "$@"; PAYOUT_PLAN="$2"; shift 2 ;; --payout-evidence) need_value "$@"; PAYOUT_EVIDENCE="$2"; shift 2 ;;
  --payout-input-dir) need_value "$@"; PAYOUT_INPUT_DIR="$2"; shift 2 ;;
  --bootstrap-bin) need_value "$@"; BOOTSTRAP_BIN="$2"; shift 2 ;; --redeem-max-steps) need_value "$@"; REDEEM_MAX_STEPS="$2"; shift 2 ;;
  --collateral-mint) need_value "$@"; MINT="$2"; shift 2 ;; --collateral-source) need_value "$@"; SOURCE="$2"; shift 2 ;;
  --token-program) need_value "$@"; TOKEN_PROGRAM="$2"; shift 2 ;; --participants) need_value "$@"; PARTICIPANTS="$2"; shift 2 ;;
  --wallet-lamports) need_value "$@"; WALLET_LAMPORTS="$2"; shift 2 ;; --token-atoms) need_value "$@"; TOKEN_ATOMS="$2"; shift 2 ;;
  --trades) need_value "$@"; TRADES="$2"; shift 2 ;; --replay) REPLAY=true; shift ;; --execute) EXECUTE=true; shift ;;
  -h|--help) usage; exit 0 ;; *) die "unknown argument $1" ;; esac; done
[ -n "$URL" ] && [ -n "$STATE" ] || die "pass --rpc-url and --state-dir"
[ "$ACK" = "$DEVNET_GENESIS" ] || die "--i-mean-devnet must be $DEVNET_GENESIS exactly"
case "$URL" in https://*) ;; *) die "external RPC must be https" ;; esac
[[ "$PARTICIPANTS" =~ ^[2-9][0-9]*$ ]] || die "--participants must be an integer >= 2"
[[ "$WALLET_LAMPORTS" =~ ^[0-9]+$ && "$TOKEN_ATOMS" =~ ^[0-9]+$ ]] || die "amounts must be unsigned integers"
[[ "$REDEEM_MAX_STEPS" =~ ^[1-9][0-9]*$ && "$REDEEM_MAX_STEPS" -le 64 ]] || die "--redeem-max-steps must be 1..64"
GENESIS="$(solana genesis-hash --url "$URL")"
[ "$GENESIS" = "$DEVNET_GENESIS" ] || die "RPC answered genesis $GENESIS, not acknowledged devnet"

mkdir -p "$STATE/keys" "$STATE/logs" "$STATE/trades" "$STATE/tokens"; chmod 700 "$STATE" "$STATE/keys"
MANIFEST="$STATE/participants.tsv"; [ -e "$MANIFEST" ] || { : > "$MANIFEST"; chmod 600 "$MANIFEST"; }
key() { printf '%s/keys/%s.json' "$STATE" "$1"; }
address() { awk -F '\t' -v name="$1" '$1 == name {print $2}' "$MANIFEST"; }
token() { awk -F '\t' -v name="$1" '$1 == name {print $3}' "$MANIFEST"; }
require_execute() { [ "$EXECUTE" = true ] || die "$1 can create accounts or submit transactions; repeat with --execute"; }

for n in $(seq 1 "$PARTICIPANTS"); do
  name="wallet-$n"; private="$(key "$name")"; public="$(address "$name")"
  if [ -z "$public" ]; then
    require_execute "participant generation"; [ ! -e "$private" ] || die "$private exists but is absent from manifest; do not adopt an unknown key"
    solana-keygen new --silent --no-bip39-passphrase --outfile "$private"; chmod 600 "$private"
    public="$(solana-keygen pubkey "$private")"; printf '%s\t%s\t\n' "$name" "$public" >> "$MANIFEST"
  fi
done
echo "devnet activity campaign: $PARTICIPANTS participants; public manifest $MANIFEST"
[ -n "$PAYER" ] || { echo "prepared only: supply --payer-keypair plus --execute to fund; no external transaction was submitted"; exit 0; }
[ -f "$PAYER" ] || die "explicit payer keypair path does not exist"

fund() { local name="$1" public current delta; public="$(address "$name")"; current="$(solana balance --lamports --url "$URL" "$public")"
  [[ "$current" =~ ^[0-9]+$ ]] || die "could not read a lamport balance for $name"
  if [ "$current" -lt "$WALLET_LAMPORTS" ]; then delta=$((WALLET_LAMPORTS-current)); require_execute "funding $name"
    solana transfer --url "$URL" --from "$PAYER" --fee-payer "$PAYER" --allow-unfunded-recipient --lamports "$public" "$delta" | tee "$STATE/logs/fund-$name.log"; fi; }
for n in $(seq 1 "$PARTICIPANTS"); do fund "wallet-$n"; done

if [ "$TOKEN_ATOMS" -gt 0 ]; then
  [ -n "$MINT" ] && [ -n "$SOURCE" ] || die "--token-atoms requires --collateral-mint and --collateral-source"
  for n in $(seq 1 "$PARTICIPANTS"); do
    name="wallet-$n"; account="$(token "$name")"
    if [ -z "$account" ]; then require_execute "creating a token account for $name"; owner="$(address "$name")"
      account="$(spl-token --url "$URL" --program-id "$TOKEN_PROGRAM" create-account "$MINT" --owner "$owner" --fee-payer "$PAYER" | awk '/Creating account/ {print $3}')"
      [ -n "$account" ] || die "spl-token did not report a created account for $name"
      awk -F '\t' -v name="$name" -v account="$account" 'BEGIN {OFS="\t"} $1==name {$3=account} {print}' "$MANIFEST" > "$MANIFEST.tmp"; mv "$MANIFEST.tmp" "$MANIFEST"; chmod 600 "$MANIFEST"; fi
    marker="$STATE/tokens/$name.signature"
    if [ -f "$marker" ]; then solana confirm --url "$URL" "$(cat "$marker")" >/dev/null || die "recorded token funding for $name is not confirmed"; continue; fi
    require_execute "token funding for $name"; log="$STATE/logs/token-$name.log"; set +e
    spl-token --url "$URL" --program-id "$TOKEN_PROGRAM" transfer "$MINT" "$TOKEN_ATOMS" "$account" --from "$SOURCE" --owner "$PAYER" --fee-payer "$PAYER" 2>&1 | tee "$log"; status=${PIPESTATUS[0]}; set -e
    signature="$(awk '/^Signature: / {print $2; exit}' "$log")"; [ -z "$signature" ] || { printf '%s\n' "$signature" > "$marker"; chmod 600 "$marker"; }; [ "$status" -eq 0 ] || die "token funding for $name did not complete; log: $log"
  done
fi

if [ -n "$TRADES" ]; then
  [ -n "$SESSION" ] && [ -n "$MARKET" ] && [ -r "$TRADES" ] || die "--trades requires readable plan plus --session and --market"
  index=0; while IFS=$'\t' read -r seller buyer route outcome fill price; do
    [ -z "${seller:-}" ] && continue; index=$((index+1)); marker="$STATE/trades/$index.signature"
    [ -n "$buyer" ] && [ -n "$route" ] && [ -n "$outcome" ] && [ -n "$fill" ] && [ -n "$price" ] || die "trade row $index needs six tab-separated fields"
    seller_key="$(key "$seller")"; buyer_key="$(key "$buyer")"; seller_token="$(token "$seller")"; buyer_token="$(token "$buyer")"
    [ -f "$seller_key" ] && [ -f "$buyer_key" ] && [ -n "$seller_token" ] && [ -n "$buyer_token" ] || die "trade row $index names a non-campaign participant or missing collateral account"
    if [ -f "$marker" ]; then solana confirm --url "$URL" "$(cat "$marker")" >/dev/null || die "recorded trade $index is not confirmed; inspect before retrying"; continue; fi
    require_execute "trade row $index"; log="$STATE/logs/trade-$index.log"; set +e
    dclutch --rpc "$URL" --session "$SESSION" sell --route "$route" --outcome "$outcome" --fill "$fill" --price "$price" --collateral "$seller_token" --keypair "$seller_key" --counter-keypair "$buyer_key" --counter-collateral "$buyer_token" --payer "$PAYER" 2>&1 | tee "$log"; status=${PIPESTATUS[0]}; set -e
    signature="$(awk '/^submitted / {print $2; exit}' "$log")"; [ -z "$signature" ] || { printf '%s\n' "$signature" > "$marker"; chmod 600 "$marker"; }; [ "$status" -eq 0 ] || die "trade row $index did not confirm; log: $log"
  done < "$TRADES"
fi
if [ "$REPLAY" = true ]; then
  [ -n "$SESSION" ] && [ -n "$MARKET" ] || die "--replay requires --session and --market"
  if [ -n "$PAYOUT_INPUT_DIR" ]; then
    [ -z "$PAYOUT_PLAN" ] && [ -z "$PAYOUT_EVIDENCE" ] || die "choose --payout-input-dir or --payout-plan/--payout-evidence, not both"
    [ -d "$PAYOUT_INPUT_DIR" ] || die "--payout-input-dir is not a directory"
  else
    [ -n "$PAYOUT_PLAN" ] && [ -n "$PAYOUT_EVIDENCE" ] || die "--replay requires --payout-input-dir or --payout-plan plus --payout-evidence"
    [ -r "$PAYOUT_PLAN" ] && [ -r "$PAYOUT_EVIDENCE" ] || die "payout plan/evidence must be readable explicit terminal artifacts"
  fi
  [ -z "$BOOTSTRAP_BIN" ] || [ -x "$BOOTSTRAP_BIN" ] || die "--bootstrap-bin is not executable"
  mkdir -p "$STATE/redemptions"
  for n in $(seq 1 "$PARTICIPANTS"); do
    name="wallet-$n"; owner="$(address "$name")"; recipient="$(token "$name")"; private="$(key "$name")"
    [ -n "$recipient" ] || die "wallet payout for $name requires its explicit collateral recipient account"
    payout_marker="$STATE/redemptions/$name.payout.signature"
    replay_marker="$STATE/redemptions/$name.replay.signature"
    if [ -f "$payout_marker" ]; then
      solana confirm --url "$URL" "$(cat "$payout_marker")" >/dev/null || die "recorded payout for $name is not confirmed"
      continue
    fi
    require_execute "wallet terminal payout for $name"
    journal="$STATE/redemptions/$name.payout-journal.json"
    alt_plan="$STATE/redemptions/$name.lookup-table-plan.json"
    for step in $(seq 1 "$REDEEM_MAX_STEPS"); do
      log="$STATE/logs/redeem-$name-$step.log"
      command=(dclutch --rpc "$URL" --session "$SESSION" redeem --market "$MARKET" --payer "$owner" --recipient "$recipient" --keypair "$private" --i-mean-devnet "$ACK" --payout-journal "$journal")
      if [ -n "$PAYOUT_INPUT_DIR" ]; then
        input="$PAYOUT_INPUT_DIR/$name.json"; [ -r "$input" ] || die "wallet payout input is not readable: $input"
        command+=(--payout-input "$input")
      else
        command+=(--spec "$PAYOUT_PLAN" --payout-evidence "$PAYOUT_EVIDENCE" --payout-alt-plan "$alt_plan")
      fi
      [ -z "$BOOTSTRAP_BIN" ] || command+=(--bootstrap-bin "$BOOTSTRAP_BIN")
      set +e; "${command[@]}" 2>&1 | tee "$log"; status=${PIPESTATUS[0]}; set -e
      replay_signature="$(sed -n 's/.*"status":"replay-finalized".*"signature":"\([^"]*\)".*/\1/p' "$log" | head -n 1)"
      [ -z "$replay_signature" ] || { printf '%s\n' "$replay_signature" > "$replay_marker"; chmod 600 "$replay_marker"; solana confirm --url "$URL" "$replay_signature" >/dev/null || die "Claims replay for $name was not confirmed"; }
      payout_signature="$(sed -n 's/.*"status":"finalized".*"signature":"\([^"]*\)".*/\1/p' "$log" | head -n 1)"
      if [ -n "$payout_signature" ]; then
        printf '%s\n' "$payout_signature" > "$payout_marker"; chmod 600 "$payout_marker"
        solana confirm --url "$URL" "$payout_signature" >/dev/null || die "wallet payout for $name was not confirmed"
        break
      fi
      [ "$status" -eq 0 ] || die "redeem step $step for $name did not complete; the CLI journal remains at $journal and log is $log"
    done
    [ -f "$payout_marker" ] || die "redeem for $name made no finalized payout in $REDEEM_MAX_STEPS steps; terminal state was not faked and the durable journal remains at $journal"
  done
fi
echo '== reconciliation (public chain facts) =='
for n in $(seq 1 "$PARTICIPANTS"); do
  name="wallet-$n"; public="$(address "$name")"; recipient="$(token "$name")"
  echo "$name $public $(solana balance --lamports --url "$URL" "$public") lamports"
  if [ -n "$recipient" ]; then
    echo "$name collateral $recipient $(spl-token --url "$URL" --program-id "$TOKEN_PROGRAM" balance "$recipient")"
  fi
  [ -z "$SESSION" ] || dclutch --rpc "$URL" --session "$SESSION" portfolio "$public"
done
