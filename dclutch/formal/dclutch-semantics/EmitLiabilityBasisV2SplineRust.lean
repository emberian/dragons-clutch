import DClutchSemantics.LiabilityBasisV2SplineAbi
import DClutchSemantics.Codec

/-!
Emit the exact B-spline profile ABI constants plus two finite corpora: a
semantic agreement corpus and a hostile-decoder refusal corpus.

The handwritten Rust kernel consumes this output; this executable emits no
Rust evaluation logic. Agreement expectations are computed by the Lean
evaluator itself, so a Rust kernel that disagrees on any listed case fails.
-/

open DClutch.LiabilityBasisV2.Spline
open DClutch.LiabilityBasisV2.Spline.PhysicalAbi

def rustByte (byte : UInt8) : String := s!"0x{DClutch.Codec.byteHex byte}"

def rustBytes (bytes : List UInt8) : String :=
  s!"[{String.intercalate ", " (bytes.map rustByte)}]"

def rustNatList (width : Nat) (values : List Nat) : String :=
  let padded := values ++ List.replicate (width - values.length) 0
  s!"[{String.intercalate ", " (padded.map toString)}]"

/-- Build one canonical record; the knot slots past the active count are the
canonical zero padding the decoder requires. -/
def request
    (scale knotDenominator coordinateDenominator : Nat) (coordinateNumerator : Int)
    (degree : Nat) (knots : List Int) : Request := {
  scale
  knotDenominator
  coordinateDenominator
  coordinateNumerator
  degree
  knotCount := knots.length
  knots := knots ++ List.replicate (maxKnots - knots.length) 0
}

def u32Maximum : Nat := 4294967295
def i64Maximum : Int := 9223372036854775807
def i64Minimum : Int := -9223372036854775808

/-- Degree-one hats over four knots: the narrowest B-spline basis, and the one
whose claims are exactly the piecewise-linear interpolation nodes. -/
def hatKnots : List Int := [0, 1, 2, 3]

/-- Clamped cubic: both endpoints at multiplicity four, one span. The basis is
exactly the cubic Bernstein polynomials. -/
def bezierKnots : List Int := [0, 0, 0, 0, 1, 1, 1, 1]

/-- Uniform clamped cubic over five spans: eight claims. -/
def uniformCubicKnots : List Int := [0, 0, 0, 0, 1, 2, 3, 4, 5, 5, 5, 5]

/-- Clamped cubic with an interior knot of multiplicity two: continuity drops
to `C^1` at that knot. Gen-1 forbade interior multiplicity outright. -/
def doubleKnotKnots : List Int := [0, 0, 0, 0, 2, 2, 4, 4, 4, 4]

/-- Clamped quadratic over three spans. -/
def quadraticKnots : List Int := [0, 0, 0, 2, 4, 6, 6, 6]

/-- Widest basis the record can express: degree one over twelve knots, ten
claims. -/
def wideHatKnots : List Int := [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]

/-- Accepted records. Every named edge of the evaluator appears: both domain
clamps, both closed domain ends, interior knot multiplicity, an exactly
representable apportionment and one that must round, the narrowest and widest
bases, degree one through three, negative knots, a knot denominator above one,
and both extremes of the physical scale. -/
def agreementRequests : List Request := [
  -- Degree one, interior midpoint: both hats at exactly half.
  request 100 1 2 3 1 hatKnots,
  -- Degree one at each closed domain end.
  request 100 1 1 1 1 hatKnots,
  request 100 1 1 2 1 hatKnots,
  -- Degree one outside the domain on both sides: the boundary clamp.
  request 100 1 1 (-1000) 1 hatKnots,
  request 100 1 1 1000 1 hatKnots,
  -- Degree one at the extremes of the physical coordinate envelope.
  request 100 1 1 i64Minimum 1 hatKnots,
  request 100 1 1 i64Maximum 1 hatKnots,
  -- Clamped cubic at the midpoint: exact cubic Bernstein, exactly apportioned.
  request 1000 1 2 1 3 bezierKnots,
  -- Clamped cubic at a quarter and three quarters: the apportionment must
  -- round, and it is not reflection symmetric.
  request 1000 1 4 1 3 bezierKnots,
  request 1000 1 4 3 3 bezierKnots,
  -- Clamped cubic at both closed domain ends: the outer claims pay in full.
  request 1000 1 1 0 3 bezierKnots,
  request 1000 1 1 1 3 bezierKnots,
  -- Uniform clamped cubic at a span midpoint and at an interior knot.
  request 1000000 1 2 5 3 uniformCubicKnots,
  request 1000000 1 1 2 3 uniformCubicKnots,
  -- Interior knot multiplicity two, at the double knot and on both sides.
  request 1000 1 1 2 3 doubleKnotKnots,
  request 1000 1 1 1 3 doubleKnotKnots,
  request 1000 1 1 3 3 doubleKnotKnots,
  -- Degree two, interior and at an interior knot.
  request 999 1 2 5 2 quadraticKnots,
  request 999 1 1 2 2 quadraticKnots,
  -- The widest basis the record can express.
  request 1000 1 2 11 1 wideHatKnots,
  -- Scale one: the categorical collateral scale, where a single atom is the
  -- whole complete set and every apportionment is a hard choice.
  request 1 1 2 1 3 bezierKnots,
  request 1 1 2 5 3 uniformCubicKnots,
  -- Scale at the `u32` maximum, both exactly divisible and not.
  request u32Maximum 1 2 1 3 bezierKnots,
  request u32Maximum 1 4 1 3 bezierKnots,
  -- Wholly negative knots.
  request 100 1 1 (-2) 1 [-4, -3, -2, -1],
  -- Knot denominator above one, so the knots are true rationals.
  request 720 5 3 7 3 [0, 0, 0, 0, 3, 6, 9, 9, 9, 9],
  -- Coordinate denominator above the knot denominator.
  request 720 1 7 15 3 uniformCubicKnots,
  -- A coordinate exactly on an interior knot of the uniform cubic, reached
  -- through a non-trivial rational.
  request 1000000 1 4 12 3 uniformCubicKnots
]

