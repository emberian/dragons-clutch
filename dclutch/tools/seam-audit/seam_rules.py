#!/usr/bin/env python3
"""The six seam-defect readers.  See ``seam_audit.py`` for the gate itself.

Each reader answers the one question the 2026-08-29 audit asked of every seam:
*which identities does the other side pin, and does this side agree?*  Nothing
here executes protocol code -- every answer is read off source text, so the
checker cannot be fooled by a fixture the same side authored, which is the
failure mode that let all ten of that day's findings ship green.
"""

from __future__ import annotations

import pathlib
import re
from collections import defaultdict

from seam_audit import (
    Function,
    SVM_MAX_PDA_SEED_BYTES,
    AuditError,
    Derivation,
    Finding,
    RUST_ROOTS,
    SeedDomain,
    Survey,
    crate_of,
    decode_rust_byte_string,
    match_line,
    meta,
    sg_run,
)

# Which granular codes belong to which charter class.  The gate groups by code;
# ``--class`` selects by class.
CLASS_CODES: dict[str, tuple[str, ...]] = {
    "SEED_LEN": ("SEED_LEN_OVER_MAX", "SEED_DOMAIN_UNASSERTED"),
    "DERIVATION": (
        "DOMAIN_ARITY_SPLIT",
        "DERIVATION_DOMAIN_ERASED",
        "DOMAIN_RAW_RESTATEMENT",
    ),
    "PIN_CENSUS": ("CENSUS_ARMS_DISAGREE", "CENSUS_OVER_PINNED_FRAME"),
    "UNSET_PIN": ("UNSET_PUBKEY_UNGUARDED", "UNSET_GUARD_PRESENT"),
    "DOMAIN_DUP": ("DOMAIN_BYTES_COLLIDE", "DOMAIN_NAME_BYTES_DISAGREE"),
    "PRIVILEGE": ("TRANSACTION_LEVEL_SIGNER_CENSUS", "PRIVILEGE_PIN_UNEXEMPTED"),
}

# Which ``&[u8]`` constants are PDA seed domains at all.  This discriminator is
# the checker's most load-bearing judgement and it is deliberately use-based:
# the tree declares far more *hash-preimage* domains than seed domains, and a
# preimage domain has no length limit whatever, so a name-driven reader reports
# `dclutch/gauntlet/claims-custody/custody-program-test/keypair-seed/v1` as an
# undereivable address and is simply wrong.  A checker that cries wolf gets
# muted, and a muted checker is worth less than none.
#
# A constant is a PDA seed domain when one of three things is true:
#   derived   -- it appears as a segment of an actual derivation site.
#   exported  -- it appears in the seed tuple a `*_seeds()` / `as_slices()`
#                constructor returns, which is the same thing one indirection
#                out (`CustodyVaultSeedsV1::as_slices()` never names a
#                derivation call).
#   declared  -- its identifier says `PDA_SEED` outright.  This arm exists for
#                exactly SEAM_AUDIT #8: both over-length domains were consumed
#                by nothing at all, so the first two arms would have measured
#                neither, and they are the class's own negative control.
_DECLARED_SEED_NAME = re.compile(r"PDA_SEED|PDA_DOMAIN")
_SEED_CONTEXT_FN = re.compile(r"seed|slices", re.IGNORECASE)
_UPPER_IDENT = re.compile(r"\b([A-Z][A-Z0-9_]{2,})\b")

# A seed segment written as an accessor on a seed-struct binding --
# ``seeds.domain()``, ``seeds.schema_release_id().as_bytes()``.  This is the
# tell that distinguishes the canonical *fix* of SEAM_AUDIT #3 from the defect:
# both spell a literal three-element array at the derivation, and only the
# provenance of the elements says which is which.
_ACCESSOR_SEGMENT = re.compile(r"^&?\s*[a-z_][a-z0-9_]*\s*\.\s*[a-z_][a-z0-9_]*\s*\(")

# Segments that carry the bump rather than identity.  ``create_program_address``
# and ``invoke_signed`` take the bump as a trailing seed, so an arity comparison
# that does not strip it reports every correct pair as a disagreement.
_BUMP_SEGMENT = re.compile(r"bump", re.IGNORECASE)

_DERIVATION_PATTERNS: tuple[tuple[str, str], ...] = (
    ("find_program_address", "Pubkey::find_program_address(&[$$$SEEDS], $PROG)"),
    ("find_program_address", "find_program_address(&[$$$SEEDS], $PROG)"),
    ("create_program_address", "Pubkey::create_program_address(&[$$$SEEDS], $PROG)"),
    ("create_program_address", "create_program_address(&[$$$SEEDS], $PROG)"),
    ("invoke_signed", "invoke_signed($IX, $ACCS, &[&[$$$SEEDS]])"),
)


def _segments(match: dict, name: str = "SEEDS") -> tuple[str, ...]:
    """The seed tuple as written, with ast-grep's comma nodes dropped."""

    raw = match["metaVariables"].get("multi", {}).get(name, [])
    return tuple(
        node["text"].strip()
        for node in raw
        if node["text"].strip() not in ("", ",")
    )


def _identifier(segment: str) -> str:
    """The bare constant name a seed segment names, if it names one.

    Seed segments arrive path-qualified in every combination the tree uses --
    ``FOO``, ``seeds::FOO``, ``dclutch_record_contract::FOO``,
    ``crate::seeds::FOO`` -- and all four are the same domain.
    """

    text = segment.strip()
    if text.startswith("&"):
        text = text[1:].strip()
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)*", text):
        return ""
    return text.rsplit("::", 1)[-1]


# --------------------------------------------------------------------------
# survey
# --------------------------------------------------------------------------


