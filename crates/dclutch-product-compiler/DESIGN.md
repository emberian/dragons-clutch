# Exact Product compiler V1

This host-only crate constructs the bounded records in
`dclutch-product-contract`; it is not an evaluator, an arbitrary bytecode VM,
or an oracle adapter. Its only V1 input axis is one rational coordinate
`n / coordinate_denominator`, but affine coefficient evaluation always uses
the integer numerator `n`. A cell coefficient row `(a0, a1)` therefore means
`(a0 + a1*n) / payout_denominator` exactly.

## Canonical artifacts

The partition artifact starts with `DCLTPAR1`, a version, coordinate
denominator, cell count, closed domain endpoints, then strictly increasing
interior cuts. Its unique semantic interpretation is `[lower, cut_0)`, ...,
`[last_cut, upper]`; this is exhaustive, pairwise disjoint, ordered, and
canonical. The compiler refuses a one-cell partition, non-interior cuts,
duplicates, reversals, empty/reversed domains, and zero denominators.

Coefficient words are exactly the capacity profile's signed eight- or
sixteen-byte little-endian integer words. They are cell-major and
degree-ascending. Every rational payout is normalized to one checked LCM
denominator. There is no rounding while compiling; the only rounding selection
is the finite claim-basis redemption policy, and the contract record rechecks
its allowed combinations.

Artifact and preimage identities use domain-separated SHA-256. The caller owns
the authenticated hash boundary for supplied release/profile identities and
occurrence bytes. SHA-256 here gives deterministic content addressing; it is
not a claim that a host compiler is a trusted oracle or on-chain verifier.

## Current exact shapes

* Binary threshold and crash tail compile to degree-zero two-cell profiles.
* Ordered buckets compile to degree-zero profiles.
* Capped ramps and tents compile to degree-one profiles, where all slopes and
  intercepts fit the selected signed word width.

The compiler rejects instead of approximating when LCM construction,
intermediate arithmetic, output capacity, or signed word conversion fails.
Approximate products require a different explicit error-bound certificate type;
V1 does not define one.

## Rechecking

`recheck` does not invoke `compile`. It decodes the partition and coefficient
artifacts, independently regenerates the expected named shape, revalidates
capacity, parses all Terms/Occurrence/ClaimBasis/Instance preimages, verifies
their links, and compares all domain-separated identities in the certificate.

## Generalization

Degree two and three need a new product-shape set plus explicit basis-variable
semantics and range proofs before selecting the existing contract degrees.
The artifact ordering can extend naturally to `(cell, degree 0..d)`, but no
new curve should silently reuse an affine evaluator release. Multivariate
products require a separately versioned canonical partition encoding (for
example, a lexicographic finite cell complex), explicit axis units, exhaustive
disjointness evidence, a specified monomial ordering, and capacity profiles
whose bounds cover the cross-product cells. They must not be smuggled into
the current univariate bytes.
