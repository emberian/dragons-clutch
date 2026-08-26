import DClutchSemantics.DealerLiquidity

/-!
# Executable Dealer liquidity cases

These cases execute the pure total machine, including its shared
EconomicKernel link.  They are semantic regression evidence, not evidence for
the Solana adapter, SPL CPI, or atomic multiprogram rollback.
-/

namespace DClutch.Dealer.Examples

open DClutch

def limit : Nat := 18446744073709551616

def policy : Policy := {
  marketId := 1
  releaseSetId := 13
  dealerId := 7
  feeRecipientId := 11
  unwindRecipientId := 12
  outcomeCount := 2
  quoteScale := 100
  feeNumerator := 1
  feeDenominator := 100
  minimumWorkFunding := 50
  replacementDelay := 5
  scalarLimit := limit
}

def releaseBinding (program artifact semantic : Nat) :
    DClutch.ExecutionRelease.Binding := {
  program
  artifactRelease := artifact
  semanticRelease := semantic
}

def selectedRelease : DClutch.ExecutionRelease.ReleaseSet := {
  releaseSetId := 13
  core := releaseBinding 1 101 201
  claims := releaseBinding 2 102 202
  trading := releaseBinding 3 103 203
  resolution := releaseBinding 4 104 204
  custody := releaseBinding 5 105 205
}

def tradingAdmission : DClutch.ExecutionRelease.Admission := {
  marketRegistryProgram := 1
  marketReleaseSetId := 13
  selected := selectedRelease
  receipt := {
    registryProgram := 1
    releaseSetId := 13
    role := .trading
    observed := selectedRelease.trading
    activationCacheAuthenticated := true
    currentDeploymentReauthenticated := true
  }
}

def invoke (command : Command) : Invocation := {
  release := tradingAdmission
  command
}

def liquidCurve : OutcomeCurve := {
  bids := [{ capacity := 100, priceNumerator := 40 }]
  asks := [{ capacity := 100, priceNumerator := 60 }]
}

def candidateV1 : Candidate := {
  candidateId := 101
  revision := 1
  validFrom := 0
  expiresAt := 1000
  curves := [liquidCurve, liquidCurve]
  minimumInventory := [0, 0]
  maximumInventory := [100, 100]
  quoteReserveFloor := 100
  workFunding := 100
  workReward := 2
}

def initial : State := {
  phase := .open
  active := candidateV1
  pending := none
  inventory := [50, 50]
  buyUsed := [0, 0]
  sellUsed := [0, 0]
  buyQuotePaid := [0, 0]
  sellQuotePaid := [0, 0]
  feeBase := 0
  feePaid := 0
  quoteCustody := 1000
  feeCustody := 0
  livenessCustody := 100
  activeWorkRemaining := 100
  pendingWorkFunding := 0
}

theorem initial_state_is_valid : valid policy initial = true := by
  native_decide

def buyEconomicPre : DClutch.Economic.State := {
  phase := .open
  hoard := 100
  supply := [100, 100]
  nativeSupply := [100, 100]
  materializedSupply := [0, 0]
  sourceNative := [50, 50]
  sourceMaterialized := [0, 0]
  destinationNative := [50, 50]
  destinationMaterialized := [0, 0]
}

def buyEconomic (quantity : Nat) : DClutch.Economic.Frame := {
  outcomeCount := 2
  scalarLimit := limit
  bindings := { source := .seller, destination := .buyer, hoard := .venue }
  pre := buyEconomicPre
  command := .transferClaim .native 0 quantity
}

def buyTen : Fill := {
  now := 10
  expectedCandidateId := 101
  expectedRevision := 1
  side := .takerBuys
  outcome := 0
  quantity := 10
  economic := buyEconomic 10
}

theorem full_fill_executes_claims_custody_fee_and_work_exactly :
    accepts policy initial (invoke (.fill buyTen)) = true ∧
    let post := run policy initial (invoke (.fill buyTen))
    post.inventory = [40, 50] ∧
    post.buyUsed = [10, 0] ∧ post.buyQuotePaid = [6, 0] ∧
    post.quoteCustody = 1006 ∧ post.feeBase = 6 ∧
    post.feePaid = 1 ∧ post.feeCustody = 1 ∧
    post.activeWorkRemaining = 98 ∧ post.livenessCustody = 98 ∧
    (physicalPlan policy initial (.fill buyTen)).custody = [
      custodyMove .takerQuote .dealerQuote 6,
      custodyMove .takerQuote .feeVault 1,
      custodyMove .livenessVault .executor 2] := by
  native_decide

