#!/usr/bin/env bash
# JRNY: one Market's WHOLE life on a validator this runner stands up itself.
#
#   gate -> archive -> campaign binary -> checked-mutable substrate (ONE
#   validator) -> market -> founding -> the whole life -> witnesses -> census
#
# THE SUBSTRATE IS THIS TIER'S OWN, and that is the change of 2026-09-06.
# Until then this runner REFUSED to start without `--market PATH`, because a
# Market can only be compiled by `DirectMarketCompilerOwnedV1::load_local`,
# which authenticates a checked local mutable plan and observes a LIVE loopback
# deployment first -- and nothing here could produce one. The banner said the
# ordering `prepare -> boot -> administer -> compile -> found -> keep going` was
# "its own unit"; this is that unit. `tools/gauntlet/relayed-vertical/src/
# substrate.rs` already implemented the bring-up and `tools/gauntlet/ladder/`
# already proved a tier can drive several shipped commands against one live
# child; the campaign links that file rather than forking it.
#
# TWO THINGS WENT AWAY WITH IT, and neither is missing:
#
#   * the seven SBF builds and the offline program-id derivation. The CHECKED
#     RELEASE GATE supplies the artifacts and `local-mutable-prepare-v1` derives
#     the identities from it, which is the same substitution the ladder made.
#   * the frame-diagnostics exemption file. `cargo build-sbf` exits ZERO when
#     the SBF backend reports that a call overwrites its own stack frame, and
#     this runner used to count those lines itself. It no longer builds the
#     ELFs, so it no longer can -- and it no longer needs to: a
#     CHECKED_UPGRADE_GATE.json is emitted ONLY in strict mode, and strict mode
#     refuses a nonzero diagnostic count. A gate that exists IS the zero-
#     diagnostic proof TIERS.md asks a tier's build stage to make.
#
# WHAT THIS PRODUCES IS LOCAL-VALIDATOR EVIDENCE at the exact revision the gate
# names. Not devnet, not mainnet. Everything runs on 127.0.0.1; nothing here
# signs with a persisted key outside its own --work root, funds an external
# account, publishes, or observes any public cluster.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: tools/gauntlet/journey/run-journey.sh --checked-release-gate PATH [options]

  --checked-release-gate PATH
                        REQUIRED. A CHECKED_UPGRADE_GATE.json built by
                        tools/release/checked-release-candidate.sh. The gate is
                        this tier's build stage: it is emitted ONLY in strict
                        mode, and strict mode refuses a nonzero SBF
                        stack-frame-overwrite diagnostic count.

                          tools/release/checked-release-candidate.sh \
                              --work DIR --commit REV --genesis-cohort \
                              --node ABS/node --node-archive ABS/node.tar.xz

  --repo PATH           source repository (default: this script's repository)
  --worktree            build the campaign from the WORKING TREE instead of from
                        `git archive` of the gate's revision. DEVELOPMENT MODE:
                        the transcript's own revision and the gate's then
                        differ, and a campaign whose host code no commit names
                        is not release evidence.
  --rpc-port PORT|auto  validator base (default: auto; a free 42-port block)
  --work PATH           scratch root (default: /private/tmp/dclutch-journey)
  --holders N           synthetic holder count, the load knob (default: 4)
  --census              fold this run's evidence into the shared census ledger
  --gauntlet-work PATH  the shared gauntlet root whose inventory and ledger the
                        census fold reads (default: /private/tmp/dclutch-gauntlet)
  -h, --help            this page, and nothing else runs

Run `tools/gauntlet/run.sh --mode census` first if there is no inventory yet;
it takes seconds and needs no chain.
USAGE
}

die() { printf 'journey: %s\n' "$*" >&2; exit 1; }
say() { printf '\n== %s\n' "$*"; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAUNTLET="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="$(cd "$GAUNTLET/../.." && pwd)"
GATE=""
RPC_PORT="auto"
WORK="/private/tmp/dclutch-journey"
WORKTREE=0
HOLDERS="4"
CENSUS=0
GAUNTLET_WORK="/private/tmp/dclutch-gauntlet"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --checked-release-gate) GATE="${2:?--checked-release-gate needs a value}"; shift 2 ;;
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --worktree) WORKTREE=1; shift ;;
        --rpc-port) RPC_PORT="${2:?--rpc-port needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --holders) HOLDERS="${2:?--holders needs a value}"; shift 2 ;;
        --census) CENSUS=1; shift ;;
        --gauntlet-work) GAUNTLET_WORK="${2:?--gauntlet-work needs a value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

