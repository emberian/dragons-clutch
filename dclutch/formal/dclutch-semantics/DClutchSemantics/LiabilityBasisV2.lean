import DClutchSemantics.ProductPayoffV2
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Runtime-width nonnegative liability bases

This module separates the economic theorem for a finite nonnegative integer
partition of unity from any fixed physical width.  A basis evaluation returns
one nonnegative integer payout per elementary claim and the payouts sum to the
positive collateral scale `Q`.

`cappedRampComplementFloorBoundaryV2` is the sole apportionment boundary in the
two-claim ramp profile.  It is definitionally the Product V2 final interpolation
floor.  The second claim receives the exact integer complement, so no second
rounding decision or unclassified residue exists.

The physical Rust profile uses bounded integers and a fixed hostile-decodable
request.  Those physical bounds are not premises of the mathematical theorems
below.
-/

namespace DClutch.LiabilityBasisV2

/-- Runtime-width dot product. A physical caller must prove equal lengths. -/
def liability : List Nat → List Nat → Nat
  | supply :: supplies, payout :: payouts =>
      supply * payout + liability supplies payouts
  | _, _ => 0

/-- Add the same complete-set quantity to every elementary claim supply. -/
def splitSupply (quantity : Nat) (supplies : List Nat) : List Nat :=
  supplies.map (fun supply => supply + quantity)

/-- Semantic contract for one finite nonnegative integer partition of unity. -/
structure Basis (Result : Type) where
  width : Nat
  scale : Nat
  widthPositive : 0 < width
  scalePositive : 0 < scale
  evaluate : Result → List Nat
  exactWidth : ∀ result, (evaluate result).length = width
  payoutBounded : ∀ result payout, payout ∈ evaluate result → payout ≤ scale
  partitionUnity : ∀ result, (evaluate result).sum = scale

theorem liability_split
    (quantity : Nat) (supplies payouts : List Nat)
    (sameWidth : supplies.length = payouts.length) :
    liability (splitSupply quantity supplies) payouts =
      liability supplies payouts + quantity * payouts.sum := by
  induction supplies generalizing payouts with
  | nil =>
      cases payouts <;> simp_all [liability, splitSupply]
  | cons supply supplies induction =>
      cases payouts with
      | nil => simp at sameWidth
      | cons payout payouts =>
          have tailWidth : supplies.length = payouts.length := by
            simpa using sameWidth
          simp only [splitSupply, List.map_cons, liability, List.sum_cons]
          change (supply + quantity) * payout +
              liability (splitSupply quantity supplies) payouts = _
          rw [induction payouts tailWidth]
          simp only [Nat.add_mul, Nat.mul_add]
          omega

/-- A complete-set split increases liability at every result by exactly
`quantity * Q`; no maximization argument or categorical one-hot premise is
needed. -/
theorem Basis.liability_split
    (basis : Basis Result) (result : Result)
    (quantity : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width) :
    liability (splitSupply quantity supplies) (basis.evaluate result) =
      liability supplies (basis.evaluate result) + quantity * basis.scale := by
  rw [DClutch.LiabilityBasisV2.liability_split
    quantity supplies (basis.evaluate result)]
  · rw [basis.partitionUnity result]
  · rw [basis.exactWidth result]
    exact sameWidth

/-- Pointwise collateralization at one result.  Global solvency quantifies this
predicate over the complete terminal-result domain. -/
def SolventAt (hoard : Nat) (supplies payouts : List Nat) : Prop :=
  liability supplies payouts ≤ hoard

/-- Solvency over the complete terminal-result domain, without assuming that
the domain is enumerable in this theorem. -/
def Basis.GloballySolvent
    (basis : Basis Result) (hoard : Nat) (supplies : List Nat) : Prop :=
  ∀ result, SolventAt hoard supplies (basis.evaluate result)

/-- Crediting `quantity * Q` collateral alongside a complete-set split
preserves pointwise solvency for every result. -/
theorem Basis.split_preserves_solvency
    (basis : Basis Result) (result : Result)
    (quantity hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width)
    (solvent : SolventAt hoard supplies (basis.evaluate result)) :
    SolventAt (hoard + quantity * basis.scale)
      (splitSupply quantity supplies) (basis.evaluate result) := by
  unfold SolventAt at solvent ⊢
  rw [basis.liability_split result quantity supplies sameWidth]
  exact Nat.add_le_add_right solvent _

