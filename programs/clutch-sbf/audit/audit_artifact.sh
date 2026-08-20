#!/usr/bin/env bash
# Offline, fail-closed supply-chain and final-artifact audit for clutch-sbf.
#
# This is deliberately independent of scripts/run_bringup.sh.  It does not
# start a validator or exercise protocol semantics.  Instead it rebuilds the
# SBF ELF twice into fresh target directories from a checksum-verified Cargo
# cache, then audits the linked artifact, backend stack diagnostics, dependency
# pins, licenses, and loader-v3 account sizing.  A third build relocates the
# Cargo home to measure (rather than hide) the path-sensitive boundary.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
sbf_root="$(cd "$here/.." && pwd)"
repo="$(cd "$sbf_root/../.." && pwd)"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

# Build-affecting overrides make a result hard to interpret.  One explicit
# distribution root is supported; mixed per-binary overrides are not.
ambiguous_env=(
  CARGO_BUILD_SBF SOLANA_BIN LLVM_OBJDUMP LLVM_READOBJ CARGO_HOME RUSTC RUSTDOC RUSTFLAGS
  CARGO_ENCODED_RUSTFLAGS CARGO_TARGET_DIR SBF_OUT_PATH SBF_SDK_PATH
  CARGO_PROFILE_RELEASE_LTO CARGO_PROFILE_RELEASE_CODEGEN_UNITS
)
for variable in "${ambiguous_env[@]}"; do
  if [ -n "${!variable-}" ]; then
    fail "$variable is set; unset build overrides or select one CLUTCH_SBF_TOOLCHAIN_ROOT"
  fi
done

toolchain_root="${CLUTCH_SBF_TOOLCHAIN_ROOT:-$HOME/.local/share/solana/install/active_release}"
[ -d "$toolchain_root" ] || fail "toolchain root does not exist: $toolchain_root"
toolchain_root="$(realpath "$toolchain_root")"
build_sbf="$toolchain_root/bin/cargo-build-sbf"
solana="$toolchain_root/bin/solana"
version_file="$toolchain_root/version.yml"
for required in "$build_sbf" "$solana"; do
  [ -x "$required" ] || fail "required tool is not executable: $required"
done
[ -f "$version_file" ] || fail "release version pin is absent: $version_file"

build_sbf_version="$($build_sbf --version)"
build_line_count="$(printf '%s\n' "$build_sbf_version" | sed -n '/^cargo-build-sbf /p' | wc -l | tr -d ' ')"
platform_line_count="$(printf '%s\n' "$build_sbf_version" | sed -n '/^platform-tools v/p' | wc -l | tr -d ' ')"
rustc_line_count="$(printf '%s\n' "$build_sbf_version" | sed -n '/^rustc /p' | wc -l | tr -d ' ')"
[ "$build_line_count" = 1 ] || fail "cargo-build-sbf version output does not select exactly one builder"
[ "$platform_line_count" = 1 ] || fail "cargo-build-sbf version output does not select exactly one platform-tools release"
[ "$rustc_line_count" = 1 ] || fail "cargo-build-sbf version output does not select exactly one Rust release"

platform_tools_version="$(printf '%s\n' "$build_sbf_version" | awk '$1 == "platform-tools" { print $2 }')"
advertised_rustc="$(printf '%s\n' "$build_sbf_version" | awk '$1 == "rustc" { print $2 }')"
platform_root="$HOME/.cache/solana/$platform_tools_version/platform-tools"
platform_rustc="$platform_root/rust/bin/rustc"
platform_cargo="$platform_root/rust/bin/cargo"
llvm_objdump="$platform_root/llvm/bin/llvm-objdump"
llvm_readobj="$platform_root/llvm/bin/llvm-readobj"
llvm_objcopy="$platform_root/llvm/bin/llvm-objcopy"
llvm_lld="$platform_root/llvm/bin/lld"
for required in "$platform_rustc" "$platform_cargo" "$llvm_objdump" "$llvm_readobj" "$llvm_objcopy" "$llvm_lld"; do
  [ -x "$required" ] || fail "selected platform-tools tree is incomplete: $required"
done
actual_rustc="$($platform_rustc --version | awk '{print $2}' | sed 's/-dev$//')"
[ "$actual_rustc" = "$advertised_rustc" ] || fail "advertised rustc $advertised_rustc != selected binary $actual_rustc"

