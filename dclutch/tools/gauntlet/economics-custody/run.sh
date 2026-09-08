#!/usr/bin/env bash
# Custody parameters/upkeep founding on a fresh checked local validator.
set -euo pipefail

repo="$(cd "$(dirname "$0")/../../.." && pwd)"
gate=""
work=""
port=""
census="false"
usage() {
    echo "usage: tools/gauntlet/economics-custody/run.sh --checked-release-gate ABS_JSON --work ABS_NEW_DIR --rpc-port PORT [--census]"
}
while [ "$#" -gt 0 ]; do
    case "$1" in
        --checked-release-gate) gate="${2:?missing gate}"; shift 2 ;;
        --work) work="${2:?missing work}"; shift 2 ;;
        --rpc-port) port="${2:?missing port}"; shift 2 ;;
        --census) census="true"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 64 ;;
    esac
done
case "$gate" in /*) ;; *) usage >&2; exit 64 ;; esac
case "$work" in /*) ;; *) usage >&2; exit 64 ;; esac
case "$port" in ''|*[!0-9]*) usage >&2; exit 64 ;; esac
[ "$port" -ge 1024 ] && [ "$port" -le 65494 ] || exit 64
command -v solana-test-validator >/dev/null 2>&1 || {
    echo "solana-test-validator is required" >&2; exit 2;
}
[ -f "$gate" ] && [ ! -L "$gate" ] || { echo "regular checked release gate required" >&2; exit 2; }
[ ! -e "$work" ] || { echo "work must be new: $work" >&2; exit 2; }
cd "$repo"
git rev-parse --show-toplevel
host_revision="$(git rev-parse HEAD)"
echo "$host_revision"
[ -z "$(git status --porcelain --untracked-files=normal)" ] || {
    echo "campaign host must be a clean committed checkout" >&2; exit 2;
}
if command -v swarm-build >/dev/null 2>&1 && [ "${SWARM_BUILD_INNER:-}" != 1 ]; then
    echo "run this campaign through swarm-build" >&2; exit 2
fi
mkdir -p "$work"
gate_sha="$(shasum -a 256 "$gate" | cut -d' ' -f1)"
runtime_revision="$(jq -er '.source_revision' "$gate")"
runtime_tree="$(jq -er '.source_tree_sha256' "$gate")"
seed="$(printf '%s' 'dclutch/gauntlet/economics-custody/seed/v1' | shasum -a 256 | cut -d' ' -f1)"
jq -n --arg host "$host_revision" --arg runtime "$runtime_revision" --arg gate "$gate_sha" \
    '{schema:"dclutch-economics-custody-source-v1",host_source_revision:$host,runtime_source_revision:$runtime,checked_release_gate_sha256:$gate,evidence_level:"local-validator",classification:"diagnostic-host-current-runtime-checked"}' \
    > "$work/source.json"
cargo build --locked --release -p dclutch-journey-campaign > "$work/build.log" 2>&1 || {
    tail -40 "$work/build.log" >&2; exit 1;
}
if grep -q 'Unit run-u.*scope was already loaded' "$work/build.log"; then
    echo "scheduler executed no build" >&2; exit 1
fi
status=0
DCLUTCH_HOST_SOURCE_REVISION="$host_revision" "$repo/target/release/dclutch-journey-campaign" economics \
    --transcript "$work/transcript.json" --work "$work/campaign" --rpc-port "$port" \
    --checked-release-gate "$gate" --expected-gate-sha256 "$gate_sha" \
    --expected-source-revision "$runtime_revision" --expected-source-tree-sha256 "$runtime_tree" \
    --seed "$seed" --holders 1 > "$work/campaign.stdout" 2> "$work/campaign.stderr" || status=$?
if [ -f "$work/transcript.json" ]; then
    jq '{completed,wall,stages:[.stages[]|{stage,outcome,exactRefusal,custom}]}' "$work/transcript.json"
else
    echo "campaign emitted no transcript" >&2
    status=1
fi
[ "$status" = 0 ] || tail -30 "$work/campaign.stderr" >&2

if [ "$status" = 0 ] && [ "$census" = "true" ]; then
    gauntlet_work="${DCLUTCH_GAUNTLET_WORK:-/private/tmp/dclutch-gauntlet}"
    inventory="$gauntlet_work/out/inventory.json"
    [ -f "$inventory" ] || {
        echo "economics-custody: --census needs $inventory; run tools/gate census first" >&2
        exit 2
    }
    programs="$work/campaign/programs.json"
    jq '{registry:.registry.program_id, core:.core.program_id, claims:.claims.program_id,
         trading:.trading.program_id, resolution:.resolution.program_id,
         custody:.custody.program_id, rent:.rent_credit.program_id,
         accelerator:.general_accelerator.program_id} |
        with_entries(select(.value != null))' \
        "$work/campaign/substrate/plan.json" > "$programs"

    "$repo/tools/gate" census observe \
        --work "$gauntlet_work" \
        --bindings "$repo/tools/gauntlet/economics-custody/bindings.json" \
        --programs "$programs" --evidence "$work/campaign/evidence.json" || status=$?
fi
exit "$status"
