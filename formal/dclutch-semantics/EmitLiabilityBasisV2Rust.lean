import DClutchSemantics.LiabilityBasisV2
import DClutchSemantics.Codec

/-!
Emit the exact provisional ramp ABI constants plus three finite corpora: a
semantic agreement corpus, a hostile-decoder refusal corpus, and a runtime
width transition corpus covering complete-set split, complete-set merge, and
single-claim terminal redemption with their stable refusal tags.

The handwritten Rust kernel consumes this output; this executable does not
emit Rust transition logic.
-/

open DClutch.LiabilityBasisV2
open DClutch.LiabilityBasisV2.PhysicalAbi
open DClutch.LiabilityBasisV2.PhysicalPlanner

def rustByte (byte : UInt8) : String := s!"0x{DClutch.Codec.byteHex byte}"

def rustBytes (bytes : List UInt8) : String :=
  s!"[{String.intercalate ", " (bytes.map rustByte)}]"

def request
    (scale knotDenominator : Nat) (left right coordinate : Int)
    (coordinateDenominator : Nat) : Request := {
  scale
  knotDenominator
  leftNumerator := left
  rightNumerator := right
  coordinateNumerator := coordinate
  coordinateDenominator
}

/-- Accepted requests. Every ramp edge case the Lean theorems name appears
here: both caps, both kinks exactly at a knot, the first and last interior
atoms, a floor residue the exact complement absorbs, negative knots, a
knot denominator above one, and the extremes of the `i64`/`u32` envelope. -/
def agreementRequests : List Request := [
  -- Strictly below the left knot: lower cap.
  request 100 1 0 10 (-1) 1,
  -- Exactly at the left knot: the lower kink.
  request 100 1 0 10 0 1,
  -- Interior thirds over two different coordinate denominators.
  request 10 1 0 1 1 3,
  request 10 1 0 1 2 6,
  request 10 1 0 1 2 3,
  -- Exactly at the right knot: the upper kink.
  request 100 1 0 10 10 1,
  -- Knots spanning zero.
  request 7 1 (-2) 2 0 1,
  -- Full `i64` and `u32` envelope.
  request 4294967295 4294967295 (-9223372036854775808)
    9223372036854775807 0 4294967295,
  -- First interior atom: strictly above the left knot, primary still zero.
  request 100 1 0 10 1 1000,
  -- Last interior atom: strictly below the right knot, primary below the cap.
  request 100 1 0 10 9999 1000,
  -- Strictly below a wholly negative knot pair.
  request 5 2 (-3) (-1) (-5) 2,
  -- Interior of a wholly negative knot pair.
  request 5 1 (-3) (-1) (-2) 1,
  -- Odd `u32`-maximum scale: the exact complement absorbs the residue atom.
  request 4294967295 1 0 2 1 1,
  -- Knot denominator above one, interior coordinate.
  request 9 4 1 3 5 8,
  -- Extreme negative scaled coordinate against the widest knot denominator.
  request 1000 4294967295 (-9223372036854775808) 9223372036854775807
    (-9223372036854775808) 1,
  -- Extreme positive scaled coordinate against the widest knot denominator.
  request 1000 4294967295 (-9223372036854775808) 9223372036854775807
    9223372036854775807 1
]

def changed (bytes : List UInt8) (offset : Nat) (value : UInt8) : List UInt8 :=
  bytes.set offset value

def zeroSpan (bytes : List UInt8) (offset width : Nat) : List UInt8 :=
  (List.range width).foldl (fun result index => result.set (offset + index) 0) bytes

/-- Refused requests. Every named ABI guard is exercised at more than one
byte position, and the refusal ordering is pinned by the combined case. -/
def hostileRequests : List (List UInt8) :=
  let base := encodeRequest (request 100 1 0 10 5 1)
  [
    changed base magicOffset 0,
    changed base 3 0x41,
    changed base 7 0,
    changed base versionOffset 3,
    changed base versionOffset 0,
    changed base versionOffset 1,
    changed base (versionOffset + 1) 1,
    changed base profileOffset 2,
    changed base profileOffset 0,
    changed base (profileOffset + 1) 1,
    changed base reservedOffset 1,
    changed base (reservedOffset + 7) 0x80,
    changed base (requestBytes - 1) 0xff,
    zeroSpan base scaleOffset 4,
    zeroSpan base knotDenominatorOffset 4,
    zeroSpan base coordinateDenominatorOffset 4,
    zeroSpan (zeroSpan base scaleOffset 4) knotDenominatorOffset 4,
    encodeRequest (request 100 1 0 0 0 1),
    encodeRequest (request 100 1 2 1 0 1)
  ]

/-- Widest runtime basis carried by the generated transition corpus. -/
def transitionMaxWidth : Nat := 4

def transition
    (operation : Operation) (supplies payouts : List Nat)
    (scale quantity claimIndex hoard : Nat) : Transition := {
  supplies
  payouts
  scale
  quantity
  claimIndex
  hoard
  operation
}

def u64Maximum : Nat := 2 ^ 64 - 1

