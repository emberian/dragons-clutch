#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" != 2 ]]; then
  echo "usage: $0 <spl-token-2022-11.0.0.crate> <SBF output directory>" >&2
  exit 2
fi

fixture_directory="$(cd "$(dirname "$0")" && pwd)"
provenance="$fixture_directory/token-2022-v11.provenance"
token_archive="$1"
sbf_out="$2"
token_source="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-token-2022-v11.XXXXXX")"
token_package="$token_source/source"

manifest_value() {
  local key="$1"
  local count
  count="$(grep -c "^${key}=" "$provenance" || true)"
  if [[ "$count" != 1 ]]; then
    echo "invalid Token-2022 provenance key: $key" >&2
    exit 1
  fi
  sed -n "s/^${key}=//p" "$provenance"
}

cleanup() {
  rm -rf -- "$token_source"
}
trap cleanup EXIT HUP INT TERM

if [[ ! -f "$token_archive" ]]; then
  echo "missing pinned spl-token-2022 crate archive: $token_archive" >&2
  exit 1
fi
mkdir -p -- "$sbf_out"

expected_crate_sha="$(manifest_value crate_sha256)"
actual_crate_sha="$(shasum -a 256 "$token_archive" | awk '{print $1}')"
if [[ "$actual_crate_sha" != "$expected_crate_sha" ]]; then
  echo "unexpected spl-token-2022 11.0.0 crate digest: $actual_crate_sha" >&2
  exit 1
fi

mkdir "$token_package"
tar -xzf "$token_archive" -C "$token_package" --strip-components=1
expected_lock_sha="$(manifest_value cargo_lock_sha256)"
actual_lock_sha="$(shasum -a 256 "$token_package/Cargo.lock" | awk '{print $1}')"
if [[ "$actual_lock_sha" != "$expected_lock_sha" ]]; then
  echo "unexpected spl-token-2022 11.0.0 Cargo.lock digest: $actual_lock_sha" >&2
  exit 1
fi
grep -Fq "$(manifest_value upstream_git_revision)" "$token_package/.cargo_vcs_info.json"
grep -Fq "\"path_in_vcs\":\"$(manifest_value upstream_path)\"" \
  <(tr -d '[:space:]' < "$token_package/.cargo_vcs_info.json")

if [[ -n "${CARGO_BUILD_SBF:-}" ]]; then
  builder=("$CARGO_BUILD_SBF")
else
  builder=(cargo build-sbf)
fi
expected_toolchain="$(printf 'cargo-build-sbf %s\nplatform-tools v%s\nrustc %s' \
  "$(manifest_value cargo_build_sbf_version)" \
  "$(manifest_value platform_tools_version)" \
  "$(manifest_value sbf_rustc_version)")"
actual_toolchain="$("${builder[@]}" --version)"
if [[ "$actual_toolchain" != "$expected_toolchain" ]]; then
  echo "unexpected SBF toolchain:" >&2
  echo "$actual_toolchain" >&2
  exit 1
fi

builder_crate="${CARGO_BUILD_SBF_CRATE:-}"
if [[ -z "$builder_crate" ]]; then
  builder_crate="$(find "${CARGO_HOME:-$HOME/.cargo}/registry/cache" \
    -name "cargo-build-sbf-$(manifest_value cargo_build_sbf_version).crate" \
    -print -quit 2>/dev/null || true)"
fi
if [[ ! -f "$builder_crate" ]]; then
  echo "missing cargo-build-sbf crate archive needed for provenance authentication" >&2
  exit 1
fi
actual_builder_crate_sha="$(shasum -a 256 "$builder_crate" | awk '{print $1}')"
if [[ "$actual_builder_crate_sha" != "$(manifest_value cargo_build_sbf_crate_sha256)" ]]; then
  echo "unexpected cargo-build-sbf crate digest: $actual_builder_crate_sha" >&2
  exit 1
fi

platform_manifest="${SBF_PLATFORM_TOOLS_VERSION_MANIFEST:-$HOME/.cache/solana/v$(manifest_value platform_tools_version)/platform-tools/version.md}"
if [[ ! -f "$platform_manifest" ]]; then
  echo "missing platform-tools version manifest: $platform_manifest" >&2
  exit 1
fi
actual_platform_manifest_sha="$(shasum -a 256 "$platform_manifest" | awk '{print $1}')"
if [[ "$actual_platform_manifest_sha" != "$(manifest_value platform_tools_manifest_sha256)" ]]; then
  echo "unexpected platform-tools version manifest digest: $actual_platform_manifest_sha" >&2
  exit 1
fi

if [[ -n "${TOKEN_2022_V11_ELF:-}" ]]; then
  cp -- "$TOKEN_2022_V11_ELF" "$sbf_out/spl_token_2022.so"
else
  actual_build_host="$(uname -s)-$(uname -m)"
  expected_build_host="$(manifest_value canonical_build_host)"
  if [[ "$actual_build_host" != "$expected_build_host" ]]; then
    echo "Token-2022 fixture requires canonical host $expected_build_host; got $actual_build_host" >&2
    echo "provide a locally available canonical artifact with TOKEN_2022_V11_ELF" >&2
    exit 1
  fi
  env \
    -u RUSTFLAGS \
    -u CARGO_ENCODED_RUSTFLAGS \
    -u CARGO_BUILD_RUSTFLAGS \
    -u RUSTC_WRAPPER \
    -u RUSTC_WORKSPACE_WRAPPER \
    -u CARGO_PROFILE_RELEASE_CODEGEN_UNITS \
    -u CARGO_PROFILE_RELEASE_DEBUG \
    -u CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS \
    -u CARGO_PROFILE_RELEASE_INCREMENTAL \
    -u CARGO_PROFILE_RELEASE_LTO \
    -u CARGO_PROFILE_RELEASE_OPT_LEVEL \
    -u CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS \
    "${builder[@]}" \
      --manifest-path "$token_package/Cargo.toml" \
      --sbf-out-dir "$sbf_out" \
      -- --locked
fi

expected_token_elf_sha="$(manifest_value canonical_elf_sha256)"
actual_token_elf_sha="$(shasum -a 256 "$sbf_out/spl_token_2022.so" | awk '{print $1}')"
if [[ "$actual_token_elf_sha" != "$expected_token_elf_sha" ]]; then
  echo "unexpected spl-token-2022 ELF digest: $actual_token_elf_sha" >&2
  exit 1
fi
actual_token_elf_bytes="$(wc -c < "$sbf_out/spl_token_2022.so" | tr -d '[:space:]')"
if [[ "$actual_token_elf_bytes" != "$(manifest_value canonical_elf_bytes)" ]]; then
  echo "unexpected spl-token-2022 ELF byte length: $actual_token_elf_bytes" >&2
  exit 1
fi
