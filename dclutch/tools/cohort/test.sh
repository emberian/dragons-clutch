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
          "$here/generate-stage-scripts.py" "$here/cohorts" "$here/frozen" "$work/$1/"
}

# Mutate one field of one row in a copy's steps.tsv: mutate COPY KEY FIELD VALUE.
mutate() {
    python3 - "$work/$1/steps.tsv" "$2" "$3" "$4" <<'PY'
import pathlib, sys
path, key, field, value = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
FIELDS = ("key","stage","since","until","replaces","shape","command","args","verifier","cost","blocks")
lines = path.read_text().splitlines()
for index, line in enumerate(lines):
    if line.startswith(key + "\t"):
        fields = line.split("\t")
        fields[FIELDS.index(field)] = value
        lines[index] = "\t".join(fields)
path.write_text("\n".join(lines) + "\n")
PY
}

# --- 1. the union is green on the tree as it stands ------------------------
if python3 "$here/check-steps.py" --prove-frozen >/dev/null 2>&1; then
    ok "the union reproduces both frozen tables"
else
    bad "the union reproduces both frozen tables" "it does not"
fi
for view in "--cohort 14" "--cohort 15" "--cohort 15 --delta"; do
    if python3 "$here/check-steps.py" $view >/dev/null 2>&1; then
        ok "check-steps $view is green"
    else
        bad "check-steps $view is green" "it is not"
    fi
done

# --- 2. a moved row breaks the reproduction --------------------------------
# The whole reason the frozen tables stay is that this can go red.
copy moved
# THE POSITIVE CONTROL FIRST. "the mutation broke it" and "the copy was never a
# working instrument" exit identically, and only this line tells them apart.
if python3 "$work/moved/check-steps.py" --prove-frozen >/dev/null 2>&1; then
    ok "the copy reproduces both frozen tables before it is mutated"
else
    bad "the copy reproduces both frozen tables before it is mutated" "the instrument was never connected"
fi
mutate moved census verifier "L1 through L8 each reported by name"   # drops the INAPPLICABLE sentence
refuses_with "a changed verifier breaks --prove-frozen" "DOES NOT reproduce" \
    python3 "$work/moved/check-steps.py" --prove-frozen

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
mutate hollow ladder verifier "the campaign reports success and exits zero"
refuses_with "an exit code wearing a verifier's clothes is refused" "exit code in disguise" \
    python3 "$work/hollow/check-steps.py" --cohort 14

# --- 5. `replaces` and `until` cannot drift apart --------------------------
copy drift
mutate drift deploy until 15          # deploy claims to still run at 15, where redeploy replaced it
refuses_with "a replaced row that claims to still run is refused" "would run both or neither" \
    python3 "$work/drift/check-steps.py" --cohort 15

# --- 6. a blocks edge into a retired row follows the replacement -----------
copy dangle
mutate dangle redeploy replaces -     # redeploy no longer says what it replaced
refuses_with "an edge into a retired row with no replacement is refused" "neither runs nor replaces" \
    python3 "$work/dangle/check-steps.py" --cohort 15

# --- 7. the args are gated for shape, not only for prose -------------------
copy unknown-driver
mutate unknown-driver seal args "cargo run --bin whatever"
refuses_with "an invocation naming a driver the generator lacks is refused" "cannot emit" \
    python3 "$work/unknown-driver/check-steps.py" --cohort 15
copy loopless
mutate loopless payout shape once
refuses_with "a \`*\` act under a shape that does not loop is refused" "does not loop" \
    python3 "$work/loopless/check-steps.py" --cohort 15
copy actless
mutate actless relay-capture args "bootstrap devnet-sponsored-push-v1 --action capture"
refuses_with "a looping shape with no \`*\` act is refused" "no \`\\*\` act to loop" \
    python3 "$work/actless/check-steps.py" --cohort 15
copy argfield
mutate argfield seal args "bootstrap prepare --output {economics.no_such_field}"
refuses_with "an args placeholder the manifest cannot answer is refused" "no field 'economics.no_such_field'" \
    python3 "$work/argfield/check-steps.py" --cohort 15

