import DClutchSemantics.ProductBasisV3Abi

/-!
# Live runtime liability-basis V3 semantics

`ProductBasisV3Abi` says where the bytes of a `DCLTPAY3` record are. This says
what the evaluator *does* with them -- and until now nothing did, in any
language. The live evaluator decides real payouts inside two deployed cdylibs
and its behaviour existed only as 1,579 lines of handwritten Rust.

The model below is deliberately a restatement of the deployed evaluator rather
than an improvement on it. Three details are load-bearing and each is a place
where the obvious formulation would be subtly wrong:

* **Comparison is by cross-multiplication, never division.** A coordinate
  `n/d` is compared with a knot `k/D` as `n * D` against `k * d`, so no
  quantization happens before the single rounding boundary.
* **There is exactly one rounding boundary and it floors.**
  `interpolationFloor` is `elapsed * amplitude / width` truncated, reached only
  in the strict interior of a ramp. Both tails clamp exactly, so the evaluator
  is total over the whole signed rational line.
* **The final claim is not evaluated, it is the remainder.** Primary claims are
  floored independently and the complement receives `Q - sum(primary)`. That is
  what makes the partition exact despite per-term flooring, and it is why this
  rounding rule cannot be swapped for the kernel's cumulative-floor
  telescoping without moving money.

Nothing here is authoritative over the chain. This is the specification the
live evaluator is checked *against*, via an emitted corpus of fixed record
bytes; the Rust remains the sole writer under `O-005`.
-/

namespace DClutch.ProductBasisV3

open DClutch.ProductBasisV3Abi

/-- One exact nonnegative term shape over Product-owned knot indices. -/
inductive Shape where
  | constant
  | rampUp (left right : Nat)
  | rampDown (left right : Nat)
  | tent (left peak right : Nat)
  deriving DecidableEq, Repr

/-- The wire tag this shape encodes as, and the three knot-index slots. A
shape that does not read a slot forces it canonically zero rather than leaving
it free, which is what the live decoder requires on the way back in. -/
def Shape.encoded : Shape → Nat × Nat × Nat × Nat
  | .constant => (constantShape, 0, 0, 0)
  | .rampUp left right => (rampUpShape, left, 0, right)
  | .rampDown left right => (rampDownShape, left, 0, right)
  | .tent left peak right => (tentShape, left, peak, right)

/-- One canonical graded term assigned to a primary basis claim. -/
structure Term where
  claimIndex : Nat
  shape : Shape
  amplitude : Nat
  deriving DecidableEq, Repr

/-- A runtime basis, in the terms the evaluator actually uses. Identities are
carried as single bytes to be splatted across their 32-byte fields: they must
be nonzero for the record to decode, and nothing in the evaluation reads
them. -/
structure Basis where
  categorical : Bool
  width : Nat
  payoutScale : Nat
  knotDenominator : Nat
  knots : List Int
  terms : List Term
  failurePayouts : List Nat
  productByte : Nat := 0x11
  resultDomainByte : Nat := 0x22
  coordinateDomainByte : Nat := 0x33
  resultUnitByte : Nat := 0x44
  evaluatorReleaseByte : Nat := 0x55
  deriving Repr

/-! ## Evaluation -/

/-- The sole rounding boundary. Reached only in the strict interior of a ramp,
where `0 < elapsed < width`, so this is an honest floor and never a division by
zero. The live Rust computes the same quotient by binary search to avoid a
big-integer dependency; the value is the quotient either way. -/
def interpolationFloor (amplitude elapsed width : Nat) : Nat :=
  elapsed * amplitude / width

/-- One clamped ramp between two knots. `rising` selects which tail is zero.
Both tails are exact, which is what makes the evaluator total. -/
def rampValue (amplitude : Nat) (left right : Int) (knotDenominator : Nat)
    (numerator : Int) (denominator : Nat) (rising : Bool) : Option Nat :=
  let coordinateScaled : Int := numerator * (knotDenominator : Int)
  let leftScaled : Int := left * (denominator : Int)
  let rightScaled : Int := right * (denominator : Int)
  if coordinateScaled ≤ leftScaled then
    some (if rising then 0 else amplitude)
  else if rightScaled ≤ coordinateScaled then
    some (if rising then amplitude else 0)
  else
    let elapsed : Int := if rising then coordinateScaled - leftScaled
                         else rightScaled - coordinateScaled
    let span : Int := rightScaled - leftScaled
    if 0 < elapsed ∧ elapsed < span then
      some (interpolationFloor amplitude elapsed.toNat span.toNat)
    else
      none

/-- Look up a Product-owned knot by index. -/
def knotAt (knots : List Int) (index : Nat) : Option Int := knots[index]?

/-- Evaluate one term at an exact rational coordinate. A tent is the pointwise
minimum of its rising and falling halves, which is how the deployed evaluator
builds it -- not as a separate piecewise definition. -/
def evalTerm (basis : Basis) (term : Term) (numerator : Int) (denominator : Nat) :
    Option Nat :=
  match term.shape with
  | .constant => some term.amplitude
  | .rampUp left right => do
      let l ← knotAt basis.knots left
      let r ← knotAt basis.knots right
      rampValue term.amplitude l r basis.knotDenominator numerator denominator true
  | .rampDown left right => do
      let l ← knotAt basis.knots left
      let r ← knotAt basis.knots right
      rampValue term.amplitude l r basis.knotDenominator numerator denominator false
  | .tent left peak right => do
      let l ← knotAt basis.knots left
      let p ← knotAt basis.knots peak
      let r ← knotAt basis.knots right
      let rise ← rampValue term.amplitude l p basis.knotDenominator numerator denominator true
      let fall ← rampValue term.amplitude p r basis.knotDenominator numerator denominator false
      some (min rise fall)

