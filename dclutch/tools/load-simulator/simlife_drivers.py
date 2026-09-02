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
import hashlib
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


# The founding's own label for the transaction that CREATES its routing table.
# `campaign` writes it into the evidence beside the signature, so the table is
# one `getTransaction` away and needs no search at all.
FROZEN_ROUTING_TABLE_CREATE_LABEL_V1 = "create DCLTGMF3 frozen routing address lookup table"


def routing_table_create_signature_v1(evidence: dict) -> Optional[str]:
    """The signature of the founding's own create-table transaction.

    Absent for a world founded before the campaign labelled it; that is an
    absence and answers `None`. A record that IS present and then fails to
    authenticate is a different thing and refuses by name.
    """
    for entry in ((evidence.get("execution") or {}).get("transactions") or []):
        if not isinstance(entry, dict):
            continue
        if entry.get("label") != FROZEN_ROUTING_TABLE_CREATE_LABEL_V1:
            continue
        if entry.get("error"):
            continue
        return entry.get("signature")
    return None


def created_address_lookup_table_v1(url: str, signature: str) -> Optional[str]:
    """The table one CreateLookupTable transaction created, from its own keys.

    `CreateLookupTable` puts the new table at account index 0 of its own
    instruction, so the address is in the transaction the founding already
    recorded. This does not decode the instruction data to prove the
    discriminant is Create: the campaign's label says which transaction this is,
    and the account is authenticated against the chain by
    `authenticated_frozen_routing_table_v1` before it is ever passed to a
    driver -- so a wrong pick refuses rather than routes.
    """
    body = rpc(url, "getTransaction", [signature, {
        "encoding": "json",
        "commitment": "finalized",
        "maxSupportedTransactionVersion": 0,
    }])
    if not body:
        return None
    message = ((body.get("transaction") or {}).get("message")) or {}
    keys = message.get("accountKeys") or []
    created = None
    for instruction in (message.get("instructions") or []):
        program = instruction.get("programIdIndex")
        if not isinstance(program, int) or program >= len(keys):
            continue
        if keys[program] != ADDRESS_LOOKUP_TABLE_PROGRAM:
            continue
        accounts = instruction.get("accounts") or []
        if not accounts or not isinstance(accounts[0], int) or accounts[0] >= len(keys):
            raise DriverRefusal(
                f"{signature} carries an Address Lookup Table instruction naming no table"
            )
        table = keys[accounts[0]]
        if created is not None and created != table:
            raise DriverRefusal(
                f"{signature} names two different lookup tables ({created}, {table}); "
                "this run refuses to choose one"
            )
        created = table
    return created


def authenticated_frozen_routing_table_v1(
    url: str, address: str, market_address: str
) -> str:
    """The two facts that make an address THE founding's routing table.

    Frozen -- authority `None`, so the extension plan is complete and nothing
    can add to it -- and routing this founding's own market. Both are read off
    the one account, by address.
    """
    import base64

    value = (rpc(url, "getAccountInfo", [address, {
        "encoding": "base64", "commitment": "finalized",
    }]) or {}).get("value")
    if not value:
        raise DriverRefusal(
            f"the founding recorded routing table {address} but the chain has no such account"
        )
    owner = value.get("owner")
    if owner != ADDRESS_LOOKUP_TABLE_PROGRAM:
        raise DriverRefusal(
            f"routing table {address} is owned by {owner}, not the Address Lookup Table program"
        )
    raw = base64.b64decode(value["data"][0])
    if len(raw) < ALT_HEADER_BYTES:
        raise DriverRefusal(
            f"routing table {address} is {len(raw)} bytes, shorter than a lookup table header"
        )
    if raw[ALT_AUTHORITY_FLAG_OFFSET] != 0:
        raise DriverRefusal(
            f"routing table {address} still carries an authority and can still be extended; "
            "the admission routes only through the FROZEN table the founding committed to"
        )
    count = (len(raw) - ALT_HEADER_BYTES) // 32
    addresses = {
        base58(raw[ALT_HEADER_BYTES + 32 * index : ALT_HEADER_BYTES + 32 * (index + 1)])
        for index in range(count)
    }
    if market_address not in addresses:
        raise DriverRefusal(
            f"frozen table {address} does not route {market_address}; it is some other "
            "founding's table and this run refuses to admit through it"
        )
    return address


