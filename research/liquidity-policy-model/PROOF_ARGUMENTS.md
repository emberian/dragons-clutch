# Checked algebraic arguments

Status: **MODEL ARGUMENTS**, not a machine-checked formal-verification claim.
The executable owner is `src/lib.rs`; the adversarial witnesses are in
`tests/semantics.rs`. A future Verus or Rocq refinement must state the exact
source digest, toolchain, assumptions, and adapter boundary before using the
word “verified.”

## 1. Full-simplex maximum liability

Assume active inventory coefficients `q_i >= 0` and a payout vector
`w_i >= 0` with `sum_i w_i = D > 0`. Let `M = max_i q_i`. Then

```text
sum_i q_i w_i <= sum_i M w_i = M D,
```

so the collateral payout `sum_i q_i w_i / D` is at most `M`. The integer
simplex contains every vertex `w_j = D, w_i = 0 (i != j)`, so choosing an index
where `q_j = M` attains the bound. Therefore the conservative full-simplex
support function is exactly `H(q) = max_i q_i`.

The exhaustive test enumerates every denominator-6 simplex vector for every
three-coordinate inventory in `[0,4]^3`, checks the inequality, and constructs
an attaining vertex. Reachable degree-1 through degree-3 B-spline weight sets
may be strict subsets of the simplex; V1 deliberately does not take credit for
that without a separate reachable-polytope proof.

## 2. Pending-quote reserve invariant

Define `s_i = sum_r c[r][i] lots[r]` componentwise across pending sell rungs,
the full sum `B = sum_r limit[r] lots[r]` across pending buys, and

```text
E(q,s,B) = B + H(q+s).
```

Admission requires `R >= E`, `E <= collateral_cap`, `q+s <= max_inventory`,
and pending buy Eggs componentwise at most `q`.

No rung-local maxima are added: all sells form `s` before the one maximum.
Conversely, every buy contributes its full ceiling to `B`; no anticipated buy
fill is netted from `q` or `s`.

- Sell fill of vector `x` changes `(q,s)` to `(q+x,s-x)`, so `q+s` and `H`
  are unchanged. Cash consideration only increases `R`.
- Sell cancellation/lapse changes `s` to `s-x`, so `H(q+s)` cannot increase.
- Buy fill of `x` changes `q` to `q-x`, releases its ceiling `C`, and debits
  actual consideration `A <= C`. Thus new reserve is `R-A >= R-C`; `H` cannot
  increase when nonnegative `x` is removed, and remaining `B` falls by `C`.
- Buy cancellation/lapse releases `C` and changes no liability.

Every successful transition therefore preserves `R >= E`, while the staged
post-state independently recomputes the cached reservation vectors and bound.
The sell proof also establishes exact reservation/inventory conservation:
moving a filled vector from `s` to `q` neither creates nor loses an Egg
obligation. Fill receipts name the corresponding collateral and Egg transfer.

## 3. Withdrawal and shares

Ignoring pending quotes for valuation, conservative equity is

```text
Q = R - H(q) + fractional_fee_carry.
```

An established tranche with share supply `S` mints `m` shares for deposit `d`
only when

```text
m = d S / Q
```

is a positive exact integer. This makes pre- and post-deposit conservative
equity per share identical. No floor remainder is silently transferred between
LP cohorts.

A withdrawal burning `b` shares may take at most `floor(Q b / S)` whole atoms
and no more than `R-E`. Subtracting no more than `R-E` directly preserves
`R' >= E`. The last share cannot leave reserve, inventory, active reservations,
or fractional carry without an owner.

Before any share mint or burn, elapsed capital-at-risk is integrated. If its
weight is nonzero, the share transition refuses until an allocation checkpoint
consumes that weight. Fee inputs and private outputs bind exact share supply
and tranche generation, so a same-epoch state change invalidates a stale
output. A zero-pot checkpoint is valid and clears weight without predicting a
future fee.

## 4. Split and refinement invariance

For fills at the same epoch, scalar/vector addition gives

```text
c(a+b) = ca + cb
price(a+b) = price*a + price*b.
```

Thus a single fill and any partition into partial fills have identical reserve,
inventory, and reservation post-state. The test compares `10` lots with `4+6`.

For schedule refinement inside one tranche, quote reservations are aggregated
componentwise before applying `H`. Replacing coefficient vector `c` by exact
pieces whose sum is `c` leaves `q+s`, `E`, free collateral, and integrated risk
weight unchanged. The test compares one two-Egg hard range with two disjoint
exact-vector rungs.

Scaling every coefficient/lot exposure by a positive integer scales `H` and
therefore `H * elapsed` by the same factor. The V1 multiplier is exactly one; a
non-unit multiplier belongs in a future authenticated fee-policy preimage, not
beside its identity. The capital homogeneity test checks a factor of two.

Risk integration is piecewise at every active quote's `expiry+1` boundary.
Pending sell and buy reservations count only while their quote can execute;
inventory created by a fill continues to count. The hostile lapse witness waits
from epoch 10 to 1,000 for a quote expiring at 15 and checks that it receives
weight only for `[10,16)`.

## 5. Fee-pot conservation and persistent carry

For a realized pot `F`, weights `W_i`, total `W = sum_i W_i > 0`, and exact old
proper-fraction carries `r_i`, each output is

```text
x_i = F W_i / W + r_i
credit_i = floor(x_i)
new_carry_i = x_i - floor(x_i).
```

Summing gives the exact rational conservation law

```text
sum_i credit_i + sum_i new_carry_i = F + sum_i old_carry_i.
```

The model also closes the physical-asset question. An exhaustive fee set is
admissible only when both carry sums are integral. Calling those exact whole
sums `old_escrow` and `new_escrow`, every batch checks

```text
F + old_escrow = sum_i credit_i + new_escrow.
```

The result names its external allocation identity and reports both escrows. A
future fee authority must own and transfer those atoms; no later fee pot backs
an earlier remainder.

Splitting one weight into several tranche entries can change when a whole atom
is credited, but not aggregate value: credits plus physical carry escrow remain
exact. The test compares one weight-2 recipient receiving all 5 atoms with two
weight-1 recipients receiving 4 atoms plus 1 escrowed atom.

All fractions are reduced with checked `u128` arithmetic; overflow refuses the
whole allocation. Repeating allocations telescopes because the prior remainder
is added exactly. The tests check `10` atoms at weights `1:2`, then show that
one 10-atom allocation equals two 5-atom allocations at weights `1:1`.

Fee input and private output bind allocation, tranche, immutable fee policy,
snapshot epoch, share supply, tranche generation, integrated weight, and old
carry. Applying an output consumes the weight, increments one exact allocation
generation, and refuses replay or stale-share application. An external
authority must still prove that the complete recipient set is exhaustive, own
the prior/new carry escrow, and consume the realized fee pot exactly once.
If terminal carries remain nonzero, the last shares stay locked; a live release
needs a separately frozen, funded terminal carry rule rather than assuming
future volume will make the fractions integral.

## 6. Refusal transactionality

Every public mutation copies the fixed-size state, performs checked arithmetic
on the copy, recomputes all invariants, and assigns the copy only after success.
The tests compare the full pre/post value on refused self-cross, sub-minimum
fill, limit violation, arithmetic overflow, early lapse, over-withdrawal,
unfunded buy, fee replay, and fractional settlement. Empty range, duplicate
identity, hostile padding, and whole-schedule overflow refuse before any state
exists.
