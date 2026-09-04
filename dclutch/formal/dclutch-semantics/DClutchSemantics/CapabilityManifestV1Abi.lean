import DClutchSemantics.AbiSchema

/-!
# Capability manifest, typed funding, and opening-readiness ABI

The sole byte-layout owner for the immutable `DCLTCAP1` capability manifest a
Market root commits to by content identity, the immutable `DCLTFQ01` typed
funding quote each manifest entry carries, the separately mutable `DCLTCFS1`
funding state, and the transient `DCLTMOR1` opening-readiness record.

Two properties of this family are protocol meaning rather than encoding, and
both are stated here so neither backend can drift from them:

* The manifest is a **preimage**.  Its bytes are hashed to the content identity
  that is the sole Market capability authority, so every offset below is part
  of that identity.  A silent offset change would not be a compatible schema
  edit; it would name a different Market.
* Native lamports and Realm collateral are **two physical dimensions**.  The
  amounts block carries seven segregated compartments plus one checked total
  per dimension, and nothing here sums or converts across them.
-/

namespace DClutch.CapabilityManifestV1Abi

open DClutch.AbiSchema

/-- Chain-derived maximum byte width of one SVM PDA seed component. -/
def svmMaxSeedBytes : Nat := 32

/-- Provisional artifact-profile bound on capability entries per manifest. -/
def maxCapabilities : Nat := 16
/-- Provisional artifact-profile bound on dependencies per entry. -/
def maxDependenciesPerCapability : Nat := 16

/-! ## Typed compartment allocation

One asset-classed amount.  The class byte and the amount are decoded together
so a caller cannot combine the class of one compartment with the amount of
another.
-/

inductive AllocationField where
  | assetClass | reserved | amount
  deriving DecidableEq, Repr

def allocationSchema : List (FieldSpec AllocationField) := [
  ⟨.assetClass, .u8⟩,
  ⟨.reserved, .reserved 7⟩,
  ⟨.amount, .u64⟩
]

def allocationLayout : List (PlacedField AllocationField) := specialize allocationSchema
def allocationBytes : Nat := schemaWidth allocationSchema
/-- Width of the canonically-zero span inside one allocation. -/
def allocationReservedBytes : Nat := 7

namespace AllocationField

def constantName : AllocationField → String
  | .assetClass => "CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1"
  | .reserved => "CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1"
  | .amount => "CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1"

end AllocationField

/-! ## The seven segregated compartments

`Rent` and `Creation` are intrinsically native lamports.  The remaining five
carry whichever asset class the immutable capability quote selected.  The order
below is canonical: it is the order the compartments occupy in the hashed
manifest preimage.
-/

inductive AmountsField where
  | rent | creation | work | provider | bounty | liquidity | service
  | nativeLamportsTotal | realmCollateralTotal
  deriving DecidableEq, Repr

def amountsSchema : List (FieldSpec AmountsField) := [
  ⟨.rent, .nested (schemaWidth allocationSchema)⟩,
  ⟨.creation, .nested (schemaWidth allocationSchema)⟩,
  ⟨.work, .nested (schemaWidth allocationSchema)⟩,
  ⟨.provider, .nested (schemaWidth allocationSchema)⟩,
  ⟨.bounty, .nested (schemaWidth allocationSchema)⟩,
  ⟨.liquidity, .nested (schemaWidth allocationSchema)⟩,
  ⟨.service, .nested (schemaWidth allocationSchema)⟩,
  ⟨.nativeLamportsTotal, .u64⟩,
  ⟨.realmCollateralTotal, .u64⟩
]

def amountsLayout : List (PlacedField AmountsField) := specialize amountsSchema
def amountsBytes : Nat := schemaWidth amountsSchema

/-- The seven compartments in canonical order, excluding the two totals. -/
def compartments : List AmountsField :=
  [.rent, .creation, .work, .provider, .bounty, .liquidity, .service]

/-- Whether a compartment's asset class is fixed by physics or selected by the
capability.  This is the browser's `assetPolicy` and the crate's
`FundingAssetPolicy`. -/
def nativeOnly : AmountsField → Bool
  | .rent | .creation => true
  | _ => false

namespace AmountsField

