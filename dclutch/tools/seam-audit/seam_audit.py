#!/usr/bin/env python3
"""Standing gate for the seam-defect classes found by hand on 2026-08-29.

``docs/evidence/SEAM_AUDIT_2026_08_29.md`` found nine always-refuses routes and
one always-admits across six seams, none of which had a failing test.  Its
closing paragraph names why: **a green suite is evidence about fixtures, not
about seams** -- each side was tested against a fixture that same side authored,
so both sides were green and the composition was dead.  No test can be added
that fixes that, because the defect is the *absence* of a joint author.

This checker is the joint author.  It reads both sides of a seam statically and
refuses the disagreements, so the six mechanical classes die as a category
rather than being re-hunted by hand.  It is deliberately *not* a test: it runs
over source text, needs no build, no validator and no SBF toolchain, and takes
seconds.

The six classes, each with the 2026-08-29 finding that is its negative control:

  1 SEED_LEN    a PDA seed domain over Solana's 32-byte maximum, and every
                seed domain with no compile-time assert holding it there.
                Control: SEAM_AUDIT #8 / ``fb076ec6``.
  2 DERIVATION  one domain spelled two ways -- differing seed-tuple arity
                across sites, or a raw tuple restated outside the crate that
                owns the domain instead of consuming its exported seed
                function.  Control: SEAM_AUDIT #3 / ``9a9f1b5c``.
  3 PIN_CENSUS  a global no-duplicate census over a frame whose own spec pins
                an equality between two coordinates.  The two sides cannot
                both be satisfied, so the route is dead.
                Control: SEAM_AUDIT #12 / ``3b98ea3a``.
  4 UNSET_PIN   a wire-supplied pubkey used as an identity in a frame that
                carries the System program, with no guard against the default
                (all-zero) pubkey.  The one class of the six with NO
                2026-08-29 defect behind it: the audit records no such finding
                and no commit that day touches the pattern, so its control is
                synthetic and it says so.  Ships mainly as a ratchet on the
                guards that already exist, so one cannot be deleted quietly.
  5 DOMAIN_DUP  two seed-domain constants under different names carrying the
                same bytes, or one name carrying bytes that contradict its
                role.  Matched on BYTES, never on identifier -- the 2026-08-29
                instance hid under a different name.
  6 PRIVILEGE   an exact-privilege census that constrains the whole
                transaction rather than the instruction's own frame: a blanket
                signer refusal over a frame that will contain the fee payer,
                or a readonly pin on a coordinate a partner instruction must
                write.  Control: SEAM_AUDIT #13b.

Gate semantics, following ``packages/dclutch-sdk/scripts/abi-coverage.mjs``:
the baseline is an exact set and the ratchet turns **both** ways.  A finding
absent from the baseline fails the gate as new; a baseline entry that no longer
reproduces also fails, because a defect that was fixed must leave the register
rather than sit there as cover for the next one.  Every baseline entry carries a
verdict tag, and every tag in use must have a written reason in
``EXCEPTIONS.md`` -- so an exception cannot be accepted silently.

Usage:

    tools/seam-audit/seam_audit.py                 # gate: exit 1 on drift
    tools/seam-audit/seam_audit.py --write         # retriage into the baseline
    tools/seam-audit/seam_audit.py --report        # the register, all findings
    tools/seam-audit/seam_audit.py --class SEED_LEN --report
    tools/seam-audit/seam_audit.py --root <dir>    # audit another checkout

This is local static evidence only.  It does not build, sign, submit, publish,
or contact a cluster.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field

SCHEMA = "dclutch-seam-audit-v1"

# Solana concatenates seed segments before hashing and refuses any single
# segment over this many bytes, for every bump.  A domain over the line has no
# derivable address at all.
SVM_MAX_PDA_SEED_BYTES = 32

TOOL_DIR = pathlib.Path(__file__).resolve().parent
BASELINE_PATH = TOOL_DIR / "baseline.json"
EXCEPTIONS_PATH = TOOL_DIR / "EXCEPTIONS.md"

# Trees that hold protocol source.  ``target`` and ``node_modules`` are never
# read; ``fixtures`` holds captured chain bytes rather than authored code.
RUST_ROOTS = ("crates", "programs", "tools", "formal")
TS_ROOTS = ("apps/dclutch-web/lib", "packages/dclutch-sdk/lib")

CLASSES = (
    "SEED_LEN",
    "DERIVATION",
    "PIN_CENSUS",
    "UNSET_PIN",
    "DOMAIN_DUP",
    "PRIVILEGE",
)


class AuditError(Exception):
    """A fault in the checker's own inputs, never a finding about the tree."""


