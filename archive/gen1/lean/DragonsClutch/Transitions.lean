import DragonsClutch.Kernel
/-!
# Transition-level theorems

The properties of `docs/EVIDENCE_MATRIX.md` §2, restated over this model's
transitions and proved.  Each theorem's docstring names its property ID and the
design claim it discharges.

The two shapes of statement, and why both are here:

* **"a successful transition lands solvent"** is the weaker half.  In this
  model — as in the Rust — every transition checks the prospective invariant
  before returning, so this direction is close to the transition's own
  definition.  It is stated anyway, because it is what a reader of the evidence
  matrix expects `P-SOLV-01` to say, and because it pins that the check is on
  the *post* state.
* **"the check never fires"** is the substantive half.  It says the prospective
  invariant check is defence in depth and not a live refusal: a solvent
  pre-state plus an admissible input *always* succeeds.  That direction is where
  the partition-of-unity theorem does real work, and it is the statement a
  design reviewer needs, because a live refusal here would be a market that
  cannot resolve.
-/

namespace DragonsClutch
namespace Market

/-! ## Inversion helpers -/

theorem checkInvariants_ok {m : Market} (hshape : m.Shape) (hsolv : m.Solvent) :
    m.checkInvariants = .ok () := by
  simp [checkInvariants, hshape, hsolv]

@[simp] theorem checkInvariants_ok_iff {m : Market} {u : Unit} :
    m.checkInvariants = .ok u ↔ m.Shape ∧ m.Solvent := by
  unfold checkInvariants
  by_cases hs : m.Shape <;> by_cases hv : m.Solvent <;> simp [hs, hv]

theorem requireActive_ok {m : Market} (h : m.resolution = Resolution.active) :
    m.requireActive = .ok () := by
  simp [requireActive, h]

/-! ## P-SOLV-01: solvency across the resolution seam -/

/-- **P-SOLV-01 / claim (i), transition form — the substantive direction.**

A derived-basis market that is Active, well-shaped, and solvent *always*
accepts an admissible payout vector: `resolveWithVector` returns `ok`, and the
resulting Resolved market is solvent.

This is the theorem that makes the design's sentence "the prospective invariant
check inside the resolve transition is defence in depth, not a live refusal"
true rather than hoped.  Its whole content is `P_SOLV_01_resolution_bound`:
the Active requirement `max_i T_i` dominates the resolved requirement
`⌈Σ_i T_i·w_i / D⌉` at every point of the frozen simplex lattice, so collateral
that covered the Active phase covers every resolution.

Every hypothesis is explicit: shape, solvency, Active phase, derived-basis
mode, (H1)+(H2) for `v` over the market's active outcomes, and `v`'s
denominator being the market's frozen `D`.  No hypothesis about knots, degree,
evidence, or how `v` was derived — the kernel checks shape, not provenance. -/
theorem P_SOLV_01_resolve_with_vector_admits {m : Market} {v : PayoutVector}
    (hshape : m.Shape) (hsolv : m.Solvent)
    (hactive : m.resolution = Resolution.active)
    (hmode : m.basisMode = BasisMode.derivedBasis)
    (hv : v.Admissible m.outcomes)
    (hD : v.denominator = m.payouts.denominator) :
    m.resolveWithVector v = .ok { m with resolution := .byVector v } := by
  have hshape' : ({ m with resolution := .byVector v } : Market).Shape := by
    obtain ⟨h1, h2, h3, h4, h5, _⟩ := hshape
    refine ⟨h1, h2, h3, h4, h5, ?_⟩
    simp only [ResolutionOk, hmode]
    exact ⟨hv, hD⟩
  have hreq : m.required = some (requiredActive m.totalSupply) := by
    simp [required, hactive, hmode]
  have hsolv' : ({ m with resolution := .byVector v } : Market).Solvent := by
    have hle : requiredResolved m.totalSupply v ≤ requiredActive m.totalSupply :=
      P_SOLV_01_resolution_bound hv
    have hcov : requiredActive m.totalSupply ≤ m.collateral := by
      unfold Solvent at hsolv
      rw [hreq] at hsolv
      exact hsolv
    unfold Solvent
    simp only [required]
    exact Nat.le_trans hle hcov
  rw [hmode] at hshape' hsolv'
  simp [resolveWithVector, checkInvariants_ok hshape hsolv, requireActive_ok hactive,
    hmode, hD, hv, checkInvariants_ok hshape' hsolv', bind, Except.bind]


/-! ## Structural helpers -/

