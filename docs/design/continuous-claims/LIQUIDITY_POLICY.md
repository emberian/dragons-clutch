# Proof-constrained liquidity policy

Status: **EXECUTABLE MODEL / NO LIVE AUTHORITY** (2026-08-19). Commit
`e58a5a674d52d481343f304de4e7a7a16fe65193` hardens the isolated,
dependency-free, safe `no_std`/`no_alloc` model in
[`../../../research/liquidity-policy-model`](../../../research/liquidity-policy-model).
It changes no kernel, account layout, SBF route, batch authority, mint
authority, or release claim. `MAX_QUOTES = 8` is a bounded schedule witness, not
a continuous or always-available AMM.

> **Supersession notice.** The original proposal's bare `R >= H(q)` reserve
> check and `withdraw <= R - H(q)` formula remain useful only after all pending
> quotes are cleared. They are insufficient admission rules while quotes can
> execute. The model now uses the non-netted encumbrance `E = B + H(q+s)` and
> permits withdrawal only from `R-E`, additionally capped by pro-rata equity.
>
> The predecessor model's multi-holder interpretation and repeated exact-
> rational carry/checkpoint story are also superseded. V1 now has one immutable
> beneficial owner per tranche and exactly one bounded terminal fee allocation.
> It makes no transferable-share or recurring-fee-allocation claim.

## V1 choice: bounded schedule-compiled range liquidity

An LP selects an immutable native market/Terms identity, payoff region,
complete quote schedule, per-Egg inventory limits, collateral cap, epoch
interval, fee policy, withdrawal policy, and compiler version. The pure compiler
accepts a hard range, an index triangle, or an exact canonically padded
coefficient vector. That exact-vector arm can carry output from the separate
shape compiler after a future admission layer chooses integer units.

The compiler emits at most eight ordinary Portfolio-shaped plans. Each plan
contains side, exact Egg coefficients per lot, lot count, all-in collateral
bound, minimum partial fill, valid epoch interval, and replay generation, and
copies the policy, tranche, market, Terms, payoff-region, and complete-schedule
identities.

Compilation preflights `coefficient * lots` on both buy and sell sides and also
proves that the sum of all sell floors fits the reserve numeric domain.