def frozen_routing_table_for(
    url: str, evidence: dict, market_address: str
) -> Optional[str]:
    """The founding's own frozen DCLTGMF3 table, looked up BY ITS ADDRESS.

    The admission message does not fit a legacy transaction and must route
    through a lookup table; SEL-SEAM measured that passing all five founding
    tables refuses `DuplicateAddress` and that exactly one -- the FROZEN one --
    is the contract.

    This used to SEARCH for it: `getProgramAccounts` over the whole
    AddressLookupTable program, keeping the frozen table whose address list
    contained the market. That cannot work on a public chain and did not.
    Measured against cohort-11 on 2026-09-01 through a real devnet endpoint, it
    answered `None` for a table that demonstrably exists, because devnet's ALT
    program holds far too many accounts to enumerate through an RPC. And the
    predicate was wrong as well as unscannable: cohort-11's frozen table routes
    the founding's `founding_market` and sixty-three other accounts, and does
    NOT contain the Core Market address a caller would most naturally reach for,
    so a search given the wrong one of the two answers a confident `None`.

    Nothing has to be searched. The founding's own create transaction is in the
    evidence under its label, and `CreateLookupTable` names the table it
    creates. Two reads by address, and the account is then authenticated
    against the founding it claims to serve.
    """
    signature = routing_table_create_signature_v1(evidence)
    if signature is None:
        return None
    address = created_address_lookup_table_v1(url, signature)
    if address is None:
        return None
    return authenticated_frozen_routing_table_v1(url, address, market_address)


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


def sha256_hex(path: Path) -> str:
    """A file's digest, for the drivers that PIN their own inputs.

    The activation command takes `--expected-plan-sha256` and two siblings and
    refuses unless each file hashes to what the caller said it would. That is a
    driver asking its caller to state what it thinks it is passing, so the
    caller has to compute it: reading bytes off disk is not the constructor
    mirroring this module refuses. A wrong digest here cannot produce a wrong
    transaction -- it produces a refusal naming both hashes.
    """
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


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
    # The Direct capability EXECUTION root's activation report, once the
    # activation command has written one. A market without it can be admitted to
    # and censused but can never be filled: FOUNDING DOES NOT CREATE THIS ROOT
    # and nothing else in this tree does either.
    activation: Optional[Path] = None
    # What the activation said if it refused, kept so a fill can be refused with
    # the reason the ACTIVATION gave rather than with the producer's downstream
    # sentence about a root that was never created.
    activation_refusal: Optional[str] = None
    # The fee rate this market was founded at, read back from its own compiled
    # input. The owned-loopback Direct producer authors its own terms and admits
    # exactly one rate, so this is what decides whether a fill is reachable.
    fee_basis_points: int = 0
    # Admission reports, per participant id: the fill driver consumes one.
    admissions: dict = dataclasses.field(default_factory=dict)
    # WHICH admitted participant was funded to the fill requirement. A Direct
    # trade on this substrate is between the market's founding founder as seller
    # and ONE admitted buyer, and only the buyer whose collateral leg moved the
    # whole requirement can be that buyer -- so the fill uses this participant's
    # report whoever the world named in the pair.
    direct_buyer: Optional[str] = None
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
    fee_basis_points: int = 0,
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
        # The rate the compiler was ASKED for. It is not read back out of the
        # compiled input, because the only place it survives there is inside
        # `direct_capability.execution_config_hex` and decoding that field would
        # be a copy of the DCLTDDEC1 codec -- the exact mirror this module
        # refuses to become. The compiler refuses a rate it cannot honour, so
        # the asked rate is the founded rate or there is no market.
        fee_basis_points=int(fee_basis_points),
    )


