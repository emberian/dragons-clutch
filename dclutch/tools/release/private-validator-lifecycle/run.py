#!/usr/bin/env python3
"""Localhost-only dClutch mutable lifecycle supervisor.

The supervisor never accepts an RPC URL.  It creates one fresh loopback
validator per named seed, provisions only seed-derived disposable local keys,
invokes the accepted exterior commands, preserves every stage artifact, and
always tears down the complete validator process group.

The one-seed founding/participant probe is accepted today.  The twenty-seed
terminal lifecycle remains fail-closed until its Direct producer, payout
executor, and aggregate receipt authenticator are all present in the exact
source revision.  Keeping the future full path visible here must never make it
dispatchable before those semantic owners converge.
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import dataclasses
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import shutil
import socket
import stat
import subprocess
import sys
import time
from typing import Any, Iterable, Sequence
from urllib import parse as urllib_parse, request

SCHEMA = "dclutch-private-validator-lifecycle-summary-v1"
PARTICIPANT_PROBE_SCHEMA = "dclutch-private-validator-participant-probe-summary-v1"
FULL_PROBE_SCHEMA = "dclutch-private-validator-full-lifecycle-probe-summary-v1"
RUN_SCHEMA = "dclutch-private-validator-lifecycle-run-v1"
PARTICIPANT_PROBE_RUN_SCHEMA = "dclutch-private-validator-participant-probe-run-v1"
FULL_PROBE_RUN_SCHEMA = "dclutch-private-validator-full-lifecycle-probe-run-v1"
PARTICIPANT_HANDOFF_SCHEMA = "dclutch-private-validator-participant-handoff-v1"
OFFLINE_PREFLIGHT_SCHEMA = "dclutch-private-lifecycle-offline-preflight-v1"
OFFLINE_PREFLIGHT_EVIDENCE_LEVEL = (
    "offline-clean-committed-source-contract-only"
)
OFFLINE_PREFLIGHT_RELATIVE_PATH = Path(
    "tools/release/private-validator-lifecycle/preflight.py"
)
RUNNER_RELATIVE_PATH = Path("tools/release/private-validator-lifecycle/run.py")
OFFLINE_PREFLIGHT_RECEIPT = "OFFLINE_PREFLIGHT.json"
MIXED_GATE_SCHEMA = "dclutch-checked-upgrade-gate-v2"
MIXED_GATE_AUTHENTICATOR_RELATIVE_PATH = Path(
    "tools/release/compose-mixed-gate.py"
)
SEED_DOMAIN = b"dclutch/private-validator-lifecycle/named-seed/v1\0"
FOUNDING_PARTICIPANT_COMMANDS = (
    "local-mutable-prepare-v1",
    "local-mutable-plan-authenticate-v1",
    "local-private-validator-market-v1",
    "campaign",
    "local-private-validator-user-position-admission-v1",
)
PYTH_PROVISION_COMMAND = "local-private-validator-pyth-vaa-provision-v1"
FLAGSHIP_RESOLUTION_COMMAND = "local-private-validator-flagship-resolution-v1"
PAYOUT_INPUT_COMMAND = "local-private-validator-wallet-terminal-payout-input-v1"
PAYOUT_EXECUTE_COMMAND = "local-private-validator-wallet-terminal-payout-v1"
TERMINAL_SEQUENCE_COMMAND = "local-private-validator-terminal-sequence-v1"
TERMINAL_RETIREMENT_COMMAND = "local-private-validator-aggregate-retirement-v1"
DIRECT_PRODUCER_COMMAND = "local-private-validator-direct-trade-produce-v1"
DIRECT_EXECUTE_COMMAND = "local-private-validator-direct-trade-v1"
DIRECT_PAYOUT_SCHEDULE_COMMAND = "local-private-validator-direct-payout-schedule-v1"
PROVIDER_CLOSURE_COMMAND = "local-private-validator-pyth-provider-closure-v1"
ACTIVITY_STAGE_COMMAND = "local-private-validator-activity-stage-completion-v1"
ACTIVITY_MANIFEST_COMMAND = "local-private-validator-activity-manifest-v1"
ACTIVITY_CAPTURE_COMMAND = "local-private-validator-finalized-activity-capture-v1"
LIFECYCLE_SESSION_COMMAND = "local-private-validator-lifecycle-session-v1"
LIFECYCLE_RECEIPT_COMMAND = "local-private-validator-lifecycle-receipt-v1"
FINAL_EVIDENCE_COMMANDS = (
    PROVIDER_CLOSURE_COMMAND,
    ACTIVITY_STAGE_COMMAND,
    ACTIVITY_MANIFEST_COMMAND,
    ACTIVITY_CAPTURE_COMMAND,
    LIFECYCLE_SESSION_COMMAND,
    LIFECYCLE_RECEIPT_COMMAND,
)
FULL_LIFECYCLE_BLOCKERS = ("exact seventeen-case resumable chaos session",)
DIRECT_PRODUCER_SCHEMA = "dclutch-owned-loopback-direct-trade-producer-receipt-v1"
DIRECT_FINALIZED_SCHEMA = "dclutch-owned-loopback-direct-trade-finalized-v1"
DIRECT_PAYOUT_SCHEDULE_SCHEMA = "dclutch-owned-loopback-direct-payout-schedule-v1"
PYTH_JOURNAL_SCHEMA = "dclutch-owned-loopback-pyth-prerequisite-transaction-v1"
RESOLUTION_PRODUCER_SCHEMA = "dclutch-owned-loopback-flagship-resolution-producer-v1"
RESOLUTION_TABLE_SCHEMA = "dclutch-owned-loopback-flagship-resolution-alt-journal-v3"
RESOLUTION_INPUT_SCHEMA = "dclutch-owned-loopback-flagship-resolution-input-v1"
RESOLUTION_CHECKPOINT_SCHEMA = (
    "dclutch-owned-loopback-flagship-resolution-checkpoint-v3"
)
PAYOUT_INPUT_SCHEMA = "dclutch-wallet-terminal-payout-plan-input-v1"
PAYOUT_EVIDENCE_SCHEMA = (
    "dclutch-local-private-validator-wallet-terminal-payout-evidence-v1"
)
TERMINAL_SESSION_SCHEMA = "dclutch-owned-loopback-terminal-sequence-session-v1"
TERMINAL_JOURNAL_SCHEMA = "dclutch-owned-loopback-terminal-sequence-journal-v1"
TERMINAL_COMPLETION_SCHEMA = (
    "dclutch-owned-loopback-aggregate-retirement-completion-v1"
)
TERMINAL_CAMPAIGN_SCHEMA = "dclutch-owned-loopback-aggregate-retirement-campaign-v1"
TERMINAL_AGGREGATE_JOURNAL_SCHEMA = (
    "dclutch-owned-loopback-aggregate-retirement-journal-v1"
)
TERMINAL_PROGRESS_SCHEMA = "dclutch-owned-loopback-aggregate-retirement-progress-v1"
TERMINAL_AGGREGATE_OPERATIONS = (
    "prepare",
    "close-vault",
    "close-replay",
    "finish",
)
MAX_RESOLUTION_TABLE_INVOCATIONS = 64
MAX_RESOLUTION_STAGE_INVOCATIONS = 16
MAX_PAYOUT_INVOCATIONS = 24
MAX_TERMINAL_INVOCATIONS = 32
MAX_DIRECT_INVOCATIONS = 32
MAX_PAYOUT_TARGETS = 32
PYTH_JOURNAL_FILES = (
    "00-router-initialize.json",
    "01-receiver-initialize.json",
    "02-treasury-capitalize.json",
    "03-encoded-vaa-create.json",
    "04-encoded-vaa-initialize.json",
    "05-encoded-vaa-write-0000.json",
    "06-encoded-vaa-write-0600.json",
    "07-encoded-vaa-verify.json",
)
PYTH_ACTIONS = (
    "router-initialize",
    "receiver-initialize",
    "treasury-capitalize",
    "encoded-vaa-create",
    "encoded-vaa-initialize",
    "encoded-vaa-write-0000",
    "encoded-vaa-write-0600",
    "encoded-vaa-verify",
)
ROLE_ORDER = ("registry", "rent", "custody", "resolution", "claims", "trading", "core")
DEVELOPMENT_FEE_BASIS_POINTS = 50
FEE_BASIS_POINTS_DENOMINATOR = 10_000
PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS = 100_000_000
VALIDATOR_MINT_ROLE = "core-upgrade-authority"
CAMPAIGN_PAYER_ROLE = "campaign-payer"
LOCAL_TEST_BANKROLL_LAMPORTS = 100_000_000_000
LOCAL_TEST_BANKROLL_SCHEMA = "dclutch-private-validator-local-test-bankroll-v1"
SYSTEM_PROGRAM_ADDRESS = "11111111111111111111111111111111"
DEVELOPMENT_FEE_RECIPIENT_ROLE = "founding-source-funder"
PARTICIPANT_ROLE = "participant"
PARTICIPANT_FIXTURE_SOURCE_ROLE = "direct-buyer"
CAMPAIGN_ADMINISTRATION_KEY_ROLES = (VALIDATOR_MINT_ROLE,)
CAMPAIGN_FOUNDING_KEY_ROLES = (
    CAMPAIGN_PAYER_ROLE,
    "collateral-mint",
    "collateral-wallet",
    "founding-beneficiary",
    "founding-projection-witness",
    "founding-source-funder",
    PARTICIPANT_ROLE,
    PARTICIPANT_FIXTURE_SOURCE_ROLE,
)
FOUNDING_SUCCESS_MUTATIONS = (
    "prepare exact controller funding ledgers and checkpoint (DCLTCFQ1)",
    "stage projected custody against prepared controller funding (DCLTPCB2)",
    "found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)",
    "core-funding-create-v1",
    "resolution-funding-activate-v1",
    "core-funding-accept-v1",
)
FOUNDING_COMPUTE_LABELS = (
    "founding-dcltcfq1",
    "founding-dcltpcb2",
    "founding-dcltgmf3",
    "founding-core-funding-create",
    "founding-resolution-funding-activate",
    "founding-core-funding-accept",
)
FOUNDING_JOURNAL_SCHEMA = "dclutch-public-founding-submission-journal-v2"
FOUNDING_JOURNAL_OPERATIONS = (
    "dcltcfq1",
    "dcltpcb2",
    "dcltgmf3",
    "core-funding-create-v1",
    "resolution-funding-activate-v1",
    "core-funding-accept-v1",
)
LOCAL_AIRDROP_ROLES: tuple[str, ...] = ()
PROTOCOL_CREATED_KEY_ROLES = (
    "collateral-mint",
    "collateral-wallet",
    "founding-beneficiary",
    "founding-projection-witness",
    "founding-source-funder",
)
CANONICAL_RESOLUTION_GATE_LABEL = "resolution"
CANONICAL_RESOLUTION_PACKAGE = "dclutch-resolution-proof-sbf"
CANONICAL_RESOLUTION_ELF_PATH = "elf/resolution.so"
BANISHED_RESOLUTION_ELF_BASENAME = "dclutch_sbf.so"
BANISHED_RESOLUTION_ELF_BYTES = 9_034_536


class Refusal(RuntimeError):
    """A fail-closed release refusal with a stable operator-facing reason."""


@dataclasses.dataclass(frozen=True)
class Paths:
    repo: Path
    release_root: Path
    expected_release_gate_sha256: str | None
    expected_release_source_revision: str | None
    expected_release_source_tree_sha256: str | None
    bootstrap: Path
    reuse_bootstrap_work: Path | None
    validator: Path
    solana: Path
    work: Path


@dataclasses.dataclass(frozen=True, order=True)
class PayoutTarget:
    """Direct-owned routing and quantity for one frozen live nonzero claim."""

    owner: str
    claim_index: int
    recipient: str
    quantity_atoms: int


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
BASE58_VALUES = {character: index for index, character in enumerate(BASE58_ALPHABET)}


def base58_bytes(value: Any, width: int, label: str) -> bytes:
    if not isinstance(value, str) or not value:
        raise Refusal(f"{label} must be nonempty base58 text")
    number = 0
    try:
        for character in value:
            number = number * 58 + BASE58_VALUES[character]
    except KeyError as error:
        raise Refusal(f"{label} contains a non-base58 character") from error
    body = (
        b"" if number == 0 else number.to_bytes((number.bit_length() + 7) // 8, "big")
    )
    decoded = b"\0" * (len(value) - len(value.lstrip("1"))) + body
    if len(decoded) != width or decoded == b"\0" * width:
        raise Refusal(f"{label} must decode to one nonzero {width}-byte value")
    return decoded


def canonical_pubkey(value: Any, label: str) -> str:
    base58_bytes(value, 32, label)
    return value


def canonical_signature(value: Any, label: str) -> str:
    base58_bytes(value, 64, label)
    return value


def positive_integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise Refusal(f"{label} must be one positive integer")
    return value


def nonnegative_integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise Refusal(f"{label} must be one nonnegative integer")
    return value


def canonical_decimal(value: Any, label: str, *, positive: bool = True) -> int:
    if (
        not isinstance(value, str)
        or not value
        or (len(value) > 1 and value.startswith("0"))
        or not value.isascii()
        or not value.isdecimal()
    ):
        raise Refusal(f"{label} must be canonical unsigned decimal text")
    number = int(value)
    if positive and number == 0:
        raise Refusal(f"{label} must be positive")
    return number


def finalized_fact(
    row: Any,
    label: str,
    *,
    signature_key: str = "signature",
    slot_key: str = "slot",
    fee_key: str = "feeLamports",
    compute_key: str = "computeUnitsConsumed",
    decimal_text: bool = False,
) -> dict[str, Any]:
    if not isinstance(row, dict):
        raise Refusal(f"{label} must be one finalized evidence object")
    slot = (
        canonical_decimal(row.get(slot_key), f"{label} slot")
        if decimal_text
        else positive_integer(row.get(slot_key), f"{label} slot")
    )
    fee = (
        canonical_decimal(row.get(fee_key), f"{label} fee", positive=False)
        if decimal_text
        else nonnegative_integer(row.get(fee_key), f"{label} fee")
    )
    compute = (
        canonical_decimal(row.get(compute_key), f"{label} compute units")
        if decimal_text
        else positive_integer(row.get(compute_key), f"{label} compute units")
    )
    return {
        "signature": canonical_signature(row.get(signature_key), f"{label} signature"),
        "slot": slot,
        "fee_lamports": fee,
        "compute_units_consumed": compute,
    }


def canonical_payout_schedule(rows: Sequence[PayoutTarget]) -> tuple[PayoutTarget, ...]:
    targets = tuple(rows)
    if not 1 <= len(targets) <= MAX_PAYOUT_TARGETS:
        raise Refusal(
            "Direct payout schedule must contain one through 32 nonzero claims"
        )
    seen: set[tuple[str, int]] = set()
    for target in targets:
        if not isinstance(target, PayoutTarget):
            raise Refusal("Direct payout schedule contains a caller-crafted row shape")
        canonical_pubkey(target.owner, "payout owner")
        canonical_pubkey(target.recipient, "payout recipient")
        if (
            isinstance(target.claim_index, bool)
            or not isinstance(target.claim_index, int)
            or not 0 <= target.claim_index <= 0xFFFFFFFF
        ):
            raise Refusal("payout claim index must be one canonical u32")
        if (
            isinstance(target.quantity_atoms, bool)
            or not isinstance(target.quantity_atoms, int)
            or not 1 <= target.quantity_atoms <= 0xFFFFFFFFFFFFFFFF
        ):
            raise Refusal("payout quantity must be one positive canonical u64")
        identity = (target.owner, target.claim_index)
        if identity in seen:
            raise Refusal("Direct payout schedule repeats an owner/claim pair")
        seen.add(identity)
    canonical = tuple(
        sorted(
            targets,
            key=lambda row: (
                base58_bytes(row.owner, 32, "owner"),
                row.claim_index,
                base58_bytes(row.recipient, 32, "recipient"),
            ),
        )
    )
    if targets != canonical:
        raise Refusal(
            "Direct payout schedule is not canonical owner/claim/recipient order"
        )
    return targets


def canonical_file(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise Refusal(f"{label} must be one existing absolute non-symlink file: {path}")
    return path.resolve(strict=True)


def canonical_directory(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_dir():
        raise Refusal(
            f"{label} must be one existing absolute non-symlink directory: {path}"
        )
    return path.resolve(strict=True)


def decode_unique_json(text: str, label: str) -> Any:
    def pairs(rows: list[tuple[str, Any]]) -> dict[str, Any]:
        output: dict[str, Any] = {}
        for key, value in rows:
            if key in output:
                raise Refusal(f"{label} duplicated JSON key {key!r}")
            output[key] = value
        return output

    try:
        return json.loads(text, object_pairs_hook=pairs)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise Refusal(f"{label} is not exact JSON: {error}") from error


def read_unique_json(path: Path, label: str) -> Any:
    try:
        return decode_unique_json(path.read_text(), label)
    except OSError as error:
        raise Refusal(f"{label} is not exact JSON: {error}") from error


def clean_commit(repo: Path) -> str:
    result = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=repo,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.stdout:
        raise Refusal(
            "private-validator lifecycle requires one clean source commit; worktree is dirty"
        )
    return subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=repo, text=True
    ).strip()


def clean_tree(repo: Path) -> str:
    try:
        tree = subprocess.check_output(
            ["git", "rev-parse", "--verify", "HEAD^{tree}"],
            cwd=repo,
            text=True,
            stderr=subprocess.PIPE,
        ).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise Refusal(f"cannot bind the private lifecycle source tree: {error}") from error
    if len(tree) != 40 or any(character not in "0123456789abcdef" for character in tree):
        raise Refusal("private lifecycle source tree is not one lowercase Git object id")
    return tree


def authenticate_offline_preflight(
    document: Any,
    *,
    paths: Paths,
    commit: str,
    tree: str,
    through: str,
) -> dict[str, Any]:
    expected_fields = {
        "schema",
        "status",
        "evidence_level",
        "through",
        "validator_started",
        "rpc_used",
        "keys_read",
        "build_run",
        "repository",
        "command_exposures",
        "recovery_exposure",
        "schema_handoffs",
        "stage_vocabulary",
        "constants",
        "economic_owner",
        "founding_geometry",
        "transaction_geometry",
        "expected_execution",
        "source_sha256",
        "model_sha256",
    }
    report = exact_keys(document, expected_fields, "private lifecycle offline preflight")
    repository = exact_keys(
        report.get("repository"),
        {"head", "tree", "source_set_sha256"},
        "private lifecycle offline preflight repository",
    )
    if (
        report.get("schema") != OFFLINE_PREFLIGHT_SCHEMA
        or report.get("status") != "accepted"
        or report.get("evidence_level") != OFFLINE_PREFLIGHT_EVIDENCE_LEVEL
        or report.get("through") != through
        or any(
            report.get(field) is not False
            for field in ("validator_started", "rpc_used", "keys_read", "build_run")
        )
        or repository.get("head") != commit
        or repository.get("tree") != tree
    ):
        raise Refusal(
            "private lifecycle offline preflight did not accept this exact clean source mode"
        )
    source_sha256 = report.get("source_sha256")
    if not isinstance(source_sha256, dict) or not source_sha256:
        raise Refusal("private lifecycle offline preflight omitted its source digest set")
    for relative, digest in source_sha256.items():
        if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
            raise Refusal("private lifecycle offline preflight named a non-relative source")
        lowercase_sha256(digest, f"private lifecycle preflight source {relative}")
    source_set_sha256 = sha256_bytes(
        json.dumps(source_sha256, sort_keys=True, separators=(",", ":")).encode()
    )
    if source_set_sha256 != lowercase_sha256(
        repository.get("source_set_sha256"),
        "private lifecycle preflight source-set digest",
    ):
        raise Refusal("private lifecycle offline preflight source-set digest changed")
    runner_path = canonical_file(
        paths.repo / RUNNER_RELATIVE_PATH,
        "private-validator lifecycle runner in target source",
    )
    executing_runner = canonical_file(
        Path(__file__), "executing private-validator lifecycle runner"
    )
    preflight_path = canonical_file(
        paths.repo / OFFLINE_PREFLIGHT_RELATIVE_PATH,
        "private lifecycle offline preflight",
    )
    if executing_runner != runner_path:
        raise Refusal("executing lifecycle runner is outside the clean target source")
    expected_source_files = {
        str(RUNNER_RELATIVE_PATH): sha256_file(runner_path),
        str(OFFLINE_PREFLIGHT_RELATIVE_PATH): sha256_file(preflight_path),
    }
    for relative, digest in expected_source_files.items():
        if source_sha256.get(relative) != digest:
            raise Refusal(
                f"private lifecycle offline preflight did not bind exact {relative} bytes"
            )
    claimed_model = lowercase_sha256(
        report.get("model_sha256"), "private lifecycle offline preflight model"
    )
    model_material = dict(report)
    del model_material["model_sha256"]
    expected_model = sha256_bytes(
        json.dumps(model_material, sort_keys=True, separators=(",", ":")).encode()
    )
    if claimed_model != expected_model:
        raise Refusal("private lifecycle offline preflight model digest changed")
    return report


def run_offline_preflight(
    paths: Paths, commit: str, tree: str, through: str
) -> tuple[bytes, dict[str, Any]]:
    preflight_path = canonical_file(
        paths.repo / OFFLINE_PREFLIGHT_RELATIVE_PATH,
        "private lifecycle offline preflight",
    )
    try:
        result = subprocess.run(
            [
                sys.executable,
                str(preflight_path),
                "--repo",
                str(paths.repo),
                "--through",
                through,
            ],
            cwd=paths.repo,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Refusal(f"private lifecycle offline preflight did not complete: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace")[-4000:]
        raise Refusal(f"private lifecycle offline preflight refused:\n{detail}")
    if result.stderr:
        raise Refusal("accepted private lifecycle offline preflight wrote stderr")
    try:
        text = result.stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise Refusal("private lifecycle offline preflight stdout was not UTF-8") from error
    report = authenticate_offline_preflight(
        decode_unique_json(text, "private lifecycle offline preflight stdout"),
        paths=paths,
        commit=commit,
        tree=tree,
        through=through,
    )
    return result.stdout, report


def canonical_resolution_link(gate: dict[str, Any]) -> dict[str, Any]:
    """Select Resolution only through the checked release's canonical role.

    The private controller has no manual ELF argument.  This extra join keeps a
    checked gate from relabelling the banished aggregate `dclutch_sbf.so` as
    Resolution while preserving otherwise plausible frame and digest fields.
    """

    links = gate.get("links")
    if not isinstance(links, list):
        raise Refusal("checked release gate links are not one array")
    rows = [row for row in links if row.get("label") == CANONICAL_RESOLUTION_GATE_LABEL]
    if len(rows) != 1:
        raise Refusal("checked release gate must carry one canonical Resolution role")
    row = rows[0]
    elf = row.get("elf")
    marker = row.get("compile_marker")
    if (
        row.get("package") != CANONICAL_RESOLUTION_PACKAGE
        or not isinstance(marker, str)
        or CANONICAL_RESOLUTION_PACKAGE not in marker
        or row.get("checked_manifest") is None
        or not isinstance(elf, dict)
        or elf.get("canonical_path") != CANONICAL_RESOLUTION_ELF_PATH
        or Path(str(elf.get("canonical_path", ""))).name
        == BANISHED_RESOLUTION_ELF_BASENAME
        or not isinstance(elf.get("bytes"), int)
        or elf["bytes"] <= 0
        or elf["bytes"] == BANISHED_RESOLUTION_ELF_BYTES
    ):
        raise Refusal(
            "Resolution is not the checked dclutch-resolution-proof-sbf role; "
            "aggregate dclutch_sbf.so substitution is banished"
        )
    return row


def load_mixed_gate_authenticator(paths: Paths) -> Any:
    module_path = canonical_file(
        paths.repo / MIXED_GATE_AUTHENTICATOR_RELATIVE_PATH,
        "mixed checked-gate authenticator",
    )
    spec = importlib.util.spec_from_file_location(
        "dclutch_private_mixed_gate_authenticator", module_path
    )
    if spec is None or spec.loader is None:
        raise Refusal("mixed checked-gate authenticator could not be loaded")
    module = importlib.util.module_from_spec(spec)
    inserted = str(module_path.parent)
    sys.path.insert(0, inserted)
    try:
        spec.loader.exec_module(module)
    finally:
        if sys.path[0] == inserted:
            sys.path.pop(0)
        else:
            sys.path.remove(inserted)
    if not callable(getattr(module, "authenticate_existing_gate", None)):
        raise Refusal("mixed checked-gate authenticator omitted its shared API")
    return module


def checked_gate(paths: Paths, commit: str) -> tuple[Path, dict[str, Any], str]:
    gate_path = canonical_file(
        paths.release_root / "CHECKED_UPGRADE_GATE.json", "checked release gate"
    )
    gate = read_unique_json(gate_path, "checked release gate")
    gate_digest = sha256_file(gate_path)
    schema = gate.get("schema")
    if schema == "dclutch-checked-upgrade-gate-v1":
        if gate.get("source_revision") != commit:
            raise Refusal(
                f"checked release gate commit {gate.get('source_revision')} differs from clean source {commit}"
            )
        for actual, expected, label in (
            (gate_digest, paths.expected_release_gate_sha256, "gate SHA-256"),
            (
                gate.get("source_revision"),
                paths.expected_release_source_revision,
                "source revision",
            ),
            (
                gate.get("source_tree_sha256"),
                paths.expected_release_source_tree_sha256,
                "source tree SHA-256",
            ),
        ):
            if expected is not None and actual != expected:
                raise Refusal(f"checked release {label} differs from its explicit pin")
    elif schema == MIXED_GATE_SCHEMA:
        pins = (
            paths.expected_release_gate_sha256,
            paths.expected_release_source_revision,
            paths.expected_release_source_tree_sha256,
        )
        if any(value is None for value in pins):
            raise Refusal(
                "mixed checked release requires explicit gate, source revision, and source-tree pins"
            )
        authenticator = load_mixed_gate_authenticator(paths)
        try:
            selection = authenticator.authenticate_existing_gate(
                gate_path,
                paths.expected_release_gate_sha256,
                paths.expected_release_source_revision,
                paths.expected_release_source_tree_sha256,
                CANONICAL_RESOLUTION_GATE_LABEL,
            )
        except (
            OSError,
            KeyError,
            TypeError,
            ValueError,
            authenticator.Refusal,
        ) as error:
            raise Refusal(f"mixed checked release refused: {error}") from error
        resolution = canonical_resolution_link(gate)
        if (
            selection.get("schema")
            != "dclutch-checked-mixed-gate-link-selection-v1"
            or selection.get("gate_path") != str(gate_path)
            or selection.get("gate_sha256") != gate_digest
            or selection.get("source_revision") != gate.get("source_revision")
            or selection.get("source_tree_sha256")
            != gate.get("source_tree_sha256")
            or selection.get("label") != resolution.get("label")
            or selection.get("package") != resolution.get("package")
            or selection.get("elf") != resolution.get("elf")
            or selection.get("checked_manifest")
            != resolution.get("checked_manifest")
            or selection.get("artifact_provenance")
            != resolution.get("artifact_provenance")
        ):
            raise Refusal(
                "mixed checked release Resolution projection differs from its admitted gate row"
            )
    else:
        raise Refusal("checked release gate schema is neither admitted v1 nor v2")
    if gate.get("link_count") != 13 or len(gate.get("links", [])) != 13:
        raise Refusal(
            "checked release gate does not carry the exact thirteen-link closure"
        )
    labels = [link.get("label") for link in gate["links"]]
    if len(set(labels)) != 13:
        raise Refusal("checked release gate link labels are not unique")
    canonical_resolution_link(gate)
    for link in gate["links"]:
        if (
            link.get("sbf_diagnostics_count") != 0
            or link.get("frame_bound_bytes") != 4096
            or link.get("frames_at_or_over_bound") != 0
            or not isinstance(link.get("deepest_frame_bytes"), int)
            or link["deepest_frame_bytes"] >= 4096
        ):
            raise Refusal(
                f"checked release link {link.get('label')} is not below the frame bound"
            )
    tree = gate.get("source_tree_manifest", {})
    tree_path = canonical_file(
        paths.release_root / tree.get("canonical_path", ""), "source tree manifest"
    )
    if sha256_file(tree_path) != gate.get("source_tree_sha256") or sha256_file(
        tree_path
    ) != tree.get("sha256"):
        raise Refusal("checked release source-tree manifest digest changed")
    return gate_path, gate, gate_digest


def command_surface(bootstrap: Path, through: str) -> str:
    result = subprocess.run(
        [str(bootstrap), "help"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if result.returncode != 0:
        raise Refusal(
            f"successor help failed before validator launch:\n{result.stdout[-4000:]}"
        )
    required = list(FOUNDING_PARTICIPANT_COMMANDS)
    if through in ("full", "full-probe"):
        required.extend(
            (
                DIRECT_PRODUCER_COMMAND,
                DIRECT_EXECUTE_COMMAND,
                DIRECT_PAYOUT_SCHEDULE_COMMAND,
                PYTH_PROVISION_COMMAND,
                FLAGSHIP_RESOLUTION_COMMAND,
                PAYOUT_INPUT_COMMAND,
                PAYOUT_EXECUTE_COMMAND,
                TERMINAL_SEQUENCE_COMMAND,
                TERMINAL_RETIREMENT_COMMAND,
            )
        )
    if through == "full":
        required.extend(FINAL_EVIDENCE_COMMANDS)
    missing = [command for command in required if command not in result.stdout]
    if missing:
        raise Refusal(
            "full localhost lifecycle has no accepted caller for: " + ", ".join(missing)
        )
    return sha256_bytes(result.stdout.encode())


def reuse_bootstrap(paths: Paths, commit: str, gate_digest: str) -> Paths:
    source_work = paths.reuse_bootstrap_work
    if source_work is None:
        raise Refusal("internal bootstrap reuse omitted its source work directory")
    source_bootstrap = canonical_file(
        source_work / "host-target/release/dclutch-local-successor-bootstrap",
        "reused successor bootstrap",
    )
    source_summary_path = canonical_file(
        source_work / "SUMMARY.json", "bootstrap source summary"
    )
    source_receipt_path = canonical_file(
        source_work / "host-build/receipt.json", "bootstrap source build receipt"
    )
    summary = read_unique_json(source_summary_path, "bootstrap source summary")
    receipt = read_unique_json(source_receipt_path, "bootstrap source build receipt")
    binary_digest = sha256_file(source_bootstrap)
    if (
        summary.get("source_revision") != commit
        or summary.get("checked_release_gate_sha256") != gate_digest
        or summary.get("bootstrap_sha256") != binary_digest
    ):
        raise Refusal(
            "reused bootstrap is not bound to this exact source and checked release gate"
        )
    if (
        receipt.get("schema") != "dclutch-private-validator-host-build-receipt-v1"
        or receipt.get("exit_status") != 0
        or not isinstance(receipt.get("rustup_toolchain"), str)
        or not isinstance(receipt.get("rustc"), str)
    ):
        raise Refusal(
            "reused bootstrap lacks a successful pinned-toolchain build receipt"
        )
    canonical_file(receipt["rustc"], "reused bootstrap rustc")

    target = paths.work / "host-target/release"
    target.mkdir(parents=True)
    bootstrap = target / "dclutch-local-successor-bootstrap"
    shutil.copyfile(source_bootstrap, bootstrap)
    bootstrap.chmod(0o755)
    stage = paths.work / "host-build"
    stage.mkdir()
    write_json_new(
        stage / "receipt.json",
        {
            "schema": "dclutch-private-validator-host-build-reuse-receipt-v1",
            "source_revision": commit,
            "checked_release_gate_sha256": gate_digest,
            "bootstrap_sha256": binary_digest,
            "source_work": str(source_work),
            "source_summary": str(source_summary_path),
            "source_summary_sha256": sha256_file(source_summary_path),
            "source_build_receipt": str(source_receipt_path),
            "source_build_receipt_sha256": sha256_file(source_receipt_path),
            "rustup_toolchain": receipt["rustup_toolchain"],
            "rustc": receipt["rustc"],
        },
    )
    return dataclasses.replace(paths, bootstrap=bootstrap.resolve())


def build_bootstrap(paths: Paths, commit: str, gate_digest: str) -> Paths:
    if paths.reuse_bootstrap_work is not None:
        return reuse_bootstrap(paths, commit, gate_digest)
    cargo_text = shutil.which("cargo")
    if cargo_text is None:
        raise Refusal("cargo is unavailable for the source-pinned successor host build")
    cargo_candidate = Path(cargo_text)
    selected_toolchain: str | None = None
    selected_rustc: Path | None = None
    if cargo_candidate.is_symlink():
        rustup_text = shutil.which("rustup")
        if rustup_text is None:
            raise Refusal("cargo is a rustup selector but rustup is unavailable")
        selected_toolchain = subprocess.check_output(
            [rustup_text, "show", "active-toolchain"],
            cwd=paths.repo,
            text=True,
            stderr=subprocess.STDOUT,
        ).split()[0]
        selected = subprocess.check_output(
            [rustup_text, "which", "--toolchain", selected_toolchain, "cargo"],
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()
        cargo = canonical_file(selected, "rustup-selected cargo")
        rustc_text = subprocess.check_output(
            [rustup_text, "which", "--toolchain", selected_toolchain, "rustc"],
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()
        selected_rustc = canonical_file(rustc_text, "rustup-selected rustc")
    else:
        cargo = canonical_file(cargo_candidate, "cargo")
    stage = paths.work / "host-build"
    stage.mkdir()
    manifest = paths.repo / "tools/local-validator/bootstrap/successor/Cargo.toml"
    if canonical_file(manifest, "successor manifest") != manifest.resolve():
        raise Refusal(
            "successor manifest did not resolve canonically inside the clean source"
        )
    target = paths.work / "host-target"
    argv = [
        str(cargo),
        "build",
        "--locked",
        "--release",
        "--manifest-path",
        str(manifest),
    ]
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    if selected_toolchain is not None and selected_rustc is not None:
        # Directly invoking the selected Cargo binary otherwise lets its child
        # `rustc` shim fall back to the host default whenever Cargo compiles a
        # registry source outside the repository's override directory.
        environment["RUSTUP_TOOLCHAIN"] = selected_toolchain
        environment["RUSTC"] = str(selected_rustc)
    started = time.monotonic_ns()
    result = subprocess.run(
        argv,
        cwd=paths.repo,
        env=environment,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    elapsed = time.monotonic_ns() - started
    write_bytes_new(stage / "stdout.bin", result.stdout)
    write_bytes_new(stage / "stderr.bin", result.stderr)
    write_json_new(
        stage / "receipt.json",
        {
            "schema": "dclutch-private-validator-host-build-receipt-v1",
            "argv": argv,
            "exit_status": result.returncode,
            "elapsed_nanoseconds": elapsed,
            "rustup_toolchain": selected_toolchain,
            "rustc": None if selected_rustc is None else str(selected_rustc),
            "stdout_sha256": sha256_bytes(result.stdout),
            "stderr_sha256": sha256_bytes(result.stderr),
        },
    )
    if result.returncode != 0:
        raise Refusal(
            "source-pinned successor host build failed:\n"
            + (result.stdout + result.stderr).decode(errors="replace")[-6000:]
        )
    bootstrap = canonical_file(
        target / "release/dclutch-local-successor-bootstrap",
        "built successor bootstrap",
    )
    return dataclasses.replace(paths, bootstrap=bootstrap)


def allocate_port_block(seed_index: int) -> int:
    low, high, stride = 21000, 48000, 64
    count = (high - low) // stride
    start = (os.getpid() + seed_index * 17) % count
    for step in range(count):
        base = low + ((start + step) % count) * stride
        held: list[socket.socket] = []
        try:
            for offset in (0, 2, 3, *range(10, 42)):
                member = socket.socket()
                member.bind(("127.0.0.1", base + offset))
                held.append(member)
        except OSError:
            for member in held:
                member.close()
            continue
        for member in held:
            member.close()
        return base
    raise Refusal("no free complete 42-port localhost block")


def rpc(url: str, method: str, params: Sequence[Any] = ()) -> Any:
    if not url.startswith("http://127.0.0.1:"):
        raise Refusal(f"private-validator RPC escaped loopback: {url}")
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": list(params)}
    ).encode()
    call = request.Request(url, data=body, headers={"content-type": "application/json"})
    with request.urlopen(
        call, timeout=5
    ) as response:  # noqa: S310 - URL is constructed loopback
        decoded = json.load(response)
    if decoded.get("error") is not None:
        raise Refusal(f"localhost RPC {method} refused: {decoded['error']}")
    return decoded.get("result")


def wait_ready(url: str, child: subprocess.Popen[bytes], timeout: float = 60.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal(
                f"validator exited before readiness with status {child.returncode}"
            )
        try:
            if rpc(url, "getHealth") == "ok":
                return
        except Exception:
            pass
        time.sleep(0.25)
    raise Refusal("validator did not become healthy within 60 seconds")


def wait_finalized_slot(
    url: str,
    child: subprocess.Popen[bytes],
    minimum_slot: int,
    timeout: float = 60.0,
) -> int:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal(f"validator exited before finalized slot {minimum_slot}")
        observed = rpc(url, "getSlot", [{"commitment": "finalized"}])
        if isinstance(observed, int) and observed >= minimum_slot:
            return observed
        time.sleep(0.25)
    raise Refusal(f"validator did not finalize checked deployment slot {minimum_slot}")


def terminate_group(child: subprocess.Popen[bytes] | None) -> None:
    if child is None or child.poll() is not None:
        return
    with contextlib.suppress(ProcessLookupError):
        os.killpg(child.pid, signal.SIGTERM)
    try:
        child.wait(timeout=10)
    except subprocess.TimeoutExpired:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(child.pid, signal.SIGKILL)
        child.wait(timeout=10)


def participant_handoff_document(
    *,
    source_revision: str,
    checked_release_gate_sha256: str,
    rpc_url: str,
    validator_pid: int,
    plan: Path,
    market: Path,
    founding: Path,
    participant: Path,
    key_directory: Path,
) -> dict[str, Any]:
    """Describe one live finalized participant state without copying key bytes.

    This is a process-control owner, not protocol evidence.  Direct consumes
    only the named immutable evidence files and key paths while this exact
    supervisor-owned validator remains stopped at the handoff boundary.
    """

    if (
        len(source_revision) != 40
        or any(character not in "0123456789abcdef" for character in source_revision)
    ):
        raise Refusal("participant handoff source revision is not lowercase hex")
    if (
        len(checked_release_gate_sha256) != 64
        or any(
            character not in "0123456789abcdef"
            for character in checked_release_gate_sha256
        )
    ):
        raise Refusal("participant handoff checked-gate digest is not lowercase hex")
    if (
        not isinstance(validator_pid, int)
        or isinstance(validator_pid, bool)
        or validator_pid <= 0
    ):
        raise Refusal("participant handoff validator PID must be positive")
    # `rpc` owns the structural loopback refusal.  A read here is deliberately
    # omitted; the live health check happens only after SIGCONT so a substituted
    # or dead process cannot trigger teardown as an accepted handoff.
    parsed = urllib_parse.urlparse(rpc_url)
    try:
        rpc_port = parsed.port
    except ValueError as error:
        raise Refusal("participant handoff RPC port is invalid") from error
    if (
        parsed.scheme != "http"
        or parsed.hostname != "127.0.0.1"
        or rpc_port is None
        or not 0 < rpc_port <= 65535
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path not in ("", "/")
        or parsed.params
        or parsed.query
        or parsed.fragment
        or parsed.netloc != f"127.0.0.1:{rpc_port}"
    ):
        raise Refusal("participant handoff RPC escaped loopback")
    exact_plan = canonical_file(plan, "participant handoff plan")
    exact_market = canonical_file(market, "participant handoff market")
    exact_founding = canonical_file(founding, "participant handoff founding evidence")
    exact_participant = canonical_file(
        participant, "participant handoff participant evidence"
    )
    exact_keys = canonical_directory(key_directory, "participant handoff key directory")
    return {
        "schema": PARTICIPANT_HANDOFF_SCHEMA,
        "status": "ready",
        "sourceRevision": source_revision,
        "checkedReleaseGateSha256": checked_release_gate_sha256,
        "rpcUrl": rpc_url,
        "validatorPid": validator_pid,
        "plan": str(exact_plan),
        "marketInput": str(exact_market),
        "foundingEvidence": str(exact_founding),
        "participantEvidence": str(exact_participant),
        "participantSha256": sha256_file(exact_participant),
        "keyDirectory": str(exact_keys),
    }


def authenticate_participant_handoff(
    receipt_path: Path,
    expected: dict[str, Any],
    validator: subprocess.Popen[bytes],
) -> None:
    """Reopen the fsynced handoff and prove the original child still owns it."""

    exact_path = canonical_file(receipt_path, "participant handoff receipt")
    if stat.S_IMODE(exact_path.stat().st_mode) != 0o600:
        raise Refusal("participant handoff receipt mode changed from 0600")
    observed = read_unique_json(exact_path, "participant handoff receipt")
    if observed != expected:
        raise Refusal("participant handoff receipt changed while the supervisor was stopped")
    for key, label in (
        ("plan", "plan"),
        ("marketInput", "market input"),
        ("foundingEvidence", "founding evidence"),
        ("participantEvidence", "participant evidence"),
    ):
        canonical_file(observed[key], f"participant handoff {label}")
    canonical_directory(observed["keyDirectory"], "participant handoff key directory")
    if sha256_file(Path(observed["participantEvidence"])) != observed["participantSha256"]:
        raise Refusal("participant handoff participant evidence changed while stopped")
    if validator.pid != expected.get("validatorPid") or validator.poll() is not None:
        raise Refusal("participant handoff validator process identity changed")
    try:
        process_group = os.getpgid(validator.pid)
    except ProcessLookupError as error:
        raise Refusal("participant handoff validator process disappeared") from error
    if process_group != validator.pid:
        raise Refusal("participant handoff validator process group identity changed")
    if rpc(expected["rpcUrl"], "getHealth") != "ok":
        raise Refusal("participant handoff validator was not healthy after resume")


def hold_after_participant(
    receipt_path: Path,
    receipt: dict[str, Any],
    validator: subprocess.Popen[bytes],
) -> None:
    """Publish one durable boundary, stop this supervisor, and verify on resume."""

    write_json_new(receipt_path, receipt)
    os.kill(os.getpid(), signal.SIGSTOP)
    authenticate_participant_handoff(receipt_path, receipt, validator)


def write_json_new(path: Path, value: Any) -> None:
    data = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
    except Exception:
        with contextlib.suppress(OSError):
            os.close(descriptor)
        raise


def write_bytes_new(path: Path, data: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
    except Exception:
        with contextlib.suppress(OSError):
            os.close(descriptor)
        raise


def run_stage(
    run: Path, ordinal: int, label: str, argv: Sequence[str]
) -> subprocess.CompletedProcess[bytes]:
    stage = run / "stages" / f"{ordinal:02d}-{label}"
    stage.mkdir(parents=True, exist_ok=False)
    intent = {
        "schema": "dclutch-private-validator-stage-intent-v1",
        "label": label,
        "argv": list(argv),
        "started_unix_ns": time.time_ns(),
    }
    write_json_new(stage / "intent.json", intent)
    started = time.monotonic_ns()
    child = subprocess.Popen(
        argv,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    interrupted_error: BaseException | None = None
    try:
        stdout, stderr = child.communicate()
    except BaseException as error:
        interrupted_error = error
        terminate_group(child)
        stdout, stderr = child.communicate()
    finally:
        terminate_group(child)
    elapsed = time.monotonic_ns() - started
    result = subprocess.CompletedProcess(argv, child.returncode, stdout, stderr)
    write_bytes_new(stage / "stdout.bin", result.stdout)
    write_bytes_new(stage / "stderr.bin", result.stderr)
    receipt = {
        "schema": "dclutch-private-validator-stage-receipt-v1",
        "label": label,
        "exit_status": result.returncode,
        "elapsed_nanoseconds": elapsed,
        "stdout_sha256": sha256_bytes(result.stdout),
        "stderr_sha256": sha256_bytes(result.stderr),
        "interrupted": interrupted_error is not None,
    }
    write_json_new(stage / "receipt.json", receipt)
    if interrupted_error is not None:
        raise interrupted_error
    if result.returncode != 0:
        tail = result.stderr.decode(errors="replace")[-6000:]
        raise Refusal(f"stage {label} failed with status {result.returncode}:\n{tail}")
    return result


def key_address(solana: Path, keypair: Path) -> str:
    return subprocess.check_output(
        [str(solana), "address", "--keypair", str(keypair)],
        text=True,
        stderr=subprocess.STDOUT,
    ).strip()


def local_bankroll_transfer_argv(
    solana: Path,
    url: str,
    source_keypair: Path,
    payer_address: str,
) -> list[str]:
    return [
        str(solana),
        "--config",
        "/dev/null",
        "--url",
        url,
        "transfer",
        payer_address,
        "100",
        "--from",
        str(source_keypair),
        "--fee-payer",
        str(source_keypair),
        "--allow-unfunded-recipient",
        "--commitment",
        "finalized",
        "--output",
        "json-compact",
    ]


def validator_argv(
    validator: Path,
    ledger: Path,
    account_dir: str,
    mint_address: str,
    port: int,
) -> list[str]:
    """Launch only the exact plan-owned eighteen-account transaction genesis.

    Provider programs are already present in ``account_dir`` as immutable
    Loader-v3 Program/ProgramData pairs. Adding ``--upgradeable-program`` here
    would replace their tag-0 authority with Agave's tag-1 default pubkey.
    """

    return [
        str(validator),
        "--config",
        "/dev/null",
        "--ledger",
        str(ledger),
        "--account-dir",
        account_dir,
        "--mint",
        mint_address,
        "--ticks-per-slot",
        "16",
        "--bind-address",
        "127.0.0.1",
        "--rpc-port",
        str(port),
        "--faucet-port",
        str(port + 2),
        "--gossip-port",
        str(port + 3),
        "--dynamic-port-range",
        f"{port + 10}-{port + 41}",
    ]


def require_role_key(report: dict[str, Any], role: str) -> Path:
    keypairs = report.get("keypairs")
    if not isinstance(keypairs, dict) or not isinstance(keypairs.get(role), str):
        raise Refusal(f"local mutable preparation omitted disposable role {role}")
    return Path(keypairs[role])


def account_is_absent(url: str, address: str) -> bool:
    observed = rpc(url, "getAccountInfo", [address, {"commitment": "finalized"}])
    return isinstance(observed, dict) and observed.get("value") is None


def balance_lamports(url: str, address: str) -> int:
    observed = rpc(url, "getBalance", [address, {"commitment": "finalized"}])
    value = observed.get("value") if isinstance(observed, dict) else None
    if not isinstance(value, int) or value < 0:
        raise Refusal(
            f"localhost balance for {address} was not an integer lamport amount"
        )
    return value


def local_bankroll_snapshot(
    url: str,
    source: str,
    payer: str,
    vacant_roles: Sequence[tuple[str, str]],
) -> dict[str, Any]:
    addresses = [source, payer, *(address for _, address in vacant_roles)]
    if len(set(addresses)) != len(addresses):
        raise Refusal("local bankroll identities alias")
    observed = rpc(
        url,
        "getMultipleAccounts",
        [addresses, {"commitment": "finalized", "encoding": "base64"}],
    )
    context = observed.get("context") if isinstance(observed, dict) else None
    values = observed.get("value") if isinstance(observed, dict) else None
    slot = context.get("slot") if isinstance(context, dict) else None
    if not isinstance(slot, int) or slot < 0 or not isinstance(values, list) or len(values) != len(addresses):
        raise Refusal("local bankroll snapshot was not one complete finalized observation")

    def system_wallet(value: Any, label: str) -> int:
        data = value.get("data") if isinstance(value, dict) else None
        lamports = value.get("lamports") if isinstance(value, dict) else None
        if (
            not isinstance(value, dict)
            or value.get("owner") != SYSTEM_PROGRAM_ADDRESS
            or value.get("executable") is not False
            or data != ["", "base64"]
            or not isinstance(lamports, int)
            or lamports < 0
        ):
            raise Refusal(f"{label} was not one exact System wallet")
        return lamports

    source_lamports = system_wallet(values[0], "local bankroll source")
    payer_lamports = None if values[1] is None else system_wallet(values[1], "campaign payer")
    vacant = []
    for (role, address), value in zip(vacant_roles, values[2:], strict=True):
        if value is not None:
            raise Refusal(f"protocol-created disposable role {role} already exists")
        vacant.append({"role": role, "address": address})
    return {
        "finalizedSlot": str(slot),
        "sourceLamports": str(source_lamports),
        "campaignPayerLamports": (
            None if payer_lamports is None else str(payer_lamports)
        ),
        "vacantProtocolRoles": vacant,
    }


def finalized_local_bankroll_transaction(
    url: str,
    signature: str,
    source: str,
    payer: str,
) -> dict[str, Any]:
    value = rpc(
        url,
        "getTransaction",
        [
            signature,
            {
                "commitment": "finalized",
                "encoding": "jsonParsed",
                "maxSupportedTransactionVersion": 0,
            },
        ],
    )
    if not isinstance(value, dict):
        raise Refusal("local bankroll transaction was not finalized")
    slot = value.get("slot")
    meta = value.get("meta")
    transaction = value.get("transaction")
    message = transaction.get("message") if isinstance(transaction, dict) else None
    signatures = transaction.get("signatures") if isinstance(transaction, dict) else None
    instructions = message.get("instructions") if isinstance(message, dict) else None
    keys = message.get("accountKeys") if isinstance(message, dict) else None
    if (
        not isinstance(slot, int)
        or slot <= 0
        or not isinstance(meta, dict)
        or meta.get("err") is not None
        or not isinstance(signatures, list)
        or signatures != [signature]
        or not isinstance(instructions, list)
        or len(instructions) != 1
        or not isinstance(keys, list)
    ):
        raise Refusal("local bankroll transaction changed its finalized envelope")
    key_rows = [
        row.get("pubkey") if isinstance(row, dict) else row
        for row in keys
    ]
    if key_rows.count(source) != 1 or key_rows.count(payer) != 1 or key_rows[0] != source:
        raise Refusal("local bankroll transaction changed its source, payer, or fee payer")
    instruction = instructions[0]
    parsed = instruction.get("parsed") if isinstance(instruction, dict) else None
    info = parsed.get("info") if isinstance(parsed, dict) else None
    if (
        not isinstance(instruction, dict)
        or instruction.get("programId") != SYSTEM_PROGRAM_ADDRESS
        or not isinstance(parsed, dict)
        or parsed.get("type") != "transfer"
        or not isinstance(info, dict)
        or info.get("source") != source
        or info.get("destination") != payer
        or info.get("lamports") != LOCAL_TEST_BANKROLL_LAMPORTS
        or meta.get("innerInstructions") not in (None, [])
    ):
        raise Refusal("local bankroll transaction was not the exact System transfer")
    fee = meta.get("fee")
    compute = meta.get("computeUnitsConsumed")
    pre = meta.get("preBalances")
    post = meta.get("postBalances")
    if (
        isinstance(fee, bool)
        or not isinstance(fee, int)
        or fee < 0
        or isinstance(compute, bool)
        or not isinstance(compute, int)
        or compute <= 0
        or not isinstance(pre, list)
        or not isinstance(post, list)
        or len(pre) != len(keys)
        or len(post) != len(keys)
        or any(isinstance(item, bool) or not isinstance(item, int) for item in pre)
        or any(isinstance(item, bool) or not isinstance(item, int) for item in post)
    ):
        raise Refusal("local bankroll transaction omitted exact fee, CU, or balances")
    source_index = key_rows.index(source)
    payer_index = key_rows.index(payer)
    source_key = keys[source_index]
    payer_key = keys[payer_index]
    if (
        not isinstance(source_key, dict)
        or source_key.get("signer") is not True
        or source_key.get("writable") is not True
        or not isinstance(payer_key, dict)
        or payer_key.get("signer") is not False
        or payer_key.get("writable") is not True
        or pre[source_index] - post[source_index]
        != LOCAL_TEST_BANKROLL_LAMPORTS + fee
        or post[payer_index] - pre[payer_index] != LOCAL_TEST_BANKROLL_LAMPORTS
        or pre[payer_index] != 0
    ):
        raise Refusal("local bankroll transaction changed exact transfer and fee conservation")
    return {
        "signature": signature,
        "finalizedSlot": str(slot),
        "feeLamports": str(fee),
        "computeUnitsConsumed": str(compute),
        "sourcePreLamports": str(pre[source_index]),
        "sourcePostLamports": str(post[source_index]),
        "campaignPayerPreLamports": str(pre[payer_index]),
        "campaignPayerPostLamports": str(post[payer_index]),
    }


def provision_disposable_funding(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
) -> dict[str, Any]:
    if LOCAL_AIRDROP_ROLES:
        raise Refusal("local lifecycle has no admitted airdrop role")
    receipt_path = run / "provisioning-poststate.json"
    if receipt_path.exists() or receipt_path.is_symlink():
        raise Refusal("local test-bankroll receipt already exists")
    source_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    payer_key = require_role_key(report, CAMPAIGN_PAYER_ROLE)
    source = canonical_pubkey(
        key_address(paths.solana, source_key), "local bankroll source"
    )
    payer = canonical_pubkey(
        key_address(paths.solana, payer_key), "campaign payer"
    )
    vacant_roles = tuple(
        (
            role,
            canonical_pubkey(
                key_address(paths.solana, require_role_key(report, role)),
                f"protocol-created role {role}",
            ),
        )
        for role in PROTOCOL_CREATED_KEY_ROLES
    )
    prestate = local_bankroll_snapshot(url, source, payer, vacant_roles)
    if prestate["campaignPayerLamports"] is not None:
        raise Refusal("campaign payer was not absent before its one local bankroll transfer")
    source_before = canonical_decimal(
        prestate["sourceLamports"], "local bankroll source prestate"
    )
    if source_before <= LOCAL_TEST_BANKROLL_LAMPORTS:
        raise Refusal("validator genesis wallet cannot cover the local test bankroll plus fee")

    transfer = run_stage(
        run,
        3,
        "local-test-bankroll",
        local_bankroll_transfer_argv(
            paths.solana,
            url,
            source_key,
            payer,
        ),
    )
    try:
        transfer_stdout = json.loads(transfer.stdout)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise Refusal(f"local bankroll transfer stdout was not JSON: {error}") from error
    if not isinstance(transfer_stdout, dict) or set(transfer_stdout) != {"signature"}:
        raise Refusal("local bankroll transfer stdout changed its exact shape")
    signature = canonical_signature(
        transfer_stdout["signature"], "local bankroll transfer signature"
    )
    transaction = finalized_local_bankroll_transaction(url, signature, source, payer)
    poststate = local_bankroll_snapshot(url, source, payer, vacant_roles)
    pre_slot = canonical_decimal(prestate["finalizedSlot"], "bankroll prestate slot", positive=False)
    tx_slot = canonical_decimal(transaction["finalizedSlot"], "bankroll transaction slot")
    post_slot = canonical_decimal(poststate["finalizedSlot"], "bankroll poststate slot", positive=False)
    if not pre_slot <= tx_slot <= post_slot:
        raise Refusal("local bankroll snapshots do not bound the finalized transaction")
    if (
        prestate["sourceLamports"] != transaction["sourcePreLamports"]
        or transaction["sourcePostLamports"] != poststate["sourceLamports"]
        or transaction["campaignPayerPreLamports"] != "0"
        or poststate["campaignPayerLamports"]
        != transaction["campaignPayerPostLamports"]
        or poststate["campaignPayerLamports"] != str(LOCAL_TEST_BANKROLL_LAMPORTS)
        or prestate["vacantProtocolRoles"] != poststate["vacantProtocolRoles"]
    ):
        raise Refusal("local bankroll prestate, transaction, and poststate do not join")
    receipt = {
        "schema": LOCAL_TEST_BANKROLL_SCHEMA,
        "cluster": "owned-loopback",
        "genesisHash": canonical_pubkey(rpc(url, "getGenesisHash"), "local genesis"),
        "status": "finalized",
        "classification": (
            "exact 100 SOL local-validator test bankroll; not a projected minimum or devnet arithmetic"
        ),
        "amountLamports": str(LOCAL_TEST_BANKROLL_LAMPORTS),
        "source": {"role": VALIDATOR_MINT_ROLE, "address": source},
        "campaignPayer": {"role": CAMPAIGN_PAYER_ROLE, "address": payer},
        "prestate": prestate,
        "transaction": transaction,
        "poststate": poststate,
        "solanaBinarySha256": sha256_file(paths.solana),
        "externalWrites": False,
    }
    write_json_new(receipt_path, receipt)
    return receipt


def run_pyth_provisioning(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    first_ordinal: int,
) -> tuple[Path, dict[str, Any], int]:
    """Execute and reauthenticate the exact eight-action local Pyth owner."""

    payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    encoded_key = require_role_key(report, "pyth-encoded-vaa")
    update_key = require_role_key(report, "pyth-update-account")
    payer = key_address(paths.solana, payer_key)
    encoded = key_address(paths.solana, encoded_key)
    update = key_address(paths.solana, update_key)
    if len({payer, encoded, update}) != 3:
        raise Refusal("local Pyth payer, EncodedVaa, and update roles are not distinct")
    if not account_is_absent(url, encoded) or not account_is_absent(url, update):
        raise Refusal("local Pyth EncodedVaa and update roles must both begin vacant")

    journal_dir = run / "journals" / "pyth"
    journal_dir.mkdir(parents=True, exist_ok=False)
    facts = run / "pyth-update-facts.json"
    argv = [
        str(paths.bootstrap),
        PYTH_PROVISION_COMMAND,
        "--rpc-url",
        url,
        "--payer",
        payer,
        "--encoded-vaa",
        encoded,
        "--update-account",
        update,
        "--journal-dir",
        str(journal_dir),
        "--facts-output",
        str(facts),
        "--payer-keypair",
        str(payer_key),
        "--encoded-vaa-keypair",
        str(encoded_key),
        "--execute",
    ]
    journal_rows: list[dict[str, Any]] = []
    prior_slot = 0
    for offset, file_name in enumerate(PYTH_JOURNAL_FILES):
        label = "pyth-" + file_name.removesuffix(".json")
        run_stage(run, first_ordinal + offset, label, argv)
        expected = set(PYTH_JOURNAL_FILES[: offset + 1])
        observed = {path.name for path in journal_dir.glob("*.json")}
        if observed != expected:
            raise Refusal(
                f"local Pyth action {label} produced a noncanonical journal prefix"
            )
        path = journal_dir / file_name
        journal = read_unique_json(path, f"local Pyth {label} journal")
        if (
            journal.get("schema") != PYTH_JOURNAL_SCHEMA
            or journal.get("cluster") != "owned-loopback"
            or journal.get("authorizedMutation") is not True
            or journal.get("phase") != "finalized"
            or not isinstance(journal.get("finalized"), dict)
            or not isinstance(journal.get("intent"), dict)
            or journal["intent"].get("action") != PYTH_ACTIONS[offset]
        ):
            raise Refusal(
                f"local Pyth action {label} did not preserve finalized evidence"
            )
        finalized = finalized_fact(journal["finalized"], f"local Pyth {label}")
        if (
            journal.get("expectedSignature") != finalized["signature"]
            or journal["intent"].get("exactFeeLamports") != finalized["fee_lamports"]
            or finalized["slot"] < prior_slot
        ):
            raise Refusal(
                f"local Pyth action {label} changed signature, exact fee, or slot order"
            )
        prior_slot = finalized["slot"]
        journal_rows.append(
            {
                "action": PYTH_ACTIONS[offset],
                "path": str(path),
                "sha256": sha256_file(path),
                **finalized,
            }
        )

    final_stage = run_stage(
        run,
        first_ordinal + len(PYTH_JOURNAL_FILES),
        "pyth-finalize",
        argv,
    )
    final_summary = json.loads(final_stage.stdout)
    document = read_unique_json(facts, "local Pyth update facts")
    if (
        final_summary.get("schema") != "dclutch-owned-loopback-pyth-prerequisites-v1"
        or final_summary.get("status") != "finalized"
        or final_summary.get("encodedVaa") != encoded
        or final_summary.get("updateAccount") != update
        or final_summary.get("facts") != str(facts)
        or document.get("format") != "dclutch-flagship-pyth-update-facts-v1"
        or document.get("encodedVaa") != encoded
        or document.get("updateAccount") != update
        or set(document)
        != {"format", "encodedVaa", "updateAccount", "postUpdateBodyBase64"}
    ):
        raise Refusal(
            "local Pyth final summary or exact four-field facts projection changed"
        )
    try:
        post_update_body = base64.b64decode(
            document["postUpdateBodyBase64"], validate=True
        )
    except (ValueError, TypeError) as error:
        raise Refusal("local Pyth PostUpdate body is not canonical base64") from error
    if not post_update_body:
        raise Refusal("local Pyth PostUpdate body is empty")
    if not account_is_absent(url, update):
        raise Refusal("local Pyth provisioner populated the Receiver update account")
    evidence = {
        "schema": "dclutch-private-validator-pyth-provisioning-v1",
        "status": "finalized",
        "payer": payer,
        "encoded_vaa": encoded,
        "update_account": update,
        "update_account_remained_vacant": True,
        "facts": str(facts),
        "facts_sha256": sha256_file(facts),
        "journals": journal_rows,
        "compute_units": {
            row["action"]: row["compute_units_consumed"] for row in journal_rows
        },
    }
    write_json_new(run / "pyth-provisioning.json", evidence)
    return facts, evidence, first_ordinal + len(PYTH_JOURNAL_FILES) + 1


def run_json_stage(
    run: Path,
    ordinal: int,
    label: str,
    argv: Sequence[str],
) -> dict[str, Any]:
    result = run_stage(run, ordinal, label, argv)
    try:
        text = result.stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise Refusal(f"stage {label} stdout was not UTF-8 JSON") from error
    document = decode_unique_json(text, f"stage {label} stdout")
    if not isinstance(document, dict):
        raise Refusal(f"stage {label} stdout was not one JSON object")
    return document


def exact_keys(document: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(document, dict) or set(document) != expected:
        raise Refusal(f"{label} fields changed from its exact semantic-owner schema")
    return document


def lowercase_sha256(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise Refusal(f"{label} must be one lowercase SHA-256")
    return value


def semantic_owner_digest(document: dict[str, Any], field: str, label: str) -> str:
    """Reproduce a Rust semantic owner's blank-self-field serde digest."""

    if field not in document:
        raise Refusal(f"{label} omitted its self digest")
    material = {**document, field: ""}
    return sha256_bytes(
        json.dumps(material, ensure_ascii=False, separators=(",", ":")).encode()
    )


