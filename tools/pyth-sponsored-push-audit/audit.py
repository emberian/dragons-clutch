#!/usr/bin/env python3
"""Executable offline audit model for the sponsored Pyth push boundary."""

from __future__ import annotations

import dataclasses
import hashlib
import json
import struct
import sys
from pathlib import Path


SCHEMA = "dclutch-pyth-sponsored-push-safety-matrix-v1"
PRICE_UPDATE_LEN = 134
PRICE_UPDATE_DISCRIMINATOR = bytes.fromhex("22f123639d7ef4cd")
PROGRAM_DERIVED_ADDRESS_DOMAIN = b"ProgramDerivedAddress"
SOL_USD_FEED_ID = bytes.fromhex(
    "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"
)
LEGACY_RECEIVER = "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
LEGACY_PUSH_PROGRAM = "pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT"
LEGACY_SOL_USD_ACCOUNT = "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"
LEGACY_SOL_USD_BUMP = 252
REQUIRED_MATRIX_ROWS = {
    "cluster",
    "release",
    "feed",
    "pda-account",
    "account-owner",
    "push-deployment",
    "receiver-deployment",
    "verification-level",
    "write-authority",
    "posted-slot",
    "publication-window",
    "clock-freshness",
    "confidence",
    "exponent-rounding",
    "digest-race",
    "snapshot-identity",
    "snapshot-admission",
    "snapshot-immutability",
    "snapshot-funding",
    "snapshot-consume",
    "snapshot-cleanup",
    "cross-market-reuse",
    "candidate-head",
    "head-finalization",
    "monotonicity-proof",
    "late-posting",
    "replay",
    "substitution",
    "closing",
    "upgrade-downgrade",
    "sponsor-liveness",
    "single-answer",
    "direct-fast-path",
    "no-failover",
    "rent-privilege",
}
MATRIX_STATUSES = {
    "closed-existing",
    "required-new-owner",
    "operational-constraint",
    "must-refuse",
}


class Refusal(ValueError):
    """A stable audit-model refusal."""


@dataclasses.dataclass(frozen=True)
class Policy:
    """Exact facts that founding and the physical outer must authenticate."""

    account: str
    owner: str
    push_program: str
    feed_id: bytes
    shard: int
    bump: int
    window_start: int
    window_end: int
    maximum_age: int
    maximum_future_skew: int
    expected_exponent: int
    maximum_confidence_bps: int


@dataclasses.dataclass(frozen=True)
class AccountSnapshot:
    """Atomic AccountInfo and Clock facts seen by one resolution instruction."""

    address: str
    owner: str
    executable: bool
    writable: bool
    rent_exempt: bool
    data: bytes
    current_slot: int
    current_unix_seconds: int


@dataclasses.dataclass(frozen=True)
class ParsedPrice:
    """Exact fully checked PriceUpdateV2 facts used by the audit model."""

    write_authority: bytes
    feed_id: bytes
    price: int
    confidence: int
    exponent: int
    publish_time: int
    previous_publish_time: int
    posted_slot: int
    body_sha256: str


@dataclasses.dataclass(frozen=True)
class Candidate:
    """Immutable submitted observation identity used by the head model."""

    address: str
    publish_time: int
    posted_slot: int
    body_sha256: str


def _base58_decode(value: str) -> bytes:
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    number = 0
    for symbol in value:
        try:
            digit = alphabet.index(symbol)
        except ValueError as error:
            raise Refusal("invalid base58") from error
        number = number * 58 + digit
    leading = len(value) - len(value.lstrip("1"))
    body = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    decoded = bytes(leading) + body
    if len(decoded) != 32:
        raise Refusal("public key is not 32 bytes")
    return decoded


def derive_with_pinned_bump(policy: Policy) -> bytes:
    """Derive the frozen shard/feed PDA using its independently pinned bump."""

    if not 0 <= policy.shard <= 0xFFFF or not 0 <= policy.bump <= 0xFF:
        raise Refusal("shard or bump is out of range")
    return hashlib.sha256(
        struct.pack("<H", policy.shard)
        + policy.feed_id
        + bytes([policy.bump])
        + _base58_decode(policy.push_program)
        + PROGRAM_DERIVED_ADDRESS_DOMAIN
    ).digest()


def parse_price_update(data: bytes) -> ParsedPrice:
    """Parse the exact SDK 2.0.0 full PriceUpdateV2 fixed layout."""

    if len(data) != PRICE_UPDATE_LEN:
        raise Refusal("price update length")
    if data[:8] != PRICE_UPDATE_DISCRIMINATOR:
        raise Refusal("price update discriminator")
    if data[40] != 1:
        raise Refusal("price update is not Full")
    if data[133] != 0:
        raise Refusal("price update allocation tail")
    return ParsedPrice(
        write_authority=data[8:40],
        feed_id=data[41:73],
        price=struct.unpack_from("<q", data, 73)[0],
        confidence=struct.unpack_from("<Q", data, 81)[0],
        exponent=struct.unpack_from("<i", data, 89)[0],
        publish_time=struct.unpack_from("<q", data, 93)[0],
        previous_publish_time=struct.unpack_from("<q", data, 101)[0],
        posted_slot=struct.unpack_from("<Q", data, 125)[0],
        body_sha256=hashlib.sha256(data).hexdigest(),
    )


