import DClutchSemantics.Codec
import Std.Tactic

/-!
# Exact categorical claim shards and explicit remainder

This is the allocation-friendly mathematical owner for the Fractional V1
successor. Token owns shard supply and holder balances; Claims owns locked
native categorical claims. While open, and for the winning outcome after
terminal resolution, their exact join is `F = D * C`.

`divideClaimShardsV1` is the sole quotient/remainder boundary. It returns whole
native claims plus change in the same transferable shard instrument. Losing
shards may burn individually only after an authenticated terminal outcome and
pay zero. No theorem introduces a wrapper balance or residual-credit ledger.
-/

namespace DClutch.FractionalClaimV1

/-- Exact Claims-native custody and Token shard-supply observation. -/
structure Reserve where
  lockedNativeClaims : Nat
  shardSupply : Nat
  deriving DecidableEq, Repr

/-- Open and winning-terminal denomination invariant. -/
def Reserve.exact (denominator : Nat) (reserve : Reserve) : Prop :=
  1 < denominator ∧ reserve.shardSupply = denominator * reserve.lockedNativeClaims

/-- Lock whole native claims and mint exactly `D` shard atoms per claim. -/
def wrap (denominator quantity : Nat) (reserve : Reserve) : Reserve := {
  lockedNativeClaims := reserve.lockedNativeClaims + quantity
  shardSupply := reserve.shardSupply + denominator * quantity
}

/-- Burn a whole-denominator multiple and release or redeem native claims. -/
def unwrapWhole (denominator wholeClaims : Nat) (reserve : Reserve) : Reserve := {
  lockedNativeClaims := reserve.lockedNativeClaims - wholeClaims
  shardSupply := reserve.shardSupply - denominator * wholeClaims
}

theorem wrap_preserves_exact
    (denominator quantity : Nat) (reserve : Reserve)
    (exact : reserve.exact denominator) :
    (wrap denominator quantity reserve).exact denominator := by
  constructor
  · exact exact.1
  · simp only [wrap]
    rw [exact.2, Nat.mul_add]

theorem unwrapWhole_preserves_exact
    (denominator wholeClaims : Nat) (reserve : Reserve)
    (exact : reserve.exact denominator) :
    (unwrapWhole denominator wholeClaims reserve).exact denominator := by
  constructor
  · exact exact.1
  · simp only [unwrapWhole]
    rw [exact.2, Nat.mul_sub_left_distrib]

/-- The only semantic quotient/remainder result. `changeShards` remains the
same Token-owned claim-shard instrument. -/
structure Division where
  inputShards : Nat
  wholeNativeClaims : Nat
  consumedShards : Nat
  changeShards : Nat
  deriving DecidableEq, Repr

/-- **The sole claim-shard quotient/remainder boundary.** -/
def divideClaimShardsV1 (denominator inputShards : Nat) : Option Division :=
  if 0 < denominator then
    some {
      inputShards
      wholeNativeClaims := inputShards / denominator
      consumedShards := denominator * (inputShards / denominator)
      changeShards := inputShards % denominator
    }
  else none

theorem division_is_exact
    (denominator inputShards : Nat) (positive : 0 < denominator) :
    match divideClaimShardsV1 denominator inputShards with
    | some result =>
        result.inputShards = result.consumedShards + result.changeShards ∧
        result.consumedShards = denominator * result.wholeNativeClaims ∧
        result.changeShards < denominator
    | none => False := by
  simp only [divideClaimShardsV1, if_pos positive]
  exact ⟨(Nat.div_add_mod inputShards denominator).symm,
    trivial, Nat.mod_lt inputShards positive⟩

theorem subdenominator_input_is_explicit_change
    (denominator inputShards : Nat) (positive : 0 < denominator)
    (small : inputShards < denominator) :
    divideClaimShardsV1 denominator inputShards = some {
      inputShards
      wholeNativeClaims := 0
      consumedShards := 0
      changeShards := inputShards
    } := by
  simp [divideClaimShardsV1, positive, Nat.div_eq_of_lt small, Nat.mod_eq_of_lt small]

/-- Authenticated lifecycle projection; the winner comes from Market terminal
state, never from a caller-selected payout branch. -/
inductive Phase where
  | open
  | terminal (winningOutcome : Nat)
  | retired
  deriving DecidableEq, Repr