def resolution_producer_identity_sha256(producer: dict[str, Any]) -> str:
    """Reproduce flagship Resolution's canonical producer-identity tuple."""

    tables = producer.get("tables")
    if not isinstance(tables, dict) or set(tables) != {"submit", "execute", "reclaim"}:
        raise Refusal("Resolution producer identity omitted its exact three table plans")
    ordered_tables = {
        stage: tables[stage] for stage in ("submit", "execute", "reclaim")
    }
    identity = [
        producer.get("planSha256"),
        producer.get("campaignEvidenceSha256"),
        producer.get("pythFactsSha256"),
        producer.get("market"),
        producer.get("generation"),
        producer.get("payer"),
        producer.get("authority"),
        ordered_tables,
        producer.get("plannedInput"),
    ]
    return sha256_bytes(
        json.dumps(identity, ensure_ascii=False, separators=(",", ":")).encode()
    )


def authenticate_resolution_producer(
    document: Any,
    *,
    require_complete: bool,
    plan: Path,
    campaign_evidence: Path,
    pyth_facts: Path,
) -> dict[str, Any]:
    producer = exact_keys(
        document,
        {
            "format",
            "planSha256",
            "campaignEvidenceSha256",
            "pythFactsSha256",
            "observationSlot",
            "observationUnixTimestamp",
            "market",
            "generation",
            "payer",
            "authority",
            "tables",
            "routes",
            "plannedInput",
            "flagshipInput",
        },
        "Resolution producer checkpoint",
    )
    expected_source_digests = {
        "planSha256": sha256_file(plan),
        "campaignEvidenceSha256": sha256_file(campaign_evidence),
        "pythFactsSha256": sha256_file(pyth_facts),
    }
    for field, expected in expected_source_digests.items():
        if lowercase_sha256(producer.get(field), f"Resolution {field}") != expected:
            raise Refusal(f"Resolution {field} differs from its exact source file")
    if producer.get("format") != RESOLUTION_PRODUCER_SCHEMA:
        raise Refusal(
            "Resolution producer checkpoint used another owned-loopback schema"
        )
    market = canonical_pubkey(producer.get("market"), "Resolution Market")
    payer = canonical_pubkey(producer.get("payer"), "Resolution payer")
    if payer != canonical_pubkey(producer.get("authority"), "Resolution authority"):
        raise Refusal("Resolution table payer and authority differ")
    positive_integer(producer.get("observationSlot"), "Resolution observation slot")
    if (
        isinstance(producer.get("generation"), bool)
        or not isinstance(producer.get("generation"), int)
        or producer["generation"] < 0
    ):
        raise Refusal("Resolution generation is not one u64")
    tables = producer.get("tables")
    routes = producer.get("routes")
    if not isinstance(tables, dict) or set(tables) != {"submit", "execute", "reclaim"}:
        raise Refusal("Resolution producer omitted its exact three table plans")
    if not isinstance(routes, dict) or set(routes) != set(tables):
        raise Refusal("Resolution producer omitted its exact three table routes")
    planned = producer.get("plannedInput")
    if (
        not isinstance(planned, dict)
        or planned.get("format") != RESOLUTION_INPUT_SCHEMA
    ):
        raise Refusal("Resolution producer planned another input schema")
    if planned.get("accounts", {}).get("market") != market:
        raise Refusal("Resolution producer planned input changed its Market")
    complete = all(
        isinstance(route, dict) and route.get("action") == "complete"
        for route in routes.values()
    )
    if require_complete and (not complete or producer.get("flagshipInput") != planned):
        raise Refusal("Resolution producer has not frozen all three exact tables")
    if not require_complete and producer.get("flagshipInput") not in (None, planned):
        raise Refusal("Resolution producer carried a substituted flagship input")
    return producer


