# Counted-retirement live-promotion plan

Status: **frozen composition and partial authentication seam implemented; root
open/close deliberately STOP on Budget capabilities; no live allocation,
handler, dispatcher, SBF, deployment, or re-enabled close**

Normative decision: [`ADR-0007`](../adr/0007-counted-retirement-and-monotone-epoch-identity.md)

Production-bound seams:

- [`clutch-retirement`](../../crates/clutch-retirement/README.md) owns the new
  fixed tails, tombstones, counts, and pure transitions;
- [`clutch-retirement-adapter`](../../crates/clutch-retirement-adapter/README.md)
  composes those tails with the authoritative `clutch-solana-layout` base
  decoders and validates runtime owner/PDA/length/header/bump facts.

Both legacy close handlers remain fail-closed. This document is an
implementation plan, not SVM or release evidence.

Source compatibility is preserved: exhaustive `RetirementErrorV1` has its
exact committed 23 variants and order, while successor-only APIs use
`RetirementErrorV2`. Conversion exists only from V1 to V2 and is exhaustive and
lossless. Compile fixtures exercise a downstream exhaustive V1 match and the
historical child-projection name.

## Registry audit and allocation proposal

At audited committed HEAD `2fac9a2df6eeec72b5a2661a1bd9d6e2b59c6554`, the
global account-header allocations are:

- core layout tags `1..=18`, excluding unallocated `4`, in
  `programs/solana-layout/src/lib.rs:270-286`;
- Reservation through RevenuePolicyRecord tags `19..=27` in their owning
  layout modules;
- ArtifactStage `0x21` in `programs/solana-layout/src/artifact.rs`;
- observation families `0x45`, `0x47`, and `0x48` in the reference/SBF
  observation adapters; and
- SourceSpec/SourceArchive families `0x71..=0x74` in the SBF source modules.

The authoritative central collision ledger now reserves these exact disabled
coordinates:

| Account family | Tag | Version | Exact bytes |
| --- | ---: | ---: | ---: |
| Position tombstone | `0x75` | 1 | 76 |
| General Epoch tombstone | `0x76` | 1 | 84 |

The ledger status is `ReservedDisabled`, not executable. The adapter regression
test cross-checks both retirement tombstone constants against the central
entries. Live integration still requires authoritative codec composition,
complete global collision coverage, and an enabled SBF route; a reserved pair
alone is not an activation.

The noncolliding promoted versions under existing tags are:

| Existing family | Current pair | Counted pair | Exact bytes |
| --- | --- | --- | ---: |
| Market | `(3, 1)` | `(3, 2)` | 734 |
| Position | `(6, 1)` | `(6, 2)` | 280 |
| General Epoch | `(11, 2)` | `(11, 5)` | 429 |
| Direct Reservation, frozen count-only | `(19, 2)` | `(19, 6)` | 627 |
| General Reservation, frozen count-only | `(19, 4)` | `(19, 5)` | 627 |
| Direct Reservation, deletable successor | `(19, 2)` | `(19, 8)` | 675 |
| General Reservation, deletable successor | `(19, 4)` | `(19, 7)` | 675 |

Epoch tag 11 already has direct versions 3 and 4. The audit therefore corrected
ADR-0007's draft general Epoch `(11,3)` to the first noncolliding counted
version, `(11,5)`, before any live allocation. The isolated `(11,5)` constant
remains non-wire until a coordinated central allocation; direct Epoch versions
3 and 4 must not be reinterpreted.

Reservation tag 19 requires historical, not merely current-decoder, accounting:
general V1 was introduced by `f428bd5`, direct V2 by `e9e8856`, general V3 by
`53dc33f`, and current general V4 by `41c231f`. A first draft incorrectly chose
direct V3 after observing only current direct V2/general V4; that would have
reinterpreted persisted historical bytes. Count-only general therefore uses V5
and count-only direct uses V6. Those 627-byte shapes are now frozen and do not
own deletion funding. All older values remain permanently burned for their
original shapes even when current decoders refuse them.

Fresh deletable general V7 and direct V8 append a 57-byte tail: the nine-byte
generation/count marker followed by the exact 48-byte payer/refundable-
principal/donation owner required for deletion. V5/V6 remain nine-byte-tail
count-only envelopes and retain their frozen pure transition behavior, but no
live route may use them for deletable creation or close. Hostile prefund
remains donation and never discounts the payer's full principal. The direct V2
base already contains a legacy funding ledger; the isolated adapter requires
its fields to match the V8 owner exactly.

Authoritative promotion would need a central maximum schema of at least 8 and
collision assertions for the complete historical pair ledger. Changing a
summary constant alone allocates nothing; V7/V8 remain codec-local today.

The inner intent ladder is contiguous through decimal 73 in
`programs/solana-layout/src/lib.rs:5335-5433`. The existing abstract actions
remain the right wires:

