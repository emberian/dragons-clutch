import DClutchSemantics.AbiCoverage

/-!
# The Dealer scenario checkpoint, and the five phases a split commit walks

Nine hundred and forty-four bytes of durable preparation state for a
lock-bounded Dealer scenario commit: bounded prepare transactions append
authenticated page receipts in canonical order, one selected accelerator
evaluation seals the best valid submitted candidate, and a final transaction
reauthenticates every mutable prestate before performing Claims and obligation
effects atomically.

`DealerScenarioCheckpointPhaseV1` was one of the four machines the route census
gates on with no Lean owner at all.  Its five discriminants were
`crates/dclutch-dealer-codec/src/scenario_checkpoint_v1.rs`'s, and so were all
thirty-one of its coordinates -- a file-private block of `const *_OFFSET:
usize` running from `8` to `816`, which is better than bare arguments and still
a second table that only agrees with the record by inspection.

## The two digest runs are what a width is made of

Twenty-three of the thirty-one fields are thirty-two-byte digests or
identities, and two of them are RUNS: six page receipts at 400 and four
reservation receipts at 816.  The Rust states each run's length in a `for`
loop's bound and its start in a `const`, and multiplies them nowhere, so
`944` was a number that had to be correct rather than one that could be
derived.  `the_page_run_is_six_receipts` and
`the_reservation_run_closes_the_record` are that arithmetic, and
`layout_covers_its_declared_width` is the whole tiling.

## The record has no reserved span, and that is a statement

Every one of its nine hundred and forty-four bytes is a named field.  A record
with no canonical-zero span has nowhere for an unowned byte to hide -- which is
the failure `AbiCoverage` exists to catch, since a reserved span looks like an
answer -- and `the_record_has_no_reserved_span` says so rather than leaving it
to be noticed.
-/

namespace DClutch.DealerScenarioCheckpointV1Abi

open DClutch.AbiSchema

/-- `DCLTDSC1`. -/
def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x44, 0x53, 0x43, 0x31]

/-- The implemented checkpoint schema version. -/
def schemaVersion : Nat := 1

/-- Maximum canonical preparation pages for one Dealer scenario. -/
def preparationPages : Nat := 6

/-- Reservation receipt slots the checkpoint carries, which is
`DEALER_SCENARIO_MAX_RESERVATIONS_V1`. -/
def reservationSlots : Nat := 4

/-- The width of one digest or content identity. -/
def digestBytes : Nat := 32

/-- The five phases one split Dealer commit walks. -/
inductive Phase where
  | collecting | evaluated | reserved | rollingBack | committed
  deriving DecidableEq, Repr

namespace Phase

def all : List Phase := [.collecting, .evaluated, .reserved, .rollingBack, .committed]

/-- The wire tag persisted in the phase byte. -/
def tag : Phase → Nat
  | .collecting => 1
  | .evaluated => 2
  | .reserved => 3
  | .rollingBack => 4
  | .committed => 5

def rustName : Phase → String
  | .collecting => "DEALER_SCENARIO_CHECKPOINT_PHASE_COLLECTING_V1"
  | .evaluated => "DEALER_SCENARIO_CHECKPOINT_PHASE_EVALUATED_V1"
  | .reserved => "DEALER_SCENARIO_CHECKPOINT_PHASE_RESERVED_V1"
  | .rollingBack => "DEALER_SCENARIO_CHECKPOINT_PHASE_ROLLING_BACK_V1"
  | .committed => "DEALER_SCENARIO_CHECKPOINT_PHASE_COMMITTED_V1"

def doc : Phase → String
  | .collecting => "Authenticated page receipts are still being collected."
  | .evaluated => "One admitted evaluation sealed the candidate and effect commitments."
  | .reserved => "Every selected Custody effect has a durable reservation receipt."
  | .rollingBack => "Expired reservations are being released in reverse order."
  | .committed =>
      "Claims and obligation liabilities committed against locked Custody value."

