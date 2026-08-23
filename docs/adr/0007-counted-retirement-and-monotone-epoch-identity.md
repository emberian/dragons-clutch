# ADR-0007: counted retirement and monotone epoch identity

Status: proposed 2026-08-22; frozen codecs and successor pure evidence are
implemented in `crates/clutch-retirement`, but **root open and retirement are
deliberately STOP, no successor is routed in SBF, and this is not authorization
to re-enable either close**

## Context

`ClosePosition` and `CloseGeneralEpoch` are intentionally fail-closed in the
current SBF program.

A general or direct sell placement moves Eggs out of `PositionAccount` and
into a separately addressed reservation. An all-in seller can therefore have
zero cash, zero reserved cash, and zero local Eggs while an ACTIVE reservation
still owns all of its assets. Position V1 has no authenticated aggregate count,
and `ClosePosition` receives no exhaustive reservation census.

The general `EpochWindowAccount` is not a child census. Its bounded registry
contains only verified candidates retained for selection. Submitted,
sealed-unverified, refused, displaced, and valid-but-noncompetitive candidate
pairs and any growing or complete ClearWork accounts can survive outside that
registry. Deleting the Epoch also deletes the only durable occupation of an
epoch index: Market V1 has no next-index cursor, so `InitEpoch` could recreate
the same Epoch and its child PDA namespace.

Solana cannot enumerate every program account from within an instruction.
Consequently neither missing fact can be reconstructed from a caller-provided
list or a static index. The safe successor has to establish authenticated
counts by induction at creation time and retain identity after economic state
is reclaimed.

The executable model for this decision is
[`research/deletion-replay-v2`](../../research/deletion-replay-v2/README.md).
It is host-tested model evidence, not an SBF, ELF, deployment, or formal-proof
claim.

The production-bound pure seam is
[`crates/clutch-retirement`](../../crates/clutch-retirement/README.md). It owns
the fixed extension/tombstone codecs and checked post-state transitions without
allocating a live instruction or touching the SBF dispatcher. Its tests close
the host codec/state obligations only. The central collision ledger reserves
its tombstone coordinates as disabled, but no live codec/SBF route follows from
that reservation, and the Promotion gate below remains open.

The isolated
[`clutch-retirement-adapter`](../../crates/clutch-retirement-adapter/README.md)
now composes those tails with the authoritative base layout decoders and owns
the owner/PDA/length/header/bump validation interface. This is still host
source, not a live registry allocation or SBF integration. The registry audit
and remaining bank campaign are in
[`COUNTED_RETIREMENT_LIVE_PROMOTION.md`](../implementation/COUNTED_RETIREMENT_LIVE_PROMOTION.md).

## Decision

Adopt the following design for a new account-version family only. Legacy
Position V1, Market V1, and Epoch V2 accounts are not locally migratable and
retain their current fail-closed behavior. Version numbers below advance the
current codec versions. ADR-0006's `V2` candidate-intent names are family
placeholders, not permission to reuse an occupied account version; a combined
implementation allocates fresh tags/versions and ships a counted general Epoch
V5 (the first free version under Epoch tag 11), never an intermediate new epoch
that still lacks retirement facts. Direct Epoch versions 3 and 4 remain owned
by their existing schemas.

### 1. Position identity is never deleted

Position V2 is the live form of the existing owner/Market PDA. It adds one
authoritative `outstanding_reservations: u32` and a mandatory rent split tail.
Its semantic fields are:

```text
PositionV2 = PositionV1 fields
           + outstanding_reservations: u32
           + RentSplitV2: 56 bytes

RentSplitV2 = payer: Pubkey
            + refundable_live_principal: u64
            + permanent_tombstone_principal: u64
            + donation_floor: u64
```

With the current 220-byte Position V1 layout, Position V2 is exactly 280 bytes.
It must use a new account version. Its terminal form uses a distinct
`POSITION_TOMBSTONE` tag/version at the same PDA and is exactly 76 bytes:

```text
tag/version: 2
market:      32
owner:       32
generation:  8
phase:       1    # CLOSED only
stored_bump: 1
```

