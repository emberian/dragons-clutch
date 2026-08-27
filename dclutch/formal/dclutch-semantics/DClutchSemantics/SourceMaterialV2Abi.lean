import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Compact Source-material V2 ABI

The immutable Source record keeps one foreign Product coordinate: the exact
content digest of `ProductRecordV2`. Stable Product identity, result-domain
identity, partition width, and cells are deliberately absent and must be
derived from the authenticated Product Runtime V2 graph. Market and generation
belong only to the later mutable Source state, avoiding a Market-PDA hash cycle.
-/

namespace DClutch.SourceMaterialV2Abi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x53, 0x4d, 0x56, 0x32]
def schemaVersion : Nat := 2
def schemaReleasePreimage : List UInt8 :=
  "dclutch/source-material-schema/v2".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x5d, 0x98, 0x9b, 0x8d, 0x65, 0xac, 0xbf, 0xee,
  0x49, 0x70, 0x08, 0xdd, 0x08, 0x99, 0x41, 0xea,
  0x7b, 0x45, 0x3f, 0x6c, 0x98, 0x2b, 0x59, 0xe7,
  0xfa, 0x91, 0xf1, 0xcf, 0x5a, 0x3e, 0xff, 0xf2
]
def derivationReleasePreimage : List UInt8 :=
  "dclutch/source-material-record-derivation/v2".toUTF8.toList
def derivationReleaseId : List UInt8 := [
  0xe0, 0xdb, 0x56, 0x77, 0x07, 0x90, 0xc9, 0x44,
  0x49, 0x69, 0xa4, 0x2d, 0xa5, 0xa1, 0x10, 0x25,
  0xeb, 0xab, 0x80, 0x4e, 0x05, 0x55, 0x10, 0xbc,
  0x89, 0xf9, 0x25, 0xa6, 0x7b, 0x26, 0x3f, 0x8c
]
def failurePolicyReleasePreimage : List UInt8 :=
  "dclutch/source-failure-after-recovery-exhausted/v2".toUTF8.toList
def failurePolicyReleaseId : List UInt8 := [
  0x59, 0x51, 0x15, 0xed, 0xdf, 0xa8, 0x6c, 0x16,
  0x81, 0x27, 0x39, 0x90, 0xbb, 0x88, 0x6b, 0x52,
  0x86, 0x66, 0xd1, 0x61, 0x1f, 0xab, 0x22, 0x3a,
  0x05, 0x16, 0x09, 0x76, 0x6d, 0x17, 0xd4, 0xe8
]

inductive Field where
  | magic | version | recoveryPresent | reserved | productRecordDigest | primarySourceSpec
  | windowSpec | statisticSpec | recoveryPolicy | failurePolicyRelease
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.recoveryPresent, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.productRecordDigest, .bytes 32⟩,
  ⟨.primarySourceSpec, .bytes 32⟩,
  ⟨.windowSpec, .bytes 32⟩,
  ⟨.statisticSpec, .bytes 32⟩,
  ⟨.recoveryPolicy, .bytes 32⟩,
  ⟨.failurePolicyRelease, .bytes 32⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "SOURCE_MATERIAL_V2_MAGIC_OFFSET"
  | .version => "SOURCE_MATERIAL_V2_VERSION_OFFSET"
  | .recoveryPresent => "SOURCE_MATERIAL_V2_RECOVERY_PRESENT_OFFSET"
  | .reserved => "SOURCE_MATERIAL_V2_RESERVED_OFFSET"
  | .productRecordDigest => "SOURCE_MATERIAL_V2_PRODUCT_RECORD_DIGEST_OFFSET"
  | .primarySourceSpec => "SOURCE_MATERIAL_V2_PRIMARY_SOURCE_SPEC_OFFSET"
  | .windowSpec => "SOURCE_MATERIAL_V2_WINDOW_SPEC_OFFSET"
  | .statisticSpec => "SOURCE_MATERIAL_V2_STATISTIC_SPEC_OFFSET"
  | .recoveryPolicy => "SOURCE_MATERIAL_V2_RECOVERY_POLICY_OFFSET"
  | .failurePolicyRelease => "SOURCE_MATERIAL_V2_FAILURE_POLICY_RELEASE_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0

