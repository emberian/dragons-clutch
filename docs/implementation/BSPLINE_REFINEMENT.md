# Native B-spline Rust/Lean refinement boundary

Status: **CHECKED FINITE EXECUTABLE AGREEMENT / OPEN UNIVERSAL REFINEMENT**
(2026-08-19).

## Result

Commit `87d2dbd60fa13d50e4f8b9e1c3697cd680697ce3` has a mechanically
source-bound bridge between the Lean B-spline semantics and the production
Rust evaluator. It is stronger than a copied reference algorithm: the
right-hand side is the actual `clutch-bspline::BasisSpec::evaluate` method from
the complete digest-pinned production `lib.rs`. The left-hand side is computed
live from the checked Lean definitions and serialized by a non-evaluating
emitter.

At these exact pins, all eight rows agree:

| Artifact | SHA-256 |
|---|---|
| `crates/clutch-bspline/src/lib.rs` | `220de128366a8311de6579c0ce334a64c97620159eaf9570f61fa10fabb6de92` |
| `crates/clutch-bspline/Cargo.toml` | `d993057affd6a9ba58a698e59e109ab882353456294ba57712c6cfac378b1c0d` |
| `crates/clutch-bspline/Cargo.lock` | `e49289a908b01a9032b096cfea0499f4a902714abf9475b91b55446d0ab43edd` |
| `crates/clutch-bspline/examples/oracle_driver.rs` | `c74ecec10c36fbebc3ab3335f2f933de6ecbaae4e061615f9e0822a917888dc7` |
| `lean/DragonsClutch/BSpline.lean` | `3e2961e765cc0aeebe232bb2b4e9667b036fc06a22e5b0960873cc51a91d52bc` |
| `verus/bspline/emit_fixtures.lean` | `5f732e09b232c2bedb665f2670530b76de21a14f162fa508d999d9efe0288337` |
| Lean fixture transcript | `017afe06dfed89e45a802060b701daf2a00e4e6fc28aecd73ecbdd108c1274f0` |
| production output transcript | `eae31ce1e369e25a60883fd2d5206b56a14acc0cff072d7d2c3de1acb7da3814` |
| `verus/bspline/run_bspline_refinement.sh` | `1778824030783f0209d0217cfe158f4f98a3f68ea53e4cb964fc186f0fd9eb67` |
| `verus/bspline/BSPLINE_REFINEMENT_ASSUMPTIONS.md` | `6e463b0c24223f953163cde4a44d78a371d325000c1b36791726ca074da806ea` |
| `verus/bspline/evidence/bspline_refinement.txt` | `b3b32b8bdd617229670e8be3844bd7d2cc88774abe6c3bccc7af76246b6deeed` |
| `verus/bspline/BSPLINE_REFINEMENT.json` | `d50579898a58f449c0e28a9a77eac44975ae5a855ce560b191b42acc157f11a8` |
| `crates/clutch-bspline/oracle/check.py` | `1751674d5c7f32d5b29246ca67f659b9570212a5324d8c5f9bca324fc7cf98d7` |

The checked toolchains are Lean 4.33.0 commit
`d8b18978322de05a8f3dba51ef03cf5461676c17` and Rust
1.98.0-nightly commit `91fe22da8084a1c9e993d78d4a56f22ab8396236`.

## Semantic coverage

The Lean rows are computed, not transcribed Rust outputs.  Their rational
bases are backed by the existing model theorems:

- `clampedDegreeOne_exact`, `clampedDegreeTwo_exact`, and the open-clamped
  endpoint exactness theorems;
- `refineTwo_exact` and `refineThree_exact` for the concrete first/interior
  `BasisFuns` columns;
- `RationalBasis.Exact.pad` for global pane placement and canonical zeros;
- `uniform_rust_expanded_knot_linkage` for the generic uniform expanded-knot
  indexing/bracketing obligation;
- `canonicalLargestRemainderSelection` and
  `quantizeLargest_canonical_admissible` for deterministic priority, exact sum,
  and admissibility.

The actual rows cover nonuniform degree one; quadratic threshold tie behavior;
quadratic first and interior panes; cubic first pane and internal knot; and
both clamped endpoints. They exercise the public evaluator's production
degree-one specialization and the degree-two/three fixed-common-denominator
Cox--de Boor recurrence, followed by the canonical largest-remainder loop.

