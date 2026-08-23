# The dual is the measure — a mathematical investigation of the clearing LP

Status: **RESEARCH document** (2026-08-18; §7.6 added 2026-08-21). This is a
thinking deliverable: a mathematical investigation of a conjecture, not a
design, not an implementation record, and not an evidence claim about any
landed code path. Nothing here is machine-checked over this repository's
definitions.

*Amendment 2026-08-21.* The original document carried the line "No code changes
accompany this document." That is no longer true of §7.6: the moment-cone
admission condition derived there **is** implemented, as the relation's V1b
stage (`crates/clutch-batch/src/relation_v1.rs::validate_price_moment_cone`,
mirrored in the streaming twin), and stated over the model basis in
`lean/DragonsClutch/MomentCone.lean`. Every other section remains
implementation-free. The plane labels below are unchanged: §7.6's theorems are
[PROVED HERE] (paper, over the repo's exact integer objects), the Lean file is
model-plane with decide-checked witnesses, and neither makes the Rust stage
"verified" in any proof-assistant sense.

**The conjecture under attack.** In a batch-clearing LP over a
partition-of-unity basis, the dual variables on the per-outcome conservation
constraints *are* the claim price vector — which, under partition of unity, is
a normalized positive quadrature rule for a risk-neutral measure. If true, an
optimality certificate for the clearing and a published implied measure are the
same object: proving the batch optimal and publishing the market's density are
one act.

**Verdict, up front.** The conjecture splits into two independent halves, and
they have different truth values with a sharp boundary:

1. **"The witness price vector is a measure"** — TRUE and unconditional for
   basis degrees 0 and 1, by a moment-body theorem (§7): *every* vector passing
   the V1 simplex gate is the basis-moment vector of an explicit probability
   measure. **REFUTED for degree ≥ 2**: there are V1-valid price vectors that
   are no measure's moments, with an explicit executable arbitrage against
   them (§7.4).
2. **"The witness price vector is the optimality certificate's dual"** — TRUE
   as a theorem for the accept set of the relation under the policy tuple
   (allocation A, AON 2a or a fully-honored 2b mask, P-a, N-a/N-b): every
   accepted candidate carries, inside its own witness, a **zero-duality-gap
   certificate** of surplus optimality for the LP relaxation, with `π = p`
   (§5). Under allocation B or an unhonored 2b mask the certificate degrades
   to an ε-certificate with an **exact, itemized, per-order integer gap
   formula** (§5.4, §8). And there exist books (one-sided outcomes) where the
   only LP-optimal duals have **negative components**, no admissible witness
   exists, and allocation A lapses — the lapse of open question §16.1 of the
   batch design is precisely "no optimal dual lies in the admissible price
   box" (§6).

So the one-act thesis holds in this exact form: **when the venue clears under
the certificate-demanding policy tuple, the accepted candidate is
simultaneously (i) a clearing, (ii) a zero-gap LP-optimality certificate with
the price vector as its dual, and (iii) — at degree ≤ 1 — a positive
normalized quadrature rule for a risk-neutral measure, published.** Where any
hypothesis fails, this document says exactly which object survives.

Claim labels used below (mapped to the handoff vocabulary: everything here is
MODEL-or-lower; no claim is "verified"):

- **[PROVED HERE]** — a paper proof in this document, over the exact integer
  objects of the repo.
- **[MACHINE-CHECKED ELSEWHERE]** — proved in Lean in
  `/Users/ember/dev/breadstuffs/metatheory/Market/CertF.lean` for a *general*
  matrix; the instantiation at our matrix is paper-level here, not done in
  Lean.
- **[STANDARD]** — a classical theorem, cited, applied to our matrix.
- **[CONJECTURE]** — stated precisely, with evidence, unproved.
- **[REFUTED]** — false, with a counterexample in this document.
- **[OPEN]** — neither proved nor refuted here.

Ground truth read: `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md`
(basis, weight map, solvency theorem, §7 portfolios), 
`docs/implementation/BATCH_RELATION_V1_DESIGN.md` (the coupled relation),
`crates/clutch-batch/src/relation_v1.rs` (the implemented verifier), and
read-only from the breadstuffs repo: `metatheory/Market/CertF.lean`,
`fhegg-solver/src/pdhg.rs`, `clearing.rs`, `fisher.rs`.

Notation: `n` = active outcome count, `S` = `PRICE_SCALE` (implemented value
`10_000`; a domain parameter), `D` = the kernel payout denominator, `Δ_S` =
the scaled integer simplex `{p ∈ Z^n : 0 ≤ p_i ≤ S, Σ p_i = S}` that V1
enforces.

---

## 1. What the implemented relation is, and what LP question we may ask of it

The implemented relation (`relation_v1.rs`) is **not an LP solver**. It is a
verifier: a candidate names free coordinates `(p, c, mask)` — the simplex
price vector, the net complete-set imbalance `c = σ − μ`, and (under 2b) the
honored-AON mask — and everything else (fills, flows, cash) is *derived* and
checked for exact equality. Selection among valid candidates is by the frozen
lexicographic score, and the repo's standing language is deliberately
"best valid submitted candidate," never "optimal."

The mathematical move of this document: write down the linear program that the
relation's stages V0–V8 are the shadow of, take its dual, and ask what the
accept set of the verifier has to do with the primal-dual optimal pairs of
that LP. The punchline (§5) is that the correspondence is much tighter than
the "no optimality claim" posture suggests — under one specific policy tuple,
the accept set sits *inside* the LP's optimal face, certificate included.

A scoping note on faithfulness. The LP of §2 models exactly the stages V1
(simplex), V2 (eligibility), V3's box `0 ≤ f ≤ q`, V4 (conservation), and V6/V8
(cash closure). It deliberately does **not** contain: the V5 pairing gate
(Hall rows — an LP *extension*, §5.6), V7 fees (excluded from the relation's
normalization by design; §9.4), the AON/minimum-fill semicontinuity (the
integer/disjunctive side; §8.1), the canonical-allocation *selection* inside
the marginal set (a tie-break on the optimal face; §5.3), and the score (a
selection rule *among* optima; §9.3). Each exclusion is analyzed, none is
waved away.

---

## 2. The clearing LP, exactly

### 2.1 Data

Fix a frozen domain and a normalized book (post-V0: admission, expiry, and —
under N-b — netting; all quantities below are the *effective* post-netting
quantities). For each admitted order `o`:

- a **side** `ς_o ∈ {buy, sell}`;
- a **coefficient vector** `a_o ∈ Z_{≥0}^n`: for a single-Egg order on outcome
  `i`, `a_o = e_i`; for a portfolio order, its coefficient array (Egg atoms
  per lot). Not all zero;
- a **capacity** `q_o ∈ Z_{>0}`: quantity in Egg atoms (single-Egg) or lots
  (portfolio). Capacities come from *order quantities*; eligibility is **not**
  a primal bound — it will fall out of the dual (§5.2);
- a **limit** `ℓ_o ∈ Z_{≥0}` in price units per unit: `limit_price` for a
  single-Egg order (so `0 ≤ ℓ_o ≤ S`), and `limit_collateral_per_lot · S` for
  a portfolio order. This is exactly the cross-multiplied form V2 compares
  (`relation_v1.rs::classify_order`), so the LP and the implementation share
  units with no division anywhere.

### 2.2 Variables

- `f_o ∈ [0, q_o]` — the fill of order `o` (atoms or lots);
- `σ ≥ 0` — global virtual split: complete sets created by the pot;
- `μ ≥ 0` — global virtual merge: complete sets destroyed by the pot.

`σ, μ` are unbounded above in the base formulation (§4.3 discusses the capped
variant needed for the literal CertF reuse).

### 2.3 Constraints — the matrix `A`, row by row

One equality row per active outcome `i < n`:

```text
(C-i)    Σ_{o buy} a_{o,i} f_o  −  Σ_{o sell} a_{o,i} f_o  −  σ  +  μ  =  0
```

**Exact meaning of row `i`:** conservation of Egg `i` inside the batch. Buy
legs receive `B_i = Σ_{buy} a_{o,i} f_o` atoms; sell legs deliver
`E_i = Σ_{sell} a_{o,i} f_o`; the split creates `σ` atoms of *every* outcome;
the merge absorbs `μ` of every outcome. Sources = sinks:
`E_i + σ = B_i + μ`, which is (C-i) rearranged and is byte-identical to the
implemented V4 check (`check_conservation_identity`:
`flows.buy[i] + virtual_merge == flows.sell[i] + virtual_split`). The
right-hand side is `0` because the pot must end the epoch empty ("pot empty,
every atom owned") — there is **no disposal column**, and §6 shows this
choice has a real dual consequence.

The matrix `A` (`n` rows, one column per order plus two):

| column | entries in row `i` | why |
|---|---|---|
| buy order `o` | `+a_{o,i}` | buyer receives its legs |
| sell order `o` | `−a_{o,i}` | seller delivers its legs |
| `σ` | `−1` in every row | a complete set is one atom of *every* outcome |
| `μ` | `+1` in every row | inverse |

The **partition of unity enters the LP in exactly two places**: the σ/μ
columns are all-ones (in claim space, `Σ_i N_i = 1` means the complete set is
the constant payoff 1), and their objective coefficients are `±S` (a complete
set is worth exactly one collateral unit at every resolved value — Theorem
(ii) of the distributional design). Nothing else about the basis — knots,
degree, overlaps — is visible to the clearing. This is worth saying plainly:
**the LP knows the basis only through PoU.** Everything in §3–§6 is therefore
degree-independent; degree enters only in §7, where the *interpretation* of
the dual vector is at stake.

Portfolio columns are the second coupling source: a portfolio column has
several nonzero rows, tying outcomes together in `A` even with `σ = μ = 0`.

### 2.4 Objective

Maximize batch surplus, in exact price units:

```text
maximize   Σ_{o buy} ℓ_o f_o  −  Σ_{o sell} ℓ_o f_o  −  S·σ  +  S·μ
```

A buy values its fill at up to `ℓ_o` per unit; a sell requires at least
`ℓ_o`; the split consumes one collateral unit (`S` price units) per set; the
merge returns one. Write `w` for this objective vector, so the LP is
`max w·(f, σ, μ)` subject to `A(f, σ, μ) = 0`, `0 ≤ f ≤ q`, `σ, μ ≥ 0`.

**Lemma 2.1 (boundedness and attainment). [PROVED HERE]** The supremum is
finite and attained. *Proof.* From (C-i), `μ − σ = E_i − B_i ≤ E_i` for any
`i`, and `E_i` is bounded by total sell-leg quantity, so the `S(μ − σ)` term
is bounded; the `ℓ_o f_o` terms are bounded by `Σ q_o ℓ_o`. The feasible set
is a nonempty (take 0) closed polyhedron; a bounded LP attains. ∎

**Lemma 2.2 (churn normalization). [PROVED HERE]** Replacing `(σ, μ)` by
`(σ − t, μ − t)`, `t = min(σ, μ)`, preserves feasibility and the objective.
So an optimum with `min(σ, μ) = 0` always exists, and the relation's
`ChurnNotCanonical` gate (`min(σ,μ) = 0`, i.e. `(σ, μ) = (c_+, c_−)`) selects
a representative of each objective-equivalent class, losing nothing. ∎

The candidate's free economic coordinate `c = σ − μ` (the implemented
`CandidateV1::imbalance`) is thus the *primal* coordinate of the witness; `p`
will turn out to be the *dual* one. The witness is a primal–dual pair.

---

## 3. The dual, exactly

Standard LP duality for `max w·x, Ax = 0, 0 ≤ x ≤ (q, ∞, ∞)`: dual variables
`π ∈ R^n` (free — the rows are equalities) on the conservation rows, and
`s_o ≥ 0` on the upper bounds `f_o ≤ q_o`. The dual program:

```text
minimize   Σ_o q_o s_o
subject to
  (buy o)     a_o · π + s_o  ≥  ℓ_o
  (sell o)   −a_o · π + s_o  ≥  −ℓ_o        i.e.  s_o ≥ a_o·π − ℓ_o
  (σ)        −Σ_i π_i        ≥  −S          i.e.  Σ_i π_i ≤ S
  (μ)        +Σ_i π_i        ≥  +S          i.e.  Σ_i π_i ≥ S
  s ≥ 0,  π free.
```

### 3.1 Economic reading of every dual object

| dual object | constraint it prices | economic reading |
|---|---|---|
| `π_i` | conservation row `i` | the batch price of one atom of Egg `i`, in price units — the marginal batch surplus of relaxing outcome-`i` conservation by one atom |
| `s_o` (buy) | `f_o ≤ q_o` | the buyer's per-unit surplus rent `(ℓ_o − a_o·π)_+`: what one more unit of this order's capacity would add |
| `s_o` (sell) | `f_o ≤ q_o` | the seller's per-unit rent `(a_o·π − ℓ_o)_+` |
| σ-column constraint | dual feasibility of splitting | no-arbitrage: the parts may not be worth more than the whole (`Σπ ≤ S`), else infinite splitting would be a money pump |
| μ-column constraint | dual feasibility of merging | no-arbitrage: the whole may not be worth more than the parts (`Σπ ≥ S`) |

### 3.2 The normalization is dual feasibility, not a constraint on `p`

**Theorem 3.1 (forced simplex sum). [PROVED HERE — standard LP duality
applied to our matrix]** Every dual-feasible `π` satisfies `Σ_i π_i = S`
exactly. *Proof.* The σ and μ columns give `Σπ ≤ S` and `Σπ ≥ S`
respectively. ∎

Three consequences worth spelling out:

1. **The implied measure normalizes because conversion exists, not because it
   is used.** Even a purely-direct clearing (`σ = μ = 0`) has `Σπ = S` forced,
   because the *availability* of the technology constrains prices. This
   matches the relation exactly: the virtual pair is always present in the
   witness shape, and V1 demands the simplex sum unconditionally.
2. **V1 is two rows of dual feasibility.** In the capped encoding of §4.3, the
   simplex-sum check `SimplexSumMismatch` is literally the certificate check
   `A^T π + s ≥ w` restricted to the two virtual columns with their slacks
   pinned to zero.
3. **PoU ⇒ normalization.** The all-ones column *is* the partition of unity,
   and Theorem 3.1 is the precise sense in which PoU forces the price vector
   to be a normalized object. (Degree does not appear; see §2.3.)

### 3.3 Whose multiplier is the normalization? (the prompt's question)

The question "does `Σ p = 1` appear as a normalization, and is its multiplier
the collateral price?" has a sharp answer, and it is *not* "yes":

- In the **primal-fills LP** (§2), `Σπ = S` is not a row at all; it
  materializes as dual feasibility of the σ/μ columns (Theorem 3.1). Its
  complementary *primal* objects are `σ` and `μ` themselves.
- In the **dual LP viewed as a program over `(π, s)`** — which is the natural
  home of the implemented witness, since the candidate carries `p` — the
  constraint `Σ_i π_i = S` *is* a row, and by taking the dual of the dual its
  multiplier is exactly `σ − μ = c`, **the net complete-set conversion
  volume**, i.e. the collateral flow through the pot measured in sets.
  **[PROVED HERE** — LP bidual; the normalization row and the conversion
  columns are a complementary pair.**]**
- The **collateral price** is not the multiplier; it is the *numéraire the
  normalization fixes*. In the homogeneous formulation with an explicit
  collateral price `π_$`, the conversion columns force `Σ_i π_i = S·π_$`, and
  `Σπ = S` is the gauge choice `π_$ = 1`. Prices in this venue are quoted in
  collateral because the simplex constraint says so; there is no residual
  degree of freedom left for a "price of collateral" to live in.

So: **normalization row ↔ conversion volume; numéraire ↔ the normalization
itself.** [PROVED HERE]

### 3.4 What is *not* forced: nonnegativity

Nothing in the dual forces `π ≥ 0`. In a Fisher market (`fisher.rs`) supply
constraints are inequalities `Σ_i x_{ij} ≤ s_j` — free disposal — so prices
are structurally nonnegative there. Our conservation is an *equality* with no
disposal column, a deliberate design fact (fully collateralized claims may
not vanish; the pot must end empty). The cost is real: §6 exhibits a book
whose only optimal duals have `π_i ≤ −S`. Nonnegativity of the published
price vector is a property of the *accept set* (§5.5), not of the dual
polyhedron.

---

## 4. Weak duality, the certificate, and the relation as a disassembled checker

### 4.1 Weak duality over our matrix

**Theorem 4.1 (weak duality). [PROVED HERE; the general-matrix statement is
MACHINE-CHECKED ELSEWHERE (`Market.weak_duality`)]** For every primal-feasible
`(f, σ, μ)` and dual-feasible `(π, s)`:

```text
Σ_buy ℓ_o f_o − Σ_sell ℓ_o f_o − Sσ + Sμ   ≤   Σ_o q_o s_o
```

*Proof.* Multiply each buy constraint by `f_o ≥ 0`, each sell constraint by
`f_o ≥ 0`, and sum:

```text
Σ_buy ℓ f − Σ_sell ℓ f  ≤  Σ_o f_o s_o + Σ_i π_i (B_i − E_i)
                         =  Σ_o f_o s_o + Σ_i π_i (σ − μ)          by (C-i)
                         =  Σ_o f_o s_o + S(σ − μ)                 by Thm 3.1.
```

Move `S(σ−μ)` left and bound `Σ f_o s_o ≤ Σ q_o s_o` by `f ≤ q`, `s ≥ 0`. ∎

**The middle step is implemented.** `Σ_i p_i(B_i − E_i) = S(σ − μ)`
rearranges to `Σ_i B_i p_i + μS = Σ_i E_i p_i + σS`, which is
byte-for-byte the V8 cash-closure check in `settle_cash`:

```text
consideration + merge_proceeds == seller_credit + split_cost
```

So the implemented per-epoch cash conservation equation **is** the
`π^T(Af) = 0` step of the weak-duality proof, evaluated in exact integers at
`π = p`. The venue already computes one line of the optimality certificate on
every accepted candidate. [PROVED HERE — an identity between two displayed
formulas, given V1 (`Σp = S`) and V4 (`Af = 0`).]

### 4.2 The objective is the implemented `limit_surplus`

Define the **minimal slacks at `π`** (exactly `pdhg.rs`'s `s = (w − A^Tπ)_+`
construction):

```text
s_o(π) := (ℓ_o − a_o·π)_+   for buys,      (a_o·π − ℓ_o)_+   for sells.
```

These are the V2 eligibility distances: `s_o(p) > 0` iff order `o` is
**Strict** at `p`; `s_o(p) = 0` with the constraint tight iff **Marginal**;
constraint strictly slack iff **Ineligible**. V2's trichotomy *is* the case
analysis of the dual constraints.

**Proposition 4.2. [PROVED HERE]** For any candidate passing V1, V2
(ineligible fills zero) and V4, with `Σp = S`:

```text
LP objective at (f, σ, μ)  =  Σ_o f_o · s_o(p)  =  ledger.limit_surplus.
```

*Proof.* Objective `= Σ_buy f(ℓ − a·p) + Σ_sell f(a·p − ℓ) + [Σ_i p_i(B_i −
E_i) − S(σ−μ)]`; the bracket is `(Σp − S)(σ−μ) = 0`; eligible orders
contribute `f·s_o(p)` (for marginal, `0`), ineligible contribute `f = 0`.
The last equality is the definition of the V6 `limit_surplus` accumulator
(`scaled_reservation(fill) − order_value` per buy, mirrored per sell). ∎

So score component 3 is the LP objective, recomputed exactly, already.

### 4.3 The certificate object, and the CertF.lean instantiation

**Corollary 4.3 (gap formula). [PROVED HERE]** For a candidate passing
V1/V2/V4 with fills in the box, taking `(π, s) = (p, s_·(p))`:

```text
gap  :=  Σ_o q_o s_o − objective  =  Σ_o s_o(p) · (q_o − f_o)  ≥ 0,
```

a sum of per-order nonnegative integers, each the order's eligibility
distance times its unfilled quantity. Every term is attributable: **the gap
is itemized, per order, in exact price units.** By
`Market.certifies_epsilon_optimal` (MACHINE-CHECKED ELSEWHERE, general
matrix), a candidate whose gap is `ε` is `ε`-optimal: no feasible clearing of
the same book beats it by more than `ε` price units of surplus.

**Fitting the literal CertF shape.** `CertF.lean`'s `FlowLP` requires a box
`0 ≤ f ≤ c` on *every* variable, while our `σ, μ` are uncapped. Two honest
routes: (a) cap `σ, μ` at `Q̄ :=` total buy-leg quantity — sound by Lemma 2.2
(a churn-free optimum has `σ ≤ min_i B_i ≤ Q̄`, `μ ≤ min_i E_i ≤ Q̄`) — and
then a certificate that sets the two new slacks `s_σ = s_μ = 0` *forces*
`Σπ = S` as two dual-feasibility rows (this is the encoding under which V1 is
literally part of the `A^Tπ + s ≥ w` check); or (b) extend the Lean
development with free-above variables. Route (a) needs no new Lean theorems:
`weak_duality`, `certifies_epsilon_optimal`, `gap_nonneg` are stated for an
arbitrary matrix over an ordered commutative ring and instantiate at `R = ℤ`,
`V = Fin n`, `E = Fin (m + 2)` with our `A`. **The instantiation is not done;
it is definitional work plus the accept-set theorem of §5, and per this
repository's standing discipline it belongs in Lean, with Rust only calling
the artifact — not re-derived in Rust.** [MACHINE-CHECKED ELSEWHERE for the
general theorems; the instantiation is OPEN work, named in §11.]

### 4.4 The relation, stage by stage, as certificate anatomy

| relation stage | certificate component |
|---|---|
| V1 simplex (`Σp = S`, `0 ≤ p_i ≤ S`) | dual feasibility of the σ/μ columns (`Σπ = S`, Thm 3.1); box membership `p ∈ [0,S]^n` is *extra* — see §6 |
| V2 eligibility trichotomy | the case analysis of dual constraints at the minimal slacks `s_o(p)`; `IneligibleFill` = complementary slackness `f_o · slack_o = 0` |
| V3 box `0 ≤ f ≤ q` + canonical equality | primal box feasibility; canonical allocation = a selection *inside* the CS-indifferent (marginal) set |
| V3 `StrictUnderfill` refusal (alloc A) | complementary slackness `s_o(q_o − f_o) = 0` enforced as a refusal — the zero-gap gate (§5) |
| V4 conservation (C-i), `ChurnNotCanonical` | primal feasibility `Af = 0`; canonical churn representative (Lemma 2.2) |
| V5 pairing gate (H-i-O) | **not in the LP** — Hall rows, an LP extension with its own duals (§5.6) |
| V6 `limit_surplus` | the LP objective (Prop 4.2) |
| V7 fees | **outside the LP** by design (§9.4) |
| V8 cash closure | the `π^T(Af) = 0` step of weak duality (§4.1) |
| V9 score | selection among valid candidates = selection on/near the optimal face (§9.3) |

The relation is a disassembled Cert-F checker whose pieces were derived
independently as conservation and refusal discipline. That is the structural
content of the conjecture, and it is true. [PROVED HERE, as the sum of the
identities above.]

---

## 5. The zero-gap theorem, complementary slackness, and market readings

### 5.1 The theorem

**Theorem 5.1 (accepted ⇒ zero-gap optimal). [PROVED HERE]** Fix the policy
tuple: allocation **A** (`PricePriorityMarginalProRata`), AON **2a** (AON and
`minimum_fill > 1` refused at admission), portfolio lots **P-a**, self-cross
**N-a or N-b**, any rounding variant, any dust variant. Let a candidate
`(p, c, fills)` be accepted by `verify` on a normalized book. Then with
`(π, s) := (p, s_·(p))` the triple `(fills, σ, μ, π, s)` is a **zero-gap
certificate**: the fills maximize batch surplus over the entire real LP
polytope of §2, and `p` is an optimal dual.

*Proof.* Primal feasibility: V3 enforces the box, V4 enforces (C-i),
`σ, μ ≥ 0` by type. Dual feasibility: `s_·(p) ≥ 0` and satisfies each order
constraint by construction of the positive part; `Σp = S` by V1 (Theorem 3.1's
two rows). Gap `= Σ_o s_o(p)(q_o − f_o)` by Corollary 4.3. A term is nonzero
only for a strict order (`s_o > 0`) not fully filled. Under the tuple: strict
single-Egg orders are filled fully or the candidate refuses
(`StrictUnderfill`, both in the per-outcome pre-check and in
`allocate_single_side`); strict portfolios are forced to full size under P-a
(`state.forced`) or refuse; marginal portfolios are excluded under P-a but
have `s_o = 0`; ineligible orders fill zero (V2) and have `s_o(p) = 0` with a
strictly slack constraint, contributing nothing; there are no AON/minimum-fill
obligations under 2a; expired orders were refused at V0. Hence every term of
the gap vanishes. Weak duality (Thm 4.1) does the rest. ∎

Three qualifications, honestly:

- The theorem is **relative to the netted book** under N-b: netting is a
  price-independent pre-transform and is documented as economically
  conservative; the LP is over effective quantities.
- The theorem covers the **accept set**, one direction. The converse — which
  zero-gap points are accepted — is a *selection*: the canonical allocation
  picks one representative rationing of the marginal set (largest remainder,
  seeded rank), all such rationings being optimal since marginal orders carry
  `s_o = 0`. `DustRejected` only ever refuses; it never admits a positive-gap
  point.
- Under **N-c** the implemented derivation may interact with owner-capping in
  ways not analyzed here; Theorem 5.1 is stated for N-a/N-b only. [OPEN for
  N-c.]

**Consequence for the repository's language.** "Best valid submitted
candidate" remains the correct *selection* claim (the score chooses among
optima, §9.3), but under this tuple every accepted candidate is provably
**surplus-optimal for the LP relaxation** — a strictly stronger statement
than the current posture allows anywhere. It should not be promoted to prose
until the named falsifier of §11 pins it over the actual code
(`every_accepted_candidate_has_zero_duality_gap`), and never phrased as
"verified" before the Lean instantiation closes.

