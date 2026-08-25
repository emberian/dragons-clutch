import DClutchSemantics.SbfProfile
import DClutchSemantics.Physical
import DClutchSemantics.Examples

/-!
# Exact-account claim-executor Solana ABI-v1 profile

This specializes the common loader-v1 layout arithmetic to the claim-only
projection and the four effects derived by `Physical.physicalPlan`.
-/

namespace DClutch.ClaimSbfProfile

open DClutch
open DClutch.SbfProfile

def projectionRole : AccountRole := {
  signer := false
  writable := true
  executable := false
  dataBytes := 80
}

def accountCountOffset : Nat := 0
def authorityOffset : Nat := 8
def projectionOffset : Nat := authorityOffset + accountSpan authorityRole
def projectionDataOffset : Nat := projectionOffset + accountHeaderBytes
def instructionLengthOffset : Nat := projectionOffset + accountSpan projectionRole
def instructionOffset : Nat := instructionLengthOffset + 8

def planBytes : List UInt8 :=
  DClutch.Codec.encodePlan
    (DClutch.Direct.Physical.physicalPlan DClutch.Direct.Examples.frame).claimEffects

def instructionBytes : Nat := planBytes.length
def programIdOffset : Nat := instructionOffset + instructionBytes

def planWord (index : Nat) : Nat :=
  SbfProfile.wordAt planBytes (8 * index)

def effectMetadata (index : Nat) : Nat := planWord (1 + 2 * index)
def effectTag (index : Nat) : Nat := effectMetadata index % (2 ^ 32)

def stateMagicWord : Nat :=
  SbfProfile.decodeLE [0x44, 0x43, 0x43, 0x53, 1, 0, 0, 0] -- `DCCS`, V1

theorem exact_offsets :
    authorityOffset = 8 ∧
    projectionOffset = 10344 ∧
    projectionDataOffset = 10432 ∧
    instructionLengthOffset = 20760 ∧
    instructionOffset = 20768 ∧
    programIdOffset = 20840 := by
  native_decide

theorem exact_frames :
    accountFrameWord authorityRole = 511 ∧
    accountFrameWord projectionRole = 65791 := by
  native_decide

theorem exact_instruction_shape :
    instructionBytes = 72 ∧
    planWord 0 = 4403520422724 ∧
    effectTag 0 = 0 ∧
    effectTag 1 = 256 ∧
    effectTag 2 = 65537 ∧
    effectTag 3 = 65794 := by
  native_decide

end DClutch.ClaimSbfProfile
