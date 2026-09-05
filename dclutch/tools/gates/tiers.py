"""The tier table: which gate runs when, what it costs, what it refuses.

One table, three readers: `tools/gate --list`, the dispatcher, and the README's
pointer to `--list`. Nothing restates it.

A tier is `run(ctx) -> (code, detail)`; raising Prereq or Failed is the same
answer. `ctx.commit` asks for a clean export of that revision wherever the tier
compiles -- on a shared checkout a working-tree build measures a revision
nobody committed. `ctx.dry_run` prints the commands instead of running them.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tempfile
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from .common import (
    EXIT_FAIL, EXIT_PASS, EXIT_PREREQ, FRAME_DIAGNOSTIC, GATES, REPO, Context, Failed, Prereq,
    archived, checked_out, dirty, have, measured_tree, note, read_tsv, scratch, sh,
)


def _run(ctx: Context, args, **kwargs) -> subprocess.CompletedProcess:
    if ctx.dry_run:
        cwd = kwargs.get("cwd")
        env = {k: v for k, v in (kwargs.get("env") or {}).items() if k not in os.environ or os.environ[k] != v}
        note("$ " + (f"cd {cwd} && " if cwd else "") + " ".join(f"{k}={v}" for k, v in env.items())
             + (" " if env else "") + " ".join(str(a) for a in args))
        return subprocess.CompletedProcess(args, 0, "", "")
    return sh(args, **kwargs)


def _env(**extra: str) -> dict:
    env = dict(os.environ)
    env.update(extra)
    return env


# --------------------------------------------------------------------------- instruments as tiers

def tier_census(ctx: Context):
    from . import census
    return census.run(commit=ctx.commit, dry_run=ctx.dry_run)


def tier_emission(ctx: Context):
    from . import emission
    return emission.cheap(dry_run=ctx.dry_run)


def tier_guards(ctx: Context):
    from . import emission
    scope = os.environ.get("DCLUTCH_GATE_RANGE")
    return emission.verdict(range_=scope, dry_run=ctx.dry_run)


def tier_frames(ctx: Context):
    from . import frames
    return frames.tier(ctx)


def tier_reference(ctx: Context):
    from . import reference
    return reference.tier(ctx)


def tier_commands(ctx: Context):
    from . import commands
    if ctx.dry_run:
        note("$ tools/gate commands --check")
        return EXIT_PASS, ""
    code = commands.main(["--root", str(REPO), "--check"])
    return code, {EXIT_PREREQ: "a published command was not probed", EXIT_FAIL: "a runbook publishes a command a reader cannot run"}.get(code, "")


def tier_budgets(ctx: Context):
    from . import budgets
    return budgets.check(dry_run=ctx.dry_run)


def tier_witness(ctx: Context):
    from . import witness
    if ctx.dry_run:
        note("$ tools/gate witness --check")
        return EXIT_PASS, ""
    code = witness.main(["--check"])
    return code, "a devnet witness does not corroborate" if code else ""


def tier_selftest(ctx: Context):
    from . import selftest
    return selftest.run(dry_run=ctx.dry_run)


# --------------------------------------------------------------------------- fmt

def tier_fmt(ctx: Context):
    baseline = GATES / "fmt-baseline.txt"
    if not baseline.is_file():
        raise Prereq("tools/gates/fmt-baseline.txt is absent")
    if not (REPO / "rustfmt.toml").is_file():
        raise Prereq("rustfmt.toml is absent, so 'formatted' has no single answer")
    if not have("cargo"):
        raise Prereq("cargo is not on PATH")
    if sh(["cargo", "fmt", "--version"], cwd=REPO, quiet=True).returncode != 0:
        raise Prereq("the rustfmt component is not installed (rustup component add rustfmt)")
    count = dirty("*.rs")
    if count:
        note(f"{count} uncommitted .rs file(s): a finding below may be a neighbouring lane's to format")
    result = _run(ctx, ["cargo", "fmt", "--all", "--check"], cwd=REPO, capture=True)
    # ONE FILE, ONE ROW. A `#[path = "../.."]` link makes rustfmt name the same
    # source under every package that compiles it: since the 2026-09-05 fold put
    # the three gauntlet campaigns in the same workspace as the successor they
    # link, `successor/src/upgrade.rs` arrives here four times under four
    # spellings. A baseline cannot be stable against that, so the path is
    # normalised before it is compared -- and a baseline row is written the one
    # way the file actually lives on disk.
    found = sorted({
        os.path.normpath(re.sub(r":\d+:$", "", line[len("Diff in "):])).removeprefix(str(REPO) + "/")
        for line in result.stdout.splitlines() if line.startswith("Diff in ")
    })
    expected = sorted({fields[0] for _, fields in read_tsv(baseline, 1)})
    new = sorted(set(found) - set(expected))
    gone = sorted(set(expected) - set(found))
    if new:
        note("rustfmt disagrees with files NOT in the baseline (format them: tools/lane.sh fmt <file>):")
        for path in new:
            note(f"  {path}")
        return EXIT_FAIL, f"{len(new)} unformatted file(s) outside the baseline"
    if gone:
        note("baseline lines that are no longer true (delete them):")
        for path in gone:
            note(f"  {path}")
        return EXIT_FAIL, f"{len(gone)} stale baseline line(s)"
    note(f"{len(expected)} file(s) still owed, each named in the baseline with its lane")
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- locks

def tier_locks(ctx: Context):
    if not have("cargo"):
        raise Prereq("cargo is not on PATH")
    stale, unproven, other, checked = [], [], [], 0
    env = {k: v for k, v in os.environ.items() if k != "CARGO_TARGET_DIR"}
    for lock in sorted(REPO.rglob("Cargo.lock")):
        if "target" in lock.parts or not (lock.parent / "Cargo.toml").is_file():
            continue
        checked += 1
        relative = str(lock.parent.relative_to(REPO))
        result = _run(ctx, ["cargo", "metadata", "--locked", "--offline", "--format-version", "1",
                            "--manifest-path", lock.parent / "Cargo.toml"], cwd=REPO, env=env, capture=True)
        if result.returncode == 0:
            continue
        if "because --locked was passed to prevent this" in result.stderr:
            stale.append(relative)
        elif "offline mode (via " in result.stderr:
            unproven.append(relative)
        else:
            other.append(f"{relative}\t{(result.stderr.strip().splitlines() or [''])[0]}")
    if unproven:
        note(f"{len(unproven)} of {checked} workspace(s) could not be resolved OFFLINE (registry cache lacks a package); their locks were NOT checked:")
        for row in unproven:
            note(f"  {row}")
        note("populate it: for m in $(git ls-files '*Cargo.toml'); do cargo fetch --locked --manifest-path \"$m\"; done")
    for row in other:
        note(f"  refused for another reason: {row}")
    if stale:
        note("lockfiles that no longer resolve (a member resolves through its root, so '.' may be the cause of the rest):")
        for row in stale:
            note(f"  {row}")
    if stale or other:
        return EXIT_FAIL, f"{len(stale) + len(other)} workspace lockfile(s) do not resolve under --locked"
    if unproven:
        return EXIT_PREREQ, f"{len(unproven)} of {checked} workspace(s) have no local registry cache to resolve against"
    note(f"{checked} workspace lockfile(s) resolved")
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- seam

def tier_seam(ctx: Context):
    tool = REPO / "tools" / "seam-audit" / "seam_audit.py"
    if not tool.is_file():
        raise Prereq("tools/seam-audit is absent")
    code = _run(ctx, ["python3", tool], cwd=REPO).returncode
    if code == EXIT_PREREQ:
        return code, "the seam checker could not run (usually: no ast-grep on PATH)"
    if code:
        return EXIT_FAIL, "new seam findings against the committed baseline"
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- release

RELEASE_SHELL = (
    "test-checked-release-freshness.sh",
    "test-devnet-activity.sh",
    "test-stage-devnet-sponsored-market-open.sh",
)
RELEASE_PYTHON = (
    "test_usage_parity.py",
    "test_artifact_provenance.py",
    "test_devnet_direct_lifecycle.py",
    "test_node_archive_members.py",
    "test_public_route_campaign.py",
    "test_successor_campaign_pack.py",
    "test-check-all-workspaces.py",
    "test-compose-mixed-gate.py",
    "test-final-generated-convergence.py",
    "test-plan-sbf-release-batch.py",
)
RELEASE_ROOT_PYTHON = ("tools/devnet-reconcile/tests/test_reconcile.py",)


def tier_release(ctx: Context):
    directory = REPO / "tools" / "release"
    present = [name for name in RELEASE_SHELL if (directory / name).is_file()]
    if not present:
        raise Prereq("tools/release test scripts are absent")
    failed, missing = [], [name for name in RELEASE_SHELL if (directory / name) .is_file() is False]
    for name in present:
        if _run(ctx, ["bash", directory / name], cwd=REPO).returncode:
            failed.append(name)
    for suite in RELEASE_PYTHON:
        path = directory / suite
        if not path.is_file():
            missing.append(suite)
            continue
        note(f"python: {suite}")
        if _run(ctx, ["python3", path.name], cwd=path.parent, env=_env(PYTHONPATH=str(REPO))).returncode:
            failed.append(suite)
    for suite in RELEASE_ROOT_PYTHON:
        path = REPO / suite
        if not path.is_file():
            missing.append(suite)
            continue
        note(f"python: {suite}")
        if _run(ctx, ["python3", path.name], cwd=path.parent, env=_env(PYTHONPATH=str(REPO))).returncode:
            failed.append(suite)
    parity, successor = directory / "usage_parity.py", REPO / "tools/local-validator/bootstrap/successor/src"
    if parity.is_file() and successor.is_dir():
        note("usage/parser parity")
        code = _run(ctx, ["python3", parity, "--crate-src", successor], cwd=REPO).returncode
        if code == EXIT_PREREQ:
            missing.append("usage_parity.py could not run")
        elif code:
            failed.append("usage_parity.py")
    else:
        missing.append("usage_parity.py or the successor crate")
    if failed:
        note("release-tooling refusal suites that FAILED: " + ", ".join(failed))
        return EXIT_FAIL, f"{len(failed)} refusal suite(s) failed"
    if missing:
        note("absent and NOT run: " + ", ".join(missing))
        return EXIT_PREREQ, f"{len(missing)} release suite(s) absent from this tree"
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- clippy

CLIPPY_WARM_S = 22


def _workspace_members(root: Path) -> dict[str, str]:
    out = sh(["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"], cwd=root, capture=True)
    if out.returncode != 0:
        raise Prereq(f"cargo metadata failed: {out.stderr.strip()[:200]}")
    return {p["id"]: p["name"] for p in json.loads(out.stdout)["packages"]}


def _lint_optin(root: Path) -> tuple[int, int]:
    out = sh(["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"], cwd=root, capture=True)
    if out.returncode != 0:
        return 0, 0
    optin = total = 0
    for package in json.loads(out.stdout)["packages"]:
        total += 1
        try:
            lints = tomllib.loads(Path(package["manifest_path"]).read_text()).get("lints")
        except (OSError, tomllib.TOMLDecodeError):
            continue
        if isinstance(lints, dict) and lints.get("workspace") is True:
            optin += 1
    return optin, total


def clippy_census(stream: Path, debt_path: Path, root: Path) -> int:
    """Judge one `cargo clippy --message-format=json` stream against the debt register, per PACKAGE.

    `--keep-going` stops at a red library, so one red kernel hides every package
    above it: the never-reached set is printed as its own number.
    """
    debt = {}
    for _, fields in read_tsv(debt_path, 6):
        if fields[0] == "status":
            continue
        if fields[0] != "debt":
            raise Prereq(f"clippy-debt.tsv: unknown status {fields[0]!r}")
        debt[fields[1]] = fields[2]
    members = _workspace_members(root)
    checked, red, findings = set(), set(), {}
    for line in stream.read_text(errors="replace").splitlines():
        if not line.startswith("{"):
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        package = members.get(message.get("package_id", ""))
        if package is None:
            continue
        if message.get("reason") == "compiler-artifact":
            checked.add(package)
        if message.get("reason") != "compiler-message" or message["message"].get("level") != "error":
            continue
        red.add(package)
        body = message["message"]
        if "could not compile" in body.get("message", ""):
            continue
        code = (body.get("code") or {}).get("code") or "rustc"
        for span in body.get("spans", []):
            if span.get("is_primary"):
                findings.setdefault(package, set()).add((span["file_name"], span["line_start"], code))
    if not checked and not red:
        raise Prereq("the clippy stream names no workspace package; nothing was measured")
    names = set(members.values())
    unreached = sorted(names - checked - red)
    optin, total = _lint_optin(root)
    note(f"{len(names)} members: {len(checked - red)} clean, {len(red)} red, {len(unreached)} never reached")
    note(f"{optin}/{total} members inherit [workspace.lints]; the rest do not get the deny table at all")
    failed = False
    for package in sorted(red - set(debt)):
        failed = True
        note(f"RED and not in clippy-debt.tsv: {package}")
        for file_name, line, code in sorted(findings.get(package, ())):
            note(f"    {file_name}:{line}  {code.removeprefix('clippy::')}")
    for package in sorted(set(debt) - red):
        failed = True
        reached = "" if package in checked else " (never reached: not evidence it is clean)"
        note(f"DEBT IS STALE, delete its row: {package} [{debt[package]}]{reached}")
    for package in unreached:
        note(f"  never reached: {package}")
    return EXIT_FAIL if failed else EXIT_PASS


def tier_clippy(ctx: Context):
    if not have("cargo"):
        raise Prereq("cargo is not on PATH")
    if sh(["cargo", "clippy", "--version"], quiet=True).returncode != 0:
        raise Prereq("cargo-clippy is not installed (rustup component add clippy)")
    with measured_tree(ctx, "crates", "programs") as (root, _):
        debt = GATES / "clippy-debt.tsv"
        if not debt.is_file():
            raise Prereq("tools/gates/clippy-debt.tsv is absent")
        target = Path(os.environ.get("DCLUTCH_GATE_CLIPPY_TARGET", root / "target" / "clippy"))
        warm = target.is_dir()
        if not warm:
            note(f"target directory is COLD ({target}); the wall-clock backstop is skipped")
        args = ["cargo", "clippy", "--workspace", "--all-targets", "--keep-going", "--message-format=json", "--", "-D", "warnings"]
        env = _env(CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS=ctx.jobs)
        if ctx.dry_run:
            _run(ctx, args, cwd=root, env=env)
            return EXIT_PASS, ""
        with scratch("clippy") as tmp:
            stream = tmp / "stream.json"
            start = time.time()
            with open(stream, "w") as handle:
                subprocess.run(args, cwd=root, env=env, stdout=handle, stderr=subprocess.DEVNULL)
            elapsed = int(time.time() - start)
            code = clippy_census(stream, debt, root)
        slack = int(os.environ.get("DCLUTCH_GATE_TIME_SLACK", "4"))
        if warm:
            note(f"{elapsed}s (budget {CLIPPY_WARM_S * slack}s = {CLIPPY_WARM_S}s measured x {slack} slack)")
            if elapsed > CLIPPY_WARM_S * slack:
                note("over the LOOSE wall-clock backstop; on a loaded machine re-run before reporting it")
                code = EXIT_FAIL
        else:
            note(f"{elapsed}s (cold: not budgeted)")
    return code, ""


# --------------------------------------------------------------------------- sbom

def tier_sbom(ctx: Context):
    tool = REPO / "tools" / "sbom" / "sbom_check.py"
    if not tool.is_file():
        raise Prereq("tools/sbom/sbom_check.py is absent")
    if not have("cargo"):
        raise Prereq("cargo is not on PATH (the closure resolves every workspace with cargo metadata)")
    unit = REPO / "tools" / "sbom" / "test_sbom_check.py"
    if unit.is_file() and _run(ctx, ["python3", unit], cwd=unit.parent, quiet=True).returncode:
        return EXIT_FAIL, "sbom_check's own classification tests failed; the checker is suspect"
    if ctx.commit:
        # The closure discovers manifests with `git ls-files`, so an export will not do.
        with checked_out(ctx.commit) as (root, sha):
            note(f"measuring commit {sha} (clean detached worktree)")
            code = _run(ctx, ["python3", root / "tools/sbom/sbom_check.py", "--verify"], cwd=root, quiet=True).returncode
    else:
        note("measuring the WORKING TREE; an uncommitted lockfile makes this green only for whoever holds it")
        code = _run(ctx, ["python3", tool, "--verify"], cwd=REPO, quiet=True).returncode
    if code == 0:
        return EXIT_PASS, ""
    if code == 1:
        note("SBOM drift, or a dependency the tree cannot license-classify: python3 tools/sbom/sbom_check.py --verify")
        return EXIT_FAIL, ""
    return EXIT_PREREQ, f"sbom_check exited {code} (usually a workspace lock that cannot resolve offline)"


# --------------------------------------------------------------------------- sbfcontracts

def _sbf_reachable_crates(root: Path) -> list[str]:
    """Every non-program first-party crate a program's NORMAL dependencies reach, plus every root crate
    declaring `check-cfg = ['cfg(target_os, values("solana"))']`. Derived, never listed."""
    reach: set[str] = set()
    manifests = sorted(root.glob("programs/*/Cargo.toml"))
    if not manifests:
        raise Prereq("no program manifests under programs/")
    for manifest in manifests:
        result = sh(["cargo", "metadata", "--format-version", "1", "--manifest-path", manifest], capture=True)
        if result.returncode != 0:
            raise Prereq(f"cargo metadata failed for {manifest.relative_to(root)}")
        md = json.loads(result.stdout)
        packages = {p["id"]: p for p in md["packages"]}
        nodes = {n["id"]: n for n in md["resolve"]["nodes"]}
        start = md["resolve"].get("root")
        stack = [start] if start else [p["id"] for p in md["packages"] if p["manifest_path"] == str(manifest)]
        seen: set[str] = set()
        while stack:
            current = stack.pop()
            if current in seen or current not in nodes:
                continue
            seen.add(current)
            for dep in nodes[current]["deps"]:
                if any(kind.get("kind") is None for kind in dep.get("dep_kinds", [])):
                    stack.append(dep["pkg"])
        for identifier in seen:
            package = packages.get(identifier)
            if not package:
                continue
            path = package.get("manifest_path", "")
            if path.startswith(str(root)) and "/programs/" not in path and package["name"].startswith("dclutch-"):
                reach.add(package["name"])
    result = sh(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=root, capture=True)
    if result.returncode == 0:
        for package in json.loads(result.stdout)["packages"]:
            manifest = Path(package["manifest_path"])
            if "/programs/" in str(manifest) or not package["name"].startswith("dclutch-"):
                continue
            try:
                if 'cfg(target_os, values("solana"))' in manifest.read_text():
                    reach.add(package["name"])
            except OSError:
                pass
    if not reach:
        raise Prereq("could not derive the SBF-reachable crate set; a gate over zero crates is not green")
    return sorted(reach)


def tier_sbfcontracts(ctx: Context):
    if not have("cargo-build-sbf"):
        raise Prereq("cargo-build-sbf is not on PATH")
    with measured_tree(ctx) as (root, _):
        packages = _sbf_reachable_crates(root)
        note(f"{len(packages)} crates: the programs' normal-dependency closure plus every solana-target-aware root crate")
        toolchains = sh(["rustup", "toolchain", "list"], capture=True).stdout.split()
        sbf = next((t for t in toolchains if "sbpf-solana" in t), None)
        if sbf is None:
            raise Prereq("no sbpf-solana rustup toolchain; run any `cargo build-sbf` once to provision it")
        note(f"toolchain {sbf}, target sbpf-solana-solana")
        args = ["cargo", f"+{sbf}", "check", "--locked", "--offline", "--target", "sbpf-solana-solana"]
        for name in packages:
            args += ["-p", name]
        if _run(ctx, args, cwd=root).returncode:
            note("a crate that ships to SBF does not compile for target_os=solana; look for a `#[cfg(not(target_os = \"solana\"))]` hiding a used surface. Do NOT narrow this gate to the crates that pass.")
            return EXIT_FAIL, f"{len(packages)} crates requested; the SBF check failed"
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- web / abi

CLIENT_TREES = ("apps/dclutch-web", "packages/dclutch-sdk")


# The cohort-liveness suite, in the tree that owns it. ONE author: the web
# imports `@dclutch/sdk`, so the SDK's answer about the shipped cohort is the
# web's answer too, and the web's copy went with the twins (`dcaba4770`). The
# tier kept asking both trees for it and read "no test files found" — exit 1 —
# as a dead cohort, which is the `twins` tier's red one shape over: a check
# whose subject was deleted reports on the checker, not on the protocol.
LIVENESS_SUITE = "lib/deploymentLiveness.live.test.ts"


def tier_web(ctx: Context):
    if not have("npx"):
        raise Prereq("node/npx is not on PATH")
    failed = ran = 0
    for tree in CLIENT_TREES:
        full = REPO / tree
        if not (full / "node_modules").is_dir():
            note(f"{tree}: node_modules absent (npm ci)")
            continue
        ran += 1
        note(tree)
        if _run(ctx, ["npm", "run", "--silent", "lint"], cwd=full).returncode:
            failed += 1
        if _run(ctx, ["npx", "vitest", "run", "--config", "vitest.config.ts",
                      "--exclude", "lib/abiVerification.test.ts", "--exclude", "lib/sbomVerify.test.ts"], cwd=full).returncode:
            failed += 1
    # The cohort-liveness gate, with its positive control: a cluster that does
    # not answer is NEVER RAN, not red. Asked once, of the trees that carry the
    # suite -- and a tree that carries none is said out loud rather than run.
    carriers = [tree for tree in CLIENT_TREES
                if (REPO / tree / "node_modules").is_dir() and (REPO / tree / LIVENESS_SUITE).is_file()]
    for tree in CLIENT_TREES:
        if tree not in carriers:
            note(f"{tree}: carries no {LIVENESS_SUITE}; the cohort-liveness question has one author")
    if ran and not carriers:
        # Not a skip. The tier's own headline is "a shipped cohort that does not
        # answer", and with the suite gone from every tree it would pass while
        # asking nothing.
        return EXIT_FAIL, f"no client tree carries {LIVENESS_SUITE}; the cohort-liveness gate has no subject"
    if carriers:
        health = _run(ctx, ["curl", "-fsS", "-m", "10", "-X", "POST", "-H", "content-type: application/json",
                            "-d", '{"jsonrpc":"2.0","id":1,"method":"getHealth"}', "https://api.devnet.solana.com"], capture=True)
        if '"result":"ok"' in (health.stdout or "") or ctx.dry_run:
            for tree in carriers:
                if _run(ctx, ["npx", "vitest", "run", "--config", "vitest.config.ts", LIVENESS_SUITE],
                        cwd=REPO / tree, env=_env(DCLUTCH_LIVE_DEVNET="1")).returncode:
                    note(f"{tree}: THE SHIPPED COHORT DID NOT ANSWER")
                    failed += 1
        else:
            note("devnet did not answer getHealth; the cohort-liveness gate NEVER RAN")
    if ran == 0:
        return EXIT_PREREQ, "no client tree had its dependencies installed"
    return (EXIT_FAIL, f"{failed} check(s) failed") if failed else (EXIT_PASS, "")


def tier_abi(ctx: Context):
    missing = [name for name, ok in (("node/npx", have("npx")), ("lake", have("lake")), ("cargo", have("cargo")),
                                       ("wasm-bindgen", have("wasm-bindgen"))) if not ok]
    if have("rustup") and "wasm32-unknown-unknown" not in sh(["rustup", "target", "list", "--installed"], capture=True).stdout:
        missing.append("the wasm32-unknown-unknown target")
    if missing:
        raise Prereq(", ".join(missing) + " not available; the verifiers refuse rather than skip without a toolchain")
    owned = "DCLUTCH_WASM_TARGET_DIR" not in os.environ
    target = os.environ.get("DCLUTCH_WASM_TARGET_DIR") or tempfile.mkdtemp(prefix="dclutch-abi-wasm.")
    note(f"wasm target directory: {target}")
    failed = ran = 0
    try:
        for tree in CLIENT_TREES:
            full = REPO / tree
            if not (full / "node_modules").is_dir():
                note(f"{tree}: node_modules absent (npm ci)")
                continue
            ran += 1
            note(tree)
            if _run(ctx, ["npx", "vitest", "run", "--config", "vitest.config.ts", "lib/abiVerification.test.ts"],
                    cwd=full, env=_env(DCLUTCH_WASM_TARGET_DIR=target)).returncode:
                failed += 1
    finally:
        if owned:
            shutil.rmtree(target, ignore_errors=True)
    if ran == 0:
        return EXIT_PREREQ, "no client tree had its dependencies installed"
    if failed:
        note("a generated client module drifted from the Rust or Lean that printed it. Regenerate from a DETACHED WORKTREE at HEAD, then carry the SDK twin (packages/dclutch-sdk/scripts/sync-from-web.mjs --copy --only).")
        return EXIT_FAIL, "a generated client module drifted from its authority"
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- journey

def _tool_packages(ctx: Context, root: Path) -> list[tuple[str, str]]:
    """Every first-party package under `tools/`, as (package name, directory)."""
    result = _run(ctx, ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1"],
                  cwd=root, capture=True)
    if result.returncode:
        raise Prereq("cargo metadata could not enumerate the workspace's tools/ packages")
    found = []
    for package in json.loads(result.stdout)["packages"]:
        directory = Path(package["manifest_path"]).parent
        try:
            relative = directory.relative_to(root).as_posix()
        except ValueError:
            continue
        if relative.startswith("tools/"):
            found.append((package["name"], relative))
    if not found:
        raise Prereq("the workspace declares no tools/ package; the #[path] tripwire would measure nothing")
    return sorted(found)


def tier_journey(ctx: Context):
    if not have("cargo"):
        raise Prereq("cargo is not on PATH")
    with measured_tree(ctx, "programs", "crates", "tools") as (root, _):
        manifest = root / "tools/gauntlet/journey/Cargo.toml"
        if not manifest.is_file():
            raise Prereq("the journey tier is not in this tree")
        code = _run(ctx, ["cargo", "test", "--manifest-path", manifest, "--bins"], cwd=root,
                    env=_env(CARGO_BUILD_JOBS=ctx.jobs)).returncode
        if code:
            note("the journey does not compile or a host test failed; most often the #[path] tripwire: a successor module moved. Fix the journey to match its upstream, never fork the module.")
        # The tripwire is every `tools/` PACKAGE, named, not every `tools/`
        # WORKSPACE. Until 2026-09-05 each of these carried its own
        # `[workspace]` table and this loop found them by that table; folding
        # the tree into one workspace would have left the loop matching
        # nothing and passing on an empty set -- a gate that measures an
        # absence reports the same green as a gate that measures a pass.
        # `cargo metadata` is the enumerator now, and a `tools/` package with
        # no row is a package this tier never checked.
        rotted, declined = [], []
        for name, relative in _tool_packages(ctx, root):
            if relative == "tools/gauntlet/journey":
                continue  # `cargo test --bins` above already compiled it
            result = _run(ctx, ["cargo", "check", "-p", name, "--all-targets"], cwd=root,
                          env=_env(CARGO_BUILD_JOBS=ctx.jobs), capture=True)
            if result.returncode:
                print(result.stdout + result.stderr)
                if "compile_error!" in result.stdout + result.stderr:
                    declined.append(relative)  # a crate demanding a feature choice, not a broken one
                else:
                    rotted.append(relative)
    if rotted:
        note("tool workspaces that do not compile: " + ", ".join(rotted))
        return EXIT_FAIL, "a tool workspace does not compile"
    if code:
        return EXIT_FAIL, "the journey does not compile or its host tests failed"
    if declined:
        note("NOT CHECKED, they decline a featureless check via compile_error!: " + ", ".join(declined))
        return EXIT_PREREQ, f"{len(declined)} tool workspace(s) declined a featureless check"
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- root-targets

ROOT_TARGETS = GATES / "root-targets.tsv"
ROOT_TARGET_TIMEOUT_S = 120
ROOT_TARGETS_TOTAL_S = 70
ROOT_TARGET_BUDGET_S = 8.00
_LAKE_MARKERS = ('"lake"',)
_ELF_MARKERS = ("solana_program_test", "ProgramTest", "SBF_OUT_DIR")


def root_targets_census(root: Path):
    """Every root-workspace integration test target, with what it needs (lake, elf, or nothing)."""
    raw = sh(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root, capture=True)
    if raw.returncode != 0:
        raise Prereq("cargo metadata failed")
    rows = []
    for package in json.loads(raw.stdout)["packages"]:
        for target in package["targets"]:
            if "test" not in target["kind"]:
                continue
            source = Path(target["src_path"])
            text = source.read_text(errors="replace") if source.exists() else ""
            needs = [name for name, markers in (("lake", _LAKE_MARKERS), ("elf", _ELF_MARKERS))
                     if any(marker in text for marker in markers)]
            rows.append((package["name"], target["name"], needs))
    return sorted(rows)


def root_targets_rows(root: Path):
    rows = []
    for number, fields in read_tsv(root / "tools/gates/root-targets.tsv", 4):
        status, package, target, secs = fields[:4]
        if status not in ("run", "quarantine", "slow"):
            raise Prereq(f"root-targets.tsv:{number}: unknown status {status!r}")
        rows.append((status, package, target, secs, fields[4] if len(fields) > 4 else ""))
    return rows


def root_targets_check(root: Path) -> int:
    """The tier's control: the cheap set and the tsv name the same targets, and every recorded time fits."""
    cheap = [(p, t) for p, t, needs in root_targets_census(root) if not needs]
    rows = root_targets_rows(root)
    listed = {(p, t): (s, secs) for s, p, t, secs, _ in rows}
    problems = 0
    for p, t in sorted(set(cheap) - set(listed)):
        problems += 1
        note(f"UNWIRED   cargo test -p {p} --test {t}")
    for p, t in sorted(set(listed) - set(cheap)):
        problems += 1
        note(f"ORPHANED  root-targets.tsv names {p} --test {t}, no longer a cheap root-workspace target")
    for status, p, t, secs, _ in rows:
        try:
            seconds = float(secs)
        except ValueError:
            problems += 1
            note(f"MALFORMED {p} --test {t}: seconds {secs!r}")
            continue
        if status in ("run", "quarantine") and seconds > ROOT_TARGET_BUDGET_S:
            problems += 1
            note(f"OVER      {p} --test {t} records {secs}s, over the {ROOT_TARGET_BUDGET_S:.2f}s budget")
        if status == "slow" and seconds <= ROOT_TARGET_BUDGET_S:
            problems += 1
            note(f"EXCUSED   {p} --test {t} is `slow` at {secs}s, within budget")
    if problems:
        note("every cheap target needs a row: run, quarantine (red today, measured), or slow (measured seconds)")
        return EXIT_FAIL
    note(f"{len(cheap)} cheap targets, all wired, all within the {ROOT_TARGET_BUDGET_S:.2f}s per-target budget")
    return EXIT_PASS


