import DClutchSemantics.LiabilityBasisV2Spline

/-!
# Provisional exact physical profile for the B-spline liability basis

`LiabilityBasisV2Spline` quantifies over unbounded `Nat` and `Int`.  This
module pins one hostile-decodable byte record for it and owns the generated
translation corpus.

The record is exactly 144 bytes: profile `2` of schema `2`, alongside the
ramp's profile `1`.  It carries a `u32` payout scale and denominators, an
`i64` coordinate numerator, a degree, and up to twelve `i64` knots over one
common denominator.  The widest basis the record can express is therefore ten
claims, at degree one.

Those are physical representation bounds, not mathematical basis-width or
degree limits, and they are not premises of any theorem in
`LiabilityBasisV2Spline`.

Two admission checks here have no counterpart among the pure theorems, and
that is deliberate:

* **Knots must be nondecreasing.** The evaluator is total on any knot list and
  the partition theorem never needs the order.  An unordered knot vector is
  still semantically meaningless, so the physical boundary refuses it rather
  than returning a well-formed partition of a nonsense basis.
* **Padding must be canonical.** Reserved bytes and the inactive knot tail
  must be zero, so one basis has exactly one encoding.

Refusal tags extend the kernel's stable set. Tags `0`-`7` are shared with the
ramp profile where they mean the same thing; `15`-`19` are new and specific to
this record.
-/

namespace DClutch.LiabilityBasisV2.Spline.PhysicalAbi

open DClutch.LiabilityBasisV2
open DClutch.LiabilityBasisV2.Spline

/-- Nondecreasing knots. Equality is admitted: that is knot multiplicity. -/
def nondecreasing : List Int → Bool
  | [] => true
  | [_] => true
  | first :: second :: rest => decide (first ≤ second) && nondecreasing (second :: rest)

def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x4c, 0x42, 0x56, 0x32]
def requestBytes : Nat := 144
def schemaVersion : Nat := 2
def profileTag : Nat := 2

/-- Physical knot capacity. Not a mathematical bound on the basis. -/
def maxKnots : Nat := 12

/-! ### The admitted degree range

`SplineProfile` already carries `0 < degree` and `degree ≤ 3` as fields. These
name the same two bounds so that the physical record, the emitted ABI and the
Rust kernel all read them from one place instead of restating `1` and `3`.
They are load-bearing: `Request.WellFormed` and the tag-`15` decoder check are
both stated in terms of them. -/
def minDegree : Nat := 1

def maxDegree : Nat := 3

/-- The de Boor triangle's widest level: `degree + 1` locally supported claims.
This is the physical stack capacity the kernel allocates for one evaluation. -/
def maxSupport : Nat := maxDegree + 1

/-- Widest basis this record can express: `maxKnots - degree - 1` at degree
one. -/
def maxWidth : Nat := 10

def magicOffset : Nat := 0
def versionOffset : Nat := 8
def profileOffset : Nat := 10
def scaleOffset : Nat := 12
def knotDenominatorOffset : Nat := 16
def coordinateDenominatorOffset : Nat := 20
def coordinateNumeratorOffset : Nat := 24
def degreeOffset : Nat := 32
def knotCountOffset : Nat := 33
def reservedOffset : Nat := 34
def reservedBytes : Nat := 14
def knotsOffset : Nat := 48
def knotBytes : Nat := 8

structure Request where
  scale : Nat
  knotDenominator : Nat
  coordinateDenominator : Nat
  coordinateNumerator : Int
  degree : Nat
  knotCount : Nat
  knots : List Int
  deriving DecidableEq, Repr

/-- The knot vector the basis actually uses. -/
def Request.activeKnots (request : Request) : List Int :=
  request.knots.take request.knotCount

/-- **Exact physical admission premises.** Every conjunct is decidable, so the
evaluator below is a total function of the decoded record. -/
def Request.WellFormed (request : Request) : Prop :=
  0 < request.scale ∧ 0 < request.knotDenominator ∧
    0 < request.coordinateDenominator ∧
    minDegree ≤ request.degree ∧ request.degree ≤ maxDegree ∧
    2 * request.degree + 2 ≤ request.activeKnots.length ∧
    request.knotCount ≤ maxKnots ∧ nondecreasing request.activeKnots = true

instance (request : Request) : Decidable request.WellFormed := by
  unfold Request.WellFormed
  infer_instance

def Request.coordinate (request : Request) : Spline.RationalCoordinate :=
  { numerator := request.coordinateNumerator,
    denominator := request.coordinateDenominator }

/-- The basis a well-formed record names. -/
def Request.profileOf (request : Request) (wellFormed : request.WellFormed) :
    SplineProfile := {
  degree := request.degree
  scale := request.scale
  knotDenominator := request.knotDenominator
  knots := request.activeKnots
  degreePositive := wellFormed.2.2.2.1
  degreeBounded := wellFormed.2.2.2.2.1
  scalePositive := wellFormed.1
  knotDenominatorPositive := wellFormed.2.1
  enoughKnots := wellFormed.2.2.2.2.2.1
}

