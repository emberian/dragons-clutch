#!/usr/bin/env bash
# The public wrapper's workflows call `tools/ci/run.sh <tier>`; the tiering lives
# in `tools/gate` now. Old tier names map to the gate's; delete this file once
# the wrapper calls tools/gate directly.
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
args=()
for a in "$@"; do
  case "$a" in
    census) args+=(emission) ;;      # was the emission coverage census
    emission) args+=(guards) ;;      # was the byte-identity guards run for real
    frameguard) args+=(frames) ;;
    genref) args+=(reference) ;;
    runbooks) args+=(commands) ;;
    *) args+=("$a") ;;
  esac
done
exec "$here/gate" "${args[@]}"