/-- The same exact split/collateral delta preserves the liability bound at
every terminal result simultaneously. -/
theorem Basis.split_preserves_global_solvency
    (basis : Basis Result) (quantity hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width)
    (solvent : basis.GloballySolvent hoard supplies) :
    basis.GloballySolvent (hoard + quantity * basis.scale)
      (splitSupply quantity supplies) := by
  intro result
  exact basis.split_preserves_solvency result quantity hoard supplies
    sameWidth (solvent result)

/-! ## Categorical embedding -/

/-- Runtime-width categorical one-hot payout. Out-of-range defensive indices
produce all zeros; `Fin width` construction rules them out below. -/
def categoricalPayoutAt : Nat → Nat → List Nat
  | 0, _ => []
  | width + 1, 0 => 1 :: List.replicate width 0
  | width + 1, winner + 1 => 0 :: categoricalPayoutAt width winner

def categoricalPayout (width : Nat) (winner : Fin width) : List Nat :=
  categoricalPayoutAt width winner.val

theorem categoricalPayoutAt_length (width winner : Nat) :
    (categoricalPayoutAt width winner).length = width := by
  induction width generalizing winner with
  | zero => simp [categoricalPayoutAt]
  | succ width induction =>
      cases winner with
      | zero => simp [categoricalPayoutAt]
      | succ winner => simp [categoricalPayoutAt, induction]

theorem categoricalPayoutAt_bounded
    (width winner payout : Nat)
    (member : payout ∈ categoricalPayoutAt width winner) : payout ≤ 1 := by
  induction width generalizing winner with
  | zero => simp [categoricalPayoutAt] at member
  | succ width induction =>
      cases winner with
      | zero =>
          simp only [categoricalPayoutAt, List.mem_cons,
            List.mem_replicate] at member
          rcases member with rfl | ⟨_, rfl⟩ <;> omega
      | succ winner =>
          simp only [categoricalPayoutAt, List.mem_cons] at member
          rcases member with rfl | member
          · omega
          · exact induction winner member

theorem categoricalPayoutAt_sum
    (width winner : Nat) (inRange : winner < width) :
    (categoricalPayoutAt width winner).sum = 1 := by
  induction width generalizing winner with
  | zero => omega
  | succ width induction =>
      cases winner with
      | zero => simp [categoricalPayoutAt]
      | succ winner =>
          simp only [categoricalPayoutAt, List.sum_cons, Nat.zero_add]
          exact induction winner (by omega)

theorem categoricalPayout_length (width : Nat) (winner : Fin width) :
    (categoricalPayout width winner).length = width := by
  exact categoricalPayoutAt_length width winner.val

theorem categoricalPayout_bounded
    (width : Nat) (winner : Fin width) (payout : Nat)
    (member : payout ∈ categoricalPayout width winner) : payout ≤ 1 := by
  exact categoricalPayoutAt_bounded width winner.val payout member

theorem categoricalPayout_sum (width : Nat) (winner : Fin width) :
    (categoricalPayout width winner).sum = 1 := by
  exact categoricalPayoutAt_sum width winner.val winner.isLt

/-- Categorical claims embed exactly as the `Q = 1` one-hot basis. -/
def categoricalBasis (width : Nat) (widthPositive : 0 < width) : Basis (Fin width) := {
  width
  scale := 1
  widthPositive
  scalePositive := by omega
  evaluate := categoricalPayout width
  exactWidth := categoricalPayout_length width
  payoutBounded := categoricalPayout_bounded width
  partitionUnity := categoricalPayout_sum width
}

/-! ## Two-claim capped ramp and exact complement -/

abbrev RationalCoordinate := DClutch.ProductV2.RationalCoordinate

/-- A two-claim capped-ramp profile. Knots are exact signed numerators over one
positive common denominator. -/
structure CappedRampComplement where
  scale : Nat
  knotDenominator : Nat
  leftNumerator : Int
  rightNumerator : Int
  scalePositive : 0 < scale
  knotDenominatorPositive : 0 < knotDenominator
  knotsOrdered : leftNumerator < rightNumerator

/-- **The sole capped-ramp apportionment boundary.** This is definitionally the
Product V2 final interpolation floor, merely given its liability-basis name. -/
def cappedRampComplementFloorBoundaryV2
    (scale : Nat) (elapsed width : Int) : Nat :=
  DClutch.ProductV2.interpolationFloor scale elapsed width

theorem cappedRampComplementFloorBoundaryV2_le
    (scale : Nat) (elapsed width : Int) :
    cappedRampComplementFloorBoundaryV2 scale elapsed width ≤ scale := by
  exact DClutch.ProductV2.interpolationFloor_le scale elapsed width

