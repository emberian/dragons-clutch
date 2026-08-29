#!/usr/bin/env bash
# Finite public-devnet activity dispatcher.  It never fabricates a Market or
# route and has no default signer.  Child callers own their signed journals.
set -euo pipefail
GENESIS=EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
RPC=""; ACK=""; STATE=""; CYCLES=""; BOOT=""; SESSION=""; MARKET=""; PUBLIC=""; EXECUTE=false; LIMIT=4; CADENCE=30
die() { echo "REFUSED: $*" >&2; exit 2; }
need() { [ "$#" -ge 2 ] || die "$1 needs a value"; }
while [ "$#" -gt 0 ]; do case "$1" in
  --rpc-url) need "$@"; RPC="$2"; shift 2 ;; --i-mean-devnet) need "$@"; ACK="$2"; shift 2 ;;
  --state-dir) need "$@"; STATE="$2"; shift 2 ;; --cycles) need "$@"; CYCLES="$2"; shift 2 ;;
  --bootstrap-bin) need "$@"; BOOT="$2"; shift 2 ;; --session) need "$@"; SESSION="$2"; shift 2 ;;
  --market) need "$@"; MARKET="$2"; shift 2 ;; --public-manifest) need "$@"; PUBLIC="$2"; shift 2 ;;
  --max-cycles) need "$@"; LIMIT="$2"; shift 2 ;; --cadence-seconds) need "$@"; CADENCE="$2"; shift 2 ;;
  --execute) EXECUTE=true; shift ;; -h|--help) echo 'usage: devnet-demo-pulse.sh --rpc-url URL --i-mean-devnet HASH --state-dir DIR --cycles TSV --bootstrap-bin BIN --session FILE --market ADDRESS --public-manifest FILE [--execute]'; exit 0 ;;
  *) die "unknown argument $1" ;; esac; done
[ "$ACK" = "$GENESIS" ] || die "--i-mean-devnet must be $GENESIS exactly"
case "$RPC" in https://*) ;; *) die "--rpc-url must be https" ;; esac
[[ "$LIMIT" =~ ^[1-9][0-9]*$ && "$LIMIT" -le 16 && "$CADENCE" =~ ^[0-9]+$ ]] || die "max cycles is 1..16 and cadence is an unsigned integer"
[ -d "$STATE" ] || mkdir -p "$STATE"; [ -r "$CYCLES" ] && [ -x "$BOOT" ] && [ -r "$SESSION" ] || die "cycles, bootstrap binary, and session must be explicit readable paths"
[ -n "$MARKET" ] && [ -n "$PUBLIC" ] || die "pass --market and --public-manifest"
[ "$(solana genesis-hash --url "$RPC")" = "$GENESIS" ] || die "RPC is not acknowledged devnet"
mkdir -p "$STATE/cycles" "$STATE/logs"; chmod 700 "$STATE"

# TSV: id owner owner-key fee-payer fee-key plan evidence min-slot admission-output direct-session [recipient payout-input]
now="$(date +%s)"; done=0; rows="$STATE/public.rows"; : > "$rows"
while IFS=$'\t' read -r id owner owner_key fee fee_key plan evidence slot admission direct recipient payout; do
  [ -z "${id:-}" ] && continue; [[ "$id" =~ ^[A-Za-z0-9_-]+$ ]] || die "cycle id is unsafe: $id"
  done=$((done+1)); [ "$done" -le "$LIMIT" ] || die "cycle file exceeds --max-cycles"
  [ -n "$owner" ] && [ -r "$owner_key" ] && [ -n "$fee" ] && [ -r "$fee_key" ] && [ -r "$plan" ] && [ -r "$evidence" ] && [ -n "$slot" ] && [ -n "$admission" ] && [ -r "$direct" ] || die "cycle $id lacks an explicit required field"
  root="$STATE/cycles/$id"; mkdir -p "$root"; admission_marker="$root/admission.done"; direct_marker="$root/direct.step"; payout_marker="$root/payout.signature"
  if [ ! -f "$admission_marker" ]; then
    args=(devnet-user-position-admission-v1 --rpc-url "$RPC" --i-mean-devnet "$GENESIS" --plan "$plan" --campaign-evidence "$evidence" --position-owner "$owner" --position-owner-keypair "$owner_key" --fee-payer "$fee" --fee-payer-keypair "$fee_key" --minimum-finalized-slot "$slot" --output "$admission")
    [ "$EXECUTE" = false ] || args+=(--execute)
    "$BOOT" "${args[@]}" | tee "$STATE/logs/$id-admission.log"
    [ "$EXECUTE" = true ] || { echo "preflight completed for $id; rerun with --execute only after authorization"; continue; }
    printf '%s\n' "$now" > "$admission_marker"
  fi
  previous="$(cat "$direct_marker" 2>/dev/null || echo 0)"; direct_at="$previous"
  if [ "$previous" != 0 ] && [ $((now-previous)) -lt "$CADENCE" ]; then die "cycle $id cadence has not elapsed"; fi
  args=(devnet-direct-trade-v1 --rpc-url "$RPC" --i-mean-devnet "$GENESIS" --session "$direct")
  [ "$EXECUTE" = false ] || args+=(--execute)
  "$BOOT" "${args[@]}" | tee "$STATE/logs/$id-direct.log"
  [ "$EXECUTE" = true ] || { echo "Direct preflight completed for $id; no key was read"; continue; }
  printf '%s\n' "$now" > "$direct_marker"; direct_at="$now"
  if [ -n "${payout:-}" ]; then
    [ -n "${recipient:-}" ] && [ -r "$payout" ] || die "cycle $id payout requires explicit recipient and payout input"
    if [ ! -f "$payout_marker" ]; then
      journal="$root/payout-journal.json"
      dclutch --rpc "$RPC" --session "$SESSION" redeem --market "$MARKET" --payer "$owner" --recipient "$recipient" --keypair "$owner_key" --i-mean-devnet "$GENESIS" --payout-input "$payout" --payout-journal "$journal" | tee "$STATE/logs/$id-payout.log"
      signature="$(sed -n 's/.*"status":"finalized".*"signature":"\([^"]*\)".*/\1/p' "$STATE/logs/$id-payout.log" | head -n 1)"
      [ -n "$signature" ] || die "cycle $id has no finalized payout; durable child journal remains $journal"
      printf '%s\n' "$signature" > "$payout_marker"
    fi
  fi
  signature="$(cat "$payout_marker" 2>/dev/null || true)"
  printf '%s\t%s\t%s\t%s\n' "$id" "$owner" "$direct_at" "$signature" >> "$rows"
done < "$CYCLES"
[ "$done" -gt 0 ] || die "cycle file is empty"
tmp="$PUBLIC.tmp"; { printf '{"schema":"dclutch-devnet-demo-pulse-public-v1","market":"%s","cycles":[' "$MARKET"; first=true; while IFS=$'\t' read -r id owner direct payout; do $first || printf ','; first=false; printf '{"id":"%s","owner":"%s","directStepAt":"%s","payoutSignature":%s}' "$id" "$owner" "$direct" "$( [ -n "$payout" ] && printf '"%s"' "$payout" || printf null )"; done < "$rows"; printf ']}\n'; } > "$tmp"; mv "$tmp" "$PUBLIC"
echo "public activity manifest: $PUBLIC"