def tier_root_targets(ctx: Context):
    if not have("cargo"):
        raise Prereq("cargo is not on PATH")
    with measured_tree(ctx, "crates", "programs") as (root, _):
        if root_targets_check(root):
            return EXIT_FAIL, "the census and root-targets.tsv disagree about what exists"
        rows = [(s, p, t) for s, p, t, _, _ in root_targets_rows(root) if s in ("run", "quarantine")]
        if not rows:
            raise Prereq("root-targets.tsv lists no runnable target")
        build = ["cargo", "build", "--tests"]
        for package in sorted({p for _, p, _ in rows}):
            build += ["-p", package]
        started = time.time()
        if _run(ctx, build, cwd=root, env=_env(CARGO_BUILD_JOBS=ctx.jobs), quiet=True).returncode:
            return EXIT_FAIL, "the cheap test targets do not compile"
        note(f"build: {int(time.time() - started)}s (not budgeted)")
        timeout = shutil.which("timeout") or shutil.which("gtimeout")
        if not timeout:
            note("no timeout(1) on PATH; a hung target hangs this tier")
        red, green, killed, total = [], [], [], 0
        for status, package, target in rows:
            start = time.time()
            args = ([timeout, str(ROOT_TARGET_TIMEOUT_S)] if timeout else []) + \
                ["cargo", "test", "-p", package, "--test", target, "-q"]
            code = _run(ctx, args, cwd=root, quiet=True).returncode
            total += int(time.time() - start)
            if code == 124:
                killed.append(f"{package} --test {target}")
            elif status == "run" and code:
                red.append(f"{package} --test {target}")
            elif status == "quarantine" and code == 0:
                green.append(f"{package} --test {target}")
        slack = int(os.environ.get("DCLUTCH_GATE_TIME_SLACK", "4"))
        budget = ROOT_TARGETS_TOTAL_S * slack
        note(f"{len(rows)} targets executed in {total}s (backstop {budget}s = {ROOT_TARGETS_TOTAL_S}s measured x {slack} slack)")
        failed = False
        if red:
            failed = True
            note("FAILED, the tsv says these pass: " + ", ".join(red))
        if green:
            failed = True
            note("QUARANTINE IS STALE, these pass now; delete or promote their rows: " + ", ".join(green))
        if killed:
            failed = True
            note(f"TIMED OUT at {ROOT_TARGET_TIMEOUT_S}s (never finished, which is not failed): " + ", ".join(killed))
        if total > budget:
            failed = True
            note("over the LOOSE wall-clock backstop; on a loaded machine re-run before reporting it")
        for package, target, needs in root_targets_census(root):
            if needs:
                note(f"excluded ({'+'.join(needs)}): cargo test -p {package} --test {target}")
        for status, package, target, secs, why in root_targets_rows(root):
            if status == "slow":
                note(f"excluded (slow, {secs}s): {package} --test {target} {why}")
    return (EXIT_FAIL, "") if failed else (EXIT_PASS, "")


