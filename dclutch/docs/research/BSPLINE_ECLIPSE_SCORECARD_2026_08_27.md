# Does LiabilityBasisV2 plus B-splines eclipse generation one? — 2026-08-27

Status: honest capability comparison between the generation-one B-spline stack
and the generation-three `LiabilityBasisV2` spline slice landed by the
LB-SPLINE lane. It is not release, deployment, or verification evidence, and
it does not close `O-013`.

## Why this document exists

`docs/ASPIRATION_LEDGER.md` M-4 is the sharpest finding in that audit: a named
ember requirement — *"it was vital to me to be able to do these properly
shaped dynamics"* — that was dropped, restored on his personal intervention,
and dropped again by the rewrite. Frontier 2 answered it with certified
nonnegative integer partition-of-unity bases and shipped one two-claim ramp.
The ledger's verdict on that answer was precise and worth repeating:

> `O-013` is a decision about a *basis*; it is not a decision about *"'5 fixed
> bands' is really not good enough."*

This lane built the basis family the requirement actually named. The question
this document answers is the narrow one: **is the successor now at least as
capable as generation one on shaped dynamics?**

**The answer is no — not yet.** Two named things stand in the way, and one of
them is a repeat of a mistake generation one already made and recorded.

## What landed

Pure theory, `formal/dclutch-semantics/DClutchSemantics/LiabilityBasisV2Spline.lean`
and `LiabilityBasisV2SplineAbi.lean`, 65 theorems, zero `sorry`, whole tree
green:

- `cumulativeFloorBoundaryV2` — one apportionment boundary, generalizing the
  ramp lane's `cappedRampComplementFloorBoundaryV2` from width two to every
  width. `apportion_width_two` and `cumulativeFloorBoundaryV2_eq_cappedRamp`
  prove the ramp is its width-two instance, so the named boundary was
  generalized rather than replaced.
- `deBoorStep` / `deBoorLevels` — Cox-de-Boor on integer numerators over one
  common denominator.
- `SplineProfile.evaluate_partition` — **payouts sum to exactly `Q`** at every
  admitted coordinate and every width. This is the theorem M-4 asks for, over
  the integer-apportioned form rather than only the rational one.
- `SplineProfile.evaluate_within_one_atom` — every claim is within one
  collateral atom of its exact rational share.
- `SplineProfile.evaluate_zero_outside_support` — B-spline local support
  survives the apportionment.
- `SplineProfile.basis : Basis` — the family AS a `LiabilityBasisV2.Basis`, so
  split, merge, transfer, terminal redemption and the `Q * peak(T)` solvency
  envelope are **inherited**, not re-proved.
- A 144-byte hostile-decodable record, an ordered check list, and a
  Lean-emitted corpus of 28 agreement and 32 refusal cases, byte-checked by
  `crates/dclutch-liability-basis-v2-kernel/check-generated-spline.sh`.

The evaluator was checked against exact values before being trusted: the
clamped cubic returns `[1/8, 3/8, 3/8, 1/8]` at its midpoint, and the uniform
clamped cubic returns `[1/48, 23/48, 23/48, 1/48]` at a span midpoint.

## The scorecard

| Capability | Generation one | This lane | Verdict |
| --- | --- | --- | --- |
| Degrees | 0–3 | 1–3 here; degree 0 is the existing `categoricalBasis` | equal |
| Exact evaluation | Cox-de-Boor, `u128` fixed common denominator | Cox-de-Boor, unbounded `Nat` in Lean, `u128` in Rust | equal |
| Rational partition of unity, machine-checked | yes, Lean, **uniform grids only** | yes, Lean, **any knot vector**, degree-generic | **gen-3 ahead** |
| Integer partition of unity, machine-checked | yes (`quantizeLargest_canonical_admissible`) | yes (`apportion_sum`) | equal |
| Rounding rule | largest remainder, lowest-index tie; needs a selection certificate with existence *and* uniqueness proved; Rust defensively refuses `residual > degree` | one floor of a running sum; exact by telescoping; no selection, no tie-break, no residual check exists to refuse | **gen-3 ahead** on auditability |
| Per-claim accuracy | < 1 atom | < 1 atom (`apportion_within_one_atom`) | equal |
| Reflection symmetry of the rounding | tested and held when remainders are distinct | **not symmetric** — order-dependent | **gen-1 ahead** |
| Interior knot multiplicity | **structurally forbidden**; the recorded consequence was that a tent is exact at degree one and inexact at every smooth degree | admitted; degenerate spans are skipped by the span locator, which is also what forces every de Boor denominator positive | **gen-3 ahead** |
| Non-uniform grids at degree ≥ 2 | **refused** (`UniformSpacingRequired`) | admitted | **gen-3 ahead** |
| Supply algebra preservation | proved per-property for the spline path | inherited from one `Basis` instance | **gen-3 ahead**, structurally |
| Maximum claims, physically | 16 | 10 (twelve knot slots in the first record) | **gen-1 ahead** |
| Edge policy | clamp **or** refuse, caller-selected | clamp only | **gen-1 ahead**, minor |
| Degree ≥ 2 arbitrage gate | built (moment cone V1b), `decide`-checked both directions, table proved *tight* — and provably incomplete on multi-span grids, with a named false acceptance. **Gen-2 built a second, integer one that refutes it** | **absent** | **behind — the largest gap. But see the correction below: the fix exists and is a transplant, not research** |
| Shape compiler (analytic target → coefficients + error certificate) | built, host-only | absent | **gen-1 ahead** |
| Occupation / path accumulator | built, never integrated | absent | gen-1 ahead on paper only |
| Lean↔Rust agreement corpus | 3,360 rows, **uniform grids only** | 28 agreement + 32 refusal, covering multiplicity, non-uniform grids, both clamps, both scale extremes | gen-1 ahead on volume, gen-3 ahead on coverage classes |
| Wired to a live consumer | no | no | equal, both no |

