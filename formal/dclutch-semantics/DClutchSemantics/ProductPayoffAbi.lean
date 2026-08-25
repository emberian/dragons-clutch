import DClutchSemantics.Codec
import DClutchSemantics.ProductPayoff

/-!
# Fixed Product payoff ABI

This module owns the one fixed-layout V1 encoding for the exact finite payoff
basis.  It admits at most sixteen Product-owned knots and sixteen nonzero,
canonically ordered terms.  Those are explicit physical-profile bounds, not
mathematical limits of `ProductPayoff`.

All integer fields are little-endian `u64`.  Unused fixed-capacity spans are
zero.  The decoder requires strictly increasing knots, normalized shape
indices, strictly increasing shape keys, and a conservative amplitude sum that
fits `u64`.  Evaluation continues to use `payoffInterpolationFloor` as its sole
rounding boundary.
-/

namespace DClutch.Product.PayoffAbi

open DClutch.Codec

def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x50, 0x41, 0x59, 0x31] -- `DCLTPAY1`

def version : Nat := 1
def maxKnots : Nat := 16
def maxTerms : Nat := 16
def headerBytes : Nat := 48
def knotBytes : Nat := 8
def termBytes : Nat := 16
def knotsOffset : Nat := headerBytes
def termsOffset : Nat := knotsOffset + maxKnots * knotBytes
def bytes : Nat := termsOffset + maxTerms * termBytes
def u64Limit : Nat := 18446744073709551616

def shapeTag : Shape → Nat
  | .constant => 0
  | .rampUp .. => 1
  | .rampDown .. => 2
  | .tent .. => 3

def shapeIndices : Shape → Nat × Nat × Nat
  | .constant => (0, 0, 0)
  | .rampUp left right | .rampDown left right => (left, 0, right)
  | .tent left peak right => (left, peak, right)

/-- A unique order key for normalized V1 shapes. -/
def shapeKey : Shape → Nat
  | .constant => 0
  | .rampUp left right => 4096 + left * 16 + right
  | .rampDown left right => 8192 + left * 16 + right
  | .tent left peak right => 12288 + left * 256 + peak * 16 + right

def Term.canonicalFor (term : Term) (domain : ResultDomain) : Bool :=
  0 < term.amplitude && term.amplitude < u64Limit &&
    term.shape.validFor domain

def Product.physicalValid (product : Product) : Bool :=
  product.valid && product.productId < u64Limit &&
    product.domain.domainId < u64Limit &&
    product.domain.coordinateUnitId < u64Limit &&
    product.payoff.payoutScale < u64Limit &&
    product.domain.knots.length ≤ maxKnots &&
    product.payoff.terms.length ≤ maxTerms &&
    product.domain.knots.all (fun knot => knot < u64Limit) &&
    product.payoff.terms.all (fun term => Term.canonicalFor term product.domain) &&
    strictlyIncreasing (product.payoff.terms.map fun term => shapeKey term.shape) &&
    product.payoff.liabilityBound < u64Limit

def encodeTerm (term : Term) : List UInt8 :=
  let indices := shapeIndices term.shape
  [UInt8.ofNat (shapeTag term.shape), UInt8.ofNat indices.1,
    UInt8.ofNat indices.2.1, UInt8.ofNat indices.2.2, 0, 0, 0, 0] ++
    encodeLE 8 term.amplitude

def encodeKnots (knots : List Nat) : List UInt8 :=
  knots.flatMap (encodeLE 8) ++
    List.replicate ((maxKnots - knots.length) * knotBytes) 0

def encodeTerms (terms : List Term) : List UInt8 :=
  terms.flatMap encodeTerm ++
    List.replicate ((maxTerms - terms.length) * termBytes) 0

/-- Encode one fixed-width physical Product payoff.  Callers admit only
`physicalValid` values; the hostile decoder enforces that boundary. -/
def encode (product : Product) : List UInt8 :=
  magic ++ encodeLE 2 version ++
    [UInt8.ofNat product.domain.knots.length,
      UInt8.ofNat product.payoff.terms.length, 0, 0, 0, 0] ++
    encodeLE 8 product.productId ++
    encodeLE 8 product.domain.domainId ++
    encodeLE 8 product.domain.coordinateUnitId ++
    encodeLE 8 product.payoff.payoutScale ++
    encodeKnots product.domain.knots ++ encodeTerms product.payoff.terms

