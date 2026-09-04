#!/usr/bin/env python3
"""Corroborate a devnet route-witness document against Solana devnet itself.

`docs/reference/routes.md` says a campaign binding is a CLAIM and that
corroboration is a separate pipeline.  For the twenty-three localhost campaigns
that pipeline is `dclutch-route-census observe`, which cross-checks every
claimed route against the finalized transaction's own `Program <id> invoke`
lines.  Devnet had no such pipeline at all: cohort evidence lives in prose
markdown in five different textual shapes, sixteen of its signatures are
elided to `X...Y`, and no document anywhere names a census route id.  So a
route driven on a public chain for three cohorts running read as
NEVER-EXECUTED, because the register had no channel a devnet transaction could
reach it through.

This is that channel, and it applies `census observe`'s rule unchanged:

  1. the signature is FINALIZED on devnet, and its error matches the outcome;
  2. the outer instruction's first eight bytes are the declared magic, sent to
     the declared program -- so the route the magic dispatches is the chain's
     own account of what ran, not a reading of a document;
  3. every claimed route's program appears in the transaction's own
     `Program <address> invoke [n]` log lines.

A claimed route whose program the chain does NOT show invoked is dropped into
`not_corroborated` with the reason, never credited.  That is the point: the
first run of this tool dropped `registry/*` from all three cohort-13 founding
records, because tier 1's bindings for the same magics claim a Registry
reauthentication that the devnet cohort does not perform.  A checker that
credited it would have been a mirror.

usage:
  corroborate.py --check              verify every docs/evidence/witnesses/*.json
  corroborate.py --write FILE         refresh one document's chain-derived fields
  corroborate.py --discover ...       build a document from a cohort's signatures

`--discover` is how a cohort is meant to reach the register, and it exists so
that nobody authors a route claim. It harvests the base58 signatures out of a
cohort's evidence documents, asks devnet what each transaction actually sent,
reads the outer instruction's first eight bytes, and resolves THAT magic to the
census route whose dispatch selects on it -- either directly, or through the
`is_*(instruction_data)` predicate the census recorded as the route's selector.
A resolution is kept only when the route's own program is the program the
instruction was sent to, which is not a formality: `DCLTHOT3` is mirrored by
`registry/hot_continuation_v2::process`, and a resolver without that guard
credits a Registry route for a Trading transaction.

`--source` is repeatable. A cohort's evidence lives in prose AND in the
terminal-sequence journals its runner wrote, and the second is where the acts
that happened last are: `direct_begin_retiring_v1` executed on devnet at slot
492,898,053 and the only artifact naming its signature was a job-directory
journal. How a source is read depends on the source: a file that names its
signatures in a `"signature"` or `"expectedSignature"` FIELD is read through
those fields only, because a machine-written record knows which of its base58
runs are transaction signatures and a blind scan of one does not -- a founding
log yielded 582 base58-shaped strings and six submitted transactions. A file
with no such field is prose and is scanned whole.

EIGHT BYTES ARE NOT ALWAYS ONE ROUTE, and the disambiguation is read from the
tree exactly as the magic is. `DCLTCRQ2` selects ELEVEN Core routes: the magic
says the instruction is a `Request` and the `Action` variant inside it says
which arm runs. So this tool resolves the variant the way the program does --
`Request::decode`'s own `decode_action(read_byte(input, REQUEST_ACTION_OFFSET))`
names the offset, `decode_action`'s match arms name the tag for each variant,
and both constants are `const` literals in the same file that declares the
magic. The byte at that offset in the chain's own instruction data picks the
variant; the census's `variant` selectors pick the routes. Where a variant
still names more than one route -- `Action::Retire` names four -- the census's
`length` selector separates them by the instruction's own length, which is the
guard the dispatch writes (`Action::Retire if instruction_data.len() == ..`).
Two routes can survive both, and that is not ambiguity: it is a dispatch arm
and the function it calls, both of which ran. Before this the tool dropped
every Core signature across three cohorts with "resolves to no core route",
which made the program the protocol drives most the one program invisible to
its own devnet witness channel.

The RPC key is read from ~/.helius-key at use time and never written anywhere;
the endpoint is redacted out of every message this tool prints.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import urllib.error
import urllib.request

SCHEMA = "dclutch-devnet-route-witness-v1"
B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
REPO = pathlib.Path(__file__).resolve().parents[3]
WITNESS_DIR = REPO / "docs" / "evidence" / "witnesses"


def b58decode(text: str) -> bytes:
    value = 0
    for character in text:
        value = value * 58 + B58.index(character)
    body = value.to_bytes((value.bit_length() + 7) // 8, "big")
    return b"\0" * (len(text) - len(text.lstrip("1"))) + body


def rpc_url() -> str:
    key = (pathlib.Path.home() / ".helius-key").read_text().strip()
    if not key:
        raise SystemExit("corroborate: ~/.helius-key is empty")
    return f"https://devnet.helius-rpc.com/?api-key={key}"


def fetch(url: str, signature: str) -> dict:
    body = json.dumps(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                signature,
                {
                    "encoding": "json",
                    "maxSupportedTransactionVersion": 0,
                    "commitment": "finalized",
                },
            ],
        }
    ).encode()
    request = urllib.request.Request(url, body, {"Content-Type": "application/json"})
    try:
        payload = json.loads(urllib.request.urlopen(request, timeout=60).read())
    except urllib.error.URLError as error:  # network, not evidence
        raise SystemExit(f"corroborate: devnet RPC unreachable: {error.reason}") from None
    if "error" in payload:
        raise SystemExit(f"corroborate: RPC refused {signature[:12]}...: {payload['error'].get('message')}")
    return payload.get("result")


def observe(result: dict) -> dict:
    """What the CHAIN says about this transaction. Nothing here is authored."""
    message = result["transaction"]["message"]
    keys = list(message["accountKeys"])
    loaded = result["meta"].get("loadedAddresses") or {}
    keys += loaded.get("writable", []) + loaded.get("readonly", [])
    instructions = []
    for entry in message["instructions"]:
        data = b58decode(entry["data"]) if entry["data"] else b""
        # The whole payload, not only the head: the eight bytes name the
        # request family and a discriminant INSIDE the payload names the arm.
        instructions.append(
            {
                "program": keys[entry["programIdIndex"]],
                "head": data[:8],
                "len": len(data),
                "data": data,
            }
        )
    invoked = []
    for line in result["meta"]["logMessages"]:
        if not line.startswith("Program ") or " invoke [" not in line:
            continue
        address = line.split(" ", 2)[1]
        if address not in invoked:
            invoked.append(address)
    return {
        "slot": result["slot"],
        "error": result["meta"]["err"],
        "compute_units": result["meta"].get("computeUnitsConsumed"),
        "instructions": instructions,
        "programs_invoked": sorted(invoked),
    }


def check_record(record: dict, programs: dict, chain: dict) -> tuple[list[str], list[dict], dict]:
    """Return (problems, corroborated routes, chain-derived fields)."""
    problems: list[str] = []
    label = record.get("stage", "?")

    expected_error = record.get("outcome") == "refused"
    if (chain["error"] is not None) != expected_error:
        problems.append(
            f"{label}: outcome is {record.get('outcome')!r} but the chain reports err={chain['error']!r}"
        )

    # 2. the outer instruction's own bytes.
    magic = record.get("magic")
    program_label = record.get("program")
    address = programs.get(program_label)
    if address is None:
        problems.append(f"{label}: program {program_label!r} is not in this document's `programs` map")
    elif magic is not None:
        wanted = magic.encode()
        matched = [
            entry
            for entry in chain["instructions"]
            if entry["program"] == address and entry["head"] == wanted
        ]
        if not matched:
            heads = ", ".join(
                f"{entry['program'][:8]}/{entry['head'].hex()}" for entry in chain["instructions"]
            )
            problems.append(
                f"{label}: no top-level instruction to {program_label} begins with {magic}; the chain shows [{heads}]"
            )
        elif record.get("instruction_data_len") is not None and matched[0]["len"] != record["instruction_data_len"]:
            problems.append(
                f"{label}: instruction data is {matched[0]['len']} bytes, the document says "
                f"{record['instruction_data_len']}"
            )
        # 2b. the discriminant INSIDE the payload, when the magic needed one to
        # reach a single route. `--discover` writes the offset and tag it read
        # out of the codec; here the chain's own bytes have to still carry that
        # tag, or the record names an arm the transaction did not take.
        action = record.get("action") if matched else None
        if action is not None:
            offset, tag = action.get("offset"), action.get("tag")
            data = matched[0]["data"]
            if offset is None or tag is None:
                problems.append(f"{label}: `action` records no offset/tag to check")
            elif offset >= len(data):
                problems.append(
                    f"{label}: `action` reads offset {offset} of a {len(data)}-byte instruction"
                )
            elif data[offset] != tag:
                problems.append(
                    f"{label}: the byte at offset {offset} is {data[offset]}, but the document "
                    f"says {tag} ({action.get('enum')}::{action.get('variant')})"
                )

    # 3. every claimed route's program must be one the chain shows invoked.
    invoked_labels = {
        name for name, addr in programs.items() if addr in chain["programs_invoked"]
    }
    corroborated, dropped = [], []
    for route in record.get("routes", []):
        owner = route.split("/", 1)[0]
        if owner in invoked_labels:
            corroborated.append(route)
        else:
            dropped.append(
                {
                    "route": route,
                    "reason": f"the transaction's own logs show no invoke of the {owner} program",
                }
            )

    derived = {
        "slot": chain["slot"],
        "compute_units": chain["compute_units"],
        "programs_invoked": [
            name for name, addr in sorted(programs.items()) if addr in chain["programs_invoked"]
        ],
        "routes_corroborated": corroborated,
        "not_corroborated": dropped,
    }
    return problems, corroborated, derived


def process(path: pathlib.Path, url: str, write: bool) -> tuple[int, int]:
    document = json.loads(path.read_text())
    if document.get("schema") != SCHEMA:
        raise SystemExit(f"corroborate: {path.name} is not a {SCHEMA} document")
    programs = document["programs"]
    problems: list[str] = []
    corroborated_routes: set[str] = set()

    for record in document["records"]:
        result = fetch(url, record["signature"])
        if result is None:
            problems.append(
                f"{record.get('stage','?')}: devnet has no finalized transaction "
                f"{record['signature'][:12]}..."
            )
            continue
        chain = observe(result)
        found, routes, derived = check_record(record, programs, chain)
        problems.extend(found)
        corroborated_routes.update(routes)
        if write:
            record.update(derived)
        else:
            for key, value in derived.items():
                if record.get(key) != value:
                    problems.append(
                        f"{record.get('stage','?')}: `{key}` in the document is not what the chain "
                        f"reports (document {record.get(key)!r}, chain {value!r})"
                    )

    if write:
        document["corroborated_route_count"] = len(corroborated_routes)
        temporary = path.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(document, indent=2) + "\n")
        temporary.replace(path)
        print(f"corroborate: wrote {path.name} ({len(corroborated_routes)} routes corroborated)")
    elif document.get("corroborated_route_count") != len(corroborated_routes):
        problems.append(
            f"corroborated_route_count is {document.get('corroborated_route_count')}, "
            f"the chain corroborates {len(corroborated_routes)}"
        )

    for problem in problems:
        print(f"corroborate: {path.name}: {problem}", file=sys.stderr)
    return len(problems), len(corroborated_routes)



# ------------------------------------------------------------------ discover

SIGNATURE = re.compile(r"\b[1-9A-HJ-NP-Za-km-z]{86,88}\b")
# A machine-written record names its signatures in a field. A blind base58 scan
# of one reads its base64 payloads too: `log-22-found-general.txt` carries 582
# base58-shaped runs and six submitted transactions.
# Any field whose NAME ends in "signature" holds one: a runner writes
# `expectedSignature`, `activationSignature` and `signature` for three stages of
# the same act, and keying on only the bare name reads a document's address-table
# transactions while missing the act it was written to record.
KEYED_SIGNATURE = re.compile(
    r'"[A-Za-z]*[Ss]ignature"\s*:\s*"([1-9A-HJ-NP-Za-km-z]{86,88})"'
)
MAGIC_CONST = re.compile(
    r'const\s+([A-Z0-9_]+)\s*:\s*\[u8;\s*8\]\s*=\s*\*b"([A-Za-z0-9]{8})"'
)
PREDICATE = re.compile(
    r"pub fn (is_[a-z0-9_]+)\(\s*instruction_data:\s*&\[u8\][^)]*\)\s*->\s*bool\s*\{(.*?)\n\}",
    re.S,
)

# The discriminant reader. Every name below is a `const` in the same file that
# declares the magic, so nothing here is authored: the offset comes from the
# decoder's own `read_byte` call, the tags from the decoder's own match arms.
USIZE_CONST = re.compile(r"const\s+([A-Z0-9_]+)\s*:\s*usize\s*=\s*(\d+)\s*;")
U8_CONST = re.compile(r"const\s+([A-Z0-9_]+)\s*:\s*u8\s*=\s*(\d+)\s*;")
EXACT_MAGIC = re.compile(r"exact_magic\(\s*(\w+)\s*,\s*([A-Z0-9_]+)\s*,\s*&([A-Z0-9_]+)\s*\)")
DECODE_CALL = re.compile(
    r"(decode_[a-z0-9_]+)\(\s*read_byte\(\s*(\w+)\s*,\s*([A-Z0-9_]+)\s*\)\s*\?\s*\)"
)
DECODE_FN = re.compile(
    r"fn (decode_[a-z0-9_]+)\(\s*tag:\s*u8\s*\)\s*->\s*Result<\s*([A-Za-z0-9_]+)\s*,[^>]*>\s*\{(.*?)\n\}",
    re.S,
)
DECODE_ARM = re.compile(r"([A-Z0-9_]+)\s*=>\s*Ok\(\s*([A-Za-z0-9_]+)::([A-Za-z0-9_]+)\s*\)")
FN_HEAD = re.compile(r"\bfn\s+[a-z0-9_]+\s*[(<]")


def function_bodies(text: str):
    """(body,) for each `fn` in a source file, by brace matching.

    The binding between a magic and a discriminant is "the same decode reads
    both", and the honest way to say "the same decode" is the enclosing
    function -- not a shared name prefix. `STATE_MAGIC_OFFSET` and
    `REQUEST_MAGIC_OFFSET` are both 0 in this codec and belong to different
    structs; only the function tells them apart.
    """
    for head in FN_HEAD.finditer(text):
        start = text.find("{", head.end() - 1)
        if start < 0:
            continue
        depth = 0
        for index in range(start, len(text)):
            character = text[index]
            if character == "{":
                depth += 1
            elif character == "}":
                depth -= 1
                if depth == 0:
                    yield text[start : index + 1]
                    break


def discriminant_index(paths) -> dict[str, dict]:
    """magic constant name -> {enum, offset, tags} read out of its own decoder.

    Eight bytes say WHICH REQUEST FAMILY; a tag inside the payload says which
    arm. `Request::decode` writes both reads in one body --
    `exact_magic(input, REQUEST_MAGIC_OFFSET, &CORE_REQUEST_MAGIC)` and
    `decode_action(read_byte(input, REQUEST_ACTION_OFFSET)?)` -- so the pair is
    a fact of the source, not a convention this tool invents. The magic's own
    offset must be 0: that is what makes the instruction data's first eight
    bytes, which is all `observe` matched, the same bytes the decoder checked.
    """
    index: dict[str, dict] = {}
    for path in paths:
        try:
            text = path.read_text()
        except OSError:
            continue
        if "exact_magic(" not in text:
            continue
        usize = {m.group(1): int(m.group(2)) for m in USIZE_CONST.finditer(text)}
        u8 = {m.group(1): int(m.group(2)) for m in U8_CONST.finditer(text)}
        decoders = {}
        for match in DECODE_FN.finditer(text):
            tags = {}
            for arm in DECODE_ARM.finditer(match.group(3)):
                if arm.group(1) in u8:
                    tags[u8[arm.group(1)]] = (arm.group(2), arm.group(3))
            if tags:
                decoders[match.group(1)] = (match.group(2), tags)
        for body in function_bodies(text):
            anchors = [
                (variable, magic)
                for variable, offset, magic in (m.groups() for m in EXACT_MAGIC.finditer(body))
                if usize.get(offset) == 0
            ]
            if len(anchors) != 1:
                continue  # two anchored magics in one body is not one family
            variable, magic_constant = anchors[0]
            for call in DECODE_CALL.finditer(body):
                function, read_variable, offset_constant = call.groups()
                if read_variable != variable or function not in decoders:
                    continue
                if offset_constant not in usize:
                    continue
                enum, tags = decoders[function]
                index.setdefault(magic_constant, {})[enum] = {
                    "offset": usize[offset_constant],
                    "offset_constant": offset_constant,
                    "tags": tags,
                    "source": str(path.relative_to(REPO)),
                }
    return index


def source_files() -> list[pathlib.Path]:
    found = []
    for root in ("programs", "crates"):
        for path in (REPO / root).rglob("*.rs"):
            if "/target/" not in str(path):
                found.append(path)
    return found


def magic_routes(inventory: dict, paths) -> tuple[dict, dict, dict]:
    """(magic -> {(program label, route id)}, magic -> constant names, literal usize consts).

    Two dispatch shapes reach a route from eight bytes. The census records the
    first directly, as a `magic` selector. The second it records as a
    `predicate` selector naming an `is_*` function, and the eight bytes are
    inside that function's body -- so the constant is found in the sources, the
    predicate that compares against it is found in the same file, and the route
    is the one whose selector names that predicate.
    """
    constants: dict[str, set[tuple[str, str]]] = {}
    predicates: dict[str, set[str]] = {}
    literals: dict[str, set[int]] = {}
    for path in paths:
        try:
            text = path.read_text()
        except OSError:
            continue
        for match in USIZE_CONST.finditer(text):
            literals.setdefault(match.group(1), set()).add(int(match.group(2)))
        found = list(MAGIC_CONST.finditer(text))
        if not found:
            continue
        for match in found:
            constants.setdefault(match.group(2), set()).add((match.group(1), str(path)))
        names = {match.group(1) for match in found}
        for predicate in PREDICATE.finditer(text):
            body = predicate.group(2)
            for name in names:
                if re.search(rf"\b{re.escape(name)}\b", body):
                    for magic, held in constants.items():
                        if any(constant == name for constant, _ in held):
                            predicates.setdefault(magic, set()).add(predicate.group(1))

    routes: dict[str, set[tuple[str, str]]] = {}
    names: dict[str, set[str]] = {
        magic: {constant for constant, _ in held} for magic, held in constants.items()
    }
    for program in inventory["programs"]:
        for route in program["routes"]:
            for selector in route.get("selectors", []):
                if selector.get("kind") == "magic" and selector.get("ascii"):
                    routes.setdefault(selector["ascii"], set()).add((program["label"], route["id"]))
                    # The census already resolved this magic to the constant
                    # that declares it, and it can read shapes the regex above
                    # cannot: the Lean emitters write a magic as a byte array,
                    # not as `*b"XXXXXXXX"`, so `CORE_REQUEST_MAGIC` appears in
                    # no source scan at all. Taking the name from the inventory
                    # is taking it from the reader that walked the dispatch.
                    if selector.get("constant"):
                        names.setdefault(selector["ascii"], set()).add(
                            selector["constant"].split("::")[-1]
                        )
                elif selector.get("kind") == "predicate":
                    name = selector["function"].split("::")[-1]
                    for magic, held in predicates.items():
                        if name in held:
                            routes.setdefault(magic, set()).add((program["label"], route["id"]))
    return routes, names, literals


def narrow_by_discriminant(hits, selectors, data, tables, literals):
    """Eleven routes behind one magic, cut to the arm the payload names.

    Returns (kept, dropped, action). `action` is the chain-derived reading --
    offset, tag, variant -- and is written into the record so `--check` can put
    the same question to devnet later. Nothing is credited on a guess: when a
    variant still names several routes AND those routes are separated by a
    length constant this reader cannot fold, the whole set is dropped with the
    reason, the same direction of error the program guard already takes.
    """
    variants = {
        route: {
            tuple(selector["path"].split("::", 1))
            for selector in selectors.get(route, [])
            if selector.get("kind") == "variant" and "::" in selector.get("path", "")
        }
        for route in hits
    }
    enums = {enum for held in variants.values() for enum, _ in held}
    usable = [enum for enum in sorted(enums) if enum in tables]
    if not usable:
        return list(hits), [], None
    if len(usable) > 1:
        return list(hits), [], None  # two discriminants in one payload: not ours to pick
    enum = usable[0]
    table = tables[enum]
    offset = table["offset"]
    if offset >= len(data):
        return (
            [],
            [
                {
                    "route": route,
                    "reason": (
                        f"the instruction is {len(data)} bytes and {enum} is read at offset "
                        f"{offset}, so the chain shows no discriminant to resolve it by"
                    ),
                }
                for route in hits
            ],
            None,
        )
    tag = data[offset]
    named = table["tags"].get(tag)
    action = {
        "enum": enum,
        "variant": named[1] if named else None,
        "tag": tag,
        "offset": offset,
        "offset_constant": table["offset_constant"],
        "source": table["source"],
    }
    if named is None:
        # An unknown tag is the fallthrough arm's own case, and the census
        # records that arm with a `fallthrough` selector and no variant.
        kept = [route for route in hits if not variants[route]]
        dropped = [
            {
                "route": route,
                "reason": f"{enum} tag {tag} at offset {offset} names no variant of this route",
            }
            for route in hits
            if variants[route]
        ]
        return kept, dropped, action
    selected = (enum, named[1])
    kept = [route for route in hits if selected in variants[route]]
    dropped = [
        {
            "route": route,
            "reason": (
                f"the payload's {enum} tag is {tag} = `{enum}::{named[1]}` at offset {offset}; "
                f"this route dispatches on "
                + (
                    ", ".join(f"`{a}::{b}`" for a, b in sorted(variants[route]))
                    if variants[route]
                    else "the fallthrough arm"
                )
            ),
        }
        for route in hits
        if selected not in variants[route]
    ]
    if len(kept) > 1:
        # The dispatch separates same-variant arms by length
        # (`Action::Retire if instruction_data.len() == ..`). Fold the
        # constants when they are literals; when they are not, credit nobody
        # rather than credit four routes for one transaction.
        # Compare the CONSTANT NAMES, not their values: these constants are
        # sums of other crates' widths, so folding them needs an evaluator this
        # reader does not have, and comparing unresolved values makes four
        # different guards look like one.
        names = {
            route: frozenset(
                (selector.get("constant") or "").split("::")[-1]
                for selector in selectors.get(route, [])
                if selector.get("kind") == "length"
            )
            for route in kept
        }
        lengths = {
            route: {
                value
                for name in names[route]
                for value in (literals.get(name) or set())
                if len(literals.get(name) or set()) == 1
            }
            for route in kept
        }
        if len({names[route] for route in kept}) > 1:
            foldable = all(
                len(literals.get(name) or set()) == 1
                for route in kept
                for name in names[route]
            )
            by_length = [route for route in kept if len(data) in lengths[route]]
            if foldable and by_length:
                dropped += [
                    {
                        "route": route,
                        "reason": (
                            f"the instruction is {len(data)} bytes and this arm of "
                            f"`{enum}::{named[1]}` is guarded on a different length"
                        ),
                    }
                    for route in kept
                    if route not in by_length
                ]
                kept = by_length
            else:
                dropped += [
                    {
                        "route": route,
                        "reason": (
                            f"`{enum}::{named[1]}` names {len(kept)} routes that the dispatch "
                            "separates by an instruction length whose constant this reader "
                            "cannot fold, so none of them is credited"
                        ),
                    }
                    for route in kept
                ]
                kept = []
    return sorted(kept), dropped, action


def harvest(source: pathlib.Path) -> list[str]:
    text = source.read_text(errors="replace")
    keyed = KEYED_SIGNATURE.findall(text)
    found = keyed if keyed else SIGNATURE.findall(text)
    signatures = []
    for signature in found:
        if signature not in signatures:
            signatures.append(signature)
    return signatures


def discover(arguments) -> int:
    inventory = json.loads(pathlib.Path(arguments.inventory).read_text())
    paths = source_files()
    resolved, magic_names, literals = magic_routes(inventory, paths)
    discriminants = discriminant_index(paths)
    selectors = {
        route["id"]: route.get("selectors", [])
        for program in inventory["programs"]
        for route in program["routes"]
    }
    programs = json.loads(pathlib.Path(arguments.programs).read_text())
    by_address = {address: label for label, address in programs.items()}

    sources = [pathlib.Path(name) for name in arguments.source]
    signatures: list[str] = []
    per_source = []
    for source in sources:
        held = harvest(source)
        per_source.append((source, len(held)))
        for signature in held:
            if signature not in signatures:
                signatures.append(signature)

    url = rpc_url()
    records, skipped = [], []
    for signature in signatures:
        result = fetch(url, signature)
        if result is None:
            skipped.append((signature, "devnet has no finalized transaction with this signature"))
            continue
        chain = observe(result)
        first_party = False
        for instruction in chain["instructions"]:
            head = instruction["head"]
            if len(head) != 8 or not all(0x20 <= byte < 0x7F for byte in head):
                continue
            magic = head.decode()
            label = by_address.get(instruction["program"])
            if label is None:
                continue
            first_party = True
            # The program guard. A magic mirrored in another package resolves to
            # a route that did NOT run; keeping only same-program resolutions is
            # what makes this a reading of the chain rather than of a name.
            hits = sorted(
                route for owner, route in resolved.get(magic, set()) if owner == label
            )
            if not hits:
                skipped.append(
                    (
                        signature,
                        f"{magic} to {label} resolves to no {label} route "
                        f"({len(resolved.get(magic, set()))} route(s) elsewhere)",
                    )
                )
                continue
            tables = {}
            for constant in magic_names.get(magic, set()):
                tables.update(discriminants.get(constant, {}))
            hits, unselected, action = narrow_by_discriminant(
                hits, selectors, instruction["data"], tables, literals
            )
            if not hits:
                skipped.append(
                    (
                        signature,
                        f"{magic} to {label}: "
                        + "; ".join(entry["reason"] for entry in unselected[:1]),
                    )
                )
                continue
            claimed = sorted(
                set(hits)
                | {
                    f"{name}/process_instruction"
                    for name, address in programs.items()
                    if address in chain["programs_invoked"]
                    and any(
                        f"{name}/process_instruction" == r["id"]
                        for p in inventory["programs"]
                        if p["label"] == name
                        for r in p["routes"]
                    )
                }
            )
            stage = f"{magic.lower()}-{chain['slot']}"
            provenance = (
                f"resolved from the chain: the outer instruction to {label} begins with "
                f"{magic}, and the tree dispatches {', '.join(hits)} on those eight bytes. "
                "The sibling `*/process_instruction` entries are the entrypoints of the "
                "programs this transaction's own logs show invoked."
            )
            if action is not None:
                stage = f"{magic.lower()}-{(action['variant'] or 'unknown').lower()}-{chain['slot']}"
                provenance = (
                    f"resolved from the chain: the outer instruction to {label} begins with "
                    f"{magic}, which names a request family rather than one route, and the "
                    f"byte at offset {action['offset']} "
                    f"(`{action['offset_constant']}`, read by the decoder in "
                    f"{action['source']}) is {action['tag']} = "
                    f"`{action['enum']}::{action['variant']}`, which selects "
                    f"{', '.join(hits)}. The sibling `*/process_instruction` entries are the "
                    "entrypoints of the programs this transaction's own logs show invoked."
                )
            record = {
                "stage": stage,
                "magic": magic,
                "program": label,
                "outcome": "refused" if chain["error"] is not None else "executed",
                "signature": signature,
                "instruction_data_len": instruction["len"],
                "routes": claimed,
                "route_provenance": provenance,
            }
            if action is not None:
                record["action"] = action
            if unselected:
                record["routes_not_selected"] = unselected
            records.append(record)
        if not first_party:
            # "Failed" and "never ran" are different facts. A finalized
            # signature whose transaction sends nothing to a program in this
            # cohort's map is a real reading, and a source list is easier to
            # trust when every signature in it is accounted for.
            skipped.append(
                (
                    signature,
                    "finalized, but no top-level instruction goes to a program in this "
                    "cohort's map with an eight-byte ASCII magic",
                )
            )

    document = {
        "schema": SCHEMA,
        "cohort": arguments.cohort,
        "cluster": "devnet",
        "note": (
            "Built by tools/gauntlet/devnet-witness/corroborate.py --discover from the signatures "
            f"in {', '.join(source.name for source in sources)}, then corroborated against devnet. "
            "No route claim in this file was "
            "authored: each comes from the eight bytes the chain shows the transaction sent, "
            "resolved against the census route that dispatches on them, and kept only when the "
            "route's program is the program the instruction was sent to. Where those eight bytes "
            "name a request FAMILY rather than one route, the `action` field records the byte the "
            "decoder itself reads to pick the arm -- its offset constant, its tag and the variant "
            "that tag names -- and `--check` re-reads that byte from the chain. Every `slot`, "
            "`compute_units`, `programs_invoked`, `routes_corroborated` and `not_corroborated` "
            "field is written by the tool from the chain's own reply. "
            "WHAT THIS DOES NOT SAY: a devnet witness corroborates the PROGRAM, the OUTER "
            "MAGIC and any wire discriminant the dispatch selects on. It does not corroborate "
            "which internal branch a program took below that, and it is not "
            "a proof about every input -- the same boundary `dclutch-route-census observe` draws "
            "for the localhost campaigns."
        ),
        "evidence_documents": [
            str(source.relative_to(REPO)) if source.is_relative_to(REPO) else str(source)
            for source in sources
        ],
        "programs": programs,
        "skipped": [{"signature": s, "reason": r} for s, r in skipped],
        "records": records,
    }
    out = pathlib.Path(arguments.out)
    out.write_text(json.dumps(document, indent=2) + "\n")
    print(
        f"corroborate: discovered {len(records)} record(s) from {len(signatures)} signature(s) "
        f"in {len(sources)} source(s); {len(skipped)} carried no resolvable first-party magic"
    )
    for source, count in per_source:
        print(f"corroborate:   {count:>4} signature(s) from {source}")
    problems, routes = process(out.resolve(), url, write=True)
    return 1 if problems else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify every witness document")
    parser.add_argument("--write", metavar="FILE", help="refresh one document's chain-derived fields")
    parser.add_argument("--discover", action="store_true", help="build a document from a cohort")
    parser.add_argument(
        "--source",
        metavar="DOC",
        action="append",
        help="--discover: a cohort evidence document or run journal; repeatable",
    )
    parser.add_argument("--inventory", metavar="FILE", help="--discover: census inventory JSON")
    parser.add_argument("--programs", metavar="FILE", help="--discover: label -> program address map")
    parser.add_argument("--cohort", metavar="N", help="--discover: the cohort's name")
    parser.add_argument("--out", metavar="FILE", help="--discover: the document to write")
    arguments = parser.parse_args()
    modes = [arguments.check, bool(arguments.write), arguments.discover]
    if sum(1 for mode in modes if mode) != 1:
        parser.error("pass exactly one of --check, --write or --discover")
    if arguments.discover:
        missing = [
            name
            for name in ("source", "inventory", "programs", "cohort", "out")
            if getattr(arguments, name) is None
        ]
        if missing:
            parser.error("--discover requires " + ", ".join(f"--{name}" for name in missing))
        return discover(arguments)

    url = rpc_url()
    paths = (
        [pathlib.Path(arguments.write).resolve()] if arguments.write else sorted(WITNESS_DIR.glob("*.json"))
    )
    if not paths:
        print("corroborate: no witness documents under docs/evidence/witnesses/")
        return 0

    failures = total = 0
    for path in paths:
        problems, routes = process(path, url, write=bool(arguments.write))
        failures += problems
        total += routes
    verb = "wrote" if arguments.write else "corroborated"
    print(f"corroborate: {verb} {len(paths)} document(s), {total} distinct routes, {failures} problem(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
