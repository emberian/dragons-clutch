#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
scratch=$(mktemp -d "${TMPDIR:-/tmp}/dclutch-protocol-parameters-generated.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM

if ! git -C "$repo_root" archive --format=tar HEAD formal/dclutch-semantics \
    | tar -xf - -C "$scratch"; then
  echo "failed to materialize clean formal archive" >&2
  exit 1
fi

formal="$scratch/formal/dclutch-semantics"
build_stdout="$scratch/build.stdout"
build_stderr="$scratch/build.stderr"
emit_stdout="$scratch/generated.rs"
emit_stderr="$scratch/emit.stderr"

if ! (cd "$formal" && lake build DClutchSemantics.ProtocolParametersV1) \
    >"$build_stdout" 2>"$build_stderr"; then
  cat "$build_stdout" >&2
  cat "$build_stderr" >&2
  exit 1
fi
if ! (cd "$formal" && lake env lean --run EmitProtocolParametersV1Rust.lean) \
    >"$emit_stdout" 2>"$emit_stderr"; then
  cat "$build_stdout" >&2
  cat "$build_stderr" >&2
  cat "$emit_stderr" >&2
  exit 1
fi
if ! diff -u "$repo_root/crates/dclutch-protocol-parameters-contract/src/generated.rs" \
    "$emit_stdout"; then
  cat "$build_stdout" >&2
  cat "$build_stderr" >&2
  cat "$emit_stderr" >&2
  exit 1
fi