@dataclass(frozen=True, order=True)
class Finding:
    """One disagreement, keyed so the baseline survives unrelated edits.

    ``key`` deliberately excludes the line number: a finding must keep its
    identity when code above it moves, or the baseline churns on every commit
    and stops meaning anything.  The line lives in ``detail``, which is printed
    but never compared.
    """

    code: str
    key: str
    path: str = ""
    line: int = 0
    detail: str = ""

    def rendered(self) -> str:
        where = f"{self.path}:{self.line}" if self.path else "(tree)"
        return f"{self.code}\t{self.key}\n    {where}  {self.detail}"


@dataclass
class SeedDomain:
    """A declared PDA seed-domain constant, resolved to its literal bytes."""

    name: str
    value: bytes
    path: str
    line: int
    crate: str
    # Why this constant is treated as a PDA seed domain at all: ``declared``
    # (its identifier says PDA_SEED/PDA_DOMAIN) or ``derived`` (the tree uses
    # it as a seed segment).  Recorded so every inclusion is auditable rather
    # than the checker asserting taste.
    role: str = ""

    @property
    def length(self) -> int:
        return len(self.value)

    @property
    def printable(self) -> str:
        try:
            return self.value.decode("utf-8")
        except UnicodeDecodeError:
            return repr(self.value)


@dataclass
class Derivation:
    """One PDA derivation site, with the seed tuple as written."""

    path: str
    line: int
    crate: str
    call: str
    domain: str
    arity: int
    segments: tuple[str, ...] = ()
    bump_convention: bool = False


@dataclass
class Function:
    """One function definition, with the source text of its body.

    Half the classes are questions about what appears *together* in one
    function -- a census beside a pin, a blanket refusal inside a frame loop --
    so the readers need the definition as a unit rather than as scattered
    matches.
    """

    path: str
    name: str
    start: int
    end: int
    params: str
    text: str
    in_test_module: bool = False

    @property
    def crate(self) -> str:
        return crate_of(self.path)

    @property
    def is_test(self) -> bool:
        """Whether this definition never runs for a downstream consumer.

        Test-only code is excluded from the frame classes on purpose: a fixture
        is free to be permissive, and the 2026-08-29 audit's own lesson is that
        fixtures are where each side's private beliefs live.  It is *included*
        for the seed classes, where a wrong domain is wrong wherever it sits.
        """

        path = self.path
        return (
            self.in_test_module
            or "/tests/" in path
            or "/program-test/" in path
            or "/test-programs/" in path
            or path.endswith("/tests.rs")
            or "/fixture" in path
        )

    def contains(self, line: int) -> bool:
        return self.start <= line <= self.end


@dataclass
class Survey:
    """Everything the readers extracted, shared across the six classes."""

    root: pathlib.Path
    domains: list[SeedDomain] = field(default_factory=list)
    derivations: list[Derivation] = field(default_factory=list)
    seed_functions: dict[str, set[str]] = field(default_factory=dict)
    asserts: dict[str, str] = field(default_factory=dict)
    functions: list[Function] = field(default_factory=list)

    def enclosing(self, path: str, line: int) -> Function | None:
        """The innermost function definition containing a source line."""

        best: Function | None = None
        for function in self.functions:
            if function.path != path or not function.contains(line):
                continue
            if best is None or function.start > best.start:
                best = function
        return best


# --------------------------------------------------------------------------
# ast-grep plumbing
# --------------------------------------------------------------------------


def sg_binary() -> str:
    for candidate in ("sg", "ast-grep"):
        probe = subprocess.run(
            [candidate, "--version"], capture_output=True, text=True, check=False
        )
        if probe.returncode == 0:
            return candidate
    raise AuditError(
        "ast-grep is not on PATH; install it with `cargo install ast-grep` "
        "(the checker is pattern-driven and has no fallback)"
    )


def sg_run(
    binary: str,
    pattern: str,
    root: pathlib.Path,
    paths: list[str],
    language: str = "rust",
) -> list[dict]:
    """Run one ast-grep pattern and return its structured matches.

    ast-grep exits 1 when a pattern matches nothing, which is a legitimate
    outcome for a checker, so only a hard failure is escalated.
    """

    present = [path for path in paths if (root / path).exists()]
    if not present:
        return []
    command = [binary, "run", "--pattern", pattern, "--lang", language, "--json=compact"]
    command.extend(present)
    result = subprocess.run(
        command, capture_output=True, text=True, cwd=root, check=False
    )
    if result.returncode not in (0, 1):
        raise AuditError(
            f"ast-grep failed on pattern {pattern!r}: {result.stderr.strip()}"
        )
    payload = result.stdout.strip()
    if not payload:
        return []
    try:
        return json.loads(payload)
    except json.JSONDecodeError as error:
        raise AuditError(f"ast-grep emitted unparsable JSON: {error}") from error


