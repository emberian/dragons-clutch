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
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence


SCHEMA = "dclutch-private-lifecycle-offline-preflight-v1"
MAX_SOURCE_BYTES = 24 * 1024 * 1024
PACKET_BYTES = 1_232
DEVNET_LOCK_LIMIT = 64
PREFLIGHT = "tools/release/private-validator-lifecycle/preflight.py"
ECONOMIC_LEDGER = "tools/economic-lifecycle-ledger/ledger.py"
PRIVATE_ECONOMIC_FIXTURE = "tools/economic-lifecycle-ledger/fixtures/private-canonical.json"
ACTIVITY_V3_ECONOMIC_FIXTURE = (
    "tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json"
)
ACTIVITY_V3_OWNER = "tools/devnet-activity/activity.py"


class Refusal(RuntimeError):
    """One fail-closed source-contract refusal."""


@dataclasses.dataclass(frozen=True)
class RepositorySnapshot:
    root: str
    head: str
    tree: str
    source_sha256: dict[str, str]
    source_set_sha256: str


_ACTIVE_SOURCE_SNAPSHOT: RepositorySnapshot | None = None


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
# The runner IMPORTS this at module scope for every `rust_schema_constant`
# derivation below, so it is part of the runner's behaviour and belongs in the
# snapshot this preflight authenticates. `run.py` binds its bytes explicitly
# too, beside its own.
SHARED_RUST_SCHEMA = "tools/lib/rust_schema.py"
# The seventeen-case chaos matrix's executable contract. The runner IMPORTS it
# too (`load_chaos_contract`), so the same argument that put the shared schema
# reader above in this snapshot puts it here: a receipt is evidence about a tree,
# and a file the runner executes is part of the tree the receipt is about. It
# arrived late because it WRITES the chaos session rather than reading one, and a
# writer looks like an output until you notice it is also stating the artifact's
# name.
CHAOS_CONTRACT = "tools/release/private-validator-lifecycle/chaos.py"
MAIN = "tools/local-validator/bootstrap/successor/src/main.rs"
SUCCESSOR = "tools/local-validator/bootstrap/successor/src"
# The retirement supply-zero gate's semantic owner. Named here rather than
# spelled at its one call site because `test_preflight.py` stages the exact set
# of files this module reads, and a path spelled twice is a path that goes stale
# on one side -- which is how the reader below came to be pointed at a file the
# gate had left.
ZERO_CLAIMS_OWNER = "crates/dclutch-wallet-terminal-input-operator/src/lib.rs"


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

SOURCE_ABORT_EXPOSURE = CommandExposure(
    "source-abort-v1",
    "source_abort_exterior::COMMAND_V1",
    f"{SUCCESSOR}/source_abort_exterior.rs",
    "usage",
    f"{SUCCESSOR}/source_abort_exterior.rs",
    "COMMAND_V1",
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
        (
            "core-upgrade-authority and distinct campaign-payer through "
            "campaign_administration_keypairs",
        ),
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
    # The successor DESERIALIZES this one (`wallet_terminal.rs` reads a
    # `PlanInputV1` off disk) and names it only inside a usage blurb, which a
    # substring check accepted for as long as the blurb existed. The wire crate
    # below is where it is declared.
    ("PAYOUT_INPUT_SCHEMA", "crates/dclutch-wallet-terminal-payout-operator/src/wire.rs"),
    ("PAYOUT_EVIDENCE_SCHEMA", f"{SUCCESSOR}/wallet_terminal_payout_exterior.rs"),
    ("TERMINAL_SESSION_SCHEMA", f"{SUCCESSOR}/terminal_sequence.rs"),
    ("TERMINAL_JOURNAL_SCHEMA", f"{SUCCESSOR}/terminal_sequence.rs"),
    ("TERMINAL_COMPLETION_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_journal.rs"),
    ("TERMINAL_CAMPAIGN_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_journal.rs"),
    ("TERMINAL_AGGREGATE_JOURNAL_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_journal.rs"),
    ("TERMINAL_PROGRESS_SCHEMA", f"{SUCCESSOR}/aggregate_retirement_exterior.rs"),
    # The odd one out, and worth saying why it belongs on a list of schemas the
    # runner reads from their PRODUCER: nothing in Rust produces a chaos session.
    # `chaos.py` writes it and `private_lifecycle.rs` authenticates it, so the
    # string is a contract the two sides pin rather than one side's output --
    # and of the two declarations, only the Rust one is readable from the other
    # language. The runner had no row here at all until now: it spelled the
    # string as a literal in a descriptor row, which this walk cannot see and
    # which the check below now forbids.
    ("CHAOS_SESSION_SCHEMA", f"{SUCCESSOR}/private_lifecycle.rs"),
)


