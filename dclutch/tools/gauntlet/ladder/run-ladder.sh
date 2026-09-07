#!/usr/bin/env bash
# LADDER: a market's funded ordered recovery ladder, on ONE live validator.
#
#   archive -> campaign binary -> per arm: checked-mutable substrate
#     (ONE validator) -> two-source market -> founding -> crank
#     -> witnesses -> census
#
# BOTH ARMS BY DEFAULT, one at a time. The exhaust arm's witnesses and the
# capture arm's witnesses are about different walks, and a witness evaluated
# against the walk that did not run reads its default -- which is an absent
# signal with no way to tell "nothing fired" from "my instrument was
# disconnected". Running both is the only version where every witness of this
# tier bites on some transcript of every run. `--walk exhaust` and
# `--walk capture` still select one arm, and then that run covers one arm and
# the other arm's witnesses are simply not evaluated rather than green.
#
# ONE validator per arm, loopback, on a per-run port block; the campaign owns
# its validator's lifetime and kills it on exit, so the arms reuse the block in
# turn. Nothing here signs with a persisted key outside its own --work root,
# funds an external account, publishes, or observes any public cluster.
#
# WHAT THIS PRODUCES IS LOCAL-VALIDATOR EVIDENCE at the exact revision the
# checked release gate names.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: tools/gauntlet/ladder/run-ladder.sh --checked-release-gate PATH [options]

  --walk MODE           exhaust | capture | both   (default: both)
                        `both` runs the two arms in turn against one port
                        block, each with its own validator, market, transcript
                        and witness evaluation. See the note at the top of this
                        file for why that is the default.
  --checked-release-gate PATH
                        REQUIRED. A CHECKED_UPGRADE_GATE.json built by
                        tools/release/checked-release-candidate.sh. The gate is
                        the tier's build stage: it is emitted ONLY in strict
                        mode, and strict mode refuses a nonzero SBF
                        stack-frame-overwrite diagnostic count, so a gate that
                        exists IS the zero-diagnostic proof TIERS.md asks a
                        tier's build stage to make.

                          tools/release/checked-release-candidate.sh \
                              --work DIR --commit REV --genesis-cohort \
                              --node ABS/node --node-archive ABS/node.tar.xz

  --repo PATH           source repository (default: this script's repository)
  --worktree            build the campaign from the WORKING TREE instead of
                        from `git archive` of the gate's revision. DEVELOPMENT
                        MODE: the transcript's own revision and the gate's then
                        differ, and a campaign whose host code no commit names
                        is not release evidence. Needed while a producer change
                        the tier depends on is newer than the newest revision a
                        strict-mode gate can be built at.
  --rpc-port PORT|auto  validator base (default: auto; a free 42-port block)
  --work PATH           scratch root (default: /private/tmp/dclutch-ladder)
  --recovery-rungs SPEC BPS:SECONDS_AFTER_PREVIOUS, comma separated
                        (default: the tier's own one-rung two-source market)
  --max-wait-seconds N  the whole budget a walk may spend waiting for a leg's
                        deadline (default: 600). A leg further away than this
                        is REPORTED, never slept for and never warped past.
  --publication-shelf-life-seconds N
                        how old the publication this run MINTS may be before
                        the transport refuses it (default: 1200), and therefore
                        how far past the mint instant the market's primary leg
                        falls due. A parameter of this TIER and of the lab that
                        holds the publication; never a market's staleness
                        policy, which is a thing a founder authors.
  --census              fold this run's evidence into the shared census ledger
  --gauntlet-work PATH  the shared gauntlet root whose inventory and ledger the
                        census fold reads (default: /private/tmp/dclutch-gauntlet)
  -h, --help            this page, and nothing else runs. Named here because a
                        help page that does not admit its own help flag is a
                        page a reader cannot verify they typed correctly, and
                        because `docs/operators/found-a-market.md` publishes
                        exactly this command.
USAGE
}

die() { echo "ladder: $1" >&2; exit 1; }
say() { printf '\n== %s\n' "$1"; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAUNTLET="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="$(cd "$GAUNTLET/../.." && pwd)"
WALK="both"
GATE=""
RPC_PORT="auto"
WORK="/private/tmp/dclutch-ladder"
WORKTREE=0
RUNGS=""
MAX_WAIT="600"
SHELF_LIFE="1200"
CENSUS=0
GAUNTLET_WORK="/private/tmp/dclutch-gauntlet"

while [ $# -gt 0 ]; do
    case "$1" in
        --walk) WALK="${2:?}"; shift 2 ;;
        --checked-release-gate) GATE="${2:?}"; shift 2 ;;
        --repo) REPO="${2:?}"; shift 2 ;;
        --worktree) WORKTREE=1; shift ;;
        --rpc-port) RPC_PORT="${2:?}"; shift 2 ;;
        --work) WORK="${2:?}"; shift 2 ;;
        --recovery-rungs) RUNGS="${2:?}"; shift 2 ;;
        --max-wait-seconds) MAX_WAIT="${2:?}"; shift 2 ;;
        --publication-shelf-life-seconds) SHELF_LIFE="${2:?}"; shift 2 ;;
        --census) CENSUS=1; shift ;;
        --gauntlet-work) GAUNTLET_WORK="${2:?}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done
