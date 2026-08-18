#!/bin/sh
set -eu

# Run Verus against the exact executable probe source when a locally installed
# Verus release has been selected. No installer, network access, or source
# rewrite is performed here.
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
SOURCE_FILE="$REPO_DIR/toolchain/probes/no_std_core/src/lib.rs"

command -v verus >/dev/null 2>&1 || {
    printf '%s\n' 'BLOCKED: verus is not installed; install a reviewed pinned release offline before running this probe.'
    exit 2
}

printf 'verus_binary=%s\n' "$(command -v verus)"
printf 'verus_version=%s\n' "$(verus --version 2>&1 | head -n 1)"
printf 'source_sha256=%s\n' "$(shasum -a 256 "$SOURCE_FILE" | awk '{print $1}')"

# Keep this invocation intentionally explicit. A release record must capture
# the exact command and output before it is promoted to a proof result.
exec verus --edition 2021 --crate-type=lib "$SOURCE_FILE"
