#!/usr/bin/env bash
# The runbook gate's own red proofs.
#
# Every claim below is proved by BREAKING the thing and watching the gate refuse,
# because a checker that has only ever been green is a checker nobody has tested.
# Each case works on a COPY under a fresh temporary directory; nothing here
# writes into the tree or into any job directory.
#
# usage: tools/cohort/test.sh
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
repo_root="$(cd "$here/../.." && pwd -P)"
work="$(mktemp -d "${TMPDIR:-/tmp}/cohort-runbook-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
passed=0
failed=0

ok()  { printf '  %-64s green\n' "$1"; passed=$((passed + 1)); }
bad() { printf '  %-64s RED   %s\n' "$1" "$2"; failed=$((failed + 1)); }

# Run a command that is EXPECTED TO REFUSE and match its output.
#
# Never `command | grep -q`: this script runs under `pipefail`, so the pipeline
# reports the refusing command's own non-zero status and every red proof that
# worked would read as a failure. Capturing first also means a case that fails
# can print what the instrument actually said.
refuses_with() {
    local what="$1" pattern="$2"; shift 2
    local output
    output="$("$@" 2>&1)"
    if printf '%s' "$output" | grep -q "$pattern"; then
        ok "$what"
    else
        bad "$what" "did not say: $pattern"
        printf '%s\n' "$output" | sed 's/^/      /' | head -4
    fi
}

# A copy of the runbook whose steps.tsv and manifests can be mutated freely.
# `check-steps.py` resolves its siblings from its own location, which is what
# makes a copy a complete instrument rather than a half of one.
copy() {
    rm -rf "$work/$1"
    mkdir -p "$work/$1"
    cp -R "$here/steps.tsv" "$here/README.md" "$here/check-steps.py" \
          "$here/generate-stage-scripts.py" "$here/cohorts" "$work/$1/"
}

# --- 1. the union is green on the tree as it stands ------------------------
if python3 "$here/check-steps.py" --prove-frozen >/dev/null 2>&1; then
    ok "the union reproduces both frozen runbooks"
else
    bad "the union reproduces both frozen runbooks" "it does not"
fi
for view in "--cohort 14" "--cohort 15" "--cohort 15 --delta"; do
    if python3 "$here/check-steps.py" $view >/dev/null 2>&1; then
        ok "check-steps $view is green"
    else
        bad "check-steps $view is green" "it is not"
    fi
done

# --- 2. a moved row breaks the reproduction --------------------------------
# The whole reason the frozen files stay is that this can go red.
copy moved
# THE POSITIVE CONTROL FIRST. "the mutation broke it" and "the copy was never a
# working instrument" exit identically, and only this line tells them apart.
if python3 "$work/moved/check-steps.py" --prove-frozen --frozen-root "$repo_root" >/dev/null 2>&1; then
    ok "the copy reproduces both frozen runbooks before it is mutated"
else
    bad "the copy reproduces both frozen runbooks before it is mutated" "the instrument was never connected"
fi
python3 - "$work/moved/steps.tsv" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
lines = path.read_text().splitlines()
for index, line in enumerate(lines):
    if line.startswith("census\t"):
        fields = line.split("\t")
        fields[7] = "L1 through L8 each reported by name"   # drops the INAPPLICABLE sentence
        lines[index] = "\t".join(fields)
path.write_text("\n".join(lines) + "\n")
PY
refuses_with "a changed verifier breaks --prove-frozen" "DOES NOT reproduce" \
    python3 "$work/moved/check-steps.py" --prove-frozen --frozen-root "$repo_root"

# --- 3. an unresolved manifest field is a refusal, not a rendered brace -----
copy unresolved
python3 - "$work/unresolved/cohorts/15.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text())
del document["economics"]["openbatch_n"]
path.write_text(json.dumps(document, indent=2))
PY
refuses_with "a missing manifest field is named and refused" "no field 'openbatch_n'" \
    python3 "$work/unresolved/check-steps.py" --cohort 15

# --- 4. a hollow verifier is still refused ---------------------------------
copy hollow
python3 - "$work/hollow/steps.tsv" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
lines = path.read_text().splitlines()
for index, line in enumerate(lines):
    if line.startswith("ladder\t"):
        fields = line.split("\t")
        fields[7] = "the campaign reports success and exits zero"
        lines[index] = "\t".join(fields)
path.write_text("\n".join(lines) + "\n")
PY
refuses_with "an exit code wearing a verifier's clothes is refused" "exit code in disguise" \
    python3 "$work/hollow/check-steps.py" --cohort 14

