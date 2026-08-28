import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Source principal capacity: the Mango-lesson founding bound

`CHAIN_STATE_SOURCES_2026_08.md` §6.5 proposes one founding-time admission
predicate for a Market whose resolution family reads third-party chain state:

```text
total_principal ≤ κ · manipulation_cost_lower_bound
```

§5.5's Mango lesson is that the *ratio* of position size to venue depth is the
invariant that was violated, so the shape of the predicate is right even before
κ has a measured value.  This module owns the predicate exactly, in ℕ, with no
division anywhere, plus the fixed-layout `ManipulationFloorV1` record that binds
a venue's derived floor to the Source it is a floor *for*.

Three deliberate decisions live here rather than in prose.

**κ is a rational, cross-multiplied.**  A conservative κ is smaller than one, so
an integer κ cannot express it and a floating κ is forbidden by the kernel
policy.  `Admits` is `principal * denominator ≤ numerator * floor`, which is the
same relation as `principal ≤ (numerator / denominator) * floor` over ℚ without
ever forming a quotient.

**The physical decision refuses on overflow, and that refusal is exact.**  The
Rust kernel computes the left-hand side in `u128` and refuses when the multiply
would wrap.  `overflow_is_exact` below shows this loses nothing: with a `u32` κ
and a `u64` floor the right-hand side is below `2^96`, so a left-hand side that
does not fit `u128` is genuinely larger and genuinely inadmissible.

**A stated bound of zero admits nothing at all.**  κ = 0, or a floor of zero, is
a coherent thing to write down and it means "no principal may be founded against
this Source."  `zero_bound_refuses_everything` is that statement.  The all-zero
reserved tail of an existing `SourceCapacityProfileV1` decodes as *unstated*
rather than as κ = 0, and an unstated capacity is refused by the chain-state
admission path — so a Source that has never thought about κ founds nothing,
which is the fail-closed reading.

The bound this module states is **provisional** in `AGENTS.md`'s sense and its
lifting plan is `liftingPlanPreimage`: κ is carried by a
`SourceCapacityProfileV1` whose `envelope` is `Provisional` and whose
`envelope_basis_id` therefore already *is* the required lifting-plan identity.
No parallel lifting mechanism is minted for κ.
-/

namespace DClutch.SourcePrincipalCapacityV1

open DClutch
open DClutch.AbiSchema

/-! ## Physical envelopes -/

/-- Exclusive upper bound of a `u32` wire field. -/
def u32Bound : Nat := 4294967296
/-- Exclusive upper bound of a `u64` wire field. -/
def u64Bound : Nat := 18446744073709551616
/-- Exclusive upper bound of the `u128` the Rust predicate computes in. -/
def u128Bound : Nat := 340282366920938463463374607431768211456

theorem u128Bound_eq : u128Bound = u64Bound * u64Bound := by native_decide

/-! ## κ and the admission predicate -/

/-- κ as an exact nonnegative rational `numerator / denominator`. -/
structure Kappa where
  numerator : Nat
  denominator : Nat
  deriving DecidableEq, Repr

/-- κ fits its two `u32` wire coordinates. -/
def Kappa.Fits (k : Kappa) : Prop :=
  k.numerator < u32Bound ∧ k.denominator < u32Bound

/-- The founding predicate of §6.5, cross-multiplied so no division exists.

`principal * denominator ≤ numerator * floor` is exactly
`principal ≤ κ · floor` whenever `denominator` is positive. -/
def Admits (k : Kappa) (floorAtoms principal : Nat) : Prop :=
  principal * k.denominator ≤ k.numerator * floorAtoms

instance (k : Kappa) (floorAtoms principal : Nat) :
    Decidable (Admits k floorAtoms principal) := by
  unfold Admits; infer_instance

/-- The exact decision the Rust kernel makes, including every refusal it makes
for a physical rather than a mathematical reason. -/
def admit (k : Kappa) (floorAtoms principal : Nat) : Bool :=
  if k.denominator = 0 then
    false
  else if principal = 0 then
    false
  else if k.numerator * floorAtoms = 0 then
    false
  else if u128Bound ≤ principal * k.denominator then
    false
  else
    decide (principal * k.denominator ≤ k.numerator * floorAtoms)

/-- A left-hand side that does not fit `u128` is genuinely inadmissible, so the
Rust kernel's overflow refusal is exact rather than conservative.  With κ in two
`u32` coordinates and the floor in a `u64`, the right-hand side is below `2^96`;
`2^128` is far above it. -/
theorem overflow_is_exact
    (k : Kappa) (floorAtoms principal : Nat)
    (kappaFits : k.Fits) (floorFits : floorAtoms < u64Bound)
    (overflowed : u128Bound ≤ principal * k.denominator) :
    ¬ Admits k floorAtoms principal := by
  intro admitted
  have bounded : k.numerator * floorAtoms < u32Bound * u64Bound :=
    Nat.mul_lt_mul_of_lt_of_le kappaFits.1 (Nat.le_of_lt floorFits)
      (Nat.pos_of_ne_zero (by intro h; simp [u64Bound] at h))
  have small : u32Bound * u64Bound ≤ u128Bound := by native_decide
  have : principal * k.denominator < u128Bound :=
    Nat.lt_of_le_of_lt admitted (Nat.lt_of_lt_of_le bounded small)
  exact absurd overflowed (Nat.not_le_of_lt this)

