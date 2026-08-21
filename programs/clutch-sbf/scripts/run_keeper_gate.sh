#!/usr/bin/env bash
# The keeper gate: permissionless liveness as an operating fact.
#
# A local validator, the non-production mock-source ELF, a real market and a
# real mixed book -- and then `clutch-keeper` as the ONLY driver of every
# permissionless step from the epoch freeze through settlement and the whole
# TerminalClosure DAG.  Nothing here replays a pregenerated crank: the keeper
# decides each action from committed account bytes it reads back off the bank.
#
# What the plan generator is still used for, and why:
#
#   * steps 1-16  -- the market, the four owners' funded endowments, their
#                    complete-set splits, and the sealed batch-policy artifact.
#                    Participants' business, not a crank.
#   * steps 21-27 -- the six placements and one cancellation.  Same.
#   * steps 29-32 -- the candidate submission, its feed, and its seal.  A
#                    candidate is SOLVER output: computing one is not a crank,
#                    and a keeper that fabricated one would be inventing the
#                    thing the relation exists to verify.
#
# Everything else -- InitEpoch, InitOrderPage, FreezeEpoch, InitClearWork and
# its four staged grows, both order passes, the slice pass, CompleteClearWork,
# FinalizeSelection, FreezeEntitlement, every EntitleSlice and SettlePage, and
# tags 60-67 -- is the keeper, from state, with its own keypair.
#
# The three gates:
#
#   1. the walk       -- the keeper alone drives the lifecycle to settled and
#                        closed, and the terminal conservation identities are
#                        re-derived from the FINAL committed bytes;
#   2. the falsifier  -- the keeper is SIGKILLed mid-clearing-walk and started
#                        again with no state but the chain, and must finish;
#   3. the wire       -- the batched-fold packet answer, measured by
#                        serialization and confirmed by this validator's own
#                        transport.  This is the discharge of the sealed
#                        `cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`
#                        caveat.
#
# Loopback by construction: the keeper refuses any non-loopback RPC URL, and
# every key this gate creates is a fresh test-only key in an mktemp directory
# it unlinks on exit.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
repo="$(cd "$root/../.." && pwd)"

if [ "$#" -gt 1 ]; then
  echo "usage: scripts/run_keeper_gate.sh [new-work-dir]"
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
  work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-keeper-gate.XXXXXX")"
fi

keys="$(mktemp -d "${TMPDIR:-/tmp}/clutch-keeper-keys.XXXXXX")"
plan="$work/plan"
out="$work/out"
log="$work/logs"
mkdir -p "$plan" "$out" "$log"

solana_home="${SOLANA_HOME:-$HOME/.local/share/solana/install/active_release/bin}"
solana_bin="${SOLANA_BIN:-$solana_home/solana}"
keygen="${SOLANA_KEYGEN:-$solana_home/solana-keygen}"
build_sbf="${CARGO_BUILD_SBF:-$solana_home/cargo-build-sbf}"
test_validator="${SOLANA_TEST_VALIDATOR:-$solana_home/solana-test-validator}"
# Ports are picked from the 9000-9099 lane range so this gate can run beside
# the other committed walks without either stealing the other's endpoint.
rpc_port="${CLUTCH_KEEPER_RPC_PORT:-9011}"
faucet_port="${CLUTCH_KEEPER_FAUCET_PORT:-9012}"
url="http://127.0.0.1:${rpc_port}"
validator_pid=""
keeper_pid=""

# The keeper's own wallet, and the four signing owners of the book.  The actor
# is also the owner of the one order the candidate gives a zero fill, so it is
# handed to the keeper as `--owner` for the single owner-signed route.
key_names=(
  payer actor owner-b owner-c owner-d
  owner-b-collateral-token owner-c-collateral-token owner-d-collateral-token
  keeper
)