/-- A resolved market's requirement is the requirement of the vector it pays
from, whichever seam installed it. -/
theorem required_of_resolved {m : Market} {v : PayoutVector}
    (h : m.effectiveVector = some v) :
    m.required = some (requiredResolved m.totalSupply v) := by
  unfold effectiveVector at h
  unfold required
  split at h
  · simp at h
  · next i heq => rw [h]
  · next w heq => simp at h; rw [h]

/-- The vector a well-shaped resolved market pays from is admissible over its
active outcomes and carries the frozen denominator — in either mode.  In mode 0
this comes from `PayoutSet.Valid`, in mode 1 from the resolution slot's own
validation. -/
theorem effectiveVector_admissible {m : Market} {v : PayoutVector}
    (hs : m.Shape) (h : m.effectiveVector = some v) :
    v.Admissible m.outcomes ∧ v.denominator = m.payouts.denominator := by
  obtain ⟨harity, hvalid, _, _, _, hres⟩ := hs
  unfold effectiveVector at h
  unfold ResolutionOk at hres
  split at h
  · simp at h
  · next i heq =>
      have hmem : v ∈ m.payouts.vectors := List.mem_of_getElem? h
      obtain ⟨_, _, _, _, hadm, hcom⟩ := hvalid
      exact ⟨harity ▸ hadm v hmem, hcom v hmem⟩
  · next w heq =>
      have hvw : v = w := by simpa using h.symm
      subst hvw
      rw [heq] at hres
      cases hmode : m.basisMode <;> rw [hmode] at hres <;> simp at hres
      exact hres

/-! ## P-SOLV-01: every accepted transition lands solvent

The weak half of `P-SOLV-01`, over the whole transition surface.  Each proof is
the same inversion: the transition's own prospective check is the last thing it
does, so acceptance carries the post-state invariant.  Stated per transition so
that the evidence matrix row can name them. -/

theorem P_SOLV_01_split_lands_solvent {m m' : Market} {p p' : Position} {q : Amount}
    (h : m.split p q = .ok (m', p')) : m'.Shape ∧ m'.Solvent := by
  unfold split at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

theorem P_SOLV_01_merge_lands_solvent {m m' : Market} {p p' : Position} {q : Amount}
    (h : m.merge p q = .ok (m', p')) : m'.Shape ∧ m'.Solvent := by
  unfold merge at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

theorem P_SOLV_01_materialize_lands_solvent {m m' : Market} {p p' : Position} {i : Nat}
    {q : Amount} (h : m.materialize p i q = .ok (m', p')) : m'.Shape ∧ m'.Solvent := by
  unfold materialize at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

theorem P_SOLV_01_dematerialize_lands_solvent {m m' : Market} {p p' : Position} {i : Nat}
    {q : Amount} (h : m.dematerialize p i q = .ok (m', p')) : m'.Shape ∧ m'.Solvent := by
  unfold dematerialize at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