```text
LiquidityPolicyV1 (MODEL CONTENT; NO CANONICAL LIVE CODEC) {
  policy_id,
  market,
  terms_digest,
  native_basis_degree,
  outcome_count,
  payout_denominator,
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

The model checks nonzero identities and copied equality, but does not derive or
authenticate those digests. A live adapter still needs canonical policy and
schedule bytes, cryptographic identity derivation, and a bounded proof that an
individually replenished plan belongs to the authenticated complete schedule.

A precommitted future rung may be admitted later. Any unplanned repricing, new
shape, quantity, expiry, or refinement requires a new authenticated
schedule/policy, normally under a successor epoch or tranche. No keeper may
mutate stored quote semantics.

## Reserve, inventory, and ownership

For the full integer payout simplex, nonnegative written inventory `q` has
exact conservative support function

```text
H(q) = max_i q_i.
```

Let pending sell exposure be aggregated componentwise before taking one
maximum,

```text
s_i = sum_r coefficients[r][i] * remaining_lots[r],
```

and let every pending buy reserve its entire cash ceiling,

```text
B = sum_r buy_limit[r] * remaining_lots[r].
```

Let the minimum cash proceeds promised by all pending writes be

```text
P = sum_r sell_limit[r] * remaining_lots[r].
```

The model maintains

```text
E(q,s,B) = B + H(q+s)
R >= E
E <= collateral_cap
q+s <= max_inventory       (componentwise)
pending_buy_inventory <= q (componentwise)
R + P <= 10^12
```

There is no buy/sell netting, no offset based on nonoverlapping quote windows,
and no assumption that a keeper will later cancel risk. Sell fills move exposure
from `s` to `q`; buy-backs can remove only already written inventory. The
policy is not mint authority. The additional `R + P` headroom makes every
admitted write executable at its floor without overflowing the reserve. A fill
releases its filled floor before adding actual proceeds and must preserve
headroom for every remaining floor; cancellation or lapse only reduces `P`.

A live sell must reserve exact Eggs already owned by the tranche, or obtain
them through the canonical fully collateralized Split/Materialize path before
its Reservation activates. The same collateral atom cannot simultaneously be
tranche reserve `R` and Hoard principal. If live Split consumes tranche cash,
the adapter must explicitly reclassify cash and Egg backing and re-prove the
withdrawal invariant. The simpler integration keeps `R` separate and requires
delivery Eggs to be additionally preowned.

Each tranche remains segregated: one immutable policy, one immutable beneficial
owner, inventory, reserve, nontransferable accounting-share supply, risk
weight, terminal fixed-grid carry, generation, and terminal ledger. Different
owners require different tranches. No tranche may withdraw another tranche's
assets, Hoard backing, venue cash, keeper funds, fee pot, or carry escrow.

## Deposits, shares, and withdrawals

Ignoring pending quotes for valuation, conservative equity is

```text
Q = R - H(q) + fractional_fee_carry.
```

After the first one-share-per-atom deposit, a deposit of `d` by the immutable
owner mints `m` accounting shares only when

```text
m = d * share_supply / Q
```

is a positive exact integer. Issuance also requires `epoch <= batch_end` and
zero live inventory/reservations. No second beneficial owner exists in this
state space. A withdrawal burning `b` shares may take at most

```text
min(floor(Q * b / share_supply), R - E).
```

The last share cannot leave reserve, inventory, pending reservations, risk
weight, or fractional carry ownerless. Partial withdrawal order is not claimed
to be invariant: at `R=10, S=3`, burns of one then two shares may return `3+7`,
while two then one may return `6+4`. Both paths return the same immutable
owner's ten atoms. Multiple owners, transferable shares, or per-holder residual
rights require a successor model and proof.

Elapsed capital-at-risk is integrated before a share change. Because ownership
cannot change, partial share changes cannot transfer historical fee rights to a
different owner. Fee outputs still bind the exact share supply, reserve,
replay counters, fee-window end, and tranche generation. These are model
equations: a live route must authenticate the owner on deposit/withdrawal and
bind generation/replay. It must not reinterpret the accounting units as a
transferable mint.

## Exact execution and fee allocation

The model stages the complete post-state and commits only after validation. It
covers schedule/rung admission, partial fills, cancellation, strictly
post-expiry lapse, inventory-bounded buy-back, exact integer-simplex settlement,
and refusal transactionality. It refuses self-cross, limit violations,
underfunding, replay, overflow, and unnamed fractional settlement.

Risk weight is the time integral of the same non-netted encumbrance, with the V1
multiplier exactly one. Pending quote risk stops after its inclusive expiry;
written inventory remains a settlement liability until offset or settlement,
but accrues no fee weight after `batch_end + 1`. A future live weight may accrue
only while authoritative frozen-page provenance proves the quote was present
and executable. Pure model admission is not page-availability evidence.

V1 admits exactly one fee allocation, only after every tranche's immutable
`batch_end + 1`. Every input has zero prior carry, zero prior allocation
generation, and a common fee-policy/snapshot/window identity. The complete set
first aggregates raw risk weights by immutable beneficial owner. If an owner
has several tranche inputs, its credit and carry are recorded on that owner's
lexicographically smallest tranche identity.

Let `U = 10^12`, owner weights be `W_i`, and `W = sum_i W_i > 0`. Hamilton
normalization computes

```text
base_i = floor(U * W_i / W)
remainder_i = (U * W_i) mod W
left = U - sum_i base_i.
```

One additional unit goes to each of the `left < owner_count` largest
remainders, with lexicographically smaller owner identity winning an exact tie.
The normalized units `u_i` sum to `U`. For terminal fee pot `F <= U`,

```text
x_i = F * u_i / U
credit_i = floor(x_i)
carry_i = x_i - credit_i.
```

This is a named bounded apportionment, not exact representation of the raw
ratio `W_i/W`. Each distinct owner's total value differs from its raw-weight
quota by less than one collateral atom. Whole credits are paid directly to the
bound owner rather than silently increasing tranche reserve. Every nonzero
carry has denominator `U`; the complete batch checks

```text
F = sum_i direct_owner_credit_i + retained_carry_escrow.
```

Outputs bind allocation, owner, tranche, fee policy, snapshot/window epochs,
share supply, reserve, replay counters, tranche generation, integrated weight,
and zero old carry. This remains pure arithmetic. A live fee authority must
authenticate the exhaustive unique-tranche set and owner bindings, consume the
single realized pot once, own the retained carry escrow, atomically pay every
credit, and apply every output once. No second allocation can consume terminal
carry. If any carry remains, the model fail-closes by locking the last shares;
release needs a separately frozen, funded terminal-carry rule, not assumed
future volume.

## Frozen arithmetic domain

V1 deliberately restricts its numeric domain:

- collateral atoms, inventory, accounting shares, fee pot, reserve, each
  tranche's integrated weight, and the fee grid are at most `10^12`;
- at most eight tranche inputs compose to aggregate fee weight `8 * 10^12`;
- `(batch_end - batch_start + 1) * collateral_cap <= 10^12`; and
- every nonzero terminal carry denominator is exactly `10^12`.

The resulting share, normalization, fee, and payout products stay below the
proved `u128` envelope; oversized inputs refuse before multiplication. Common-
denominator integer addition makes the single complete terminal allocation
input-order independent. This bounded domain is part of the model semantics,
not an implementation hint or a claim of arbitrary-precision liquidity.

## Optional future cost-function policy

A cost-function maker is a separate future policy family. It must expose one
canonical integer potential `C_hat` and charge only endpoint differences:

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

Independent price overlays and dynamic per-region depth do not satisfy this
gate. The landed schedule model has no endogenous curve or potential and makes
no continuous-availability claim.

## No insurance promise in V1

An LP loss floor is itself a contingent liability. It is admissible only when a
separate reserve covers the joint worst case across every protected tranche, or
Terms freeze a precise priority/haircut rule which never mutates live promises.
Future trading fees, token fees, future volume, and governance action are not
reserve. The landed model contains no insurance, floor, leverage, margin,
liquidation, bailout, or future-fee receivable.

## Live integration STOPs

Before passive liquidity can be called live, the adapter needs:

1. canonical policy/schedule codecs, cryptographic digest derivation, and
   membership verification;
2. a program-owned tranche PDA/account with exact owner, length, bump, version,
   rent, generation, and replay rules;
3. atomic sell-Egg and buy-cash Reservation funding without Hoard/reserve
   double-counting;
4. frozen-page provenance, candidate selection, partial allocation, vector
   receipt/entitlement creation, and terminal Reservation closure; Direct V2 is
   single-Egg-only and its top-three Select is a measured compute STOP, while
   staged Direct V3 remains model-only;
5. single terminal fee-pot and carry-escrow custody, exhaustive unique-tranche
   and owner aggregation, atomic direct-credit/application authority, replay
   protection, and a funded terminal-carry rule;
6. immutable beneficial-owner authentication on deposit, cancellation, and
   withdrawal; V1 accounting shares must remain nontransferable and cannot be
   projected into a multi-holder token;
7. authenticated native Resolution binding under a registered production
   source, fractional settlement policy, terminal token burns, and collateral
   transfer;
8. account-size, rent, CU, stack, blank-bank, signed-walk, and hostile runtime
   evidence, including authenticated rent principal, third-party donation
   separation, and replay-safe close/tombstone routes;
9. explicit terminal disposition for Hoard-token donations, external claim-burn
   forfeiture, fractional fragments, and current outcome mints that lack
   `MintCloseAuthority`; and
10. named Verus/Rocq refinement if a formal claim is desired.

The successor's debug and release campaigns pass 20/20 tests; strict Clippy,
rustdoc, and an external six-case hostile harness are green. The independent
verdict is scoped to the frozen isolated model diff. It is not evidence that a
deployed program offers passive liquidity, that any live authority exists, or
that the relation is formally verified.
