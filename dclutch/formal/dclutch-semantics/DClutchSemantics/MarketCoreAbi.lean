import DClutchSemantics.AbiSchema
import DClutchSemantics.MarketCore

/-!
# Fixed Market Core physical ABI V3

The fixed 368-byte header contains only lifecycle, the immutable Market identity,
the source-projected complete-set principal ceiling
references, and the canonical PDA bumps recorded at founding. The Product-owned
outcome count remains runtime data, while all
mutable claim vectors and Hoard principal belong exclusively to the Claims
aggregate deterministically derived by the selected Claims program. Source/Resolution exclusively owns
action funding, work balances, and closure. Core stores neither parallel child
ledger nor economic tail and therefore introduces no semantic N ceiling.
Offsets are specialized from field data rather than maintained by hand.

## The bump tail

The founding is the only party that ever derives the Market state address and
the Realm record pair from seeds it has already authenticated; every later
reader repeats those searches at 1,500 compute units per rejected candidate.
The three bump bytes let a reader reproduce the address directly instead. A
zero byte means the founding recorded no bump, and its reader searches — so the
tail is a pure superset and no bump is ever an authority: a wrong bump
reproduces a different address and refuses.

The reserved bytes are slack for the bumps not yet carried. They exist because
the census that motivated this tail found every other carrier on the route
already full, and widening this account costs a re-founding rather than a
patch.

## Four of those five reserved bytes are now the Product graph, packed

The Trading prelude and the Dealer accelerator each walk the same four Registry
records — Product, ResultDomain, Portfolio, linked basis — and
`authenticate_record` searches for TWO addresses per record, the raw body and
its staging cursor.  Measured 2026-09-03 on real SBF ELFs, those eight searches
are 30,172 of the accelerator's 39,217-CU walk, and a hinted walk keeps 12,000
of it: the saving is 17,467 per walk and Trading pays a second one.

Eight bumps do not fit in five bytes, and the alternative — appending eight and
moving `STATE_BYTES` to 376 — costs exactly what this doc's last paragraph
says: a re-founding, because Core's only `resize` is `resize(0)` and every
market already written would be refused by length forever.  So the eight ride
as NIBBLES in four of the five reserved bytes and the width does not move.

The nibble encoding is the byte encoding's own idiom one level down.  Zero is
unrecorded and its reader searches, exactly as a zero byte is; a recorded
nibble `v` in `1..=15` is the bump `256 - v`, so it carries 255 down to 241.  A
bump below 241 is not representable and is recorded as unrecorded, which costs
a search on roughly one derivation in 32,768 and can never cost a refusal.
An account written before this field existed holds five zero bytes and reads
as eight unrecorded bumps, so every market founded before it keeps the search.
-/

namespace DClutch.MarketCoreAbi

open DClutch.AbiSchema

def stateMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x4f, 0x52, 0x33]
def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x52, 0x51, 0x32]
def version : Nat := 3

inductive StateField where
  | magic | version | phase | readiness | terminalWinner
  | marketId | identityRealm | productRecord | productId
  | resolutionPolicy | capabilityManifest | selectedReleaseSet | registryProgram | generation
  | outstandingCapabilities
  | principalCapSets
  | rentBeneficiary
  | terminalReceipt
  | marketBump | realmRawRecordBump | realmStagingRecordBump
  | productGraphBumps | reservedBumps
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩, ⟨.readiness, .u8⟩,
  ⟨.terminalWinner, .u32⟩,
  ⟨.marketId, .bytes 32⟩, ⟨.identityRealm, .bytes 32⟩,
  ⟨.productRecord, .bytes 32⟩, ⟨.productId, .bytes 32⟩,
  ⟨.resolutionPolicy, .bytes 32⟩,
  ⟨.capabilityManifest, .bytes 32⟩,
  ⟨.selectedReleaseSet, .bytes 32⟩, ⟨.registryProgram, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.outstandingCapabilities, .u64⟩,
  ⟨.principalCapSets, .u64⟩,
  ⟨.rentBeneficiary, .bytes 32⟩,
  ⟨.terminalReceipt, .bytes 32⟩,
  ⟨.marketBump, .u8⟩,
  ⟨.realmRawRecordBump, .u8⟩, ⟨.realmStagingRecordBump, .u8⟩,
  ⟨.productGraphBumps, .bytes 4⟩,
  ⟨.reservedBumps, .reserved 1⟩
]