def authenticate_resolution_table_journal(
    document: Any,
    *,
    require_complete: bool,
    producer: dict[str, Any],
) -> list[dict[str, Any]]:
    journal = exact_keys(
        document,
        {
            "format",
            "producerIdentitySha256",
            "phase",
            "intent",
            "intentSha256",
            "signedTransactionBase64",
            "signedTransactionSha256",
            "expectedSignature",
            "finalized",
            "receipts",
        },
        "Resolution table journal",
    )
    if journal.get("format") != RESOLUTION_TABLE_SCHEMA:
        raise Refusal("Resolution table journal used another owned-loopback schema")
    if (
        lowercase_sha256(
            journal.get("producerIdentitySha256"), "Resolution producer identity"
        )
        != resolution_producer_identity_sha256(producer)
    ):
        raise Refusal("Resolution table journal names another producer identity")
    receipts = journal.get("receipts")
    if not isinstance(receipts, list):
        raise Refusal("Resolution table journal receipts are not one array")
    signatures: set[str] = set()
    prior_slot = 0
    facts: list[dict[str, Any]] = []
    for index, receipt in enumerate(receipts):
        fact = finalized_fact(receipt, f"Resolution table receipt {index}")
        if fact["signature"] in signatures or fact["slot"] < prior_slot:
            raise Refusal(
                "Resolution table receipts repeat a signature or regress slots"
            )
        signatures.add(fact["signature"])
        prior_slot = fact["slot"]
        facts.append(fact)
    if require_complete and (
        journal.get("phase") != "finalized"
        or journal.get("intent") is not None
        or journal.get("finalized") is not None
    ):
        raise Refusal("Resolution table journal still carries an unfinished action")
    return facts


