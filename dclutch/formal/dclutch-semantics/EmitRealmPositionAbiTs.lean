import DClutchSemantics.RealmPositionAbi
import DClutchSemantics.TsEmit

/-! Emit the Lean-owned Realm byte ABI, and the Position seed domain, as a
TypeScript module.

This is the same schema the Rust emitter prints, through the second backend, so
the browser and the contract are two views of one Lean object rather than two
readings of a byte layout.  The Position RECORD is not emitted on either: it
was banished with the DCLTCAT1 stratum and the emission outlived it, which is
what left the browser's explorer holding a decoder arm for a record nothing
writes. -/

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
    [ domain "POSITION_PDA_DOMAIN_V1" positionPdaDomain ]
  ]