/-- Accumulate one term's payout into the claim it names. -/
def depositAt (payouts : List Nat) (index amount : Nat) : List Nat :=
  payouts.mapIdx fun position value =>
    if position = index then value + amount else value

/-- Evaluate an ordinary coordinate into an exact partition of `Q`.

The final claim is assigned `Q - sum(primary)` rather than evaluated, so the
partition is exact even though every primary claim was floored independently.
`none` is a refusal: it means the primaries oversubscribed the scale, which the
live evaluator reports as `NonPartition`. -/
def evaluateRational (basis : Basis) (numerator : Int) (denominator : Nat) :
    Option (List Nat) := do
  let mut payouts : List Nat := List.replicate basis.width 0
  let mut total : Nat := 0
  for term in basis.terms do
    let payout ← evalTerm basis term numerator denominator
    payouts := depositAt payouts term.claimIndex payout
    total := total + payout
  if total > basis.payoutScale then
    none
  else
    some (payouts.set (basis.width - 1) (basis.payoutScale - total))

/-- The categorical `Q = 1` embedding: a one-hot vector at runtime width. -/
def evaluateCategorical (basis : Basis) (selector : Nat) : Option (List Nat) :=
  if selector < basis.width then
    some ((List.replicate basis.width 0).set selector 1)
  else
    none

/-! ## Encoding

The corpus is only worth something if its bytes are the bytes the chain would
see, so this reproduces the deployed encoder exactly rather than inventing a
convenient serialization. -/

def leBytes (width value : Nat) : List UInt8 :=
  (List.range width).map fun index =>
    UInt8.ofNat (value / (256 ^ index) % 256)

/-- Two's-complement little-endian, for the signed knot numerators. -/
def leBytesInt (width : Nat) (value : Int) : List UInt8 :=
  let modulus : Nat := 256 ^ width
  leBytes width (((value % (modulus : Int) + (modulus : Int)) % (modulus : Int)).toNat)

def idBytes (byte : Nat) : List UInt8 := List.replicate 32 (UInt8.ofNat byte)

def encodeTerm (term : Term) : List UInt8 :=
  let (tag, left, peak, right) := term.shape.encoded
  leBytes 4 term.claimIndex ++ leBytes 1 tag ++ List.replicate 3 0 ++
    leBytes 4 left ++ leBytes 4 peak ++ leBytes 4 right ++
    List.replicate 4 0 ++ leBytes 8 term.amplitude

def recordBytes (basis : Basis) : Nat :=
  let failures := if basis.categorical then 0 else basis.width
  headerBytes + failures * 8 + basis.knots.length * knotBytes +
    basis.terms.length * termBytes

def encodeBasis (basis : Basis) : List UInt8 :=
  let kind := if basis.categorical then categoricalKind else gradedExactComplementKind
  let rounding :=
    if basis.categorical then exactCategoricalBoundary else termFloorExactComplementBoundary
  let header :=
    basisMagic ++
    leBytes 2 basisSchemaVersion ++
    leBytes 2 headerBytes ++
    leBytes 4 (recordBytes basis) ++
    leBytes 1 kind ++
    leBytes 1 rounding ++
    List.replicate 2 0 ++
    leBytes 4 basis.width ++
    leBytes 4 basis.knots.length ++
    leBytes 4 basis.terms.length ++
    idBytes basis.productByte ++
    idBytes basis.resultDomainByte ++
    idBytes basis.coordinateDomainByte ++
    idBytes basis.resultUnitByte ++
    leBytes 8 basis.payoutScale ++
    leBytes 8 basis.knotDenominator ++
    idBytes basis.evaluatorReleaseByte ++
    List.replicate 48 0
  let failures := if basis.categorical then [] else basis.failurePayouts.flatMap (leBytes 8)
  header ++ failures ++ basis.knots.flatMap (leBytesInt knotBytes) ++
    basis.terms.flatMap encodeTerm

/-! ## Properties the corpus relies on

These are cheap, but they are the difference between a corpus that pins the
evaluator and a corpus that pins a typo in this file. -/

theorem le_bytes_length (width value : Nat) : (leBytes width value).length = width := by
  simp [leBytes]

theorem encode_term_width (term : Term) : (encodeTerm term).length = 32 := by
  cases term with | mk claimIndex shape amplitude =>
    cases shape <;> simp [encodeTerm, Shape.encoded, leBytes]

/-- The sole rounding boundary never exceeds the term's own amplitude. Both
ramp tails clamp to exactly `0` or `amplitude`, so with this the whole
evaluator is bounded per term -- which is the first half of the partition
argument. The complement supplies the second half by construction. -/
theorem interpolation_floor_le_amplitude (amplitude elapsed span : Nat)
    (inside : elapsed ≤ span) :
    interpolationFloor amplitude elapsed span ≤ amplitude := by
  unfold interpolationFloor
  exact Nat.div_le_of_le_mul (Nat.mul_le_mul_right amplitude inside)

/-- The boundary never rounds up: the payout it returns, scaled back by the
span, never exceeds the exact rational numerator it came from. This is the
property that makes per-term flooring safe to combine with an exact
complement, and it is the live counterpart of the kernel's own
`never_rounds_up`. -/
theorem interpolation_floor_never_rounds_up (amplitude elapsed span : Nat) :
    interpolationFloor amplitude elapsed span * span ≤ elapsed * amplitude := by
  unfold interpolationFloor
  exact Nat.div_mul_le_self _ _

end DClutch.ProductBasisV3
