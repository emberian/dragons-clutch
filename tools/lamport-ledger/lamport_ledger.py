#!/usr/bin/env python3
"""The whole-market lamport statement: where every lamport came from and went.

This is deliberately NOT an eighth conservation law. The protocol already has
seven (`tools/gauntlet/journey/src/ledger.rs`), and the lamport one -- L7,
`payer_delta + fees + watched_growth == 0` -- is a DELTA law over a WATCHED SET
identified by LABEL. That shape forces it to abstain exactly where a founding
lives:

  * `tools/gauntlet/journey/src/ledger.rs:655-670` reports L7 `inapplicable`
    whenever a boundary admits a label the previous census did not watch, and a
    founding is nothing but account admission;
  * `tools/gauntlet/journey/src/journey.rs:124` marks the whole
    "founding through Open" boundary inapplicable on purpose;
  * `tools/local-validator/bootstrap/successor/src/main.rs:804` -- the only
    EXTERNAL verifier of a real founded market -- hardcodes `inapplicable`
    because it "refuses to guess their fees";
  * all five claims in `tools/gauntlet/relayed-vertical/src/vertical.rs`
    (329, 559, 968, 1027, 1179) are `inapplicable`.

So no lamport law has ever been evaluated over a founding. This tool closes that
by changing the shape rather than the arithmetic. A label has no predecessor
balance; an ADDRESS always does, because a nonexistent account holds zero, and
that is a fact rather than a guess. Stating the identity over addresses in a
closure makes it total: nothing needs to abstain.

DESIGN RULE (the day's principle). This tool DERIVES; it never keeps its own
copy of a fact. Every number it prints carries the evidence it came from -- a
journal path plus JSON pointer, or `chain:<address>@<slot>`. Where two sources
state the same fact, it cross-checks them and reports the disagreement instead
of choosing a favourite.

WHAT IT READS
  journals  a `runs/seed-01` run root produced by
            `tools/release/private-validator-lifecycle/run.py`
  chain     finalized account state over loopback JSON-RPC, from a validator
            resumed on the run's own preserved ledger
            (`tools/gauntlet/frontend/resume-validator.sh`)
  universe  optionally, the complete address set of the cluster, from
            `agave-ledger-tool accounts --no-account-data --include-sysvars`

WHAT IT REFUSES TO DO
  It never invents a `class` to make a total balance. An account it cannot
  attribute is reported as an `unclassified` row with its address, owner and
  balance, because "an account exists that no flow class claims" is a finding
  and a silently-absorbed residual is not.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import sys
import urllib.error
import urllib.request
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

STATEMENT_SCHEMA = "dclutch-lamport-statement-v1"

# `tools/economic-lifecycle-ledger/ledger.py:24`. Emitting this shape is how
# this tool hands its DERIVED trace to the repo's existing PREDICTIVE oracle,
# which never reads RPC and so could only ever be fed by hand until now.
LAMPORT_TRACE_SCHEMA = "dclutch-exact-lamport-trace-v1"

MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"

SYSTEM_PROGRAM = "11111111111111111111111111111111"
NATIVE_LOADER = "NativeLoader1111111111111111111111111111111"
SYSVAR_OWNER = "Sysvar1111111111111111111111111111111111111"
FEATURE_OWNER = "Feature111111111111111111111111111111111111"
VOTE_PROGRAM = "Vote111111111111111111111111111111111111111"
STAKE_PROGRAM = "Stake11111111111111111111111111111111111111"
CONFIG_PROGRAM = "Config1111111111111111111111111111111111111"
ALT_PROGRAM = "AddressLookupTab1e1111111111111111111111111"
TOKEN_PROGRAM = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
TOKEN_2022_PROGRAM = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
LOADER_V3 = "BPFLoaderUpgradeab1e11111111111111111111111"
LOADER_V2 = "BPFLoader2111111111111111111111111111111111"

# Half of every transaction fee is burned and half credits the leader
# (measured, not assumed: a vote transaction on a resumed test validator charges
# 10,000 and the block's `Fee` reward credits the identity 5,000). On an idle
# resumed ledger the only traffic is one vote per slot, so the cluster's total
# supply falls by exactly this much per voted slot.
VOTE_BURN_PER_SLOT = 5_000

CLUSTER_OWNERS = {
    NATIVE_LOADER: "cluster.native-program",
    SYSVAR_OWNER: "cluster.sysvar",
    FEATURE_OWNER: "cluster.feature-gate",
    VOTE_PROGRAM: "cluster.consensus-vote",
    STAKE_PROGRAM: "cluster.consensus-stake",
    CONFIG_PROGRAM: "cluster.config",
}

# Each protocol role program, and the flow-class family its accounts belong to.
# The mapping from role name to program id is NEVER hardcoded here: it is read
# off the run's own `founding.json.roles`, so a redeployed cohort cannot make
# this tool quietly classify against stale ids.
ROLE_CLASS = {
    "registry": "market.rent.registry-record",
    "core": "market.rent.core-state",
    "claims": "market.rent.claims-ledger",
    "custody": "market.rent.custody",
    "trading": "market.rent.trading",
    "resolution": "market.rent.resolution",
    "rent-credit": "market.rent.lifecycle-credit",
}

# Flow classes that hold lamports which NO route can ever return. Read off the
# dispatchers, not off prose: the Registry dispatcher
# (`programs/dclutch-registry-sbf/src/lib.rs:183-188`) routes only ActivateRole
# and Reauthenticate, and Core's infrastructure profile
# (`programs/dclutch-core-sbf/src/infrastructure.rs:524-566`) has no close route
# at all. Any claim that "all protocol rent is recoverable" is false, and this
# tool separates terminal holdings from refundable ones so a reader can see it.
TERMINAL_HOLDING_NOTE = {
    "market.rent.registry-record": (
        "immutable records: raw-record rent is locked for the life of the "
        "artifact (programs/dclutch-registry-sbf/src/record_v1.rs:284,647); only "
        "the staging cursor closes (:432 -> :593-628)"
    ),
}


def fail(message: str) -> "NoReturn":  # type: ignore[valid-type]
    print(f"refusal: {message}", file=sys.stderr)
    raise SystemExit(2)


# --------------------------------------------------------------------------
# Sourced quantities. A number without a provenance is not admissible here.
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class Sourced:
    """One integer quantity and the exact evidence that states it."""

    lamports: int
    source: str

    def __post_init__(self) -> None:
        if not isinstance(self.lamports, int) or isinstance(self.lamports, bool):
            fail(f"quantity from {self.source} is not an integer")
        if not self.source:
            fail("a quantity was offered with no source")


@dataclass
class AccountRow:
    """One account in the closure, as the chain reports it, once classified."""

    address: str
    lamports: int
    owner: str
    data_len: int
    executable: bool
    slot: int
    flow_class: str = "unclassified"
    label: str | None = None
    label_source: str | None = None

    @property
    def source(self) -> str:
        return f"chain:{self.address}@{self.slot}"

    def as_json(self) -> dict[str, Any]:
        out = {
            "address": self.address,
            "lamports": str(self.lamports),
            "owner": self.owner,
            "dataLen": self.data_len,
            "executable": self.executable,
            "flowClass": self.flow_class,
            "source": self.source,
        }
        if self.label:
            out["label"] = self.label
            out["labelSource"] = self.label_source
        return out


@dataclass
class FeeEvent:
    """One transaction's fee, as the chain charged it, and who paid it.

    `payer` is load-bearing and was the first thing this tool got wrong. A run
    has MORE THAN ONE fee payer: the administration stage's transactions are
    paid by the deployer / upgrade authority, and the founding's by the
    campaign payer that the bankroll transfer funded. Summing both against one
    opening balance silently misattributes every administration fee.
    """

    signature: str
    slot: int | None
    lamports: int
    label: str
    stage: str
    errored: bool
    source: str
    payer: str


@dataclass
class Divergence:
    """A gap the statement refuses to absorb, named by class and accounts."""

    kind: str
    lamports: int
    explanation: str
    accounts: list[str] = field(default_factory=list)

    def as_json(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "lamports": str(self.lamports),
            "explanation": self.explanation,
            "accounts": self.accounts,
        }


# --------------------------------------------------------------------------
# Strict evidence parsing
# --------------------------------------------------------------------------


def read_json(path: Path, what: str) -> Any:
    if not path.is_file():
        fail(f"{what} is not a file: {path}")
    try:
        return json.loads(path.read_bytes())
    except json.JSONDecodeError as error:
        fail(f"{what} is not valid JSON ({path}): {error}")


def integer(value: Any, where: str) -> int:
    """Accept the two shapes this evidence family uses, and nothing else.

    `execution.transactions[].fee_lamports` is a JSON number;
    `provisioning-poststate.json` states every lamport quantity as canonical
    decimal TEXT so that no reader can round it. Both are exact; a float is
    not, and is refused rather than coerced.
    """
    if isinstance(value, bool):
        fail(f"{where} is a boolean, not a lamport quantity")
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        if not re.fullmatch(r"(0|[1-9][0-9]*)", value):
            fail(f"{where} is not canonical decimal text: {value!r}")
        return int(value)
    fail(f"{where} is {type(value).__name__}, not an exact lamport quantity")


@dataclass
class Evidence:
    """Everything the run's own journals state, with paths kept for citation."""

    run_root: Path
    payer: str
    roles: dict[str, str]
    genesis_accounts: dict[str, int]
    genesis_hash: str | None
    opening: Sourced | None
    funding_source: str | None
    fees: list[FeeEvent]
    named_accounts: dict[str, tuple[str, str]]  # address -> (label, source)
    journal_lamports: dict[str, tuple[int, str]]  # address -> (lamports, source)
    stage_payers: dict[str, str] = field(default_factory=dict)
    harvested: set[str] = field(default_factory=set)
    source_opening: Sourced | None = None
    # Submission journals the run never saw finalize. A journal in `submitted`
    # carries a null fee because the driver never read the transaction's
    # metadata -- but the chain charged for it all the same if it landed. These
    # are the first suspects whenever a funder is poorer than its accounts
    # explain. Captured at load time: this tool reads its evidence once.
    unfinalized: list[dict[str, Any]] = field(default_factory=list)

    @property
    def total_fees(self) -> int:
        return sum(event.lamports for event in self.fees)

    def fees_paid_by(self, payer: str) -> int:
        return sum(event.lamports for event in self.fees if event.payer == payer)

    def fees_by_payer(self) -> dict[str, int]:
        out: dict[str, int] = defaultdict(int)
        for event in self.fees:
            out[event.payer] += event.lamports
        return dict(out)