stop_keeper() {
  if [ -n "$keeper_pid" ] && kill -0 "$keeper_pid" 2>/dev/null; then
    kill -9 "$keeper_pid" 2>/dev/null || true
    wait "$keeper_pid" 2>/dev/null || true
  fi
  keeper_pid=""
}

stop_validator() {
  if [ -n "$validator_pid" ] && kill -0 "$validator_pid" 2>/dev/null; then
    kill "$validator_pid" 2>/dev/null || true
    wait "$validator_pid" 2>/dev/null || true
  fi
  validator_pid=""
}

cleanup() {
  stop_keeper
  # Belt and braces: never leave a keeper behind for the next run to trip over.
  pkill -9 -f "clutch-keeper run --url $url" 2>/dev/null || true
  stop_validator
  # The only secrets this gate creates are these exact files in the
  # mktemp-owned directory.  Unlink them rather than moving them to Trash.
  local files=() name
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
  echo "FAIL: $url was already serving before the keeper gate"
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
  >"$log/build.log" 2>&1 || { tail -60 "$log/build.log"; exit 1; }
elf="$out/clutch_sbf.so"
elf_sha256="$(shasum -a 256 "$elf" | awk '{print $1}')"
echo "sbf_elf_sha256=$elf_sha256"

echo
echo "== plan (market, owners, book, candidate) =="
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
    >"$log/plan.log" 2>&1
program_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["program_id"])' "$plan/committed.json")"
echo "program_id=$program_id"

# The keeper is invoked as its built binary, never through `cargo run`.  The
# falsifier SIGKILLs it, and `cargo run` would put cargo's pid between the
# gate and the process it means to kill -- leaving an orphaned keeper alive
# beside the restarted one, which would quietly destroy the very claim this
# gate makes ("no other driver").
CARGO_NET_OFFLINE=true cargo build --offline -q \
  --manifest-path "$root/Cargo.toml" -p clutch-keeper >"$log/keeper-build.log" 2>&1 || {
    tail -40 "$log/keeper-build.log"; exit 1; }
keeper_bin="$root/target/debug/clutch-keeper"
[ -x "$keeper_bin" ] || { echo "FAIL: no keeper binary at $keeper_bin"; exit 1; }
keeper() {
  "$keeper_bin" "$@"
}

# --- small chain readers -------------------------------------------------
# The gate polls the bank itself rather than trusting the keeper's own log for
# anything it is going to assert.

role_address() {
  python3 - "$plan/committed.json" "$1" <<'PY'
import json, sys
plan = json.load(open(sys.argv[1]))
want = sys.argv[2]
for step in plan["steps"]:
    for entry in step.get("compare", []):
        if entry["role"] == want:
            print(entry["address"])
            raise SystemExit(0)
raise SystemExit(f"no such plan role: {want}")
PY
}

# The reader is a file rather than a here-doc because it is used in a
# pipeline: `python3 - <<'PY'` would hand the interpreter the here-doc as its
# stdin and the piped account bytes would never arrive.
cat >"$work/account_field.py" <<'PY'
import base64, json, sys

offset, length, mode = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
value = json.load(sys.stdin).get("result", {}).get("value")
if value is None:
    print("ABSENT")
    raise SystemExit(0)
raw = base64.b64decode(value["data"][0])[offset:offset + length]
if mode == "base58":
    alphabet = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    number = int.from_bytes(raw, "big")
    text = b""
    while number:
        number, digit = divmod(number, 58)
        text = alphabet[digit:digit + 1] + text
    text = b"1" * len(raw[:len(raw) - len(raw.lstrip(b"\0"))]) + text
    print(text.decode())
elif mode == "u8":
    print(raw[0])
else:
    print(raw.hex())
PY

account_field() {
  # account_field <address> <offset> <len> [hex|u8|base58]
  local address="$1" offset="$2" length="$3" mode="${4:-hex}"
  curl -s -m 10 "$url" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$address\",{\"encoding\":\"base64\",\"commitment\":\"confirmed\"}]}" \
    | python3 "$work/account_field.py" "$offset" "$length" "$mode"
}

