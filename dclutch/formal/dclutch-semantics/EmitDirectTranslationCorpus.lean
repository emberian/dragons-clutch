import DClutchSemantics.DirectControllerCodec
import DClutchSemantics.DirectProgram

/-!
# Direct translation-validation corpus

This executable does not generate implementation source. It evaluates the
Lean-owned ABI decoder/encoder and transition program over deterministic
boundary and hostile cases. The independent Rust validator consumes the
result and compares the public safe-Rust codec and interpreter with these
observations.
-/

open DClutch
open DClutch.Codec
open DClutch.Direct
open DClutch.DirectControllerCodec
open DClutch.TransitionVM

def natCsv (values : Array Nat) : String :=
  String.intercalate "," (values.toList.map toString)

def bytes32 (seed : Nat) : Bytes32 := fun index =>
  UInt8.ofNat (seed + index.val * 29)

def boundary64 : List Nat := [
  0, 1, 2, 254, 255, 256, 257, 65534, 65535, 65536,
  4294967295, 4294967296, 18446744073709551614, 18446744073709551615
]

def boundary16 : List Nat := [0, 1, 254, 255, 256, 257, 65534, 65535]

def atCycle (values : List Nat) (index : Nat) : Nat :=
  values[index % values.length]?.getD 0

def intentFor (index : Nat) : CompactIntentV1 := {
  side := UInt8.ofNat (index * 17)
  outcome := UInt8.ofNat (index * 31 + 1)
  lifecycle := UInt8.ofNat (index * 47 + 2)
  market := bytes32 (index * 19 + 3)
  generation := atCycle boundary64 index
  nonce := atCycle boundary64 (index + 3)
  validFrom := atCycle boundary64 (index + 5)
  validThrough := atCycle boundary64 (index + 7)
  maximumFill := atCycle boundary64 (index + 9)
  limitPrice := atCycle boundary64 (index + 11)
  feeBasisPoints := atCycle boundary16 index
  collateralAccount := bytes32 (index * 23 + 11)
}

def intentName (index : Nat) : String := s!"intent-{index}"

def changedByte (byte : UInt8) : UInt8 := UInt8.ofNat (byte.toNat + 1)

def acceptance (value : Option α) : String :=
  if value.isSome then "accept" else "reject"

def emitIntent (index : Nat) : IO Unit := do
  let intent := intentFor index
  IO.println <| String.intercalate "|" [
    "intent", intentName index,
    toString intent.side.toNat,
    toString intent.outcome.toNat,
    toString intent.lifecycle.toNat,
    hex (encodeBytes32 intent.market),
    toString intent.generation,
    toString intent.nonce,
    toString intent.validFrom,
    toString intent.validThrough,
    toString intent.maximumFill,
    toString intent.limitPrice,
    toString intent.feeBasisPoints,
    hex (encodeBytes32 intent.collateralAccount),
    hex (encodeCompactIntentV1 intent)
  ]
  let encoded := encodeCompactIntentV1 intent
  for offset in List.range encoded.length do
    let mutated := encoded.set offset (changedByte (encoded[offset]?.getD 0))
    IO.println <| String.intercalate "|" [
      "intent-mutation", intentName index, toString offset,
      acceptance (decodeCompactIntentV1 mutated)
    ]

def controllerFor (index : Nat) : ControllerInstructionV1 := {
  controllerBump := UInt8.ofNat (index * 7 + 1)
  sellerReplayBump := UInt8.ofNat (index * 7 + 2)
  buyerReplayBump := UInt8.ofNat (index * 7 + 3)
  sellerPositionBump := UInt8.ofNat (index * 7 + 4)
  buyerPositionBump := UInt8.ofNat (index * 7 + 5)
  fill := atCycle boundary64 (index * 2)
  executionPrice := atCycle boundary64 (index * 2 + 1)
  seller := intentFor (index * 2)
  buyer := intentFor (index * 2 + 1)
}

def controllerName (index : Nat) : String := s!"controller-{index}"