/-- Every admitted founding really satisfies §6.5's inequality. -/
theorem admit_sound
    (k : Kappa) (floorAtoms principal : Nat)
    (accepted : admit k floorAtoms principal = true) :
    Admits k floorAtoms principal := by
  unfold admit at accepted
  split at accepted
  · exact absurd accepted (by simp)
  · split at accepted
    · exact absurd accepted (by simp)
    · split at accepted
      · exact absurd accepted (by simp)
      · split at accepted
        · exact absurd accepted (by simp)
        · simpa [Admits] using of_decide_eq_true accepted

/-- Nothing that §6.5 admits is refused for a physical reason: inside the wire
envelope, with a positive principal and a positive bound, the decision is the
mathematical predicate. -/
theorem admit_complete
    (k : Kappa) (floorAtoms principal : Nat)
    (kappaFits : k.Fits) (floorFits : floorAtoms < u64Bound)
    (statedKappa : k.denominator ≠ 0) (positivePrincipal : principal ≠ 0)
    (positiveBound : k.numerator * floorAtoms ≠ 0)
    (admitted : Admits k floorAtoms principal) :
    admit k floorAtoms principal = true := by
  have notOverflowed : ¬ u128Bound ≤ principal * k.denominator := by
    intro overflowed
    exact overflow_is_exact k floorAtoms principal kappaFits floorFits overflowed admitted
  unfold admit
  simp [statedKappa, positivePrincipal, positiveBound, notOverflowed, Admits] at *
  exact admitted

/-- A stated bound of zero — κ = 0, or a venue floor of zero — refuses every
founding, including a degenerate one. -/
theorem zero_bound_refuses_everything
    (k : Kappa) (floorAtoms principal : Nat)
    (zeroBound : k.numerator * floorAtoms = 0) :
    admit k floorAtoms principal = false := by
  unfold admit
  by_cases statedKappa : k.denominator = 0
  · simp [statedKappa]
  · by_cases positivePrincipal : principal = 0
    · simp [statedKappa, positivePrincipal]
    · simp [statedKappa, positivePrincipal, zeroBound]

/-- κ = 0 is a stated bound of zero. -/
theorem zero_kappa_refuses_everything
    (k : Kappa) (floorAtoms principal : Nat) (zeroKappa : k.numerator = 0) :
    admit k floorAtoms principal = false :=
  zero_bound_refuses_everything k floorAtoms principal (by simp [zeroKappa])

/-- A venue floor of zero is a stated bound of zero. -/
theorem zero_floor_refuses_everything
    (k : Kappa) (principal : Nat) :
    admit k 0 principal = false :=
  zero_bound_refuses_everything k 0 principal (by simp)

/-- Admission is downward closed in the principal: if a Market may be founded at
one size it may be founded smaller.  This is what makes the predicate a *cap*
rather than a target. -/
theorem admits_monotone_in_principal
    (k : Kappa) (floorAtoms principal smaller : Nat)
    (admitted : Admits k floorAtoms principal) (le : smaller ≤ principal) :
    Admits k floorAtoms smaller :=
  Nat.le_trans (Nat.mul_le_mul_right k.denominator le) admitted

/-- Admission is upward closed in the floor: a deeper venue admits at least what
a shallower one admits.  §6.5's companion warning still stands — the floor may
*fall* after founding, and that is the observation-time refusal's problem, not
this predicate's. -/
theorem admits_monotone_in_floor
    (k : Kappa) (floorAtoms deeper principal : Nat)
    (admitted : Admits k floorAtoms principal) (le : floorAtoms ≤ deeper) :
    Admits k deeper principal :=
  Nat.le_trans admitted (Nat.mul_le_mul_left k.numerator le)

/-! ## The carried cap, and why a Market has to carry one

§6.5's predicate needs the whole Source graph in scope: κ, the venue floor, and
the three identities the floor is bound to.  A founding route can have all of
that.  A route that *grows* principal afterwards — a complete-set split — cannot,
and re-deriving the predicate there would be worse than not checking it.
`ManipulationFloorV1` binds the Source, the venue configuration and the
collateral unit, but nothing pins *which* floor record carries those bindings, so
two floors that agree on all three and disagree on `floorAtoms` both validate.
Re-deriving at split therefore lets the caller choose its own bound.

So the number is computed once, where the graph is authenticated, and **carried**.
`cap` is that number.  `cap_is_the_predicate` is the statement that carrying it
is not a second semantics: for a stated κ, comparing a principal against the
carried cap decides exactly what re-running §6.5 would have decided.  This is the
whole reason a carried `u128` is allowed to stand in for a record graph.
-/

/-- The largest principal κ admits against a floor: the number a founded Market
carries forward, in the Market's collateral atoms. -/
def cap (k : Kappa) (floorAtoms : Nat) : Nat := k.numerator * floorAtoms / k.denominator

/-- Comparing against the carried cap **is** §6.5, not an approximation of it.

Over ℕ, `principal ≤ ⌊n·f / d⌋` and `principal · d ≤ n · f` are the same
proposition whenever `d` is positive, so the floored division loses nothing. -/
theorem cap_is_the_predicate
    (k : Kappa) (floorAtoms principal : Nat) (statedKappa : 0 < k.denominator) :
    principal ≤ cap k floorAtoms ↔ Admits k floorAtoms principal :=
  Nat.le_div_iff_mul_le statedKappa