[ -n "$GATE" ] || { usage >&2; die "--checked-release-gate is required"; }
case "$GATE" in /*) ;; *) die "--checked-release-gate must be an absolute path" ;; esac
[ -f "$GATE" ] || die "--checked-release-gate is not a readable file: $GATE"
case "$WORK" in /*) ;; *) die "--work must be absolute" ;; esac
case "$HOLDERS" in ''|*[!0-9]*) die "--holders must be a decimal count" ;; esac
[ "$HOLDERS" -gt 0 ] || die "--holders must be positive"
if [ "$RPC_PORT" != "auto" ]; then
    case "$RPC_PORT" in ''|*[!0-9]*) die "--rpc-port must be a decimal port or 'auto'" ;; esac
    [ "$RPC_PORT" -ge 1024 ] && [ "$RPC_PORT" -le 65494 ] \
        || die "--rpc-port must be 1024-65494 so the launcher's 42-port block fits under 65535"
fi

for tool in git jq shasum python3 cargo solana-test-validator; do
    command -v "$tool" >/dev/null 2>&1 || die "required command not found: $tool"
done
# hbox is co-tenant with another project's build. Containment is structural.
if command -v swarm-build >/dev/null 2>&1; then WRAP="swarm-build"; else WRAP=""; fi
run_build() { if [ -n "$WRAP" ]; then "$WRAP" "$@"; else "$@"; fi; }

# ------------------------------------------------------------- the ledger lock
#
# `census observe` is a READ-MODIFY-WRITE of one JSON file every family runner
# defaults to sharing, so two concurrent folds would silently lose one side's
# observations -- a corruption that looks exactly like "that route never
# executed". `mkdir` is the lock because it is atomic on every filesystem this
# runs on and needs no flock(1), which macOS does not ship.
ledger_lock() {
    local lock="$1.lock" waited=0
    while ! mkdir "$lock" 2>/dev/null; do
        if [ "$waited" -ge 300 ]; then
            echo "breaking a ledger lock held over 5 minutes: $lock" >&2
            rm -rf "$lock"; continue
        fi
        [ "$waited" = 0 ] && echo "waiting for the ledger lock at $lock" >&2
        sleep 1; waited=$((waited + 1))
    done
    printf '%s\n' "$$" > "$lock/pid"; LEDGER_LOCK_HELD="$lock"
}
ledger_unlock() { [ -n "${LEDGER_LOCK_HELD:-}" ] && rm -rf "$LEDGER_LOCK_HELD"; LEDGER_LOCK_HELD=""; }
trap ledger_unlock EXIT

mkdir -p "$WORK"/{logs,runs}
LOGS="$WORK/logs"

# ------------------------------------------------------------------ 1. the gate
# THE GATE NAMES THE REVISION, and the campaign is built from that exact
# revision rather than from whatever the working tree happens to hold. A
# campaign whose host code and whose ELFs come from two commits is measuring a
# pair nobody can reproduce.
GATE_SHA256="$(sha256 "$GATE")"
GATE_REVISION="$(jq -r '.source_revision' "$GATE")"
GATE_TREE="$(jq -r '.source_tree_sha256' "$GATE")"
printf '%s\n' "$GATE_REVISION" | grep -Eq '^[0-9a-f]{40}$' || die "gate names no 40-hex source revision"
printf '%s\n' "$GATE_TREE" | grep -Eq '^[0-9a-f]{64}$' || die "gate names no 64-hex source tree digest"
say "journey at gate $GATE_SHA256, revision $GATE_REVISION (holders=$HOLDERS)"

SOURCE="$WORK/source"
if [ "$WORKTREE" = 1 ]; then
    SOURCE="$REPO"
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
# The tier's own files, which may not be in the gate's revision yet. Copied
# rather than symlinked so the built binary names one directory.
if [ "$WORKTREE" != 1 ]; then
    rm -rf "$SOURCE/tools/gauntlet/journey"
    mkdir -p "$SOURCE/tools/gauntlet"
    cp -R "$SCRIPT_DIR" "$SOURCE/tools/gauntlet/journey"
fi

# ------------------------------------------------------------- 2. the campaign
HOST_TARGET="$WORK/host-target"
say "stage campaign binary"
( cd "$SOURCE" && CARGO_TARGET_DIR="$HOST_TARGET" \
    run_build cargo build --release -p dclutch-journey-campaign ) \
    > "$LOGS/build-journey.log" 2>&1 \
    || { tail -n 40 "$LOGS/build-journey.log" >&2; die "campaign build failed"; }
JOURNEY_BIN="$HOST_TARGET/release/dclutch-journey-campaign"
[ -x "$JOURNEY_BIN" ] || die "journey binary missing: $JOURNEY_BIN"

# ------------------------------------------------------------ 3. the port block
#
# Deliberately NOT `bind(0)`. The kernel's ephemeral range is the range it also
# hands to every ordinary outbound connection, so a base drawn from it is the
# most likely port on the machine to be stolen out from under a validator. And
# it is resolved LATE, immediately before the campaign, because a base chosen at
# argument-parse time is a build away from being used -- both halves measured,
# not theorised.
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
[ "$BASE" = "auto" ] && { BASE="$(allocate_rpc_port)" || die "--rpc-port auto: no free 42-port block"; }

RUN="$WORK/runs/$(date -u '+%Y%m%dT%H%M%SZ')-${GATE_REVISION:0:12}-h$HOLDERS"
mkdir -p "$RUN"
say "campaign: living one Market's whole life (validator base $BASE, run $RUN)"

# The prepare seed: a STATED derivation rather than a number somebody typed. It
# is safe here and ONLY here -- the producer refuses the flag outright unless the
# endpoint is loopback, and this tier only ever names a 127.0.0.1 origin.
SEED="$(printf '%s' 'dclutch/gauntlet/journey/campaign-seed/v1' | shasum -a 256 | cut -d' ' -f1)"

JOURNEY_ARGS=(run
    --transcript "$RUN/transcript.json"
    --work "$RUN/campaign"
    --rpc-port "$BASE"
    --checked-release-gate "$GATE"
    --expected-gate-sha256 "$GATE_SHA256"
    --expected-source-revision "$GATE_REVISION"
    --expected-source-tree-sha256 "$GATE_TREE"
    --seed "$SEED"
    --holders "$HOLDERS")

STATUS=0
if ! "$JOURNEY_BIN" "${JOURNEY_ARGS[@]}" \
        > "$RUN/campaign.stdout" 2> "$RUN/campaign.stderr"; then
    tail -n 60 "$RUN/campaign.stderr" >&2
    echo "journey: campaign FAILED; evidence, transcript and logs are under $RUN" >&2
    STATUS=1
fi
printf '%s\n' "$RUN" > "$WORK/last-run"

TRANSCRIPT="$RUN/transcript.json"
EVIDENCE="$RUN/campaign/evidence.json"
# A campaign that met a wall still writes both documents -- that is the whole
# design -- so the fold below runs on a failed run too, and only the exit code
# says the run failed.
[ -f "$TRANSCRIPT" ] || die "campaign transcript missing: $TRANSCRIPT"
[ -f "$EVIDENCE" ] || die "campaign evidence missing: $EVIDENCE"

# ---------------------------------------------------------------- 4. the ledger
say "conservation ledger"
jq -r '
    "verdict: \(.conservation_verdict)   holders: \(.holder_count)   claim unit: \(.claim_unit_atoms) atoms",
    "",
    (.observations[] |
       "  \(.stage)",
       "    collateral tracked \(.tracked_collateral) of supply \(.mint_supply); Hoard \(.hoard_atoms)",
       "    aggregate supply \(.aggregate_supply)   Positions \(.position_totals)",
       (.verdicts[] | "      \(.law) \(.status)  \(.detail)"))
' "$TRANSCRIPT"

say "stages"
jq -r '.stages[] | "  \(.outcome | ascii_upcase)  \(.stage)  [\(.transactions) tx, \(.compute_units) CU]"' \
    "$TRANSCRIPT"

# ------------------------------------------------------------- 5. witnesses
say "witnesses: tier 1's, against this campaign's evidence"
"$GAUNTLET/tier1/check-witnesses.sh" "$GAUNTLET/tier1/witnesses.json" \
    "$EVIDENCE" "$RUN/campaign/substrate/plan.json" || STATUS=1
say "witnesses: the journey's, against its transcript"
"$GAUNTLET/tier1/check-witnesses.sh" "$SCRIPT_DIR/witnesses.json" \
    "$EVIDENCE" "$TRANSCRIPT" || STATUS=1

# ---------------------------------------------------------------- 6. census
if [ "$CENSUS" = 1 ]; then
    say "census"
    INVENTORY="$GAUNTLET_WORK/out/inventory.json"
    LEDGER="$GAUNTLET_WORK/out/ledger.json"
    [ -f "$INVENTORY" ] || die "--census needs $INVENTORY; run 'tools/gauntlet/run.sh --mode census' first"
    jq '{registry:.registry.program_id, core:.core.program_id, claims:.claims.program_id,
         trading:.trading.program_id, resolution:.resolution.program_id,
         custody:.custody.program_id, rent:.rent_credit.program_id}' \
        "$RUN/campaign/substrate/plan.json" > "$RUN/programs.json"
    # One copy of tier 1's bindings, merged at run time: the journey submits
    # every founding transaction the infrastructure floor does before its own,
    # and a second hand-maintained copy of them would rot the first time tier 1
    # changed. There is exactly one copy.
    jq -s '{campaign: "journey",
            note: ("Whole-life journey. Tier 1'"'"'s bindings are merged in at run time from " +
                   "tools/gauntlet/tier1/bindings.json because the journey submits every " +
                   "founding transaction before its own; there is exactly one copy of them."),
            bindings: (.[0].bindings + .[1].bindings)}' \
        "$GAUNTLET/tier1/bindings.json" "$SCRIPT_DIR/bindings.json" > "$RUN/bindings.json"
    ledger_lock "$LEDGER"
    cargo run --quiet --manifest-path "$GAUNTLET/census/Cargo.toml" -- observe \
        --inventory "$INVENTORY" --ledger "$LEDGER" \
        --bindings "$RUN/bindings.json" --programs "$RUN/programs.json" \
        --evidence "$EVIDENCE" || STATUS=1
    ledger_unlock
fi

say "done"
echo "evidence:   $EVIDENCE"
echo "transcript: $TRANSCRIPT"
echo "journey: render the report with 'tools/gate census'"
exit "$STATUS"
