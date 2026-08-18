# Optimality-certificate mapping (Cert-F ↔ `BatchRelationV1`)

Status: **PROPOSED throughout.** Nothing in this file is implemented, frozen,
landed, or an evidence claim. No sentence here upgrades any repository claim.
The selected candidate of any epoch remains only the **best valid submitted
candidate**; §4 states precisely what would and would not change under this
mapping, and the answer is that almost nothing changes without work this
document does not authorize.

Claim labels follow the handoff vocabulary: IMPLEMENTED / MODEL / PROPOSED /
BLOCKER.

## 0. What this document is, and the provenance boundary

A sibling repository (`/Users/ember/dev/breadstuffs`) has a landed,
Lean-proven, zero-`sorry` LP optimality-certificate stack ("Cert-F"). This
document asks one question: **could our clearing relation consume it, and what
would we actually be allowed to say if it did?**

Provenance rules that bind this document, per
[`docs/PROVENANCE.md`](../PROVENANCE.md) §2:

* **No code moves.** Every reference below is to *ideas and stated theorems*,
  read read-only. Nothing is copied, translated, vendored, or depended on. A
  provenance manifest would be required before any artifact moved, and a
  sibling lane owns that inventory; this lane proposes no movement.
* The sibling material read for this mapping, for attribution:
  * `metatheory/Market/CertF.lean` — the semantic theorems;
  * `metatheory/Market/CertFDescriptor.lean`, `CertFGolden.lean` — the
    Lean-authored AIR descriptor and its byte-pinned emission;
  * `fhegg-solver/src/{pdhg,cert,clearing,pricecert,uniform_allocation_cert,qp_exact}.rs`;
  * `fhir/src/qp_certificate.rs`;
  * `circuit-prove/src/cert_f_air.rs`.
* House rule, restated because §5 brushes it: **any AIR / constraint system /
  gadget is authored in Lean, never hand-written in Rust.** §5.6 reaches a
  circuit boundary, names it, and stops there.

Our side, for reference: `crates/clutch-batch/src/relation_v1.rs` (IMPLEMENTED
batch verifier), `relation_v1_stream.rs` (IMPLEMENTED streaming verifier),
[`BATCH_RELATION_V1_DESIGN.md`](BATCH_RELATION_V1_DESIGN.md),
[`STREAMING_RELATION_DESIGN.md`](STREAMING_RELATION_DESIGN.md).

### 0.1 What Cert-F actually says

`Market.CertF` is generic over `[CommRing R] [PartialOrder R] [IsOrderedRing R]`
— **not** over `ℚ` or `ℝ`. The only instantiation in the file is `ℤ`. The
program is `structure FlowLP where A : Matrix V E R; w c : E → R; ε : R`,
maximizing `wᵀf`:

```text
PrimalFeasible lp f  :=  A *ᵥ f = 0  ∧  0 ≤ f  ∧  f ≤ c
DualFeasible  lp π s :=  0 ≤ s  ∧  w ≤ π ᵥ* A + s
Certified     lp f π s := PrimalFeasible ∧ DualFeasible ∧ c ⬝ᵥ s − w ⬝ᵥ f ≤ ε
```

with `weak_duality : PrimalFeasible → DualFeasible → wᵀf ≤ cᵀs`,
`gap_nonneg`, and

```text
certifies_epsilon_optimal :
  Certified lp f π s → PrimalFeasible lp f' → w ⬝ᵥ f' ≤ w ⬝ᵥ f + ε
```

Zero `sorry`; `#assert_all_clean` pins the axiom set to
`{propext, Classical.choice, Quot.sound}`.

Two properties of that statement matter enormously for us and are easy to miss:

1. **`R` is an arbitrary ordered commutative ring.** Instantiating at `ℤ` is
   free. Our relation is exact-integer everywhere. There is no impedance
   mismatch at the *theorem* level — only at the *carrier* level (§3c).
2. **`certifies_epsilon_optimal` discards primal feasibility of `f`**
   (`obtain ⟨_, hd, hgap⟩`). The certificate bounds `wᵀf'` for every feasible
   `f'` against the *claimed* `wᵀf`. Primal feasibility of our own `f` is a
   separate obligation — which our relation already discharges, exhaustively,
   at V3/V4. This is a good division of labour, not a gap.

---

## 1. The exact LP

### 1.1 The coordinate that is held fixed

Our relation's candidate has exactly three free economic coordinates
(`relation_v1.rs`, `canonical_candidate`):

```text
(p, c, m)   p : the simplex price vector      (V1)
            c : the net imbalance σ − μ        (V4)
            m : the honored all-or-none mask   (V3, policy 2b only)
```

Everything else — the whole fill vector, σ, μ, every ledger term, the score —
is *derived* from `(p, c, m)` and checked for byte equality. "A fixed price
tick" therefore means a fixed `(p, m)`; `c` is either fixed too (giving an
inhomogeneous system) or left as an LP column (giving Cert-F's exact
homogeneous form). **We take the second option**, which is what makes the
mapping land on `A f = 0` and not on `A f = b`.

Fixed data at that coordinate: `k := domain.outcome_count ∈ 2..=16`,
`S := domain.price_scale = 10_000`, `n := book.len ≤ MAX_ORDERS = 64`.

### 1.2 Decision variables

| symbol | count | meaning | units |
| --- | --- | --- | --- |
| `f_j`, `j ∈ J` | `n ≤ 64` | fill of order `j` | Egg atoms (single-Egg) / lots (portfolio) |
| `σ` | 1 | `candidate.virtual_split` | complete-set units |
| `μ` | 1 | `candidate.virtual_merge` | complete-set units |

Column count `|E| = n + 2 ≤ 66`. `σ` and `μ` are genuine LP columns, not
constants. **The canonicality condition `min(σ, μ) = 0` is deliberately NOT an
LP constraint** — it is a disjunction, not a half-space. It stays exactly where
it already is: V4's `ChurnNotCanonical` refusal, checked outside the LP. §2.4
shows this costs nothing, because the LP optimum has `σ = μ = 0` anyway.

### 1.3 Every row of `A`

One row per outcome `i ∈ 0..k`. Let

