import DClutchSemantics.Direct

/-!
# Canonical Effect IR wire encoding

The V1 encoding has one eight-byte header followed by fixed sixteen-byte effect
records. It contains no width-specialized branch and no Rust-owned schema.
-/

namespace DClutch.Codec

open DClutch

def magic : List UInt8 := [0x44, 0x43, 0x45, 0x46] -- `DCEF`
def version : UInt8 := 1
def headerBytes : Nat := 8
def effectBytes : Nat := 16
def maxEffects : Nat := 7

def opcode : Effect → UInt8
  | .set .. => 0
  | .debit .. => 1
  | .credit .. => 2

def partyTag : Party → UInt8
  | .seller => 0
  | .buyer => 1
  | .venue => 2

def resourceTag : Resource → UInt8
  | .replayNonce => 0
  | .outcomeClaim _ => 1
  | .collateral => 2

def outcomeCoordinate : Resource → Nat
  | .outcomeClaim outcome => outcome
  | _ => 0

def effectCell : Effect → Cell
  | .set cell _ => cell
  | .debit cell _ => cell
  | .credit cell _ => cell

def effectAmount : Effect → Nat
  | .set _ value => value
  | .debit _ amount => amount
  | .credit _ amount => amount

/-- One little-endian byte. Physical profile checks ensure discarded high bits
are zero before an artifact is admitted. -/
def littleEndianByte (value byteIndex : Nat) : UInt8 :=
  UInt8.ofNat ((value / (256 ^ byteIndex)) % 256)

def encodeLE (width value : Nat) : List UInt8 :=
  (List.range width).map (littleEndianByte value)

theorem encodeLE_length (width value : Nat) : (encodeLE width value).length = width := by
  simp [encodeLE]

/-- Fixed sixteen-byte effect record.

Bytes: opcode, party, resource, zero; `u32` outcome; `u64` value.
-/
def encodeEffect (effect : Effect) : List UInt8 :=
  let cell := effectCell effect
  [opcode effect, partyTag cell.party, resourceTag cell.resource, 0] ++
    encodeLE 4 (outcomeCoordinate cell.resource) ++
    encodeLE 8 (effectAmount effect)

theorem encodeEffect_length (effect : Effect) : (encodeEffect effect).length = effectBytes := by
  simp [encodeEffect, effectBytes, encodeLE_length]

def encodeHeader (count : Nat) : List UInt8 :=
  magic ++ [version, UInt8.ofNat count, 0, 0]

theorem encodeHeader_length (count : Nat) : (encodeHeader count).length = headerBytes := by
  simp [encodeHeader, magic, headerBytes]

/-- Canonical first-order bytes. An artifact admission layer separately refuses
plans exceeding `maxEffects`, `u32` outcomes, or `u64` values. -/
def encodePlan (plan : EffectPlan) : List UInt8 :=
  encodeHeader plan.effects.length ++ plan.effects.flatMap encodeEffect

private theorem flatMap_effect_length : ∀ effects : List Effect,
    (effects.flatMap encodeEffect).length = effects.length * effectBytes
  | [] => by simp
  | effect :: rest => by
      simp [encodeEffect_length, flatMap_effect_length rest, effectBytes]
      omega

theorem encodePlan_length (plan : EffectPlan) :
    (encodePlan plan).length = headerBytes + plan.effects.length * effectBytes := by
  unfold encodePlan
  rw [List.length_append, encodeHeader_length, flatMap_effect_length]

def hexDigit (value : Nat) : Char :=
  match value with
  | 0 => '0' | 1 => '1' | 2 => '2' | 3 => '3'
  | 4 => '4' | 5 => '5' | 6 => '6' | 7 => '7'
  | 8 => '8' | 9 => '9' | 10 => 'a' | 11 => 'b'
  | 12 => 'c' | 13 => 'd' | 14 => 'e' | _ => 'f'

def byteHex (byte : UInt8) : String :=
  let value := byte.toNat
  String.ofList [hexDigit (value / 16), hexDigit (value % 16)]

def hex (bytes : List UInt8) : String :=
  String.join (bytes.map byteHex)

end DClutch.Codec
