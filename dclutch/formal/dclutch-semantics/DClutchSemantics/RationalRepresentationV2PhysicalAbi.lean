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

def version : Nat := 3

/-!
The descriptor schema is independently versioned. Version three removes the
representation-authority field from the hashed preimage: that authority is the
Claims PDA derived from the finalized descriptor digest, so persisting it in
the digest preimage would require a hash/PDA fixed point.
-/
def descriptorVersion : Nat := 3

/-! ## Immutable finalized descriptor -/

inductive DescriptorField where
  | magic | version | reservedHeader | graphId | graphDigest | rootId | market | releaseSet
  | receiptMint | tokenProgram | outcomeCount
  | reservedTail | denominator
  deriving DecidableEq, Repr

def descriptorSchema : List (FieldSpec DescriptorField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 6⟩,
  ⟨.graphId, .bytes 32⟩, ⟨.graphDigest, .bytes 32⟩,
  ⟨.rootId, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.receiptMint, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩,
  ⟨.outcomeCount, .u32⟩, ⟨.reservedTail, .reserved 4⟩,
  ⟨.denominator, .u64⟩
]

def descriptorLayout : List (PlacedField DescriptorField) := specialize descriptorSchema
def descriptorHeaderBytes : Nat := schemaWidth descriptorSchema
def descriptorCoefficientBytes : Nat := 8

theorem descriptorHeaderIsNonCircular : descriptorHeaderBytes = 256 := by decide

namespace DescriptorField

def all : List DescriptorField := [
  .magic, .version, .reservedHeader, .graphId, .graphDigest, .rootId, .market, .releaseSet,
  .receiptMint, .tokenProgram, .outcomeCount,
  .reservedTail, .denominator
]

def rustName : DescriptorField → String
  | .magic => "DESCRIPTOR_MAGIC_OFFSET"
  | .version => "DESCRIPTOR_VERSION_OFFSET"
  | .reservedHeader => "DESCRIPTOR_RESERVED_HEADER_OFFSET"
  | .graphId => "DESCRIPTOR_GRAPH_ID_OFFSET"
  | .graphDigest => "DESCRIPTOR_GRAPH_DIGEST_OFFSET"
  | .rootId => "DESCRIPTOR_ROOT_ID_OFFSET"
  | .market => "DESCRIPTOR_MARKET_ID_OFFSET"
  | .releaseSet => "DESCRIPTOR_RELEASE_SET_ID_OFFSET"
  | .receiptMint => "DESCRIPTOR_RECEIPT_MINT_OFFSET"
  | .tokenProgram => "DESCRIPTOR_TOKEN_PROGRAM_OFFSET"
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

/-!
## Request vocabulary and the three action-class layouts

Version three makes the request header ACTION-CONDITIONAL.  Version two shipped
one 488-byte header for every action, in which eight to fifteen bytes-worth of
fields were forced to a constant the request contract already checked: `realm`
and `collateralRecipient` are zero unless the action is terminal redemption,
`receiptAccount` is zero unless the action is Structured, each expected-revision
field is the absent sentinel exactly where its action reads no such record, a
selected outcome is `u32::MAX` for a Structured action, and `assetCount` is
`outcomeCount` for a Structured action and one otherwise.  A field that is
forced to a constant by the action carries no information, so version three does
not send it; the decoder restores the constant and the validator still checks
every relation.

The three layouts are NOT three schemas.  `commonSchema` is one list shared by
literal identity, so no field of the common prefix can move in one action's
header without moving in all three, and `specializeFrom_append` (AbiSchema)
turns that sharing into equal offsets rather than a per-class arithmetic audit.
Each `classTail` draws from `requestVocabulary`, which owns the name/width
pairing for the whole ABI: `vocabulary_is_carried_or_derived` then forces every
field of the vocabulary to be carried by some class or named in `derivedFields`,
so a field cannot leave the wire silently.
-/

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

/-- Every field this ABI can name, with the width that name has EVERYWHERE.
No layout may pair a name with a different width; `class_fields_are_vocabulary`
is what forbids it. -/
def requestVocabulary : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.callerRole, .u8⟩,
  ⟨.reservedHeader, .reserved 4⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.graphId, .bytes 32⟩,
  ⟨.descriptorId, .bytes 32⟩, ⟨.parentContext, .bytes 32⟩,
  ⟨.actor, .bytes 32⟩, ⟨.receiptMint, .bytes 32⟩,
  ⟨.representationAuthority, .bytes 32⟩, ⟨.tokenProgram, .bytes 32⟩,
  ⟨.expectedRepresentationRevision, .u64⟩,
  ⟨.generation, .u64⟩, ⟨.quantity, .u64⟩, ⟨.denominator, .u64⟩,
  ⟨.expectedReceiptSupply, .u64⟩, ⟨.outcomeCount, .u32⟩,
  ⟨.receiptAccount, .bytes 32⟩,
  ⟨.realm, .bytes 32⟩, ⟨.collateralRecipient, .bytes 32⟩,
  ⟨.expectedClaimsMarketRevision, .u64⟩,
  ⟨.expectedActorPositionRevision, .u64⟩,
  ⟨.expectedCustodyPositionRevision, .u64⟩,
  ⟨.expectedCustodyReplayRevision, .u64⟩,
  ⟨.selectedOutcome, .u32⟩, ⟨.assetCount, .u32⟩,
  ⟨.reservedTail, .reserved 4⟩
]