release_commit="$(awk '$1 == "commit:" { print $2 }' "$version_file")"
[ -n "$release_commit" ] || fail "release commit is absent from $version_file"
solana_version="$($solana --version)"
solana_src="$(printf '%s\n' "$solana_version" | sed -nE 's/.*\(src:([0-9a-f]+);.*/\1/p')"
[ -n "$solana_src" ] || fail "Solana CLI version does not expose a source commit: $solana_version"
case "$release_commit" in
  "$solana_src"*) ;;
  *) fail "Solana CLI source $solana_src does not match release commit $release_commit" ;;
esac

cd "$repo"
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || fail "repository is not a Git checkout"
source_commit="$(git rev-parse HEAD)"

# Conservative build-input closure.  The harness manifest is included because
# Cargo reads every workspace-member manifest while resolving the locked
# workspace, although harness/src is not linked into the program ELF.
# research/batch-policy-identity is the one first-party path dependency that
# lives outside programs/ and crates/ (the sol_sha256 policy-identity folds);
# earlier seals documented it as a closure gap and compensated with a clean
# detached worktree.  It is declared here so the dirty-tree gate and the
# source-closure digest cover every first-party package in the linked graph
# (crates/clutch-batch was already covered by the whole-directory `crates`
# entry).
source_paths=(
  programs/clutch-sbf/.cargo
  programs/clutch-sbf/Cargo.toml
  programs/clutch-sbf/Cargo.lock
  programs/clutch-sbf/program
  programs/clutch-sbf/harness/Cargo.toml
  programs/clutch-sbf/vendor
  programs/solana-layout
  programs/solana-reference
  crates
  research/batch-policy-identity
)
source_dirty="$(git status --porcelain -- "${source_paths[@]}")"
if [ -n "$source_dirty" ]; then
  printf 'FAIL: declared ELF input closure is dirty:\n%s\n' "$source_dirty" >&2
  exit 1
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/clutch-sbf-artifact-audit.XXXXXX")"
cleanup() {
  if [ "${CLUTCH_SBF_AUDIT_KEEP:-0}" = 1 ]; then
    echo "audit_work=$work"
  else
    rm -rf "$work"
  fi
}
trap cleanup EXIT

git ls-files -- "${source_paths[@]}" | LC_ALL=C sort > "$work/source-files.txt"
[ -s "$work/source-files.txt" ] || fail "declared source closure is empty"
source_digest="$(python3 - "$repo" "$work/source-files.txt" <<'PY'
import hashlib
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
files = pathlib.Path(sys.argv[2]).read_text().splitlines()
digest = hashlib.sha256()
for relative in files:
    path = root / relative
    if not path.is_file():
        raise SystemExit(f"tracked build input is not a regular file: {relative}")
    data = path.read_bytes()
    digest.update(relative.encode())
    digest.update(b"\0")
    digest.update(hashlib.sha256(data).digest())
    digest.update(b"\0")
print(digest.hexdigest())
PY
)"

# Audit resolution and licenses before compiling.  A registry package without
# a checksum or a declared license is a hard failure.  Git sources and unknown
# registries are forbidden in this offline artifact lane.
metadata_home="$work/cargo-home-metadata"
mkdir -p "$metadata_home/registry"
[ -d "$HOME/.cargo/registry/index" ] || fail "Cargo registry index cache is absent"
[ -d "$HOME/.cargo/registry/cache" ] || fail "Cargo registry archive cache is absent"
ln -s "$HOME/.cargo/registry/index" "$metadata_home/registry/index"
ln -s "$HOME/.cargo/registry/cache" "$metadata_home/registry/cache"
CARGO_HOME="$metadata_home" CARGO_NET_OFFLINE=true \
  "$platform_cargo" metadata --manifest-path "$sbf_root/program/Cargo.toml" \
    --offline --locked --format-version 1 > "$work/metadata.json"

python3 - "$sbf_root/Cargo.lock" "$work/metadata.json" \
  "$HOME/.cargo/registry/cache" "$repo" "$work/dependencies.tsv" <<'PY'
import glob
import hashlib
import json
import pathlib
import sys
import tomllib