/-- The decision a route with only the carried cap in scope makes.

A cap of zero refuses everything, which is exactly what an *absent* cap must
read as: a Market root that was never given a cap is indistinguishable on the
wire from one whose κ or floor was zero, and both mean "grow no principal here."
So the fail-closed reading costs no extra field. -/
def admitAgainstCap (capAtoms principal : Nat) : Bool :=
  if principal = 0 then false
  else if capAtoms = 0 then false
  else decide (principal ≤ capAtoms)

/-- `admit` is exactly "a positive principal that §6.5 admits", inside the wire
envelope.  Every one of its physical refusals — unstated κ, zero principal, zero
bound, `u128` overflow — is either a hypothesis here or is subsumed. -/
theorem admit_iff_positive_and_admits
    (k : Kappa) (floorAtoms principal : Nat)
    (kappaFits : k.Fits) (floorFits : floorAtoms < u64Bound)
    (statedKappa : k.denominator ≠ 0) :
    admit k floorAtoms principal = true ↔ (principal ≠ 0 ∧ Admits k floorAtoms principal) := by
  constructor
  · intro accepted
    refine ⟨?_, admit_sound k floorAtoms principal accepted⟩
    intro zeroPrincipal
    rw [zeroPrincipal] at accepted
    unfold admit at accepted
    simp [statedKappa] at accepted
  · rintro ⟨positivePrincipal, admitted⟩
    by_cases zeroBound : k.numerator * floorAtoms = 0
    · exfalso
      rw [Admits, zeroBound, Nat.le_zero, Nat.mul_eq_zero] at admitted
      exact admitted.elim positivePrincipal statedKappa
    · exact admit_complete k floorAtoms principal kappaFits floorFits statedKappa
        positivePrincipal zeroBound admitted

/-- The carried-cap decision is exactly "a positive principal that §6.5 admits"
too — with no envelope hypotheses at all, because comparing two numbers cannot
overflow. -/
theorem admit_against_cap_iff_positive_and_admits
    (k : Kappa) (floorAtoms principal : Nat) (statedKappa : 0 < k.denominator) :
    admitAgainstCap (cap k floorAtoms) principal = true ↔
      (principal ≠ 0 ∧ Admits k floorAtoms principal) := by
  unfold admitAgainstCap
  by_cases zeroPrincipal : principal = 0
  · simp [zeroPrincipal]
  · rw [if_neg zeroPrincipal]
    by_cases zeroCap : cap k floorAtoms = 0
    · rw [if_pos zeroCap]
      constructor
      · intro impossible
        exact absurd impossible (by simp)
      · rintro ⟨_, admitted⟩
        exfalso
        have le := (cap_is_the_predicate k floorAtoms principal statedKappa).mpr admitted
        rw [zeroCap, Nat.le_zero] at le
        exact zeroPrincipal le
    · rw [if_neg zeroCap, decide_eq_true_iff]
      exact ⟨fun le => ⟨zeroPrincipal,
          (cap_is_the_predicate k floorAtoms principal statedKappa).mp le⟩,
        fun pair => (cap_is_the_predicate k floorAtoms principal statedKappa).mpr pair.2⟩

/-- **The lane's load-bearing theorem.**  A route holding only the carried cap
refuses exactly what a route holding the whole Source graph would refuse.  This
is what licenses computing the bound once at founding and checking it at every
later complete-set split, where the graph is not in scope. -/
theorem carried_cap_decides_exactly_what_the_graph_decides
    (k : Kappa) (floorAtoms principal : Nat)
    (kappaFits : k.Fits) (floorFits : floorAtoms < u64Bound)
    (statedKappa : 0 < k.denominator) :
    admitAgainstCap (cap k floorAtoms) principal = admit k floorAtoms principal := by
  have nonzero : k.denominator ≠ 0 := Nat.ne_of_gt statedKappa
  have ext : ∀ (carried graph : Bool), (carried = true ↔ graph = true) → carried = graph := by
    intro carried graph agree
    cases carried <;> cases graph <;> simp_all
  exact ext _ _
    ((admit_against_cap_iff_positive_and_admits k floorAtoms principal statedKappa).trans
      (admit_iff_positive_and_admits k floorAtoms principal kappaFits floorFits nonzero).symm)

/-- The carried cap is downward closed in the principal, inheriting the property
that makes §6.5 a cap rather than a target. -/
theorem carried_cap_monotone_in_principal
    (capAtoms principal smaller : Nat) (positive : smaller ≠ 0)
    (admitted : admitAgainstCap capAtoms principal = true) (le : smaller ≤ principal) :
    admitAgainstCap capAtoms smaller = true := by
  unfold admitAgainstCap at admitted ⊢
  by_cases zeroPrincipal : principal = 0
  · simp [zeroPrincipal] at admitted
  · by_cases zeroCap : capAtoms = 0
    · simp [zeroPrincipal, zeroCap] at admitted
    · simp only [if_neg zeroPrincipal, if_neg zeroCap, decide_eq_true_iff] at admitted
      simp only [if_neg positive, if_neg zeroCap, decide_eq_true_iff]
      exact Nat.le_trans le admitted

/-! ## Projection to complete-set units