# --------------------------------------------------------------------------- programs

# The five protocol programs the trading fixture installs, the rent link a test reads,
# and the three test-only callers `waist::elves` loads beside them.
PROGRAM_MANIFESTS = (
    "programs/dclutch-trading-sbf/Cargo.toml",
    "programs/dclutch-registry-sbf/Cargo.toml",
    "programs/dclutch-core-sbf/Cargo.toml",
    "programs/dclutch-claims-sbf/Cargo.toml",
    "programs/dclutch-custody-sbf/Cargo.toml",
    "programs/dclutch-rent-sbf/Cargo.toml",
    "programs/dclutch-trading-sbf/program-test/test-programs/trading-outer/Cargo.toml",
    "programs/dclutch-trading-sbf/program-test/test-programs/core-caller/Cargo.toml",
    "programs/dclutch-trading-sbf/program-test/test-programs/registry/Cargo.toml",
)
# Hot-continuation hostiles: they read POSTJOIN_SBF_OUT_DIR, which run-postjoin-hostiles.sh (a suites row) builds.
PROGRAMS_SKIPPED = (
    "nonselected_claims_supply_corruption_after_real_child_commit_rolls_back",
    "omitted_token_close_authority_corruption_after_real_custody_commit_rolls_back",
    "omitted_custody_replay_lineage_corruption_after_real_child_commit_rolls_back",
)