_CONST_PATTERNS = (
    # Shapes A and B: `&[u8] = b"..."`, wrapped or not -- ast-grep is
    # whitespace-insensitive, so one pattern covers both.
    "pub const $NAME: &[u8] = $VAL;",
    "const $NAME: &[u8] = $VAL;",
    # Shape C: the sized reference, `&[u8; 29] = b"..."`.
    "pub const $NAME: &[u8; $LEN] = $VAL;",
    "const $NAME: &[u8; $LEN] = $VAL;",
    # Shapes D and E: the Lean-generated hex arrays, owned and borrowed.  These
    # carry no `b""` literal anywhere, so a byte-string-only reader is blind to
    # every generated domain -- including one that is genuinely unguarded.
    "pub const $NAME: [u8; $LEN] = $VAL;",
    "const $NAME: [u8; $LEN] = $VAL;",
)

_HEX_ARRAY = re.compile(r"^&?\s*\[\s*((?:0x[0-9a-fA-F]{2}\s*,\s*)*0x[0-9a-fA-F]{2})\s*,?\s*\]$")


def _decode_value(literal: str) -> bytes | None:
    """Decode either declaration dialect the tree uses for a domain."""

    text = literal.strip()
    direct = decode_rust_byte_string(text.lstrip("&").strip())
    if direct is not None:
        return direct
    hexed = _HEX_ARRAY.match(text)
    if hexed is None:
        # A domain built by ``concat!``, or aliased to another constant.
        # Measuring a length the reader cannot see would be worse than
        # declining to, so it is skipped rather than guessed at.
        return None
    return bytes(int(byte, 16) for byte in hexed.group(1).split(",") if byte.strip())


def _read_constants(binary: str, root: pathlib.Path) -> list[SeedDomain]:
    """Every byte-valued constant, before the seed/preimage judgement."""

    constants: list[SeedDomain] = []
    seen: set[tuple[str, str]] = set()
    for pattern in _CONST_PATTERNS:
        for match in sg_run(binary, pattern, root, list(RUST_ROOTS)):
            name = meta(match, "NAME")
            path = match["file"]
            if (path, name) in seen:
                continue
            value = _decode_value(meta(match, "VAL"))
            if value is None:
                continue
            seen.add((path, name))
            constants.append(
                SeedDomain(
                    name=name,
                    value=value,
                    path=path,
                    line=match_line(match),
                    crate=crate_of(path),
                )
            )
    return constants


def _seed_context_names(
    binary: str, root: pathlib.Path, derivations: list[Derivation]
) -> set[str]:
    """Constant names the tree actually uses as a PDA seed segment.

    Two sources, because a domain reaches a derivation by either route: named
    directly in the seed tuple, or one indirection out in the ``as_slices()``
    body of a typed seed struct, which is the shape ``CustodyVaultSeedsV1`` and
    twenty-seven siblings use and which never names a derivation call at all.
    """

    names: set[str] = set()
    for derivation in derivations:
        for segment in derivation.segments:
            identifier = _identifier(segment)
            if identifier:
                names.add(identifier)
    for pattern in (
        "pub fn $NAME($$$ARGS) -> $RET { $$$BODY }",
        "pub const fn $NAME($$$ARGS) -> $RET { $$$BODY }",
        "fn $NAME($$$ARGS) -> $RET { $$$BODY }",
    ):
        for match in sg_run(binary, pattern, root, list(RUST_ROOTS)):
            if not _SEED_CONTEXT_FN.search(meta(match, "NAME")):
                continue
            names.update(_UPPER_IDENT.findall(match["text"]))
    return names


def _classify_domains(
    constants: list[SeedDomain], seed_context: set[str]
) -> list[SeedDomain]:
    """Keep the PDA seed domains and drop the hash-preimage domains.

    This is the checker's most load-bearing judgement, and getting it wrong in
    the permissive direction is worse than missing a domain: eighteen
    `dclutch:` literals in this tree are over 32 bytes and every one of them is
    a legitimate *hash* domain, which has no length limit whatever.  The tree
    states the rule itself, at
    ``crates/dclutch-structured-v2-contract/src/seeds.rs:120`` -- "this one is
    hashed rather than seeded, so it is deliberately NOT length-bound."
    """

    kept: list[SeedDomain] = []
    for constant in constants:
        if _DECLARED_SEED_NAME.search(constant.name):
            constant.role = "declared"
        elif constant.name in seed_context:
            constant.role = "derived"
        else:
            continue
        kept.append(constant)
    return kept


def _read_asserts(binary: str, root: pathlib.Path) -> dict[str, str]:
    """Which domain names a compile-time assert holds under the seed maximum.

    Matched on the ``NAME.len()`` mention inside a ``const _: () = ...`` item
    rather than on an exact assert spelling, which is what makes the reader
    survive the five guard dialects the tree actually uses: bare `32`, a named
    `MAX_PDA_SEED_BYTES`, a cross-module path to one, a message or none, and --
    the dialect a plain `assert!(` grep misses entirely -- the *block* form,
    ``const _: () = { assert!(a); assert!(b); };``, under which
    ``dclutch-custody-contract`` and ``dclutch-general-adapter-contract`` guard
    eleven domains the 2026-08-29 audit recorded as unguarded.

    Keyed by constant name across the whole tree, not per crate: a guard in a
    consumer still turns a `cargo check` of this workspace red, which is the
    property being asserted.  Assertions inside ``#[test]`` functions are
    excluded by construction -- they are not ``const _`` items, they fire on
    `cargo test` only, and never for a downstream consumer at all.
    """

    covered: dict[str, str] = {}
    for match in sg_run(binary, "const _: () = $BODY;", root, list(RUST_ROOTS)):
        body = match["text"]
        if ".len()" not in body:
            continue
        where = f"{match['file']}:{match_line(match)}"
        for name in re.findall(r"([A-Z][A-Z0-9_]{2,})\s*\.\s*len\s*\(\s*\)", body):
            covered.setdefault(name, where)
    return covered


