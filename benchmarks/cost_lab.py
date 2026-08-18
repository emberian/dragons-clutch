#!/usr/bin/env python3
"""Deterministic offline cost and wire-layout laboratory for Dragon's Clutch.

This module intentionally has no Solana SDK, RPC, signing, or deployment surface. It emits
synthetic transaction byte strings with correct legacy/v0 framing and reports analytical topology
and information lower bounds separately from those local byte measurements.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import math
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


SCHEMA = "dragons.cost_lab.matrix.v1"
ROW_SCHEMA = "dragons.cost_lab.row.v1"
ROOT = Path(__file__).resolve().parent
CONSTANTS_PATH = ROOT / "constants.json"
DEFAULT_GOLDEN = ROOT / "golden"


class ModelError(ValueError):
    """Raised when a scenario cannot be represented by the bounded synthetic model."""


def canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def load_constants() -> dict[str, Any]:
    with CONSTANTS_PATH.open("rb") as handle:
        value = json.load(handle)
    if value.get("schema") != "dragons.cost_lab.constants.v1":
        raise ModelError("unknown constants schema")
    return value


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


def generate_rows(constants: dict[str, Any]) -> list[dict[str, Any]]:
    rows = claim_rows(constants) + page_rows(constants) + accumulator_rows(constants) + batch_rows(constants)
    rows.sort(key=lambda row: row["scenario_id"])
    validate_rows(rows, constants)
    return rows


def validate_rows(rows: list[dict[str, Any]], constants: dict[str, Any]) -> None:
    ids = [row["scenario_id"] for row in rows]
    if len(ids) != len(set(ids)):
        raise ModelError("duplicate scenario ID")
    for row in rows:
        if row["schema"] != ROW_SCHEMA:
            raise ModelError("row schema mismatch")
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

    expected_counts = {
        "claim_transition": 5 * 4 * 2,
        "order_page_layout": 5 * 3 * 3,
        "accumulator_fold": 3 * 3 * 2,
        "batch_verification": 5 * 3 * 3 * 2,
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
        "source_binding": {
            "cost_lab_py_sha256": sha256_bytes(harness_bytes),
            "constants_json_sha256": sha256_bytes(constants_bytes),
            "external_baseline": constants["source_baseline"],
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
    "family",
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
        rent_value = output.get(
            "order_page_rent_principal_lamports",
            output.get(
                "half_mix_total_rent_principal_lamports",
                output.get(
                    "summary_rent_principal_lamports",
                    output.get("outcome_mint_rent_principal_lamports", ""),
                ),
            ),
        )
        writer.writerow(
            {
                "scenario_id": row["scenario_id"],
                "family": row["family"],
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
    lines = [
        "# Deterministic cost-lab summary",
        "",
        "Evidence ceiling: offline synthetic wire measurement plus analytical lower bounds. No SBF, validator, RPC, fee-market, or landing measurement occurred.",
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
            "## Interpretation",
            "",
            "- `n=24` is always a V1 refusal even when one synthetic resource axis appears green.",
            "- Legacy inline addresses become the first obvious byte bottleneck for broad outcome operations; ALT is not relief from locks or CPIs.",
            "- Rent values are refundable principal under the pinned package default, not fees and not a cluster quote.",
            "- No total-CU number appears because no Dragon SBF program exists to measure. The only CU field is the pinned runtime CPI invocation charge component.",
            "- Batch verification remains Omega(orders) without a separately verified succinct proof; page layout changes rent and transaction partitioning, not that information bound.",
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


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    generate = subparsers.add_parser("generate", help="write deterministic golden artifacts")
    generate.add_argument("--output", type=Path, default=DEFAULT_GOLDEN)
    check = subparsers.add_parser("check", help="validate model and checked-in artifacts")
    check.add_argument("--output", type=Path, default=DEFAULT_GOLDEN)
    subparsers.add_parser("summary", help="print the deterministic derived summary")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    constants = load_constants()
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
