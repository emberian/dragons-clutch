#!/usr/bin/env python3
"""Offline source-contract preflight for the PRIVATE lifecycle.

This is deliberately a source reader, not a validator probe and not a second
protocol implementation.  It joins the Python supervisor to the Rust exterior
callers and their semantic-owner constants before Cargo, a validator, a key, or
RPC can enter the run.  The report is an expected-execution model whose facts
are either extracted from the current source or named as checked source
predicates.
"""

from __future__ import annotations

import argparse
import ast
import dataclasses
import hashlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA = "dclutch-private-lifecycle-offline-preflight-v1"
MAX_SOURCE_BYTES = 24 * 1024 * 1024
PACKET_BYTES = 1_232
DEVNET_LOCK_LIMIT = 64


class Refusal(RuntimeError):
    """One fail-closed source-contract refusal."""


@dataclasses.dataclass(frozen=True)
class CommandExposure:
    command: str
    dispatch_fragment: str
    help_path: str
    help_function: str
    owner_path: str | None = None
    owner_fragment: str | None = None


@dataclasses.dataclass(frozen=True)
class StageContract:
    stage: str
    completion_stage: bool
    commands: tuple[str, ...]
    input_artifacts: tuple[str, ...]
    signer_sources: tuple[str, ...]
    geometry_owner: str
    prestate_predicates: tuple[str, ...]
    poststate_predicates: tuple[str, ...]
    terminal: str
    consumer: str
    handoff_schema: str
    completion_pointer: str
    completion_value: str


RUNNER = "tools/release/private-validator-lifecycle/run.py"
MAIN = "tools/local-validator/bootstrap/successor/src/main.rs"
SUCCESSOR = "tools/local-validator/bootstrap/successor/src"


EXPOSURES: tuple[CommandExposure, ...] = (
    CommandExposure(
        "local-mutable-prepare-v1",
        'Some("local-mutable-prepare-v1")',
        f"{SUCCESSOR}/local_mutable.rs",
        "usage",
    ),
    CommandExposure(
        "local-mutable-plan-authenticate-v1",
        'Some("local-mutable-plan-authenticate-v1")',
        f"{SUCCESSOR}/local_mutable.rs",
        "usage",
    ),
    CommandExposure(
        "local-private-validator-market-v1",
        'Some("local-private-validator-market-v1")',
        f"{SUCCESSOR}/local_mutable.rs",
        "usage",
    ),
    CommandExposure(
        "campaign",
        'Some("campaign")',
        MAIN,
        "campaign_usage_v1",
    ),
    CommandExposure(
        "local-private-validator-user-position-admission-v1",
        'Some("local-private-validator-user-position-admission-v1")',
        f"{SUCCESSOR}/user_position_admission.rs",
        "local_usage",
    ),
    CommandExposure(
        "local-private-validator-direct-trade-produce-v1",
        'Some("local-private-validator-direct-trade-produce-v1")',
        f"{SUCCESSOR}/direct_trade_producer.rs",
        "usage",
    ),
    CommandExposure(
        "local-private-validator-direct-trade-v1",
        'Some("local-private-validator-direct-trade-v1")',
        f"{SUCCESSOR}/direct_trade.rs",
        "usage",
    ),
    CommandExposure(
        "local-private-validator-direct-payout-schedule-v1",
        "private_lifecycle::DIRECT_PAYOUT_SCHEDULE_COMMAND_V1",
        f"{SUCCESSOR}/private_lifecycle.rs",
        "direct_payout_schedule_usage",
        f"{SUCCESSOR}/private_lifecycle.rs",
        "DIRECT_PAYOUT_SCHEDULE_COMMAND_V1",
    ),
    CommandExposure(
        "local-private-validator-pyth-vaa-provision-v1",
        'Some("local-private-validator-pyth-vaa-provision-v1")',
        f"{SUCCESSOR}/terminal_exterior_pyth.rs",
        "usage",
    ),
    CommandExposure(
        "local-private-validator-flagship-resolution-v1",
        "OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[0]",
        f"{SUCCESSOR}/flagship_resolution.rs",
        "owned_loopback_usage",
        MAIN,
        "OWNED_LOOPBACK_TERMINAL_COMMANDS_V1",
    ),
    CommandExposure(
        "local-private-validator-wallet-terminal-payout-input-v1",
        "terminal_lifecycle::OWNED_LOOPBACK_WALLET_TERMINAL_INPUT_COMMAND_V1",
        f"{SUCCESSOR}/terminal_lifecycle.rs",
        "owned_loopback_usage",
        f"{SUCCESSOR}/terminal_lifecycle.rs",
        "OWNED_LOOPBACK_WALLET_TERMINAL_INPUT_COMMAND_V1",
    ),
    CommandExposure(
        "local-private-validator-wallet-terminal-payout-v1",
        'Some("local-private-validator-wallet-terminal-payout-v1")',
        f"{SUCCESSOR}/wallet_terminal_payout_exterior.rs",
        "usage",
    ),
    CommandExposure(
        "local-private-validator-terminal-sequence-v1",
        "OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[1]",
        f"{SUCCESSOR}/terminal_sequence.rs",
        "owned_loopback_usage",
        MAIN,
        "OWNED_LOOPBACK_TERMINAL_COMMANDS_V1",
    ),
    CommandExposure(
        "local-private-validator-aggregate-retirement-v1",
        "aggregate_retirement_exterior::COMMAND_V1",
        f"{SUCCESSOR}/aggregate_retirement_exterior.rs",
        "usage",
        f"{SUCCESSOR}/aggregate_retirement_exterior.rs",
        "COMMAND_V1",
    ),
    CommandExposure(
        "local-private-validator-pyth-provider-closure-v1",
        'Some("local-private-validator-pyth-provider-closure-v1")',
        MAIN,
        "usage",
    ),
    CommandExposure(
        "local-private-validator-activity-stage-completion-v1",
        "private_activity::STAGE_COMMAND_V1",
        f"{SUCCESSOR}/private_activity.rs",
        "usage",
        f"{SUCCESSOR}/private_activity.rs",
        "STAGE_COMMAND_V1",
    ),
    CommandExposure(
        "local-private-validator-activity-manifest-v1",
        "private_activity::MANIFEST_COMMAND_V1",
        f"{SUCCESSOR}/private_activity.rs",
        "usage",
        f"{SUCCESSOR}/private_activity.rs",
        "MANIFEST_COMMAND_V1",
    ),
    CommandExposure(
        "local-private-validator-finalized-activity-capture-v1",
        "private_activity::CAPTURE_COMMAND_V1",
        f"{SUCCESSOR}/private_activity.rs",
        "usage",
        f"{SUCCESSOR}/private_activity.rs",
        "CAPTURE_COMMAND_V1",
    ),
    CommandExposure(
        "local-private-validator-lifecycle-session-v1",
        "private_activity::LIFECYCLE_SESSION_COMMAND_V1",
        f"{SUCCESSOR}/private_activity.rs",
        "usage",
        f"{SUCCESSOR}/private_activity.rs",
        "LIFECYCLE_SESSION_COMMAND_V1",
    ),
    CommandExposure(
        "local-private-validator-lifecycle-receipt-v1",
        "private_lifecycle::COMMAND_V1",
        f"{SUCCESSOR}/private_lifecycle.rs",
        "usage",
        f"{SUCCESSOR}/private_lifecycle.rs",
        "COMMAND_V1",
    ),
)


