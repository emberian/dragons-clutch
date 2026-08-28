import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Compact Source-material V3 ABI

V3 makes the Source graph root the sole owner of principal policy. A bounded
material selects one exact `ManipulationFloorV1` content identity; an explicitly
unbounded material selects no floor. The resulting content graph is acyclic:

`CapacityProfile -> SourceSpec -> ManipulationFloor -> SourceMaterialV3`.

The Market commits the Source-material identity, so a founding witness cannot
substitute a second, larger floor that happens to carry the same Source, adapter,
and collateral bindings.
-/

namespace DClutch.SourceMaterialV3Abi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x53, 0x4d, 0x56, 0x33]
def schemaVersion : Nat := 3
def schemaReleasePreimage : List UInt8 :=
  "dclutch/source-material-schema/v3".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x46, 0x41, 0x59, 0xe8, 0xe2, 0xe5, 0xd3, 0x18,
  0x1b, 0x57, 0x6b, 0xb2, 0x21, 0x65, 0x65, 0x1d,
  0x4c, 0xda, 0x0b, 0x54, 0xf5, 0x9d, 0x6f, 0xe0,
  0xd1, 0xa4, 0x88, 0x2a, 0xb8, 0x18, 0xa3, 0xcc
]
def derivationReleasePreimage : List UInt8 :=
  "dclutch/source-material-record-derivation/v3".toUTF8.toList
def derivationReleaseId : List UInt8 := [
  0x55, 0x19, 0x34, 0xbe, 0xf0, 0x55, 0xe0, 0x6e,
  0x1c, 0xb3, 0xfe, 0xf0, 0x4a, 0xc7, 0x43, 0xe3,
  0x52, 0x53, 0xda, 0xe0, 0x96, 0xa0, 0x79, 0x06,
  0xe5, 0x23, 0xf8, 0x4a, 0xb7, 0x31, 0x4f, 0x50
]

def explicitlyUnboundedTag : Nat := 1
def boundedByFloorTag : Nat := 2

inductive PrincipalPolicy where
  | explicitlyUnbounded
  | boundedByFloor
  deriving DecidableEq, Repr

def PrincipalPolicy.tag : PrincipalPolicy -> Nat
  | .explicitlyUnbounded => explicitlyUnboundedTag
  | .boundedByFloor => boundedByFloorTag

inductive Field where
  | magic | version | recoveryPresent | principalPolicy | reserved
  | productRecordDigest | primarySourceSpec | windowSpec | statisticSpec
  | recoveryPolicy | failurePolicyRelease | manipulationFloor
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.recoveryPresent, .u8⟩,
  ⟨.principalPolicy, .u8⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.productRecordDigest, .bytes 32⟩,
  ⟨.primarySourceSpec, .bytes 32⟩,
  ⟨.windowSpec, .bytes 32⟩,
  ⟨.statisticSpec, .bytes 32⟩,
  ⟨.recoveryPolicy, .bytes 32⟩,
  ⟨.failurePolicyRelease, .bytes 32⟩,
  ⟨.manipulationFloor, .bytes 32⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field -> String
  | .magic => "SOURCE_MATERIAL_V3_MAGIC_OFFSET"
  | .version => "SOURCE_MATERIAL_V3_VERSION_OFFSET"
  | .recoveryPresent => "SOURCE_MATERIAL_V3_RECOVERY_PRESENT_OFFSET"
  | .principalPolicy => "SOURCE_MATERIAL_V3_PRINCIPAL_POLICY_OFFSET"
  | .reserved => "SOURCE_MATERIAL_V3_RESERVED_OFFSET"
  | .productRecordDigest => "SOURCE_MATERIAL_V3_PRODUCT_RECORD_DIGEST_OFFSET"
  | .primarySourceSpec => "SOURCE_MATERIAL_V3_PRIMARY_SOURCE_SPEC_OFFSET"
  | .windowSpec => "SOURCE_MATERIAL_V3_WINDOW_SPEC_OFFSET"
  | .statisticSpec => "SOURCE_MATERIAL_V3_STATISTIC_SPEC_OFFSET"
  | .recoveryPolicy => "SOURCE_MATERIAL_V3_RECOVERY_POLICY_OFFSET"
  | .failurePolicyRelease => "SOURCE_MATERIAL_V3_FAILURE_POLICY_RELEASE_OFFSET"
  | .manipulationFloor => "SOURCE_MATERIAL_V3_MANIPULATION_FLOOR_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0

end Field

theorem exact_width : bytes = 240 := by native_decide

theorem schema_well_formed : WellFormed schema := by
  simp [WellFormed, schema, FieldKind.byteWidth]

theorem layout_is_byte_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

structure Material where
  productRecordDigest : Nat
  primarySourceSpec : Nat
  windowSpec : Nat
  statisticSpec : Nat
  recoveryPresent : Bool
  recoveryPolicy : Nat
  failurePolicyRelease : Nat
  principalPolicy : PrincipalPolicy
  manipulationFloor : Nat
  deriving DecidableEq, Repr

def fitsId (value : Nat) : Bool := value < 256 ^ 32

