#!/usr/bin/env bash
# Acceptance dry run for the devnet paces driver.
#
# Rehearses the exact devnet campaign against a blank local test validator:
# both ELFs (default empty-registry and NON-PRODUCTION mock-source) are loaded
# at fresh program ids on ONE validator, with NO genesis-injected Clutch or
# provider accounts — the same blank public cluster shape devnet presents.
# Runs the `default` and `mock` profiles green, then a negative control
# (`default` expectations against the mock ELF) which must go red on the
# refusal-code mismatch, proving the driver distinguishes the deployed ELFs.
#
# This is LOCAL evidence about the DRIVER, not devnet evidence: the driver's
# own transcripts label loopback runs "loopback-or-other", never "devnet".
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

if [ "$#" -gt 1 ]; then
  echo "usage: scripts/run_devnet_paces_dryrun.sh [new-work-dir]"
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
  work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-devnet-paces.XXXXXX")"
fi
keys="$work/keys"
elves="$work/elves"
log="$work/logs"
mkdir -p "$keys" "$elves" "$log"

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
keygen="${SOLANA_KEYGEN:-$solana_home/solana-keygen}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"
test_validator="${SOLANA_TEST_VALIDATOR:-$solana_home/solana-test-validator}"
rpc_port="${CLUTCH_PACES_RPC_PORT:-18939}"
faucet_port="${CLUTCH_PACES_FAUCET_PORT:-19940}"
gossip_port="${CLUTCH_PACES_GOSSIP_PORT:-18200}"
dynamic_port_range="${CLUTCH_PACES_DYNAMIC_PORT_RANGE:-18201-18299}"
url="http://127.0.0.1:${rpc_port}"
validator_pid=""

cleanup() {
  if [ -n "$validator_pid" ] && kill -0 "$validator_pid" 2>/dev/null; then
    kill "$validator_pid" 2>/dev/null || true
    wait "$validator_pid" 2>/dev/null || true
  fi
  perl -e 'unlink for @ARGV' \
    "$keys/payer.json" "$keys/default-program.json" "$keys/mock-program.json" \
    2>/dev/null || true
  for output in out-default out-mock out-negative; do
    perl -e 'unlink for @ARGV' \
      "$work/$output/keys/actor.json" \
      "$work/$output/keys/bearer.json" \
      "$work/$output/keys/collateral-mint.json" \
      "$work/$output/keys/actor-collateral-token.json" \
      "$work/$output/keys/bearer-collateral-token.json" \
      2>/dev/null || true
  done
}
trap cleanup EXIT

if curl -s -m 1 "$url" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null \
    | grep -q '"result":"ok"'; then
  echo "FAIL: $url was already serving before the dry run"
  exit 1
fi

echo "== fresh throwaway keys =="
for name in payer default-program mock-program; do
  "$keygen" new --no-bip39-passphrase --silent --force \
    -o "$keys/$name.json" >/dev/null 2>&1
done
payer="$("$keygen" pubkey "$keys/payer.json")"
default_id="$("$keygen" pubkey "$keys/default-program.json")"
mock_id="$("$keygen" pubkey "$keys/mock-program.json")"
echo "payer=$payer"
echo "default_program_id=$default_id"
echo "mock_program_id=$mock_id"

echo
echo "== SBF ELFs (default empty-registry, NON-PRODUCTION mock-source) =="
CARGO_NET_OFFLINE=true "$build_sbf" \
  --manifest-path "$root/program/Cargo.toml" --sbf-out-dir "$elves/default" \
  >"$log/build-default.log" 2>&1 || {
    tail -40 "$log/build-default.log"
    exit 1
  }
CARGO_NET_OFFLINE=true "$build_sbf" \
  --manifest-path "$root/program/Cargo.toml" --sbf-out-dir "$elves/mock" \
  --features non-production-mock-source \
  >"$log/build-mock.log" 2>&1 || {
    tail -40 "$log/build-mock.log"
    exit 1
  }
