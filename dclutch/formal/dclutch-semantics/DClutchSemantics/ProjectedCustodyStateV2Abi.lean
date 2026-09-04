import DClutchSemantics.AbiCoverage

/-!
# The projected-custody state, and the four phases a founding walks

Eight hundred and eight bytes of pre-founding custody for a Market that does
not exist yet: a projection is `Initialized`, opens a Hoard, may fund a source
compartment, locks, and is then either realized into a founding or aborted back
out.  Twelve guards across three crates read that phase.

`ProjectedCustodyPhaseV1` was one of the four machines the route census gates
on with no Lean owner at all.  Its four discriminants were
`crates/dclutch-custody-contract/src/projected.rs`'s, and its coordinate had no
name in any language: `encode` writes `put_u8(&mut output, 10, ..)` and
`decode` reads `read_u8(input, 10)`, both bare, which is why the SDK generator
had to recover the offset by requiring those two expressions to AGREE.  Every
coordinate in this record was a bare argument -- `16`, `32`, `704`, `712`,
`720`, `728`, `736`, `744`, `752`, `760`, `768`, `776` -- and the two
canonical-zero spans existed only as the arguments of two `any_nonzero` calls.

## Bit zero is never occupied, and that is the record's defence

The machine numbers from ONE.  So a zeroed account's phase byte is `0`, which
no phase claims and `decode`'s hostile arm refuses -- the opposite of the
ticket state, whose `Prepared` is `0` and which therefore leans entirely on its
magic.  `the_machine_numbers_from_one` is that statement, and it is also what
`projected_admission_v1.rs` means by `PHASE_LIMIT: u8 = 5` for four phases:
the bitset wastes bit zero on purpose, and the bound is one past the last
variant rather than the count.

## The identities are one span, not twenty-one fields

`read_identities::<21>(input, 32)` and its writer treat the whole block as one
addressed run, and every identity in it is a full content id.  Stated as a
single 672-byte field with a declared count, the record's width becomes
arithmetic anybody can check rather than a number that happened to be typed
correctly once.
-/

namespace DClutch.ProjectedCustodyStateV2Abi

open DClutch.AbiSchema

/-- `DCLPCS02`. -/
def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x50, 0x43, 0x53, 0x30, 0x32]

/-- The implemented persisted-state schema version. -/
def schemaVersion : Nat := 2

/-- The four phases a projected founding walks. -/
inductive Phase where
  | initialized | hoardOpen | hoardLocked | sourceFunded
  deriving DecidableEq, Repr

namespace Phase

def all : List Phase := [.initialized, .hoardOpen, .hoardLocked, .sourceFunded]

/-- The wire tag persisted in the phase byte. -/
def tag : Phase → Nat
  | .initialized => 1
  | .hoardOpen => 2
  | .hoardLocked => 3
  | .sourceFunded => 4

def rustName : Phase → String
  | .initialized => "PROJECTED_CUSTODY_PHASE_INITIALIZED_V1"
  | .hoardOpen => "PROJECTED_CUSTODY_PHASE_HOARD_OPEN_V1"
  | .hoardLocked => "PROJECTED_CUSTODY_PHASE_HOARD_LOCKED_V1"
  | .sourceFunded => "PROJECTED_CUSTODY_PHASE_SOURCE_FUNDED_V1"

def doc : Phase → String
  | .initialized => "Projection state exists; vault is not created."
  | .hoardOpen => "Empty projected Hoard vault exists."
  | .hoardLocked => "Exact principal has been credited into the projected Hoard."
  | .sourceFunded =>
      "The normal source compartment exists and holds the exact principal."

/-- The two phases that carry a nonzero custodied amount, wherever the
principal currently sits: in the funded source compartment before Lock, in the
Hoard after it.  `decode` refuses any other phase with a nonzero
`locked_amount` and these two with a zero one. -/
def custodied : Phase → Bool
  | .hoardLocked | .sourceFunded => true
  | .initialized | .hoardOpen => false

end Phase

/-- One past the greatest tag, which is `PHASE_LIMIT` in
`projected_admission_v1.rs`.  It is one past the last variant rather than the
number of variants, because the machine numbers from one. -/
def phaseLimit : Nat := 5

/-- Identities in the immutable request block, and the width of each. -/
def identityCount : Nat := 21
def identityBytes : Nat := 32

inductive Field where
  | magic | schemaVersion | phase | bump | fundingSourceCompartment
  | headReserved | principalCapSets | capReserved | identities
  | generation | expirySlot | nextRevision | lockedAmount
  | stateRentLamports | vaultRentLamports | fundingSourceReplayRevision
  | fundingSourceStateRentLamports | fundingSourceVaultRentLamports
  | lastRequestDigest
  deriving DecidableEq, Repr

