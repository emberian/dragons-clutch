# ADR-0007: counted retirement and monotone epoch identity

Status: proposed 2026-08-22; production codec/transition seam implemented in
`crates/clutch-retirement`, but **not integrated into live accounts and not
authorization to re-enable either close**

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
the host codec/state obligations only; its provisional tombstone tag bytes are
not live wire allocations, and the Promotion gate below remains open.

## Decision

Adopt the following design for a new account-version family only. Legacy
Position V1, Market V1, and Epoch V2 accounts are not locally migratable and
retain their current fail-closed behavior. Version numbers below advance the
current codec versions. ADR-0006's `V2` candidate-intent names are family
placeholders, not permission to reuse an occupied account version; a combined
implementation allocates fresh tags/versions and ships a counted Epoch V3 (or
later), never an intermediate new epoch that still lacks retirement facts.

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
never made absent. Reopen validates that exact tombstone, increments generation
with checked arithmetic, reallocates to Position V2, and writes a fresh rent
split. Generation zero remains reserved. Overflow refuses permanently.

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

Every reservation family that can own Position assets participates, including
general and direct reservations. There is no DREGG-specific branch.

### 2. Reservation registration and exact-once economic debit

Every successor reservation persists:

```text
epoch_generation: u64
position_counted: u8   # canonical 1 in ACTIVE/ENTITLED, 0 in terminal states
```

The reservation version is bumped. The marker is not a client hint: it is the
once-only debit state owned by the reservation.

Reservation creation is one Solana transaction that:

1. authenticates an OPEN Position V2 and OPEN counted Epoch;
2. checks both counters can increment;
3. moves the exact cash/Egg envelope out of Position;
4. creates and writes the reservation with both parent generations and
   `position_counted = 1`;
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

Rent-close of the terminal reservation is a different transition. It requires
`position_counted = 0`, deletes the reservation bundle exactly once, and
decrements the Epoch's `reservation_archives` count in that same transaction.
It never touches the Position counter.

`ClosePositionV2` authenticates local economic zero *and*
`outstanding_reservations == 0`, then writes the tombstone/refund split
atomically. Solana's writable account lock is the race barrier; every
reservation creator must therefore include the Position writable.

### 3. Market owns the monotone epoch cursor

Market V2 appends one `next_general_epoch_index: u64` (734 bytes at the current
Market width). Market creation selects the first index once; zero is a normal
default, not a protocol requirement. `InitEpochV3` requires the intent index to
equal this cursor and rejects `u64::MAX`, then creates the complete root and
advances the cursor by exactly one in the same transaction. Retirement never
changes the cursor.

Market V1 cannot initialize this field safely: historical Epoch indices were
caller-selected and there is no bounded, onchain maximum-index census. A claim
that an old Market has no omitted Epoch PDA is not an authenticated migration
proof. Legacy Markets therefore keep their roots occupied indefinitely; a new
Market V2 is required for counted retirement.

### 4. Epoch V3 counts every independently addressed child bundle

Epoch V3 adds `epoch_generation: u64`, nine `u32` counters, and the same 56-byte
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

At the current 329-byte Epoch V2 width this yields a 429-byte live Epoch V3.
`EpochWindow` also receives the epoch generation. Epoch, Window, and their
mandatory funding state are one root bundle, so Window is not self-counted.

Each child version persists the parent's epoch generation and is accepted only
after program-owner, exact tag/version/length, full identity fields, and
canonical PDA checks. Required bumps are:

- CandidateRecord V4 and CandidateFeed V2 (including its staging prefix):
  append the same `epoch_generation: u64`;
- CandidateIndexPage, CandidateVerdict, and CandidateEscrow from ADR-0006:
  persist the same `epoch_generation: u64` in their first counted versions;
- ClearWork V2 (growing and complete headers): append
  `epoch_generation: u64`;
- OrderPage V5, Reservation V5, SettlementReceipt V3, and FinalPot V3: append
  `epoch_generation: u64`.

One `candidate_bundle` is CandidateRecord + CandidateFeed + their funding
identity. `SubmitCandidate`/`BeginCandidate` creates the entire bundle and
increments once. Candidate status never changes its count: current submitted,
sealed-unverified, verified-retained, superseded, and refused states and
ADR-0006 staging, sealed, verified-valid/refused, expired, and selected states
are equally live children. The close requires the complete candidate bundle
present and its canonical ClearWork absent, then closes the bundle and
decrements once. The current feed-optional close shape is not carried into V3.

The retirement seam does not re-declare either candidate state machine. Its
adapter projection carries an opaque `(candidate tag, candidate version,
status)` witness produced only after that schema's owning decoder and lifecycle
validator succeed. Status updates preserve the registered tag/version and do
not touch the count. Thus exhaustive counting covers every admitted status
without creating a second semantic owner or making retirement interpret a
candidate terminality byte.

ADR-0006's CandidateIndex pages, CandidateVerdicts, and CandidateEscrows have
independent creation and close times, so each has its own counter. Their close
routes authenticate only their epoch generation and canonical identities; they
must not require an already-closable CandidateRecord to remain present. The
epoch-level `EpochCandidateWorkBudgetV2` is created and closed atomically with
the root bundle; if implementation gives it an independent lifetime, it must
instead gain a tenth counter before creation is enabled.

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

This list is exhaustive for the union of the current general-epoch account DAG
and ADR-0006's proposed candidate-lifecycle children. Adding another
epoch-owned PDA family requires an Epoch version bump and a new counter (or a
documented atomic inclusion in an existing bundle) before that family can be
created. A reserved counter is not silently repurposed.

### 5. Epoch retirement leaves an identity tombstone

After the Epoch reaches its existing terminal economic phase, child rent-close
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

The terminal root transition closes Window and its funding identity and
reallocates Epoch V3 at the same PDA to an exact 84-byte
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

Creation prepays the full 84-byte permanent minimum independently. Root close
refunds only the recorded live delta, routes surplus to the neutral sink, and
leaves exactly that minimum. `InitEpochV3` requires the tombstone target to be
absent and the Market cursor to match, so neither deletion, replay, nor a
residual child can reopen an old identity.

### 6. Atomicity and adapter write order

No counter transition is split across transactions. For each instruction the
adapter must:

1. decode and authenticate every account and generation;
2. compute the complete post-state in local values with checked arithmetic;
3. perform any CPI account creation/resize;
4. encode every post-state and execute transfers; and
5. return success only after every component is written.

Solana transaction rollback is the crash boundary. There is no recovery path
that guesses whether a counter write happened. Durable `position_counted` plus
child account presence are the only once-only markers. Keeper retries are
ordinary replays and must either reach the next valid transition or return the
same refusal with byte-identical prestate.

## Invariants

For every reachable V2/V3 state:

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
