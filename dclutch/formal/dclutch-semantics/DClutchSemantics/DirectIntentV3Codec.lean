import DClutchSemantics.AbiSchema
import DClutchSemantics.DirectControllerCodec
import DClutchSemantics.DirectIntentV2Codec

/-!
# The RFQ ticket: Direct signed intent V3

The batch spine (`docs/design/MECHANISM_BATCH_SPINE_2026_09_04.md` §1.5) keeps
Direct as the RFQ venue — a batch of two whose collection window is one
transaction — and gives the ticket one new field: an optional `counterparty`.
A maker who wants a specific taker names one; a bearer ticket carries zero and
stays bearer. The field is signed, so nobody but the maker can narrow or widen
who may accept the quote.

Everything else is `DirectIntentV2Codec` byte for byte: the same sixteen
fields at the same offsets, then 32 more bytes. The magic, version and
signature domain all move, so a V2 preimage cannot replay as a V3 one and a
V3 preimage cannot be read by a V2 decoder.

`limitPrice` keeps V2's meaning — the seller's FLOOR or the buyer's CAP — and
under the RFQ neither party names the execution price: `DirectRfqV1` derives
it from the two limits. `lifecycle = 2` (registered, resting on chain) has no
meaning under the RFQ and is refused by the request codec; a resting order is
General's `PlaceOrder`.
-/

namespace DClutch.DirectIntentV3Codec

open DClutch.AbiSchema
open DClutch.Codec
open DClutch.DirectControllerCodec (Bytes32 encodeBytes32)

def intentMagicV3 : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x44, 0x49, 0x57, 0x33] -- `DCLTDIW3`

def intentVersionV3 : Nat := 3

def signatureDomainPreimageV3 : String :=
  "dclutch/signature/direct-compact-intent-v3"

/-- SHA-256 of `signatureDomainPreimageV3`. -/
def signatureDomainIdV3 : List UInt8 := [
  0xeb, 0x4b, 0x19, 0xbd, 0xb8, 0x6a, 0x40, 0x02,
  0xaf, 0x83, 0xbc, 0xf3, 0x7e, 0x99, 0x04, 0x4d,
  0xff, 0x67, 0x8b, 0x64, 0xf3, 0xf5, 0x13, 0xbf,
  0x8a, 0x81, 0x75, 0x6b, 0x57, 0xaa, 0xf9, 0xa2]

inductive IntentFieldV3 where
  | magic | version | side | lifecycle | reservedA | outcome | market
  | generation | nonce | validFrom | validThrough | maximumFill | limitPrice
  | feeBasisPoints | reservedB | collateralAccount | counterparty
  deriving DecidableEq, Repr

def intentSchemaV3 : List (FieldSpec IntentFieldV3) := [
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
  ⟨.collateralAccount, .bytes 32⟩,
  ⟨.counterparty, .bytes 32⟩
]

def intentLayoutV3 : List (PlacedField IntentFieldV3) := specialize intentSchemaV3
def compactIntentBytesV3 : Nat := schemaWidth intentSchemaV3
def signedPreimageBytesV3 : Nat := signatureDomainIdV3.length + compactIntentBytesV3

namespace IntentFieldV3

def all : List IntentFieldV3 := [
  .magic, .version, .side, .lifecycle, .reservedA, .outcome, .market,
  .generation, .nonce, .validFrom, .validThrough, .maximumFill, .limitPrice,
  .feeBasisPoints, .reservedB, .collateralAccount, .counterparty]

def coordinate (field : IntentFieldV3) : Nat × Nat :=
  (coordinate? field intentLayoutV3).getD (0, 0)

def offset (field : IntentFieldV3) : Nat := (coordinate field).1
def width (field : IntentFieldV3) : Nat := (coordinate field).2