`BasisSpec::evaluate` first performs the complete hostile-input validation and
constructs a private-field `ValidatedBasisSpec` capability. Degree two uses the
common denominator `2*h^2`; degree three uses `12*h^3`. Because admitted smooth
bases have uniform power-of-two spacing `h`, every Cox divisor is `h`, `2*h`,
or `3*h`: the production path cancels powers of two by shifts and the only
possible odd factor, three, before checked multiplication. Scaled quotient and
remainder extraction likewise uses shifts and, for cubic bases, division by
three. There is no production `Fraction` or variable-width GCD dispatch.

The former exact reduced-`Fraction` evaluator is retained only under
`#[cfg(test)]` as an oracle. The 15 Rust tests compare its output byte for byte
with the fixed-denominator path across every admitted degree-two/three pane
shape, clamped edges, internal boundaries, translated coordinates near
`u128::MAX`, and the last accepted/refused arithmetic bounds. That oracle is
not constructible or callable in production builds and is not substituted for
the public seam by the Lean runner.

The concrete fixture-to-CSV association remains reviewed: Lean proves the
generic cell/index facts and the fixture rational exactness, but there is no
single theorem reducing every CSV input through a Lean parser and evaluator to
the fixture rational.  This is an explicit finite adapter boundary.

## Refutable mutations

The runner creates private temporary crate copies from the pinned production
source.  Each mutant must compile, execute all eight rows, and disagree with
the Lean transcript:

| Mutation | Required result |
|---|---|
| replace strict-greater remainder replacement with greater-or-equal behavior | red; lower-index tie rule changes |
| suppress the residual-award loop | red; partition result changes/refuses |
| write smooth floors at local rather than `pane + local` indices | red; interior global placement changes |
| use `span = degree + pane + 1` | red; open-clamped recurrence indexing changes |
| change closed-top `>=` to `>` | red; exact upper endpoint no longer returns the final one-hot vector |

These are executable semantic mutants, not assertions about a separate oracle.

The pinned runner passes a Lean build, 15 Rust tests, all 8/8 byte-exact
Lean/production rows, and all 5/5 expected-red source mutants. A separate
independent Python campaign passes 31,814 exact differential cases with seed
`880230` and six mutants. The Python result and the broader Rust
fixed-denominator/`Fraction` differential strengthen finite testing; neither
enlarges the eight-row Lean refinement claim.

## Exact claim and trust boundary

The permissible claim is:

> Lean 4.33.0 checked the named exact-basis, knot-linkage,
> largest-remainder, and admissibility theorems about the mathematical model at
> the pinned Lean source digest.  Eight model-computed vectors agree byte for
> byte with `BasisSpec::evaluate` at the pinned complete Rust source and driver
> digests.  Five production-source mutants execute and disagree.  This is
> finite executable refinement evidence, not a universal theorem about Rust or
> an SBF binary.

The campaign does **not** prove:

- all valid degree-one through degree-three inputs or any degree-zero input;
- all parser/validation refusals or their ordering;
- universal correctness, exact cancellation, or overflow freedom of the
  production fixed-denominator recurrence and scaled-parts arithmetic;
- equivalence of the `#[cfg(test)]` reduced-`Fraction` oracle and production
  fixed-denominator evaluator outside the finite Rust differential cases;
- equivalence between Lean naturals and every bounded `u64`/`u128` execution;
- compiler correctness, SBF code generation, Solana accounts/runtime, source
  authentication, or deployment identity; or
- any active SBF integration claim.

The full assumption list is
`verus/bspline/BSPLINE_REFINEMENT_ASSUMPTIONS.md`.  Despite the evidence
directory name, this campaign invokes no Verus theorem. A full source-level
proof would still need a checked translation/refinement of validation and the
private capability boundary, pane selection, virtual expanded-knot lookup,
fixed-common-denominator recurrence, exact divisor cancellation, scaled-parts
arithmetic, and canonical selection for every admitted input.

## Reproduction

```sh
sh verus/bspline/run_bspline_refinement.sh
```

The runner checks source/toolchain/transcript pins, builds Lean, runs the crate's
ordinary tests, compares live model and production rows, and then requires all
five mutants to go red. It uses only private temporary directories and does not
touch `programs/clutch-sbf`.
