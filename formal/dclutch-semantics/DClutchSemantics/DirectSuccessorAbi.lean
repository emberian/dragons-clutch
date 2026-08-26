import DClutchSemantics.AbiSchema
import DClutchSemantics.DirectControllerCodec
import DClutchSemantics.DirectSuccessor

/-!
# Direct successor fixed ABIs

The immutable execution config, global Direct root tail, and per-maker replay
root are three disjoint authorities.  Config bytes are selected by the
CapabilityProgram descriptor's `config` content ID.  The common capability
root header owns Market/generation/release selection; the 24-byte Direct tail
therefore persists only global lifecycle and maker-root count.
-/

namespace DClutch.DirectSuccessorAbi

open DClutch.AbiSchema
open DClutch.DirectControllerCodec

def configMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x44, 0x45, 0x43, 0x31]
def rootMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x44, 0x52, 0x54, 0x31]
def makerMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x44, 0x4d, 0x52, 0x31]
def recordMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x44, 0x52, 0x49, 0x31]
def version : Nat := 1

inductive ConfigField where
  | magic | version | reservedA | priceScale | feeBasisPoints | reservedB | feeRecipient
  deriving DecidableEq, Repr

def configSchema : List (FieldSpec ConfigField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reservedA, .reserved 6⟩,
  ⟨.priceScale, .u64⟩,
  ⟨.feeBasisPoints, .u16⟩,
  ⟨.reservedB, .reserved 6⟩,
  ⟨.feeRecipient, .bytes 32⟩
]

def configLayout : List (PlacedField ConfigField) := specialize configSchema
def configBytes : Nat := schemaWidth configSchema

namespace ConfigField

def all : List ConfigField :=
  [.magic, .version, .reservedA, .priceScale, .feeBasisPoints, .reservedB, .feeRecipient]

def coordinate (field : ConfigField) : Nat × Nat :=
  (coordinate? field configLayout).getD (0, 0)

def offset (field : ConfigField) : Nat := (coordinate field).1
def width (field : ConfigField) : Nat := (coordinate field).2

def rustName : ConfigField → String
  | .magic => "DIRECT_CONFIG_MAGIC_OFFSET_V1"
  | .version => "DIRECT_CONFIG_VERSION_OFFSET_V1"
  | .reservedA => "DIRECT_CONFIG_RESERVED_A_OFFSET_V1"
  | .priceScale => "DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1"
  | .feeBasisPoints => "DIRECT_CONFIG_FEE_BPS_OFFSET_V1"
  | .reservedB => "DIRECT_CONFIG_RESERVED_B_OFFSET_V1"
  | .feeRecipient => "DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1"

end ConfigField

inductive RootField where
  | magic | version | phase | reserved | openMakerRootCount
  deriving DecidableEq, Repr

def rootSchema : List (FieldSpec RootField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.phase, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.openMakerRootCount, .u64⟩
]

def rootLayout : List (PlacedField RootField) := specialize rootSchema
def rootBytes : Nat := schemaWidth rootSchema

namespace RootField

def all : List RootField := [.magic, .version, .phase, .reserved, .openMakerRootCount]

def coordinate (field : RootField) : Nat × Nat :=
  (coordinate? field rootLayout).getD (0, 0)

def offset (field : RootField) : Nat := (coordinate field).1
def width (field : RootField) : Nat := (coordinate field).2

def rustName : RootField → String
  | .magic => "DIRECT_ROOT_MAGIC_OFFSET_V1"
  | .version => "DIRECT_ROOT_VERSION_OFFSET_V1"
  | .phase => "DIRECT_ROOT_PHASE_OFFSET_V1"
  | .reserved => "DIRECT_ROOT_RESERVED_OFFSET_V1"
  | .openMakerRootCount => "DIRECT_ROOT_OPEN_MAKER_COUNT_OFFSET_V1"

end RootField

inductive MakerField where
  | magic | version | bump | reserved | market | generation | maker
  | nextNonce | liveCount | minimumLiveNonce | rentOwner | rentPrincipal
  deriving DecidableEq, Repr

def makerSchema : List (FieldSpec MakerField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.bump, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.maker, .bytes 32⟩,
  ⟨.nextNonce, .u64⟩,
  ⟨.liveCount, .u64⟩,
  ⟨.minimumLiveNonce, .u64⟩,
  ⟨.rentOwner, .bytes 32⟩,
  ⟨.rentPrincipal, .u64⟩
]

