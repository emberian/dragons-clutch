import DClutchSemantics.TransitionVMV2
import Std.Tactic

/-!
# Runtime-tail transition VM V3

V2 gave every program its exact common bank widths. V3 adds the Product-owned
tail: a program is one fixed prelude, one item body folded once per
authenticated tail coordinate, and one fixed epilogue. Register operands carry
a physical *space* — common, or the per-item stride — and the item space is
addressable only from the item body. The `u16` operand and count bounds are a
physical representation boundary, not a family, outcome, or semantic width.

This module owns the V3 vocabulary, its fold semantics, and its exact byte
encoding. Programs are authored here and emitted; no Rust executor is entitled
to add an admission relation that does not appear in an authored program.
-/

namespace DClutch.TransitionVMV3

abbrev State := DClutch.TransitionVM.State

def u16Limit : Nat := 2 ^ 16
def u64Limit : Nat := DClutch.Direct.u64Limit

/-- Physical register space. `item` coordinates are resolved against the fold's
current tail ordinal and exist only inside the item body. -/
inductive Space where
  | common
  | item
  deriving DecidableEq, Repr

/-- One physical register coordinate: a space and an index within that space. -/
structure Reg where
  space : Space
  index : Nat
  deriving DecidableEq, Repr

/-- A common-bank coordinate. -/
def common (index : Nat) : Reg := ⟨.common, index⟩

/-- A per-item coordinate inside the Product-owned tail stride. -/
def item (index : Nat) : Reg := ⟨.item, index⟩

/-- Runtime-tail V3 vocabulary. Operand meaning is V2's; only the operand type
widened from a common index to a spaced coordinate. -/
inductive Op where
  | loadConst (destination : Reg) (value : Nat)
  | scalarEq (left right : Reg)
  | identityEq (left right : Reg)
  | identityNe (left right : Reg)
  | scalarLt (left right : Reg)
  | scalarLe (left right : Reg)
  | nonzero (source : Reg)
  | lifecycleAccepts (lifecycle maximum fill : Reg)
  | incrementInto (source destination : Reg)
  | mulDivExact (left right denominator destination : Reg)
  | mulDivFloor (left right denominator destination : Reg)
  | addLe (left right limit : Reg)
  | addFitsU64 (left right : Reg)
  | subInto (minuend subtrahend destination : Reg)
  | selectEq (left right ifEqual destination : Reg)
  | selectZero (source ifZero destination : Reg)
  | checkedAddInto (left right destination : Reg)
  | checkedMulInto (left right destination : Reg)
  | minInto (left right destination : Reg)
  | maxInto (left right destination : Reg)
  | copyScalar (source destination : Reg)
  | copyIdentity (source destination : Reg)
  deriving DecidableEq, Repr

/-- A V3 program: three operation sections and the four affine bank widths. -/
structure Program where
  commonScalars : Nat
  itemScalarStride : Nat
  commonIdentities : Nat
  itemIdentityStride : Nat
  «prelude» : List Op
  itemBody : List Op
  epilogue : List Op
  deriving DecidableEq, Repr

namespace Program

/-- Every operation in canonical execution and encoding order. -/
def operations (program : Program) : List Op :=
  program.prelude ++ program.itemBody ++ program.epilogue

/-- Exact checked affine bank width for one tail count. -/
def scalarWidth (program : Program) (tailCount : Nat) : Nat :=
  program.commonScalars + tailCount * program.itemScalarStride

/-- Exact checked affine identity-bank width for one tail count. -/
def identityWidth (program : Program) (tailCount : Nat) : Nat :=
  program.commonIdentities + tailCount * program.itemIdentityStride

end Program

/-- Resolve one coordinate against a bank's common width and item stride. An
item coordinate outside the item body has no ordinal and does not resolve. -/
def resolve (commonWidth stride : Nat) (ordinal : Option Nat) : Reg → Option Nat
  | ⟨.common, index⟩ => if index < commonWidth then some index else none
  | ⟨.item, index⟩ =>
      if index < stride then
        match ordinal with
        | some ordinal => some (ordinal * stride + commonWidth + index)
        | none => none
      else none

def scalarIndex (program : Program) (ordinal : Option Nat) (register : Reg) : Option Nat :=
  resolve program.commonScalars program.itemScalarStride ordinal register