```text
α_ij  =  1                    if j is a single-Egg BUY order on outcome i
      =  coefficients_j[i]    if j is a portfolio BUY order
      =  0                    otherwise
β_ij  =  1                    if j is a single-Egg SELL order on outcome i
      =  coefficients_j[i]    if j is a portfolio SELL order
      =  0                    otherwise
```

Row `i` of `A` is then

```text
(C-i)    Σ_j α_ij f_j  −  Σ_j β_ij f_j  +  μ  −  σ   =   0
```

i.e. `A[i][j] = α_ij − β_ij`, `A[i][σ] = −1`, `A[i][μ] = +1`. This is exactly
[`BATCH_RELATION_V1_DESIGN.md`](BATCH_RELATION_V1_DESIGN.md) §7.2's `B_i + μ =
E_i + σ`, and it is precisely `A *ᵥ f = 0` in `PrimalFeasible`. Row count
`|V| = k ≤ 16`.

**The constant-net-imbalance property, as a statement about `A`.** Subtracting
row `i` from row `i'` eliminates `σ` and `μ` entirely, leaving
`(B_i − E_i) − (B_{i'} − E_{i'}) = 0`. So `A` has rank `k` but its kernel
forces `B_i − E_i = σ − μ =: c` to be *the same integer on every outcome*.
That is the complete-set coupling made arithmetic, and it is what closes P1-B:
the executed counterexample (a buy on outcome 0, a sell on outcome 1) needs
`c = 1` and `c = −1` simultaneously, and no `f` in `ker A` does that. In LP
language: **P1-B is not "refused by a check", it is not in the feasible set.**

**The all-ones structure of the σ/μ pair matters** (§3c): the `σ` column is
`−1` in every row and the `μ` column is `+1` in every row, so the two columns
are negatives of each other and any square submatrix containing both is
singular. That is the load-bearing fact in the total-unimodularity argument.

### 1.4 The capacity vector `c`

Per column, at the fixed coordinate `(p, m)` (`relation_v1.rs`,
`derivation_state` + `classify_all`):

```text
c_j  =  0                                if class(j) = Ineligible
     =  0                                if effective_quantity(j) = 0
     =  0                                if j carries a minimum obligation
                                            under AON policy 2b and m_j = 0
     =  effective_quantity(j)            otherwise
c_σ  =  c_μ  =  U   where U := Σ_j effective_quantity(j)   (any valid bound)
```

`effective_quantity(j) = quantity_j − cancelled_j`, where `cancelled_j` is the
N-b self-cross netting. Note that eligibility is decided by an **exact
cross-multiplied integer comparison with no division** (`classify_order`), so
`c` is an exact integer vector, computed from `(p, m)` alone.

**Minimum-fill and all-or-none are NOT bounds.** This is the single most
commonly mis-stated part of the mapping and the prompt's framing overstates
what our code does. Under AON policy 2b (`WitnessedHonoredMask`),
`derivation_state` sets, for an order carrying a minimum obligation:

```text
m_j = 1  →  active[j] ∧ forced[j]   →   f_j is PINNED to effective_quantity(j)
m_j = 0  →  ¬active[j]              →   f_j is PINNED to 0
```

So an obligated order is not a variable with a box `[q_j, q_j] ∪ {0}` (which is
not a box at all). Given the mask it is a **constant**, moved out of the LP
entirely. The same is true of every *portfolio* order under policy P-a
(`StrictWholeOrder`): active portfolios are always `forced`, hence pinned to
full lots. **The LP's free columns are exactly the active, non-forced,
single-Egg orders** — the pro-rata pool members — plus `σ` and `μ`.

This is not a weakening. It is what makes §3c's exactness result available.

### 1.5 The objective `w`

`ScoreV1` is a **lexicographic five-tuple** (`relation_v1.rs`,
`ScoreV1::total_order`), not a scalar. Cert-F certifies one linear `w`. Only
component 1 is a candidate.

**Component 1 (`weighted_direct_volume`) is linear on the feasible set.** Let
`g_i := p_i · (S − p_i)`, the dispersion weight (an exact integer,
`0 ≤ g_i ≤ S²/4 = 25_000_000`). `score_of` computes
`Σ_i g_i · (direct_i − overlap_i)` where
`direct_i := F_i − σ − μ` and `F_i := B_i + μ`. Therefore

```text
direct_i  =  B_i + μ − σ − μ  =  B_i − σ            (identically)
          =  E_i − μ                                 (by C-i)
```

`direct_i` is **linear**, not a `min`. (Under canonical `min(σ,μ) = 0` it
coincides with `min(B_i, E_i)`, which is where the "crossed volume" reading
comes from, but the LP does not need the `min` at all.) Hence, with the overlap
term set aside (see the N-c caveat below):

```text
w_j  =  g_{i(j)}                       single-Egg BUY on outcome i(j)
w_j  =  0                              single-Egg SELL
w_j  =  Σ_i g_i · coefficients_j[i]    portfolio BUY
w_j  =  0                              portfolio SELL
w_σ  =  − Σ_i g_i
w_μ  =  0
```

so `wᵀ(f,σ,μ) = Σ_i g_i (B_i − σ) = Σ_i g_i · direct_i` = component 1 exactly.

*Aside worth carrying:* the symmetric "sell-side" representation
(`w'_j = g_i` on sells, `w'_σ = 0`, `w'_μ = −Σ g_i`) computes the same
functional on the feasible set, and `w − w' = Aᵀπ` for `π_i = g_i`. **The
representation ambiguity of the objective is literally the dual variable.**
That is why the closed-form dual of §3c falls out for free.

**Component 3 (`limit_surplus_price_units`) is also linear.** `verify_inner`
accumulates `|limit_j − p_{i(j)}| · f_j` over filled legs (`ledger.limit_surplus`,
buys `limit − value`, sells `value − limit`), so

```text
w3_j = limit_j − p_{i(j)}   (buy)   ;   p_{i(j)} − limit_j   (sell)   ;   0 on σ, μ
```

This is the classical uniform-price surplus objective. It matters for §3a.

