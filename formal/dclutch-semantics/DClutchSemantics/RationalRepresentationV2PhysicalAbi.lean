import DClutchSemantics.AbiSchema
import DClutchSemantics.RationalRepresentationV2

/-!
# Rational representation physical composition ABI

This module owns the request, dynamic asset row, and fixed receipt layout for
the Claims-owned physical adapter.  The request binds one upstream context and
every Token account participating in denomination, reconstitution, Structured
issue/unwrap, or exact terminal shard redemption.  The adapter composes the
canonical Claims and Custody contracts; it does not introduce another native
claim, collateral, supply, or holder ledger.

The request tail is runtime-width. Selected-outcome actions carry exactly one
asset row; Structured actions carry exactly the Product outcome count in
Product order. Every offset below is specialized from schema data.
-/

namespace DClutch.RationalRepresentationV2PhysicalAbi

open DClutch.AbiSchema

def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x52, 0x52, 0x50, 0x52, 0x51, 0x32] -- `DCRRPRQ2`
def receiptMagic : List UInt8 :=
  [0x44, 0x43, 0x52, 0x52, 0x50, 0x52, 0x43, 0x32] -- `DCRRPRC2`

def version : Nat := 2

/-! ## Immutable finalized descriptor -/

inductive DescriptorField where
  | magic | version | reservedHeader | graphId | rootId | market | releaseSet
  | receiptMint | tokenProgram | representationAuthority | outcomeCount
  | reservedTail | denominator
  deriving DecidableEq, Repr

def descriptorSchema : List (FieldSpec DescriptorField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 6⟩,
  ⟨.graphId, .bytes 32⟩, ⟨.rootId, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.receiptMint, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩, ⟨.representationAuthority, .bytes 32⟩,
  ⟨.outcomeCount, .u32⟩, ⟨.reservedTail, .reserved 4⟩,
  ⟨.denominator, .u64⟩
]

def descriptorLayout : List (PlacedField DescriptorField) := specialize descriptorSchema
def descriptorHeaderBytes : Nat := schemaWidth descriptorSchema
def descriptorCoefficientBytes : Nat := 8

namespace DescriptorField

def all : List DescriptorField := [
  .magic, .version, .reservedHeader, .graphId, .rootId, .market, .releaseSet,
  .receiptMint, .tokenProgram, .representationAuthority, .outcomeCount,
  .reservedTail, .denominator
]

def rustName : DescriptorField → String
  | .magic => "DESCRIPTOR_MAGIC_OFFSET"
  | .version => "DESCRIPTOR_VERSION_OFFSET"
  | .reservedHeader => "DESCRIPTOR_RESERVED_HEADER_OFFSET"
  | .graphId => "DESCRIPTOR_GRAPH_ID_OFFSET"
  | .rootId => "DESCRIPTOR_ROOT_ID_OFFSET"
  | .market => "DESCRIPTOR_MARKET_ID_OFFSET"
  | .releaseSet => "DESCRIPTOR_RELEASE_SET_ID_OFFSET"
  | .receiptMint => "DESCRIPTOR_RECEIPT_MINT_OFFSET"
  | .tokenProgram => "DESCRIPTOR_TOKEN_PROGRAM_OFFSET"
  | .representationAuthority => "DESCRIPTOR_AUTHORITY_OFFSET"
  | .outcomeCount => "DESCRIPTOR_OUTCOME_COUNT_OFFSET"
  | .reservedTail => "DESCRIPTOR_RESERVED_OFFSET"
  | .denominator => "DESCRIPTOR_DENOMINATOR_OFFSET"

def offset (field : DescriptorField) : Nat :=
  ((coordinate? field descriptorLayout).getD (0, 0)).1

end DescriptorField

inductive Action where
  | denominate | reconstitute | issueStructured | unwrapStructured | redeemTerminal
  deriving DecidableEq, Repr

def Action.tag : Action → Nat
  | .denominate => 1
  | .reconstitute => 2
  | .issueStructured => 3
  | .unwrapStructured => 4
  | .redeemTerminal => 5

inductive CallerRole where
  | core | trading
  deriving DecidableEq, Repr

def CallerRole.tag : CallerRole → Nat
  | .core => 0
  | .trading => 2