/-- Runtime-width transition cases. The accepted block covers all three
operations at several widths, including a zero-quantity no-op and a
zero-payout redemption; the refused block reaches every refusal tag the
ordered check list can produce. -/
def transitionCases : List Transition := [
  -- Accepted: complete-set split against an interior ramp payout.
  transition .split [7, 11] [3, 7] 10 5 0 200,
  -- Accepted: complete-set split against a categorical `Q = 1` payout.
  transition .split [4, 9, 2, 6] [0, 1, 0, 0] 1 3 0 12,
  -- Accepted: zero-quantity split is an exact no-op.
  transition .split [7, 11] [3, 7] 10 0 0 200,
  -- Accepted: complete-set merge at both extremes of the backing check.
  transition .merge [7, 11] [3, 7] 10 7 0 200,
  transition .merge [5, 5, 5] [1, 0, 2] 3 5 0 40,
  -- Accepted: terminal redemption of a winning claim.
  transition .terminalRedeem [7, 11] [3, 7] 10 4 1 200,
  -- Accepted: terminal redemption of a zero-payout claim releases nothing.
  transition .terminalRedeem [7, 11] [0, 10] 10 7 0 200,
  -- Accepted: terminal redemption of an entire claim supply.
  transition .terminalRedeem [7, 11] [3, 7] 10 11 1 200,
  -- Accepted: split whose candidate Hoard is exactly the `u64` maximum.
  transition .split [1, 1] [0, 1] 1 4 0 (u64Maximum - 4),
  -- Refused 8: empty basis.
  transition .split [] [] 1 1 0 10,
  transition .split [1] [] 1 1 0 10,
  -- Refused 9: width mismatch.
  transition .split [1, 2] [1] 1 1 0 10,
  -- Refused 5: zero payout scale.
  transition .split [1, 2] [0, 0] 0 1 0 10,
  -- Refused 10: payouts that do not sum to the named scale.
  transition .split [1, 2] [3, 3] 10 1 0 10,
  -- Refused 10: a single payout above the named scale.
  transition .split [1, 2] [11, 0] 10 1 0 10,
  -- Refused 13: redemption coordinate outside the runtime width.
  transition .terminalRedeem [7, 11] [3, 7] 10 1 2 200,
  -- Refused 14: merge unbacked at one coordinate.
  transition .merge [7, 11] [3, 7] 10 8 0 200,
  -- Refused 14: redemption unbacked at the named coordinate.
  transition .terminalRedeem [7, 11] [3, 7] 10 12 1 200,
  -- Refused 11: incoming liability outside the `u64` envelope.
  transition .split [u64Maximum, u64Maximum] [1, 1] 2 1 0 u64Maximum,
  -- Refused 12: incoming liability above the Hoard.
  transition .split [7, 11] [3, 7] 10 1 0 10,
  -- Refused 11: locked collateral outside the `u64` envelope.
  transition .split [1, 1] [0, 2] 2 u64Maximum 0 u64Maximum,
  -- Refused 11: candidate Hoard outside the `u64` envelope.
  transition .split [1, 1] [0, 1] 1 8 0 (u64Maximum - 3),
  -- Refused 12: merge releasing more collateral than the Hoard holds.
  transition .merge [7, 11] [3, 7] 10 7 0 60,
  -- Refused 11: candidate supply outside the `u64` envelope.
  transition .split [u64Maximum, 0] [0, 1] 1 1 0 10
]

def rustNat (value : Nat) : String := toString value

def rustNatList (width : Nat) (values : List Nat) : String :=
  let padded := values ++ List.replicate (width - values.length) 0
  s!"[{String.intercalate ", " (padded.map rustNat)}]"

def requireRepresentable (index : Nat) (label : String) (value : Nat) : IO Unit :=
  if value ≤ u64Maximum then pure ()
  else throw <| IO.userError s!"transition case {index} field {label} exceeds u64"

def emitTransition (index : Nat) (transition : Transition) : IO Unit := do
  if transition.supplies.length > transitionMaxWidth then
    throw <| IO.userError s!"transition case {index} supplies exceed the corpus width"
  if transition.payouts.length > transitionMaxWidth then
    throw <| IO.userError s!"transition case {index} payouts exceed the corpus width"
  if transitionMaxWidth ≤ transition.claimIndex then
    throw <| IO.userError s!"transition case {index} claim coordinate exceeds the corpus width"
  for supply in transition.supplies do
    requireRepresentable index "supply" supply
  for payout in transition.payouts do
    requireRepresentable index "payout" payout
  requireRepresentable index "scale" transition.scale
  requireRepresentable index "quantity" transition.quantity
  requireRepresentable index "hoard" transition.hoard
  let (accepted, hoardAfter, liabilityBefore, liabilityAfter, tag) :=
    match transition.plan? with
    | .ok outcome =>
        (true, outcome.hoardAfter, outcome.liabilityBefore, outcome.liabilityAfter, 0)
    | .error tag => (false, 0, 0, 0, tag)
  if accepted then
    requireRepresentable index "hoard_after" hoardAfter
    requireRepresentable index "liability_before" liabilityBefore
    requireRepresentable index "liability_after" liabilityAfter
  IO.println "    TransitionCaseV2 {"
  IO.println s!"        supplies: {rustNatList transitionMaxWidth transition.supplies},"
  IO.println s!"        payouts: {rustNatList transitionMaxWidth transition.payouts},"
  IO.println s!"        width: {transition.supplies.length},"
  IO.println s!"        payout_width: {transition.payouts.length},"
  IO.println s!"        scale: {transition.scale},"
  IO.println s!"        quantity: {transition.quantity},"
  IO.println s!"        claim_index: {transition.claimIndex},"
  IO.println s!"        hoard: {transition.hoard},"
  IO.println s!"        operation: {transition.operation.tag},"
  IO.println s!"        accepted: {if accepted then "true" else "false"},"
  IO.println s!"        hoard_after: {hoardAfter},"
  IO.println s!"        liability_before: {liabilityBefore},"
  IO.println s!"        liability_after: {liabilityAfter},"
  IO.println s!"        error_tag: {tag},"
  IO.println "    },"

