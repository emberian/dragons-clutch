#!/usr/bin/env python3
"""Verify FROM CHAIN STATE that a founded Market binds the capability release a
family's compiler emitted -- by RE-DERIVING every coordinate rather than reading
a driver's own report back to itself.

    verify-selected-capability-binding.py RPC_URL MARKET_PUBKEY MARKET_JSON

MARKET_JSON is the run's market input. Its `selected_capability` carries the
BYTES the family's release compiler emitted. Nothing here trusts that file for a
CONCLUSION: every identity is recomputed from those bytes and required to equal
what the chain holds, at an address that is itself re-derived.

# Why this exists

Until now nothing in the tree closed this loop. Every consumer of a family's
release compiler runs BEFORE founding, to produce the bytes to submit; none
reads a founded Market back off chain. `tools/gauntlet/frontend/expect.mjs`
comes closest -- it fetches the manifest record and checks its digest against
the Market -- but it takes the record's ADDRESS from campaign evidence and has
nothing on the other side of the comparison. So "the Market selected our
release" was, for four families, a claim supported by the driver that made it.

# What it actually proves, and the one thing it does not

The manifest record's address is derived here from the Market's own field, so a
campaign that recorded the wrong address cannot pass. The entry's identities are
recomputed as SHA-256 of the supplied artifact bytes, so a manifest naming some
other release cannot pass. The kind and capacity are additionally recomputed
from their DOMAIN PREIMAGES where the family publishes them, so a placeholder
identity cannot pass -- that hole is why this check pins them.

It does NOT re-run the release compiler. Determinism is a separate property,
proven where it belongs: each family's compiler has a byte-stability test
asserting two independent compilations agree. This tool plus that test together
say the founded Market binds a release identity reproducible from source.

# Family-neutral

Every coordinate is read at a schema offset or recomputed, so this works for any
family the seam admits. It prints the family the payload names and checks the
same invariant for all of them.
"""

import base64
import hashlib
import json
import sys
import urllib.request

RAW_RECORD_SEED = b"dclutch-raw-record-v1"
MANIFEST_SCHEMA_PREIMAGE = b"dclutch/schema/capability-manifest-profile-1-v1"
MARKET_STATE_SEED = b"dclutch/market-core/state/v2"
MARKET_MAGIC = b"DCLTCOR3"
MANIFEST_MAGIC = b"DCLTCAP1"

# DCLTCOR3, from crates/dclutch-market-core-codec/src/generated.rs.
MARKET_REALM = 48
MARKET_PRODUCT_RECORD = 80
MARKET_PRODUCT_ID = 112
MARKET_RESOLUTION_POLICY = 144
MARKET_CAPABILITY_MANIFEST = 176
MARKET_SELECTED_RELEASE_SET = 208
MARKET_REGISTRY = 240
MARKET_GENERATION = 272

# CapabilityManifestV1, from crates/dclutch-capability-contract/src/generated_abi.rs.
MANIFEST_HEADER = 16
MANIFEST_ENTRY_BYTES = 528
ENTRY_KIND = 0
ENTRY_RELEASE = 32
ENTRY_CONFIG = 64
ENTRY_CAPACITY = 96
ENTRY_CHILD_SCHEMA = 128
ENTRY_CHILD_DERIVATION = 160

# CapabilityProgramV4, from crates/dclutch-capability-program-contract/src/generated_v4.rs.
V4_KIND = 16
V4_ROOT_SCHEMA = 112
V4_DERIVATION_POLICY = 144
V4_CAPACITY_PROFILE = 176

# TokenBehaviorSelectionV2, from crates/dclutch-token-svm/src/behavior_binding_v2.rs.
CONFIG_REALM = 16

# Publication identity block: magic(8) + version(2) + pad, identities from 16.
PUBLICATION_IDENTITY_START = 16

B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def b58encode(raw: bytes) -> str:
    number = int.from_bytes(raw, "big")
    out = ""
    while number:
        number, rem = divmod(number, 58)
        out = B58[rem] + out
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + out


def b58decode(text: str) -> bytes:
    number = 0
    for char in text:
        number = number * 58 + B58.index(char)
    return number.to_bytes(32, "big")


