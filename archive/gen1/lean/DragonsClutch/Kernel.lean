import DragonsClutch.Solvency
/-!
# The kernel state machine

A Rust-independent model of the semantic plane of `crates/clutch-kernel`: the
market state, one claimant position, and the ten transitions.  Every transition
is a **total function into `Except Error`** — refusal is a value, never a
partial function, a panic, or an unstated precondition.

What is modelled, and what deliberately is not, is the subject of
`docs/implementation/LEAN_MODEL_PLAN.md`.  In one line: this file owns the
semantic plane of `docs/ARCHITECTURE.md` §1 and nothing of the hostile
integration plane.  There are no accounts, bytes, mints, PDAs, clocks, or CPIs
here, and there is no claim anywhere in this model about the Rust program.

Two structural differences from the Rust encoding, both deliberate and both
costs as well as benefits:

1. **Resolution is one inductive value**, not a `(phase, index, vector)` triple
   plus a validator.  The kernel's "one resolution seam per mode, never both"
   is therefore true by construction here.  The cost is real: this model cannot
   catch a defect in the Rust's `validate_resolution`, because it cannot
   express the state that check exists to refuse.  That obligation belongs to
   the vector spine and to Verus, and is named in the plan.
2. **Amounts are `Nat` with an explicit `amountMax` bound checked at every
   write.**  Fixed-width behaviour is modelled as refusal, never as wraparound.
   Intermediate arithmetic is exact; the Rust's `u128` intermediates are
   covered by `P_PAY_01_liability_fits_u128` on the liability path.
-/

namespace DragonsClutch

/-- Refusal classes, mirroring `clutch_kernel::Error` one-for-one so that the
vector spine can compare error *classes* across executors. -/
inductive Error where
  | invalidOutcomeCount
  | invalidPayoutCount
  | invalidPayoutIndex
  | invalidDenominator
  | invalidPayoutWeights
  | zeroQuantity
  | arithmeticOverflow
  | arithmeticUnderflow
  | insufficientBalance
  | insufficientCollateral
  | notActive
  | alreadyResolved
  | notResolved
  | invariantViolation
  | remainderRequired
  | wrongResolutionMode
deriving Repr, DecidableEq, Inhabited

inductive Phase where
  | active
  | resolved
deriving Repr, DecidableEq, Inhabited

/-- The single resolution seam a market freezes at construction. -/
inductive BasisMode where
  | finitePreset
  | derivedBasis
deriving Repr, DecidableEq, Inhabited

/-- The resolution slot.  One inductive value replaces the Rust's
`(phase, resolved_payout, resolved_vector)` triple; see the file header. -/
inductive Resolution where
  | active
  | byIndex (i : Nat)
  | byVector (v : PayoutVector)
deriving Repr, DecidableEq, Inhabited

/-- Which side of a position a redemption draws from. -/
inductive Side where
  | internal
  | external
deriving Repr, DecidableEq, Inhabited

/-- Phase gate for `transferInternal`: the frozen alternatives T-a and T-b of
`BATCH_RELATION_V1_DESIGN.md` §14.2.  Neither is a default. -/
inductive TransferPhasePolicy where
  | activeOnly
  | activeOrResolved
deriving Repr, DecidableEq, Inhabited

/-- The market: a frozen payout set and resolution seam, the conservative
aggregate claim supply, and the Hoard collateral. -/
structure Market where
  outcomes : Nat
  basisMode : BasisMode
  payouts : PayoutSet
  resolution : Resolution
  collateral : Amount
  totalSupply : List Amount
deriving Repr, DecidableEq, Inhabited

/-- One claimant's internal and external (bearer) balances. -/
structure Position where
  internal : List Amount
  external : List Amount
deriving Repr, DecidableEq, Inhabited

namespace Market

def phase (m : Market) : Phase :=
  match m.resolution with
  | .active => .active
  | _ => .resolved

/-- The structural rules the resolution slot obeys, per mode.  Mirrors
`validate_resolution`, minus the two impossible-by-construction cases. -/
def ResolutionOk (m : Market) : Prop :=
  match m.basisMode, m.resolution with
  | .finitePreset, .active => True
  | .finitePreset, .byIndex i => i < m.payouts.vectors.length
  | .finitePreset, .byVector _ => False
  | .derivedBasis, .active => True
  | .derivedBasis, .byIndex _ => False
  | .derivedBasis, .byVector v =>
      v.Admissible m.outcomes ∧ v.denominator = m.payouts.denominator

