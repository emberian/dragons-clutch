import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# State Lifecycle Policy V5 fixed ABI

This module owns only the fixed physical coordinates added by Lifecycle V5:
the common policy header and the bounded current-Rent quote declaration.  The
safe Rust kernel remains the executable owner of policy validation, account
profile joins, quote authentication, and lifecycle planning.  The outer
adapter remains responsible for current-Rent observation and runtime accounts.
-/

namespace DClutch.StateLifecyclePolicyV5Abi

open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x44, 0x50, 0x30, 0x33]
def schemaVersion : Nat := 3
def artifactProfile : Nat := 4
def maxCurrentRentQuotes : Nat := 16
def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/state-lifecycle-policy-v5-current-rent-quotes-v1".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x10, 0xfb, 0xed, 0x6c, 0x13, 0x26, 0x12, 0x7c,
  0xf7, 0xe5, 0x47, 0x83, 0xb1, 0xa5, 0x97, 0xd7,
  0x7c, 0xa3, 0xe7, 0x6b, 0x53, 0xde, 0x97, 0xc0,
  0x8f, 0x27, 0x3f, 0x5e, 0x67, 0xe3, 0x98, 0x3b
]

inductive HeaderField where
  | magic | schemaVersion | artifactProfile
  | recipeCount | seedCount | planCount | protectedOutputCount
  | immutableIdentityBindingCount | currentRentQuoteCount | reserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.recipeCount, .u16⟩,
  ⟨.seedCount, .u16⟩,
  ⟨.planCount, .u16⟩,
  ⟨.protectedOutputCount, .u16⟩,
  ⟨.immutableIdentityBindingCount, .u16⟩,
  ⟨.currentRentQuoteCount, .u16⟩,
  ⟨.reserved, .reserved 16⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema
def headerBytes : Nat := schemaWidth headerSchema

namespace HeaderField

def rustName : HeaderField → String
  | .magic => "STATE_LIFECYCLE_V5_MAGIC_OFFSET"
  | .schemaVersion => "STATE_LIFECYCLE_V5_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "STATE_LIFECYCLE_V5_ARTIFACT_PROFILE_OFFSET"
  | .recipeCount => "STATE_LIFECYCLE_V5_RECIPE_COUNT_OFFSET"
  | .seedCount => "STATE_LIFECYCLE_V5_SEED_COUNT_OFFSET"
  | .planCount => "STATE_LIFECYCLE_V5_PLAN_COUNT_OFFSET"
  | .protectedOutputCount => "STATE_LIFECYCLE_V5_PROTECTED_OUTPUT_COUNT_OFFSET"
  | .immutableIdentityBindingCount =>
      "STATE_LIFECYCLE_V5_IMMUTABLE_IDENTITY_BINDING_COUNT_OFFSET"
  | .currentRentQuoteCount => "STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_COUNT_OFFSET"
  | .reserved => "STATE_LIFECYCLE_V5_RESERVED_OFFSET"

def offset (field : HeaderField) : Nat :=
  (coordinate? field headerLayout).map (·.1) |>.getD 0

end HeaderField

inductive CurrentRentQuoteField where
  | exactDataLen | scalarDestination | actionScope | action | reserved
  deriving DecidableEq, Repr

-- The action tag is carved out of the FRONT of what used to be ten reserved
-- bytes, so an unscoped quote -- scope 0, action 0 -- is byte-identical to the
-- same quote before the tag existed. No artifact this schema has ever emitted
-- moves, and no pinned digest moves with it.
def currentRentQuoteSchema : List (FieldSpec CurrentRentQuoteField) := [
  ⟨.exactDataLen, .u32⟩,
  ⟨.scalarDestination, .u16⟩,
  ⟨.actionScope, .u8⟩,
  ⟨.action, .u32⟩,
  ⟨.reserved, .reserved 5⟩
]

def currentRentQuoteLayout : List (PlacedField CurrentRentQuoteField) :=
  specialize currentRentQuoteSchema
def currentRentQuoteBytes : Nat := schemaWidth currentRentQuoteSchema

namespace CurrentRentQuoteField

def rustName : CurrentRentQuoteField → String
  | .exactDataLen => "STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_EXACT_DATA_LEN_OFFSET"
  | .scalarDestination =>
      "STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_SCALAR_DESTINATION_OFFSET"
  | .actionScope => "STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_ACTION_SCOPE_OFFSET"
  | .action => "STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_ACTION_OFFSET"
  | .reserved => "STATE_LIFECYCLE_V5_CURRENT_RENT_QUOTE_RESERVED_OFFSET"

def offset (field : CurrentRentQuoteField) : Nat :=
  (coordinate? field currentRentQuoteLayout).map (·.1) |>.getD 0

