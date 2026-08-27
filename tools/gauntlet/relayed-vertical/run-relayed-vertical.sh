#!/usr/bin/env bash
# DEMO-VERT: the relayed graduation market, end to end, on a local rehearsal.
#
#   archive/worktree -> ELFs -> relayer binary -> campaign binary
#     -> success walk (twin + daemon + consume) -> witnesses
#     -> failure walk (silent daemon + funded walk) -> witnesses
#
# TWO validators per walk, both loopback, both on per-run ports: the successor
# validator on this runner's allocated block, and a mainnet-twin
# solana-test-validator the campaign binary itself allocates under --work.
# Nothing here signs with a persisted key, funds an external account,
# publishes, or observes any public cluster.
#
# WHAT THIS PRODUCES IS LOCAL-VALIDATOR EVIDENCE with a REHEARSAL-TWIN
# provider: the daemon's attestations claim the cluster the adapter release
# pins while reading a loopback twin, and every artifact carries that label.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: tools/gauntlet/relayed-vertical/run-relayed-vertical.sh [options]

  --walk MODE           success | failure | both   (default: both)
  --repo PATH           source repository (default: this script's repository)
  --commit REV          revision to archive and build (default: HEAD)
  --worktree            build from the WORKING TREE instead of an archive.
                        Development mode: the transcript records it, because a
                        campaign whose attestations name no revision is not
                        release evidence.
  --rpc-port PORT|auto  successor validator base (default: auto)
  --work PATH           scratch root (default: /private/tmp/dclutch-relayed-vertical)
  --keypair-seed HEX    the producer's TEST-ONLY loopback-only determinism
                        switch (default: none; the relayer key is fresh per
                        run either way, because it is founding content)
USAGE
}

die() { echo "relayed-vertical: $1" >&2; exit 1; }
say() { printf '\n== %s\n' "$1"; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
sha256_stdin() { shasum -a 256 | cut -d' ' -f1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAUNTLET="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="$(cd "$GAUNTLET/../.." && pwd)"
WALK="both"
COMMIT="HEAD"
WORKTREE=0
RPC_PORT="auto"
WORK="/private/tmp/dclutch-relayed-vertical"
KEYPAIR_SEED="none"

while [ $# -gt 0 ]; do
    case "$1" in
        --walk) WALK="${2:?}"; shift 2 ;;
        --repo) REPO="${2:?}"; shift 2 ;;
        --commit) COMMIT="${2:?}"; shift 2 ;;
        --worktree) WORKTREE=1; shift ;;
        --rpc-port) RPC_PORT="${2:?}"; shift 2 ;;
        --work) WORK="${2:?}"; shift 2 ;;
        --keypair-seed) KEYPAIR_SEED="${2:?}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done
case "$WALK" in success|failure|both) ;; *) die "--walk must be success, failure or both" ;; esac

for tool in git jq shasum python3 cargo solana-test-validator cargo-build-sbf; do
    command -v "$tool" >/dev/null 2>&1 || die "required command not found: $tool"
done

if command -v swarm-build >/dev/null 2>&1; then WRAP="swarm-build"; else WRAP=""; fi
run_build() { if [ -n "$WRAP" ]; then "$WRAP" "$@"; else "$@"; fi; }

mkdir -p "$WORK"/{logs,elf,runs}
LOGS="$WORK/logs"
ELF_DIR="$WORK/elf"

# ------------------------------------------------------------------- 1. source
if [ "$WORKTREE" = 1 ]; then
    SOURCE="$REPO"
    SOURCE_REVISION="worktree-$(git -C "$REPO" rev-parse HEAD)"
    SOURCE_DIGEST="worktree"
    say "building from the WORKING TREE (development mode; not release evidence)"
