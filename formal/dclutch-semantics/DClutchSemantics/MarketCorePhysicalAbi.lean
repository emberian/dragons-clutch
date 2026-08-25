import DClutchSemantics.AbiSchema
import DClutchSemantics.MarketCoreAbi

/-!
# Market Core cross-program physical ABI

Core effect envelopes authenticate routing and replay coordinates without
duplicating Claims, Custody, or Resolution request semantics. The role-owned
request follows the fixed envelope and is bound by exact length and SHA-256
digest. The normalized acknowledgement binds the digest of that complete
effect, both resource revisions, and the post-resource digest.
-/

namespace DClutch.MarketCorePhysicalAbi

open DClutch.AbiSchema

def effectMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x45, 0x46, 0x31]
def ackMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x41, 0x4b, 0x31]
def seriesMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x53, 0x52, 0x31]
def seriesAckMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x53, 0x41, 0x31]
def capabilityFundingMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x46, 0x4c, 0x31]
def effectDigestDomain : List UInt8 := "dclutch/core-effect/v1".toUTF8.toList
def seriesCallerAuthorityDomain : List UInt8 := "dclutch/series-core-caller/v1".toUTF8.toList
def marketStateDomain : List UInt8 := "dclutch/market-core/state/v1".toUTF8.toList
def version : Nat := 1

theorem effect_digest_domain_fits_sha_seed : effectDigestDomain.length ≤ 32 := by native_decide
theorem series_caller_authority_domain_fits_pda_seed :
    seriesCallerAuthorityDomain.length ≤ 32 := by native_decide
theorem market_state_domain_fits_pda_seed : marketStateDomain.length ≤ 32 := by native_decide

/-! The profile-1 funding list uses the same semantic maximum as the canonical
capability-manifest profile. Lifting the bound requires a new manifest and
physical ABI profile; decoders never truncate an oversized list. -/
def capabilityFundingMaxEntries : Nat := 16

inductive CapabilityFundingHeaderField where
  | magic | version | count | reservedHeader | selectedEntryIndex | reservedBody
  deriving DecidableEq, Repr

def capabilityFundingHeaderSchema : List (FieldSpec CapabilityFundingHeaderField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.count, .u8⟩,
  ⟨.reservedHeader, .reserved 1⟩, ⟨.selectedEntryIndex, .u16⟩,
  ⟨.reservedBody, .reserved 2⟩
]

def capabilityFundingHeaderLayout : List (PlacedField CapabilityFundingHeaderField) :=
  specialize capabilityFundingHeaderSchema
def capabilityFundingHeaderBytes : Nat := schemaWidth capabilityFundingHeaderSchema

namespace CapabilityFundingHeaderField

def rustName : CapabilityFundingHeaderField → String
  | .magic => "CAPABILITY_FUNDING_MAGIC_OFFSET"
  | .version => "CAPABILITY_FUNDING_VERSION_OFFSET"
  | .count => "CAPABILITY_FUNDING_COUNT_OFFSET"
  | .reservedHeader => "CAPABILITY_FUNDING_RESERVED_HEADER_OFFSET"
  | .selectedEntryIndex => "CAPABILITY_FUNDING_SELECTED_ENTRY_INDEX_OFFSET"
  | .reservedBody => "CAPABILITY_FUNDING_RESERVED_BODY_OFFSET"

end CapabilityFundingHeaderField

inductive CapabilityFundingDescriptorField where
  | entryIndex | reserved | fundingAccount
  deriving DecidableEq, Repr

def capabilityFundingDescriptorSchema : List (FieldSpec CapabilityFundingDescriptorField) := [
  ⟨.entryIndex, .u16⟩, ⟨.reserved, .reserved 2⟩, ⟨.fundingAccount, .bytes 32⟩
]

def capabilityFundingDescriptorLayout : List (PlacedField CapabilityFundingDescriptorField) :=
  specialize capabilityFundingDescriptorSchema
def capabilityFundingDescriptorBytes : Nat := schemaWidth capabilityFundingDescriptorSchema

namespace CapabilityFundingDescriptorField

