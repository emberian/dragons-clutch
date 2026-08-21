#!/usr/bin/env bash
# The Operator Bench M0 gate: the whole general-clearing walk, watched through
# the daemon's own API rather than through its stdout.
#
# `operatord serve` reproduces `run_general_committed.sh`'s prologue in
# process -- fresh test-only keys, the NON-PRODUCTION mock-source ELF, the
# plan emitted by the repository's own builders, a fresh local ledger with the
# genesis accounts installed, the same readiness probe -- and then drives the
# forty-four signed transactions, publishing every transition to
# `/api/events`.
#
# This gate attaches to that stream as a client, exactly as the browser does,
# and passes only if the stream itself shows: forty-four steps resolved, the
# three refusals refused, the conservation identities re-derived from the
# runner's observed bytes, and a PASS verdict.  Nothing is read from the
# daemon's console.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

http_port="${CLUTCH_OPERATOR_PORT:-9130}"
rpc_port="${CLUTCH_OPERATOR_RPC_PORT:-9137}"
faucet_port="${CLUTCH_OPERATOR_FAUCET_PORT:-9138}"

work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-operator-bench.XXXXXX")"
events="$work/events.ndjson"
daemon_pid=""
watcher_pid=""

cleanup() {
  [ -n "$watcher_pid" ] && kill "$watcher_pid" 2>/dev/null || true
  [ -n "$daemon_pid" ] && kill "$daemon_pid" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "== source =="
(cd "$root/../.." && git rev-parse HEAD)
echo "operator_http_port=$http_port"
echo "operator_rpc_port=$rpc_port"
echo "operator_faucet_port=$faucet_port"

echo
echo "== operatord serve (M0 watch mode) =="
CARGO_NET_OFFLINE=true cargo run --offline --quiet \
  --manifest-path "$root/operatord/Cargo.toml" -- serve \
  --port "$http_port" --rpc-port "$rpc_port" --faucet-port "$faucet_port" \
  --work "$work/bench" --exit-when-done >"$work/daemon.log" 2>&1 &
daemon_pid=$!

ready=0
for _ in $(seq 1 900); do
  if ! kill -0 "$daemon_pid" 2>/dev/null; then
    break
  fi
  if curl -fsS -m 2 -o /dev/null "127.0.0.1:$http_port/"; then
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

curl -sN -m 1800 "127.0.0.1:$http_port/api/events" \
  | sed -u -n 's/^data: //p' >"$events" &
watcher_pid=$!

set +e
wait "$daemon_pid"
daemon_status=$?
set -e
daemon_pid=""
sleep 1
kill "$watcher_pid" 2>/dev/null || true
watcher_pid=""

echo
echo "== the walk, as the API reported it =="
python3 - "$events" <<'PY'
import json, sys

events = []
for line in open(sys.argv[1]):
    line = line.strip()
    if not line:
        continue
    try:
        events.append(json.loads(line))
    except json.JSONDecodeError:
        pass

kinds = {}
for event in events:
    kinds[event.get("type")] = kinds.get(event.get("type"), 0) + 1
print("  event types:", ", ".join(f"{name}={count}" for name, count in sorted(kinds.items())))

identity = next((e for e in events if e["type"] == "identity"), None)
plan = next((e for e in events if e["type"] == "plan"), None)
done = next((e for e in events if e["type"] == "done"), None)
terminal = [e for e in events if e["type"] == "conservation" and not e.get("live")]
faults = [e for e in events if e["type"] == "fault"]

failed = []
if identity is None:
    failed.append("no identity event")
else:
    print(f"  source_profile={identity['source_profile']}")
    print(f"  sbf_elf_sha256={identity['elf_sha256']}")
    print(f"  sbf_elf_bytes={identity['elf_bytes']}")
    print(f"  genesis_assisted={identity['genesis_assisted']} precreated={len(identity['precreated'])}")
    if "non-production-mock-source" not in identity["source_profile"]:
        failed.append("the banner did not name the non-production mock-source profile")
    if identity["evidence_scope"] != "SBF_EXECUTED" or identity["promotion"] != "unpromoted":
        failed.append("the banner did not carry the unpromoted SBF-EXECUTED scope")

if plan is None:
    failed.append("no plan event")
else:
    declared = len(plan["steps"])
    print(f"  declared_steps={declared} compute_unit_ceiling={plan['compute_unit_ceiling']}")

resolved = {}
for event in events:
    if event["type"] == "step" and event.get("state") in ("accepted", "refused"):
        resolved[event["ordinal"]] = event
accepted = sum(1 for e in resolved.values() if e["state"] == "accepted")
refused = sum(1 for e in resolved.values() if e["state"] == "refused")
print(f"  resolved_steps={len(resolved)} accepted={accepted} refused_as_expected={refused}")
reported = [e for e in resolved.values() if e.get("cu") is not None]
if reported:
    peak = max(reported, key=lambda e: e["cu"])
    print(f"  peak_compute_units={peak['cu']} at {peak['name']}")
if plan is not None and len(resolved) != len(plan["steps"]):
    failed.append(f"only {len(resolved)} of {len(plan['steps'])} steps resolved")
if refused != 3:
    failed.append(f"expected 3 refusals in the stream, saw {refused}")

states = [e for e in events if e["type"] == "state"]
print(f"  account_reloads_published={len(states)}")
decoded = sum(1 for e in states if e["decoded"].get("kind") != "opaque")
print(f"  decoded_through_layout_codecs={decoded}")

clocks = [e for e in events if e["type"] == "clock"]
reasons = sorted({e["reason"] for e in clocks})
print(f"  clock_ticks={len(clocks)} waits={reasons}")
if len(reasons) != 2:
    failed.append(f"expected two real-clock waits, saw {reasons}")

if not terminal:
    failed.append("no terminal conservation event")
else:
    strip = terminal[-1]
    print(f"  conservation cash_total={strip['cash_total']} eggs={strip['eggs']} "
          f"locked={strip['locked']} custody={strip['custody']}")
    for entry in strip["checks"] + strip["identities"]:
        verdict = "ok" if entry["ok"] else "FAIL"
        print(f"    {entry['label']}: observed={entry['observed']} expected={entry['expected']} {verdict}")
        if not entry["ok"]:
            failed.append(f"conservation: {entry['label']}")
    if strip["verdict"] != "RE-DERIVED-FROM-OBSERVED-BYTES":
        failed.append("conservation verdict is not re-derived-from-observed-bytes")

for fault in faults:
    failed.append(f"fault: {fault['text']}")
if done is None:
    failed.append("no done event")
elif done["verdict"] != "PASS":
    failed.append(f"verdict {done['verdict']}")

print()
for reason in failed:
    print(f"  FAIL {reason}")
print("operator_bench_steps_resolved=%d" % len(resolved))
print("operator_bench_reloads_published=%d" % len(states))
print("operator_bench=%s" % ("PASS" if not failed else "FAIL"))
sys.exit(1 if failed else 0)
PY

if [ "$daemon_status" -ne 0 ]; then
  echo "FAIL: the daemon exited $daemon_status"
  tail -40 "$work/daemon.log"
  exit 1
fi
echo "operator_bench_daemon_exit=0"
echo "work_dir=$work"
