#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Verify the sealed R1 liveness evidence and derive its fail-closed profile.

The checker deliberately separates measured bank facts, policy inputs, runtime
reward constants, and terminal admission.  It never clamps an impossible CU
headroom request into a finite lamport quote and never emits a protocol-wide
``LivenessPolicy`` while a mandatory route is stopped.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from admission_math import (
    QuotePolicy,
    RouteQuote,
    batched_resolution_path_quote,
    exact_unique_labels,
    quote_route,
    require_runtime_schedule_covers_batches,
    require_runtime_schedule_covers_policy,
    resolution_path_quote,
)
from terminal_admission import validate_terminal_admission
from terminal_profile import ACCOUNT_ROWS, EXPECTED_ACCOUNTS, build_terminal


PROFILE_DIR = Path(__file__).resolve().parent
REPO = PROFILE_DIR.parents[1]
EVIDENCE_PATH = PROFILE_DIR / "evidence.json"
SEALED_PROBE_PATHS = {
    "research/liveness-policy-profile/Cargo.toml",
    "research/liveness-policy-profile/Cargo.lock",
    "research/liveness-policy-profile/src/main.rs",
}
SAME_ELF_MEASUREMENTS = {
    "resolution_work",
    "resolution_work_batch",
    "native_point_v3",
    "native_bearer_redeem",
    "occupation_v4",
    "direct_v2",
    "source_endow",
    "blank_bank_market_creation",
    "order_reservation",
    "withdraw_cash",
    "general_epoch",
    "clear_walk",
    "candidate_selection",
    "entitled_clearing",
    "direct_v3",
    "direct_v3_close",
    "terminal_closure",
    "disagreement_exhibit",
    "revenue_boundary",
}
REQUIRED_EVIDENCE_SUFFIXES = {
    "audit/RUNTIME_ARTIFACT_AUDIT.md",
    "audit/backend-stack-diagnostics.txt",
    "audit/dependencies.tsv",
    "audit/elf-summary.txt",
    "audit/frame-summary.txt",
    "audit/metadata.json",
    "audit/registry-source-verification.tsv",
    "audit/source-files.txt",
    "audit/stack-summary.txt",
    "audit/upstream-SHA256SUMS",
    "audit/vendor.diff",
    "logs/sbf-build-1.log",
    "logs/sbf-build-2.log",
    "logs/sbf-build-relocated.log",
    "logs/bank/artifact_transport.log",
    "logs/bank/blank_bank_lifecycle.log",
    "logs/bank/collateral_leg.log",
    "logs/bank/direct_selection_v2.log",
    "logs/bank/native_resolution.log",
    "logs/bank/order_reservation.log",
    "logs/bank/prefund_creation.log",
    "logs/bank/resolution_work.log",
    "logs/bank/source_archive.log",
}
# Bank logs measured after a superseded seal was retained extend only the
# current artifact root: a historical seal keeps exactly the evidence set it
# was sealed with and is never asked for files that postdate it.  The three
# T2-6 logs seal the general-epoch lifecycle and streaming-walk CU evidence
# (the clear_walk measurement family draws on both walk logs); the two
# T2-7/T2-8 logs seal the selection and entitlement/settlement CU evidence;
# and the cross-path build log records the relocation probe the 2026-08-20
# build-path protocol amendment requires
# (docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md).  The three
# ``direct_selection_v3`` logs seal the V1-rung Direct V3 campaign: three
# independent bank runs of the same suite against this same ELF.  Three and
# not one because the V3 CU rows are *not* reproducible — the suite's fixture
# keypairs are freshly random per run and each PDA bump probe costs 1,500 CU,
# so a row moves in 1,500-CU steps between runs.  Sealing the spread is the
# honest form; every close, refund, conservation, rollback, and strand number
# in the logs is byte-identical across all three.
CURRENT_EVIDENCE_SUFFIXES = REQUIRED_EVIDENCE_SUFFIXES | {
    "logs/bank/resolution_work_batch.log",
    "logs/bank/general_epoch.log",
    "logs/bank/clear_walk.log",
    "logs/bank/clear_lifecycle.log",
    "logs/bank/candidate_selection.log",
    "logs/bank/entitled_clearing.log",
    "logs/bank/direct_selection_v3.log",
    "logs/bank/direct_selection_v3_run2.log",
    "logs/bank/direct_selection_v3_run3.log",
    "logs/bank/direct_selection_v3_run4.log",
    "logs/bank/terminal_closure.log",
    "logs/bank/disagreement_exhibit.log",
    "logs/bank/revenue_policy.log",
    "logs/sbf-build-crosspath.log",
    # The six scale campaigns.  These are the ``scale_clearing`` family's own
    # logs: every one of its 64 quoted (route, shape) rows is read out of them.
    "logs/bank/scale_max_book.log",
    "logs/bank/scale_multi_epoch.log",
    "logs/bank/scale_partial_fills.log",
    "logs/bank/scale_pot.log",
    "logs/bank/scale_tick_table.log",
    "logs/bank/scale_ties.log",
    # Retained per-run audit tables.
    "audit/account-probe-c55f471.txt",
    "audit/first-party-frame-audit.txt",
    # RECORDED, NOT QUOTED.  These suites ran green against this exact ELF in
    # the same locked pass as everything above, and their logs are sealed so
    # the run is reproducible from the tree — but no measurement family reads
    # them and NO CU ROW, QUOTE, OR REWARD IS DERIVED FROM ANY OF THEM.  A log
    # in the evidence set is a record that the suite ran, not a promotion.
    "logs/bank/SUMMARY.txt",
    "logs/bank/clear_work_creation.log",
    "logs/bank/cone_gate.log",
    "logs/bank/coupled_authority.log",
    "logs/bank/coupled_settlement.log",
    "logs/bank/degree_terms_admission.log",
    "logs/bank/joined_lifecycle.log",
    "logs/bank/native_full_lifecycle.log",
    "logs/bank/native_window_preflight.log",
    "logs/bank/pot_position_close.log",
    "logs/bank/r2_pull_endow.log",
    "logs/bank/r2_pull_identity.log",
    "logs/bank/r2_v2_wire.log",
    "logs/bank/source_ingest.log",
    "logs/bank/token_leg.log",
    "logs/bank/vpot_merge.log",
    "logs/bank/vpot_split.log",
}
# The Direct V3 account families classified after the sealed v2 probe.  The
# probe source archived at ``runtime_ref`` enumerates none of them, so these
# rows are pinned byte-exactly here and excluded from the probe equality;
# every other terminal row must still match the probe row-for-row, and a
# terminal row that is neither probed nor pinned refuses.  Byte pins come
# from programs/solana-layout/src/direct_selection_v3.rs
# (DIRECT_EPOCH_V4_BYTES, DIRECT_CANDIDATE_V3_BYTES, DIRECT_WINDOW_V3_BYTES,
# DIRECT_WORK_BUDGET_BYTES, DIRECT_RESERVATION_V2_BYTES,
# DIRECT_BATCH_POLICY_V3_BYTES, and the artifact stage header of
# ARTIFACT_STAGE_HEADER_BYTES = 136 over the 96-byte body).
POST_PROBE_DIRECT_V3_ROWS = {
    "direct.epoch.v4": 672,
    "direct.candidate.v3": 488,
    "direct.window.v3": 632,
    "direct.work_budget.v1": 248,
    "direct.reservation.v2": 618,
    "artifact.direct_batch_policy_v3.final": 96,
    "artifact.direct_batch_policy_v3.stage": 232,
}
# The two T2-8 general-plane families classified after the sealed probe: the
# entitlement freeze reuses the direct plane's exact byte shapes at new
# walk-plane PDAs (FinalPotAccount at seeds::pot_pda per general epoch,
# SettlementReceiptAccount at seeds::receipt_pda per (candidate, slice)), and
# the probe archived at ``runtime_ref`` enumerates only the direct-plane
# instances of those shapes.  Byte pins come from
# programs/solana-layout/src/lib.rs (account_len::FINAL_POT = 262,
# account_len::SETTLEMENT_RECEIPT = 217); the same probe-equality teeth as the
# V3 pins apply.
POST_PROBE_T2_8_ROWS = {
    "epoch.final_pot": 262,
    "epoch.receipt": 217,
}
# The TerminalClosure family's one new persistent account, classified after the
# sealed probe for the same reason: the probe source archived at ``runtime_ref``
# predates it.  The byte pin comes from
# programs/solana-layout/src/clearing.rs
# (GENERAL_FUNDING_LEDGER_BYTES = 2 + 32 + 32 + 8 + 8 + 1 + 1 + 1 = 85, account
# tag GENERAL_FUNDING_LEDGER_TAG = 26); the same probe-equality teeth apply.
POST_PROBE_TERMINAL_CLOSURE_ROWS = {
    "general.funding_ledger": 85,
}
# The revenue plane's one account family, classified after the sealed probe
# for the same reason (the probe source archived at ``runtime_ref`` predates
# it) and landed in the inventory BEFORE its implementation lane per the B4f
# rows-first rule.  Byte pin comes from
# programs/solana-layout/src/revenue.rs (REVENUE_POLICY_RECORD_BYTES =
# 2 + 32 + 32 + 32 + 56 + 1 + 1 = 156, tag REVENUE_POLICY_RECORD_TAG = 27);
# the same probe-equality teeth apply.  The design's RevenueVaultV1 row is
# deliberately absent: B4c builds no vault.
POST_PROBE_REVENUE_ROWS = {
    "revenue.policy_record.v1": 156,
}
# The inventory row the driven fee-bearing boundary creates, and the one
# residual it honestly stops on: CloseRevenuePolicyRecord (tag 68) exists and
# pays the exact recorded payer, but it is gated on the Realm account being
# gone, and the ``realm`` row is PERMANENT_INFRA with no close route at all.
REVENUE_RECORD_ROW = "revenue.policy_record.v1"
REVENUE_RECORD_BLOCKER = "REVENUE.REALM_PERMANENCE_HOLDS_RECORD"
# What TerminalClosure (tags 60-67) does and does not retire, welded to the
# sealed bank walk by ``require_terminal_closure_evidence``.  The close DAG is
# complete, permissionless except for the owner-signed release edge, and
# exactly conserving; what it does NOT do is make any general-plane row
# REFUNDABLE_TRANSIENT, because the funding ledger is optional at creation and
# an abandoned reservation holds the root open.  Both residuals are blocking
# ids, and the checker refuses either half drifting alone.
TERMINAL_CLOSURE_INTENTS = list(range(60, 68))
TERMINAL_CLOSURE_ROWS = {
    "epoch.window",
    "epoch.final_pot",
    "epoch.receipt",
    "general.funding_ledger",
    "legacy.epoch.v2",
    "legacy.candidate",
    "legacy.candidate_feed",
    "legacy.clear_work",
}
TERMINAL_CLOSURE_BLOCKERS = {
    "RENT.ACCOUNT_REFUND_UNOWNED",
    "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT",
}
# The declared-permanent residual a closed CLEARED general epoch leaves behind:
# exactly the sealed 64-byte batch-policy artifact, whose own row is the
# authority on the number.
TERMINAL_CLOSURE_RESIDUAL_ROW = "artifact.batch_policy.final"
# ---------------------------------------------------------------------------
# Walk plane, promotion rung W1 (D1 of
# docs/decisions/REPORT_clearing-plane-promotion_2026-08-20.md, adopted in
# docs/decisions/ADOPTED_2026-08-20.md item 10, unblocked by item 1's freeze of
# GENERAL_CLEARING_POLICY_V1 and CANDIDATE_WINDOW_SLOTS = 1,000).
#
# W1 is QUOTES WITHOUT LIVE FLAGS and nothing else.  ``derive`` computes the
# selected compute limit, external fee cap, and keeper reward for every general
# clearing route whose CU maximum this seal actually carries, by the same
# ``quote_route`` arithmetic every promoted family uses.  What W1 does NOT do,
# each welded below rather than merely written down:
#
#   * no live flag moves — the four families keep
#     ``UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`` and the projection keeps
#     ``live_flags: UNTOUCHED``.  A walk family that acquires a live flag
#     refuses, because W2's evidence does not exist;
#   * no keeper program consumes these quotes.  There is no runtime reward
#     schedule for the plane to cover (contrast
#     ``require_runtime_schedule_covers_policy``), so a quote here is a policy
#     row, not an operational promise;
#   * the rent side is NOT quoted.  TerminalClosure gave the plane real close
#     routes, and every general-plane row still STOPs on the two residuals of
#     the cycle-E reclassification; W1 publishes which rows those are and
#     prices none of them;
#   * tags 60-67 get no row at all.  The ``terminal_closure`` family declares
#     ``per_route_cu: NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED`` and the suite
#     prints no per-route CU label, so there is nothing to quote and nothing is
#     invented;
#   * W2 — live flags, lifecycle path quotes, operational keeper promises —
#     stays blocked on ``WALK_PLANE_W2_BLOCKING_IDS`` and the evidence gaps in
#     ``WALK_PLANE_W2_EVIDENCE_GAPS``.
#
# The shape is not new to this profile: V2's Select route is quoted PASS inside
# a family-level STOP, with the comment that a passing select quote does not
# promote the subsystem.  W1 is that shape, applied to twenty-five routes.
WALK_PLANE_W1_ADMISSION = "W1_QUOTED_NO_LIVE_FLAG"
WALK_PLANE_W1_STOPPED_ADMISSION = "W1_STOP_HEADROOM_NO_QUOTE"
WALK_PLANE_W1_FAMILIES = (
    "general_epoch",
    "clear_walk",
    "candidate_selection",
    "entitled_clearing",
    # The L2 disagreement exhibit joined the rung at the df0aece1… seal.  It is
    # not a new plane: it drives the SAME general-plane routes against the SAME
    # ELF under the SAME frozen policy, at a third book composition (13 orders,
    # 7 slices, five entitled single crossings plus one portfolio full pair),
    # and it prints its labels.  A quoted route measured hotter by a suite the
    # rung does not read would be an understated quote, which is exactly the
    # failure the "nothing measured goes unpublished" rule exists to prevent —
    # and a family outside this tuple escapes that rule entirely.  Its rows
    # quote only its own observations; the four original families' rows are
    # unchanged, because each row already bounds its own measured composition
    # and no other.
    "disagreement_exhibit",
    # The six scale campaigns, joining at the 0d52c561… seal for the same
    # reason the exhibit joined at df0aece1…: they drive the SAME general-plane
    # routes against the SAME ELF under the SAME frozen policy, at shapes the
    # sealed books never reached, and several of those observations are far
    # hotter than the quoted rows.  A family outside this tuple escapes the
    # "nothing measured goes unpublished" rule entirely, which is exactly the
    # loophole a 759,892-CU EntitleSlice would have slipped through while the
    # profile published 207,315 for the same instruction.
    "scale_clearing",
)
WALK_PLANE_FAMILIES = (*WALK_PLANE_W1_FAMILIES, "terminal_closure")
# How a route's measured maximum relates to the shapes the suite drove.  The
# vocabulary is closed and every row must use one of these three, so a
# genuinely variable route can never be published as if one number bounded it.
W1_FIXED_SHAPE = "FIXED_SHAPE"
W1_SHAPE_LABELLED = "SHAPE_LABELLED_BY_THE_ROUTE_KEY"
W1_BATCH_VARIABLE = "BATCH_SHAPE_VARIABLE_OBSERVED_MAXIMUM_ONLY"
WALK_PLANE_W1_VARIABILITY = {W1_FIXED_SHAPE, W1_SHAPE_LABELLED, W1_BATCH_VARIABLE}
# route key, measurement family, the evidence fields whose union the route
# maximum covers, variability class.  A source is either a flat CU-list field
# name or ``(row-table field, shape label)``.  ``AdvanceClearWork`` and
# ``AdvanceClearSlices`` are the genuinely variable routes: the driver chooses
# how many orders, reservations, or slices ride in one transaction, and the
# sealed suite drove eleven distinct pass-1 slot shapes on the forty-order
# book alone (1 to 16 records, 0 to 11 reservations).  Their rows quote the
# observed maximum and say so; they bound no unmeasured batch composition.
WALK_PLANE_W1_ROUTES: tuple[tuple[str, str, tuple[Any, ...], str], ...] = (
    ("init_epoch", "general_epoch", ("init_epoch_cu",), W1_FIXED_SHAPE),
    (
        "place_order_single",
        "general_epoch",
        ("place_order_single_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "place_order_portfolio",
        "general_epoch",
        ("place_order_portfolio_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "freeze_epoch_1page_4orders",
        "general_epoch",
        (("freeze_epoch_rows", (1, 4)),),
        W1_SHAPE_LABELLED,
    ),
    (
        "freeze_epoch_2pages_17orders",
        "general_epoch",
        (("freeze_epoch_rows", (2, 17)),),
        W1_SHAPE_LABELLED,
    ),
    (
        "freeze_epoch_3pages_40orders",
        "general_epoch",
        (("freeze_epoch_rows", (3, 40)),),
        W1_SHAPE_LABELLED,
    ),
    (
        "advance_clear_work_pass1_small_book",
        "clear_walk",
        ("small_book_pass1_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "advance_clear_work_pass2_small_book",
        "clear_walk",
        ("small_book_pass2_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "advance_clear_work_pass1_forty_order",
        "clear_walk",
        ("forty_order_pass1_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "advance_clear_work_pass2_forty_order",
        "clear_walk",
        ("forty_order_pass2_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "advance_clear_slices",
        "clear_walk",
        ("advance_slices_cu",),
        W1_BATCH_VARIABLE,
    ),
    ("complete_clear_work_walk", "clear_walk", ("complete_cu",), W1_FIXED_SHAPE),
    (
        "submit_candidate",
        "candidate_selection",
        ("submit_candidate_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "write_candidate_feed_fills",
        "candidate_selection",
        ("write_feed_fills_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "write_candidate_feed_slices",
        "candidate_selection",
        ("write_feed_slices_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "seal_candidate_including_displacing",
        "candidate_selection",
        ("seal_candidate_cu", "seal_candidate_displacing_cu"),
        W1_SHAPE_LABELLED,
    ),
    (
        "finalize_selection_3_retained_winner",
        "candidate_selection",
        (("finalize_selection_rows", "3_retained_2_verified_selects_winner"),),
        W1_SHAPE_LABELLED,
    ),
    (
        "finalize_selection_digest_tie",
        "candidate_selection",
        (("finalize_selection_rows", "2_verified_beyond_128_bit_digest_tie"),),
        W1_SHAPE_LABELLED,
    ),
    (
        "finalize_selection_honest_lapse",
        "candidate_selection",
        (("finalize_selection_rows", "0_verified_honest_lapse"),),
        W1_SHAPE_LABELLED,
    ),
    (
        "complete_clear_work_selection",
        "candidate_selection",
        ("complete_clear_work_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "freeze_entitlement",
        "entitled_clearing",
        ("freeze_entitlement_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "entitle_slice_single",
        "entitled_clearing",
        ("entitle_slice_single_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "entitle_slice_portfolio_pair",
        "entitled_clearing",
        ("entitle_slice_portfolio_pair_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "settle_page_entitled_direct_slice",
        "entitled_clearing",
        ("settle_page_entitled_direct_slice_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "settle_page_entitled_portfolio_full_pair",
        "entitled_clearing",
        ("settle_page_entitled_portfolio_full_pair_cu",),
        W1_SHAPE_LABELLED,
    ),
    # The partial-fill wave's routes, measured by the same suite on the same
    # one-page book.  Each is its own shape — an inexact book that funds the
    # rounding pot, a mixed portfolio/single book cleared leg by leg, a
    # fragmented buy, and the four non-converting strands — so each gets its own
    # quote rather than widening the five rows above.  They are all UNLEDGERED,
    # like the rest of this family: see the scale_clearing rows for the ledgered
    # shape a keeper can actually close.
    (
        "entitle_slice_inexact_pot_funding",
        "entitled_clearing",
        ("entitle_slice_inexact_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "settle_page_potted",
        "entitled_clearing",
        ("settle_page_potted_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "entitle_slice_mixed_leg",
        "entitled_clearing",
        ("entitle_slice_mixed_leg_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "settle_page_mixed_leg",
        "entitled_clearing",
        ("settle_page_mixed_leg_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "entitle_slice_fragmented_buy",
        "entitled_clearing",
        ("entitle_slice_fragmented_buy_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "settle_page_partial_slice",
        "entitled_clearing",
        ("settle_page_partial_slice_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "entitle_slice_strand",
        "entitled_clearing",
        ("entitle_slice_strand_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "settle_page_strand",
        "entitled_clearing",
        ("settle_page_strand_cu",),
        W1_BATCH_VARIABLE,
    ),
    # The disagreement exhibit's third book composition.  Each route is its own
    # quote over its own observations; the exhibit measures several of these
    # routes HOTTER than the two-suite books do, which is precisely why it is
    # quoted rather than filed as prose.
    (
        "init_clear_work_exhibit_book",
        "disagreement_exhibit",
        ("init_clear_work_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "advance_clear_work_pass1_exhibit_book",
        "disagreement_exhibit",
        ("exhibit_pass1_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "advance_clear_work_pass2_exhibit_book",
        "disagreement_exhibit",
        ("exhibit_pass2_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "advance_clear_slices_exhibit_book",
        "disagreement_exhibit",
        ("advance_slices_cu",),
        W1_BATCH_VARIABLE,
    ),
    (
        "complete_clear_work_exhibit_book",
        "disagreement_exhibit",
        ("complete_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "freeze_entitlement_exhibit_book",
        "disagreement_exhibit",
        ("freeze_entitlement_cu",),
        W1_FIXED_SHAPE,
    ),
    (
        "entitle_slice_single_exhibit_book",
        "disagreement_exhibit",
        ("entitle_slice_single_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "entitle_slice_portfolio_pair_exhibit_book",
        "disagreement_exhibit",
        ("entitle_slice_portfolio_pair_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "settle_page_entitled_direct_slice_exhibit_book",
        "disagreement_exhibit",
        ("settle_page_entitled_direct_slice_cu",),
        W1_SHAPE_LABELLED,
    ),
    (
        "settle_page_entitled_portfolio_full_pair_exhibit_book",
        "disagreement_exhibit",
        ("settle_page_entitled_portfolio_full_pair_cu",),
        W1_SHAPE_LABELLED,
    ),
)
# Measured CU fields that are deliberately NOT routes.  The walk suite prices
# the ComputeBudget ``request_heap_frame(262144)`` instruction itself at 150 CU
# (450 with it, 300 without); it rides inside the same transaction as an
# AdvanceClearWork route, so ``require_walk_plane_w1_quotes`` refuses any
# clear_walk row whose selected limit would not still cover the route plus the
# surcharge.  It is published as a declared surcharge rather than dropped.
WALK_PLANE_W1_SURCHARGE_FIELDS = {
    "clear_walk": "request_heap_frame_262144_surcharge_cu",
}
# Families whose quoted routes must still cover that measured rider.  The
# disagreement exhibit's own walk sender attaches the identical
# ``request_heap_frame(262144)`` instruction to EVERY transaction it measures
# but never re-prices it, so its routes are charged the ``clear_walk`` figure
# rather than a figure invented for them; the declaration below is welded so
# the borrowing cannot become silent.
WALK_PLANE_W1_SURCHARGE_BEARING_FAMILIES = ("clear_walk", "disagreement_exhibit")
WALK_PLANE_W1_BORROWED_SURCHARGE_DECLARATION = {
    "disagreement_exhibit": (
        "REQUEST_HEAP_FRAME_262144_ON_EVERY_WALK_TRANSACTION_"
        "SURCHARGE_FROM_CLEAR_WALK"
    ),
}
# Shape labels the row tables must carry exactly.  A new freeze shape or a new
# finalize shape appearing in the evidence refuses rather than going unquoted.
WALK_PLANE_W1_ROW_TABLES = {
    "freeze_epoch_rows": "general_epoch",
    "finalize_selection_rows": "candidate_selection",
}
# ---------------------------------------------------------------------------
# The scale campaigns, and the coordinate correction they forced.
#
# Six campaigns drove the general plane at shapes the two sealed books never
# reached — the maximum 64-order book across four dense pages, thirty partial
# fills across two, a twelve-completion rounding pot, three concurrent epochs,
# the complete 64-tick table, and a sixteen-deep tied candidate field.  They
# print 399 labelled CU rows.
#
# **What they found is that a page count is not a nuisance parameter.**  The
# sealed ``entitle_slice_single`` row is 207,315 CU and its suite's epoch is
# ONE page.  The same instruction on a two-page book measures 416,385 and on
# the four-page maximum 759,892 — 3.7x its own sealed row.  ``EntitleSlice`` is
# the page-set-wide route: it must be presented with the whole bound page set
# and re-derives the live orders by walking every page in it.  A flat quote for
# that route is not a slightly stale number, it is a quote for a different
# transaction, and it understates the real one by a factor.
#
# So the shape coordinate goes INTO THE ROUTE KEY.  The campaigns' rows collapse
# into (route, shape) groups; each group is quoted as its own W1 route named
# ``<route>_<coordinate>`` with variability SHAPE_LABELLED_BY_THE_ROUTE_KEY, and
# the group's maximum bounds every observation taken at that shape and no other.
# There is deliberately no combined ``entitle_slice`` row: the routes that
# differ by a page count are different routes, and the profile now says so in
# the key rather than in a footnote.
#
# The routes are DERIVED from the tables rather than hand-listed, which the four
# original families are not.  That is the point: their keys are prose and few,
# these are mechanical coordinates and many, and generating them means a shape
# the campaigns start driving becomes a published quote automatically instead of
# waiting for someone to notice.  A table not declared here still refuses.
#
# Every scale row is LEDGERED.  Each created account carries its optional
# ``GeneralFundingLedgerV1`` sibling, because an account created without one
# records no payer and no close route will ever guess it — so the ledgered shape
# is the only one a keeper can actually drive to a close.  The unledgered rows
# the four original families carry are kept and labelled as the non-closeable
# variant rather than dropped, because they are what the older suites measured.
SCALE_CLEARING_FAMILY = "scale_clearing"
# table -> the ordered coordinate names that form its shape key.
SCALE_CLEARING_ROW_TABLES: dict[str, tuple[str, ...]] = {
    "init_epoch_rows": ("ticks",),
    "init_order_page_rows": ("pages",),
    "place_order_single_rows": ("ticks",),
    "place_order_worst_rank_rows": ("rank",),
    "place_order_tick_probe_rows": ("ticks",),
    "freeze_epoch_rows": ("pages", "orders"),
    "submit_candidate_rows": ("pages",),
    "write_feed_fills_rows": ("chunk",),
    "write_feed_slices_rows": ("chunk",),
    "seal_candidate_rows": ("retained",),
    "seal_candidate_displacing_rows": ("retained",),
    "seal_candidate_refused_tied_rows": ("retained",),
    "init_clear_work_plus_4_grows_rows": ("pages",),
    "advance_pass1_rows": ("orders",),
    "advance_pass2_rows": ("pages",),
    "advance_slices_rows": ("batch",),
    "complete_clear_work_rows": ("pages",),
    "finalize_selection_rows": ("retained",),
    "finalize_selection_digest_tie_rows": ("retained",),
    "freeze_entitlement_rows": ("pages",),
    "freeze_entitlement_inexact_rows": ("pages",),
    "entitle_slice_single_rows": ("pages",),
    "entitle_slice_single_inexact_rows": ("pages",),
    "settle_page_direct_rows": ("pages",),
    "settle_page_potted_rows": ("pages",),
}
SCALE_CLEARING_LEDGER_DECLARATION = (
    "EVERY_CREATED_ACCOUNT_CARRIES_ITS_GENERAL_FUNDING_LEDGER_V1_SIBLING_"
    "THE_ONLY_SHAPE_A_KEEPER_CAN_CLOSE"
)
# What full admission (W2) is still blocked on.  The three ids are the ones the
# walk plane's own terminal rows carry; the gaps are section 3 of the promotion
# report.  W1 publishes the intersection that is still live and refuses to keep
# calling W2 blocked once every named id has retired.
WALK_PLANE_W2_BLOCKING_IDS = {
    "RENT.ACCOUNT_REFUND_UNOWNED",
    "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT",
    "PROFILE.STORAGE_INVENTORY_INCOMPLETE",
}
WALK_PLANE_W2_EVIDENCE_GAPS = (
    "WIDER_PAGE_ORDER_AND_CANDIDATE_GRIDS",
    "FULL_WIDTH_TIE_AND_DISPLACEMENT_CAMPAIGNS",
    "SECOND_INDEPENDENT_BANK_PROFILE",
    "RENT_AND_CLOSE_ROWS_UNDER_A_RATIFIED_R4_CARVE_OUT",
    "FREEZE_TO_SETTLE_PATH_QUOTE_MODEL",
)
# The PDA-attempt quantum, carried in the model instead of averaged into a
# shape it does not belong to.
#
# The program resolves addresses with ``find_program_address``, which counts a
# bump down from 255 and pays one ``create_program_address`` per failed
# attempt.  The scale campaigns measured that attempt at 1,500 CU: across two
# runs whose random genesis keys give every plane different canonical bumps,
# the placement rows agree only at that value, and the shape terms then
# reproduce exactly (+318 CU for fifty-three extra ticks of scan, +3,318 CU for
# the wider table at equal depth).  A route deriving *m* addresses therefore
# carries ``sum(255 - bump_i) * 1500`` CU of pure fixture noise.
#
# The consequence for a quote is stated rather than hidden: a route sealed from
# a SINGLE observation cannot separate its shape term from its bump term, so
# its measured CU is only known to within an integer number of these quanta.
# Every W1 row therefore publishes ``single_observation`` and, when true, the
# quantum that bounds the unresolved term.  A row with several observations
# does not get the caveat, because the spread itself shows the term — the
# exhibit's five ``EntitleSlice`` sends differ by exactly 3,000 and 4,500 CU,
# two and three quanta.
#
# This is a statement about the MEASUREMENT, not a widening of the quote: the
# selected limit still derives from the observed maximum by the ordinary 5/4
# rule.  It says what the maximum is known to, which is what a reader needs in
# order to decide whether one send was enough.
PDA_ATTEMPT_QUANTUM_CU = 1500
# ---------------------------------------------------------------------------
# The fold batch's real bound, and the plan it supersedes.
#
# Until this seal the fold-batch widths were chosen on COMPUTE alone: twelve is
# the largest batch that stays under the 1,120,000-CU raw admission bound, so
# twelve was the largest measured width and the sealed fewest-transaction plan
# for a 32-record work item was ``[12, 12, 8]`` — three transactions.
#
# Compute is not what binds a fold batch on the wire.  The keeper's
# ``fold-wire-probe`` measured the serialized message at every width and had a
# real validator's ``sendTransaction`` agree with the serializer: **six** Fold
# instructions frame at 1,216 bytes and seven do not, at 1,347 against the
# 1,232-byte legacy packet budget.  A twelve-fold message is 2,002 bytes.
#
# So ``[12, 12, 8]`` prices three transactions that cannot be submitted, and
# this seal supersedes it.  The measured rows for widths 8 and 12 are KEPT —
# they are real bank measurements of real transactions, and the compute figure
# is what it is — but they are excluded from the PLAN and labelled with the
# reason.  The plan is now composed only of widths that fit a packet, which is
# what ``cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`` was standing in
# for; that caveat is discharged by the probe and replaced by this bound.
FOLD_PACKET_BUDGET_BYTES = 1232
FOLD_MAX_SENDABLE_BATCH = 6
FOLD_SENDABLE_BOUND_EVIDENCE = (
    "KEEPER_FOLD_WIRE_PROBE_SERIALIZER_AND_VALIDATOR_TRANSPORT_AGREE_"
    "SIX_FOLDS_1216_BYTES_SEVEN_1347_BYTES_TWELVE_2002_BYTES"
)
FOLD_UNSENDABLE_ROW_DISPOSITION = (
    "MEASURED_ON_A_BANK_BUT_OVER_THE_1232_BYTE_PACKET_BUDGET_EXCLUDED_FROM_THE_PLAN"
)
# The Direct V3 close campaign (rung V1 of the clearing-plane promotion
# report).  These four families are the ones ``DIRECT.V3_CLOSE_EVIDENCE_
# UNSEALED`` blocked, and the id's own text says what retires it: a sealed
# bank measurement of the close and its rollback, "exactly as
# DIRECT.TOP3_SELECT_CU_STOP retired".  The measurement now exists, so the
# rows are REFUNDABLE_TRANSIENT — and the two halves are welded together
# below.  ``require_v3_close_evidence`` refuses a row classified refundable
# without a measured close route, refuses sealed close evidence for a row
# that quietly went back to STOP, refuses a route whose lamports did not
# conserve exactly, and refuses the retired blocker reappearing.  Neither
# half can drift without the derivation failing.
V3_REFUNDABLE_ROWS = {
    "direct.candidate.v3",
    "direct.window.v3",
    "direct.work_budget.v1",
    "direct.reservation.v2",
}
RESOLUTION_REFUNDABLE_ROWS = {"resolution.work.v1", "resolution.reserve.v1"}
V3_CLOSE_ROUTES = {
    "SubmitDirectCandidateV3 displacing",
    "FinalizeDirectSelectionV3",
    "SettleDirectV3",
    "LapseEmptyDirectV3",
    "LapseUnselectedDirectV3",
    "LapseSelectedDirectV3",
    "AbortUnfrozenDirectV4 empty",
    "AbortUnfrozenDirectV4 one",
    "AbortUnfrozenDirectV4 two",
}
V3_ROLLBACK_OBSERVATIONS = {
    "FreezeDirectEpochV4 underfunded",
    "SubmitDirectCandidateV3 underfunded",
    "FinalizeDirectSelectionV3 wrong close recipient",
    "AbortUnfrozenDirectV4 wrong close recipient",
}
EXACT_CONSERVATION = {"EXACT_ZERO_SUM", "EXACT_CLOSED_EQUALS_RECIPIENTS"}
# What the same campaign measured as unreclaimable, per Direct V3 epoch.  The
# promotion report's rent story names only the first two (7,127,040 lamports);
# the sealed run shows the V4 OrderPage is stranded by the same absence of a
# close handler, which is four times the other two together.  The rows stay
# UNCLASSIFIED_STOP and the number is published rather than rounded away.
V3_STRUCTURAL_STRAND_ROWS = {
    "direct.epoch.v4",
    "artifact.direct_batch_policy_v3.final",
    "order.page",
}
# The V4 OrderPage strand now carries its own blocking id rather than hiding
# inside the generic unowned-refund one.  ``require_v3_close_evidence`` requires
# the id to be present on the row it names, so the corrected 35,941,440-lamport
# strand figure and its blocker cannot drift apart.
V3_STRAND_BLOCKING_IDS = {
    "direct.epoch.v4": "DIRECT.EPOCH_RECEIPT_RENT_PERSISTS",
    "artifact.direct_batch_policy_v3.final": "DIRECT.POLICY_ARTIFACT_RENT_PERSISTS",
    "order.page": "DIRECT.ORDER_PAGE_RENT_PERSISTS",
}
MINIMUM_V3_BANK_RUNS = 3


TRACKING_UNAVAILABLE = "sealed-evidence git tracking UNAVAILABLE"


class CheckError(RuntimeError):
    """A pinned identity or arithmetic check failed."""


class TrackingUnavailable(CheckError):
    """Git could not answer whether the sealed evidence is in the repository.

    This is a refusal, never a pass: an unanswerable tracking question must not
    be reported as "tracked".  It subclasses :class:`CheckError` so every caller
    that already fails closed on a check error also fails closed here, while the
    distinct type lets a caller name the degraded case exactly.
    """


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(argv: list[str], *, cwd: Path = REPO, env: dict[str, str] | None = None) -> str:
    result = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if result.returncode != 0:
        raise CheckError(f"command failed ({result.returncode}): {' '.join(argv)}\n{result.stdout}")
    return result.stdout


def git_tracking(argv: list[str], *, repo: Path, stdin: str | None = None) -> str:
    """Run a git query whose failure must never be read as "tracked"."""

    try:
        result = subprocess.run(
            ["git", *argv],
            cwd=repo,
            input=stdin,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as error:
        raise TrackingUnavailable(
            f"{TRACKING_UNAVAILABLE}: cannot run 'git {argv[0]}' in {repo}: {error}"
        ) from error
    if result.returncode != 0:
        diagnostic = (result.stderr or result.stdout).strip().splitlines()
        raise TrackingUnavailable(
            f"{TRACKING_UNAVAILABLE}: 'git {' '.join(argv[:2])}' failed "
            f"({result.returncode}) in {repo}: "
            + (diagnostic[-1] if diagnostic else "no diagnostic output")
        )
    return result.stdout


def require_equal(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise CheckError(f"{label}: expected {expected!r}, got {actual!r}")


def load_evidence(path: Path = EVIDENCE_PATH) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as stream:
        evidence = json.load(stream)
    if evidence.get("schema") != "dragons-clutch/liveness-policy-evidence/v2":
        raise CheckError("unexpected evidence schema")
    return evidence


def quote_policy(evidence: dict[str, Any]) -> QuotePolicy:
    row = evidence["policy_inputs"]
    return QuotePolicy(
        headroom_numerator=row["cu_headroom_numerator"],
        headroom_denominator=row["cu_headroom_denominator"],
        rounding_quantum_cu=row["cu_rounding_quantum"],
        transaction_ceiling_cu=row["transaction_cu_ceiling"],
        base_fee_cap_lamports=row["base_transaction_fee_cap_lamports"],
        micro_lamports_per_cu_cap=row["micro_lamports_per_cu_cap"],
        keeper_tip_lamports=row["keeper_surplus_lamports"],
    )


def quote_dict(route: RouteQuote) -> dict[str, Any]:
    return {
        "measured_cu": route.measured_cu,
        "required_headroom_cu": route.required_headroom_cu,
        "selected_limit_cu": route.selected_limit_cu,
        "external_fee_cap_lamports": route.external_fee_cap_lamports,
        "keeper_reward_lamports": route.keeper_reward_lamports,
        "status": route.status,
    }


def route_dict(measured_cu: int, policy: QuotePolicy) -> dict[str, Any]:
    return quote_dict(quote_route(measured_cu, policy))


def require_v3_close_evidence(
    close: dict[str, Any], terminal: dict[str, Any]
) -> int:
    """Weld the Direct V3 refundable classification to its sealed measurement.

    Returns the per-epoch structural strand in lamports, taken from the
    terminal rent rows rather than from the measured balances so that a
    prefund donation can never flatter the published number.
    """

    if close.get("runs_agree_exactly") is not True:
        raise CheckError("Direct V3 close evidence does not claim exact run agreement")
    runs = close.get("bank_runs")
    if not isinstance(runs, int) or runs < MINIMUM_V3_BANK_RUNS:
        raise CheckError(
            f"Direct V3 close evidence needs at least {MINIMUM_V3_BANK_RUNS} bank "
            f"runs, declares {runs!r}"
        )
    routes = close["routes"]
    require_equal(set(routes), V3_CLOSE_ROUTES, "Direct V3 measured close routes")
    for name, row in routes.items():
        if row["conservation"] not in EXACT_CONSERVATION:
            raise CheckError(
                f"Direct V3 close route {name} is not exactly conserved: "
                f"{row['conservation']!r}"
            )
        if not row["recipient_deltas"]:
            raise CheckError(f"Direct V3 close route {name} names no recipient")
    require_equal(
        set(close["rollback_observations"]),
        V3_ROLLBACK_OBSERVATIONS,
        "Direct V3 measured close/rollback observations",
    )

    accounts = terminal["accounts"]
    require_equal(set(close["closed_rows"]), V3_REFUNDABLE_ROWS, "Direct V3 closed rows")
    for name, covering in close["closed_rows"].items():
        unknown = set(covering) - V3_CLOSE_ROUTES
        if unknown or not covering:
            raise CheckError(
                f"Direct V3 row {name} cites unmeasured close routes: "
                + (", ".join(sorted(unknown)) or "none at all")
            )
        row = accounts[name]
        if row["lifecycle_class"] != "REFUNDABLE_TRANSIENT":
            raise CheckError(
                f"{name} carries sealed close evidence but is classified "
                f"{row['lifecycle_class']!r}"
            )
        for field in ("close_bank_evidence", "rollback_bank_evidence"):
            require_equal(row[field], "PASS", f"{name} {field}")

    refundable = {
        name
        for name, row in accounts.items()
        if row["lifecycle_class"] == "REFUNDABLE_TRANSIENT"
    }
    require_equal(
        refundable,
        V3_REFUNDABLE_ROWS | RESOLUTION_REFUNDABLE_ROWS,
        "refundable terminal rows",
    )
    if "DIRECT.V3_CLOSE_EVIDENCE_UNSEALED" in terminal["blocking_ids"]:
        raise CheckError(
            "DIRECT.V3_CLOSE_EVIDENCE_UNSEALED is retired by this seal but is "
            "still in the terminal blocker set"
        )

    measured_strand = close["structural_strand_lamports"]
    require_equal(
        set(measured_strand), V3_STRUCTURAL_STRAND_ROWS, "Direct V3 stranded rows"
    )
    strand = 0
    for name, observed in measured_strand.items():
        row = accounts[name]
        if row["lifecycle_class"] != "UNCLASSIFIED_STOP":
            raise CheckError(f"{name} strands rent but is not an honest STOP")
        if observed < row["rent_lamports"]:
            raise CheckError(
                f"{name} stranded {observed} lamports, below its own rent floor "
                f"{row['rent_lamports']}"
            )
        owed = V3_STRAND_BLOCKING_IDS[name]
        if owed not in row["blocking_ids"]:
            raise CheckError(
                f"{name} strands {observed} lamports in the sealed campaign but "
                f"does not carry its own blocking id {owed}"
            )
        strand += row["rent_lamports"]
    return strand


def require_terminal_closure_evidence(
    closure: dict[str, Any], terminal: dict[str, Any]
) -> dict[str, Any]:
    """Weld the general-plane terminal classification to its sealed close walk.

    TerminalClosure (tags 60-67) is the first close path the general clearing
    plane has ever had, and the sealed walk drives it to the epoch root twice.
    That retires the reason those rows carried
    ``PROFILE.STORAGE_INVENTORY_INCOMPLETE`` — but it does not make them
    refundable, and this function refuses the over-claim in both directions:

    * a general-plane row that quietly became ``REFUNDABLE_TRANSIENT`` while the
      funding ledger is still optional at creation;
    * sealed evidence that stops declaring the ledger optional or the release
      edge owner-signed while the rows still STOP on exactly those two ids;
    * a close walk whose lamports did not conserve exactly;
    * a cleared-epoch residual that is not exactly the declared-permanent
      artifact's own rent row;
    * either residual blocking id vanishing from the global set.

    Returns the two walks' reclaim summary for the projection.
    """

    if closure.get("admission") != "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY":
        raise CheckError(
            "TerminalClosure evidence lost its unpromoted declaration: "
            f"{closure.get('admission')!r}"
        )
    require_equal(
        closure["intents"], TERMINAL_CLOSURE_INTENTS, "TerminalClosure intent tags"
    )
    # The two structural residuals must still be declared by the evidence, or
    # the rows below are STOPping on something the measurement no longer shows.
    for field in ("funding_ledger_optional_at_creation", "release_is_owner_signed"):
        if closure.get(field) is not True:
            raise CheckError(
                f"TerminalClosure evidence no longer declares {field}, but the "
                "general-plane rows still STOP on exactly that residual"
            )
    if closure.get("closes_are_permissionless") is not True:
        raise CheckError("TerminalClosure evidence must declare its closes permissionless")

    accounts = terminal["accounts"]
    walks = closure["walks"]
    require_equal(
        set(walks), {"cleared_epoch", "lapsed_epoch"}, "TerminalClosure sealed walks"
    )
    for name, walk in walks.items():
        if walk["conservation"] != "EXACT_INVENTORY_EQUALS_RECLAIMED_PLUS_BURNED":
            raise CheckError(f"TerminalClosure walk {name} is not exactly conserved")
        inventory = walk["machinery_inventory_lamports"]
        reclaimed = walk["reclaimed_lamports"]
        burned = walk["burned_at_frozen_sink_lamports"]
        if inventory != reclaimed + burned:
            raise CheckError(
                f"TerminalClosure walk {name}: {inventory} held != {reclaimed} "
                f"reclaimed + {burned} burned"
            )
        if reclaimed <= 0:
            raise CheckError(f"TerminalClosure walk {name} reclaimed nothing")
    # The cleared walk's residual is the declared-permanent artifact, taken
    # from the terminal row rather than from the measured balance so that a
    # prefund donation can never flatter it.
    require_equal(
        walks["cleared_epoch"]["residual_lamports"],
        accounts[TERMINAL_CLOSURE_RESIDUAL_ROW]["rent_lamports"],
        "TerminalClosure cleared-epoch residual",
    )
    # The lapsed walk proves the unledgered state is reachable, which is what
    # keeps RENT.ACCOUNT_REFUND_UNOWNED honest on every general-plane row.
    if walks["lapsed_epoch"]["unregistered_residual_lamports"] <= 0:
        raise CheckError(
            "the lapsed walk must exhibit the unledgered residual that keeps "
            "RENT.ACCOUNT_REFUND_UNOWNED on the general-plane rows"
        )

    blockers = set(terminal["blocking_ids"])
    missing = TERMINAL_CLOSURE_BLOCKERS - blockers
    if missing:
        raise CheckError(
            "TerminalClosure residual blocking ids are absent from the terminal "
            "set: " + ", ".join(sorted(missing))
        )
    for name in TERMINAL_CLOSURE_ROWS:
        row = accounts[name]
        if row["lifecycle_class"] != "UNCLASSIFIED_STOP":
            raise CheckError(
                f"{name} is classified {row['lifecycle_class']!r}, but the "
                "general plane's funding ledger is optional at creation and its "
                "release edge is owner-signed — neither "
                "rent_principal_recorded nor expiry_or_reaper holds "
                "unconditionally for this family"
            )
        if not TERMINAL_CLOSURE_BLOCKERS & set(row["blocking_ids"]):
            raise CheckError(
                f"{name} is a general-plane STOP that names neither residual of "
                "the sealed close walk"
            )
    return {
        "cleared_epoch_reclaimed_lamports": walks["cleared_epoch"]["reclaimed_lamports"],
        "cleared_epoch_inventory_lamports": walks["cleared_epoch"][
            "machinery_inventory_lamports"
        ],
        "cleared_epoch_burned_lamports": walks["cleared_epoch"][
            "burned_at_frozen_sink_lamports"
        ],
        "cleared_epoch_residual_lamports": walks["cleared_epoch"]["residual_lamports"],
        "lapsed_epoch_reclaimed_lamports": walks["lapsed_epoch"]["reclaimed_lamports"],
        "lapsed_epoch_unregistered_residual_lamports": walks["lapsed_epoch"][
            "unregistered_residual_lamports"
        ],
    }


def require_revenue_boundary_evidence(
    family: dict[str, Any], terminal: dict[str, Any]
) -> dict[str, Any]:
    """Weld the driven fee-bearing boundary to the row it created.

    The revenue suite is a refusal battery, not a measurement: it prints no CU
    label and no headline, so this seal derives **no** CU row, no quote, and no
    refusal code from it — the codes it asserts live in the suite source, and a
    number transcribed out of source is not evidence.  What the family may
    carry is exactly what its log supports, and the three claims that would
    change this profile's funding story if either half drifted:

    * both fee rates are zero and no fee-bearing epoch admits.  The profile
      never treats a fee as liveness funding; the day a fee-bearing epoch can
      open, that rule is a decision, not a derivation, and this refuses first;
    * the treasury is the distinguished unset sentinel, so the refusal is
      structural rather than a policy value someone could set;
    * the record row it creates is in the inventory as an honest STOP carrying
      its own residual id.  A row that quietly became refundable while its
      close stays gated on a PERMANENT_INFRA Realm would be an over-admission.
    """

    invented = sorted(
        name
        for name, value in family.items()
        if (name.endswith("_cu") or name.endswith("_rows"))
        and not (isinstance(value, str) and value.startswith("NOT_"))
    )
    if invented:
        raise CheckError(
            "the revenue suite prints no CU label, so no compute row may be "
            f"derived from it: {', '.join(invented)}"
        )
    require_equal(
        family.get("per_route_cu"),
        "NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED",
        "revenue per-route CU declaration",
    )
    require_equal(
        family.get("refusal_codes"),
        "NOT_PRINTED_BY_SUITE_ASSERTED_IN_SOURCE_ONLY",
        "revenue refusal-code declaration",
    )
    require_equal(
        family.get("rates"),
        "BOTH_ZERO_NO_FEE_BEARING_EPOCH_ADMITS",
        "revenue rate declaration",
    )
    require_equal(
        family.get("treasury"),
        "REVENUE_TREASURY_UNSET_V1_SENTINEL_REFUSES_STRUCTURALLY",
        "revenue treasury declaration",
    )
    executed = family.get("executed_tests")
    if not isinstance(executed, list) or not executed:
        raise CheckError("the revenue boundary family names no executed test")
    if not str(family.get("test_result", "")).startswith("PASS_"):
        raise CheckError(
            f"revenue boundary evidence is not a pass: {family.get('test_result')!r}"
        )

    row = terminal["accounts"][REVENUE_RECORD_ROW]
    if row["lifecycle_class"] != "UNCLASSIFIED_STOP":
        raise CheckError(
            f"{REVENUE_RECORD_ROW} is classified {row['lifecycle_class']!r}, but "
            "its close is gated on the absence of a PERMANENT_INFRA Realm, so "
            "its principal is capitalized for the Realm's whole life"
        )
    if REVENUE_RECORD_BLOCKER not in set(row["blocking_ids"]):
        raise CheckError(
            f"{REVENUE_RECORD_ROW} is a STOP that does not name its own "
            f"residual {REVENUE_RECORD_BLOCKER}"
        )
    return {
        "record_row": REVENUE_RECORD_ROW,
        "record_bytes": row["bytes"],
        "record_rent_lamports": row["rent_lamports"],
        "residual_blocking_id": REVENUE_RECORD_BLOCKER,
        "executed_tests": list(executed),
        "cu_rows_derived": False,
        "quote_rows_derived": False,
    }


def walk_plane_row_label(field: str, row: dict[str, Any], family: str = "") -> Any:
    """Return the shape label of one measured row-table entry."""

    if family == SCALE_CLEARING_FAMILY:
        coordinates = SCALE_CLEARING_ROW_TABLES.get(field)
        if coordinates is None:
            raise CheckError(f"undeclared scale-clearing row table: {field}")
        missing = [name for name in coordinates if name not in row]
        if missing:
            raise CheckError(
                f"scale-clearing row in {field} is missing its shape "
                f"coordinate(s) {', '.join(missing)}; a row whose shape is not "
                "stated cannot be quoted shape by shape"
            )
        return tuple(row[name] for name in coordinates)
    if field == "freeze_epoch_rows":
        return (row["pages"], row["orders"])
    if field == "finalize_selection_rows":
        return row["shape"]
    raise CheckError(f"unknown walk-plane row table: {field}")


def scale_clearing_route_key(field: str, label: tuple[Any, ...]) -> str:
    """The route key one scale (table, shape) group publishes.

    The coordinate is IN the key.  ``entitle_slice_single_4pages`` and
    ``entitle_slice_single_1page`` are different routes with different quotes,
    which is the whole correction: they were one row at 207,315 CU while the
    four-page send actually costs 759,892.
    """

    # Namespaced, because a coordinate key can otherwise collide with a
    # hand-listed one measured on a different book: ``general_epoch`` already
    # quotes ``freeze_epoch_1page_4orders`` from its own suite, and the
    # campaigns measure that same shape on a 64-tick grid with three epochs
    # live.  Two measurements of one shape on two books are two routes.
    stem = "scale_" + field[: -len("_rows")]
    coordinates = SCALE_CLEARING_ROW_TABLES[field]
    parts = []
    for name, value in zip(coordinates, label):
        if name == "pages":
            parts.append(f"{value}page" + ("s" if value != 1 else ""))
        elif name == "orders":
            parts.append(f"{value}orders")
        elif name == "ticks":
            parts.append(f"{value}ticks")
        elif name == "retained":
            parts.append(f"{value}retained")
        elif name == "chunk":
            parts.append(f"x{value}")
        elif name == "batch":
            parts.append(f"batch{value}")
        elif name == "rank":
            parts.append(f"rank{value}")
        else:
            raise CheckError(f"unknown scale shape coordinate: {name}")
    return "_".join([stem, *parts])


def scale_clearing_routes(
    measurements: dict[str, Any]
) -> tuple[tuple[str, str, tuple[Any, ...], str], ...]:
    """Derive one W1 route per measured (table, shape) group.

    Generated rather than hand-listed, so a shape the campaigns start driving
    becomes a published quote instead of waiting to be noticed.  An undeclared
    table, a duplicated shape, or a row with no shape coordinate refuses.
    """

    family = measurements[SCALE_CLEARING_FAMILY]
    tables = sorted(
        name
        for name in family
        if name.endswith("_rows") or name.endswith("_cu")
    )
    undeclared = [name for name in tables if name not in SCALE_CLEARING_ROW_TABLES]
    if undeclared:
        raise CheckError(
            "scale-clearing measures "
            + ", ".join(undeclared)
            + ", which no declared row table covers; a measured shape may not "
            "reach the projection without a coordinate"
        )
    routes: list[tuple[str, str, tuple[Any, ...], str]] = []
    for field in tables:
        labels = [
            walk_plane_row_label(field, entry, SCALE_CLEARING_FAMILY)
            for entry in family[field]
        ]
        if len(set(labels)) != len(labels):
            raise CheckError(f"scale-clearing table {field} labels a shape twice")
        for label in labels:
            routes.append(
                (
                    scale_clearing_route_key(field, label),
                    SCALE_CLEARING_FAMILY,
                    ((field, label),),
                    W1_SHAPE_LABELLED,
                )
            )
    return tuple(routes)


def walk_plane_route_observations(
    measurements: dict[str, Any], family: str, sources: tuple[Any, ...]
) -> list[int]:
    """Collect every sealed CU observation one W1 route quote must cover."""

    row = measurements[family]
    observations: list[int] = []
    for source in sources:
        if isinstance(source, str):
            values = row[source]
        else:
            field, label = source
            table = row[field]
            matches = [
                entry["cu"]
                for entry in table
                if walk_plane_row_label(field, entry, family) == label
            ]
            if len(matches) != 1:
                raise CheckError(
                    f"walk-plane row table {family}.{field} carries "
                    f"{len(matches)} rows labelled {label!r}, expected exactly one"
                )
            values = matches[0]
        if not isinstance(values, list) or not values:
            raise CheckError(
                f"walk-plane W1 source {family}.{source!r} carries no observation"
            )
        for value in values:
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                raise CheckError(
                    f"walk-plane W1 source {family}.{source!r} carries a "
                    f"non-CU observation: {value!r}"
                )
        observations.extend(values)
    return observations


def require_walk_plane_w1_quotes(
    measurements: dict[str, Any],
    terminal: dict[str, Any],
    policy: QuotePolicy,
) -> dict[str, Any]:
    """Derive rung W1: CU/quote/reward rows for the walk plane, no live flags.

    Every refusal here is one half of a weld that cannot drift alone:

    * a quoted family that stops declaring itself unpromoted, or that acquires
      a live flag, refuses — W2's evidence does not exist, and this rung is
      explicitly the one that does not move a flag;
    * a measured CU field in a quoted family that no W1 route consumes refuses,
      so a newly measured route cannot be silently left unquoted while the
      block still says it prices the plane;
    * a route whose selected limit would exceed the transaction ceiling is
      published as a STOP with **no lamport quote** and drops the whole block
      to STOP_HEADROOM — impossible envelopes are never clamped into prices;
    * a ``clear_walk`` row whose limit would not still cover the route plus the
      measured heap-frame surcharge refuses;
    * a general-plane terminal row that stopped being an honest STOP refuses,
      because W1 declares the rent side unquoted and those rows STOPped;
    * the ``terminal_closure`` family labelling per-route CU refuses, because
      W1 declares tags 60-67 unquotable for exactly that reason;
    * every W2 blocking id retiring refuses, because the block would then be
      publishing a "W2 is blocked" declaration that is no longer true.

    The row values themselves are re-derived from the sealed maxima on every
    run, so a CU maximum that drifts re-derives its limit and reward and the
    projection equality in :func:`check` catches a stale published row.
    """

    for family in WALK_PLANE_FAMILIES:
        declared = measurements[family].get("admission")
        if declared != "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY":
            raise CheckError(
                f"walk measurement family {family} lost its unpromoted "
                f"declaration: {declared!r}"
            )

    # 1. Live flags.  W1 is the rung that quotes and moves nothing.
    outstanding = sorted(WALK_PLANE_W2_BLOCKING_IDS & set(terminal["blocking_ids"]))
    for family in WALK_PLANE_FAMILIES:
        claimed = sorted(
            name
            for name, value in measurements[family].items()
            if name.startswith("live") and value is not False
        )
        if claimed:
            raise CheckError(
                f"walk family {family} claims live flag(s) {', '.join(claimed)}, "
                "but this is rung W1 (quotes, no live flags); a live flag needs "
                "rung W2, whose evidence does not exist — still outstanding: "
                + (", ".join(outstanding) or "no blocking id, but the W2 gaps "
                   + ", ".join(WALK_PLANE_W2_EVIDENCE_GAPS) + " remain")
            )

    # 2. Tags 60-67 are unquotable, and the reason must still be declared.
    closure = measurements["terminal_closure"]
    require_equal(
        closure.get("per_route_cu"),
        "NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED",
        "TerminalClosure per-route CU declaration",
    )

    # 2b. The scale campaigns' ledger declaration.  Their rows are the ledgered
    #     shapes, and that is the whole reason they are the quotable ones.
    require_equal(
        measurements[SCALE_CLEARING_FAMILY].get("funding_ledger"),
        SCALE_CLEARING_LEDGER_DECLARATION,
        "scale-clearing funding-ledger declaration",
    )

    # 3. Every measured CU field of a quoted family is either a W1 route source
    #    or a declared non-route surcharge.  Nothing measured goes unpublished.
    #
    #    Row tables are keyed by (family, field): ``freeze_epoch_rows`` exists in
    #    both ``general_epoch`` and ``scale_clearing`` at different shapes, and
    #    collapsing them under a bare field name would let one family's coverage
    #    vouch for the other's.
    all_routes = (*WALK_PLANE_W1_ROUTES, *scale_clearing_routes(measurements))
    row_tables: dict[tuple[str, str], str] = {
        (family, field): field for field, family in WALK_PLANE_W1_ROW_TABLES.items()
    }
    for field in SCALE_CLEARING_ROW_TABLES:
        if field in measurements[SCALE_CLEARING_FAMILY]:
            row_tables[(SCALE_CLEARING_FAMILY, field)] = field
    consumed: dict[str, set[str]] = {family: set() for family in WALK_PLANE_W1_FAMILIES}
    labels: dict[tuple[str, str], set[Any]] = {key: set() for key in row_tables}
    for _, family, sources, variability in all_routes:
        if family not in consumed:
            raise CheckError(f"W1 route names a family it may not quote: {family}")
        if variability not in WALK_PLANE_W1_VARIABILITY:
            raise CheckError(f"W1 route declares an unknown variability: {variability!r}")
        for source in sources:
            if isinstance(source, str):
                consumed[family].add(source)
                continue
            field, label = source
            consumed[family].add(field)
            if (family, field) not in labels:
                raise CheckError(f"W1 route reads an undeclared row table: {family}.{field}")
            labels[(family, field)].add(label)
    # A row table is quoted shape by shape, so a shape the suite starts
    # measuring must be quoted too rather than disappearing under a field name
    # that is already covered.
    for (family, field) in row_tables:
        if field not in consumed[family]:
            raise CheckError(f"W1 quotes no row of the {family}.{field} table")
        measured_labels = [
            walk_plane_row_label(field, entry, family)
            for entry in measurements[family][field]
        ]
        if len(set(measured_labels)) != len(measured_labels):
            raise CheckError(f"{family}.{field} labels a shape twice")
        require_equal(
            set(measured_labels),
            labels[(family, field)],
            f"W1 shape coverage of {family}.{field}",
        )
    for family in WALK_PLANE_W1_FAMILIES:
        measured = {
            name
            for name in measurements[family]
            if name.endswith("_cu") or name.endswith("_rows")
        }
        surcharge = WALK_PLANE_W1_SURCHARGE_FIELDS.get(family)
        expected = consumed[family] | ({surcharge} if surcharge else set())
        require_equal(measured, expected, f"W1 route coverage of family {family}")

    # 4. The rows themselves, re-derived from the sealed maxima every run.
    #
    #    Each row also says what its measured maximum is KNOWN TO.  A route
    #    sealed from one send cannot separate its shape term from the PDA-bump
    #    term the fixture's random keys give it, so it publishes the quantum
    #    that bounds the unresolved part instead of presenting one observation
    #    as an exact figure.  A route with several sends does not carry the
    #    caveat: the spread is the evidence.
    routes: dict[str, Any] = {}
    for key, family, sources, variability in all_routes:
        observations = walk_plane_route_observations(measurements, family, sources)
        quote = quote_route(max(observations), policy)
        single = len(observations) == 1
        routes[key] = {
            "family": family,
            "observations": len(observations),
            "shape_variability": variability,
            "admission": (
                WALK_PLANE_W1_ADMISSION
                if quote.admitted
                else WALK_PLANE_W1_STOPPED_ADMISSION
            ),
            "single_observation": single,
            "measured_cu_known_to_within": (
                f"PLUS_OR_MINUS_K_TIMES_{PDA_ATTEMPT_QUANTUM_CU}_CU_PDA_ATTEMPT_QUANTUM"
                if single
                else "SPREAD_OVER_MULTIPLE_SENDS_SEALED"
            ),
            **quote_dict(quote),
        }
    if len(routes) != len(all_routes):
        raise CheckError("W1 route keys are not unique")

    # 5. The heap-frame request is a measured 150-CU rider on the same
    #    transaction as an AdvanceClearWork route; a limit that would not cover
    #    route + surcharge may not be published as that route's limit.
    surcharge_field = WALK_PLANE_W1_SURCHARGE_FIELDS["clear_walk"]
    surcharge = max(measurements["clear_walk"][surcharge_field])
    for family, declaration in WALK_PLANE_W1_BORROWED_SURCHARGE_DECLARATION.items():
        require_equal(
            measurements[family].get("heap_frame_rider"),
            declaration,
            f"{family} borrowed heap-frame rider declaration",
        )
    for key, row in routes.items():
        if (
            row["family"] not in WALK_PLANE_W1_SURCHARGE_BEARING_FAMILIES
            or row["selected_limit_cu"] is None
        ):
            continue
        with_rider = quote_route(row["measured_cu"] + surcharge, policy)
        if (
            not with_rider.admitted
            or with_rider.selected_limit_cu > row["selected_limit_cu"]
        ):
            raise CheckError(
                f"W1 row {key} selects {row['selected_limit_cu']} CU, which does "
                f"not cover its own measured {surcharge}-CU heap-frame request"
            )

    # 6. The rent side is declared unquoted, and the rows it names must still
    #    be the honest STOPs the cycle-E reclassification left them as.
    accounts = terminal["accounts"]
    for name in sorted(TERMINAL_CLOSURE_ROWS):
        if accounts[name]["lifecycle_class"] != "UNCLASSIFIED_STOP":
            raise CheckError(
                f"W1 publishes {name} as an unquoted general-plane STOP, but it "
                f"is classified {accounts[name]['lifecycle_class']!r}"
            )

    # 7. W2 must still be blocked for the block's own declaration to be true.
    if not outstanding:
        raise CheckError(
            "rung W1 declares W2 blocked, but every id in "
            + ", ".join(sorted(WALK_PLANE_W2_BLOCKING_IDS))
            + " has retired; the rung must be re-decided, not silently upgraded"
        )

    stopped = sorted(key for key, row in routes.items() if row["status"] != "PASS")
    worst = max(routes.items(), key=lambda item: item[1]["measured_cu"])
    return {
        "rung": "W1",
        "decision": "ADOPTED_2026-08-20 item 10 (REPORT_clearing-plane-promotion D1)",
        "enabled_by": "GENERAL_CLEARING_POLICY_V1_AND_CANDIDATE_WINDOW_SLOTS_FROZEN",
        "row_admission": WALK_PLANE_W1_ADMISSION,
        "status": "PASS" if not stopped else "STOP_HEADROOM",
        "quoted_families": list(WALK_PLANE_W1_FAMILIES),
        "quoted_route_count": len(routes),
        "pda_attempt_quantum_cu": PDA_ATTEMPT_QUANTUM_CU,
        "single_observation_routes": sorted(
            key for key, row in routes.items() if row["single_observation"]
        ),
        "scale_shape_coordinates": {
            field: list(coordinates)
            for field, coordinates in sorted(SCALE_CLEARING_ROW_TABLES.items())
            if field in measurements[SCALE_CLEARING_FAMILY]
        },
        "scale_funding_ledger": SCALE_CLEARING_LEDGER_DECLARATION,
        "stopped_routes": stopped,
        "worst_route": worst[0],
        "worst_measured_cu": worst[1]["measured_cu"],
        "routes": routes,
        "live_flags": "UNTOUCHED",
        "keeper_program_consumes_quotes": False,
        "runtime_reward_schedule": "NONE_NO_KEEPER_PROGRAM_READS_THESE_QUOTES",
        "path_quote": "NOT_DESIGNED_NO_BOUNDED_TRANSACTION_PLAN",
        "rent_side": "NOT_QUOTED_GENERAL_PLANE_ROWS_KEEP_THEIR_STOPS",
        "unquoted_rent_rows": sorted(TERMINAL_CLOSURE_ROWS),
        "excluded_families": ["terminal_closure"],
        "excluded_intents": TERMINAL_CLOSURE_INTENTS,
        "exclusion_reason": closure["per_route_cu"],
        "heap_frame_request_surcharge_cu": surcharge,
        "surcharge_absorbed_by_selected_limits": True,
        "w2_status": "BLOCKED",
        "w2_blocking_ids": outstanding,
        "w2_evidence_gaps": list(WALK_PLANE_W2_EVIDENCE_GAPS),
    }


def derive(evidence: dict[str, Any]) -> dict[str, Any]:
    """Derive the only promoted subsystem quote and protocol STOP."""

    policy = quote_policy(evidence)
    measurements = evidence["measurements"]
    work = measurements["resolution_work"]
    work_batch = measurements["resolution_work_batch"]
    exact_unique_labels(work["fold_widths"], [1, 2, 3, 4], "ResolutionWork Fold")
    exact_unique_labels(
        work_batch["batch_sizes"], [2, 4, 6, 8, 12], "ResolutionWork FoldBatch"
    )
    exact_unique_labels(
        [row["degree"] for row in measurements["occupation_v4"]["degree_rows"]],
        [1, 2, 3],
        "occupation-v4 degree",
    )
    exact_unique_labels(
        [row["degree"] for row in measurements["native_point_v3"]["degree_rows"]],
        [1, 2, 3],
        "native point-v3 degree",
    )
    exact_unique_labels(
        [row["degree"] for row in measurements["native_bearer_redeem"]["degree_rows"]],
        [1, 2, 3],
        "native bearer degree",
    )

    # The batch campaign also measured Begin and singleton Fold(1) rows at
    # spans past four; each route quote covers every observation of its route.
    begin = quote_route(max([*work["begin_cu"], *work_batch["begin_cu"]]), policy)
    folds = {
        1: quote_route(
            max([*work["fold_1_cu"], *work_batch["singleton_fold_cu"]]), policy
        ),
        **{
            width: quote_route(max(work[f"fold_{width}_cu"]), policy)
            for width in range(2, 5)
        },
    }
    finalize = quote_route(max(work["finalize_cu"]), policy)
    abort = quote_route(max(work["abort_cu"]), policy)
    path = resolution_path_quote(
        record_count=evidence["resolution_work"]["maximum_records"],
        begin=begin,
        fold_quotes=folds,
        finalize=finalize,
        abort=abort,
        rent_principal_lamports=(
            evidence["accounts"]["resolution.work.v1"]["rent_lamports"]
            + evidence["accounts"]["resolution.reserve.v1"]["rent_lamports"]
        ),
    )

    runtime_schedule = evidence["resolution_work"]["runtime_reward_schedule"]
    require_runtime_schedule_covers_policy(
        fold_quotes=folds,
        fold_base_reward=runtime_schedule["fold_base_lamports"],
        fold_per_record_reward=runtime_schedule["fold_per_record_lamports"],
        finalize_quote=finalize,
        finalize_reward=runtime_schedule["finalize_lamports"],
        abort_quote=abort,
        abort_reward=runtime_schedule["abort_lamports"],
    )

    # A batch of one is the measured singleton Fold(1) transaction itself.
    batch_quotes = {
        size: quote_route(max(work_batch[f"fold_batch_{size}_cu"]), policy)
        for size in work_batch["batch_sizes"]
    }
    # Every measured width keeps its row.  Only the widths that fit a packet
    # may compose the plan: a plan is a claim that a keeper can send these
    # transactions, and at widths 8 and 12 that claim is false.
    unsendable = sorted(
        size for size in batch_quotes if size > FOLD_MAX_SENDABLE_BATCH
    )
    plan_quotes = {
        1: folds[1],
        **{
            size: quote
            for size, quote in batch_quotes.items()
            if size <= FOLD_MAX_SENDABLE_BATCH
        },
    }
    batched = batched_resolution_path_quote(
        record_count=evidence["resolution_work"]["maximum_records"],
        begin=begin,
        batch_quotes=plan_quotes,
        finalize=finalize,
        abort=abort,
        rent_principal_lamports=(
            evidence["accounts"]["resolution.work.v1"]["rent_lamports"]
            + evidence["accounts"]["resolution.reserve.v1"]["rent_lamports"]
        ),
    )
    require_runtime_schedule_covers_batches(
        batch_quotes=plan_quotes,
        fold_base_reward=runtime_schedule["fold_base_lamports"],
        fold_per_record_reward=runtime_schedule["fold_per_record_lamports"],
    )

    direct = measurements["direct_v2"]
    native_point_routes = [
        {"degree": row["degree"], "resolve": route_dict(row["resolve_cu"], policy)}
        for row in measurements["native_point_v3"]["degree_rows"]
    ]
    occupation_routes = [
        {
            "degree": row["degree"],
            "initial": route_dict(row["initial_cu"], policy),
            "retry": route_dict(row["retry_cu"], policy),
        }
        for row in measurements["occupation_v4"]["degree_rows"]
    ]
    occupation_status = (
        "PASS"
        if all(
            row["initial"]["status"] == "PASS" and row["retry"]["status"] == "PASS"
            for row in occupation_routes
        )
        else "STOP_HEADROOM"
    )
    # The V2 select route is quoted from its measurement like every other
    # route: every artifact since 187d5ee1… completes it, where the af6bb79c…
    # artifact exhausted the transaction ceiling (a cost of the software
    # SHA-256, not of the selection).  A passing select quote does not promote
    # the subsystem: V2 stays stopped while its empty-frozen lapse is
    # unimplemented.
    select_route = route_dict(max(direct["select_cu"]), policy)
    direct_v2_status = (
        "PASS"
        if select_route["status"] == "PASS" and direct["empty_frozen_lapse"] == "PASS"
        else "STOP"
    )
    # T2-6/T2-7/T2-8: the general epoch lifecycle, streaming walk, candidate
    # selection, and entitlement/settlement are SBF-executed bank evidence
    # sealed with this artifact.  At rung W1 (adopted 2026-08-20 item 10) the
    # four measured families are QUOTED — selected compute limit and keeper
    # reward per route, by the same arithmetic every promoted family uses — and
    # nothing else moves: the families keep their unpromoted declaration, the
    # family status stays a STOP, no live flag moves, no keeper program reads
    # the quotes, the rent side is unquoted, and tags 60-67 get no row.  Full
    # admission (W2) stays ember's decision and stays blocked.  The derivation
    # refuses a family that stops saying it is unpromoted.
    for family in WALK_PLANE_FAMILIES:
        declared = measurements[family].get("admission")
        if declared != "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY":
            raise CheckError(
                f"walk measurement family {family} lost its unpromoted "
                f"declaration: {declared!r}"
            )
    # Direct V3 (tags 36-46), rung V1: the syscall-era CU rows and the
    # close/rollback campaign are SBF-executed bank evidence sealed with this
    # same artifact and deliberately unpromoted.  No V3 admission row, quote,
    # or reward is derived and ``live_v3`` stays false; what the close family
    # does move is the terminal classification of the four families whose only
    # blocker was the absence of exactly this measurement.  The derivation
    # refuses a V3 family that stops saying it is unpromoted.
    for family in ("direct_v3", "direct_v3_close"):
        declared = measurements[family].get("admission")
        if declared != "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY":
            raise CheckError(
                f"Direct V3 measurement family {family} lost its unpromoted "
                f"declaration: {declared!r}"
            )
    terminal = build_terminal(evidence["runtime_ref"])
    terminal_status = validate_terminal_admission(
        terminal,
        expected_accounts=EXPECTED_ACCOUNTS,
    )
    v3_strand_lamports = require_v3_close_evidence(
        measurements["direct_v3_close"], terminal
    )
    closure_summary = require_terminal_closure_evidence(
        measurements["terminal_closure"], terminal
    )
    walk_w1 = require_walk_plane_w1_quotes(measurements, terminal, policy)
    revenue_summary = require_revenue_boundary_evidence(
        measurements["revenue_boundary"], terminal
    )

    return {
        "status": "MEASURED_RUNTIME_ECONOMIC_ADMISSION_STOP",
        "complete_liveness_policy": "NOT_EMITTED_STOP",
        "maximum_raw_cu_with_requested_headroom": (
            policy.transaction_ceiling_cu
            * policy.headroom_denominator
            // policy.headroom_numerator
        ),
        "resolution_work": {
            "status": path.status,
            "routes": {
                "begin": quote_dict(begin),
                **{f"fold_{width}": quote_dict(folds[width]) for width in range(1, 5)},
                "finalize": quote_dict(finalize),
                "abort": quote_dict(abort),
            },
            "maximum_records": path.record_count,
            "fold_path_lamports": path.fold_path_lamports,
            "success_rewards_lamports": path.success_rewards_lamports,
            "worst_abort_rewards_lamports": path.worst_abort_rewards_lamports,
            "spendable_reserve_lamports": path.spendable_reserve_lamports,
            "rent_principal_lamports": path.rent_principal_lamports,
            "persistent_reserve_lamports": path.persistent_reserve_lamports,
            "begin_external_lamports": path.begin_external_lamports,
            "payer_cold_outlay_lamports": path.payer_cold_outlay_lamports,
            "runtime_schedule_matches_policy": True,
        },
        "resolution_work_batched": {
            "status": batched.status,
            "routes": {
                f"fold_batch_{size}": quote_dict(batch_quotes[size])
                for size in work_batch["batch_sizes"]
            },
            "maximum_admitted_batch": max(
                (
                    size
                    for size in work_batch["batch_sizes"]
                    if batch_quotes[size].admitted
                ),
                default=None,
            ),
            "outcome_equality": work_batch["outcome_equality"],
            "mid_batch_invalid_fold": work_batch["mid_batch_invalid_fold"],
            # Measured, not modelled, and no longer a caveat.  The keeper's
            # wire probe discharged UNMODELED_BANK_TRANSPORT_ONLY by measuring
            # the serialized message at every width and confirming it against a
            # real validator's transport.
            "cluster_packet_budget_bytes": FOLD_PACKET_BUDGET_BYTES,
            "maximum_sendable_batch": FOLD_MAX_SENDABLE_BATCH,
            "sendable_bound_evidence": FOLD_SENDABLE_BOUND_EVIDENCE,
            "measured_but_unsendable_batches": unsendable,
            "unsendable_row_disposition": FOLD_UNSENDABLE_ROW_DISPOSITION,
            "plan_batches": sorted(plan_quotes),
            "superseded_plan": [12, 12, 8],
            "superseded_plan_reason": (
                "CHOSEN_ON_COMPUTE_ALONE_AND_UNSENDABLE_A_TWELVE_FOLD_MESSAGE_IS_2002_BYTES"
            ),
            "maximum_records": batched.record_count,
            "fewest_transaction_plan": (
                list(batched.batch_plan) if batched.batch_plan is not None else None
            ),
            "fold_transactions": batched.fold_transactions,
            "fold_path_lamports": batched.fold_path_lamports,
            "success_rewards_lamports": batched.success_rewards_lamports,
            "worst_abort_rewards_lamports": batched.worst_abort_rewards_lamports,
            "spendable_reserve_lamports": batched.spendable_reserve_lamports,
            "rent_principal_lamports": batched.rent_principal_lamports,
            "persistent_reserve_lamports": batched.persistent_reserve_lamports,
            "begin_external_lamports": batched.begin_external_lamports,
            "payer_cold_outlay_lamports": batched.payer_cold_outlay_lamports,
        },
        "source_value_admission": {
            "status": "FAIL_CLOSED_STOP",
            "default_release_available": False,
            "endow_refusal_code": measurements["source_endow"]["default_refusal_code"],
            "refusal_cu_not_priced_as_success": True,
        },
        "direct_v2": {
            "status": direct_v2_status,
            "select": select_route,
            "select_result": direct["select_result"],
            "empty_frozen_lapse": direct["empty_frozen_lapse"],
            "live_v3": False,
        },
        "native_point_v3": {
            "status": "PASS" if all(row["resolve"]["status"] == "PASS" for row in native_point_routes) else "STOP",
            "degree_routes": native_point_routes,
        },
        "occupation_v4_monolithic": {
            "status": occupation_status,
            "degree_routes": occupation_routes,
            "live_action": (
                "MONOLITHIC_INITIAL_AND_RETRY_ADMITTED"
                if occupation_status == "PASS"
                else "USE_MEASURED_RESOLUTIONWORK_OR_FAIL_CLOSED"
            ),
        },
        "direct_selection_v3": {
            "status": "SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP",
            "admission_rows_derived": False,
            "live_flags": "UNTOUCHED",
            "measured_families": ["direct_v3", "direct_v3_close"],
            "bank_runs": measurements["direct_v3_close"]["bank_runs"],
            "cu_rows_reproducible": False,
            "close_routes_measured": sorted(V3_CLOSE_ROUTES),
            "close_rollback_observations": sorted(V3_ROLLBACK_OBSERVATIONS),
            "refundable_rows_sealed": sorted(V3_REFUNDABLE_ROWS),
            "retired_blocking_id": "DIRECT.V3_CLOSE_EVIDENCE_UNSEALED",
            "structural_strand_rows": sorted(V3_STRUCTURAL_STRAND_ROWS),
            "structural_strand_rent_lamports_per_epoch": v3_strand_lamports,
            "decision_owner": "ember",
        },
        "general_clearing_walk": {
            "status": "SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP",
            "promotion_rung": "W1",
            "admission_declaration": "ADMISSION_ROWS_NO_LIVE_FLAGS",
            "admission_rows_derived": True,
            "live_flags": "UNTOUCHED",
            "measured_families": list(WALK_PLANE_FAMILIES),
            "w1": walk_w1,
            "decision_owner": "ember",
        },
        "general_terminal_closure": {
            "status": "SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP",
            "admission_rows_derived": False,
            "live_flags": "UNTOUCHED",
            "intents": TERMINAL_CLOSURE_INTENTS,
            "close_dag_complete": True,
            "closes_are_permissionless": True,
            "release_edge_is_owner_signed": True,
            "funding_ledger_optional_at_creation": True,
            "per_route_cu_rows_derived": False,
            **closure_summary,
            "rows_reclassified_refundable": [],
            "residual_blocking_ids": sorted(TERMINAL_CLOSURE_BLOCKERS),
            "retired_reason": "PROFILE.STORAGE_INVENTORY_INCOMPLETE_NO_CLOSE_PATH",
            "decision_owner": "ember",
        },
        "revenue_boundary": {
            "status": "SBF_EXECUTED_REFUSAL_BOUNDARY_UNPROMOTED_STOP",
            "admission_rows_derived": False,
            "live_flags": "UNTOUCHED",
            "fee_rates": "BOTH_ZERO_UNDECIDED",
            "fee_bearing_epoch_admits": False,
            "treasury": "DEFERRED_UNSET_SENTINEL",
            "vault_built": False,
            "fees_as_liveness_funding": "NEVER_NOT_AT_ANY_RATE",
            **revenue_summary,
            "decision_owner": "ember",
        },
        "terminal_status": terminal_status,
        "terminal_blocking_ids": terminal["blocking_ids"],
    }


def parse_probe(output: str) -> tuple[dict[str, dict[str, int]], dict[str, int]]:
    accounts: dict[str, dict[str, int]] = {}
    metadata: dict[str, int] = {}
    lines = output.splitlines()
    if not lines or lines[0] != "schema\tdragons-clutch/liveness-account-inventory/v2":
        raise CheckError("historical probe schema mismatch")
    for line in lines[1:]:
        fields = line.split("\t")
        if len(fields) == 2 and fields[1].isdigit():
            metadata[fields[0]] = int(fields[1])
        elif len(fields) == 3 and fields[1].isdigit() and fields[2].isdigit():
            accounts[fields[0]] = {
                "bytes": int(fields[1]),
                "rent_lamports": int(fields[2]),
            }
        else:
            raise CheckError(f"malformed historical probe row: {line!r}")
    return accounts, metadata


def historical_probe(evidence: dict[str, Any]) -> str:
    """Execute only the manifest/lock/source archived at ``runtime_ref``."""

    with tempfile.TemporaryDirectory(prefix="clutch-liveness-profile-") as temp_name:
        temp = Path(temp_name)
        archive = subprocess.Popen(
            ["git", "archive", "--format=tar", evidence["runtime_ref"]],
            cwd=REPO,
            stdout=subprocess.PIPE,
        )
        assert archive.stdout is not None
        extracted = subprocess.run(["tar", "-xf", "-"], cwd=temp, stdin=archive.stdout, check=False)
        archive.stdout.close()
        if archive.wait() != 0 or extracted.returncode != 0:
            raise CheckError("could not materialize historical evidence tree")
        if not all((temp / path).is_file() for path in SEALED_PROBE_PATHS):
            raise CheckError("historical tree does not contain the sealed probe")
        manifest = temp / "research/liveness-policy-profile/Cargo.toml"
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(PROFILE_DIR / "target" / evidence["runtime_ref"])
        return run(
            ["cargo", "run", "--offline", "--locked", "--quiet", "--manifest-path", str(manifest)],
            cwd=temp,
            env=env,
        )


def check_source_identity(evidence: dict[str, Any]) -> None:
    missing = SEALED_PROBE_PATHS - set(evidence["source_blobs"])
    if missing:
        raise CheckError("unsealed historical probe inputs: " + ", ".join(sorted(missing)))
    for path, expected in evidence["source_blobs"].items():
        actual = run(["git", "rev-parse", f"{evidence['runtime_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"source blob {path}")
    for path, expected in evidence["source_trees"].items():
        actual = run(["git", "rev-parse", f"{evidence['runtime_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"source tree {path}")
    for path, expected in evidence["test_blobs"].items():
        actual = run(["git", "rev-parse", f"{evidence['evidence_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"test blob {path}")
    for path, expected in evidence["test_trees"].items():
        actual = run(["git", "rev-parse", f"{evidence['evidence_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"test tree {path}")


def check_files(evidence: dict[str, Any]) -> None:
    artifact = evidence["artifact"]
    artifact_path = REPO / artifact["path"]
    require_equal(artifact_path.stat().st_size, artifact["bytes"], "ELF bytes")
    require_equal(sha256(artifact_path), artifact["sha256"], "ELF sha256")
    for relative, expected in evidence["evidence_files"].items():
        path = REPO / relative
        if not path.is_file():
            raise CheckError(f"evidence file is absent: {relative}")
        require_equal(path.stat().st_size, expected["bytes"], f"{relative} bytes")
        require_equal(sha256(path), expected["sha256"], f"{relative} sha256")


def check_artifact_binding(evidence: dict[str, Any]) -> None:
    """Refuse historical/mixed-ELF rows in the current measurement plane.

    A reseal supersedes the previous artifact; it never retracts it.  Every
    superseded seal therefore keeps its own artifact root and its own complete
    evidence set on disk, and the current seal may not be written over it.
    """

    artifact = evidence["artifact"]
    digest = artifact["sha256"]
    require_equal(artifact["source_ref"], evidence["runtime_ref"], "artifact source ref")
    for build in ("normal_build_1", "normal_build_2"):
        require_equal(
            evidence["artifact_reproducibility"][build],
            digest,
            f"{build} artifact",
        )
    for name in SAME_ELF_MEASUREMENTS:
        require_equal(
            evidence["measurements"][name]["artifact_sha256"],
            digest,
            f"measurement artifact {name}",
        )

    # Cross-path builds are recorded as an observed-digest LIST under the
    # 2026-08-20 build-path protocol amendment
    # (docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md), never as an equality
    # claim.  The e8ba31d5... seal recorded a single cross-path build that came
    # back byte-identical and read it as a property; the V3 campaign then
    # observed two further digests at two further paths, and this seal observes
    # another.  The checker therefore refuses the shape that made the wrong
    # reading possible: a scalar field, or a list that silently contains the
    # canonical digest as if a coincidence were a guarantee.
    reproducibility = evidence["artifact_reproducibility"]
    if "cross_path_build" in reproducibility:
        raise CheckError(
            "cross_path_build is a scalar equality claim; the protocol requires "
            "the observed-digest list cross_path_builds"
        )
    observed = reproducibility["cross_path_builds"]
    if not isinstance(observed, list) or not observed:
        raise CheckError("cross_path_builds must be a non-empty observed-digest list")
    if len({row["sha256"] for row in observed}) != len(observed):
        raise CheckError("cross_path_builds lists a digest twice")
    if len({row["path"] for row in observed}) != len(observed):
        raise CheckError("cross_path_builds lists a path twice")
    require_equal(
        reproducibility["cross_path_disposition"],
        "PATH_TIED_SYMBOL_ORDER",
        "cross-path disposition",
    )
    for row in observed:
        require_equal(set(row), {"path", "sha256", "bytes"}, "cross-path observation")
        if row["sha256"] == digest:
            raise CheckError(
                "a cross-path build that happens to equal the canonical digest "
                "is a coincidence, not a reproducibility claim; record it in the "
                "audit prose rather than as evidence of path independence"
            )
    # The relocated-Cargo-home probe measures, rather than hides, the
    # registry-path boundary.  Its disposition must agree with its own digest:
    # a probe that did not reproduce the canonical bytes may never be written
    # up as INDEPENDENT, and one that did may never be written up as sensitive.
    relocated_independent = reproducibility["relocated_cargo_home"] == digest
    disposition = reproducibility["relocated_disposition"]
    if relocated_independent != disposition.startswith("INDEPENDENT"):
        raise CheckError(
            f"relocated-Cargo-home disposition {disposition!r} disagrees with its "
            f"own digest {reproducibility['relocated_cargo_home'][:16]}"
        )
    if not relocated_independent and not disposition.startswith("PATH_SENSITIVE"):
        raise CheckError(
            f"a relocated-home probe that diverged must say PATH_SENSITIVE, not "
            f"{disposition!r}"
        )
    # A diverged probe says WHICH relocation the divergence tracks, and it may
    # only say so with controls.  The protocol probe
    # (programs/clutch-sbf/audit/audit_artifact.sh) builds its relocated home
    # under $TMPDIR, which on this host is reached through the `/var` ->
    # `/private/var` symlink; two independent controls at ordinary paths
    # reproduced the canonical bytes exactly.  The attribution is therefore
    # narrower than "the Cargo home moved", and the narrower claim has to carry
    # the observations that earned it: at least one control that reproduced the
    # canonical bytes, none of them at the protocol probe's own path, and every
    # control's disposition agreeing with its own digest.  Without a
    # reproducing control the attribution field is a story, and refuses.
    if not relocated_independent:
        controls = reproducibility.get("relocated_controls")
        if not isinstance(controls, list) or not controls:
            raise CheckError(
                "a PATH_SENSITIVE relocated-home probe must carry the control "
                "builds that locate its cause, not a bare disposition"
            )
        reproduced = 0
        for row in controls:
            require_equal(
                set(row),
                {"path", "sha256", "bytes", "disposition"},
                "relocated control observation",
            )
            matches = row["sha256"] == digest
            expected = (
                "REPRODUCED_CANONICAL_BYTES" if matches else "DIVERGED_FROM_CANONICAL"
            )
            require_equal(
                row["disposition"], expected, f"relocated control {row['path']}"
            )
            reproduced += matches
        if len({row["path"] for row in controls}) != len(controls):
            raise CheckError("relocated_controls lists a path twice")
        if not reproduced:
            raise CheckError(
                "every relocated-home control diverged, so the divergence is not "
                "attributable to the protocol probe's own path; the attribution "
                "must be widened rather than kept"
            )
        attribution = reproducibility.get("relocated_attribution")
        if not isinstance(attribution, str) or not attribution:
            raise CheckError(
                "a PATH_SENSITIVE probe with a reproducing control must name "
                "what the divergence actually tracks"
            )

    artifact_root = Path(artifact["path"]).parent
    expected_files = {str(artifact_root / suffix) for suffix in CURRENT_EVIDENCE_SUFFIXES}
    require_equal(set(evidence["evidence_files"]), expected_files, "current evidence set")
    for relative in evidence["evidence_files"]:
        try:
            Path(relative).relative_to(artifact_root)
        except ValueError as error:
            raise CheckError(f"current evidence escapes artifact root: {relative}") from error

    for old_digest, row in evidence["historical_artifacts"].items():
        if old_digest == digest:
            raise CheckError("current artifact cannot also be historical")
        if row["disposition"] != "HISTORICAL_ONLY_NOT_USED_IN_CURRENT_PROJECTION":
            raise CheckError(f"historical artifact disposition is not fail-closed: {old_digest}")
        historical_root = Path(row["path"]).parent
        if historical_root == artifact_root:
            raise CheckError(f"historical seal shares the current artifact root: {old_digest}")
        for suffix in sorted(REQUIRED_EVIDENCE_SUFFIXES):
            retained = REPO / historical_root / suffix
            if not retained.is_file():
                raise CheckError(f"superseded seal evidence was removed: {historical_root / suffix}")
        old_path = REPO / row["path"]
        if not old_path.is_file():
            raise CheckError(f"superseded seal artifact was removed: {row['path']}")
        require_equal(sha256(old_path), old_digest, f"historical artifact {old_digest}")


def sealed_disk_paths(evidence: dict[str, Any]) -> list[str]:
    """Every repository path whose bytes this checker reads from the disk.

    ``check_files``, ``check_capture``, and ``check_artifact_binding`` all hash
    working-tree files, so each of them is satisfied by bytes that merely happen
    to sit in a working directory.  This is exactly the set that has to exist in
    the repository as well, current seal and retained superseded seals alike.
    """

    paths = {evidence["artifact"]["path"], evidence["capture"]["path"]}
    paths.update(evidence["evidence_files"])
    for row in evidence["historical_artifacts"].values():
        paths.add(row["path"])
        historical_root = Path(row["path"]).parent
        paths.update(str(historical_root / suffix) for suffix in REQUIRED_EVIDENCE_SUFFIXES)
    return sorted(paths)


def check_tracked_evidence(evidence: dict[str, Any], *, repo: Path = REPO) -> None:
    """Refuse sealed evidence that is on this disk but not in this repository.

    ``.gitignore`` excludes ``*.so`` and ``*.log``, so a plain ``git add`` of a
    new artifact root silently commits only a fraction of it while every
    disk-reading check above keeps passing.  A seal like that is green for
    whoever built it and unverifiable for whoever clones it.

    Each sealed path must therefore be tracked *and* resolve to a committed blob
    whose hash equals the hash of the file on disk, which refuses an
    ignored-but-present file, a staged-but-never-committed file, and a committed
    file whose working-tree bytes were changed afterwards.  When git cannot
    answer the question at all this raises :class:`TrackingUnavailable` naming
    the exact reason; it never degrades into a pass.
    """

    paths = sealed_disk_paths(evidence)
    for relative in paths:
        if not relative or relative.startswith("/") or "\n" in relative:
            raise CheckError(f"sealed evidence path is not a plain repository path: {relative!r}")

    toplevel = git_tracking(["rev-parse", "--show-toplevel"], repo=repo).strip()
    if Path(toplevel).resolve() != Path(repo).resolve():
        raise TrackingUnavailable(
            f"{TRACKING_UNAVAILABLE}: {repo} is not its own git work tree "
            f"(git reports {toplevel})"
        )
    head = git_tracking(["rev-parse", "--verify", "HEAD^{commit}"], repo=repo).strip()

    absent = [relative for relative in paths if not (Path(repo) / relative).is_file()]
    if absent:
        raise CheckError("sealed evidence file is absent: " + ", ".join(absent))

    listed = set(git_tracking(["ls-files", "-z", "--", *paths], repo=repo).split("\0"))
    untracked = [relative for relative in paths if relative not in listed]
    if untracked:
        raise CheckError(
            "sealed evidence is on disk but untracked by git (ignored or never added): "
            + ", ".join(untracked)
        )

    committed = git_tracking(
        ["cat-file", "--batch-check"],
        repo=repo,
        stdin="".join(f"{head}:{relative}\n" for relative in paths),
    ).splitlines()
    on_disk = git_tracking(
        ["hash-object", "--stdin-paths"],
        repo=repo,
        stdin="".join(f"{relative}\n" for relative in paths),
    ).splitlines()
    if len(committed) != len(paths) or len(on_disk) != len(paths):
        raise TrackingUnavailable(
            f"{TRACKING_UNAVAILABLE}: git answered for {len(committed)}/{len(on_disk)} of "
            f"{len(paths)} sealed paths"
        )

    uncommitted: list[str] = []
    divergent: list[str] = []
    for relative, row, disk_blob in zip(paths, committed, on_disk):
        if row.endswith(" missing") or row.endswith(" ambiguous"):
            uncommitted.append(relative)
            continue
        fields = row.split()
        if len(fields) != 3 or fields[1] != "blob":
            raise TrackingUnavailable(
                f"{TRACKING_UNAVAILABLE}: unreadable git object row for {relative}: {row!r}"
            )
        if fields[0] != disk_blob:
            divergent.append(f"{relative} (committed {fields[0][:12]}, on disk {disk_blob[:12]})")
    if uncommitted:
        raise CheckError(
            f"sealed evidence is not committed at HEAD {head[:12]}: " + ", ".join(uncommitted)
        )
    if divergent:
        raise CheckError(
            f"sealed evidence differs from its committed blob at HEAD {head[:12]}: "
            + ", ".join(divergent)
        )


def check_capture(evidence: dict[str, Any]) -> None:
    path = REPO / evidence["capture"]["path"]
    require_equal(sha256(path), evidence["capture"]["sha256"], "capture sha256")
    with path.open("r", encoding="utf-8") as stream:
        capture = json.load(stream)
    require_equal(capture["schema"], "dragons-clutch/liveness-bank-capture/v2", "capture schema")
    require_equal(capture["runtime_ref"], evidence["runtime_ref"], "capture runtime ref")
    require_equal(capture["evidence_ref"], evidence["evidence_ref"], "capture evidence ref")
    require_equal(capture["artifact"], evidence["artifact"], "capture artifact")
    require_equal(capture["measurements"], evidence["measurements"], "capture measurements")


def check_rent_and_accounts(evidence: dict[str, Any]) -> None:
    rent = evidence["rent"]
    effective = (
        rent["lamports_per_byte_year"] * rent["exemption_threshold_numerator"]
    ) // rent["exemption_threshold_denominator"]
    require_equal(effective, rent["effective_lamports_per_byte"], "effective rent rate")
    for name, row in evidence["accounts"].items():
        expected = max(1, (row["bytes"] + rent["account_storage_overhead_bytes"]) * effective)
        require_equal(row["rent_lamports"], expected, f"rent row {name}")
    terminal_rows = {
        name: {"bytes": bytes_, "rent_lamports": lamports}
        for name, bytes_, lamports, *_ in ACCOUNT_ROWS
    }
    pin_sets = (
        ("V3", POST_PROBE_DIRECT_V3_ROWS),
        ("T2-8", POST_PROBE_T2_8_ROWS),
        ("TerminalClosure", POST_PROBE_TERMINAL_CLOSURE_ROWS),
        ("Revenue", POST_PROBE_REVENUE_ROWS),
    )
    seen: set[str] = set()
    for _, pins in pin_sets:
        if not seen.isdisjoint(pins):
            raise CheckError("post-probe pin dictionaries overlap")
        seen |= set(pins)
    for label, pins in pin_sets:
        for name, pinned_bytes in pins.items():
            row = terminal_rows.pop(name, None)
            if row is None:
                raise CheckError(
                    f"post-probe {label} row missing from terminal inventory: {name}"
                )
            require_equal(row["bytes"], pinned_bytes, f"post-probe {label} bytes {name}")
            require_equal(
                row["rent_lamports"],
                max(1, (pinned_bytes + rent["account_storage_overhead_bytes"]) * effective),
                f"post-probe {label} rent {name}",
            )
    require_equal(evidence["accounts"], terminal_rows, "terminal/probe account inventory")
    output = historical_probe(evidence)
    accounts, metadata = parse_probe(output)
    maximum_stage = accounts.pop("artifact.maximum.stage", None)
    require_equal(
        maximum_stage,
        evidence["accounts"]["artifact.terms.stage"],
        "probe maximum stage alias",
    )
    require_equal(accounts, evidence["accounts"], "historical account probe")
    require_equal(metadata["lamports_per_byte"], effective, "probe rent rate")
    require_equal(
        metadata["account_storage_overhead"],
        rent["account_storage_overhead_bytes"],
        "probe storage overhead",
    )


def check(evidence: dict[str, Any]) -> None:
    check_source_identity(evidence)
    check_files(evidence)
    check_artifact_binding(evidence)
    check_capture(evidence)
    check_tracked_evidence(evidence)
    check_rent_and_accounts(evidence)
    require_equal(derive(evidence), evidence["projection"], "policy projection")


def check_current(evidence: dict[str, Any]) -> None:
    """Optional strict gate: runtime source closures must match the frozen ref."""

    drift: list[str] = []
    for path in set(evidence["source_trees"]) | set(evidence["source_blobs"]):
        tracked = subprocess.run(
            ["git", "diff", "--quiet", evidence["runtime_ref"], "--", path],
            cwd=REPO,
            check=False,
        )
        untracked = run(["git", "ls-files", "--others", "--exclude-standard", "--", path]).strip()
        if tracked.returncode != 0 or untracked:
            drift.append(path)
    if drift:
        raise CheckError("working runtime source drift: " + ", ".join(sorted(drift)))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-current", action="store_true")
    args = parser.parse_args()
    try:
        evidence = load_evidence()
        check(evidence)
        if args.check_current:
            check_current(evidence)
    except (CheckError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print("PASS: exact R1 artifact, bank capture, account probe, rewards, and STOPs agree")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
