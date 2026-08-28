#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
CHECKER="$HERE/check_sbf_build_freshness.py"
RUNNER="$HERE/checked-release-candidate.sh"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-release-freshness.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

RUN_ID="$(printf 'ab%.0s' {1..32})"
OTHER_RUN_ID="$(printf 'cd%.0s' {1..32})"
EXPECTED="$SCRATCH/expected.tsv"
DIAGNOSTICS="$SCRATCH/diagnostics.txt"

pass=0
fail=0
ok() { pass=$((pass + 1)); printf 'ok %s - %s\n' "$pass" "$1"; }
not_ok() { fail=$((fail + 1)); printf 'not ok - %s\n' "$1" >&2; }

expect_pass() {
    local name=$1; shift
    if "$@" >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"; then ok "$name"; else
        sed -n '1,8p' "$SCRATCH/stderr" >&2
        not_ok "$name"
    fi
}

expect_refusal() {
    local name=$1 needle=$2; shift 2
    if "$@" >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"; then
        not_ok "$name (unexpected success)"
    elif grep -Fq -- "$needle" "$SCRATCH/stderr"; then
        ok "$name"
    else
        sed -n '1,8p' "$SCRATCH/stderr" >&2
        not_ok "$name (wrong refusal)"
    fi
}

write_manifests() {
    printf 'core\tdclutch-core-sbf\nclaims\tdclutch-claims-sbf\n' > "$EXPECTED"
    printf 'core=0\nclaims=0\n' > "$DIAGNOSTICS"
}

write_fresh_log() {
    local label=$1 package=$2 run_id=${3:-$RUN_ID}
    {
        printf 'dclutch-sbf-build-run-v1=%s\n' "$run_id"
        printf '   Compiling %s v0.1.0 (/scratch/programs/%s)\n' "$package" "$package"
        printf '    Finished release [optimized] target(s) in 1.00s\n'
    } > "$SCRATCH/build-$label.log"
}

check() {
    "$CHECKER" --work "$SCRATCH" --expected "$EXPECTED" \
        --diagnostics "$DIAGNOSTICS" --run-id "$RUN_ID"
}

write_manifests
write_fresh_log core dclutch-core-sbf
write_fresh_log claims dclutch-claims-sbf
expect_pass "matching run stamps and top-package compile markers pass" check

rm "$SCRATCH/build-claims.log"
expect_refusal "missing build log refuses" "missing build log for claims" check

write_fresh_log claims dclutch-claims-sbf
{
    printf 'dclutch-sbf-build-run-v1=%s\n' "$RUN_ID"
    printf '    Finished release [optimized] target(s) in 0.01s\n'
} > "$SCRATCH/build-claims.log"
expect_refusal "warm build with no compile marker refuses" \
    "no fresh top-package compile marker for dclutch-claims-sbf" check

write_fresh_log claims dclutch-claims-sbf "$OTHER_RUN_ID"
expect_refusal "stale log from another run refuses" \
    "belongs to a different or unstamped run" check

write_fresh_log claims dclutch-core-sbf
expect_refusal "dependency compile marker cannot stand in for top package" \
    "no fresh top-package compile marker for dclutch-claims-sbf" check

write_fresh_log claims dclutch-claims-sbf
printf 'core=0\n' > "$DIAGNOSTICS"
expect_refusal "missing diagnostics row refuses" "diagnostics labels differ" check

write_manifests
ln -sf "$SCRATCH/build-core.log" "$SCRATCH/build-claims.log"
expect_refusal "symlink build log refuses" "is not a regular file" check
rm "$SCRATCH/build-claims.log"
write_fresh_log claims dclutch-claims-sbf

printf 'core\tdclutch-core-sbf\ncore\tdclutch-claims-sbf\n' > "$EXPECTED"
expect_refusal "duplicate expected link label refuses" "duplicate expected label: core" check

write_manifests
printf 'core=zero\nclaims=0\n' > "$DIAGNOSTICS"
expect_refusal "nonnumeric diagnostic count refuses" \
    "diagnostics row 1 has an unsafe label or count" check

write_manifests
expect_refusal "malformed build run identifier refuses" \
    "--run-id must be 64 lowercase hexadecimal characters" \
    "$CHECKER" --work "$SCRATCH" --expected "$EXPECTED" \
        --diagnostics "$DIAGNOSTICS" --run-id not-a-run-id

expect_refusal "legacy keep-elf mode refuses before using stale evidence" \
    "refusing --keep-elf" "$RUNNER" --work "$SCRATCH/keep" --keep-elf

if grep -Fq 'cargo build-sbf --manifest-path "programs/$package/Cargo.toml" -- --locked' "$RUNNER" \
    && grep -Fq "build_command=cargo build-sbf --manifest-path programs/%s/Cargo.toml -- --locked" "$RUNNER"; then
    ok "release builds and recorded commands require the committed lockfiles"
else
    not_ok "release runner lost its locked-build admission"
fi

if grep -Fq 'CHECKED_UPGRADE_GATE.json' "$RUNNER" \
    && grep -Fq 'checked Upgrade admission requires the exact 13-link shipped set' "$RUNNER" \
    && grep -Fq 'RUSTFLAGS="-Zemit-stack-sizes --emit=obj,link"' "$RUNNER" \
    && grep -Fq 'frames_at_or_over_bound=0' "$RUNNER" \
    && grep -Fq 'checked_upgrade_gate_sha256=' "$RUNNER"; then
    ok "generated Upgrade gate remains behind exact all-13 fresh frame admission"
else
    not_ok "generated Upgrade gate lost an all-link/frame admission seam"
fi

if [ "$fail" -ne 0 ]; then
    printf '%s tests failed; %s passed\n' "$fail" "$pass" >&2
    exit 1
fi
printf 'all %s release-freshness tests passed\n' "$pass"
