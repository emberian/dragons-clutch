#!/usr/bin/env bash
# The dClutch functional gauntlet's census entry point and tier-1 runner.
#
#   census (seconds, no chain)
#   build -> deploy -> tier-1 campaign -> census (full)
#
# Census mode produces no chain evidence. The campaign is limited to
# LOCAL-VALIDATOR EVIDENCE: never devnet or mainnet evidence, and never a
# persisted signer, external account, publication, or remote deploy.
#
# Read tools/gauntlet/DESIGN.md before changing anything here, and TIERS.md
# before adding a tier.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: tools/gauntlet/run.sh [options]

  --repo PATH      source repository (default: this script's repository)
  --work PATH      scratch root (default: /private/tmp/dclutch-gauntlet)
  --commit REV     source revision to archive and build (default: HEAD)
  --mode MODE      census | full   (default: full)
                     census  static enumeration + report only, seconds, no chain
                     full    build seven ELFs, launch a localhost validator, run
                             the tier-1 campaign, fold it into the census.
                             MEASURED 2026-09-03 on an M-series laptop: 18m01s of
                             campaign (195 transactions) after 7m of archive,
                             build, tool and inventory with a warm --work; a cold
                             --work adds ~6m of SBF builds. Budget 25-31 minutes,
                             and read TIERS.md before treating a run as evidence:
                             the campaign does not currently complete.
  --from STAGE     force a restart at a stage: archive|elf|tool|inventory|
                   campaign|census  (later stages always re-run)
  --keep-runs      keep every campaign run directory (default: newest three)
  --rpc-port PORT|auto
                   validator RPC base; `auto` takes a free 42-port block.
                   Also readable from $DCLUTCH_GAUNTLET_RPC_PORT.
  --record-publication genesis|transaction
                   where the infrastructure record bodies come from
  --allow-stale-fixture-pins
                   proceed when a fixture pin no longer matches the tree
  -h, --help       show this message

Stages are stamped by their exact inputs under --work/stamps. A re-run skips a
stage whose stamp matches and re-runs everything downstream of the first stage
that does not.

Tier 1 has no external Market producer and does not borrow one. `demo-market`
is retired, `devnet-market` and `graduation-market` require acknowledged
external facts and a fee policy this runner does not own, and the successor's
loopback planner authenticates a checked-MUTABLE plan while tier 1 is the
immutable release set -- it refuses immutable-Core semantics by name. So the
spec this script assembles OMITS `market`, and the supervisor compiles a
fixture input from the plan it builds (`SuccessorRunSpec::market`). That path
is loopback-only by construction and its one invented fact, a zero-basis-point
Direct fee to the Registry address, is declared where it is made.

Tier 1 cannot be made available by substituting ProgramTest: it depends on
genesis Loader-v3 ProgramData spans, a real Loader SetAuthority(Some -> None),
and real per-transaction compute. Family tiers may declare a fast lane; see
TIERS.md for the bar.
USAGE
}

die() { printf 'gauntlet: %s\n' "$*" >&2; exit 1; }

REPO=""
WORK="/private/tmp/dclutch-gauntlet"
COMMIT="HEAD"
MODE="full"
FROM=""
KEEP_RUNS="false"
ALLOW_STALE_PINS="false"
RECORD_PUBLICATION="genesis"
RPC_PORT="${DCLUTCH_GAUNTLET_RPC_PORT:-20890}"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --commit) COMMIT="${2:?--commit needs a value}"; shift 2 ;;
        --mode) MODE="${2:?--mode needs a value}"; shift 2 ;;
        --from) FROM="${2:?--from needs a value}"; shift 2 ;;
        --keep-runs) KEEP_RUNS="true"; shift ;;
        --allow-stale-fixture-pins) ALLOW_STALE_PINS="true"; shift ;;
        --record-publication) RECORD_PUBLICATION="${2:?--record-publication needs a value}"; shift 2 ;;
        --rpc-port) RPC_PORT="${2:?--rpc-port needs a value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

case "$MODE" in census|full) ;; *) echo "--mode must be census or full" >&2; exit 2 ;; esac

if [ -z "$REPO" ]; then
    REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi
