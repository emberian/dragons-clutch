# Exact portfolio execution authority V2

Status: **IMPLEMENTED PURE ACCOUNT CONTRACT; SBF ROUTE NOT YET INTEGRATED**
(2026-08-23).

This module supersedes the authority claims of the historical model-only
`portfolio_settlement` seam. The old seam remains reference code, but its
content digest, entitlement value, and mutable host structs are not accepted by
this contract as account authentication, counted SettlementRoot authority, or
replay evidence.

## Semantic ownership

- `EconomicOrderV2` is the sole owner of the exact `[u64; 16]` coefficient
  vector, quantity, minimum, AON policy, expiry, side, and `u128` limit.
- `SelectedPortfolioOrderRecordV2` owns only authenticated consumption
  coordinates: Market/Epoch/candidate/order-set identities, counted
  SettlementRoot prestate, retained Feed account/body and traversal index,
  settlement witness, page account/index/slot/generation, dense RelationV2
  order index, order/owner identities, Position account/incarnation, and exact
  selected fill.
- General Reservation V9, Position V3, and 298-byte rent-owned SettlementReceipt V5
  codecs remain the persisted account owners. This module accepts only exact
  pre/post semantic identities confirmed by the adapter and derives the value
  effects they must encode.
- Purpose Replay V3 owns the per-Position call ordinal. The pair receipt binds
  each exact pre ordinal and its checked `+1` successor.
- `PortfolioPairReceiptV2` is the canonical 680-byte vector-transition
  preimage. The counted SettlementReceipt V5 must already be
  `PortfolioPairPending` (persisted kind `1`, zero commitment) after both ends
  are accounted. Atomic delivery derives the V5-owned domain hash and changes
  that same receipt to `PortfolioPairCommitted(nonzero)` exactly once;
  it is not a new account, rent obligation, or portfolio bearer claim.

The adapter is an explicitly unverified boundary. Its implementation of
`PortfolioAdapterV2` must check actual program owner, canonical PDA/bump,
complete hostile body decode, semantic body identity, generation, privileges,
and exact canonical postimage derivation. A mock implementation is test
scaffolding, not runtime evidence.

The adjacent `portfolio_book_v2` module owns the complete-book consumption
boundary requested by Dealer. Its 600-byte fixed record binds the same
read-only SettlementRoot and retained Feed traversal to an active prefix of at
most four authenticated OrderPage V5 accounts (16 slots each, 64 rows total).
The caller supplies no `EconomicBookV2`; only the authenticated page adapter
may construct the owner-blind book carried by the private capability.

## Admitted pair

V2 admits exactly one shape:

1. two authenticated Portfolio page memberships from the same counted
   SettlementRoot prestate, order set, retained Feed traversal, and settlement witness;
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
- an endpoint with no Position value or child-count change retains its exact
  pre semantic identity even though its purpose Replay still advances;
- each Position-purpose Replay advances by exactly one; and
- the rent-owned, counted SettlementReceipt V5 postimage retains the canonical
  V5-domain commitment to the 680-byte vector-transition preimage, whose
  transition ID binds all exact prestates, effects, and successors; and
- SettlementRoot remains read-only during delivery; `slice_index:u16` is exact
  retained-Feed traversal evidence and Receipt sequence remains exactly
  `u64::from(slice_index) + 1`.

The claim debit and credit arrays are byte-identical, so this transfer changes
neither aggregate native-Egg supply nor ClaimLedger liability. Hoard principal,
fees, rent, and rewards are not sources of consideration.

## Authored refusal evidence

The unexecuted adversarial suite in `src/portfolio_execution_v2_tests.rs`
covers all 16 outcomes, single-boundary exact valuation, remainder refusal,
coefficient mutation at the last outcome, canonical codec padding, Reservation
underfunding, nonzero fee refusal, seller-claim substitution, account and
transition authentication refusal, Replay overflow, pending-kind/accounted-end
and pre-delivery Receipt V5 refusals, and transition-commitment sensitivity to
Replay prestate. Per task direction, no build or test command was run while
authoring this slice.

## Remaining live blockers

1. The General runtime adapter must project OrderPage V5, counted
   SettlementRoot/retained Feed traversal, Reservation V9 (`0x13/9`, 666
   bytes), and the now-frozen SettlementReceipt V5 (`0x0f/5`, 298 bytes) into
   these expectations without a parallel coefficient DTO.
2. That adapter must invoke V5's canonical
   `commit_portfolio_pair_delivery`, reproduce the V5-owned transition hash
   from the exact 680-byte preimage, and authenticate the derived V5 post-data
   ID. Caller-selected receipt kinds or hashes remain forbidden.
3. The SBF handler must atomically authenticate the read-only SettlementRoot and compose two Reservation V9 CONSUMED
   postimages, two Position V3 postimages, two Replay V3 successors, the
   SettlementReceipt V5 successor, and the collateral/native-Egg transfers. No
   partial write may be observable.
4. Dealer settlement must consume `AuthenticatedCompletePortfolioBookV2` to
   derive its full allocation order set, then join each verified allocation row
   to the private selected-order capability. Caller-authored books, cash, Egg
   arrays, or coefficients remain forbidden.
5. Nonzero portfolio fees remain refused until the owner-scoped fee/carry
   semantic owner can produce authenticated exact atom effects at this boundary.
6. Partial, nonexclusive, mixed single/portfolio, virtual split/merge, and
   multi-pair candidates need separately bounded receipt graphs and measured
   SBF routes. They must continue to refuse rather than reuse this full-pair
   capability.
7. The 568-byte membership and 680-byte receipt codecs need hostile SBF frame,
   PDA, owner, privilege, rent, rollback, and compute evidence before any release
   manifest can name the route available.
