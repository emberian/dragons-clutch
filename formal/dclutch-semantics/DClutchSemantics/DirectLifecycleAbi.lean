import DClutchSemantics.AbiSchema
import DClutchSemantics.DirectControllerCodec
import DClutchSemantics.DirectLifecycleProgram

/-!
# Registered Direct state ABI

This module gives the registered-intent successor one persisted byte owner.  A
claims/replay program owns the account; the authenticated controller coordinate
names the only controller allowed to advance it.  The account embeds the exact
signed compact intent rather than copying selected terms into a second mutable
truth.  `remaining` and `sequence` are the only evolving numeric authorities.

The layout is specialized from field data.  No handwritten offset table is
used by the encoder, decoder, or Rust generator.
-/

namespace DClutch.DirectLifecycleAbi

open DClutch
open DClutch.AbiSchema
open DClutch.DirectControllerCodec

def stateMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x52, 0x47, 0x49, 0x31] -- `DCLTRGI1`

def version : Nat := 1

inductive StateField where
  | magic | version | phase | reserved | controller | maker | intent
  | remaining | sequence
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.phase, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.controller, .bytes 32⟩,
  ⟨.maker, .bytes 32⟩,
  ⟨.intent, .nested compactIntentBytes⟩,
  ⟨.remaining, .u64⟩,
  ⟨.sequence, .u64⟩
]

def stateLayout : List (PlacedField StateField) := specialize stateSchema

def stateBytes : Nat := schemaWidth stateSchema

namespace StateField

def all : List StateField := [
  .magic, .version, .phase, .reserved, .controller, .maker, .intent,
  .remaining, .sequence
]

def dynamic : List StateField := [
  .phase, .controller, .maker, .intent, .remaining, .sequence
]

def coordinate (field : StateField) : Nat × Nat :=
  (coordinate? field stateLayout).getD (0, 0)

def offset (field : StateField) : Nat := (coordinate field).1

def width (field : StateField) : Nat := (coordinate field).2

def rustName : StateField → String
  | .magic => "REGISTERED_STATE_MAGIC_OFFSET"
  | .version => "REGISTERED_STATE_VERSION_OFFSET"
  | .phase => "REGISTERED_STATE_PHASE_OFFSET"
  | .reserved => "REGISTERED_STATE_RESERVED_OFFSET"
  | .controller => "REGISTERED_STATE_CONTROLLER_OFFSET"
  | .maker => "REGISTERED_STATE_MAKER_OFFSET"
  | .intent => "REGISTERED_STATE_INTENT_OFFSET"
  | .remaining => "REGISTERED_STATE_REMAINING_OFFSET"
  | .sequence => "REGISTERED_STATE_SEQUENCE_OFFSET"

def rustFieldName : StateField → String
  | .magic => "magic"
  | .version => "version"
  | .phase => "phase"
  | .reserved => "reserved"
  | .controller => "controller"
  | .maker => "maker"
  | .intent => "intent"
  | .remaining => "remaining"
  | .sequence => "sequence"

theorem all_fields_are_schema_order :
    stateSchema.map (fun field => field.name) = all := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

theorem dynamic_fields_are_unique : dynamic.Nodup := by
  native_decide

end StateField

theorem state_schema_width : stateBytes = 232 := by
  native_decide

theorem state_schema_well_formed : WellFormed stateSchema := by
  constructor
  · native_decide
  · intro field member
    simp [stateSchema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;>
      decide

theorem state_coordinates : coordinates stateLayout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.phase, 10, 1),
    (.reserved, 11, 5),
    (.controller, 16, 32),
    (.maker, 48, 32),
    (.intent, 80, 136),
    (.remaining, 216, 8),
    (.sequence, 224, 8)
  ] := by
  native_decide

namespace StateField

