import DClutchSemantics.AbiCoverage

/-!
# Claims conservation request ABI (`DCLCNS01`)

The wire of a split or a merge as a USER ACT: one Position owner's signed
statement of the complete sets they create or destroy, the collateral those
sets are worth at the Market's basis scale, and every account and prestate the
act is held to.  `crates/dclutch-claims/src/conservation/mod.rs` decoded and
encoded it against twenty-nine decimal literals; every one of them is a
placement below, and the width the crate pinned as a literal `592` is the
schema's own width.

The header is the tree's shared prologue -- magic, version, a one-byte
selector, a reserved span to sixteen -- followed by fifteen identities, eleven
scalars, the runtime width and a reserved tail.  Nothing in the layout says
which SHAPE the Market is (categorical or refunding): the record says that, and
the same bytes drive both.
-/

namespace DClutch.ClaimsConservationV1Abi

open DClutch.AbiSchema

/-- Implemented schema version. -/
def version : Nat := 1

/-- `DCLCNS01`. -/
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x43, 0x4e, 0x53, 0x30, 0x31]

/-- The two directions, as the one-byte selector spells them. -/
def splitTag : Nat := 0
def mergeTag : Nat := 1

inductive RequestField where
  | magic | version | direction | reservedHeader
  | realm | market | releaseSet | custodyContext | aggregate | position | owner
  | externalCollateral | hoardVault | mint | tokenProgram | claimsProgram
  | productRecordDigest | linkedBasisRecordDigest | semanticBasisId
  | generation | quantity | basisScale | collateralAtoms
  | expectedMarketRevision | expectedPositionRevision | expectedCustodyRevision
  | preExternalAmount | postExternalAmount | preHoardAmount | postHoardAmount
  | claimCount | reservedTail
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.direction, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.realm, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.custodyContext, .bytes 32⟩, ⟨.aggregate, .bytes 32⟩, ⟨.position, .bytes 32⟩,
  ⟨.owner, .bytes 32⟩, ⟨.externalCollateral, .bytes 32⟩, ⟨.hoardVault, .bytes 32⟩,
  ⟨.mint, .bytes 32⟩, ⟨.tokenProgram, .bytes 32⟩, ⟨.claimsProgram, .bytes 32⟩,
  ⟨.productRecordDigest, .bytes 32⟩, ⟨.linkedBasisRecordDigest, .bytes 32⟩,
  ⟨.semanticBasisId, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.quantity, .u64⟩, ⟨.basisScale, .u64⟩,
  ⟨.collateralAtoms, .u64⟩,
  ⟨.expectedMarketRevision, .u64⟩, ⟨.expectedPositionRevision, .u64⟩,
  ⟨.expectedCustodyRevision, .u64⟩,
  ⟨.preExternalAmount, .u64⟩, ⟨.postExternalAmount, .u64⟩,
  ⟨.preHoardAmount, .u64⟩, ⟨.postHoardAmount, .u64⟩,
  ⟨.claimCount, .u32⟩, ⟨.reservedTail, .reserved 4⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestBytes : Nat := schemaWidth requestSchema

namespace RequestField

def all : List RequestField := [
  .magic, .version, .direction, .reservedHeader,
  .realm, .market, .releaseSet, .custodyContext, .aggregate, .position, .owner,
  .externalCollateral, .hoardVault, .mint, .tokenProgram, .claimsProgram,
  .productRecordDigest, .linkedBasisRecordDigest, .semanticBasisId,
  .generation, .quantity, .basisScale, .collateralAtoms,
  .expectedMarketRevision, .expectedPositionRevision, .expectedCustodyRevision,
  .preExternalAmount, .postExternalAmount, .preHoardAmount, .postHoardAmount,
  .claimCount, .reservedTail
]