lock_path, metadata_path, cache_root, repo_root, report_path = sys.argv[1:]
lock = tomllib.loads(pathlib.Path(lock_path).read_text())
metadata = json.loads(pathlib.Path(metadata_path).read_text())
repo = pathlib.Path(repo_root).resolve()
lock_by_key = {}
for package in lock["package"]:
    key = (package["name"], package["version"], package.get("source"))
    if key in lock_by_key:
        raise SystemExit(f"duplicate lock identity: {key}")
    lock_by_key[key] = package

rows = []
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"], p.get("source") or "")):
    source = package.get("source")
    license_id = package.get("license")
    if not license_id:
        raise SystemExit(f"package has no declared license: {package['name']} {package['version']}")
    if source is None:
        manifest = pathlib.Path(package["manifest_path"]).resolve()
        try:
            relative = manifest.relative_to(repo)
        except ValueError as error:
            raise SystemExit(f"path dependency escapes repository: {manifest}") from error
        kind = "vendored" if str(relative).startswith("programs/clutch-sbf/vendor/") else "first-party"
        if kind == "first-party" and license_id != "AGPL-3.0-or-later":
            raise SystemExit(f"unexpected first-party license {license_id}: {relative}")
        rows.append((package["name"], package["version"], kind, "-", license_id))
        continue

    if source != "registry+https://github.com/rust-lang/crates.io-index":
        raise SystemExit(f"forbidden or ambiguous external source: {package['name']} {source}")
    locked = lock_by_key.get((package["name"], package["version"], source))
    if locked is None or not locked.get("checksum"):
        raise SystemExit(f"registry package lacks locked checksum: {package['name']} {package['version']}")
    archives = glob.glob(str(pathlib.Path(cache_root) / "*" / f"{package['name']}-{package['version']}.crate"))
    if len(archives) != 1:
        raise SystemExit(
            f"expected exactly one cached archive for {package['name']} {package['version']}; got {len(archives)}"
        )
    actual = hashlib.sha256(pathlib.Path(archives[0]).read_bytes()).hexdigest()
    if actual != locked["checksum"]:
        raise SystemExit(f"archive checksum mismatch: {package['name']} {package['version']}")
    rows.append((package["name"], package["version"], "crates.io", actual, license_id))

pathlib.Path(report_path).write_text(
    "name\tversion\tsource\tsha256\tlicense\n"
    + "".join("\t".join(row) + "\n" for row in rows)
)
PY

# Cargo verifies an archive checksum when unpacking, but an already-unpacked
# registry source is subsequently trusted.  The two runtime-recipe builds below
# intentionally use the ordinary fixed Cargo home so their digest matches the
# bring-up recipe.  Re-derive and compare every unpacked file first.
python3 - "$sbf_root/Cargo.lock" "$HOME/.cargo/registry/cache" \
  "$HOME/.cargo/registry/src" "$work/registry-source-verification.tsv" <<'PY'
import glob
import hashlib
import io
import pathlib
import sys
import tarfile
import tomllib

lock_path, cache_root, source_root, report_path = sys.argv[1:]
lock = tomllib.loads(pathlib.Path(lock_path).read_text())
rows = []
for package in lock["package"]:
    source = package.get("source", "")
    if not source.startswith("registry+"):
        continue
    name = package["name"]
    version = package["version"]
    archives = glob.glob(str(pathlib.Path(cache_root) / "*" / f"{name}-{version}.crate"))
    trees = glob.glob(str(pathlib.Path(source_root) / "*" / f"{name}-{version}"))
    if len(archives) != 1 or len(trees) != 1:
        raise SystemExit(
            f"ambiguous registry cache for {name} {version}: archives={len(archives)} trees={len(trees)}"
        )
    archive = pathlib.Path(archives[0])
    tree = pathlib.Path(trees[0])
    actual_archive = hashlib.sha256(archive.read_bytes()).hexdigest()
    if actual_archive != package.get("checksum"):
        raise SystemExit(f"archive checksum mismatch: {name} {version}")

    expected = {}
    prefix = f"{name}-{version}/"
    with tarfile.open(archive, "r:gz") as bundle:
        for member in bundle.getmembers():
            if not member.isfile():
                continue
            if not member.name.startswith(prefix):
                raise SystemExit(f"archive member escapes package root: {member.name}")
            relative = member.name[len(prefix):]
            extracted = bundle.extractfile(member)
            if extracted is None:
                raise SystemExit(f"cannot extract archive member: {member.name}")
            expected[relative] = hashlib.sha256(extracted.read()).hexdigest()
    actual = {
        path.relative_to(tree).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in tree.rglob("*")
        if path.is_file() and path.name != ".cargo-ok"
    }
    if expected != actual:
        missing = sorted(expected.keys() - actual.keys())
        extra = sorted(actual.keys() - expected.keys())
        changed = sorted(key for key in expected.keys() & actual.keys() if expected[key] != actual[key])
        raise SystemExit(
            f"unpacked registry source mismatch for {name} {version}: "
            f"missing={missing[:3]} extra={extra[:3]} changed={changed[:3]}"
        )
    rows.append((name, version, actual_archive, str(tree)))

