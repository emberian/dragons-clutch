#!/usr/bin/env python3
"""Deterministic model-based multiwallet lifecycle scenarios.

This module owns control-flow expectations (nonce consumption, replay refusal,
winner selection, payout progress, and retirement ordering).  ``ledger.py``
remains the only owner of economic arithmetic and conservation invariants.
Nothing in this module speaks to an RPC endpoint or handles a signer.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any, Mapping, Sequence

import ledger as economic


CONTRACT_SCHEMA = "dclutch-model-based-multiwallet-seed-contract-v1"
OUTPUT_SCHEMA = "dclutch-model-based-multiwallet-ensemble-v1"
OBSERVED_SCHEMA = "dclutch-model-based-multiwallet-observed-v1"
SEED_DOMAIN = b"dclutch/private-validator-lifecycle/named-seed/v1\0"
ROOT = Path(__file__).resolve().parents[2]
EXPECTED_ROWS = (
    ("seed-01", "baseline-crossed-owner", "below-first"),
    ("seed-02", "seller-future-nonce", "at-first"),
    ("seed-03", "buyer-future-nonce", "below-second"),
    ("seed-04", "duplicate-ticket", "at-second"),
    ("seed-05", "simultaneous-seller", "above-second"),
    ("seed-06", "simultaneous-buyer", "failure"),
    ("seed-07", "seller-account-switch", "below-first"),
    ("seed-08", "buyer-account-switch", "at-first"),
    ("seed-09", "foreign-account-refusal", "below-second"),
    ("seed-10", "winner-below-first", "below-first"),
    ("seed-11", "winner-at-first", "at-first"),
    ("seed-12", "winner-below-second", "below-second"),
    ("seed-13", "winner-at-second", "at-second"),
    ("seed-14", "winner-above-second", "above-second"),
    ("seed-15", "winner-provider-failure", "failure"),
    ("seed-16", "fee-floor-below", "below-first"),
    ("seed-17", "fee-floor-at", "at-first"),
    ("seed-18", "direct-replay", "below-second"),
    ("seed-19", "partial-payout-resume", "at-second"),
    ("seed-20", "retirement-before-zero", "failure"),
)


class ActionRefusal(RuntimeError):
    def __init__(self, code: str) -> None:
        super().__init__(code)
        self.code = code


def digest(value: Any) -> str:
    return hashlib.sha256(economic.canonical_bytes(value)).hexdigest()


def load_json(path: Path) -> Any:
    def object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise economic.Refusal(f"JSON field {key!r} repeats")
            result[key] = value
        return result

    try:
        return json.loads(path.read_text(), object_pairs_hook=object_pairs)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise economic.Refusal(f"cannot read canonical JSON {path}: {error}") from error


def canonical_source_fixtures(value: Any) -> list[dict[str, str]]:
    if not isinstance(value, list) or len(value) != 2:
        raise economic.Refusal("source fixture partition changed")
    expected = (
        ("private", "tools/economic-lifecycle-ledger/fixtures/private-canonical.json"),
        ("activity-v3", "tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json"),
    )
    result: list[dict[str, str]] = []
    for raw, (consumer, relative) in zip(value, expected, strict=True):
        row = economic.exact_keys(raw, {"consumer", "path", "sha256"}, "source fixture")
        if row["consumer"] != consumer or row["path"] != relative:
            raise economic.Refusal("source fixture identity or canonical order changed")
        sha256 = row["sha256"]
        if (
            not isinstance(sha256, str)
            or len(sha256) != 64
            or any(character not in "0123456789abcdef" for character in sha256)
        ):
            raise economic.Refusal("source fixture SHA-256 is not canonical lowercase hex")
        path = ROOT / relative
        try:
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
        except OSError as error:
            raise economic.Refusal(f"cannot read source fixture {relative}: {error}") from error
        if actual != sha256:
            raise economic.Refusal(f"source fixture {consumer} digest changed")
        result.append({"consumer": consumer, "path": relative, "sha256": sha256})
    return result


def authenticate_contract(value: Any) -> dict[str, Any]:
    contract = economic.exact_keys(
        value,
        {"schema", "sourceAuthority", "sourceFixtures", "seedDomain", "base", "seeds"},
        "multiwallet seed contract",
    )
    if contract["schema"] != CONTRACT_SCHEMA:
        raise economic.Refusal("multiwallet seed contract schema changed")
    authority = contract["sourceAuthority"]
    if not isinstance(authority, list) or authority != [
        "tools/release/private-validator-lifecycle/run.py::named_seed",
        "crates/dclutch-direct-codec/src/successor.rs::consume_nonce_v2",
        "crates/dclutch-direct-codec/src/successor.rs::settle_inline_ordinary_v2",
        "tools/economic-lifecycle-ledger/ledger.py",
    ]:
        raise economic.Refusal("multiwallet source authority changed")
    fixtures = canonical_source_fixtures(contract["sourceFixtures"])
    if contract["seedDomain"] != SEED_DOMAIN[:-1].decode() + "\0":
        raise economic.Refusal("named-seed domain changed")
    base = economic.exact_keys(
        contract["base"],
        {
            "wallets", "collateralAtomsPerAccount", "completeSetAtoms",
            "feeBasisPointsPerSide", "feeDenominator", "priceScaleAtoms",
            "cutDenominator", "cuts",
        },
        "multiwallet base",
    )
    wallets = base["wallets"]
    if wallets != ["ash", "birch", "cobalt", "dahlia"]:
        raise economic.Refusal("multiwallet role partition or order changed")
    collateral_per_account = economic.decimal(
        base["collateralAtomsPerAccount"], "collateral atoms per account", positive=True
    )
    complete_raw = economic.exact_keys(
        base["completeSetAtoms"], set(wallets), "complete-set owner quantities"
    )
    complete = {
        owner: economic.decimal(complete_raw[owner], f"{owner} complete set", positive=True)
        for owner in wallets
    }
    bps = base["feeBasisPointsPerSide"]
    if isinstance(bps, bool) or not isinstance(bps, int) or bps != 50:
        raise economic.Refusal("per-side fee policy changed")
    fee_denominator = economic.decimal(base["feeDenominator"], "fee denominator", positive=True)
    price_scale = economic.decimal(base["priceScaleAtoms"], "price scale", positive=True)
    cut_denominator = economic.decimal(base["cutDenominator"], "cut denominator", positive=True)
    cuts_raw = base["cuts"]
    if not isinstance(cuts_raw, list) or len(cuts_raw) != 2:
        raise economic.Refusal("winner cut partition changed")
    cuts = [economic.decimal(item, "winner cut", positive=True) for item in cuts_raw]
    if cuts[0] >= cuts[1] or fee_denominator != 10_000 or price_scale != 1_000:
        raise economic.Refusal("fee, price, or ordered cut scale changed")
    seeds = contract["seeds"]
    if not isinstance(seeds, list) or len(seeds) != len(EXPECTED_ROWS):
        raise economic.Refusal("named-seed matrix must contain exactly twenty rows")
    canonical_seeds: list[dict[str, str]] = []
    for raw, expected in zip(seeds, EXPECTED_ROWS, strict=True):
        row = economic.exact_keys(raw, {"name", "profile", "winnerCase"}, "named-seed row")
        actual = (row["name"], row["profile"], row["winnerCase"])
        if actual != expected:
            raise economic.Refusal("named-seed matrix is missing, duplicated, or reordered")
        canonical_seeds.append(
            {"name": expected[0], "profile": expected[1], "winnerCase": expected[2]}
        )
    return {
        "schema": CONTRACT_SCHEMA,
        "sourceAuthority": list(authority),
        "sourceFixtures": fixtures,
        "seedDomain": contract["seedDomain"],
        "base": {
            "wallets": list(wallets),
            "collateralAtomsPerAccount": collateral_per_account,
            "completeSetAtoms": complete,
            "feeBasisPointsPerSide": bps,
            "feeDenominator": fee_denominator,
            "priceScaleAtoms": price_scale,
            "cutDenominator": cut_denominator,
            "cuts": cuts,
        },
        "seeds": canonical_seeds,
    }


def named_seed(seed_name: str) -> str:
    return hashlib.sha256(SEED_DOMAIN + seed_name.encode()).hexdigest()


def conservation(snapshot: Mapping[str, Any]) -> dict[str, Any]:
    economic_snapshot = snapshot["economic"]
    collateral_sum = sum(int(value) for value in economic_snapshot["collateralAccounts"].values())
    positions = economic_snapshot["positions"]
    width = len(economic_snapshot["claimAggregateSupplyAtoms"])
    position_sums = [
        sum(int(row[coordinate]) for row in positions.values()) for coordinate in range(width)
    ]
    payout = economic_snapshot["payoutAtomsPerClaim"]
    winner = None if payout is None else payout.index("1")
    hoard = economic_snapshot["hoardPrincipalAtoms"]
    backing = (
        economic_snapshot["claimAggregateSupplyAtoms"]
        if winner is None
        else [economic_snapshot["claimAggregateSupplyAtoms"][winner]]
    )
    return {
        "collateralAccountSumAtoms": str(collateral_sum),
        "collateralMintSupplyAtoms": economic_snapshot["collateralMintSupplyAtoms"],
        "claimPositionSumsAtoms": [str(value) for value in position_sums],
        "claimAggregateSupplyAtoms": list(economic_snapshot["claimAggregateSupplyAtoms"]),
        "backingMode": "all-outcomes" if winner is None else "winning-outcome-only",
        "backedClaimIndices": list(range(width)) if winner is None else [winner],
        "backedLiabilityAtoms": list(backing),
        "hoardPrincipalAtoms": hoard,
        "holds": True,
    }


class Scenario:
    def __init__(self, contract: Mapping[str, Any], seed: Mapping[str, str]) -> None:
        self.base = contract["base"]
        self.seed_name = seed["name"]
        self.seed_sha256 = named_seed(self.seed_name)
        self.profile = seed["profile"]
        self.winner_case = seed["winnerCase"]
        self.owners = list(self.base["wallets"])
        collateral: dict[str, str] = {}
        self.account_owner: dict[str, str] = {}
        for owner in self.owners:
            for suffix in ("primary", "alternate"):
                account = f"{owner}-{suffix}"
                collateral[account] = str(self.base["collateralAtomsPerAccount"])
                self.account_owner[account] = owner
        collateral["hoard-principal"] = "0"
        collateral["protocol-fee"] = "0"
        fixture = {
            "outcomeCount": 4,
            "collateralMintSupplyAtoms": str(sum(int(item) for item in collateral.values())),
            "hoardCollateralAccount": "hoard-principal",
            "feeCollateralAccount": "protocol-fee",
            "initial": {"collateralAccounts": collateral, "claimOwners": self.owners},
        }
        self.ledger = economic.Ledger(fixture)
        self.next_nonce = {owner: 0 for owner in self.owners}
        self.consumed: list[str] = []
        self.resolved_winner: int | None = None
        self.redeemed_rows: list[dict[str, Any]] = []
        self.transitions: list[dict[str, Any]] = []
        self.accepted_direct: list[dict[str, Any]] = []
        self.record(
            "initial", "initial", "checkpoint", "checkpoint", None, None, {}, self.snapshot()
        )

    def snapshot(self) -> dict[str, Any]:
        return {
            "economic": self.ledger.snapshot(),
            "control": {
                "makerNextNonces": {
                    owner: str(self.next_nonce[owner]) for owner in sorted(self.next_nonce)
                },
                "consumedPairedIntentIds": list(self.consumed),
                "resolvedWinner": self.resolved_winner,
                "redeemedScheduleRows": copy.deepcopy(self.redeemed_rows),
                "retired": self.ledger.retired,
            },
        }

    def record(
        self,
        action_id: str,
        stage: str,
        kind: str,
        status: str,
        refusal_code: str | None,
        dispatch_group: str | None,
        details: Mapping[str, Any],
        pre: Mapping[str, Any],
    ) -> None:
        post = self.snapshot()
        self.transitions.append(
            {
                "ordinal": len(self.transitions),
                "actionId": action_id,
                "stage": stage,
                "dispatchGroup": dispatch_group,
                "kind": kind,
                "expectedStatus": status,
                "refusalCode": refusal_code,
                "preSnapshotSha256": digest(pre),
                "postSnapshotSha256": digest(post),
                "snapshot": post,
                "conservation": conservation(post),
                "details": copy.deepcopy(dict(details)),
            }
        )

    def founding(self) -> None:
        pre = self.snapshot()
        candidate = copy.deepcopy(self.ledger)
        events = []
        for owner in self.owners:
            quantity = self.base["completeSetAtoms"][owner]
            event = {
                "kind": "complete-set",
                "sourceCollateral": f"{owner}-primary",
                "owner": owner,
                "quantityAtoms": str(quantity),
            }
            candidate.apply(event, "founding")
            events.append(event)
        self.ledger = candidate
        self.record("founding", "founding", "complete-set-batch", "accepted", None, None, {"events": events}, pre)

    def make_direct(
        self,
        label: str,
        seller: str,
        buyer: str,
        outcome: int,
        fill: int,
        price: int,
        seller_account: str,
        buyer_account: str,
        *,
        seller_nonce: int | None = None,
        buyer_nonce: int | None = None,
    ) -> dict[str, Any]:
        action = {
            "sellerOwner": seller,
            "buyerOwner": buyer,
            "sellerCollateral": seller_account,
            "buyerCollateral": buyer_account,
            "feeCollateral": "protocol-fee",
            "outcome": outcome,
            "fillAtoms": str(fill),
            "executionPriceAtoms": str(price),
            "priceScaleAtoms": str(self.base["priceScaleAtoms"]),
            "feeBasisPoints": self.base["feeBasisPointsPerSide"],
            "feeDenominator": str(self.base["feeDenominator"]),
            "sellerNonce": self.next_nonce[seller] if seller_nonce is None else seller_nonce,
            "buyerNonce": self.next_nonce[buyer] if buyer_nonce is None else buyer_nonce,
        }
        paired_payload = {"seedSha256": self.seed_sha256, **action}
        return {"label": label, "pairedIntentId": digest(paired_payload), **action}

    def execute_direct(
        self,
        action: Mapping[str, Any],
        *,
        expected_refusal: str | None = None,
        dispatch_group: str | None = None,
    ) -> None:
        pre = self.snapshot()
        refusal: str | None = None
        try:
            paired = action["pairedIntentId"]
            seller = action["sellerOwner"]
            buyer = action["buyerOwner"]
            if paired in self.consumed:
                raise ActionRefusal("duplicate-paired-intent")
            if seller == buyer or seller not in self.next_nonce or buyer not in self.next_nonce:
                raise ActionRefusal("maker-partition-mismatch")
            if action["sellerNonce"] != self.next_nonce[seller]:
                raise ActionRefusal("seller-nonce-mismatch")
            if action["buyerNonce"] != self.next_nonce[buyer]:
                raise ActionRefusal("buyer-nonce-mismatch")
            if self.account_owner.get(action["sellerCollateral"]) != seller:
                raise ActionRefusal("seller-collateral-owner-mismatch")
            if self.account_owner.get(action["buyerCollateral"]) != buyer:
                raise ActionRefusal("buyer-collateral-owner-mismatch")
            event = {
                "kind": "direct",
                "sellerOwner": seller,
                "buyerOwner": buyer,
                "sellerCollateral": action["sellerCollateral"],
                "buyerCollateral": action["buyerCollateral"],
                "feeCollateral": action["feeCollateral"],
                "outcome": action["outcome"],
                "fillAtoms": action["fillAtoms"],
                "executionPriceAtoms": action["executionPriceAtoms"],
                "priceScaleAtoms": action["priceScaleAtoms"],
                "feeBasisPoints": action["feeBasisPoints"],
                "feeDenominator": action["feeDenominator"],
                "expectedQuote": None,
            }
            candidate = copy.deepcopy(self.ledger)
            candidate.apply(event, "direct")
        except ActionRefusal as error:
            refusal = error.code
        except economic.Refusal as error:
            raise economic.Refusal(f"generated Direct action is economically invalid: {error}") from error
        if refusal is not None:
            if refusal != expected_refusal:
                raise economic.Refusal(
                    f"{action['label']} refused {refusal}, expected {expected_refusal or 'acceptance'}"
                )
            self.record(
                str(action["label"]), "direct", "paired-direct", "refused", refusal,
                dispatch_group, {"action": dict(action)}, pre,
            )
            return
        if expected_refusal is not None:
            raise economic.Refusal(f"{action['label']} accepted instead of refusing {expected_refusal}")
        self.ledger = candidate
        self.next_nonce[seller] += 1
        self.next_nonce[buyer] += 1
        self.consumed.append(str(action["pairedIntentId"]))
        quote = copy.deepcopy(self.ledger.direct_quotes[-1])
        accepted = {"action": copy.deepcopy(dict(action)), "quote": quote}
        self.accepted_direct.append(accepted)
        self.record(
            str(action["label"]), "direct", "paired-direct", "accepted", None,
            dispatch_group, accepted, pre,
        )

    def winner_facts(self) -> dict[str, Any]:
        cut_denominator = self.base["cutDenominator"]
        first, second = self.base["cuts"]
        if self.winner_case == "failure":
            return {
                "case": self.winner_case,
                "providerStatus": "failure",
                "priceNumerator": None,
                "priceDenominator": None,
                "cutNumerators": [str(first), str(second)],
                "cutDenominator": str(cut_denominator),
                "selectedOutcome": 3,
                "comparison": "provider-failure-terminal-outcome",
            }
        prices = {
            "below-first": first - 1,
            "at-first": first,
            "below-second": second - 1,
            "at-second": second,
            "above-second": second + 1,
        }
        price = prices[self.winner_case]
        left_first = economic.checked_mul_u128(price, cut_denominator, "winner price cross product")
        right_first = economic.checked_mul_u128(first, cut_denominator, "first cut cross product")
        left_second = economic.checked_mul_u128(price, cut_denominator, "winner price cross product")
        right_second = economic.checked_mul_u128(second, cut_denominator, "second cut cross product")
        winner = 0 if left_first < right_first else 1 if left_second < right_second else 2
        return {
            "case": self.winner_case,
            "providerStatus": "success",
            "priceNumerator": str(price),
            "priceDenominator": str(cut_denominator),
            "cutNumerators": [str(first), str(second)],
            "cutDenominator": str(cut_denominator),
            "selectedOutcome": winner,
            "comparison": "exact-u128-cross-multiplication-left-closed-cuts",
        }

    def resolve(self) -> dict[str, Any]:
        pre = self.snapshot()
        facts = self.winner_facts()
        winner = facts["selectedOutcome"]
        payout = ["1" if coordinate == winner else "0" for coordinate in range(4)]
        candidate = copy.deepcopy(self.ledger)
        candidate.apply({"kind": "resolve", "payoutAtomsPerClaim": payout}, "resolution")
        self.ledger = candidate
        self.resolved_winner = winner
        details = {
            "winner": facts,
            "payoutAtomsPerClaim": payout,
            "frozenScheduleSha256": digest(self.ledger.schedule_rows()),
        }
        self.record("resolution", "resolution", "categorical-resolution", "accepted", None, None, details, pre)
        return details

    def attempt_retirement_refusal(self) -> None:
        pre = self.snapshot()
        candidate = copy.deepcopy(self.ledger)
        try:
            candidate.apply({"kind": "retire"}, "aggregate-retirement")
        except economic.Refusal as error:
            if "exhaustive winning and losing" not in str(error):
                raise
        else:
            raise economic.Refusal("retirement-before-zero unexpectedly accepted")
        self.record(
            "retirement-before-zero", "aggregate-retirement", "retire", "refused",
            "retirement-before-zero", None, {"required": "all winning and losing rows burned"}, pre,
        )

    def payouts_and_retire(self) -> dict[str, Any]:
        schedule = self.ledger.schedule_rows()
        if schedule is None:
            raise economic.Refusal("payout schedule was not frozen")
        schedule_sha = digest(schedule)
        split = (len(schedule) + 1) // 2
        zero_burns = 0
        for order, row in enumerate(schedule):
            pre = self.snapshot()
            payout = int(row["quantityAtoms"]) if row["claimIndex"] == self.resolved_winner else 0
            event = {
                "kind": "redeem",
                "owner": row["owner"],
                "recipientCollateral": f"{row['owner']}-primary",
                "outcome": row["claimIndex"],
                "quantityAtoms": row["quantityAtoms"],
            }
            candidate = copy.deepcopy(self.ledger)
            candidate.apply(event, "payouts")
            self.ledger = candidate
            redeemed = {
                "scheduleIndex": order,
                "owner": row["owner"],
                "claimIndex": row["claimIndex"],
                "quantityAtoms": row["quantityAtoms"],
                "payoutCollateralAtoms": str(payout),
                "recipientCollateral": event["recipientCollateral"],
            }
            self.redeemed_rows.append(redeemed)
            if payout == 0:
                zero_burns += 1
            batch = "payout-batch-1" if order < split else "payout-batch-2"
            self.record(
                f"payout-{order:02d}", "payouts", "full-frozen-row-redemption", "accepted",
                None, batch, {"frozenScheduleSha256": schedule_sha, **redeemed}, pre,
            )
            if self.profile == "retirement-before-zero" and order == 0:
                self.attempt_retirement_refusal()
            if self.profile == "partial-payout-resume" and order + 1 == split:
                checkpoint = self.snapshot()
                self.record(
                    "payout-resume-frontier", "payouts", "resume-checkpoint", "checkpoint",
                    None, None,
                    {
                        "frozenScheduleSha256": schedule_sha,
                        "completedRows": split,
                        "remainingRows": len(schedule) - split,
                    },
                    checkpoint,
                )
        pre = self.snapshot()
        candidate = copy.deepcopy(self.ledger)
        candidate.apply({"kind": "retire"}, "aggregate-retirement")
        self.ledger = candidate
        self.record(
            "aggregate-retirement", "aggregate-retirement", "retire", "accepted", None,
            None, {"frozenScheduleSha256": schedule_sha, "exhaustiveRows": len(schedule)}, pre,
        )
        return {
            "frozenScheduleSha256": schedule_sha,
            "frozenScheduleRows": len(schedule),
            "zeroPayoutBurnRows": zero_burns,
        }

    def run_direct_matrix(self) -> None:
        if self.profile == "seller-future-nonce":
            hostile = self.make_direct(
                "seller-future-nonce", "ash", "birch", 0, 1, 1000,
                "ash-primary", "birch-primary", seller_nonce=1,
            )
            self.execute_direct(hostile, expected_refusal="seller-nonce-mismatch")
        if self.profile == "buyer-future-nonce":
            hostile = self.make_direct(
                "buyer-future-nonce", "ash", "birch", 0, 1, 1000,
                "ash-primary", "birch-primary", buyer_nonce=1,
            )
            self.execute_direct(hostile, expected_refusal="buyer-nonce-mismatch")
        if self.profile == "foreign-account-refusal":
            hostile = self.make_direct(
                "foreign-account-refusal", "ash", "birch", 0, 1, 1000,
                "birch-primary", "birch-primary",
            )
            self.execute_direct(hostile, expected_refusal="seller-collateral-owner-mismatch")
        if self.profile == "simultaneous-seller":
            actions = [
                self.make_direct("simultaneous-seller-a", "ash", "birch", 0, 1, 1000, "ash-primary", "birch-primary"),
                self.make_direct("simultaneous-seller-b", "ash", "cobalt", 1, 1, 1000, "ash-alternate", "cobalt-primary"),
            ]
            for order, action in enumerate(sorted(actions, key=lambda item: item["pairedIntentId"])):
                self.execute_direct(
                    action,
                    expected_refusal=None if order == 0 else "seller-nonce-mismatch",
                    dispatch_group="simultaneous-seller-canonical-order",
                )
        if self.profile == "simultaneous-buyer":
            actions = [
                self.make_direct("simultaneous-buyer-a", "ash", "birch", 0, 1, 1000, "ash-primary", "birch-primary"),
                self.make_direct("simultaneous-buyer-b", "cobalt", "birch", 2, 1, 1000, "cobalt-primary", "birch-alternate"),
            ]
            for order, action in enumerate(sorted(actions, key=lambda item: item["pairedIntentId"])):
                self.execute_direct(
                    action,
                    expected_refusal=None if order == 0 else "buyer-nonce-mismatch",
                    dispatch_group="simultaneous-buyer-canonical-order",
                )

        trade_rows = [
            ("ash", "birch", 0, 400, 250, "ash-primary", "birch-primary"),
            ("birch", "cobalt", 1, 700, 500, "birch-alternate", "cobalt-primary"),
            ("cobalt", "dahlia", 2, 400, 750, "cobalt-alternate", "dahlia-primary"),
            ("dahlia", "ash", 3, 100, 900, "dahlia-alternate", "ash-alternate"),
        ]
        if self.profile == "fee-floor-below":
            trade_rows[0] = ("ash", "birch", 0, 1000, 199, "ash-primary", "birch-primary")
        if self.profile == "fee-floor-at":
            trade_rows[0] = ("ash", "birch", 0, 1000, 200, "ash-primary", "birch-primary")
        first_action: dict[str, Any] | None = None
        for order, row in enumerate(trade_rows):
            action = self.make_direct(f"direct-{order}", *row)
            self.execute_direct(action)
            if order == 0:
                first_action = action
                if self.profile == "duplicate-ticket":
                    self.execute_direct(action, expected_refusal="duplicate-paired-intent")
                if self.profile == "direct-replay":
                    replay = self.make_direct(
                        "direct-replay-stale-nonce", "ash", "birch", 0, 200, 250,
                        "ash-alternate", "birch-alternate", seller_nonce=action["sellerNonce"],
                        buyer_nonce=action["buyerNonce"],
                    )
                    self.execute_direct(replay, expected_refusal="seller-nonce-mismatch")
        if first_action is None:
            raise economic.Refusal("base Direct ensemble disappeared")

    def derive(self) -> dict[str, Any]:
        self.founding()
        self.run_direct_matrix()
        resolution = self.resolve()
        payout = self.payouts_and_retire()
        terminal = self.snapshot()
        if (
            terminal["economic"]["claimAggregateSupplyAtoms"] != ["0"] * 4
            or terminal["economic"]["hoardPrincipalAtoms"] != "0"
            or terminal["economic"]["retired"] is not True
        ):
            raise economic.Refusal("generated scenario did not reach exact aggregate retirement")
        refused = [row for row in self.transitions if row["expectedStatus"] == "refused"]
        checkpoints = [row for row in self.transitions if row["expectedStatus"] == "checkpoint"]
        return {
            "seedName": self.seed_name,
            "seedSha256": self.seed_sha256,
            "profile": self.profile,
            "winnerCase": self.winner_case,
            "winner": resolution["winner"],
            "transitions": self.transitions,
            "summary": {
                "transitionCount": len(self.transitions),
                "acceptedCount": sum(row["expectedStatus"] == "accepted" for row in self.transitions),
                "refusedCount": len(refused),
                "checkpointCount": len(checkpoints),
                "refusalCodes": [row["refusalCode"] for row in refused],
                "acceptedDirectActions": self.accepted_direct,
                "terminalMakerNextNonces": terminal["control"]["makerNextNonces"],
                **payout,
                "terminalSnapshotSha256": digest(terminal),
            },
        }


def derive_contract(value: Any) -> dict[str, Any]:
    contract = authenticate_contract(value)
    scenarios = [Scenario(contract, seed).derive() for seed in contract["seeds"]]
    seed_set = [
        {"name": row["seedName"], "sha256": row["seedSha256"]} for row in scenarios
    ]
    return {
        "schema": OUTPUT_SCHEMA,
        "contractSha256": digest(value),
        "sourceAuthority": contract["sourceAuthority"],
        "sourceFixtures": contract["sourceFixtures"],
        "seedSetSha256": digest(seed_set),
        "scenarioCount": len(scenarios),
        "scenarios": scenarios,
    }


def expected_transition(derived: Mapping[str, Any], seed_name: str, ordinal: int) -> Mapping[str, Any]:
    scenarios = derived.get("scenarios")
    if not isinstance(scenarios, list):
        raise economic.Refusal("derived multiwallet scenarios are absent")
    matches = [row for row in scenarios if row.get("seedName") == seed_name]
    if len(matches) != 1:
        raise economic.Refusal("observed seed is absent or ambiguous")
    transitions = matches[0].get("transitions")
    if (
        isinstance(ordinal, bool)
        or not isinstance(ordinal, int)
        or not isinstance(transitions, list)
        or not 0 <= ordinal < len(transitions)
        or transitions[ordinal].get("ordinal") != ordinal
    ):
        raise economic.Refusal("observed transition ordinal is absent")
    return transitions[ordinal]


def check_observed(derived: Mapping[str, Any], value: Any) -> dict[str, Any]:
    observed = economic.exact_keys(
        value, {"schema", "contractSha256", "seedName", "ordinal", "snapshot"},
        "multiwallet observed snapshot",
    )
    if observed["schema"] != OBSERVED_SCHEMA:
        raise economic.Refusal("multiwallet observed schema changed")
    if observed["contractSha256"] != derived.get("contractSha256"):
        raise economic.Refusal("observed snapshot belongs to another contract")
    transition = expected_transition(derived, observed["seedName"], observed["ordinal"])
    if observed["snapshot"] != transition["snapshot"]:
        raise economic.Refusal("observed multiwallet snapshot differs from the exact model")
    return {
        "status": "accepted",
        "seedName": observed["seedName"],
        "ordinal": observed["ordinal"],
        "snapshotSha256": transition["postSnapshotSha256"],
    }


def write_canonical(path: Path, value: Any) -> None:
    payload = economic.canonical_bytes(value)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(payload)
    temporary.replace(path)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    derive_parser = subparsers.add_parser("derive")
    derive_parser.add_argument("contract", type=Path)
    emit_parser = subparsers.add_parser("emit")
    emit_parser.add_argument("contract", type=Path)
    emit_parser.add_argument("output", type=Path)
    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("contract", type=Path)
    check_parser.add_argument("observed", type=Path)
    arguments = parser.parse_args(argv)
    try:
        derived = derive_contract(load_json(arguments.contract))
        if arguments.command == "derive":
            print(economic.canonical_bytes(derived).decode(), end="")
        elif arguments.command == "emit":
            write_canonical(arguments.output, derived)
            print(digest(derived))
        else:
            print(
                economic.canonical_bytes(check_observed(derived, load_json(arguments.observed))).decode(),
                end="",
            )
    except economic.Refusal as error:
        parser.exit(2, f"refused: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