def emitAgreement (index : Nat) (request : Request) : IO Unit := do
  let decoded ← match decodeRequest (encodeRequest request) with
    | .ok decoded => pure decoded
    | .error error => throw <| IO.userError s!"agreement request {index} decoded as {error}"
  let expected ← match decoded.evaluate? with
    | some [primary, complement] => pure [primary, complement]
    | _ => throw <| IO.userError s!"agreement request {index} did not evaluate"
  IO.println "    AgreementCaseV2 {"
  IO.println s!"        request: {rustBytes (encodeRequest request)},"
  IO.println s!"        expected: [{expected[0]!}, {expected[1]!}],"
  IO.println "    },"

def emitRefusal (index : Nat) (request : List UInt8) : IO Unit := do
  let error ← match decodeRequest request with
    | .error error => pure error
    | .ok _ => throw <| IO.userError s!"hostile request {index} was admitted"
  IO.println "    RefusalCaseV2 {"
  IO.println s!"        request: {rustBytes request},"
  IO.println s!"        error_tag: {error},"
  IO.println "    },"

def main : IO Unit := do
  IO.println "// @generated by formal/dclutch-semantics/EmitLiabilityBasisV2Rust.lean; do not edit."
  IO.println "use super::{AgreementCaseV2, RefusalCaseV2, TransitionCaseV2};"
  IO.println s!"pub const RAMP_REQUEST_BYTES_V2: usize = {requestBytes};"
  IO.println s!"pub const RAMP_SCHEMA_VERSION_V2: u16 = {schemaVersion};"
  IO.println s!"pub const RAMP_PROFILE_V2: u16 = {profile};"
  IO.println s!"pub const RAMP_MAGIC_OFFSET_V2: usize = {magicOffset};"
  IO.println s!"pub const RAMP_VERSION_OFFSET_V2: usize = {versionOffset};"
  IO.println s!"pub const RAMP_PROFILE_OFFSET_V2: usize = {profileOffset};"
  IO.println s!"pub const RAMP_SCALE_OFFSET_V2: usize = {scaleOffset};"
  IO.println s!"pub const RAMP_KNOT_DENOMINATOR_OFFSET_V2: usize = {knotDenominatorOffset};"
  IO.println s!"pub const RAMP_LEFT_NUMERATOR_OFFSET_V2: usize = {leftNumeratorOffset};"
  IO.println s!"pub const RAMP_RIGHT_NUMERATOR_OFFSET_V2: usize = {rightNumeratorOffset};"
  IO.println s!"pub const RAMP_COORDINATE_NUMERATOR_OFFSET_V2: usize = {coordinateNumeratorOffset};"
  IO.println s!"pub const RAMP_COORDINATE_DENOMINATOR_OFFSET_V2: usize = {coordinateDenominatorOffset};"
  IO.println s!"pub const RAMP_RESERVED_OFFSET_V2: usize = {reservedOffset};"
  IO.println s!"pub const RAMP_RESERVED_BYTES_V2: usize = {reservedBytes};"
  IO.println s!"pub const RAMP_MAGIC_V2: [u8; 8] = {rustBytes requestMagic};"
  IO.println s!"pub const AGREEMENT_CASES_V2: [AgreementCaseV2; {agreementRequests.length}] = ["
  for indexed in agreementRequests.zipIdx do
    emitAgreement indexed.2 indexed.1
  IO.println "];"
  IO.println s!"pub const REFUSAL_CASES_V2: [RefusalCaseV2; {hostileRequests.length}] = ["
  for indexed in hostileRequests.zipIdx do
    emitRefusal indexed.2 indexed.1
  IO.println "];"
  IO.println s!"pub const TRANSITION_MAX_WIDTH_V2: usize = {transitionMaxWidth};"
  IO.println s!"pub const TRANSITION_CASES_V2: [TransitionCaseV2; {transitionCases.length}] = ["
  for indexed in transitionCases.zipIdx do
    emitTransition indexed.2 indexed.1
  IO.println "];"