account_present() {
  local value
  value="$(account_field "$1" 0 1 hex)"
  [ "$value" != "ABSENT" ]
}

require_absent() {
  if account_present "$2"; then
    echo "FAIL: $1 is still present at $2"
    exit 1
  fi
  echo "  closed: $1"
}

wait_for_epoch_phase() {
  # EpochAccount phase lives at byte 325 of its exact 328-byte image.
  local target="$1" reason="$2" seen
  for _ in $(seq 1 2400); do
    seen="$(account_field "$epoch_account" 325 1 u8)"
    if [ "$seen" = "$target" ]; then
      return 0
    fi
    sleep 0.5
  done
  echo "FAIL: the epoch never reached phase $target ($reason); last=$seen"
  exit 1
}

wait_for_log() {
  local file="$1" needle="$2" reason="$3"
  for _ in $(seq 1 2400); do
    if grep -q -- "$needle" "$file" 2>/dev/null; then
      return 0
    fi
    if [ -n "$keeper_pid" ] && ! kill -0 "$keeper_pid" 2>/dev/null; then
      echo "FAIL: the keeper exited before $reason"
      tail -30 "$file"
      exit 1
    fi
    sleep 0.5
  done
  echo "FAIL: never observed $reason"
  tail -30 "$file"
  exit 1
}

start_validator() {
  local validator_args=(
    --ledger "$work/ledger" --reset --quiet
    --rpc-port "$rpc_port" --faucet-port "$faucet_port" --mint "${pub[payer]}"
    --bpf-program "$program_id" "$elf"
  )
  while read -r role address file; do
    [ -z "$role" ] && continue
    validator_args+=(--account "$address" "$plan/$file")
  done < "$plan/genesis.txt"
  "$test_validator" "${validator_args[@]}" >"$log/validator.log" 2>&1 &
  validator_pid=$!
  for _ in $(seq 1 120); do
    if ! kill -0 "$validator_pid" 2>/dev/null; then break; fi
    if curl -s -m 2 "$url" -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$program_id\",{\"encoding\":\"base64\"}]}" \
        2>/dev/null | grep -q '"executable":true'; then
      return 0
    fi
    sleep 0.25
  done
  echo "FAIL: local validator never exposed the executable program"
  tail -60 "$log/validator.log"
  exit 1
}

# ---------------------------------------------------------------------------
# Everything past here talks to a validator and takes minutes, so it is
# serialized behind the shared-tree suite lock.
# ---------------------------------------------------------------------------
lock=/tmp/claude-501/suite.lock
mkdir -p /tmp/claude-501
until mkdir "$lock" 2>/dev/null; do sleep 20; done
release_lock() { rmdir "$lock" 2>/dev/null || true; }
trap 'release_lock; cleanup' EXIT

gate_started=$SECONDS
echo
echo "== validator (rpc $rpc_port, faucet $faucet_port) =="
start_validator

owner_keys=()
for name in payer actor owner-b owner-c owner-d \
            owner-b-collateral-token owner-c-collateral-token owner-d-collateral-token; do
  owner_keys+=(--key "$keys/$name.json")
done

echo
echo "== prime: market, endowments, splits, sealed policy artifact (steps 1-16) =="
keeper prime --url "$url" --plan "$plan" "${owner_keys[@]}" --steps 1-16 \
  | tee "$log/prime-market.log" | tail -3

market_account="$(role_address general-market.market)"
market_id="$(account_field "$market_account" 2 32 base58)"
realm_id="$(account_field "$market_account" 34 32 base58)"
echo "market_id=$market_id"
echo "realm_id=$realm_id"

echo
echo "== keeper-derived addresses (its own, from the program's seed constants) =="
addresses="$(keeper addresses --program "$program_id" --realm "$realm_id" \
  --market "$market_id" --epoch-index 1)"
