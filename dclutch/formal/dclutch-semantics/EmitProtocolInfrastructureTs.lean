import DClutchSemantics.ProtocolInfrastructureProfileAbi
import DClutchSemantics.TsEmit

/-! Emit the protocol-infrastructure profile ABI as TypeScript.

This module was a mirror of a mirror: `generate-protocol-infrastructure.mjs`
ran a regular expression over the Lean-EMITTED Rust to rebuild coordinates the
emitter already knew, and it broke the moment a coordinate moved from the
hand-written half of that crate into the generated half. The layout, both
magics and both seed domains come straight from the schema here. -/

open DClutch.AbiSchema
open DClutch.ProtocolInfrastructureProfileAbi
open DClutch.TsEmit

def main : IO Unit :=
  emit (header "EmitProtocolInfrastructureTs.lean" "abi:infrastructure") [
    [ nat "PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1" profileBytes,
      nat "PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1" schemaVersion,
      nat "PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1" artifactProfile ]
      ++ offsets ProfileField.rustName profileLayout,
    [ bytes "PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1" profileMagic,
      domain "PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1" profilePdaDomainTextV1 ],
    [ nat "PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2" profileBytesV2,
      nat "PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V2" schemaVersionV2 ]
      ++ offsets ProfileFieldV2.rustName profileLayoutV2,
    [ bytes "PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V2" profileMagicV2,
      domain "PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2" profilePdaDomainTextV2 ]
  ]
