# On-chain settlement checkpoint: preflight landed, economic transition STOP

Status: **IMPLEMENTED byte-level preflight; STOP before relation execution**.

The old “39 KiB batch verifier cannot fit SBF” diagnosis is no longer the
active blocker. `relation_v1_stream` reduces the largest measured relation
frame to 1,280 bytes and has a bounded host equivalence gate. What is missing
is the lossless, funded, stable-byte join from Solana accounts into that
relation and from an accepted relation verdict into frozen settlement
entitlements.

`programs/clutch-sbf/program/src/instructions/orders_batch/settlement.rs` now
contains the smallest safe executable seam:

1. decode the epoch, candidate, feed, and ClearWork header through their owning
   codecs;
2. verify and recompute the **complete** frozen page set, not one page which
   merely claims the epoch's order-set identity;
3. bind that set to the frozen epoch;
4. bind a `SUBMITTED` candidate to the epoch;
5. bind every shared candidate/feed coordinate and claimed score component;
6. bind the feed to the recomputed order set;
7. bind the checkpoint header to `(market, epoch, candidate, page cursor)`; and
8. write nothing.

A successful preflight is not a candidate verdict. The production
`SettlePage` branch still returns `NotYetImplemented` before reading accounts,
so this preflight does not freeze an account ABI which the missing state owners
may later invalidate.

## Ranked prerequisites

These are dependency ordered. None may be bypassed by an adapter convention.

| rank | prerequisite | current counterexample | required owner/change |
| ---: | --- | --- | --- |
| 1 | funded reservations | `PlaceOrder` writes no Position/Hoard reservation; settling its fills would create an unbacked debit | order admission + collateral plane; reserve atomically and release on cancellation/unfilled refund |
| 2 | live cardinality after cancellation | `EpochAccount.order_count` is populated slots including tombstones; candidate/feed `order_len` is live orders; `CandidateRecord::binds_epoch` requires equality to the former | layout semantic owner: persist or derive one exact live count, then bind candidate once there |
| 3 | frozen policy preimage | epoch stores `policy: Hash32`; `RelationDomainV1` needs every `FrozenPolicyV1` variant and fee parameter | a versioned policy account/codec whose digest is the epoch's policy |
| 4 | lossless domain identities | onchain market/book/policy/order-set identities are `Hash32`; relation V1 takes four `u64`s | relation revision to carry full identities, or another demonstrably injective representation; truncation is forbidden |
| 5 | portable checkpoint bytes | `ClearWorkV1` is `repr(Rust)` and contains enums/bools; layout calls its 48,592-byte body opaque | explicit versioned byte codec or a byte-native streaming state; no reference cast into hostile bytes |
| 6 | authenticated initialization | CandidateFeed has no init instruction/PDA; ClearWork needs five realloc steps and rent top-ups | seed schema plus resumable create/realloc/init transitions whose partial states always refuse |
| 7 | candidate-set closure | verifying one candidate does not prove it is the best valid submitted candidate | bounded submission-set commitment, deterministic comparison, one selected-candidate transition |
| 8 | entitlement freeze | FinalPot and SettlementReceipt codecs exist, but no transition creates a complete immutable entitlement set | pre-resolution receipt/pot construction and closure proof, then candidate/epoch phase transition |

The executable `SETTLEMENT_BLOCKERS` constant and fail-closed checkpoint test
pin this order. `advance_relation` returns rank 1 and leaves the checkpoint
byte-identical.

## Cancellation finding

The adversarial test builds a valid two-slot frozen page with one retirement.
The epoch truth is `order_count = 2`; the relation truth is one live order. A
candidate using `order_len = 1` fails the current semantic-owner binding, while
one using `order_len = 2` cannot bind the one-element feed. The preflight does
not weaken either check. This is a schema decision, not an adapter bug.

The likely repair is to add `live_order_count` to the epoch freeze result,
computed from every page's authenticated `tombstone_count`, while retaining the
existing populated-slot count for page closure. That is only a recommendation;
the layout owner must freeze the exact field and migration.

## Pre-resolution freeze, post-resolution consumption (PROPOSED)

Resolution must not wait for every user to consume a lazy settlement. The safe
phase order is:

```text
Frozen epoch
  -> candidate relation verified
  -> best valid submitted candidate selected
  -> complete pot + receipt entitlement set frozen
  -> resolution may occur
  -> each previously frozen receipt may be consumed once (T-b)
  -> pot/receipt closure
```

“T-b” here authorizes only consumption of an entitlement already fixed before
resolution. Resolution cannot create a receipt, change its candidate, outcome,
quantity, price, consideration, sequence, or counterparties. Generic
post-resolution position mutation remains forbidden. An entitlement set must
also be complete before the epoch becomes clearable: otherwise an executor
could omit losing legs and preserve only favorable receipts. Receipt
consumption needs a candidate/slice replay domain distinct from the existing
per-owner request replay domain, with at-most-once consumption. Closure must
also account for every unfilled reservation and release unused cash or claims
exactly once; an authenticated order page alone is never settlement authority.

This direction remains **PROPOSED** until ranks 1–8 land. The current code does
not claim to create, consume, or close an entitlement.

## Properties exercised now

The focused tests cover:

- complete-page inclusion and epoch binding;
- candidate/feed/checkpoint identity binding;
- candidate status gating;
- page, feed-order-set, and checkpoint-order-set tampering;
- the tombstone cardinality STOP;
- exact-replay idempotence;
- conflicting-replay refusal atomicity; and
- relation-phase unreachability while a prerequisite is missing.

They do **not** execute `relation_v1_stream`, prove its batch equivalence,
measure compute units, create Solana accounts, reserve collateral, select a
candidate, create receipts, or settle balances.

Focused offline gate:

```sh
cargo test --manifest-path programs/clutch-sbf/Cargo.toml \
  --offline --locked settlement
cargo clippy --manifest-path programs/clutch-sbf/Cargo.toml \
  --offline --locked -p clutch-sbf --all-targets -- -D warnings
```
