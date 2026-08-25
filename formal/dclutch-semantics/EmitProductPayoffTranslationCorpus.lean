import DClutchSemantics.ProductPayoffAbi

/-!
# Product-payoff translation corpus

Finite differential observations for the Lean-owned fixed ABI and evaluator.
This executable is test evidence, not a source-refinement or SBF theorem.
-/

open DClutch
open DClutch.Codec
open DClutch.Product
open DClutch.Product.PayoffAbi

namespace ProductPayoffTranslationCorpus

def domain (id : Nat) (knots : List Nat) : ResultDomain := {
  domainId := id
  coordinateUnitId := 9
  knots
}

def product (id : Nat) (knots : List Nat) (terms : List Term) : Product := {
  productId := id
  domain := domain (id + 1000) knots
  payoff := { payoutScale := 100, terms }
}

def constantProduct : Product := product 1 [0, 100] [
  { shape := .constant, amplitude := 7 }
]

def rampUpProduct : Product := product 2 [0, 25, 50, 75, 100] [
  { shape := .rampUp 0 4, amplitude := 10 }
]

def rampDownProduct : Product := product 3 [0, 25, 50, 75, 100] [
  { shape := .rampDown 0 4, amplitude := 10 }
]

def tentProduct : Product := product 4 [0, 25, 50, 75, 100] [
  { shape := .tent 1 2 3, amplitude := 20 }
]

def combinedProduct : Product := PayoffAbi.exampleProduct

def wideRampProduct : Product := product 6 [0, 18446744073709551615] [
  { shape := .rampUp 0 1, amplitude := 18446744073709551615 }
]

def programs : List (String × Product) := [
  ("constant", constantProduct),
  ("ramp-up", rampUpProduct),
  ("ramp-down", rampDownProduct),
  ("tent", tentProduct),
  ("combined", combinedProduct),
  ("wide-ramp", wideRampProduct)
]

def changedByte (byte : UInt8) : UInt8 := UInt8.ofNat (byte.toNat + 1)

def disposition (value : Option α) : String :=
  if value.isSome then "accept" else "reject"

def emitProgram (name : String) (value : Product) : IO Unit := do
  let encoded := PayoffAbi.encode value
  IO.println <| String.intercalate "|" [
    "payoff", name, hex encoded, toString value.payoff.liabilityBound
  ]
  for offset in List.range encoded.length do
    let mutated := encoded.set offset (changedByte (encoded[offset]?.getD 0))
    IO.println <| String.intercalate "|" [
      "payoff-mutation", name, toString offset,
      disposition (PayoffAbi.decode mutated)
    ]
  for width in List.range encoded.length do
    let truncated := encoded.take width
    IO.println <| String.intercalate "|" [
      "payoff-hostile-width", name, s!"truncate-{width}",
      hex truncated, disposition (PayoffAbi.decode truncated)
    ]
  let padded := encoded ++ [0]
  IO.println <| String.intercalate "|" [
    "payoff-hostile-width", name, "pad-1", hex padded,
    disposition (PayoffAbi.decode padded)
  ]

def evaluations : List (String × Product × List Nat) := [
  ("constant", constantProduct, [0, 37, 100, 101]),
  ("ramp-up", rampUpProduct, [0, 25, 37, 50, 75, 100, 101]),
  ("ramp-down", rampDownProduct, [0, 25, 37, 50, 75, 100, 101]),
  ("tent", tentProduct, [0, 25, 37, 50, 63, 75, 100, 101]),
  ("combined", combinedProduct, [0, 25, 37, 50, 63, 75, 100, 101]),
  ("wide-ramp", wideRampProduct,
    [0, 1, 18446744073709551614, 18446744073709551615])
]

def emitEvaluations (name : String) (value : Product) (coordinates : List Nat) : IO Unit := do
  for coordinate in coordinates do
    match PayoffAbi.evaluate? value coordinate with
    | none => IO.println <| String.intercalate "|" [
        "payoff-eval", name, toString coordinate, "reject"
      ]
    | some payout => IO.println <| String.intercalate "|" [
        "payoff-eval", name, toString coordinate, "accept", toString payout
      ]