The account is reallocated and rewritten to this tombstone; its address is
never made absent. The founding generation is zero, matching the authoritative
Position initializer. Reopen validates that exact tombstone, increments
generation with checked arithmetic, reallocates to Position V2, and writes a
fresh rent split. Overflow refuses permanently.

At initial creation, the payer transfers the full live rent-exempt minimum even
if the PDA was prefunded. On reopen, the tombstone's independently prepaid
minimum remains in place and the payer transfers the exact live-minus-tombstone
delta, again without a prefund discount. `permanent_tombstone_principal` equals
the then-current minimum for 76 bytes; `refundable_live_principal` equals the
live minimum minus that permanent minimum. Prefund and later unsolicited
surplus enter `donation_floor` and can never reduce either payer obligation.
Close refunds only the stored refundable amount to the stored payer, leaves
exactly the permanent minimum in the tombstone, and sends all surplus to the
frozen neutral sink. Hoard principal, collateral, fees, or future revenue fund
none of this.

Position reopen and fresh generation-scoped Replay creation are one funding
bundle. Each account has its own hostile-prefund admission and persisted rent
owner, but repeated payer identities must present the same authenticated
starting balance. Their full debits are coalesced and checked before either
account changes. Position/Replay target aliases, target/payer aliases, and
target/neutral-sink aliases refuse.

Every reservation family that can own Position assets participates, including
general and direct reservations. There is no DREGG-specific branch.

### 2. Reservation registration and exact-once economic debit

Every successor reservation persists:

```text
epoch_generation: u64
position_counted: u8   # canonical 1 in ACTIVE/ENTITLED, 0 in terminal states
rent_payer: Pubkey
refundable_principal: u64
donation_floor: u64
```

The already-committed general V5 and direct V6 schemas remain frozen count-only
envelopes: `618 + 9 = 627` bytes. Neither owns deletion funding, and neither may
authorize a live deletable creation or rent-close. Their exact decoders and
committed pure count transitions remain available so old bytes and downstream
source are never reinterpreted.

The exhaustive 23-variant `RetirementErrorV1` and frozen pure signatures retain
their committed source behavior. Successor-only APIs use
`RetirementErrorV2`; adapter successor APIs similarly use
`RetirementAdapterErrorV2`. Only exhaustive, lossless V1-to-V2 conversions
exist, and compile fixtures freeze both V1 error variant sets and prove that
historical exhaustive matches still compile.

Deletable general Reservation V7 and direct Reservation V8 are fresh successor
schemas. Each is exactly `618 + 9 + 48 = 675` bytes. The count marker is not a
client hint: it is the once-only debit state owned by the reservation. The
48-byte funding owner has no tombstone compartment because the terminal
Reservation is fully deleted. V7/V8 have exact version discrimination and no
fallback to V5/V6.

Creation transfers the full recorded principal from the stored payer even if
the canonical target was hostilely prefunded. Initial prefund becomes
`donation_floor`; it never discounts principal. Close requires at least
`principal + donation_floor`, returns exactly principal to the stored payer,
routes all other lamports to the frozen neutral sink, and leaves zero balance.
The current direct V2 base already carries an equivalent historical funding
ledger. The isolated adapter requires exact equality with the appended owner;
a clean direct base successor that removes this compatibility mirror remains a
central-layout activation decision.

Successor V7/V8 Reservation creation is one future Solana transaction that:

1. authenticates an OPEN Position V2 and OPEN counted Epoch;
2. checks both counters can increment;
3. moves the exact cash/Egg envelope out of Position;
4. creates and writes the reservation with both parent generations, the
   authenticated Position owner, and `position_counted = 1`;
5. increments `PositionV2.outstanding_reservations`; and
6. increments the Epoch's `reservation_archives` count.

Any refusal rolls all six effects back. The instruction must precompute every
checked add/subtract before its first write.

ACTIVE and ENTITLED are the only counted states. The first successful economic
terminal transition performs all of the following in one transaction:

```text
ACTIVE or ENTITLED -> RELEASED or CONSUMED
remaining assets  -> exactly zero
Position outstanding_reservations -= 1
position_counted  -> 0
```

