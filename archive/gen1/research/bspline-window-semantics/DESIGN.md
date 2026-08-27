# Native B-spline window semantics

Status: **isolated executable research**, 2026-08-19. Nothing in this directory
is a deployed rule, production adapter, formal-verification result, or permission
to enable degree two or three. The executable oracle uses exact rational
arithmetic and the landed point evaluator's largest-remainder rule.

## 1. The market must name the statistic

These are not interchangeable implementations of one idea:

1. `POINT`: evaluate the native basis at the data provider's canonical point;
2. `BASIS_AT_TWAP`: evaluate it at the exact time-weighted mean price;
3. `BASIS_OCCUPATION`: average the basis vectors visited through time; and
4. `CONSERVATIVE_INVARIANT`: accept an uncertainty interval only when every
   compatible source atom produces the same canonical payout vector.

For nonlinear degree-two/three bases,

```text
W_D(sum(dt*x)/sum(dt)) != round_D(sum(dt*W_D(x))/sum(dt))
```

in general. The left side describes the curve at the mean. The right side
describes how long the path occupied each smooth basis lobe. Neither may be
quietly substituted for the other. `compare.py` exhibits the difference.

Every mode freezes the degree, distinct open-clamped knots, edge policy,
payout denominator `D`, point quantizer version, source identity, window, and
the named statistic. A client may present friendly curve language, but the
terms bytes own the meaning.

## 2. Point plus confidence

Two coherent confidence policies exist; a market must choose one.

### 2.1 `POINT_AUTHORITATIVE`

The provider's signed canonical integer point `p` is the settlement fact and
confidence is an **admission-quality bound**. The adapter first rejects stale,
wide, malformed, or otherwise inadmissible observations, then returns
`W_D(p)`. This does not claim that every economically possible value in the
confidence band pays the same. It is suitable only when the event definition
really names the provider's canonical publication.

Cost after source admission is one bounded degree-two/three basis evaluation.
This is the easiest honest route to native smooth terminal claims.

### 2.2 `PAYOUT_INVARIANT`

The confidence band is a set of compatible settlement facts. For an inclusive
integer interval `[lo,hi]`, the model evaluates **every** source atom and
accepts only if all canonical quantized vectors equal one target. The scan has
a frozen hard cap; a wider interval refuses. It never samples, interpolates,
or chooses a midpoint.

Endpoint equality is unsound. With quadratic open-clamped knots
`(0,16,32)` and `D=4`:

```text
W(7) = (1,2,1,0)
W(8) = (1,3,0,0)
W(9) = (1,2,1,0)
```

Thus `W(lo)=W(hi)` does not certify the interior. This exact counterexample is
a permanent test. The same bounded full-lattice scan works for degrees two
and three.

The certificate is exact only for its declared discrete lattice. If a source
claims every real number in a continuous interval is compatible, integer
enumeration is not a proof. Such a profile must refuse until an exact
pane-polynomial certificate exists. A later accelerator can use rational
Bernstein bounds and exact de Casteljau subdivision to prove that floors,
fractional-remainder ordering, and tie boundaries cannot change; failure to
close that proof must fall back to bounded enumeration or refusal.

The model also implements the corresponding rational lattice for TWAP:
`n/T` for every integer numerator `n` in a conservative integral interval.

## 3. Evaluate at exact TWAP

For accepted point observations `(dt_j,p_j)`, retain

```text
P = sum_j dt_j*p_j
T = sum_j dt_j
```

and resolve `W_D(P/T)` using exact rational arithmetic. Flooring `P/T` first
would create a second, unjustified statistic. The summary law is componentwise
addition:

```text
(P_a,T_a) combine (P_b,T_b) = (P_a+P_b,T_a+T_b)
```

with identity `(0,0)` and an adjacency/domain guard. Addition is associative,
so differently parenthesized folds agree. Storage is constant and the shared
source accumulator already needs the same integral for ordinary TWAP.

This statistic is cheap and expressive, but it forgets the path. A path which
spent half its time at each tail can have the same TWAP as a constant central
path. Degree-two/three nonlinearity makes their basis payouts different under
occupation semantics. Documentation must say “basis at TWAP,” not “average
distribution.”

Under confidence-as-uncertainty, accumulate conservative integral bounds
`[P_lo,P_hi]` and accept only if the bounded rational-numerator certificate
proves `W_D(n/T)` invariant for every compatible `n`. Under
point-authoritative confidence, accumulate the admitted canonical points.

## 4. Time-average basis occupation

This is the path-sensitive shaped statistic.

### 4.1 Canonical quantized occupation

The recommended consensus candidate defines one bucket's basis vector by the
already-canonical point rule `W_D(p_j)`, then retains

```text
M_i = sum_j dt_j * W_D(p_j)_i
T   = sum_j dt_j
```

For every observation, `sum_i W_D(p_j)_i = D`, hence
`sum_i M_i = D*T`. At finalization, the exact average `M_i/(D*T)` is quantized
to denominator `D` once with the same largest-remainder/tie rule. There is no
dominant-component residual shortcut.

The summary `(M[0..n],T)` combines by componentwise checked addition. It is an
associative partial monoid over adjacent spans with a basis/domain identity;
the all-zero empty summary is the identity. An update evaluates one basis and
touches at most `degree+1` masses. Combining pages touches `n <= 16` masses.