theorem P_SOLV_01_resolve_lands_solvent {m m' : Market} {i : Nat}
    (h : m.resolve i = .ok m') : m'.Shape ∧ m'.Solvent := by
  unfold resolve at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

theorem P_SOLV_01_resolve_with_vector_lands_solvent {m m' : Market} {v : PayoutVector}
    (h : m.resolveWithVector v = .ok m') : m'.Shape ∧ m'.Solvent := by
  unfold resolveWithVector at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

theorem P_SOLV_01_redeem_lands_solvent {m m' : Market} {p p' : Position} {side : Side}
    {i : Nat} {q payout : Amount} (h : m.redeem p side i q = .ok (m', p', payout)) :
    m'.Shape ∧ m'.Solvent := by
  unfold redeem at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals (try simp_all)
  next hfin => obtain ⟨h1, _, h3⟩ := h; subst h3; subst h1; exact hfin

theorem P_SOLV_01_redeem_complete_set_lands_solvent {m m' : Market} {p p' : Position}
    {q payout : Amount} (h : m.redeemCompleteSet p q = .ok (m', p', payout)) :
    m'.Shape ∧ m'.Solvent := by
  unfold redeemCompleteSet at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals (try simp_all)
  next hfin => obtain ⟨h1, _, h3⟩ := h; subst h3; subst h1; exact hfin




/-! ## P-SOLV-01: split and merge (the design's claim (iii)) -/

theorem Shape.outcomes_pos {m : Market} (h : m.Shape) : 0 < m.outcomes := by
  have harity : m.outcomes = m.payouts.outcomes := h.1
  have hlb : minOutcomes ≤ m.payouts.outcomes := h.2.1.1
  have : minOutcomes = 2 := rfl
  omega

theorem Shape.supply_ne_nil {m : Market} (h : m.Shape) : m.totalSupply ≠ [] := by
  have hlen : m.totalSupply.length = m.outcomes := h.2.2.1
  have hpos := Shape.outcomes_pos h
  intro hnil
  rw [hnil] at hlen
  simp at hlen
  omega

theorem Shape.vectors_ne_nil {m : Market} (h : m.Shape) : m.payouts.vectors ≠ [] := by
  have hne : 0 < m.payouts.vectors.length := h.2.1.2.2.1
  intro hnil
  rw [hnil] at hne
  simp at hne

/-- A uniform supply increase raises the resolved requirement by exactly the
same quantity.  Dual of `requiredResolved_dropAll`, and needs no balance
hypothesis. -/
theorem requiredResolved_bumpAll {T : List Amount} {v : PayoutVector} {n q : Nat}
    (hv : v.Admissible n) (hlen : T.length = n) :
    requiredResolved (bumpAll q T) v = requiredResolved T v + q := by
  have hlw : T.length = v.weights.length := by rw [hlen, hv.arity]
  unfold requiredResolved liability
  rw [dot_bumpAll q T v.weights hlw, hv.pou, ceilDiv_add_mul hv.denom_pos]

/-- **The `split`/`merge` invariant, in one lemma.**  In an Active market of
either mode, raising every outcome's supply by `q` raises the collateral
requirement by exactly `q` — no more (which would make `split` refuse a
correctly collateralized mint) and no less (which would let a mint dilute
existing claims).

Mode 1 is the maximum-liability functional and the statement is
`max_i (T_i + q) = (max_i T_i) + q`.  Mode 0 is the preset maximum, and each
preset's requirement moves by exactly `q` because the weights sum to `D`; the
maximum of a uniformly shifted list shifts by the same amount. -/
theorem required_bumpAll {m : Market} (hshape : m.Shape)
    (hactive : m.resolution = Resolution.active) (q : Amount) :
    ∃ r, m.required = some r ∧
      ({ m with totalSupply := bumpAll q m.totalSupply } : Market).required = some (r + q) := by
  have harity : m.outcomes = m.payouts.outcomes := hshape.1
  have hsuplen : m.totalSupply.length = m.outcomes := hshape.2.2.1
  have hadm : ∀ w ∈ m.payouts.vectors, w.Admissible m.payouts.outcomes := hshape.2.1.2.2.2.2.1
  cases hmode : m.basisMode with
  | derivedBasis =>
      refine ⟨requiredActive m.totalSupply, by simp [required, hactive, hmode], ?_⟩
      simp only [required, hactive, requiredActive]
      rw [maxOf_bumpAll q m.totalSupply (Shape.supply_ne_nil hshape)]
  | finitePreset =>
      refine ⟨maxOf (m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w)),
        by simp [required, hactive, hmode], ?_⟩
      simp only [required, hactive]
      have hmap : m.payouts.vectors.map (fun w => requiredResolved (bumpAll q m.totalSupply) w)
          = bumpAll q (m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w)) := by
        unfold bumpAll
        rw [List.map_map]
        refine List.map_congr_left ?_
        intro w hw
        exact requiredResolved_bumpAll (harity ▸ hadm w hw) hsuplen
      rw [hmap, maxOf_bumpAll q _ (by
        intro hnil
        exact absurd (List.eq_nil_of_map_eq_nil hnil) (Shape.vectors_ne_nil hshape))]

/-! ## P-SOLV-01: the finite-preset resolution seam -/

/-- **P-SOLV-01 / claim (i), mode 0.**  A finite-preset market that is Active,
well-shaped, and solvent always accepts a resolution to any preset in its frozen
set, and lands solvent.

The mode-0 argument is the *finite* one and does not need the partition of
unity: the Active requirement is by definition the maximum over the presets, and
resolution selects one of them.  It is stated next to the mode-1 theorem because
together they say the same sentence about both seams — a market that could pay
its worst case can pay the case it got. -/
theorem P_SOLV_01_resolve_admits {m : Market} {i : Nat} {v : PayoutVector}
    (hshape : m.Shape) (hsolv : m.Solvent)
    (hactive : m.resolution = Resolution.active)
    (hmode : m.basisMode = BasisMode.finitePreset)
    (hi : m.payouts.vectors[i]? = some v) :
    m.resolve i = .ok { m with resolution := .byIndex i } := by
  have hlt : i < m.payouts.vectors.length := by
    obtain ⟨h, _⟩ := List.getElem?_eq_some_iff.mp hi; exact h
  have hshape' : ({ m with resolution := .byIndex i } : Market).Shape := by
    obtain ⟨h1, h2, h3, h4, h5, _⟩ := hshape
    exact ⟨h1, h2, h3, h4, h5, by simp only [ResolutionOk, hmode]; exact hlt⟩
  have hreq : m.required =
      some (maxOf (m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w))) := by
    simp [required, hactive, hmode]
  have hcov : maxOf (m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w))
      ≤ m.collateral := by
    unfold Market.Solvent at hsolv; rw [hreq] at hsolv; exact hsolv
  have hmem : requiredResolved m.totalSupply v
      ∈ m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w) :=
    List.mem_map.mpr ⟨v, List.mem_of_getElem? hi, rfl⟩
  have hsolv' : ({ m with resolution := .byIndex i } : Market).Solvent := by
    unfold Market.Solvent
    simp only [required, hi]
    exact Nat.le_trans (le_maxOf_of_mem hmem) hcov
  rw [hmode] at hshape' hsolv'
  simp [resolve, checkInvariants_ok hshape hsolv, requireActive_ok hactive, hmode,
    Nat.not_le.mpr hlt, checkInvariants_ok hshape' hsolv', bind, Except.bind]


