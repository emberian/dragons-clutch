#!/usr/bin/env python3
"""Deterministic offline cost and wire-layout laboratory for Dragon's Clutch.

This module intentionally has no Solana SDK, RPC, signing, or deployment surface. It emits
synthetic transaction byte strings with correct legacy/v0 framing and reports analytical topology
and information lower bounds separately from those local byte measurements.

Every row carries an `arm`. `layout_hypothesis` rows are the original design sketch, retained
unchanged. `abi_landed` rows consume the landed codec in `programs/solana-layout` and the landed
relation bounds in `crates/clutch-batch`; the harness re-derives each landed width from that
file's own field terms rather than quoting a total. `abi_differential` rows carry the delta
between the two arms for every object that exists in both. A landed width is still not a measured
cost: no Dragon SBF program exists, so no arm reports a total CU, heap, stack, or landing figure.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import math
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


SCHEMA = "dragons.cost_lab.matrix.v2"
ROW_SCHEMA = "dragons.cost_lab.row.v2"
CONSTANTS_SCHEMA = "dragons.cost_lab.constants.v2"
ROOT = Path(__file__).resolve().parent
CONSTANTS_PATH = ROOT / "constants.json"
DEFAULT_GOLDEN = ROOT / "golden"
REPO_ROOT = ROOT.parent

ARM_HYPOTHESIS = "layout_hypothesis"
ARM_LANDED = "abi_landed"
ARM_DIFFERENTIAL = "abi_differential"
ARMS = (ARM_HYPOTHESIS, ARM_LANDED, ARM_DIFFERENTIAL)

LANDED_ACCOUNT_ORDER = (
    "realm",
    "profile",
    "market",
    "hoard",
    "position",
    "feed_head",
    "order_page",
    "supply_ledger",
    "terms",
    "price_grid",
    "epoch",
    "candidate_record",
    "final_pot",
    "settlement_receipt",
    "resolution",
    "clear_work",
    "candidate_feed",
)

LANDED_INTENT_ORDER = (
    "create_market",
    "split",
    "merge",
    "materialize",
    "dematerialize",
    "feed_advance",
    "place_order",
    "cancel_order",
    "settle_page",
)


class ModelError(ValueError):
    """Raised when a scenario cannot be represented by the bounded synthetic model."""


def canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def load_constants() -> dict[str, Any]:
    with CONSTANTS_PATH.open("rb") as handle:
        value = json.load(handle)
    if value.get("schema") != CONSTANTS_SCHEMA:
        raise ModelError("unknown constants schema")
    verify_landed_arm(value)
    return value


def verify_landed_arm(constants: dict[str, Any]) -> None:
    """Re-derive every pinned landed width from the codec's own field terms.

    The landed arm exists so no cost conclusion is attributed to a stale layout. A pinned total
    that does not equal the sum of the terms transcribed from `account_len`/`encoded_len` is a
    transcription error, and the harness must refuse rather than publish it.
    """

    landed = constants[ARM_LANDED]
    bounds = landed["bounds"]
    if landed["arm"] != ARM_LANDED or constants["dragon_design_bounds"]["arm"] != ARM_HYPOTHESIS:
        raise ModelError("cost-lab arms are not distinctly named")
    for name in LANDED_ACCOUNT_ORDER:
        account = landed["accounts"][name]
        if sum(account["field_terms"]) != account["bytes"]:
            raise ModelError(f"landed account width does not equal its field terms: {name}")
    if set(landed["accounts"]) != set(LANDED_ACCOUNT_ORDER):
        raise ModelError("landed account family does not match the pinned inventory")
    for name in LANDED_INTENT_ORDER:
        intent = landed["intents"][name]
        if sum(intent["field_terms"]) != intent["bytes"]:
            raise ModelError(f"landed intent width does not equal its field terms: {name}")
        if intent["bytes"] > bounds["max_intent_bytes"]:
            raise ModelError(f"landed intent exceeds MAX_INTENT_BYTES: {name}")
    if set(landed["intents"]) != set(LANDED_INTENT_ORDER):
        raise ModelError("landed intent family does not match the pinned inventory")
    if sum(bounds["order_record_field_terms"]) != bounds["order_record_bytes"]:
        raise ModelError("landed order record width does not equal its field terms")
    if sum(bounds["portfolio_record_field_terms"]) != bounds["portfolio_record_bytes"]:
        raise ModelError("landed portfolio record width does not equal its field terms")
    if sum(bounds["order_slot_field_terms"]) != bounds["order_slot_bytes"]:
        raise ModelError("landed order slot width does not equal its field terms")
    if bounds["order_slot_bytes"] != 1 + bounds["portfolio_record_bytes"]:
        raise ModelError("landed order slot is not a kind byte plus the widest record body")
    if bounds["portfolio_record_bytes"] <= bounds["order_record_bytes"]:
        raise ModelError("landed slot width is not set by the portfolio record")
    if bounds["max_portfolio_orders"] != bounds["relation_max_portfolio_orders"]:
        raise ModelError("landed page-set and relation portfolio caps disagree")
    page = landed["accounts"]["order_page"]["bytes"]
    derived_page = (
        bounds["order_page_header_bytes"]
        + bounds["max_orders_per_page"] * bounds["order_slot_bytes"]
    )
    if derived_page != page:
        raise ModelError("landed order page is not its header plus a dense slot array")
    if bounds["max_epoch_orders"] != bounds["max_orders_per_page"] * bounds["max_order_pages"]:
        raise ModelError("landed epoch book capacity is not the page geometry")
    if bounds["max_epoch_orders"] != bounds["relation_max_orders"]:
        raise ModelError("landed page set and relation book capacity disagree")
    if (
        bounds["relation_max_legs"]
        != bounds["relation_max_orders"]
        + bounds["relation_max_portfolio_orders"] * bounds["max_outcomes"]
    ):
        raise ModelError("landed relation leg capacity is not its order and portfolio bound")
    if (
        bounds["relation_max_slices"]
        != 2 * bounds["relation_max_legs"] + 2 * bounds["max_outcomes"]
    ):
        raise ModelError("landed relation slice capacity is not its leg and outcome bound")
    if bounds["max_outcomes"] != constants["dragon_design_bounds"]["max_v1_outcomes"]:
        raise ModelError("landed outcome bound and V1 policy bound disagree")


def shortvec(value: int) -> bytes:
    """Encode Solana's compact-u16/short-vec length without accepting unbounded values."""

    if not 0 <= value <= 0xFFFF:
        raise ModelError(f"shortvec value outside u16: {value}")
    output = bytearray()
    remaining = value
    while True:
        elem = remaining & 0x7F
        remaining >>= 7
        if remaining:
            elem |= 0x80
        output.append(elem)
        if not remaining:
            return bytes(output)


def shortvec_len(value: int) -> int:
    return len(shortvec(value))


def deterministic_bytes(label: str, size: int) -> bytes:
    if size < 0:
        raise ModelError("negative deterministic byte request")
    output = bytearray()
    counter = 0
    while len(output) < size:
        output.extend(hashlib.sha256(f"{label}:{counter}".encode()).digest())
        counter += 1
    return bytes(output[:size])


@dataclass(frozen=True)
class WireSpec:
    tx_format: str
    total_accounts: int
    writable_accounts: int
    static_accounts_v0: int
    instruction_data: bytes

    def validate(self) -> None:
        if self.tx_format not in {"legacy_inline", "v0_alt"}:
            raise ModelError(f"unknown transaction format: {self.tx_format}")
        if not 2 <= self.total_accounts <= 256:
            raise ModelError(f"account count outside u8 message model: {self.total_accounts}")
        if not 1 <= self.writable_accounts < self.total_accounts:
            raise ModelError("writable count must include payer and leave a program account")
        if self.tx_format == "v0_alt":
            if not 2 <= self.static_accounts_v0 <= self.total_accounts:
                raise ModelError("v0 static accounts must retain payer and Dragon program")
            loaded = self.total_accounts - self.static_accounts_v0
            loaded_writable = self.writable_accounts - 1
            if loaded_writable < 0 or loaded_writable > loaded:
                raise ModelError("v0 writable accounts do not fit loaded address set")


def _compiled_instruction(program_index: int, account_indices: Iterable[int], data: bytes) -> bytes:
    indices = bytes(account_indices)
    if program_index > 255 or any(index > 255 for index in indices):
        raise ModelError("compiled instruction index exceeds u8")
    return bytes([program_index]) + shortvec(len(indices)) + indices + shortvec(len(data)) + data


def serialize_synthetic_transaction(spec: WireSpec) -> bytes:
    """Serialize a one-signature, one-top-level-instruction synthetic transaction.

    Inner CPIs are not transaction message instructions. The byte string is deliberately unsigned:
    a fixed 64-byte placeholder occupies the real signature width.
    """

    spec.validate()
    signature_section = shortvec(1) + deterministic_bytes("signature-placeholder", 64)
    blockhash = deterministic_bytes("recent-blockhash-placeholder", 32)

    if spec.tx_format == "legacy_inline":
        keys = b"".join(
            deterministic_bytes(f"legacy-account-{index}", 32)
            for index in range(spec.total_accounts)
        )
        program_index = spec.writable_accounts
        instruction_accounts = [
            index for index in range(spec.total_accounts) if index != program_index
        ]
        instruction = _compiled_instruction(
            program_index, instruction_accounts, spec.instruction_data
        )
        header = bytes(
            [
                1,
                0,
                spec.total_accounts - spec.writable_accounts,
            ]
        )
        message = (
            header
            + shortvec(spec.total_accounts)
            + keys
            + blockhash
            + shortvec(1)
            + instruction
        )
        return signature_section + message

    static_count = spec.static_accounts_v0
    loaded_count = spec.total_accounts - static_count
    loaded_writable = spec.writable_accounts - 1
    loaded_readonly = loaded_count - loaded_writable
    static_keys = b"".join(
        deterministic_bytes(f"v0-static-account-{index}", 32) for index in range(static_count)
    )
    program_index = 1
    instruction_accounts = [
        index for index in range(spec.total_accounts) if index != program_index
    ]
    instruction = _compiled_instruction(program_index, instruction_accounts, spec.instruction_data)
    header = bytes([1, 0, static_count - 1])
    if loaded_count:
        lookup = (
            deterministic_bytes("address-lookup-table-placeholder", 32)
            + shortvec(loaded_writable)
            + bytes(range(loaded_writable))
            + shortvec(loaded_readonly)
            + bytes(range(loaded_writable, loaded_count))
        )
        lookups = shortvec(1) + lookup
    else:
        lookups = shortvec(0)
    message = (
        bytes([0x80])
        + header
        + shortvec(static_count)
        + static_keys
        + blockhash
        + shortvec(1)
        + instruction
        + lookups
    )
    return signature_section + message


def analytical_wire_size(spec: WireSpec) -> int:
    """Independent field-width sum for the same bounded synthetic message."""

    spec.validate()
    signature_section = shortvec_len(1) + 64
    instruction_accounts = spec.total_accounts - 1
    instruction = (
        1
        + shortvec_len(instruction_accounts)
        + instruction_accounts
        + shortvec_len(len(spec.instruction_data))
        + len(spec.instruction_data)
    )
    common = 3 + 32 + shortvec_len(1) + instruction
    if spec.tx_format == "legacy_inline":
        return (
            signature_section
            + common
            + shortvec_len(spec.total_accounts)
            + 32 * spec.total_accounts
        )
    static_count = spec.static_accounts_v0
    loaded_count = spec.total_accounts - static_count
    loaded_writable = spec.writable_accounts - 1
    loaded_readonly = loaded_count - loaded_writable
    lookup = 0
    if loaded_count:
        lookup = (
            32
            + shortvec_len(loaded_writable)
            + loaded_writable
            + shortvec_len(loaded_readonly)
            + loaded_readonly
        )
    return (
        signature_section
        + 1
        + common
        + shortvec_len(static_count)
        + 32 * static_count
        + shortvec_len(1 if loaded_count else 0)
        + lookup
    )


def rent_minimum(data_len: int, constants: dict[str, Any]) -> int:
    rent = constants["rent_package_default"]
    if data_len < 0:
        raise ModelError("negative account data length")
    return (rent["account_storage_overhead_bytes"] + data_len) * rent[
        "default_lamports_per_byte"
    ]


def admission(outcomes: int | None, constants: dict[str, Any]) -> dict[str, Any]:
    if outcomes is None:
        return {"v1_admitted": True, "reason": "not_outcome_dimensioned"}
    maximum = constants["dragon_design_bounds"]["max_v1_outcomes"]
    if 2 <= outcomes <= maximum:
        return {"v1_admitted": True, "reason": "within_frozen_v1_outcome_bound"}
    return {
        "v1_admitted": False,
        "reason": f"refuse_outcome_count_above_{maximum}",
    }


def wire_outputs(spec: WireSpec, constants: dict[str, Any]) -> dict[str, Any]:
    encoded = serialize_synthetic_transaction(spec)
    analytical = analytical_wire_size(spec)
    if len(encoded) != analytical:
        raise ModelError("wire serializer and analytical field sum diverged")
    limits = constants["protocol_limits"]
    packet_limit = limits["packet_data_size_bytes"]
    account_limit = limits["runtime_account_lock_limit"]
    return {
        "wire_bytes_measured": len(encoded),
        "wire_bytes_analytical": analytical,
        "wire_sha256": sha256_bytes(encoded),
        "packet_margin_bytes": packet_limit - len(encoded),
        "fits_packet_snapshot": len(encoded) <= packet_limit,
        "account_count": spec.total_accounts,
        "account_lock_margin": account_limit - spec.total_accounts,
        "fits_account_lock_snapshot": spec.total_accounts <= account_limit,
        "writable_accounts": spec.writable_accounts,
        "instruction_data_bytes": len(spec.instruction_data),
    }


