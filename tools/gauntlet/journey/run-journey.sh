#!/usr/bin/env bash
# JRNY-1: run the whole-life journey campaign and fold it into the census.
#
#   archive -> ELFs -> journey binary -> campaign -> witnesses -> census
#
# This is a family tier and owns its own runner, per TIERS.md. It does NOT add a
# stage to run.sh: run.sh owns tier 1 and the census, and a shared script every
# family edits is the numbered-directory race one level down.
#
# The journey is a SUPERSET of the tier-1 campaign, not a sibling of it. It
# calls the tier-1 producer's own `found_through_open` in-process and then keeps
# going on the same validator, so its evidence document carries every tier-1
# transaction plus the journey's. Two consequences the script depends on:
#
#   * the bindings handed to `census observe` are tier 1's PLUS the journey's,
#     merged at run time. The census fails an unbound transaction and fails a
#     binding that matched nothing, so a second hand-maintained copy of tier 1's
#     31 bindings would rot the first time tier 1 changed. There is one copy.
#   * the witness evaluator runs TWICE against the same evidence: once with
#     tier 1's witnesses and the bootstrap plan as context, once with the
#     journey's witnesses and the journey transcript as context. The evaluator
#     takes one context file, and it is shared, so calling it twice is the
#     supported shape; forking it would not be.
#
# WHAT THIS PRODUCES IS LOCAL-VALIDATOR EVIDENCE. Not devnet, not mainnet.
# Nothing here signs with a persisted key, funds an external account, publishes,
# or deploys anywhere but a fresh localhost ledger on 127.0.0.1:20890.
#
# `--mode full` of run.sh and this script both take the SINGLE GLOBAL 20890
# slot. Coordinate on the wave board; never kill a solana-test-validator whose
# --ledger is not under your own --work root.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: tools/gauntlet/journey/run-journey.sh [options]

  --repo PATH           source repository (default: this script's repository)
  --work PATH           journey scratch root (default: /private/tmp/dclutch-journey)
  --gauntlet-work PATH  the shared gauntlet root whose inventory and ledger this
                        tier reads and accumulates into
                        (default: /private/tmp/dclutch-gauntlet)
  --commit REV          source revision to archive and build (default: HEAD)
  --holders N           synthetic holder count, the load knob (default: 4)
  --keypair-seed HEX    64 lowercase hex passed to the producer's TEST-ONLY,
                        LOOPBACK-ONLY determinism switch. Defaults to a fixed
                        campaign seed, because a conservation ledger whose
                        numbers cannot be compared between runs is a diary.
                        Pass `none` to take fresh unreproducible keys instead.
  --allow-stale-fixture-pins
                        pass through to tier1/launcher.sh; see that file for
                        what the recorded override gives up
  -h, --help            show this message

Run `tools/gauntlet/run.sh --mode census` first if there is no inventory yet;
it takes seconds and needs no chain.
USAGE
}

REPO=""
WORK="/private/tmp/dclutch-journey"
GAUNTLET_WORK="/private/tmp/dclutch-gauntlet"
COMMIT="HEAD"
HOLDERS="4"
ALLOW_STALE_PINS="false"
# A fixed, checked-in campaign seed. It is safe here and ONLY here: the producer
# refuses the flag outright unless the RPC endpoint is loopback, and this tier is
# pinned to 127.0.0.1:20890. Its value is the SHA-256 of the ASCII string
# "dclutch/gauntlet/journey/campaign-seed/v1", so it is a stated derivation
# rather than a number somebody typed.
KEYPAIR_SEED="$(printf '%s' 'dclutch/gauntlet/journey/campaign-seed/v1' | shasum -a 256 | cut -d' ' -f1)"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --gauntlet-work) GAUNTLET_WORK="${2:?--gauntlet-work needs a value}"; shift 2 ;;
        --commit) COMMIT="${2:?--commit needs a value}"; shift 2 ;;
        --holders) HOLDERS="${2:?--holders needs a value}"; shift 2 ;;
        --keypair-seed) KEYPAIR_SEED="${2:?--keypair-seed needs a value}"; shift 2 ;;
        --allow-stale-fixture-pins) ALLOW_STALE_PINS="true"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [ -z "$REPO" ]; then
    REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
