# Native B-spline reference integration

Status: **REFERENCE MODEL / NOT AN SBF OR LAYOUT CLAIM** (2026-08-19).

This note records exactly what the `clutch-bspline` integration into
`clutch-solana-reference::resolution` establishes, and what it does not. The
semantic distinction in `docs/design/NATIVE_AND_LOWERED_SEMANTICS.md` remains
controlling: these weights are native settlement semantics. A degree-zero
basket produced by sampling the same curve is a compatibility lowering, not
an equivalent implementation of a smooth Egg.

## Frozen basis representation

`BasisSpec` is the single evaluator input. It freezes:

- degree `d in {0,1,2,3}`;
- active outcome count `n` and common integer denominator `D > 0`;
- distinct active breakpoints, followed by canonical zero padding;
- the checked uniform power-of-two spacing declaration required by degrees
  two and three;
- the admitted coordinate ceiling; and
- clamp or refuse edge handling (degree zero is clamp-only).

The stored-count relations are:

```text
d = 0: K = n - 1       (interior categorical boundaries)
d > 0: K = n + 1 - d   (distinct clamped-basis breakpoints)
```

For smooth degrees the evaluator expands the distinct breakpoints to the
standard open-clamped knot vector with endpoint multiplicity `d + 1`. It uses
an exact reduced-rational bounded `BasisFuns` recurrence for degrees two and
three, and an equivalent exact hat specialization for degree one. The upper
endpoint is closed and awards the final outcome full weight.

## Integer quantization

The active rule named `WEIGHT-ROUND-01` is deterministic largest remainder:

1. compute every nonzero exact basis value;
2. floor each value after scaling by `D`;
3. award the remaining at-most-`d` atoms in descending order of the exact
   fractional remainders; and
4. break an exact remainder tie in favor of the lowest outcome index.

The rule returns nonnegative weights, zero canonical padding, support of at
most `d + 1`, and an exact integer sum of `D`. It supersedes both earlier
host-only directional degree-one rounding and the older draft's
highest-index residual rule. No deployed compatibility authority depended on
either. Largest remainder has lower aggregate L1 quantization error in the
checked oracle campaign; it is not described as statistically unbiased.

## Evidence rule and public seams

The reference resolution module has two modes:

- degree zero uses `derive_payout`, conservatively locates both evidence
  endpoints in the exhaustive categorical partition, and refuses a straddle;
- degrees one through three use `derive_payout_vector`, returning the native
  member-shaped weight vector for a digest-derived `ResolutionTerms` and a
  domain-matched sealed `WindowResult`.

Degree one may admit a conservative interval only when the two quantized
endpoint vectors agree. Its ordered first moment is monotone, so equality
cannot hide an interior change in the quantized vector. Degrees two and three
admit only point evidence **after edge handling**. A remaining non-point
interval refuses `R-15 NonPointEvidence`; the reference never substitutes a
midpoint or chooses an endpoint. Thus an interval wholly outside one side of
a clamping span may collapse to the same endpoint, while the same interval
under a refusing edge policy returns `R-14 ValueOutOfRange`.

Derived TWAP remains `R-05 StatisticUnsupported`. It needs a separately
bounded exact-rational path; this integration does not manufacture an integer
point from the ratio.

`derive_payout` still contains a preset-membership bridge for every derived
degree because the old account-shaped resolution path names a payout index.
That is a compatibility residue, not the native semantic ceiling. The direct
reference seam is `derive_payout_vector` followed by the kernel's
resolve-with-vector transition. A runtime adapter must authenticate terms,
source, window, and account identities before that pure seam is reachable.

## Checked evidence

The focused local campaign is:

```sh
cargo test --manifest-path crates/clutch-bspline/Cargo.toml
(cd crates/clutch-bspline && python3 oracle/check.py --random 5000)
cargo test --manifest-path programs/solana-reference/Cargo.toml
```

The Python oracle independently evaluates exact `Fraction` Cox-de Boor bases,
exhaustively covers small bounded grids, adds deterministic random cases, and
kills mutants for independent floors, interior formulas at clamped edges,
open-top handling, and directional residual placement. Reference tests pin
quadratic and cubic point vectors, smooth non-point refusal even when a coarse
denominator could make endpoint vectors equal, edge-policy ordering, and the
continued derived-TWAP/nonuniform-smooth refusals.

These are tests and a differential oracle, not a formal proof.

## Unclosed promotion gates

This integration does **not** establish:

- Solana account-layout support for recording an arbitrary resolved vector;
- an SBF instruction, compute budget, runtime rollback campaign, or audited
  deployed ELF;
- source authentication or permissionless source/archive operation;
- exact fractional bearer redemption through a frozen lot or remainder-credit
  rule;
- a proved interval evaluator for degrees two and three;
- smooth TWAP evaluation;
- a proof that any named analytic curve belongs to the finite spline span, or
  an approximation certificate when it does not; or
- atomic wrapper semantics for a named shaped portfolio.

Until each separate gate closes, degrees two and three are reference-native
semantics only. They must not be advertised as enabled on the program, and a
categorical approximation must remain visibly labeled as lowered.