def canonical : List UInt8 := encodeRequest (request 1000 1 2 1 3 bezierKnots)

def changed (bytes : List UInt8) (offset : Nat) (value : UInt8) : List UInt8 :=
  bytes.set offset value

def zeroSpan (bytes : List UInt8) (offset width : Nat) : List UInt8 :=
  (List.range width).foldl (fun result index => result.set (offset + index) 0) bytes

/-- Refused records. Every named guard is exercised at more than one byte
position where the field admits one, and the ordering of the check list is
pinned by records that fail several guards at once. -/
def hostileRequests : List (List UInt8) := [
  -- 0: not the sole canonical width, short and long.
  canonical.take (requestBytes - 1),
  canonical ++ [0],
  [],
  -- 1: magic selecting another record family, at two positions.
  changed canonical magicOffset 0,
  changed canonical 7 0x33,
  -- 2: another semantic schema, above and below.
  changed canonical versionOffset 3,
  changed canonical versionOffset 0,
  changed canonical (versionOffset + 1) 1,
  -- 3: the ramp profile and an unknown profile in this record's own layout.
  changed canonical profileOffset 1,
  changed canonical profileOffset 3,
  -- 4: reserved bytes not canonical, at both ends of the span.
  changed canonical reservedOffset 1,
  changed canonical (reservedOffset + reservedBytes - 1) 0x80,
  -- 5: zero payout scale.
  zeroSpan canonical scaleOffset 4,
  -- 6: each zero denominator separately, then both.
  zeroSpan canonical knotDenominatorOffset 4,
  zeroSpan canonical coordinateDenominatorOffset 4,
  zeroSpan (zeroSpan canonical knotDenominatorOffset 4) coordinateDenominatorOffset 4,
  -- 15: degree zero (the categorical basis, which is not this record) and
  -- degree four, which is outside the admitted family.
  changed canonical degreeOffset 0,
  changed canonical degreeOffset 4,
  changed canonical degreeOffset 255,
  -- 16: too few knots for the named degree, and more than the record holds.
  changed canonical knotCountOffset 7,
  changed canonical knotCountOffset 13,
  changed canonical knotCountOffset 255,
  encodeRequest (request 1000 1 2 1 3 [0, 0, 0, 0, 1, 1, 1]),
  -- 17: a non-canonical inactive knot slot.
  changed (encodeRequest (request 100 1 2 3 1 hatKnots))
    (knotsOffset + 4 * knotBytes) 1,
  changed (encodeRequest (request 100 1 2 3 1 hatKnots)) (requestBytes - 1) 0x40,
  -- 18: knots not nondecreasing, in the interior and at the end.
  encodeRequest (request 100 1 2 3 1 [0, 2, 1, 3]),
  encodeRequest (request 100 1 2 3 1 [0, 1, 2, 1]),
  -- 19: every knot equal, so the domain has no non-degenerate span at all.
  encodeRequest (request 1000 1 1 0 3 [5, 5, 5, 5, 5, 5, 5, 5]),
  encodeRequest (request 1000 1 1 0 1 [2, 2, 2, 2]),
  -- Ordering: a record failing the magic, schema, degree and knot-count
  -- guards at once must report the magic.
  changed (changed (changed canonical magicOffset 0) versionOffset 9)
    degreeOffset 7,
  -- Ordering: a zero scale and an unsupported degree together must report the
  -- scale, because the scale guard is checked first.
  changed (zeroSpan canonical scaleOffset 4) degreeOffset 6,
  -- Ordering: an unsupported degree and a degenerate domain together must
  -- report the degree.
  encodeRequest (request 1000 1 1 0 0 [5, 5, 5, 5, 5, 5, 5, 5])
]