def claim_instruction_data(operation: str, outcomes: int) -> bytes:
    tags = {
        "internal_split": 1,
        "external_split": 2,
        "materialize_one": 3,
        "materialize_all": 4,
    }
    return struct.pack("<BBBQ", 1, tags[operation], outcomes, 1)


def claim_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    bounds = constants["dragon_design_bounds"]
    token = constants["token_2022_base_layout"]
    limits = constants["protocol_limits"]
    rows: list[dict[str, Any]] = []
    for outcomes in bounds["outcome_axis"]:
        operation_models = {
            "internal_split": {
                "accounts": 8,
                "writable": 5,
                "token_cpis": 1,
                "local_steps": outcomes,
            },
            "external_split": {
                "accounts": 2 * outcomes + 7,
                "writable": 2 * outcomes + 4,
                "token_cpis": outcomes + 1,
                "local_steps": outcomes,
            },
            "materialize_one": {
                "accounts": 8,
                "writable": 5,
                "token_cpis": 1,
                "local_steps": 1,
            },
            "materialize_all": {
                "accounts": 2 * outcomes + 6,
                "writable": 2 * outcomes + 3,
                "token_cpis": outcomes,
                "local_steps": outcomes,
            },
        }
        for operation, model in operation_models.items():
            trace_entries = 1 + model["token_cpis"]
            for tx_format in ("legacy_inline", "v0_alt"):
                spec = WireSpec(
                    tx_format=tx_format,
                    total_accounts=model["accounts"],
                    writable_accounts=model["writable"],
                    static_accounts_v0=3,
                    instruction_data=claim_instruction_data(operation, outcomes),
                )
                output = wire_outputs(spec, constants)
                output.update(
                    {
                        "token_cpi_count_lower_bound": model["token_cpis"],
                        "instruction_trace_entries_lower_bound": trace_entries,
                        "instruction_trace_margin": limits["max_instruction_trace_length"]
                        - trace_entries,
                        "cpi_invocation_charge_component_cu": model["token_cpis"]
                        * limits["cpi_invocation_charge_compute_units"],
                        "local_fixed_store_or_mint_steps_lower_bound": model["local_steps"],
                        "outcome_mint_data_bytes_total": outcomes * token["bare_mint_bytes"],
                        "outcome_mint_rent_principal_lamports": outcomes
                        * rent_minimum(token["bare_mint_bytes"], constants),
                        "external_destination_data_bytes_total": outcomes
                        * token["base_token_account_bytes"],
                        "external_destination_rent_principal_lamports_if_created": outcomes
                        * rent_minimum(token["base_token_account_bytes"], constants),
                        "hypothetical_active_position_data_bytes": bounds[
                            "position_header_bytes"
                        ]
                        + 8 * outcomes,
                        "v1_fixed_position_data_bytes": bounds["position_header_bytes"]
                        + 8 * bounds["max_v1_outcomes"],
                        "hypothetical_supply_ledger_data_bytes": bounds[
                            "supply_ledger_header_bytes"
                        ]
                        + 16 * outcomes,
                    }
                )
                rows.append(
                    {
                        "schema": ROW_SCHEMA,
                        "scenario_id": f"claim-{operation}-n{outcomes}-{tx_format}",
                        "arm": ARM_HYPOTHESIS,
                        "family": "claim_transition",
                        "inputs": {
                            "outcomes": outcomes,
                            "operation": operation,
                            "tx_format": tx_format,
                            "existing_destination_accounts": True,
                            "top_level_instructions": 1,
                        },
                        "outputs": output,
                        "evidence": {
                            "wire_bytes_measured": "measured_local_serialization",
                            "wire_bytes_analytical": "independent_analytical_field_sum",
                            "accounts_cpi_trace_work": "analytical_layout_and_operation_lower_bound",
                            "rent": "analytical_package_default_not_cluster_measurement",
                            "compute": "pinned_cpi_invocation_charge_component_not_total_cu",
                        },
                        "admission": admission(outcomes, constants),
                        "caveats": [
                            "Synthetic account roles are a layout hypothesis, not a frozen ABI.",
                            "No ATA creation, compute-budget instruction, priority fee, program execution, or signature validity is modeled.",
                        ],
                    }
                )
    return rows


def pack_record_pages(page_size: int, header_size: int, record_sizes: list[int]) -> int:
    payload = page_size - header_size
    if payload <= 0:
        raise ModelError("page has no payload")
    pages = 0
    remaining = 0
    for record_size in record_sizes:
        if record_size <= 0 or record_size > payload:
            raise ModelError("record does not fit page")
        if record_size > remaining:
            pages += 1
            remaining = payload
        remaining -= record_size
    return pages


def page_shape(
    outcomes: int, page_size: int, order_count: int, constants: dict[str, Any]
) -> dict[str, int]:
    bounds = constants["dragon_design_bounds"]
    header = bounds["page_header_bytes"]
    single_size = bounds["single_egg_order_bytes"]
    portfolio_size = (
        bounds["portfolio_order_fixed_bytes"]
        + bounds["portfolio_coefficient_bytes_per_outcome"] * outcomes
    )
    single_records = [single_size] * order_count
    portfolio_records = [portfolio_size] * order_count
    mixed_records = [
        portfolio_size if index % 2 else single_size for index in range(order_count)
    ]
    payload = page_size - header
    return {
        "single_order_bytes": single_size,
        "portfolio_order_bytes": portfolio_size,
        "single_capacity_floor": payload // single_size,
        "portfolio_capacity_floor": payload // portfolio_size,
        "single_pages": pack_record_pages(page_size, header, single_records),
        "portfolio_pages": pack_record_pages(page_size, header, portfolio_records),
        "half_mix_pages": pack_record_pages(page_size, header, mixed_records),
    }


def page_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    bounds = constants["dragon_design_bounds"]
    rows: list[dict[str, Any]] = []
    for outcomes in bounds["outcome_axis"]:
        for page_size in bounds["page_sizes_bytes"]:
            for order_count in bounds["order_counts"]:
                shape = page_shape(outcomes, page_size, order_count, constants)
                page_rent = rent_minimum(page_size, constants)
                rows.append(
                    {
                        "schema": ROW_SCHEMA,
                        "scenario_id": f"page-n{outcomes}-b{page_size}-m{order_count}",
                        "arm": ARM_HYPOTHESIS,
                        "family": "order_page_layout",
                        "inputs": {
                            "outcomes": outcomes,
                            "page_bytes": page_size,
                            "order_count": order_count,
                            "page_header_bytes": bounds["page_header_bytes"],
                        },
                        "outputs": {
                            **shape,
                            "page_rent_principal_lamports": page_rent,
                            "half_mix_total_rent_principal_lamports": shape["half_mix_pages"]
                            * page_rent,
                        },
                        "evidence": {
                            "layout": "analytical_layout_hypothesis",
                            "rent": "analytical_package_default_not_cluster_measurement",
                        },
                        "admission": admission(outcomes, constants),
                        "caveats": [
                            "Packed pages are sequential and do not split records across pages.",
                            "Record widths are hypotheses; portfolio coefficients grow with n only for diagnostic comparison.",
                        ],
                    }
                )
    return rows


def accumulator_instruction_data(summary_kind: str, page_count: int) -> bytes:
    feature_tag = {"terminal": 1, "twap": 2, "full": 3}[summary_kind]
    return (
        struct.pack("<BBBB", 1, 20, feature_tag, page_count)
        + deterministic_bytes("window-id", 32)
        + struct.pack("<H", 0)
    )


def accumulator_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    bounds = constants["dragon_design_bounds"]
    limits = constants["protocol_limits"]
    rows: list[dict[str, Any]] = []
    for summary_kind, layout in bounds["summary_layouts"].items():
        for page_count in bounds["accumulator_page_counts"]:
            for tx_format in ("legacy_inline", "v0_alt"):
                spec = WireSpec(
                    tx_format=tx_format,
                    total_accounts=page_count + 4,
                    writable_accounts=2,
                    static_accounts_v0=2,
                    instruction_data=accumulator_instruction_data(summary_kind, page_count),
                )
                output = wire_outputs(spec, constants)
                output.update(
                    {
                        "token_cpi_count_lower_bound": 0,
                        "instruction_trace_entries_lower_bound": 1,
                        "instruction_trace_margin": limits["max_instruction_trace_length"] - 1,
                        "cpi_invocation_charge_component_cu": 0,
                        "summary_data_bytes": layout["data_bytes"],
                        "summary_rent_principal_lamports": rent_minimum(
                            layout["data_bytes"], constants
                        ),
                        "adjacent_combine_count_lower_bound": max(0, page_count - 1),
                        "scalar_combine_steps_lower_bound": max(0, page_count - 1)
                        * layout["combine_scalar_steps"],
                    }
                )
                rows.append(
                    {
                        "schema": ROW_SCHEMA,
                        "scenario_id": f"accumulator-{summary_kind}-p{page_count}-{tx_format}",
                        "arm": ARM_HYPOTHESIS,
                        "family": "accumulator_fold",
                        "inputs": {
                            "summary_kind": summary_kind,
                            "page_count": page_count,
                            "tx_format": tx_format,
                            "top_level_instructions": 1,
                        },
                        "outputs": output,
                        "evidence": {
                            "wire_bytes_measured": "measured_local_serialization",
                            "wire_bytes_analytical": "independent_analytical_field_sum",
                            "accounts_trace_work": "analytical_layout_and_monoid_lower_bound",
                            "rent": "analytical_package_default_not_cluster_measurement",
                            "compute": "not_measured_no_sbf_program",
                        },
                        "admission": admission(None, constants),
                        "caveats": [
                            "Summary fields and widths are hypotheses pending algebra and ABI freeze.",
                            "Scalar combine counts are semantic work counters, not compute units.",
                        ],
                    }
                )
    return rows


def batch_instruction_data(outcomes: int, page_count: int) -> bytes:
    prices = b"".join(struct.pack("<Q", index + 1) for index in range(outcomes))
    return (
        struct.pack("<BBBB", 1, 30, outcomes, 0)
        + deterministic_bytes("epoch-domain", 32)
        + deterministic_bytes("candidate-digest", 32)
        + prices
        + struct.pack("<QQHH", 0, 0, 0, page_count)
    )


def max_pages_by_wire(
    outcomes: int, tx_format: str, constants: dict[str, Any]
) -> int:
    limits = constants["protocol_limits"]
    maximum = limits["runtime_account_lock_limit"] - 6
    accepted = 0
    for pages in range(1, maximum + 1):
        spec = WireSpec(
            tx_format=tx_format,
            total_accounts=pages + 6,
            writable_accounts=2,
            static_accounts_v0=2,
            instruction_data=batch_instruction_data(outcomes, pages),
        )
        if len(serialize_synthetic_transaction(spec)) <= limits["packet_data_size_bytes"]:
            accepted = pages
        else:
            break
    return accepted


