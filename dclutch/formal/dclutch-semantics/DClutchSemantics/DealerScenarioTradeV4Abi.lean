import DClutchSemantics.AbiCoverage

/-!
# Selector-9 Dealer scenario trade request header

The one exact-fill action in the global Dealer selector space.  A signed family
request carries the complete portfolio and quote intent in a fixed 392-byte
header, followed by a runtime `u64[width]` candidate-obligation vector and a
SignedDeltaV3 Claims witness.

Until this module existed the header had no Lean owner at all.  The three
Lean-emitted Dealer artifacts are the liquidity ABI, the trading profile and the
netting corpus; this record was authored by
`programs/dclutch-trading-sbf/src/dealer/v3_trade.rs`, and only FIVE of its
twenty-eight coordinates had names there.  The other twenty-three were bare
decimal literals written TWICE -- once in `encode_scenario_trade_request_v3` and
once in `decode` -- so `market` was `48` in the encoder and `48` in the decoder
and nothing in the repository related the two numbers to each other.  That is
the shape a same-width transposition hides in: swap the two `bytes 32` identity
fields at 48 and 80 in both places and every round trip still passes, because
the encoder and the decoder agree with each other about a layout that no longer
matches the chain's.

Three further facts the Rust stated in prose and could not check:

* The two bytes at 390 are not spare room.  They exist because the candidate
  vector that begins at the header's end is a `u64[]` and the six route-span
  counts end at 390, which is not an eight-byte boundary.
  `the_pad_is_what_aligns_the_candidate_vector` is that sentence.
* The CapabilityProgramSet selector offset is not an independent number.  The
  selector IS this header's first twelve bytes and `select` reads the `u16` the
  header calls `action`, so `10` was written three times: once as
  `DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3`, once as the encoder's bare
  `write_bytes(output, 10, ...)`, and once as the bare slice index in
  `selector[10..12]`.
* The header grew from 384 to 392 by APPENDING.  The six route-span counts
  begin exactly where the superseded header ended, which is why every V3
  coordinate below 384 is unmoved and why the version had to move anyway.
-/

namespace DClutch.DealerScenarioTradeV4Abi

open DClutch.AbiSchema

/-- Implemented request version.  Five and not four: `f5d4912e` grew the header
by eight bytes, so a request an older encoder built is a different shape and
must refuse at `decode` rather than be read with its fields out of place. -/
def version : Nat := 5

/-- The sole exact-fill action in the global Dealer selector space. -/
def action : Nat := 9

/-- `DCLDST03`. -/
def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x44, 0x53, 0x54, 0x30, 0x33]

/-- Exact number of optional Custody routes selector 9 can enable.  Each is a
whole Custody transfer frame or nothing at all. -/
def routeSpanCount : Nat := 6

/-- One enabled optional Custody route's exact account width, as a request
byte.  DECLARED here rather than derived from the Custody frame on purpose: the
Rust keeps its own `const _: () = assert!` against
`DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3`, and two independent authorities that
must agree is the point.  Deriving this would turn that assertion into a
tautology and take away its ability to go red. -/
def routeSpanWidth : Nat := 14

/-- One candidate obligation in the runtime tail: a little-endian `u64`. -/
def candidateStride : Nat := 8

/-- Fixed identity width shared by every address and digest coordinate. -/
def identityBytes : Nat := 32

/-- The header this one supersedes, and the version it carried.  Named so the
reason the version moved lives in the same object as the width that moved. -/
def supersededHeaderBytes : Nat := 384
def supersededVersion : Nat := 4

/-! ## The header -/

inductive Field where
  | magic | version | action | obligationWidth
  | releaseSet | market | childRoot | obligationAddress
  | currentObligationDigest | candidateObligationDigest
  | dealerOwner | counterpartyOwner | counterpartyAccount
  | currentObligationRevision | candidateObligationRevision
  | dealerPositionRevision | counterpartyPositionRevision
  | claimsRevision | generation | expiresAt | principal | realizedFee
  | direction | claimsPositionCount | dealerEvidenceCount
  | evidenceSpanCount | claimsPacketBytes | routeSpanCounts | reserved
  deriving DecidableEq, Repr

