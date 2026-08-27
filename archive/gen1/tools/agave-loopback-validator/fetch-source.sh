#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR
# shellcheck source=common.sh
source "$here/common.sh"

require_command git
require_command shasum
require_command awk

if [ ! -e "$source_dir" ]; then
  mkdir -p "$cache_root"
  git init -q "$source_dir"
  git -C "$source_dir" remote add origin "$AGAVE_UPSTREAM_URL"
  git -C "$source_dir" fetch --depth=1 origin "$AGAVE_COMMIT"
  git -C "$source_dir" checkout -q --detach FETCH_HEAD
elif [ ! -d "$source_dir/.git" ]; then
  die "refusing non-Git source path: $source_dir"
fi

head="$(git -C "$source_dir" rev-parse HEAD)"
[ "$head" = "$AGAVE_COMMIT" ] ||
  die "existing source HEAD is $head, expected $AGAVE_COMMIT"

if git -C "$source_dir" diff --quiet -- &&
   git -C "$source_dir" diff --cached --quiet --; then
  require_sha256 \
    "$source_dir/quic-client/src/nonblocking/quic_client.rs" \
    "$AGAVE_UPSTREAM_QUIC_CLIENT_SHA256"
  require_sha256 \
    "$source_dir/udp-client/src/lib.rs" \
    "$AGAVE_UPSTREAM_UDP_CLIENT_SHA256"
  require_sha256 \
    "$source_dir/validator/src/bin/solana-test-validator.rs" \
    "$AGAVE_UPSTREAM_CLI_SHA256"
  require_sha256 \
    "$source_dir/test-validator/src/lib.rs" \
    "$AGAVE_UPSTREAM_LIBRARY_SHA256"
  git -C "$source_dir" apply --unidiff-zero --check "$patch_path"
  git -C "$source_dir" apply --unidiff-zero "$patch_path"
fi

verify_checkout
echo "prepared pinned patched source: $source_dir"