def rustName : IntentFieldV3 → String
  | .magic => "COMPACT_INTENT_MAGIC_OFFSET_V3"
  | .version => "COMPACT_INTENT_VERSION_OFFSET_V3"
  | .side => "COMPACT_INTENT_SIDE_OFFSET_V3"
  | .lifecycle => "COMPACT_INTENT_LIFECYCLE_OFFSET_V3"
  | .reservedA => "COMPACT_INTENT_RESERVED_A_OFFSET_V3"
  | .outcome => "COMPACT_INTENT_OUTCOME_OFFSET_V3"
  | .market => "COMPACT_INTENT_MARKET_OFFSET_V3"
  | .generation => "COMPACT_INTENT_GENERATION_OFFSET_V3"
  | .nonce => "COMPACT_INTENT_NONCE_OFFSET_V3"
  | .validFrom => "COMPACT_INTENT_VALID_FROM_OFFSET_V3"
  | .validThrough => "COMPACT_INTENT_VALID_THROUGH_OFFSET_V3"
  | .maximumFill => "COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V3"
  | .limitPrice => "COMPACT_INTENT_LIMIT_PRICE_OFFSET_V3"
  | .feeBasisPoints => "COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V3"
  | .reservedB => "COMPACT_INTENT_RESERVED_B_OFFSET_V3"
  | .collateralAccount => "COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V3"
  | .counterparty => "COMPACT_INTENT_COUNTERPARTY_OFFSET_V3"

end IntentFieldV3

/-- The RFQ ticket. `counterparty` all-zero is a bearer ticket. -/
structure CompactIntentV3 where
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
  counterparty : Bytes32

def zeroBytes32 : Bytes32 := fun _ => 0

/-- A ticket anyone may accept: every counterparty byte is zero. Decided over
the encoded bytes, which is also the form the request codec reads. -/
def CompactIntentV3.bearer (intent : CompactIntentV3) : Bool :=
  (encodeBytes32 intent.counterparty).all (· == 0)

def encodeCompactIntentV3 (intent : CompactIntentV3) : List UInt8 :=
  intentMagicV3 ++
  encodeLE 2 intentVersionV3 ++
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
  encodeBytes32 intent.collateralAccount ++
  encodeBytes32 intent.counterparty

def signedPreimageV3 (intent : CompactIntentV3) : List UInt8 :=
  signatureDomainIdV3 ++ encodeCompactIntentV3 intent

/-- The V2 terms a V3 ticket carries: every field but the counterparty. The
economics (`DirectSuccessor`, `DirectOrdinaryV3`) read exactly these. -/
def CompactIntentV3.terms (intent : CompactIntentV3) : DirectIntentV2Codec.CompactIntentV2 := {
  side := intent.side
  lifecycle := intent.lifecycle
  outcome := intent.outcome
  market := intent.market
  generation := intent.generation
  nonce := intent.nonce
  validFrom := intent.validFrom
  validThrough := intent.validThrough
  maximumFill := intent.maximumFill
  limitPrice := intent.limitPrice
  feeBasisPoints := intent.feeBasisPoints
  collateralAccount := intent.collateralAccount
}

theorem intent_width : compactIntentBytesV3 = 172 := by native_decide
theorem signature_domain_width : signatureDomainIdV3.length = 32 := by native_decide
theorem signed_preimage_width : signedPreimageBytesV3 = 204 := by native_decide

/-- The V3 ticket is the V2 ticket plus one 32-byte field. -/
theorem v3_extends_v2 :
    compactIntentBytesV3 = DirectIntentV2Codec.compactIntentBytesV2 + 32 := by native_decide

theorem intent_names_unique : (intentSchemaV3.map fun field => field.name).Nodup := by
  native_decide

theorem intent_fields_disjoint : intentLayoutV3.Pairwise Before :=
  specializeFrom_pairwise 0 intentSchemaV3

theorem intent_coordinates : coordinates intentLayoutV3 = [
    (.magic, 0, 8), (.version, 8, 2), (.side, 10, 1),
    (.lifecycle, 11, 1), (.reservedA, 12, 4), (.outcome, 16, 4),
    (.market, 20, 32), (.generation, 52, 8), (.nonce, 60, 8),
    (.validFrom, 68, 8), (.validThrough, 76, 8),
    (.maximumFill, 84, 8), (.limitPrice, 92, 8),
    (.feeBasisPoints, 100, 2), (.reservedB, 102, 6),
    (.collateralAccount, 108, 32), (.counterparty, 140, 32)] := by native_decide