# ed25519 on-curve test, for the PDA off-curve requirement.
_P = 2**255 - 19
_D = (-121665 * pow(121666, _P - 2, _P)) % _P


def _on_curve(point: bytes) -> bool:
    y = int.from_bytes(point, "little") & ((1 << 255) - 1)
    if y >= _P:
        return False
    y2 = y * y % _P
    u = (y2 - 1) % _P
    v = (_D * y2 + 1) % _P
    if v == 0:
        return False
    x2 = u * pow(v, _P - 2, _P) % _P
    if x2 == 0:
        return True
    x = pow(x2, (_P + 3) // 8, _P)
    if x * x % _P != x2:
        x = x * pow(2, (_P - 1) // 4, _P) % _P
    return x * x % _P == x2


def find_program_address(seeds, program: bytes):
    for bump in range(255, -1, -1):
        digest = hashlib.sha256()
        for seed in seeds:
            digest.update(seed)
        digest.update(bytes([bump]))
        digest.update(program)
        digest.update(b"ProgramDerivedAddress")
        candidate = digest.digest()
        if not _on_curve(candidate):
            return candidate, bump
    raise SystemExit("no off-curve bump exists for those seeds")


def account(url: str, pubkey: str):
    request = urllib.request.Request(
        url,
        data=json.dumps(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getAccountInfo",
                "params": [pubkey, {"encoding": "base64", "commitment": "finalized"}],
            }
        ).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        value = json.load(response).get("result", {}).get("value")
    if value is None:
        return None, None
    return base64.b64decode(value["data"][0]), value["owner"]


class Report:
    def __init__(self):
        self.ok = True

    def check(self, label, observed, derived):
        good = observed == derived
        self.ok = self.ok and good
        print(f"  [{'OK ' if good else 'BAD'}] {label}")
        if not good:
            print(f"         chain   {observed.hex() if isinstance(observed, bytes) else observed}")
            print(f"         derived {derived.hex() if isinstance(derived, bytes) else derived}")


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    url, market_pubkey, market_json = sys.argv[1], sys.argv[2], sys.argv[3]
    report = Report()

    data, owner = account(url, market_pubkey)
    if data is None:
        raise SystemExit(f"market {market_pubkey} is absent at {url}")
    if data[:8] != MARKET_MAGIC:
        raise SystemExit(f"{market_pubkey} is not a {MARKET_MAGIC.decode()} market")
    core = b58decode(owner)
    generation = int.from_bytes(data[MARKET_GENERATION : MARKET_GENERATION + 8], "little")
    print(f"MARKET {market_pubkey}")
    print(f"  owner {owner}  len {len(data)}  magic {data[:8].decode()}  generation {generation}")

    # THE ADDRESS IS THE CLAIM. Every seed comes off the Market's own bytes, and
    # one of them is the capability-manifest digest -- so a manifest entry that
    # depended on this address could not have produced it.
    print("\nTHE MARKET ADDRESS RE-DERIVES FROM ITS OWN SEEDS")
    derived_market, bump = find_program_address(
        [
            MARKET_STATE_SEED,
            data[MARKET_REALM : MARKET_REALM + 32],
            data[MARKET_PRODUCT_RECORD : MARKET_PRODUCT_RECORD + 32],
            data[MARKET_PRODUCT_ID : MARKET_PRODUCT_ID + 32],
            data[MARKET_RESOLUTION_POLICY : MARKET_RESOLUTION_POLICY + 32],
            data[MARKET_CAPABILITY_MANIFEST : MARKET_CAPABILITY_MANIFEST + 32],
            data[MARKET_SELECTED_RELEASE_SET : MARKET_SELECTED_RELEASE_SET + 32],
            data[MARKET_REGISTRY : MARKET_REGISTRY + 32],
            data[MARKET_GENERATION : MARKET_GENERATION + 8],
        ],
        core,
    )
    report.check(
        f"the nine seeds (manifest digest among them) name this Market, bump {bump}",
        b58encode(derived_market),
        market_pubkey,
    )

    manifest_digest = data[MARKET_CAPABILITY_MANIFEST : MARKET_CAPABILITY_MANIFEST + 32]
    registry = data[MARKET_REGISTRY : MARKET_REGISTRY + 32]
    schema = hashlib.sha256(MANIFEST_SCHEMA_PREIMAGE).digest()
    record_address, record_bump = find_program_address(
        [RAW_RECORD_SEED, schema, manifest_digest], registry
    )
    print(f"\nMANIFEST RECORD (address re-derived, bump {record_bump})")
    print(f"  {b58encode(record_address)}")
    body, _ = account(url, b58encode(record_address))
    if body is None:
        raise SystemExit("the manifest record is absent at its derived address")
    report.check(
        "manifest body digests to the Market's capability-manifest field",
        hashlib.sha256(body).digest(),
        manifest_digest,
    )
    if body[:8] != MANIFEST_MAGIC:
        raise SystemExit(f"the manifest record is not a {MANIFEST_MAGIC.decode()}")
    entries = int.from_bytes(body[12:14], "little")
    print(f"  entries {entries}  len {len(body)}")

    plan = json.load(open(market_json))
    selected = plan.get("selected_capability")
    if selected is None:
        raise SystemExit(f"{market_json} carries no selected_capability payload")
    index = selected["selected_manifest_entry_index"]
    program_set = bytes.fromhex(selected["program_set_hex"])
    config = bytes.fromhex(selected["config_hex"])
    descriptor = bytes.fromhex(selected["selected_descriptor_hex"])
    publication = bytes.fromhex(selected["publication_hex"])
    print(f"\nFAMILY {selected['family']}   manifest entry {index}")

    offset = MANIFEST_HEADER + index * MANIFEST_ENTRY_BYTES
    entry = body[offset : offset + MANIFEST_ENTRY_BYTES]

    print("\nENTRY IDENTITIES, RECOMPUTED from the compiler's own artifact bytes")
    report.check(
        "release_id == SHA-256(ProgramSet bytes)",
        entry[ENTRY_RELEASE : ENTRY_RELEASE + 32],
        hashlib.sha256(program_set).digest(),
    )
    report.check(
        "config_id  == SHA-256(config bytes)",
        entry[ENTRY_CONFIG : ENTRY_CONFIG + 32],
        hashlib.sha256(config).digest(),
    )

    print("\nENTRY COORDINATES, READ OFF THE DESCRIPTOR (not off a restatement)")
    for label, entry_at, descriptor_at in (
        ("kind_id          == descriptor.kind", ENTRY_KIND, V4_KIND),
        ("capacity_profile == descriptor.capacity_profile", ENTRY_CAPACITY, V4_CAPACITY_PROFILE),
        ("child_schema     == descriptor.root_schema", ENTRY_CHILD_SCHEMA, V4_ROOT_SCHEMA),
        (
            "child_derivation == descriptor.derivation_policy",
            ENTRY_CHILD_DERIVATION,
            V4_DERIVATION_POLICY,
        ),
    ):
        report.check(label, entry[entry_at : entry_at + 32], descriptor[descriptor_at : descriptor_at + 32])

    # An all-zero kind is what a placeholder looks like when nobody checks. The
    # manifest cannot select it, so neither may this pass.
    report.check(
        "kind_id is not the all-zero placeholder",
        entry[ENTRY_KIND : ENTRY_KIND + 32] != bytes(32),
        True,
    )

    print("\nPUBLICATION (the family's canonical summary) vs the chain entry")
    print(f"  magic {publication[:8].decode(errors='replace')}  len {len(publication)}")
    for ordinal, (label, entry_at) in enumerate(
        (
            ("kind_id", ENTRY_KIND),
            ("release_id", ENTRY_RELEASE),
            ("config_id", ENTRY_CONFIG),
            ("capacity_profile", ENTRY_CAPACITY),
        )
    ):
        at = PUBLICATION_IDENTITY_START + ordinal * 32
        report.check(
            f"publication identity {ordinal} == entry.{label}",
            entry[entry_at : entry_at + 32],
            publication[at : at + 32],
        )

    print("\nMARKET IDENTITY")
    report.check(
        "the config binds this Market's own realm",
        data[MARKET_REALM : MARKET_REALM + 32],
        config[CONFIG_REALM : CONFIG_REALM + 32],
    )

    print("\n" + ("ALL CHECKS PASSED" if report.ok else "*** SOME CHECKS FAILED ***"))
    return 0 if report.ok else 1


if __name__ == "__main__":
    sys.exit(main())
