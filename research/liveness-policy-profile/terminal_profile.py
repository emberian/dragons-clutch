#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Complete current-runtime account/value terminal classification.

The compact table mirrors the sealed v2 account probe, plus the account
families the probe predates — the Direct V3 rows, the two T2-8 general-plane
rows, and the TerminalClosure funding ledger (the probe source archived at the
sealed ``runtime_ref`` enumerates none of them; ``policy.py`` pins the
post-probe rows byte-exactly and keeps the probe equality on everything else).
``build_terminal`` expands each row into the stricter schema checked by
``terminal_admission``.  PASS on a permanent row means only that permanent
capitalization is stated honestly; it does not make the protocol-level
terminal result pass.
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
    # The per-Realm revenue-policy record (B4f, ADOPTED_2026-08-20 item 8;
    # docs/design/REVENUE_POLICY_V1.md section 3), classified beside the Realm
    # row it serves and landed here BEFORE its implementation lane per the
    # rows-first rule.  Bound: exactly one per Realm, ever -- creatable only
    # inside the Realm's own InitRealm transition (D4: absence IS the
    # zero-take state; no retrofit route exists).  Byte pin:
    # programs/solana-layout/src/revenue.rs REVENUE_POLICY_RECORD_BYTES =
    # 2 + 32 + 32 + 32 + 56 + 1 + 1 = 156, tag 27.  Unlike every legacy
    # general-plane family, the record embeds the TerminalIdentityV1 header
    # AND its GeneralFundingLedgerV1 sibling is MANDATORY at creation, so the
    # unowned-refund residual cannot arise; CloseRevenuePolicyRecord (tag 68)
    # is the standing close route, paying the exact recorded principal to the
    # exact recorded payer with the surplus burned.  The row still STOPs, on
    # the one honest residual: the close is gated on the Realm account's
    # absence, and the realm row above is PERMANENT_INFRA with no close route,
    # so the record's principal is capitalized for the Realm's whole life --
    # "Realm-lifetime" means permanent in practice, and the admissible close
    # is what lets this classification tighten later without an ABI change.
    ("revenue.policy_record.v1", 156, 1_976_640, "PER_REALM_ID", 1, S,
     ("REVENUE.REALM_PERMANENCE_HOLDS_RECORD",)),
    # The RevenueVaultV1 row of the design's section 4 is deliberately NOT
    # here: B4c froze all five ResolutionWork charges at zero as policy and
    # NO VAULT IS BUILT.  The row lands, contingent, only if a future
    # optional-service charge decision revives L1.
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
    # TerminalClosure (tags 60-67) closes the GENERAL plane's instance of this
    # 4,012-byte page: CloseGeneralPage (tag 63) pays the recorded principal to
    # the recorded payer once every live record's reservation is RELEASED or
    # CONSUMED, and the sealed cleared-epoch walk drives it.  The row still
    # STOPs, because the row's scope also covers the Direct V4 instance, which
    # has no close route at all: `init_direct_v4_order_page`
    # (instructions/genesis.rs) creates it with
    # `create_pda_account_full_principal` — no funding-ledger sibling, no
    # recorded payer — and no V3 route closes it.  The sealed V3 campaign
    # measures 28,814,401 lamports still held on that page after both settle
    # and lapse (`DirectV3 STRAND ... order.page` in all three bank runs), so
    # the strand is named as its own blocking id rather than left inside the
    # generic unowned-refund one.
    ("order.page", 4_012, 28_814_400, "PER_MARKET_EPOCH_PAGE", None, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED", "DIRECT.EMPTY_FROZEN_NO_LAPSE",
      "DIRECT.ORDER_PAGE_RENT_PERSISTS")),
    # The general plane's reservations release (tag 60, owner-signed, whole
    # envelope to the stored owner) and their archives close (tag 62), both
    # driven in the sealed cleared- and lapsed-epoch walks.  The row keeps its
    # STOP on the direct-plane instances, which have neither route.
    # Reservation schema generation v3 (version byte 4) at the 0d52c561… seal:
    # the PartialFillLedger wave made the account the cumulative consumption
    # ledger (entitled_units/consumed_units plus the reserved fee zone) and the
    # VirtualMergeCredit wave split quantity from cash (paid_units), 570 -> 618
    # bytes.  Same family, same tag, no new row.
    ("order.reservation.v1", 618, 5_192_160, "PER_ORDER", None, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED", "DIRECT.EMPTY_FROZEN_NO_LAPSE")),
    # DIRECT.TOP3_SELECT_CU_STOP was retired with the 187d5ee1… artifact: V2
    # top-three selection, which exhausted the 1.4m-CU ceiling on every prior
    # seal, now completes at a measured 226,071 CU.  The rows below still STOP
    # on their remaining blockers (no lapse, unowned refunds, persistent rent).
    # +1 byte for basis_degree, the moment cone bound on chain (0d52c561…).
    ("direct.epoch.v3", 345, 3_292_080, "PER_DIRECT_EPOCH", None, S,
     ("DIRECT.EMPTY_FROZEN_NO_LAPSE",)),
    ("direct.candidate.v2", 440, 3_953_280, "PER_EPOCH_CANDIDATE", 64, S,
     ("DIRECT.CANDIDATE_RENT_PERSISTS", "DIRECT.ACCOUNT_REFUND_UNOWNED")),
    ("direct.window.v1", 456, 4_064_640, "PER_DIRECT_EPOCH", 1, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED",)),
    ("direct.receipt", 217, 2_401_200, "PER_SELECTED_CANDIDATE", 1, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED",)),
    ("direct.final_pot", 262, 2_714_400, "PER_SELECTED_CANDIDATE", 1, S,
     ("DIRECT.ACCOUNT_REFUND_UNOWNED",)),
    # The Direct V3 merge introduced six persistent program-owned families
    # (programs/solana-layout/src/direct_selection_v3.rs).  Unlike V1/V2,
    # every transient V3 account records its exact payer principal and a
    # monotone donation floor in a DirectFundingLedgerV3, and every phase
    # has a terminal exit that physically closes it via
    # close_funded_account (principal to the exact stored payer, all
    # surplus to the frozen incinerator sink, resize 0, reassign):
    # SettleDirectV3 closes candidate, window, receipt, pot, WorkBudget,
    # and both reservations; LapseEmpty / LapseUnselected / LapseSelected
    # close every transient live in their phase; SubmitCandidate closes a
    # displaced fourth candidate in the admitting transaction; and
    # AbortUnfrozen closes the 0..2-reservation prefix
    # (programs/clutch-sbf/program/src/instructions/direct_selection_v3/).
    # DIRECT.V3_CLOSE_EVIDENCE_UNSEALED is RETIRED at the e8ba31d5… seal,
    # by exactly the measurement its own text named — "a sealed
    # measurement, exactly as DIRECT.TOP3_SELECT_CU_STOP retired".  Three
    # independent bank runs of
    # programs/clutch-sbf/svm-tests/tests/direct_selection_v3.rs against
    # this artifact (logs/bank/direct_selection_v3{,_run2,_run3}.log) drive
    # every close route those four families have — the displacing Submit,
    # Finalize's two unselected closes, Settle's seven, all three Lapse
    # phases, and the zero/one/two AbortUnfrozen prefixes — and record, per
    # route, what each account held before it closed, the exact lamport
    # delta on every recorded payer and on the frozen sink, and an asserted
    # equality between the two.  Rollback is measured on the same routes:
    # substituting a close recipient at Finalize or at the two-order Abort
    # refuses and leaves the accounts byte-and-lamport identical, as does an
    # underfunded Freeze or Submit.  Every one of those numbers is identical
    # across all three runs.  The four rows are therefore
    # REFUNDABLE_TRANSIENT, not STOP; policy.py::require_v3_close_evidence
    # welds the two halves together so neither the classification nor the
    # evidence can drift alone.  This retires one blocker; it promotes no
    # subsystem — live_v3 stays false and no V3 admission row is derived.
    # Other families strand rent
    # structurally: the terminal Epoch V4 persists forever as the durable
    # receipt — every terminal route (settle, all three lapses, abort)
    # ends in write_epoch_v4, and no handler closes an Epoch V4, so its
    # recorded principal is unreclaimable
    # (DIRECT.EPOCH_RECEIPT_RENT_PERSISTS) — and the 96-byte
    # DirectBatchPolicy V3 final artifact is epoch-context-addressed
    # (seeds::direct_batch_policy_v3_pda binds epoch id and digest), so
    # one permanent copy of identical bytes accrues per epoch with no
    # close route for final artifacts
    # (DIRECT.POLICY_ARTIFACT_RENT_PERSISTS).  The sealed campaign found a
    # THIRD stranded family the promotion report's rent story omits: the
    # 4,012-byte order.page above.  InitOrderPageV4 creates one per V4
    # epoch and records its principal in the Epoch's page ledger, but no V3
    # route closes it — the measured post-settle and post-lapse balance is
    # 28,814,401 lamports, four times the other two together.  The row
    # already STOPs on its own blockers, so nothing is over-admitted, but
    # the honest per-epoch V3 strand is 35,941,440 lamports of rent, not
    # the 7,127,040 the report quotes.
    # Candidate V3 bounds: at
    # most 3 live per epoch (top-3 retention; the displaced worst closes
    # immediately) and at most 64 ever admitted (one per competitive tick,
    # MAX_DIRECT_TICKS_V3).  Reservation V2: exactly the frozen two-order
    # pair per V4 epoch.  The V3 flow also creates receipt/pot instances
    # of the already-inventoried 217/262-byte shapes at v3 PDAs with
    # funding recorded in the Window; both close at Settle or
    # LapseSelected, so no new persistent family arises from them.
    ("direct.epoch.v4", 672, 5_568_000, "PER_DIRECT_EPOCH", None, S,
     ("DIRECT.EPOCH_RECEIPT_RENT_PERSISTS",)),
    ("direct.candidate.v3", 488, 4_287_360, "PER_EPOCH_CANDIDATE", 3, R, ()),
    ("direct.window.v3", 632, 5_289_600, "PER_DIRECT_EPOCH", 1, R, ()),
    ("direct.work_budget.v1", 248, 2_616_960, "PER_DIRECT_EPOCH", 1, R, ()),
    ("direct.reservation.v2", 618, 5_192_160, "PER_DIRECT_EPOCH", 2, R, ()),
    ("artifact.direct_batch_policy_v3.final", 96, 1_559_040,
     "PER_EPOCH_CONTENT_DIGEST", 1, S,
     ("DIRECT.POLICY_ARTIFACT_RENT_PERSISTS",)),
    ("artifact.direct_batch_policy_v3.stage", 232, 2_505_600,
     "PER_EPOCH_CONTENT_DIGEST", 1, S,
     ("RENT.ARTIFACT_PREFUND_WINDFALL",)),
    # The T2-6 general epoch plane reuses the epoch/candidate/feed/clear-work
    # families below at their exact widths (InitEpoch creates the same
    # 328-byte EpochAccount at seeds::epoch_pda; the walk consumes the same
    # feed and checkpoint shapes), so no new row arises from that reuse.  Its
    # one new persistent family is the deadline-window companion: created by
    # InitEpoch (tag 49) at seeds::epoch_window_pda, exactly one per general
    # epoch, and closed by no handler — TerminalClosure (rent-reclaim close
    # of ClearWork/feed/receipts/pot) is a recorded ranked blocker in the
    # Tier 2 plan, deliberately standing.  T2-7 moves the row 84 -> 231:
    # EpochWindow v2 (EPOCH_WINDOW_ACCOUNT_BYTES, layout clearing.rs) appends
    # the candidate-window deadline, the frozen live cardinality, the bounded
    # 3-slot retained-candidate registry, and the selection result — the
    # promised format revision of the same account, not a second family.
    # The T2-7 CandidateFeedStage prefix (account tag 25) is likewise NOT a
    # new row: it is the same 6,266-byte account as the feed (tag 18) while
    # its content is being written — a stage prefix instead of a feed header,
    # replaced one-way by SealCandidate — so legacy.candidate_feed covers it.
    #
    # TERMINAL CLOSURE (tags 60-67, the 966ee2c merge) gives every row below a
    # real physical close route, driven end to end on a real bank in
    # `logs/bank/terminal_closure.log`: a CLEARED epoch's machinery held
    # 531,652,377 lamports and 531,639,600 of them were reclaimed to the exact
    # recorded payers, 12,777 burned at the frozen incinerator (exactly the two
    # injected donations), residual exactly 1,336,320 — the declared-permanent
    # 64-byte batch-policy artifact.  `PROFILE.STORAGE_INVENTORY_INCOMPLETE` was
    # attached to these rows because *no close path existed*; that reason is
    # discharged.  It is replaced, not deleted, by the two residuals that
    # actually remain — both named, both structural, neither hidden:
    #
    #   RENT.ACCOUNT_REFUND_UNOWNED.  The GeneralFundingLedgerV1 sibling is
    #   OPTIONAL at every creating instruction of the family — each accepts
    #   `accounts.len() == N || N + 1` (general_epoch.rs:145, selection.rs:196,
    #   entitlement.rs:262, genesis.rs:897).  Every close runs through
    #   `close_ledgered_group`, which requires the ledger at its canonical PDA,
    #   so an account created without it refuses to close forever and no payer
    #   is ever guessed.  The Landing commit says exactly this ("an account
    #   created without the ledger keeps the unowned-refund blocker and stands
    #   forever, by design") and the sealed LAPSED walk *proves the state is
    #   reachable*: the unregistered candidate pair stands at 47,738,640
    #   lamports after the root closes.  `rent_principal_recorded` is therefore
    #   a property of the creating call, not of the family, and no row here may
    #   claim REFUNDABLE_TRANSIENT, whose contract requires it unconditionally.
    #
    #   GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT.  `release_terminal_reservation`
    #   (tag 60) is the ONLY signer-gated step in the whole DAG — every close,
    #   tags 61-67, is permissionless.  A zero-fill or lapsed ACTIVE reservation
    #   can only be released by its owner, and CloseGeneralPage (63) requires
    #   every live record's reservation RELEASED or CONSUMED, CloseGeneralPot
    #   (64) requires every page absent, and CloseGeneralEpoch (67) requires
    #   both.  So owner abandonment holds the page, the pot, and the epoch root
    #   open at recorded rent cost.  The design declines to invent a sweep right
    #   ("retirement guaranteed is not a promise"), so `expiry_or_reaper` cannot
    #   be PASS for any row whose close sits behind that edge.  It does not
    #   apply to `epoch.receipt`: consumption is permissionless, and tag 61
    #   gates only on the receipt being exhausted.
    #
    # Both halves are welded to the sealed measurement by
    # policy.py::require_terminal_closure_evidence, in both directions.
    ("epoch.window", 231, 2_498_640, "PER_GENERAL_EPOCH", 1, S,
     ("RENT.ACCOUNT_REFUND_UNOWNED",
      "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT")),
    # T2-8's entitlement freeze creates two persistent general-plane families
    # at walk-plane PDAs, reusing the direct plane's exact byte shapes
    # (account_len::FINAL_POT / account_len::SETTLEMENT_RECEIPT) at new
    # addresses, so each needs its own row with its own scope.  Creation:
    # FreezeEntitlement (tag 58, orders_batch/entitlement.rs) creates the
    # epoch's one FinalPotAccount at seeds::pot_pda, POT_PHASE_CLOSED with
    # provably zero scalars (virtual/rounding summaries refuse); EntitleSlice
    # (tag 59) creates one SettlementReceiptAccount per (candidate, slice) at
    # seeds::receipt_pda for the SELECTED candidate — at most MAX_SLICES =
    # 416 per selected candidate, replay held by the receipt's own
    # non-existence.  Close: NO handler closes either family — consumption
    # (the widened SettlePage) stamps the receipt exhausted in place and the
    # consumed reservations persist as their own archive — the standing
    # TerminalClosure row of the SETTLEMENT_BLOCKERS ledger
    # (orders_batch/settlement.rs, which records CandidateWindowClosure and
    # EntitlementFreeze as T2-7/T2-8's retirements).  Both rows are pinned
    # post-probe in policy.py (the sealed probe enumerates the direct-plane
    # instances only) and stay UNCLASSIFIED_STOP until a close path and
    # sealed bank evidence exist.
    ("epoch.final_pot", 262, 2_714_400, "PER_GENERAL_EPOCH", 1, S,
     ("RENT.ACCOUNT_REFUND_UNOWNED",
      "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT")),
    ("epoch.receipt", 217, 2_401_200, "PER_SELECTED_CANDIDATE", 416, S,
     ("RENT.ACCOUNT_REFUND_UNOWNED",)),
    # The general funding ledger itself (account tag 26,
    # clearing.rs::GENERAL_FUNDING_LEDGER_BYTES = 85): one optional sibling per
    # created group, written in the same transition that debits the payer, and
    # closed as a member of its own group by `close_ledgered_group` — its
    # lamports are inside the group's `observed` total, so the sealed
    # conservation covers it.  It is post-probe pinned in policy.py (the probe
    # source archived at runtime_ref predates the family) and STOPs on its
    # unadmitted cardinality: one per created account of an unbounded family.
    ("general.funding_ledger", 85, 1_482_480, "PER_LEDGERED_GROUP", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE",
      "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT")),
    # The four legacy/general rows below now have real close routes too
    # (CloseGeneralEpoch 67, CloseGeneralCandidate 65 for the record/feed pair,
    # CloseGeneralClearWork 66), all driven in the sealed cleared-epoch walk.
    # They keep PROFILE.STORAGE_INVENTORY_INCOMPLETE for a reason the closure
    # wave does not touch: their cardinality is UNADMITTED, and
    # terminal_admission refuses a REFUNDABLE_TRANSIENT row whose instance count
    # is unbounded.  The optional-ledger residual is added alongside.
    # +1 byte for basis_degree, the moment cone bound on chain (0d52c561…).
    ("legacy.epoch.v2", 329, 3_180_720, "PER_LEGACY_EPOCH", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE", "RENT.ACCOUNT_REFUND_UNOWNED",
      "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT")),
    # CandidateRecord v3 (the d6929549… seal): T2-6 appends the 32-byte
    # score_digest that CompleteClearWork stamps at verification, moving the
    # record 305 -> 337 bytes; the row tracks the probe.
    ("legacy.candidate", 337, 3_236_400, "PER_LEGACY_CANDIDATE", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE", "RENT.ACCOUNT_REFUND_UNOWNED")),
    ("legacy.candidate_feed", 6_266, 44_502_240, "PER_LEGACY_EPOCH", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE", "RENT.ACCOUNT_REFUND_UNOWNED")),
    # T2-1 re-pinned CLEAR_WORK_BODY_BYTES to the checkpoint codec's
    # ENCODED_BYTES (48,592 -> 47,846), so account_len::CLEAR_WORK moved
    # 48,750 -> 48,004 at the fda59705… seal; T2-6 inserts the 2,050-byte
    # owner-interner region between header and body (the d6929549… seal),
    # moving it 48,004 -> 50,054.  The row tracks the probe.
    ("legacy.clear_work", 50_054, 349_266_720, "PER_LEGACY_CLEAR_JOB", None, S,
     ("PROFILE.STORAGE_INVENTORY_INCOMPLETE", "RENT.ACCOUNT_REFUND_UNOWNED")),
    ("resolution.work.v1", 1_296, 9_911_040, "PER_ACTIVE_MARKET_WORK", 1, R, ()),
    ("resolution.reserve.v1", 0, 890_880, "PER_ACTIVE_MARKET_WORK", 1, R, ()),
)


EXPECTED_ACCOUNTS = {row[0] for row in ACCOUNT_ROWS}

# Per-row refundable facts: the physical close route, the immutable field
# that records the principal owner, and the deadline-gated permissionless
# route that guarantees the account cannot outlive its phase.  A
# REFUNDABLE_TRANSIENT row must appear here; `_row` refuses one that does
# not, so the class can never be claimed without naming its mechanism.
#
# Every Direct V3 expiry route below is permissionless and bounded only from
# below: AbortUnfrozenDirectV4 requires `now >= submission_opens_slot` with
# no upper bound and no privileged signer
# (instructions/direct_selection_v3/freeze_abort.rs), and the three Lapse
# phases open at the selection or settlement deadline and never close.  So
# every phase a transient can be alive in has a route any keeper can call
# forever afterwards, and the sealed campaign drives all of them.
#
# NO general-plane row appears here, deliberately, even though TerminalClosure
# (tags 60-67) gives every one of them a driven, exactly-conserving close.  A
# REFUNDABLE_TRANSIENT row must carry `rent_principal_recorded is True` and
# `expiry_or_reaper == PASS` unconditionally, and the general plane satisfies
# neither unconditionally: the funding ledger is optional at creation, and the
# one owner-signed edge of the close DAG (tag 60) lets an abandoned reservation
# hold the root open.  See the block above `epoch.window` for the exact
# mechanism and the sealed evidence for each.  The correct successor is a
# versioned family whose header is embedded rather than a sibling, plus an
# admitted cardinality bound — recorded, not built.
REFUNDABLE_FACTS: dict[str, tuple[str, str, str]] = {
    "resolution.work.v1": (
        "FinalizeResolutionWorkV1_or_AbortResolutionWorkV1",
        "ResolutionWork.payer",
        "AbortResolutionWorkV1_after_expiry",
    ),
    "resolution.reserve.v1": (
        "FinalizeResolutionWorkV1_or_AbortResolutionWorkV1",
        "ResolutionWork.payer",
        "AbortResolutionWorkV1_after_expiry",
    ),
    "direct.candidate.v3": (
        "SubmitDirectCandidateV3_displacing_or_FinalizeDirectSelectionV3_or_"
        "SettleDirectV3_or_LapseUnselectedDirectV3_or_LapseSelectedDirectV3",
        "DirectCandidateV3.funding.payer",
        "LapseUnselectedDirectV3_or_LapseSelectedDirectV3_after_deadline",
    ),
    "direct.window.v3": (
        "SettleDirectV3_or_LapseUnselectedDirectV3_or_LapseSelectedDirectV3",
        "DirectWindowV3.funding.payer",
        "LapseUnselectedDirectV3_or_LapseSelectedDirectV3_after_deadline",
    ),
    "direct.work_budget.v1": (
        "SettleDirectV3_or_LapseEmptyDirectV3_or_LapseUnselectedDirectV3_or_"
        "LapseSelectedDirectV3",
        "DirectWorkBudgetV1.funding.payer",
        "LapseEmptyDirectV3_or_LapseUnselectedDirectV3_or_"
        "LapseSelectedDirectV3_after_deadline",
    ),
    "direct.reservation.v2": (
        "SettleDirectV3_or_LapseEmptyDirectV3_or_LapseUnselectedDirectV3_or_"
        "LapseSelectedDirectV3_or_AbortUnfrozenDirectV4",
        "DirectReservationV2.funding.payer",
        "AbortUnfrozenDirectV4_unfrozen_or_any_LapseDirectV3_after_deadline",
    ),
}


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
        facts = REFUNDABLE_FACTS.get(name)
        if facts is None:
            raise ValueError(f"{name}: refundable row names no close mechanism")
        close_route, principal_owner, reaper = facts
        common.update(
            physical_close_route=close_route,
            rent_principal_recorded=True,
            rent_principal_owner=principal_owner,
            donation_disposition="FROZEN_NEUTRAL_SINK",
            expiry_or_reaper="PASS",
            expiry_route=reaper,
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
            # TerminalClosure makes both general-plane terminal shapes
            # REACHABLE for the first time — each is driven to the epoch root
            # on a real bank in `logs/bank/terminal_closure.log`.  Both still
            # STOP, on the optional-ledger and abandoned-reservation residuals
            # recorded against the rows above.
            "general.cleared_epoch_closure": {
                "reachable": True,
                "required": True,
                "status": "STOP",
            },
            "general.lapsed_epoch_closure": {
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
