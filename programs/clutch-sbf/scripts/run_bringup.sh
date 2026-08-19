#!/usr/bin/env bash
# Dragon's Clutch SBF bring-up gate.
#
# Builds the program ELF twice into fresh target directories, records both
# hashes, generates the differential plan with the offline reference adapter,
# starts a *local loopback* `solana-test-validator` with the program and every
# pre-state account loaded at genesis, and compares the SVM post-state against
# the oracle post-state byte for byte, for every implemented instruction family.
#
# It then runs the PROJECT.md section-10 **lifecycle walk** in the same
# validator session: one market taken end to end as one ordered gate, closing
# with the section-10 item-10 accounting identity read out of the on-chain
# bytes.  Both stages must pass for this script to exit zero.
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
build_sbf_version="$("$build_sbf" --version)"
printf '%s\n' "$build_sbf_version" | tr '\n' ' '
echo

echo "== source pin =="
# A reproducible-build record that names no commit is not a record.  The tree
# state is printed rather than assumed clean: a dirty tree still produces two
# identical ELFs, but the digest then names a working tree and not a commit,
# and that has to be visible in the evidence.
repo="$(cd "$root/../.." && pwd)"
# Exactly the paths `cargo-build-sbf --manifest-path program/Cargo.toml` reads.
# A tree that is dirty *only* outside this list still produces a digest that
# names a commit, and saying which of the two situations holds is the whole
# point of printing the tree state at all.
elf_inputs=(
  programs/clutch-sbf/.cargo
  programs/clutch-sbf/program
  programs/clutch-sbf/Cargo.toml
  programs/clutch-sbf/Cargo.lock
  programs/clutch-sbf/vendor
  programs/solana-layout
  programs/solana-reference
  crates
)
( cd "$repo" && git rev-parse HEAD 2>/dev/null || echo "(not a git checkout)" )
dirty="$( cd "$repo" && git status --porcelain 2>/dev/null )"
elf_dirty="$( cd "$repo" && git status --porcelain -- "${elf_inputs[@]}" 2>/dev/null )"
if [ -n "$elf_dirty" ]; then
  echo "elf_inputs=DIRTY (the ELF digest below names this working tree, not a commit)"
  echo "$elf_dirty" | sed 's/^/  /'
elif [ -n "$dirty" ]; then
  echo "elf_inputs=clean (the ELF digest below names the commit above)"
  echo "tree=DIRTY outside every ELF input; the dirty files are:"
  echo "$dirty" | sed 's/^/  /'
else
  echo "elf_inputs=clean"
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
stack_findings="$({ grep -E "^Error: (Function|A function call)" "$log/sbf-build-1.log" || true; } | sort -u)"
if [ -z "$stack_findings" ]; then
  echo "(none)"
else
  printf '%s\n' "$stack_findings"

  # The SBF backend diagnoses every function while it compiles dependency
  # rlibs, before fat LTO removes host-only public APIs.  A diagnostic is a
  # deployment blocker if its symbol survives into the final program, but it
  # is not executable undefined behavior if the final ELF does not contain
  # that symbol at all.  Check the unstripped linked ELF instead of asking a
  # reviewer to infer reachability from a noisy build log.
  platform_tools_version="$(printf '%s\n' "$build_sbf_version" | awk '$1 == "platform-tools" { print $2 }')"
  llvm_objdump="${LLVM_OBJDUMP:-$HOME/.cache/solana/$platform_tools_version/platform-tools/llvm/bin/llvm-objdump}"
  unstripped_elf="$(find "$work/target-1" -type f -path '*/release/deps/clutch_sbf.so' -print -quit)"
  if [ -z "$platform_tools_version" ] || [ ! -x "$llvm_objdump" ] || [ -z "$unstripped_elf" ]; then
    echo "FAIL: cannot inspect final ELF reachability for backend stack findings"
    exit 1
  fi
  "$llvm_objdump" --syms "$unstripped_elf" > "$log/final-elf-symbols.txt"
  stack_symbols="$(
    printf '%s\n' "$stack_findings" \
      | sed -E -n \
          -e 's/^Error: Function ([^ ]+) .*/\1/p' \
          -e 's/^Error: A function call in method ([^ ]+) .*/\1/p' \
      | sort -u
  )"
  reachable=0
  while IFS= read -r symbol; do
    [ -z "$symbol" ] && continue
    if grep -Fq "$symbol" "$log/final-elf-symbols.txt"; then
      echo "FAIL: stack-diagnostic symbol survived final ELF LTO: $symbol"
      reachable=1
    fi
  done <<< "$stack_symbols"
  if [ "$reachable" -ne 0 ]; then
    exit 1
  fi
  stack_symbol_count="$(printf '%s\n' "$stack_symbols" | sed '/^$/d' | wc -l | tr -d ' ')"
  echo "final_elf_stack_diagnostic_symbols=ABSENT ($stack_symbol_count dependency symbols removed by LTO)"