case "$WALK" in
    exhaust) ARMS=(exhaust) ;;
    capture) ARMS=(capture) ;;
    both)    ARMS=(exhaust capture) ;;
    *) die "--walk must be exhaust, capture or both" ;;
esac
[ -n "$GATE" ] || { usage >&2; die "--checked-release-gate is required"; }
[ -f "$GATE" ] || die "--checked-release-gate is not a readable file: $GATE"
case "$GATE" in /*) ;; *) die "--checked-release-gate must be an absolute path" ;; esac

for tool in git jq shasum python3 cargo solana-test-validator; do
    command -v "$tool" >/dev/null 2>&1 || die "required command not found: $tool"
done
if command -v swarm-build >/dev/null 2>&1; then WRAP="swarm-build"; else WRAP=""; fi
run_build() { if [ -n "$WRAP" ]; then "$WRAP" "$@"; else "$@"; fi; }

ledger_lock() {
    local lock="$1.lock" waited=0
    while ! mkdir "$lock" 2>/dev/null; do
        [ "$waited" -ge 300 ] && { echo "breaking a ledger lock held over 5 minutes: $lock" >&2; rm -rf "$lock"; continue; }
        [ "$waited" = 0 ] && echo "waiting for the ledger lock at $lock" >&2
        sleep 1; waited=$((waited + 1))
    done
    printf '%s\n' "$$" > "$lock/pid"; LEDGER_LOCK_HELD="$lock"
}
ledger_unlock() { [ -n "${LEDGER_LOCK_HELD:-}" ] && rm -rf "$LEDGER_LOCK_HELD"; LEDGER_LOCK_HELD=""; }
trap ledger_unlock EXIT

mkdir -p "$WORK"/{logs,runs}
LOGS="$WORK/logs"

# ----------------------------------------------------------- 1. the gate
# THE GATE NAMES THE REVISION, and the campaign is built from that exact
# revision rather than from whatever the working tree happens to hold. A
# campaign whose host code and whose ELFs come from two commits is measuring a
# pair nobody can reproduce.
GATE_SHA256="$(sha256 "$GATE")"
GATE_REVISION="$(jq -r '.source_revision' "$GATE")"
GATE_TREE="$(jq -r '.source_tree_sha256' "$GATE")"
printf '%s\n' "$GATE_REVISION" | grep -Eq '^[0-9a-f]{40}$' || die "gate names no 40-hex source revision"
printf '%s\n' "$GATE_TREE" | grep -Eq '^[0-9a-f]{64}$' || die "gate names no 64-hex source tree digest"
say "gate $GATE_SHA256 at $GATE_REVISION"

SOURCE="$WORK/source"
CAMPAIGN_BUILD_MODE="checked-gate-archive"
CAMPAIGN_HOST_REVISION="$GATE_REVISION"
if [ "$WORKTREE" = 1 ]; then
    SOURCE="$REPO"
    CAMPAIGN_BUILD_MODE="working-tree-diagnostic"
    CAMPAIGN_HOST_REVISION="$(git -C "$REPO" rev-parse HEAD)"
    say "building the campaign from the WORKING TREE (development mode; not release evidence)"
elif [ ! -f "$WORK/stamps.archive" ] || [ "$(cat "$WORK/stamps.archive")" != "$GATE_REVISION" ]; then
    say "stage archive ($GATE_REVISION)"
    rm -rf "$SOURCE"; mkdir -p "$SOURCE"
    git -C "$REPO" archive "$GATE_REVISION" | tar -x -C "$SOURCE" \
        || die "the gate's revision is not in this repository: $GATE_REVISION"
    printf '%s\n' "$GATE_REVISION" > "$WORK/stamps.archive"
else
    echo "stage archive: up to date"
fi
# The campaign stays inside the gate's exact archive. Overlaying this runner's
# working-tree modules here would silently change what the source revision
# names. Developing a newer host against an older gate uses --worktree, whose
# evidence is explicitly diagnostic.

# --------------------------------------------------------- 2. the campaign
HOST_TARGET="$WORK/host-target"
say "stage campaign binary"
( cd "$SOURCE" && CARGO_TARGET_DIR="$HOST_TARGET" \
    run_build cargo build --release -p dclutch-ladder-campaign ) > "$LOGS/build-ladder.log" 2>&1 \
    || { tail -n 40 "$LOGS/build-ladder.log" >&2; die "campaign build failed"; }
CAMPAIGN_BIN="$HOST_TARGET/release/dclutch-ladder-campaign"
[ -x "$CAMPAIGN_BIN" ] || die "campaign binary missing: $CAMPAIGN_BIN"

# -------------------------------------------------------- 3. the port block
allocate_rpc_port() {
    python3 - "$$" <<'PY'
import socket, sys
BAND_LOW, BAND_HIGH, STRIDE = 21000, 48000, 64
count = (BAND_HIGH - BAND_LOW) // STRIDE
start = int(sys.argv[1]) % count
for step in range(count):
    base = BAND_LOW + ((start + step) % count) * STRIDE
    held = []
    try:
        for offset in (0, 2, 3, *range(10, 42)):
            member = socket.socket(); member.bind(("127.0.0.1", base + offset)); held.append(member)
    except OSError:
        for sock in held: sock.close()
        continue
    for sock in held: sock.close()
    print(base); break
else:
    raise SystemExit("no free 42-port block in 21000-48000 on 127.0.0.1")
PY
}

BASE="$RPC_PORT"
[ "$BASE" = "auto" ] && BASE="$(allocate_rpc_port)"
RUN="$WORK/runs/$(date -u '+%Y%m%dT%H%M%SZ')-$WALK"
mkdir -p "$RUN"
say "walk: $WALK (arms: ${ARMS[*]}; validator base $BASE, run $RUN)"
printf '%s\n' "$RUN" > "$WORK/last-run"

# The prepare seed: a STATED derivation rather than a number somebody typed.
SEED="$(printf 'dclutch/gauntlet/ladder/prepare-seed/v1' | shasum -a 256 | cut -d' ' -f1)"

STATUS=0
for ARM in "${ARMS[@]}"; do
    say "arm: $ARM"
    ARM_RUN="$RUN/$ARM"
    mkdir -p "$ARM_RUN"
    ARGS=(run --walk "$ARM" --transcript "$ARM_RUN/transcript.json" --work "$ARM_RUN/campaign"
          --rpc-port "$BASE" --checked-release-gate "$GATE"
          --expected-gate-sha256 "$GATE_SHA256"
          --expected-source-revision "$GATE_REVISION"
          --expected-source-tree-sha256 "$GATE_TREE"
          --seed "$SEED" --max-wait-seconds "$MAX_WAIT"
          --publication-shelf-life-seconds "$SHELF_LIFE")
    [ -n "$RUNGS" ] && ARGS+=(--recovery-rungs "$RUNGS")

    # An arm that fails STOPS THE RUN. The two arms are about the same market
    # shape on two histories, so an exhaust arm that could not reach its
    # terminal is not a reason to go on and ask whether a rung answers.
    if ! "$CAMPAIGN_BIN" "${ARGS[@]}" > "$ARM_RUN/campaign.stdout" 2> "$ARM_RUN/campaign.stderr"; then
        tail -n 40 "$ARM_RUN/campaign.stderr" >&2
        echo "ladder: $ARM arm FAILED; artifacts under $ARM_RUN" >&2
        exit 1
    fi
    [ -f "$ARM_RUN/transcript.json" ] || die "transcript missing: $ARM_RUN/transcript.json"
    [ -f "$ARM_RUN/campaign/evidence.json" ] || die "evidence missing: $ARM_RUN/campaign/evidence.json"
    TRANSCRIPT_TMP="$ARM_RUN/transcript.json.tmp"
    jq --arg mode "$CAMPAIGN_BUILD_MODE" --arg revision "$CAMPAIGN_HOST_REVISION" \
       --arg gate "$GATE_REVISION" \
       '. + {campaign_build:{mode:$mode, source_revision:$revision, gate_revision:$gate}}' \
       "$ARM_RUN/transcript.json" > "$TRANSCRIPT_TMP" \
       || die "could not label campaign build provenance"
    mv "$TRANSCRIPT_TMP" "$ARM_RUN/transcript.json"

    # The arm-independent witnesses, then the ones about THIS arm. The second
    # file is the whole point of running both: its witnesses carry no disjunct
    # over `.walk`, so each of them is red on the arm it is not about and green
    # only on a transcript of the walk it names.
    say "witnesses (shared)"
    "$GAUNTLET/tier1/check-witnesses.sh" "$SCRIPT_DIR/witnesses.json" \
        "$ARM_RUN/campaign/evidence.json" "$ARM_RUN/transcript.json" || STATUS=1
    say "witnesses ($ARM)"
    "$GAUNTLET/tier1/check-witnesses.sh" "$SCRIPT_DIR/witnesses-$ARM.json" \
        "$ARM_RUN/campaign/evidence.json" "$ARM_RUN/transcript.json" || STATUS=1

    if [ "$CENSUS" = 1 ]; then
        say "census fold ($ARM)"
        INVENTORY="$GAUNTLET_WORK/out/inventory.json"
        CENSUS_LEDGER="$GAUNTLET_WORK/out/ledger.json"
        [ -f "$INVENTORY" ] || die "--census needs $INVENTORY; run 'tools/gauntlet/run.sh --mode census' first"
        jq '{registry:.registry.program_id, core:.core.program_id, claims:.claims.program_id,
             trading:.trading.program_id, resolution:.resolution.program_id,
             custody:.custody.program_id, rent:.rent_credit.program_id}' \
            "$ARM_RUN/campaign/substrate/plan.json" > "$ARM_RUN/programs.json"
        ledger_lock "$CENSUS_LEDGER"
        cargo run --quiet --manifest-path "$GAUNTLET/census/Cargo.toml" -- observe \
            --inventory "$INVENTORY" --ledger "$CENSUS_LEDGER" \
            --bindings "$SCRIPT_DIR/bindings.json" --programs "$ARM_RUN/programs.json" \
            --evidence "$ARM_RUN/campaign/evidence.json" || STATUS=1
        ledger_unlock
    fi

    say "$ARM arm transcript summary"
    jq -r '
        "walk: \(.walk)   market: \(.market)   rungs: \(.recovery_rungs)",
        (.stages[] | "  \(.outcome | ascii_upcase)  \(.stage)"),
        (.cranks[] | "  crank seq \(.sequence): \(.outcome)\(if .secondsUntilDue then "  (due in \(.secondsUntilDue)s)" else "" end)\(if .computeUnitsConsumed then "  \(.computeUnitsConsumed) CU" else "" end)")
    ' "$ARM_RUN/transcript.json"
done
exit "$STATUS"
