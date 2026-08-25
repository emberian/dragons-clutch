import DClutchSemantics.DirectProgram
import DClutchSemantics.TransitionVMV2
import Std.Tactic

/-!
# Runtime-width Direct program projection

Direct ordinary semantics remain owned by `DirectProgram.program`. This module
only projects that exact operation list into the runtime-width DCTV2 physical
representation. It adds no family enum, Product-width branch, or alternative
transition meaning.
-/

namespace DClutch.DirectProgramV2

open DClutch

abbrev LegacyOp := TransitionVM.Op
abbrev RuntimeOp := TransitionVMV2.Op

/-- Meaning-preserving embedding of the V1 vocabulary into DCTV2. -/
def liftOp : LegacyOp → RuntimeOp
  | .loadConst destination value => .loadConst destination value
  | .scalarEq left right => .scalarEq left right
  | .identityEq left right => .identityEq left right
  | .identityNe left right => .identityNe left right
  | .scalarLt left right => .scalarLt left right
  | .scalarLe left right => .scalarLe left right
  | .nonzero source => .nonzero source
  | .lifecycleAccepts lifecycle maximum fill => .lifecycleAccepts lifecycle maximum fill
  | .incrementInto source destination => .incrementInto source destination
  | .mulDivExact left right denominator destination =>
      .mulDivExact left right denominator destination
  | .mulDivFloor left right denominator destination =>
      .mulDivFloor left right denominator destination
  | .addLe left right limit => .addLe left right limit
  | .addFitsU64 left right => .addFitsU64 left right
  | .subInto minuend subtrahend destination => .subInto minuend subtrahend destination
  | .selectEq left right ifEqual destination => .selectEq left right ifEqual destination
  | .selectZero source ifZero destination => .selectZero source ifZero destination

/-- The one runtime-width descriptor program for ordinary Direct matching. -/
def program : TransitionVMV2.Program := {
  scalarWidth := DirectProgram.Scalar.count
  identityWidth := DirectProgram.Identity.count
  operations := DirectProgram.program.map liftOp
}

theorem instruction_count : program.operations.length = 35 := by
  native_decide

theorem scalar_count : program.scalarWidth = 41 := by
  native_decide

theorem identity_count : program.identityWidth = 4 := by
  native_decide

theorem well_formed : program.wellFormed = true := by
  native_decide

theorem encoded_width :
    (TransitionVMV2.Codec.encodeProgram program).length = 856 := by
  native_decide

theorem example_runs :
    (program.execute (DirectProgram.state Direct.Examples.frame)).bind
      DirectProgram.outputs = some (1, 1, 1000, 2) := by
  native_decide

theorem hostile_zero_fill_refuses :
    program.execute (DirectProgram.state Direct.Examples.hostileZeroFill) = none := by
  native_decide

end DClutch.DirectProgramV2
