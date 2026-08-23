# Exact portfolio execution authority V2

Status: **IMPLEMENTED PURE ACCOUNT CONTRACT; SBF ROUTE NOT YET INTEGRATED**
(2026-08-23).

This module supersedes the authority claims of the historical model-only
`portfolio_settlement` seam. The old seam remains reference code, but its
content digest, entitlement value, and mutable host structs are not accepted by
this contract as account authentication, selected-candidate authority, or
replay evidence.

## Semantic ownership

- `EconomicOrderV2` is the sole owner of the exact `[u64; 16]` coefficient
  vector, quantity, minimum, AON policy, expiry, side, and `u128` limit.
- `SelectedPortfolioOrderRecordV2` owns only authenticated consumption
  coordinates: Market/Epoch/candidate/order-set identities, selected Feed and
  settlement witness, page account/index/slot/generation, dense RelationV2
  order index, order/owner identities, Position account/incarnation, and exact
  selected fill.
- General Reservation V9, Position V3, and rent-owned SettlementReceipt V5
  codecs remain the persisted account owners. This module accepts only exact
  pre/post semantic identities confirmed by the adapter and derives the value
  effects they must encode.
- Purpose Replay V3 owns the per-Position call ordinal. The pair receipt binds
  each exact pre ordinal and its checked `+1` successor.
- `PortfolioPairReceiptV2` is the canonical vector transition-receipt preimage.
  Its semantic hash is retained in the counted SettlementReceipt V5 postimage;
  it is not a new account, rent obligation, or portfolio bearer claim.

The adapter is an explicitly unverified boundary. Its implementation of
`PortfolioAdapterV2` must check actual program owner, canonical PDA/bump,
complete hostile body decode, semantic body identity, generation, privileges,
and exact canonical postimage derivation. A mock implementation is test
scaffolding, not runtime evidence.

## Admitted pair

V2 admits exactly one shape:

1. two authenticated Portfolio page memberships from the same selected
   candidate, order set, selected Feed, and settlement witness;
2. one buy and one sell with distinct order IDs, owners, and Position accounts;
3. no virtual split or merge and no other nonzero candidate fill;
4. each selected fill equals that order's full RelationV2 quantity;
5. all sixteen coefficient cells are exactly equal, including canonical inactive
   padding; and
6. both RelationV2 limit checks pass under the one authenticated simplex.

The exact pair vector is `payoff_i = coefficient_i * units`. The cash value is

```text
unit_value_price_units = sum_i coefficient_i * price_i
total_value_price_units = unit_value_price_units * units
consideration_atoms = total_value_price_units / price_scale
```

Every multiplication and sum is checked at its current RelationV2 width. There
is one conversion boundary, `ExactReceiptDivisionV1`. If the last division has
a remainder or the quotient does not fit `u64` collateral atoms, the pair is
refused. There is no per-leg division, per-order dust, residual pot, or caller
rounding selector.

## Atomic effects

The buyer Reservation must contain at least exact consideration cash, no native
Eggs, and zero fee ceiling. The seller Reservation must contain exactly the
full 16-wide payoff, no cash, and zero fee ceiling. Both must be ENTITLED and
bound to the selected order, owner, Position account, and stable Position
incarnation generation.

On success:

- buyer Position cash decreases by consideration;
- buyer Position reserved cash decreases by the whole remaining Reservation
  envelope, leaving the difference as unlocked refund cash;
- buyer Position native Eggs increase by the exact payoff vector;
- seller Position cash increases by consideration;
- seller Position native Eggs do not change because the sold Eggs already live
  in its Reservation;
- both Reservation postimages are CONSUMED with zero remaining cash/Eggs;
- neither Position incarnation generation nor outstanding Reservation count
  changes during filled settlement; the later counted Reservation retirement
  owns child-count decrement and rent refund;
- each Position-purpose Replay advances by exactly one; and
- the rent-owned, counted SettlementReceipt V5 postimage retains the semantic
  hash of the canonical 680-byte vector transition-receipt preimage, whose
  transition ID binds all exact prestates, effects, and successors.

The claim debit and credit arrays are byte-identical, so this transfer changes
neither aggregate native-Egg supply nor ClaimLedger liability. Hoard principal,
fees, rent, and rewards are not sources of consideration.

## Authored refusal evidence

The unexecuted adversarial suite in `src/portfolio_execution_v2_tests.rs`
covers all 16 outcomes, single-boundary exact valuation, remainder refusal,
coefficient mutation at the last outcome, canonical codec padding, Reservation
underfunding, nonzero fee refusal, seller-claim substitution, account and
transition authentication refusal, Replay overflow, and receipt identity
sensitivity to Replay prestate. Per task direction, no build or test command was
run while authoring this slice.

## Remaining live blockers

1. General must project OrderPage V5, the newly frozen rent-owned General
   Reservation V9 (`0x13/9`, 666 bytes), and rent-owned SettlementReceipt V5
   (`0x0f/5`, 265 bytes) into these exact selection and transition expectations
   without a parallel coefficient DTO.
2. Candidate finalization must make the counted SettlementReceipt V5 the sole
   persistence owner for the vector transition-receipt semantic hash and bind
   it to the exact SettlementCandidate, witness, pair endpoints, and ordinal.
3. The SBF handler must atomically compose two Reservation V9 CONSUMED
   postimages, two Position V3 postimages, two Replay V3 successors, the
   SettlementReceipt V5 successor, and the collateral/native-Egg transfers. No
   partial write may be observable.
4. Dealer settlement must join its private verified allocation row to the
   private selected-order capability. Caller-authored cash, Egg arrays, or
   coefficients remain forbidden.
5. Nonzero portfolio fees remain refused until the owner-scoped fee/carry
   semantic owner can produce authenticated exact atom effects at this boundary.
6. Partial, nonexclusive, mixed single/portfolio, virtual split/merge, and
   multi-pair candidates need separately bounded receipt graphs and measured
   SBF routes. They must continue to refuse rather than reuse this full-pair
   capability.
7. The 536-byte membership and 680-byte receipt codecs need hostile SBF frame,
   PDA, owner, privilege, rent, rollback, and compute evidence before any release
   manifest can name the route available.