# ---------------------------------------------------------------------------
# The routes
# ---------------------------------------------------------------------------


# Where a world's generations START. The local market compiler defaults to
# generation 1 and a held probe founds one market there before any world does,
# so a world numbering from 1 would put its first market on the substrate's own
# bootstrap coordinate. Ten leaves room for a substrate that founds several.
WORLD_GENERATION_BASE_V1 = 10


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
    adopted = adopt_completed_founding(context, market_id, planned)
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


def adopt_completed_founding(
    context: DriverContext, market_id: str, planned=None
) -> Optional["FoundedMarket"]:
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
        founded = founded_market_from_evidence(
            market_id, evidence, market_input, attempt / "keys",
            fee_basis_points=founding_fee_basis_points(planned),
        )
        founded.routing_table = frozen_routing_table_for(
            context.rpc_url, body, founded.address
        )
        adopted_activation = attempt.parent / "activation.json"
        if adopted_activation.is_file():
            founded.activation = adopted_activation
        return founded
    return None


# The one Direct fee rate the DEPLOYED setup release admits, and therefore the
# only rate at which a market on this substrate can ever be filled by the
# owned-loopback producer. It is not a preference of this module: the producer
# pins it (`direct_trade_producer.rs`, `FEE_BASIS_POINTS_V1`) because it has no
# ticket to read and must author its own terms, and the chain pins it too --
# `direct_token_setup_v1` is the sole creator of the seller and venue Direct
# token accounts and refuses unless the Market's finalized Direct config reads
# exactly `DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1`.
#
# A market founded at any other rate is a market this substrate can found, open,
# activate, admit to and census, and can never fill. That is a fact about the
# release rather than a defect in the world, so the world is allowed to draw
# such a market and the refusal is kept.
DIRECT_ADMITTED_FEE_BASIS_POINTS_V1 = 50


def founding_fee_basis_points(planned) -> int:
    """The rate a planned market asks its founding for.

    Old worlds have no such field and mean zero: the fee was a `Constant(0)` on
    every archetype until fee-bearing founding was shown to fit.
    """
    if planned is None:
        return 0
    return int(getattr(planned, "fee_basis_points", 0) or 0)


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
            # THE RATE THE WORLD DREW, and it may be nonzero.
            #
            # This was `"0"` unconditionally, with a comment saying fee-bearing
            # founding "does not fit in one transaction on today's wire" and
            # citing FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md. That
            # citation was a misreading and it cost the fill route: the document
            # is about the Direct HOT FILL's fee leg -- two Custody CPIs that
            # the transition co-enables and whose measured floor sat over the
            # 1,400,000 CU ceiling -- and says nothing about founding. A
            # fee-bearing founding fits, was measured fitting on 2026-08-30, and
            # is REQUIRED for a fill: a zero-fee local market can never be
            # filled by this driver whatever else is true of it.
            "--fee-basis-points", str(founding_fee_basis_points(planned)),
            "--fee-recipient-keypair", str(keys / "fee-recipient.json"),
            "--cuts", ",".join(str(cut) for cut in planned.cuts),
            "--cut-denominator", str(planned.cut_denominator),
            "--coefficients", ",".join(str(value) for value in planned.coefficients),
            "--initial-collateral-atoms", str(planned.founding_collateral_atoms),
            "--terminal-window-width-seconds", str(terminal_window_seconds(planned)),
            # The generation separates two markets that drew the same band, so
            # their derived identities cannot collide.
            #
            # OFFSET PAST THE COMPILER'S OWN DEFAULT, which is 1
            # (`LocalMarketShapeV1::default`). A held probe founds one market at
            # that default before this run starts, so a world whose first market
            # took generation 1 would be a world sharing a coordinate with the
            # substrate's own bootstrap market -- a collision that only appears
            # if the two also drew the same band, which is exactly the kind of
            # conditional failure that surfaces once a month and never in a test.
            "--generation", str(WORLD_GENERATION_BASE_V1 + int(market_id.lstrip("m") or 0)),
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
    market = founded_market_from_evidence(
        market_id, evidence, market_input, keys,
        fee_basis_points=founding_fee_basis_points(planned),
    )
    market.routing_table = frozen_routing_table_for(
        context.rpc_url, json.loads(evidence.read_text()), market.address
    )
    return market, run