Founding authenticates the Source graph in collateral atoms, while every later
split already counts complete sets. `capSets` is the one named unit boundary:
it floors the authenticated atom cap by the Market's positive basis scale.
`cap_sets_is_the_atom_predicate` proves that this is not a second bound.

The runtime wire is a `u64`. Its maximum value is the explicit unbounded
sentinel, so any mathematical quotient at or above that value saturates to the
sentinel. That saturation loses no refusal: every complete-set count the wire
can express is then below the authenticated atom cap.
-/

/-- Project one authenticated collateral-atom cap into complete-set units. -/
def capSets (capAtoms basisScale : Nat) : Nat := capAtoms / basisScale

/-- Comparing complete sets against the projected cap is exactly comparing the
corresponding collateral atoms against the atom cap. -/
theorem cap_sets_is_the_atom_predicate
    (capAtoms basisScale sets : Nat) (positiveScale : 0 < basisScale) :
    sets ≤ capSets capAtoms basisScale ↔ sets * basisScale ≤ capAtoms :=
  Nat.le_div_iff_mul_le positiveScale

/-- The largest value of one `u64` wire field. This value is the explicit
unbounded sentinel of the projected runtime cap. -/
def capSetsUnbounded : Nat := u64Bound - 1

/-- The exact projection written to the runtime wire. `none` is the explicit
zero-scale refusal; `some 0` remains the fail-closed absent cap. -/
def projectCapSets (capAtoms basisScale : Nat) : Option Nat :=
  if basisScale = 0 then none
  else some (min (capSets capAtoms basisScale) capSetsUnbounded)

/-- Below the sentinel, the wire projection is the exact mathematical floor. -/
theorem project_cap_sets_exact_below_sentinel
    (capAtoms basisScale : Nat) (positiveScale : basisScale ≠ 0)
    (below : capSets capAtoms basisScale ≤ capSetsUnbounded) :
    projectCapSets capAtoms basisScale = some (capSets capAtoms basisScale) := by
  simp [projectCapSets, positiveScale, below]

/-- Saturation to the explicit unbounded sentinel is canonical. -/
theorem project_cap_sets_saturates
    (capAtoms basisScale : Nat) (positiveScale : basisScale ≠ 0)
    (saturated : capSetsUnbounded ≤ capSets capAtoms basisScale) :
    projectCapSets capAtoms basisScale = some capSetsUnbounded := by
  simp [projectCapSets, positiveScale, Nat.min_eq_right saturated]

/-- A saturated cap admits every positive complete-set count representable in
the `u64` runtime field, and the atom predicate agrees. -/
theorem saturated_set_cap_admits_every_representable_count
    (capAtoms basisScale sets : Nat) (positiveScale : 0 < basisScale)
    (saturated : capSetsUnbounded ≤ capSets capAtoms basisScale)
    (representable : sets < u64Bound) :
    sets * basisScale ≤ capAtoms := by
  apply (cap_sets_is_the_atom_predicate capAtoms basisScale sets positiveScale).mp
  exact Nat.le_trans (Nat.le_sub_one_of_lt representable) saturated

/-- Lean-owned boundary cases grading the Rust projection. -/
def capProjectionCases : List (Nat × Nat) := [
  (0, 1),
  (1, 1),
  (4654518500, 1),
  (4654518500, 1000000000),
  (4654518500, 10000000000),
  (u64Bound - 2, 1),
  (u64Bound - 1, 1),
  (u64Bound, 1),
  (u128Bound - 1, 1),
  (u128Bound - 1, u64Bound - 1),
  (1, 0)
]

theorem cap_projection_cases_are_physical :
    capProjectionCases.all fun candidate =>
      candidate.1 < u128Bound && candidate.2 < u64Bound := by
  native_decide

/-- A carried cap at the top of its `u128` wire field admits every principal the
wire can express.  This is why "explicitly unbounded" needs no escape hatch in
the decision: it is an ordinary cap whose value happens to bound everything, so
one comparison still decides the whole question. -/
theorem saturated_cap_admits_every_representable_principal
    (principal : Nat) (positive : principal ≠ 0) (representable : principal < u128Bound) :
    admitAgainstCap (u128Bound - 1) principal = true := by
  unfold admitAgainstCap
  have capNonzero : u128Bound - 1 ≠ 0 := by native_decide
  simp only [if_neg positive, if_neg capNonzero, decide_eq_true_iff]
  exact Nat.le_sub_one_of_lt representable

/-! ## The conservative default and the graduation instance

§5.4 derives the pump.fun graduation floor exactly: **18.618074 SOL** of
unrecoverable loss on the buy-out-and-exit round trip.  Two properties make it
usable as this predicate's right-hand side, and neither holds for a marginal
price manipulation: it is fixed by the curve's own published parameters and does
not fall as the coin's real liquidity thins, and it is realized loss rather than
capital at risk, so a flash loan does not reduce it.

κ = 1/4 is the conservative default, and it is conservative in a stateable way.
An attacker who forces the observed outcome captures at most the whole Hoard,
so κ = 1 is break-even against a *perfectly* extracting attacker; κ = 1/4 keeps
a four-fold margin against one.  The lifting plan is measurement of the fraction
an attacker can actually realize on a given venue, followed by a per-venue κ. -/

/-- The conservative default κ for a chain-state Source: one quarter. -/
def defaultKappa : Kappa := { numerator := 1, denominator := 4 }