def rustName : CapabilityFundingDescriptorField → String
  | .entryIndex => "CAPABILITY_FUNDING_DESCRIPTOR_ENTRY_INDEX_OFFSET"
  | .reserved => "CAPABILITY_FUNDING_DESCRIPTOR_RESERVED_OFFSET"
  | .fundingAccount => "CAPABILITY_FUNDING_DESCRIPTOR_ACCOUNT_OFFSET"

end CapabilityFundingDescriptorField

def capabilityFundingBytes (count : Nat) : Nat :=
  capabilityFundingHeaderBytes + count * capabilityFundingDescriptorBytes

theorem capability_funding_header_width : capabilityFundingHeaderBytes = 16 := by native_decide
theorem capability_funding_descriptor_width : capabilityFundingDescriptorBytes = 36 := by native_decide
theorem capability_funding_profile_max_width :
    capabilityFundingBytes capabilityFundingMaxEntries = 592 := by native_decide
theorem capability_funding_header_unique :
    (capabilityFundingHeaderSchema.map fun field => field.name).Nodup := by native_decide
theorem capability_funding_header_disjoint :
    capabilityFundingHeaderLayout.Pairwise Before := specializeFrom_pairwise 0 capabilityFundingHeaderSchema
theorem capability_funding_descriptor_unique :
    (capabilityFundingDescriptorSchema.map fun field => field.name).Nodup := by native_decide
theorem capability_funding_descriptor_disjoint :
    capabilityFundingDescriptorLayout.Pairwise Before :=
  specializeFrom_pairwise 0 capabilityFundingDescriptorSchema

inductive EffectField where
  | magic | version | action | targetRole | reservedHeader
  | callerProgram | callerAuthority | releaseSet | market | context
  | parentStateDigest | roleRequestDigest
  | generation | expectedResourceARevision | expectedResourceBRevision
  | roleRequestBytes | reservedBody
  deriving DecidableEq, Repr

def effectSchema : List (FieldSpec EffectField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.targetRole, .u8⟩,
  ⟨.reservedHeader, .reserved 4⟩,
  ⟨.callerProgram, .bytes 32⟩, ⟨.callerAuthority, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.context, .bytes 32⟩,
  ⟨.parentStateDigest, .bytes 32⟩, ⟨.roleRequestDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.expectedResourceARevision, .u64⟩,
  ⟨.expectedResourceBRevision, .u64⟩, ⟨.roleRequestBytes, .u32⟩,
  ⟨.reservedBody, .reserved 12⟩
]

def effectLayout : List (PlacedField EffectField) := specialize effectSchema
def effectBytes : Nat := schemaWidth effectSchema

namespace EffectField

def rustName : EffectField → String
  | .magic => "EFFECT_MAGIC_OFFSET" | .version => "EFFECT_VERSION_OFFSET"
  | .action => "EFFECT_ACTION_OFFSET" | .targetRole => "EFFECT_TARGET_ROLE_OFFSET"
  | .reservedHeader => "EFFECT_RESERVED_HEADER_OFFSET"
  | .callerProgram => "EFFECT_CALLER_PROGRAM_OFFSET"
  | .callerAuthority => "EFFECT_CALLER_AUTHORITY_OFFSET"
  | .releaseSet => "EFFECT_RELEASE_SET_OFFSET" | .market => "EFFECT_MARKET_OFFSET"
  | .context => "EFFECT_CONTEXT_OFFSET" | .parentStateDigest => "EFFECT_PARENT_STATE_DIGEST_OFFSET"
  | .roleRequestDigest => "EFFECT_ROLE_REQUEST_DIGEST_OFFSET"
  | .generation => "EFFECT_GENERATION_OFFSET"
  | .expectedResourceARevision => "EFFECT_EXPECTED_RESOURCE_A_REVISION_OFFSET"
  | .expectedResourceBRevision => "EFFECT_EXPECTED_RESOURCE_B_REVISION_OFFSET"
  | .roleRequestBytes => "EFFECT_ROLE_REQUEST_BYTES_OFFSET"
  | .reservedBody => "EFFECT_RESERVED_BODY_OFFSET"

end EffectField

theorem effect_schema_width : effectBytes = 280 := by native_decide
theorem effect_schema_unique : (effectSchema.map fun field => field.name).Nodup := by native_decide
theorem effect_fields_disjoint : effectLayout.Pairwise Before := specializeFrom_pairwise 0 effectSchema

