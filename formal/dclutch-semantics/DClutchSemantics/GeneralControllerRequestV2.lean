import DClutchSemantics.AbiSchema
import DClutchSemantics.GeneralControllerAbi

/-!
# Runtime-width General controller request V2

The successor keeps the exact 64-byte request while assigning byte 11 to the
canonical verifier-emitted manifest row ordinal and the final request bytes to
untrusted canonical-PDA bump witnesses. The manifest ordinal selects a row;
that row owns its distinct source page/execution coordinates. Generic Trading
recomputes every PDA from authenticated seeds, so request bytes are never
authority by themselves.
-/

namespace DClutch.General.ControllerRequestV2

open DClutch.AbiSchema

def abiVersion : Nat := 2
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x52, 0x45, 0x51, 0x30, 0x32] -- `DCGREQ02`

inductive RequestField where
  | magic | version | action | manifestOrderIndex | reservedA | expectedRevision | candidateId
  | pageIndex | executionIndex | stateBump | terminalRecordBump | reservedB
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.manifestOrderIndex, .u8⟩, ⟨.reservedA, .reserved 4⟩,
  ⟨.expectedRevision, .u64⟩,
  ⟨.candidateId, .bytes 32⟩, ⟨.pageIndex, .u32⟩,
  ⟨.executionIndex, .u8⟩, ⟨.stateBump, .u8⟩,
  ⟨.terminalRecordBump, .u8⟩, ⟨.reservedB, .reserved 1⟩
]

def requestLayout := specialize requestSchema
def requestBytes := schemaWidth requestSchema

theorem exact_request_width : requestBytes = 64 := by native_decide

theorem request_is_well_formed : WellFormed requestSchema := by
  simp [WellFormed, requestSchema, FieldKind.byteWidth]

theorem request_coordinates_are_canonical : coordinates requestLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1),
    (.manifestOrderIndex, 11, 1), (.reservedA, 12, 4),
    (.expectedRevision, 16, 8),
    (.candidateId, 24, 32), (.pageIndex, 56, 4),
    (.executionIndex, 60, 1), (.stateBump, 61, 1),
    (.terminalRecordBump, 62, 1), (.reservedB, 63, 1)] := by native_decide

end DClutch.General.ControllerRequestV2
