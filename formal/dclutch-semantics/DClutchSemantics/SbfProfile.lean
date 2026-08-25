import DClutchSemantics.Examples

/-!
# Exact-account Solana ABI-v1 proof profile

This file owns the data from which the first alias-simple SBF proof target is
specialized.  It does not model the loader implementation: its account-buffer
formula is an explicit adapter assumption, checked against the pinned Agave ABI
v1 layout when release evidence is produced.
-/

namespace DClutch.SbfProfile

open DClutch

/-- Loader-v1 reserves this many bytes after each account's current data. -/
def maxPermittedDataIncrease : Nat := 10240

/-- Fixed bytes before one non-duplicate account's data. -/
def accountHeaderBytes : Nat := 88

/-- Fixed rent-epoch bytes after data, growth reserve, and alignment. -/
def rentEpochBytes : Nat := 8

/-- One statically specialized loader-v1 account role. -/
structure AccountRole where
  signer : Bool
  writable : Bool
  executable : Bool
  dataBytes : Nat
  deriving DecidableEq, Repr

def authorityRole : AccountRole := {
  signer := true
  writable := false
  executable := false
  dataBytes := 0
}

def projectionRole : AccountRole := {
  signer := false
  writable := true
  executable := false
  dataBytes := 104
}

def boolNat (value : Bool) : Nat := if value then 1 else 0

/-- Packed first eight bytes beginning at a non-duplicate marker.

The last four bytes are loader padding on entry. The SDK deserializer later
overwrites them with original data length, but this proof target deliberately
runs before that SDK transformation.
-/
def accountFrameWord (role : AccountRole) : Nat :=
  255 +
    256 * boolNat role.signer +
    65536 * boolNat role.writable +
    16777216 * boolNat role.executable

def alignPadding8 (length : Nat) : Nat := (8 - length % 8) % 8

def accountSpan (role : AccountRole) : Nat :=
  accountHeaderBytes + role.dataBytes + maxPermittedDataIncrease +
    alignPadding8 role.dataBytes + rentEpochBytes

def accountCountOffset : Nat := 0
def authorityOffset : Nat := 8
def projectionOffset : Nat := authorityOffset + accountSpan authorityRole
def projectionDataOffset : Nat := projectionOffset + accountHeaderBytes
def instructionLengthOffset : Nat := projectionOffset + accountSpan projectionRole
def instructionOffset : Nat := instructionLengthOffset + 8

def planBytes : List UInt8 :=
  DClutch.Codec.encodePlan (DClutch.Direct.effectPlan DClutch.Direct.Examples.frame)

def instructionBytes : Nat := planBytes.length
def programIdOffset : Nat := instructionOffset + instructionBytes

/-- Interpret at most eight little-endian bytes as a natural number. -/
def decodeLE : List UInt8 → Nat
  | [] => 0
  | byte :: rest => byte.toNat + 256 * decodeLE rest

def wordAt (bytes : List UInt8) (offset : Nat) : Nat :=
  decodeLE ((bytes.drop offset).take 8)

def planWord (index : Nat) : Nat := wordAt planBytes (8 * index)
def effectMetadata (index : Nat) : Nat := planWord (1 + 2 * index)
def effectTag (index : Nat) : Nat := effectMetadata index % (2 ^ 32)

def stateMagicWord : Nat :=
  decodeLE [0x44, 0x43, 0x45, 0x53, 1, 0, 0, 0] -- `DCES`, V1, reserved

theorem exact_offsets :
    authorityOffset = 8 ∧
    projectionOffset = 10344 ∧
    projectionDataOffset = 10432 ∧
    instructionLengthOffset = 20784 ∧
    instructionOffset = 20792 ∧
    programIdOffset = 20912 := by
  native_decide

theorem exact_frames :
    accountFrameWord authorityRole = 511 ∧
    accountFrameWord projectionRole = 65791 := by
  native_decide

theorem exact_instruction_shape :
    instructionBytes = 120 ∧
    planWord 0 = 7702055306052 ∧
    effectTag 0 = 0 ∧
    effectTag 1 = 256 ∧
    effectTag 2 = 65537 ∧
    effectTag 3 = 65794 ∧
    effectTag 4 = 131329 ∧
    effectTag 5 = 131074 ∧
    effectTag 6 = 131586 := by
  native_decide

end DClutch.SbfProfile