def tier_programs(ctx: Context):
    if not have("cargo-build-sbf"):
        raise Prereq("cargo-build-sbf is not on PATH (sh -c \"$(curl -sSfL https://release.anza.xyz/stable/install)\")")
    if not (REPO / "programs/dclutch-trading-sbf/program-test/Cargo.toml").is_file():
        raise Prereq("the trading program-test is not in this tree")
    owned = "DCLUTCH_GATE_SBF_OUT_DIR" not in os.environ
    elf_dir = Path(os.environ.get("DCLUTCH_GATE_SBF_OUT_DIR") or tempfile.mkdtemp(prefix="dclutch-gate-elf."))
    elf_dir.mkdir(parents=True, exist_ok=True)
    try:
        with measured_tree(ctx, "programs", "crates") as (root, _):
            diagnostics = 0
            for manifest in PROGRAM_MANIFESTS:
                link = Path(manifest).parent.name
                note(f"build {link}")
                args = ["cargo", "build-sbf", "--manifest-path", manifest, "--sbf-out-dir", elf_dir]
                if ctx.dry_run:
                    _run(ctx, args, cwd=root)
                    continue
                log = elf_dir / f"build-{link}.log"
                with open(log, "w") as handle:
                    result = subprocess.run(args, cwd=root, stdout=handle, stderr=subprocess.STDOUT)
                if result.returncode:
                    print(log.read_text()[-3000:])
                    return EXIT_FAIL, f"an SBF program did not build: {manifest}"
                count = log.read_text().count(FRAME_DIAGNOSTIC)
                if count:
                    note(f"{link}: {count} SBF stack-frame-overwrite diagnostics")
                    diagnostics += count
            if diagnostics:
                note("REFUSING: the toolchain says these calls may cause undefined behavior; measure with tools/sbf-frame-sizes.py")
                return EXIT_FAIL, f"{diagnostics} SBF stack-frame-overwrite diagnostics"
            args = ["cargo", "test", "--manifest-path", "programs/dclutch-trading-sbf/program-test/Cargo.toml"]
            args += os.environ.get("DCLUTCH_GATE_PROGRAM_TESTS", "").split()
            args += ["--", "--nocapture"]
            for name in PROGRAMS_SKIPPED:
                args += ["--skip", name]
            if _run(ctx, args, cwd=root, env=_env(SBF_OUT_DIR=str(elf_dir))).returncode:
                note("if the failing row is direct_hot_top_level: the public Direct route lost margin. Find what got more expensive before touching the gate's constant; check the shared contract crates first.")
                return EXIT_FAIL, "the trading program-test suite failed"
    finally:
        if owned:
            shutil.rmtree(elf_dir, ignore_errors=True)
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- suites

