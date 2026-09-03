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
cohort's evidence document, asks devnet what each transaction actually sent,
reads the outer instruction's first eight bytes, and resolves THAT magic to the
census route whose dispatch selects on it -- either directly, or through the
`is_*(instruction_data)` predicate the census recorded as the route's selector.
A resolution is kept only when the route's own program is the program the
instruction was sent to, which is not a formality: `DCLTHOT3` is mirrored by
`registry/hot_continuation_v2::process`, and a resolver without that guard
credits a Registry route for a Trading transaction.

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
        instructions.append({"program": keys[entry["programIdIndex"]], "head": data[:8], "len": len(data)})
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
MAGIC_CONST = re.compile(
    r'const\s+([A-Z0-9_]+)\s*:\s*\[u8;\s*8\]\s*=\s*\*b"([A-Za-z0-9]{8})"'
)
PREDICATE = re.compile(
    r"pub fn (is_[a-z0-9_]+)\(\s*instruction_data:\s*&\[u8\][^)]*\)\s*->\s*bool\s*\{(.*?)\n\}",
    re.S,
)


def magic_routes(inventory: dict) -> dict[str, set[tuple[str, str]]]:
    """magic -> {(program label, route id)}, read from the tree, never authored.

    Two dispatch shapes reach a route from eight bytes. The census records the
    first directly, as a `magic` selector. The second it records as a
    `predicate` selector naming an `is_*` function, and the eight bytes are
    inside that function's body -- so the constant is found in the sources, the
    predicate that compares against it is found in the same file, and the route
    is the one whose selector names that predicate.
    """
    constants: dict[str, set[tuple[str, str]]] = {}
    predicates: dict[str, set[str]] = {}
    for root in ("programs", "crates"):
        for path in (REPO / root).rglob("*.rs"):
            if "/target/" in str(path):
                continue
            try:
                text = path.read_text()
            except OSError:
                continue
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
    for program in inventory["programs"]:
        for route in program["routes"]:
            for selector in route.get("selectors", []):
                if selector.get("kind") == "magic" and selector.get("ascii"):
                    routes.setdefault(selector["ascii"], set()).add((program["label"], route["id"]))
                elif selector.get("kind") == "predicate":
                    name = selector["function"].split("::")[-1]
                    for magic, held in predicates.items():
                        if name in held:
                            routes.setdefault(magic, set()).add((program["label"], route["id"]))
    return routes


def discover(arguments) -> int:
    inventory = json.loads(pathlib.Path(arguments.inventory).read_text())
    resolved = magic_routes(inventory)
    programs = json.loads(pathlib.Path(arguments.programs).read_text())
    by_address = {address: label for label, address in programs.items()}
    source = pathlib.Path(arguments.source)
    text = source.read_text()
    signatures = []
    for signature in SIGNATURE.findall(text):
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
        for instruction in chain["instructions"]:
            head = instruction["head"]
            if len(head) != 8 or not all(0x20 <= byte < 0x7F for byte in head):
                continue
            magic = head.decode()
            label = by_address.get(instruction["program"])
            if label is None:
                continue
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
            records.append(
                {
                    "stage": f"{magic.lower()}-{chain['slot']}",
                    "magic": magic,
                    "program": label,
                    "outcome": "refused" if chain["error"] is not None else "executed",
                    "signature": signature,
                    "instruction_data_len": instruction["len"],
                    "routes": claimed,
                    "route_provenance": (
                        f"resolved from the chain: the outer instruction to {label} begins with "
                        f"{magic}, and the tree dispatches {', '.join(hits)} on those eight bytes. "
                        "The sibling `*/process_instruction` entries are the entrypoints of the "
                        "programs this transaction's own logs show invoked."
                    ),
                }
            )

    document = {
        "schema": SCHEMA,
        "cohort": arguments.cohort,
        "cluster": "devnet",
        "note": (
            "Built by tools/gauntlet/devnet-witness/corroborate.py --discover from the signatures "
            f"in {source.name}, then corroborated against devnet. No route claim in this file was "
            "authored: each comes from the eight bytes the chain shows the transaction sent, "
            "resolved against the census route that dispatches on them, and kept only when the "
            "route's program is the program the instruction was sent to. Every `slot`, "
            "`compute_units`, `programs_invoked`, `routes_corroborated` and `not_corroborated` "
            "field is written by the tool from the chain's own reply. "
            "WHAT THIS DOES NOT SAY: a devnet witness corroborates the PROGRAM and the OUTER "
            "MAGIC. It does not corroborate which internal branch a program took, and it is not "
            "a proof about every input -- the same boundary `dclutch-route-census observe` draws "
            "for the localhost campaigns."
        ),
        "evidence_document": str(source.relative_to(REPO)) if source.is_relative_to(REPO) else str(source),
        "programs": programs,
        "skipped": [{"signature": s, "reason": r} for s, r in skipped],
        "records": records,
    }
    out = pathlib.Path(arguments.out)
    out.write_text(json.dumps(document, indent=2) + "\n")
    print(
        f"corroborate: discovered {len(records)} record(s) from {len(signatures)} signature(s) "
        f"in {source.name}; {len(skipped)} carried no resolvable first-party magic"
    )
    problems, routes = process(out.resolve(), url, write=True)
    return 1 if problems else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify every witness document")
    parser.add_argument("--write", metavar="FILE", help="refresh one document's chain-derived fields")
    parser.add_argument("--discover", action="store_true", help="build a document from a cohort")
    parser.add_argument("--source", metavar="DOC", help="--discover: the cohort evidence document")
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