/-- The first three fields, which are also exactly the selector prefix the
CapabilityProgramSet dispatches on. -/
def selectorPrefix : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u16⟩
]

/-- The nine authenticated identities, in wire order. -/
def identities : List (FieldSpec Field) := [
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.childRoot, .bytes 32⟩,
  ⟨.obligationAddress, .bytes 32⟩, ⟨.currentObligationDigest, .bytes 32⟩,
  ⟨.candidateObligationDigest, .bytes 32⟩, ⟨.dealerOwner, .bytes 32⟩,
  ⟨.counterpartyOwner, .bytes 32⟩, ⟨.counterpartyAccount, .bytes 32⟩
]

/-- The nine copied-from-chain scalars, in wire order. -/
def scalars : List (FieldSpec Field) := [
  ⟨.currentObligationRevision, .u64⟩, ⟨.candidateObligationRevision, .u64⟩,
  ⟨.dealerPositionRevision, .u64⟩, ⟨.counterpartyPositionRevision, .u64⟩,
  ⟨.claimsRevision, .u64⟩, ⟨.generation, .u64⟩, ⟨.expiresAt, .u64⟩,
  ⟨.principal, .u64⟩, ⟨.realizedFee, .u64⟩
]

/-- The four coordinates the executor reads before it can size the request at
all, plus the direction byte they follow. -/
def routing : List (FieldSpec Field) := [
  ⟨.direction, .u8⟩, ⟨.claimsPositionCount, .u8⟩,
  ⟨.dealerEvidenceCount, .u8⟩, ⟨.evidenceSpanCount, .u8⟩,
  ⟨.claimsPacketBytes, .u32⟩
]

/-- The declared frame shape, and the pad that keeps the candidate vector
aligned.  A header with a free byte is a header with an unauthenticated one, so
the pad is `reserved` and canonically zero rather than opaque `bytes`. -/
def frameShape : List (FieldSpec Field) := [
  ⟨.routeSpanCounts, .bytes routeSpanCount⟩, ⟨.reserved, .reserved 2⟩
]

def schema : List (FieldSpec Field) :=
  selectorPrefix ++ [⟨.obligationWidth, .u32⟩] ++ identities ++ scalars ++
    routing ++ frameShape

def layout : List (PlacedField Field) := specialize schema
def headerBytes : Nat := schemaWidth schema

/-- The runtime candidate-obligation vector begins exactly at the header's
end. -/
def obligationsOffset : Nat := headerBytes

namespace Field

def all : List Field := [
  .magic, .version, .action, .obligationWidth,
  .releaseSet, .market, .childRoot, .obligationAddress,
  .currentObligationDigest, .candidateObligationDigest,
  .dealerOwner, .counterpartyOwner, .counterpartyAccount,
  .currentObligationRevision, .candidateObligationRevision,
  .dealerPositionRevision, .counterpartyPositionRevision,
  .claimsRevision, .generation, .expiresAt, .principal, .realizedFee,
  .direction, .claimsPositionCount, .dealerEvidenceCount,
  .evidenceSpanCount, .claimsPacketBytes, .routeSpanCounts, .reserved
]