shasum -a 256 "$elves/default/clutch_sbf.so" "$elves/mock/clutch_sbf.so"

echo
echo "== devnet-paces driver build, clippy, unit tests =="
(cd "$root/devnet-paces" \
  && cargo build --quiet \
  && cargo clippy --quiet --all-targets \
  && cargo test --quiet 2>&1 | tail -2)
paces="$root/devnet-paces/target/debug/devnet-paces"

echo
echo "== blank local validator carrying both deployed ELFs =="
"$test_validator" \
  --ledger "$work/ledger" --reset --quiet \
  --rpc-port "$rpc_port" --faucet-port "$faucet_port" --mint "$payer" \
  --gossip-port "$gossip_port" --dynamic-port-range "$dynamic_port_range" \
  --bpf-program "$default_id" "$elves/default/clutch_sbf.so" \
  --bpf-program "$mock_id" "$elves/mock/clutch_sbf.so" \
  >"$log/validator.log" 2>&1 &
validator_pid=$!
ready=0
for _ in $(seq 1 80); do
  kill -0 "$validator_pid" 2>/dev/null || break
  ok=1
  for program in "$default_id" "$mock_id"; do
    curl -s -m 2 "$url" -H 'Content-Type: application/json' \
      -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$program\",{\"encoding\":\"base64\",\"dataSlice\":{\"offset\":0,\"length\":0}}]}" \
      2>/dev/null \
      | python3 -c 'import json,sys; v=json.load(sys.stdin).get("result",{}).get("value"); raise SystemExit(not (v and v.get("executable") is True))' \
      2>/dev/null || ok=0
  done
  slot="$(curl -s -m 2 "$url" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}' 2>/dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("result",0))' \
    2>/dev/null || echo 0)"
  if [ "$ok" -eq 1 ] && [ "${slot:-0}" -ge 1 ] 2>/dev/null; then
    ready=1
    break
  fi
  sleep 0.25
done
if [ "$ready" -ne 1 ]; then
  echo "FAIL: local validator never exposed both executable programs"
  tail -40 "$log/validator.log"
  exit 1
fi

echo
echo "== profile default: fail-closed campaign against the default ELF =="
"$paces" --url "$url" --program-id "$default_id" --payer "$keys/payer.json" \
  --profile default --out "$work/out-default" --throttle-ms 50 \
  | tee "$log/paces-default.log"

echo
echo "== profile mock: boundary campaign against the mock ELF =="
"$paces" --url "$url" --program-id "$mock_id" --payer "$keys/payer.json" \
  --profile mock --out "$work/out-mock" --throttle-ms 50 \
  | tee "$log/paces-mock.log"

echo
echo "== negative control: default expectations against the mock ELF must go red =="
if "$paces" --url "$url" --program-id "$mock_id" --payer "$keys/payer.json" \
    --profile default --out "$work/out-negative" --throttle-ms 50 \
    >"$log/paces-negative.log" 2>&1; then
  echo "FAIL: the profile/ELF mismatch still passed"
  exit 1
fi
if ! grep -q 'expected Custom(0x0079)' "$log/paces-negative.log" \
    || ! grep -q '0x007a' "$log/paces-negative.log"; then
  echo "FAIL: negative control went red for an unrelated reason"
  tail -20 "$log/paces-negative.log"
  exit 1
fi
grep -m1 'expected Custom(0x0079)' "$log/paces-negative.log" | sed 's/^/  red: /'

echo
for profile in default mock; do
  python3 - "$work/out-$profile/transcript.json" <<'PY'
import json, sys
t = json.load(open(sys.argv[1]))
steps = t["steps"]
refusals = [s for s in steps if s["kind"] == "refuse"]
print(f"profile={t['profile']} network={t['network']} outcome={t['outcome']} "
      f"steps={len(steps)} refusals={[hex(s['expect_code']) for s in refusals]} "
      f"boundaries={len(t['boundaries'])}")
PY
done
echo "negative_control=RED_AS_REQUIRED"
echo "work_dir=$work"