def identityIndex (program : Program) (ordinal : Option Nat) (register : Reg) : Option Nat :=
  resolve program.commonIdentities program.itemIdentityStride ordinal register

def readScalar (program : Program) (ordinal : Option Nat) (state : State)
    (register : Reg) : Option Nat := do
  state.scalars[← scalarIndex program ordinal register]?

def readIdentity (program : Program) (ordinal : Option Nat) (state : State)
    (register : Reg) : Option Nat := do
  state.identities[← identityIndex program ordinal register]?

def writeScalar (program : Program) (ordinal : Option Nat) (state : State)
    (register : Reg) (value : Nat) : Option State := do
  let index ← scalarIndex program ordinal register
  if index < state.scalars.size then
    some { state with scalars := state.scalars.setIfInBounds index value }
  else none

def writeIdentity (program : Program) (ordinal : Option Nat) (state : State)
    (register : Reg) (value : Nat) : Option State := do
  let index ← identityIndex program ordinal register
  if index < state.identities.size then
    some { state with identities := state.identities.setIfInBounds index value }
  else none

def require (condition : Bool) (state : State) : Option State :=
  if condition then some state else none

/-- One operation against one state at one tail ordinal. -/
def step (program : Program) (ordinal : Option Nat) (operation : Op)
    (state : State) : Option State := do
  let scalar := readScalar program ordinal state
  let ident := readIdentity program ordinal state
  let put := writeScalar program ordinal state
  match operation with
  | .loadConst destination value =>
      if value < u64Limit then put destination value else none
  | .scalarEq left right => require ((← scalar left) = (← scalar right)) state
  | .identityEq left right => require ((← ident left) = (← ident right)) state
  | .identityNe left right => require ((← ident left) ≠ (← ident right)) state
  | .scalarLt left right => require ((← scalar left) < (← scalar right)) state
  | .scalarLe left right => require ((← scalar left) ≤ (← scalar right)) state
  | .nonzero source => require ((← scalar source) ≠ 0) state
  | .lifecycleAccepts lifecycle maximum fill =>
      let lifecycle ← scalar lifecycle
      let maximum ← scalar maximum
      let fill ← scalar fill
      match lifecycle with
      | 0 => require (fill = maximum) state
      | 1 | 2 => require (fill ≤ maximum) state
      | _ => none
  | .incrementInto source destination =>
      let next := (← scalar source) + 1
      if next < u64Limit then put destination next else none
  | .mulDivExact left right denominator destination =>
      let numerator := (← scalar left) * (← scalar right)
      let denominator ← scalar denominator
      if denominator = 0 || numerator % denominator ≠ 0 then none
      else
        let quotient := numerator / denominator
        if quotient < u64Limit then put destination quotient else none
  | .mulDivFloor left right denominator destination =>
      let numerator := (← scalar left) * (← scalar right)
      let denominator ← scalar denominator
      if denominator = 0 then none
      else
        let quotient := numerator / denominator
        if quotient < u64Limit then put destination quotient else none
  | .addLe left right limit =>
      require ((← scalar left) + (← scalar right) ≤ (← scalar limit)) state
  | .addFitsU64 left right =>
      require ((← scalar left) + (← scalar right) < u64Limit) state
  | .subInto minuend subtrahend destination =>
      let left ← scalar minuend
      let right ← scalar subtrahend
      if right ≤ left then put destination (left - right) else none
  | .selectEq left right ifEqual destination =>
      let leftValue ← scalar left
      let rightValue ← scalar right
      let selected ← scalar ifEqual
      let _ ← scalar destination
      if leftValue = rightValue then put destination selected else some state
  | .selectZero source ifZero destination =>
      let sourceValue ← scalar source
      let selected ← scalar ifZero
      let _ ← scalar destination
      if sourceValue = 0 then put destination selected else some state
  | .checkedAddInto left right destination =>
      let value := (← scalar left) + (← scalar right)
      if value < u64Limit then put destination value else none
  | .checkedMulInto left right destination =>
      let value := (← scalar left) * (← scalar right)
      if value < u64Limit then put destination value else none
  | .minInto left right destination =>
      put destination (min (← scalar left) (← scalar right))
  | .maxInto left right destination =>
      put destination (max (← scalar left) (← scalar right))
  | .copyScalar source destination => put destination (← scalar source)
  | .copyIdentity source destination =>
      writeIdentity program ordinal state destination (← ident source)