def constantName : AmountsField → String
  | .rent => "CAPABILITY_FUNDING_AMOUNTS_RENT_OFFSET_V1"
  | .creation => "CAPABILITY_FUNDING_AMOUNTS_CREATION_OFFSET_V1"
  | .work => "CAPABILITY_FUNDING_AMOUNTS_WORK_OFFSET_V1"
  | .provider => "CAPABILITY_FUNDING_AMOUNTS_PROVIDER_OFFSET_V1"
  | .bounty => "CAPABILITY_FUNDING_AMOUNTS_BOUNTY_OFFSET_V1"
  | .liquidity => "CAPABILITY_FUNDING_AMOUNTS_LIQUIDITY_OFFSET_V1"
  | .service => "CAPABILITY_FUNDING_AMOUNTS_SERVICE_OFFSET_V1"
  | .nativeLamportsTotal => "CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1"
  | .realmCollateralTotal => "CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1"

/-- The compartment name the browser labels a quote with. -/
def label : AmountsField → String
  | .rent => "Rent"
  | .creation => "Creation"
  | .work => "Work"
  | .provider => "Provider"
  | .bounty => "Bounty"
  | .liquidity => "Liquidity"
  | .service => "Service"
  | .nativeLamportsTotal => "NativeLamportsTotal"
  | .realmCollateralTotal => "RealmCollateralTotal"

end AmountsField

/-! ## Realm-collateral binding

Present exactly when the quote selects Realm collateral; canonically zero
otherwise.
-/

inductive BindingField where
  | realmId | collateralReleaseId | tokenProgram | mint | refundBeneficiary
  deriving DecidableEq, Repr

def bindingSchema : List (FieldSpec BindingField) := [
  ⟨.realmId, .bytes 32⟩,
  ⟨.collateralReleaseId, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩,
  ⟨.mint, .bytes 32⟩,
  ⟨.refundBeneficiary, .bytes 32⟩
]

def bindingLayout : List (PlacedField BindingField) := specialize bindingSchema
def bindingBytes : Nat := schemaWidth bindingSchema

namespace BindingField

def constantName : BindingField → String
  | .realmId => "CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1"
  | .collateralReleaseId => "CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1"
  | .tokenProgram => "CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1"
  | .mint => "CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1"
  | .refundBeneficiary => "CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1"

/-- The identity name the browser reports a zero binding coordinate under. -/
def label : BindingField → String
  | .realmId => "Realm identity"
  | .collateralReleaseId => "collateral release identity"
  | .tokenProgram => "token program"
  | .mint => "collateral mint"
  | .refundBeneficiary => "refund token beneficiary"

end BindingField

/-! ## Immutable typed funding quote -/

def fundingQuoteMagic : String := "DCLTFQ01"
def fundingQuoteSchemaVersion : Nat := 1

inductive QuoteField where
  | magic | schemaVersion | collateralKind | reserved | collateralBinding | amounts
  deriving DecidableEq, Repr

def quoteSchema : List (FieldSpec QuoteField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.collateralKind, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.collateralBinding, .nested (schemaWidth bindingSchema)⟩,
  ⟨.amounts, .nested (schemaWidth amountsSchema)⟩
]

def quoteLayout : List (PlacedField QuoteField) := specialize quoteSchema
def quoteBytes : Nat := schemaWidth quoteSchema
/-- Width of the canonically-zero span in the quote header. -/
def quoteReservedBytes : Nat := 5

namespace QuoteField

def constantName : QuoteField → String
  | .magic => "CAPABILITY_FUNDING_QUOTE_MAGIC_OFFSET_V1"
  | .schemaVersion => "CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1"
  | .collateralKind => "CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1"
  | .reserved => "CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1"
  | .collateralBinding => "CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1"
  | .amounts => "CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1"

end QuoteField

/-! ## Separately mutable typed funding state -/

def fundingStateMagic : String := "DCLTCFS1"
def fundingStateSchemaVersion : Nat := 1

/-- Adapter PDA seed domain for a manifest-selected funding-state account. -/
def fundingPdaDomain : String := "dclutch/cap-funding/v1"
/-- Adapter PDA seed domain for its token-signing funding authority. -/
def fundingAuthorityPdaDomain : String := "dclutch/cap-fund-auth/v1"
/-- Adapter PDA seed domain for its optional Realm-collateral vault. -/
def fundingVaultPdaDomain : String := "dclutch/cap-fund-vault/v1"

