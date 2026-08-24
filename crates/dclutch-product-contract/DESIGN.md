# Product contract ontology

Status: SDK-free, `no_std`, `no_alloc`, fixed-layout semantic contracts. This
crate does not hash content, authenticate Solana accounts, mint tokens, verify
partition artifacts, or execute transfers.

## Native liabilities

Product V1 has one native claim basis: `CategoricalUnitV1`. An authenticated
Terms partition is exhaustive, pairwise disjoint, ordered, and canonical. The
basis creates exactly one elementary claim kind per partition cell. Claim `i`
pays one collateral atom exactly when terminal cell `i` occurs and zero
otherwise.

This makes the native liability theorem small: one complete set contains one
unit of every elementary claim, so exactly one unit pays at any valid terminal
cell. The Market kernel and custody adapter remain responsible for proving
complete-set mint/burn conservation and physical collateralization.

Product V1 has no polynomial claim, payout evaluator, signed coefficient,
redemption rounding, fractional credit, or payout denominator in its native
basis. Those concepts previously conflated user payoff construction with the
protocol's elementary liabilities.

`CategoricalUnitV1` is 56 bytes:

| Field | Bytes |
| --- | ---: |
| magic, schema, reserved | 16 |
| capacity-profile content ID | 32 |
| partition/native-claim count | 4 |
| reserved | 4 |

## Portfolio recipes

`PortfolioTemplateV1<N>` is a content-addressable user/execution recipe, not a
Product instance or liability basis. It binds one authenticated categorical
ClaimBasis content ID and contains `N` nonnegative `u64` numerators with one
positive common `u64` denominator. At least one numerator is nonzero, and the
gcd of the denominator and every numerator is one. This gives every rational
vector a unique canonical preimage.

The exact width is `56 + 8N`: 72 bytes for a binary template and 184 bytes at
the current maximum profile. The selected decoder knows `N`, requires that one
exact physical width, and checks the encoded width byte. The constructor
gcd-normalizes; the hostile decoder refuses a reducible wire form.

Materialization takes a caller scale and produces `N` native `u64` claim
quantities only if every multiplication is checked, every division is exact,
and every quotient fits `u64`. There is no rounding boundary. Refusal leaves
the caller's output unchanged. A portfolio does not mint an aggregate bearer
claim by itself: execution must acquire or mint the returned elementary
quantities through separately authorized Market paths.

`2 <= N <= 16` is the current **provisional artifact-profile bound**. It is not
a mathematical restriction or a reason for a binary recipe to allocate 16
entries. Its lifting plan is a newly identified wider exact-width profile or a
paged template whose authenticated aggregate commits to the same ordered
native claim vector. Native liabilities and Product instances need not change.

The `u64` numerator, denominator, scale, and materialized token quantities are
**chain-derived representation choices**. Richer host-side rational inputs
must normalize and fit this release or be refused.

## Capacity profile

`CapacityProfileV1` governs only partition/artifact bytes, canonical page
geometry, and partition width. It no longer claims authority over coefficient
word widths or counts. Its exact layout is 96 bytes:

| Field | Bytes |
| --- | ---: |
| magic, schema, envelope, reserved | 16 |
| partition/artifact verifier release ID | 32 |
| measurement manifest or lifting-plan ID | 32 |
| artifact, page-payload, page-count, partition bounds | 16 |

Every numeric bound is labeled by `CapacityEnvelope`: `Measured` names a
measurement manifest; `Provisional` names a lifting-plan artifact. Changing a
bound or its evidence changes the profile content identity.

## Deleted pre-release ontology

This correction deletes `FiniteExactV1`, `CoefficientDegree`, coefficient
artifacts, evaluator/coefficient-profile identities, and redemption rounding
from Product authority. It also deletes the generic `ClaimBasisProfileV1`
branch: V1 has exactly one native basis.

There is no compatibility decoder. Existing pre-release content identities
must be rebuilt from categorical basis and, when useful, a separate portfolio
template. Downstream Found/operator code must:

1. create and authenticate only `CategoricalUnitV1` for native liabilities;
2. validate `InstanceV1` directly against that basis;
3. treat portfolio templates as optional content-addressed recipes; and
4. refuse within-cell graded products until a separately versioned
   nonnegative partition-of-unity basis, liability theorem, and evaluator are
   implemented.

Capped ramps and tents cannot be approximated by silently treating polynomial
coefficients as native claims. A compiler must return
`UnsupportedWithinCellGradedShape` for those shapes in this release.