def buyFour : Fill := {
  buyTen with
  quantity := 4
  economic := buyEconomic 4
}

def afterBuyFour : State := run policy initial (invoke (.fill buyFour))

def buySixEconomicPre : DClutch.Economic.State := {
  buyEconomicPre with
  sourceNative := [46, 50]
  destinationNative := [54, 50]
}

def buySix : Fill := {
  buyTen with
  quantity := 6
  economic := { buyEconomic 6 with pre := buySixEconomicPre }
}

/-- Both quote rounding and the percentage fee are cumulative.  A 4+6 split
therefore produces the exact same aggregate debit and fee as one fill of 10. -/
theorem fragmented_fill_cannot_multiply_rounding_or_fees :
    accepts policy initial (invoke (.fill buyFour)) = true ∧
    accepts policy afterBuyFour (invoke (.fill buySix)) = true ∧
    let fragmented := run policy afterBuyFour (invoke (.fill buySix))
    let whole := run policy initial (invoke (.fill buyTen))
    fragmented.inventory = whole.inventory ∧
    fragmented.buyQuotePaid = whole.buyQuotePaid ∧
    fragmented.feeBase = whole.feeBase ∧
    fragmented.feePaid = whole.feePaid ∧
    fragmented.quoteCustody = whole.quoteCustody ∧
    fragmented.feeCustody = whole.feeCustody := by
  native_decide

def hostileResetPaid : State := {
  afterBuyFour with buyQuotePaid := [0, 0]
}

def hostileStaleCandidate : Fill := {
  buyTen with expectedCandidateId := 99
}

def hostileUnderfundedWork : State := {
  initial with
  livenessCustody := 1
  activeWorkRemaining := 1
}

def hostileQuoteOverflow : State := {
  initial with quoteCustody := limit - 1
}

def substitutedRelease : Invocation := {
  release := { tradingAdmission with marketReleaseSetId := 14 }
  command := .fill buyTen
}

def unauthenticatedRelease : Invocation := {
  release := {
    tradingAdmission with
    receipt := {
      tradingAdmission.receipt with currentDeploymentReauthenticated := false
    }
  }
  command := .fill buyTen
}

theorem hostile_rounding_replay_stale_release_and_underfunded_work_refuse :
    accepts policy hostileResetPaid (invoke (.fill buySix)) = false ∧
    run policy hostileResetPaid (invoke (.fill buySix)) = hostileResetPaid ∧
    accepts policy initial (invoke (.fill hostileStaleCandidate)) = false ∧
    run policy initial (invoke (.fill hostileStaleCandidate)) = initial ∧
    valid policy hostileUnderfundedWork = true ∧
    accepts policy hostileUnderfundedWork (invoke (.fill buyTen)) = false ∧
    run policy hostileUnderfundedWork (invoke (.fill buyTen)) = hostileUnderfundedWork ∧
    valid policy hostileQuoteOverflow = true ∧
    accepts policy hostileQuoteOverflow (invoke (.fill buyTen)) = false := by
  native_decide

theorem release_substitution_and_unauthenticated_deployment_refuse :
    accepts policy initial substitutedRelease = false ∧
    run policy initial substitutedRelease = initial ∧
    accepts policy initial unauthenticatedRelease = false ∧
    run policy initial unauthenticatedRelease = initial := by
  native_decide

def candidateV2 : Candidate := {
  candidateV1 with
  candidateId := 102
  revision := 2
  validFrom := 20
  expiresAt := 2000
}

def scheduleV2 : Replacement := {
  authenticatedDealerId := 7
  now := 10
  candidate := candidateV2
  fundingDeposit := 100
}

def scheduled : State := run policy initial (invoke (.scheduleReplacement scheduleV2))

theorem replacement_is_precommitted_prepaid_and_revision_ordered :
    accepts policy initial (invoke (.scheduleReplacement scheduleV2)) = true ∧
    scheduled.pending = some candidateV2 ∧
    scheduled.pendingWorkFunding = 100 ∧ scheduled.livenessCustody = 200 ∧
    (physicalPlan policy initial (.scheduleReplacement scheduleV2)).custody = [
      custodyMove .dealerOwner .livenessVault 100] ∧
    accepts policy scheduled (invoke (.activateReplacement { now := 19 })) = false ∧
    accepts policy scheduled (invoke (.activateReplacement { now := 20 })) = true ∧
    let activated := run policy scheduled (invoke (.activateReplacement { now := 20 }))
    activated.active = candidateV2 ∧ activated.pending = none ∧
    activated.buyUsed = [0, 0] ∧ activated.sellUsed = [0, 0] ∧
    activated.activeWorkRemaining = 100 ∧ activated.livenessCustody = 100 := by
  native_decide