inductive StateField where
  | magic | schemaVersion | status | headerReserved | manifestId | entryIndex
  | bodyReserved | activationSlot | remaining | released
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.status, .u8⟩,
  ⟨.headerReserved, .reserved 5⟩,
  ⟨.manifestId, .bytes 32⟩,
  ⟨.entryIndex, .u16⟩,
  ⟨.bodyReserved, .reserved 6⟩,
  ⟨.activationSlot, .u64⟩,
  ⟨.remaining, .nested (schemaWidth amountsSchema)⟩,
  ⟨.released, .nested (schemaWidth amountsSchema)⟩
]

def stateLayout : List (PlacedField StateField) := specialize stateSchema
def stateBytes : Nat := schemaWidth stateSchema
def stateHeaderReservedBytes : Nat := 5
def stateBodyReservedBytes : Nat := 6

namespace StateField

def constantName : StateField → String
  | .magic => "CAPABILITY_FUNDING_STATE_MAGIC_OFFSET_V1"
  | .schemaVersion => "CAPABILITY_FUNDING_STATE_SCHEMA_OFFSET_V1"
  | .status => "CAPABILITY_FUNDING_STATE_STATUS_OFFSET_V1"
  | .headerReserved => "CAPABILITY_FUNDING_STATE_HEADER_RESERVED_OFFSET_V1"
  | .manifestId => "CAPABILITY_FUNDING_STATE_MANIFEST_ID_OFFSET_V1"
  | .entryIndex => "CAPABILITY_FUNDING_STATE_ENTRY_INDEX_OFFSET_V1"
  | .bodyReserved => "CAPABILITY_FUNDING_STATE_BODY_RESERVED_OFFSET_V1"
  | .activationSlot => "CAPABILITY_FUNDING_STATE_ACTIVATION_SLOT_OFFSET_V1"
  | .remaining => "CAPABILITY_FUNDING_STATE_REMAINING_OFFSET_V1"
  | .released => "CAPABILITY_FUNDING_STATE_RELEASED_OFFSET_V1"

end StateField

/-- Byte offset of the remaining Rent-compartment lamport amount inside a live
funding-state account.

Published for one caller shape: a data-defined capability activation, whose
`AccountProfileV1` must project this exact scalar with a `ProjectDataU64`
operation so its EffectProgram can move that many lamports into the root it is
creating.  An interpreted artifact carries no decoder, so without this it would
restate the layout and become a second authority for it. -/
def stateRemainingRentAmountOffset : Nat :=
  (match coordinate? StateField.remaining stateLayout with
   | some (offset, _) => offset
   | none => 0) +
  (match coordinate? AmountsField.rent amountsLayout with
   | some (offset, _) => offset
   | none => 0) +
  (match coordinate? AllocationField.amount allocationLayout with
   | some (offset, _) => offset
   | none => 0)

/-! ## Manifest-keyed ordered funding ledger

`FundingLedgerV2` replaces one mutable account per selected manifest entry with
one controller-homogeneous subset account. The account has an exact header
followed by one congruent slot for each set bit of `selectedMask`, in ascending
manifest-index order. A slot stores only physical
remaining principal: asset classes, per-asset totals, and released principal
are derived from the authenticated immutable quote and therefore cannot become
parallel persisted truths.
-/

def fundingLedgerMagicV2 : String := "DCLTFL02"
def fundingLedgerSchemaVersionV2 : Nat := 2

/-- Adapter PDA seed domain for the one manifest-keyed funding ledger. -/
def fundingLedgerPdaDomainV2 : String := "dclutch/cap-funding-ledger/v2"
/-- Per-entry token-signing authority under one funding ledger. -/
def fundingLedgerAuthorityPdaDomainV2 : String := "dclutch/cap-ledger-auth/v2"
/-- Optional per-entry Realm-collateral vault. -/
def fundingLedgerVaultPdaDomainV2 : String := "dclutch/cap-ledger-vault/v2"

inductive LedgerHeaderFieldV2 where
  | magic | schemaVersion | selectedMask | reserved | manifestId
  deriving DecidableEq, Repr

