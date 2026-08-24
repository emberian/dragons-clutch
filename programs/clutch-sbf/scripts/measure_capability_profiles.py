#!/usr/bin/env python3
"""Build identity-manifested capability profiles and emit schema-V2 evidence.

No profile inventory lives in this producer. A build requires an explicit,
fully linked capability-profile manifest containing the central registry
coverage and all semantic-owner identities. In their absence the producer
emits an unavailable record and refuses instead of fabricating a live row.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from typing import Any

import check_capability_profile as checker


RENT_EXEMPT_LAMPORTS_PER_BILLABLE_BYTE = 6_960
ACCOUNT_STORAGE_OVERHEAD_BYTES = 128
PROGRAM_DATA_LEN_BYTES = 36
PROGRAMDATA_METADATA_DATA_LEN_BYTES = 45
BUFFER_METADATA_DATA_LEN_BYTES = 37
MAX_PERMITTED_DATA_LENGTH = 10 * 1024 * 1024
PLATFORM_TOOLS_RE = re.compile(r"^platform-tools (v[^ ]+)$", re.MULTILINE)
LEGACY_RUST_SYMBOL_HASH = re.compile(r"17h[0-9a-f]{16}E\Z")

# Conservative first-party ELF input closure. It intentionally covers every
# crate because a newly added path dependency must not evade the dirty-input
# gate before this list is updated. The identity manifests supplied on the CLI
# are appended as exact tracked paths.
SOURCE_CLOSURE = (
    "crates",
    "programs/clutch-sbf/.cargo",
    "programs/clutch-sbf/Cargo.toml",
    "programs/clutch-sbf/Cargo.lock",
    "programs/clutch-sbf/program",
    "programs/clutch-sbf/harness/Cargo.toml",
    "programs/clutch-sbf/scripts",
    "programs/clutch-sbf/vendor",
    "programs/solana-layout",
    "programs/solana-reference",
    "research/batch-policy-identity",
    *(path for _role, path in checker.LINKED_MEASUREMENT_CODE_INPUTS),
)


class MeasurementError(ValueError):
    """A deterministic, fail-closed measurement refusal."""


def run_bytes(
    command: list[str], *, cwd: Path, env: dict[str, str] | None = None
) -> bytes:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if completed.returncode != 0:
        sys.stderr.buffer.write(completed.stdout)
        raise MeasurementError(
            f"command failed ({completed.returncode}): {' '.join(command)}"
        )
    return completed.stdout


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> str:
    return run_bytes(command, cwd=cwd, env=env).decode("utf-8", errors="strict").strip()


def tool_version(command: list[str], *, cwd: Path) -> str:
    return " | ".join(
        line.strip() for line in run(command, cwd=cwd).splitlines() if line.strip()
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_document(document: dict[str, Any], output: Path | None) -> None:
    encoded = json.dumps(document, indent=2, sort_keys=True) + "\n"
    if output is None:
        sys.stdout.write(encoded)
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(encoded, encoding="utf-8")


def unavailable_document(*reasons: str) -> dict[str, Any]:
    return {
        "schema": checker.LINKED_MEASUREMENT_SCHEMA,
        "availability": "unavailable",
        "release_declaration": False,
        "manifest_input_source_clean": False,
        "source": None,
        "toolchain": None,
        "rent_model": None,
        "profiles": [],
        "refusals": list(reasons),
    }


def repository_relative(repo: Path, path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(repo.resolve()).as_posix()
    except ValueError as exc:
        raise MeasurementError(f"identity manifest escapes repository: {path}") from exc


def split_status(lines: list[str]) -> tuple[list[str], list[str]]:
    tracked: list[str] = []
    untracked: list[str] = []
    for line in lines:
        if line.startswith("?? "):
            untracked.append(line[3:])
        else:
            tracked.append(line)
    return sorted(tracked), sorted(untracked)


def source_state(repo: Path, closure_paths: list[str]) -> dict[str, Any]:
    status_text = run(
        [
            "git",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            *closure_paths,
        ],
        cwd=repo,
    )
    tracked_dirty, untracked = split_status(
        status_text.splitlines() if status_text else []
    )
    listed = run_bytes(["git", "ls-files", "-z", "--", *closure_paths], cwd=repo)
    files = sorted(
        item.decode("utf-8", errors="strict") for item in listed.split(b"\0") if item
    )
    if not files:
        raise MeasurementError("declared source closure is empty")
    digest = hashlib.sha256()
    for relative in files:
        path = repo / relative
        if path.is_symlink() or not path.is_file():
            raise MeasurementError(
                f"tracked build input is not a regular in-repository file: {relative}"
            )
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256_file(path)))
        digest.update(b"\0")
    return {
        "tracked_dirty": tracked_dirty,
        "untracked": untracked,
        "files": files,
        "digest": digest.hexdigest(),
    }


def require_clean_state(state: dict[str, Any], phase: str) -> None:
    if state["tracked_dirty"] or state["untracked"]:
        raise MeasurementError(
            f"{phase} linked source closure is dirty: "
            f"tracked={state['tracked_dirty']} untracked={state['untracked']}"
        )


def section_extent(readobj_text: str, name: str) -> tuple[int, int]:
    """Return the exact virtual-address base and byte size of one ELF section."""

    matches: list[tuple[int, int]] = []
    for block in re.findall(
        r"(?:^|\n)\s*Section \{\n(.*?)(?=\n\s*\}(?:\n|\Z))",
        readobj_text,
        flags=re.DOTALL,
    ):
        section_name = re.search(r"^\s*Name: ([^ ]+)(?: \(\d+\))?\s*$", block, re.MULTILINE)
        if section_name is None or section_name.group(1) != name:
            continue
        address = re.search(r"^\s*Address: (0x[0-9A-Fa-f]+|[0-9]+)\s*$", block, re.MULTILINE)
        size = re.search(r"^\s*Size: (0x[0-9A-Fa-f]+|[0-9]+)\s*$", block, re.MULTILINE)
        if address is None or size is None:
            raise MeasurementError(f"final ELF {name} section lacks address or size")
        matches.append((int(address.group(1), 0), int(size.group(1), 0)))
    if len(matches) != 1:
        raise MeasurementError(f"final ELF has no exact {name} section")
    return matches[0]


def section_size(readobj_text: str, name: str) -> int:
    return section_extent(readobj_text, name)[1]


def measurement_code_provenance(repo: Path, commit: str) -> list[dict[str, str]]:
    """Bind every executed first-party measurement body to bytes and Git blob."""

    rows: list[dict[str, str]] = []
    for role, relative in checker.LINKED_MEASUREMENT_CODE_INPUTS:
        path = repo / relative
        if not path.is_file() or path.is_symlink():
            raise MeasurementError(f"measurement code is not a regular file: {relative}")
        blob = run(["git", "rev-parse", f"{commit}:{relative}"], cwd=repo)
        if checker.GIT_OBJECT_ID.fullmatch(blob) is None or set(blob) == {"0"}:
            raise MeasurementError(f"malformed Git blob identity for {relative}: {blob}")
        working_sha256 = sha256_file(path)
        committed_bytes = run_bytes(
            ["git", "cat-file", "blob", f"{commit}:{relative}"], cwd=repo
        )
        committed_sha256 = hashlib.sha256(committed_bytes).hexdigest()
        if working_sha256 != committed_sha256:
            raise MeasurementError(
                f"measurement code differs from selected Git blob: {relative}"
            )
        rows.append(
            {
                "role": role,
                "path": relative,
                "sha256": working_sha256,
                "git_blob_oid": blob,
            }
        )
    return rows


def undefined_dynamic_symbols(readobj_text: str) -> list[str]:
    symbols: set[str] = set()
    for block in re.findall(r"  Symbol \{(.*?)\n  \}", readobj_text, re.DOTALL):
        if "Section: Undefined" not in block:
            continue
        match = re.search(r"\n    Name: ([^ ]*)", block)
        if match is not None and match.group(1):
            symbols.add(match.group(1))
    return sorted(symbols)


def backend_stack_audit(build_log: str, symbol_text: str) -> dict[str, int]:
    diagnostic_lines = sorted(
        {
            line
            for line in build_log.splitlines()
            if line.startswith("Error: Function ")
            or line.startswith("Error: A function call")
        }
    )
    patterns = (
        re.compile(r"^Error: Function ([^ ]+) "),
        re.compile(r"^Error: A function call in method ([^ ]+) "),
    )
    diagnosed: set[str] = set()
    for line in diagnostic_lines:
        for pattern in patterns:
            match = pattern.match(line)
            if match is not None:
                diagnosed.add(match.group(1))
                break
        else:
            raise MeasurementError(f"unparsed backend stack diagnostic: {line}")
    survivors = sorted(symbol for symbol in diagnosed if symbol in symbol_text)
    if survivors:
        raise MeasurementError(
            "backend stack-diagnostic symbols survived final LTO: "
            + ", ".join(survivors)
        )
    return {
        "backend_stack_diagnostic_lines": len(diagnostic_lines),
        "backend_stack_diagnostic_symbols": len(diagnosed),
        "backend_stack_diagnostic_survivors": 0,
    }


def stable_symbol_identity(symbol: str) -> str:
    """Remove only rustc's per-compilation legacy symbol hash.

    Cargo-default and explicit-feature builds can produce an identical stripped
    deployable ELF while the unstripped audit symbols carry different rustc
    hashes. The function path and every numeric frame result remain evidence;
    the compiler-internal suffix is not part of deployable artifact identity.
    """

    return LEGACY_RUST_SYMBOL_HASH.sub("E", symbol)


def final_frame_audit(symbol_text: str, disassembly: str) -> dict[str, Any]:
    function_symbols: set[str] = set()
    function_addresses: set[int] = set()
    for line in symbol_text.splitlines():
        fields = line.split()
        if len(fields) >= 6 and fields[2] == "F" and fields[3] == ".text":
            function_symbols.add(fields[-1])
            function_addresses.add(int(fields[0], 16))
    if not function_symbols:
        raise MeasurementError("final ELF symbol table exposes no text functions")

    current: str | None = None
    seen_addresses: set[int] = set()
    max_offset = 0
    max_function: str | None = None
    references = 0
    header = re.compile(r"^([0-9a-f]+) <(.+)>:$")
    reference = re.compile(r"\[r10\s*([+-])\s*0x([0-9a-f]+)\]")
    for line in disassembly.splitlines():
        match = header.match(line.strip())
        if match is not None:
            seen_addresses.add(int(match.group(1), 16))
            current = match.group(2)
            continue
        for match in reference.finditer(line):
            references += 1
            sign, encoded = match.groups()
            offset = int(encoded, 16)
            if current is None:
                raise MeasurementError(
                    f"stack reference outside a named function: {line}"
                )
            if sign != "-" or not 1 <= offset <= 4096:
                raise MeasurementError(
                    f"out-of-frame direct r10 reference in {current}: {line.strip()}"
                )
            if offset > max_offset:
                max_offset = offset
                max_function = stable_symbol_identity(current)
    missing = function_addresses - seen_addresses
    if missing:
        raise MeasurementError(
            f"{len(missing)} text-function addresses were not disassembled; first 0x{min(missing):x}"
        )
    return {
        "final_text_function_symbols": len(function_symbols),
        "final_text_function_addresses": len(function_addresses),
        "disassembled_function_regions": len(seen_addresses),
        "direct_r10_references": references,
        "deepest_direct_r10_offset": max_offset,
        "deepest_direct_r10_function": max_function,
        "direct_frame_limit_bytes": 4096,
        "direct_frame_bounds": "PASS",
    }


def loader_account(role: str, data_len: int) -> dict[str, int | str]:
    billable = data_len + ACCOUNT_STORAGE_OVERHEAD_BYTES
    return {
        "lifetime": "transient-recyclable" if role == "buffer" else "persistent",
        "data_len_bytes": data_len,
        "storage_overhead_bytes": ACCOUNT_STORAGE_OVERHEAD_BYTES,
        "billable_bytes": billable,
        "rent_exempt_lamports": billable * RENT_EXEMPT_LAMPORTS_PER_BILLABLE_BYTE,
    }


def loader_measurement(elf_bytes: int, chosen_max_len: int) -> dict[str, Any]:
    if elf_bytes > chosen_max_len:
        raise MeasurementError(
            f"ELF length {elf_bytes} exceeds chosen ProgramData max_len {chosen_max_len}"
        )
    programdata_len = PROGRAMDATA_METADATA_DATA_LEN_BYTES + chosen_max_len
    if programdata_len > MAX_PERMITTED_DATA_LENGTH:
        raise MeasurementError(
            f"ProgramData data length {programdata_len} exceeds loader limit {MAX_PERMITTED_DATA_LENGTH}"
        )
    program = loader_account("program", PROGRAM_DATA_LEN_BYTES)
    programdata = loader_account("programdata", programdata_len)
    buffer = loader_account("buffer", BUFFER_METADATA_DATA_LEN_BYTES + chosen_max_len)
    return {
        "current_elf_len_bytes": elf_bytes,
        "chosen_programdata_max_len": chosen_max_len,
        "exact_size_allocation": elf_bytes == chosen_max_len,
        "program": program,
        "programdata": programdata,
        "buffer": buffer,
        "persistent_program_plus_programdata_rent_lamports": (
            int(program["rent_exempt_lamports"])
            + int(programdata["rent_exempt_lamports"])
        ),
        "transient_buffer_rent_lamports": int(buffer["rent_exempt_lamports"]),
    }


def find_unstripped_elf(target: Path) -> Path:
    matches = sorted(target.glob("**/release/deps/clutch_sbf.so"))
    if len(matches) != 1:
        raise MeasurementError(
            f"expected exactly one unstripped final ELF; found {len(matches)} under {target}"
        )
    return matches[0]


def measure_elf(
    *,
    repo: Path,
    readobj: Path,
    objdump: Path,
    elf: Path,
    unstripped: Path,
    build_log: str,
    run_number: int,
    build_mode: str,
    chosen_max_len: int,
    expected_syscalls: list[str],
) -> dict[str, Any]:
    readobj_text = run(
        [
            str(readobj),
            "--file-headers",
            "--sections",
            "--program-headers",
            "--dynamic-table",
            "--dyn-symbols",
            str(elf),
        ],
        cwd=repo,
    )
    for marker in (
        "Format: elf64-sbf",
        "Arch: sbf",
        "Type: SharedObject",
        "Machine: EM_SBF",
    ):
        if marker not in readobj_text:
            raise MeasurementError(f"final artifact missing ELF marker: {marker}")
    if "Flags [ (0x7)" in readobj_text:
        raise MeasurementError(
            "final artifact contains a writable-executable program segment"
        )
    symbols = run([str(objdump), "--syms", str(unstripped)], cwd=repo)
    disassembly = run(
        [str(objdump), "--disassemble", "--no-show-raw-insn", str(unstripped)], cwd=repo
    )
    syscalls = undefined_dynamic_symbols(readobj_text)
    if syscalls != expected_syscalls:
        raise MeasurementError(
            f"undefined dynamic-symbol surface mismatch: got {syscalls}, expected {expected_syscalls}"
        )
    elf_bytes = elf.stat().st_size
    return {
        "run": run_number,
        "build_mode": build_mode,
        "elf_sha256": sha256_file(elf),
        "elf_bytes": elf_bytes,
        "text_bytes": section_size(readobj_text, ".text"),
        "rodata_bytes": section_size(readobj_text, ".rodata"),
        "undefined_dynamic_symbols": syscalls,
        **backend_stack_audit(build_log, symbols),
        "final_frame_audit": final_frame_audit(symbols, disassembly),
        "loader": loader_measurement(elf_bytes, chosen_max_len),
    }


def build_once(
    *,
    repo: Path,
    cargo_build_sbf: Path,
    readobj: Path,
    objdump: Path,
    manifest_summary: dict[str, Any],
    run_number: int,
    build_mode: str,
    keep_workdirs: bool,
) -> tuple[dict[str, Any], str | None]:
    name = manifest_summary["profile_name"]
    work = Path(tempfile.mkdtemp(prefix=f"dragons-clutch-{name}-run{run_number}-"))
    target = work / "target"
    output = work / "out"
    output.mkdir()
    env = os.environ.copy()
    for hostile in (
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_PROFILE_RELEASE_OPT_LEVEL",
        "CARGO_TARGET_DIR",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTDOC",
        "RUSTFLAGS",
    ):
        env.pop(hostile, None)
    env["CARGO_NET_OFFLINE"] = "true"
    env["CARGO_TARGET_DIR"] = str(target)
    command = [
        str(cargo_build_sbf),
        "--manifest-path",
        "programs/clutch-sbf/program/Cargo.toml",
        "--arch",
        "v0",
        "--offline",
        "--skip-tools-install",
        "--tools-version",
        manifest_summary["platform_tools"],
    ]
    if build_mode == "explicit-profile":
        command.extend(
            [
                "--no-default-features",
                "--features",
                ",".join(manifest_summary["cargo_features"]),
            ]
        )
    elif build_mode != "cargo-default":
        raise MeasurementError(f"unknown build mode: {build_mode}")
    command.extend(["--sbf-out-dir", str(output), "--", "--locked"])
    print(f"building {name} {build_mode} run {run_number}", file=sys.stderr, flush=True)
    try:
        build_log = run(command, cwd=repo, env=env)
        measurement = measure_elf(
            repo=repo,
            readobj=readobj,
            objdump=objdump,
            elf=output / "clutch_sbf.so",
            unstripped=find_unstripped_elf(target),
            build_log=build_log,
            run_number=run_number,
            build_mode=build_mode,
            chosen_max_len=manifest_summary["limits"]["programdata_max_len"],
            expected_syscalls=manifest_summary["build_contract"][
                "expected_undefined_dynamic_symbols"
            ],
        )
        return measurement, str(work) if keep_workdirs else None
    finally:
        if not keep_workdirs:
            shutil.rmtree(work)


def comparable_measurement(value: dict[str, Any]) -> dict[str, Any]:
    return {
        key: item for key, item in value.items() if key not in {"run", "build_mode"}
    }


def load_identity_manifest(repo: Path, path: Path) -> dict[str, Any]:
    relative = repository_relative(repo, path)
    try:
        run(["git", "ls-files", "--error-unmatch", "--", relative], cwd=repo)
    except MeasurementError as exc:
        raise MeasurementError(f"identity manifest is not tracked: {relative}") from exc
    try:
        data = checker.load_json(repo / relative)
        summary = checker.validate_manifest(data, repo=repo)
    except checker.ProfileError as exc:
        raise MeasurementError(f"invalid identity manifest {relative}: {exc}") from exc
    if summary["measurement_class"] != "planned":
        raise MeasurementError(
            f"measurement input {relative} must use artifact_budget.measurement_class=planned"
        )
    if summary["planned_capabilities"]:
        raise MeasurementError(
            f"measurement input {relative} has model-only/planned semantic owners: "
            f"{summary['planned_capabilities']}"
        )
    profile = data["profile"]
    return {
        **summary,
        "profile_name": profile["name"],
        "profile_label": profile["label"],
        "build_contract": data["build_contract"],
        "identity_manifest_path": relative,
        "identity_manifest_sha256": checker.measurement_input_manifest_sha256(data),
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write JSON here instead of stdout")
    parser.add_argument("--keep-workdirs", action="store_true")
    parser.add_argument(
        "--identity-manifest",
        action="append",
        type=Path,
        help="tracked schema-V2 planning manifest to measure (repeatable)",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo = Path(__file__).resolve().parents[3]
    if not args.identity_manifest:
        write_document(
            unavailable_document(
                "no concrete fully linked semantic/profile manifest was supplied; no ELF was built"
            ),
            args.output,
        )
        return 2

    try:
        manifests = [
            load_identity_manifest(repo, path if path.is_absolute() else repo / path)
            for path in args.identity_manifest
        ]
        names = [manifest["profile_name"] for manifest in manifests]
        if len(set(names)) != len(names):
            raise MeasurementError(f"duplicate profile names: {names}")
        identity_paths = [manifest["identity_manifest_path"] for manifest in manifests]
        closure_paths = sorted(set(SOURCE_CLOSURE) | set(identity_paths))
        before = source_state(repo, closure_paths)
        require_clean_state(before, "pre-build")
        commit_before = run(["git", "rev-parse", "HEAD"], cwd=repo)
        tree_before = run(["git", "rev-parse", "HEAD^{tree}"], cwd=repo)
        code_provenance = measurement_code_provenance(repo, commit_before)

        cargo_build_sbf = Path(
            os.environ.get(
                "CARGO_BUILD_SBF",
                str(
                    Path.home()
                    / ".local/share/solana/install/active_release/bin/cargo-build-sbf"
                ),
            )
        )
        if not cargo_build_sbf.is_file():
            raise MeasurementError(f"required builder is missing: {cargo_build_sbf}")
        builder_version = tool_version([str(cargo_build_sbf), "--version"], cwd=repo)
        platform_match = PLATFORM_TOOLS_RE.search(builder_version.replace(" | ", "\n"))
        if platform_match is None:
            raise MeasurementError(
                "cargo-build-sbf did not name one platform-tools version"
            )
        platform_tools = platform_match.group(1)
        platform = Path.home() / ".cache/solana" / platform_tools / "platform-tools"
        readobj = platform / "llvm/bin/llvm-readobj"
        objdump = platform / "llvm/bin/llvm-objdump"
        rustc = platform / "rust/bin/rustc"
        for tool in (readobj, objdump, rustc):
            if not tool.is_file():
                raise MeasurementError(f"required pinned tool is missing: {tool}")
        for manifest in manifests:
            manifest["platform_tools"] = platform_tools

        results: list[dict[str, Any]] = []
        for manifest in manifests:
            first, first_work = build_once(
                repo=repo,
                cargo_build_sbf=cargo_build_sbf,
                readobj=readobj,
                objdump=objdump,
                manifest_summary=manifest,
                run_number=1,
                build_mode="explicit-profile",
                keep_workdirs=args.keep_workdirs,
            )
            second, second_work = build_once(
                repo=repo,
                cargo_build_sbf=cargo_build_sbf,
                readobj=readobj,
                objdump=objdump,
                manifest_summary=manifest,
                run_number=2,
                build_mode="explicit-profile",
                keep_workdirs=args.keep_workdirs,
            )
            reproducible = comparable_measurement(first) == comparable_measurement(
                second
            )
            if not reproducible:
                raise MeasurementError(
                    f"non-reproducible profile build: {manifest['profile_name']}"
                )

            default_equivalence: dict[str, Any] | None = None
            if (
                manifest["build_contract"]["cargo_profile_feature"] == "profile-full"
                and manifest["build_contract"]["source_identity"] == "production-inert"
                and manifest["build_contract"]["collateral_release_identity"]
                == "production-inert"
            ):
                default, default_work = build_once(
                    repo=repo,
                    cargo_build_sbf=cargo_build_sbf,
                    readobj=readobj,
                    objdump=objdump,
                    manifest_summary=manifest,
                    run_number=3,
                    build_mode="cargo-default",
                    keep_workdirs=args.keep_workdirs,
                )
                matches = comparable_measurement(default) == comparable_measurement(
                    first
                )
                if not matches:
                    raise MeasurementError(
                        "Cargo default build differs from explicit profile-full under the same identity manifest"
                    )
                default_equivalence = {
                    "capability_profile_identity_sha256": manifest[
                        "profile_identity_sha256"
                    ],
                    "measurement": default,
                    "matches_explicit": True,
                }
                retained = [
                    path for path in (first_work, second_work, default_work) if path
                ]
            else:
                retained = [path for path in (first_work, second_work) if path]

            results.append(
                {
                    "name": manifest["profile_name"],
                    "label": manifest["profile_label"],
                    "source_identity": manifest["source_identity"],
                    "collateral_release_identity": manifest[
                        "collateral_release_identity"
                    ],
                    "cargo_features": manifest["cargo_features"],
                    "capability_profile_identity_sha256": manifest[
                        "profile_identity_sha256"
                    ],
                    "identity_manifest_sha256": manifest["identity_manifest_sha256"],
                    "semantic_owners": manifest["capabilities"],
                    "central_registry": manifest["central_registry"],
                    "wire_surface": manifest["wire_surface"],
                    "wire_surface_sha256": manifest["wire_surface_sha256"],
                    "reproducible": True,
                    "measurements": [first, second],
                    "default_feature_equivalence": default_equivalence,
                    "retained_workdirs": retained,
                }
            )

        after = source_state(repo, closure_paths)
        require_clean_state(after, "post-build")
        if before["files"] != after["files"] or before["digest"] != after["digest"]:
            raise MeasurementError("linked source closure changed during measurement")
        commit_after = run(["git", "rev-parse", "HEAD"], cwd=repo)
        tree_after = run(["git", "rev-parse", "HEAD^{tree}"], cwd=repo)
        if commit_before != commit_after or tree_before != tree_after:
            raise MeasurementError("repository HEAD changed during measurement")

        document = {
            "schema": checker.LINKED_MEASUREMENT_SCHEMA,
            "availability": "available",
            "release_declaration": False,
            "manifest_input_source_clean": True,
            "source": {
                "git_commit": commit_before,
                "git_tree": tree_before,
                "closure_paths": closure_paths,
                "closure_file_count": len(before["files"]),
                "closure_digest_sha256": before["digest"],
                "measurement_code": code_provenance,
                "cleanliness": {
                    "tracked_before": before["tracked_dirty"],
                    "untracked_before": before["untracked"],
                    "tracked_after": after["tracked_dirty"],
                    "untracked_after": after["untracked"],
                },
            },
            "toolchain": {
                "cargo_build_sbf": {
                    "version": builder_version,
                    "sha256": sha256_file(cargo_build_sbf),
                },
                "platform_rustc": {
                    "version": tool_version([str(rustc), "--version"], cwd=repo),
                    "sha256": sha256_file(rustc),
                },
                "llvm_readobj": {
                    "version": tool_version([str(readobj), "--version"], cwd=repo),
                    "sha256": sha256_file(readobj),
                },
                "llvm_objdump": {
                    "version": tool_version([str(objdump), "--version"], cwd=repo),
                    "sha256": sha256_file(objdump),
                },
                "platform_tools": platform_tools,
                "cargo_profile": "release",
                "lto": "fat",
                "codegen_units": 1,
                "overflow_checks": True,
            },
            "rent_model": {
                "model": "upgradeable-loader-v3",
                "rent_exempt_lamports_per_billable_byte": RENT_EXEMPT_LAMPORTS_PER_BILLABLE_BYTE,
                "account_storage_overhead_bytes": ACCOUNT_STORAGE_OVERHEAD_BYTES,
                "program_data_len_bytes": PROGRAM_DATA_LEN_BYTES,
                "programdata_metadata_data_len_bytes": PROGRAMDATA_METADATA_DATA_LEN_BYTES,
                "buffer_metadata_data_len_bytes": BUFFER_METADATA_DATA_LEN_BYTES,
            },
            "profiles": results,
            "refusals": [],
        }
        write_document(document, args.output)
        return 0
    except (MeasurementError, checker.ProfileError) as exc:
        write_document(unavailable_document(str(exc)), args.output)
        print(f"REFUSE: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
