import DClutchSemantics.AbiSchema

/-!
# Realm and Position ABI

The sole byte-layout owner for the two immutable core records
`dclutch-realm-contract` publishes: a reusable collateral `Realm` and a compact
native `Position`.

It also owns the two SVM seed domains those records are addressed under.  A
seed domain is protocol meaning, not an implementation detail — it selects
which account a signature can move — and until this module existed it was
restated by hand in the crate, in the operator, and again in the browser.  It
is stated once here.
-/

namespace DClutch.RealmPositionAbi

open DClutch.AbiSchema

/-- Chain-derived maximum byte width of one SVM PDA seed component. -/
def svmMaxSeedBytes : Nat := 32

/-! ## Realm -/

def realmMagic : String := "DCLTRLM1"
def realmSchemaVersion : Nat := 1
/-- Domain seed preceding a Realm content identity in its PDA derivation. -/
def realmPdaDomain : String := "dclutch/realm/v1"

inductive RealmField where
  | magic | schemaVersion | mintAuthorityPolicy | freezeAuthorityPolicy
  | reserved | tokenProgram | collateralMint | adapterReleaseId
  deriving DecidableEq, Repr

def realmSchema : List (FieldSpec RealmField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.mintAuthorityPolicy, .u8⟩,
  ⟨.freezeAuthorityPolicy, .u8⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.tokenProgram, .bytes 32⟩,
  ⟨.collateralMint, .bytes 32⟩,
  ⟨.adapterReleaseId, .bytes 32⟩
]

def realmLayout : List (PlacedField RealmField) := specialize realmSchema
def realmBytes : Nat := schemaWidth realmSchema
/-- Width of the canonically-zero Realm reserved span. -/
def realmReservedBytes : Nat := 4

namespace RealmField

def constantName : RealmField → String
  | .magic => "REALM_MAGIC_OFFSET_V1"
  | .schemaVersion => "REALM_SCHEMA_VERSION_OFFSET_V1"
  | .mintAuthorityPolicy => "REALM_MINT_AUTHORITY_POLICY_OFFSET_V1"
  | .freezeAuthorityPolicy => "REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1"
  | .reserved => "REALM_RESERVED_OFFSET_V1"
  | .tokenProgram => "REALM_TOKEN_PROGRAM_OFFSET_V1"
  | .collateralMint => "REALM_COLLATERAL_MINT_OFFSET_V1"
  | .adapterReleaseId => "REALM_ADAPTER_RELEASE_ID_OFFSET_V1"

end RealmField

/-! ## Position

A Position carries `N` eight-byte outcome balances after a fixed base, so the
schema states the base and the balance stride; the two measured widths follow
from the provisional categorical profile rather than being separately asserted.
-/

def positionMagic : String := "DCLTPOS1"
def positionSchemaVersion : Nat := 1
/-- Domain seed preceding the exact Market and owner keys in a Position PDA
derivation. -/
def positionPdaDomain : String := "dclutch/position/v1"

/-- Minimum categorical width represented by the current measured profile. -/
def minOutcomes : Nat := 2
/-- Maximum categorical width in the current provisional measured profile. -/
def maxOutcomes : Nat := 16

inductive PositionField where
  | magic | schemaVersion | outcomeCount | reserved | market | owner | generation
  deriving DecidableEq, Repr

def positionSchema : List (FieldSpec PositionField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.owner, .bytes 32⟩,
  ⟨.generation, .u64⟩
]

def positionLayout : List (PlacedField PositionField) := specialize positionSchema
/-- Fixed Position bytes before its outcome balances. -/
def positionBaseBytes : Nat := schemaWidth positionSchema
/-- Width of the canonically-zero Position reserved span. -/
def positionReservedBytes : Nat := 5
/-- Stride of one outcome balance. -/
def outcomeBalanceBytes : Nat := 8

/-- Exact width of a Position of a given categorical width. -/
def positionBytes (outcomes : Nat) : Nat :=
  positionBaseBytes + outcomes * outcomeBalanceBytes

namespace PositionField

def constantName : PositionField → String
  | .magic => "POSITION_MAGIC_OFFSET_V1"
  | .schemaVersion => "POSITION_SCHEMA_VERSION_OFFSET_V1"
  | .outcomeCount => "POSITION_OUTCOME_COUNT_OFFSET_V1"
  | .reserved => "POSITION_RESERVED_OFFSET_V1"
  | .market => "POSITION_MARKET_OFFSET_V1"
  | .owner => "POSITION_OWNER_OFFSET_V1"
  | .generation => "POSITION_GENERATION_OFFSET_V1"

end PositionField

/-! ## Exact-value pins

These are the widths the deployed contracts and the browser already agree on.
They are stated here as theorems so a schema edit that would move a live
account boundary fails in Lean before it can reach either backend.
-/

theorem realm_width_is_exact : realmBytes = 112 := by native_decide
theorem realm_names_are_unique :
    (realmSchema.map fun field => field.name).Nodup := by native_decide
theorem realm_fields_are_disjoint :
    realmLayout.Pairwise Before := specializeFrom_pairwise 0 realmSchema
theorem realm_is_wellFormed : WellFormed realmSchema := by
  refine ⟨by native_decide, ?_⟩
  intro field member
  simp only [realmSchema, List.mem_cons, List.not_mem_nil, or_false] at member
  rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem position_base_width_is_exact : positionBaseBytes = 88 := by native_decide
theorem position_names_are_unique :
    (positionSchema.map fun field => field.name).Nodup := by native_decide
theorem position_fields_are_disjoint :
    positionLayout.Pairwise Before := specializeFrom_pairwise 0 positionSchema
theorem position_is_wellFormed : WellFormed positionSchema := by
  refine ⟨by native_decide, ?_⟩
  intro field member
  simp only [positionSchema, List.mem_cons, List.not_mem_nil, or_false] at member
  rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

/-- The binary Position width the Direct family transacts against. -/
theorem binary_position_width_is_exact : positionBytes minOutcomes = 104 := by
  native_decide
/-- The widest Position the provisional categorical profile admits. -/
theorem maximum_position_width_is_exact : positionBytes maxOutcomes = 216 := by
  native_decide

/-- Both seed domains fit one SVM seed component.  A domain that outgrew this
bound would not be a layout change; it would make every derived address
underivable, so the bound is proved rather than assumed. -/
theorem realm_domain_is_seedable :
    realmPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem position_domain_is_seedable :
    positionPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide

/-- The two domains are distinct, so no Realm address can be reached by a
Position derivation or the reverse. -/
theorem domains_are_distinct : realmPdaDomain ≠ positionPdaDomain := by
  native_decide

/-- The two record magics are distinct, so the browser's magic dispatch cannot
route one record to the other's decoder. -/
theorem magics_are_distinct : realmMagic ≠ positionMagic := by native_decide

end DClutch.RealmPositionAbi
