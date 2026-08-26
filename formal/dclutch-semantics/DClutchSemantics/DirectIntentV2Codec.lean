import DClutchSemantics.AbiSchema
import DClutchSemantics.DirectControllerCodec

/-!
# Runtime-width Direct signed intent V2

This module replaces the successor Direct intent's `u8` outcome coordinate
with the Product-V2 `u32` coordinate.  The native Ed25519 message is the
32-byte digest of the named signature domain followed by the exact canonical
intent bytes.  Neither the legacy magic nor the legacy bare-message signing
shape can replay as this authority.
-/

namespace DClutch.DirectIntentV2Codec

open DClutch.AbiSchema
open DClutch.Codec
open DClutch.DirectControllerCodec (Bytes32 encodeBytes32)

def intentMagicV2 : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x44, 0x49, 0x57, 0x32] -- `DCLTDIW2`

def intentVersionV2 : Nat := 2

def signatureDomainPreimageV2 : String :=
  "dclutch/signature/direct-compact-intent-v2"

/-- SHA-256 of `signatureDomainPreimageV2`. -/
def signatureDomainIdV2 : List UInt8 := [
  0xfb, 0x92, 0xc3, 0x7d, 0x0b, 0xc7, 0x80, 0x57,
  0x30, 0x4b, 0x2e, 0x61, 0xd1, 0xf3, 0x62, 0xb8,
  0x5a, 0xbe, 0x39, 0x0e, 0x5f, 0xe1, 0xca, 0xb7,
  0x93, 0xbe, 0x2a, 0xd7, 0x31, 0xfb, 0x32, 0x82]

inductive IntentFieldV2 where
  | magic | version | side | lifecycle | reservedA | outcome | market
  | generation | nonce | validFrom | validThrough | maximumFill | limitPrice
  | feeBasisPoints | reservedB | collateralAccount
  deriving DecidableEq, Repr

def intentSchemaV2 : List (FieldSpec IntentFieldV2) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.side, .u8⟩,
  ⟨.lifecycle, .u8⟩,
  ⟨.reservedA, .reserved 4⟩,
  ⟨.outcome, .u32⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.nonce, .u64⟩,
  ⟨.validFrom, .u64⟩,
  ⟨.validThrough, .u64⟩,
  ⟨.maximumFill, .u64⟩,
  ⟨.limitPrice, .u64⟩,
  ⟨.feeBasisPoints, .u16⟩,
  ⟨.reservedB, .reserved 6⟩,
  ⟨.collateralAccount, .bytes 32⟩
]

def intentLayoutV2 : List (PlacedField IntentFieldV2) := specialize intentSchemaV2
def compactIntentBytesV2 : Nat := schemaWidth intentSchemaV2
def signedPreimageBytesV2 : Nat := signatureDomainIdV2.length + compactIntentBytesV2

namespace IntentFieldV2

def all : List IntentFieldV2 := [
  .magic, .version, .side, .lifecycle, .reservedA, .outcome, .market,
  .generation, .nonce, .validFrom, .validThrough, .maximumFill, .limitPrice,
  .feeBasisPoints, .reservedB, .collateralAccount]

def coordinate (field : IntentFieldV2) : Nat × Nat :=
  (coordinate? field intentLayoutV2).getD (0, 0)

def offset (field : IntentFieldV2) : Nat := (coordinate field).1
def width (field : IntentFieldV2) : Nat := (coordinate field).2

def rustName : IntentFieldV2 → String
  | .magic => "COMPACT_INTENT_MAGIC_OFFSET_V2"
  | .version => "COMPACT_INTENT_VERSION_OFFSET_V2"
  | .side => "COMPACT_INTENT_SIDE_OFFSET_V2"
  | .lifecycle => "COMPACT_INTENT_LIFECYCLE_OFFSET_V2"
  | .reservedA => "COMPACT_INTENT_RESERVED_A_OFFSET_V2"
  | .outcome => "COMPACT_INTENT_OUTCOME_OFFSET_V2"
  | .market => "COMPACT_INTENT_MARKET_OFFSET_V2"
  | .generation => "COMPACT_INTENT_GENERATION_OFFSET_V2"
  | .nonce => "COMPACT_INTENT_NONCE_OFFSET_V2"
  | .validFrom => "COMPACT_INTENT_VALID_FROM_OFFSET_V2"
  | .validThrough => "COMPACT_INTENT_VALID_THROUGH_OFFSET_V2"
  | .maximumFill => "COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2"
  | .limitPrice => "COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2"
  | .feeBasisPoints => "COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2"
  | .reservedB => "COMPACT_INTENT_RESERVED_B_OFFSET_V2"
  | .collateralAccount => "COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2"