def makerLayout : List (PlacedField MakerField) := specialize makerSchema
def makerBytes : Nat := schemaWidth makerSchema

namespace MakerField

def all : List MakerField := [
  .magic, .version, .bump, .reserved, .market, .generation, .maker,
  .nextNonce, .liveCount, .minimumLiveNonce, .rentOwner, .rentPrincipal
]

def coordinate (field : MakerField) : Nat × Nat :=
  (coordinate? field makerLayout).getD (0, 0)

def offset (field : MakerField) : Nat := (coordinate field).1
def width (field : MakerField) : Nat := (coordinate field).2

def rustName : MakerField → String
  | .magic => "DIRECT_MAKER_MAGIC_OFFSET_V1"
  | .version => "DIRECT_MAKER_VERSION_OFFSET_V1"
  | .bump => "DIRECT_MAKER_BUMP_OFFSET_V1"
  | .reserved => "DIRECT_MAKER_RESERVED_OFFSET_V1"
  | .market => "DIRECT_MAKER_MARKET_OFFSET_V1"
  | .generation => "DIRECT_MAKER_GENERATION_OFFSET_V1"
  | .maker => "DIRECT_MAKER_IDENTITY_OFFSET_V1"
  | .nextNonce => "DIRECT_MAKER_NEXT_NONCE_OFFSET_V1"
  | .liveCount => "DIRECT_MAKER_LIVE_COUNT_OFFSET_V1"
  | .minimumLiveNonce => "DIRECT_MAKER_MINIMUM_LIVE_NONCE_OFFSET_V1"
  | .rentOwner => "DIRECT_MAKER_RENT_OWNER_OFFSET_V1"
  | .rentPrincipal => "DIRECT_MAKER_RENT_PRINCIPAL_OFFSET_V1"

end MakerField

inductive RecordField where
  | magic | version | bump | reserved | maker | intent | filled
  | reservedClaims | reservedCollateral | cumulativeGross | cumulativeFee
  | rentOwner | rentPrincipal
  deriving DecidableEq, Repr

def recordSchema : List (FieldSpec RecordField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.bump, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.maker, .bytes 32⟩,
  ⟨.intent, .nested compactIntentBytes⟩, ⟨.filled, .u64⟩,
  ⟨.reservedClaims, .u64⟩, ⟨.reservedCollateral, .u64⟩,
  ⟨.cumulativeGross, .u64⟩, ⟨.cumulativeFee, .u64⟩,
  ⟨.rentOwner, .bytes 32⟩, ⟨.rentPrincipal, .u64⟩
]

def recordLayout : List (PlacedField RecordField) := specialize recordSchema
def recordBytes : Nat := schemaWidth recordSchema

namespace RecordField

def all : List RecordField := [
  .magic, .version, .bump, .reserved, .maker, .intent, .filled,
  .reservedClaims, .reservedCollateral, .cumulativeGross, .cumulativeFee,
  .rentOwner, .rentPrincipal
]

def coordinate (field : RecordField) : Nat × Nat :=
  (coordinate? field recordLayout).getD (0, 0)

def offset (field : RecordField) : Nat := (coordinate field).1
def width (field : RecordField) : Nat := (coordinate field).2

def rustName : RecordField → String
  | .magic => "DIRECT_RECORD_MAGIC_OFFSET_V1"
  | .version => "DIRECT_RECORD_VERSION_OFFSET_V1"
  | .bump => "DIRECT_RECORD_BUMP_OFFSET_V1"
  | .reserved => "DIRECT_RECORD_RESERVED_OFFSET_V1"
  | .maker => "DIRECT_RECORD_MAKER_OFFSET_V1"
  | .intent => "DIRECT_RECORD_INTENT_OFFSET_V1"
  | .filled => "DIRECT_RECORD_FILLED_OFFSET_V1"
  | .reservedClaims => "DIRECT_RECORD_RESERVED_CLAIMS_OFFSET_V1"
  | .reservedCollateral => "DIRECT_RECORD_RESERVED_COLLATERAL_OFFSET_V1"
  | .cumulativeGross => "DIRECT_RECORD_CUMULATIVE_GROSS_OFFSET_V1"
  | .cumulativeFee => "DIRECT_RECORD_CUMULATIVE_FEE_OFFSET_V1"
  | .rentOwner => "DIRECT_RECORD_RENT_OWNER_OFFSET_V1"
  | .rentPrincipal => "DIRECT_RECORD_RENT_PRINCIPAL_OFFSET_V1"