STAGE_TX_LINE = re.compile(
    rb"campaign transaction: slot=(\d+) fee=(\d+) compute_units=(\d+) (.*)"
)

BASE58 = re.compile(r"^[1-9A-HJ-NP-Za-km-z]{32,44}$")
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

# Fixtures that exist on every cluster and were never created by any campaign.
# The wrapped-SOL native mint is the one that matters here: it is Token-owned
# and 82 bytes, so it looks exactly like a campaign-created collateral mint, and
# counting its lamports as campaign rent overstates the founding by a round
# 1,000,000,000 -- a number seductive enough to be mistaken for a real balance.
CLUSTER_FIXTURES = {
    "So11111111111111111111111111111111111111112": "wrapped-SOL native mint",
}


def looks_like_pubkey(value: str) -> bool:
    if not BASE58.fullmatch(value):
        return False
    number = 0
    for character in value:
        index = BASE58_ALPHABET.find(character)
        if index < 0:
            return False
        number = number * 58 + index
    return number.bit_length() <= 256


def harvest_addresses(node: Any, into: set[str]) -> None:
    """Every address the evidence names, anywhere, at any depth.

    The closure is DERIVED rather than declared. A hand-listed set of accounts
    is how a statement misses the interesting ones -- the founding leaves rent
    in wallets that appear only as a `vacantProtocolRoles` entry or a bare
    string in a poststate, and those are exactly the accounts a leak would hide
    in. Anything that decodes as a 32-byte key is queried; the chain then says
    whether it exists.
    """
    if isinstance(node, str):
        if looks_like_pubkey(node):
            into.add(node)
    elif isinstance(node, dict):
        for value in node.values():
            harvest_addresses(value, into)
    elif isinstance(node, list):
        for value in node:
            harvest_addresses(value, into)


def stage_log_fees(run_root: Path, stage: str, payer: str) -> list[FeeEvent]:
    """Recover per-transaction fees from a stage's own stderr.

    The driver prints `campaign transaction: slot=N fee=N compute_units=N LABEL`
    for every transaction as it lands, including transactions from stages a
    later resume skipped. The line carries no signature, so these events can be
    summed but not joined to a journal; the statement says which of the two
    sources it used rather than blending them silently.
    """
    stages_dir = run_root / "stages"
    if not stages_dir.is_dir():
        return []
    events: list[FeeEvent] = []
    for directory in sorted(stages_dir.iterdir()):
        if not directory.is_dir() or stage not in directory.name:
            continue
        log = directory / "stderr.bin"
        if not log.is_file():
            continue
        for index, match in enumerate(STAGE_TX_LINE.finditer(log.read_bytes())):
            slot, fee, _cu, label = match.groups()
            events.append(
                FeeEvent(
                    signature=f"<unsigned:{directory.name}:{index}>",
                    slot=int(slot),
                    lamports=int(fee),
                    label=label.decode(errors="replace").strip(),
                    stage=stage,
                    errored=False,
                    source=f"{log}#line-match[{index}]",
                    payer=payer,
                )
            )
    return events


