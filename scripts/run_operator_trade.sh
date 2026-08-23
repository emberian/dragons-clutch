#!/usr/bin/env bash
# The Operator Bench M1 gate: a person creates the Friday clutch and trades it
# against the automaton, end to end, driven entirely through the HTTP API.
#
# Nothing here is pregenerated.  `operatord serve --mode trade` boots a fresh
# local validator with the Friday clutch's frozen prerequisites installed,
# founds the eight-outcome degree-1 market with a signed `CreateMarket`, funds
# both actors with signed `Endow` and `Split`, opens the epoch, and lets the
# fixed-belief automaton rest its opening book.  This script then does exactly
# what a person at the keyboard does -- POST an intent, read the stream -- and
# passes only if the stream shows the epoch reaching `settled` with the value
# plane conserved.
#
# It is a *client*.  It never reads the daemon's console, never touches the
# ledger, and never builds a transaction: every byte the bank sees was built by
# `clutch_sbf_harness::general_transaction`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

http_port="${CLUTCH_OPERATOR_TRADE_PORT:-9560}"
rpc_port="${CLUTCH_OPERATOR_TRADE_RPC_PORT:-9567}"
# Agave reserves rpc_port + 1 (9568) for RPC WebSocket.
faucet_port="${CLUTCH_OPERATOR_TRADE_FAUCET_PORT:-9569}"
gossip_port="${CLUTCH_OPERATOR_TRADE_GOSSIP_PORT:-9570}"
dynamic_port_range="${CLUTCH_OPERATOR_TRADE_DYNAMIC_PORT_RANGE:-9571-9620}"
# Slots between the epoch opening and its freeze deadline.  Long enough for the
# scripted flow below to place its book on a real clock, short enough that the
# gate is not dominated by waiting for a deadline nobody is using.
freeze_window="${CLUTCH_OPERATOR_FREEZE_WINDOW:-260}"

work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-operator-trade.XXXXXX")"
events="$work/events.sse"
cookie_jar="$work/operator.cookies"
daemon_pid=""
watcher_pid=""