/-- The Rust constant naming each coordinate.  The five names that already
existed are preserved exactly -- nothing here moves, so nothing here is
renamed -- and the twenty-three bare literals get names for the first time. -/
def rustName : Field → String
  | .magic => "DEALER_SCENARIO_TRADE_MAGIC_OFFSET_V4"
  | .version => "DEALER_SCENARIO_TRADE_VERSION_OFFSET_V4"
  | .action => "DEALER_SCENARIO_TRADE_ACTION_OFFSET_V4"
  | .obligationWidth => "DEALER_SCENARIO_TRADE_WIDTH_OFFSET_V4"
  | .releaseSet => "DEALER_SCENARIO_TRADE_RELEASE_SET_OFFSET_V4"
  | .market => "DEALER_SCENARIO_TRADE_MARKET_OFFSET_V4"
  | .childRoot => "DEALER_SCENARIO_TRADE_CHILD_ROOT_OFFSET_V4"
  | .obligationAddress => "DEALER_SCENARIO_TRADE_OBLIGATION_ADDRESS_OFFSET_V4"
  | .currentObligationDigest =>
      "DEALER_SCENARIO_TRADE_CURRENT_OBLIGATION_DIGEST_OFFSET_V4"
  | .candidateObligationDigest =>
      "DEALER_SCENARIO_TRADE_CANDIDATE_OBLIGATION_DIGEST_OFFSET_V4"
  | .dealerOwner => "DEALER_SCENARIO_TRADE_DEALER_OWNER_OFFSET_V4"
  | .counterpartyOwner => "DEALER_SCENARIO_TRADE_COUNTERPARTY_OWNER_OFFSET_V4"
  | .counterpartyAccount =>
      "DEALER_SCENARIO_TRADE_COUNTERPARTY_ACCOUNT_OFFSET_V4"
  | .currentObligationRevision =>
      "DEALER_SCENARIO_TRADE_CURRENT_OBLIGATION_REVISION_OFFSET_V4"
  | .candidateObligationRevision =>
      "DEALER_SCENARIO_TRADE_CANDIDATE_OBLIGATION_REVISION_OFFSET_V4"
  | .dealerPositionRevision =>
      "DEALER_SCENARIO_TRADE_DEALER_POSITION_REVISION_OFFSET_V4"
  | .counterpartyPositionRevision =>
      "DEALER_SCENARIO_TRADE_COUNTERPARTY_POSITION_REVISION_OFFSET_V4"
  | .claimsRevision => "DEALER_SCENARIO_TRADE_CLAIMS_REVISION_OFFSET_V4"
  | .generation => "DEALER_SCENARIO_TRADE_GENERATION_OFFSET_V4"
  | .expiresAt => "DEALER_SCENARIO_TRADE_EXPIRES_AT_OFFSET_V4"
  | .principal => "DEALER_SCENARIO_TRADE_PRINCIPAL_OFFSET_V4"
  | .realizedFee => "DEALER_SCENARIO_TRADE_REALIZED_FEE_OFFSET_V4"
  | .direction => "DEALER_SCENARIO_TRADE_DIRECTION_OFFSET_V4"
  | .claimsPositionCount => "DEALER_SCENARIO_TRADE_POSITION_COUNT_OFFSET_V3"
  | .dealerEvidenceCount =>
      "DEALER_SCENARIO_TRADE_DEALER_EVIDENCE_COUNT_OFFSET_V3"
  | .evidenceSpanCount => "DEALER_SCENARIO_TRADE_EVIDENCE_SPAN_COUNT_OFFSET_V3"
  | .claimsPacketBytes =>
      "DEALER_SCENARIO_TRADE_CLAIMS_PACKET_BYTES_OFFSET_V3"
  | .routeSpanCounts => "DEALER_SCENARIO_TRADE_ROUTE_SPAN_COUNTS_OFFSET_V4"
  | .reserved => "DEALER_SCENARIO_TRADE_RESERVED_OFFSET_V4"

