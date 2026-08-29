#!/usr/bin/env python3
"""Run the one-shot generated-output convergence after source freeze.

This tool has no RPC, signing, build, or deployment surface.  It owns only
workspace locks and generated repository projections.  ``--write`` refreshes
those projections in a fixed order and then runs the complete read-only check;
``--check`` only verifies them.  Both modes require one clean, exact HEAD so a
result cannot silently span two source revisions.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tomllib
from typing import Iterable


REPO = Path(__file__).resolve().parents[2]
PACKAGE_ROOTS = (Path("packages/dclutch-sdk"), Path("apps/dclutch-web"))
GENERATED_PREFIXES = (
    "packages/dclutch-sdk/lib/generated/",
    "apps/dclutch-web/lib/generated/",
    "docs/reference/",
)
GENERATED_FILES = {"tools/sbom/SBOM.md", "tools/sbom/NOTICES.md"}


class Refusal(RuntimeError):
    pass


def command_text(command: Iterable[str]) -> str:
    return " ".join(shlex.quote(part) for part in command)


def environment() -> dict[str, str]:
    value = os.environ.copy()
    value["CARGO_NET_OFFLINE"] = "true"
    value["CARGO_TERM_COLOR"] = "never"
    value["NO_COLOR"] = "1"
    value["npm_config_audit"] = "false"
    value["npm_config_fund"] = "false"
    return value


def run(
    command: list[str],
    *,
    cwd: Path = REPO,
    capture: bool = False,
    accepted: tuple[int, ...] = (0,),
) -> subprocess.CompletedProcess[str]:
    print(f"+ ({cwd.relative_to(REPO) if cwd != REPO else '.'}) {command_text(command)}")
    process = subprocess.run(
        command,
        cwd=cwd,
        env=environment(),
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
    )
    if process.returncode not in accepted:
        detail = ""
        if capture:
            detail = (process.stderr or process.stdout or "").strip()
            if detail:
                detail = "\n" + "\n".join(detail.splitlines()[-20:])
        raise Refusal(
            f"command exited {process.returncode}: {command_text(command)}{detail}"
        )
    return process


def git_output(*arguments: str) -> str:
    return run(["git", *arguments], capture=True).stdout


def exact_head(expected: str) -> str:
    head = git_output("rev-parse", "HEAD").strip()
    if len(expected) != 40 or any(character not in "0123456789abcdef" for character in expected):
        raise Refusal("--expected-head must be one lowercase full 40-hex commit")
    if head != expected:
        raise Refusal(f"HEAD changed: expected {expected}, found {head}")
    return head


def require_clean() -> None:
    status = git_output("status", "--porcelain=v1", "--untracked-files=all")
    if status:
        paths = "\n".join(status.splitlines()[:20])
        raise Refusal(f"source tree is not clean before convergence:\n{paths}")


def tracked_paths(pattern: str) -> list[str]:
    raw = run(
        ["git", "ls-files", "-z", "--", pattern], capture=True
    ).stdout
    return sorted(path for path in raw.split("\0") if path)


def discover_workspaces(
    root: Path, manifests: Iterable[str], tracked_locks: set[str]
) -> list[tuple[str, str]]:
    workspaces: list[tuple[str, str]] = []
    for relative in sorted(manifests):
        manifest = root / relative
        try:
            parsed = tomllib.loads(manifest.read_text())
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            raise Refusal(f"cannot parse {relative}: {error}") from error
        if "workspace" not in parsed:
            continue
        lock = (Path(relative).parent / "Cargo.lock").as_posix()
        if lock not in tracked_locks:
            raise Refusal(f"workspace {relative} lacks its tracked adjacent {lock}")
        candidate = root / lock
        if candidate.is_symlink() or not candidate.is_file():
            raise Refusal(f"workspace lock is not one regular file: {lock}")
        workspaces.append((relative, lock))
    if not workspaces:
        raise Refusal("no tracked Cargo workspaces discovered")
    return workspaces


def cargo_metadata(manifest: str, *, locked: bool) -> subprocess.CompletedProcess[str]:
    command = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--offline",
        "--manifest-path",
        manifest,
    ]
    if locked:
        command.insert(4, "--locked")
    return run(command, capture=True, accepted=(0, 1, 101))


def converge_locks(workspaces: list[tuple[str, str]], *, write: bool) -> None:
    stale: list[tuple[str, str]] = []
    for index, (manifest, lock) in enumerate(workspaces, start=1):
        process = cargo_metadata(manifest, locked=True)
        if process.returncode == 0:
            print(f"LOCK PASS {index}/{len(workspaces)} {manifest}")
            continue
        stale.append((manifest, lock))
        print(f"LOCK STALE {index}/{len(workspaces)} {manifest}")
    if stale and not write:
        rendered = "\n".join(f"  {manifest} -> {lock}" for manifest, lock in stale)
        raise Refusal(f"{len(stale)} workspace lock(s) are stale:\n{rendered}")
    for index, (manifest, _lock) in enumerate(stale, start=1):
        process = cargo_metadata(manifest, locked=False)
        if process.returncode != 0:
            detail = (process.stderr or process.stdout or "").strip()
            detail = "\n".join(detail.splitlines()[-20:])
            raise Refusal(
                f"workspace lock refresh refused {index}/{len(stale)} {manifest}:\n{detail}"
            )
        print(f"LOCK UPDATED {index}/{len(stale)} {manifest}")
    if stale:
        converge_locks(workspaces, write=False)


def abi_tasks(root: Path = REPO) -> tuple[list[tuple[Path, str]], list[tuple[Path, str]]]:
    writers: list[tuple[Path, str]] = []
    verifiers: list[tuple[Path, str]] = []
    for relative in PACKAGE_ROOTS:
        package = root / relative / "package.json"
        try:
            scripts = json.loads(package.read_text()).get("scripts", {})
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise Refusal(f"cannot parse {package.relative_to(root)}: {error}") from error
        if not isinstance(scripts, dict):
            raise Refusal(f"package scripts are not an object: {package.relative_to(root)}")
        abi = {
            name
            for name, command in scripts.items()
            if isinstance(name, str)
            and isinstance(command, str)
            and name.startswith("abi:")
            and name != "abi:coverage"
        }
        package_writers = sorted(name for name in abi if not name.endswith(":verify"))
        package_verifiers = sorted(name for name in abi if name.endswith(":verify"))
        expected_verifiers = {f"{name}:verify" for name in package_writers}
        if set(package_verifiers) != expected_verifiers:
            missing = sorted(expected_verifiers - set(package_verifiers))
            orphan = sorted(set(package_verifiers) - expected_verifiers)
            raise Refusal(
                f"ABI writer/verifier mismatch in {relative}: missing={missing}, orphan={orphan}"
            )
        writers.extend((relative, name) for name in package_writers)
        verifiers.extend((relative, name) for name in package_verifiers)
    return writers, verifiers


def run_abi(*, write: bool) -> None:
    writers, verifiers = abi_tasks()
    tasks = writers if write else verifiers
    for relative, script in tasks:
        run(["npm", "run", "--silent", script], cwd=REPO / relative)
    for relative in PACKAGE_ROOTS:
        try:
            run(["npm", "run", "--silent", "abi:coverage"], cwd=REPO / relative)
        except Refusal as error:
            raise Refusal(
                f"ABI coverage ratchet refused in {relative}; convert the hand-stated "
                "surface or obtain an explicit review before updating its baseline"
            ) from error


def run_genref(*, write: bool) -> None:
    command = ["tools/genref/generate.sh"]
    if not write:
        command.append("--check")
    run(command)


def run_sbom(*, write: bool) -> None:
    command = ["python3", "tools/sbom/sbom_check.py"]
    if not write:
        command.append("--verify")
    run(command)


def allowed_output(path: str, workspace_locks: set[str]) -> bool:
    return (
        path in workspace_locks
        or path in GENERATED_FILES
        or path.startswith(GENERATED_PREFIXES)
    )


def changed_paths() -> list[str]:
    tracked = git_output("diff", "--name-only", "HEAD").splitlines()
    untracked = git_output("ls-files", "--others", "--exclude-standard").splitlines()
    return sorted(set(tracked + untracked))


def enforce_output_ownership(workspace_locks: set[str]) -> list[str]:
    changed = changed_paths()
    foreign = [
        path for path in changed if not allowed_output(path, workspace_locks)
    ]
    if foreign:
        raise Refusal(
            "generator changed path(s) outside release-owned outputs:\n  "
            + "\n  ".join(foreign)
        )
    return changed


def plan() -> None:
    manifests = tracked_paths("**/Cargo.toml") + tracked_paths("Cargo.toml")
    locks = set(tracked_paths("**/Cargo.lock") + tracked_paths("Cargo.lock"))
    workspaces = discover_workspaces(REPO, set(manifests), locks)
    writers, verifiers = abi_tasks()
    print("Final generated convergence plan (offline; no RPC/signing/deployment):")
    print(f"  Cargo: {len(workspaces)} workspace owners; {len(locks)} tracked locks")
    print(f"  ABI:   {len(writers)} writers; {len(verifiers)} byte verifiers; 2 coverage ratchets")
    print("  GENREF owner: docs/reference/**")
    print("  SBOM owners: tools/sbom/SBOM.md, tools/sbom/NOTICES.md")
    print("  Write order: locks -> ABI writers -> ABI coverage -> GENREF -> SBOM -> full check")
    print(
        "  Check order: locks -> ABI verifiers -> ABI coverage -> "
        "GENREF --check -> SBOM --verify"
    )


def converge(*, write: bool, expected_head: str) -> None:
    exact_head(expected_head)
    require_clean()
    manifests = set(tracked_paths("**/Cargo.toml") + tracked_paths("Cargo.toml"))
    locks = set(tracked_paths("**/Cargo.lock") + tracked_paths("Cargo.lock"))
    workspaces = discover_workspaces(REPO, manifests, locks)
    workspace_locks = {lock for _manifest, lock in workspaces}
    try:
        converge_locks(workspaces, write=write)
        exact_head(expected_head)
        run_abi(write=write)
        exact_head(expected_head)
        run_genref(write=write)
        exact_head(expected_head)
        run_sbom(write=write)
        exact_head(expected_head)
    finally:
        if write:
            # Even a failed generator is not allowed to leave a foreign source
            # edit behind. Allowed partial generated output remains an ordinary
            # visible git diff and can be rerun after the named refusal is fixed.
            enforce_output_ownership(workspace_locks)
    if write:
        converge_locks(workspaces, write=False)
        run_abi(write=False)
        run_genref(write=False)
        run_sbom(write=False)
        exact_head(expected_head)
        changed = enforce_output_ownership(workspace_locks)
        print(f"GENERATED CONVERGENCE WRITE PASS {expected_head}: {len(changed)} path(s)")
        for path in changed:
            print(f"  {path}")
    else:
        require_clean()
        print(f"GENERATED CONVERGENCE CHECK PASS {expected_head}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--plan", action="store_true", help="print owners/order only")
    mode.add_argument("--check", action="store_true", help="verify without writes")
    mode.add_argument("--write", action="store_true", help="regenerate then verify")
    parser.add_argument(
        "--expected-head",
        help="required full commit for --check/--write; refuses a moving source",
    )
    arguments = parser.parse_args()
    try:
        if arguments.plan:
            if arguments.expected_head:
                raise Refusal("--expected-head is not used with --plan")
            plan()
        else:
            if not arguments.expected_head:
                raise Refusal("--expected-head is required with --check/--write")
            converge(write=arguments.write, expected_head=arguments.expected_head)
        return 0
    except (OSError, Refusal) as error:
        print(f"FINAL GENERATED CONVERGENCE REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