## The two things standing in the way

### 1. The claim plane has shipped ahead of the price plane again

Generation one built the moment-cone gate because at degree ≥ 2 the simplex
condition `p ≥ 0, sum p = S` **stops being the no-arbitrage condition**.
Interior degree-2 basis functions peak at `3/4`, so `3·1 − 4·e_j` is a
portfolio with a globally nonnegative payoff and a strictly negative price at
`p = S·e_j`. That is an executable arbitrage, not a theoretical one, and it
does not exist at degree ≤ 1.

Generation one shipped the claim plane first anyway and recorded the
consequence in its own words: *"The claim plane landed; the admission story
for prices did not move with it."* Degree-2 and degree-3 markets were
creatable with the hole open for about two days.

**This lane has done the same thing.** Nothing here gates prices. The
mitigating facts are that nothing is wired to a consumer, no Market can
select this basis, and no layout has changed — so the hole is not reachable.
The load-bearing point is that it must be closed **before** a Market can
select degree ≥ 2, not after, and the trigger has to be written down now
rather than rediscovered.

Generation one's own gate was **provably incomplete** on multi-span grids, with
a pinned false acceptance (degree two, five claims, breakpoints `[0,1,2,3]`:
`p/S = (1/3,2/3,0,0,0)` passes the gate while `(1, -2, 10, 40, 64)` has the
globally nonnegative payoff `(3x-1)^2` and price `-S`).

### Correction: there is a third option, and it is the right one

**An earlier revision of this document said the choice was "port a
sound-but-incomplete gate, or do the per-span Hausdorff witness generation one
designed and never built." Both halves of that sentence were wrong**, and the
reason is that this document compared generation one to generation three and
never looked at **generation two**. `ASPIRATION_LEDGER.md` `G-1` caught it the
same afternoon.

Generation two built the gate independently, over integers, and it is not in
the purged monolith — it is intact on `dragons-clutch` `main` today as
`crates/clutch-price-measure`, 8,843 Rust lines, dated 2026-08-23/24. It
contains **both** of the things named above:

- `verify_continuous_price_measure_v2` — the per-span Hausdorff/Bernstein
  witness generation one designed. **It was built.** It enforces
  `w1^2 <= 4*w0*w2` at degree two and `w1^2 <= 3*w0*w2`, `w2^2 <= 3*w1*w3` at
  degree three *per span*, then requires exact reconstruction through a
  transfer matrix generated from the B-spline recurrence rather than supplied
  by the caller.
- `verify_quantized_atom_mixture_v1` — the one that actually fits
  `LiabilityBasisV2`. It checks that the price vector is a nonnegative integer
  mixture of *actually attainable* quantized payout vectors:

  ```text
  sum_k weight_k = W
  sum_i price_i  = D
  atom_k = evaluate(coordinate_k)              -- recomputed, never supplied
  price_i * W = sum_k weight_k * atom_k[i]     for every claim i
  ```

  That is convex-hull membership over the real atom set. Checked `u128`, no
  floats, `no_std`, allocation-free, at most 16 atoms, a fixed 544-byte
  certificate.

