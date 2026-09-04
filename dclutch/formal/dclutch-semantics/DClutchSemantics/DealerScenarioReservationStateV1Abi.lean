import DClutchSemantics.AbiCoverage

/-!
# The Dealer scenario reservation state, and the three statuses one effect walks

Five hundred and twelve bytes of per-effect reservation lifecycle: value is
held in escrow while `Active`, returned to its original source when
`RolledBack` after expiry, or delivered to its original destination when
`Activated`.  One such account exists per selected Custody effect, and the
checkpoint's reservation receipt run carries one digest per slot.

`DealerScenarioReservationStateStatusV1` is the last of the four machines the
route census gates on that had no Lean owner at all.  Its three discriminants
were `crates/dclutch-dealer-codec/src/scenario_custody_reservation_v1.rs`'s,
and so were its twenty-five coordinates.

## The header this record shares, and the header it owns

That file holds FOUR records -- the custody effect, the effect manifest, the
reservation batch and this state -- and they share one header shape through
four file-private constants: `VERSION_OFFSET`, `TAG_OFFSET`, `ORDINAL_OFFSET`
and `COUNT_OFFSET`, each read by two to four different decoders.  Only this
record has a Lean owner, so what is emitted here are THIS record's coordinates,
under names that say whose they are.  The shared constants stay where the other
three records need them, pinned to the emission by `const _: () = assert!`, and
that is named debt rather than a silence: the other three are still their own
authors and the next lane to own one of them can take the shared block apart.

## The two runs a width is made of

Fourteen thirty-two-byte digests and identities from `16` to `464`, then four
eight-byte balance measurements, then a sixteen-byte canonical-zero tail.  The
Rust states each coordinate and multiplies nothing, so `512` was a number that
had to be right; `layout_covers_its_declared_width` makes it arithmetic.
-/

namespace DClutch.DealerScenarioReservationStateV1Abi

open DClutch.AbiSchema

/-- `DCLTDST1`. -/
def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x44, 0x53, 0x54, 0x31]

/-- The implemented Dealer scenario custody-state schema version, which the
four records in this family share. -/
def schemaVersion : Nat := 1

/-- The three statuses one reservation walks. -/
inductive Status where
  | active | rolledBack | activated
  deriving DecidableEq, Repr

namespace Status

def all : List Status := [.active, .rolledBack, .activated]

/-- The wire tag persisted in the status byte. -/
def tag : Status → Nat
  | .active => 1
  | .rolledBack => 2
  | .activated => 3

def rustName : Status → String
  | .active => "DEALER_SCENARIO_RESERVATION_STATUS_ACTIVE_V1"
  | .rolledBack => "DEALER_SCENARIO_RESERVATION_STATUS_ROLLED_BACK_V1"
  | .activated => "DEALER_SCENARIO_RESERVATION_STATUS_ACTIVATED_V1"

def doc : Status → String
  | .active => "Value is held in escrow."
  | .rolledBack => "Value returned to the original source after expiry."
  | .activated => "Value delivered to the original destination."

/-- Whether the escrow still holds the reserved amount.  `validate` requires
`escrow_after == amount` in exactly this status and `escrow_after == 0` in the
other two, so the predicate is the record's own canonicity rule. -/
def escrowHolds : Status → Bool
  | .active => true
  | .rolledBack | .activated => false

end Status

/-- One past the greatest tag.  The machine numbers from one, so bit zero is
never occupied. -/
def statusLimit : Nat := 4

inductive Field where
  | magic | schemaVersion | status | ordinal | effectCount | headReserved
  | batch | checkpoint | requestDigest | effectsDigest | effectDigest
  | source | destination | escrow | mint | tokenProgram
  | sourcePrestateDigest | destinationPrestateDigest
  | effectPoststateDigest | sourcePoststateDigest
  | amount | sourceAfter | destinationBefore | escrowAfter | tailReserved
  deriving DecidableEq, Repr

/-- The header the four records in this family share -- magic, version, a tag
byte, an ordinal and a count -- and the canonical-zero span that pads it. -/
def header : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.schemaVersion, .u16⟩,
  ⟨.status, .u8⟩, ⟨.ordinal, .u8⟩, ⟨.effectCount, .u8⟩,
  ⟨.headReserved, .reserved 3⟩
]

