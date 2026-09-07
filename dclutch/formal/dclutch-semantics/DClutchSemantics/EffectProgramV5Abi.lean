import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# EffectProgram V5 / DCE6 fixed ABI assurance

This module owns the DCE6 funding successor's fixed header, its funding-action
record and its funding-seed record, and the three funding operations the record
admits. `Create` tops up, allocates and assigns one vacant System account;
`Close` drains one live Trading-owned account; `Fund` tops one live
Trading-owned account up to an exact target register through the System
program, debiting the signing payer -- the movement a work escrow needs and a
local lamport transfer out of a System-owned payer cannot make
(`ExternalAccountLamportSpend`, measured on SubmitCandidate 2026-09-04).

The safe Rust kernel (`dclutch_vm::effect::v5`) remains the executable owner of
hostile decoding, table ordering, seed resolution and the embedded DCE5 base.
Lean emits constants, the per-operation shape predicate, and canonical and
hostile byte witnesses for the differential translation test.
-/

namespace DClutch.EffectProgramV5Abi

open DClutch.AbiSchema

def magic : List UInt8 := "DCE6".toUTF8.toList
def version : Nat := 6
def maxFundingActions : Nat := 16
def maxFundingSeeds : Nat := 64
def maxActionSeeds : Nat := 16
def unusedCoordinate : Nat := 65535

def opcodeCreate : Nat := 0
def opcodeClose : Nat := 1
def opcodeFund : Nat := 2

def seedLiteral : Nat := 0
def seedCommonScalar : Nat := 1
def seedCommonIdentity : Nat := 2
def seedCanonicalBump : Nat := 3

def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/effect-program-v6-funding-account-lifecycle-v1".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x68, 0xdf, 0x4b, 0x64, 0xc6, 0xa7, 0x1c, 0xc4,
  0x40, 0xc5, 0x12, 0xeb, 0xf0, 0x87, 0x24, 0x5c,
  0xb9, 0xb3, 0x18, 0x82, 0x21, 0xf3, 0xee, 0x86,
  0x97, 0xc1, 0x45, 0x8b, 0x27, 0x26, 0x06, 0x84
]

inductive HeaderField where
  | magic | version | reservedByte | actionCount | seedCount | reservedWord
  | baseBytes | reservedTail
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 4⟩, ⟨.version, .u8⟩, ⟨.reservedByte, .reserved 1⟩,
  ⟨.actionCount, .u16⟩, ⟨.seedCount, .u16⟩, ⟨.reservedWord, .reserved 2⟩,
  ⟨.baseBytes, .u32⟩, ⟨.reservedTail, .reserved 16⟩
]

inductive ActionField where
  | operation | seedCount | state | counterparty | refundDestination
  | systemProgram | lamportsScalar | refundOwnerIdentity | seedStart | liveBytes
  | reserved
  deriving DecidableEq, Repr

def actionSchema : List (FieldSpec ActionField) := [
  ⟨.operation, .u8⟩, ⟨.seedCount, .u8⟩, ⟨.state, .u16⟩, ⟨.counterparty, .u16⟩,
  ⟨.refundDestination, .u16⟩, ⟨.systemProgram, .u16⟩, ⟨.lamportsScalar, .u16⟩,
  ⟨.refundOwnerIdentity, .u16⟩, ⟨.seedStart, .u16⟩, ⟨.liveBytes, .u32⟩,
  ⟨.reserved, .reserved 4⟩
]

inductive SeedField where
  | kind | width | source | payload | reserved
  deriving DecidableEq, Repr

def seedSchema : List (FieldSpec SeedField) := [
  ⟨.kind, .u8⟩, ⟨.width, .u8⟩, ⟨.source, .u16⟩, ⟨.payload, .bytes 32⟩,
  ⟨.reserved, .reserved 4⟩
]

def headerLayout := specialize headerSchema
def actionLayout := specialize actionSchema
def seedLayout := specialize seedSchema
def headerBytes := schemaWidth headerSchema
def actionBytes := schemaWidth actionSchema
def seedBytes := schemaWidth seedSchema

namespace HeaderField
def rustName : HeaderField → String
  | .magic => "EFFECT_V5_MAGIC_OFFSET"
  | .version => "EFFECT_V5_VERSION_OFFSET"
  | .reservedByte => "EFFECT_V5_RESERVED_BYTE_OFFSET"
  | .actionCount => "EFFECT_V5_ACTION_COUNT_OFFSET"
  | .seedCount => "EFFECT_V5_SEED_COUNT_OFFSET"
  | .reservedWord => "EFFECT_V5_RESERVED_WORD_OFFSET"
  | .baseBytes => "EFFECT_V5_BASE_BYTES_OFFSET"
  | .reservedTail => "EFFECT_V5_RESERVED_TAIL_OFFSET"