def emitAgreement (index : Nat) (candidate : Request) : IO Unit := do
  let bytes := encodeRequest candidate
  let decoded ← match decodeRequest bytes with
    | .ok decoded => pure decoded
    | .error tag => throw <| IO.userError s!"agreement request {index} refused with {tag}"
  let payouts ← match decoded.evaluate? with
    | some payouts => pure payouts
    | none => throw <| IO.userError s!"agreement request {index} did not evaluate"
  if payouts.length > maxWidth then
    throw <| IO.userError s!"agreement request {index} exceeds the corpus width"
  if payouts.sum != candidate.scale then
    throw <| IO.userError s!"agreement request {index} did not partition its scale"
  IO.println "    SplineAgreementCaseV2 {"
  IO.println s!"        request: {rustBytes bytes},"
  IO.println s!"        width: {payouts.length},"
  IO.println s!"        expected: {rustNatList maxWidth payouts},"
  IO.println "    },"

def emitRefusal (index : Nat) (bytes : List UInt8) : IO Unit := do
  let tag ← match decodeRequest bytes with
    | .error tag => pure tag
    | .ok _ => throw <| IO.userError s!"hostile request {index} was admitted"
  if bytes.length > requestBytes + 1 then
    throw <| IO.userError s!"hostile request {index} exceeds the corpus buffer"
  let padded := bytes ++ List.replicate (requestBytes + 1 - bytes.length) 0
  IO.println "    SplineRefusalCaseV2 {"
  IO.println s!"        request: {rustBytes padded},"
  IO.println s!"        request_len: {bytes.length},"
  IO.println s!"        error_tag: {tag},"
  IO.println "    },"

def main : IO Unit := do
  IO.println "// @generated by formal/dclutch-semantics/EmitLiabilityBasisV2SplineRust.lean; do not edit."
  IO.println "use super::{SplineAgreementCaseV2, SplineRefusalCaseV2};"
  IO.println s!"pub const SPLINE_REQUEST_BYTES_V2: usize = {requestBytes};"
  IO.println s!"pub const SPLINE_SCHEMA_VERSION_V2: u16 = {schemaVersion};"
  IO.println s!"pub const SPLINE_PROFILE_V2: u16 = {profileTag};"
  IO.println s!"pub const SPLINE_MAX_KNOTS_V2: usize = {maxKnots};"
  IO.println s!"pub const SPLINE_MAX_WIDTH_V2: usize = {maxWidth};"
  IO.println s!"pub const SPLINE_MAGIC_OFFSET_V2: usize = {magicOffset};"
  IO.println s!"pub const SPLINE_VERSION_OFFSET_V2: usize = {versionOffset};"
  IO.println s!"pub const SPLINE_PROFILE_OFFSET_V2: usize = {profileOffset};"
  IO.println s!"pub const SPLINE_SCALE_OFFSET_V2: usize = {scaleOffset};"
  IO.println s!"pub const SPLINE_KNOT_DENOMINATOR_OFFSET_V2: usize = {knotDenominatorOffset};"
  IO.println s!"pub const SPLINE_COORDINATE_DENOMINATOR_OFFSET_V2: usize = {coordinateDenominatorOffset};"
  IO.println s!"pub const SPLINE_COORDINATE_NUMERATOR_OFFSET_V2: usize = {coordinateNumeratorOffset};"
  IO.println s!"pub const SPLINE_DEGREE_OFFSET_V2: usize = {degreeOffset};"
  IO.println s!"pub const SPLINE_KNOT_COUNT_OFFSET_V2: usize = {knotCountOffset};"
  IO.println s!"pub const SPLINE_RESERVED_OFFSET_V2: usize = {reservedOffset};"
  IO.println s!"pub const SPLINE_RESERVED_BYTES_V2: usize = {reservedBytes};"
  IO.println s!"pub const SPLINE_KNOTS_OFFSET_V2: usize = {knotsOffset};"
  IO.println s!"pub const SPLINE_KNOT_BYTES_V2: usize = {knotBytes};"
  IO.println s!"pub const SPLINE_REFUSAL_BUFFER_V2: usize = {requestBytes + 1};"
  IO.println s!"pub const SPLINE_MAGIC_V2: [u8; 8] = {rustBytes requestMagic};"
  IO.println s!"pub const SPLINE_AGREEMENT_CASES_V2: [SplineAgreementCaseV2; {agreementRequests.length}] = ["
  let mut index := 0
  for candidate in agreementRequests do
    emitAgreement index candidate
    index := index + 1
  IO.println "];"
  IO.println s!"pub const SPLINE_REFUSAL_CASES_V2: [SplineRefusalCaseV2; {hostileRequests.length}] = ["
  let mut hostileIndex := 0
  for bytes in hostileRequests do
    emitRefusal hostileIndex bytes
    hostileIndex := hostileIndex + 1
  IO.println "];"
