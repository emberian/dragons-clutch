import DClutchSemantics.ProductPayoff

/-!
# Executable Product payoff examples

These vectors exercise canonical boundary ownership, floor interpolation,
overlapping ramp/tent terms, structural collateral, and the deliberately
coarse categorical error theorem.
-/

namespace DClutch.Product.Examples

def domain : ResultDomain := {
  domainId := 7001
  coordinateUnitId := 9
  knots := [0, 25, 50, 75, 100]
}

def smoothProduct : Product := {
  productId := 8101
  domain
  payoff := {
    payoutScale := 100
    terms := [
      { shape := .rampUp 0 4, amplitude := 10 },
      { shape := .tent 1 2 3, amplitude := 20 }
    ]
  }
}

/-- One value for each canonical segment, sampled at its left endpoint. -/
def categorical : CategoricalApproximation := {
  domainId := domain.domainId
  values := [0, 2, 25, 7]
}

example : domain.valid = true := by native_decide
example : smoothProduct.valid = true := by native_decide
example : smoothProduct.payoff.liabilityBound = 30 := by native_decide
example : smoothProduct.collateralizedBy 30 = true := by native_decide
example : smoothProduct.collateralizedBy 29 = false := by native_decide

/-! Interior knots belong to the cell on their right; the last endpoint stays
in the final cell. -/
example : domain.cellIndex 0 = 0 := by native_decide
example : domain.cellIndex 24 = 0 := by native_decide
example : domain.cellIndex 25 = 1 := by native_decide
example : domain.cellIndex 74 = 2 := by native_decide
example : domain.cellIndex 75 = 3 := by native_decide
example : domain.cellIndex 100 = 3 := by native_decide

/-! The ramp contributes `floor (10 * 37 / 100) = 3`; the tent contributes
`floor (20 * 12 / 25) = 9`. -/
example : smoothProduct.payoff.evaluate domain 37 = 12 := by native_decide
example : smoothProduct.payoff.evaluate domain 50 = 25 := by native_decide
example : smoothProduct.payoff.evaluate domain 100 = 10 := by native_decide
example : smoothProduct.evaluate? 101 = none := by native_decide

example : categorical.validFor smoothProduct = true := by native_decide
example : categorical.evaluate smoothProduct 37 = 2 := by native_decide
example :
    absoluteError
      (smoothProduct.payoff.evaluate domain 37)
      (categorical.evaluate smoothProduct 37) = 10 := by native_decide

example :
    absoluteError
        (smoothProduct.payoff.evaluate domain 37)
        (categorical.evaluate smoothProduct 37) ≤
      smoothProduct.payoff.liabilityBound := by
  exact categorical_approximation_sound smoothProduct categorical 37
    (by native_decide) (by native_decide) (by native_decide)

/-! The exact compiler-side enumeration proves that this particular left-knot
projection is within 21 scaled payout units at all 101 integer coordinates.
It rejects the next smaller advertised tolerance. -/
example : categorical.certifiesError smoothProduct 21 = true := by native_decide
example : categorical.certifiesError smoothProduct 20 = false := by native_decide

example :
    absoluteError
        (smoothProduct.payoff.evaluate domain 63)
        (categorical.evaluate smoothProduct 63) ≤ 21 := by
  exact checked_categorical_approximation_sound smoothProduct categorical 21 63
    (by native_decide) (by native_decide)

end DClutch.Product.Examples