end HeaderField

namespace ActionField
def rustName : ActionField → String
  | .operation => "EFFECT_V5_ACTION_OPERATION_OFFSET"
  | .seedCount => "EFFECT_V5_ACTION_SEED_COUNT_OFFSET"
  | .state => "EFFECT_V5_ACTION_STATE_OFFSET"
  | .counterparty => "EFFECT_V5_ACTION_COUNTERPARTY_OFFSET"
  | .refundDestination => "EFFECT_V5_ACTION_REFUND_DESTINATION_OFFSET"
  | .systemProgram => "EFFECT_V5_ACTION_SYSTEM_PROGRAM_OFFSET"
  | .lamportsScalar => "EFFECT_V5_ACTION_LAMPORTS_SCALAR_OFFSET"
  | .refundOwnerIdentity => "EFFECT_V5_ACTION_REFUND_OWNER_IDENTITY_OFFSET"
  | .seedStart => "EFFECT_V5_ACTION_SEED_START_OFFSET"
  | .liveBytes => "EFFECT_V5_ACTION_LIVE_BYTES_OFFSET"
  | .reserved => "EFFECT_V5_ACTION_RESERVED_OFFSET"
end ActionField

namespace SeedField
def rustName : SeedField → String
  | .kind => "EFFECT_V5_SEED_KIND_OFFSET"
  | .width => "EFFECT_V5_SEED_WIDTH_OFFSET"
  | .source => "EFFECT_V5_SEED_SOURCE_OFFSET"
  | .payload => "EFFECT_V5_SEED_PAYLOAD_OFFSET"
  | .reserved => "EFFECT_V5_SEED_RESERVED_OFFSET"
end SeedField

/-! ## The per-operation shape, as one predicate -/

structure Action where
  operation : Nat
  seedCount : Nat
  state : Nat
  counterparty : Nat
  refundDestination : Nat
  systemProgram : Nat
  lamportsScalar : Nat
  refundOwnerIdentity : Nat
  seedStart : Nat
  liveBytes : Nat
  deriving DecidableEq, Repr

/-- The conjuncts every operation shares: coordinates present, distinct, and
the two parties never the same account. -/
def commonShape (action : Action) : Bool :=
  action.state ≠ 0 && action.state ≠ unusedCoordinate &&
    action.counterparty ≠ 0 && action.counterparty ≠ unusedCoordinate &&
    action.state ≠ action.counterparty

def createShape (action : Action) : Bool :=
  action.operation = opcodeCreate && commonShape action &&
    action.seedCount > 0 && action.seedCount ≤ maxActionSeeds && action.liveBytes > 0 &&
    action.refundDestination ≠ unusedCoordinate && action.refundDestination ≠ 0 &&
    action.systemProgram ≠ unusedCoordinate && action.systemProgram ≠ 0 &&
    action.state ≠ action.refundDestination && action.state ≠ action.systemProgram &&
    action.counterparty ≠ action.refundDestination &&
    action.counterparty ≠ action.systemProgram &&
    action.refundDestination ≠ action.systemProgram

def closeShape (action : Action) : Bool :=
  action.operation = opcodeClose && commonShape action &&
    action.seedCount = 0 && action.seedStart = 0 && action.liveBytes = 0 &&
    action.refundDestination = unusedCoordinate && action.systemProgram = unusedCoordinate

/-- A Fund derives no address (no seeds), allocates nothing (no width), refunds
nothing (no refund destination) and moves lamports by CPI (a System program
distinct from both parties). -/
def fundShape (action : Action) : Bool :=
  action.operation = opcodeFund && commonShape action &&
    action.seedCount = 0 && action.seedStart = 0 && action.liveBytes = 0 &&
    action.refundDestination = unusedCoordinate &&
    action.systemProgram ≠ unusedCoordinate && action.systemProgram ≠ 0 &&
    action.systemProgram ≠ action.state && action.systemProgram ≠ action.counterparty

def valid (action : Action) : Bool :=
  createShape action || closeShape action || fundShape action

/-- General's candidate work-escrow top-up: the primary state at 5, the solver
payer at 6, the System program at 11, the target in common scalar 93
(`scalar::SCRATCH_B`, rent principal plus work capacity), the refund owner in
common identity 18 (`identity::PAYER`).

These are the coordinates the 2026-09-06 SubmitCandidate profile actually
publishes. The System program sits at 11 -- the first coordinate past the three
readonly evidence accounts that start at 8 -- and NOT at 8, which is where an
earlier reading of this witness put it by mistaking the evidence start for the
System account. Rust derives every one of them
(`general_system_program_account_v3`), so nothing executable ever read the wrong
number; this witness is a claim about the profile and was simply a false one. -/
def generalWorkEscrowFund : Action := {
  operation := opcodeFund, seedCount := 0, state := 5, counterparty := 6,
  refundDestination := unusedCoordinate, systemProgram := 11, lamportsScalar := 93,
  refundOwnerIdentity := 18, seedStart := 0, liveBytes := 0
}

