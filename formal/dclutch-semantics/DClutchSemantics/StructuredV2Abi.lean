import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.StructuredV2
import Std.Tactic

/-!
# Structured V2 fixed physical ABI

This module is the sole owner of every Structured V2 byte coordinate.  Offsets
are never written down: `DClutch.AbiSchema.specialize` places fields
left-to-right, so overlap and drift are structurally impossible, and the
regression witnesses below compare the derived layout against the intended
public ABI rather than restating it.

Four records exist and no more:

* immutable finalized **terms** — the Market, Product, release, Token, shard
  layer, receipt Mint, width and the `K` exact backing coefficients;
* Structured-owned persisted **root** — replay revision and permanent rent
  beneficiary only, with no supply, coefficient, payout, or phase mirror;
* adapter-owned runtime **projection** — the observed lifecycle phase, receipt
  supply, and per-coordinate observed shard custody and authenticated payout;
* the family **request** — one action, its identities, and one quantity.

The lifecycle phase is deliberately absent from the root.  It is authenticated
per transaction from the Market and Product terminal record, so persisting it
would create a second owner for a fact Core already owns.

The checked arithmetic profile uses `u64` scalars and at most 256 representation
coordinates.  That is an executable capacity profile, not a restriction on the
protocol ontology.
-/

namespace DClutch.StructuredV2Abi

open DClutch.AbiSchema

/-! ## Profile constants and identities -/

def schemaVersion : Nat := 2
def receiptDecimals : Nat := 0
def minCoordinates : Nat := 1
def maxCoordinates : Nat := 256
def minDenominator : Nat := 2
def maxU64 : Nat := 2 ^ 64 - 1

/-- Canonical absent optimistic coordinate. -/
def noCoordinate : Nat := 2 ^ 32 - 1

def termsMagic : List UInt8 := "DCSTTRM2".toUTF8.toList
def rootMagic : List UInt8 := "DCSTRT02".toUTF8.toList
def projectionMagic : List UInt8 := "DCSTPRJ2".toUTF8.toList
def requestMagic : List UInt8 := "DCSTREQ2".toUTF8.toList

def termsSchemaPreimage : List UInt8 :=
  "dclutch/schema/structured-receipt-terms-v2|header384|K-coefficients8|shard-backed|exact-custody-equals-supply-times-coefficient|no-remainder-ledger".toUTF8.toList
def termsSchemaId : List UInt8 := [
  0x60, 0x3f, 0x5e, 0x2a, 0xc4, 0xbd, 0x21, 0x73,
  0x2e, 0x46, 0xd2, 0x87, 0x13, 0x55, 0x4b, 0xfc,
  0xdc, 0x4f, 0x1b, 0x42, 0xc8, 0x0b, 0x72, 0x2c,
  0xe8, 0x01, 0xcc, 0x20, 0x3e, 0x55, 0x9f, 0xb2
]

def rootSchemaPreimage : List UInt8 :=
  "dclutch/schema/structured-receipt-root-v2|bytes128|replay-and-rent-only|no-supply-mirror".toUTF8.toList
def rootSchemaId : List UInt8 := [
  0x0f, 0x0e, 0xde, 0x26, 0x3c, 0x5f, 0x88, 0x09,
  0xe7, 0xcd, 0x0f, 0x21, 0xcf, 0x09, 0x51, 0xfb,
  0x9f, 0x7e, 0xef, 0x9c, 0xa8, 0xe6, 0x81, 0x08,
  0x5f, 0x0c, 0x9c, 0x6c, 0x3c, 0x62, 0xd0, 0x46
]

def requestSchemaPreimage : List UInt8 :=
  "dclutch/schema/structured-receipt-request-v2|bytes432|terms-select-shard-layer|no-payout-input|no-coefficient-input".toUTF8.toList
def requestSchemaId : List UInt8 := [
  0x9e, 0x73, 0x67, 0x03, 0x4c, 0x09, 0x63, 0xb1,
  0xcf, 0xf9, 0x55, 0x99, 0x27, 0xc6, 0xc3, 0x41,
  0x98, 0x3c, 0xf0, 0xa8, 0x0d, 0x9b, 0x74, 0xbe,
  0x8e, 0x78, 0x02, 0x85, 0xfa, 0xd6, 0xc2, 0x24
]

