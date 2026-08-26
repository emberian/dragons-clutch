import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Product-free Source recovery policy V2

This finalized record owns the ordered, funded recovery sequence selected by
`SourceMaterialV2`.  It contains no Product identity.  Product authority stays
at the Runtime V2 graph root and exhaustion-to-failure meaning stays at the
Source-material failure-policy release.
-/

namespace DClutch.SourceRecoveryPolicyV2Abi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x43, 0x56, 0x32]
def schemaVersion : Nat := 2
def schemaReleasePreimage : List UInt8 :=
  "dclutch/source-recovery-policy-schema/v2".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x05, 0x8f, 0x9d, 0xd0, 0x45, 0x7a, 0x7a, 0x22,
  0x0f, 0xbb, 0xde, 0x4b, 0x42, 0x19, 0xe5, 0x11,
  0x63, 0x46, 0x0a, 0xba, 0xf8, 0xd3, 0x35, 0xd6,
  0x86, 0x1a, 0x84, 0xd1, 0xe8, 0x5c, 0x09, 0x86
]
def maxAttempts : Nat := 4

inductive AttemptField where
  | sourceSpec | providerRelease | deadline | reserved | fundingAllocation
  deriving DecidableEq, Repr

def attemptSchema : List (FieldSpec AttemptField) := [
  ⟨.sourceSpec, .bytes 32⟩,
  ⟨.providerRelease, .bytes 32⟩,
  ⟨.deadline, .u64⟩,
  ⟨.reserved, .reserved 8⟩,
  ⟨.fundingAllocation, .bytes 32⟩
]

def attemptLayout : List (PlacedField AttemptField) := specialize attemptSchema
def attemptBytes : Nat := schemaWidth attemptSchema

namespace AttemptField

def rustName : AttemptField → String
  | .sourceSpec => "RECOVERY_ATTEMPT_V2_SOURCE_SPEC_OFFSET"
  | .providerRelease => "RECOVERY_ATTEMPT_V2_PROVIDER_RELEASE_OFFSET"
  | .deadline => "RECOVERY_ATTEMPT_V2_DEADLINE_OFFSET"
  | .reserved => "RECOVERY_ATTEMPT_V2_RESERVED_OFFSET"
  | .fundingAllocation => "RECOVERY_ATTEMPT_V2_FUNDING_ALLOCATION_OFFSET"

def offset (field : AttemptField) : Nat :=
  (coordinate? field attemptLayout).map (fun value => value.1) |>.getD 0

end AttemptField

inductive Field where
  | magic | version | attemptCount | reserved | capacityProfile
  | attempt0 | attempt1 | attempt2 | attempt3
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.attemptCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.capacityProfile, .bytes 32⟩,
  ⟨.attempt0, .bytes attemptBytes⟩,
  ⟨.attempt1, .bytes attemptBytes⟩,
  ⟨.attempt2, .bytes attemptBytes⟩,
  ⟨.attempt3, .bytes attemptBytes⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "RECOVERY_POLICY_V2_MAGIC_OFFSET"
  | .version => "RECOVERY_POLICY_V2_VERSION_OFFSET"
  | .attemptCount => "RECOVERY_POLICY_V2_ATTEMPT_COUNT_OFFSET"
  | .reserved => "RECOVERY_POLICY_V2_RESERVED_OFFSET"
  | .capacityProfile => "RECOVERY_POLICY_V2_CAPACITY_PROFILE_OFFSET"
  | .attempt0 => "RECOVERY_POLICY_V2_ATTEMPT_0_OFFSET"
  | .attempt1 => "RECOVERY_POLICY_V2_ATTEMPT_1_OFFSET"
  | .attempt2 => "RECOVERY_POLICY_V2_ATTEMPT_2_OFFSET"
  | .attempt3 => "RECOVERY_POLICY_V2_ATTEMPT_3_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0

end Field

theorem attempt_exact_width : attemptBytes = 112 := by native_decide
theorem exact_width : bytes = 496 := by native_decide
theorem attempt_schema_well_formed : WellFormed attemptSchema := by
  simp [WellFormed, attemptSchema, FieldKind.byteWidth]
theorem schema_well_formed : WellFormed schema := by
  simp [WellFormed, schema, attemptBytes, attemptSchema, schemaWidth, FieldKind.byteWidth]
theorem attempt_layout_disjoint : attemptLayout.Pairwise Before :=
  specializeFrom_pairwise 0 attemptSchema
theorem layout_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

structure Attempt where
  sourceSpec : Nat
  providerRelease : Nat
  deadline : Nat
  fundingAllocation : Nat
  deriving DecidableEq, Repr

def Attempt.valid (value : Attempt) : Bool :=
  value.sourceSpec != 0 && value.sourceSpec < 256 ^ 32 &&
  value.providerRelease != 0 && value.providerRelease < 256 ^ 32 &&
  value.deadline != 0 && value.deadline < 256 ^ 8 &&
  value.fundingAllocation != 0 && value.fundingAllocation < 256 ^ 32

