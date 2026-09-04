#!/usr/bin/env bash
# Everything about a cohort that can be checked BEFORE a lamport moves.
#
# There is one of these, for every cohort, because the questions do not change
# between cohorts -- only the answers do, and the answers live in
# `cohorts/<n>.json`. Cohort-13 spent two hours on a founding whose evidence a
# later consumer refused, and its resolution lane opened seven minutes after the
# only window its market would ever have. Both were knowable in advance and
# neither was checked in advance, so this script is the set of questions that
# cost nothing to ask and cost a cohort to skip.
#
# It runs offline. It signs nothing, reads no keypair, and touches no cluster
# unless --rpc-url is supplied, in which case it makes a bounded set of reads.
#
# usage: tools/cohort/preflight.sh --cohort N|PATH [--commit REV] [--rpc-url URL] [--tests]
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd -P)"
repo_root="$(cd "$here/../.." && pwd)"
cohort=""
commit=""
rpc_url=""
run_tests="no"
failures=0

while [ "$#" -gt 0 ]; do
    case "$1" in
        --cohort) cohort="${2:?--cohort needs a number or a manifest path}"; shift 2 ;;
        --commit) commit="${2:?--commit needs a revision}"; shift 2 ;;
        --rpc-url) rpc_url="${2:?--rpc-url needs a URL}"; shift 2 ;;
        --tests) run_tests="yes"; shift ;;
        -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
        *) echo "cohort preflight: unknown argument $1" >&2; exit 64 ;;
    esac
done
[ -n "$cohort" ] || { echo "cohort preflight: --cohort is required" >&2; exit 64; }

say()  { printf '  %-58s %s\n' "$1" "$2"; }
fail() { say "$1" "RED   $2"; failures=$((failures + 1)); }
pass() { say "$1" "green $2"; }

cd "$repo_root" || exit 2

# THE MANIFEST IS THE ONLY PLACE A COHORT'S FACTS LIVE. Reading them here rather
# than typing them is the whole point: a hex constant typed into a preflight is
# a mirror of the thing it checks, and it agrees right up until the tree changes.
manifest_path="$cohort"
[ -f "$manifest_path" ] || manifest_path="$here/cohorts/$cohort.json"
[ -f "$manifest_path" ] || { echo "cohort preflight: no manifest at $manifest_path" >&2; exit 2; }
field() { python3 -c 'import json,sys
node=json.load(open(sys.argv[1]))
for part in sys.argv[2].split("."):
    node=node[part] if isinstance(node,dict) and part in node else None
    if node is None: break
print("" if node is None else node)' "$manifest_path" "$1"; }

cohort_number="$(field cohort)"
prior_cohort="$(field prior_cohort)"
[ -n "$cohort_number" ] || { echo "cohort preflight: $manifest_path names no cohort" >&2; exit 2; }
[ -n "$commit" ] || commit="$(field deploy_commit)"
[ -n "$commit" ] || commit=HEAD

resolved="$(git rev-parse --verify "$commit^{commit}" 2>/dev/null)" || {
    echo "cohort preflight: $commit is not a commit in $repo_root" >&2
    exit 2
}
echo "cohort-$cohort_number preflight"
echo "  tree root  $(git rev-parse --show-toplevel)"
echo "  commit     $resolved"
echo "  manifest   $manifest_path"
echo

# 0. THE RUNBOOK RESOLVES AGAINST THIS MANIFEST.
#
# Before any question about the chain: can this cohort's rows even be rendered?
# A row naming a field the manifest does not carry is a step with no author, and
# it is free to find out here rather than at 3am with the deploy half done.
echo "the runbook renders for this cohort"
if steps_out="$(python3 "$here/check-steps.py" --cohort "$manifest_path" 2>&1)"; then
    pass "steps.tsv + README + manifest" "$(printf '%s' "$steps_out" | head -1 | cut -c1-70)"
else
    fail "steps.tsv + README + manifest" "the runbook does not render"
    printf '%s\n' "$steps_out" | sed 's/^/      /'
fi
echo

# 1. THE COMMITS THE DEPLOY MUST CONTAIN.
#
# Each one is a property of the SHIPPED BYTES, so no tooling closes it after the
# fact: a cohort deployed without them cannot do the thing, and the only repair
# is another full redeploy.
echo "the deploy commit contains what cohort-$cohort_number must carry"
# The loop runs in a subshell, so its verdicts come back as text and are
# counted here. Never a temp file inside the tree: parallel lanes share this
# checkout and an untracked file appearing under tools/ is somebody's next
# twenty minutes.
commit_report="$(python3 -c 'import json,sys
required=json.load(open(sys.argv[1])).get("required_commits",{})
for slug,entry in required.items():
    print("%s\t%s\t%s" % (slug, entry["rev"], entry["why"]))' "$manifest_path" |
while IFS=$'\t' read -r slug rev why; do
    if ! git rev-parse --verify "$rev^{commit}" >/dev/null 2>&1; then
        printf '  %-58s %s\n' "$rev" "RED   not a commit in this tree"
    elif git merge-base --is-ancestor "$rev" "$resolved" 2>/dev/null; then
        printf '  %-58s %s\n' "$rev" "green $why"
    else
        printf '  %-58s %s\n' "$rev" "RED   NOT an ancestor -- $why"
    fi
done)"
printf '%s\n' "$commit_report"
failures=$((failures + $(printf '%s' "$commit_report" | grep -c 'RED')))
echo

