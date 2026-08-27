#!/usr/bin/env bash
# Signed, committing, same-market local SBF evidence against the explicitly
# different non-production mock-source ELF.
#
# This script refuses non-loopback operation by construction. It creates fresh
# test-only keys for the payer, actor, bearer, and their ordinary Token-2022
# accounts, passes only their public keys to the plan generator,
# builds and loads the current SBF ELF into a local validator, submits every
# plan transaction with real Ed25519 signatures, confirms it, and reloads exact
# account bytes.  It then corrupts one terminal expectation, starts a fresh
# local ledger, and requires the same walk to go red.
#
# The plan is explicitly genesis-assisted and its successful Endow depends on
# the one deterministic laboratory release compiled by
# `non-production-mock-source`. The default production-inert ELF is exercised by
# separate fail-closed gates and must never be credited with this success. Its
# terminal WithdrawCash steps
# drain both owners' free cash and the pooled Hoard, but it still does not prove
# authenticated source ingestion or an end-to-end venue lifecycle.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
repo="$(cd "$root/../.." && pwd)"

if [ "$#" -gt 1 ]; then
  echo "usage: scripts/run_committed.sh [new-work-dir]"
  exit 2
fi
if [ "$#" -eq 1 ]; then
  work="$1"
  if [ -e "$work" ]; then
    echo "FAIL: refusing to replace existing work directory: $work"
    exit 1
  fi
  mkdir -p "$work"
else
  work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-sbf-committed.XXXXXX")"
fi

keys="$(mktemp -d "${TMPDIR:-/tmp}/clutch-sbf-keys.XXXXXX")"
plan="$work/plan"
out="$work/out"
log="$work/logs"
mkdir -p "$plan" "$out" "$log"

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
solana_bin="${SOLANA_BIN:-$solana_home/solana}"
keygen="${SOLANA_KEYGEN:-$solana_home/solana-keygen}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"
loopback_tools="$repo/tools/agave-loopback-validator"
loopback_cache="${CLUTCH_AGAVE_LOOPBACK_CACHE:-$repo/.cache/agave-loopback-validator}"
test_validator="${CLUTCH_LOOPBACK_TEST_VALIDATOR:-${SOLANA_TEST_VALIDATOR:-$loopback_cache/bin/solana-test-validator}}"
listener_probe="$loopback_tools/probe-listeners.sh"
rpc_port="${CLUTCH_COMMITTED_RPC_PORT:-18929}"
faucet_port="${CLUTCH_COMMITTED_FAUCET_PORT:-19930}"
gossip_port="${CLUTCH_COMMITTED_GOSSIP_PORT:-18100}"
dynamic_port_range="${CLUTCH_COMMITTED_DYNAMIC_PORT_RANGE:-18101-18199}"
url="http://127.0.0.1:${rpc_port}"
validator_pid=""
victim=""
python3 "$loopback_tools/verify-runtime.py" --binary "$test_validator" \
  | tee "$log/validator-runtime.txt"

stop_validator() {
  if [ -n "$validator_pid" ] && kill -0 "$validator_pid" 2>/dev/null; then
    kill "$validator_pid" 2>/dev/null || true
    wait "$validator_pid" 2>/dev/null || true
  fi
  validator_pid=""
}

cleanup() {
  stop_validator
  if [ -n "$victim" ] && [ -f "$victim.orig" ]; then
    mv "$victim.orig" "$victim"
  fi
  # The only secrets this gate creates are these exact files in the
  # mktemp-owned directory.  Unlink them rather than moving them to Trash.
  perl -e 'unlink for @ARGV' \
    "$keys/payer.json" \
    "$keys/actor.json" \
    "$keys/actor-outcome-token.json" \
    "$keys/payer-collateral-token.json" \
    "$keys/holder.json" \
    "$keys/holder-outcome-token.json" \
    "$keys/holder-collateral-token.json" 2>/dev/null || true
  rmdir "$keys" 2>/dev/null || true
}
trap cleanup EXIT

if curl -s -m 1 "$url" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null \
    | grep -q '"result":"ok"'; then
  echo "FAIL: $url was already serving before the committed gate"
  exit 1
fi