/-- Semantic evaluation after the exact physical premises are checked. This
owns the generated agreement corpus; it is not Rust execution. -/
def Request.evaluate? (request : Request) : Option (List Nat) :=
  if wellFormed : request.WellFormed then
    if (request.profileOf wellFormed).admits request.coordinate then
      some ((request.profileOf wellFormed).evaluate request.coordinate)
    else none
  else none

/-- The runtime claim width a well-formed record names. -/
def Request.width (request : Request) : Nat :=
  request.activeKnots.length - request.degree - 1

/-! ### Totality and the exact partition at the physical boundary -/

/-- **Evaluator totality.** Evaluation succeeds on exactly the records that are
well formed and whose located span is a real one: no admitted terminal result
is left without a payout, and no inadmissible record silently receives one. -/
theorem Request.evaluate?_isSome_iff (request : Request) :
    request.evaluate?.isSome = true ↔
      ∃ wellFormed : request.WellFormed,
        (request.profileOf wellFormed).admits request.coordinate = true := by
  unfold Request.evaluate?
  by_cases wellFormed : request.WellFormed
  · by_cases admitted : (request.profileOf wellFormed).admits request.coordinate = true
    · simp [wellFormed, admitted]
    · simp only [dif_pos wellFormed, if_neg admitted]
      refine ⟨fun contra => by simp at contra, fun existence => ?_⟩
      obtain ⟨_, holds⟩ := existence
      exact absurd holds admitted
  · simp [wellFormed]

theorem Request.evaluate?_eq_profile
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) :
    ∃ wellFormed : request.WellFormed,
      (request.profileOf wellFormed).admits request.coordinate = true ∧
        payouts = (request.profileOf wellFormed).evaluate request.coordinate := by
  unfold Request.evaluate? at evaluated
  split at evaluated
  · rename_i wellFormed
    split at evaluated
    · rename_i admitted
      exact ⟨wellFormed, admitted, by simpa using evaluated.symm⟩
    · simp at evaluated
  · simp at evaluated

/-- **Exact partition sum at the physical boundary.** Every payout vector the
byte record can produce sums to exactly its own named scale. -/
theorem Request.evaluate?_partition
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) :
    payouts.sum = request.scale := by
  obtain ⟨wellFormed, admitted, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  exact (request.profileOf wellFormed).evaluate_partition request.coordinate admitted

theorem Request.evaluate?_length
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) :
    payouts.length = request.width := by
  obtain ⟨wellFormed, admitted, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  exact (request.profileOf wellFormed).evaluate_length request.coordinate admitted

theorem Request.evaluate?_bounded
    (request : Request) (payouts : List Nat) (payout : Nat)
    (evaluated : request.evaluate? = some payouts) (member : payout ∈ payouts) :
    payout ≤ request.scale := by
  obtain ⟨wellFormed, admitted, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  exact (request.profileOf wellFormed).evaluate_bounded request.coordinate admitted
    payout member

/-- The hostile partition checker never refuses a payout vector the byte record
produced. -/
theorem Request.evaluate?_validPartition
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) :
    validPartition payouts request.scale = true := by
  obtain ⟨wellFormed, admitted, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  exact (request.profileOf wellFormed).validPartition_evaluate ⟨request.coordinate, admitted⟩

/-- **`maxSupport` is a real bound, not a decoration.** Every admitted record's
de Boor triangle fits in `degree + 1 ≤ maxSupport` values, which is the stack
capacity the physical kernel allocates for one evaluation. -/
theorem Request.localNumerators_length_le_maxSupport
    (request : Request) (wellFormed : request.WellFormed)
    (admitted : (request.profileOf wellFormed).admits request.coordinate = true) :
    ((request.profileOf wellFormed).localNumerators request.coordinate).length
      ≤ maxSupport := by
  rw [(request.profileOf wellFormed).localNumerators_length request.coordinate admitted]
  have bound : request.degree ≤ maxDegree := wellFormed.2.2.2.2.1
  have degreeIs : (request.profileOf wellFormed).degree = request.degree := rfl
  simp only [maxSupport, degreeIs]
  omega

/-! ### The byte record -/

open DClutch.LiabilityBasisV2.PhysicalAbi (encodeI64 decodeI64 field)

def encodeKnots : List Int → List UInt8
  | [] => []
  | knot :: rest => encodeI64 knot ++ encodeKnots rest