def ledgerHeaderSchemaV2 : List (FieldSpec LedgerHeaderFieldV2) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.selectedMask, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.manifestId, .bytes 32⟩
]

def ledgerHeaderLayoutV2 : List (PlacedField LedgerHeaderFieldV2) :=
  specialize ledgerHeaderSchemaV2
def ledgerHeaderBytesV2 : Nat := schemaWidth ledgerHeaderSchemaV2
def ledgerHeaderReservedBytesV2 : Nat := 4

namespace LedgerHeaderFieldV2

def constantName : LedgerHeaderFieldV2 → String
  | .magic => "CAPABILITY_FUNDING_LEDGER_MAGIC_OFFSET_V2"
  | .schemaVersion => "CAPABILITY_FUNDING_LEDGER_SCHEMA_OFFSET_V2"
  | .selectedMask => "CAPABILITY_FUNDING_LEDGER_SELECTED_MASK_OFFSET_V2"
  | .reserved => "CAPABILITY_FUNDING_LEDGER_RESERVED_OFFSET_V2"
  | .manifestId => "CAPABILITY_FUNDING_LEDGER_MANIFEST_ID_OFFSET_V2"

end LedgerHeaderFieldV2

inductive LedgerSlotFieldV2 where
  | status | reserved | activationSlot
  | rent | creation | work | provider | bounty | liquidity | service
  deriving DecidableEq, Repr

def ledgerSlotSchemaV2 : List (FieldSpec LedgerSlotFieldV2) := [
  ⟨.status, .u8⟩,
  ⟨.reserved, .reserved 7⟩,
  ⟨.activationSlot, .u64⟩,
  ⟨.rent, .u64⟩,
  ⟨.creation, .u64⟩,
  ⟨.work, .u64⟩,
  ⟨.provider, .u64⟩,
  ⟨.bounty, .u64⟩,
  ⟨.liquidity, .u64⟩,
  ⟨.service, .u64⟩
]

def ledgerSlotLayoutV2 : List (PlacedField LedgerSlotFieldV2) :=
  specialize ledgerSlotSchemaV2
def ledgerSlotBytesV2 : Nat := schemaWidth ledgerSlotSchemaV2
def ledgerSlotReservedBytesV2 : Nat := 7

namespace LedgerSlotFieldV2

def constantName : LedgerSlotFieldV2 → String
  | .status => "CAPABILITY_FUNDING_LEDGER_SLOT_STATUS_OFFSET_V2"
  | .reserved => "CAPABILITY_FUNDING_LEDGER_SLOT_RESERVED_OFFSET_V2"
  | .activationSlot => "CAPABILITY_FUNDING_LEDGER_SLOT_ACTIVATION_SLOT_OFFSET_V2"
  | .rent => "CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_RENT_OFFSET_V2"
  | .creation => "CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_CREATION_OFFSET_V2"
  | .work => "CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_WORK_OFFSET_V2"
  | .provider => "CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_PROVIDER_OFFSET_V2"
  | .bounty => "CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_BOUNTY_OFFSET_V2"
  | .liquidity => "CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_LIQUIDITY_OFFSET_V2"
  | .service => "CAPABILITY_FUNDING_LEDGER_SLOT_REMAINING_SERVICE_OFFSET_V2"

end LedgerSlotFieldV2

/-- The lifecycle one manifest-indexed ledger slot walks.

The first per-ROW machine of the eight the route census gates on: one ledger
account holds a slot for every selected manifest entry, and each of them walks
these three independently, so there is no single answer per Market and no
single answer per account.

The three tags were `funding.rs`'s alone -- an `#[repr(u8)]` enum with explicit
literal discriminants, which is the strongest form Rust offers and is still one
author.  This module already owned the slot's LAYOUT (`ledgerSlotSchemaV2`,
whose `.status` field is the byte they are written at); the tags written at that
byte belonged somewhere else entirely. -/
inductive LedgerSlotStatusV2 where
  | pending | active | closed
  deriving DecidableEq, Repr

namespace LedgerSlotStatusV2

def all : List LedgerSlotStatusV2 := [.pending, .active, .closed]

/-- The wire tag persisted in the slot's status byte. -/
def tag : LedgerSlotStatusV2 → Nat
  | .pending => 0
  | .active => 1
  | .closed => 2

