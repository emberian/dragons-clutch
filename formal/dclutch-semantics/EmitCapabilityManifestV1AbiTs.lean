import DClutchSemantics.CapabilityManifestV1Abi
import DClutchSemantics.TsEmit

/-! Emit the Lean-owned capability-manifest and typed-funding byte ABI as a
TypeScript module.

The browser's shared `DCLTCAP1` decoder enforces the manifest grammar so it
refuses exactly what the chain refuses.  It can only do that against the same
layout the contract compiles against, which is this one. -/

open DClutch.AbiSchema
open DClutch.CapabilityManifestV1Abi
open DClutch.TsEmit

/-- Offset of one compartment inside the amounts block. -/
def compartmentOffset (field : AmountsField) : Nat :=
  match coordinate? field amountsLayout with
  | some (offset, _) => offset
  | none => 0

/-- The seven segregated compartments as one frozen table: canonical order,
offset, and whether the compartment's asset class is fixed by physics.  The
browser derives its compartment name and asset-policy types from this table
rather than restating them. -/
def compartmentTable : List String :=
  let row := fun field =>
    let policy := if nativeOnly field then "native-lamports-only" else "capability-selected"
    let body :=
      s!"name: '{AmountsField.label field}', offset: {compartmentOffset field}," ++
        s!" assetPolicy: '{policy}'"
    "  Object.freeze({ " ++ body ++ " }),"
  ["export const FUNDING_COMPARTMENTS_V1 = Object.freeze(["] ++
    compartments.map row ++
    ["] as const);"]

def main : IO Unit :=
  emit (header "EmitCapabilityManifestV1AbiTs.lean" "abi:capability-manifest") [
    [ nat "SVM_MAX_SEED_BYTES" svmMaxSeedBytes ],
    [ ascii "CAPABILITY_MANIFEST_MAGIC_V1" manifestMagic,
      nat "CAPABILITY_MANIFEST_SCHEMA_VERSION_V1" manifestSchemaVersion,
      nat "CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1" manifestArtifactProfile,
      nat "CAPABILITY_MANIFEST_HEADER_BYTES_V1" headerBytes,
      nat "CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1" headerReservedBytes,
      nat "CAPABILITY_MANIFEST_MAX_BYTES_V1" maxManifestBytes,
      nat "MAX_CAPABILITIES_V1" maxCapabilities,
      nat "MAX_DEPENDENCIES_PER_CAPABILITY_V1" maxDependenciesPerCapability ],
    offsets HeaderField.constantName headerLayout,
    [ nat "CAPABILITY_ENTRY_BYTES_V1" entryBytes,
      nat "CAPABILITY_ENTRY_RESERVED_BYTES_V1" entryReservedBytes ],
    offsets EntryField.constantName entryLayout,
    [ ascii "CAPABILITY_FUNDING_QUOTE_MAGIC_V1" fundingQuoteMagic,
      nat "CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1" fundingQuoteSchemaVersion,
      nat "CAPABILITY_FUNDING_QUOTE_BYTES_V1" quoteBytes,
      nat "CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1" quoteReservedBytes ],
    offsets QuoteField.constantName quoteLayout,
    [ nat "CAPABILITY_FUNDING_ALLOCATION_BYTES_V1" allocationBytes,
      nat "CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1" allocationReservedBytes ],
    offsets AllocationField.constantName allocationLayout,
    [ nat "CAPABILITY_FUNDING_AMOUNTS_BYTES_V1" amountsBytes ],
    offsets AmountsField.constantName amountsLayout,
    [ nat "CAPABILITY_FUNDING_BINDING_BYTES_V1" bindingBytes ],
    offsets BindingField.constantName bindingLayout,
    [ ascii "CAPABILITY_FUNDING_STATE_MAGIC_V1" fundingStateMagic,
      nat "CAPABILITY_FUNDING_STATE_SCHEMA_VERSION_V1" fundingStateSchemaVersion,
      nat "CAPABILITY_FUNDING_STATE_BYTES_V1" stateBytes,
      nat "CAPABILITY_FUNDING_STATE_HEADER_RESERVED_BYTES_V1" stateHeaderReservedBytes,
      nat "CAPABILITY_FUNDING_STATE_BODY_RESERVED_BYTES_V1" stateBodyReservedBytes,
      nat "CAPABILITY_FUNDING_STATE_REMAINING_RENT_AMOUNT_OFFSET_V1"
        stateRemainingRentAmountOffset ],
    offsets StateField.constantName stateLayout,
    [ domain "CAPABILITY_FUNDING_PDA_DOMAIN_V1" fundingPdaDomain,
      domain "CAPABILITY_FUNDING_AUTHORITY_PDA_DOMAIN_V1" fundingAuthorityPdaDomain,
      domain "CAPABILITY_FUNDING_VAULT_PDA_DOMAIN_V1" fundingVaultPdaDomain ],
    [ ascii "MARKET_OPENING_READINESS_MAGIC_V1" readinessMagic,
      nat "MARKET_OPENING_READINESS_SCHEMA_VERSION_V1" readinessSchemaVersion,
      nat "MARKET_OPENING_READINESS_BYTES_V1" readinessBytes,
      domain "MARKET_OPENING_READINESS_PDA_DOMAIN_V1" readinessPdaDomain ],
    compartmentTable
  ]