def _read_derivations(binary: str, root: pathlib.Path) -> list[Derivation]:
    derivations: list[Derivation] = []
    seen: set[tuple[str, int, str]] = set()
    for call, pattern in _DERIVATION_PATTERNS:
        for match in sg_run(binary, pattern, root, list(RUST_ROOTS)):
            path = match["file"]
            line = match_line(match)
            if (path, line, call) in seen:
                continue
            seen.add((path, line, call))
            segments = _segments(match)
            if not segments:
                continue
            trailing_bump = bool(segments) and bool(_BUMP_SEGMENT.search(segments[-1]))
            arity = len(segments) - (1 if trailing_bump else 0)
            derivations.append(
                Derivation(
                    path=path,
                    line=line,
                    crate=crate_of(path),
                    call=call,
                    domain=_identifier(segments[0]),
                    arity=arity,
                    segments=segments,
                    bump_convention=trailing_bump,
                )
            )
    return derivations


def _read_seed_functions(binary: str, root: pathlib.Path) -> dict[str, set[str]]:
    """Crate -> the seed constructors it exports for other crates to consume.

    These are the single authors a seam is supposed to have: a crate that
    exports ``raw_record_pda_seeds()`` has said, in code, that nobody else
    should be spelling that tuple out.
    """

    exported: dict[str, set[str]] = defaultdict(set)
    for pattern in (
        "pub fn $NAME($$$ARGS) -> $RET { $$$BODY }",
        "pub const fn $NAME($$$ARGS) -> $RET { $$$BODY }",
    ):
        for match in sg_run(binary, pattern, root, list(RUST_ROOTS)):
            name = meta(match, "NAME")
            if "seed" not in name.lower():
                continue
            exported[crate_of(match["file"])].add(name)
    for pattern in ("pub struct $NAME { $$$FIELDS }", "pub struct $NAME<$$$G> { $$$F }"):
        for match in sg_run(binary, pattern, root, list(RUST_ROOTS)):
            name = meta(match, "NAME")
            if "seeds" in name.lower():
                exported[crate_of(match["file"])].add(name)
    return exported


_FN_HEADER = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")


def _code_mask(text: str) -> bytearray:
    """1 for every character that is real code, 0 inside strings and comments.

    Necessary rather than fastidious: this tree's doc comments quote seed
    tuples, census bodies and `fn` signatures constantly, and a brace inside a
    ``//`` line or a ``b"..."`` literal would desynchronise the scanner for the
    rest of the file.
    """

    mask = bytearray(b"\x01") * len(text)
    index = 0
    size = len(text)
    while index < size:
        char = text[index]
        if char == "/" and index + 1 < size and text[index + 1] == "/":
            while index < size and text[index] != "\n":
                mask[index] = 0
                index += 1
            continue
        if char == "/" and index + 1 < size and text[index + 1] == "*":
            depth = 1
            mask[index] = mask[index + 1] = 0
            index += 2
            while index < size and depth:
                if text.startswith("/*", index):
                    depth += 1
                    mask[index] = mask[index + 1] = 0
                    index += 2
                    continue
                if text.startswith("*/", index):
                    depth -= 1
                    mask[index] = mask[index + 1] = 0
                    index += 2
                    continue
                mask[index] = 0
                index += 1
            continue
        if char == "r" and index + 1 < size and text[index + 1] in "#\"":
            probe = index + 1
            hashes = 0
            while probe < size and text[probe] == "#":
                hashes += 1
                probe += 1
            if probe < size and text[probe] == '"':
                terminator = '"' + "#" * hashes
                close = text.find(terminator, probe + 1)
                close = size if close < 0 else close + len(terminator)
                for position in range(index, close):
                    mask[position] = 0
                index = close
                continue
        if char in "\"'":
            quote = char
            probe = index + 1
            while probe < size:
                if text[probe] == "\\":
                    probe += 2
                    continue
                if text[probe] == quote:
                    probe += 1
                    break
                if quote == "'" and probe - index > 3:
                    # A lifetime (`'a`), not a character literal.
                    break
                probe += 1
            for position in range(index, min(probe, size)):
                mask[position] = 0
            index = max(probe, index + 1)
            continue
        index += 1
    return mask


def _match_braces(text: str, mask: bytearray, opening: int) -> int:
    depth = 0
    for index in range(opening, len(text)):
        if not mask[index]:
            continue
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    return len(text) - 1


