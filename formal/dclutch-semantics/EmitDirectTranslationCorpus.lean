import DClutchSemantics.DirectControllerCodec
import DClutchSemantics.DirectProgram
import DClutchSemantics.RegisteredControllerAbi

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

namespace RegisteredTerminalCorpus

open DClutch.Direct
open DClutch.DirectLifecycle
open DClutch.Direct.RegisteredControllerAbi

def actionTag : RegisteredPhysical.TerminalAction → Nat
  | .cancel => 0
  | .expire => 1

def actionName : RegisteredPhysical.TerminalAction → String
  | .cancel => "cancel"
  | .expire => "expire"

def actions : List RegisteredPhysical.TerminalAction := [.cancel, .expire]

def boundaries : List Nat := [
  0, 1, 2, 254, 255, 256, 257, 65534, 65535, 65536,
  4294967295, 4294967296, 18446744073709551614, 18446744073709551615
]

def atCycle (index : Nat) : Nat :=
  boundaries[index % boundaries.length]?.getD 0

def changedByte (byte : UInt8) : UInt8 := UInt8.ofNat (byte.toNat + 1)

def acceptance (value : Option α) : String :=
  if value.isSome then "accept" else "reject"

def controllerInstruction
    (action : RegisteredPhysical.TerminalAction) (index : Nat) : Terminal.InstructionV1 := {
  action
  controllerBump := UInt8.ofNat (index * 11 + 2)
  registrationBump := UInt8.ofNat (index * 13 + 3)
  expectedSequence := atCycle index
}

def caseName (action : RegisteredPhysical.TerminalAction) (index : Nat) : String :=
  s!"{actionName action}-{index}"

def emitController (action : RegisteredPhysical.TerminalAction) (index : Nat) : IO Unit := do
  let instruction := controllerInstruction action index
  let name := caseName action index
  let encoded := Terminal.encode instruction
  IO.println <| String.intercalate "|" [
    "terminal-controller", name, toString (actionTag action),
    toString instruction.controllerBump.toNat,
    toString instruction.registrationBump.toNat,
    toString instruction.expectedSequence, hex encoded
  ]
  for offset in List.range encoded.length do
    let mutated := encoded.set offset (changedByte (encoded[offset]?.getD 0))
    IO.println <| String.intercalate "|" [
      "terminal-controller-mutation", name, toString offset,
      acceptance (Terminal.decode mutated)
    ]
  for width in List.range encoded.length do
    let truncated := encoded.take width
    IO.println <| String.intercalate "|" [
      "terminal-controller-hostile", name, s!"truncate-{width}",
      hex truncated, acceptance (Terminal.decode truncated)
    ]
  let padded := encoded ++ [0]
  IO.println <| String.intercalate "|" [
    "terminal-controller-hostile", name, "pad-1", hex padded,
    acceptance (Terminal.decode padded)
  ]

def emitClaim (action : RegisteredPhysical.TerminalAction) (index : Nat) : IO Unit := do
  let sequence := atCycle index
  let name := caseName action index
  let encoded := RegisteredPhysical.encodeTerminalInstruction action sequence
  IO.println <| String.intercalate "|" [
    "terminal-claim", name, toString (actionTag action),
    toString sequence, hex encoded
  ]
  for offset in List.range encoded.length do
    let mutated := encoded.set offset (changedByte (encoded[offset]?.getD 0))
    IO.println <| String.intercalate "|" [
      "terminal-claim-mutation", name, toString offset,
      acceptance (RegisteredPhysical.decodeTerminalInstruction mutated)
    ]
  for width in List.range encoded.length do
    let truncated := encoded.take width
    IO.println <| String.intercalate "|" [
      "terminal-claim-hostile", name, s!"truncate-{width}",
      hex truncated,
      acceptance (RegisteredPhysical.decodeTerminalInstruction truncated)
    ]
  let padded := encoded ++ [0]
  IO.println <| String.intercalate "|" [
    "terminal-claim-hostile", name, "pad-1", hex padded,
    acceptance (RegisteredPhysical.decodeTerminalInstruction padded)
  ]

def intent (maker maximum validThrough : Nat) : Intent := {
  market := 101
  generation := 3
  maker
  nonce := 0
  validFromSlot := 10
  validThroughSlot := validThrough
  side := .sell
  lifecycle := .goodTillCancelled
  outcome := 1
  maxFill := maximum
  limitPrice := 500000
  feeBasisPoints := 25
}

def state (phase : DirectLifecycle.Phase) (remaining maximum sequence validThrough maker : Nat) :
    DirectLifecycle.State := {
  terms := intent maker maximum validThrough
  phase
  remaining
  sequence
}

structure TransitionCase where
  name : String
  action : RegisteredPhysical.TerminalAction
  state : DirectLifecycle.State
  slot : Nat
  expectedSequence : Nat
  actorMaker : Nat

def transition (test : TransitionCase) : Option DirectLifecycle.State :=
  match test.action with
  | .cancel =>
      if test.actorMaker = test.state.terms.maker then
        DirectLifecycle.cancel { state := test.state, expectedSequence := test.expectedSequence }
      else none
  | .expire =>
      DirectLifecycle.expire {
        state := test.state
        slot := test.slot
        expectedSequence := test.expectedSequence
      }

