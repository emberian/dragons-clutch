import DClutchSemantics.ProductBasisV3

open DClutch DClutch.ProductBasisV3Abi DClutch.ProductBasisV3

/-!
Emit the conformance corpus for the LIVE runtime basis evaluator.

Every case is a complete `DCLTPAY3` record in the bytes the chain would see,
paired with the exact partition this specification says the evaluator must
produce at a named coordinate. A case is emitted only if it encodes, evaluates
and partitions here first: a corpus that quietly recorded a refusal as an
expectation would pin nothing.

The corpus lives under `tests/` on purpose. It is several kilobytes of record
bytes, and the crate it checks is linked into two deployed cdylibs on a route
with four figures of CU headroom; test data has no business in that ELF.
-/

def rustByte (byte : UInt8) : String := s!"0x{Codec.byteHex byte}"

def rustByteSlice (bytes : List UInt8) : String :=
  s!"&[{String.intercalate ", " (bytes.map rustByte)}]"

def rustNatSlice (values : List Nat) : String :=
  s!"&[{String.intercalate ", " (values.map toString)}]"

/-- One graded case: a basis, a coordinate, and the story it tells. -/
structure GradedCase where
  note : String
  basis : Basis
  numerator : Int
  denominator : Nat

/-- One categorical case. -/
structure CategoricalCase where
  note : String
  basis : Basis
  selector : Nat

/-! ## The bases the corpus is built from -/

/-- Width 2, `Q = 100`, knots at 0/10/20 with denominator 1. -/
def gradedBasis (terms : List Term) : Basis :=
  { categorical := false, width := 2, payoutScale := 100, knotDenominator := 1,
    knots := [0, 10, 20], terms := terms, failurePayouts := [0, 100] }

def rampUpBasis : Basis := gradedBasis [{ claimIndex := 0, shape := .rampUp 0 1, amplitude := 100 }]
def rampDownBasis : Basis :=
  gradedBasis [{ claimIndex := 0, shape := .rampDown 0 1, amplitude := 100 }]
def tentBasis : Basis := gradedBasis [{ claimIndex := 0, shape := .tent 0 1 2, amplitude := 100 }]
def constantBasis : Basis :=
  gradedBasis [{ claimIndex := 0, shape := .constant, amplitude := 40 }]

/-- Two terms on ONE claim, so the accumulation path is exercised rather than
assumed. Canonical `(claim, shape)` order puts `rampUp` before `rampDown`. -/
def accumulatingBasis : Basis :=
  gradedBasis [{ claimIndex := 0, shape := .rampUp 0 1, amplitude := 30 },
               { claimIndex := 0, shape := .rampDown 1 2, amplitude := 30 }]

/-- Width 3: two primary claims plus the complement. -/
def threeClaimBasis : Basis :=
  { categorical := false, width := 3, payoutScale := 100, knotDenominator := 1,
    knots := [0, 10, 20],
    terms := [{ claimIndex := 0, shape := .rampDown 0 1, amplitude := 60 },
              { claimIndex := 1, shape := .rampUp 1 2, amplitude := 60 }],
    failurePayouts := [50, 30, 20] }

/-- Negative knots, so the signed line and the two's-complement encoding of a
knot numerator are both exercised rather than taken on trust. -/
def negativeKnotBasis : Basis :=
  { categorical := false, width := 2, payoutScale := 100, knotDenominator := 1,
    knots := [-20, -10, 0],
    terms := [{ claimIndex := 0, shape := .rampUp 0 1, amplitude := 100 }],
    failurePayouts := [0, 100] }

/-- Knot denominator 4 against coordinate denominator 4: the comparison is a
cross-multiplication and this is the case that would catch it being a
division. -/
def scaledKnotBasis : Basis :=
  { categorical := false, width := 2, payoutScale := 100, knotDenominator := 4,
    knots := [0, 10, 20],
    terms := [{ claimIndex := 0, shape := .rampUp 0 1, amplitude := 100 }],
    failurePayouts := [100, 0] }

def categoricalBasis (width : Nat) : Basis :=
  { categorical := true, width := width, payoutScale := 1, knotDenominator := 1,
    knots := [], terms := [], failurePayouts := [] }

/-! ## The cases

The coverage argument, stated so a reader can check it rather than trust it:
every named branch of the deployed evaluator appears below. Both clamped tails
of a rising ramp and both of a falling one; both closed endpoints, where the
comparison is `<=` rather than `<` and an off-by-one would hide; the strict
interior, which is the only place rounding happens; a floor with a nonzero
remainder, which distinguishes flooring from rounding; a tent on each side of
its peak and at it; term accumulation onto one claim; more than one primary
claim; negative coordinates; and a knot denominator differing from the
coordinate denominator. -/

