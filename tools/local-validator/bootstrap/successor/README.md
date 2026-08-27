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

Measured on a real localhost validator, with **all seven real artifacts bound
into the release set**, the campaign creates the Market through canonical Core
Found31 and then **executes `DCLTPCB1`, the four-stage projected-Custody
founding prestate**, at its own generation. Found31 costs **232,537**
compute units, 16.6% of Solana's 1,400,000 per-transaction maximum.
Release-set activation is one role per transaction; the worst of the five is
Trading at **682,276** CU, 48.7% of the maximum.

Both of those numbers are a change from the first campaign, and this paragraph
used to say the opposite. Found31 then exhausted the maximum outright, and
five-role activation with the real artifacts could not execute at all, so the
run had to bind Claims, Trading, Resolution, and Custody to distinct immutable
deployments of the much smaller Registry ELF. Both causes were the same:
on-chain hashing of whole ProgramData ELFs, about one compute unit per two
bytes, with the ~1.0 MB Core ELF hashed twice inside one transaction. `c61376d`
fixed it in two different ways, because the two sites needed opposite
treatments. Recurring readers stopped re-deriving a digest that activation had
already authenticated and that an immutable Loader v3 deployment cannot change.
First admission — the one site that checks an artifact record's *claimed* digest
against the bytes actually deployed — kept hashing, and activation was split so
that a transaction hashes one artifact rather than five. Nothing was weakened;
the fast path additionally requires the immutable policy and an absent live
upgrade authority, which the hashing path never demanded on its own.

**The Market is Found, not Open — and at this revision no runner can open it.**
That is a different sentence from the one this file carried before, which said
the distance was a missing runner. It is not.

## Why `DCLTGMF1` is not built here

**Not because it cannot succeed.** It could not, until `dba22b5`. Core's Found
stage authenticates the liability basis as a **Registry**-owned `ProductBasisV3`
(magic `DCLTPAY3`, schema `GRADED_BASIS_RECORD_SCHEMA_ID_V3`, semantic domain
`dclutch/product-basis/semantic/v3`) and writes that record's digest and
semantic identity into the `ClaimsFoundingRequestV5` it commits to inside the
one-shot permit. Claims `FoundingV5` then demanded an account carrying **that
same digest** which decoded as a legacy **Core**-owned `LinkedBasisRecordV2`
(magic `DCLTLNK2`, length 224 or 248). Equal digests mean equal bytes, and no
byte string is both records — so the Lock → Found/permit → Realize → Claims →
Open chain was unsatisfiable, and this runner did not ship a stage that pretends.

The Claims lane has since made `FoundingV5` authenticate the basis Core actually
commits and deleted the legacy path. **This campaign already publishes exactly
that record**, so nothing here needed to change: the founding outer is now
satisfiable and simply has not been built.

What building it needs, recorded because establishing it was most of this lane's
cost:

- **137 accounts** (`134 + funding_count`): four readonly raw-request accounts,
  then Lock (14), Found (`34 + funding_count + 15`), Realize (12), Claims (32),
  Open (23), over the same `publish_routing_table` machinery `DCLTPCB1` uses.
- **No account in the frame may be a transaction-level signer** — every stage's
  signer is a PDA under `invoke_signed` — so the fee payer must be a key that
  appears nowhere in it.
- **Four pre-fundings, not three.** Claims `FoundingV5` allocates the aggregate,
  the founder Position, and the admission with `allocate` + `assign` and never
  transfers, so all three vacant program addresses need
  `rent.minimum_balance(width)`. The fourth is the Found caller-authority PDA:
  index 0 of the Found sub-frame is both the Trading caller PDA and Core's
  `payer`, and `FoundAccounts::parse` requires it signer-and-writable.
- The prestate `DCLTPCB1` leaves is exactly what its Lock stage consumes, so the
  two stages compose without a third party agreeing about anything.

## What the campaign does reach

## What the campaign does reach

Every protocol gap W1b, W1c, and W1d found on the *prestate* path is closed and
now **executed**, not merely assemblable:

- the founding capability root — derived, never created (`728299a`,
  `docs/decisions/0004-founding-capability-root.md`). Core derives the root
  address from the authenticated Market-selected capability manifest entry and
  never persists or reads a root account at founding.
- the projected replay and the Hoard vault (`28d2da6`), the funded source
  compartment (`d3ba6a1`), and the capability `FundingState`s (`2fffe79`) — all
  four staged by `DCLTPCB1` in one rollback domain, against a Market that does
  not exist yet, without weakening normal Custody's live-Market membrane.
- the liability basis the Product declares — published for the first time
  (`4b12ee1`). It had never existed; Found31 does not read it and nothing
  noticed.

The bootstrap stage runs two adversarial cases against the real chain, both
asserting the whole four-stage domain leaves nothing behind: a well-formed but
**non-terminal** projected-Custody request in the terminal slot, and a
**reordered FundingState tail**, which additionally must roll a preceding
transfer back to a fee-only debit. The tail is the manifest binding — reordering
it derives an address the manifest entry at that position does not name.

Four things the frame forced, recorded because each fails late and opaquely:

- The founding runs at **its own generation**. Every projected-Custody stage
  asserts the inverse of a live Market and Core's projection requires the Market
  vacant, so the Found31 Market cannot be reused.
- The **principal supplier is not the rent payer**: Custody requires the source
  funder's owner to sign while non-writable, and the payer must be writable.
  Privileges are per key, not per index.
- The projection sub-frame's payer slot is a **rent-capacity witness** — a
  distinct funded readonly key, because `parse_project` requires it unsigned and
  unwritable while the kernel still debits the Market rent against its lamports.
- `context_digest = sha256(b"dclutch:projected-hoard-context:v1" || context)`
  while `funding_source_context` is that context **undigested**; both are
  caller-PDA seed inputs, so a wrong one produces an address for which no
  signature exists.

Two earlier defects on that path were fixed at their semantic owners: the host
Found/RentV2 projections refused the real System Program (`c25de27`), and the
capability-root selection was a SHA-256 fixed point and so unsatisfiable for
every well-formed artifact (`386f254`). The full decomposition, the measured
per-role activation table, the current artifact digests, and the complete
transcript are in
`docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md`.
