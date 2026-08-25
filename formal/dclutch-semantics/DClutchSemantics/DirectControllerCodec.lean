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

end Examples

end DClutch.DirectControllerCodec