def constantName : LedgerSlotStatusV2 → String
  | .pending => "CAPABILITY_FUNDING_LEDGER_STATUS_PENDING_V2"
  | .active => "CAPABILITY_FUNDING_LEDGER_STATUS_ACTIVE_V2"
  | .closed => "CAPABILITY_FUNDING_LEDGER_STATUS_CLOSED_V2"

def doc : LedgerSlotStatusV2 → String
  | .pending => "The exact quote remains prepaid and activation has not run."
  | .active => "Activation ran once; Rent and Creation have been released."
  | .closed => "This logical entry has closed and retains no principal."

/-- Whether the slot may still hold principal.  `all_closed` folds the
complement of this across every row to decide whether the shared ACCOUNT may be
freed, which is a fact about the ledger rather than about one act on one slot. -/
def holdsPrincipal : LedgerSlotStatusV2 → Bool
  | .pending | .active => true
  | .closed => false

/-- The status an unwritten slot's zero byte decodes as. -/
def zeroTag : LedgerSlotStatusV2 := .pending

end LedgerSlotStatusV2

/-- One past the greatest slot-status tag: the `STATE_COUNT` that
`funding_admission_v2.rs` asserts its `u8` bitset against. -/
def ledgerSlotStatusLimitV2 : Nat := 3

/-- The one exact account width for a canonically bounded manifest count. -/
def fundingLedgerBytesV2 (slotCount : Nat) : Nat :=
  ledgerHeaderBytesV2 + slotCount * ledgerSlotBytesV2

/-! ## One capability entry -/

inductive EntryField where
  | kindId | releaseId | configId | capacityProfileId | childSchemaId
  | childDerivationId | activationPolicy | dependencyCount | reserved
  | activationDeadline | dependencies | quote
  deriving DecidableEq, Repr

def entrySchema : List (FieldSpec EntryField) := [
  ⟨.kindId, .bytes 32⟩,
  ⟨.releaseId, .bytes 32⟩,
  ⟨.configId, .bytes 32⟩,
  ⟨.capacityProfileId, .bytes 32⟩,
  ⟨.childSchemaId, .bytes 32⟩,
  ⟨.childDerivationId, .bytes 32⟩,
  ⟨.activationPolicy, .u8⟩,
  ⟨.dependencyCount, .u8⟩,
  ⟨.reserved, .reserved 6⟩,
  ⟨.activationDeadline, .u64⟩,
  ⟨.dependencies, .bytes maxDependenciesPerCapability⟩,
  ⟨.quote, .nested (schemaWidth quoteSchema)⟩
]

def entryLayout : List (PlacedField EntryField) := specialize entrySchema
def entryBytes : Nat := schemaWidth entrySchema
/-- Width of the canonically-zero span inside one entry. -/
def entryReservedBytes : Nat := 6

namespace EntryField

def constantName : EntryField → String
  | .kindId => "CAPABILITY_ENTRY_KIND_ID_OFFSET_V1"
  | .releaseId => "CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1"
  | .configId => "CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1"
  | .capacityProfileId => "CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1"
  | .childSchemaId => "CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1"
  | .childDerivationId => "CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1"
  | .activationPolicy => "CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1"
  | .dependencyCount => "CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1"
  | .reserved => "CAPABILITY_ENTRY_RESERVED_OFFSET_V1"
  | .activationDeadline => "CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1"
  | .dependencies => "CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1"
  | .quote => "CAPABILITY_ENTRY_QUOTE_OFFSET_V1"

/-- The content-addressed identity name the browser reports a zero entry
coordinate under, in manifest order. -/
def identityLabel : EntryField → Option String
  | .kindId => some "kind"
  | .releaseId => some "programSet"
  | .configId => some "config"
  | .capacityProfileId => some "capacity"
  | .childSchemaId => some "rootSchema"
  | .childDerivationId => some "derivation"
  | _ => none

end EntryField

/-! ## Manifest header -/

def manifestMagic : String := "DCLTCAP1"
def manifestSchemaVersion : Nat := 1
def manifestArtifactProfile : Nat := 1

inductive HeaderField where
  | magic | schemaVersion | artifactProfile | entryCount | reserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.entryCount, .u16⟩,
  ⟨.reserved, .reserved 2⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema
def headerBytes : Nat := schemaWidth headerSchema
/-- Width of the canonically-zero span in the manifest header. -/
def headerReservedBytes : Nat := 2

/-- Exact width of a manifest carrying a given number of entries. -/
def manifestBytes (entries : Nat) : Nat := headerBytes + entries * entryBytes

/-- Maximum profile-1 manifest byte width. -/
def maxManifestBytes : Nat := manifestBytes maxCapabilities

namespace HeaderField

def constantName : HeaderField → String
  | .magic => "CAPABILITY_MANIFEST_MAGIC_OFFSET_V1"
  | .schemaVersion => "CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1"
  | .artifactProfile => "CAPABILITY_MANIFEST_PROFILE_OFFSET_V1"
  | .entryCount => "CAPABILITY_MANIFEST_COUNT_OFFSET_V1"
  | .reserved => "CAPABILITY_MANIFEST_RESERVED_OFFSET_V1"

end HeaderField

/-! ## Transient Market-opening readiness -/

def readinessMagic : String := "DCLTMOR1"
def readinessSchemaVersion : Nat := 1
/-- Adapter PDA seed domain for one transient Market-opening readiness child. -/
def readinessPdaDomain : String := "dclutch/open-readiness/v1"

inductive ReadinessField where
  | magic | schemaVersion | headerReserved | market | generation | manifestId
  | entryCount | nextEntry | bodyReserved | rentRefund
  deriving DecidableEq, Repr

def readinessSchema : List (FieldSpec ReadinessField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.headerReserved, .reserved 6⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.manifestId, .bytes 32⟩,
  ⟨.entryCount, .u16⟩,
  ⟨.nextEntry, .u16⟩,
  ⟨.bodyReserved, .reserved 4⟩,
  ⟨.rentRefund, .bytes 32⟩
]

def readinessLayout : List (PlacedField ReadinessField) := specialize readinessSchema
def readinessBytes : Nat := schemaWidth readinessSchema
def readinessHeaderReservedBytes : Nat := 6
def readinessBodyReservedBytes : Nat := 4

namespace ReadinessField

def constantName : ReadinessField → String
  | .magic => "READINESS_MAGIC_OFFSET_V1"
  | .schemaVersion => "READINESS_SCHEMA_OFFSET_V1"
  | .headerReserved => "READINESS_RESERVED_OFFSET_V1"
  | .market => "READINESS_MARKET_OFFSET_V1"
  | .generation => "READINESS_GENERATION_OFFSET_V1"
  | .manifestId => "READINESS_MANIFEST_OFFSET_V1"
  | .entryCount => "READINESS_ENTRY_COUNT_OFFSET_V1"
  | .nextEntry => "READINESS_NEXT_ENTRY_OFFSET_V1"
  | .bodyReserved => "READINESS_BODY_RESERVED_OFFSET_V1"
  | .rentRefund => "READINESS_RENT_REFUND_OFFSET_V1"

end ReadinessField

/-! ## Exact-value pins

Every width below is already committed to by deployed content identities and by
the browser's shared decoder.  Stating them as theorems makes a schema edit that
would move a live boundary fail in Lean, before either backend regenerates.
-/

theorem allocation_width_is_exact : allocationBytes = 16 := by native_decide
theorem amounts_width_is_exact : amountsBytes = 128 := by native_decide
theorem binding_width_is_exact : bindingBytes = 160 := by native_decide
theorem quote_width_is_exact : quoteBytes = 304 := by native_decide
theorem state_width_is_exact : stateBytes = 320 := by native_decide
theorem entry_width_is_exact : entryBytes = 528 := by native_decide
theorem header_width_is_exact : headerBytes = 16 := by native_decide
theorem readiness_width_is_exact : readinessBytes = 128 := by native_decide
theorem max_manifest_width_is_exact : maxManifestBytes = 8464 := by native_decide
theorem published_rent_amount_offset_is_exact :
    stateRemainingRentAmountOffset = 72 := by native_decide

theorem allocation_names_are_unique :
    (allocationSchema.map fun field => field.name).Nodup := by native_decide
theorem amounts_names_are_unique :
    (amountsSchema.map fun field => field.name).Nodup := by native_decide
theorem binding_names_are_unique :
    (bindingSchema.map fun field => field.name).Nodup := by native_decide
