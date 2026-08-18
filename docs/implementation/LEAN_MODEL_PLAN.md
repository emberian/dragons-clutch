# The Lean model: architecture, claim shape, and theorem inventory

Status: **MODEL (proved, non-vacuous) for what §5 marks landed; PROPOSED for
everything else.** Written 2026-08-18. This document describes `lean/`, which
now contains a built, zero-`sorry` mathematical model of the kernel's semantic
plane and eleven proved theorems. It describes no verified implementation, no
release evidence, and no deployment.

Read with [`../ARCHITECTURE.md`](../ARCHITECTURE.md) §1 (the two planes),
[`../EVIDENCE_MATRIX.md`](../EVIDENCE_MATRIX.md) §6 and its dated addendum, and
[`DISTRIBUTIONAL_CLAIMS_DESIGN.md`](DISTRIBUTIONAL_CLAIMS_DESIGN.md) §3, whose
proof sketch this lane discharged and, in four places, corrected (§6).

---

## 1. The decision this document implements

**Properties are proven of the mathematical model, in Lean, independently of the
Rust Solana implementation.** Two implementations of the semantic plane — one in
Lean for proving, one in Rust for running — is the accepted cost. The
correspondence between them is bounded *empirically*, by the canonical semantic
vectors both evaluate. It is never claimed as proven.

This promotes Lean from "optional seam" (EVIDENCE_MATRIX §6 as written) to
**the primary home of semantic-plane theorems**. It does not demote Verus, which
remains the only tool that can say anything at all about the Rust source, and it
does not retire the Rocq shadow, which stays as written until someone decides
otherwise (§9.4).

### 1.1 The claim shape, verbatim

Every statement about this model, in commits, handoffs, filings, and
conversation, has this shape and no other:

> **Lean 4.33.0 checked theorem `T` about the model `M` in `lean/` at source
> digest `d`, under hypotheses `H`. `M` is a hand-written mathematical model of
> the kernel's semantic plane. Its correspondence to `crates/clutch-kernel` is
> manual, unproved, and bounded only by the semantic vectors both evaluate. No
> theorem in `M` is a statement about the Rust program, the compiled SBF ELF, or
> any deployed program.**

Sentences that are **false** and may never be written:

- "The kernel is verified in Lean." (`M` is not the kernel.)
- "Lean proves the Rust preserves solvency." (Lean proves it of `M`.)
- "The model is refined to the implementation." (There is no refinement proof
  and no formal semantics of Rust to refine into. See `AGENTS.md`.)
- "Vector agreement shows the model matches the implementation." (Vector
  agreement shows they agree **on the vectors**, on the facts a vector names.)
- "A green build is evidence." (A build with no theorem is no evidence; a
  theorem with a vacuous statement is worse than none.)

### 1.2 Why a model rather than more Verus

Verus proves properties *of the Rust source*, which is the strongest link in the
chain and the narrowest: it can only say local, per-function things about code
that already exists, and it inherits Rust's absent formal semantics at every
boundary it does not own. The properties this project's economics rest on —
"resolution can never breach solvency, over the whole frozen simplex lattice,
for every basis family that satisfies a partition of unity" — are statements
about a mathematical object with a quantifier over *all* admissible weight maps.
That is a theorem about mathematics, not about a function body, and it is
provable once, cheaply, in a model, or awkwardly and partially in a program
logic. Both layers stay; they answer different questions.

---

## 2. What is modelled, and what deliberately is not

The line is the plane boundary of `ARCHITECTURE.md` §1, drawn exactly.