inductive RequestField where
  | magic | version | action | callerRole | reservedHeader
  | releaseSet | market | graphId | descriptorId | parentContext | actor
  | receiptMint | receiptAccount | representationAuthority | tokenProgram
  | realm | collateralRecipient
  | expectedRepresentationRevision | expectedClaimsMarketRevision
  | expectedActorPositionRevision | expectedCustodyPositionRevision
  | expectedCustodyReplayRevision | generation | quantity | denominator
  | expectedReceiptSupply | outcomeCount | selectedOutcome | assetCount
  | reservedTail
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.callerRole, .u8⟩,
  ⟨.reservedHeader, .reserved 4⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.graphId, .bytes 32⟩,
  ⟨.descriptorId, .bytes 32⟩, ⟨.parentContext, .bytes 32⟩,
  ⟨.actor, .bytes 32⟩, ⟨.receiptMint, .bytes 32⟩,
  ⟨.receiptAccount, .bytes 32⟩, ⟨.representationAuthority, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩, ⟨.realm, .bytes 32⟩,
  ⟨.collateralRecipient, .bytes 32⟩,
  ⟨.expectedRepresentationRevision, .u64⟩,
  ⟨.expectedClaimsMarketRevision, .u64⟩,
  ⟨.expectedActorPositionRevision, .u64⟩,
  ⟨.expectedCustodyPositionRevision, .u64⟩,
  ⟨.expectedCustodyReplayRevision, .u64⟩,
  ⟨.generation, .u64⟩, ⟨.quantity, .u64⟩, ⟨.denominator, .u64⟩,
  ⟨.expectedReceiptSupply, .u64⟩,
  ⟨.outcomeCount, .u32⟩, ⟨.selectedOutcome, .u32⟩,
  ⟨.assetCount, .u32⟩, ⟨.reservedTail, .reserved 4⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestHeaderBytes : Nat := schemaWidth requestSchema

namespace RequestField

def all : List RequestField := [
  .magic, .version, .action, .callerRole, .reservedHeader, .releaseSet,
  .market, .graphId, .descriptorId, .parentContext, .actor, .receiptMint,
  .receiptAccount, .representationAuthority, .tokenProgram, .realm,
  .collateralRecipient, .expectedRepresentationRevision,
  .expectedClaimsMarketRevision, .expectedActorPositionRevision,
  .expectedCustodyPositionRevision, .expectedCustodyReplayRevision,
  .generation, .quantity, .denominator, .expectedReceiptSupply,
  .outcomeCount, .selectedOutcome, .assetCount, .reservedTail
]

def rustName : RequestField → String
  | .magic => "REQUEST_MAGIC_OFFSET"
  | .version => "REQUEST_VERSION_OFFSET"
  | .action => "REQUEST_ACTION_OFFSET"
  | .callerRole => "REQUEST_CALLER_ROLE_OFFSET"
  | .reservedHeader => "REQUEST_RESERVED_HEADER_OFFSET"
  | .releaseSet => "REQUEST_RELEASE_SET_OFFSET"
  | .market => "REQUEST_MARKET_OFFSET"
  | .graphId => "REQUEST_GRAPH_ID_OFFSET"
  | .descriptorId => "REQUEST_DESCRIPTOR_ID_OFFSET"
  | .parentContext => "REQUEST_PARENT_CONTEXT_OFFSET"
  | .actor => "REQUEST_ACTOR_OFFSET"
  | .receiptMint => "REQUEST_RECEIPT_MINT_OFFSET"
  | .receiptAccount => "REQUEST_RECEIPT_ACCOUNT_OFFSET"
  | .representationAuthority => "REQUEST_REPRESENTATION_AUTHORITY_OFFSET"
  | .tokenProgram => "REQUEST_TOKEN_PROGRAM_OFFSET"
  | .realm => "REQUEST_REALM_OFFSET"
  | .collateralRecipient => "REQUEST_COLLATERAL_RECIPIENT_OFFSET"
  | .expectedRepresentationRevision => "REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET"
  | .expectedClaimsMarketRevision => "REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET"
  | .expectedActorPositionRevision => "REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET"
  | .expectedCustodyPositionRevision => "REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET"
  | .expectedCustodyReplayRevision => "REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET"
  | .generation => "REQUEST_GENERATION_OFFSET"
  | .quantity => "REQUEST_QUANTITY_OFFSET"
  | .denominator => "REQUEST_DENOMINATOR_OFFSET"
  | .expectedReceiptSupply => "REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET"
  | .outcomeCount => "REQUEST_OUTCOME_COUNT_OFFSET"
  | .selectedOutcome => "REQUEST_SELECTED_OUTCOME_OFFSET"
  | .assetCount => "REQUEST_ASSET_COUNT_OFFSET"
  | .reservedTail => "REQUEST_RESERVED_TAIL_OFFSET"