instance (m : Market) : Decidable m.ResolutionOk := by unfold ResolutionOk; split <;> infer_instance

/-- Structural validity: `validate_shape` plus the amount bounds that the Rust
gets from `u64` typing. -/
def Shape (m : Market) : Prop :=
  m.outcomes = m.payouts.outcomes ∧
  m.payouts.Valid ∧
  m.totalSupply.length = m.outcomes ∧
  (∀ t ∈ m.totalSupply, t ≤ amountMax) ∧
  m.collateral ≤ amountMax ∧
  m.ResolutionOk

instance (m : Market) : Decidable m.Shape := by unfold Shape; infer_instance

/-- The payout vector a resolved market actually pays from: the preset named by
index in mode 0, the installed vector in mode 1.  `none` while Active. -/
def effectiveVector (m : Market) : Option PayoutVector :=
  match m.resolution with
  | .active => none
  | .byIndex i => m.payouts.vectors[i]?
  | .byVector v => some v

/-- The collateral requirement, by phase and mode.  `none` is the malformed
case that `Shape` excludes: an out-of-range resolved index. -/
def required (m : Market) : Option Amount :=
  match m.resolution with
  | .active =>
      match m.basisMode with
      | .finitePreset =>
          some (maxOf (m.payouts.vectors.map (fun v => requiredResolved m.totalSupply v)))
      | .derivedBasis => some (requiredActive m.totalSupply)
  | .byIndex i =>
      match m.payouts.vectors[i]? with
      | some v => some (requiredResolved m.totalSupply v)
      | none => none
  | .byVector v => some (requiredResolved m.totalSupply v)

/-- Solvency: collateral covers the requirement of the phase the market is in. -/
def Solvent (m : Market) : Prop :=
  match m.required with
  | none => False
  | some r => r ≤ m.collateral

instance (m : Market) : Decidable m.Solvent := by unfold Solvent; split <;> infer_instance

/-- `check_invariants`: shape and solvency, both reported as
`invariantViolation`, exactly as the Rust coerces them. -/
def checkInvariants (m : Market) : Except Error Unit :=
  if m.Shape then
    if m.Solvent then .ok () else .error .invariantViolation
  else .error .invariantViolation

def requireActive (m : Market) : Except Error Unit :=
  match m.resolution with
  | .active => .ok ()
  | _ => .error .alreadyResolved

def requireResolved (m : Market) : Except Error PayoutVector :=
  match m.effectiveVector with
  | some v => .ok v
  | none => .error .notResolved

end Market

namespace Position

/-- Position shape: both sides carry one entry per active outcome, and every
entry is within the stored-amount bound. -/
def Ok (p : Position) (n : Nat) : Prop :=
  p.internal.length = n ∧ p.external.length = n ∧
  (∀ x ∈ p.internal, x ≤ amountMax) ∧ (∀ x ∈ p.external, x ≤ amountMax)

instance (p : Position) (n : Nat) : Decidable (p.Ok n) := by unfold Ok; infer_instance

def side (p : Position) : Side → List Amount
  | .internal => p.internal
  | .external => p.external

def withSide (p : Position) : Side → List Amount → Position
  | .internal, l => { p with internal := l }
  | .external, l => { p with external := l }

end Position

/-! ## Total helpers -/

-- `bumpAll` and `dropAll` (the uniform split/merge shifts) live in
-- `DragonsClutch.Basic` with their effect on the two solvency functionals.

/-- Read one outcome's entry; `none` when the index is out of range. -/
def entry (xs : List Amount) (i : Nat) : Option Amount := xs[i]?

/-- Write one outcome's entry, leaving the list unchanged if out of range
(the callers all guard the index first). -/
def setEntry (xs : List Amount) (i : Nat) (v : Amount) : List Amount := xs.set i v

/-! ## Transitions

Each transition takes the pre-state, refuses or returns the post-state, and
never mutates anything on a refusal — which in a pure model is not a discipline
but a type: a refusal returns no state at all. -/

namespace Market

/-- Construct an Active market with a frozen payout set and resolution seam. -/
def new (outcomes : Nat) (mode : BasisMode) (payouts : PayoutSet) (collateral : Amount) :
    Except Error Market :=
  let m : Market :=
    { outcomes := outcomes, basisMode := mode, payouts := payouts,
      resolution := .active, collateral := collateral,
      totalSupply := List.replicate outcomes 0 }
  if outcomes ≠ payouts.outcomes then .error .invalidOutcomeCount
  else if ¬ payouts.Valid then .error .invalidPayoutWeights
  else do
    m.checkInvariants
    .ok m