echo "$addresses"
epoch_account="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["epoch"])' <<<"$addresses")"
window_account="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["window"])' <<<"$addresses")"
page_account="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["page0"])' <<<"$addresses")"
pot_account="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["pot"])' <<<"$addresses")"
policy_artifact="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["policy_artifact"])' <<<"$addresses")"

# The keeper derived these from the program's own SEED_* constants and the
# frozen policy identity.  If they disagree with the plan's, one of the two is
# wrong and nothing below would mean anything.
for pair in "general.epoch:$epoch_account" "general.window:$window_account" \
            "general.page:$page_account" "general.pot:$pot_account" \
            "general.policy:$policy_artifact"; do
  role="${pair%%:*}"; derived="${pair#*:}"
  expected="$(role_address "$role")"
  if [ "$derived" != "$expected" ]; then
    echo "FAIL: keeper derived $role as $derived, the plan says $expected"
    exit 1
  fi
done
echo "  every keeper-derived address agrees with the plan"

keeper_common=(
  --url "$url" --program "$program_id" --realm "$realm_id" --market "$market_id"
  --epoch-index 1 --payer "$keys/keeper.json" --fund 5000000000
)

echo
echo "== keeper opens the epoch and its page (ledgered, so they are closable) =="
keeper run "${keeper_common[@]}" --open --deadline-slots 320 \
  --exit-when-idle --poll-ms 300 | tee "$log/keeper-open.log"
if ! grep -q 'action=InitEpoch .*result=accepted' "$log/keeper-open.log"; then
  echo "FAIL: the keeper did not open the epoch"
  exit 1
fi
if ! grep -q 'action=InitOrderPage .*result=accepted' "$log/keeper-open.log"; then
  echo "FAIL: the keeper did not open the order page"
  exit 1
fi

echo
echo "== prime: the mixed book (steps 21-27) =="
keeper prime --url "$url" --plan "$plan" "${owner_keys[@]}" --steps 21-27 \
  | tee "$log/prime-book.log" | tail -3

echo
echo "== keeper #1: the only driver from here (freeze onward) =="
# Launched as the binary itself, not through the `keeper` shell function: a
# function invoked in the background costs a subshell, `$!` would be that
# subshell, and the SIGKILL would orphan the keeper underneath it -- the same
# class of mistake as running it through `cargo run`, one level down.
"$keeper_bin" run "${keeper_common[@]}" --owner "$keys/actor.json" --poll-ms 700 \
  >"$log/keeper-1.log" 2>&1 &
keeper_pid=$!

wait_for_log "$log/keeper-1.log" 'action=FreezeEpoch' "the permissionless freeze"
wait_for_epoch_phase 1 "FROZEN"
echo "  epoch FROZEN by the keeper alone"

echo
echo "== prime: the solver's candidate (steps 29-32) =="
keeper prime --url "$url" --plan "$plan" "${owner_keys[@]}" --steps 29-32 \
  | tee "$log/prime-candidate.log" | tail -3

# ---------------------------------------------------------------------------
# Gate 2: the crash-safety falsifier.  SIGKILL the keeper in the middle of the
# resumable clearing walk -- after at least one pass has folded into the
# checkpoint -- and require a fresh process with no state but the chain to
# finish the lifecycle.
# ---------------------------------------------------------------------------
echo
echo "== falsifier: SIGKILL mid-clearing-walk =="
wait_for_log "$log/keeper-1.log" 'action=AdvanceClearWork' "a clearing-walk pass"
sleep 1
before_actions="$(grep -c 'result=accepted' "$log/keeper-1.log" || true)"
kill -9 "$keeper_pid" 2>/dev/null || true
wait "$keeper_pid" 2>/dev/null || true
if kill -0 "$keeper_pid" 2>/dev/null; then
  echo "FAIL: the keeper survived SIGKILL; the falsifier would be measuring two drivers"
  exit 1
