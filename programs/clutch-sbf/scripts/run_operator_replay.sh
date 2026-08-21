#!/usr/bin/env bash
# The Operator Bench replay falsifier.
#
# The bench's central claim is that the browser cannot originate a
# transaction: every byte that reaches the bank is built by
# `clutch_sbf_harness`, the same library the sealed lane's plan generator is.
# This gate makes that claim checkable rather than asserted.
#
# Three verdicts, all required:
#
#   1. the plan `operatord` emits through the library is byte-identical to the
#      plan the `clutch-sbf-harness` CLI writes -- one builder, two callers;
#   2. `operatord replay` rebuilds every file of a plan through those builders
#      and finds zero differences across all 44 transactions;
#   3. corrupting one byte of one emitted transaction turns the replay RED.
#
# This is a byte comparison between two callers of one function plus a
# corruption that must be caught.  It is not a proof about the wire format and
# it is not translation validation; there is no formal semantics of Rust to do
# that against.  Described here at exactly the resolution it has.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-operator-replay.XXXXXX")"
plan="$work/plan"
cli="$work/cli"
trap 'rm -rf "$work"' EXIT

operatord=(cargo run --offline --quiet --manifest-path "$root/operatord/Cargo.toml" --)
harness=(cargo run --offline --quiet --manifest-path "$root/Cargo.toml" -p clutch-sbf-harness --)

echo "== source =="
(cd "$root/../.." && git rev-parse HEAD)

echo
echo "== 1. one builder, two callers =="
CARGO_NET_OFFLINE=true "${operatord[@]}" emit "$plan" >"$work/emit.log" 2>&1
mkdir -p "$cli"
CARGO_NET_OFFLINE=true "${harness[@]}" "$cli" --general-clearing >"$work/cli.log" 2>&1
if ! diff -r "$cli" "$plan" >"$work/cross.diff" 2>&1; then
  echo "FAIL: the library plan and the CLI plan differ"
  head -40 "$work/cross.diff"
  exit 1
fi
files="$(find "$plan" -type f | wc -l | tr -d ' ')"
transactions="$(find "$plan/tx" -type f -name '*.b64' | wc -l | tr -d ' ')"
echo "  library plan == CLI plan: $files files, $transactions transactions, byte identical"

echo
echo "== 2. replay: rebuild every transaction through the builders =="
CARGO_NET_OFFLINE=true "${operatord[@]}" replay "$plan" | tee "$work/replay.log" | grep '^replay'

echo
echo "== 3. falsifiability: corrupt one transaction byte and require red =="
victim="$plan/tx/general-22-place-single-buy.b64"
cp "$victim" "$victim.orig"
python3 - "$victim" <<'PY'
import sys
path = sys.argv[1]
text = open(path).read().strip()
alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
first = alphabet[(alphabet.index(text[0]) + 1) % len(alphabet)]
open(path, "w").write(first + text[1:] + "\n")
PY
if CARGO_NET_OFFLINE=true "${operatord[@]}" replay "$plan" >"$work/falsify.log" 2>&1; then
  echo "FAIL: a corrupted transaction still replayed green"
  mv "$victim.orig" "$victim"
  exit 1
fi
if ! grep -q 'DIFFERS tx/general-22-place-single-buy.b64' "$work/falsify.log"; then
  echo "FAIL: the negative run went red for an unrelated reason"
  tail -30 "$work/falsify.log"
  mv "$victim.orig" "$victim"
  exit 1
fi
grep -m1 'DIFFERS' "$work/falsify.log" | sed 's/^/  red: /'
mv "$victim.orig" "$victim"

echo
echo "operator_replay_files_compared=$files"
echo "operator_replay_transactions=$transactions"
echo "operator_replay_cross_check=IDENTICAL"
echo "operator_replay=PASS"
echo "operator_replay_falsifiability=PASS"