RELEASED additionally returns the exact remaining envelope to the same live
Position generation. CONSUMED additionally requires the existing exact
entitlement/quantity/payment terminal equalities. Partial settlement and an
ACTIVE-to-ENTITLED transition do not decrement. A terminal replay sees
`position_counted = 0`/a terminal state and refuses without mutation; counter
underflow is always a refusal, never a clamp.

Terminal debit cross-binds the Reservation's authenticated Market and owner to
the exact Position in addition to matching Position generation. Archive close
cross-binds the Reservation's Market and Epoch identities to its general
parent. Equal generations alone never authorize a cross-Position or
cross-Epoch debit.

Rent-close of the terminal reservation is a different transition. It requires
`position_counted = 0`, deletes the reservation bundle exactly once, and
decrements the Epoch's `reservation_archives` count in that same transaction.
It never touches the Position counter.

Direct Reservation generation is not an instruction scalar. The isolated
adapter authenticates exact Direct Epoch V4 owner/PDA/header/length/bump bytes,
runs the authoritative decoder, and derives the child generation as checked
`epoch_index + 1`. It also projects the exact six-state lifecycle, and V8
registration requires pre-freeze-open; frozen, selected, settled, and
prefreeze-aborted parents refuse. Index `u64::MAX` refuses. The pure DTO is
forgeable and no live route calls the bridge, so runtime activation must
consume that exact adapter path and never accept a caller projection or scalar.

`ClosePositionV2` authenticates local economic zero *and*
`outstanding_reservations == 0`, then writes the tombstone/refund split
atomically. Solana's writable account lock is the race barrier; every
reservation creator must therefore include the Position writable.

### 2a. Replay is a closeable generation sibling

The reference Replay sequence is scoped to one Position generation. Retaining
one Replay account forever for every closed generation leaks rent; deleting it
separately from Position permits a half-close. The counted successor therefore
models Replay as a separately funded sibling carrying the same exact 48-byte
deletable funding owner.

Position retirement has no live-authorizable standalone successor plan. The
frozen root-only pure symbols retain their committed behavior for source
compatibility, but no runtime route may use them. One atomic successor plan
must authenticate exact `(market, owner, position_generation)` equality, precompute
the Position tombstone split and Replay deletion, coalesce payer/sink credits,
then write both or neither. Reopen requires old Replay absence, increments the
Position generation with checked arithmetic, and creates a fresh sequence-zero
Replay for that generation with full-principal hostile-prefund admission.
The adapter must authenticate the absent prior-generation Replay PDA and the
distinct next-generation Replay target; the pure plan cross-binds the absence
proof to the Position tombstone's exact Market, owner, and generation before
admitting reopen.

The current reference Replay body is 84 bytes, so the projected successor is
132 bytes. The existing Replay PDA seed is already generation-bearing. An
central registry reserves `0x7a/v1` as `ReservedDisabled`, and an in-flight
external general-v2 contract proposes the 132-byte shape. Neither this ADR nor
the retirement crates provide its authoritative composition codec/SBF route.
Exact codec composition, a seed audit, and SBF rollback tests remain activation
blockers. Legacy Replay routes remain
unchanged and cannot enable Position retirement.

### 3. Market owns the monotone epoch cursor

Market V2 appends one `next_general_epoch_index: u64` (734 bytes at the current
Market width). Market creation selects the first index once; zero is a normal
default, not a protocol requirement. `InitEpochV5` requires the intent index to
equal this cursor and rejects `u64::MAX`, then creates the complete root and
advances the cursor by exactly one in the same transaction. Retirement never
changes the cursor.

The pure successor root constructor checks Market cursor advancement and models
Epoch, Window, and candidate-work Budget together. Their independent rent-only
admission projections are bound to the same sink and coalesced by payer balance
before any modeled debit. That arithmetic is non-executable evidence: the
authoritative Budget owner has not supplied its complete reward-funding
capability, so every otherwise-valid root open returns
`BudgetFundingUnauthenticated` and creates nothing. General
`epoch_generation` is canonically `epoch_index + 1` in the fresh-root
constructor; `u64::MAX` refuses.