SUITE_RUNNERS = (
    ("custody", "programs/dclutch-custody-sbf/run-program-test.sh", "Custody vault routes against a real caller link"),
    ("core", "programs/dclutch-core-sbf/run-open-market-program-test.sh", "every core program-test target, discovered from tests/"),
    ("claims", "programs/dclutch-claims-sbf/run-rational-representation-v2-program-test.sh", "the rational representation V2 lowering"),
    ("claims-lifecycle", "programs/dclutch-claims-sbf/program-test/rational-lifecycle/run-program-test.sh", "the Token-2022 receipt and coordinate lifecycle, on the pinned v11 artifact"),
    ("claims-position", "programs/dclutch-claims-sbf/program-test/protocol-position/run-program-test.sh", "the ordered Fractional retirement walk closing a real shard Mint"),
    ("claims-fractional", "programs/dclutch-claims-sbf/program-test/fractional-atomic/run-program-test.sh", "four Fractional campaigns: atomicity, the permissioned burn wall, compaction, escrow PDA handover"),
    ("sparse-chain", "programs/dclutch-claims-sbf/program-test/sparse-chain/run-program-test.sh", "the sparse native transfer chain"),
    ("affine-batch", "programs/dclutch-claims-sbf/program-test/affine-batch/run-program-test.sh", "the affine batch V2 lowering"),
    ("signed-delta", "programs/dclutch-claims-sbf/program-test/fractional-signed-delta/run-program-test.sh", "the fractional signed-delta route"),
    ("dealer", "programs/dclutch-accelerator-sbf/dealer-program-test/run-program-test.sh", "the accelerator link, driven through its Dealer arm and the Dealer family tests"),
    ("general", "programs/dclutch-accelerator-sbf/program-test/run-program-test.sh", "the accelerator link through its General arm, its freeze wall and its hot instruction"),
    ("userposition", "programs/dclutch-trading-sbf/program-test/user-position-admission/run-program-test.sh", "user position admission across the lifecycle"),
    ("registry", "programs/dclutch-registry-sbf/run-lineage-program-test.sh", "the release-set successor declaration and the walk that follows the hop"),
    ("fee2tx", "programs/dclutch-trading-sbf/program-test/run-fee-second-transaction.sh", "the Direct fee leg in a transaction of its own, against real Custody"),
    ("postjoin", "programs/dclutch-trading-sbf/program-test/run-postjoin-hostiles.sh", "Trading refuses three isolated child adversaries and rolls the whole transaction back"),
)


