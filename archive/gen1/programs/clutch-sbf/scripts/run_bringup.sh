#!/usr/bin/env bash
# Dragon's Clutch SBF bring-up gate.
#
# Builds two deliberately different reproducible ELF profiles. The default ELF
# has an empty production source registry and must refuse Endow with 0x0079.
# The explicitly named NON-PRODUCTION mock-source ELF preserves the successful
# differential and PROJECT.md section-10 lifecycle campaigns. Separate plan,
# log, ledger, digest and manifest paths make the two results non-interchangeable.
#
# It touches no public network, signs nothing, and deploys nothing.  It is
# bring-up evidence for an instruction set, not a deployment.
#
# Usage: scripts/run_bringup.sh [work-dir]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
repo="$(cd "$root/../.." && pwd)"
work="${1:-${TMPDIR:-/tmp}/clutch-sbf-bringup}"

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
export SOLANA_BIN="${SOLANA_BIN:-$solana_home/solana}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"
loopback_tools="$repo/tools/agave-loopback-validator"
loopback_cache="${CLUTCH_AGAVE_LOOPBACK_CACHE:-$repo/.cache/agave-loopback-validator}"
test_validator="${CLUTCH_LOOPBACK_TEST_VALIDATOR:-${SOLANA_TEST_VALIDATOR:-$loopback_cache/bin/solana-test-validator}}"
listener_probe="$loopback_tools/probe-listeners.sh"
rpc_port="${CLUTCH_RPC_PORT:-18899}"
faucet_port="${CLUTCH_FAUCET_PORT:-19900}"
gossip_port="${CLUTCH_GOSSIP_PORT:-18000}"
dynamic_port_range="${CLUTCH_DYNAMIC_PORT_RANGE:-18001-18099}"
url="http://127.0.0.1:${rpc_port}"

rm -rf "$work"
default_plan="$work/plan-default-production-inert"
mock_plan="$work/plan-non-production-mock-source"
default_log="$work/logs/default-production-inert"
mock_log="$work/logs/non-production-mock-source"
mkdir -p "$default_plan" "$mock_plan" "$default_log" "$mock_log"
python3 "$loopback_tools/verify-runtime.py" --binary "$test_validator" \
  | tee "$work/validator-runtime.txt"

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
echo "== DEFAULT EMPTY-REGISTRY ELF (twice, fresh target dirs) =="
default_hashes=()
for pass in 1 2; do
  target="$work/default-target-$pass"
  out="$work/default-out-$pass"
  mkdir -p "$out"
  CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$target" \
    "$build_sbf" --manifest-path "$root/program/Cargo.toml" --sbf-out-dir "$out" \
    > "$default_log/sbf-build-$pass.log" 2>&1 || {
      echo "default SBF build $pass failed; see $default_log/sbf-build-$pass.log"; tail -30 "$default_log/sbf-build-$pass.log"; exit 1; }
  hash="$(shasum -a 256 "$out/clutch_sbf.so" | awk '{print $1}')"
  size="$(wc -c < "$out/clutch_sbf.so" | tr -d ' ')"
  default_hashes+=("$hash")
  echo "default pass $pass  sha256=$hash  bytes=$size"
done
if [ "${default_hashes[0]}" != "${default_hashes[1]}" ]; then
  echo "FAIL: the two default SBF builds differ"
  exit 1
fi
default_elf="$work/default-out-1/clutch_sbf.so"

echo
echo "== NON-PRODUCTION MOCK-SOURCE ELF (twice, fresh target dirs) =="
mock_hashes=()
for pass in 1 2; do
  target="$work/mock-target-$pass"
  out="$work/mock-out-$pass"
  mkdir -p "$out"
  CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$target" \
    "$build_sbf" --manifest-path "$root/program/Cargo.toml" \
      --features non-production-mock-source --sbf-out-dir "$out" \
    > "$mock_log/sbf-build-$pass.log" 2>&1 || {
      echo "mock SBF build $pass failed; see $mock_log/sbf-build-$pass.log"; tail -30 "$mock_log/sbf-build-$pass.log"; exit 1; }
  hash="$(shasum -a 256 "$out/clutch_sbf.so" | awk '{print $1}')"
  size="$(wc -c < "$out/clutch_sbf.so" | tr -d ' ')"
  mock_hashes+=("$hash")
  echo "NON-PRODUCTION mock pass $pass  sha256=$hash  bytes=$size"
done
if [ "${mock_hashes[0]}" != "${mock_hashes[1]}" ]; then
  echo "FAIL: the two non-production mock SBF builds differ"
  exit 1