Market V1 cannot initialize this field safely: historical Epoch indices were
caller-selected and there is no bounded, onchain maximum-index census. A claim
that an old Market has no omitted Epoch PDA is not an authenticated migration
proof. Legacy Markets therefore keep their roots occupied indefinitely; a new
Market V2 is required for counted retirement.

### 4. General Epoch V5 counts every independently addressed child bundle

General Epoch V5 adds `epoch_generation: u64`, nine `u32` counters, and the same 56-byte
rent split:

```text
candidate_bundles
candidate_index_pages
candidate_verdicts
candidate_escrows
clear_work_bundles
order_pages
reservation_archives
settlement_receipts
final_pots              # constrained to 0 or 1
```

At the current 329-byte Epoch V2 width this yields a 429-byte live general Epoch V5.
`EpochWindow` and `EpochCandidateWorkBudget` also receive the epoch generation.
Epoch, Window, and Budget are one root bundle, so neither sibling is
self-counted. Their funding is nevertheless disjoint: Epoch owns only its live
refund plus permanent tombstone split, Window owns one deletable payer/
principal/donation record, and Budget owns another. No compartment funds
another.

Each child version persists the parent's epoch generation and is accepted only
after program-owner, exact tag/version/length, full identity fields, and
canonical PDA checks. Required bumps are:

- CandidateRecord V4 and CandidateFeed V2 (including its staging prefix):
  append the same `epoch_generation: u64`;
- CandidateIndexPage, CandidateVerdict, and CandidateEscrow from ADR-0006:
  persist the same `epoch_generation: u64` in their first counted versions;
- ClearWork V2 (growing and complete headers): append
  `epoch_generation: u64`;
- OrderPage V5, deletable Reservation V7, SettlementReceipt V3, and FinalPot V3: append
  `epoch_generation: u64`.

One `candidate_bundle` is CandidateRecord + CandidateFeed + their funding
identity. `SubmitCandidate`/`BeginCandidate` creates the entire bundle and
increments once. Candidate status never changes its count: current submitted,
sealed-unverified, verified-retained, superseded, and refused states and
ADR-0006 staging, sealed, verified-valid/refused, expired, and selected states
are equally live children. The close requires the complete candidate bundle
present and its canonical ClearWork absent, then closes the bundle and
decrements once. The current feed-optional close shape is not carried into the
counted successor family.

The retirement seam does not re-declare either candidate state machine. Its
pure projection carries `(candidate tag, candidate version, status)` only after
the caller claims owning-decoder/lifecycle validation. That DTO is not runtime
authority. Status updates preserve tag/version and do not touch the count; the
future live adapter must supply exact codec/PDA/owner validation.

ADR-0006's CandidateIndex pages, CandidateVerdicts, and CandidateEscrows have
independent creation and close times, so each has its own counter. Their close
routes authenticate only their epoch generation and canonical identities; they
must not require an already-closable CandidateRecord to remain present.
CandidateWindowV4's admission-node ledger is a different ownership domain:
admission nodes are not retrofitted into a tenth frozen V1 Epoch count. Root
retirement requires a private-field pure witness that the complete Window
ledger is finalized, headless, and fully closed. Exact Window codec/PDA/owner
authentication is still missing. Budget has an independent root lifetime and
cannot close until its authoritative economic owner supplies an opaque terminal
disposition.

One `clear_work_bundle` is a growing or complete ClearWork plus its funding
identity. `InitClearWork` increments once at first-stage creation; grow and
complete do not. Its terminal close decrements once. Candidate and ClearWork
counts are independent so a submitted record with no work, a half-grown work,
and a complete work are all represented without a registry scan.

Pages, reservation archives, receipts, and the pot increment only on actual
PDA creation and decrement only on authenticated deletion. A reused existing
receipt on its second endpoint does not increment. Every governed account and
funding ledger/tail is created and closed atomically as one counted bundle.
Counter overflow/underflow refuses before mutation.

Creation phase is exact rather than merely "not terminal": OrderPage and
Reservation archive creation require OPEN; candidate bundles, CandidateIndex
pages, CandidateVerdicts, CandidateEscrows, and ClearWork bundles require
FROZEN; SettlementReceipts and FinalPot require CLEARED. SETTLED and LAPSED
admit no new child. The lifecycle owner still validates each family's economic
preconditions before supplying its counted transition.

