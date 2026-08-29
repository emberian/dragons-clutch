#!/usr/bin/env python3
"""Finite, signed, crash-safe orchestration for repeated Activity-v3 cycles.

This is a supervisor contract, not a wallet or RPC driver.  It authenticates a
finite list of exact Activity-v3 manifests, derives disjoint work/key/session
slots, authenticates a separate exact Rent envelope for every cycle, and
cryptographically verifies a fresh V4 run authorization before it creates the
run journal or any work directory.  The existing V1--V3 single-cycle ABI is not
imported as a mutable authority and remains unchanged.
"""

from __future__ import annotations

import argparse
import base64
import dataclasses
import datetime as dt
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[2]
ACTIVITY_PATH = Path(__file__).with_name("activity.py")
LEDGER_PATH = ROOT / "tools/economic-lifecycle-ledger/ledger.py"
CANONICAL_ECONOMIC_AUTHORITY = (
    ROOT / "tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json"
)

ONGOING_MANIFEST_SCHEMA = "dclutch-devnet-activity-ongoing-manifest-v1"
RENT_ENVELOPE_SCHEMA = "dclutch-devnet-activity-cycle-rent-envelope-v1"
ONGOING_PLAN_SCHEMA = "dclutch-devnet-activity-ongoing-plan-v1"
V4_AUTHORIZATION_BODY_SCHEMA = (
    "dclutch-devnet-activity-live-authorization-body-v4"
)
V4_AUTHORIZATION_SCHEMA = "dclutch-devnet-activity-live-authorization-v4"
RUN_JOURNAL_SCHEMA = "dclutch-devnet-activity-ongoing-run-journal-v1"
CYCLE_WORK_MARKER_SCHEMA = "dclutch-devnet-activity-cycle-work-marker-v1"
DIRECT_PRODUCER_JOURNAL_SCHEMA = "dclutch-devnet-direct-trade-producer-journal-v1"
VERIFIER_RESULT_SCHEMA = "dclutch-ed25519-verification-v1"
AUTHORIZATION_PHRASE = "authorize-finite-devnet-activity-v4-live-send"
BINARY_ROLES = ("dclutch", "successor", "solana-keygen", "solana")
MAX_CYCLES = 72


class Refusal(RuntimeError):
    """A fail-closed finite-run orchestration refusal."""


