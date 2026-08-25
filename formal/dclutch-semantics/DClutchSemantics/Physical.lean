import DClutchSemantics.DirectProofs
import DClutchSemantics.Codec

/-!
# Multiprogram physical plan

The high-level Direct ledger intentionally talks about claims and collateral in
one state. The Solana successor must not pretend SPL collateral is an internal
integer. This file derives two disjoint physical plans from the one admitted
transition: replay/claim effects for a program-owned executor, and exact token
transfers for a custody adapter.
-/

namespace DClutch.Direct.Physical

open DClutch
open DClutch.Direct

/-- Program-owned replay and native-claim projection. -/
structure ClaimState where
  sellerNextNonce : Nat
  buyerNextNonce : Nat
  sellerClaims : Nat
  buyerClaims : Nat
  deriving DecidableEq, Repr

/-- Realm-selected SPL collateral projection used only for semantic refinement. -/
structure CustodyState where
  buyerCollateral : Nat
  sellerCollateral : Nat
  venueCollateral : Nat
  deriving DecidableEq, Repr

/-- One exact custody transfer. Debit and credit cannot be separated. -/
structure CustodyTransfer where
  source : Party
  destination : Party
  amount : Nat
  deriving DecidableEq, Repr

/-- Disjoint program inputs derived from one admitted Direct frame. -/
structure PhysicalPlan where
  claimEffects : EffectPlan
  custodyTransfers : List CustodyTransfer
  deriving DecidableEq, Repr

def claimPre (frame : FillFrame) : ClaimState := {
  sellerNextNonce := frame.pre.sellerNextNonce
  buyerNextNonce := frame.pre.buyerNextNonce
  sellerClaims := frame.pre.sellerClaims
  buyerClaims := frame.pre.buyerClaims
}

def custodyPre (frame : FillFrame) : CustodyState := {
  buyerCollateral := frame.pre.buyerCollateral
  sellerCollateral := frame.pre.sellerCollateral
  venueCollateral := frame.pre.venueCollateral
}

def claimPost (frame : FillFrame) : ClaimState := {
  sellerNextNonce := frame.pre.sellerNextNonce + 1
  buyerNextNonce := frame.pre.buyerNextNonce + 1
  sellerClaims := frame.pre.sellerClaims - frame.fill
  buyerClaims := frame.pre.buyerClaims + frame.fill
}

def custodyPost (frame : FillFrame) : CustodyState := {
  buyerCollateral := frame.pre.buyerCollateral - (frame.gross + frame.fee)
  sellerCollateral := frame.pre.sellerCollateral + frame.gross
  venueCollateral := frame.pre.venueCollateral + frame.fee
}

/-- The compiler projection: four state effects and two indivisible transfers. -/
def physicalPlan (frame : FillFrame) : PhysicalPlan := {
  claimEffects := {
    effects := [
      .set sellerReplayCell (frame.pre.sellerNextNonce + 1),
      .set buyerReplayCell (frame.pre.buyerNextNonce + 1),
      .debit (sellerClaimCell frame.sellerIntent.outcome) frame.fill,
      .credit (buyerClaimCell frame.sellerIntent.outcome) frame.fill
    ]
  }
  custodyTransfers := [
    { source := .buyer, destination := .seller, amount := frame.gross },
    { source := .buyer, destination := .venue, amount := frame.fee }
  ]
}

theorem physical_plan_shape (frame : FillFrame) :
    (physicalPlan frame).claimEffects.effects.length = 4 ∧
    (physicalPlan frame).custodyTransfers.length = 2 := by
  simp [physicalPlan]

namespace Codec

def custodyMagic : List UInt8 := [0x44, 0x43, 0x43, 0x50] -- `DCCP`
def custodyVersion : UInt8 := 1
def custodyHeaderBytes : Nat := 8
def custodyTransferBytes : Nat := 16

def encodeCustodyHeader (count : Nat) : List UInt8 :=
  custodyMagic ++ [custodyVersion, UInt8.ofNat count, 0, 0]