def capabilityKindPreimage : List UInt8 :=
  "dclutch/capability-kind/structured-receipt-v2|depth2-representation-dag|shard-backed-receipt".toUTF8.toList
def capabilityKindId : List UInt8 := [
  0x87, 0x47, 0xf7, 0x21, 0x96, 0xdf, 0x2c, 0x4f,
  0xbf, 0xf1, 0x8d, 0xf7, 0x2d, 0x9a, 0xd8, 0x51,
  0xe0, 0x34, 0x2d, 0x29, 0x9c, 0x31, 0x46, 0x31,
  0x4d, 0x38, 0x66, 0x22, 0x03, 0xae, 0x8a, 0xed
]

def capacityProfilePreimage : List UInt8 :=
  "dclutch/capacity/structured-receipt-v2/coordinates256/u64".toUTF8.toList
def capacityProfileId : List UInt8 := [
  0xca, 0x0c, 0xf9, 0x53, 0x80, 0x43, 0x70, 0x09,
  0xcf, 0x76, 0x18, 0x99, 0x96, 0xf3, 0x69, 0x50,
  0xa2, 0x50, 0xa5, 0xb4, 0x9b, 0x93, 0x34, 0x87,
  0x88, 0xb3, 0xb6, 0x1a, 0x4f, 0xc5, 0xd3, 0x68
]

/-! ## Lifecycle actions and phase tags -/

/-- Exactly the four Structured V2 actions.  There is no Terminalize action
because the lifecycle phase has no Structured-owned persisted copy. -/
inductive Action where
  /-- Lock the exact shard basket and mint receipt atoms. -/
  | issue
  /-- Burn receipt atoms and release the exact shard basket. -/
  | unwrap
  /-- Burn receipt atoms after terminal resolution and settle exactly. -/
  | terminalRedeem
  /-- Close a zero-supply, zero-custody node and recover rent. -/
  | zeroSupplyRetire
  deriving DecidableEq, Repr

/-- Stable wire discriminator. -/
def Action.tag : Action → Nat
  | .issue => 0
  | .unwrap => 1
  | .terminalRedeem => 2
  | .zeroSupplyRetire => 3

/-- Every admitted action, in wire order. -/
def actions : List Action := [.issue, .unwrap, .terminalRedeem, .zeroSupplyRetire]

/-- Whether the action carries a positive receipt quantity. -/
def Action.carriesQuantity : Action → Bool
  | .issue | .unwrap | .terminalRedeem => true
  | .zeroSupplyRetire => false

/-- Whether the action requires an authenticated terminal record digest. -/
def Action.requiresTerminal : Action → Bool
  | .terminalRedeem | .zeroSupplyRetire => true
  | .issue | .unwrap => false

theorem action_tags_are_unique : (actions.map Action.tag).Nodup := by decide

theorem action_tags_are_contiguous :
    actions.map Action.tag = List.range actions.length := by decide

/-- Wire tag of the authenticated lifecycle phase. -/
def phaseTag : DClutch.StructuredV2.Phase → Nat
  | .open => 0
  | .terminal _ => 1
  | .retired => 2

/-! ## Immutable finalized terms -/

inductive TermsField where
  | magic | version | receiptDecimals | reservedHeader | market | productRecord
  | resultDomain | releaseSet | tokenProgram | tokenBehavior | shardTerms
  | shardExposure | receiptMint | graphId | representationWidth | reservedWidth
  | denominator | reservedTail
  deriving DecidableEq, Repr

def termsSchema : List (FieldSpec TermsField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.receiptDecimals, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.market, .bytes 32⟩,
  ⟨.productRecord, .bytes 32⟩, ⟨.resultDomain, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.tokenProgram, .bytes 32⟩,
  ⟨.tokenBehavior, .bytes 32⟩, ⟨.shardTerms, .bytes 32⟩,
  ⟨.shardExposure, .bytes 32⟩, ⟨.receiptMint, .bytes 32⟩,
  ⟨.graphId, .bytes 32⟩, ⟨.representationWidth, .u32⟩,
  ⟨.reservedWidth, .reserved 4⟩, ⟨.denominator, .u64⟩,
  ⟨.reservedTail, .reserved 32⟩
]

def termsLayout : List (PlacedField TermsField) := specialize termsSchema

def termsHeaderBytes : Nat := schemaWidth termsSchema