end RecordField

theorem config_width : configBytes = 64 := by native_decide
theorem root_width : rootBytes = 24 := by native_decide
theorem maker_width : makerBytes = 152 := by native_decide
theorem record_width : recordBytes = 264 := by native_decide

theorem config_names_unique : (configSchema.map fun field => field.name).Nodup := by
  native_decide
theorem root_names_unique : (rootSchema.map fun field => field.name).Nodup := by
  native_decide
theorem maker_names_unique : (makerSchema.map fun field => field.name).Nodup := by
  native_decide
theorem record_names_unique : (recordSchema.map fun field => field.name).Nodup := by
  native_decide

theorem config_fields_disjoint : configLayout.Pairwise Before :=
  specializeFrom_pairwise 0 configSchema
theorem root_fields_disjoint : rootLayout.Pairwise Before :=
  specializeFrom_pairwise 0 rootSchema
theorem maker_fields_disjoint : makerLayout.Pairwise Before :=
  specializeFrom_pairwise 0 makerSchema
theorem record_fields_disjoint : recordLayout.Pairwise Before :=
  specializeFrom_pairwise 0 recordSchema

theorem config_coordinates : coordinates configLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.reservedA, 10, 6),
    (.priceScale, 16, 8), (.feeBasisPoints, 24, 2),
    (.reservedB, 26, 6), (.feeRecipient, 32, 32)] := by native_decide

theorem root_coordinates : coordinates rootLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.phase, 10, 1),
    (.reserved, 11, 5), (.openMakerRootCount, 16, 8)] := by native_decide

theorem maker_coordinates : coordinates makerLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.bump, 10, 1), (.reserved, 11, 5),
    (.market, 16, 32), (.generation, 48, 8), (.maker, 56, 32),
    (.nextNonce, 88, 8), (.liveCount, 96, 8),
    (.minimumLiveNonce, 104, 8), (.rentOwner, 112, 32),
    (.rentPrincipal, 144, 8)] := by native_decide

theorem record_coordinates : coordinates recordLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.bump, 10, 1), (.reserved, 11, 5),
    (.maker, 16, 32), (.intent, 48, 136), (.filled, 184, 8),
    (.reservedClaims, 192, 8), (.reservedCollateral, 200, 8),
    (.cumulativeGross, 208, 8), (.cumulativeFee, 216, 8),
    (.rentOwner, 224, 32), (.rentPrincipal, 256, 8)] := by native_decide

def zeroBytes32 : Bytes32 := fun _ => 0
def bytesNonzero (bytes : Bytes32) : Bool := (encodeBytes32 bytes).any (fun byte => byte != 0)

structure ExecutionConfigV1 where
  priceScale : Nat
  feeBasisPoints : Nat
  feeRecipient : Bytes32

def encodeExecutionConfigV1 (config : ExecutionConfigV1) : List UInt8 :=
  configMagic ++
  Codec.encodeLE 2 version ++
  zeros 6 ++
  Codec.encodeLE 8 config.priceScale ++
  Codec.encodeLE 2 config.feeBasisPoints ++
  zeros 6 ++
  encodeBytes32 config.feeRecipient

theorem encode_config_length (config : ExecutionConfigV1) :
    (encodeExecutionConfigV1 config).length = configBytes := by
  simp [encodeExecutionConfigV1, configMagic, configBytes, configSchema,
    Codec.encodeLE_length, encodeBytes32_length, zeros]
  native_decide

structure RootStateV1 where
  phase : UInt8
  openMakerRootCount : Nat
  deriving DecidableEq

def encodeRootStateV1 (root : RootStateV1) : List UInt8 :=
  rootMagic ++
  Codec.encodeLE 2 version ++
  [root.phase] ++
  zeros 5 ++
  Codec.encodeLE 8 root.openMakerRootCount

