#!/bin/sh
set -eu

crate_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_dir=$(CDPATH= cd -- "$crate_dir/../.." && pwd)
formal_dir="$repository_dir/formal/dclutch-semantics"
generated=$(mktemp "${TMPDIR:-/tmp}/dclutch-market-core-rust.XXXXXX")
generated_physical=$(mktemp "${TMPDIR:-/tmp}/dclutch-market-core-physical-rust.XXXXXX")
generated_retirement=$(mktemp "${TMPDIR:-/tmp}/dclutch-market-core-retirement-rust.XXXXXX")
trap 'rm -f "$generated" "$generated_physical" "$generated_retirement"' EXIT HUP INT TERM

(
  cd "$formal_dir"
  lake build DClutchSemantics.MarketCoreAbi
  lake build DClutchSemantics.MarketCorePhysicalAbi
  lake build DClutchSemantics.MarketRetirementV1Abi
  lake build DClutchSemantics.MarketCoreExamples
  lake env lean --run EmitMarketCoreRust.lean > "$generated"
  lake env lean --run EmitMarketCorePhysicalRust.lean > "$generated_physical"
  lake env lean --run EmitMarketRetirementV1Rust.lean > "$generated_retirement"
)

cmp "$generated" "$crate_dir/src/generated.rs"
cmp "$generated_physical" "$crate_dir/src/generated_physical.rs"
cmp "$generated_retirement" "$crate_dir/src/generated_retirement_v1.rs"
cargo fmt --check --manifest-path "$crate_dir/Cargo.toml"
cargo test --quiet --manifest-path "$crate_dir/Cargo.toml"
cargo clippy --quiet --manifest-path "$crate_dir/Cargo.toml" --all-targets -- -D warnings