def load_evidence(run_root: Path) -> Evidence:
    founding_path = run_root / "founding.json"
    founding = read_json(founding_path, "the founding campaign report")

    roles_raw = founding.get("roles")
    if not isinstance(roles_raw, list) or not roles_raw:
        fail(f"{founding_path} has no `roles` list; cannot name the role programs")
    roles: dict[str, str] = {}
    for entry in roles_raw:
        role, program = entry.get("role"), entry.get("program_id")
        if not role or not program:
            fail(f"{founding_path} `roles` entry lacks role/program_id")
        roles[role] = program

    # The campaign payer, in descending order of directness. A run that never
    # populated `execution` still names its payer on every submission journal,
    # and that is the same key the bankroll transfer funded.
    payer = founding.get("payer")
    if not payer:
        for journal in founding.get("foundingSubmissionJournals") or []:
            if journal.get("payer"):
                payer = journal["payer"]
                break
    if not payer:
        fail(
            f"{founding_path} names no payer, and no submission journal carries "
            "one either; without a payer there is no identity to state"
        )

    plan_path = run_root / "mutable" / "plan.json"
    plan = read_json(plan_path, "the infrastructure plan")
    genesis_accounts: dict[str, int] = {}
    for key, entry in (plan.get("genesis_accounts") or {}).items():
        address = entry.get("address")
        if not address:
            fail(f"{plan_path} genesis_accounts[{key}] has no address")
        genesis_accounts[address] = integer(
            entry.get("lamports", 0), f"{plan_path}#genesis_accounts.{key}.lamports"
        )

    # The genesis transfer is the ONE external inflow: the whole campaign's
    # lamports enter here and nowhere else.
    opening: Sourced | None = None
    source_opening: Sourced | None = None
    funding_source: str | None = None
    harvested: set[str] = set()
    provisioning_path = run_root / "provisioning-poststate.json"
    if provisioning_path.is_file():
        provisioning = read_json(provisioning_path, "the bankroll poststate")
        harvest_addresses(provisioning, harvested)
        poststate = provisioning.get("poststate") or {}
        if "campaignPayerLamports" in poststate:
            opening = Sourced(
                integer(
                    poststate["campaignPayerLamports"],
                    f"{provisioning_path}#poststate.campaignPayerLamports",
                ),
                f"{provisioning_path}#poststate.campaignPayerLamports",
            )
        # The funding source is a payer too. It pays for the administration
        # stage, so its own balance movement is a second identity -- and on this
        # run it is the SAME key as the genesis mint, which is why an
        # unsuspecting statement books its spending against nobody.
        if "sourceLamports" in poststate:
            source_opening = Sourced(
                integer(
                    poststate["sourceLamports"],
                    f"{provisioning_path}#poststate.sourceLamports",
                ),
                f"{provisioning_path}#poststate.sourceLamports",
            )
        source_entry = provisioning.get("source") or {}
        funding_source = source_entry.get("address")

    fees: list[FeeEvent] = []
    named: dict[str, tuple[str, str]] = {}
    journal_lamports: dict[str, tuple[int, str]] = {}

    stage_payers: dict[str, str] = {}

    for stage, filename in (("administration", "administration.json"), ("founding", "founding.json")):
        path = run_root / filename
        if not path.is_file():
            continue
        document = founding if filename == "founding.json" else read_json(path, f"the {stage} report")
        harvest_addresses(document, harvested)
        execution = document.get("execution") or {}

        # Each stage declares its own fee payer. The administration stage is
        # paid by the deployer / upgrade authority; the founding by the campaign
        # payer. They are different keys and must never be summed together.
        stage_payer = document.get("payer") or payer
        for journal in document.get("foundingSubmissionJournals") or []:
            if journal.get("payer"):
                stage_payer = journal["payer"]
                break
        stage_payers[stage] = stage_payer

        rows = execution.get("transactions") or []
        if not rows:
            # A resumed or recovered run can finish with `execution` empty while
            # the stage's own stderr still holds the complete per-transaction
            # record the driver printed as it went. That log is a SUPERSET of
            # `execution.transactions`, so it is a fallback, never a preference.
            fees.extend(stage_log_fees(run_root, stage, stage_payer))
            continue

        for index, row in enumerate(rows):
            fee = row.get("fee_lamports")
            if fee is None:
                # A transaction whose metadata the chain no longer serves. It is
                # counted as a KNOWN UNKNOWN, never as zero.
                fees.append(
                    FeeEvent(
                        signature=row.get("signature", "<unknown>"),
                        slot=row.get("slot"),
                        lamports=0,
                        label=row.get("label", ""),
                        stage=stage,
                        errored=row.get("error") is not None,
                        source=f"{path}#execution.transactions[{index}] (fee unavailable)",
                        payer=stage_payer,
                    )
                )
                continue
            fees.append(
                FeeEvent(
                    signature=row.get("signature", "<unknown>"),
                    slot=row.get("slot"),
                    lamports=integer(fee, f"{path}#execution.transactions[{index}].fee_lamports"),
                    label=row.get("label", ""),
                    stage=stage,
                    errored=row.get("error") is not None,
                    source=f"{path}#execution.transactions[{index}]",
                    payer=stage_payer,
                )
            )

        # Every account the campaign ever named, with the journal's own view of
        # its balance. This is the tool's cross-check target, not its authority.
        for holder, pointer in (
            (execution.get("market") or {}).get("accounts"), f"{path}#execution.market.accounts",
        ), ((document.get("foundingCheckpoint") or {}).get("accounts"), f"{path}#foundingCheckpoint.accounts"):
            if not isinstance(holder, dict):
                continue
            for label, entry in holder.items():
                address = entry.get("address")
                if not address:
                    continue
                named.setdefault(address, (label, f"{pointer}.{label}"))
                if "lamports" in entry:
                    journal_lamports.setdefault(
                        address,
                        (
                            integer(entry["lamports"], f"{pointer}.{label}.lamports"),
                            f"{pointer}.{label}.lamports",
                        ),
                    )

        for index, journal in enumerate(document.get("foundingSubmissionJournals") or []):
            for jndex, poststate in enumerate(journal.get("finalizedPoststates") or []):
                address = poststate.get("address")
                if not address:
                    continue
                pointer = f"{path}#foundingSubmissionJournals[{index}].finalizedPoststates[{jndex}]"
                operation = journal.get("operation", f"journal-{index}")
                named.setdefault(address, (f"{operation}:poststate", pointer))
                if "lamports" in poststate:
                    journal_lamports.setdefault(
                        address, (integer(poststate["lamports"], pointer + ".lamports"), pointer + ".lamports")
                    )

    return Evidence(
        run_root=run_root,
        payer=payer,
        roles=roles,
        genesis_accounts=genesis_accounts,
        genesis_hash=founding.get("genesis_hash"),
        opening=opening,
        funding_source=funding_source,
        fees=fees,
        named_accounts=named,
        journal_lamports=journal_lamports,
        stage_payers=stage_payers,
        harvested=harvested,
        source_opening=source_opening,
        unfinalized=[
            journal
            for journal in founding.get("foundingSubmissionJournals") or []
            if journal.get("phase") != "finalized"
        ],
    )


