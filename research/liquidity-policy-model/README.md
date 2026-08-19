# Proof-constrained liquidity-policy model

Status: **MODEL**, offline only. This crate is safe Rust, `no_std`, `no_alloc`,
fixed-capacity, float-free, dependency-free, and unpublished. It implements the
accounting semantics proposed by
[`LIQUIDITY_POLICY.md`](../../docs/design/continuous-claims/LIQUIDITY_POLICY.md)
without changing the kernel, Solana account layouts, SBF program, batch
authority, or any release claim.

The crate models one immutable beneficial owner as a segregated, fully
capitalized underwriter. An
immutable `LiquidityPolicyV1` binds the market and complete Terms digest,
native B-spline degree 0 through 3, outcome count, payout denominator, payoff
region, complete schedule, per-Egg inventory limits, collateral cap, epoch
interval, fee policy, withdrawal policy, and compiler version. A distinct
immutable tranche identity and owner bind every compiled plan and state
transition. V1 shares are nontransferable accounting units for that one owner,
not multi-holder tokens. Multiple owners require separate tranches.

`MAX_QUOTES = 8` is a finite fixed-capacity research witness, not equivalence
to a continuously available AMM. Quotes have finite lots, exogenous limits,
explicit epochs, keeper-dependent admission, batch execution, and eight
persistent lifetime slots. There is no endogenous curve, canonical potential,
or always-on quote guarantee. A cost-function maker remains a separate future
policy family.

## Exact V1 semantics

The bounded compiler accepts:

- a constant hard range over native Eggs;
- a triangle sampled at native Egg indices, with the one named floor operation
  at each rational interpolation; or
- an exact, canonically padded coefficient vector produced by the existing
  offline shape compiler (including capped linear, exact samples, and admitted
  kernel tables).

It emits ordinary Portfolio-shaped plans: side, active length, exact Egg
coefficients per lot, lot count, all-in collateral bound, minimum partial fill,
valid epoch interval, and replay generation. Plans copy the policy, tranche,
market, Terms, payoff-region, and complete-schedule identities. Compilation
checks the entire bounded schedule under the conservative assumption that all
sell capacity and all buy cash ceilings are simultaneously live. Both buy and
sell coefficient vectors are multiplied by lots and checked against the
inventory domain before any plan can be admitted. The sum of every sell floor
times lots must also fit the reserve numeric domain.

More precisely, simultaneous sell rungs are summed componentwise before one
support-function maximum, while every buy ceiling is reserved in full:

```text
schedule sell liability = max_i sum_r(coeff[r][i] * lots[r])
schedule buy cash       = sum_r(limit[r] * lots[r])
schedule encumbrance    = sell liability + buy cash
schedule sell floor     = sum_r(sell_limit[r] * lots[r])
```

There is no buy/sell netting. The compiler applies this even to rungs whose
active intervals do not overlap. A future rung already signed into the same
immutable schedule may be admitted later. Any unplanned repricing, new shape,
quantity, expiry, or refinement requires a newly authenticated schedule/policy
(normally a successor epoch/tranche); it cannot mutate a stored live plan.

The policy is not mint authority. A future adapter must materialize or reserve
the exact sell-side Eggs through the existing authorized Split/Position path
before it places the ordinary Portfolio order. The pure model represents that
precondition as a segregated liability reservation; it cannot create a claim.

For current written inventory `q`, pending sell inventory `s`, pending buy cash
`B`, and LP reserve `R`, the model uses the full-simplex support function

```text
H(q) = max_i q_i
E = B + H(q + s)
R >= E
E <= collateral_cap
q + s <= max_inventory       (componentwise)
reserved_buy_inventory <= q  (componentwise)
R + pending_sell_floor_cash <= 10^12
```

`E` is deliberately stronger than the bare `R >= H(q)` invariant. A pending
buy is not assumed to offset inventory before it fills, and a pending sell is
not ignored merely because a keeper may later withhold or cancel it. Free
collateral is exactly `R - E`; withdrawals are also capped by the holder's
pro-rata conservative equity entitlement. In V1 that holder is the tranche's
single immutable owner. The sell-floor headroom is separate from liability: it
ensures every admitted write remains executable at its floor without making a
future receipt overflow the reserve. A higher clearing price is accepted only
when it also leaves headroom for every remaining sell floor.

The policy cap also bounds fresh reserve contributions. Realized sell proceeds
may take `R` above that policy cap, but never above the frozen arithmetic-domain
cap and never authorize a larger encumbrance. Whole-atom fee credits are paid
directly from the realized-pot authority to the immutable owner; they are not
silently reinvested into `R`.

The state machine supports:

- exact pro-rata deposits by the immutable owner, refusing any unnamed share
  remainder, any live exposure, and every issuance after `batch_end`;
- atomic schedule or individual-rung admission;
- partial and complete fills with exact cash/Egg receipts;
- owner-bound cancellation and strictly post-expiry permissionless lapse;
- inventory-bounded buy-backs, never long or leveraged positions;
- quote-aware free-collateral withdrawal;
- exact integer-simplex settlement, refusing a fractional collateral atom;
- time-integrated capital-at-risk weights; and
- one terminal fixed-grid fee allocation with named whole-atom carry escrow.

