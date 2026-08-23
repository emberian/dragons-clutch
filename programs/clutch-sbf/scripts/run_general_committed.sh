#!/usr/bin/env bash
# Signed, committing, same-market local SBF evidence for the Tier-2 GENERAL
# CLEARING plane (intents 47-59) against the explicitly different
# non-production mock-source ELF.
#
# This script refuses non-loopback operation by construction.  It creates
# fresh test-only keys for the payer and the four distinct signing owners
# (founding actor plus three traders) and their ordinary Token-2022 account
# identities, passes only their public keys to the plan generator, builds and
# loads the current SBF ELF into a local validator, submits every plan
# transaction with real Ed25519 signatures, confirms each one, reloads exact
# account bytes at every step, waits on the validator's real clock for the
# epoch freeze deadline and the candidate-window deadline, and re-derives the
# terminal conservation identities from the runner's OBSERVED reloaded bytes.
# It then corrupts one terminal expectation, starts a fresh local ledger, and
# requires the same walk to go red.
#
# The plan is explicitly genesis-assisted (frozen Realm artifacts and the
# mock SourceSpec) and its funded reservations depend on the one deterministic
# laboratory release compiled by `non-production-mock-source`.  The default
# empty-registry ELF refuses Endow with 0x0079 and must never be credited
# with this success.  This walk proves the general clearing plane end to end
# on a committing bank: policy artifact stage/seal, InitEpoch + window, funded
# single and portfolio placements from distinct signing owners, a cancel
# tombstone, the deadline freeze, staged checkpoint + feed creation, the
# candidate submission wire, the reservation-sweeping walk, selection,
# entitlement, and consumption of both an entitled single slice and the
# portfolio full pair.  It does not prove authenticated source ingestion or a
# production deployment.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
repo="$(cd "$root/../.." && pwd)"

if [ "$#" -gt 1 ]; then
  echo "usage: scripts/run_general_committed.sh [new-work-dir]"
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
  work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-sbf-general.XXXXXX")"
fi

keys="$(mktemp -d "${TMPDIR:-/tmp}/clutch-sbf-general-keys.XXXXXX")"
plan="$work/plan"
out="$work/out"
log="$work/logs"
observed="$work/observed"
mkdir -p "$plan" "$out" "$log" "$observed"

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
solana_bin="${SOLANA_BIN:-$solana_home/solana}"
keygen="${SOLANA_KEYGEN:-$solana_home/solana-keygen}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"
test_validator="${SOLANA_TEST_VALIDATOR:-$solana_home/solana-test-validator}"
rpc_port="${CLUTCH_GENERAL_RPC_PORT:-18949}"
faucet_port="${CLUTCH_GENERAL_FAUCET_PORT:-19950}"
# Agave reserves rpc_port + 1 (18950) for RPC WebSocket.
gossip_port="${CLUTCH_GENERAL_GOSSIP_PORT:-18951}"
dynamic_port_range="${CLUTCH_GENERAL_DYNAMIC_PORT_RANGE:-18952-19051}"
url="http://127.0.0.1:${rpc_port}"
validator_pid=""
victim=""

key_names=(
  payer actor owner-b owner-c owner-d
  owner-b-collateral-token owner-c-collateral-token owner-d-collateral-token
)

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
  local files=()
  local name
  for name in "${key_names[@]}"; do
    files+=("$keys/$name.json")
  done
  perl -e 'unlink for @ARGV' "${files[@]}" 2>/dev/null || true
  rmdir "$keys" 2>/dev/null || true
}
trap cleanup EXIT

if curl -s -m 1 "$url" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null \
    | grep -q '"result":"ok"'; then
  echo "FAIL: $url was already serving before the committed gate"
  exit 1
fi

declare -A pub
for name in "${key_names[@]}"; do
  "$keygen" new --no-bip39-passphrase --silent --force \
    -o "$keys/$name.json" >/dev/null 2>&1
  pub[$name]="$("$keygen" pubkey "$keys/$name.json")"
done

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
elf_bytes="$(wc -c < "$elf" | tr -d ' ')"
echo "sbf_elf_sha256=$elf_sha256"
echo "sbf_elf_bytes=$elf_bytes"

echo
echo "== general-clearing committed plan =="
CLUTCH_COMMITTED_PAYER="${pub[payer]}" \
CLUTCH_COMMITTED_ACTOR="${pub[actor]}" \
CLUTCH_COMMITTED_HOLDER="${pub[owner-b]}" \
CLUTCH_COMMITTED_HOLDER_COLLATERAL_TOKEN="${pub[owner-b-collateral-token]}" \
CLUTCH_COMMITTED_TRADER_C="${pub[owner-c]}" \
CLUTCH_COMMITTED_TRADER_D="${pub[owner-d]}" \
CLUTCH_COMMITTED_TRADER_C_COLLATERAL_TOKEN="${pub[owner-c-collateral-token]}" \
CLUTCH_COMMITTED_TRADER_D_COLLATERAL_TOKEN="${pub[owner-d-collateral-token]}" \
  cargo run --offline -q --manifest-path "$root/Cargo.toml" \
    -p clutch-sbf-harness -- "$plan" --general-clearing \
    | tee "$log/plan.log"