def emitCollateral (name : String) (value : Product) : IO Unit := do
  let bound := value.payoff.liabilityBound
  for available in [bound - 1, bound, 18446744073709551615] do
    IO.println <| String.intercalate "|" [
      "payoff-collateral", name, toString available,
      if PayoffAbi.collateralizedBy value available then "accept" else "reject"
    ]

def zeroSpan (input : List UInt8) (offset width : Nat) : List UInt8 :=
  (List.range width).foldl (fun bytes index => bytes.set (offset + index) 0) input

def invalidKnotDuplicate : Product := product 20 [0, 0] [
  { shape := .constant, amplitude := 1 }
]

def invalidKnotOrder : Product := product 21 [0, 100, 50] [
  { shape := .constant, amplitude := 1 }
]

def invalidTermOrder : Product := product 22 [0, 50, 100] [
  { shape := .rampDown 0 2, amplitude := 2 },
  { shape := .rampUp 0 2, amplitude := 3 }
]

def invalidDuplicateShape : Product := product 23 [0, 50, 100] [
  { shape := .rampUp 0 2, amplitude := 2 },
  { shape := .rampUp 0 2, amplitude := 3 }
]

def invalidLiabilityOverflow : Product := product 24 [0, 50, 100] [
  { shape := .constant, amplitude := 1 },
  { shape := .rampUp 0 2, amplitude := 18446744073709551615 }
]

def hostilePrograms : List (String × List UInt8) :=
  let base := PayoffAbi.encode constantProduct
  let ramp := PayoffAbi.encode rampUpProduct
  [
    ("empty", []),
    ("header-reserved", base.set 12 1),
    ("zero-product-id", zeroSpan base 16 8),
    ("zero-domain-id", zeroSpan base 24 8),
    ("zero-coordinate-unit", zeroSpan base 32 8),
    ("zero-payout-scale", zeroSpan base 40 8),
    ("knot-count-one", base.set 10 1),
    ("knot-count-over-profile", base.set 10 17),
    ("term-count-zero", base.set 11 0),
    ("term-count-over-profile", base.set 11 17),
    ("unused-knot-nonzero", base.set (knotsOffset + 2 * knotBytes) 1),
    ("unused-term-nonzero", base.set (termsOffset + termBytes) 1),
    ("zero-amplitude", zeroSpan base (termsOffset + 8) 8),
    ("unknown-shape", base.set termsOffset 4),
    ("constant-carries-index", base.set (termsOffset + 1) 1),
    ("term-reserved", base.set (termsOffset + 4) 1),
    ("ramp-carries-peak", ramp.set (termsOffset + 2) 1),
    ("ramp-reversed", ramp.set (termsOffset + 1) 4 |>.set (termsOffset + 3) 0),
    ("duplicate-knots", PayoffAbi.encode invalidKnotDuplicate),
    ("unordered-knots", PayoffAbi.encode invalidKnotOrder),
    ("unordered-terms", PayoffAbi.encode invalidTermOrder),
    ("duplicate-shape", PayoffAbi.encode invalidDuplicateShape),
    ("liability-overflow", PayoffAbi.encode invalidLiabilityOverflow)
  ]

def emitHostile (name : String) (encoded : List UInt8) : IO Unit :=
  IO.println <| String.intercalate "|" [
    "payoff-hostile", name, hex encoded, disposition (PayoffAbi.decode encoded)
  ]

end ProductPayoffTranslationCorpus

def main : IO Unit := do
  for entry in ProductPayoffTranslationCorpus.programs do
    ProductPayoffTranslationCorpus.emitProgram entry.1 entry.2
  for entry in ProductPayoffTranslationCorpus.evaluations do
    ProductPayoffTranslationCorpus.emitEvaluations entry.1 entry.2.1 entry.2.2
  for entry in ProductPayoffTranslationCorpus.programs do
    ProductPayoffTranslationCorpus.emitCollateral entry.1 entry.2
  for entry in ProductPayoffTranslationCorpus.hostilePrograms do
    ProductPayoffTranslationCorpus.emitHostile entry.1 entry.2