def drive_direct_activation(context: DriverContext, market: FoundedMarket) -> Optional[Invocation]:
    """Create the market's Direct capability EXECUTION root.

    FOUNDING DOES NOT CREATE THIS ROOT AND NOTHING ELSE DOES EITHER. The
    campaign's last stage is `founding`; the root is written by Core's
    `ActivateCapability` route CPI-ing Trading's `process_activation`, and
    `local-private-validator-direct-capability-activation-v1` is the only command
    line in this tree that reaches it on a loopback validator. Until that command
    existed (2026-08-30) no local Direct fill was reachable at any market width,
    which is what the twenty-one refused fills in
    `SIMULATOR_POPULATION_DRIVEN_2026_08_30.md` were actually recording: the
    producer's "Direct root owner or width changed" was an ABSENT account
    rendered by a finalized snapshot as a System-owned zero-length placeholder,
    arriving at a width check wearing an owner change's clothes.

    The order is compile -> found -> **this** -> admit -> produce -> execute, and
    skipping this step is not a slower route to the same place.

    Idempotent by the driver's own design: a live Trading-owned root reports
    `already-active` and signs nothing. This wrapper is idempotent one level up
    too -- the command refuses to overwrite its `--output`, so a report already
    on disk is ADOPTED rather than re-walked.

    Returns the invocation, or `None` when the report was adopted.
    """
    output = context.market_dir(market.market_id) / "activation.json"
    if output.is_file():
        market.activation = output
        return None
    output.parent.mkdir(parents=True, exist_ok=True)
    argv = [
        context.bootstrap_bin, "local-private-validator-direct-capability-activation-v1",
        "--rpc-url", context.rpc_url,
        # Each of the three sealed inputs is passed WITH the digest the caller
        # believes it has. The driver refuses a mismatch by naming both hashes,
        # so this pin cannot silently pass the wrong file.
        "--plan", context.plan,
        "--expected-plan-sha256", sha256_hex(Path(context.plan)),
        "--market-input", str(market.market_input),
        "--expected-market-input-sha256", sha256_hex(market.market_input),
        "--campaign-report", str(market.evidence),
        "--expected-campaign-report-sha256", sha256_hex(market.evidence),
        # The campaign payer, which is the substrate's own funded identity and
        # the same one the founding used; the activation's payer signs the one
        # transaction and is not a protocol role.
        "--payer", market.payer,
        "--payer-keypair", context.campaign_payer_keypair,
        "--output", str(output),
        "--execute",
    ]
    run = run_driver(argv, context.log(market.market_id, "activate.log"), context.timeout)
    if run.returncode != 0:
        raise DriverRefusal(f"activating the Direct capability of {market.market_id}: "
                            f"{run.first_error()}")
    market.activation = output
    return run


# The market's fixture liquidity, split across however many participants ask for
# collateral. Small on purpose: the fixture is 100,000,000 atoms and it is the
# only collateral outside the Hoard that a participant can be given, so a share
# that could exhaust it would make the LAST admission of a market fail for a
# reason about the fixture rather than about the admission.
FIXTURE_SHARE_DIVISOR = 16


def fixture_share_atoms(market: FoundedMarket, *, taker: bool = False) -> int:
    """How many fixture atoms one admission moves to one participant.

    ONE TAKER PER MARKET, and the fixture decides that rather than the world.
    A Direct fill debits the buyer's participant token account
    `DIRECT_FILL_COLLATERAL_REQUIREMENT_ATOMS_V1`; the fixture holds
    `LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1` and is PINNED to exactly that
    value by the compiler (`market.rs` refuses any other nonzero amount), so
    two fully funded takers do not fit in one market and never will while the
    fixture is one constant. The first admission of a market that wants to trade
    takes the requirement; everyone after it takes the small share out of what
    is left, which is enough to hold a position and NOT enough to fill.

    That second group is not a defect either: an admission funded with less
    refuses on the BALANCE rather than on the trade, and a run that never drew
    one would never have measured which of the two walls it was standing at.
    """
    if market.participant_fixture_source is None:
        return 0
    if taker:
        return DIRECT_FILL_COLLATERAL_REQUIREMENT_ATOMS_V1
    return LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1 // FIXTURE_SHARE_DIVISOR


