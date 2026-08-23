#!/usr/bin/env python3
"""Measure source-derived SBF profile sizes without promoting profile identity.

This is an optimization diagnostic, not the manifest-linked producer.  It
builds an exact Git commit from a temporary ``git archive`` checkout, never
from dirty worktree bytes.  The caller must name every profile and Cargo
feature explicitly.  Results deliberately say that semantic-owner and central
registry manifests are absent, so they cannot satisfy
``check_capability_profile.py`` or declare a deployable/release artifact.

Use ``measure_capability_profiles.py`` for the fail-closed schema-V2 identity
and promotion lane.
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
import tarfile
import tempfile
from typing import Any

import check_capability_profile as checker
import measure_capability_profiles as linked


SCHEMA = "dragons-clutch/capability-profile-size-diagnostic/v1"
PROFILE_SPEC = re.compile(r"([a-z0-9][a-z0-9-]{0,63})=([a-z0-9][a-z0-9-]{0,127})\Z")
TEXT_SYMBOL = re.compile(
    r"^([0-9a-f]+)\s+\w\s+F\s+\.text\s+([0-9a-f]+)\s+"
    r"(?:\.hidden\s+)?(.*)$"
)


class DiagnosticError(ValueError):
    """A deterministic diagnostic refusal."""


def explicit_profile_features(feature: str) -> list[str]:
    """Return the complete Cargo feature set for one explicit profile build.

    Cargo enables its named ``default`` feature on the default route.  Even
    though no program source branches on that marker, omitting it changes
    rustc's crate identity and can perturb LTO ordering.  Explicit full builds
    retain the marker so they are byte-comparable with Cargo defaults.  Narrow
    profiles cannot enable it because it expands to ``profile-full``.
    """

    return checker.cargo_features(
        {
            "cargo_profile_feature": feature,
            "source_identity": "production-inert",
        }
    )


def parse_profile_specs(values: list[str] | None) -> list[tuple[str, str]]:
    """Parse ordered, unique ``NAME=FEATURE`` diagnostic selectors."""

    if not values:
        raise DiagnosticError("at least one explicit --profile NAME=FEATURE is required")
    parsed: list[tuple[str, str]] = []
    seen_names: set[str] = set()
    seen_features: set[str] = set()
    for value in values:
        match = PROFILE_SPEC.fullmatch(value)
        if match is None:
            raise DiagnosticError(f"malformed profile selector: {value!r}")
        name, feature = match.groups()
        if feature not in checker.PROFILE_FEATURES:
            raise DiagnosticError(f"unknown capability-profile feature: {feature!r}")
        if name in seen_names:
            raise DiagnosticError(f"duplicate profile name: {name!r}")
        if feature in seen_features:
            raise DiagnosticError(f"duplicate profile feature: {feature!r}")
        seen_names.add(name)
        seen_features.add(feature)
        parsed.append((name, feature))
    return parsed


def extract_git_archive(repo: Path, commit: str, destination: Path) -> None:
    """Extract exactly one committed tree without consulting worktree bytes."""

    process = subprocess.Popen(
        ["git", "archive", "--format=tar", commit],
        cwd=repo,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert process.stdout is not None
    try:
        with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
            archive.extractall(destination, filter="data")
    except (tarfile.TarError, OSError) as exc:
        process.kill()
        process.wait()
        raise DiagnosticError(f"cannot extract git archive for {commit}: {exc}") from exc
    stderr = process.stderr.read() if process.stderr is not None else b""
    status = process.wait()
    if status != 0:
        raise DiagnosticError(
            f"git archive failed ({status}): {stderr.decode('utf-8', errors='replace').strip()}"
        )


def observed_syscalls(repo: Path, readobj: Path, elf: Path) -> list[str]:
    """Return the artifact's observed undefined dynamic-symbol surface."""

    text = linked.run([str(readobj), "--dyn-symbols", str(elf)], cwd=repo)
    return linked.undefined_dynamic_symbols(text)


def symbol_group(name: str) -> str:
    """Map one demangled final symbol to a stable, reviewable code family."""

    encoded_crate = re.search(r"\$LT\$([a-z][a-z0-9_]*)\.\.", name)
    if encoded_crate is not None:
        return encoded_crate.group(1)
    parts = name.split("::")
    if len(parts) >= 4 and parts[:2] == ["clutch_sbf", "instructions"]:
        return "::".join(parts[:3])
    if len(parts) >= 2 and parts[0] == "clutch_sbf":
        return "::".join(parts[:2])
    if len(parts) >= 2 and re.fullmatch(r"[a-z][a-z0-9_]*", parts[0]):
        return parts[0]
    return "rust-runtime-or-unattributed"