# The same wiring, one file out. `chaos.py` is where the session's schema is
# WRITTEN, so a literal here is worse than a literal in the runner: it names the
# artifact rather than merely describing it. Both of its strings are read from
# the same Rust owner.
CHAOS_CONTRACT_SCHEMA_OWNERS: tuple[tuple[str, str], ...] = (
    ("SESSION_SCHEMA_V2", f"{SUCCESSOR}/private_lifecycle.rs"),
    ("CASE_SCHEMA_V1", f"{SUCCESSOR}/private_lifecycle.rs"),
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


def git_output(repo: Path, arguments: Sequence[str], label: str) -> bytes:
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=repo,
            check=False,
            capture_output=True,
            timeout=15,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Refusal(f"cannot inspect repository {label}: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise Refusal(f"cannot inspect repository {label}: {detail}")
    return result.stdout


def git_identity(repo: Path) -> tuple[str, str]:
    root = git_output(repo, ["rev-parse", "--show-toplevel"], "root").decode().strip()
    if root != str(repo):
        raise Refusal("--repo is not the exact Git working-tree root")
    status = git_output(
        repo,
        ["status", "--porcelain=v1", "--untracked-files=all", "-z"],
        "status",
    )
    if status:
        raise Refusal("repository is not clean; offline preflight requires one exact committed snapshot")
    head = git_output(repo, ["rev-parse", "--verify", "HEAD"], "HEAD").decode().strip()
    tree = git_output(repo, ["rev-parse", "--verify", "HEAD^{tree}"], "tree").decode().strip()
    if not re.fullmatch(r"[0-9a-f]{40}", head) or not re.fullmatch(r"[0-9a-f]{40}", tree):
        raise Refusal("repository HEAD or tree is not one full lowercase Git object id")
    return head, tree


def modeled_source_paths() -> set[str]:
    return {
        PREFLIGHT,
        RUNNER,
        SHARED_RUST_SCHEMA,
        CHAOS_CONTRACT,
        MAIN,
        ECONOMIC_LEDGER,
        PRIVATE_ECONOMIC_FIXTURE,
        ACTIVITY_V3_ECONOMIC_FIXTURE,
        ACTIVITY_V3_OWNER,
        SOURCE_ABORT_EXPOSURE.help_path,
        *(row.help_path for row in EXPOSURES),
        *(row.owner_path for row in EXPOSURES if row.owner_path is not None),
        *(owner for _, owner in SCHEMA_OWNERS),
        f"{SUCCESSOR}/market.rs",
        f"{SUCCESSOR}/founding_submission_journal.rs",
        f"{SUCCESSOR}/private_activity.rs",
        f"{SUCCESSOR}/private_lifecycle.rs",
        f"{SUCCESSOR}/direct_trade.rs",
        f"{SUCCESSOR}/terminal_lifecycle.rs",
        f"{SUCCESSOR}/user_position_admission.rs",
        "crates/dclutch-operator/src/wallet_terminal_payout_v3.rs",
        ZERO_CLAIMS_OWNER,
    }


def raw_source_bytes(repo: Path, relative: str) -> bytes:
    path = repo / relative
    try:
        stat = path.lstat()
    except FileNotFoundError as error:
        raise Refusal(f"required lifecycle source is absent: {relative}") from error
    if path.is_symlink() or not path.is_file() or stat.st_size <= 0 or stat.st_size > MAX_SOURCE_BYTES:
        raise Refusal(f"required lifecycle source is not one bounded regular file: {relative}")
    try:
        return path.read_bytes()
    except OSError as error:
        raise Refusal(f"required lifecycle source cannot be read: {relative}: {error}") from error


def capture_repository_snapshot(repo: Path, paths: Iterable[str]) -> RepositorySnapshot:
    head, tree = git_identity(repo)
    tracked = {
        row.decode("utf-8")
        for row in git_output(repo, ["ls-files", "-z"], "tracked files").split(b"\0")
        if row
    }
    selected = sorted(set(paths))
    missing = set(selected) - tracked
    if missing:
        raise Refusal("modeled lifecycle sources are not tracked: " + ", ".join(sorted(missing)))
    source_sha256 = {relative: sha256(raw_source_bytes(repo, relative)) for relative in selected}
    source_set_sha256 = sha256(
        json.dumps(source_sha256, sort_keys=True, separators=(",", ":")).encode()
    )
    return RepositorySnapshot(str(repo), head, tree, source_sha256, source_set_sha256)


def require_same_snapshot(before: RepositorySnapshot, after: RepositorySnapshot) -> None:
    if before != after:
        raise Refusal("repository HEAD, tree, or modeled source bytes changed during offline preflight")


def read_source(repo: Path, relative: str) -> str:
    data = raw_source_bytes(repo, relative)
    if _ACTIVE_SOURCE_SNAPSHOT is not None:
        expected = _ACTIVE_SOURCE_SNAPSHOT.source_sha256.get(relative)
        if expected is None:
            raise Refusal(f"lifecycle source read escaped the modeled snapshot: {relative}")
        if sha256(data) != expected:
            raise Refusal(f"modeled lifecycle source changed during offline preflight: {relative}")
    try:
        return data.decode("utf-8")
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


def rust_str_const(source: str, name: str, label: str) -> str:
    matches = re.findall(
        rf"(?m)^\s*(?:pub(?:\([a-z]+\))?\s+)?const\s+{re.escape(name)}"
        rf"\s*:\s*&(?:'static\s+)?str\s*=\s*(?:\r?\n\s*)?\"([^\"]*)\"\s*;",
        source,
    )
    if len(matches) != 1 or not matches[0]:
        raise Refusal(f"{label} must own exactly one non-empty &str {name}")
    return matches[0]


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
    if constants.get("CAMPAIGN_ADMINISTRATION_KEY_ROLES") != (
        "core-upgrade-authority",
        "campaign-payer",
    ):
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


def validate_exposure(
    repo: Path,
    row: CommandExposure,
    main: str,
    dispatch: str,
    main_usage: str,
) -> dict[str, str]:
    if row.dispatch_fragment not in dispatch:
        raise Refusal(f"{row.command} is absent from the successor dispatch function")
    if row.owner_path is not None:
        owner = read_source(repo, row.owner_path)
        if row.owner_fragment not in owner or row.command not in owner:
            raise Refusal(f"{row.command} dispatch constant is absent from {row.owner_path}")
    help_source = main if row.help_path == MAIN else read_source(repo, row.help_path)
    help_body = (
        main_usage
        if row.help_path == MAIN and row.help_function == "usage"
        else rust_function(help_source, row.help_function, row.help_path)
    )
    if row.command not in help_body:
        raise Refusal(f"{row.command} is dispatched but absent from its accepted help function")
    if r"\n+" in help_body:
        raise Refusal(f"{row.command} help contains a literal patch-marker prefix")
    return {
        "command": row.command,
        "dispatch": f"{MAIN}::{row.dispatch_fragment}",
        "help": f"{row.help_path}::{row.help_function}",
    }


def validate_exposures(repo: Path, constants: dict[str, Any], through: str) -> list[dict[str, str]]:
    required = command_groups(constants, through)
    mapping = {row.command: row for row in EXPOSURES}
    if set(required) - set(mapping):
        raise Refusal("offline preflight has no dispatch/help model for: " + ", ".join(sorted(set(required) - set(mapping))))
    main = read_source(repo, MAIN)
    dispatch = rust_function(main, "run", MAIN)
    main_usage = rust_function(main, "usage", MAIN)
    return [
        validate_exposure(repo, mapping[command], main, dispatch, main_usage)
        for command in required
    ]


def python_schema_derivations(source: str, label: str) -> dict[str, tuple[str, str]]:
    """Which Rust constant each derived schema name in one Python file names.

    Every accepted row is a top-level ``NAME = rust_schema_constant(dir, file,
    CONST)``. An assignment in any other shape -- above all a plain string
    literal -- is not a derivation and is simply absent from this map, which is
    what `validate_derived_schemas` refuses on.
    """
    try:
        tree = ast.parse(source)
    except SyntaxError as error:
        raise Refusal(f"{label} is not valid Python: {error}") from error
    values = python_constants(source)
    found: dict[str, tuple[str, str]] = {}
    for row in tree.body:
        if not (
            isinstance(row, ast.Assign)
            and len(row.targets) == 1
            and isinstance(row.targets[0], ast.Name)
            and isinstance(row.value, ast.Call)
            and isinstance(row.value.func, ast.Name)
            and row.value.func.id == "rust_schema_constant"
        ):
            continue
        if row.value.keywords or len(row.value.args) != 3:
            raise Refusal(
                f"{label} {row.targets[0].id} calls rust_schema_constant with an "
                "unreadable argument list"
            )
        try:
            directory, file_name, constant = (
                _python_value(argument, values) for argument in row.value.args
            )
        except (KeyError, TypeError, ValueError) as error:
            raise Refusal(
                f"{label} {row.targets[0].id} names a schema owner this reader "
                f"cannot resolve: {error}"
            ) from error
        if not all(isinstance(part, str) and part for part in (directory, file_name, constant)):
            raise Refusal(f"{label} {row.targets[0].id} names an empty schema owner")
        found[row.targets[0].id] = (f"{directory}/{file_name}", constant)
    return found


def validate_derived_schemas(
    repo: Path,
    source: str,
    owners: tuple[tuple[str, str], ...],
    label: str,
    constant_key: str,
) -> list[dict[str, str]]:
    """One Python file reads each shared schema from the Rust; check the wiring.

    It used to compare a Python copy of the string against its owner file, and
    that is the check whose absence of a copy now makes it unnecessary: the value
    has one author. What is left to go wrong is the WIRING -- a reader pointed at
    a file that is not the semantic owner, the owner declaring the constant twice
    or not at all, or a derivation quietly reverting to a literal -- and each of
    those is refused here by name.
    """
    derivations = python_schema_derivations(source, label)
    report = []
    for name, owner in owners:
        declared = derivations.get(name)
        if declared is None:
            raise Refusal(
                f"{label} {name} is not read from its semantic owner {owner}; a "
                "restated schema string is a second author for a value that has one"
            )
        declared_owner, constant = declared
        if declared_owner != owner:
            raise Refusal(
                f"{label} {name} reads {declared_owner}, not semantic owner {owner}"
            )
        value = rust_str_const(read_source(repo, owner), constant, owner)
        if not value.startswith("dclutch-"):
            raise Refusal(f"{owner} {constant} is not a dclutch schema string")
        report.append(
            {
                constant_key: name,
                "schema": value,
                "owner": owner,
                "owner_constant": constant,
            }
        )
    return report


def validate_schemas(repo: Path, runner_source: str) -> list[dict[str, str]]:
    return validate_derived_schemas(
        repo, runner_source, SCHEMA_OWNERS, "runner", "runner_constant"
    )


def validate_chaos_contract_schemas(repo: Path) -> list[dict[str, str]]:
    """The chaos contract is the WRITER, and it derives too.

    Everything on `SCHEMA_OWNERS` is a name the runner uses to read a session
    back. These two are the names a session is written UNDER, by `chaos.py`, and
    they were Python literals until the same sweep that landed this check --
    which is why the runner and the contract could have disagreed about what the
    file on disk is called with nothing anywhere going red.
    """
    return validate_derived_schemas(
        repo,
        read_source(repo, CHAOS_CONTRACT),
        CHAOS_CONTRACT_SCHEMA_OWNERS,
        "chaos contract",
        "contract_constant",
    )


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
            "authenticate_aggregate_retirement_conservation_receipt_v1",
            "project_aggregate_retirement_completion",
            "AggregateRetirementOperationV1::ORDERED",
            "retirement activity requires four AggregateRetirement events",
            '"_deriveFinalizedLamports": true',
        ),
        "private activity projection",
    )
    return {"private": list(activity), "chaos": list(chaos), "public": list(manifest)}


