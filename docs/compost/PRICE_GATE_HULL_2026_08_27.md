# PRICE-GATE-HULL-001: the degree-`≥ 2` integer hull-membership requirement

## Decision

dClutch adopts, from Dragon's Clutch, the **requirement and the shape** of
generation two's quantized price-admission certificate. **No Rust source
crosses the repository boundary.** The mathematics was written in Lean against
`LiabilityBasisV2.Basis` and shares no line with the original; the `no_std`
kernel is handwritten against the Lean-emitted corpus, per this repository's
own translation pattern.

This is a transplant of an *invariant*, not of a function body — the posture
`COMPOST.md` states as preferred.

## Source and provenance

- Source repository: `~/dev/dragons-clutch`, branch `main`
- Source path: `crates/clutch-price-measure/` — 8,843 Rust lines across eight
  files, 5,715 source and 2,128 test
- Introducing commit: `d085dbdfa7c90448309def7a4053c847e79a89b2`, 2026-08-23
  00:41 −0400, *"Separate continuous and quantized price certificates"*
- Last commit touching the crate:
  `be43c726ed04b725aa851d9dee6c87b5a882b588`, 2026-08-23 14:24 −0400,
  *"Search exact quantized supports through outcome width"*
- The whole crate has a **one-day history**: ten commits, all 2026-08-23.
  `src/atom_mixture_v1.rs` arrived in `7977d55f`.
- License: `AGPL-3.0-or-later` declared at
  `crates/clutch-price-measure/Cargo.toml:6`, `publish = false`. The repository
  root `LICENSE` is the GNU AGPL v3 text. There are no per-file SPDX or
  copyright headers anywhere in the crate.
- Provenance conclusion: **same project, same author, same license family.**
  Both repositories are ember's, both AGPL-3.0-or-later, and no third-party
  code is involved. No separate license decision is required. Because nothing
  is copied, no attribution obligation attaches to any byte in this tree
  either — the citation below is intellectual honesty, not license compliance.

## The retained semantic invariant

Stated once, in the form this tree proves it:

> A price vector `p` on a basis's own payout scale is admitted when a
> certificate exhibits it as a **nonnegative integer mixture of actually
> attainable payout vectors**:
>
> ```text
> 0 < W,  every weight positive,  sum weights = W
> W * p_i = sum over atoms of weight * evaluate(coordinate)_i   for every claim i
> ```
>
> where every atom is **recomputed by the basis's own evaluator** and never
> supplied by the caller.

The certificate needs exactly one thing from a basis: a deterministic integer
evaluator whose payouts sum to a fixed scale. That is precisely what
`LiabilityBasisV2.Basis` is, which is why this rule fits the successor better
than generation one's moment cone — the cone cannot even be *stated* over a
non-uniform grid or an interior knot multiplicity, and hull membership is
indifferent to both.

## The new semantic owner, and why it belongs there

| Concern | Owner |
| --- | --- |
| The mathematics, over an arbitrary `Basis` | `formal/dclutch-semantics/DClutchSemantics/LiabilityBasisV2PriceGate.lean` |
| The 320-byte record and its hostile decoder | `…/LiabilityBasisV2PriceGateAbi.lean` |
| Decided witnesses, including both refutation directions | `…/LiabilityBasisV2PriceGateExamples.lean` |
| The agreement and refusal corpora | `formal/dclutch-semantics/EmitLiabilityBasisV2PriceGateRust.lean` |
| The `no_std` physical implementation | `crates/dclutch-liability-basis-v2-kernel/src/price_gate.rs` |

It belongs in the liability-basis kernel because that crate already owns the
evaluator the certificate recomputes against, and because the admission
conjunct has to sit at the *evaluator* boundary — the point where a degree is
selected — rather than in any layout.

## What this tree adds that neither predecessor had