def tier_suites(ctx: Context):
    if not have("cargo-build-sbf"):
        raise Prereq("cargo-build-sbf is not on PATH")
    if ctx.commit:
        note("--commit does not reach this tier: these runners build the working tree and take no revision")
    wanted = os.environ.get("DCLUTCH_GATE_SUITES", "").split()
    present = failed = 0
    absent, unrun = [], []
    for name, script, what in SUITE_RUNNERS:
        if wanted and name not in wanted:
            continue
        path = REPO / script
        if not (path.is_file() and os.access(path, os.X_OK)):
            absent.append(name)
            continue
        present += 1
        note(f"{name} -- {what}")
        code = _run(ctx, [path], cwd=REPO, env=_env(CARGO_BUILD_JOBS=ctx.jobs)).returncode
        if code == EXIT_PREREQ:
            unrun.append(name)
        elif code:
            failed += 1
            note(f"{name}: FAILED")
    if absent:
        note("runners not in this tree: " + " ".join(absent))
    if unrun:
        note("rows that DID NOT RUN (missing prerequisite): " + " ".join(unrun))
    if present == 0:
        return EXIT_PREREQ, "no suite runner is present"
    if failed:
        return EXIT_FAIL, f"{failed} of {present} suites failed"
    if unrun:
        return EXIT_PREREQ, "rows did not run: " + " ".join(unrun)
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- workspaces