def _scan_functions(root: pathlib.Path) -> list[Function]:
    """Every function definition, with its parameters and its source text.

    Written by hand rather than with ast-grep, and the reason is a defect this
    checker had: an ast-grep ``fn $NAME(...)`` pattern silently fails to match
    any function carrying an attribute or a ``pub(crate)``, because those are
    children of the same node.  On ``programs/dclutch-core-sbf/src/capability.rs``
    it found 25 of 32 -- and the seven it missed included ``process``, which is
    the exact function holding SEAM_AUDIT #12.  A reader blind to the one
    function its own negative control lives in is worse than no reader, so this
    scans the text and matches braces itself.
    """

    functions: list[Function] = []
    for relative in RUST_ROOTS:
        base = root / relative
        if not base.exists():
            continue
        for path in sorted(base.rglob("*.rs")):
            if "/target/" in str(path):
                continue
            try:
                text = path.read_text(errors="replace")
            except OSError:
                continue
            mask = _code_mask(text)
            starts = [0]
            for index, char in enumerate(text):
                if char == "\n":
                    starts.append(index + 1)

            def line_of(offset: int, table: list[int] = starts) -> int:
                low, high = 0, len(table) - 1
                while low < high:
                    middle = (low + high + 1) // 2
                    if table[middle] <= offset:
                        low = middle
                    else:
                        high = middle - 1
                return low + 1

            # `mod tests { ... }` extents, so a fixture is recognisable as one.
            test_spans: list[tuple[int, int]] = []
            for probe in re.finditer(r"\bmod\s+tests?\b", text):
                if not mask[probe.start()]:
                    continue
                opening = text.find("{", probe.end())
                if opening >= 0:
                    test_spans.append((opening, _match_braces(text, mask, opening)))

            relative_path = str(path.relative_to(root))
            for header in _FN_HEADER.finditer(text):
                if not mask[header.start()]:
                    continue
                cursor = header.end()
                while cursor < len(text) and text[cursor] not in "(<":
                    cursor += 1
                if cursor >= len(text):
                    continue
                # Step over the generic list, then capture the parameters.
                if text[cursor] == "<":
                    depth = 0
                    while cursor < len(text):
                        if mask[cursor] and text[cursor] == "<":
                            depth += 1
                        elif mask[cursor] and text[cursor] == ">":
                            depth -= 1
                            if depth == 0:
                                cursor += 1
                                break
                        cursor += 1
                    while cursor < len(text) and text[cursor] != "(":
                        cursor += 1
                if cursor >= len(text) or text[cursor] != "(":
                    continue
                depth = 0
                params_start = cursor + 1
                while cursor < len(text):
                    if mask[cursor] and text[cursor] == "(":
                        depth += 1
                    elif mask[cursor] and text[cursor] == ")":
                        depth -= 1
                        if depth == 0:
                            break
                    cursor += 1
                params = text[params_start:cursor]
                opening = -1
                for index in range(cursor, len(text)):
                    if not mask[index]:
                        continue
                    if text[index] == ";":
                        break
                    if text[index] == "{":
                        opening = index
                        break
                if opening < 0:
                    # A trait method or an extern declaration: no body to read.
                    continue
                closing = _match_braces(text, mask, opening)
                in_tests = any(
                    span[0] <= header.start() <= span[1] for span in test_spans
                )
                preamble = text[max(0, header.start() - 400) : header.start()]
                functions.append(
                    Function(
                        path=relative_path,
                        name=header.group(1),
                        start=line_of(header.start()),
                        end=line_of(closing),
                        params=params,
                        text=text[header.start() : closing + 1],
                        in_test_module=in_tests or "#[test]" in preamble,
                    )
                )
    return functions


def survey_tree(binary: str, root: pathlib.Path) -> Survey:
    survey = Survey(root=root)
    survey.derivations = _read_derivations(binary, root)
    constants = _read_constants(binary, root)
    survey.domains = _classify_domains(
        constants, _seed_context_names(binary, root, survey.derivations)
    )
    survey.seed_functions = _read_seed_functions(binary, root)
    survey.asserts = _read_asserts(binary, root)
    survey.functions = _scan_functions(root)
    if not survey.domains or not survey.derivations:
        raise AuditError(
            "the survey found no seed domains or no derivation sites at all; "
            "the reader is broken or --root points somewhere else"
        )
    return survey


# --------------------------------------------------------------------------
# class 1 -- seed lengths and the guards that hold them
# --------------------------------------------------------------------------


def class_seed_len(survey: Survey) -> list[Finding]:
    """SEAM_AUDIT #8: two domains over the 32-byte maximum, in the one
    seed-defining crate with no compile-time guard.

    Two findings, and the second is the one that matters.  An over-length
    domain is a dead address today; an *unguarded* domain is the next one,
    because nothing stops a rename from crossing the line.  The audit counted
    51 unguarded domains against 27 guarded, and every guard that existed
    worked.  So the register carries both, and the ratchet turns as guards land.
    """

    findings: list[Finding] = []
    for domain in survey.domains:
        if domain.length > SVM_MAX_PDA_SEED_BYTES:
            findings.append(
                Finding(
                    code="SEED_LEN_OVER_MAX",
                    key=f"{domain.crate}\t{domain.name}",
                    path=domain.path,
                    line=domain.line,
                    detail=(
                        f"{domain.length} bytes > {SVM_MAX_PDA_SEED_BYTES}: "
                        f"{domain.printable!r} has no derivable address for "
                        f"any bump"
                    ),
                )
            )
            continue
        if domain.name not in survey.asserts:
            findings.append(
                Finding(
                    code="SEED_DOMAIN_UNASSERTED",
                    key=f"{domain.crate}\t{domain.name}",
                    path=domain.path,
                    line=domain.line,
                    detail=(
                        f"{domain.length} bytes ({domain.role}), within the "
                        f"maximum today, but no `const _: () = "
                        f"assert!({domain.name}.len() <= 32)` holds it there"
                    ),
                )
            )
    return findings


# --------------------------------------------------------------------------
# class 5 -- one domain, two names; one name, two meanings
# --------------------------------------------------------------------------

_ROLE_STOPWORDS = frozenset(
    {"pda", "seed", "seeds", "domain", "dclutch", "prefix", "const"}
)
_VERSION = re.compile(r"^v\d+$")


def _name_tokens(name: str) -> list[str]:
    tokens = [token.lower() for token in name.split("_") if token]
    return [
        token
        for token in tokens
        if token not in _ROLE_STOPWORDS and not _VERSION.match(token)
    ]


def _value_tokens(value: bytes) -> list[str]:
    try:
        text = value.decode("utf-8").lower()
    except UnicodeDecodeError:
        return []
    tokens = [token for token in re.split(r"[^a-z0-9]+", text) if token]
    return [
        token
        for token in tokens
        if token not in _ROLE_STOPWORDS and not _VERSION.match(token)
    ]


