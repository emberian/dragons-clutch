# Distributional claims: a B-spline payout basis on a frozen knot grid

Status: **PROPOSED design document** (2026-08-18). Nothing in this file is a
landed implementation, a frozen policy, or an evidence claim. No code changes
this round; several implementation lanes are live in the tree and this document
deliberately touches none of their files. Claim labels follow the handoff
vocabulary: IMPLEMENTED / MODEL / PROPOSED / BLOCKER. Where this document says
"recommended," that is a design recommendation awaiting the falsifier gate and
the user's decision, not a selection. *(Since written: the terms-revision wave
at `927d4bc` landed the §6 TermsAccount revision — see the §15 implementation
addendum for exactly what is now IMPLEMENTED versus still PROPOSED.)*

The ask this design answers: real distributions and curves over outcomes, not a
handful of fixed bins — a parameterized kernel general enough to describe the
payoff shapes people actually want, cheap enough on the Solana side that the
generality costs nearly nothing at resolution and redemption time.

Ground truth read for this design:

- `crates/clutch-kernel/src/lib.rs` — IMPLEMENTED `PayoutVector`/`PayoutSet`
  (weights sum exactly to a common denominator `D`), `MAX_OUTCOMES = 16`,
  `MAX_PAYOUTS = 8`, resolve-by-index, exact-or-refuse redemption
  (`RemainderRequired`), `redeem_complete_set`, ceiling-rounded
  `required_collateral`.
- [`RESOLUTION_EVIDENCE_PLAN.md`](RESOLUTION_EVIDENCE_PLAN.md) §2 and
  `programs/solana-reference/src/resolution.rs` — the landed `derive_payout`
  seam, the ordinal-partition pinning, and obligation 18 of
  [`SOLANA_REFERENCE_ADAPTER.md`](SOLANA_REFERENCE_ADAPTER.md) (the
  `TermsAccount` revision this document designs).
- `programs/solana-layout/src/lib.rs` — IMPLEMENTED `TermsAccount` v2
  (1,304 bytes, self-certifying digest) and `PriceGridAccount`.
- [`POLICY_ANALYSIS_LOTS_FEES.md`](POLICY_ANALYSIS_LOTS_FEES.md) §1 — the
  P1-A fractional-payout analysis, candidates (a)/(b)/(c), and the §1.5
  complete-set redemption (landed in the kernel).
- [`BATCH_RELATION_V1_DESIGN.md`](BATCH_RELATION_V1_DESIGN.md) — portfolio
  coefficient vectors, virtual complete-set conversion, per-outcome
  conservation, and the §18 implementation record.
- [`../../PROJECT.md`](../../PROJECT.md) §6 — the current posture that
  fractional payout vectors are reserved for ambiguity policies. **This design
  changes that posture** (§10.1).

---

## 1. The defect and the thesis

Today's product resolves to **one cell of a step function**. Every claim pays
`1` or `0` (deg-0 indicator basis), and a market with five bands answers "which
band" with a knife edge at every boundary: an atom of price movement across a
boundary moves a full collateral unit of payout. Five — or sixteen — fixed
bands cannot express "pay me proportionally to how far the price fell," a
capped linear payoff, a smooth tail hedge, or any curve at all. Adding bins
does not fix this: payout resolution scales linearly with state, split/merge
loop length, and mint count, and the knife edges multiply rather than vanish.

**Thesis.** Replace the indicator basis with a **B-spline partition of unity on
a frozen knot grid**, degree `d ∈ {0, 1, 2, 3}`:

- **Degree 0 is exactly the current bins.** Deg-0 B-splines on a knot vector
  are the indicators of its cells; the obligation-18 threshold boundary table
  is the deg-0 special case of this design, which is why the `TermsAccount`
  revision for both is designed here as **one** revision (§6).
- **Degree 1 (hat functions) buys the vanilla-option span at grid
  resolution.** Claim `i` pays the hat function centered at knot `t_i`. A
  portfolio holding `g(t_i)` units of claim `i` pays the piecewise-linear
  interpolant of `g` at the resolved value — every call, put, spread,
  butterfly, and capped linear payoff with strikes on the grid, from sixteen
  claims. By linear precision of the hat basis, the portfolio with
  coefficients `t_i` pays the resolved value itself: the "linear claim" is a
  portfolio, not a new product.
- **Degrees 2–3 buy smooth curves** (`C^1`/`C^2`), at the cost of a genuinely
  harder ambiguity story (§5.3); they are specified here but recommended for a
  second wave.

Every standing invariant survives because every one of them depends only on
two properties of the resolved weight vector: **nonnegativity** and **exact
pointwise sum-to-one** (weights sum to `D`). B-splines have both by
construction; §3 states and proves the resulting solvency theorem, and §2.3
gives the integer construction that keeps the sum *exactly* `D` under
quantization — the one place the thesis could have broken, and the place this
document checks hardest.

A second, quieter benefit: for `d ≥ 1` the payout is Lipschitz in the resolved
value (slope at most `D/gap` per claim), so the value of manipulating the feed
near a boundary is proportional to the displacement achieved, instead of the
current all-or-nothing knife edge. The adversarial price of an oracle nudge
goes from "one full unit per straddling holder" to "linear in the nudge."

What this is not: not more outcomes (`MAX_OUTCOMES` stays 16), not a new venue
order family (portfolio orders already express curves, §7), not a continuous
payout oracle (resolution still consumes one sealed `WindowResult` through the
same evidence gate), and not a relaxation of any refusal.

---

## 2. Basis family specification

### 2.1 Knot grid

A market's terms freeze one knot vector `t_0 < t_1 < … < t_{K-1}` of `u128`
values in the admitted value domain `[0, MAX_VALUE = 10^24]`, together with a
degree `d` and the weight denominator `D` (the payout set's existing common
denominator — no second `D` is introduced). Knot semantics per degree:

| `d` | knot semantics | claims `n` | nonzero weights at any `x̂` | `K` bound (`n ≤ 16`) |
|---|---|---|---|---|
| 0 | interior cell boundaries (the §2.2 boundary table of the evidence plan, verbatim) | `K + 1` | 1 | `K ≤ 15` |
| 1 | claim anchor sites: claim `i` = "the value at `t_i`" | `K` | ≤ 2 | `K ≤ 16` |
| 2, 3 | clamped uniform grid anchors | `K − 1 + d` | ≤ `d + 1` | `K ≤ 17 − d` |

so `MAX_KNOTS = 16` bounds every degree. Values outside `[t_0, t_{K-1}]` are
handled by the frozen **edge policy** (§5.4): `EDGE-CLAMP-01` (default —
evaluation clamps, so the extreme claims pay their full weight, flat
extrapolation) or `EDGE-REFUSE-02` (out-of-range `x̂` refuses into the failure
policy). Under clamping the map from resolved value to weights is total on the
whole admitted domain; there is no unmapped value.

Spacing variants:

- **Uniform power-of-two spacing** (`t_{i+1} − t_i = 2^s` for all `i`,
  declared by a `uniform_log2_spacing = s` terms field and *checked* against
  the explicit knot array at decode — the array is always the single semantic
  owner; the field is a validated promise, never a second truth). This
  enables the shift-based settlement path and the deg-1 exactness argument
  (§2.4), and is **mandatory for `d ≥ 2`** (§2.5).
- **General strictly-increasing knots** (`uniform_log2_spacing = 0xFF`),
  admitted for `d ∈ {0, 1}`. This is how log-spaced strikes are expressed:
  the author computes geometric knots offline and freezes them; no logarithm
  ever executes on-chain. Deg-1 evaluation over non-uniform knots costs one
  extra integer division versus the uniform shift (§2.3) — it is not a
  different algorithm.

Decision — **knots do not unify with `PriceGridAccount`**. The price grid is
venue infrastructure: `u64` limit ticks on `[0, PRICE_SCALE]`, shared across a
realm's books, owned by order admission. Knots are resolution infrastructure:
`u128` observation-domain values, per-market, owned by the immutable terms
digest. Unifying them would couple venue admission to resolution semantics,
force `u128` ticks on the relation, and give one account two owners.
Rejected; knots live inside `TermsAccount` (§6), self-certified by the same
digest that already binds the payout set.

