#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
CHECKER="$HERE/check_sbf_build_freshness.py"
RUNNER="$HERE/checked-release-candidate.sh"
PROVENANCE="$HERE/artifact_provenance.py"
PUBLIC_ROUTE="$HERE/public_route_campaign.py"
DEVNET_DIRECT_LIFECYCLE="$HERE/devnet_direct_lifecycle.py"
NODE_ARCHIVE_LISTER="$HERE/node_archive_members.py"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-release-freshness.XXXXXX")"
SCRATCH="$(cd "$SCRATCH" && pwd -P)"
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
        printf 'dclutch-sbf-build-invocation-v1=CARGO_TARGET_DIR=fixture cargo build-sbf --manifest-path programs/%s/Cargo.toml -- --locked\n' "$package"
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
    printf 'dclutch-sbf-build-invocation-v1=fresh but warm fixture\n'
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
sed -i.bak '2d' "$SCRATCH/build-claims.log"
rm "$SCRATCH/build-claims.log.bak"
expect_refusal "missing build invocation stamp refuses" \
    "omitted its exact invocation stamp" check

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

expect_refusal "unknown builder refuses before any build" \
    "--builder must be local, persvati, container, or hbox" \
    "$RUNNER" --work "$SCRATCH/builder-unknown" --builder other

expect_refusal "hbox label refuses outside swarm-build" \
    "--builder hbox requires the whole runner to execute inside swarm-build" \
    "$RUNNER" --work "$SCRATCH/builder-hbox" --builder hbox

expect_refusal "hbox label is admitted inside the scheduler boundary" \
    "--predecessor-profile is required" \
    env SWARM_BUILD_INNER=1 "$RUNNER" --work "$SCRATCH/builder-hbox-inner" --builder hbox

# THE NAMED RELEASE BUILDER ARTIFACT, asserted on whichever host is running
# this. Both branches assert something: nothing here is skipped, because a
# skip that prints `ok` is the failure mode this file exists to refuse.
#
# The gate sits after every argument refusal and before the Node gate, so on a
# host that is not the named artifact the Node gate is unreachable without
# saying --diagnostic-builder out loud -- which is the second assertion.
if [ "$(uname -s)/$(uname -m)" = "Linux/x86_64" ]; then
    expect_refusal "the named release builder host passes the gate and reaches the Node gate" \
        "--node is required" \
        "$RUNNER" --work "$SCRATCH/builder-host" --genesis-cohort
else
    expect_refusal "a host that is not the named release builder artifact refuses before the Node gate" \
        "named release builder" \
        "$RUNNER" --work "$SCRATCH/builder-host" --genesis-cohort
    expect_refusal "--diagnostic-builder carries a NON-RELEASE build past the gate" \
        "--node is required" \
        "$RUNNER" --work "$SCRATCH/builder-diag" --genesis-cohort --diagnostic-builder
fi

expect_refusal "relative predecessor profile refuses before source or build work" \
    "--predecessor-profile must be an absolute canonical path" \
    "$RUNNER" --work "$SCRATCH/predecessor-relative" --predecessor-profile Cargo.toml

printf 'short\n' > "$SCRATCH/predecessor-short.bin"
expect_refusal "wrong-width predecessor profile refuses before source or build work" \
    "--predecessor-profile must be exactly 144 bytes" \
    "$RUNNER" --work "$SCRATCH/predecessor-short" \
        --predecessor-profile "$SCRATCH/predecessor-short.bin"

dd if=/dev/zero of="$SCRATCH/predecessor.bin" bs=144 count=1 2>/dev/null
ln -s "$SCRATCH/predecessor.bin" "$SCRATCH/predecessor-link.bin"
expect_refusal "symlink predecessor profile refuses before source or build work" \
    "--predecessor-profile must not be a symlink" \
    "$RUNNER" --work "$SCRATCH/predecessor-link" \
        --predecessor-profile "$SCRATCH/predecessor-link.bin"