end Field

theorem exact_width : bytes = 208 := by native_decide

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
  value.failurePolicyRelease != 0 && fitsId value.failurePolicyRelease

def encode (value : Material) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++
  [if value.recoveryPresent then 1 else 0] ++ List.replicate 5 0 ++
  Codec.encodeLE 32 value.productRecordDigest ++
  Codec.encodeLE 32 value.primarySourceSpec ++
  Codec.encodeLE 32 value.windowSpec ++
  Codec.encodeLE 32 value.statisticSpec ++
  Codec.encodeLE 32 value.recoveryPolicy ++
  Codec.encodeLE 32 value.failurePolicyRelease

theorem encoding_length (value : Material) : (encode value).length = bytes := by
  simp [encode, bytes, schema, schemaWidth, magic, Codec.encodeLE_length,
    FieldKind.byteWidth]

/-- Source contains exactly one Product foreign key; no stable Product ID,
domain digest, partition count, or partition cell has an ABI coordinate. -/
theorem sole_product_foreign_key :
    (schema.filter fun field => field.name = .productRecordDigest).length = 1 := by
  native_decide

def exampleMaterial : Material := {
  productRecordDigest := 1
  primarySourceSpec := 2
  windowSpec := 3
  statisticSpec := 4
  recoveryPresent := true
  recoveryPolicy := 5
  failurePolicyRelease := 6
}

theorem example_valid : exampleMaterial.valid = true := by native_decide

def zeroRecoveryExample : Material := {
  exampleMaterial with recoveryPresent := false, recoveryPolicy := 0
}

theorem zero_recovery_is_canonical : zeroRecoveryExample.valid = true := by native_decide

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def validBytes (input : List UInt8) : Bool :=
  input.length = bytes && input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (sliceNat input Field.recoveryPresent.offset 1 = 0 ||
    sliceNat input Field.recoveryPresent.offset 1 = 1) &&
  (input.drop Field.reserved.offset).take 5 = List.replicate 5 0 &&
  sliceNat input Field.productRecordDigest.offset 32 != 0 &&
  sliceNat input Field.primarySourceSpec.offset 32 != 0 &&
  sliceNat input Field.windowSpec.offset 32 != 0 &&
  sliceNat input Field.statisticSpec.offset 32 != 0 &&
  (if sliceNat input Field.recoveryPresent.offset 1 = 1 then
    sliceNat input Field.recoveryPolicy.offset 32 != 0
  else sliceNat input Field.recoveryPolicy.offset 32 = 0) &&
  sliceNat input Field.failurePolicyRelease.offset 32 != 0

def refusalCorpus : List (List UInt8) := [
  (encode exampleMaterial).set 0 0,
  (encode exampleMaterial).set Field.version.offset 3,
  (encode exampleMaterial).set Field.recoveryPresent.offset 2,
  (encode exampleMaterial).set Field.reserved.offset 1,
  (encode exampleMaterial).set Field.productRecordDigest.offset 0,
  (encode exampleMaterial).set Field.primarySourceSpec.offset 0,
  (encode exampleMaterial).set Field.windowSpec.offset 0,
  (encode exampleMaterial).set Field.statisticSpec.offset 0,
  (encode exampleMaterial).set Field.recoveryPolicy.offset 0,
  (encode zeroRecoveryExample).set Field.recoveryPolicy.offset 5,
  (encode exampleMaterial).set Field.failurePolicyRelease.offset 0
]

theorem example_bytes_accepted : validBytes (encode exampleMaterial) = true := by native_decide

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

def productRecordMatches (material : Material) (authenticatedRecord : Nat) : Bool :=
  material.valid && material.productRecordDigest = authenticatedRecord

theorem substituted_product_record_refuses
    (material : Material) (authenticatedRecord : Nat)
    (valid : material.valid = true)
    (substituted : material.productRecordDigest ≠ authenticatedRecord) :
    productRecordMatches material authenticatedRecord = false := by
  simp [productRecordMatches, valid, substituted]

end DClutch.SourceMaterialV2Abi