case "$WORK" in /*) ;; *) echo "--work must be absolute" >&2; exit 2 ;; esac
case "$RECORD_PUBLICATION" in genesis|transaction) ;; *) echo "--record-publication must be genesis or transaction" >&2; exit 2 ;; esac

# ------------------------------------------------------------------- staging
#
# The stage names are validated HERE, beside the other argument checks and
# before anything is created, because `--from` is an argument like `--mode`:
# refusing it after `mkdir -p "$WORK"` has built a scratch tree is refusing it
# too late, and it exited 1 where every sibling exits 2.
STAGE_ORDER="archive elf tool inventory campaign census"
FORCED=""
stage_index() {
    local wanted=$1 index=0 stage
    for stage in $STAGE_ORDER; do
        if [ "$stage" = "$wanted" ]; then printf '%s\n' "$index"; return 0; fi
        index=$((index + 1))
    done
    printf '99\n'
}
if [ -n "$FROM" ]; then
    if [ "$(stage_index "$FROM")" = "99" ]; then
        echo "--from must name a stage: $STAGE_ORDER" >&2
        exit 2
    fi
    FORCED="$(stage_index "$FROM")"
fi


# ------------------------------------------------------------------ the origin
#
# `--rpc-port` is why two of these can run at once. The origin is in NO
# authenticated material -- not in the keypair derivation, not in a program
# address, not in a semantic release ID, not in an artifact attestation, not in
# the genesis plan -- so moving it moves nothing a budget row or a witness reads.
# What it does move is the ledger, the run directory and the port block, which
# is exactly the contention it exists to end.
#
# 20890 stays the default and reproduces the historical run byte for byte.
# `auto` is resolved LATE, at the campaign stage, by allocate_rpc_port below.
if [ "$RPC_PORT" != "auto" ]; then
    case "$RPC_PORT" in
        ''|*[!0-9]*) echo "--rpc-port must be a decimal port or 'auto'" >&2; exit 2 ;;
    esac
    [ "$RPC_PORT" -ge 1024 ] && [ "$RPC_PORT" -le 65494 ] || {
        echo "--rpc-port must be 1024-65494 so the launcher's 42-port block fits under 65535" >&2
        exit 2
    }
fi

GAUNTLET="$REPO/tools/gauntlet"
SOURCE="$WORK/source"
ELF_DIR="$WORK/elf"
BUILD_TARGET="$WORK/build-target"
HOST_TARGET="$WORK/host-target"
CENSUS_TARGET="$WORK/census-target"
STAMPS="$WORK/stamps"
OUT="$WORK/out"
LOGS="$WORK/logs"
RUNS="$WORK/runs"
CENSUS_BIN="$CENSUS_TARGET/release/dclutch-route-census"
BOOTSTRAP_BIN="$HOST_TARGET/release/dclutch-local-successor-bootstrap"

INVENTORY="$OUT/inventory.json"
LEDGER="$OUT/ledger.json"
REPORT="$OUT/CENSUS.md"
BLOCKED="$GAUNTLET/blocked.json"
BINDINGS="$GAUNTLET/tier1/bindings.json"
WITNESSES="$GAUNTLET/tier1/witnesses.json"

mkdir -p "$WORK" "$STAMPS" "$OUT" "$LOGS" "$RUNS"

sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
sha256_stdin() { shasum -a 256 | cut -d' ' -f1; }
say() { printf '\n== %s\n' "$*"; }
# Resolve `--rpc-port auto` into a base whose WHOLE 42-port block binds.
#
# Deliberately NOT `bind(0)`. The kernel's ephemeral range is the range it also
# hands to every ordinary outbound connection, so a base drawn from it is the
# most likely port on the machine to be stolen out from under a validator. And
# it is resolved LATE, immediately before the campaign, because a base chosen
# at argument-parse time is six minutes of SBF builds away from being used.
# Both halves are measured, not theorised: the first parallel campaign attempt
# picked ephemeral 49952 at parse time and found it occupied when it got there.
#
# So: a band well below the ephemeral range, a start offset keyed to this
# process so two concurrent runs do not begin at the same candidate, and every
# candidate proved by actually binding all 42 ports at once.
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
            member = socket.socket()
            member.bind(("127.0.0.1", base + offset))
            held.append(member)
    except OSError:
        for sock in held:
            sock.close()
        continue
    for sock in held:
        sock.close()
    print(base)
    break
else:
    raise SystemExit("no free 42-port block in 21000-48000 on 127.0.0.1")
PY
}


# ------------------------------------------------------------- the ledger lock
#
# `census observe` is a READ-MODIFY-WRITE of one JSON file. Every family runner
# and this script default to the same `/private/tmp/dclutch-gauntlet/out/
# ledger.json`, so now that campaigns can run concurrently, two of them folding
# evidence at the same moment would silently lose one side's observations --
# a corruption that looks exactly like "that route never executed".
#
# `mkdir` is the lock because it is atomic on every filesystem this runs on and
# needs no flock(1), which macOS does not ship. The holder's pid is recorded so
# a stale lock names who to look for, and a lock older than the timeout is
# broken rather than deadlocking a lane at 3am.
ledger_lock() {
    local lock="$1.lock" waited=0
    while ! mkdir "$lock" 2>/dev/null; do
        if [ "$waited" -ge 300 ]; then
            local holder=""
            [ -f "$lock/pid" ] && holder="$(cat "$lock/pid" 2>/dev/null || true)"
            echo "ledger lock at $lock held for over 5 minutes by pid ${holder:-unknown}; breaking it" >&2
            rm -rf "$lock"
            continue
        fi
        [ "$waited" = 0 ] && echo "waiting for the ledger lock at $lock" >&2
        sleep 1
        waited=$((waited + 1))
    done
    printf '%s\n' "$$" > "$lock/pid"
    LEDGER_LOCK_HELD="$lock"
}
ledger_unlock() {
    [ -n "${LEDGER_LOCK_HELD:-}" ] && rm -rf "$LEDGER_LOCK_HELD"
    LEDGER_LOCK_HELD=""
}
trap ledger_unlock EXIT


for tool in git jq shasum python3; do
    command -v "$tool" >/dev/null 2>&1 || die "required command not found: $tool"
done

# hbox is co-tenant with codex's HOL build. Containment is structural, not
# polite: every build goes through swarm-build when it exists.
if command -v swarm-build >/dev/null 2>&1; then
    WRAP="swarm-build"
else
    WRAP=""
fi
run_build() { if [ -n "$WRAP" ]; then "$WRAP" "$@"; else "$@"; fi; }

# A stage runs when it is at or after --from, when its stamp differs, or when
# any earlier stage ran in this invocation.
DIRTY_FROM=99
stage_needed() {
    local stage=$1 key=$2 index
    index="$(stage_index "$stage")"
    if [ "$index" -ge "$DIRTY_FROM" ]; then return 0; fi
    if [ -n "$FORCED" ] && [ "$index" -ge "$FORCED" ]; then return 0; fi
    if [ ! -f "$STAMPS/$stage" ]; then return 0; fi
    [ "$(cat "$STAMPS/$stage")" = "$key" ] && return 1 || return 0
}
stage_done() {
    local stage=$1 key=$2 index
    index="$(stage_index "$stage")"
    printf '%s\n' "$key" > "$STAMPS/$stage"
    if [ "$index" -lt "$DIRTY_FROM" ]; then DIRTY_FROM="$index"; fi
}

# --------------------------------------------------------------- 1. archive
SOURCE_REVISION="$(git -C "$REPO" rev-parse "$COMMIT")"
SOURCE_DIGEST="$(git -C "$REPO" ls-tree -r --full-tree "$SOURCE_REVISION" | sha256_stdin)"
say "gauntlet at $SOURCE_REVISION"
echo "repo:   $REPO"
echo "work:   $WORK"
echo "mode:   $MODE"

if stage_needed archive "$SOURCE_DIGEST"; then
    say "stage archive"
    rm -rf "$SOURCE"
    mkdir -p "$SOURCE"
    # `-m` -- extract at NOW, not at the commit's timestamp.
    #
    # `git archive` stamps every file with the commit time, and the build
    # target directory lives outside $SOURCE and survives across commits. So a
    # newly extracted tree whose commit predates the last build looks OLDER
    # than that build's outputs, cargo rebuilds nothing, and the ELF stage
    # copies and digests the PREVIOUS commit's artifact -- replaying its cached
    # warnings and its frame diagnostics as though they were this commit's.
    # Caught 2026-08-30: a fix for seven frame diagnostics reported seven,
    # under an ELF digest identical to the unfixed build's.
    git -C "$REPO" archive "$SOURCE_REVISION" | tar -xm -C "$SOURCE"
    stage_done archive "$SOURCE_DIGEST"
else
    echo "stage archive: up to date"
fi

# ------------------------------------------------------------------ 2. ELFs
# Registry, Core, Claims, Trading, Resolution, Custody, Rent: the seven the
# transaction-only bootstrap binds into the five-role release set plus the
# Registry and RentCredit infrastructure.
ROLES="registry:dclutch-registry-sbf:dclutch_registry_sbf
core:dclutch-core-sbf:dclutch_core_sbf
claims:dclutch-claims-sbf:dclutch_claims_sbf
trading:dclutch-trading-sbf:dclutch_trading_sbf
resolution:dclutch-resolution-proof-sbf:dclutch_resolution_proof_sbf
custody:dclutch-custody-sbf:dclutch_custody_sbf
rent:dclutch-rent-sbf:dclutch_rent_sbf"

# `cargo build-sbf` exits zero even when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution". Count them per role and say so; an artifact the toolchain calls
# potentially-undefined has no business entering a campaign unnoticed.
DIAGNOSTIC_PATTERN='overwrites values in the frame'

if [ "$MODE" = "full" ]; then
    if stage_needed elf "$SOURCE_DIGEST"; then
        say "stage elf"
        command -v cargo-build-sbf >/dev/null 2>&1 || die "cargo-build-sbf not found"
        mkdir -p "$ELF_DIR"
        : > "$WORK/build-diagnostics.txt"
        for entry in $ROLES; do
            role="${entry%%:*}"; rest="${entry#*:}"
            package="${rest%%:*}"; stem="${rest#*:}"
            echo "build: $role ($package)"
            (
                cd "$SOURCE"
                CARGO_TARGET_DIR="$BUILD_TARGET" \
                    run_build cargo build-sbf --manifest-path "programs/$package/Cargo.toml"
            ) > "$LOGS/build-$role.log" 2>&1 \
                || { tail -n 40 "$LOGS/build-$role.log" >&2; die "SBF build failed: $role"; }
            cp "$BUILD_TARGET/deploy/$stem.so" "$ELF_DIR/$role.so"
            count="$(grep -c "$DIAGNOSTIC_PATTERN" "$LOGS/build-$role.log" || true)"
            printf '%s=%s\n' "$role" "$count" >> "$WORK/build-diagnostics.txt"
            # Whether this build actually compiled the crate. cargo replays a
            # cached build's warnings AND the SBF backend's frame errors, so a
            # count from a build that recompiled nothing is the previous
            # commit's answer wearing this one's name. Reported, not refused:
            # on an unchanged source digest, reusing is correct.
            fresh="reused"
            grep -q "Compiling $package " "$LOGS/build-$role.log" && fresh="recompiled"
            printf '  %s  %s (%s frame diagnostics, %s)\n' \
                "$(sha256 "$ELF_DIR/$role.so")" "$role" "$count" "$fresh"
        done
        stage_done elf "$SOURCE_DIGEST"
    else
        echo "stage elf: up to date"
    fi
    # And REFUSE, in aggregate. This was a warning until 2026-08-30, on the
    # reasoning that the campaign is evidence and evidence should still be
    # produced. That reasoning was wrong twice over: evidence gathered on an
    # artifact the toolchain calls potentially-undefined is evidence about
    # nothing in particular, and a warning in the output of a tool that prints
    # thousands of lines is read by whoever happens to be looking.
    #
    # It was not read. Seven diagnostics rode the shipped Trading link -- in
    # `direct_replay_setup_v1::invoke_replay_child_v1`, from an eight-byte
    # account widening that pushed a 4,088-byte frame to 4,096 -- and were
    # found by a lane reading this build output while answering an unrelated
    # question. `run-program-test.sh` has refused on the ACCELERATOR links
    # since 2026-08-27 and says in its own header why the role links matter
    # too; this closes that half.
    #
    # `tools/sbf-frame-sizes.py` measures the frames themselves, which is the
    # number to reach for once this refuses: the count below is a detector at
    # the wall, not a distance to it.
    if [ -f "$WORK/build-diagnostics.txt" ]; then
        noisy="$(awk -F= '$2 != 0 {printf "%s(%s) ", $1, $2}' "$WORK/build-diagnostics.txt")"
        if [ -n "$noisy" ]; then
            echo "gauntlet: SBF stack-frame-overwrite diagnostics: $noisy" >&2
            grep -h "$DIAGNOSTIC_PATTERN" "$LOGS"/build-*.log | sort -u >&2
            # Drop the stage stamp, or the next run reads "stage elf: up to
            # date" and never rebuilds the link it just refused.
            rm -f "$STAMPS/elf"
            die "the toolchain says these calls may cause undefined behavior during execution. Refusing to run a campaign on them; measure the frames with tools/sbf-frame-sizes.py."
        fi
    fi
fi

# ------------------------------------------------------------- 3. host tools
CENSUS_DIGEST="$(cat "$GAUNTLET/census/Cargo.toml" "$GAUNTLET"/census/src/*.rs | sha256_stdin)"
if stage_needed tool "$CENSUS_DIGEST-$SOURCE_DIGEST"; then
    say "stage tool"
    ( cd "$GAUNTLET/census" && CARGO_TARGET_DIR="$CENSUS_TARGET" \
        run_build cargo build --release ) > "$LOGS/build-census.log" 2>&1 \
        || { tail -n 40 "$LOGS/build-census.log" >&2; die "census tool build failed"; }
    # The census's own adversarial tests. They are the thing standing between
    # this suite and being a mirror one level up: each one fails against a
    # deliberately weakened fold.
    ( cd "$GAUNTLET/census" && CARGO_TARGET_DIR="$CENSUS_TARGET" \
        run_build cargo test --release ) > "$LOGS/test-census.log" 2>&1 \
        || { tail -n 40 "$LOGS/test-census.log" >&2; die "census tool tests failed"; }
    if [ "$MODE" = "full" ]; then
        ( cd "$SOURCE/tools/local-validator/bootstrap/successor" \
            && CARGO_TARGET_DIR="$HOST_TARGET" run_build cargo build --release ) \
            > "$LOGS/build-bootstrap.log" 2>&1 \
            || { tail -n 40 "$LOGS/build-bootstrap.log" >&2; die "bootstrap build failed"; }
    fi
    stage_done tool "$CENSUS_DIGEST-$SOURCE_DIGEST"
else
    echo "stage tool: up to date"
fi
[ -x "$CENSUS_BIN" ] || die "census binary missing: $CENSUS_BIN"

# --------------------------------------------------------------- 4. inventory
if stage_needed inventory "$CENSUS_DIGEST-$SOURCE_DIGEST"; then
    say "stage inventory"
    # --check-unique runs the refusal-code gate BEFORE the inventory is
    # written: no two programs may claim one custom error code, and no code may
    # fall outside the band its package owns (decision 0007). It sweeps wider
    # than the route inventory does -- test-program crates included -- because
    # that is where the collisions the census had been annotating around lived.
    "$CENSUS_BIN" inventory \
        --root "$SOURCE" \
        --out "$INVENTORY" \
        --revision "$SOURCE_REVISION" \
        --check-unique
    stage_done inventory "$CENSUS_DIGEST-$SOURCE_DIGEST"
else
    echo "stage inventory: up to date"
fi

# ---------------------------------------------------------------- 5. campaign
if [ "$MODE" = "full" ]; then
    ELF_DIGESTS="$(for entry in $ROLES; do
        role="${entry%%:*}"
        printf '%s=%s\n' "$role" "$(sha256 "$ELF_DIR/$role.so")"
    done)"
    # ------------------------------------------------------ campaign keypairs
    # Tier 1 pins its signing keys to a fixed seed, and that is the whole reason
    # the CU budgets can have teeth.
    #
    # Unseeded, every campaign draws fresh keys, which changes how many
    # iterations `find_program_address` needs to find an off-curve bump, and
    # each iteration is one `sol_create_program_address` syscall at 1,500 CU.
    # Measured band: 58,494 CU on DCLTGMF1, and 79,500 on DCLTPCB1 INSIDE ONE
    # CAMPAIGN. A tolerance wide enough to absorb that cannot also catch a
    # regression smaller than it. See CU_BUDGETS.md.
    #
    # The seed is HASHED FROM ITS PREIMAGE, the same way the Resolution semantic
    # release below is, so what is pinned is a sentence and not a magic number
    # nobody can check. Changing the preimage moves every campaign key and every
    # CU number with it -- which is a re-pin of CU_BUDGETS.json, so the seed is
    # in the campaign stamp and a change to it re-runs the campaign rather than
    # being skipped as up to date.
    #
    # `--keypair-seed` is refused by the bootstrap for any endpoint that is not
    # loopback. That refusal is the affordance's safety gate and it is not this
    # script's to relax; see
    # tools/local-validator/bootstrap/successor/src/seed.rs.
    KEYPAIR_SEED_PREIMAGE='dclutch/gauntlet/tier1/keypair-seed/v1'
    KEYPAIR_SEED="$(printf '%s' "$KEYPAIR_SEED_PREIMAGE" | sha256_stdin)"

    # Deliberately NOT keyed on the bindings file: authoring a binding must
    # never cost a 13-minute campaign re-run. Bindings are consumed by the
    # census stage, which is cheap and always re-runs.
    SPEC_INPUT_DIGEST="$(printf '%s\n%s\n%s\n%s\n' \
        "$SOURCE_REVISION" "$ELF_DIGESTS" "$RECORD_PUBLICATION" \
        "$KEYPAIR_SEED" | sha256_stdin)"

    if stage_needed campaign "$SPEC_INPUT_DIGEST"; then
        say "stage campaign (tier 1)"
        command -v solana-test-validator >/dev/null 2>&1 \
            || die "solana-test-validator not found"
        [ -x "$BOOTSTRAP_BIN" ] || die "bootstrap binary missing: $BOOTSTRAP_BIN"

        if [ "$RPC_PORT" = "auto" ]; then
            RPC_PORT="$(allocate_rpc_port)" || die "--rpc-port auto: no free 42-port block"
            echo "allocated rpc base: $RPC_PORT"
        fi

        # The launcher refuses to start while anything else listens on its base,
        # and so does the bootstrap. Say so here before a 60-second timeout
        # does. Deliberately NOT in SPEC_INPUT_DIGEST above: the origin moves no
        # address, no digest and no compute-unit number, so changing it must not
        # cost a 13-minute campaign re-run.
        if python3 -c 'import socket,sys
s=socket.socket()
s.settimeout(0.5)
sys.exit(0 if s.connect_ex(("127.0.0.1",int(sys.argv[1])))==0 else 1)' "$RPC_PORT"; then
            die "127.0.0.1:$RPC_PORT is occupied. Pass --rpc-port auto to take a free base instead."
        fi

        RUN="$RUNS/$(date -u '+%Y%m%dT%H%M%SZ')-${SOURCE_REVISION:0:12}"
        mkdir -p "$RUN"
        # The gauntlet's launcher shim prefers the committed launcher verbatim
        # and only takes its recorded-override path when the committed Pyth
        # fixture pin list does not verify. See tier1/launcher.sh.
        LAUNCHER="$GAUNTLET/tier1/launcher.sh"
        chmod +x "$LAUNCHER" "$SOURCE/tools/local-validator/dclutch-successor-validator"
        export GAUNTLET_SOURCE_ROOT="$SOURCE"
        export GAUNTLET_ALLOW_STALE_FIXTURE_PINS="$ALLOW_STALE_PINS"

        SOLANA_VERSION="$(solana --version 2>/dev/null | head -n 1 || echo unknown)"
        BUILD_SBF_RAW="$(cargo-build-sbf --version)"
        BUILD_SBF_VERSION="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '1p')"
        PLATFORM_TOOLS="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '2p')"
        SBF_RUSTC="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '3p')"

        # Candidate-local program addresses: derived offline from a fixed
        # domain and the role name, so they are stable across runs and across
        # an artifact changing under a role. No private key exists for any of
        # them, and none names a deployed program anywhere.
        program_id_for() {
            python3 - "$1" <<'PY'
import hashlib, sys
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
raw = hashlib.sha256(
    b"dclutch/gauntlet/program-id/v1\nrole=" + sys.argv[1].encode()
).digest()
value = int.from_bytes(raw, "big")
out = ""
while value:
    value, remainder = divmod(value, 58)
    out = ALPHABET[remainder] + out
for byte in raw:
    if byte:
        break
    out = "1" + out
print(out)
PY
        }

        # ------------------------------------------------ deployment slots
        #
        # DEVNET_DEMO_DEPLOY.md section 7 blocker A: plan.rs used to build every
        # ArtifactReleaseV1 with `deployment_slot` literal 0, which is correct
        # for a genesis install and wrong for every real deploy. It is
        # load-bearing on chain -- artifact.rs returns DeploymentSlotMismatch
        # when the observed slot differs -- and the value cannot be
        # pre-committed, because the measured local deploy landed at slot 167
        # and its redeploy at 531.
        #
        # 993a9ec fixed the tool. NOTHING drove it: no caller in the repository
        # supplied a nonzero slot, so the whole path stayed at 0 == 0 and the
        # rule it enforces was never exercised.
        #
        # It is driven HERE, and only in `transaction` mode, which is the
        # devnet rehearsal (deploy -> revoke -> observe -> mint -> publish).
        # `genesis` mode is the local install where slot 0 is the honest
        # answer, so its plan, its addresses and every one of its CU budget
        # rows are byte-identical to before this existed.
        #
        # Distinct primes, all small: the Loader's own rule is that a program
        # is not executable until AFTER the slot it was deployed in, so the
        # campaign must wait the chain past the highest of them -- at 16 ticks
        # a slot that is a few seconds, and it is the first time that wait has
        # ever had anything to wait for. Distinct rather than uniform so a
        # role reading another role's slot cannot pass by coincidence.
        genesis_deployment_slot_for() {
            case "$1" in
                registry) printf 11 ;; core) printf 13 ;; claims) printf 17 ;;
                trading)  printf 19 ;; resolution) printf 23 ;; custody) printf 29 ;;
                rent)     printf 31 ;;
                *) die "no genesis deployment slot for role $1" ;;
            esac
        }

        mkdir -p "$RUN/attestation"
        for entry in $ROLES; do
            role="${entry%%:*}"; rest="${entry#*:}"
            package="${rest%%:*}"
            elf="$ELF_DIR/$role.so"
            elf_hash="$(sha256 "$elf")"
            program_id="$(program_id_for "$role")"
            log="$LOGS/build-$role.log"
            frame_diagnostics="$(grep -c "$DIAGNOSTIC_PATTERN" "$log" || true)"
            # `verifier` is the SBF *verifier*: the build produced a
            # well-formed, loadable ELF. Backend frame diagnostics are a
            # separate, honestly separate, field — collapsing them would either
            # hide a real defect or block every run on an unrelated lint.
            jq -n \
                --arg elf_path "$elf" \
                --arg elf_sha256 "$elf_hash" \
                --arg program_id "$program_id" \
                --arg commit "$SOURCE_REVISION" \
                --arg archive_sha256 "$SOURCE_DIGEST" \
                --arg cargo_build_sbf_version "$BUILD_SBF_VERSION" \
                --arg platform_tools_version "$PLATFORM_TOOLS" \
                --arg rustc_version "$SBF_RUSTC" \
                --arg solana_version "$SOLANA_VERSION" \
                --arg build_command "cargo build-sbf --manifest-path programs/$package/Cargo.toml" \
                --arg build_log_sha256 "$(sha256 "$log")" \
                --argjson frame_diagnostics "${frame_diagnostics:-0}" \
                '{
                    schema: "dclutch-gauntlet-artifact-attestation-v1",
                    elf_path: $elf_path,
                    elf_sha256: $elf_sha256,
                    program_id: $program_id,
                    commit: $commit,
                    archive_sha256: $archive_sha256,
                    cargo_build_sbf_version: $cargo_build_sbf_version,
                    platform_tools_version: $platform_tools_version,
                    rustc_version: $rustc_version,
                    solana_version: $solana_version,
                    build_command: $build_command,
                    build_log_sha256: $build_log_sha256,
                    verifier: { status: "clean", diagnostic_count: 0 },
                    sbf_backend_frame_diagnostics: $frame_diagnostics,
                    assumptions: [
                        "program_id is a gauntlet-local address derived offline from a fixed domain and the role name; no private key exists for it",
                        "verifier.status records that cargo build-sbf produced a loadable ELF, not an absence of backend frame diagnostics",
                        "archive_sha256 is SHA-256 of the git ls-tree -r --full-tree listing at the exact source revision"
                    ]
                }' > "$RUN/attestation/$role.json"
        done

        # NO ROLE'S semantic release identity is a gauntlet-local value. Every
        # one of them is a PROTOCOL FACT and the bootstrap refuses any other:
        # `plan.rs` derives each id from that role's own shipped ELF digest and
        # names both when they disagree, because a cohort founded under one
        # release-set identity and sealed under another is what stranded
        # cohort 12. This script used to invent an id from the commit for five
        # of the seven roles and hash a RETIRED v4 preimage for Resolution, and
        # was refused by name the first time the unparked tier submitted.
        #
        # The gauntlet HASHES THE PREIMAGE rather than copying a digest
        # constant, so the check is against the semantic statement and not
        # against the code under test:
        #
        #   registry|core|claims|custody|rent   ARTIFACT_SEMANTIC_RELEASE_DOMAIN_V2
        #                                       || role label || NUL || ELF SHA-256 (hex ascii)
        #   trading                             DIRECT_SEMANTIC_RELEASE_PREIMAGE_V1
        #   resolution                          RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V7
        #
        # The two whole-string preimages are contracts rather than artifacts:
        # Trading's and Resolution's semantics are owned by a codec, which is
        # why `checked_semantic_release_preimage_v1` returns them before it
        # looks at any digest.
        ARTIFACT_SEMANTIC_RELEASE_DOMAIN_V2='dclutch/checked-semantic-release/artifact/v2'
        DIRECT_SEMANTIC_RELEASE_PREIMAGE='dclutch/release/direct-compiled-controller-v1'
        RESOLUTION_RELEASE_PREIMAGE='dclutch/release/source-resolution-controller-direct-activation-receipt-permissionless-close-v7'
        semantic_release_for() {
            case "$1" in
                trading)    printf '%s' "$DIRECT_SEMANTIC_RELEASE_PREIMAGE" | sha256_stdin ;;
                resolution) printf '%s' "$RESOLUTION_RELEASE_PREIMAGE" | sha256_stdin ;;
                *)
                    # `rent` is spelled `rent-credit` in the protocol's own role
                    # labels; the gauntlet's short role name is not the label.
                    label="$1"
                    [ "$label" = "rent" ] && label="rent-credit"
                    # The domain constant ends in a newline and the label is
                    # NUL-terminated, so the preimage is assembled byte-exactly
                    # rather than through printf's own separators.
                    printf '%s\n%s\000%s' \
                        "$ARTIFACT_SEMANTIC_RELEASE_DOMAIN_V2" "$label" "$(sha256 "$ELF_DIR/$1.so")" \
                        | sha256_stdin
                    ;;
            esac
        }

        # `demo-market` is retired and this script no longer calls it. The
        # spec below omits `market` entirely, and the supervisor compiles the
        # input from the plan it builds -- see the note in the usage text and
        # `SuccessorRunSpec::market`. Asserted here rather than assumed,
        # because a bootstrap that quietly reintroduced a Market default would
        # found a market this script never described:
        if "$BOOTSTRAP_BIN" demo-market > /dev/null 2>&1; then
            die "the retired demo-market planner produced a market; the tier-1 spec omits market on the belief that it cannot"
        fi

        # Assemble the run spec. Output paths must not exist yet: the runner
        # refuses to overwrite prior evidence, which is exactly right.
        SPEC="$RUN/spec.json"
        {
            printf '{\n'
            printf '  "schema": "dclutch-local-successor-run-spec-v2",\n'
            printf '  "rpc_url": "http://127.0.0.1:%s/",\n' "$RPC_PORT"
            printf '  "launcher": "%s",\n' "$LAUNCHER"
            printf '  "ledger": "%s/ledger",\n' "$RUN"
            printf '  "account_dir": "%s/accounts",\n' "$RUN"
            printf '  "plan": "%s/plan.json",\n' "$RUN"
            printf '  "output": "%s/evidence.json",\n' "$RUN"
            for entry in $ROLES; do
                role="${entry%%:*}"
                key="$role"
                [ "$key" = "rent" ] && key="rent_credit"
                printf '  "%s": {\n' "$key"
                printf '    "program_id": "%s",\n' "$(program_id_for "$role")"
                printf '    "elf_path": "%s",\n' "$ELF_DIR/$role.so"
                printf '    "elf_sha256": "%s",\n' "$(sha256 "$ELF_DIR/$role.so")"
                printf '    "semantic_release_id": "%s",\n' "$(semantic_release_for "$role")"
                if [ "$RECORD_PUBLICATION" = "transaction" ]; then
                    printf '    "genesis_deployment_slot": %s,\n' "$(genesis_deployment_slot_for "$role")"
                fi
                printf '    "attestation": "%s"\n' "$RUN/attestation/$role.json"
                printf '  },\n'
            done
            printf '  "record_publication": "%s"\n' "$RECORD_PUBLICATION"
            printf '}\n'
        } > "$SPEC.raw"
        jq . "$SPEC.raw" > "$SPEC" || die "assembled run spec is not valid JSON"

        say "campaign: submitting real transactions to a fresh localhost ledger"
        echo "rpc origin:         http://127.0.0.1:$RPC_PORT/"
        echo "record publication: $RECORD_PUBLICATION"
        echo "keypair seed:       $KEYPAIR_SEED"
        echo "             from:  $KEYPAIR_SEED_PREIMAGE"
        echo "run directory: $RUN"
        if ! "$BOOTSTRAP_BIN" run --spec "$SPEC" --keypair-seed "$KEYPAIR_SEED" \
            > "$RUN/campaign.stdout" 2> "$RUN/campaign.stderr"; then
            tail -n 60 "$RUN/campaign.stderr" >&2
            echo "gauntlet: campaign FAILED; evidence and logs are under $RUN" >&2
            printf '%s\n' "$RUN" > "$WORK/last-run"
            exit 1
        fi
        printf '%s\n' "$RUN" > "$WORK/last-run"
        stage_done campaign "$SPEC_INPUT_DIGEST"

        if [ "$KEEP_RUNS" != "true" ]; then
            # Keep the last three; a campaign run directory holds a whole ledger.
            # BSD head has no negative count; drop all but the newest three.
            ls -1d "$RUNS"/*/ 2>/dev/null | sort | sed '$d' | sed '$d' | sed '$d' \
                | while read -r stale; do rm -rf "$stale"; done || true
        fi
    else
        echo "stage campaign: up to date ($(cat "$WORK/last-run" 2>/dev/null || echo '?'))"
    fi
