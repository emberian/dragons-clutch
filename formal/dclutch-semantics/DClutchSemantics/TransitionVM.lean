import DClutchSemantics.Codec

/-!
# Canonical transition-checking bytecode

The successor does not grow one Rust admission function per action.  Lean emits
a small first-order program over authenticated scalar and identity registers.
An SBF adapter may supply physical `u64` values and 32-byte identities, but the
instruction sequence and its meaning live here.
-/

namespace DClutch.TransitionVM

/-- Abstract registers. Identity values support equality only; the physical
adapter refines them to exact 32-byte public keys. -/
structure State where
  scalars : Array Nat
  identities : Array Nat
  deriving DecidableEq, Repr

/-- Fixed transition instruction vocabulary. -/
inductive Op where
  | loadConst (destination value : Nat)
  | scalarEq (left right : Nat)
  | identityEq (left right : Nat)
  | identityNe (left right : Nat)
  | scalarLt (left right : Nat)
  | scalarLe (left right : Nat)
  | nonzero (source : Nat)
  | lifecycleAccepts (lifecycle maximum fill : Nat)
  | incrementInto (source destination : Nat)
  | mulDivExact (left right denominator destination : Nat)
  | mulDivFloor (left right denominator destination : Nat)
  | addLe (left right limit : Nat)
  | addFitsU64 (left right : Nat)
  | subInto (minuend subtrahend destination : Nat)
  | selectEq (left right ifEqual destination : Nat)
  | selectZero (source ifZero destination : Nat)
  deriving DecidableEq, Repr

def scalar (state : State) (index : Nat) : Option Nat := state.scalars[index]?
def identity (state : State) (index : Nat) : Option Nat := state.identities[index]?

def setScalar (state : State) (index value : Nat) : Option State :=
  if index < state.scalars.size then
    some { state with scalars := state.scalars.setIfInBounds index value }
  else none

def require (condition : Bool) (state : State) : Option State :=
  if condition then some state else none

def step (operation : Op) (state : State) : Option State := do
  match operation with
  | .loadConst destination value => setScalar state destination value
  | .scalarEq left right => require ((← scalar state left) = (← scalar state right)) state
  | .identityEq left right =>
      require ((← identity state left) = (← identity state right)) state
  | .identityNe left right =>
      require ((← identity state left) ≠ (← identity state right)) state
  | .scalarLt left right => require ((← scalar state left) < (← scalar state right)) state
  | .scalarLe left right => require ((← scalar state left) ≤ (← scalar state right)) state
  | .nonzero source => require ((← scalar state source) ≠ 0) state
  | .lifecycleAccepts lifecycle maximum fill =>
      let lifecycle ← scalar state lifecycle
      let maximum ← scalar state maximum
      let fill ← scalar state fill
      match lifecycle with
      | 0 => require (fill = maximum) state
      | 1 => require (fill ≤ maximum) state
      | 2 => require (fill ≤ maximum) state
      | _ => none
  | .incrementInto source destination =>
      let value ← scalar state source
      let next := value + 1
      if next < DClutch.Direct.u64Limit then setScalar state destination next else none
  | .mulDivExact left right denominator destination =>
      let numerator := (← scalar state left) * (← scalar state right)
      let denominator ← scalar state denominator
      if denominator = 0 || numerator % denominator ≠ 0 then none
      else
        let quotient := numerator / denominator
        if quotient < DClutch.Direct.u64Limit then setScalar state destination quotient else none
  | .mulDivFloor left right denominator destination =>
      let numerator := (← scalar state left) * (← scalar state right)
      let denominator ← scalar state denominator
      if denominator = 0 then none
      else
        let quotient := numerator / denominator
        if quotient < DClutch.Direct.u64Limit then setScalar state destination quotient else none
  | .addLe left right limit =>
      require ((← scalar state left) + (← scalar state right) ≤ (← scalar state limit)) state
  | .addFitsU64 left right =>
      require ((← scalar state left) + (← scalar state right) < DClutch.Direct.u64Limit) state
  | .subInto minuend subtrahend destination =>
      let left ← scalar state minuend
      let right ← scalar state subtrahend
      if right ≤ left then setScalar state destination (left - right) else none
  | .selectEq left right ifEqual destination =>
      let leftValue ← scalar state left
      let rightValue ← scalar state right
      let selected ← scalar state ifEqual
      let _ ← scalar state destination
      if leftValue = rightValue then setScalar state destination selected else some state
  | .selectZero source ifZero destination =>
      let sourceValue ← scalar state source
      let selected ← scalar state ifZero
      let _ ← scalar state destination
      if sourceValue = 0 then setScalar state destination selected else some state