# --- 8. THE VALUE TEST: an emitted script may name no path outside the job --
#
# This is db1e4eaa6's rule, applied to the whole family. Proved by making the
# generator try to write one: the genesis hash is interpolated into every
# emitted script, so a manifest carrying a path there is the cheapest way to
# make the generator produce exactly the defect the test exists to catch. It
# must refuse AND LEAVE NOTHING BEHIND -- a refusal that leaves 22 scripts on
# disk is a refusal somebody will run.
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

# --- 9. an emitted script may not hold an endpoint credential --------------
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

# --- 10. a market fact the manifest does not carry refuses BY NAME ---------
# and leaves nothing behind: an operator records it and regenerates.
copy nomarket
python3 - "$work/nomarket/cohorts/15.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
document = json.loads(path.read_text())
del document["markets"][0]["lookup_table"]
path.write_text(json.dumps(document, indent=2))
PY
nomarket_output="$(python3 "$work/nomarket/generate-stage-scripts.py" --cohort 15 --out "$work/nomarket-out" 2>&1)"
nomarket_left="$(ls "$work/nomarket-out"/*.sh 2>/dev/null | wc -l | tr -d ' ')"
if printf '%s' "$nomarket_output" | grep -q "has no field 'market.lookup_table'" && [ "$nomarket_left" = "0" ]; then
    ok "a missing market fact is named, refused, and nothing is left on disk"
else
    bad "a missing market fact is named, refused, and nothing is left on disk" \
        "$nomarket_left scripts survived; said: $(printf '%s' "$nomarket_output" | head -1)"
fi

# --- 11. the generator will not clobber a hand-written stage script --------
mkdir -p "$work/handwritten"
printf '#!/bin/bash\n# a lane wrote this by hand\n' > "$work/handwritten/capture.sh"
refuses_with "the generator refuses a job directory holding hand-written scripts" \
    "refusing to write beside a hand-written script" \
    python3 "$here/generate-stage-scripts.py" --cohort 15 --out "$work/handwritten"

# --- 12. every emitted script is a script bash can parse, one per market ---
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
per_market="$(ls "$work/parse-out"/*-relay-capture-*.sh 2>/dev/null | wc -l | tr -d ' ')"
if [ "$per_market" = "2" ]; then
    ok "a per-market row fans out once per direct market (2)"
else
    bad "a per-market row fans out once per direct market (2)" "found $per_market"
fi

# --- 13. a row carrying no args refuses rather than running a partial ------
refuses_with "a row whose args the runbook does not carry refuses to run" \
    "carries no args yet" \
    bash "$(ls "$work/parse-out"/*-route-witness.sh)"

# --- 14. THE PEER-CHAINING GUARD: nothing starts until what blocks it is green
# The old scripts grepped each other's logs for SETTLE_LANDED. Now a stage
# refuses at its first line unless every blocker left a GREEN marker -- and it
# refuses BEFORE asking for the endpoint, so this proof needs no credential.
refuses_with "a stage refuses to start until its blocker has gone green" \
    "has not gone green" \
    env DCLUTCH_RPC_URL=https://example.invalid bash "$(ls "$work/parse-out"/*-relay-settle-1.sh)" --preflight
mkdir -p "$work/parse-out/relay-capture-1" && touch "$work/parse-out/relay-capture-1/GREEN"
# With the blocker green the same script reaches the endpoint guard; the
# unreachable endpoint is now what stops it, which is the guard we wanted lifted.
lifted="$(cd "$work/parse-out" && env DCLUTCH_RPC_URL=https://example.invalid bash ./*-relay-settle-1.sh --preflight 2>&1)"
if ! printf '%s' "$lifted" | grep -q "has not gone green"; then
    ok "the same stage starts once its blocker is green"
else
    bad "the same stage starts once its blocker is green" "still guarded"
fi

echo "----------------------------------------------------------------------"
echo "cohort runbook tests: $passed green, $failed RED"
[ "$failed" -eq 0 ]
