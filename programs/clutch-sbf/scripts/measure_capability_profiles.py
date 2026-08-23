#!/usr/bin/env python3
"""Build each deployable capability profile twice and emit exact evidence.

The output is a release-manifest input, not a release declaration.  It binds
one capability identity to one reproducible SBF ELF hash, executable-text
size, and the exact loader-v3 rent model used by this repository.
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


LAMPORTS_PER_SOL = 1_000_000_000
RENT_LAMPORTS_PER_BYTE = 6_960
PROGRAM_ACCOUNT_BYTES = 164
PROGRAMDATA_METADATA_BYTES = 173
TEN_SOL_LAMPORTS = 10 * LAMPORTS_PER_SOL
MAX_ELF_BYTES_UNDER_TEN_SOL = (
    TEN_SOL_LAMPORTS // RENT_LAMPORTS_PER_BYTE
    - PROGRAM_ACCOUNT_BYTES
    - PROGRAMDATA_METADATA_BYTES
)

COMMON_TAGS = [*range(1, 6), 7, *range(10, 22), 68, *range(70, 74)]

PROFILES: tuple[dict[str, Any], ...] = (
    {
        "name": "full",
        "feature": "profile-full",
        "label": "dragons-clutch/capability-profile/full/v1",
        "profile_id": "f2066613610b8e3cff18485d2e6f3e3c9fdcfcbb757b46b407733ea15c5e9ac8",
        "enabled_intent_tags": list(range(1, 74)),
        "source_generations": ["v1", "v2"],
        "clearing_families": ["direct-v2", "direct-v3", "general"],
        "resolution_modes": ["categorical", "derived-point", "occupation"],
        "artifact_kinds": [1, 2, 3, 4, 5],
    },
    {
        "name": "direct-v3-source-v2-point",
        "feature": "profile-direct-v3-source-v2-point",
        "label": "dragons-clutch/capability-profile/direct-v3-source-v2-point/v1",
        "profile_id": "b735872284691ced6a7129e458833e2121793009bad12c45e6cbaa3c886e7897",
        "enabled_intent_tags": sorted([*COMMON_TAGS, *range(36, 47)]),
        "source_generations": ["v2"],
        "clearing_families": ["direct-v3"],
        "resolution_modes": ["categorical", "derived-point"],
        "artifact_kinds": [1, 2, 3, 5],
    },
    {
        "name": "general-source-v2-point",
        "feature": "profile-general-source-v2-point",
        "label": "dragons-clutch/capability-profile/general-source-v2-point/v1",
        "profile_id": "1f9e2f274c09a830145060efe1709128780a1272c083c7c2254f353aa78bf820",
        "enabled_intent_tags": sorted([*COMMON_TAGS, 8, 9, *range(47, 68), 69]),
        "source_generations": ["v2"],
        "clearing_families": ["general"],
        "resolution_modes": ["categorical", "derived-point"],
        "artifact_kinds": [1, 2, 3, 4],
    },
)

SOURCE_CLOSURE = (
    "crates",
    "programs/clutch-sbf",
    "programs/solana-layout",
    "programs/solana-reference",
    "research/batch-policy-identity",
)


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    if completed.returncode != 0:
        sys.stderr.write(completed.stdout)
        raise SystemExit(f"command failed ({completed.returncode}): {' '.join(command)}")
    return completed.stdout.strip()


def version(command: list[str], *, cwd: Path) -> str:
    return run(command, cwd=cwd).splitlines()[0]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def executable_text_bytes(readobj: Path, elf: Path, repo: Path) -> int:
    sections = run([str(readobj), "--sections", str(elf)], cwd=repo)
    match = re.search(
        r"Name: \.text \(\d+\).*?\n\s*Size: (\d+)", sections, flags=re.DOTALL
    )
    if match is None:
        raise SystemExit(f"could not locate decimal .text size in {elf}")
    return int(match.group(1))


def measure_elf(readobj: Path, elf: Path, repo: Path, run_number: int) -> dict[str, Any]:
    elf_bytes = elf.stat().st_size
    programdata_rent = (
        elf_bytes + PROGRAMDATA_METADATA_BYTES
    ) * RENT_LAMPORTS_PER_BYTE
    program_rent = PROGRAM_ACCOUNT_BYTES * RENT_LAMPORTS_PER_BYTE
    total_rent = programdata_rent + program_rent
    return {
        "run": run_number,
        "elf_sha256": sha256_file(elf),
        "elf_bytes": elf_bytes,
        "text_bytes": executable_text_bytes(readobj, elf, repo),
        "program_account_rent_lamports": program_rent,
        "programdata_account_rent_lamports": programdata_rent,
        "total_loader_rent_lamports": total_rent,
        "total_loader_rent_sol": f"{total_rent / LAMPORTS_PER_SOL:.9f}",
        "fits_under_ten_sol": total_rent <= TEN_SOL_LAMPORTS,
        "elf_headroom_bytes_under_ten_sol": MAX_ELF_BYTES_UNDER_TEN_SOL - elf_bytes,
    }


def build_once(
    *,
    repo: Path,
    cargo_build_sbf: Path,
    readobj: Path,
    profile: dict[str, Any],
    run_number: int,
    keep_workdirs: bool,
) -> tuple[dict[str, Any], str | None]:
    work = Path(tempfile.mkdtemp(prefix=f"dragons-clutch-{profile['name']}-run{run_number}-"))
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
        "v1.53",
        "--no-default-features",
        "--features",
        f"custom-heap,{profile['feature']}",
        "--sbf-out-dir",
        str(output),
        "--",
        "--locked",
    ]
    print(f"building {profile['name']} run {run_number}/2", file=sys.stderr, flush=True)
    try:
        run(command, cwd=repo, env=env)
        measurement = measure_elf(readobj, output / "clutch_sbf.so", repo, run_number)
        return measurement, str(work) if keep_workdirs else None
    finally:
        if not keep_workdirs:
            shutil.rmtree(work)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="write JSON here instead of stdout")
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="measure a dirty source closure, marking the manifest non-release-ready",
    )
    parser.add_argument("--keep-workdirs", action="store_true")
    parser.add_argument(
        "--profile",
        action="append",
        choices=[profile["name"] for profile in PROFILES],
        help="measure only this profile (repeatable; default: all)",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    repo = Path(__file__).resolve().parents[3]
    cargo_build_sbf = Path(
        os.environ.get(
            "CARGO_BUILD_SBF",
            str(Path.home() / ".local/share/solana/install/active_release/bin/cargo-build-sbf"),
        )
    )
    platform = Path.home() / ".cache/solana/v1.53/platform-tools"
    readobj = platform / "llvm/bin/llvm-readobj"
    rustc = platform / "rust/bin/rustc"
    for tool in (cargo_build_sbf, readobj, rustc):
        if not tool.is_file():
            raise SystemExit(f"required pinned tool is missing: {tool}")

    dirty = run(
        ["git", "status", "--porcelain", "--untracked-files=no", "--", *SOURCE_CLOSURE],
        cwd=repo,
    ).splitlines()
    if dirty and not args.allow_dirty:
        raise SystemExit(
            "tracked capability-profile source closure is dirty; commit it or pass --allow-dirty"
        )

    selected = [
        profile for profile in PROFILES if args.profile is None or profile["name"] in args.profile
    ]
    results: list[dict[str, Any]] = []
    for profile in selected:
        expected_id = hashlib.sha256(profile["label"].encode()).hexdigest()
        if expected_id != profile["profile_id"]:
            raise SystemExit(f"profile label/id mismatch: {profile['name']}")
        first, first_work = build_once(
            repo=repo,
            cargo_build_sbf=cargo_build_sbf,
            readobj=readobj,
            profile=profile,
            run_number=1,
            keep_workdirs=args.keep_workdirs,
        )
        second, second_work = build_once(
            repo=repo,
            cargo_build_sbf=cargo_build_sbf,
            readobj=readobj,
            profile=profile,
            run_number=2,
            keep_workdirs=args.keep_workdirs,
        )
        comparable = ("elf_sha256", "elf_bytes", "text_bytes")
        reproducible = all(first[key] == second[key] for key in comparable)
        if not reproducible:
            raise SystemExit(f"non-reproducible profile build: {profile['name']}")
        enabled = profile["enabled_intent_tags"]
        results.append(
            {
                **{key: value for key, value in profile.items() if key != "enabled_intent_tags"},
                "cargo_features": ["custom-heap", profile["feature"]],
                "enabled_intent_tags": enabled,
                "disabled_canonical_intent_tags": [
                    tag for tag in range(1, 74) if tag not in enabled
                ],
                "deterministic_disabled_tag_refusal": "UnsupportedInstruction before account reads",
                "reproducible": reproducible,
                "measurements": [first, second],
                "retained_workdirs": [path for path in (first_work, second_work) if path],
            }
        )

    commit = version(["git", "rev-parse", "HEAD"], cwd=repo)
    tree = version(["git", "rev-parse", "HEAD^{tree}"], cwd=repo)
    document = {
        "schema": "dragons-clutch/capability-profile-measurement/v1",
        "release_ready": not dirty,
        "source": {
            "git_commit": commit,
            "git_tree": tree,
            "closure_paths": list(SOURCE_CLOSURE),
            "tracked_dirty": dirty,
        },
        "builder": {
            "cargo_build_sbf": version([str(cargo_build_sbf), "--version"], cwd=repo),
            "platform_tools": "v1.53",
            "rustc": version([str(rustc), "--version"], cwd=repo),
            "llvm_readobj": version([str(readobj), "--version"], cwd=repo),
            "cargo_profile": "release",
            "lto": "fat",
            "codegen_units": 1,
            "overflow_checks": True,
        },
        "rent_model": {
            "model": "upgradeable-loader-v3-program-plus-programdata",
            "rent_lamports_per_byte": RENT_LAMPORTS_PER_BYTE,
            "program_account_bytes": PROGRAM_ACCOUNT_BYTES,
            "programdata_metadata_bytes": PROGRAMDATA_METADATA_BYTES,
            "max_elf_bytes_under_ten_sol": MAX_ELF_BYTES_UNDER_TEN_SOL,
        },
        "profiles": results,
    }
    encoded = json.dumps(document, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded)
    else:
        sys.stdout.write(encoded)


if __name__ == "__main__":
    main()
