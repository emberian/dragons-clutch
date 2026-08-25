import DClutchSemantics.Codec

/-!
# Direct controller ABI

Lean owns the exact byte layout presented to the native Ed25519 precompile and
the successor Direct controller.  A maker identity is deliberately absent from
`CompactIntentV1`: the verified Ed25519 public key is its sole semantic owner.
The signed Market key commits the Product, Realm, capability manifest, and
generation through the canonical Market identity. Each maker also signs the
exact accepted fee rate. Runtime-owned Market/Realm/manifest/policy records,
not a parallel execution-profile DTO, supply the remaining execution facts.
-/

namespace DClutch.DirectControllerCodec

open DClutch.Codec

/-- A fixed 32-byte coordinate without an allocation-dependent runtime shape. -/
abbrev Bytes32 := Fin 32 → UInt8

def encodeBytes32 (bytes : Bytes32) : List UInt8 := List.ofFn bytes

theorem encodeBytes32_length (bytes : Bytes32) :
    (encodeBytes32 bytes).length = 32 := by
  simp [encodeBytes32]

/-- Hostile fixed-width decoder. The dependent length witness ensures every
lookup is total without padding or truncation. -/
def decodeBytes32 (bytes : List UInt8) : Option Bytes32 :=
  if exact : bytes.length = 32 then
    some fun index => bytes.get ⟨index.val, by omega⟩
  else none

theorem decodeBytes32_encodeBytes32 (bytes : Bytes32) :
    decodeBytes32 (encodeBytes32 bytes) = some bytes := by
  unfold decodeBytes32
  rw [dif_pos (encodeBytes32_length bytes)]
  congr
  funext index
  change (List.ofFn bytes)[index.val] = bytes index
  rw [List.getElem_ofFn]

def zeros (count : Nat) : List UInt8 := List.replicate count 0

def intentMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x44, 0x49, 0x52, 0x33] -- `DCLTDIR3`

def controllerMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x43, 0x54, 0x4c, 0x31] -- `DCLTCTL1`

def version : Nat := 1
def compactIntentBytes : Nat := 136
def controllerInstructionBytes : Nat := 304
def magicOffset : Nat := 0
def versionOffset : Nat := 8

/-- Dynamic fields in one signed intent. Fixed magic, version, and reserved
spans are emitted separately from the same module. -/
inductive IntentField where
  | side | outcome | lifecycle | market | generation | nonce | validFrom
  | validThrough | maximumFill | limitPrice | feeBasisPoints
  | collateralAccount
  deriving DecidableEq, Repr

namespace IntentField

def all : List IntentField := [
  .side, .outcome, .lifecycle, .market, .generation, .nonce, .validFrom,
  .validThrough, .maximumFill, .limitPrice, .feeBasisPoints,
  .collateralAccount
]

def offset : IntentField → Nat
  | .side => 10
  | .outcome => 11
  | .lifecycle => 12
  | .market => 16
  | .generation => 48
  | .nonce => 56
  | .validFrom => 64
  | .validThrough => 72
  | .maximumFill => 80
  | .limitPrice => 88
  | .feeBasisPoints => 96
  | .collateralAccount => 104

def width : IntentField → Nat
  | .side | .outcome | .lifecycle => 1
  | .feeBasisPoints => 2
  | .market | .collateralAccount => 32
  | _ => 8

def rustName : IntentField → String
  | .side => "INTENT_SIDE_OFFSET"
  | .outcome => "INTENT_OUTCOME_OFFSET"
  | .lifecycle => "INTENT_LIFECYCLE_OFFSET"
  | .market => "INTENT_MARKET_OFFSET"
  | .generation => "INTENT_GENERATION_OFFSET"
  | .nonce => "INTENT_NONCE_OFFSET"
  | .validFrom => "INTENT_VALID_FROM_OFFSET"
  | .validThrough => "INTENT_VALID_THROUGH_OFFSET"
  | .maximumFill => "INTENT_MAXIMUM_FILL_OFFSET"
  | .limitPrice => "INTENT_LIMIT_PRICE_OFFSET"
  | .feeBasisPoints => "INTENT_FEE_BASIS_POINTS_OFFSET"
  | .collateralAccount => "INTENT_COLLATERAL_ACCOUNT_OFFSET"

theorem spans_are_bounded :
    ∀ field ∈ all, offset field + width field ≤ compactIntentBytes := by
  native_decide

