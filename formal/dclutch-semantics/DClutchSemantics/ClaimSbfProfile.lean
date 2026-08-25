import DClutchSemantics.SbfProfile
import DClutchSemantics.Physical
import DClutchSemantics.Examples
import DClutchSemantics.RegisteredPhysical
import DClutchSemantics.DirectLifecycleAbi

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

/-! ## Registered-intent fill route

The first account after authority identifies the route by its exact data
length: ordinary fills use a 48-byte replay root; registered fills use the
232-byte canonical registration state.  Both routes retain the same account
count and the same sole owner for Position balances.
-/

namespace Registered

def registrationRole : AccountRole := {
  signer := false
  writable := true
  executable := false
  dataBytes := DClutch.DirectLifecycleAbi.stateBytes
}

def sellerRegistrationOffset : Nat := authorityOffset + accountSpan authorityRole
def buyerRegistrationOffset : Nat := sellerRegistrationOffset + accountSpan registrationRole
def sellerPositionOffset : Nat := buyerRegistrationOffset + accountSpan registrationRole
def buyerPositionOffset : Nat := sellerPositionOffset + accountSpan positionRole
def sellerRegistrationDataOffset : Nat := sellerRegistrationOffset + accountHeaderBytes
def buyerRegistrationDataOffset : Nat := buyerRegistrationOffset + accountHeaderBytes
def sellerPositionDataOffset : Nat := sellerPositionOffset + accountHeaderBytes
def buyerPositionDataOffset : Nat := buyerPositionOffset + accountHeaderBytes
def instructionLengthOffset : Nat := buyerPositionOffset + accountSpan positionRole
def instructionOffset : Nat := instructionLengthOffset + 8
def instructionBytes : Nat := Direct.RegisteredPhysical.instructionBytes
def programIdOffset : Nat := instructionOffset + instructionBytes

def registrationMagicWord : Nat :=
  SbfProfile.decodeLE DClutch.DirectLifecycleAbi.stateMagic

def instructionHeaderWord : Nat :=
  SbfProfile.wordAt Direct.RegisteredPhysical.instructionMagic 0

theorem exact_offsets :
    sellerRegistrationOffset = 10344 ∧
    buyerRegistrationOffset = 20912 ∧
    sellerPositionOffset = 31480 ∧
    buyerPositionOffset = 41872 ∧
    sellerRegistrationDataOffset = 10432 ∧
    buyerRegistrationDataOffset = 21000 ∧
    sellerPositionDataOffset = 31568 ∧
    buyerPositionDataOffset = 41960 ∧
    instructionLengthOffset = 52264 ∧
    instructionOffset = 52272 ∧
    programIdOffset = 52288 := by
  native_decide

theorem exact_frames :
    accountFrameWord registrationRole = 65791 ∧
    registrationRole.dataBytes = 232 ∧
    instructionBytes = 16 := by
  native_decide

end Registered

namespace RegisteredTerminal

def registrationRole : AccountRole := Registered.registrationRole
def registrationOffset : Nat := authorityOffset + accountSpan authorityRole
def registrationDataOffset : Nat := registrationOffset + accountHeaderBytes
def instructionLengthOffset : Nat := registrationOffset + accountSpan registrationRole
def instructionOffset : Nat := instructionLengthOffset + 8
def instructionBytes : Nat := Direct.RegisteredPhysical.terminalInstructionBytes
def programIdOffset : Nat := instructionOffset + instructionBytes

def cancelHeaderWord : Nat :=
  SbfProfile.wordAt
    (Direct.RegisteredPhysical.terminalInstructionMagic .cancel) 0

def expireHeaderWord : Nat :=
  SbfProfile.wordAt
    (Direct.RegisteredPhysical.terminalInstructionMagic .expire) 0

theorem exact_profile :
    registrationOffset = 10344 ∧
    registrationDataOffset = 10432 ∧
    instructionLengthOffset = 20912 ∧
    instructionOffset = 20920 ∧
    instructionBytes = 16 ∧
    programIdOffset = 20936 := by
  native_decide

end RegisteredTerminal

end DClutch.ClaimSbfProfile