pathlib.Path(report_path).write_text(
    "name\tversion\tarchive_sha256\tunpacked_source\n"
    + "".join("\t".join(row) + "\n" for row in sorted(rows))
)
PY

# The fixed Cargo home configuration is an input to the bring-up recipe.  It
# may tune unrelated host targets, but may not redirect a registry/source or
# inject Rust flags into this artifact without an explicit review.
global_cargo_config="$HOME/.cargo/config.toml"
[ -f "$global_cargo_config" ] || fail "fixed Cargo home config is absent: $global_cargo_config"
python3 - "$global_cargo_config" <<'PY'
import pathlib
import sys
import tomllib

config = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())
for forbidden in ("source", "registries"):
    if config.get(forbidden):
        raise SystemExit(f"global Cargo config has active [{forbidden}] selection")
if config.get("patch"):
    nonempty = {key: value for key, value in config["patch"].items() if value}
    if nonempty:
        raise SystemExit(f"global Cargo config has active patch selection: {sorted(nonempty)}")
if config.get("build", {}).get("rustflags"):
    raise SystemExit("global Cargo config injects build.rustflags")
for target, values in config.get("target", {}).items():
    if values.get("rustflags"):
        raise SystemExit(f"global Cargo config injects target.{target}.rustflags")
PY
global_cargo_config_sha256="$(sha256 "$global_cargo_config")"