STAGES: tuple[StageContract, ...] = (
    StageContract(
        "prepare",
        False,
        ("local-mutable-prepare-v1", "local-mutable-plan-authenticate-v1"),
        ("CHECKED_UPGRADE_GATE.json", "13 checked SBF links", "18-account genesis directory"),
        ("seed-derived disposable roles; no key is opened by the Python supervisor"),
        "local_mutable.rs",
        ("clean source and checked release bind the same source/tree digest",),
        ("seven distinct mutable Loader pairs and two immutable Pyth pairs",),
        "dclutch-local-mutable-prepare-report-v1",
        "bankroll and administration",
        "dclutch-local-mutable-prepare-report-v1",
        "/schema",
        "dclutch-local-mutable-prepare-report-v1",
    ),
    StageContract(
        "funding",
        False,
        (),
        ("prepare report", "vacant campaign payer", "five vacant protocol-created roles"),
        ("core-upgrade-authority signs and pays the one local Solana transfer",),
        "run.py finalized_local_bankroll_transaction",
        ("campaign payer absent", "source balance exceeds 100 SOL plus fee", "created roles vacant"),
        ("campaign payer exactly 100,000,000,000 lamports", "source delta is amount plus fee"),
        "dclutch-private-validator-local-test-bankroll-v1 /status=finalized",
        "administration and founding campaign fee payer",
        "dclutch-private-validator-local-test-bankroll-v1",
        "/status",
        "finalized",
    ),
    StageContract(
        "administration",
        False,
        ("campaign",),
        ("authenticated mutable plan", "funded campaign payer state"),
        ("core-upgrade-authority through campaign_administration_keypairs",),
        "campaign.rs durable transactions",
        ("seven Loader pairs at checked slots", "Registry/Core infrastructure uninitialized"),
        ("Core infrastructure initialized", "five-role release activated"),
        "campaign report /execution/completed=true, through activation",
        "market-input and founding",
        "dclutch-successor-campaign-report-v1",
        "/execution/completed",
        "true",
    ),
    StageContract(
        "founding",
        True,
        ("local-private-validator-market-v1", "campaign"),
        ("plan", "market input", "administration", "six founding journals"),
        ("campaign_founding_keypairs plus founding-founder public identity",),
        "market.rs + founding_submission_journal.rs dynamic census",
        ("five exact rent prefundings", "fixture supply partition", "Market/custody/Claims state vacant"),
        ("Open Market", "Claims aggregate/founder Position/admission", "accepted Resolution funding ledger"),
        "campaign report /execution/completed=true, exact six-mutation order",
        "participant admission and Direct producer",
        "dclutch-successor-campaign-report-v1",
        "/execution/completed",
        "true",
    ),
    StageContract(
        "participant",
        True,
        ("local-private-validator-user-position-admission-v1",),
        ("founding evidence", "100,000,000-atom fixture source", "finalized slot floor"),
        ("participant owner; core-upgrade-authority fee payer",),
        "user_position_admission.rs durable admission/collateral messages",
        ("participant Position vacant", "source token owns exact fixture quantity"),
        ("Position admitted", "participant token state finalized", "exact Custody delegation"),
        "admission execution /phase=finalized and collateral /phase=finalized",
        "Direct producer",
        "dclutch-owned-loopback-user-position-admission-execution-v1",
        "/phase",
        "finalized",
    ),
    StageContract(
        "alt",
        True,
        ("local-private-validator-direct-trade-v1",),
        ("Direct private session", "ordered 57-address routing set"),
        ("Direct payer from authenticated private session",),
        "direct_trade.rs lookup create/extend/freeze journals",
        ("lookup table vacant or exact durable prefix",),
        ("one frozen activated ALT with exact 57-address digest",),
        "ordered Direct mutation journals /phase=finalized",
        "capability seal",
        "dclutch-owned-loopback-direct-trade-journal-v1",
        "/phase",
        "finalized",
    ),
    StageContract(
        "seal",
        True,
        ("local-private-validator-direct-trade-v1",),
        ("frozen Direct ALT", "public manifest", "checked release"),
        ("Direct payer from authenticated private session",),
        "direct_trade.rs capability-seal message",
        ("seal vacant or exact finalized journal",),
        ("capability seal exact bytes and routing digest",),
        "Direct capability-seal journal /phase=finalized",
        "Direct Hot",
        "dclutch-owned-loopback-direct-trade-journal-v1",
        "/phase",
        "finalized",
    ),
    StageContract(
        "direct",
        True,
        (
            "local-private-validator-direct-trade-produce-v1",
            "local-private-validator-direct-trade-v1",
            "local-private-validator-direct-payout-schedule-v1",
        ),
        ("founding report", "participant report", "ALT", "seal", "two signed intents"),
        ("founding-founder seller; participant buyer; session payer",),
        "direct_trade.rs exact v0 Hot geometry",
        ("maker roots/replays vacant or authenticated", "seller/buyer Positions and tokens exact"),
        ("10 exact poststates", "K seller claims plus one buyer claim", "payout schedule"),
        "Direct evidence /status=finalized and ordered mutation closure through Hot",
        "Pyth prerequisites; then Resolution and payouts",
        "dclutch-owned-loopback-direct-trade-finalized-v1",
        "/status",
        "finalized",
    ),
    StageContract(
        "resolution",
        True,
        ("local-private-validator-pyth-vaa-provision-v1", "local-private-validator-flagship-resolution-v1"),
        ("eight Pyth journals", "four-field Pyth facts", "founding funding ledger", "Resolution ALTs"),
        ("table authority; submitter; resolver; vacant update account signer",),
        "flagship_resolution.rs bounded v0 messages",
        ("update account vacant", "active accepted funding", "Core Open"),
        ("submit/provider-execute/Core-accept/reclaim", "verified terminal Core"),
        "Resolution checkpoint /verifiedTerminal=true",
        "every scheduled wallet payout",
        "dclutch-owned-loopback-flagship-resolution-checkpoint-v3",
        "/verifiedTerminal",
        "true",
    ),
    StageContract(
        "payout",
        True,
        (
            "local-private-validator-wallet-terminal-payout-input-v1",
            "local-private-validator-wallet-terminal-payout-v1",
        ),
        ("verified terminal Market", "Direct K+1 payout schedule", "per-owner Position and recipient"),
        ("claim owner; core-upgrade-authority fee payer",),
        "wallet_terminal_payout_exterior.rs canonical ALT and packet bound",
        ("selected Position balance and aggregate supply nonzero",),
        ("claim burned", "positive winner credited or losing claim burns with zero Custody effect"),
        "wallet payout evidence /phase=finalized for every nonzero claim balance",
        "terminal supply-zero authentication and retirement",
        "dclutch-local-private-validator-wallet-terminal-payout-evidence-v1",
        "/phase",
        "finalized",
    ),
    StageContract(
        "retirement",
        True,
        ("local-private-validator-terminal-sequence-v1", "local-private-validator-aggregate-retirement-v1"),
        ("all Claims supplies zero", "terminal prelude session", "source receipt", "frozen ALT"),
        ("core-upgrade-authority fee payer; protocol child authorities are PDAs",),
        "terminal_sequence.rs prelude + aggregate_retirement_exterior.rs four v0 packets",
        ("Core terminal and all liabilities discharged", "retirement accounts live",),
        ("prepare", "close-vault", "close-replay", "finish; exact rent/refund conservation"),
        "aggregate retirement completion /status=finalized",
        "activity capture, lifecycle session, receipt, dossier",
        "dclutch-owned-loopback-aggregate-retirement-completion-v1",
        "/status",
        "finalized",
    ),
)