program_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["program_id"])' "$plan/committed.json")"
read -r committed_steps committed_refusals committed_watched committed_precreated <<EOF
$(python3 - "$plan/committed.json" <<'PY'
import json, sys
plan = json.load(open(sys.argv[1]))
steps = plan["steps"]
watched = {entry["address"] for step in steps for entry in step.get("compare", [])}
print(len(steps), sum(step["kind"] == "refuse" for step in steps), len(watched), len(plan["precreated_program_accounts"]))
PY
)
EOF

# Stock Agave applies --bind-address to gossip/node sockets, not RPC/faucet.
validator_args=(
  --ledger "$work/ledger" --reset --quiet
  --bind-address 127.0.0.1
  --rpc-port "$rpc_port" --faucet-port "$faucet_port" --mint "${pub[payer]}"
  --gossip-port "$gossip_port" --dynamic-port-range "$dynamic_port_range"
  --bpf-program "$program_id" "$elf"
)
while read -r role address file; do
  [ -z "$role" ] && continue
  validator_args+=(--account "$address" "$plan/$file")
done < "$plan/genesis.txt"

start_validator() {
  "$test_validator" "${validator_args[@]}" >"$log/validator.log" 2>&1 &
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
    tail -60 "$log/validator.log"
    exit 1
  fi
}

run_signed() {
  local dump_dir="$1"
  local files=()
  local name
  for name in "${key_names[@]}"; do
    files+=("$keys/$name.json")
  done
  CLUTCH_COMMITTED_OBSERVED_DIR="$dump_dir" \
    cargo run --offline --quiet \
      --manifest-path "$root/committed-harness/Cargo.toml" -- \
      "$url" "$plan" "${files[@]}"
}

echo
echo "== signed, confirmed, committed general-clearing walk =="
walk_started=$SECONDS
start_validator
run_signed "$observed" | tee "$log/committed.log"
stop_validator
walk_seconds=$((SECONDS - walk_started))

echo
echo "== conservation, re-derived from the runner's OBSERVED reloaded bytes =="
python3 - "$plan/committed.json" "$observed" <<'PY' | tee "$log/conservation.log"
import json, sys
plan = json.load(open(sys.argv[1]))
observed_dir = sys.argv[2]
rules = plan["conservation"]
offsets = rules["offsets"]

def load(step, role):
    text = open(f"{observed_dir}/{step}.{role}.hex").read().strip()
    return bytes.fromhex(text)

def u64(data, offset):
    return int.from_bytes(data[offset:offset + 8], "little")

cash_total = 0
eggs = [0, 0]
for entry in rules["positions"]:
    data = load(entry["step"], entry["role"])
    cash = u64(data, offsets["position_cash"])
    reserved = u64(data, offsets["position_reserved"])
    egg0 = u64(data, offsets["position_internal0"])
    egg1 = u64(data, offsets["position_internal1"])
    cash_total += cash
    eggs[0] += egg0
    eggs[1] += egg1
    print(f"  {entry['role']}: cash={cash} reserved={reserved} eggs=[{egg0}, {egg1}]")
hoard = u64(load(rules["hoard"]["step"], rules["hoard"]["role"]), offsets["hoard_collateral"])
custody = u64(load(rules["hoard_token"]["step"], rules["hoard_token"]["role"]), offsets["token_amount"])
print(f"  hoard locked backing = {hoard}")
print(f"  pooled custody token = {custody}")
expected = rules["expected"]
checks = [
    ("position cash total", cash_total, expected["cash_total"]),
    ("eggs outcome 0", eggs[0], expected["eggs_outcome0"]),
    ("eggs outcome 1", eggs[1], expected["eggs_outcome1"]),
    ("locked backing", hoard, expected["locked"]),
    ("pooled custody", custody, expected["custody"]),
]
failed = False
for label, got, want in checks:
    verdict = "ok" if got == want else "FAIL"
    if got != want:
        failed = True
    print(f"  {label}: observed={got} expected={want} {verdict}")
identities = [
    ("cash_total + locked == endowed_total", cash_total + hoard, rules["endowed_total"]),
    ("eggs[0] == split_total", eggs[0], rules["split_total"]),
    ("eggs[1] == split_total", eggs[1], rules["split_total"]),
    ("custody == endowed_total", custody, rules["endowed_total"]),
]
for label, got, want in identities:
    verdict = "ok" if got == want else "FAIL"
    if got != want:
        failed = True
    print(f"  identity {label}: {got} == {want} {verdict}")
sys.exit(1 if failed else 0)
PY

echo
echo "== falsifiability: corrupt one terminal byte and require red =="
victim="$plan/expected/general-43-settle-portfolio-pair.owner-c.position.hex"
cp "$victim" "$victim.orig"
python3 - "$victim" <<'PY'
import sys
path = sys.argv[1]
text = open(path).read().strip()
byte = (int(text[:2], 16) + 1) % 256
open(path, "w").write(f"{byte:02x}" + text[2:] + "\n")
PY
start_validator
if run_signed "$work/observed-falsify" >"$log/falsify.log" 2>&1; then
  echo "FAIL: corrupted terminal expectation still passed"
  exit 1
fi
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
echo "committed_watched_accounts=$committed_watched"
echo "genesis_assisted_program_accounts=$committed_precreated"
echo "portfolio_pair=CLEARED"
echo "single_slice=CLEARED"
echo "conservation=RE-DERIVED-FROM-OBSERVED-BYTES"
echo "falsifiability=PASS"
echo "walk_wall_seconds=$walk_seconds"
echo "sbf_elf_sha256=$elf_sha256"
echo "sbf_elf_bytes=$elf_bytes"
echo "work_dir=$work"
