import DClutchSemantics.AbiSchema

/-!
# Realm ABI, and the Position seed domain

The sole byte-layout owner for the one immutable core record
`dclutch-realm-contract` publishes: a reusable collateral `Realm`.

It also owns two SVM seed domains.  A seed domain is protocol meaning, not an
implementation detail — it selects which account a signature can move — and
until this module existed it was restated by hand in the crate, in the
operator, and again in the browser.  It is stated once here.

## Why one domain outlived its record

This module used to state a second RECORD as well: the compact native
`PositionV1`, magic `DCLTPOS1`, eighty-eight base bytes plus a stride of
outcome balances.  That record was banished with the DCLTCAT1 stratum — its
only two consumers, `dclutch-direct-contract` and the browser's fixture
generator, were deleted in that series — and the emission outlived it on both
backends.  What that cost is exactly measurable: `POSITION_MAGIC_V1` appeared
on ONE line of Rust in the whole tree, its own declaration, inside a module the
crate marks `dead_code`; no type, no writer, no program header.  The browser's
explorer carried a decoder arm for a record nothing writes until `958901b45`
deleted it, and had to hold an exemption saying the emission could not be
narrowed from that tree.

`positionPdaDomain` is the half that is alive, and it is alive for a DIFFERENT
family: the Direct controller derives one per-outcome Position account from
`[domain, market, maker, outcome]`, which `directTransaction.ts` builds on every
Direct trade.  Two families shared the domain string and only one of them ever
had an account type here, which is why the dead half could be cut without
touching the live one — and why `domains_are_distinct` is still the theorem
that matters.
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

/-! ## The Position seed domain

The record is gone; the domain is not.  See the module header for which family
still derives against it.
-/

/-- Domain seed preceding the exact Market and owner keys in a Position PDA
derivation.  The Direct controller's per-outcome Position family is the live
consumer; the compact native `PositionV1` this domain was written for was
banished with the DCLTCAT1 stratum. -/
def positionPdaDomain : String := "dclutch/position/v1"

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

/-- Both seed domains fit one SVM seed component.  A domain that outgrew this
bound would not be a layout change; it would make every derived address
underivable, so the bound is proved rather than assumed. -/
theorem realm_domain_is_seedable :
    realmPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem position_domain_is_seedable :
    positionPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide

/-- The two domains are distinct, so no Realm address can be reached by a
Position derivation or the reverse.  This outlived the Position RECORD on
purpose: the derivation it separates the Realm from is the Direct controller's,
which is live. -/
theorem domains_are_distinct : realmPdaDomain ≠ positionPdaDomain := by
  native_decide

/-- Canonical finalized-record schema label, and its SHA-256 identity.

The label and the digest lived in the crate as hand-typed constants. Lean does
not hash, so the digest is DATA here and the byte-compare guard is what holds it
to its label -- the same arrangement `SourceMaterialV2Abi` uses, and the same
argument 52bbd463 made about a magic: the VALUE is not a theorem and should not
be one; what matters is that exactly one place states it. The crate keeps its
own hashing test, which is the independent check. -/
def schemaReleasePreimage : String := "dclutch/schema/realm-v1"
def schemaReleaseId : List UInt8 := [
  0x94, 0xfe, 0x1f, 0xd6, 0xd7, 0x25, 0x9f, 0x47,
  0x50, 0x3d, 0x6a, 0xc5, 0x7e, 0xc7, 0xda, 0x78,
  0xdc, 0x38, 0x06, 0xa5, 0xed, 0x49, 0x8f, 0xea,
  0xe4, 0x3e, 0xd3, 0x78, 0x5b, 0x5d, 0x0c, 0x69
]

theorem schema_release_id_is_thirty_two_bytes :
    schemaReleaseId.length = 32 := by native_decide

end DClutch.RealmPositionAbi
