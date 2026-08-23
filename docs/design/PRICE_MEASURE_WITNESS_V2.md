# Price-measure witness V2

Status: **DESIGN / NOT A RUNTIME CLAIM**.

The first executable exact-arithmetic model is now under
`research/price-measure-witness`. It generates every degree-two/three transfer
table it exercises from the canonical open-clamped recurrence and validates
point measures, cross-span mixtures, quadrics, canonical denominators, and
mutations. It remains an offline model, not adapter or SBF evidence.

This design turns the exact degree-two/three membership theorem already proved
in `docs/research/DUAL_IS_THE_MEASURE.md` section 7.6 into a bounded candidate
witness. It is the path to keeping smooth coupled markets without pretending
that the current finite `V1b` inequalities are a complete no-arbitrage test.

The existing `V1b` gate remains useful: each refusal names an executable
arbitrage, it is exact for degree zero/one and single-span degree two/three,
and it cheaply removes obvious bad prices. For a multi-span degree-two/three
public profile, however, acceptance should additionally require the witness
below.

There is now a small exact falsifier for the residual. On the degree-two,
five-claim open-clamped uniform grid with breakpoints `[0, 1, 2, 3]`, take the
simplex price

```text
p / S = (1/3, 2/3, 0, 0, 0).
```

It passes every V1b ceiling and butterfly inequality: the second claim is
exactly on both its `2/3` ceiling and `p_1 <= 2(p_0 + p_2)` butterfly. But the
portfolio coefficient vector

```text
c = (1, -2, 10, 40, 64)
```

has basis payoff `sum_i c_i N_i(x) = (3x - 1)^2 >= 0` throughout the domain,
while its price is `dot(c, p) = -S`. The venue therefore prices a nonnegative
payoff at a strictly negative amount. This vector is the first required
runtime/model regression for the witness checker.

## 1. Claim boundary

For the market's immutable open-clamped uniform basis, let

- `d` be the degree, in `{2, 3}`;
- `n` be the outcome count, at most 16;
- `K = n + 1 - d` be the stored-breakpoint count;
- `S` be the exact integer price scale;
- `p_i` be the candidate's exact integer simplex prices; and
- `T_k` be the canonical rational matrix expressing the `d + 1` B-splines
  active on span `k` in that span's Bernstein basis.

The candidate supplies nonnegative Bernstein moment rows `w[k][r]` over one
positive common denominator `W`. They represent

```text
w[k][r] / W = integral over span k of BernsteinBasis[r] dQ.
```

The verifier accepts only if all of the following hold exactly:

```text
sum(k,r) w[k][r] = W

p_i / S = sum(k,r) T_k[i,r] * w[k][r] / W    for every i

degree 2: w[k][1]^2 <= 4*w[k][0]*w[k][2]

degree 3: w[k][1]^2 <= 3*w[k][0]*w[k][2]
          w[k][2]^2 <= 3*w[k][1]*w[k][3]
```

The inequalities are the exact truncated Hausdorff moment conditions in
Bernstein coordinates. The linear equations join the per-span measures into
the candidate price vector. Therefore every accepted witness constructs a
representing nonnegative measure and proves that the price vector lies in the
true basis-moment body.

This is an acceptance certificate, not an optimality certificate. It proves
price coherence; it says nothing about whether the candidate maximizes the
venue's score or fills the largest possible volume.

## 2. Exact integer form

The transfer matrices are program-derived facts, never caller-supplied facts.
For each admitted `(d, n)`, generation produces one positive denominator `D`
and signed integer numerators `A[k][i][r]` such that

```text
T_k[i,r] = A[k][i][r] / D.
```

The verifier checks, with pre-proved arithmetic bounds:

```text
D * W * p_i == S * sum(k,r) A[k][i][r] * w[k][r]
```

for every outcome. Negative transfer coefficients, if any appear in a chosen
canonical expansion, require a checked signed accumulator; the preferred
generated representation is a nonnegative refinement table so the runtime can
remain in `u128`. The generator must prove which representation it emitted.

`W` and every `w[k][r]` are bounded unsigned integers. The bound is not chosen
by this document. It must be derived jointly from:

- the maximum generated `D` and transfer numerator;
- `S`, `n <= 16`, and at most 15 spans;
- the largest quadratic product; and
- the SBF implementation's checked `u128` envelope.

A finite `W` bound may reject a coherent price vector whose only available
witness exceeds that bound. It cannot admit an incoherent vector. Promotion
must therefore call the bounded form a **sufficient inner certificate** until
one of these is proved:

1. every admitted integer price vector has a rational witness within the
   selected bound; or
2. candidate construction restricts the price lattice to one for which the
   bound is constructive.

The common representation should be canonical: require
`gcd(W, all w[k][r]) == 1`. Canonical reduction prevents multiple byte
identities for the same witness and makes candidate digests stable. It is not
part of the mathematical soundness argument.

## 3. Candidate-side account

A successor sidecar should have one semantic owner and contain no duplicate
copy of Market or Candidate facts:

```text
PriceMeasureWitnessV2 {
    schema_version
    candidate_feed
    relation_domain_digest
    candidate_price_digest
    basis_degree
    outcome_count
    span_count
    common_denominator W
    moments[(K - 1) * (d + 1)]
    body_digest
    lifecycle_state
}
```

The canonical PDA is derived from the candidate feed, not merely the submitter
or epoch. The relation-domain digest binds Market, Terms, price scale, epoch,
and score-policy generation through the existing candidate identity. The
price digest binds the exact vector being certified. The verifier reloads and
checks those owners rather than trusting the sidecar's descriptive fields.

At the current maximum, the body carries at most `15 * 4 = 60` moments, or 480
raw bytes at `u64`, plus a small fixed header. That is materially smaller than
`CandidateFeed` and `ClearWork`; exact account size and rent still require a
layout implementation and measurement.

Lifecycle follows candidate lifecycle V2:

1. initialize only during candidate submission;
2. append or write each canonical row once;
3. seal before submission close;
4. verify during the separately funded verification interval;
5. retain with the candidate through selection and challenge/finalization;
6. close only after the authenticated candidate/ClearWork child count permits
   closure.

No witness may be replaced after its candidate price digest is sealed.

## 4. Streaming verification

The verifier can check all spans in one linear pass:

1. validate the fixed header and exact body length;
2. recompute the reduced common denominator;
3. for each span, check nonnegativity and the one or two Hausdorff quadrics;
4. add the row sum into `total_mass`;
5. multiply the row by the generated transfer numerators and add into at most
   `d + 1` of 16 outcome accumulators;
6. require `total_mass == W`;
7. compare all 16 accumulated equations to `D * W * p_i` exactly; and
8. bind the successful digest into the candidate's verified checkpoint.

If a one-transaction implementation lacks CU or stack headroom, a small
`MeasureWorkV2` checkpoint stores only:

- candidate and witness digests;
- the next span index;
- total mass;
- 16 checked accumulators; and
- a rolling canonical-body digest.

Resume reauthenticates the immutable candidate/witness bodies and advances a
monotone span cursor. No proof-only precondition or unchecked partial decode is
permitted. Failure leaves the candidate unverified and must not mutate the
retained-best registry.

## 5. Candidate construction

The offchain solver may use floating-point conic optimization only to search.
Before submission it must rationally reconstruct `W, w`, reduce the witness,
and run the same exact integer verifier locally. The chain trusts none of the
search method, solver software, or claimed objective value.

An exact constructive path is preferable:

1. solve the per-span second-order-cone feasibility problem;
2. rationally reconstruct a point inside the feasible face;
3. project the linear equalities exactly over rationals;
4. if reconstruction approaches a curved boundary, derive the boundary atom
   or deliberately move to an interior lattice price; and
5. serialize only after the exact verifier accepts.

The reference solver should emit a derivation manifest naming its algorithm,
input candidate digest, unreduced and reduced denominator, maximum residual
before rational reconstruction, and final exact verifier result. The residual
is diagnostic only and never enters consensus.

## 6. Profiles and compatibility

- Degree zero/one: simplex membership remains exact; no sidecar is accepted or
  required.
- Single-span degree two/three: the existing quantifier-free Hankel checks are
  exact; a profile may omit the sidecar.
- Multi-span degree two/three, `certified-refusal-v1b`: retains today's wider
  acceptance boundary and must be labeled as necessary-not-sufficient.
- Multi-span degree two/three, `measure-witness-v2`: requires the sidecar and
  admits only prices with a checked representing-measure witness.

This is why capability profiles are not a reduction in the protocol's vision.
They allow the sophisticated surface to carry its real certificate and cost
without forcing categorical markets to pay for or misdescribe it.

## 7. Promotion gates

Before this becomes an SBF claim:

1. Generate every `(d, n)` transfer table from the canonical B-spline
   evaluator and commit its source/derivation manifest.
2. Differentially check the table at every polynomial coefficient, not only at
   sampled points.
3. Prove or explicitly bound signedness and every `u128` product/sum.
4. Add exact accepted point masses, mixtures across every span boundary, and
   single-span corpus vectors.
5. Add mutations for one moment, denominator, span order, price, candidate
   binding, transfer-table identity, quadratic boundary, and body padding.
6. Generate coherent vectors that V1b accepts and rejects; require the witness
   checker to agree with independent high-precision/conic search followed by
   exact reconstruction.
7. Require the five-claim `(1/3, 2/3, 0, 0, 0)` V1b false acceptance above to
   be refused for absence of a valid witness, and independently regenerate
   additional wide-support examples.
8. Measure host, SBF, streamed-resume, account-rent, and candidate-close costs.
9. Join selection so an unverified sidecar can never exclude or evict a fully
   verified candidate.
10. Extend the named Lean theorem only for the transfer-table/checker
    correspondence actually proved; do not call the SBF adapter formally
    verified.

The next executable milestone is an isolated safe-Rust, fixed-capacity checker
and generated-table corpus equivalent to the exact Python model. Runtime wiring
follows only after independent table generation agrees and the integer bounds
above are proved.
