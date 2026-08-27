#!/bin/sh
set -eu

SPEC=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/ClutchKernel.v
TMP=$(mktemp -d "${TMPDIR:-/tmp}/clutch-rocq-check.XXXXXX")
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

if command -v rocq >/dev/null 2>&1; then
  echo "rocq=$(rocq --version 2>&1 | head -n 1)"
  # Keep generated .vo/.glob files out of the source tree.
  cp "$SPEC" "$TMP/ClutchKernel.v"
  (cd "$TMP" && rocq compile -q ClutchKernel.v)
  echo "status=PASS"
  exit 0
fi

if command -v coqc >/dev/null 2>&1; then
  echo "coqc=$(coqc --version 2>&1 | head -n 1)"
  cp "$SPEC" "$TMP/ClutchKernel.v"
  (cd "$TMP" && coqc -q ClutchKernel.v)
  echo "status=PASS"
  exit 0
fi

echo "status=UNAVAILABLE"
echo "reason=no rocq or coqc executable found on PATH"
exit 2