fi

# ------------------------------------------------------------------ 6. census
say "stage census"
CENSUS_PROBLEMS=0
if [ "$MODE" = "full" ]; then
    RUN="$(cat "$WORK/last-run")"
    EVIDENCE="$RUN/evidence.json"
    [ -f "$EVIDENCE" ] || die "campaign evidence missing: $EVIDENCE"

    # The label -> program-address map comes from the bootstrap's own plan, not
    # from the gauntlet's beliefs: the census cross-checks the campaign's claims
    # against the chain, so its program identities must be chain-derived too.
    jq '{
        registry: .registry.program_id,
        core: .core.program_id,
        claims: .claims.program_id,
        trading: .trading.program_id,
        resolution: .resolution.program_id,
        custody: .custody.program_id,
        rent: .rent_credit.program_id
    }' "$RUN/plan.json" > "$OUT/programs.json"

    [ -f "$BINDINGS" ] || die "tier-1 bindings missing: $BINDINGS"
    ledger_lock "$LEDGER"
    if ! "$CENSUS_BIN" observe \
        --inventory "$INVENTORY" \
        --ledger "$LEDGER" \
        --bindings "$BINDINGS" \
        --programs "$OUT/programs.json" \
        --evidence "$EVIDENCE"; then
        CENSUS_PROBLEMS=1
    fi
    ledger_unlock

    if [ -f "$WITNESSES" ]; then
        say "witnesses"
        "$GAUNTLET/tier1/check-witnesses.sh" "$WITNESSES" "$EVIDENCE" "$RUN/plan.json" \
            || CENSUS_PROBLEMS=1
    fi
fi

[ -f "$BLOCKED" ] || die "blocked-route register missing: $BLOCKED"
ledger_lock "$LEDGER"
"$CENSUS_BIN" report \
    --inventory "$INVENTORY" \
    --ledger "$LEDGER" \
    --blocked "$BLOCKED" \
    --out "$REPORT"
ledger_unlock

say "done"
echo "inventory: $INVENTORY"
echo "ledger:    $LEDGER"
echo "report:    $REPORT"
if [ "$CENSUS_PROBLEMS" != "0" ]; then
    echo "gauntlet: the census or the witnesses reported problems (above); NOT green" >&2
    exit 1
fi
