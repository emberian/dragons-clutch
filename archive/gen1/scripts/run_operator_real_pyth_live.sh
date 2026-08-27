#!/usr/bin/env bash
# Start the Operator's live, unretained two-boundary Pyth laboratory.
#
# The daemon supervises the tracked local-real runner. That child deploys the
# exact captured Pyth receiver/router Program and ProgramData accounts into a
# fresh loopback validator, submits the signed SourceV2 + settled-trade
# lifecycle. The daemon owns the child's private session directory, signers,
# validator hold/stop contract, and exact cleanup. The browser is only a live
# telemetry reader; it receives no key material and has no campaign verb.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

http_port="${CLUTCH_OPERATOR_PYTH_LIVE_PORT:-9130}"
rpc_port="${CLUTCH_OPERATOR_PYTH_LIVE_RPC_PORT:-9137}"
faucet_port="${CLUTCH_OPERATOR_PYTH_LIVE_FAUCET_PORT:-9139}"
gossip_port="${CLUTCH_OPERATOR_PYTH_LIVE_GOSSIP_PORT:-9200}"
dynamic_port_range="${CLUTCH_OPERATOR_PYTH_LIVE_DYNAMIC_PORT_RANGE:-9201-9250}"
exit_when_done="${CLUTCH_OPERATOR_PYTH_LIVE_EXIT_WHEN_DONE:-0}"
work_base="${CLUTCH_OPERATOR_PYTH_LIVE_WORK_BASE:-}"
daemon_pid=""

cleanup() {
  if [ -n "$daemon_pid" ] && kill -0 "$daemon_pid" 2>/dev/null; then
    # Ask the direct campaign child to run its own TERM cleanup before the
    # supervisor goes away. This is deliberately PID-scoped, never a broad
    # validator/process-name kill.
    for child_pid in $(pgrep -P "$daemon_pid" 2>/dev/null || true); do
      kill -TERM "$child_pid" 2>/dev/null || true
    done
    # The daemon observes child exit and drops only its marked private session
    # root. Give that owner cleanup a bounded chance before forcing it down.
    for _ in $(seq 1 50); do
      kill -0 "$daemon_pid" 2>/dev/null || break
      sleep 0.1
    done
    kill -TERM "$daemon_pid" 2>/dev/null || true
    for _ in $(seq 1 20); do
      kill -0 "$daemon_pid" 2>/dev/null || break
      sleep 0.1
    done
    kill -KILL "$daemon_pid" 2>/dev/null || true
  fi
  wait "$daemon_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

case "$exit_when_done" in
  0|1) ;;
  *) echo "FAIL: CLUTCH_OPERATOR_PYTH_LIVE_EXIT_WHEN_DONE must be 0 or 1" >&2; exit 1 ;;
esac

# A caller cannot turn this live surface into a retained-transcript reader or
# writer through an inherited campaign variable.
unset CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR

CARGO_NET_OFFLINE=true cargo build --locked --offline --quiet \
  --manifest-path "$root/programs/clutch-sbf/operatord/Cargo.toml"
daemon="$root/programs/clutch-sbf/operatord/target/debug/clutch-sbf-operatord"
args=(
  serve --mode non-production-synthetic-source-v2-live
  --port "$http_port"
  --rpc-port "$rpc_port"
  --faucet-port "$faucet_port"
  --gossip-port "$gossip_port"
  --dynamic-port-range "$dynamic_port_range"
)
if [ "$exit_when_done" = 1 ]; then
  args+=(--exit-when-done)
fi
if [ -n "$work_base" ]; then
  args+=(--work "$work_base")
fi

echo "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE"
echo "Operator: http://127.0.0.1:$http_port/"
echo "Live child RPC: http://127.0.0.1:$rpc_port"
"$daemon" "${args[@]}" &
daemon_pid=$!

set +e
wait "$daemon_pid"
status=$?
set -e
daemon_pid=""
exit "$status"
