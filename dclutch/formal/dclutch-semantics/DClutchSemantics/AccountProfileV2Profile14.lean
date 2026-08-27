import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# AccountProfile V2 Profile 14 fixed-data prestate predicates

Profile 14 preserves Profile 13's dynamic-span and route-alias semantics and
adds a canonical, read-only predicate table over fixed, self-representative
account data.  This module owns the new fixed coordinates and the admissible
predicate shapes.  The safe Rust kernel remains the executable interpreter;
the SVM adapter remains outside this theorem boundary.
-/

namespace DClutch.AccountProfileV2Profile14

open DClutch.AbiSchema

def artifactProfile : Nat := 14
def headerBytes : Nat := 48
def dynamicSpanEntryBytes : Nat := 20
def predicateBytes : Nat := 16
def profilePreimage : List UInt8 :=
  "dclutch/account-profile-v2/profile14/fixed-data-prestate-predicates-v1".toUTF8.toList
def profileId : List UInt8 := [
  0x3a, 0x12, 0x29, 0x57, 0xe4, 0xb2, 0x53, 0x2e,
  0xc0, 0x06, 0x41, 0x06, 0xb6, 0x8c, 0xf5, 0xfe,
  0x0a, 0x9f, 0xa8, 0x14, 0xb1, 0x12, 0x50, 0x87,
  0xed, 0x45, 0x7e, 0x7d, 0x44, 0x3b, 0x26, 0xda
]

def dynamicSpanCountOffset : Nat := 40
def predicateCountOffset : Nat := 42
def headerReservedOffset : Nat := 44

inductive PredicateField where
  | opcode | reserved | account | dataOffset | payload
  deriving DecidableEq, Repr

def predicateSchema : List (FieldSpec PredicateField) := [
  ⟨.opcode, .bytes 1⟩,
  ⟨.reserved, .reserved 1⟩,
  ⟨.account, .u16⟩,
  ⟨.dataOffset, .u32⟩,
  ⟨.payload, .bytes 8⟩
]

def predicateLayout : List (PlacedField PredicateField) := specialize predicateSchema

namespace PredicateField

def rustName : PredicateField → String
  | .opcode => "FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2"
  | .reserved => "FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2"
  | .account => "FIXED_DATA_PREDICATE_ACCOUNT_OFFSET_V2"
  | .dataOffset => "FIXED_DATA_PREDICATE_DATA_OFFSET_V2"
  | .payload => "FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2"

end PredicateField

inductive PredicateKind where
  | requireDataU8 | requireDataU16 | requireDataU32 | requireDataU64 | requireZeroRange
  deriving DecidableEq, Repr

def PredicateKind.opcode : PredicateKind → Nat
  | .requireDataU8 => 1
  | .requireDataU16 => 2
  | .requireDataU32 => 3
  | .requireDataU64 => 4
  | .requireZeroRange => 5

def PredicateKind.width : PredicateKind → Nat → Nat
  | .requireDataU8, _ => 1
  | .requireDataU16, _ => 2
  | .requireDataU32, _ => 4
  | .requireDataU64, _ => 8
  | .requireZeroRange, payload => payload

structure PredicateShape where
  kind : PredicateKind
  account : Nat
  dataOffset : Nat
  payload : Nat
  inactivePayloadZero : Bool
  deriving DecidableEq, Repr

def canonical (shape : PredicateShape) : Bool :=
  shape.inactivePayloadZero &&
  (match shape.kind with
   | .requireZeroRange => shape.payload != 0
   | _ => true) &&
  shape.dataOffset + shape.kind.width shape.payload <= 0xffffffff

def before (left right : PredicateShape) : Bool :=
  left.account < right.account ||
  (left.account == right.account &&
    left.dataOffset + left.kind.width left.payload <= right.dataOffset &&
    left.dataOffset < right.dataOffset)

def canonicalTable (values : List PredicateShape) : Bool :=
  values.all canonical && values.Pairwise (fun left right => before left right = true)

def agreement : List PredicateShape := [
  ⟨.requireDataU64, 0, 0, 0x325041544c4344, true⟩,
  ⟨.requireDataU16, 0, 8, 2, true⟩,
  ⟨.requireZeroRange, 0, 10, 6, true⟩,
  ⟨.requireDataU8, 1, 16, 7, true⟩
]

def refusalCorpus : List (List PredicateShape) := [
  [⟨.requireZeroRange, 0, 0, 0, true⟩],
  [⟨.requireDataU8, 0, 1, 1, false⟩],
  [⟨.requireDataU16, 0, 8, 2, true⟩, ⟨.requireDataU16, 0, 8, 2, true⟩],
  [⟨.requireDataU64, 0, 4, 9, true⟩, ⟨.requireDataU8, 0, 8, 1, true⟩]
]

theorem predicate_width_is_exact : schemaWidth predicateSchema = predicateBytes := by
  native_decide

theorem predicate_coordinates_are_canonical : coordinates predicateLayout = [
    (.opcode, 0, 1), (.reserved, 1, 1), (.account, 2, 2),
    (.dataOffset, 4, 4), (.payload, 8, 8)
  ] := by native_decide

theorem predicate_fields_are_disjoint : predicateLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 predicateSchema

theorem agreement_is_canonical : canonicalTable agreement = true := by native_decide

theorem hostile_corpus_refuses :
    refusalCorpus.all (fun hostile => canonicalTable hostile = false) := by native_decide

theorem profile_coordinates_are_exact :
    artifactProfile = 14 ∧ headerBytes = 48 ∧ dynamicSpanEntryBytes = 20 ∧
      predicateBytes = 16 ∧ dynamicSpanCountOffset = 40 ∧
      predicateCountOffset = 42 ∧ headerReservedOffset = 44 := by
  native_decide

theorem profile_identity_has_exact_width :
    profilePreimage.length = 70 ∧ profileId.length = 32 := by native_decide

end DClutch.AccountProfileV2Profile14
