/-!
# Compact protocol intermediate representations

These are data, not a family of width-specialized functions.  Product data owns
the result coordinate system, frame data owns the account-role ABI, and effect
data owns the bounded state mutation plan.
-/

namespace DClutch

/-- One semantic party in the first Direct vertical slice. -/
inductive Party where
  | seller
  | buyer
  | venue
  deriving DecidableEq, Repr

/-- A typed resource coordinate. -/
inductive Resource where
  | replayNonce
  | outcomeClaim (outcome : Nat)
  | collateral
  deriving DecidableEq, Repr

/-- One typed cell in the protocol state projection. -/
structure Cell where
  party : Party
  resource : Resource
  deriving DecidableEq, Repr

/-- A bounded-state effect. `set` is reserved for non-resource facts. -/
inductive Effect where
  | set (cell : Cell) (value : Nat)
  | debit (cell : Cell) (amount : Nat)
  | credit (cell : Cell) (amount : Nat)
  deriving DecidableEq, Repr

/-- A first-order effect plan emitted by one admitted semantic transition. -/
structure EffectPlan where
  effects : List Effect
  deriving DecidableEq, Repr

/-- Permissions carried by account-role data rather than handwritten codecs. -/
inductive Permission where
  | read
  | write
  | signer
  | executable
  deriving DecidableEq, Repr

/-- One semantic role in an instruction frame. -/
structure FrameRole where
  tag : Nat
  permissions : List Permission
  deriving DecidableEq, Repr

/-- Versioned frame data from which adapters and clients can be generated. -/
structure FrameIR where
  version : Nat
  roles : List FrameRole
  maxEffects : Nat
  deriving DecidableEq, Repr

/-- A finite result domain. There is deliberately no semantic N=16 ceiling. -/
structure ProductIR where
  outcomeCount : Nat
  outcomeCountPositive : 0 < outcomeCount
  priceScale : Nat
  priceScalePositive : 0 < priceScale

namespace FrameTags

def market : Nat := 1
def sellerReplay : Nat := 2
def buyerReplay : Nat := 3
def sellerPosition : Nat := 4
def buyerPosition : Nat := 5
def buyerCollateral : Nat := 6
def sellerCollateral : Nat := 7
def venueCollateral : Nat := 8

end FrameTags

/-- Semantic frame for an inline ordinary Direct fill.

Transport-only roles such as the Instructions sysvar and token program live in
the Solana adapter manifest. They are not falsely presented as economic state.
-/
def inlineOrdinaryFrame : FrameIR := {
  version := 1
  roles := [
    { tag := FrameTags.market, permissions := [.read] },
    { tag := FrameTags.sellerReplay, permissions := [.write] },
    { tag := FrameTags.buyerReplay, permissions := [.write] },
    { tag := FrameTags.sellerPosition, permissions := [.write] },
    { tag := FrameTags.buyerPosition, permissions := [.write] },
    { tag := FrameTags.buyerCollateral, permissions := [.write] },
    { tag := FrameTags.sellerCollateral, permissions := [.write] },
    { tag := FrameTags.venueCollateral, permissions := [.write] }
  ]
  maxEffects := 7
}

theorem inlineOrdinaryFrame_role_count : inlineOrdinaryFrame.roles.length = 8 := by
  rfl

end DClutch
