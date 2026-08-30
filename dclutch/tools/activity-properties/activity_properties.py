#!/usr/bin/env python3
"""Executable whole-lifecycle properties over authenticated activity dossiers.

The reconciler remains the semantic owner of the dossier and its protocol
facts.  This module consumes that exact projection and proves cross-operation
properties which do not belong to any one operation adapter.
"""

from __future__ import annotations

import argparse
import importlib.util
import pathlib
import sys
from collections import defaultdict
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
RECONCILER_PATH = ROOT / "tools" / "devnet-reconcile" / "reconcile.py"
REPORT_SCHEMA = "dclutch-activity-lifecycle-property-report-v1"


def _load_reconciler() -> Any:
    spec = importlib.util.spec_from_file_location("dclutch_devnet_reconcile", RECONCILER_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load reconciler from {RECONCILER_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


reconcile = _load_reconciler()
Refusal = reconcile.Refusal
refuse = reconcile.refuse
DOSSIER_SCHEMA = reconcile.OWNED_LOOPBACK_DOSSIER_SCHEMA
PHASES = reconcile.EVENT_KINDS


def _signed_decimal(value: Any, label: str) -> int:
    return reconcile.decimal(value, label, signed=True)


def _account_maps(dossier: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], dict[str, str]]:
    rows = dossier["accounts"]
    if not isinstance(rows, list) or not rows or len(rows) > reconcile.MAX_ACCOUNTS:
        refuse("dossier accounts must be a nonempty bounded array")
    by_ref: dict[str, dict[str, Any]] = {}
    by_address: dict[str, str] = {}
    for index, row in enumerate(rows):
        row = reconcile.exact_keys(
            row,
            {"ref", "address", "kind", "role"},
            {"mint", "assetClass", "authority", "programOwner"},
            f"dossier accounts[{index}]",
        )
        ref = reconcile.text(row["ref"], f"dossier accounts[{index}] ref")
        address = reconcile.pubkey(row["address"], f"dossier account {ref} address")
        if ref in by_ref or address in by_address:
            refuse("dossier account inventory aliases a ref or address")
        if row["kind"] not in ("wallet", "token", "position", "certificate", "protocol"):
            refuse(f"dossier account {ref} has an unknown kind")
        if row["kind"] == "token":
            if set(row) != {
                "ref", "address", "kind", "role", "mint", "assetClass", "authority", "programOwner",
            }:
                refuse(f"dossier token account {ref} omits its exact typed identity")
            reconcile.pubkey(row["mint"], f"dossier token {ref} mint")
            reconcile.pubkey(row["authority"], f"dossier token {ref} authority")
            reconcile.pubkey(row["programOwner"], f"dossier token {ref} program owner")
            if row["assetClass"] not in ("collateral", "claim"):
                refuse(f"dossier token account {ref} has an unknown asset class")
        elif set(row) != {"ref", "address", "kind", "role"}:
            refuse(f"dossier non-token account {ref} carries token-only identity")
        by_ref[ref] = row
        by_address[address] = ref
    return by_ref, by_address


def _delta_rows(
    rows: Any,
    *,
    accounts: dict[str, dict[str, Any]],
    amount_field: str,
    label: str,
) -> dict[str, int]:
    if not isinstance(rows, list):
        refuse(f"{label} must be an array")
    out: dict[str, int] = {}
    for index, row in enumerate(rows):
        row = reconcile.exact_keys(row, {"account", amount_field}, set(), f"{label}[{index}]")
        ref = reconcile.text(row["account"], f"{label}[{index}] account")
        if ref not in accounts or ref in out:
            refuse(f"{label} contains an unknown or duplicate account")
        out[ref] = _signed_decimal(row[amount_field], f"{label}[{index}] {amount_field}")
    return out


def _state(value: Any, label: str) -> dict[str, Any]:
    value = reconcile.exact_keys(
        value,
        {"claimCount", "revision", "aggregateHex", "ownerHex", "basisHex", "balancesAtoms"},
        set(),
        label,
    )
    claim_count = reconcile.decimal(value["claimCount"], f"{label} claimCount")
    reconcile.decimal(value["revision"], f"{label} revision")
    for field in ("aggregateHex", "ownerHex", "basisHex"):
        reconcile.text(value[field], f"{label} {field}")
    balances = value["balancesAtoms"]
    if not isinstance(balances, list) or len(balances) != claim_count:
        refuse(f"{label} balances do not match claimCount")
    for index, balance in enumerate(balances):
        reconcile.decimal(balance, f"{label} balancesAtoms[{index}]")
    return value


def _observation_rows(
    event: dict[str, Any],
    accounts: dict[str, dict[str, Any]],
    *,
    token: bool,
) -> dict[str, dict[str, Any]]:
    field = "tokenObservations" if token else "lamportObservations"
    rows = event[field]
    if not isinstance(rows, list):
        refuse(f"event {event['id']} {field} must be an array")
    out: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        if token:
            row = reconcile.exact_keys(
                row,
                {"account", "mint", "owner", "beforeAtoms", "afterAtoms", "deltaAtoms"},
                set(),
                f"event {event['id']} {field}[{index}]",
            )
            before_field, after_field, delta_field = "beforeAtoms", "afterAtoms", "deltaAtoms"
            reconcile.pubkey(row["mint"], f"event {event['id']} token mint")
            reconcile.pubkey(row["owner"], f"event {event['id']} token owner")
        else:
            row = reconcile.exact_keys(
                row,
                {"account", "beforeLamports", "afterLamports", "deltaLamports"},
                set(),
                f"event {event['id']} {field}[{index}]",
            )
            before_field, after_field, delta_field = "beforeLamports", "afterLamports", "deltaLamports"
        ref = reconcile.text(row["account"], f"event {event['id']} {field} account")
        if ref not in accounts or ref in out:
            refuse(f"event {event['id']} {field} contains an unknown or duplicate account")
        before = reconcile.decimal(row[before_field], f"event {event['id']} {before_field}")
        after = reconcile.decimal(row[after_field], f"event {event['id']} {after_field}")
        delta = _signed_decimal(row[delta_field], f"event {event['id']} {delta_field}")
        if after - before != delta:
            refuse(f"event {event['id']} {field} does not bind its exact transition")
        out[ref] = row
    return out


def _position_transition(
    row: Any,
    *,
    accounts: dict[str, dict[str, Any]],
    label: str,
    role: str | None,
) -> dict[str, Any]:
    required = {"account", "pre", "post"}
    if role is not None:
        required |= {"role", "owner"}
    row = reconcile.exact_keys(row, required, set(), label)
    ref = reconcile.text(row["account"], f"{label} account")
    if ref not in accounts or accounts[ref]["kind"] != "position":
        refuse(f"{label} references a non-Position account")
    if role is not None:
        if row["role"] != role:
            refuse(f"{label} is out of canonical role order")
        reconcile.pubkey(row["owner"], f"{label} owner")
    pre = _state(row["pre"], f"{label} pre")
    post = _state(row["post"], f"{label} post")
    if any(pre[field] != post[field] for field in ("claimCount", "aggregateHex", "ownerHex", "basisHex")):
        refuse(f"{label} substitutes Position identity or geometry")
    if int(post["revision"]) != int(pre["revision"]) + 1:
        refuse(f"{label} is not the exact next Position revision")
    return row


def _validate_direct(
    event: dict[str, Any],
    accounts: dict[str, dict[str, Any]],
    token_deltas: dict[str, int],
) -> tuple[int, int, list[dict[str, Any]]]:
    direct = reconcile.exact_keys(
        event.get("direct"),
        {
            "fillAtoms", "executionPrice", "priceScale", "feeBasisPointsPerSide",
            "sellerToken", "buyerToken", "feeRecipientToken", "mint",
            "grossAtoms", "sellerFeeAtoms", "buyerFeeAtoms", "feeRecipientAtoms",
        },
        set(),
        f"event {event['id']} Direct facts",
    )
    fill = reconcile.decimal(direct["fillAtoms"], "Direct fillAtoms")
    price = reconcile.decimal(direct["executionPrice"], "Direct executionPrice")
    scale = reconcile.decimal(direct["priceScale"], "Direct priceScale")
    bps = reconcile.decimal(direct["feeBasisPointsPerSide"], "Direct feeBasisPointsPerSide")
    if fill == 0 or scale == 0 or bps != 50:
        refuse("Direct fill/scale or frozen 50-bps-per-side policy is invalid")
    product = fill * price
    if product % scale:
        refuse("Direct gross quote crosses an unnamed rounding boundary")
    gross = product // scale
    seller_fee = gross * bps // 10_000
    buyer_fee = gross * bps // 10_000
    fee_total = seller_fee + buyer_fee
    for field, expected in (
        ("grossAtoms", gross), ("sellerFeeAtoms", seller_fee),
        ("buyerFeeAtoms", buyer_fee), ("feeRecipientAtoms", fee_total),
    ):
        if reconcile.decimal(direct[field], f"Direct {field}") != expected:
            refuse(f"Direct {field} differs from exact scaled-integer arithmetic")
    refs = (direct["sellerToken"], direct["buyerToken"], direct["feeRecipientToken"])
    if len(set(refs)) != 3:
        refuse("Direct token roles alias")
    mint = reconcile.pubkey(direct["mint"], "Direct mint")
    if any(ref not in accounts or accounts[ref].get("mint") != mint for ref in refs):
        refuse("Direct token roles cross or omit the collateral mint")
    expected_deltas = (gross - seller_fee, -(gross + buyer_fee), fee_total)
    if tuple(token_deltas.get(ref) for ref in refs) != expected_deltas:
        refuse("Direct token movements differ from exact gross and side-floor fees")
    positions = event.get("positions")
    if not isinstance(positions, list) or len(positions) != 2:
        refuse("whole-lifecycle properties require exact seller and buyer Direct Positions")
    projected = [
        _position_transition(positions[index], accounts=accounts, label=f"Direct {role} Position", role=role)
        for index, role in enumerate(("seller", "buyer"))
    ]
    seller, buyer = projected
    if any(seller["pre"][field] != buyer["pre"][field] for field in ("claimCount", "aggregateHex", "basisHex")):
        refuse("Direct Position geometry differs across counterparties")
    seller_delta = [int(after) - int(before) for before, after in zip(seller["pre"]["balancesAtoms"], seller["post"]["balancesAtoms"], strict=True)]
    buyer_delta = [int(after) - int(before) for before, after in zip(buyer["pre"]["balancesAtoms"], buyer["post"]["balancesAtoms"], strict=True)]
    changed = [index for index, pair in enumerate(zip(seller_delta, buyer_delta, strict=True)) if pair != (0, 0)]
    if len(changed) != 1 or seller_delta[changed[0]] != -fill or buyer_delta[changed[0]] != fill:
        refuse("Direct Position transfers do not conserve the exact fill")
    return fee_total, fill, projected


def _validate_payout(
    event: dict[str, Any],
    accounts: dict[str, dict[str, Any]],
    token_deltas: dict[str, int],
) -> tuple[int, dict[str, Any]]:
    payout = reconcile.exact_keys(
        event.get("payout"),
        {"hoardToken", "recipientToken", "position", "principalAtoms", "claimsBurnedAtoms", "mint", "principalClass"},
        set(),
        f"event {event['id']} payout facts",
    )
    if payout["principalClass"] != "hoard-principal-not-fee":
        refuse("payout misclassifies Hoard principal")
    principal = reconcile.decimal(payout["principalAtoms"], "payout principalAtoms")
    hoard, recipient = payout["hoardToken"], payout["recipientToken"]
    if hoard == recipient or hoard not in accounts or recipient not in accounts:
        refuse("payout token roles alias or are absent")
    mint = reconcile.pubkey(payout["mint"], "payout mint")
    if any(accounts[ref].get("mint") != mint for ref in (hoard, recipient)):
        refuse("payout token roles cross the collateral mint")
    if token_deltas.get(hoard) != -principal or token_deltas.get(recipient) != principal:
        refuse("payout does not conserve exact Hoard principal")
    position = _position_transition(event.get("position"), accounts=accounts, label="payout Position", role=None)
    if position["account"] != payout["position"]:
        refuse("payout Position reference differs from its exact transition")
    burns = payout["claimsBurnedAtoms"]
    if not isinstance(burns, list):
        refuse("payout claimsBurnedAtoms must be an array")
    burn_values = [reconcile.decimal(value, f"payout claimsBurnedAtoms[{index}]") for index, value in enumerate(burns)]
    before = [int(value) for value in position["pre"]["balancesAtoms"]]
    after = [int(value) for value in position["post"]["balancesAtoms"]]
    if len(burn_values) != len(before) or any(left - right != burn for left, right, burn in zip(before, after, burn_values, strict=True)):
        refuse("payout claim burns differ from the exact Position debit")
    return principal, position


def validate_lifecycle(dossier: Any) -> dict[str, Any]:
    dossier = reconcile.exact_keys(
        dossier,
        {"schema", "signatureScheme", "activityId", "cluster", "evidence", "accounts", "events", "finalAccounts", "totals", "dossierSha256"},
        set(),
        "activity dossier",
    )
    if dossier["schema"] != DOSSIER_SCHEMA or dossier["signatureScheme"] != "none":
        refuse("activity dossier schema or signature classification is not admitted")
    activity_id = reconcile.text(dossier["activityId"], "activity dossier activityId")
    cluster = reconcile.exact_keys(dossier["cluster"], {"kind", "genesisHash"}, set(), "activity dossier cluster")
    if cluster["kind"] != "owned-loopback":
        refuse("activity dossier schema crosses its cluster evidence class")
    genesis = reconcile.pubkey(cluster["genesisHash"], "activity dossier genesisHash")
    claimed_digest = reconcile.digest(dossier["dossierSha256"], "activity dossier digest")
    core = {key: value for key, value in dossier.items() if key != "dossierSha256"}
    if reconcile.sha256_bytes(reconcile.canonical_bytes(core)) != claimed_digest:
        refuse("activity dossier self digest differs from exact bytes")
    accounts, _ = _account_maps(dossier)
    events = dossier["events"]
    if not isinstance(events, list) or not events or len(events) > reconcile.MAX_EVENTS:
        refuse("activity dossier events must be a nonempty bounded array")
    evidence = reconcile.exact_keys(
        dossier["evidence"],
        {"manifestSha256", "sourceDigests", "rpc"},
        {"ownedLoopbackSession"},
        "activity dossier evidence",
    )
    reconcile.digest(evidence["manifestSha256"], "activity dossier manifestSha256")
    if not isinstance(evidence["rpc"], dict) or not evidence["rpc"]:
        refuse("activity dossier RPC provenance must be a nonempty typed object")
    if "ownedLoopbackSession" in evidence and not isinstance(evidence["ownedLoopbackSession"], dict):
        refuse("owned-loopback session evidence must be an object")
    seen_ids: set[str] = set()
    seen_signatures: set[str] = set()
    seen_phases: set[str] = set()
    prior_phase = -1
    prior_slot = -1
    predecessor: str | None = None
    fee_total = 0
    compute_total = 0
    protocol_fee_total = 0
    principal_total = 0
    closed_rent_total = 0
    refund_total = 0
    direct_sources: set[str] = set()
    payer_addresses: set[str] = set()
    positions: list[dict[str, Any]] = []
    signatures: list[str] = []
    token_net_after_founding: dict[str, int] = defaultdict(int)
    closed_refs: set[str] = set()
    last_lamports: dict[str, int] = {}
    last_tokens: dict[str, int] = {}
    last_positions: dict[str, dict[str, Any]] = {}
    final_rows = dossier["finalAccounts"]
    if not isinstance(final_rows, list):
        refuse("activity dossier finalAccounts must be an array")
    final_by_ref: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(final_rows):
        row = reconcile.exact_keys(
            row,
            {"account", "address", "closed", "observedSlot"},
            {"owner", "lamports", "dataSha256", "mint", "authority", "amountAtoms", "position", "certificate"},
            f"finalAccounts[{index}]",
        )
        ref = reconcile.text(row["account"], f"finalAccounts[{index}] account")
        if ref not in accounts or ref in final_by_ref or row["address"] != accounts[ref]["address"] or not isinstance(row["closed"], bool):
            refuse("finalAccounts substitutes, duplicates, or aliases an account")
        reconcile.decimal(row["observedSlot"], f"finalAccounts[{index}] observedSlot")
        if row["closed"]:
            if set(row) != {"account", "address", "closed", "observedSlot"}:
                refuse(f"closed final account {ref} carries live-account facts")
        else:
            base_fields = {"account", "address", "closed", "observedSlot", "owner", "lamports", "dataSha256"}
            for field in ("owner", "lamports", "dataSha256"):
                if field not in row:
                    refuse(f"live final account {ref} omitted {field}")
            reconcile.pubkey(row["owner"], f"final account {ref} owner")
            reconcile.decimal(row["lamports"], f"final account {ref} lamports")
            reconcile.digest(row["dataSha256"], f"final account {ref} dataSha256")
            if accounts[ref]["kind"] == "token":
                if set(row) != base_fields | {"mint", "authority", "amountAtoms"}:
                    refuse(f"final token account {ref} has a noncanonical projection")
                if not {"mint", "authority", "amountAtoms"}.issubset(row):
                    refuse(f"final token account {ref} omitted exact token state")
                if row["mint"] != accounts[ref]["mint"] or row["authority"] != accounts[ref]["authority"]:
                    refuse(f"final token account {ref} substitutes mint or authority")
                reconcile.decimal(row["amountAtoms"], f"final token account {ref} amountAtoms")
            elif accounts[ref]["kind"] == "position":
                if set(row) != base_fields | {"position"}:
                    refuse(f"final Position {ref} has a noncanonical projection")
                if "position" not in row:
                    refuse(f"final Position {ref} omitted decoded state")
                _state(row["position"], f"final Position {ref}")
            elif accounts[ref]["kind"] == "certificate":
                if set(row) != base_fields | {"certificate"} or not isinstance(row["certificate"], dict):
                    refuse(f"final certificate {ref} has a noncanonical projection")
            elif set(row) != base_fields:
                refuse(f"final account {ref} has facts outside its declared kind")
        final_by_ref[ref] = row
    for index, event in enumerate(events):
        event = reconcile.exact_keys(
            event,
            {
                "id", "kind", "operation", "predecessor", "signature", "slot", "feePayer",
                "transactionFeeLamports", "computeUnitsConsumed", "lamportDeltas", "tokenDeltas", "sourceSha256",
                "lamportObservations", "tokenObservations",
            },
            {"direct", "positions", "position", "certificate", "payout", "retirement"},
            f"dossier events[{index}]",
        )
        event_id = reconcile.text(event["id"], f"dossier events[{index}] id")
        reconcile.text(event["operation"], f"event {event_id} operation")
        signature = reconcile.text(event["signature"], f"event {event_id} signature")
        reconcile.digest(event["sourceSha256"], f"event {event_id} sourceSha256")
        if event_id in seen_ids or signature in seen_signatures:
            refuse("activity dossier repeats an event id or transaction signature")
        if event["predecessor"] != predecessor:
            refuse(f"event {event_id} forks or omits its exact predecessor")
        if event["kind"] not in PHASES:
            refuse(f"event {event_id} has an unknown lifecycle phase")
        phase_index = PHASES.index(event["kind"])
        if phase_index < prior_phase:
            refuse("activity dossier moves backward across lifecycle phases")
        slot = reconcile.decimal(event["slot"], f"event {event_id} slot")
        if slot < prior_slot:
            refuse("activity dossier slots regress")
        fee_ref = reconcile.text(event["feePayer"], f"event {event_id} feePayer")
        if fee_ref not in accounts or accounts[fee_ref]["kind"] != "wallet":
            refuse(f"event {event_id} fee payer is not a declared wallet")
        payer_addresses.add(accounts[fee_ref]["address"])
        fee = reconcile.decimal(event["transactionFeeLamports"], f"event {event_id} transaction fee")
        compute = reconcile.decimal(event["computeUnitsConsumed"], f"event {event_id} compute units")
        if compute == 0:
            refuse(f"event {event_id} compute units must be positive")
        lamport_deltas = _delta_rows(event["lamportDeltas"], accounts=accounts, amount_field="lamports", label=f"event {event_id} lamportDeltas")
        token_deltas = _delta_rows(event["tokenDeltas"], accounts=accounts, amount_field="atoms", label=f"event {event_id} tokenDeltas")
        lamport_observations = _observation_rows(event, accounts, token=False)
        token_observations = _observation_rows(event, accounts, token=True)
        if {ref: _signed_decimal(row["deltaLamports"], "lamport observation delta") for ref, row in lamport_observations.items()} != lamport_deltas:
            refuse(f"event {event_id} lamport observations differ from declared deltas")
        if {ref: _signed_decimal(row["deltaAtoms"], "token observation delta") for ref, row in token_observations.items()} != token_deltas:
            refuse(f"event {event_id} token observations differ from declared deltas")
        for ref, row in lamport_observations.items():
            before = int(row["beforeLamports"])
            if ref in last_lamports and last_lamports[ref] != before:
                refuse(f"activity lamport history for {ref} is discontinuous")
            last_lamports[ref] = int(row["afterLamports"])
        for ref, row in token_observations.items():
            before = int(row["beforeAtoms"])
            if ref in last_tokens and last_tokens[ref] != before:
                refuse(f"activity token history for {ref} is discontinuous")
            last_tokens[ref] = int(row["afterAtoms"])
        if sum(lamport_deltas.values()) != -fee:
            refuse(f"event {event_id} leaks or creates lamports beyond its exact transaction fee")
        for ref, delta in token_deltas.items():
            account = accounts[ref]
            if account["kind"] != "token" or token_observations[ref]["mint"] != account["mint"] or token_observations[ref]["owner"] != account["authority"]:
                refuse(f"event {event_id} token transition substitutes its typed account identity")
            if event["kind"] != "founding":
                token_net_after_founding[account["mint"]] += delta
        if event["kind"] == "direct" and "direct" in event:
            protocol_fee, _, direct_positions = _validate_direct(event, accounts, token_deltas)
            protocol_fee_total += protocol_fee
            for row in direct_positions:
                ref = row["account"]
                if ref in last_positions and last_positions[ref] != row["pre"]:
                    refuse(f"activity Position history for {ref} is discontinuous")
                last_positions[ref] = row["post"]
                positions.append({"activityId": activity_id, "eventId": event_id, "slot": slot, **row})
            direct_sources.add(reconcile.digest(event["sourceSha256"], f"event {event_id} sourceSha256"))
        elif "direct" in event or "positions" in event:
            refuse("Direct economics or paired Positions belong only to the Direct phase")
        if event["kind"] == "payout" and "payout" in event:
            principal, payout_position = _validate_payout(event, accounts, token_deltas)
            principal_total += principal
            ref = payout_position["account"]
            if ref in last_positions and last_positions[ref] != payout_position["pre"]:
                refuse(f"activity Position history for {ref} is discontinuous")
            last_positions[ref] = payout_position["post"]
            positions.append({"activityId": activity_id, "eventId": event_id, "slot": slot, "role": "recipient", **payout_position})
        elif "payout" in event or "position" in event:
            refuse("payout facts or singular Position belong only to the payout phase")
        if event["kind"] == "retirement":
            retirement = reconcile.exact_keys(event.get("retirement"), {"stage", "closedAccounts", "refundLamports"}, set(), f"event {event_id} retirement")
            reconcile.text(retirement["stage"], f"event {event_id} retirement stage")
            event_closed = retirement["closedAccounts"]
            if not isinstance(event_closed, list) or len(event_closed) != len(set(event_closed)):
                refuse(f"event {event_id} retirement closure set is not unique")
            event_refunds = _delta_rows(retirement["refundLamports"], accounts=accounts, amount_field="lamports", label=f"event {event_id} refunds")
            if any(amount <= 0 or lamport_deltas.get(ref) != amount for ref, amount in event_refunds.items()):
                refuse(f"event {event_id} retirement refund differs from its exact transaction")
            for ref in event_closed:
                if ref in closed_refs or ref not in accounts or ref not in lamport_observations:
                    refuse(f"event {event_id} closure set is duplicate, unknown, or unobserved")
                observation = lamport_observations[ref]
                before = reconcile.decimal(observation["beforeLamports"], f"event {event_id} closed rent")
                after = reconcile.decimal(observation["afterLamports"], f"event {event_id} closed poststate")
                delta = _signed_decimal(observation["deltaLamports"], f"event {event_id} closed delta")
                if before == 0 or after != 0 or delta != -before:
                    refuse(f"event {event_id} does not bind exact rent removal for closed account {ref}")
                if ref not in final_by_ref or final_by_ref[ref]["closed"] is not True:
                    refuse(f"event {event_id} closure lacks a finalized vacant account")
                closed_refs.add(ref)
                closed_rent_total += before
            refund_total += sum(event_refunds.values())
        elif "retirement" in event:
            refuse("retirement facts belong only to retirement transactions")
        fee_total += fee
        compute_total += compute
        signatures.append(signature)
        seen_ids.add(event_id)
        seen_signatures.add(signature)
        seen_phases.add(event["kind"])
        predecessor = event_id
        prior_phase = phase_index
        prior_slot = slot
    if seen_phases != set(PHASES):
        refuse("activity dossier does not cover the exact whole lifecycle")
    if any(total != 0 for total in token_net_after_founding.values()):
        refuse("post-founding scaled integer assets are not exactly conserved by mint")
    if closed_rent_total != refund_total:
        refuse("retirement rent removed from closed accounts differs from exact refunds")
    if len(direct_sources) != 1:
        refuse("activity dossier lacks one unambiguous Direct semantic-owner digest")
    source_digests = evidence["sourceDigests"]
    expected_sources = [
        {"event": event["id"], "sha256": event["sourceSha256"]}
        for event in events
    ]
    if source_digests != expected_sources:
        refuse("activity dossier source evidence differs from the exact event chain")
    final_closed = {ref for ref, row in final_by_ref.items() if row["closed"]}
    if final_closed != closed_refs:
        refuse("final vacant accounts differ from the exact retirement closure set")
    for ref, value in last_lamports.items():
        if ref in final_by_ref and not final_by_ref[ref]["closed"]:
            if int(final_by_ref[ref]["lamports"]) != value:
                refuse(f"final lamport account {ref} advanced outside the lifecycle")
    for ref, value in last_tokens.items():
        if ref not in final_by_ref or final_by_ref[ref]["closed"]:
            refuse(f"transitioned token account {ref} lacks a live final observation")
        if int(final_by_ref[ref]["amountAtoms"]) != value:
            refuse(f"final token account {ref} advanced outside the lifecycle")
    for ref, value in last_positions.items():
        if ref not in final_by_ref or final_by_ref[ref]["closed"]:
            refuse(f"transitioned Position {ref} lacks a live final observation")
        if final_by_ref[ref].get("position") != value:
            refuse(f"final Position {ref} advanced outside the lifecycle")
    totals = reconcile.exact_keys(
        dossier["totals"],
        {"transactionFeesLamports", "computeUnitsConsumed", "protocolFeesAtoms", "hoardPrincipalPaidAtoms", "hoardPrincipalClassification"},
        set(),
        "activity dossier totals",
    )
    if reconcile.decimal(totals["transactionFeesLamports"], "dossier transaction fees") != fee_total:
        refuse("dossier transaction-fee total differs from exact event fees")
    if reconcile.decimal(totals["computeUnitsConsumed"], "dossier compute units") != compute_total:
        refuse("dossier compute-unit total differs from exact event history")
    if reconcile.decimal(totals["protocolFeesAtoms"], "dossier protocol fees") != protocol_fee_total:
        refuse("dossier protocol-fee total differs from exact Direct side-floor fees")
    if reconcile.decimal(totals["hoardPrincipalPaidAtoms"], "dossier Hoard principal") != principal_total:
        refuse("dossier Hoard-principal total differs from exact payouts")
    if totals["hoardPrincipalClassification"] != "collateral-principal-not-fee-bounty-rent-reserve-or-treasury":
        refuse("dossier misclassifies Hoard principal")
    return {
        "activityId": activity_id,
        "dossierSha256": claimed_digest,
        "cluster": cluster["kind"],
        "genesisHash": genesis,
        "transactionCount": len(events),
        "transactionFeesLamports": fee_total,
        "computeUnitsConsumed": compute_total,
        "protocolFeesAtoms": protocol_fee_total,
        "hoardPrincipalPaidAtoms": principal_total,
        "closedRentLamports": closed_rent_total,
        "refundLamports": refund_total,
        "payerAddresses": sorted(payer_addresses),
        "signatures": signatures,
        "directSourceSha256": next(iter(direct_sources)),
        "positions": positions,
        "accounts": accounts,
        "events": events,
    }


def _cross_activity_observation_chains(results: list[dict[str, Any]], *, token: bool) -> None:
    field = "tokenObservations" if token else "lamportObservations"
    before_field = "beforeAtoms" if token else "beforeLamports"
    after_field = "afterAtoms" if token else "afterLamports"
    chains: dict[str, list[tuple[int, str, int, int]]] = defaultdict(list)
    for result in results:
        for event in result["events"]:
            for row in event[field]:
                address = result["accounts"][row["account"]]["address"]
                chains[address].append((
                    reconcile.decimal(event["slot"], "cross-activity slot"),
                    result["activityId"],
                    reconcile.decimal(row[before_field], f"cross-activity {before_field}"),
                    reconcile.decimal(row[after_field], f"cross-activity {after_field}"),
                ))
    for address, rows in chains.items():
        if len({row[1] for row in rows}) < 2:
            continue
        rows.sort()
        for prior, current in zip(rows, rows[1:]):
            if prior[0] == current[0]:
                refuse(f"shared account {address} has ambiguous same-slot concurrent transitions")
            if prior[3] != current[2]:
                refuse(f"shared account {address} has a crossed or missing concurrent transition")


def validate_many(dossiers: list[Any]) -> dict[str, Any]:
    if not dossiers:
        refuse("at least one activity dossier is required")
    results = [validate_lifecycle(dossier) for dossier in dossiers]
    if len({result["activityId"] for result in results}) != len(results):
        refuse("concurrent activity set repeats an activityId")
    if len({result["dossierSha256"] for result in results}) != len(results):
        refuse("concurrent activity set repeats a dossier")
    if len({result["genesisHash"] for result in results}) != 1 or len({result["cluster"] for result in results}) != 1:
        refuse("concurrent activity set crosses cluster or genesis identity")
    all_signatures = [signature for result in results for signature in result["signatures"]]
    if len(all_signatures) != len(set(all_signatures)):
        refuse("concurrent activity set replays a transaction signature")
    direct_sources = [result["directSourceSha256"] for result in results]
    if len(direct_sources) != len(set(direct_sources)):
        refuse("concurrent activity set replays a Direct semantic-owner journal")
    payer_addresses = [address for result in results for address in result["payerAddresses"]]
    if len(results) > 1 and len(payer_addresses) != len(set(payer_addresses)):
        refuse("multiwallet activity set aliases a disposable fee-payer wallet")
    position_chains: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for result in results:
        for transition in result["positions"]:
            address = result["accounts"][transition["account"]]["address"]
            position_chains[address].append(transition)
    for address, rows in position_chains.items():
        if len({row["activityId"] for row in rows}) < 2:
            continue
        rows.sort(key=lambda row: (int(row["pre"]["revision"]), row["slot"], row["activityId"]))
        pre_revisions = [int(row["pre"]["revision"]) for row in rows]
        if len(pre_revisions) != len(set(pre_revisions)):
            refuse(f"shared Position {address} replays a revision nonce")
        for prior, current in zip(rows, rows[1:]):
            if prior["post"] != current["pre"]:
                refuse(f"shared Position {address} has a crossed or missing revision")
    _cross_activity_observation_chains(results, token=False)
    _cross_activity_observation_chains(results, token=True)
    report = {
        "schema": REPORT_SCHEMA,
        "status": "holds",
        "cluster": {"kind": results[0]["cluster"], "genesisHash": results[0]["genesisHash"]},
        "activities": [
            {
                "activityId": result["activityId"],
                "dossierSha256": result["dossierSha256"],
                "transactionCount": str(result["transactionCount"]),
            }
            for result in results
        ],
        "totals": {
            "transactionFeesLamports": str(sum(result["transactionFeesLamports"] for result in results)),
            "computeUnitsConsumed": str(sum(result["computeUnitsConsumed"] for result in results)),
            "protocolFeesAtoms": str(sum(result["protocolFeesAtoms"] for result in results)),
            "hoardPrincipalPaidAtoms": str(sum(result["hoardPrincipalPaidAtoms"] for result in results)),
            "closedRentLamports": str(sum(result["closedRentLamports"] for result in results)),
            "refundLamports": str(sum(result["refundLamports"] for result in results)),
        },
        "multiwallet": {
            "status": "holds" if len(results) > 1 else "inapplicable",
            "payerAddresses": sorted(payer_addresses),
        },
    }
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check exact conservation and concurrency properties over reconciled activity dossiers."
    )
    parser.add_argument("--dossier", action="append", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        dossiers = [reconcile.load_json(path) for path in args.dossier]
        report = validate_many(dossiers)
    except Refusal as error:
        print(f"REFUSED: {error}", file=sys.stderr)
        return 2
    sys.stdout.buffer.write(reconcile.canonical_bytes(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
