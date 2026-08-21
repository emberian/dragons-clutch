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
    "logs/bank/terminal_closure.log",
    "logs/sbf-build-crosspath.log",
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


def derive(evidence: dict[str, Any]) -> dict[str, Any]:
    """Derive the only promoted subsystem quote and protocol STOP."""

    policy = quote_policy(evidence)
    measurements = evidence["measurements"]
    work = measurements["resolution_work"]
    work_batch = measurements["resolution_work_batch"]
    exact_unique_labels(work["fold_widths"], [1, 2, 3, 4], "ResolutionWork Fold")
    exact_unique_labels(
        work_batch["batch_sizes"], [2, 4, 8, 12], "ResolutionWork FoldBatch"
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
    plan_quotes = {1: folds[1], **batch_quotes}
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
    # sealed with this artifact and deliberately unpromoted — no admission,
    # quote, or reward row is derived for any general-clearing route
    # (tags 49-59) and no live flag moves.  Admission-policy treatment of
    # the plane is ember's decision, not this seal's.  The derivation refuses
    # a family that stops saying so out loud.
    for family in (
        "general_epoch",
        "clear_walk",
        "candidate_selection",
        "entitled_clearing",
        "terminal_closure",
    ):
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
            "cluster_packet_budget": "UNMODELED_BANK_TRANSPORT_ONLY",
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
            "admission_rows_derived": False,
            "live_flags": "UNTOUCHED",
            "measured_families": [
                "general_epoch",
                "clear_walk",
                "candidate_selection",
                "entitled_clearing",
                "terminal_closure",
            ],
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