/-- The action classes the request contract already distinguishes.  `selected`
is Denominate and Reconstitute; `terminal` is RedeemTerminal; `structured` is
Structured issue and unwrap. -/
inductive RequestClass where
  | structured | selected | terminal
  deriving DecidableEq, Repr

def RequestClass.all : List RequestClass := [.structured, .selected, .terminal]

/-- The fields every action carries, at offsets every action shares.  One list,
shared by identity -- not three copies that a theorem later compares. -/
def commonSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.callerRole, .u8⟩,
  ⟨.reservedHeader, .reserved 4⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.graphId, .bytes 32⟩,
  ⟨.descriptorId, .bytes 32⟩, ⟨.parentContext, .bytes 32⟩,
  ⟨.actor, .bytes 32⟩, ⟨.receiptMint, .bytes 32⟩,
  ⟨.representationAuthority, .bytes 32⟩, ⟨.tokenProgram, .bytes 32⟩,
  ⟨.expectedRepresentationRevision, .u64⟩,
  ⟨.generation, .u64⟩, ⟨.quantity, .u64⟩, ⟨.denominator, .u64⟩,
  ⟨.expectedReceiptSupply, .u64⟩, ⟨.outcomeCount, .u32⟩
]

/-- What each action class carries beyond the common prefix.  A field is absent
exactly where the request contract forces it to a constant:

* `receiptAccount` is nonzero iff the action is Structured.
* `realm` and `collateralRecipient` are nonzero iff the action is terminal.
* `expectedClaimsMarketRevision` is present iff the action executes a Claims
  plan, which Structured does not.
* `expectedActorPositionRevision` is present only for `selected`.
* `expectedCustodyPositionRevision` is present for `selected` and `terminal`.
* `expectedCustodyReplayRevision` is present only for `terminal`, where it is
  genuinely conditional on a positive payout and so must ride the wire.
* `selectedOutcome` is `u32::MAX` for Structured.
-/
def classTail : RequestClass → List (FieldSpec RequestField)
  | .structured => [
      ⟨.receiptAccount, .bytes 32⟩,
      ⟨.reservedTail, .reserved 4⟩]
  | .selected => [
      ⟨.expectedClaimsMarketRevision, .u64⟩,
      ⟨.expectedActorPositionRevision, .u64⟩,
      ⟨.expectedCustodyPositionRevision, .u64⟩,
      ⟨.selectedOutcome, .u32⟩,
      ⟨.reservedTail, .reserved 4⟩]
  | .terminal => [
      ⟨.realm, .bytes 32⟩, ⟨.collateralRecipient, .bytes 32⟩,
      ⟨.expectedClaimsMarketRevision, .u64⟩,
      ⟨.expectedCustodyPositionRevision, .u64⟩,
      ⟨.expectedCustodyReplayRevision, .u64⟩,
      ⟨.selectedOutcome, .u32⟩,
      ⟨.reservedTail, .reserved 4⟩]

def classSchema (kind : RequestClass) : List (FieldSpec RequestField) :=
  commonSchema ++ classTail kind

def classLayout (kind : RequestClass) : List (PlacedField RequestField) :=
  specialize (classSchema kind)

def classHeaderBytes (kind : RequestClass) : Nat := schemaWidth (classSchema kind)

def commonPrefixBytes : Nat := schemaWidth commonSchema

/-- Carried by no class: `assetCount` is `outcomeCount` for a Structured action
and one for every selected-outcome action, so the decoder derives it from the
action and the outcome count rather than reading it. -/
def derivedFields : List RequestField := [.assetCount]

namespace RequestField

