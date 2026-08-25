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

def encodeLE : Nat → Nat → List UInt8
  | 0, _ => []
  | width + 1, value => UInt8.ofNat (value % 256) :: encodeLE width (value / 256)

theorem encodeLE_length (width value : Nat) : (encodeLE width value).length = width := by
  induction width generalizing value <;> simp [encodeLE, *]

/-- Interpret every supplied byte as one little-endian natural number. Width
and representability checks belong to the surrounding canonical parser. -/
def decodeLE : List UInt8 → Nat
  | [] => 0
  | byte :: rest => byte.toNat + 256 * decodeLE rest

theorem decodeLE_encodeLE
    (width value : Nat) (fits : value < 256 ^ width) :
    decodeLE (encodeLE width value) = value := by
  induction width generalizing value with
  | zero =>
      simp at fits
      subst value
      rfl
  | succ width induction =>
      have quotientFits : value / 256 < 256 ^ width := by
        apply (Nat.div_lt_iff_lt_mul (by decide : 0 < 256)).2
        simpa [Nat.pow_succ, Nat.mul_comm] using fits
      simp only [encodeLE, decodeLE, UInt8.toNat_ofNat', Nat.reducePow]
      rw [Nat.mod_mod_of_dvd value (by decide : 256 ∣ 256),
        induction (value / 256) quotientFits]
      exact Nat.mod_add_div value 256

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

/-- Exact representability conditions for the canonical V1 record widths. -/
def EffectEncodable (effect : Effect) : Prop :=
  outcomeCoordinate (effectCell effect).resource < 256 ^ 4 ∧
    effectAmount effect < 256 ^ 8

def decodeParty : UInt8 → Option Party
  | 0 => some .seller
  | 1 => some .buyer
  | 2 => some .venue
  | _ => none

def decodeResource (tag : UInt8) (outcome : Nat) : Option Resource :=
  match tag with
  | 0 => if outcome = 0 then some .replayNonce else none
  | 1 => some (.outcomeClaim outcome)
  | 2 => if outcome = 0 then some .collateral else none
  | _ => none

def decodeOperation (tag : UInt8) (cell : Cell) (amount : Nat) : Option Effect :=
  match tag with
  | 0 => some (.set cell amount)
  | 1 => some (.debit cell amount)
  | 2 => some (.credit cell amount)
  | _ => none

/-- Hostile decoder for one exact V1 effect record. -/
def decodeEffect (bytes : List UInt8) : Option Effect := do
  if bytes.length != effectBytes then none else
  let operation ← bytes[0]?
  let partyTag ← bytes[1]?
  let resourceTag ← bytes[2]?
  let reserved ← bytes[3]?
  if reserved != 0 then none else
  let outcome := decodeLE ((bytes.drop 4).take 4)
  let amount := decodeLE (bytes.drop 8)
  let party ← decodeParty partyTag
  let resource ← decodeResource resourceTag outcome
  decodeOperation operation { party, resource } amount

def decodeEffects : Nat → List UInt8 → Option (List Effect)
  | 0, bytes => if bytes.isEmpty then some [] else none
  | count + 1, bytes => do
      let effect ← decodeEffect (bytes.take effectBytes)
      let rest ← decodeEffects count (bytes.drop effectBytes)
      some (effect :: rest)

/-- Hostile decoder for one exact canonical V1 plan. -/
def decodePlan : List UInt8 → Option EffectPlan
  | 0x44 :: 0x43 :: 0x45 :: 0x46 :: wireVersion :: count :: reservedA :: reservedB :: records => do
      if wireVersion != version || reservedA != 0 || reservedB != 0 then none else
      let count := count.toNat
      if count > maxEffects || records.length != count * effectBytes then none else
      let effects ← decodeEffects count records
      some { effects }
  | _ => none

theorem decodeEffect_encodeEffect
    (effect : Effect) (encodable : EffectEncodable effect) :
    decodeEffect (encodeEffect effect) = some effect := by
  rcases encodable with ⟨outcomeFits, amountFits⟩
  cases effect with
  | set cell amount | debit cell amount | credit cell amount =>
      cases cell with
      | mk party resource =>
          have outcomeDecoded := decodeLE_encodeLE 4
            (outcomeCoordinate resource) outcomeFits
          have amountDecoded := decodeLE_encodeLE 8 amount amountFits
          cases party <;> cases resource
          all_goals
            simp only [outcomeCoordinate] at outcomeDecoded
            simp [decodeEffect, encodeEffect, effectCell, effectAmount,
              outcomeCoordinate, opcode, partyTag, resourceTag, effectBytes,
              decodeParty, decodeResource, decodeOperation, encodeLE_length,
              outcomeDecoded, amountDecoded]

private theorem decodeEffects_encodeEffects
    (effects : List Effect)
    (encodable : ∀ effect ∈ effects, EffectEncodable effect) :
    decodeEffects effects.length (effects.flatMap encodeEffect) = some effects := by
  induction effects with
  | nil => simp [decodeEffects]
  | cons effect rest induction =>
      have headEncodable := encodable effect (by simp)
      have restEncodable : ∀ candidate ∈ rest, EffectEncodable candidate := by
        intro candidate member
        exact encodable candidate (by simp [member])
      simp [decodeEffects, encodeEffect_length,
        decodeEffect_encodeEffect effect headEncodable,
        induction restEncodable]

theorem decodePlan_encodePlan
    (plan : EffectPlan)
    (countFits : plan.effects.length ≤ maxEffects)
    (encodable : ∀ effect ∈ plan.effects, EffectEncodable effect) :
    decodePlan (encodePlan plan) = some plan := by
  have countLt : plan.effects.length < 256 :=
    Nat.lt_of_le_of_lt countFits (by decide)
  have countByte : (UInt8.ofNat plan.effects.length).toNat = plan.effects.length := by
    simp [UInt8.toNat_ofNat', Nat.mod_eq_of_lt countLt]
  have recordsLength :
      (plan.effects.flatMap encodeEffect).length =
        plan.effects.length * effectBytes :=
    flatMap_effect_length plan.effects
  simp [encodePlan, encodeHeader, magic, decodePlan, version, countByte,
    countFits, recordsLength, decodeEffects_encodeEffects plan.effects encodable]

theorem hostile_plan_decodings_refuse :
    decodePlan [] = none ∧
      decodePlan [0x44, 0x43, 0x45, 0x46, 1, 8, 0, 0] = none ∧
      decodePlan [0x44, 0x43, 0x45, 0x46, 1, 0, 1, 0] = none ∧
      decodePlan [
        0x44, 0x43, 0x45, 0x46, 1, 1, 0, 0,
        0, 0, 0, 0, 1, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0
      ] = none := by
  native_decide

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
