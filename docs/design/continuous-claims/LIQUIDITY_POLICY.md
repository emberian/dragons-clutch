# Proof-constrained liquidity policy

Status: **PROPOSED**. This is the missing passive-range-liquidity product seam.
It does not alter Hoard accounting, resolution, or the current batch relation.

## V1 choice: schedule-compiled range liquidity

An LP selects an immutable payoff/risk region, collateral cap, quote schedule,
expiry, and fee policy. An offline constructor compiles that choice into a
bounded set of ordinary portfolio orders. The onchain relation accepts a quote
only when its full worst-case delivery is already reserved.

```text
LiquidityPolicyV1 {
  policy_id,
  market_terms_digest,
  payoff_region_digest,
  quote_schedule_digest,
  max_inventory[MAX_OUTCOMES],
  collateral_cap,
  batch_start,
  batch_end,
  fee_policy_id,
  withdrawal_policy_id,
  compiler_version,
}
```

The policy is a quote-generation right, not mint authority. Candidate quotes are
untrusted until the batch verifier recomputes eligibility, valuation, allocation,
fees, and per-asset conservation.

## Reserve and ownership

For tranche inventory `q`, reserve `R`, and payout polytope `K`, define

```text
H_K(q) = sup_{p in K} q dot p.
```

Every transition maintains `R >= H_K(q)`. Hoard principal is separate and may
only satisfy canonical Egg liabilities. LP reserve, venue cash, fee pots,
keeper endowment, and insurance are distinct ownership domains.

V1 uses segregated tranches because range-local attribution is auditable:

- each tranche has one immutable policy, inventory, collateral reserve, share
  supply, fee carry, and settlement ledger;
- every fill is assigned to exactly one tranche or to ordinary counterparty
  flow;
- no tranche can withdraw another tranche's reserve; and
- a quote which would exceed the tranche's inventory or collateral cap refuses.

## Deposits, shares, and withdrawals

Deposits mint tranche shares against one frozen equity convention. A withdrawal
may pay no more than the holder's pro-rata entitlement and no more than free
collateral:

```text
withdraw <= R - H_K(q).
```

Inventory, reserve, shares, fee carry, and policy version are snapshotted in one
atomic transition. If no free collateral exists, withdrawal waits until quotes
expire, inventory is offset, or settlement completes. Transferable LP shares do
not imply that underlying reserve is withdrawable.

## Exact fee allocation

Candidate fee weights use time-integrated capital at risk:

```text
W_i = integral(capital_at_risk_i * frozen_multiplier_i) dt
fee_i = floor_with_persistent_carry(fee_pot * W_i / sum_j W_j).
```

The relation must prove pot conservation, capital homogeneity, position-split
invariance, equivalent-range-refinement invariance, and zero reward for
self-cross volume. No expected fee stream capitalizes present liabilities.

## Optional future cost-function policy

A cost-function maker is a separate policy family. It must expose one canonical
integer potential `C_hat` and charge only endpoint differences:

```text
charge(q, delta) = C_hat(q + delta) - C_hat(q).
```

Admission requires:

1. convexity/cyclic monotonicity on the admitted inventory domain;
2. prices/subgradients inside the actual payoff polytope;
3. complete-set cash invariance;
4. a finite worst-case-loss certificate capitalized before the first trade;
5. canonical rounding/carry so split trades telescope; and
6. immutable parameters, or a separately capitalized value-transfer rule for
   every parameter change.

Different per-bin `b_i` softmax quotes and simultaneous LMSR-plus-sigmoid
pricing do not satisfy this gate. A dynamic liquidity parameter is economic
state, not a display knob.

## No insurance promise in V1

A claimed LP loss floor is itself a contingent liability. It is admitted only
if a separate reserve covers the joint worst case across all protected tranches,
or if terms freeze a precise priority/haircut rule which never mutates live
promises. Future trading fees, token creator fees, and governance action are not
reserve.

## Batch integration

The existing frequent batch remains the authoritative execution relation:

```text
policy compiler -> candidate portfolio orders -> frozen epoch pages
                -> untrusted candidate -> exact verifier -> lazy settlement
```

The policy signs market, epoch range, portfolio artifact, all-in price bounds,
maximum quantity, expiration, fee policy, and tranche identity. A keeper may
submit or replenish quotes but has no power to change their signed semantics.

## Release gates

- independent schedule compiler and golden vectors;
- exact reserve accounting through partial fills, cancellations, retries, and
  batch finalization;
- refusal transactionality for every arithmetic and ownership failure;
- adversarial split-position, self-cross, empty-range, quote-withholding, and
  withdrawal-order tests;
- elasticity/fee simulation clearly separated from solvency evidence; and
- Verus/Rocq refinement plus SBF account/compute benchmarks at frozen bounds.