# --------------------------------------------------------------------------
# The chain. Read-only, and loopback unless a remote cluster is acknowledged.
# --------------------------------------------------------------------------


class Rpc:
    """A finalized-commitment, read-only JSON-RPC reader."""

    def __init__(self, url: str, allow_remote: bool) -> None:
        loopback = url.startswith("http://127.0.0.1:") or url.startswith("http://localhost:")
        if not loopback and not allow_remote:
            fail(
                f"{url} is not loopback. Pass --allow-remote-rpc to read a real "
                "cluster; this tool never writes, but reading a remote cluster "
                "should be a deliberate act."
            )
        self.url = url
        self.calls = 0

    def call(self, method: str, params: list[Any]) -> Any:
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        request = urllib.request.Request(
            self.url, data=body, headers={"Content-Type": "application/json"}
        )
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                payload = json.load(response)
        except (urllib.error.URLError, TimeoutError) as error:
            fail(f"RPC {method} to {self.url} failed: {error}")
        if "error" in payload:
            fail(f"RPC {method} refused: {payload['error']}")
        self.calls += 1
        return payload["result"]

    def genesis_hash(self) -> str:
        return self.call("getGenesisHash", [])

    def slot(self) -> int:
        return self.call("getSlot", [{"commitment": "finalized"}])

    def accounts(self, addresses: list[str]) -> tuple[int, dict[str, dict[str, Any] | None]]:
        out: dict[str, dict[str, Any] | None] = {}
        slot = 0
        for start in range(0, len(addresses), 100):
            chunk = addresses[start : start + 100]
            result = self.call(
                "getMultipleAccounts",
                [chunk, {"encoding": "base64", "dataSlice": {"offset": 0, "length": 0},
                         "commitment": "finalized"}],
            )
            slot = result["context"]["slot"]
            for address, value in zip(chunk, result["value"]):
                out[address] = value
        return slot, out

    def program_accounts(self, program: str) -> list[tuple[str, dict[str, Any]]]:
        result = self.call(
            "getProgramAccounts",
            [program, {"encoding": "base64", "dataSlice": {"offset": 0, "length": 0},
                       "commitment": "finalized"}],
        )
        return [(item["pubkey"], item["account"]) for item in result]


def universe_addresses(path: Path) -> list[str]:
    """Every address in the cluster, from an `agave-ledger-tool accounts` dump.

    The dump prints balances as FLOAT SOL, which cannot represent a genesis-scale
    account exactly, so only the ADDRESSES are taken from it. Exact lamports
    always come from RPC.
    """
    text = path.read_text(errors="replace")
    found = re.findall(r"^Public Key: (\S+)\s*$", text, flags=re.MULTILINE)
    if not found:
        fail(f"{path} contains no `Public Key:` lines; is it an accounts dump?")
    return found


# --------------------------------------------------------------------------
# Classification
# --------------------------------------------------------------------------


def classify(row: AccountRow, evidence: Evidence, program_ids: dict[str, str]) -> None:
    address, owner = row.address, row.owner

    named = evidence.named_accounts.get(address)
    if named:
        row.label, row.label_source = named

    if address in CLUSTER_FIXTURES:
        row.flow_class = "cluster.fixture"
        row.label = row.label or CLUSTER_FIXTURES[address]
        row.label_source = row.label_source or "lamport_ledger.CLUSTER_FIXTURES"
        return
    if address == evidence.payer:
        row.flow_class = "campaign.payer-residual"
        return
    if evidence.funding_source and address == evidence.funding_source:
        row.flow_class = "cluster.genesis-source"
        return
    if address in evidence.genesis_accounts:
        row.flow_class = "protocol.deployed-program" if row.executable else "protocol.programdata"
        return
    if owner in CLUSTER_OWNERS:
        row.flow_class = CLUSTER_OWNERS[owner]
        return
    if owner in (LOADER_V3, LOADER_V2):
        row.flow_class = "protocol.deployed-program" if row.executable else "protocol.programdata"
        return
    if owner == ALT_PROGRAM:
        # L7's one legitimate escape hatch, made ordinary: an address lookup
        # table's address derives from the slot that created it, so it cannot be
        # watched in advance (tools/gauntlet/journey/src/ledger.rs:96-104). By
        # address rather than label, it needs no exemption.
        row.flow_class = "market.rent.routing-table"
        return
    if owner in (TOKEN_PROGRAM, TOKEN_2022_PROGRAM):
        row.flow_class = "market.collateral-account"
        return
    role = program_ids.get(owner)
    if role:
        row.flow_class = ROLE_CLASS.get(role, f"market.rent.{role}")
        return
    if owner == SYSTEM_PROGRAM:
        # A bare wallet is campaign business only if the run NAMED it. In a
        # whole-cluster closure the validator's own identity account is also a
        # System-owned wallet, and it holds a genesis-scale balance -- counting
        # it as campaign-placed rent overstates a founding by six orders of
        # magnitude. Naming is the discriminator, and the run's journals are
        # where naming lives.
        named_by_run = address in evidence.harvested or address in evidence.named_accounts
        row.flow_class = "wallet.campaign-named" if named_by_run else "cluster.unnamed-wallet"
        return
    row.flow_class = "unclassified"


# --------------------------------------------------------------------------
# The statement
# --------------------------------------------------------------------------


