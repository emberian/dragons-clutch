#!/usr/bin/env bash
# Dragon's Clutch SBF bring-up gate.
#
# Builds the program ELF twice into fresh target directories, records both
# hashes, generates the differential plan with the offline reference adapter,
# starts a *local loopback* `solana-test-validator` with the program and the
# pre-state accounts loaded at genesis, and compares the SVM post-state against
# the reference post-state byte for byte.
#
# It touches no public network, signs nothing, and deploys nothing.  It is
# bring-up evidence for one instruction, not a deployment.
#
# Usage: scripts/run_bringup.sh [work-dir]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
work="${1:-${TMPDIR:-/tmp}/clutch-sbf-bringup}"

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
export SOLANA_BIN="${SOLANA_BIN:-$solana_home/solana}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"
test_validator="${SOLANA_TEST_VALIDATOR:-$solana_home/solana-test-validator}"
rpc_port="${CLUTCH_RPC_PORT:-18899}"
faucet_port="${CLUTCH_FAUCET_PORT:-19900}"
url="http://127.0.0.1:${rpc_port}"

rm -rf "$work"
mkdir -p "$work"
plan="$work/plan"
log="$work/logs"
mkdir -p "$plan" "$log"

echo "== toolchain =="
"$SOLANA_BIN" --version
"$build_sbf" --version | tr '\n' ' '
echo

echo "== reproducible ELF build (twice, fresh target dirs) =="
hashes=()
for pass in 1 2; do
  target="$work/target-$pass"
  out="$work/out-$pass"
  mkdir -p "$out"
  CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$target" \
    "$build_sbf" --manifest-path "$root/program/Cargo.toml" --sbf-out-dir "$out" \
    > "$log/sbf-build-$pass.log" 2>&1 || {
      echo "SBF build $pass failed; see $log/sbf-build-$pass.log"; tail -30 "$log/sbf-build-$pass.log"; exit 1; }
  hash="$(shasum -a 256 "$out/clutch_sbf.so" | awk '{print $1}')"
  size="$(wc -c < "$out/clutch_sbf.so" | tr -d ' ')"
  hashes+=("$hash")
  echo "pass $pass  sha256=$hash  bytes=$size"
done
if [ "${hashes[0]}" != "${hashes[1]}" ]; then
  echo "FAIL: the two SBF builds differ"
  exit 1
fi
echo "sbf_reproducibility=PASS"
elf="$work/out-1/clutch_sbf.so"

echo
echo "== stack findings reported by the SBF backend =="
grep -E "^Error: (Function|A function call)" "$log/sbf-build-1.log" | sort -u || echo "(none)"

echo
echo "== differential plan (offline reference adapter is the oracle) =="
(cd "$root" && cargo run --offline -q -p clutch-sbf-harness -- "$plan")

manifest="$plan/manifest.txt"
value() { grep "^$1=" "$manifest" | cut -d= -f2-; }

validator_args=(
  --ledger "$work/ledger" --reset --quiet
  --rpc-port "$rpc_port" --faucet-port "$faucet_port"
  --mint "$(value payer)"
  --bpf-program "$(value program_id)" "$elf"
)
for role in realm profile market hoard position kernel external replay; do
  validator_args+=(--account "$(value "account.$role")" "$plan/accounts/$role.json")
done
validator_args+=(--account "$(value imposter)" "$plan/accounts/replay-imposter.json")

echo
echo "== local loopback validator =="
"$test_validator" "${validator_args[@]}" > "$log/validator.log" 2>&1 &
validator_pid=$!
cleanup() {
  if kill -0 "$validator_pid" 2>/dev/null; then
    kill "$validator_pid" 2>/dev/null || true
    wait "$validator_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

ready=0
for _ in $(seq 1 60); do
  if curl -s -m 2 "$url" -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null | grep -q '"result":"ok"'; then
    ready=1
    break
  fi
  sleep 1
done
if [ "$ready" -ne 1 ]; then
  echo "FAIL: local validator did not become healthy"
  tail -30 "$log/validator.log"
  exit 1
fi
echo "validator healthy on $url (loopback only)"

echo
echo "== differential and refusal checks =="
status=0
python3 "$here/simulate.py" --url "$url" --plan "$plan" || status=$?

echo
echo "work dir: $work"
exit "$status"
