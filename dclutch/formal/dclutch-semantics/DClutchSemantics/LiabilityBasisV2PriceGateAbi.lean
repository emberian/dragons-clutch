import DClutchSemantics.LiabilityBasisV2PriceGate
import DClutchSemantics.LiabilityBasisV2SplineAbi

/-!
# Exact physical profile for the degree-`≥ 2` arbitrage gate

`LiabilityBasisV2PriceGate` quantifies over unbounded `Nat` and `Int` and over
an arbitrary `Basis`.  This module pins one hostile-decodable byte record for
it against the B-spline profile, and owns the generated translation corpus.

The record is exactly 320 bytes: profile `1` of schema `1`, magic
`DCLTPGT1`.  It carries a `u32` payout scale, a `u64` mixture mass, the
degree and width it claims to be a certificate for, up to ten `u64` prices,
and up to ten atoms — each a `u64` weight, an `i64` coordinate numerator and a
`u32` coordinate denominator.

**Ten atoms is not an arbitrary capacity.**  Every payout vector lies in the
affine hyperplane `sum = Q`, whose dimension is at most `width - 1`, so affine
Carathéodory gives a support bound of `width` — and `width` is at most
`Spline.PhysicalAbi.maxWidth = 10`.  A price inside the hull that needs more
atoms than that does not exist; a price that needs a *mass* larger than `u64`
may, and that is the profile's real residual, named below.

## What is checked here that no theorem needs

Four admission checks have no counterpart among the pure theorems, exactly as
in the spline record, and for the same reason:

* **Atoms must be strictly ordered** by coordinate.  The mathematics is
  indifferent to the order and to repeated coordinates; the physical boundary
  refuses both so that one support has one encoding.
* **Weights must be primitive against the mass.**  `Certificate.Valid` is
  invariant under scaling weights and mass together; the boundary refuses the
  scaled forms.  This is canonicalization, *not* a uniqueness claim: one price
  can still have many supports (generation two pinned that at
  `dragons-clutch crates/clutch-price-measure/tests/adversarial.rs:321`).
* **Padding must be canonical.**  Inactive price, weight and coordinate slots
  must be zero.
* **The certificate must repeat the basis it is for.**  Scale, degree and
  width are checked against the *authenticated* spline request rather than
  against a digest of one, so there is no hash preimage question and no second
  copy of the basis to disagree with.

## The residual this profile carries

The mass is a `u64`.  A price that lies in the hull but whose every
representation needs a larger common denominator is refused.  That is a
**sufficient inner certificate**, and it fails closed.  Generation two carried
the same residual and named it in
`docs/design/PRICE_MEASURE_WITNESS_V2.md:188`; nothing here closes it, and the
scorecard says so.
-/

namespace DClutch.LiabilityBasisV2.PriceGate.PhysicalAbi

open DClutch.LiabilityBasisV2
open DClutch.LiabilityBasisV2.Spline
open DClutch.LiabilityBasisV2.PriceGate

def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x50, 0x47, 0x54, 0x31]
def requestBytes : Nat := 320
def schemaVersion : Nat := 1
def profileTag : Nat := 1

/-- Physical price capacity: the spline record's own claim capacity. -/
def maxWidth : Nat := Spline.PhysicalAbi.maxWidth

/-- Physical support capacity.  Affine Carathéodory bounds the support of a
hull point by the width, and the width is bounded by `maxWidth`. -/
def maxAtoms : Nat := maxWidth

def magicOffset : Nat := 0
def versionOffset : Nat := 8
def profileOffset : Nat := 10
def scaleOffset : Nat := 12
def massOffset : Nat := 16
def degreeOffset : Nat := 24
def widthOffset : Nat := 25
def atomCountOffset : Nat := 26
def reservedOffset : Nat := 27
def reservedBytes : Nat := 13
def pricesOffset : Nat := 40
def priceBytes : Nat := 8
def weightsOffset : Nat := 120
def weightBytes : Nat := 8
def numeratorsOffset : Nat := 200
def numeratorBytes : Nat := 8
def denominatorsOffset : Nat := 280
def denominatorBytes : Nat := 4

structure Request where
  scale : Nat
  mass : Nat
  degree : Nat
  width : Nat
  atomCount : Nat
  prices : List Nat
  weights : List Nat
  numerators : List Int
  denominators : List Nat
  deriving DecidableEq, Repr

