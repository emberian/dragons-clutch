import DClutchSemantics.AbiCoverage

/-!
# Claims LiabilityBasisV2 persisted state ABI

A Core Market root carries identity and lifecycle.  The per-claim SUPPLY vector
of a market, and every owner's BALANCE vector, live in Claims-owned
LiabilityBasisV2 accounts at PDAs derived from the market and the owner.  Both
are a fixed header followed by a runtime `u64[claim_count]` tail.

Until this module existed the layouts had no Lean owner at all.
`crates/dclutch-claims-svm/src/liability_basis_state_v2.rs` called itself "the
sole SDK-free owner of the aggregate and Position byte layouts" and wrote
sixteen offsets as decimal literals; the browser then read those literals back
out with a regular expression.  Worse, the coordinates the module did NOT name
were the prologue ones: `require_prefix` reads the magic at a bare `0` and the
version at a bare `8`, the encoder writes them at a bare `0` and `8`, and the
Position decoder zero-checks its header gap with a bare `(10, 2)`.  The five
numbers a record shares with every other record in the tree were the five nobody
made a field.

Every offset below is a placement, and the two records are written as a shared
prologue followed by their own fields, so "both begin the same way" is a theorem
rather than a coincidence that held five times.
-/

namespace DClutch.ClaimsLiabilityBasisStateV2Abi

open DClutch.AbiSchema

/-- Implemented state ABI version, shared by both records. -/
def version : Nat := 2

/-- `DCLLBM02` -- the aggregate. -/
def marketMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x4c, 0x42, 0x4d, 0x30, 0x32]

/-- `DCLLBP02` -- one owner's Position. -/
def positionMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x4c, 0x42, 0x50, 0x30, 0x32]

/-- Aggregate PDA seed domain: `[marketSeed, logical_market]`. -/
def marketSeed : String := "dclutch:lbv2:market"

/-- Position PDA seed domain: `[positionSeed, aggregate, owner]`. -/
def positionSeed : String := "dclutch:lbv2:position"

/-- One claim atom in either runtime tail: a little-endian `u64`. -/
def claimStride : Nat := 8

/-! ## The aggregate -/

inductive MarketField where
  | magic | version | reserved | claimCount | revision
  | logicalMarket | releaseSet | registryProgram | productInstance
  | basis | realm | custodyContext | generation
  deriving DecidableEq, Repr

/-- The five coordinates both records share.  The first reserved byte is the
account's own canonical PDA bump -- a memo of the creator's derivation, never an
authority, and deliberately not zero-checked -- which is why the reserved span
is two bytes wide and not one. -/
def marketPrologue : List (FieldSpec MarketField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reserved, .reserved 2⟩,
  ⟨.claimCount, .u32⟩, ⟨.revision, .u64⟩
]

def marketSchema : List (FieldSpec MarketField) := marketPrologue ++ [
  ⟨.logicalMarket, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.registryProgram, .bytes 32⟩, ⟨.productInstance, .bytes 32⟩,
  ⟨.basis, .bytes 32⟩, ⟨.realm, .bytes 32⟩, ⟨.custodyContext, .bytes 32⟩,
  ⟨.generation, .u64⟩
]

def marketLayout : List (PlacedField MarketField) := specialize marketSchema
def marketHeaderBytes : Nat := schemaWidth marketSchema

namespace MarketField

def all : List MarketField := [
  .magic, .version, .reserved, .claimCount, .revision,
  .logicalMarket, .releaseSet, .registryProgram, .productInstance,
  .basis, .realm, .custodyContext, .generation
]

def rustName : MarketField → String
  | .magic => "MARKET_MAGIC_OFFSET"
  | .version => "MARKET_VERSION_OFFSET"
  | .reserved => "MARKET_RESERVED_OFFSET"
  | .claimCount => "MARKET_CLAIM_COUNT_OFFSET"
  | .revision => "MARKET_REVISION_OFFSET"
  | .logicalMarket => "MARKET_LOGICAL_ID_OFFSET"
  | .releaseSet => "MARKET_RELEASE_SET_OFFSET"
  | .registryProgram => "MARKET_REGISTRY_OFFSET"
  | .productInstance => "MARKET_PRODUCT_OFFSET"
  | .basis => "MARKET_BASIS_OFFSET"
  | .realm => "MARKET_REALM_OFFSET"
  | .custodyContext => "MARKET_CUSTODY_CONTEXT_OFFSET"
  | .generation => "MARKET_GENERATION_OFFSET"

def coordinate (field : MarketField) : Nat × Nat :=
  (coordinate? field marketLayout).getD (0, 0)

def offset (field : MarketField) : Nat := (coordinate field).1
def width (field : MarketField) : Nat := (coordinate field).2

end MarketField

/-- The aggregate's own PDA bump: the first byte of the reserved span, which is
what the Rust says in prose and used to say by writing `10`. -/
def marketBumpOffset : Nat := MarketField.offset .reserved

/-! ## One owner's Position -/

inductive PositionField where
  | magic | version | reservedHeader | claimCount | revision
  | market | owner | basis | reservedTail
  deriving DecidableEq, Repr

def positionPrologue : List (FieldSpec PositionField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 2⟩,
  ⟨.claimCount, .u32⟩, ⟨.revision, .u64⟩
]