@dataclass
class Statement:
    slot: int
    genesis_hash: str
    evidence: Evidence
    rows: list[AccountRow]
    divergences: list[Divergence] = field(default_factory=list)
    universe_complete: bool = False
    capitalization: int | None = None

    def by_class(self) -> dict[str, list[AccountRow]]:
        grouped: dict[str, list[AccountRow]] = defaultdict(list)
        for row in self.rows:
            grouped[row.flow_class].append(row)
        return dict(grouped)

    @property
    def payer_closing(self) -> int:
        for row in self.rows:
            if row.address == self.evidence.payer:
                return row.lamports
        fail("the payer does not appear in the closure; the chain read is incomplete")

    def balance_of(self, address: str) -> int | None:
        for row in self.rows:
            if row.address == address:
                return row.lamports
        return None

    def rent_implied_by_all_funders(self) -> tuple[int, list[str]]:
        """What every funder's own balance movement says it placed as rent.

        A run has more than one funder. Summing only the campaign payer books
        the administration stage's account creations against nobody and leaves a
        residual that looks like a leak.
        """
        evidence = self.evidence
        lines: list[str] = []
        total = 0
        for who, opening, address in (
            ("campaign payer", evidence.opening, evidence.payer),
            ("funding source", evidence.source_opening, evidence.funding_source),
        ):
            if opening is None or address is None:
                continue
            closing = self.balance_of(address)
            if closing is None:
                continue
            fees = evidence.fees_paid_by(address)
            implied = opening.lamports - closing - fees
            total += implied
            lines.append(
                f"{who} {address}: opened {opening.lamports:,d}, closed "
                f"{closing:,d}, paid {fees:,d} in fees => placed {implied:,d}"
            )
        return total, lines

    @property
    def campaign_created_rent(self) -> int:
        """Lamports sitting in accounts the campaign created.

        The complement of the genesis endowment and the cluster's own furniture:
        everything here was placed by a campaign transaction.
        """
        return sum(
            row.lamports
            for row in self.rows
            if row.flow_class.startswith(("market.", "wallet.campaign-named"))
            and row.address not in self.evidence.genesis_accounts
            and row.address not in CLUSTER_FIXTURES
        )


def build_statement(
    evidence: Evidence, rpc: Rpc, universe: Path | None, capitalization: int | None = None
) -> Statement:
    genesis_hash = rpc.genesis_hash()
    if genesis_hash == MAINNET_GENESIS_HASH:
        fail("that RPC endpoint is MAINNET. This tool never reads mainnet.")
    if evidence.genesis_hash and evidence.genesis_hash != genesis_hash:
        fail(
            "the chain is not the one this run was driven against: journal says "
            f"{evidence.genesis_hash}, endpoint says {genesis_hash}. A statement "
            "joining two different chains would be fiction."
        )

    program_ids = {program: role for role, program in evidence.roles.items()}

    if universe is not None:
        addresses = universe_addresses(universe)
        complete = True
    else:
        # Without a dump, the closure is assembled from what the run NAMED plus
        # everything the protocol's own programs own. That is complete for the
        # market; it is not the whole cluster, and the statement says so.
        seen: set[str] = set()
        for program in list(evidence.roles.values()) + [
            TOKEN_2022_PROGRAM,
            TOKEN_PROGRAM,
            ALT_PROGRAM,
        ]:
            for address, _ in rpc.program_accounts(program):
                seen.add(address)
        seen.update(evidence.named_accounts)
        seen.update(evidence.genesis_accounts)
        seen.update(evidence.harvested)
        seen.add(evidence.payer)
        if evidence.funding_source:
            seen.add(evidence.funding_source)
        addresses = sorted(seen)
        complete = False

    slot, values = rpc.accounts(addresses)
    rows: list[AccountRow] = []
    for address in addresses:
        value = values.get(address)
        if value is None:
            continue  # an address the dump saw and the chain has since closed
        row = AccountRow(
            address=address,
            lamports=value["lamports"],
            owner=value["owner"],
            data_len=value.get("space", 0),
            executable=value.get("executable", False),
            slot=slot,
        )
        classify(row, evidence, program_ids)
        rows.append(row)

    statement = Statement(
        slot=slot,
        genesis_hash=genesis_hash,
        evidence=evidence,
        rows=rows,
        universe_complete=complete,
        capitalization=capitalization,
    )
    detect_divergences(statement)
    return statement


def detect_divergences(statement: Statement) -> None:
    evidence = statement.evidence

    # (1) Accounts nothing claims.
    unclassified = [row for row in statement.rows if row.flow_class == "unclassified"]
    if unclassified:
        statement.divergences.append(
            Divergence(
                kind="unclassified-accounts",
                lamports=sum(row.lamports for row in unclassified),
                explanation=(
                    "these accounts hold lamports and no flow class in the "
                    "inventory claims them; each is either a flow this tool does "
                    "not know about or a program it was not told the role of"
                ),
                accounts=[f"{row.address} owner={row.owner} lamports={row.lamports}" for row in unclassified],
            )
        )

    # (2) The journal's own view of a balance vs the chain's.
    disagreements: list[str] = []
    for row in statement.rows:
        claim = evidence.journal_lamports.get(row.address)
        if claim and claim[0] != row.lamports:
            disagreements.append(
                f"{row.address} journal={claim[0]} chain={row.lamports} "
                f"(journal at {claim[1]})"
            )
    if disagreements:
        statement.divergences.append(
            Divergence(
                kind="journal-vs-chain",
                lamports=0,
                explanation=(
                    "a journal states a balance the chain does not confirm. The "
                    "chain is authoritative; a journal that disagrees was written "
                    "at a different slot or is stale"
                ),
                accounts=disagreements,
            )
        )

    # (3) The payer identity. This is the statement's spine.
    if evidence.opening is not None:
        implied_rent, _lines = statement.rent_implied_by_all_funders()
        observed_rent = statement.campaign_created_rent
        gap = observed_rent - implied_rent
        if gap != 0:
            candidates = [
                f"{row.address} class={row.flow_class} lamports={row.lamports}"
                + (f" label={row.label}" if row.label else "")
                for row in sorted(
                    (r for r in statement.rows if r.flow_class.startswith("market.")),
                    key=lambda r: -r.lamports,
                )[:12]
            ]
            if gap > 0:
                kind = "rent-from-an-unnamed-funder"
                explanation = (
                    f"accounts hold {observed_rent} lamports of campaign-placed "
                    f"rent but the known funders' balances only explain "
                    f"{implied_rent}. Some OTHER key funded the difference, or an "
                    "account counted as campaign-created predates the campaign"
                )
            else:
                # The funders are POORER than the accounts explain. Lamports left
                # them and did not arrive anywhere the closure can see. On a
                # chain the only such destination is a transaction fee -- so this
                # is the campaign's own fee record being INCOMPLETE, which is
                # precisely the failure L7 exists to catch and has never been in
                # a position to.
                kind = "spend-exceeds-observed-holdings"
                explanation = (
                    f"the funders are {-gap} lamports poorer than the accounts "
                    "they created can explain. Lamports left a funder and "
                    "arrived nowhere in the closure; on a chain the only such "
                    "destination is a transaction fee. The campaign's own fee "
                    "record is therefore a LOWER BOUND: it logs a transaction "
                    "when the driver observes it confirm, so a transaction that "
                    "was submitted and never observed is charged by the chain "
                    "and never counted here"
                )
                unfinalized = [
                    f"{journal.get('operation')} phase={journal.get('phase')} "
                    f"feeLamports={journal.get('feeLamports')}"
                    for journal in evidence.unfinalized
                ]
                tiers = sorted({event.lamports for event in evidence.fees if event.lamports})
                candidates = unfinalized + [
                    f"observed fee tiers in this run: {tiers}",
                    f"unaccounted {-gap} lamports is "
                    + (
                        f"{-gap // tiers[0]}-{-gap // tiers[-1]} transactions at those tiers"
                        if tiers
                        else "unattributable without a fee tier"
                    ),
                ]
            statement.divergences.append(
                Divergence(
                    kind=kind, lamports=gap, explanation=explanation, accounts=candidates
                )
            )

    missing_fee = [event for event in evidence.fees if "fee unavailable" in event.source]
    if missing_fee:
        statement.divergences.append(
            Divergence(
                kind="fees-not-stated",
                lamports=0,
                explanation=(
                    f"{len(missing_fee)} transaction(s) carry no fee in the "
                    "journal, so the fee total is a LOWER BOUND, not a total"
                ),
                accounts=[event.signature for event in missing_fee],
            )
        )


# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------


def money(value: int) -> str:
    return f"{value:>20,d}"


def render_text(statement: Statement) -> str:
    evidence = statement.evidence
    out: list[str] = []
    add = out.append

    add("=" * 96)
    add("  THE LAMPORT STATEMENT — where every lamport came from and where it went")
    add("=" * 96)
    add(f"  run       {evidence.run_root}")
    add(f"  chain     genesis {statement.genesis_hash}, read finalized at slot {statement.slot}")
    add(f"  payer     {evidence.payer}")
    add(
        "  closure   "
        + (
            "the whole cluster (every account in the ledger)"
            if statement.universe_complete
            else "the market (protocol-owned + journal-named accounts); pass "
            "--universe for the whole cluster"
        )
    )
    add("")

    add("-" * 96)
    add("  CONSERVATION IDENTITY")
    add("-" * 96)
    if evidence.opening is None:
        add("  INAPPLICABLE: this run states no opening balance, so there is no")
        add("  external inflow to conserve against. It never guesses one.")
    else:
        own_fees = evidence.fees_paid_by(evidence.payer)
        own_count = sum(1 for e in evidence.fees if e.payer == evidence.payer)
        spent = evidence.opening.lamports - statement.payer_closing
        implied = spent - own_fees
        add(f"  payer opening balance          {money(evidence.opening.lamports)}")
        add(f"    less network fees paid        {money(-own_fees)}   ({own_count} transactions)")
        add(f"    less rent placed in accounts  {money(-implied)}")
        add(f"  {'':<30} {'-' * 20}")
        add(f"  = payer closing balance        {money(evidence.opening.lamports - own_fees - implied)}")
        add(f"  observed on chain              {money(statement.payer_closing)}")
        residual = statement.payer_closing - (evidence.opening.lamports - own_fees - implied)
        verdict = "BALANCES to the lamport" if residual == 0 else f"RESIDUAL {residual}"
        add(f"  {'':<30} {verdict}")
        add("    (that line is an ARRANGEMENT of the definition of `rent placed`,")
        add("     not yet a test. The test is below: it asks whether the rent the")
        add("     funders' balances IMPLY is the rent the chain actually HOLDS.)")
        add("")
        add("  THE TEST — implied placement vs observed holdings")
        implied_all, lines = statement.rent_implied_by_all_funders()
        for line in lines:
            add(f"    {line}")
        add(f"  rent implied by every funder's own movement {money(implied_all)}")
        add(f"  rent observed in campaign-created accounts  {money(statement.campaign_created_rent)}")
        difference = statement.campaign_created_rent - implied_all
        add(f"  difference                                 {money(difference)}")
        if difference == 0:
            add("  EVERY LAMPORT THE FUNDERS SPENT IS ACCOUNTED FOR IN A NAMED ACCOUNT.")
    add("")

    add("-" * 96)
    add("  HOLDINGS — every lamport in the closure, by flow class")
    add("-" * 96)
    add(f"  {'flow class':<34} {'n':>5} {'lamports':>20}")
    grouped = statement.by_class()
    total = 0
    for flow_class in sorted(grouped, key=lambda k: -sum(r.lamports for r in grouped[k])):
        rows = grouped[flow_class]
        subtotal = sum(row.lamports for row in rows)
        total += subtotal
        marker = " *" if flow_class in TERMINAL_HOLDING_NOTE else ""
        add(f"  {flow_class:<34} {len(rows):>5} {money(subtotal)}{marker}")
    add(f"  {'-' * 34} {'-' * 5} {'-' * 20}")
    add(f"  {'TOTAL IN CLOSURE':<34} {len(statement.rows):>5} {money(total)}")
    for flow_class, note in TERMINAL_HOLDING_NOTE.items():
        if flow_class in grouped:
            add(f"  * {flow_class}: {note}")
    add("")

    if statement.capitalization is not None:
        add("-" * 96)
        add("  WHOLE-CLUSTER CLOSURE — is the address set complete?")
        add("-" * 96)
        add(f"  sum of every account this statement reads {money(total)}")
        add(f"  capitalization reported by the ledger      {money(statement.capitalization)}")
        drift = statement.capitalization - total
        add(f"  difference                                 {money(drift)}")
        add("  A nonzero difference here is NOT a protocol leak: capitalization is")
        add("  measured at the ledger's own replay slot and this read is later, so")
        add("  the gap is the fee burn of the slots in between (half of each fee,")
        add(f"  {VOTE_BURN_PER_SLOT:,d} lamports per voted slot on a resumed test validator).")
        if drift == 0:
            add("  EXACT: the address set is complete and no slot elapsed between.")
        elif drift > 0 and drift % VOTE_BURN_PER_SLOT == 0:
            add(
                f"  EXACTLY {drift // VOTE_BURN_PER_SLOT:,d} slots of vote burn, to the "
                "lamport: the address set is COMPLETE, and every lamport that left"
            )
            add("  the cluster between the two reads is consensus traffic, not protocol.")
        else:
            add(
                f"  NOT a multiple of {VOTE_BURN_PER_SLOT:,d}: {drift % VOTE_BURN_PER_SLOT:,d} "
                "lamports are unexplained, which means the dump MISSED an address."
            )
        add("")

    add("-" * 96)
    add("  OUTFLOWS — value that left the accounted universe")
    add("-" * 96)
    by_stage: dict[tuple[str, str], list[FeeEvent]] = defaultdict(list)
    for event in evidence.fees:
        by_stage[(event.stage, event.payer)].append(event)
    for (stage, fee_payer), events in sorted(by_stage.items()):
        total_stage = sum(event.lamports for event in events)
        errored = sum(1 for event in events if event.errored)
        role = "campaign payer" if fee_payer == evidence.payer else "OTHER payer"
        add(
            f"  network fees / {stage:<18} {len(events):>5} {money(total_stage)}"
            + (f"   ({errored} refused, and a refused transaction still pays)" if errored else "")
        )
        add(f"    paid by {fee_payer}  [{role}]")
    add(f"  {'-' * 34} {'-' * 5} {'-' * 20}")
    add(f"  {'TOTAL DESTROYED (fees)':<34} {len(evidence.fees):>5} {money(evidence.total_fees)}")
    add("  fees are the only value that leaves: half burns, half credits the")
    add("  leader. No dClutch route records them (inventory hole #1).")
    add("")

    add("-" * 96)
    add("  DIVERGENCES")
    add("-" * 96)
    if not statement.divergences:
        add("  none. Every lamport in the closure is claimed by exactly one flow class.")
    for divergence in statement.divergences:
        add(f"  [{divergence.kind}]  {divergence.lamports:,d} lamports")
        add(f"    {divergence.explanation}")
        for account in divergence.accounts[:14]:
            add(f"      - {account}")
        if len(divergence.accounts) > 14:
            add(f"      ... and {len(divergence.accounts) - 14} more")
        add("")
    return "\n".join(out)