/-- The header: the family's magic and version, this record's three one-byte
tags, and the canonical-zero span that pads them to the cap count. -/
def header : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.schemaVersion, .u16⟩,
  ⟨.phase, .u8⟩, ⟨.bump, .u8⟩, ⟨.fundingSourceCompartment, .u8⟩,
  ⟨.headReserved, .reserved 3⟩,
  ⟨.principalCapSets, .u64⟩, ⟨.capReserved, .reserved 8⟩
]

/-- The immutable request's identities, as the one addressed run
`read_identities` and `write_identities` treat them as. -/
def identities : List (FieldSpec Field) := [
  ⟨.identities, .bytes (identityCount * identityBytes)⟩
]

/-- The mutable tail: the request's two slot bounds, the replay revision and
locked amount the phase constrains, four rent measurements, and the digest of
the request that produced this state. -/
def tail : List (FieldSpec Field) := [
  ⟨.generation, .u64⟩, ⟨.expirySlot, .u64⟩,
  ⟨.nextRevision, .u64⟩, ⟨.lockedAmount, .u64⟩,
  ⟨.stateRentLamports, .u64⟩, ⟨.vaultRentLamports, .u64⟩,
  ⟨.fundingSourceReplayRevision, .u64⟩,
  ⟨.fundingSourceStateRentLamports, .u64⟩,
  ⟨.fundingSourceVaultRentLamports, .u64⟩,
  ⟨.lastRequestDigest, .bytes 32⟩
]

def schema : List (FieldSpec Field) := header ++ identities ++ tail

def layout : List (PlacedField Field) := specialize schema
def projectedStateBytes : Nat := schemaWidth schema

/-- Where the identity block begins: the width of the header in front of it. -/
def identitiesOffset : Nat := schemaWidth header
/-- Where the mutable tail begins: the header plus the whole identity run. -/
def tailOffset : Nat := schemaWidth header + schemaWidth identities

namespace Field

def all : List Field := [
  .magic, .schemaVersion, .phase, .bump, .fundingSourceCompartment,
  .headReserved, .principalCapSets, .capReserved, .identities,
  .generation, .expirySlot, .nextRevision, .lockedAmount,
  .stateRentLamports, .vaultRentLamports, .fundingSourceReplayRevision,
  .fundingSourceStateRentLamports, .fundingSourceVaultRentLamports,
  .lastRequestDigest
]

def rustName : Field → String
  | .magic => "PROJECTED_CUSTODY_STATE_MAGIC_OFFSET_V2"
  | .schemaVersion => "PROJECTED_CUSTODY_STATE_VERSION_OFFSET_V2"
  | .phase => "PROJECTED_CUSTODY_STATE_PHASE_OFFSET_V2"
  | .bump => "PROJECTED_CUSTODY_STATE_BUMP_OFFSET_V2"
  | .fundingSourceCompartment =>
      "PROJECTED_CUSTODY_STATE_FUNDING_SOURCE_COMPARTMENT_OFFSET_V2"
  | .headReserved => "PROJECTED_CUSTODY_STATE_HEAD_RESERVED_OFFSET_V2"
  | .principalCapSets => "PROJECTED_CUSTODY_STATE_PRINCIPAL_CAP_SETS_OFFSET_V2"
  | .capReserved => "PROJECTED_CUSTODY_STATE_CAP_RESERVED_OFFSET_V2"
  | .identities => "PROJECTED_CUSTODY_STATE_IDENTITIES_OFFSET_V2"
  | .generation => "PROJECTED_CUSTODY_STATE_GENERATION_OFFSET_V2"
  | .expirySlot => "PROJECTED_CUSTODY_STATE_EXPIRY_SLOT_OFFSET_V2"
  | .nextRevision => "PROJECTED_CUSTODY_STATE_NEXT_REVISION_OFFSET_V2"
  | .lockedAmount => "PROJECTED_CUSTODY_STATE_LOCKED_AMOUNT_OFFSET_V2"
  | .stateRentLamports => "PROJECTED_CUSTODY_STATE_STATE_RENT_OFFSET_V2"
  | .vaultRentLamports => "PROJECTED_CUSTODY_STATE_VAULT_RENT_OFFSET_V2"
  | .fundingSourceReplayRevision =>
      "PROJECTED_CUSTODY_STATE_SOURCE_REPLAY_REVISION_OFFSET_V2"
  | .fundingSourceStateRentLamports =>
      "PROJECTED_CUSTODY_STATE_SOURCE_STATE_RENT_OFFSET_V2"
  | .fundingSourceVaultRentLamports =>
      "PROJECTED_CUSTODY_STATE_SOURCE_VAULT_RENT_OFFSET_V2"
  | .lastRequestDigest => "PROJECTED_CUSTODY_STATE_LAST_REQUEST_DIGEST_OFFSET_V2"

