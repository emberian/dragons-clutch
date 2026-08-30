#!/usr/bin/env bash
# Prove sustain mode + clean SIGTERM against a live config:
# start --sustain --execute, wait for >=MIN_CYCLES finalized cycles, SIGTERM,
# then assert: exit 0, every journal sealed, final status not halted.
# Usage: run-sustain-proof.sh CONFIG_JSON SIM_WORK [MIN_CYCLES]
set -euo pipefail
die() { echo "REFUSED: $*" >&2; exit 2; }
CONFIG="${1:-}"; SIMWORK="${2:-}"; MIN="${3:-2}"
[ -r "$CONFIG" ] && [ -n "$SIMWORK" ] || die "usage: run-sustain-proof.sh CONFIG SIM_WORK [MIN_CYCLES]"
HERE="$(cd "$(dirname "$0")" && pwd)"

python3 "$HERE/simulator.py" run --config "$CONFIG" --sustain --execute \
  > "$SIMWORK.sustain.log" 2>&1 &
SIM_PID=$!
echo "sustain pid $SIM_PID"

finalized() { grep -rls '"phase": "finalized"' "$SIMWORK/journal" 2>/dev/null | wc -l | tr -d ' '; }
deadline=$(( $(date +%s) + 900 ))
while [ "$(finalized)" -lt "$MIN" ]; do
  kill -0 "$SIM_PID" 2>/dev/null || { tail -20 "$SIMWORK.sustain.log"; die "sustain loop exited early"; }
  [ "$(date +%s)" -lt "$deadline" ] || die "no $MIN finalized cycles within 15m"
  sleep 5
done

echo "reached $(finalized) finalized cycles; sending SIGTERM"
kill -TERM "$SIM_PID"
wait "$SIM_PID"; STATUS=$?
[ "$STATUS" -eq 0 ] || die "sustain exit status $STATUS after SIGTERM"
grep -q "stopped cleanly" "$SIMWORK.sustain.log" || die "no clean-stop line in log"

python3 - "$SIMWORK" <<'EOF'
import json, pathlib, sys
work = pathlib.Path(sys.argv[1])
for p in sorted(work.glob("journal/cycle-*/cycle.json")):
    body = json.loads(p.read_text())
    if body["phase"] != "finalized":
        raise SystemExit(f"unsealed journal after SIGTERM: {p} phase={body['phase']}")
status = json.loads((work / "status.json").read_text())
assert not status["halted"], "status halted after clean stop"
print("sustain proof: exit 0, all journals sealed, cycles run =", status["cycles"]["run"])
EOF