def run : List Op → State → Option State
  | [], state => some state
  | operation :: rest, state => (step operation state).bind (run rest)

theorem run_append (first second : List Op) (state : State) :
    run (first ++ second) state = (run first state).bind (run second) := by
  induction first generalizing state with
  | nil => rfl
  | cons operation rest induction =>
      simp only [List.cons_append, run]
      cases stepped : step operation state <;> simp [induction]

theorem run_append_fn (first second : List Op) :
    run (first ++ second) = fun state => (run first state).bind (run second) := by
  funext state
  exact run_append first second state

namespace Codec

def magic : List UInt8 := [0x44, 0x43, 0x54, 0x56] -- `DCTV`
def version : UInt8 := 1
def headerBytes : Nat := 8
def instructionBytes : Nat := 16

def opcode : Op → UInt8
  | .loadConst .. => 0
  | .scalarEq .. => 1
  | .identityEq .. => 2
  | .identityNe .. => 3
  | .scalarLt .. => 4
  | .scalarLe .. => 5
  | .nonzero .. => 6
  | .lifecycleAccepts .. => 7
  | .incrementInto .. => 8
  | .mulDivExact .. => 9
  | .mulDivFloor .. => 10
  | .addLe .. => 11
  | .addFitsU64 .. => 12
  | .subInto .. => 13
  | .selectEq .. => 14
  | .selectZero .. => 15

def arguments : Op → List Nat
  | .loadConst destination _ => [destination]
  | .scalarEq left right => [left, right]
  | .identityEq left right => [left, right]
  | .identityNe left right => [left, right]
  | .scalarLt left right => [left, right]
  | .scalarLe left right => [left, right]
  | .nonzero source => [source]
  | .lifecycleAccepts lifecycle maximum fill => [lifecycle, maximum, fill]
  | .incrementInto source destination => [source, destination]
  | .mulDivExact left right denominator destination =>
      [left, right, denominator, destination]
  | .mulDivFloor left right denominator destination =>
      [left, right, denominator, destination]
  | .addLe left right limit => [left, right, limit]
  | .addFitsU64 left right => [left, right]
  | .subInto minuend subtrahend destination => [minuend, subtrahend, destination]
  | .selectEq left right ifEqual destination => [left, right, ifEqual, destination]
  | .selectZero source ifZero destination => [source, ifZero, destination]

def immediate : Op → Nat
  | .loadConst _ value => value
  | _ => 0

def argumentByte (operation : Op) (index : Nat) : UInt8 :=
  UInt8.ofNat ((arguments operation)[index]?.getD 0)

def encodeInstruction (operation : Op) : List UInt8 :=
  [opcode operation,
    argumentByte operation 0,
    argumentByte operation 1,
    argumentByte operation 2,
    argumentByte operation 3,
    0, 0, 0] ++ DClutch.Codec.encodeLE 8 (immediate operation)

theorem encode_instruction_length (operation : Op) :
    (encodeInstruction operation).length = instructionBytes := by
  simp [encodeInstruction, instructionBytes, DClutch.Codec.encodeLE_length]

def encodeHeader (count : Nat) : List UInt8 :=
  magic ++ [version, UInt8.ofNat count, 0, 0]

def encodeProgram (program : List Op) : List UInt8 :=
  encodeHeader program.length ++ program.flatMap encodeInstruction

private theorem flatMap_instruction_length : ∀ program : List Op,
    (program.flatMap encodeInstruction).length = program.length * instructionBytes
  | [] => by simp
  | operation :: rest => by
      simp [encode_instruction_length, flatMap_instruction_length rest, instructionBytes]
      omega

theorem encode_program_length (program : List Op) :
    (encodeProgram program).length = headerBytes + program.length * instructionBytes := by
  unfold encodeProgram
  rw [List.length_append, flatMap_instruction_length]
  simp [encodeHeader, magic, headerBytes]

end Codec

end DClutch.TransitionVM
