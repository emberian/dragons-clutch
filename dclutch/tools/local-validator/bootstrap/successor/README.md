# Successor immutable-infrastructure bootstrap

This standalone localhost utility prepares the exact immutable substrate for
the current multi-program successor and then drives the first market lifecycle
through a real local validator. It does not seed mutable protocol state or call
a legacy direct Resolution ABI.

The prepared plan binds seven pairwise-distinct, real SBF artifacts:

- Registry
- Core
- Claims
- Trading
- Resolution
- Custody
- RentCredit

Registry, Claims, Trading, Resolution, Custody, and Rent are represented by
immutable Loader-v3 Program accounts and exact fixed-offset ProgramData
headers followed by the exact ELFs. Core begins with the same exact ELF and a
single ephemeral upgrade authority, then must reach that immutable header by
Loader revocation before release recognition. The plan also creates distinct
`ArtifactReleaseV1` bodies, the five-role
`ExecutionReleaseSetV1`, the captured local-Pyth release body, and the expected
144-byte `ProtocolInfrastructureProfileV1` body selecting Registry and Rent.
The profile itself is not genesis-injected: its sole PDA is derived under Core
and must be created by the canonical initialization transaction.

## Evidence boundary

Only Loader accounts and the infrastructure release records required to start
the successor are prepared as genesis fixtures. Core's genesis ProgramData is
explicitly pre-init and not an accepted immutable release observation. The
supervisor executes the remaining infrastructure and market boundary as real
localhost transactions:

1. Core initialization of the sole Registry/Rent infrastructure profile.
2. Loader-v3 revocation of Core's ephemeral authority to `None`, followed by
   Registry activation of the five-role immutable release set. Activation is
   **one role per transaction**: whole-ELF hashing costs about one compute unit
   per two bytes, so admitting the real seven artifacts in a single transaction
   cannot fit under the chain maximum. A partially activated cache cannot
   decode, so no reader can consume a half-activated release set.

Loader-v3 owns authority presence with the tag at byte 12. Its real
`Some -> None` serialization leaves bytes 13..45 as inactive storage rather
than clearing the former key; the ELF still begins at byte 45. The runner pins
and verifies that exact retained-byte poststate, while Registry never exposes
inactive bytes as an authority.

3. Creation of a real Token-2022 collateral Mint and wallet, preserving raw
   `u64` atoms and treating the full `u8` decimals field as display metadata.
4. Bounded Registry `Begin -> Append -> Finalize` publication of the Realm,
   Runtime-V2 Product graph, Source material, recovery policy, and capability
   manifest. The Product root, result domain, and portfolio are compiled and
   published through one chain-derived graph state machine.
5. One same-slot pre-credit projection of the canonical Market and
   `Market+generation` lifecycle-rent PDA, followed by RentCreditV2 creation
   and finalized reacquisition.
6. Publication of a finalized address lookup table covering the Found frame,
   and submission of Found31 as a packet-safe v0 transaction. With its keys
   inline the canonical 31-account frame serialises to 1,242 raw bytes against
   Solana's 1,232-byte legacy limit — it misses by ten. Routing is table data,
   never protocol authority: only
   non-signer coordinates and the invoked Program are routed, the fee payer and
   every signer stay in the message's static key list, and the table is
   authority-owned rather than frozen so its rent stays recoverable.
7. Canonical Core Found31 creation from the post-credit snapshot.

It emits finalized transaction metadata, exact poststate account hashes, and
hostile observations for wrong infrastructure authority, pre-revocation
activation, late atomic rollback, substituted Registry refund wallet, a
substituted lifecycle credit in Found31, and a substituted Market coordinate
under attacker-chosen routing whose whole multi-instruction transaction must
roll back to a fee-only debit.

The runner creates every signing keypair in process memory, gives `prepare`
only the Core authority public key, and retains no private key on disk. The run
spec contains semantic market inputs—not account addresses or caller-authored
digests. The Rust compiler and chain-derived operators own every record digest,
PDA, instruction frame, and next publication action.

## The current stopping point

Measured on a real localhost validator on 2026-08-26, with **all seven real
artifacts bound into the release set**, the campaign runs 46 finalized
transactions in 747 seconds and **creates the Market**. Canonical Core Found31
executes at **234,043** compute units, 16.7% of Solana's 1,400,000
per-transaction maximum. Release-set activation is now one role per transaction;
the worst of the five is Trading at **682,276** CU, 48.7% of the maximum.

That is a change from the first campaign, which is worth stating plainly because
this paragraph used to say the opposite. Found31 then exhausted the maximum
outright, and five-role activation with the real artifacts could not execute at
all, so the run had to bind Claims, Trading, Resolution, and Custody to distinct
immutable deployments of the much smaller Registry ELF. Both causes were the
same: on-chain hashing of whole ProgramData ELFs, about one compute unit per two
bytes, with the ~1.0 MB Core ELF hashed twice inside one transaction. `c61376d`
fixed it in two different ways, because the two sites needed opposite
treatments. Recurring readers stopped re-deriving a digest that activation had
already authenticated and that an immutable Loader v3 deployment cannot change.
First admission — the one site that checks an artifact record's *claimed* digest
against the bytes actually deployed — kept hashing, and activation was split so
that a transaction hashes one artifact rather than five. Nothing was weakened;
the fast path additionally requires the immutable policy and an absent live
upgrade authority, which the hashing path never demanded on its own.

