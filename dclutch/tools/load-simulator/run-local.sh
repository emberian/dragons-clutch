#!/usr/bin/env bash
# Run the existing load simulator against the journey's HELD validator:
#   config from the handoff -> one preflight -> N executed cycles with
#   reconciliation -> a byte-identical resume proof.
# Usage: run-local.sh HANDOFF_JSON SIM_WORK CYCLES [BOOTSTRAP_BIN]
set -euo pipefail
die() { echo "REFUSED: $*" >&2; exit 2; }
HANDOFF="${1:-}"; SIMWORK="${2:-}"; CYCLES="${3:-3}"
[ -f "$HANDOFF" ] || die "HANDOFF_JSON must be the journey's participant handoff"
[ -n "$SIMWORK" ] || die "SIM_WORK required"
case "$CYCLES" in ''|*[!0-9]*) die "CYCLES must be a positive count" ;; esac
[ "$CYCLES" -gt 0 ] || die "CYCLES must be a positive count"
HERE="$(cd "$(dirname "$0")" && pwd)"
CONFIG="$SIMWORK.config.json"
EXTRA=()
if [ -n "${4:-}" ]; then EXTRA=(--bootstrap-bin "$4"); fi

python3 "$HERE/build_config_from_probe.py" \
  --handoff "$HANDOFF" --sim-work "$SIMWORK" --output "$CONFIG" "${EXTRA[@]}"

echo "== preflight (signs nothing)"
python3 "$HERE/simulator.py" run --config "$CONFIG" --cycles 1

echo "== execute $CYCLES cycles"
python3 "$HERE/simulator.py" run --config "$CONFIG" --cycles "$CYCLES" --execute

echo "== resume proof: rerun must be a byte-identical no-op"
before="$(find "$SIMWORK/journal" -name cycle.json -exec shasum -a 256 {} + | sort)"
logs_before="$(find "$SIMWORK/logs" -type f | wc -l | tr -d ' ')"
python3 "$HERE/simulator.py" run --config "$CONFIG" --cycles "$CYCLES" --execute
after="$(find "$SIMWORK/journal" -name cycle.json -exec shasum -a 256 {} + | sort)"
logs_after="$(find "$SIMWORK/logs" -type f | wc -l | tr -d ' ')"
[ "$before" = "$after" ] || die "resume rewrote a finalized cycle journal"
[ "$logs_before" = "$logs_after" ] || die "resume re-invoked a driver ($logs_before -> $logs_after logs)"
echo "resume proof: journals byte-identical, no driver re-invoked"

echo "== status"
python3 - "$SIMWORK/status.json" <<'EOF'
import json, sys
s = json.load(open(sys.argv[1]))
print(json.dumps({k: s[k] for k in ("cycles", "trades", "last_reconciliation", "halted")}, indent=2))
EOF