fi
if [ "${default_hashes[0]}" = "${mock_hashes[0]}" ]; then
  echo "FAIL: default and mock feature produced the same ELF digest"
  exit 1
fi
mock_elf="$work/mock-out-1/clutch_sbf.so"
echo "default_reproducibility=PASS"
echo "mock_reproducibility=PASS"
echo "profile_separation=PASS"

echo
echo "== stack findings reported by the SBF backend =="
stack_findings="$({ grep -E "^Error: (Function|A function call)" "$default_log/sbf-build-1.log" || true; } | sort -u)"
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
  unstripped_elf="$(find "$work/default-target-1" -type f -path '*/release/deps/clutch_sbf.so' -print -quit)"
  if [ -z "$platform_tools_version" ] || [ ! -x "$llvm_objdump" ] || [ -z "$unstripped_elf" ]; then
    echo "FAIL: cannot inspect final ELF reachability for backend stack findings"
    exit 1
  fi
  "$llvm_objdump" --syms "$unstripped_elf" > "$default_log/final-elf-symbols.txt"
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
    if grep -Fq "$symbol" "$default_log/final-elf-symbols.txt"; then
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
echo "== profile-bound plans =="
(cd "$root" && cargo run --offline -q -p clutch-sbf-harness -- \
  "$default_plan" --default-source-refusal) | tee "$default_log/plan.log"
(cd "$root" && cargo run --offline -q -p clutch-sbf-harness -- \
  "$mock_plan") | tee "$mock_log/plan.log"
python3 - "$default_plan/plan.json" "$mock_plan/plan.json" <<'CHECK'
import json, sys
default = json.load(open(sys.argv[1]))
mock = json.load(open(sys.argv[2]))
assert default["source_mode"] == "default-production-inert"
assert mock["source_mode"] == "non-production-mock-source"
default_endow = next(case for case in default["cases"] if case["name"] == "endow")
mock_endow = next(case for case in mock["cases"] if case["name"] == "endow")
assert default_endow["kind"] == "refuse"
assert default_endow["expect_code"] == 0x0079
assert "compare" not in default_endow
assert default["lifecycle"] is None
assert mock_endow["kind"] == "accept"
assert mock_endow["compare"]
assert isinstance(mock["lifecycle"], dict)
CHECK
echo
echo "== refusal rollback checker capability self-test =="
python3 "$here/simulate.py" --self-test-refusal-rollback 2>&1 \
  | tee "$default_log/falsify-refusal-rollback-checker.log"