/-- The phases in which the candidate and effect commitments are already
sealed, which is what `validate` reads them for. -/
def sealed : Phase → Bool
  | .collecting => false
  | .evaluated | .reserved | .rollingBack | .committed => true

end Phase

/-- One past the greatest tag.  The machine numbers from one, so bit zero is
never occupied. -/
def phaseLimit : Nat := 6

inductive Field where
  | magic | schemaVersion | phase | pageCount | nextPage | effectCount
  | reservationCount | rollbackCount
  | revision | generation | createdSlot | expiresAt
  | releaseSet | market | childRoot | obligation | refundBeneficiary
  | requestDigest | rootPrestateDigest | claimsPrestateDigest
  | obligationPrestateDigest | custodyPrestateDigest
  | lastCheckpointPrestateDigest | pageReceiptDigests
  | evaluationReceiptDigest | candidateBankDigest | candidateObligationDigest
  | claimsDeltaDigest | effectsDigest | membershipManifestDigest
  | lastMembershipKey | reservationReceiptDigests
  deriving DecidableEq, Repr

/-- The header: magic, version, and the six one-byte counters the route reads
before it reads anything else. -/
def header : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.schemaVersion, .u16⟩,
  ⟨.phase, .u8⟩, ⟨.pageCount, .u8⟩, ⟨.nextPage, .u8⟩,
  ⟨.effectCount, .u8⟩, ⟨.reservationCount, .u8⟩, ⟨.rollbackCount, .u8⟩
]

/-- The four eight-byte lifecycle words: the replay revision, the Market
generation, and the slot window. -/
def clock : List (FieldSpec Field) := [
  ⟨.revision, .u64⟩, ⟨.generation, .u64⟩,
  ⟨.createdSlot, .u64⟩, ⟨.expiresAt, .u64⟩
]

/-- The scenario's identities and the prestates the final transaction
reauthenticates, ending with the run of six page receipts. -/
def input : List (FieldSpec Field) := [
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.childRoot, .bytes 32⟩,
  ⟨.obligation, .bytes 32⟩, ⟨.refundBeneficiary, .bytes 32⟩,
  ⟨.requestDigest, .bytes 32⟩,
  ⟨.rootPrestateDigest, .bytes 32⟩, ⟨.claimsPrestateDigest, .bytes 32⟩,
  ⟨.obligationPrestateDigest, .bytes 32⟩, ⟨.custodyPrestateDigest, .bytes 32⟩,
  ⟨.lastCheckpointPrestateDigest, .bytes 32⟩,
  ⟨.pageReceiptDigests, .bytes (preparationPages * digestBytes)⟩
]

/-- What one admitted evaluation seals, the effect commitment it produces, and
the run of four reservation receipts that closes the record. -/
def evaluation : List (FieldSpec Field) := [
  ⟨.evaluationReceiptDigest, .bytes 32⟩, ⟨.candidateBankDigest, .bytes 32⟩,
  ⟨.candidateObligationDigest, .bytes 32⟩, ⟨.claimsDeltaDigest, .bytes 32⟩,
  ⟨.effectsDigest, .bytes 32⟩, ⟨.membershipManifestDigest, .bytes 32⟩,
  ⟨.lastMembershipKey, .bytes 32⟩,
  ⟨.reservationReceiptDigests, .bytes (reservationSlots * digestBytes)⟩
]

def schema : List (FieldSpec Field) := header ++ clock ++ input ++ evaluation

def layout : List (PlacedField Field) := specialize schema
def checkpointBytes : Nat := schemaWidth schema

/-- Where the four lifecycle words begin: the width of the header in front of
them. -/
def clockOffset : Nat := schemaWidth header
/-- Where the scenario's identities begin. -/
def inputOffset : Nat := schemaWidth header + schemaWidth clock
/-- Where the evaluation's commitments begin. -/
def evaluationOffset : Nat :=
  schemaWidth header + schemaWidth clock + schemaWidth input

namespace Field