theorem general_fund_is_a_fund : fundShape generalWorkEscrowFund := by native_decide
theorem general_fund_is_valid : valid generalWorkEscrowFund := by native_decide
theorem a_fund_is_neither_create_nor_close :
    !createShape generalWorkEscrowFund ∧ !closeShape generalWorkEscrowFund := by native_decide
theorem fund_with_seeds_refuses :
    !valid { generalWorkEscrowFund with seedCount := 1 } := by native_decide
theorem fund_with_width_refuses :
    !valid { generalWorkEscrowFund with liveBytes := 1 } := by native_decide
theorem fund_with_refund_refuses :
    !valid { generalWorkEscrowFund with refundDestination := 7 } := by native_decide
theorem fund_without_system_refuses :
    !valid { generalWorkEscrowFund with systemProgram := unusedCoordinate } := by native_decide
theorem fund_with_aliased_system_refuses :
    !valid { generalWorkEscrowFund with systemProgram := 6 } := by native_decide
theorem fund_on_its_own_payer_refuses :
    !valid { generalWorkEscrowFund with state := 6 } := by native_decide
theorem opcodes_are_distinct :
    opcodeCreate ≠ opcodeClose ∧ opcodeClose ≠ opcodeFund ∧ opcodeCreate ≠ opcodeFund := by
  decide

/-! ## Canonical byte witnesses and refusal corpus -/

def zeros (count : Nat) : List UInt8 := List.replicate count 0

def encodeHeader (actionCount seedCount baseBytes : Nat) : List UInt8 :=
  magic ++ [UInt8.ofNat version] ++ zeros 1 ++
  DClutch.Codec.encodeLE 2 actionCount ++ DClutch.Codec.encodeLE 2 seedCount ++ zeros 2 ++
  DClutch.Codec.encodeLE 4 baseBytes ++ zeros 16

def encodeAction (action : Action) : List UInt8 :=
  [UInt8.ofNat action.operation, UInt8.ofNat action.seedCount] ++
  DClutch.Codec.encodeLE 2 action.state ++ DClutch.Codec.encodeLE 2 action.counterparty ++
  DClutch.Codec.encodeLE 2 action.refundDestination ++
  DClutch.Codec.encodeLE 2 action.systemProgram ++
  DClutch.Codec.encodeLE 2 action.lamportsScalar ++
  DClutch.Codec.encodeLE 2 action.refundOwnerIdentity ++
  DClutch.Codec.encodeLE 2 action.seedStart ++ DClutch.Codec.encodeLE 4 action.liveBytes ++
  zeros 4

/-- One Fund and no seeds over a base whose width the consumer patches in. -/
def oneFundHeaderWitness : List UInt8 := encodeHeader 1 0 0
def fundActionWitness : List UInt8 := encodeAction generalWorkEscrowFund
def hostileFundWithSeeds : List UInt8 :=
  encodeAction { generalWorkEscrowFund with seedCount := 1 }
def hostileFundWithWidth : List UInt8 :=
  encodeAction { generalWorkEscrowFund with liveBytes := 1 }
def hostileFundWithRefund : List UInt8 :=
  encodeAction { generalWorkEscrowFund with refundDestination := 7 }

theorem fixed_layout_widths_are_exact :
    headerBytes = 32 ∧ actionBytes = 24 ∧ seedBytes = 40 := by native_decide

theorem fixed_layouts_are_pairwise_disjoint :
    headerLayout.Pairwise Before ∧ actionLayout.Pairwise Before ∧
      seedLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 headerSchema,
    specializeFrom_pairwise 0 actionSchema,
    specializeFrom_pairwise 0 seedSchema⟩

theorem schema_release_coordinates_are_exact :
    schemaReleasePreimage.length = 61 ∧ schemaReleaseId.length = 32 := by native_decide

theorem canonical_witness_widths_are_exact :
    oneFundHeaderWitness.length = headerBytes ∧ fundActionWitness.length = actionBytes := by
  native_decide

theorem hostile_corpus_preserves_exact_action_width :
    hostileFundWithSeeds.length = actionBytes ∧ hostileFundWithWidth.length = actionBytes ∧
      hostileFundWithRefund.length = actionBytes := by native_decide

theorem fund_witness_carries_its_opcode :
    fundActionWitness.head? = some (UInt8.ofNat opcodeFund) := by native_decide

end DClutch.EffectProgramV5Abi