"$keygen" new --no-bip39-passphrase --silent --force \
  -o "$keys/payer.json" >/dev/null 2>&1
"$keygen" new --no-bip39-passphrase --silent --force \
  -o "$keys/actor.json" >/dev/null 2>&1
"$keygen" new --no-bip39-passphrase --silent --force \
  -o "$keys/actor-outcome-token.json" >/dev/null 2>&1
"$keygen" new --no-bip39-passphrase --silent --force \
  -o "$keys/payer-collateral-token.json" >/dev/null 2>&1
"$keygen" new --no-bip39-passphrase --silent --force \
  -o "$keys/holder.json" >/dev/null 2>&1
"$keygen" new --no-bip39-passphrase --silent --force \
  -o "$keys/holder-outcome-token.json" >/dev/null 2>&1
"$keygen" new --no-bip39-passphrase --silent --force \
  -o "$keys/holder-collateral-token.json" >/dev/null 2>&1
payer="$("$keygen" pubkey "$keys/payer.json")"
actor="$("$keygen" pubkey "$keys/actor.json")"
actor_outcome_token="$("$keygen" pubkey "$keys/actor-outcome-token.json")"
payer_collateral_token="$("$keygen" pubkey "$keys/payer-collateral-token.json")"
holder="$("$keygen" pubkey "$keys/holder.json")"
holder_outcome_token="$("$keygen" pubkey "$keys/holder-outcome-token.json")"
holder_collateral_token="$("$keygen" pubkey "$keys/holder-collateral-token.json")"

echo "== toolchain =="
"$solana_bin" --version
"$build_sbf" --version
echo
echo "== source =="
(cd "$repo" && git rev-parse HEAD)
(cd "$repo" && git status --porcelain -- programs/clutch-sbf programs/solana-layout programs/solana-reference crates) \
  | sed 's/^/  /'

echo
echo "== NON-PRODUCTION mock-source SBF ELF =="
echo "source_profile=NON-PRODUCTION-non-production-mock-source"
CARGO_NET_OFFLINE=true "$build_sbf" \
  --manifest-path "$root/program/Cargo.toml" --sbf-out-dir "$out" \
  --features non-production-mock-source \
  >"$log/build.log" 2>&1 || {
    tail -60 "$log/build.log"
    exit 1
  }
elf="$out/clutch_sbf.so"
elf_sha256="$(shasum -a 256 "$elf" | awk '{print $1}')"
echo "sbf_elf_sha256=$elf_sha256"

echo
echo "== same-address committed plan =="
CLUTCH_COMMITTED_PAYER="$payer" \
CLUTCH_COMMITTED_ACTOR="$actor" \
CLUTCH_COMMITTED_ACTOR_OUTCOME_TOKEN="$actor_outcome_token" \
CLUTCH_COMMITTED_PAYER_COLLATERAL_TOKEN="$payer_collateral_token" \
CLUTCH_COMMITTED_HOLDER="$holder" \
CLUTCH_COMMITTED_HOLDER_OUTCOME_TOKEN="$holder_outcome_token" \
CLUTCH_COMMITTED_HOLDER_COLLATERAL_TOKEN="$holder_collateral_token" \
  cargo run --offline -q --manifest-path "$root/Cargo.toml" \
    -p clutch-sbf-harness -- "$plan" --committed \
    | tee "$log/plan.log"
program_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["program_id"])' "$plan/committed.json")"
read -r committed_steps committed_refusals committed_exhausted committed_watched committed_precreated <<EOF
$(python3 - "$plan/committed.json" <<'PY'
import json, sys
plan = json.load(open(sys.argv[1]))
steps = plan["steps"]
watched = {entry["address"] for step in steps for entry in step.get("compare", [])}
print(len(steps), sum(step["kind"] == "refuse" for step in steps), sum(step["kind"] == "exhausted" for step in steps), len(watched), len(plan["precreated_program_accounts"]))
PY
)
EOF