def scaledCoordinate
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) : Int :=
  coordinate.numerator * profile.knotDenominator

def scaledKnot
    (coordinate : RationalCoordinate) (numerator : Int) : Int :=
  numerator * coordinate.denominator

/-- Primary capped-ramp payout. Defensive zero-denominator input remains total;
physical admission rejects it before evaluation. -/
def CappedRampComplement.primary
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) : Nat :=
  let observed := scaledCoordinate profile coordinate
  let left := scaledKnot coordinate profile.leftNumerator
  let right := scaledKnot coordinate profile.rightNumerator
  if observed ≤ left then 0
  else if right ≤ observed then profile.scale
  else cappedRampComplementFloorBoundaryV2 profile.scale
    (observed - left) (right - left)

theorem CappedRampComplement.primary_le
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) :
    profile.primary coordinate ≤ profile.scale := by
  unfold CappedRampComplement.primary
  simp only
  split
  · omega
  · split
    · omega
    · exact cappedRampComplementFloorBoundaryV2_le _ _ _

/-- Exact two-claim payout. The complement receives all integer atoms not
assigned by the single floor boundary. -/
def CappedRampComplement.evaluate
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) : List Nat :=
  let primary := profile.primary coordinate
  [primary, profile.scale - primary]

theorem CappedRampComplement.evaluate_length
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) :
    (profile.evaluate coordinate).length = 2 := by
  simp [CappedRampComplement.evaluate]

theorem CappedRampComplement.evaluate_bounded
    (profile : CappedRampComplement) (coordinate : RationalCoordinate)
    (payout : Nat) (member : payout ∈ profile.evaluate coordinate) :
    payout ≤ profile.scale := by
  simp only [CappedRampComplement.evaluate, List.mem_cons] at member
  rcases member with rfl | member
  · exact profile.primary_le coordinate
  · rcases member with rfl | impossible
    · exact Nat.sub_le _ _
    · contradiction

theorem CappedRampComplement.evaluate_partition
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) :
    (profile.evaluate coordinate).sum = profile.scale := by
  simp only [CappedRampComplement.evaluate, List.sum_cons, List.sum_nil,
    Nat.add_zero]
  exact Nat.add_sub_of_le (profile.primary_le coordinate)

/-- The capped ramp and its exact complement form a width-two `Q`-scaled
nonnegative liability basis. -/
def CappedRampComplement.basis
    (profile : CappedRampComplement) : Basis RationalCoordinate := {
  width := 2
  scale := profile.scale
  widthPositive := by omega
  scalePositive := profile.scalePositive
  evaluate := profile.evaluate
  exactWidth := profile.evaluate_length
  payoutBounded := profile.evaluate_bounded
  partitionUnity := profile.evaluate_partition
}

/-! ## Provisional exact physical profile

The 64-byte request is only the first measured differential profile.  Its
`i64` numerators and `u32` positive scales/denominators are provisional
representation bounds, not premises of `Basis` or its preservation theorems.
-/

namespace PhysicalAbi

def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x4c, 0x42, 0x56, 0x32]
def requestBytes : Nat := 64
def schemaVersion : Nat := 2
def profile : Nat := 1

def magicOffset : Nat := 0
def versionOffset : Nat := 8
def profileOffset : Nat := 10
def scaleOffset : Nat := 12
def knotDenominatorOffset : Nat := 16
def leftNumeratorOffset : Nat := 20
def rightNumeratorOffset : Nat := 28
def coordinateNumeratorOffset : Nat := 36
def coordinateDenominatorOffset : Nat := 44
def reservedOffset : Nat := 48
def reservedBytes : Nat := 16

structure Request where
  scale : Nat
  knotDenominator : Nat
  leftNumerator : Int
  rightNumerator : Int
  coordinateNumerator : Int
  coordinateDenominator : Nat
  deriving DecidableEq, Repr

def i64Min : Int := -(2 ^ 63)
def i64Max : Int := 2 ^ 63 - 1
def u32Limit : Nat := 2 ^ 32

def Request.physicallyRepresentable (request : Request) : Bool :=
  request.scale < u32Limit && request.knotDenominator < u32Limit &&
    request.coordinateDenominator < u32Limit &&
    i64Min ≤ request.leftNumerator && request.leftNumerator ≤ i64Max &&
    i64Min ≤ request.rightNumerator && request.rightNumerator ≤ i64Max &&
    i64Min ≤ request.coordinateNumerator && request.coordinateNumerator ≤ i64Max

