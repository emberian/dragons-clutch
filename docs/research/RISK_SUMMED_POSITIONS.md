# Risk-summed positions: the sup-norm geometry of collateral, netting, and fees

Status: **RESEARCH note (2026-08-18).** Nothing here is an implementation, a
frozen policy, or an evidence claim. Claim labels: **PROVEN HERE** means a
pen-and-paper proof is given in this document over the stated model — it is
*not* machine-checked, and per the standing promotion boundary nothing may be
called verified until the pinned proof toolchain closes it over the actual
definitions. **MODEL** cites landed offline code or falsifiers. **CONJECTURE**,
**REFUTED**, and **ECONOMICS** mean what they say; economics is stated as
economics, never disguised as mechanism. Negative results are labeled as
results.

Ground truth read: `crates/clutch-kernel/src/lib.rs` (the transitions,
`required_collateral_for`, `resolve_with_vector`, hypotheses (H1)/(H2));
[`../implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md`](../implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md)
(the B-spline basis, §3 solvency theorem, §7 portfolios, §15–16 addenda);
[`../../PROJECT.md`](../../PROJECT.md) (product frame, §9 non-goals);
[`../FEE_GEOMETRY.md`](../FEE_GEOMETRY.md) and
[`../implementation/POLICY_ANALYSIS_LOTS_FEES.md`](../implementation/POLICY_ANALYSIS_LOTS_FEES.md)
(the dispersion fee thesis, hypothesis-grade).

The organizing idea, formalized from the phrase "unitary evolution of
risk-summed positions": a position is a vector over a partition-of-unity
basis; its payoff is a non-expansive linear image of that vector; required
collateral is the vector's sup-norm; complete sets are the diagonal, so risk
lives in the quotient by the diagonal; every pre-resolution transition is an
isometry of that structure; resolution is the one non-isometry — an
evaluation, i.e. a measurement. "Unitary" is an analogy and is retired after
this sentence: there is no inner product here and nothing is literally
unitary. The honest algebraic home is *isometries of a seminormed space with a
conserved quotient class*, and that is the language used below.

---

## 1. The formal structure

### 1.1 Position space and the payoff operator

Fix one market: `n` active outcomes, frozen common denominator `D >= 1`,
frozen basis terms inducing the weight map `w : X -> Z^n` over the admitted
value domain `X`, satisfying the kernel's two vector hypotheses
(`PayoutVector::validate`, design §3.1):

```text
(H1)  0 <= w_i(x) <= D          for every x in X, i < n
(H2)  sum_i w_i(x) = D          for every x in X
```

The ambient **position space** is `V = R^n` with the sup-norm
`||T||_inf = max_i |T_i|`; actual holdings live in the integer cone
`V_+ = Z^n_{>=0}` (the kernel's `Position` internal/external balances and
`total_supply` are `u64` arrays — there are no native shorts; a "short" is a
held complement, §1.4). The **payoff operator** is

```text
Phi : V -> B(X),      Phi(T)(x) = <T, w(x)> / D = sum_i T_i w_i(x) / D
```

into bounded functions on `X` with the sup-norm. Two structural facts restate
(H1)/(H2) exactly:

- **Positivity:** `T >= 0` (componentwise) implies `Phi(T) >= 0`. This is (H1).
- **Unitality:** `Phi(1) = 1`, the constant function. This is (H2): the
  complete set pays one collateral unit at every resolved value.

So (H1)+(H2) say precisely: **`Phi` is a positive unital linear map** — the
adjoint of the Markov kernel `x -> w(x)/D` from `X` to the outcome simplex.
Everything in §1 is a standard consequence of positivity and unitality; the
value of writing it this way is that each protocol invariant becomes a named
one-line corollary rather than a page of per-transition argument.

### 1.2 Non-expansiveness and the collateral norm

**Proposition 1 (non-expansiveness). PROVEN HERE.** For every `T in V`,

```text
min_i T_i <= Phi(T)(x) <= max_i T_i     for every x in X,
```

hence `||Phi(T)||_sup <= ||T||_inf`, and for any two positions
`||Phi(T) - Phi(T')||_sup <= ||T - T'||_inf`.

*Proof.* `Phi(T)(x)` is a convex combination of the components `T_i` with
weights `w_i(x)/D` (nonnegative by (H1), summing to 1 by (H2)); a convex
combination lies between the min and the max. Linearity gives the second
statement. ∎

**Proposition 2 (required collateral is the sup-norm). PROVEN HERE for the
first sentence; the tightness clause is degree-dependent as stated.** In
`BasisMode::DerivedBasis`, the kernel's Active-phase requirement
(`required_collateral_for`, `lib.rs` lines 322–337) is

```text
required_active(T) = max_i T_i = ||T||_inf        (T >= 0)
```

and the resolved requirement is `ceil(Phi(T)(x_hat))` (`required_for_vector`,
lines 367–398). By Proposition 1, `required_resolved <= required_active`
always — this is design Theorem 3.2(i), landed as the 63,108-case falsifier
`mode_one_resolution_never_raises_the_requirement` (§16). Moreover:

- **Degrees 0 and 1 (the only degrees implemented today — deg 2/3 refuse at
  terms admission, §15): the norm is *exact*, not conservative.** Every vertex
  `e_i` of the simplex is attained by the weight map: deg 0 on the interior of
  cell `i`; deg 1 at knot `t_i` (`u = 0` gives `w = D·e_i`), at the closed top
  knot, and at the clamped edges. Hence
  `sup_x Phi(T)(x) = max_i T_i` exactly: **the reserved collateral equals the
  supremum of the actually realizable payoff function.** No slack.