def all : List Field := [
  .magic, .schemaVersion, .phase, .pageCount, .nextPage, .effectCount,
  .reservationCount, .rollbackCount,
  .revision, .generation, .createdSlot, .expiresAt,
  .releaseSet, .market, .childRoot, .obligation, .refundBeneficiary,
  .requestDigest, .rootPrestateDigest, .claimsPrestateDigest,
  .obligationPrestateDigest, .custodyPrestateDigest,
  .lastCheckpointPrestateDigest, .pageReceiptDigests,
  .evaluationReceiptDigest, .candidateBankDigest, .candidateObligationDigest,
  .claimsDeltaDigest, .effectsDigest, .membershipManifestDigest,
  .lastMembershipKey, .reservationReceiptDigests
]

def rustName : Field → String
  | .magic => "DEALER_SCENARIO_CHECKPOINT_MAGIC_OFFSET_V1"
  | .schemaVersion => "DEALER_SCENARIO_CHECKPOINT_VERSION_OFFSET_V1"
  | .phase => "DEALER_SCENARIO_CHECKPOINT_PHASE_OFFSET_V1"
  | .pageCount => "DEALER_SCENARIO_CHECKPOINT_PAGE_COUNT_OFFSET_V1"
  | .nextPage => "DEALER_SCENARIO_CHECKPOINT_NEXT_PAGE_OFFSET_V1"
  | .effectCount => "DEALER_SCENARIO_CHECKPOINT_EFFECT_COUNT_OFFSET_V1"
  | .reservationCount => "DEALER_SCENARIO_CHECKPOINT_RESERVATION_COUNT_OFFSET_V1"
  | .rollbackCount => "DEALER_SCENARIO_CHECKPOINT_ROLLBACK_COUNT_OFFSET_V1"
  | .revision => "DEALER_SCENARIO_CHECKPOINT_REVISION_OFFSET_V1"
  | .generation => "DEALER_SCENARIO_CHECKPOINT_GENERATION_OFFSET_V1"
  | .createdSlot => "DEALER_SCENARIO_CHECKPOINT_CREATED_SLOT_OFFSET_V1"
  | .expiresAt => "DEALER_SCENARIO_CHECKPOINT_EXPIRES_AT_OFFSET_V1"
  | .releaseSet => "DEALER_SCENARIO_CHECKPOINT_RELEASE_SET_OFFSET_V1"
  | .market => "DEALER_SCENARIO_CHECKPOINT_MARKET_OFFSET_V1"
  | .childRoot => "DEALER_SCENARIO_CHECKPOINT_CHILD_ROOT_OFFSET_V1"
  | .obligation => "DEALER_SCENARIO_CHECKPOINT_OBLIGATION_OFFSET_V1"
  | .refundBeneficiary => "DEALER_SCENARIO_CHECKPOINT_REFUND_BENEFICIARY_OFFSET_V1"
  | .requestDigest => "DEALER_SCENARIO_CHECKPOINT_REQUEST_DIGEST_OFFSET_V1"
  | .rootPrestateDigest => "DEALER_SCENARIO_CHECKPOINT_ROOT_PRESTATE_DIGEST_OFFSET_V1"
  | .claimsPrestateDigest => "DEALER_SCENARIO_CHECKPOINT_CLAIMS_PRESTATE_DIGEST_OFFSET_V1"
  | .obligationPrestateDigest =>
      "DEALER_SCENARIO_CHECKPOINT_OBLIGATION_PRESTATE_DIGEST_OFFSET_V1"
  | .custodyPrestateDigest => "DEALER_SCENARIO_CHECKPOINT_CUSTODY_PRESTATE_DIGEST_OFFSET_V1"
  | .lastCheckpointPrestateDigest =>
      "DEALER_SCENARIO_CHECKPOINT_LAST_PRESTATE_DIGEST_OFFSET_V1"
  | .pageReceiptDigests => "DEALER_SCENARIO_CHECKPOINT_PAGE_RECEIPT_DIGESTS_OFFSET_V1"
  | .evaluationReceiptDigest =>
      "DEALER_SCENARIO_CHECKPOINT_EVALUATION_RECEIPT_DIGEST_OFFSET_V1"
  | .candidateBankDigest => "DEALER_SCENARIO_CHECKPOINT_CANDIDATE_BANK_DIGEST_OFFSET_V1"
  | .candidateObligationDigest =>
      "DEALER_SCENARIO_CHECKPOINT_CANDIDATE_OBLIGATION_DIGEST_OFFSET_V1"
  | .claimsDeltaDigest => "DEALER_SCENARIO_CHECKPOINT_CLAIMS_DELTA_DIGEST_OFFSET_V1"
  | .effectsDigest => "DEALER_SCENARIO_CHECKPOINT_EFFECTS_DIGEST_OFFSET_V1"
  | .membershipManifestDigest =>
      "DEALER_SCENARIO_CHECKPOINT_MEMBERSHIP_MANIFEST_DIGEST_OFFSET_V1"
  | .lastMembershipKey => "DEALER_SCENARIO_CHECKPOINT_LAST_MEMBERSHIP_KEY_OFFSET_V1"
  | .reservationReceiptDigests =>
      "DEALER_SCENARIO_CHECKPOINT_RESERVATION_RECEIPT_DIGESTS_OFFSET_V1"