def all : List RequestField := [
  .magic, .version, .action, .callerRole, .reservedHeader, .releaseSet,
  .market, .graphId, .descriptorId, .parentContext, .actor, .receiptMint,
  .representationAuthority, .tokenProgram, .expectedRepresentationRevision,
  .generation, .quantity, .denominator, .expectedReceiptSupply, .outcomeCount,
  .receiptAccount, .realm, .collateralRecipient,
  .expectedClaimsMarketRevision, .expectedActorPositionRevision,
  .expectedCustodyPositionRevision, .expectedCustodyReplayRevision,
  .selectedOutcome, .assetCount, .reservedTail
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

/-- Offset within one action class, or `none` when that class does not carry
the field.  There is no class-free offset in version three; asking for one is
the mistake this signature makes impossible. -/
def offsetIn? (kind : RequestClass) (field : RequestField) : Option Nat :=
  (coordinate? field (classLayout kind)).map Prod.fst

end RequestField

namespace RequestClass

def rustName : RequestClass → String
  | .structured => "STRUCTURED"
  | .selected => "SELECTED"
  | .terminal => "TERMINAL"

end RequestClass

/-!
## The asset row after commit-don't-inline

Version two sent four 32-byte keys per coordinate.  Three of them --
`shardMint`, `structuredCustodyAccount` and `claimsCustodyOwner` -- are program
addresses the Claims adapter DERIVES from `(program_id, descriptorId, outcome)`
and then required to equal the inlined copy.  Derivation is the authentication,
so the copy authenticated nothing the chain did not already compute; version
three drops all three and the adapter's derivation becomes their only author.
`actorShardAccount` is not derivable -- it is the actor's own Token Account --
and stays.

What this costs, stated rather than left implicit: the pairwise distinctness of
the three derived keys across rows was an explicit wire check and is now a
consequence of `find_program_address` injectivity over distinct `outcome`
seeds.  `actorShardAccount` remains explicitly checked, both pairwise and
against the receipt accounts, because it still rides the wire.
-/

inductive AssetField where
  | actorShardAccount
  | coefficient | expectedShardSupply | expectedActorShards
  | expectedStructuredShards
  deriving DecidableEq, Repr

def assetSchema : List (FieldSpec AssetField) := [
  ⟨.actorShardAccount, .bytes 32⟩,
  ⟨.coefficient, .u64⟩, ⟨.expectedShardSupply, .u64⟩,
  ⟨.expectedActorShards, .u64⟩, ⟨.expectedStructuredShards, .u64⟩
]

def assetLayout : List (PlacedField AssetField) := specialize assetSchema
def assetBytes : Nat := schemaWidth assetSchema

namespace AssetField

def all : List AssetField := [
  .actorShardAccount, .coefficient, .expectedShardSupply,
  .expectedActorShards, .expectedStructuredShards
]

def rustName : AssetField → String
  | .actorShardAccount => "ASSET_ACTOR_SHARD_ACCOUNT_OFFSET"
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

/-! ## Layout laws -/

theorem asset_schema_well_formed : WellFormed assetSchema := by
  simp [WellFormed, assetSchema, FieldKind.byteWidth]
theorem receipt_schema_well_formed : WellFormed receiptSchema := by
  simp [WellFormed, receiptSchema, FieldKind.byteWidth]
theorem descriptor_schema_well_formed : WellFormed descriptorSchema := by
  simp [WellFormed, descriptorSchema, FieldKind.byteWidth]

theorem asset_layout_disjoint : assetLayout.Pairwise Before :=
  specializeFrom_pairwise 0 assetSchema

theorem receipt_layout_disjoint : receiptLayout.Pairwise Before :=
  specializeFrom_pairwise 0 receiptSchema

theorem descriptor_layout_disjoint : descriptorLayout.Pairwise Before :=
  specializeFrom_pairwise 0 descriptorSchema

/-! ### The three action-class layouts

Each law below is stated per class rather than once over an abstraction,
because the point of the exercise is that a reader can see the numbers a
generator will emit. -/

theorem class_schema_well_formed (kind : RequestClass) :
    WellFormed (classSchema kind) := by
  cases kind <;> exact ⟨by native_decide, by native_decide⟩

theorem class_layout_disjoint (kind : RequestClass) :
    (classLayout kind).Pairwise Before :=
  specializeFrom_pairwise 0 (classSchema kind)

/-- THE WIDTHS. Version two was one 488-byte header for every action. -/
theorem structured_header_bytes : classHeaderBytes .structured = 384 := by native_decide
theorem selected_header_bytes : classHeaderBytes .selected = 380 := by native_decide
theorem terminal_header_bytes : classHeaderBytes .terminal = 444 := by native_decide
theorem common_prefix_bytes : commonPrefixBytes = 348 := by native_decide

/-- The common prefix is placed identically in all three classes.  This is not
an arithmetic coincidence to be re-audited per class: `classSchema` is
`commonSchema ++ classTail kind` by definition, and `specializeFrom_append`
turns that into equal offsets. -/
theorem common_prefix_offsets_agree (kind : RequestClass) :
    (classLayout kind).take commonSchema.length = specialize commonSchema := by
  have expand : classLayout kind =
      specialize commonSchema ++
        specializeFrom (0 + schemaWidth commonSchema) (classTail kind) := by
    simpa [classLayout, classSchema, specialize] using
      specializeFrom_append 0 commonSchema (classTail kind)
  rw [expand, List.take_left' (by simp [specialize, specializeFrom_length])]

/-- Every field any class carries is a field of the one vocabulary, with the
vocabulary's width.  This is what forbids a name meaning one width in one
action's header and another width in another. -/
theorem class_fields_are_vocabulary (kind : RequestClass) :
    ∀ field ∈ classSchema kind, field ∈ requestVocabulary := by
  cases kind <;> native_decide

/-- No field of the vocabulary leaves the wire silently: it is carried by some
action class, or it is named in `derivedFields`. -/
theorem vocabulary_is_carried_or_derived :
    ∀ field ∈ requestVocabulary,
      field ∈ classSchema .structured ∨ field ∈ classSchema .selected ∨
        field ∈ classSchema .terminal ∨ field.name ∈ derivedFields := by
  native_decide

/-- And nothing named derived is on any wire. -/
theorem derived_fields_are_carried_by_no_class (kind : RequestClass) :
    ∀ field ∈ classSchema kind, field.name ∉ derivedFields := by
  cases kind <;> native_decide

/-- The vocabulary names one owner per field. -/
theorem request_vocabulary_well_formed : WellFormed requestVocabulary :=
  ⟨by native_decide, by native_decide⟩

/-- THE COORDINATES a generator emits, per class.  A field that moves breaks
the class it moved in, by name. -/
theorem structured_coordinates : coordinates (classLayout .structured) = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1), (.callerRole, 11, 1),
    (.reservedHeader, 12, 4), (.releaseSet, 16, 32), (.market, 48, 32),
    (.graphId, 80, 32), (.descriptorId, 112, 32), (.parentContext, 144, 32),
    (.actor, 176, 32), (.receiptMint, 208, 32),
    (.representationAuthority, 240, 32), (.tokenProgram, 272, 32),
    (.expectedRepresentationRevision, 304, 8), (.generation, 312, 8),
    (.quantity, 320, 8), (.denominator, 328, 8),
    (.expectedReceiptSupply, 336, 8), (.outcomeCount, 344, 4),
    (.receiptAccount, 348, 32), (.reservedTail, 380, 4)
  ] := by native_decide

