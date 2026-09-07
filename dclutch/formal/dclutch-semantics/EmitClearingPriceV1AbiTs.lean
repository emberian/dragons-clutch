import DClutchSemantics.ClearingPriceV1Abi
import DClutchSemantics.TsEmit

/-! The browser and SDK half of `EmitClearingPriceV1AbiRust.lean`: the batch
record's V2 magic, version and status tags, and the clearing tail's absolute
coordinates, printed as
`packages/dclutch-sdk/lib/generated/generalClearingPriceV1.ts`.

    node scripts/lean-emit.mjs DClutchSemantics.ClearingPriceV1Abi EmitClearingPriceV1AbiTs.lean lib/generated/generalClearingPriceV1.ts [--check]

Offsets are ABSOLUTE record offsets, exactly as the Rust emitter prints them,
so a fixed-offset EffectProgram write and the SDK decoder read one number.

What this module does NOT own, and neither does Lean: the V1 batch prefix
(`outcome_count` through `cancelled_count`, bytes 12..224). That field
sequence lives only in `GeneralBatchLayoutV2` in
`crates/dclutch-trading/src/general/collection_v1.rs`, and the browser reads
it through `generate-general-successor-v5.mjs`, which scrapes that `impl`.
Promoting the prefix to a `ClearingPriceV1Abi`-style field list is the exit;
until then the batch record has one Lean-authored half and one Rust-authored
half, and `generalClearingV1.ts` says which is which at each coordinate. -/

open DClutch.General.RuntimeWireV2 (Field place)
open DClutch.General.ClearingPriceV1
open DClutch.TsEmit

/-- The tail's fields at their absolute record coordinates. -/
def placementLines (recordPrefix : String) (base : Nat) (fields : List Field) : List String :=
  (place fields).flatMap fun entry =>
    let upper := entry.1.toUpper
    [ nat s!"{recordPrefix}_{upper}_OFFSET_V1" (base + entry.2.1),
      nat s!"{recordPrefix}_{upper}_BYTES_V1" entry.2.2 ]

def main : IO Unit :=
  emit (header "EmitClearingPriceV1AbiTs.lean" "abi:general-clearing-v1") [
    [ bytes "GENERAL_BATCH_MAGIC_V2" (batchMagic.map (fun byte => byte.toUInt8)),
      nat "GENERAL_BATCH_VERSION_V2" batchVersion,
      nat "GENERAL_BATCH_STATUS_COLLECTING_V2" statusCollecting,
      nat "GENERAL_BATCH_STATUS_CLOSED_V2" statusClosed,
      nat "GENERAL_BATCH_STATUS_CLEARED_V2" statusCleared ],
    [ nat "GENERAL_BATCH_V1_BYTES" batchV1Bytes,
      nat "GENERAL_CLEARING_OFFSET_V1" clearingOffset,
      nat "GENERAL_CLEARING_FIXED_BYTES_V1" clearingFixedBytes,
      nat "GENERAL_CLEARING_PRICES_OFFSET_V1" pricesOffset,
      nat "GENERAL_CLEARING_TAIL_STRIDE_V1" tailStride,
      nat "GENERAL_CLEARING_TAIL_COUNT_V1" tailCount ],
    [ nat "GENERAL_CLEARING_MOVE_NONE_V1" moveNone,
      nat "GENERAL_CLEARING_MOVE_MINT_V1" moveMint,
      nat "GENERAL_CLEARING_MOVE_MERGE_V1" moveMerge ],
    placementLines "GENERAL_CLEARING" clearingOffset clearingFields
  ]
