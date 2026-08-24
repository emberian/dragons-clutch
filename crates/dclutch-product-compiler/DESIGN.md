# Categorical Product compiler V1

This host-only crate constructs Product records from named exact recipes. It is
not an evaluator, payout VM, oracle adapter, or second source of native
liabilities. Every compiled Product has one elementary categorical-unit basis:
one native claim for each exhaustive ordered partition cell. A user-facing
payoff is a separate `PortfolioTemplateV1<N>` recipe over those native claims.

## Exact-N artifacts

`CompileRequest<N>`, `CompiledProduct<N>`, and `PortfolioTemplateV1<N>` share
the same compile-time width. Compilation also checks that the runtime partition
has exactly `N` cells. The record therefore encodes exactly `N` coefficients;
it neither max-allocates a 16-entry DTO nor leaves unused coefficient slots.

The partition artifact begins with `DCLTPAR1`, followed by its version,
coordinate denominator, cell count, closed domain endpoints, and strictly
increasing interior cuts. Its interpretation is `[lower, cut_0)`, ...,
`[last_cut, upper]`. Construction and decoding both refuse empty or reversed
domains, zero coordinate denominators, one-cell partitions, repeated or
unordered cuts, non-interior cuts, trailing bytes, and nonzero reserved bytes.

Binary thresholds and crash tails emit two cells. Ordered range buckets emit
one more cell than cut. The compiler reduces each rational payout, computes a
checked common denominator, converts every value to an exact `u64` coefficient,
and gcd-normalizes the entire vector with its denominator. No rounding occurs.
An all-zero recipe is not a portfolio and is refused by the contract.

`CappedRamp` and `Tent` are explicit
`UnsupportedWithinCellGradedShape` refusals. Representing either as polynomial
coefficients would reintroduce a parallel payout evaluator and make a
user-selected recipe look like native Product liability. A caller may instead
choose a categorical discretization as an explicit ordered-bucket product; a
future genuinely graded native claim family requires a separately reviewed
contract rather than an approximation hidden in this compiler.

## Authority and identities

`CompilationContext` contains only the authenticated capacity profile and ID,
the Terms semantic release ID, and canonical occurrence bytes. Evaluator,
coefficient-profile, and rounding-policy identities do not exist in this
ontology. The 56-byte categorical basis binds the capacity profile and outcome
count. The portfolio template binds the resulting claim-basis content ID.

Partition, partition evidence, Terms, occurrence artifact, Occurrence,
categorical basis, portfolio template, Instance, and shape commitment use
domain-separated SHA-256 content identities. SHA-256 here is deterministic
content addressing, not evidence that the host compiler or its inputs are an
authenticated oracle. The caller still owns that authentication boundary.

## Independent recheck

`recheck` does not call `compile`. It parses the partition, categorical basis,
portfolio template, Terms, Occurrence, and Instance preimages; regenerates the
named constant-per-cell recipe; verifies exact-N width and capacity; checks the
partition/basis/Instance/template links; verifies gcd-normalized coefficients
and denominator; materializes the template at its denominator to recover the
exact coefficient vector without rounding; and recomputes every certificate
identity. Artifact, basis, template, or certificate substitution is refused.

The capacity profile limits partition and content-artifact work only. It has no
coefficient word width or maximum coefficient count: those were artifacts of
the removed evaluator model. The current contract's two-through-sixteen
portfolio width is a provisional profile bound with an explicit future lifting
path, not a mathematical Product restriction.
