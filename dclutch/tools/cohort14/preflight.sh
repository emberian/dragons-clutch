#!/usr/bin/env bash
# Everything about cohort-14 that can be checked BEFORE a lamport moves.
#
# Cohort-13 spent two hours on a founding whose evidence a later consumer
# refused, and its resolution lane opened seven minutes after the only window
# its market would ever have. Both were knowable in advance and neither was
# checked in advance, so this script is the set of questions that cost nothing
# to ask and cost a cohort to skip.
#
# It runs offline. It signs nothing, reads no keypair, and touches no cluster
# unless --rpc-url is supplied, in which case it makes exactly one read.
#
# usage: tools/cohort14/preflight.sh [--commit REV] [--rpc-url URL] [--tests]
set -uo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
commit="HEAD"
rpc_url=""
run_tests="no"
failures=0

while [ "$#" -gt 0 ]; do
    case "$1" in
        --commit) commit="${2:?--commit needs a revision}"; shift 2 ;;
        --rpc-url) rpc_url="${2:?--rpc-url needs a URL}"; shift 2 ;;
        --tests) run_tests="yes"; shift ;;
        -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
        *) echo "cohort14 preflight: unknown argument $1" >&2; exit 64 ;;
    esac
done

say()  { printf '  %-58s %s\n' "$1" "$2"; }
fail() { say "$1" "RED   $2"; failures=$((failures + 1)); }
pass() { say "$1" "green $2"; }

cd "$repo_root" || exit 2
resolved="$(git rev-parse --verify "$commit^{commit}" 2>/dev/null)" || {
    echo "cohort14 preflight: $commit is not a commit in $repo_root" >&2
    exit 2
}
echo "cohort14 preflight"
echo "  tree root  $(git rev-parse --show-toplevel)"
echo "  commit     $resolved"
echo

# 1. THE FOUR COMMITS THE DEPLOY MUST CONTAIN.
#
# Each one is a property of the SHIPPED BYTES, so no tooling closes it after the
# fact: a cohort deployed without them cannot do the thing, and the only repair
# is another full redeploy.
echo "the deploy commit contains what cohort-14 must carry"
required_a517d27c="a517d27c  Trading's inline CPI input transport; OpenBatch cannot run without it"
required_90a8563f="90a8563f  the Registry observes an artifact deployment at finalization"
required_e7ecfb2e="e7ecfb2e  Claims admits the ImmutableOwner destination"
required_d218b963="d218b963  the third collateral adapter release, which a realm founds under"
for entry in "$required_a517d27c" "$required_90a8563f" "$required_e7ecfb2e" "$required_d218b963"; do
    rev="${entry%% *}"
    why="${entry#* }"
    why="${why# }"
    if ! git rev-parse --verify "$rev^{commit}" >/dev/null 2>&1; then
        fail "$rev" "not a commit in this tree"
    elif git merge-base --is-ancestor "$rev" "$resolved" 2>/dev/null; then
        pass "$rev" "$why"
    else
        fail "$rev" "NOT an ancestor -- $why"
    fi
done
echo

# 2. THE RELEASE A REALM WILL BE FOUNDED UNDER.
#
# RUN THE TEST, DO NOT GREP FOR THE DIGEST. A hex constant typed into a
# preflight is a mirror of the thing it checks: it agrees right up until the
# tree changes, and then it agrees with what the tree USED to say. The named
# tests below hold the digests as bytes and derive the tree's answer beside
# them, so running them is the only check that cannot pass for the wrong
# reason. That costs a build, so it is opt-in and its absence is reported as
# NOT CHECKED rather than as a pass.
echo "the collateral adapter release cohort-14 founds under"
if [ "$run_tests" = "yes" ]; then
    if (cd tools/local-validator/bootstrap/successor &&
        cargo test --quiet --bin dclutch-local-successor-bootstrap -- \
            --test-threads=1 collateral_release >/dev/null 2>&1); then
        pass "collateral_release tests" "founded id is the third release; cohort-13's is still admitted"
    else
        fail "collateral_release tests" "the founded release or cohort-13's admission moved"
    fi
else
    say "collateral_release tests" "not checked (pass --tests; they take a build)"
fi
echo "  founded          430369ce72f5e1dcfa19dcee63d5e15f9fbf2d6c9950c5caab53d5c028ae0a2d"
echo "  cohort-13 keeps  228c14f9e501f86138d3f19e5ea815af628c0adf499dc6a93dd8cb185c870e29"
echo

# 3. THE ACCELERATOR THE GENERAL MARKET WILL PIN.
#
# One flipped bit of this artifact release moves the manifest entry's release_id
# AND its config_id, and the entry is a seed of the Market PDA -- so a cohort
# that founds against the wrong accelerator founds a DIFFERENT MARKET, not the
# same market misconfigured.
echo "the General accelerator's deployment"
accelerator_program="8pgnyNvgdue7Jc8aw75BGWoghsKGevWJvFom8omUWvQY"
accelerator_slot="491959038"
accelerator_digest="61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae"
if grep -q "DEPLOYMENT_SLOT: u64 = 491_959_038" programs/dclutch-registry-sbf/src/record_v1.rs; then
    pass "slot pinned by the Registry's own partition" "$accelerator_slot"
else
    fail "slot" "record_v1.rs no longer pins $accelerator_slot in devnet_general_accelerator_observation"
fi
if [ -n "$rpc_url" ]; then
    live="$(curl -sS -X POST -H 'content-type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$accelerator_program\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"}]}" \
        "$rpc_url" 2>/dev/null)" || live=""
    case "$live" in
        *'"executable":true'*) pass "accelerator is live and executable" "$accelerator_program" ;;
        "")                    fail "accelerator" "the endpoint did not answer; NOT CHECKED is not a pass" ;;
        *)                     fail "accelerator" "$accelerator_program is not an executable program at finalized" ;;
    esac
else
    say "accelerator liveness" "not checked (pass --rpc-url to make one read)"
fi
echo "  expected ELF digest                                        $accelerator_digest"
echo

# 4. THE ORDER, which is the one thing cohort-12 got wrong and cohort-13 fixed.
echo "the ordering rule this runbook exists to hold"
if grep -q "^04	seal" tools/cohort14/steps.tsv && grep -q "^05	found-direct" tools/cohort14/steps.tsv; then
    pass "seal precedes founding" "cohort-12 founded first and stranded its market"
else
    fail "seal/found order" "steps.tsv must seal at 04 and found at 05"
fi
echo

# 5. THE SCHEDULE, replayed against cohort-13's own recorded window.
#
# A schedule that cannot say what it WOULD have done against a window that
# already closed is a schedule nobody can check before trusting it.
echo "the relay schedule, replayed against cohort-13's window"
say "capture would have fired" "17:23:39 UTC / 13:23:39 EDT, 1740s of window left"
say "settle would have fired"  "19:53:09 UTC / 15:53:09 EDT, 29s past the deadline"
say "reproduce it with" "devnet-sponsored-relay-schedule-v1 --replay-window 1788369759,1788371559,7200,0 --replay-now 1788348159"
echo

echo "----------------------------------------------------------------------"
if [ "$failures" -eq 0 ]; then
    echo "cohort14 preflight: every checkable precondition is green."
    echo "  What this does NOT check is named in README.md under 'What no"
    echo "  preflight can answer' -- read it before spending a lamport."
    exit 0
fi
echo "cohort14 preflight: $failures precondition(s) RED. Nothing should be deployed."
exit 1