else
    SOURCE_REVISION="$(git -C "$REPO" rev-parse "$COMMIT")"
    SOURCE_DIGEST="$(git -C "$REPO" ls-tree -r --full-tree "$SOURCE_REVISION" | sha256_stdin)"
    SOURCE="$WORK/source"
    if [ ! -f "$WORK/stamps.archive" ] || [ "$(cat "$WORK/stamps.archive")" != "$SOURCE_DIGEST" ]; then
        say "stage archive ($SOURCE_REVISION)"
        rm -rf "$SOURCE"; mkdir -p "$SOURCE"
        git -C "$REPO" archive "$SOURCE_REVISION" | tar -x -C "$SOURCE"
        printf '%s\n' "$SOURCE_DIGEST" > "$WORK/stamps.archive"
    fi
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
BUILD_TARGET="$WORK/sbf-target"

elf_inputs_digest() {
    if [ "$WORKTREE" = 1 ]; then
        # HEAD's tree plus the working diff: content-sensitive without walking
        # the nested target/ directories a find would drown in.
        { git -C "$REPO" ls-tree -r --full-tree HEAD \
              -- programs crates Cargo.toml Cargo.lock rust-toolchain.toml
          git -C "$REPO" diff HEAD \
              -- programs crates Cargo.toml Cargo.lock rust-toolchain.toml
        } | sha256_stdin
    else
        git -C "$REPO" ls-tree -r --full-tree "$SOURCE_REVISION" \
            -- programs crates Cargo.toml Cargo.lock rust-toolchain.toml | sha256_stdin
    fi
}
ELF_INPUT_DIGEST="$(elf_inputs_digest)"
if [ ! -f "$WORK/stamps.elf" ] || [ "$(cat "$WORK/stamps.elf")" != "$ELF_INPUT_DIGEST" ]; then
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
    printf '%s\n' "$ELF_INPUT_DIGEST" > "$WORK/stamps.elf"
else
    echo "stage elf: up to date"
fi

# Refuse potentially-undefined artifacts, exactly as the journey does.
TOTAL_DIAGNOSTICS=0
for entry in $ROLES; do
    role="${entry%%:*}"
    count="$(grep -c "$DIAGNOSTIC_PATTERN" "$LOGS/build-$role.log" 2>/dev/null || true)"
    TOTAL_DIAGNOSTICS=$((TOTAL_DIAGNOSTICS + ${count:-0}))
done
[ "$TOTAL_DIAGNOSTICS" = 0 ] || die "SBF stack-frame-overwrite diagnostics present ($TOTAL_DIAGNOSTICS); refusing to run the vertical on artifacts the toolchain calls potentially-undefined"

# ------------------------------------------------- 3. relayer + campaign tools
HOST_TARGET="$WORK/host-target"
say "stage tools"
( cd "$SOURCE/tools/relayer" && CARGO_TARGET_DIR="$HOST_TARGET/relayer" \
    run_build cargo build --release ) > "$LOGS/build-relayer.log" 2>&1 \
    || { tail -n 40 "$LOGS/build-relayer.log" >&2; die "relayer build failed"; }
RELAYER_BIN="$HOST_TARGET/relayer/release/dclutch-relayer"
[ -x "$RELAYER_BIN" ] || die "relayer binary missing: $RELAYER_BIN"
( cd "$SOURCE/tools/gauntlet/relayed-vertical" && CARGO_TARGET_DIR="$HOST_TARGET/vertical" \
    run_build cargo build --release ) > "$LOGS/build-vertical.log" 2>&1 \
    || { tail -n 40 "$LOGS/build-vertical.log" >&2; die "campaign build failed"; }
CAMPAIGN_BIN="$HOST_TARGET/vertical/release/dclutch-relayed-vertical-campaign"
[ -x "$CAMPAIGN_BIN" ] || die "campaign binary missing: $CAMPAIGN_BIN"

# --------------------------------------------------------------- 4. port block
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

RESOLUTION_RELEASE_PREIMAGE='dclutch/release/source-resolution-controller-core-effects-source-closure-v4'
semantic_release_for() {
    if [ "$1" = "resolution" ]; then
        printf '%s' "$RESOLUTION_RELEASE_PREIMAGE" | sha256_stdin
    else
        printf 'dclutch/gauntlet/semantic-release/v1\nrole=%s\ncommit=%s\n' "$1" "$SOURCE_REVISION" | sha256_stdin
    fi
}