- `CloseGeneralEpoch = 67` should dispatch legacy V2 to its refusal and the
  future counted version to tombstoning;
- `ClosePosition = 69` should dispatch Position V1 to its refusal and Position
  V2 to tombstoning.

No new tag is required merely because the terminal representation changes by
authenticated account version. If a genuinely new action such as a standalone
reopen is adopted, decimal `74` (`0x4a`) is the first free inner-intent tag,
subject to a fresh whole-tree audit and central `Intent`/dispatcher/capability
allocation. It must not be confused with account tag `0x74` or the provisional
tombstone codecs whose coordinates are centrally reserved disabled.

## Exact composition and authentication order

For each promoted root, the adapter must:

1. derive the canonical PDA from the exact registered seed schema and program
   id;
2. compare the actual key, owner, writable bit, exact length, tag/version, and
   stored bump;
3. copy exactly the legacy-width prefix, restore only its legacy version byte,
   and run the existing authoritative base decoder;
4. decode the exact appended retirement tail;
5. cross-bind identities, generation, phase, and economics before calling a
   pure transition; and
6. produce every checked post-state and lamport balance before the first write.

The generic child seam accepts only a registry-supplied tag, legacy/counting
version pair, exact base width, and stored-bump offset. Its downgraded prefix
must still be decoded by that child's semantic owner. For candidate bundles,
every member (record, feed/stage, and funding owner) must eventually be
authenticated and created/closed atomically, while the Epoch count changes once
for the bundle.
Frozen General Epoch V5 encode/decode deliberately preserves the committed
accepted set: any nonzero retirement generation is codec-valid. The distinct
successor projection checks
`retirement.epoch_generation == base.epoch_index + 1`; an exhausted index or
independently chosen tail generation cannot enter successor transitions.

Public `Adapter*ProjectionV1` inputs are forgeable DTOs, not capabilities. The
isolated adapter currently implements an end-to-end exact account bridge only
for Direct Epoch V4. It projects all six exact lifecycle phases, and Direct V8
registration admits only pre-freeze-open; exact frozen, selected, settled, and
prefreeze-aborted parents refuse. Exact CandidateWindowV4, general Market/Realm
neutral-sink, Position/Replay identity, Replay absence, and Budget bridges are
activation blockers. `ValidatedAdmissionLedgerRetiredV1` has private fields and
validates complete pure Window terminal structure, but proves no runtime
owner/PDA/bytes.

## Atomic transition plans

Pure host plans are necessary but not sufficient. The corrected Position plan
requires its exact generation-scoped Replay sibling and precomputes Position
tombstoning, Replay deletion, and coalesced recipient credits as one unit. The
Epoch arithmetic models mandatory EpochWindow and Budget siblings with
disjoint rent compartments, but it is not an executable success plan. Root open
always returns `BudgetFundingUnauthenticated`, and root retirement always
returns `BudgetRetirementUnauthenticated`, until the authoritative Budget owner
supplies opaque capabilities covering reward liabilities, cleanup markers, and
every economic compartment. The eventual live handler ordering is:

```text
authenticate all accounts and immutable bindings
  -> reject source/source and source/recipient aliases
  -> decode and validate all base/tail values
  -> precompute every checked counter and economic post-state
  -> coalesce every funding debit or close credit by authenticated balance
  -> precompute payer, neutral-sink, and retained balances
  -> perform CPI/transfers/realloc/writes
  -> return success only after every postcondition is re-read or checked
```

Any failure in the last line must roll back every earlier runtime effect. Host
failure-injection tests prove the intended staging discipline; only local-bank
tests against a fresh ELF can prove Solana transaction rollback.

Position/Epoch retirement must leave:

```text
account_before
  = stored_payer_refund
  + permanent_tombstone_balance
  + neutral_sink_surplus
```

The neutral amount includes the persisted donation floor and every later
unsolicited lamport. Recipient additions are checked before mutation. Hoard
principal, collateral, rent principal, future fees, and liveness reserves are
not interchangeable compartments.

Open/reopen admission is bundle-level, not a set of independent successful
subtractions. When one payer funds multiple members, each admission must name
the same authenticated starting balance, the complete debit is coalesced, and
the combined subtraction must succeed before creation. Market/Epoch/Window/
Budget and Position/prior-Replay/next-Replay identities must be mutually
consistent and distinct from payer and sink roles. Close plans likewise reject
every source/source and source/recipient alias before computing credits.

## Replay successor activation blocker