end CurrentRentQuoteField

def canonicalEmptyHeader : List UInt8 :=
  magic ++
  DClutch.Codec.encodeLE 2 schemaVersion ++
  DClutch.Codec.encodeLE 2 artifactProfile ++
  List.replicate 12 0 ++
  List.replicate 16 0

def encodeCurrentRentQuoteScoped
    (exactDataLen scalarDestination : Nat) (action : Option Nat) : List UInt8 :=
  DClutch.Codec.encodeLE 4 exactDataLen ++
  DClutch.Codec.encodeLE 2 scalarDestination ++
  DClutch.Codec.encodeLE 1 (match action with | some _ => 1 | none => 0) ++
  DClutch.Codec.encodeLE 4 (action.getD 0) ++
  List.replicate 5 0

def encodeCurrentRentQuote (exactDataLen scalarDestination : Nat) : List UInt8 :=
  encodeCurrentRentQuoteScoped exactDataLen scalarDestination none

def currentRentQuoteAgreement : List UInt8 := encodeCurrentRentQuote 512 39

def currentRentQuoteCanonical (bytes : List UInt8) : Bool :=
  bytes.length == currentRentQuoteBytes &&
  DClutch.Codec.decodeLE
      ((bytes.drop (CurrentRentQuoteField.offset .exactDataLen)).take 4) != 0 &&
  -- Exactly one encoding of "every action": an unscoped quote carries no action,
  -- so a digest cannot be varied through bytes the decoder ignores.
  (DClutch.Codec.decodeLE
      ((bytes.drop (CurrentRentQuoteField.offset .actionScope)).take 1) == 0 ||
   DClutch.Codec.decodeLE
      ((bytes.drop (CurrentRentQuoteField.offset .actionScope)).take 1) == 1) &&
  (DClutch.Codec.decodeLE
      ((bytes.drop (CurrentRentQuoteField.offset .actionScope)).take 1) == 1 ||
   DClutch.Codec.decodeLE
      ((bytes.drop (CurrentRentQuoteField.offset .action)).take 4) == 0) &&
  (bytes.drop (CurrentRentQuoteField.offset .reserved)).take 5 == List.replicate 5 0

def currentRentQuoteRefusalCorpus : List (List UInt8) := [
  currentRentQuoteAgreement.drop 1,
  encodeCurrentRentQuote 0 39,
  -- Probes the NEW reserved run. The old third hostile set byte 6, which is now
  -- the action scope and is legal there; the non-canonical-reserved probe has to
  -- move with the field it probes.
  List.set currentRentQuoteAgreement
    (CurrentRentQuoteField.offset .reserved) 1,
  -- An unrecognised scope tag, refused rather than read as unscoped.
  List.set currentRentQuoteAgreement
    (CurrentRentQuoteField.offset .actionScope) 2,
  -- An unscoped quote carrying an action: the second encoding of "every action"
  -- that must not exist.
  List.set currentRentQuoteAgreement
    (CurrentRentQuoteField.offset .action) 1
]

theorem header_width_is_exact : headerBytes = 40 := by native_decide
theorem current_rent_quote_width_is_exact : currentRentQuoteBytes = 16 := by native_decide
theorem release_coordinates_have_exact_width :
    schemaReleasePreimage.length = 63 ∧ schemaReleaseId.length = 32 := by native_decide
theorem header_coordinates_are_canonical : coordinates headerLayout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2), (.artifactProfile, 10, 2),
    (.recipeCount, 12, 2), (.seedCount, 14, 2), (.planCount, 16, 2),
    (.protectedOutputCount, 18, 2), (.immutableIdentityBindingCount, 20, 2),
    (.currentRentQuoteCount, 22, 2), (.reserved, 24, 16)
  ] := by native_decide
theorem current_rent_quote_coordinates_are_canonical :
    coordinates currentRentQuoteLayout = [
      (.exactDataLen, 0, 4), (.scalarDestination, 4, 2), (.actionScope, 6, 1),
      (.action, 7, 4), (.reserved, 11, 5)
    ] := by native_decide
theorem layouts_are_disjoint :
    headerLayout.Pairwise Before ∧ currentRentQuoteLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 headerSchema,
    specializeFrom_pairwise 0 currentRentQuoteSchema⟩
theorem canonical_empty_header_has_exact_width : canonicalEmptyHeader.length = 40 := by
  native_decide
theorem current_rent_quote_agreement_is_canonical :
    currentRentQuoteCanonical currentRentQuoteAgreement = true := by native_decide
theorem current_rent_quote_refusal_corpus_is_noncanonical :
    currentRentQuoteRefusalCorpus.all
      (fun hostile => currentRentQuoteCanonical hostile = false) := by native_decide

end DClutch.StateLifecyclePolicyV5Abi
