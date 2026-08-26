#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
formal_root="$repo_root/formal/dclutch-semantics"
generated_intent="$repo_root/crates/dclutch-direct-codec/src/generated_intent_v2.rs"
generated_successor="$repo_root/crates/dclutch-direct-codec/src/generated_successor.rs"
candidate_intent=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-intent-v2.XXXXXX")
candidate_successor=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-successor.XXXXXX")
trap 'rm -f "$candidate_intent" "$candidate_successor"' EXIT HUP INT TERM

(
  cd "$formal_root"
  lake build DClutchSemantics.DirectIntentV2Codec >/dev/null
  lake build DClutchSemantics.DirectSuccessorAbi >/dev/null
  lake env lean --run EmitDirectIntentV2Rust.lean >"$candidate_intent"
  lake env lean --run EmitDirectSuccessorAbiRust.lean >"$candidate_successor"
)

test "$(wc -l <"$candidate_intent" | tr -d ' ')" -gt 45
test "$(wc -l <"$candidate_successor" | tr -d ' ')" -gt 90
rustfmt --edition 2024 "$candidate_intent" "$candidate_successor"
cmp --silent "$candidate_intent" "$generated_intent"
cmp --silent "$candidate_successor" "$generated_successor"