def authenticate(policy: Policy, snapshot: AccountSnapshot) -> ParsedPrice:
    """Apply the proposed boundary without claiming it is production authority."""

    if snapshot.address != policy.account:
        raise Refusal("account substitution")
    if snapshot.owner != policy.owner or snapshot.executable:
        raise Refusal("receiver ownership")
    if snapshot.writable or not snapshot.rent_exempt:
        raise Refusal("account privilege or rent")
    if derive_with_pinned_bump(policy) != _base58_decode(policy.account):
        raise Refusal("push PDA derivation")
    update = parse_price_update(snapshot.data)
    if update.write_authority != _base58_decode(policy.account):
        raise Refusal("write authority")
    if update.feed_id != policy.feed_id:
        raise Refusal("feed substitution")
    if update.posted_slot == 0 or update.posted_slot > snapshot.current_slot:
        raise Refusal("posted slot")
    if update.publish_time <= 0 or update.previous_publish_time > update.publish_time:
        raise Refusal("publication ordering")
    if not policy.window_start <= update.publish_time <= policy.window_end:
        raise Refusal("publication window")
    if snapshot.current_unix_seconds <= 0:
        raise Refusal("clock")
    oldest = snapshot.current_unix_seconds - policy.maximum_age
    newest = snapshot.current_unix_seconds + policy.maximum_future_skew
    if not oldest <= update.publish_time <= newest:
        raise Refusal("publication freshness")
    # This explicit global admission deadline closes the candidate set without
    # trusting mutable upstream history. At and before the boundary a value
    # published at window_end can still satisfy the inclusive freshness rule;
    # strictly after it, no in-window value can.
    if snapshot.current_unix_seconds > primary_deadline(policy):
        raise Refusal("candidate admission deadline")
    if update.exponent != policy.expected_exponent:
        raise Refusal("exponent")
    if not 0 < policy.maximum_confidence_bps <= 10_000:
        raise Refusal("confidence policy")
    if update.confidence * 10_000 > abs(update.price) * policy.maximum_confidence_bps:
        raise Refusal("confidence")
    # This is deliberately identity normalization. Source's existing
    # PythAdapterConfig owns it: exact exponent or refusal, i64 -> i128, no
    # rescaling and therefore no second rounding boundary.
    return update


def primary_deadline(policy: Policy) -> int:
    """Latest Clock second at which any in-window candidate may be admitted."""

    if policy.maximum_age < 0 or policy.window_end < policy.window_start:
        raise Refusal("invalid time policy")
    return policy.window_end + policy.maximum_age


def candidate_from_update(address: str, update: ParsedPrice) -> Candidate:
    """Create the audit projection of one immutable onchain candidate."""

    if len(update.body_sha256) != 64:
        raise Refusal("candidate digest")
    try:
        bytes.fromhex(update.body_sha256)
    except ValueError as error:
        raise Refusal("candidate digest") from error
    return Candidate(
        address=address,
        publish_time=update.publish_time,
        posted_slot=update.posted_slot,
        body_sha256=update.body_sha256,
    )


def candidate_rank(candidate: Candidate) -> tuple[int, int, bytes]:
    """Canonical best-valid-submitted ordering, never provider optimality."""

    if candidate.publish_time <= 0 or candidate.posted_slot <= 0:
        raise Refusal("candidate chronology")
    if len(candidate.body_sha256) != 64:
        raise Refusal("candidate digest")
    try:
        digest = bytes.fromhex(candidate.body_sha256)
    except ValueError as error:
        raise Refusal("candidate digest") from error
    return (candidate.publish_time, candidate.posted_slot, digest)


def advance_head(current: Candidate | None, submitted: Candidate) -> Candidate:
    """Monotonically select the greatest submitted candidate."""

    submitted_rank = candidate_rank(submitted)
    if current is None:
        return submitted
    current_rank = candidate_rank(current)
    return submitted if submitted_rank > current_rank else current


def candidate_set_closed(
    policy: Policy,
    current_unix_seconds: int,
    upstream: ParsedPrice | None = None,
    exact_push_monotonicity_proven: bool = False,
) -> bool:
    """Evaluate either independently safe candidate-set closure rule.

    The Clock deadline is always sufficient. Upstream advancement is merely an
    earlier closure optimization and is disabled unless the exact deployed push
    ELF has been bound to a strict-increase semantic proof or equivalent hostile.
    """

    if current_unix_seconds <= 0:
        raise Refusal("clock")
    if current_unix_seconds > primary_deadline(policy):
        return True
    return bool(
        exact_push_monotonicity_proven
        and upstream is not None
        and upstream.feed_id == policy.feed_id
        and upstream.publish_time > policy.window_end
    )