def run (program : Program) (ordinal : Option Nat) : List Op → State → Option State
  | [], state => some state
  | operation :: rest, state =>
      (step program ordinal operation state).bind (run program ordinal rest)

/-- Fold the item body over ordinals `tailCount - remaining … tailCount - 1`. -/
def runItems (program : Program) (tailCount : Nat) : Nat → State → Option State
  | 0, state => some state
  | remaining + 1, state =>
      (run program (some (tailCount - (remaining + 1))) program.itemBody state).bind
        (runItems program tailCount remaining)

/-- Prelude, then one item body per tail coordinate in ascending order, then
epilogue. -/
def fold (program : Program) (tailCount : Nat) (state : State) : Option State :=
  ((run program none program.prelude state).bind
      (runItems program tailCount tailCount)).bind
    (run program none program.epilogue)

namespace Op

/-- Scalar-space operands, in canonical slot order. -/
def scalarOperands : Op → List Reg
  | .loadConst destination _ => [destination]
  | .scalarEq left right | .scalarLt left right | .scalarLe left right
  | .addFitsU64 left right | .incrementInto left right
  | .copyScalar left right => [left, right]
  | .identityEq .. | .identityNe .. | .copyIdentity .. => []
  | .nonzero source => [source]
  | .lifecycleAccepts lifecycle maximum fill => [lifecycle, maximum, fill]
  | .mulDivExact left right denominator destination
  | .mulDivFloor left right denominator destination
  | .selectEq left right denominator destination => [left, right, denominator, destination]
  | .addLe left right limit | .subInto left right limit
  | .selectZero left right limit | .checkedAddInto left right limit
  | .checkedMulInto left right limit | .minInto left right limit
  | .maxInto left right limit => [left, right, limit]

/-- Identity-space operands, in canonical slot order. -/
def identityOperands : Op → List Reg
  | .identityEq left right | .identityNe left right
  | .copyIdentity left right => [left, right]
  | _ => []

/-- Every operand in canonical slot order. Exactly one of the two operand
lists is nonempty for each opcode, so this is the encoded slot sequence. -/
def operands (operation : Op) : List Reg :=
  match operation.identityOperands with
  | [] => operation.scalarOperands
  | identities => identities

def immediate : Op → Nat
  | .loadConst _ value => value
  | _ => 0

/-- Whether this operation addresses the item space at all. -/
def usesItemSpace (operation : Op) : Bool :=
  operation.operands.any fun register => register.space = .item

/-- Operand indices lie inside the space each one names. -/
def indicesWithin (operation : Op) (program : Program) : Bool :=
  operation.scalarOperands.all (fun register =>
      register.index <
        (match register.space with
          | .common => program.commonScalars
          | .item => program.itemScalarStride)) &&
    operation.identityOperands.all (fun register =>
      register.index <
        (match register.space with
          | .common => program.commonIdentities
          | .item => program.itemIdentityStride))

def immediateWithin (operation : Op) : Bool := operation.immediate < u64Limit

end Op

/-- Exact physical encodability, matching the hostile decoder's prevalidation:
canonical section counts, canonical bank widths, in-space operands, and item
coordinates confined to the item body. -/
def Program.wellFormed (program : Program) : Bool :=
  program.operations ≠ [] &&
    program.prelude.length < u16Limit &&
    program.itemBody.length < u16Limit &&
    program.epilogue.length < u16Limit &&
    program.operations.length < u16Limit &&
    program.commonScalars < u16Limit &&
    program.itemScalarStride < u16Limit &&
    program.commonIdentities < u16Limit &&
    program.itemIdentityStride < u16Limit &&
    (program.commonScalars ≠ 0 || program.itemScalarStride ≠ 0 ||
      program.commonIdentities ≠ 0 || program.itemIdentityStride ≠ 0) &&
    program.operations.all (fun operation =>
      operation.indicesWithin program && operation.immediateWithin) &&
    program.prelude.all (fun operation => !operation.usesItemSpace) &&
    program.epilogue.all (fun operation => !operation.usesItemSpace)

