#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Complete current-runtime account/value terminal classification.

The compact table mirrors the sealed v2 account probe.  ``build_terminal``
expands each row into the stricter schema checked by ``terminal_admission``.
PASS on a permanent row means only that permanent capitalization is stated
honestly; it does not make the protocol-level terminal result pass.
"""

from __future__ import annotations

from typing import Any


P = "PERMANENT_INFRA"
S = "UNCLASSIFIED_STOP"
R = "REFUNDABLE_TRANSIENT"

# name, bytes, Rent::default minimum, semantic scope, hard maximum in that
# scope (None means no admitted bound), class, explicit STOP blockers.
ACCOUNT_ROWS: tuple[tuple[str, int, int, str, int | None, str, tuple[str, ...]], ...] = (
    ("artifact.policy.final", 266, 2_742_240, "PER_CONTENT_DIGEST", 1, P, ()),
    ("artifact.policy.stage", 402, 3_688_800, "PER_CONTENT_DIGEST", 1, S,
     ("RENT.ARTIFACT_PREFUND_WINDFALL",)),
    ("artifact.grid.final", 589, 4_990_320, "PER_CONTENT_DIGEST", 1, P, ()),
    ("artifact.grid.stage", 725, 5_936_880, "PER_CONTENT_DIGEST", 1, S,
     ("RENT.ARTIFACT_PREFUND_WINDFALL",)),
    ("artifact.terms.final", 1_656, 12_416_640, "PER_CONTENT_DIGEST", 1, P, ()),
    ("artifact.terms.stage", 1_792, 13_363_200, "PER_CONTENT_DIGEST", 1, S,
     ("RENT.ARTIFACT_PREFUND_WINDFALL",)),
    ("artifact.batch_policy.final", 64, 1_336_320, "PER_CONTENT_DIGEST", 1, P, ()),
    ("artifact.batch_policy.stage", 200, 2_282_880, "PER_CONTENT_DIGEST", 1, S,
     ("RENT.ARTIFACT_PREFUND_WINDFALL",)),
    ("realm", 70, 1_378_080, "PER_REALM_ID", 1, P, ()),
    ("profile", 100, 1_586_880, "PER_PROFILE_ID", 1, P, ()),
    ("market", 726, 5_943_840, "PER_MARKET_ID", 1, P, ()),
    ("hoard", 108, 1_642_560, "PER_MARKET_ID", 1, P, ()),
    ("kernel", 1_255, 9_625_680, "PER_MARKET_ID", 1, P, ()),
    ("supply_ledger", 333, 3_208_560, "PER_MARKET_ID", 1, P, ()),
    ("resolution.v2", 165, 2_039_280, "PER_MARKET_SELECTED_SCHEMA", 1, P, ()),
    ("resolution.v3", 319, 3_111_120, "PER_MARKET_SELECTED_SCHEMA", 1, P, ()),
    ("resolution.v4", 383, 3_556_560, "PER_MARKET_SELECTED_SCHEMA", 1, P, ()),
    ("token.outcome_mint", 82, 1_461_600, "PER_MARKET", 8, P, ()),
    ("token.hoard_immutable_owner", 170, 2_074_080, "PER_MARKET", 1, P, ()),
    ("position", 220, 2_422_080, "PER_MARKET_OWNER", None, S,
     ("RENT.ACCOUNT_REFUND_UNOWNED", "PROFILE.STORAGE_INVENTORY_INCOMPLETE")),
    ("replay", 84, 1_475_520, "PER_MARKET_OWNER_GENERATION", None, S,
     ("RENT.ACCOUNT_REFUND_UNOWNED", "PROFILE.STORAGE_INVENTORY_INCOMPLETE")),
    ("feed", 124, 1_753_920, "PER_SOURCE_FEED", None, S,
     ("SOURCE.NO_TERMINAL_RELEASE", "RENT.ACCOUNT_REFUND_UNOWNED")),
    ("source.spec", 292, 2_923_200, "PER_SOURCE_SPEC", None, S,
     ("SOURCE.NO_TERMINAL_RELEASE", "RENT.ACCOUNT_REFUND_UNOWNED")),
    ("source.archive", 2_560, 18_708_480, "PER_SOURCE_WINDOW", None, S,
     ("SOURCE.NO_TERMINAL_RELEASE", "RENT.ACCOUNT_REFUND_UNOWNED")),
    ("order.page", 4_012, 28_814_400, "PER_MARKET_EPOCH_PAGE", None, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED", "DIRECT.EMPTY_FROZEN_NO_LAPSE")),
    ("order.reservation.v1", 570, 4_858_080, "PER_ORDER", None, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED", "DIRECT.EMPTY_FROZEN_NO_LAPSE")),
    # DIRECT.TOP3_SELECT_CU_STOP was retired with the 187d5ee1… artifact: V2
    # top-three selection, which exhausted the 1.4m-CU ceiling on every prior
    # seal, now completes at a measured 226,071 CU.  The rows below still STOP
    # on their remaining blockers (no lapse, unowned refunds, persistent rent).
    ("direct.epoch.v3", 344, 3_285_120, "PER_DIRECT_EPOCH", None, S,
     ("DIRECT.EMPTY_FROZEN_NO_LAPSE",)),
    ("direct.candidate.v2", 440, 3_953_280, "PER_EPOCH_CANDIDATE", 64, S,
     ("DIRECT.CANDIDATE_RENT_PERSISTS", "DIRECT.ACCOUNT_REFUND_UNOWNED")),
    ("direct.window.v1", 456, 4_064_640, "PER_DIRECT_EPOCH", 1, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED",)),
    ("direct.receipt", 217, 2_401_200, "PER_SELECTED_CANDIDATE", 1, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED",)),
    ("direct.final_pot", 262, 2_714_400, "PER_SELECTED_CANDIDATE", 1, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED",)),
    ("legacy.epoch.v2", 328, 3_173_760, "PER_LEGACY_EPOCH", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE",)),
    ("legacy.candidate", 305, 3_013_680, "PER_LEGACY_CANDIDATE", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE",)),
    ("legacy.candidate_feed", 6_266, 44_502_240, "PER_LEGACY_EPOCH", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE",)),
    ("legacy.clear_work", 48_750, 340_190_880, "PER_LEGACY_CLEAR_JOB", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE",)),
    ("resolution.work.v1", 1_296, 9_911_040, "PER_ACTIVE_MARKET_WORK", 1, R, ()),
    ("resolution.reserve.v1", 0, 890_880, "PER_ACTIVE_MARKET_WORK", 1, R, ()),
)


EXPECTED_ACCOUNTS = {row[0] for row in ACCOUNT_ROWS}


def _row(
    name: str,
    bytes_: int,
    rent: int,
    scope: str,
    hard_max: int | None,
    lifecycle: str,
    blockers: tuple[str, ...],
) -> dict[str, Any]:
    maximum: dict[str, Any]
    if hard_max is None:
        maximum = {"kind": "UNADMITTED", "hard_max": None}
    else:
        maximum = {"kind": "FIXED", "hard_max": hard_max}
    common: dict[str, Any] = {
        "bytes": bytes_,
        "rent_lamports": rent,
        "scope": scope,
        "max_instances": maximum,
        "lifecycle_class": lifecycle,
    }
    if lifecycle == P:
        common.update(
            physical_close_route="NONE",
            rent_principal_owner="NONE",
            full_rent_capitalized=True,
            promotion="PASS",
        )
    elif lifecycle == S:
        common.update(
            physical_close_route="NONE",
            rent_principal_owner="UNRECORDED",
            promotion="STOP",
            blocking_ids=list(blockers),
        )
    elif lifecycle == R:
        common.update(
            physical_close_route="FinalizeResolutionWorkV1_or_AbortResolutionWorkV1",
            rent_principal_recorded=True,
            rent_principal_owner="ResolutionWork.payer",
            donation_disposition="FROZEN_NEUTRAL_SINK",
            expiry_or_reaper="PASS",
            close_bank_evidence="PASS",
            rollback_bank_evidence="PASS",
            promotion="PASS",
        )
    else:  # pragma: no cover - table construction invariant
        raise ValueError(f"unknown lifecycle class {lifecycle!r}")
    return common


def build_terminal(runtime_ref: str = "0" * 40) -> dict[str, Any]:
    accounts = {
        name: _row(name, bytes_, rent, scope, maximum, lifecycle, blockers)
        for name, bytes_, rent, scope, maximum, lifecycle, blockers in ACCOUNT_ROWS
    }
    blockers = sorted(
        {
            "SOURCE.DEFAULT_REGISTRY_EMPTY",
            "TOKEN.OUTCOME_MINT_PERMANENT",
            "HOARD.RESIDUAL_DISPOSITION_UNSELECTED",
            "CLAIM.SUBLOT_FRAGMENT_NO_TOTAL_EXIT",
            *(blocker for row in ACCOUNT_ROWS for blocker in row[6]),
        }
    )
    return {
        "schema": "dragons-clutch/terminal-admission/v1",
        "runtime_ref": runtime_ref,
        "status": "STOP",
        "source_release": {
            "default_release_available": False,
            "value_admission_without_release": False,
            "endow_fail_closed_bank_evidence": "PASS",
        },
        "reachable_terminal_shapes": {
            "resolution_work.success": {"reachable": True, "required": True, "status": "PASS"},
            "resolution_work.expired_abort": {
                "reachable": True,
                "required": True,
                "status": "PASS",
            },
            "source.default_value_admission": {
                "reachable": False,
                "required": True,
                "status": "STOP",
            },
            "direct.empty_frozen": {"reachable": True, "required": True, "status": "STOP"},
            "direct.retained_top_three": {
                "reachable": True,
                "required": True,
                "status": "STOP",
            },
            "all_program_rent_reclaimed": {
                "reachable": False,
                "required": True,
                "status": "STOP",
            },
        },
        "accounts": accounts,
        "token_residues": {
            "outcome_mint_close": {"required": True, "status": "STOP"},
            "hoard_donation_or_burn_forfeiture": {"required": True, "status": "STOP"},
            "fractional_sub_lot": {"required": True, "status": "STOP"},
        },
        "hoard_counts_as_liveness_funding": False,
        "claims_universal_no_stranded_value": False,
        "blocking_ids": blockers,
    }

# Keep this module data-only: the executable validation lives in
# terminal_admission.py.