/-- Every V2 field keeps its V2 offset: a V3 reader of the first 140 bytes
sees exactly what a V2 reader saw, which is what lets `terms` be a projection
rather than a re-encoding. -/
theorem v2_offsets_preserved :
    IntentFieldV3.offset .side = DirectIntentV2Codec.IntentFieldV2.offset .side ∧
    IntentFieldV3.offset .lifecycle = DirectIntentV2Codec.IntentFieldV2.offset .lifecycle ∧
    IntentFieldV3.offset .outcome = DirectIntentV2Codec.IntentFieldV2.offset .outcome ∧
    IntentFieldV3.offset .market = DirectIntentV2Codec.IntentFieldV2.offset .market ∧
    IntentFieldV3.offset .generation = DirectIntentV2Codec.IntentFieldV2.offset .generation ∧
    IntentFieldV3.offset .nonce = DirectIntentV2Codec.IntentFieldV2.offset .nonce ∧
    IntentFieldV3.offset .validFrom = DirectIntentV2Codec.IntentFieldV2.offset .validFrom ∧
    IntentFieldV3.offset .validThrough = DirectIntentV2Codec.IntentFieldV2.offset .validThrough ∧
    IntentFieldV3.offset .maximumFill = DirectIntentV2Codec.IntentFieldV2.offset .maximumFill ∧
    IntentFieldV3.offset .limitPrice = DirectIntentV2Codec.IntentFieldV2.offset .limitPrice ∧
    IntentFieldV3.offset .feeBasisPoints =
      DirectIntentV2Codec.IntentFieldV2.offset .feeBasisPoints ∧
    IntentFieldV3.offset .collateralAccount =
      DirectIntentV2Codec.IntentFieldV2.offset .collateralAccount := by
  native_decide

theorem encode_length (intent : CompactIntentV3) :
    (encodeCompactIntentV3 intent).length = compactIntentBytesV3 := by
  rw [intent_width]
  simp [encodeCompactIntentV3, intentMagicV3, encodeBytes32, encodeLE_length]

theorem signed_preimage_length (intent : CompactIntentV3) :
    (signedPreimageV3 intent).length = signedPreimageBytesV3 := by
  simp [signedPreimageV3, signedPreimageBytesV3, encode_length]

/-- Neither the legacy magic nor V2's can be read as a V3 ticket. -/
theorem legacy_magic_refused :
    intentMagicV3 ≠ DClutch.DirectControllerCodec.intentMagic := by native_decide

theorem v2_magic_refused :
    intentMagicV3 ≠ DirectIntentV2Codec.intentMagicV2 := by native_decide

/-- A V2 signature cannot be presented as a V3 one: the domains differ. -/
theorem v2_domain_refused :
    signatureDomainIdV3 ≠ DirectIntentV2Codec.signatureDomainIdV2 := by native_decide

def byte32 (value : UInt8) : Bytes32 := fun _ => value

namespace Examples

/-- A maker's quote naming its taker. -/
def named : CompactIntentV3 := {
  side := 0
  lifecycle := 0
  outcome := 3
  market := byte32 0x21
  generation := 9
  nonce := 12
  validFrom := 100
  validThrough := 200
  maximumFill := 5000
  limitPrice := 400000
  feeBasisPoints := 25
  collateralAccount := byte32 0x45
  counterparty := byte32 0x77
}

/-- The same quote as a bearer ticket. -/
def bearer : CompactIntentV3 := { named with counterparty := zeroBytes32 }

theorem named_is_not_bearer : named.bearer = false := by native_decide
theorem bearer_is_bearer : bearer.bearer = true := by native_decide

/-- The terms projection forgets exactly the counterparty: the two tickets
encode to different V3 bytes and to the same V2 terms. -/
theorem terms_agree :
    DirectIntentV2Codec.encodeCompactIntentV2 named.terms =
      DirectIntentV2Codec.encodeCompactIntentV2 bearer.terms := by native_decide

theorem tickets_differ : encodeCompactIntentV3 named ≠ encodeCompactIntentV3 bearer := by
  native_decide

end Examples

end DClutch.DirectIntentV3Codec