def encodeCustodyTransfer (transfer : CustodyTransfer) : List UInt8 :=
  [DClutch.Codec.partyTag transfer.source,
    DClutch.Codec.partyTag transfer.destination, 0, 0, 0, 0, 0, 0] ++
    DClutch.Codec.encodeLE 8 transfer.amount

theorem encode_custody_header_length (count : Nat) :
    (encodeCustodyHeader count).length = custodyHeaderBytes := by
  simp [encodeCustodyHeader, custodyMagic, custodyHeaderBytes]

theorem encode_custody_transfer_length (transfer : CustodyTransfer) :
    (encodeCustodyTransfer transfer).length = custodyTransferBytes := by
  simp [encodeCustodyTransfer, custodyTransferBytes, DClutch.Codec.encodeLE_length]

def encodeCustodyPlan (transfers : List CustodyTransfer) : List UInt8 :=
  encodeCustodyHeader transfers.length ++ transfers.flatMap encodeCustodyTransfer

private theorem flatMap_transfer_length : ∀ transfers : List CustodyTransfer,
    (transfers.flatMap encodeCustodyTransfer).length =
      transfers.length * custodyTransferBytes
  | [] => by simp
  | transfer :: rest => by
      simp [encode_custody_transfer_length, flatMap_transfer_length rest,
        custodyTransferBytes]
      omega

theorem encode_custody_plan_length (transfers : List CustodyTransfer) :
    (encodeCustodyPlan transfers).length =
      custodyHeaderBytes + transfers.length * custodyTransferBytes := by
  unfold encodeCustodyPlan
  rw [List.length_append, encode_custody_header_length, flatMap_transfer_length]

end Codec

def applyClaimEffect (outcome : Nat) (state : ClaimState) (effect : Effect) : Option ClaimState :=
  match effect with
  | .set cell value =>
      if value < u64Limit then
        match cell.party, cell.resource with
        | .seller, .replayNonce => some { state with sellerNextNonce := value }
        | .buyer, .replayNonce => some { state with buyerNextNonce := value }
        | _, _ => none
      else none
  | .debit cell amount =>
      match cell.party, cell.resource with
      | .seller, .outcomeClaim selected =>
          if selected = outcome then
            (checkedDebit state.sellerClaims amount).map fun value =>
              { state with sellerClaims := value }
          else none
      | _, _ => none
  | .credit cell amount =>
      match cell.party, cell.resource with
      | .buyer, .outcomeClaim selected =>
          if selected = outcome then
            (checkedCredit state.buyerClaims amount).map fun value =>
              { state with buyerClaims := value }
          else none
      | _, _ => none

def runClaimEffects (outcome : Nat) : List Effect → ClaimState → Option ClaimState
  | [], state => some state
  | effect :: rest, state =>
      (applyClaimEffect outcome state effect).bind (runClaimEffects outcome rest)

def applyCustodyTransfer
    (state : CustodyState) (transfer : CustodyTransfer) : Option CustodyState :=
  match transfer.source, transfer.destination with
  | .buyer, .seller => do
      let buyer ← checkedDebit state.buyerCollateral transfer.amount
      let seller ← checkedCredit state.sellerCollateral transfer.amount
      some { state with buyerCollateral := buyer, sellerCollateral := seller }
  | .buyer, .venue => do
      let buyer ← checkedDebit state.buyerCollateral transfer.amount
      let venue ← checkedCredit state.venueCollateral transfer.amount
      some { state with buyerCollateral := buyer, venueCollateral := venue }
  | _, _ => none

def runCustodyTransfers : List CustodyTransfer → CustodyState → Option CustodyState
  | [], state => some state
  | transfer :: rest, state =>
      (applyCustodyTransfer state transfer).bind (runCustodyTransfers rest)