/-- The prices the certificate actually claims. -/
def Request.activePrices (request : Request) : List Nat :=
  request.prices.take request.width

/-- The atoms the certificate actually names: a coordinate and its weight. -/
def Request.activeAtoms (request : Request) : List (Spline.RationalCoordinate × Nat) :=
  (List.range request.atomCount).map (fun index =>
    ({ numerator := request.numerators[index]?.getD 0,
       denominator := request.denominators[index]?.getD 0 },
      request.weights[index]?.getD 0))

def Request.activeCoordinates (request : Request) : List Spline.RationalCoordinate :=
  request.activeAtoms.map Prod.fst

/-- Strictly increasing rational coordinates, compared by cross-multiplication
over positive denominators. -/
def strictlyIncreasing : List Spline.RationalCoordinate → Bool
  | [] => true
  | [_] => true
  | first :: second :: rest =>
      decide (first.numerator * (second.denominator : Int)
        < second.numerator * (first.denominator : Int)) &&
        strictlyIncreasing (second :: rest)

def gcdAll : Nat → List Nat → Nat
  | seed, [] => seed
  | seed, value :: values => gcdAll (Nat.gcd seed value) values

/-! ### The join to one authenticated basis

The certificate is always checked *against* a spline request the caller has
already authenticated.  There is no second copy of the basis to disagree with
and no digest whose preimage would have to be argued about: the three fields
the certificate repeats are compared to that request's own.
-/

/-- The certificate repeats the basis it is for. -/
def Request.bindsBasis (request : Request) (profile : SplineProfile) : Bool :=
  decide (request.scale = profile.scale) &&
    decide (request.degree = profile.degree) &&
    decide (request.width = profile.width)

/-- Every named coordinate is one the evaluator admits. -/
def Request.coordinatesAdmitted (request : Request) (profile : SplineProfile) : Bool :=
  request.activeCoordinates.all (fun coordinate => profile.admits coordinate)

/-- **The hull equation.**  Every atom is recomputed by the profile's own
evaluator; nothing about a payout vector comes off the wire. -/
def Request.hullCloses (request : Request) (profile : SplineProfile) : Bool :=
  decide (scaleVector request.mass request.activePrices
    = rawMixture profile.width profile.evaluate request.activeAtoms)

/-! ## The byte record -/

open DClutch.LiabilityBasisV2.PhysicalAbi (encodeI64 decodeI64 field)

def encodeNats (width : Nat) : List Nat → List UInt8
  | [] => []
  | value :: rest => DClutch.Codec.encodeLE width value ++ encodeNats width rest

def encodeInts : List Int → List UInt8
  | [] => []
  | value :: rest => encodeI64 value ++ encodeInts rest

def encodeRequest (request : Request) : List UInt8 :=
  requestMagic ++
    DClutch.Codec.encodeLE 2 schemaVersion ++
    DClutch.Codec.encodeLE 2 profileTag ++
    DClutch.Codec.encodeLE 4 request.scale ++
    DClutch.Codec.encodeLE 8 request.mass ++
    DClutch.Codec.encodeLE 1 request.degree ++
    DClutch.Codec.encodeLE 1 request.width ++
    DClutch.Codec.encodeLE 1 request.atomCount ++
    List.replicate reservedBytes 0 ++
    encodeNats priceBytes (request.prices.take maxWidth) ++
    List.replicate ((maxWidth - request.prices.length) * priceBytes) 0 ++
    encodeNats weightBytes (request.weights.take maxAtoms) ++
    List.replicate ((maxAtoms - request.weights.length) * weightBytes) 0 ++
    encodeInts (request.numerators.take maxAtoms) ++
    List.replicate ((maxAtoms - request.numerators.length) * numeratorBytes) 0 ++
    encodeNats denominatorBytes (request.denominators.take maxAtoms) ++
    List.replicate ((maxAtoms - request.denominators.length) * denominatorBytes) 0

def projectSlots (bytes : List UInt8) (offset width count : Nat) : List Nat :=
  (List.range count).map (fun slot =>
    DClutch.Codec.decodeLE (field bytes (offset + slot * width) width))

def projectSigned (bytes : List UInt8) (offset width count : Nat) : List Int :=
  (List.range count).map (fun slot =>
    decodeI64 (field bytes (offset + slot * width) width))