def doc : Field → String
  | .magic => "Canonical checkpoint magic."
  | .schemaVersion => "This record's ABI version coordinate."
  | .phase => "The persisted `DealerScenarioCheckpointPhaseV1` wire tag."
  | .pageCount => "Declared preparation page count; exactly the emitted maximum."
  | .nextPage => "Next canonical preparation page this checkpoint will admit."
  | .effectCount => "Custody effects the admitted evaluation selected."
  | .reservationCount => "Durable reservation receipts recorded so far."
  | .rollbackCount => "Reservations released so far, in reverse order."
  | .revision => "Replay revision every mutating transaction advances."
  | .generation => "Market generation this scenario was authenticated against."
  | .createdSlot => "Slot the checkpoint was opened at."
  | .expiresAt => "Slot after which preparation refuses and rollback opens."
  | .releaseSet => "Release set the whole scenario executes under."
  | .market => "Market the scenario trades against."
  | .childRoot => "Dealer child root the scenario drives."
  | .obligation => "Obligation account the commit writes liabilities to."
  | .refundBeneficiary => "Account the checkpoint's rent returns to."
  | .requestDigest => "Digest of the request that opened this checkpoint."
  | .rootPrestateDigest => "Root prestate the final transaction reauthenticates."
  | .claimsPrestateDigest => "Joined Claims prestate for preparation and commit."
  | .obligationPrestateDigest => "Obligation prestate the final transaction reauthenticates."
  | .custodyPrestateDigest => "Joined Custody prestate for preparation and commit."
  | .lastCheckpointPrestateDigest => "Digest of this checkpoint's own last accepted prestate."
  | .pageReceiptDigests => "The run of authenticated page receipts, in canonical order."
  | .evaluationReceiptDigest => "Digest of the one selected accelerator evaluation receipt."
  | .candidateBankDigest => "Bank commitment the admitted evaluation sealed."
  | .candidateObligationDigest => "Obligation commitment the admitted evaluation sealed."
  | .claimsDeltaDigest => "Claims delta the commit applies."
  | .effectsDigest => "Ordered active Custody effect commitment."
  | .membershipManifestDigest => "Digest of the producer-owned membership manifest."
  | .lastMembershipKey => "Last membership key admitted in canonical order."
  | .reservationReceiptDigests => "The run of durable reservation receipts, one per effect slot."

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

/-- The thirty-two fields cover the nine hundred and forty-four bytes every
reader allocates: no gap, and the last field ends exactly at the declared
width. -/
theorem layout_covers_its_declared_width :
    checkpointBytes = 944 ∧ tiles 0 layout 944 = true := by
  native_decide