def terminal_selection(
    policy: Policy,
    current_unix_seconds: int,
    head: Candidate | None,
    upstream: ParsedPrice | None = None,
    exact_push_monotonicity_proven: bool = False,
) -> str:
    """Return the only terminal branch after candidate-set closure."""

    if not candidate_set_closed(
        policy,
        current_unix_seconds,
        upstream,
        exact_push_monotonicity_proven,
    ):
        raise Refusal("candidate set is open")
    return "best-valid-submitted-candidate" if head is not None else "funded-failure"


def validate_matrix(repository: Path, matrix_path: Path) -> None:
    """Validate exact matrix coverage and that every cited tree path exists."""

    value = json.loads(matrix_path.read_text(encoding="utf-8"))
    if set(value) != {"schema", "target", "verdict", "rows"}:
        raise Refusal("matrix envelope fields")
    if value["schema"] != SCHEMA:
        raise Refusal("matrix schema")
    target = value["target"]
    expected_target = {
        "cluster": "devnet",
        "account": LEGACY_SOL_USD_ACCOUNT,
        "receiverProgram": LEGACY_RECEIVER,
        "pushProgram": LEGACY_PUSH_PROGRAM,
        "shard": "0",
        "bump": str(LEGACY_SOL_USD_BUMP),
        "feedIdHex": SOL_USD_FEED_ID.hex(),
    }
    if target != expected_target:
        raise Refusal("matrix target changed")
    verdict = value["verdict"]
    if set(verdict) != {"currentHead", "design"}:
        raise Refusal("matrix verdict fields")
    if verdict["currentHead"] != "reject-drop-in" or verdict["design"] != "conditional-accept":
        raise Refusal("matrix verdict changed")
    rows = value["rows"]
    ids = {row.get("id") for row in rows}
    if ids != REQUIRED_MATRIX_ROWS or len(rows) != len(ids):
        raise Refusal("matrix row coverage")
    required = {
        "id",
        "boundary",
        "predicate",
        "owner",
        "attack",
        "status",
        "evidencePath",
        "requiredChange",
    }
    for row in rows:
        if set(row) != required or any(row[field] == "" for field in required):
            raise Refusal(f"invalid matrix row {row.get('id')}")
        if row["status"] not in MATRIX_STATUSES:
            raise Refusal(f"invalid matrix status {row['id']}")
        evidence = repository / row["evidencePath"]
        if not evidence.is_file():
            raise Refusal(f"missing evidence path {row['evidencePath']}")


def canonical_policy() -> Policy:
    """Return a deterministic hostile-test policy, not a live observation."""

    publish_time = 1_787_939_239
    return Policy(
        account=LEGACY_SOL_USD_ACCOUNT,
        owner=LEGACY_RECEIVER,
        push_program=LEGACY_PUSH_PROGRAM,
        feed_id=SOL_USD_FEED_ID,
        shard=0,
        bump=LEGACY_SOL_USD_BUMP,
        window_start=publish_time - 600,
        window_end=publish_time + 600,
        maximum_age=5_000,
        maximum_future_skew=5,
        expected_exponent=-8,
        maximum_confidence_bps=100,
    )


def canonical_snapshot() -> AccountSnapshot:
    """Return deterministic full-shape bytes for offline hostile tests."""

    policy = canonical_policy()
    publish_time = 1_787_939_239
    data = bytearray(PRICE_UPDATE_LEN)
    data[:8] = PRICE_UPDATE_DISCRIMINATOR
    data[8:40] = _base58_decode(policy.account)
    data[40] = 1
    data[41:73] = policy.feed_id
    struct.pack_into("<q", data, 73, 10_450_253_500)
    struct.pack_into("<Q", data, 81, 1_000_000)
    struct.pack_into("<i", data, 89, -8)
    struct.pack_into("<q", data, 93, publish_time)
    struct.pack_into("<q", data, 101, publish_time - 300)
    struct.pack_into("<q", data, 109, 10_450_000_000)
    struct.pack_into("<Q", data, 117, 1_100_000)
    struct.pack_into("<Q", data, 125, 489_486_551)
    return AccountSnapshot(
        address=policy.account,
        owner=policy.owner,
        executable=False,
        writable=False,
        rent_exempt=True,
        data=bytes(data),
        current_slot=489_486_600,
        current_unix_seconds=publish_time + 10,
    )


def main(arguments: list[str]) -> int:
    if arguments:
        raise Refusal("usage: audit.py")
    tool = Path(__file__).resolve().parent
    repository = tool.parents[1]
    validate_matrix(repository, tool / "matrix.json")
    update = authenticate(canonical_policy(), canonical_snapshot())
    print(
        "sponsored push safety matrix: exact; "
        f"hostile model accepted body {update.body_sha256}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, Refusal, json.JSONDecodeError) as error:
        print(f"sponsored-push-audit: {error}", file=sys.stderr)
        raise SystemExit(1) from error