def Material.valid (value : Material) : Bool :=
  value.productRecordDigest != 0 && fitsId value.productRecordDigest &&
  value.primarySourceSpec != 0 && fitsId value.primarySourceSpec &&
  value.windowSpec != 0 && fitsId value.windowSpec &&
  value.statisticSpec != 0 && fitsId value.statisticSpec &&
  (if value.recoveryPresent then
    value.recoveryPolicy != 0 && fitsId value.recoveryPolicy
  else value.recoveryPolicy = 0) &&
  value.failurePolicyRelease != 0 && fitsId value.failurePolicyRelease &&
  match value.principalPolicy with
  | .explicitlyUnbounded => value.manipulationFloor = 0
  | .boundedByFloor => value.manipulationFloor != 0 && fitsId value.manipulationFloor

def encode (value : Material) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++
  [if value.recoveryPresent then 1 else 0] ++
  [UInt8.ofNat value.principalPolicy.tag] ++ List.replicate 4 0 ++
  Codec.encodeLE 32 value.productRecordDigest ++
  Codec.encodeLE 32 value.primarySourceSpec ++
  Codec.encodeLE 32 value.windowSpec ++
  Codec.encodeLE 32 value.statisticSpec ++
  Codec.encodeLE 32 value.recoveryPolicy ++
  Codec.encodeLE 32 value.failurePolicyRelease ++
  Codec.encodeLE 32 value.manipulationFloor

theorem encoding_length (value : Material) : (encode value).length = bytes := by
  simp [encode, bytes, schema, schemaWidth, magic, Codec.encodeLE_length,
    FieldKind.byteWidth]

def boundedExample : Material := {
  productRecordDigest := 1
  primarySourceSpec := 2
  windowSpec := 3
  statisticSpec := 4
  recoveryPresent := true
  recoveryPolicy := 5
  failurePolicyRelease := 6
  principalPolicy := .boundedByFloor
  manipulationFloor := 7
}

def unboundedExample : Material := {
  boundedExample with
  recoveryPresent := false
  recoveryPolicy := 0
  principalPolicy := .explicitlyUnbounded
  manipulationFloor := 0
}

theorem bounded_example_valid : boundedExample.valid = true := by native_decide
theorem unbounded_example_valid : unboundedExample.valid = true := by native_decide

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def validBytes (input : List UInt8) : Bool :=
  input.length = bytes && input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (sliceNat input Field.recoveryPresent.offset 1 = 0 ||
    sliceNat input Field.recoveryPresent.offset 1 = 1) &&
  (sliceNat input Field.principalPolicy.offset 1 = explicitlyUnboundedTag ||
    sliceNat input Field.principalPolicy.offset 1 = boundedByFloorTag) &&
  (input.drop Field.reserved.offset).take 4 = List.replicate 4 0 &&
  sliceNat input Field.productRecordDigest.offset 32 != 0 &&
  sliceNat input Field.primarySourceSpec.offset 32 != 0 &&
  sliceNat input Field.windowSpec.offset 32 != 0 &&
  sliceNat input Field.statisticSpec.offset 32 != 0 &&
  (if sliceNat input Field.recoveryPresent.offset 1 = 1 then
    sliceNat input Field.recoveryPolicy.offset 32 != 0
  else sliceNat input Field.recoveryPolicy.offset 32 = 0) &&
  sliceNat input Field.failurePolicyRelease.offset 32 != 0 &&
  (if sliceNat input Field.principalPolicy.offset 1 = boundedByFloorTag then
    sliceNat input Field.manipulationFloor.offset 32 != 0
  else sliceNat input Field.manipulationFloor.offset 32 = 0)

def refusalCorpus : List (List UInt8) := [
  (encode boundedExample).set 0 0,
  (encode boundedExample).set Field.version.offset 2,
  (encode boundedExample).set Field.recoveryPresent.offset 2,
  (encode boundedExample).set Field.principalPolicy.offset 3,
  (encode boundedExample).set Field.reserved.offset 1,
  (encode boundedExample).set Field.productRecordDigest.offset 0,
  (encode boundedExample).set Field.primarySourceSpec.offset 0,
  (encode boundedExample).set Field.windowSpec.offset 0,
  (encode boundedExample).set Field.statisticSpec.offset 0,
  (encode boundedExample).set Field.recoveryPolicy.offset 0,
  (encode unboundedExample).set Field.recoveryPolicy.offset 5,
  (encode boundedExample).set Field.failurePolicyRelease.offset 0,
  (encode boundedExample).set Field.manipulationFloor.offset 0,
  (encode unboundedExample).set Field.manipulationFloor.offset 7
]

theorem bounded_example_bytes_accepted : validBytes (encode boundedExample) = true := by
  native_decide

theorem unbounded_example_bytes_accepted : validBytes (encode unboundedExample) = true := by
  native_decide

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

def selectedFloorMatches (material : Material) (authenticatedFloor : Nat) : Bool :=
  material.valid && material.principalPolicy = .boundedByFloor &&
  material.manipulationFloor = authenticatedFloor

theorem substituted_floor_refuses
    (material : Material) (authenticatedFloor : Nat)
    (valid : material.valid = true)
    (bounded : material.principalPolicy = .boundedByFloor)
    (substituted : material.manipulationFloor ≠ authenticatedFloor) :
    selectedFloorMatches material authenticatedFloor = false := by
  simp [selectedFloorMatches, valid, bounded, substituted]

theorem explicitly_unbounded_selects_no_floor
    (material : Material) (authenticatedFloor : Nat)
    (unbounded : material.principalPolicy = .explicitlyUnbounded) :
    selectedFloorMatches material authenticatedFloor = false := by
  simp [selectedFloorMatches, unbounded]

end DClutch.SourceMaterialV3Abi
