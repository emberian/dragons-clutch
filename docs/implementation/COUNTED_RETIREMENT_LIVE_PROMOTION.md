# Counted-retirement live-promotion plan

Status: **composition/authentication seam implemented; no live allocation,
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

No scalar account tag in that committed tree, or in the separately scanned
in-flight worktree, uses `0x75` or `0x76`. The recommended authoritative central
reservations are therefore:

| Account family | Tag | Version | Exact bytes |
| --- | ---: | ---: | ---: |
| Position tombstone | `0x75` | 1 | 76 |
| General Epoch tombstone | `0x76` | 1 | 84 |

Those numbers remain **provisional and non-wire** in `clutch-retirement` until
the central layout account registry exports them and a global collision test
covers every decentralized account module. A handler-local constant is not an
allocation. The two tombstone codecs must move or be re-exported through that
semantic owner before live integration.

The noncolliding promoted versions under existing tags are:

| Existing family | Current pair | Counted pair | Exact bytes |
| --- | --- | --- | ---: |
| Market | `(3, 1)` | `(3, 2)` | 734 |
| Position | `(6, 1)` | `(6, 2)` | 280 |
| General Epoch | `(11, 2)` | `(11, 5)` | 429 |
| Direct Reservation | `(19, 2)` | `(19, 6)` | 627 |
| General Reservation | `(19, 4)` | `(19, 5)` | 627 |

Epoch tag 11 already has direct versions 3 and 4. The audit therefore corrected
ADR-0007's draft general Epoch `(11,3)` to the first noncolliding counted
version, `(11,5)`, before any live allocation. The isolated `(11,5)` constant
remains non-wire until a coordinated central allocation; direct Epoch versions
3 and 4 must not be reinterpreted.

Reservation tag 19 requires historical, not merely current-decoder, accounting:
general V1 was introduced by `f428bd5`, direct V2 by `e9e8856`, general V3 by
`53dc33f`, and current general V4 by `41c231f`. A first draft incorrectly chose
direct V3 after observing only current direct V2/general V4; that would have
reinterpreted persisted historical bytes. Counted general therefore uses V5
and counted direct uses V6. All four older values remain permanently burned for
their original shapes even when current decoders refuse them.

Authoritative promotion must also raise the central maximum-schema
`LAYOUT_VERSION` from 4 to 6 and add collision assertions for the complete
historical pair ledger. Changing that summary constant alone allocates nothing.

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
tombstone tags.

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
every member (record, feed/stage, and funding owner) is authenticated and
created/closed atomically, while the Epoch count changes once for the bundle.

## Atomic transition plans

Pure host plans are necessary but not sufficient. The current plan values cover
the Position or Epoch root account's post-state and lamport split only. Before
Epoch promotion, the adapter needs one additional complete plan for the
mandatory EpochWindow and funding-identity closures, including account aliases,
recipients, and late CPI failures. The live handler ordering is:

```text
authenticate all accounts and immutable bindings
  -> decode and validate all base/tail values
  -> precompute every checked counter and economic post-state
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

Candidate status never changes the Candidate-bundle count. The lifecycle owner
validates status; retirement accepts only its opaque `(tag, version, status)`
witness. General and direct Reservations both increment the same Position
counter. The first terminal economic transition decrements once; archive rent
close never decrements Position again.

## Required local-bank campaign

Before either close can be enabled, a fresh SBF ELF and retained transcript
must cover:

1. exact positive decode/encode for every full promoted account and every
   tombstone, plus wrong owner/PDA/bump/tag/version/length and alias negatives;
2. late failures after each transfer, CPI, realloc, and data write during
   general/direct registration, reservation terminality, child creation/close,
   Position close/reopen, and Epoch close;
3. an economically-zero all-in seller with both a general and direct live
   Reservation, exact count `2→1→0`, and replay refusal at each terminal step;
4. all admitted candidate lifecycle statuses, a no-work candidate, growing and
   complete ClearWork, out-of-registry candidates, and all nine child counts;
5. two same-cursor Epoch-open transactions: one success, one stale-cursor
   refusal after writable Market serialization, plus old-index/tombstone replay;
6. prefunded and unsolicited-lamport cases proving exact stored-payer refund,
   exact tombstone retention, and all surplus to the immutable neutral sink;
7. counter overflow/underflow, generation mismatch, terminal replay, withheld
   child, and wrong child-class negatives with byte-identical prestates; and
8. fresh ELF digest correspondence plus compute-unit, stack, account-meta, and
   deployable capability-profile headroom.

Until every item passes, `ClosePosition` and `CloseGeneralEpoch` remain
fail-closed for every live schema.