def render_json(statement: Statement) -> dict[str, Any]:
    evidence = statement.evidence
    grouped = statement.by_class()
    identity: dict[str, Any] = {"applicable": evidence.opening is not None}
    if evidence.opening is not None:
        own_fees = evidence.fees_paid_by(evidence.payer)
        spent = evidence.opening.lamports - statement.payer_closing
        implied = spent - own_fees
        identity.update(
            {
                "payer": evidence.payer,
                "payerOpeningLamports": str(evidence.opening.lamports),
                "payerOpeningSource": evidence.opening.source,
                "networkFeesLamports": str(own_fees),
                "networkFeesAllPayersLamports": str(evidence.total_fees),
                "feesByPayer": {k: str(v) for k, v in sorted(evidence.fees_by_payer().items())},
                "rentImpliedByPayerLamports": str(implied),
                "rentObservedInCampaignAccountsLamports": str(statement.campaign_created_rent),
                "payerClosingLamports": str(statement.payer_closing),
                "payerClosingSource": f"chain:{evidence.payer}@{statement.slot}",
                "holds": statement.campaign_created_rent == implied,
            }
        )
    return {
        "schema": STATEMENT_SCHEMA,
        "runRoot": str(evidence.run_root),
        "genesisHash": statement.genesis_hash,
        "slot": statement.slot,
        "closure": "cluster" if statement.universe_complete else "market",
        "identity": identity,
        "holdingsByClass": {
            flow_class: {
                "accounts": len(rows),
                "lamports": str(sum(row.lamports for row in rows)),
            }
            for flow_class, rows in sorted(grouped.items())
        },
        "accounts": [row.as_json() for row in sorted(statement.rows, key=lambda r: -r.lamports)],
        "fees": [
            {
                "signature": event.signature,
                "slot": event.slot,
                "lamports": str(event.lamports),
                "stage": event.stage,
                "label": event.label,
                "errored": event.errored,
                "source": event.source,
            }
            for event in evidence.fees
        ],
        "divergences": [divergence.as_json() for divergence in statement.divergences],
    }


ORACLE_NAME_ALPHABET = set("abcdefghijklmnopqrstuvwxyz0123456789-.")


def stable_name(row: AccountRow, taken: set[str]) -> str:
    """A name the existing oracle will accept, for an account the chain names.

    `tools/economic-lifecycle-ledger/ledger.py:63-70` requires every identifier
    to be lowercase `[a-z0-9-.]`, because its vocabulary is LOGICAL WALLETS --
    `genesis-mint`, `lifecycle-payer`. A chain's identifiers are base58, which
    is mixed case, so an observed trace cannot be spoken in that vocabulary
    without a mapping. This is that mapping, and it is a DERIVATION rather than
    an invention: the name is the run's own journal label wherever the run gave
    one, and a digest of the address only where it did not.
    """
    if row.label:
        slug = "".join(
            character if character in ORACLE_NAME_ALPHABET else "-"
            for character in row.label.lower()
        ).strip("-")
    else:
        slug = row.flow_class.replace("_", "-")
    digest = hashlib.sha256(row.address.encode()).hexdigest()[:12]
    candidate = slug or "account"
    if candidate in taken:
        candidate = f"{candidate}-{digest}"
    while candidate in taken:  # pragma: no cover - digest collision
        digest = hashlib.sha256(digest.encode()).hexdigest()[:12]
        candidate = f"{slug}-{digest}"
    taken.add(candidate)
    return candidate[:128]