def text_symbol_attribution(
    symbol_text: str, text_start: int, text_size: int
) -> dict[str, Any]:
    """Require a canonical symbol-region union exactly equal to final ``.text``."""

    if text_start < 0 or text_size <= 0:
        raise DiagnosticError("final .text extent must have a nonnegative base and positive size")
    text_end = text_start + text_size

    regions: dict[tuple[int, int], list[str]] = {}
    for line in symbol_text.splitlines():
        match = TEXT_SYMBOL.match(line)
        if match is None:
            continue
        address_text, size_text, name = match.groups()
        address = int(address_text, 16)
        size = int(size_text, 16)
        if size == 0:
            raise DiagnosticError(f"zero-sized final .text symbol at 0x{address:x}: {name}")
        end = address + size
        if address < text_start or end > text_end:
            raise DiagnosticError(
                f"final .text symbol range 0x{address:x}..0x{end:x} is outside "
                f"section 0x{text_start:x}..0x{text_end:x}: {name}"
            )
        regions.setdefault((address, size), []).append(name)
    if not regions:
        raise DiagnosticError("final ELF exposes no attributable text symbols")
    cursor = text_start
    for address, size in sorted(regions):
        if address < cursor:
            raise DiagnosticError(
                f"overlapping final .text symbol regions at 0x{address:x}; "
                f"canonical union already reaches 0x{cursor:x}"
            )
        if address > cursor:
            raise DiagnosticError(
                f"gap in final .text symbol regions: 0x{cursor:x}..0x{address:x}"
            )
        cursor = address + size
    if cursor != text_end:
        raise DiagnosticError(
            f"gap at end of final .text symbol union: 0x{cursor:x}..0x{text_end:x}"
        )
    groups: dict[str, int] = {}
    merged_alias_regions = 0
    for (_address, size), names in regions.items():
        candidate_groups = {symbol_group(name) for name in names}
        if len(candidate_groups) == 1:
            group = next(iter(candidate_groups))
        else:
            group = "merged-aliases"
            merged_alias_regions += 1
        groups[group] = groups.get(group, 0) + size
    attributed = sum(groups.values())
    if attributed != text_size:
        raise DiagnosticError(
            f"final text symbol attribution covered {attributed} of {text_size} bytes"
        )
    return {
        "classification": (
            "canonical exact union of final .text address ranges; only identical folded "
            "aliases are deduplicated and cross-family aliases are named merged-aliases"
        ),
        "text_section_start": text_start,
        "text_section_end_exclusive": text_end,
        "text_section_bytes": text_size,
        "canonical_union_start": text_start,
        "canonical_union_end_exclusive": cursor,
        "canonical_union_exact_coverage": True,
        "function_regions": len(regions),
        "merged_alias_regions": merged_alias_regions,
        "attributed_text_bytes": attributed,
        "matches_text_section": True,
        "groups": [
            {"name": name, "text_bytes": size}
            for name, size in sorted(groups.items(), key=lambda item: (-item[1], item[0]))
        ],
    }


def build_once(
    *,
    source: Path,
    cargo_build_sbf: Path,
    readobj: Path,
    objdump: Path,
    work: Path,
    name: str,
    feature: str,
    run_number: int,
    build_mode: str,
    expected_syscalls: list[str] | None,
) -> dict[str, Any]:
    """Build and audit one fresh diagnostic artifact."""

    target = work / f"target-{name}-{build_mode}-run{run_number}"
    output = work / f"out-{name}-{build_mode}-run{run_number}"
    output.mkdir(parents=True)
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
        str(source / "programs/clutch-sbf/program/Cargo.toml"),
        "--arch",
        "v0",
        "--offline",
        "--skip-tools-install",
        "--tools-version",
        "v1.53",
    ]
    if build_mode == "explicit-profile":
        command.extend(
            [
                "--no-default-features",
                "--features",
                ",".join(explicit_profile_features(feature)),
            ]
        )
    elif build_mode != "cargo-default":
        raise DiagnosticError(f"unknown build mode: {build_mode}")
    command.extend(["--sbf-out-dir", str(output), "--", "--locked"])
    print(
        f"building diagnostic {name} {build_mode} run {run_number}",
        file=sys.stderr,
        flush=True,
    )
    build_log = linked.run(command, cwd=source, env=env)
    elf = output / "clutch_sbf.so"
    if not elf.is_file():
        raise DiagnosticError(f"builder did not emit {elf}")
    syscalls = observed_syscalls(source, readobj, elf)
    if expected_syscalls is not None and syscalls != expected_syscalls:
        raise DiagnosticError(
            f"{name} syscall surface changed between runs: "
            f"{expected_syscalls} != {syscalls}"
        )
    measurement = linked.measure_elf(
        repo=source,
        readobj=readobj,
        objdump=objdump,
        elf=elf,
        unstripped=linked.find_unstripped_elf(target),
        build_log=build_log,
        run_number=run_number,
        build_mode=build_mode,
        chosen_max_len=elf.stat().st_size,
        expected_syscalls=syscalls,
    )
    demangled_symbols = linked.run(
        [str(objdump), "--syms", "--demangle", str(linked.find_unstripped_elf(target))],
        cwd=source,
    )
    section_report = linked.run(
        [str(readobj), "--sections", str(elf)], cwd=source
    )
    text_start, text_size = linked.section_extent(section_report, ".text")
    if text_size != measurement["text_bytes"]:
        raise DiagnosticError("independent .text extent disagrees with ELF measurement")
    measurement["text_symbol_attribution"] = text_symbol_attribution(
        demangled_symbols, text_start, text_size
    )
    return measurement