/-- One exact backing coefficient `c_i` in coordinate order. -/
def termsCoefficientBytes : Nat := 8

/-- Exact encoded terms width for `K` coordinates. -/
def termsBytes (representationWidth : Nat) : Nat :=
  termsHeaderBytes + representationWidth * termsCoefficientBytes

/-! ## Structured-owned persisted root -/

inductive RootField where
  | magic | version | bump | reservedHeader | terms | market | rentBeneficiary
  | revision | historicalRentPrincipal
  deriving DecidableEq, Repr

def rootSchema : List (FieldSpec RootField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.bump, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.terms, .bytes 32⟩,
  ⟨.market, .bytes 32⟩, ⟨.rentBeneficiary, .bytes 32⟩,
  ⟨.revision, .u64⟩, ⟨.historicalRentPrincipal, .u64⟩
]

def rootLayout : List (PlacedField RootField) := specialize rootSchema

def rootBytes : Nat := schemaWidth rootSchema

/-! ## Adapter-owned runtime projection -/

inductive ProjectionField where
  | magic | version | phase | reservedHeader | termsId | market | shardTerms
  | representationWidth | reservedWidth | denominator | receiptSupply | revision
  | reservedTail
  deriving DecidableEq, Repr

def projectionSchema : List (FieldSpec ProjectionField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.termsId, .bytes 32⟩,
  ⟨.market, .bytes 32⟩, ⟨.shardTerms, .bytes 32⟩,
  ⟨.representationWidth, .u32⟩, ⟨.reservedWidth, .reserved 4⟩,
  ⟨.denominator, .u64⟩, ⟨.receiptSupply, .u64⟩, ⟨.revision, .u64⟩,
  ⟨.reservedTail, .reserved 16⟩
]

def projectionLayout : List (PlacedField ProjectionField) := specialize projectionSchema

def projectionHeaderBytes : Nat := schemaWidth projectionSchema

inductive ProjectionRowField where
  | observedShardCustody | payoutPerClaim
  deriving DecidableEq, Repr

def projectionRowSchema : List (FieldSpec ProjectionRowField) := [
  ⟨.observedShardCustody, .u64⟩, ⟨.payoutPerClaim, .u64⟩
]

def projectionRowLayout : List (PlacedField ProjectionRowField) :=
  specialize projectionRowSchema

def projectionRowBytes : Nat := schemaWidth projectionRowSchema

/-- Exact encoded projection width for `K` coordinates. -/
def projectionBytes (representationWidth : Nat) : Nat :=
  projectionHeaderBytes + representationWidth * projectionRowBytes

/-! ## Family request -/

inductive RequestField where
  | magic | version | action | reservedHeader | releaseSet | market
  | productRecord | resultDomain | terms | tokenBehavior | shardTerms
  | shardExposure | owner | receiptSource | receiptDestination | terminalDigest
  | expectedRevision | quantity | reservedTail
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.market, .bytes 32⟩, ⟨.productRecord, .bytes 32⟩,
  ⟨.resultDomain, .bytes 32⟩, ⟨.terms, .bytes 32⟩,
  ⟨.tokenBehavior, .bytes 32⟩, ⟨.shardTerms, .bytes 32⟩,
  ⟨.shardExposure, .bytes 32⟩, ⟨.owner, .bytes 32⟩,
  ⟨.receiptSource, .bytes 32⟩, ⟨.receiptDestination, .bytes 32⟩,
  ⟨.terminalDigest, .bytes 32⟩, ⟨.expectedRevision, .u64⟩,
  ⟨.quantity, .u64⟩, ⟨.reservedTail, .reserved 16⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema

def requestBytes : Nat := schemaWidth requestSchema

/-! ## Layout regression witnesses -/

theorem termsHeaderBytes_is_384 : termsHeaderBytes = 384 := by decide
theorem rootBytes_is_128 : rootBytes = 128 := by decide
theorem projectionHeaderBytes_is_160 : projectionHeaderBytes = 160 := by decide
theorem projectionRowBytes_is_16 : projectionRowBytes = 16 := by decide
theorem requestBytes_is_432 : requestBytes = 432 := by decide

