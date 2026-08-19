# Proof-constrained liquidity-policy model

Status: **MODEL**, offline only. This crate is safe Rust, `no_std`, `no_alloc`,
fixed-capacity, float-free, dependency-free, and unpublished. It implements the
accounting semantics proposed by
[`LIQUIDITY_POLICY.md`](../../docs/design/continuous-claims/LIQUIDITY_POLICY.md)
without changing the kernel, Solana account layouts, SBF program, batch
authority, or any release claim.

The crate models an LP as a segregated, fully capitalized underwriter. An
immutable `LiquidityPolicyV1` binds the market and complete Terms digest,
native B-spline degree 0 through 3, outcome count, payout denominator, payoff
region, complete schedule, per-Egg inventory limits, collateral cap, epoch
interval, fee policy, withdrawal policy, and compiler version. A distinct
immutable tranche identity and beneficial owner bind every compiled plan and
state transition.

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
sell capacity and all buy cash ceilings are simultaneously live.

More precisely, simultaneous sell rungs are summed componentwise before one
support-function maximum, while every buy ceiling is reserved in full:

```text
schedule sell liability = max_i sum_r(coeff[r][i] * lots[r])
schedule buy cash       = sum_r(limit[r] * lots[r])
schedule encumbrance    = sell liability + buy cash
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
```

`E` is deliberately stronger than the bare `R >= H(q)` invariant. A pending
buy is not assumed to offset inventory before it fills, and a pending sell is
not ignored merely because a keeper may later withhold or cancel it. Free
collateral is exactly `R - E`; withdrawals are also capped by the holder's
pro-rata conservative equity entitlement.

The cap also bounds fresh reserve contributions. Realized sell proceeds or fee
credits may take `R` above the cap, but never authorize a larger encumbrance.

The state machine supports:

- exact pro-rata deposits, refusing any unnamed share remainder;
- atomic schedule or individual-rung admission;
- partial and complete fills with exact cash/Egg receipts;
- cancellation and strictly post-expiry lapse;
- inventory-bounded buy-backs, never long or leveraged positions;
- quote-aware free-collateral withdrawal;
- exact integer-simplex settlement, refusing a fractional collateral atom;
- time-integrated capital-at-risk weights; and
- exact reduced rational fee carry across realized fee-pot allocations, with
  named whole-atom carry escrow in each allocation batch.

Every mutating method stages a complete copied post-state, validates it, and
commits only on success. Self-cross fills refuse before state mutation and
therefore earn zero weight or fee credit.

Risk weight uses the same non-netted buy-cash-plus-liability encumbrance, with
the V1 multiplier frozen to one. Each pending quote contributes only through
its inclusive expiry; written inventory remains at risk until offset or exact
settlement. Deposit and withdrawal refuse while historical weight is
uncheckpointed, so share changes cannot acquire or abandon prior fee rights. A
zero-pot allocation can close such a checkpoint without assuming future
volume.

Fractional fee rights are not future-fee receivables. Across the exhaustive
recipient set, old and new fractional carries must each sum to whole atoms. The
allocation batch identifies the prior and retained fee-authority escrow and
checks exactly

```text
new realized pot + prior carry escrow = whole credits + retained carry escrow.
```

If terminal fractional carries remain, the last shares stay locked. This model
does not assume another fee pot will arrive; a live release needs a separately
frozen, funded terminal carry rule.

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

The 15-test integration suite covers all native degrees, compiler goldens and
hostile padding, an exhaustive small full-simplex liability campaign, cash/Egg
conservation, partial fill/cancel/lapse/settlement, inventory-bounded buy-back,
withdrawal bounds, exact share issuance, fill splitting, range refinement,
capital homogeneity, persistent fee carry, replay refusal, self-cross, limit,
overflow, fractional-settlement, cached-ledger mutants, every three-holder
withdrawal order, and a two-round small fee-weight/pot escrow campaign.
