#!/bin/sh
set -eu

# Run Verus against the exact executable probe source using the reviewed,
# pinned Verus release. No installer, network access, or source rewrite is
# performed here.
#
# The pin recorded below is authoritative. See toolchain/PINNED_PROOF_TOOLS.md
# for the release provenance (tag, commit, artifact digest, install prefix) and
# toolchain/versions.env for the machine-readable snapshot. Changing any pinned
# constant in this file without refreshing both of those documents is a
# review defect, not a configuration tweak.
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
SOURCE_FILE="$REPO_DIR/toolchain/probes/no_std_core/src/lib.rs"

# Reviewed pin. Retrieved 2026-08-18.
VERUS_PINNED_VERSION='0.2026.08.15.7d4628a'
VERUS_PINNED_COMMIT='7d4628a8543d3e51e6e314c52032c9bab43f0f53'
VERUS_PINNED_TOOLCHAIN='1.97.1-aarch64-apple-darwin'
VERUS_PINNED_PREFIX_DEFAULT="$HOME/toolchains/verus-0.2026.08.15.7d4628a/verus-arm64-macos"
VERUS_PREFIX=${VERUS_PREFIX:-$VERUS_PINNED_PREFIX_DEFAULT}

# The probe source is itself pinned. A Verus run against a different source
# than the reviewed one is not evidence about the reviewed one.
SOURCE_SHA256_PIN='10b2087683d3c2cb423768eb9c612c00ea929b171835c15d3d16792d6b8b19ac'

# Resolve the pinned binary first. The PATH lookup is retained only so that the
# original refusal fires when no Verus is present at all; a PATH-provided Verus
# is still subject to the version pin check below.
if [ -x "$VERUS_PREFIX/verus" ]; then
    VERUS_BIN="$VERUS_PREFIX/verus"
elif command -v verus >/dev/null 2>&1; then
    VERUS_BIN=$(command -v verus)
else
    printf '%s\n' 'BLOCKED: verus is not installed; install a reviewed pinned release offline before running this probe.'
    exit 2
fi

printf 'verus_binary=%s\n' "$VERUS_BIN"
printf 'verus_version=%s\n' "$("$VERUS_BIN" --version 2>&1 | head -n 1)"

VERUS_OBSERVED_VERSION=$("$VERUS_BIN" --version 2>&1 |
    awk '/Version:/ { print $2; exit }')
VERUS_OBSERVED_TOOLCHAIN=$("$VERUS_BIN" --version 2>&1 |
    awk '/Toolchain:/ { print $2; exit }')
printf 'verus_observed_version=%s\n' "$VERUS_OBSERVED_VERSION"
printf 'verus_observed_toolchain=%s\n' "$VERUS_OBSERVED_TOOLCHAIN"
printf 'verus_pinned_version=%s\n' "$VERUS_PINNED_VERSION"
printf 'verus_pinned_commit=%s\n' "$VERUS_PINNED_COMMIT"
printf 'verus_pinned_toolchain=%s\n' "$VERUS_PINNED_TOOLCHAIN"

if [ "$VERUS_OBSERVED_VERSION" != "$VERUS_PINNED_VERSION" ]; then
    printf '%s\n' 'BLOCKED: resolved verus does not match the reviewed pin; refusing to report an off-pin run as probe evidence.'
    exit 3
fi

if [ "$VERUS_OBSERVED_TOOLCHAIN" != "$VERUS_PINNED_TOOLCHAIN" ]; then
    printf '%s\n' 'BLOCKED: resolved verus reports an unpinned Rust frontend toolchain; refusing to run.'
    exit 3
fi

# Record the solver actually reachable by this Verus, since the release ships
# its own z3 next to the binary and that copy takes precedence.
if [ -x "$VERUS_PREFIX/z3" ]; then
    printf 'z3_binary=%s\n' "$VERUS_PREFIX/z3"
    printf 'z3_version=%s\n' "$("$VERUS_PREFIX/z3" --version 2>&1 | head -n 1)"
fi

SOURCE_SHA256=$(shasum -a 256 "$SOURCE_FILE" | awk '{print $1}')
printf 'source_sha256=%s\n' "$SOURCE_SHA256"
printf 'source_sha256_pin=%s\n' "$SOURCE_SHA256_PIN"

if [ "$SOURCE_SHA256" != "$SOURCE_SHA256_PIN" ]; then
    printf '%s\n' 'BLOCKED: probe source digest does not match the reviewed pin; refusing to run Verus against an unreviewed source.'
    exit 4
fi

# Keep this invocation intentionally explicit. A release record must capture
# the exact command and output before it is promoted to a proof result.
# A non-zero exit from here is a real Verus result and must be reported as
# such, never suppressed.
exec "$VERUS_BIN" --edition 2021 --crate-type=lib "$SOURCE_FILE"