/-- What this reservation is over: the batch and checkpoint that authorized it,
the request and effect commitments, the three token accounts it moves value
between, the mint and token program, and the four prestate/poststate digests
the activation reauthenticates. -/
def identities : List (FieldSpec Field) := [
  ⟨.batch, .bytes 32⟩, ⟨.checkpoint, .bytes 32⟩,
  ⟨.requestDigest, .bytes 32⟩, ⟨.effectsDigest, .bytes 32⟩,
  ⟨.effectDigest, .bytes 32⟩,
  ⟨.source, .bytes 32⟩, ⟨.destination, .bytes 32⟩, ⟨.escrow, .bytes 32⟩,
  ⟨.mint, .bytes 32⟩, ⟨.tokenProgram, .bytes 32⟩,
  ⟨.sourcePrestateDigest, .bytes 32⟩, ⟨.destinationPrestateDigest, .bytes 32⟩,
  ⟨.effectPoststateDigest, .bytes 32⟩, ⟨.sourcePoststateDigest, .bytes 32⟩
]

/-- The four exact balances the status constrains, then the canonical-zero
tail. -/
def balances : List (FieldSpec Field) := [
  ⟨.amount, .u64⟩, ⟨.sourceAfter, .u64⟩,
  ⟨.destinationBefore, .u64⟩, ⟨.escrowAfter, .u64⟩,
  ⟨.tailReserved, .reserved 16⟩
]

def schema : List (FieldSpec Field) := header ++ identities ++ balances

def layout : List (PlacedField Field) := specialize schema
def reservationStateBytes : Nat := schemaWidth schema

/-- Where the identity block begins: the width of the shared header. -/
def identitiesOffset : Nat := schemaWidth header
/-- Where the four balance measurements begin. -/
def balancesOffset : Nat := schemaWidth header + schemaWidth identities

namespace Field

def all : List Field := [
  .magic, .schemaVersion, .status, .ordinal, .effectCount, .headReserved,
  .batch, .checkpoint, .requestDigest, .effectsDigest, .effectDigest,
  .source, .destination, .escrow, .mint, .tokenProgram,
  .sourcePrestateDigest, .destinationPrestateDigest,
  .effectPoststateDigest, .sourcePoststateDigest,
  .amount, .sourceAfter, .destinationBefore, .escrowAfter, .tailReserved
]

def rustName : Field → String
  | .magic => "DEALER_SCENARIO_RESERVATION_STATE_MAGIC_OFFSET_V1"
  | .schemaVersion => "DEALER_SCENARIO_RESERVATION_STATE_VERSION_OFFSET_V1"
  | .status => "DEALER_SCENARIO_RESERVATION_STATE_STATUS_OFFSET_V1"
  | .ordinal => "DEALER_SCENARIO_RESERVATION_STATE_ORDINAL_OFFSET_V1"
  | .effectCount => "DEALER_SCENARIO_RESERVATION_STATE_EFFECT_COUNT_OFFSET_V1"
  | .headReserved => "DEALER_SCENARIO_RESERVATION_STATE_HEAD_RESERVED_OFFSET_V1"
  | .batch => "DEALER_SCENARIO_RESERVATION_STATE_BATCH_OFFSET_V1"
  | .checkpoint => "DEALER_SCENARIO_RESERVATION_STATE_CHECKPOINT_OFFSET_V1"
  | .requestDigest => "DEALER_SCENARIO_RESERVATION_STATE_REQUEST_OFFSET_V1"
  | .effectsDigest => "DEALER_SCENARIO_RESERVATION_STATE_EFFECTS_OFFSET_V1"
  | .effectDigest => "DEALER_SCENARIO_RESERVATION_STATE_EFFECT_DIGEST_OFFSET_V1"
  | .source => "DEALER_SCENARIO_RESERVATION_STATE_SOURCE_OFFSET_V1"
  | .destination => "DEALER_SCENARIO_RESERVATION_STATE_DESTINATION_OFFSET_V1"
  | .escrow => "DEALER_SCENARIO_RESERVATION_STATE_ESCROW_OFFSET_V1"
  | .mint => "DEALER_SCENARIO_RESERVATION_STATE_MINT_OFFSET_V1"
  | .tokenProgram => "DEALER_SCENARIO_RESERVATION_STATE_TOKEN_PROGRAM_OFFSET_V1"
  | .sourcePrestateDigest => "DEALER_SCENARIO_RESERVATION_STATE_SOURCE_PRESTATE_OFFSET_V1"
  | .destinationPrestateDigest =>
      "DEALER_SCENARIO_RESERVATION_STATE_DESTINATION_PRESTATE_OFFSET_V1"
  | .effectPoststateDigest => "DEALER_SCENARIO_RESERVATION_STATE_ESCROW_POSTSTATE_OFFSET_V1"
  | .sourcePoststateDigest => "DEALER_SCENARIO_RESERVATION_STATE_SOURCE_POSTSTATE_OFFSET_V1"
  | .amount => "DEALER_SCENARIO_RESERVATION_STATE_AMOUNT_OFFSET_V1"
  | .sourceAfter => "DEALER_SCENARIO_RESERVATION_STATE_SOURCE_AFTER_OFFSET_V1"
  | .destinationBefore => "DEALER_SCENARIO_RESERVATION_STATE_DESTINATION_BEFORE_OFFSET_V1"
  | .escrowAfter => "DEALER_SCENARIO_RESERVATION_STATE_ESCROW_AFTER_OFFSET_V1"
  | .tailReserved => "DEALER_SCENARIO_RESERVATION_STATE_RESERVED_OFFSET_V1"

