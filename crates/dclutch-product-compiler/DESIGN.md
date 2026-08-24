# Categorical Product compiler V1

This host-only crate constructs Product records from named exact recipes. It is
not an evaluator, payout VM, oracle adapter, or second source of native
liabilities. Every compiled Product has one elementary categorical-unit basis:
one native claim for each exhaustive ordered partition cell. A user-facing
payoff is a separate `PortfolioTemplateV1<N>` recipe over those native claims.

## Exact-N artifacts

`CompileRequest<N>`, `CompiledProduct<N>`, and `PortfolioTemplateV1<N>` share
the same total native-outcome width. The Product-owned result domain has
exactly `N-1` ordinary numeric regions and one explicit final failure outcome.
The template therefore encodes exactly `N` coefficients, including the
caller's separately declared failure payoff; it neither max-allocates a
16-entry DTO nor leaves unused coefficient slots.

`FiniteResultDomainV1` is the only persisted partition authority. It begins
with `DCLTRDV1` and binds coordinate-domain semantics, result unit, its closed
mapping release, one common positive denominator, strictly increasing cut
numerators, identity-ordered ordinary selectors, and the distinct final
failure selector. Its numeric interpretation is `(-inf, cut_0)`, ...,
`[last_cut, +inf)`. The old compiler-only `DCLTPAR1` artifact is deleted.

Binary thresholds and crash tails emit two ordinary regions plus failure.
Ordered range buckets emit one more ordinary region than cut, plus failure.
The compiler reduces each rational payout, computes a
checked common denominator, converts every value to an exact `u64` coefficient,
and gcd-normalizes the entire vector with its denominator. No rounding occurs.
An all-zero recipe is not a portfolio and is refused by the contract.

`CappedRamp` and `Tent` remain explicit
`UnsupportedWithinCellGradedShape` refusals. Representing either as polynomial
coefficients would reintroduce a parallel payout evaluator and make a
user-selected recipe look like native Product liability. The `graded` module
instead exposes `CellMidpoint` as one named, explicit projection boundary. All
formula knots must be partition cuts; the compiler samples each finite
compiler cell at its exact rational midpoint and emits an ordinary
ordered-bucket portfolio plus an explicit failure payout. This is a named
categorical approximation, not a pointwise error guarantee.
The one-hot native basis therefore still sums to exactly one collateral atom,
and no redemption rounding or new liability kind is introduced.

This categorical successor follows from the atom-level impossibility result:
a nonnegative integer payout vector summing to one is necessarily one-hot.
Nontrivial native hat weights would require fractional collateral,
bundle-dependent remainders, or a minimum settlement lot and a new resolved
liability invariant. None is smuggled into V1 under the name “graded.”

## Authority and identities

`CompilationContext` contains the authenticated capacity profile and ID,
Terms semantic release ID, coordinate-domain ID, result-unit ID, and canonical
occurrence bytes. Evaluator,
coefficient-profile, and rounding-policy identities do not exist in this
ontology. The 56-byte categorical basis binds the capacity profile and outcome
count. The Product Instance and portfolio template both bind the exact
result-domain content ID as well as the categorical basis.

Result domain, partition evidence, Terms, occurrence artifact, Occurrence,
categorical basis, portfolio template, Instance, and shape commitment use
domain-separated SHA-256 content identities. SHA-256 here is deterministic
content addressing, not evidence that the host compiler or its inputs are an
authenticated oracle. The caller still owns that authentication boundary.

## Independent recheck

`recheck` does not call `compile`. It parses the result domain, categorical basis,
portfolio template, Terms, Occurrence, and Instance preimages; regenerates the
named constant-per-cell recipe; verifies exact-N width and capacity; checks the
domain/basis/Instance/template links; verifies gcd-normalized coefficients
and denominator; materializes the template at its denominator to recover the
exact coefficient vector without rounding; and recomputes every certificate
identity. Artifact, basis, template, or certificate substitution is refused.

The capacity profile limits partition and content-artifact work only. It has no
coefficient word width or maximum coefficient count: those were artifacts of
the removed evaluator model. The current contract's two-through-sixteen
portfolio width is a provisional profile bound with an explicit future lifting
path, not a mathematical Product restriction.