def authenticate_resolution_checkpoint(
    document: Any, *, input_path: Path
) -> list[dict[str, Any]]:
    checkpoint = exact_keys(
        document,
        {"format", "inputSha256", "stagePlan", "receipts", "verifiedTerminal"},
        "Resolution checkpoint",
    )
    if (
        checkpoint.get("format") != RESOLUTION_CHECKPOINT_SCHEMA
        or checkpoint.get("stagePlan") is not None
        or checkpoint.get("verifiedTerminal") is not True
    ):
        raise Refusal("Resolution checkpoint is not verified terminal")
    if (
        lowercase_sha256(checkpoint.get("inputSha256"), "Resolution input digest")
        != sha256_file(input_path)
    ):
        raise Refusal("Resolution checkpoint names another exact input file")
    receipts = checkpoint.get("receipts")
    if not isinstance(receipts, list) or [row.get("stage") for row in receipts] != [
        "submit",
        "resolution-provider-execute-v1",
        "core-terminal-accept-v1",
        "reclaim",
    ]:
        raise Refusal(
            "Resolution checkpoint omitted submit/provider-execute/Core-accept/reclaim receipts"
        )
    facts = [
        finalized_fact(row, f"Resolution {row.get('stage')} receipt")
        for row in receipts
    ]
    if len({fact["signature"] for fact in facts}) != len(facts) or any(
        left["slot"] >= right["slot"] for left, right in zip(facts, facts[1:])
    ):
        raise Refusal("Resolution receipts repeat a signature or do not advance slots")
    return facts


def keypairs_by_address(paths: Paths, report: dict[str, Any]) -> dict[str, Path]:
    rows = report.get("keypairs")
    if not isinstance(rows, dict) or not rows:
        raise Refusal("local mutable preparation omitted disposable keypairs")
    output: dict[str, Path] = {}
    for role, value in sorted(rows.items()):
        if not isinstance(role, str) or not isinstance(value, str):
            raise Refusal("local mutable keypair projection changed shape")
        key = canonical_file(value, f"disposable keypair {role}")
        address = canonical_pubkey(
            key_address(paths.solana, key), f"disposable key {role}"
        )
        if address in output:
            raise Refusal("two disposable keypair roles alias one signer")
        output[address] = key
    return output


def require_keypair(keys: dict[str, Path], address: str, label: str) -> Path:
    canonical_pubkey(address, label)
    try:
        return keys[address]
    except KeyError as error:
        raise Refusal(
            f"{label} is not one supervisor-owned disposable signer"
        ) from error


def authenticate_payout_input(
    document: Any,
    target: PayoutTarget,
    market: str,
) -> dict[str, Any]:
    payout = exact_keys(
        document,
        {
            "format",
            "market",
            "owner",
            "recipientOwner",
            "recipient",
            "collateralMint",
            "tokenProgram",
            "quantity",
            "claimIndex",
            "transferIndex",
            "parentContext",
            "custodyContext",
            "releaseSet",
            "terminalCertificate",
            "programs",
            "records",
        },
        "wallet payout input",
    )
    if (
        payout.get("format") != PAYOUT_INPUT_SCHEMA
        or payout.get("market") != market
        or payout.get("owner") != target.owner
        or payout.get("recipientOwner") != target.owner
        or payout.get("recipient") != target.recipient
        or payout.get("claimIndex") != target.claim_index
        or payout.get("transferIndex") != 0
    ):
        raise Refusal("wallet payout input changed Direct's canonical target identity")
    quantity = canonical_decimal(payout.get("quantity"), "wallet payout quantity")
    if quantity != target.quantity_atoms:
        raise Refusal("wallet payout quantity changed Direct's frozen schedule")
    for field in ("collateralMint", "tokenProgram", "terminalCertificate"):
        canonical_pubkey(payout.get(field), f"wallet payout {field}")
    return payout


def authenticate_payout_evidence(
    document: Any,
    input_path: Path,
    target: PayoutTarget,
    market: str,
) -> dict[str, Any]:
    evidence = exact_keys(
        document,
        {
            "schema",
            "cluster",
            "inputSha256",
            "payoutIntentSha256",
            "journalStateSha256",
            "signature",
            "finalizedSlot",
            "feeLamports",
            "computeUnitsConsumed",
            "feePayer",
            "owner",
            "market",
            "recipient",
            "payout",
            "lookupTable",
            "lookupAddressesSha256",
            "payoutInstructionSha256",
            "custodyRequestSha256",
            "returnDataProducer",
            "returnDataBase64",
            "poststates",
            "evidenceSha256",
        },
        "wallet payout evidence",
    )
    if (
        evidence.get("schema") != PAYOUT_EVIDENCE_SCHEMA
        or evidence.get("cluster") != "owned-loopback"
        or evidence.get("inputSha256") != sha256_file(input_path)
        or evidence.get("owner") != target.owner
        or evidence.get("market") != market
        or evidence.get("recipient") != target.recipient
    ):
        raise Refusal(
            "wallet payout evidence changed its exact input/owner/market/recipient join"
        )
    finalized_fact(
        evidence,
        "wallet payout evidence",
        slot_key="finalizedSlot",
    )
    for field in (
        "inputSha256",
        "payoutIntentSha256",
        "journalStateSha256",
        "lookupAddressesSha256",
        "payoutInstructionSha256",
        "evidenceSha256",
    ):
        lowercase_sha256(evidence.get(field), f"wallet payout {field}")
    return evidence


def authenticate_terminal_sequence_journal(
    document: Any, *, url: str, label: str
) -> dict[str, Any]:
    journal = exact_keys(
        document,
        {
            "schema",
            "cluster",
            "rpcUrl",
            "authorizedMutation",
            "stateSha256",
            "phase",
            "intentSha256",
            "intent",
            "signedPacketBase64",
            "expectedSignature",
            "finalized",
        },
        label,
    )
    intent = journal.get("intent")
    mutation = intent.get("mutation") if isinstance(intent, dict) else None
    finalization = journal.get("finalized")
    fact = finalized_fact(finalization, f"{label} finalization")
    if (
        journal.get("schema") != TERMINAL_JOURNAL_SCHEMA
        or journal.get("cluster") != "owned-loopback"
        or journal.get("rpcUrl") != url
        or journal.get("authorizedMutation") is not True
        or journal.get("phase") != "finalized"
        or not isinstance(mutation, dict)
        or not isinstance(mutation.get("kind"), str)
        or not isinstance(journal.get("signedPacketBase64"), str)
        or journal.get("expectedSignature") != fact["signature"]
        or not isinstance(intent, dict)
        or sha256_bytes(
            json.dumps(intent, ensure_ascii=False, separators=(",", ":")).encode()
        )
        != lowercase_sha256(journal.get("intentSha256"), f"{label} intent")
        or semantic_owner_digest(journal, "stateSha256", label)
        != lowercase_sha256(journal.get("stateSha256"), f"{label} state")
    ):
        raise Refusal(f"{label} was not its exact finalized semantic owner")
    lowercase_sha256(finalization.get("packetSha256"), f"{label} packet")
    return {
        "mutation": mutation,
        "state_sha256": journal["stateSha256"],
        "finalized": fact,
    }