inductive AckField where
  | magic | version | action | targetRole | reserved
  | roleProgram | releaseSet | market | context | effectDigest | postResourceDigest
  | preResourceARevision | postResourceARevision
  | preResourceBRevision | postResourceBRevision
  deriving DecidableEq, Repr

def ackSchema : List (FieldSpec AckField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.targetRole, .u8⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.roleProgram, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.context, .bytes 32⟩, ⟨.effectDigest, .bytes 32⟩,
  ⟨.postResourceDigest, .bytes 32⟩,
  ⟨.preResourceARevision, .u64⟩, ⟨.postResourceARevision, .u64⟩,
  ⟨.preResourceBRevision, .u64⟩, ⟨.postResourceBRevision, .u64⟩
]

def ackLayout : List (PlacedField AckField) := specialize ackSchema
def ackBytes : Nat := schemaWidth ackSchema

namespace AckField

def rustName : AckField → String
  | .magic => "ACK_MAGIC_OFFSET" | .version => "ACK_VERSION_OFFSET"
  | .action => "ACK_ACTION_OFFSET" | .targetRole => "ACK_TARGET_ROLE_OFFSET"
  | .reserved => "ACK_RESERVED_OFFSET" | .roleProgram => "ACK_ROLE_PROGRAM_OFFSET"
  | .releaseSet => "ACK_RELEASE_SET_OFFSET" | .market => "ACK_MARKET_OFFSET"
  | .context => "ACK_CONTEXT_OFFSET" | .effectDigest => "ACK_EFFECT_DIGEST_OFFSET"
  | .postResourceDigest => "ACK_POST_RESOURCE_DIGEST_OFFSET"
  | .preResourceARevision => "ACK_PRE_RESOURCE_A_REVISION_OFFSET"
  | .postResourceARevision => "ACK_POST_RESOURCE_A_REVISION_OFFSET"
  | .preResourceBRevision => "ACK_PRE_RESOURCE_B_REVISION_OFFSET"
  | .postResourceBRevision => "ACK_POST_RESOURCE_B_REVISION_OFFSET"

end AckField

theorem ack_schema_width : ackBytes = 240 := by native_decide
theorem ack_schema_unique : (ackSchema.map fun field => field.name).Nodup := by native_decide
theorem ack_fields_disjoint : ackLayout.Pairwise Before := specializeFrom_pairwise 0 ackSchema

inductive SeriesField where
  | magic | version | action | reservedHeader
  | releaseSet | template | ticket | market | realm | product | beneficiary | founder
  | occurrence | reservedBody | expectedSeriesRevision | expectedTicketRevision
  | marketRent | capabilityRent | work | hoardPrincipal | seriesCloseRent
  deriving DecidableEq, Repr

def seriesSchema : List (FieldSpec SeriesField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.reservedHeader, .reserved 5⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.template, .bytes 32⟩, ⟨.ticket, .bytes 32⟩,
  ⟨.market, .bytes 32⟩, ⟨.realm, .bytes 32⟩, ⟨.product, .bytes 32⟩,
  ⟨.beneficiary, .bytes 32⟩, ⟨.founder, .bytes 32⟩,
  ⟨.occurrence, .u32⟩, ⟨.reservedBody, .reserved 4⟩,
  ⟨.expectedSeriesRevision, .u64⟩, ⟨.expectedTicketRevision, .u64⟩,
  ⟨.marketRent, .u64⟩, ⟨.capabilityRent, .u64⟩, ⟨.work, .u64⟩,
  ⟨.hoardPrincipal, .u64⟩, ⟨.seriesCloseRent, .u64⟩
]

def seriesLayout : List (PlacedField SeriesField) := specialize seriesSchema
def seriesBytes : Nat := schemaWidth seriesSchema

namespace SeriesField

