# Continuous-claim algebra

Status: **IMPLEMENTED NATIVE ALGEBRA / HOST-ONLY SHAPE COMPILATION / LIVE
PORTFOLIO STOP** (2026-08-19). The open-clamped degree-0--3 evaluator,
`WEIGHT-ROUND-01`, immutable native basis mode, point-v3 and occupation-v4
resolution records, and exact primitive-Egg redemption are implemented. The
shape compiler and exact coefficient-portfolio settlement are executable host
seams. Their analytic certificates and atomic claim identities are not
committed by current onchain Terms or accounts. The detailed basis design
remains
[`../../implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md`](../../implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md).

> **Supersession notice.** The original proposal's degree-1-only description
> and “floor all but the highest-index nonzero weight” formula are historical,
> not valid implementation guidance. The only current point-weight rule is the
> largest-remainder algorithm below, with the lowest outcome index winning an
> exact remainder tie.

## Consensus object

Let `Omega` be the frozen finite evidence-result domain and let the `n`
canonical native Eggs have payout weights

```text
w(omega) = (w_0,...,w_(n-1)),
0 <= w_i <= D,
sum_i w_i = D.
```

One atom of Egg `i` pays `w_i/D` collateral atoms subject to the kernel's exact
division/refusal rule. For degrees one through three, the sole persisted native
vector owner is the point-v3 or occupation-v4 Resolution account. The kernel
reconstructs that vector ephemerally; it does not search the categorical preset
set. The consensus identity is the exact Terms and Resolution bytes, not the
words “continuous,” “Gaussian,” or “range.”

An integer coefficient portfolio is `a in [0,S]^n`. Holding `a_i` atoms of
native Egg `i` produces terminal payout

```text
P_a(omega) = sum_i a_i w_i(omega)/D.
```

No new liability is created when the claim is represented as a fully funded
portfolio. A separately transferable named wrapper must escrow the exact basket
or enter the authoritative supply ledger. The live program supports primitive
Egg holdings and signed portfolio placement/reservation; canonical paired
portfolio settlement is still a pure semantic seam because no live authority
can create its vector entitlement receipt.

## Solvency theorem

Let `T_i` be the aggregate issued supply of Egg `i`. Because `w/D` lies in the
simplex,

```text
sum_i T_i w_i/D <= max_i T_i.
```

Therefore a Hoard containing at least `max_i T_i` collateral atoms covers every
admitted native payout vector. This theorem is independent of price formation
and of a probability interpretation. Fees, keeper budgets, LP reserves, and
insurance do not count as Hoard collateral.

The stronger exact complete-set identity is

```text
P_(q,...,q)(omega) = q
```

for every `omega`. It licenses split/merge and complete-set redemption without
an oracle probability model. A separate LP reserve may not count the same
collateral atom simultaneously as Hoard backing.

## Native basis and shaped compilation

The native basis is the Terms-frozen open-clamped basis of degree `0..=3`, with
canonical zero padding and `MAX_OUTCOMES = 16`:

- degree zero is the exhaustive categorical cell basis;
- degree one gives piecewise-linear hat functions;
- degrees two and three give the bounded smooth polynomial basis; and
- a coefficient vector defines its native spline payout exactly, whether or not
  a human-readable analytic label is exact in that finite space.

The dependency-light host compiler in
[`../../../research/bspline-shape-compiler`](../../../research/bspline-shape-compiler)
uses exact rational construction/certification arithmetic. It currently
classifies:

- aligned hard ranges and tails as exact in degree zero;
- triangles and capped call/put spreads as exact in degree one when every kink
  is a frozen knot;
- globally affine restrictions as exact in every smooth degree;
- degree-2/3 Greville samples as a certified quasi-interpolant, never mislabeled
  as interpolation; and
- Gaussian proximity kernels as certified approximations, never exact members
  of the finite polynomial spline span.

The compiler emits rational coefficients and rational sup/L1 error enclosures.
It also reports a conservative consensus-weight quantization term and can
measure a categorical compatibility lowering against the native spline. Its
`NativeShapeCertificateV1` has canonical, domain-separated bytes binding the
Terms digest, `BasisSpec`, shape, compiler/evaluator/rounding versions,
coefficients, and error fields. The Rust decoder rejects noncanonical rationals
and recompiles the certificate. This remains an offline research tool, not
consensus code, and floating point is display-only.

## Host artifact and future consensus boundary

The landed host artifact is explanatory evidence in the direction
`Terms -> basis/shape -> rational Compilation`. It also emits exact typed Terms
upload and `CreateMarket` intent bytes. The direction is intentionally one-way:
the current Terms body does not contain the certificate digest and the SBF
program neither parses nor recomputes it.

The following remains a future consensus admission requirement, not a frozen or
decoded onchain wire format:

```text
ClaimArtifactV1 (NOT LIVE) {
  market_terms_digest,
  basis_kind,
  knots_or_cells_digest,
  coefficient_denominator,
  coefficients[MAX_OUTCOMES],
  analytic_source_digest,
  rounding_rule,
  approximation_norm,
  approximation_bound,
  compiler_version,
}
```

Current Terms do not contain this object, and the onchain program does not
authenticate the landed host certificate. Promotion requires a frozen
rational-to-integer coefficient scale, canonical zero padding, bounded
arithmetic, an admission relation, and an atomic position/receipt identity when
the product promises a named shaped claim. A valid native coefficient vector
does not by itself prove the truth of its analytic label.

## Approximation contract

For target `f`, rational coefficients `a`, and exact rational native basis
functions `B_i`, the compiler distinguishes shape error from consensus-weight
quantization error:

```text
S(x) = sum_i a_i B_i(x)
sup_x |f(x) - S(x)| <= epsilon_shape
sup_x |f(x) - sum_i a_i w_i(x)/D|
    <= epsilon_shape + epsilon_weight.
```

The certificate covers the full admitted closed interval, including
between-grid maxima, discontinuities, and closed-top behavior. Sampling only
knots or bin centers is insufficient. The current host compiler uses exact
piecewise rational/Lipschitz enclosures and separate rational Gaussian sample
enclosures. These are executable conservative calculations, not a
machine-checked theorem and not an onchain commitment. Refining the grid would
create new Terms; it cannot change a live claim's payout.

## Named rounding boundaries

> **Historical formula, retired.** The first proposal floored earlier active
> weights and set `w_last = D - sum(previous weights)` at the highest active
> index. It remains useful as a regression mutant and provenance record only.
> No current Terms mode selects it, and it must not be used for new vectors.

### Native point evaluator: `WEIGHT-ROUND-01`

For exact nonzero basis values `B_i(x)`:

1. compute every exact scaled value `D * B_i(x)`;
2. floor every scaled value;
3. let `r = D - sum_i floor(D * B_i(x))`;
4. award one atom to each of the `r` largest exact fractional remainders; and
5. break an exact remainder tie in favor of the lowest outcome index.

At degree `d`, at most `d` residual atoms are awarded. The result is
nonnegative, has support at most `d + 1`, has canonical zero padding, and sums
exactly to `D`. The independent oracle campaign found lower aggregate L1 error
than the retired directional residual rule; no statistical-unbiasedness claim
is made.

### Occupation-v4 finalizer

Statistics 6 and 7 first evaluate each accepted exact source bucket with the
same point rule, giving masses `M_i` whose sum is `D * coverage`.

- statistic 6 (`ExactOnly`) requires every `M_i / coverage` to divide exactly;
- statistic 7 (`LargestRemainderV1`) floors those averages and applies one
  separately named descending-remainder allocation, again with lowest-index
  exact ties.

This is occupation of the canonical quantized basis. It is not evaluate-at-TWAP
and not the research-only exact-rational-basis occupation control arm. Smooth
TWAP remains refused.

Any future coefficient quantization needs its own named rule, scale, and dust
owner. The current host compiler retains rational coefficients; it does not
silently choose an onchain beneficiary.

## Refusals and remaining STOPs

The implemented native paths refuse unknown basis/evaluator/statistic versions,
noncanonical padding, invalid degree/knot/count relations, arithmetic overflow,
inadmissible edge handling, smooth TWAP, and unsupported non-point evidence.
Occupation-v4 additionally refuses positive-width observations, gaps in the
current archive, archive substitution, and a mismatched finalizer.

Promotion of named shaped positions still requires:

1. canonical onchain coefficient/artifact bytes and certificate commitment;
2. exact rational-to-integer coefficient scaling and its one rounding boundary;
3. authoritative portfolio candidate selection and a program-created vector
   entitlement receipt; the routed direct authority is deliberately limited to
   a two-order, single-Egg, full-fill, zero-fee profile, its maximum top-three
   Select is a measured exact-1.4M-CU rollback STOP, and staged Direct V3 is
   model-only, so none closes this coefficient-portfolio boundary;
4. atomic wrapper or position semantics wherever separability is not promised;
5. a reviewed production provider/parser registry entry and adapter evidence;
   the live construction ABI is currently inert in the default artifact; and
6. universal refinement beyond the finite checked evaluator fixtures.

## Verification status and targets

- The production evaluator uses a validated private basis capability and one
  fixed-common-denominator Cox path. The reduced-`Fraction` evaluator is a
  `cfg(test)` differential oracle, not a second production dispatch arm.
- Fifteen Rust tests and the independent Python oracle pass 31,814 exact
  differential cases at seed `880230`; six oracle mutants are killed.
- Lean proves named mathematical-model basis, knot-linkage,
  largest-remainder, and admissibility results. Eight Lean-computed rows agree
  with the digest-pinned production Rust evaluator, and five source mutants go
  red.
- That bridge is finite executable refinement evidence, not a universal theorem
  about Rust, the SBF binary, account parsing, source authentication, or token
  settlement. At commit `87d2dbd60fa13d50e4f8b9e1c3697cd680697ce3`,
  the runner passed against production-source digest
  `220de128366a8311de6579c0ce334a64c97620159eaf9570f61fa10fabb6de92`;
  its own digest is
  `1778824030783f0209d0217cfe158f4f98a3f68ea53e4cb964fc186f0fd9eb67`
  and the recorded evidence digest is
  `b3b32b8bdd617229670e8be3844bd7d2cc88774abe6c3bccc7af76246b6deeed`.
  Any later source drift blocks until fixtures, mutants, and pins are
  regenerated.
- Local SVM campaigns execute native point-v3 and occupation-v4 resolution and
  exact-lot redemption. Exact measurements admit no monolithic initial
  occupation Resolve at the selected 25% headroom gate for spans `1..=3` and
  degrees `1..=3`. Routed `ResolutionWork` now executes bounded
  Begin/Fold/Finalize/Abort and measures Finalize at 1,094,832 CU for the tested
  span-three shape, but unmeasured shapes remain unadmitted and the integrated
  release identity is pending. This is runtime evidence, not formal proof or
  deployment evidence.
- Increasing `MAX_OUTCOMES = 16` requires new account, arithmetic, CU, and proof
  gates.

Exact-lot refusal is safe but not universal bearer liveness: an isolated native
sub-lot can remain permanently nonredeemable without voluntary reaggregation.
Likewise, externally burned winning claims and unsolicited Hoard-token
donations can leave collateral with no selected terminal recipient. The current
protocol has no authority to reinterpret those residues as fees or reserves.
