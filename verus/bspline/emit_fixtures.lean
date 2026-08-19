import DragonsClutch.BSpline

open DragonsClutch

private def csvNats (values : List Nat) : String :=
  String.intercalate "," (values.map toString)

/-!
This is a serializer, not another evaluator.  Every right-hand side is
computed by `bsplineRefinementFixtures` in the checked Lean model.  The shell
runner converts the rows to the production oracle driver's input/output
format and compares them byte for byte.
-/
#eval do
  for fixture in bsplineRefinementFixtures do
    IO.println s!"{fixture.driverInput}|{csvNats fixture.modelWeights}"