theorem defaultKappa_fits : defaultKappa.Fits := by
  constructor <;> native_decide

/-- §5.4's exact pump.fun graduation floor, in lamports. -/
def graduationFloorLamports : Nat := 18618074000

/-- The largest founding principal the default κ admits against §5.4's floor,
and the first lamport it refuses.  This is the number a graduation Market is
actually founded under. -/
theorem default_graduation_cap :
    admit defaultKappa graduationFloorLamports 4654518500 = true ∧
      admit defaultKappa graduationFloorLamports 4654518501 = false := by
  constructor <;> native_decide

/-- The same number, as the value a founded graduation Market **carries**.  The
boundary pair is the same pair whether it is decided from the Source graph or
from the carried cap, which is `carried_cap_decides_exactly_what_the_graph_decides`
made concrete at the one instance that ships. -/
theorem default_graduation_cap_is_carried :
    cap defaultKappa graduationFloorLamports = 4654518500 ∧
      admitAgainstCap (cap defaultKappa graduationFloorLamports) 4654518500 = true ∧
        admitAgainstCap (cap defaultKappa graduationFloorLamports) 4654518501 = false := by
  refine ⟨by native_decide, by native_decide, by native_decide⟩

/-- The 85 SOL nominal figure is *not* the attack cost (§5.4's first correction);
sizing a graduation Market against it over-states safety.  Stated as an
inequality the predicate makes visible: a principal admitted against the nominal
figure is refused against the real floor. -/
theorem nominal_curve_cost_would_over_admit :
    admit defaultKappa 85005359000 21251339750 = true ∧
      admit defaultKappa graduationFloorLamports 21251339750 = false := by
  constructor <;> native_decide

/-! ## `ManipulationFloorV1`

The floor is a venue fact, and the substitution it must refuse is a floor
derived for a *different* venue, unit, or Source.  Every one of those bindings
is a field here, and the admission path compares all three.  The record carries
no principal, no Market and no generation: it is the same immutable venue
derivation for every Market founded against that Source. -/

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x4d, 0x46, 0x4c, 0x31] -- `DCLTMFL1`
def schemaVersion : Nat := 1

def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/source-manipulation-floor-v1".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x9c, 0x1d, 0xc9, 0x00, 0xe6, 0xb6, 0xbf, 0x2c,
  0x7e, 0xf2, 0xfe, 0xbe, 0xbe, 0x2c, 0x0a, 0xa0,
  0x85, 0x29, 0xaf, 0x8c, 0x44, 0xc4, 0xfd, 0x1d,
  0x22, 0xab, 0xb7, 0x65, 0xb1, 0xda, 0x16, 0x00
]

/-- The lifting plan κ is provisional against.  A `SourceCapacityProfileV1`
carrying κ with a `Provisional` envelope names this in `envelope_basis_id`. -/
def liftingPlanPreimage : List UInt8 :=
  "dclutch/lifting-plan/source-principal-capacity-kappa/v1".toUTF8.toList
def liftingPlanId : List UInt8 := [
  0x36, 0x98, 0x96, 0x3c, 0xa3, 0x79, 0x55, 0x87,
  0xd2, 0x31, 0xb4, 0xa2, 0x7f, 0xa7, 0xc3, 0x8b,
  0x7d, 0x21, 0x01, 0x1e, 0xf5, 0xfa, 0x11, 0x02,
  0xb5, 0x38, 0xb4, 0x2a, 0xed, 0x57, 0x12, 0x11
]

/-- The named derivation of a bonding curve's buy-out-and-exit floor: §5.4's
exact-decimal arithmetic over the published constant product and fee
parameters.  A floor record naming this release claims that derivation and not
an observed pool depth. -/
def curveDerivationReleasePreimage : List UInt8 :=
  "dclutch/source-manipulation-floor-derivation/bonding-curve-buyout-exit/v1".toUTF8.toList
def curveDerivationReleaseId : List UInt8 := [
  0x0c, 0x2e, 0x4a, 0x32, 0xad, 0x01, 0x43, 0x52,
  0xec, 0x6f, 0xf0, 0xe7, 0x2d, 0x6a, 0x7a, 0xf1,
  0xee, 0xfb, 0x01, 0x99, 0x7e, 0xdf, 0x56, 0xfb,
  0x83, 0x08, 0xdb, 0xf1, 0x9b, 0xcf, 0x80, 0xbd
]

/-- How a floor was arrived at.  The two differ in whether the number falls when
liquidity thins: a curve-derived floor does not, an observed-depth floor does,
and §6.5 requires the observation-time refusal precisely because of the second. -/
inductive FloorBasis where
  /-- Fixed by the venue's own published curve parameters (§5.4). -/
  | curveDerived
  /-- Read from the venue's reserves at founding (§5.2). -/
  | observedDepth
  deriving DecidableEq, Repr

def FloorBasis.tag : FloorBasis → Nat
  | .curveDerived => 1
  | .observedDepth => 2

def FloorBasis.ofTag? : Nat → Option FloorBasis
  | 1 => some .curveDerived
  | 2 => some .observedDepth
  | _ => none

theorem floorBasis_tag_round_trip (basis : FloorBasis) :
    FloorBasis.ofTag? basis.tag = some basis := by
  cases basis <;> rfl