/-- The exact field projection the hostile decoder performs before it applies
any semantic guard. -/
def projectRequest (bytes : List UInt8) : Request := {
  scale := DClutch.Codec.decodeLE (field bytes scaleOffset 4)
  mass := DClutch.Codec.decodeLE (field bytes massOffset 8)
  degree := DClutch.Codec.decodeLE (field bytes degreeOffset 1)
  width := DClutch.Codec.decodeLE (field bytes widthOffset 1)
  atomCount := DClutch.Codec.decodeLE (field bytes atomCountOffset 1)
  prices := projectSlots bytes pricesOffset priceBytes maxWidth
  weights := projectSlots bytes weightsOffset weightBytes maxAtoms
  numerators := projectSigned bytes numeratorsOffset numeratorBytes maxAtoms
  denominators := projectSlots bytes denominatorsOffset denominatorBytes maxAtoms
}

/-- Inactive price slots, which must be canonical zero. -/
def pricePadding (bytes : List UInt8) : List Nat :=
  (projectRequest bytes).prices.drop (projectRequest bytes).width

/-- Inactive weight and coordinate slots, which must be canonical zero. -/
def atomPadding (bytes : List UInt8) : List Int :=
  let request := projectRequest bytes
  (request.weights.drop request.atomCount).map Int.ofNat ++
    request.numerators.drop request.atomCount ++
    (request.denominators.drop request.atomCount).map Int.ofNat

/-- **Hostile decoder checks, in order.**  The first failing check names the
refusal tag, so this ORDER is part of the translation contract the Rust kernel
must reproduce.

Shared with the ramp and spline profiles, where they mean the same thing:
length `0`, magic `1`, schema `2`, profile `3`, reserved `4`, scale `5`,
denominator `6`, unsupported degree `15`, degenerate span `19`.

New to this record: zero mass `20`, width out of range `21`, atom count out of
range `22`, non-canonical gate padding `23`, zero atom weight `24`,
non-canonical atom order `25`, weight mass mismatch `26`, non-primitive weight
scale `27`, price not a partition `28`, basis mismatch `29`, and price
reconstruction mismatch `30`.  Tag `31` belongs to the admission conjunct
below: a degree-`≥ 2` basis offered with no certificate at all. -/
def decodeChecks
    (bytes : List UInt8) (profile : SplineProfile) : List (Nat × Bool) :=
  [ (0, decide (bytes.length = requestBytes)),
    (1, decide (field bytes magicOffset requestMagic.length = requestMagic)),
    (2, decide (DClutch.Codec.decodeLE (field bytes versionOffset 2) = schemaVersion)),
    (3, decide (DClutch.Codec.decodeLE (field bytes profileOffset 2) = profileTag)),
    (4, (field bytes reservedOffset reservedBytes).all (fun byte => byte == 0)),
    (5, decide (0 < (projectRequest bytes).scale)),
    (20, decide (0 < (projectRequest bytes).mass)),
    (15, decide (Spline.PhysicalAbi.minDegree ≤ (projectRequest bytes).degree) &&
        decide ((projectRequest bytes).degree ≤ Spline.PhysicalAbi.maxDegree)),
    (21, decide ((projectRequest bytes).degree < (projectRequest bytes).width) &&
        decide ((projectRequest bytes).width ≤ maxWidth)),
    (22, decide (0 < (projectRequest bytes).atomCount) &&
        decide ((projectRequest bytes).atomCount ≤ maxAtoms)),
    (23, (pricePadding bytes).all (fun value => value == 0) &&
        (atomPadding bytes).all (fun value => value == 0)),
    (6, (projectRequest bytes).activeCoordinates.all
        (fun coordinate => decide (0 < coordinate.denominator))),
    (24, (projectRequest bytes).activeAtoms.all (fun atom => decide (0 < atom.2))),
    (25, strictlyIncreasing (projectRequest bytes).activeCoordinates),
    (26, decide (((projectRequest bytes).activeAtoms.map Prod.snd).sum
        = (projectRequest bytes).mass)),
    (27, decide (gcdAll (projectRequest bytes).mass
        ((projectRequest bytes).activeAtoms.map Prod.snd) = 1)),
    (28, decide ((projectRequest bytes).activePrices.sum = (projectRequest bytes).scale)),
    (29, (projectRequest bytes).bindsBasis profile),
    (19, (projectRequest bytes).coordinatesAdmitted (profile)),
    (30, (projectRequest bytes).hullCloses (profile)) ]

