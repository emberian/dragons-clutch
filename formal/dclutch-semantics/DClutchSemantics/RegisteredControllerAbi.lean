import DClutchSemantics.AbiSchema
import DClutchSemantics.RegisteredPhysical

/-!
# Registered Direct controller ABI

Residual fills do not repeat maker signatures.  The controller instruction
selects only canonical PDA bumps, a positive fill, and an execution price; the
two claim-owned registration accounts remain the sole owners of signed terms,
maker identity, remaining quantity, and local replay sequence.
-/

namespace DClutch.Direct.RegisteredControllerAbi

open DClutch DClutch.AbiSchema

def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x52, 0x47, 0x46, 0x31] -- `DCLTRGF1`

def version : Nat := 1

inductive Field where
  | magic | version | controllerBump | sellerRegistrationBump
  | buyerRegistrationBump | sellerPositionBump | buyerPositionBump
  | reserved | fill | executionPrice
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.controllerBump, .u8⟩,
  ⟨.sellerRegistrationBump, .u8⟩,
  ⟨.buyerRegistrationBump, .u8⟩,
  ⟨.sellerPositionBump, .u8⟩,
  ⟨.buyerPositionBump, .u8⟩,
  ⟨.reserved, .reserved 1⟩,
  ⟨.fill, .u64⟩,
  ⟨.executionPrice, .u64⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def all : List Field := [
  .magic, .version, .controllerBump, .sellerRegistrationBump,
  .buyerRegistrationBump, .sellerPositionBump, .buyerPositionBump,
  .reserved, .fill, .executionPrice
]

def coordinate (field : Field) : Nat × Nat :=
  (coordinate? field layout).getD (0, 0)

def offset (field : Field) : Nat := (coordinate field).1
def width (field : Field) : Nat := (coordinate field).2

def rustName : Field → String
  | .magic => "REGISTERED_CONTROLLER_MAGIC_OFFSET"
  | .version => "REGISTERED_CONTROLLER_VERSION_OFFSET"
  | .controllerBump => "REGISTERED_CONTROLLER_BUMP_OFFSET"
  | .sellerRegistrationBump => "REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET"
  | .buyerRegistrationBump => "REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET"
  | .sellerPositionBump => "REGISTERED_SELLER_POSITION_BUMP_OFFSET"
  | .buyerPositionBump => "REGISTERED_BUYER_POSITION_BUMP_OFFSET"
  | .reserved => "REGISTERED_CONTROLLER_RESERVED_OFFSET"
  | .fill => "REGISTERED_CONTROLLER_FILL_OFFSET"
  | .executionPrice => "REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET"

theorem all_fields_are_schema_order :
    schema.map (fun field => field.name) = all := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

end Field

theorem schema_width : bytes = 32 := by native_decide

theorem schema_well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;>
      decide

theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.version, 8, 2), (.controllerBump, 10, 1),
    (.sellerRegistrationBump, 11, 1), (.buyerRegistrationBump, 12, 1),
    (.sellerPositionBump, 13, 1), (.buyerPositionBump, 14, 1),
    (.reserved, 15, 1), (.fill, 16, 8), (.executionPrice, 24, 8)
  ] := by
  native_decide

theorem fields_disjoint : layout.Pairwise Before := by
  exact specializeFrom_pairwise 0 schema

structure RegisteredFillInstructionV1 where
  controllerBump : UInt8
  sellerRegistrationBump : UInt8
  buyerRegistrationBump : UInt8
  sellerPositionBump : UInt8
  buyerPositionBump : UInt8
  fill : Nat
  executionPrice : Nat
  deriving DecidableEq, Repr

def Encodable (instruction : RegisteredFillInstructionV1) : Prop :=
  instruction.fill < 256 ^ 8 ∧ instruction.executionPrice < 256 ^ 8

def encode (instruction : RegisteredFillInstructionV1) : List UInt8 :=
  magic ++ Codec.encodeLE 2 version ++
  [instruction.controllerBump, instruction.sellerRegistrationBump,
    instruction.buyerRegistrationBump, instruction.sellerPositionBump,
    instruction.buyerPositionBump, 0] ++
  Codec.encodeLE 8 instruction.fill ++
  Codec.encodeLE 8 instruction.executionPrice

def decode (input : List UInt8) : Option RegisteredFillInstructionV1 := do
  if input.length != bytes then none else
  if input.take (Field.offset .version) != magic then none else
  if Codec.decodeLE ((input.drop (Field.offset .version)).take 2) != version then none else
  if input[(Field.offset .reserved)]? != some 0 then none else
  some {
    controllerBump := <- input[(Field.offset .controllerBump)]?
    sellerRegistrationBump := <- input[(Field.offset .sellerRegistrationBump)]?
    buyerRegistrationBump := <- input[(Field.offset .buyerRegistrationBump)]?
    sellerPositionBump := <- input[(Field.offset .sellerPositionBump)]?
    buyerPositionBump := <- input[(Field.offset .buyerPositionBump)]?
    fill := Codec.decodeLE ((input.drop (Field.offset .fill)).take 8)
    executionPrice := Codec.decodeLE ((input.drop (Field.offset .executionPrice)).take 8)
  }

def exampleInstruction : RegisteredFillInstructionV1 := {
  controllerBump := 1
  sellerRegistrationBump := 2
  buyerRegistrationBump := 3
  sellerPositionBump := 4
  buyerPositionBump := 5
  fill := 2000
  executionPrice := 500000
}

theorem encode_length (instruction : RegisteredFillInstructionV1) :
    (encode instruction).length = bytes := by
  simp [encode, magic, bytes, schema, Codec.encodeLE_length]
  native_decide

theorem example_round_trip :
    decode (encode exampleInstruction) = some exampleInstruction := by
  native_decide

theorem hostile_examples_refuse :
    decode [] = none ∧
    decode (encode exampleInstruction |>.drop 1) = none ∧
    decode (List.set (encode exampleInstruction) (Field.offset .magic) 0) = none ∧
    decode (List.set (encode exampleInstruction) (Field.offset .version) 2) = none ∧
    decode (List.set (encode exampleInstruction) (Field.offset .reserved) 1) = none := by
  native_decide

end DClutch.Direct.RegisteredControllerAbi
