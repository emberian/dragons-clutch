#!/usr/bin/env bash
# Nonzero Dealer execution on a fresh checked, owned local validator.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../../.." && pwd)"
gate=""
work=""
port=""
usage() {
    echo "usage: tools/gauntlet/scoring-dealer/run.sh --checked-release-gate ABS_JSON --work ABS_NEW_DIR --rpc-port PORT"
}
while [ "$#" -gt 0 ]; do
    case "$1" in
        --checked-release-gate) gate="${2:?missing gate}"; shift 2 ;;
        --work) work="${2:?missing work}"; shift 2 ;;
        --rpc-port) port="${2:?missing port}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; exit 64 ;;
    esac
done
case "$gate" in /*) ;; *) usage >&2; exit 64 ;; esac
case "$work" in /*) ;; *) usage >&2; exit 64 ;; esac
case "$port" in ''|*[!0-9]*) usage >&2; exit 64 ;; esac
[ "$port" -ge 1024 ] && [ "$port" -le 65494 ] || exit 64
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
revision="$(jq -er '.source_revision' "$gate")"
tree="$(jq -er '.source_tree_sha256' "$gate")"
seed="$(printf '%s' 'dclutch/gauntlet/scoring-dealer/seed/v1' | shasum -a 256 | cut -d' ' -f1)"
jq -n --arg host "$host_revision" --arg release "$revision" --arg gate "$gate_sha" \
    '{schema:"dclutch-dealer-campaign-source-v1",host_source_revision:$host,release_source_revision:$release,checked_release_gate_sha256:$gate,evidence_level:"local-validator"}' \
    > "$work/source.json"
# The host and release revisions are both recorded. This campaign does not
# certify a release; a later release cut must rerun against its final artifacts.
cargo build --locked --release -p dclutch-journey-campaign > "$work/build.log" 2>&1 || {
    tail -40 "$work/build.log" >&2; exit 1;
}
if grep -q 'Unit run-u.*scope was already loaded' "$work/build.log"; then
    echo "scheduler executed no build" >&2; exit 1
fi
status=0
"$repo/target/release/dclutch-journey-campaign" dealer \
    --transcript "$work/transcript.json" --work "$work/campaign" --rpc-port "$port" \
    --checked-release-gate "$gate" --expected-gate-sha256 "$gate_sha" \
    --expected-source-revision "$revision" --expected-source-tree-sha256 "$tree" \
    --seed "$seed" --holders 1 > "$work/campaign.stdout" 2> "$work/campaign.stderr" || status=$?
if [ -f "$work/transcript.json" ]; then
    jq '{completed,wall,stages:[.stages[]|{stage,outcome}]}' "$work/transcript.json"
else
    echo "campaign emitted no transcript" >&2
    status=1
fi
[ "$status" = 0 ] || tail -30 "$work/campaign.stderr" >&2
exit "$status"