@[simp] theorem offset_magic : offset .magic = 0 := by native_decide
@[simp] theorem offset_version : offset .version = 8 := by native_decide
@[simp] theorem offset_phase : offset .phase = 10 := by native_decide
@[simp] theorem offset_reserved : offset .reserved = 11 := by native_decide
@[simp] theorem offset_controller : offset .controller = 16 := by native_decide
@[simp] theorem offset_maker : offset .maker = 48 := by native_decide
@[simp] theorem offset_intent : offset .intent = 80 := by native_decide
@[simp] theorem offset_remaining : offset .remaining = 216 := by native_decide
@[simp] theorem offset_sequence : offset .sequence = 224 := by native_decide

@[simp] theorem width_magic : width .magic = 8 := by native_decide
@[simp] theorem width_version : width .version = 2 := by native_decide
@[simp] theorem width_phase : width .phase = 1 := by native_decide
@[simp] theorem width_reserved : width .reserved = 5 := by native_decide
@[simp] theorem width_controller : width .controller = 32 := by native_decide
@[simp] theorem width_maker : width .maker = 32 := by native_decide
@[simp] theorem width_intent : width .intent = 136 := by native_decide
@[simp] theorem width_remaining : width .remaining = 8 := by native_decide
@[simp] theorem width_sequence : width .sequence = 8 := by native_decide

end StateField