# What one Direct fill costs the buyer's participant token account.
#
# STATED, not derived from the chain, and it is the one number in this module
# that is arithmetic over host constants rather than a read: the producer authors
# its own terms as `FILL_ATOMS_V1` 100,000,000 at `EXECUTION_PRICE_V1` 500,000
# over `EXPECTED_PRICE_SCALE_V1` 1,000,000 -- 50,000,000 atoms -- plus the 50 bps
# fee, 250,000. It is a POLICY number: it decides how much collateral an
# admission moves, never what a transaction contains. If it is wrong the fill
# refuses on the balance and says the true number, so a stale value here cannot
# produce a wrong result, only a measured one.
DIRECT_FILL_COLLATERAL_REQUIREMENT_ATOMS_V1 = 50_250_000


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
        if (collateral_atoms >= DIRECT_FILL_COLLATERAL_REQUIREMENT_ATOMS_V1
                and market.direct_buyer is None):
            market.direct_buyer = participant_id
    return run


def _produce_direct_trade(context: DriverContext, market: FoundedMarket, subject: str,
                          participant_report: Path, key_dir: Path, produced: Path,
                          slug: str) -> None:
    """Freeze two signed Direct intents into a public manifest and a session."""
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
    run = run_driver(
        argv, context.log(market.market_id, f"fill-{slug}-produce.log"), context.timeout
    )
    if run.returncode != 0:
        raise DriverRefusal(f"producing the trade for {subject}: {run.first_error()}")