/-- Semantic evaluation after the exact positive/ordered physical premises are
checked. This owns the generated agreement corpus, not Rust execution. -/
def Request.evaluate? (request : Request) : Option (List Nat) :=
  if _represented : request.physicallyRepresentable then
    if scalePositive : 0 < request.scale then
      if denominatorPositive : 0 < request.knotDenominator then
        if _coordinateDenominatorPositive : 0 < request.coordinateDenominator then
          if knotsOrdered : request.leftNumerator < request.rightNumerator then
            let profile : CappedRampComplement := {
              scale := request.scale
              knotDenominator := request.knotDenominator
              leftNumerator := request.leftNumerator
              rightNumerator := request.rightNumerator
              scalePositive
              knotDenominatorPositive := denominatorPositive
              knotsOrdered
            }
            some (profile.evaluate {
              numerator := request.coordinateNumerator
              denominator := request.coordinateDenominator
            })
          else none
        else none
      else none
    else none
  else none

/-- Two's-complement low 64 bits. Physical corpus inputs separately fit `i64`. -/
def encodeI64 (value : Int) : List UInt8 :=
  let bits := if 0 ≤ value then value.toNat else ((2 ^ 64 : Int) + value).toNat
  DClutch.Codec.encodeLE 8 bits

def encodeRequest (request : Request) : List UInt8 :=
  requestMagic ++
    DClutch.Codec.encodeLE 2 schemaVersion ++
    DClutch.Codec.encodeLE 2 profile ++
    DClutch.Codec.encodeLE 4 request.scale ++
    DClutch.Codec.encodeLE 4 request.knotDenominator ++
    encodeI64 request.leftNumerator ++
    encodeI64 request.rightNumerator ++
    encodeI64 request.coordinateNumerator ++
    DClutch.Codec.encodeLE 4 request.coordinateDenominator ++
    List.replicate reservedBytes 0

/-- Decode one signed two's-complement `i64` field from exactly eight bytes. -/
def decodeI64 (bytes : List UInt8) : Int :=
  let bits := DClutch.Codec.decodeLE bytes
  if bits < 2 ^ 63 then Int.ofNat bits else Int.ofNat bits - 2 ^ 64

def field (bytes : List UInt8) (offset width : Nat) : List UInt8 :=
  (bytes.drop offset).take width

/-- Hostile semantic decoder used to own the generated refusal corpus. Error
tags match the handwritten Rust kernel: length `0`, magic `1`, schema `2`,
profile `3`, reserved `4`, scale `5`, denominator `6`, knot order `7`. -/
def decodeRequest (bytes : List UInt8) : Except Nat Request := do
  if bytes.length != requestBytes then throw 0
  if field bytes magicOffset requestMagic.length != requestMagic then throw 1
  if DClutch.Codec.decodeLE (field bytes versionOffset 2) != schemaVersion then throw 2
  if DClutch.Codec.decodeLE (field bytes profileOffset 2) != profile then throw 3
  if !(field bytes reservedOffset reservedBytes).all (fun byte => byte == 0) then throw 4
  let request : Request := {
    scale := DClutch.Codec.decodeLE (field bytes scaleOffset 4)
    knotDenominator := DClutch.Codec.decodeLE (field bytes knotDenominatorOffset 4)
    leftNumerator := decodeI64 (field bytes leftNumeratorOffset 8)
    rightNumerator := decodeI64 (field bytes rightNumeratorOffset 8)
    coordinateNumerator := decodeI64 (field bytes coordinateNumeratorOffset 8)
    coordinateDenominator :=
      DClutch.Codec.decodeLE (field bytes coordinateDenominatorOffset 4)
  }
  if request.scale = 0 then throw 5
  if request.knotDenominator = 0 || request.coordinateDenominator = 0 then throw 6
  if request.leftNumerator >= request.rightNumerator then throw 7
  pure request

theorem encodeI64_length (value : Int) : (encodeI64 value).length = 8 := by
  simp [encodeI64, DClutch.Codec.encodeLE_length]

theorem encodeRequest_length (request : Request) :
    (encodeRequest request).length = requestBytes := by
  simp [encodeRequest, requestMagic, requestBytes, reservedBytes,
    DClutch.Codec.encodeLE_length, encodeI64_length]

end PhysicalAbi

end DClutch.LiabilityBasisV2