/-- `required` never reads collateral. -/
theorem required_update_collateral (m : Market) (c : Amount) :
    ({ m with collateral := c } : Market).required = m.required := rfl

/-- The post-state of a `split` of `q`: `q` more claims of every outcome and
`q` more collateral atoms. -/
def splitState (m : Market) (q : Amount) : Market :=
  { m with collateral := m.collateral + q, totalSupply := bumpAll q m.totalSupply }

/-- **P-SOLV-01 / claim (iii), `split`.**  A well-shaped solvent Active market
always accepts a mint of `q` complete sets against `q` collateral atoms, as long
as no stored amount leaves the fixed-width range, and the result is solvent.

The requirement and the collateral both rise by exactly `q`, so the invariant is
preserved with neither slack gained nor lost.  The only refusals left on this
path are the fixed-width bounds — which is the honest statement of "split cannot
fail for a solvency reason". -/
theorem P_SOLV_01_split_admits {m : Market} {p : Position} {q : Amount}
    (hshape : m.Shape) (hsolv : m.Solvent) (hpos : p.Ok m.outcomes)
    (hactive : m.resolution = Resolution.active) (hq : q ≠ 0)
    (hcb : m.collateral + q ≤ amountMax)
    (hsb : ∀ t ∈ m.totalSupply, t + q ≤ amountMax)
    (hpb : ∀ x ∈ p.internal, x + q ≤ amountMax) :
    m.split p q = .ok (splitState m q, { p with internal := bumpAll q p.internal }) := by
  obtain ⟨r, hr, hr'⟩ := required_bumpAll hshape hactive q
  have hcov : r ≤ m.collateral := by
    unfold Market.Solvent at hsolv; rw [hr] at hsolv; exact hsolv
  have hpostshape : (splitState m q).Shape := by
    refine ⟨hshape.1, hshape.2.1, by simpa [splitState] using hshape.2.2.1, ?_,
      by simpa [splitState] using hcb, hshape.2.2.2.2.2⟩
    intro t ht
    simp only [splitState] at ht
    obtain ⟨x, hx, rfl⟩ := mem_bumpAll ht
    exact hsb x hx
  have hpostsolv : (splitState m q).Solvent := by
    unfold Market.Solvent
    rw [show (splitState m q).required
        = ({ m with totalSupply := bumpAll q m.totalSupply } : Market).required from rfl, hr']
    show r + q ≤ (splitState m q).collateral
    simp only [splitState]
    omega
  have hcheck := checkInvariants_ok hshape hsolv
  have hb1 : (∀ t ∈ m.totalSupply, t + q ≤ amountMax) = True := eq_true hsb
  have hb2 : (∀ x ∈ p.internal, x + q ≤ amountMax) = True := eq_true hpb
  have hfin := checkInvariants_ok hpostshape hpostsolv
  simp only [splitState] at hfin
  simp only [split, bind, Except.bind, hcheck, hpos, requireActive_ok hactive, hq, hcb,
    hb1, hb2, hfin]
  simp [splitState]

/-- A supply floor is a requirement floor: if every outcome carries at least `q`
claims, the resolved requirement is at least `q`.  The partition of unity again:
`Σ_i T_i·w_i ≥ q·Σ_i w_i = q·D`. -/
theorem requiredResolved_ge {T : List Amount} {w : PayoutVector} {n q : Nat}
    (hw : w.Admissible n) (hlen : T.length = n) (hsup : ∀ t ∈ T, q ≤ t) :
    q ≤ requiredResolved T w := by
  have hlw : T.length = w.weights.length := by rw [hlen, hw.arity]
  have hlow : q * w.weights.sum ≤ dot T w.weights := bound_mul_sum_le_dot T w.weights hlw hsup
  rw [hw.pou] at hlow
  have := ceilDiv_le_ceilDiv (b := w.denominator) hlow
  rwa [ceilDiv_mul_cancel hw.denom_pos] at this

/-- **P-SOLV-01, and the R8 check-order finding.**  In a well-shaped solvent
market whose every outcome supply is at least `q`, the collateral is at least
`q` too.

`merge` tests collateral before balances (the landed order pinned as R8 in
`VECTOR_SPINE_PROPOSAL.md`), which is the only reason
`insufficientCollateral` is observable from `merge` at all.  This theorem is the
precise sense in which that refusal is an ordering artifact: whenever the
balance tests would pass, the collateral test cannot fail.  It holds in both
modes, and in the resolved phase as well. -/
theorem required_ge_of_supply_ge {m : Market} {q : Amount} (hshape : m.Shape)
    (hsup : ∀ t ∈ m.totalSupply, q ≤ t) : ∃ r, m.required = some r ∧ q ≤ r := by
  have hsuplen : m.totalSupply.length = m.outcomes := hshape.2.2.1
  have harity : m.outcomes = m.payouts.outcomes := hshape.1
  have hadm : ∀ w ∈ m.payouts.vectors, w.Admissible m.payouts.outcomes := hshape.2.1.2.2.2.2.1
  -- the resolved-phase bound, reused by three of the four cases
  have hres : ∀ w : PayoutVector, w.Admissible m.outcomes →
      q ≤ requiredResolved m.totalSupply w := fun w hw => requiredResolved_ge hw hsuplen hsup
  cases hr : m.resolution with
  | active =>
      cases hmode : m.basisMode with
      | derivedBasis =>
          refine ⟨requiredActive m.totalSupply, by simp [required, hr, hmode], ?_⟩
          have hmem := maxOf_mem m.totalSupply (Shape.supply_ne_nil hshape)
          exact hsup _ hmem
      | finitePreset =>
          refine ⟨maxOf (m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w)),
            by simp [required, hr, hmode], ?_⟩
          have hne : m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w) ≠ [] := by
            intro hnil
            exact absurd (List.eq_nil_of_map_eq_nil hnil) (Shape.vectors_ne_nil hshape)
          obtain ⟨w, hw, hval⟩ := List.mem_map.mp (maxOf_mem _ hne)
          rw [← hval]
          exact hres w (harity ▸ hadm w hw)
  | byIndex i =>
      have hlt : i < m.payouts.vectors.length := by
        have := hshape.2.2.2.2.2
        unfold Market.ResolutionOk at this
        cases hmode : m.basisMode <;> rw [hmode, hr] at this <;> simp at this
        exact this
      obtain ⟨w, hw⟩ : ∃ w, m.payouts.vectors[i]? = some w :=
        ⟨m.payouts.vectors[i], List.getElem?_eq_getElem hlt⟩
      refine ⟨requiredResolved m.totalSupply w, by simp [required, hr, hw], ?_⟩
      exact hres w (harity ▸ hadm w (List.mem_of_getElem? hw))
  | byVector w =>
      have hadmw : w.Admissible m.outcomes := by
        have := hshape.2.2.2.2.2
        unfold Market.ResolutionOk at this
        cases hmode : m.basisMode <;> rw [hmode, hr] at this <;> simp at this
        exact this.1
      exact ⟨requiredResolved m.totalSupply w, by simp [required, hr], hres w hadmw⟩