def emitController (index : Nat) : IO Unit := do
  let instruction := controllerFor index
  IO.println <| String.intercalate "|" [
    "controller", controllerName index,
    toString instruction.controllerBump.toNat,
    toString instruction.sellerReplayBump.toNat,
    toString instruction.buyerReplayBump.toNat,
    toString instruction.sellerPositionBump.toNat,
    toString instruction.buyerPositionBump.toNat,
    toString instruction.fill,
    toString instruction.executionPrice,
    intentName (index * 2),
    intentName (index * 2 + 1),
    hex (encodeControllerInstructionV1 instruction)
  ]
  let encoded := encodeControllerInstructionV1 instruction
  for offset in List.range encoded.length do
    let mutated := encoded.set offset (changedByte (encoded[offset]?.getD 0))
    IO.println <| String.intercalate "|" [
      "controller-mutation", controllerName index, toString offset,
      acceptance (decodeControllerInstructionV1 mutated)
    ]

def setScalars (state : State) (patches : List (Nat × Nat)) : State := {
  state with
  scalars := patches.foldl
    (fun values patch => values.setIfInBounds patch.1 patch.2)
    state.scalars
}

def setIdentity (state : State) (index value : Nat) : State := {
  state with identities := state.identities.setIfInBounds index value
}

def emitVmCase (name : String) (initial : State) : IO Unit := do
  let fields := ["vm", name, natCsv initial.scalars, natCsv initial.identities]
  match run DClutch.DirectProgram.program initial with
  | none => IO.println <| String.intercalate "|" (fields ++ ["reject"])
  | some final =>
      IO.println <| String.intercalate "|"
        (fields ++ ["accept", natCsv final.scalars, natCsv final.identities])

def baseState : State := DClutch.DirectProgram.state DClutch.Direct.Examples.frame

def coordinatedStates : List (String × State) := [
  ("baseline", baseState),
  ("ioc-partial", setScalars baseState [
    (13, 1), (14, 3000), (15, 1), (16, 3000), (28, 1000)
  ]),
  ("zero-fee", setScalars baseState [(25, 0), (26, 0), (27, 0)]),
  ("max-price", setScalars baseState [
    (14, 1), (16, 1), (21, 18446744073709551615),
    (22, 18446744073709551615), (23, 18446744073709551615),
    (24, 18446744073709551615), (25, 0), (26, 0), (27, 0),
    (28, 1), (29, 1), (30, 0), (31, 1), (32, 0), (33, 0)
  ]),
  ("max-fill", setScalars baseState [
    (14, 18446744073709551615), (16, 18446744073709551615),
    (21, 1), (22, 1), (23, 1), (24, 1),
    (25, 0), (26, 0), (27, 0), (28, 18446744073709551615),
    (29, 18446744073709551615), (30, 0),
    (31, 18446744073709551615), (32, 0), (33, 0)
  ]),
  ("max-fee", setScalars baseState [
    (14, 10), (16, 10), (21, 1), (22, 1), (23, 1), (24, 1),
    (25, 10000), (26, 10000), (27, 10000), (28, 10),
    (29, 10), (30, 0), (31, 20), (32, 0), (33, 0)
  ])
]

def main : IO Unit := do
  IO.println "dclutch-direct-translation-corpus-v1"
  IO.println s!"program|{hex (TransitionVM.Codec.encodeProgram DClutch.DirectProgram.program)}"
  for index in List.range boundary64.length do
    emitIntent index
  for index in List.range (boundary64.length / 2) do
    emitController index
  for entry in coordinatedStates do
    emitVmCase entry.1 entry.2
  for scalarIndex in List.range DClutch.DirectProgram.ScalarSlot.inputs.length do
    for valueIndex in List.range boundary64.length do
      emitVmCase s!"scalar-{scalarIndex}-{valueIndex}"
        (setScalars baseState [(scalarIndex, atCycle boundary64 valueIndex)])
  for identityIndex in List.range DClutch.DirectProgram.Identity.count do
    for value in [0, 1, 11, 12, 101, 255, 65535, 18446744073709551615] do
      emitVmCase s!"identity-{identityIndex}-{value}"
        (setIdentity baseState identityIndex value)