- **Degrees 2–3 (currently refused):** interior basis functions peak below 1
  (`3/4` at deg 2, `2/3` at deg 3), so `||T||_inf` strictly over-reserves for
  unbalanced `T` — by a factor at most `4/3` (deg 2) / `3/2` (deg 3) on a
  single interior claim. At the kernel's *aggregate* level this never bites
  (Lemma 1 below: Active aggregates are diagonal, where every degree is
  tight); it would bite only in a hypothetical position-level margin layer.

*One integer note.* The resolved-phase `ceil` reserves at most one atom above
the exact liability; it never crosses `max_i T_i` because the exact value is
bounded by an integer (design §3.2 proof). All statements here are exact over
integers — `max` of integers is an integer.

### 1.3 The quotient: where risk lives

By unitality, `Phi(T + c·1) = Phi(T) + c`: adding `c` complete sets adds the
constant `c` to the payoff. The diagonal `span(1)` is exactly the risk-free
directions, so the **risk content** of a position is its class in the
quotient

```text
Q = V / span(1),      [T] = T + span(1).
```

The quotient norm induced by the sup-norm is the classical half-range:

**Lemma (quotient norm). PROVEN HERE.**

```text
||[T]||_Q = inf_c ||T - c·1||_inf = (max_i T_i - min_i T_i) / 2 = R(T)/2
```

where `R(T) := max_i T_i - min_i T_i` is the **range (oscillation)
seminorm**. *Proof:* the optimal `c` is the midrange; any other `c` is at
least `R/2` from one extreme. ∎

Both normalizations appear below; `R` is the economically primary one:

- **`R(T)` is the model-free at-risk capital of position `T`:** its
  resolution value is confined to `[min T, max T]` (Prop. 1), an interval of
  length exactly `R(T)` — and of *exactly* that length for deg <= 1, since
  both endpoints are attained. The complete-set component `min(T)·1` is cash
  in escrow (mergeable at par pre-resolution, worth exactly `min T` at every
  resolved value); only the residual `T - min(T)·1`, with range `R(T)`, is at
  risk.
- `||[T]||_Q = R(T)/2` is the same functional in the metric-geometry
  normalization; factors of 2 are tracked explicitly wherever both appear.

**Proposition 3 (the payoff map is a quotient isometry, deg <= 1). PROVEN
HERE.** `Phi` maps `span(1)` to the constants, so it descends to
`Phi_bar : Q -> B(X)/constants`, which is non-expansive for the quotient
norms; for degrees 0 and 1 it is an **isometry**, and moreover `Phi` itself is
injective there.

*Proof.* Non-expansiveness descends (Prop. 1). For deg 1,
`Phi(T)(t_i) = T_i` at each knot (vertex attainment), so
`osc(Phi(T)) >= R(T)`; Prop. 1 gives `<=`; hence oscillation is preserved and
the quotient norms agree. Injectivity: `Phi(T) = 0` forces `T_i = 0` at every
knot. Deg 0: same argument with cell interiors. ∎

So for the implemented modes the position vector *is* the payoff function
(read off at the knots), and every norm statement about coefficients is a
statement about realized payoffs with no translation loss.

### 1.4 The conservation law, and exactly which moves are free

**Lemma 1 (Active aggregates are diagonal). PROVEN HERE (trivial induction);
implicitly MODEL via the kernel suite.** In the Active phase the kernel's
aggregate supply satisfies `total_supply = q·1` where `q` is the number of
complete sets outstanding: only `split` (+`q` to every coordinate) and
`merge` (−`q` from every coordinate) write supply pre-resolution. (External
holder burns make the kernel's book conservative — the true outstanding total
is `<=` the recorded diagonal; PROJECT.md §5.) Post-resolution, single-claim
`redeem` leaves the diagonal; that is the measured regime.

