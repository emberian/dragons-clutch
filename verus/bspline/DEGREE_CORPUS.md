# The generated degree corpus

Status: **checked finite source-bound agreement on 3,360 generated rows; not a
universal refinement proof** (2026-08-21).

```sh
sh verus/bspline/run_degree_corpus.sh
```

## Why this exists next to `run_bspline_refinement.sh`

The eight-row campaign in this directory is bounded by human effort, and says
so: `BSPLINE_REFINEMENT_ASSUMPTIONS.md` item 2 records that "associating each
fixture's concrete `Split` literals with its CSV knot/value row is reviewed and
finite, not a general parser/refinement theorem". Every row in that campaign
was derived by hand — that is why there are eight of them, and why they cover
one first pane, one interior pane, one internal knot, one tie, and both
endpoints rather than a sweep.

`lean/DragonsClutch/BSplineCorpus.lean` removes the per-row cost for the
uniform family. It assembles pieces that were already proved in
`BSpline.lean` — `locateUniformPane`, `expandedKnotAt`, `uniformBasisFunsCell`,
`refineOne`/`refineTwo`/`refineThree`, `openClampedLeft`/`openClampedRight`,
`RationalBasis.pad`, and the canonical largest-remainder selection — into one
total computable function `uniformSmoothBasis?`, and proves:

* `uniformSmoothBasis?_exact` — for every positive uniform grid, every degree
  one through three, and **every** observed value including both closed
  endpoints and everything the edge policy clamps, the evaluation succeeds and
  its exact rational vector is a partition of unity. The interior case chains
  `refineOne_exact` → `refineTwo_exact` → `refineThree_exact`: each column's own
  exactness discharges the `a + b = q` hypothesis of the next, so no column
  assumes the conclusion.
* `uniformSmoothWeights?_sum` — the emitted integer weights sum to `D`, by
  `quantizeLargest_admissible`.

This is what makes a generated row non-vacuous: a disagreement with Rust is a
disagreement about the semantics, not a report that the Lean side computed
nothing.

## Cross-check against the hand-derived rows

Five `decide` theorems in `BSplineCorpus.lean` check that the generic evaluator
reproduces the hand-derived witness vectors it never saw:

| theorem | row |
| --- | --- |
| `corpus_matches_degreeTwoFirst` | equals `refinementDegreeTwoFirst` literally |
| `corpus_matches_degreeTwoInterior` | equals `refinementDegreeTwoInterior` literally |
| `corpus_matches_degreeThreeFirst` | equals `2 ×` it — the hand derivation seeded column three from the reduced `(16,40,8)/64`, the generic evaluator never reduces |
| `corpus_matches_degreeThreeBoundary` | equals it literally, including all three degenerate-split literals |
| `corpus_endpoints_are_open_clamped` | both closed endpoints take the `openClamped*` branch, and exterior values clamp onto them |

A wrong `span`, pane, or knot expansion would break these while leaving every
hand-derived theorem in `BSpline.lean` untouched.

## The corpus

3,360 rows: six uniform grids × degrees {1, 2, 3} × eight scales
`D ∈ {1, 2, 3, 7, 8, 16, 63, 1000}` × every integer observed value from two
below the low endpoint to two above the high one. 1,120 rows per degree.

The grids reach every structural case: a single-pane grid where both clamped
end effects overlap (`count = 2`), short grids where the end panes still touch,
and grids long enough to contain an interior pane whose four expanded knots are
all distinct. Scales span the exact-shift path (`D` a power of two at least the
gap), coarse scales where the residual is maximal, and odd scales coprime to
every pane denominator.