# 2. THE RELEASE A REALM WILL BE FOUNDED UNDER.
#
# RUN THE TEST, DO NOT GREP FOR THE DIGEST. The named tests hold the digests as
# bytes and derive the tree's answer beside them, so running them is the only
# check that cannot pass for the wrong reason. That costs a build, so it is
# opt-in and its absence is reported as NOT CHECKED rather than as a pass.
echo "the collateral adapter release cohort-$cohort_number founds under"
release_manifest="$(field collateral_release.test_manifest)"
release_bin="$(field collateral_release.test_bin)"
release_filter="$(field collateral_release.test_filter)"
if [ "$run_tests" = "yes" ] && [ -n "$release_manifest" ]; then
    if (cd "$(dirname "$release_manifest")" &&
        cargo test --quiet --bin "$release_bin" -- --test-threads=1 "$release_filter" >/dev/null 2>&1); then
        pass "$release_filter tests" "founded id is current; the prior cohort's is still admitted"
    else
        fail "$release_filter tests" "the founded release or cohort-$prior_cohort's admission moved"
    fi
else
    say "$release_filter tests" "not checked (pass --tests; they take a build)"
fi
echo "  founded             $(field collateral_release.founded)"
echo "  cohort-$prior_cohort keeps    $(field collateral_release.prior_keeps)"
echo

# 3. THE ACCELERATOR THE GENERAL MARKET WILL PIN.
#
# One flipped bit of this artifact release moves the manifest entry's release_id
# AND its config_id, and the entry is a seed of the Market PDA -- so a cohort
# that founds against the wrong accelerator founds a DIFFERENT MARKET, not the
# same market misconfigured.
echo "the General accelerator's deployment"
accelerator_program="$(field general_accelerator.program_id)"
accelerator_slot="$(field general_accelerator.deployment_slot)"
pin_source="$(field general_accelerator.registry_pin_source)"
pin_text="$(field general_accelerator.registry_pin_text)"
if [ -n "$pin_text" ] && grep -q "$pin_text" "$pin_source"; then
    pass "slot pinned by the Registry's own partition" "$accelerator_slot"
else
    fail "slot" "$pin_source no longer pins $accelerator_slot"
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
echo "  expected ELF digest                                        $(field general_accelerator.elf_sha256)"
echo

# 4. THE PROVIDER RELEASE THE MARKET WILL PIN.
#
# COHORT-14 PAID FOR THIS ONE. A sponsored market pins Pyth's Receiver and push
# oracle by exact (ProgramData, deployment_slot, upgrade_authority) equality,
# because rehashing 1.64 MiB on every capture is not a transaction path, so
# Loader-v3's monotonic slot is the proxy and any deployment movement fails
# closed as `ReleaseSuperseded`. A THIRD conjunct wears the same code:
# `hash(receiver_config) != receiver_config_digest` is also `0x8014`.
#
# A market no longer pins the typed constant; it pins what
# `sponsored_release_observation` reads off the chain at plan time, with the
# constant as the DECLARATION each observed field is compared against. So drift
# is reported and is not a failure. What IS a failure is a chain a founding
# could not mint an honest release against: a silent endpoint, an immutable
# provider, a deployment slot BELOW the declared one, or a Receiver Config that
# is not a canonical V2 body.
echo "the Pyth provider release a sponsored market would pin"
declaration="$(field pyth.declaration_source)"
pinned_receiver_slot="$(sed -n 's/.*receiver_deployment_slot: \([0-9_]*\),.*/\1/p' "$declaration" | tr -d '_' | head -1)"
pinned_push_slot="$(sed -n 's/.*push_oracle_deployment_slot: \([0-9_]*\),.*/\1/p' "$declaration" | tr -d '_' | head -1)"
say "release DECLARES receiver slot" "${pinned_receiver_slot:-<not found>}"
say "release DECLARES push oracle slot" "${pinned_push_slot:-<not found>}"
if [ -n "$rpc_url" ]; then
    rpc_account() {
        curl -sS -X POST -H 'content-type: application/json' \
            -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getAccountInfo\",\"params\":[\"$1\",{\"encoding\":\"base64\",\"commitment\":\"finalized\"$2}]}" \
            "$rpc_url" 2>/dev/null
    }
    live_header() {
        rpc_account "$1" ',"dataSlice":{"offset":0,"length":45}' |
            python3 -c 'import base64,json,struct,sys
value=json.load(sys.stdin).get("result",{}).get("value")
if not value:
    raise SystemExit(0)
raw=base64.b64decode(value["data"][0])
print(struct.unpack("<Q", raw[4:12])[0], "authority" if raw[12] == 1 else "IMMUTABLE")' 2>/dev/null
    }
    for entry in "receiver $(field pyth.receiver) $pinned_receiver_slot" \
                 "push-oracle $(field pyth.push_oracle) $pinned_push_slot"; do
        set -- $entry
        header="$(live_header "$2")"
        observed="${header%% *}"
        authority="${header##* }"
        if [ -z "$observed" ]; then
            fail "pyth $1 slot" "the endpoint did not answer; NOT CHECKED is not a pass"
        elif [ "$authority" = "IMMUTABLE" ]; then
            fail "pyth $1" "carries no Loader-v3 upgrade authority; a sponsored release has no encoding for an immutable provider"
        elif [ "$observed" -lt "$3" ]; then
            fail "pyth $1" "slot ROLLBACK -- declared $3, live $observed; no monotonic loader produced both numbers"
        elif [ "$observed" = "$3" ]; then
            pass "pyth $1 deployment slot" "$observed (declared, unmoved)"
        else
            pass "pyth $1 deployment slot" "$observed (declared $3; the market pins the OBSERVED value)"
        fi
    done
    config_state="$(rpc_account "$(field pyth.receiver_config)" '' |
        python3 -c 'import base64,hashlib,json,sys