def batch_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    bounds = constants["dragon_design_bounds"]
    limits = constants["protocol_limits"]
    rows: list[dict[str, Any]] = []
    for outcomes in bounds["outcome_axis"]:
        for page_size in bounds["page_sizes_bytes"]:
            for order_count in bounds["order_counts"]:
                shape = page_shape(outcomes, page_size, order_count, constants)
                page_count = shape["half_mix_pages"]
                for tx_format in ("legacy_inline", "v0_alt"):
                    all_pages_spec = WireSpec(
                        tx_format=tx_format,
                        total_accounts=page_count + 6,
                        writable_accounts=2,
                        static_accounts_v0=2,
                        instruction_data=batch_instruction_data(outcomes, page_count),
                    )
                    one_page_spec = WireSpec(
                        tx_format=tx_format,
                        total_accounts=7,
                        writable_accounts=2,
                        static_accounts_v0=2,
                        instruction_data=batch_instruction_data(outcomes, 1),
                    )
                    output = wire_outputs(all_pages_spec, constants)
                    one_page = wire_outputs(one_page_spec, constants)
                    page_batch_capacity = max_pages_by_wire(outcomes, tx_format, constants)
                    output.update(
                        {
                            "page_count": page_count,
                            "one_page_wire_bytes_measured": one_page["wire_bytes_measured"],
                            "one_page_packet_margin_bytes": one_page["packet_margin_bytes"],
                            "pages_per_transaction_wire_and_account_upper_bound": page_batch_capacity,
                            "minimum_transactions_from_wire_and_account_only": (
                                math.ceil(page_count / page_batch_capacity)
                                if page_batch_capacity
                                else None
                            ),
                            "token_cpi_count_lower_bound": 0,
                            "instruction_trace_entries_lower_bound": 1,
                            "instruction_trace_margin": limits["max_instruction_trace_length"] - 1,
                            "cpi_invocation_charge_component_cu": 0,
                            "order_authentications_lower_bound": order_count,
                            "fill_bound_checks_lower_bound": order_count,
                            "simplex_terms_lower_bound": outcomes,
                            "asset_closure_checks_lower_bound": outcomes + 1,
                            "portfolio_dot_terms_at_zero_percent": 0,
                            "portfolio_dot_terms_at_fifty_percent": (order_count // 2)
                            * outcomes,
                            "portfolio_dot_terms_at_one_hundred_percent": order_count
                            * outcomes,
                            "primitive_relation_steps_floor_excluding_hash_and_allocation": 2
                            * order_count
                            + 2 * outcomes
                            + 1,
                            "order_page_rent_principal_lamports": page_count
                            * rent_minimum(page_size, constants),
                        }
                    )
                    rows.append(
                        {
                            "schema": ROW_SCHEMA,
                            "scenario_id": (
                                f"batch-n{outcomes}-b{page_size}-m{order_count}-{tx_format}"
                            ),
                            "arm": ARM_HYPOTHESIS,
                            "family": "batch_verification",
                            "inputs": {
                                "outcomes": outcomes,
                                "page_bytes": page_size,
                                "order_count": order_count,
                                "portfolio_share": "50_percent_alternating",
                                "tx_format": tx_format,
                                "top_level_instructions": 1,
                            },
                            "outputs": output,
                            "evidence": {
                                "wire_bytes_measured": "measured_local_serialization",
                                "wire_bytes_analytical": "independent_analytical_field_sum",
                                "accounts_trace_work": "analytical_layout_and_information_lower_bound",
                                "rent": "analytical_package_default_not_cluster_measurement",
                                "compute": "not_measured_no_sbf_program",
                            },
                            "admission": admission(outcomes, constants),
                            "caveats": [
                                "All-pages wire fit does not imply compute fit; the design remains paginated.",
                                "No hash, allocation, fee, score, account-load, or SBF CU cost is assigned a synthetic constant.",
                                "The relation must authenticate every frozen order without a separately verified succinct proof.",
                            ],
                        }
                    )
    return rows


def landed_bounds(constants: dict[str, Any]) -> dict[str, Any]:
    return constants[ARM_LANDED]["bounds"]


def landed_account_bytes(constants: dict[str, Any], name: str) -> int:
    return constants[ARM_LANDED]["accounts"][name]["bytes"]


def landed_intent_bytes(constants: dict[str, Any], name: str) -> int:
    return constants[ARM_LANDED]["intents"][name]["bytes"]


def landed_admission(
    constants: dict[str, Any],
    outcomes: int | None = None,
    order_count: int | None = None,
) -> dict[str, Any]:
    """Landed-codec refusals stack on top of the unchanged V1 policy admission rule."""

    bounds = landed_bounds(constants)
    if outcomes is not None and not (
        bounds["minimum_outcomes"] <= outcomes <= bounds["max_outcomes"]
    ):
        return {
            "v1_admitted": False,
            "reason": "refuse_landed_codec_outcome_count_outside_"
            f"{bounds['minimum_outcomes']}_to_{bounds['max_outcomes']}",
        }
    if order_count is not None and order_count > bounds["max_epoch_orders"]:
        return {
            "v1_admitted": False,
            "reason": f"refuse_landed_order_count_above_{bounds['max_epoch_orders']}",
        }
    return admission(outcomes, constants)


def landed_intent_data(constants: dict[str, Any], name: str) -> bytes:
    """Emit one landed intent payload: exact landed field widths, placeholder identities.

    Only the widths and their order are landed. The identities are deterministic placeholders, so
    this is a byte-width measurement of a landed encoding and never a valid signed instruction.
    """

    intent = constants[ARM_LANDED]["intents"][name]
    terms = intent["field_terms"]
    if terms[0] != 2:
        raise ModelError(f"landed intent does not open with a tag/version header: {name}")
    payload = bytearray([intent["intent_tag"], intent["intent_version"]])
    for index, width in enumerate(terms[1:]):
        payload.extend(deterministic_bytes(f"landed-intent-{name}-field-{index}", width))
    if len(payload) != intent["bytes"]:
        raise ModelError(f"landed intent payload width drifted: {name}")
    return bytes(payload)


def landed_page_count(constants: dict[str, Any], order_count: int) -> int | None:
    """Pages in a frozen epoch book, or None when the landed codec refuses the book."""

    bounds = landed_bounds(constants)
    if order_count < 1 or order_count > bounds["max_epoch_orders"]:
        return None
    per_page = bounds["max_orders_per_page"]
    return (order_count + per_page - 1) // per_page


LANDED_ACCOUNT_ROLES = {
    "realm": "Realm collateral/profile namespace, frozen by an external adapter.",
    "profile": "Collateral profile identity bound to a Realm.",
    "market": "Market namespace with its 512-byte question commitment.",
    "hoard": "Market collateral custody seam.",
    "position": "Owner internal balances over the fixed 16-outcome vector.",
    "feed_head": "Feed cursor and evidence digest; not a fold summary.",
    "order_page": "Dense 16-slot order page carrying either admitted order family or a retirement, with cross-page closure fields.",
    "supply_ledger": "Per-outcome internal and external supply totals.",
    "terms": "Immutable payout terms over at most 8 payout vectors, plus the v3 resolution basis, its knot vector and the per-market collateral cap.",
    "price_grid": "Frozen strictly increasing 64-tick price grid.",
    "epoch": "Book domain, phase, and frozen page-set binding.",
    "candidate_record": "Submitted clearing candidate and its score.",
    "final_pot": "Terminal per-outcome pot the settlement path draws from.",
    "settlement_receipt": "One settled leg's replay-separated receipt.",
    "resolution": "Resolved payout index and its evidence binding.",
    "clear_work": "Streaming clearing checkpoint framing plus its opaque, owner-defined work body.",
    "candidate_feed": "Streaming candidate fill vector and bounded pairing-witness feed.",
}


def landed_inventory_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    rent = constants["rent_package_default"]
    landed = constants[ARM_LANDED]
    overhead = rent["account_storage_overhead_bytes"]
    per_byte = rent["default_lamports_per_byte"]
    rows: list[dict[str, Any]] = []
    total_bytes = 0
    total_rent = 0
    for name in LANDED_ACCOUNT_ORDER:
        account = landed["accounts"][name]
        data_bytes = account["bytes"]
        principal = rent_minimum(data_bytes, constants)
        total_bytes += data_bytes
        total_rent += principal
        rows.append(
            {
                "schema": ROW_SCHEMA,
                "scenario_id": f"landed-account-{name.replace('_', '-')}",
                "arm": ARM_LANDED,
                "family": "landed_account_inventory",
                "inputs": {
                    "account": name,
                    "rust_const": account["rust_const"],
                    "discriminator_tag": account["discriminator_tag"],
                    "schema_version": account["schema_version"],
                    "formula": account["formula"],
                    "role": LANDED_ACCOUNT_ROLES[name],
                },
                "outputs": {
                    "data_bytes": data_bytes,
                    "field_term_sum": sum(account["field_terms"]),
                    "field_term_count": len(account["field_terms"]),
                    "rent_principal_lamports": principal,
                    "rent_payload_component_lamports": data_bytes * per_byte,
                    "rent_overhead_component_lamports": overhead * per_byte,
                    "instances_per_market_note": "not_modeled",
                },
                "evidence": {
                    "layout": "landed_codec_constant",
                    "layout_derivation": "independent_field_term_sum",
                    "rent": "analytical_package_default_not_cluster_measurement",
                    "compute": "not_measured_no_sbf_program",
                },
                "admission": landed_admission(constants),
                "caveats": [
                    "A landed byte width is an exact encoding fact, not a measured operation cost.",
                    "Rent principal is refundable under the pinned package default and is not a target-cluster quote.",
                    "Account instance counts per market are a lifecycle question this lab does not model.",
                ],
            }
        )
    largest = max(LANDED_ACCOUNT_ORDER, key=lambda name: landed["accounts"][name]["bytes"])
    smallest = min(LANDED_ACCOUNT_ORDER, key=lambda name: landed["accounts"][name]["bytes"])
    rows.append(
        {
            "schema": ROW_SCHEMA,
            "scenario_id": "landed-account-inventory-one-instance",
            "arm": ARM_LANDED,
            "family": "landed_account_inventory",
            "inputs": {
                "account": "one_instance_of_every_landed_account",
                "account_count": len(LANDED_ACCOUNT_ORDER),
                "codec_commit": landed["source"]["commit_short"],
            },
            "outputs": {
                "data_bytes": total_bytes,
                "rent_principal_lamports": total_rent,
                "rent_payload_component_lamports": total_bytes * per_byte,
                "rent_overhead_component_lamports": len(LANDED_ACCOUNT_ORDER) * overhead * per_byte,
                "rent_overhead_bytes_total": len(LANDED_ACCOUNT_ORDER) * overhead,
                "largest_account": largest,
                "largest_account_bytes": landed["accounts"][largest]["bytes"],
                "smallest_account": smallest,
                "smallest_account_bytes": landed["accounts"][smallest]["bytes"],
            },
            "evidence": {
                "layout": "landed_codec_constant",
                "layout_derivation": "independent_field_term_sum",
                "rent": "analytical_package_default_not_cluster_measurement",
                "compute": "not_measured_no_sbf_program",
            },
            "admission": landed_admission(constants),
            "caveats": [
                "One instance of each account is an inventory unit, not a deployment plan: a live market holds many Positions, pages, and receipts.",
                "The per-account 128-byte storage overhead is a fifth of this inventory's principal, so account count matters as much as payload.",
            ],
        }
    )
    return rows


def landed_epoch_book_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    landed = constants[ARM_LANDED]
    bounds = landed["bounds"]
    per_page = bounds["max_orders_per_page"]
    slot_bytes = bounds["order_slot_bytes"]
    record_bytes = bounds["order_record_bytes"]
    page_bytes = landed_account_bytes(constants, "order_page")
    epoch_bytes = landed_account_bytes(constants, "epoch")
    page_rent = rent_minimum(page_bytes, constants)
    epoch_rent = rent_minimum(epoch_bytes, constants)
    settle_bytes = landed_intent_bytes(constants, "settle_page")
    rows: list[dict[str, Any]] = []
    for order_count in landed["epoch_book_order_counts"]:
        pages = landed_page_count(constants, order_count)
        representable = pages is not None
        if representable:
            final_page_orders = order_count - (pages - 1) * per_page
            outputs = {
                "landed_codec_representable": True,
                "refusal_reason": None,
                "page_count": pages,
                "dense_pages": pages - 1,
                "final_page_order_count": final_page_orders,
                "occupied_slot_bytes": order_count * slot_bytes,
                "allocated_slot_bytes": pages * per_page * slot_bytes,
                "padding_slot_bytes": (pages * per_page - order_count) * slot_bytes,
                "single_egg_intra_slot_padding_bytes_if_all_single": order_count
                * (slot_bytes - 1 - record_bytes),
                "page_header_bytes_total": pages * bounds["order_page_header_bytes"],
                "page_account_data_bytes_total": pages * page_bytes,
                "page_rent_principal_lamports_per_page": page_rent,
                "rent_principal_lamports": pages * page_rent,
                "epoch_account_data_bytes": epoch_bytes,
                "epoch_account_rent_principal_lamports": epoch_rent,
                "book_rent_principal_lamports": pages * page_rent + epoch_rent,
                "settle_page_intent_bytes": settle_bytes,
                "settle_page_instructions_lower_bound": pages,
                "page_accounts_locked_if_one_transaction": pages,
            }
        else:
            outputs = {
                "landed_codec_representable": False,
                "refusal_reason": f"order_count_above_landed_max_epoch_orders_{bounds['max_epoch_orders']}",
                "page_count": None,
                "dense_pages": None,
                "final_page_order_count": None,
                "occupied_slot_bytes": None,
                "allocated_slot_bytes": None,
                "padding_slot_bytes": None,
                "single_egg_intra_slot_padding_bytes_if_all_single": None,
                "page_header_bytes_total": None,
                "page_account_data_bytes_total": None,
                "page_rent_principal_lamports_per_page": page_rent,
                "rent_principal_lamports": None,
                "epoch_account_data_bytes": epoch_bytes,
                "epoch_account_rent_principal_lamports": epoch_rent,
                "book_rent_principal_lamports": None,
                "settle_page_intent_bytes": settle_bytes,
                "settle_page_instructions_lower_bound": None,
                "page_accounts_locked_if_one_transaction": None,
            }
        rows.append(
            {
                "schema": ROW_SCHEMA,
                "scenario_id": f"landed-epoch-book-m{order_count}",
                "arm": ARM_LANDED,
                "family": "landed_epoch_book",
                "inputs": {
                    "order_count": order_count,
                    "orders_per_page": per_page,
                    "max_order_pages": bounds["max_order_pages"],
                    "order_page_bytes": page_bytes,
                    "order_slot_bytes": slot_bytes,
                    "order_record_bytes": record_bytes,
                    "portfolio_record_bytes": bounds["portfolio_record_bytes"],
                },
                "outputs": outputs,
                "evidence": {
                    "layout": "landed_codec_constant",
                    "packing": "landed_dense_page_rule_not_a_packer_hypothesis",
                    "rent": "analytical_package_default_not_cluster_measurement",
                    "instruction_bytes": "landed_codec_constant",
                    "compute": "not_measured_no_sbf_program",
                },
                "admission": landed_admission(constants, order_count=order_count),
                "caveats": [
                    "The landed page rule is not a packing choice: every non-final page of a frozen set must hold exactly 16 slots, so the page count is forced by the order count.",
                    "Padding slot bytes are paid rent for canonically zeroed slots, not spare capacity for later orders in a frozen set.",
                    "A slot is one fixed width for both admitted families, so an all-single-Egg book also pays the intra-slot padding column; the harness does not model a family mix per order.",
                    "Whether one transaction may lock the whole page set is an account-topology question the landed ABI does not answer.",
                ],
            }
        )
    return rows


def landed_intent_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    landed = constants[ARM_LANDED]
    bounds = landed["bounds"]
    topology = landed["intent_account_topology_hypothesis"]
    limits = constants["protocol_limits"]
    rows: list[dict[str, Any]] = []
    for name in LANDED_INTENT_ORDER:
        intent = landed["intents"][name]
        shape = topology[name]
        data = landed_intent_data(constants, name)
        for tx_format in ("legacy_inline", "v0_alt"):
            spec = WireSpec(
                tx_format=tx_format,
                total_accounts=shape["total_accounts"],
                writable_accounts=shape["writable_accounts"],
                static_accounts_v0=shape["static_accounts_v0"],
                instruction_data=data,
            )
            output = wire_outputs(spec, constants)
            output.update(
                {
                    "intent_bytes": intent["bytes"],
                    "intent_tag": intent["intent_tag"],
                    "intent_version": intent["intent_version"],
                    "max_intent_bytes_margin": bounds["max_intent_bytes"] - intent["bytes"],
                    "instruction_trace_entries_lower_bound": 1,
                    "instruction_trace_margin": limits["max_instruction_trace_length"] - 1,
                }
            )
            rows.append(
                {
                    "schema": ROW_SCHEMA,
                    "scenario_id": f"landed-intent-{name.replace('_', '-')}-{tx_format}",
                    "arm": ARM_LANDED,
                    "family": "landed_intent_wire",
                    "inputs": {
                        "intent": name,
                        "tx_format": tx_format,
                        "total_accounts_hypothesis": shape["total_accounts"],
                        "writable_accounts_hypothesis": shape["writable_accounts"],
                        "top_level_instructions": 1,
                    },
                    "outputs": output,
                    "evidence": {
                        "wire_bytes_measured": "measured_local_serialization",
                        "wire_bytes_analytical": "independent_analytical_field_sum",
                        "instruction_data_bytes": "landed_codec_constant",
                        "account_topology": "layout_hypothesis_not_landed",
                        "compute": "not_measured_no_sbf_program",
                    },
                    "admission": landed_admission(constants),
                    "caveats": [
                        "The payload width is landed; the account set around it is a hypothesis and is labeled as one.",
                        "One intent is modeled as one top-level instruction. The landed crate does not fix that mapping.",
                        "No CPI, ATA creation, compute-budget instruction, or signature validity is modeled.",
                    ],
                }
            )
    return rows


def landed_relation_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    landed = constants[ARM_LANDED]
    bounds = landed["bounds"]
    page_bytes = landed_account_bytes(constants, "order_page")
    page_rent = rent_minimum(page_bytes, constants)
    frozen_companions = ("epoch", "price_grid", "candidate_record")
    companion_bytes = sum(landed_account_bytes(constants, name) for name in frozen_companions)
    companion_rent = sum(
        rent_minimum(landed_account_bytes(constants, name), constants) for name in frozen_companions
    )
    rows: list[dict[str, Any]] = []
    for outcomes in constants["dragon_design_bounds"]["outcome_axis"]:
        codec_outcomes_ok = bounds["minimum_outcomes"] <= outcomes <= bounds["max_outcomes"]
        for order_count in landed["relation_order_counts"]:
            pages = landed_page_count(constants, order_count)
            if pages is None:
                raise ModelError("landed relation order count exceeds the landed book")
            rows.append(
                {
                    "schema": ROW_SCHEMA,
                    "scenario_id": f"landed-relation-n{outcomes}-m{order_count}",
                    "arm": ARM_LANDED,
                    "family": "landed_batch_relation",
                    "inputs": {
                        "outcomes": outcomes,
                        "order_count": order_count,
                        "max_orders": bounds["relation_max_orders"],
                        "price_scale": bounds["relation_price_scale"],
                        "relation_version": bounds["relation_version"],
                    },
                    "outputs": {
                        "landed_codec_representable": codec_outcomes_ok,
                        "page_count": pages,
                        "order_page_accounts": pages,
                        "order_authentications_lower_bound": order_count,
                        "fill_bound_checks_lower_bound": order_count,
                        "simplex_terms_lower_bound": outcomes,
                        "asset_closure_checks_lower_bound": outcomes + 1,
                        "primitive_relation_steps_floor_excluding_hash_and_allocation": 2
                        * order_count
                        + 2 * outcomes
                        + 1,
                        "relation_leg_capacity": bounds["relation_max_legs"],
                        "relation_slice_capacity": bounds["relation_max_slices"],
                        "grid_tick_capacity": bounds["max_grid_ticks"],
                        "portfolio_orders_admitted_by_relation_upper_bound": bounds[
                            "relation_max_portfolio_orders"
                        ],
                        "portfolio_orders_persistable_in_landed_pages": min(
                            order_count, bounds["max_portfolio_orders"]
                        ),
                        "order_page_data_bytes_total": pages * page_bytes,
                        "rent_principal_lamports": pages * page_rent,
                        "frozen_epoch_companion_bytes": companion_bytes,
                        "frozen_epoch_state_bytes": pages * page_bytes + companion_bytes,
                        "frozen_epoch_state_rent_principal_lamports": pages * page_rent
                        + companion_rent,
                    },
                    "evidence": {
                        "layout": "landed_codec_constant",
                        "bounds": "landed_relation_crate_constant",
                        "work": "analytical_information_lower_bound",
                        "rent": "analytical_package_default_not_cluster_measurement",
                        "instruction_bytes": "absent_no_landed_verification_instruction",
                        "compute": "not_measured_no_sbf_program",
                    },
                    "admission": landed_admission(constants, outcomes=outcomes),
                    "caveats": [
                        "No wire row is emitted here: the landed ABI has no candidate-verification instruction, and this arm refuses to invent one.",
                        "The page carries both admitted families in one tagged slot array, so a relation book of portfolio orders is persistable up to the shared MAX_PORTFOLIO_ORDERS cap; a set above that cap is a codec refusal, not an expensive case.",
                        "Work counters are semantic steps, not compute units.",
                        "MAX_ORDERS=64 caps one frozen book, not a market's order flow; more orders means more epochs, not a bigger book.",
                    ],
                }
            )
    return rows


# The superseded v2 order page: a 235-byte header over sixteen bare 99-byte single-Egg records,
# with no kind discriminator, no portfolio family and no retirement. It is quoted only as the
# baseline the tagged-slot page grew from and is not read from the codec, which no longer has it.
V2_SINGLE_FAMILY_PAGE_BYTES = 1819


def differential_entries(constants: dict[str, Any]) -> list[dict[str, Any]]:
    bounds = constants["dragon_design_bounds"]
    landed = constants[ARM_LANDED]
    landed_bounds_value = landed["bounds"]
    hypothesis_page = 8192
    hypothesis_payload = hypothesis_page - bounds["page_header_bytes"]
    landed_only = [
        name
        for name in LANDED_ACCOUNT_ORDER
        if name not in {"position", "supply_ledger", "order_page"}
    ]
    landed_only_bytes = sum(landed["accounts"][name]["bytes"] for name in landed_only)
    return [
        {
            "object": "position_account",
            "unit": "bytes",
            "rent_class": True,
            "hypothesis": bounds["position_header_bytes"] + 8 * bounds["max_v1_outcomes"],
            "hypothesis_source": "position_header_bytes + 8 * max_v1_outcomes",
            "landed": landed["accounts"]["position"]["bytes"],
            "landed_source": "account_len::POSITION",
            "change": "The 128-byte 16-outcome balance vector is unchanged; the landed account also stores market and owner identities, a replay generation, cash and reserved-cash atoms, a stored bump and a close state, so the header is 92 bytes rather than the hypothetical 64.",
        },
        {
            "object": "supply_ledger_account",
            "unit": "bytes",
            "rent_class": True,
            "hypothesis": bounds["supply_ledger_header_bytes"] + 16 * bounds["max_v1_outcomes"],
            "hypothesis_source": "supply_ledger_header_bytes + 16 * max_v1_outcomes",
            "landed": landed["accounts"]["supply_ledger"]["bytes"],
            "landed_source": "account_len::SUPPLY_LEDGER",
            "change": "Both arms carry two u64 totals per outcome (256 bytes); the landed header is 77 bytes of market, realm, generation, outcome count, bump and flags rather than the hypothetical 64.",
        },
        {
            "object": "single_egg_order_record",
            "unit": "bytes",
            "rent_class": False,
            "hypothesis": bounds["single_egg_order_bytes"],
            "hypothesis_source": "single_egg_order_bytes",
            "landed": landed_bounds_value["order_record_bytes"],
            "landed_source": "ORDER_RECORD_BYTES",
            "change": "The landed record spends 64 bytes on owner and order identity plus quantity, limit, minimum fill, generation, outcome, side, flags and, since v4, an eight-byte per-order expiry epoch; the 80-byte sketch had no room for the replay generation, the expiry horizon or dual 32-byte identities.",
        },
        {
            "object": "portfolio_order_record",
            "unit": "bytes",
            "rent_class": False,
            "hypothesis": bounds["portfolio_order_fixed_bytes"]
            + bounds["portfolio_coefficient_bytes_per_outcome"] * bounds["max_v1_outcomes"],
            "hypothesis_source": "portfolio_order_fixed_bytes + 8 * max_v1_outcomes at n=16",
            "landed": landed_bounds_value["portfolio_record_bytes"],
            "landed_source": "PORTFOLIO_RECORD_BYTES",
            "change": "The portfolio order has a persisted page encoding: the same 128-byte 16-slot coefficient vector plus dual 32-byte identities, side, active length, flags and five u64s (lots, per-lot collateral bound, minimum fill, replay generation, expiry epoch), so the landed body is "
            f"{landed_bounds_value['portfolio_record_bytes']} bytes against the {bounds['portfolio_order_fixed_bytes'] + bounds['portfolio_coefficient_bytes_per_outcome'] * bounds['max_v1_outcomes']}-byte sketch. It rides a "
            f"{landed_bounds_value['order_slot_bytes']}-byte tagged slot shared with the single-Egg family and with retirements.",
        },
        {
            "object": "order_page_account",
            "unit": "bytes",
            "rent_class": True,
            "hypothesis": hypothesis_page,
            "hypothesis_source": "8 KiB page hypothesis",
            "landed": landed["accounts"]["order_page"]["bytes"],
            "landed_source": "account_len::ORDER_PAGE",
            "change": "The landed page is a fixed 16-slot array with cross-page closure fields, not a variable byte budget, so page size stopped being a tunable parameter. The slot is wide enough for every admitted slot kind, both order families and a retirement, which is why it is "
            f"{landed['accounts']['order_page']['bytes']} bytes rather than the {V2_SINGLE_FAMILY_PAGE_BYTES} of the single-family v2 page.",
        },
        {
            "object": "order_page_header",
            "unit": "bytes",
            "rent_class": False,
            "hypothesis": bounds["page_header_bytes"],
            "hypothesis_source": "page_header_bytes",
            "landed": landed_bounds_value["order_page_header_bytes"],
            "landed_source": "account_len::ORDER_PAGE minus the slot array",
            "change": "The landed header carries seven 32-byte identities (market, epoch, order set, page digest, first, last and previous-page-last order ids) that the hypothesis never budgeted for, plus the page/set counters and the v4 retirement count.",
        },
        {
            "object": "order_page_record_capacity",
            "unit": "records_per_page",
            "rent_class": False,
            "hypothesis": hypothesis_payload // bounds["single_egg_order_bytes"],
            "hypothesis_source": "(8192 - 128) // 80",
            "landed": landed_bounds_value["max_orders_per_page"],
            "landed_source": "MAX_ORDERS_PER_PAGE",
            "change": "A landed page holds 16 slots, not about a hundred records, so any per-page cost is amortized over six times fewer orders.",
        },
        {
            "object": "epoch_book_order_capacity",
            "unit": "orders_per_book",
            "rent_class": False,
            "hypothesis": max(bounds["order_counts"]),
            "hypothesis_source": "largest modeled order count",
            "landed": landed_bounds_value["max_epoch_orders"],
            "landed_source": "MAX_EPOCH_ORDERS and clutch-batch MAX_ORDERS",
            "change": "One frozen book is capped at 64 orders across 4 pages, so the 128- and 512-order cases describe multiple epochs, never one relation instance.",
        },
        {
            "object": "claim_instruction_internal_split",
            "unit": "bytes",
            "rent_class": False,
            "hypothesis": 11,
            "hypothesis_source": "synthetic claim instruction data",
            "landed": landed["intents"]["split"]["bytes"],
            "landed_source": "Intent::Split encoded_len",
            "change": "The landed payload names market and owner by 32-byte identity instead of packing an outcome count and a u64 into 11 bytes.",
        },
        {
            "object": "claim_instruction_materialize_one",
            "unit": "bytes",
            "rent_class": False,
            "hypothesis": 11,
            "hypothesis_source": "synthetic claim instruction data",
            "landed": landed["intents"]["materialize"]["bytes"],
            "landed_source": "Intent::Materialize encoded_len",
            "change": "The landed payload adds a 32-byte destination and an outcome index to the market/owner pair, still far inside MAX_INTENT_BYTES.",
        },
        {
            "object": "accumulator_full_summary",
            "unit": "bytes",
            "rent_class": True,
            "hypothesis": bounds["summary_layouts"]["full"]["data_bytes"],
            "hypothesis_source": "summary_layouts.full.data_bytes",
            "landed": None,
            "landed_source": "absent",
            "change": "No accumulator summary account exists in the landed family; FeedHead is a 124-byte cursor plus evidence digest, not a fold summary, so the accumulator arm stays entirely hypothetical.",
        },
        {
            "object": "landed_only_account_family",
            "unit": "bytes",
            "rent_class": True,
            "hypothesis": None,
            "hypothesis_source": "absent",
            "landed": landed_only_bytes,
            "landed_source": f"sum of {len(landed_only)} landed accounts with no hypothesis counterpart",
            "change": "Realm, Profile, Market, Hoard, FeedHead, Terms, PriceGrid, Epoch, CandidateRecord, FinalPot, SettlementReceipt, Resolution, ClearWork and CandidateFeed were never in the hypothesis arm, so most of the landed rent inventory is not represented by the design sketch.",
        },
    ]


def differential_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for entry in differential_entries(constants):
        hypothesis = entry["hypothesis"]
        landed = entry["landed"]
        both = hypothesis is not None and landed is not None
        delta = landed - hypothesis if both else None
        rent_class = entry["rent_class"]
        hypothesis_rent = (
            rent_minimum(hypothesis, constants)
            if rent_class and hypothesis is not None
            else None
        )
        landed_rent = (
            rent_minimum(landed, constants) if rent_class and landed is not None else None
        )
        rows.append(
            {
                "schema": ROW_SCHEMA,
                "scenario_id": f"diff-{entry['object'].replace('_', '-')}",
                "arm": ARM_DIFFERENTIAL,
                "family": "abi_differential",
                "inputs": {
                    "object": entry["object"],
                    "unit": entry["unit"],
                    "hypothesis_source": entry["hypothesis_source"],
                    "landed_source": entry["landed_source"],
                    "present_in_both_arms": both,
                },
                "outputs": {
                    "hypothesis_value": hypothesis,
                    "landed_value": landed,
                    "delta": delta,
                    "landed_over_hypothesis_permille": (
                        (landed * 1000) // hypothesis if both and hypothesis else None
                    ),
                    "hypothesis_rent_principal_lamports": hypothesis_rent,
                    "landed_rent_principal_lamports": landed_rent,
                    "delta_rent_principal_lamports": (
                        landed_rent - hypothesis_rent
                        if hypothesis_rent is not None and landed_rent is not None
                        else None
                    ),
                    "change": entry["change"],
                },
                "evidence": {
                    "hypothesis_value": "layout_hypothesis",
                    "landed_value": "landed_codec_constant",
                    "delta": "exact_integer_difference_of_the_two_arms",
                    "rent": "analytical_package_default_not_cluster_measurement",
                    "compute": "not_measured_no_sbf_program",
                },
                "admission": landed_admission(constants),
                "caveats": [
                    "A delta is a layout fact, not a cost verdict: no arm has a measured CU, account-copy, or landing figure.",
                    "The hypothesis value is retained for falsification history and must not be quoted as current layout.",
                ],
            }
        )
    return rows


def generate_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    rows = (
        claim_rows(constants)
        + page_rows(constants)
        + accumulator_rows(constants)
        + batch_rows(constants)
        + landed_inventory_rows(constants)
        + landed_epoch_book_rows(constants)
        + landed_intent_rows(constants)
        + landed_relation_rows(constants)
        + differential_rows(constants)
    )
    rows.sort(key=lambda row: (ARMS.index(row["arm"]), row["scenario_id"]))
    validate_rows(rows, constants)
    return rows


RETAINED_HYPOTHESIS_ROW_COUNT = 193


def validate_landed_rows(rows: list[dict[str, Any]], constants: dict[str, Any]) -> None:
    """Guard the properties that make the landed arm usable as evidence."""

    bounds = landed_bounds(constants)
    hypothesis_rows = [row for row in rows if row["arm"] == ARM_HYPOTHESIS]
    if len(hypothesis_rows) != RETAINED_HYPOTHESIS_ROW_COUNT:
        raise ModelError(
            "the layout_hypothesis arm must be retained whole: expected "
            f"{RETAINED_HYPOTHESIS_ROW_COUNT} rows, found {len(hypothesis_rows)}"
        )

    inventory = {
        row["inputs"]["account"]: row["outputs"]["data_bytes"]
        for row in rows
        if row["family"] == "landed_account_inventory"
    }
    for name in LANDED_ACCOUNT_ORDER:
        if inventory[name] != constants[ARM_LANDED]["accounts"][name]["bytes"]:
            raise ModelError(f"landed inventory row drifted from the pinned ABI: {name}")
    total = inventory["one_instance_of_every_landed_account"]
    if total != sum(inventory[name] for name in LANDED_ACCOUNT_ORDER):
        raise ModelError("landed one-instance inventory is not the sum of its accounts")

    for row in rows:
        if row["arm"] != ARM_LANDED:
            continue
        outcomes = row["inputs"].get("outcomes")
        if outcomes is not None and outcomes > bounds["max_outcomes"]:
            if row["admission"]["v1_admitted"]:
                raise ModelError("landed arm admitted an outcome count the codec refuses")
            if row["outputs"].get("landed_codec_representable"):
                raise ModelError("landed arm marked an unencodable outcome count representable")
        order_count = row["inputs"].get("order_count")
        if order_count is not None and order_count > bounds["max_epoch_orders"]:
            if row["admission"]["v1_admitted"]:
                raise ModelError("landed arm admitted a book larger than MAX_EPOCH_ORDERS")
        if row["family"] == "landed_batch_relation":
            if row["outputs"]["order_authentications_lower_bound"] != row["inputs"]["order_count"]:
                raise ModelError("landed relation must authenticate every frozen order")
            persistable = min(row["inputs"]["order_count"], bounds["max_portfolio_orders"])
            if row["outputs"]["portfolio_orders_persistable_in_landed_pages"] != persistable:
                raise ModelError("landed page set and relation portfolio capacity disagree")
        if row["family"] == "landed_epoch_book" and row["outputs"]["landed_codec_representable"]:
            pages = row["outputs"]["page_count"]
            per_page = bounds["max_orders_per_page"]
            covered = (pages - 1) * per_page + row["outputs"]["final_page_order_count"]
            if covered != row["inputs"]["order_count"] or not 1 <= row["outputs"][
                "final_page_order_count"
            ] <= per_page:
                raise ModelError("landed page set does not close its order count exactly")

    differential = {row["inputs"]["object"]: row["outputs"] for row in rows if row["arm"] == ARM_DIFFERENTIAL}
    for output in differential.values():
        hypothesis = output["hypothesis_value"]
        landed = output["landed_value"]
        if hypothesis is not None and landed is not None:
            if output["delta"] != landed - hypothesis:
                raise ModelError("differential delta is not the exact integer difference")
        elif output["delta"] is not None:
            raise ModelError("differential delta exists without both arms")


def validate_rows(rows: list[dict[str, Any]], constants: dict[str, Any]) -> None:
    ids = [row["scenario_id"] for row in rows]
    if len(ids) != len(set(ids)):
        raise ModelError("duplicate scenario ID")
    for row in rows:
        if row["schema"] != ROW_SCHEMA:
            raise ModelError("row schema mismatch")
        if row["arm"] not in ARMS:
            raise ModelError(f"row carries no known arm: {row['scenario_id']}")
        outcomes = row["inputs"].get("outcomes")
        if outcomes == 24 and row["admission"]["v1_admitted"]:
            raise ModelError("n=24 must remain a V1 refusal")
        output = row["outputs"]
        if "wire_bytes_measured" in output:
            if output["wire_bytes_measured"] != output["wire_bytes_analytical"]:
                raise ModelError("measured and analytical wire byte count diverged")
        evidence_values = set(row["evidence"].values())
        if "measured_validator_execution" in evidence_values:
            raise ModelError("offline harness cannot emit validator measurements")
        if row["arm"] != ARM_HYPOTHESIS and any(key.endswith("_cu") for key in output):
            raise ModelError(
                f"landed and differential arms must not report compute units: {row['scenario_id']}"
            )

    validate_landed_rows(rows, constants)

    expected_counts = {
        "claim_transition": 5 * 4 * 2,
        "order_page_layout": 5 * 3 * 3,
        "accumulator_fold": 3 * 3 * 2,
        "batch_verification": 5 * 3 * 3 * 2,
        "landed_account_inventory": 17 + 1,
        "landed_epoch_book": 7,
        "landed_intent_wire": 9 * 2,
        "landed_batch_relation": 5 * 3,
        "abi_differential": 12,
    }
    actual_counts = {
        family: sum(1 for row in rows if row["family"] == family)
        for family in expected_counts
    }
    if actual_counts != expected_counts:
        raise ModelError(f"scenario matrix incomplete: {actual_counts}")

    external = {
        (row["inputs"]["outcomes"], row["inputs"]["tx_format"]): row
        for row in rows
        if row["family"] == "claim_transition"
        and row["inputs"]["operation"] == "external_split"
    }
    for outcomes in constants["dragon_design_bounds"]["outcome_axis"]:
        for tx_format in ("legacy_inline", "v0_alt"):
            expected_cpis = outcomes + 1
            if external[(outcomes, tx_format)]["outputs"][
                "token_cpi_count_lower_bound"
            ] != expected_cpis:
                raise ModelError("external split CPI lower bound drift")


def matrix_document(constants: dict[str, Any], rows: list[dict[str, Any]]) -> dict[str, Any]:
    constants_bytes = CONSTANTS_PATH.read_bytes()
    harness_bytes = Path(__file__).read_bytes()
    return {
        "schema": SCHEMA,
        "evidence_ceiling": "offline_wire_measurement_and_analytical_lower_bounds_only",
        "arms": {
            ARM_HYPOTHESIS: {
                "role": "original design sketch, retained unchanged for falsification history",
                "evidence_class": "layout_hypothesis",
                "row_count": sum(1 for row in rows if row["arm"] == ARM_HYPOTHESIS),
            },
            ARM_LANDED: {
                "role": "widths read from the landed codec and relation crate",
                "evidence_class": "landed_codec_constant",
                "source": constants[ARM_LANDED]["source"],
                "row_count": sum(1 for row in rows if row["arm"] == ARM_LANDED),
            },
            ARM_DIFFERENTIAL: {
                "role": "exact integer delta for every object present in both arms",
                "evidence_class": "exact_integer_difference_of_the_two_arms",
                "row_count": sum(1 for row in rows if row["arm"] == ARM_DIFFERENTIAL),
            },
        },
        "source_binding": {
            "cost_lab_py_sha256": sha256_bytes(harness_bytes),
            "constants_json_sha256": sha256_bytes(constants_bytes),
            "external_baseline": constants["source_baseline"],
            "landed_abi_source": constants[ARM_LANDED]["source"],
        },
        "determinism": {
            "randomness": "none",
            "network_calls": 0,
            "rpc_calls": 0,
            "signatures": "fixed_width_placeholders_only",
            "timestamps_in_rows": "none",
        },
        "row_count": len(rows),
        "rows": rows,
    }


CSV_COLUMNS = [
    "scenario_id",
    "arm",
    "family",
    "object",
    "outcomes",
    "operation",
    "tx_format",
    "page_bytes",
    "order_count",
    "summary_kind",
    "wire_bytes_measured",
    "packet_margin_bytes",
    "account_count",
    "account_lock_margin",
    "writable_accounts",
    "token_cpi_count_lower_bound",
    "instruction_trace_entries_lower_bound",
    "cpi_invocation_charge_component_cu",
    "page_count",
    "order_authentications_lower_bound",
    "portfolio_dot_terms_at_fifty_percent",
    "data_bytes",
    "delta",
    "rent_principal_lamports",
    "v1_admitted",
]


def csv_bytes(rows: list[dict[str, Any]]) -> bytes:
    stream = io.StringIO(newline="")
    writer = csv.DictWriter(stream, fieldnames=CSV_COLUMNS, lineterminator="\n")
    writer.writeheader()
    for row in rows:
        inputs = row["inputs"]
        output = row["outputs"]
        rent_value = ""
        for key in (
            "order_page_rent_principal_lamports",
            "half_mix_total_rent_principal_lamports",
            "summary_rent_principal_lamports",
            "outcome_mint_rent_principal_lamports",
            "rent_principal_lamports",
            "landed_rent_principal_lamports",
        ):
            if output.get(key) is not None:
                rent_value = output[key]
                break
        data_bytes = output.get("data_bytes", output.get("landed_value", ""))
        if data_bytes is None:
            data_bytes = ""
        delta_value = output.get("delta", "")
        if delta_value is None:
            delta_value = ""
        writer.writerow(
            {
                "scenario_id": row["scenario_id"],
                "arm": row["arm"],
                "family": row["family"],
                "object": inputs.get("object", inputs.get("account", inputs.get("intent", ""))),
                "outcomes": inputs.get("outcomes", ""),
                "operation": inputs.get("operation", ""),
                "tx_format": inputs.get("tx_format", ""),
                "page_bytes": inputs.get("page_bytes", ""),
                "order_count": inputs.get("order_count", ""),
                "summary_kind": inputs.get("summary_kind", ""),
                "wire_bytes_measured": output.get("wire_bytes_measured", ""),
                "packet_margin_bytes": output.get("packet_margin_bytes", ""),
                "account_count": output.get("account_count", ""),
                "account_lock_margin": output.get("account_lock_margin", ""),
                "writable_accounts": output.get("writable_accounts", ""),
                "token_cpi_count_lower_bound": output.get(
                    "token_cpi_count_lower_bound", ""
                ),
                "instruction_trace_entries_lower_bound": output.get(
                    "instruction_trace_entries_lower_bound", ""
                ),
                "cpi_invocation_charge_component_cu": output.get(
                    "cpi_invocation_charge_component_cu", ""
                ),
                "page_count": output.get("page_count", ""),
                "order_authentications_lower_bound": output.get(
                    "order_authentications_lower_bound", ""
                ),
                "portfolio_dot_terms_at_fifty_percent": output.get(
                    "portfolio_dot_terms_at_fifty_percent", ""
                ),
                "data_bytes": data_bytes,
                "delta": delta_value,
                "rent_principal_lamports": rent_value,
                "v1_admitted": str(row["admission"]["v1_admitted"]).lower(),
            }
        )
    return stream.getvalue().encode()


def find_row(rows: list[dict[str, Any]], scenario_id: str) -> dict[str, Any]:
    for row in rows:
        if row["scenario_id"] == scenario_id:
            return row
    raise ModelError(f"missing summary row: {scenario_id}")


def summary_bytes(constants: dict[str, Any], rows: list[dict[str, Any]]) -> bytes:
    landed = constants[ARM_LANDED]
    lines = [
        "# Deterministic cost-lab summary",
        "",
        "Evidence ceiling: offline synthetic wire measurement plus analytical lower bounds. No SBF, validator, RPC, fee-market, or landing measurement occurred.",
        "",
        f"Arms: `layout_hypothesis` (design sketch, retained) and `abi_landed` (read from `{landed['source']['codec_path']}` at `{landed['source']['commit_short']}`), plus their `abi_differential`. A landed width is an encoding fact, never a measured cost.",
        "",
        "## Claim transition envelope",
        "",
        "| n | external legacy bytes | external v0+ALT bytes | accounts | token CPIs | trace entries | V1 |",
        "|---:|---:|---:|---:|---:|---:|---|",
    ]
    for outcomes in constants["dragon_design_bounds"]["outcome_axis"]:
        legacy = find_row(rows, f"claim-external_split-n{outcomes}-legacy_inline")
        v0 = find_row(rows, f"claim-external_split-n{outcomes}-v0_alt")
        output = legacy["outputs"]
        lines.append(
            "| {n} | {legacy} | {v0} | {accounts} | {cpis} | {trace} | {v1} |".format(
                n=outcomes,
                legacy=output["wire_bytes_measured"],
                v0=v0["outputs"]["wire_bytes_measured"],
                accounts=output["account_count"],
                cpis=output["token_cpi_count_lower_bound"],
                trace=output["instruction_trace_entries_lower_bound"],
                v1="admit" if legacy["admission"]["v1_admitted"] else "refuse",
            )
        )
    lines.extend(
        [
            "",
            "ALT compression changes wire bytes, not logical account locks, CPI work, or the V1 outcome bound. The account topology itself is a Dragon layout hypothesis.",
            "",
            "## 8 KiB page hypothesis at n=16",
            "",
            "| orders | single pages | 50% alternating pages | portfolio pages | package-default rent for 50% mix (lamports) |",
            "|---:|---:|---:|---:|---:|",
        ]
    )
    for orders in constants["dragon_design_bounds"]["order_counts"]:
        row = find_row(rows, f"page-n16-b8192-m{orders}")
        output = row["outputs"]
        lines.append(
            f"| {orders} | {output['single_pages']} | {output['half_mix_pages']} | {output['portfolio_pages']} | {output['half_mix_total_rent_principal_lamports']} |"
        )
    lines.extend(
        [
            "",
            "## Batch verification example: n=16, 512 orders, 8 KiB pages",
            "",
            "| format | pages | all-pages bytes | one-page bytes | wire/account pages per transaction | minimum transactions from wire/accounts only | order authentications | 50% portfolio dot terms |",
            "|---|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for tx_format in ("legacy_inline", "v0_alt"):
        row = find_row(rows, f"batch-n16-b8192-m512-{tx_format}")
        output = row["outputs"]
        lines.append(
            f"| {tx_format} | {output['page_count']} | {output['wire_bytes_measured']} | {output['one_page_wire_bytes_measured']} | {output['pages_per_transaction_wire_and_account_upper_bound']} | {output['minimum_transactions_from_wire_and_account_only']} | {output['order_authentications_lower_bound']} | {output['portfolio_dot_terms_at_fifty_percent']} |"
        )
    lines.extend(
        [
            "",
            "These minimum transaction counts ignore compute. They cannot be used to claim that an all-pages verification will execute or land.",
            "",
            "## Accumulator full-summary fold",
            "",
            "| pages | legacy bytes | v0+ALT bytes | combine steps | summary data bytes | package-default rent (lamports) |",
            "|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for pages in constants["dragon_design_bounds"]["accumulator_page_counts"]:
        legacy = find_row(rows, f"accumulator-full-p{pages}-legacy_inline")
        v0 = find_row(rows, f"accumulator-full-p{pages}-v0_alt")
        output = legacy["outputs"]
        lines.append(
            f"| {pages} | {output['wire_bytes_measured']} | {v0['outputs']['wire_bytes_measured']} | {output['scalar_combine_steps_lower_bound']} | {output['summary_data_bytes']} | {output['summary_rent_principal_lamports']} |"
        )
    lines.extend(
        [
            "",
            "## Landed ABI inventory",
            "",
            f"Source: `{landed['source']['codec_path']}` at `{landed['source']['commit_short']}`, one instance of each account.",
            "",
            "| account | Rust constant | data bytes | package-default rent principal (lamports) |",
            "|---|---|---:|---:|",
        ]
    )
    for name in LANDED_ACCOUNT_ORDER:
        row = find_row(rows, f"landed-account-{name.replace('_', '-')}")
        lines.append(
            f"| {name} | `{row['inputs']['rust_const']}` | {row['outputs']['data_bytes']} |"
            f" {row['outputs']['rent_principal_lamports']} |"
        )
    total_row = find_row(rows, "landed-account-inventory-one-instance")
    lines.append(
        f"| **one instance of each ({total_row['inputs']['account_count']})** | | "
        f"**{total_row['outputs']['data_bytes']}** | "
        f"**{total_row['outputs']['rent_principal_lamports']}** |"
    )
    lines.extend(
        [
            "",
            f"Of that principal, {total_row['outputs']['rent_overhead_component_lamports']} lamports is the per-account 128-byte storage overhead, so account count is a first-class capital term.",
            "",
            "## Landed epoch book",
            "",
            "| orders | representable | pages | padding slot bytes | page rent principal (lamports) | SettlePage instructions |",
            "|---:|---|---:|---:|---:|---:|",
        ]
    )
    for order_count in landed["epoch_book_order_counts"]:
        row = find_row(rows, f"landed-epoch-book-m{order_count}")
        output = row["outputs"]
        if output["landed_codec_representable"]:
            lines.append(
                f"| {order_count} | yes | {output['page_count']} | {output['padding_slot_bytes']} |"
                f" {output['rent_principal_lamports']} | {output['settle_page_instructions_lower_bound']} |"
            )
        else:
            lines.append(
                f"| {order_count} | no: {output['refusal_reason']} | - | - | - | - |"
            )
    lines.extend(
        [
            "",
            "## Landed intent payloads on the wire",
            "",
            "| intent | payload bytes | legacy bytes | v0+ALT bytes | accounts (hypothesis) |",
            "|---|---:|---:|---:|---:|",
        ]
    )
    for name in LANDED_INTENT_ORDER:
        legacy = find_row(rows, f"landed-intent-{name.replace('_', '-')}-legacy_inline")
        v0 = find_row(rows, f"landed-intent-{name.replace('_', '-')}-v0_alt")
        lines.append(
            f"| {name} | {legacy['outputs']['intent_bytes']} |"
            f" {legacy['outputs']['wire_bytes_measured']} |"
            f" {v0['outputs']['wire_bytes_measured']} |"
            f" {legacy['outputs']['account_count']} |"
        )
    lines.extend(
        [
            "",
            "Payload widths are landed; the account sets are hypotheses and are labeled as such in every row.",
            "",
            "## Landed relation at MAX_ORDERS=64",
            "",
            "| n | orders | pages | order authentications | relation steps floor | frozen epoch state bytes | frozen epoch rent principal (lamports) | V1 |",
            "|---:|---:|---:|---:|---:|---:|---:|---|",
        ]
    )
    for outcomes in constants["dragon_design_bounds"]["outcome_axis"]:
        for order_count in landed["relation_order_counts"]:
            row = find_row(rows, f"landed-relation-n{outcomes}-m{order_count}")
            output = row["outputs"]
            lines.append(
                f"| {outcomes} | {order_count} | {output['page_count']} |"
                f" {output['order_authentications_lower_bound']} |"
                f" {output['primitive_relation_steps_floor_excluding_hash_and_allocation']} |"
                f" {output['frozen_epoch_state_bytes']} |"
                f" {output['frozen_epoch_state_rent_principal_lamports']} |"
                f" {'admit' if row['admission']['v1_admitted'] else 'refuse'} |"
            )
    lines.extend(
        [
            "",
            "## Hypothesis versus landed ABI",
            "",
            "| object | unit | hypothesis | landed | delta | what changed |",
            "|---|---|---:|---:|---:|---|",
        ]
    )
    for entry in differential_entries(constants):
        row = find_row(rows, f"diff-{entry['object'].replace('_', '-')}")
        output = row["outputs"]
        rendered = {
            key: ("absent" if output[key] is None else str(output[key]))
            for key in ("hypothesis_value", "landed_value", "delta")
        }
        if output["delta"] is not None and output["delta"] > 0:
            rendered["delta"] = f"+{output['delta']}"
        lines.append(
            f"| {entry['object']} | {entry['unit']} | {rendered['hypothesis_value']} |"
            f" {rendered['landed_value']} | {rendered['delta']} | {output['change']} |"
        )
    lines.extend(
        [
            "",
            "## Interpretation",
            "",
            "- `n=24` is always a V1 refusal even when one synthetic resource axis appears green, and in the landed arm the codec itself refuses it.",
            "- Legacy inline addresses become the first obvious byte bottleneck for broad outcome operations; ALT is not relief from locks or CPIs.",
            "- Rent values are refundable principal under the pinned package default, not fees and not a cluster quote.",
            "- No total-CU number appears because no Dragon SBF program exists to measure. The only CU field is the pinned runtime CPI invocation charge component.",
            "- Batch verification remains Omega(orders) without a separately verified succinct proof; page layout changes rent and transaction partitioning, not that information bound.",
            "- The landed page is not a tunable byte budget: 16 slots per page is forced, so the 4/8/10 KiB page trade in the hypothesis arm no longer describes the current layout.",
            "- One frozen landed book holds 64 orders, so the 128- and 512-order hypothesis rows describe several epochs rather than one relation instance.",
            f"- Portfolio orders have a persisted page encoding: one {landed['bounds']['order_slot_bytes']}-byte tagged slot holds either order family and a retirement, so the seam `relation_v1` opened against the page is closed. The price is a common slot width, which is the whole of the page growth from {V2_SINGLE_FAMILY_PAGE_BYTES} to {landed['accounts']['order_page']['bytes']} bytes.",
            "- No landed candidate-verification instruction exists, so the landed arm reports relation work and rent without any wire byte count for that step.",
            "",
        ]
    )
    return "\n".join(lines).encode()


def rendered_artifacts(constants: dict[str, Any], rows: list[dict[str, Any]]) -> dict[str, bytes]:
    primary = {
        "matrix.json": canonical_json_bytes(matrix_document(constants, rows)),
        "matrix.csv": csv_bytes(rows),
        "SUMMARY.md": summary_bytes(constants, rows),
    }
    checksum_lines = [
        f"{sha256_bytes(primary[name])}  {name}" for name in sorted(primary)
    ]
    primary["checksums.sha256"] = ("\n".join(checksum_lines) + "\n").encode()
    return primary


def write_artifacts(output: Path, artifacts: dict[str, bytes]) -> None:
    output.mkdir(parents=True, exist_ok=True)
    expected = set(artifacts)
    existing = {path.name for path in output.iterdir() if path.is_file()}
    unexpected = existing - expected
    if unexpected:
        raise ModelError(f"refusing to overwrite directory with unexpected files: {sorted(unexpected)}")
    for name, payload in artifacts.items():
        (output / name).write_bytes(payload)


def check_artifacts(output: Path, artifacts: dict[str, bytes]) -> None:
    errors: list[str] = []
    for name, expected in artifacts.items():
        path = output / name
        if not path.is_file():
            errors.append(f"missing {path}")
            continue
        actual = path.read_bytes()
        if actual != expected:
            errors.append(
                f"drift {path}: expected sha256={sha256_bytes(expected)} actual sha256={sha256_bytes(actual)}"
            )
    if errors:
        raise ModelError("\n".join(errors))


# The offline ABI audit: re-derive the landed arm from the codec source on disk.
#
# Everything below is a pure function of (pinned constants, codec source text) except
# `abi_audit`, which reads the one file and reports. The audit is a tripwire, so its only two
# outcomes are "no drift" and a *named* drift list: an expression the evaluator cannot read, a
# module it cannot find, or an identifier the lab does not pin are all drift entries that say
# what moved and how to re-pin it, never a bare crash. `refusing to evaluate unknown token`
# raised as an exception once left this gate dead for several commits without anyone noticing.
PIN_TABLE = "RUST_IDENTIFIER_VALUES in benchmarks/cost_lab.py"

RUST_IDENTIFIER_VALUES = {
    "CLEAR_WORK_BODY_BYTES": 48_592,
    "HASH_BYTES": 32,
    "MAX_EPOCH_ORDERS": 64,
    "MAX_GRID_TICKS": 64,
    "MAX_INTENT_BYTES": 310,
    "MAX_KNOTS": 16,
    "MAX_ORDERS_PER_PAGE": 16,
    "MAX_ORDER_PAGES": 4,
    "MAX_OUTCOMES": 16,
    "MAX_PAYOUTS": 8,
    "MAX_PORTFOLIO_ORDERS": 8,
    "MAX_SLICES": 416,
    "ORDER_RECORD_BYTES": 107,
    "ORDER_SLOT_BYTES": 236,
    "PORTFOLIO_RECORD_BYTES": 235,
    "TOMBSTONE_RECORD_BYTES": 80,
}

# These paths deliberately name the only cross-module constants that the root codec uses in
# widths audited here. They are an allowlist, not a Rust evaluator: a new path must be
# explicitly reviewed and pinned before the audit can use it.
EXPLICIT_RUST_PATH_VALUES = {
    "artifact::ARTIFACT_CHUNK_BYTES": 192,
    "resolution_work::ABORT_RESOLUTION_WORK_BYTES": 74,
    "resolution_work::BEGIN_RESOLUTION_WORK_BYTES": 83,
    "resolution_work::FINALIZE_RESOLUTION_WORK_BYTES": 74,
    "resolution_work::FOLD_RESOLUTION_WORK_BYTES": 107,
    "super::clearing::PAIRING_SLICE_BYTES": 13,
}
ALL_RUST_IDENTIFIER_VALUES = RUST_IDENTIFIER_VALUES | EXPLICIT_RUST_PATH_VALUES

# Intent delegates are not arithmetic expressions. Resolve only these exact, reviewed method
# calls to the one constant that each delegated codec promises to return.
INTENT_DELEGATE_EXPRESSIONS = {
    "AbortResolutionWork": "resolution_work::ABORT_RESOLUTION_WORK_BYTES",
    "BeginResolutionWork": "resolution_work::BEGIN_RESOLUTION_WORK_BYTES",
    "FinalizeResolutionWork": "resolution_work::FINALIZE_RESOLUTION_WORK_BYTES",
    "FoldResolutionWork": "resolution_work::FOLD_RESOLUTION_WORK_BYTES",
}

BOUNDS_IDENTIFIERS = {
    "hash_bytes": "HASH_BYTES",
    "intent_version": "INTENT_VERSION",
    "intent_version_v1": "INTENT_VERSION_V1",
    "layout_version": "LAYOUT_VERSION",
    "layout_version_v1": "LAYOUT_VERSION_V1",
    "layout_version_v2": "LAYOUT_VERSION_V2",
    "layout_version_v3": "LAYOUT_VERSION_V3",
    "max_epoch_orders": "MAX_EPOCH_ORDERS",
    "max_grid_ticks": "MAX_GRID_TICKS",
    "max_intent_bytes": "MAX_INTENT_BYTES",
    "max_order_pages": "MAX_ORDER_PAGES",
    "max_orders_per_page": "MAX_ORDERS_PER_PAGE",
    "max_outcomes": "MAX_OUTCOMES",
    "max_portfolio_orders": "MAX_PORTFOLIO_ORDERS",
    "order_record_bytes": "ORDER_RECORD_BYTES",
    "order_slot_bytes": "ORDER_SLOT_BYTES",
    "portfolio_record_bytes": "PORTFOLIO_RECORD_BYTES",
    "tombstone_record_bytes": "TOMBSTONE_RECORD_BYTES",
}

BOUNDS_RECORD_TERMS = {
    "max_intent_field_terms": "MAX_INTENT_BYTES",
    "order_record_field_terms": "ORDER_RECORD_BYTES",
    "order_slot_field_terms": "ORDER_SLOT_BYTES",
    "portfolio_record_field_terms": "PORTFOLIO_RECORD_BYTES",
    "tombstone_record_field_terms": "TOMBSTONE_RECORD_BYTES",
}

RUST_INTEGER_SUFFIXES = ("usize", "u128", "u64", "u32", "u16", "u8")


class UnknownRustToken(ModelError):
    """A token in a codec expression that the pin table does not define.

    Carried as a value rather than a bare refusal so the audit can turn it into a drift line
    that names the token, the constant that referenced it, and the table to add it to.
    """

    def __init__(self, token: str, expression: str) -> None:
        self.token = token
        self.expression = expression.strip()
        super().__init__(
            f"refusing to evaluate unknown token in ABI expression: {token} "
            f"(in `{self.expression}`)"
        )


def strip_rust_comments(source: str) -> str:
    """Blank every comment and string body, preserving offsets and line structure.

    The declaration scanners below are regular expressions over this text, so a `pub const`
    inside a doc comment, a block comment or a string literal must not be readable as a
    declaration. Nested block comments are Rust-legal and are counted.
    """

    out: list[str] = []
    index = 0
    length = len(source)
    depth = 0
    while index < length:
        char = source[index]
        following = source[index + 1] if index + 1 < length else ""
        if depth:
            if char == "/" and following == "*":
                depth += 1
                out.append("  ")
                index += 2
                continue
            if char == "*" and following == "/":
                depth -= 1
                out.append("  ")
                index += 2
                continue
            out.append("\n" if char == "\n" else " ")
            index += 1
            continue
        if char == "/" and following == "*":
            depth = 1
            out.append("  ")
            index += 2
            continue
        if char == "/" and following == "/":
            while index < length and source[index] != "\n":
                out.append(" ")
                index += 1
            continue
        if char == '"':
            out.append('"')
            index += 1
            while index < length:
                inner = source[index]
                if inner == "\\" and index + 1 < length:
                    out.append("  ")
                    index += 2
                    continue
                if inner == '"':
                    out.append('"')
                    index += 1
                    break
                out.append("\n" if inner == "\n" else " ")
                index += 1
            continue
        out.append(char)
        index += 1
    return "".join(out)


TOP_LEVEL_CONST_PATTERN = re.compile(
    r"^(?:pub(?:\([^)]*\))? )?const (?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:\s*[^=;]+=\s*(?P<expr>[^;]*);",
    re.MULTILINE,
)
SCOPED_CONST_PATTERN = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))? )?const (?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*:\s*[^=;]+=\s*(?P<expr>[^;]*);",
    re.MULTILINE,
)


def rust_top_level_constants(stripped: str) -> dict[str, str]:
    """Every column-zero `const NAME: T = expr;` declaration, expression text unevaluated.

    The expression pattern runs to the terminating semicolon rather than to end of line, so a
    rustfmt-wrapped multi-line declaration reads exactly like a one-line one. The audit used to
    be line-oriented here and died with a bare SyntaxError on a wrapped constant.
    """

    return {
        match.group("name"): match.group("expr")
        for match in TOP_LEVEL_CONST_PATTERN.finditer(stripped)
        if match.group("name") != "_"
    }


def rust_block_body(stripped: str, marker: str, what: str) -> str:
    start = stripped.find(marker)
    if start < 0:
        raise ModelError(f"codec source has no {what} (looked for `{marker.strip()}`)")
    index = start + len(marker)
    depth = 1
    while index < len(stripped) and depth:
        if stripped[index] == "{":
            depth += 1
        elif stripped[index] == "}":
            depth -= 1
        index += 1
    if depth:
        raise ModelError(f"codec source {what} is never brace-closed")
    return stripped[start + len(marker) : index - 1]


def rust_module_constants(stripped: str, module: str) -> dict[str, str]:
    body = rust_block_body(stripped, f"pub mod {module} {{", f"a `{module}` module")
    return {
        match.group("name"): match.group("expr")
        for match in SCOPED_CONST_PATTERN.finditer(body)
        if match.group("name") != "_"
    }


def rust_additive_terms(expression: str) -> list[str]:
    """Split a size expression into its top-level `+` terms, parentheses intact.

    The landed arm stores each width as the codec's own field terms rather than a total, so the
    audit has to see the same decomposition the codec wrote: a re-pin that lumps two fields into
    one term still sums correctly and would otherwise pass unnoticed.
    """

    terms: list[str] = []
    depth = 0
    current = ""
    for char in expression:
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth < 0:
                raise ModelError(f"unbalanced parentheses in ABI expression `{expression.strip()}`")
        if char == "+" and depth == 0:
            terms.append(current)
            current = ""
            continue
        current += char
    if depth:
        raise ModelError(f"unbalanced parentheses in ABI expression `{expression.strip()}`")
    terms.append(current)
    return [term.strip() for term in terms]


def normalized_rust_expression(expression: str) -> str:
    return " ".join(expression.split())


def rust_tokens(expression: str) -> list[str]:
    spaced = expression
    for symbol in "()+*":
        spaced = spaced.replace(symbol, f" {symbol} ")
    return spaced.split()


def rust_integer_literal(token: str) -> int | None:
    body = token
    for suffix in RUST_INTEGER_SUFFIXES:
        if body.endswith(suffix):
            body = body[: -len(suffix)]
            break
    body = body.rstrip("_").replace("_", "")
    return int(body) if body.isdigit() else None


def rust_expression_identifiers(expression: str) -> list[str]:
    """Identifiers a size expression depends on, in first-appearance order."""

    seen: list[str] = []
    for token in rust_tokens(expression):
        if token in {"(", ")", "+", "*"} or rust_integer_literal(token) is not None:
            continue
        if token not in seen:
            seen.append(token)
    return seen


def evaluate_rust_arithmetic(expression: str, environment: dict[str, int] | None = None) -> int:
    """Evaluate one `+`/`*`/parenthesis Rust size expression over an identifier environment.

    Anything else refuses. This exists to re-derive `account_len` from the codec instead of
    trusting a transcription, so it must never become a general evaluator. The environment
    defaults to the pinned table; the audit passes the codec's own derived values instead, so a
    stale pin cannot be substituted into a width and hide itself.
    """

    values = ALL_RUST_IDENTIFIER_VALUES if environment is None else environment
    parts = rust_tokens(expression)
    if not parts:
        raise ModelError(
            "refusing to evaluate an empty ABI expression: the declaration parsed to no terms"
        )
    rendered: list[str] = []
    for part in parts:
        if part in {"(", ")", "+", "*"}:
            rendered.append(part)
            continue
        literal = rust_integer_literal(part)
        if literal is not None:
            rendered.append(str(literal))
            continue
        if part in values:
            rendered.append(str(values[part]))
            continue
        raise UnknownRustToken(part, expression)
    try:
        node = compile(" ".join(rendered), "<abi>", "eval")
    except SyntaxError as exc:
        raise ModelError(
            f"refusing to evaluate malformed ABI expression `{expression.strip()}`: {exc.msg}"
        ) from exc
    if node.co_names:  # pragma: no cover - defensive, every name was already substituted
        raise ModelError(f"ABI expression referenced names: {sorted(node.co_names)}")
    value = eval(node, {"__builtins__": {}}, {})  # noqa: S307 - digits and + * only
    if not isinstance(value, int):
        raise ModelError("ABI expression did not evaluate to an integer")
    return value


def resolve_rust_constant(
    name: str,
    declarations: dict[str, str],
    resolved: dict[str, int],
    pending: tuple[str, ...] = (),
) -> int:
    """Evaluate one declared constant from the codec's own declarations, dependencies first."""

    if name in resolved:
        return resolved[name]
    if name in EXPLICIT_RUST_PATH_VALUES:
        return EXPLICIT_RUST_PATH_VALUES[name]
    if name in pending:
        raise ModelError(f"codec constant {name} is defined in terms of itself")
    if name not in declarations:
        raise UnknownRustToken(name, f"<no `const {name}` declaration in the codec source>")
    expression = declarations[name]
    environment: dict[str, int] = {}
    for identifier in rust_expression_identifiers(expression):
        environment[identifier] = resolve_rust_constant(
            identifier, declarations, resolved, pending + (name,)
        )
    value = evaluate_rust_arithmetic(expression, environment)
    resolved[name] = value
    return value


def resolve_expression_from_source(
    expression: str, declarations: dict[str, str], resolved: dict[str, int]
) -> int:
    environment = {
        identifier: resolve_rust_constant(identifier, declarations, resolved)
        for identifier in rust_expression_identifiers(expression)
    }
    return evaluate_rust_arithmetic(expression, environment)


def derive_identifier_values_from_source(
    source: str,
) -> tuple[dict[str, int], dict[str, str]]:
    """Re-derive every pinned Rust identifier from its own declaration in the codec.

    `RUST_IDENTIFIER_VALUES` is the table every codec expression is evaluated against, so a
    stale pin there would move every derived width in lockstep and an account-level comparison
    would not notice. Reading each name back from the crate's own declaration closes that: an
    identifier is only ever trusted after the codec restates it, and a name the codec derives
    from another name (`ORDER_SLOT_BYTES = 1 + PORTFOLIO_RECORD_BYTES`) is resolved from the
    codec's value of that other name, never from the lab's pin of it.
    """

    declarations = rust_top_level_constants(strip_rust_comments(source))
    resolved: dict[str, int] = {}
    values: dict[str, int] = {}
    failures: dict[str, str] = {}
    for name in sorted(RUST_IDENTIFIER_VALUES):
        try:
            values[name] = resolve_rust_constant(name, declarations, resolved)
        except ModelError as exc:
            failures[name] = str(exc)
    return values, failures


def derive_pinned_identifiers_from_source(source: str) -> dict[str, int]:
    values, failures = derive_identifier_values_from_source(source)
    if failures:
        raise ModelError(
            "codec source does not declare every pinned identifier: "
            + "; ".join(f"{name}: {reason}" for name, reason in sorted(failures.items()))
        )
    return values


def derive_account_lengths_from_source(source: str) -> dict[str, int]:
    """Every `account_len` constant, evaluated over the codec's own identifier values."""

    stripped = strip_rust_comments(source)
    declarations = rust_top_level_constants(stripped)
    resolved: dict[str, int] = {}
    derived: dict[str, int] = {}
    for name, expression in rust_module_constants(stripped, "account_len").items():
        derived[name] = resolve_expression_from_source(expression, declarations, resolved)
    return derived


INTENT_ARM_PATTERN = re.compile(r"^\s*(?P<patterns>[^=]*?)\s*=>\s*(?P<value>.+?),?\s*$")
VARIANT_PATTERN = re.compile(r"(?P<enum>Self|OrderSlot)::(?P<variant>[A-Za-z_][A-Za-z0-9_]*)")


def derive_intent_lengths_from_source(source: str) -> dict[str, str]:
    """Map every `Intent::encoded_len` match arm to its size expression.

    A placement's width depends on which slot kind it carries, so the nested `match slot` arms
    are keyed `PlaceOrder.Single`, `PlaceOrder.Portfolio` and so on. An arm whose value is not
    a size expression (the nested `match` itself) contributes no width and is skipped; a pinned
    intent whose key is therefore missing becomes a named drift entry rather than silence.
    """

    stripped = strip_rust_comments(source)
    body = rust_block_body(
        stripped,
        "pub const fn encoded_len(&self) -> usize {",
        "an `Intent::encoded_len` function",
    )
    arms: dict[str, str] = {}
    outer = ""
    for line in body.splitlines():
        match = INTENT_ARM_PATTERN.match(line)
        if match is None:
            continue
        variants = VARIANT_PATTERN.findall(match.group("patterns"))
        if not variants:
            continue
        value = match.group("value").strip()
        if value.endswith("{"):
            outer = variants[0][1] if variants[0][0] == "Self" else outer
            continue
        for enum_name, variant in variants:
            key = variant if enum_name == "Self" else f"{outer}.{variant}"
            if enum_name == "Self" and value == "value.encoded_len()":
                arms[key] = INTENT_DELEGATE_EXPRESSIONS.get(key, value)
            else:
                arms[key] = value
    return arms


def intent_source_key(intent: dict[str, Any]) -> str:
    variant = intent["rust_variant"]
    kind = intent.get("rust_slot_kind")
    return f"{variant}.{kind}" if kind else variant


def cross_check_expression(
    label: str,
    expression: str,
    declarations: dict[str, str],
    resolved: dict[str, int],
    drift: list[str],
) -> int | None:
    """Compare every identifier a codec expression references against its pin, then evaluate.

    Both halves are reported. Substituting the pinned value of a referenced identifier would
    make a lockstep move invisible: with `ORDER_SLOT_BYTES` pinned at 228 while the codec said
    236, an `ORDER_PAGE` re-derived over the pins moved by one byte instead of 129, and the
    audit called that no drift.
    """

    unresolved = False
    for identifier in rust_expression_identifiers(expression):
        try:
            codec_value = resolve_rust_constant(identifier, declarations, resolved)
        except ModelError as exc:
            drift.append(f"{label} references {identifier}, which this audit cannot resolve: {exc}")
            unresolved = True
            continue
        pinned = ALL_RUST_IDENTIFIER_VALUES.get(identifier)
        if pinned is None:
            drift.append(
                f"{label} references {identifier}, which the cost lab does not pin: "
                f"the codec declares it as {codec_value}; add "
                f'"{identifier}": {codec_value} to {PIN_TABLE} and re-run abi-audit'
            )
        elif pinned != codec_value:
            drift.append(
                f"{label} references {identifier}: codec says {codec_value}, "
                f"cost lab pins {pinned}"
            )
    if unresolved:
        return None
    try:
        return resolve_expression_from_source(expression, declarations, resolved)
    except ModelError as exc:
        drift.append(f"{label}: this audit cannot evaluate the codec expression: {exc}")
        return None


def cross_check_terms(
    label: str,
    expression: str,
    pinned: dict[str, Any],
    terms_key: str,
    declarations: dict[str, str],
    resolved: dict[str, int],
    drift: list[str],
) -> None:
    """Hold the pinned formula text and field terms to the codec's own decomposition."""

    codec_formula = normalized_rust_expression(expression)
    if pinned.get("formula") != codec_formula:
        drift.append(
            f"{label}: codec expression is `{codec_formula}`, cost lab pins "
            f"`{pinned.get('formula')}`"
        )
    try:
        codec_terms = [
            resolve_expression_from_source(term, declarations, resolved)
            for term in rust_additive_terms(expression)
        ]
    except ModelError as exc:
        drift.append(f"{label}: this audit cannot decompose the codec expression: {exc}")
        return
    if pinned.get(terms_key) != codec_terms:
        drift.append(
            f"{label}: codec field terms are {codec_terms}, cost lab pins "
            f"{pinned.get(terms_key)}"
        )


def abi_drift(constants: dict[str, Any], source: str) -> list[str]:
    """Every way the pinned landed arm disagrees with this codec source, as named lines."""

    landed = constants[ARM_LANDED]
    bounds = landed["bounds"]
    drift: list[str] = []
    stripped = strip_rust_comments(source)
    try:
        declarations = rust_top_level_constants(stripped)
    except ModelError as exc:  # pragma: no cover - defensive, the scanner cannot raise today
        return [f"codec source is unreadable to this audit: {exc}"]
    resolved: dict[str, int] = {}

    identifiers, failures = derive_identifier_values_from_source(source)
    for name, pinned in sorted(RUST_IDENTIFIER_VALUES.items()):
        if name in failures:
            drift.append(
                f"{name}: the cost lab pins {pinned} and this audit cannot read it back from "
                f"the codec ({failures[name]}); re-pin or drop the entry in {PIN_TABLE}"
            )
        elif identifiers[name] != pinned:
            drift.append(f"{name}: codec says {identifiers[name]}, cost lab pins {pinned}")

    for key, name in sorted(BOUNDS_IDENTIFIERS.items()):
        if key not in bounds:
            drift.append(
                f"bounds.{key} is gone from constants.json and the audit still cross-checks "
                f"it against {name}"
            )
            continue
        try:
            codec_value = resolve_rust_constant(name, declarations, resolved)
        except ModelError as exc:
            drift.append(f"bounds.{key}: the codec no longer declares {name} readably ({exc})")
            continue
        if bounds[key] != codec_value:
            drift.append(
                f"bounds.{key}: codec says {codec_value} for {name}, "
                f"constants.json pins {bounds[key]}"
            )

    for key, name in sorted(BOUNDS_RECORD_TERMS.items()):
        if name not in declarations:
            drift.append(f"bounds.{key}: the codec no longer declares {name}")
            continue
        codec_terms_ok = key in bounds
        if not codec_terms_ok:
            drift.append(
                f"bounds.{key} is gone from constants.json and the audit still cross-checks it "
                f"against {name}"
            )
            continue
        try:
            codec_terms = [
                resolve_expression_from_source(term, declarations, resolved)
                for term in rust_additive_terms(declarations[name])
            ]
        except ModelError as exc:
            drift.append(f"bounds.{key}: this audit cannot decompose {name}: {exc}")
            continue
        if bounds[key] != codec_terms:
            drift.append(
                f"bounds.{key}: codec field terms for {name} are {codec_terms}, "
                f"constants.json pins {bounds[key]}"
            )

    try:
        lengths = rust_module_constants(stripped, "account_len")
    except ModelError as exc:
        drift.append(f"account_len is unreadable to this audit: {exc}")
        lengths = {}
    try:
        versions = rust_module_constants(stripped, "account_version")
    except ModelError as exc:
        drift.append(f"account_version is unreadable to this audit: {exc}")
        versions = {}

    for name in LANDED_ACCOUNT_ORDER:
        account = landed["accounts"][name]
        rust_name = account["rust_const"].split("::", 1)[1]
        if rust_name not in lengths:
            drift.append(f"{name}: {account['rust_const']} is gone from the codec")
        else:
            derived = cross_check_expression(
                account["rust_const"], lengths[rust_name], declarations, resolved, drift
            )
            if derived is not None and derived != account["bytes"]:
                drift.append(
                    f"{name}: codec says {derived} bytes, cost lab pins {account['bytes']}"
                )
            cross_check_terms(
                account["rust_const"],
                lengths[rust_name],
                account,
                "field_terms",
                declarations,
                resolved,
                drift,
            )
        if rust_name not in versions:
            drift.append(f"{name}: account_version::{rust_name} is gone from the codec")
        else:
            derived_version = cross_check_expression(
                f"account_version::{rust_name}", versions[rust_name], declarations, resolved, drift
            )
            if derived_version is not None and derived_version != account["schema_version"]:
                drift.append(
                    f"{name}: codec writes schema version {derived_version}, "
                    f"cost lab pins {account['schema_version']}"
                )
        tag_name = f"{rust_name}_TAG"
        if tag_name not in declarations:
            drift.append(f"{name}: the codec has no `const {tag_name}` discriminator")
        else:
            derived_tag = cross_check_expression(
                tag_name, declarations[tag_name], declarations, resolved, drift
            )
            if derived_tag is not None and derived_tag != account["discriminator_tag"]:
                drift.append(
                    f"{name}: codec discriminator {tag_name} is {derived_tag}, "
                    f"cost lab pins {account['discriminator_tag']}"
                )

    extra = set(lengths) - {
        landed["accounts"][name]["rust_const"].split("::", 1)[1] for name in LANDED_ACCOUNT_ORDER
    }
    for name in sorted(extra):
        drift.append(f"account_len::{name} exists in the codec and is absent from the cost lab")

    try:
        arms = derive_intent_lengths_from_source(source)
    except ModelError as exc:
        drift.append(f"Intent::encoded_len is unreadable to this audit: {exc}")
        arms = {}
    widest = 0
    for name in LANDED_INTENT_ORDER:
        intent = landed["intents"][name]
        unpinned = [key for key in ("rust_variant", "rust_tag_const") if key not in intent]
        if unpinned:
            drift.append(
                f"intent {name}: constants.json pins no {' or '.join(unpinned)}, so this audit "
                "cannot read the intent back from the codec at all"
            )
            continue
        key = intent_source_key(intent)
        if key not in arms:
            drift.append(
                f"intent {name}: the codec has no readable `Intent::{key}` encoded_len arm, so "
                "this audit cannot verify its width; re-pin rust_variant/rust_slot_kind in "
                "benchmarks/constants.json"
            )
        else:
            derived = cross_check_expression(
                f"Intent::{key} encoded_len", arms[key], declarations, resolved, drift
            )
            if derived is not None and derived != intent["bytes"]:
                drift.append(
                    f"intent {name}: codec says {derived} bytes, cost lab pins {intent['bytes']}"
                )
            cross_check_terms(
                f"Intent::{key} encoded_len",
                arms[key],
                intent,
                "field_terms",
                declarations,
                resolved,
                drift,
            )
        tag_name = intent["rust_tag_const"]
        if tag_name not in declarations:
            drift.append(f"intent {name}: the codec has no `const {tag_name}` discriminator")
        else:
            derived_tag = cross_check_expression(
                tag_name, declarations[tag_name], declarations, resolved, drift
            )
            if derived_tag is not None and derived_tag != intent["intent_tag"]:
                drift.append(
                    f"intent {name}: codec discriminator {tag_name} is {derived_tag}, "
                    f"cost lab pins {intent['intent_tag']}"
                )
        pinned_version = bounds.get("intent_version")
        if pinned_version is not None and intent["intent_version"] != pinned_version:
            drift.append(
                f"intent {name}: pinned version {intent['intent_version']} is not the pinned "
                f"INTENT_VERSION {pinned_version}"
            )
    for key, expression in sorted(arms.items()):
        try:
            widest = max(widest, resolve_expression_from_source(expression, declarations, resolved))
        except ModelError as exc:
            drift.append(f"Intent::{key} encoded_len is not a size expression to this audit: {exc}")
    ceiling = identifiers.get("MAX_INTENT_BYTES")
    if arms and ceiling is not None and widest != ceiling:
        drift.append(
            "MAX_INTENT_BYTES is not the widest admitted intent: the widest encoded_len arm is "
            f"{widest} and the codec declares the ceiling as {ceiling}"
        )
    return drift


def landed_codec_path(constants: dict[str, Any]) -> Path:
    return REPO_ROOT / constants[ARM_LANDED]["source"]["codec_path"]


def abi_audit(constants: dict[str, Any]) -> tuple[list[str], list[str]]:
    """Compare the pinned landed arm against the codec on disk. Not part of golden closure."""

    landed = constants[ARM_LANDED]
    source_path = landed_codec_path(constants)
    notes = [f"pinned commit: {landed['source']['commit']}"]
    if not source_path.is_file():
        return notes + [f"codec source not present at {source_path}; audit skipped"], []
    source = source_path.read_text()
    digest = sha256_bytes(source_path.read_bytes())
    if digest == landed["source"]["codec_blob_sha256_at_commit"]:
        notes.append("working-tree codec digest equals the pinned blob")
    else:
        notes.append(
            "working-tree codec digest differs from the pinned blob "
            f"({digest}); widths are re-derived below"
        )
    drift = abi_drift(constants, source)
    for key, path_key in (
        ("relation_blob_sha256_at_commit", "relation_path"),
        ("relation_v1_blob_sha256_at_commit", "relation_v1_path"),
    ):
        relation_path = REPO_ROOT / landed["source"][path_key]
        if not relation_path.is_file():
            drift.append(f"{landed['source'][path_key]} is gone and the arm still pins its bounds")
            continue
        relation_digest = sha256_bytes(relation_path.read_bytes())
        if relation_digest != landed["source"][key]:
            drift.append(
                f"{landed['source'][path_key]}: blob moved to {relation_digest}; this audit does "
                "not re-derive relation bounds, so re-verify them by hand and re-pin the digest"
            )
    counted = []
    for label, reader in (
        ("pinned identifiers", lambda: derive_identifier_values_from_source(source)[0]),
        ("account_len constants", lambda: derive_account_lengths_from_source(source)),
        ("Intent::encoded_len arms", lambda: derive_intent_lengths_from_source(source)),
    ):
        try:
            counted.append(f"{len(reader())} {label}")
        except ModelError:
            counted.append(f"no readable {label}")
    notes.append("re-derived " + ", ".join(counted) + " from the codec source")
    notes.append(
        "relation bounds are pinned from crates/clutch-batch and are not re-derived here; "
        "their blob digests are checked instead"
    )
    return notes, drift


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    generate = subparsers.add_parser("generate", help="write deterministic golden artifacts")
    generate.add_argument("--output", type=Path, default=DEFAULT_GOLDEN)
    check = subparsers.add_parser("check", help="validate model and checked-in artifacts")
    check.add_argument("--output", type=Path, default=DEFAULT_GOLDEN)
    subparsers.add_parser("summary", help="print the deterministic derived summary")
    subparsers.add_parser(
        "abi-audit",
        help="re-derive the landed ABI from the codec on disk and report drift",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    constants = load_constants()
    if args.command == "abi-audit":
        notes, drift = abi_audit(constants)
        for note in notes:
            print(note)
        for item in drift:
            print(f"drift: {item}")
        if drift:
            raise ModelError(
                f"landed ABI arm is stale: {len(drift)} drift "
                f"{'line' if len(drift) == 1 else 'lines'} above, each naming what moved; re-pin "
                "benchmarks/constants.json (and RUST_IDENTIFIER_VALUES where a line says so) and "
                "regenerate the goldens"
            )
        if not landed_codec_path(constants).is_file():
            print("abi-audit skipped: the pinned codec source is not present in this checkout")
            return 0
        print("abi-audit passed: landed arm equals the codec on disk")
        return 0
    rows = generate_rows(constants)
    artifacts = rendered_artifacts(constants, rows)
    if args.command == "generate":
        write_artifacts(args.output, artifacts)
        print(f"generated {len(rows)} scenarios in {args.output}")
    elif args.command == "check":
        check_artifacts(args.output, artifacts)
        print(f"check passed: {len(rows)} scenarios; offline; rpc=0; validator=0")
    elif args.command == "summary":
        sys.stdout.buffer.write(artifacts["SUMMARY.md"])
    else:  # pragma: no cover - argparse makes this unreachable.
        raise ModelError(f"unhandled command: {args.command}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ModelError as exc:
        print(f"cost-lab error: {exc}", file=sys.stderr)
        raise SystemExit(2)