def rustName : RequestField → String
  | .magic => "MAGIC_OFFSET"
  | .version => "VERSION_OFFSET"
  | .direction => "DIRECTION_OFFSET"
  | .reservedHeader => "HEADER_RESERVED_OFFSET"
  | .realm => "REALM_OFFSET"
  | .market => "MARKET_OFFSET"
  | .releaseSet => "RELEASE_SET_OFFSET"
  | .custodyContext => "CUSTODY_CONTEXT_OFFSET"
  | .aggregate => "AGGREGATE_OFFSET"
  | .position => "POSITION_OFFSET"
  | .owner => "OWNER_OFFSET"
  | .externalCollateral => "EXTERNAL_COLLATERAL_OFFSET"
  | .hoardVault => "HOARD_VAULT_OFFSET"
  | .mint => "MINT_OFFSET"
  | .tokenProgram => "TOKEN_PROGRAM_OFFSET"
  | .claimsProgram => "CLAIMS_PROGRAM_OFFSET"
  | .productRecordDigest => "PRODUCT_RECORD_DIGEST_OFFSET"
  | .linkedBasisRecordDigest => "LINKED_BASIS_RECORD_DIGEST_OFFSET"
  | .semanticBasisId => "SEMANTIC_BASIS_ID_OFFSET"
  | .generation => "GENERATION_OFFSET"
  | .quantity => "QUANTITY_OFFSET"
  | .basisScale => "BASIS_SCALE_OFFSET"
  | .collateralAtoms => "COLLATERAL_ATOMS_OFFSET"
  | .expectedMarketRevision => "EXPECTED_MARKET_REVISION_OFFSET"
  | .expectedPositionRevision => "EXPECTED_POSITION_REVISION_OFFSET"
  | .expectedCustodyRevision => "EXPECTED_CUSTODY_REVISION_OFFSET"
  | .preExternalAmount => "PRE_EXTERNAL_AMOUNT_OFFSET"
  | .postExternalAmount => "POST_EXTERNAL_AMOUNT_OFFSET"
  | .preHoardAmount => "PRE_HOARD_AMOUNT_OFFSET"
  | .postHoardAmount => "POST_HOARD_AMOUNT_OFFSET"
  | .claimCount => "CLAIM_COUNT_OFFSET"
  | .reservedTail => "TAIL_RESERVED_OFFSET"

def coordinate (field : RequestField) : Nat × Nat :=
  (coordinate? field requestLayout).getD (0, 0)

def offset (field : RequestField) : Nat := (coordinate field).1
def width (field : RequestField) : Nat := (coordinate field).2

end RequestField

/-! ## What the layout says -/

theorem request_schema_well_formed : WellFormed requestSchema := by
  constructor
  · native_decide
  · intro field member
    simp [requestSchema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem request_layout_disjoint : requestLayout.Pairwise Before :=
  specializeFrom_pairwise 0 requestSchema

/-- The fields tile the 592 bytes the crate allocated: no gap, and the last
field ends exactly at the declared width. -/
theorem request_layout_covers_its_width :
    requestBytes = 592 ∧ tiles 0 requestLayout 592 = true := by
  native_decide

/-- Every coordinate the Rust module wrote as a decimal literal. -/
theorem request_coordinates_are_canonical : coordinates requestLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.direction, 10, 1), (.reservedHeader, 11, 5),
    (.realm, 16, 32), (.market, 48, 32), (.releaseSet, 80, 32),
    (.custodyContext, 112, 32), (.aggregate, 144, 32), (.position, 176, 32),
    (.owner, 208, 32), (.externalCollateral, 240, 32), (.hoardVault, 272, 32),
    (.mint, 304, 32), (.tokenProgram, 336, 32), (.claimsProgram, 368, 32),
    (.productRecordDigest, 400, 32), (.linkedBasisRecordDigest, 432, 32),
    (.semanticBasisId, 464, 32),
    (.generation, 496, 8), (.quantity, 504, 8), (.basisScale, 512, 8),
    (.collateralAtoms, 520, 8),
    (.expectedMarketRevision, 528, 8), (.expectedPositionRevision, 536, 8),
    (.expectedCustodyRevision, 544, 8),
    (.preExternalAmount, 552, 8), (.postExternalAmount, 560, 8),
    (.preHoardAmount, 568, 8), (.postHoardAmount, 576, 8),
    (.claimCount, 584, 4), (.reservedTail, 588, 4)
  ] := by
  native_decide

/-- The prologue is the tree's: magic at 0, version at 8, the selector at 10,
which is where `ClaimsLiabilityBasisStateV2Abi` and `MarketRetirementV1Abi` put
theirs. -/
theorem request_begins_with_the_shared_prologue :
    RequestField.offset .magic = 0 ∧ RequestField.offset .version = 8 ∧
    RequestField.offset .direction = 10 := by
  native_decide

theorem magic_is_eight_bytes : requestMagic.length = 8 := by native_decide

theorem direction_tags_differ : splitTag ≠ mergeTag := by native_decide

end DClutch.ClaimsConservationV1Abi