def doc : Field → String
  | .magic => "Canonical projected-custody state magic."
  | .schemaVersion => "This record's ABI version coordinate."
  | .phase => "The persisted `ProjectedCustodyPhaseV1` wire tag."
  | .bump => "Mined bump of the projected-custody state PDA."
  | .fundingSourceCompartment => "Tag of the compartment the projection funds from."
  | .headReserved => "Canonical-zero span between the three tags and the cap count."
  | .principalCapSets => "Principal cap sets this projection was admitted under; never zero."
  | .capReserved => "Canonical-zero span between the cap count and the identity block."
  | .identities => "The immutable request's twenty-one content identities, one addressed run."
  | .generation => "Market generation the projection was authenticated against."
  | .expirySlot => "Slot after which the founding refuses and the abort opens."
  | .nextRevision => "Replay revision this state carries; never zero."
  | .lockedAmount => "Principal currently custodied; nonzero exactly in the two custodied phases."
  | .stateRentLamports => "Rent held for the projected state account."
  | .vaultRentLamports => "Rent held for the projected Hoard vault."
  | .fundingSourceReplayRevision => "Replay revision of the funding source compartment."
  | .fundingSourceStateRentLamports => "Rent held for the funding source replay account."
  | .fundingSourceVaultRentLamports => "Rent held for the funding source vault."
  | .lastRequestDigest => "Digest of the request that produced this state; never zero."

def coordinate (field : Field) : Nat × Nat :=
  (coordinate? field layout).getD (0, 0)

def offset (field : Field) : Nat := (coordinate field).1
def width (field : Field) : Nat := (coordinate field).2

end Field

/-! ## The header five records share

`projected.rs` holds five records -- the request, this state, the receipt, the
lock receipt, and the request's own re-encode -- and all five open with the
same two words: an eight-byte magic, then a `u16` ABI version. One private
`header_version` helper reads that shape for every one of them and wrote both
coordinates as bare literals: `input.get(..8)` for the magic and
`read_u16(input, 8)` for the version. Four `encode` methods wrote the same two
literals back.

That was the debt `7ee656e2d` named rather than paid: this record's version
coordinate was emitted, the family's was not, and a `const _: () = assert!` in
Rust pinned the two together because no Lean module owned the second. It does
now. The family header is exactly this record's first two fields -- the other
four records repeat the shape, they do not vary it -- so the constants below
are emitted under FAMILY names, which say whose they are, and the assert has
nothing left to compare.
-/

/-- Where the eight-byte magic sits in every record of this family. -/
def familyMagicOffset : Nat := Field.offset .magic
/-- The magic's width, which is the `..8` slice four encoders wrote. -/
def familyMagicBytes : Nat := Field.width .magic
/-- Where the `u16` ABI version sits in every record of this family. -/
def familyVersionOffset : Nat := Field.offset .schemaVersion
/-- Total width of the shared header: the magic plus the version word. -/
def familyHeaderBytes : Nat := familyVersionOffset + Field.width .schemaVersion

/-- Physical predicate a schema-level statement can be made about. -/
def isReserved : FieldKind → Bool
  | .reserved _ => true
  | _ => false

/-! ## What the layout says -/

theorem schema_well_formed : WellFormed schema := by
  constructor
  · native_decide
  · native_decide

theorem layout_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

/-- The nineteen fields cover the eight hundred and eight bytes every reader
allocates: no gap, and the last field ends exactly at the declared width. -/
theorem layout_covers_its_declared_width :
    projectedStateBytes = 808 ∧ tiles 0 layout 808 = true := by
  native_decide

/-- Every coordinate, including the twelve that were bare arguments inside
`encode` and `decode` and had no name in any language. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2),
    (.phase, 10, 1), (.bump, 11, 1), (.fundingSourceCompartment, 12, 1),
    (.headReserved, 13, 3),
    (.principalCapSets, 16, 8), (.capReserved, 24, 8),
    (.identities, 32, 672),
    (.generation, 704, 8), (.expirySlot, 712, 8),
    (.nextRevision, 720, 8), (.lockedAmount, 728, 8),
    (.stateRentLamports, 736, 8), (.vaultRentLamports, 744, 8),
    (.fundingSourceReplayRevision, 752, 8),
    (.fundingSourceStateRentLamports, 760, 8),
    (.fundingSourceVaultRentLamports, 768, 8),
    (.lastRequestDigest, 776, 32)
  ] := by
  native_decide