### 2.2 Weight map

At resolved value `x̂` (after edge handling), find the pane
`[t_k, t_{k+1})` containing `x̂` by linear scan (≤ 15 comparisons, exactly
`cell_of`'s cost today; under the uniform flag the pane index is
`(x̂ − t_0) >> s` and the scan is skipped), let `g = t_{k+1} − t_k` and
`u = x̂ − t_k ∈ [0, g)`. The exact rational basis values on that pane are

```text
d = 0:  N_k = 1
d = 1:  N_k = (g − u)/g,   N_{k+1} = u/g
d = 2:  { (h−u)², h² + 2hu − 2u², u² } / 2h²          (uniform h; interior pane)
d = 3:  { (h−u)³, 4h³ − 6hu² + 3u³, h³ + 3h²u + 3hu² − 3u³, u³ } / 6h³
```

Each numerator is a nonnegative integer for integer `u`, and each family sums
exactly to its denominator (`2h²`, `6h³`) — checked by direct expansion, and
pinned by an exhaustive falsifier (§12). For `d ≥ 2` the first and last `d−1`
panes use the clamped end-pane polynomials (derived once, offline, by exact de
Boor over the clamped knot vector with end multiplicity `d+1`; same
denominators). Closed-top convention: `x̂ = t_{K-1}` maps to the last basis
function at full weight, mirroring the kernel plan's closed top cell.

### 2.3 Integer weights and the one named rounding boundary

The kernel consumes integer weights `w ∈ Z^n` with `Σ w_i = D` **exactly**.
`D·N_i(x̂)` is rational; independent rounding of the ≤ `d+1` nonzero values is
the one construction that would break the partition of unity — with two
independent floors the sum can land at `D − 1`, and with two independent
round-halves it can land at `D + 1`, either of which corrupts the complete-set
identity. The danger is real and the fix is structural:

> **`WEIGHT-ROUND-01` (derive-last-and-subtract).** For the ≤ `d+1` nonzero
> basis values in ascending knot order, set
> `w_i = ⌊D · num_i / den⌋` for all but the **last** (highest-index) one, and
> set the last to `w_last = D − Σ (earlier w_i)`. Zero everywhere else.

This is the **single named rounding boundary** of the whole design (the
existing redemption division stays exact-or-refuse and the existing collateral
ceiling stays a conservative reservation, exactly as today — neither is a new
rounding). Properties, each a falsifier:

- **Exact partition of unity, always:** `Σ w_i = D` by construction, for every
  degree, every knot grid, every `x̂`. Never two independent roundings.
- **Nonnegativity of the subtracted weight:** each floored `w_i ≤ D·N_i`, so
  `Σ_{earlier} w_i ≤ D·(1 − N_last)`, hence `w_last ≥ D·N_last ≥ 0`. And
  `w_last ≤ D` because the earlier terms are nonnegative. So every weight lies
  in `[0, D]` and `PayoutVector::validate` passes unchanged.
- **Bounded quantization:** each floored weight errs less than `1` low; the
  subtracted weight errs less than `d` high. Payout error per claim unit is
  below `d/D` collateral units — at `D = 2^16`, under one part in ten
  thousand.
- **Deg-1 specialization:** `w_right = ⌊D·u/g⌋`, `w_left = D − w_right`; since
  `u < g`, `w_right ≤ D − 1` and `w_left ≥ 1`; at `u = 0` the vector is
  `D·e_k`, continuous with the previous pane's limit. One multiply, one
  divide, one subtract.

### 2.4 Degree-1 exactness under power-of-two spacing

With uniform spacing `g = 2^s` and `D = 2^m`, `m ≥ s`:

```text
w_right = D·u / g = u · 2^{m−s}     — exact; the division is a left shift
w_left  = D − w_right
```

No rounding occurs anywhere on the path: pane index is a shift, `u` is a mask,
`w_right` is a shift, `w_left` is a subtraction. In the canonical case
`D = g = 2^s` the pane-local coordinate **is** the weight (`w_right = u`), so
the resolved payout of a deg-1 portfolio is the piecewise-linear interpolant
of its coefficients **exactly**, with zero quantization error. Argument:
`u ∈ [0, 2^s)` and `m ≥ s` make `u·2^{m−s} < 2^m = D` an exact integer with no
discarded bits; `w_left = D − w_right ∈ (0, D]`; the sum is `D` by
construction and by exactness simultaneously — the floor in `WEIGHT-ROUND-01`
is the identity on this path. Confidence in this argument is high: it is a
two-line divisibility fact (`2^s | D·u` because `2^s | D`), and the §12
falsifier checks it exhaustively over whole panes rather than sampling.

When `m < s` (denominator coarser than the spacing) the floor is a real
quantization and §2.3's bounds apply; §5.3 and §9 explain why a *coarser* `D`
is often the better product choice anyway, which is why exactness is a named
variant (`B1-EXACT`) rather than a mandate.

### 2.5 Freeze-time arithmetic bounds

All evaluation arithmetic is checked `u128`. Overflow refusals exist on every
multiply, and terms admission makes them unreachable by checking, **once, at
terms freeze**:

```text
d = 1:  D · (g_max − 1)            < 2^127    (g_max = largest knot gap)
d = 2:  D · 2h²  and every |numerator| bound  < 2^127
d = 3:  D · 6h³                    < 2^127
```

With `D ≤ 2^32` these admit gaps up to `2^95` (deg 1) and uniform spacing up
to `2^31`-ish (deg 3) — far beyond any sane market. A terms blob violating its
degree's bound refuses at decode (`R-12`, §5.5), so the runtime overflow
refusal is defense in depth, kept for the same reason the TWAP product check
is kept. **For `d ≥ 2`, uniform power-of-two spacing is mandatory** — it is
what makes the denominators (`2h²`, `6h³`) and the freeze-time bound statement
closed-form; non-uniform `d ≥ 2` (full de Boor with per-gap denominators) is a
named extension gated on its own bounds proof, not admitted here.

### 2.6 Statistic admissibility per degree

The weight map consumes an integer `x̂` (or an integer conservative interval).
`STAT-TERMINAL-01`, `STAT-SAMPLED-MIN-02`, `STAT-SAMPLED-MAX-03` produce
`ValueInterval`s of integers and are admitted for every degree.
`STAT-TWAP-04` produces a ratio `num/den` with `num` up to
`MAX_VALUE · covered_duration ≈ 8.64 × 10^34 ≈ 2^116`; deg-0 comparison
cross-multiplies boundaries and fits, but a deg ≥ 1 weight derivation needs
`⌊D·(num − t_k·den)/(g·den)⌋`, whose intermediate product overflows `u128`
headroom for large `D`. **TWAP with `d ≥ 1` is therefore deferred with exactly
the plan §2.3 discipline**: admit it only with a checked 256-bit path, a
narrowed `MAX_VALUE`, or a frozen `D` small enough to prove the bound — pick
one and write the proof before registering it. `STAT-RELATIVE-TERMINAL-TWAP-05`
stays inadmissible as before.

---

## 3. The central theorem

### 3.1 Setting

A market has `n` active outcomes, frozen common denominator `D ≥ 1`, and a
frozen basis family `B = (d, K, t, edge policy, D)` inducing the weight map
`w : X → Z^n` of §2 over the admitted value domain `X`. Write `T_i` for the
kernel's conservative per-outcome total supply and `C` for Hoard collateral.
Two hypotheses, both established by the §2.3 construction lemma:

```text
(H1) nonnegativity:            0 ≤ w_i(x̂) ≤ D          for every x̂ ∈ X, i < n
(H2) exact partition of unity: Σ_{i<n} w_i(x̂) = D       for every x̂ ∈ X
```

and one definition replacing the Active-phase preset maximum for
distributional markets:

```text
(DEF) required_active(T)          := max_i T_i
      required_resolved(T, x̂)     := ⌈ Σ_i T_i · w_i(x̂) / D ⌉
```

