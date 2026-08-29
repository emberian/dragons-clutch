#!/usr/bin/env bash
# Prove the load simulator end-to-end against a HELD private-validator probe:
#   config from the handoff -> one preflight -> N executed cycles with
#   reconciliation -> a byte-identical resume proof.
# Usage: run-local.sh PROBE_WORK SIM_WORK CYCLES
set -euo pipefail
die() { echo "REFUSED: $*" >&2; exit 2; }
PROBE="${1:-}"; SIMWORK="${2:-}"; CYCLES="${3:-3}"
[ -d "$PROBE" ] || die "PROBE_WORK must be a held probe --work dir"
[ -n "$SIMWORK" ] || die "SIM_WORK required"
HERE="$(cd "$(dirname "$0")" && pwd)"
CONFIG="$SIMWORK.config.json"

python3 "$HERE/build_config_from_probe.py" \
  --probe-work "$PROBE" --sim-work "$SIMWORK" --output "$CONFIG"

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