SCHEMA_OWNERS: tuple[tuple[str, str], ...] = (
    ("DIRECT_PRODUCER_SCHEMA", f"{SUCCESSOR}/direct_trade_producer.rs"),
    ("DIRECT_FINALIZED_SCHEMA", f"{SUCCESSOR}/direct_trade.rs"),
    ("DIRECT_PAYOUT_SCHEDULE_SCHEMA", f"{SUCCESSOR}/private_lifecycle.rs"),
    ("PYTH_JOURNAL_SCHEMA", f"{SUCCESSOR}/terminal_exterior_pyth.rs"),
    ("RESOLUTION_PRODUCER_SCHEMA", f"{SUCCESSOR}/flagship_resolution.rs"),
    ("RESOLUTION_TABLE_SCHEMA", f"{SUCCESSOR}/flagship_resolution.rs"),
    ("RESOLUTION_INPUT_SCHEMA", f"{SUCCESSOR}/flagship_resolution.rs"),
    ("RESOLUTION_CHECKPOINT_SCHEMA", f"{SUCCESSOR}/flagship_resolution.rs"),
    ("PAYOUT_INPUT_SCHEMA", f"{SUCCESSOR}/terminal_lifecycle.rs"),
    ("PAYOUT_EVIDENCE_SCHEMA", f"{SUCCESSOR}/wallet_terminal_payout_exterior.rs"),
    ("TERMINAL_SESSION_SCHEMA", f"{SUCCESSOR}/terminal_sequence.rs"),
    ("TERMINAL_JOURNAL_SCHEMA", f"{SUCCESSOR}/terminal_sequence.rs"),
    ("TERMINAL_COMPLETION_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_journal.rs"),
    ("TERMINAL_CAMPAIGN_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_journal.rs"),
    ("TERMINAL_AGGREGATE_JOURNAL_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_journal.rs"),
    ("TERMINAL_PROGRESS_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_exterior.rs"),
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def exact_repo(path: Path) -> Path:
    if not path.is_absolute():
        raise Refusal("--repo must be absolute")
    resolved = path.resolve(strict=True)
    if resolved != path or not resolved.is_dir():
        raise Refusal("--repo must be one canonical existing directory")
    return resolved


def read_source(repo: Path, relative: str) -> str:
    path = repo / relative
    try:
        stat = path.lstat()
    except FileNotFoundError as error:
        raise Refusal(f"required lifecycle source is absent: {relative}") from error
    if path.is_symlink() or not path.is_file() or stat.st_size <= 0 or stat.st_size > MAX_SOURCE_BYTES:
        raise Refusal(f"required lifecycle source is not one bounded regular file: {relative}")
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError as error:
        raise Refusal(f"required lifecycle source is not UTF-8: {relative}") from error


def _python_value(node: ast.AST, values: dict[str, Any]) -> Any:
    if isinstance(node, ast.Constant):
        return node.value
    if isinstance(node, (ast.Tuple, ast.List)):
        rows = [_python_value(row, values) for row in node.elts]
        return tuple(rows) if isinstance(node, ast.Tuple) else rows
    if isinstance(node, ast.Name) and node.id in values:
        return values[node.id]
    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
        return _python_value(node.left, values) + _python_value(node.right, values)
    raise ValueError(ast.dump(node, include_attributes=False))


def python_constants(source: str) -> dict[str, Any]:
    try:
        tree = ast.parse(source)
    except SyntaxError as error:
        raise Refusal(f"private lifecycle runner is not valid Python: {error}") from error
    pending: dict[str, ast.AST] = {}
    for row in tree.body:
        if isinstance(row, ast.Assign) and len(row.targets) == 1 and isinstance(row.targets[0], ast.Name):
            pending[row.targets[0].id] = row.value
        elif isinstance(row, ast.AnnAssign) and isinstance(row.target, ast.Name) and row.value is not None:
            pending[row.target.id] = row.value
    values: dict[str, Any] = {}
    changed = True
    while changed:
        changed = False
        for name, node in tuple(pending.items()):
            try:
                values[name] = _python_value(node, values)
            except (KeyError, TypeError, ValueError):
                continue
            del pending[name]
            changed = True
    return values


def rust_function(source: str, name: str, label: str) -> str:
    matches = list(
        re.finditer(
            rf"(?m)^\s*(?:pub(?:\(crate\))?\s+)?(?:const\s+)?fn\s+{re.escape(name)}\s*(?:<[^{{;]*>)?\s*\(",
            source,
        )
    )
    if len(matches) != 1:
        raise Refusal(f"{label} must define exactly one Rust function {name}; found {len(matches)}")
    opening = source.find("{", matches[0].end())
    if opening < 0:
        raise Refusal(f"{label} function {name} has no body")
    depth = 0
    index = opening
    state = "code"
    block_depth = 0
    raw_hashes = 0
    while index < len(source):
        char = source[index]
        nxt = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            raw = re.match(r'r(#+)?"', source[index:])
            if raw:
                raw_hashes = len(raw.group(1) or "")
                index += len(raw.group(0))
                state = "raw"
                continue
            if char == '"':
                state = "string"
            elif char == "/" and nxt == "/":
                state = "line-comment"
                index += 1
            elif char == "/" and nxt == "*":
                state = "block-comment"
                block_depth = 1
                index += 1
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    return source[matches[0].start() : index + 1]
        elif state == "string":
            if char == "\\":
                index += 1
            elif char == '"':
                state = "code"
        elif state == "raw":
            closing = '"' + ("#" * raw_hashes)
            if source.startswith(closing, index):
                index += len(closing) - 1
                state = "code"
        elif state == "line-comment":
            if char == "\n":
                state = "code"
        elif state == "block-comment":
            if char == "/" and nxt == "*":
                block_depth += 1
                index += 1
            elif char == "*" and nxt == "/":
                block_depth -= 1
                index += 1
                if block_depth == 0:
                    state = "code"
        index += 1
    raise Refusal(f"{label} function {name} has an unterminated body")


def rust_string_array(source: str, name: str, label: str) -> tuple[str, ...]:
    match = re.search(
        rf"(?s)(?:pub(?:\(crate\))?\s+)?const\s+{re.escape(name)}\s*:\s*\[[^=]+\]\s*=\s*\[(.*?)\];",
        source,
    )
    if match is None:
        raise Refusal(f"{label} omitted Rust array {name}")
    rows = tuple(re.findall(r'"([^"\\]*)"', match.group(1)))
    if not rows:
        raise Refusal(f"{label} Rust array {name} has no literal rows")
    return rows


def rust_usize(source: str, name: str, label: str) -> int:
    matches = re.findall(
        rf"(?m)^\s*(?:pub(?:\(crate\))?\s+)?const\s+{re.escape(name)}\s*:\s*usize\s*=\s*([0-9_]+)\s*;",
        source,
    )
    if len(matches) != 1:
        raise Refusal(f"{label} must own exactly one usize {name}")
    return int(matches[0].replace("_", ""))


def require_fragments(source: str, fragments: Iterable[str], label: str) -> None:
    missing = [fragment for fragment in fragments if fragment not in source]
    if missing:
        raise Refusal(f"{label} omitted required source predicates: {', '.join(missing)}")


def command_groups(constants: dict[str, Any], through: str) -> tuple[str, ...]:
    try:
        commands = list(constants["FOUNDING_PARTICIPANT_COMMANDS"])
        if through in ("full-probe", "full"):
            commands.extend(
                constants[name]
                for name in (
                    "DIRECT_PRODUCER_COMMAND",
                    "DIRECT_EXECUTE_COMMAND",
                    "DIRECT_PAYOUT_SCHEDULE_COMMAND",
                    "PYTH_PROVISION_COMMAND",
                    "FLAGSHIP_RESOLUTION_COMMAND",
                    "PAYOUT_INPUT_COMMAND",
                    "PAYOUT_EXECUTE_COMMAND",
                    "TERMINAL_SEQUENCE_COMMAND",
                    "TERMINAL_RETIREMENT_COMMAND",
                )
            )
        if through == "full":
            commands.extend(constants["FINAL_EVIDENCE_COMMANDS"])
    except (KeyError, TypeError) as error:
        raise Refusal(f"runner command constants are incomplete: {error}") from error
    if len(commands) != len(set(commands)):
        raise Refusal("runner required-command surface contains a duplicate")
    return tuple(commands)


def validate_constants(constants: dict[str, Any]) -> dict[str, Any]:
    expected_roles = ("registry", "rent", "custody", "resolution", "claims", "trading", "core")
    if constants.get("ROLE_ORDER") != expected_roles:
        raise Refusal("checked mutable role order is not the exact seven-program substrate")
    if constants.get("CAMPAIGN_ADMINISTRATION_KEY_ROLES") != ("core-upgrade-authority",):
        raise Refusal("administration signer projection changed")
    founding_roles = constants.get("CAMPAIGN_FOUNDING_KEY_ROLES")
    required_founding = {
        "campaign-payer",
        "collateral-mint",
        "collateral-wallet",
        "founding-beneficiary",
        "founding-projection-witness",
        "founding-source-funder",
        "participant",
        "direct-buyer",
    }
    if not isinstance(founding_roles, tuple) or set(founding_roles) != required_founding:
        raise Refusal("founding signer/key projection omits or invents a role")
    if constants.get("PROTOCOL_CREATED_KEY_ROLES") != (
        "collateral-mint",
        "collateral-wallet",
        "founding-beneficiary",
        "founding-projection-witness",
        "founding-source-funder",
    ):
        raise Refusal("bankroll vacancy census changed its protocol-created role set")
    if constants.get("LOCAL_AIRDROP_ROLES") != ():
        raise Refusal("private lifecycle reintroduced an airdrop role")
    amount = constants.get("LOCAL_TEST_BANKROLL_LAMPORTS")
    liquidity = constants.get("PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS")
    fee = constants.get("DEVELOPMENT_FEE_BASIS_POINTS")
    denominator = constants.get("FEE_BASIS_POINTS_DENOMINATOR")
    if amount != 100_000_000_000 or liquidity != 100_000_000:
        raise Refusal("local bankroll or participant fixture quantity changed")
    if not isinstance(fee, int) or not isinstance(denominator, int) or not 0 <= fee <= denominator:
        raise Refusal("development fee scale is invalid")
    mutations = constants.get("FOUNDING_SUCCESS_MUTATIONS")
    operations = constants.get("FOUNDING_JOURNAL_OPERATIONS")
    metrics = constants.get("FOUNDING_COMPUTE_LABELS")
    if not all(isinstance(rows, tuple) and len(rows) == 6 for rows in (mutations, operations, metrics)):
        raise Refusal("founding is not one six-journal/six-mutation/six-metric closure")
    pyth_files = constants.get("PYTH_JOURNAL_FILES")
    pyth_actions = constants.get("PYTH_ACTIONS")
    if not isinstance(pyth_files, tuple) or not isinstance(pyth_actions, tuple) or len(pyth_files) != 8 or len(pyth_actions) != 8:
        raise Refusal("Pyth prerequisite sequence is not exactly eight actions")
    for index, (path, action) in enumerate(zip(pyth_files, pyth_actions, strict=True)):
        if not path.startswith(f"{index:02d}-") or path.removeprefix(f"{index:02d}-").removesuffix(".json") != action:
            raise Refusal("Pyth journal file order differs from its action vocabulary")
    if constants.get("TERMINAL_AGGREGATE_OPERATIONS") != (
        "prepare",
        "close-vault",
        "close-replay",
        "finish",
    ):
        raise Refusal("aggregate retirement is not the exact four-packet sequence")
    for name in (
        "MAX_RESOLUTION_TABLE_INVOCATIONS",
        "MAX_RESOLUTION_STAGE_INVOCATIONS",
        "MAX_PAYOUT_INVOCATIONS",
        "MAX_TERMINAL_INVOCATIONS",
        "MAX_DIRECT_INVOCATIONS",
        "MAX_PAYOUT_TARGETS",
    ):
        value = constants.get(name)
        if not isinstance(value, int) or isinstance(value, bool) or not 1 <= value <= 160:
            raise Refusal(f"{name} is absent or outside the offline 1..160 bound")
    return {
        "program_roles": list(expected_roles),
        "bankroll_lamports": amount,
        "participant_fixture_liquidity_atoms": liquidity,
        "fee_basis_points": fee,
        "fee_denominator": denominator,
        "founding_mutations": list(mutations),
        "founding_journal_operations": list(operations),
        "pyth_actions": list(pyth_actions),
        "aggregate_retirement_operations": list(constants["TERMINAL_AGGREGATE_OPERATIONS"]),
    }


def validate_exposures(repo: Path, constants: dict[str, Any], through: str) -> list[dict[str, str]]:
    required = command_groups(constants, through)
    mapping = {row.command: row for row in EXPOSURES}
    if set(required) - set(mapping):
        raise Refusal("offline preflight has no dispatch/help model for: " + ", ".join(sorted(set(required) - set(mapping))))
    main = read_source(repo, MAIN)
    dispatch = rust_function(main, "run", MAIN)
    main_usage = rust_function(main, "usage", MAIN)
    report: list[dict[str, str]] = []
    for command in required:
        row = mapping[command]
        if row.dispatch_fragment not in dispatch:
            raise Refusal(f"{command} is absent from the successor dispatch function")
        if row.owner_path is not None:
            owner = read_source(repo, row.owner_path)
            if row.owner_fragment not in owner or command not in owner:
                raise Refusal(f"{command} dispatch constant is absent from {row.owner_path}")
        help_source = main if row.help_path == MAIN else read_source(repo, row.help_path)
        help_body = main_usage if row.help_path == MAIN and row.help_function == "usage" else rust_function(help_source, row.help_function, row.help_path)
        if command not in help_body:
            raise Refusal(f"{command} is dispatched but absent from its accepted help function")
        if r"\n+" in help_body:
            raise Refusal(f"{command} help contains a literal patch-marker prefix")
        report.append(
            {
                "command": command,
                "dispatch": f"{MAIN}::{row.dispatch_fragment}",
                "help": f"{row.help_path}::{row.help_function}",
            }
        )
    return report


def validate_schemas(repo: Path, constants: dict[str, Any]) -> list[dict[str, str]]:
    report = []
    for name, owner in SCHEMA_OWNERS:
        value = constants.get(name)
        if not isinstance(value, str) or not value:
            raise Refusal(f"runner omitted schema constant {name}")
        source = read_source(repo, owner)
        if value not in source:
            raise Refusal(f"runner {name} differs from semantic owner {owner}")
        report.append({"runner_constant": name, "schema": value, "owner": owner})
    return report


def validate_stage_vocabulary(repo: Path) -> dict[str, list[str]]:
    private_lifecycle = read_source(repo, f"{SUCCESSOR}/private_lifecycle.rs")
    activity = rust_string_array(private_lifecycle, "ACTIVITY_STAGES", "private lifecycle")
    chaos = rust_string_array(private_lifecycle, "CHAOS_STAGES", "private lifecycle")
    manifest = rust_string_array(private_lifecycle, "MANIFEST_EVENT_KINDS", "private lifecycle")
    expected_activity = ("founding", "participant", "alt", "seal", "direct", "resolution", "payout", "retirement")
    expected_chaos = ("founding", "participant", "alt", "seal", "hot", "resolution", "payout", "retire")
    expected_manifest = ("founding", "participant", "direct", "resolution", "payout", "retirement")
    if activity != expected_activity or chaos != expected_chaos or manifest != expected_manifest:
        raise Refusal("private/public/chaos lifecycle vocabularies no longer form the exact 8/8/6 projections")
    modeled = tuple(row.stage for row in STAGES if row.completion_stage)
    if modeled != expected_activity:
        raise Refusal("offline execution model differs from the Rust-owned eight-stage sequence")
    private_activity = read_source(repo, f"{SUCCESSOR}/private_activity.rs")
    require_fragments(
        private_activity,
        (
            "dclutch-owned-loopback-private-lifecycle-session-v1",
            "dclutch-owned-loopback-activity-reconcile-manifest-v1",
            "activity manifest differs from the six authenticated stage completions",
        ),
        "private activity projection",
    )
    return {"private": list(activity), "chaos": list(chaos), "public": list(manifest)}


def validate_founding_geometry(repo: Path, constants: dict[str, Any]) -> dict[str, Any]:
    market = read_source(repo, f"{SUCCESSOR}/market.rs")
    magic_matches = re.findall(
        r'const\s+GENERIC_MARKET_FOUNDING_MAGIC_V(\d+)\s*:\s*\[u8;\s*8\]\s*=\s*\*b"(DCLTGMF\d)"\s*;',
        market,
    )
    if len(magic_matches) != 1:
        raise Refusal("market source must own exactly one current generic founding magic")
    version, magic = magic_matches[0]
    fixed_name = f"GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V{version}"
    funding_name = f"GENERIC_MARKET_FOUNDING_PHYSICAL_FUNDING_ACCOUNTS_V{version}"
    complete_name = f"GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V{version}"
    fixed = rust_usize(market, fixed_name, "generic founding")
    physical = rust_usize(market, funding_name, "generic founding")
    complete = rust_usize(market, complete_name, "generic founding")
    if fixed < 1 or physical < 1 or complete < 1 or complete > DEVNET_LOCK_LIMIT:
        raise Refusal("generic founding source-derived geometry exceeds devnet's 64-key limit")
    require_fragments(
        market,
        (
            "require_devnet_complete_key_limit_v1(census)?;",
            "instruction.accounts.len() != expected_frame",
            "census.complete_keys !=",
            "loaded_writable",
            "loaded_readonly",
        ),
        "generic founding geometry",
    )
    founding_journal = read_source(repo, f"{SUCCESSOR}/founding_submission_journal.rs")
    require_fragments(
        founding_journal,
        (
            magic,
            "exact_unique_message_accounts",
            "expected_wire_bytes > 1_232",
            "founding message geometry changed",
        ),
        "founding durable journal",
    )
    mutations = constants["FOUNDING_SUCCESS_MUTATIONS"]
    operations = constants["FOUNDING_JOURNAL_OPERATIONS"]
    if sum(magic in row for row in mutations) != 1 or sum(magic.lower() in row for row in operations) != 1:
        raise Refusal("runner founding mutation/journal vocabulary differs from current generic founding magic")
    return {
        "magic": magic,
        "abi_version": int(version),
        "instruction_accounts": fixed + physical,
        "fixed_accounts": fixed,
        "physical_funding_accounts": physical,
        "complete_transaction_keys": complete,
        "devnet_lock_limit": DEVNET_LOCK_LIMIT,
        "packet_limit_bytes": PACKET_BYTES,
    }


def validate_geometry_and_state(repo: Path) -> dict[str, Any]:
    direct = read_source(repo, f"{SUCCESSOR}/direct_trade.rs")
    require_fragments(
        direct,
        (
            "evidence.static_account_count != 4",
            "evidence.unique_message_account_count != 61",
            "evidence.lookup_address_count != 57",
            "evidence.wire_bytes != 1_159",
            "evidence.poststates.len() != 10",
            "packet.len() > 1_232",
        ),
        "Direct terminal geometry",
    )
    private_lifecycle = read_source(repo, f"{SUCCESSOR}/private_lifecycle.rs")
    require_fragments(
        private_lifecycle,
        (
            "seller_claims != expected_seller_claims",
            "buyer_claims.len() != 1",
            "seller plus filled buyer partition",
        ),
        "Direct cross-stage handoff",
    )
    runner = read_source(repo, RUNNER)
    require_fragments(
        runner,
        (
            '"replay-setup": 0',
            '"token-setup": 1',
            '"lookup-create": 2',
            '"lookup-extend": 3',
            '"lookup-freeze": 4',
            '"capability-seal": 5',
            '"hot": 6',
        ),
        "Direct controller vocabulary",
    )
    terminal_lifecycle = read_source(repo, f"{SUCCESSOR}/terminal_lifecycle.rs")
    payout_operator = read_source(repo, "crates/dclutch-operator/src/wallet_terminal_payout_v3.rs")
    require_fragments(
        terminal_lifecycle,
        ("Claims supply at index {claim_index} is {supply}", "produce and execute wallet terminal payouts first"),
        "retirement supply-zero gate",
    )
    require_fragments(
        payout_operator,
        (
            "Exact collateral atoms paid; zero is a real burn outcome.",
            "let custody_replay_bytes = if report.payout == 0",
            "zero_payout_burn_requires_byte_identical_custody_and_tokens",
        ),
        "wallet zero-payout semantics",
    )
    participant = read_source(repo, f"{SUCCESSOR}/user_position_admission.rs")
    require_fragments(
        participant,
        (
            "durable collateral packet width changed",
            "durable admission packet width changed",
            "exact Custody allowance",
        ),
        "participant message geometry",
    )
    pyth = read_source(repo, f"{SUCCESSOR}/terminal_exterior_pyth.rs")
    require_fragments(
        pyth,
        (
            "dclutch-owned-loopback-pyth-prerequisite-transaction-v1",
            "PostUpdate",
            "update account",
        ),
        "Pyth prerequisite owner",
    )
    resolution = read_source(repo, f"{SUCCESSOR}/flagship_resolution.rs")
    require_fragments(
        resolution,
        ("compiled_wire_bytes > 1_232", "resolution-provider-execute-v1", "core-terminal-accept-v1", "reclaim"),
        "Resolution geometry/completion",
    )
    payout = read_source(repo, f"{SUCCESSOR}/wallet_terminal_payout_exterior.rs")
    require_fragments(
        payout,
        ("expected_wire_bytes > 1_232", "packet.len() > 1_232", "lookup_addresses_sha256"),
        "wallet payout geometry",
    )
    terminal = read_source(repo, f"{SUCCESSOR}/terminal_sequence.rs")
    require_fragments(
        terminal,
        ("15-retirement-replay-handoff.json", "maximum_preflight_wire_bytes > PACKET_DATA_BYTES", "Claims supply became nonzero during terminal retirement"),
        "terminal prelude geometry",
    )
    aggregate = read_source(repo, f"{SUCCESSOR}/aggregate_retirement_exterior.rs")
    require_fragments(
        aggregate,
        (
            "VersionedMessage::V0",
            "message.address_table_lookups.len() != 1",
            "prepare, close-vault, close-replay, and finish",
        ),
        "aggregate retirement geometry",
    )
    return {
        "direct_hot": {"static": 4, "loaded": 57, "unique": 61, "wire_bytes": 1_159, "poststates": 10},
        "packet_limit_bytes": PACKET_BYTES,
        "participant": "source-owned dynamic message width; exact history equality",
        "resolution": "source-owned routed messages; each <=1232 bytes",
        "payout": "source-owned canonical ALT; each packet <=1232 bytes",
        "retirement": "source-owned terminal ALT plus four v0 AggregateRetirement packets",
    }


def validate_runner_source_predicates(repo: Path) -> None:
    runner = read_source(repo, RUNNER)
    require_fragments(
        runner,
        (
            "prestate[\"campaignPayerLamports\"] is not None",
            "prestate[\"vacantProtocolRoles\"] != poststate[\"vacantProtocolRoles\"]",
            "founding_atoms + PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS",
            'fixture.get("mintAuthorityRemoved") is not True',
            "Direct mutation sequence is not replay through Hot exactly once",
            '"submit",\n        "resolution-provider-execute-v1",\n        "core-terminal-accept-v1",\n        "reclaim",',
            "terminal sequence handoff session identity changed",
            "packet-inadmissible monolithic terminal completion appeared",
        ),
        "private lifecycle runner cross-stage predicates",
    )


def stage_report() -> list[dict[str, Any]]:
    return [dataclasses.asdict(row) for row in STAGES]


def source_digests(repo: Path, paths: Iterable[str]) -> dict[str, str]:
    return {path: sha256(read_source(repo, path).encode()) for path in sorted(set(paths))}


def run_preflight(repo: Path, through: str) -> dict[str, Any]:
    repo = exact_repo(repo)
    if through not in ("participant", "full-probe", "full"):
        raise Refusal("--through must be participant, full-probe, or full")
    runner_source = read_source(repo, RUNNER)
    constants = python_constants(runner_source)
    constant_report = validate_constants(constants)
    exposures = validate_exposures(repo, constants, through)
    schemas = validate_schemas(repo, constants)
    vocabulary = validate_stage_vocabulary(repo)
    founding = validate_founding_geometry(repo, constants)
    geometry = validate_geometry_and_state(repo)
    validate_runner_source_predicates(repo)
    selected_stages = (
        [row for row in STAGES if row.stage in ("prepare", "funding", "administration", "founding", "participant")]
        if through == "participant"
        else list(STAGES)
    )
    digest_paths = {
        RUNNER,
        MAIN,
        *(row.help_path for row in EXPOSURES if row.command in command_groups(constants, through)),
        *(owner for _, owner in SCHEMA_OWNERS),
        f"{SUCCESSOR}/market.rs",
        f"{SUCCESSOR}/founding_submission_journal.rs",
        f"{SUCCESSOR}/private_activity.rs",
        f"{SUCCESSOR}/private_lifecycle.rs",
        "crates/dclutch-operator/src/wallet_terminal_payout_v3.rs",
    }
    report: dict[str, Any] = {
        "schema": SCHEMA,
        "status": "accepted",
        "evidence_level": "offline-source-contract-only",
        "through": through,
        "validator_started": False,
        "rpc_used": False,
        "keys_read": False,
        "build_run": False,
        "command_exposures": exposures,
        "schema_handoffs": schemas,
        "stage_vocabulary": vocabulary,
        "constants": constant_report,
        "founding_geometry": founding,
        "transaction_geometry": geometry,
        "expected_execution": [dataclasses.asdict(row) for row in selected_stages],
        "source_sha256": source_digests(repo, digest_paths),
    }
    canonical = json.dumps(report, sort_keys=True, separators=(",", ":")).encode()
    report["model_sha256"] = sha256(canonical)
    return report


def write_new(path: Path, report: dict[str, Any]) -> None:
    if not path.is_absolute():
        raise Refusal("--output must be absolute")
    parent = path.parent.resolve(strict=True)
    if parent != path.parent or path.exists() or path.is_symlink():
        raise Refusal("--output must be one absent path in a canonical directory")
    data = (json.dumps(report, indent=2, sort_keys=True) + "\n").encode()
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        output.write(data)
        output.flush()
        os.fsync(output.fileno())


def parse(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--through", choices=("participant", "full-probe", "full"), default="full-probe")
    parser.add_argument("--output", type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str]) -> int:
    arguments = parse(argv)
    report = run_preflight(arguments.repo, arguments.through)
    if arguments.output is not None:
        write_new(arguments.output, report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except Refusal as error:
        print(f"private-lifecycle-preflight: REFUSED: {error}", file=sys.stderr)
        raise SystemExit(1)