**Components 2, 4, 5, 6 are not linear functionals of `f`, at all:**

| component | form | why it is not an LP objective |
| --- | --- | --- |
| 2 self-overlap `Σ_O Σ_i min(buyfill_i(O), sellfill_i(O))` | concave | subtracted from component 1 ⇒ maximizing a **convex** function over a polytope. Identically zero under N-a/N-b; live only under N-c |
| 4 `distinct_owners` | cardinality of the support, grouped by owner | an indicator/counting functional. Maximizing it is a support-maximization problem, not an LP |
| 5 `churn = σ + μ` | linear, but minimized *after* two maximized keys | lexicographic, not scalarizable without weight-separation bounds we have not established |
| 6 `digest` | a `mix`-permutation of the fills | pseudorandom; no order-theoretic relationship to `f` |

**Consequence, stated plainly: at most component 1 (and, in a different LP,
component 3) is in scope for Cert-F. Certifying `ScoreV1` is not certifying an
LP.**

### 1.6 What is NOT in this LP, and why

| our stage | in the LP? | why not |
| --- | --- | --- |
| V0 admission, id monotonicity, padding | no | representational and structural predicates; no numeric relaxation exists |
| V0 self-cross N-b netting | no | it *rewrites the capacity vector* before the LP exists; `c` is an output of normalization, not a constraint |
| V1 simplex validation | no | it fixes the coordinate the LP is defined at |
| V2 eligibility classification | no | it *produces* `c`; the classification itself is a comparison, not a row |
| **V3 canonical allocation** | **no** | largest-remainder + seeded rank is a *selection among LP-feasible points*, chosen for fairness. §2 shows it is not the argmax of our own score. This is the central finding |
| V3 min-fill / AON obligations | no | disjunctive (`f_j ∈ {0} ∪ [minfill_j, q_j]`); the mask turns them into constants (§1.4), i.e. it selects one branch |
| V4 canonicality `min(σ,μ)=0` | no | a disjunction; kept as a separate refusal |
| **V5 pairing feasibility (H-i-O)** | **could be** | `part_i(O) ≤ F_i` *is* linear in `f`. §5.5 proposes the wide variant. It is not in the narrow LP |
| V6 consideration + the named rounding boundary | no | the one division in the relation; a floor/ceil, not a half-space |
| V7 fee relation | no | policy arithmetic on top of the fills |
| V8 per-asset conservation closure | no | a recomputation identity, already exact |
| V9 digest, score components 2/4/5/6 | no | §1.5 |
| the whole streaming feed protocol | no | `consumed_fold`, pass schedule, refusal ladder — verification mechanics, not economics |

---

## 2. The decomposition claim, tested

**Claim under test.** *Optimality over the frozen grid = per-coordinate LP
optimality (Cert-F) + finite exhaustive comparison across coordinates (what
`propose_best_valid` already does).*

**Verdict: FALSE as stated, for two independent reasons, both with executable
counterexamples. It is TRUE only for score component 1, only under N-a/N-b and
P-a, and there it is already a two-line closed form that needs no certificate.**

The probe is `research/lp-mapping-probe/` (PROPOSED research artifact; not a
shipped crate, not in any workspace, uses only the public `clutch-batch` API).
Run `cargo run --release` in that directory to reproduce every number below.

### 2.1 What the relation actually does per coordinate: it canonicalizes, it does not optimize

`derive_canonical` computes, per outcome, `B_i = min(D_i, S_i + c)` and
`E_i = B_i − c`, then distributes `B_i`/`E_i` over the pool by largest-remainder
pro-rata, then demands **byte equality** with the submitted fills. So the
relation evaluates the score at *exactly one point per coordinate*. It never
compares two fill vectors at the same coordinate. "Per-tick optimality" is
therefore a claim about whether that one canonical point happens to be the
argmax — not about any optimization the relation performs.

### 2.2 Component 1: the closed form IS the LP optimum (E1, confirmed)

`B_i ≤ D_i` (capacity) and `E_i = B_i − c ≤ S_i` give `B_i ≤ min(D_i, S_i + c)`,
attained. Because a single-Egg order touches exactly one outcome, **the
outcomes decouple** and this per-outcome bound is simultaneously achievable.
So `derive_canonical`'s flow is the LP maximum of component 1, by a two-line
argument.

*Experiment E1.* Six-order, two-outcome book; for each of the 11 ticks on a
`price_step = 1000` grid, brute-force every integer fill vector in the box
satisfying the constant-imbalance condition, take the maximum of component 1,
and compare against the relation's own best verified candidate over
`c ∈ −4..=4`.

```
  agree = 11    disagree = 0
```

**The certificate would be checking a fact we can already prove in two lines.**

### 2.3 The tie-breaking does NOT survive (E2 — counterexample)

The prompt asks whether "max volume, then min imbalance, then highest tick"
survives. Two different rules exist in this repo and they answer differently:

**(a) The scalar lab's rule survives, vacuously.** `crates/clutch-batch/src/lib.rs`
implements `TieRule::MaxQuantityMinImbalanceHighTick` in `choose_tick`: maximize
`min(buy_total, sell_total)`, then minimize `|buy − sell|`, then take the highest
tick. All three keys are functions of **the tick alone** — no allocation
freedom enters any of them — so the decomposition holds trivially: per tick the
"LP" has a closed-form optimum, and the cross-tick comparison is a scan over a
totally ordered key.

**(b) `ScoreV1` — the coupled relation's actual rule — does NOT survive.**
Component 4 (`distinct_owners`) is allocation-sensitive, and largest-remainder
pro-rata does not maximize it.

*Experiment E2, the counterexample.* Two outcomes, `S = 10_000`,
policy `(A, N-a, 2a, P-a)`, `p = (5000, 5000)`, `c = 0`:

```
order 1: owner 0, outcome 0, BUY  q=10  limit = 5000   → marginal
order 2: owner 1, outcome 0, BUY  q= 1  limit = 5000   → marginal
order 3: owner 2, outcome 0, BUY  q= 1  limit = 5000   → marginal
order 4: owner 3, outcome 0, SELL q= 3  limit =    0   → strict
```

