import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec

/-!
# Series Core Found Acknowledgment V2 ABI

This Found-only acknowledgment promotes the pre-Core funding-count routing hint
only after Core has authenticated the exact ordered FundingState span.  It binds
the nonzero count and the canonical funding-list identity alongside the exact
Market, one-shot permit, request digest, and Core post-resource digest.  Global
plans and executable artifacts remain outer-owned authenticated facts and are
not copied into this Core receipt.
-/

namespace DClutch.SeriesCoreFoundAckV2Abi

open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x53, 0x46, 0x32]
def schemaVersion : Nat := 2
def schemaReleasePreimage : List UInt8 := [
  0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x63, 0x68, 0x65,
  0x6d, 0x61, 0x2f, 0x73, 0x65, 0x72, 0x69, 0x65, 0x73, 0x2d, 0x63, 0x6f,
  0x72, 0x65, 0x2d, 0x66, 0x6f, 0x75, 0x6e, 0x64, 0x2d, 0x61, 0x63, 0x6b,
  0x2d, 0x76, 0x32
]
def schemaReleaseId : List UInt8 := [
  0xdd, 0xd0, 0x5b, 0x1b, 0xdd, 0xf6, 0x61, 0x26, 0xc0, 0xf9, 0x1f, 0x58, 0x21, 0xbf, 0xe1, 0x1e,
  0xb9, 0x54, 0x41, 0xc6, 0x0d, 0x10, 0x23, 0x3a, 0x44, 0x1c, 0x67, 0x7f, 0x32, 0xd5, 0x1a, 0xb3
]

inductive Field where
  | magic | version | fundingCount | reserved
  | coreProgram | releaseSet | template | ticket | market | permit
  | requestDigest | fundingListId | postResourceDigest
  | marketGeneration | expectedSeriesRevision | expectedTicketRevision
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.fundingCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.coreProgram, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩,
  ⟨.template, .bytes 32⟩,
  ⟨.ticket, .bytes 32⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.permit, .bytes 32⟩,
  ⟨.requestDigest, .bytes 32⟩,
  ⟨.fundingListId, .bytes 32⟩,
  ⟨.postResourceDigest, .bytes 32⟩,
  ⟨.marketGeneration, .u64⟩,
  ⟨.expectedSeriesRevision, .u64⟩,
  ⟨.expectedTicketRevision, .u64⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "SERIES_CORE_FOUND_ACK_MAGIC_OFFSET_V2"
  | .version => "SERIES_CORE_FOUND_ACK_VERSION_OFFSET_V2"
  | .fundingCount => "SERIES_CORE_FOUND_ACK_FUNDING_COUNT_OFFSET_V2"
  | .reserved => "SERIES_CORE_FOUND_ACK_RESERVED_OFFSET_V2"
  | .coreProgram => "SERIES_CORE_FOUND_ACK_CORE_PROGRAM_OFFSET_V2"
  | .releaseSet => "SERIES_CORE_FOUND_ACK_RELEASE_SET_OFFSET_V2"
  | .template => "SERIES_CORE_FOUND_ACK_TEMPLATE_OFFSET_V2"
  | .ticket => "SERIES_CORE_FOUND_ACK_TICKET_OFFSET_V2"
  | .market => "SERIES_CORE_FOUND_ACK_MARKET_OFFSET_V2"
  | .permit => "SERIES_CORE_FOUND_ACK_PERMIT_OFFSET_V2"
  | .requestDigest => "SERIES_CORE_FOUND_ACK_REQUEST_DIGEST_OFFSET_V2"
  | .fundingListId => "SERIES_CORE_FOUND_ACK_FUNDING_LIST_ID_OFFSET_V2"
  | .postResourceDigest => "SERIES_CORE_FOUND_ACK_POST_RESOURCE_DIGEST_OFFSET_V2"
  | .marketGeneration => "SERIES_CORE_FOUND_ACK_MARKET_GENERATION_OFFSET_V2"
  | .expectedSeriesRevision => "SERIES_CORE_FOUND_ACK_EXPECTED_SERIES_REVISION_OFFSET_V2"
  | .expectedTicketRevision => "SERIES_CORE_FOUND_ACK_EXPECTED_TICKET_REVISION_OFFSET_V2"

end Field

structure FoundAck where
  fundingCount : Nat
  coreProgram : List UInt8
  releaseSet : List UInt8
  template : List UInt8
  ticket : List UInt8
  market : List UInt8
  permit : List UInt8
  requestDigest : List UInt8
  fundingListId : List UInt8
  postResourceDigest : List UInt8
  marketGeneration : Nat
  expectedSeriesRevision : Nat
  expectedTicketRevision : Nat
  deriving DecidableEq, Repr

def identity (value : List UInt8) : Bool := value.length = 32 && value.any (· != 0)

def valid (ack : FoundAck) : Bool :=
  ack.fundingCount > 0 && ack.fundingCount < 256 &&
    ack.marketGeneration > 0 &&
    [ack.coreProgram, ack.releaseSet, ack.template, ack.ticket, ack.market,
      ack.permit, ack.requestDigest, ack.fundingListId, ack.postResourceDigest].all identity

def canonicalExample : FoundAck := {
  fundingCount := 3
  coreProgram := List.replicate 32 0x11
  releaseSet := List.replicate 32 0x12
  template := List.replicate 32 0x13
  ticket := List.replicate 32 0x14
  market := List.replicate 32 0x15
  permit := List.replicate 32 0x16
  requestDigest := List.replicate 32 0x17
  fundingListId := List.replicate 32 0x18
  postResourceDigest := List.replicate 32 0x19
  marketGeneration := 7
  expectedSeriesRevision := 8
  expectedTicketRevision := 9
}

theorem width_is_exact : bytes = 328 := by native_decide
theorem fields_unique : (schema.map fun field => field.name).Nodup := by native_decide
theorem fields_disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema
theorem schema_identity_widths :
    schemaReleasePreimage.length = 39 ∧ schemaReleaseId.length = 32 := by native_decide
theorem example_valid : valid canonicalExample := by native_decide
theorem zero_funding_refuses : !valid { canonicalExample with fundingCount := 0 } := by native_decide
theorem zero_list_refuses :
    !valid { canonicalExample with fundingListId := List.replicate 32 0 } := by
  native_decide
theorem zero_permit_refuses :
    !valid { canonicalExample with permit := List.replicate 32 0 } := by
  native_decide

end DClutch.SeriesCoreFoundAckV2Abi