cleanup() {
  [ -n "$watcher_pid" ] && kill "$watcher_pid" 2>/dev/null || true
  if [ -n "$daemon_pid" ]; then
    kill "$daemon_pid" 2>/dev/null || true
    for _ in $(seq 1 20); do
      kill -0 "$daemon_pid" 2>/dev/null || break
      sleep 0.5
    done
    kill -9 "$daemon_pid" 2>/dev/null || true
  fi
  pkill -f "solana-test-validator.*--rpc-port $rpc_port" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

# One action, posted the way the page posts it.
api() {
  curl -fsS -m 120 -b "$cookie_jar" -H 'Content-Type: application/json' \
    --data-binary "$1" "127.0.0.1:$http_port/api"
}

phase() {
  api '{"action":"status"}' | python3 -c 'import json,sys; print(json.load(sys.stdin).get("phase",""))'
}

echo "== source =="
(cd "$root" && git rev-parse HEAD)
echo "operator_http_port=$http_port"
echo "operator_rpc_port=$rpc_port"
echo "operator_rpc_websocket_port=$((rpc_port + 1))"
echo "operator_faucet_port=$faucet_port"
echo "operator_gossip_port=$gossip_port"
echo "operator_dynamic_port_range=$dynamic_port_range"
echo "freeze_window_slots=$freeze_window"

echo
echo "== no external reference in anything the bench serves =="
# The daemon serves an allowlisted extension set; those files, and only those,
# are what a browser fetches.  A URL literal in any of them would mean the page
# depends on something other than this machine, which is the one thing the
# Bench's zero-dependency rule exists to prevent.  The README is prose about
# this gate and is deliberately not served.
served=$(find "$root/apps/operator" -type f \
  \( -name '*.html' -o -name '*.css' -o -name '*.js' -o -name '*.svg' -o -name '*.json' \))
if grep -nE 'https?:|ftp:|cdn\.|unpkg|jsdelivr|googleapis|integrity=|crossorigin' $served; then
  echo "FAIL: a served file names an off-machine address"
  exit 1
fi
echo "  $(echo "$served" | wc -l | tr -d ' ') served files, no external reference"

echo
echo "== build =="
CARGO_NET_OFFLINE=true cargo build --offline --quiet \
  --manifest-path "$root/programs/clutch-sbf/operatord/Cargo.toml"
daemon="$root/programs/clutch-sbf/operatord/target/debug/clutch-sbf-operatord"
[ -x "$daemon" ] || { echo "FAIL: $daemon is not executable"; exit 1; }

echo
echo "== operatord serve --mode trade =="
"$daemon" serve --mode trade \
  --port "$http_port" --rpc-port "$rpc_port" --faucet-port "$faucet_port" \
  --gossip-port "$gossip_port" --dynamic-port-range "$dynamic_port_range" \
  --work "$work/bench" --freeze-window "$freeze_window" --exit-when-done \
  >"$work/daemon.log" 2>&1 &
daemon_pid=$!

ready=0
for _ in $(seq 1 900); do
  kill -0 "$daemon_pid" 2>/dev/null || break
  if curl -fsS -m 2 -c "$cookie_jar" -o /dev/null \
    "127.0.0.1:$http_port/" 2>/dev/null; then
    ready=1
    break
  fi
  sleep 1
done
if [ "$ready" -ne 1 ]; then
  echo "FAIL: the bench never served its index"
  tail -40 "$work/daemon.log"
  exit 1
fi
echo "  bench is serving; attaching to /api/events as a client"
curl -sN -m 3600 -b "$cookie_jar" \
  "127.0.0.1:$http_port/api/events" >"$events" &
watcher_pid=$!

# The market has to be founded and the automaton's book rested before a person
# can trade against it.  Founding is a real signed sequence on a real bank, so
# this waits on it rather than assuming it.
open=0
for _ in $(seq 1 300); do
  kill -0 "$daemon_pid" 2>/dev/null || break
  if [ "$(phase 2>/dev/null || true)" = "open" ]; then
    open=1
    break
  fi
  sleep 2
done
if [ "$open" -ne 1 ]; then
  echo "FAIL: the session never opened"
  tail -40 "$work/daemon.log"
  exit 1
fi
echo "  the clutch is open"

echo
echo "== the automaton, as it discloses itself =="
api '{"action":"bot"}' | python3 -m json.tool

echo
echo "== funding, on demand, the way the Funding tab does it =="
# The founding sequence already endowed and split both actors; these are the
# same two transitions posted as intents, which is what makes them a control
# rather than a script step.
for fund in \
  '{"action":"endow","amount":5000}' \
  '{"action":"split","quantity":1000}'
do
  echo "  POST $fund"
  api "$fund" | python3 -c 'import json,sys; r=json.load(sys.stdin); print("    ->", "ok" if r.get("ok") else "REFUSED "+str(r.get("detail")))'
done

echo
echo "== three orders, placed the way the ticket places them =="
# Each crosses one of the automaton's resting quotes: a sell into its bid at
# the $160 knot, and two buys lifting its offers at the $120 and $200 knots.
for order in \
  '{"action":"place","outcome":3,"side":"sell","quantity":500,"limit":5800}' \
  '{"action":"place","outcome":1,"side":"buy","quantity":500,"limit":400}' \
  '{"action":"place","outcome":5,"side":"buy","quantity":500,"limit":200}'
do
  echo "  POST $order"
  api "$order" | python3 -c 'import json,sys; r=json.load(sys.stdin); print("    ->", "ok" if r.get("ok") else "REFUSED "+str(r.get("detail")))'
done

echo
echo "== the painted belief =="
belief='[200,400,1500,3300,2700,1300,400,200]'
echo "  POST propose $belief"
api "{\"action\":\"propose\",\"belief\":$belief}" | python3 -m json.tool
echo "  POST paint $belief"
api "{\"action\":\"paint\",\"belief\":$belief}" \
  | python3 -c 'import json,sys; r=json.load(sys.stdin); print("    -> placed", len(r.get("placed",[])), "orders,", len(r.get("skipped",[])), "skipped")'

echo
echo "== the resolution weights the painter previews =="
api '{"action":"weights","cents":16340}' | python3 -m json.tool

echo
echo "== freeze, crank, settle =="
api '{"action":"freeze"}' | python3 -c 'import json,sys; print("   ", json.load(sys.stdin).get("detail"))'

set +e
wait "$daemon_pid"
daemon_status=$?
set -e
daemon_pid=""
sleep 1
kill "$watcher_pid" 2>/dev/null || true
watcher_pid=""

echo
echo "== the session, as the API reported it =="
python3 - "$events" <<'PY'
import json, sys

events = []
for line in open(sys.argv[1]):
    if not line.startswith("data: "):
        continue
    try:
        events.append(json.loads(line[6:].strip()))
    except json.JSONDecodeError:
        pass

kinds = {}
for event in events:
    kinds[event.get("type")] = kinds.get(event.get("type"), 0) + 1
print("  event types:", ", ".join(f"{k}={v}" for k, v in sorted(kinds.items())))

failed = []
def exact_int(value, label):
    if isinstance(value, bool):
        failed.append(f"{label} is a boolean, not an exact integer")
        return 0
    if isinstance(value, int):
        return value
    if isinstance(value, str) and (value == "0" or (value.isascii() and value.isdigit() and not value.startswith("0"))):
        return int(value)
    failed.append(f"{label} is not a canonical decimal integer")
    return 0

def exact_sum(values, label):
    return sum(exact_int(value, f"{label}[{index}]") for index, value in enumerate(values))

identity = next((e for e in events if e["type"] == "identity"), None)
market = next((e for e in events if e["type"] == "market"), None)
bot = next((e for e in events if e["type"] == "bot"), None)
belief = next((e for e in events if e["type"] == "belief"), None)
candidate_plan = next((e for e in events if e["type"] == "candidate-plan"), None)
done = next((e for e in events if e["type"] == "done"), None)
faults = [e for e in events if e["type"] == "fault"]
sessions = [e for e in events if e["type"] == "session"]
conservations = [e for e in events if e["type"] == "conservation"]

if identity is None:
    failed.append("no identity event")
else:
    print(f"  source_profile={identity['source_profile']}")
    print(f"  sbf_elf_sha256={identity['elf_sha256']}")
    print(f"  genesis_assisted={identity['genesis_assisted']} precreated={len(identity['precreated'])}")
    if "non-production-mock-source" not in identity["source_profile"]:
        failed.append("the banner did not name the non-production mock-source profile")
    if identity["evidence_scope"] != "SBF_EXECUTED" or identity["promotion"] != "unpromoted":
        failed.append("the banner did not carry the unpromoted SBF-EXECUTED scope")

if market is None:
    failed.append("no market identity event")
else:
    m = market["identity"]
    print(f"  market outcomes={m['outcome_count']} basis_degree={m['basis_degree']} "
          f"knots={m['knot_count']} price_scale={m['price_scale']} ladder_step={m['ladder_step']}")
    if m["basis_degree"] != 1 or m["outcome_count"] != 8 or m["knot_count"] != 8:
        failed.append("the founded market is not the eight-outcome degree-1 clutch")

if bot is None:
    failed.append("no automaton disclosure")
else:
    d = bot["disclosure"]
    print(f"  automaton kind={d['kind']!r} belief={d['belief']} quoted={d['quoted_belief']}")
    if d["kind"] != "fixed-belief automaton":
        failed.append("the opponent is not labelled a fixed-belief automaton")
    if exact_sum(d["belief"], "automaton belief") != 10000:
        failed.append("the automaton's published belief is not a price vector")

if belief is None:
    failed.append("no painted belief event")
else:
    print(f"  painted belief={belief['belief']} -> {len(belief['proposed'])} proposed orders")
    if belief.get("label") != "MODEL-ONLY":
        failed.append("the painted belief was not labelled MODEL-ONLY")
    if exact_sum(belief["belief"], "painted belief") != 10000:
        failed.append("the painted belief is not a price vector")

steps = [e for e in events if e["type"] == "step" and e.get("state") in ("accepted", "refused")]
accepted = [e for e in steps if e["state"] == "accepted"]
refused = [e for e in steps if e["state"] == "refused"]
print(f"  transactions={len(steps)} accepted={len(accepted)} refused={len(refused)}")
for step in refused:
    print(f"    REFUSED {step['name']}: {step.get('refusal_code')} {step.get('error')}")
    failed.append(f"refused: {step['name']}")
with_cu = [e for e in accepted if e.get("cu") is not None]
if with_cu:
    peak = max(with_cu, key=lambda e: exact_int(e["cu"], "compute units"))
    print(f"  peak_compute_units={peak['cu']} at {peak['name']}")
    print("  every submitted transaction carries its compute units: "
          f"{len(with_cu)}/{len(accepted)}")
if len(with_cu) != len(accepted):
    failed.append("some accepted transactions reported no compute units")

funding = [e for e in accepted if e["family"] == "Funding"]
print(f"  funding_transactions={len(funding)} ({', '.join(e['name'] for e in funding)})")
if len(funding) < 6:
    failed.append("expected four founding fundings plus the two posted as intents")

places = [e for e in accepted if e["family"] == "PlaceOrder"]
print(f"  orders_placed={len(places)}")
if len(places) < 14:
    failed.append(f"expected the automaton's eight quotes plus the person's book, saw {len(places)}")

attempts = [e for e in events if e["type"] == "candidate-trial"]
for attempt in attempts:
    verdict = "TAKEN" if attempt["taken"] else f"refused ({attempt['refusal']})"
    print(f"  price coordinate {attempt['basis']!r}: {attempt['prices']} {verdict}")
    if attempt.get("schema") != "dragons-clutch/operator/candidate-trial/v1":
        failed.append("candidate trial has an unknown schema")
if not any(a["taken"] for a in attempts):
    failed.append("no stated price coordinate was admitted")

if candidate_plan is None:
    failed.append("no candidate-plan event")
else:
    print(f"  pre-submit candidate plan={candidate_plan['prices']} ({candidate_plan['price_basis']})")
    print(f"  model fills={candidate_plan['fills']} slices={candidate_plan['slices']}")
    if candidate_plan.get("schema") != "dragons-clutch/operator/candidate-plan/v1":
        failed.append("candidate plan has an unknown schema")
    if exact_sum(candidate_plan["prices"], "candidate-plan prices") != 10000:
        failed.append("the candidate-plan vector is not on the price simplex")
    if exact_int(candidate_plan["slices"], "candidate-plan slices") == 0:
        failed.append("the candidate plan paired nothing")
if any(e.get("type") in ("clearing", "clearing-attempt") for e in events):
    failed.append("legacy pre-submit clearing event vocabulary was emitted")

snapshots = [e for e in events if e["type"] == "account-snapshot-v2"]
states = [state for snapshot in snapshots for state in snapshot.get("states", [])]
decoded = sum(
    1 for e in states
    if isinstance(e.get("decoded"), dict) and e["decoded"].get("kind") != "opaque"
)
print(f"  account_reloads_published={len(states)} decoded_through_layout_codecs={decoded}")
print(f"  graph_snapshot_v2_batches={len(snapshots)}")
if not snapshots:
    failed.append("no graph-root-bracketed snapshot V2 event")
for snapshot in snapshots:
    if snapshot.get("schema") != "dragons-clutch/operator/graph-root-bracketed-account-snapshot/v2":
        failed.append("graph snapshot has an unknown schema")
    batch = snapshot.get("states")
    if not isinstance(batch, list):
        failed.append("graph snapshot has no complete state array")
        continue
    if exact_int(snapshot.get("account_count"), "snapshot account_count") != len(batch):
        failed.append("graph snapshot account_count does not match its state array")
    roles = [state.get("role") for state in batch]
    if len(roles) != len(set(roles)):
        failed.append("graph snapshot contains duplicate roles")
    for state in batch:
        if state.get("snapshot_schema") != snapshot.get("schema"):
            failed.append(f"state {state.get('role')} is not snapshot-V2-bound")
        if state.get("context_slot") != snapshot.get("context_slot"):
            failed.append(f"state {state.get('role')} does not share the snapshot context")
        if state.get("ordinal") != snapshot.get("ordinal"):
            failed.append(f"state {state.get('role')} does not share the snapshot ordinal")
        if state.get("present") is False:
            if state.get("decoded") is not None:
                failed.append(f"absent state {state.get('role')} retained decoded data")
        elif not (
            state.get("present") is True
            and isinstance(state.get("owner"), str)
            and state.get("executable") is False
            and isinstance(state.get("account_schema"), dict)
        ):
            failed.append(f"present state {state.get('role')} lacks its validated envelope")
if any(e.get("type") in ("snapshot-v2", "state") for e in events):
    failed.append("legacy sequential snapshot events were emitted")

phases = [e["phase"] for e in sessions]
print(f"  phases={sorted(set(phases))}")
if "settled" not in phases:
    failed.append("the epoch never reached settled")

terminal = [c for c in conservations if c.get("identities")]
if not terminal:
    failed.append("no conservation strip with identities")
else:
    strip = terminal[-1]
    print(f"  conservation cash_total={strip['cash_total']} reserved={strip['reserved_total']} "
          f"locked={strip['locked']} custody={strip['custody']}")
    print(f"  eggs={strip['eggs']}")
    for entry in strip["identities"]:
        verdict = "ok" if entry["ok"] else "FAIL"
        print(f"    {entry['label']}: observed={entry['observed']} "
              f"expected={entry['expected']} {verdict}")
        if not entry["ok"]:
            failed.append(f"conservation: {entry['label']}")

for fault in faults:
    failed.append(f"fault: {fault['text']}")
if done is None:
    failed.append("no done event")
elif done["verdict"] != "SETTLED":
    failed.append(f"verdict {done['verdict']}")

print()
for reason in failed:
    print(f"  FAIL {reason}")
print("operator_trade_transactions=%d" % len(steps))
print("operator_trade_orders=%d" % len(places))
print("operator_trade_reloads=%d" % len(states))
print("operator_trade=%s" % ("PASS" if not failed else "FAIL"))
sys.exit(1 if failed else 0)
PY

if [ "$daemon_status" -ne 0 ]; then
  echo "FAIL: the daemon exited $daemon_status"
  tail -40 "$work/daemon.log"
  exit 1
fi
echo "operator_trade_daemon_exit=0"
echo "work_dir=$work"