def meta(match: dict, name: str) -> str:
    return match["metaVariables"]["single"].get(name, {}).get("text", "")


def match_line(match: dict) -> int:
    return int(match["range"]["start"]["line"]) + 1


def crate_of(path: str) -> str:
    """The owning unit of a source path: the crate root, not the file.

    A domain declared in ``crates/dclutch-record-contract/src/lib.rs`` is owned
    by ``crates/dclutch-record-contract``, and every module inside that crate is
    a legitimate author of its seed tuples.
    """

    parts = pathlib.PurePosixPath(path).parts
    if len(parts) >= 2 and parts[0] in ("crates", "programs"):
        return f"{parts[0]}/{parts[1]}"
    if len(parts) >= 2 and parts[0] == "tools":
        return "/".join(parts[:3]) if len(parts) >= 3 else "/".join(parts[:2])
    if len(parts) >= 1 and parts[0] == "formal":
        return "/".join(parts[:2])
    return parts[0] if parts else path


# --------------------------------------------------------------------------
# literal decoding
# --------------------------------------------------------------------------

_BYTE_STRING = re.compile(r'^b"((?:[^"\\]|\\.)*)"$')
_ESCAPE = re.compile(r"\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)")


def decode_rust_byte_string(literal: str) -> bytes | None:
    """Decode a Rust ``b"..."`` literal to its exact bytes.

    Returns ``None`` for anything that is not a plain byte-string literal --
    a ``concat!``, a const-fn call, a reference to another constant -- because
    measuring a length the checker cannot see would be worse than declining to.
    """

    match = _BYTE_STRING.match(literal.strip())
    if match is None:
        return None
    body = match.group(1)
    out = bytearray()
    index = 0
    while index < len(body):
        if body[index] != "\\":
            out.extend(body[index].encode("utf-8"))
            index += 1
            continue
        escape = _ESCAPE.match(body, index)
        if escape is None:
            return None
        token = escape.group(1)
        if token.startswith("x"):
            out.append(int(token[1:], 16))
        elif token.startswith("u{"):
            out.extend(chr(int(token[2:-1], 16)).encode("utf-8"))
        elif token == "n":
            out.append(0x0A)
        elif token == "r":
            out.append(0x0D)
        elif token == "t":
            out.append(0x09)
        elif token == "0":
            out.append(0x00)
        elif token in ('"', "\\", "'"):
            out.extend(token.encode("utf-8"))
        else:
            return None
        index = escape.end()
    return bytes(out)


# --------------------------------------------------------------------------
# baseline and exceptions
# --------------------------------------------------------------------------


def load_baseline(path: pathlib.Path) -> dict:
    if not path.exists():
        return {"schema": SCHEMA, "findings": {}}
    try:
        loaded = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise AuditError(f"cannot read the baseline at {path}: {error}") from error
    if loaded.get("schema") != SCHEMA:
        raise AuditError(
            f"baseline at {path} declares schema {loaded.get('schema')!r}, "
            f"this checker writes {SCHEMA!r}"
        )
    return loaded


def exception_tags(path: pathlib.Path) -> set[str]:
    """Every verdict tag that ``EXCEPTIONS.md`` actually gives a reason for.

    A tag is a level-3 heading, ``### tag-name``.  The reasons file is not
    decoration: the gate refuses a baseline that uses a tag with no written
    reason, so an exception cannot be accepted by editing JSON alone.
    """

    if not path.exists():
        return set()
    return {
        line[4:].strip()
        for line in path.read_text().splitlines()
        if line.startswith("### ")
    }