/-- Mint `q` internal claims of every outcome against `q` collateral atoms. -/
def split (m : Market) (p : Position) (q : Amount) :
    Except Error (Market × Position) := do
  m.checkInvariants
  if ¬ p.Ok m.outcomes then .error .invariantViolation else
  m.requireActive
  if q = 0 then .error .zeroQuantity else
  if ¬ (m.collateral + q ≤ amountMax) then .error .arithmeticOverflow else
  if ¬ (∀ t ∈ m.totalSupply, t + q ≤ amountMax) then .error .arithmeticOverflow else
  if ¬ (∀ x ∈ p.internal, x + q ≤ amountMax) then .error .arithmeticOverflow else
  let m' : Market :=
    { m with collateral := m.collateral + q, totalSupply := bumpAll q m.totalSupply }
  let p' : Position := { p with internal := bumpAll q p.internal }
  m'.checkInvariants
  .ok (m', p')

/-- Burn `q` internal claims of every outcome and release `q` collateral atoms.

The collateral test precedes the balance tests, which is the landed check order
`VECTOR_SPINE_PROPOSAL.md` R8 pins and the only reason
`insufficientCollateral` is observable here at all. -/
def merge (m : Market) (p : Position) (q : Amount) :
    Except Error (Market × Position) := do
  m.checkInvariants
  if ¬ p.Ok m.outcomes then .error .invariantViolation else
  m.requireActive
  if q = 0 then .error .zeroQuantity else
  if m.collateral < q then .error .insufficientCollateral else
  if ¬ (∀ x ∈ p.internal, q ≤ x) then .error .insufficientBalance else
  if ¬ (∀ t ∈ m.totalSupply, q ≤ t) then .error .insufficientBalance else
  let m' : Market :=
    { m with collateral := m.collateral - q, totalSupply := dropAll q m.totalSupply }
  let p' : Position := { p with internal := dropAll q p.internal }
  m'.checkInvariants
  .ok (m', p')