### 5.2 Complementary slackness in market terms

Complementary slackness for our LP, read as market sentences:

| CS condition | market reading | where implemented |
|---|---|---|
| `s_o > 0 ⇒ f_o = q_o` | an order strictly inside its limit must be **fully filled** — price priority is not a policy preference, it is dual feasibility | alloc A strict-full-fill + `StrictUnderfill` |
| `f_o > 0 ⇒ a_o·π + s_o = ℓ_o` (buys) | a filled order trades at a price consistent with its limit; with minimal slacks this is an identity for eligible orders | V2 + V6 `ConsiderationMismatch` (fill valued above limit refuses) |
| `slack_o > 0 ⇒ f_o = 0` | an order strictly outside its limit fills **zero** | V2 `IneligibleFill` |
| `0 < f_o < q_o ⇒ s_o = 0` | **the marginal order trades exactly at its limit** — partial fills happen only at indifference, which is why rationing needs a canonical rule at all: the LP is silent exactly there | marginal pro-rata, largest remainder, seeded rank |
| `σ > 0 ⇒ Σπ = S` tight (capped form) | complete sets are manufactured only when the parts are worth exactly the whole | `Σp = S` (V1) + `ChurnNotCanonical` |

The scalar special case: at `n = 1` with `σ = μ = 0` the LP degenerates to
the uniform-price crossing of `fhegg-solver/clearing.rs` — `min(D(j), S(j))`
volume, dual price in the marginal interval. Our relation is that crossing
with the price interval coupled across outcomes by the all-ones column.