def class_domain_dup(survey: Survey) -> list[Finding]:
    """The GEO shape: the second copy hid under a different name.

    Matched on BYTES and on semantic role, never on the identifier -- that is
    the whole point of the class.  Two readers:

    * ``DOMAIN_BYTES_COLLIDE`` -- one byte string declared under two different
      names.  Two authors each believe they own the address space, and the
      accounts they derive land on top of each other.  Re-declaring the *same*
      name in a second crate is the ordinary Rust/TypeScript mirror and is not
      a finding.
    * ``DOMAIN_NAME_BYTES_DISAGREE`` -- the identifier claims a role the bytes
      do not carry.  This is how a constant renamed without its literal
      following, or a literal copied from a sibling, reads at the seam: the
      name a reviewer trusts and the bytes the chain sees say different things.
    """

    findings: list[Finding] = []

    by_value: dict[bytes, list[SeedDomain]] = defaultdict(list)
    for domain in survey.domains:
        by_value[domain.value].append(domain)

    for value, group in sorted(by_value.items()):
        names = sorted({domain.name for domain in group})
        if len(names) < 2:
            continue
        first = sorted(group, key=lambda d: (d.path, d.line))[0]
        findings.append(
            Finding(
                code="DOMAIN_BYTES_COLLIDE",
                key="\t".join(names),
                path=first.path,
                line=first.line,
                detail=(
                    f"{len(names)} names carry the identical domain "
                    f"{first.printable!r}: "
                    + ", ".join(
                        f"{d.name} ({d.path}:{d.line})"
                        for d in sorted(group, key=lambda d: (d.path, d.line))
                    )
                ),
            )
        )

    for domain in survey.domains:
        wanted = _name_tokens(domain.name)
        carried = _value_tokens(domain.value)
        if not wanted or not carried:
            continue
        shared = [
            token
            for token in wanted
            if any(
                token.startswith(other) or other.startswith(token)
                for other in carried
            )
        ]
        # Deliberately the *total* mismatch and not the partial one.  Partial
        # disagreement is ordinary abbreviation in this tree
        # (``LIABILITY_BASIS_MARKET_SEED_V2`` carries ``dclutch:lbv2:market``)
        # and flagging it would bury the class in noise.  Sharing no segment at
        # all is different in kind: it is what a literal copied from a sibling
        # constant, or a rename the bytes did not follow, reads like.
        if shared:
            continue
        findings.append(
            Finding(
                code="DOMAIN_NAME_BYTES_DISAGREE",
                key=f"{domain.crate}\t{domain.name}",
                path=domain.path,
                line=domain.line,
                detail=(
                    f"the name claims {'/'.join(wanted)} and the bytes "
                    f"{domain.printable!r} share none of it -- a reviewer "
                    f"trusts the name and the chain sees the bytes"
                ),
            )
        )

    # A third reader was written here and deliberately removed: "one role
    # spelled at two versions", grouping domains by their version-stripped
    # tokens.  Every one of its findings was a pair like
    # `dclutch/source-close/v1` beside `.../v2` -- coexisting versions, which
    # is this protocol's intended shape and not a defect at all.  A reader
    # whose findings are all correct-by-design does not become useful by
    # sitting in a baseline; it dilutes the register and trains a reviewer to
    # skim it.  The genuine version defect -- a bump the *bytes* did not
    # follow, as in CLAIMS_FOUNDING_AGGREGATE_SEED_V4 and _V5 -- is already a
    # DOMAIN_BYTES_COLLIDE, which is where it belongs.
    return findings


# --------------------------------------------------------------------------
# class 2 -- one address, two derivations
# --------------------------------------------------------------------------

_PARAM = re.compile(r"(?:^|,)\s*(?:mut\s+)?([a-z_][a-z0-9_]*)\s*:\s*([^,]+)")


def _byte_slice_params(params: str) -> set[str]:
    """Parameter names typed as raw seed material, e.g. ``domain: &[u8]``."""

    return {
        name
        for name, kind in _PARAM.findall(params)
        if "[u8]" in kind.replace(" ", "")
    }