/-- **The coordinate the Rust never named.**  The phase byte begins exactly
where the version word ends and is exactly one byte wide, so `10` is a
placement.  The SDK generator had to recover it by matching an encode and a
decode expression and refusing when they disagreed. -/
theorem the_phase_follows_the_version_word :
    Field.offset .phase = Field.offset .schemaVersion + Field.width .schemaVersion ∧
      Field.width .phase = 1 ∧ Field.offset .phase = 10 := by
  native_decide

/-- The two canonical-zero spans, which are exactly the two `decode` refuses a
nonzero byte in: `(13, 3)` and `(24, 8)`.  Both were the arguments of an
`any_nonzero` call and had no name. -/
theorem the_two_reserved_spans_are_the_ones_decode_enforces :
    schema.filter (fun field => isReserved field.kind) =
      [⟨.headReserved, .reserved 3⟩, ⟨.capReserved, .reserved 8⟩] ∧
      Field.offset .headReserved = 13 ∧ Field.width .headReserved = 3 ∧
      Field.offset .capReserved = 24 ∧ Field.width .capReserved = 8 := by
  native_decide

/-- The head span pads the three one-byte tags to the cap count, and the cap
span pads that to the identity block, so no byte of the header is unowned. -/
theorem the_reserved_spans_pad_the_header_exactly :
    Field.offset .headReserved =
        Field.offset .fundingSourceCompartment +
          Field.width .fundingSourceCompartment ∧
      Field.offset .headReserved + Field.width .headReserved =
        Field.offset .principalCapSets ∧
      Field.offset .capReserved + Field.width .capReserved =
        Field.offset .identities ∧
      identitiesOffset = Field.offset .identities := by
  native_decide

/-- The identity block is exactly twenty-one content ids and the mutable tail
begins where it ends.  `read_identities::<21>(input, 32)` states the count in a
turbofish and the offset as an argument, and nothing said the two multiplied to
the distance to `generation`. -/
theorem the_identity_block_is_twenty_one_content_ids :
    Field.width .identities = identityCount * identityBytes ∧
      identityCount = 21 ∧ identityBytes = 32 ∧
      Field.offset .identities + Field.width .identities =
        Field.offset .generation ∧
      tailOffset = Field.offset .generation := by
  native_decide

/-- **Bit zero is never occupied.**  The machine numbers from one, so a zeroed
account's phase byte claims no phase and the hostile arm refuses it -- unlike
the occurrence-ticket state, whose `Prepared` is zero.  `PHASE_LIMIT` is
therefore one past the last variant rather than the count, and it still fits a
`u8` bitset. -/
theorem the_machine_numbers_from_one :
    (Phase.all.map Phase.tag) = [1, 2, 3, 4] ∧
      (Phase.all.map Phase.tag).Nodup ∧
      Phase.all.all (fun phase => 0 < Phase.tag phase) = true ∧
      Phase.all.all (fun phase => Phase.tag phase < phaseLimit) = true ∧
      phaseLimit = 5 ∧ phaseLimit ≤ 8 := by
  native_decide

/-- The two phases that carry principal are the last two, which is why
`decode` refuses a nonzero locked amount in either of the first two and a zero
one in either of the last two. -/
theorem exactly_the_last_two_phases_are_custodied :
    Phase.all.filter Phase.custodied = [.hoardLocked, .sourceFunded] := by
  native_decide

/-- **The header the other four records repeat.**  It is this record's first
two fields and nothing else: the magic at zero for eight bytes, the version
word immediately behind it, ten bytes in total before any record's own content
begins.  `header_version` read all three of those numbers as literals. -/
theorem the_family_header_is_this_record_s_first_two_fields :
    familyMagicOffset = 0 ∧ familyMagicBytes = 8 ∧
      familyVersionOffset = 8 ∧ familyHeaderBytes = 10 ∧
      familyVersionOffset = familyMagicOffset + familyMagicBytes := by
  native_decide

theorem magic_is_eight_bytes : magic.length = 8 := by native_decide

theorem magic_fills_its_field : magic.length = Field.width .magic := by
  native_decide

theorem rust_names_are_distinct : (Field.all.map Field.rustName).Nodup := by
  native_decide

theorem phase_rust_names_are_distinct : (Phase.all.map Phase.rustName).Nodup := by
  native_decide

theorem every_placed_field_is_named :
    Field.all = schema.map (fun field => field.name) := by native_decide

end DClutch.ProjectedCustodyStateV2Abi
