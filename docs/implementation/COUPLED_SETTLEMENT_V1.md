# Coupled settlement V1: narrow direct consumption seam

Status date: 2026-08-19.

This note describes one executable settlement subset. It is not a complete
clearing lifecycle, candidate-selection mechanism, entitlement freeze, or
permissionless venue.

## 1. Exact admitted subset

`Intent::SettlePage` consumes exactly one already-frozen pairing receipt when
all of these facts hold:

- the Epoch is `CLEARED` and the Candidate is `SELECTED`;
- the CandidateFeed is the canonical PDA for `(epoch, candidate)`, verifies as
  a whole, and binds field-for-field to the Candidate and Epoch order set;
- the named pairing slice has two real order legs, not a virtual split/merge;
- both relation indices land on the one supplied frozen page, whose page-set
  commitment is the Epoch commitment and which contains no tombstones;
- both records are single-Egg, opposite-side, distinct-owner, same-outcome,
  full-fill orders; each order quantity, candidate fill, and slice quantity is
  exactly equal;
- both Position accounts and both exact per-order reservation PDAs bind the
  same market, epoch, Terms, grid, policy, owner, generation, order and page;
- both reservations are `ACTIVE`, unchanged from admission, and carry zero fee
  authorization;
- the selected price satisfies both limits, and
  `quantity * price / price_scale` is exact in collateral atoms; and
- the canonical receipt PDA is the immutable direct entitlement for precisely
  this slice, order pair, outcome, quantity, price and consideration. Its
  sequence is `slice_index + 1`, and it is still wholly unconsumed.

The fixed account list is Epoch, Candidate, CandidateFeed, one order page,
buyer Position, seller Position, buyer reservation, seller reservation, and
receipt. No settlement signer exists: once the prerequisite entitlement is
frozen, consumption is permissionless.

## 2. Atomic transition

Placement left buyer cash in its Position but marked the exact envelope as
reserved; it moved seller Eggs from the Position into the sell reservation.
Settlement performs the coupled transition:

```text
buyer.cash             -= consideration_atoms
buyer.reserved_cash    -= buyer_reservation.remaining_cash
buyer.internal[outcome] += quantity
seller.cash            += consideration_atoms

both reservations.remaining = 0
both reservations.state     = CONSUMED
receipt.settled_quantity     = receipt.quantity
receipt.consumed_flags       = BUY | SELL | SLICE_EXHAUSTED
```

The unused difference between the buyer's admitted limit envelope and actual
consideration remains in `buyer.cash` and becomes free when the complete
reservation is released. The seller's reserved Eggs move exactly once to the
buyer. There is no token CPI and no Hoard movement: this is an internal
ownership reclassification under the already-backed pooled-custody equation.

Every fallible identity, arithmetic and post-state validation runs over staged
values before account write-back. Solana transaction rollback is the final
adapter boundary for an unexpected late data borrow/write refusal.

For the admitted transition:

```text
buyer cash + seller cash                     is conserved
buyer internal_i + seller internal_i
  + sell-reservation remaining_i             is conserved
fee revenue                                  changes by zero
Hoard tokens and locked complete-set backing do not change
```

The page supplies authenticated order definitions. It is not settlement
authority: a page without the selected CandidateFeed slice, exact reservations
and exact receipt cannot move value.

## 3. Explicit refusals

The seam refuses, without partial account writes:

- an Epoch not cleared or a Candidate not selected;
- a noncanonical/substituted feed, page, Position, reservation or receipt PDA;
- a stale, released, entitled or consumed reservation;
- a replayed/exhausted receipt;
- a virtual leg, portfolio, partial fill, multi-slice order, zero quantity,
  cross-page pair, tombstoned page, same owner, same-side pair, or cross-outcome
  pair;
- a candidate price outside either order limit;
- nonzero fee headroom, because no frozen fee-policy preimage and recipient
  transition exists yet;
- a non-integral price-unit conversion; and
- every checked underflow/overflow or post-state invariant failure.

Narrow refusal is intentional. In particular, rounding a consideration here
would silently select a receipt-level rounding boundary and pot policy that the
protocol has not frozen.

## 4. Runtime evidence

The focused host tests in
`program/src/instructions/orders_batch/settlement.rs` exercise success,
conservation, one-shot consumption, stale selection, partial fill,
cross-outcome substitution and nonzero-fee refusal.

The real-bank consumption campaign is
`svm-tests/tests/coupled_settlement.rs`. It loads the Candidate as already
selected and the receipt at genesis because verification/selection and receipt
freeze are still missing, then executes the actual SBF ELF. The separate
`SubmitDirectPage` constructor and its evidence are documented in
`COUPLED_AUTHORITY_V1.md`; its output remains `SUBMITTED` and cannot reach this
seam by itself. Against the joined working tree on 2026-08-19:

```text
ELF sha256: 07b759e09867a13a89b6f0c27fdfb3f65b03fb4a2e186b94ea5ac87a21ac80a3
successful SettlePage transaction: 862,084 CU
program log for that instruction: 861,934 CU
focused real-SBF tests: 2 passed, 0 failed
```

The campaign verifies a valid transition, substituted buyer/seller Position
refusal, stale Candidate refusal, cross-outcome refusal and double-settlement
refusal. Each negative case compares all five writable accounts byte-for-byte.

This is `SBF-EXECUTED` focused evidence for that exact local ELF. It is not a
deployment, audit, proof, blank-bank lifecycle, or evidence for any broader
settlement shape. The SBF build still reports pre-existing large-frame
diagnostics in unused buffered/reference functions; the live settlement frame
diagnostic was removed before this evidence run.

## 5. Remaining STOPs

The full venue remains STOP until all of the following exist and execute:

1. complete candidate submission-window closure, relation verification,
   comparison and one onchain `SELECTED` transition;
2. authenticated general CandidateFeed/ClearWork construction beyond the
   narrow two-order submitted-feed constructor;
3. a complete reservation-set commitment proving every selected order has one
   funded reservation;
4. complete immutable receipt/FinalPot construction before resolution;
5. cumulative per-order/per-slice state for partial and multi-slice fills;
6. portfolio and virtual split/merge consumption with exact pot accounting;
7. one frozen fee policy, recipient and rounding/carry boundary;
8. cancelled/tombstoned and cross-page live-index semantics;
9. permissionless lapse/refund and a terminal sweep proving every reservation,
   receipt and pot balance is consumed or returned exactly once; and
10. a blank-bank lifecycle and joined release evidence over the final ELF.

The existence of this consumption seam must not be summarized as “onchain
clearing is implemented.” It means only that one narrow, preauthorized coupled
entitlement can now be consumed safely.