theorem quote_names_are_unique :
    (quoteSchema.map fun field => field.name).Nodup := by native_decide
theorem state_names_are_unique :
    (stateSchema.map fun field => field.name).Nodup := by native_decide
theorem entry_names_are_unique :
    (entrySchema.map fun field => field.name).Nodup := by native_decide
theorem header_names_are_unique :
    (headerSchema.map fun field => field.name).Nodup := by native_decide
theorem readiness_names_are_unique :
    (readinessSchema.map fun field => field.name).Nodup := by native_decide

theorem allocation_fields_are_disjoint :
    allocationLayout.Pairwise Before := specializeFrom_pairwise 0 allocationSchema
theorem amounts_fields_are_disjoint :
    amountsLayout.Pairwise Before := specializeFrom_pairwise 0 amountsSchema
theorem binding_fields_are_disjoint :
    bindingLayout.Pairwise Before := specializeFrom_pairwise 0 bindingSchema
theorem quote_fields_are_disjoint :
    quoteLayout.Pairwise Before := specializeFrom_pairwise 0 quoteSchema
theorem state_fields_are_disjoint :
    stateLayout.Pairwise Before := specializeFrom_pairwise 0 stateSchema
theorem ledger_header_fields_are_disjoint :
    ledgerHeaderLayoutV2.Pairwise Before := specializeFrom_pairwise 0 ledgerHeaderSchemaV2
theorem ledger_slot_fields_are_disjoint :
    ledgerSlotLayoutV2.Pairwise Before := specializeFrom_pairwise 0 ledgerSlotSchemaV2
theorem entry_fields_are_disjoint :
    entryLayout.Pairwise Before := specializeFrom_pairwise 0 entrySchema
theorem header_fields_are_disjoint :
    headerLayout.Pairwise Before := specializeFrom_pairwise 0 headerSchema
theorem readiness_fields_are_disjoint :
    readinessLayout.Pairwise Before := specializeFrom_pairwise 0 readinessSchema

/-- The dependency array is exactly wide enough to name every entry the
artifact profile admits, so a dependency list can never be truncated by its own
container. -/
theorem dependency_array_covers_the_entry_bound :
    coordinate? EntryField.dependencies entryLayout =
      some (208, maxCapabilities) := by native_decide

/-- Seven compartments plus two checked totals exactly fill the amounts block:
there is no unaccounted span in which a third dimension could hide. -/
theorem amounts_block_is_exactly_seven_compartments_and_two_totals :
    compartments.length * allocationBytes + 8 + 8 = amountsBytes := by
  native_decide

/-- Remaining and released amounts are the same block shape, so a released
total can be compared against a remaining total coordinate for coordinate. -/
theorem state_carries_two_congruent_amount_blocks :
    coordinate? StateField.remaining stateLayout = some (64, amountsBytes) ∧
      coordinate? StateField.released stateLayout = some (192, amountsBytes) := by
  native_decide

/-- Every ledger entry has one and only one fixed-width physical slot. -/
theorem ledger_slot_is_exactly_status_slot_and_seven_remaining_amounts :
    ledgerSlotBytesV2 = 72 := by native_decide

/-- **The three tags `funding.rs` authored alone.**  They are distinct, they
run from zero, and every one of them indexes its own bit of the `u8` bitset
`funding_admission_v2.rs` writes as `STATE_COUNT <= 8` plus one `assert!` on the
last variant; here it is the whole enumeration. -/
theorem the_slot_statuses_are_distinct_bit_indices :
    (LedgerSlotStatusV2.all.map LedgerSlotStatusV2.tag) = [0, 1, 2] ∧
      (LedgerSlotStatusV2.all.map LedgerSlotStatusV2.tag).Nodup ∧
      LedgerSlotStatusV2.all.all
        (fun status => LedgerSlotStatusV2.tag status < ledgerSlotStatusLimitV2) = true ∧
      ledgerSlotStatusLimitV2 ≤ 8 := by
  native_decide