def terminal_sequence_finalized_history(
    journal_dir: Path, *, url: str
) -> list[dict[str, Any]]:
    paths = sorted(journal_dir.glob("*.json"), key=lambda path: path.name)
    rows = []
    for index, path in enumerate(paths):
        canonical_file(path, f"terminal sequence journal {index}")
        row = authenticate_terminal_sequence_journal(
            read_unique_json(path, f"terminal sequence journal {index}"),
            url=url,
            label=f"terminal sequence journal {index}",
        )
        row["path"] = str(path)
        row["sha256"] = sha256_file(path)
        rows.append(row)
    kinds = [row["mutation"]["kind"] for row in rows]
    expected = ["lookup-create"]
    extension_count = kinds.count("lookup-extend")
    if extension_count < 1:
        raise Refusal("terminal sequence omitted its ALT extension prefix")
    expected.extend(["lookup-extend"] * extension_count)
    expected.extend(
        [
            "lookup-freeze",
            "core-begin-retiring",
            "direct-begin-retiring",
        ]
    )
    if "resolution-receipt-prepay" in kinds:
        expected.append("resolution-receipt-prepay")
    expected.extend(
        [
            "resolution-close-fund",
            "direct-close-capability",
            "retirement-replay-handoff",
        ]
    )
    if kinds != expected:
        raise Refusal("terminal sequence changed its exact finalized handoff order")
    extension_prefixes = []
    for path, row in zip(paths, rows, strict=True):
        kind = row["mutation"]["kind"]
        expected_name = {
            "lookup-create": "00-alt-create.json",
            "lookup-freeze": "02-alt-freeze.json",
            "core-begin-retiring": "10-core-begin-retiring.json",
            "direct-begin-retiring": "11-direct-begin-retiring.json",
            "resolution-receipt-prepay": "12-resolution-receipt-prepay.json",
            "resolution-close-fund": "13-resolution-close-fund.json",
            "direct-close-capability": "14-direct-close-capability.json",
            "retirement-replay-handoff": "15-retirement-replay-handoff.json",
        }.get(kind)
        if kind == "lookup-extend":
            prefix = positive_integer(
                row["mutation"].get("prefixLen"), "terminal ALT extension prefix"
            )
            extension_prefixes.append(prefix)
            expected_name = f"01-alt-extend-{prefix:03}.json"
        elif set(row["mutation"]) != {"kind"}:
            raise Refusal("terminal sequence mutation fields changed")
        if path.name != expected_name:
            raise Refusal("terminal sequence journal filename changed")
    if extension_prefixes != sorted(set(extension_prefixes)):
        raise Refusal("terminal ALT extension prefixes were not canonical")
    return rows


def terminal_sequence_handoff(
    session_path: Path,
    journal_dir: Path,
    *,
    url: str,
    plan: Path,
    market_input: Path,
    evidence: Path,
    market: str,
    payer: str,
) -> dict[str, Any] | None:
    handoff_path = journal_dir / "15-retirement-replay-handoff.json"
    if not handoff_path.exists():
        return None
    if (journal_dir / "16-aggregate-retirement.json").exists():
        raise Refusal(
            "packet-inadmissible monolithic aggregate-retirement journal appeared"
        )
    session = exact_keys(
        read_unique_json(session_path, "terminal sequence session"),
        {
            "schema",
            "ownedLoopbackGenesisHash",
            "rpcUrl",
            "planSha256",
            "marketInputSha256",
            "evidenceSha256",
            "market",
            "payer",
            "sourceReceipt",
            "receiptInitialLamports",
            "receiptRentLamports",
            "suppliedLookupTable",
            "lookupTable",
            "lookupRecentSlot",
            "lookupAddresses",
            "lookupAddressesSha256",
            "sessionSha256",
        },
        "terminal sequence session",
    )
    initial_rent = nonnegative_integer(
        session.get("receiptInitialLamports"), "terminal receipt initial rent"
    )
    receipt_rent = nonnegative_integer(
        session.get("receiptRentLamports"), "terminal receipt rent"
    )
    addresses = session.get("lookupAddresses")
    if not isinstance(addresses, list) or not addresses:
        raise Refusal("terminal session omitted its exact ALT union")
    decoded = [
        base58_bytes(address, 32, f"terminal ALT address {index}")
        for index, address in enumerate(addresses)
    ]
    vector_digest = hashlib.sha256()
    vector_digest.update(b"dclutch/terminal-alt-stable-addresses/v1")
    vector_digest.update(len(decoded).to_bytes(8, "little"))
    for address in decoded:
        vector_digest.update(address)
    if (
        session.get("schema") != TERMINAL_SESSION_SCHEMA
        or canonical_pubkey(
            session.get("ownedLoopbackGenesisHash"), "terminal session genesis"
        )
        != session["ownedLoopbackGenesisHash"]
        or session.get("rpcUrl") != url
        or session.get("planSha256") != sha256_file(plan)
        or session.get("marketInputSha256") != sha256_file(market_input)
        or session.get("evidenceSha256") != sha256_file(evidence)
        or session.get("market") != market
        or session.get("payer") != payer
        or session.get("suppliedLookupTable") is not False
        or positive_integer(
            session.get("lookupRecentSlot"), "terminal lookup recent slot"
        )
        <= 0
        or initial_rent > receipt_rent
        or decoded != sorted(decoded)
        or len(set(decoded)) != len(decoded)
        or vector_digest.hexdigest()
        != lowercase_sha256(
            session.get("lookupAddressesSha256"), "terminal ALT union digest"
        )
        or semantic_owner_digest(
            session, "sessionSha256", "terminal sequence session"
        )
        != lowercase_sha256(
            session.get("sessionSha256"), "terminal session digest"
        )
    ):
        raise Refusal("terminal sequence handoff session identity changed")
    source_receipt = canonical_pubkey(
        session.get("sourceReceipt"), "terminal Source receipt"
    )
    lookup_table = canonical_pubkey(
        session.get("lookupTable"), "terminal lookup table"
    )
    history = terminal_sequence_finalized_history(journal_dir, url=url)
    if history[-1]["mutation"] != {"kind": "retirement-replay-handoff"}:
        raise Refusal("terminal replay handoff was not the finalized prefix terminal")
    return {
        "source_receipt": source_receipt,
        "lookup_table": lookup_table,
        "genesis_hash": session["ownedLoopbackGenesisHash"],
        "session_sha256": session["sessionSha256"],
        "journal": str(handoff_path),
        "journal_sha256": sha256_file(handoff_path),
        "finalized": history[-1]["finalized"],
        "transactions": [
            {"mutation": row["mutation"]["kind"], **row["finalized"]}
            for row in history
        ],
    }


def authenticate_terminal_campaign(
    document: Any,
    *,
    url: str,
    plan: Path,
    evidence: Path,
    market: str,
    payer: str,
    source_receipt: str,
    lookup_table: str,
    genesis_hash: str,
) -> dict[str, Any]:
    campaign = exact_keys(
        document,
        {
            "schema",
            "cluster",
            "genesisHash",
            "rpcUrl",
            "planSha256",
            "evidenceSha256",
            "payer",
            "lookupTable",
            "lookupTableSha256",
            "coreProgram",
            "claimsProgram",
            "market",
            "rentCredit",
            "checkpoint",
            "custodyReplay",
            "hoardVault",
            "sourceReceipt",
            "refundWallet",
            "classifiedLamports",
            "operations",
            "campaignSha256",
        },
        "AggregateRetirement campaign",
    )
    operations = campaign.get("operations")
    if (
        campaign.get("schema") != TERMINAL_CAMPAIGN_SCHEMA
        or campaign.get("cluster") != "owned-loopback"
        or campaign.get("genesisHash") != genesis_hash
        or campaign.get("rpcUrl") != url
        or campaign.get("planSha256") != sha256_file(plan)
        or campaign.get("evidenceSha256") != sha256_file(evidence)
        or campaign.get("payer") != payer
        or campaign.get("lookupTable") != lookup_table
        or not isinstance(operations, list)
        or [row.get("operation") for row in operations if isinstance(row, dict)]
        != list(TERMINAL_AGGREGATE_OPERATIONS)
        or semantic_owner_digest(
            campaign, "campaignSha256", "AggregateRetirement campaign"
        )
        != lowercase_sha256(
            campaign.get("campaignSha256"), "AggregateRetirement campaign digest"
        )
    ):
        raise Refusal("AggregateRetirement campaign changed its exact invocation")
    lowercase_sha256(campaign.get("lookupTableSha256"), "retirement ALT digest")
    for label in ("coreProgram", "claimsProgram"):
        canonical_pubkey(campaign.get(label), f"retirement {label}")
    account_fields = {
        "address",
        "owner",
        "lamports",
        "executable",
        "dataLen",
        "dataSha256",
        "accountSha256",
    }
    for label in (
        "market",
        "rentCredit",
        "checkpoint",
        "custodyReplay",
        "hoardVault",
        "sourceReceipt",
        "refundWallet",
    ):
        account = exact_keys(campaign.get(label), account_fields, f"retirement {label}")
        canonical_pubkey(account.get("address"), f"retirement {label} address")
        canonical_pubkey(account.get("owner"), f"retirement {label} owner")
        nonnegative_integer(account.get("lamports"), f"retirement {label} lamports")
        nonnegative_integer(account.get("dataLen"), f"retirement {label} data length")
        if not isinstance(account.get("executable"), bool):
            raise Refusal(f"retirement {label} executable flag changed type")
        lowercase_sha256(account.get("dataSha256"), f"retirement {label} data")
        if semantic_owner_digest(account, "accountSha256", f"retirement {label}") != lowercase_sha256(
            account.get("accountSha256"), f"retirement {label} account"
        ):
            raise Refusal(f"retirement {label} account digest changed")
    if (
        campaign["market"]["address"] != market
        or campaign["sourceReceipt"]["address"] != source_receipt
    ):
        raise Refusal("AggregateRetirement campaign changed Market or Source receipt")
    classified = exact_keys(
        campaign.get("classifiedLamports"),
        {
            "market",
            "rentCredit",
            "claimsRefund",
            "custodyReplay",
            "hoardVault",
            "expectedRefundDelta",
            "refundWalletBefore",
        },
        "AggregateRetirement classified lamports",
    )
    for label, value in classified.items():
        nonnegative_integer(value, f"retirement classified {label}")
    if classified["expectedRefundDelta"] != sum(
        classified[label]
        for label in (
            "market",
            "rentCredit",
            "claimsRefund",
            "custodyReplay",
            "hoardVault",
        )
    ):
        raise Refusal("AggregateRetirement classified refund sum changed")
    for index, operation in enumerate(operations):
        row = exact_keys(
            operation,
            {
                "operation",
                "programId",
                "accounts",
                "dataBase64",
                "dataSha256",
                "expectedWireBytes",
                "exactProtocolAndPayerKeys",
            },
            f"AggregateRetirement operation {index}",
        )
        if (
            row.get("programId") != campaign.get("coreProgram")
            or not isinstance(row.get("accounts"), list)
            or len(row["accounts"]) != 35
            or positive_integer(
                row.get("expectedWireBytes"), "retirement expected wire bytes"
            )
            <= 0
            or row.get("exactProtocolAndPayerKeys") != 36
        ):
            raise Refusal("AggregateRetirement operation geometry changed")
        try:
            data = base64.b64decode(row.get("dataBase64"), validate=True)
        except (TypeError, ValueError) as error:
            raise Refusal("AggregateRetirement operation data was not base64") from error
        if sha256_bytes(data) != lowercase_sha256(
            row.get("dataSha256"), "retirement instruction data"
        ):
            raise Refusal("AggregateRetirement operation data digest changed")
    return campaign


def authenticate_terminal_completion(
    document: Any,
    *,
    campaign: dict[str, Any],
    journal_dir: Path,
    market: str,
    payer: str,
) -> dict[str, Any]:
    completion = exact_keys(
        document,
        {
            "schema",
            "status",
            "campaignSha256",
            "market",
            "checkpoint",
            "rentCredit",
            "refundWallet",
            "payer",
            "classifiedLamports",
            "totalTransactionFeesLamports",
            "terminalRefundWalletLamports",
            "journals",
            "receiptSha256",
        },
        "AggregateRetirement completion",
    )
    journals = completion.get("journals")
    if (
        completion.get("schema") != TERMINAL_COMPLETION_SCHEMA
        or completion.get("status") != "finalized"
        or completion.get("campaignSha256") != campaign.get("campaignSha256")
        or completion.get("market") != market
        or completion.get("payer") != payer
        or completion.get("checkpoint") != campaign["checkpoint"]["address"]
        or completion.get("rentCredit") != campaign["rentCredit"]["address"]
        or completion.get("refundWallet") != campaign["refundWallet"]["address"]
        or completion.get("classifiedLamports") != campaign.get("classifiedLamports")
        or not isinstance(journals, list)
        or [row.get("operation") for row in journals if isinstance(row, dict)]
        != list(TERMINAL_AGGREGATE_OPERATIONS)
    ):
        raise Refusal("AggregateRetirement completion changed its exact campaign join")
    fees = 0
    facts: list[dict[str, Any]] = []
    signatures: set[str] = set()
    predecessor = (
        "ready",
        "claims-closed",
        "hoard-vault-closed",
        "custody-replay-closed",
    )
    successor = (
        "claims-closed",
        "hoard-vault-closed",
        "custody-replay-closed",
        "complete",
    )
    for index, operation in enumerate(TERMINAL_AGGREGATE_OPERATIONS):
        compact = exact_keys(
            journals[index],
            {
                "operation",
                "journalSha256",
                "signature",
                "finalizedSlot",
                "feeLamports",
                "computeUnitsConsumed",
                "packetSha256",
                "poststateSha256",
            },
            f"AggregateRetirement completion journal {index}",
        )
        path = canonical_file(
            journal_dir / f"{index:02d}-{operation}.json",
            f"AggregateRetirement journal {index}",
        )
        journal = exact_keys(
            read_unique_json(path, f"AggregateRetirement journal {index}"),
            {
                "schema",
                "campaignSha256",
                "operation",
                "phase",
                "predecessor",
                "successor",
                "plannedPrestateSha256",
                "intentSha256",
                "packet",
                "finalization",
                "stateSha256",
            },
            f"AggregateRetirement journal {index}",
        )
        finalization = exact_keys(
            journal.get("finalization"),
            {
                "signature",
                "finalizedSlot",
                "packetSha256",
                "feeLamports",
                "computeUnitsConsumed",
                "poststateSha256",
                "checkpointHistorySha256",
            },
            f"AggregateRetirement journal {index} finalization",
        )
        fact = finalized_fact(
            compact,
            f"AggregateRetirement journal {index}",
            slot_key="finalizedSlot",
        )
        if (
            journal.get("schema") != TERMINAL_AGGREGATE_JOURNAL_SCHEMA
            or journal.get("campaignSha256") != campaign.get("campaignSha256")
            or journal.get("operation") != operation
            or journal.get("phase") != "finalized"
            or journal.get("predecessor") != predecessor[index]
            or journal.get("successor") != successor[index]
            or not isinstance(journal.get("packet"), dict)
            or finalization.get("signature") != compact.get("signature")
            or finalization.get("finalizedSlot") != compact.get("finalizedSlot")
            or finalization.get("feeLamports") != compact.get("feeLamports")
            or finalization.get("computeUnitsConsumed")
            != compact.get("computeUnitsConsumed")
            or finalization.get("packetSha256") != compact.get("packetSha256")
            or finalization.get("poststateSha256") != compact.get("poststateSha256")
            or semantic_owner_digest(
                journal, "stateSha256", f"AggregateRetirement journal {index}"
            )
            != lowercase_sha256(
                compact.get("journalSha256"),
                f"AggregateRetirement journal {index} state",
            )
            or journal.get("stateSha256") != compact.get("journalSha256")
        ):
            raise Refusal("AggregateRetirement compact journal projection changed")
        for field in (
            "plannedPrestateSha256",
            "intentSha256",
            "stateSha256",
        ):
            lowercase_sha256(journal.get(field), f"retirement journal {field}")
        for field in ("packetSha256", "poststateSha256"):
            lowercase_sha256(compact.get(field), f"retirement compact {field}")
        if fact["signature"] in signatures:
            raise Refusal("AggregateRetirement repeated one transaction signature")
        signatures.add(fact["signature"])
        facts.append(fact)
        fees += fact["fee_lamports"]
    if completion.get("totalTransactionFeesLamports") != fees:
        raise Refusal("AggregateRetirement total fee projection changed")
    terminal_lamports = nonnegative_integer(
        completion.get("terminalRefundWalletLamports"),
        "AggregateRetirement terminal refund wallet",
    )
    classified = campaign["classifiedLamports"]
    expected_terminal = (
        classified["refundWalletBefore"] + classified["expectedRefundDelta"]
    )
    if payer == campaign["refundWallet"]["address"]:
        expected_terminal -= fees
    if expected_terminal < 0 or terminal_lamports != expected_terminal:
        raise Refusal("AggregateRetirement refund/fee conservation changed")
    if semantic_owner_digest(
        completion, "receiptSha256", "AggregateRetirement completion"
    ) != lowercase_sha256(
        completion.get("receiptSha256"), "AggregateRetirement completion digest"
    ):
        raise Refusal("AggregateRetirement completion digest changed")
    return {"completion": completion, "facts": facts}


def authenticate_terminal_stdout(
    document: Any,
    *,
    campaign: dict[str, Any],
    campaign_path: Path,
    completion_path: Path,
    journal_dir: Path,
) -> dict[str, Any]:
    summary = exact_keys(
        document,
        {
            "schema",
            "status",
            "campaign",
            "campaignSha256",
            "journalDirectory",
            "completion",
            "completionSha256",
            "message",
        },
        "terminal retirement stdout",
    )
    if (
        summary.get("schema") != TERMINAL_PROGRESS_SCHEMA
        or summary.get("status") != "finalized"
        or summary.get("campaign") != str(campaign_path)
        or summary.get("campaignSha256") != campaign.get("campaignSha256")
        or summary.get("journalDirectory") != str(journal_dir)
        or summary.get("completion") != str(completion_path)
        or summary.get("completionSha256") != sha256_file(completion_path)
        or summary.get("message")
        != "Aggregate retirement finalized through prepare, close-vault, close-replay, and finish; exact rent/refund conservation reverified."
    ):
        raise Refusal(
            "AggregateRetirement stdout changed its exact completion path, hash, or semantic summary"
        )
    return summary