# The one vendored crate has no local .crate archive.  Require its checked-in
# provenance declaration and byte identity with Cargo's previously unpacked
# cache, while recording the limitation in the evidence document.
vendor="$sbf_root/vendor/solana-define-syscall-5.1.0"
vendor_cache_matches=("$HOME"/.cargo/registry/src/*/solana-define-syscall-5.1.0)
[ "${#vendor_cache_matches[@]}" = 1 ] || fail "expected exactly one unpacked upstream source for vendored solana-define-syscall 5.1.0"
grep -Fq '21e14a4f604117f379840956a8fc8695e4c84f5b0ebed192f31f60d9b85d581d' \
  "$sbf_root/vendor/PROVENANCE.md" || fail "vendored upstream archive checksum declaration is absent"
diff -qr -x .cargo-ok "$vendor" "${vendor_cache_matches[0]}" > "$work/vendor.diff" \
  || fail "vendored solana-define-syscall differs from the unpacked cache; see $work/vendor.diff"
vendor_digest="$(python3 - "$vendor" <<'PY'
import hashlib
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
digest = hashlib.sha256()
for path in sorted(p for p in root.rglob("*") if p.is_file() and p.name != ".cargo-ok"):
    relative = path.relative_to(root).as_posix()
    digest.update(relative.encode())
    digest.update(b"\0")
    digest.update(hashlib.sha256(path.read_bytes()).digest())
    digest.update(b"\0")
print(digest.hexdigest())
PY
)"

# Assert the deterministic profile instead of relying on the builder default.
python3 - "$sbf_root/Cargo.toml" <<'PY'
import pathlib
import sys
import tomllib

manifest = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())
release = manifest.get("profile", {}).get("release", {})
expected = {"lto": "fat", "codegen-units": 1, "overflow-checks": True}
for key, value in expected.items():
    if release.get(key) != value:
        raise SystemExit(f"release profile {key}={release.get(key)!r}; expected {value!r}")
PY

hashes=()
sizes=()
unstripped_elves=()
for pass in 1 2; do
  target="$work/target-$pass"
  out="$work/out-$pass"
  log="$work/sbf-build-$pass.log"
  mkdir -p "$out"

  (
    cd "$sbf_root"
    CARGO_HOME="$HOME/.cargo" CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$target" \
      "$build_sbf" --manifest-path program/Cargo.toml --arch v0 --offline \
        --skip-tools-install --tools-version "$platform_tools_version" \
        --sbf-out-dir "$out" -- --locked
  ) > "$log" 2>&1 || {
    tail -80 "$log" >&2
    fail "SBF build pass $pass failed"
  }

  elf="$out/clutch_sbf.so"
  [ -f "$elf" ] || fail "build pass $pass did not emit clutch_sbf.so"
  hashes+=("$(sha256 "$elf")")
  sizes+=("$(wc -c < "$elf" | tr -d ' ')")
  unstripped="$(find "$target" -type f -path '*/release/deps/clutch_sbf.so' -print)"
  unstripped_count="$(printf '%s\n' "$unstripped" | sed '/^$/d' | wc -l | tr -d ' ')"
  [ "$unstripped_count" = 1 ] || fail "expected exactly one unstripped final ELF in pass $pass; got $unstripped_count"
  unstripped_elves+=("$unstripped")
done
[ "${hashes[0]}" = "${hashes[1]}" ] || fail "fresh SBF builds differ: ${hashes[0]} != ${hashes[1]}"
[ "${sizes[0]}" = "${sizes[1]}" ] || fail "fresh SBF sizes differ"

# Relocation probe.  It is deliberately not the attested artifact: it measures
# whether dependency source paths embedded by rustc make the recipe sensitive
# to Cargo-home location.  The result narrows the reproducibility claim.
relocated_home="$work/cargo-home-relocated"
relocated_target="$work/target-relocated"
relocated_out="$work/out-relocated"
mkdir -p "$relocated_home/registry" "$relocated_out"
ln -s "$HOME/.cargo/registry/index" "$relocated_home/registry/index"
ln -s "$HOME/.cargo/registry/cache" "$relocated_home/registry/cache"
(
  cd "$sbf_root"
  CARGO_HOME="$relocated_home" CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$relocated_target" \
    "$build_sbf" --manifest-path program/Cargo.toml --arch v0 --offline \
      --skip-tools-install --tools-version "$platform_tools_version" \
      --sbf-out-dir "$relocated_out" -- --locked
) > "$work/sbf-build-relocated.log" 2>&1 || {
  tail -80 "$work/sbf-build-relocated.log" >&2
  fail "relocated-Cargo-home SBF probe failed"
}
relocated_hash="$(sha256 "$relocated_out/clutch_sbf.so")"
if [ "$relocated_hash" = "${hashes[0]}" ]; then
  cargo_home_relocation="INDEPENDENT"
else
  cargo_home_relocation="PATH_SENSITIVE"
fi

# Ensure no concurrent edit changed the source closure during the two builds.
source_digest_after="$(python3 - "$repo" "$work/source-files.txt" <<'PY'
import hashlib
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
digest = hashlib.sha256()
for relative in pathlib.Path(sys.argv[2]).read_text().splitlines():
    digest.update(relative.encode())
    digest.update(b"\0")
    digest.update(hashlib.sha256((root / relative).read_bytes()).digest())
    digest.update(b"\0")
print(digest.hexdigest())
PY
)"
[ "$source_digest" = "$source_digest_after" ] || fail "declared source closure changed during the audit"

elf="$work/out-1/clutch_sbf.so"
unstripped="${unstripped_elves[0]}"
"$llvm_objdump" --syms "$unstripped" > "$work/final-symbols.txt"
"$llvm_objdump" --disassemble --no-show-raw-insn "$unstripped" > "$work/final-disassembly.txt"
"$llvm_readobj" --file-headers --sections --program-headers --dynamic-table --dyn-symbols \
  "$elf" > "$work/final-readobj.txt"

grep -E '^Error: (Function|A function call)' "$work/sbf-build-1.log" \
  | LC_ALL=C sort -u > "$work/backend-stack-diagnostics.txt" || true
