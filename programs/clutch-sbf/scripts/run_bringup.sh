#!/usr/bin/env bash
# Dragon's Clutch SBF bring-up gate.
#
# Builds the program ELF twice into fresh target directories, records both
# hashes, generates the differential plan with the offline reference adapter,
# starts a *local loopback* `solana-test-validator` with the program and every
# pre-state account loaded at genesis, and compares the SVM post-state against
# the oracle post-state byte for byte, for every implemented instruction family.
#
# It touches no public network, signs nothing, and deploys nothing.  It is
# bring-up evidence for an instruction set, not a deployment.
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

echo "== source pin =="
# A reproducible-build record that names no commit is not a record.  The tree
# state is printed rather than assumed clean: a dirty tree still produces two
# identical ELFs, but the digest then names a working tree and not a commit,
# and that has to be visible in the evidence.
( cd "$root" && git rev-parse HEAD 2>/dev/null || echo "(not a git checkout)" )
if [ -n "$( cd "$root" && git status --porcelain 2>/dev/null )" ]; then
  echo "tree=DIRTY (the ELF digest below names this working tree, not a commit)"
  ( cd "$root" && git status --porcelain ) | sed 's/^/  /'
else
  echo "tree=clean"
fi

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
echo "== differential plan (the offline reference adapter is the oracle) =="
(cd "$root" && cargo run --offline -q -p clutch-sbf-harness -- "$plan") | tee "$log/plan.log"

program_id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["program_id"])' "$plan/plan.json")"
payer="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["payer"])' "$plan/plan.json")"

validator_args=(
  --ledger "$work/ledger" --reset --quiet
  --rpc-port "$rpc_port" --faucet-port "$faucet_port"
  --mint "$payer"
  --bpf-program "$program_id" "$elf"
)
# Every account of every market plane in the plan, plus the Realm-wide
# accounts, the two feed heads, the caller-supplied buffers, the batch-auction
# plane no implemented instruction touches, and the imposter replay account.
while read -r role address file; do
  [ -z "$role" ] && continue
  validator_args+=(--account "$address" "$plan/$file")
done < "$plan/genesis.txt"

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
python3 "$here/simulate.py" --url "$url" --plan "$plan" 2>&1 | tee "$log/differential.log" || status=$?
if [ "$status" -eq 0 ] && ! grep -q '^PASS$' "$log/differential.log"; then
  status=1
fi

if [ "$status" -eq 0 ]; then
  echo
  echo "== falsifiability self-check (same validator session) =="
  # A comparison that cannot go red is not evidence.  One byte of one oracle
  # expectation is flipped -- the Hoard collateral a Split moved -- and the
  # split differential is re-run against the same still-running validator and
  # the same unmodified ELF.  It must fail, and only on that account.
  victim="$plan/expected/split.seam.hoard.hex"
  cp "$victim" "$victim.orig"
  python3 - "$victim" <<'MUTATE'
import sys
path = sys.argv[1]
text = open(path).read().strip()
# Byte 98 is the Hoard's collateral field; bump its low byte by one.
index = 98 * 2
byte = (int(text[index:index + 2], 16) + 1) % 256
open(path, "w").write(text[:index] + f"{byte:02x}" + text[index + 2:] + "\n")
MUTATE
  if python3 "$here/simulate.py" --url "$url" --plan "$plan" --only split       > "$log/falsify.log" 2>&1; then
    echo "FAIL: a mutated oracle expectation still passed"
    status=1
  else
    echo "one byte of the Hoard collateral expectation was flipped; the differential went red:"
    grep -m1 'on-chain bytes != oracle bytes' "$log/falsify.log" | sed 's/^/  /'
  fi
  mv "$victim.orig" "$victim"
fi

echo "${hashes[0]}" > "$work/elf.sha256"
echo
echo "elf sha256: ${hashes[0]}"
echo "work dir: $work"
exit "$status"
