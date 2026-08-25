import DClutchSemantics.EconomicKernel

/-!
# Executable hostile cases for the shared economic microkernel

These fixtures exercise the Lean program itself.  They are semantic regression
evidence, not Solana-adapter or deployment evidence.
-/

namespace DClutch.Economic.Examples

open DClutch

def limit : Nat := 18446744073709551616

def bindings : Bindings := {
  source := .seller
  destination := .buyer
  hoard := .venue
}

def emptyOpen : State := {
  phase := .open
  hoard := 0
  supply := [0, 0, 0]
  nativeSupply := [0, 0, 0]
  materializedSupply := [0, 0, 0]
  sourceNative := [0, 0, 0]
  sourceMaterialized := [0, 0, 0]
  destinationNative := [0, 0, 0]
  destinationMaterialized := [0, 0, 0]
}

def splitFrame : Frame := {
  outcomeCount := 3
  scalarLimit := limit
  bindings
  pre := emptyOpen
  command := .splitCompleteSet .destination .native 10
}

theorem split_executes_exactly :
    runState splitFrame = {
      emptyOpen with
      hoard := 10
      supply := [10, 10, 10]
      nativeSupply := [10, 10, 10]
      destinationNative := [10, 10, 10]
    } := by
  native_decide

theorem split_emits_width_owned_plan :
    (compile splitFrame).claimEffects.effects.length = 3 ∧
    (compile splitFrame).custodyTransfers = [{
      source := Party.buyer, destination := Party.venue, amount := 10
    }] := by
  native_decide

def mergeFrame : Frame := {
  splitFrame with
  pre := runState splitFrame
  command := .mergeCompleteSet .destination .native 10
}

theorem split_merge_round_trip_is_exact :
    runState mergeFrame = emptyOpen ∧
    (compile mergeFrame).claimEffects.effects.length = 3 ∧
    (compile mergeFrame).custodyTransfers = [{
      source := Party.venue, destination := Party.buyer, amount := 10
    }] := by
  native_decide

def hostileMergeOverdraw : Frame := {
  mergeFrame with command := .mergeCompleteSet .destination .native 11
}

def hostileSplitOverflow : Frame := {
  splitFrame with
  pre := { emptyOpen with
    hoard := limit - 1
    supply := [limit - 1, limit - 1, limit - 1]
    nativeSupply := [limit - 1, limit - 1, limit - 1]
    destinationNative := [limit - 1, limit - 1, limit - 1] }
  command := .splitCompleteSet .destination .native 1
}

theorem hostile_complete_set_cases_refuse_and_roll_back :
    accepts hostileMergeOverdraw = false ∧
    runState hostileMergeOverdraw = hostileMergeOverdraw.pre ∧
    accepts hostileSplitOverflow = false ∧
    runState hostileSplitOverflow = hostileSplitOverflow.pre := by
  native_decide

def transferPre : State := {
  emptyOpen with
  hoard := 10
  supply := [10, 10, 10]
  nativeSupply := [10, 10, 10]
  sourceNative := [7, 7, 7]
  destinationNative := [3, 3, 3]
}

def transferFrame : Frame := {
  splitFrame with
  pre := transferPre
  command := .transferClaim .native 2 5
}

def materializeFrame : Frame := {
  transferFrame with
  command := .materializeClaim 1 4
}

theorem materialization_executes_exactly :
    (runState materializeFrame).supply = [10, 10, 10] ∧
    (runState materializeFrame).hoard = 10 ∧
    (runState materializeFrame).nativeSupply = [10, 6, 10] ∧
    (runState materializeFrame).materializedSupply = [0, 4, 0] ∧
    (runState materializeFrame).sourceNative = [7, 3, 7] ∧
    (runState materializeFrame).destinationMaterialized = [0, 4, 0] := by
  native_decide

def dematerializePre : State := {
  (runState materializeFrame) with
  sourceNative := [3, 3, 3]
  sourceMaterialized := [0, 4, 0]
  destinationNative := [7, 3, 7]
  destinationMaterialized := [0, 0, 0]
}

def dematerializeFrame : Frame := {
  materializeFrame with
  pre := dematerializePre
  command := .dematerializeClaim 1 4
}