def class_derivation(survey: Survey) -> list[Finding]:
    """SEAM_AUDIT #3 and the dealer-batch fix: one address, spelled twice.

    Three readers, because the class arrives in three grammars and only the
    first is obvious.

    ``DOMAIN_ARITY_SPLIT`` -- one domain derived with two different seed
    counts.  Solana concatenates segments before hashing, so two arities are
    two preimages and no input satisfies both sides.  This is the dealer-batch
    defect (`eae9a0c9`): Trading authenticated a three-seed address for an
    account Custody signs into existence with two, so no batch Custody could
    create was ever authenticatable.

    ``DERIVATION_DOMAIN_ERASED`` -- a derivation whose domain segment is a
    *parameter* rather than a resolvable constant.  This is SEAM_AUDIT #3
    exactly, and it is the reason that defect survived review: with the domain
    erased behind ``domain: &[u8]``, the helper served five record families at
    once and neither a reader nor a checker could see which arity any of them
    wanted.  The Registry's own ``derive_record_pda`` binds its domain from two
    named constants in the same function and is deliberately not a finding --
    the domain is still resolvable there.

    ``DOMAIN_RAW_RESTATEMENT`` -- a seed tuple spelled out in a crate that does
    not own the domain, when the owning crate exports a seed constructor.  Note
    what this reader must *not* flag: the canonical fix of #3 still writes a
    literal three-element array.  What changed is the provenance of the
    elements -- ``seeds.domain()``, ``seeds.schema_release_id().as_bytes()`` --
    so the discriminator is accessor-vs-constant, never the array itself.
    """

    findings: list[Finding] = []
    owner: dict[str, SeedDomain] = {}
    for domain in survey.domains:
        owner.setdefault(domain.name, domain)

    arities: dict[str, dict[int, list[Derivation]]] = defaultdict(
        lambda: defaultdict(list)
    )
    for derivation in survey.derivations:
        if derivation.domain not in owner:
            continue
        # This tree pins a *wrong* spelling on purpose, as an `assert_ne!`
        # against the right one -- `admitted_composition_v3.rs:829` and
        # `dealer_scenario_checkpoint_v1.rs:2767` both say "this is not the
        # address the other program signs", and both were added as the
        # regression guard for the very defect this reader hunts.  Counting a
        # deliberate counterexample as a second spelling would make the
        # checker refuse its own fix.
        function = survey.enclosing(derivation.path, derivation.line)
        if function is not None and "assert_ne!" in function.text:
            continue
        arities[derivation.domain][derivation.arity].append(derivation)

    for name, by_arity in sorted(arities.items()):
        if len(by_arity) < 2:
            continue
        sites = sorted(
            (site for group in by_arity.values() for site in group),
            key=lambda d: (d.path, d.line),
        )
        findings.append(
            Finding(
                code="DOMAIN_ARITY_SPLIT",
                key=f"{owner[name].crate}\t{name}",
                path=sites[0].path,
                line=sites[0].line,
                detail=(
                    f"derived with {sorted(by_arity)} seeds at different "
                    f"sites; Solana hashes the concatenation, so these are "
                    f"different addresses: "
                    + ", ".join(
                        f"{site.arity}@{site.path}:{site.line}" for site in sites[:6]
                    )
                ),
            )
        )

    for derivation in survey.derivations:
        if derivation.domain in owner or not derivation.segments:
            continue
        first = _identifier(derivation.segments[0])
        if not first:
            continue
        function = survey.enclosing(derivation.path, derivation.line)
        if function is None or first not in _byte_slice_params(function.params):
            continue
        findings.append(
            Finding(
                code="DERIVATION_DOMAIN_ERASED",
                key=f"{derivation.path}\t{function.name}",
                path=derivation.path,
                line=derivation.line,
                detail=(
                    f"derives a PDA under `{first}`, a `&[u8]` parameter, so "
                    f"the seed family is not statically knowable and its "
                    f"{derivation.arity}-seed arity is checked against nothing"
                ),
            )
        )

    for derivation in survey.derivations:
        declared = owner.get(derivation.domain)
        if declared is None or declared.crate == derivation.crate:
            continue
        if not survey.seed_functions.get(declared.crate):
            continue
        if any(_ACCESSOR_SEGMENT.match(segment) for segment in derivation.segments[1:]):
            continue
        findings.append(
            Finding(
                code="DOMAIN_RAW_RESTATEMENT",
                key=f"{derivation.domain}\t{derivation.path}",
                path=derivation.path,
                line=derivation.line,
                detail=(
                    f"spells the {derivation.arity}-seed tuple for a domain "
                    f"owned by {declared.crate}, which exports "
                    f"{sorted(survey.seed_functions[declared.crate])[:3]} for "
                    f"exactly this"
                ),
            )
        )
    return findings


# --------------------------------------------------------------------------
# class 3 -- required to repeat, forbidden to repeat
# --------------------------------------------------------------------------

# Name is not polarity in this tree: ``distinct()`` returns true-is-good,
# ``has_duplicate*()`` returns true-is-bad, and
# ``dclutch-release-set-contract``'s ``validate_aliases`` forbids *inconsistent*
# aliasing rather than aliasing at all.  What matters here is only strictness:
# a census that admits no repeat anywhere, versus one that carries an exemption.
_BLANKET_CENSUS = re.compile(
    r"\b(require_distinct|require_distinct_accounts|require_distinct_keys|"
    r"require_distinct_identities|require_handoff_distinct|require_basis_distinct|"
    r"has_duplicate_keys|has_duplicate|has_duplicate_program)\s*\(\s*[a-z_]"
)
_EXEMPT_CENSUS = re.compile(
    r"\b([a-z_]*_except|[a-z_]*alias[a-z_]*)\s*\(\s*[a-z_]"
)
# A frame pinning two of its own coordinates to one key: required-to-repeat.
_COORDINATE_PIN = re.compile(
    r"account\s*\(\s*accounts\s*,[^)]*\)\s*\??\s*\.key\s*[!=]=\s*"
    r"account\s*\(\s*accounts\s*,[^)]*\)\s*\??\s*\.key"
    r"|accounts\s*\[[^\]]+\]\s*\.key\s*[!=]=\s*accounts\s*\[[^\]]+\]\s*\.key"
)


def class_pin_census(survey: Survey) -> list[Finding]:
    """SEAM_AUDIT #12: required to repeat a key, forbidden to repeat any key.

    The 2026-08-29 instance is subtler than a bare blanket census, and a
    checker that only flagged an unconditional ``require_distinct`` would have
    missed it.  ``480e18f0`` fixed *one arm of an if* and left the other:

        if request.action == Action::CloseCapability {
            require_close_capability_aliases(accounts, ...)?;   // alias-aware
        } else {
            require_distinct(accounts)?;                        // blanket
        }

    over one frame whose alias structure does not depend on the action.  So the
    generalised reader is ``CENSUS_ARMS_DISAGREE``: one function reaching for
    two censuses of different strictness has, by construction, two beliefs
    about whether its own frame repeats a key, and at most one can be right.

    ``CENSUS_OVER_PINNED_FRAME`` is the class stated directly: a function that
    pins two coordinates equal *and* runs a census admitting no repeat.  The
    canonical fix is not the absence of a census but a two-phase one -- pin
    each pair positively, then run all-pairs with exactly those excused -- so
    an exemption-aware census beside a pin is the *good* shape and is not
    flagged.
    """

    findings: list[Finding] = []
    for function in survey.functions:
        if function.is_test:
            continue
        body = function.text
        blanket = _BLANKET_CENSUS.search(body)
        if blanket is None:
            continue
        exempt = _EXEMPT_CENSUS.search(body)
        if exempt is not None:
            findings.append(
                Finding(
                    code="CENSUS_ARMS_DISAGREE",
                    key=f"{function.path}\t{function.name}",
                    path=function.path,
                    line=function.start,
                    detail=(
                        f"reaches for both `{blanket.group(1)}` and "
                        f"`{exempt.group(1)}` over one frame: two beliefs "
                        f"about whether this frame repeats a key, and the "
                        f"frame's alias structure does not depend on which "
                        f"branch was taken"
                    ),
                )
            )
            continue
        pin = _COORDINATE_PIN.search(body)
        if pin is None:
            continue
        findings.append(
            Finding(
                code="CENSUS_OVER_PINNED_FRAME",
                key=f"{function.path}\t{function.name}",
                path=function.path,
                line=function.start,
                detail=(
                    f"pins two coordinates to one key and then runs "
                    f"`{blanket.group(1)}`, which admits no repeat anywhere: "
                    f"required to repeat and forbidden to repeat, "
                    f"unsatisfiable for every input"
                ),
            )
        )
    return findings


