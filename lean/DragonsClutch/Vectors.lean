import DragonsClutch.Transitions
/-!
# The model runs: two canonical vectors, checked at build time

The model's transitions are total *computable* functions, so the same file that
carries the theorems can evaluate a semantic vector.  These two are transcribed
by hand from `fixtures/vectors/kernel/core.json` — the `lean-checker` column of
that manifest is `pending` today, and this file is the existence proof that the
column is reachable, not the column itself.  The design of the real checker (a
program that reads the JSON rather than restating it in Lean source) is in
`docs/implementation/LEAN_MODEL_PLAN.md`.

`#guard` fails the build when the equation does not hold, so these are checks,
not comments.

Two honest limits, both recorded in the plan:

* A hand-transcribed vector checks the model against a *reading* of the
  manifest, not against its bytes.  Only a JSON reader closes that.
* Agreement here is agreement on success values and on refusal *classes*.  It is
  not agreement with the Rust implementation on anything a vector does not name.
-/

namespace DragonsClutch
namespace Vectors

/-! ## `kernel-binary-split-resolve-redeem-exact` -/

def binaryPayouts : PayoutSet :=
  { outcomes := 2,
    vectors := [ { denominator := 1, weights := [1, 0] },
                 { denominator := 1, weights := [0, 1] } ] }

def binaryMarket : Market :=
  { outcomes := 2, basisMode := .finitePreset, payouts := binaryPayouts,
    resolution := .active, collateral := 0, totalSupply := [0, 0] }

def emptyPosition : Position := { internal := [0, 0], external := [0, 0] }

/-! The constructor admits the initial state of the vector. -/
#guard (Market.new 2 .finitePreset binaryPayouts 0).isOk

def splitResolveRedeem : Except Error (Market × Position × Amount) := do
  let (m₁, p₁) ← binaryMarket.split emptyPosition 11
  let m₂ ← m₁.resolve 1
  m₂.redeem p₁ .internal 1 10

def observe : Except Error (Market × Position × Amount) →
    Option (Amount × List Amount × List Amount × List Amount × Amount)
  | .ok (m, p, a) => some (m.collateral, m.totalSupply, p.internal, p.external, a)
  | .error _ => none

/-! The vector's `final_state` and success value, exactly. -/
#guard observe splitResolveRedeem == some (1, [11, 1], [11, 1], [0, 0], 10)

/-! ## `kernel-complete-set-exits-the-fractional-payout-trap`

One payout vector `[1, 1] / 2`, under which every single-outcome redemption of
an odd quantity remainders forever, and the complete set still exits. -/

def fractionalPayouts : PayoutSet :=
  { outcomes := 2, vectors := [ { denominator := 2, weights := [1, 1] } ] }

def fractionalMarket : Market :=
  { outcomes := 2, basisMode := .finitePreset, payouts := fractionalPayouts,
    resolution := .active, collateral := 0, totalSupply := [0, 0] }

def resolvedFractional : Except Error (Market × Position) := do
  let (m₁, p₁) ← fractionalMarket.split emptyPosition 1
  let m₂ ← m₁.resolve 0
  .ok (m₂, p₁)

def singleOutcomeRefusal (i : Nat) : Option Error :=
  match resolvedFractional with
  | .ok (m, p) => match m.redeem p .internal i 1 with
      | .error e => some e
      | .ok _ => none
  | .error e => some e

/-! Both single-outcome redemptions refuse, and refuse with the exact class the
vector names (`arith.remainder-not-representable`). -/
#guard singleOutcomeRefusal 0 == some Error.remainderRequired
#guard singleOutcomeRefusal 1 == some Error.remainderRequired

def completeSetExit : Option (Amount × List Amount × List Amount × Amount) :=
  match resolvedFractional with
  | .ok (m, p) => match m.redeemCompleteSet p 1 with
      | .ok (m', p', payout) => some (m'.collateral, m'.totalSupply, p'.internal, payout)
      | .error _ => none
  | .error _ => none

/-! The complete set redeems for exactly `1`, which is `P_PAY_02` on one point of
the lattice rather than for all of it. -/
#guard completeSetExit == some (0, [0, 0], [0, 0], 1)

end Vectors
end DragonsClutch