/-- One line of Rust doc per coordinate.  The header is signed and every field
is copied from authenticated chain state, so what a reader needs at each
coordinate is which authority it came from. -/
def doc : Field → String
  | .magic => "Canonical exact-fill request magic."
  | .version => "Implemented request version."
  | .action =>
      "Family selector: the `u16` the CapabilityProgramSet dispatches on."
  | .obligationWidth => "Runtime Product width of the candidate vector."
  | .releaseSet => "Authenticated release set the executable cohort belongs to."
  | .market => "Authenticated Core Market this trade settles inside."
  | .childRoot => "Authenticated Trading child root address."
  | .obligationAddress => "Authenticated Dealer obligation account address."
  | .currentObligationDigest => "Digest of the obligation state being replaced."
  | .candidateObligationDigest => "Digest of the staged replacement state."
  | .dealerOwner => "Authenticated Dealer Claims Position owner."
  | .counterpartyOwner => "Authenticated counterparty Claims Position owner."
  | .counterpartyAccount => "Authenticated counterparty principal account."
  | .currentObligationRevision => "Optimistic revision of the current state."
  | .candidateObligationRevision =>
      "Optimistic revision of the replacement, exactly one past the current."
  | .dealerPositionRevision => "Optimistic revision of the Dealer Position."
  | .counterpartyPositionRevision =>
      "Optimistic revision of the counterparty Position."
  | .claimsRevision => "Expected Claims aggregate revision."
  | .generation => "Market generation this request is bound to."
  | .expiresAt => "Exact expiry slot."
  | .principal => "Exact principal moved by the fill."
  | .realizedFee => "Exact realized fee, bounded by the principal when dealer-pays."
  | .direction => "Quote direction: 0 counterparty-pays, 1 dealer-pays."
  | .claimsPositionCount => "Claims Position-table width: exactly one or two."
  | .dealerEvidenceCount =>
      "Conditional Dealer Position evidence account: one for P1, zero for P2."
  | .evidenceSpanCount =>
      "Projected width of the trailing readonly evidence span."
  | .claimsPacketBytes => "Exact borrowed SignedDeltaV3 witness width."
  | .routeSpanCounts =>
      "The six exact optional-Custody route-span counts, in slot order."
  | .reserved =>
      "Canonical-zero pad keeping the candidate-obligation vector aligned."

def coordinate (field : Field) : Nat × Nat :=
  (coordinate? field layout).getD (0, 0)

def offset (field : Field) : Nat := (coordinate field).1
def width (field : Field) : Nat := (coordinate field).2

end Field

/-- The CapabilityProgramSet selector offset, which is not an independent
number: `select` reads the `u16` this header calls `action`. -/
def selectorOffset : Nat := Field.offset .action

/-- The selector buffer a caller builds is exactly this header's prefix. -/
def selectorBytes : Nat := Field.offset .action + Field.width .action

/-! ## What the layout says -/

theorem schema_well_formed : WellFormed schema := by
  constructor
  · native_decide
  · native_decide

theorem layout_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

/-- The fields cover the 392 bytes the Rust declares and the executor hashes:
no gap, and the last field ends exactly at the declared width.  This is the
statement disjointness does not make, and a two-byte pad is exactly where the
gap it rules out would have hidden. -/
theorem header_covers_its_declared_width :
    headerBytes = 392 ∧ tiles 0 layout 392 = true := by
  native_decide

/-- Every coordinate the Rust wrote as a decimal literal, including the
twenty-three it never gave a name to. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 2),
    (.obligationWidth, 12, 4),
    (.releaseSet, 16, 32), (.market, 48, 32), (.childRoot, 80, 32),
    (.obligationAddress, 112, 32), (.currentObligationDigest, 144, 32),
    (.candidateObligationDigest, 176, 32), (.dealerOwner, 208, 32),
    (.counterpartyOwner, 240, 32), (.counterpartyAccount, 272, 32),
    (.currentObligationRevision, 304, 8),
    (.candidateObligationRevision, 312, 8),
    (.dealerPositionRevision, 320, 8),
    (.counterpartyPositionRevision, 328, 8),
    (.claimsRevision, 336, 8), (.generation, 344, 8), (.expiresAt, 352, 8),
    (.principal, 360, 8), (.realizedFee, 368, 8),
    (.direction, 376, 1), (.claimsPositionCount, 377, 1),
    (.dealerEvidenceCount, 378, 1), (.evidenceSpanCount, 379, 1),
    (.claimsPacketBytes, 380, 4), (.routeSpanCounts, 384, 6),
    (.reserved, 390, 2)
  ] := by
  native_decide

