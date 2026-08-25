#!/bin/sh
set -eu

crate_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_dir=$(CDPATH= cd -- "$crate_dir/../.." && pwd)
formal_dir="$repository_dir/formal/dclutch-semantics"
generated=$(mktemp "${TMPDIR:-/tmp}/dclutch-market-core-rust.XXXXXX")
generated_physical=$(mktemp "${TMPDIR:-/tmp}/dclutch-market-core-physical-rust.XXXXXX")
trap 'rm -f "$generated" "$generated_physical"' EXIT HUP INT TERM

(
  cd "$formal_dir"
  lake build DClutchSemantics.MarketCoreAbi
  lake build DClutchSemantics.MarketCorePhysicalAbi
  lake build DClutchSemantics.MarketCoreExamples
  lake env lean --run EmitMarketCoreRust.lean > "$generated"
  lake env lean --run EmitMarketCorePhysicalRust.lean > "$generated_physical"
)

cmp "$generated" "$crate_dir/src/generated.rs"
cmp "$generated_physical" "$crate_dir/src/generated_physical.rs"
cargo fmt --check --manifest-path "$crate_dir/Cargo.toml"
cargo test --quiet --manifest-path "$crate_dir/Cargo.toml"
cargo clippy --quiet --manifest-path "$crate_dir/Cargo.toml" --all-targets -- -D warnings