def drive_fill(
    context: DriverContext,
    market: FoundedMarket,
    subject: str,
    participant_report: Path,
    buyer_keypair: Optional[Path] = None,
) -> Invocation:
    """One Direct trade: produce the session, then advance it.

    `…-direct-trade-produce-v1` freezes two already-signed host-verified Direct
    intents into a public manifest and a private session; `…-direct-trade-v1
    --execute` advances exactly one durable ALT, seal or Hot action per
    invocation and never blind-resubmits an ambiguous packet. Both are shipped
    and both own their own journal, so this composes neither.
    """
    if market.activation is None:
        # NOT a producer refusal, and the difference is the whole point of
        # wiring the activation in: without the execution root the producer
        # refuses with a sentence about a root, and a reader has to already know
        # that absence and an owner change look alike at that check. This says
        # which step did not happen.
        raise DriverRefusal(
            f"{market.market_id} has no Direct capability EXECUTION root: its activation "
            + (f"refused -- {market.activation_refusal}" if market.activation_refusal
               else "was never run")
            + ". Founding does not create that root and nothing but "
              "local-private-validator-direct-capability-activation-v1 does"
        )
    slug = subject.replace("/", "_").replace(">", "-")
    key_dir = _trade_key_dir(context, market, buyer_keypair)
    produced = context.market_dir(market.market_id) / "fills" / slug
    # A TRADE ALREADY PRODUCED IS ADOPTED, NOT REFUSED, and this used to refuse.
    #
    # Same doctrine as the founding's, and here it is load-bearing rather than
    # tidy: a Direct trade is about ten durable actions, each with its own
    # journal, and the whole point of that design is that an interrupted trade
    # can be picked up where it stopped. Refusing a non-empty directory threw
    # that away -- a trade that had finalized its replay and token setup could
    # never be advanced again, and the run had to abandon signed work the chain
    # had already accepted.
    #
    # The producer's own refusal is still respected: it is never re-run over a
    # directory that holds a session.
    sessions = sorted(produced.glob("*session*.json")) if produced.is_dir() else []
    if not sessions:
        if produced.exists() and any(produced.iterdir()):
            raise DriverRefusal(
                f"{subject} has a trade directory with no session in it; the producer refuses a "
                "non-empty output directory rather than overwrite one, and there is nothing here "
                "to resume"
            )
        # EXISTING and EMPTY, both: the producer refuses a path that does not
        # exist ("Direct output directory ...: No such file or directory") and
        # refuses one that already holds a session.
        produced.mkdir(parents=True, exist_ok=True)
        _produce_direct_trade(context, market, subject, participant_report, key_dir, produced, slug)
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
    # ONE INVOCATION ADVANCES ONE ACTION, and this used to call it once.
    #
    # The driver's own usage says it: "Execute advances exactly one durable ALT,
    # seal, or Hot action and never blind-resubmits an ambiguous packet." A
    # Direct trade is about ten of those -- replay setup, token setup, lookup
    # create, three extends, freeze, activation, capability seal, Hot -- so a
    # single call advanced the FIRST one and returned zero, and this module
    # recorded a fill as `executed` over a trade that had barely started.
    #
    # Driven to completion the same way the resolution's table provisioning is,
    # and completion is the driver's OWN word for it: once the trade has
    # finalized it prints its persisted evidence document rather than a journal,
    # and that document's schema is the terminal state. Bounded, because a
    # driver that stopped making progress must stop this run rather than spin.
    last = None
    for attempt in range(DIRECT_TRADE_ACTION_CEILING_V1):
        run = run_driver(
            argv, context.log(market.market_id, f"fill-{slug}-execute-{attempt:02d}.log"),
            context.timeout, split=True,
        )
        if run.returncode != 0:
            raise DriverRefusal(
                f"advancing the trade for {subject} at action {attempt}: {run.first_error()}"
            )
        progress = _direct_trade_progress(run.stdout)
        if progress is not None and progress[0] == DIRECT_TRADE_FINALIZED_SCHEMA_V1:
            return run
        if progress is not None and progress == last:
            raise DriverRefusal(
                f"the trade for {subject} printed a byte-identical {progress[0]} "
                f"{progress[1]} report twice in a row without finalizing, so it is not advancing "
                "and this run stopped rather than resubmitting"
            )
        last = progress
    raise DriverRefusal(
        f"the trade for {subject} did not finalize within "
        f"{DIRECT_TRADE_ACTION_CEILING_V1} durable actions; its journal is on disk and a rerun "
        "resumes it"
    )


# How many durable actions one Direct trade may take before this module stops
# asking. The route is about ten -- replay setup, token setup, lookup create,
# the extends, freeze, activation, capability seal, Hot -- and the extension
# count depends on the market's width, so this is the route's own shape with
# room rather than a round number.
DIRECT_TRADE_ACTION_CEILING_V1 = 24

# What the trade driver prints once the trade has FINALIZED: its persisted
# evidence document rather than another journal. Read as a schema string rather
# than scraped from prose, because a driver's schema is a promise and its
# progress lines are not.
DIRECT_TRADE_FINALIZED_SCHEMA_V1 = "dclutch-owned-loopback-direct-trade-finalized-v1"