def encodeAttempt (value : Attempt) : List UInt8 :=
  Codec.encodeLE 32 value.sourceSpec ++ Codec.encodeLE 32 value.providerRelease ++
  Codec.encodeLE 8 value.deadline ++ List.replicate 8 0 ++
  Codec.encodeLE 32 value.fundingAllocation

theorem attempt_encoding_length (value : Attempt) :
    (encodeAttempt value).length = attemptBytes := by
  simp [encodeAttempt, attemptBytes, attemptSchema, schemaWidth,
    Codec.encodeLE_length, FieldKind.byteWidth]

structure Policy where
  capacityProfile : Nat
  attempts : List Attempt
  deriving DecidableEq, Repr

def deadlinesIncreasing : List Attempt → Bool
  | [] | [_] => true
  | first :: second :: rest =>
      first.deadline < second.deadline && deadlinesIncreasing (second :: rest)

def Policy.valid (value : Policy) : Bool :=
  value.capacityProfile != 0 && value.capacityProfile < 256 ^ 32 &&
  value.attempts.length ≥ 1 && value.attempts.length ≤ maxAttempts &&
  value.attempts.all Attempt.valid && deadlinesIncreasing value.attempts

def paddedAttempts (attempts : List Attempt) : List UInt8 :=
  (attempts.take maxAttempts).flatMap encodeAttempt ++
  List.replicate ((maxAttempts - attempts.length) * attemptBytes) 0

def encode (value : Policy) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++ [UInt8.ofNat value.attempts.length] ++
  List.replicate 5 0 ++ Codec.encodeLE 32 value.capacityProfile ++
  paddedAttempts value.attempts

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def attemptAt (input : List UInt8) (index : Nat) : Attempt :=
  let offset := Field.attempt0.offset + index * attemptBytes
  {
    sourceSpec := sliceNat input (offset + AttemptField.sourceSpec.offset) 32
    providerRelease := sliceNat input (offset + AttemptField.providerRelease.offset) 32
    deadline := sliceNat input (offset + AttemptField.deadline.offset) 8
    fundingAllocation := sliceNat input (offset + AttemptField.fundingAllocation.offset) 32
  }

def attemptReservedValid (input : List UInt8) (index : Nat) : Bool :=
  let offset := Field.attempt0.offset + index * attemptBytes + AttemptField.reserved.offset
  (input.drop offset).take 8 = List.replicate 8 0

def slotValid (input : List UInt8) (count index : Nat) : Bool :=
  if index < count then (attemptAt input index).valid && attemptReservedValid input index
  else
    let offset := Field.attempt0.offset + index * attemptBytes
    (input.drop offset).take attemptBytes = List.replicate attemptBytes 0

def decodedAttempts (input : List UInt8) (count : Nat) : List Attempt :=
  (List.range count).map (attemptAt input)

def validBytes (input : List UInt8) : Bool :=
  let count := sliceNat input Field.attemptCount.offset 1
  let policy : Policy := {
    capacityProfile := sliceNat input Field.capacityProfile.offset 32
    attempts := decodedAttempts input count
  }
  input.length = bytes && input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reserved.offset).take 5 = List.replicate 5 0 &&
  policy.valid && (List.range maxAttempts).all (slotValid input count)

def examplePolicy : Policy := {
  capacityProfile := 1
  attempts := [
    { sourceSpec := 2, providerRelease := 3, deadline := 100, fundingAllocation := 4 },
    { sourceSpec := 5, providerRelease := 6, deadline := 200, fundingAllocation := 7 }
  ]
}

theorem example_valid : examplePolicy.valid = true := by native_decide
theorem example_encoding_length : (encode examplePolicy).length = bytes := by native_decide
theorem example_bytes_valid : validBytes (encode examplePolicy) = true := by native_decide

def refusalCorpus : List (List UInt8) := [
  (encode examplePolicy).set 0 0,
  (encode examplePolicy).set Field.version.offset 3,
  (encode examplePolicy).set Field.attemptCount.offset 0,
  (encode examplePolicy).set Field.attemptCount.offset 5,
  (encode examplePolicy).set Field.reserved.offset 1,
  (encode examplePolicy).set Field.capacityProfile.offset 0,
  (encode examplePolicy).set (Field.attempt0.offset + AttemptField.sourceSpec.offset) 0,
  (encode examplePolicy).set (Field.attempt0.offset + AttemptField.providerRelease.offset) 0,
  (encode examplePolicy).set (Field.attempt0.offset + AttemptField.deadline.offset) 0,
  (encode examplePolicy).set (Field.attempt0.offset + AttemptField.reserved.offset) 1,
  (encode examplePolicy).set (Field.attempt0.offset + AttemptField.fundingAllocation.offset) 0,
  (encode examplePolicy).set (Field.attempt1.offset + AttemptField.deadline.offset) 50,
  (encode examplePolicy).set Field.attempt2.offset 1
]

theorem refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

end DClutch.SourceRecoveryPolicyV2Abi
