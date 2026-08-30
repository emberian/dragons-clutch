#!/usr/bin/env python3
"""Exact-integer lifecycle ledger oracle.

This is deliberately a source model, not an RPC client.  It predicts the
collateral, Claims, fee, Hoard-principal, and retirement ledger from an
authenticated fixture.  Runtime callers may compare a captured snapshot with
``check`` before advancing to the next expensive validator stage.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any, Mapping, Sequence


U64_MAX = (1 << 64) - 1
U128_MAX = (1 << 128) - 1
FIXTURE_SCHEMA = "dclutch-exact-economic-lifecycle-fixture-v1"
OUTPUT_SCHEMA = "dclutch-exact-economic-lifecycle-ledger-v1"
OBSERVED_SCHEMA = "dclutch-exact-economic-lifecycle-observed-snapshot-v1"
LAMPORT_TRACE_SCHEMA = "dclutch-exact-lamport-trace-v1"
HOARD_CLASSIFICATION = (
    "collateral-principal-not-fee-bounty-rent-reserve-or-treasury"
)


class Refusal(RuntimeError):
    """The input could not satisfy the exact lifecycle contract."""


def exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise Refusal(f"{label} fields changed")
    return value


def decimal(value: Any, label: str, *, positive: bool = False) -> int:
    if (
        not isinstance(value, str)
        or not value
        or not value.isascii()
        or not value.isdecimal()
        or (len(value) > 1 and value.startswith("0"))
    ):
        raise Refusal(f"{label} is not canonical unsigned decimal text")
    number = int(value)
    if number > U64_MAX:
        raise Refusal(f"{label} exceeds u64")
    if positive and number == 0:
        raise Refusal(f"{label} must be positive")
    return number


def index(value: Any, width: int, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value < width:
        raise Refusal(f"{label} is outside the canonical outcome partition")
    return value


def name(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value) > 128
        or any(character not in "abcdefghijklmnopqrstuvwxyz0123456789-." for character in value)
    ):
        raise Refusal(f"{label} is not one stable identifier")
    return value


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def checked_add(left: int, right: int, label: str) -> int:
    result = left + right
    if result > U64_MAX:
        raise Refusal(f"{label} overflows u64")
    return result


def checked_mul_u128(left: int, right: int, label: str) -> int:
    result = left * right
    if result > U128_MAX:
        raise Refusal(f"{label} overflows u128")
    return result


def exact_direct_quote(
    fill_atoms: int,
    execution_price_atoms: int,
    price_scale_atoms: int,
    fee_basis_points: int,
    fee_denominator: int,
) -> dict[str, int]:
    """Return the Direct quote with its two named integer boundaries."""

    if min(fill_atoms, execution_price_atoms, price_scale_atoms, fee_denominator) <= 0:
        raise Refusal("Direct quote requires positive fill, price, scale, and denominator")
    if not 0 <= fee_basis_points <= fee_denominator:
        raise Refusal("Direct fee basis points exceed their denominator")
    product = checked_mul_u128(fill_atoms, execution_price_atoms, "Direct gross product")
    gross, remainder = divmod(product, price_scale_atoms)
    if remainder != 0 or gross > U64_MAX:
        raise Refusal("Direct gross quote is not one exact u64 atom quantity")
    fee_product = checked_mul_u128(gross, fee_basis_points, "Direct fee product")
    fee = fee_product // fee_denominator
    seller_net = gross - fee
    buyer_debit = checked_add(gross, fee, "Direct buyer debit")
    fee_credit = checked_add(fee, fee, "Direct two-sided fee credit")
    if seller_net + fee_credit != buyer_debit:
        raise Refusal("Direct two-sided fee conservation changed")
    return {
        "grossCollateralAtoms": gross,
        "grossRemainderAtoms": remainder,
        "sellerFeeAtoms": fee,
        "buyerFeeAtoms": fee,
        "sellerNetAtoms": seller_net,
        "buyerDebitAtoms": buyer_debit,
        "feeRecipientCreditAtoms": fee_credit,
    }


class Ledger:
    def __init__(self, fixture: Mapping[str, Any]) -> None:
        initial = exact_keys(
            fixture.get("initial"),
            {"collateralAccounts", "claimOwners"},
            "fixture initial state",
        )
        outcome_count = fixture.get("outcomeCount")
        if isinstance(outcome_count, bool) or not isinstance(outcome_count, int) or not 1 <= outcome_count <= 32:
            raise Refusal("outcomeCount is not one bounded positive integer")
        self.outcome_count = outcome_count
        raw_accounts = initial["collateralAccounts"]
        if not isinstance(raw_accounts, dict) or not raw_accounts:
            raise Refusal("initial collateral accounts are absent")
        self.collateral: dict[str, int] = {}
        for raw_account, raw_atoms in raw_accounts.items():
            account = name(raw_account, "collateral account")
            if account in self.collateral:
                raise Refusal("collateral account repeats")
            self.collateral[account] = decimal(raw_atoms, f"{account} collateral")
        self.hoard_account = name(fixture.get("hoardCollateralAccount"), "Hoard account")
        self.fee_account = name(fixture.get("feeCollateralAccount"), "fee account")
        if self.hoard_account not in self.collateral or self.fee_account not in self.collateral:
            raise Refusal("Hoard or fee account is absent from collateral accounts")
        owners = initial["claimOwners"]
        if not isinstance(owners, list) or not owners:
            raise Refusal("claim owners are absent")
        self.positions: dict[str, list[int]] = {}
        for raw_owner in owners:
            owner = name(raw_owner, "claim owner")
            if owner in self.positions:
                raise Refusal("claim owner repeats")
            self.positions[owner] = [0] * outcome_count
        self.aggregate = [0] * outcome_count
        self.collateral_supply = sum(self.collateral.values())
        if self.collateral_supply > U64_MAX:
            raise Refusal("collateral Mint supply exceeds u64")
        expected_supply = decimal(
            fixture.get("collateralMintSupplyAtoms"), "collateral Mint supply", positive=True
        )
        if self.collateral_supply != expected_supply:
            raise Refusal("initial collateral accounts do not exhaust Mint supply")
        self.protocol_fees = 0
        self.payout: list[int] | None = None
        self.frozen_schedule: tuple[tuple[str, int, int], ...] | None = None
        self.retired = False
        self.direct_quotes: list[dict[str, Any]] = []
        self.account_prefix_delta = {account: 0 for account in self.collateral}
        self.account_min_prefix = {account: 0 for account in self.collateral}
        self.account_debits = {account: 0 for account in self.collateral}
        self.account_credits = {account: 0 for account in self.collateral}
        self.assert_invariants()

    def collateral_delta(self, account: str, delta: int, label: str) -> None:
        if account not in self.collateral:
            raise Refusal(f"{label} names absent collateral account {account}")
        after = self.collateral[account] + delta
        if after < 0 or after > U64_MAX:
            raise Refusal(f"{label} exceeds available collateral or u64")
        self.collateral[account] = after
        self.account_prefix_delta[account] += delta
        self.account_min_prefix[account] = min(
            self.account_min_prefix[account], self.account_prefix_delta[account]
        )
        if delta < 0:
            self.account_debits[account] += -delta
        else:
            self.account_credits[account] += delta

    def transfer_collateral(self, event: Mapping[str, Any]) -> None:
        row = exact_keys(
            event,
            {"kind", "sourceCollateral", "destinationCollateral", "quantityAtoms"},
            "collateral transfer",
        )
        quantity = decimal(row["quantityAtoms"], "collateral transfer quantity", positive=True)
        source = name(row["sourceCollateral"], "collateral transfer source")
        destination = name(row["destinationCollateral"], "collateral transfer destination")
        if source == destination:
            raise Refusal("collateral transfer aliases source and destination")
        self.collateral_delta(source, -quantity, "collateral transfer")
        self.collateral_delta(destination, quantity, "collateral transfer")

    def complete_set(self, event: Mapping[str, Any]) -> None:
        row = exact_keys(
            event,
            {"kind", "sourceCollateral", "owner", "quantityAtoms"},
            "complete-set event",
        )
        if self.payout is not None:
            raise Refusal("complete-set liabilities cannot mint after resolution")
        quantity = decimal(row["quantityAtoms"], "complete-set quantity", positive=True)
        source = name(row["sourceCollateral"], "complete-set collateral source")
        owner = name(row["owner"], "complete-set owner")
        if owner not in self.positions:
            raise Refusal("complete-set owner is absent")
        self.collateral_delta(source, -quantity, "complete-set principal debit")
        self.collateral_delta(self.hoard_account, quantity, "complete-set Hoard principal")
        for coordinate in range(self.outcome_count):
            self.positions[owner][coordinate] = checked_add(
                self.positions[owner][coordinate], quantity, "Position liability"
            )
            self.aggregate[coordinate] = checked_add(
                self.aggregate[coordinate], quantity, "aggregate liability"
            )

    def direct(self, event: Mapping[str, Any], stage: str) -> None:
        keys = {
            "kind", "sellerOwner", "buyerOwner", "sellerCollateral",
            "buyerCollateral", "feeCollateral", "outcome", "fillAtoms",
            "executionPriceAtoms", "priceScaleAtoms", "feeBasisPoints",
            "feeDenominator", "expectedQuote",
        }
        row = exact_keys(event, keys, "Direct event")
        if self.payout is not None:
            raise Refusal("Direct cannot execute after resolution")
        seller = name(row["sellerOwner"], "Direct seller")
        buyer = name(row["buyerOwner"], "Direct buyer")
        if seller == buyer or seller not in self.positions or buyer not in self.positions:
            raise Refusal("Direct owners alias or are absent")
        coordinate = index(row["outcome"], self.outcome_count, "Direct outcome")
        fill = decimal(row["fillAtoms"], "Direct fill", positive=True)
        price = decimal(row["executionPriceAtoms"], "Direct execution price", positive=True)
        scale = decimal(row["priceScaleAtoms"], "Direct price scale", positive=True)
        denominator = decimal(row["feeDenominator"], "Direct fee denominator", positive=True)
        bps = row["feeBasisPoints"]
        if isinstance(bps, bool) or not isinstance(bps, int):
            raise Refusal("Direct fee basis points changed type")
        quote = exact_direct_quote(fill, price, scale, bps, denominator)
        expected_quote = row["expectedQuote"]
        if expected_quote is not None:
            expected = {
                key: decimal(value, f"expected Direct {key}")
                for key, value in exact_keys(
                    expected_quote, set(quote), "expected Direct quote"
                ).items()
            }
            if quote != expected:
                raise Refusal("fixture Direct quote differs from exact arithmetic")
        if self.positions[seller][coordinate] < fill:
            raise Refusal("Direct seller lacks the selected claim quantity")
        self.positions[seller][coordinate] -= fill
        self.positions[buyer][coordinate] = checked_add(
            self.positions[buyer][coordinate], fill, "Direct buyer Position"
        )
        seller_collateral = name(row["sellerCollateral"], "Direct seller collateral")
        buyer_collateral = name(row["buyerCollateral"], "Direct buyer collateral")
        fee_collateral = name(row["feeCollateral"], "Direct fee collateral")
        if fee_collateral != self.fee_account:
            raise Refusal("Direct fee destination changed")
        self.collateral_delta(
            buyer_collateral, -quote["buyerDebitAtoms"], "Direct buyer debit"
        )
        self.collateral_delta(
            seller_collateral, quote["sellerNetAtoms"], "Direct seller net"
        )
        self.collateral_delta(
            fee_collateral, quote["feeRecipientCreditAtoms"], "Direct fee credit"
        )
        self.protocol_fees = checked_add(
            self.protocol_fees, quote["feeRecipientCreditAtoms"], "protocol fee revenue"
        )
        self.direct_quotes.append({"stage": stage, **{key: str(value) for key, value in quote.items()}})

    def resolve(self, event: Mapping[str, Any]) -> None:
        row = exact_keys(event, {"kind", "payoutAtomsPerClaim"}, "resolution event")
        if self.payout is not None:
            raise Refusal("Market resolves more than once")
        raw = row["payoutAtomsPerClaim"]
        if not isinstance(raw, list) or len(raw) != self.outcome_count:
            raise Refusal("resolution payout partition width changed")
        payout = [decimal(value, "resolution payout atom") for value in raw]
        if payout.count(1) != 1 or any(value not in (0, 1) for value in payout):
            raise Refusal("canonical categorical payout must be one-hot at unit scale")
        self.payout = payout
        self.frozen_schedule = tuple(
            sorted(
                (owner, coordinate, quantity)
                for owner, balances in self.positions.items()
                for coordinate, quantity in enumerate(balances)
                if quantity > 0
            )
        )
        if not self.frozen_schedule:
            raise Refusal("resolution produced an empty payout schedule")

    def redeem(self, event: Mapping[str, Any]) -> None:
        row = exact_keys(
            event,
            {"kind", "owner", "recipientCollateral", "outcome", "quantityAtoms"},
            "redemption event",
        )
        if self.payout is None or self.frozen_schedule is None:
            raise Refusal("redemption requires a frozen terminal schedule")
        owner = name(row["owner"], "redemption owner")
        if owner not in self.positions:
            raise Refusal("redemption owner is absent")
        coordinate = index(row["outcome"], self.outcome_count, "redemption outcome")
        quantity = decimal(row["quantityAtoms"], "redemption quantity", positive=True)
        if self.positions[owner][coordinate] != quantity:
            raise Refusal("redemption quantity differs from the frozen live claim")
        if (owner, coordinate, quantity) not in self.frozen_schedule:
            raise Refusal("redemption is absent from the frozen schedule")
        payout = checked_mul_u128(quantity, self.payout[coordinate], "terminal payout")
        if payout > U64_MAX or self.aggregate[coordinate] < quantity:
            raise Refusal("terminal burn exceeds aggregate liability")
        self.positions[owner][coordinate] = 0
        self.aggregate[coordinate] -= quantity
        if payout:
            recipient = name(row["recipientCollateral"], "redemption recipient")
            self.collateral_delta(self.hoard_account, -payout, "Hoard payout principal")
            self.collateral_delta(recipient, payout, "redemption collateral credit")

    def retire(self, event: Mapping[str, Any]) -> None:
        exact_keys(event, {"kind"}, "retirement event")
        if self.payout is None:
            raise Refusal("retirement requires terminal resolution")
        if any(self.aggregate) or any(any(row) for row in self.positions.values()):
            raise Refusal("retirement requires exhaustive winning and losing claim burns")
        if self.collateral[self.hoard_account] != 0:
            raise Refusal("retirement requires zero Hoard principal")
        self.retired = True

    def apply(self, event: Mapping[str, Any], stage: str) -> None:
        if self.retired:
            raise Refusal("no economic event may follow retirement")
        kind = event.get("kind") if isinstance(event, dict) else None
        if kind == "transfer-collateral":
            self.transfer_collateral(event)
        elif kind == "complete-set":
            self.complete_set(event)
        elif kind == "direct":
            self.direct(event, stage)
        elif kind == "resolve":
            self.resolve(event)
        elif kind == "redeem":
            self.redeem(event)
        elif kind == "retire":
            self.retire(event)
        else:
            raise Refusal(f"unknown economic event kind {kind!r}")
        self.assert_invariants()

    def assert_invariants(self) -> None:
        if sum(self.collateral.values()) != self.collateral_supply:
            raise Refusal("Token-2022 collateral Mint supply is not conserved")
        for coordinate in range(self.outcome_count):
            position_sum = sum(row[coordinate] for row in self.positions.values())
            if position_sum != self.aggregate[coordinate]:
                raise Refusal("Claims aggregate supply differs from Position liabilities")
        hoard = self.collateral[self.hoard_account]
        if self.payout is None:
            if any(value != hoard for value in self.aggregate):
                raise Refusal("pre-resolution complete-set liabilities lack exact Hoard backing")
        else:
            winner = self.payout.index(1)
            if self.aggregate[winner] != hoard:
                raise Refusal("remaining winning liabilities differ from Hoard principal")

    def schedule_rows(self) -> list[dict[str, Any]] | None:
        if self.frozen_schedule is None:
            return None
        return [
            {"owner": owner, "claimIndex": coordinate, "quantityAtoms": str(quantity)}
            for owner, coordinate, quantity in self.frozen_schedule
        ]

    def snapshot(self) -> dict[str, Any]:
        return {
            "collateralMintSupplyAtoms": str(self.collateral_supply),
            "collateralAccounts": {
                account: str(self.collateral[account]) for account in sorted(self.collateral)
            },
            "hoardPrincipalAtoms": str(self.collateral[self.hoard_account]),
            "hoardPrincipalClassification": HOARD_CLASSIFICATION,
            "protocolFeeRevenueAtoms": str(self.protocol_fees),
            "claimAggregateSupplyAtoms": [str(value) for value in self.aggregate],
            "positions": {
                owner: [str(value) for value in self.positions[owner]]
                for owner in sorted(self.positions)
            },
            "payoutAtomsPerClaim": (
                None if self.payout is None else [str(value) for value in self.payout]
            ),
            "frozenPayoutSchedule": self.schedule_rows(),
            "retired": self.retired,
            "invariants": {
                "token2022CollateralSupplyConserved": True,
                "claimsAggregateEqualsPositions": True,
                "hoardBacksWinningLiabilityOnly": True,
                "hoardPrincipalExcludedFromRevenue": True,
            },
        }

    def collateral_envelopes(self) -> dict[str, dict[str, str]]:
        return {
            account: {
                "totalDebitAtoms": str(self.account_debits[account]),
                "totalCreditAtoms": str(self.account_credits[account]),
                "minimumInitialAtomsForThisOrdering": str(-self.account_min_prefix[account]),
                "initialAtoms": str(
                    self.collateral[account] - self.account_prefix_delta[account]
                ),
            }
            for account in sorted(self.collateral)
        }


def derive_fixture(value: Any) -> dict[str, Any]:
    fixture = exact_keys(
        value,
        {
            "schema", "fixtureId", "sourceAuthority", "outcomeCount",
            "collateralMintSupplyAtoms", "hoardCollateralAccount",
            "feeCollateralAccount", "initial", "stages", "lamportContract",
            "activityV3Authority",
        },
        "economic lifecycle fixture",
    )
    if fixture["schema"] != FIXTURE_SCHEMA:
        raise Refusal("economic lifecycle fixture schema changed")
    fixture_id = name(fixture["fixtureId"], "fixture id")
    if not isinstance(fixture["sourceAuthority"], list) or not fixture["sourceAuthority"]:
        raise Refusal("fixture source authority is absent")
    ledger = Ledger(fixture)
    snapshots: list[dict[str, Any]] = [{"stage": "initial", "snapshot": ledger.snapshot()}]
    stages = fixture["stages"]
    if not isinstance(stages, list) or not stages:
        raise Refusal("fixture stages are absent")
    seen: set[str] = set()
    for raw_stage in stages:
        stage = exact_keys(raw_stage, {"id", "events"}, "fixture stage")
        stage_id = name(stage["id"], "stage id")
        if stage_id in seen or stage_id == "initial":
            raise Refusal("fixture stage repeats")
        seen.add(stage_id)
        events = stage["events"]
        if not isinstance(events, list):
            raise Refusal("fixture stage events are not an array")
        for event in events:
            ledger.apply(event, stage_id)
        snapshots.append({"stage": stage_id, "snapshot": ledger.snapshot()})
    if not ledger.retired:
        raise Refusal("fixture does not reach aggregate retirement")
    return {
        "schema": OUTPUT_SCHEMA,
        "fixtureId": fixture_id,
        "fixtureSha256": hashlib.sha256(canonical_bytes(fixture)).hexdigest(),
        "sourceAuthority": fixture["sourceAuthority"],
        "stageSnapshots": snapshots,
        "directQuotes": ledger.direct_quotes,
        "collateralSpendEnvelopes": ledger.collateral_envelopes(),
        "lamportContract": authenticate_lamport_contract(fixture["lamportContract"]),
        "activityV3Authority": authenticate_activity_v3_authority(
            fixture["activityV3Authority"]
        ),
    }


def authenticate_activity_v3_authority(value: Any) -> dict[str, Any] | None:
    if value is None:
        return None
    authority = exact_keys(
        value,
        {
            "clusterTarget", "payerWallet", "wallets", "authorization",
            "allLifecycleMutationsExpected", "feeBasisPointsPerSide",
            "feeDenominator",
        },
        "Activity-v3 authority",
    )
    if authority["clusterTarget"] != "devnet":
        raise Refusal("Activity-v3 economic authority is devnet-only")
    payer = name(authority["payerWallet"], "Activity-v3 payer")
    rows = authority["wallets"]
    if not isinstance(rows, list) or len(rows) != 10:
        raise Refusal("corrected Activity-v3 authority must contain exactly ten wallets")
    wallets: list[dict[str, Any]] = []
    ids: set[str] = set()
    roles: set[str] = set()
    participant_post_init = 0
    payer_initial = None
    fresh_roles = {
        "collateral-mint",
        "collateral-wallet",
        "founding-beneficiary",
        "founding-projection-witness",
        "founding-source-funder",
    }
    participant_ids = {"ash", "birch", "cobalt", "dahlia"}
    for raw in rows:
        row = exact_keys(
            raw,
            {"id", "role", "initialFundingLamports", "postInitFundingLamports"},
            "Activity-v3 wallet",
        )
        wallet = name(row["id"], "Activity-v3 wallet id")
        role = name(row["role"], "Activity-v3 wallet role")
        initial = decimal(row["initialFundingLamports"], f"{wallet} initial funding")
        post_init = decimal(row["postInitFundingLamports"], f"{wallet} post-init funding")
        if wallet in ids or role in roles:
            raise Refusal("Activity-v3 wallets repeat an id or exact role")
        ids.add(wallet)
        roles.add(role)
        if wallet == payer:
            if role != "campaign-payer" or post_init != 0:
                raise Refusal("Activity-v3 payer role or funding phase changed")
            payer_initial = initial
        elif role in fresh_roles:
            if initial != 0 or post_init != 0:
                raise Refusal("Activity-v3 fresh signer role was prefunded")
        elif wallet in participant_ids and role == f"participant-{wallet}":
            if initial != 0 or post_init != 50_000_000:
                raise Refusal("Activity-v3 participant funding changed")
            participant_post_init = checked_add(
                participant_post_init, post_init, "post-init funding total"
            )
        else:
            raise Refusal("Activity-v3 wallet is outside the exact role partition")
        wallets.append(
            {
                "id": wallet,
                "role": role,
                "initialFundingLamports": str(initial),
                "postInitFundingLamports": str(post_init),
            }
        )
    if roles != {"campaign-payer", *fresh_roles, *(f"participant-{item}" for item in participant_ids)}:
        raise Refusal("Activity-v3 ten-wallet role partition changed")
    authorization = exact_keys(
        authority["authorization"],
        {
            "initialFundingLamports", "maxPostInitTransferLamports",
            "maxPostInitFeeLamports", "maxFeeLamports", "maxSpendLamports",
            "guaranteedPreLifecycleResidualLamports",
        },
        "Activity-v3 authorization",
    )
    values = {
        key: decimal(raw, f"Activity-v3 {key}")
        for key, raw in authorization.items()
    }
    if (
        payer_initial != values["initialFundingLamports"]
        or payer_initial != 360_000_000
        or participant_post_init != 200_000_000
        or values["maxPostInitTransferLamports"] != participant_post_init
        or values["maxPostInitFeeLamports"] != 10_000_000
        or values["maxFeeLamports"] != 10_000_000
        or values["maxSpendLamports"]
        != participant_post_init + values["maxPostInitFeeLamports"]
        or values["maxSpendLamports"] > payer_initial
        or values["guaranteedPreLifecycleResidualLamports"]
        != payer_initial - values["maxSpendLamports"]
        or values["guaranteedPreLifecycleResidualLamports"] != 150_000_000
    ):
        raise Refusal("Activity-v3 spend cap and residual arithmetic changed")
    if authority["allLifecycleMutationsExpected"] is not True:
        raise Refusal("Activity-v3 authority retains a nonmutating lifecycle gap")
    if authority["feeBasisPointsPerSide"] != 50:
        raise Refusal("Activity-v3 per-side fee changed")
    if decimal(authority["feeDenominator"], "Activity-v3 fee denominator", positive=True) != 10_000:
        raise Refusal("Activity-v3 fee denominator changed")
    return {
        "clusterTarget": "devnet",
        "payerWallet": payer,
        "wallets": wallets,
        "authorization": {key: str(values[key]) for key in authorization},
        "allLifecycleMutationsExpected": True,
        "feeBasisPointsPerSide": 50,
        "feeDenominator": "10000",
        "classification": "devnet-only-authorized-spend-not-mainnet-evidence",
    }


def authenticate_activity_v3_scenario(value: Any, authority_value: Any) -> dict[str, Any]:
    """Join a scenario to the corrected authority; reject the old projection."""

    authority = authenticate_activity_v3_authority(authority_value)
    if authority is None:
        raise Refusal("Activity-v3 scenario check requires a devnet authority")
    if not isinstance(value, dict) or value.get("schema") != "dclutch-devnet-economic-scenario-v1":
        raise Refusal("Activity-v3 scenario schema changed")
    body = value.get("body")
    if not isinstance(body, dict):
        raise Refusal("Activity-v3 scenario body is absent")
    if body.get("evidenceLevel") == "scenario-only":
        raise Refusal("old flagship remains scenario-only, not authenticated Activity-v3 authority")
    if body.get("clusterTarget") != "devnet":
        raise Refusal("Activity-v3 scenario is not exact devnet")
    operations = body.get("operations")
    if not isinstance(operations, list) or not operations:
        raise Refusal("Activity-v3 operations are absent")
    for order, operation in enumerate(operations):
        if (
            not isinstance(operation, dict)
            or operation.get("order") != order
            or operation.get("mutationExpected") is not True
        ):
            raise Refusal("Activity-v3 scenario retains a nonmutating or unordered lifecycle gap")
        predecessor = None if order == 0 else operations[order - 1].get("id")
        if operation.get("predecessorId") != predecessor:
            raise Refusal("Activity-v3 predecessor chain changed")
    market = body.get("market")
    if (
        not isinstance(market, dict)
        or market.get("feeBasisPointsPerSide") != 50
        or decimal(market.get("feeDenominator"), "scenario fee denominator", positive=True)
        != 10_000
    ):
        raise Refusal("Activity-v3 scenario fee policy changed")
    scenario_wallets = body.get("wallets")
    if not isinstance(scenario_wallets, list):
        raise Refusal("Activity-v3 scenario wallets are absent")
    by_id = {
        row.get("id"): row for row in scenario_wallets if isinstance(row, dict)
    }
    if len(by_id) != len(scenario_wallets) or set(by_id) != {
        row["id"] for row in authority["wallets"]
    }:
        raise Refusal("Activity-v3 scenario wallet partition changed")
    for expected in authority["wallets"]:
        row = by_id[expected["id"]]
        roles = row.get("roles")
        expected_role = expected["role"]
        role = "participant" if expected_role.startswith("participant-") else expected_role
        if not isinstance(roles, list) or role not in roles:
            raise Refusal("Activity-v3 scenario wallet role changed")
        expected_funding = (
            expected["initialFundingLamports"]
            if expected["id"] == authority["payerWallet"]
            else expected["postInitFundingLamports"]
        )
        if row.get("fundingLamports") != expected_funding:
            raise Refusal("Activity-v3 scenario wallet funding changed")
    return {
        "status": "accepted",
        "scenarioId": body.get("scenarioId"),
        "walletCount": len(scenario_wallets),
        "operationCount": len(operations),
        "classification": authority["classification"],
    }


def authenticate_lamport_contract(value: Any) -> dict[str, Any]:
    contract = exact_keys(
        value,
        {"requiredFundingTransfers", "aggregateRefundClasses", "notes"},
        "lamport contract",
    )
    transfers = contract["requiredFundingTransfers"]
    if not isinstance(transfers, list):
        raise Refusal("required funding transfers are not an array")
    canonical_transfers = []
    for raw in transfers:
        row = exact_keys(raw, {"source", "destination", "lamports"}, "funding transfer")
        canonical_transfers.append(
            {
                "source": name(row["source"], "funding source"),
                "destination": name(row["destination"], "funding destination"),
                "lamports": str(decimal(row["lamports"], "funding principal", positive=True)),
            }
        )
    classes = contract["aggregateRefundClasses"]
    if not isinstance(classes, list) or len(classes) != len(set(classes)):
        raise Refusal("aggregate refund classes are not one unique array")
    canonical_classes = [name(item, "aggregate refund class") for item in classes]
    if not isinstance(contract["notes"], list) or not all(
        isinstance(item, str) and item for item in contract["notes"]
    ):
        raise Refusal("lamport contract notes changed")
    return {
        "requiredFundingTransfers": canonical_transfers,
        "aggregateRefundClasses": canonical_classes,
        "notes": contract["notes"],
        "exactWalletSpendFormula": (
            "peak_prefix(sum(outgoing transfer principal + rent locks + network fees "
            "- incoming transfers - exact rent refunds))"
        ),
        "terminalRefundFormula": (
            "refundWalletBefore + sum(aggregateRefundClasses) "
            "- aggregateTransactionFeesIfRefundWalletIsPayer"
        ),
    }


def derive_lamport_trace(contract_value: Any, trace_value: Any) -> dict[str, Any]:
    """Validate exact rent/fee/refund ownership and derive per-wallet envelopes."""

    contract = authenticate_lamport_contract(contract_value)
    trace = exact_keys(trace_value, {"schema", "events"}, "lamport trace")
    if trace["schema"] != LAMPORT_TRACE_SCHEMA or not isinstance(trace["events"], list):
        raise Refusal("lamport trace schema or events changed")
    delta: dict[str, int] = {}
    prefix: dict[str, int] = {}
    minimum: dict[str, int] = {}
    debits: dict[str, int] = {}
    credits: dict[str, int] = {}
    fees: dict[str, int] = {}
    locked: dict[str, tuple[str, str, int]] = {}
    refunded_classes: dict[str, int] = {}
    transfers: list[dict[str, str]] = []

    def apply(wallet: str, amount: int) -> None:
        delta[wallet] = delta.get(wallet, 0) + amount
        prefix[wallet] = prefix.get(wallet, 0) + amount
        minimum[wallet] = min(minimum.get(wallet, 0), prefix[wallet])
        if amount < 0:
            debits[wallet] = debits.get(wallet, 0) - amount
        else:
            credits[wallet] = credits.get(wallet, 0) + amount

    for raw in trace["events"]:
        if not isinstance(raw, dict):
            raise Refusal("lamport event is not an object")
        kind = raw.get("kind")
        if kind == "transfer":
            row = exact_keys(raw, {"kind", "stage", "source", "destination", "lamports"}, "lamport transfer")
            source = name(row["source"], "lamport transfer source")
            destination = name(row["destination"], "lamport transfer destination")
            amount = decimal(row["lamports"], "lamport transfer principal", positive=True)
            if source == destination:
                raise Refusal("lamport transfer aliases source and destination")
            apply(source, -amount)
            apply(destination, amount)
            transfers.append({"source": source, "destination": destination, "lamports": str(amount)})
        elif kind == "network-fee":
            row = exact_keys(raw, {"kind", "stage", "payer", "lamports"}, "network fee")
            payer = name(row["payer"], "network fee payer")
            amount = decimal(row["lamports"], "network fee")
            apply(payer, -amount)
            fees[payer] = checked_add(fees.get(payer, 0), amount, "wallet fees")
        elif kind == "rent-lock":
            row = exact_keys(raw, {"kind", "stage", "payer", "account", "class", "lamports"}, "rent lock")
            payer = name(row["payer"], "rent payer")
            account = name(row["account"], "rent account")
            refund_class = name(row["class"], "rent class")
            amount = decimal(row["lamports"], "rent lock", positive=True)
            if account in locked:
                raise Refusal("rent account repeats")
            locked[account] = (payer, refund_class, amount)
            apply(payer, -amount)
        elif kind == "rent-refund":
            row = exact_keys(raw, {"kind", "stage", "recipient", "account", "class", "lamports"}, "rent refund")
            recipient = name(row["recipient"], "rent refund recipient")
            account = name(row["account"], "rent refund account")
            refund_class = name(row["class"], "rent refund class")
            amount = decimal(row["lamports"], "rent refund", positive=True)
            if locked.get(account) is None or locked[account][1:] != (refund_class, amount):
                raise Refusal("rent refund differs from the exact locked account/class/lamports")
            del locked[account]
            apply(recipient, amount)
            refunded_classes[refund_class] = checked_add(
                refunded_classes.get(refund_class, 0), amount, "classified refund"
            )
        else:
            raise Refusal(f"unknown lamport event kind {kind!r}")

    required = contract["requiredFundingTransfers"]
    for row in required:
        if transfers.count(row) != 1:
            raise Refusal("lamport trace omits or repeats required funding principal")
    required_classes = contract["aggregateRefundClasses"]
    if required_classes and set(refunded_classes) != set(required_classes):
        raise Refusal("aggregate retirement refund classification changed")
    envelopes = {
        wallet: {
            "grossDebitLamports": str(debits.get(wallet, 0)),
            "grossCreditLamports": str(credits.get(wallet, 0)),
            "netDeltaLamports": str(delta.get(wallet, 0)),
            "networkFeeLamports": str(fees.get(wallet, 0)),
            "minimumStartingLamportsForThisOrdering": str(-minimum.get(wallet, 0)),
        }
        for wallet in sorted(delta)
    }
    return {
        "walletEnvelopes": envelopes,
        "liveRefundableRentLamports": str(sum(row[2] for row in locked.values())),
        "classifiedRefundLamports": {
            item: str(refunded_classes[item]) for item in sorted(refunded_classes)
        },
        "totalNetworkFeesLamports": str(sum(fees.values())),
        "conservation": {
            "sumWalletDeltaPlusLiveRentPlusFees": str(
                sum(delta.values()) + sum(row[2] for row in locked.values()) + sum(fees.values())
            ),
            "holds": sum(delta.values()) + sum(row[2] for row in locked.values()) + sum(fees.values()) == 0,
        },
    }


def stage_snapshot(derived: Mapping[str, Any], stage: str) -> dict[str, Any]:
    for row in derived["stageSnapshots"]:
        if row["stage"] == stage:
            return row["snapshot"]
    raise Refusal(f"unknown lifecycle stage {stage}")


def check_observed(derived: Mapping[str, Any], value: Any) -> None:
    observed = exact_keys(value, {"schema", "fixtureSha256", "stage", "snapshot"}, "observed snapshot")
    if observed["schema"] != OBSERVED_SCHEMA:
        raise Refusal("observed snapshot schema changed")
    if observed["fixtureSha256"] != derived["fixtureSha256"]:
        raise Refusal("observed snapshot names another fixture")
    expected = stage_snapshot(derived, name(observed["stage"], "observed stage"))
    if observed["snapshot"] != expected:
        raise Refusal("observed economic snapshot differs from the source model")


def read_json(path: Path, label: str) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Refusal(f"cannot read exact {label}: {error}") from error


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    derive = subparsers.add_parser("derive")
    derive.add_argument("fixture", type=Path)
    check = subparsers.add_parser("check")
    check.add_argument("fixture", type=Path)
    check.add_argument("observed", type=Path)
    lamports = subparsers.add_parser("check-lamports")
    lamports.add_argument("fixture", type=Path)
    lamports.add_argument("trace", type=Path)
    scenario = subparsers.add_parser("check-activity-v3-scenario")
    scenario.add_argument("fixture", type=Path)
    scenario.add_argument("scenario", type=Path)
    args = parser.parse_args(argv)
    fixture = read_json(args.fixture, "fixture")
    derived = derive_fixture(fixture)
    if args.command == "check":
        check_observed(derived, read_json(args.observed, "observed snapshot"))
        result: Any = {"status": "accepted", "stage": read_json(args.observed, "observed snapshot")["stage"]}
    elif args.command == "check-lamports":
        result = derive_lamport_trace(
            fixture["lamportContract"], read_json(args.trace, "lamport trace")
        )
    elif args.command == "check-activity-v3-scenario":
        result = authenticate_activity_v3_scenario(
            read_json(args.scenario, "Activity-v3 scenario"),
            fixture["activityV3Authority"],
        )
    else:
        result = derived
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Refusal as error:
        raise SystemExit(f"refusal: {error}") from error