def compare(
    findings: list[Finding], baseline: dict, tags: set[str]
) -> tuple[list[str], list[str]]:
    """Diff the survey against the baseline.  Returns (failures, notes)."""

    recorded: dict[str, dict[str, str]] = baseline.get("findings", {})
    current: dict[str, set[str]] = defaultdict(set)
    detail: dict[tuple[str, str], Finding] = {}
    for finding in findings:
        current[finding.code].add(finding.key)
        detail[(finding.code, finding.key)] = finding

    failures: list[str] = []
    notes: list[str] = []

    for code in sorted(set(current) | set(recorded)):
        was = set(recorded.get(code, {}))
        now = current.get(code, set())
        for key in sorted(now - was):
            found = detail[(code, key)]
            failures.append(
                f"NEW {code}: {key}\n"
                f"      {found.path}:{found.line}  {found.detail}"
            )
        for key in sorted(was - now):
            failures.append(
                f"GONE {code}: {key}\n"
                f"      no longer reproduces -- if it was fixed, "
                f"rerun with --write so the register shrinks"
            )

    used = {
        verdict
        for entries in recorded.values()
        for verdict in entries.values()
    }
    for tag in sorted(used - tags):
        failures.append(
            f"UNREASONED {tag}: the baseline accepts findings under this "
            f"verdict but EXCEPTIONS.md gives no reason for it"
        )

    from seam_rules import CLASS_CODES  # noqa: PLC0415

    for name in CLASSES:
        codes = CLASS_CODES.get(name, ())
        total = sum(len(current.get(code, set())) for code in codes)
        notes.append(f"  {name:<12} {total:>4} findings")

    # The verdict census is not decoration.  A register whose entries are all
    # "accepted" reads exactly like a clean tree, and the whole point of the
    # 2026-08-29 audit was that a green surface said nothing about the seams
    # underneath it.  Print what the register actually holds.
    verdicts: dict[str, int] = defaultdict(int)
    for entries in recorded.values():
        for verdict in entries.values():
            verdicts[verdict] += 1
    if verdicts:
        notes.append("")
        for verdict, count in sorted(verdicts.items(), key=lambda pair: -pair[1]):
            notes.append(f"  {verdict:<28} {count:>4}")
    return failures, notes


def write_baseline(
    path: pathlib.Path, findings: list[Finding], previous: dict
) -> None:
    """Retriage: keep every verdict already recorded, tag the rest untriaged."""

    recorded: dict[str, dict[str, str]] = previous.get("findings", {})
    out: dict[str, dict[str, str]] = {}
    for finding in sorted(findings):
        verdict = recorded.get(finding.code, {}).get(finding.key, "untriaged")
        out.setdefault(finding.code, {})[finding.key] = verdict
    path.write_text(
        json.dumps(
            {"schema": SCHEMA, "findings": out},
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Static gate for the six mechanical seam-defect classes.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--root",
        default=str(TOOL_DIR.parent.parent),
        help="repository root to audit (default: this checkout)",
    )
    parser.add_argument(
        "--write", action="store_true", help="retriage findings into the baseline"
    )
    parser.add_argument(
        "--report", action="store_true", help="print every finding, gate or not"
    )
    parser.add_argument(
        "--class",
        dest="only",
        action="append",
        choices=CLASSES,
        help="restrict to one class (repeatable); the gate still needs them all",
    )
    parser.add_argument(
        "--baseline", default=str(BASELINE_PATH), help="baseline register path"
    )
    args = parser.parse_args(argv)

    root = pathlib.Path(args.root).resolve()
    if not (root / "Cargo.toml").exists():
        raise AuditError(f"{root} is not a repository root (no Cargo.toml)")

    from seam_rules import survey_tree, run_classes  # noqa: PLC0415

    binary = sg_binary()
    survey = survey_tree(binary, root)
    selected = tuple(args.only) if args.only else CLASSES
    findings = run_classes(binary, survey, selected)

    if args.report:
        for finding in sorted(findings):
            print(finding.rendered())
        print(f"\n{len(findings)} findings across {len(selected)} classes")

    baseline_path = pathlib.Path(args.baseline)
    if args.write:
        if args.only:
            raise AuditError("--write must retriage every class; drop --class")
        write_baseline(baseline_path, findings, load_baseline(baseline_path))
        print(f"wrote {baseline_path} with {len(findings)} findings")
        return 0

    if args.only:
        print("--class restricts the survey; the gate verdict needs every class")
        return 0

    baseline = load_baseline(baseline_path)
    failures, notes = compare(findings, baseline, exception_tags(EXCEPTIONS_PATH))
    print("seam audit:")
    for note in notes:
        print(note)
    if failures:
        print(f"\n{len(failures)} gate failures:\n")
        for failure in failures:
            print(f"  {failure}")
        print(
            "\nA NEW finding is a seam disagreement this tree did not have. "
            "Fix it, or record it in the baseline with a verdict whose reason "
            "is written in tools/seam-audit/EXCEPTIONS.md."
        )
        return 1
    print("\nno seam drift against the baseline")
    return 0


if __name__ == "__main__":
    sys.path.insert(0, str(TOOL_DIR))
    try:
        sys.exit(main())
    except AuditError as error:
        print(f"seam audit: {error}", file=sys.stderr)
        sys.exit(2)