inductive Field where
  | magic | version | basis | reserved | sourceSpec | adapterConfig
  | collateralUnit | derivationRelease | floorAtoms | tailReserved
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.basis, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.sourceSpec, .bytes 32⟩,
  ⟨.adapterConfig, .bytes 32⟩,
  ⟨.collateralUnit, .bytes 32⟩,
  ⟨.derivationRelease, .bytes 32⟩,
  ⟨.floorAtoms, .u64⟩,
  ⟨.tailReserved, .reserved 8⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "MANIPULATION_FLOOR_V1_MAGIC_OFFSET"
  | .version => "MANIPULATION_FLOOR_V1_VERSION_OFFSET"
  | .basis => "MANIPULATION_FLOOR_V1_BASIS_OFFSET"
  | .reserved => "MANIPULATION_FLOOR_V1_RESERVED_OFFSET"
  | .sourceSpec => "MANIPULATION_FLOOR_V1_SOURCE_SPEC_OFFSET"
  | .adapterConfig => "MANIPULATION_FLOOR_V1_ADAPTER_CONFIG_OFFSET"
  | .collateralUnit => "MANIPULATION_FLOOR_V1_COLLATERAL_UNIT_OFFSET"
  | .derivationRelease => "MANIPULATION_FLOOR_V1_DERIVATION_RELEASE_OFFSET"
  | .floorAtoms => "MANIPULATION_FLOOR_V1_FLOOR_ATOMS_OFFSET"
  | .tailReserved => "MANIPULATION_FLOOR_V1_TAIL_RESERVED_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0

end Field

theorem exact_width : bytes = 160 := by native_decide

theorem schema_well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem layout_is_byte_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

theorem coordinates_are_pinned : coordinates layout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.basis, 10, 1),
    (.reserved, 11, 5),
    (.sourceSpec, 16, 32),
    (.adapterConfig, 48, 32),
    (.collateralUnit, 80, 32),
    (.derivationRelease, 112, 32),
    (.floorAtoms, 144, 8),
    (.tailReserved, 152, 8)
  ] := by
  native_decide

structure Floor where
  basis : FloorBasis
  sourceSpec : Nat
  adapterConfig : Nat
  collateralUnit : Nat
  derivationRelease : Nat
  floorAtoms : Nat
  deriving DecidableEq, Repr

def fitsId (value : Nat) : Bool := value < 256 ^ 32

/-- A floor of zero is representable and means "found nothing here"; a zero
identity is not, because every binding this record exists to check would then
be vacuous. -/
def Floor.valid (value : Floor) : Bool :=
  value.sourceSpec != 0 && fitsId value.sourceSpec &&
  value.adapterConfig != 0 && fitsId value.adapterConfig &&
  value.collateralUnit != 0 && fitsId value.collateralUnit &&
  value.derivationRelease != 0 && fitsId value.derivationRelease &&
  value.floorAtoms < u64Bound

def encode (value : Floor) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++
  Codec.encodeLE 1 value.basis.tag ++ List.replicate 5 0 ++
  Codec.encodeLE 32 value.sourceSpec ++
  Codec.encodeLE 32 value.adapterConfig ++
  Codec.encodeLE 32 value.collateralUnit ++
  Codec.encodeLE 32 value.derivationRelease ++
  Codec.encodeLE 8 value.floorAtoms ++
  List.replicate 8 0

theorem encoding_length (value : Floor) : (encode value).length = bytes := by
  simp [encode, bytes, schema, schemaWidth, magic, Codec.encodeLE_length,
    FieldKind.byteWidth]

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

/-- Named hostile-decode refusals, in the exact order the Rust decoder tests
them.  A corpus case that only says "this refuses" would also pass against a
decoder that refuses for the wrong reason. -/
inductive Refusal where
  | invalidLength
  | invalidMagic
  | unsupportedSchema
  | nonCanonicalReserved
  | unknownBasis
  | zeroContentId
  deriving DecidableEq, Repr

def Refusal.tag : Refusal → Nat
  | .invalidLength => 0
  | .invalidMagic => 1
  | .unsupportedSchema => 2
  | .nonCanonicalReserved => 3
  | .unknownBasis => 4
  | .zeroContentId => 5

def refuse? (input : List UInt8) : Option Refusal :=
  if input.length ≠ bytes then some .invalidLength
  else if input.take 8 ≠ magic then some .invalidMagic
  else if sliceNat input Field.version.offset 2 ≠ schemaVersion then some .unsupportedSchema
  else if (input.drop Field.reserved.offset).take 5 ≠ List.replicate 5 0 then
    some .nonCanonicalReserved
  else if (input.drop Field.tailReserved.offset).take 8 ≠ List.replicate 8 0 then
    some .nonCanonicalReserved
  else if (FloorBasis.ofTag? (sliceNat input Field.basis.offset 1)).isNone then
    some .unknownBasis
  else if sliceNat input Field.sourceSpec.offset 32 = 0 then some .zeroContentId
  else if sliceNat input Field.adapterConfig.offset 32 = 0 then some .zeroContentId
  else if sliceNat input Field.collateralUnit.offset 32 = 0 then some .zeroContentId
  else if sliceNat input Field.derivationRelease.offset 32 = 0 then some .zeroContentId
  else none

def validBytes (input : List UInt8) : Bool := (refuse? input).isNone