This list is exhaustive for the union of the current general-epoch account DAG
and ADR-0006's proposed candidate-lifecycle children. Adding another
epoch-owned PDA family requires an Epoch version bump and a new counter (or a
documented atomic inclusion in an existing bundle) before that family can be
created. A reserved counter is not silently repurposed.

### 5. Epoch retirement leaves an identity tombstone

After the Epoch reaches SETTLED or LAPSED, child rent-close
order is:

1. settle or release every reservation economically;
2. exhaust and close receipts, pot, candidate verdicts/escrows, and candidate
   index pages as their economic and enumeration dependencies permit;
3. close pages, then terminal reservation archives;
4. close every ClearWork, including growing work, before its candidate bundle;
5. close every candidate bundle; and
6. close the root only when all nine authenticated counts are zero.

Steps within a dependency level may be permuted. A root close additionally
checks the canonical child slots it receives where applicable, but no presented
list is treated as the aggregate proof; the counters are authoritative.

The intended terminal root transition would close Window and Budget and
reallocate general Epoch V5 at the same PDA to an exact 84-byte
`GENERAL_EPOCH_TOMBSTONE`:

```text
tag/version:      2
epoch:           32
market:          32
epoch_index:      8
epoch_generation: 8
phase:            1    # CLOSED only
stored_bump:      1
```

Creation must prepay the full 84-byte permanent minimum independently. Root
close may refund only Epoch's recorded live delta and Window's separately
recorded principal. Budget reward liabilities, cleanup markers, and funding
disposition are not modeled as surplus. Consequently public root close always
returns `BudgetRetirementUnauthenticated`; the internal rent-only coalescing
calculation is explicitly non-executable evidence. Source aliases and scalar
sink substitution still refuse before that STOP where applicable.

### 6. Atomicity and adapter write order

No counter transition is split across transactions. For each instruction the
adapter must:

1. decode and authenticate every account and generation in the live adapter;
2. reject source/source and source/recipient aliases and compute the complete
   post-state plus coalesced funding debits or recipient credits with checked
   arithmetic;
3. perform any CPI account creation/resize;
4. encode every post-state and execute transfers; and
5. return success only after every component is written.

Solana transaction rollback is the crash boundary. There is no recovery path
that guesses whether a counter write happened. Durable `position_counted` plus
child account presence are the only once-only markers. Keeper retries are
ordinary replays and must either reach the next valid transition or return the
same refusal with byte-identical prestate.

Current public `Adapter*ProjectionV1` structs are forgeable pure DTOs. Only the
private-field `ValidatedAdmissionLedgerRetiredV1` proves complete pure Window
terminal structure, and even it proves no runtime owner/PDA/account bytes. The
Direct Epoch V4 bridge is the only exact end-to-end account projection in the
isolated adapter; its projected lifecycle is checked again by successor
registration. General neutral-sink provenance, Position/Replay identity and
absence, Window V4, and Budget capabilities are promotion blockers.

## Invariants

For every reachable counted-successor state:

```text
Position.outstanding_reservations
  = count(reservations bound to its current generation
          with position_counted = 1)

Epoch.child_count[k]
  = count(live authenticated child bundles of kind k
          bound to that epoch generation)

Position is a tombstone => local economic zero and outstanding count = 0
Epoch is a tombstone    => all nine child counts = 0
Market.next_epoch_index > every admitted epoch_index
```

The first two equalities are inductive runtime obligations, not global account
scans. They hold initially at zero and every authorized creator/deleter must
preserve them. No admin repair instruction may assign a count directly.

## Promotion gate

Neither current close may be re-enabled until all of the following land for one
newly built, digest-recorded ELF:

- authoritative Budget funding and terminal-disposition capabilities covering
  every reward liability, cleanup marker, and economic compartment;
- exact runtime bridges for CandidateWindowV4, general Market/Realm neutral
  sink, Position/Replay identities, and generation-scoped Replay absence;
- an exact Replay successor composition codec/seed and SBF route (the central
  `0x7a/v1` coordinate is reserved disabled; the external 132-byte proposal is
  not an executable implementation here);