`D_0 = 12`, `S_0 = 3`, so the buy target is 3 over a pool of quantities
`(10, 1, 1)`. Largest remainder: floors `(2, 0, 0)`, dust 1, remainders
`(6, 3, 3)` ⇒ the `+1` goes to order 1.

```
  canonical fills   = [3, 0, 0, 3]
  canonical score   = c1 = 75_000_000   c3 = 15_000   owners = 2   churn = 0

  rival fills       = [1, 1, 1, 3]
  rival verdict     = Err(CandidateMismatch)
  rival c1 = 75_000_000   rival distinct_owners = 4
```

The rival is LP-feasible (`Af = 0`, `0 ≤ f ≤ c`), **ties component 1 and
component 3 exactly**, and beats the canonical candidate on component 4 by
2 owners — and our relation **refuses it**. Under the frozen `ScoreV1` total
order the rival is strictly better, and the relation cannot express it.

**This is the decisive result of the mapping.** The relation's validity
predicate is *canonical-allocation equality*, which is strictly narrower than
LP feasibility, and the canonical representative is not the score argmax. No
LP certificate repairs this, because component 4 is not an LP objective.

### 2.4 The frozen grid itself is policy-shrunk (E3 — second counterexample)

*Experiment E3.* Two orders, `price_step = 500`, `max_imbalance = 2`:

```
order 1: owner 0, outcome 0, BUY  q=10  limit = 6000
order 2: owner 1, outcome 0, SELL q= 5  limit = 4000
```

```
  A price-priority   best p0 = 6000   c1 = 120_000_000   fills = [5, 5]
  A price-priority   admitted ticks (c=0):  [6000]
  B full pro-rata    best p0 = 5000   c1 = 125_000_000   fills = [5, 5]
  B full pro-rata    admitted ticks (c=0):  every tick 0 … 10000
```

At `p = (5000, 5000)` the strict buy demands 10 and only 5 can clear, so
allocation A raises `StrictUnderfill` and the tick is **not in A's grid at
all** — even though the *same* flow (`fills = [5,5]`, `c1 = 125_000_000`) is
LP-feasible there and B accepts it. A's grid argmax is strictly worse than the
LP-over-grid argmax by 4%.

This is a legitimate mechanism choice (uniform-price price-priority behaviour:
the solver is expected to move `p` until the rationed order becomes marginal).
It is *not* a bug. But it means: **"optimal over the frozen grid" and "optimal
over the LP relaxation restricted to the grid" are different statements, and
under A ours is the weaker one.** Any claim language must not blur them.

### 2.5 Under N-c the primary objective is not even an LP objective

Under `SelfCrossPolicyV1::AllowGateAtPairing` the overlap term
`Σ_O Σ_i min(buyfill_i(O), sellfill_i(O))` is live. It is concave in `f` and it
is *subtracted*, so component 1 becomes `linear − concave` = a **convex**
function being maximized over a polytope. Cert-F does not apply; weak duality
gives no bound. Largest-remainder pro-rata additionally spreads fills onto
self-crossing owners, which raises overlap and *lowers* component 1 — so under
N-c the canonical point is not even a local optimum of the primary key.

**Scope conclusion.** The mapping is coherent only under
`self_cross ∈ {N-a, N-b}` and `portfolio_lots = P-a`. Say so wherever it is
used.

### 2.6 Decomposition verdict, in one paragraph

Per-coordinate optimality holds for score component 1 under N-a/N-b + P-a, and
there it is a closed form that a certificate would restate rather than
establish (E1). It fails for component 4 at every coordinate (E2). The
cross-coordinate half is a bounded exhaustive search whose *grid* is itself
narrowed by allocation policy A (E3). Under N-c the primary objective leaves
the LP class entirely (§2.5). **The decomposition is therefore not a route to a
stronger claim about the accepted candidate; it is a route to a stronger claim
about the accepted candidate's per-outcome flow totals, which is a much smaller
object.**

---

## 3. The three gaps, quantified

### 3a. Integrality — and the reframing it forces

**There is no integrality gap under P-a.** The reason is total unimodularity.

*Claim (PROPOSED; MODEL-level argument, formal shadow not discharged).*
Restrict to policy P-a, so every portfolio order is `forced` and leaves the LP
as a constant. Then every remaining column of `A` is one of:

* a single-Egg column: exactly one nonzero entry, `±1`, in row `i(j)`;
* the `σ` column: `−1` in every row;
* the `μ` column: `+1` in every row.

Take any square submatrix `M`. If it contains both the `σ` and `μ` columns they
are negatives of each other and `det M = 0`. Otherwise at most one all-`±1`
column is present and every other column has exactly one nonzero; expanding
along any single-nonzero column gives `det M = ±1 · det M'` with `M'` of the
same shape, and the base case is a `1×1` entry in `{0, ±1}`. Hence
`det M ∈ {0, ±1}` for every square submatrix: **`A` is totally unimodular.**

Two consequences, both exact:

1. `{x : Ax = 0, 0 ≤ x ≤ c}` with integral `c` is an **integral polytope** — the
   LP optimum is attained at an integer vertex. The continuous relaxation is
   *tight*. There is nothing to round.
2. `[Aᵀ | I]` is TU whenever `A` is, so with integral `w` the dual polyhedron
   `{(π, s) : Aᵀπ + s ≥ w, s ≥ 0}` is integral too: **an integer optimal dual
   exists, and strong duality gives `ε = 0` exactly.** This is §3c's answer.

**So the real gap is not integrality — it is fairness-versus-optimality.** Our
allocator deliberately does *not* take the LP vertex (which is the
price-priority extreme: fill some orders whole, others zero). It takes the
largest-remainder rounding of the pro-rata point, which is an interior-ish
integer point chosen for allocative fairness. The quantities are:

**(i) The rounding deviation of our allocator — exact.** For one pool of `n`
members with quantities `q_j`, total `Q`, target `T`, ideal
`x*_j = q_j T / Q`, remainders `r_j = frac(x*_j)`, dust `D = Σ r_j ∈ ℤ`:

