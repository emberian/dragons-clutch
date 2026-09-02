#!/usr/bin/env python3
"""Read one `cargo clippy --message-format=json` stream and judge it.

`tools/ci/run.sh clippy` runs the cargo invocation and hands the JSON here.  The
split is deliberate: cargo's exit status says only "something was red", and the
three facts this tier needs are which PACKAGE was red, which packages clippy
never reached, and whether either set moved against `tools/ci/clippy-debt.tsv`.

WHY THE PACKAGE IS THE UNIT.  `--keep-going` checks everything whose
dependencies compiled and stops at a red library, so one red kernel hides every
package above it.  A per-lint quarantine would let the hidden set grow silently;
a per-package one makes "how much of the workspace did we actually look at" a
number this tier prints on every run.

Exit codes are `tools/ci/run.sh`'s: 0 the census agrees with the tsv, 1 it does
not, 2 the inputs were not there to judge.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import subprocess
import sys

EXIT_PASS = 0
EXIT_GATE_FAILED = 1
EXIT_PREREQ_MISSING = 2


def read_debt(path: pathlib.Path) -> dict[str, dict[str, str]]:
    rows: dict[str, dict[str, str]] = {}
    for line in path.read_text().splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.split("\t")
        if fields[0] == "status":
            continue
        if len(fields) < 6:
            raise ValueError(f"clippy-debt.tsv row has {len(fields)} columns, not 6: {line}")
        status, package, lints, sites, owner, note = fields[:6]
        if status != "debt":
            raise ValueError(f"clippy-debt.tsv: unknown status {status!r}")
        rows[package] = {"lints": lints, "sites": sites, "owner": owner, "note": note}
    return rows


def workspace_members(root: pathlib.Path) -> dict[str, str]:
    """package id -> name, for the root workspace only."""
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"],
        cwd=root, capture_output=True, text=True, check=False,
    )
    if out.returncode != 0:
        raise RuntimeError(f"cargo metadata failed: {out.stderr.strip()[:400]}")
    return {p["id"]: p["name"] for p in json.loads(out.stdout)["packages"]}


def parse_stream(stream_path: pathlib.Path, members: dict[str, str]):
    """-> (checked, red, findings) over member NAMES."""
    checked: set[str] = set()
    red: set[str] = set()
    findings: dict[str, set[tuple[str, str, int]]] = collections.defaultdict(set)
    for line in stream_path.read_text(errors="replace").splitlines():
        line = line.strip()
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
        if message.get("reason") != "compiler-message":
            continue
        body = message["message"]
        if body.get("level") != "error":
            continue
        red.add(package)
        # `could not compile X (lib) due to N previous errors` is a summary of
        # findings already counted; carrying it would double every site.
        if "could not compile" in body.get("message", ""):
            continue
        code = (body.get("code") or {}).get("code") or "rustc"
        for span in body.get("spans", []):
            if span.get("is_primary"):
                findings[package].add((code, span["file_name"], span["line_start"]))
    return checked, red, findings


def lint_table_optin(root: pathlib.Path) -> tuple[int, int]:
    """How many workspace members inherit `[workspace.lints]`, and how many exist.

    A member without `[lints] workspace = true` does not get the deny table at
    Cargo.toml:119 AT ALL -- `unwrap_used`, `panic` and `indexing_slicing` are
    allow-by-default restriction lints, so `-D warnings` does not reach them
    either.  The tier prints this because a package can be green here for the
    uninteresting reason.
    """
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"],
        cwd=root, capture_output=True, text=True, check=False,
    )
    if out.returncode != 0:
        return (0, 0)
    optin = 0
    total = 0
    for package in json.loads(out.stdout)["packages"]:
        manifest = pathlib.Path(package["manifest_path"])
        total += 1
        try:
            text = manifest.read_text()
        except OSError:
            continue
        if "[lints]" in text and "workspace = true" in text.split("[lints]", 1)[1][:64]:
            optin += 1
    return (optin, total)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stream", required=True, help="cargo clippy --message-format=json output")
    parser.add_argument("--debt", required=True, help="tools/ci/clippy-debt.tsv")
    parser.add_argument("--root", default=".", help="workspace root")
    arguments = parser.parse_args()

    stream = pathlib.Path(arguments.stream)
    debt_path = pathlib.Path(arguments.debt)
    root = pathlib.Path(arguments.root)
    if not stream.is_file():
        print(f"clippy census: no message stream at {stream}")
        return EXIT_PREREQ_MISSING
    if not debt_path.is_file():
        print(f"clippy census: no debt list at {debt_path}")
        return EXIT_PREREQ_MISSING

    try:
        debt = read_debt(debt_path)
        members = workspace_members(root)
    except (ValueError, RuntimeError) as error:
        print(f"clippy census: {error}")
        return EXIT_PREREQ_MISSING

    names = set(members.values())
    checked, red, findings = parse_stream(stream, members)
    if not checked and not red:
        print("clippy census: the stream names no workspace package -- nothing was measured")
        return EXIT_PREREQ_MISSING
    unreached = sorted(names - checked - red)

    optin, total = lint_table_optin(root)
    print(f"    {len(names)} workspace members: {len(checked - red)} clean, {len(red)} red, "
          f"{len(unreached)} never reached (a package they need is red)")
    print(f"    {optin}/{total} members opt into [workspace.lints] with `[lints] workspace = true`;")
    print(f"    the other {total - optin} do not get the deny table at Cargo.toml:119 at all")

    failed = False

    unrecorded = sorted(red - set(debt))
    if unrecorded:
        print(f"    RED AND NOT IN tools/ci/clippy-debt.tsv -- {len(unrecorded)} package(s):")
        for package in unrecorded:
            lints = sorted({code.removeprefix("clippy::") for code, _, _ in findings[package]})
            print(f"      {package}: {', '.join(lints) or 'see the log'}")
            for code, file_name, line in sorted(findings[package], key=lambda f: (f[1], f[2])):
                print(f"          {file_name}:{line}  {code.removeprefix('clippy::')}")
        print("    Fix it, or -- if it belongs to a lane that is not yours -- add a `debt`")
        print("    row naming the file, the lint and the last commit to touch it.")
        failed = True

    stale = sorted(set(debt) - red)
    if stale:
        print(f"    DEBT IS STALE -- {len(stale)} package(s) recorded RED now pass:")
        for package in stale:
            reached = "" if package in checked else " (and clippy never reached it -- a"\
                " dependency is red, so this is not evidence it is clean)"
            print(f"      {package}  [{debt[package]['lints']}]{reached}")
        print("    This is good news reported as a failure ON PURPOSE. Delete the row from")
        print("    tools/ci/clippy-debt.tsv in the commit that fixed it -- a debt list that")
        print("    only grows is how a known red becomes a red nobody looks at.")
        failed = True

    if unreached:
        print(f"    never reached, and why: {len(unreached)} package(s) depend on something red")
        for package in unreached:
            print(f"      {package}")
        print("    These are NOT PASSING. They are unmeasured, and the way to measure them")
        print("    is to clear a `debt` row above, not to add one here.")

    if failed:
        return EXIT_GATE_FAILED
    print("    every red package is recorded, and every recorded package is still red")
    return EXIT_PASS


if __name__ == "__main__":
    sys.exit(main())