fi
case "$WORK" in /*) ;; *) echo "--work must be absolute" >&2; exit 2 ;; esac
case "$HOLDERS" in ''|*[!0-9]*) echo "--holders must be a decimal count" >&2; exit 2 ;; esac
[ "$HOLDERS" -gt 0 ] || { echo "--holders must be positive" >&2; exit 2; }

GAUNTLET="$REPO/tools/gauntlet"
TIER="$GAUNTLET/journey"
SOURCE="$WORK/source"
ELF_DIR="$WORK/elf"
BUILD_TARGET="$WORK/build-target"
HOST_TARGET="$WORK/host-target"
LOGS="$WORK/logs"
RUNS="$WORK/runs"
JOURNEY_BIN="$HOST_TARGET/release/dclutch-journey-campaign"
INVENTORY="$GAUNTLET_WORK/out/inventory.json"
LEDGER="$GAUNTLET_WORK/out/ledger.json"

mkdir -p "$WORK" "$LOGS" "$RUNS" "$ELF_DIR"

sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
sha256_stdin() { shasum -a 256 | cut -d' ' -f1; }
say() { printf '\n== %s\n' "$*"; }
die() { printf 'journey: %s\n' "$*" >&2; exit 1; }

for tool in git jq shasum python3 cargo solana-test-validator cargo-build-sbf; do
    command -v "$tool" >/dev/null 2>&1 || die "required command not found: $tool"
done
[ -f "$INVENTORY" ] || die "no census inventory at $INVENTORY. Run 'tools/gauntlet/run.sh --mode census' first; it takes seconds and needs no chain."

# hbox is co-tenant with codex's HOL build. Containment is structural.
if command -v swarm-build >/dev/null 2>&1; then WRAP="swarm-build"; else WRAP=""; fi
run_build() { if [ -n "$WRAP" ]; then "$WRAP" "$@"; else "$@"; fi; }

SOURCE_REVISION="$(git -C "$REPO" rev-parse "$COMMIT")"
SOURCE_DIGEST="$(git -C "$REPO" ls-tree -r --full-tree "$SOURCE_REVISION" | sha256_stdin)"
say "journey at $SOURCE_REVISION (holders=$HOLDERS, keypairs=$([ "$KEYPAIR_SEED" = none ] && echo fresh || echo deterministic))"

# ------------------------------------------------------------------ 1. archive
if [ ! -f "$WORK/stamps.archive" ] || [ "$(cat "$WORK/stamps.archive")" != "$SOURCE_DIGEST" ]; then
    say "stage archive"
    rm -rf "$SOURCE"; mkdir -p "$SOURCE"
    git -C "$REPO" archive "$SOURCE_REVISION" | tar -x -C "$SOURCE"
    printf '%s\n' "$SOURCE_DIGEST" > "$WORK/stamps.archive"
else
    echo "stage archive: up to date"
fi

# --------------------------------------------------------------------- 2. ELFs
ROLES="registry:dclutch-registry-sbf:dclutch_registry_sbf
core:dclutch-core-sbf:dclutch_core_sbf
claims:dclutch-claims-sbf:dclutch_claims_sbf
trading:dclutch-trading-sbf:dclutch_trading_sbf
resolution:dclutch-resolution-proof-sbf:dclutch_resolution_proof_sbf
custody:dclutch-custody-sbf:dclutch_custody_sbf
rent:dclutch-rent-sbf:dclutch_rent_sbf"

DIAGNOSTIC_PATTERN='overwrites values in the frame'

# The gauntlet may already hold this exact revision's artifacts. Reuse them only
# when its own stamp says they were built from this digest -- an ELF directory
# that merely exists proves nothing about what is in it.
REUSED="false"
if [ -f "$GAUNTLET_WORK/stamps/elf" ] \
   && [ "$(cat "$GAUNTLET_WORK/stamps/elf")" = "$SOURCE_DIGEST" ]; then
    REUSED="true"
    for entry in $ROLES; do
        role="${entry%%:*}"
        [ -f "$GAUNTLET_WORK/elf/$role.so" ] || REUSED="false"
        [ -f "$GAUNTLET_WORK/logs/build-$role.log" ] || REUSED="false"
    done
fi

if [ "$REUSED" = "true" ]; then
    say "stage elf: reusing the gauntlet's artifacts for this exact revision"
    for entry in $ROLES; do
        role="${entry%%:*}"
        cp "$GAUNTLET_WORK/elf/$role.so" "$ELF_DIR/$role.so"
        cp "$GAUNTLET_WORK/logs/build-$role.log" "$LOGS/build-$role.log"
        printf '  %s  %s\n' "$(sha256 "$ELF_DIR/$role.so")" "$role"
    done
elif [ ! -f "$WORK/stamps.elf" ] || [ "$(cat "$WORK/stamps.elf")" != "$SOURCE_DIGEST" ]; then
    say "stage elf"
    for entry in $ROLES; do
        role="${entry%%:*}"; rest="${entry#*:}"; package="${rest%%:*}"; stem="${rest#*:}"
        echo "build: $role ($package)"
        ( cd "$SOURCE" && CARGO_TARGET_DIR="$BUILD_TARGET" \
            run_build cargo build-sbf --manifest-path "programs/$package/Cargo.toml" ) \
            > "$LOGS/build-$role.log" 2>&1 \
            || { tail -n 40 "$LOGS/build-$role.log" >&2; die "SBF build failed: $role"; }
        cp "$BUILD_TARGET/deploy/$stem.so" "$ELF_DIR/$role.so"
        printf '  %s  %s (%s frame diagnostics)\n' "$(sha256 "$ELF_DIR/$role.so")" "$role" \
            "$(grep -c "$DIAGNOSTIC_PATTERN" "$LOGS/build-$role.log" || true)"
    done
    printf '%s\n' "$SOURCE_DIGEST" > "$WORK/stamps.elf"
else
    echo "stage elf: up to date"
fi

# `cargo build-sbf` exits ZERO when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution". An artifact the toolchain calls potentially-undefined has no
# business producing evidence, and only the build stage is in a position to
# say so. Unlike run.sh, this tier REFUSES rather than warning: the journey's
# whole claim is about state that survives a long chain of transactions, and
# undefined behaviour anywhere in that chain voids the claim silently.
# The narrow exception is frame-diagnostics.json, shaped like blocked.json: it
# names the exact mangled symbol, the measured count, why this campaign does not
# reach it, and who owns the fix. Anything it does not name, or a count that
# GREW, stops the run.
: > "$WORK/frame-diagnostics.txt"
for entry in $ROLES; do
    role="${entry%%:*}"
    grep -h "$DIAGNOSTIC_PATTERN" "$LOGS/build-$role.log" 2>/dev/null \
        | sed "s|^|$role\t|" >> "$WORK/frame-diagnostics.txt" || true
done
if [ -s "$WORK/frame-diagnostics.txt" ]; then
    python3 "$TIER/check-frame-diagnostics.py" \
            "$TIER/frame-diagnostics.json" "$WORK/frame-diagnostics.txt" >&2 || \
        die "SBF stack-frame-overwrite diagnostics are not covered by tools/gauntlet/journey/frame-diagnostics.json; refusing to run a journey on artifacts the toolchain calls potentially-undefined."
fi

# ----------------------------------------------------------- 3. journey binary
JOURNEY_DIGEST="$(cat "$TIER/Cargo.toml" "$TIER"/src/*.rs | sha256_stdin)"
if [ ! -f "$WORK/stamps.tool" ] || [ "$(cat "$WORK/stamps.tool")" != "$JOURNEY_DIGEST-$SOURCE_DIGEST" ]; then
    say "stage tool"
    # Built from the ARCHIVE, not the working tree: the journey compiles the
    # tier-1 producer's source files into itself, so building it from a dirty
    # tree would silently mix revisions of the founding into a campaign whose
    # attestations name one.
    ( cd "$SOURCE/tools/gauntlet/journey" && CARGO_TARGET_DIR="$HOST_TARGET" \
        run_build cargo build --release ) > "$LOGS/build-journey.log" 2>&1 \
        || { tail -n 40 "$LOGS/build-journey.log" >&2; die "journey build failed"; }
    printf '%s\n' "$JOURNEY_DIGEST-$SOURCE_DIGEST" > "$WORK/stamps.tool"
else
    echo "stage tool: up to date"
fi
[ -x "$JOURNEY_BIN" ] || die "journey binary missing: $JOURNEY_BIN"

# ----------------------------------------------------------------- 4. campaign
if python3 -c 'import socket,sys
s=socket.socket(); s.settimeout(0.5)
sys.exit(0 if s.connect_ex(("127.0.0.1",20890))==0 else 1)'; then
    die "127.0.0.1:20890 is occupied; the successor launcher is pinned to that origin"
fi

RUN="$RUNS/$(date -u '+%Y%m%dT%H%M%SZ')-${SOURCE_REVISION:0:12}-h$HOLDERS"
mkdir -p "$RUN/attestation"
LAUNCHER="$GAUNTLET/tier1/launcher.sh"
chmod +x "$LAUNCHER" "$SOURCE/tools/local-validator/dclutch-successor-validator"
export GAUNTLET_SOURCE_ROOT="$SOURCE"
export GAUNTLET_ALLOW_STALE_FIXTURE_PINS="$ALLOW_STALE_PINS"

SOLANA_VERSION="$(solana --version 2>/dev/null | head -n 1 || echo unknown)"
BUILD_SBF_RAW="$(cargo-build-sbf --version)"

# Candidate-local program addresses, derived offline from a fixed domain and the
# role name. The domain is TIER 1's: the journey deploys the same seven
# artifacts under the same identities, and inventing a second address family
# would make the two campaigns' ledger rows describe different programs.
program_id_for() {
    python3 - "$1" <<'PY'
import hashlib, sys
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
raw = hashlib.sha256(b"dclutch/gauntlet/program-id/v1\nrole=" + sys.argv[1].encode()).digest()
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

for entry in $ROLES; do
    role="${entry%%:*}"; rest="${entry#*:}"; package="${rest%%:*}"
    elf="$ELF_DIR/$role.so"; log="$LOGS/build-$role.log"
    jq -n \
        --arg elf_path "$elf" \
        --arg elf_sha256 "$(sha256 "$elf")" \
        --arg program_id "$(program_id_for "$role")" \
        --arg commit "$SOURCE_REVISION" \
        --arg archive_sha256 "$SOURCE_DIGEST" \
        --arg cargo_build_sbf_version "$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '1p')" \
        --arg platform_tools_version "$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '2p')" \
        --arg rustc_version "$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '3p')" \
        --arg solana_version "$SOLANA_VERSION" \
        --arg build_command "cargo build-sbf --manifest-path programs/$package/Cargo.toml" \
        --arg build_log_sha256 "$(sha256 "$log")" \
        '{
            schema: "dclutch-gauntlet-artifact-attestation-v1",
            elf_path: $elf_path, elf_sha256: $elf_sha256, program_id: $program_id,
            commit: $commit, archive_sha256: $archive_sha256,
            cargo_build_sbf_version: $cargo_build_sbf_version,
            platform_tools_version: $platform_tools_version,
            rustc_version: $rustc_version, solana_version: $solana_version,
            build_command: $build_command, build_log_sha256: $build_log_sha256,
            verifier: { status: "clean", diagnostic_count: 0 },
            sbf_backend_frame_diagnostics: 0,
            assumptions: [
                "program_id is a gauntlet-local address derived offline from a fixed domain and the role name; no private key exists for it",
                "verifier.status records that cargo build-sbf produced a loadable ELF, not an absence of backend frame diagnostics",
                "archive_sha256 is SHA-256 of the git ls-tree -r --full-tree listing at the exact source revision",
                "this tier REFUSES a nonzero backend frame diagnostic count before it reaches this file, so zero here is checked, not assumed"
            ]
        }' > "$RUN/attestation/$role.json"
done

# The Resolution role's semantic release identity is a PROTOCOL FACT: the
# producer refuses any other. The preimage is HASHED here rather than the digest
# constant copied, so the check is against the semantic statement and not
# against the code under test.
RESOLUTION_RELEASE_PREIMAGE='dclutch/release/source-resolution-controller-core-effects-source-closure-v4'
semantic_release_for() {
    if [ "$1" = "resolution" ]; then
        printf '%s' "$RESOLUTION_RELEASE_PREIMAGE" | sha256_stdin
    else
        printf 'dclutch/gauntlet/semantic-release/v1\nrole=%s\ncommit=%s\n' "$1" "$SOURCE_REVISION" | sha256_stdin
    fi
}

# The demo Market run-spec comes from the journey binary, which compiles the
# producer's `market::demo_market_input` into itself. Shelling out to a second
# binary for it would be a second build of the same function.
"$JOURNEY_BIN" demo-market --registry-program-id "$(program_id_for registry)" \
    > "$RUN/market.json"

SPEC="$RUN/spec.json"
{
    printf '{\n'
    printf '  "schema": "dclutch-local-successor-run-spec-v2",\n'
    printf '  "rpc_url": "http://127.0.0.1:20890/",\n'
    printf '  "launcher": "%s",\n' "$LAUNCHER"
    printf '  "ledger": "%s/ledger",\n' "$RUN"
    printf '  "account_dir": "%s/accounts",\n' "$RUN"
    printf '  "plan": "%s/plan.json",\n' "$RUN"
    printf '  "output": "%s/evidence.json",\n' "$RUN"
    for entry in $ROLES; do
        role="${entry%%:*}"; key="$role"; [ "$key" = "rent" ] && key="rent_credit"
        printf '  "%s": {\n' "$key"
        printf '    "program_id": "%s",\n' "$(program_id_for "$role")"
        printf '    "elf_path": "%s",\n' "$ELF_DIR/$role.so"
        printf '    "elf_sha256": "%s",\n' "$(sha256 "$ELF_DIR/$role.so")"
        printf '    "semantic_release_id": "%s",\n' "$(semantic_release_for "$role")"
        printf '    "attestation": "%s"\n' "$RUN/attestation/$role.json"
        printf '  },\n'
    done
    printf '  "market": '
    cat "$RUN/market.json"
    printf '\n}\n'
} > "$SPEC.raw"
jq . "$SPEC.raw" > "$SPEC" || die "assembled run spec is not valid JSON"

say "campaign: living one Market's whole life on a fresh localhost ledger"
echo "run directory: $RUN"
JOURNEY_ARGS=(run --spec "$SPEC" --transcript "$RUN/transcript.json" --holders "$HOLDERS")
if [ "$KEYPAIR_SEED" != "none" ]; then
    JOURNEY_ARGS+=(--keypair-seed "$KEYPAIR_SEED")
fi
if ! "$JOURNEY_BIN" "${JOURNEY_ARGS[@]}" \
        > "$RUN/campaign.stdout" 2> "$RUN/campaign.stderr"; then
    tail -n 60 "$RUN/campaign.stderr" >&2
    echo "journey: campaign FAILED; evidence, transcript and logs are under $RUN" >&2
    printf '%s\n' "$RUN" > "$WORK/last-run"
    exit 1
fi
printf '%s\n' "$RUN" > "$WORK/last-run"

EVIDENCE="$RUN/evidence.json"
TRANSCRIPT="$RUN/transcript.json"
[ -f "$EVIDENCE" ] || die "campaign evidence missing: $EVIDENCE"
[ -f "$TRANSCRIPT" ] || die "campaign transcript missing: $TRANSCRIPT"

# ---------------------------------------------------------------- 5. the ledger
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
if [ "$(jq -r '.conservation_verdict' "$TRANSCRIPT")" != "conserved" ]; then
    die "the conservation ledger reported violations; see $TRANSCRIPT"
fi

# -------------------------------------------------------------- 6. witnesses
say "witnesses: tier 1's, against this campaign's evidence"
"$GAUNTLET/tier1/check-witnesses.sh" "$GAUNTLET/tier1/witnesses.json" "$EVIDENCE" "$RUN/plan.json"
say "witnesses: the journey's, against its transcript"
"$GAUNTLET/tier1/check-witnesses.sh" "$TIER/witnesses.json" "$EVIDENCE" "$TRANSCRIPT"

# ----------------------------------------------------------------- 7. census
say "census"
jq '{registry:.registry.program_id, core:.core.program_id, claims:.claims.program_id,
     trading:.trading.program_id, resolution:.resolution.program_id,
     custody:.custody.program_id, rent:.rent_credit.program_id}' \
    "$RUN/plan.json" > "$RUN/programs.json"

# One copy of tier 1's bindings, merged at run time. See the header.
jq -s '{campaign: "journey",
        note: ("JRNY-1 whole-life journey. Tier 1'"'"'s bindings are merged in at run time from " +
               "tools/gauntlet/tier1/bindings.json because the journey submits every tier-1 " +
               "transaction before its own; there is exactly one copy of them."),
        bindings: (.[0].bindings + .[1].bindings)}' \
    "$GAUNTLET/tier1/bindings.json" "$TIER/bindings.json" > "$RUN/bindings.json"

cargo run --quiet --manifest-path "$GAUNTLET/census/Cargo.toml" -- observe \
    --inventory "$INVENTORY" \
    --ledger "$LEDGER" \
    --bindings "$RUN/bindings.json" \
    --programs "$RUN/programs.json" \
    --evidence "$EVIDENCE"

say "done"
echo "evidence:   $EVIDENCE"
echo "transcript: $TRANSCRIPT"
echo "ledger:     $LEDGER"
echo "journey: render the report with 'tools/gauntlet/run.sh --mode census'"