def Program.stateMatches (program : Program) (tailCount : Nat) (state : State) : Bool :=
  state.scalars.size = program.scalarWidth tailCount &&
    state.identities.size = program.identityWidth tailCount

/-- The authored meaning of a V3 program at one authenticated tail count. -/
def Program.execute (program : Program) (tailCount : Nat) (state : State) : Option State :=
  if !program.wellFormed || !program.stateMatches tailCount state then none
  else fold program tailCount state

namespace Codec

def magic : List UInt8 := [0x44, 0x43, 0x54, 0x56]
def version : UInt8 := 3
def headerBytes : Nat := 32
def instructionBytes : Nat := 24

def versionOffset : Nat := 4
def flagsOffset : Nat := 5
def preludeCountOffset : Nat := 6
def itemCountOffset : Nat := 8
def epilogueCountOffset : Nat := 10
def commonScalarCountOffset : Nat := 12
def itemScalarStrideOffset : Nat := 14
def commonIdentityCountOffset : Nat := 16
def itemIdentityStrideOffset : Nat := 18
def headerReservedOffset : Nat := 20
def opcodeOffset : Nat := 0
def spacesOffset : Nat := 1
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

/-- Low nibble space bitmap: bit `i` marks slot `i` as an item coordinate. -/
def spaces (operation : Op) : UInt8 :=
  let bit (slot : Nat) : Nat :=
    match (Op.operands operation)[slot]? with
    | some register => if register.space = .item then 2 ^ slot else 0
    | none => 0
  UInt8.ofNat (bit 0 + bit 1 + bit 2 + bit 3)

def argument (operation : Op) (slot : Nat) : List UInt8 :=
  DClutch.Codec.encodeLE 2 (((Op.operands operation)[slot]?.map Reg.index).getD 0)

def encodeInstruction (operation : Op) : List UInt8 :=
  [opcode operation, spaces operation] ++
    argument operation 0 ++ argument operation 1 ++
    argument operation 2 ++ argument operation 3 ++
    [0, 0, 0, 0, 0, 0] ++ DClutch.Codec.encodeLE 8 (Op.immediate operation)

theorem encode_instruction_length (operation : Op) :
    (encodeInstruction operation).length = instructionBytes := by
  simp [encodeInstruction, argument, DClutch.Codec.encodeLE_length, instructionBytes]

def encodeHeader (program : Program) : List UInt8 :=
  magic ++ [version, 0] ++
    DClutch.Codec.encodeLE 2 program.prelude.length ++
    DClutch.Codec.encodeLE 2 program.itemBody.length ++
    DClutch.Codec.encodeLE 2 program.epilogue.length ++
    DClutch.Codec.encodeLE 2 program.commonScalars ++
    DClutch.Codec.encodeLE 2 program.itemScalarStride ++
    DClutch.Codec.encodeLE 2 program.commonIdentities ++
    DClutch.Codec.encodeLE 2 program.itemIdentityStride ++
    List.replicate 12 0

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

/-- A witness that the tail is ordinary program data: two common scalars, a
one-wide item stride, and an epilogue that reads what the fold accumulated. -/
def tailExample : Program := {
  commonScalars := 2
  itemScalarStride := 1
  commonIdentities := 0
  itemIdentityStride := 0
  «prelude» := [.loadConst (common 0) 0, .loadConst (common 1) 1]
  itemBody := [.checkedAddInto (common 0) (item 0) (common 0)]
  epilogue := [.scalarEq (common 0) (common 1)]
}

theorem tail_example_is_well_formed : tailExample.wellFormed = true := by native_decide

theorem tail_example_encoded_width :
    (Codec.encodeProgram tailExample).length = 128 := by native_decide

/-- One item carrying `1` accumulates to the epilogue's expected total. -/
theorem tail_example_admits_single_unit :
    (tailExample.execute 1 ⟨#[7, 7, 1], #[]⟩).map (·.scalars) = some #[1, 1, 1] := by
  native_decide

/-- Two items carrying `1` do not: the epilogue refuses the accumulated two. -/
theorem tail_example_refuses_double_unit :
    tailExample.execute 2 ⟨#[7, 7, 1, 1], #[]⟩ = none := by native_decide

end DClutch.TransitionVMV3