def run_flagship_resolution(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    campaign_evidence: Path,
    pyth_facts: Path,
    first_ordinal: int,
) -> tuple[dict[str, Any], int]:
    """Drive only the accepted Resolution semantic owners through Complete."""

    root = run / "resolution"
    root.mkdir(mode=0o700)
    producer_path = root / "producer.json"
    table_path = root / "tables.json"
    input_path = root / "input.json"
    checkpoint_path = root / "checkpoint.json"
    for path in (producer_path, table_path, input_path, checkpoint_path):
        if path.exists():
            raise Refusal(
                "fresh Resolution controller root contains a preexisting artifact"
            )
    keys = keypairs_by_address(paths, report)
    ordinal = first_ordinal

    def produce() -> dict[str, Any]:
        nonlocal ordinal
        document = run_json_stage(
            run,
            ordinal,
            f"resolution-produce-{ordinal - first_ordinal:02d}",
            [
                str(paths.bootstrap),
                FLAGSHIP_RESOLUTION_COMMAND,
                "--produce-input",
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--campaign-evidence",
                str(campaign_evidence),
                "--pyth-facts",
                str(pyth_facts),
                "--producer-checkpoint",
                str(producer_path),
                "--output",
                str(input_path),
            ],
        )
        ordinal += 1
        if read_unique_json(producer_path, "persisted Resolution producer") != document:
            raise Refusal(
                "Resolution producer stdout differs from its persisted checkpoint"
            )
        return document

    producer = produce()
    for _attempt in range(MAX_RESOLUTION_TABLE_INVOCATIONS):
        authenticated = authenticate_resolution_producer(
            producer,
            require_complete=False,
            plan=plan,
            campaign_evidence=campaign_evidence,
            pyth_facts=pyth_facts,
        )
        if authenticated.get("flagshipInput") is not None:
            authenticate_resolution_producer(
                authenticated,
                require_complete=True,
                plan=plan,
                campaign_evidence=campaign_evidence,
                pyth_facts=pyth_facts,
            )
            if (
                read_unique_json(input_path, "Resolution input")
                != authenticated["plannedInput"]
            ):
                raise Refusal(
                    "Resolution input differs from the completed producer checkpoint"
                )
            break
        authority = authenticated["authority"]
        document = run_json_stage(
            run,
            ordinal,
            f"resolution-table-{ordinal - first_ordinal:02d}",
            [
                str(paths.bootstrap),
                FLAGSHIP_RESOLUTION_COMMAND,
                "--provision-tables",
                "--rpc-url",
                url,
                "--producer-checkpoint",
                str(producer_path),
                "--table-journal",
                str(table_path),
                "--authority-keypair",
                str(require_keypair(keys, authority, "Resolution table authority")),
                "--execute",
            ],
        )
        ordinal += 1
        if (
            read_unique_json(table_path, "persisted Resolution table journal")
            != document
        ):
            raise Refusal("Resolution table stdout differs from its persisted journal")
        authenticate_resolution_table_journal(
            document, require_complete=False, producer=authenticated
        )
        producer = produce()
    else:
        raise Refusal(
            "Resolution tables exceeded the bounded 64-invocation controller loop"
        )

    table = read_unique_json(table_path, "completed Resolution table journal")
    table_facts = authenticate_resolution_table_journal(
        table, require_complete=True, producer=producer
    )
    planned = producer["plannedInput"]
    submitter = canonical_pubkey(planned.get("submitter"), "Resolution submitter")
    resolver = canonical_pubkey(planned.get("resolver"), "Resolution resolver")
    update_account = canonical_pubkey(
        planned.get("accounts", {}).get("updateAccount"), "Resolution update account"
    )
    execute_argv = [
        str(paths.bootstrap),
        FLAGSHIP_RESOLUTION_COMMAND,
        "--rpc-url",
        url,
        "--input",
        str(input_path),
        "--checkpoint",
        str(checkpoint_path),
        "--through",
        "complete",
        "--submitter-keypair",
        str(require_keypair(keys, submitter, "Resolution submitter")),
        "--resolver-keypair",
        str(require_keypair(keys, resolver, "Resolution resolver")),
        "--update-keypair",
        str(require_keypair(keys, update_account, "Resolution update signer")),
        "--execute",
    ]
    checkpoint: dict[str, Any] | None = None
    for attempt in range(MAX_RESOLUTION_STAGE_INVOCATIONS):
        checkpoint = run_json_stage(
            run,
            ordinal,
            f"resolution-execute-{attempt:02d}",
            execute_argv,
        )
        ordinal += 1
        if (
            read_unique_json(checkpoint_path, "persisted Resolution checkpoint")
            != checkpoint
        ):
            raise Refusal(
                "Resolution executor stdout differs from its persisted checkpoint"
            )
        if checkpoint.get("verifiedTerminal") is True:
            stage_facts = authenticate_resolution_checkpoint(
                checkpoint, input_path=input_path
            )
            return {
                "schema": "dclutch-private-validator-resolution-controller-v1",
                "status": "finalized",
                "producer": str(producer_path),
                "producer_sha256": sha256_file(producer_path),
                "table_journal": str(table_path),
                "table_journal_sha256": sha256_file(table_path),
                "input": str(input_path),
                "input_sha256": sha256_file(input_path),
                "checkpoint": str(checkpoint_path),
                "checkpoint_sha256": sha256_file(checkpoint_path),
                "market": producer["market"],
                "table_transactions": table_facts,
                "stage_transactions": stage_facts,
            }, ordinal
    raise Refusal("Resolution exceeded the bounded 16-invocation stage controller loop")


def run_wallet_payouts(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    campaign_evidence: Path,
    market: str,
    schedule: Sequence[PayoutTarget],
    first_ordinal: int,
) -> tuple[list[dict[str, Any]], int]:
    """Produce and execute exactly one semantic-owner payout per nonzero claim."""

    targets = canonical_payout_schedule(schedule)
    canonical_pubkey(market, "payout Market")
    root = run / "payouts"
    root.mkdir(mode=0o700)
    keys = keypairs_by_address(paths, report)
    payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    payer = canonical_pubkey(key_address(paths.solana, payer_key), "payout fee payer")
    ordinal = first_ordinal
    rows: list[dict[str, Any]] = []
    for index, target in enumerate(targets):
        payout_root = root / f"{index:03d}"
        payout_root.mkdir(mode=0o700)
        input_path = payout_root / "input.json"
        evidence_path = payout_root / "evidence.json"
        journal_dir = payout_root / "journals"
        journal_dir.mkdir(mode=0o700)
        input_result = run_stage(
            run,
            ordinal,
            f"payout-input-{index:03d}",
            [
                str(paths.bootstrap),
                PAYOUT_INPUT_COMMAND,
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--evidence",
                str(campaign_evidence),
                "--market",
                market,
                "--owner",
                target.owner,
                "--recipient",
                target.recipient,
                "--claim-index",
                str(target.claim_index),
            ],
        )
        ordinal += 1
        try:
            input_document = decode_unique_json(
                input_result.stdout.decode("utf-8"), f"payout input {index}"
            )
        except UnicodeDecodeError as error:
            raise Refusal(f"payout input {index} was not UTF-8") from error
        authenticate_payout_input(input_document, target, market)
        write_bytes_new(input_path, input_result.stdout)
        execute_argv = [
            str(paths.bootstrap),
            PAYOUT_EXECUTE_COMMAND,
            "--rpc-url",
            url,
            "--input",
            str(input_path),
            "--fee-payer",
            payer,
            "--fee-payer-keypair",
            str(payer_key),
            "--owner-keypair",
            str(require_keypair(keys, target.owner, "payout owner")),
            "--journal-dir",
            str(journal_dir),
            "--evidence",
            str(evidence_path),
            "--execute",
        ]
        for attempt in range(MAX_PAYOUT_INVOCATIONS):
            stdout = run_json_stage(
                run,
                ordinal,
                f"payout-{index:03d}-{attempt:02d}",
                execute_argv,
            )
            ordinal += 1
            if evidence_path.is_file():
                persisted = read_unique_json(evidence_path, f"payout evidence {index}")
                if stdout != persisted:
                    # One extra semantic-owner call must reauthenticate and print exact evidence.
                    stdout = run_json_stage(
                        run,
                        ordinal,
                        f"payout-authenticate-{index:03d}",
                        execute_argv,
                    )
                    ordinal += 1
                if stdout != persisted:
                    raise Refusal(
                        "payout reauthentication differs from persisted evidence"
                    )
                evidence = authenticate_payout_evidence(
                    persisted, input_path, target, market
                )
                rows.append(
                    {
                        "target": dataclasses.asdict(target),
                        "input": str(input_path),
                        "input_sha256": sha256_file(input_path),
                        "evidence": str(evidence_path),
                        "evidence_sha256": sha256_file(evidence_path),
                        "finalized": finalized_fact(
                            evidence,
                            f"payout {index}",
                            slot_key="finalizedSlot",
                        ),
                    }
                )
                break
        else:
            raise Refusal(f"payout {index} exceeded the bounded 24-invocation loop")
    return rows, ordinal


def run_terminal_retirement(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    market_input: Path,
    campaign_evidence: Path,
    market: str,
    first_ordinal: int,
) -> tuple[dict[str, Any], int]:
    """Advance the terminal prelude, then the four-packet aggregate owner."""

    root = run / "retirement"
    root.mkdir(mode=0o700)
    sequence_journal_dir = root / "sequence-journals"
    sequence_journal_dir.mkdir(mode=0o700)
    session_path = root / "sequence-session.json"
    legacy_completion_path = root / "packet-inadmissible-monolith.json"
    aggregate_campaign_path = root / "aggregate-campaign.json"
    aggregate_journal_dir = root / "aggregate-journals"
    aggregate_journal_dir.mkdir(mode=0o700)
    completion_path = root / "completion.json"
    payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    payer = canonical_pubkey(
        key_address(paths.solana, payer_key), "retirement fee payer"
    )
    sequence_argv = [
        str(paths.bootstrap),
        TERMINAL_SEQUENCE_COMMAND,
        "--rpc-url",
        url,
        "--plan",
        str(plan),
        "--market-input",
        str(market_input),
        "--evidence",
        str(campaign_evidence),
        "--market",
        market,
        "--fee-payer",
        payer,
        "--fee-payer-keypair",
        str(payer_key),
        "--session",
        str(session_path),
        "--journal-dir",
        str(sequence_journal_dir),
        "--completion",
        str(legacy_completion_path),
        "--execute",
    ]
    ordinal = first_ordinal
    for attempt in range(MAX_TERMINAL_INVOCATIONS):
        handoff = terminal_sequence_handoff(
            session_path,
            sequence_journal_dir,
            url=url,
            plan=plan,
            market_input=market_input,
            evidence=campaign_evidence,
            market=market,
            payer=payer,
        )
        if handoff is not None:
            break
        run_json_stage(
            run,
            ordinal,
            f"retirement-sequence-{attempt:02d}",
            sequence_argv,
        )
        ordinal += 1
    else:
        raise Refusal("terminal sequence exceeded the bounded pre-aggregate loop")
    if legacy_completion_path.exists():
        raise Refusal("packet-inadmissible monolithic terminal completion appeared")
    aggregate_argv = [
        str(paths.bootstrap),
        TERMINAL_RETIREMENT_COMMAND,
        "--rpc-url",
        url,
        "--plan",
        str(plan),
        "--evidence",
        str(campaign_evidence),
        "--market",
        market,
        "--source-receipt",
        handoff["source_receipt"],
        "--fee-payer",
        payer,
        "--fee-payer-keypair",
        str(payer_key),
        "--lookup-table",
        handoff["lookup_table"],
        "--campaign",
        str(aggregate_campaign_path),
        "--journal-dir",
        str(aggregate_journal_dir),
        "--completion",
        str(completion_path),
        "--execute",
    ]
    for attempt in range(MAX_TERMINAL_INVOCATIONS):
        stdout = run_json_stage(
            run,
            ordinal,
            f"retirement-aggregate-{attempt:02d}",
            aggregate_argv,
        )
        ordinal += 1
        if completion_path.is_file():
            campaign = authenticate_terminal_campaign(
                read_unique_json(
                    aggregate_campaign_path, "AggregateRetirement campaign"
                ),
                url=url,
                plan=plan,
                evidence=campaign_evidence,
                market=market,
                payer=payer,
                source_receipt=handoff["source_receipt"],
                lookup_table=handoff["lookup_table"],
                genesis_hash=handoff["genesis_hash"],
            )
            completion = read_unique_json(
                completion_path, "AggregateRetirement completion"
            )
            authenticated = authenticate_terminal_completion(
                completion,
                campaign=campaign,
                journal_dir=aggregate_journal_dir,
                market=market,
                payer=payer,
            )
            authenticate_terminal_stdout(
                stdout,
                campaign=campaign,
                campaign_path=aggregate_campaign_path,
                completion_path=completion_path,
                journal_dir=aggregate_journal_dir,
            )
            transactions = [
                {
                    "mutation": operation,
                    **authenticated["facts"][index],
                }
                for index, operation in enumerate(TERMINAL_AGGREGATE_OPERATIONS)
            ]
            return {
                "schema": "dclutch-private-validator-retirement-controller-v1",
                "status": "finalized",
                "terminal_sequence_session": str(session_path),
                "terminal_sequence_session_sha256": sha256_file(session_path),
                "terminal_sequence_handoff": handoff,
                "campaign": str(aggregate_campaign_path),
                "campaign_sha256": sha256_file(aggregate_campaign_path),
                "journal_dir": str(aggregate_journal_dir),
                "completion": str(completion_path),
                "completion_sha256": sha256_file(completion_path),
                "transactions": [*handoff["transactions"], *transactions],
            }, ordinal
    raise Refusal("AggregateRetirement exceeded the bounded four-packet loop")


def run_post_direct_lifecycle(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    market_input: Path,
    campaign_evidence: Path,
    payout_schedule: Sequence[PayoutTarget],
    first_ordinal: int,
) -> tuple[dict[str, Any], int]:
    """Post-Direct seam; caller must come from the future frozen Direct adapter."""

    pyth_facts, pyth, ordinal = run_pyth_provisioning(
        run, paths, report, url, first_ordinal
    )
    resolution, ordinal = run_flagship_resolution(
        run,
        paths,
        report,
        url,
        plan,
        campaign_evidence,
        pyth_facts,
        ordinal,
    )
    payouts, ordinal = run_wallet_payouts(
        run,
        paths,
        report,
        url,
        plan,
        campaign_evidence,
        resolution["market"],
        payout_schedule,
        ordinal,
    )
    retirement, ordinal = run_terminal_retirement(
        run,
        paths,
        report,
        url,
        plan,
        market_input,
        campaign_evidence,
        resolution["market"],
        ordinal,
    )
    result = {
        "schema": "dclutch-private-validator-post-direct-lifecycle-v1",
        "status": "finalized",
        "pyth": pyth,
        "resolution": resolution,
        "payouts": payouts,
        "retirement": retirement,
    }
    result["compute_units"] = post_direct_compute_units(result)
    return result, ordinal


def post_direct_compute_units(result: dict[str, Any]) -> dict[str, int]:
    metrics: dict[str, int] = {}
    for action, value in result["pyth"]["compute_units"].items():
        metrics[f"pyth-{action}"] = positive_integer(value, f"Pyth {action} CU")
    for family in ("table_transactions", "stage_transactions"):
        for index, fact in enumerate(result["resolution"][family]):
            metrics[f"resolution-{family}-{index:02d}"] = positive_integer(
                fact.get("compute_units_consumed"), f"Resolution {family} CU"
            )
    for index, payout in enumerate(result["payouts"]):
        metrics[f"payout-{index:03d}"] = positive_integer(
            payout["finalized"].get("compute_units_consumed"), f"payout {index} CU"
        )
    for index, transaction in enumerate(result["retirement"]["transactions"]):
        metrics[f"retirement-{index:02d}-{transaction['mutation']}"] = positive_integer(
            transaction.get("compute_units_consumed"), f"retirement {index} CU"
        )
    return metrics


def authenticate_direct_producer_receipt(
    document: Any,
    root: Path,
    plan: Path,
    market_input: Path,
    campaign_report: Path,
    participant_report: Path,
) -> tuple[Path, Path, str]:
    receipt = exact_keys(
        document,
        {
            "schema",
            "status",
            "producerReceipt",
            "publicManifest",
            "publicManifestSha256",
            "privateSession",
            "privateSessionSha256",
            "planSha256",
            "marketInputSha256",
            "campaignReportSha256",
            "participantReportSha256",
            "participantAdmissionSignature",
            "participantAdmissionSlot",
            "participantCollateralSignature",
            "participantCollateralSlot",
            "replaySetup",
            "tokenSetup",
            "receiptSha256",
        },
        "Direct producer receipt",
    )
    expected_receipt = root / "direct-trade-produced.json"
    expected_public = root / "direct-trade-public.json"
    expected_session = root / "direct-trade-session.json"
    receipt_path = canonical_file(receipt.get("producerReceipt"), "Direct producer receipt")
    public_path = canonical_file(receipt.get("publicManifest"), "Direct public manifest")
    session_path = canonical_file(receipt.get("privateSession"), "Direct private session")
    if (
        receipt.get("schema") != DIRECT_PRODUCER_SCHEMA
        or receipt.get("status") != "produced"
        or receipt_path != expected_receipt
        or public_path != expected_public
        or session_path != expected_session
        or sha256_file(public_path)
        != lowercase_sha256(receipt.get("publicManifestSha256"), "Direct public manifest digest")
        or sha256_file(session_path)
        != lowercase_sha256(receipt.get("privateSessionSha256"), "Direct private session digest")
        or sha256_file(plan)
        != lowercase_sha256(receipt.get("planSha256"), "Direct plan digest")
        or sha256_file(market_input)
        != lowercase_sha256(receipt.get("marketInputSha256"), "Direct Market input digest")
        or sha256_file(campaign_report)
        != lowercase_sha256(receipt.get("campaignReportSha256"), "Direct campaign digest")
        or sha256_file(participant_report)
        != lowercase_sha256(
            receipt.get("participantReportSha256"), "Direct participant digest"
        )
        or not isinstance(receipt.get("replaySetup"), dict)
        or not isinstance(receipt.get("tokenSetup"), dict)
    ):
        raise Refusal("Direct producer receipt changed its exact authenticated inputs")
    canonical_signature(
        receipt.get("participantAdmissionSignature"),
        "Direct participant admission signature",
    )
    canonical_signature(
        receipt.get("participantCollateralSignature"),
        "Direct participant collateral signature",
    )
    positive_integer(
        receipt.get("participantAdmissionSlot"), "Direct participant admission slot"
    )
    positive_integer(
        receipt.get("participantCollateralSlot"), "Direct participant collateral slot"
    )
    lowercase_sha256(receipt.get("receiptSha256"), "Direct producer receipt digest")
    public = read_unique_json(public_path, "Direct public manifest")
    if (
        not isinstance(public, dict)
        or public.get("schema")
        != "dclutch-owned-loopback-direct-trade-public-manifest-v1"
        or public.get("cluster") != "owned-loopback"
    ):
        raise Refusal("Direct public manifest changed its owned-loopback schema")
    market = canonical_pubkey(public.get("market"), "Direct Market")
    return session_path, root / "direct-trade-finalized.json", market


