#!/usr/bin/env python3
"""Reproduce the bounded rent and capital-time arithmetic audit.

This is a standard-library-only, offline calculator.  It does not build an
SBF program, inspect a wallet, contact an RPC, or promote historical/local
measurements into current linked-program evidence.

The output separates three evidence classes:

* ``HISTORICAL_ARTIFACT``: exact numbers bound to an older named artifact;
* ``SOURCE_DERIVED``: exact integer arithmetic over pinned source constants;
* ``MODEL_ONLY``: a proposed format or illustrative economic projection.

Run ``--check`` for the small frozen-example gate used by the unit tests.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


SCHEMA = "dragons-clutch/rent-capital-time-audit/v1"
AS_OF_DATE = "2026-08-23"

HISTORICAL_ARTIFACT = "HISTORICAL_ARTIFACT"
SOURCE_DERIVED = "SOURCE_DERIVED"
MODEL_ONLY = "MODEL_ONLY"

LAMPORTS_PER_RENT_BYTE = 6_960
ACCOUNT_RENT_OVERHEAD_BYTES = 128
LOADER_V3_PROGRAMDATA_METADATA_BYTES = 45
LOADER_V3_PROGRAM_ACCOUNT_BYTES = 36

U32_MAX = (1 << 32) - 1
U64_MAX = (1 << 64) - 1

# Current General V1 exact layouts.  These remain source-derived layout facts,
# not current-cluster rent quotes or a current linked SBF claim.
CLEAR_WORK_V1_ACCOUNT_BYTES = 50_054
CANDIDATE_FEED_V1_ACCOUNT_BYTES = 6_266
SETTLEMENT_RECEIPT_V2_ACCOUNT_BYTES = 217
GENERAL_FUNDING_LEDGER_V1_ACCOUNT_BYTES = 85

# Proposed/model-only formats.
CANDIDATE_FEED_V2_FIXED_BYTES = 218
PAIRING_SLICE_BYTES = 13
RECEIPT_PAGE_V1_ACCOUNT_BYTES = 3_632
RECEIPTS_PER_PAGE = 16

# Exact historical artifact identities.  These values are comparison evidence
# only and say nothing about the current working tree's linked ELF.
CURRENT_UNSEALED_HISTORICAL_ELF_SHA256 = (
    "193c08723eaefeff9a1c2aa53c9e3feb58960a919fb0bbb7ca5da3bd817aa95b"
)
CURRENT_UNSEALED_HISTORICAL_ELF_BYTES = 2_082_320
CAPABILITY_PROFILE_COMMIT = "625cd65ac0c17be3ed4371df5ab8f23db67b9eae"
CAPABILITY_PROFILE_ELF_BYTES = {
    "full": 2_083_112,
    "general_source_v2_point": 1_435_352,
    "direct_v3_source_v2_point": 1_056_864,
}

# Whole-file digests bind immutable historical evidence and the active-width
# implementation.  Semantic-fragment digests bind mutable source files tightly
# enough to catch a changed constant without making an unrelated rustfmt or
# prose edit invalidate this arithmetic gate.
PROVENANCE_SHA256 = {
    "crates/clutch-batch/src/relation_v1_stream_v2.rs": (
        "9e59ab155702263c05a3a4cfb85d5555cf4c4cd720674db4221d5d9535e3ee53"
    ),
    "crates/clutch-batch/CLEAR_WORK_V2_EVIDENCE.md": (
        "2bc7b5d7ce62d4463b93a35ba98ea3265b8d946be734a11ce65c0f92b2b9ad44"
    ),
    "programs/clutch-sbf/audit/evidence/2026-08-22-capability-profiles.json": (
        "d1b5b3b43f1fcead508af44f34207befcee7187a40130150145c449b2d2eb453"
    ),
    "docs/reviews/CURRENT_UNSEALED_SBF_SNAPSHOT_2026-08-22.md": (
        "c4cdc4dea73c1b072dcdd318998f6b581890445f103b5dfde1b978ad7a4e6687"
    ),
}

PROVENANCE_FRAGMENTS = {
    "programs/solana-layout/src/lib.rs": (
        "pub const CLEAR_WORK_BODY_BYTES: usize = 47_846;",
        "assert_eq!(account_len::SETTLEMENT_RECEIPT, 217);",
    ),
    "programs/solana-layout/src/clearing.rs": (
        "assert_eq!(account_len::CLEAR_WORK, 50_054);",
        "assert_eq!(account_len::CANDIDATE_FEED, 6_266);",
        "pub const PAIRING_SLICE_BYTES: usize = 2 + 2 + 1 + 8;",
        "pub const GENERAL_FUNDING_LEDGER_BYTES: usize = 2 + 32 + 32 + 8 + 8 + 1 + 1 + 1;",
    ),
    "docs/reviews/STATE_RENT_AUDIT_2026-08-22.md": (
        "minimum_balance(bytes) = (bytes + 128) × 6,960 lamports",
        "embedded funding identity fit in about 3,632 bytes.",
        "`218 + 8×outcomes + 8×orders + 13×slices`",
    ),
    "docs/design/PRODUCT_COMPILER_AND_SERIES_V1.md": (
        "start_j    = first_start_bucket + j * stride_buckets",
        "instance_count * creation/rent allocation",
        "instance_count * mandatory work/keeper allocation",
        "instance_count * liquidity-blueprint tranche cap",
    ),
    "crates/clutch-product-series/src/artifacts.rs": (
        "pub const BASIS_BYTES: usize = 2_352;",
        "pub const EVIDENCE_ONLY_RECOVERY_POLICY_BYTES: usize = 208;",
        "pub const PRODUCT_TEMPLATE_BYTES: usize = 256;",
        "pub const MARKET_GENESIS_PROFILE_BYTES: usize = 352;",
        "pub const SERIES_ATTACHMENT_PLAN_BYTES: usize = 112;",
        "pub const SERIES_PLAN_BYTES: usize = 152;",
        "pub const SERIES_FUNDING_TERMS_BYTES: usize = 208;",
    ),
    "crates/clutch-product-series/src/funding.rs": (
        "pub const SERIES_FUNDING_QUOTE_BYTES: usize = 264;",
    ),
}


class AuditInputError(ValueError):
    """An integer or geometry input is outside the audited domain."""


def _integer(name: str, value: int, *, minimum: int, maximum: int | None = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise AuditInputError(f"{name} must be an integer")
    if value < minimum or (maximum is not None and value > maximum):
        suffix = f"..={maximum}" if maximum is not None else " or greater"
        raise AuditInputError(f"{name} must be in {minimum}{suffix}")
    return value


def ceil_div(numerator: int, denominator: int) -> int:
    numerator = _integer("numerator", numerator, minimum=0)
    denominator = _integer("denominator", denominator, minimum=1)
    return (numerator + denominator - 1) // denominator


def rent_exempt_lamports(data_bytes: int) -> int:
    """Apply the pinned local default ``(bytes + 128) * 6,960`` model."""

    data_bytes = _integer("data_bytes", data_bytes, minimum=0, maximum=U64_MAX)
    return (data_bytes + ACCOUNT_RENT_OVERHEAD_BYTES) * LAMPORTS_PER_RENT_BYTE


def loader_v3_persistent_rent_lamports(max_elf_bytes: int) -> int:
    """ProgramData plus Program principal for exact-size loader-v3 allocation."""

    max_elf_bytes = _integer("max_elf_bytes", max_elf_bytes, minimum=1, maximum=U64_MAX)
    return rent_exempt_lamports(
        LOADER_V3_PROGRAMDATA_METADATA_BYTES + max_elf_bytes
    ) + rent_exempt_lamports(LOADER_V3_PROGRAM_ACCOUNT_BYTES)


def clear_work_v2_body_bytes(outcomes: int, orders: int, owners: int) -> int:
    """Exact active-width relation body from ``relation_v1_stream_v2``."""

    outcomes = _integer("outcomes", outcomes, minimum=2, maximum=16)
    orders = _integer("orders", orders, minimum=0, maximum=64)
    owners = _integer("owners", owners, minimum=0, maximum=64)
    if owners > orders:
        raise AuditInputError("owners cannot exceed orders")
    return (
        678
        + 73 * orders
        + 68 * owners
        + 336 * outcomes
        + 16 * orders * outcomes
        + 16 * owners * outcomes
    )


def clear_work_v2_account_bytes(outcomes: int, orders: int, owners: int) -> int:
    """V1 outer header plus active owner interner plus exact V2 body."""

    body = clear_work_v2_body_bytes(outcomes, orders, owners)
    # 158-byte fixed header + 2-byte owner count + 32 bytes/active owner.
    return 160 + 32 * owners + body


def candidate_feed_v2_account_bytes(outcomes: int, orders: int, slices: int) -> int:
    """Model-only active CandidateFeed successor projection."""

    outcomes = _integer("outcomes", outcomes, minimum=2, maximum=16)
    orders = _integer("orders", orders, minimum=1, maximum=64)
    slices = _integer("slices", slices, minimum=0, maximum=416)
    return (
        CANDIDATE_FEED_V2_FIXED_BYTES
        + 8 * outcomes
        + 8 * orders
        + PAIRING_SLICE_BYTES * slices
    )


def active_width_candidate_savings_lamports(
    outcomes: int,
    orders: int,
    owners: int,
    slices: int,
    *,
    feed_orders: int | None = None,
    candidates: int = 1,
) -> int:
    """Current ClearWork+Feed rent minus proposed active-width rent.

    ``feed_orders`` is explicit because the frozen historical comparison used a
    four-order ClearWork fixture and a separately recorded two-order feed
    fixture.  Omitting it models one coherent active geometry.
    """

    candidates = _integer("candidates", candidates, minimum=1, maximum=U32_MAX)
    feed_orders = orders if feed_orders is None else feed_orders
    current = rent_exempt_lamports(CLEAR_WORK_V1_ACCOUNT_BYTES) + rent_exempt_lamports(
        CANDIDATE_FEED_V1_ACCOUNT_BYTES
    )
    proposed = rent_exempt_lamports(
        clear_work_v2_account_bytes(outcomes, orders, owners)
    ) + rent_exempt_lamports(
        candidate_feed_v2_account_bytes(outcomes, feed_orders, slices)
    )
    return candidates * (current - proposed)


def receipt_and_ledger_rent_lamports() -> int:
    return rent_exempt_lamports(
        SETTLEMENT_RECEIPT_V2_ACCOUNT_BYTES
    ) + rent_exempt_lamports(GENERAL_FUNDING_LEDGER_V1_ACCOUNT_BYTES)


def receipt_page_minimum_live_entries() -> int:
    """First live-entry count for which one modeled page locks less principal."""

    threshold = ceil_div(
        rent_exempt_lamports(RECEIPT_PAGE_V1_ACCOUNT_BYTES),
        receipt_and_ledger_rent_lamports(),
    )
    if threshold > RECEIPTS_PER_PAGE:
        raise AuditInputError("receipt page has no rent crossover within its capacity")
    return threshold


def receipt_page_savings_lamports(receipts: int) -> int:
    """Model-only saving with enough 16-entry pages to cover ``receipts``."""

    receipts = _integer("receipts", receipts, minimum=1, maximum=U32_MAX)
    pages = ceil_div(receipts, RECEIPTS_PER_PAGE)
    current = receipts * receipt_and_ledger_rent_lamports()
    proposed = pages * rent_exempt_lamports(RECEIPT_PAGE_V1_ACCOUNT_BYTES)
    return current - proposed


def series_capital_time(
    allocation_atoms: int,
    instance_count: int,
    activation_slot: int,
    first_debit_slot: int,
    stride_slots: int,
) -> int:
    """Exact fully-prepaid Series capital-time in atom-slots.

    ``allocation_atoms`` may be lamports or one separately reported collateral
    atom compartment.  Mixing those units is forbidden by the caller-facing
    review.  Inputs follow the finite Series schedule: singleton stride is zero;
    multi-instance stride is positive; the final debit slot must fit ``u64``.
    The result intentionally uses Python's unbounded integer because a valid
    sum of per-instance ``u64`` quantities need not itself fit ``u64``.
    """

    allocation_atoms = _integer(
        "allocation_atoms", allocation_atoms, minimum=0, maximum=U64_MAX
    )
    instance_count = _integer(
        "instance_count", instance_count, minimum=1, maximum=U32_MAX
    )
    activation_slot = _integer(
        "activation_slot", activation_slot, minimum=0, maximum=U64_MAX
    )
    first_debit_slot = _integer(
        "first_debit_slot", first_debit_slot, minimum=0, maximum=U64_MAX
    )
    stride_slots = _integer("stride_slots", stride_slots, minimum=0, maximum=U64_MAX)
    if first_debit_slot < activation_slot:
        raise AuditInputError("first debit cannot precede activation")
    if (instance_count == 1 and stride_slots != 0) or (
        instance_count > 1 and stride_slots == 0
    ):
        raise AuditInputError("singleton stride must be zero and recurring stride positive")
    final_debit_slot = first_debit_slot + (instance_count - 1) * stride_slots
    if final_debit_slot > U64_MAX:
        raise AuditInputError("final debit slot exceeds u64")
    wait_sum_slots = (
        instance_count * (first_debit_slot - activation_slot)
        + stride_slots * instance_count * (instance_count - 1) // 2
    )
    return allocation_atoms * wait_sum_slots


def claim_basis_v2_body_bytes(
    degree: int, outcomes: int, payout_count: int, knot_count: int
) -> int:
    """Model-only active codec that reconstructs all omitted V1 padding."""

    degree = _integer("degree", degree, minimum=0, maximum=3)
    outcomes = _integer("outcomes", outcomes, minimum=2, maximum=16)
    payout_count = _integer("payout_count", payout_count, minimum=0, maximum=16)
    knot_count = _integer("knot_count", knot_count, minimum=1, maximum=16)
    expected_knots = outcomes - 1 if degree == 0 else outcomes + 1 - degree
    if knot_count != expected_knots:
        raise AuditInputError("knot_count does not match degree/outcome geometry")
    if degree == 0:
        if payout_count == 0:
            raise AuditInputError("degree zero requires at least one payout row")
        return 32 + 8 * payout_count * outcomes + outcomes + 16 * knot_count
    if payout_count != 0:
        raise AuditInputError("degrees one through three require zero payout rows")
    return 32 + 16 * knot_count


def _provenance_rows(root: Path) -> dict[str, dict[str, Any]]:
    rows: dict[str, dict[str, Any]] = {}
    for relative, expected in PROVENANCE_SHA256.items():
        path = root / relative
        actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        rows[relative] = {
            "kind": "whole_file",
            "expected_sha256": expected,
            "actual_sha256": actual,
        }
    for relative, fragments in PROVENANCE_FRAGMENTS.items():
        path = root / relative
        source = path.read_text(encoding="utf-8") if path.is_file() else ""
        missing = [fragment for fragment in fragments if fragment not in source]
        expected = hashlib.sha256("\0".join(fragments).encode("utf-8")).hexdigest()
        rows[relative] = {
            "kind": "semantic_fragments",
            "fragment_count": len(fragments),
            "missing_fragments": missing,
            "expected_sha256": expected,
            "actual_sha256": None if missing else expected,
        }
    return rows


def snapshot(root: Path | None = None) -> dict[str, Any]:
    root = root or Path(__file__).resolve().parents[1]
    receipt_pair_rent = receipt_and_ledger_rent_lamports()
    receipt_page_rent = rent_exempt_lamports(RECEIPT_PAGE_V1_ACCOUNT_BYTES)
    historical_profiles = {
        name: {
            "elf_bytes": size,
            "persistent_loader_rent_lamports": loader_v3_persistent_rent_lamports(size),
        }
        for name, size in CAPABILITY_PROFILE_ELF_BYTES.items()
    }
    return {
        "schema": SCHEMA,
        "as_of_date": AS_OF_DATE,
        "claim_boundary": (
            "No current-working-tree linked ELF, CU, account-meta, stack, deployment, "
            "devnet, or mainnet claim is made."
        ),
        "constants": {
            "evidence_class": SOURCE_DERIVED,
            "account_rent_overhead_bytes": ACCOUNT_RENT_OVERHEAD_BYTES,
            "lamports_per_rent_byte": LAMPORTS_PER_RENT_BYTE,
            "rent_formula": "(data_bytes + 128) * 6960",
        },
        "historical_artifacts": {
            "evidence_class": HISTORICAL_ARTIFACT,
            "unsealed_193c": {
                "elf_sha256": CURRENT_UNSEALED_HISTORICAL_ELF_SHA256,
                "elf_bytes": CURRENT_UNSEALED_HISTORICAL_ELF_BYTES,
                "persistent_loader_rent_lamports": loader_v3_persistent_rent_lamports(
                    CURRENT_UNSEALED_HISTORICAL_ELF_BYTES
                ),
            },
            "capability_profile_commit": CAPABILITY_PROFILE_COMMIT,
            "capability_profiles": historical_profiles,
        },
        "active_width": {
            "clear_work_evidence_class": SOURCE_DERIVED,
            "candidate_feed_evidence_class": MODEL_ONLY,
            "clear_work_shape": {"outcomes": 2, "orders": 4, "owners": 3},
            "candidate_feed_shape": {"outcomes": 2, "orders": 2, "slices": 1},
            "clear_work_v1_bytes": CLEAR_WORK_V1_ACCOUNT_BYTES,
            "clear_work_v2_bytes": clear_work_v2_account_bytes(2, 4, 3),
            "candidate_feed_v1_bytes": CANDIDATE_FEED_V1_ACCOUNT_BYTES,
            "candidate_feed_v2_bytes": candidate_feed_v2_account_bytes(2, 2, 1),
            "three_candidate_savings_lamports": active_width_candidate_savings_lamports(
                2, 4, 3, 1, feed_orders=2, candidates=3
            ),
        },
        "receipt_page": {
            "evidence_class": MODEL_ONLY,
            "receipt_and_ledger_rent_lamports": receipt_pair_rent,
            "page_rent_lamports": receipt_page_rent,
            "minimum_live_entries_for_lower_locked_principal": (
                receipt_page_minimum_live_entries()
            ),
            "receipts": 416,
            "pages": ceil_div(416, RECEIPTS_PER_PAGE),
            "savings_lamports": receipt_page_savings_lamports(416),
        },
        "series_capital_time": {
            "evidence_class": MODEL_ONLY,
            "formula": (
                "A * (N * (first_debit_slot - activation_slot) "
                "+ stride_slots * N * (N - 1) / 2)"
            ),
            "illustrative_units": "atom-slots; lamports and collateral atoms stay separate",
            "illustrative_input": {
                "allocation_atoms": 7,
                "instance_count": 4,
                "activation_slot": 10,
                "first_debit_slot": 20,
                "stride_slots": 3,
            },
            "illustrative_result_atom_slots": series_capital_time(7, 4, 10, 20, 3),
        },
        "compressed_claim_basis": {
            "evidence_class": MODEL_ONLY,
            "fixed_v1_body_bytes": 2_352,
            "binary_degree_zero_body_bytes": claim_basis_v2_body_bytes(0, 2, 2, 1),
            "binary_rent_savings_lamports": rent_exempt_lamports(2_352)
            - rent_exempt_lamports(claim_basis_v2_body_bytes(0, 2, 2, 1)),
        },
        "provenance": _provenance_rows(root),
    }


def check_frozen_examples(root: Path | None = None) -> None:
    """Fail closed on arithmetic or pinned-provenance drift."""

    root = root or Path(__file__).resolve().parents[1]
    expected = {
        "historical_loader_rent": 14_495_292_720,
        "clear_work_small_bytes": 2_326,
        "clear_work_small_saving": 332_186_880,
        "feed_small_bytes": 263,
        "feed_small_saving": 41_780_880,
        "three_candidate_saving": 1_121_903_280,
        "receipt_pair_rent": 3_883_680,
        "receipt_page_rent": 26_169_600,
        "receipt_crossover": 7,
        "receipt_416_saving": 935_201_280,
        "series_fixture": 406,
        "basis_binary_bytes": 82,
        "basis_binary_saving": 15_799_200,
    }
    actual = {
        "historical_loader_rent": loader_v3_persistent_rent_lamports(
            CURRENT_UNSEALED_HISTORICAL_ELF_BYTES
        ),
        "clear_work_small_bytes": clear_work_v2_account_bytes(2, 4, 3),
        "clear_work_small_saving": rent_exempt_lamports(CLEAR_WORK_V1_ACCOUNT_BYTES)
        - rent_exempt_lamports(clear_work_v2_account_bytes(2, 4, 3)),
        "feed_small_bytes": candidate_feed_v2_account_bytes(2, 2, 1),
        "feed_small_saving": rent_exempt_lamports(CANDIDATE_FEED_V1_ACCOUNT_BYTES)
        - rent_exempt_lamports(candidate_feed_v2_account_bytes(2, 2, 1)),
        "three_candidate_saving": active_width_candidate_savings_lamports(
            2, 4, 3, 1, feed_orders=2, candidates=3
        ),
        "receipt_pair_rent": receipt_and_ledger_rent_lamports(),
        "receipt_page_rent": rent_exempt_lamports(RECEIPT_PAGE_V1_ACCOUNT_BYTES),
        "receipt_crossover": receipt_page_minimum_live_entries(),
        "receipt_416_saving": receipt_page_savings_lamports(416),
        "series_fixture": series_capital_time(7, 4, 10, 20, 3),
        "basis_binary_bytes": claim_basis_v2_body_bytes(0, 2, 2, 1),
        "basis_binary_saving": rent_exempt_lamports(2_352)
        - rent_exempt_lamports(claim_basis_v2_body_bytes(0, 2, 2, 1)),
    }
    if actual != expected:
        raise AssertionError(f"frozen arithmetic drift: expected={expected!r} actual={actual!r}")
    provenance = _provenance_rows(root)
    drift = {
        path: row
        for path, row in provenance.items()
        if row["actual_sha256"] != row["expected_sha256"]
    }
    if drift:
        raise AssertionError(f"pinned provenance drift: {drift!r}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="check frozen examples and provenance digests"
    )
    args = parser.parse_args()
    if args.check:
        check_frozen_examples()
        print("rent-capital-time-audit: PASS")
        return 0
    print(json.dumps(snapshot(), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