def pairwise_deltas(profiles: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Derive exact signed size and rent deltas for every ordered input pair."""

    rows: list[dict[str, Any]] = []
    for left_index, left in enumerate(profiles):
        left_measurement = left["measurements"][0]
        for right in profiles[left_index + 1 :]:
            right_measurement = right["measurements"][0]
            left_rent = left_measurement["loader"][
                "persistent_program_plus_programdata_rent_lamports"
            ]
            right_rent = right_measurement["loader"][
                "persistent_program_plus_programdata_rent_lamports"
            ]
            left_groups = {
                row["name"]: row["text_bytes"]
                for row in left_measurement["text_symbol_attribution"]["groups"]
            }
            right_groups = {
                row["name"]: row["text_bytes"]
                for row in right_measurement["text_symbol_attribution"]["groups"]
            }
            group_deltas = [
                {"name": name, "text_bytes_delta": right_groups.get(name, 0) - left_groups.get(name, 0)}
                for name in sorted(set(left_groups) | set(right_groups))
                if right_groups.get(name, 0) != left_groups.get(name, 0)
            ]
            group_deltas.sort(key=lambda row: (-abs(row["text_bytes_delta"]), row["name"]))
            rows.append(
                {
                    "from": left["name"],
                    "to": right["name"],
                    "elf_bytes_delta": right_measurement["elf_bytes"]
                    - left_measurement["elf_bytes"],
                    "text_bytes_delta": right_measurement["text_bytes"]
                    - left_measurement["text_bytes"],
                    "rodata_bytes_delta": right_measurement["rodata_bytes"]
                    - left_measurement["rodata_bytes"],
                    "persistent_loader_rent_lamports_delta": right_rent - left_rent,
                    "text_symbol_group_deltas": group_deltas,
                }
            )
    return rows


def comparison(
    explicit: dict[str, Any], defaults: list[dict[str, Any]]
) -> dict[str, Any]:
    """Describe default-route reproducibility and its exact artifact fork."""

    reproducible = linked.comparable_measurement(defaults[0]) == linked.comparable_measurement(
        defaults[1]
    )
    if not reproducible:
        raise DiagnosticError("Cargo-default diagnostic builds are not reproducible")
    explicit_comparable = linked.comparable_measurement(explicit)
    default_comparable = linked.comparable_measurement(defaults[0])
    comparable_keys = sorted(set(explicit_comparable) | set(default_comparable))
    mismatches = {
        key: {
            "explicit": explicit_comparable.get(key),
            "cargo_default": default_comparable.get(key),
        }
        for key in comparable_keys
        if explicit_comparable.get(key) != default_comparable.get(key)
    }
    byte_identical = explicit.get("elf_sha256") == defaults[0].get("elf_sha256")
    strict_equivalent = not mismatches
    return {
        "cargo_default_reproducible": True,
        "byte_identity_basis": "stripped_deployable_elf_sha256_only",
        "byte_identical_to_explicit_profile": byte_identical,
        "strict_equivalence_basis": "measure_capability_profiles.comparable_measurement",
        "strict_v2_default_equivalence_gate": "PASS" if strict_equivalent else "REFUSE",
        "mismatches": mismatches,
        "measurements": defaults,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile",
        action="append",
        help="diagnostic NAME=FEATURE selector; repeat for each profile",
    )
    parser.add_argument(
        "--cargo-default-profile",
        help="also build Cargo defaults twice and compare with this named profile",
    )
    parser.add_argument("--commit", default="HEAD", help="Git commit to archive")
    parser.add_argument("--output", type=Path, help="write JSON here instead of stdout")
    parser.add_argument("--keep-workdir", action="store_true")
    return parser.parse_args(argv)


def write_document(document: dict[str, Any], output: Path | None) -> None:
    encoded = json.dumps(document, indent=2, sort_keys=True) + "\n"
    if output is None:
        sys.stdout.write(encoded)
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(encoded, encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo = Path(__file__).resolve().parents[3]
    work: Path | None = None
    work_created = False
    try:
        specs = parse_profile_specs(args.profile)
        names = {name for name, _feature in specs}
        if args.cargo_default_profile is not None and args.cargo_default_profile not in names:
            raise DiagnosticError("--cargo-default-profile must name one --profile")
        commit = linked.run(["git", "rev-parse", f"{args.commit}^{{commit}}"], cwd=repo)
        tree = linked.run(["git", "rev-parse", f"{commit}^{{tree}}"], cwd=repo)
        head_before = linked.run(["git", "rev-parse", "HEAD"], cwd=repo)
        evidence_class = "source-derived-selected-commit-artifact"

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
            raise DiagnosticError(f"required builder is missing: {cargo_build_sbf}")
        builder_version = linked.tool_version(
            [str(cargo_build_sbf), "--version"], cwd=repo
        )
        platform_match = linked.PLATFORM_TOOLS_RE.search(
            builder_version.replace(" | ", "\n")
        )
        if platform_match is None or platform_match.group(1) != "v1.53":
            raise DiagnosticError(
                f"diagnostic requires platform-tools v1.53, got {builder_version!r}"
            )
        platform = Path.home() / ".cache/solana/v1.53/platform-tools"
        readobj = platform / "llvm/bin/llvm-readobj"
        objdump = platform / "llvm/bin/llvm-objdump"
        rustc = platform / "rust/bin/rustc"
        for tool in (readobj, objdump, rustc):
            if not tool.is_file():
                raise DiagnosticError(f"required pinned tool is missing: {tool}")

        work = (
            Path(tempfile.gettempdir())
            / "dragons-clutch-profile-size-diagnostic-v1"
            / commit
        )
        try:
            work.mkdir(parents=True, exist_ok=False)
        except FileExistsError as exc:
            raise DiagnosticError(
                f"deterministic diagnostic workdir already exists: {work}"
            ) from exc
        work_created = True
        source = work / "source"
        source.mkdir()
        extract_git_archive(repo, commit, source)
        profiles: list[dict[str, Any]] = []
        for name, feature in specs:
            first = build_once(
                source=source,
                cargo_build_sbf=cargo_build_sbf,
                readobj=readobj,
                objdump=objdump,
                work=work,
                name=name,
                feature=feature,
                run_number=1,
                build_mode="explicit-profile",
                expected_syscalls=None,
            )
            second = build_once(
                source=source,
                cargo_build_sbf=cargo_build_sbf,
                readobj=readobj,
                objdump=objdump,
                work=work,
                name=name,
                feature=feature,
                run_number=2,
                build_mode="explicit-profile",
                expected_syscalls=first["undefined_dynamic_symbols"],
            )
            if linked.comparable_measurement(first) != linked.comparable_measurement(second):
                raise DiagnosticError(f"non-reproducible profile build: {name}")
            profiles.append(
                {
                    "name": name,
                    "cargo_features": explicit_profile_features(feature),
                    "profile_manifest_linkage": "absent",
                    "reproducible_same_source_checkout": True,
                    "measurements": [first, second],
                }
            )

        default_comparison: dict[str, Any] | None = None
        if args.cargo_default_profile is not None:
            selected = next(
                profile
                for profile in profiles
                if profile["name"] == args.cargo_default_profile
            )
            feature = next(
                feature for name, feature in specs if name == args.cargo_default_profile
            )
            default_first = build_once(
                source=source,
                cargo_build_sbf=cargo_build_sbf,
                readobj=readobj,
                objdump=objdump,
                work=work,
                name=args.cargo_default_profile,
                feature=feature,
                run_number=1,
                build_mode="cargo-default",
                expected_syscalls=None,
            )
            default_second = build_once(
                source=source,
                cargo_build_sbf=cargo_build_sbf,
                readobj=readobj,
                objdump=objdump,
                work=work,
                name=args.cargo_default_profile,
                feature=feature,
                run_number=2,
                build_mode="cargo-default",
                expected_syscalls=default_first["undefined_dynamic_symbols"],
            )
            defaults = [default_first, default_second]
            default_comparison = {
                "profile": args.cargo_default_profile,
                **comparison(selected["measurements"][0], defaults),
            }

        head_after = linked.run(["git", "rev-parse", "HEAD"], cwd=repo)
        if head_before != head_after:
            raise DiagnosticError("repository HEAD changed during diagnostic")
        document = {
            "schema": SCHEMA,
            "availability": "available",
            "evidence_class": evidence_class,
            "release_declaration": False,
            "deployment_evidence": False,
            "manifest_input_source_clean": None,
            "source": {
                "git_commit": commit,
                "git_tree": tree,
                "selected_commit_was_head_at_measurement": commit == head_before,
                "input_method": "git-archive",
                "worktree_bytes_used": False,
                "source_checkout_path_sha256": hashlib.sha256(
                    str(source.resolve()).encode("utf-8")
                ).hexdigest(),
                "reproducibility_scope": (
                    "two fresh target directories under a commit-keyed deterministic local "
                    "archive path; cross-host temporary-root paths can still affect Cargo "
                    "path-dependency crate disambiguators"
                ),
            },
            "producer": {
                "path": Path(__file__).resolve().relative_to(repo).as_posix(),
                "sha256": linked.sha256_file(Path(__file__)),
                "imported_inputs": [
                    {
                        "path": Path(linked.__file__).resolve().relative_to(repo).as_posix(),
                        "sha256": linked.sha256_file(Path(linked.__file__)),
                    },
                    {
                        "path": Path(checker.__file__).resolve().relative_to(repo).as_posix(),
                        "sha256": linked.sha256_file(Path(checker.__file__)),
                    },
                ],
            },
            "toolchain": {
                "cargo_build_sbf_version": builder_version,
                "cargo_build_sbf_sha256": linked.sha256_file(cargo_build_sbf),
                "platform_rustc_version": linked.tool_version(
                    [str(rustc), "--version"], cwd=repo
                ),
                "platform_rustc_sha256": linked.sha256_file(rustc),
                "llvm_readobj_sha256": linked.sha256_file(readobj),
                "llvm_objdump_sha256": linked.sha256_file(objdump),
                "cargo_profile": "release",
                "lto": "fat",
                "codegen_units": 1,
                "overflow_checks": True,
            },
            "rent_model": {
                "evidence_class": "model-only-derived-from-selected-commit-artifact-bytes",
                "model": "upgradeable-loader-v3-exact-size-allocation",
                "rent_exempt_lamports_per_billable_byte": linked.RENT_EXEMPT_LAMPORTS_PER_BILLABLE_BYTE,
                "account_storage_overhead_bytes": linked.ACCOUNT_STORAGE_OVERHEAD_BYTES,
                "program_data_len_bytes": linked.PROGRAM_DATA_LEN_BYTES,
                "programdata_metadata_data_len_bytes": linked.PROGRAMDATA_METADATA_DATA_LEN_BYTES,
                "buffer_metadata_data_len_bytes": linked.BUFFER_METADATA_DATA_LEN_BYTES,
            },
            "profiles": profiles,
            "pairwise_deltas": pairwise_deltas(profiles),
            "cargo_default_comparison": default_comparison,
            "promotion_gates": [
                "not accepted as linked evidence by check_capability_profile.py",
                "no semantic-owner or central-registry identity manifest",
                "no runtime, deployment, public-cluster, release, or production evidence",
                "loader rent is a formula over exact-size allocation, not a funded account observation",
            ],
            "retained_workdir": str(work) if args.keep_workdir else None,
            "refusals": [],
        }
        write_document(document, args.output)
        return 0
    except (DiagnosticError, linked.MeasurementError, checker.ProfileError) as exc:
        document = {
            "schema": SCHEMA,
            "availability": "unavailable",
            "evidence_class": "source-derived-selected-commit-artifact",
            "release_declaration": False,
            "deployment_evidence": False,
            "profiles": [],
            "refusals": [str(exc)],
        }
        write_document(document, args.output)
        print(f"REFUSE: {exc}", file=sys.stderr)
        return 2
    finally:
        if work is not None and work_created and not args.keep_workdir:
            shutil.rmtree(work)


if __name__ == "__main__":
    raise SystemExit(main())