/-- **Pending is the zero tag, so a zeroed slot is a prepaid one.**  A ledger
account is created at its exact width and every slot in it reads `Pending`
before anything is written, which is precisely the intended initial state --
the one place in this record where an unwritten byte and a meaningful state
coincide on purpose.  The partition against an unwritten ACCOUNT is
`CAPABILITY_FUNDING_LEDGER_MAGIC_V2` in the header, never a slot's status
byte. -/
theorem the_zero_tag_is_pending_so_a_fresh_slot_is_prepaid :
    LedgerSlotStatusV2.tag LedgerSlotStatusV2.zeroTag = 0 ∧
      LedgerSlotStatusV2.zeroTag = .pending ∧
      LedgerSlotStatusV2.all.filter LedgerSlotStatusV2.holdsPrincipal =
        [.pending, .active] := by
  native_decide

/-- The status byte is the slot's first, so a slot's address and its tag's
address differ by nothing: the reader's arithmetic is
`headerBytes + rowBytes * row`, with no third term. -/
theorem the_status_heads_the_slot :
    coordinate? LedgerSlotFieldV2.status ledgerSlotLayoutV2 = some (0, 1) := by
  native_decide

/-- A four-entry subset uses the exact dynamic width rather than reserving the
sixteen-entry maximum. -/
theorem four_slot_funding_ledger_is_336_bytes :
    fundingLedgerBytesV2 4 = 336 := by native_decide

/-- Decision 0013's Direct market uses one three-entry Resolution subset and
one one-entry Direct subset. -/
theorem resolution_and_direct_subset_data_is_384_bytes :
    fundingLedgerBytesV2 3 + fundingLedgerBytesV2 1 = 384 := by native_decide

/-- Every funding seed domain fits one SVM seed component. -/
theorem funding_domain_is_seedable :
    fundingPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem funding_authority_domain_is_seedable :
    fundingAuthorityPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem funding_vault_domain_is_seedable :
    fundingVaultPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem funding_ledger_domain_is_seedable :
    fundingLedgerPdaDomainV2.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem funding_ledger_authority_domain_is_seedable :
    fundingLedgerAuthorityPdaDomainV2.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem funding_ledger_vault_domain_is_seedable :
    fundingLedgerVaultPdaDomainV2.toUTF8.size ≤ svmMaxSeedBytes := by native_decide
theorem readiness_domain_is_seedable :
    readinessPdaDomain.toUTF8.size ≤ svmMaxSeedBytes := by native_decide

/-- The four record magics in this family are pairwise distinct, so magic
dispatch cannot route one record to another's decoder. -/
theorem magics_are_pairwise_distinct :
    [manifestMagic, fundingQuoteMagic, fundingStateMagic, fundingLedgerMagicV2,
      readinessMagic].Nodup := by
  native_decide

/-- The four seed domains are pairwise distinct, so no account addressed under
one can be reached by a derivation under another. -/
theorem domains_are_pairwise_distinct :
    [fundingPdaDomain, fundingAuthorityPdaDomain, fundingVaultPdaDomain,
      fundingLedgerPdaDomainV2, fundingLedgerAuthorityPdaDomainV2,
      fundingLedgerVaultPdaDomainV2, readinessPdaDomain].Nodup := by
  native_decide


/-- Canonical finalized-record schema label, and its SHA-256 identity.

The label and the digest lived in the crate as hand-typed constants. Lean does
not hash, so the digest is DATA here and the byte-compare guard is what holds it
to its label -- the same arrangement `SourceMaterialV2Abi` uses, and the same
argument 52bbd463 made about a magic: the VALUE is not a theorem and should not
be one; what matters is that exactly one place states it. The crate keeps its
own hashing test, which is the independent check. -/
def schemaReleasePreimage : String := "dclutch/schema/capability-manifest-profile-1-v1"
def schemaReleaseId : List UInt8 := [
  0x6b, 0xce, 0xf7, 0xb2, 0x83, 0x67, 0xcb, 0x8d,
  0x08, 0x97, 0x10, 0xba, 0x58, 0xe6, 0x84, 0x31,
  0x2f, 0x43, 0x4c, 0x4b, 0xc4, 0x20, 0xee, 0xfd,
  0x0f, 0x7a, 0x15, 0x0a, 0x90, 0x82, 0x88, 0xdf
]

theorem schema_release_id_is_thirty_two_bytes :
    schemaReleaseId.length = 32 := by native_decide

end DClutch.CapabilityManifestV1Abi