# --------------------------------------------------------------------------
# class 4 -- the unset pubkey
# --------------------------------------------------------------------------

_DEFAULT_GUARD = re.compile(
    r"[!=]=\s*&?\s*Pubkey::default\(\)"
    r"|Pubkey::default\(\)\s*[!=]="
    r"|[!=]=\s*\[\s*0(_u8|u8)?\s*;\s*32\s*\]"
    r"|\b(is_zero|nonzero|nonzero_array|require_nonzero)\s*\("
)
_SYSTEM_KEY_PIN = re.compile(r"\.key\s*[!=]=\s*&?\s*system_program::ID")
_WIRE_KEY_COMPARISON = re.compile(
    r"\.key\s*\.\s*to_bytes\s*\(\s*\)\s*[!=]=\s*[a-z_]+\.[a-z_]+"
    r"|\.key\s*\.\s*to_bytes\s*\(\s*\)\s*[!=]=\s*[a-z_]+\.[a-z_]+\(\)"
)


def class_unset_pin(survey: Survey) -> list[Finding]:
    """The one class of the six with no 2026-08-29 defect behind it.

    Stated plainly rather than dressed up: the charter described this class as
    "swept clean today", and it was not -- the audit records no default-pubkey
    finding and no commit that day touches the pattern.  The class is nearly
    absent from on-chain code by *construction*: the ``no_std`` contract crates
    have no ``Pubkey`` at all, so they guard identities with ``fn
    is_zero(&[u8; 32])`` and 104 siblings, and the default pubkey is simply not
    a value they can express.

    So this reader is a ratchet on presence rather than a hunt for absence.
    ``UNSET_GUARD_PRESENT`` inventories every existing guard, and the gate's
    two-way comparison means deleting one fails as loudly as adding a defect --
    which is the only useful thing to assert about a class that is already
    clean.  ``UNSET_PUBKEY_UNGUARDED`` is the forward-looking half: a frame
    that pins the System program by key *and* authenticates a coordinate
    against a wire-supplied pubkey, with nothing anywhere in it refusing the
    all-zero one.
    """

    findings: list[Finding] = []
    guards: dict[str, int] = {}
    for function in survey.functions:
        if function.is_test:
            continue
        body = function.text
        guarded = _DEFAULT_GUARD.search(body)
        if guarded is not None:
            # Keyed by FILE, not by function, and the tradeoff is deliberate.
            # Function-level keys inventoried 586 guards and would have failed
            # the gate twice -- once GONE, once NEW -- every time anyone renamed
            # or moved a guarded function, for a class with no known defect
            # behind it.  A ratchet nobody can live with gets switched off, and
            # then it guards nothing.  File-level keys still refuse the thing
            # worth refusing, a file quietly losing its last unset-pubkey
            # guard, at a fraction of the churn.
            guards.setdefault(function.path, function.start)
            continue
        if _SYSTEM_KEY_PIN.search(body) and _WIRE_KEY_COMPARISON.search(body):
            findings.append(
                Finding(
                    code="UNSET_PUBKEY_UNGUARDED",
                    key=f"{function.path}\t{function.name}",
                    path=function.path,
                    line=function.start,
                    detail=(
                        "pins the System program by key and authenticates a "
                        "coordinate against a wire pubkey, with no guard "
                        "refusing the all-zero one"
                    ),
                )
            )
    for path, line in sorted(guards.items()):
        findings.append(
            Finding(
                code="UNSET_GUARD_PRESENT",
                key=path,
                path=path,
                line=line,
                detail=(
                    "refuses the unset pubkey somewhere in this file; recorded "
                    "so the gate fails if the last such guard here is deleted"
                ),
            )
        )
    return findings


# --------------------------------------------------------------------------
# class 6 -- privileges are a property of the transaction
# --------------------------------------------------------------------------

_BLANKET_SIGNER_REFUSAL = re.compile(
    r"\.any\s*\(\s*\|\s*[a-z_]+\s*\|\s*[a-z_]+\.is_signer\s*\)"
    r"|if\s+[a-z_]+\.is_signer\s*(\|\||\{|$)"
)
_FRAME_LOOP = re.compile(
    r"for\s*\(\s*[a-z_]+\s*,\s*[a-z_]+\s*\)\s*in\s+[a-z_]+\s*\.iter\(\)\s*\.enumerate\(\)"
    r"|\.iter\(\)\s*\.enumerate\(\)\s*\.any"
    r"|\.iter\(\)\s*\.any"
)
_PRIVILEGE_EXEMPTION = re.compile(
    r"[a-z_]*_pinned\s*=|index\s*!=\s*[A-Z][A-Z0-9_]+|child_index\s*!=\s*\d"
    r"|writability_is_free|_writability_"
)

