import DClutchSemantics.LiabilityBasisV2PriceGateAbi
import DClutchSemantics.Codec
import DClutchSemantics.RustEmit

open DClutch.RustEmit (rustBytes)

/-!
Emit the exact degree-`≥ 2` price-gate ABI constants plus two finite corpora:
a semantic agreement corpus of certificates the gate admits, and a hostile
refusal corpus reaching every guarded tag.

The handwritten Rust kernel consumes this output; this executable emits no
Rust verification logic.  Agreement cases are decided by the Lean checker
itself, so a Rust kernel that admits or refuses any listed case differently
fails.
-/

open DClutch.LiabilityBasisV2
open DClutch.LiabilityBasisV2.Spline
open DClutch.LiabilityBasisV2.PriceGate
open DClutch.LiabilityBasisV2.PriceGate.PhysicalAbi

def rustNatList (width : Nat) (values : List Nat) : String :=
  let padded := values ++ List.replicate (width - values.length) 0
  s!"[{String.intercalate ", " (padded.map toString)}]"

/-- One canonical spline request naming the basis a certificate is for.  Its
own coordinate field is irrelevant to the gate and is fixed at zero. -/
def splineRequest (degree scale knotDenominator : Nat) (knots : List Int) :
    Spline.PhysicalAbi.Request := {
  scale
  knotDenominator
  coordinateDenominator := 1
  coordinateNumerator := 0
  degree
  knotCount := knots.length
  knots := knots ++ List.replicate (Spline.PhysicalAbi.maxKnots - knots.length) 0
}

def profileOf? (request : Spline.PhysicalAbi.Request) : Option SplineProfile :=
  if wellFormed : request.WellFormed then some (request.profileOf wellFormed) else none

/-- One canonical certificate body.  `atoms` are `(numerator, denominator,
weight)` triples in the record's own strictly increasing coordinate order. -/
def certificate
    (scale mass degree width : Nat) (prices : List Nat)
    (atoms : List (Int × Nat × Nat)) : DClutch.LiabilityBasisV2.PriceGate.PhysicalAbi.Request := {
  scale
  mass
  degree
  width
  atomCount := atoms.length
  prices
  weights := atoms.map (fun atom => atom.2.2)
  numerators := atoms.map Prod.fst
  denominators := atoms.map (fun atom => atom.2.1)
}

/-! ## The bases the corpus certifies against -/