# Runtime provenance and listener isolation are mandatory around every walk.
validator_args=(
  --ledger "$work/ledger" --reset --quiet
  --bind-address 127.0.0.1
  --rpc-port "$rpc_port" --faucet-port "$faucet_port" --mint "$payer"
  --gossip-port "$gossip_port" --dynamic-port-range "$dynamic_port_range"
  --bpf-program "$program_id" "$elf"
)
while read -r role address file; do
  [ -z "$role" ] && continue
  validator_args+=(--account "$address" "$plan/$file")
done < "$plan/genesis.txt"

start_validator() {
  local label="$1"
  "$test_validator" "${validator_args[@]}" >"$log/validator-$label.log" 2>&1 &
  validator_pid=$!
  local ready=0
  for _ in $(seq 1 80); do
    if ! kill -0 "$validator_pid" 2>/dev/null; then
      break
    fi
    local account slot
    account="$(curl -s -m 2 "$url" -H 'Content-Type: application/json' \
      -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$program_id\",{\"encoding\":\"base64\"}]}" \
      2>/dev/null || true)"
    slot="$(curl -s -m 2 "$url" -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}' 2>/dev/null \
      | python3 -c 'import json,sys; print(json.load(sys.stdin).get("result",0))' \
      2>/dev/null || true)"
    if [ "${slot:-0}" -ge 1 ] 2>/dev/null \
        && printf '%s' "$account" \
          | python3 -c 'import json,sys; v=json.load(sys.stdin).get("result",{}).get("value"); raise SystemExit(not (v and v.get("executable") is True))' \
          2>/dev/null; then
      ready=1
      break
    fi
    sleep 0.25
  done
  if [ "$ready" -ne 1 ]; then
    echo "FAIL: local validator never exposed the executable program after slot zero"
    tail -60 "$log/validator-$label.log"
    exit 1
  fi
  "$listener_probe" "$validator_pid" "$rpc_port" "$faucet_port" "$test_validator" \
    | tee "$log/listeners-$label-before.txt"
}

probe_after() {
  local label="$1"
  "$listener_probe" "$validator_pid" "$rpc_port" "$faucet_port" "$test_validator" \
    | tee "$log/listeners-$label-after.txt"
}

run_signed() {
  cargo run --offline --quiet \
    --manifest-path "$root/committed-harness/Cargo.toml" -- \
    "$url" "$plan" \
    "$keys/payer.json" \
    "$keys/actor.json" \
    "$keys/actor-outcome-token.json" \
    "$keys/payer-collateral-token.json" \
    "$keys/holder.json" \
    "$keys/holder-outcome-token.json" \
    "$keys/holder-collateral-token.json"
}

echo
echo "== signed, confirmed, committed walk =="
start_validator committed
run_signed | tee "$log/committed.log"
probe_after committed
stop_validator

echo
echo "== falsifiability: corrupt one terminal byte and require red =="
victim="$plan/expected/committed-22-withdraw-second-owner-cash.committed-market.hoard-token.hex"
cp "$victim" "$victim.orig"
python3 - "$victim" <<'PY'
import sys
path = sys.argv[1]
text = open(path).read().strip()
byte = (int(text[:2], 16) + 1) % 256
open(path, "w").write(f"{byte:02x}" + text[2:] + "\n")
PY
start_validator falsify
if run_signed >"$log/falsify.log" 2>&1; then
  echo "FAIL: corrupted terminal expectation still passed"
  exit 1
fi
probe_after falsify
if ! grep -q 'committed bytes differ' "$log/falsify.log"; then
  echo "FAIL: negative run failed for an unrelated reason"
  tail -60 "$log/falsify.log"
  exit 1
fi
grep -m1 'committed bytes differ' "$log/falsify.log" | sed 's/^/  red: /'
stop_validator
mv "$victim.orig" "$victim"
victim=""

echo
echo "committed_signed_transactions=$committed_steps"
echo "committed_expected_refusals=$committed_refusals"
echo "committed_compute_exhaustions=$committed_exhausted"
echo "committed_watched_accounts=$committed_watched"
echo "genesis_assisted_program_accounts=$committed_precreated"
echo "withdraw_cash=DRIVEN_TO_ZERO"
echo "redeem_external=DRIVEN"
echo "falsifiability=PASS"
echo "sbf_elf_sha256=$elf_sha256"
echo "work_dir=$work"