theorem encode_root_length (root : RootStateV1) :
    (encodeRootStateV1 root).length = rootBytes := by
  simp [encodeRootStateV1, rootMagic, rootBytes, rootSchema,
    Codec.encodeLE_length, zeros]
  native_decide

structure MakerRootStateV1 where
  bump : UInt8
  market : Bytes32
  generation : Nat
  maker : Bytes32
  nextNonce : Nat
  liveCount : Nat
  minimumLiveNonce : Nat
  rentOwner : Bytes32
  rentPrincipal : Nat

def encodeMakerRootStateV1 (root : MakerRootStateV1) : List UInt8 :=
  makerMagic ++
  Codec.encodeLE 2 version ++
  [root.bump] ++
  zeros 5 ++
  encodeBytes32 root.market ++
  Codec.encodeLE 8 root.generation ++
  encodeBytes32 root.maker ++
  Codec.encodeLE 8 root.nextNonce ++
  Codec.encodeLE 8 root.liveCount ++
  Codec.encodeLE 8 root.minimumLiveNonce ++
  encodeBytes32 root.rentOwner ++
  Codec.encodeLE 8 root.rentPrincipal

theorem encode_maker_length (root : MakerRootStateV1) :
    (encodeMakerRootStateV1 root).length = makerBytes := by
  simp [encodeMakerRootStateV1, makerMagic, makerBytes, makerSchema,
    Codec.encodeLE_length, encodeBytes32_length, zeros]
  native_decide

structure RegisteredRecordStateV1 where
  bump : UInt8
  maker : Bytes32
  intent : CompactIntentV1
  filled : Nat
  reservedClaims : Nat
  reservedCollateral : Nat
  cumulativeGross : Nat
  cumulativeFee : Nat
  rentOwner : Bytes32
  rentPrincipal : Nat

def encodeRegisteredRecordStateV1 (record : RegisteredRecordStateV1) : List UInt8 :=
  recordMagic ++
  Codec.encodeLE 2 version ++
  [record.bump] ++
  zeros 5 ++
  encodeBytes32 record.maker ++
  encodeCompactIntentV1 record.intent ++
  Codec.encodeLE 8 record.filled ++
  Codec.encodeLE 8 record.reservedClaims ++
  Codec.encodeLE 8 record.reservedCollateral ++
  Codec.encodeLE 8 record.cumulativeGross ++
  Codec.encodeLE 8 record.cumulativeFee ++
  encodeBytes32 record.rentOwner ++
  Codec.encodeLE 8 record.rentPrincipal

theorem encode_record_length (record : RegisteredRecordStateV1) :
    (encodeRegisteredRecordStateV1 record).length = recordBytes := by
  simp [encodeRegisteredRecordStateV1, recordMagic, recordBytes, recordSchema,
    Codec.encodeLE_length, encodeBytes32_length, encodeCompactIntentV1_length, zeros]
  native_decide

namespace Examples

def bytes32 (value : UInt8) : Bytes32 := fun _ => value

def config : ExecutionConfigV1 := {
  priceScale := 1_000_000
  feeBasisPoints := 25
  feeRecipient := bytes32 9
}

def root : RootStateV1 := { phase := 0, openMakerRootCount := 3 }

def maker : MakerRootStateV1 := {
  bump := 7
  market := bytes32 1
  generation := 4
  maker := bytes32 2
  nextNonce := 9
  liveCount := 2
  minimumLiveNonce := 5
  rentOwner := bytes32 3
  rentPrincipal := 2_000_000
}

def record : RegisteredRecordStateV1 := {
  bump := 6
  maker := bytes32 2
  intent := { DirectControllerCodec.Examples.sellerIntent with lifecycle := 2 }
  filled := 3
  reservedClaims := 1997
  reservedCollateral := 0
  cumulativeGross := 1
  cumulativeFee := 0
  rentOwner := bytes32 3
  rentPrincipal := 2_500_000
}

theorem exact_example_widths :
    (encodeExecutionConfigV1 config).length = 64 ∧
    (encodeRootStateV1 root).length = 24 ∧
    (encodeMakerRootStateV1 maker).length = 152 ∧
    (encodeRegisteredRecordStateV1 record).length = 264 := by native_decide

theorem zero_recipient_is_not_valid : bytesNonzero zeroBytes32 = false := by native_decide

end Examples

end DClutch.DirectSuccessorAbi