def load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise Refusal(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


activity = load_module("dclutch_devnet_activity_ongoing", ACTIVITY_PATH)


@dataclasses.dataclass(frozen=True)
class AcceptedFile:
    path: Path
    sha256: str

    def document(self) -> dict[str, str]:
        return {"path": str(self.path), "sha256": self.sha256}


@dataclasses.dataclass(frozen=True)
class CyclePlan:
    ordinal: int
    cycle_id: str
    relative_work_path: str
    manifest: Any
    manifest_file: AcceptedFile
    rent_file: AcceptedFile
    rent_lamports: int
    wallet_slots: tuple[Mapping[str, Any], ...]
    session_slots: tuple[Mapping[str, str], ...]
    direct_session_producers: tuple[Mapping[str, str], ...]
    envelope: Mapping[str, str]

    def document(self) -> dict[str, Any]:
        return {
            "ordinal": self.ordinal,
            "cycleId": self.cycle_id,
            "relativeWorkPath": self.relative_work_path,
            "manifest": self.manifest_file.document(),
            "scenarioSha256": self.manifest.scenario.sha256,
            "rentEnvelope": self.rent_file.document(),
            "walletSlots": [dict(row) for row in self.wallet_slots],
            "sessionSlots": [dict(row) for row in self.session_slots],
            "directSessionProducers": [dict(row) for row in self.direct_session_producers],
            "envelope": dict(self.envelope),
        }


@dataclasses.dataclass(frozen=True)
class OngoingPlan:
    path: Path
    sha256: str
    run_id: str
    work_base: Path
    economic_authority: AcceptedFile
    accepted_harness: Mapping[str, str]
    binaries: tuple[Mapping[str, str], ...]
    cycles: tuple[CyclePlan, ...]
    aggregate_envelope: Mapping[str, str]

    def document(self) -> dict[str, Any]:
        return {
            "schema": ONGOING_PLAN_SCHEMA,
            "ongoingManifestSha256": self.sha256,
            "runId": self.run_id,
            "workBase": str(self.work_base),
            "maxCycles": len(self.cycles),
            "economicAuthority": self.economic_authority.document(),
            "acceptedHarness": dict(self.accepted_harness),
            "binaries": [dict(row) for row in self.binaries],
            "cycles": [cycle.document() for cycle in self.cycles],
            "aggregateEnvelope": dict(self.aggregate_envelope),
        }

    @property
    def plan_sha256(self) -> str:
        return sha256_bytes(canonical_json(self.document()))


@dataclasses.dataclass(frozen=True)
class VerifiedAuthorization:
    path: Path
    sha256: str
    body: Mapping[str, Any]
    signed_body_sha256: str
    signer_public_key: str
    signature: str
    verifier_path: Path
    verifier_sha256: str
    verified_at: str

    @property
    def run_envelope_id(self) -> str:
        return str(self.body["runEnvelopeId"])


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return activity.sha256_file(path)


def exact_object(value: Any, label: str) -> dict[str, Any]:
    try:
        return activity.exact_object(value, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def exact_list(value: Any, label: str) -> list[Any]:
    try:
        return activity.exact_list(value, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def exact_keys(value: Mapping[str, Any], keys: set[str], label: str) -> None:
    try:
        activity.exact_keys(value, keys, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def digest(value: Any, label: str) -> str:
    try:
        return activity.digest_text(value, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def decimal(value: Any, label: str, *, positive: bool = False) -> int:
    try:
        return activity.decimal(value, label, positive=positive)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def checked_add(left: int, right: int, label: str) -> int:
    value = left + right
    if value > 2**64 - 1:
        raise Refusal(f"{label} exceeds u64")
    return value


def accepted_file(value: Any, label: str, *, executable: bool = False) -> AcceptedFile:
    source = exact_object(value, label)
    exact_keys(source, {"path", "sha256"}, label)
    expected = digest(source["sha256"], f"{label} digest")
    try:
        path = activity.canonical_existing_file(
            source["path"], label, executable=executable
        )
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    if sha256_file(path) != expected:
        raise Refusal(f"{label} bytes differ from their accepted SHA-256")
    return AcceptedFile(path, expected)


def canonical_work_base(value: Any) -> Path:
    if not isinstance(value, str) or not value or value.strip() != value:
        raise Refusal("ongoing workBase must be canonical text")
    path = Path(value)
    if not path.is_absolute() or path == Path("/") or ".." in path.parts:
        raise Refusal("ongoing workBase must be a bounded absolute path")
    if (
        not path.exists()
        or path.is_symlink()
        or not path.is_dir()
        or path.resolve(strict=True) != path
    ):
        raise Refusal("ongoing workBase must be one existing canonical directory")
    return path.resolve(strict=True)


def stable_id(value: Any, label: str) -> str:
    try:
        return activity.stable_id(value, label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error


def parse_rent_envelope(
    reference: Any, manifest: Any, label: str
) -> tuple[AcceptedFile, int]:
    file = accepted_file(reference, label)
    try:
        value = exact_object(activity.read_exact_json(file.path, label), label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    exact_keys(
        value,
        {
            "schema",
            "manifestSha256",
            "scenarioSha256",
            "devnetGenesisHash",
            "observedSlot",
            "rentSysvarSha256",
            "entries",
            "totalRentLamports",
        },
        label,
    )
    if value["schema"] != RENT_ENVELOPE_SCHEMA:
        raise Refusal(f"{label} schema changed")
    if (
        value["manifestSha256"] != manifest.sha256
        or value["scenarioSha256"] != manifest.scenario.sha256
        or value["devnetGenesisHash"] != activity.DEVNET_GENESIS_HASH
    ):
        raise Refusal(f"{label} belongs to another cycle, scenario, or cluster")
    decimal(value["observedSlot"], f"{label} observed slot", positive=True)
    digest(value["rentSysvarSha256"], f"{label} Rent sysvar digest")
    total = 0
    account_refs: set[str] = set()
    rows = exact_list(value["entries"], f"{label} entries")
    if not rows:
        raise Refusal(f"{label} must name at least one exact Rent debit")
    for index, raw in enumerate(rows):
        row = exact_object(raw, f"{label} entry {index}")
        exact_keys(row, {"accountRef", "lamports"}, f"{label} entry {index}")
        try:
            account_ref = activity.logical_ref(
                row["accountRef"], f"{label} entry {index} account"
            )
        except activity.Refusal as error:
            raise Refusal(str(error)) from error
        if account_ref in account_refs:
            raise Refusal(f"{label} repeats Rent account {account_ref}")
        account_refs.add(account_ref)
        total = checked_add(
            total,
            decimal(row["lamports"], f"{label} {account_ref} lamports", positive=True),
            f"{label} total",
        )
    if decimal(value["totalRentLamports"], f"{label} total", positive=True) != total:
        raise Refusal(f"{label} total does not equal its exact entries")
    return file, total


def slot_id(prefix: str, cycle_id: str, *parts: str) -> str:
    preimage = "\0".join((cycle_id, *parts)).encode()
    return f"{prefix}-{sha256_bytes(preimage)[:24]}"


def cycle_envelope(authority: Mapping[str, Any], rent_lamports: int) -> dict[str, str]:
    values = {
        key: decimal(raw, f"economic authority {key}")
        for key, raw in exact_object(
            authority["authorization"], "economic authorization"
        ).items()
    }
    payer = values["initialFundingLamports"]
    transfer = values["maxPostInitTransferLamports"]
    post_fee = values["maxPostInitFeeLamports"]
    activity_fee = values["maxFeeLamports"]
    spend = values["maxSpendLamports"]
    if spend != checked_add(transfer, post_fee, "economic spend envelope"):
        raise Refusal("economic maxSpend no longer equals transfer plus post-init fee")
    maximum_debit = checked_add(
        checked_add(spend, activity_fee, "cycle payer debit"),
        rent_lamports,
        "cycle payer debit",
    )
    if maximum_debit > payer:
        raise Refusal("cycle fee plus exact Rent envelope exceeds payer funding")
    return {
        "payerFundingLamports": str(payer),
        "postInitTransferLamports": str(transfer),
        "postInitFeeCapLamports": str(post_fee),
        "activityFeeCapLamports": str(activity_fee),
        "rentEnvelopeLamports": str(rent_lamports),
        "maximumPayerDebitLamports": str(maximum_debit),
        "minimumPayerResidualLamports": str(payer - maximum_debit),
    }


def aggregate_envelopes(rows: Sequence[Mapping[str, str]]) -> dict[str, str]:
    keys = (
        "payerFundingLamports",
        "postInitTransferLamports",
        "postInitFeeCapLamports",
        "activityFeeCapLamports",
        "rentEnvelopeLamports",
        "maximumPayerDebitLamports",
        "minimumPayerResidualLamports",
    )
    totals = {key: 0 for key in keys}
    for row in rows:
        if set(row) != set(keys):
            raise Refusal("cycle envelope has another field set")
        for key in keys:
            totals[key] = checked_add(
                totals[key], decimal(row[key], f"cycle {key}"), f"aggregate {key}"
            )
    totals["feeEnvelopeLamports"] = checked_add(
        totals["postInitFeeCapLamports"],
        totals["activityFeeCapLamports"],
        "aggregate fee envelope",
    )
    return {key: str(value) for key, value in totals.items()}


def authenticate_cycle_manifest(
    file: AcceptedFile, authority: Mapping[str, Any], ledger: Any, label: str
) -> Any:
    try:
        manifest = activity.parse_manifest(file.path)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    if manifest.sha256 != file.sha256:
        raise Refusal(f"{label} parsed bytes changed")
    if (
        manifest.schema != activity.MANIFEST_SCHEMA
        or manifest.campaign is None
        or manifest.scenario.cluster_target != "devnet"
        or manifest.rpc_url != activity.DEVNET_MANIFEST_RPC_URL
        or manifest.devnet_genesis_hash != activity.DEVNET_GENESIS_HASH
    ):
        raise Refusal(f"{label} is not exact public-devnet Activity-v3")
    try:
        ledger.authenticate_activity_v3_scenario(
            activity.read_exact_json(manifest.scenario.path, f"{label} scenario"),
            authority,
        )
    except Exception as error:
        raise Refusal(f"{label} differs from the economic semantic owner: {error}") from error
    authorization = exact_object(authority["authorization"], "economic authorization")
    if manifest.campaign.initial_funding_lamports != decimal(
        authorization["initialFundingLamports"], "economic initial funding"
    ):
        raise Refusal(f"{label} payer funding differs from economic authority")
    planned_transfer = sum(
        row.transfer_lamports for row in manifest.campaign.post_init_funding
    )
    if planned_transfer != decimal(
        authorization["maxPostInitTransferLamports"], "economic post-init transfer"
    ):
        raise Refusal(f"{label} post-init funding differs from economic authority")
    if not manifest.adapters or any(
        not operation.mutation_expected for operation in manifest.scenario.operations
    ):
        raise Refusal(f"{label} retains a nonmutating lifecycle expectation")
    if not any(adapter.progressive is not None for adapter in manifest.adapters):
        raise Refusal(f"{label} has no progressive Direct or terminal session")
    return manifest


def producer_state_sha256(value: Mapping[str, Any]) -> str:
    """Match the successor journal's serde-json digest boundary exactly."""
    projected = dict(value)
    projected["stateSha256"] = ""
    return sha256_bytes(json.dumps(projected, separators=(",", ":")).encode())


def authenticate_finalized_direct_session_producer(
    reference: Any,
    manifest: Any,
    adapter: Any,
    label: str,
) -> Mapping[str, str]:
    """Accept only the successor-owned terminal producer fact for one Direct.

    The producer is the sole authority for the private-session wire.  This
    adapter merely joins its finalized fact to the immutable V3 input that the
    ordinary child will later consume; it never manufactures a replacement.
    """
    file = accepted_file(reference, label)
    try:
        value = exact_object(activity.read_exact_json(file.path, label), label)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    expected_keys = {
        "schema", "phase", "cluster", "genesisHash", "plan", "planSha256",
        "marketInput", "marketInputSha256", "campaignReport", "campaignReportSha256",
        "buyerParticipant", "buyerParticipantSha256", "checkedExecutionRelease",
        "checkedExecutionReleaseSha256", "sellerTicket", "sellerTicketSha256",
        "buyerTicket", "buyerTicketSha256", "payer", "payerKeypair", "observationSlot",
        "publicManifest", "publicManifestSha256", "publicManifestBase64", "privateSession",
        "privateSessionSha256", "privateSessionBase64", "journalDir", "evidenceFile",
        "previousStateSha256", "stateSha256",
    }
    exact_keys(value, expected_keys, label)
    if (
        value["schema"] != DIRECT_PRODUCER_JOURNAL_SCHEMA
        or value["phase"] != "finalized"
        or value["cluster"] != "devnet"
        or value["genesisHash"] != activity.DEVNET_GENESIS_HASH
    ):
        raise Refusal(f"{label} is not one Finalized reachable devnet Direct producer journal")
    if digest(value["stateSha256"], f"{label} state digest") != producer_state_sha256(value):
        raise Refusal(f"{label} state digest changed")
    previous = digest(value["previousStateSha256"], f"{label} prepared predecessor")
    session_sha = digest(value["privateSessionSha256"], f"{label} private session digest")
    spec = adapter.progressive
    assert spec is not None
    session_path = manifest.inputs[spec.session_input_id]
    source_path = manifest.inputs[spec.source_input_id]
    market_path = manifest.inputs[spec.market_input_id]
    plan_path = manifest.inputs.get("checked-release")
    if plan_path is None:
        raise Refusal(f"{label} has no checked-release Activity input")
    expected = {
        "publicManifest": str(source_path),
        "publicManifestSha256": spec.source_sha256,
        "plan": str(plan_path),
        "planSha256": sha256_file(plan_path),
        "marketInput": str(market_path),
        "marketInputSha256": spec.market_sha256,
        "privateSession": str(session_path),
        "privateSessionSha256": session_sha,
        "previousStateSha256": previous,
    }
    for key, expected_value in expected.items():
        if value.get(key) != expected_value:
            raise Refusal(f"{label} does not bind the manifest Direct {key}")
    if sha256_file(session_path) != session_sha or session_sha != spec.session_sha256:
        raise Refusal(f"{label} private session bytes differ from the manifest binding")
    return {
        "adapterId": adapter.adapter_id,
        "producerJournalPath": str(file.path),
        "producerJournalSha256": file.sha256,
        "producerStateSha256": str(value["stateSha256"]),
        "privateSessionPath": str(session_path),
        "privateSessionSha256": session_sha,
    }


def parse_ongoing_manifest(path: Path, expected_sha256: str) -> OngoingPlan:
    try:
        manifest_path = activity.canonical_existing_file(path, "ongoing manifest")
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    expected = digest(expected_sha256, "ongoing manifest digest")
    if sha256_file(manifest_path) != expected:
        raise Refusal("ongoing manifest bytes differ from their accepted SHA-256")
    try:
        value = exact_object(
            activity.read_exact_json(manifest_path, "ongoing manifest"),
            "ongoing manifest",
        )
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    exact_keys(
        value,
        {
            "schema",
            "runId",
            "workBase",
            "maxCycles",
            "economicAuthority",
            "acceptedHarness",
            "binaries",
            "cycles",
        },
        "ongoing manifest",
    )
    if value["schema"] != ONGOING_MANIFEST_SCHEMA:
        raise Refusal("ongoing manifest schema changed")
    run_id = stable_id(value["runId"], "ongoing run id")
    work_base = canonical_work_base(value["workBase"])
    max_cycles = value["maxCycles"]
    if (
        not isinstance(max_cycles, int)
        or isinstance(max_cycles, bool)
        or not 1 <= max_cycles <= MAX_CYCLES
    ):
        raise Refusal(f"ongoing maxCycles must be in 1..{MAX_CYCLES}")

    economic = accepted_file(value["economicAuthority"], "economic authority")
    if economic.path != CANONICAL_ECONOMIC_AUTHORITY.resolve(strict=True):
        raise Refusal("ongoing run names another economic semantic owner")
    ledger = load_module("dclutch_economic_lifecycle_ongoing", LEDGER_PATH)
    try:
        fixture = activity.read_exact_json(economic.path, "economic authority")
        authority = fixture["activityV3Authority"]
        authenticated_authority = ledger.authenticate_activity_v3_authority(
            authority
        )
    except Exception as error:
        raise Refusal(f"economic semantic owner refused: {error}") from error
    if authenticated_authority is None:
        raise Refusal("economic semantic owner omitted Activity-v3 authority")

    harness = exact_object(value["acceptedHarness"], "accepted harness")
    exact_keys(harness, {"path", "sha256", "sourceCommit"}, "accepted harness")
    harness_file = accepted_file(
        {"path": harness["path"], "sha256": harness["sha256"]},
        "accepted harness",
    )
    if harness_file.path != ACTIVITY_PATH.resolve(strict=True):
        raise Refusal("ongoing run names another Activity-v3 harness")
    source_commit = harness["sourceCommit"]
    if (
        not isinstance(source_commit, str)
        or activity.COMMIT_RE.fullmatch(source_commit) is None
    ):
        raise Refusal("accepted harness source commit is not one exact commit")
    harness_document = {
        "path": str(harness_file.path),
        "sha256": harness_file.sha256,
        "sourceCommit": source_commit,
    }

    binary_rows = exact_list(value["binaries"], "ongoing binaries")
    if len(binary_rows) != len(BINARY_ROLES):
        raise Refusal("ongoing manifest must name exactly four binaries")
    binaries: list[dict[str, str]] = []
    for index, (raw, expected_role) in enumerate(
        zip(binary_rows, BINARY_ROLES, strict=True)
    ):
        row = exact_object(raw, f"ongoing binary {index}")
        exact_keys(row, {"role", "path", "sha256"}, f"ongoing binary {index}")
        if row["role"] != expected_role:
            raise Refusal("ongoing binary roles are not canonical")
        file = accepted_file(
            {"path": row["path"], "sha256": row["sha256"]},
            f"ongoing {expected_role} binary",
            executable=True,
        )
        binaries.append(
            {"role": expected_role, "path": str(file.path), "sha256": file.sha256}
        )

    raw_cycles = exact_list(value["cycles"], "ongoing cycles")
    if len(raw_cycles) != max_cycles:
        raise Refusal("ongoing maxCycles differs from its exact cycle list")
    cycles: list[CyclePlan] = []
    manifest_paths: set[Path] = set()
    manifest_digests: set[str] = set()
    rent_paths: set[Path] = set()
    session_paths: set[Path] = set()
    session_digests: set[str] = set()
    wallet_slot_ids: set[str] = set()
    session_slot_ids: set[str] = set()
    producer_paths: set[Path] = set()
    producer_digests: set[str] = set()
    for ordinal, raw in enumerate(raw_cycles):
        row = exact_object(raw, f"ongoing cycle {ordinal}")
        exact_keys(
            row,
            {"manifest", "rentEnvelope", "directSessionProducers"},
            f"ongoing cycle {ordinal}",
        )
        manifest_file = accepted_file(row["manifest"], f"cycle {ordinal} manifest")
        if (
            manifest_file.path in manifest_paths
            or manifest_file.sha256 in manifest_digests
        ):
            raise Refusal("ongoing cycles reuse Activity-v3 manifest bytes or paths")
        manifest_paths.add(manifest_file.path)
        manifest_digests.add(manifest_file.sha256)
        manifest = authenticate_cycle_manifest(
            manifest_file, authority, ledger, f"cycle {ordinal} manifest"
        )
        rent_file, rent_lamports = parse_rent_envelope(
            row["rentEnvelope"], manifest, f"cycle {ordinal} Rent envelope"
        )
        if rent_file.path in rent_paths:
            raise Refusal("ongoing cycles reuse a Rent envelope path")
        rent_paths.add(rent_file.path)

        progressive = [
            adapter for adapter in manifest.adapters if adapter.progressive is not None
        ]
        session_set_sha = sha256_bytes(
            canonical_json(
                [
                    {
                        "adapterId": adapter.adapter_id,
                        "sessionSha256": adapter.progressive.session_sha256,
                    }
                    for adapter in progressive
                ]
            )
        )
        cycle_id = (
            f"cycle-{ordinal + 1:03d}-{manifest.sha256[:12]}-{session_set_sha[:12]}"
        )
        relative_work = f"cycles/{ordinal + 1:03d}-{cycle_id}"
        wallet_slots: list[dict[str, Any]] = []
        for wallet in manifest.scenario.wallets:
            key_slot = slot_id("wallet", cycle_id, wallet.wallet_id)
            if key_slot in wallet_slot_ids:
                raise Refusal("derived wallet key slot collision")
            wallet_slot_ids.add(key_slot)
            wallet_slots.append(
                {
                    "walletRef": wallet.wallet_id,
                    "roles": list(wallet.roles),
                    "keySlotId": key_slot,
                }
            )
        session_slots: list[dict[str, str]] = []
        for adapter in progressive:
            assert adapter.progressive is not None
            session_path = manifest.inputs[adapter.progressive.session_input_id]
            session_sha = adapter.progressive.session_sha256
            if session_path in session_paths or session_sha in session_digests:
                raise Refusal("ongoing cycles reuse progressive session bytes or paths")
            session_paths.add(session_path)
            session_digests.add(session_sha)
            session_slot = slot_id(
                "session", cycle_id, adapter.adapter_id, session_sha
            )
            if session_slot in session_slot_ids:
                raise Refusal("derived progressive session slot collision")
            session_slot_ids.add(session_slot)
            session_slots.append(
                {
                    "adapterId": adapter.adapter_id,
                    "sourceInputId": adapter.progressive.source_input_id,
                    "sourcePath": str(
                        manifest.inputs[adapter.progressive.source_input_id]
                    ),
                    "sourceSha256": adapter.progressive.source_sha256,
                    "inputId": adapter.progressive.session_input_id,
                    "path": str(session_path),
                    "sha256": session_sha,
                    "marketInputId": adapter.progressive.market_input_id,
                    "marketPath": str(
                        manifest.inputs[adapter.progressive.market_input_id]
                    ),
                    "marketSha256": adapter.progressive.market_sha256,
                    "sessionSlotId": session_slot,
                }
            )
        direct_adapters = [
            adapter
            for adapter in progressive
            if adapter.argv[0] == "devnet-direct-trade-v1"
        ]
        raw_producers = exact_list(
            row["directSessionProducers"],
            f"ongoing cycle {ordinal} Direct session producers",
        )
        if len(raw_producers) != len(direct_adapters):
            raise Refusal(
                "ongoing cycle must bind exactly one Finalized producer journal per Direct adapter"
            )
        direct_session_producers: list[Mapping[str, str]] = []
        seen_direct_adapters: set[str] = set()
        for producer_index, raw_producer in enumerate(raw_producers):
            producer = exact_object(
                raw_producer,
                f"ongoing cycle {ordinal} Direct producer {producer_index}",
            )
            exact_keys(
                producer,
                {"adapterId", "journal"},
                f"ongoing cycle {ordinal} Direct producer {producer_index}",
            )
            adapter_id = stable_id(
                producer["adapterId"],
                f"ongoing cycle {ordinal} Direct producer adapter",
            )
            adapter = next(
                (item for item in direct_adapters if item.adapter_id == adapter_id),
                None,
            )
            if adapter is None or adapter_id in seen_direct_adapters:
                raise Refusal("ongoing Direct producer adapter partition changed")
            binding = authenticate_finalized_direct_session_producer(
                producer["journal"],
                manifest,
                adapter,
                f"ongoing cycle {ordinal} Direct producer {adapter_id}",
            )
            journal_path = Path(binding["producerJournalPath"])
            journal_sha = binding["producerJournalSha256"]
            if journal_path in producer_paths or journal_sha in producer_digests:
                raise Refusal("ongoing cycles reuse Direct producer journal bytes or paths")
            producer_paths.add(journal_path)
            producer_digests.add(journal_sha)
            seen_direct_adapters.add(adapter_id)
            direct_session_producers.append(binding)
        cycles.append(
            CyclePlan(
                ordinal,
                cycle_id,
                relative_work,
                manifest,
                manifest_file,
                rent_file,
                rent_lamports,
                tuple(wallet_slots),
                tuple(session_slots),
                tuple(direct_session_producers),
                cycle_envelope(authority, rent_lamports),
            )
        )
    return OngoingPlan(
        manifest_path,
        expected,
        run_id,
        work_base,
        economic,
        harness_document,
        tuple(binaries),
        tuple(cycles),
        aggregate_envelopes([cycle.envelope for cycle in cycles]),
    )


def parse_timestamp(value: Any, label: str) -> dt.datetime:
    if not isinstance(value, str) or not value or value.strip() != value:
        raise Refusal(f"{label} must be one RFC3339 timestamp")
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise Refusal(f"{label} must be one RFC3339 timestamp") from error
    if parsed.tzinfo is None:
        raise Refusal(f"{label} must carry a timezone")
    return parsed


def authorization_body(
    plan: OngoingPlan,
    *,
    run_nonce: str,
    not_before: str,
    expires_at: str,
    signer_public_key: str,
    accepted_verifier_sha256: str,
) -> dict[str, Any]:
    nonce = digest(run_nonce, "V4 run nonce")
    signer = signer_public_key
    try:
        activity.base58_decode(signer, "V4 authorization signer", 32)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    verifier_sha = digest(accepted_verifier_sha256, "accepted verifier digest")
    start = parse_timestamp(not_before, "V4 notBefore")
    end = parse_timestamp(expires_at, "V4 expiresAt")
    if start >= end or end - start > dt.timedelta(hours=6):
        raise Refusal("V4 authorization window must be ordered and at most six hours")
    core = {
        "schema": V4_AUTHORIZATION_BODY_SCHEMA,
        "ongoingManifestSha256": plan.sha256,
        "ongoingPlanSha256": plan.plan_sha256,
        "runId": plan.run_id,
        "runNonce": nonce,
        "workBase": str(plan.work_base),
        "devnetGenesisHash": activity.DEVNET_GENESIS_HASH,
        "notBefore": not_before,
        "expiresAt": expires_at,
        "maxCycles": len(plan.cycles),
        "economicAuthoritySha256": plan.economic_authority.sha256,
        "aggregateEnvelope": dict(plan.aggregate_envelope),
        "acceptedHarnessSha256": plan.accepted_harness["sha256"],
        "acceptedHarnessSourceCommit": plan.accepted_harness["sourceCommit"],
        "binaries": [dict(row) for row in plan.binaries],
        "authorizationSignerPublicKeyBase58": signer,
        "acceptedVerifierSha256": verifier_sha,
        "authorization": AUTHORIZATION_PHRASE,
    }
    run_envelope_id = "run-" + sha256_bytes(canonical_json(core))[:32]
    return {**core, "runEnvelopeId": run_envelope_id}


def verify_authorization(
    path: Path,
    plan: OngoingPlan,
    verifier_path: Path,
    accepted_signer_public_key: str,
    *,
    allow_expired: bool = False,
    now: dt.datetime | None = None,
) -> VerifiedAuthorization:
    try:
        authorization_path = activity.canonical_existing_file(
            path, "V4 live authorization"
        )
        verifier = activity.canonical_existing_file(
            verifier_path, "accepted Ed25519 verifier", executable=True
        )
        value = exact_object(
            activity.read_exact_json(authorization_path, "V4 live authorization"),
            "V4 live authorization",
        )
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    exact_keys(
        value,
        {
            "schema",
            "body",
            "signedBodySha256",
            "publicKeyBase58",
            "signatureBase58",
        },
        "V4 live authorization",
    )
    if value["schema"] != V4_AUTHORIZATION_SCHEMA:
        raise Refusal("V4 live authorization schema changed")
    body = exact_object(value["body"], "V4 signed body")
    exact_keys(
        body,
        {
            "schema",
            "ongoingManifestSha256",
            "ongoingPlanSha256",
            "runId",
            "runNonce",
            "runEnvelopeId",
            "workBase",
            "devnetGenesisHash",
            "notBefore",
            "expiresAt",
            "maxCycles",
            "economicAuthoritySha256",
            "aggregateEnvelope",
            "acceptedHarnessSha256",
            "acceptedHarnessSourceCommit",
            "binaries",
            "authorizationSignerPublicKeyBase58",
            "acceptedVerifierSha256",
            "authorization",
        },
        "V4 signed body",
    )
    public_key = value["publicKeyBase58"]
    if public_key != accepted_signer_public_key or body[
        "authorizationSignerPublicKeyBase58"
    ] != accepted_signer_public_key:
        raise Refusal("V4 authorization signer differs from the accepted public key")
    try:
        activity.base58_decode(public_key, "V4 accepted signer", 32)
        activity.base58_decode(value["signatureBase58"], "V4 signature", 64)
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    expected_body = authorization_body(
        plan,
        run_nonce=body["runNonce"],
        not_before=body["notBefore"],
        expires_at=body["expiresAt"],
        signer_public_key=accepted_signer_public_key,
        accepted_verifier_sha256=body["acceptedVerifierSha256"],
    )
    if body != expected_body:
        raise Refusal("V4 signed body differs from the canonical run envelope")
    message = canonical_json(body)
    message_sha = sha256_bytes(message)
    if digest(value["signedBodySha256"], "V4 signed body digest") != message_sha:
        raise Refusal("V4 signed body digest changed")
    verifier_sha = sha256_file(verifier)
    if verifier_sha != body["acceptedVerifierSha256"]:
        raise Refusal("installed Ed25519 verifier differs from its signed accepted hash")
    current = now or dt.datetime.now(dt.timezone.utc)
    start = parse_timestamp(body["notBefore"], "V4 notBefore")
    end = parse_timestamp(body["expiresAt"], "V4 expiresAt")
    if not allow_expired and not start <= current < end:
        raise Refusal("V4 live authorization is outside its current window")

    completed = subprocess.run(
        [
            str(verifier),
            "--public-key-base58",
            public_key,
            "--signature-base58",
            value["signatureBase58"],
            "--message-base64",
            base64.b64encode(message).decode(),
            "--message-sha256",
            message_sha,
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=30,
        check=False,
    )
    if completed.returncode != 0:
        raise Refusal("accepted Ed25519 verifier rejected the V4 signed body")
    try:
        result = exact_object(
            activity.parse_exact_json_bytes(
                completed.stdout, "Ed25519 verifier result"
            ),
            "Ed25519 verifier result",
        )
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    exact_keys(
        result,
        {
            "schema",
            "messageSha256",
            "publicKeyBase58",
            "signatureBase58",
            "verified",
        },
        "Ed25519 verifier result",
    )
    expected_result = {
        "schema": VERIFIER_RESULT_SCHEMA,
        "messageSha256": message_sha,
        "publicKeyBase58": public_key,
        "signatureBase58": value["signatureBase58"],
        "verified": True,
    }
    if result != expected_result:
        raise Refusal("Ed25519 verifier returned another verification result")
    return VerifiedAuthorization(
        authorization_path,
        sha256_file(authorization_path),
        body,
        message_sha,
        public_key,
        value["signatureBase58"],
        verifier,
        verifier_sha,
        activity.utc_now(),
    )


def run_root(plan: OngoingPlan, authorization: VerifiedAuthorization) -> Path:
    return plan.work_base / authorization.run_envelope_id


def new_run_journal(
    plan: OngoingPlan, authorization: VerifiedAuthorization
) -> dict[str, Any]:
    return {
        "schema": RUN_JOURNAL_SCHEMA,
        "ongoingManifestSha256": plan.sha256,
        "ongoingPlanSha256": plan.plan_sha256,
        "runId": plan.run_id,
        "runEnvelopeId": authorization.run_envelope_id,
        "workRoot": str(run_root(plan, authorization)),
        "maxCycles": len(plan.cycles),
        "liveAuthorizationSha256": authorization.sha256,
        "authorizationVerification": {
            "signedBodySha256": authorization.signed_body_sha256,
            "signerPublicKeyBase58": authorization.signer_public_key,
            "signatureBase58": authorization.signature,
            "acceptedVerifierPath": str(authorization.verifier_path),
            "acceptedVerifierSha256": authorization.verifier_sha256,
            "verifiedAt": authorization.verified_at,
            "verified": True,
        },
        "aggregateEnvelope": dict(plan.aggregate_envelope),
        "phase": "pending",
        "activeCycleOrdinal": None,
        "cycles": [
            {
                "ordinal": cycle.ordinal,
                "cycleId": cycle.cycle_id,
                "manifestSha256": cycle.manifest.sha256,
                "relativeWorkPath": cycle.relative_work_path,
                "walletKeySlotIds": [
                    row["keySlotId"] for row in cycle.wallet_slots
                ],
                "sessionSlotIds": [
                    row["sessionSlotId"] for row in cycle.session_slots
                ],
                "directSessionProducers": [
                    dict(row) for row in cycle.direct_session_producers
                ],
                "phase": "pending",
                "startedAt": None,
                "completedAt": None,
                "supervisorStatus": None,
                "walletLedger": None,
                "reconciliation": None,
            }
            for cycle in plan.cycles
        ],
    }


def bind_run_journal(
    value: Mapping[str, Any], plan: OngoingPlan, authorization: VerifiedAuthorization
) -> None:
    expected = new_run_journal(plan, authorization)
    for key in (
        "schema",
        "ongoingManifestSha256",
        "ongoingPlanSha256",
        "runId",
        "runEnvelopeId",
        "workRoot",
        "maxCycles",
        "liveAuthorizationSha256",
        "authorizationVerification",
        "aggregateEnvelope",
    ):
        if value.get(key) != expected[key]:
            raise Refusal(f"ongoing run journal changed {key}")
    rows = exact_list(value.get("cycles"), "ongoing run journal cycles")
    if len(rows) != len(plan.cycles):
        raise Refusal("ongoing run journal changed its cycle count")
    active = 0
    saw_pending = False
    for cycle, raw in zip(plan.cycles, rows, strict=True):
        row = exact_object(raw, f"run journal cycle {cycle.ordinal}")
        if (
            row.get("ordinal") != cycle.ordinal
            or row.get("cycleId") != cycle.cycle_id
            or row.get("manifestSha256") != cycle.manifest.sha256
            or row.get("relativeWorkPath") != cycle.relative_work_path
            or row.get("walletKeySlotIds")
            != [slot["keySlotId"] for slot in cycle.wallet_slots]
            or row.get("sessionSlotIds")
            != [slot["sessionSlotId"] for slot in cycle.session_slots]
            or row.get("directSessionProducers")
            != [dict(slot) for slot in cycle.direct_session_producers]
        ):
            raise Refusal(f"run journal cycle {cycle.ordinal} changed identity or slots")
        phase = row.get("phase")
        if phase not in {"pending", "active", "complete"}:
            raise Refusal(f"run journal cycle {cycle.ordinal} has another phase")
        if phase == "active":
            active += 1
        if phase == "pending":
            saw_pending = True
        elif saw_pending:
            raise Refusal("run journal cycles are not a canonical completed/active/pending prefix")
    if active > 1:
        raise Refusal("run journal has more than one active lifecycle")
    active_ordinal = value.get("activeCycleOrdinal")
    observed_active = next(
        (row["ordinal"] for row in rows if row["phase"] == "active"), None
    )
    if active_ordinal != observed_active:
        raise Refusal("run journal active cycle pointer changed")


def prepare_run(
    plan: OngoingPlan, authorization: VerifiedAuthorization
) -> Path:
    root = run_root(plan, authorization)
    journal_path = root / "run-journal.json"
    if journal_path.exists():
        try:
            value = activity.authenticated_state(journal_path, "ongoing run journal")
        except activity.Refusal as error:
            raise Refusal(str(error)) from error
        bind_run_journal(value, plan, authorization)
        return journal_path
    if root.exists():
        if root.is_symlink() or not root.is_dir() or any(root.iterdir()):
            raise Refusal("deterministic V4 work root exists without its exact journal")
    else:
        root.mkdir(mode=0o700, parents=False)
    activity.atomic_write_json(
        journal_path, new_run_journal(plan, authorization), mode=0o600
    )
    return journal_path


def next_cycle_action(value: Mapping[str, Any]) -> tuple[str, int | None]:
    rows = exact_list(value.get("cycles"), "ongoing run journal cycles")
    active = [row for row in rows if row.get("phase") == "active"]
    if len(active) > 1:
        raise Refusal("more than one Activity-v3 lifecycle is active")
    if active:
        return "resume", active[0]["ordinal"]
    for row in rows:
        if row.get("phase") == "pending":
            return "start", row["ordinal"]
        if row.get("phase") != "complete":
            raise Refusal("ongoing cycle journal has another phase")
    return "complete", None


def cycle_work_path(
    plan: OngoingPlan, authorization: VerifiedAuthorization, ordinal: int
) -> Path:
    if not 0 <= ordinal < len(plan.cycles):
        raise Refusal("cycle ordinal is outside the finite run")
    return run_root(plan, authorization) / plan.cycles[ordinal].relative_work_path


def materialize_cycle_work(
    journal_path: Path,
    plan: OngoingPlan,
    authorization: VerifiedAuthorization,
    ordinal: int,
) -> Path:
    try:
        journal = activity.authenticated_state(journal_path, "ongoing run journal")
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    bind_run_journal(journal, plan, authorization)
    row = exact_object(journal["cycles"][ordinal], f"run journal cycle {ordinal}")
    if row["phase"] != "active":
        raise Refusal("cycle work may exist only after its write-ahead active marker")
    work = cycle_work_path(plan, authorization, ordinal)
    marker_path = work / "cycle-work.json"
    expected_marker = {
        "schema": CYCLE_WORK_MARKER_SCHEMA,
        "ongoingPlanSha256": plan.plan_sha256,
        "runEnvelopeId": authorization.run_envelope_id,
        "cycleId": plan.cycles[ordinal].cycle_id,
        "manifestSha256": plan.cycles[ordinal].manifest.sha256,
        "relativeWorkPath": plan.cycles[ordinal].relative_work_path,
    }
    cycle_parent = run_root(plan, authorization) / "cycles"
    if cycle_parent.exists():
        if (
            cycle_parent.is_symlink()
            or not cycle_parent.is_dir()
            or cycle_parent.resolve(strict=True) != cycle_parent
        ):
            raise Refusal("cycle work parent exists with another identity")
    else:
        cycle_parent.mkdir(mode=0o700, parents=False)
    if work.exists():
        if work.is_symlink() or not work.is_dir():
            raise Refusal("cycle work path exists with another kind")
        if marker_path.exists():
            try:
                marker = activity.authenticated_state(
                    marker_path, "cycle work marker"
                )
            except activity.Refusal as error:
                raise Refusal(str(error)) from error
            for key, expected in expected_marker.items():
                if marker.get(key) != expected:
                    raise Refusal("cycle work marker belongs to another run")
            return work
        if any(work.iterdir()):
            raise Refusal("cycle work path was reused before its exact marker")
    else:
        work.mkdir(mode=0o700, parents=False)
    activity.atomic_write_json(marker_path, expected_marker, mode=0o600)
    return work


def begin_or_resume_cycle(
    journal_path: Path, plan: OngoingPlan, authorization: VerifiedAuthorization
) -> tuple[str, CyclePlan, Path]:
    try:
        journal = activity.authenticated_state(journal_path, "ongoing run journal")
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    bind_run_journal(journal, plan, authorization)
    action, ordinal = next_cycle_action(journal)
    if action == "complete" or ordinal is None:
        raise Refusal("finite Activity-v3 run is already complete")
    if action == "start":
        work = cycle_work_path(plan, authorization, ordinal)
        if work.exists():
            raise Refusal("fresh cycle work path was already used")
        updated = dict(journal)
        rows = [dict(row) for row in journal["cycles"]]
        rows[ordinal]["phase"] = "active"
        rows[ordinal]["startedAt"] = activity.utc_now()
        updated["cycles"] = rows
        updated["phase"] = "active"
        updated["activeCycleOrdinal"] = ordinal
        activity.atomic_write_json(journal_path, updated, mode=0o600)
    work = materialize_cycle_work(
        journal_path, plan, authorization, ordinal
    )
    return action, plan.cycles[ordinal], work


def completed_wallet_addresses(journal: Mapping[str, Any]) -> set[str]:
    addresses: set[str] = set()
    for index, raw in enumerate(exact_list(journal["cycles"], "run cycles")):
        row = exact_object(raw, f"run cycle {index}")
        if row.get("phase") != "complete":
            continue
        reference = row.get("walletLedger")
        if reference is None:
            raise Refusal(f"completed cycle {index} omitted its admitted wallet ledger")
        ledger_file = accepted_file(reference, f"completed cycle {index} wallet ledger")
        try:
            ledger = exact_object(
                activity.read_exact_json(ledger_file.path, "completed wallet ledger"),
                "completed wallet ledger",
            )
        except activity.Refusal as error:
            raise Refusal(str(error)) from error
        for wallet_index, wallet_raw in enumerate(
            exact_list(ledger.get("wallets"), "completed wallet ledger wallets")
        ):
            wallet = exact_object(wallet_raw, f"completed wallet {wallet_index}")
            try:
                address = activity.pubkey_text(
                    wallet.get("address"), f"completed wallet {wallet_index} address"
                )
            except activity.Refusal as error:
                raise Refusal(str(error)) from error
            if address in addresses:
                raise Refusal("completed cycles reused a disposable wallet public key")
            addresses.add(address)
    return addresses


def admit_active_wallet_ledger(
    journal_path: Path,
    plan: OngoingPlan,
    authorization: VerifiedAuthorization,
    wallet_ledger_path: Path,
) -> AcceptedFile:
    """Admit disposable public keys before funding or lifecycle mutation.

    Key creation remains outside this key-free supervisor, but the accepted
    launcher must call this immediately after `prepare-wallets` and before any
    funding or activity command.  Reused addresses are therefore rejected at
    the pre-mutation boundary rather than discovered only at reconciliation.
    """

    try:
        journal = activity.authenticated_state(journal_path, "ongoing run journal")
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    bind_run_journal(journal, plan, authorization)
    action, ordinal = next_cycle_action(journal)
    if action != "resume" or ordinal is None:
        raise Refusal("wallet admission requires one write-ahead active cycle")
    cycle = plan.cycles[ordinal]
    try:
        ledger_path = activity.canonical_existing_file(
            wallet_ledger_path, "active cycle wallet ledger"
        )
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    wallet_file = AcceptedFile(ledger_path, sha256_file(ledger_path))
    row = exact_object(journal["cycles"][ordinal], f"run journal cycle {ordinal}")
    if row.get("walletLedger") is not None:
        if row["walletLedger"] != wallet_file.document():
            raise Refusal("active cycle wallet ledger changed after admission")
        return wallet_file
    try:
        wallet = exact_object(
            activity.read_exact_json(wallet_file.path, "active cycle wallet ledger"),
            "active cycle wallet ledger",
        )
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    if (
        wallet.get("schema") != activity.WALLET_LEDGER_SCHEMA
        or wallet.get("manifestSha256") != cycle.manifest.sha256
        or wallet.get("scenarioSha256") != cycle.manifest.scenario.sha256
    ):
        raise Refusal("active cycle wallet ledger belongs to another lifecycle")
    prior_addresses = completed_wallet_addresses(journal)
    current_addresses: set[str] = set()
    wallet_rows = exact_list(wallet.get("wallets"), "active cycle wallet ledger wallets")
    if len(wallet_rows) != len(cycle.wallet_slots):
        raise Refusal("active cycle wallet ledger changed the disposable role count")
    for slot, raw in zip(cycle.wallet_slots, wallet_rows, strict=True):
        item = exact_object(raw, "active cycle wallet ledger row")
        if item.get("id") != slot["walletRef"]:
            raise Refusal("active cycle wallet ledger changed wallet slot ordering")
        try:
            address = activity.pubkey_text(item.get("address"), "active cycle wallet address")
        except activity.Refusal as error:
            raise Refusal(str(error)) from error
        if address in prior_addresses or address in current_addresses:
            raise Refusal("finite run reused a disposable wallet public key")
        current_addresses.add(address)
    updated = dict(journal)
    rows = [dict(item) for item in journal["cycles"]]
    rows[ordinal]["walletLedger"] = wallet_file.document()
    updated["cycles"] = rows
    activity.atomic_write_json(journal_path, updated, mode=0o600)
    return wallet_file


def complete_active_cycle(
    journal_path: Path,
    plan: OngoingPlan,
    authorization: VerifiedAuthorization,
    *,
    supervisor_status_path: Path,
    wallet_ledger_path: Path,
    reconciliation_path: Path,
) -> None:
    try:
        journal = activity.authenticated_state(journal_path, "ongoing run journal")
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    bind_run_journal(journal, plan, authorization)
    action, ordinal = next_cycle_action(journal)
    if action != "resume" or ordinal is None:
        raise Refusal("only the active cycle may complete")
    cycle = plan.cycles[ordinal]
    active_row = exact_object(
        journal["cycles"][ordinal], f"run journal cycle {ordinal}"
    )
    status_file = accepted_file(
        {"path": str(supervisor_status_path), "sha256": sha256_file(supervisor_status_path)},
        "cycle supervisor status",
    )
    wallet_file = accepted_file(
        {"path": str(wallet_ledger_path), "sha256": sha256_file(wallet_ledger_path)},
        "cycle wallet ledger",
    )
    reconciliation_file = accepted_file(
        {"path": str(reconciliation_path), "sha256": sha256_file(reconciliation_path)},
        "cycle reconciliation",
    )
    try:
        status = activity.authenticated_state(status_file.path, "cycle supervisor status")
        reconciliation = activity.authenticated_state(
            reconciliation_file.path, "cycle reconciliation"
        )
        wallet = exact_object(
            activity.read_exact_json(wallet_file.path, "cycle wallet ledger"),
            "cycle wallet ledger",
        )
    except activity.Refusal as error:
        raise Refusal(str(error)) from error
    if active_row.get("walletLedger") != wallet_file.document():
        raise Refusal("cycle completion requires its pre-mutation admitted wallet ledger")
    if (
        status.get("schema") != activity.V3_SUPERVISOR_STATUS_SCHEMA
        or status.get("manifestSha256") != cycle.manifest.sha256
        or status.get("scenarioSha256") != cycle.manifest.scenario.sha256
        or status.get("cycleId") != cycle.cycle_id
        or status.get("status")
        not in {
            "complete-reconciled-live-send",
            "complete-reconciled-poll-only",
        }
        or status.get("reconciliationSha256") != reconciliation_file.sha256
    ):
        raise Refusal("cycle supervisor status is not exact reconciled completion")
    if (
        reconciliation.get("schema") != activity.RECONCILIATION_SCHEMA
        or reconciliation.get("manifestSha256") != cycle.manifest.sha256
        or reconciliation.get("scenarioSha256") != cycle.manifest.scenario.sha256
        or reconciliation.get("untrustedProjectionUsed") is not False
    ):
        raise Refusal("cycle reconciliation belongs to another accepted lifecycle")
    if (
        wallet.get("schema") != activity.WALLET_LEDGER_SCHEMA
        or wallet.get("manifestSha256") != cycle.manifest.sha256
        or wallet.get("scenarioSha256") != cycle.manifest.scenario.sha256
    ):
        raise Refusal("cycle wallet ledger belongs to another lifecycle")
    updated = dict(journal)
    rows = [dict(row) for row in journal["cycles"]]
    rows[ordinal].update(
        {
            "phase": "complete",
            "completedAt": activity.utc_now(),
            "supervisorStatus": status_file.document(),
            "walletLedger": active_row["walletLedger"],
            "reconciliation": reconciliation_file.document(),
        }
    )
    updated["cycles"] = rows
    updated["activeCycleOrdinal"] = None
    updated["phase"] = (
        "complete" if ordinal + 1 == len(plan.cycles) else "pending"
    )
    activity.atomic_write_json(journal_path, updated, mode=0o600)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--manifest", required=True)
    result.add_argument("--manifest-sha256", required=True)
    subparsers = result.add_subparsers(dest="command", required=True)
    plan_parser = subparsers.add_parser("plan")
    plan_parser.add_argument("--output")
    auth_parser = subparsers.add_parser("verify-and-prepare")
    auth_parser.add_argument("--live-authorization", required=True)
    auth_parser.add_argument("--verifier", required=True)
    auth_parser.add_argument("--accepted-signer-public-key", required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    try:
        arguments = parser().parse_args(argv)
        plan = parse_ongoing_manifest(
            Path(arguments.manifest), arguments.manifest_sha256
        )
        if arguments.command == "plan":
            if arguments.output is None:
                print(canonical_json(plan.document()).decode())
            else:
                output = Path(arguments.output)
                if not output.is_absolute() or output.exists() or output.is_symlink():
                    raise Refusal("ongoing plan output must be one absent absolute path")
                activity.atomic_write_json(output, plan.document(), mode=0o644)
        elif arguments.command == "verify-and-prepare":
            verified = verify_authorization(
                Path(arguments.live_authorization),
                plan,
                Path(arguments.verifier),
                arguments.accepted_signer_public_key,
            )
            print(str(prepare_run(plan, verified)))
        else:
            raise Refusal("unknown ongoing command")
        return 0
    except Refusal as error:
        print(f"ongoing activity refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