/-- Move `q` of one outcome from the internal side of a position to its
external (bearer) side.  Supply- and collateral-neutral. -/
def materialize (m : Market) (p : Position) (i : Nat) (q : Amount) :
    Except Error (Market × Position) := do
  m.checkInvariants
  if ¬ p.Ok m.outcomes then .error .invariantViolation else
  m.requireActive
  if q = 0 then .error .zeroQuantity else
  if i ≥ m.outcomes then .error .invalidPayoutIndex else
  match entry p.internal i, entry p.external i with
  | some inI, some exI =>
      if inI < q then .error .insufficientBalance else
      if ¬ (exI + q ≤ amountMax) then .error .arithmeticOverflow else
      let p' : Position :=
        { internal := setEntry p.internal i (inI - q),
          external := setEntry p.external i (exI + q) }
      m.checkInvariants
      .ok (m, p')
  | _, _ => .error .invalidPayoutIndex

/-- The inverse of `materialize`. -/
def dematerialize (m : Market) (p : Position) (i : Nat) (q : Amount) :
    Except Error (Market × Position) := do
  m.checkInvariants
  if ¬ p.Ok m.outcomes then .error .invariantViolation else
  m.requireActive
  if q = 0 then .error .zeroQuantity else
  if i ≥ m.outcomes then .error .invalidPayoutIndex else
  match entry p.internal i, entry p.external i with
  | some inI, some exI =>
      if exI < q then .error .insufficientBalance else
      if ¬ (inI + q ≤ amountMax) then .error .arithmeticOverflow else
      let p' : Position :=
        { internal := setEntry p.internal i (inI + q),
          external := setEntry p.external i (exI - q) }
      m.checkInvariants
      .ok (m, p')
  | _, _ => .error .invalidPayoutIndex

/-- Fix the payout vector by index into the immutable finite payout set
(`BasisMode.finitePreset` only). -/
def resolve (m : Market) (i : Nat) : Except Error Market := do
  m.checkInvariants
  m.requireActive
  if m.basisMode ≠ BasisMode.finitePreset then .error .wrongResolutionMode else
  if i ≥ m.payouts.vectors.length then .error .invalidPayoutIndex else
  let m' : Market := { m with resolution := .byIndex i }
  m'.checkInvariants
  .ok m'

/-- Fix the resolved payout to a derived, validated vector
(`BasisMode.derivedBasis` only).

The kernel checks shape, not provenance: exactly (H1) and (H2) against the
market's frozen `D`.  Binding the vector to evidence is the adapter's
derivation, as binding an index to evidence is in mode 0. -/
def resolveWithVector (m : Market) (v : PayoutVector) : Except Error Market := do
  m.checkInvariants
  m.requireActive
  if m.basisMode ≠ BasisMode.derivedBasis then .error .wrongResolutionMode else
  if v.denominator ≠ m.payouts.denominator then .error .invalidDenominator else
  if ¬ v.Admissible m.outcomes then .error .invalidPayoutWeights else
  let m' : Market := { m with resolution := .byVector v }
  m'.checkInvariants
  .ok m'

/-- Redeem `q` claims of one outcome from one side of a position after
resolution.  A payout that is not an exact number of atoms is refused, never
floored. -/
def redeem (m : Market) (p : Position) (side : Side) (i : Nat) (q : Amount) :
    Except Error (Market × Position × Amount) := do
  m.checkInvariants
  if ¬ p.Ok m.outcomes then .error .invariantViolation else
  let v ← m.requireResolved
  if q = 0 then .error .zeroQuantity else
  if i ≥ m.outcomes then .error .invalidPayoutIndex else
  match entry (p.side side) i, entry m.totalSupply i, entry v.weights i with
  | some avail, some supply, some w =>
      if avail < q then .error .insufficientBalance else
      if supply < q then .error .insufficientBalance else
      if q * w % v.denominator ≠ 0 then .error .remainderRequired else
      let payout := q * w / v.denominator
      if m.collateral < payout then .error .insufficientCollateral else
      let m' : Market :=
        { m with collateral := m.collateral - payout,
                 totalSupply := setEntry m.totalSupply i (supply - q) }
      let p' : Position := p.withSide side (setEntry (p.side side) i (avail - q))
      m'.checkInvariants
      .ok (m', p', payout)
  | _, _, _ => .error .invalidPayoutIndex

/-- Redeem `q` complete sets after resolution: burn `q` internal claims of every
active outcome and pay exactly the per-set collateral.

Unlike `merge`, the balance tests precede the collateral test, so
`insufficientCollateral` is unreachable defence in depth on this path — the
deliberate divergence recorded in the Rust's own docs. -/
def redeemCompleteSet (m : Market) (p : Position) (q : Amount) :
    Except Error (Market × Position × Amount) := do
  m.checkInvariants
  if ¬ p.Ok m.outcomes then .error .invariantViolation else
  let v ← m.requireResolved
  if q = 0 then .error .zeroQuantity else
  if ¬ (∀ x ∈ p.internal, q ≤ x) then .error .insufficientBalance else
  if ¬ (∀ t ∈ m.totalSupply, q ≤ t) then .error .insufficientBalance else
  if liability (List.replicate m.outcomes q) v % v.denominator ≠ 0 then
    .error .remainderRequired else
  let payout := liability (List.replicate m.outcomes q) v / v.denominator
  if payout ≠ q then .error .invariantViolation else
  if m.collateral < payout then .error .insufficientCollateral else
  let m' : Market :=
    { m with collateral := m.collateral - payout, totalSupply := dropAll q m.totalSupply }
  let p' : Position := { p with internal := dropAll q p.internal }
  m'.checkInvariants
  .ok (m', p', payout)

/-- Move `q` of one outcome's internal claims from one position to another.
Supply- and collateral-neutral by construction: the market is read, never
written. -/
def transferInternal (m : Market) (from_ to : Position) (i : Nat) (q : Amount)
    (policy : TransferPhasePolicy) : Except Error (Position × Position) := do
  m.checkInvariants
  if ¬ from_.Ok m.outcomes then .error .invariantViolation else
  if ¬ to.Ok m.outcomes then .error .invariantViolation else
  match policy with
  | .activeOnly => m.requireActive
  | .activeOrResolved => pure ()
  if q = 0 then .error .zeroQuantity else
  if i ≥ m.outcomes then .error .invalidPayoutIndex else
  match entry from_.internal i, entry to.internal i with
  | some f, some t =>
      if f < q then .error .insufficientBalance else
      if ¬ (t + q ≤ amountMax) then .error .arithmeticOverflow else
      .ok ({ from_ with internal := setEntry from_.internal i (f - q) },
           { to with internal := setEntry to.internal i (t + q) })
  | _, _ => .error .invalidPayoutIndex

end Market

end DragonsClutch