/-- **R8.**  A solvent, well-shaped market whose every outcome supply is at least
`q` holds at least `q` collateral atoms.  Therefore the `insufficientCollateral`
refusal that `merge` can report — because it tests collateral before balances —
is only ever reachable in states where the balance test would also have failed.
The refusal is an ordering artifact, not a distinct failure mode. -/
theorem R8_merge_collateral_refusal_is_ordering_artifact {m : Market} {q : Amount}
    (hshape : m.Shape) (hsolv : m.Solvent) (hsup : ∀ t ∈ m.totalSupply, q ≤ t) :
    q ≤ m.collateral := by
  obtain ⟨r, hr, hqr⟩ := required_ge_of_supply_ge hshape hsup
  unfold Market.Solvent at hsolv
  rw [hr] at hsolv
  omega

/-! ## P-PAY-02: the complete-set exit is never stranded -/

/-- The vector a market pays from does not change when supply or collateral
change. -/
theorem effectiveVector_update (m : Market) (c : Amount) (T : List Amount) :
    ({ m with collateral := c, totalSupply := T } : Market).effectiveVector =
      m.effectiveVector := rfl

/-- A uniform supply decrease lowers the resolved requirement by exactly the
same quantity — no rounding drift, because the partition of unity makes the
numerator move by exactly `q · D`. -/
theorem requiredResolved_dropAll {T : List Amount} {v : PayoutVector} {n q : Nat}
    (hv : v.Admissible n) (hlen : T.length = n) (hq : ∀ t ∈ T, q ≤ t) :
    requiredResolved (dropAll q T) v + q = requiredResolved T v := by
  have hlw : T.length = v.weights.length := by rw [hlen, hv.arity]
  have hd : dot (dropAll q T) v.weights + q * v.weights.sum = dot T v.weights :=
    dot_dropAll q T v.weights hlw hq
  unfold requiredResolved liability
  rw [← hd, hv.pou, ceilDiv_add_mul hv.denom_pos]

