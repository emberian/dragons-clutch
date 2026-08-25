#!/bin/sh
set -eu

crate_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_dir=$(CDPATH= cd -- "$crate_dir/../.." && pwd)
formal_dir="$repository_dir/formal/dclutch-semantics"
generated=$(mktemp "${TMPDIR:-/tmp}/dclutch-market-core-rust.XXXXXX")
trap 'rm -f "$generated"' EXIT HUP INT TERM

(
  cd "$formal_dir"
  lake build DClutchSemantics.MarketCoreAbi
  lake env lean --run EmitMarketCoreRust.lean > "$generated"
)

cmp "$generated" "$crate_dir/src/generated.rs"
cargo fmt --check --manifest-path "$crate_dir/Cargo.toml"
cargo test --quiet --manifest-path "$crate_dir/Cargo.toml"
cargo clippy --quiet --manifest-path "$crate_dir/Cargo.toml" --all-targets -- -D warnings
