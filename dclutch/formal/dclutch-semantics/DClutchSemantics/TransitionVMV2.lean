import DClutchSemantics.TransitionVM
import Std.Tactic

/-!
# Runtime-width transition VM V2

V1's Lean semantics already used runtime arrays, but its physical Rust profile
compiled 64 scalar and 16 identity registers into the executor. V2 makes each
program declare its exact bank widths and uses `u16` instruction operands. The
`u16` counts are a physical representation boundary, not a family, outcome, or
semantic-width constant.
-/

namespace DClutch.TransitionVMV2

abbrev State := DClutch.TransitionVM.State

def u16Limit : Nat := 2 ^ 16
def u64Limit : Nat := DClutch.Direct.u64Limit

/-- Runtime-width V2 vocabulary. The first sixteen operations retain V1
meaning. The final six remove family-specific arithmetic/copy helpers. -/
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
  | checkedAddInto (left right destination : Nat)
  | checkedMulInto (left right destination : Nat)
  | minInto (left right destination : Nat)
  | maxInto (left right destination : Nat)
  | copyScalar (source destination : Nat)
  | copyIdentity (source destination : Nat)
  deriving DecidableEq, Repr

/-- V2 program with explicit physical register-bank requirements. -/
structure Program where
  scalarWidth : Nat
  identityWidth : Nat
  operations : List Op
  deriving DecidableEq, Repr

def scalar (state : State) (index : Nat) : Option Nat := state.scalars[index]?
def identity (state : State) (index : Nat) : Option Nat := state.identities[index]?

def setScalar (state : State) (index value : Nat) : Option State :=
  if index < state.scalars.size then
    some { state with scalars := state.scalars.setIfInBounds index value }
  else none

def setIdentity (state : State) (index value : Nat) : Option State :=
  if index < state.identities.size then
    some { state with identities := state.identities.setIfInBounds index value }
  else none

def require (condition : Bool) (state : State) : Option State :=
  if condition then some state else none

def step (operation : Op) (state : State) : Option State := do
  match operation with
  | .loadConst destination value =>
      if value < u64Limit then setScalar state destination value else none
  | .scalarEq left right => require ((← scalar state left) = (← scalar state right)) state
  | .identityEq left right => require ((← identity state left) = (← identity state right)) state
  | .identityNe left right => require ((← identity state left) ≠ (← identity state right)) state
  | .scalarLt left right => require ((← scalar state left) < (← scalar state right)) state
  | .scalarLe left right => require ((← scalar state left) ≤ (← scalar state right)) state
  | .nonzero source => require ((← scalar state source) ≠ 0) state
  | .lifecycleAccepts lifecycle maximum fill =>
      let lifecycle ← scalar state lifecycle
      let maximum ← scalar state maximum
      let fill ← scalar state fill
      match lifecycle with
      | 0 => require (fill = maximum) state
      | 1 | 2 => require (fill ≤ maximum) state
      | _ => none
  | .incrementInto source destination =>
      let next := (← scalar state source) + 1
      if next < u64Limit then setScalar state destination next else none
  | .mulDivExact left right denominator destination =>
      let numerator := (← scalar state left) * (← scalar state right)
      let denominator ← scalar state denominator
      if denominator = 0 || numerator % denominator ≠ 0 then none
      else
        let quotient := numerator / denominator
        if quotient < u64Limit then setScalar state destination quotient else none
  | .mulDivFloor left right denominator destination =>
      let numerator := (← scalar state left) * (← scalar state right)
      let denominator ← scalar state denominator
      if denominator = 0 then none
      else
        let quotient := numerator / denominator
        if quotient < u64Limit then setScalar state destination quotient else none
  | .addLe left right limit =>
      require ((← scalar state left) + (← scalar state right) ≤ (← scalar state limit)) state
  | .addFitsU64 left right =>
      require ((← scalar state left) + (← scalar state right) < u64Limit) state
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
  | .checkedAddInto left right destination =>
      let value := (← scalar state left) + (← scalar state right)
      if value < u64Limit then setScalar state destination value else none
  | .checkedMulInto left right destination =>
      let value := (← scalar state left) * (← scalar state right)
      if value < u64Limit then setScalar state destination value else none
  | .minInto left right destination =>
      setScalar state destination (min (← scalar state left) (← scalar state right))
  | .maxInto left right destination =>
      setScalar state destination (max (← scalar state left) (← scalar state right))
  | .copyScalar source destination => setScalar state destination (← scalar state source)
  | .copyIdentity source destination => setIdentity state destination (← identity state source)

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

def scalarIndices : Op → List Nat
  | .loadConst destination _ => [destination]
  | .scalarEq left right | .scalarLt left right | .scalarLe left right |
      .addFitsU64 left right | .incrementInto left right |
      .copyScalar left right => [left, right]
  | .identityEq .. | .identityNe .. | .copyIdentity .. => []
  | .nonzero source => [source]
  | .lifecycleAccepts lifecycle maximum fill => [lifecycle, maximum, fill]
  | .mulDivExact left right denominator destination |
      .mulDivFloor left right denominator destination |
      .selectEq left right denominator destination => [left, right, denominator, destination]
  | .addLe left right limit | .subInto left right limit |
      .selectZero left right limit | .checkedAddInto left right limit |
      .checkedMulInto left right limit | .minInto left right limit |
      .maxInto left right limit => [left, right, limit]

def identityIndices : Op → List Nat
  | .identityEq left right | .identityNe left right |
      .copyIdentity left right => [left, right]
  | _ => []

