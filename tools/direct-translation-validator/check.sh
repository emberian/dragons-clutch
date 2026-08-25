#!/bin/sh
set -eu

validator_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_dir=$(CDPATH= cd -- "$validator_dir/../.." && pwd)
corpus=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-translation.XXXXXX")
trap 'rm -f "$corpus"' EXIT HUP INT TERM

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

(
  cd "$repository_dir/formal/dclutch-semantics"
  lake build
  lake env lean --run EmitDirectTranslationCorpus.lean > "$corpus"
)

cargo run --quiet --manifest-path "$validator_dir/Cargo.toml" -- "$corpus"
cargo clippy --quiet --manifest-path "$validator_dir/Cargo.toml" --all-targets -- -D warnings

printf 'corpus_sha256=%s\n' "$(sha256_file "$corpus")"
printf 'lean_source_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectControllerCodec.lean")"
printf 'lean_direct_program_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectProgram.lean")"
printf 'lean_vm_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/TransitionVM.lean")"
printf 'rust_codec_sha256=%s\n' \
  "$(sha256_file "$repository_dir/crates/dclutch-direct-codec/src/lib.rs")"
printf 'rust_vm_sha256=%s\n' \
  "$(sha256_file "$repository_dir/crates/dclutch-transition-vm/src/lib.rs")"
printf 'program_include_sha256=%s\n' \
  "$(sha256_file "$repository_dir/programs/dclutch-controller-proof-sbf/src/generated_direct_program.rs")"
rustc -Vv
(cd "$repository_dir/formal/dclutch-semantics" && lake --version)