python3 - "$work/backend-stack-diagnostics.txt" "$work/final-symbols.txt" \
  "$work/stack-summary.txt" <<'PY'
import pathlib
import re
import sys

diagnostics_path, symbols_path, summary_path = map(pathlib.Path, sys.argv[1:])
diagnostics = diagnostics_path.read_text().splitlines()
patterns = [
    re.compile(r"^Error: Function ([^ ]+) "),
    re.compile(r"^Error: A function call in method ([^ ]+) "),
]
diagnosed = set()
for line in diagnostics:
    for pattern in patterns:
        match = pattern.match(line)
        if match:
            diagnosed.add(match.group(1))
            break
    else:
        raise SystemExit(f"unparsed backend stack diagnostic: {line}")

symbol_text = symbols_path.read_text()
survivors = sorted(symbol for symbol in diagnosed if symbol in symbol_text)
if survivors:
    raise SystemExit("backend stack-diagnostic symbols survived final LTO: " + ", ".join(survivors))
summary_path.write_text(
    f"backend_stack_diagnostic_lines={len(diagnostics)}\n"
    f"backend_stack_diagnostic_symbols={len(diagnosed)}\n"
    "backend_stack_diagnostic_survivors=0\n"
)
PY

# Independent final-artifact check: enumerate every function resident after
# LTO and reject direct r10 frame references outside the 4096-byte SBF frame.
# This complements (but does not replace or overstate) the backend diagnostics.
python3 - "$work/final-symbols.txt" "$work/final-disassembly.txt" \
  "$work/frame-summary.txt" <<'PY'
import pathlib
import re
import sys

symbols_path, disassembly_path, summary_path = map(pathlib.Path, sys.argv[1:])
function_symbols = set()
function_addresses = set()
for line in symbols_path.read_text().splitlines():
    fields = line.split()
    if len(fields) >= 6 and fields[2] == "F" and fields[3] == ".text":
        function_symbols.add(fields[-1])
        function_addresses.add(int(fields[0], 16))
if not function_symbols:
    raise SystemExit("final ELF symbol table exposes no text functions")

current = None
seen_functions = set()
seen_addresses = set()
max_offset = 0
max_function = None
references = 0
header = re.compile(r"^([0-9a-f]+) <(.+)>:$")
reference = re.compile(r"\[r10\s*([+-])\s*0x([0-9a-f]+)\]")
for line in disassembly_path.read_text().splitlines():
    match = header.match(line.strip())
    if match:
        seen_addresses.add(int(match.group(1), 16))
        current = match.group(2)
        seen_functions.add(current)
        continue
    for match in reference.finditer(line):
        references += 1
        sign, encoded = match.groups()
        offset = int(encoded, 16)
        if current is None:
            raise SystemExit(f"stack reference outside a named function: {line}")
        if sign != "-" or not 1 <= offset <= 4096:
            raise SystemExit(f"out-of-frame direct r10 reference in {current}: {line.strip()}")
        if offset > max_offset:
            max_offset = offset
            max_function = current

missing_addresses = function_addresses - seen_addresses
if missing_addresses:
    raise SystemExit(
        f"{len(missing_addresses)} text-function addresses were not disassembled; "
        f"first: 0x{min(missing_addresses):x}"
    )
summary_path.write_text(
    f"final_text_function_symbols={len(function_symbols)}\n"
    f"final_text_function_addresses={len(function_addresses)}\n"
    f"disassembled_function_regions={len(seen_addresses)}\n"
    f"direct_r10_references={references}\n"
    f"deepest_direct_r10_offset={max_offset}\n"
    f"deepest_direct_r10_function={max_function}\n"
    "direct_frame_bounds=PASS\n"
)
PY