- exact codecs, length/version refusal tests, seed tests, and rent-split tests
  for every new account shape;
- host transition tests covering every reservation creator and every release,
  entitlement, partial-settlement, consumption, and archive-close route;
- adversarial tests for all candidate statuses, growing/full ClearWork,
  displaced candidates, stale generations, counter overflow/underflow, wrong
  PDAs, wrong owners, wrong parents, and double close;
- local-bank rollback tests that force a late failure after each multi-account
  mutation path and assert every writable account and lamport balance equals
  prestate;
- an all-in seller SVM test proving local zero cannot close while its live
  reservation is counted;
- a full lifecycle SVM test proving counters return to zero exactly once, the
  Position and Epoch shrink to their permanent tombstones, the stored payer
  alone receives refundable principal, and surplus goes only to the neutral
  sink;
- replay tests proving old Position generations, old epoch indices, closed
  candidate/work tickets, and exact transaction retries cannot mutate state;
- randomized differential traces against the model crate, with invariant
  checks after every successful transition and byte-identical snapshots after
  every refusal; and
- current-tree CU/account-meta measurements. If the new writable parents make
  any route exceed its admitted capability profile, that route remains STOP;
  a counter is never omitted to recover headroom.

The checked model currently covers value-level exact counts, candidate status
exhaustiveness, ClearWork-before-candidate closure, all-in seller refusal,
generation/cursor monotonicity, exact-once replay refusal, forged count/ticket
refusal, every child family, and injected rollback at modeled write boundaries.
It does not verify Solana account locking, codec correspondence, CPI behavior,
rent arithmetic, or an ELF.

The current general SBF lifecycle treats CLEARED as its post-selection working
phase and does not stamp the layout's SETTLED phase before legacy cleanup/root
close. Counted retirement deliberately does not collapse CLEARED into
terminality: the successor runtime must add and authenticate an exact SETTLED
transition after every settlement dependency is economically terminal. Until
that wider lifecycle change and its rollback tests land, general counted child
cleanup and root retirement cannot be activated. LAPSED remains independently
terminal.

## Consequences

- Safe deletion is a new-format feature, not a patch to legacy bytes.
- The epoch tombstone permanently locks a small rent principal; this is the
  price of inspectable replay identity. Market's eight-byte cursor is still the
  monotone allocation authority and prevents index skipping/reuse.
- Counter parents become writable on more creation, settlement, release, and
  close paths. That raises account-meta contention and CU and must be measured.
- Candidate and ClearWork rent can be reclaimed without trusting the retained
  registry or an offchain index.
- Position close becomes safe even for an all-in seller because local zero is
  no longer treated as aggregate zero.

## Rejected alternatives

- **Re-enable either legacy close with its present checks.** Demonstrably
  unsafe; the live all-in reservation and unretained candidate shapes are
  counterexamples.
- **Require callers to present every child.** Absence from a transaction is not
  a proof that another PDA does not exist.
- **Use `EpochWindow.retained` as the candidate count.** It is deliberately
  bounded and non-exhaustive.
- **Initialize a cursor or count on old roots from an indexer snapshot.** Static
  clients are untrusted projections and the program cannot prove the snapshot
  complete.
- **Delete the tombstone and rely only on a caller sequence.** That makes the
  replay fact caller-controlled again.
- **Decrement Position count at reservation rent-close.** Economic ownership
  ends earlier, and current dependency order may delete pages before archives;
  coupling semantic liveness to later rent recovery unnecessarily blocks
  Position retirement.
- **Decrement before partial settlement completes.** It makes a locally zero
  Position closable while an entitlement/payment obligation remains.

## Evidence

- `docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md`, P0 Position and Epoch
  findings;
- `docs/reviews/STATE_RENT_AUDIT_2026-08-22.md`, current byte/rent inventory and
  tombstone estimate;
- `programs/clutch-sbf/program/src/instructions/orders_batch/terminal_closure.rs`,
  current fail-closed runtime;
- `research/deletion-replay-v2`, executable model and adversarial tests; and
- `crates/clutch-retirement`, production-bound fixed codecs, frozen vectors,
  hostile-byte decoders, and pure exact-once transitions.