fi
if pgrep -f "clutch-keeper run" >/dev/null 2>&1; then
  echo "FAIL: a keeper process is still alive after the kill"
  pgrep -lf "clutch-keeper run"
  exit 1
fi
keeper_pid=""
echo "  killed after $before_actions accepted action(s); no keeper process survives"
if [ "${before_actions:-0}" -lt 3 ]; then
  echo "FAIL: the kill landed too early to falsify anything"
  exit 1
fi
# The checkpoint must be genuinely mid-walk: created, not complete.
clear_work_status="$(account_field "$(role_address general.clear-work)" 155 1 u8)"
echo "  clear-work status at the kill: $clear_work_status (2 would mean COMPLETE)"

echo
echo "== keeper #2: a fresh process, no state but the chain =="
"$keeper_bin" run "${keeper_common[@]}" --owner "$keys/actor.json" --poll-ms 700 \
  --exit-when-blocked >"$log/keeper-2.log" 2>&1 &
keeper_pid=$!
wait_for_log "$log/keeper-2.log" 'keeper stop reason=lifecycle-complete' \
  "the restarted keeper finishing the lifecycle"
wait "$keeper_pid" 2>/dev/null || true
keeper_pid=""
walk_seconds=$((SECONDS - gate_started))

echo
echo "== what the two keeper processes actually drove =="
cat "$log/keeper-1.log" "$log/keeper-2.log" >"$log/keeper-all.log"
grep 'result=' "$log/keeper-all.log" | sed 's/^/  /'

echo
echo "== the action set: every permissionless step must appear =="
# `CloseGeneralCandidate` is deliberately NOT here.  The candidate pair comes
# from the plan's `SubmitCandidate`, which creates it WITHOUT the optional
# funding ledger, so it records no payer and its close refuses forever.  That
# is the ratified `RENT.ACCOUNT_REFUND_UNOWNED` tolerance, and it is asserted
# as a recorded residual below rather than papered over here.
# `CompleteClearWork` is deliberately NOT here either, and for a reason worth
# stating: the SIGKILL can land between a transaction's confirmation and the
# keeper's log line for it.  That IS the crash this gate injects, so demanding
# a log line for the step nearest the kill would be demanding that the crash
# not happen.  Its evidence is asserted on chain below instead.
required=(
  FreezeEpoch InitClearWork+Grow AdvanceClearWork AdvanceClearSlices
  FinalizeSelection FreezeEntitlement EntitleSlice
  SettlePage ReleaseTerminalReservation CloseGeneralReceipt
  CloseGeneralPage CloseGeneralReservation CloseGeneralPot
  CloseGeneralClearWork CloseGeneralEpoch
)
missing=0
for action in "${required[@]}"; do
  if grep -q "action=$action " "$log/keeper-all.log"; then
    echo "  drove $action"
  else
    echo "  MISSING $action"
    missing=1
  fi
done
if [ "$missing" -ne 0 ]; then
  echo "FAIL: the keeper did not drive every permissionless step"
  exit 1
fi
# Exactly one route in the family is owner-signed, and it must be the only one
# the keeper claims as such.
owner_signed="$(grep -c 'authority=owner-signed' "$log/keeper-all.log" || true)"
if [ "$owner_signed" -ne 1 ]; then
  echo "FAIL: expected exactly one owner-signed action, saw $owner_signed"
  exit 1
fi
echo "  authority split: 1 owner-signed (tag 60), the rest permissionless"

echo
echo "== the epoch's machinery is closed =="
require_absent "epoch" "$epoch_account"
require_absent "window" "$window_account"
require_absent "order page" "$page_account"
require_absent "final pot" "$pot_account"
require_absent "clear work" "$(role_address general.clear-work)"
for slice in 0 1 2; do
  require_absent "receipt $slice" "$(role_address "general.receipt-$slice")"
done
for rank in 1 2 3 4 5 6; do
  require_absent "reservation $rank" "$(role_address "general.reservation-$rank")"
done