def slice (input : List UInt8) (offset width : Nat) : List UInt8 :=
  (input.drop offset).take width

def byteAt (input : List UInt8) (offset : Nat) : Option Nat := do
  let byte ← input[offset]?
  some byte.toNat

def allZero (input : List UInt8) : Bool :=
  input.all (· == 0)

def decodeTerm (input : List UInt8) : Option Term := do
  if input.length != termBytes then none else
  if !allZero (slice input 4 4) then none else
  let tag ← byteAt input 0
  let left ← byteAt input 1
  let peak ← byteAt input 2
  let right ← byteAt input 3
  let amplitude := decodeLE (slice input 8 8)
  if amplitude = 0 then none else
  let shape ← match tag with
    | 0 => if left = 0 && peak = 0 && right = 0 then some .constant else none
    | 1 => if peak = 0 then some (.rampUp left right) else none
    | 2 => if peak = 0 then some (.rampDown left right) else none
    | 3 => some (.tent left peak right)
    | _ => none
  some { shape, amplitude }

def decodeTerms (count : Nat) (input : List UInt8) : Option (List Term) :=
  (List.range count).mapM fun index =>
    decodeTerm (slice input (termsOffset + index * termBytes) termBytes)

def decodeKnots (count : Nat) (input : List UInt8) : List Nat :=
  (List.range count).map fun index =>
    decodeLE (slice input (knotsOffset + index * knotBytes) knotBytes)

/-- Hostile decoder for the exact 432-byte Product payoff ABI. -/
def decode (input : List UInt8) : Option Product := do
  if input.length != bytes then none else
  if slice input 0 8 != magic then none else
  if decodeLE (slice input 8 2) != version then none else
  if !allZero (slice input 12 4) then none else
  let knotCount ← byteAt input 10
  let termCount ← byteAt input 11
  if knotCount < 2 || maxKnots < knotCount then none else
  if termCount = 0 || maxTerms < termCount then none else
  if !allZero (slice input (knotsOffset + knotCount * knotBytes)
      ((maxKnots - knotCount) * knotBytes)) then none else
  if !allZero (slice input (termsOffset + termCount * termBytes)
      ((maxTerms - termCount) * termBytes)) then none else
  let terms ← decodeTerms termCount input
  let product : Product := {
    productId := decodeLE (slice input 16 8)
    domain := {
      domainId := decodeLE (slice input 24 8)
      coordinateUnitId := decodeLE (slice input 32 8)
      knots := decodeKnots knotCount input
    }
    payoff := {
      payoutScale := decodeLE (slice input 40 8)
      terms
    }
  }
  if Product.physicalValid product then some product else none

/-- Exact physical evaluation.  The conservative bound accepted by the
decoder ensures the result fits `u64`. -/
def evaluate? (product : Product) (coordinate : Nat) : Option Nat :=
  if Product.physicalValid product && coordinate < u64Limit &&
      product.domain.inDomain coordinate then
    some (product.payoff.evaluate product.domain coordinate)
  else none

/-- Conservative collateral admission for one Product unit. -/
def collateralizedBy (product : Product) (available : Nat) : Bool :=
  Product.physicalValid product && available < u64Limit &&
    product.payoff.liabilityBound ≤ available

def exampleProduct : Product := {
  productId := 8101
  domain := {
    domainId := 7001
    coordinateUnitId := 9
    knots := [0, 25, 50, 75, 100]
  }
  payoff := {
    payoutScale := 100
    terms := [
      { shape := .constant, amplitude := 2 },
      { shape := .rampUp 0 4, amplitude := 10 },
      { shape := .rampDown 0 4, amplitude := 5 },
      { shape := .tent 1 2 3, amplitude := 20 }
    ]
  }
}

theorem schema_width : bytes = 432 := by native_decide
theorem example_physical : Product.physicalValid exampleProduct = true := by native_decide
theorem example_round_trip : decode (encode exampleProduct) = some exampleProduct := by
  native_decide
theorem example_floor : evaluate? exampleProduct 37 = some 17 := by native_decide
theorem example_liability : exampleProduct.payoff.liabilityBound = 37 := by native_decide

end DClutch.Product.PayoffAbi