def gradedCases : List GradedCase := [
  -- Rising ramp: below the left knot, at it, interior, at the right, beyond.
  { note := "rampUp below left clamps to zero", basis := rampUpBasis,
    numerator := -5, denominator := 1 },
  { note := "rampUp AT left clamps to zero, the closed endpoint",
    basis := rampUpBasis, numerator := 0, denominator := 1 },
  { note := "rampUp midpoint floors to half the amplitude",
    basis := rampUpBasis, numerator := 5, denominator := 1 },
  { note := "rampUp at three tenths", basis := rampUpBasis,
    numerator := 3, denominator := 1 },
  { note := "rampUp AT right clamps to the amplitude, the closed endpoint",
    basis := rampUpBasis, numerator := 10, denominator := 1 },
  { note := "rampUp beyond right clamps to the amplitude",
    basis := rampUpBasis, numerator := 15, denominator := 1 },
  -- The two cases that distinguish a floor from a round: 100/30 is 3.33 and
  -- 200/30 is 6.67, so a rounding evaluator would answer 3 and 7.
  { note := "rampUp at one third: floor(100/30) = 3, NOT the rounded 3",
    basis := rampUpBasis, numerator := 1, denominator := 3 },
  { note := "rampUp at two thirds: floor(200/30) = 6, where rounding gives 7",
    basis := rampUpBasis, numerator := 2, denominator := 3 },
  -- Falling ramp: the mirror of all three regions.
  { note := "rampDown below left clamps to the amplitude",
    basis := rampDownBasis, numerator := -5, denominator := 1 },
  { note := "rampDown midpoint", basis := rampDownBasis,
    numerator := 5, denominator := 1 },
  { note := "rampDown beyond right clamps to zero", basis := rampDownBasis,
    numerator := 15, denominator := 1 },
  -- Tent: outside on both sides, at the peak, and interior on each half.
  { note := "tent left of its support is zero", basis := tentBasis,
    numerator := -5, denominator := 1 },
  { note := "tent rising half", basis := tentBasis, numerator := 5, denominator := 1 },
  { note := "tent AT the peak is the full amplitude", basis := tentBasis,
    numerator := 10, denominator := 1 },
  { note := "tent falling half", basis := tentBasis, numerator := 15, denominator := 1 },
  { note := "tent right of its support is zero", basis := tentBasis,
    numerator := 25, denominator := 1 },
  -- Constant, which reads no knot at all.
  { note := "constant term ignores the coordinate", basis := constantBasis,
    numerator := 7, denominator := 1 },
  -- Accumulation, several claims, signed knots, scaled knots.
  { note := "two terms on one claim accumulate", basis := accumulatingBasis,
    numerator := 15, denominator := 1 },
  { note := "three claims, first primary active", basis := threeClaimBasis,
    numerator := 5, denominator := 1 },
  { note := "three claims, second primary active", basis := threeClaimBasis,
    numerator := 15, denominator := 1 },
  { note := "negative knots and a negative coordinate", basis := negativeKnotBasis,
    numerator := -15, denominator := 1 },
  { note := "knot denominator 4 against coordinate denominator 4",
    basis := scaledKnotBasis, numerator := 5, denominator := 4 }
]

def categoricalCases : List CategoricalCase := [
  { note := "categorical width 2, first claim", basis := categoricalBasis 2, selector := 0 },
  { note := "categorical width 2, last claim", basis := categoricalBasis 2, selector := 1 },
  { note := "categorical width 5, interior claim", basis := categoricalBasis 5, selector := 3 }
]

/-! ## Emission

Each case must survive evaluation here before it is written out. An expectation
this file could not compute is a bug in the specification, and it should stop
the build rather than become a comment in a corpus. -/

def emitGraded (index : Nat) (case : GradedCase) : IO Unit := do
  let bytes := encodeBasis case.basis
  if bytes.length != recordBytes case.basis then
    throw <| IO.userError s!"graded case {index} encoded to the wrong width"
  let payouts ← match evaluateRational case.basis case.numerator case.denominator with
    | some payouts => pure payouts
    | none => throw <| IO.userError s!"graded case {index} ({case.note}) did not evaluate"
  if payouts.length != case.basis.width then
    throw <| IO.userError s!"graded case {index} produced the wrong width"
  if payouts.sum != case.basis.payoutScale then
    throw <| IO.userError s!"graded case {index} did not partition its scale exactly"
  if case.basis.failurePayouts.sum != case.basis.payoutScale then
    throw <| IO.userError s!"graded case {index} has a failure vector that is not a partition"
  IO.println s!"    // {case.note}"
  IO.println "    BasisAgreementCaseV3 {"
  IO.println s!"        record: {rustByteSlice bytes},"
  IO.println s!"        coordinate_numerator: {case.numerator},"
  IO.println s!"        coordinate_denominator: {case.denominator},"
  IO.println s!"        expected: {rustNatSlice payouts},"
  IO.println s!"        expected_failure: {rustNatSlice case.basis.failurePayouts},"
  IO.println "    },"

def emitCategorical (index : Nat) (case : CategoricalCase) : IO Unit := do
  let bytes := encodeBasis case.basis
  let payouts ← match evaluateCategorical case.basis case.selector with
    | some payouts => pure payouts
    | none => throw <| IO.userError s!"categorical case {index} ({case.note}) did not evaluate"
  if payouts.sum != 1 then
    throw <| IO.userError s!"categorical case {index} was not one-hot"
  IO.println s!"    // {case.note}"
  IO.println "    BasisCategoricalCaseV3 {"
  IO.println s!"        record: {rustByteSlice bytes},"
  IO.println s!"        selector: {case.selector},"
  IO.println s!"        expected: {rustNatSlice payouts},"
  IO.println "    },"

def main : IO Unit := do
  IO.println "// @generated by formal/dclutch-semantics/EmitProductBasisV3CorpusRust.lean; do not edit."
  IO.println "use super::{BasisAgreementCaseV3, BasisCategoricalCaseV3};"
  IO.println
    s!"pub const BASIS_AGREEMENT_CASES_V3: [BasisAgreementCaseV3; {gradedCases.length}] = ["
  let mut index := 0
  for case in gradedCases do
    emitGraded index case
    index := index + 1
  IO.println "];"
  IO.println
    s!"pub const BASIS_CATEGORICAL_CASES_V3: [BasisCategoricalCaseV3; {categoricalCases.length}] = ["
  index := 0
  for case in categoricalCases do
    emitCategorical index case
    index := index + 1
  IO.println "];"