| Modelled in Lean (semantic plane) | Deliberately not modelled (hostile integration plane, or out of scope) |
|---|---|
| Market state: bounded outcomes, conservative per-outcome supply, Hoard collateral, the resolution slot | Accounts, PDAs, rent, `TermsAccount`/`PriceGridAccount` layouts, CPI, Token-2022 |
| One claimant `Position` (internal and external balances) | The aggregate-versus-authenticated reconciliation an adapter owes |
| The ten transitions: `split`, `merge`, `materialize`, `dematerialize`, `resolve`, `resolveWithVector`, `redeem` (both sides), `redeemCompleteSet`, `transferInternal`, plus the constructor | Instruction dispatch, signer checks, account aliasing, atomic rollback |
| Payout vectors, the finite preset set, and the frozen common denominator `D` | Canonical codecs and byte images (`P-CODEC-01` stays a Verus/vector property) |
| **Admissibility** — (H1) and (H2) of `DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.1 | Knots, degree, panes, de Boor evaluation, edge policy, `WEIGHT-ROUND-01` (see §5.2 — the *next* theorem, not a permanent exclusion) |
| A **basis family** as a total weight map `X → PayoutVector` over an admitted value domain `X` | What makes a particular `x̂` the right resolved value: feeds, windows, evidence, the adapter's derivation |
| Exact integer arithmetic with an explicit stored-amount bound and refusal | Fixed-width wraparound (never modelled as wraparound; always as refusal), and `u128` intermediates except on the liability path (`P-PAY-01`, proved) |
| Refusal classes, mirroring `clutch_kernel::Error` one-for-one | The taxonomy *numbers* of `fixtures/vectors/TAXONOMY.json` (§7.3 owes a reviewed map) |
| — | The batch relation, the accumulator, fees, pools, liveness booking. Nothing about `P-BATCH-*`, `P-ACC-*`, `P-FEE-01`, `P-POOL-01`, `P-LIVE-01` is modelled yet (§5.3) |

The exclusions are not a backlog of things that "should" be in Lean. The
integration plane belongs in Rust, bounded by vectors and adversarial tests,
because there is nothing mathematical about a Solana account and modelling one
would produce a fiction with a proof attached.

---

## 3. Model architecture and its costs

`lean/` is a dependency-free Lake package. Six modules:

```text
lean/DragonsClutch/Basic.lean        amounts, ceiling division, dot, max, uniform shifts
lean/DragonsClutch/Basis.lean        payout vectors, admissibility (H1)/(H2), weight maps, preset sets
lean/DragonsClutch/Solvency.lean     the liability functionals and the central theorems
lean/DragonsClutch/Kernel.lean       market state, position, the ten transitions
lean/DragonsClutch/Transitions.lean  the transition-level theorems
lean/DragonsClutch/Vectors.lean      two canonical vectors, evaluated and checked at build time
```

Five representation decisions, each with the cost stated:

1. **Amounts are `Nat` with an explicit `amountMax = 2^64 − 1` checked at every
   write.** Fixed-width behaviour is modelled as *refusal*, never as
   wraparound. Cost: the model cannot exhibit a wraparound bug, so it cannot
   catch one; that is a Verus/vector obligation. Benefit: intermediate
   arithmetic is exact, and the `u128` headroom question becomes a theorem
   (`P-PAY-01`) rather than an assumption.
   `Amount` is *notation* for `Nat`, not an `abbrev`: `omega` collects no
   constraints from hypotheses whose type is a definitional alias.
2. **Vectors are `List Nat` of length `n`** — the active prefix of the Rust's
   zero-padded fixed arrays. Length agreement is an explicit hypothesis
   everywhere it matters. Cost: the fixed-array padding discipline (`weights[i]
   = 0` beyond the active prefix) is not modelled and remains a Rust-side
   validation checked by vectors.
3. **The resolution slot is one inductive value** (`active | byIndex i |
   byVector v`), not the Rust's `(phase, resolved_payout, resolved_vector)`
   triple plus a `validate_resolution` guard. "One resolution seam per mode,
   never both" is therefore true by construction. **This is a real cost and is
   named as an obligation:** the model cannot express the state
   `validate_resolution` exists to refuse, so it can never catch a defect in
   that function. Vectors and Verus own it.
4. **Transitions are total functions into `Except Error`**, with `Error`
   mirroring `clutch_kernel::Error` one-for-one, and the guard *order* mirroring
   the Rust's (R8's collateral-before-balance in `merge`, and its deliberate
   inversion in `redeemCompleteSet`). Refusal is a value. Cost: the model
   asserts an error-class correspondence that only vectors can check.
5. **Every predicate is decidable and the model computes.** `Shape`, `Solvent`,
   `Admissible`, `Valid` are `Prop`s built from decidable pieces, so the same
   definitions serve the theorems and the evaluator. `Vectors.lean` runs two
   canonical vectors through the model with `#guard`, which fails the build if
   the model disagrees. This is what makes §7 a design and not a rewrite.

### 3.1 No Mathlib, deliberately