/-- The post-state of a complete-set redemption of `q`: `q` fewer claims of
every outcome and `q` fewer collateral atoms. -/
def completeSetState (m : Market) (q : Amount) : Market :=
  { m with collateral := m.collateral - q, totalSupply := dropAll q m.totalSupply }

/-- **P-PAY-02 — the complete-set exit theorem.**

A holder of `q` units of every active outcome in a well-shaped, solvent,
resolved market *always* redeems the complete set, and is paid exactly `q`
collateral atoms.  Neither the remainder refusal nor the collateral refusal on
this path can fire.

This is the unconditional exit from the fractional-payout trap: single-outcome
redemption can refuse forever under a fractional weight (`⌈q·w_i/D⌉` need not be
an integer), but the complete set never remainders, at any resolved value, in
either resolution mode.  The partition of unity does all the work —
`Σ_i q·w_i = q·D` is (H2) multiplied by `q`.

Hypotheses, all explicit: market shape and solvency, position shape, the market
is resolved (`effectiveVector = some v`), a nonzero quantity, and balances of at
least `q` in every outcome both in the position and in the conservative
aggregate supply.  Nothing about which seam resolved the market, which basis
family it uses, or what value it resolved to. -/
theorem P_PAY_02_complete_set_never_stranded {m : Market} {p : Position}
    {v : PayoutVector} {q : Amount}
    (hshape : m.Shape) (hsolv : m.Solvent) (hpos : p.Ok m.outcomes)
    (hres : m.effectiveVector = some v) (hq : q ≠ 0)
    (hbal : ∀ x ∈ p.internal, q ≤ x) (hsup : ∀ t ∈ m.totalSupply, q ≤ t) :
    m.redeemCompleteSet p q =
      .ok (completeSetState m q, { p with internal := dropAll q p.internal }, q) := by
  obtain ⟨harity, hvalid, hsuplen, hsupbd, hcolbd, hresok⟩ := hshape
  obtain ⟨hv, hvD⟩ := effectiveVector_admissible ⟨harity, hvalid, hsuplen, hsupbd, hcolbd, hresok⟩ hres
  have hDpos : 0 < v.denominator := hv.denom_pos
  -- the complete-set identity
  have hliab : liability (List.replicate m.outcomes q) v = q * v.denominator :=
    P_PAY_02_complete_set_liability_exact hv
  have hmod : liability (List.replicate m.outcomes q) v % v.denominator = 0 := by
    rw [hliab]; exact Nat.mul_mod_left q v.denominator
  have hdiv : liability (List.replicate m.outcomes q) v / v.denominator = q := by
    rw [hliab]; exact Nat.mul_div_cancel q hDpos
  -- the pre-state requirement, and the collateral it guarantees
  have hreq : m.required = some (requiredResolved m.totalSupply v) := required_of_resolved hres
  have hcov : requiredResolved m.totalSupply v ≤ m.collateral := by
    unfold Market.Solvent at hsolv; rw [hreq] at hsolv; exact hsolv
  have hlw : m.totalSupply.length = v.weights.length := by rw [hsuplen, hv.arity]
  have hqle : q ≤ requiredResolved m.totalSupply v := by
    have hlow : q * v.weights.sum ≤ dot m.totalSupply v.weights :=
      bound_mul_sum_le_dot m.totalSupply v.weights hlw hsup
    rw [hv.pou] at hlow
    have : ceilDiv (q * v.denominator) v.denominator ≤ requiredResolved m.totalSupply v :=
      ceilDiv_le_ceilDiv hlow
    rwa [ceilDiv_mul_cancel hDpos] at this
  -- the post state
  have hpostshape : (completeSetState m q).Shape := by
    refine ⟨harity, hvalid, by simpa [completeSetState] using hsuplen, ?_, by simp only [completeSetState]; omega, hresok⟩
    intro t ht
    simp only [completeSetState] at ht
    obtain ⟨x, hx, rfl⟩ := mem_dropAll ht
    have := hsupbd x hx
    omega
  have hpostsolv : (completeSetState m q).Solvent := by
    have hev : (completeSetState m q).effectiveVector = some v := by
      simp only [completeSetState]; rw [effectiveVector_update]; exact hres
    have hreq' := required_of_resolved hev
    have hshift : requiredResolved (dropAll q m.totalSupply) v + q
        = requiredResolved m.totalSupply v :=
      requiredResolved_dropAll hv hsuplen hsup
    unfold Market.Solvent
    rw [hreq']
    show requiredResolved (completeSetState m q).totalSupply v ≤ (completeSetState m q).collateral
    simp only [completeSetState]
    omega
  have hcheck : m.checkInvariants = .ok () :=
    checkInvariants_ok ⟨harity, hvalid, hsuplen, hsupbd, hcolbd, hresok⟩ hsolv
  have hresolved : m.requireResolved = .ok v := by
    unfold Market.requireResolved; rw [hres]
  have hfin : (completeSetState m q).checkInvariants = .ok () := checkInvariants_ok hpostshape hpostsolv
  have hcol : q ≤ m.collateral := Nat.le_trans hqle hcov
  have hb : (∀ x ∈ p.internal, q ≤ x) = True := eq_true hbal
  have hs : (∀ t ∈ m.totalSupply, q ≤ t) = True := eq_true hsup
  simp only [completeSetState] at hfin
  simp only [redeemCompleteSet, bind, Except.bind, hcheck, hpos, hresolved, hq, hb, hs,
    hmod, hdiv, hfin]
  simp [Nat.not_lt.mpr hcol, completeSetState]

/-- **The `merge` invariant.**  Lowering every outcome's supply by `q` lowers the
collateral requirement by exactly `q`, in either mode.  Dual of
`required_bumpAll`, and needs the balance hypothesis that `merge` checks. -/
theorem required_dropAll {m : Market} (hshape : m.Shape)
    (hactive : m.resolution = Resolution.active) {q : Amount}
    (hsup : ∀ t ∈ m.totalSupply, q ≤ t) :
    ∃ r r', m.required = some r ∧
      ({ m with totalSupply := dropAll q m.totalSupply } : Market).required = some r' ∧
      r' + q = r := by
  have harity : m.outcomes = m.payouts.outcomes := hshape.1
  have hsuplen : m.totalSupply.length = m.outcomes := hshape.2.2.1
  have hadm : ∀ w ∈ m.payouts.vectors, w.Admissible m.payouts.outcomes := hshape.2.1.2.2.2.2.1
  cases hmode : m.basisMode with
  | derivedBasis =>
      refine ⟨requiredActive m.totalSupply, requiredActive (dropAll q m.totalSupply),
        by simp [required, hactive, hmode], by simp [required, hactive], ?_⟩
      exact maxOf_dropAll q m.totalSupply (Shape.supply_ne_nil hshape) hsup
  | finitePreset =>
      refine ⟨maxOf (m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w)),
        maxOf (m.payouts.vectors.map (fun w => requiredResolved (dropAll q m.totalSupply) w)),
        by simp [required, hactive, hmode], by simp [required, hactive], ?_⟩
      have hmap : m.payouts.vectors.map (fun w => requiredResolved (dropAll q m.totalSupply) w)
          = dropAll q (m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w)) := by
        unfold dropAll
        rw [List.map_map]
        refine List.map_congr_left ?_
        intro w hw
        have hshift := requiredResolved_dropAll (harity ▸ hadm w hw) hsuplen hsup
        simp only [dropAll] at hshift
        simp only [Function.comp]
        omega
      have hne : m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w) ≠ [] := by
        intro hnil
        exact absurd (List.eq_nil_of_map_eq_nil hnil) (Shape.vectors_ne_nil hshape)
      have hge : ∀ x ∈ m.payouts.vectors.map (fun w => requiredResolved m.totalSupply w), q ≤ x := by
        intro x hx
        obtain ⟨w, hw, rfl⟩ := List.mem_map.mp hx
        exact requiredResolved_ge (harity ▸ hadm w hw) hsuplen hsup
      rw [hmap]
      exact maxOf_dropAll q _ hne hge