theorem state_fields_bounded (placed : PlacedField StateField)
    (member : placed ∈ stateLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ stateBytes := by
  simpa [stateLayout, stateBytes, specialize] using
    specializeFrom_bounded 0 stateSchema placed member

theorem state_fields_disjoint : stateLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 stateSchema

/-- Physical state owned by the canonical claims/replay program.  The compact
intent is precisely the signed message accepted during registration. -/
structure RegisteredIntentStateV1 where
  phase : UInt8
  controller : Bytes32
  maker : Bytes32
  intent : CompactIntentV1
  remaining : Nat
  sequence : Nat

def encodeRegisteredIntentStateV1 (state : RegisteredIntentStateV1) : List UInt8 :=
  stateMagic ++
  Codec.encodeLE 2 version ++
  [state.phase] ++
  zeros 5 ++
  encodeBytes32 state.controller ++
  encodeBytes32 state.maker ++
  encodeCompactIntentV1 state.intent ++
  Codec.encodeLE 8 state.remaining ++
  Codec.encodeLE 8 state.sequence

theorem encode_registered_state_length (state : RegisteredIntentStateV1) :
    (encodeRegisteredIntentStateV1 state).length = stateBytes := by
  simp [encodeRegisteredIntentStateV1, stateMagic, stateBytes, stateSchema,
    compactIntentBytes,
    Codec.encodeLE_length, encodeBytes32_length, encodeCompactIntentV1_length,
    zeros]
  native_decide

def StateEncodable (state : RegisteredIntentStateV1) : Prop :=
  state.phase.toNat ≤ 3 ∧
  IntentEncodable state.intent ∧
  state.remaining < 256 ^ 8 ∧
  state.sequence < 256 ^ 8

/-- Hostile exact-width decoder.  Unknown phases and nonzero reserved bytes
refuse before the state can reach the lifecycle transition. -/
def decodeRegisteredIntentStateV1
    (bytes : List UInt8) : Option RegisteredIntentStateV1 := do
  if bytes.length != stateBytes then none else
  if bytes.take (StateField.offset .version) != stateMagic then none else
  let wireVersion := Codec.decodeLE
    ((bytes.drop (StateField.offset .version)).take (StateField.width .version))
  if wireVersion != version then none else
  let phase ← bytes[(StateField.offset .phase)]?
  if phase.toNat > 3 then none else
  if (bytes.drop (StateField.offset .reserved)).take (StateField.width .reserved) !=
      zeros (StateField.width .reserved) then none else
  let controller ← decodeBytes32
    ((bytes.drop (StateField.offset .controller)).take (StateField.width .controller))
  let maker ← decodeBytes32
    ((bytes.drop (StateField.offset .maker)).take (StateField.width .maker))
  let intent ← decodeCompactIntentV1
    ((bytes.drop (StateField.offset .intent)).take (StateField.width .intent))
  let remaining := Codec.decodeLE
    ((bytes.drop (StateField.offset .remaining)).take (StateField.width .remaining))
  let sequence := Codec.decodeLE
    ((bytes.drop (StateField.offset .sequence)).take (StateField.width .sequence))
  some { phase, controller, maker, intent, remaining, sequence }

/-- Every representable registration state survives canonical serialization
and hostile decoding exactly. -/
theorem decode_registered_state_encode
    (state : RegisteredIntentStateV1) (encodable : StateEncodable state) :
    decodeRegisteredIntentStateV1 (encodeRegisteredIntentStateV1 state) = some state := by
  rcases encodable with ⟨phaseValid, intentFits, remainingFits, sequenceFits⟩
  have intentDecoded := decodeCompactIntentV1_encode state.intent intentFits
  have versionDecoded := Codec.decodeLE_encodeLE 2 version (by native_decide)
  have versionDecodedOne : Codec.decodeLE (Codec.encodeLE 2 1) = 1 := by
    simpa [version] using versionDecoded
  have remainingDecoded := Codec.decodeLE_encodeLE 8 state.remaining remainingFits
  have sequenceDecoded := Codec.decodeLE_encodeLE 8 state.sequence sequenceFits
  have phaseAccepted : ¬state.phase.toNat > 3 := by omega
  simp [decodeRegisteredIntentStateV1, encodeRegisteredIntentStateV1,
    stateBytes, stateSchema, schemaWidth, FieldKind.byteWidth, stateMagic, version,
    compactIntentBytes, zeros, Codec.encodeLE_length, encodeBytes32_length,
    encodeCompactIntentV1_length, decodeBytes32_encodeBytes32,
    versionDecodedOne, intentDecoded, remainingDecoded, sequenceDecoded, phaseAccepted,
    List.drop_append, List.take_append, List.drop_eq_nil_of_le,
    List.take_of_length_le]

namespace Examples

def registeredState : RegisteredIntentStateV1 := {
  phase := 0
  controller := DirectControllerCodec.Examples.bytes32 7
  maker := DirectControllerCodec.Examples.bytes32 8
  intent := DirectControllerCodec.Examples.sellerIntent
  remaining := 2000
  sequence := 0
}

theorem registered_state_length :
    (encodeRegisteredIntentStateV1 registeredState).length = 232 := by
  native_decide

theorem registered_state_round_trip :
    decodeRegisteredIntentStateV1 (encodeRegisteredIntentStateV1 registeredState) =
      some registeredState := by
  apply decode_registered_state_encode
  simp [StateEncodable, registeredState,
    DirectControllerCodec.Examples.sellerIntent, IntentEncodable]

theorem hostile_registered_state_refuses :
    decodeRegisteredIntentStateV1 [] = none ∧
    decodeRegisteredIntentStateV1
      (encodeRegisteredIntentStateV1 registeredState |>.drop 1) = none ∧
    decodeRegisteredIntentStateV1
      (List.set (encodeRegisteredIntentStateV1 registeredState)
        (StateField.offset .magic) 0) = none ∧
    decodeRegisteredIntentStateV1
      (List.set (encodeRegisteredIntentStateV1 registeredState)
        (StateField.offset .version) 2) = none ∧
    decodeRegisteredIntentStateV1
      (List.set (encodeRegisteredIntentStateV1 registeredState)
        (StateField.offset .phase) 4) = none ∧
    decodeRegisteredIntentStateV1
      (List.set (encodeRegisteredIntentStateV1 registeredState)
        (StateField.offset .reserved) 1) = none := by
  set_option maxRecDepth 10000 in
    exact ⟨rfl, rfl, rfl, rfl, rfl, rfl⟩

end Examples

end DClutch.DirectLifecycleAbi