**The Market is Found. It is not Open.** The campaign still stops where the
measurement above stops; this runner has gained no new stage. The Open-last chain
needs the atomic generic founding outer (`DCLTGMF1`,
`programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs`), which has the
correct Lock -> Found/permit -> Realize -> Claims FoundingV5 -> Core Open-last
order in one rollback domain.

**Every protocol gap on that path is now closed.** Four structural blockers were
found by executing toward it, one under the next; all four are implemented, and
none of them was implemented by weakening anything:

- the founding capability root — **implemented** (`728299a`,
  `docs/decisions/0004-founding-capability-root.md`). Core derives the root
  address from the authenticated Market-selected capability manifest entry and
  never persists or reads a root account at founding; ordinary activation still
  creates it afterwards. Two accounts left the outer frame, so `DCLTGMF1` at
  three funding states is **137** account references, not 139.
- no live Trading route emitted the projected-Custody `Initialize`/`OpenHoard`
  requests whose resulting state the Lock stage consumes — **implemented**
  (`28d2da6`). `DCLTPCB1` drives both under their single-use
  `ProjectedCustodyCallerSeedsV1` signers in one rollback domain. It also needed
  `f30d087`: the caller PDA seed domain was thirty-five bytes against Solana's
  thirty-two-byte cap, so **no projected-Custody transition could ever have
  signed**, and the whole projected family was dead at runtime.
- the Lock stage's funding source was not creatable — **implemented**
  (`d3ba6a1`). `OpenSourceCompartment` is a projected-family Custody operation
  that creates the normal `CustodyReplayV1` and the funded source Vault against
  a **vacant** Market. `authenticate_market` is byte-for-byte untouched and is
  not on that path: the new operation is admitted by the projected family's own
  membrane plus `require_vacant_market`, which is the *inverse* of
  `authenticate_market` rather than a relaxation of it.
- **nothing in the protocol could create the FundingState prestate Core's Found
  stage consumes** — **implemented** (`2fffe79`). The only allocator was the
  Series ticket-consume path, which had no caller anywhere; and a host cannot
  supply them at all, because they are program addresses owned by Trading, so no
  signature for them exists. `DCLTPCB1` gained a fourth stage that stages them
  from the manifest Core itself authenticated during the `ProjectFound`
  projection, prepaid by the founding's payer, bound to the artifact's own
  `funding_list_id`.

## What this runner still has to build

Two transactions, both requiring an address lookup table, and neither of them
exists here yet. `publish_routing_table` (`src/market.rs`) is reusable verbatim
for both. The complete index-by-index frame maps, all PDA seed orders, the wire
layouts, and the exact rent facts are recorded in the W1d supersession section of
`docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md`; this is the summary.

**1. `DCLTPCB1`, the prestate bootstrap — 81 accounts** at the demo Market's
three-entry manifest (`78 + funding_count`). Eight-byte instruction data, no
payload. Two readonly raw-request accounts (the 400-byte founding artifact and
the 768-byte terminal Lock request), the Custody program, a 42-account Initialize
sub-frame whose tail is Core's own 31-account `ProjectFound` sub-frame, a
15-account OpenHoard sub-frame, an 18-account OpenSourceCompartment sub-frame,
then one FundingState account per manifest entry. Forty-nine distinct keys.

**2. `DCLTGMF1`, the founding outer — 137 accounts.** Eight-byte instruction
data. Four readonly raw-request accounts (400-byte founding artifact, 768-byte
Lock, 768-byte Realize, 832-byte Claims request), then Lock (14), Found
(`34 + funding_count + 15`), Realize (12), Claims (32), Open (23). Eleven
distinct keys must be writable. **No account in the frame may be a
transaction-level signer** — every stage's signer is a PDA signed by
`invoke_signed` — so the fee payer must be a key that appears nowhere in it.

Three things that are easy to get wrong and fail late:

- `context_digest = sha256(b"dclutch:projected-hoard-context:v1" || found.context())`,
  while `funding_source_context` is `found.context()` **undigested**. Both are
  caller-PDA seed inputs, so a wrong one produces an address for which no
  signature exists.
- `projection_receipt_digest = sha256(ProjectFoundReceiptV1 bytes)`. The receipt
  is a pure function of facts this runner already holds, so it is derivable
  off-chain; it does not require simulating the CPI.
- Claims `FoundingV5` allocates the aggregate, the founder Position, and the
  admission with `System::allocate` + `assign` only. It never transfers
  lamports. **This runner must pre-fund those three vacant program addresses**
  to at least `rent.minimum_balance(width)` each, or the founding refuses inside
  Claims. A plain System transfer does it; no protocol route is needed.

And two request-shape pins: the terminal Lock carries `expected_revision = 3`
and `funding_source_replay_revision = 1`, and the founding artifact carries
`projected_resulting_revision = 5`.

`REMAINING_OPEN_SEAM` in `src/market.rs` is rewritten by whoever adds those two
stages; it still carries first-campaign prose this README has since corrected
three times.

Two earlier defects on that path were fixed at their semantic owners: the host
Found/RentV2 projections refused the real System Program (`c25de27`), and the
capability-root selection was a SHA-256 fixed point and so unsatisfiable for
every well-formed artifact (`386f254`). The full decomposition, the measured
per-role activation table, the current artifact digests, and the complete
46-transaction transcript are in
`docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md`.