def offset (field : RequestField) : Nat :=
  ((coordinate? field requestLayout).getD (0, 0)).1

end RequestField

inductive AssetField where
  | shardMint | actorShardAccount | structuredCustodyAccount | claimsCustodyOwner
  | coefficient | expectedShardSupply | expectedActorShards
  | expectedStructuredShards
  deriving DecidableEq, Repr

def assetSchema : List (FieldSpec AssetField) := [
  ⟨.shardMint, .bytes 32⟩, ⟨.actorShardAccount, .bytes 32⟩,
  ⟨.structuredCustodyAccount, .bytes 32⟩, ⟨.claimsCustodyOwner, .bytes 32⟩,
  ⟨.coefficient, .u64⟩, ⟨.expectedShardSupply, .u64⟩,
  ⟨.expectedActorShards, .u64⟩, ⟨.expectedStructuredShards, .u64⟩
]

def assetLayout : List (PlacedField AssetField) := specialize assetSchema
def assetBytes : Nat := schemaWidth assetSchema

namespace AssetField

def all : List AssetField := [
  .shardMint, .actorShardAccount, .structuredCustodyAccount,
  .claimsCustodyOwner, .coefficient, .expectedShardSupply,
  .expectedActorShards, .expectedStructuredShards
]

def rustName : AssetField → String
  | .shardMint => "ASSET_SHARD_MINT_OFFSET"
  | .actorShardAccount => "ASSET_ACTOR_SHARD_ACCOUNT_OFFSET"
  | .structuredCustodyAccount => "ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET"
  | .claimsCustodyOwner => "ASSET_CLAIMS_CUSTODY_OWNER_OFFSET"
  | .coefficient => "ASSET_COEFFICIENT_OFFSET"
  | .expectedShardSupply => "ASSET_EXPECTED_SHARD_SUPPLY_OFFSET"
  | .expectedActorShards => "ASSET_EXPECTED_ACTOR_SHARDS_OFFSET"
  | .expectedStructuredShards => "ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET"

def offset (field : AssetField) : Nat :=
  ((coordinate? field assetLayout).getD (0, 0)).1

end AssetField

inductive ReceiptField where
  | magic | version | action | callerRole | reservedHeader | releaseSet | market
  | graphId | descriptorId | parentContext | requestDigest | actor
  | representationProgram | claimsProgram | tokenProgram | claimsPlanDigest
  | claimsResourceDigest | tokenEffectDigest | custodyRequestDigest
  | custodyReceiptDigest | postResourceDigest | preRepresentationRevision
  | postRepresentationRevision | postClaimsMarketRevision
  | postActorPositionRevision | postCustodyPositionRevision
  | postReceiptSupply | payout | outcomeCount | reservedTail
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.callerRole, .u8⟩,
  ⟨.reservedHeader, .reserved 4⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.market, .bytes 32⟩, ⟨.graphId, .bytes 32⟩, ⟨.descriptorId, .bytes 32⟩,
  ⟨.parentContext, .bytes 32⟩, ⟨.requestDigest, .bytes 32⟩, ⟨.actor, .bytes 32⟩,
  ⟨.representationProgram, .bytes 32⟩, ⟨.claimsProgram, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩, ⟨.claimsPlanDigest, .bytes 32⟩,
  ⟨.claimsResourceDigest, .bytes 32⟩, ⟨.tokenEffectDigest, .bytes 32⟩,
  ⟨.custodyRequestDigest, .bytes 32⟩, ⟨.custodyReceiptDigest, .bytes 32⟩,
  ⟨.postResourceDigest, .bytes 32⟩,
  ⟨.preRepresentationRevision, .u64⟩, ⟨.postRepresentationRevision, .u64⟩,
  ⟨.postClaimsMarketRevision, .u64⟩, ⟨.postActorPositionRevision, .u64⟩,
  ⟨.postCustodyPositionRevision, .u64⟩, ⟨.postReceiptSupply, .u64⟩,
  ⟨.payout, .u64⟩, ⟨.outcomeCount, .u32⟩, ⟨.reservedTail, .reserved 4⟩
]

def receiptLayout : List (PlacedField ReceiptField) := specialize receiptSchema
def receiptBytes : Nat := schemaWidth receiptSchema

namespace ReceiptField

def all : List ReceiptField := [
  .magic, .version, .action, .callerRole, .reservedHeader, .releaseSet, .market,
  .graphId, .descriptorId, .parentContext, .requestDigest, .actor,
  .representationProgram, .claimsProgram, .tokenProgram, .claimsPlanDigest,
  .claimsResourceDigest, .tokenEffectDigest, .custodyRequestDigest,
  .custodyReceiptDigest, .postResourceDigest, .preRepresentationRevision,
  .postRepresentationRevision, .postClaimsMarketRevision,
  .postActorPositionRevision, .postCustodyPositionRevision,
  .postReceiptSupply, .payout, .outcomeCount, .reservedTail
]

