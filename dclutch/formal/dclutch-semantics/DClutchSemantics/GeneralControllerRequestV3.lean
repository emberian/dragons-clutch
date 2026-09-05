import DClutchSemantics.AbiSchema
import DClutchSemantics.GeneralControllerRequestV2

/-!
# Width-preserving General controller request V3

The settlement-only V2 request reserved its final byte and assigned the prior
byte solely to settlement `Close`. GEN-SEVEN needs three independent canonical
PDA bump witnesses on the terminal candidate-verification row: Candidate,
verifier and verified-candidate result. V3 preserves the 64-byte packet width
and action selector offset while giving those final bytes one action-selected
meaning.
-/

namespace DClutch.General.ControllerRequestV3

open DClutch.AbiSchema

def abiVersion : Nat := 3
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x52, 0x45, 0x51, 0x30, 0x33] -- `DCGREQ03`

inductive RequestField where
  | magic | version | action | manifestOrderIndex | reservedA | expectedRevision
  | subjectId | pageIndex | executionIndex | primaryStateBump | secondaryStateBump
  | resultStateBump
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.manifestOrderIndex, .u8⟩, ⟨.reservedA, .reserved 4⟩,
  ⟨.expectedRevision, .u64⟩, ⟨.subjectId, .bytes 32⟩,
  ⟨.pageIndex, .u32⟩, ⟨.executionIndex, .u8⟩,
  ⟨.primaryStateBump, .u8⟩, ⟨.secondaryStateBump, .u8⟩,
  ⟨.resultStateBump, .u8⟩
]

def requestLayout := specialize requestSchema
def requestBytes := schemaWidth requestSchema

def fieldOffset [DecidableEq α] (αLayout : List (PlacedField α)) (name : α) : Nat :=
  (coordinate? name αLayout).map Prod.fst |>.getD 0

theorem exact_request_width : requestBytes = 64 := by native_decide

theorem request_is_well_formed : WellFormed requestSchema := by
  simp [WellFormed, requestSchema, FieldKind.byteWidth]

theorem request_coordinates_are_canonical : coordinates requestLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1),
    (.manifestOrderIndex, 11, 1), (.reservedA, 12, 4),
    (.expectedRevision, 16, 8), (.subjectId, 24, 32),
    (.pageIndex, 56, 4), (.executionIndex, 60, 1),
    (.primaryStateBump, 61, 1), (.secondaryStateBump, 62, 1),
    (.resultStateBump, 63, 1)] := by native_decide

/-- The wire break preserves every settlement coordinate: each of these fields
sits where V2 put it, stated against V2's own layout rather than against the
numbers both happen to be. -/
theorem selector_and_settlement_prefix_match_v2 :
    fieldOffset requestLayout .action =
      fieldOffset ControllerRequestV2.requestLayout .action ∧
    fieldOffset requestLayout .manifestOrderIndex =
      fieldOffset ControllerRequestV2.requestLayout .manifestOrderIndex ∧
    fieldOffset requestLayout .expectedRevision =
      fieldOffset ControllerRequestV2.requestLayout .expectedRevision ∧
    fieldOffset requestLayout .pageIndex =
      fieldOffset ControllerRequestV2.requestLayout .pageIndex ∧
    fieldOffset requestLayout .executionIndex =
      fieldOffset ControllerRequestV2.requestLayout .executionIndex ∧
    requestBytes = ControllerRequestV2.requestBytes := by native_decide

end DClutch.General.ControllerRequestV3
