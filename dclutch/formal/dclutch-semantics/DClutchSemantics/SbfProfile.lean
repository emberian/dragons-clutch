/-!
# Common exact-account Solana ABI-v1 layout

This file owns only the shared loader-v1 layout arithmetic used by specialized
proof profiles. It does not model the loader implementation: the formula is an
explicit adapter assumption checked against the pinned Agave ABI v1 layout when
release evidence is produced.
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

/-- Interpret at most eight little-endian bytes as a natural number. -/
def decodeLE : List UInt8 → Nat
  | [] => 0
  | byte :: rest => byte.toNat + 256 * decodeLE rest

def wordAt (bytes : List UInt8) (offset : Nat) : Nat :=
  decodeLE ((bytes.drop offset).take 8)

end DClutch.SbfProfile