**The conservation law (the "risk-summed" statement). PROVEN HERE, at book
level.** Let `{T^(h)}` be the holder decomposition of the aggregate
(`sum_h T^(h) = q·1`, which is per-outcome conservation — the batch
relation's C-i identity). Then in the quotient:

```text
sum_h [T^(h)] = [q·1] = 0.
```

**Total risk across holders is identically zero at every Active state.**
Every trade (`transfer_internal`, `&self` — structurally supply- and
collateral-neutral) moves class mass between holders and conserves the sum;
`split`/`merge` change `q` and touch no class; `materialize`/`dematerialize`
are the identity on `(T, cash)`. Risk is created and destroyed *pairwise and
oppositely* at trade boundaries, and in no other pre-resolution way. This is
the precise content of "risk-summed positions."

**Proposition 4 (exact characterization of free rebalancing). PROVEN HERE
(deg <= 1).** Call a single-holder move `(T, c) -> (T', c')` (claims, cash)
*counterparty-free and value-preserving* if it is reachable without any other
party's claims changing and satisfies the pointwise value identity
`Phi(T')(x) + c' = Phi(T)(x) + c` for every `x in X`. The complete set of
such moves is exactly:

```text
T' = T + d·1,   c' = c - d,      d in Z,  d >= -min_i T_i,  d <= c  (if d > 0)
```

composed with arbitrary internal/external representation moves
(`materialize`/`dematerialize`). Nothing else.

*Proof.* Sufficiency: `split`(d>0)/`merge`(d<0) implement exactly these, and
unitality gives the value identity. Necessity: the identity forces
`Phi(T' - T)` constant `= c - c'`; by injectivity of `Phi` on coefficients at
knots (Prop. 3), `T' - T = (c - c')·1`. ∎

Consequences, stated as what a venue can offer as *structurally* free
(no new collateral, no risk moved, hence — under any fee base that factors
through the quotient, §3 — no fee):

1. **Diagonal motion with cash in lockstep** — already live: kernel
   `split`/`merge` at par, and the batch relation's virtual complete-set
   conversion at exactly `PRICE_SCALE` per set (design §7.2). The fee side is
   already consistent: `G(a + c·1, p) = G(a, p)`.
2. **Representation moves** — `materialize`/`dematerialize`, already free at
   the kernel (the adapter's CPI costs are transport, not economics).
3. **Intent canonicalization (PROPOSED mechanism note, not designed here):**
   any committed intent vector `a` may be replaced by its min-zero canonical
   form `a - (min_i a_i)·1` plus a cash leg of `min_i a_i` per lot. Same risk
   class, same fee under `G`, strictly smaller gross claim flow and escrow.
   The quotient structure says this is *the* canonical form of a trade; the
   batch-relation lane owns whether to adopt it (open question 4, §5).

And the converse boundary, from Prop. 4: **any change of `[T]` requires a
counterparty.** There is no self-rebalancing that alters risk. A "short" is
the held complement: exposure `-[a]` is carried as
`(max_i a_i)·1 - a in V_+`, at capital `max a` minus sale proceeds — fully
funded by construction, which is why no liquidation machinery exists anywhere
in this geometry (§2.4).

### 1.5 Resolution is the one non-isometry

Resolution at `x_hat` composes every position with the evaluation functional
`delta_{x_hat} ∘ Phi : T -> <T, w(x_hat)>/D` — a rank-one linear functional.
On the quotient it collapses the `(n-1)`-dimensional risk space to a point:
all positions with equal evaluated payoff become economically identical, and
the information distinguishing them is destroyed. It is the unique transition
in the system that is not an isometry of the structure above, and it is
exactly a projection (a measurement, in the analogy retired in the preamble).
Post-resolution, `redeem`/`redeem_complete_set` convert at the measured rate,
exact-or-refuse — isometries again, of the measured (one-dimensional)
structure. The requirement functional degrades monotonically:
`ceil(Phi(T)(x_hat)) <= ||T||_inf` (Prop. 2), and §2's Proposition 8 extends
this monotonicity to multi-market bundles.

Summary of §1 in one table:

| Transition | Action on `(T, c)` | On the class `[T]` | Isometry? | Collateral |
|---|---|---|---|---|
| `split(q)` | `T + q·1, c - q` | identity | yes (value-preserving) | +q, in lockstep with requirement |
| `merge(q)` | `T - q·1, c + q` | identity | yes | −q, in lockstep |
| `materialize`/`dematerialize` | representation only | identity | yes (literally identity) | none |
| `transfer_internal` | moves claims between holders | moves class mass; **sum conserved** | yes (system value) | none (`&self`) |
| `resolve(x_hat)` | none (marks phase) | **collapses `Q` to a scalar** | **no — projection** | requirement falls to `ceil(Phi(T)(x_hat))` |
| `redeem*` | claims → cash at measured rate | identity on measured value | yes | −payout, in lockstep |

---

## 2. Portfolio margining across markets, model-free

The question: a participant holds positions `T^1, ..., T^m` in `m` markets of
one Realm. Their joint exposure is one function on a joint outcome space; its
sup-norm is the model-free capital of the bundle. When is that joint space
known, what is the exact requirement, and what is the honest bound when it is
not known?

Charter note first, honestly: PROJECT.md §9 lists **cross-market collateral
netting as an explicit non-goal**, and every Hoard is market-local. Nothing
below proposes changing that. What follows is (i) the exact mathematics of
what model-free netting *is*, because it determines which product shapes
(single markets on joint statistics) capture the same benefit in-charter, and
(ii) a precise account of what the protocol's refusal costs relative to
SPAN/VaR venues. §2.5 states the three postures.

### 2.1 The admissible joint space

Each market `j` resolves to `x_j in X_j` (or to its failure branch, §2.3.5).
Define the **admissible joint space** `Omega ⊆ X_1 × ... × X_m` as the set of
tuples *not excluded by the deterministic semantics of the frozen terms*.
This is the model-free object: a tuple is excluded only when a theorem about
the terms themselves — shared Feed identity, shared Window, statistic
definitions, frozen conversions — makes it unrealizable on any single
realized data history. No price dynamics, no correlation, no probability
enters. The bundle's payoff and model-free exposure are

```text
F(x) = sum_j Phi_j(T^j)(x_j),        E(T^1..T^m) = sup_{x in Omega} F(x).
```

### 2.2 The two unconditional theorems

**Proposition 5 (subadditivity). PROVEN HERE.** For every `Omega`:

```text
E <= sum_j sup_{x_j} Phi_j(T^j)(x_j) = sum_j max_i T^j_i.
```

*Proof:* sup of a sum is at most the sum of sups. ∎ The right side is the
current protocol's implicit rule: `m` market-local Hoards, each at its own
sup-norm. The protocol is therefore **never under-collateralized relative to
the true joint exposure, for any relationship between the markets** — the
conservative direction is free and unconditional.

**Proposition 6 (products give equality — no model-free netting exists).
PROVEN HERE.** If `Omega` contains a product `A_1 × ... × A_m` where each
`A_j` attains market `j`'s sup (in particular if `Omega` is the full
product), then `E = sum_j max_i T^j_i` **exactly**: the subadditive bound is
not loose — it is the answer.

*Proof:* choose `x_j in A_j` attaining each sup; `F` at that tuple equals the
sum of sups. ∎

When is `Omega` the full product? Whenever no deterministic linkage exists:

- **different references** (BTC vs ETH; nothing excludes any co-movement);
- **the same reference through different Feeds** (two publishers of "the same"
  price are two data series; divergence is not deterministically excluded —
  linkage is a property of the recorded series, never of the platonic price);
- **the same Feed, disjoint or non-nested windows with non-extremal
  statistics** — in particular **terminal value at `T_1` vs terminal value at
  `T_2`: calendar pairs have full-product joint space.** Even "prices are a
  martingale" or "prices are continuous" is a model; model-free, any pair of
  terminal values is admissible.

**Consequence, stated plainly: calendar spreads and cross-asset books get
exactly zero model-free margin relief, and this is not a weakness of the
bound — it is the true worst case.** Any venue giving calendar or cross-asset
relief is asserting a joint distribution. That assertion is precisely what
SPAN's inter-month charge tables and inter-commodity credits are (§2.4), and
precisely what this protocol refuses to hold as consensus state.

### 2.3 The known cases: exact requirements

Netting exists model-free exactly where the terms share objects. In every
case below the requirement is a **finite, exact, integer computation from
frozen terms alone** — no oracle, no sampling.

**2.3.1 Same market (shared everything, shared knot grid).** Positions add in
one space: requirement `= ||T^1 + T^2||_inf <= ||T^1||_inf + ||T^2||_inf`,
strict whenever the argmax coordinates differ. This netting is *native and
already live*: a portfolio inside one market is a single coefficient vector,
margined at its own sup-norm, never leg-by-leg. Degenerate but worth naming:
the venue already cross-margins across all 16 strikes of one market with no
margin rule at all — the data structure is the netting (see §4).

**2.3.2 Same reference, same statistic, same Window; different grids/degrees/
`D`.** `Omega` is the diagonal `{x_1 = ... = x_m}`. Requirement
`= sup_x sum_j Phi_j(T^j)(x)`: a single piecewise-linear function on the
union knot grid; its sup is attained at a union knot or clamp plateau —
at most `sum_j K_j` evaluations, exact in integers.
Release `= sum_j max_i T^j_i - sup_x sum_j V_j(x) >= 0`, strict whenever the
argmax cells differ across markets.

**2.3.3 One reference a frozen deterministic function of another.**
`x_2 = phi(x_1)` with `phi` decodable from terms (a frozen unit conversion; a
statistic that is by definition a function of another statistic of the same
Window). `Omega` is the graph; pull back market 2's payoff and reduce to
2.3.2 on the union of grid 1 with `phi^{-1}`(grid 2) — finite and exact for
piecewise-monotone `phi`. Honest scope note: such pairs are rare. Terminal
and TWAP of the same window are *not* functions of one another; most
same-asset market pairs fall under 2.3.4 or under Prop. 6.

**2.3.4 Same Feed, statistic inequalities — including across nested
windows.** This is the richest genuinely model-free case, and it is *about
the realized data series*, hence theorem-grade:

```text
same window:      min <= TWAP <= max,   min <= terminal <= max,
                  min <= any sampled statistic of the window <= max
nested windows    max over W2 >= max over W1,   min over W2 <= min over W1,
[W1 ⊆ W2, same    max over W2 >= value at any sample point of W2 —
sample grid]:       in particular >= terminal at T1 when T1 is a W2 sample
```

`Omega` is then a polyhedral region `Gamma` (order constraints between
coordinates), and `E = sup_Gamma F` — a separable piecewise-linear objective
over a polyhedron intersected with the product grid: exact, finite (evaluate
on the vertices of the induced subdivision). Worked strictness example: hold
`V_1` increasing in the window-min (pays when the low was high) and `V_2`
decreasing in the terminal (pays when the close was low), both with range
`M`. Unconstrained sum of sups: `2M`. On `Gamma = {min <= terminal}`:
`sup (m + M - t) over m <= t` is `M`. Release: `M` — the entire smaller leg.
"The low was high" and "the close was low" cannot both pay in full, and that
is a fact about one path, not about any model.

**This is the model-free replacement for calendar margining:** not
terminal-vs-terminal (Prop. 6 forbids it) but extremal-vs-terminal and
extremal-vs-extremal across nested windows. A venue that lists running-max /
running-min / drawdown markets alongside terminal markets on the *same Feed
with nested sample grids* creates deterministically nettable structure.
Checkable-at-freeze conditions: same Feed identity (the account, not the
asset name), sample-grid containment, and both markets' coverage/repair
conditions closing — all decidable from terms, none requiring an oracle.

**2.3.5 The failure branches shrink netting (honest cost). PROVEN HERE by
example.** Each market resolves *or* fires its frozen failure policy, so the
true joint space is over `(X_j ∪ {⊥_j})` with `V_j(⊥_j)` = the refund
payoff. Failure of one market but not another is admissible whenever their
evidence paths are separate. Example: two binary markets on the same question
(2.3.2 with shared grid), positions `a = (10, 0)` and `b = (0, 10)`, uniform
refund `(D/2, D/2)`. On the diagonal the bundle is constant `10` — a joint
complete set, nominal release 10. But at `(x_hat, ⊥_2)` it pays up to
`10 + 5 = 15`. Model-free requirement: 15, release only 5. **Exception:**
markets consuming the *same sealed Window* co-fail — `(x, x)` or
`(⊥, ⊥)` only — restoring the full release when the refund vectors also
complement. Netting analysis is therefore inseparable from failure-policy
co-design (open question 3, §5); the failure payout was already flagged as an
adversarial surface in PROJECT.md §6, and this is one more reason.

**Proposition 7 (bounds on the release). PROVEN HERE.** With every
`V_j >= 0` and each marginal of `Omega` full (true in all cases above):

```text
0 <= release := sum_j sup V_j - E <= sum_j sup V_j - max_j sup V_j
```

i.e. at most the sum of all but the largest leg (for two markets: at most the
smaller leg), with the maximum attained exactly when `F` is constant on
`Omega` — a cross-market synthetic complete set. *Proof:* `E >= sup V_j` for
each `j` (evaluate at that market's argmax, others nonnegative). ∎

**Proposition 8 (netted requirements are monotone under partial resolution).
PROVEN HERE.** When market 1 resolves at `x_hat_1`, the bundle requirement
becomes `sup over Omega ∩ {x_1 = x_hat_1}`, a sup over a subset: **the
requirement never rises as components measure.** *Proof:* trivial, and worth
stating because it is false for statistical margining — under SPAN/VaR,
expiry or settlement of one leg routinely *raises* the margin on the residual
book (the hedge leaves before the exposure). Sup-norm netting cannot produce
a post-settlement margin call, structurally. ∎

### 2.4 Comparison with SPAN/VaR clearinghouses and crypto liquidation venues (ECONOMICS)

| Axis | SPAN / VaR clearing | Crypto perp / cross-margin | This geometry |
|---|---|---|---|
| Margin quantity | scenario scan / tail quantile of modeled P&L | maintenance fraction of oracle-marked notional | exact sup of realizable payoff (Prop. 2) |
| Inputs | price ranges, vol ranges, inter-month & inter-commodity correlation tables | mark-price oracle, funding, ADL parameters | **frozen terms only — no price, no vol, no oracle enters the requirement** |
| Under-margin states | reachable (model error); absorbed by calls → liquidations → default fund → mutualization | reachable (gap risk); absorbed by liquidation engine, insurance fund, ADL | **unreachable — every claim prepaid at worst case; the phrase "margin call" has no referent** |
| Procyclicality | margin rises in stress (model re-estimates) | liquidation cascades | requirement is outcome-space geometry; invariant to market state |
| Calendar netting | yes (inter-month charge = a model) | yes (unified margin = a model) | **no for terminal/terminal (Prop. 6 — and that sum is exact, not lazy); partial via nested extremal statistics (2.3.4)** |
| Cross-asset netting | yes (correlation credits) | yes | **no, provably none exists model-free** |
| Leg settlement | can raise residual margin | can trigger liquidation | never raises requirement (Prop. 8) |
| Capital for defined-risk spreads | ≈ max loss (comparable) | varies | max loss exactly, natively netted within a market |
| Capital for naked short premium | small fraction of notional | small fraction | **full max payout minus premium — the honest price of no liquidation engine** |
| Verifiability of the number | clearinghouse computation, parameters semi-public | venue-internal | anyone recomputes exactly from public frozen terms |

What we give up, precisely: **we cannot margin what we cannot relate.**
Leverage in all forms; calendar and cross-asset relief (exactly the cases
where linkage would be a correlation assumption); and capital efficiency for
sellers of low-probability claims, who lock the full worst case against small
premia — the venue structurally taxes tail *writers*, and should expect wider
tail offers than margined venues (this interacts with the fee fork, §3.4).
What we gain: no liquidation engine, no insurance fund, no ADL, no
mutualized default, no margin oracle, no procyclical calls, no
post-settlement margin surprises — each the direct corollary of one
proposition above rather than an operational aspiration.

### 2.5 Postures available in-charter

1. **Joint-statistic markets (in-charter today).** The netting of 2.3.2–2.3.4
   internalizes into a *single* market whose outcome space is the joint
   statistic vector with a product basis over `Gamma`. The binding constraint
   is `MAX_OUTCOMES = 16`: a 2-D grid caps at 4×4 knots — coarse but real.
   This is the only path that makes netting a kernel fact (one Hoard, one
   sup-norm) rather than a display.
2. **Client-side exposure display (no protocol change).** Static Glass can
   compute and show a participant's true model-free bundle exposure `E` and
   release versus sum-of-legs, from public terms — information, not
   collateral. Zero consensus surface.
3. **Realm-level joint Hoards (out of charter; recorded, not recommended).**
   What it would take, for the record: consensus verification of
   `sup_Gamma F` (finite and exact but a new consensus computation), the
   failure-branch joint space of 2.3.5, and per-market resolution ordering
   via Prop. 8. Every piece is well-defined; the charter cost is a new
   cross-market coupling of exactly the kind §9 exists to refuse.

---

## 3. The fee consequence: is dispersion the quotient norm?

Short answer: **no — REFUTED, with the exact relationship computed — and the
fee model is not thereby demoted; it is pinned.** Dispersion is the
implied-measure Gini seminorm on the risk quotient; the quotient norm is its
price envelope. What the quotient structure *derives* is a necessary
condition both candidates satisfy, two characterization theorems that make
`G` unique in its family, and one sharp economic fork that the mathematics
cannot close.

Throughout, prices `p_i >= 0`, `sum p_i = S`, implied measure
`p_hat = p/S`; trade vector `a`; and (FEE_GEOMETRY §2)

```text
Gamma_p(a) := G_num(a, p)/S^2 = sum_{i<j} p_hat_i p_hat_j |a_i - a_j|
            = (1/2) E|a_I - a_J|,     I, J iid ~ p_hat.
```

### 3.1 What the quotient structure derives (the necessary condition)

Diagonal motion is economically free (§1.4: value-preserving,
counterparty-free, collateral in lockstep). A fee that charged it would tax
risk-free legs, contradicting FEE_GEOMETRY §1's own objective. Therefore:

> **Any principled fee base must vanish on `span(1)` — i.e. factor through
> the quotient `Q = V/span(1)` — and, to kill fragmentation arbitrage, be
> subadditive and 1-homogeneous: a seminorm on `Q`.** PROVEN HERE as a
> consequence of §1.4 plus FEE_GEOMETRY's stated objective.

`Gamma_p` satisfies this **for interior prices**, and here is the first
sharp, previously unstated fact:

**Proposition 9 (kernel of the dispersion seminorm). PROVEN HERE.**

```text
ker Gamma_p = { a : a constant on supp(p) } = span(1) ⊕ R^{Z(p)},
```

`Z(p)` the zero-price outcomes. This equals `span(1)` **iff every price is
positive.** *Proof:* `Gamma_p(a) = 0` iff `a_i = a_j` whenever
`p_i p_j > 0`. ∎

So at boundary prices the dispersion fee's kernel is *strictly larger* than
the risk quotient: **risk transfer supported on zero-priced outcomes is
literally feeless**, however large its model-free range. Whether the batch
relation can clear fills at price zero (and if not, what the one-tick floor
bounds this hole at) is a named check for the relation lane — see the
falsifier note at the end of §3.4. The quotient-norm base (below) has no such
hole; this asymmetry is part of the fork.

### 3.2 The refutation, exactly

**Proposition 10 (dispersion vs the quotient norm). PROVEN HERE.** For every
`a` and every `p`:

```text
Gamma_p(a) <= R(a)/4 = ||[a]||_Q / 2,
```

with equality iff `p_hat` puts mass `1/2` on argmax outcomes and `1/2` on
argmin outcomes; and the bound is the exact envelope:

```text
sup_p Gamma_p(a) = R(a)/4,        i.e.   ||[a]||_Q = 2 sup_p Gamma_p(a).
```

For a *fixed interior* `p`, `Gamma_p` is a genuine norm on `Q`, equivalent to
`||.||_Q` with degenerating constants:
`(min_i p_hat_i)^2 R(a) <= Gamma_p(a) <= R(a)/4`, and
`inf over interior p` of `Gamma_p(a)` is `0` for every nonconstant `a`.

*Proof.* Write `X = a_I`. The layer identity
`E|X - Y| = 2 ∫ F(t)(1-F(t)) dt` (from
`|X-Y| = ∫ |1_{X>t} - 1_{Y>t}| dt` and
`E|1_{X>t}-1_{Y>t}| = 2F(t)(1-F(t))` for iid draws) gives
`Gamma_p(a) = ∫ F(1-F) dt` over `[min a, max a]`; pointwise
`F(1-F) <= 1/4` yields the bound; equality forces `F ≡ 1/2` on the open
interval, i.e. half mass at each extreme, which also attains it. The lower
bound for fixed `p` is the single `(argmax, argmin)` pair term. ∎

**Verdict (REFUTED): dispersion is not the quotient norm.** It is bounded by
half of it, touches that envelope only at maximally uncertain two-point
prices, and degenerates to zero as prices concentrate — while the quotient
norm (the model-free at-risk capital, §1.3) stays fixed. The single-Egg case
displays the whole gap: `Gamma = q p_hat (1 - p_hat)` versus
`||[q e_k]||_Q = q/2`, ratio `2 p_hat (1-p_hat) → 0` at extreme prices.

### 3.3 What dispersion *is*: two characterization theorems

The refutation is not a demotion; `G` turns out to be exactly one natural
object, and provably the only one of its kind twice over.

**Proposition 11 (layer decomposition: `G` is the digital-additive extension
of the binary candidate). PROVEN HERE.** Sort the distinct values of `a` as
`v_1 < ... < v_r` and write the layer-cake decomposition into digitals
`d^k := indicator of {i : a_i > v_k}`:

```text
a = v_1·1 + sum_{k<r} (v_{k+1} - v_k) · d^k.
```

Each digital is a binary claim with implied probability
`q_k = P(a_I > v_k)`, and

```text
Gamma_p(a) = sum_{k<r} (v_{k+1} - v_k) · q_k (1 - q_k)
```

— the payoff sliced into digital layers, each layer charged exactly the
binary fee `q p (1-p)` at its own implied probability, and the diagonal layer
charged zero. Moreover `Gamma_p` is the **unique** positively 1-homogeneous
functional that (i) reduces to `q(1-q)` on digitals and (ii) is additive over
layer-cake (nested-digital) decompositions. *Proof:* the formula is the
discrete form of the integral in Prop. 10's proof; uniqueness because (i) and
(ii) force the displayed value on every vector. ∎

**Proposition 12 (uniqueness in the pairwise family). PROVEN HERE.** Within
the family `Phi_phi(a, p) = sum_{i<j} p_hat_i p_hat_j phi(a_i - a_j)` with
`phi >= 0`, relabeling symmetry and positive 1-homogeneity in `a` force
`phi(t) = c|t|`, i.e. `G` up to scale. *Proof:* homogeneity on
`a = (t, 0, ...)` gives `phi(λt) = λ phi(t)`; symmetry gives
`phi(-t) = phi(t)`; hence `phi = phi(1)·|.|`, and `phi(0) = 0`. ∎

So FEE_GEOMETRY's "useful exact generalization" upgrades from construction to
characterization: **accept the binary calibration `q p (1-p)` and layer
additivity, and `G` is derived — uniquely.** What is *not* derivable is the
binary calibration itself, and that is precisely the fork:

### 3.4 The fork, stated as economics (ECONOMICS)

Every axiom FEE_GEOMETRY §3 lists — complete-set invariance, relabeling
symmetry, homogeneity, subadditivity, partition-refinement invariance, cheap
exact integer verification — is satisfied by **both** candidates:

| | `Gamma_p` (dispersion, price-weighted) | `kappa'·R(a)` (quotient/range norm, price-free) |
|---|---|---|
| Factors through `Q` | interior `p` only (Prop. 9 hole at boundary) | always, unconditionally |
| Single-Egg reduction `q p(1-p)` | **yes — this is the one axiom that forces price dependence** | no: charges `kappa' q` at every price |
| What it measures | expected payoff variability under the market's own implied measure (Gini) | model-free at-risk capital — the same functional the solvency machinery locks (§1.3) |
| Fee / consideration, single Egg | `kappa (1 - p_hat)`, bounded by `kappa` at every price | `kappa'/p_hat` — unbounded on cheap claims |
| Fee / worst-case-risk, tails | `→ 0` as `p_hat → 0` or `1`: near-certain flow near-free, tail transfers near-free per unit of range | constant `kappa'` per unit of range at every price |
| Manipulation surface | fee depends on clearing prices (batch-coupled); zero-price kernel hole | none via prices; overcharges consensus-priced flow |

The choice is a real economic question the mathematics cannot answer: **is
the venue charging for risk under the market's own measure (then `G`, and
tails trade nearly free relative to their worst case — consistent with
"insurance should be cheap when the market deems it unlikely," inconsistent
with the fact that the venue's *collateral* machinery prices those same tails
at full worst case), or for model-free risk moved (then `R`, price-free and
manipulation-free, but charging a 99-cent claim's buyer the same absolute fee
as a 50-cent claim's, with fee/consideration exploding on lottery tickets)?**

**Verdict for the fee thesis:** not upgraded to a derivation, and not
refutable either — *pinned*. The principled fee base is necessarily a
quotient seminorm (§3.1; this part **is** now derived, and both candidates
pass); `G` is the unique implied-measure member (Props. 11–12); `R` is the
unique price-free envelope member (Prop. 10). FEE_GEOMETRY's promotion gate
(§7) already demands lab arms; this analysis adds, concretely:

- **arm (6): the quotient-norm base `kappa'·R(a)`** as a control beside
  FEE_GEOMETRY §6's five, with incidence measured by implied probability —
  the two bases differ most exactly where §6 already asks for burden-by-
  probability data;
- **a zero-price laundering falsifier**: whether the relation can clear fills
  at `p_i = 0` (or at the minimum tick), and the fee leaked through Prop. 9's
  kernel enlargement at tick-floor prices, measured;
- **a tail-liquidity interaction note** for the economics lab: `G` undercharges
  tail transfers per unit of locked capital while §2.4 shows tail writers are
  the most capital-taxed participants — the two effects push the same
  direction (thin tails), and the lab should measure them jointly, not
  separately.

One mechanism corollary, fee-base-independent: since every admissible base
vanishes on the diagonal, the §1.4 intent canonicalization (min-zero form) is
fee-neutral under any candidate, so adopting it can never be a fee-avoidance
vector — it is safe to offer as pure escrow relief.

---

## 4. The trader-facing translation

What the geometry above means at the order screen — honest in both
directions. "Can" claims below are deg <= 1 / current-implementation facts;
comparisons are to listed-options venues and bins-based prediction markets.

### 4.1 What you can do here that you cannot do there

- **Buy the density directly.** A deg-1 claim *is* the discrete
  Breeden–Litzenberger butterfly at its knot: `p_i` is a state price, quoted
  and traded as a primitive. On an options venue every butterfly is three
  legs, three spreads, and a margin computation; here it is one asset. A
  16-knot curve order is *one* atomic portfolio intent priced at `dot(c, p)`
  (design §7.1), filled or not as a unit — no leg risk, ever.
- **Native netting within a market.** Margin on a butterfly, condor, ladder,
  or any curve is the sup-norm of its coefficient vector — the max, not the
  sum of legs (§2.3.1). There is no margin *rule* to trust; the data
  structure is the netting.
- **Finite-difference Greeks, exactly, statically.** Holding `c` pays the
  interpolant of `c` at resolution: delta per pane is the forward difference
  `(c_{i+1} - c_i)/(t_{i+1} - t_i)`; gamma is knot-concentrated second
  differences — buying the hat at `t_i` is buying one unit of pure discrete
  gamma at `t_i`, with zero rebalancing and no vol model, exact at resolution
  (B1-EXACT: zero quantization on the power-of-two path). The linear claim
  `c_i = t_i` pays the resolved value itself — a fully-funded, capped
  synthetic forward with no liquidation price.
- **Path digitals as European claims.** Sampled-min/max statistics
  (`STAT-SAMPLED-MIN/MAX`, admitted at every degree) make one-touch /
  no-touch / drawdown-band payoffs ordinary claims — exotic-desk products on
  options venues, unavailable on bins markets tied to a terminal snapshot.
  And nested-window extremal claims are the deterministically nettable
  calendar-adjacent structure (§2.3.4).
- **No liquidation, no assignment, no margin oracle.** A held position cannot
  be forcibly closed by any price path; the worst case is prepaid (§2.4). A
  wick through your strike is a resolution input, not a margin event.
- **Linear manipulation exposure instead of a knife edge.** Deg-1 payout is
  Lipschitz in the resolved value (`D/gap` per claim): nudging the feed near
  your boundary buys the manipulator value proportional to the nudge, versus
  a full unit per straddling holder on any bins venue (design §1). Your curve
  degrades continuously.
- **Cash parking at par.** A complete set is collateral, redeemable at par
  pre-resolution (`merge`) and exactly at par post-resolution
  (`redeem_complete_set`) — carry a "everything except my view" book without
  financing risk.

### 4.2 What is worse here, plainly

- **Grid resolution and frozen strikes.** At most 16 knots, frozen at terms
  creation, forever. No new strikes as the price moves; payoff structure
  between knots is inexpressible; knot placement is a terms-author bet.
- **Bounded payoffs only.** Edge clamping caps every payoff at the grid edge:
  no unbounded calls, no perps, nothing resembling unlimited upside — a
  product restriction inherent to the partition, not a parameter.
- **No leverage, and the full price falls on tail writers.** Buyers and
  defined-risk spreads lock ≈ max loss — comparable to portfolio-margined
  options. Sellers of low-probability claims lock the entire worst case minus
  a small premium; expected return on locked capital is structurally poor,
  and tail offers should be expected wider than on margined venues (§2.4,
  §3.4).
- **No cross-margin where the joint space is unknown** — calendars and
  cross-asset books margin as the *sum*, and Prop. 6 says that sum is the
  honest number, not a lazy one. The relief a clearinghouse would give you is
  a correlation model; this venue does not hold one.
- **Epochs, not a continuous book.** Batch cadence latency; no continuous
  immediacy; liquidity is whatever the epoch gathers.
- **Redemption granularity.** Fractional resolved weights make single-claim
  redemption exact-or-refuse (`RemainderRequired`); the balanced exit is the
  complete set; the proposed uniform lot `L = D` quantizes external claims
  (≈ 0.066 tokens per external atom on 6-decimal collateral — coarse).
  Unbalanced sub-lot fragments can be stuck until recombined (design §9).
- **Resolution can refuse.** Interval evidence crossing a weight step refuses
  into the frozen failure policy: your curve can resolve to the refund
  vector. Options settle to an official settlement price essentially always;
  here refusal-over-discretion is a design choice you are exposed to — and
  §2.3.5 shows it also degrades multi-market hedges.
- **Capital is locked to term.** No early exercise; exit before resolution
  only into venue liquidity or by holding complete sets.

---

## 5. Open questions, ranked by how much they would change the protocol

1. **Joint-statistic product surface (changes the product line and pressures
   `MAX_OUTCOMES`).** §2.3.4's deterministic netting is real, in-charter, and
   currently unreachable in product form: a 2-D `(min, terminal)` or
   `(terminal, running-max)` market needs a tensor grid, and 16 outcomes caps
   it at 4×4. Deciding whether joint-statistic markets are wanted decides
   whether the partition compiler grows a product-basis family and whether
   design open question 4 (`MAX_OUTCOMES` beyond 16) gets promoted from
   "nothing forecloses it" to "something demands it." This is the largest
   lever in this document: it is the only path to calendar-adjacent netting
   that keeps every Hoard market-local.
2. **The fee-base fork — shape decided 2026-08-20; rates open.**
   `Gamma` vs `R` (§3.4) was resolved by taking *both*: the selected V1 base
   **shape** is the additive composite `kappa*Gamma_p(a) + kappa'*R(a)`,
   dispersion with a price-free quotient-norm floor
   ([../decisions/ADOPTED_2026-08-20.md](../decisions/ADOPTED_2026-08-20.md)
   item 9, on
   [../decisions/REPORT_fee-base-selection_2026-08-20.md](../decisions/REPORT_fee-base-selection_2026-08-20.md)).
   The recommended lab work landed and decided it: the quotient-norm arm and
   the zero-price / tick-floor laundering falsifier were added and run, and
   the floor makes the kernel exactly `span(1)` at every admissible price
   vector — closing the Proposition-9 channel that all three
   consideration-proportional bases share. Prop. 11's characterization
   survives re-parameterized, with the binary calibration shifted to
   `kappa*q(1-q) + kappa'`. **Both rates remain undecided**, every byte stays
   `FeeBaseV1::None`, and the selection is reversible until a rate freezes;
   the §7 promotion gate was rewritten in the same act.
3. **Failure-policy co-design for multi-market exposure (changes the failure
   gate's evaluation criteria).** §2.3.5: separate evidence paths admit
   one-sided failure, which caps model-free netting and silently degrades any
   client-side "hedged" display; shared-Window markets co-fail and do not.
   Whether refund vectors should be chosen for cross-market complementarity,
   and whether hedge-intended market pairs should be steered to share
   Windows, belongs inside the already-open failure-policy design gate
   (PROJECT.md §6), not appended after it.
4. **Intent canonicalization and range-based escrow (changes the batch
   relation, moderately).** Prop. 4 says min-zero form is *the* canonical
   representative of a trade; §3.4 says it is fee-neutral under every
   admissible base. Adopting it at the relation reduces gross claim flow and
   escrow reservations at zero economic cost; the relation lane owns whether
   the rounding/rationing variants survive the rewrite.
5. **Client-side model-free exposure display (changes nothing on-chain;
   first user-visible fruit of this note).** Static Glass computing `E`,
   release-vs-sum, and the §2.3.5 failure-branch caveat from public terms:
   the entire §2 computation is finite, exact, and oracle-free, so it can
   ship as pure display with zero consensus surface — and it is the honest
   version of what margined venues call a "portfolio margin estimate."

Not listed as open: deg ≥ 2 capital tightness (Prop. 2's gap) — the kernel's
diagonal aggregates make it moot at market level, deg ≥ 2 currently refuses
at terms admission, and the design's own open question 1 (the interval
ambiguity rule) gates any of it becoming live.

## 6. Non-claims

- No proof here is machine-checked; "PROVEN HERE" is pen-and-paper over the
  stated real/integer model, and the standing BLOCKER (unpinned proof
  toolchain) applies to every candidate promotion into Rocq/Verus.
- Nothing here proposes cross-market collateral netting; §2.5's third posture
  is recorded to make the refusal informed, not to soften it.
- Nothing here promotes a fee base, `kappa`, or an allocation; §3 narrows the
  candidate set and sharpens the experiments, and the FEE_GEOMETRY §7 gates
  are untouched.
- The trader translation (§4) describes mechanism-level capabilities of the
  offline prototype's semantics; no venue, market, or deployment exists, and
  no liquidity, pricing, or demand claim is made anywhere in this document.
