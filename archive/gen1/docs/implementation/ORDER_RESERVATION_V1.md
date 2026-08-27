# Funded order reservation V1

Status: **design frozen for the admission seam; runtime integration in
progress**.

This document owns one narrow prerequisite of the coupled venue: an accepted
order must already have one exact, owner-funded reservation.  It does not make
> **Current disposition (2026-08-19).** Production placement and cancellation
> now create and consume exact funded reservations, with real-SBF tests. A
> narrow full-fill direct single-Egg settlement consumer is also live; see
> [COUPLED_SETTLEMENT_V1.md](COUPLED_SETTLEMENT_V1.md). Candidate selection,
> general settlement, fee policy, lapse, and complete terminal closure remain
> outside this reservation document.

This design did not by itself make `SettlePage` reachable, select a candidate, or claim that the current fee
proposal is final.

## 1. One reservation identity

The canonical reservation identity is the domain-separated digest of

```text
(market, epoch, owner, position_generation, order_id)
```

and its PDA is derived from that digest.  The account additionally binds the
page index, order generation, immutable Terms/basis identity, price-grid
identity, policy identity, order family, side, and the owner's signed
`max_fee_atoms`.  The page is the public order commitment; the reservation
account is the asset owner.  A page by itself is never evidence that cash or
Eggs were reserved.  Binding Terms explicitly prevents the same coefficient
bytes from being replayed under another installed basis even though Epoch
identity already provides a second domain boundary.

`order_id` is the canonical positional rank already fixed by the page codec.
The reservation codec recomputes the page index from that rank, so a caller
cannot bind the same order to a different page.

## 2. Exact admission amounts

All arithmetic is checked.  Price-unit amounts use the epoch's frozen
`price_scale`; conversion into collateral atoms has one boundary:

```text
ceil_atoms(x) = ceil(x / price_scale)
```

For one single-Egg order of quantity `q` and limit `l`:

```text
buy:  reserved_cash = ceil_atoms(q * l) + max_fee_atoms
sell: reserved_egg[outcome] = q
```

For one portfolio order of `lots` and coefficient vector `a` over the exact
Egg basis installed by immutable Terms:

```text
buy:  reserved_cash = lots * limit_collateral_per_lot + max_fee_atoms
sell: reserved_egg[i] = lots * a[i]
```

Inactive Egg entries are canonical zero.  The buy formula is equivalent to
the relation's price-unit reservation
`lots * limit_collateral_per_lot * price_scale`, converted at the single named
boundary.  A product that does not fit the persisted `u64` balance refuses;
accepting it merely because an intermediate `u128` fits would create an order
that no account can fund.

The fee cap is an owner-authored maximum, not a caller assertion that a fee was
computed correctly.  A later candidate must recompute the fee from the exact
policy preimage and refuse a fill whose fee exceeds this cap.  Until the policy
preimage and reservation-set commitment are joined to the relation, settlement
remains fail-closed.

The coefficients may name categorical basis Eggs or exact native B-spline
basis products; reservation does not lower them to a one-hot representation or
reinterpret their payoff semantics.  It moves exactly the onchain Egg atoms
the signed intent names.

Sell-side fees are withheld from proceeds under the current proposed fee
direction, so sell admission reserves Eggs rather than free cash.  The signed
fee cap is still persisted and bound.  Changing the fee payer direction is an
ABI/economics decision, not an adapter convention.

## 3. Position and custody transitions

`PositionAccount.cash_atoms` remains total pooled trading cash and
`reserved_cash_atoms` remains the encumbered aggregate.  Placement requires

```text
reserved_cash_for_order <= cash_atoms - reserved_cash_atoms
```

and increases only `reserved_cash_atoms`.  The per-order account owns the
decomposition.  Cash never leaves the Hoard and Hoard locked backing does not
change.

Reserved internal Eggs are moved out of `PositionAccount.internal` into the
per-order reservation.  This makes existing split, merge, materialize, and
redemption paths unable to spend them without adding a second aggregate shadow
vector to Position.  Total market claim supply does not change:

```text
free internal Eggs + reserved Eggs + materialized Eggs = total supply
```

External Token-2022 Eggs are not accepted directly by the native venue.
Their holder must dematerialize them into the owning Position before placing a
sell.  No transfer hook or wallet balance is interpreted as an order escrow.

## 4. Cancellation and later settlement ownership

Cancellation is admitted only while both Epoch and page are open.  It checks
the live order, reservation, Position generation, signer, policy/grid binding,
and reservation PDA before any write.  It then:

1. retires the exact page slot;
2. subtracts only that reservation's remaining cash from the Position's
   reserved aggregate;
3. returns only that reservation's remaining Eggs to the Position; and
4. marks the reservation released with zero remaining assets.

An exact or conflicting replay sees a tombstone/released reservation and
refuses without another release.  Cancellation after page or Epoch freeze
refuses.  Expiry/lapse uses the same release transition, but no permissionless
lapse instruction is claimed until Epoch creation/freeze owns a canonical
clock and phase transition.

Future candidate selection must atomically move every selected reservation
from `ACTIVE` into an immutable entitlement ownership phase before resolution.
Only then may lazy post-resolution consumption occur.  Neither an order page
nor a candidate may debit an active reservation directly, and one byte may not
be both a reservation and a settlement pot.

## 5. Required falsifiers

- the same free cash or Egg atom reserved twice;
- cash reservation above `cash_atoms - reserved_cash_atoms`;
- coefficient, limit, fee-headroom, or aggregate overflow;
- wrong market, Epoch, owner, Position generation, order generation, page,
  policy, grid, reservation identity, PDA, or signer;
- an order page substituted without its reservation, or vice versa;
- cancel replay, conflicting release generation, and cross-order release;
- cancellation after Epoch/page seal;
- reserved cash consumed by Split or Withdraw;
- reserved Eggs consumed by merge, materialization, redemption, or a second
  order;
- external bearer Eggs offered without dematerialization; and
- a late account/CPI failure that preserves anything other than the complete
  pre-state.

Passing these tests establishes funded admission and release only.  It does
not discharge live-cardinality, policy-preimage, full-width relation identity,
checkpoint, candidate-selection, entitlement-freeze, settlement, liveness, or
release-manifest gates.