validator_pid=""
cleanup() {
  if [ -n "$validator_pid" ] && kill -0 "$validator_pid" 2>/dev/null; then
    kill "$validator_pid" 2>/dev/null || true
    wait "$validator_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

status=0
run_profile() {
  profile="$1"
  plan="$2"
  profile_log="$3"
  elf="$4"
  lifecycle="$5"
  program_id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["program_id"])' "$plan/plan.json")"
  payer="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["payer"])' "$plan/plan.json")"
  # The patched binary plus the pre/post probes are both mandatory. The flag is
  # still explicit because it scopes validator-node and gossip sockets too.
  validator_args=(
    --ledger "$work/ledger-$profile" --reset --quiet
    --bind-address 127.0.0.1
    --rpc-port "$rpc_port" --faucet-port "$faucet_port"
    --gossip-port "$gossip_port" --dynamic-port-range "$dynamic_port_range"
    --mint "$payer"
    --bpf-program "$program_id" "$elf"
  )
  while read -r role address file; do
    [ -z "$role" ] && continue
    validator_args+=(--account "$address" "$plan/$file")
  done < "$plan/genesis.txt"

  echo
  echo "== $profile local loopback validator =="
  if curl -s -m 1 "$url" -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' 2>/dev/null \
      | grep -q '"result":"ok"'; then
    echo "FAIL: $url was already serving before the $profile gate started"
    exit 1
  fi
  "$test_validator" "${validator_args[@]}" > "$profile_log/validator.log" 2>&1 &
  validator_pid=$!
  ready=0
  for _ in $(seq 1 60); do
    if ! kill -0 "$validator_pid" 2>/dev/null; then break; fi
    if curl -fsS -m 2 "$url" -X POST -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$program_id\",{\"encoding\":\"base64\"}]}" 2>/dev/null \
        | python3 -c 'import json,sys; v=json.load(sys.stdin).get("result", {}).get("value"); raise SystemExit(not (v and v.get("executable") is True))' 2>/dev/null; then
      ready=1
      break
    fi
    sleep 1
  done
  if [ "$ready" -ne 1 ]; then
    echo "FAIL: $profile validator did not expose the executable program"
    tail -30 "$profile_log/validator.log"
    status=1
    cleanup
    validator_pid=""
    return
  fi
  "$listener_probe" "$validator_pid" "$rpc_port" "$faucet_port" "$test_validator" \
    | tee "$profile_log/listeners-before.txt"

  profile_status=0
  python3 "$here/simulate.py" --url "$url" --plan "$plan" 2>&1 \
    | tee "$profile_log/differential.log" || profile_status=$?
  if [ "$profile_status" -eq 0 ] && ! grep -q '^PASS$' "$profile_log/differential.log"; then
    profile_status=1
  fi
  if [ "$profile_status" -ne 0 ]; then status=1; fi

  if [ "$profile_status" -eq 0 ]; then
    echo
    echo "== $profile differential falsifiability self-check =="
    victim="$plan/expected/split.seam.hoard.hex"
    cp "$victim" "$victim.orig"
    python3 - "$victim" <<'MUTATE'
import sys
path = sys.argv[1]
text = open(path).read().strip()
index = 98 * 2
byte = (int(text[index:index + 2], 16) + 1) % 256
open(path, "w").write(text[:index] + f"{byte:02x}" + text[index + 2:] + "\n")
MUTATE
    if python3 "$here/simulate.py" --url "$url" --plan "$plan" --only split \
        > "$profile_log/falsify.log" 2>&1; then
      echo "FAIL: $profile mutated oracle expectation still passed"
      status=1
    elif ! grep -q 'on-chain bytes != oracle bytes' "$profile_log/falsify.log"; then
      echo "FAIL: $profile oracle mutation failed for an unrelated reason"
      tail -30 "$profile_log/falsify.log"
      status=1
    else
      grep -m1 'on-chain bytes != oracle bytes' "$profile_log/falsify.log" \
        | sed 's/^/  red: /'
    fi
    mv "$victim.orig" "$victim"
  fi

  if [ "$lifecycle" = "yes" ]; then
    echo
    echo "== NON-PRODUCTION mock-source lifecycle (same validator session) =="
    walk_status=0
    python3 "$here/simulate.py" --url "$url" --plan "$plan" --lifecycle 2>&1 \
      | tee "$profile_log/lifecycle.log" || walk_status=$?
    if [ "$walk_status" -eq 0 ] && ! grep -q '^PASS$' "$profile_log/lifecycle.log"; then
      walk_status=1
    fi
    if [ "$walk_status" -ne 0 ]; then status=1; fi

    if [ "$walk_status" -eq 0 ]; then
      echo
      echo "== NON-PRODUCTION mock lifecycle falsifiability self-check =="
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
          > "$profile_log/lifecycle-falsify-readout.log" 2>&1; then
        echo "FAIL: a mutated terminal readout still passed"
        status=1
      else
        grep -m1 'terminal identity:' "$profile_log/lifecycle-falsify-readout.log" \
          | sed 's/^/  red: /'
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
          > "$profile_log/lifecycle-falsify-identity.log" 2>&1; then
        echo "FAIL: a mutated accounting identity still closed"
        status=1
      else
        grep -m1 'does not close' "$profile_log/lifecycle-falsify-identity.log" \
          | sed 's/^/  red: /'
      fi
      mv "$plan/plan.json.orig" "$plan/plan.json"
    fi
  fi
  "$listener_probe" "$validator_pid" "$rpc_port" "$faucet_port" "$test_validator" \
    | tee "$profile_log/listeners-after.txt"
  cleanup
  validator_pid=""
}

run_profile "default-production-inert" "$default_plan" "$default_log" "$default_elf" "no"
run_profile "non-production-mock-source" "$mock_plan" "$mock_log" "$mock_elf" "yes"

cat_manifest="$work/elf-profiles.txt"
printf '%s\n' \
  "default_profile=default-production-inert" \
  "default_elf_sha256=${default_hashes[0]}" \
  "default_endow=REFUSE_0x0079" \
  "mock_profile=NON-PRODUCTION-non-production-mock-source" \
  "mock_elf_sha256=${mock_hashes[0]}" \
  "mock_endow=SUCCESS_EVIDENCE_ONLY" \
  "digests_distinct=true" > "$cat_manifest"
printf '%s\n' \
  "default_sbf_elf_sha256=${default_hashes[0]}" \
  "non_production_mock_sbf_elf_sha256=${mock_hashes[0]}" \
  "profile_manifest=$cat_manifest" \
  "work dir: $work"
exit "$status"
