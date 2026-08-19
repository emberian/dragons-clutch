# Exact model and integration boundary

This crate is an offline semantic model. It is intentionally isolated from
Solana SDK, Token-2022, account memory, CPI, clocks, oracle code, the batch
solver, and cryptographic dependencies. No function here is a live authority.

## Facts owned by the model

- validation of the bounded `LiquidityPolicyV1` field set;
- deterministic hard-range, index-triangle, and exact-vector compilation;
- copied policy/tranche/Terms/schedule bindings on every quote plan;
- conservative full-integer-simplex liability `max_i(q_i)`;
- aggregate sell-Egg, minimum-proceeds headroom, and buy-cash quote
  reservations;
- segregated tranche reserve, inventory, single-owner nontransferable accounting
  shares, risk weight, fee carry, local generation, and terminal settlement
  ledger;
- staged partial-fill, cancel, lapse, withdrawal, and settlement arithmetic;
  and
- one terminal fixed-grid fee-pot apportionment for a supplied complete input
  set, including direct owner credits and physically conserved whole-atom carry
  escrow.

The eight-rung schedule is only a bounded witness. It is not a continuous AMM,
does not expose a cost potential, and does not promise continuous availability.
Simultaneous sells aggregate componentwise before one maximum; buys reserve
their full cash ceiling without netting, and both sides preflight
`coefficient*lots`. Active sell floors reserve numeric proceeds headroom, so an
admitted write is executable at its floor. A precommitted future rung may be
admitted later, but any quote change absent from the authenticated artifact
requires a new schedule/policy transition rather than in-place mutation.

The admitted numeric domain is part of V1, not an implementation hint: atoms,
shares, per-tranche weight, and the fixed fee grid are capped at `10^12`;
fee-window duration times collateral cap must also fit `10^12`. Up to eight
tranches compose to aggregate allocation weight `8*10^12`. The one terminal
allocation uses a named Hamilton normalization and never claims unbounded
rational precision.

## Existing semantic owners to reuse

A live implementation should translate, not duplicate, these existing facts:

| Model object | Existing/future owner |
|---|---|
| native degree, Egg count, denominator, Terms digest | `clutch-solana-layout::TermsAccount` |
| ordinary coefficient quote | `clutch-solana-layout::PortfolioRecord` |
| buy cash or sell Egg envelope | `clutch-solana-layout::reservation::ReservationPlan` and `ReservationAccount` |
| LP cash/Egg custody and generation | authenticated program-owned Position/tranche accounts |
| sell-side Egg creation | canonical fully collateralized Split/Materialize authority, never this policy |
| exact portfolio valuation and allocation | `clutch-batch` candidate verifier |
| paired coefficient transfer preflight | `clutch-solana-layout::portfolio_settlement` after its listed runtime blockers close |
| native payout vector | authenticated resolved Kernel/Terms state |
| token movements and supply checks | SBF adapter plus exact post-CPI reloads |

`PortfolioQuotePlanV1` deliberately resembles the economic fields of an
ordinary `PortfolioRecord`, but it is not a second persisted codec. Promotion
should add the smallest policy/tranche account codec and translate a checked
plan into the existing order/reservation owners. The live relation must not
accept caller-constructed model structs as authority.

## Authority still missing

Before any live route, the adapter must provide and test all of the following:

1. A canonical policy byte encoding and cryptographic `policy_id` derivation.
   This model only refuses zero IDs and checks copied equality.
2. Authentication that the payoff-region and complete quote-schedule artifact
   bytes match their digests, compiler version, and policy.
3. A bounded membership proof that an individually replenished plan belongs to
   that authenticated schedule. Copied schedule-digest equality alone is not
   membership authority.
4. A program-owned tranche PDA/account codec whose policy and tranche identity
   cannot be substituted, with rent, owner, length, bump, version, and replay
   checks. Asset-changing instructions must bind the expected tranche
   generation so a retried fill/cancel/withdraw cannot consume a later state;
   cancellation must additionally authenticate the immutable beneficial owner.
   Post-expiry lapse may remain permissionless. The model's monotone generation
   is evidence, not a live replay gate.
5. Atomic funding of sell plans by exact Eggs already owned by the tranche, or
   by a canonical Split that moves collateral to Hoard and receives those Eggs
   before the Reservation becomes active. The liquidity policy never mints.
   A direct refinement may not count one collateral atom simultaneously as
   tranche `R` and Hoard principal: if Split consumes tranche cash, the live
   account model must distinguish/reclassify cash and canonical Egg backing and
   re-prove the withdrawal invariant. The simpler alternative is that modeled
   `R` remains a separate capital reserve and delivery Eggs are additional
   preowned assets.
6. Atomic funding of buy plans from tranche cash, with exact reservation
   ownership and post-transfer reloads.
7. Frozen-page provenance, candidate selection, exact partial allocation,
   portfolio receipt/entitlement initialization, and terminal reservation
   closure in the existing batch state machine.
8. A fee-pot authority that authenticates the realized pot, the exhaustive set
   of unique tranche inputs and their bound beneficial owners, common fee
   policy/snapshot/window epoch, and zero prior allocation/carry. It must
   consume the single terminal pot
   once, implement the frozen fixed-grid/tie rule, own the retained carry-escrow
   atoms reported by the batch, atomically pay every whole credit directly to
   its bound owner, and apply every output once. No projected volume is an
   input, and no second allocation is allowed. Capital-at-risk time may accrue
   only for quotes proven present and executable in the authoritative frozen
   page interval; pure model admission is not page-availability evidence. A
   separately frozen terminal carry rule is still required when funded
   fractional carries remain; the model fail-closes by locking the last shares.
9. Authentication of the one immutable beneficial owner on every deposit and
   withdrawal. V1 intentionally has no holder-balance ledger and no transfer
   policy: its shares are internal accounting units, and different owners need
   different tranches. The terminal allocator aggregates duplicate owners over
   the complete input set before rounding and assigns each owner's credit and
   carry to its lexicographically smallest tranche identity. The live authority
   must prove that set exhaustive so no owner's tranche weight is omitted.
   Adding multi-owner or transferable shares is a successor model/account
   version requiring named per-holder residual claims
   and an exit order-invariance proof. It cannot be enabled only in an adapter.
10. Resolution authentication, exact payout-vector binding, fractional payout
    policy, terminal token burns, and collateral transfer. The model's exact
    settlement intentionally refuses an unnamed fractional atom.
11. Compute, account-size, rent, CU-headroom, final-LTO stack, blank-bank,
    signed-transaction, and adversarial mutation evidence at frozen bounds.
12. Verus/Rocq refinement of the named algebraic invariants if a formal claim is
    desired, including the unverified runtime and adapter assumptions.

Until those authorities exist, this crate is evidence that the accounting
relation is finite, deterministically rounded, and implementable—not evidence
that passive liquidity is available in a deployed program.

## Economic exclusions preserved at integration

The adapter must reject any attempt to reinterpret this model as leverage,
uncollateralized inventory, an insurance promise, a dynamic `b`, a second
pricing curve, a future-fee receivable, or a right to Hoard principal. A future
cost-function policy is a separate family and must pass the independent convex
potential and worst-case-loss gates in the design document.
