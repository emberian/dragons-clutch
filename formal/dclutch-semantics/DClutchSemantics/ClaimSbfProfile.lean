import DClutchSemantics.SbfProfile
import DClutchSemantics.Physical
import DClutchSemantics.Examples

/-!
# Exact-account claim-executor Solana ABI-v1 profile

This specializes the common loader-v1 layout arithmetic to four canonical
state owners: one replay root and one Position for each maker. No combined
pairwise projection is allowed to own or fragment either fact.
-/

namespace DClutch.ClaimSbfProfile

open DClutch
open DClutch.SbfProfile

def replayRole : AccountRole := {
  signer := false
  writable := true
  executable := false
  dataBytes := 48
}

def positionRole : AccountRole := {
  signer := false
  writable := true
  executable := false
  dataBytes := 56
}

def accountCountOffset : Nat := 0
def authorityOffset : Nat := 8
def sellerReplayOffset : Nat := authorityOffset + accountSpan authorityRole
def buyerReplayOffset : Nat := sellerReplayOffset + accountSpan replayRole
def sellerPositionOffset : Nat := buyerReplayOffset + accountSpan replayRole
def buyerPositionOffset : Nat := sellerPositionOffset + accountSpan positionRole
def sellerReplayDataOffset : Nat := sellerReplayOffset + accountHeaderBytes
def buyerReplayDataOffset : Nat := buyerReplayOffset + accountHeaderBytes
def sellerPositionDataOffset : Nat := sellerPositionOffset + accountHeaderBytes
def buyerPositionDataOffset : Nat := buyerPositionOffset + accountHeaderBytes
def instructionLengthOffset : Nat := buyerPositionOffset + accountSpan positionRole
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

def replayMagicWord : Nat :=
  SbfProfile.decodeLE [0x44, 0x43, 0x52, 0x50, 1, 0, 0, 0] -- `DCRP`, V1

def positionMagicWord : Nat :=
  SbfProfile.decodeLE [0x44, 0x43, 0x50, 0x4e, 1, 0, 0, 0] -- `DCPN`, V1

theorem exact_offsets :
    authorityOffset = 8 ∧
    sellerReplayOffset = 10344 ∧
    buyerReplayOffset = 20728 ∧
    sellerPositionOffset = 31112 ∧
    buyerPositionOffset = 41504 ∧
    sellerReplayDataOffset = 10432 ∧
    buyerReplayDataOffset = 20816 ∧
    sellerPositionDataOffset = 31200 ∧
    buyerPositionDataOffset = 41592 ∧
    instructionLengthOffset = 51896 ∧
    instructionOffset = 51904 ∧
    programIdOffset = 51976 := by
  native_decide

theorem exact_frames :
    accountFrameWord authorityRole = 511 ∧
    accountFrameWord replayRole = 65791 ∧
    accountFrameWord positionRole = 65791 := by
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
