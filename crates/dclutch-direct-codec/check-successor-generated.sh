#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
formal_root="$repo_root/formal/dclutch-semantics"
generated="$repo_root/crates/dclutch-direct-codec/src/generated_successor.rs"
candidate=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-successor.XXXXXX")
trap 'rm -f "$candidate"' EXIT HUP INT TERM

(
  cd "$formal_root"
  lake build DClutchSemantics.DirectSuccessorAbi >/dev/null
  lake env lean --run EmitDirectSuccessorAbiRust.lean >"$candidate"
)

test "$(wc -l <"$candidate" | tr -d ' ')" -gt 90
rustfmt --edition 2024 "$candidate"
cmp --silent "$candidate" "$generated"