```text
f_j = ⌊x*_j⌋ + [ j ∈ top-D by (remainder desc, seeded rank asc, id asc) ]

‖f − x*‖_∞  <  1                                      (strict)
‖f − x*‖_1  =  2 (D − Σ_{top-D} r_j)  ≤  2 D (n − D) / n  ≤  n / 2
```

Every pool member is a single-Egg order in exactly one pool
([`STREAMING_RELATION_DESIGN.md`](STREAMING_RELATION_DESIGN.md) §3), so summing
over pools, `Σ_pools n_pool ≤ MAX_ORDERS = 64` and the whole-book bound is
**`‖f − x*‖_1 ≤ 32 atoms`, `‖f − x*‖_∞ < 1 atom`.**

*Experiment E4* exhausts pools with `n ∈ 2..=6`, `q_j ∈ 1..=6`, `T ∈ 1..=12`:

```
  worst ‖f − x*‖_1   = 3.0000   at n=6, q=[1,1,1,1,1,1], T=3, fills=[0,0,1,1,0,1]
  worst ‖f − x*‖_inf = 0.8333
```

`2D(n−D)/n = 2·3·3/6 = 3.0` — **the L1 bound is tight, not merely valid.**

**(ii) The objective cost of that rounding.** `|wᵀf − wᵀx*| ≤ ‖w‖_∞ ‖f − x*‖_1`.
For component 1: `w` is constant on each pool (`g_i` on all buys of outcome
`i`), and the allocator conserves the pool total exactly (`Σ_pool f_j = T` by
construction), so **`wᵀf = wᵀx*` — the rounding cost on component 1 is exactly
zero.** For component 3: `‖w3‖_∞ ≤ S = 10_000`, giving
**`≤ 10_000 × 32 = 320_000` price units** across the whole book.

**(iii) The policy cost — the one that is actually large.** Under allocation A
every strict order is filled whole and every marginal order has `w3_j = 0`
(marginal means `limit = p` exactly), so component 3 is a function of `(p, c, m)`
alone and the pro-rata split is surplus-neutral: **policy cost zero under A.**
Under allocation B (`FullProRata`) strict and marginal orders share one pool,
`w3` is not zero on the pool, and pro-rata is not the surplus argmax. Worst
case: `n` equal-quantity pool members of which one carries all the surplus;
pro-rata takes `1/n` of the LP value, so the **relative** gap approaches
`1 − 1/n ≤ 1 − 1/64 = 98.4 %`. This is a mechanism-design fact about B, not an
arithmetic error, and it is unbounded in relative terms.

**(iv) Where a genuine integrality gap would appear.** Only under P-b
(`portfolio_lots` general rationing), where portfolio orders become LP columns
with coefficients `≥ 2`. Then `A` has columns with several entries of magnitude
`> 1`, TU is lost, and fractional optima are real: two outcomes, a portfolio buy
with `coefficients = (2, 1)` and `lots ≤ 1`, single-Egg sells with caps `(1, 3)`
forces `2x ≤ 1`, so the LP optimum is `x = 1/2` while the integer optimum is
`x = 0` — a 100 % gap on that column. **P-b is exactly the policy under which a
certificate would start earning its keep, and exactly the policy under which
`ε = 0` stops being available.** That trade is worth stating to the
mechanism owner.

### 3b. AON / minimum-fill — the exact claim shape

Our relation never *computes* an honored set; it only *checks* a submitted
mask (`AonPolicyV1::WitnessedHonoredMask`, `derivation_state`). In LP terms:

> The obligation `f_j ∈ {0} ∪ [minfill_j, q_j]` is **disjunctive**. The mask `m`
> selects one branch of a `2^κ` branch tree (`κ` = number of obligated orders;
> `propose_best_valid` refuses with `SearchBudgetExceeded` past `κ = 16`, so
> `2^κ ≤ 65_536`). Fixing `m` fixes the branch and yields an LP.

Therefore the strongest available statement has this exact shape, and no
stronger one:

> **Conditional-node optimality.** For the price vector `p`, imbalance `c`, and
> honored mask `m` that the accepted candidate carries, the candidate's flow
> attains `max { wᵀf : A f = 0, 0 ≤ f ≤ c(p, m) }`, where `c(p, m)` pins every
> obligated order to `q_j` or `0` per `m`.

This is a **branch-and-bound node certificate, not a MILP certificate.** A MILP
optimality certificate requires, additionally, a bound certificate for every
*pruned* node — an object `2^κ` times larger in the worst case, which nothing
in this design produces. Any sentence of the form "optimal under all-or-none"
is false; the true sentence always carries "at the witnessed honored mask".

Under AON policy 2a (`RefuseAdmission`) the disjunction does not arise: `κ = 0`,
the branch tree is a point, and the conditional claim becomes unconditional —
at the cost of refusing all-or-none orders at admission. Under 2c
(`FullSizeCounting`) the obligations are enforced on the *derived* vector after
allocation, which is neither a bound nor a branch selector; the mapping does
not cover 2c.

### 3c. ε versus exact integers — the interface, decided

