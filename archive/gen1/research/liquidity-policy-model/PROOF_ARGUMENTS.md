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

Let `P` be the sum of `sell_limit * remaining_lots` across active writes and
`A = 10^12` the reserve numeric bound. Admission also requires `R + P <= A`.
On a sell fill, the filled floor `L` leaves `P` before actual proceeds `C >= L`
enter reserve; the fill is admitted only when `R + C + (P-L) <= A`. Therefore
every active write is executable at its floor, while a higher clearing price
cannot consume numeric headroom promised to remaining writes. Cancellation or
lapse only lowers `P`.

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

## 3. Single-owner withdrawal and shares

Ignoring pending quotes for valuation, conservative equity is

```text
Q = R - H(q) + fractional_fee_carry.
```

An established tranche with share supply `S` mints `m` accounting units for
owner deposit `d` only when

```text
m = d S / Q
```

is a positive exact integer. This makes pre- and post-deposit conservative
equity per accounting unit identical. Issuance also requires the immutable
owner, `epoch <= batch_end`, and zero live inventory/reservations.

A withdrawal burning `b` shares may take at most `floor(Q b / S)` whole atoms
and no more than `R-E`. Subtracting no more than `R-E` directly preserves
`R' >= E`. The last share cannot leave reserve, inventory, active reservations,
or fractional carry without an owner.

V1 does not represent multiple holder balances and does not claim holder-level
withdrawal-order invariance. At `R=10, S=3`, legal same-owner partitions can be
`3+7` or `6+4`. Both return the immutable owner's complete ten atoms. A second
beneficial owner or transferable unit is outside the admitted state space and
requires a successor model with named per-holder residual ownership.

Before any share mint or burn, elapsed capital-at-risk is integrated. Because
the only beneficial owner is immutable, a partial share change cannot transfer
historical fee rights to another owner. The last share nevertheless remains
locked while weight or carry is nonzero. Fee inputs and private outputs bind
exact share supply, reserve, replay counters, fee-window end, and tranche
generation, so a same-epoch state change invalidates a stale output. A terminal
zero-pot allocation may consume weight without predicting future volume.

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

Risk integration is piecewise at every active quote's `expiry+1` boundary and
stops globally at the immutable fee-window end `batch_end+1`. Pending sell and
buy reservations count only while their quote can execute; inventory created
by a fill counts through that window end. It remains a settlement liability
after the fee window but earns no additional historical fee weight. The
hostile lapse witness waits from epoch 10 to 1,000 for a quote expiring at 15
and checks that it receives weight only for `[10,16)`.

## 5. Terminal fee-pot conservation and funded carry

V1 admits exactly one fee allocation per tranche, after the immutable fee
window. Every input must bind `snapshot_epoch >= batch_end+1`, zero prior carry,
zero allocation generation, and zero prior allocation identity. Therefore no
per-round approximation or tie preference can accumulate over a lifetime.

Let `U = 10^12`, raw weights be `W_i`, and `W = sum_i W_i > 0`. The model first
computes Hamilton grid units

```text
base_i = floor(U W_i / W)
remainder_i = (U W_i) mod W
left = U - sum_i base_i
```

and gives one additional unit to the `left < owner_count` largest remainders,
breaking ties by lexicographically smaller owner identity. The complete input
set must have unique tranche identities. Before Hamilton rounding, the model
sums weights by immutable beneficial owner. The owner credit and carry are
placed on that owner's lexicographically smallest tranche, so splitting one
owner's weight across multiple tranche inputs cannot alter any owner's
aggregate quota. The resulting owner-group `u_i` satisfy `sum_i u_i = U` and

```text
abs(u_i/U - W_i/W) < 1/U.
```

For terminal realized pot `F <= U`, each output is

```text
x_i = F u_i / U
credit_i = floor(x_i)
carry_i = x_i - floor(x_i).
```

Thus the explicit apportionment differs from the unbounded-precision raw quota
by less than `F/U <= 1` collateral atom per distinct owner. This is a named bounded
rounding rule, not an exact-rational `W_i/W` claim. Because `sum_i u_i = U`,

```text
sum_i credit_i + sum_i carry_i = F.
```

Every nonzero carry uses denominator `U`, so the carry sum is an exact whole
number of atoms. The batch reports that retained escrow and checks

```text
F = sum_i direct_owner_credit_i + retained_carry_escrow.
```

Whole credits are paid directly by the fee authority to each immutable owner;
they do not enter tranche reserve. Fee input and private output bind allocation,
owner, tranche, immutable fee policy, snapshot and window epochs, share supply,
reserve, replay counters, tranche generation, integrated weight, and zero old
carry. Applying an output consumes the weight and refuses replay or stale state.
An external authority must still prove the recipient set exhaustive, own the
retained carry escrow, atomically pay every direct credit, and apply every
output exactly once.

No second allocation can consume terminal carry. If a nonzero carry remains,
the last share stays locked; a live release needs a separately frozen, funded
terminal carry rule. The model never assumes future volume will clear it.

## 6. Bounded arithmetic closure

Let `A = 10^12`, per-tranche weight `W = 10^12`, and grid `U = 10^12`. V1
validates every collateral, inventory, share, fee-pot, reserve, per-tranche
weight, and carry denominator against the corresponding bound. At most eight
valid inputs compose to aggregate fee weight `8W`. Policy validation requires

```text
fee_window_epochs * collateral_cap <= W.
```

The largest relevant products are then bounded as follows:

- equity numerator: `< A*D + D < 10^24 + 10^12`;
- share mint and withdrawal numerator: `< A*D*A < 10^36`;
- owner-group weight normalization: `<= 8W*U = 8*10^24`;
- fee-pot times normalized units: `<= A*U = 10^24`;
- payout dot product: at most `16*A*(2^64-1) < 3*10^32`.

Every value is below `2^120`, hence below `u128::MAX`. Schedule compilation
checks `coefficient*lots` on both buy and sell sides. Exact carry combinations
use one common denominator and integer addition, so no LCM grows across inputs.
Sell admission separately reserves numeric headroom for every minimum future
proceed; a fill releases its floor before adding actual proceeds and must leave
headroom for all remaining floors. Whole fee credits bypass reserve and pay the
bound owner directly. Oversized inputs refuse before multiplication.

## 7. Refusal transactionality

Every public mutation copies the fixed-size state, performs checked arithmetic
on the copy, recomputes all invariants, and assigns the copy only after success.
The tests compare the full pre/post value on refused self-cross, sub-minimum
fill, limit violation, arithmetic overflow, early lapse, over-withdrawal,
unfunded buy, fee replay, and fractional settlement. Empty range, duplicate
identity, hostile padding, and whole-schedule overflow refuse before any state
exists.
