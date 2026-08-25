import DClutchSemantics.EconomicExamples

/-!
# Canonical fixture encoding for the shared economic microkernel

`DCES` encodes only the active state prefix.  The bounded Rust refinement
hostile-decodes it into fixed-capacity arrays and proves the inactive tail is
zero by construction.  Claim and custody plans retain the existing `DCEF` and
`DCCP` encodings; this module creates no parallel effect vocabulary.
-/

namespace DClutch.Economic.Codec

open DClutch

def stateMagic : List UInt8 := [0x44, 0x43, 0x45, 0x53] -- `DCES`
def stateVersion : UInt8 := 1
def stateHeaderBytes : Nat := 16
def stateVectorCount : Nat := 7

def phaseTag : Phase → UInt8
  | .open => 0
  | .terminal _ => 1
  | .retiring _ => 2
  | .retired => 3

def phaseWinner : Phase → Nat
  | .terminal winner | .retiring winner => winner
  | .open | .retired => 0

def encodeVector (outcomeCount : Nat) (values : List Nat) : List UInt8 :=
  (values.take outcomeCount).flatMap (DClutch.Codec.encodeLE 8)

/-- Canonical compact state fixture.  `valid` supplies the exact vector lengths
at execution time; the Rust hostile decoder independently checks count, length,
phase shape, partition, projection bounds, and solvency. -/
def encodeState (outcomeCount : Nat) (state : State) : List UInt8 :=
  stateMagic ++ [
    stateVersion,
    phaseTag state.phase,
    UInt8.ofNat outcomeCount,
    UInt8.ofNat (phaseWinner state.phase)
  ] ++ DClutch.Codec.encodeLE 8 state.hoard ++
  encodeVector outcomeCount state.supply ++
  encodeVector outcomeCount state.nativeSupply ++
  encodeVector outcomeCount state.materializedSupply ++
  encodeVector outcomeCount state.sourceNative ++
  encodeVector outcomeCount state.sourceMaterialized ++
  encodeVector outcomeCount state.destinationNative ++
  encodeVector outcomeCount state.destinationMaterialized

def encodeClaimPlan (frame : Frame) : List UInt8 :=
  DClutch.Codec.encodePlan (compile frame).claimEffects

def encodeCustodyPlan (frame : Frame) : List UInt8 :=
  DClutch.Direct.Physical.Codec.encodeCustodyPlan (compile frame).custodyTransfers

theorem split_state_fixture_length :
    (encodeState Examples.splitFrame.outcomeCount Examples.splitFrame.pre).length =
      stateHeaderBytes + stateVectorCount * Examples.splitFrame.outcomeCount * 8 := by
  native_decide

theorem split_claim_fixture_length :
    (encodeClaimPlan Examples.splitFrame).length = 56 := by
  native_decide

theorem split_custody_fixture_length :
    (encodeCustodyPlan Examples.splitFrame).length = 24 := by
  native_decide

end DClutch.Economic.Codec