end IntentFieldV2

/-- The only signed intent accepted by the successor Direct lifecycle. -/
structure CompactIntentV2 where
  side : UInt8
  lifecycle : UInt8
  outcome : Nat
  market : Bytes32
  generation : Nat
  nonce : Nat
  validFrom : Nat
  validThrough : Nat
  maximumFill : Nat
  limitPrice : Nat
  feeBasisPoints : Nat
  collateralAccount : Bytes32

def encodeCompactIntentV2 (intent : CompactIntentV2) : List UInt8 :=
  intentMagicV2 ++
  encodeLE 2 intentVersionV2 ++
  [intent.side, intent.lifecycle] ++
  List.replicate 4 0 ++
  encodeLE 4 intent.outcome ++
  encodeBytes32 intent.market ++
  encodeLE 8 intent.generation ++
  encodeLE 8 intent.nonce ++
  encodeLE 8 intent.validFrom ++
  encodeLE 8 intent.validThrough ++
  encodeLE 8 intent.maximumFill ++
  encodeLE 8 intent.limitPrice ++
  encodeLE 2 intent.feeBasisPoints ++
  List.replicate 6 0 ++
  encodeBytes32 intent.collateralAccount

def signedPreimageV2 (intent : CompactIntentV2) : List UInt8 :=
  signatureDomainIdV2 ++ encodeCompactIntentV2 intent

theorem intent_width : compactIntentBytesV2 = 140 := by native_decide
theorem signature_domain_width : signatureDomainIdV2.length = 32 := by native_decide
theorem signed_preimage_width : signedPreimageBytesV2 = 172 := by native_decide

theorem intent_names_unique : (intentSchemaV2.map fun field => field.name).Nodup := by
  native_decide

theorem intent_fields_disjoint : intentLayoutV2.Pairwise Before :=
  specializeFrom_pairwise 0 intentSchemaV2

theorem intent_coordinates : coordinates intentLayoutV2 = [
    (.magic, 0, 8), (.version, 8, 2), (.side, 10, 1),
    (.lifecycle, 11, 1), (.reservedA, 12, 4), (.outcome, 16, 4),
    (.market, 20, 32), (.generation, 52, 8), (.nonce, 60, 8),
    (.validFrom, 68, 8), (.validThrough, 76, 8),
    (.maximumFill, 84, 8), (.limitPrice, 92, 8),
    (.feeBasisPoints, 100, 2), (.reservedB, 102, 6),
    (.collateralAccount, 108, 32)] := by native_decide

theorem encode_length (intent : CompactIntentV2) :
    (encodeCompactIntentV2 intent).length = compactIntentBytesV2 := by
  rw [intent_width]
  simp [encodeCompactIntentV2, intentMagicV2, encodeBytes32, encodeLE_length]

theorem signed_preimage_length (intent : CompactIntentV2) :
    (signedPreimageV2 intent).length = signedPreimageBytesV2 := by
  simp [signedPreimageV2, signedPreimageBytesV2, encode_length]

theorem legacy_magic_refused :
    intentMagicV2 ≠ DClutch.DirectControllerCodec.intentMagic := by native_decide

def cancelThroughMagicV2 : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x43, 0x54, 0x48, 0x32] -- `DCLTCTH2`

def cancelThroughSignatureDomainPreimageV2 : String :=
  "dclutch/signature/direct-cancel-through-v2"

/-- SHA-256 of `cancelThroughSignatureDomainPreimageV2`. -/
def cancelThroughSignatureDomainIdV2 : List UInt8 := [
  0x2e, 0xdc, 0xb9, 0xae, 0x86, 0x42, 0x9f, 0xa7,
  0xfb, 0xfe, 0x3a, 0x94, 0x9b, 0xfa, 0x21, 0xfa,
  0xdf, 0x41, 0x36, 0x7a, 0x96, 0xc1, 0x5c, 0xd2,
  0x9d, 0x52, 0xc9, 0xdc, 0x3b, 0x33, 0x40, 0xf2]

