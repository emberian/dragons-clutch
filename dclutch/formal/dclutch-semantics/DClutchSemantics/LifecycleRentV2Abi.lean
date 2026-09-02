import DClutchSemantics.AbiCoverage
import DClutchSemantics.MarketRetirementV1Abi

/-!
# Lifecycle rent V2 ABI

One Market-scoped rent credit, the three instructions that create, sweep and
close it, and the receipt the close produces.  Six records, thirty offsets, two
seed domains, three magics and an action alphabet, all of which
`crates/dclutch-rent-contract/src/lifecycle_v2.rs` wrote for itself.

The browser's position was worse than transcription.  `generate-core-found.mjs`
could not find the magic's coordinate as a constant, because there was none, so
it read the ENCODER'S CALL: a regular expression over `put(&mut output, 0,
&LIFECYCLE_RENT_CREDIT_MAGIC_V2)`, recovering an offset from a function
argument. And the action byte was worse still: the browser had `0` written by
hand where `LifecycleRentActionV2::Create` is `1`, so every lifecycle RentCredit
it built was refused at decode, unnoticed because the console only ever
downloaded the packet.

The three instructions share a sixteen-byte prologue -- magic, version, action,
five reserved bytes -- and `every_instruction_begins_with_the_prologue` says so
rather than leaving it to be true three times.  The Close request's payload is a
Core retirement receipt, so its width is `MarketRetirementV1Abi.coreReceiptBytes`
and not a `512` that happens to agree.
-/

namespace DClutch.LifecycleRentV2Abi

open DClutch.AbiSchema

def version : Nat := 2

/-- `DCLRNTL2` -- the persisted credit. -/
def creditMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x52, 0x4e, 0x54, 0x4c, 0x32]
/-- `DCLRNCI2` -- every instruction. -/
def instructionMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x52, 0x4e, 0x43, 0x49, 0x32]
/-- `DCLRNCR2` -- the close receipt. -/
def closeReceiptMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x52, 0x4e, 0x43, 0x52, 0x32]

/-- `[creditPdaDomain, market, release_set]`. -/
def creditPdaDomain : String := "dclutch/rent-market/v2"
/-- `[coreCloseAuthorityDomain, market, release_set]`. -/
def coreCloseAuthorityDomain : String := "dclutch/rent-core-close/v2"

inductive Action where
  | create | sweep | close
  deriving DecidableEq, Repr

namespace Action

def all : List Action := [.create, .sweep, .close]

def tag : Action → Nat
  | .create => 1
  | .sweep => 2
  | .close => 3

def rustName : Action → String
  | .create => "LIFECYCLE_RENT_ACTION_CREATE_V2"
  | .sweep => "LIFECYCLE_RENT_ACTION_SWEEP_V2"
  | .close => "LIFECYCLE_RENT_ACTION_CLOSE_V2"

def doc : Action → String
  | .create => "Wire tag: create one Market-scoped credit."
  | .sweep => "Wire tag: sweep surplus to the immutable wallet, preserving Rent."
  | .close => "Wire tag: close after complete producer-subtree retirement."

end Action

/-! ## The persisted credit -/

inductive StateField where
  | magic | version | bump | reservedHeader | refundWallet | market
  | releaseSet | generation | reservedBody
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.bump, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.refundWallet, .bytes 32⟩,
  ⟨.market, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩, ⟨.generation, .u64⟩,
  ⟨.reservedBody, .reserved 8⟩
]

def stateLayout : List (PlacedField StateField) := specialize stateSchema
def stateBytes : Nat := schemaWidth stateSchema

namespace StateField
def all : List StateField := [
  .magic, .version, .bump, .reservedHeader, .refundWallet, .market,
  .releaseSet, .generation, .reservedBody
]
def rustName : StateField → String
  | .magic => "STATE_MAGIC_OFFSET"
  | .version => "STATE_VERSION_OFFSET"
  | .bump => "STATE_BUMP_OFFSET"
  | .reservedHeader => "STATE_RESERVED_HEADER_OFFSET"
  | .refundWallet => "STATE_REFUND_WALLET_OFFSET"
  | .market => "STATE_MARKET_OFFSET"
  | .releaseSet => "STATE_RELEASE_SET_OFFSET"
  | .generation => "STATE_GENERATION_OFFSET"
  | .reservedBody => "STATE_RESERVED_BODY_OFFSET"
def coordinate (field : StateField) : Nat × Nat :=
  (coordinate? field stateLayout).getD (0, 0)
def offset (field : StateField) : Nat := (coordinate field).1
def width (field : StateField) : Nat := (coordinate field).2
end StateField

/-! ## The instruction prologue, and the three instructions -/

inductive InstructionField where
  | magic | version | action | reserved
  deriving DecidableEq, Repr

def instructionSchema : List (FieldSpec InstructionField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.reserved, .reserved 5⟩
]

def instructionLayout : List (PlacedField InstructionField) := specialize instructionSchema
def instructionHeaderBytes : Nat := schemaWidth instructionSchema

namespace InstructionField
def all : List InstructionField := [.magic, .version, .action, .reserved]
def rustName : InstructionField → String
  | .magic => "INSTRUCTION_MAGIC_OFFSET"
  | .version => "INSTRUCTION_VERSION_OFFSET"
  | .action => "INSTRUCTION_ACTION_OFFSET"
  | .reserved => "INSTRUCTION_RESERVED_OFFSET"
def coordinate (field : InstructionField) : Nat × Nat :=
  (coordinate? field instructionLayout).getD (0, 0)
def offset (field : InstructionField) : Nat := (coordinate field).1
def width (field : InstructionField) : Nat := (coordinate field).2
end InstructionField

/-- The prologue as a field list every instruction reuses.  The names are the
instruction ones; only a record's OWN fields are emitted per record, so no
coordinate is printed twice under two names. -/
def instructionPrologue : List (FieldSpec InstructionField) := instructionSchema

inductive CreateField where
  | magic | version | action | reserved
  | refundWallet | market | releaseSet | generation | bump | createReserved
  deriving DecidableEq, Repr

def createSchema : List (FieldSpec CreateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.reserved, .reserved 5⟩,
  ⟨.refundWallet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.bump, .u8⟩, ⟨.createReserved, .reserved 7⟩
]

def createLayout : List (PlacedField CreateField) := specialize createSchema
def createBytes : Nat := schemaWidth createSchema

namespace CreateField
def body : List CreateField :=
  [.refundWallet, .market, .releaseSet, .generation, .bump, .createReserved]
def rustName : CreateField → String
  | .magic => "INSTRUCTION_MAGIC_OFFSET"
  | .version => "INSTRUCTION_VERSION_OFFSET"
  | .action => "INSTRUCTION_ACTION_OFFSET"
  | .reserved => "INSTRUCTION_RESERVED_OFFSET"
  | .refundWallet => "CREATE_REFUND_WALLET_OFFSET"
  | .market => "CREATE_MARKET_OFFSET"
  | .releaseSet => "CREATE_RELEASE_SET_OFFSET"
  | .generation => "CREATE_GENERATION_OFFSET"
  | .bump => "CREATE_BUMP_OFFSET"
  | .createReserved => "CREATE_RESERVED_OFFSET"
def coordinate (field : CreateField) : Nat × Nat :=
  (coordinate? field createLayout).getD (0, 0)
def offset (field : CreateField) : Nat := (coordinate field).1
def width (field : CreateField) : Nat := (coordinate field).2
end CreateField

inductive SweepField where
  | magic | version | action | reserved | amount
  deriving DecidableEq, Repr

def sweepSchema : List (FieldSpec SweepField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.reserved, .reserved 5⟩,
  ⟨.amount, .u64⟩
]

def sweepLayout : List (PlacedField SweepField) := specialize sweepSchema
def sweepBytes : Nat := schemaWidth sweepSchema

namespace SweepField
def body : List SweepField := [.amount]
def rustName : SweepField → String
  | .magic => "INSTRUCTION_MAGIC_OFFSET"
  | .version => "INSTRUCTION_VERSION_OFFSET"
  | .action => "INSTRUCTION_ACTION_OFFSET"
  | .reserved => "INSTRUCTION_RESERVED_OFFSET"
  | .amount => "SWEEP_AMOUNT_OFFSET"
def coordinate (field : SweepField) : Nat × Nat :=
  (coordinate? field sweepLayout).getD (0, 0)
def offset (field : SweepField) : Nat := (coordinate field).1
def width (field : SweepField) : Nat := (coordinate field).2
end SweepField

inductive CloseField where
  | magic | version | action | reserved | receipt
  deriving DecidableEq, Repr

/-- The Close payload is a Core retirement receipt, so its width comes from the
module that owns that record rather than from a `512` repeated here. -/
def closeSchema : List (FieldSpec CloseField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.reserved, .reserved 5⟩,
  ⟨.receipt, .nested DClutch.MarketRetirementV1Abi.coreReceiptBytes⟩
]

def closeLayout : List (PlacedField CloseField) := specialize closeSchema
def closeBytes : Nat := schemaWidth closeSchema

namespace CloseField
def body : List CloseField := [.receipt]
def rustName : CloseField → String
  | .magic => "INSTRUCTION_MAGIC_OFFSET"
  | .version => "INSTRUCTION_VERSION_OFFSET"
  | .action => "INSTRUCTION_ACTION_OFFSET"
  | .reserved => "INSTRUCTION_RESERVED_OFFSET"
  | .receipt => "CLOSE_RECEIPT_OFFSET"
def coordinate (field : CloseField) : Nat × Nat :=
  (coordinate? field closeLayout).getD (0, 0)
def offset (field : CloseField) : Nat := (coordinate field).1
def width (field : CloseField) : Nat := (coordinate field).2
end CloseField

/-! ## The close receipt -/

inductive ReceiptField where
  | magic | version | kind | reservedHeader | credit | refundWallet | market
  | releaseSet | postResourceDigest | generation | closedLamports
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.credit, .bytes 32⟩,
  ⟨.refundWallet, .bytes 32⟩, ⟨.market, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.postResourceDigest, .bytes 32⟩, ⟨.generation, .u64⟩,
  ⟨.closedLamports, .u64⟩
]

def receiptLayout : List (PlacedField ReceiptField) := specialize receiptSchema
def receiptBytes : Nat := schemaWidth receiptSchema

namespace ReceiptField
def all : List ReceiptField := [
  .magic, .version, .kind, .reservedHeader, .credit, .refundWallet, .market,
  .releaseSet, .postResourceDigest, .generation, .closedLamports
]
def rustName : ReceiptField → String
  | .magic => "RECEIPT_MAGIC_OFFSET"
  | .version => "RECEIPT_VERSION_OFFSET"
  | .kind => "RECEIPT_KIND_OFFSET"
  | .reservedHeader => "RECEIPT_RESERVED_HEADER_OFFSET"
  | .credit => "RECEIPT_CREDIT_OFFSET"
  | .refundWallet => "RECEIPT_REFUND_WALLET_OFFSET"
  | .market => "RECEIPT_MARKET_OFFSET"
  | .releaseSet => "RECEIPT_RELEASE_SET_OFFSET"
  | .postResourceDigest => "RECEIPT_POST_RESOURCE_DIGEST_OFFSET"
  | .generation => "RECEIPT_GENERATION_OFFSET"
  | .closedLamports => "RECEIPT_CLOSED_LAMPORTS_OFFSET"
def coordinate (field : ReceiptField) : Nat × Nat :=
  (coordinate? field receiptLayout).getD (0, 0)
def offset (field : ReceiptField) : Nat := (coordinate field).1
def width (field : ReceiptField) : Nat := (coordinate field).2
end ReceiptField

/-! ## What the layouts say -/

theorem every_record_covers_its_declared_width :
    (stateBytes = 128 ∧ tiles 0 stateLayout 128) ∧
    (instructionHeaderBytes = 16 ∧ tiles 0 instructionLayout 16) ∧
    (createBytes = 128 ∧ tiles 0 createLayout 128) ∧
    (sweepBytes = 24 ∧ tiles 0 sweepLayout 24) ∧
    (closeBytes = 528 ∧ tiles 0 closeLayout 528) ∧
    (receiptBytes = 192 ∧ tiles 0 receiptLayout 192) := by
  native_decide

/-- All three instructions begin with the same four placements, which is the
fact `put_instruction_header` implements and `require_instruction` relies on. -/
theorem every_instruction_begins_with_the_prologue :
    (createLayout.take 4).map (fun f => (f.offset, f.spec.kind.byteWidth)) =
      (instructionLayout.map fun f => (f.offset, f.spec.kind.byteWidth)) ∧
    (sweepLayout.take 4).map (fun f => (f.offset, f.spec.kind.byteWidth)) =
      (instructionLayout.map fun f => (f.offset, f.spec.kind.byteWidth)) ∧
    (closeLayout.take 4).map (fun f => (f.offset, f.spec.kind.byteWidth)) =
      (instructionLayout.map fun f => (f.offset, f.spec.kind.byteWidth)) := by
  native_decide

/-- Every instruction body begins exactly where the prologue ends, so a record's
own fields never reach into the header. -/
theorem instruction_bodies_start_at_the_prologue_width :
    CreateField.offset .refundWallet = instructionHeaderBytes ∧
    SweepField.offset .amount = instructionHeaderBytes ∧
    CloseField.offset .receipt = instructionHeaderBytes := by
  native_decide

/-- The Close request carries a whole Core retirement receipt: its width is that
record's width plus the prologue, not a number of its own. -/
theorem close_request_is_the_prologue_plus_a_core_receipt :
    closeBytes = instructionHeaderBytes + DClutch.MarketRetirementV1Abi.coreReceiptBytes := by
  native_decide

theorem schemas_are_well_formed :
    (stateSchema.map (fun f => f.name)).Nodup ∧
    (instructionSchema.map (fun f => f.name)).Nodup ∧
    (createSchema.map (fun f => f.name)).Nodup ∧
    (sweepSchema.map (fun f => f.name)).Nodup ∧
    (closeSchema.map (fun f => f.name)).Nodup ∧
    (receiptSchema.map (fun f => f.name)).Nodup := by
  native_decide

theorem state_layout_disjoint : stateLayout.Pairwise Before :=
  specializeFrom_pairwise 0 stateSchema
theorem create_layout_disjoint : createLayout.Pairwise Before :=
  specializeFrom_pairwise 0 createSchema
theorem receipt_layout_disjoint : receiptLayout.Pairwise Before :=
  specializeFrom_pairwise 0 receiptSchema

theorem record_magics_are_pairwise_distinct :
    [creditMagic, instructionMagic, closeReceiptMagic].Nodup := by native_decide

theorem seed_domains_are_admissible_and_distinct :
    [creditPdaDomain, coreCloseAuthorityDomain].Nodup ∧
    creditPdaDomain.toUTF8.toList.length <= 32 ∧
    coreCloseAuthorityDomain.toUTF8.toList.length <= 32 := by native_decide

/-- The action alphabet is distinct and, critically, starts at ONE.  Zero is not
an action: a caller that leaves the byte unwritten is refused rather than
silently creating a credit, and a browser that guessed `0` was refused at every
attempt. -/
theorem action_tags_are_distinct_and_nonzero :
    (Action.all.map Action.tag).Nodup ∧
    Action.all.all (fun action => Action.tag action != 0) := by
  native_decide

theorem state_coordinates_are_canonical : coordinates stateLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.bump, 10, 1), (.reservedHeader, 11, 5),
    (.refundWallet, 16, 32), (.market, 48, 32), (.releaseSet, 80, 32),
    (.generation, 112, 8), (.reservedBody, 120, 8)
  ] := by native_decide

theorem receipt_coordinates_are_canonical : coordinates receiptLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.kind, 10, 1), (.reservedHeader, 11, 5),
    (.credit, 16, 32), (.refundWallet, 48, 32), (.market, 80, 32),
    (.releaseSet, 112, 32), (.postResourceDigest, 144, 32),
    (.generation, 176, 8), (.closedLamports, 184, 8)
  ] := by native_decide

end DClutch.LifecycleRentV2Abi
