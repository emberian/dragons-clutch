# Finite payoff generality review

Status: **MODEL architecture review**, 2026-08-18. This review changes no
consensus code. “Proved” below refers only to elementary derivations stated in
this note and exhaustively checked over the model's bounded test domains; it is
not a Verus/Rocq or deployed-program verification claim.

## Finding

The current finite payoff algebra is general enough for continuous-range,
graded-proximity, piecewise-polynomial, tabulated-kernel, and distributional
products **as finite approximations**. It does not need an arbitrary payoff VM.

There are two independent layers:

1. a settlement basis produces nonnegative integer weights `w` whose exact sum
   is `D`; and
2. a shaped claim is a nonnegative integer coefficient vector `a` over that
   basis.

The terminal value is `dot(a,w)/D`. For every such vector,

```text
dot(a,w)/D <= max(a) * sum(w)/D = max(a).
```

Thus worst-case liability is a scan over at most sixteen coefficients. The
chain need not know whether the vector was described as a range, Gaussian,
cubic, hedge, or distributional statistic. Those are reproducible compiler
inputs and display metadata; the consensus-relevant object is the exact vector.

`ExactSamples` makes the inclusion algebraically complete at fixed `n`: it can
name every member of `[0,u64::MAX]^n`. A degree-0 one-hot settlement interprets
those values as a piecewise-constant payoff. A degree-1 partition-of-unity
basis interprets the same values as a piecewise-linear payoff. Samples of a
piecewise polynomial or any bounded kernel are therefore ordinary members of
the language; only the approximation certificate depends on the analytic
family.

## What the current architecture already supports

- The pure kernel accepts finite preset payouts and a derived validated simplex
  vector. Its derived Active collateral requirement is `max_i supply_i`.
- Immutable terms already carry a bounded basis degree, knots, denominator,
  rounding and evidence policies. The pure reference evaluator implements the
  degree-1 hat vector and refuses unsupported higher degrees.
- The batch relation already has bounded portfolio coefficient vectors; a
  shaped position need not become a new primitive mint.
- The finite one-hot path is the most mature end-to-end account path.

The important remaining distinction is not payoff expressivity. It is whether
smoothness lives in each primitive Egg's redemption rule (native fractional
basis) or in a portfolio sampled over exact one-hot Eggs. See
[`ONE_HOT_VS_DERIVED.md`](ONE_HOT_VS_DERIVED.md).

## Product-family inclusion

| Product | Exact finite representation | Approximation boundary |
|---|---|---|
| binary/categorical | one coefficient unit vector | none |
| hard range/tail | contiguous indicator coefficients | exact when boundaries align to cells |
| graded proximity | triangle/capped-linear/sample vector | cell or knot spacing |
| call/put/spread | capped-linear or exact samples | strike-grid alignment |
| piecewise polynomial | its exact integer values at anchors | between-anchor interpolation and coefficient rounding |
| Gaussian-like kernel | certified Gaussian constructor or exact table | compiler-reported knot + interpolation bound |
| arbitrary bounded tabulated kernel | `ExactSamples` | table producer owns analytic error claim |
| full distribution | prices of exhaustive one-hot Eggs | histogram resolution |
| expectation of any compiled claim | dot product of distribution prices and coefficients | price and coefficient scales |

For `n >= 3` this is strict containment at fixed `n`: ranges and the other named
constructors occupy a proper subset of the admitted coefficient cube. (At
`n = 1`, Constant is already complete; at `n = 2`, a capped line can name any
pair.) It is only parametric containment of continuous functions.
`MAX_OUTCOMES = 16` can be too coarse for a requested error tolerance over a
wide domain, and the system must refuse rather than call that approximation
precise.

## Gaussian certificate in the model

The Gaussian constructor is included because an unchecked `Gaussian` label on
an integer table would prove nothing. For each knot it bounds

```text
H * exp(-(x-center)^2 / (2*sigma^2)).
```

It uses positive Taylor partial sums for `exp(z)`, a geometric upper bound on
the remaining tail, reciprocal interval endpoints, and directed integer
rounding. The iteration count is frozen at 32 and the fixed-point scale at
`2^40`; there is no caller-programmable loop. Values beyond `z=8` compile to
zero with error below `H/2048`. It then adds either:

- linear-basis error `ceil(H*h^2/(8*sigma^2))`; or
- nearest-anchor one-hot error `ceil(H*ceil(h/2)/sigma)`.

The latter uses the deliberately loose but simple global bound
`|Gaussian'| <= H/sigma`. These are sup-norm error bounds on the frozen knot
domain, not statistical confidence intervals.