echo
echo "== CompleteClearWork, evidenced on chain rather than in a log =="
# `CandidateRecord::status` is the third byte from the end of its 337-byte
# image.  Only `CompleteClearWork` ever writes VERIFIED, and only
# `FinalizeSelection` promotes VERIFIED to SELECTED (2), so a SELECTED record
# is proof both ran -- whichever keeper process happened to emit them, and
# whether or not the kill ate the log line.
candidate_status="$(account_field "$(role_address general.candidate)" 334 1 u8)"
if [ "$candidate_status" != "2" ]; then
  echo "FAIL: the candidate record is status $candidate_status, not SELECTED(2);"
  echo "      CompleteClearWork and FinalizeSelection did not both land"
  exit 1
fi
echo "  candidate record is SELECTED(2): CompleteClearWork ran and selection promoted it"
if grep -q 'action=CompleteClearWork ' "$log/keeper-all.log"; then
  echo "  (its log line survived the kill this run)"
else
  echo "  (its log line did not survive the kill this run, which is the crash itself)"
fi

echo
echo "== the recorded residual: the unledgered candidate pair =="
# The plan's SubmitCandidate created the record and feed without the optional
# funding ledger, so no payer is recorded and no payer is ever guessed.  The
# keeper must leave the pair standing AND the root must still have closed past
# it.  Both halves are the assertion; either one alone would be worthless.
candidate_record="$(role_address general.candidate)"
if ! account_present "$candidate_record"; then
  echo "FAIL: the unledgered candidate record was closed; no payer could have been known"
  exit 1
fi
if grep -q 'action=CloseGeneralCandidate' "$log/keeper-all.log"; then
  echo "FAIL: the keeper attempted a close that records no payer"
  exit 1
fi
echo "  candidate record stands at $candidate_record (RENT.ACCOUNT_REFUND_UNOWNED)"
echo "  the epoch root closed past it, which is the recorded tolerance"

echo
echo "== conservation, re-derived from the FINAL committed bytes =="
python3 - "$plan/committed.json" "$url" <<'PY' | tee "$log/conservation.log"
import base64, json, subprocess, sys

plan = json.load(open(sys.argv[1]))
url = sys.argv[2]
rules = plan["conservation"]
offsets = rules["offsets"]

roles = {}
for step in plan["steps"]:
    for entry in step.get("compare", []):
        roles.setdefault(entry["role"], entry["address"])

def account(address):
    body = json.dumps({
        "jsonrpc": "2.0", "id": 1, "method": "getAccountInfo",
        "params": [address, {"encoding": "base64", "commitment": "confirmed"}],
    })
    raw = subprocess.run(
        ["curl", "-fsS", "--max-time", "30", "-H", "Content-Type: application/json",
         "-X", "POST", "--data-binary", body, url],
        capture_output=True, check=True,
    ).stdout
    value = json.loads(raw)["result"]["value"]
    if value is None:
        raise SystemExit(f"account {address} is absent at the epilogue")
    return base64.b64decode(value["data"][0])

def u64(data, offset):
    return int.from_bytes(data[offset:offset + 8], "little")

cash_total = 0
eggs = [0, 0]
for entry in rules["positions"]:
    data = account(roles[entry["role"]])
    cash = u64(data, offsets["position_cash"])
    reserved = u64(data, offsets["position_reserved"])
    egg0 = u64(data, offsets["position_internal0"])
    egg1 = u64(data, offsets["position_internal1"])
    cash_total += cash
    eggs[0] += egg0
    eggs[1] += egg1
    print(f"  {entry['role']}: cash={cash} reserved={reserved} eggs=[{egg0}, {egg1}]")
hoard = u64(account(roles[rules["hoard"]["role"]]), offsets["hoard_collateral"])
custody = u64(account(roles[rules["hoard_token"]["role"]]), offsets["token_amount"])
print(f"  hoard locked backing = {hoard}")
print(f"  pooled custody token = {custody}")