/-- The two bytes at 390 are not spare room.  The six route-span counts end at
390, the vector that begins at the header's end is a `u64[]`, and the pad is
narrower than one element -- so it is the smallest span that can align the
vector, which is what "reserved" was carrying in prose. -/
theorem the_pad_is_what_aligns_the_candidate_vector :
    (Field.offset .routeSpanCounts + Field.width .routeSpanCounts)
        % candidateStride ≠ 0 ∧
      obligationsOffset % candidateStride = 0 ∧
      Field.width .reserved < candidateStride := by
  native_decide

/-- The selector is this header's own prefix.  `10` had three authors: a
constant, the encoder's bare `write_bytes(output, 10, ...)`, and the bare slice
index in `selector[10..12]`; the twelve-byte buffer is the first three fields
and ends exactly where the runtime width begins. -/
theorem the_selector_reads_the_action_field :
    selectorOffset = 10 ∧ Field.width .action = 2 ∧ selectorBytes = 12 ∧
      selectorBytes = Field.offset .obligationWidth ∧
      schemaWidth selectorPrefix = selectorBytes := by
  native_decide

/-- The header grew by APPENDING: the six route-span counts begin exactly where
the superseded 384-byte header ended, so every coordinate below 384 is where V4
left it -- and the version still had to move, because a decoder that trusts the
prefix would read a shorter request as a valid one. -/
theorem the_growth_was_appended :
    Field.offset .routeSpanCounts = supersededHeaderBytes ∧
      headerBytes = supersededHeaderBytes + 8 ∧
      version = supersededVersion + 1 := by
  native_decide

/-- The declared frame shape is six single bytes, one per optional Custody
route, and each admits exactly `{0, routeSpanWidth}`.  A third value would carve
an account window no route can fill. -/
theorem the_route_span_table_is_one_byte_per_route :
    Field.width .routeSpanCounts = routeSpanCount ∧ routeSpanCount = 6 ∧
      routeSpanWidth = 14 ∧ routeSpanWidth < 256 := by
  native_decide

/-- The four routing coordinates are contiguous and sit immediately before the
frame shape, which is why a decoder can size the request from a 384-byte prefix
before it trusts anything after it. -/
theorem the_routing_coordinates_are_contiguous :
    Field.offset .claimsPositionCount = Field.offset .direction + 1 ∧
      Field.offset .dealerEvidenceCount =
        Field.offset .claimsPositionCount + 1 ∧
      Field.offset .evidenceSpanCount =
        Field.offset .dealerEvidenceCount + 1 ∧
      Field.offset .claimsPacketBytes = Field.offset .evidenceSpanCount + 1 ∧
      Field.offset .claimsPacketBytes + Field.width .claimsPacketBytes =
        Field.offset .routeSpanCounts := by
  native_decide

/-- Every identity coordinate is the same width, and there are nine of them.
The Rust wrote `read_identity(bytes, 48)` nine times with nine bare numbers and
never said they shared a width. -/
theorem the_identities_are_nine_equal_widths :
    identities.length = 9 ∧
      identities.all (fun field => field.kind.byteWidth == identityBytes) =
        true := by
  native_decide

/-- The nine chain-copied scalars are all `u64`, which is what makes a
transposition among them invisible to a round trip and visible only to a
byte-compare against the committed emission. -/
theorem the_scalars_are_nine_equal_widths :
    scalars.length = 9 ∧
      scalars.all (fun field => field.kind.byteWidth == candidateStride) =
        true := by
  native_decide

theorem magic_is_eight_bytes : magic.length = 8 := by native_decide

/-- The magic occupies exactly the coordinate it is written at. -/
theorem magic_fills_its_field : magic.length = Field.width .magic := by
  native_decide

/-- Every field has a distinct Rust name, so no two coordinates can be emitted
under one constant. -/
theorem rust_names_are_distinct :
    (Field.all.map Field.rustName).Nodup := by native_decide

/-- The name table covers the schema exactly: every placed field is named, and
no name is printed for a field the schema does not place. -/
theorem every_placed_field_is_named :
    Field.all = schema.map (fun field => field.name) := by native_decide

end DClutch.DealerScenarioTradeV4Abi
