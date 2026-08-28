#!/bin/sh
set -eu

tool_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

cargo test --locked --offline --manifest-path "$tool_dir/Cargo.toml"
cargo run --locked --offline --quiet --manifest-path "$tool_dir/Cargo.toml" -- \
  check "$tool_dir/fixtures"