/-- Every coordinate, against the file-private offset table this replaces. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2),
    (.phase, 10, 1), (.pageCount, 11, 1), (.nextPage, 12, 1),
    (.effectCount, 13, 1), (.reservationCount, 14, 1), (.rollbackCount, 15, 1),
    (.revision, 16, 8), (.generation, 24, 8),
    (.createdSlot, 32, 8), (.expiresAt, 40, 8),
    (.releaseSet, 48, 32), (.market, 80, 32), (.childRoot, 112, 32),
    (.obligation, 144, 32), (.refundBeneficiary, 176, 32),
    (.requestDigest, 208, 32), (.rootPrestateDigest, 240, 32),
    (.claimsPrestateDigest, 272, 32), (.obligationPrestateDigest, 304, 32),
    (.custodyPrestateDigest, 336, 32),
    (.lastCheckpointPrestateDigest, 368, 32), (.pageReceiptDigests, 400, 192),
    (.evaluationReceiptDigest, 592, 32), (.candidateBankDigest, 624, 32),
    (.candidateObligationDigest, 656, 32), (.claimsDeltaDigest, 688, 32),
    (.effectsDigest, 720, 32), (.membershipManifestDigest, 752, 32),
    (.lastMembershipKey, 784, 32), (.reservationReceiptDigests, 816, 128)
  ] := by
  native_decide

/-- The phase byte begins exactly where the version word ends and is one byte
wide, and the five counters that follow it are contiguous single bytes ending
at the replay revision. -/
theorem the_phase_heads_the_counter_run :
    Field.offset .phase = Field.offset .schemaVersion + Field.width .schemaVersion ∧
      Field.width .phase = 1 ∧ Field.offset .phase = 10 ∧
      Field.offset .rollbackCount + Field.width .rollbackCount =
        Field.offset .revision ∧
      clockOffset = Field.offset .revision := by
  native_decide

/-- **The record has no reserved span.**  Every one of its nine hundred and
forty-four bytes is a named field, so there is nowhere for an unowned byte to
hide -- which is the exact failure a reserved span disguises. -/
theorem the_record_has_no_reserved_span :
    schema.filter (fun field => isReserved field.kind) = [] := by
  native_decide

/-- The page receipt run is exactly six digests, and it ends where the
evaluation's commitments begin.  The Rust states the six in a `for` loop bound
and the 400 in a `const`, and multiplies them nowhere. -/
theorem the_page_run_is_six_receipts :
    Field.width .pageReceiptDigests = preparationPages * digestBytes ∧
      preparationPages = 6 ∧ digestBytes = 32 ∧
      Field.offset .pageReceiptDigests + Field.width .pageReceiptDigests =
        Field.offset .evaluationReceiptDigest ∧
      evaluationOffset = Field.offset .evaluationReceiptDigest := by
  native_decide

/-- The reservation receipt run is exactly four digests and it closes the
record, so the width is `816 + 4 * 32` rather than a number somebody typed. -/
theorem the_reservation_run_closes_the_record :
    Field.width .reservationReceiptDigests = reservationSlots * digestBytes ∧
      reservationSlots = 4 ∧
      Field.offset .reservationReceiptDigests +
          Field.width .reservationReceiptDigests = checkpointBytes := by
  native_decide

/-- The five tags are distinct, number from one, and every one indexes its own
bit of a `u8` bitset. -/
theorem the_tags_are_distinct_bit_indices :
    (Phase.all.map Phase.tag) = [1, 2, 3, 4, 5] ∧
      (Phase.all.map Phase.tag).Nodup ∧
      Phase.all.all (fun phase => 0 < Phase.tag phase) = true ∧
      Phase.all.all (fun phase => Phase.tag phase < phaseLimit) = true ∧
      phaseLimit = 6 ∧ phaseLimit ≤ 8 := by
  native_decide

/-- `Collecting` is the only phase in which the candidate and effect
commitments are not yet sealed, which is what every guard reading this machine
is actually asking. -/
theorem collecting_is_the_only_unsealed_phase :
    Phase.all.filter (fun phase => !Phase.sealed phase) = [.collecting] := by
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

end DClutch.DealerScenarioCheckpointV1Abi
