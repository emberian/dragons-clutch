import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Runtime-width Source resolution state V2

The mutable state binds one Market generation to the content digest of the
compact `SourceMaterialV2`.  It stores a native `u32` Product selector; Product
outcome width remains foreign authenticated context and is never copied into
the state.  `decisionValid` is the terminal read join used by Core.
-/

namespace DClutch.SourceResolutionStateV2Abi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x53, 0x52, 0x53, 0x32]
def schemaVersion : Nat := 2
def pdaDomain : List UInt8 := "dclutch/source-state/v2".toUTF8.toList

inductive Field where
  | magic | version | phase | activeAttempt | terminalRoute | pdaBump
  | reservedHeader | selector | reservedSelector
  | market | generation | materialDigest | rentBeneficiary
  | reopenLink | resolutionEvidence | terminalSequence
  | resolvedAt | retiredAt | reservedTail
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.phase, .u8⟩,
  ⟨.activeAttempt, .u8⟩,
  ⟨.terminalRoute, .u8⟩,
  ⟨.pdaBump, .u8⟩,
  ⟨.reservedHeader, .reserved 2⟩,
  ⟨.selector, .u32⟩,
  ⟨.reservedSelector, .reserved 4⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.materialDigest, .bytes 32⟩,
  ⟨.rentBeneficiary, .bytes 32⟩,
  ⟨.reopenLink, .bytes 32⟩,
  ⟨.resolutionEvidence, .bytes 32⟩,
  ⟨.terminalSequence, .u64⟩,
  ⟨.resolvedAt, .u64⟩,
  ⟨.retiredAt, .u64⟩,
  ⟨.reservedTail, .reserved 8⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "SOURCE_RESOLUTION_STATE_V2_MAGIC_OFFSET"
  | .version => "SOURCE_RESOLUTION_STATE_V2_VERSION_OFFSET"
  | .phase => "SOURCE_RESOLUTION_STATE_V2_PHASE_OFFSET"
  | .activeAttempt => "SOURCE_RESOLUTION_STATE_V2_ACTIVE_ATTEMPT_OFFSET"
  | .terminalRoute => "SOURCE_RESOLUTION_STATE_V2_TERMINAL_ROUTE_OFFSET"
  | .pdaBump => "SOURCE_RESOLUTION_STATE_V2_PDA_BUMP_OFFSET"
  | .reservedHeader => "SOURCE_RESOLUTION_STATE_V2_RESERVED_HEADER_OFFSET"
  | .selector => "SOURCE_RESOLUTION_STATE_V2_SELECTOR_OFFSET"
  | .reservedSelector => "SOURCE_RESOLUTION_STATE_V2_RESERVED_SELECTOR_OFFSET"
  | .market => "SOURCE_RESOLUTION_STATE_V2_MARKET_OFFSET"
  | .generation => "SOURCE_RESOLUTION_STATE_V2_GENERATION_OFFSET"
  | .materialDigest => "SOURCE_RESOLUTION_STATE_V2_MATERIAL_DIGEST_OFFSET"
  | .rentBeneficiary => "SOURCE_RESOLUTION_STATE_V2_RENT_BENEFICIARY_OFFSET"
  | .reopenLink => "SOURCE_RESOLUTION_STATE_V2_REOPEN_LINK_OFFSET"
  | .resolutionEvidence => "SOURCE_RESOLUTION_STATE_V2_RESOLUTION_EVIDENCE_OFFSET"
  | .terminalSequence => "SOURCE_RESOLUTION_STATE_V2_TERMINAL_SEQUENCE_OFFSET"
  | .resolvedAt => "SOURCE_RESOLUTION_STATE_V2_RESOLVED_AT_OFFSET"
  | .retiredAt => "SOURCE_RESOLUTION_STATE_V2_RETIRED_AT_OFFSET"
  | .reservedTail => "SOURCE_RESOLUTION_STATE_V2_RESERVED_TAIL_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0

end Field

theorem exact_width : bytes = 224 := by native_decide

theorem schema_well_formed : WellFormed schema := by
  simp [WellFormed, schema, FieldKind.byteWidth]

theorem layout_is_byte_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

structure State where
  phase : Nat
  activeAttempt : Nat
  terminalRoute : Nat
  pdaBump : Nat
  selector : Nat
  market : Nat
  generation : Nat
  materialDigest : Nat
  rentBeneficiary : Nat
  reopenLink : Nat
  resolutionEvidence : Nat
  terminalSequence : Nat
  resolvedAt : Nat
  retiredAt : Nat
  deriving DecidableEq, Repr

def fits (width value : Nat) : Bool := value < 256 ^ width
def terminalPhase (phase : Nat) : Bool := phase = 2 || phase = 4 || phase = 5

def State.valid (value : State) : Bool :=
  value.phase ≤ 5 && value.activeAttempt < 4 && value.terminalRoute ≤ 3 &&
  fits 1 value.pdaBump && fits 4 value.selector &&
  value.market != 0 && fits 32 value.market &&
  value.generation != 0 && fits 8 value.generation &&
  value.materialDigest != 0 && fits 32 value.materialDigest &&
  value.rentBeneficiary != 0 && fits 32 value.rentBeneficiary &&
  fits 32 value.reopenLink && fits 32 value.resolutionEvidence &&
  fits 8 value.terminalSequence && fits 8 value.resolvedAt && fits 8 value.retiredAt &&
  match value.phase with
  | 0 | 3 => value.activeAttempt = 0 && value.terminalRoute = 0 &&
      value.selector = 0 && value.resolutionEvidence = 0 &&
      value.terminalSequence = 0 && value.resolvedAt = 0 && value.retiredAt = 0
  | 1 => value.terminalRoute = 0 && value.selector = 0 &&
      value.resolutionEvidence = 0 && value.terminalSequence = 0 &&
      value.resolvedAt = 0 && value.retiredAt = 0
  | 2 => value.activeAttempt = 0 && (value.terminalRoute = 1 || value.terminalRoute = 2) &&
      value.resolutionEvidence != 0 && value.terminalSequence != 0 &&
      value.resolvedAt != 0 && value.retiredAt = 0
  | 4 => value.activeAttempt = 0 && value.terminalRoute = 3 &&
      value.resolutionEvidence != 0 && value.terminalSequence != 0 &&
      value.resolvedAt != 0 && value.retiredAt = 0
  | 5 => value.activeAttempt = 0 && value.terminalRoute != 0 &&
      value.resolutionEvidence != 0 && value.terminalSequence != 0 &&
      value.resolvedAt != 0 && value.retiredAt ≥ value.resolvedAt
  | _ => false