For interval observations, a bucket contributes only when the full-lattice
certificate proves `W_D` constant. A midpoint is never admitted. If even one
compatible atom gives another vector, that bucket/window follows its frozen
failure policy.

### 4.2 Exact-basis occupation oracle

The model also accumulates exact rational `B_i(p_j)` and quantizes only once at
the end. For integer points on a uniform grid it uses the conservative common
scale

```text
Q = (lcm(1..degree) * knot_gap)^degree.
```

Every recurrence divisor is `m*knot_gap`, `m in 1..degree`, and any term uses
at most `degree` such divisors, so `Q` clears them. This oracle more closely
approximates an ideal continuous basis occupation, but it is **not** identical
to occupation of the canonical onchain `W_D`: per-bucket quantization and one
final quantization can differ by payout atoms. It also consumes substantially
more arithmetic headroom. It is a comparison/control arm, not the V1 choice.

## 5. Cost and accumulator placement

| statistic | streaming state | per accepted bucket | merge | final work |
|---|---:|---:|---:|---:|
| canonical point | last point/confidence | source admission | O(1) | one basis evaluation |
| basis at TWAP | `P,T` | two checked additions | O(1) | one rational basis evaluation |
| quantized basis occupation | `n` masses + `T` + domain | one basis evaluation, <= `d+1` mass additions | O(n) | `n` divisions/remainders |
| exact-basis occupation oracle | `n` larger masses + `T,Q` | exact recurrence + <= `d+1` additions | O(n) | `n` exact ratios |
| invariant interval | no persistent vector | — | — | `(hi-lo+1)` basis evaluations, hard-capped |

With `n=16` and `u128` masses, occupation alone is 256 bytes before domain,
span, duration, and commitment fields. More importantly, it is **basis
specific**. Updating every subscribed market on every feed tick would violate
the shared-accumulator/no-fanout architecture.

The viable placement is bounded resolution replay over sealed archive buckets,
or a content-addressed derived summary shared only by markets with exactly the
same basis, source, and window. Work can be paged because the summary combines
associatively. Its worst-case replay/page/finalize work must be prepaid; future
fees are not liveness capital.

## 6. Partition unity and solvency

The following are algebraic proofs for this model, backed by executable
falsifiers but **not machine-checked proofs**.

1. Cox--de Boor starts with nonnegative degree-zero cells. Each recurrence
   combines nonnegative terms with nonnegative coefficients. On the clamped
   knot span its normalized basis functions sum to one; the closed top is the
   explicit last one-hot vector.
2. If exact simplex values are `b_i`, let `f_i=floor(D*b_i)` and
   `r=D-sum_i f_i`. Largest remainder adds exactly one atom to exactly `r`
   components. Every output is nonnegative and the output sum is exactly `D`.
3. TWAP returns one such vector. Quantized occupation has
   `sum_i M_i=D*T`; exact-basis occupation has `sum_i A_i=T`. Their exact
   averages are simplex vectors, so final largest-remainder quantization also
   sums to `D`.
4. For nonnegative outstanding supplies `T_i` and any resolved vector `w`,

   ```text
   liability = sum_i (w_i/D)*T_i
             <= sum_i (w_i/D)*max_j(T_j)
             = max_j(T_j).
   ```

   Therefore every mode preserves the central maximum-supply collateral bound.
   A complete set with equal supplies redeems exactly one collateral unit per
   set because `sum_i w_i=D`. Per-wallet divisibility still requires the
   separately frozen lot/remainder-credit rule; this proof does not erase that
   boundary.

`test_model.py` exhaustively checks small exact grids for nonnegativity, local
support, and both exact/quantized partition unity; tests monoid associativity;
pins the endpoint counterexample; compares path modes; and checks the solvency
inequality over deterministic adversarial supply vectors.

## 7. Honest V1 recommendation

1. **Keep the modes first-class and separately named.** Do not make a generic
   “smooth resolution” flag whose adapter chooses point, TWAP, or occupation.
2. **Admit native degree-two/three point settlement first** when the source
   profile explicitly defines its signed point as authoritative and confidence
   as an admission bound. If confidence denotes uncertainty, use the bounded
   full-lattice invariant certificate or refuse.
3. **Admit `BASIS_AT_TWAP` once the safe-Rust evaluator accepts a checked exact
   numerator/denominator.** It reuses the generic shared accumulator and has
   the smallest new liveness/storage surface. Describe it narrowly.
4. **Keep `BASIS_OCCUPATION` in V1 scope as the advanced path-shaped mode, but
   gate it on a measured bounded archive-replay/work-account implementation.**
   Use canonical quantized occupation, `u128` checked masses, the exact summary
   identity, and the same final largest-remainder rule. It is the mode that
   actually records shaped dynamics through time.
5. **Do not promote the exact-basis occupation oracle yet.** First quantify
   its divergence from canonical occupation, prove fixed-width headroom, and
   decide whether changing point semantics to support it is worth a new
   version. It remains valuable as a bias/error oracle.
6. **Never accept endpoint equality or an arbitrary midpoint for degree two or
   three.** The bounded exact scan is safe for declared integer/rational
   lattices today. Wider or continuous uncertainty refuses until a real
   polynomial certificate is implemented and proved.

This recommendation restores native shaped settlement without pretending the
source, path statistic, uncertainty policy, and quantizer are one decision.
