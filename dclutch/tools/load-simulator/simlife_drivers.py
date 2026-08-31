#!/usr/bin/env python3
"""One function per mutation route, each calling the SHIPPED driver that owns it.

`simlife.py` draws a world and `simlife_drive.py` walks it; this is the layer
that turns one planned event into one invocation of the successor bootstrap.

THE RULE THIS MODULE EXISTS TO KEEP. Every function here builds a command line
for a driver that already exists, already owns its own signed journal, and
already decides for itself what is admissible. Nothing here reimplements a
constructor, derives a PDA, or signs anything. That is not fastidiousness: it is
FOUND-5182, where the founding driver's own hand-written copy of a kernel
constructor drifted by three bytes -- `StateBumpsV1::UNRECORDED` against real
bumps -- and every local founding refused `0x5182` for a day while the
"independent" control passed. A mirror of shipped code is a bug with a delay
fuse. So each route below is a subprocess and an exit code.

WHAT A RESULT MEANS. The four endings are `simlife.py`'s and they are not
interchangeable:

  executed     the driver's own report says a transaction finalized
  refused      the driver ran and said no, in its own words
  unattempted  there is no driver for this route at all
  blocked      a prerequisite of this event never executed

A driver that refuses is a MEASUREMENT of this substrate and belongs in the
artifact with its own sentence. Folding it into "not attempted" would hide the
one reading a reader actually wants.
"""

from __future__ import annotations

import dataclasses
import json
import os
from pathlib import Path
import subprocess
from typing import Optional

# The address lookup table program. A founding creates five routing tables and
# freezes exactly one of them (DCLTGMF3); the admission packet does not fit a
# legacy message and must route through that frozen one.
ADDRESS_LOOKUP_TABLE_PROGRAM = "AddressLookupTab1e1111111111111111111111111"
# `LookupTableMeta`: u32 discriminator, u64 deactivation slot, u64 last extended
# slot, u8 start index, Option<Pubkey> authority, then the addresses.
ALT_HEADER_BYTES = 56
ALT_AUTHORITY_FLAG_OFFSET = 21


class DriverRefusal(RuntimeError):
    """The driver ran and said no. Carries the driver's own first line."""


@dataclasses.dataclass
class Invocation:
    """One child process, kept so the ledger can name what ran."""

    argv: list
    returncode: int
    output: str
    log: Path
    stdout: str = ""

    def first_error(self) -> str:
        """The driver's own words, head one first.

        A driver that refuses prints its reason and then, often, a few hundred
        lines of the state it authenticated. The reason is the part a reader
        needs; the rest is on disk in the log this names.
        """
        lines = [line.strip() for line in self.output.splitlines() if line.strip()]
        for line in lines:
            if line.startswith("Error:") or "refus" in line.lower():
                return line[:600]
        return (lines[-1] if lines else "the driver exited nonzero and said nothing")[:600]


def run_driver(argv: list, log: Path, timeout: float, *, split: bool = False) -> Invocation:
    """One child, its whole transcript on disk.

    `split` keeps stdout separate, and it is not a convenience: the compilers
    PRINT THEIR DOCUMENT ON STDOUT and their progress on stderr, so a merged
    capture writes a `MarketRunInput` with a progress line inside it -- a
    document that then fails to parse two commands later, a long way from the
    place that broke it.
    """
    log.parent.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        [str(a) for a in argv],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE if split else subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    out = (proc.stdout or b"").decode("utf-8", errors="replace")
    err = (proc.stderr or b"").decode("utf-8", errors="replace") if split else ""
    log.write_text(out if not split else f"--- stdout ---\n{out}\n--- stderr ---\n{err}")
    return Invocation(
        argv=argv, returncode=proc.returncode, output=(err or out) if split else out,
        log=log, stdout=out,
    )


# ---------------------------------------------------------------------------
# Reading a chain, without signing anything
# ---------------------------------------------------------------------------