def rustName : ReceiptField → String
  | .magic => "RECEIPT_MAGIC_OFFSET"
  | .version => "RECEIPT_VERSION_OFFSET"
  | .action => "RECEIPT_ACTION_OFFSET"
  | .callerRole => "RECEIPT_CALLER_ROLE_OFFSET"
  | .reservedHeader => "RECEIPT_RESERVED_HEADER_OFFSET"
  | .releaseSet => "RECEIPT_RELEASE_SET_OFFSET"
  | .market => "RECEIPT_MARKET_OFFSET"
  | .graphId => "RECEIPT_GRAPH_ID_OFFSET"
  | .descriptorId => "RECEIPT_DESCRIPTOR_ID_OFFSET"
  | .parentContext => "RECEIPT_PARENT_CONTEXT_OFFSET"
  | .requestDigest => "RECEIPT_REQUEST_DIGEST_OFFSET"
  | .actor => "RECEIPT_ACTOR_OFFSET"
  | .representationProgram => "RECEIPT_REPRESENTATION_PROGRAM_OFFSET"
  | .claimsProgram => "RECEIPT_CLAIMS_PROGRAM_OFFSET"
  | .tokenProgram => "RECEIPT_TOKEN_PROGRAM_OFFSET"
  | .claimsPlanDigest => "RECEIPT_CLAIMS_PLAN_DIGEST_OFFSET"
  | .claimsResourceDigest => "RECEIPT_CLAIMS_RESOURCE_DIGEST_OFFSET"
  | .tokenEffectDigest => "RECEIPT_TOKEN_EFFECT_DIGEST_OFFSET"
  | .custodyRequestDigest => "RECEIPT_CUSTODY_REQUEST_DIGEST_OFFSET"
  | .custodyReceiptDigest => "RECEIPT_CUSTODY_RECEIPT_DIGEST_OFFSET"
  | .postResourceDigest => "RECEIPT_POST_RESOURCE_DIGEST_OFFSET"
  | .preRepresentationRevision => "RECEIPT_PRE_REPRESENTATION_REVISION_OFFSET"
  | .postRepresentationRevision => "RECEIPT_POST_REPRESENTATION_REVISION_OFFSET"
  | .postClaimsMarketRevision => "RECEIPT_POST_CLAIMS_MARKET_REVISION_OFFSET"
  | .postActorPositionRevision => "RECEIPT_POST_ACTOR_POSITION_REVISION_OFFSET"
  | .postCustodyPositionRevision => "RECEIPT_POST_CUSTODY_POSITION_REVISION_OFFSET"
  | .postReceiptSupply => "RECEIPT_POST_RECEIPT_SUPPLY_OFFSET"
  | .payout => "RECEIPT_PAYOUT_OFFSET"
  | .outcomeCount => "RECEIPT_OUTCOME_COUNT_OFFSET"
  | .reservedTail => "RECEIPT_RESERVED_TAIL_OFFSET"

def offset (field : ReceiptField) : Nat :=
  ((coordinate? field receiptLayout).getD (0, 0)).1

end ReceiptField

theorem request_schema_well_formed : WellFormed requestSchema := by
  simp [WellFormed, requestSchema, FieldKind.byteWidth]
theorem asset_schema_well_formed : WellFormed assetSchema := by
  simp [WellFormed, assetSchema, FieldKind.byteWidth]
theorem receipt_schema_well_formed : WellFormed receiptSchema := by
  simp [WellFormed, receiptSchema, FieldKind.byteWidth]
theorem descriptor_schema_well_formed : WellFormed descriptorSchema := by
  simp [WellFormed, descriptorSchema, FieldKind.byteWidth]

theorem request_layout_disjoint : requestLayout.Pairwise Before :=
  specializeFrom_pairwise 0 requestSchema

theorem asset_layout_disjoint : assetLayout.Pairwise Before :=
  specializeFrom_pairwise 0 assetSchema

theorem receipt_layout_disjoint : receiptLayout.Pairwise Before :=
  specializeFrom_pairwise 0 receiptSchema

theorem descriptor_layout_disjoint : descriptorLayout.Pairwise Before :=
  specializeFrom_pairwise 0 descriptorSchema

end DClutch.RationalRepresentationV2PhysicalAbi