/-- **P-SOLV-01 / claim (iii), `merge`, and the R8 corollary.**  A well-shaped
solvent Active market always accepts a merge of `q` complete sets held by the
caller, and the collateral test that precedes the balance tests cannot be the
reason a merge is refused: the balances imply the collateral.

The requirement and the collateral both fall by exactly `q`. -/
theorem P_SOLV_01_merge_admits {m : Market} {p : Position} {q : Amount}
    (hshape : m.Shape) (hsolv : m.Solvent) (hpos : p.Ok m.outcomes)
    (hactive : m.resolution = Resolution.active) (hq : q ≠ 0)
    (hbal : ∀ x ∈ p.internal, q ≤ x) (hsup : ∀ t ∈ m.totalSupply, q ≤ t) :
    m.merge p q = .ok (completeSetState m q, { p with internal := dropAll q p.internal }) := by
  obtain ⟨r, r', hr, hr', hshift⟩ := required_dropAll hshape hactive hsup
  have hcov : r ≤ m.collateral := by
    unfold Market.Solvent at hsolv; rw [hr] at hsolv; exact hsolv
  have hcol : q ≤ m.collateral := R8_merge_collateral_refusal_is_ordering_artifact hshape hsolv hsup
  have hcolbd : m.collateral ≤ amountMax := hshape.2.2.2.2.1
  have hpostshape : (completeSetState m q).Shape := by
    refine ⟨hshape.1, hshape.2.1, by simpa [completeSetState] using hshape.2.2.1, ?_,
      by simp only [completeSetState]; omega, hshape.2.2.2.2.2⟩
    intro t ht
    simp only [completeSetState] at ht
    obtain ⟨x, hx, rfl⟩ := mem_dropAll ht
    have := hshape.2.2.2.1 x hx
    omega
  have hpostsolv : (completeSetState m q).Solvent := by
    unfold Market.Solvent
    rw [show (completeSetState m q).required
        = ({ m with totalSupply := dropAll q m.totalSupply } : Market).required from rfl, hr']
    show r' ≤ (completeSetState m q).collateral
    simp only [completeSetState]
    omega
  have hcheck := checkInvariants_ok hshape hsolv
  have hfin := checkInvariants_ok hpostshape hpostsolv
  simp only [completeSetState] at hfin
  have hb1 : (∀ x ∈ p.internal, q ≤ x) = True := eq_true hbal
  have hb2 : (∀ t ∈ m.totalSupply, q ≤ t) = True := eq_true hsup
  simp only [merge, bind, Except.bind, hcheck, hpos, requireActive_ok hactive, hq,
    Nat.not_lt.mpr hcol, hb1, hb2, hfin]
  simp [completeSetState]

/-! ## P-SUP-01: the materialization boundary is supply-neutral -/

/-- **P-SUP-01.**  `materialize` returns the market unchanged: total per-outcome
supply and Hoard collateral are untouched at the internal/external boundary.
The claim moves sides within one position; nothing is minted or burned. -/
theorem P_SUP_01_materialize_market_unchanged {m m' : Market} {p p' : Position} {i : Nat}
    {q : Amount} (h : m.materialize p i q = .ok (m', p')) : m' = m := by
  unfold materialize at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

/-- **P-SUP-01.**  `dematerialize` likewise. -/
theorem P_SUP_01_dematerialize_market_unchanged {m m' : Market} {p p' : Position} {i : Nat}
    {q : Amount} (h : m.dematerialize p i q = .ok (m', p')) : m' = m := by
  unfold dematerialize at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals simp_all

/-- Writing one entry of a list that has that entry reads back exactly. -/
theorem entry_setEntry_self {l : List Amount} {i : Nat} {a v : Amount}
    (h : entry l i = some a) : entry (setEntry l i v) i = some v := by
  unfold entry setEntry at *
  obtain ⟨hlt, _⟩ := List.getElem?_eq_some_iff.mp h
  simp [List.getElem?_set_self hlt]

/-- **P-SUP-01.**  `transferInternal` is claim-conserving: the two positions'
holdings of the transferred outcome sum to the same total after the transfer as
before.  Market supply and collateral are untouched structurally — the
transition returns no market at all. -/
theorem P_SUP_01_transfer_conserves {m : Market} {from_ to from' to' : Position}
    {i : Nat} {q : Amount} {policy : TransferPhasePolicy}
    (h : m.transferInternal from_ to i q policy = .ok (from', to')) :
    ∃ f t, entry from_.internal i = some f ∧ entry to.internal i = some t ∧
      entry from'.internal i = some (f - q) ∧ entry to'.internal i = some (t + q) ∧
      (f - q) + (t + q) = f + t := by
  unfold transferInternal at h; simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals (try simp_all)
  all_goals (
    obtain ⟨hfrom, hto⟩ := h
    subst hfrom
    subst hto
    refine ⟨entry_setEntry_self (by assumption), entry_setEntry_self (by assumption), ?_⟩
    omega)

end Market
end DragonsClutch
