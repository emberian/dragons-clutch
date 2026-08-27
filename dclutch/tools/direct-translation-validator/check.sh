#!/bin/sh
set -eu

if [ "$#" -gt 1 ]; then
  printf 'usage: %s [NEW_EVIDENCE_DIRECTORY]\n' "$0" >&2
  exit 2
fi

validator_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_dir=$(CDPATH= cd -- "$validator_dir/../.." && pwd)
corpus=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-translation.XXXXXX")
validator_result=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-validator-result.XXXXXX")
rustc_verbose=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-rustc.XXXXXX")
lake_version=$(mktemp "${TMPDIR:-/tmp}/dclutch-direct-lake.XXXXXX")
trap 'rm -f "$corpus" "$validator_result" "$rustc_verbose" "$lake_version"' EXIT HUP INT TERM

evidence_dir=${1-}
if [ -n "$evidence_dir" ]; then
  if [ -e "$evidence_dir" ]; then
    printf 'refusing to overwrite evidence path: %s\n' "$evidence_dir" >&2
    exit 2
  fi
  mkdir -p "$evidence_dir"
fi

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
  lake env lean --run EmitRegisteredCreationTranslationCorpus.lean >> "$corpus"
)

"$validator_dir/check-generated.sh"

cargo run --quiet --manifest-path "$validator_dir/Cargo.toml" -- "$corpus" | tee "$validator_result"
cargo clippy --quiet --manifest-path "$validator_dir/Cargo.toml" --all-targets -- -D warnings

rustc -Vv | tee "$rustc_verbose"
(cd "$repository_dir/formal/dclutch-semantics" && lake --version) | tee "$lake_version"

if [ -n "$evidence_dir" ]; then
  cp "$corpus" "$evidence_dir/corpus.bin"
  cp "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectControllerCodec.lean" \
    "$evidence_dir/lean_direct_controller_codec.bin"
  cp "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectProgram.lean" \
    "$evidence_dir/lean_direct_program.bin"
  cp "$repository_dir/formal/dclutch-semantics/DClutchSemantics/TransitionVM.lean" \
    "$evidence_dir/lean_transition_vm.bin"
  cp "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectLifecycle.lean" \
    "$evidence_dir/lean_direct_lifecycle.bin"
  cp "$repository_dir/formal/dclutch-semantics/DClutchSemantics/RegisteredControllerAbi.lean" \
    "$evidence_dir/lean_registered_controller_abi.bin"
  cp "$repository_dir/formal/dclutch-semantics/DClutchSemantics/RegisteredPhysical.lean" \
    "$evidence_dir/lean_registered_physical.bin"
  cp "$repository_dir/formal/dclutch-semantics/EmitDirectTranslationCorpus.lean" \
    "$evidence_dir/lean_direct_corpus_emitter.bin"
  cp "$repository_dir/formal/dclutch-semantics/EmitRegisteredCreationTranslationCorpus.lean" \
    "$evidence_dir/lean_registered_creation_corpus_emitter.bin"
  cp "$repository_dir/crates/dclutch-direct-codec/src/lib.rs" \
    "$evidence_dir/rust_direct_codec.bin"
  cp "$repository_dir/crates/dclutch-transition-vm/src/lib.rs" \
    "$evidence_dir/rust_transition_vm.bin"
  cp "$repository_dir/crates/dclutch-direct-aot-contract/src/lib.rs" \
    "$evidence_dir/rust_direct_aot.bin"
  cp "$repository_dir/crates/dclutch-direct-aot-contract/src/generated.rs" \
    "$evidence_dir/rust_direct_aot_generated.bin"
  cp "$validator_dir/src/main.rs" "$evidence_dir/rust_validator.bin"
  cp "$validator_dir/src/registration.rs" "$evidence_dir/rust_registration_validator.bin"
  cp "$validator_dir/src/terminal.rs" "$evidence_dir/rust_terminal_validator.bin"
  cp "$validator_dir/src/generated_direct_program.rs" \
    "$evidence_dir/interpreter_program_include.bin"
  cp "$validator_result" "$evidence_dir/validator_result.bin"
  cp "$rustc_verbose" "$evidence_dir/rustc_verbose.bin"
  cp "$lake_version" "$evidence_dir/lake_version.bin"
  cp "$validator_dir/Cargo.lock" "$evidence_dir/validator_cargo_lock.bin"
  printf 'evidence_dir=%s\n' "$evidence_dir"
fi

printf 'corpus_sha256=%s\n' "$(sha256_file "$corpus")"
printf 'lean_source_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectControllerCodec.lean")"
printf 'lean_direct_program_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectProgram.lean")"
printf 'lean_vm_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/TransitionVM.lean")"
printf 'lean_terminal_controller_abi_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/RegisteredControllerAbi.lean")"
printf 'lean_terminal_physical_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/RegisteredPhysical.lean")"
printf 'lean_direct_lifecycle_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/DClutchSemantics/DirectLifecycle.lean")"
printf 'lean_creation_corpus_emitter_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/EmitRegisteredCreationTranslationCorpus.lean")"
printf 'lean_direct_corpus_emitter_sha256=%s\n' \
  "$(sha256_file "$repository_dir/formal/dclutch-semantics/EmitDirectTranslationCorpus.lean")"
printf 'rust_codec_sha256=%s\n' \
  "$(sha256_file "$repository_dir/crates/dclutch-direct-codec/src/lib.rs")"
printf 'rust_vm_sha256=%s\n' \
  "$(sha256_file "$repository_dir/crates/dclutch-transition-vm/src/lib.rs")"
printf 'rust_direct_aot_sha256=%s\n' \
  "$(sha256_file "$repository_dir/crates/dclutch-direct-aot-contract/src/lib.rs")"
printf 'rust_direct_aot_generated_sha256=%s\n' \
  "$(sha256_file "$repository_dir/crates/dclutch-direct-aot-contract/src/generated.rs")"
printf 'rust_terminal_validator_sha256=%s\n' \
  "$(sha256_file "$validator_dir/src/terminal.rs")"
printf 'rust_registration_validator_sha256=%s\n' \
  "$(sha256_file "$validator_dir/src/registration.rs")"
printf 'rust_validator_sha256=%s\n' \
  "$(sha256_file "$validator_dir/src/main.rs")"
printf 'program_include_sha256=%s\n' \
  "$(sha256_file "$validator_dir/src/generated_direct_program.rs")"
printf 'validator_cargo_lock_sha256=%s\n' "$(sha256_file "$validator_dir/Cargo.lock")"