### 5.3 Non-uniqueness of the dual: the published measure has a chosen point

Two-order example, `n = 2`, orders only on outcome 0: buy `q=1, ℓ=9_000`;
sell `q=1, ℓ=1_000` (price units, `S = 10_000`). Conservation on the empty
outcome 1 forces `c = 0`; the optimal fills are `f = (1,1)`, surplus `8_000`.
*Every* `p_0 ∈ [1_000, 9_000]` (with `p_1 = S − p_0`) is an optimal dual, and
every corresponding candidate is accepted. The optimal dual face is an
interval — generically a polytope — and **"the implied measure" is a set
until something selects a point.** In the implemented system the selector is
the score (component 1, dispersion-weighted volume, then surplus, then the
digest tie-break): a frozen, deterministic selection rule *on the optimal
face*. Whether that selection has a clean variational characterization
(analytic-center-like, entropy-like) is [OPEN], §11.

### 5.4 What breaks the zero gap, exactly

- **Allocation B** (full pro-rata): strict orders may be partially filled;
  the accepted candidate's gap is `Σ_strict s_o(p)(q_o − f_o) > 0` in
  general. Allocation B accepts clearings **without a certificate at the
  witness price**; the certificate degrades to ε-optimality with computable
  ε. §6.3 shows the gap can even be loose (the fills optimal, the certificate
  not). **Allocation A is exactly the policy "clear only with certificate";
  the `StrictUnderfill` refusal is a positive-gap detector.** [PROVED HERE]
- **AON 2b** (witnessed honored mask `M`): an unhonored obligated strict
  order sits at `f = 0` with `s_o > 0`. Two-level statement [PROVED HERE]:
  the accepted candidate is **zero-gap optimal for the branch LP** `LP_M`
  (honored orders' fills fixed at `q_o` — forced full by
  `AonMaskDishonored`, so their gap terms vanish; unhonored obligated orders
  have capacity 0 in `LP_M`), while its gap against the *full relaxation* is
  exactly `Σ_{unhonored obligated, strict} q_o · s_o(p)`. The certificate is
  branch-tight; **branch selection is the uncertified search** — consistent
  with the design's warning that mask maximality is not verified, and with
  the nonmonotone 2-cycle that makes the honored set a lattice search rather
  than a fixed point.

### 5.5 Nonnegativity on the accept set

The dual polyhedron does not force `π ≥ 0` (§3.4) — but the *witness* does:
V1 demands `p ∈ Δ_S ⊂ [0, S]^n`. So on the accept set, Theorem 5.1 delivers a
**nonnegative** optimal dual for free. This is the hinge on which §7's
measure statement turns, and §6 shows the demand is not vacuous: books exist
where no nonnegative optimal dual exists, and there allocation A does not
clear at all.

### 5.6 The pairing gate is the one genuinely new dual family

V5's feasibility inequality `part_i(O) ≤ F_i = B_i + μ` (Hall condition
H-i-O) is linear in the fills, so the *true* relation polytope adds one row
per (outcome, owner). Under N-a/N-b these rows are implied by
single-sidedness (`part_i(O) ≤ B_i ≤ B_i + μ`) hence redundant, and the dual
of the extended LP extends any dual of §3 by zero multipliers — **the
ownership constraints price at zero** [PROVED HERE — a redundant row can
carry a zero multiplier and feasibility is unchanged]. Under N-c they can
bind, and their multipliers `η_{i,O} ≥ 0` enter the affected orders' dual
constraints as an **owner-congestion price**: an owner standing on both sides
of an outcome faces a worse effective price by exactly the shadow cost of the
no-self-trade rule. No analogue exists in the σ/μ or Fisher structure; this
is the one place the venue's dual has a term textbook market duality lacks.
[Economic reading PROVED at LP level; interaction with the implemented N-c
derivation OPEN.]

### 5.7 Comparison with `fisher.rs` `CertEq`

| `CertEq` (Eisenberg–Gale KKT) | this LP | comment |
|---|---|---|
| `x ≥ 0`, `Σ_i x_{ij} ≤ s_j` | `0 ≤ f ≤ q`, `Af = 0` | theirs is an *inequality* (free disposal) — the structural reason their prices are nonneg and ours are not (§3.4) |
| `p ≥ 0` | not structural; supplied by V1 on the accept set | see §5.5, §6 |
| stationarity `β_i u_{ij} ≤ p_j` | dual feasibility `ℓ_o ≤ a_o·π + s_o` | quasilinear utilities: our "β" ≡ 1 because limits are already in money |
| buyer CS `Σ x_{ij}(p_j − β_i u_{ij}) = 0` | `f_o·(a_o·π + s_o − ℓ_o) = 0` | with minimal slacks this is an *identity* for eligible orders; all content shifts into the box CS (next row) — a real structural difference from EG, where β is derived, not chosen minimal |
| market-clearing CS `Σ_j p_j(s_j − Σ_i x_{ij}) = 0` | `π^T(Af) = 0` | trivial for us (equality rows) — and *implemented* as the V8 cash identity (§4.1) |
| budget exhaustion (derived) | per-owner `debit ≤ reservation`, refund closure | ours is individual rationality per order (limits), not budgets; V6's owner ledger is the analogue |
| bilinear check `O(n·g)` | linear check (Cert-F class) | our LP has *fixed* weights (limits are data), so the certificate stays in the linear, AIR-friendly class — the same reason CertF, not CertEq, is the right Lean anchor |