def rpc(url: str, method: str, params: list, timeout: float = 60.0):
    """One JSON-RPC call. Read-only by construction: this module never builds a
    transaction, so the only methods it can name are reads."""
    import urllib.request

    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
    request = urllib.request.Request(
        url, data=body.encode(), headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        answer = json.load(response)
    if "error" in answer:
        raise DriverRefusal(f"{method}: {answer['error']}")
    return answer.get("result")


_BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58(raw: bytes) -> str:
    number = int.from_bytes(raw, "big")
    text = ""
    while number:
        number, remainder = divmod(number, 58)
        text = _BASE58[remainder] + text
    for byte in raw:
        if byte:
            break
        text = "1" + text
    return text


def frozen_routing_table_for(url: str, market_address: str) -> Optional[str]:
    """The founding's own frozen DCLTGMF3 table, read off the chain.

    The admission message does not fit a legacy transaction and must route
    through a lookup table; SEL-SEAM measured that passing all five founding
    tables refuses `DuplicateAddress` and that exactly one -- the FROZEN one --
    is the contract, and named as residue that the founding campaign does not
    record its address in the evidence.

    It does not have to. A frozen table is one whose authority is `None`, and
    the founding's own is the frozen table whose address list contains the
    market. That is two facts already on the chain, so this reads them rather
    than asking the campaign to start writing a sixth thing down.
    """
    import base64

    accounts = rpc(url, "getProgramAccounts", [
        ADDRESS_LOOKUP_TABLE_PROGRAM,
        {"encoding": "base64"},
    ]) or []
    for entry in accounts:
        raw = base64.b64decode(entry["account"]["data"][0])
        if len(raw) < ALT_HEADER_BYTES:
            continue
        if raw[ALT_AUTHORITY_FLAG_OFFSET] != 0:
            # An authority is still set, so this table can still be extended:
            # it is not the frozen one the founding committed to.
            continue
        count = (len(raw) - ALT_HEADER_BYTES) // 32
        addresses = {
            base58(raw[ALT_HEADER_BYTES + 32 * index : ALT_HEADER_BYTES + 32 * (index + 1)])
            for index in range(count)
        }
        if market_address in addresses:
            return entry["pubkey"]
    return None


# Token-2022. Every market this compiler emits uses it for its collateral Mint.
TOKEN_2022_PROGRAM = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"


def collateral_accounts_for_mint(url: str, mint: str) -> list:
    """Every Token-2022 account holding this Mint, right now.

    Used ONCE, when a market is bound, and only to name what this chain ALREADY
    held. That distinction is the whole point and it is worth being explicit
    about: naming every account continuously would make L1 an identity -- the
    tracked total would be the mint supply by construction and the law would
    stop being able to say anything. Naming the PRIOR ones once, and then
    requiring every later account to come from the run's own admissions, leaves
    L1 doing exactly its job for the duration of the run: an account that
    appears mid-run and was not created by an admission this run drove is
    collateral going somewhere nobody named, and the census still says so.

    A rehearsal chain has a history -- probes, earlier runs, other lanes -- and
    a run that refused to acknowledge it would halt on its first census over
    somebody else's leftovers rather than on its own mistake.
    """
    accounts = rpc(url, "getProgramAccounts", [
        TOKEN_2022_PROGRAM,
        {
            "encoding": "base64",
            "filters": [{"memcmp": {"offset": 0, "bytes": mint}}],
        },
    ]) or []
    return sorted(entry["pubkey"] for entry in accounts)


def keypair_pubkey(path: Path) -> str:
    """The public half of a keypair FILE, computed without opening a wallet.

    A Solana keypair file is the 64-byte expanded secret; its second half is the
    Ed25519 public key verbatim, so no signing library and no secret material
    beyond what is already on disk is involved.
    """
    raw = json.loads(Path(path).read_text())
    if not isinstance(raw, list) or len(raw) != 64:
        raise DriverRefusal(f"{path} is not a 64-byte keypair file")
    return base58(bytes(raw[32:]))


# ---------------------------------------------------------------------------
# What a founded market is
# ---------------------------------------------------------------------------


def _existing(path: Path) -> Path:
    """A directory the driver requires to EXIST before it will journal into it.

    The retirement and payout drivers both refuse "--journal-dir must be an
    existing absolute directory" rather than create one, which is the right
    behaviour for a driver that fsyncs signed packets into it: creating a
    journal directory is the caller stating where its crash-safety lives.
    """
    path.mkdir(parents=True, exist_ok=True)
    return path


@dataclasses.dataclass
class FoundedMarket:
    """A market as the chain has it, assembled from the founding's OWN evidence.

    Every field is read out of the campaign report the founding driver wrote --
    never derived here, never guessed from the plan. The plan says what was
    asked for; this says what exists.
    """

    market_id: str
    address: str
    mint: str
    hoard: str
    aggregate: str
    payer: str
    founder_position: str
    founder_wallet: str
    participant_fixture_source: Optional[str]
    outcome_count: int
    claim_unit_atoms: int
    evidence: Path
    market_input: Path
    keys: Path
    routing_table: Optional[str] = None
    # Admission reports, per participant id: the fill driver consumes one.
    admissions: dict = dataclasses.field(default_factory=dict)
    # Every token account an admission's collateral leg CREATED, by holder.
    #
    # These are not decoration: collateral that moves into an account the census
    # does not name is collateral L1 correctly reports as gone. Measured --
    # a census of this market without the account reads "tracked 5172807456 !=
    # Mint supply 5173807456; 1000000 atoms are in accounts this ledger does not
    # name", and with it reads HOLDS. The address is the driver's own
    # `collateral.intent.participantTokenAccount`, never derived here.
    holder_tokens: dict = dataclasses.field(default_factory=dict)

    def census_binding(self) -> dict:
        binding = {
            "mint": self.mint,
            "payer": self.payer,
            "hoard": self.hoard,
            "aggregate": self.aggregate,
            "claim_unit_atoms": self.claim_unit_atoms,
            "outcome_count": self.outcome_count,
            "basis": "categorical-degree-0",
            "positions": {"founder": self.founder_position},
            "tokens": {"founder_wallet": self.founder_wallet},
        }
        for label, address in sorted(self.holder_tokens.items()):
            binding["tokens"][f"holder_{label}"] = address
        if self.participant_fixture_source:
            # L1 fails BY CONSTRUCTION without this one: these markets carry
            # 100,000,000 atoms of participant fixture liquidity that a census
            # naming only the Hoard and the founder's wallet cannot see, and the
            # law correctly reports the gap as atoms in accounts it does not
            # name.
            binding["tokens"]["participant_fixture_source"] = self.participant_fixture_source
        return binding


def founded_market_from_evidence(
    market_id: str,
    evidence_path: Path,
    market_input: Path,
    keys: Path,
) -> FoundedMarket:
    evidence = json.loads(Path(evidence_path).read_text())
    accounts = evidence["execution"]["market"]["accounts"]
    compiled = json.loads(Path(market_input).read_text())

    def address(name: str) -> str:
        entry = accounts.get(name)
        if not isinstance(entry, dict) or not entry.get("address"):
            raise DriverRefusal(
                f"the founding evidence for {market_id} names no {name}; this run refuses to "
                "guess an address it was not told"
            )
        return entry["address"]

    fixture = accounts.get("local_participant_fixture_source")
    return FoundedMarket(
        market_id=market_id,
        address=address("founding_market"),
        mint=address("collateral_mint"),
        hoard=address("founding_hoard_vault"),
        aggregate=address("claims_aggregate"),
        payer=evidence["payer"],
        founder_position=address("founder_position"),
        founder_wallet=address("collateral_wallet"),
        participant_fixture_source=(fixture or {}).get("address"),
        outcome_count=len(compiled["coefficients"]),
        # The claim unit is not a compiler parameter: `compile_linked_basis_v3`
        # hard-wires `payout_scale: 1` beside the categorical basis kind. The
        # census is told the truth rather than the plan's wish.
        claim_unit_atoms=1,
        evidence=Path(evidence_path),
        market_input=Path(market_input),
        keys=Path(keys),
    )


# ---------------------------------------------------------------------------
# The routes
# ---------------------------------------------------------------------------


FOUNDING_ROLES = (
    "collateral-mint",
    "collateral-wallet",
    "founding-beneficiary",
    "founding-projection-witness",
    "founding-source-funder",
    # `participant` and `direct-buyer` are required founding roles for THIS
    # compiler and no README says so: campaign.rs extends FOUNDING_REQUIRED_ROLES
    # with both whenever the market carries participant fixture liquidity, which
    # is every market this compiler emits.
    "participant",
    "direct-buyer",
    "fee-recipient",
)


@dataclasses.dataclass
class DriverContext:
    """Everything a route needs that is the same for every event."""

    bootstrap_bin: str
    rpc_url: str
    plan: str
    work: Path
    timeout: float
    # The five protocol-created founding roles start VACANT and are created by
    # the founding itself; the campaign payer is the substrate's and is reused.
    campaign_payer_keypair: str
    founding_founder: str
    substituted_founder: str
    solana_keygen: Optional[str] = None
    # The substrate's own prepared key directory. The Direct trade producer
    # needs two keys from it that are NOT founding roles and are not in a
    # market's own key set: `core-upgrade-authority` is the Direct payer and
    # `founding-founder` is the Direct seller. Discovered by running the
    # producer and reading which file it named next.
    substrate_keys: Optional[str] = None
    # How many times a founding may be retried with a WHOLE NEW key set. Two,
    # because the failure this exists for is a finalization transient inside a
    # ~90-transaction ladder and a second walk is cheap; a third would be
    # hammering a chain that has already said something real.
    founding_attempts: int = 2

    def market_dir(self, market_id: str) -> Path:
        return self.work / "markets" / market_id

    def log(self, market_id: str, name: str) -> Path:
        return self.work / "logs" / market_id / name


def new_keypair(context: DriverContext, path: Path) -> None:
    """One disposable local key.

    A partially-consumed key set is UNRESUMABLE -- a second founding attempt
    with the same keys refuses before any transaction, because no durable
    DCLTPCB2 checkpoint authenticates a safe suffix resume -- so every founding
    gets its own, and this never overwrites one that exists.
    """
    if path.exists():
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    if context.solana_keygen:
        subprocess.run(
            [context.solana_keygen, "new", "--no-bip39-passphrase", "--silent", "-o", str(path)],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        os.chmod(path, 0o600)
        return
    raise DriverRefusal(
        "no solana-keygen was configured, and this module will not forge an Ed25519 keypair "
        "itself: a key this run generated by hand is a key no wallet tool can read back"
    )


def drive_founding(context: DriverContext, market_id: str, planned) -> tuple:
    """Compile one market at the PLAN's shape and open it.

    Two commands, both shipped:

      `local-private-validator-market-v1` compiles a canonical `MarketRunInput`
      against the LIVE local deployment -- and since the shape widening it takes
      the band, the collateral and the terminal window as arguments, so the
      market it emits is the one the plan drew rather than the one constant it
      used to emit.

      `campaign --founding-only --through founding --execute` opens it.

    Returns `(FoundedMarket, Invocation)` or raises `DriverRefusal` carrying the
    driver's own refusal.
    """
    adopted = adopt_completed_founding(context, market_id)
    if adopted is not None:
        return adopted, None
    attempts = max(1, int(context.founding_attempts))
    last: Optional[DriverRefusal] = None
    for attempt in range(attempts):
        try:
            return _found_once(context, market_id, planned, attempt)
        except DriverRefusal as refusal:
            # A PARTIALLY CONSUMED KEY SET IS UNRESUMABLE. A second attempt over
            # the same five created signer coordinates refuses before any
            # transaction -- "this founding has STARTED on this chain but no
            # compatible durable DCLTPCB2 checkpoint authenticates a safe suffix
            # resume" -- so a retry means a whole new key set, in its own
            # directory, and the abandoned attempt stays on disk under its own
            # name rather than being cleaned up into invisibility.
            last = refusal
    raise DriverRefusal(f"{last} (after {attempts} founding attempts, each with fresh keys)")


def adopt_completed_founding(context: DriverContext, market_id: str) -> Optional["FoundedMarket"]:
    """A founding this work directory already completed, read back.

    Same doctrine as `MarketCensus.adopt_existing`: a rerun over a work
    directory continues it rather than restarting it. It has to be, here, for a
    harder reason -- a partially consumed founding key set is unresumable, so a
    run that re-founded on every restart would burn a fresh key set and ~85
    transactions per market to arrive at the market it already had. The
    evidence's own `execution.completed` is what decides; an attempt that did
    not complete is skipped and the next attempt number gets fresh keys.
    """
    root = context.market_dir(market_id)
    if not root.is_dir():
        return None
    for attempt in sorted(root.iterdir()):
        evidence = attempt / "founding-evidence.json"
        market_input = attempt / "market.json"
        if not evidence.is_file() or not market_input.is_file():
            continue
        try:
            body = json.loads(evidence.read_text())
        except ValueError:
            continue
        if not (body.get("execution") or {}).get("completed"):
            continue
        founded = founded_market_from_evidence(market_id, evidence, market_input, attempt / "keys")
        founded.routing_table = frozen_routing_table_for(context.rpc_url, founded.address)
        return founded
    return None


def _found_once(context: DriverContext, market_id: str, planned, attempt: int) -> tuple:
    out = context.market_dir(market_id) / f"attempt-{attempt:02d}"
    keys = out / "keys"
    for role in FOUNDING_ROLES:
        new_keypair(context, keys / f"{role}.json")

    market_input = out / "market.json"
    if not market_input.exists():
        compile_argv = [
            context.bootstrap_bin, "local-private-validator-market-v1",
            "--plan", context.plan,
            "--rpc-url", context.rpc_url,
            # ZERO FEE, always: fee-bearing founding does not fit in one
            # transaction on today's wire, and the world refuses a nonzero rate
            # where it is drawn rather than here.
            "--fee-basis-points", "0",
            "--fee-recipient-keypair", str(keys / "fee-recipient.json"),
            "--cuts", ",".join(str(cut) for cut in planned.cuts),
            "--cut-denominator", str(planned.cut_denominator),
            "--coefficients", ",".join(str(value) for value in planned.coefficients),
            "--initial-collateral-atoms", str(planned.founding_collateral_atoms),
            "--terminal-window-width-seconds", str(terminal_window_seconds(planned)),
            # The generation separates two markets that drew the same band, so
            # their derived identities cannot collide.
            "--generation", str(1 + int(market_id.lstrip("m") or 0)),
        ]
        log = context.log(market_id, f"compile-{attempt:02d}.log")
        run = run_driver(compile_argv, log, context.timeout, split=True)
        if run.returncode != 0:
            raise DriverRefusal(f"compiling {market_id}: {run.first_error()}")
        market_input.parent.mkdir(parents=True, exist_ok=True)
        market_input.write_text(run.stdout)

    evidence = out / "founding-evidence.json"
    found_argv = [
        context.bootstrap_bin, "campaign", "--founding-only",
        "--through", "founding", "--execute",
        "--rpc-url", context.rpc_url,
        "--plan", context.plan,
        "--market", str(market_input),
        "--evidence", str(evidence),
        "--founding-founder", context.founding_founder,
        "--substituted-founder", context.substituted_founder,
        "--keypair-campaign-payer", context.campaign_payer_keypair,
    ]
    for role in FOUNDING_ROLES:
        if role == "fee-recipient":
            continue
        found_argv += [f"--keypair-{role}", str(keys / f"{role}.json")]
    run = run_driver(
        found_argv, context.log(market_id, f"found-{attempt:02d}.log"), context.timeout
    )
    if run.returncode != 0:
        raise DriverRefusal(f"founding {market_id}: {run.first_error()}")
    market = founded_market_from_evidence(market_id, evidence, market_input, keys)
    market.routing_table = frozen_routing_table_for(context.rpc_url, market.address)
    return market, run


# The market's fixture liquidity, split across however many participants ask for
# collateral. Small on purpose: the fixture is 100,000,000 atoms and it is the
# only collateral outside the Hoard that a participant can be given, so a share
# that could exhaust it would make the LAST admission of a market fail for a
# reason about the fixture rather than about the admission.
FIXTURE_SHARE_DIVISOR = 16


def fixture_share_atoms(market: FoundedMarket) -> int:
    if market.participant_fixture_source is None:
        return 0
    return LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1 // FIXTURE_SHARE_DIVISOR


# The exact fixture liquidity every market this compiler emits carries, mirrored
# from `market.rs`'s own constant so the share arithmetic above is stated
# somewhere a reader can check it against the compiler.
LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1 = 100_000_000


def terminal_window_seconds(planned) -> int:
    """The plan's deadline in SLOTS, stated in the seconds the window wants.

    Two clocks, and this is the conversion between them. A market's terminal
    window is wall-clock and ENDS at the captured fixture publication, which is
    in the past on every local chain -- so a local market is past its terminal
    boundary the instant it is founded, and the width is how far back the window
    reaches rather than how long anybody has to wait. A slot is 400 ms by the
    cluster's own target, so a deadline of N slots is 2N/5 seconds, floored at
    one so a window is never an instant.
    """
    return max(1, (int(planned.deadline_slots) * 2) // 5)


def drive_admission(
    context: DriverContext,
    market: FoundedMarket,
    participant_id: str,
    owner_keypair: Path,
    stake_atoms: int,
    collateral_atoms: int = 0,
) -> Invocation:
    """One participant admitted to one market, through the shipped driver.

    TWO facts this needs that no README states. The owner wallet must already
    EXIST and be funded, or the driver refuses "snapshot missing required
    account" before it compiles anything -- an admission is over a wallet, and a
    wallet that has never been paid is not one. And the admission packet does
    not fit a legacy message: it routes through the founding's own frozen
    DCLTGMF3 lookup table, which is why `--routing-table` is here.
    """
    if market.routing_table is None:
        raise DriverRefusal(
            f"{market.market_id} has no frozen routing table on this chain, and the admission "
            "packet does not fit a legacy message without one"
        )
    output = context.market_dir(market.market_id) / "admissions" / f"{participant_id}.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    argv = [
        context.bootstrap_bin, "local-private-validator-user-position-admission-v1",
        "--rpc-url", context.rpc_url,
        "--plan", context.plan,
        "--campaign-evidence", str(market.evidence),
        "--position-owner", keypair_pubkey(owner_keypair),
        "--position-owner-keypair", str(owner_keypair),
        "--fee-payer", market.payer,
        "--fee-payer-keypair", context.campaign_payer_keypair,
        "--minimum-finalized-slot", "1",
        "--routing-table", market.routing_table,
        "--output", str(output),
        "--execute",
    ]
    if collateral_atoms > 0:
        # THE COLLATERAL LEG, and the Direct trade route requires it: the trade
        # producer refuses "Direct participant evidence omitted finalized
        # collateral preparation" over an admission that moved no collateral. It
        # is a separately journaled Token-2022 transfer out of the market's own
        # fixture source into the chain-derived participant account.
        if market.participant_fixture_source is None:
            raise DriverRefusal(
                f"{market.market_id} has no participant fixture source, so there is nothing to "
                "prepare collateral from"
            )
        source_owner = market.keys / "participant.json"
        argv += [
            "--collateral-source-owner", keypair_pubkey(source_owner),
            "--collateral-source-owner-keypair", str(source_owner),
            "--collateral-source-account", market.participant_fixture_source,
            "--collateral-quantity-atoms", str(collateral_atoms),
        ]
    run = run_driver(
        argv, context.log(market.market_id, f"admit-{participant_id}.log"), context.timeout
    )
    if run.returncode != 0:
        raise DriverRefusal(f"admitting {participant_id}: {run.first_error()}")
    market.admissions[participant_id] = output
    if collateral_atoms > 0:
        try:
            report = json.loads(output.read_text())
        except (OSError, ValueError) as error:
            raise DriverRefusal(
                f"the admission of {participant_id} wrote no readable report, so the account its "
                f"collateral moved into cannot be named to the census: {error}"
            ) from error
        account = (((report.get("collateral") or {}).get("intent") or {})
                   .get("participantTokenAccount"))
        if not account:
            raise DriverRefusal(
                f"the admission of {participant_id} prepared collateral and its report names no "
                "participantTokenAccount; the census would report those atoms as gone"
            )
        market.holder_tokens[participant_id] = account
    return run


def drive_fill(
    context: DriverContext,
    market: FoundedMarket,
    subject: str,
    participant_report: Path,
) -> Invocation:
    """One Direct trade: produce the session, then advance it.

    `…-direct-trade-produce-v1` freezes two already-signed host-verified Direct
    intents into a public manifest and a private session; `…-direct-trade-v1
    --execute` advances exactly one durable ALT, seal or Hot action per
    invocation and never blind-resubmits an ambiguous packet. Both are shipped
    and both own their own journal, so this composes neither.
    """
    slug = subject.replace("/", "_").replace(">", "-")
    key_dir = _trade_key_dir(context, market)
    produced = context.market_dir(market.market_id) / "fills" / slug
    if produced.exists() and any(produced.iterdir()):
        raise DriverRefusal(
            f"{subject} already has a produced trade directory; the producer refuses a "
            "non-empty output directory rather than overwrite a signed session"
        )
    # EXISTING and EMPTY, both: the producer refuses a path that does not exist
    # ("Direct output directory ...: No such file or directory") and refuses one
    # that already holds a session.
    produced.mkdir(parents=True, exist_ok=True)
    argv = [
        context.bootstrap_bin, "local-private-validator-direct-trade-produce-v1",
        "--rpc-url", context.rpc_url,
        "--plan", context.plan,
        "--market-input", str(market.market_input),
        "--campaign-report", str(market.evidence),
        "--participant-report", str(participant_report),
        "--key-dir", str(key_dir),
        "--output-dir", str(produced),
    ]
    run = run_driver(argv, context.log(market.market_id, f"fill-{slug}-produce.log"), context.timeout)
    if run.returncode != 0:
        raise DriverRefusal(f"producing the trade for {subject}: {run.first_error()}")
    sessions = sorted(produced.glob("*session*.json"))
    if not sessions:
        raise DriverRefusal(
            f"the trade producer wrote no session for {subject}; nothing to advance"
        )
    argv = [
        context.bootstrap_bin, "local-private-validator-direct-trade-v1",
        "--rpc-url", context.rpc_url,
        "--session", str(sessions[0]),
        "--execute",
    ]
    run = run_driver(argv, context.log(market.market_id, f"fill-{slug}-execute.log"), context.timeout)
    if run.returncode != 0:
        raise DriverRefusal(f"advancing the trade for {subject}: {run.first_error()}")
    return run


# The two keys the Direct trade producer reads that a market's own founding
# never created. Named here rather than passed as a directory, so a substrate
# key directory holding something else cannot be swept into a trade by accident.
TRADE_SHARED_KEYS = ("core-upgrade-authority", "founding-founder")


def _substrate_key(context: DriverContext, name: str) -> Path:
    """One of the substrate's own prepared role keys, by name.

    Named rather than guessed: each of these was established by a driver
    refusing and SAYING which identity it authenticated instead.
    """
    if context.substrate_keys is None:
        raise DriverRefusal(
            f"this route needs the substrate's {name} key and no lifecycle.substrate_keys "
            "directory was configured"
        )
    path = Path(context.substrate_keys) / f"{name}.json"
    if not path.is_file():
        raise DriverRefusal(f"the substrate key directory has no {name}.json")
    return path


def _trade_key_dir(context: DriverContext, market: FoundedMarket) -> Path:
    """The market's own keys plus the two the producer needs from the substrate.

    The producer refuses by NAMING the file it wanted next -- "Direct payer
    keypair …/core-upgrade-authority.json: No such file or directory", then
    "Direct seller keypair …/founding-founder.json" -- which is how this set was
    established: by running it, not by reading a README that does not mention
    either.
    """
    merged = market.keys.parent / "trade-keys"
    merged.mkdir(parents=True, exist_ok=True)
    import shutil

    for source in sorted(market.keys.glob("*.json")):
        target = merged / source.name
        if not target.exists():
            shutil.copyfile(source, target)
            os.chmod(target, 0o600)
    if context.substrate_keys is None:
        raise DriverRefusal(
            "the Direct trade producer needs the substrate's core-upgrade-authority and "
            "founding-founder keys and no lifecycle.substrate_keys directory was configured; "
            "neither is a founding role, so a market's own key set does not contain them"
        )
    for name in TRADE_SHARED_KEYS:
        source = Path(context.substrate_keys) / f"{name}.json"
        if not source.is_file():
            raise DriverRefusal(f"the substrate key directory has no {name}.json")
        target = merged / source.name
        if not target.exists():
            shutil.copyfile(source, target)
            os.chmod(target, 0o600)
    return merged


def drive_resolution(context: DriverContext, market: FoundedMarket, pyth_facts: Optional[str]):
    """The flagship resolution, three shipped modes in their required order.

    `--produce-input` is key-free and reads the chain; `--provision-tables`
    creates, extends and freezes the three exact typed tables one journaled
    action per invocation; the executor then walks submit -> execute -> reclaim
    -> complete. The producer needs `dclutch-flagship-pyth-update-facts-v1`,
    which is a fact about the CHAIN rather than about a market -- one
    provisioning serves every market on it.
    """
    if not pyth_facts or not Path(pyth_facts).is_file():
        raise DriverRefusal(
            "this chain has no dclutch-flagship-pyth-update-facts-v1 document, and the "
            "resolution producer will not invent one: the Pyth update account must be "
            "provisioned by local-private-validator-pyth-vaa-provision-v1 first"
        )
    out = context.market_dir(market.market_id) / "resolution"
    out.mkdir(parents=True, exist_ok=True)
    argv = [
        context.bootstrap_bin, "local-private-validator-flagship-resolution-v1",
        "--produce-input",
        "--rpc-url", context.rpc_url,
        "--plan", context.plan,
        "--campaign-evidence", str(market.evidence),
        "--pyth-facts", pyth_facts,
        "--producer-checkpoint", str(out / "producer-checkpoint.json"),
        "--output", str(out / "input.json"),
    ]
    run = run_driver(argv, context.log(market.market_id, "resolve-produce.log"), context.timeout)
    if run.returncode != 0:
        raise DriverRefusal(f"producing the resolution input: {run.first_error()}")
    # Table provisioning executes exactly one journaled action per invocation and
    # says so; it is driven to completion rather than assumed to be one call.
    for attempt in range(12):
        argv = [
            context.bootstrap_bin, "local-private-validator-flagship-resolution-v1",
            "--provision-tables",
            "--rpc-url", context.rpc_url,
            "--producer-checkpoint", str(out / "producer-checkpoint.json"),
            "--table-journal", str(out / "table-journal.json"),
            "--execute",
            # The FOUNDING FOUNDER, not the campaign payer. The provisioner
            # authenticates its input's own authority and says which: "authority
            # keypair public key ... differs from authenticated input ...".
            "--authority-keypair", str(_substrate_key(context, "founding-founder")),
        ]
        run = run_driver(
            argv, context.log(market.market_id, f"resolve-tables-{attempt:02d}.log"),
            context.timeout,
        )
        if run.returncode != 0:
            raise DriverRefusal(f"provisioning the resolution tables: {run.first_error()}")
        if "complete" in run.output.lower() and "frozen" in run.output.lower():
            break
    for stage in ("submit", "execute", "reclaim", "complete"):
        argv = [
            context.bootstrap_bin, "local-private-validator-flagship-resolution-v1",
            "--rpc-url", context.rpc_url,
            "--input", str(out / "input.json"),
            "--checkpoint", str(out / "checkpoint.json"),
            "--through", stage,
            "--execute",
            "--submitter-keypair", str(_substrate_key(context, "founding-founder")),
            "--resolver-keypair", str(_substrate_key(context, "resolver")),
            "--update-keypair", str(_substrate_key(context, "pyth-update-account")),
        ]
        run = run_driver(
            argv, context.log(market.market_id, f"resolve-{stage}.log"), context.timeout
        )
        if run.returncode != 0:
            raise DriverRefusal(f"resolution through {stage}: {run.first_error()}")
    return run


def drive_deadline_failure(context: DriverContext, market: FoundedMarket, signer_keypair: Path):
    """The failure walk, through the one CLI path that reaches the frame.

    The bare relay `RelayActionV1::CommitDeadlineFailure` has NO driver in the
    successor binary; `…-sponsored-push-v1 --action commit-failure` is the only
    command line that reaches it, and it consumes a sponsored-push input
    document rather than a market address.
    """
    out = context.market_dir(market.market_id) / "failure"
    out.mkdir(parents=True, exist_ok=True)
    source = out / "input.json"
    if not source.exists():
        raise DriverRefusal(
            "the failure walk needs a sponsored-push input document and this chain has none: "
            "the local market resolves through the PULL Pyth family (a captured fixture "
            "publication), and sponsored-push consumes the SPONSORED family's input. The two "
            "are different provider access profiles, so there is no such document to hand it"
        )
    argv = [
        context.bootstrap_bin, "local-private-validator-sponsored-push-v1",
        "--rpc-url", context.rpc_url,
        "--input", str(source),
        "--output", str(out / "output.json"),
        "--action", "commit-failure",
        "--signer", keypair_pubkey(signer_keypair),
        "--signer-keypair", str(signer_keypair),
        "--execute",
    ]
    run = run_driver(argv, context.log(market.market_id, "deadline-failure.log"), context.timeout)
    if run.returncode != 0:
        raise DriverRefusal(f"committing the deadline failure: {run.first_error()}")
    return run


def drive_redemption(
    context: DriverContext,
    market: FoundedMarket,
    participant_id: str,
    owner_keypair: Path,
    claim_index: int,
):
    """A holder collects: derive the payout input, then advance the payout.

    The input driver is read-only and derives a CRASH-STABLE parent context from
    the immutable request and the authenticated prestate rather than from the
    observation slot, which is why it is a separate command and not a flag.
    """
    out = context.market_dir(market.market_id) / "redemptions" / participant_id
    out.mkdir(parents=True, exist_ok=True)
    owner = keypair_pubkey(owner_keypair)
    argv = [
        context.bootstrap_bin, "local-private-validator-wallet-terminal-payout-input-v1",
        "--rpc-url", context.rpc_url,
        "--plan", context.plan,
        "--evidence", str(market.evidence),
        "--market", market.address,
        "--owner", owner,
        "--recipient", owner,
        "--claim-index", str(claim_index),
    ]
    run = run_driver(
        argv, context.log(market.market_id, f"redeem-{participant_id}-input.log"),
        context.timeout, split=True,
    )
    if run.returncode != 0:
        raise DriverRefusal(f"deriving the payout input for {participant_id}: {run.first_error()}")
    payout_input = out / "input.json"
    payout_input.write_text(run.stdout)
    argv = [
        context.bootstrap_bin, "local-private-validator-wallet-terminal-payout-v1",
        "--rpc-url", context.rpc_url,
        "--input", str(payout_input),
        "--fee-payer", market.payer,
        "--fee-payer-keypair", context.campaign_payer_keypair,
        "--owner-keypair", str(owner_keypair),
        "--journal-dir", str(_existing(out / "journal")),
        "--evidence", str(out / "evidence.json"),
        "--execute",
    ]
    run = run_driver(
        argv, context.log(market.market_id, f"redeem-{participant_id}.log"), context.timeout
    )
    if run.returncode != 0:
        raise DriverRefusal(f"paying out {participant_id}: {run.first_error()}")
    return run


def drive_retirement(context: DriverContext, market: FoundedMarket, source_receipt: Optional[str]):
    """The aggregate retirement: an immutable four-packet journaled campaign.

    Permissionless by design -- the point of a crank is that a stranger can turn
    it -- so the driver takes a fee payer and a lookup table rather than the
    holder's key.
    """
    if source_receipt is None:
        raise DriverRefusal(
            "the retirement driver requires --source-receipt, the terminal receipt this market "
            "reached, and this market has none: nothing terminal has happened to it"
        )
    if market.routing_table is None:
        raise DriverRefusal(f"{market.market_id} has no frozen routing table to retire through")
    out = context.market_dir(market.market_id) / "retirement"
    out.mkdir(parents=True, exist_ok=True)
    argv = [
        context.bootstrap_bin, "local-private-validator-aggregate-retirement-v1",
        "--rpc-url", context.rpc_url,
        "--plan", context.plan,
        "--evidence", str(market.evidence),
        "--market", market.address,
        "--source-receipt", source_receipt,
        "--fee-payer", market.payer,
        "--fee-payer-keypair", context.campaign_payer_keypair,
        "--lookup-table", market.routing_table,
        "--campaign", str(out / "campaign.json"),
        "--journal-dir", str(_existing(out / "journal")),
        "--completion", str(out / "completion.json"),
        "--execute",
    ]
    run = run_driver(argv, context.log(market.market_id, "retire.log"), context.timeout)
    if run.returncode != 0:
        raise DriverRefusal(f"retiring {market.market_id}: {run.first_error()}")
    return run


# Claim-check compaction by a stranger. There is NO driver anywhere -- not in
# the successor binary, not in the gauntlet, not in the SDK -- and this module
# will not become the first one, because a compaction it built by hand would be
# exactly the mirror this whole file exists to avoid. It is implemented and
# green in ProgramTest (`programs/dclutch-claims-sbf/tests/claim_check/mod.rs`,
# sixteen tests) and census-unbound.
#
# SIZED, because "impossible" would be false: the work is one new
# `local-private-validator-claim-check-compaction-v1` subcommand shaped like
# `…-wallet-terminal-payout-v1` -- read the holder's claim check and the
# market's terminal receipt, build the one Claims instruction the ProgramTest
# already calls, journal the packet before the send, verify the poststate. The
# ProgramTest gives it its own oracle. Estimate: 6-10 hours for the subcommand,
# its argument parser, its journal domain and one hostile test that a holder
# cannot compact their own check; plus 1-2 hours for the gauntlet binding, since
# an unbound transaction fails the census. It is a driver-tier lane, not a
# protocol one: no program changes.
COMPACTION_ABSENT = (
    "claim-check compaction by a stranger has NO CLI anywhere. It is implemented and green in "
    "ProgramTest (programs/dclutch-claims-sbf/tests/claim_check/mod.rs, sixteen tests including "
    "a_market_retires_a_sleeping_holders_position_and_the_holder_is_still_paid) and no gauntlet "
    "binding names it, so it is covered and census-unbound. Sized rather than guessed: one new "
    "local-private-validator-claim-check-compaction-v1 subcommand shaped like the wallet payout "
    "driver, 6-10 hours plus 1-2 for the gauntlet binding, no program change"
)