# --diagnostic-builder on both of the Node-gate cases below, because the Node
# gate is downstream of the release-builder gate and these two are about Node.
# On the named builder host the flag is a no-op; anywhere else it is the only
# way to reach a refusal past the gate, and saying it is the point.
expect_refusal "ambient Node is never accepted for the Product handoff gate" \
    "--node is required for the source-pinned Product handoff gate" \
    "$RUNNER" --work "$SCRATCH/node-required" --diagnostic-builder \
        --predecessor-profile "$SCRATCH/predecessor.bin"

# Infrastructure lineage is a stated choice, and both ways of failing to state
# it refuse before any source or build work. Omission cannot silently select a
# succession, and a genesis cannot silently carry a predecessor it does not
# succeed -- the two candidates commit different profile VERSIONS to different
# chain acts, so an ambiguous invocation must never reach a build.
expect_refusal "a genesis candidate refuses a predecessor it does not succeed" \
    "--genesis-cohort and --predecessor-profile are mutually exclusive" \
    "$RUNNER" --work "$SCRATCH/lineage-both" --genesis-cohort \
        --predecessor-profile "$SCRATCH/predecessor.bin"

expect_refusal "stating no lineage at all refuses before source or build work" \
    "For a cohort that succeeds nothing, pass --genesis-cohort instead" \
    "$RUNNER" --work "$SCRATCH/lineage-neither"

# A genesis candidate is the only one a cold machine can build, so its own
# arguments must clear the lineage gate and stop at the NEXT honest wall (the
# pinned Node runtime) rather than at a predecessor it will never have.
expect_refusal "a genesis candidate needs no predecessor and reaches the Node gate" \
    "--node is required for the source-pinned Product handoff gate" \
    "$RUNNER" --work "$SCRATCH/lineage-genesis" --genesis-cohort --diagnostic-builder

if grep -Fq 'run_tool derive-genesis-infrastructure-profile' "$RUNNER" \
    && grep -Fq 'infrastructure_lineage=genesis' "$RUNNER" \
    && grep -Fq 'infrastructure_lineage=succession' "$RUNNER" \
    && grep -Fq 'predecessor_infrastructure_profile=none' "$RUNNER"; then
    ok "each lineage names itself in the summary and derives its own profile version"
else
    not_ok "each lineage names itself in the summary and derives its own profile version"
fi

if grep -Fq 'cargo build-sbf --manifest-path "programs/$package/Cargo.toml" -- --locked' "$RUNNER" \
    && grep -Fq "build_command=cargo build-sbf --manifest-path programs/%s/Cargo.toml -- --locked" "$RUNNER" \
    && grep -Fq 'cargo build --release --locked --offline -p dclutch-release-tool' "$RUNNER"; then
    ok "release and host-tool builds require the committed lockfiles"
else
    not_ok "release runner lost its locked-build admission"
fi

if grep -Fq 'cargo-locks-before.tsv' "$RUNNER" \
    && grep -Fq 'cargo-locks-after.tsv' "$RUNNER" \
    && grep -Fq 'cargo_lock_immutability=passed' "$RUNNER" \
    && grep -Fq 'refusing: Cargo.lock set changed while building the candidate' "$RUNNER"; then
    ok "complete Cargo.lock set is byte-compared around the candidate build"
else
    not_ok "release runner lost its repository-wide lock immutability proof"
fi

if grep -Fq 'CHECKED_UPGRADE_GATE.json' "$RUNNER" \
    && grep -Fq 'checked Upgrade admission requires the exact shipped link set' "$RUNNER" \
    && grep -Fq 'SHIPPED_LINKS_AUTHORITY="$SOURCE/tools/local-validator/bootstrap/successor/src/upgrade.rs"' "$RUNNER" \
    && ! grep -Eq '^SHIPPED_LINK_COUNT=[0-9]' "$RUNNER" \
    && grep -Fq 'RUSTFLAGS="-Zemit-stack-sizes --emit=obj,link"' "$RUNNER" \
    && grep -Fq 'frames_at_or_over_bound=0' "$RUNNER" \
    && grep -Fq 'checked_upgrade_gate_sha256=' "$RUNNER"; then
    ok "generated Upgrade gate remains behind exact all-link fresh frame admission"
else
    not_ok "generated Upgrade gate lost an all-link/frame admission seam"