theorem spans_are_disjoint :
    (all.flatMap fun field => List.range' (offset field) (width field)).Nodup := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

end IntentField

def intentReservedAOffset : Nat := 13
def intentReservedAWidth : Nat := 3
def intentReservedBOffset : Nat := 98
def intentReservedBWidth : Nat := 6

/-- Dynamic fields in the matcher-owned controller envelope. -/
inductive ControllerField where
  | controllerBump | sellerReplayBump | buyerReplayBump
  | sellerPositionBump | buyerPositionBump | fill | executionPrice
  | seller | buyer
  deriving DecidableEq, Repr

namespace ControllerField

def all : List ControllerField := [
  .controllerBump, .sellerReplayBump, .buyerReplayBump,
  .sellerPositionBump, .buyerPositionBump, .fill, .executionPrice,
  .seller, .buyer
]

def offset : ControllerField → Nat
  | .controllerBump => 10
  | .sellerReplayBump => 11
  | .buyerReplayBump => 12
  | .sellerPositionBump => 13
  | .buyerPositionBump => 14
  | .fill => 16
  | .executionPrice => 24
  | .seller => 32
  | .buyer => 168

def width : ControllerField → Nat
  | .controllerBump | .sellerReplayBump | .buyerReplayBump
  | .sellerPositionBump | .buyerPositionBump => 1
  | .fill | .executionPrice => 8
  | .seller | .buyer => compactIntentBytes

def rustName : ControllerField → String
  | .controllerBump => "CONTROLLER_BUMP_OFFSET"
  | .sellerReplayBump => "CONTROLLER_SELLER_REPLAY_BUMP_OFFSET"
  | .buyerReplayBump => "CONTROLLER_BUYER_REPLAY_BUMP_OFFSET"
  | .sellerPositionBump => "CONTROLLER_SELLER_POSITION_BUMP_OFFSET"
  | .buyerPositionBump => "CONTROLLER_BUYER_POSITION_BUMP_OFFSET"
  | .fill => "CONTROLLER_FILL_OFFSET"
  | .executionPrice => "CONTROLLER_EXECUTION_PRICE_OFFSET"
  | .seller => "CONTROLLER_SELLER_OFFSET"
  | .buyer => "CONTROLLER_BUYER_OFFSET"

theorem spans_are_bounded :
    ∀ field ∈ all,
      offset field + width field ≤ controllerInstructionBytes := by
  native_decide

theorem spans_are_disjoint :
    (all.flatMap fun field => List.range' (offset field) (width field)).Nodup := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

end ControllerField

def controllerReservedOffset : Nat := 15
def controllerReservedWidth : Nat := 1

/-- One independently signed reusable limit intent.

`side`, `outcome`, and `lifecycle` are semantic tags. All numeric fields are
encoded little-endian. Physical admission separately checks that Nat values fit
their named widths before this encoder's output is released.
-/
structure CompactIntentV1 where
  side : UInt8
  outcome : UInt8
  lifecycle : UInt8
  market : Bytes32
  generation : Nat
  nonce : Nat
  validFrom : Nat
  validThrough : Nat
  maximumFill : Nat
  limitPrice : Nat
  feeBasisPoints : Nat
  collateralAccount : Bytes32

def encodeCompactIntentV1 (intent : CompactIntentV1) : List UInt8 :=
  intentMagic ++
  encodeLE 2 version ++
  [intent.side, intent.outcome, intent.lifecycle] ++
  zeros 3 ++
  encodeBytes32 intent.market ++
  encodeLE 8 intent.generation ++
  encodeLE 8 intent.nonce ++
  encodeLE 8 intent.validFrom ++
  encodeLE 8 intent.validThrough ++
  encodeLE 8 intent.maximumFill ++
  encodeLE 8 intent.limitPrice ++
  encodeLE 2 intent.feeBasisPoints ++
  zeros 6 ++
  encodeBytes32 intent.collateralAccount

theorem encodeCompactIntentV1_length (intent : CompactIntentV1) :
    (encodeCompactIntentV1 intent).length = compactIntentBytes := by
  simp [encodeCompactIntentV1, intentMagic, compactIntentBytes,
    encodeLE_length, encodeBytes32_length, zeros]

/-- Exact representability conditions for every integer in the signed intent.
The byte-valued semantic tags and fixed coordinates are representable by
construction. -/
def IntentEncodable (intent : CompactIntentV1) : Prop :=
  intent.generation < 256 ^ 8 ∧
  intent.nonce < 256 ^ 8 ∧
  intent.validFrom < 256 ^ 8 ∧
  intent.validThrough < 256 ^ 8 ∧
  intent.maximumFill < 256 ^ 8 ∧
  intent.limitPrice < 256 ^ 8 ∧
  intent.feeBasisPoints < 256 ^ 2

/-- Hostile decoder for one independently signed intent. It accepts exactly
one canonical 136-byte representation and refuses padding, truncation,
alternate magic/version values, and nonzero reserved bytes. -/
def decodeCompactIntentV1 (bytes : List UInt8) : Option CompactIntentV1 := do
  if bytes.length != compactIntentBytes then none else
  if bytes.take versionOffset != intentMagic then none else
  let wireVersion := decodeLE ((bytes.drop versionOffset).take 2)
  if wireVersion != version then none else
  let side ← bytes[(IntentField.offset .side)]?
  let outcome ← bytes[(IntentField.offset .outcome)]?
  let lifecycle ← bytes[(IntentField.offset .lifecycle)]?
  if (bytes.drop intentReservedAOffset).take intentReservedAWidth !=
      zeros intentReservedAWidth then none else
  let market ← decodeBytes32
    ((bytes.drop (IntentField.offset .market)).take (IntentField.width .market))
  let generation := decodeLE
    ((bytes.drop (IntentField.offset .generation)).take (IntentField.width .generation))
  let nonce := decodeLE
    ((bytes.drop (IntentField.offset .nonce)).take (IntentField.width .nonce))
  let validFrom := decodeLE
    ((bytes.drop (IntentField.offset .validFrom)).take (IntentField.width .validFrom))
  let validThrough := decodeLE
    ((bytes.drop (IntentField.offset .validThrough)).take (IntentField.width .validThrough))
  let maximumFill := decodeLE
    ((bytes.drop (IntentField.offset .maximumFill)).take (IntentField.width .maximumFill))
  let limitPrice := decodeLE
    ((bytes.drop (IntentField.offset .limitPrice)).take (IntentField.width .limitPrice))
  let feeBasisPoints := decodeLE
    ((bytes.drop (IntentField.offset .feeBasisPoints)).take
      (IntentField.width .feeBasisPoints))
  if (bytes.drop intentReservedBOffset).take intentReservedBWidth !=
      zeros intentReservedBWidth then none else
  let collateralAccount ← decodeBytes32
    ((bytes.drop (IntentField.offset .collateralAccount)).take
      (IntentField.width .collateralAccount))
  some {
    side, outcome, lifecycle, market, generation, nonce, validFrom,
    validThrough, maximumFill, limitPrice, feeBasisPoints, collateralAccount
  }

/-- Every representable semantic intent survives canonical serialization and
hostile decoding exactly. -/
theorem decodeCompactIntentV1_encode
    (intent : CompactIntentV1) (encodable : IntentEncodable intent) :
    decodeCompactIntentV1 (encodeCompactIntentV1 intent) = some intent := by
  rcases encodable with
    ⟨generationFits, nonceFits, validFromFits, validThroughFits,
      maximumFillFits, limitPriceFits, feeBasisPointsFits⟩
  have generationDecoded := decodeLE_encodeLE 8 intent.generation generationFits
  have nonceDecoded := decodeLE_encodeLE 8 intent.nonce nonceFits
  have validFromDecoded := decodeLE_encodeLE 8 intent.validFrom validFromFits
  have validThroughDecoded := decodeLE_encodeLE 8 intent.validThrough validThroughFits
  have maximumFillDecoded := decodeLE_encodeLE 8 intent.maximumFill maximumFillFits
  have limitPriceDecoded := decodeLE_encodeLE 8 intent.limitPrice limitPriceFits
  have feeBasisPointsDecoded :=
    decodeLE_encodeLE 2 intent.feeBasisPoints feeBasisPointsFits
  simp [decodeCompactIntentV1, encodeCompactIntentV1,
    intentMagic, versionOffset, version,
    compactIntentBytes, IntentField.offset, IntentField.width,
    intentReservedAOffset, intentReservedAWidth,
    intentReservedBOffset, intentReservedBWidth,
    List.drop_append, List.take_append, List.drop_eq_nil_of_le,
    List.take_of_length_le,
    encodeLE_length, encodeBytes32_length, zeros,
    decodeBytes32_encodeBytes32, generationDecoded, nonceDecoded,
    validFromDecoded, validThroughDecoded, maximumFillDecoded,
    limitPriceDecoded, feeBasisPointsDecoded]
  native_decide

/-- Matcher-selected coordinates surrounding two independently signed intents. -/
structure ControllerInstructionV1 where
  controllerBump : UInt8
  sellerReplayBump : UInt8
  buyerReplayBump : UInt8
  sellerPositionBump : UInt8
  buyerPositionBump : UInt8
  fill : Nat
  executionPrice : Nat
  seller : CompactIntentV1
  buyer : CompactIntentV1

def encodeControllerInstructionV1 (instruction : ControllerInstructionV1) : List UInt8 :=
  controllerMagic ++
  encodeLE 2 version ++
  [instruction.controllerBump, instruction.sellerReplayBump,
    instruction.buyerReplayBump, instruction.sellerPositionBump,
    instruction.buyerPositionBump] ++
  zeros 1 ++
  encodeLE 8 instruction.fill ++
  encodeLE 8 instruction.executionPrice ++
  encodeCompactIntentV1 instruction.seller ++
  encodeCompactIntentV1 instruction.buyer

theorem encodeControllerInstructionV1_length (instruction : ControllerInstructionV1) :
    (encodeControllerInstructionV1 instruction).length = controllerInstructionBytes := by
  simp [encodeControllerInstructionV1, controllerMagic,
    controllerInstructionBytes, compactIntentBytes, encodeLE_length,
    encodeCompactIntentV1_length, zeros]

/-- Exact representability conditions for the matcher envelope and its two
independently signed child intents. -/
def ControllerEncodable (instruction : ControllerInstructionV1) : Prop :=
  instruction.fill < 256 ^ 8 ∧
  instruction.executionPrice < 256 ^ 8 ∧
  IntentEncodable instruction.seller ∧
  IntentEncodable instruction.buyer

/-- Hostile decoder for the exact matcher envelope. Nested intents are decoded
by the same canonical decoder used for standalone signature payloads. -/
def decodeControllerInstructionV1
    (bytes : List UInt8) : Option ControllerInstructionV1 := do
  if bytes.length != controllerInstructionBytes then none else
  if bytes.take versionOffset != controllerMagic then none else
  let wireVersion := decodeLE ((bytes.drop versionOffset).take 2)
  if wireVersion != version then none else
  let controllerBump ← bytes[(ControllerField.offset .controllerBump)]?
  let sellerReplayBump ← bytes[(ControllerField.offset .sellerReplayBump)]?
  let buyerReplayBump ← bytes[(ControllerField.offset .buyerReplayBump)]?
  let sellerPositionBump ← bytes[(ControllerField.offset .sellerPositionBump)]?
  let buyerPositionBump ← bytes[(ControllerField.offset .buyerPositionBump)]?
  if (bytes.drop controllerReservedOffset).take controllerReservedWidth !=
      zeros controllerReservedWidth then none else
  let fill := decodeLE
    ((bytes.drop (ControllerField.offset .fill)).take (ControllerField.width .fill))
  let executionPrice := decodeLE
    ((bytes.drop (ControllerField.offset .executionPrice)).take
      (ControllerField.width .executionPrice))
  let seller ← decodeCompactIntentV1
    ((bytes.drop (ControllerField.offset .seller)).take (ControllerField.width .seller))
  let buyer ← decodeCompactIntentV1
    ((bytes.drop (ControllerField.offset .buyer)).take (ControllerField.width .buyer))
  some {
    controllerBump, sellerReplayBump, buyerReplayBump,
    sellerPositionBump, buyerPositionBump, fill, executionPrice, seller, buyer
  }

/-- Every representable matcher envelope survives canonical serialization and
hostile decoding exactly, including both independently signed child intents. -/
theorem decodeControllerInstructionV1_encode
    (instruction : ControllerInstructionV1)
    (encodable : ControllerEncodable instruction) :
    decodeControllerInstructionV1 (encodeControllerInstructionV1 instruction) =
      some instruction := by
  rcases encodable with ⟨fillFits, executionPriceFits, sellerFits, buyerFits⟩
  have fillDecoded := decodeLE_encodeLE 8 instruction.fill fillFits
  have executionPriceDecoded :=
    decodeLE_encodeLE 8 instruction.executionPrice executionPriceFits
  have sellerDecoded := decodeCompactIntentV1_encode instruction.seller sellerFits
  have buyerDecoded := decodeCompactIntentV1_encode instruction.buyer buyerFits
  simp [decodeControllerInstructionV1, encodeControllerInstructionV1,
    controllerMagic, versionOffset, version, controllerInstructionBytes,
    compactIntentBytes, ControllerField.offset, ControllerField.width,
    controllerReservedOffset, controllerReservedWidth,
    List.drop_append, List.take_append, List.drop_eq_nil_of_le,
    List.take_of_length_le, encodeLE_length, encodeCompactIntentV1_length,
    zeros, fillDecoded, executionPriceDecoded, sellerDecoded, buyerDecoded]
  native_decide

namespace Examples

def bytes32 (byte : UInt8) : Bytes32 := fun _ => byte

def sellerIntent : CompactIntentV1 := {
  side := 0
  outcome := 1
  lifecycle := 0
  market := bytes32 4
  generation := 3
  nonce := 0
  validFrom := 0
  validThrough := 18446744073709551615
  maximumFill := 2000
  limitPrice := 400000
  feeBasisPoints := 25
  collateralAccount := bytes32 5
}

def buyerIntent : CompactIntentV1 := {
  side := 1
  outcome := 1
  lifecycle := 0
  market := bytes32 4
  generation := 3
  nonce := 0
  validFrom := 0
  validThrough := 18446744073709551615
  maximumFill := 2000
  limitPrice := 600000
  feeBasisPoints := 25
  collateralAccount := bytes32 6
}

def controllerInstruction : ControllerInstructionV1 := {
  controllerBump := 1
  sellerReplayBump := 2
  buyerReplayBump := 3
  sellerPositionBump := 4
  buyerPositionBump := 5
  fill := 2000
  executionPrice := 500000
  seller := sellerIntent
  buyer := buyerIntent
}

theorem concrete_lengths :
    (encodeCompactIntentV1 sellerIntent).length = 136 ∧
    (encodeCompactIntentV1 buyerIntent).length = 136 ∧
    (encodeControllerInstructionV1 controllerInstruction).length = 304 := by
  native_decide

theorem concrete_roundtrips :
    decodeCompactIntentV1 (encodeCompactIntentV1 sellerIntent) = some sellerIntent ∧
    decodeCompactIntentV1 (encodeCompactIntentV1 buyerIntent) = some buyerIntent ∧
    decodeControllerInstructionV1
        (encodeControllerInstructionV1 controllerInstruction) =
      some controllerInstruction := by
  exact ⟨
    decodeCompactIntentV1_encode sellerIntent (by simp [IntentEncodable, sellerIntent]),
    decodeCompactIntentV1_encode buyerIntent (by simp [IntentEncodable, buyerIntent]),
    decodeControllerInstructionV1_encode controllerInstruction (by
      simp [ControllerEncodable, IntentEncodable, controllerInstruction,
        sellerIntent, buyerIntent])
  ⟩

/-- Executable hostile witnesses cover every canonical envelope guard. The
general roundtrip theorems above establish acceptance independently. -/
theorem hostile_intent_decodings_refuse :
    decodeCompactIntentV1 [] = none ∧
    decodeCompactIntentV1 (encodeCompactIntentV1 sellerIntent |>.drop 1) = none ∧
    decodeCompactIntentV1
        (List.set (encodeCompactIntentV1 sellerIntent) magicOffset 0) = none ∧
    decodeCompactIntentV1
        (List.set (encodeCompactIntentV1 sellerIntent) versionOffset 2) = none ∧
    decodeCompactIntentV1
        (List.set (encodeCompactIntentV1 sellerIntent) intentReservedAOffset 1) = none ∧
    decodeCompactIntentV1
        (List.set (encodeCompactIntentV1 sellerIntent) intentReservedBOffset 1) = none := by
  set_option maxRecDepth 10000 in
    exact ⟨rfl, rfl, rfl, rfl, rfl, rfl⟩

theorem hostile_controller_decodings_refuse :
    decodeControllerInstructionV1 [] = none ∧
    decodeControllerInstructionV1
        (encodeControllerInstructionV1 controllerInstruction |>.drop 1) = none ∧
    decodeControllerInstructionV1
        (List.set (encodeControllerInstructionV1 controllerInstruction) magicOffset 0) = none ∧
    decodeControllerInstructionV1
        (List.set (encodeControllerInstructionV1 controllerInstruction) versionOffset 2) = none ∧
    decodeControllerInstructionV1
        (List.set (encodeControllerInstructionV1 controllerInstruction)
          controllerReservedOffset 1) = none := by
  set_option maxRecDepth 10000 in
    exact ⟨rfl, rfl, rfl, rfl, rfl⟩

end Examples

end DClutch.DirectControllerCodec