Generation two's crate contains **zero formal content**: no `.lean` files, 48
`#[test]` functions, no theorems. Its own promotion gate 9 (*"extend Lean only
for the exact checker correspondence proved"*) was never run. The repository's
Lean corpus (`lean/DragonsClutch/`, including `MomentCone.lean` with 14
theorems) does not reach this crate at all — and the one thing it *does*
formalize is generation one's V1b rule, which generation two's own tests
demonstrate to be unsound.

This tree proves, zero `sorry`, zero `native_decide`, three standard axioms:

- `Certificate.no_arbitrage` — a certified price admits no portfolio with a
  globally nonnegative payoff and a strictly negative price.
- `Certificate.check_eq_true_iff` — the decidable checker decides validity
  **exactly**, so a weakened conjunct fails rather than silently admitting.
- `Certificate.price_sum` — the simplex condition is a consequence of hull
  membership, not a second premise.
- `no_certificate_of_capped_claim` / `no_cap_of_attained_scale` — why the gate
  has teeth at degree ≥ 2 and why degree ≤ 1 is exempt *by proof* rather than
  by assertion.

## API and layout changes made during transplantation

| | Generation two | Here |
| --- | --- | --- |
| Certificate size | 544 bytes (`QUANTIZED_ATOM_MIXTURE_CERTIFICATE_BYTES_V1`) | 320 bytes |
| Magic | `DCQAMV1\0` | `DCLTPGT1` |
| Max atoms | 16 (`MAX_QUANTIZED_ATOMS = MAX_OUTCOMES`) | 10 — this record's width, same Carathéodory argument |
| Mass field | `weight_denominator: u64` (V2 surface: `common_denominator`) | `mass: u64` — **unchanged width, deliberately** |
| Basis binding | market / terms / basis / price identity digests plus a `body_digest`, checked against an adapter | scale, degree and width compared against an *already authenticated* `SplineRequestV2` |
| Degree range | certificate restricted to `{2, 3}` | record admits 1–3; a certificate offered at any degree is checked |
| Per-atom simplex check | `AtomSimplexMismatch` at runtime | not a check — `basis.partitionUnity` and `mixture_sum` are theorems |
| Mixture total check | `MixtureSumMismatch` at runtime | not a check — same reason |
| Edge policy | `Clamp` or `Refuse`, caller-selected, with `IncompleteRefusingDomain` | clamp only; the spline family's standing deficit, not a gate matter |
| Refusal vocabulary | ~30 structured variants carrying field and index payloads | 12 flat tags, `20`–`31`, ordered to match `PhysicalAbi.decodeChecks` position for position |

The two dropped runtime checks are the clearest gain from having proofs: what
generation two had to verify on every certificate, this tree knows about every
basis.

## Adversarial tests recreated from the invariant

All three are reproduced against **this tree's** evaluator, which shares no
line with either predecessor.

1. **`adversarial.rs:262` — generation one accepted an executable arbitrage.**
   Degree two, breakpoints `[0,1,2,3]`, scale 12. V1b accepts
   `p = (4,8,0,0,0)`; the portfolio `(1,-2,10,40,64)` — the B-spline
   coefficients of `(3x−1)²` — costs exactly `−12 = −S`. Reproduced as
   `decide` witnesses in `PriceGateExamples` (V1b is transcribed there *only*
   so its acceptance can be decided rather than recalled; nothing in this tree
   calls it), as `gen1_price_has_no_certificate_on_grid`, as three corpus
   refusal cases at tag 30, and as a 90-coordinate Rust sweep.

2. **`adversarial.rs:281` — generation one refused an attainable point.**
   `basis(2, [0,128,256,384], 10_000).evaluate(85) = [1128, 6667, 2205, 0, 0]`,
   which V1b refuses because `3 · 6667 > 2 · 10000`. It has an exact
   single-atom certificate. Reproduced as corpus agreement cases 0 and 1 (the
   point and its mirror at 299) and as `decide` witnesses. Worth recording on
   its own: generation two rounds by **largest remainder** and this tree floors
   a **running cumulative sum**, and both return the same vector.

3. **`adversarial.rs:321` — one price, several accepted witnesses.** An
   accepted certificate is *not* a canonical identifier for a price.
   Reproduced with this tree's own numbers: on generation one's basis the price
   `(0,7,5,0,0)/12` is certified both by a single atom at `5/6` and by an even
   mixture of `3/4` and `1` — both primitive, both admitted. Corpus agreement
   cases, plus `one_price_can_have_two_accepted_supports`.

## Old assumptions deliberately rejected

- **Digest binding.** Generation two binds four identity digests and a
  `body_digest`. Rejected: the certificate here is checked against an already
  authenticated record, so there is no hash preimage question and no second
  copy of the basis that could disagree with the first.
- **The continuous checker.** `verify_continuous_price_measure_v2` — the
  per-span Bernstein/Hausdorff witness generation one designed and generation
  two built — is **not** transplanted. Only the quantized hull rule is. The
  continuous rule is the one that would close the infinite-domain residual
  below, and it remains untaken.
- **The prover.** The 2,850-line exact solver (`atom_solver_v1.rs`) and the
  1,058-line 2048-bit Bareiss substrate (`fraction_free_v1.rs`) are the
  *prover*, not the verifier. They are off-chain and are not transplanted.
- **A certificate as a price identity.** Explicitly rejected, on generation
  two's own evidence — see recreated test 3.

## What is *not* closed, stated plainly

- **The `u64` mass residual is inherited unchanged.** A price inside the hull
  whose every representation needs a larger common denominator is refused. It
  fails closed. Generation two named it verbatim at
  `docs/design/PRICE_MEASURE_WITNESS_V2.md:188`: *"The fixed `u64` mass
  denominator remains an inner-certificate bound; support-boundedness alone
  does not prove that every lattice price has such a small denominator."* It
  is still open there (`:354`) and it is still open here.
- **`OutOfProfile` is not adopted, and that is a small regression.**
  Generation two's solver distinguishes *"a valid mixture exists but its
  primitive masses exceed `u64`"* from *"no representable certificate was
  found"* (`atom_solver_v1.rs:601`, `:618`). This tree has **no solver** — it
  only verifies — so it has nothing to make that distinction with, and a
  too-large representation is simply a refusal. Nothing is lost that this tree
  ever had; something is *not gained* that generation two had.
- **The Carathéodory support bound is prose in both trees.** Generation two
  asserts it in English at `src/lib.rs:224` and never proves it; the ten-atom
  capacity here is justified the same way in
  `LiabilityBasisV2PriceGateAbi.lean`'s header. **Soundness does not depend on
  it** — the capacity bounds only what can be *expressed*, so an under-capacity
  refuses rather than admitting. Completeness does depend on it, and neither
  tree has it.
- **The refutation of the false acceptance is over a finite grid.** 90
  coordinates. Showing that *no* certificate exists at *any* rational
  coordinate needs the arbitrage portfolio to pay nonnegatively over an
  infinite domain. Generation one asserted the continuous form analytically and
  never machine-checked it; generation two checked one supplied moment witness
  against its per-span cone. Neither closed it and neither does this.
