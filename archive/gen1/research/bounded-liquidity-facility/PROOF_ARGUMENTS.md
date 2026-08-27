# Checked arguments and proof boundary

Status: **ALGEBRAIC ARGUMENTS PLUS EXECUTABLE FALSIFIERS**, not formal
verification.

Let `n >= 2`, `b > 0`, `Q = sum_i q_i`, exact initial simplex price
`pi`, and `y_i = q_i - Q/n`. The rational potential can be rewritten as

```text
C(q) = dot(pi,q) + sum_i(y_i^2)/(2b).
```

All executable calculations use checked `u64`/`u128` integers. The proof text
below explains the intended theorems; it does not close the Rust compiler,
Solana runtime, Token-2022, or adapter boundary.

## 1. Convexity and exact prices

The Hessian is `(I - 11^T/n)/b`: positive semidefinite, with a zero direction
only along complete-set translation. Therefore `C` is convex. Differentiation
gives

```text
p_i = pi_i + (q_i - Q/n)/b.
```

The exact integer policy represents `pi_i=a_i/A`, `sum_i a_i=A`. With common
denominator `A*b*n`, the price numerator is

```text
a_i*b*n + A*(n*q_i-Q).
```

These numerators sum exactly to `A*b*n`. The model requires each to be
nonnegative on every endpoint, then recomputes and checks the exact rational
simplex price vector. For uniform `pi`, this domain simplifies to
`Q-n*min(q)<=b`. In general it remains an intersection of linear half-spaces,
so the entire segment from zero to an admitted `q` is also admitted.

## 2. Potential never exceeds full-simplex liability

The full integer payout simplex has support function `H(q)=max_i q_i` for
nonnegative inventory. Along the segment `t*q`, the gradient is a simplex
vector, hence

```text
C(q) - C(0) = integral_0^1 <grad C(tq), q> dt <= H(q).
```

`H(q)` is an integer, so `ceil(C(q)) <= H(q)` as well. The executable function
checks this consequence rather than trusting the derivation.

## 3. Global worst-case loss

For any selected terminal vertex `j`, let `P=I-11^T/n`. Since
`e_j-pi` sums to zero, translation cancels and the rational loss is

```text
q_j - C(q) = <e_j-pi, Pq> - ||Pq||^2/(2b).
```

The concave quadratic is maximized by

```text
Pq = b*(e_j-pi),
```

which yields `b/2*||e_j-pi||^2`. Taking the maximum over `j` gives the frozen
capital rule. For uniform `pi`, this is `b*(n-1)/(2n)` and the maximizing
inventory is, up to complete-set translation, `(b,0,...,0)`. For a nonuniform
prior some unconstrained maximizers can fall outside the nonnegative inventory
or price domain; retaining their value is conservative. Every graded
integer-simplex payout is a convex combination of vertices, so the same bound
covers it. Since `C_hat(q)=ceil(C(q)) >= C(q)`, rounding can only reduce sponsor
loss.

The model requires

```text
K >= ceil(b/2 * max_j ||e_j-pi||^2)
```

before initialization. Thus `K + C_hat(q) - H(q) >= 0` for every admitted
state, independently of future volume or fees.

## 4. Exact rounding and path independence

There is one rounded state potential, not one rounding operation per algebraic
term. A trade from `q` to `q'` exchanges

```text
Delta = C_hat(q') - C_hat(q).
```

For any intermediate endpoint `r`,

```text
[C_hat(r)-C_hat(q)] + [C_hat(q')-C_hat(r)]
= C_hat(q')-C_hat(q).
```

This proves exact split/recomposition and zero-cost round trips before external
batch fees. It also prevents a wrapper from obtaining a different facility
price merely by decomposing an identical native coefficient endpoint.

Complete-set translation is exact over integers because `sum_i pi_i=1`:

```text
C(q + a*1) = C(q) + a
ceil(C(q) + a) = ceil(C(q)) + a.
```

So selling `a` of every Egg charges exactly the `a` collateral needed to split
those complete sets, leaving free facility cash unchanged.

## 5. Physical Egg and cash conservation

Set `H=max(q)` and `r_i=H-q_i`. For a transition with Eggs sold to users `s_i`,
Eggs bought from users `u_i`, and `q'_i=q_i+s_i-u_i`, let

```text
split = max(H'-H, 0)
merge = max(H-H', 0).
```

Substitution proves, componentwise,

```text
r_i + u_i + split = r'_i + s_i + merge.
```

The implementation checks that identity for every active outcome. Free cash is
defined by `F=K+C_hat(q)-H`; adding trader cash in and merged Hoard collateral,
then subtracting split collateral and trader cash out, yields exactly
`F'=K+C_hat(q')-H'`. Every subtraction is checked.

## 6. Resolution conservation

Let integer payout weights `w_i >= 0` sum to denominator `D`. The facility's
external Egg payout is

```text
E = sum_i(q_i*w_i)/D.
```