expected = rules["expected"]
failed = False
for label, got, want in [
    ("position cash total", cash_total, expected["cash_total"]),
    ("eggs outcome 0", eggs[0], expected["eggs_outcome0"]),
    ("eggs outcome 1", eggs[1], expected["eggs_outcome1"]),
    ("locked backing", hoard, expected["locked"]),
    ("pooled custody", custody, expected["custody"]),
    ("identity cash_total + locked == endowed_total",
     cash_total + hoard, rules["endowed_total"]),
    ("identity eggs[0] == split_total", eggs[0], rules["split_total"]),
    ("identity eggs[1] == split_total", eggs[1], rules["split_total"]),
    ("identity custody == endowed_total", custody, rules["endowed_total"]),
]:
    verdict = "ok" if got == want else "FAIL"
    failed = failed or got != want
    print(f"  {label}: observed={got} expected={want} {verdict}")
# Every reservation of the epoch must be gone: released or consumed, then
# archived and closed.  The keeper drove all of it.
sys.exit(1 if failed else 0)
PY

# ---------------------------------------------------------------------------
# Gate 3: the batched-fold packet answer, against this validator's transport.
# ---------------------------------------------------------------------------
echo
echo "== fold-batch wire answer (the UNMODELED_BANK_TRANSPORT_ONLY discharge) =="
terms_digest="$(account_field "$market_account" 98 32 base58)"
keeper fold-wire-probe --url "$url" --program "$program_id" \
  --realm "$realm_id" --market "$market_id" \
  --feed "$market_id" --window "$market_id" --terms "$terms_digest" \
  --widths 1,2,4,6,7,8,12 | tee "$log/fold-wire.log"

fitting="$(grep -o 'largest_fitting_folds=[0-9]*' "$log/fold-wire.log" | cut -d= -f2)"
if [ -z "$fitting" ] || [ "$fitting" -lt 1 ]; then
  echo "FAIL: the fold-wire probe produced no answer"
  exit 1
fi
# The measurement and the validator must agree: every width the serializer
# calls framed must be admitted by transport, and every width it calls
# over-budget must be refused by transport before execution.
python3 - "$log/fold-wire.log" <<'PY'
import re, sys
bad = 0
for line in open(sys.argv[1]):
    row = dict(re.findall(r"(\w+)=(\S+)", line))
    if "folds" not in row or "transport" not in row:
        continue
    fits = row["fits_packet"] == "true"
    admitted = row["transport"] == "admitted"
    if fits != admitted:
        print(f"  DISAGREEMENT at folds={row['folds']}: "
              f"fits_packet={row['fits_packet']} transport={row['transport']}")
        bad += 1
    else:
        print(f"  folds={row['folds']} bytes={row['bytes']} "
              f"serializer={'fits' if fits else 'over'} transport={row['transport']} agree")
if bad:
    print("FAIL: the serializer and the real transport disagree")
    sys.exit(1)
PY

stop_validator
release_lock
trap cleanup EXIT

echo
echo "keeper_gate=PASS"
echo "driver=CLUTCH_KEEPER_ONLY"
echo "permissionless_actions=$(grep -c 'authority=permissionless' "$log/keeper-all.log")"
echo "owner_signed_actions=$owner_signed"
echo "restart_falsifier=PASS_KILLED_AFTER_${before_actions}_ACTIONS_RESUMED_FROM_CHAIN"
echo "conservation=RE-DERIVED-FROM-FINAL-COMMITTED-BYTES"
echo "close_dag=EPOCH_WINDOW_PAGE_POT_CLEARWORK_ALL_ABSENT"
echo "fold_wire_largest_fitting_folds=$fitting"
echo "$(grep -m1 'fold_wire_plan' "$log/fold-wire.log")"
echo "sbf_elf_sha256=$elf_sha256"
echo "gate_wall_seconds=$walk_seconds"
echo "rpc_port=$rpc_port faucet_port=$faucet_port"
echo "work_dir=$work"