def Op.indicesWithin (operation : Op) (scalarWidth identityWidth : Nat) : Bool :=
  (scalarIndices operation).all (· < scalarWidth) &&
    (identityIndices operation).all (· < identityWidth)

def Op.immediateWithin : Op → Bool
  | .loadConst _ value => value < u64Limit
  | _ => true

/-- Exact physical encodability. No smaller fixed bank or instruction maximum
appears here; `u16` is the named representation bound. -/
def Program.wellFormed (program : Program) : Bool :=
  program.operations ≠ [] &&
    program.operations.length < u16Limit &&
    program.scalarWidth < u16Limit &&
    program.identityWidth < u16Limit &&
    (program.scalarWidth ≠ 0 || program.identityWidth ≠ 0) &&
    program.operations.all fun operation =>
      operation.indicesWithin program.scalarWidth program.identityWidth &&
        operation.immediateWithin

def Program.stateMatches (program : Program) (state : State) : Bool :=
  state.scalars.size = program.scalarWidth &&
    state.identities.size = program.identityWidth

def Program.execute (program : Program) (state : State) : Option State := do
  if !program.wellFormed || !program.stateMatches state then none
  else run program.operations state

namespace Codec

def magic : List UInt8 := [0x44, 0x43, 0x54, 0x56]
def version : UInt8 := 2
def headerBytes : Nat := 16
def instructionBytes : Nat := 24

def versionOffset : Nat := 4
def flagsOffset : Nat := 5
def instructionCountOffset : Nat := 6
def scalarCountOffset : Nat := 8
def identityCountOffset : Nat := 10
def headerReservedOffset : Nat := 12
def opcodeOffset : Nat := 0
def instructionReservedByteOffset : Nat := 1
def argumentAOffset : Nat := 2
def argumentBOffset : Nat := 4
def argumentCOffset : Nat := 6
def argumentDOffset : Nat := 8
def instructionReservedOffset : Nat := 10
def immediateOffset : Nat := 16

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
  | .checkedAddInto .. => 16
  | .checkedMulInto .. => 17
  | .minInto .. => 18
  | .maxInto .. => 19
  | .copyScalar .. => 20
  | .copyIdentity .. => 21

def arguments : Op → List Nat
  | .loadConst destination _ => [destination]
  | .scalarEq left right | .identityEq left right | .identityNe left right |
      .scalarLt left right | .scalarLe left right | .addFitsU64 left right |
      .incrementInto left right | .copyScalar left right |
      .copyIdentity left right => [left, right]
  | .nonzero source => [source]
  | .lifecycleAccepts lifecycle maximum fill => [lifecycle, maximum, fill]
  | .mulDivExact left right denominator destination |
      .mulDivFloor left right denominator destination |
      .selectEq left right denominator destination => [left, right, denominator, destination]
  | .addLe left right limit | .subInto left right limit |
      .selectZero left right limit | .checkedAddInto left right limit |
      .checkedMulInto left right limit | .minInto left right limit |
      .maxInto left right limit => [left, right, limit]

def immediate : Op → Nat
  | .loadConst _ value => value
  | _ => 0

def argument (operation : Op) (index : Nat) : List UInt8 :=
  DClutch.Codec.encodeLE 2 ((arguments operation)[index]?.getD 0)

def encodeInstruction (operation : Op) : List UInt8 :=
  [opcode operation, 0] ++
    argument operation 0 ++ argument operation 1 ++
    argument operation 2 ++ argument operation 3 ++
    [0, 0, 0, 0, 0, 0] ++ DClutch.Codec.encodeLE 8 (immediate operation)

theorem encode_instruction_length (operation : Op) :
    (encodeInstruction operation).length = instructionBytes := by
  simp [encodeInstruction, argument, DClutch.Codec.encodeLE_length, instructionBytes]

def encodeHeader (program : Program) : List UInt8 :=
  magic ++ [version, 0] ++
    DClutch.Codec.encodeLE 2 program.operations.length ++
    DClutch.Codec.encodeLE 2 program.scalarWidth ++
    DClutch.Codec.encodeLE 2 program.identityWidth ++ [0, 0, 0, 0]

theorem encode_header_length (program : Program) :
    (encodeHeader program).length = headerBytes := by
  simp [encodeHeader, magic, DClutch.Codec.encodeLE_length, headerBytes]

def encodeProgram (program : Program) : List UInt8 :=
  encodeHeader program ++ program.operations.flatMap encodeInstruction

private theorem flatMap_instruction_length : ∀ operations : List Op,
    (operations.flatMap encodeInstruction).length = operations.length * instructionBytes
  | [] => by simp
  | operation :: rest => by
      simp [encode_instruction_length, flatMap_instruction_length rest, instructionBytes]
      omega

theorem encode_program_length (program : Program) :
    (encodeProgram program).length =
      headerBytes + program.operations.length * instructionBytes := by
  unfold encodeProgram
  rw [List.length_append, encode_header_length, flatMap_instruction_length]

end Codec

/-- A witness that widths beyond V1's compiled arrays are ordinary V2
program data. -/
def wideExample : Program := {
  scalarWidth := 300
  identityWidth := 257
  operations := [
    .loadConst 299 41,
    .incrementInto 299 298,
    .copyIdentity 256 255
  ]
}

theorem wide_example_is_well_formed : wideExample.wellFormed = true := by native_decide

theorem wide_example_encoded_width :
    (Codec.encodeProgram wideExample).length = 88 := by native_decide

end DClutch.TransitionVMV2