def _direct_trade_progress(stdout: str) -> Optional[tuple]:
    """WHERE the trade is, as a tuple the stall check can compare.

    SCHEMA ALONE IS TOO COARSE and that cost a hand-run its trade. The setup
    actions -- `replay-setup` then `token-setup` -- both print
    `dclutch-direct-trade-setup-journal-v1`, so a check keyed on the schema saw
    the same word after two consecutive FINALIZED actions and called a working
    trade stalled. The stage is what moves; the schema only says which family of
    document is describing it.

    `None` when the driver printed something this module cannot read, which is a
    reason to keep going rather than to stop: the exit code is what says whether
    the action landed, and a document this has no opinion about is not evidence
    of a stall.
    """
    text = (stdout or "").strip()
    if not text:
        return None
    try:
        body = json.loads(text)
    except ValueError:
        return None
    if not isinstance(body, dict):
        return None
    schema = body.get("schema")
    if not isinstance(schema, str):
        return None
    # THE WHOLE DOCUMENT, digested, and nothing narrower.
    #
    # Two narrower keys were tried and both called a working trade stalled. The
    # SCHEMA repeats across `replay-setup` and `token-setup`; the schema plus the
    # STAGE repeats across the three consecutive `lookup-extend` actions, each of
    # which finalized its own transaction. Every one of those is progress.
    #
    # A driver that is genuinely not advancing prints the SAME REPORT -- same
    # signature, same slot, same receipt -- so the exact document is the only
    # key that separates "did the same thing again" from "did the next thing of
    # the same kind". The stage is kept beside it for the refusal's sentence,
    # which a reader has to be able to act on.
    stage = str(body.get("stage") or body.get("nextAction") or "")
    return (schema, stage, hashlib.sha256(text.encode()).hexdigest())


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


# The three files the Direct producer opens out of `--key-dir`, and which
# identity each one has to be. Read off the producer rather than guessed
# (`direct_trade_producer.rs`, the three `key_dir.join(...)` calls), because
# guessing two of them right and one wrong is what a whole run of refusals looks
# like from outside.
TRADE_KEY_FILES = {
    "payer": "core-upgrade-authority.json",
    "seller": "founding-founder.json",
    "buyer": "participant.json",
}


def _trade_key_dir(
    context: DriverContext, market: FoundedMarket, buyer_keypair: Optional[Path] = None
) -> Path:
    """The three keypairs the producer opens, each the identity it authenticates.

    The producer reads exactly `core-upgrade-authority.json`,
    `founding-founder.json` and `participant.json` from this directory and
    refuses unless each expands to the public identity its EVIDENCE derives:
    "a private key file did not expand to its evidence-derived public identity".

    THE BUYER IS THE ADMITTED WALLET, NOT THE FOUNDING `participant` ROLE, and
    that collision cost this lane a whole run. Both are called `participant`.
    The founding role owns the market's fixture liquidity and is created by the
    campaign; the buyer is the wallet the ADMISSION named as position owner, and
    it is the one the participant report's public half is derived from. Copying
    the market's key set wholesale put the founding role at `participant.json`,
    and every fill on a market whose fee rate was otherwise admissible refused
    on the identity rather than on anything about the trade.

    So the buyer is written LAST and unconditionally, over whatever the market's
    own key set left there.
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
    if buyer_keypair is not None:
        # LAST AND UNCONDITIONAL. See the docstring: the market's own key set
        # carries a founding role of the same name, and letting it win is the
        # identity refusal.
        target = merged / TRADE_KEY_FILES["buyer"]
        shutil.copyfile(buyer_keypair, target)
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
    #
    # COMPLETION IS READ OFF THE JOURNAL, not scraped from the driver's prose.
    # This used to break on `"complete" in output and "frozen" in output`, a
    # pattern written from memory about sentences the driver may or may not
    # print -- the same species as the frame-diagnostic grep that reported a
    # confident zero over a build carrying forty-three. The journal is the
    # driver's own durable record: an invocation that adds no receipt and leaves
    # no intent behind advanced nothing, and there is nothing left to do.
    table_journal = out / "table-journal.json"
    receipts = -1
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
        landed = _table_journal_receipts(table_journal)
        if landed is not None and landed == receipts:
            break
        receipts = landed if landed is not None else receipts
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


def _table_journal_receipts(path: Path) -> Optional[int]:
    """How many table actions this journal has FINALIZED, or `None`.

    `None` means the journal is not readable yet, which is a reason to keep
    going rather than to stop: the first invocation writes it.
    """
    try:
        body = json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return None
    receipts = body.get("receipts")
    if not isinstance(receipts, list):
        return None
    # An intent still on the journal is an action mid-flight, so this is not a
    # resting point however many receipts are behind it.
    if body.get("intent") is not None:
        return None
    return len(receipts)


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