def render_trace(statement: Statement) -> dict[str, Any]:
    """Emit the shape `tools/economic-lifecycle-ledger/ledger.py` already checks.

    That oracle is PREDICTIVE and never opens RPC, so until now its
    `check-lamports` trace could only be written by hand. This is the same
    protocol's observed history in the oracle's own vocabulary, which is what
    makes the two one system rather than two.
    """
    evidence = statement.evidence
    events: list[dict[str, str]] = []
    if evidence.opening is not None:
        events.append(
            {
                "kind": "transfer",
                "stage": "bankroll",
                "source": "genesis-mint",
                "destination": "lifecycle-payer",
                "lamports": str(evidence.opening.lamports),
            }
        )
    for event in evidence.fees:
        events.append(
            {
                "kind": "network-fee",
                "stage": event.stage,
                "payer": "lifecycle-payer",
                "lamports": str(event.lamports),
            }
        )
    taken: set[str] = set()
    for row in sorted(statement.rows, key=lambda r: r.address):
        if not row.flow_class.startswith(("market.", "wallet.campaign-named")):
            continue
        if row.address in evidence.genesis_accounts or row.address in CLUSTER_FIXTURES:
            continue
        events.append(
            {
                "kind": "rent-lock",
                "stage": "founding",
                "payer": "lifecycle-payer",
                "account": stable_name(row, taken),
                "class": row.flow_class,
                "lamports": str(row.lamports),
            }
        )
    return {"schema": LAMPORT_TRACE_SCHEMA, "events": events}


# --------------------------------------------------------------------------


def check_against_existing_oracle(statement: Statement) -> dict[str, Any]:
    """Run the REPO'S OWN conservation arithmetic over this derived history.

    `tools/economic-lifecycle-ledger/ledger.py` owns the identity
    `sum(wallet deltas) + live rent + network fees == 0` (its lines 809-814).
    That oracle is predictive and never opens RPC, so its trace could only ever
    be written by hand. Importing its checker rather than restating it is the
    whole point of this lane: two implementations that agree prove nothing
    except that someone copied a formula.

    The contract is FOUNDING-SCOPE. `fixtures/private-canonical.json` binds five
    `aggregateRefundClasses` and its check demands that every one of them appear
    as a `rent-refund` -- correct for a full lifecycle, and unsatisfiable for a
    market that is founded and not retired, which is every run on disk. The
    oracle itself allows this: an empty class list skips that check
    (`ledger.py:790`, `if required_classes and ...`). Nothing is weakened; a
    narrower contract is stated for a narrower claim.
    """
    spec = importlib.util.spec_from_file_location(
        "dclutch_economic_lifecycle_ledger",
        Path(__file__).resolve().parent.parent / "economic-lifecycle-ledger" / "ledger.py",
    )
    if spec is None or spec.loader is None:
        return {"available": False, "reason": "the economic-lifecycle-ledger oracle is not importable"}
    oracle = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = oracle
    spec.loader.exec_module(oracle)

    evidence = statement.evidence
    if evidence.opening is None:
        return {"available": False, "reason": "the run states no funding transfer to require"}
    contract = {
        "requiredFundingTransfers": [
            {
                "source": "genesis-mint",
                "destination": "lifecycle-payer",
                "lamports": str(evidence.opening.lamports),
            }
        ],
        "aggregateRefundClasses": [],
        "notes": [
            "founding scope: this market is founded and not retired, so no "
            "rent-refund has happened and no refund classification exists yet"
        ],
    }
    try:
        derived = oracle.derive_lamport_trace(contract, render_trace(statement))
    except Exception as error:  # the oracle raises its own Refusal type
        return {"available": True, "accepted": False, "refusal": str(error)}
    return {"available": True, "accepted": True, "result": derived}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="lamport_ledger.py",
        description="Derive one whole-market lamport statement from journals + chain.",
    )
    parser.add_argument("--run-root", required=True, type=Path,
                        help="a run's runs/seed-01 directory")
    parser.add_argument("--rpc-url", required=True,
                        help="loopback RPC of a validator resumed on that run's ledger")
    parser.add_argument("--universe", type=Path, default=None,
                        help="an `agave-ledger-tool accounts --include-sysvars` dump, for "
                             "a whole-cluster closure instead of a market one")
    parser.add_argument("--allow-remote-rpc", action="store_true",
                        help="permit a non-loopback endpoint (never mainnet)")
    parser.add_argument("--json", type=Path, default=None, help="write the statement as JSON")
    parser.add_argument("--trace", type=Path, default=None,
                        help="write a dclutch-exact-lamport-trace-v1 for check-lamports")
    parser.add_argument("--capitalization", type=int, default=None,
                        help="total lamports from `agave-ledger-tool capitalization`, in "
                             "LAMPORTS, to check the address set is complete")
    parser.add_argument("--cross-check-oracle", action="store_true",
                        help="run tools/economic-lifecycle-ledger/ledger.py's own "
                             "conservation arithmetic over this derived history")
    parser.add_argument("--strict", action="store_true",
                        help="exit nonzero if any divergence is reported")
    args = parser.parse_args(argv)

    evidence = load_evidence(args.run_root)
    rpc = Rpc(args.rpc_url, args.allow_remote_rpc)
    statement = build_statement(evidence, rpc, args.universe, args.capitalization)

    print(render_text(statement))

    if args.cross_check_oracle:
        verdict = check_against_existing_oracle(statement)
        print("-" * 96)
        print("  CROSS-CHECK — the existing oracle's arithmetic, on derived evidence")
        print("-" * 96)
        if not verdict.get("available"):
            print(f"  unavailable: {verdict.get('reason')}")
        elif not verdict.get("accepted"):
            print(f"  the oracle REFUSED this trace: {verdict.get('refusal')}")
        else:
            conservation = verdict["result"]["conservation"]
            print(f"  sum(wallet deltas) + live rent + fees = "
                  f"{conservation['sumWalletDeltaPlusLiveRentPlusFees']}")
            print(f"  holds: {conservation['holds']}")
            print(f"  live refundable rent  {verdict['result']['liveRefundableRentLamports']}")
            print(f"  total network fees    {verdict['result']['totalNetworkFeesLamports']}")
            print("  What this does and does not prove: the trace is EXPRESSIBLE in and")
            print("  consistent with the protocol's existing economic vocabulary -- its")
            print("  schema, its unique-rent-account rule, its arithmetic bounds. It is")
            print("  not an independent conservation proof: every rent-lock here is a")
            print("  payer->account pair, so the sum is zero by construction of the")
            print("  event list. THE INDEPENDENT TEST IS THE ONE ABOVE, which compares")
            print("  the funders' balances against what the chain actually holds.")
        print("")

    if args.json:
        args.json.write_text(json.dumps(render_json(statement), indent=1, sort_keys=True) + "\n")
        print(f"  wrote {args.json}")
    if args.trace:
        args.trace.write_text(json.dumps(render_trace(statement), indent=1, sort_keys=True) + "\n")
        print(f"  wrote {args.trace}")

    if args.strict and statement.divergences:
        print(
            f"\nrefusal: {len(statement.divergences)} divergence(s); "
            "the statement does not close",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