---

## 6. Where the dual is NOT the price vector: negative prices, lapse, and the grid

### 6.1 The one-sided book: negative optimal duals [REFUTED: "the dual is always an admissible price vector"]

Book: `n = 3`, `S = 10_000`. Two buy orders, quantity `Q` each, at the
maximum limit `ℓ = S`: one on outcome 1, one on outcome 2. **Nothing touches
outcome 0; no sells anywhere.**

*Primal.* (C-0) with `B_0 = E_0 = 0` forces `μ = σ`; then (C-1), (C-2) force
`B_1 = B_2 = σ − μ = 0`. Optimal value **0**: nothing can clear. The buyers
would happily pay `S` each for legs 1 and 2 — a split would gross up to `2S`
against cost `S` — but the leftover Egg-0 has no buyer, no merge partner, and
**no disposal**, so the equality rows refuse the trade. (The real system
refuses it too: split capacity must be exactly consumed by buy legs.)

*Dual.* Minimize `Q(S − π_1)_+ + Q(S − π_2)_+` over `Σπ = S`. The minimum is
`0`, attained **only** where `π_1 ≥ S` and `π_2 ≥ S`, hence `π_0 ≤ −S < 0`.
Every optimal dual leaves the box `[0, S]^3`. The negative price is
meaningful: it is the shadow price of the missing disposal — relaxing (C-0)
by one atom (letting one Egg-0 vanish) raises optimal surplus by exactly `S`
(then `σ = 1`, `B_1 = B_2 = 1`, surplus `2S − S = S`). [PROVED HERE]

*The relation, exhaustively.* At every V1-valid `p`: `p_1 = p_2 = S` is
impossible, so at least one buy is Strict (`p_i < S = ℓ`); under allocation A
it must fill fully; but for `c > 0` outcome 0 refuses `InfeasibleVirtualLeg`
(`min(D_0, S_0 + c) = 0 < c`), for `c = 0` all `B_i = 0` and the strict buy
refuses `StrictUnderfill`, and `c < 0` refuses on outcomes 1, 2. **No valid
candidate exists at any `(p, c)`: the epoch lapses.** [PROVED HERE] This is a
concrete witness for open question §16.1 of the batch design — and a
reframing:

> **Under allocation A, the epoch lapses exactly when the LP's optimal dual
> face does not meet the admissible price set.** The lapse is a *dual*
> phenomenon: the venue refuses to clear without a certificate, and in this
> book no admissible certificate exists.

(Proved as stated for this instance; the general equivalence is Theorem 6.2.)

Under allocation B the empty candidate **is** valid here — and it is in fact
LP-optimal (value 0) — but its own certificate at any admissible `p` has gap
`≥ Q·min(S−p_1, S−p_2)_+ > 0`: the fills are optimal, the certificate is
loose. The gap bounds suboptimality; it does not measure it exactly when the
dual is forced off the box. Allocation A internalizes this by refusing;
allocation B clears uncertified.

### 6.2 When is the lapse exactly dual-infeasibility-in-the-box?

**Theorem 6.2 (existence, single-Egg books). [PROVED HERE at proof-sketch
rigor; promoted only by the §11 falsifier]** For a book of single-Egg orders
only, under (A, 2a, N-a/N-b, dust = distribute): a valid candidate exists iff
some LP-optimal dual lies in `Δ_S` (equivalently, in `[0,S]^n` — Theorem 3.1
gives the sum). Moreover the integer grid costs nothing: the constraint
matrix `[±e_i | −1 | +1]` has the consecutive-ones interval property (each
column is ± an interval indicator: a singleton or the full column), hence is
totally unimodular [STANDARD: interval matrices are TU; appending identity
rows for the box preserves TU], so with integer data (`ℓ ∈ Z`, `S ∈ Z`,
`q ∈ Z`) the optimal dual face, if it meets the box, meets it at an integral
point on the tick grid [STANDARD: Hoffman–Kruskal].

*Sketch of the constructive direction.* Take an optimal dual `p* ∈ Δ_S` and
an optimal churn-free primal with imbalance `c*`. At `(p*, c*)` the canonical
derivation sets `B_i = min(D_i(p*), S_i(p*) + c*) ≥ B_i*`, so strict demand
is covered (no `StrictUnderfill`), `D_i ≥ B_i* ≥ c*` blocks
`InfeasibleVirtualLeg`, and any volume beyond `B_i*` is marginal-against-
marginal (`s = 0` both sides), staying zero-gap. The derived candidate
verifies. ∎(sketch)

### 6.3 Portfolio columns break the grid [CONJECTURE with evidence]

Portfolio coefficient columns destroy the interval property. Subdeterminant
witness: columns `(1,2,0)`, `(0,1,2)` and the all-ones μ-column give
`det [[1,0,1],[2,1,1],[0,2,1]] = 3`. Consequence: dual vertices with
denominator 3. Pinning instance: if two portfolio buys with coefficients
`a_1 = (1,2,0)`, `a_2 = (0,1,2)` and limit one collateral per lot
(`L = S`) are both **marginal and partially filled** at the optimum, the
pinning system `a_1·π = S`, `a_2·π = S`, `Σπ = S` has the unique solution
`π = (S/3, S/3, S/3)` — off the integer grid for `S = 10_000`. Nearby grid
points change the eligibility pattern (e.g. at `(3333, 3333, 3334)`:
`a_1·p = 9_999 < S` strict, `a_2·p = 10_001 > S` ineligible), so the exact
LP optimum is unreachable and the best achievable certificate gap is a
positive integer number of price units. **Conjecture 6.3:** there exists a
complete book (with the counterparties that make both portfolios marginal
and partially filled) admitting *no* zero-gap grid candidate while the real
LP dual face is nonempty in the box. The pinning arithmetic above is the
evidence; the full book is deliberately left as the named falsifier
`dual_face_off_grid_book` (§11) rather than asserted. Under allocation A
such a book would lapse for grid reasons, not economic ones — a second,
distinct lapse mode from §6.1.

---

## 7. The measure claim: when the price vector is a quadrature rule

Now, and only now, the basis enters. Let the market's frozen basis be
`N_0, …, N_{n−1}` of degree `d` on the knot grid, with the edge policy
applied, so the resolved-value domain is effectively `X = [t_0, t_{K−1}]`,
`Σ_i N_i(x) = 1` and `N_i ≥ 0` on all of `X` (H1/H2 of the distributional
design). A **representing measure** for a price vector `p ∈ Δ_S` is a
probability measure `Q` on `X` with

```text
p_i / S = E_Q[ N_i(X) ]        for every i.
```

Define the **moment body** `M_d := { (∫ N_i dQ)_i : Q a probability measure
on X }` — the closed convex hull of the moment curve `x ↦ N(x)`. The
conjecture's measure half asks: is `Δ_S/S ⊆ M_d`? Note first what is *not*
being asked: the LP dual is not derived from a measure, and nothing in
§2–§6 mentions the basis. The honest question is whether the vector the
venue publishes always *admits* a risk-neutral reading.

### 7.1 Degree 0 — the price vector is literally the cell probabilities [PROVED HERE]

Degree-0 basis functions are the cell indicators, so
`∫ N_i dQ = Q(cell_i)`: **`M_0` is the entire simplex** (place mass `p_i/S`
anywhere in cell `i`), and moreover `p_i/S` is the cell mass of *every*
representing measure — at cell resolution the implied measure is unique.
The price vector is the probability mass function of the market's implied
distribution over cells, exactly as the distributional design's Theorem (iv)
reads it.

### 7.2 Degree 1 — every simplex vector is a hat-moment vector [PROVED HERE]

Degree-1 hats are **cardinal**: `N_j(t_i) = δ_{ij}` (at `u = 0` the §2.2
weight vector is `D·e_k`; the quantized map satisfies `w(t_i) = D e_i`
exactly, and exactly-exactly under `B1-EXACT`). Hence for any `p ∈ Δ_S`, the
atomic measure

```text
Q* := Σ_i (p_i / S) · δ_{t_i}
```

represents `p`: `E_{Q*}[N_j] = Σ_i (p_i/S) N_j(t_i) = p_j/S`. **`M_1` is the
entire simplex**, and combined with §5.5:

> **Theorem 7.1.** At degree ≤ 1, *every* vector the venue can ever publish
> (every V1-valid `p`) is the basis-moment vector of a probability measure,
> exhibited explicitly; and on the allocation-A accept set that same vector
> is a zero-gap optimal dual (Thm 5.1). The certificate and the measure are
> carried by one object. [PROVED HERE]

**The quadrature reading.** For any portfolio with coefficients `c`, the
venue's valuation is `dot(c, p)/S = Σ_i c_i (p_i/S) = E_Q[ĝ(X)]` for every
representing `Q`, where `ĝ = Σ c_i N_i` is the piecewise-linear interpolant
of `c` — i.e. **`{(t_i, p_i/S)}` is a positive, normalized quadrature rule,
exact on the whole tradable spline space**. Positivity is `p ≥ 0`;
normalization is `Σp = S` = Theorem 3.1 = PoU. That is the conjecture's
phrase "normalized positive quadrature rule for the risk-neutral measure,"
made precise.

**Resolution honesty (what "the" measure means).** `Q` is not unique for
`d = 1`: two measures are venue-indistinguishable iff they integrate every
piecewise-linear-on-the-grid function identically (within a pane, only mass
and conditional mean can matter, since the weights are affine in `x`; and
only `n` linear functionals of those survive). Publishing `p` publishes the
risk-neutral measure **at exactly the resolution the market trades** — its
projection onto the dual of the spline span — and nothing finer. This is not
a defect; it is self-consistency: the published object prices exactly the
claims that exist, and is silent about distinctions no claim can pay on.
Degree 0 vs degree ≥ 1, as the prompt asks: at `d = 0` the vector is
literal cell *masses* (unique at cell resolution, §7.1); at `d = 1` it is a
**mollified object** — `p_i/S = ∫ N_i dQ` is the density averaged against
the hat kernel, a kernel-smoothed sample of the density, and division by the
gap gives the density estimate `p_i/(S·g)` — which is the interesting case.