def rustName : SeriesField → String
  | .magic => "SERIES_MAGIC_OFFSET" | .version => "SERIES_VERSION_OFFSET"
  | .action => "SERIES_ACTION_OFFSET" | .reservedHeader => "SERIES_RESERVED_HEADER_OFFSET"
  | .releaseSet => "SERIES_RELEASE_SET_OFFSET" | .template => "SERIES_TEMPLATE_OFFSET"
  | .ticket => "SERIES_TICKET_OFFSET" | .market => "SERIES_MARKET_OFFSET"
  | .realm => "SERIES_REALM_OFFSET" | .product => "SERIES_PRODUCT_OFFSET"
  | .beneficiary => "SERIES_BENEFICIARY_OFFSET" | .founder => "SERIES_FOUNDER_OFFSET"
  | .occurrence => "SERIES_OCCURRENCE_OFFSET" | .reservedBody => "SERIES_RESERVED_BODY_OFFSET"
  | .expectedSeriesRevision => "SERIES_EXPECTED_SERIES_REVISION_OFFSET"
  | .expectedTicketRevision => "SERIES_EXPECTED_TICKET_REVISION_OFFSET"
  | .marketRent => "SERIES_MARKET_RENT_OFFSET" | .capabilityRent => "SERIES_CAPABILITY_RENT_OFFSET"
  | .work => "SERIES_WORK_OFFSET" | .hoardPrincipal => "SERIES_HOARD_PRINCIPAL_OFFSET"
  | .seriesCloseRent => "SERIES_CLOSE_RENT_OFFSET"

end SeriesField

theorem series_schema_width : seriesBytes = 336 := by native_decide
theorem series_schema_unique : (seriesSchema.map fun field => field.name).Nodup := by native_decide
theorem series_fields_disjoint : seriesLayout.Pairwise Before := specializeFrom_pairwise 0 seriesSchema

inductive SeriesAckField where
  | magic | version | action | reserved
  | coreProgram | releaseSet | template | ticket | market | requestDigest | postResourceDigest
  | marketGeneration | expectedSeriesRevision | expectedTicketRevision
  deriving DecidableEq, Repr

def seriesAckSchema : List (FieldSpec SeriesAckField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.reserved, .reserved 5⟩,
  ⟨.coreProgram, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩, ⟨.template, .bytes 32⟩,
  ⟨.ticket, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.requestDigest, .bytes 32⟩,
  ⟨.postResourceDigest, .bytes 32⟩, ⟨.marketGeneration, .u64⟩,
  ⟨.expectedSeriesRevision, .u64⟩, ⟨.expectedTicketRevision, .u64⟩
]

def seriesAckLayout : List (PlacedField SeriesAckField) := specialize seriesAckSchema
def seriesAckBytes : Nat := schemaWidth seriesAckSchema

namespace SeriesAckField

def rustName : SeriesAckField → String
  | .magic => "SERIES_ACK_MAGIC_OFFSET" | .version => "SERIES_ACK_VERSION_OFFSET"
  | .action => "SERIES_ACK_ACTION_OFFSET" | .reserved => "SERIES_ACK_RESERVED_OFFSET"
  | .coreProgram => "SERIES_ACK_CORE_PROGRAM_OFFSET"
  | .releaseSet => "SERIES_ACK_RELEASE_SET_OFFSET" | .template => "SERIES_ACK_TEMPLATE_OFFSET"
  | .ticket => "SERIES_ACK_TICKET_OFFSET" | .market => "SERIES_ACK_MARKET_OFFSET"
  | .requestDigest => "SERIES_ACK_REQUEST_DIGEST_OFFSET"
  | .postResourceDigest => "SERIES_ACK_POST_RESOURCE_DIGEST_OFFSET"
  | .marketGeneration => "SERIES_ACK_MARKET_GENERATION_OFFSET"
  | .expectedSeriesRevision => "SERIES_ACK_EXPECTED_SERIES_REVISION_OFFSET"
  | .expectedTicketRevision => "SERIES_ACK_EXPECTED_TICKET_REVISION_OFFSET"

end SeriesAckField

theorem series_ack_schema_width : seriesAckBytes = 264 := by native_decide
theorem series_ack_schema_unique : (seriesAckSchema.map fun field => field.name).Nodup := by native_decide
theorem series_ack_fields_disjoint : seriesAckLayout.Pairwise Before :=
  specializeFrom_pairwise 0 seriesAckSchema

end DClutch.MarketCorePhysicalAbi