def doc : Field → String
  | .magic => "Canonical reservation state magic."
  | .schemaVersion => "This record's ABI version coordinate."
  | .status => "The persisted `DealerScenarioReservationStateStatusV1` wire tag."
  | .ordinal => "This reservation's slot in the effect ordering; below the effect count."
  | .effectCount => "Custody effects the checkpoint's evaluation selected."
  | .headReserved => "Canonical-zero span between the three tags and the batch identity."
  | .batch => "Reservation batch that authorized this reservation."
  | .checkpoint => "Checkpoint the batch belongs to."
  | .requestDigest => "Digest of the request the scenario is executing."
  | .effectsDigest => "Ordered active Custody effect commitment."
  | .effectDigest => "Digest of the one effect this reservation is over."
  | .source => "Token account value was reserved from."
  | .destination => "Token account value is delivered to on activation."
  | .escrow => "Token account holding the reserved value while Active."
  | .mint => "Mint of the reserved value."
  | .tokenProgram => "Token program the three accounts belong to."
  | .sourcePrestateDigest => "Source account prestate the reserve authenticated against."
  | .destinationPrestateDigest => "Destination account prestate the activation reauthenticates."
  | .effectPoststateDigest => "Escrow poststate this reservation commits to."
  | .sourcePoststateDigest => "Source poststate this reservation commits to."
  | .amount => "Positive locked token amount."
  | .sourceAfter => "Required source balance after reserve."
  | .destinationBefore => "Destination balance before activation."
  | .escrowAfter => "Required escrow balance while Active; zero once terminal."
  | .tailReserved => "Canonical-zero tail span."

def coordinate (field : Field) : Nat × Nat :=
  (coordinate? field layout).getD (0, 0)

def offset (field : Field) : Nat := (coordinate field).1
def width (field : Field) : Nat := (coordinate field).2

end Field

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

/-- The twenty-five fields cover the five hundred and twelve bytes every reader
allocates: no gap, and the last field ends exactly at the declared width. -/
theorem layout_covers_its_declared_width :
    reservationStateBytes = 512 ∧ tiles 0 layout 512 = true := by
  native_decide