def encodeRequest (request : Request) : List UInt8 :=
  requestMagic ++
    DClutch.Codec.encodeLE 2 schemaVersion ++
    DClutch.Codec.encodeLE 2 profileTag ++
    DClutch.Codec.encodeLE 4 request.scale ++
    DClutch.Codec.encodeLE 4 request.knotDenominator ++
    DClutch.Codec.encodeLE 4 request.coordinateDenominator ++
    encodeI64 request.coordinateNumerator ++
    DClutch.Codec.encodeLE 1 request.degree ++
    DClutch.Codec.encodeLE 1 request.knotCount ++
    List.replicate reservedBytes 0 ++
    encodeKnots (request.knots.take maxKnots) ++
    List.replicate ((maxKnots - request.knots.length) * knotBytes) 0

/-- Decode all twelve knot slots; the inactive tail is separately required to
be canonical zero. -/
def projectKnots (bytes : List UInt8) : List Int :=
  (List.range maxKnots).map (fun slot =>
    decodeI64 (field bytes (knotsOffset + slot * knotBytes) knotBytes))

/-- The exact field projection the hostile decoder performs before it applies
any semantic guard. -/
def projectRequest (bytes : List UInt8) : Request := {
  scale := DClutch.Codec.decodeLE (field bytes scaleOffset 4)
  knotDenominator := DClutch.Codec.decodeLE (field bytes knotDenominatorOffset 4)
  coordinateDenominator :=
    DClutch.Codec.decodeLE (field bytes coordinateDenominatorOffset 4)
  coordinateNumerator := decodeI64 (field bytes coordinateNumeratorOffset 8)
  degree := DClutch.Codec.decodeLE (field bytes degreeOffset 1)
  knotCount := DClutch.Codec.decodeLE (field bytes knotCountOffset 1)
  knots := projectKnots bytes
}

/-- The inactive knot slots, which must be canonical zero. -/
def knotPadding (bytes : List UInt8) : List UInt8 :=
  (bytes.drop (knotsOffset + (projectRequest bytes).knotCount * knotBytes)).take
    ((maxKnots - (projectRequest bytes).knotCount) * knotBytes)

/-- **Hostile decoder checks, in order.** The first failing check names the
refusal tag, so this ORDER is part of the translation contract the Rust kernel
must reproduce.

Shared with the ramp profile: length `0`, magic `1`, schema `2`, profile `3`,
reserved `4`, scale `5`, denominator `6`.

New to this record: unsupported degree `15`, knot count out of range `16`,
non-canonical knot padding `17`, knots not nondecreasing `18`, and `19` for a
record whose located span is degenerate or outside the domain — the one
refusal that depends on the coordinate rather than on the basis. -/
def decodeChecks (bytes : List UInt8) : List (Nat × Bool) := [
  (0, decide (bytes.length = requestBytes)),
  (1, decide (field bytes magicOffset requestMagic.length = requestMagic)),
  (2, decide (DClutch.Codec.decodeLE (field bytes versionOffset 2) = schemaVersion)),
  (3, decide (DClutch.Codec.decodeLE (field bytes profileOffset 2) = profileTag)),
  (4, (field bytes reservedOffset reservedBytes).all (fun byte => byte == 0)),
  (5, decide (0 < (projectRequest bytes).scale)),
  (6, decide (0 < (projectRequest bytes).knotDenominator) &&
      decide (0 < (projectRequest bytes).coordinateDenominator)),
  (15, decide (minDegree ≤ (projectRequest bytes).degree) &&
      decide ((projectRequest bytes).degree ≤ maxDegree)),
  (16, decide (2 * (projectRequest bytes).degree + 2 ≤ (projectRequest bytes).knotCount) &&
      decide ((projectRequest bytes).knotCount ≤ maxKnots)),
  (17, (knotPadding bytes).all (fun byte => byte == 0)),
  (18, nondecreasing (projectRequest bytes).activeKnots),
  (19, (projectRequest bytes).evaluate?.isSome)
]

/-- The tag of the first failing check, or `none` when the record is accepted. -/
def refusal? (bytes : List UInt8) : Option Nat :=
  ((decodeChecks bytes).find? (fun check => !check.2)).map Prod.fst

/-- **The hostile semantic decoder.** -/
def decodeRequest (bytes : List UInt8) : Except Nat Request :=
  match refusal? bytes with
  | some tag => .error tag
  | none => .ok (projectRequest bytes)

theorem refusal_none_check
    (bytes : List UInt8) (check : Nat × Bool)
    (accepted : refusal? bytes = none)
    (member : check ∈ decodeChecks bytes) : check.2 = true := by
  unfold refusal? at accepted
  rw [Option.map_eq_none_iff, List.find?_eq_none] at accepted
  simpa using accepted check member

theorem refusal_none_getElem
    (bytes : List UInt8) (index : Nat)
    (bound : index < (decodeChecks bytes).length)
    (accepted : refusal? bytes = none) :
    ((decodeChecks bytes)[index]).2 = true :=
  refusal_none_check bytes _ accepted (List.getElem_mem bound)