def transitionCases : List TransitionCase := [
  ⟨"cancel", .cancel, state .open 100 100 7 20 11, 0, 7, 11⟩,
  ⟨"cancel-stale-sequence", .cancel, state .open 100 100 7 20 11, 0, 6, 11⟩,
  ⟨"cancel-sequence-overflow", .cancel,
    state .open 100 100 18446744073709551615 20 11,
    0, 18446744073709551615, 11⟩,
  ⟨"cancel-wrong-maker", .cancel, state .open 100 100 7 20 11, 0, 7, 12⟩,
  ⟨"cancel-cancelled", .cancel, state .cancelled 100 100 7 20 11, 0, 7, 11⟩,
  ⟨"cancel-expired", .cancel, state .expired 100 100 7 20 11, 0, 7, 11⟩,
  ⟨"cancel-filled", .cancel, state .filled 0 100 7 20 11, 0, 7, 11⟩,
  ⟨"cancel-invalid-open-zero", .cancel, state .open 0 100 7 20 11, 0, 7, 11⟩,
  ⟨"cancel-invalid-remaining", .cancel, state .open 101 100 7 20 11, 0, 7, 11⟩,
  ⟨"expire", .expire, state .open 100 100 7 20 11, 21, 7, 99⟩,
  ⟨"expire-at-boundary", .expire, state .open 100 100 7 20 11, 20, 7, 99⟩,
  ⟨"expire-before-boundary", .expire, state .open 100 100 7 20 11, 19, 7, 99⟩,
  ⟨"expire-stale-sequence", .expire, state .open 100 100 7 20 11, 21, 6, 99⟩,
  ⟨"expire-sequence-overflow", .expire,
    state .open 100 100 18446744073709551615 20 11,
    21, 18446744073709551615, 99⟩,
  ⟨"expire-cancelled", .expire, state .cancelled 100 100 7 20 11, 21, 7, 99⟩,
  ⟨"expire-expired", .expire, state .expired 100 100 7 20 11, 21, 7, 99⟩,
  ⟨"expire-filled", .expire, state .filled 0 100 7 20 11, 21, 7, 99⟩,
  ⟨"expire-invalid-open-zero", .expire, state .open 0 100 7 20 11, 21, 7, 99⟩,
  ⟨"expire-invalid-remaining", .expire, state .open 101 100 7 20 11, 21, 7, 99⟩
]

def emitTransition (test : TransitionCase) : IO Unit := do
  let fields := [
    "terminal-transition", test.name, toString (actionTag test.action),
    toString (DirectLifecycleProgram.phaseTag test.state.phase),
    toString test.state.remaining, toString test.state.terms.maxFill,
    toString test.state.sequence, toString test.state.terms.validThroughSlot,
    toString test.slot, toString test.expectedSequence,
    toString test.state.terms.maker, toString test.actorMaker
  ]
  match transition test with
  | none => IO.println <| String.intercalate "|" (fields ++ ["reject"])
  | some result =>
      IO.println <| String.intercalate "|" (fields ++ [
        "accept", toString (DirectLifecycleProgram.phaseTag result.phase),
        toString result.remaining, toString result.terms.maxFill,
        toString result.sequence, toString result.terms.validThroughSlot,
        toString result.terms.maker
      ])

end RegisteredTerminalCorpus

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

def emitVmProgramCase (name : String) (program : List Op) (initial : State) : IO Unit := do
  let fields := ["vm-program", name,
    hex (TransitionVM.Codec.encodeProgram program),
    natCsv initial.scalars, natCsv initial.identities]
  match run program initial with
  | none => IO.println <| String.intercalate "|" (fields ++ ["reject"])
  | some final =>
      IO.println <| String.intercalate "|"
        (fields ++ ["accept", natCsv final.scalars, natCsv final.identities])

def microProgramCases : List (String × List Op × State) := [
  ("gtc-partial", [.lifecycleAccepts 0 1 2], { scalars := #[2, 100, 35], identities := #[] }),
  ("subtract", [.subInto 0 1 2], { scalars := #[100, 35, 999], identities := #[] }),
  ("subtract-underflow", [.subInto 0 1 2],
    { scalars := #[35, 100, 999], identities := #[] }),
  ("select-equal", [.selectEq 0 1 2 3],
    { scalars := #[7, 7, 42, 9], identities := #[] }),
  ("select-unequal", [.selectEq 0 1 2 3],
    { scalars := #[7, 8, 42, 9], identities := #[] }),
  ("select-zero", [.selectZero 0 1 2],
    { scalars := #[0, 42, 9], identities := #[] }),
  ("select-nonzero", [.selectZero 0 1 2],
    { scalars := #[1, 42, 9], identities := #[] })
]

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
  for action in RegisteredTerminalCorpus.actions do
    for index in List.range RegisteredTerminalCorpus.boundaries.length do
      RegisteredTerminalCorpus.emitController action index
      RegisteredTerminalCorpus.emitClaim action index
  for test in RegisteredTerminalCorpus.transitionCases do
    RegisteredTerminalCorpus.emitTransition test
  for entry in microProgramCases do
    emitVmProgramCase entry.1 entry.2.1 entry.2.2
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