The current reference Replay body is 84 bytes. Appending the required 48-byte
deletable funding owner yields a projected 132-byte generation-scoped
successor. The pure seam enforces exact Position identity/generation binding,
atomic close, checked reopen generation, sequence-zero recreation, and
full-principal hostile-prefund admission. Reopen carries a forgeable adapter
projection claiming absence of the prior-generation Replay PDA and a distinct
next-generation target; the pure plan cross-binds its semantic fields to the
Position tombstone. Exact system-owner/zero-data/PDA absence authentication is
not implemented and remains an activation blocker.

The central registry reserves Replay `0x7a/v1` as disabled. An in-flight
external general-v2 contract proposes its 132-byte shape, but that proposal is
not an exact retirement/reference composition codec or an SBF route here.
Before Position retirement can be enabled, those boundaries must land. The
existing PDA seed already includes Position generation, but that seed and every
split/merge/materialize/withdraw/resolution consumer must be re-audited and cut
over together. Founding Position creation must also create its Replay sibling
atomically. Legacy Replay routes and Position V1 remain fail-closed for
retirement.

## General SETTLED-phase activation blocker

The pure successor models all five authoritative Epoch phases. Order pages and
Reservations are created only in OPEN; candidate/index/verdict/escrow/
ClearWork bundles only in FROZEN; receipts and FinalPot only in CLEARED; child
rent cleanup and root retirement only in SETTLED or LAPSED. The current general
SBF lifecycle never stamps SETTLED and its legacy terminal-closure family uses
CLEARED as both settlement-working and cleanup phase. The counted successor
must add an authenticated, rollback-tested transition to SETTLED after every
economic settlement dependency is terminal. Until then, general counted close
routes remain STOP; the pure seam does not weaken terminality to fit the legacy
state graph.

## Exhaustive child and replay matrix

Every independently addressed child class has one authoritative count:

| Class | Increment | Exact-once decrement gate |
| --- | --- | --- |
| Candidate bundle | complete Begin/Submit bundle creation | full bundle present; canonical ClearWork absent |
| CandidateIndex page | actual page creation | authenticated page cleanup |
| CandidateVerdict | immutable verdict creation | verdict dependency exhausted |
| CandidateEscrow | escrow creation | every refund/slash/reward/claim terminal |
| ClearWork bundle | first-stage creation | growing or complete work safely closed |
| OrderPage | actual page creation | page economically empty/retired |
| Reservation archive | general reservation creation | terminal and `position_counted=0` |
| SettlementReceipt | first endpoint creation only | receipt dependency exhausted |
| FinalPot | unique pot creation | pot empty and settlement terminal |

Candidate admission nodes are owned by CandidateWindowV4's reverse-linked
admission ledger, not a retrofitted tenth frozen Epoch V1 count. Root close
requires the privately minted pure terminal-ledger witness and remains blocked
until an exact Window V4 runtime adapter exists.

Candidate status never changes the Candidate-bundle count. The lifecycle owner
validates status; retirement accepts only its opaque `(tag, version, status)`
witness. General and direct Reservations both increment the same Position
counter. The first terminal economic transition decrements once; archive rent
close never decrements Position again.

Direct V8 Reservation generation is derived through the exact authenticated
Direct Epoch V4 adapter bridge as checked `epoch_index + 1`; `u64::MAX`
refuses. The frozen V6 scalar-taking pure symbol retains its committed count
semantics, but is never successor generation authority. No live intent or
caller projection may choose successor generation.

## Required local-bank campaign

Before either close can be enabled, a fresh SBF ELF and retained transcript
must cover:

1. exact positive decode/encode for every full promoted account, the Replay
   successor, and every tombstone, plus wrong owner/PDA/bump/tag/version/length
   and alias negatives;
2. late failures after each transfer, CPI, realloc, and data write during
   general/direct registration, reservation terminality and rent deletion,
   child creation/close, Position+Replay close/reopen, and
   Epoch+Window+Budget open/close, including same-payer combined-debit
   shortfall and inconsistent starting balances;
3. an economically-zero all-in seller with both a general and direct live
   Reservation, exact count `2→1→0`, and replay refusal at each terminal step;
4. all admitted candidate lifecycle statuses, a no-work candidate, growing and
   complete ClearWork, out-of-registry candidates, and all nine child counts;
5. two same-cursor Epoch-open transactions: one success, one stale-cursor
   refusal after writable Market serialization, plus old-index/tombstone replay;
6. prefunded and unsolicited-lamport cases proving exact stored-payer refund,
   exact tombstone retention, and all surplus to the immutable neutral sink;
7. counter overflow/underflow, generation mismatch, terminal replay, withheld
   child, wrong parent/owner, wrong child class/phase, neutral-sink mismatch,
   and all source/target/recipient alias negatives with byte-identical
   prestates; and
8. fresh ELF digest correspondence plus compute-unit, stack, account-meta, and
   deployable capability-profile headroom.

Until every item passes, `ClosePosition` and `CloseGeneralEpoch` remain
fail-closed for every live schema.