Every mutating method stages a complete copied post-state, validates it, and
commits only on success. Self-cross fills refuse before state mutation and
therefore earn zero weight or fee credit.

Risk weight uses the same non-netted buy-cash-plus-liability encumbrance, with
the V1 multiplier frozen to one. Each pending quote contributes only through
its inclusive expiry. Written inventory remains a settlement liability until
offset or settlement, but its fee weight stops at the immutable fee-window end
`batch_end + 1`. Policy validation proves that the maximum window times the
collateral cap fits the per-tranche weight domain. Because the beneficial owner
is immutable, partial deposits and withdrawals cannot transfer historical fee
rights to another owner; fee outputs nevertheless bind the exact share supply,
reserve, replay counters, and tranche generation. The last share stays locked
while weight or carry remains. A zero-pot allocation can close terminal weight
without assuming future volume.

Raw risk weights are deterministically normalized onto a common `10^12`-unit
grid after aggregation by owner. Floors are assigned first; at most seven
remaining units go by descending raw remainder, then lexicographically smaller
owner identity. Every admitted fee set has unique tranche identities. Before
rounding, it aggregates all
entries with the same immutable beneficial owner. One owner's credit and carry
land on that owner's lexicographically smallest tranche identity, so splitting
one owner across entries cannot gain rounding units. The owner-grid units sum
to exactly `10^12`, and every nonzero carry uses that same denominator.

This is an explicit bounded apportionment rule, not a claim that `W_i/sum(W)` is
represented with unbounded rational precision. Each distinct owner's terminal
quota differs from its raw-weight quota by less than
`fee_pot/10^12 <= 1` collateral atom. V1 admits exactly one allocation, only
after `batch_end + 1`, so this bounded apportionment error cannot repeat or
compound. Inputs must have zero carry and no prior allocation generation.
Across the exhaustive recipient set,
new fixed-grid carries must sum to whole atoms. Whole credits are direct owner
payouts. The allocation batch identifies the retained fee-authority escrow and
checks exactly

```text
new realized pot = whole credits + retained carry escrow.
```

If terminal fractional carries remain, the last shares stay locked. No second
allocation is allowed and this model does not assume another fee pot will
arrive; a live release needs a separately frozen, funded terminal carry rule.

## Frozen arithmetic domain

V1 intentionally refuses a broad integer domain it cannot prove. Collateral,
inventory, shares, fee pots, and reserve are at most `10^12`; per-tranche
capital-time weight is at most `10^12`, while an eight-recipient allocation may
sum to `8*10^12`; and every nonzero carry denominator is exactly `10^12`.
Policy validation also requires

```text
(batch_end - batch_start + 1) * collateral_cap <= 10^12.
```

These bounds put the largest mint and withdrawal numerator below
`10^36 < 2^120`; fixed-grid weight normalization and fee multiplication remain
below `8*10^24`. Carry summation is common-denominator integer addition, so the
single terminal allocation is input-order independent over the complete
bounded input set.

V1 deliberately makes no multi-holder exit-order claim. For example, at
`R=10, S=3`, burning one share and then two can pay `3,7`, while burning two and
then one can pay `6,4`. Because the same immutable owner receives every V1
withdrawal, both partitions return that owner's same ten atoms. Transferable or
multi-owner shares require a successor model with authenticated holder balances
and explicit per-holder residual ownership.

## Explicit exclusions

V1 has no leverage, margin borrowing, liquidation, dynamic liquidity
parameter, cost-function pricing, uncapitalized insurance, future-volume
assumption, creator-fee assumption, governance bailout, or LP loss floor.
Reserve is tranche-owned value. It is never Hoard principal, a keeper
endowment, an insurance reserve, or a fee pot. Realized fee pots may be
allocated only after they exist.

The crate does not claim formal verification. The checked algebraic arguments,
assumptions, and adversarial coverage are listed in
[`PROOF_ARGUMENTS.md`](PROOF_ARGUMENTS.md). The unverified adapter and authority
work needed before live use is exact in
[`MODEL_BOUNDARY.md`](MODEL_BOUNDARY.md).

## Verification

```sh
cargo test --manifest-path research/liquidity-policy-model/Cargo.toml
cargo clippy --manifest-path research/liquidity-policy-model/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path research/liquidity-policy-model/Cargo.toml --no-deps
```

The 20-test integration suite covers all native degrees, compiler goldens and
hostile padding, an exhaustive small full-simplex liability campaign, cash/Egg
conservation, partial fill/cancel/lapse/settlement, inventory-bounded buy-back,
withdrawal bounds, exact share issuance, fill splitting, range refinement,
capital homogeneity, funded terminal carry and final-share lock, replay refusal,
self-cross, limit,
overflow, fractional settlement, cached-ledger mutants, same-owner withdrawal
partitions, fixed-grid terminal permutation invariance and repeat refusal,
noncanonical carry refusal, late/exposed issuance refusal, buy coefficient
overflow, sell-proceeds headroom, direct fee payout at the reserve boundary,
same-owner aggregation and split neutrality, aggregate weight composition, and
an exhaustive small terminal fee-weight/pot escrow campaign.