# The signer half's exemption, and it is the *harm statement's own negation*.
#
# This reader's complaint is precise: a blanket signer refusal is dead "for any
# builder that pays with an account it also names".  A function that refuses
# the payer being named in the frame at all has closed exactly that hole, in
# code, one line from the census -- so the harm cannot occur and the site is
# not a finding.
#
# That the writability half already honours an in-place exemption
# (``_PRIVILEGE_EXEMPTION``, and the README says so: "the presence of any
# exemption is what keeps a site off this list") while the signer half honoured
# none was an asymmetry in the reader rather than a fact about the code.
#
# It does not weaken the live control.  SEAM_AUDIT #13b
# (``authenticate_expired_checkpoint_v1``) has no carve-out of any kind, and
# its builder *places the campaign payer* at ``FUNDING_ABORT_FUNDING_SOURCE``;
# that is what a site looks like when the answer is yes.
_PAYER_CARVE_OUT = re.compile(
    r"\.key\s*==\s*[a-z_]*payer|[a-z_]*payer\s*==\s*[a-z_.]*\.key"
    r"|\.pubkey\s*==\s*[a-z_]*payer|[a-z_]*payer\s*==\s*[a-z_.]*\.pubkey"
    r"|is_signer\s*!=\s*\("
)

# Tokens that make a branch a refusal rather than a classification.
_REFUSAL_TOKEN = re.compile(r"return\s+Err|Err\(|refusal\(|bail!|Error::|_ERROR")


def _guarded_block(body: str, start: int) -> str:
    """The balanced ``{ .. }`` block that opens at or after ``start``.

    Returns the whole remainder when the braces do not balance, which keeps the
    caller's test conservative: an unparseable body is still reported.
    """

    open_at = body.find("{", start)
    if open_at < 0:
        return body[start:]
    depth = 0
    for index in range(open_at, len(body)):
        char = body[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return body[open_at : index + 1]
    return body[open_at:]


def _censuses_signers(body: str) -> bool:
    """Whether this body *refuses* on a blanket signer read.

    The ``if x.is_signer {`` spelling is not by itself a refusal.  It is also
    how a builder *classifies* a coordinate it is about to emit --
    ``terminal_sequence.rs`` maps it straight into
    ``TerminalAddressClassV1::InlineSigner`` and returns no error at all -- and
    reporting that as an always-refuses frame is the reader mistaking a read
    for a rejection.  So the ``if`` form has to reach a refusal in its own
    guarded block; the ``.any(..)`` form is already a predicate inside one.
    """

    for match in _BLANKET_SIGNER_REFUSAL.finditer(body):
        if match.group(0).lstrip().startswith("if"):
            if not _REFUSAL_TOKEN.search(_guarded_block(body, match.start())):
                continue
        return True
    return False


def class_privilege(survey: Survey) -> list[Finding]:
    """SEAM_AUDIT #13b and the FAMS principle behind it.

    *An exact-privilege census is a constraint on the whole transaction, not on
    your instruction.*  Solana merges account privileges across the
    instructions of one transaction, and the fee payer is message key 0, so it
    reads ``is_signer == true`` in every instruction that names it regardless
    of the ``AccountMeta`` flag it was given.

    ``TRANSACTION_LEVEL_SIGNER_CENSUS`` therefore reads a blanket
    ``is_signer`` refusal over a whole frame as a finding on sight.  It is not
    a constraint this instruction can express: it says the caller may not have
    built the rest of its transaction a particular way, and it makes the frame
    unsatisfiable the moment a builder places the payer in it -- which is
    exactly what ``5ca145e8`` did to the three abort routes, so an expired
    founding can never be unwound.

    ``PRIVILEGE_PIN_UNEXEMPTED`` is the writability half, the shape
    ``16351a13`` fixed on Custody: an exact three-way privilege census over
    every coordinate with no exemption anywhere.  Custody pinned the checkpoint
    readonly while its documented atomic partner, Trading's ingest, must take
    it writable -- so the pin was never a constraint on Custody's own
    instruction at all.  The fix is not a relaxation but an exemption, one
    coordinate wide and reasoned in place; the presence of any exemption is
    what keeps a site off this list.
    """

    findings: list[Finding] = []
    for function in survey.functions:
        if function.is_test:
            continue
        body = function.text
        if not _FRAME_LOOP.search(body):
            continue
        if _censuses_signers(body) and not _PAYER_CARVE_OUT.search(body):
            findings.append(
                Finding(
                    code="TRANSACTION_LEVEL_SIGNER_CENSUS",
                    key=f"{function.path}\t{function.name}",
                    path=function.path,
                    line=function.start,
                    detail=(
                        "refuses every signer across the whole frame, but "
                        "is_signer is a transaction-level property: the fee "
                        "payer reads true here whatever meta it was given, so "
                        "this frame is dead for any builder that pays with an "
                        "account it also names"
                    ),
                )
            )
        # No `continue`: the two halves are separate facts about the same
        # frame, and a function can carry both.  Skipping the writability test
        # once the signer test fired made the class silently under-report --
        # twelve sites had a pin finding hidden behind a signer finding, so
        # fixing the signer half made them surface as if they were new.  A gate
        # that reports one defect per function is a gate that hides the second
        # one behind the first, which is this tool's own subject matter.
        if "is_writable !=" in body and not _PRIVILEGE_EXEMPTION.search(body):
            findings.append(
                Finding(
                    code="PRIVILEGE_PIN_UNEXEMPTED",
                    key=f"{function.path}\t{function.name}",
                    path=function.path,
                    line=function.start,
                    detail=(
                        "pins exact writability on every coordinate with no "
                        "exemption; privileges merge across the instructions "
                        "of one transaction, so a readonly pin here also "
                        "constrains the caller's other instructions"
                    ),
                )
            )
    return findings


# --------------------------------------------------------------------------
# dispatch
# --------------------------------------------------------------------------

_READERS = {
    "SEED_LEN": class_seed_len,
    "DERIVATION": class_derivation,
    "PIN_CENSUS": class_pin_census,
    "UNSET_PIN": class_unset_pin,
    "DOMAIN_DUP": class_domain_dup,
    "PRIVILEGE": class_privilege,
}


def run_classes(
    binary: str, survey: Survey, selected: tuple[str, ...]
) -> list[Finding]:
    findings: list[Finding] = []
    for name in selected:
        reader = _READERS.get(name)
        if reader is None:
            continue
        findings.extend(reader(survey))
    return sorted(findings)
