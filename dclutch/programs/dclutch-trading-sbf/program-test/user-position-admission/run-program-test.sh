#!/usr/bin/env bash
# Real-SBF evidence for the wallet-authorized Trading -> Claims Position outer.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-user-position-admission.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"

diagnostics=0
for manifest in \
  programs/dclutch-trading-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-rent-sbf/Cargo.toml
do
  label="$(basename "$(dirname "$manifest")")"
  log="$sbf_out/build-$label.log"
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out" \
    > "$log" 2>&1 || { tail -n 60 "$log" >&2; exit 1; }
  count="$(grep -c 'overwrites values in the frame' "$log" || true)"
  diagnostics=$((diagnostics + count))
  printf '  built %-48s %s frame diagnostics\n' "$manifest" "${count:-0}"
  if [ "${count:-0}" != "0" ]; then
    grep 'overwrites values in the frame' "$log" | sort -u >&2
  fi
done
if [ "$diagnostics" -ne 0 ]; then
  echo "user-position-admission: refusing -- $diagnostics SBF stack-frame-overwrite diagnostics" >&2
  exit 1
fi

SBF_OUT_DIR="$sbf_out" cargo test --locked \
  --manifest-path programs/dclutch-trading-sbf/program-test/user-position-admission/Cargo.toml \
  --test lifecycle -- --nocapture
