#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd -P)
TOOL="$ROOT/tools/local-validator/dclutch-local-validator"

[[ -x "$TOOL" ]] || { printf 'tool is not executable: %s\n' "$TOOL" >&2; exit 1; }
"$TOOL" verify-fixtures | grep -qx 'verified 10 committed Pyth fixture artifacts'

stderr_file=$(mktemp /tmp/dclutch-local-validator-test.XXXXXX)
if "$TOOL" start --ledger /tmp/not-a-fresh-ledger 2>"$stderr_file"; then
  printf 'incomplete start unexpectedly succeeded\n' >&2
  exit 1
fi
grep -q 'start requires --ledger' "$stderr_file"

grep -q 'release_bound_provider_loader_evidence' "$TOOL"
grep -q -- '--bpf-program' "$ROOT/tools/local-validator/README.md"
printf 'local-validator shell tests passed\n'