/-- The tag of the first failing check, or `none` when the record is accepted. -/
def refusal?
    (bytes : List UInt8) (profile : SplineProfile) : Option Nat :=
  ((decodeChecks bytes profile).find? (fun check => !check.2)).map Prod.fst

/-- **The hostile semantic decoder.** -/
def decodeRequest
    (bytes : List UInt8) (profile : SplineProfile) : Except Nat Request :=
  match refusal? bytes profile with
  | some tag => .error tag
  | none => .ok (projectRequest bytes)

theorem refusal_none_getElem
    (bytes : List UInt8) (profile : SplineProfile) (index : Nat)
    (bound : index < (decodeChecks bytes profile).length)
    (accepted : refusal? bytes profile = none) :
    ((decodeChecks bytes profile)[index]).2 = true := by
  unfold refusal? at accepted
  rw [Option.map_eq_none_iff, List.find?_eq_none] at accepted
  simpa using accepted _ (List.getElem_mem bound)

theorem decodeRequest_ok_accepted
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request) :
    refusal? bytes profile = none ∧ request = projectRequest bytes := by
  unfold decodeRequest at decoded
  split at decoded
  · simp at decoded
  · rename_i accepted
    exact ⟨accepted, by simpa using decoded.symm⟩

/-- Any input that is not exactly the canonical physical width is refused with
the stable length tag; no short or long record is ever partially decoded. -/
theorem decodeRequest_refuses_wrong_length
    (bytes : List UInt8) (profile : SplineProfile) (wrongLength : bytes.length ≠ requestBytes) :
    decodeRequest bytes profile = .error 0 := by
  unfold decodeRequest refusal? decodeChecks
  simp [wrongLength, List.find?]

/-! ### What an accepted record certifies

Four of the twenty checks are load-bearing for the theorems below; the rest
are canonicalization and shape.  Each is extracted from the decoder's own list
by position, so the tag order above is what these theorems depend on.
-/

theorem decodeRequest_mass_positive
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request) :
    0 < request.mass := by
  obtain ⟨accepted, rfl⟩ := decodeRequest_ok_accepted bytes profile _ decoded
  have check := refusal_none_getElem bytes profile 6 (by simp [decodeChecks]) accepted
  simpa [decodeChecks] using check

theorem decodeRequest_price_sum
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request) :
    request.activePrices.sum = request.scale := by
  obtain ⟨accepted, rfl⟩ := decodeRequest_ok_accepted bytes profile _ decoded
  have check :=
    refusal_none_getElem bytes profile 16 (by simp [decodeChecks]) accepted
  simpa [decodeChecks] using check

theorem decodeRequest_coordinatesAdmitted
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request) :
    request.coordinatesAdmitted (profile) = true := by
  obtain ⟨accepted, rfl⟩ := decodeRequest_ok_accepted bytes profile _ decoded
  have check :=
    refusal_none_getElem bytes profile 18 (by simp [decodeChecks]) accepted
  simpa [decodeChecks] using check

theorem decodeRequest_hullCloses
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request) :
    request.hullCloses (profile) = true := by
  obtain ⟨accepted, rfl⟩ := decodeRequest_ok_accepted bytes profile _ decoded
  have check :=
    refusal_none_getElem bytes profile 19 (by simp [decodeChecks]) accepted
  simpa [decodeChecks] using check

/-- Every atom of an accepted record names an admitted coordinate, so every
recomputed payout vector really is a partition of the basis's own scale. -/
theorem decodeRequest_admits_atom
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request)
    (atom : Spline.RationalCoordinate × Nat) (member : atom ∈ request.activeAtoms) :
    (profile).admits atom.1 = true := by
  have admitted :=
    decodeRequest_coordinatesAdmitted bytes profile request decoded
  unfold Request.coordinatesAdmitted Request.activeCoordinates at admitted
  rw [List.all_eq_true] at admitted
  exact admitted atom.1 (List.mem_map_of_mem member)

