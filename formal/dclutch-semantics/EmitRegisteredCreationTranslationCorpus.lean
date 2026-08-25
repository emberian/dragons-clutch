import DClutchSemantics.DirectLifecycle
import DClutchSemantics.RegisteredControllerAbi

/-!
# Registered-creation translation-validation fixture

This disjoint executable emits finite ABI and semantic observations for the
independent safe-Rust translation validator. It does not generate shipping
Rust, an SBF program, or a deployment artifact.
-/

open DClutch
open DClutch.Codec
open DClutch.Direct
open DClutch.DirectControllerCodec
open DClutch.DirectLifecycle
open DClutch.Direct.RegisteredControllerAbi

namespace RegisteredCreationTranslationCorpus

def boundary64 : List Nat := [
  0, 1, 2, 254, 255, 256, 257, 65534, 65535, 65536,
  4294967295, 4294967296, 18446744073709551614, 18446744073709551615
]

def boundary16 : List Nat := [0, 1, 254, 255, 256, 257, 65534, 65535]

def atCycle (values : List Nat) (index : Nat) : Nat :=
  values[index % values.length]?.getD 0

def bytes32 (seed : Nat) : Bytes32 := fun index =>
  UInt8.ofNat (seed + index.val * 29)

def compactIntentFor (index : Nat) : CompactIntentV1 := {
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

def instructionFor (index : Nat) : Registration.InstructionV1 := {
  controllerBump := UInt8.ofNat (index * 11 + 2)
  replayBump := UInt8.ofNat (index * 13 + 3)
  registrationBump := UInt8.ofNat (index * 17 + 5)
  intent := compactIntentFor index
}

def caseName (index : Nat) : String := s!"create-{index}"

def changedByte (byte : UInt8) : UInt8 := UInt8.ofNat (byte.toNat + 1)

def acceptance (value : Option α) : String :=
  if value.isSome then "accept" else "reject"

def emitInstruction (index : Nat) : IO Unit := do
  let instruction := instructionFor index
  let name := caseName index
  let encoded := Registration.encode instruction
  IO.println <| String.intercalate "|" [
    "registered-create", name,
    toString instruction.controllerBump.toNat,
    toString instruction.replayBump.toNat,
    toString instruction.registrationBump.toNat,
    hex (encodeBytes32 instruction.intent.market),
    toString instruction.intent.generation, toString instruction.intent.nonce,
    hex (encodeCompactIntentV1 instruction.intent), hex encoded
  ]
  for offset in List.range encoded.length do
    let mutated := encoded.set offset (changedByte (encoded[offset]?.getD 0))
    IO.println <| String.intercalate "|" [
      "registered-create-mutation", name, toString offset,
      acceptance (Registration.decode mutated)
    ]
  for width in List.range encoded.length do
    let truncated := encoded.take width
    IO.println <| String.intercalate "|" [
      "registered-create-hostile", name, s!"truncate-{width}",
      hex truncated, acceptance (Registration.decode truncated)
    ]
  let padded := encoded ++ [0]
  IO.println <| String.intercalate "|" [
    "registered-create-hostile", name, "pad-1", hex padded,
    acceptance (Registration.decode padded)
  ]

def product4 : ProductIR := {
  outcomeCount := 4
  outcomeCountPositive := by decide
  priceScale := 1000000
  priceScalePositive := by decide
}

def policy25 : FeePolicy := {
  basisPoints := 25
  basisPointsBounded := by decide
}

def policy26 : FeePolicy := {
  basisPoints := 26
  basisPointsBounded := by decide
}

def semanticIntent
    (market generation maker nonce validFrom validThrough outcome maximum fee : Nat) :
    Intent := {
  market
  generation
  maker
  nonce
  validFromSlot := validFrom
  validThroughSlot := validThrough
  side := .buy
  lifecycle := .goodTillCancelled
  outcome
  maxFill := maximum
  limitPrice := 500000
  feeBasisPoints := fee
}

structure TransitionCase where
  name : String
  vacant : Bool
  marketPhase : Direct.Phase
  slot : Nat
  intent : Intent
  makerNextNonce : Nat
  feePolicy : FeePolicy

def marketPhaseTag : Direct.Phase → Nat
  | .founding => 0
  | .open => 1
  | .resolved => 2
  | .retiring => 3
  | .retired => 4

def registrationPhaseTag : DirectLifecycle.Phase → Nat
  | .open => 0
  | .filled => 1
  | .cancelled => 2
  | .expired => 3

def transition (test : TransitionCase) : Option (DirectLifecycle.State × Nat) :=
  DirectLifecycle.register {
    product := product4
    feePolicy := test.feePolicy
    marketPhase := test.marketPhase
    slot := test.slot
    intent := test.intent
    makerNextNonce := test.makerNextNonce
    vacant := test.vacant
  }

def transitionCase
    (name : String) (vacant : Bool) (marketPhase : Direct.Phase) (slot : Nat)
    (intent : Intent) (makerNextNonce : Nat) (feePolicy : FeePolicy) : TransitionCase := {
  name, vacant, marketPhase, slot, intent, makerNextNonce, feePolicy
}

def transitionCases : List TransitionCase := [
  transitionCase "create-first" true .open 10
    (semanticIntent 101 3 11 0 10 20 1 100 25) 0 policy25,
  transitionCase "create-reused-replay" true .open 10
    (semanticIntent 101 3 11 1 10 20 1 100 25) 1 policy25,
  transitionCase "create-slot-boundary" true .open 20
    (semanticIntent 101 3 11 7 10 20 1 100 25) 7 policy25,
  transitionCase "create-future-valid-from" true .open 10
    (semanticIntent 101 3 11 7 50 100 1 100 25) 7 policy25,
  transitionCase "create-maximum-fill" true .open 10
    (semanticIntent 101 3 11 7 10 20 1 18446744073709551615 25) 7 policy25,
  transitionCase "create-next-nonce-maximum" true .open 10
    (semanticIntent 101 3 11 18446744073709551614 10 20 1 100 25)
    18446744073709551614 policy25,
  transitionCase "create-occupied" false .open 10
    (semanticIntent 101 3 11 7 10 20 1 100 25) 7 policy25,
  transitionCase "create-founding" true .founding 10
    (semanticIntent 101 3 11 7 10 20 1 100 25) 7 policy25,
  transitionCase "create-resolved" true .resolved 10
    (semanticIntent 101 3 11 7 10 20 1 100 25) 7 policy25,
  transitionCase "create-reversed-window" true .open 10
    (semanticIntent 101 3 11 7 21 20 1 100 25) 7 policy25,
  transitionCase "create-expired" true .open 21
    (semanticIntent 101 3 11 7 10 20 1 100 25) 7 policy25,
  transitionCase "create-zero-maximum" true .open 10
    (semanticIntent 101 3 11 7 10 20 1 0 25) 7 policy25,
  transitionCase "create-outcome-out-of-range" true .open 10
    (semanticIntent 101 3 11 7 10 20 4 100 25) 7 policy25,
  transitionCase "create-fee-mismatch" true .open 10
    (semanticIntent 101 3 11 7 10 20 1 100 25) 7 policy26,
  transitionCase "create-skipped-replay-nonce" true .open 10
    (semanticIntent 101 3 11 3 10 20 1 100 25) 2 policy25,
  transitionCase "create-reused-replay-nonce" true .open 10
    (semanticIntent 101 3 11 1 10 20 1 100 25) 2 policy25,
  transitionCase "create-replay-nonce-overflow" true .open 10
    (semanticIntent 101 3 11 18446744073709551615 10 20 1 100 25)
    18446744073709551615 policy25
]

def emitTransition (test : TransitionCase) : IO Unit := do
  let fields := [
    "registered-create-transition", test.name,
    if test.vacant then "1" else "0", toString (marketPhaseTag test.marketPhase),
    toString test.slot, toString test.intent.market,
    toString test.intent.generation, toString test.intent.maker,
    toString test.intent.nonce, toString test.intent.validFromSlot,
    toString test.intent.validThroughSlot, toString test.intent.maxFill,
    toString test.intent.outcome, toString product4.outcomeCount,
    toString test.intent.feeBasisPoints, toString test.feePolicy.basisPoints,
    toString test.makerNextNonce
  ]
  match transition test with
  | none => IO.println <| String.intercalate "|" (fields ++ ["reject"])
  | some (state, nextNonce) =>
      IO.println <| String.intercalate "|" (fields ++ [
        "accept", toString (registrationPhaseTag state.phase),
        toString state.remaining, toString state.sequence,
        toString state.terms.market, toString state.terms.generation,
        toString state.terms.maker, toString state.terms.nonce,
        toString nextNonce
      ])

end RegisteredCreationTranslationCorpus

def main : IO Unit := do
  for index in List.range RegisteredCreationTranslationCorpus.boundary64.length do
    RegisteredCreationTranslationCorpus.emitInstruction index
  for test in RegisteredCreationTranslationCorpus.transitionCases do
    RegisteredCreationTranslationCorpus.emitTransition test