def tier_workspaces(ctx: Context):
    tool = REPO / "tools/release/check-all-workspaces.py"
    if not tool.is_file():
        raise Prereq("tools/release/check-all-workspaces.py is absent")
    if not have("cargo"):
        raise Prereq("cargo is not on PATH")
    with scratch("ws") as tmp:
        code = _run(ctx, ["python3", tool, "--work", tmp / "run", "--commit", ctx.commit or "HEAD"], cwd=REPO).returncode
    if code:
        return EXIT_FAIL, "a tracked Cargo workspace does not check at this revision, or a lockfile moved inside the archive"
    return EXIT_PASS, ""


# --------------------------------------------------------------------------- the table

@dataclass(frozen=True)
class Tier:
    name: str
    cost: str
    needs: str
    gates: str
    run: Callable[[Context], tuple[int, str]]


TIERS: tuple[Tier, ...] = (
    Tier("selftest", "~25s (2026-09-04)", "python3, bash", "the gates' own refusal tests: frames, reference, commands, lane, seam-audit, the gauntlet CLI", tier_selftest),
    Tier("census", "~25s warm, ~50s cold (2026-09-04)", "cargo", "routes, refusal codes, magics and schema identities enumerated from the AST; a code outside its band, a duplicated magic, an identity that is not its label's digest", tier_census),
    Tier("emission", "~2s (2026-09-04)", "python3, rustfmt", "a generated file with no byte-identity guard, or one rustfmt would move out from under a raw-comparing guard; a two-sided wire vector whose reviewed digest moved", tier_emission),
    Tier("budgets", "<1s (2026-09-04)", "python3", "a CU budget that is not measured+tolerance, above the 1,400,000 ceiling, or naming a campaign no register knows", tier_budgets),
    Tier("fmt", "~10s (2026-09-03)", "cargo, rustfmt", "rustfmt disagreeing with a file outside fmt-baseline.txt, or a baseline line no longer true (every package: one workspace since 2026-09-05)", tier_fmt),
    Tier("locks", "~30s (2026-09-02)", "cargo", "a tracked Cargo.lock that no longer resolves under --locked --offline", tier_locks),
    Tier("seam", "~20s (2026-09-01)", "ast-grep", "a new structural seam finding against tools/seam-audit's triaged baseline", tier_seam),
    Tier("commands", "~15s (2026-09-04)", "python3", "a runbook command whose program is absent, whose flags its own --help does not name, or which omits a required argument", tier_commands),
    Tier("release", "~45s (2026-09-03)", "python3, bash, git", "the release tooling's refusal suites: forged build evidence admitted, a market founded at a fee that cannot trade, usage text its parser rejects", tier_release),
    Tier("reference", "~3 min (2026-09-03)", "cargo, node", "docs/reference and its client mirrors not at their fixpoint at the measured commit", tier_reference),
    Tier("clippy", "22s warm, minutes cold (2026-09-03)", "cargo, clippy", "a red package outside clippy-debt.tsv, or a debt row that went green; never-reached packages counted apart", tier_clippy),
    Tier("sbom", "~3 min (2026-09-01)", "cargo, python3", "a git-sourced or checksum-less dependency, or drift in the committed SBOM/NOTICES", tier_sbom),
    Tier("sbfcontracts", "minutes (2026-09-01)", "cargo-build-sbf", "a non-program first-party crate that does not compile for target_os=solana", tier_sbfcontracts),
    Tier("web", "~1 min (2026-09-04)", "node, cargo", "the web + SDK lint and vitest suites, and a shipped cohort that does not answer", tier_web),
    Tier("abi", "~3 min (2026-09-03)", "lake, cargo, wasm-bindgen", "a generated client module that no longer matches the Rust or Lean that printed it (53 verifiers)", tier_abi),
    Tier("guards", "86s warm, 195s cold (2026-09-04)", "lake, rustfmt, node", "a generated file whose bytes no longer match its emitter (77 guards run for real)", tier_guards),
    Tier("frames", "~4 min (2026-09-02)", "cargo-build-sbf", "a function in any of the twelve SBF links whose exact frame differs from the admitted ratchet; names the commits that owe rows", tier_frames),
    Tier("journey", "minutes (2026-09-03)", "cargo", "the journey campaign or any tools/ workspace failing to compile (the #[path] tripwire)", tier_journey),
    Tier("root-targets", "~4 min (2026-09-03)", "cargo", "a cheap root-workspace integration test that fails, a quarantined one that passes, or a new one with no row", tier_root_targets),
    Tier("programs", "minutes (2026-09-03)", "cargo-build-sbf", "an SBF program that does not build, a frame-overwrite diagnostic, or the public Direct route losing compute margin", tier_programs),
    Tier("suites", "~25 min (2026-09-03)", "cargo-build-sbf", "any other SBF program-test suite failing, each run through the runner its owner maintains", tier_suites),
    Tier("witness", "~20s, devnet RPC (2026-09-04)", "python3, ~/.helius-key", "a devnet route witness the chain does not corroborate", tier_witness),
    Tier("workspaces", "slow (2026-09-03)", "cargo", "any tracked Cargo workspace failing to check from an archived revision (the cut tier)", tier_workspaces),
)

CHEAP = ("selftest", "census", "emission", "budgets", "fmt", "locks", "seam", "commands", "release")
ALL = CHEAP + ("reference", "clippy", "sbom", "sbfcontracts", "web", "abi", "guards", "frames",
               "journey", "root-targets", "programs", "suites", "witness")