### 3.2 Theorem (partition-of-unity maximum-liability solvency) — PROPOSED, with proof sketch

**Claim (i) — resolution can never breach the invariant.** For every supply
vector `T` and every `x̂ ∈ X`:

```text
required_resolved(T, x̂) ≤ required_active(T).
```

*Proof.* By (H1) every product `T_i·w_i(x̂)` is nonnegative and bounded by
`(max_j T_j)·w_i(x̂)`; summing and applying (H2),
`Σ_i T_i·w_i(x̂) ≤ (max_j T_j)·Σ_i w_i(x̂) = (max_j T_j)·D`. Dividing by `D`
gives a rational at most `max_j T_j`, which is an integer, so the ceiling does
not cross it. ∎

Consequently `resolve` under any admitted `x̂` preserves
`C ≥ required` whenever the Active invariant held — the prospective invariant
check inside the resolve transition is defense in depth, not a live refusal.

**Claim (ii) — complete-set exactness.** For every quantity `q` and every
`x̂`: `Σ_i q·w_i(x̂) = q·D`, exactly, by (H2). So `redeem_complete_set` pays
exactly `q`, remainders never, at every resolved value — the landed §1.5
primitive survives verbatim, and its `RemainderRequired` arm stays unreachable
for validated vectors.

**Claim (iii) — transition preservation.** `split` adds `q` to every `T_i` and
to `C`, raising both sides of `C ≥ max_i T_i` by `q`; `merge` is its exact
inverse and tests `C ≥ q` first; `materialize`/`dematerialize` touch neither
`T` nor `C`; `transfer_internal` is structurally neutral (`&self`); resolved
`redeem` of `q` units of claim `i` pays exactly `q·w_i/D` (exact-or-refuse),
which decreases the resolved liability numerator by exactly `D` times the
payment, so the ceiling-rounded requirement falls by at least the payment and
the invariant is preserved — all four arguments are *identical to today's*,
because none of them ever reads more of the payout vector than (H1) and (H2).

**Claim (iv) — tightness and pricing.** `required_active` is the exact
supremum of `required_resolved` over `x̂` whenever the basis attains a
one-hot vector (deg 0 everywhere; deg 1 at every knot and clamped edge). For
`d ≥ 2` interior basis functions peak below 1 (max `3/4` at `d = 2`, `2/3` at
`d = 3`), so `max_i T_i` is a sound over-reservation; in practice it costs
nothing, because Active-phase kernel supplies are *equal* across outcomes
(only split/merge move them, and they move all outcomes together), and for
equal supplies `T` every simplex vector yields liability exactly `T` — the
Active requirement equals collateral exactly, as today. Simplex pricing is
untouched: by (H2) a complete set is worth exactly one collateral unit at
every resolved value, so basis prices live on the `PRICE_SCALE` simplex and a
price `p_i` reads as the market's expectation of `w_i/D` — the natural
generalization of "probability of cell `i`."

Adversarial review of the thesis, recorded honestly: the one place the
partition of unity could break is weight quantization, and the §2.3
construction closes it *for every degree* — the sum is exact by construction,
never by cancellation of roundings. The residual quantization (< `d/D` per
claim unit) lands entirely inside the *interpolation* accuracy, never inside
solvency, conservation, or the complete-set identity. No other break was
found: no kernel transition, batch stage, or accounting identity reads weight
values beyond (H1)/(H2) (§7 verifies the batch side specifically).

### 3.3 Who checks what

The kernel checks (H1) and (H2) — they are literally `PayoutVector::validate`
against the frozen `D`, unchanged. The kernel does **not** check that the
vector is the right point of the basis for the evidence; that binding lives in
the adapter's derivation (§5), exactly where the binding of "which index"
lives today. The system-level discretion-free claim is preserved — vector =
deterministic function of digest-bound terms and a sealed `WindowResult` — but
the *kernel-alone* claim honestly narrows from "payout ∈ frozen 8-set" to
"payout ∈ frozen simplex lattice `{w ∈ Z^n : (H1), (H2)}`". §10 carries this
to the Rocq/Verus obligations and the filings language.

---

## 4. Kernel API delta

Smallest honest change, three pieces; everything else in the kernel is
untouched.

