import DragonsClutch.BSplineCorpus

open DragonsClutch

private def csvNats (values : List Nat) : String :=
  String.intercalate "," (values.map toString)

/-!
This is a serializer, not another evaluator.  Every right-hand side is
computed by `corpusRows` in the checked Lean model, whose exactness at every
admitted uniform input is `uniformSmoothBasis?_exact` and whose integer sum is
`uniformSmoothWeights?_sum`.  The shell runner converts the rows to the
production oracle driver's input/output format and compares them byte for
byte.
-/
#eval do
  for row in corpusRows do
    IO.println s!"{row.driverInput}|{csvNats row.weights}"
