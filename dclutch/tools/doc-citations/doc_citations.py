#!/usr/bin/env python3
"""Standing report for ledger 8.2: the citation outlives the thing cited.

A doc comment that names a symbol in backticks is a pointer, and pointers rot
silently.  Nothing in a Rust build checks that ``funded::process_funded_transition``
still exists, so when the function is deleted the sentence stays, keeps its
authority, and the next reader reasons from it.  This tree paid for that on
2026-09-01: five comments across four files cited that exact symbol in the
PRESENT tense as the reason for a live on-chain refusal
(``CoreSbfError::RecoveryWalkUnavailable``), one of them a lifting plan telling
someone to *resurrect* it -- and the function had no definition anywhere.  The
lane that deleted the surrounding dead code had already misreported once from
the same source, reading a deleted route's nouns as the live vocabulary.

WHAT THIS IS NOT.  It is not name resolution.  There is no rustc here, no
type information, no imports, no glob expansion, no macro expansion.  It reads
source text, needs no build, and takes seconds -- the same posture as
``tools/seam-audit``.

WHAT IT THEREFORE DOES.  It judges the SIGNAL and declines the noise, and says
which is which.  The signal is a namespaced path -- ``a::b``, ``Type::method``,
``crate::mod::item`` -- whose leading segment belongs to this workspace and
whose final segment is declared nowhere in it.  Everything else is declined out
loud: prose in backticks, file paths, code fragments, and any path rooted in a
crate we cannot see.  A declined citation is not a passing citation; it is a
citation this tool refuses to have an opinion about, and the count is printed so
nobody mistakes silence for coverage.

EXIT CODE.  Zero, always, unless ``--check`` is given with a baseline.  The
category is worth watching before it is worth gating, and a reporter that
nobody can turn off is a reporter everybody routes around.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import sys

# Pruned during the walk, not filtered after it.  `target/` alone is large
# enough that filtering afterwards took the run from seconds to two and a half
# minutes, and a checker nobody will wait for is a checker nobody runs.
SKIP_DIRS = {"target", ".git", "node_modules", "worktrees", ".claude"}

# Item declarations we can see without a compiler.
# Scanned with finditer rather than matched at line start: a one-line
# `pub mod m { pub fn f() {} }` declares two items and an anchored pattern sees
# only the first.  The `(?:^|[\s{])` prefix keeps it from firing mid-identifier.
ITEM_RE = re.compile(
    r"(?:^|[\s{])(?:pub(?:\([^)]*\))?\s+)?"
    r"(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?"
    r"(fn|struct|enum|trait|union|type|const|static|mod)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)"
)
MACRO_RE = re.compile(r"^\s*macro_rules!\s+([A-Za-z_][A-Za-z0-9_]*)")
ENUM_OPEN_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)[^{]*\{"
)
STRUCT_OPEN_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:struct|union)\s+([A-Za-z_][A-Za-z0-9_]*)[^{;]*\{"
)
VARIANT_RE = re.compile(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:[,({=]|$)")
# A named field inside a braced struct.  Docs cite these as `Type::field` even
# though that is not path syntax, so they have to be in the index or every one
# of them reads as a dangling citation.
FIELD_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*[^=]")
DOC_RE = re.compile(r"^\s*(?://[/!])\s?(.*)$")
# An ordinary `//` comment, including a trailing one.  The `(?:^|\s)` prefix is
# what keeps `http://` out: a URL's slashes are preceded by a colon.  A `//`
# inside a string literal can still slip through, and lands in declined-prose.
LINE_RE = re.compile(r"(?:^|\s)//(?![/!])(.*)$")
# Single- and double-backtick spans.
SPAN_RE = re.compile(r"``([^`]+)``|`([^`\n]+)`")
PATH_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+(?:\(\))?$")

# Roots we can never see the items of.  A path rooted here is declined, not judged.
EXTERNAL_ROOTS = {
    "std", "core", "alloc", "proc_macro", "test",
    "Self", "self", "super",
}


def walk(root: pathlib.Path):
    """Yield every .rs and Cargo.toml under root, pruning the heavy trees."""
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for name in filenames:
            if name.endswith(".rs") or name == "Cargo.toml":
                yield pathlib.Path(dirpath) / name


def survey(root: pathlib.Path) -> tuple[list[pathlib.Path], set[str]]:
    files: list[pathlib.Path] = []
    crates: set[str] = set()
    for path in walk(root):
        if path.name == "Cargo.toml":
            for line in path.read_text(errors="replace").splitlines():
                m = re.match(r'^\s*name\s*=\s*"([^"]+)"', line)
                if m:
                    crates.add(m.group(1).replace("-", "_"))
                    break
        else:
            files.append(path)
    return sorted(files), crates


def symbol_index(files) -> tuple[set[str], dict[str, list[str]]]:
    """Every item name declared in the tree, plus enum variants and struct fields.

    Variants and fields are here because docs cite them constantly as
    ``Type::Variant`` and ``Type::field``.  Without them the report is nothing
    but those, and a report that is mostly noise is one nobody reads twice.
    """
    names: set[str] = set()
    where: dict[str, list[str]] = {}

    def note(name: str, path: str) -> None:
        names.add(name)
        where.setdefault(name, []).append(path)

    for path in files:
        try:
            lines = path.read_text(errors="replace").splitlines()
        except OSError:
            continue
        brace = 0
        enum_at = 0
        struct_at = 0
        for line in lines:
            m = MACRO_RE.match(line)
            if m:
                note(m.group(1), str(path))
            for m in ITEM_RE.finditer(line):
                note(m.group(2), str(path))
            if enum_at and brace >= enum_at:
                v = VARIANT_RE.match(line)
                if v:
                    note(v.group(1), str(path))
            if struct_at and brace >= struct_at:
                f = FIELD_RE.match(line)
                if f:
                    note(f.group(1), str(path))
            opened_enum = ENUM_OPEN_RE.match(line)
            opened_struct = None if opened_enum else STRUCT_OPEN_RE.match(line)
            if opened_enum or opened_struct:
                # A single-line `enum E { A, B }` or `struct S { a: u8 }` puts
                # its members on the opening line, where the block scan below
                # never sees them. Found by the synthetic control, which is what
                # the control is for.
                tail = line.split("{", 1)[1].rstrip().rstrip("}")
                member = VARIANT_RE if opened_enum else FIELD_RE
                for piece in tail.split(","):
                    hit = member.match(" " + piece.strip())
                    if hit:
                        note(hit.group(1), str(path))
                enum_at = brace + 1 if opened_enum else enum_at
                struct_at = brace + 1 if opened_struct else struct_at
            brace += line.count("{") - line.count("}")
            if enum_at and brace < enum_at:
                enum_at = 0
            if struct_at and brace < struct_at:
                struct_at = 0
    return names, where


def citations(path: pathlib.Path, include_line_comments: bool):
    """Yield (line, span, kind) for every backtick span in a comment.

    `kind` is "doc" for `///` and `//!`, "line" for an ordinary `//`.  They are
    reported separately because they are different corpora with different
    prose-to-citation ratios, and a boundary between them should be a measured
    number rather than an inherited caution.
    """
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return
    for number, line in enumerate(lines, 1):
        doc = DOC_RE.match(line)
        if doc:
            body, kind = doc.group(1), "doc"
        elif include_line_comments:
            hit = LINE_RE.search(line)
            if not hit:
                continue
            body, kind = hit.group(1), "line"
        else:
            continue
        for double, single in SPAN_RE.findall(body):
            yield number, (double or single).strip(), kind


def classify(span: str, crates: set[str], names: set[str]):
    if not PATH_RE.match(span):
        return "declined-prose", None
    segments = span.rstrip("()").split("::")
    head, tail = segments[0], segments[-1]
    if head in EXTERNAL_ROOTS:
        return "declined-external", tail
    if head in crates and head not in names:
        # A crate root we own; its items are indexed under their own names.
        pass
    elif head not in names and head not in crates:
        return "declined-external", tail
    if tail in names:
        return "resolved", tail
    return "unresolved", tail


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", default=".", help="repository root")
    ap.add_argument("--baseline", default=None, help="JSON baseline to compare against")
    ap.add_argument("--write", action="store_true", help="write the baseline and exit")
    ap.add_argument("--check", action="store_true", help="exit 1 on citations absent from the baseline")
    ap.add_argument("--quiet", action="store_true", help="totals only")
    ap.add_argument(
        "--comments",
        choices=("doc", "line", "all"),
        default="all",
        help="which comment corpus to scan (default: all)",
    )
    args = ap.parse_args()

    root = pathlib.Path(args.root).resolve()
    files, crates = survey(root)
    names, _ = symbol_index(files)

    kinds = ("doc", "line") if args.comments == "all" else (args.comments,)
    tally = {k: {"resolved": 0, "declined-prose": 0, "declined-external": 0, "unresolved": 0}
             for k in ("doc", "line")}
    unresolved: list[tuple[str, int, str, str]] = []
    for path in files:
        rel = str(path.relative_to(root))
        for number, span, kind in citations(path, include_line_comments="line" in kinds):
            if kind not in kinds:
                continue
            verdict, tail = classify(span, crates, names)
            tally[kind][verdict] += 1
            if verdict == "unresolved":
                unresolved.append((rel, number, span, tail or span))

    label = {"doc": "/// and //!", "line": "ordinary //"}
    print(f"doc citations: {len(files)} files, corpora = {args.comments}")
    for kind in kinds:
        t = tally[kind]
        spans = sum(t.values())
        judged = t["resolved"] + t["unresolved"]
        print(f"  {label[kind]:<12} {spans:>6} spans   judged {judged:>5} "
              f"({t['resolved']} resolve, {t['unresolved']} do not)   "
              f"declined {t['declined-prose']} prose + {t['declined-external']} external")
    print("  declined is not passed: this tool has no opinion on those.")

    by_symbol: dict[str, list[tuple[str, int, str]]] = {}
    for rel, number, span, tail in unresolved:
        by_symbol.setdefault(tail, []).append((rel, number, span))

    if unresolved and not args.quiet:
        print(f"\n{len(unresolved)} citations name a symbol declared nowhere in this tree:\n")
        for tail in sorted(by_symbol, key=lambda k: (-len(by_symbol[k]), k)):
            sites = by_symbol[tail]
            print(f"  {tail}  ({len(sites)} citation{'s' if len(sites) != 1 else ''})")
            for rel, number, span in sorted(sites):
                print(f"      {rel}:{number}  `{span}`")

    if args.baseline:
        keys = sorted({f"{tail}\t{rel}:{number}" for rel, number, span, tail in unresolved})
        bpath = pathlib.Path(args.baseline)
        if args.write:
            bpath.write_text(json.dumps({"unresolved": keys}, indent=2) + "\n")
            print(f"\nwrote {bpath} with {len(keys)} entries")
            return 0
        if args.check:
            known = set(json.loads(bpath.read_text()).get("unresolved", []))
            new = [k for k in keys if k not in known]
            if new:
                print(f"\n{len(new)} NEW dangling citation(s) since the baseline:")
                for k in new:
                    print(f"  {k.replace(chr(9), '  ')}")
                return 1
            print("\nno new dangling citations against the baseline")
    return 0


if __name__ == "__main__":
    sys.exit(main())