# ELF shape and syscall surface.  A new undefined dynamic symbol is a review
# event, not something this supply-chain gate silently accepts.
#
# `sol_sha256` is the first reviewed addition to the 2026-08-18 baseline.  The
# program calls it only through `solana-sha256-hasher =3.1.0`, whose SBF branch
# re-exports `solana_define_syscall::definitions::sol_sha256` and whose safe
# `hashv` wrapper owns the unsafe result buffer.  Pin the exact crates.io
# archive identity here as well as in Cargo.lock/dependency verification so an
# unrelated package cannot inherit this syscall admission merely by name.
#
# `sol_memmove_` is the second reviewed addition (2026-08-20, the T2-8 wave).
# It is not called by any first-party source: LLVM lowers the potentially
# overlapping in-place copies of the portfolio full-pair seam
# (`clutch_solana_layout::portfolio_settlement::prepare_full_pair` and
# `orders_batch::entitlement::settle_portfolio_pair` are the only two callers
# in the final disassembly) to the `memmove` intrinsic, and the resident
# 4-instruction hidden `memmove` shim is the platform-tools Rust fork's
# `compiler-builtins` wrapper over the syscall
# (rustlib/src/rust/library/compiler-builtins/compiler-builtins/src/mem/mod.rs)
# — the exact provenance of the long-admitted `memcpy`/`memset`/`memcmp`
# shims, so the trust class of the surface is unchanged and the pin that
# covers the shim is the platform-tools release identity this script already
# verifies, not a crates.io archive.
python3 - "$work/final-readobj.txt" "$work/dependencies.tsv" \
  "$work/elf-summary.txt" <<'PY'
import pathlib
import re
import sys

text = pathlib.Path(sys.argv[1]).read_text()
dependency_rows = pathlib.Path(sys.argv[2]).read_text().splitlines()
expected_sha256_wrapper = (
    "solana-sha256-hasher\t3.1.0\tcrates.io\t"
    "db7dc3011ea4c0334aaaa7e7128cb390ecf546b28d412e9bf2064680f57f588f\t"
    "Apache-2.0"
)
if dependency_rows.count(expected_sha256_wrapper) != 1:
    raise SystemExit(
        "reviewed sol_sha256 wrapper provenance changed; expected exactly: "
        + expected_sha256_wrapper
    )
required = [
    "Format: elf64-sbf",
    "Arch: sbf",
    "Type: SharedObject",
    "Machine: EM_SBF",
]
for marker in required:
    if marker not in text:
        raise SystemExit(f"final artifact missing ELF marker: {marker}")
if "Flags [ (0x7)" in text:
    raise SystemExit("final artifact contains a writable-executable program segment")

entry_match = re.search(r"\n  Entry: (0x[0-9A-Fa-f]+)", text)
entry_symbol = re.search(
    r"Name: entrypoint .*?Value: (0x[0-9A-Fa-f]+).*?Section: \.text", text, re.S
)
if not entry_match or not entry_symbol or int(entry_match.group(1), 16) != int(entry_symbol.group(1), 16):
    raise SystemExit("ELF header entry does not equal the defined entrypoint symbol")

undefined = set()
for block in re.findall(r"  Symbol \{(.*?)\n  \}", text, re.S):
    if "Section: Undefined" not in block:
        continue
    name = re.search(r"\n    Name: ([^ ]*)", block)
    if name and name.group(1):
        undefined.add(name.group(1))
baseline_allowed = {
    "abort",
    "sol_invoke_signed_rust",
    "sol_log_",
    "sol_memcmp_",
    "sol_memcpy_",
    "sol_memset_",
    "sol_panic_",
    "sol_try_find_program_address",
}
reviewed_additions = {"sol_sha256", "sol_memmove_"}
allowed = baseline_allowed | reviewed_additions
if allowed - baseline_allowed != {"sol_sha256", "sol_memmove_"}:
    raise SystemExit(
        "syscall review widened beyond the intended sol_sha256 + sol_memmove_ additions"
    )

def require_exact_surface(observed):
    if observed != allowed:
        raise ValueError(
            f"undefined dynamic-symbol surface changed: got {sorted(observed)}, "
            f"expected {sorted(allowed)}"
        )

# Hostile self-check: the exact-surface predicate used on the ELF must still
# reject a second hash syscall.  This prevents a future edit from accidentally
# turning the reviewed one-symbol addition into a prefix or family admission.
hostile = allowed | {"sol_keccak256"}
try:
    require_exact_surface(hostile)
except ValueError as error:
    if "sol_keccak256" not in str(error):
        raise SystemExit("hostile syscall-surface check failed opaquely") from error
else:
    raise SystemExit("hostile syscall-surface check admitted sol_keccak256")

try:
    require_exact_surface(undefined)
except ValueError as error:
    raise SystemExit(str(error)) from error