Their carrier: `fhegg-solver` PDHG is `f64` throughout; `cert.rs` stays in
`f64` and derives `s = (w − Aᵀπ)₊` so dual feasibility holds by construction,
checking the rest against a named tolerance (`1e-3·scale`, or `1e-9·scale`
after `restore_feasibility`); `circuit-prove/src/cert_f_air.rs` bridges to
`i64` at fixed-point `scale = 100`, where the gap scales by `scale²` (so the
registered `ε = 2000` is `0.2` in solver units, ≈ 1.1 % of that program's
optimum). Their `qp_exact.rs` shows they know the exact-integer road too:
`i128` at `MAX_SCALE = 18`, with `ε = 0` explicitly reachable ("f64 cannot
honestly make an ε = 0 claim; the integer carrier can").

Our carrier: exact `u64`/`u128`/`i128`, no floats anywhere, one named division
boundary in V6, `ArithmeticOverflow` on every unchecked width.

**Decision: take `ε = 0` with exact integer duals. Do not import a tolerance.**
Under P-a, §3a's TU result guarantees an integral optimal dual exists, so this
is not wishful — and the dual has a closed form.

**The closed-form dual (PROPOSED; MODEL, derived by hand, checked against E1's
brute force).** At the LP optimum `σ = μ = 0` (raising `σ` by 1 raises `B_i` by
at most 1 and lowers `direct_i = B_i − σ` weakly, so churn never helps
component 1 — which is exactly the design's "complete-set churn earns
nothing", now as an LP fact). With outcomes decoupled:

```text
π_i  =  g_i   if  S_i ≤ D_i   (supply binds on outcome i)
     =  0     otherwise        (demand binds)

s_j  =  (w_j − (Aᵀπ)_j)₊      derived, never submitted free:
        buy  j on i  →  (g_i − π_i)₊  =  g_i if demand binds, else 0
        sell j on i  →  (π_i)₊        =  g_i if supply binds, else 0
        σ            →  0             (since π_i ≤ g_i)
        μ            →  0
```

Then `cᵀs = Σ_i g_i · min(D_i, S_i)` and `wᵀf = Σ_i g_i · min(D_i, S_i)`, so

```text
c ⬝ᵥ s − w ⬝ᵥ f  =  0     exactly, in ℤ.
```

Deriving `s` rather than accepting it (the sibling's own construction trick in
`cert.rs::from_solution` and `cert_f_air.rs::bridge_solution_json`) means
`DualFeasible` holds by construction and only the gap can fail — which halves
the checking surface and removes a forgery class.

**Arithmetic bounds either choice needs.**

| quantity | bound at `S = 10_000`, `k ≤ 16`, `n ≤ 64` | width |
| --- | --- | --- |
| `g_i = p_i(S − p_i)` | `≤ S²/4 = 25_000_000 < 2^25` | `u32` suffices; carry `u64` |
| `π_i`, `s_j` | `≤ 25_000_000` | `i64` / `u64` |
| `c_j` | `≤ u64::MAX` (order quantity) | `u64` |
| `cᵀs` | `≤ 66 · 2^64 · 2.5·10^7 ≈ 2.95·10^28` | `u128` (`< 3.4·10^38`, ~10 decimal digits of headroom) |
| `wᵀf` | same order | `i128` |

**No new arithmetic width is required. `u128` already covers it**, and the
existing `checked_*`/`ArithmeticOverflow` discipline already guards it. If we
ever wanted the rational road (only needed under P-b), denominators are bounded
by the largest subdeterminant of `A`, which by Hadamard is
`≤ (max|a_ij| · √k)^k` with `k ≤ 16` — astronomically large for general
portfolio coefficients. **Reject rationals.** If P-b is ever frozen, state ε in
*atoms of the score's price units* and re-derive the bound; do not import a
floating tolerance under any circumstances.

---

## 4. The claim-language delta

This section is the point of the document. The repository's most carefully
guarded phrase is **"best valid submitted candidate"**, and the honest answer is
that this mapping does not replace it.

### 4.1 What stays, verbatim

> The selected candidate of any epoch is only ever the **best valid submitted
> candidate**; no statement here is an optimality claim.

**This sentence stays exactly as it is.** Nothing in §§1–3 earns its removal.

### 4.2 The one sentence that could be *added* — and everything it requires first

If, and only if, all of the following land, one narrow sentence becomes
sayable **alongside** (never instead of) the guarded phrase:

*Preconditions.* (1) frozen policy has `self_cross ∈ {N-a, N-b}`,
`portfolio_lots = P-a`; (2) the closed-form dual of §3c is implemented and the
verifier checks `DualFeasible` per column and `cᵀs − wᵀf ≤ 0` at finalize;
(3) the TU argument of §3a has a discharged formal shadow, or the claim drops
the word "optimal" and says "attains the closed-form flow bound"; (4) a
Lean-side instantiation of the `FlowLP` shape at `ℤ` over *our* `(A, w, c)`
exists — at the Mathlib-versus-core cost named in §6 obligation 7 — and the
Rust checker's correspondence to it is stated as an undischarged residual, not
as a proof.

*The sentence:*

> For the price vector, net imbalance, and witnessed honored mask that the
> accepted candidate carries, its per-outcome crossed volume attains the
> maximum of score component 1 over every integer fill vector satisfying
> per-outcome conservation and the eligibility capacities at that coordinate,
> with gap exactly zero, witnessed by an integer dual pair the verifier checks
> in one pass. Across coordinates, and across every other score component, the
> accepted candidate remains only the best valid submitted candidate.

Note how little that says. It is a statement about **flow totals at one
coordinate**, not about fills, not about the score, not about the grid.

### 4.3 What would still be false to say

Each of these is false under this mapping, and E1–E4 or §§1.5/2.5/3b say why:

1. **"the optimal clearing" / "the optimal candidate"** — false at every scope.
2. **"optimal over the price grid"** — false. The certificate is
   per-coordinate; cross-coordinate selection is an untrusted bounded search
   (`propose_best_valid`, `SearchBoundsV1`), and under allocation A the grid is
   itself policy-shrunk so the LP-over-grid argmax may not be in it (E3).
3. **"the accepted fills are the best fills at that price"** — false, E2. The
   canonical allocation ties component 1 and *loses* component 4 to a
   feasible rival that the relation refuses with `CandidateMismatch`. The
   relation canonicalizes; it does not optimize the allocation.
4. **"score-optimal" / "maximizes the score"** — false. `ScoreV1` is a
   five-component lexicographic order; components 4 and 6 are not linear
   functionals of `f` and component 2 is concave-and-subtracted.
5. **"optimal under all-or-none / minimum fill"** — false. The claim is
   conditional on the witnessed mask: one node of a `2^κ` branch tree, not a
   MILP certificate (§3b).
6. **"optimal" under N-c** — false, and worse: under N-c component 1 is not
   even a concave objective, so weak duality gives no bound at all (§2.5).
7. **"verified" / "proven" for any Rust code path** — false. `CertF.lean`'s
   theorems are about `Market.Certified` over an ordered ring. A Rust checker's
   agreement with that predicate is an undischarged refinement residual — the
   sibling repo says so itself about its own checkers, and we must not claim
   more than they do about theirs.
8. **"we consume the sibling's proof"** — false. No code moves (§0). At most we
   would *restate* the same theorem shape against our own `(A, w, c)`, in our
   own Lean, with attribution.

### 4.4 The blunt summary for the mechanism owner

Where the certificate is cheap (N-a/N-b + P-a + allocation A), it certifies a
fact we can already prove in two lines, because the outcomes decouple and the
flow bound is closed-form. Where it would be genuinely informative (P-b, where
portfolio columns couple the outcomes and the closed form breaks), it is also
where `ε = 0` and total unimodularity are lost. **The value of Cert-F to this
relation is proportional to the outcome-coupling that portfolio orders as LP
columns would introduce — which is precisely the coupling policy P-a exists to
avoid.** That is the trade to put in front of the mechanism-owner gate, and it
is a policy question, not a proof question.

---

## 5. The integration seam (design only)

Nothing in this section is implemented. It is a shape, sized against measured
numbers, so the cost is visible before anyone commits to it.

### 5.1 Where a certificate rides

**The candidate account, not a separate one.** The certificate is
solver-produced, candidate-specific, and must be frozen with the candidate;
a separate account reintroduces exactly the binding problem the design already
solved for fills.

| field | shape | bytes |
| --- | --- | --- |
| `pi[MAX_OUTCOMES]` | `i64`, in the `StreamCandidateV1` header alongside `prices` | 128 |
| `s[j]` | `u64`, **one per order, riding the existing per-order feed next to `fill_j`** | 8 × `order_len` ≤ 512 |
| `s_sigma`, `s_mu` | `u64` in the header | 16 |

**≈ 656 B added to a candidate account whose header is already ≈ 250 B, fills
≤ 512 B, and slices ≤ 7.5 KiB.** The `canonical_candidate_digest` must be
extended to cover `(pi, s)`, or a certificate could be lifted from one
candidate onto another; that is a one-line change to the digest fold and a new
mutation-suite family (`stale certificate`, `swapped dual`).

Note that `s` needs no transport at all if it is **derived** rather than
submitted (§3c): `s_j = (w_j − (Aᵀπ)_j)₊` is computable per order from `π` and
the order's own record. That reduces the certificate to **`π` alone: 128 bytes
in the header**, and removes the forgery surface entirely. This is the
recommended variant, and it is the sibling's own trick.

### 5.2 Who verifies

The streaming verifier, as two additions that ride passes it already makes:

* **`push_order`** — per column, one dual-feasibility comparison
  `w_j ≤ (Aᵀπ)_j + s_j`, where `(Aᵀπ)_j = ±π_{i(j)}` for a single-Egg order
  (one array index, one negation) and `Σ_i π_i · coefficients_j[i]` for a
  portfolio order. Plus two `u128` fused accumulations into running `cᵀs` and
  `wᵀf`.
* **`end_pass` (final finalize)** — one comparison `cᵀs − wᵀf ≤ 0`.

New refusal classes: `DualInfeasible { order }` and `GapExceeded`. **Both must
be assigned positions on the §6 refusal ladder of
[`STREAMING_RELATION_DESIGN.md`](STREAMING_RELATION_DESIGN.md)** — placed after
V3 (they are facts about the same fills) and before V5 — or verdict identity
between the batch and streaming paths breaks. That is a hard obligation, not a
detail; the equivalence gate is the gate.

### 5.3 Frame and account budget, against the measured numbers

Measured today (`STREAMING_RELATION_DESIGN.md` §9, pinned platform-tools
v1.53): `push_order` = **1,280 B** of a **4,096 B** maximum; `end_pass` = 832 B;
`ClearWorkV1` = **48,592 B**.

| addition | cost |
| --- | --- |
| `push_order` locals: two `u128` accumulator temporaries, one `i64` dual, one comparison | ≈ +48 B frame → ≈ 1,328 B, still **32 % of the maximum** |
| `end_pass`: one `u128` subtraction and compare | ≈ +16 B → ≈ 848 B |
| `ClearWorkV1`: `cᵀs` and `wᵀf` accumulators (2 × 16 B) + `π` copy (128 B) + latch slot | **≈ +176 B on 48,592 B = +0.36 %** |

`clear_work_size_is_pinned` would need repinning; that is the intended
tripwire, and it firing is the correct behaviour.

### 5.4 Compute units

The check adds ≈ 4 `u128` operations per order per pass and **no new pass** —
it rides the pass that already reads fills. Against the existing per-order work
(V0 admission, eligibility classification, the 16-outcome leg loop, the V5
participation row, the V6–V8 ledger terms) this is well under 1 %. The
dominant on-chain cost of the certificate is therefore **the ~656 B (or 128 B,
derived-`s` variant) of extra candidate-account data**, not compute. That is
the right shape: cheap to check, small to carry.

For contrast: the cross-*coordinate* half cannot ride on-chain at any budget.
The grid has `C(steps + k − 1, k − 1)` points:

| outcomes `k` | `price_step` 1000 | `price_step` 100 |
| --- | --- | --- |
| 2 | 11 | 101 |
| 3 | 66 | 5,151 |
| 4 | 286 | 176,851 |
| 8 | 19,448 | 26,075,972,546 |
| 16 | 3,268,760 | 2.40 × 10^18 |

times `(2·max_imbalance + 1)` times up to `2^16` masks. **Exhaustive
cross-coordinate optimality is not an on-chain object for `k > 3` at any
useful tick resolution.** The certificate can only ever upgrade the per-coordinate
claim; the cross-coordinate claim stays "best valid submitted candidate", and
that is a structural fact about the search space, not a limitation we could
engineer away.

### 5.5 PROPOSED variant: "Cert-F-wide" (pairing folded into the LP)

The V5 gate `part_i(O) ≤ F_i` **is linear in `f`**, and
[`BATCH_RELATION_V1_DESIGN.md`](BATCH_RELATION_V1_DESIGN.md) §8.2's sufficiency
proof already exhibits the whole object as an **integer-capacity network flow**
— which is exactly the class Cert-F's `FlowLP` name targets, and network
constraint matrices are TU. So conservation *and* pairing feasibility could be
one certified LP with `ε = 0`.

Cost: `k × owner_slots ≤ 16 × 64 = 1,024` extra rows, hence 1,024 extra slack
columns. At `u64` that is **8 KiB of certificate**, comparable to the explicit
slice witness (≈ 7.5 KiB), and the per-order push would touch one row per
`(owner, outcome)` it participates in. Recorded as a variant, not recommended
for a first lane; the narrow LP is where the analysis is settled.

### 5.6 The circuit boundary — named, and stopped at

If this mapping were pushed to a STARK, it would be circuit work, and the
house rule applies: **the AIR would be authored in Lean, never hand-written in
Rust.** For the record, the sibling stack satisfies that rule — its constraints
come from `metatheory/Market/CertFDescriptor.lean`, are serialized by
`emitVmJson2`, are `#guard`ed byte-for-byte against `CertFGolden.lean`, and the
Rust side `include_str!`s the committed JSON with a test that byte-compares it
back to the Lean emission. (Their column-layout *functions* are duplicated by
hand in Rust and pinned only by matching width/count guards; that is a named
residual on their side, and it is exactly the kind of drift our rule exists to
prevent.)

Three facts make this a stop, not a next step:

1. **Their prover is a fixed-program registry.** `try_cert_f_descriptor`
   refuses any `(A, w, c, ε)` that is not one of two registered programs
   (`ring3`: 3 nodes / 3 edges; `market4`: 3 nodes / 4 edges). Our `A` is a
   function of the frozen book and changes every epoch. A per-epoch descriptor
   emission is not an on-chain operation, and making `A` a *witness* rather
   than a public constant turns every currently-affine gate quadratic.
2. **Their field cannot hold our accumulators.** BabyBear `p = 2013265921 ≈ 2^31`
   with `VALUE_BITS = 28`. Our `g_i` alone reaches `25_000_000 < 2^28` — barely
   — and `cᵀs` reaches `≈ 2.95 × 10^28`. A Cert-F AIR over our LP needs
   multi-limb arithmetic, not a single-field 28-bit range gadget.
3. **The proof-toolchain situation is live, and the Mathlib boundary bites.**
   The design's §8.4 and `ROCQ_SPEC_STATUS.md` record Rocq/Lean as unpinned; a
   concurrent lane is landing `lean/` with `leanprover/lean4:v4.33.0` and
   `lakefile.toml` stating "No dependencies, deliberately … the trusted base is
   the Lean kernel plus nothing else." **`CertF.lean` is written entirely
   against Mathlib** — `Matrix`, `Matrix.mulVec`, `Matrix.vecMul`,
   `dotProduct`, `Fintype`, `IsOrderedRing`, and the pointwise `Pi` order.
   Restating it here would therefore either add Mathlib to our trusted base —
   reversing a deliberate decision that is not this lane's to reverse — or
   require re-deriving the finite-dimensional linear-algebra layer over Lean
   core. That is a real, sized obstacle and it belongs in front of whoever owns
   the Lean tree, not inside a circuit lane. See §6, obligation 7.

**Stopping here.** No AIR is designed in this document, and none may be written
in Rust.

---

## 6. Named obligations if this is ever pursued

Ordered by what has to be true first. Every one is PROPOSED.

1. **Policy fork first.** The mechanism owner decides `self_cross` and
   `portfolio_lots`. The mapping is coherent only under N-a/N-b + P-a, and
   §4.4 is the trade to put in front of that gate. Nothing downstream is worth
   starting before this.
2. **`certificate_dual_is_exact_and_gap_is_zero`** — an exhaustive oracle over
   the existing 2,592-book domain asserting that the §3c closed-form dual
   satisfies `DualFeasible` and `cᵀs − wᵀf = 0` at every accepted candidate.
   This is the falsifier that makes §3c more than an argument.
3. **`certificate_does_not_change_any_verdict`** — the certificate must be a
   *redundant* check on every currently-accepted candidate. If it ever refuses
   one, that is a finding about §3c or about `derive_canonical`, never a tune.
4. **`component_four_is_not_maximized_by_canonical_allocation`** — E2, promoted
   from this document's probe into the crate's falsifier suite under its own
   name, so the false claim in §4.3(3) can never be quietly made.
5. **`allocation_a_grid_can_exclude_the_flow_maximal_tick`** — E3, likewise.
6. **Refusal-ladder positions** for `DualInfeasible` and `GapExceeded`, with the
   streaming/batch equivalence gate extended to cover them (§5.2).
7. **Formal shadow of the TU argument** (§3a) — the only part of this document
   that is a mathematical claim rather than a measurement, and the only part
   that needs a proof assistant. **Open question for the Lean-tree owner, not a
   decision for this lane:** the concurrent `lean/` tree pins
   `leanprover/lean4:v4.33.0` with *no dependencies, deliberately*, while
   `CertF.lean` is Mathlib-native (`Matrix`, `mulVec`, `vecMul`, `dotProduct`,
   `Fintype`, `IsOrderedRing`). Restating the certificate theorems against our
   `(A, w, c)` costs either Mathlib in the trusted base or a core-only
   re-derivation of the linear-algebra layer. Neither is free; neither is
   chosen here.
8. **A provenance manifest** before any artifact moves, per
   [`docs/PROVENANCE.md`](../PROVENANCE.md) §2. This document proposes no
   movement and needs none.

## 7. Non-claims

* Nothing here is implemented. `research/lp-mapping-probe/` is a research probe
  outside every shipped crate and every workspace; it changes no program.
* Nothing here is verified. E1–E4 are bounded oracles over small books, not
  theorems. The TU argument in §3a is a hand argument with no discharged formal
  shadow.
* No code, and no proof, moves between repositories. The sibling theorems are
  cited by statement, with attribution, and would have to be *restated* against
  our own `(A, w, c)` in our own Lean before any of this could be claimed.
* This document does not select a policy. Allocation A/B, N-a/N-b/N-c, P-a/P-b
  and AON 2a/2b/2c all remain unfrozen, and §4.4 exists to inform that choice,
  not to pre-empt it.
* **The accepted candidate of any epoch remains only the best valid submitted
  candidate.**