/-- Join disjoint physical projections back into the semantic ledger. -/
def join (claims : ClaimState) (custody : CustodyState) : Ledger := {
  sellerNextNonce := claims.sellerNextNonce
  buyerNextNonce := claims.buyerNextNonce
  sellerClaims := claims.sellerClaims
  buyerClaims := claims.buyerClaims
  buyerCollateral := custody.buyerCollateral
  sellerCollateral := custody.sellerCollateral
  venueCollateral := custody.venueCollateral
}

theorem pre_join (frame : FillFrame) :
    join (claimPre frame) (custodyPre frame) = frame.pre := by
  rfl

theorem post_join (frame : FillFrame) :
    join (claimPost frame) (custodyPost frame) = postState frame := by
  rfl

theorem claim_plan_refines (frame : FillFrame) (admitted : Admissible frame) :
    runClaimEffects frame.sellerIntent.outcome
      (physicalPlan frame).claimEffects.effects (claimPre frame) =
        some (claimPost frame) := by
  simp only [physicalPlan, runClaimEffects, applyClaimEffect, sellerReplayCell,
    buyerReplayCell, sellerClaimCell, buyerClaimCell, claimPre, claimPost,
    checkedCredit, checkedDebit]
  simp [admitted.sellerNonceCanAdvance, admitted.buyerNonceCanAdvance,
    admitted.sellerHasClaims, admitted.buyerClaimCreditFits]

theorem custody_plan_refines (frame : FillFrame) (admitted : Admissible frame) :
    runCustodyTransfers (physicalPlan frame).custodyTransfers (custodyPre frame) =
      some (custodyPost frame) := by
  have buyerHasCollateral := admitted.buyerHasCollateral
  have grossAvailable : frame.gross ≤ frame.pre.buyerCollateral := by
    omega
  have feeAvailable :
      frame.fee ≤ frame.pre.buyerCollateral - frame.gross := by
    omega
  simp only [physicalPlan, runCustodyTransfers, applyCustodyTransfer, custodyPre,
    custodyPost, checkedCredit, checkedDebit]
  simp [grossAvailable, feeAvailable, admitted.sellerCollateralCreditFits,
    admitted.venueCreditFits]
  omega

theorem custody_plan_conserves (frame : FillFrame) (admitted : Admissible frame) :
    (custodyPost frame).buyerCollateral +
        (custodyPost frame).sellerCollateral +
        (custodyPost frame).venueCollateral =
      (custodyPre frame).buyerCollateral +
        (custodyPre frame).sellerCollateral +
        (custodyPre frame).venueCollateral := by
  simp only [custodyPost, custodyPre]
  have := admitted.buyerHasCollateral
  omega

/-- The two successful program projections denote exactly the one semantic post-state. -/
theorem physical_plan_refines (frame : FillFrame) (admitted : Admissible frame) :
    runClaimEffects frame.sellerIntent.outcome
        (physicalPlan frame).claimEffects.effects (claimPre frame) =
          some (claimPost frame) ∧
      runCustodyTransfers (physicalPlan frame).custodyTransfers (custodyPre frame) =
          some (custodyPost frame) ∧
      join (claimPost frame) (custodyPost frame) = postState frame := by
  exact ⟨claim_plan_refines frame admitted, custody_plan_refines frame admitted,
    post_join frame⟩

/-- Abstract transaction envelope: any failed child call exposes the original ledger. -/
def atomicCommit
    (pre : Ledger) (claims : Option ClaimState) (custody : Option CustodyState) : Ledger :=
  match claims, custody with
  | some claimState, some custodyState => join claimState custodyState
  | _, _ => pre

theorem claim_refusal_rolls_back (pre : Ledger) (custody : Option CustodyState) :
    atomicCommit pre none custody = pre := by
  cases custody <;> rfl

theorem custody_refusal_rolls_back (pre : Ledger) (claims : Option ClaimState) :
    atomicCommit pre claims none = pre := by
  cases claims <;> rfl

theorem successful_atomic_commit (frame : FillFrame) :
    atomicCommit frame.pre (some (claimPost frame)) (some (custodyPost frame)) =
      postState frame := by
  simp [atomicCommit, post_join]

end DClutch.Direct.Physical