The model imports nothing but Lean 4 core. Reasons, in order: the repository is
offline-first (`adr/0004`); a proof closure whose trusted base is "the Lean
kernel and nothing else" is a smaller thing to pin than one carrying a
3.2 GB library at a moving revision; and the mathematics needed so far is
elementary integer arithmetic that `omega` and twenty lines of induction handle.

The decision has a named expiry: **the first `P-BATCH` theorem needs it.**
`BATCH_RELATION_V1_DESIGN.md` §8.2's pairing feasibility is a Hall/max-flow
argument, and mechanizing max-flow-min-cut and integrality from scratch is not a
side quest. When that lane starts, the choice is (a) add Mathlib and pin its
revision in `toolchain/PINNED_PROOF_TOOLS.md`, or (b) prove the *specialized*
statement the relation actually checks — that the single-forbidden-partner
matching completes iff (H-i-O) holds — directly, which is plausibly a few
hundred lines and keeps the closure small. Decide it deliberately, in that
lane, not by drifting into an `import Mathlib`.

---

## 4. Toolchain pin

Recorded as installed and used for the build in §5. This is a pin, not a
verification result.

| Field | Value |
|---|---|
| `elan` | 4.2.1 (3d5138e15 2026-03-18) |
| Lean | 4.33.0, `arm64-apple-darwin24.6.0` |
| Lean commit | `d8b18978322de05a8f3dba51ef03cf5461676c17` |
| Lake | 5.0.0-src+d8b1897 |
| `lean` binary sha256 | `1b370cfcbf44e80d1b004ab1b1ab9a4c73951f9f7c242140bcff9bc577576554` |
| `lake` binary sha256 | `58261a1a2fa1a362376c71e02ca854a093e71cc5e6ea64b287a931cb2565273d` |
| Toolchain prefix | `~/.elan/toolchains/leanprover--lean4---v4.33.0` |
| Pin file | `lean/lean-toolchain` = `leanprover/lean4:v4.33.0` |
| Dependencies | none — `lean/lake-manifest.json` has `"packages": []` |
| Platform | aarch64-apple-darwin (Darwin 25.6.0) |

`toolchain/PINNED_PROOF_TOOLS.md` has no Lean section. It should gain one with
these values; this lane did not edit that file because it is outside its scope.

Reproduce:

```sh
cd lean && lake build          # zero errors, zero warnings, ~4s clean
```

Axiom audit (the only acceptable result is the three standard Lean axioms):

```sh
cd lean && echo 'import DragonsClutch
#print axioms DragonsClutch.P_SOLV_01_resolution_bound' > /tmp/ax.lean \
  && LEAN_PATH=.lake/build/lib/lean lean /tmp/ax.lean
```

Forbidden-construct audit:

```sh
grep -rn "sorry\|axiom\|native_decide\|unsafe\|@\[implemented_by\]" lean/DragonsClutch/
```

---

## 5. Theorem inventory

### 5.1 Landed — proved, checked, non-vacuous

All 86 theorems in the package depend on no axioms beyond `propext`,
`Classical.choice`, and `Quot.sound` (Lean's own logical axioms; there is no
project axiom, no `sorry`, no `native_decide`). Ranked by value.