def exampleFloor : Floor := {
  basis := .curveDerived
  sourceSpec := 0x51
  adapterConfig := 0x52
  collateralUnit := 0x53
  derivationRelease := 0x54
  floorAtoms := graduationFloorLamports
}

theorem example_valid : exampleFloor.valid = true := by native_decide
theorem example_bytes_accepted : validBytes (encode exampleFloor) = true := by native_decide

def refusalCorpus : List (List UInt8) := [
  (encode exampleFloor).set 0 0,
  (encode exampleFloor).set Field.version.offset 2,
  (encode exampleFloor).set Field.basis.offset 0,
  (encode exampleFloor).set Field.basis.offset 3,
  (encode exampleFloor).set Field.reserved.offset 1,
  (encode exampleFloor).set (Field.reserved.offset + 4) 1,
  (encode exampleFloor).set Field.sourceSpec.offset 0,
  (encode exampleFloor).set Field.adapterConfig.offset 0,
  (encode exampleFloor).set Field.collateralUnit.offset 0,
  (encode exampleFloor).set Field.derivationRelease.offset 0,
  (encode exampleFloor).set Field.tailReserved.offset 1,
  (encode exampleFloor).set (Field.tailReserved.offset + 7) 1,
  (encode exampleFloor).take (bytes - 1),
  encode exampleFloor ++ [0]
]

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

/-- Every corpus case refuses for a *named* reason, and the corpus covers every
named reason at least once. -/
theorem generated_refusal_corpus_is_exhaustive :
    (refusalCorpus.filterMap refuse?).length = refusalCorpus.length ∧
      [Refusal.invalidLength, .invalidMagic, .unsupportedSchema, .nonCanonicalReserved,
        .unknownBasis, .zeroContentId].all
        (fun reason => (refusalCorpus.filterMap refuse?).contains reason) := by
  constructor <;> native_decide

/-- Zeroing the `sourceSpec` identity is not the only substitution attack — the
interesting one replaces it with a *different* real venue's identity.  The
binding check is an equality against the authenticated Source, so a floor that
decodes perfectly still refuses. -/
def BindsTo (value : Floor)
    (authenticatedSourceSpec authenticatedAdapterConfig marketCollateralUnit : Nat) : Bool :=
  value.valid &&
    value.sourceSpec = authenticatedSourceSpec &&
    value.adapterConfig = authenticatedAdapterConfig &&
    value.collateralUnit = marketCollateralUnit

theorem substituted_source_refuses
    (value : Floor) (sourceSpec adapterConfig collateralUnit : Nat)
    (substituted : value.sourceSpec ≠ sourceSpec) :
    BindsTo value sourceSpec adapterConfig collateralUnit = false := by
  simp [BindsTo, substituted]

theorem substituted_venue_refuses
    (value : Floor) (sourceSpec adapterConfig collateralUnit : Nat)
    (substituted : value.adapterConfig ≠ adapterConfig) :
    BindsTo value sourceSpec adapterConfig collateralUnit = false := by
  simp [BindsTo, substituted]

theorem substituted_unit_refuses
    (value : Floor) (sourceSpec adapterConfig collateralUnit : Nat)
    (substituted : value.collateralUnit ≠ collateralUnit) :
    BindsTo value sourceSpec adapterConfig collateralUnit = false := by
  simp [BindsTo, substituted]

/-! ## κ's coordinates inside `SourceCapacityProfileV1`

κ does not get a record of its own.  `SourceCapacityProfileV1` is already the
Source's capacity envelope, already carries the `Measured`/`Provisional`
distinction and the lifting-plan identity, and is already named by
`SourceSpecV1.capacity_profile_id`, so κ takes two `u32` coordinates out of that
record's reserved tail.  The width does not move. -/

def capacityProfileBytes : Nat := 112
def capacityProfileTailOffset : Nat := 88
def capacityProfileTailBytes : Nat := 24

/-- κ's two coordinates and the reserved span that survives them. -/
inductive TailField where
  | principalCapacityNumerator | principalCapacityDenominator | tailReserved
  deriving DecidableEq, Repr

def tailSchema : List (FieldSpec TailField) := [
  ⟨.principalCapacityNumerator, .u32⟩,
  ⟨.principalCapacityDenominator, .u32⟩,
  ⟨.tailReserved, .reserved 16⟩
]

def tailLayout : List (PlacedField TailField) :=
  specializeFrom capacityProfileTailOffset tailSchema

namespace TailField

def rustName : TailField → String
  | .principalCapacityNumerator => "SOURCE_CAPACITY_PRINCIPAL_NUMERATOR_OFFSET_V1"
  | .principalCapacityDenominator => "SOURCE_CAPACITY_PRINCIPAL_DENOMINATOR_OFFSET_V1"
  | .tailReserved => "SOURCE_CAPACITY_PRINCIPAL_TAIL_RESERVED_OFFSET_V1"

def offset (field : TailField) : Nat :=
  (coordinate? field tailLayout).map (fun value => value.1) |>.getD 0

end TailField

/-- κ consumes exactly the reserved span the profile already had, and the record
still ends at 112 bytes. -/
theorem tail_fits_former_reserved :
    schemaWidth tailSchema = capacityProfileTailBytes ∧
      capacityProfileTailOffset + schemaWidth tailSchema = capacityProfileBytes := by
  constructor <;> native_decide