def gen1Knots : List Int := [0, 0, 0, 1, 2, 3, 3, 3]
def gen2LiveKnots : List Int := [0, 0, 0, 128, 256, 384, 384, 384]
def hatKnots : List Int := [0, 1, 2, 3]
def bezierKnots : List Int := [0, 0, 0, 0, 1, 1, 1, 1]
def quadraticKnots : List Int := [0, 0, 0, 2, 4, 6, 6, 6]
def uniformCubicKnots : List Int := [0, 0, 0, 0, 1, 2, 3, 4, 5, 5, 5, 5]
def doubleKnotKnots : List Int := [0, 0, 0, 0, 2, 2, 4, 4, 4, 4]
def wideHatKnots : List Int := [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
def negativeHatKnots : List Int := [-4, -3, -2, -1]
def rationalCubicKnots : List Int := [0, 0, 0, 0, 3, 6, 9, 9, 9, 9]
def degenerateHatKnots : List Int := [5, 5, 5, 5]

def u32Maximum : Nat := 4294967295

def gen1 : Spline.PhysicalAbi.Request := splineRequest 2 12 1 gen1Knots
def gen2Live : Spline.PhysicalAbi.Request := splineRequest 2 10000 1 gen2LiveKnots
def hats : Spline.PhysicalAbi.Request := splineRequest 1 100 1 hatKnots
def bezier : Spline.PhysicalAbi.Request := splineRequest 3 1000 1 bezierKnots
def quadratic : Spline.PhysicalAbi.Request := splineRequest 2 1200 1 quadraticKnots
def uniformCubic : Spline.PhysicalAbi.Request := splineRequest 3 1200 1 uniformCubicKnots
def doubleKnot : Spline.PhysicalAbi.Request := splineRequest 3 1000 1 doubleKnotKnots
def wideHat : Spline.PhysicalAbi.Request := splineRequest 1 1000 1 wideHatKnots
def negativeHat : Spline.PhysicalAbi.Request := splineRequest 1 100 1 negativeHatKnots
def rationalCubic : Spline.PhysicalAbi.Request := splineRequest 3 720 5 rationalCubicKnots
def maximalScale : Spline.PhysicalAbi.Request := splineRequest 2 u32Maximum 1 [0, 0, 0, 1, 2, 3, 3, 3]
def unitScale : Spline.PhysicalAbi.Request := splineRequest 3 1 1 bezierKnots
def degenerateHat : Spline.PhysicalAbi.Request := splineRequest 1 100 1 degenerateHatKnots

/-! ## Admitted certificates

Every named edge appears: a single atom and the widest support the record can
carry, degree one through three, integer and rational coordinates, negative
knots, a knot denominator above one, interior knot multiplicity, and both
extremes of the physical payout scale.
-/

def agreementCases : List (Spline.PhysicalAbi.Request × DClutch.LiabilityBasisV2.PriceGate.PhysicalAbi.Request) := [
  -- The live quantized point generation one's gate refused, and its mirror:
  -- a single atom at weight one is the whole certificate.
  (gen2Live, certificate 10000 1 2 5 [1128, 6667, 2205, 0, 0] [(85, 1, 1)]),
  (gen2Live, certificate 10000 1 2 5 [0, 0, 2204, 6667, 1129] [(299, 1, 1)]),
  -- Generation one's own counterexample basis, at its own scale twelve. What
  -- IS attainable on it is certified, so the direction-one refusals below are
  -- about that one price and not about the basis carrying it.
  (gen1, certificate 12 1 2 5 [0, 6, 6, 0, 0] [(1, 1, 1)]),
  (gen1, certificate 12 2 2 5 [0, 3, 6, 3, 0] [(1, 1, 1), (2, 1, 1)]),
  -- ONE PRICE, TWO SUPPORTS.  Generation two pinned the same fact about its
  -- own certificate at `adversarial.rs:321`: an accepted certificate is not a
  -- canonical identifier for a price.  Here the price (0,7,5,0,0)/12 is
  -- reachable both as a single atom at 5/6 and as an even mixture of 3/4 and
  -- 1.  Both are primitive and both are admitted, which is what the checker
  -- claims: `mixture_valid` admits EVERY honest mixture, not a distinguished
  -- normal form.  The primitivity guard canonicalizes the SCALE of one
  -- support; it does not make the support unique, and nothing here should be
  -- read as though it did.
  (gen1, certificate 12 1 2 5 [0, 7, 5, 0, 0] [(5, 6, 1)]),
  (gen1, certificate 12 2 2 5 [0, 7, 5, 0, 0] [(3, 4, 1), (1, 1, 1)]),
  -- Degree one: a genuine two-atom mixture, and one endpoint alone.
  (hats, certificate 100 100 1 2 [37, 63] [(1, 1, 37), (2, 1, 63)]),
  (hats, certificate 100 1 1 2 [100, 0] [(1, 1, 1)]),
  -- Degree three, clamped: equal and unequal weights over the two ends.
  (bezier, certificate 1000 2 3 4 [500, 0, 0, 500] [(0, 1, 1), (1, 1, 1)]),
  (bezier, certificate 1000 4 3 4 [250, 0, 0, 750] [(0, 1, 1), (1, 1, 3)]),
  -- A rational coordinate, at a quarter of the single span.
  (bezier, certificate 1000 1 3 4 [421, 422, 141, 16] [(1, 4, 1)]),
  -- Degree two interior mixtures of two and three atoms.
  (quadratic, certificate 1200 2 2 5 [150, 450, 525, 75, 0] [(1, 1, 1), (3, 1, 1)]),
  (quadratic, certificate 1200 3 2 5 [0, 250, 700, 250, 0]
    [(2, 1, 1), (3, 1, 1), (4, 1, 1)]),
  -- Width eight, at a span midpoint and as mixtures.
  (uniformCubic, certificate 1200 1 3 8 [0, 0, 25, 575, 575, 25, 0, 0] [(5, 2, 1)]),
  (uniformCubic, certificate 1200 2 3 8 [0, 150, 350, 200, 400, 100, 0, 0]
    [(1, 1, 1), (3, 1, 1)]),
  (uniformCubic, certificate 1200 4 3 8 [0, 75, 225, 300, 300, 225, 75, 0]
    [(1, 1, 1), (2, 1, 1), (3, 1, 1), (4, 1, 1)]),
  -- Interior knot multiplicity two, at the double knot itself.
  (doubleKnot, certificate 1000 1 3 6 [0, 0, 500, 500, 0, 0] [(2, 1, 1)]),
  -- The widest basis and the widest support the record can express: ten
  -- claims, ten atoms, which is the affine Caratheodory bound.
  (wideHat, certificate 1000 10 1 10 [100, 100, 100, 100, 100, 100, 100, 100, 100, 100]
    [(1, 1, 1), (2, 1, 1), (3, 1, 1), (4, 1, 1), (5, 1, 1),
      (6, 1, 1), (7, 1, 1), (8, 1, 1), (9, 1, 1), (10, 1, 1)]),
  -- Wholly negative knots.
  (negativeHat, certificate 100 100 1 2 [41, 59] [(-3, 1, 41), (-2, 1, 59)]),
  -- A knot denominator above one, so the knots are true rationals.
  (rationalCubic, certificate 720 1 3 6 [0, 0, 0, 0, 0, 720] [(7, 3, 1)]),
  -- Both extremes of the physical payout scale.
  (maximalScale, certificate u32Maximum 1 2 5
    [0, 536870911, 3221225472, 536870912, 0] [(3, 2, 1)]),
  (unitScale, certificate 1 1 3 4 [0, 0, 0, 1] [(1, 2, 1)])
]

/-! ## Refused certificates -/

def canonicalBasis : Spline.PhysicalAbi.Request := hats

def canonicalCertificate : DClutch.LiabilityBasisV2.PriceGate.PhysicalAbi.Request :=
  certificate 100 100 1 2 [37, 63] [(1, 1, 37), (2, 1, 63)]

def canonical : List UInt8 := encodeRequest canonicalCertificate

def changed (bytes : List UInt8) (offset : Nat) (value : UInt8) : List UInt8 :=
  bytes.set offset value

def zeroSpan (bytes : List UInt8) (offset width : Nat) : List UInt8 :=
  (List.range width).foldl (fun result index => result.set (offset + index) 0) bytes

def hostileCases : List (Spline.PhysicalAbi.Request × List UInt8) := [
  -- 0: not the sole canonical width, short, long and empty.
  (canonicalBasis, canonical.take (requestBytes - 1)),
  (canonicalBasis, canonical ++ [0]),
  (canonicalBasis, []),
  -- 1: magic selecting another record family, at two positions.
  (canonicalBasis, changed canonical magicOffset 0),
  (canonicalBasis, changed canonical 7 0x32),
  -- 2: another semantic schema, above and below.
  (canonicalBasis, changed canonical versionOffset 2),
  (canonicalBasis, changed canonical versionOffset 0),
  -- 3: an unknown profile in this record's own layout.
  (canonicalBasis, changed canonical profileOffset 2),
  -- 4: reserved bytes not canonical, at both ends of the span.
  (canonicalBasis, changed canonical reservedOffset 1),
  (canonicalBasis, changed canonical (reservedOffset + reservedBytes - 1) 0x80),
  -- 5: zero payout scale.
  (canonicalBasis, zeroSpan canonical scaleOffset 4),
  -- 20: zero mixture mass, which would make the hull equation vacuous.
  (canonicalBasis, zeroSpan canonical massOffset 8),
  -- 15: degree zero, which is the categorical basis and not this record, and
  -- degree four, which is outside the admitted family.
  (canonicalBasis, changed canonical degreeOffset 0),
  (canonicalBasis, changed canonical degreeOffset 4),
  -- 21: a width at or below the degree, and a width past the record.
  (canonicalBasis, changed canonical widthOffset 1),
  (canonicalBasis, changed canonical widthOffset 11),
  (canonicalBasis, changed canonical widthOffset 255),
  -- 22: no atoms at all, and more atoms than the Caratheodory bound.
  (canonicalBasis, changed canonical atomCountOffset 0),
  (canonicalBasis, changed canonical atomCountOffset 11),
  -- 23: non-canonical padding in each of the four slot arrays.
  (canonicalBasis, changed canonical (pricesOffset + 2 * priceBytes) 1),
  (canonicalBasis, changed canonical (weightsOffset + 2 * weightBytes) 1),
  (canonicalBasis, changed canonical (numeratorsOffset + 2 * numeratorBytes) 1),
  (canonicalBasis, changed canonical (denominatorsOffset + 2 * denominatorBytes) 1),
  -- 6: an active coordinate denominator of zero.
  (canonicalBasis, zeroSpan canonical denominatorsOffset denominatorBytes),
  -- 24: an active weight of zero, which a sparse support must omit instead.
  (canonicalBasis, encodeRequest (certificate 100 63 1 2 [0, 63] [(1, 1, 0), (2, 1, 63)])),
  -- 25: atoms out of coordinate order, and a repeated coordinate.
  (canonicalBasis, encodeRequest (certificate 100 100 1 2 [63, 37] [(2, 1, 63), (1, 1, 37)])),
  (canonicalBasis, encodeRequest (certificate 100 100 1 2 [37, 63] [(1, 1, 37), (1, 1, 63)])),
  -- 26: weights that do not sum to the named mass, above and below.
  (canonicalBasis, encodeRequest (certificate 100 99 1 2 [37, 63] [(1, 1, 37), (2, 1, 63)])),
  (canonicalBasis, encodeRequest (certificate 100 101 1 2 [37, 63] [(1, 1, 37), (2, 1, 63)])),
  -- 27: the same mixture at a non-primitive scale.
  (canonicalBasis, encodeRequest (certificate 100 200 1 2 [37, 63] [(1, 1, 74), (2, 1, 126)])),
  -- 28: prices that do not sum to the named scale.
  (canonicalBasis, encodeRequest (certificate 100 100 1 2 [37, 62] [(1, 1, 37), (2, 1, 63)])),
  -- 29: a certificate for another basis entirely, and for the right basis at
  -- the wrong width and the wrong degree.
  (canonicalBasis, encodeRequest (certificate 99 100 1 2 [37, 62] [(1, 1, 37), (2, 1, 63)])),
  (bezier, encodeRequest (certificate 1000 1 2 4 [500, 0, 0, 500] [(0, 1, 1)])),
  (quadratic, encodeRequest (certificate 1200 1 2 4 [1200, 0, 0, 0] [(0, 1, 1)])),
  (bezier, encodeRequest (certificate 999 1 3 4 [999, 0, 0, 0] [(0, 1, 1)])),
  -- 19: a basis whose knots are all equal, so no coordinate is admitted at
  -- all.  The spline decoder accepts that record and refuses it at
  -- evaluation; the gate refuses it here, with the same tag.
  (degenerateHat, encodeRequest (certificate 100 100 1 2 [37, 63] [(1, 1, 37), (2, 1, 63)])),
  -- 30: the hull equation off by one atom in each direction.
  (canonicalBasis, encodeRequest (certificate 100 100 1 2 [38, 62] [(1, 1, 37), (2, 1, 63)])),
  (canonicalBasis, encodeRequest (certificate 100 100 1 2 [36, 64] [(1, 1, 37), (2, 1, 63)])),
  (gen2Live, encodeRequest (certificate 10000 1 2 5 [1129, 6666, 2205, 0, 0] [(85, 1, 1)])),
  -- 30: DIRECTION ONE of generation two's adversarial pair.  The price
  -- (4,8,0,0,0)/12 is the false acceptance generation one's moment cone
  -- admitted (`dragons-clutch crates/clutch-price-measure/tests/adversarial.rs`
  -- line 262), where the portfolio (1,-2,10,40,64) costs exactly -S.  It is
  -- simplex-admissible, so it passes every shape guard including the partition
  -- check and dies at the hull equation -- offered at the single interior
  -- coordinate closest to its own shape, at an endpoint, and as a two-atom
  -- mixture.  `Examples.gen1_price_has_no_certificate_on_grid` is the theorem
  -- that no support on the swept grid can rescue it.
  (gen1, encodeRequest (certificate 12 1 2 5 [4, 8, 0, 0, 0] [(1, 2, 1)])),
  (gen1, encodeRequest (certificate 12 1 2 5 [4, 8, 0, 0, 0] [(0, 1, 1)])),
  (gen1, encodeRequest (certificate 12 2 2 5 [4, 8, 0, 0, 0] [(0, 1, 1), (1, 1, 1)])),
  -- Ordering: a record failing the magic, the mass and the width guards at
  -- once must report the magic.
  (canonicalBasis,
    changed (changed (zeroSpan canonical massOffset 8) magicOffset 0) widthOffset 255),
  -- Ordering: a zero scale and an unsupported degree together must report the
  -- scale, because the scale guard is checked first.
  (canonicalBasis, changed (zeroSpan canonical scaleOffset 4) degreeOffset 7),
  -- Ordering: a basis mismatch and a broken hull equation together must
  -- report the mismatch.
  (bezier, encodeRequest (certificate 100 100 1 2 [38, 62] [(1, 1, 37), (2, 1, 63)]))
]

def emitAgreement
    (index : Nat) (basis : Spline.PhysicalAbi.Request)
    (candidate : DClutch.LiabilityBasisV2.PriceGate.PhysicalAbi.Request) : IO Unit := do
  let profile ← match profileOf? basis with
    | some profile => pure profile
    | none => throw <| IO.userError s!"agreement case {index} names a malformed basis"
  let bytes := encodeRequest candidate
  let decoded ← match decodeRequest bytes profile with
    | .ok decoded => pure decoded
    | .error tag => throw <| IO.userError s!"agreement case {index} refused with {tag}"
  if decoded.activePrices.length != candidate.width then
    throw <| IO.userError s!"agreement case {index} price width disagrees"
  if decoded.activePrices.sum != candidate.scale then
    throw <| IO.userError s!"agreement case {index} price is not a partition"
  IO.println "    PriceGateAgreementCaseV1 {"
  IO.println s!"        basis: {rustBytes (Spline.PhysicalAbi.encodeRequest basis)},"
  IO.println s!"        certificate: {rustBytes bytes},"
  IO.println s!"        width: {candidate.width},"
  IO.println s!"        atom_count: {candidate.atomCount},"
  IO.println s!"        prices: {rustNatList maxWidth decoded.activePrices},"
  IO.println "    },"

def emitRefusal
    (index : Nat) (basis : Spline.PhysicalAbi.Request) (bytes : List UInt8) : IO Unit := do
  let profile ← match profileOf? basis with
    | some profile => pure profile
    | none => throw <| IO.userError s!"hostile case {index} names a malformed basis"
  let tag ← match decodeRequest bytes profile with
    | .error tag => pure tag
    | .ok _ => throw <| IO.userError s!"hostile case {index} was admitted"
  if bytes.length > requestBytes + 1 then
    throw <| IO.userError s!"hostile case {index} exceeds the corpus buffer"
  let padded := bytes ++ List.replicate (requestBytes + 1 - bytes.length) 0
  IO.println "    PriceGateRefusalCaseV1 {"
  IO.println s!"        basis: {rustBytes (Spline.PhysicalAbi.encodeRequest basis)},"
  IO.println s!"        certificate: {rustBytes padded},"
  IO.println s!"        certificate_len: {bytes.length},"
  IO.println s!"        error_tag: {tag},"
  IO.println "    },"

def main : IO Unit := do
  IO.println "// @generated by formal/dclutch-semantics/EmitLiabilityBasisV2PriceGateRust.lean; do not edit."
  IO.println "use super::{PriceGateAgreementCaseV1, PriceGateRefusalCaseV1};"
  IO.println s!"pub const PRICE_GATE_REQUEST_BYTES_V1: usize = {requestBytes};"
  IO.println s!"pub const PRICE_GATE_SCHEMA_VERSION_V1: u16 = {schemaVersion};"
  IO.println s!"pub const PRICE_GATE_PROFILE_V1: u16 = {profileTag};"
  IO.println s!"pub const PRICE_GATE_MAX_WIDTH_V1: usize = {maxWidth};"
  IO.println s!"pub const PRICE_GATE_MAX_ATOMS_V1: usize = {maxAtoms};"
  IO.println s!"pub const PRICE_GATE_MAGIC_OFFSET_V1: usize = {magicOffset};"
  IO.println s!"pub const PRICE_GATE_VERSION_OFFSET_V1: usize = {versionOffset};"
  IO.println s!"pub const PRICE_GATE_PROFILE_OFFSET_V1: usize = {profileOffset};"
  IO.println s!"pub const PRICE_GATE_SCALE_OFFSET_V1: usize = {scaleOffset};"
  IO.println s!"pub const PRICE_GATE_MASS_OFFSET_V1: usize = {massOffset};"
  IO.println s!"pub const PRICE_GATE_DEGREE_OFFSET_V1: usize = {degreeOffset};"
  IO.println s!"pub const PRICE_GATE_WIDTH_OFFSET_V1: usize = {widthOffset};"
  IO.println s!"pub const PRICE_GATE_ATOM_COUNT_OFFSET_V1: usize = {atomCountOffset};"
  IO.println s!"pub const PRICE_GATE_RESERVED_OFFSET_V1: usize = {reservedOffset};"
  IO.println s!"pub const PRICE_GATE_RESERVED_BYTES_V1: usize = {reservedBytes};"
  IO.println s!"pub const PRICE_GATE_PRICES_OFFSET_V1: usize = {pricesOffset};"
  IO.println s!"pub const PRICE_GATE_PRICE_BYTES_V1: usize = {priceBytes};"
  IO.println s!"pub const PRICE_GATE_WEIGHTS_OFFSET_V1: usize = {weightsOffset};"
  IO.println s!"pub const PRICE_GATE_WEIGHT_BYTES_V1: usize = {weightBytes};"
  IO.println s!"pub const PRICE_GATE_NUMERATORS_OFFSET_V1: usize = {numeratorsOffset};"
  IO.println s!"pub const PRICE_GATE_NUMERATOR_BYTES_V1: usize = {numeratorBytes};"
  IO.println s!"pub const PRICE_GATE_DENOMINATORS_OFFSET_V1: usize = {denominatorsOffset};"
  IO.println s!"pub const PRICE_GATE_DENOMINATOR_BYTES_V1: usize = {denominatorBytes};"
  IO.println s!"pub const PRICE_GATE_REFUSAL_BUFFER_V1: usize = {requestBytes + 1};"
  IO.println s!"pub const PRICE_GATE_EXEMPT_DEGREE_V1: u8 = {exemptDegree};"
  IO.println s!"pub const PRICE_GATE_REQUIRED_TAG_V1: u8 = {priceGateRequiredTag};"
  IO.println s!"pub const PRICE_GATE_MAGIC_V1: [u8; 8] = {rustBytes requestMagic};"
  -- `static`, not `const`: each case carries a basis record as well as a
  -- certificate, so these two arrays are large enough that a `const` would
  -- materialize a fresh copy at every use site (clippy::large_const_arrays).
  IO.println s!"pub static PRICE_GATE_AGREEMENT_CASES_V1: [PriceGateAgreementCaseV1; {agreementCases.length}] = ["
  let mut index := 0
  for (basis, candidate) in agreementCases do
    emitAgreement index basis candidate
    index := index + 1
  IO.println "];"
  IO.println s!"pub static PRICE_GATE_REFUSAL_CASES_V1: [PriceGateRefusalCaseV1; {hostileCases.length}] = ["
  let mut hostileIndex := 0
  for (basis, bytes) in hostileCases do
    emitRefusal hostileIndex basis bytes
    hostileIndex := hostileIndex + 1
  IO.println "];"