fi

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
# Refuse to mistake an older validator on the fixed port for this run.  A
# health-only readiness check cannot distinguish the process we just spawned
# from a pre-existing listener.
if curl -s -m 1 "$url" -X POST -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null \
    | grep -q '"result":"ok"'; then
  echo "FAIL: $url was already serving before this gate started"
  exit 1
fi
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
  if ! kill -0 "$validator_pid" 2>/dev/null; then
    break
  fi
  # `getHealth` turns green ~0.45s after launch, while the bank is still at
  # SLOT 0 -- and a program whose deployment slot is the current slot is not
  # yet visible to the runtime, which logs "Program is not deployed" and
  # returns `UnsupportedProgramId`.  MEASURED: at slot 0 the program account is
  # already present, `executable=true`, and owned by BPFLoader2, and every
  # transaction still fails; the identical transaction succeeds at slot 1.
  #
  # So an account-shape check CANNOT close this window and must not replace the
  # probe below: the only readiness signal that works is actually executing
  # something.  That is why this waits on a real `--only split` simulation.
  if curl -fsS -m 2 "$url" -X POST -H 'Content-Type: application/json' \
      -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$program_id\",{\"encoding\":\"base64\"}]}" \
      2>/dev/null \
      | python3 -c 'import json,sys; v=json.load(sys.stdin).get("result", {}).get("value"); raise SystemExit(not (v and v.get("executable") is True))' \
      2>/dev/null \
      && python3 "$here/simulate.py" --url "$url" --plan "$plan" --only split \
        > "$log/readiness.log" 2>&1; then
    ready=1
    break
  fi
  sleep 1
done
if [ "$ready" -ne 1 ]; then
  echo "FAIL: local validator did not execute the program readiness probe"
  tail -30 "$log/readiness.log" 2>/dev/null || true
  tail -30 "$log/validator.log"
  exit 1
fi
echo "validator executed program readiness probe on $url (loopback only)"

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

echo
echo "== the PROJECT.md section 10 lifecycle walk (same validator session) =="
# One market, walked end to end, as ONE gate.  The walk is not ten more
# independent checks: any step that diverges, refuses where it should accept,
# or accepts where it should refuse fails the whole walk, and the terminal
# accounting identity is part of the same gate.
walk_status=0
python3 "$here/simulate.py" --url "$url" --plan "$plan" --lifecycle 2>&1 \
  | tee "$log/lifecycle.log" || walk_status=$?
if [ "$walk_status" -eq 0 ] && ! grep -q '^PASS$' "$log/lifecycle.log"; then
  walk_status=1
fi
if [ "$walk_status" -ne 0 ]; then
  status=1
fi

if [ "$walk_status" -eq 0 ]; then
  echo
  echo "== lifecycle falsifiability self-check (same validator session) =="
  # The terminal identity is the one claim in this repository that reads a
  # number off the chain and asserts an equation over it, so it gets its own
  # falsification: first the readout (does the gate really compare the bytes
  # the bank returned?), then the arithmetic (does the gate really evaluate
  # the equation?).  Both mutations must turn the walk red.
  cp "$plan/plan.json" "$plan/plan.json.orig"

  python3 - "$plan/plan.json" <<'MUTATE'
import json
import sys

path = sys.argv[1]
plan = json.load(open(path))
values = plan["lifecycle"]["terminal"]["values"]
target = next(value for value in values if value["label"] == "hoard_collateral")
target["expected"] += 1
json.dump(plan, open(path, "w"))
MUTATE
  if python3 "$here/simulate.py" --url "$url" --plan "$plan" --lifecycle \
      > "$log/lifecycle-falsify-readout.log" 2>&1; then
    echo "FAIL: a mutated terminal readout still passed"
    status=1
  else
    echo "the terminal Hoard expectation was moved by one atom; the walk went red:"
    grep -m1 'terminal identity:' "$log/lifecycle-falsify-readout.log" | sed 's/^/  /'
  fi
  cp "$plan/plan.json.orig" "$plan/plan.json"

  python3 - "$plan/plan.json" <<'MUTATE'
import json
import sys

path = sys.argv[1]
plan = json.load(open(path))
for identity in plan["lifecycle"]["terminal"]["identities"]:
    for term in identity["right"]:
        if term.get("label") == "kernel_total_supply_1" and term.get("scale") == 1:
            term["scale"] = 2
            json.dump(plan, open(path, "w"))
            sys.exit(0)
raise SystemExit("no identity term to mutate")
MUTATE
  if python3 "$here/simulate.py" --url "$url" --plan "$plan" --lifecycle \
      > "$log/lifecycle-falsify-identity.log" 2>&1; then
    echo "FAIL: a mutated accounting identity still closed"
    status=1
  else
    echo "one payout weight in one identity was doubled; the walk went red:"
    grep -m1 'does not close' "$log/lifecycle-falsify-identity.log" | sed 's/^/  /'
  fi
  mv "$plan/plan.json.orig" "$plan/plan.json"
fi

echo "${hashes[0]}" > "$work/elf.sha256"
echo
echo "elf sha256: ${hashes[0]}"
echo "sbf_elf_sha256=${hashes[0]}"
echo "work dir: $work"
exit "$status"