def staleReplacement : Replacement := {
  scheduleV2 with
  candidate := { candidateV2 with candidateId := 103, revision := 1 }
}

theorem stale_or_unauthenticated_replacement_refuses :
    accepts policy initial (invoke (.scheduleReplacement staleReplacement)) = false ∧
    accepts policy initial (invoke (.scheduleReplacement
      { scheduleV2 with authenticatedDealerId := 8 })) = false := by
  native_decide

def enterTerminal : Resolution := {
  coreMarketId := 1
  releaseSetId := 13
  winner := 0
}

def terminal : State := run policy initial (invoke (.enterTerminal enterTerminal))

def winningUnwindEconomic : DClutch.Economic.Frame := {
  outcomeCount := 2
  scalarLimit := limit
  bindings := { source := .seller, destination := .buyer, hoard := .venue }
  pre := {
    phase := .retiring 0
    hoard := 50
    supply := [50, 50]
    nativeSupply := [50, 50]
    materializedSupply := [0, 0]
    sourceNative := [50, 50]
    sourceMaterialized := [0, 0]
    destinationNative := [0, 0]
    destinationMaterialized := [0, 0]
  }
  command := .redeemTerminal .source .native 0 50
}

def winningUnwind : Unwind := {
  outcome := 0
  quantity := 50
  economic := winningUnwindEconomic
}

def afterWinner : State := run policy terminal (invoke (.unwind winningUnwind))

def losingUnwindEconomic : DClutch.Economic.Frame := {
  winningUnwindEconomic with
  pre := {
    phase := .retiring 0
    hoard := 0
    supply := [0, 50]
    nativeSupply := [0, 50]
    materializedSupply := [0, 0]
    sourceNative := [0, 50]
    sourceMaterialized := [0, 0]
    destinationNative := [0, 0]
    destinationMaterialized := [0, 0]
  }
  command := .redeemTerminal .source .native 1 50
}

def losingUnwind : Unwind := {
  outcome := 1
  quantity := 50
  economic := losingUnwindEconomic
}

def emptyTerminal : State := run policy afterWinner (invoke (.unwind losingUnwind))

theorem terminal_unwind_redeems_winner_burns_loser_and_pays_real_work :
    accepts policy initial (invoke (.enterTerminal enterTerminal)) = true ∧
    accepts policy terminal (invoke (.unwind winningUnwind)) = true ∧
    afterWinner.inventory = [0, 50] ∧ afterWinner.quoteCustody = 1050 ∧
    afterWinner.activeWorkRemaining = 98 ∧
    accepts policy afterWinner (invoke (.unwind losingUnwind)) = true ∧
    emptyTerminal.inventory = [0, 0] ∧
    emptyTerminal.quoteCustody = 1050 ∧
    emptyTerminal.activeWorkRemaining = 96 ∧
    (physicalPlan policy terminal (.unwind winningUnwind)).custody = [
      custodyMove .livenessVault .executor 2] := by
  native_decide

def retired : State := run policy emptyTerminal (invoke .retire)

theorem terminal_retirement_closes_every_custody_compartment :
    accepts policy emptyTerminal (invoke .retire) = true ∧
    retired.phase = .retired ∧ valid policy retired = true ∧
    retired.quoteCustody = 0 ∧ retired.feeCustody = 0 ∧
    retired.livenessCustody = 0 ∧
    (physicalPlan policy emptyTerminal .retire).custody = [
      custodyMove .dealerQuote .unwindRecipient 1050,
      custodyMove .livenessVault .dealerOwner 96] := by
  native_decide

def hostileEarlyRetirement : Command := .retire

def hostileWrongCoreMarket : Command := .enterTerminal {
  coreMarketId := 8
  releaseSetId := 13
  winner := 0
}

def hostileWrongTerminalRelease : Command := .enterTerminal {
  coreMarketId := 1
  releaseSetId := 14
  winner := 0
}

theorem hostile_terminal_source_and_early_retirement_refuse :
    accepts policy initial (invoke hostileEarlyRetirement) = false ∧
    run policy initial (invoke hostileEarlyRetirement) = initial ∧
    accepts policy initial (invoke hostileWrongCoreMarket) = false ∧
    run policy initial (invoke hostileWrongCoreMarket) = initial ∧
    accepts policy initial (invoke hostileWrongTerminalRelease) = false ∧
    run policy initial (invoke hostileWrongTerminalRelease) = initial := by
  native_decide

end DClutch.Dealer.Examples