def validate_source_abort_recovery(repo: Path, constants: dict[str, Any]) -> dict[str, Any]:
    if SOURCE_ABORT_EXPOSURE.command in command_groups(constants, "full") or any(
        SOURCE_ABORT_EXPOSURE.command in stage.commands for stage in STAGES
    ):
        raise Refusal("SourceAbort recovery was incorrectly admitted as a happy-path stage")
    main = read_source(repo, MAIN)
    exposure = validate_exposure(
        repo,
        SOURCE_ABORT_EXPOSURE,
        main,
        rust_function(main, "run", MAIN),
        rust_function(main, "usage", MAIN),
    )
    market = read_source(repo, f"{SUCCESSOR}/market.rs")
    operations = (
        "source-abort-custody-v1",
        "source-abort-controller-first-v1",
        "source-abort-controller-terminal-v1",
    )
    require_fragments(
        market,
        (
            "pub(crate) enum SourceAbortRecoveryOperationV1",
            "pub(crate) const ORDERED: [Self; 3]",
            *operations,
            "SourceAbortRecoveryPhaseV1::Complete",
            "SourceAbort refuses while the staged founding remains satisfiable",
        ),
        "SourceAbort semantic owner",
    )
    exterior = read_source(repo, SOURCE_ABORT_EXPOSURE.help_path)
    require_fragments(
        exterior,
        (
            'const EVIDENCE_SCHEMA_V1: &str = "dclutch-source-abort-exterior-evidence-v1"',
            'const COMPLETION_SCHEMA_V1: &str = "dclutch-source-abort-completion-v1"',
            "SourceAbortRecoveryOperationV1::ORDERED.len()",
            "SourceAbort completion requires three distinct finalized receipts",
            "SourceAbort journals were not one exact canonical adjacent prefix",
            "finalized SourceAbort packet did not produce its exact successor phase",
            "incomplete conservation",
        ),
        "SourceAbort exterior",
    )
    return {
        "happy_path_stage": False,
        "command_exposure": exposure,
        "evidence_schema": "dclutch-source-abort-exterior-evidence-v1",
        "completion_schema": "dclutch-source-abort-completion-v1",
        "operations": list(operations),
        "terminal_phase": "complete",
    }


