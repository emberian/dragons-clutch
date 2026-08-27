import DClutchSemantics.RealmPositionAbi
import DClutchSemantics.TsEmit

/-! Emit the Lean-owned Realm and Position byte ABI as a TypeScript module.

This is the same schema the Rust emitter prints, through the second backend, so
the browser and the contract are two views of one Lean object rather than two
readings of a byte layout. -/

open DClutch.RealmPositionAbi
open DClutch.TsEmit

def main : IO Unit :=
  emit (header "EmitRealmPositionAbiTs.lean" "abi:realm-position") [
    [ nat "SVM_MAX_SEED_BYTES" svmMaxSeedBytes ],
    [ ascii "REALM_MAGIC_V1" realmMagic,
      nat "REALM_SCHEMA_VERSION_V1" realmSchemaVersion,
      nat "REALM_BYTES_V1" realmBytes,
      nat "REALM_RESERVED_BYTES_V1" realmReservedBytes,
      domain "REALM_PDA_DOMAIN_V1" realmPdaDomain ],
    offsets RealmField.constantName realmLayout,
    [ ascii "POSITION_MAGIC_V1" positionMagic,
      nat "POSITION_SCHEMA_VERSION_V1" positionSchemaVersion,
      nat "POSITION_BASE_BYTES_V1" positionBaseBytes,
      nat "POSITION_RESERVED_BYTES_V1" positionReservedBytes,
      nat "POSITION_OUTCOME_BALANCE_BYTES_V1" outcomeBalanceBytes,
      nat "MIN_OUTCOMES_V1" minOutcomes,
      nat "MAX_OUTCOMES_V1" maxOutcomes,
      nat "BINARY_POSITION_BYTES_V1" (positionBytes minOutcomes),
      nat "MAX_POSITION_BYTES_V1" (positionBytes maxOutcomes),
      domain "POSITION_PDA_DOMAIN_V1" positionPdaDomain ],
    offsets PositionField.constantName positionLayout,
    [ "/** Exact width of a Position of a given categorical width. */",
      "export function positionBytesV1(outcomes: number): number {",
      "  return POSITION_BASE_BYTES_V1 + outcomes * POSITION_OUTCOME_BALANCE_BYTES_V1;",
      "}" ]
  ]