theorem representation_round_trip_preserves_supply_and_hoard :
    (runState dematerializeFrame).nativeSupply = transferPre.nativeSupply ∧
    (runState dematerializeFrame).materializedSupply = transferPre.materializedSupply ∧
    (runState dematerializeFrame).supply = transferPre.supply ∧
    (runState dematerializeFrame).hoard = transferPre.hoard := by
  native_decide

/- This hostile fixture tries to materialize from the empty source projection
after a split credited the destination projection. -/
def wrongHolderMaterialization : Frame := {
  splitFrame with
  pre := runState splitFrame
  command := .materializeClaim 1 4
}

theorem wrong_holder_materialization_refuses_and_rolls_back :
    accepts wrongHolderMaterialization = false ∧
    runState wrongHolderMaterialization = wrongHolderMaterialization.pre := by
  native_decide

theorem transfer_is_liability_neutral :
    (runState transferFrame).sourceNative = [7, 7, 2] ∧
    (runState transferFrame).destinationNative = [3, 3, 8] ∧
    (runState transferFrame).supply = transferPre.supply ∧
    (runState transferFrame).hoard = transferPre.hoard := by
  native_decide

def hostileOverdraw : Frame := {
  transferFrame with command := .transferClaim .native 2 8
}

def hostileOutcome : Frame := {
  transferFrame with command := .transferClaim .native 3 1
}

def hostileAlias : Frame := {
  transferFrame with
  bindings := { bindings with destination := .seller }
}

theorem hostile_claim_moves_refuse_and_roll_back :
    accepts hostileOverdraw = false ∧
    runState hostileOverdraw = hostileOverdraw.pre ∧
    accepts hostileOutcome = false ∧
    runState hostileOutcome = hostileOutcome.pre ∧
    accepts hostileAlias = false ∧
    runState hostileAlias = hostileAlias.pre := by
  native_decide

def winningTerminal : State := {
  phase := .retiring 1
  hoard := 6
  supply := [0, 6, 0]
  nativeSupply := [0, 6, 0]
  materializedSupply := [0, 0, 0]
  sourceNative := [0, 6, 0]
  sourceMaterialized := [0, 0, 0]
  destinationNative := [0, 0, 0]
  destinationMaterialized := [0, 0, 0]
}

def redeemWinner : Frame := {
  splitFrame with
  pre := winningTerminal
  command := .redeemTerminal .source .native 1 6
}

theorem winning_redemption_retires_exact_backing :
    (runState redeemWinner).hoard = 0 ∧
    (runState redeemWinner).supply = [0, 0, 0] ∧
    (compile redeemWinner).custodyTransfers = [{
      source := Party.venue, destination := Party.seller, amount := 6
    }] := by
  native_decide

def losingTerminal : State := {
  winningTerminal with
  hoard := 6
  supply := [4, 6, 0]
  nativeSupply := [4, 6, 0]
  sourceNative := [4, 6, 0]
}

def redeemLoser : Frame := {
  redeemWinner with
  pre := losingTerminal
  command := .redeemTerminal .source .native 0 4
}

theorem losing_redemption_burns_without_touching_hoard :
    (runState redeemLoser).hoard = 6 ∧
    (runState redeemLoser).supply = [0, 6, 0] ∧
    (compile redeemLoser).custodyTransfers = [] := by
  native_decide

def retireEmpty : Frame := {
  redeemWinner with
  pre := runState redeemWinner
  command := .retireTerminal
}

theorem empty_terminal_retires :
    (runState retireEmpty).phase = .retired ∧
    (runState retireEmpty).hoard = 0 ∧
    (runState retireEmpty).supply = [0, 0, 0] := by
  native_decide

def hostileEarlyRetirement : Frame := {
  redeemWinner with command := .retireTerminal
}

def hostileWinningOverdraw : Frame := {
  redeemWinner with command := .redeemTerminal .source .native 1 7
}

theorem hostile_terminal_cases_refuse_and_roll_back :
    accepts hostileEarlyRetirement = false ∧
    runState hostileEarlyRetirement = hostileEarlyRetirement.pre ∧
    accepts hostileWinningOverdraw = false ∧
    runState hostileWinningOverdraw = hostileWinningOverdraw.pre := by
  native_decide

end DClutch.Economic.Examples