| # | Theorem | ID | Statement |
|---|---|---|---|
| 1 | `P_SOLV_01_resolution_bound` | `P-SOLV-01` | for every supply `T` and every admissible `v`: `requiredResolved T v ≤ requiredActive T`. Hypothesis: `v.Admissible n` — `0 < D`, `n` weights, `Σ w = D`. Nothing else. |
| 2 | `P_SOLV_01_sup_bound` | `P-SOLV-01` | the same bound at **every** admissible value `x : X` of a whole basis family `B : WeightMap X n D`, i.e. it bounds the supremum over the admitted value domain. |
| 3 | `P_SOLV_01_required_active_is_exact_sup` | `P-SOLV-01` | `max_i T_i` bounds *and is attained* over the frozen simplex lattice — the exact supremum, not a chosen over-reservation (design claim (iv), corrected: see §6.2). |
| 4 | `P_SOLV_01_resolve_with_vector_admits` | `P-SOLV-01` | a shaped, solvent, Active, derived-basis market **always accepts** an admissible vector and lands solvent. The prospective invariant check inside `resolveWithVector` is defence in depth, never a live refusal. |
| 5 | `P_PAY_02_complete_set_never_stranded` | `P-PAY-02` (proposed) | a holder of `q` of every outcome in a shaped, solvent, resolved market **always** redeems the complete set and is paid exactly `q`. Neither the remainder refusal nor the collateral refusal can fire, in either mode, at any resolved value. |
| 6 | `P_PAY_01_liability_fits_u128` | `P-PAY-01` | with supplies and `D` bounded by `u64::MAX`, the liability numerator is `≤ (2^64−1)^2 < 2^128`. The partition of unity is what makes the kernel's `u128` accumulator unable to overflow (§6.4). |
| 7 | `P_SOLV_01_split_admits`, `P_SOLV_01_merge_admits` | `P-SOLV-01` | design claim (iii): the requirement and the collateral both move by **exactly** `q`; the only refusals left on those paths are the fixed-width bounds and the balance tests. |
| 8 | `R8_merge_collateral_refusal_is_ordering_artifact` | `P-SOLV-01` | in a shaped solvent market whose supplies are all `≥ q`, `q ≤ collateral`. The `insufficientCollateral` that `merge` can report is reachable only in states where the balance test also fails — the prose claim in `kernel-merge-reports-collateral-before-balance`'s `precedence_note`, now proved. |
| 9 | `P_SOLV_01_*_lands_solvent` (8 transitions) | `P-SOLV-01` | every accepted transition lands in a shaped, solvent state. Stated about the **post** state (see §6.5). |
| 10 | `P_SUP_01_materialize_market_unchanged`, `..._dematerialize_...` | `P-SUP-01` | the materialization boundary returns the market unchanged: supply and collateral are untouched. |
| 11 | `P_SUP_01_transfer_conserves` | `P-SUP-01` | a transfer preserves the two positions' summed holding of the transferred outcome, and touches no market state at all. |
| — | `P_PAY_02_complete_set_liability_exact`, `..._required_exact` | `P-PAY-02` | the arithmetic core of (5): `Σ_i q·w_i = q·D` exactly, at every admissible vector. |
| — | `PayoutVector.Admissible.bounded` | — | (H1)'s upper half derived from (H2) (§6.1). |

### 5.2 Next, ranked

1. **`WEIGHT-ROUND-01` admissibility** (`P-PART-02`-adjacent, design §2.3).
   "Floor all but the last basis weight, set the last to `D − Σ`" produces an
   admissible vector, for every degree, every knot grid, every `x̂`. This is the
   one place the design says the thesis could break, and it is the last
   unproved link between "the B-spline construction" and "(H1)+(H2)", which is
   all any theorem above needs. Estimated small: sum-of-floors ≤ floor-of-sum,
   then induction.
2. **Redeem exactness and requirement bookkeeping** (`P-PAY-01`): a successful
   single-outcome redemption pays exactly `q·w_i/D` and lowers the requirement
   by exactly the payment — the exact form of design claim (iii)'s "at least"
   (§6.3).