def derive_economic_fixture(repo: Path, relative: str) -> tuple[dict[str, Any], dict[str, Any]]:
    fixture_source = read_source(repo, relative)
    try:
        fixture = json.loads(fixture_source)
    except json.JSONDecodeError as error:
        raise Refusal(f"economic semantic-owner fixture is not JSON: {relative}: {error}") from error
    environment = os.environ.copy()
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    try:
        result = subprocess.run(
            [sys.executable, str(repo / ECONOMIC_LEDGER), "derive", str(repo / relative)],
            cwd=repo,
            check=False,
            capture_output=True,
            text=True,
            timeout=20,
            env=environment,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Refusal(f"economic semantic owner could not derive {relative}: {error}") from error
    if result.returncode != 0:
        raise Refusal(
            f"economic semantic owner refused {relative}: {result.stderr.strip()}"
        )
    try:
        derived = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise Refusal(f"economic semantic owner emitted non-JSON for {relative}") from error
    canonical_fixture = (json.dumps(fixture, sort_keys=True, separators=(",", ":")) + "\n").encode()
    if derived.get("schema") != "dclutch-exact-economic-lifecycle-ledger-v1" or derived.get(
        "fixtureSha256"
    ) != sha256(canonical_fixture):
        raise Refusal("economic semantic owner changed its output schema or fixture digest")
    snapshots = derived.get("stageSnapshots")
    if not isinstance(snapshots, list) or not snapshots:
        raise Refusal("economic semantic owner omitted its stage snapshots")
    for row in snapshots:
        snapshot = row.get("snapshot") if isinstance(row, dict) else None
        invariants = snapshot.get("invariants") if isinstance(snapshot, dict) else None
        if not isinstance(invariants, dict) or not invariants or set(invariants.values()) != {True}:
            raise Refusal("economic semantic owner did not prove every snapshot invariant")
    terminal = snapshots[-1]
    terminal_snapshot = terminal.get("snapshot") if isinstance(terminal, dict) else None
    terminal_stage = terminal.get("stage") if isinstance(terminal, dict) else None
    positions = terminal_snapshot.get("positions") if isinstance(terminal_snapshot, dict) else None
    aggregate = (
        terminal_snapshot.get("claimAggregateSupplyAtoms")
        if isinstance(terminal_snapshot, dict)
        else None
    )
    if (
        terminal_stage != "aggregate-retirement"
        or not isinstance(terminal_snapshot, dict)
        or terminal_snapshot.get("retired") is not True
        or terminal_snapshot.get("hoardPrincipalAtoms") != "0"
        or not isinstance(aggregate, list)
        or not aggregate
        or set(aggregate) != {"0"}
        or not isinstance(positions, dict)
        or not positions
        or any(not isinstance(row, list) or not row or set(row) != {"0"} for row in positions.values())
    ):
        raise Refusal("economic semantic owner did not discharge every liability before retirement")
    return fixture, derived


def validate_economic_owner(repo: Path, constants: dict[str, Any]) -> dict[str, Any]:
    read_source(repo, ECONOMIC_LEDGER)
    private_fixture, private = derive_economic_fixture(repo, PRIVATE_ECONOMIC_FIXTURE)
    activity_fixture, activity = derive_economic_fixture(repo, ACTIVITY_V3_ECONOMIC_FIXTURE)
    participant_events = [
        event
        for stage in private_fixture.get("stages", [])
        if stage.get("id") == "participant"
        for event in stage.get("events", [])
        if event.get("kind") == "transfer-collateral"
    ]
    direct_events = [
        event
        for stage in private_fixture.get("stages", [])
        for event in stage.get("events", [])
        if event.get("kind") == "direct"
    ]
    funding = private.get("lamportContract", {}).get("requiredFundingTransfers")
    if (
        len(participant_events) != 1
        or participant_events[0].get("quantityAtoms")
        != str(constants["PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS"])
        or not direct_events
        or any(
            event.get("feeBasisPoints") != constants["DEVELOPMENT_FEE_BASIS_POINTS"]
            or event.get("feeDenominator")
            != str(constants["FEE_BASIS_POINTS_DENOMINATOR"])
            for event in direct_events
        )
        or not isinstance(funding, list)
        or len(funding) != 1
        or funding[0].get("lamports") != str(constants["LOCAL_TEST_BANKROLL_LAMPORTS"])
    ):
        raise Refusal("PRIVATE runner constants differ from the economic semantic owner")
    resolution = next(
        (row["snapshot"] for row in private["stageSnapshots"] if row["stage"] == "resolution"),
        None,
    )
    schedule = resolution.get("frozenPayoutSchedule") if isinstance(resolution, dict) else None
    payouts = resolution.get("payoutAtomsPerClaim") if isinstance(resolution, dict) else None
    if (
        not isinstance(schedule, list)
        or not schedule
        or not isinstance(payouts, list)
        or not payouts
    ):
        raise Refusal("PRIVATE economic owner omitted its frozen zero-payout burn schedule")
    losing_burn = False
    for row in schedule:
        index = row.get("claimIndex") if isinstance(row, dict) else None
        if not isinstance(index, int) or isinstance(index, bool) or not 0 <= index < len(payouts):
            raise Refusal("PRIVATE economic owner emitted a bad payout claim coordinate")
        losing_burn |= payouts[index] == "0"
    if not losing_burn:
        raise Refusal("PRIVATE economic owner omitted its frozen zero-payout burn schedule")
    authority = activity.get("activityV3Authority")
    read_source(repo, ACTIVITY_V3_OWNER)
    if (
        private.get("activityV3Authority") is not None
        or not isinstance(authority, dict)
        or authority.get("clusterTarget") != "devnet"
        or authority.get("allLifecycleMutationsExpected") is not True
        or activity_fixture.get("activityV3Authority") is None
    ):
        raise Refusal("corrected Activity-v3 economic authority is absent or misclassified")
    return {
        "private": {
            "fixture": PRIVATE_ECONOMIC_FIXTURE,
            "fixture_sha256": private["fixtureSha256"],
            "stages": [row["stage"] for row in private["stageSnapshots"]],
            "frozen_payout_rows": len(schedule),
            "terminal_liabilities_zero": True,
        },
        "activity_v3": {
            "fixture": ACTIVITY_V3_ECONOMIC_FIXTURE,
            "fixture_sha256": activity["fixtureSha256"],
            "classification": authority.get("classification"),
            "stages": [row["stage"] for row in activity["stageSnapshots"]],
            "terminal_liabilities_zero": True,
        },
    }


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
            # The single `expected_frame` equality became a TWO-frame admission
            # when the price-gated founding arm landed: a bare DCLTGMF3 frame,
            # or the price-gate frame built from
            # GENERIC_MARKET_FOUNDING_PRICE_GATE_FIXED_ACCOUNTS_V4. Both halves
            # are named here rather than one, so this fragment is now strictly
            # more specific than the predicate it replaces -- deleting either
            # arm re-reds the gate, and the old single-frame spelling could not
            # have detected the gated arm going missing at all.
            "let gated = instruction.accounts.len() == gated_frame;",
            "instruction.accounts.len() != bare_frame && !gated",
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
    private_activity = read_source(repo, f"{SUCCESSOR}/private_activity.rs")
    require_fragments(
        private_activity,
        (
            f"found the Market atomically: Lock, Found, Realize, Claims, Open ({magic})",
            f"create {magic} routing address lookup table",
            f"extend {magic} routing table page ",
            f"DCLTCFQ1 -> DCLTPCB2 -> {magic} -> CreateFund -> Activate -> Accept",
        ),
        "private activity founding vocabulary",
    )
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
            "evidence.wire_bytes != 1_167",
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
    # THE GATE MOVED, AND THIS READER DID NOT. `d376896db` made
    # `authenticate_zero_claims_v1` a workspace crate so the browser could reach
    # it, and left `terminal_lifecycle.rs` holding a `use` of it. Reading the
    # predicate where it no longer lives refused EVERY preflight before any
    # other gate ran, so twelve of this suite's cases -- nine of which assert a
    # different refusal entirely -- reported this one. So: read it at its owner,
    # and keep a second predicate on the successor's delegation, because a
    # re-fork of the gate is exactly what the first predicate stopped catching.
    zero_claims_owner = read_source(repo, ZERO_CLAIMS_OWNER)
    require_fragments(
        zero_claims_owner,
        ("Claims supply at index {claim_index} is {supply}", "produce and execute wallet terminal payouts first"),
        "retirement supply-zero gate",
    )
    require_fragments(
        terminal_lifecycle,
        ("dclutch_wallet_terminal_input_operator::authenticate_zero_claims_v1",),
        "retirement supply-zero delegation",
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
        "direct_hot": {"static": 4, "loaded": 57, "unique": 61, "wire_bytes": 1_167, "poststates": 10},
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


def run_preflight(
    repo: Path,
    through: str,
    *,
    _stability_hook: Callable[[], None] | None = None,
) -> dict[str, Any]:
    global _ACTIVE_SOURCE_SNAPSHOT
    repo = exact_repo(repo)
    if through not in ("participant", "full-probe", "full"):
        raise Refusal("--through must be participant, full-probe, or full")
    if _ACTIVE_SOURCE_SNAPSHOT is not None:
        raise Refusal("offline preflight may not nest source snapshots")
    paths = modeled_source_paths()
    before = capture_repository_snapshot(repo, paths)
    try:
        executed_preflight_sha256 = sha256(Path(__file__).resolve(strict=True).read_bytes())
    except OSError as error:
        raise Refusal(f"cannot bind the executing offline preflight: {error}") from error
    if executed_preflight_sha256 != before.source_sha256[PREFLIGHT]:
        raise Refusal("executing offline preflight bytes differ from the clean target repository")
    _ACTIVE_SOURCE_SNAPSHOT = before
    try:
        runner_source = read_source(repo, RUNNER)
        constants = python_constants(runner_source)
        constant_report = validate_constants(constants)
        exposures = validate_exposures(repo, constants, through)
        schemas = validate_schemas(repo, runner_source)
        chaos_schemas = validate_chaos_contract_schemas(repo)
        vocabulary = validate_stage_vocabulary(repo)
        founding = validate_founding_geometry(repo, constants)
        geometry = validate_geometry_and_state(repo)
        recovery = validate_source_abort_recovery(repo, constants)
        economics = validate_economic_owner(repo, constants)
        validate_runner_source_predicates(repo)
        selected_stages = (
            [
                row
                for row in STAGES
                if row.stage in ("prepare", "funding", "administration", "founding", "participant")
            ]
            if through == "participant"
            else list(STAGES)
        )
        if _stability_hook is not None:
            _stability_hook()
        after = capture_repository_snapshot(repo, paths)
        require_same_snapshot(before, after)
        report: dict[str, Any] = {
            "schema": SCHEMA,
            "status": "accepted",
            "evidence_level": "offline-clean-committed-source-contract-only",
            "through": through,
            "validator_started": False,
            "rpc_used": False,
            "keys_read": False,
            "build_run": False,
            "repository": {
                "head": before.head,
                "tree": before.tree,
                "source_set_sha256": before.source_set_sha256,
            },
            "command_exposures": exposures,
            "recovery_exposure": recovery,
            "schema_handoffs": schemas,
            "chaos_contract_schemas": chaos_schemas,
            "stage_vocabulary": vocabulary,
            "constants": constant_report,
            "economic_owner": economics,
            "founding_geometry": founding,
            "transaction_geometry": geometry,
            "expected_execution": [dataclasses.asdict(row) for row in selected_stages],
            "source_sha256": before.source_sha256,
        }
        canonical = json.dumps(report, sort_keys=True, separators=(",", ":")).encode()
        report["model_sha256"] = sha256(canonical)
        return report
    finally:
        _ACTIVE_SOURCE_SNAPSHOT = None


def write_new(path: Path, report: dict[str, Any], repo: Path) -> None:
    repo = exact_repo(repo)
    repository = report.get("repository")
    source_sha256 = report.get("source_sha256")
    if not isinstance(repository, dict) or not isinstance(source_sha256, dict):
        raise Refusal("offline preflight report omitted its repository snapshot")
    expected = RepositorySnapshot(
        str(repo),
        repository.get("head"),
        repository.get("tree"),
        source_sha256,
        repository.get("source_set_sha256"),
    )
    current = capture_repository_snapshot(repo, source_sha256)
    require_same_snapshot(expected, current)
    if not path.is_absolute():
        raise Refusal("--output must be absolute")
    try:
        path.relative_to(repo)
    except ValueError:
        pass
    else:
        raise Refusal("--output must remain outside the clean source repository")
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
        write_new(arguments.output, report, arguments.repo)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except Refusal as error:
        print(f"private-lifecycle-preflight: REFUSED: {error}", file=sys.stderr)
        raise SystemExit(1)