def positionSchema : List (FieldSpec PositionField) := positionPrologue ++ [
  ⟨.market, .bytes 32⟩, ⟨.owner, .bytes 32⟩, ⟨.basis, .bytes 32⟩,
  ⟨.reservedTail, .reserved 8⟩
]

def positionLayout : List (PlacedField PositionField) := specialize positionSchema
def positionHeaderBytes : Nat := schemaWidth positionSchema

namespace PositionField

def all : List PositionField := [
  .magic, .version, .reservedHeader, .claimCount, .revision,
  .market, .owner, .basis, .reservedTail
]

def rustName : PositionField → String
  | .magic => "POSITION_MAGIC_OFFSET"
  | .version => "POSITION_VERSION_OFFSET"
  | .reservedHeader => "POSITION_RESERVED_HEADER_OFFSET"
  | .claimCount => "POSITION_CLAIM_COUNT_OFFSET"
  | .revision => "POSITION_REVISION_OFFSET"
  | .market => "POSITION_MARKET_OFFSET"
  | .owner => "POSITION_OWNER_OFFSET"
  | .basis => "POSITION_BASIS_OFFSET"
  | .reservedTail => "POSITION_RESERVED_OFFSET"

def coordinate (field : PositionField) : Nat × Nat :=
  (coordinate? field positionLayout).getD (0, 0)

def offset (field : PositionField) : Nat := (coordinate field).1
def width (field : PositionField) : Nat := (coordinate field).2

end PositionField

/-- A Position's own PDA bump: the first byte of its reserved tail, the same
carry the aggregate makes in its reserved header. -/
def positionBumpOffset : Nat := PositionField.offset .reservedTail

/-! ## What the layouts say -/

theorem market_schema_well_formed : WellFormed marketSchema := by
  constructor
  · native_decide
  · intro field member
    simp [marketSchema, marketPrologue] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl <;> decide

theorem position_schema_well_formed : WellFormed positionSchema := by
  constructor
  · native_decide
  · intro field member
    simp [positionSchema, positionPrologue] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;>
      decide

theorem market_layout_disjoint : marketLayout.Pairwise Before :=
  specializeFrom_pairwise 0 marketSchema

theorem position_layout_disjoint : positionLayout.Pairwise Before :=
  specializeFrom_pairwise 0 positionSchema

/-- The aggregate's fields cover the 256 bytes its readers allocate: no gap, and
the last field ends exactly at the declared header width.  This is the statement
disjointness does not make. -/
theorem market_layout_covers_its_header :
    marketHeaderBytes = 256 ∧ tiles 0 marketLayout 256 = true := by
  native_decide

/-- The same for a Position's 128. -/
theorem position_layout_covers_its_header :
    positionHeaderBytes = 128 ∧ tiles 0 positionLayout 128 = true := by
  native_decide

/-- Every coordinate the Rust module wrote as a decimal literal, and the four
prologue coordinates it never named at all. -/
theorem market_coordinates_are_canonical : coordinates marketLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.reserved, 10, 2),
    (.claimCount, 12, 4), (.revision, 16, 8),
    (.logicalMarket, 24, 32), (.releaseSet, 56, 32),
    (.registryProgram, 88, 32), (.productInstance, 120, 32),
    (.basis, 152, 32), (.realm, 184, 32), (.custodyContext, 216, 32),
    (.generation, 248, 8)
  ] := by
  native_decide

theorem position_coordinates_are_canonical : coordinates positionLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.reservedHeader, 10, 2),
    (.claimCount, 12, 4), (.revision, 16, 8),
    (.market, 24, 32), (.owner, 56, 32), (.basis, 88, 32),
    (.reservedTail, 120, 8)
  ] := by
  native_decide

/-- Both records begin with the same five placements.  The Rust implemented this
by writing `0`, `8`, `10`, `12` and `16` twice; saying it once is the difference
between a shared prologue and five numbers that happened to agree. -/
theorem both_records_begin_with_the_prologue :
    (marketLayout.take 5).map (fun field => (field.offset, field.spec.kind.byteWidth)) =
      (positionLayout.take 5).map
        (fun field => (field.offset, field.spec.kind.byteWidth)) := by
  native_decide

/-- The two magics differ, which is the whole reason an aggregate and a Position
are distinguishable at all: they share a prologue, a version and a claim count,
and only the magic separates them. -/
theorem record_magics_differ : marketMagic ≠ positionMagic := by native_decide

/-- The two seed domains differ, so an aggregate address can never collide with
a Position address under the same program. -/
theorem seed_domains_differ : marketSeed ≠ positionSeed := by native_decide

theorem magics_are_eight_bytes :
    marketMagic.length = 8 ∧ positionMagic.length = 8 := by native_decide

/-- A bump is the first byte of a reserved span in both records, and both spans
are wide enough to have kept a byte in reserve after giving one up. -/
theorem bumps_are_the_first_reserved_byte :
    marketBumpOffset = 10 ∧ MarketField.width .reserved = 2 ∧
    positionBumpOffset = 120 ∧ PositionField.width .reservedTail = 8 := by
  native_decide

end DClutch.ClaimsLiabilityBasisStateV2Abi