3. **A reachability invariant** (`P-SOLV-01`, the Rocq gate's ask): every state
   reachable from `Market.new` by any transition sequence is shaped and solvent.
   The per-transition theorems compose into it; the statement is worth having as
   one named theorem because that is the sentence a reviewer wants.
4. **Total categorical supply** (`P-SUP-01`, sharper): `internal + external` per
   outcome is preserved by materialize/dematerialize *within a position*, and
   the model-level statement of `total_i = internal + accounted_external`.
5. **Partition compilation** (`P-PART-01`, `P-PART-02`): ordered, exhaustive,
   disjoint numeric partitions and unique cell selection. Currently modelled
   nowhere in Lean; it is the other half of the "semantic plane" and it is
   finite, small, and provable.
6. **`P-BATCH-02` conservation** over a finite relation, then `P-BATCH-01`
   limits and `P-BATCH-05` deterministic ordering. `P-BATCH-*` is where the
   Mathlib decision in §3.1 gets made.

### 5.3 Not planned in Lean

`P-CODEC-01` (bytes are a Rust/Verus property), `P-POOL-01` (account planes),
`P-ACC-02` monotonicity of cursors as an *adapter* property, and anything whose
statement needs an account, a clock, or a byte.

---

## 6. Findings against `DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.2

Proving the sketch found no error in its conclusions. It found four places
where the stated hypotheses or the stated strength are wrong in a way that
matters downstream, and one that is a genuine addition.

### 6.1 (H1)'s upper bound is not a hypothesis — it is a consequence

§3.1 lists `0 ≤ w_i(x̂) ≤ D` as hypothesis (H1). Over unsigned weights, `w_i ≤ D`
follows from (H2) alone: `w_i ≤ Σ_j w_j = D`. Proved as
`PayoutVector.Admissible.bounded`. Further, the proof of claim (i) never uses
it: the bound it needs is `T_i ≤ max_j T_j`, which is the definition of the
maximum, not a property of the weights. §3.2's sentence "By (H1) every product
`T_i·w_i(x̂)` is nonnegative and bounded by `(max_j T_j)·w_i(x̂)`" uses (H1) only
for *nonnegativity*, which is structural in `u64`/`Nat`.

Consequence, and why it is worth recording: the kernel's per-weight
`weight > denominator` check in `PayoutVector::validate` is **redundant given
the exact sum check**, on any unsigned encoding with checked addition. It is
sound defence in depth against a malformed encoding and should stay. But §10's
propagation of "(H1) and (H2)" into the Rocq/Verus obligations and the filings
language should say what is actually assumed: *nonnegativity (structural) and
the exact sum*. One hypothesis, not two.

### 6.2 Claim (iv) is stated over the wrong index set for the claim the kernel makes

§3.2 (iv) says `required_active` is the exact supremum "whenever the basis
attains a one-hot vector (deg 0 everywhere; deg 1 at every knot and clamped
edge)", and that for `d ≥ 2` it is "a sound over-reservation".

Both halves are true of the supremum over the *image of the basis*. But the
kernel's mode-1 Active requirement is not indexed by `x̂` at all: §3.3 is
explicit that the kernel checks shape and not provenance, so the set the
requirement must cover is the whole frozen simplex lattice
`{w ∈ Z^n : w_i ≥ 0, Σ w_i = D}` — every vector `resolve_with_vector` will
accept. Over that lattice the one-hot vectors are admissible, so the supremum
**is attained, for every degree**, including `d ≥ 2`. That is
`P_SOLV_01_required_active_is_exact_sup`.

Why it matters: "for `d ≥ 2`, `max_i T_i` is a sound over-reservation" invites
the optimization "so a `d ≥ 2` market could reserve less." It could not — not
without the kernel learning the basis, which its charter forbids. The tightness
statement should be split in two: over the lattice (exact, all degrees, and this
is the kernel-alone claim), and over a particular basis image (attained for
`d ≤ 1`, slack for `d ≥ 2`, and this is a *system* claim that depends on the
adapter's derivation being the only source of vectors).

### 6.3 Claim (iii) is exact, not an inequality

§3.2 (iii) says resolved redemption makes "the ceiling-rounded requirement fall
by at least the payment". It falls by **exactly** the payment: exact-or-refuse
redemption moves the numerator by exactly `payment · D`, and
`⌈(N − pD)/D⌉ = ⌈N/D⌉ − p`. Same for `split` and `merge`: the requirement moves
by exactly `q` in both modes (`required_bumpAll`, `required_dropAll`).

"At least" is not wrong, it is loose, and the loose form hides the property
worth having: the market never *accumulates* requirement slack across
operations. A protocol whose reserve drifted upward by a ceiling remainder on
every operation would be quietly over-collateralizing forever, and "at least"
does not exclude it.

### 6.4 The `u128` headroom is a corollary of the partition of unity, and nobody had said so

Not in the design at all; it is open obligation 6 of
[`ROCQ_SPEC_STATUS.md`](ROCQ_SPEC_STATUS.md) ("all Rust `u128` checked-product
and checked-sum bounds are represented").

`MarketState::required_for_vector` accumulates `Σ_{i<16} T_i·w_i` in a checked
`u128`. Per-term, `(2^64−1)^2 < 2^128`, so `checked_mul` cannot fire. The
*sum*, bounded naively, is `16·(2^64−1)^2 ≈ 2^132`, which does **not** fit — so
`checked_add` returning `ArithmeticOverflow` is not obviously unreachable, and
if it ever fired, `check_invariants` would fail and *every* transition on that
market would refuse: a bricked market. The partition of unity is exactly what
rules it out: `Σ_i T_i·w_i ≤ (max_i T_i)·D ≤ (2^64−1)^2 < 2^128`
(`P_PAY_01_liability_fits_u128`).

Recommendation: add this to §3 as claim (v), and note the fragile direction —
the width argument depends on (H2), not on the per-field `u64` typing. Anyone
who relaxes the sum rule must redo it.

### 6.5 A defect not repeated

`VECTOR_SPINE_PROPOSAL.md` §2.7 ROCQ-5 records that the Rocq model's
`successful_transition_is_well_formed` states
`∀ s o, resolve s o = Some s → …`, binding the *input* state where the output
was intended — a vacuously satisfiable obligation. Every Lean theorem of the
`lands_solvent` family is stated as `… = .ok m' → m'.Shape ∧ m'.Solvent`, about
a *distinct* post-state variable. The Rocq defect is not repeated here, and the
Lean statements are the shape `P-SOLV-01` vectors should reference.

Two further honesty notes on that family: those eight theorems are the *weak*
half of `P-SOLV-01` — each transition checks its own prospective invariant, so
the direction is close to the definition. The substantive statements are the
`_admits` family (4, 5, 7 in §5.1), which say the check **never fires**, and
those are where the partition-of-unity theorem does real work.

---

## 7. Lean as an executor column in the vector spine

`fixtures/vectors/*/*.json` already carries a `lean-checker` executor on every
vector, currently `{"mode": "pending", "blocked_by": "optional per
EVIDENCE_MATRIX.md#6"}` (22 pending, 3 not-applicable). This section designs
how that column gets filled. **It is a design; nothing in it is built this
round.** `lean/DragonsClutch/Vectors.lean` hand-transcribes two kernel vectors
and checks them with `#guard` at build time, which demonstrates the model
evaluates, and is explicitly *not* the checker: a hand transcription checks the
model against a reading of the manifest, not against its bytes.

### 7.1 Shape

```text
lean/DragonsClutch/Checker/Json.lean     decode the §3.3 manifest schema (Lean core `Lean.Data.Json`)
lean/DragonsClutch/Checker/Forms.lean    `kernel.market-position/v1` -> Market x Position
lean/DragonsClutch/Checker/Codes.lean    Error -> taxonomy code, one total function
lean/DragonsClutch/Checker/Run.lean      operation dispatch, disposition, report
lean/Main.lean                           `lake exe clutch-lean-check --root ../fixtures/vectors`
```

Dependency direction matches `fixtures/vectors/README.md`: the checker depends
on the vectors and on the model; nothing depends on the checker; the model gains
no edge from it. `Lean.Data.Json` is in the Lean distribution, so the package
stays dependency-free (§3.1). The `Main.lean` executable is the only place the
model touches `IO`.

### 7.2 Rules, in the style of §2.7's ROCQ-1…5

- **LEAN-1.** `Except.ok v` compares against `result_kind: ok` **and against the
  named success value**; `Except.error e` compares against a declared error
  code through §7.3's map. Unlike the Rocq shadow, the Lean model can carry a
  code, so its target capability is **exact**, not refusal-only.
- **LEAN-2.** Until §7.3's map is reviewed, the checker runs in **refusal-only**
  mode and says so in the report. A code map invented by an implementer is a
  parallel truth (finding A.4.2/A.4.3 of the spine addendum), and this lane will
  not create a second one.
- **LEAN-3.** The model has no byte plane and no account plane: every vector
  requiring `byte_exact` or naming a 4xxx account code gets `not-applicable`
  with reason `no-byte-plane` / `no-account-plane`, never `skipped`.
- **LEAN-4.** The model has no fixed-width representation, so a vector that
  exists to pin *wraparound* behaviour is `not-applicable` with a reason token
  that does not exist yet and must be added: `no-fixed-width-plane`. (Today no
  such vector exists; the kernel refuses instead of wrapping.)
- **LEAN-5.** The checker recomputes `digests.vector`, `digests.manifest`, and
  `digests.taxonomy` exactly as `tools/vector-check` does (SHA-256 over RFC 8785
  canonical JSON) and refuses a placeholder (DIG-5). Two independent
  canonicalizer implementations disagreeing is itself a finding.
- **LEAN-6.** The checker may never edit a vector, and a Lean/Rust disagreement
  is triaged as an implementation defect, a model defect, or a vector defect —
  never as a checker exception. There is no exception field and none will be
  added.
- **LEAN-7.** A vector the model cannot *express* (a form with no model
  analogue) is a `pending` with a named blocker, counted in the ratio the report
  prints on every run.

### 7.3 The one thing that must be decided first

`Error -> taxonomy code` is a **reviewed table**, not a function an implementer
writes. `clutch_kernel::Error` has 16 variants; `TAXONOMY.json` defines 152
codes of which 26 have a vector. The Lean map must reproduce the same
`code()` the Rust surface uses, and the spine addendum's finding A.4.2 — that
`CodecError::code()` already extends a PROPOSED registry from inside a crate —
is the reason this table is a review item and not a convenience. Until it is
reviewed, LEAN-2 holds.

### 7.4 What this column is worth, stated honestly

A green Lean run means: an independently written model, whose theorems are
proved, agrees with the Rust implementation on the facts these vectors name.
It is the **only** empirical bound on the model-to-Rust correspondence, and it
is exactly as strong as the vector set is wide — today, 26 of 152 codes and 25
vectors. It is not a refinement, not a proof, and not cross-runtime agreement
about anything a vector does not name.

---

## 8. What would make this effort fail

Stated plainly, so that failure is recognizable rather than gradual.

1. **The model drifts from the Rust and the vectors do not catch it.** This is
   the central risk of the whole decision. Two implementations only stay honest
   if a wide, growing vector set binds them. If the kernel gains a transition,
   a refusal, or a check-order change and no vector covers it, the Lean
   theorems keep building and start describing a system nobody runs. The
   mitigation is structural: a kernel change without a vector is the defect, and
   the checker's printed coverage ratio is the alarm. **If, six months on, the
   model has theorems about transitions the vectors never exercise, this lane
   failed.**
2. **The model becomes a transcription of Rust control flow.** The value here is
   that the theorems are about *mathematics* — a quantifier over all admissible
   weight maps, not a walk through one function's branches. A model that mirrors
   `if` for `if` proves the code's shape, not the protocol's properties, and
   inherits every one of the implementation's accidents. The tell is a theorem
   whose statement needs the reader to know the Rust.
3. **Theorems get weakened to make them provable.** A `lands_solvent` theorem
   whose hypothesis already assumes the conclusion, a vacuous `P → P`, an
   obligation stated about the input state (ROCQ-5) — all build green. The gate
   is an adversarial read of the *statements*, and the standing rule from this
   lane: a named obstruction is worth more than a weakened theorem.
4. **The claim shape slips.** The first time a summary says "the kernel is
   verified in Lean", the whole apparatus becomes a marketing artifact. §1.1 is
   the antidote and must be quoted, not paraphrased.
5. **Toolchain or dependency drift.** An unpinned Lean, or an `import Mathlib`
   added for one lemma, converts a 4-second reproducible offline build into
   something nobody can reproduce in a year. §3.1 and §4.
6. **The model outgrows its reviewers.** Six modules and 86 theorems are
   readable in an afternoon. Sixty modules of unread proof are indistinguishable
   from no proof at all. Growth should be theorem-driven — each new module
   justified by a property ID somebody asked for.

---

## 9. Open items this lane did not close

1. `toolchain/PINNED_PROOF_TOOLS.md` has no Lean section (§4 has the values).
2. `EVIDENCE_MATRIX.md` §2's table still says "optional" / "optional reproduce"
   in the Lean column for `P-SOLV-01`, `P-SUP-01`, and `P-PAY-01`. The dated
   addendum to §6 records the promotion; the table itself is the coordinator's
   edit, not this lane's.
3. `P-PAY-02` (complete-set exactness) is a **proposed new property ID**. The
   matrix has no row for the property that a complete set redeems for exactly
   `q` at every admissible resolved value, and it is the single most
   user-visible protocol guarantee.
4. The Rocq shadow (`rocq/ClutchKernel.v`) is unchanged and unchecked (no
   `coqc` on this machine). Whether it stays as a second independent model, is
   retired in favour of Lean, or is kept as the extraction oracle it was
   designed to be, is a decision for the coordinator, not a side effect of this
   lane. Its ROCQ-5 defect is recorded in §6.5 and not repeated here.
5. `DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.2 should absorb §6.1–§6.4. This lane
   did not edit that document.