theorem tail_coordinates_are_pinned : coordinates tailLayout = [
    (.principalCapacityNumerator, 88, 4),
    (.principalCapacityDenominator, 92, 4),
    (.tailReserved, 96, 16)
  ] := by
  native_decide

theorem tail_layout_is_byte_disjoint : tailLayout.Pairwise Before :=
  specializeFrom_pairwise capacityProfileTailOffset tailSchema

/-- How a decoded κ tail reads. -/
inductive PrincipalCapacity where
  /-- Both coordinates zero: this Source states no principal bound at all.  This
  is what every profile written before κ existed decodes as, and the chain-state
  admission path refuses it. -/
  | unstated
  /-- A stated κ.  The numerator may be zero; the denominator may not. -/
  | bounded (value : Kappa)
  deriving DecidableEq, Repr

def readCapacity (numerator denominator : Nat) : Option PrincipalCapacity :=
  if denominator = 0 then
    if numerator = 0 then some .unstated else none
  else
    some (.bounded { numerator, denominator })

/-- The pre-κ record decodes as `unstated`, not as κ = 0, and not as a decode
failure.  Fail-closed happens at admission, where `unstated` is refused, rather
than by making every existing capacity profile undecodable. -/
theorem legacy_zero_tail_is_unstated : readCapacity 0 0 = some .unstated := by rfl

/-- A numerator without a denominator is not a rational and is refused on the
wire rather than interpreted. -/
theorem numerator_without_denominator_refuses
    (numerator : Nat) (nonzero : numerator ≠ 0) :
    readCapacity numerator 0 = none := by
  simp [readCapacity, nonzero]

/-- The whole chain-state founding admission, as one decision: read the tail,
bind the floor, apply §6.5. -/
def admitFounding
    (numerator denominator : Nat) (floor : Floor)
    (authenticatedSourceSpec authenticatedAdapterConfig marketCollateralUnit
      principal : Nat) : Bool :=
  match readCapacity numerator denominator with
  | none => false
  | some .unstated => false
  | some (.bounded k) =>
      BindsTo floor authenticatedSourceSpec authenticatedAdapterConfig marketCollateralUnit &&
        admit k floor.floorAtoms principal

/-- A Source that never stated κ founds nothing. -/
theorem unstated_capacity_founds_nothing
    (floor : Floor) (sourceSpec adapterConfig collateralUnit principal : Nat) :
    admitFounding 0 0 floor sourceSpec adapterConfig collateralUnit principal = false := by
  simp [admitFounding, legacy_zero_tail_is_unstated]

/-- The demo graduation Market's founding, decided end to end. -/
theorem graduation_market_founds_under_default_kappa :
    admitFounding defaultKappa.numerator defaultKappa.denominator exampleFloor
        exampleFloor.sourceSpec exampleFloor.adapterConfig exampleFloor.collateralUnit
        4654518500 = true ∧
      admitFounding defaultKappa.numerator defaultKappa.denominator exampleFloor
        exampleFloor.sourceSpec exampleFloor.adapterConfig exampleFloor.collateralUnit
        4654518501 = false := by
  constructor <;> native_decide

/-! ## The admission corpus

Every case is `(κ numerator, κ denominator, floor atoms, principal atoms)`; the
expected verdict is `admit` applied to it, so the corpus cannot disagree with
the theorems above by construction. -/

def admissionCases : List (Nat × Nat × Nat × Nat) := [
  -- The demo graduation Market, at the cap and one atom past it.
  (1, 4, graduationFloorLamports, 4654518500),
  (1, 4, graduationFloorLamports, 4654518501),
  -- Exact equality is admitted; the predicate is `≤`.
  (1, 1, 1000, 1000),
  (1, 1, 1000, 1001),
  -- κ above one is expressible and is not this default.
  (3, 2, 1000, 1500),
  (3, 2, 1000, 1501),
  -- A stated bound of zero admits nothing, by either factor.
  (0, 1, graduationFloorLamports, 1),
  (1, 4, 0, 1),
  (0, 1, 0, 1),
  -- A principal of zero is not a Market.
  (1, 4, graduationFloorLamports, 0),
  -- An unstated capacity: no denominator, so no rational.
  (0, 0, graduationFloorLamports, 1),
  (1, 0, graduationFloorLamports, 1),
  -- Rounding: the predicate never divides, so a bound that is not an integer
  -- multiple still admits exactly its floor.
  (1, 3, 10, 3),
  (1, 3, 10, 4),
  -- The widest wire envelope that is still admissible.
  (4294967295, 1, 18446744073709551615, 18446744073709551615),
  -- Overflow of `principal * denominator` in `u128`, refused exactly.
  (4294967295, 1, 18446744073709551615, 340282366920938463463374607431768211455),
  (1, 4294967295, 18446744073709551615, 340282366920938463463374607431768211455)
]

/-- The corpus exercises both verdicts; a corpus that only refused would pass
against a decoder that refuses everything. -/
theorem admission_corpus_exercises_both_verdicts :
    (admissionCases.any fun case =>
        admit { numerator := case.1, denominator := case.2.1 } case.2.2.1 case.2.2.2) ∧
      (admissionCases.any fun case =>
        !admit { numerator := case.1, denominator := case.2.1 } case.2.2.1 case.2.2.2) := by
  constructor <;> native_decide

end DClutch.SourcePrincipalCapacityV1