theorem selected_coordinates : coordinates (classLayout .selected) = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1), (.callerRole, 11, 1),
    (.reservedHeader, 12, 4), (.releaseSet, 16, 32), (.market, 48, 32),
    (.graphId, 80, 32), (.descriptorId, 112, 32), (.parentContext, 144, 32),
    (.actor, 176, 32), (.receiptMint, 208, 32),
    (.representationAuthority, 240, 32), (.tokenProgram, 272, 32),
    (.expectedRepresentationRevision, 304, 8), (.generation, 312, 8),
    (.quantity, 320, 8), (.denominator, 328, 8),
    (.expectedReceiptSupply, 336, 8), (.outcomeCount, 344, 4),
    (.expectedClaimsMarketRevision, 348, 8),
    (.expectedActorPositionRevision, 356, 8),
    (.expectedCustodyPositionRevision, 364, 8),
    (.selectedOutcome, 372, 4), (.reservedTail, 376, 4)
  ] := by native_decide

theorem terminal_coordinates : coordinates (classLayout .terminal) = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1), (.callerRole, 11, 1),
    (.reservedHeader, 12, 4), (.releaseSet, 16, 32), (.market, 48, 32),
    (.graphId, 80, 32), (.descriptorId, 112, 32), (.parentContext, 144, 32),
    (.actor, 176, 32), (.receiptMint, 208, 32),
    (.representationAuthority, 240, 32), (.tokenProgram, 272, 32),
    (.expectedRepresentationRevision, 304, 8), (.generation, 312, 8),
    (.quantity, 320, 8), (.denominator, 328, 8),
    (.expectedReceiptSupply, 336, 8), (.outcomeCount, 344, 4),
    (.realm, 348, 32), (.collateralRecipient, 380, 32),
    (.expectedClaimsMarketRevision, 412, 8),
    (.expectedCustodyPositionRevision, 420, 8),
    (.expectedCustodyReplayRevision, 428, 8),
    (.selectedOutcome, 436, 4), (.reservedTail, 440, 4)
  ] := by native_decide

/-- The asset row after the three derived keys leave it. -/
theorem asset_bytes_after_commit_dont_inline : assetBytes = 64 := by native_decide

theorem asset_coordinates : coordinates assetLayout = [
    (.actorShardAccount, 0, 32), (.coefficient, 32, 8),
    (.expectedShardSupply, 40, 8), (.expectedActorShards, 48, 8),
    (.expectedStructuredShards, 56, 8)
  ] := by native_decide

end DClutch.RationalRepresentationV2PhysicalAbi