**1. A resolution-mode field.** `MarketState` gains
`basis_mode: u8` — `0 = FinitePreset` (today's semantics, bit-for-bit),
`1 = DerivedBasis` — frozen at `MarketState::new` and never written again.
Deg-0 markets are `FinitePreset` and keep resolve-by-index; `DerivedBasis` is
what deg ≥ 1 markets freeze.

**2. One new transition.**

```rust
/// Fix the resolved payout to a derived, validated vector (DerivedBasis only).
///
/// The kernel checks shape, not provenance: the vector must carry the
/// market's frozen common denominator and validate over the active outcomes
/// (weights nonnegative, ≤ D, zero beyond the active prefix, sum exactly D).
/// Binding the vector to evidence is the adapter's derivation (§5), exactly
/// as binding an index to evidence is today.
pub fn resolve_with_vector(&mut self, vector: PayoutVector) -> Result<()>
```

Semantics mirror `resolve` verbatim: `validate_shape`, `check_invariants`,
`require_active`, then the vector gate, then the prospective invariant check
(unreachable by Theorem (i); kept as defense in depth), then the writes.
Refusal set:

| refusal | condition |
|---|---|
| `WrongResolutionMode` (new variant) | called on a `FinitePreset` market; symmetrically, `resolve(index)` refuses on a `DerivedBasis` market — one resolution seam per mode, never both |
| `InvalidDenominator` | `vector.denominator != payouts.vectors[0].denominator` (the frozen `D`) |
| `InvalidPayoutWeights` | weight sum ≠ `D`, any weight > `D`, nonzero weight beyond the active prefix |
| `AlreadyResolved` | phase gate, as today |
| `InvariantViolation` | prospective check fails (defense in depth) |

**3. Resolved-vector storage and the Active-phase requirement.**
`MarketState` gains `resolved_vector: PayoutVector` (written only by
`resolve_with_vector`; `PayoutVector::ZERO` while Active). A private
`effective_resolved_vector()` returns the preset by index in mode 0 and
`resolved_vector` in mode 1; `redeem`, `redeem_complete_set`, and the resolved
arm of `required_collateral_for` read only through it — no other line of the
redemption paths changes. The Active arm of `required_collateral_for`
computes, in mode 1, `max_i T_i` directly (the exact simplex supremum, per
Theorem (iv)) instead of the preset maximum; in mode 0 it is byte-identical to
today. Note `max_i T_i` dominates every preset's liability too, so mode 1's
Active requirement is never weaker than mode 0's over the same presets.

`PayoutSet` survives unchanged and stays mandatory (`count ≥ 1`): in
`DerivedBasis` markets the presets are the *named* vectors — at minimum the
frozen failure-refund vector (§5.4) — and continue to anchor the common `D`.
The failure path also flows through `resolve_with_vector` (the adapter passes
the digest-bound preset as the vector), so a derived-mode market has exactly
one resolution entry point.

Not done, and why: enumerating reachable derived vectors as presets is
impossible (`MAX_PAYOUTS = 8` against a lattice with as many members as
admissible `x̂` values); teaching the kernel knots/`u128` values would move
observation semantics into a crate whose charter excludes them; and widening
`resolve(index)` to secretly accept vectors would blur the two modes'
audit stories. Three refused alternatives, all worse than one new transition.

---

## 5. The `derive_payout` extension

### 5.1 Shape

The landed seam stays and gains a sibling with the same discipline (pure,
total, allocation-free, checked, reads no clock/signer/account):

```text
derive_payout        : (ResolutionTerms, WindowResult) -> Result<PayoutIndex,  R>   // FinitePreset (d = 0)
derive_payout_vector : (ResolutionTerms, WindowResult) -> Result<PayoutVector, R>   // DerivedBasis (d ≥ 1)
```

`ResolutionTerms` grows the basis fields it currently pins or lacks
(statistic, ambiguity/edge policies, degree, knots, uniform flag — all read
from the revised `TermsAccount` of §6, so `from_market_terms` stops pinning
the ordinal partition and starts *decoding* the frozen one; every value that
could change the answer stays inside the terms digest, caller-steerable
never). The evidence gate around it (`resolve_from_evidence`) is unchanged
except that in derived mode the requested payout is a requested **value**
`x̂`, checked for equality against the derived one, and the
`ResolutionAccount` records `resolved_value: u128` (§6.3) — redemption
re-derives the vector from immutable terms plus the recorded value, so
weights have no second persisted copy and exactly one semantic owner.

### 5.2 Derivation

Given the conservative interval `[lo, hi]` from the registered statistic:
apply the edge policy to both ends; locate panes; derive `w(lo)` and `w(hi)`
by §2.3; then apply the ambiguity rule below. On success the result is
`w(lo)` (= `w(hi)`).

### 5.3 Ambiguity: the generalized `AMBIG-REFUSE-01`

The deg-0 rule "refuse unless the interval lies in one cell" generalizes to:

> **Refuse unless `w(lo) = w(hi)`, and this must imply `w` is constant on the
> whole interval.**

For **`d = 1`** the implication is a theorem. Define
`φ(x) := Σ_i i · w_i(x)`. Within a pane `k`, `φ = k·D + w_right` with
`w_right = ⌊D·u/g⌋` nondecreasing in `u`; across the pane boundary the left
limit `k·D + (D − ⌈D/g⌉) < (k+1)·D = φ(t_{k+1})`; clamp regions are constant.
So `φ` is nondecreasing on all of `X`, and (pane, `w_right`) is recoverable
from `φ` (the value `(k+1)·D` is attained only at `u = 0` of pane `k+1`, since
`w_right ≤ D − 1` inside a pane). Hence `w(lo) = w(hi)` ⟹ `φ(lo) = φ(hi)` ⟹
`φ` constant on `[lo, hi]` ⟹ `w` constant on `[lo, hi]`. MODEL-grade today
(the §12 falsifier checks it exhaustively on small grids), a named Rocq
obligation after.

For **`d ≥ 2`** the quantized weights are not componentwise monotone (the
smooth center-of-mass **is** strictly increasing by B-spline linear precision,
but individual floored weights can dip and return inside an interval whose
endpoints agree), so endpoint equality does not pin the interior. Rather than
invent a tolerance rule that manufactures precision, **`d ≥ 2` admits
point evidence only in this design: refuse unless `lo = hi`** (after edge
clamping). This is the honest cost of smoothness and one reason deg 2/3 are
second-wave (§13.1 records the open question of a proven interval rule).

**The `D`-granularity trade, stated plainly.** With `d ≥ 1`, `D` is the knob
that trades payout resolution against refusal frequency: weights step once per
`g/D` atoms of value, so an evidence interval of width `ε` refuses with
frequency about `ε·D/g`. The `B1-EXACT` variant (`D = g = 2^s`) maximizes
resolution and therefore refuses any nonzero-width interval that crosses an
atom boundary; a quantized `D` (e.g. `2^12`–`2^16`) tolerates intervals up to
the step width. Terms authors choose `D` against their feed's precision; the
design mandates neither. (Deg-0 markets have the same trade today — a boundary
is just a step of size `D` — the generalization only makes the knob visible.)

### 5.4 Edge and failure

`EDGE-CLAMP-01` (default): values below `t_0` derive `D·e_0`, above `t_{K-1}`
derive `D·e_{n-1}`; the map is total and no new failure enters.
`EDGE-REFUSE-02`: out-of-range `x̂` refuses `R-14`, control passes to the
frozen failure policy. Failure policies are unchanged in structure;
`FAIL-UNIFORM-REFUND-01` in derived mode names its refund vector explicitly as
a preset (`failure_payout_index`, §6), removing the "must already be a member"
side condition by construction — the refund vector need not be uniform when
`n ∤ D`, it need only be a frozen simplex member, which the terms digest now
proves.

### 5.5 Refusal registry extension

`R-01 .. R-11` keep their exact meanings (`R-06 AmbiguousInterval` now
implemented as §5.3). New classes:

| Id | Class | Raised when |
|---|---|---|
| R-12 | `BasisMalformed` | degree ∉ {0..3}; `outcome_count` ≠ the §2.1 count rule; knots not strictly increasing / out of domain; uniform flag contradicts the knot array; freeze-time arithmetic bound (§2.5) violated; `d ≥ 2` without the uniform flag |
| R-13 | `WeightDerivationOverflow` | a checked product in §2.3 overflowed (unreachable for admitted terms; kept, as with `R-11`) |
| R-14 | `ValueOutOfRange` | `EDGE-REFUSE-02` and `x̂ ∉ [t_0, t_{K-1}]` |
| R-15 | `NonPointEvidence` | `d ≥ 2` and `lo ≠ hi` |

`R-12` is a terms-shape refusal and fires at decode, before any evidence is
read, exactly like `R-02`/`R-03` today.

---

## 6. The unified `TermsAccount` revision (obligation 18, discharged once)

Obligation 18 requires a terms revision carrying the statistic id, ambiguity
policy id, coverage parameter, repair generation, source/evaluator versions,
source-adapter identity, and a boundary table with its payout map. The
distributional basis needs a degree, a knot vector, a uniform flag, and an
edge policy — and its deg-0 case **is** the boundary table. Designing these as
two revisions would bump the terms version twice and freeze a boundary-table
shape that the very next revision generalizes. **This design proposes exactly
one revision: `account_version::TERMS = 3`,** whose deg-0 encoding is the
obligation-18 threshold market verbatim and whose `d ≥ 1` encodings are this
document.

### 6.1 New fields (all inside the digest, appended to the v2 body)

| Field | Bytes | Meaning |
|---|---:|---|
| `statistic_id` | 2 | registered statistic (§2.6 admissibility per degree) |
| `ambiguity_policy_id` | 1 | `AMBIG-REFUSE-01` (generalized §5.3) is the only registered value |
| `edge_policy_id` | 1 | `EDGE-CLAMP-01` or `EDGE-REFUSE-02` |
| `basis_degree` | 1 | `d ∈ {0, 1, 2, 3}` |
| `knot_count` | 1 | `K`, per-degree bound of §2.1 |
| `uniform_log2_spacing` | 1 | `s` when all gaps are `2^s`; `0xFF` otherwise; validated against the array |
| `failure_payout_index` | 1 | preset index of the frozen failure-refund vector; `< payout_count` |
| `reserved` | 1 | zero |
| `coverage_policy_parameter` | 8 | bounded-gaps bound; must be zero for `COMPLETE_REQUIRED` |
| `repair_generation` | 8 | pinned generation under `GEN-EXACT-01` (V1 pins 0 today with no field) |
| `source_version` | 4 | replaces the pinned `V1_SOURCE_VERSION` |
| `evaluator_version` | 4 | replaces the pinned `V1_EVALUATOR_VERSION` |
| `source_adapter_id` | 32 | replaces the `feed`-doubles-as-both pinning |
| `payout_map` | 16 | deg-0 cell → preset index, `0xFF`-padded; **must be all-`0xFF` for `d ≥ 1`** (derived mode has no map) |
| `knots` | 256 | `[u128; 16]`, strictly increasing active prefix, zero padding |
| `reserved` | 7 | zero |
| **total new** | **344** | |

`TermsAccount` v3: `1,304 + 344 = 1,648` bytes (digest body `1,268 → 1,612`).
Validation extends the existing discipline verbatim — exact length, known
magic/version, registered ids, strict knot monotonicity, per-degree count
rule, canonical padding, freeze-time §2.5 bounds, byte-for-byte re-encode,
self-certifying digest. The obligation-18 refusal in
`ResolutionTerms::from_market_terms` (only the ordinal partition resolves)
is replaced by decoding these fields; every V1 pin becomes a stored, digested
value.

Deg-0 special case, explicitly: `basis_degree = 0`, `knots[0..K]` = the
evidence-plan boundary table `b_0 .. b_{n-2}` (so `K = n − 1`), explicit
boundaries with the closed top cell exactly as plan §2.2, `payout_map` live,
resolution through `derive_payout` and `resolve(index)` — a threshold market
finally resolves, and nothing about it touches the derived-vector machinery.

### 6.2 What stays out

`D` (already the payout set's common denominator — one owner), the price grid
(separate account, §2.1 decision), and any per-market fee/lot policy (owned by
their own gates). The kernel never sees this account; it sees only the mode
flag and vectors.

### 6.3 `ResolutionAccount` revision

One new digested field `resolved_value: u128` (+16 bytes, version bump), the
sealed `x̂` under derived mode (zero and unused in deg-0 markets, whose
authority stays `payout_index`). `payout_index` in derived mode carries the
sentinel `PAYOUT_INDEX_DERIVED = 0xFE` (distinct from
`PAYOUT_INDEX_UNRESOLVED = 0xFF`). Redemption in derived mode re-derives the
weight vector from (immutable terms, `resolved_value`) — deterministic, ~a
dozen integer ops — and installs it into the kernel `MarketState` before the
transition; the kernel account itself does not persist the vector, so there is
no second copy to drift.

---

## 7. Portfolio and batch implications

### 7.1 Buying a curve costs nothing new

A curve **is** a `PortfolioOrderV1`: coefficients `c_i = g(t_i)` (deg 1 —
nonnegative bounded integers, exactly PROJECT.md's payoff class), and the
relation prices it today: per-lot value `dot(c, p)` in exact price units,
strict/marginal by cross-multiplied comparison, no division (V2), lot
rationing per the frozen variant. No new order family, no relation change, no
new witness field. What changes is only the *interpretation*: at resolution
the position pays `Σ c_i·w_i(x̂)/D` = the piecewise-linear interpolant of `g`
(exact under `B1-EXACT`; within `d/D` per unit otherwise), and by linear
precision the portfolio `c_i = t_i` (scaled) pays the resolved value itself.

### 7.2 The conservation arguments survive — checked, not assumed

Every batch-relation stage operates **before resolution, on claim counts and
cash**, never on payout weights:

- Per-outcome conservation `B_i + μ = E_i + σ` (C-i) is claim-count
  arithmetic; nothing in it references the payout basis. The P1-B closure is
  untouched.
- Virtual split/merge conversion mints/burns complete sets at exactly
  `PRICE_SCALE` price units per set. Its books close because a complete set is
  worth exactly one collateral unit — which is (H2), the partition of unity,
  the *same* property that closed them under one-hot resolution. Post-
  resolution pot unwinding is the one real interaction: `merge` refuses after
  resolution, but the pot's complete sets exit through
  `redeem_complete_set` for **exactly the same amount** (`q` per set, Theorem
  (ii), independent of `x̂`), so the epoch-terminal "pot empty, every atom
  owned" condition closes identically whether resolution lands before or
  after settlement. This must be a falsifier, not prose (§12).
- The pairing gate (H-i-O), eligibility, allocation, consideration ledger,
  and the R-a/R-b/R-c rounding boundary read fills, owners, and prices only.
- `transfer_internal` under T-b is payout-agnostic by construction (`&self`).
- Fees: `G_num` is a seminorm on committed payoff vectors; complete-set
  invariance `G(a + c·1, p) = G(a, p)` still makes conversion fee-free.
  Score component 1's dispersion weighting keeps its arithmetic; its
  *economic* reading shifts slightly (prices are expected weights, not cell
  probabilities), which the score-promotion oracle should note but which
  changes no integer.

One honest wrinkle: T-b settlement after resolution now transfers claims
whose *single-claim* redemption will usually remainder (§9). The receiving
side of a lazy settlement must be told, in docs and client, that its exit is
the complete-set/lot/credit machinery — the same statement P1-A already
requires, now load-bearing for the normal case.

### 7.3 What the venue might add later (not this design)

A client-side "curve builder" lowering `g` to knot coefficients, and a quote
convention for interpolant units. Neither touches the relation.

---

## 8. Cost table (PROPOSED estimates, to be measured by the implementation wave)

Comparison: today's deg-0 (16 outcomes), this design at deg 1 and deg 3, and a
naive "just add bins" design at 1,024 bins targeting comparable payout
resolution.

| Axis | today (deg 0, n=16) | deg 1 (n=16) | deg 3 (n=16) | naive 1,024 bins |
|---|---|---|---|---|
| terms account bytes | 1,304 | **1,648** | 1,648 | ≈ 20,600 (1,023 × 16-byte boundaries + map) |
| kernel market state | 16-wide arrays | + `PayoutVector` + 2 mode bytes (≈ +140 B in the reference kernel account) | same | 1,024-wide arrays ≈ +16 KiB |
| position account | 220 B | 220 B | 220 B | ≈ 16.4 KiB internal alone |
| split / merge / complete-set loop | O(16) | O(16) | O(16) | O(1,024) |
| resolve: derivation arithmetic | ≤ 15 compares | pane scan (or 1 shift) + 1 mul + 1 div (or shift) + 1 sub | + ≈ 12 muls (pane polynomials, Horner) | 1,024-entry search + same |
| resolve: kernel validation | index bound check | O(16) adds (weight sum) | O(16) adds | O(1,024) |
| redeem (one claim) | 1 mul + 1 div | 1 re-derivation (≈ resolve arithmetic) + 1 mul + 1 div | same | 1 mul + 1 div, giant state |
| full materialization (mints/ATAs) | 16 | 16 | 16 | 1,024 |
| payout resolution in `x̂` | 1 of 16 cells | `g/D` atoms per weight step (e.g. `2^-16` of a pane) | same | 1 of 1,024 cells |
| sup-norm error hedging a smooth payoff | O(1/16) | **O(1/16²)** (piecewise-linear vs. step) | O(1/16³)-class | O(1/1,024) |

The last two rows are the argument in one line: binning buys resolution
linearly in state and CU, the spline basis buys it through `D` (a constant)
and through approximation order — a 16-knot deg-1 market beats a 256-bin
market on smooth-payoff hedging error with 6% of its state and none of its
per-transition loop cost. The marginal Solana-side cost of the whole
generalization at deg 1 is a few hundred CU of integer arithmetic and +344
terms bytes; the dominant costs (account decode, terms digest recompute,
token CPIs) are unchanged in kind and within ~26% in the one account that
grows.

---

## 9. The P1-A / lots interaction — fractional payouts become the norm

Deg-0 one-hot resolution makes fractional weights the exception
(`POLICY_ANALYSIS_LOTS_FEES.md` leans (a1): refuse non-one-hot sets in V1).
**Deg ≥ 1 inverts this: at almost every `x̂`, the ≤ `d+1` straddling claims
carry weights strictly between 0 and `D`.** Losing claims still always exit
(weight 0 divides everything — §1.1 fact 1), and complete sets still always
exit exactly (Theorem (ii)), but a single straddling claim redeems only in
multiples of `L_i = D / gcd(w_i, D)` — and `w_i` is *data-dependent at
resolution*, so issuance-time per-outcome lots cannot be computed from the
payout set at all: the reachable weight lattice drives `gcd` to 1 and every
`L_i` to `D` in the worst case. Candidate-by-candidate:

- **(a1) one-hot-only admission — inapplicable by construction.** Choosing
  distributional markets *is* the "morning decision wants fractional live"
  flip condition of §1.6. (a1) remains the recommended posture for deg-0
  markets and is simply not on the menu for `d ≥ 1`.
- **(b) lots — collapses to one uniform lot, and becomes attractive.**
  Since every reachable `L_i` divides `D` and the worst case is `D` itself,
  the only issuance-time-sound lot table is the uniform one: **`L_i = D` for
  every outcome.** Then `q = m·D` makes `q·w_i/D = m·w_i` exact for *every*
  weight vector — every redemption of every claim is exact at every resolved
  value, with zero stored state. Uniformity also dissolves (b2)'s worst
  hazard: one external Token-2022 atom represents `D` internal atoms *for
  every outcome mint alike*, so there is no per-outcome scale table to
  display and no cross-mint scale confusion. At `D = 2^16` on 9-decimal
  collateral the external atom is ≈ 6.6 × 10⁻⁵ tokens — fine-grained; on
  6-decimal collateral it is 0.066 tokens — coarse but usable. The batch
  interface obligation ("fills quantized to `L_i`") becomes one global lot,
  which composes with rounding variant R-a; freezing `D = PRICE_SCALE` (both
  powers of two) would merge the two lot constraints into one, an alignment
  worth evaluating in the same falsifier run.
- **(c) remainder credits — the sub-lot alternative.** Unchanged from §1.4,
  and its two structural advantages (no lot leak into the venue, no blowup
  under any `D`) are exactly the advantages that matter here. Its costs
  (Position layout growth, positionless external redemption lost) are also
  unchanged.

**Recommended posture (PROPOSED, not a decision):** deg-0 markets keep the
(a1) lean untouched; **`d ≥ 1` markets freeze (b2) with the uniform lot
`L = D`** — lot-gated `split`/`merge`/`materialize`, lot-scaled external
mints, fills quantized to `D` at relation admission — with the landed
complete-set redemption as the universal balanced exit, and **(c)** named as
the fallback if sub-lot granularity is judged non-negotiable for internal
balances. Rationale: zero stored state, every-path exactness (which also makes
portfolio curve payouts exact integers, §7.1), a one-number lot story for
UI/terms, and the smallest adversarial surface — (b1)-style external dust
stranding is bounded to sub-`D` fragments a wallet chose to create, and the
retirement-griefing analysis of §1.3 carries over with `L_split = D`. The
EXP-LOT matrix should gain a `deg1-uniform-lot` arm exercising exactly this
(§12).

---

## 10. What does not survive, or gets harder

### 10.1 The product posture in PROJECT.md §6 — changed, explicitly

Current text: "Normal resolution selects one cell of the Market's exhaustive
partition … Fractional payout vectors are reserved for explicitly admitted
ambiguity policies, not the ordinary product model." Under this design that
sentence is false for `d ≥ 1` markets: fractional vectors **are** the ordinary
product model, admitted at market creation by the frozen basis, not by an
ambiguity policy. PROJECT.md §6 needs a two-sentence revision distinguishing
"categorical markets resolve to one cell" from "distributional markets resolve
to a frozen interpolation rule's weights over adjacent cells; both resolve to
one immutable payout vector determined by evidence." This document is the
design authority for that edit but does not make it (coordinator owns
PROJECT.md).

### 10.2 Payout-set enumeration in `MAX_PAYOUTS`

Everything that treats "the possible resolutions" as ≤ 8 enumerable vectors
loses that finiteness in derived mode: the Active-phase preset max (replaced
by `max_i T_i`, §4), any exhaustive walk over `resolve(j)` (economics lab,
kernel traces, Rocq model), and any UI that lists payouts. Presets remain as
*named* vectors (failure refund), so enumeration survives with reinterpreted
meaning: "the named vectors," not "the reachable ones."

### 10.3 The economics lab's one-hot walks

`ECONOMICS_LAB.md`'s `one_hot` admission arm and `enumerate_solvency_traces`
walk resolutions by payout index. Derived mode needs a new arm: walk sampled
`x̂` values (all pane boundaries ± 1 atom, pane midpoints, clamp regions —
exhaustive over small grids), derive weights by §2.3, and assert solvency,
exit-liveness per §9 posture, and the complete-set identity per trace. The
kernel/lab alignment fixtures (§3.4 of the policy analysis) gain a
`basis` fixture family: `(degree, knots, D, x̂) → weights` vectors both
languages must reproduce byte-identically.

### 10.4 Rocq / Verus shadows

`rocq/ClutchKernel.v` models finite indexed `resolve` only. It needs
`resolve_with_vector` plus hypotheses (H1)/(H2) as the vector-admission
predicate, and the §3.2 theorem joins the open-obligation list (its Rocq
statement is *simpler* than the preset max — a sum bound — but it is new
work). The weight-map construction lemma (§2.3) belongs beside it as a
separate obligation over the pane polynomials. The Verus-first shadow policy
(ADR-0003) applies unchanged; none of this is claimed proved, and the
BLOCKER remains the unpinned proof toolchain.

### 10.5 The regulatory story — a sentence-level plan

The filings language currently narrates deg-0: CFTC pre-meeting packet, "an
objective and predetermined observation procedure identifies the realized
cell, its claim redeems for one unit and the other claims redeem for zero.
More generally, only an immutable bounded payout vector admitted at market
creation may be used." The second sentence already gestures at generality but
"admitted at market creation" reads as preset membership. Plan: keep the
first sentence for categorical markets and replace the second with —
"Alternatively, a market's immutable terms may fix a deterministic
interpolation formula over a predetermined value grid; the observation
procedure then determines each claim's redemption fraction, every claim
remains bounded by one collateral unit, and one complete set always redeems
for exactly one unit regardless of the observed value." The same edit pattern
applies to the shareable brief and PROJECT.md §1's "identifies the realized
cell" phrasing. The invariant the filings lean on — full collateralization,
no discretion, bounded payouts — is *strengthened* in the telling by Theorem
(ii), but the prose must stop implying winner-take-all is the only mode
before any `d ≥ 1` market is described to a regulator. Owner: regulatory docs
lane; gate: before any external share of a distributional example.

### 10.6 Ambiguity for `d ≥ 2`

Point-evidence-only (§5.3) is a real product restriction — interval-valued
evidence (gapped coverage, wide observations) cannot resolve a deg-2/3 market
at all. A proven interval rule is an open question (§13.1), and until it
exists, deg 2/3 markets are honestly worse at resolving under imperfect
evidence than deg ≤ 1. This is the biggest capability regression the design
accepts, and the reason deg ≤ 1 is the recommended first wave.

### 10.7 Smaller casualties

`ResolutionAccount`/`TermsAccount`/reference `KernelAccount` version bumps and
every golden vector touching them; the `derive_payout` pinning tests (which
exist to be replaced by field decoding); the static client's terms rendering
(must display knots/degree/`D` and the §9 lot); `expected_window_preimage`
unchanged but its callers' fixtures churn; and the mental model "one Egg wins"
in every README-level document.

---

## 11. Migration and compatibility

Nothing is deployed (Gate L0 open; offline prototype), so "migration" means
schema and fixture churn, not live-market surgery:

- **Terms:** v3 refuses v2 bytes and vice versa per the crate's standing
  version discipline; v2 goldens are retained as refusal fixtures. A deg-0 v3
  market's *digest* necessarily differs from any v2 market's (the body
  grew), so terms identity is not byte-compatible across the bump — but deg-0
  v3 **semantics** are bit-identical: same `derive_payout` outcomes, same
  kernel path (`FinitePreset`, resolve-by-index), pinned by a differential
  falsifier running the ordinal and threshold cases through both the v2-pinned
  and v3-decoded derivations (§12).
- **Kernel:** `basis_mode = 0` markets execute today's code paths with today's
  bytes plus two new fields; every existing kernel test must pass unmodified
  against mode-0 markets, which is itself a falsifier ("deg-0 markets
  byte-compatible" holds at the kernel-state level up to the appended fields,
  and behavior-compatible absolutely).
- **Reference adapter:** the evidence gate order is unchanged; `Resolve` in
  derived mode carries `requested_value` where it carried `requested_payout`;
  the intent codec gains one variant rather than mutating one.
- **No dual-mode market exists:** a market is `FinitePreset` xor
  `DerivedBasis` from creation to retirement; there is no upgrade transition
  between modes, on purpose.

---

## 12. Falsifier list for the implementation wave

Every item is MODEL work over host-only code; names follow the standing
convention that a falsifier which fires becomes a permanent fixture.

```text
# construction lemma (§2.3) — the load-bearing ones
weights_sum_exactly_D_for_every_xhat_every_degree      # exhaustive: d ∈ {0..3}, small K, all gaps ≤ 32, all u, several D
derive_last_subtract_weight_never_negative_never_exceeds_D
independent_rounding_mutant_breaks_partition_of_unity  # mutate to two floors; the PoU falsifier must fire

# deg-1 exactness (§2.4)
deg1_pow2_weights_are_exact_shifts                     # D = g = 2^s: w_right == u, whole panes, several s
deg1_pane_boundary_continuity                          # u→g limit equals next pane u = 0

# central theorem (§3)
resolved_liability_never_exceeds_active_requirement    # exhaustive T, x̂ over small domains
complete_set_redeems_exactly_q_at_every_resolved_value
active_supplies_equal_implies_requirement_exact        # Theorem (iv) practical-tightness
resolve_with_vector_defense_in_depth_check_unreachable

# kernel seam (§4)
resolve_with_vector_refuses_wrong_mode_denominator_sum_padding_phase
resolve_by_index_refuses_on_derived_market
mode0_markets_pass_entire_existing_kernel_suite_unmodified

# derivation and ambiguity (§5)
phi_is_monotone_and_pins_deg1_interval                 # exhaustive small grids: w(lo)=w(hi) iff w constant on [lo,hi]
deg2_interval_endpoint_equality_does_not_pin_interior  # the counterexample justifying point-only; expected to FIRE
edge_clamp_derives_e0_and_elast_exactly
edge_refuse_routes_to_failure_policy
freeze_time_bounds_make_runtime_overflow_unreachable
twap_with_degree_ge_1_refuses                          # §2.6 deferral pinned

# terms revision (§6)
terms_v3_golden_vectors_and_per_field_bitflip_refusals
deg0_v3_derivation_matches_v2_pinned_derivation        # ordinal + threshold differential
uniform_flag_lie_refused_r12
payout_map_must_be_unused_for_degree_ge_1

# batch interaction (§7)
pot_unwinds_identically_before_and_after_resolution    # merge vs redeem_complete_set, same atoms, any x̂
coupled_trace_with_fractional_resolution_closes_conservation
portfolio_payout_equals_interpolant_up_to_D_quantization
greville_portfolio_pays_resolved_value                 # linear-precision check, deg 1: c_i = t_i (scaled)

# P1-A posture (§9)
uniform_lot_D_makes_every_redemption_exact_at_every_xhat
sub_lot_external_fragment_bounded_stranding            # (b2) arm, 3-wallet model
economics_lab_distributional_arm_agrees_with_kernel    # §10.3 fixture family, both languages
```

---

## 13. Open questions

1. **A proven interval-ambiguity rule for `d ≥ 2`** (§5.3). Candidate
   direction: refuse unless endpoints agree *and* the interval fits inside a
   certified-constant region computed from exact pane-polynomial monotonicity
   decomposition (each pane polynomial has ≤ `d − 1` interior extrema at
   algebraic points; a rule that brackets them exactly in integers may
   exist). Until proven, point-only stands.
2. **`D = PRICE_SCALE` alignment** (§9): one lot for venue rounding (R-a) and
   redemption exactness, or two independent constants. Data question for the
   falsifier run.
3. **Where the deg-2/3 clamped end-pane polynomial tables live** — generated
   into terms-validation code at build time versus derived at decode; both
   exact, different audit surfaces.
4. **`MAX_OUTCOMES` beyond 16.** Knot-resolution appetite may eventually
   exceed 16 anchors; raising the bound is a kernel/layout-wide cost decision
   explicitly out of scope here, and nothing in this design forecloses it.
5. **Failure-vector shape for distributional markets** — uniform refund vs. a
   frozen "mark at last good value" vector (derive at a terms-named fallback
   `x̂`). The latter is expressible with zero new machinery (it is just a
   frozen preset computed offline) but is an economics decision, not a
   mechanism one.

---

## 14. Non-claims and promotion boundary

- PROPOSED throughout; implementing it produces MODEL/IMPLEMENTED offline
  falsifiers only. No SVM work, deployment, token, RPC, or public-network
  action is authorized; Gate L0 remains open.
- Theorem 3.2 and the construction lemma are design arguments with proof
  sketches and an exhaustive-oracle falsifier plan. They become theorems only
  when the pinned proof toolchain closes them over the actual definitions;
  until then no statement here may be called verified, and the §10.4
  obligations are named, not discharged.
- Nothing here authenticates evidence: the derived vector inherits exactly the
  trust boundary of the landed evidence gate — honest about a fold, silent
  about a source — and obligations 15–17 of the reference adapter are
  untouched by this design.
- No claim is made that 16 knots suffice for any particular market's hedging
  demand, that any `D` value is canonical, or that the §9 posture is decided.
  The estimates in §8 are estimates; the implementation wave measures.

---

## 15. Implementation addendum 2026-08-18 (terms-revision wave)

Status of this section: IMPLEMENTED (offline, host-tested, SBF-built), against
the PROPOSED design above. Where the implementation had to decide something
the design left open — or where the design disagreed with itself — the
decision is recorded here rather than silently absorbed.

**Landed, as §6 specified with one addition.** `account_version::TERMS = 3`
carries the full §6.1 field list **plus a `collateral_cap: u64`** (the
`RESOLUTION_EVIDENCE_PLAN.md` §3.5 finding rode the same single revision, as
its queue note asked): the appended block is 352 bytes rather than 344, the
account is 1,656 bytes, the digest body 1,620, and the digest domain moved to
`dragons-clutch/terms/v2` under the order-page precedent (preimage shape
changed, domain moves). Cap zero refuses at decode — a terms artifact with no
cap decision cannot exist. Deg-0 v3 semantics are behavior-identical to the
v2-pinned ordinal derivation: the entire pre-existing reference and clutch-sbf
test suites, including the exact byte-level lifecycle vectors, pass unmodified
except for fixture field additions, which is the §12
`deg0_v3_derivation_matches_v2_pinned_derivation` differential in
landed-test form.

**Degree 1 landed to the §5.1 seam shape; degrees 2-3 refuse.**
`derive_payout_vector : (ResolutionTerms, WindowResult) -> Result<PayoutVectorBytes, R>`
is pure, checked, allocation-free: edge policy (`EDGE-CLAMP-01` /
`EDGE-REFUSE-02` — deg-0 markets must freeze CLAMP, whose §2.2 partition it is
the identity on), pane location by shift under the uniform declaration or a
≤ 15-comparison scan otherwise, the §2.3/§2.4 weight construction, the
generalized `AMBIG-REFUSE-01` (`w(lo) = w(hi)`, justified by the §5.3
monotonicity argument), and `validate_active` member-shape validation before
return. TWAP at degree ≥ 1 refuses (`R-05`, the §2.6 deferral); degrees 2 and
3 refuse at terms admission as unimplemented variants (`R-02`) — the honest
consequence of §10.6's missing interval rule. New refusal classes: `R-12`
`BasisMalformed`, `R-13` `WeightDerivationOverflow`, `R-14` `ValueOutOfRange`,
`R-16`/`R-17` below; `R-15 NonPointEvidence` is reserved, unconstructible
until a degree ≥ 2 lands. The §2.5 freeze-time bound is checked at both the
layout codec and the derivation; the §12 falsifiers that apply at deg ≤ 1
(exact-shift exactness over whole panes, partition-of-unity at every `x̂`,
pane-boundary continuity, shift-path = scan-path, ambiguity refusal, edge
clamp/refuse, TWAP deferral, uniform-flag lie, per-field terms refusals) are
landed tests in the layout and reference crates.

**One internal discrepancy in the design, resolved.** §2.3's general
`WEIGHT-ROUND-01` ("floor all but the highest-index, subtract for the last")
and its own deg-1 specialization (`w_right = ⌊D·u/g⌋`, `w_left = D − w_right`)
subtract at opposite ends: for `D = 7, g = 8, u = 1` the general rule gives
`(6, 1)` and the specialization `(7, 0)`. The specialization is what §2.4's
exactness argument, §5.3's `φ`-monotonicity proof, and the §12 falsifier names
(`w_right == u`) all use, so **the deg-1 specialization is the implemented
rule**; a future deg ≥ 2 implementation must restate the general rule
consistently before relying on it.

**The kernel delta did not land — §4 is the one named residue.** The kernel
crate was out of scope this wave, so `basis_mode`, `resolve_with_vector`, and
`resolved_vector` do not exist. The implemented adapter-side representation is
**preset membership**: `derive_payout` on a degree-1 market derives the
validated vector and resolves by the index of the preset equal to it —
whereupon the kernel's own resolve-by-index installs exactly the derived
vector, redemption reads it back by index, and no second copy of any weight
exists. A terms author whose reachable lattice fits `MAX_PAYOUTS` (two
outcomes, `D ≤ 7`: all `D + 1` reachable vectors frozen as presets) gets full
derived resolution end-to-end today, exercised as a landed reference test
including exact fractional redemption (`14·5/7` and `14·2/7`). Every other
derived vector refuses `R-16 DerivedVectorUnrepresentable` — fail-closed,
never approximated — and `derive_payout_vector` on a categorical market
refuses `R-17 WrongResolutionMode` (§4's one-seam-per-mode rule at the
adapter). Consequences deferred with the kernel delta: §6.3's
`ResolutionAccount.resolved_value` (unneeded while resolution is by member
index), the §4 `Active`-phase `max_i T_i` requirement, and the §9 lot posture
decision. The §10.1 PROJECT.md edit remains the coordinator's.

*(Superseded history, 2026-08-19: the kernel delta has since landed.
KernelAccount v2 persists an immutable `basis_mode`, smooth resolution
persists the sole 319-byte v3 native Resolution vector, and public
`derive_payout` is degree-zero-only while `derive_payout_vector` owns degrees
one through three; the preset-membership bridge no longer exists. See
`docs/reviews/NATIVE_SEMANTICS_AUDIT_V4.md` and `CURRENT_TRUTH.md` §3.)*

**Cost, measured rather than §8-estimated.** The on-chain gate reached the v3
terms only after a decode-once rework (`TermsAccount::decode_unchecked` /
`*_into`, digest paid once per transaction in the account plane): with it,
`Resolve` measures 536,123 program units on the pinned runtime — it
previously did not fit the 1,400,000-unit transaction ceiling at all — and
`RedeemInternal` 408,294 (was 1,356,878). The +352 terms bytes cost ~27% per
SHA-256 recomputation, which is exactly why the recomputation count had to
drop to one. Numbers and method: `SBF_BRINGUP.md`'s regenerated resource
envelope.

---

## 16. Implementation addendum 2026-08-18 (kernel-vector wave)

Status of this section: IMPLEMENTED (offline, host-tested), against the
PROPOSED §4 above. It records the kernel half of the §15 residue closing, and
narrows what is left from "the kernel delta" to a strictly smaller layout
statement.

**The §4 kernel delta landed as specified, with the mode as an enum.**
`clutch-kernel` gains `BasisMode { FinitePreset = 0, DerivedBasis = 1 }` —
an enum rather than §4's `basis_mode: u8`, so an out-of-range mode byte is
unrepresentable instead of merely refused — a required argument to
`MarketState::new` with **no `Default`**, `MarketState::resolved_vector`, and
the transition `resolve_with_vector(&mut self, vector: PayoutVector)`. Refusal
classes are §4's table verbatim, with `Error::WrongResolutionMode` **appended**
to the error enum so every previously assigned discriminant (the SBF program's
`0x2000 + n` block) is unmoved. Mode 0 is preserved bit-for-bit: the whole
pre-existing kernel suite passes with no change but the new constructor
argument, and the resolved-index bound check keeps its original site and its
original `InvariantViolation` class.

**Both gates are structural, not merely transitional.** `resolve` refuses
`WrongResolutionMode` on a `DerivedBasis` market and `resolve_with_vector`
refuses it on a `FinitePreset` one — after the phase gate, so an
already-resolved market still reports `AlreadyResolved` first. Beyond the two
entry points, the mode also owns which resolution *slot* may be non-empty in
every reachable state: mode 0 never carries a vector, mode 1 never carries an
index, and a forged state that violates either is refused by every public
operation rather than only by the seam that would have written it.

**Theorem (i) is a landed bounded-exhaustive falsifier, not a proof sketch.**
`mode_one_resolution_never_raises_the_requirement` sweeps 63,108 cases —
`D ∈ {2, 4, 8, 16}` with all 34 admitted two-weight vectors against 441 supply
shapes each, plus `D ∈ {2, 4, 8}` with all 66 admitted three-weight vectors
against 729 supply shapes each — and finds no counterexample to
`required_resolved(T, x̂) ≤ required_active(T)`. It runs each case through the
kernel's own transition with the market funded to *exactly* its Active
requirement, so the claim it establishes is the operational one: the
prospective invariant check inside `resolve_with_vector` is unreachable over
these lattices, which is what "defense in depth, not a live refusal" means. The
count is pinned exactly, so a narrowed loop bound fails rather than silently
sampling. Claim (ii) is checked over the whole reachable `D = 8` lattice, and
the mode-1 Active requirement is checked to equal `max_i T_i` and to dominate
the mode-0 preset maximum over the same set.

**The preset-membership bridge is gone from the derived path.**
`clutch_solana_reference::resolve_derived_market` joins §5.1's
`derive_payout_vector` to §4's `resolve_with_vector`: the derived vector is
installed verbatim and no step consults the preset set, so `R-16` has no site
to arise at there. The `MAX_PAYOUTS` cap on the reachable lattice is lifted
and measured — the nine-member `D = g = 8` lattice resolves entirely against
eight preset slots, and the `(40, 24)` over `D = 64` vector that §15 recorded
as refusing now resolves and redeems exactly. Remainder refusal and the
complete-set exit are unchanged on the new path, because redemption reads the
installed vector through one private accessor that does not know which mode it
is serving.

**What is left is the layout half, and it is now stated exactly.** Two byte
facts block derived-basis resolution through the *account* plane, and neither
is the kernel's:

1. `ResolutionAccount` (frozen in `clutch-solana-layout`) names a payout
   *index* and carries no `resolved_value`. A redemption takes its authority
   from that record and may not re-fold the window, so a market resolved to a
   non-preset vector has no record-bound authority to check against. §6.3's
   `resolved_value` is therefore not "deferred as unneeded" any more — it is
   the blocker.
2. The reference kernel account carries neither a basis-mode byte nor a
   resolved-vector slot, so the market it can reconstruct is a `FinitePreset`
   one. Adding them moves `KERNEL_ACCOUNT_LEN` (1,254 bytes today; +137 for
   both) and with it every pinned byte-level fixture and the SBF account
   sizing, which is a layout revision with its own evidence, not a side effect
   of this wave.

Consequently `derive_payout` keeps its degree-1 preset-membership bridge for
the `Action::Resolve` account path, and `R-16` stays a live, load-bearing
refusal *there* — reclassified from "the missing kernel transition" to "the
index-shaped resolution record". Also unaddressed and unchanged: the §9 lot
posture decision, and the §10.1 `PROJECT.md` edit, which remains the
coordinator's.

*(Superseded history, 2026-08-19: both byte facts above have since been
resolved — the v3 native Resolution record persists the full vector as its
sole owner and KernelAccount v2 carries the immutable `basis_mode`. Public
`derive_payout` is now degree-zero-only, `derive_payout_vector` owns degrees
one through three, and R-16 is an unreachable reserved class. See
`docs/reviews/NATIVE_SEMANTICS_AUDIT_V4.md` and `CURRENT_TRUTH.md` §3.)*

**One cost the design did not price.** `MarketState` grows from 1,240 to
1,376 bytes (`resolved_vector` is 136; the mode byte lands in existing
padding), and the SBF program holds a whole `MarketState` in one call frame —
a frame the split path's own comment already describes as "most of an SBF call
frame on its own". Measured here as a struct size only; whether it moves stack
depth or compute units on the pinned runtime is a resource question for the
on-chain lane and is not measured here.