value=json.load(sys.stdin).get("result",{}).get("value")
if not value:
    raise SystemExit(0)
raw=base64.b64decode(value["data"][0])
print(len(raw), raw[:8].hex(), hashlib.sha256(raw).hexdigest())' 2>/dev/null)"
    set -- $config_state
    declared_config=$(python3 - "$declaration" <<'DECLARED_CONFIG'
import re, sys
source = open(sys.argv[1]).read()
# Scope to the CONSTANT, not the struct field of the same name declared above it.
source = source.split("pub fn devnet_sponsored_sol_usd_release_v1", 1)[1]
body = source.split("receiver_config_digest: [", 1)[1].split("],", 1)[0]
print("".join("%02x" % int(value, 16) for value in re.findall(r"0x[0-9a-fA-F]{2}", body)))
DECLARED_CONFIG
)
    if [ -z "${1:-}" ]; then
        fail "pyth receiver config" "the endpoint did not answer; NOT CHECKED is not a pass"
    elif [ "$1" != "$(field pyth.config_bytes)" ] || [ "$2" != "$(field pyth.config_discriminator)" ]; then
        fail "pyth receiver config" "not a canonical $(field pyth.config_bytes)-byte V2 Config: $1 bytes, discriminator $2"
    elif [ "$3" = "$declared_config" ]; then
        pass "pyth receiver config digest" "$3 (declared, unmoved)"
    else
        pass "pyth receiver config digest" "$3 (declared ${declared_config:-<not found>}; the market pins the OBSERVED value)"
    fi
else
    say "pyth deployment slots" "not checked (pass --rpc-url to make three reads)"
fi
echo

# 5. THE ORDER, which is the one thing cohort-12 got wrong and cohort-13 fixed.
#
# Asked of the RENDERED runbook rather than by grepping for two literal ids, so
# it keeps meaning the same thing when the rows renumber.
echo "the ordering rule this runbook exists to hold"
order="$(python3 "$here/check-steps.py" --cohort "$manifest_path" --emit-legacy 2>/dev/null |
         awk -F'\t' '$2=="seal"{s=$1} $2 ~ /^(re)?found/{if (f=="") f=$1} END{print s" "f}')"
seal_id="${order%% *}"; found_id="${order##* }"
if [ -n "$seal_id" ] && [ -n "$found_id" ] && [ "$seal_id" \< "$found_id" ]; then
    pass "seal precedes founding" "seal at $seal_id, first founding at $found_id"
else
    fail "seal/found order" "cohort-12 founded before sealing and stranded its market (seal '$seal_id', found '$found_id')"
fi
echo

# 6. THE SCHEDULE, replayed against a window that already closed.
#
# A schedule that cannot say what it WOULD have done against a window that
# already closed is a schedule nobody can check before trusting it.
echo "the relay schedule, replayed against a closed window"
say "capture would have fired" "$(field relay_replay.capture_would_fire)"
say "settle would have fired"  "$(field relay_replay.settle_would_fire)"
say "reproduce it with" "devnet-sponsored-relay-schedule-v1 --replay-window $(field relay_replay.window) --replay-now $(field relay_replay.now)"
echo

echo "----------------------------------------------------------------------"
if [ "$failures" -eq 0 ]; then
    echo "cohort-$cohort_number preflight: every checkable precondition is green."
    echo "  What this does NOT check is named in README.md under 'What no"
    echo "  preflight can answer' -- read it before spending a lamport."
    exit 0
fi
echo "cohort-$cohort_number preflight: $failures precondition(s) RED. Nothing should be deployed."
exit 1