/-- **No arbitrage, at the physical boundary.**  A portfolio that pays
nonnegatively at every coordinate an accepted record names cannot have a
strictly negative price under that record's price vector.

The support is finite and every coordinate on it is decidable, so this
theorem's hypothesis is something a checker can establish — which is what
makes the gate executable rather than a definition. -/
theorem decodeRequest_no_arbitrage
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request)
    (portfolio : List Int)
    (nonneg : ∀ coordinate ∈ request.activeCoordinates,
      0 ≤ portfolioValue portfolio
        ((profile).evaluate coordinate)) :
    0 ≤ portfolioValue portfolio request.activePrices := by
  refine nonneg_price_of_raw_mixture (profile).width
    (profile).evaluate portfolio request.activePrices
    request.mass request.activeAtoms ?_
    (decodeRequest_mass_positive bytes profile request decoded)
    (of_decide_eq_true (decodeRequest_hullCloses bytes profile request decoded))
    ?_
  · intro atom member
    exact (profile).evaluate_length atom.1
      (decodeRequest_admits_atom bytes profile request decoded atom member)
  · intro atom member
    exact nonneg atom.1 (List.mem_map_of_mem member)

/-- **An accepted price is a partition of the collateral scale.**  The simplex
condition is checked here rather than derived, because the decoder wants a
distinct refusal tag for it; `PriceGate.Certificate.price_sum` proves it is
implied by hull membership anyway, so this check can only ever refuse
earlier. -/
theorem decodeRequest_validPartition
    (bytes : List UInt8) (profile : SplineProfile) (request : Request)
    (decoded : decodeRequest bytes profile = .ok request) :
    request.activePrices.sum = request.scale :=
  decodeRequest_price_sum bytes profile request decoded

/-! ### The admission conjunct, at the physical boundary -/

/-- Stable refusal tag for a degree-`≥ 2` basis offered with no certificate. -/
def priceGateRequiredTag : Nat := 31

/-- **The admission rule the evaluator boundary gains.**  A spline request of
degree `> exemptDegree` is admitted only alongside a certificate the decoder
accepts against that same request.  Degree `≤ exemptDegree` needs none — LB-SPLINE
pinned a degree-one hat attaining the whole complete set, and
`PriceGate.no_cap_of_attained_scale` is why that is the exemption — but a
certificate that *is* offered is checked regardless of degree, so a present
input is never silently ignored. -/
def admitEvaluation
    (profile : SplineProfile)
    (certificate : Option (List UInt8)) : Except Nat (Option Request) :=
  match certificate with
  | some bytes => (decodeRequest bytes profile).map some
  | none =>
      if profile.degree ≤ exemptDegree then .ok none
      else .error priceGateRequiredTag

/-- **Nothing at degree `≥ 2` is evaluated for sale without a certificate.** -/
theorem admitEvaluation_refuses_graded_without_certificate
    (profile : SplineProfile)
    (graded : exemptDegree < profile.degree) :
    admitEvaluation profile none = .error priceGateRequiredTag := by
  simp only [admitEvaluation]
  rw [if_neg (by omega)]

/-- **Degree `≤ 1` is admitted with no certificate**, and that is the whole
exemption. -/
theorem admitEvaluation_admits_exempt
    (profile : SplineProfile)
    (exempt : profile.degree ≤ exemptDegree) :
    admitEvaluation profile none = .ok none := by
  simp only [admitEvaluation]
  rw [if_pos exempt]

/-- **Every certificate that gets past the boundary really closes the hull
equation**, so `decodeRequest_no_arbitrage` applies to every admission this
rule grants. -/
theorem admitEvaluation_hullCloses
    (profile : SplineProfile)
    (bytes : List UInt8) (request : Request)
    (admitted : admitEvaluation profile (some bytes) = .ok (some request)) :
    request.hullCloses (profile) = true := by
  simp only [admitEvaluation] at admitted
  cases decoded : decodeRequest bytes profile with
  | error tag =>
      rw [decoded] at admitted
      simp [Except.map] at admitted
  | ok decodedRequest =>
      rw [decoded] at admitted
      simp only [Except.map, Except.ok.injEq, Option.some.injEq] at admitted
      subst admitted
      exact decodeRequest_hullCloses bytes profile decodedRequest decoded

end DClutch.LiabilityBasisV2.PriceGate.PhysicalAbi