theorem termsSchema_wellFormed : WellFormed termsSchema := by
  constructor
  · decide
  · intro field member
    simp only [termsSchema, List.mem_cons, List.not_mem_nil, or_false] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem rootSchema_wellFormed : WellFormed rootSchema := by
  constructor
  · decide
  · intro field member
    simp only [rootSchema, List.mem_cons, List.not_mem_nil, or_false] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem projectionSchema_wellFormed : WellFormed projectionSchema := by
  constructor
  · decide
  · intro field member
    simp only [projectionSchema, List.mem_cons, List.not_mem_nil, or_false] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl <;> decide

theorem projectionRowSchema_wellFormed : WellFormed projectionRowSchema := by
  constructor
  · decide
  · intro field member
    simp only [projectionRowSchema, List.mem_cons, List.not_mem_nil, or_false] at member
    rcases member with rfl | rfl <;> decide

theorem requestSchema_wellFormed : WellFormed requestSchema := by
  constructor
  · decide
  · intro field member
    simp only [requestSchema, List.mem_cons, List.not_mem_nil, or_false] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem termsFields_disjoint : termsLayout.Pairwise Before :=
  specializeFrom_pairwise 0 termsSchema

theorem rootFields_disjoint : rootLayout.Pairwise Before :=
  specializeFrom_pairwise 0 rootSchema

theorem projectionFields_disjoint : projectionLayout.Pairwise Before :=
  specializeFrom_pairwise 0 projectionSchema

theorem projectionRowFields_disjoint : projectionRowLayout.Pairwise Before :=
  specializeFrom_pairwise 0 projectionRowSchema

theorem requestFields_disjoint : requestLayout.Pairwise Before :=
  specializeFrom_pairwise 0 requestSchema

theorem termsFields_bounded (placed : PlacedField TermsField)
    (member : placed ∈ termsLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ termsHeaderBytes :=
  specializeFrom_bounded 0 termsSchema placed member

theorem rootFields_bounded (placed : PlacedField RootField)
    (member : placed ∈ rootLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ rootBytes :=
  specializeFrom_bounded 0 rootSchema placed member

theorem projectionFields_bounded (placed : PlacedField ProjectionField)
    (member : placed ∈ projectionLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ projectionHeaderBytes :=
  specializeFrom_bounded 0 projectionSchema placed member

theorem requestFields_bounded (placed : PlacedField RequestField)
    (member : placed ∈ requestLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ requestBytes :=
  specializeFrom_bounded 0 requestSchema placed member

/-- Regression witness for the intended public terms ABI. -/
theorem termsCoordinates : coordinates termsLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.receiptDecimals, 10, 1),
    (.reservedHeader, 11, 5), (.market, 16, 32), (.productRecord, 48, 32),
    (.resultDomain, 80, 32), (.releaseSet, 112, 32), (.tokenProgram, 144, 32),
    (.tokenBehavior, 176, 32), (.shardTerms, 208, 32), (.shardExposure, 240, 32),
    (.receiptMint, 272, 32), (.graphId, 304, 32), (.representationWidth, 336, 4),
    (.reservedWidth, 340, 4), (.denominator, 344, 8), (.reservedTail, 352, 32)
  ] := by decide

/-- Regression witness for the intended public root ABI. -/
theorem rootCoordinates : coordinates rootLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.bump, 10, 1), (.reservedHeader, 11, 5),
    (.terms, 16, 32), (.market, 48, 32), (.rentBeneficiary, 80, 32),
    (.revision, 112, 8), (.historicalRentPrincipal, 120, 8)
  ] := by decide

/-- Regression witness for the intended public projection ABI. -/
theorem projectionCoordinates : coordinates projectionLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.phase, 10, 1), (.reservedHeader, 11, 5),
    (.termsId, 16, 32), (.market, 48, 32), (.shardTerms, 80, 32),
    (.representationWidth, 112, 4), (.reservedWidth, 116, 4),
    (.denominator, 120, 8), (.receiptSupply, 128, 8), (.revision, 136, 8),
    (.reservedTail, 144, 16)
  ] := by decide

/-- Regression witness for the intended public projection row ABI. -/
theorem projectionRowCoordinates : coordinates projectionRowLayout = [
    (.observedShardCustody, 0, 8), (.payoutPerClaim, 8, 8)
  ] := by decide