For `H = 1_000_000` atoms on a six-decimal collateral, maximum payout and
worst-state collateral are exactly one collateral unit. Coefficient
quantization is below one atom plus the reported interval width. If
`h = sigma/2`, the conservative interpolation components are:

```text
one-hot nearest cell: H/4  = 250_000 atoms
degree-1 linear:      H/32 =  31_250 atoms
```

This makes the tradeoff visible: atom scaling is cheap, but basis resolution is
not. Increasing `H` improves coefficient quantization without changing the
normalized collateral promise; decreasing `h` improves shape accuracy but may
require more primitive outcomes, accounts, mints, transaction bytes, and proof
work.

## Cheap SVM evaluation

Compilation is offline. Consensus sees fixed arrays only.

- One-hot settlement: select one payout index. A portfolio's economic payout
  is its coefficient at that index; primitive redemptions are `q` or zero.
- Degree-1 settlement: locate one pane, derive at most two nonzero weights with
  one checked multiply/divide/subtract (or shifts for the power-of-two exact
  variant), and store/bind the resolved vector once.
- Liability admission: scan at most `n <= 16` coefficients for the maximum.
- Trading: use the existing bounded portfolio dot product; no analytic curve
  executes in the batch verifier.

A V1 static client can compile and display the vector, while the program checks
canonical bytes, bounds, the market identity, and any registered compiler
artifact digest. An untrusted client never gets authority to supply a different
vector at resolution.

## Exact-redemption theorem

For one-hot primitive Eggs, every integer portfolio redeems atom-exact at every
quantity: the selected coefficient is an integer.

For native fractional weights, the model proves the universal exact lot formula

```text
L = lcm_i D/gcd(D, |a_i-a_0|).
```

Proof: since `sum(w_i)=D`,

```text
L*dot(a,w) = L*a_0*D + sum_(i>0) L*(a_i-a_0)*w_i.
```

This is divisible by `D` for every integer-simplex vector exactly when every
coefficient difference multiplied by `L` is divisible by `D`. Each individual
condition has least multiplier `D/gcd(D, difference)`; their least common
multiple satisfies all of them and is minimal. The executable test enumerates
all 1,792 combinations of three coefficients in `0..=3` and all three-weight
compositions of denominator six, checking the liability inequality and exact
lot at every point.

## Exact integration path

No shared-kernel edit is required for the recommended first path.

1. Promote this model only after freezing a canonical `ClaimArtifactV1` byte
   codec in a new dependency-free compiler crate. Include market-terms digest,
   basis/partition digest, compiler kind/version, exact coefficients, atom
   scale, rounding ID, approximation norm/bound, and canonical zero padding.
   `CompiledClaimV1` deliberately has private fields so safe Rust callers can
   obtain one only through `compile`; a future hostile-byte decoder must
   recompile or independently check the certificate-to-source binding rather
   than merely checking that its error fields add up.
2. Bind portfolio orders to the artifact digest or include the exact bounded
   coefficients in the signed order preimage. The existing batch relation
   remains the sole conservation authority.
3. Keep externally materialized market Eggs categorical and one-hot. A shaped
   claim is initially a displayed/traded basket, not a new token class.
4. If a wrapper is later justified, require exact escrow of every coefficient
   basket before minting one wrapper lot; wrapper supply and escrow get one
   semantic owner. The wrapper adds no market liability.
5. Add golden compiler vectors and independent reproduction before letting a
   human-facing client use analytic names. Unknown compiler versions refuse.
6. Benchmark `n = 2,4,8,16` before changing the outcome cap. Publish the error
   bound beside the transaction/account/CU cost.
7. At admission, additionally prove `maximum_payout * maximum_order_lots` and
   every coefficient/order product fit the kernel, batch, and account amount
   widths. The model returns `u128` liability; that does not by itself license a
   `u64` onchain amount.

Native fractional mode has a separate integration path: revise the resolution
record and kernel-account layouts so the account plane binds the resolved
vector, then choose exact lots, persistent remainder credits, or
portfolio-atomic aggregate redemption. Degree 2/3 basis evaluators should not
land until their interval-evidence ambiguity rule and integer construction are
proved. Neither extension is needed to ship the one-hot portfolio language.

## What is not implemented here

- a consensus codec or digest for `PayoffSpecV1`/`CompiledClaimV1`;
- account ownership, signer, source, clock, CPI, or Token-2022 checks;
- a wrapper mint or additional supply ledger;
- proof-tool theorems over the Rust definitions;
- a higher outcome cap or its SVM resource measurements; and
- analytic certificates for arbitrary `ExactSamples` tables.

Those are explicit promotion gates. The model's tests are evidence for the
model, not for an SBF artifact.