/-- Every coordinate, against the `STATE_*` block this replaces. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2),
    (.status, 10, 1), (.ordinal, 11, 1), (.effectCount, 12, 1),
    (.headReserved, 13, 3),
    (.batch, 16, 32), (.checkpoint, 48, 32), (.requestDigest, 80, 32),
    (.effectsDigest, 112, 32), (.effectDigest, 144, 32),
    (.source, 176, 32), (.destination, 208, 32), (.escrow, 240, 32),
    (.mint, 272, 32), (.tokenProgram, 304, 32),
    (.sourcePrestateDigest, 336, 32), (.destinationPrestateDigest, 368, 32),
    (.effectPoststateDigest, 400, 32), (.sourcePoststateDigest, 432, 32),
    (.amount, 464, 8), (.sourceAfter, 472, 8),
    (.destinationBefore, 480, 8), (.escrowAfter, 488, 8),
    (.tailReserved, 496, 16)
  ] := by
  native_decide

/-- **This record's own header coordinates.**  The status byte begins where the
version word ends, and the ordinal and effect count follow it as single bytes.
The four constants the Rust reads them through are shared with three other
records in the same file, so what makes these THIS record's is the placement,
not the name. -/
theorem the_status_heads_this_records_tag_run :
    Field.offset .status = Field.offset .schemaVersion + Field.width .schemaVersion ∧
      Field.width .status = 1 ∧ Field.offset .status = 10 ∧
      Field.offset .ordinal = 11 ∧ Field.offset .effectCount = 12 ∧
      Field.offset .effectCount + Field.width .effectCount =
        Field.offset .headReserved := by
  native_decide

/-- The two canonical-zero spans, which are exactly the two `decode` refuses a
nonzero byte in: `(13, 3)` and `(496, 16)`.  The first was the bare arguments
of a `require_zero` call and the second a pair of `STATE_RESERVED_*`
constants. -/
theorem the_two_reserved_spans_are_the_ones_decode_enforces :
    schema.filter (fun field => isReserved field.kind) =
      [⟨.headReserved, .reserved 3⟩, ⟨.tailReserved, .reserved 16⟩] ∧
      Field.offset .headReserved = 13 ∧ Field.width .headReserved = 3 ∧
      Field.offset .tailReserved = 496 ∧ Field.width .tailReserved = 16 := by
  native_decide

/-- The head span pads the three tags to the identity block, and the tail span
closes the record, so no byte is unowned at either end. -/
theorem the_reserved_spans_pad_both_ends :
    Field.offset .headReserved + Field.width .headReserved =
        Field.offset .batch ∧
      identitiesOffset = Field.offset .batch ∧
      Field.offset .tailReserved =
        Field.offset .escrowAfter + Field.width .escrowAfter ∧
      Field.offset .tailReserved + Field.width .tailReserved =
        reservationStateBytes := by
  native_decide

/-- The fourteen identities are one contiguous run of full-width coordinates
ending exactly where the balances begin. -/
theorem the_identity_run_is_fourteen_full_coordinates :
    identities.length = 14 ∧
      schemaWidth identities = 14 * 32 ∧
      balancesOffset = Field.offset .amount ∧
      Field.offset .sourcePoststateDigest + Field.width .sourcePoststateDigest =
        Field.offset .amount := by
  native_decide

/-- The three tags are distinct, number from one, and every one indexes its own
bit of a `u8` bitset. -/
theorem the_tags_are_distinct_bit_indices :
    (Status.all.map Status.tag) = [1, 2, 3] ∧
      (Status.all.map Status.tag).Nodup ∧
      Status.all.all (fun status => 0 < Status.tag status) = true ∧
      Status.all.all (fun status => Status.tag status < statusLimit) = true ∧
      statusLimit = 4 ∧ statusLimit ≤ 8 := by
  native_decide

/-- `Active` is the only status in which the escrow still holds the reserved
amount, which is the canonicity rule `validate` enforces against
`escrow_after`. -/
theorem active_is_the_only_status_the_escrow_holds_in :
    Status.all.filter Status.escrowHolds = [.active] := by
  native_decide

theorem magic_is_eight_bytes : magic.length = 8 := by native_decide

theorem magic_fills_its_field : magic.length = Field.width .magic := by
  native_decide

theorem rust_names_are_distinct : (Field.all.map Field.rustName).Nodup := by
  native_decide

theorem status_rust_names_are_distinct : (Status.all.map Status.rustName).Nodup := by
  native_decide

theorem every_placed_field_is_named :
    Field.all = schema.map (fun field => field.name) := by native_decide

end DClutch.DealerScenarioReservationStateV1Abi