def encode (value : State) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++
  [UInt8.ofNat value.phase, UInt8.ofNat value.activeAttempt,
    UInt8.ofNat value.terminalRoute, UInt8.ofNat value.pdaBump] ++
  List.replicate 2 0 ++ Codec.encodeLE 4 value.selector ++ List.replicate 4 0 ++
  Codec.encodeLE 32 value.market ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 32 value.materialDigest ++ Codec.encodeLE 32 value.rentBeneficiary ++
  Codec.encodeLE 32 value.reopenLink ++ Codec.encodeLE 32 value.resolutionEvidence ++
  Codec.encodeLE 8 value.terminalSequence ++ Codec.encodeLE 8 value.resolvedAt ++
  Codec.encodeLE 8 value.retiredAt ++ List.replicate 8 0

theorem encoding_length (value : State) : (encode value).length = bytes := by
  simp [encode, bytes, schema, schemaWidth, magic, Codec.encodeLE_length,
    FieldKind.byteWidth]

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def decodedState (input : List UInt8) : State := {
  phase := sliceNat input Field.phase.offset 1
  activeAttempt := sliceNat input Field.activeAttempt.offset 1
  terminalRoute := sliceNat input Field.terminalRoute.offset 1
  pdaBump := sliceNat input Field.pdaBump.offset 1
  selector := sliceNat input Field.selector.offset 4
  market := sliceNat input Field.market.offset 32
  generation := sliceNat input Field.generation.offset 8
  materialDigest := sliceNat input Field.materialDigest.offset 32
  rentBeneficiary := sliceNat input Field.rentBeneficiary.offset 32
  reopenLink := sliceNat input Field.reopenLink.offset 32
  resolutionEvidence := sliceNat input Field.resolutionEvidence.offset 32
  terminalSequence := sliceNat input Field.terminalSequence.offset 8
  resolvedAt := sliceNat input Field.resolvedAt.offset 8
  retiredAt := sliceNat input Field.retiredAt.offset 8
}

def validBytes (input : List UInt8) : Bool :=
  input.length = bytes && input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reservedHeader.offset).take 2 = List.replicate 2 0 &&
  (input.drop Field.reservedSelector.offset).take 4 = List.replicate 4 0 &&
  (input.drop Field.reservedTail.offset).take 8 = List.replicate 8 0 &&
  (decodedState input).valid

def freshExample : State := {
  phase := 0, activeAttempt := 0, terminalRoute := 0, pdaBump := 7
  selector := 0, market := 1, generation := 9, materialDigest := 2
  rentBeneficiary := 3, reopenLink := 0, resolutionEvidence := 0
  terminalSequence := 0, resolvedAt := 0, retiredAt := 0
}

def wideTerminalExample : State := {
  freshExample with
    phase := 2
    terminalRoute := 1
    selector := 257
    resolutionEvidence := 4
    terminalSequence := 1
    resolvedAt := 100
}

theorem fresh_example_valid : freshExample.valid = true := by native_decide
theorem wide_terminal_example_valid : wideTerminalExample.valid = true := by native_decide

def decisionValid (state : State) (authenticatedOutcomeCount : Nat) : Bool :=
  state.valid && terminalPhase state.phase && authenticatedOutcomeCount ≥ 2 &&
  authenticatedOutcomeCount < 256 ^ 4 && state.selector < authenticatedOutcomeCount

theorem selector_257_is_not_truncated :
    decisionValid wideTerminalExample 258 = true := by native_decide

theorem selector_equal_to_count_refuses :
    decisionValid wideTerminalExample 257 = false := by native_decide

def refusalCorpus : List (List UInt8) := [
  (encode freshExample).set 0 0,
  (encode freshExample).set Field.version.offset 3,
  (encode freshExample).set Field.phase.offset 6,
  (encode freshExample).set Field.activeAttempt.offset 1,
  (encode freshExample).set Field.terminalRoute.offset 1,
  (encode freshExample).set Field.reservedHeader.offset 1,
  (encode freshExample).set Field.selector.offset 1,
  (encode freshExample).set Field.reservedSelector.offset 1,
  (encode freshExample).set Field.market.offset 0,
  (encode freshExample).set Field.generation.offset 0,
  (encode freshExample).set Field.materialDigest.offset 0,
  (encode freshExample).set Field.rentBeneficiary.offset 0,
  (encode freshExample).set Field.resolutionEvidence.offset 1,
  (encode freshExample).set Field.terminalSequence.offset 1,
  (encode freshExample).set Field.resolvedAt.offset 1,
  (encode freshExample).set Field.retiredAt.offset 1,
  (encode freshExample).set Field.reservedTail.offset 1
]

theorem encoded_examples_accepted :
    validBytes (encode freshExample) = true ∧
    validBytes (encode wideTerminalExample) = true := by native_decide

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

end DClutch.SourceResolutionStateV2Abi