**Required assumptions, named.** (i) Zero interest over the epoch: the
complete set trades at `S` now and pays one collateral unit at resolution,
so prices are undiscounted expectations (the measure is the T-forward
measure under the venue's zero-carry convention). (ii) Frictionlessness
inside the relation: true by design — fees never enter the normalization
(§9.4). (iii) The LP-relaxation reading (divisibility); §8 prices the
difference. (iv) Quantization: the kernel pays `w_i(x̂)/D`, not `N_i(x̂)`;
the discrepancy is below `d/D` per claim unit (§2.3 of the distributional
design), zero on the `B1-EXACT` path, and the quantized statement of
Theorem 7.1 survives verbatim because `w(t_i) = D e_i` exactly.

### 7.3 Breeden–Litzenberger is pre-inverted: the hat basis IS the butterfly basis [PROVED HERE]

On a uniform grid of gap `g`, for interior `i`:

```text
N_i(x) = [ (x − t_{i−1})_+ − 2(x − t_i)_+ + (x − t_{i+1})_+ ] / g
```

(direct check on each pane). Taking `E_Q` of both sides, with
`C(k) := E_Q[(X − k)_+]` the undiscounted call price:

```text
p_i / S = [ C(t_{i−1}) − 2 C(t_i) + C(t_{i+1}) ] / g,
```

the discrete Breeden–Litzenberger second difference **is the claim price
itself**. There is no inversion step, no differentiation of a fitted
surface, no smile interpolation: the market quotes the butterflies natively,
and the density at grid resolution is `p_i/(S·g)` read directly off the
published vector. By linear precision, the portfolio `c_i = t_i` pays the
resolved value itself, so `Σ_i t_i p_i / S = E_Q[X]` — **the implied
(undiscounted, clamped) forward is a single portfolio quote**, the same
number for every representing `Q`.

### 7.4 Degree ≥ 2 — the measure half of the conjecture is REFUTED

Interior basis functions of degree 2 peak at `3/4` (degree 3: `2/3`) — the
distributional design's Theorem (iv) tightness discussion. Then for interior
`j`, `e_j ∉ M_d`: a representing measure would need
`1 = ∫ N_j dQ ≤ max_x N_j = 3/4`. Since `e_j` is an extreme point of the
simplex, **`M_d ⊊ Δ` strictly for `d ≥ 2`**: there are V1-valid price
vectors that are no measure's moments. [PROVED HERE]

The separation is not abstract — the arbitrage is explicit and executable in
the admitted order language. At `d = 2`, take the portfolio

```text
c = 3·1 − 4·e_j        (three complete sets, short four units of claim j)
```

Its payoff is `3 − 4 N_j(x) ≥ 3 − 4·(3/4) = 0` for every `x`, yet its price
at `p = S·e_j` is `3S − 4S = −S < 0`. Executable form (coefficients in the
order language must be nonnegative): split 4 sets (`σ`, cost `4` collateral),
sell 4 single-Egg units of claim `j` at limit `S` (eligible at `p_j = S`,
proceeds `4`), keep the complement `4(1 − N_j)` — net outlay zero, resolved
payout at least `4·(1 − 3/4) = 1` collateral, surely. At `d ≤ 1` the same
position can pay exactly `0` (at `x = t_j`, since hats attain 1), so no
arbitrage — consistent with Theorem 7.1. [PROVED HERE]

Consequences, stated carefully:

- **V1 (simplex membership) is exactly the no-static-arbitrage condition on
  the tradable span for `d ∈ {0, 1}`, and strictly weaker than it for
  `d ≥ 2`.** The clearing knows the basis only through PoU (§2.3); the extra
  geometry of overlapping quadratic/cubic supports is invisible to every
  stage of the relation.
- Nothing forces the LP dual into `M_d` either: the dual is constrained by
  the *order* structure, so it lands in the moment body only if the book
  itself contains the arbitrage orders that would punish an exterior price.
- **This warning is now live, not design-ahead.** *(Corrected 2026-08-21.
  The bullet previously read "a design-ahead warning, not a live defect:
  degrees 2–3 currently refuse at terms admission (implementation addendum
  §15)". That sentence was written against `DISTRIBUTIONAL_CLAIMS_DESIGN.md`
  §15 (2026-08-18) and stopped being true on 2026-08-19: `TermsAccount`
  admits `basis_degree` 0 through 3, `derive_payout_vector` owns degrees one
  through three, and `blank_bank_joined_lifecycle_degree_two` /
  `_degree_three` run a full create/split/resolve/redeem walk against a
  local bank. See `CURRENT_TRUTH.md` §3.)* The claim plane landed; the
  admission story for prices did not move with it. The clearing knows the
  basis only through PoU, so nothing in the relation distinguishes
  `M_d` from `Δ`, and no gate in front of the relation restricts a market's
  degree either — as of 2026-08-20 `programs/solana-layout/src/clearing.rs`
  and `crates/clutch-batch` contained no occurrence of `degree`; the second
  half of that stopped being true with V1b, and the first half has not. The
  one live degree-≥2 restriction is on *evidence*, not price:
  `ResolutionRefusal::NonPointEvidence` refuses a conservative interval that
  is not a point. *(Superseded 2026-08-21 by §7.6: the exact price test
  `p/S ∈ M_d` is now derived — Theorem 7.6.5, exact integers, a per-span
  Hausdorff system — the finite certified family that approximates it from
  outside is landed as the relation's V1b stage
  (`relation_v1.rs::validate_price_moment_cone`), and the price vector above
  is refused by it. The stage is off, and provably so, at `d ≤ 1`; the
  wide-support residual it does not catch is §7.6.7, still [OPEN].)*

### 7.5 The measure question, answered as asked

"Is the dual price vector exactly the quadrature rule `p_i = E[B_i(X)]`?"

- The correct direction of the statement is existential, and it factors
  (§0's decomposition): **measure-hood** — for `d ≤ 1`, *yes*: any published
  `p` equals `(S·E_Q[N_i(X)])_i` for the explicit `Q* = Σ (p_i/S) δ_{t_i}`,
  and the venue's pricing of every tradable claim is exactly `E_{Q}`
  quadrature [PROVED HERE]; for `d ≥ 2`, *no* in general [REFUTED, §7.4].
- **Dual-hood** — on the allocation-A accept set the same `p` is a zero-gap
  optimal dual [PROVED HERE, Thm 5.1]; under 2b/B it is an ε-certificate
  with itemized ε (§5.4); on lapse books there is no admissible dual and
  nothing is published (§6.1) — the two failure modes never produce a
  published non-measure. **The venue, at `d ≤ 1`, cannot publish a
  non-measure**; what policy changes is only whether the published measure
  also certifies optimality.
- "THE risk-neutral measure" (definite article) is a category error at
  `d = 1`: the object pinned is the measure's spline-dual coordinates (§7.2
  resolution honesty), and the point chosen on the optimal dual face is a
  score-policy selection (§5.3). Both indeterminacies are intrinsic, not
  implementation gaps.

### 7.6 The moment cone at degrees two and three, and the admission condition

*(Added 2026-08-21, with the landed V1b stage. §7.4 refuted the measure half
above degree one and left "the exact price test" as open question §11.4. This
subsection closes the mathematics of that question — the exact membership
condition, in exact integers, for every admitted grid — proves that no finite
family of linear price inequalities can decide it, and derives the finite
certified family that the relation now enforces.)*

#### 7.6.1 What the admitted basis actually is, and why the cone is `(d, n)`

The frozen representation (`crates/clutch-bspline`, and
`DISTRIBUTIONAL_CLAIMS_DESIGN.md`) admits exactly one shape above degree zero:
`K` distinct stored breakpoints `t_0 < … < t_{K−1}`, expanded to the **open
clamped** knot vector by repeating each endpoint `d + 1` times and keeping each
interior breakpoint once; `n = K − 1 + d` claims; `X = [t_0, t_{K−1}]` with the
edge policy mapping everything outside to the endpoints; and — **mandatory at
`d ≥ 2`** (`Error::UniformSpacingRequired`) — uniform spacing `2^s`. Bounds:
`n ≤ 16`, `K ≤ 16`, `d ≤ 3`.

Two consequences, both used below.

**Lemma 7.6.1 (the cone is a function of `(d, n)` alone). [PROVED HERE]** For
`d ≥ 2` the moment body `M_d` depends only on the degree and the outcome count
— not on the knot values, the spacing exponent, the payout denominator, or the
edge policy. *Proof.* Uniform spacing makes the affine map `φ(x) = t_0 + h·x`
carry the standard grid `0, 1, …, K−1` onto the stored one, and B-splines are
defined by their knot vector, so `N_i = N_i^{std} ∘ φ^{−1}`. For any
probability measure `Q` on `X`, `∫ N_i dQ = ∫ N_i^{std} d(φ^{−1}_* Q)`, and
`φ^{−1}_*` is a bijection between probability measures on `X` and on
`[0, K−1]`; hence the two moment bodies coincide. `K = n + 1 − d`. The edge
policy only decides whether an out-of-span value is refused or clamped to an
endpoint of `X`, which changes no integral over `X`. ∎

*This is the wiring theorem.* An admission test for prices needs the degree and
the outcome count and **nothing else** — one byte, not a knot vector. That is
what makes V1b implementable without widening the relation's domain.

**Lemma 7.6.2 (clamped ends attain 1). [PROVED HERE]** `N_0(t_0) = 1` and
`N_{n−1}(t_{K−1}) = 1` at every degree, by endpoint multiplicity `d + 1`
(equivalently: `evaluate_point` returns `one_hot` at and beyond each endpoint).
So `e_0` and `e_{n−1}` **are** moment vectors at every degree — the §7.4 failure
is interior-only, and any admission condition must leave the two end claims
alone. ∎

#### 7.6.2 Membership is exactly no-arbitrage (the dual form)

Write `C_d := cone{ N(x) : x ∈ X }` and `K_d := { c ∈ R^n : g_c := Σ_i c_i N_i
≥ 0 on X }` — the coefficient vectors of the **nonnegative splines**.

**Theorem 7.6.3. [PROVED HERE]** For `p ∈ Δ_S` the following are equivalent:

1. `p/S ∈ M_d` (some probability measure has `p/S` as its basis moments);
2. `⟨c, p⟩ ≥ 0` for every `c ∈ K_d`;
3. no portfolio in the admitted order language has a payoff that is
   nonnegative at every resolved value and a strictly negative price at `p`.

*Proof.* `{N(x) : x ∈ X}` is compact (continuity, `X` compact) and lies in the
hyperplane `Σ = 1` by PoU, so `conv{N(x)} = M_d` is compact, `0 ∉ M_d`, and
`C_d = R_{≥0}·M_d` is a closed convex cone whose dual is exactly `K_d`
(`⟨c, N(x)⟩ ≥ 0 ∀x` is `g_c ≥ 0`). The bipolar theorem gives `C_d = K_d^*`,
which is (1)⇔(2); PoU turns cone membership into body membership by scaling.
For (3): a position that buys `b ∈ Z_{≥0}^n`, sells `s ∈ Z_{≥0}^n`, splits `σ`
and merges `μ` has claim-space coefficient `c = b − s + (σ − μ)·1` and cost
`⟨c, p⟩` (§2.4), and every integer vector arises this way, so the executable
directions span `K_d ∩ Z^n`; density gives the rest. ∎

So the admission question is not decorative: **outside the cone, a
nonnegative-payoff portfolio is priced negative, and the venue's own order
language executes it.** §7.4's `c = 3·1 − 4 e_j` at `p = S e_j` is one such `c`.

#### 7.6.3 The exact condition, in exact integers

Let the spans be `S_k = [t_k, t_{k+1}]`, `k = 0 … K−2`. On `S_k` exactly the
`d + 1` functions `N_k, …, N_{k+d}` are nonzero; let `T_k` be the (invertible)
matrix writing their restrictions in the Bernstein basis `β_0..β_d` of `S_k`.
Write `H_d ⊂ R^{d+1}` for the truncated **Hausdorff moment cone in Bernstein
coordinates**: the vectors `(∫ β_r dν)_r` over nonnegative measures `ν` on the
span.

**Lemma 7.6.4 (`H_d` explicitly, `d ≤ 3`). [PROVED HERE; STANDARD input:
Hausdorff's truncated moment theorem]**

```text
H_0 = { m ≥ 0 },      H_1 = { m ≥ 0 },
H_2 = { m ≥ 0 :  m_1^2 ≤ 4 m_0 m_2 },
H_3 = { m ≥ 0 :  m_1^2 ≤ 3 m_0 m_2  and  m_2^2 ≤ 3 m_1 m_3 },
```

i.e. `r(d−r)·m_r^2 ≤ (r+1)(d−r+1)·m_{r−1} m_{r+1}` for `1 ≤ r ≤ d−1`.
*Proof.* Hausdorff: `(μ_0..μ_d)` are the ordinary moments of a nonnegative
measure on `[0,1]` iff the two Hankel forms are PSD — for `d = 2`,
`[[μ_0,μ_1],[μ_1,μ_2]] ⪰ 0` and `μ_1 ≥ μ_2`; for `d = 3`,
`[[μ_1,μ_2],[μ_2,μ_3]] ⪰ 0` and `[[μ_0−μ_1, μ_1−μ_2],[μ_1−μ_2, μ_2−μ_3]] ⪰ 0`.
Substituting the Bernstein change of variables (`d = 2`: `μ_2 = m_2`,
`μ_1 = m_1/2 + m_2`, `μ_0 = Σm`; `d = 3`: `μ_3 = m_3`, `μ_2 = m_2/3 + m_3`,
`μ_1 = m_1/3 + 2m_2/3 + m_3`, `μ_0 = Σm`) turns `μ_1 − μ_2` into `m_1/2`
resp. `m_2/3`, the linear conditions into `m ≥ 0`, and the two determinants
into exactly the displayed quadrics — the cross terms cancel identically. Every
step is a rational identity. ∎

**Theorem 7.6.5 (exact membership). [PROVED HERE]**

```text
p/S ∈ M_d   ⟺   ∃ m^0, …, m^{K−2} ∈ H_d  with   p/S = Σ_k E_k T_k m^k,
```

where `E_k` embeds `R^{d+1}` into coordinates `k … k+d`. *Proof.* (⇒) Split a
representing `Q` across the spans (`Q_k := Q|_{S_k}`, shared endpoints assigned
to the lower span) and take `m^k` to be `Q_k`'s Bernstein moments; the
restriction of `N_i` to `S_k` is `Σ_r (T_k)_{(i−k) r} β_r`, so summing
reproduces `p/S`. (⇐) Given `m^k ∈ H_d`, Hausdorff supplies a nonnegative
measure `ν_k` on each span with those Bernstein moments; `Q := Σ ν_k` has total
mass `Σ_i p_i/S = 1` by PoU and the right moments. ∎

This **is** the explicit finite condition: `(d+1)(K−1) ≤ 60` rational unknowns,
`n ≤ 16` linear equations with integer coefficients, and at most `2(K−1)`
integer quadratic inequalities — a decidable, exactly-integer system, and one a
witness makes checkable in linear time. What it is *not* is quantifier-free:
the existential over the span moments does not eliminate in closed form for
`K > 2`, which §7.6.4 shows is not an accident of presentation.

**Corollary 7.6.6 (single-span grids: quantifier-free and exact). [PROVED
HERE]** When `K = 2` — i.e. `n = d + 1`, the shortest admitted grid at each
degree — the basis *is* the Bernstein basis of one span, `T_0 = I`, and
membership is exactly

```text
d = 2 (n = 3):   p ≥ 0,  Σp = S,   p_1^2 ≤ 4 p_0 p_2
d = 3 (n = 4):   p ≥ 0,  Σp = S,   p_1^2 ≤ 3 p_0 p_2,   p_2^2 ≤ 3 p_1 p_3
```

Both quadrics are tight at every point mass (`p = S·β(u)`), which is the
statement that the moment curve is the exposed boundary. ∎

**Corollary 7.6.7 (the reduction at `d ≤ 1`). [PROVED HERE]** At `d ∈ {0,1}`,
`H_d` is the nonnegative orthant, and `M_d = Δ`: the exact condition of Theorem
7.6.5 is `p ≥ 0, Σp = S` — **exactly the V1 simplex gate**, with nothing added.
*Proof.* For `d ≤ 1` Lemma 7.6.4 gives `H_d = {m ≥ 0}` (no quadric exists: the
range `1 ≤ r ≤ d−1` is empty), so the system is solvable for any `p ≥ 0` — and
constructively so, which is §7.1/§7.2's explicit `Q*` again: cell masses at
`d = 0`, knot atoms `Q* = Σ (p_i/S) δ_{t_i}` at `d = 1`. ∎

*This is the regression anchor.* Any admission stage that implements the
condition of Theorem 7.6.5, or any sound approximation of it that is exact at
`d ≤ 1`, is **the constant true** on every degree-0 and degree-1 market, so no
landed verdict moves.

#### 7.6.4 No finite family of linear price inequalities decides membership

**Theorem 7.6.8. [PROVED HERE]** For `d ≥ 2` and every admitted `n`, `M_d` is
not a polytope: its boundary contains a strictly convex arc, so `C_d` has
uncountably many exposed extreme rays and `K_d` is not finitely generated.
*Proof.* Take the single-span case first (Corollary 7.6.6): the boundary piece
`{p ≥ 0, Σp = S, p_1^2 = 4 p_0 p_2}` is the image of `u ↦ S·β(u)`, and
`p_1^2 − 4p_0p_2` is an irreducible quadratic form of signature `(1, 2)`, so
every point of the arc is an exposed extreme point (the tangent at `u` supports
`M_2` and touches only there). For `K > 2` restrict to measures supported in
one interior span: the same arc appears as a face of `M_d`. Extreme points of a
face are extreme points of the body. ∎

The design consequence is sharp and worth stating as a design fact rather than
a limitation discovered later:

> **An admission stage that is a finite conjunction of linear price
> inequalities can be *sound* (every refusal exhibits an executable arbitrage)
> or *complete* (every non-measure is refused), but never both.** A stage that
> is exactly `M_d` needs either the quadrics — available quantifier-free only
> on the single-span grids — or a witness in the candidate.

The relation takes **soundness of refusal**: it enforces a finite family of
exact *necessary* conditions, each of which is the no-arbitrage inequality of a
named portfolio. That is the same discipline the rest of the repository
enforces on rounding — a refusal must be attributable — and it costs no
lapses: a candidate refused by V1b is one against which a concrete admitted
position is a sure profit.

#### 7.6.5 The implemented family: ceiling, butterfly, single-span quadrics

**(G1) Ceiling certificates (window one).** The projection of `M_d` onto
coordinate `j` is `[0, max_x N_j(x)]`, and the separating certificate for a
violation is `c = a·1 − b·e_j` with `a/b ≥ max N_j`: **`a` complete sets short
`b` units of claim `j`** — the §7.4 split-and-sell position, at its general
index. Its payoff `a − b N_j(x) ≥ 0` everywhere; so its price
`(a·S − b·p_j)/S` must be nonnegative:

```text
(G1)     b_j · p_j  ≤  a_j · S,        a_j/b_j  ≥  max_x N_j(x).
```

The exact maxima of the open-clamped uniform basis, computed exactly (rational
arithmetic over the per-span polynomials of every admitted `(d, n)`; the
generator is committed at
`crates/clutch-batch/fixtures/generate_moment_cone_tables.py`, and the entries
are decide-checked against the model basis at every integer resolved value of
two grids in `lean/DragonsClutch/MomentCone.lean`):

```text
d = 2:  n = 3:      [1, 1/2, 1]
        n ≥ 4:      [1, 2/3, 3/4, 3/4, …, 3/4, 2/3, 1]
d = 3:  n = 4:      [1, 4/9, 4/9, 1]
        n = 5:      [1,  α , 1/2,  α , 1]
        n ≥ 6:      [1,  α ,  β , 2/3, …, 2/3,  β ,  α , 1]
        α = (18 + 8√2)/49 ≈ 0.5982390,   β = (33 + 18√2)/98 ≈ 0.5964879
```

`3/4` and `2/3` are the familiar interior peaks of §7.4; `1` at both ends is
Lemma 7.6.2. The two degree-three near-edge classes are irrational, so the
implemented bound uses `3/5` for both — sound because a *larger* ceiling only
weakens the certificate, and `3/5 ≥ α` ⟺ `57 ≥ 40√2` ⟺ `3249 ≥ 3200`, and
`3/5 ≥ β` ⟺ `129 ≥ 90√2` ⟺ `16641 ≥ 16200`. Everything else in the table is
exact, so at degree two `(G1)` is the exact window-one condition.

**(G2) Butterfly certificates (window three).** For an interior `j`, `k ≥ 0`
with `k(N_{j−1} + N_{j+1}) − N_j ≥ 0` on `X` gives the certificate
`c = k e_{j−1} − e_j + k e_{j+1}` — the **neighbour spread**, the exact analogue
of the classical butterfly no-arbitrage condition on option prices, and the
reason §7.3's "the hat claims *are* the butterflies" stops being trivial above
degree one:

```text
(G2)     p_j  ≤  k_j · (p_{j−1} + p_{j+1}),      k_j ≥ sup_x N_j/(N_{j−1}+N_{j+1}).
```

The suprema, same computation — exact at degree two, where they are integers,
and reported numerically at degree three, where they are algebraic numbers of
higher degree that the gate does not need to name (each one is *certified* by
checking a rational upper bound against the exact per-span polynomials, which
is what soundness requires):

```text
d = 2:  n = 3: 1        n ≥ 4: [–, 2, 3, 3, …, 3, 2, –]           (exact integers)
d = 3:  n = 4: ≈0.86603
        n = 5: [–, ≈1.58875, 1, ≈1.58875, –]
        n ≥ 6: [–, ≈1.55213, ≈1.47917, 2, …, 2, ≈1.47917, ≈1.55213, –]
```

The implemented weights are `1, 2, 3` at degree two (exact) and
`7/8, 8/5, 3/2, 2` at degree three (each a certified rational upper bound of
the class it covers; larger `k` is sound).

**(G3) Single-span quadrics.** On `n = d + 1` grids the two Hankel quadrics of
Corollary 7.6.6 are added, and there the stage is **exactly** moment-cone
membership.

**Theorem 7.6.9 (what the stage is). [PROVED HERE]** Let `V1b(p)` be the
conjunction of (G1), (G2) over all interior `j`, and (G3) where it applies.
Then:

1. **Sound refusal.** If `V1b(p)` fails then `p/S ∉ M_d`, and the violated
   member is an explicit portfolio with nonnegative payoff and negative price —
   executable in the admitted order language.  The violated member is bought
   for a negative outlay, so the sure profit is at least
   `b_j p_j − a_j S > 0` resp. `p_j − k_j(p_{j−1}+p_{j+1}) > 0` price units per
   unit position, on top of a payoff that is never negative.
2. **Off below degree two.** At `d ≤ 1`, (G1) is `p_j ≤ S` (Lemma 7.6.2 and
   `max N_j = 1` for hats and indicators — a hat attains 1 at its own knot),
   which V1 already enforces; (G2) is **empty**, because at a point where a
   hat or an indicator equals its maximum both neighbours vanish, so no finite
   `k` exists; (G3) does not apply. Hence `V1b ≡ true` and, by Corollary 7.6.7,
   it is also *exact* there.
3. **Strictly stronger than V1 above degree one.** `p = S e_j` for interior `j`
   fails (G1) at every admitted `(d, n)`, `d ≥ 2`.
4. **Exact on the single-span grids** (`n = d + 1`), by Corollary 7.6.6.
5. **Convex.** Each member is a convex constraint, so the admitted price set is
   a convex subset of `Δ_S` containing `M_d`. ∎

Sanity, and the reason the two families are worth having together: at the
degree-two interior peak the *true* moment vector `S·(⅛, ¾, ⅛)` sits on the
boundary of **both** — `4·(3/4)S = 3S` and `3·(⅛+⅛)S = ¾S` — so (G1) and (G2)
are simultaneously tight exactly where the moment curve touches, and a single
price atom moved onto the peak leaves the cone. The same holds at the
degree-three knot vector `S·(⅙, ⅔, ⅙)`.

#### 7.6.6 The exact window-three condition at degree two

The implemented (G2) is the *tangent* of a curved exact condition, and it is
worth recording what the exact one is, since it is the first available
tightening.

**Theorem 7.6.10. [PROVED HERE]** Let `j` be an index whose window
`{j−1, j, j+1}` sits in the interior-uniform region (all three functions of
full interior shape). The projection of `M_2` onto that window is exactly

```text
p_j ≤ p_{j−1} + p_{j+1} + 4·√(p_{j−1} p_{j+1}),
```

equivalently, in exact integers: `p_j ≤ p_{j−1} + p_{j+1}`, or
`(p_j − p_{j−1} − p_{j+1})^2 ≤ 16 p_{j−1} p_{j+1}`.

*Proof.* Certificates supported on the window are `c = (a, −b, a')`; writing
each span's restriction in that span's Bernstein basis, the binding span is the
one carrying all three functions, where the Bernstein coefficients are
`((a−b)/2, −b, (a'−b)/2)`, and the neighbouring spans give `a, a' ≥ 0`,
`a ≥ b`, `a' ≥ b`. A quadratic with nonnegative end coefficients and negative
middle is nonnegative on the span iff `B^2 ≤ AC`, i.e. `4b^2 ≤ (a−b)(a'−b)`.
Normalising `b = 1` and minimising `a p_{j−1} + a' p_{j+1}` over
`{(a−1)(a'−1) ≥ 4}` gives `p_{j−1} + p_{j+1} + 2·2√(p_{j−1}p_{j+1})` by AM–GM.
∎

At the peak `(⅛, ¾, ⅛)`: `¾ = ⅛ + ⅛ + 4·⅛`, equality — the linear `k = 3` is
its tangent at the symmetric point, and the two agree exactly there and
nowhere else. The condition is *not* implemented: its validity depends on the
window being interior-uniform, and the position-exact linear family covers
every window at both degrees with one uniform mechanism. Named as the first
tightening in §11.

#### 7.6.7 The residual, stated exactly

What the family does **not** catch: by Theorem 7.6.8 no finite linear family
can be complete, and the concrete gap is the certificates of **wide support**.
For `d = 2` and `n ≥ 5` there are extreme rays of `K_2` given by nonnegative
quadratic splines with `⌊(n−1)/2⌋` interior double zeros, positive elsewhere;
these have full support and are not nonnegative combinations of window-one and
window-three certificates. A price vector may therefore violate one of them —
be outside `M_d`, and carry an executable arbitrage — while passing V1b. The
arbitrage such a residual price admits is a *wide* position (it touches most
claims at once), and by the tightness computation above the residual region is
strictly inside the "obvious" violations. A concrete residual is now pinned by
`relation_v1_moment_cone_tests`: on the degree-two, five-claim grid with
breakpoints `[0,1,2,3]`, `p/S = (1/3,2/3,0,0,0)` passes V1b, while the
coefficient vector `(1,-2,10,40,64)` has the globally nonnegative payoff
`(3x-1)^2` and price `-S`. Thus the residual is nonempty; its complete size and
geometry remain open. `docs/design/PRICE_MEASURE_WITNESS_V2.md` specifies the
exact per-span Hausdorff witness that can close acceptance soundly.

The honest one-line status: **at degrees two and three the venue no longer
admits the §7.4 coordinate counterexample family, every V1b refusal exhibits an
executable arbitrage, the stage is exact at `d ≤ 1` and on the single-span
grids, and a named wide-support false acceptance proves that a witness is still
required for exact multi-span membership.**

---

## 8. What integrality, AON masks, and the tick grid do — the gap's shape

The LP of §2 is a relaxation of the venue's true feasible set, which is
integral (`f ∈ Z`), lot-quantized (portfolio fills in whole lots),
mask-disjunctive (AON/minimum-fill), and grid-priced (`p ∈ Δ_S ∩ Z^n`,
`c ∈ Z`). The gap between the two is not an amorphous "integrality gap"; it
has a precise, itemized shape [PROVED HERE, all via Corollary 4.3]:

```text
gap(p, f)  =  Σ_o s_o(p) · (q_o − f_o),        s_o(p) = eligibility distance,
```

with every possible source of positivity localized:

1. **Integrality of fills, single-Egg books: costs nothing.** The matrix is
   TU (§6.2), so integral optimal fills exist at every integral `p`; the
   canonical largest-remainder rationing moves fills only within the
   marginal set (`s = 0`), contributing `0` to the gap. The relaxation is
   exact.
2. **AON / minimum-fill (2b):** gap concentrates on the *unhonored obligated
   strict* orders, exactly `Σ q_o s_o(p)` over them; the honored branch
   `LP_M` is certificate-tight (§5.4). The uncertified object is the mask
   choice — a lattice search with no unique maximum (the design's 2-cycle) —
   i.e. the NP-hard all-or-nothing selection lives entirely in branch
   selection, never in branch verification. Under 2a the term is empty by
   admission.
3. **Whole-lot portfolios (P-a):** strict portfolios fill whole or the
   candidate refuses; marginal portfolios are excluded at `s = 0`. Lot
   quantization therefore never *pays* a gap — it converts would-be gap into
   refusal or into volume loss inside the indifferent set. Its real cost is
   candidate existence, not certified surplus.
4. **The tick grid:** for single-Egg books, free (TU puts an optimal dual on
   the grid whenever one is in the box). With portfolio columns, dual
   vertices can have denominators = subdeterminants (witness 3, §6.3), the
   grid can miss the whole optimal face, and the minimum achievable gap over
   grid candidates becomes a positive integer of price units — or, under
   allocation A, the book lapses. The gap per candidate remains exactly
   computable by the displayed formula in every case.
5. **Negative-dual books (§6.1):** under A, refusal (lapse), gap never
   materializes; under B, an accepted candidate whose *fills* may even be
   optimal while its certificate is loose — the one regime where the gap
   overstates true suboptimality, and provably cannot be repaired within the
   admissible price box.

One sentence of synthesis: **in this system the LP relaxation's error is
never diffuse; it is a sum of named, per-order, integer terms, and the
policy tuple decides whether each term is refused (A / 2a / P-a), certified
and accepted (2b branch, B with ε), or priced as a lapse.** That property —
attributable slack — is the same refusal-quality discipline the rest of the
repo enforces on rounding, here appearing in the optimality layer.

---

## 9. Consequences

### 9.1 If true — and it is, in the proven scope

For degree ≤ 1 markets cleared under (A, 2a, P-a, N-a/b):

- **One act.** The accepted candidate `(p, c, fills)` already *contains* the
  optimality certificate: `π = p`, `s` recomputed from eligibility (V2), the
  gap identically zero (V3's strict-full-fill), primal feasibility checked
  (V3/V4), the weak-duality inner product checked (V8). Publishing the batch
  result **is** publishing the certificate **is** publishing the implied
  measure `Q* = Σ (p_i/S) δ_{t_i}` at grid resolution. No second artifact,
  no post-processing.
- **No Breeden–Litzenberger inversion exists in the pipeline** because the
  claims are already butterflies (§7.3): density = `p_i/(S·g)`, implied
  forward = one portfolio quote `Σ t_i p_i / S`. The "implied distribution"
  product feature is a *read* of the clearing output, not a computation.
- **The certificate is machine-checkable with landed theorems.** The general
  soundness core (`weak_duality`, `certifies_epsilon_optimal`) is already
  proved in `Market/CertF.lean` for arbitrary matrices; what remains is
  instantiating our `A` (capped form, §4.3) and proving the accept-set
  theorem — Lean-authored, with Rust calling the artifact, per the standing
  substrate discipline. The certificate is linear in the witness (Cert-F
  class, not CertEq's bilinear class), hence also AIR-emittable in the
  `O(m + nnz A)` shape CertF §4 demonstrates, should the venue ever want the
  clearing proved on-chain.
- **Language upgrade available, gated.** "Best valid submitted candidate"
  can, for this tuple, be strengthened to "surplus-optimal for the divisible
  relaxation, certificate published" — after the §11 falsifiers, and never
  as "verified" before the Lean instantiation closes over the real
  definitions.

### 9.2 Where it is false or partial — the exact breakage map

| regime | what survives | what breaks |
|---|---|---|
| allocation B | measure-hood of `p` (d ≤ 1); ε-certificate with itemized ε | zero gap; "clearing = optimum" |
| AON 2b, unhonored obligated orders | branch-optimality certificate for `LP_M`; measure-hood | full-relaxation optimality; mask choice uncertified |
| one-sided outcomes (§6.1) | refusal discipline (A lapses; nothing false is published) | existence: no admissible dual at all; under B, loose certificates |
| portfolio off-grid duals (§6.3) | exact per-candidate gap accounting | zero gap on the grid [CONJECTURE]; a second lapse mode |
| degree ≥ 2 | `Σπ = S` (PoU is all the LP sees); the whole of §2–§6 | measure-hood of some V1-valid vectors; "hat = butterfly"; V1 as the no-arb gate |
| N-c self-crossing | LP + Hall-row extension with η duals (§5.6) | this document's accept-set theorem (unanalyzed interaction) |

### 9.3 The score selects the published measure

All valid allocation-A candidates being LP-optimal (Thm 5.1), the
lexicographic score is a *selection rule on the optimal face*: component 1
(dispersion-weighted direct volume) chooses among optimal duals/rationings,
component 3 (limit surplus) is constant across them only when the face is a
single objective level set — it is (they are all optima), so component 3 is
inert under A and live under B. The published implied measure therefore
carries a frozen, deterministic tie-break within the no-arbitrage band
(§5.3's interval). This should be said in any external description of "the
implied distribution": within the band the venue's number is a policy
choice, not a market revelation.

### 9.4 Fees

The relation excludes fees from eligibility and normalization by design
("fees never enter the normalization"), which is exactly what makes §2's LP
fee-free and Theorem 5.1 clean. A fee-inclusive LP would shift effective
limits by the fee schedule and re-derive; whether the dual then remains an
undistorted measure (or picks up a wedge, as in transaction-cost pricing
theory) is [OPEN] and is the mathematical face of the unfrozen fee-policy
question. Note one boundary effect already visible: a *marginal* buy under a
flat-notional fee pays value = limit *plus* fee, i.e. strictly above its
limit — individually rational only net of the liveness/participation value.
The LP sees none of this; honesty requires saying the certificate certifies
pre-fee surplus.

### 9.5 Solvency is untouched

Nothing here touches the kernel's solvency theorem: §3 of the distributional
design consumes only (H1)/(H2), and this document consumes its Theorem (ii)
(complete set worth exactly one unit) as the `±S` objective coefficients.
The two documents meet at PoU and nowhere else.

---

## 10. Verdict table

| # | claim | status |
|---|---|---|
| 1 | The clearing LP of §2 models V1–V4, V6, V8 exactly (exclusions named) | [PROVED HERE] (modeling argument, §1/§4.4) |
| 2 | Every dual-feasible `π` has `Σπ = S`; normalization = dual feasibility of σ/μ; PoU ⇒ normalization | [PROVED HERE] |
| 3 | Multiplier of the normalization row = net conversion `c`; collateral price = numéraire, fixed by the row itself | [PROVED HERE] |
| 4 | V8 cash closure = `π^T(Af) = 0` step of weak duality; `limit_surplus` = LP objective | [PROVED HERE] |
| 5 | Certificate soundness (gap ≤ ε ⇒ ε-optimal) over our matrix | [MACHINE-CHECKED ELSEWHERE] general matrix; instantiation paper-level (§4.3) |
| 6 | Accepted ⇒ zero-gap LP-optimal, `π = p` (tuple A/2a/P-a/N-a,b) | [PROVED HERE] (code-level pinning: falsifier §11) |
| 7 | Gap formula `Σ s_o(q_o − f_o)`; itemized; 2b = branch-tight; B = ε | [PROVED HERE] |
| 8 | Eligibility classes = dual-constraint trichotomy; StrictUnderfill = CS as refusal | [PROVED HERE] |
| 9 | Books with only negative optimal duals exist; allocation-A lapse there; §16.1 witness | [PROVED HERE] |
| 10 | Single-Egg matrix TU ⇒ grid costs nothing; lapse ⇔ dual face misses the box | [PROVED HERE at sketch rigor] (§6.2) |
| 11 | Portfolio books can force the dual face off the grid | [CONJECTURE] with subdeterminant + pinning evidence (§6.3) |
| 12 | `M_0 = M_1 = Δ`: every publishable `p` is a measure's moment vector, explicit `Q*` | [PROVED HERE] |
| 13 | Quadrature: `dot(c,p)/S = E_Q[interpolant]`, exact on the spline span; BL pre-inverted; forward = one quote | [PROVED HERE] |
| 14 | `M_d ⊊ Δ` for `d ≥ 2`; explicit executable arbitrage at `p = S·e_j`; V1 insufficient as no-arb gate | [PROVED HERE] / [REFUTED: the measure half at `d ≥ 2`] |
| 15 | "The dual is the measure," unconditionally, all degrees, all policies | [REFUTED] as stated; TRUE in the factored, scoped form of §0/§7.5 |
| 16 | `M_d` depends only on `(degree, outcome_count)` on admitted grids | [PROVED HERE] §7.6.1 |
| 17 | `p/S ∈ M_d` ⟺ no nonnegative-payoff portfolio is priced negative | [PROVED HERE] §7.6.2 |
| 18 | Exact membership = a per-span Hausdorff system, exact integers, `d ≤ 3` | [PROVED HERE] §7.6.3 |
| 19 | Exact and quantifier-free on single-span grids (`n = d+1`); reduces to the simplex gate at `d ≤ 1` | [PROVED HERE] §7.6.6, §7.6.7 |
| 20 | No finite family of linear price inequalities decides membership at `d ≥ 2` | [PROVED HERE] §7.6.8 |
| 21 | The landed V1b family: every refusal exhibits an executable arbitrage; off at `d ≤ 1`; incomplete above | [PROVED HERE] §7.6.9, residual §7.6.7 [OPEN] |

---

## 11. Open questions and named falsifiers

Open questions:

1. **Lean instantiation** of `Market.CertF` at the capped clearing matrix
   (§4.3), plus the accept-set zero-gap theorem over the real
   `relation_v1.rs` semantics — the promotion gate for any optimality
   language. Lean-authored; Rust calls the artifact.
2. **Existence, general books:** extend Theorem 6.2 beyond single-Egg
   (portfolio columns; N-c; 2b branches). Is lapse always "dual face misses
   the admissible set," or can canonical-derivation details refuse a book
   whose dual face is admissible?
3. **Off-grid dual face** (Conjecture 6.3): complete the pinning instance to
   a full refusing book, or prove portfolio books always admit a grid
   optimal dual (the subdeterminant evidence says they should not).
4. ~~**`d = 2` admission gate:** is `p/S ∈ M_2` decidable by per-pane integer
   discriminant conditions, and cheap enough to be a V1 extension if degree
   2 ever lands?~~ **Answered 2026-08-21, §7.6.** Yes to decidability (Theorem
   7.6.5: a per-span Hausdorff system in exact integers, `(d+1)(K−1) ≤ 60`
   unknowns), no to a quantifier-free *linear* form (Theorem 7.6.8: the cone
   has a strictly convex boundary arc). Landed as the V1b certified-refusal
   family (§7.6.5), and **bound on chain 2026-08-21**: the Epoch account
   carries the degree its immutable terms froze, and the clearing walk turns
   that byte into the descriptor `begin_with_basis` consumes, so a degree-2
   market's out-of-cone candidate is refused by the real program (bank
   evidence: `svm-tests/tests/cone_gate.rs`). Three successors remain open:
   **(4a)** the wide-support residual (§7.6.7) — construct a price vector that
   passes V1b, fails `M_d`, and carries an executable arbitrage, or prove the
   window families are complete on the admitted grids;
   **(4b)** the exact window-three condition (Theorem 7.6.10) as an
   implemented tightening, including its edge-window variants;
   **(4c)** the witness route — a candidate that *carries* the per-span
   moments of Theorem 7.6.5, making V1b exact at every grid in exchange for a
   wider candidate domain and a moved digest.
5. **Score-as-selection:** characterize which point of the optimal dual face
   the dispersion component selects; is it a recognizable center?
6. **Fee-inclusive dual** (§9.4): does the measure reading survive a fee
   wedge, and in what deformed form?
7. **N-c Hall duals:** exercise `η_{i,O}` on a book where H-i-O binds and
   read the congestion price off a worked example.

Falsifier names proposed for the lab (MODEL work, host-only, exhaustive over
tiny books, matching the §15 convention of the batch design):

```text
every_accepted_candidate_has_zero_duality_gap          # tuple A/2a/P-a/N-a,b; recompute s_o(p), gap == 0
gap_formula_matches_exhaustive_lp_oracle               # tiny books: gap == LP_opt - surplus, alloc B and 2b branches
one_sided_book_lapses_and_dual_is_negative             # §6.1 book: no valid candidate at any (p, c); LP oracle dual off-box
lapse_iff_no_boxed_optimal_dual_single_egg             # Theorem 6.2 both directions, exhaustive single-Egg books
dual_face_off_grid_book                                # Conjecture 6.3: construct or refute
simplex_vector_is_hat_moment_vector_deg01              # Q* atoms reproduce p exactly, incl. quantized w(t_i) = D e_i
deg2_simplex_vector_with_executable_arbitrage          # §7.4: split-and-sell realizes sure profit at p = S·e_j
                                                       #   LANDED as the relation's discriminating pair, clutch-batch
                                                       #   relation_v1_moment_cone_tests.rs
moment_cone_gate_is_the_constant_true_below_degree_two # §7.6.9(2)/Cor 7.6.7: the regression anchor — LANDED
single_span_gate_is_the_exact_hankel_condition         # §7.6.6: exactness at n = d+1 — LANDED
wide_support_certificate_passes_the_window_gate        # §7.6.7: the residual — OPEN, unbuilt
butterfly_identity_prices_match_hat_claims             # §7.3 second-difference identity on uniform grids, exact integers
v8_cash_identity_equals_price_dot_conservation         # §4.1: the two computations coincide on random valid candidates
limit_surplus_equals_lp_objective                      # Prop 4.2 over random valid candidates
```

---

## 12. Summary in one paragraph

Write the batch as the LP it is: fills in a box, one conservation row per
outcome, and the split/merge pair as two all-ones columns priced at `S` —
the only trace the basis leaves on the clearing, and it is exactly the
partition of unity. Dualize: the multipliers on the conservation rows are a
price vector whose sum is *forced* to `S` by the mere availability of
conversion; eligibility is the dual constraint trichotomy; the implemented
cash-closure check is the inner-product step of weak duality; the
`limit_surplus` accumulator is the LP objective; and under the
certificate-demanding policy tuple the `StrictUnderfill` refusal makes every
accepted candidate a zero-gap optimum with its own witness price as the
optimal dual. At degree ≤ 1 that vector is, always and explicitly, a
probability measure's basis-moment vector — a positive normalized quadrature
rule, with Breeden–Litzenberger already inverted because the hat claims are
butterflies — so clearing, certifying, and publishing the market's density
are one act. The boundary of the theorem is as sharp as its interior:
allocation B and unhonored AON masks trade the certificate for an exactly
itemized ε; one-sided books force the true dual negative and the venue,
rather than publish a non-measure, lapses; portfolio subdeterminants can
push the dual off the tick grid; and at degree ≥ 2 the simplex stops being
the no-arbitrage body and the measure half of the conjecture fails, with an
executable arbitrage as the witness.