def accepted_direct_payout_schedule(
    schedule_path: Path, direct_evidence: Path
) -> tuple[tuple[PayoutTarget, ...], dict[str, int], dict[str, Any]]:
    direct_evidence = canonical_file(direct_evidence, "Direct finalized evidence")
    schedule = exact_keys(
        read_unique_json(schedule_path, "Direct payout schedule"),
        {
            "schema",
            "status",
            "cluster",
            "directEvidence",
            "market",
            "planSha256",
            "marketInputSha256",
            "finalizedSlot",
            "mutations",
            "claims",
            "scheduleSetSha256",
        },
        "Direct payout schedule",
    )
    direct = exact_keys(
        schedule.get("directEvidence"),
        {"path", "sha256", "schema", "evidenceSha256"},
        "Direct finalized evidence reference",
    )
    finalized_slot = canonical_decimal(
        schedule.get("finalizedSlot"), "Direct finalized slot"
    )
    if (
        schedule.get("schema") != DIRECT_PAYOUT_SCHEDULE_SCHEMA
        or schedule.get("status") != "finalized"
        or schedule.get("cluster") != "owned-loopback"
        or canonical_file(direct.get("path"), "Direct finalized evidence")
        != direct_evidence
        or direct.get("schema") != DIRECT_FINALIZED_SCHEMA
        or sha256_file(direct_evidence)
        != lowercase_sha256(direct.get("sha256"), "Direct finalized file digest")
    ):
        raise Refusal("Direct payout schedule names another terminal semantic owner")
    lowercase_sha256(direct.get("evidenceSha256"), "Direct semantic evidence digest")
    canonical_pubkey(schedule.get("market"), "Direct payout Market")
    lowercase_sha256(schedule.get("planSha256"), "Direct payout plan digest")
    lowercase_sha256(
        schedule.get("marketInputSha256"), "Direct payout Market input digest"
    )

    mutations = schedule.get("mutations")
    if not isinstance(mutations, list) or not 7 <= len(mutations) <= MAX_DIRECT_INVOCATIONS:
        raise Refusal("Direct terminal mutation sequence is absent or unbounded")
    vocabulary = {
        "replay-setup": 0,
        "token-setup": 1,
        "lookup-create": 2,
        "lookup-extend": 3,
        "lookup-freeze": 4,
        "capability-seal": 5,
        "hot": 6,
    }
    ranks: list[int] = []
    signatures: set[str] = set()
    journals: set[Path] = set()
    compute_units: dict[str, int] = {}
    kind_counts: dict[str, int] = {}
    for index, raw in enumerate(mutations):
        row = exact_keys(
            raw,
            {
                "kind",
                "prefixLen",
                "path",
                "sha256",
                "intentSha256",
                "schema",
                "completionPointer",
                "completionValue",
                "signature",
                "slot",
                "feePayer",
                "feeLamports",
                "computeUnitsConsumed",
            },
            f"Direct mutation {index}",
        )
        kind = row.get("kind")
        if kind not in vocabulary:
            raise Refusal("Direct terminal mutation kind changed")
        rank = vocabulary[kind]
        ranks.append(rank)
        prefix = row.get("prefixLen")
        if kind == "lookup-extend":
            canonical_decimal(prefix, f"Direct mutation {index} prefix length")
        elif prefix is not None:
            raise Refusal("only a Direct lookup extension may carry prefixLen")
        journal = canonical_file(row.get("path"), f"Direct mutation {index} journal")
        signature = canonical_signature(
            row.get("signature"), f"Direct mutation {index} signature"
        )
        slot = canonical_decimal(row.get("slot"), f"Direct mutation {index} slot")
        canonical_decimal(
            row.get("feeLamports"),
            f"Direct mutation {index} fee",
            positive=False,
        )
        units = canonical_decimal(
            row.get("computeUnitsConsumed"), f"Direct mutation {index} CU"
        )
        if (
            journal in journals
            or signature in signatures
            or sha256_file(journal)
            != lowercase_sha256(row.get("sha256"), f"Direct mutation {index} digest")
            or row.get("completionPointer") != "/phase"
            or row.get("completionValue") != "finalized"
            or slot > finalized_slot
        ):
            raise Refusal("Direct mutation journal closure changed or repeated")
        lowercase_sha256(
            row.get("intentSha256"), f"Direct mutation {index} intent digest"
        )
        if not isinstance(row.get("schema"), str) or not row["schema"]:
            raise Refusal("Direct mutation journal schema is absent")
        canonical_pubkey(row.get("feePayer"), f"Direct mutation {index} fee payer")
        journals.add(journal)
        signatures.add(signature)
        occurrence = kind_counts.get(kind, 0)
        kind_counts[kind] = occurrence + 1
        compute_units[f"direct-{index:02d}-{kind}-{occurrence:02d}"] = units
    if (
        ranks != sorted(ranks)
        or ranks[0] != 0
        or ranks[-1] != 6
        or any(kind_counts.get(kind) != 1 for kind in vocabulary if kind != "lookup-extend")
        or kind_counts.get("lookup-extend", 0) < 1
    ):
        raise Refusal("Direct mutation sequence is not replay through Hot exactly once")

    claims = schedule.get("claims")
    if not isinstance(claims, list):
        raise Refusal("Direct payout schedule claims are not one array")
    canonical_rows: list[PayoutTarget] = []
    for index, raw in enumerate(claims):
        row = exact_keys(
            raw,
            {"owner", "position", "recipientToken", "claimIndex", "quantityAtoms"},
            f"Direct payout claim {index}",
        )
        owner = canonical_pubkey(row.get("owner"), f"Direct payout owner {index}")
        canonical_pubkey(row.get("position"), f"Direct payout Position {index}")
        recipient = canonical_pubkey(
            row.get("recipientToken"), f"Direct payout recipient {index}"
        )
        claim_index = canonical_decimal(
            row.get("claimIndex"), f"Direct payout claim index {index}", positive=False
        )
        if claim_index > 0xFFFFFFFF:
            raise Refusal("Direct payout claim index exceeds u32")
        quantity_atoms = canonical_decimal(
            row.get("quantityAtoms"), f"Direct payout quantity {index}"
        )
        if quantity_atoms > 0xFFFFFFFFFFFFFFFF:
            raise Refusal("Direct payout quantity exceeds u64")
        canonical_rows.append(
            PayoutTarget(owner, claim_index, recipient, quantity_atoms)
        )
    encoded_claims = (
        json.dumps(claims, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()
    if sha256_bytes(encoded_claims) != lowercase_sha256(
        schedule.get("scheduleSetSha256"), "Direct payout schedule-set digest"
    ):
        raise Refusal("Direct payout schedule-set digest changed")
    targets = canonical_payout_schedule(tuple(canonical_rows))
    return targets, compute_units, schedule


def run_direct_lifecycle(
    run: Path,
    paths: Paths,
    url: str,
    plan: Path,
    market_input: Path,
    campaign_report: Path,
    participant_report: Path,
    key_directory: Path,
    first_ordinal: int,
) -> tuple[dict[str, Any], tuple[PayoutTarget, ...], int]:
    root = run / "direct"
    root.mkdir(mode=0o700)
    produced = run_json_stage(
        run,
        first_ordinal,
        "direct-produce",
        [
            str(paths.bootstrap),
            DIRECT_PRODUCER_COMMAND,
            "--rpc-url",
            url,
            "--plan",
            str(plan),
            "--market-input",
            str(market_input),
            "--campaign-report",
            str(campaign_report),
            "--participant-report",
            str(participant_report),
            "--key-dir",
            str(canonical_directory(key_directory, "Direct key directory")),
            "--output-dir",
            str(root),
        ],
    )
    receipt_path = root / "direct-trade-produced.json"
    if read_unique_json(receipt_path, "persisted Direct producer receipt") != produced:
        raise Refusal("Direct producer stdout differs from its durable receipt")
    session_path, evidence_path, market = authenticate_direct_producer_receipt(
        produced,
        root,
        plan,
        market_input,
        campaign_report,
        participant_report,
    )
    ordinal = first_ordinal + 1
    for attempt in range(MAX_DIRECT_INVOCATIONS):
        document = run_json_stage(
            run,
            ordinal,
            f"direct-execute-{attempt:02d}",
            [
                str(paths.bootstrap),
                DIRECT_EXECUTE_COMMAND,
                "--rpc-url",
                url,
                "--session",
                str(session_path),
                "--execute",
            ],
        )
        ordinal += 1
        if evidence_path.is_file():
            if read_unique_json(evidence_path, "persisted Direct finalized evidence") != document:
                raise Refusal("Direct executor stdout differs from its terminal evidence")
            break
    else:
        raise Refusal("Direct executor exceeded its bounded 32-mutation loop")

    schedule_path = root / "direct-payout-schedule.json"
    projected = run_json_stage(
        run,
        ordinal,
        "direct-payout-schedule",
        [
            str(paths.bootstrap),
            DIRECT_PAYOUT_SCHEDULE_COMMAND,
            "--rpc-url",
            url,
            "--plan",
            str(plan),
            "--market-input",
            str(market_input),
            "--market",
            market,
            "--direct-evidence",
            str(evidence_path),
            "--output",
            str(schedule_path),
        ],
    )
    ordinal += 1
    if read_unique_json(schedule_path, "persisted Direct payout schedule") != projected:
        raise Refusal("Direct payout schedule stdout differs from its durable receipt")
    targets, compute_units, authenticated_schedule = accepted_direct_payout_schedule(
        schedule_path, evidence_path
    )
    result = {
        "schema": "dclutch-private-validator-direct-controller-v1",
        "status": "finalized",
        "producer_receipt": str(receipt_path),
        "producer_receipt_sha256": sha256_file(receipt_path),
        "private_session": str(session_path),
        "private_session_sha256": sha256_file(session_path),
        "finalized_evidence": str(evidence_path),
        "finalized_evidence_sha256": sha256_file(evidence_path),
        "payout_schedule": str(schedule_path),
        "payout_schedule_sha256": sha256_file(schedule_path),
        "market": authenticated_schedule["market"],
        "compute_units": compute_units,
    }
    return result, targets, ordinal


def checked_mutable_slot_floor(plan_path: Path) -> int:
    plan = read_unique_json(plan_path, "checked local mutable plan")
    checked_set = plan.get("checked_local_mutable_set")
    roles = checked_set.get("roles") if isinstance(checked_set, dict) else None
    if not isinstance(roles, list) or len(roles) != len(ROLE_ORDER):
        raise Refusal(
            "checked local mutable plan omitted its exact seven-role slot projection"
        )
    slots = [role.get("deployment_slot") for role in roles if isinstance(role, dict)]
    if len(slots) != len(ROLE_ORDER) or any(
        not isinstance(slot, int) or slot < 0 for slot in slots
    ):
        raise Refusal("checked local mutable plan carried an invalid deployment slot")
    return max(slots)


def named_seed(index: int) -> tuple[str, str]:
    name = f"seed-{index:02d}"
    return name, hashlib.sha256(SEED_DOMAIN + name.encode()).hexdigest()


def key_flags(
    report: dict[str, Any],
    projection: str = "campaign_founding_keypairs",
) -> list[str]:
    if projection not in (
        "campaign_administration_keypairs",
        "campaign_founding_keypairs",
    ):
        raise Refusal("campaign keypair projection is not one frozen mode")
    flags: list[str] = []
    campaign_keypairs = report.get(projection)
    expected_roles = (
        CAMPAIGN_ADMINISTRATION_KEY_ROLES
        if projection == "campaign_administration_keypairs"
        else CAMPAIGN_FOUNDING_KEY_ROLES
    )
    if not isinstance(campaign_keypairs, dict) or set(campaign_keypairs) != set(
        expected_roles
    ):
        raise Refusal(
            f"local mutable preparation changed its exact Rust-owned {projection} projection"
        )
    for role, path in sorted(campaign_keypairs.items()):
        if not isinstance(role, str) or not role or not isinstance(path, str) or not path:
            raise Refusal("Rust-owned campaign keypair projection changed shape")
        flags.extend((f"--keypair-{role}", path))
    return flags


def campaign_public_identities(report: dict[str, Any]) -> dict[str, str]:
    identities = report.get("campaign_public_identities")
    if not isinstance(identities, dict) or set(identities) != {
        "founding-founder",
        "substituted-founder",
    }:
        raise Refusal(
            "local mutable preparation omitted its exact two public founding identities"
        )
    founder = canonical_pubkey(identities["founding-founder"], "founding founder")
    substituted = canonical_pubkey(
        identities["substituted-founder"], "substituted founder"
    )
    if founder == substituted:
        raise Refusal("public founding identities alias")
    return {
        "founding-founder": founder,
        "substituted-founder": substituted,
    }


def administration_campaign_argv(
    bootstrap: Path,
    url: str,
    plan: Path,
    evidence: Path,
    report: dict[str, Any],
) -> list[str]:
    return [
        str(bootstrap),
        "campaign",
        "--rpc-url",
        url,
        "--plan",
        str(plan),
        "--evidence",
        str(evidence),
        "--through",
        "activation",
        "--execute",
        *key_flags(report, "campaign_administration_keypairs"),
    ]


def founding_campaign_argv(
    bootstrap: Path,
    url: str,
    plan: Path,
    market: Path,
    evidence: Path,
    report: dict[str, Any],
) -> list[str]:
    identities = campaign_public_identities(report)
    return [
        str(bootstrap),
        "campaign",
        "--founding-only",
        "--rpc-url",
        url,
        "--plan",
        str(plan),
        "--market",
        str(market),
        "--evidence",
        str(evidence),
        "--through",
        "founding",
        "--founding-founder",
        identities["founding-founder"],
        "--substituted-founder",
        identities["substituted-founder"],
        "--execute",
        *key_flags(report, "campaign_founding_keypairs"),
    ]


def authenticate_campaign_completion(
    document: Any,
    expected_mode: str,
    expected_plan: Path,
    expected_market: Path | None,
) -> dict[str, Any]:
    if not isinstance(document, dict):
        raise Refusal("campaign evidence was not an object")
    intent = document.get("execution_intent")
    execution = document.get("execution")
    if (
        document.get("schema") != "dclutch-successor-campaign-report-v1"
        or document.get("cluster") != "loopback"
        or document.get("mode") != "execute"
        or not isinstance(intent, dict)
        or intent.get("campaign_mode") != expected_mode
        or intent.get("plan") != str(expected_plan)
        or intent.get("market")
        != (str(expected_market) if expected_market is not None else None)
        or intent.get("authorized_mutation") is not True
        or not isinstance(execution, dict)
        or execution.get("completed") is not True
    ):
        raise Refusal("campaign completion changed its exact mode, inputs, or status")
    campaign_genesis = canonical_pubkey(
        document.get("genesis_hash"), "campaign observed genesis"
    )
    if expected_mode == "administration":
        if (
            intent.get("through_stage") != "activation"
            or execution.get("market") is not None
        ):
            raise Refusal("administration campaign escaped its infrastructure-only boundary")
        return execution
    if expected_mode != "founding-only" or expected_market is None:
        raise Refusal("campaign completion expected an unsupported mode")
    if (
        intent.get("through_stage") != "founding"
        or execution.get("recoveredFinalizedFounding") is not False
    ):
        raise Refusal("founding campaign was not one fresh finalized founding")
    transactions = execution.get("transactions")
    if not isinstance(transactions, list):
        raise Refusal("founding campaign omitted its finalized transaction history")
    labels = [
        row.get("label")
        for row in transactions
        if isinstance(row, dict) and row.get("label") in FOUNDING_SUCCESS_MUTATIONS
    ]
    if labels != list(FOUNDING_SUCCESS_MUTATIONS):
        raise Refusal("founding campaign changed its exact six-mutation success order")
    market = execution.get("market")
    accounts = market.get("accounts") if isinstance(market, dict) else None
    ledger = accounts.get("resolution_funding_ledger") if isinstance(accounts, dict) else None
    if not isinstance(ledger, dict):
        raise Refusal("founding campaign omitted its stable Resolution funding ledger")
    canonical_pubkey(ledger.get("address"), "Resolution funding ledger")
    rows = {
        row["label"]: row
        for row in transactions
        if isinstance(row, dict) and row.get("label") in FOUNDING_SUCCESS_MUTATIONS
    }
    for label in FOUNDING_SUCCESS_MUTATIONS:
        row = rows[label]
        canonical_signature(row.get("signature"), f"{label} signature")
        positive_integer(row.get("slot"), f"{label} finalized slot")
        nonnegative_integer(row.get("fee_lamports"), f"{label} fee")
        positive_integer(row.get("compute_units_consumed"), f"{label} CU")
        if row.get("error") is not None or row.get("transaction_metadata_available") is not True:
            raise Refusal("founding campaign mutation omitted exact finalized metadata")
    journals = document.get("foundingSubmissionJournals")
    if (
        not isinstance(journals, list)
        or [row.get("operation") for row in journals if isinstance(row, dict)]
        != list(FOUNDING_JOURNAL_OPERATIONS)
    ):
        raise Refusal("founding campaign omitted its exact six durable journal owners")
    for index, (journal, label) in enumerate(
        zip(journals, FOUNDING_SUCCESS_MUTATIONS, strict=True)
    ):
        row = rows[label]
        genesis = canonical_pubkey(journal.get("genesisHash"), "founding journal genesis")
        if (
            journal.get("schema") != FOUNDING_JOURNAL_SCHEMA
            or journal.get("cluster") != "loopback"
            or journal.get("phase") != "finalized"
            or journal.get("evidencePath") != document.get("evidence_output")
            or journal.get("rpcUrl") != document.get("rpc_url")
            or journal.get("planSha256") != document.get("plan_sha256")
            or journal.get("marketSha256") != document.get("market_sha256")
            or journal.get("payer") != document.get("payer")
            or genesis != campaign_genesis
            or journal.get("expectedSignature") != row.get("signature")
            or journal.get("finalizedSlot") != row.get("slot")
            or journal.get("feeLamports") != row.get("fee_lamports")
            or journal.get("computeUnitsConsumed")
            != row.get("compute_units_consumed")
        ):
            raise Refusal(
                f"founding durable journal {index} does not join its finalized transaction"
            )
        for field in (
            "intentSha256",
            "signedPacketSha256",
            "transactionSha256",
            "finalizedPoststatesSha256",
            "stateSha256",
        ):
            lowercase_sha256(journal.get(field), f"founding journal {index} {field}")
    return execution


def founding_compute_units(document: Any) -> dict[str, int]:
    execution = document.get("execution") if isinstance(document, dict) else None
    transactions = execution.get("transactions") if isinstance(execution, dict) else None
    if not isinstance(transactions, list):
        raise Refusal("founding evidence omitted transaction CU history")
    rows = {
        row.get("label"): row
        for row in transactions
        if isinstance(row, dict) and row.get("label") in FOUNDING_SUCCESS_MUTATIONS
    }
    if set(rows) != set(FOUNDING_SUCCESS_MUTATIONS):
        raise Refusal("founding evidence omitted one exact success-mutation CU")
    return {
        metric: positive_integer(
            rows[label].get("compute_units_consumed"), f"{label} CU"
        )
        for metric, label in zip(
            FOUNDING_COMPUTE_LABELS,
            FOUNDING_SUCCESS_MUTATIONS,
            strict=True,
        )
    }


def authenticate_participant_fixture_liquidity(
    campaign: dict[str, Any],
    market: dict[str, Any],
    participant: str,
    source_account: str,
) -> dict[str, Any]:
    execution = campaign.get("execution")
    fixture = (
        execution.get("localParticipantFixtureLiquidity")
        if isinstance(execution, dict)
        else None
    )
    expected_keys = {
        "sourceTokenAccount",
        "sourceOwner",
        "quantityAtoms",
        "foundingCollateralAtoms",
        "totalSupplyAtoms",
        "mint",
        "mintAuthorityRemoved",
        "transactionSignature",
        "finalizedSlot",
        "computeUnitsConsumed",
    }
    founding_atoms = market.get("initial_collateral_atoms")
    expected_mint = campaign.get("founding_targets", {}).get("collateral_mint")
    if (
        not isinstance(fixture, dict)
        or set(fixture) != expected_keys
        or fixture.get("sourceTokenAccount") != source_account
        or fixture.get("sourceOwner") != participant
        or fixture.get("quantityAtoms") != PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS
        or not isinstance(founding_atoms, int)
        or founding_atoms <= 0
        or fixture.get("foundingCollateralAtoms") != founding_atoms
        or fixture.get("totalSupplyAtoms")
        != founding_atoms + PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS
        or fixture.get("mint") != expected_mint
        or fixture.get("mintAuthorityRemoved") is not True
        or not isinstance(fixture.get("transactionSignature"), str)
        or not fixture["transactionSignature"]
        or not isinstance(fixture.get("finalizedSlot"), int)
        or fixture["finalizedSlot"] <= 0
        or not isinstance(fixture.get("computeUnitsConsumed"), int)
        or fixture["computeUnitsConsumed"] <= 0
    ):
        raise Refusal(
            "founding evidence omitted the exact 100,000,000-atom, "
            "authority-removed local participant liquidity receipt"
        )
    return fixture


def run_one(
    paths: Paths,
    gate_path: Path,
    gate: dict[str, Any],
    gate_digest: str,
    index: int,
    through: str,
    hold_participant: bool,
) -> dict[str, Any]:
    name, seed = named_seed(index)
    run = paths.work / "runs" / name
    run.mkdir(parents=True, exist_ok=False)
    (run / "stages").mkdir()
    port = allocate_port_block(index)
    url = f"http://127.0.0.1:{port}"
    prepare_work = run / "mutable"
    plan = prepare_work / "plan.json"
    prepare = run_stage(
        run,
        1,
        "prepare-mutable",
        [
            str(paths.bootstrap),
            "local-mutable-prepare-v1",
            "--work",
            str(prepare_work),
            "--output",
            str(plan),
            "--checked-release-gate",
            str(gate_path),
            "--expected-checked-release-gate-sha256",
            gate_digest,
            "--expected-source-revision",
            gate["source_revision"],
            "--expected-source-tree-sha256",
            gate["source_tree_sha256"],
            "--seed",
            seed,
        ],
    )
    report = json.loads(prepare.stdout)
    if report.get("schema") != "dclutch-local-mutable-prepare-report-v1":
        raise Refusal("local mutable preparation returned another report schema")
    expected_key_root = (prepare_work / "keys").resolve()
    for role, key_text in report.get("keypairs", {}).items():
        key = canonical_file(key_text, f"disposable local key {role}")
        if key.parent != expected_key_root:
            raise Refusal(f"local mutable key {role} escaped the owned seed directory")
    run_stage(
        run,
        2,
        "authenticate-mutable",
        [
            str(paths.bootstrap),
            "local-mutable-plan-authenticate-v1",
            "--plan",
            str(plan),
        ],
    )

    ledger = run / "ledger"
    validator_log = (run / "validator.log").open("wb")
    child: subprocess.Popen[bytes] | None = None
    watchdog: subprocess.Popen[bytes] | None = None
    try:
        mint_key = require_role_key(report, VALIDATOR_MINT_ROLE)
        mint_address = key_address(paths.solana, mint_key)
        child = subprocess.Popen(
            validator_argv(
                paths.validator,
                ledger,
                report["account_dir"],
                mint_address,
                port,
            ),
            stdin=subprocess.DEVNULL,
            stdout=validator_log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        watchdog = subprocess.Popen(
            [
                sys.executable,
                str(Path(__file__).with_name("watchdog.py")),
                "--supervisor-pid",
                str(os.getpid()),
                "--validator-pid",
                str(child.pid),
                "--ledger",
                str(ledger),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        wait_ready(url, child)
        if rpc(url, "getGenesisHash") == "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d":
            raise Refusal(
                "fresh private validator reported the mainnet-beta genesis hash"
            )
        funding_poststate = provision_disposable_funding(run, paths, report, url)
        slot_floor = checked_mutable_slot_floor(plan)
        funding_poststate["finalized_slot"] = wait_finalized_slot(
            url, child, slot_floor
        )
        funding_poststate["checked_mutable_slot_floor"] = slot_floor
        # Preserve the finalized-slot observation separately because the
        # funding poststate was already fsynced before this wait began.
        write_json_new(
            run / "validator-readiness.json",
            {
                "schema": "dclutch-private-validator-readiness-v1",
                "checked_mutable_slot_floor": slot_floor,
                "observed_finalized_slot": funding_poststate["finalized_slot"],
            },
        )

        administration = run / "administration.json"
        run_stage(
            run,
            4,
            "administration",
            administration_campaign_argv(
                paths.bootstrap,
                url,
                plan,
                administration,
                report,
            ),
        )
        authenticate_campaign_completion(
            read_unique_json(administration, "finalized administration evidence"),
            "administration",
            plan,
            None,
        )

        market = run / "market.json"
        fee_recipient_key = require_role_key(report, DEVELOPMENT_FEE_RECIPIENT_ROLE)
        fee_recipient = key_address(paths.solana, fee_recipient_key)
        market_stage = run_stage(
            run,
            5,
            "market-input",
            [
                str(paths.bootstrap),
                "local-private-validator-market-v1",
                "--plan",
                str(plan),
                "--rpc-url",
                url,
                "--fee-basis-points",
                str(DEVELOPMENT_FEE_BASIS_POINTS),
                "--fee-recipient-keypair",
                str(fee_recipient_key),
            ],
        )
        write_bytes_new(market, market_stage.stdout)
        evidence = run / "founding.json"
        run_stage(
            run,
            6,
            "founding",
            founding_campaign_argv(
                paths.bootstrap,
                url,
                plan,
                market,
                evidence,
                report,
            ),
        )

        participant = run / "participant.json"
        participant_key = require_role_key(report, PARTICIPANT_ROLE)
        fixture_source_key = require_role_key(report, PARTICIPANT_FIXTURE_SOURCE_ROLE)
        participant_address = key_address(paths.solana, participant_key)
        fixture_source_address = key_address(paths.solana, fixture_source_key)
        campaign_report = read_unique_json(evidence, "finalized founding evidence")
        authenticate_campaign_completion(
            campaign_report,
            "founding-only",
            plan,
            market,
        )
        if campaign_report.get("genesis_hash") != funding_poststate.get("genesisHash"):
            raise Refusal("founding campaign changed the local test-bankroll genesis")
        market_report = read_unique_json(market, "market input")
        fixture_liquidity = authenticate_participant_fixture_liquidity(
            campaign_report,
            market_report,
            participant_address,
            fixture_source_address,
        )
        payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
        minimum_slot = rpc(url, "getSlot", [{"commitment": "finalized"}])
        if not isinstance(minimum_slot, int) or minimum_slot <= 0:
            raise Refusal(
                "localhost finalized slot after founding was not a positive integer"
            )
        run_stage(
            run,
            7,
            "participant",
            [
                str(paths.bootstrap),
                "local-private-validator-user-position-admission-v1",
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--campaign-evidence",
                str(evidence),
                "--position-owner",
                participant_address,
                "--position-owner-keypair",
                str(participant_key),
                "--fee-payer",
                key_address(paths.solana, payer_key),
                "--fee-payer-keypair",
                str(payer_key),
                "--minimum-finalized-slot",
                str(minimum_slot),
                "--output",
                str(participant),
                "--collateral-source-owner",
                participant_address,
                "--collateral-source-owner-keypair",
                str(participant_key),
                "--collateral-source-account",
                fixture_source_address,
                "--collateral-quantity-atoms",
                str(PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS),
                "--execute",
            ],
        )
        participant_report = read_unique_json(
            participant, "participant admission evidence"
        )
        if (
            participant_report.get("schema")
            != "dclutch-owned-loopback-user-position-admission-execution-v1"
            or participant_report.get("cluster") != "owned-loopback"
            or participant_report.get("phase") != "finalized"
            or not participant_report.get("authorizedMutation")
            or not isinstance(participant_report.get("finalized"), dict)
            or not isinstance(participant_report.get("collateral"), dict)
            or participant_report["collateral"].get("phase") != "finalized"
            or not isinstance(participant_report["collateral"].get("finalized"), dict)
            or participant_report["collateral"].get("intent", {}).get("sourceAccount")
            != fixture_source_address
            or participant_report["collateral"].get("intent", {}).get("sourceOwner")
            != participant_address
            or participant_report["collateral"].get("intent", {}).get("quantityAtoms")
            != PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS
        ):
            raise Refusal(
                "participant admission did not preserve finalized admission plus exact "
                "100,000,000-atom collateral preparation"
            )

        founding_metrics = founding_compute_units(campaign_report)
        metric = founding_metrics["founding-dcltgmf3"]
        bankroll_compute_units = canonical_decimal(
            funding_poststate["transaction"]["computeUnitsConsumed"],
            "local test-bankroll CU",
        )
        fee_profile = {
            "basis_points_numerator": DEVELOPMENT_FEE_BASIS_POINTS,
            "basis_points_denominator": FEE_BASIS_POINTS_DENOMINATOR,
            "gross_unit": "collateral-atoms",
            "rounding": "floor-per-side",
            "seller_fee_atoms": {
                "gross_multiplier": DEVELOPMENT_FEE_BASIS_POINTS,
                "divisor": FEE_BASIS_POINTS_DENOMINATOR,
            },
            "buyer_fee_atoms": {
                "gross_multiplier": DEVELOPMENT_FEE_BASIS_POINTS,
                "divisor": FEE_BASIS_POINTS_DENOMINATOR,
            },
            "recipient_credit": "seller_fee_atoms + buyer_fee_atoms",
            "recipient": fee_recipient,
            "recipient_role": "founding-source-funder",
            "market_input_sha256": sha256_file(market),
        }
        if through == "participant":
            handoff_path = run / "participant-handoff.json"
            result = {
                "schema": PARTICIPANT_PROBE_RUN_SCHEMA,
                "name": name,
                "seed_sha256": sha256_bytes(bytes.fromhex(seed)),
                "status": "passed",
                "finalized_stages": ["founding", "participant"],
                "dcltgmf3_compute_units": metric,
                "compute_units": {
                    "local-test-bankroll": bankroll_compute_units,
                    **founding_metrics,
                },
                "fee_profile": fee_profile,
                "participant_fixture_liquidity": fixture_liquidity,
                "plan": str(plan),
                "administration_evidence": str(administration),
                "administration_evidence_sha256": sha256_file(administration),
                "provisioning_evidence": str(run / "provisioning-poststate.json"),
                "provisioning_evidence_sha256": sha256_file(
                    run / "provisioning-poststate.json"
                ),
                "founding_evidence": str(evidence),
                "participant_evidence": str(participant),
                "validator_log": str(run / "validator.log"),
            }
            write_json_new(run / "RESULT.json", result)
            if hold_participant:
                handoff = participant_handoff_document(
                    source_revision=gate["source_revision"],
                    checked_release_gate_sha256=gate_digest,
                    rpc_url=url,
                    validator_pid=child.pid,
                    plan=plan,
                    market=market,
                    founding=evidence,
                    participant=participant,
                    key_directory=prepare_work / "keys",
                )
                hold_after_participant(handoff_path, handoff, child)
            return result

        direct, schedule, next_ordinal = run_direct_lifecycle(
            run,
            paths,
            url,
            plan,
            market,
            evidence,
            participant,
            prepare_work / "keys",
            8,
        )
        post_direct, _next_ordinal = run_post_direct_lifecycle(
            run,
            paths,
            report,
            url,
            plan,
            market,
            evidence,
            schedule,
            next_ordinal,
        )
        result = {
            "schema": RUN_SCHEMA if through == "full" else FULL_PROBE_RUN_SCHEMA,
            "name": name,
            "seed_sha256": sha256_bytes(bytes.fromhex(seed)),
            "status": "passed",
            "dcltgmf3_compute_units": metric,
            "compute_units": {
                "local-test-bankroll": bankroll_compute_units,
                **founding_metrics,
                **direct["compute_units"],
                **post_direct["compute_units"],
            },
            "fee_profile": fee_profile,
            "plan": str(plan),
            "administration_evidence": str(administration),
            "administration_evidence_sha256": sha256_file(administration),
            "provisioning_evidence": str(run / "provisioning-poststate.json"),
            "provisioning_evidence_sha256": sha256_file(
                run / "provisioning-poststate.json"
            ),
            "founding_evidence": str(evidence),
            "participant_evidence": str(participant),
            "direct": direct,
            "post_direct": post_direct,
            "validator_log": str(run / "validator.log"),
        }
        write_json_new(run / "RESULT.json", result)
        return result
    finally:
        terminate_group(child)
        if watchdog is not None:
            try:
                watchdog.wait(timeout=5)
            except subprocess.TimeoutExpired:
                terminate_group(watchdog)
        validator_log.close()


def arithmetic_mean(values: Iterable[int]) -> dict[str, int]:
    rows = list(values)
    if not rows:
        return {"numerator": 0, "denominator": 0, "floor": 0, "remainder": 0}
    total = sum(rows)
    count = len(rows)
    return {
        "numerator": total,
        "denominator": count,
        "floor": total // count,
        "remainder": total % count,
    }


def compute_unit_report(runs: Sequence[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    rows: dict[str, list[int]] = {}
    for run in runs:
        metrics = run.get("compute_units", {})
        if not isinstance(metrics, dict):
            raise Refusal(
                "passed lifecycle run omitted its named compute-unit projection"
            )
        for label, value in metrics.items():
            if (
                not isinstance(label, str)
                or not label
                or not isinstance(value, int)
                or value < 0
            ):
                raise Refusal(
                    "passed lifecycle run carried an invalid named compute-unit value"
                )
            rows.setdefault(label, []).append(value)
    return {
        label: {"pass_count": len(values), "arithmetic_mean": arithmetic_mean(values)}
        for label, values in sorted(rows.items())
    }


def parse(argv: Sequence[str]) -> tuple[Paths, int, str, bool]:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--release-root", required=True)
    parser.add_argument("--expected-release-gate-sha256")
    parser.add_argument("--expected-release-source-revision")
    parser.add_argument("--expected-release-source-tree-sha256")
    parser.add_argument("--validator", required=True)
    parser.add_argument("--solana", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--reuse-bootstrap-work")
    parser.add_argument("--seeds", type=int, default=20)
    parser.add_argument(
        "--through", choices=("participant", "full-probe", "full"), default="full"
    )
    parser.add_argument("--hold-after-participant", action="store_true")
    args = parser.parse_args(argv)
    if args.hold_after_participant and args.through != "participant":
        raise Refusal("--hold-after-participant requires --through participant")
    if args.through == "full" and args.seeds != 20:
        raise Refusal("full release evidence requires exactly 20 named seeds")
    if args.through == "full-probe" and args.seeds != 1:
        raise Refusal(
            "the full-lifecycle development probe requires exactly one named seed"
        )
    if args.through == "full":
        raise Refusal(
            "twenty-seed private-validator release evidence is not accepted in this revision; "
            "missing semantic owners: " + ", ".join(FULL_LIFECYCLE_BLOCKERS)
        )
    if args.through == "participant" and args.seeds != 1:
        raise Refusal(
            "the founding/participant development probe requires exactly one named seed"
        )
    work = Path(args.work)
    if not work.is_absolute() or work.exists() or not work.parent.is_dir():
        raise Refusal(
            "--work must be a fresh absolute directory with an existing parent"
        )
    paths = Paths(
        repo=canonical_directory(args.repo, "repository"),
        release_root=canonical_directory(args.release_root, "checked release root"),
        expected_release_gate_sha256=args.expected_release_gate_sha256,
        expected_release_source_revision=args.expected_release_source_revision,
        expected_release_source_tree_sha256=args.expected_release_source_tree_sha256,
        bootstrap=work / "host-target/release/dclutch-local-successor-bootstrap",
        reuse_bootstrap_work=(
            None
            if args.reuse_bootstrap_work is None
            else canonical_directory(args.reuse_bootstrap_work, "bootstrap source work")
        ),
        validator=canonical_file(args.validator, "solana-test-validator"),
        solana=canonical_file(args.solana, "solana CLI"),
        work=work,
    )
    return paths, args.seeds, args.through, args.hold_after_participant


def main(argv: Sequence[str]) -> int:
    paths, seeds, through, hold_participant = parse(argv)
    commit = clean_commit(paths.repo)
    tree = clean_tree(paths.repo)
    preflight_bytes, preflight = run_offline_preflight(paths, commit, tree, through)
    if clean_commit(paths.repo) != commit or clean_tree(paths.repo) != tree:
        raise Refusal(
            "private lifecycle source changed after its accepted offline preflight"
        )
    gate_path, gate, gate_digest = checked_gate(paths, commit)
    if clean_commit(paths.repo) != commit or clean_tree(paths.repo) != tree:
        raise Refusal(
            "private lifecycle source changed between preflight and work creation"
        )
    paths.work.mkdir(mode=0o700)
    preflight_receipt = paths.work / OFFLINE_PREFLIGHT_RECEIPT
    write_bytes_new(preflight_receipt, preflight_bytes)
    (paths.work / "runs").mkdir()
    paths = build_bootstrap(paths, commit, gate_digest)
    help_sha256 = command_surface(paths.bootstrap, through)
    runner_path = canonical_file(Path(__file__), "private-validator lifecycle runner")
    watchdog_path = canonical_file(
        Path(__file__).with_name("watchdog.py"), "private-validator lifecycle watchdog"
    )
    runs: list[dict[str, Any]] = []
    failures: list[dict[str, str]] = []
    for index in range(1, seeds + 1):
        try:
            runs.append(
                run_one(
                    paths,
                    gate_path,
                    gate,
                    gate_digest,
                    index,
                    through,
                    hold_participant,
                )
            )
        except Exception as error:
            failures.append({"seed": f"seed-{index:02d}", "error": str(error)})
    mean = arithmetic_mean(row["dcltgmf3_compute_units"] for row in runs)
    compute_units = compute_unit_report(runs)
    summary = {
        "schema": (
            SCHEMA
            if through == "full"
            else (
                FULL_PROBE_SCHEMA
                if through == "full-probe"
                else PARTICIPANT_PROBE_SCHEMA
            )
        ),
        "evidence_level": (
            "local-private-validator"
            if through == "full"
            else (
                "local-private-validator-full-lifecycle-development-probe"
                if through == "full-probe"
                else "local-private-validator-founding-participant-probe"
            )
        ),
        "source_revision": commit,
        "source_tree": tree,
        "offline_preflight": {
            "path": str(preflight_receipt),
            "sha256": sha256_file(preflight_receipt),
            "model_sha256": preflight["model_sha256"],
            "source_set_sha256": preflight["repository"]["source_set_sha256"],
        },
        "checked_release_gate": str(gate_path),
        "checked_release_gate_sha256": gate_digest,
        "bootstrap_sha256": sha256_file(paths.bootstrap),
        "bootstrap_help_sha256": help_sha256,
        "orchestrator_files": {
            "runner": {"path": str(runner_path), "sha256": sha256_file(runner_path)},
            "watchdog": {
                "path": str(watchdog_path),
                "sha256": sha256_file(watchdog_path),
            },
        },
        "named_seed_count": seeds,
        "through": through,
        "pass_count": len(runs),
        "fail_count": len(failures),
        "dcltgmf3_compute_units_arithmetic_mean": mean,
        "compute_unit_report": compute_units,
        "runs": runs,
        "failures": failures,
        "external_writes": False,
        "network": "fresh-loopback-validator-only",
    }
    write_json_new(paths.work / "SUMMARY.json", summary)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if len(runs) == seeds else 1


def interrupted(signum: int, _frame: Any) -> None:
    raise Refusal(
        f"interrupted by signal {signum}; contained children are being terminated"
    )


if __name__ == "__main__":
    try:
        signal.signal(signal.SIGINT, interrupted)
        signal.signal(signal.SIGTERM, interrupted)
        raise SystemExit(main(sys.argv[1:]))
    except Refusal as error:
        print(f"private-validator-lifecycle: REFUSED: {error}", file=sys.stderr)
        raise SystemExit(2) from error