**Degree one is in the corpus on purpose.** Production evaluates it through a
*different* function — `evaluate_degree_one`, the division/shift specialization
— than degrees two and three, while the Lean side runs the same generic
recurrence for all three. Those 1,120 rows are therefore a check of the
specialization against the general rule, which
`DISTRIBUTIONAL_CLAIMS_DESIGN.md` §15 explicitly left owed ("a future deg ≥ 2
implementation must restate the general rule consistently before relying on
it").

## Mutants

Eight temporary mutations of the pinned production source must compile, execute
all 3,360 rows, and disagree. The runner never rewrites the checkout.

| mutation | edit | rows disagreeing |
| --- | --- | --- |
| `cubic-denominator` | `12*h³` common denominator becomes `6*h³` | 377 |
| `quadratic-denominator` | `2*h²` common denominator becomes `h²` | 414 |
| `high-endpoint-repeat` | the clamped high endpoint denotes `knots[K-2]` | 878 |
| `tie-direction` | largest-remainder exact ties go to the higher index | 129 |
| `residual-awards` | the residual atoms are never awarded | 1496 |
| `pane-placement` | the local block is written at global index zero | 894 |
| `span-index` | `span = degree + pane + 1` | 1606 |
| `closed-top` | the top coordinate is half-open | 720 |

`cubic-denominator` is the executable form of a real documented divergence:
`DISTRIBUTIONAL_CLAIMS_DESIGN.md` §2.2 still prints `6h³` for the cubic while
the shipped evaluator uses `12h³`. `tie-direction` disagreeing on 129 rows is
the evidence that the corpus actually contains exact largest-remainder ties
rather than only generic points.

The symmetric low-end guard `index <= degree` in `expanded_knot` is
deliberately **not** mutated: flipping it to `index < degree` is a semantic
no-op, because the interior branch then computes `knots[degree - degree] =
knots[0]` anyway. It is recorded here so a reader does not mistake its absence
for an oversight.

## Registration — OWED

This campaign is **not** yet a `MANIFEST.baseline.json` gate. It was written
and left unregistered deliberately: `scripts/test_baseline_manifest.py` pins
the gate count (`assertEqual(len(gates), 100)`), so adding the entry without
regenerating the manifest in the same commit makes
`scripts/baseline_manifest.py check` fail for every other lane, and
regenerating requires a clean tree plus a full `--run-gates` pass. That is the
reseal cadence's job, not a shared-tree edit.

The block to add to `scripts/baseline_manifest.py`, immediately after
`proof.bspline_finite_refinement`, together with bumping the pinned gate count
to 101:

```python
{
    "id": "proof.bspline_degree_corpus",
    "section": "current-proof-boundary",
    "command": "sh verus/bspline/run_degree_corpus.sh",
    "expected": {"mode": "zero", "exit": 0},
    "proof_content": "checked-finite",
    "key_patterns": [
        r"^lean_version=",
        r"^production_source_sha256=",
        r"^degree_1_rows=1120$",
        r"^degree_2_rows=1120$",
        r"^degree_3_rows=1120$",
        r"^baseline=PASS rows=3360 seam=BasisSpec::evaluate$",
        r"^mutation=.* status=EXPECTED_RED rows_disagreeing=",
        r"^status=PASS$",
        r"^boundary=",
    ],
    "note": (
        "digest-bound 3,360-row Lean/Rust comparison generated from the "
        "checked generic evaluator, plus eight source mutants; uniform "
        "stored grids only, and no universal source, SBF, or runtime "
        "refinement is claimed"
    ),
},
```

## Assumptions

Everything in `BSPLINE_REFINEMENT_ASSUMPTIONS.md` applies, with item 2 replaced
by a strictly weaker obligation and one new item:

2'. The generic evaluator's control flow is *asserted* to denote the production
    entry point's: edge clamp, the two closed-endpoint branches, uniform pane
    location, `span = degree + pane`, the `BasisFuns` columns, and pane-offset
    padding. Lean proves that flow exact; it does not prove that Rust
    implements that flow. The five agreement theorems above and the 3,360
    executed rows are the finite evidence for the association, and the eight
    mutants are the evidence that the association is not vacuous.

9.  **Uniform stored grids only.** Every linkage theorem underneath this file
    is stated over `uniformStoredKnots origin gap count`. Nonuniform degree-one
    grids — which production admits — are outside this corpus, and remain
    covered only by the hand-derived row `1,7,5,c,n,3,0,3,8` in the eight-row
    campaign. Degree zero is outside both.

The result assumes no Solana account, source/archive, statistic, signer,
Token-2022, SBF VM, runtime, deployment, or network fact. It does not prove all
admitted inputs, hostile-input refusal order, or arithmetic-bound sufficiency
outside the finite rows.