fi

if grep -Fq 'SUCCESSOR_CAMPAIGN_PACK.json' "$RUNNER" \
    && grep -Fq 'successor_campaign_pack.py' "$RUNNER" \
    && grep -Fq 'public_route_campaign.py' "$HERE/successor_campaign_pack.py" \
    && grep -Fq 'devnet_direct_lifecycle.py' "$HERE/successor_campaign_pack.py" \
    && [ -x "$PUBLIC_ROUTE" ] \
    && [ -x "$DEVNET_DIRECT_LIFECYCLE" ] \
    && grep -Fq 'python3 "$CAMPAIGN_PACK_TOOL" emit --root "$WORK"' "$RUNNER"; then
    ok "strict checked candidate emits its source-pinned successor campaign pack"
else
    not_ok "release runner lost its successor campaign-pack integration"
fi

if grep -Fq 'PINNED_PREDECESSOR_PROFILE="$INFRA_DIR/predecessor-profile.bin"' "$RUNNER" \
    && grep -Fq 'cp "$PREDECESSOR_PROFILE" "$PINNED_PREDECESSOR_PROFILE"' "$RUNNER" \
    && grep -Fq -- '--predecessor-profile "$PINNED_PREDECESSOR_PROFILE"' "$RUNNER" \
    && grep -Fq 'predecessor_infrastructure_profile_sha256=' "$RUNNER"; then
    ok "candidate preserves and derives only from its admitted predecessor profile"
else
    not_ok "release runner lost its cold-reproducible predecessor input"
fi

if grep -Fq 'NODE_ARCHIVE_EXPECTED_SHA256="5c4286dcd5bbd5acb1ccc7eb0e088bd5eb1e3affad671ee9364004f8f6a4a431"' "$RUNNER" \
    && grep -Fq 'NODE_VERSION" = "v26.4.0"' "$RUNNER" \
    && [ -x "$NODE_ARCHIVE_LISTER" ] \
    && grep -Fq 'python3 "$NODE_ARCHIVE_LISTER"' "$RUNNER" \
    && grep -Fq 'grep -Fxc "$NODE_ARCHIVE_MEMBER" "$NODE_ARCHIVE_LISTING"' "$RUNNER" \
    && ! grep -Fq 'tar -tf "$NODE_ARCHIVE" | grep' "$RUNNER" \
    && grep -Fq 'source/tools/release/node_archive_members.py' "$HERE/successor_campaign_pack.py" \
    && grep -Fq 'PINNED_NODE_ARCHIVE="$TOOLCHAIN_DIR/$NODE_ARCHIVE_NAME"' "$RUNNER" \
    && grep -Fq 'cargo build --release --locked --offline' "$RUNNER" \
    && grep -Fq 'spline-product-handoff-smoke.sh' "$RUNNER" \
    && grep -Fq 'spline_product_handoff=passed' "$RUNNER" \
    && grep -Fq 'host_rustc_verbose_sha256=' "$RUNNER" \
    && grep -Fq '"host_substrate": host_substrate' "$HERE/successor_campaign_pack.py"; then
    ok "candidate authenticates Node and records the public spline Product build substrate"
else
    not_ok "release runner lost its pinned public spline Product handoff gate"
fi

if grep -Fq 'rm -f "$link_target/deploy/$stem.so"' "$RUNNER" \
    && grep -Fq 'artifact_provenance.py' "$RUNNER" \
    && grep -Fq 'emit-gate' "$RUNNER" \
    && grep -Fq '"artifact_provenance": evidence(' "$PROVENANCE" \
    && grep -Fq -- '--frame-object "frame-target-$label/$TARGET_TRIPLE/release/deps/$object_stem.o"' "$RUNNER"; then
    ok "each gate link binds a newly emitted ELF to exact source/build/frame provenance"
else
    not_ok "release runner lost its named-link artifact provenance binding"
fi

if [ "$fail" -ne 0 ]; then
    printf '%s tests failed; %s passed\n' "$fail" "$pass" >&2
    exit 1
fi
printf 'all %s release-freshness tests passed\n' "$pass"