inductive CancelThroughFieldV2 where
  | magic | version | reserved | market | generation | minimumLiveNonce
  deriving DecidableEq, Repr

def cancelThroughSchemaV2 : List (FieldSpec CancelThroughFieldV2) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reserved, .reserved 6⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.minimumLiveNonce, .u64⟩
]

def cancelThroughLayoutV2 : List (PlacedField CancelThroughFieldV2) :=
  specialize cancelThroughSchemaV2
def cancelThroughBytesV2 : Nat := schemaWidth cancelThroughSchemaV2
def cancelThroughSignedPreimageBytesV2 : Nat :=
  cancelThroughSignatureDomainIdV2.length + cancelThroughBytesV2

namespace CancelThroughFieldV2

def rustName : CancelThroughFieldV2 → String
  | .magic => "CANCEL_THROUGH_MAGIC_OFFSET_V2"
  | .version => "CANCEL_THROUGH_VERSION_OFFSET_V2"
  | .reserved => "CANCEL_THROUGH_RESERVED_OFFSET_V2"
  | .market => "CANCEL_THROUGH_MARKET_OFFSET_V2"
  | .generation => "CANCEL_THROUGH_GENERATION_OFFSET_V2"
  | .minimumLiveNonce => "CANCEL_THROUGH_MINIMUM_LIVE_NONCE_OFFSET_V2"

end CancelThroughFieldV2

/-- The sole maker-signed O(1) invalidation threshold admitted by Direct V2. -/
structure CancelThroughV2 where
  market : Bytes32
  generation : Nat
  minimumLiveNonce : Nat

def encodeCancelThroughV2 (message : CancelThroughV2) : List UInt8 :=
  cancelThroughMagicV2 ++
  encodeLE 2 intentVersionV2 ++
  List.replicate 6 0 ++
  encodeBytes32 message.market ++
  encodeLE 8 message.generation ++
  encodeLE 8 message.minimumLiveNonce

def signedCancelThroughPreimageV2 (message : CancelThroughV2) : List UInt8 :=
  cancelThroughSignatureDomainIdV2 ++ encodeCancelThroughV2 message

theorem cancel_through_width : cancelThroughBytesV2 = 64 := by native_decide
theorem cancel_through_signed_width : cancelThroughSignedPreimageBytesV2 = 96 := by
  native_decide
theorem cancel_through_names_unique :
    (cancelThroughSchemaV2.map fun field => field.name).Nodup := by native_decide
theorem cancel_through_fields_disjoint : cancelThroughLayoutV2.Pairwise Before :=
  specializeFrom_pairwise 0 cancelThroughSchemaV2
theorem cancel_through_coordinates : coordinates cancelThroughLayoutV2 = [
    (.magic, 0, 8), (.version, 8, 2), (.reserved, 10, 6),
    (.market, 16, 32), (.generation, 48, 8), (.minimumLiveNonce, 56, 8)] := by
  native_decide
theorem encode_cancel_through_length (message : CancelThroughV2) :
    (encodeCancelThroughV2 message).length = cancelThroughBytesV2 := by
  rw [cancel_through_width]
  simp [encodeCancelThroughV2, cancelThroughMagicV2, encodeBytes32, encodeLE_length]
theorem signed_cancel_through_length (message : CancelThroughV2) :
    (signedCancelThroughPreimageV2 message).length = cancelThroughSignedPreimageBytesV2 := by
  simp [signedCancelThroughPreimageV2, cancelThroughSignedPreimageBytesV2,
    encode_cancel_through_length]

def byte32 (value : UInt8) : Bytes32 := fun _ => value

namespace Examples

def intent : CompactIntentV2 := {
  side := 1
  lifecycle := 2
  outcome := 70000
  market := byte32 0x21
  generation := 9
  nonce := 12
  validFrom := 100
  validThrough := 200
  maximumFill := 5000
  limitPrice := 600000
  feeBasisPoints := 25
  collateralAccount := byte32 0x45
}

def cancelThrough : CancelThroughV2 := {
  market := byte32 0x21
  generation := 9
  minimumLiveNonce := 13
}

end Examples

end DClutch.DirectIntentV2Codec
