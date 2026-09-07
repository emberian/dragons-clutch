import DClutchSemantics.GeneralOrderV2Abi
import DClutchSemantics.TsEmit

/-! The browser and SDK half of `EmitGeneralOrderV2AbiRust.lean`: the same
magic, version, phase, side tags, widths and field offsets, printed as
`packages/dclutch-sdk/lib/generated/generalOrderV2.ts`.

    node scripts/lean-emit.mjs DClutchSemantics.GeneralOrderV2Abi EmitGeneralOrderV2AbiTs.lean lib/generated/generalOrderV2.ts [--check]

The joint clearing shipped this record with a Rust emitter and no TypeScript
one, so `packages/dclutch-sdk/lib/generalClearingV1.ts` hand-kept every
coordinate below and the SDK's ABI-coverage baseline grew to record it. Both
backends print the same `GeneralOrderV2Abi` object now, so the record has one
author again and a field that moves in Lean moves in the browser without
anyone remembering. -/

open DClutch.General.RuntimeWireV2 (Field place recordBytes)
open DClutch.General.OrderV2
open DClutch.TsEmit

/-- Every placed field as an offset and an exact width, named the way the
Rust emitter names it with `GENERAL_` in front: the two modules are read side
by side whenever a coordinate is in doubt. -/
def placementLines (recordPrefix : String) (fields : List Field) : List String :=
  (place fields).flatMap fun entry =>
    let upper := entry.1.toUpper
    [ nat s!"{recordPrefix}_{upper}_OFFSET_V2" entry.2.1,
      nat s!"{recordPrefix}_{upper}_BYTES_V2" entry.2.2 ]

def main : IO Unit :=
  emit (header "EmitGeneralOrderV2AbiTs.lean" "abi:general-order-v2") [
    [ bytes "GENERAL_ORDER_MAGIC_V2" (orderMagic.map (fun byte => byte.toUInt8)),
      nat "GENERAL_ORDER_VERSION_V2" orderVersion,
      nat "GENERAL_ORDER_PHASE_V2" orderPhase,
      nat "GENERAL_ORDER_SIDE_BUY_V2" sideBuy,
      nat "GENERAL_ORDER_SIDE_SELL_V2" sideSell ],
    [ nat "GENERAL_ORDER_HEADER_BYTES_V2" (recordBytes orderHeaderFields),
      nat "GENERAL_ORDER_STATE_BYTES_V2" (recordBytes orderStateFields),
      nat "GENERAL_ORDER_STATE_OFFSET_V2" orderStateOffset,
      nat "GENERAL_ORDER_ROW_BASE_V2" orderRowBase,
      nat "GENERAL_ORDER_ROW_STRIDE_V2" rowStride,
      nat "GENERAL_ORDER_SHAPE_BYTES_V2" (recordBytes shapeFields),
      nat "GENERAL_ORDER_V1_HEADER_BYTES" (recordBytes orderHeaderV1Fields) ],
    placementLines "GENERAL_ORDER" orderFixedFields
  ]