# --- 5. `replaces` and `until` cannot drift apart --------------------------
copy drift
python3 - "$work/drift/steps.tsv" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
lines = path.read_text().splitlines()
for index, line in enumerate(lines):
    if line.startswith("deploy\t"):
        fields = line.split("\t")
        fields[3] = "15"          # deploy claims to still run at 15, where redeploy replaced it
        lines[index] = "\t".join(fields)
path.write_text("\n".join(lines) + "\n")
PY
refuses_with "a replaced row that claims to still run is refused" "would run both or neither" \
    python3 "$work/drift/check-steps.py" --cohort 15

# --- 6. a blocks edge into a retired row follows the replacement -----------
copy dangle
python3 - "$work/dangle/steps.tsv" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
lines = path.read_text().splitlines()
for index, line in enumerate(lines):
    if line.startswith("redeploy\t"):
        fields = line.split("\t")
        fields[4] = "-"           # redeploy no longer says what it replaced
        lines[index] = "\t".join(fields)
path.write_text("\n".join(lines) + "\n")
PY
refuses_with "an edge into a retired row with no replacement is refused" "neither runs nor replaces" \
    python3 "$work/dangle/check-steps.py" --cohort 15

# --- 7. THE VALUE TEST: an emitted script may name no path outside the job --
#
# This is db1e4eaa6's rule, applied to the whole family instead of one script.
# Proved by making the generator try to write one: the genesis hash is
# interpolated into every emitted script, so a manifest carrying a path there
# is the cheapest way to make the generator produce exactly the defect the test
# exists to catch. It must refuse AND LEAVE NOTHING BEHIND -- a refusal that
# leaves 22 scripts on disk is a refusal somebody will run.
copy value
python3 - "$work/value/cohorts/15.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text())
document["cluster"]["genesis_hash"] = "/private/tmp/some-build-scratch/genesis"
path.write_text(json.dumps(document, indent=2))
PY
out="$work/value-out"
value_output="$(python3 "$work/value/generate-stage-scripts.py" --cohort 15 --out "$out" 2>&1)"
value_left="$(ls "$out"/*.sh 2>/dev/null | wc -l | tr -d ' ')"
if printf '%s' "$value_output" | grep -q "depend on paths outside it" && [ "$value_left" = "0" ]; then
    ok "an emitted absolute path is refused and nothing is left on disk"
else
    bad "an emitted absolute path is refused and nothing is left on disk" \
        "$value_left scripts survived; said: $(printf '%s' "$value_output" | head -1)"
fi

# --- 8. an emitted script may not hold an endpoint credential --------------
copy cred
python3 - "$work/cred/cohorts/15.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text())
document["cluster"]["genesis_hash"] = "https://devnet.example.invalid/?api-key=SECRET"
path.write_text(json.dumps(document, indent=2))
PY
refuses_with "an emitted endpoint credential is refused" "endpoint credential" \
    python3 "$work/cred/generate-stage-scripts.py" --cohort 15 --out "$work/cred-out"

# --- 9. the generator will not clobber a hand-written stage script ---------
#
# COHORT-15D is live in its job directory as this is written. The generator
# refusing here is the only reason it is safe to have at all.
mkdir -p "$work/handwritten"
printf '#!/bin/bash\n# a lane wrote this by hand\n' > "$work/handwritten/capture.sh"
refuses_with "the generator refuses a job directory holding hand-written scripts" \
    "refusing to write beside a hand-written script" \
    python3 "$here/generate-stage-scripts.py" --cohort 15 --out "$work/handwritten"

# --- 10. every emitted script is a script bash can parse -------------------
python3 "$here/generate-stage-scripts.py" --cohort 15 --out "$work/parse-out" >/dev/null 2>&1
broken=0
for script in "$work/parse-out"/*.sh; do
    bash -n "$script" 2>/dev/null || broken=$((broken + 1))
done
emitted="$(ls "$work/parse-out"/*.sh 2>/dev/null | wc -l | tr -d ' ')"
if [ "$broken" = "0" ] && [ "$emitted" -gt 0 ]; then
    ok "all $emitted emitted scripts parse under bash"
else
    bad "all emitted scripts parse under bash" "$broken of $emitted do not"
fi

# --- 11. a stage with no flags file refuses rather than running a partial --
refuses_with "a stage whose flags the runbook does not carry refuses to run" \
    "not in the runbook yet" \
    env DCLUTCH_RPC_URL=https://example.invalid bash "$work/parse-out/08-activate-direct.sh"

echo "----------------------------------------------------------------------"
echo "cohort runbook tests: $passed green, $failed RED"
[ "$failed" -eq 0 ]