def stateLayout : List (PlacedField StateField) := specialize stateSchema
def stateBytes : Nat := schemaWidth stateSchema

namespace StateField

def rustName : StateField → String
  | .magic => "STATE_MAGIC_OFFSET" | .version => "STATE_VERSION_OFFSET"
  | .phase => "STATE_PHASE_OFFSET" | .readiness => "STATE_READINESS_OFFSET"
  | .terminalWinner => "STATE_TERMINAL_WINNER_OFFSET"
  | .marketId => "STATE_MARKET_ID_OFFSET" | .identityRealm => "STATE_IDENTITY_REALM_OFFSET"
  | .productRecord => "STATE_PRODUCT_RECORD_OFFSET"
  | .productId => "STATE_PRODUCT_ID_OFFSET"
  | .resolutionPolicy => "STATE_RESOLUTION_POLICY_OFFSET"
  | .capabilityManifest => "STATE_CAPABILITY_MANIFEST_OFFSET"
  | .selectedReleaseSet => "STATE_SELECTED_RELEASE_SET_OFFSET"
  | .registryProgram => "STATE_REGISTRY_PROGRAM_OFFSET" | .generation => "STATE_GENERATION_OFFSET"
  | .outstandingCapabilities => "STATE_OUTSTANDING_CAPABILITIES_OFFSET"
  | .principalCapSets => "STATE_PRINCIPAL_CAP_SETS_OFFSET"
  | .rentBeneficiary => "STATE_RENT_BENEFICIARY_OFFSET"
  | .terminalReceipt => "STATE_TERMINAL_RECEIPT_OFFSET"
  | .marketBump => "STATE_MARKET_BUMP_OFFSET"
  | .realmRawRecordBump => "STATE_REALM_RAW_RECORD_BUMP_OFFSET"
  | .realmStagingRecordBump => "STATE_REALM_STAGING_RECORD_BUMP_OFFSET"
  | .productGraphBumps => "STATE_PRODUCT_GRAPH_BUMPS_OFFSET"
  | .reservedBumps => "STATE_RESERVED_BUMPS_OFFSET"

end StateField

theorem state_schema_width : stateBytes = 368 := by native_decide
theorem state_schema_unique : (stateSchema.map fun field => field.name).Nodup := by native_decide

/-- Regression witness for every persisted state offset.  It is here to pin one
property the bump tail depends on: the tail is an append, so every field that
existed at 360 bytes still begins exactly where it began.  A widening that
displaced a field would be a silent reinterpretation of live account bytes;
this theorem makes it a red row instead. -/
theorem state_coordinates : coordinates stateLayout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.phase, 10, 1),
    (.readiness, 11, 1),
    (.terminalWinner, 12, 4),
    (.marketId, 16, 32),
    (.identityRealm, 48, 32),
    (.productRecord, 80, 32),
    (.productId, 112, 32),
    (.resolutionPolicy, 144, 32),
    (.capabilityManifest, 176, 32),
    (.selectedReleaseSet, 208, 32),
    (.registryProgram, 240, 32),
    (.generation, 272, 8),
    (.outstandingCapabilities, 280, 8),
    (.principalCapSets, 288, 8),
    (.rentBeneficiary, 296, 32),
    (.terminalReceipt, 328, 32),
    (.marketBump, 360, 1),
    (.realmRawRecordBump, 361, 1),
    (.realmStagingRecordBump, 362, 1),
    (.productGraphBumps, 363, 4),
    (.reservedBumps, 367, 1)
  ] := by native_decide