text_size_match = re.search(r"Name: \.text .*?Size: ([0-9]+)", text, re.S)
load_segments = len(re.findall(r"Type: PT_LOAD", text))
if not text_size_match or load_segments != 3:
    raise SystemExit("unexpected final ELF section or load-segment shape")
pathlib.Path(sys.argv[3]).write_text(
    f"entrypoint={entry_match.group(1)}\n"
    f"text_bytes={text_size_match.group(1)}\n"
    f"load_segments={load_segments}\n"
    f"undefined_dynamic_symbols={','.join(sorted(undefined))}\n"
    "elf_shape=PASS\n"
)
PY

# Loader-v3 wire sizes: Program=36, Buffer metadata=37, ProgramData metadata=45.
# MAX_PERMITTED_DATA_LENGTH is 10 MiB.  This is size arithmetic only; it is not
# a deployment, rent quote, or proof of acceptance by any public cluster.
elf_bytes="${sizes[0]}"
program_account_bytes=36
buffer_account_bytes=$((37 + elf_bytes))
programdata_account_bytes=$((45 + elf_bytes))
max_permitted_data_bytes=$((10 * 1024 * 1024))
[ "$programdata_account_bytes" -le "$max_permitted_data_bytes" ] \
  || fail "loader-v3 ProgramData size $programdata_account_bytes exceeds $max_permitted_data_bytes"
loader_headroom_bytes=$((max_permitted_data_bytes - programdata_account_bytes))

dependency_rows="$(($(wc -l < "$work/dependencies.tsv") - 1))"
registry_rows="$(awk -F '\t' 'NR > 1 && $3 == "crates.io" { n += 1 } END { print n + 0 }' "$work/dependencies.tsv")"
first_party_rows="$(awk -F '\t' 'NR > 1 && $3 == "first-party" { n += 1 } END { print n + 0 }' "$work/dependencies.tsv")"
vendored_rows="$(awk -F '\t' 'NR > 1 && $3 == "vendored" { n += 1 } END { print n + 0 }' "$work/dependencies.tsv")"

echo "source_commit=$source_commit"
echo "source_scope_sha256=$source_digest"
echo "source_scope_files=$(wc -l < "$work/source-files.txt" | tr -d ' ')"
echo "toolchain_release_commit=$release_commit"
echo "solana_version=$solana_version"
printf '%s\n' "$build_sbf_version" | sed 's/^/toolchain: /'
echo "cargo_build_sbf_sha256=$(sha256 "$build_sbf")"
echo "solana_cli_sha256=$(sha256 "$solana")"
echo "platform_rustc_sha256=$(sha256 "$platform_rustc")"
echo "platform_cargo_sha256=$(sha256 "$platform_cargo")"
echo "llvm_objdump_sha256=$(sha256 "$llvm_objdump")"
echo "llvm_readobj_sha256=$(sha256 "$llvm_readobj")"
echo "llvm_objcopy_sha256=$(sha256 "$llvm_objcopy")"
echo "llvm_lld_sha256=$(sha256 "$llvm_lld")"
echo "global_cargo_config_sha256=$global_cargo_config_sha256"
echo "dependency_packages=$dependency_rows"
echo "dependency_registry_archives_verified=$registry_rows"
echo "dependency_first_party=$first_party_rows"
echo "dependency_vendored=$vendored_rows"
echo "vendored_tree_sha256=$vendor_digest"
echo "elf_pass_1_sha256=${hashes[0]}"
echo "elf_pass_2_sha256=${hashes[1]}"
echo "elf_relocated_cargo_home_sha256=$relocated_hash"
echo "cargo_home_relocation=$cargo_home_relocation"
echo "elf_bytes=$elf_bytes"
cat "$work/stack-summary.txt"
cat "$work/frame-summary.txt"
cat "$work/elf-summary.txt"
echo "loader_v3_program_account_bytes=$program_account_bytes"
echo "loader_v3_buffer_account_bytes=$buffer_account_bytes"
echo "loader_v3_programdata_account_bytes=$programdata_account_bytes"
echo "loader_v3_max_permitted_data_bytes=$max_permitted_data_bytes"
echo "loader_v3_headroom_bytes=$loader_headroom_bytes"
echo "artifact_audit=PASS"