SOLANA_VERSION="$(solana --version 2>/dev/null | head -n 1 || echo unknown)"
BUILD_SBF_RAW="$(cargo-build-sbf --version)"
LAUNCHER="$GAUNTLET/tier1/launcher.sh"
chmod +x "$LAUNCHER" "$SOURCE/tools/local-validator/dclutch-successor-validator" 2>/dev/null || true
export GAUNTLET_SOURCE_ROOT="$SOURCE"

run_walk() {
    local walk="$1"
    local base="$RPC_PORT"
    if [ "$base" = "auto" ]; then
        base="$(allocate_rpc_port)" || die "no free port block"
    fi
    local run="$WORK/runs/$(date -u '+%Y%m%dT%H%M%SZ')-$walk"
    mkdir -p "$run/attestation"
    say "walk: $walk (successor base $base, run $run)"

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
                    "this runner REFUSES a nonzero backend frame diagnostic count before it reaches this file, so zero here is checked, not assumed"
                ]
            }' > "$run/attestation/$role.json"
    done

    local template="$run/spec-template.json"
    {
        printf '{\n'
        printf '  "schema": "dclutch-local-successor-run-spec-v2",\n'
        printf '  "rpc_url": "http://127.0.0.1:%s/",\n' "$base"
        printf '  "launcher": "%s",\n' "$LAUNCHER"
        printf '  "ledger": "%s/ledger",\n' "$run"
        printf '  "account_dir": "%s/accounts",\n' "$run"
        printf '  "plan": "%s/plan.json",\n' "$run"
        printf '  "output": "%s/evidence.json",\n' "$run"
        for entry in $ROLES; do
            role="${entry%%:*}"; key="$role"; [ "$key" = "rent" ] && key="rent_credit"
            printf '  "%s": {\n' "$key"
            printf '    "program_id": "%s",\n' "$(program_id_for "$role")"
            printf '    "elf_path": "%s",\n' "$ELF_DIR/$role.so"
            printf '    "elf_sha256": "%s",\n' "$(sha256 "$ELF_DIR/$role.so")"
            printf '    "semantic_release_id": "%s",\n' "$(semantic_release_for "$role")"
            printf '    "attestation": "%s"\n' "$run/attestation/$role.json"
            printf '  },\n'
        done
        printf '  "market": null\n'
        printf '}\n'
    } > "$template.raw"
    jq . "$template.raw" > "$template" || die "assembled spec template is not valid JSON"

    local args=(run --walk "$walk" --spec-template "$template" \
        --transcript "$run/transcript.json" --relayer-bin "$RELAYER_BIN" --work "$run/vertical")
    if [ "$KEYPAIR_SEED" != "none" ]; then
        args+=(--keypair-seed "$KEYPAIR_SEED")
    fi
    if ! "$CAMPAIGN_BIN" "${args[@]}" > "$run/campaign.stdout" 2> "$run/campaign.stderr"; then
        tail -n 40 "$run/campaign.stderr" >&2
        echo "relayed-vertical: $walk walk FAILED; artifacts under $run" >&2
        printf '%s\n' "$run" > "$WORK/last-run"
        return 1
    fi
    printf '%s\n' "$run" > "$WORK/last-run"

    [ -f "$run/transcript.json" ] || die "transcript missing: $run/transcript.json"
    [ -f "$run/evidence.json" ] || die "evidence missing: $run/evidence.json"

    say "witnesses ($walk)"
    "$GAUNTLET/tier1/check-witnesses.sh" "$SCRIPT_DIR/witnesses.json" \
        "$run/evidence.json" "$run/transcript.json"

    say "$walk walk transcript summary"
    jq -r '
        "walk: \(.walk)   conservation: \(.conservation_verdict)",
        (.stages[] | "  \(.outcome | ascii_upcase | .[0:9])  \(.stage)"),
        (.walk_detail | to_entries[] | "    \(.key): \(.value)")
    ' "$run/transcript.json"
    return 0
}

STATUS=0
case "$WALK" in
    success) run_walk success || STATUS=1 ;;
    failure) run_walk failure || STATUS=1 ;;
    both)
        run_walk success || STATUS=1
        run_walk failure || STATUS=1
        ;;
esac
exit "$STATUS"