**It refutes generation one's gate in both directions, and the tests are in the
tree.** `crates/clutch-price-measure/tests/adversarial.rs:262` asserts that V1b
*accepts* `(4,8,0,0,0)/12` — the pinned false acceptance quoted above — that the
generation-two checker returns `Err(QuadraticMomentOutsideCone { span: 0 })`,
and that the arbitrage portfolio costs exactly `-12 = -S`. Line `281` goes the
other way: a live quantized point V1b *refuses* is given a valid single-atom
certificate. Generation two fixes both the unsoundness and the over-refusal.

**Why it fits `LiabilityBasisV2` better than either alternative.** The quantized
verifier needs exactly one thing from a basis: a deterministic integer evaluator
whose payouts sum to a fixed scale. No uniformity, no knot vector, no degree, no
span decomposition. Every axis on which this lane is *ahead* of generation one —
interior knot multiplicity, non-uniform grids at degree >= 2, a
partition-of-unity proof not restricted to uniform grids — is an axis on which
generation one's moment cone cannot even be stated, and to which hull membership
is simply indifferent. The expensive half is off-chain: the 2,850-line exact
solver and the 1,058-line 2048-bit Bareiss substrate are the *prover*; the chain
runs only the verifier.

**What is honestly still open.** Generation two's gate has **no Lean at all** —
48 Rust tests, zero theorems, and its own promotion gate 9 (*"extend Lean only
for the exact checker correspondence proved"*) was never run. It also carries a
named residual: *"The fixed `u64` mass denominator remains an inner-certificate
bound; support-boundedness alone does not prove that every lattice price has
such a small denominator"* (`docs/design/PRICE_MEASURE_WITNESS_V2.md:188`), with
`OutOfProfile` distinguishing "no representation" from "representation too large
to encode", so it fails **closed**. That is a real hole, and a much better hole
than an unsound one.

So the trade is plain: **generation one's gate is machine-checked and wrong;
generation two's is only Rust-tested and right.** Generation three has the Lean
machinery neither predecessor pointed at this problem, and the theorem worth
stating is finite and `decide`-shaped — *a price admitted by the certificate
admits no arbitrage against the finite atom set*. Landing that would put
generation three ahead of **both** predecessors on the largest gap in this
table, rather than merely level with one of them.

Transplanting requires a `docs/compost/` manifest under `COMPOST.md`'s rule;
`PYTH-FIXTURE-001` is the only existing row and the template.

**Degree 1 is unaffected.** At degree ≤ 1 the gate is vacuous and provably so.
A degree-1 wave is the whole vanilla-option span at grid resolution and needs
no price-plane work at all.

### 2. Width and the shape compiler

Ten claims against generation one's sixteen is a property of the *first*
144-byte record, not of anything proved. The Lean carries no width bound. But
until a wider record exists, this is a real regression on the axis a user
would notice first.

The shape compiler is the piece that turns *"pay me proportionally to how far
the price fell"* into a coefficient vector. Without it, a user has the basis
but not the vocabulary, and generation one's framing stands: the tradable span
is a finite spline space, and a target outside it may be compiled only with an
explicit approximation certificate and a named error norm.

## What the layout slice needs

Frontier 2's own gate — *"Market and Claims layouts do not change until the
pure theorem and hostile translation corpus are accepted"* — is now met for
this slice. What a layout change would then have to carry:

- **A resolved coordinate, not a winner.** Source resolution produces a
  categorical outcome today. A spline Market resolves to a
  `RationalCoordinate` (signed numerator over a positive denominator), which
  is the coordinate type Product V2 already uses. Generation one recorded a
  hard consequence here: interval-valued evidence cannot resolve a degree-2/3
  market at all under a point-evidence rule, which it called *"the biggest
  capability regression the design accepts"* and the reason degree ≤ 1 was the
  recommended first wave. That reasoning transfers unchanged.
- **`Q` collateral atoms per complete set**, in place of one.
- **`maximum_liability_v2 = Q * peak(T)`** as the pre-resolution envelope,
  which `Basis.peak_bound_globally_solvent` already certifies and which is
  proved *attained* for both existing evaluator families.
- **The basis descriptor**: Frontier 2's `basis_width`, `payout_scale`,
  `evaluator_release`, `certificate_schema`, `capacity_profile`. The 144-byte
  record here is a *request* format, not an account layout, and should not be
  mistaken for one.
- **A CU measurement.** Degree 3 at width 10 is six de Boor weights, roughly
  ten `u128` multiply/divide pairs, plus `K` divisions for the apportionment.
  Unmeasured.

## Provenance of the generation-one column

Two integrity events are already on this project's record, both about tables
that were asserted rather than checked. Every generation-one claim in the table
above was therefore re-read from the actual blob in `dragons-clutch` history,
not recalled and not taken from a summary:

| Claim | Source, verified |
| --- | --- |
| `RationalBasis` is `Nat` numerators over one positive common denominator, unreduced | `lean/DragonsClutch/BSpline.lean`, blob `1bd69c4` — *"No reduction or coprimality is required"* |
| Degrees 0–3; `refineOne/Two/Three` are Piegl–Tiller `BasisFuns` column steps | same blob, module header |
| Rounding is deterministic largest remainder, ties to the lower index | same blob, `WEIGHT-ROUND-01`, `quantizeLargest`, `remainderOrder` |
| The knot-linkage theorem covers uniform grids only | same blob: *"closes that obligation for every positive uniform grid and degree one through three"* |
| 16 claims, degree ≤ 3, `EdgePolicy::{Clamp, Refuse}`, `UniformSpacingRequired` at degree ≥ 2 | `crates/clutch-bspline/src/lib.rs`, blob `3e37a37` |
| Interior multiplicity structurally forbidden | same blob: `if knot <= previous { return Err(Error::InvalidKnot) }` |
| Degree-2 interior ceiling `3/4`, degree-3 interior ceiling `2/3`, gate off at degree ≤ 1 | `lean/DragonsClutch/MomentCone.lean`, blob `f9a4540`, all `decide`-checked |
| The multi-span false acceptance | `docs/research/DUAL_IS_THE_MEASURE.md`, blob `ce9a991`, §7.6.7 — quoted verbatim above |
| *"the biggest capability regression the design accepts, and the reason deg ≤ 1 is the recommended first wave"* | `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md`, commit `eacf95fa` |

The generation-two column added by the correction above was verified the same
way, in `dragons-clutch` working tree on `main`-equivalent content:

| Claim | Source, verified |
| --- | --- |
| `clutch-price-measure` is 8,843 lines across five modules and three test files | `wc -l crates/clutch-price-measure/{src,tests}/*.rs` |
| `verify_quantized_atom_mixture_v1(bound, prices, certificate)` exists with that signature | `src/atom_mixture_v1.rs:564` |
| It refuses generation one's pinned false acceptance, and the arbitrage costs `-S` | `tests/adversarial.rs:262`, asserting `v1b_degree_two_accepts` *and* `Err(QuadraticMomentOutsideCone { span: 0 })` *and* `cost == -12` |
| It also fixes generation one's over-refusal | `tests/adversarial.rs:281` |
| The `u64` mass-denominator completeness residual, failing closed via `OutOfProfile` | `docs/design/PRICE_MEASURE_WITNESS_V2.md:188`, `:271`, `:354` |
| `G-1` says the ledger itself could not judge soundness — this document now does | `ASPIRATION_LEDGER.md:1285`, *"Whether the quantized checker is sound, complete, or cheap is not something this sweep can say"* |

One further fact from that reading belongs here, because it is the closest
thing to a tie in this comparison. Generation one's own `BSpline.lean` header
says:

> The connection from a stored knot vector to those positive distance records
> is an integration theorem **still owed** by the executable evaluator. This
> file does not pretend that a caller-supplied valid `Split` proves the knot
> indexing.

This lane has the same obligation and discharges it differently: rather than
owing a theorem that the located span yields positive de Boor denominators,
`SplineProfile.admits` **decides** it, and it is a premise of every theorem
rather than an assumption. That is not strictly stronger — a proof would cover
every profile at once where a check covers one evaluation — but it is the
difference between an owed theorem and a refusal, and a refusal cannot be
forgotten.

## The verdict, stated plainly

On the **claim plane** — the basis itself — the successor is now ahead of
generation one on knot multiplicity, non-uniform grids, the generality of the
machine-checked partition of unity, and the auditability of the rounding rule;
behind on physical width, edge policy, and rounding symmetry; and equal on
degree range, exactness and accuracy.

On the **whole instrument**, it is behind, because no price-plane gate and no
shape compiler exist here and nothing is wired to a consumer. That verdict is
unchanged by the correction above — but its *cost* is much lower than it first
appeared. The largest gap in this table is not open research: generation two
already wrote a sound integer gate that fits `LiabilityBasisV2` better than
generation one's, and the remaining work is a reviewed transplant plus the Lean
correspondence neither predecessor ever ran.

`O-013` remains open. What has changed is that its second slice is no longer
a ramp, and the row's own closure condition can now be written against
something real. Ledger recommendation 3 — surface `O-013` to ember as a
substitution rather than a table cell — is **not** discharged by this lane and
should not be treated as discharged by it. The question *"is a certified
integer partition-of-unity basis the same thing as 'properly shaped
dynamics'?"* is his to answer, and it is now easier to answer because the
answer can be evaluated instead of imagined.
