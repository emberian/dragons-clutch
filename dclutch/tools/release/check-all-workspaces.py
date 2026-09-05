#!/usr/bin/env python3
"""Check every tracked Cargo workspace from one exact archived revision.

The root workspace is not an inventory of this repository: program-test,
fixture, generator, and operator tools deliberately carry independent
``[workspace]`` tables.  This release gate discovers those roots from the
archived source itself, gives each one a fresh target directory, and runs the
same locked/offline check over all targets.  It also proves that no Cargo
invocation added, removed, or changed any lockfile in the archive.

This is local evidence only.  It does not sign, submit, publish, or contact a
cluster.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import pathlib
import subprocess
import sys
import tarfile
import tomllib
from dataclasses import dataclass


SCHEMA = "dclutch-all-workspace-check-v1"


@dataclass(frozen=True)
class Workspace:
    manifest: str
    lock: str


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def regular_file(path: pathlib.Path, description: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"{description} is not one regular file: {path}")


def relative_files(root: pathlib.Path, name: str) -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for candidate in root.rglob(name):
        regular_file(candidate, name)
        files.append(candidate.relative_to(root))
    return sorted(files, key=lambda path: path.as_posix().encode())


def discover_workspaces(source: pathlib.Path) -> list[Workspace]:
    workspaces: list[Workspace] = []
    for relative in relative_files(source, "Cargo.toml"):
        manifest = source / relative
        try:
            parsed = tomllib.loads(manifest.read_text())
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            raise ValueError(f"cannot parse {relative.as_posix()}: {error}") from error
        if "workspace" not in parsed:
            continue
        lock_relative = relative.parent / "Cargo.lock"
        lock = source / lock_relative
        regular_file(lock, f"workspace lock for {relative.as_posix()}")
        workspaces.append(
            Workspace(manifest=relative.as_posix(), lock=lock_relative.as_posix())
        )
    if not workspaces:
        raise ValueError("archived source contains no Cargo workspace roots")
    return workspaces


def lock_rows(source: pathlib.Path) -> list[tuple[str, str]]:
    return [
        (relative.as_posix(), sha256(source / relative))
        for relative in relative_files(source, "Cargo.lock")
    ]


def write_rows(path: pathlib.Path, rows: list[tuple[str, str]]) -> None:
    with path.open("w") as output:
        for relative, digest in rows:
            output.write(f"{relative}\t{digest}\n")


def resolve_revision(repo: pathlib.Path, revision: str) -> str:
    process = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "--verify", f"{revision}^{{commit}}"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if process.returncode != 0:
        raise ValueError(
            f"cannot resolve source revision {revision!r}: {process.stderr.strip()}"
        )
    return process.stdout.strip()


def archived_script(repo: pathlib.Path, revision: str) -> bytes:
    process = subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "show",
            f"{revision}:tools/release/check-all-workspaces.py",
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if process.returncode != 0:
        raise ValueError(
            "source revision does not contain tools/release/check-all-workspaces.py"
        )
    return process.stdout


def safe_extract_archive(repo: pathlib.Path, revision: str, source: pathlib.Path) -> None:
    archive = subprocess.Popen(
        ["git", "-C", str(repo), "archive", "--format=tar", revision],
        stdout=subprocess.PIPE,
    )
    assert archive.stdout is not None
    try:
        # Git archives only the exact tracked tree.  Still use the filter that
        # refuses absolute paths, traversal, links escaping the destination,
        # and special-device extraction on modern Python.
        with tarfile.open(fileobj=archive.stdout, mode="r|") as tar:
            tar.extractall(source, filter="data")
    finally:
        archive.stdout.close()
    status = archive.wait()
    if status != 0:
        raise ValueError(f"git archive failed with status {status}")


def write_workspace_inventory(path: pathlib.Path, workspaces: list[Workspace]) -> None:
    with path.open("w") as output:
        for index, workspace in enumerate(workspaces, start=1):
            output.write(f"{index}\t{workspace.manifest}\t{workspace.lock}\n")


# Workspaces whose members REFUSE to compile without an explicit feature
# selection, and the selections that satisfy them.
#
# This is not a skip list and it must never become one. The one entry it
# ever held was `tools/gauntlet/aot-cu`, whose twin crate refused a bare check
# with `select exactly one evaluator: --features aot | interpreted | null` --
# a deliberate `compile_error!` so that nobody links an evaluator by accident.
# Refusing a default is the RIGHT behaviour for a crate whose whole point is
# mutually exclusive evaluators, so the checker makes the choice the crate
# demands rather than the crate acquiring a default to keep a gate quiet. That
# workspace left with the direct-aot program on 2026-09-04; the table is empty
# and stays a table, because the rule outlives its first instance.
#
# Every listed selection is checked, not just the first: checking one
# evaluator would leave the others uncompiled while the row read PASS.
#
# A workspace that lands here without a stated reason is a workspace someone
# quieted. Keep the reason with the entry.
FEATURE_SELECTIONS: dict[str, tuple[str, ...]] = {}


def run_checks(
    source: pathlib.Path,
    work: pathlib.Path,
    workspaces: list[Workspace],
) -> list[tuple[Workspace, int]]:
    results: list[tuple[Workspace, int]] = []
    logs = work / "logs"
    targets = work / "targets"
    logs.mkdir()
    targets.mkdir()
    for index, workspace in enumerate(workspaces, start=1):
        log = logs / f"{index:03d}.log"
        target = targets / f"{index:03d}"
        selections = FEATURE_SELECTIONS.get(workspace.manifest, (None,))
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = str(target)
        environment["CARGO_TERM_COLOR"] = "never"
        returncode = 0
        with log.open("wb") as output:
            for selection in selections:
                command = [
                    "cargo",
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--locked",
                    "--offline",
                    "--manifest-path",
                    workspace.manifest,
                ]
                if selection is not None:
                    command += ["--features", selection]
                output.write(("command=" + " ".join(command) + "\n").encode())
                output.flush()
                process = subprocess.run(
                    command,
                    cwd=source,
                    env=environment,
                    check=False,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                )
                # Every listed selection runs and the FIRST failure is the
                # verdict; a later pass never overwrites an earlier failure.
                if process.returncode != 0 and returncode == 0:
                    returncode = process.returncode
        results.append((workspace, returncode))
        state = "PASS" if returncode == 0 else "FAIL"
        detail = "" if selections == (None,) else f" [{', '.join(selections)}]"
        print(f"{state} {index}/{len(workspaces)} {workspace.manifest}{detail}", flush=True)
    return results


def write_summary(
    path: pathlib.Path,
    revision: str,
    workspaces: list[Workspace],
    locks: list[tuple[str, str]],
    results: list[tuple[Workspace, int]],
) -> None:
    passed = sum(status == 0 for _workspace, status in results)
    with path.open("w") as output:
        output.write(f"format={SCHEMA}\n")
        output.write("evidence_level=local-archived-source-check\n")
        output.write("not_a_deployment=true\n")
        output.write(f"source_revision={revision}\n")
        output.write(f"workspace_count={len(workspaces)}\n")
        output.write(f"workspace_pass_count={passed}\n")
        output.write(f"workspace_fail_count={len(results) - passed}\n")
        output.write(f"cargo_lock_count={len(locks)}\n")
        output.write("cargo_lock_immutability=passed\n")
        for index, (workspace, status) in enumerate(results, start=1):
            output.write(
                f"workspace.{index}.status={'passed' if status == 0 else 'failed'}\n"
            )
            output.write(f"workspace.{index}.manifest={workspace.manifest}\n")
            output.write(f"workspace.{index}.lock={workspace.lock}\n")


def parse_args(arguments: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        default=None,
        help="source repository (default: repository containing this script)",
    )
    parser.add_argument(
        "--work",
        required=True,
        help="new absolute output directory; must not already exist",
    )
    parser.add_argument(
        "--commit",
        default="HEAD",
        help="exact source revision to archive (default: HEAD)",
    )
    parser.add_argument(
        "--inventory-only",
        action="store_true",
        help="archive and enumerate, but do not run Cargo (never an admitted release result)",
    )
    return parser.parse_args(arguments)


def main(arguments: list[str]) -> int:
    args = parse_args(arguments)
    script = pathlib.Path(__file__).resolve(strict=True)
    repo = pathlib.Path(args.repo).resolve() if args.repo else script.parents[2]
    work = pathlib.Path(args.work)
    if not work.is_absolute():
        raise ValueError("--work must be absolute")
    # A non-existing work root makes freshness structural: no prior target,
    # log, or summary can be mistaken for this run's evidence.
    if work.exists():
        raise ValueError(f"--work must not already exist: {work}")
    work.mkdir(parents=True)
    source = work / "source"
    source.mkdir()

    revision = resolve_revision(repo, args.commit)
    if script.read_bytes() != archived_script(repo, revision):
        raise ValueError(
            "invoke the all-workspace checker from the exact --commit source revision"
        )
    print(f"source_revision={revision}")
    safe_extract_archive(repo, revision, source)

    workspaces = discover_workspaces(source)
    before = lock_rows(source)
    write_workspace_inventory(work / "workspaces.tsv", workspaces)
    write_rows(work / "cargo-locks-before.tsv", before)
    print(f"workspace_count={len(workspaces)}")
    print(f"cargo_lock_count={len(before)}")

    if args.inventory_only:
        print("inventory_only=true")
        return 0

    results = run_checks(source, work, workspaces)
    after = lock_rows(source)
    write_rows(work / "cargo-locks-after.tsv", after)
    if before != after:
        raise ValueError("Cargo.lock set changed while checking all workspaces")
    write_summary(work / "SUMMARY.txt", revision, workspaces, before, results)
    failed = [workspace.manifest for workspace, status in results if status != 0]
    print(f"workspace_pass_count={len(workspaces) - len(failed)}")
    print(f"workspace_fail_count={len(failed)}")
    print(f"summary={work / 'SUMMARY.txt'}")
    if failed:
        print("failed workspaces:", file=sys.stderr)
        for manifest in failed:
            print(f"  {manifest}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(f"all-workspaces: {error}", file=sys.stderr)
        raise SystemExit(2) from error