/-- Regression witness for the intended public request ABI. -/
theorem requestCoordinates : coordinates requestLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1), (.reservedHeader, 11, 5),
    (.releaseSet, 16, 32), (.market, 48, 32), (.productRecord, 80, 32),
    (.resultDomain, 112, 32), (.terms, 144, 32), (.tokenBehavior, 176, 32),
    (.shardTerms, 208, 32), (.shardExposure, 240, 32), (.owner, 272, 32),
    (.receiptSource, 304, 32), (.receiptDestination, 336, 32),
    (.terminalDigest, 368, 32), (.expectedRevision, 400, 8), (.quantity, 408, 8),
    (.reservedTail, 416, 16)
  ] := by decide

/-! ## Width arithmetic -/

theorem terms_width_positive (representationWidth : Nat) :
    0 < termsBytes representationWidth := by
  unfold termsBytes
  rw [termsHeaderBytes_is_384]
  omega

theorem projection_width_positive (representationWidth : Nat) :
    0 < projectionBytes representationWidth := by
  unfold projectionBytes
  rw [projectionHeaderBytes_is_160]
  omega

theorem terms_width_is_strictly_monotone (left right : Nat) (smaller : left < right) :
    termsBytes left < termsBytes right := by
  unfold termsBytes termsCoefficientBytes
  omega

theorem projection_width_is_strictly_monotone (left right : Nat) (smaller : left < right) :
    projectionBytes left < projectionBytes right := by
  unfold projectionBytes
  rw [projectionRowBytes_is_16]
  omega

/-- Both runtime-width records stay well inside the current account profile at
the maximum admitted coordinate count. -/
theorem maximum_widths :
    termsBytes maxCoordinates = 2432 ∧ projectionBytes maxCoordinates = 4256 := by
  decide

/-! ## Rust name projections used only by the generator -/

def TermsField.rustName : TermsField → String
  | .magic => "STRUCTURED_TERMS_MAGIC_OFFSET_V2"
  | .version => "STRUCTURED_TERMS_VERSION_OFFSET_V2"
  | .receiptDecimals => "STRUCTURED_TERMS_RECEIPT_DECIMALS_OFFSET_V2"
  | .reservedHeader => "STRUCTURED_TERMS_RESERVED_HEADER_OFFSET_V2"
  | .market => "STRUCTURED_TERMS_MARKET_OFFSET_V2"
  | .productRecord => "STRUCTURED_TERMS_PRODUCT_RECORD_OFFSET_V2"
  | .resultDomain => "STRUCTURED_TERMS_RESULT_DOMAIN_OFFSET_V2"
  | .releaseSet => "STRUCTURED_TERMS_RELEASE_SET_OFFSET_V2"
  | .tokenProgram => "STRUCTURED_TERMS_TOKEN_PROGRAM_OFFSET_V2"
  | .tokenBehavior => "STRUCTURED_TERMS_TOKEN_BEHAVIOR_OFFSET_V2"
  | .shardTerms => "STRUCTURED_TERMS_SHARD_TERMS_OFFSET_V2"
  | .shardExposure => "STRUCTURED_TERMS_SHARD_EXPOSURE_OFFSET_V2"
  | .receiptMint => "STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2"
  | .graphId => "STRUCTURED_TERMS_GRAPH_ID_OFFSET_V2"
  | .representationWidth => "STRUCTURED_TERMS_REPRESENTATION_WIDTH_OFFSET_V2"
  | .reservedWidth => "STRUCTURED_TERMS_RESERVED_WIDTH_OFFSET_V2"
  | .denominator => "STRUCTURED_TERMS_DENOMINATOR_OFFSET_V2"
  | .reservedTail => "STRUCTURED_TERMS_RESERVED_TAIL_OFFSET_V2"

def RootField.rustName : RootField → String
  | .magic => "STRUCTURED_ROOT_MAGIC_OFFSET_V2"
  | .version => "STRUCTURED_ROOT_VERSION_OFFSET_V2"
  | .bump => "STRUCTURED_ROOT_BUMP_OFFSET_V2"
  | .reservedHeader => "STRUCTURED_ROOT_RESERVED_HEADER_OFFSET_V2"
  | .terms => "STRUCTURED_ROOT_TERMS_OFFSET_V2"
  | .market => "STRUCTURED_ROOT_MARKET_OFFSET_V2"
  | .rentBeneficiary => "STRUCTURED_ROOT_RENT_BENEFICIARY_OFFSET_V2"
  | .revision => "STRUCTURED_ROOT_REVISION_OFFSET_V2"
  | .historicalRentPrincipal => "STRUCTURED_ROOT_RENT_PRINCIPAL_OFFSET_V2"