/-- Phase-dependent reserve admission for one Product-owned outcome. -/
def reserveAdmitted
    (denominator outcome : Nat) (phase : Phase) (reserve : Reserve) : Prop :=
  match phase with
  | .open => reserve.exact denominator
  | .terminal winningOutcome =>
      if outcome = winningOutcome then reserve.exact denominator
      else reserve.shardSupply ≤ denominator * reserve.lockedNativeClaims
  | .retired => reserve.lockedNativeClaims = 0 ∧ reserve.shardSupply = 0

/-- Losing-shard zero burn preserves the terminal upper bound and changes no
native custody. -/
def burnLosingShards (quantity : Nat) (reserve : Reserve) : Reserve := {
  reserve with shardSupply := reserve.shardSupply - quantity
}

theorem losing_burn_preserves_upper_bound
    (denominator quantity : Nat) (reserve : Reserve)
    (bounded : reserve.shardSupply ≤ denominator * reserve.lockedNativeClaims) :
    (burnLosingShards quantity reserve).shardSupply ≤
      denominator * (burnLosingShards quantity reserve).lockedNativeClaims := by
  simp only [burnLosingShards]
  exact Nat.le_trans (Nat.sub_le _ _) bounded

/-- A funded winning burn cannot redeem more native claims than custody owns.
This is the kernel's no-double-redemption arithmetic fact. -/
theorem winning_burn_bounded_by_locked
    (denominator wholeClaims : Nat) (reserve : Reserve)
    (positive : 0 < denominator)
    (exactSupply : reserve.shardSupply = denominator * reserve.lockedNativeClaims)
    (funded : denominator * wholeClaims ≤ reserve.shardSupply) :
    wholeClaims ≤ reserve.lockedNativeClaims := by
  rw [exactSupply] at funded
  exact Nat.le_of_mul_le_mul_left funded positive

/-! ## Lean-owned fixed physical layout -/

namespace PhysicalAbi

def schemaVersion : Nat := 1
def termsMagic : List UInt8 := [0x44, 0x43, 0x46, 0x52, 0x54, 0x52, 0x4d, 0x31]
def projectionMagic : List UInt8 := [0x44, 0x43, 0x46, 0x50, 0x52, 0x4f, 0x4a, 0x31]

def termsHeaderBytes : Nat := 192
def termsMintBytes : Nat := 32
def projectionHeaderBytes : Nat := 96
def projectionRowBytes : Nat := 16

def termsVersionOffset : Nat := 8
def termsReservedAOffset : Nat := 10
def termsReservedABytes : Nat := 6
def termsMarketOffset : Nat := 16
def termsResultDomainOffset : Nat := 48
def termsReleaseSetOffset : Nat := 80
def termsTokenProgramOffset : Nat := 112
def termsTokenBehaviorOffset : Nat := 144
def termsOutcomeCountOffset : Nat := 176
def termsReservedBOffset : Nat := 180
def termsReservedBBytes : Nat := 4
def termsDenominatorOffset : Nat := 184

def projectionVersionOffset : Nat := 8
def projectionPhaseOffset : Nat := 10
def projectionReservedOffset : Nat := 11
def projectionReservedBytes : Nat := 5
def projectionTermsIdOffset : Nat := 16
def projectionMarketOffset : Nat := 48
def projectionOutcomeCountOffset : Nat := 80
def projectionTerminalOutcomeOffset : Nat := 84
def projectionRevisionOffset : Nat := 88
def noTerminalOutcome : Nat := 2 ^ 32 - 1

def termsBytes (outcomeCount : Nat) : Nat :=
  termsHeaderBytes + outcomeCount * termsMintBytes

def projectionBytes (outcomeCount : Nat) : Nat :=
  projectionHeaderBytes + outcomeCount * projectionRowBytes

theorem terms_layout_contiguous :
    termsDenominatorOffset + 8 = termsHeaderBytes := by decide

theorem projection_layout_contiguous :
    projectionRevisionOffset + 8 = projectionHeaderBytes := by decide

theorem terms_width_positive (outcomeCount : Nat) :
    0 < termsBytes outcomeCount := by
  unfold termsBytes termsHeaderBytes termsMintBytes
  omega

theorem projection_width_positive (outcomeCount : Nat) :
    0 < projectionBytes outcomeCount := by
  unfold projectionBytes projectionHeaderBytes projectionRowBytes
  omega

end PhysicalAbi

end DClutch.FractionalClaimV1