theorem decodeRequest_ok_accepted
    (bytes : List UInt8) (request : Request)
    (decoded : decodeRequest bytes = .ok request) :
    refusal? bytes = none ∧ request = projectRequest bytes := by
  unfold decodeRequest at decoded
  split at decoded
  · simp at decoded
  · rename_i accepted
    exact ⟨accepted, by simpa using decoded.symm⟩

/-- **Hostile decode is total into evaluation.** Every accepted record has an
exact payout vector; there is no accepted-but-unevaluable input. -/
theorem decodeRequest_evaluates
    (bytes : List UInt8) (request : Request)
    (decoded : decodeRequest bytes = .ok request) :
    request.evaluate?.isSome = true := by
  obtain ⟨accepted, rfl⟩ := decodeRequest_ok_accepted bytes request decoded
  have check := refusal_none_getElem bytes 11 (by simp [decodeChecks]) accepted
  simpa [decodeChecks] using check

/-- Any input that is not exactly the canonical physical width is refused with
the stable length tag; no short or long record is ever partially decoded. -/
theorem decodeRequest_refuses_wrong_length
    (bytes : List UInt8) (wrongLength : bytes.length ≠ requestBytes) :
    decodeRequest bytes = .error 0 := by
  unfold decodeRequest refusal? decodeChecks
  simp [wrongLength, List.find?]

/-- Every accepted record names a degree inside the admitted family. -/
theorem decodeRequest_degree_in_range
    (bytes : List UInt8) (request : Request)
    (decoded : decodeRequest bytes = .ok request) :
    minDegree ≤ request.degree ∧ request.degree ≤ maxDegree := by
  obtain ⟨accepted, rfl⟩ := decodeRequest_ok_accepted bytes request decoded
  have check := refusal_none_getElem bytes 7 (by simp [decodeChecks]) accepted
  simp only [decodeChecks, List.getElem_cons_succ, List.getElem_cons_zero,
    Bool.and_eq_true, decide_eq_true_eq] at check
  exact check

/-- Every accepted record's knots really are nondecreasing, so no accepted
basis is a nonsense one. -/
theorem decodeRequest_ordered_knots
    (bytes : List UInt8) (request : Request)
    (decoded : decodeRequest bytes = .ok request) :
    nondecreasing request.activeKnots = true := by
  obtain ⟨accepted, rfl⟩ := decodeRequest_ok_accepted bytes request decoded
  have check := refusal_none_getElem bytes 10 (by simp [decodeChecks]) accepted
  simpa [decodeChecks] using check

/-- **Every accepted record's payouts sum to exactly its named scale.** The
end-to-end statement joining hostile decoding to the partition theorem. -/
theorem decodeRequest_partition
    (bytes : List UInt8) (request : Request) (payouts : List Nat)
    (_decoded : decodeRequest bytes = .ok request)
    (evaluated : request.evaluate? = some payouts) :
    payouts.sum = request.scale :=
  request.evaluate?_partition payouts evaluated

theorem encodeKnots_length (knots : List Int) :
    (encodeKnots knots).length = knots.length * knotBytes := by
  induction knots with
  | nil => rfl
  | cons knot knots induction =>
      simp only [encodeKnots, List.length_append, List.length_cons,
        DClutch.LiabilityBasisV2.PhysicalAbi.encodeI64_length, induction]
      simp [knotBytes]
      omega

theorem encodeRequest_length
    (request : Request) (capacity : request.knots.length = maxKnots) :
    (encodeRequest request).length = requestBytes := by
  simp only [encodeRequest, List.length_append, List.length_replicate,
    requestMagic, encodeKnots_length, List.length_take, capacity,
    DClutch.Codec.encodeLE_length,
    DClutch.LiabilityBasisV2.PhysicalAbi.encodeI64_length]
  simp [requestBytes, reservedBytes, maxKnots, knotBytes]

/-- The ramp and the spline are ONE request family. They share `requestMagic`
by construction -- both emitters print that one Lean object -- and they are
separated by the profile tag, at the same coordinate in both records.

This is stated because the Rust used to print the shared magic under two names,
`RAMP_MAGIC_V2` and `SPLINE_MAGIC_V2`, and the uniqueness census read one wire
value as two claimants as soon as it could see emitted magics at all. The right
adjudication was to declare it once, not to re-letter one side, and this is why:
nothing dispatches on the magic. Re-lettering the spline would have invented a
second family on the wire to make a census green. -/
theorem the_families_share_a_magic_and_differ_by_profile :
    profileTag ≠ DClutch.LiabilityBasisV2.PhysicalAbi.profile ∧
    profileOffset = DClutch.LiabilityBasisV2.PhysicalAbi.profileOffset := by
  native_decide

end DClutch.LiabilityBasisV2.Spline.PhysicalAbi
