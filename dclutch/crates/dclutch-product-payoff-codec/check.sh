#!/bin/sh
set -eu

crate_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_dir=$(CDPATH= cd -- "$crate_dir/../.." && pwd)
formal_dir="$repository_dir/formal/dclutch-semantics"
corpus=$(mktemp "${TMPDIR:-/tmp}/dclutch-product-payoff.XXXXXX")
generated=$(mktemp "${TMPDIR:-/tmp}/dclutch-product-payoff-rust.XXXXXX")
trap 'rm -f "$corpus" "$generated"' EXIT HUP INT TERM

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

(
  cd "$formal_dir"
  lake build
  mkdir -p .lake/build/lib/lean/DClutchSemantics
  lake env lean DClutchSemantics/ProductPayoff.lean \
    -o .lake/build/lib/lean/DClutchSemantics/ProductPayoff.olean
  lake env lean DClutchSemantics/ProductPayoffAbi.lean \
    -o .lake/build/lib/lean/DClutchSemantics/ProductPayoffAbi.olean
  lake env lean --run EmitProductPayoffRust.lean > "$generated"
  lake env lean --run EmitProductPayoffTranslationCorpus.lean > "$corpus"
)

cmp "$generated" "$crate_dir/src/generated.rs"
cargo fmt --check --manifest-path "$crate_dir/Cargo.toml"
cargo test --quiet --manifest-path "$crate_dir/Cargo.toml"
cargo clippy --quiet --manifest-path "$crate_dir/Cargo.toml" --all-targets -- -D warnings
cargo run --quiet --manifest-path "$crate_dir/Cargo.toml" --bin validate -- "$corpus"

printf 'corpus_sha256=%s\n' "$(sha256_file "$corpus")"
printf 'lean_payoff_sha256=%s\n' \
  "$(sha256_file "$formal_dir/DClutchSemantics/ProductPayoff.lean")"
printf 'lean_abi_sha256=%s\n' \
  "$(sha256_file "$formal_dir/DClutchSemantics/ProductPayoffAbi.lean")"
printf 'lean_generator_sha256=%s\n' \
  "$(sha256_file "$formal_dir/EmitProductPayoffRust.lean")"
printf 'lean_corpus_emitter_sha256=%s\n' \
  "$(sha256_file "$formal_dir/EmitProductPayoffTranslationCorpus.lean")"
printf 'generated_rust_sha256=%s\n' \
  "$(sha256_file "$crate_dir/src/generated.rs")"
rustc -Vv
(cd "$formal_dir" && lake --version)