theorem state_fields_disjoint : stateLayout.Pairwise Before := specializeFrom_pairwise 0 stateSchema
theorem state_fields_bounded (placed : PlacedField StateField) (member : placed ∈ stateLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ stateBytes := by
  simpa [stateLayout, stateBytes, specialize] using
    specializeFrom_bounded 0 stateSchema placed member

inductive Action where
  | found | verifyReadiness | openMarket | admitTerminal
  | split | redeem | beginRetiring | retire | activateCapability | closeCapability
  | executeProvider
  deriving DecidableEq, Repr

def Action.tag : Action → Nat
  | .found => 0 | .verifyReadiness => 1 | .openMarket => 2 | .admitTerminal => 3
  | .split => 4 | .redeem => 5 | .beginRetiring => 6 | .retire => 7
  | .activateCapability => 8 | .closeCapability => 9 | .executeProvider => 10

def phaseFoundingTag : Nat := 0
def phaseOpenTag : Nat := 1
def phaseTerminalTag : Nat := 2
def phaseRetiringTag : Nat := 3
def phaseRetiredTag : Nat := 4

def readinessPrepaidTag : Nat := 0
def readinessReadyTag : Nat := 1
def readinessConsumedTag : Nat := 2

def holderNoneTag : Nat := 0
def holderSourceTag : Nat := 1
def holderDestinationTag : Nat := 2

def representationNoneTag : Nat := 0
def representationNativeTag : Nat := 1
def representationMaterializedTag : Nat := 2

theorem action_tags_fit_u8 :
    [Action.found, .verifyReadiness, .openMarket, .admitTerminal, .split, .redeem,
      .beginRetiring, .retire, .activateCapability, .closeCapability, .executeProvider].all
      (fun action => action.tag < 256) = true := by native_decide

theorem action_tags_unique :
    ([Action.found, .verifyReadiness, .openMarket, .admitTerminal, .split, .redeem,
      .beginRetiring, .retire, .activateCapability, .closeCapability, .executeProvider].map Action.tag).Nodup := by
  native_decide

inductive RequestField where
  | magic | version | action | holder | representation | reservedA
  | outcome | reservedB | quantity | generation | market
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.holder, .u8⟩, ⟨.representation, .u8⟩, ⟨.reservedA, .reserved 3⟩,
  ⟨.outcome, .u32⟩, ⟨.reservedB, .reserved 4⟩, ⟨.quantity, .u64⟩,
  ⟨.generation, .u64⟩, ⟨.market, .bytes 32⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestBytes : Nat := schemaWidth requestSchema

namespace RequestField

def rustName : RequestField → String
  | .magic => "REQUEST_MAGIC_OFFSET" | .version => "REQUEST_VERSION_OFFSET"
  | .action => "REQUEST_ACTION_OFFSET" | .holder => "REQUEST_HOLDER_OFFSET"
  | .representation => "REQUEST_REPRESENTATION_OFFSET" | .reservedA => "REQUEST_RESERVED_A_OFFSET"
  | .outcome => "REQUEST_OUTCOME_OFFSET" | .reservedB => "REQUEST_RESERVED_B_OFFSET"
  | .quantity => "REQUEST_QUANTITY_OFFSET" | .generation => "REQUEST_GENERATION_OFFSET"
  | .market => "REQUEST_MARKET_OFFSET"

end RequestField

theorem request_schema_width : requestBytes = 72 := by native_decide
theorem request_schema_unique : (requestSchema.map fun field => field.name).Nodup := by native_decide
theorem request_fields_disjoint : requestLayout.Pairwise Before := specializeFrom_pairwise 0 requestSchema
theorem request_fields_bounded (placed : PlacedField RequestField) (member : placed ∈ requestLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ requestBytes := by
  simpa [requestLayout, requestBytes, specialize] using
    specializeFrom_bounded 0 requestSchema placed member

end DClutch.MarketCoreAbi