Retained complement Eggs redeem for

```text
sum_i((H-q_i)*w_i)/D = H-E.
```

Therefore terminal sponsor cash is

```text
F + H - E = K + C_hat(q) - E >= K + C_hat(q) - H >= 0.
```

Every inventory coordinate is a multiple of `D`, so both divisions are exact
under every admitted integer payout vector. The state records both terminal
sponsor cash and external-holder payout, and validates their sum against
`K+C_hat(q)`. It also retains exactly `E` as facility-attributed Hoard backing;
that collateral remains claimant principal until external holders redeem under
the protocol's ordinary resolved-claim owner.

## 7. Conditional lifecycle progress

Define claim-safety progress phases

```text
Trading < BuybackOnly < Resolved.
```

At or after the immutable close slot, any caller can advance `Trading` to
`BuybackOnly` without sponsor authority or an asset movement. At or after
maturity, a caller holding the already-authenticated payout vector can advance
either live phase directly to `Resolved` in one transition. That transition
uses only already-backed retained Eggs and never asks the sponsor, fee pot,
treasury, or future flow for capital. Buybacks are optional and therefore
cannot block claim resolution. Sponsor withdrawal/`Retired` is deliberately
excluded from claimant liveness.

This is a conditional state-machine liveness argument: if one valid transaction
can be executed, one step reaches the safe terminal claim phase. The model does
not prove transaction inclusion, source publication, rent availability, or
caller compensation. A live Series must prepay those external progress inputs
and prove their adapters separately.

## 8. Arithmetic envelope

V1 caps `n <= 16` and each atom value, inventory coordinate, and `b` at
`10^12`. Consequently:

- `Q <= 16*10^12`;
- `sum(q_i^2) <= 16*10^24`;
- the largest potential numerator is below `2^128` by a wide margin; and
- every persisted output fits its checked `u64` domain.

For the signed dealer, `MAX_ATOMS` is a single-source/input bound rather than an
aggregate pool bound. Checked admission proves live cash is at most three such
sources (`c0 + K + C_hat(q) <= 3*MAX_ATOMS`). Retained Egg redemption contributes
at most one additional source because an exact simplex payout is bounded by the
largest custody coordinate. Thus terminal cash is at most `4*MAX_ATOMS`.
Dedicated aggregate constants encode those conservative bounds; all individual
capital, inventory, depth, and allocation inputs remain capped at `MAX_ATOMS`.
The curve term has magnitude at most one inventory-coordinate bound because
prices remain a nonnegative simplex throughout the box: integrating along the
straight segment from zero bounds `|C(q)|` by `||q||_infinity`; integer ceiling
does not exceed the integer endpoint bound.

The largest-domain test exercises 16 outcomes, `b=10^12`, every inventory
coordinate at `10^12`, an exact `10^12`-set translation, and sponsor capital
`468,750,000,000`.

## 9. What remains unproved

The test suite is not a universal proof. A formal claim would still need a
named theorem set over the Rust implementation or a proved refinement into it,
plus a source digest and exact toolchain. More importantly, the pure state does
not prove account identity, token custody, transaction atomicity, candidate-set
completeness, source authentication, clock authenticity, transaction
inclusion, or runtime compute bounds. Those are explicit adapter obligations.

## 10. Signed covered-dealer extension

The `signed_dealer` module permits `q_i` on both sides of zero while retaining
the same potential and global loss proof. LPs contribute fixed assets `c0,g0`,
and actual custody is

```text
c(q)=c0+K+C_hat(q)
g(q)=g0-q.
```

The policy proves its complete signed box has nonnegative simplex prices by
checking, for each `i`, the mixed corner minimizing `p_i`: `q_i=-B_i` and every
other `q_j=U_j`. Coordinatewise monotonicity then makes the all-buy lower corner
the exact cash minimum. Structural admission checks `g0_i>=U_i` and the maximum
custody arithmetic bound `g0_i+B_i<=10^12`. Initialization checks the actual
sponsor deposit against both the curve-loss subsidy and
`c0+K+C_hat(L)>=0`. This permits sound sponsor overcapitalization instead of
unnecessarily requiring all bid financing from LP cash.

At payout `w/D`, terminal pool value factors exactly:

```text
T = c0 + g0 dot w/D + [K+C_hat(q)-q dot w/D].
```

The bracket is nonnegative by the same generalized quadratic conjugate bound,
which applies to signed `q`. Each per-share contributed Egg coordinate is a
multiple of `D`; therefore each holder's baseline basket payoff is an integer.
Hamilton allocation of `S*baseline+yield` gives every holder at least its exact
share-scaled baseline and allocates only nonnegative yield dust.

This extension assumes zero pool-borne expenses. It does not prove token
custody, Hoard refinement, beneficial-owner identity, candidate transaction
atomicity, or inclusion liveness. Its sponsor capital becomes an irrevocable
LP-pool donation only at activation; changing that waterfall requires a
successor theorem.