def ProjectionField.rustName : ProjectionField → String
  | .magic => "STRUCTURED_PROJECTION_MAGIC_OFFSET_V2"
  | .version => "STRUCTURED_PROJECTION_VERSION_OFFSET_V2"
  | .phase => "STRUCTURED_PROJECTION_PHASE_OFFSET_V2"
  | .reservedHeader => "STRUCTURED_PROJECTION_RESERVED_HEADER_OFFSET_V2"
  | .termsId => "STRUCTURED_PROJECTION_TERMS_OFFSET_V2"
  | .market => "STRUCTURED_PROJECTION_MARKET_OFFSET_V2"
  | .shardTerms => "STRUCTURED_PROJECTION_SHARD_TERMS_OFFSET_V2"
  | .representationWidth => "STRUCTURED_PROJECTION_REPRESENTATION_WIDTH_OFFSET_V2"
  | .reservedWidth => "STRUCTURED_PROJECTION_RESERVED_WIDTH_OFFSET_V2"
  | .denominator => "STRUCTURED_PROJECTION_DENOMINATOR_OFFSET_V2"
  | .receiptSupply => "STRUCTURED_PROJECTION_RECEIPT_SUPPLY_OFFSET_V2"
  | .revision => "STRUCTURED_PROJECTION_REVISION_OFFSET_V2"
  | .reservedTail => "STRUCTURED_PROJECTION_RESERVED_TAIL_OFFSET_V2"

def ProjectionRowField.rustName : ProjectionRowField → String
  | .observedShardCustody => "STRUCTURED_PROJECTION_ROW_CUSTODY_OFFSET_V2"
  | .payoutPerClaim => "STRUCTURED_PROJECTION_ROW_PAYOUT_OFFSET_V2"

def RequestField.rustName : RequestField → String
  | .magic => "STRUCTURED_REQUEST_MAGIC_OFFSET_V2"
  | .version => "STRUCTURED_REQUEST_VERSION_OFFSET_V2"
  | .action => "STRUCTURED_REQUEST_ACTION_OFFSET_V2"
  | .reservedHeader => "STRUCTURED_REQUEST_RESERVED_HEADER_OFFSET_V2"
  | .releaseSet => "STRUCTURED_REQUEST_RELEASE_SET_OFFSET_V2"
  | .market => "STRUCTURED_REQUEST_MARKET_OFFSET_V2"
  | .productRecord => "STRUCTURED_REQUEST_PRODUCT_RECORD_OFFSET_V2"
  | .resultDomain => "STRUCTURED_REQUEST_RESULT_DOMAIN_OFFSET_V2"
  | .terms => "STRUCTURED_REQUEST_TERMS_OFFSET_V2"
  | .tokenBehavior => "STRUCTURED_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2"
  | .shardTerms => "STRUCTURED_REQUEST_SHARD_TERMS_OFFSET_V2"
  | .shardExposure => "STRUCTURED_REQUEST_SHARD_EXPOSURE_OFFSET_V2"
  | .owner => "STRUCTURED_REQUEST_OWNER_OFFSET_V2"
  | .receiptSource => "STRUCTURED_REQUEST_RECEIPT_SOURCE_OFFSET_V2"
  | .receiptDestination => "STRUCTURED_REQUEST_RECEIPT_DESTINATION_OFFSET_V2"
  | .terminalDigest => "STRUCTURED_REQUEST_TERMINAL_DIGEST_OFFSET_V2"
  | .expectedRevision => "STRUCTURED_REQUEST_EXPECTED_REVISION_OFFSET_V2"
  | .quantity => "STRUCTURED_REQUEST_QUANTITY_OFFSET_V2"
  | .reservedTail => "STRUCTURED_REQUEST_RESERVED_TAIL_OFFSET_V2"

def Action.rustName : Action → String
  | .issue => "STRUCTURED_ACTION_ISSUE_V2"
  | .unwrap => "STRUCTURED_ACTION_UNWRAP_V2"
  | .terminalRedeem => "STRUCTURED_ACTION_TERMINAL_REDEEM_V2"
  | .zeroSupplyRetire => "STRUCTURED_ACTION_ZERO_SUPPLY_RETIRE_V2"

end DClutch.StructuredV2Abi
